use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
    response::Response,
    routing::post,
};
use ngs_olezhek::{
    App, ClientError, FieldErrors, LuaType, Valid, Validate, include_lua, lua_types,
    security::{Session, Sessions},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tower::ServiceExt;

async fn send(router: &Router, req: Request<Body>) -> Response {
    router.clone().oneshot(req).await.unwrap()
}

async fn json_body(res: Response) -> Value {
    serde_json::from_slice(&to_bytes(res.into_body(), usize::MAX).await.unwrap()).unwrap()
}

fn post_json(path: &str, body: &str) -> Request<Body> {
    Request::post(path)
        .header(header::CONTENT_TYPE, "application/json")
        .header("sec-fetch-site", "same-origin")
        .body(Body::from(body.to_owned()))
        .unwrap()
}

#[derive(Deserialize)]
struct Signup {
    name: String,
    age: u32,
}

impl Validate for Signup {
    fn validate(&self, errors: &mut FieldErrors) {
        errors.check(!self.name.trim().is_empty(), "name", "cannot be empty");
        errors.check(self.age >= 18, "age", "must be 18 or older");
    }
}

async fn signup(Valid(s): Valid<Signup>) -> Result<String, FieldErrors> {
    if s.name == "taken" {
        return Err(FieldErrors::new().field("name", "already taken").message("Try another name"));
    }
    Ok(format!("hello {}", s.name))
}

fn form_app() -> Router {
    App::new()
        .dev(false)
        .lua(include_lua!("tests/fixtures/lua"))
        .api(Router::new().route("/api/signup", post(signup)))
        .into_router()
        .unwrap()
}

#[tokio::test]
async fn valid_answers_422_with_field_errors() {
    let r = form_app();
    let res = send(&r, post_json("/api/signup", r#"{"name":" ","age":12}"#)).await;
    assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(json_body(res).await, json!({ "errors": { "name": ["cannot be empty"], "age": ["must be 18 or older"] } }));

    let res = send(&r, post_json("/api/signup", r#"{"name":"a","age":"old"}"#)).await;
    assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let body = json_body(res).await;
    assert!(body["errors"]["age"][0].as_str().unwrap().contains("invalid type"), "{body}");

    let res = send(&r, post_json("/api/signup", r#"{"age":20}"#)).await;
    assert_eq!(json_body(res).await, json!({ "errors": { "name": ["required"] } }));

    let res = send(&r, post_json("/api/signup", r#"{"name":"taken","age":20}"#)).await;
    assert_eq!(json_body(res).await, json!({ "message": "Try another name", "errors": { "name": ["already taken"] } }));

    let res = send(&r, post_json("/api/signup", r#"{"name":"Ann","age":20}"#)).await;
    assert_eq!(res.status(), StatusCode::OK);
}

fn error_app(seen: Arc<Mutex<Vec<ClientError>>>) -> App {
    async fn login(session: Session) -> &'static str {
        session.login(7u64).unwrap();
        "ok"
    }
    App::new()
        .dev(false)
        .lua(include_lua!("tests/fixtures/lua"))
        .sessions(Sessions::memory())
        .api(Router::new().route("/api/login", post(login)))
        .on_client_error(move |e| seen.lock().unwrap().push(e))
}

fn report(body: &str, cookie: Option<&str>) -> Request<Body> {
    let mut b = Request::post("/_ngs/errors").header("sec-fetch-site", "same-origin").header(header::USER_AGENT, "test-agent");
    if let Some(c) = cookie {
        b = b.header(header::COOKIE, c);
    }
    b.body(Body::from(body.to_owned())).unwrap()
}

#[tokio::test]
async fn client_errors_reach_the_handler_with_the_user() {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let r = error_app(seen.clone()).into_router().unwrap();

    let res = send(&r, report(r#"{"page":"/a","errors":[{"message":"boom","count":3}]}"#, None)).await;
    assert_eq!(res.status(), StatusCode::NO_CONTENT);

    let res = send(&r, Request::post("/api/login").header("sec-fetch-site", "same-origin").body(Body::empty()).unwrap()).await;
    let cookie = res.headers()[header::SET_COOKIE].to_str().unwrap().split(';').next().unwrap().to_owned();
    send(&r, report(r#"{"page":"/b","errors":[{"message":"bang"}]}"#, Some(&cookie))).await;

    let seen = seen.lock().unwrap();
    assert_eq!(seen.len(), 2);
    assert_eq!((seen[0].message.as_str(), seen[0].count, seen[0].page.as_str()), ("boom", 3, "/a"));
    assert_eq!(seen[0].user_agent.as_deref(), Some("test-agent"));
    assert_eq!(seen[0].user, None);
    assert_eq!((seen[1].count, seen[1].user.clone()), (1, Some(json!(7))));
}

#[tokio::test]
async fn client_error_endpoint_is_bounded() {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let r = error_app(seen.clone()).into_router().unwrap();
    let many: Vec<_> = (0..50).map(|i| json!({ "message": format!("e{i}") })).collect();
    let body = json!({ "page": "/", "errors": many }).to_string();
    send(&r, report(&body, None)).await;
    assert_eq!(seen.lock().unwrap().len(), 20, "at most 20 entries per report");

    let huge = json!({ "errors": [{ "message": "x".repeat(100_000) }] }).to_string();
    assert_eq!(send(&r, report(&huge, None)).await.status(), StatusCode::PAYLOAD_TOO_LARGE);
    assert_eq!(send(&r, report("not json", None)).await.status(), StatusCode::BAD_REQUEST);
    let cross = Request::post("/_ngs/errors").header("sec-fetch-site", "cross-site").body(Body::from("{}")).unwrap();
    assert_eq!(send(&r, cross).await.status(), StatusCode::FORBIDDEN);

    let mut limited = false;
    for _ in 0..40 {
        if send(&r, report(r#"{"errors":[]}"#, None)).await.status() == StatusCode::TOO_MANY_REQUESTS {
            limited = true;
            break;
        }
    }
    assert!(limited, "the endpoint is rate limited");
}

#[tokio::test]
async fn client_errors_can_be_turned_off_and_are_off_in_dev() {
    let r = error_app(Arc::default()).client_errors(false).into_router().unwrap();
    assert_eq!(send(&r, report(r#"{"errors":[]}"#, None)).await.status(), StatusCode::NOT_FOUND);
    let r = error_app(Arc::default()).dev(true).into_router().unwrap();
    assert_eq!(send(&r, report(r#"{"errors":[]}"#, None)).await.status(), StatusCode::NOT_FOUND);
}

/// A user as the API returns it.
#[derive(Serialize, LuaType)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct ApiUser {
    id: u64,
    /// Shown in the header.
    display_name: String,
    email: Option<String>,
    roles: Vec<ApiRole>,
    #[serde(skip)]
    password_hash: String,
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    settings: HashMap<String, f64>,
    #[serde(rename = "type")]
    kind: (u8, String),
    #[lua(type = "string")]
    avatar: Vec<u8>,
}

#[derive(Serialize, LuaType)]
#[serde(rename_all = "snake_case")]
#[allow(dead_code)]
enum ApiRole {
    /// Can do everything.
    SuperAdmin,
    Player,
}

#[derive(Serialize, LuaType)]
#[serde(tag = "kind")]
#[allow(dead_code)]
enum Shape {
    Circle { r: f64 },
    Empty,
}

#[derive(Serialize, LuaType)]
#[allow(dead_code)]
struct Page<T> {
    items: Vec<T>,
    #[serde(flatten)]
    meta: PageMeta,
}

#[derive(Serialize, LuaType)]
#[allow(dead_code)]
struct PageMeta {
    total: u32,
}

#[derive(Serialize, LuaType)]
#[allow(dead_code)]
struct UserId(u64);

#[test]
fn lua_types_follow_serde() {
    let text = lua_types();
    let section = |start: &str| {
        let from = text.find(start).unwrap_or_else(|| panic!("{start} missing in\n{text}"));
        let rest = &text[from..];
        rest[..rest.find("\n\n").unwrap_or(rest.len())].trim_end().to_string()
    };
    assert!(text.starts_with("---@meta\n"));
    assert_eq!(
        section("--- A user"),
        "--- A user as the API returns it.\n---@class ApiUser\n---@field id integer\n---@field displayName string Shown in the header.\n\
         ---@field email? string\n---@field roles ApiRole[]\n---@field settings? table<string, number>\n\
         ---@field type [integer, string]\n---@field avatar string\n"
            .trim_end()
    );
    assert_eq!(section("---@alias ApiRole"), "---@alias ApiRole\n---| \"super_admin\" # Can do everything.\n---| \"player\"");
    assert_eq!(section("---@alias Shape"), "---@alias Shape\n---| { kind: \"Circle\", r: number }\n---| { kind: \"Empty\" }");
    assert_eq!(section("---@class Page"), "---@class Page<T>: PageMeta\n---@field items T[]");
    assert_eq!(section("---@alias UserId"), "---@alias UserId integer");
}

#[cfg(feature = "sqlite")]
mod sqlite {
    use std::time::Duration;

    use ngs_olezhek::security::{Record, SessionStore, SqliteStore};
    use serde_json::{Map, json};

    fn record(n: u64) -> Record {
        let mut data = Map::new();
        data.insert("n".into(), json!(n));
        Record { data, created: 100, seen: 200 }
    }

    #[tokio::test]
    async fn store_round_trip_and_expiry() {
        let store = SqliteStore::in_memory().unwrap();
        assert!(store.load("k").await.unwrap().is_none());
        store.save("k", &record(1), Duration::from_secs(60)).await.unwrap();
        let r = store.load("k").await.unwrap().unwrap();
        assert_eq!((r.data["n"].clone(), r.created, r.seen), (json!(1), 100, 200));
        store.save("k", &record(2), Duration::from_secs(60)).await.unwrap();
        assert_eq!(store.load("k").await.unwrap().unwrap().data["n"], json!(2));
        store.delete("k").await.unwrap();
        assert!(store.load("k").await.unwrap().is_none());

        store.save("old", &record(3), Duration::ZERO).await.unwrap();
        assert!(store.load("old").await.unwrap().is_none(), "expired rows are not returned");
        assert_eq!(store.sweep().await.unwrap(), 1);
    }

    #[tokio::test]
    async fn sessions_survive_a_restart() {
        let dir = std::env::temp_dir().join(format!("ngs-sessions-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("sessions.db");
        let _ = std::fs::remove_file(&path);
        SqliteStore::open(&path).unwrap().save("k", &record(5), Duration::from_secs(60)).await.unwrap();
        let reopened = SqliteStore::open(&path).unwrap();
        assert_eq!(reopened.load("k").await.unwrap().unwrap().data["n"], json!(5));
        drop(reopened);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn table_names_are_checked() {
        let conn = ngs_olezhek::security::rusqlite::Connection::open_in_memory().unwrap();
        assert!(SqliteStore::with_table(conn, "x; DROP TABLE y").is_err());
    }
}
