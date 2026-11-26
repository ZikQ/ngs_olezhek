use std::sync::{LazyLock, Mutex};

use axum::{Json, http::StatusCode};
use ngs_olezhek::{App, get, include_lua, post, routes};
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize)]
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
}

#[post("/api/users/{id}/rename")]
async fn rename_user(id: u64, Json(body): Json<Rename>) -> Result<Json<User>, (StatusCode, String)> {
    let name = body.name.trim();
    if name.is_empty() {
        return Err((StatusCode::UNPROCESSABLE_ENTITY, "имя не может быть пустым".into()));
    }
    let mut users = USERS.lock().unwrap();
    let user = users.iter_mut().find(|u| u.id == id).ok_or((StatusCode::NOT_FOUND, format!("пользователя {id} нет")))?;
    user.name = name.to_string();
    Ok(Json(user.clone()))
}

#[get("/api/search")]
async fn search(#[query] q: String) -> Json<Vec<User>> {
    let q = q.to_lowercase();
    Json(USERS.lock().unwrap().iter().filter(|u| u.name.to_lowercase().contains(&q)).cloned().collect())
}

#[tokio::main]
async fn main() {
    let app = App::new()
        .title("ngs_olezhek demo")
        .lua(include_lua!("examples/hello/lua"))
        .public(concat!(env!("CARGO_MANIFEST_DIR"), "/examples/hello/public"))
        .api(routes![get_user, rename_user, search]);

    if let Err(e) = app.serve("127.0.0.1:3000").await {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
