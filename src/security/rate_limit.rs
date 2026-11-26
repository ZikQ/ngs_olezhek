use std::{
    collections::HashMap,
    fmt,
    net::SocketAddr,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU32, Ordering},
    },
    task::{Context, Poll},
    time::{Duration, Instant},
};

use axum::{
    extract::{ConnectInfo, Request},
    http::{HeaderName, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use tower::{Layer, Service};

use super::BoxFuture;

type KeyFn = Arc<dyn Fn(&Request) -> Option<String> + Send + Sync>;

/// Whose requests are counted together.
#[derive(Clone)]
pub enum ClientKey {
    /// The TCP peer address. `App::serve` provides it; behind a reverse proxy every client
    /// would share the proxy's address, so use `Forwarded` there.
    Peer,
    /// The address in `X-Forwarded-For` written by the outermost of `hops` trusted proxies.
    /// Entries further left are client-controlled and ignored.
    Forwarded { hops: usize },
    /// A header value, e.g. an API key.
    Header(HeaderName),
    /// Anything else (a user id from the session, a path segment). `None` falls back to the peer address.
    Custom(KeyFn),
}

impl fmt::Debug for ClientKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ClientKey::Peer => f.write_str("Peer"),
            ClientKey::Forwarded { hops } => write!(f, "Forwarded {{ hops: {hops} }}"),
            ClientKey::Header(h) => write!(f, "Header({h})"),
            ClientKey::Custom(_) => f.write_str("Custom"),
        }
    }
}

impl ClientKey {
    fn of(&self, req: &Request) -> String {
        let peer = || {
            req.extensions()
                .get::<ConnectInfo<SocketAddr>>()
                .map(|c| c.0.ip().to_string())
                // Without ConnectInfo (a router served by hand or a test) all clients share one bucket.
                .unwrap_or_else(|| "unknown".into())
        };
        match self {
            ClientKey::Peer => peer(),
            ClientKey::Forwarded { hops } => {
                let list: Vec<&str> = req
                    .headers()
                    .get_all("x-forwarded-for")
                    .iter()
                    .filter_map(|v| v.to_str().ok())
                    .flat_map(|v| v.split(','))
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .collect();
                match list.len().checked_sub(*hops) {
                    Some(i) if *hops > 0 => list[i].to_string(),
                    _ => peer(),
                }
            }
            ClientKey::Header(name) => match req.headers().get(name).and_then(|v| v.to_str().ok()) {
                Some(v) => format!("h:{v}"),
                None => peer(),
            },
            ClientKey::Custom(f) => f(req).map(|k| format!("c:{k}")).unwrap_or_else(peer),
        }
    }
}

/// GCRA rate limiter as a tower layer: `count` requests per `period`, at most `burst` at once.
/// Over the limit the answer is 429 with `Retry-After`. Clones share counters, so one limit can
/// cover several routers.
///
/// ```ignore
/// routes![login].layer(RateLimit::per_minute(5))
/// ```
#[derive(Clone)]
pub struct RateLimit {
    inner: Arc<Limiter>,
}

struct Limiter {
    interval: Duration,
    tolerance: Duration,
    key: ClientKey,
    tat: Mutex<HashMap<String, Instant>>,
    calls: AtomicU32,
}

impl fmt::Debug for RateLimit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RateLimit")
            .field("interval", &self.inner.interval)
            .field("tolerance", &self.inner.tolerance)
            .field("key", &self.inner.key)
            .finish()
    }
}

impl RateLimit {
    /// Panics if `count` is 0.
    pub fn new(count: u32, period: Duration) -> Self {
        assert!(count > 0, "RateLimit: count must be positive");
        Self::build(period / count, count, ClientKey::Peer)
    }

    pub fn per_second(count: u32) -> Self {
        Self::new(count, Duration::from_secs(1))
    }

    pub fn per_minute(count: u32) -> Self {
        Self::new(count, Duration::from_secs(60))
    }

    pub fn per_hour(count: u32) -> Self {
        Self::new(count, Duration::from_secs(3600))
    }

    /// Requests allowed back to back before the steady rate applies. Defaults to `count`.
    pub fn burst(self, burst: u32) -> Self {
        let key = self.inner.key.clone();
        Self::build(self.inner.interval, burst.max(1), key)
    }

    pub fn key(self, key: ClientKey) -> Self {
        let burst = (self.inner.tolerance.as_nanos() / self.inner.interval.as_nanos().max(1)) as u32 + 1;
        Self::build(self.inner.interval, burst, key)
    }

    fn build(interval: Duration, burst: u32, key: ClientKey) -> Self {
        Self {
            inner: Arc::new(Limiter {
                interval,
                tolerance: interval * (burst - 1),
                key,
                tat: Mutex::default(),
                calls: AtomicU32::new(0),
            }),
        }
    }

    /// Counts one request for `key`; `Err` holds how long to wait.
    pub fn check(&self, key: &str) -> Result<(), Duration> {
        self.check_at(key, Instant::now())
    }

    fn check_at(&self, key: &str, now: Instant) -> Result<(), Duration> {
        let l = &self.inner;
        let mut map = l.tat.lock().unwrap_or_else(|e| e.into_inner());
        if l.calls.fetch_add(1, Ordering::Relaxed) % 1024 == 1023 {
            map.retain(|_, tat| *tat > now);
        }
        let tat = map.get(key).copied().filter(|t| *t > now).unwrap_or(now);
        let ahead = tat - now;
        if ahead > l.tolerance {
            return Err(ahead - l.tolerance);
        }
        map.insert(key.to_owned(), tat + l.interval);
        Ok(())
    }
}

fn too_many(wait: Duration) -> Response {
    let secs = wait.as_secs() + u64::from(wait.subsec_nanos() > 0);
    let mut res = (StatusCode::TOO_MANY_REQUESTS, "Too Many Requests").into_response();
    res.headers_mut().insert(header::RETRY_AFTER, HeaderValue::from(secs.max(1)));
    res
}

impl<S> Layer<S> for RateLimit {
    type Service = RateLimitService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        RateLimitService { inner, limit: self.clone() }
    }
}

#[derive(Clone)]
pub struct RateLimitService<S> {
    inner: S,
    limit: RateLimit,
}

impl<S> Service<Request> for RateLimitService<S>
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
        let key = self.limit.inner.key.of(&req);
        match self.limit.check(&key) {
            Ok(()) => Box::pin(self.inner.call(req)),
            Err(wait) => Box::pin(async move { Ok(too_many(wait)) }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn burst_then_steady_rate() {
        let limit = RateLimit::per_second(2);
        let t0 = Instant::now();
        assert!(limit.check_at("a", t0).is_ok());
        assert!(limit.check_at("a", t0).is_ok());
        let wait = limit.check_at("a", t0).unwrap_err();
        assert_eq!(wait, Duration::from_millis(500));
        assert!(limit.check_at("b", t0).is_ok(), "keys are independent");
        assert!(limit.check_at("a", t0 + Duration::from_millis(500)).is_ok());
        assert!(limit.check_at("a", t0 + Duration::from_millis(500)).is_err());
    }

    #[test]
    fn burst_one_spaces_requests() {
        let limit = RateLimit::per_minute(60).burst(1);
        let t0 = Instant::now();
        assert!(limit.check_at("a", t0).is_ok());
        assert_eq!(limit.check_at("a", t0 + Duration::from_millis(200)).unwrap_err(), Duration::from_millis(800));
        assert!(limit.check_at("a", t0 + Duration::from_secs(1)).is_ok());
    }

    #[test]
    fn changing_key_keeps_burst() {
        let limit = RateLimit::per_second(3).burst(2).key(ClientKey::Forwarded { hops: 1 });
        let t0 = Instant::now();
        assert!(limit.check_at("a", t0).is_ok());
        assert!(limit.check_at("a", t0).is_ok());
        assert!(limit.check_at("a", t0).is_err());
    }
}
