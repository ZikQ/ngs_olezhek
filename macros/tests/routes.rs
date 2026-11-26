use axum::{
    Router,
    body::{Body, to_bytes},
    extract::{Path, State},
    http::{Method, Request, StatusCode},
};
use ngs_olezhek_macros::{delete, get, post, route, routes};
use serde::Deserialize;
use tower::ServiceExt;

#[derive(Clone)]
struct AppState {
    greeting: &'static str,
}

#[get("/users/{id}")]
async fn user(id: u64) -> String {
    format!("user {id}")
}

#[get("/users/{user_id}/posts/{post_id}")]
async fn post_by_args_reversed(post_id: u64, user_id: String) -> String {
    format!("{user_id}:{post_id}")
}

#[get("/skip/{a}/{b}/{c}")]
async fn only_middle(b: u8) -> String {
    format!("b={b}")
}

#[get("/files/{*path}")]
async fn file(path: String) -> String {
    format!("file {path}")
}

#[get("/greet/{name}")]
async fn greet(State(state): State<AppState>, name: String) -> String {
    format!("{} {name}", state.greeting)
}

#[get("/manual/{id}")]
async fn manual(Path(id): Path<u32>) -> String {
    format!("manual {id}")
}

#[get("/mut/{n}")]
async fn mutable(mut n: u32) -> String {
    n += 1;
    n.to_string()
}

#[get("/items/{id}")]
async fn item_get(id: u32) -> String {
    format!("get {id}")
}

#[delete("/items/{id}")]
async fn item_delete(id: u32) -> String {
    format!("delete {id}")
}

#[post("/echo/{id}")]
async fn echo(id: u32, body: String) -> String {
    format!("{id}:{body}")
}

#[route("/multi", GET, post)]
async fn multi() -> &'static str {
    "multi"
}

#[get("/braces/{{literal}}")]
async fn braces() -> &'static str {
    "braces"
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "lowercase")]
enum Sort {
    Asc,
    Desc,
}

#[get("/search")]
async fn search(#[query] q: String, #[query] page: Option<u32>) -> String {
    format!("{q}:{page:?}")
}

#[get("/users/{id}/posts")]
async fn user_posts(#[query] limit: Option<u32>, id: u64, #[query] mut skip: u32) -> String {
    skip += 1;
    format!("{id}:{limit:?}:{skip}")
}

#[get("/sorted")]
async fn sorted(#[query] sort: Sort) -> String {
    format!("{sort:?}")
}

#[get("/with-state/{name}")]
async fn query_with_state(State(state): State<AppState>, name: String, #[query] times: u8) -> String {
    format!("{} {name} x{times}", state.greeting)
}

#[post("/echo-query")]
async fn echo_query(#[query] tag: String, body: String) -> String {
    format!("{tag}:{body}")
}

mod api {
    use super::Sort;
    use ngs_olezhek_macros::get;

    #[get("/api/ping")]
    pub async fn ping() -> &'static str {
        "pong"
    }

    #[get("/api/sorted")]
    pub async fn sorted(#[query] sort: Sort) -> String {
        format!("{sort:?}")
    }
}

fn app() -> Router {
    routes![
        user,
        post_by_args_reversed,
        only_middle,
        file,
        manual,
        mutable,
        item_get,
        item_delete,
        echo,
        multi,
        braces,
        api::ping,
        api::sorted,
        search,
        user_posts,
        sorted,
        echo_query,
    ]
}

async fn call(app: Router, method: Method, uri: &str, body: &str) -> (StatusCode, String) {
    let req = Request::builder()
        .method(method)
        .uri(uri)
        .body(Body::from(body.to_owned()))
        .unwrap();
    let res = app.oneshot(req).await.unwrap();
    let status = res.status();
    let bytes = to_bytes(res.into_body(), usize::MAX).await.unwrap();
    (status, String::from_utf8(bytes.to_vec()).unwrap())
}

#[tokio::test]
async fn typed_path_param() {
    assert_eq!(call(app(), Method::GET, "/users/42", "").await, (StatusCode::OK, "user 42".into()));
}

#[tokio::test]
async fn bad_param_type_is_400() {
    let (status, _) = call(app(), Method::GET, "/users/abc", "").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn args_order_does_not_matter() {
    let (_, body) = call(app(), Method::GET, "/users/bob/posts/7", "").await;
    assert_eq!(body, "bob:7");
}

#[tokio::test]
async fn unused_params_are_skipped() {
    let (status, body) = call(app(), Method::GET, "/skip/x/5/z", "").await;
    assert_eq!((status, body.as_str()), (StatusCode::OK, "b=5"));
}

#[tokio::test]
async fn wildcard_param() {
    let (_, body) = call(app(), Method::GET, "/files/a/b/c.txt", "").await;
    assert_eq!(body, "file a/b/c.txt");
}

#[tokio::test]
async fn state_with_path_param() {
    let app = Router::new()
        .route(greet::PATH, axum::routing::on(greet::METHOD, greet))
        .with_state(AppState { greeting: "hi" });
    let (_, body) = call(app, Method::GET, "/greet/ann", "").await;
    assert_eq!(body, "hi ann");
}

#[tokio::test]
async fn manual_path_extractor_is_untouched() {
    let (_, body) = call(app(), Method::GET, "/manual/3", "").await;
    assert_eq!(body, "manual 3");
}

#[tokio::test]
async fn mut_param() {
    let (_, body) = call(app(), Method::GET, "/mut/9", "").await;
    assert_eq!(body, "10");
}

#[tokio::test]
async fn same_path_different_methods_are_merged() {
    assert_eq!(call(app(), Method::GET, "/items/1", "").await.1, "get 1");
    assert_eq!(call(app(), Method::DELETE, "/items/1", "").await.1, "delete 1");
    assert_eq!(call(app(), Method::PUT, "/items/1", "").await.0, StatusCode::METHOD_NOT_ALLOWED);
}

#[tokio::test]
async fn body_extractor_after_path() {
    let (_, body) = call(app(), Method::POST, "/echo/5", "payload").await;
    assert_eq!(body, "5:payload");
}

#[tokio::test]
async fn route_with_several_methods() {
    assert_eq!(call(app(), Method::GET, "/multi", "").await.1, "multi");
    assert_eq!(call(app(), Method::POST, "/multi", "").await.1, "multi");
    assert_eq!(call(app(), Method::DELETE, "/multi", "").await.0, StatusCode::METHOD_NOT_ALLOWED);
}

#[tokio::test]
async fn escaped_braces_are_not_params() {
    let (status, body) = call(app(), Method::GET, "/braces/{literal}", "").await;
    assert_eq!((status, body.as_str()), (StatusCode::OK, "braces"));
}

#[tokio::test]
async fn handler_from_nested_module() {
    let (_, body) = call(app(), Method::GET, "/api/ping", "").await;
    assert_eq!(body, "pong");
}

#[tokio::test]
async fn query_required_and_optional() {
    assert_eq!(call(app(), Method::GET, "/search?q=rust&page=2", "").await.1, "rust:Some(2)");
    assert_eq!(call(app(), Method::GET, "/search?q=rust", "").await.1, "rust:None");
}

#[tokio::test]
async fn missing_or_bad_query_is_400() {
    assert_eq!(call(app(), Method::GET, "/search", "").await.0, StatusCode::BAD_REQUEST);
    assert_eq!(call(app(), Method::GET, "/search?q=a&page=x", "").await.0, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn path_and_query_together() {
    let (_, body) = call(app(), Method::GET, "/users/7/posts?skip=4&limit=10", "").await;
    assert_eq!(body, "7:Some(10):5");
}

#[tokio::test]
async fn query_with_custom_type() {
    assert_eq!(call(app(), Method::GET, "/sorted?sort=desc", "").await.1, "Desc");
    assert_eq!(call(app(), Method::GET, "/sorted?sort=up", "").await.0, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn query_type_from_parent_module() {
    assert_eq!(call(app(), Method::GET, "/api/sorted?sort=asc", "").await.1, "Asc");
}

#[tokio::test]
async fn query_with_state_and_path() {
    let app = Router::new()
        .route(query_with_state::PATH, axum::routing::on(query_with_state::METHOD, query_with_state))
        .with_state(AppState { greeting: "yo" });
    assert_eq!(call(app, Method::GET, "/with-state/kim?times=3", "").await.1, "yo kim x3");
}

#[tokio::test]
async fn query_before_body_extractor() {
    let (_, body) = call(app(), Method::POST, "/echo-query?tag=t", "payload").await;
    assert_eq!(body, "t:payload");
}
