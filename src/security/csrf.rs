use std::{
    collections::HashSet,
    sync::Arc,
    task::{Context, Poll},
};

use axum::{
    extract::Request,
    http::{HeaderMap, Method, StatusCode, Uri, header},
    response::{IntoResponse, Response},
};
use tower::{Layer, Service};

use super::BoxFuture;

/// CSRF protection without tokens: rejects state-changing requests (unsafe methods and
/// WebSocket handshakes) that a browser sent on behalf of another site.
///
/// Uses Fetch Metadata (`Sec-Fetch-Site`) and, for browsers without it, compares `Origin` with
/// the request host. Works the same for every extractor (forms, multipart, text), not only for
/// `Json`. Requests with neither header (curl, server-to-server, webhooks) pass: they carry no
/// ambient browser credentials.
#[derive(Clone, Default)]
pub struct CrossOrigin {
    trusted: Arc<HashSet<String>>,
}

impl CrossOrigin {
    pub fn new() -> Self {
        Self::default()
    }

    /// Accept requests from another origin, e.g. `https://admin.example.com`.
    pub fn trust(mut self, origin: impl AsRef<str>) -> Self {
        Arc::make_mut(&mut self.trusted).insert(normalize(origin.as_ref()));
        self
    }

    pub fn allows(&self, method: &Method, uri: &Uri, headers: &HeaderMap) -> bool {
        // WebSocket handshakes are GETs, but the socket that follows is not bound by the
        // same-origin policy, so a hostile page could talk to it with the user's cookies.
        let upgrade = headers.get(header::UPGRADE).is_some_and(|v| v.as_bytes().eq_ignore_ascii_case(b"websocket"));
        if !upgrade && matches!(*method, Method::GET | Method::HEAD | Method::OPTIONS) {
            return true;
        }
        let origin = headers.get(header::ORIGIN).and_then(|v| v.to_str().ok());
        if origin.is_some_and(|o| self.trusted.contains(&normalize(o))) {
            return true;
        }
        match headers.get("sec-fetch-site").map(|v| v.as_bytes()) {
            // "none" is a request the user started directly (address bar, bookmark).
            Some(b"same-origin" | b"none") => return true,
            Some(_) => return false,
            None => {}
        }
        let Some(origin) = origin else { return true };
        // HTTP/2 carries the host in :authority, which hyper puts into the URI.
        let host = headers.get(header::HOST).and_then(|v| v.to_str().ok()).or_else(|| uri.authority().map(|a| a.as_str()));
        // "null" (sandboxed iframes, opaque redirects) has no "://" and never matches.
        match (origin.split_once("://"), host) {
            (Some((_, origin_host)), Some(host)) => origin_host.eq_ignore_ascii_case(host),
            _ => false,
        }
    }
}

fn normalize(origin: &str) -> String {
    origin.trim().trim_end_matches('/').to_ascii_lowercase()
}

impl<S> Layer<S> for CrossOrigin {
    type Service = CrossOriginService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        CrossOriginService { inner, policy: self.clone() }
    }
}

#[derive(Clone)]
pub struct CrossOriginService<S> {
    inner: S,
    policy: CrossOrigin,
}

impl<S> Service<Request> for CrossOriginService<S>
where
    S: Service<Request, Response = Response> + Send + 'static,
    S::Future: Send + 'static,
{
    type Response = Response;
    type Error = S::Error;
    type Future = BoxFuture<'static, Result<Response, S::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), S::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request) -> Self::Future {
        if self.policy.allows(req.method(), req.uri(), req.headers()) {
            Box::pin(self.inner.call(req))
        } else {
            Box::pin(async { Ok((StatusCode::FORBIDDEN, "cross-origin request blocked").into_response()) })
        }
    }
}
