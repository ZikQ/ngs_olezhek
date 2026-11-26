use axum::{
    Json, Router,
    body::{Body, to_bytes},
    http::{Method, Request, StatusCode, header},
};
use ngs_olezhek_macros::{get, post, put, routes};
use serde::{Deserialize, Serialize};
use tower::ServiceExt;

#[derive(Deserialize)]
struct NewUser {
    name: String,
}

#[derive(Serialize)]
struct User {
    id: u64,
    name: String,
}

#[get("/users/{id}")]
async fn get_user(id: u64) -> Json<User> {
    Json(User { id, name: "bob".into() })
}

#[post("/users")]
async fn create_user(Json(body): Json<NewUser>) -> (StatusCode, Json<User>) {
    (StatusCode::CREATED, Json(User { id: 1, name: body.name }))
}

#[put("/users/{id}")]
async fn update_user(id: u64, Json(body): Json<NewUser>) -> Json<User> {
    Json(User { id, name: body.name })
}

#[post("/users/{id}/notes")]
async fn add_note(id: u64, #[query] notify: Option<bool>, Json(body): Json<NewUser>) -> String {
    format!("{id}:{notify:?}:{}", body.name)
}

fn app() -> Router {
    routes![get_user, create_user, update_user, add_note]
}

async fn send(method: Method, uri: &str, content_type: Option<&str>, body: &str) -> (StatusCode, String) {
    let mut req = Request::builder().method(method).uri(uri);
    if let Some(ct) = content_type {
        req = req.header(header::CONTENT_TYPE, ct);
    }
    let res = app().oneshot(req.body(Body::from(body.to_owned())).unwrap()).await.unwrap();
    let status = res.status();
    let bytes = to_bytes(res.into_body(), usize::MAX).await.unwrap();
    (status, String::from_utf8(bytes.to_vec()).unwrap())
}

#[tokio::test]
async fn json_response_with_path_param() {
    let (status, body) = send(Method::GET, "/users/5", None, "").await;
    assert_eq!((status, body.as_str()), (StatusCode::OK, r#"{"id":5,"name":"bob"}"#));
}

#[tokio::test]
async fn json_request_and_response() {
    let (status, body) = send(Method::POST, "/users", Some("application/json"), r#"{"name":"ann"}"#).await;
    assert_eq!((status, body.as_str()), (StatusCode::CREATED, r#"{"id":1,"name":"ann"}"#));
}

#[tokio::test]
async fn path_param_and_json_body() {
    let (status, body) = send(Method::PUT, "/users/7", Some("application/json"), r#"{"name":"kim"}"#).await;
    assert_eq!((status, body.as_str()), (StatusCode::OK, r#"{"id":7,"name":"kim"}"#));
}

#[tokio::test]
async fn json_errors() {
    let ok = r#"{"name":"x"}"#;
    assert_eq!(send(Method::POST, "/users", None, ok).await.0, StatusCode::UNSUPPORTED_MEDIA_TYPE);
    assert_eq!(send(Method::POST, "/users", Some("application/json"), "{oops").await.0, StatusCode::BAD_REQUEST);
    assert_eq!(send(Method::POST, "/users", Some("application/json"), "{}").await.0, StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn path_query_and_json_together() {
    let (status, body) = send(Method::POST, "/users/3/notes?notify=true", Some("application/json"), r#"{"name":"hi"}"#).await;
    assert_eq!((status, body.as_str()), (StatusCode::OK, "3:Some(true):hi"));
}
