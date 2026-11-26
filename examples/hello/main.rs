use std::{
    collections::HashMap,
    sync::{
        LazyLock, Mutex, OnceLock,
        atomic::{AtomicU64, Ordering},
    },
};

use axum::{
    Json, Router,
    extract::{Multipart, WebSocketUpgrade, ws::Message},
    http::StatusCode,
    response::{IntoResponse, Sse, sse::Event},
};
use ngs_olezhek::{
    App, FieldErrors, LuaType, Valid, Validate, get, include_lua, post, routes,
    security::{Auth, RateLimit, Session, Sessions, password},
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, LuaType)]
struct User {
    id: u64,
    name: String,
    role: String,
}

static USERS: LazyLock<Mutex<Vec<User>>> = LazyLock::new(|| {
    let user = |id, name: &str, role: &str| User { id, name: name.into(), role: role.into() };
    Mutex::new(vec![user(1, "Олежек", "admin"), user(2, "Гарри", "player"), user(3, "Кляйнер", "player")])
});

#[get("/api/users/{id}")]
async fn get_user(id: u64) -> Result<Json<User>, (StatusCode, String)> {
    USERS
        .lock()
        .unwrap()
        .iter()
        .find(|u| u.id == id)
        .cloned()
        .map(Json)
        .ok_or((StatusCode::NOT_FOUND, format!("пользователя {id} нет")))
}

#[derive(Deserialize)]
struct Rename {
    name: String,
    role: String,
}

impl Validate for Rename {
    fn validate(&self, errors: &mut FieldErrors) {
        let name = self.name.trim();
        errors.check(!name.is_empty(), "name", "имя не может быть пустым");
        errors.check(name.chars().count() <= 20, "name", "не длиннее 20 символов");
        errors.check(!name.eq_ignore_ascii_case("admin"), "name", "это имя зарезервировано");
        errors.check(matches!(self.role.as_str(), "player" | "admin"), "role", "нет такой роли");
    }
}

#[post("/api/users/{id}/rename")]
async fn rename_user(id: u64, Valid(body): Valid<Rename>) -> Result<Json<User>, (StatusCode, String)> {
    let mut users = USERS.lock().unwrap();
    let user = users.iter_mut().find(|u| u.id == id).ok_or((StatusCode::NOT_FOUND, format!("пользователя {id} нет")))?;
    user.name = body.name.trim().to_string();
    user.role = body.role;
    Ok(Json(user.clone()))
}

#[derive(Deserialize)]
struct Signup {
    name: String,
    email: String,
    password: String,
    age: Option<u32>,
    role: String,
    agree: bool,
}

impl Validate for Signup {
    fn validate(&self, errors: &mut FieldErrors) {
        let name = self.name.trim();
        errors.check(!name.is_empty(), "name", "имя не может быть пустым");
        let taken = USERS.lock().unwrap().iter().any(|u| u.name.eq_ignore_ascii_case(name));
        errors.check(!taken, "name", "это имя уже занято");
        errors.check(self.email.contains('@'), "email", "нужен адрес почты");
        errors.check(!self.email.ends_with("@example.com"), "email", "с example.com регистрироваться нельзя");
        errors.check(self.password.chars().count() >= 8, "password", "не короче 8 символов");
        errors.check(self.age.is_none_or(|a| a >= 14), "age", "регистрация с 14 лет");
        errors.check(matches!(self.role.as_str(), "player" | "scientist" | "security"), "role", "нет такой роли");
        errors.check(self.agree, "agree", "без согласия с правилами нельзя");
    }
}

#[derive(Serialize, LuaType)]
struct SignupResult {
    id: u64,
    name: String,
}

#[post("/api/forms/signup")]
async fn signup(Valid(body): Valid<Signup>) -> Json<SignupResult> {
    static NEXT_ID: AtomicU64 = AtomicU64::new(100);
    Json(SignupResult { id: NEXT_ID.fetch_add(1, Ordering::Relaxed), name: body.name.trim().to_string() })
}

#[derive(Deserialize)]
struct Promo {
    code: String,
}

impl Validate for Promo {
    fn validate(&self, errors: &mut FieldErrors) {
        errors.check(!self.code.trim().is_empty(), "code", "введите промокод");
    }
}

#[derive(Serialize, LuaType)]
struct PromoResult {
    discount: u32,
}

#[post("/api/forms/promo")]
async fn promo(Valid(body): Valid<Promo>) -> Result<Json<PromoResult>, FieldErrors> {
    if !body.code.trim().eq_ignore_ascii_case("olezhek") {
        return Err(FieldErrors::new().message("Промокод не найден или истёк"));
    }
    Ok(Json(PromoResult { discount: 15 }))
}

static PASSWORDS: OnceLock<HashMap<u64, String>> = OnceLock::new();

#[derive(Deserialize)]
struct Login {
    name: String,
    password: String,
}

#[post("/api/login")]
async fn login(session: Session, Json(body): Json<Login>) -> Result<Json<User>, (StatusCode, &'static str)> {
    let user = USERS.lock().unwrap().iter().find(|u| u.name == body.name).cloned();
    let hash = user.as_ref().and_then(|u| PASSWORDS.get()?.get(&u.id)).map(String::as_str);
    if !password::verify(body.password, hash).await {
        return Err((StatusCode::UNAUTHORIZED, "неверное имя или пароль"));
    }
    let user = user.expect("verify passed, so the user exists");
    session.login(user.id).map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "сессия не сохранилась"))?;
    Ok(Json(user))
}

#[post("/api/logout")]
async fn logout(session: Session) -> StatusCode {
    session.logout();
    StatusCode::NO_CONTENT
}

#[get("/api/me")]
async fn me(Auth(id): Auth<u64>) -> Result<Json<User>, StatusCode> {
    USERS.lock().unwrap().iter().find(|u| u.id == id).cloned().map(Json).ok_or(StatusCode::UNAUTHORIZED)
}

#[get("/api/search")]
async fn search(#[query] q: String) -> Json<Vec<User>> {
    let q = q.to_lowercase();
    Json(USERS.lock().unwrap().iter().filter(|u| u.name.to_lowercase().contains(&q)).cloned().collect())
}

#[derive(Serialize, LuaType)]
struct GridRow {
    id: u32,
    label: String,
}

#[derive(Serialize, LuaType)]
struct RowBatch {
    total: u32,
    rows: Vec<GridRow>,
}

#[get("/api/rows")]
async fn grid_rows(#[query] offset: u32, #[query] limit: u32) -> Result<Json<RowBatch>, StatusCode> {
    const TOTAL: u32 = 10_000;
    if !(1..=256).contains(&limit) {
        return Err(StatusCode::BAD_REQUEST);
    }
    let end = offset.saturating_add(limit).min(TOTAL);
    let rows = (offset..end).map(|i| GridRow { id: i + 1, label: format!("Запись {}", i + 1) }).collect();
    Ok(Json(RowBatch { total: TOTAL, rows }))
}

async fn echo(ws: WebSocketUpgrade) -> impl IntoResponse {
    ws.on_upgrade(|mut socket| async move {
        while let Some(Ok(message)) = socket.recv().await {
            match message {
                Message::Text(text) => {
                    if socket.send(Message::Text(text)).await.is_err() { break; }
                }
                Message::Close(_) => break,
                _ => (),
            }
        }
    })
}

async fn events() -> impl IntoResponse {
    let event = Ok::<_, std::convert::Infallible>(Event::default().data("SSE подключён"));
    Sse::new(futures_util::stream::iter([event]))
}

async fn upload(mut form: Multipart) -> Result<Json<Vec<String>>, StatusCode> {
    let mut names = Vec::new();
    while let Some(field) = form.next_field().await.map_err(|e| e.status())? {
        if let Some(name) = field.file_name() {
            names.push(name.to_owned());
        }
        field.bytes().await.map_err(|e| e.status())?;
    }
    Ok(Json(names))
}

#[tokio::main]
async fn main() {
    let ids: Vec<u64> = USERS.lock().unwrap().iter().map(|u| u.id).collect();
    let mut hashes = HashMap::new();
    for id in ids {
        hashes.insert(id, password::hash("lambda").await);
    }
    PASSWORDS.set(hashes).unwrap();

    let app = App::new()
        .title("ngs_olezhek demo")
        .meta("description", "Демонстрация компонентов ngs_olezhek")
        .lua(include_lua!("examples/hello/lua"))
        .public(concat!(env!("CARGO_MANIFEST_DIR"), "/examples/hello/public"))
        .sessions(Sessions::memory())
        .api(routes![get_user, rename_user, search, grid_rows, logout, me, signup, promo])
        .api(routes![login].layer(RateLimit::per_minute(5)))
        .api(Router::new()
            .route("/api/echo", axum::routing::get(echo))
            .route("/api/events", axum::routing::get(events))
            .route("/api/upload", axum::routing::post(upload)));

    if let Err(e) = app.serve("127.0.0.1:3000").await {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
