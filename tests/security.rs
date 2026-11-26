use axum::{
    Form, Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
    response::Response,
    routing::{get, post},
};
use ngs_olezhek::{
    App, include_lua,
    security::{Auth, ClientKey, Cors, RateLimit, Session, Sessions, password},
};
use serde::Deserialize;
use tower::ServiceExt;

#[derive(Deserialize)]
struct Transfer {
    amount: u32,
}

async fn transfer(Form(t): Form<Transfer>) -> String {
    format!("sent {}", t.amount)
}

async fn login(session: Session) -> &'static str {
    session.login(42u64).unwrap();
    "ok"
}

async fn me(Auth(id): Auth<u64>) -> String {
    id.to_string()
}

async fn maybe_me(user: Option<Auth<u64>>) -> String {
    user.map_or("guest".into(), |Auth(id)| id.to_string())
}

async fn logout(session: Session) -> &'static str {
    session.logout();
    "bye"
}

async fn remember(session: Session) -> &'static str {
    session.insert("theme", "dark").unwrap();
    "ok"
}

async fn theme(session: Session) -> String {
    session.get::<String>("theme").unwrap_or_default()
}

fn app() -> App {
    App::new()
        .dev(false)
        .lua(include_lua!("tests/fixtures/lua"))
        .sessions(Sessions::memory())
        .api(
            Router::new()
                .route("/api/transfer", post(transfer))
                .route("/api/login", post(login))
                .route("/api/logout", post(logout))
                .route("/api/me", get(me))
                .route("/api/maybe", get(maybe_me))
                .route("/api/remember", post(remember))
                .route("/api/theme", get(theme))
                .route("/api/ws", get(|| async { "upgraded" })),
        )
        .api(Router::new().route("/api/limited", get(|| async { "ok" })).layer(RateLimit::per_minute(2)))
}

async fn send(router: &Router, req: Request<Body>) -> Response {
    router.clone().oneshot(req).await.unwrap()
}

async fn body(res: Response) -> String {
    String::from_utf8(to_bytes(res.into_body(), usize::MAX).await.unwrap().to_vec()).unwrap()
}

fn form_post(path: &str, headers: &[(&str, &str)]) -> Request<Body> {
    let mut b = Request::post(path).header(header::CONTENT_TYPE, "application/x-www-form-urlencoded").header(header::HOST, "app.test");
    for (k, v) in headers {
        b = b.header(*k, *v);
    }
    b.body(Body::from("amount=100")).unwrap()
}

fn cookie_of(res: &Response) -> Option<String> {
    let set = res.headers().get(header::SET_COOKIE)?.to_str().ok()?;
    Some(set.split(';').next()?.to_string())
}

#[tokio::test]
async fn cross_site_form_post_is_blocked() {
    let r = app().into_router().unwrap();
    let res = send(&r, form_post("/api/transfer", &[("sec-fetch-site", "cross-site"), ("origin", "https://evil.test")])).await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
    let res = send(&r, form_post("/api/transfer", &[("sec-fetch-site", "same-site")])).await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN, "a sibling subdomain is another origin");
}

#[tokio::test]
async fn same_origin_and_non_browser_posts_pass() {
    let r = app().into_router().unwrap();
    let res = send(&r, form_post("/api/transfer", &[("sec-fetch-site", "same-origin"), ("origin", "https://app.test")])).await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(body(res).await, "sent 100");
    let res = send(&r, form_post("/api/transfer", &[])).await;
    assert_eq!(res.status(), StatusCode::OK, "curl sends neither Origin nor Sec-Fetch-Site");
}

#[tokio::test]
async fn origin_is_checked_when_fetch_metadata_is_missing() {
    let r = app().into_router().unwrap();
    let res = send(&r, form_post("/api/transfer", &[("origin", "https://evil.test")])).await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
    let res = send(&r, form_post("/api/transfer", &[("origin", "null")])).await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
    let res = send(&r, form_post("/api/transfer", &[("origin", "https://APP.test")])).await;
    assert_eq!(res.status(), StatusCode::OK);
}

#[tokio::test]
async fn cross_site_websocket_handshake_is_blocked() {
    let r = app().into_router().unwrap();
    let ws = |site: &str| {
        Request::get("/api/ws")
            .header(header::UPGRADE, "websocket")
            .header(header::CONNECTION, "upgrade")
            .header("sec-fetch-site", site)
            .body(Body::empty())
            .unwrap()
    };
    assert_eq!(send(&r, ws("cross-site")).await.status(), StatusCode::FORBIDDEN);
    assert_eq!(send(&r, ws("same-origin")).await.status(), StatusCode::OK);
    let plain = Request::get("/api/ws").header("sec-fetch-site", "cross-site").body(Body::empty()).unwrap();
    assert_eq!(send(&r, plain).await.status(), StatusCode::OK, "plain GETs stay readable only through CORS");
}

#[tokio::test]
async fn dev_live_socket_rejects_other_sites() {
    let r = app().dev(true).into_router().unwrap();
    let req = Request::get("/_ngs/live")
        .header(header::UPGRADE, "websocket")
        .header(header::CONNECTION, "upgrade")
        .header("sec-fetch-site", "cross-site")
        .header(header::ORIGIN, "https://evil.test")
        .body(Body::empty())
        .unwrap();
    assert_eq!(send(&r, req).await.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn trusted_origin_and_disabled_protection() {
    let r = app().trust_origin("https://admin.test/").into_router().unwrap();
    let res = send(&r, form_post("/api/transfer", &[("sec-fetch-site", "cross-site"), ("origin", "https://admin.test")])).await;
    assert_eq!(res.status(), StatusCode::OK);
    let r = app().csrf(false).into_router().unwrap();
    let res = send(&r, form_post("/api/transfer", &[("sec-fetch-site", "cross-site"), ("origin", "https://evil.test")])).await;
    assert_eq!(res.status(), StatusCode::OK);
}

#[tokio::test]
async fn cors_answers_preflight_and_trusts_its_origins() {
    let r = app().cors(Cors::origins(["https://admin.test"]).credentials(true)).into_router().unwrap();
    let pre = Request::builder()
        .method("OPTIONS")
        .uri("/api/transfer")
        .header(header::ORIGIN, "https://admin.test")
        .header(header::ACCESS_CONTROL_REQUEST_METHOD, "POST")
        .body(Body::empty())
        .unwrap();
    let res = send(&r, pre).await;
    assert_eq!(res.headers().get(header::ACCESS_CONTROL_ALLOW_ORIGIN).unwrap(), "https://admin.test");
    assert_eq!(res.headers().get(header::ACCESS_CONTROL_ALLOW_CREDENTIALS).unwrap(), "true");
    let res = send(&r, form_post("/api/transfer", &[("sec-fetch-site", "cross-site"), ("origin", "https://admin.test")])).await;
    assert_eq!(res.status(), StatusCode::OK);
    let res = send(&r, form_post("/api/transfer", &[("sec-fetch-site", "cross-site"), ("origin", "https://other.test")])).await;
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
    assert!(res.headers().get(header::ACCESS_CONTROL_ALLOW_ORIGIN).is_none());
}

#[tokio::test]
async fn login_session_and_logout() {
    let r = app().into_router().unwrap();
    assert_eq!(send(&r, Request::get("/api/me").body(Body::empty()).unwrap()).await.status(), StatusCode::UNAUTHORIZED);
    let guest = send(&r, Request::get("/api/maybe").body(Body::empty()).unwrap()).await;
    assert!(guest.headers().get(header::SET_COOKIE).is_none(), "reading creates no session");
    assert_eq!(body(guest).await, "guest");

    let res = send(&r, Request::post("/api/login").body(Body::empty()).unwrap()).await;
    let set = res.headers().get(header::SET_COOKIE).unwrap().to_str().unwrap().to_string();
    assert!(set.starts_with("__Host-ngs-session="), "{set}");
    for attr in ["HttpOnly", "SameSite=Lax", "Secure", "Path=/"] {
        assert!(set.contains(attr), "{attr} missing in {set}");
    }
    assert_eq!(res.headers().get(header::CACHE_CONTROL).unwrap(), "no-store");
    let cookie = cookie_of(&res).unwrap();

    let res = send(&r, Request::get("/api/me").header(header::COOKIE, &cookie).body(Body::empty()).unwrap()).await;
    assert_eq!(res.status(), StatusCode::OK);
    assert!(res.headers().get(header::SET_COOKIE).is_none());
    assert_eq!(body(res).await, "42");

    let res = send(&r, Request::post("/api/logout").header(header::COOKIE, &cookie).body(Body::empty()).unwrap()).await;
    assert!(res.headers().get(header::SET_COOKIE).unwrap().to_str().unwrap().contains("Max-Age=0"));
    let res = send(&r, Request::get("/api/me").header(header::COOKIE, &cookie).body(Body::empty()).unwrap()).await;
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED, "the old cookie is dead after logout");
}

#[tokio::test]
async fn login_replaces_a_planted_session_id() {
    let r = app().into_router().unwrap();
    let res = send(&r, Request::post("/api/remember").body(Body::empty()).unwrap()).await;
    let before = cookie_of(&res).unwrap();
    let res = send(&r, Request::post("/api/login").header(header::COOKIE, &before).body(Body::empty()).unwrap()).await;
    let after = cookie_of(&res).unwrap();
    assert_ne!(before, after);
    let res = send(&r, Request::get("/api/theme").header(header::COOKIE, &after).body(Body::empty()).unwrap()).await;
    assert_eq!(body(res).await, "dark", "data survives the new id");
    let res = send(&r, Request::get("/api/me").header(header::COOKIE, &before).body(Body::empty()).unwrap()).await;
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn forged_or_unknown_cookie_is_cleared() {
    let r = app().into_router().unwrap();
    let fake = format!("__Host-ngs-session={}", "A".repeat(43));
    let res = send(&r, Request::get("/api/me").header(header::COOKIE, &fake).body(Body::empty()).unwrap()).await;
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    assert!(res.headers().get(header::SET_COOKIE).unwrap().to_str().unwrap().contains("Max-Age=0"));
}

#[tokio::test]
async fn dev_cookie_is_not_secure() {
    let r = app().dev(true).into_router().unwrap();
    let res = send(&r, Request::post("/api/login").body(Body::empty()).unwrap()).await;
    let set = res.headers().get(header::SET_COOKIE).unwrap().to_str().unwrap();
    assert!(set.starts_with("ngs-session=") && !set.contains("Secure"), "{set}");
}

#[tokio::test]
async fn rate_limit_answers_429_with_retry_after() {
    let r = app().into_router().unwrap();
    let get = || Request::get("/api/limited").body(Body::empty()).unwrap();
    assert_eq!(send(&r, get()).await.status(), StatusCode::OK);
    assert_eq!(send(&r, get()).await.status(), StatusCode::OK);
    let res = send(&r, get()).await;
    assert_eq!(res.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(res.headers().get(header::RETRY_AFTER).unwrap(), "30");
    let other = send(&r, Request::get("/api/transfer").body(Body::empty()).unwrap()).await;
    assert_ne!(other.status(), StatusCode::TOO_MANY_REQUESTS, "other routes are not limited");
}

#[tokio::test]
async fn rate_limit_keys_by_forwarded_client() {
    let limited = Router::new()
        .route("/x", get(|| async { "ok" }))
        .layer(RateLimit::per_minute(1).key(ClientKey::Forwarded { hops: 1 }));
    let from = |chain: &str| Request::get("/x").header("x-forwarded-for", chain).body(Body::empty()).unwrap();
    assert_eq!(send(&limited, from("6.6.6.6, 1.1.1.1")).await.status(), StatusCode::OK);
    // The left part is whatever the client wrote; only the proxy's entry counts.
    assert_eq!(send(&limited, from("7.7.7.7, 1.1.1.1")).await.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(send(&limited, from("1.1.1.1, 2.2.2.2")).await.status(), StatusCode::OK);
}

#[tokio::test]
async fn password_hash_round_trip() {
    let h = password::hash("correct horse").await;
    assert!(h.starts_with("$argon2id$"), "{h}");
    assert!(password::verify("correct horse", Some(&h)).await);
    assert!(!password::verify("wrong", Some(&h)).await);
    assert!(!password::verify("correct horse", None).await);
    assert!(!password::verify("x", Some("not a hash")).await);
}
