use std::sync::Arc;

use axum::{
    Router,
    body::to_bytes,
    extract::Request,
    http::{StatusCode, header},
    routing::post,
};
use serde::Deserialize;
use serde_json::Value;

use crate::security::{RateLimit, Session};

pub(crate) const PATH: &str = "/_ngs/errors";
const MAX_BODY: usize = 64 * 1024;
const MAX_ENTRIES: usize = 20;
const MAX_MESSAGE: usize = 8 * 1024;

/// A Lua error from a visitor's browser. Release builds send them (dev mode shows them on the
/// page instead). Messages come from stripped bytecode, so tracebacks have no line numbers
/// unless the app runs with `App::strip(false)`.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct ClientError {
    pub message: String,
    /// How many times the browser hit this message since its previous report.
    pub count: u32,
    /// Path of the page that was open, without the query string.
    pub page: String,
    pub user_agent: Option<String>,
    /// What `Session::login` stored, if sessions are on and the visitor is logged in.
    pub user: Option<Value>,
}

pub(crate) type Handler = Arc<dyn Fn(ClientError) + Send + Sync>;

pub(crate) fn print(e: ClientError) {
    let user = e.user.map(|u| format!(" user={u}")).unwrap_or_default();
    let count = if e.count > 1 { format!(" (x{})", e.count) } else { String::new() };
    eprintln!("ngs_olezhek: client error on {}{user}{count}: {}", e.page, e.message);
}

#[derive(Deserialize)]
struct Report {
    #[serde(default)]
    page: String,
    errors: Vec<Entry>,
}

#[derive(Deserialize)]
struct Entry {
    message: String,
    #[serde(default)]
    count: u32,
}

fn clip(mut s: String, max: usize) -> String {
    if s.len() > max {
        let mut end = max;
        while !s.is_char_boundary(end) {
            end -= 1;
        }
        s.truncate(end);
    }
    s
}

/// The endpoint is open to anyone, so the report is size-capped and rate-limited: a script
/// posting garbage can at worst add a few lines per minute to the log.
pub(crate) fn router(handler: Handler, limit: RateLimit) -> Router {
    Router::new()
        .route(
            PATH,
            post(move |req: Request| async move {
                let (parts, body) = req.into_parts();
                let Ok(bytes) = to_bytes(body, MAX_BODY).await else { return StatusCode::PAYLOAD_TOO_LARGE };
                let Ok(report) = serde_json::from_slice::<Report>(&bytes) else { return StatusCode::BAD_REQUEST };
                let user_agent = parts.headers.get(header::USER_AGENT).and_then(|v| v.to_str().ok()).map(|s| clip(s.to_owned(), 512));
                let user = parts.extensions.get::<Session>().and_then(|s| s.user::<Value>());
                let page = clip(report.page, 1024);
                for entry in report.errors.into_iter().take(MAX_ENTRIES) {
                    handler(ClientError {
                        message: clip(entry.message, MAX_MESSAGE),
                        count: entry.count.max(1),
                        page: page.clone(),
                        user_agent: user_agent.clone(),
                        user: user.clone(),
                    });
                }
                StatusCode::NO_CONTENT
            }),
        )
        .layer(limit)
}
