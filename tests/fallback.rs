use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode},
    routing::get,
};
use ngs_olezhek::{App, include_lua};
use tower::ServiceExt;

fn router() -> Router {
    App::new()
        .dev(false)
        .lua(include_lua!("tests/fixtures/lua"))
        .api(Router::new().route("/api/ping", get(|| async { "pong" })))
        .into_router()
        .unwrap()
}

async fn send(req: Request<Body>) -> (StatusCode, bool, String) {
    let res = router().oneshot(req).await.unwrap();
    let status = res.status();
    let shell = res.headers().contains_key("x-ngs-shell");
    let body = String::from_utf8(to_bytes(res.into_body(), usize::MAX).await.unwrap().to_vec()).unwrap();
    (status, shell, body)
}

fn get_req(path: &str, headers: &[(&str, &str)]) -> Request<Body> {
    let mut b = Request::get(path);
    for (k, v) in headers {
        b = b.header(*k, *v);
    }
    b.body(Body::empty()).unwrap()
}

#[tokio::test]
async fn navigation_gets_the_app_page() {
    let (status, shell, body) =
        send(get_req("/profile/1", &[("sec-fetch-mode", "navigate"), ("accept", "text/html")])).await;
    assert_eq!(status, StatusCode::OK);
    assert!(shell);
    assert!(body.contains("/_ngs/runtime.js"));
}

#[tokio::test]
async fn old_client_accepting_html_gets_the_app_page() {
    let (status, shell, _) = send(get_req("/", &[("accept", "text/html,application/xhtml+xml")])).await;
    assert_eq!(status, StatusCode::OK);
    assert!(shell);
}

#[tokio::test]
async fn api_typo_is_404() {
    let (status, shell, body) =
        send(get_req("/api/usres/1", &[("sec-fetch-mode", "navigate"), ("accept", "text/html")])).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(!shell);
    assert!(!body.contains("<html"));
}

#[tokio::test]
async fn fetch_to_unknown_page_url_is_404() {
    let (status, _, _) = send(get_req("/nowhere", &[("sec-fetch-mode", "cors"), ("accept", "*/*")])).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn json_only_client_is_404() {
    let (status, _, _) = send(get_req("/nowhere", &[("accept", "application/json")])).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn post_to_unknown_path_is_404() {
    let req = Request::post("/nowhere").header("accept", "text/html").body(Body::empty()).unwrap();
    let (status, _, _) = send(req).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn unknown_internal_path_is_404() {
    let (status, _, _) = send(get_req("/_ngs/nope", &[("accept", "text/html")])).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn real_api_route_still_works() {
    let (status, shell, body) = send(get_req("/api/ping", &[("sec-fetch-mode", "cors")])).await;
    assert_eq!(status, StatusCode::OK);
    assert!(!shell);
    assert_eq!(body, "pong");
}

#[tokio::test]
async fn custom_api_prefix() {
    let router = App::new().dev(false).lua(include_lua!("tests/fixtures/lua")).api_prefix("/v1/").into_router().unwrap();
    let res = router.clone().oneshot(get_req("/v1/x", &[("accept", "text/html")])).await.unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    let res = router.oneshot(get_req("/api/x", &[("accept", "text/html")])).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK, "/api is a page path once the prefix moved");
}
