use std::{
    collections::HashMap,
    io,
    sync::{Arc, Mutex, MutexGuard},
    task::{Context, Poll},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use axum::{
    extract::{FromRequestParts, OptionalFromRequestParts, Request},
    http::{HeaderMap, HeaderValue, StatusCode, header, request::Parts},
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::{Map, Value};
use tower::{Layer, Service};

use super::{BoxFuture, random_token, sha256_hex};

const USER_KEY: &str = "__user";
/// An untouched session is written back at most this often to move its idle deadline.
const TOUCH_EVERY: u64 = 60;

/// What a store keeps per session. Times are Unix seconds.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Record {
    pub data: Map<String, Value>,
    pub created: u64,
    pub seen: u64,
}

/// Session persistence. Keys are SHA-256 of the cookie token, so a leaked store (a dump, a
/// backup, a log) cannot be turned back into working cookies.
pub trait SessionStore: Send + Sync + 'static {
    fn load<'a>(&'a self, key: &'a str) -> BoxFuture<'a, io::Result<Option<Record>>>;
    /// `ttl` is how long the record must survive; a store with native expiry can use it.
    fn save<'a>(&'a self, key: &'a str, record: &'a Record, ttl: Duration) -> BoxFuture<'a, io::Result<()>>;
    fn delete<'a>(&'a self, key: &'a str) -> BoxFuture<'a, io::Result<()>>;
}

/// In-process store: fine for one server, lost on restart.
#[derive(Default)]
pub struct MemoryStore {
    map: Mutex<HashMap<String, (Record, Instant)>>,
}

impl MemoryStore {
    pub fn new() -> Self {
        Self::default()
    }

    fn map(&self) -> MutexGuard<'_, HashMap<String, (Record, Instant)>> {
        self.map.lock().unwrap_or_else(|e| e.into_inner())
    }
}

impl SessionStore for MemoryStore {
    fn load<'a>(&'a self, key: &'a str) -> BoxFuture<'a, io::Result<Option<Record>>> {
        let now = Instant::now();
        let r = self.map().get(key).filter(|(_, until)| *until > now).map(|(r, _)| r.clone());
        Box::pin(async move { Ok(r) })
    }

    fn save<'a>(&'a self, key: &'a str, record: &'a Record, ttl: Duration) -> BoxFuture<'a, io::Result<()>> {
        let now = Instant::now();
        let mut map = self.map();
        // Abandoned sessions would pile up otherwise; an O(n) sweep per new session is cheap enough here.
        if !map.contains_key(key) {
            map.retain(|_, (_, until)| *until > now);
        }
        map.insert(key.to_owned(), (record.clone(), now + ttl));
        Box::pin(async { Ok(()) })
    }

    fn delete<'a>(&'a self, key: &'a str) -> BoxFuture<'a, io::Result<()>> {
        self.map().remove(key);
        Box::pin(async { Ok(()) })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SameSite {
    Strict,
    Lax,
    None,
}

/// Cookie sessions with server-side storage, as a tower layer. Handlers take [`Session`] or
/// [`Auth<T>`]. The cookie is HttpOnly, SameSite=Lax and, unless `secure(false)`, `__Host-`
/// prefixed and Secure. A session is created only once something is stored in it.
#[derive(Clone)]
pub struct Sessions {
    store: Arc<dyn SessionStore>,
    secure: Option<bool>,
    same_site: SameSite,
    idle: Duration,
    lifetime: Duration,
    name: Option<String>,
}

impl Sessions {
    pub fn new(store: impl SessionStore) -> Self {
        Self {
            store: Arc::new(store),
            secure: None,
            same_site: SameSite::Lax,
            idle: Duration::from_secs(24 * 3600),
            lifetime: Duration::from_secs(30 * 24 * 3600),
            name: None,
        }
    }

    pub fn memory() -> Self {
        Self::new(MemoryStore::new())
    }

    /// Secure cookies only travel over HTTPS. `App` defaults this to `!dev`; used directly as
    /// a layer it defaults to true.
    pub fn secure(mut self, on: bool) -> Self {
        self.secure = Some(on);
        self
    }

    pub fn same_site(mut self, same_site: SameSite) -> Self {
        self.same_site = same_site;
        self
    }

    /// The session ends after this long without requests. Default: one day.
    pub fn idle_timeout(mut self, idle: Duration) -> Self {
        self.idle = idle;
        self
    }

    /// The session ends this long after it was created (or after `login`), however active. Default: 30 days.
    pub fn lifetime(mut self, lifetime: Duration) -> Self {
        self.lifetime = lifetime;
        self
    }

    pub fn cookie_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    pub(crate) fn default_secure(mut self, on: bool) -> Self {
        self.secure.get_or_insert(on);
        self
    }

    fn is_secure(&self) -> bool {
        self.secure.unwrap_or(true)
    }

    fn name(&self) -> &str {
        match &self.name {
            Some(n) => n,
            None if self.is_secure() => "__Host-ngs-session",
            None => "ngs-session",
        }
    }

    fn deadline(&self, r: &Record) -> u64 {
        (r.created + self.lifetime.as_secs()).min(r.seen + self.idle.as_secs())
    }

    fn cookie(&self, value: &str, max_age: u64) -> HeaderValue {
        let same_site = match self.same_site {
            SameSite::Strict => "Strict",
            SameSite::Lax => "Lax",
            SameSite::None => "None",
        };
        let secure = if self.is_secure() || self.same_site == SameSite::None { "; Secure" } else { "" };
        let c = format!("{}={value}; Path=/; HttpOnly; SameSite={same_site}; Max-Age={max_age}{secure}", self.name());
        HeaderValue::from_str(&c).expect("cookie is ASCII")
    }

    fn plan(&self, session: &Session, old: Option<String>, had_cookie: bool, now: u64) -> Plan {
        let mut st = session.lock();
        let expire = if had_cookie { Plan::Expire(old.clone()) } else { Plan::Nothing };
        if st.destroyed {
            return expire;
        }
        match old {
            Some(key) if !st.cycle => {
                if st.changed || now.saturating_sub(st.seen) >= TOUCH_EVERY {
                    Plan::Save(key, Record { data: std::mem::take(&mut st.data), created: st.created, seen: now })
                } else {
                    Plan::Nothing
                }
            }
            old if st.cycle || !st.data.is_empty() => {
                Plan::Create(old, Record { data: std::mem::take(&mut st.data), created: now, seen: now })
            }
            _ => expire,
        }
    }

    async fn finish(&self, session: &Session, old: Option<String>, had_cookie: bool, now: u64, res: &mut Response) -> io::Result<()> {
        let set = |res: &mut Response, c: HeaderValue| {
            let h = res.headers_mut();
            h.append(header::SET_COOKIE, c);
            // A shared cache must never hand one user's Set-Cookie to another.
            h.entry(header::CACHE_CONTROL).or_insert(HeaderValue::from_static("no-store"));
        };
        let ttl = |r: &Record| Duration::from_secs(self.deadline(r).saturating_sub(now));
        match self.plan(session, old, had_cookie, now) {
            Plan::Nothing => {}
            Plan::Expire(old) => {
                if let Some(key) = old {
                    self.store.delete(&key).await?;
                }
                set(res, self.cookie("", 0));
            }
            Plan::Save(key, record) => self.store.save(&key, &record, ttl(&record)).await?,
            Plan::Create(old, record) => {
                if let Some(key) = old {
                    self.store.delete(&key).await?;
                }
                let token = random_token();
                self.store.save(&sha256_hex(token.as_bytes()), &record, ttl(&record)).await?;
                set(res, self.cookie(&token, self.lifetime.as_secs()));
            }
        }
        Ok(())
    }
}

enum Plan {
    Nothing,
    /// Clear the browser's cookie, deleting the stored session if there was one.
    Expire(Option<String>),
    Save(String, Record),
    /// New id: a first write, a login or an explicit cycle. The old record goes away.
    Create(Option<String>, Record),
}

fn cookie_value<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .filter_map(|pair| pair.trim().split_once('='))
        .find(|(k, _)| *k == name)
        .map(|(_, v)| v.trim())
}

fn plausible_token(t: &str) -> bool {
    t.len() == 43 && t.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

fn unix_now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

struct State {
    data: Map<String, Value>,
    created: u64,
    seen: u64,
    changed: bool,
    cycle: bool,
    destroyed: bool,
}

/// The current request's session. Changes are saved after the handler returns.
#[derive(Clone)]
pub struct Session(Arc<Mutex<State>>);

impl Session {
    fn new(record: Option<Record>, now: u64) -> Self {
        let (data, created, seen) = match record {
            Some(r) => (r.data, r.created, r.seen),
            None => (Map::new(), now, now),
        };
        Self(Arc::new(Mutex::new(State { data, created, seen, changed: false, cycle: false, destroyed: false })))
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn get<T: DeserializeOwned>(&self, key: &str) -> Option<T> {
        self.lock().data.get(key).and_then(|v| serde_json::from_value(v.clone()).ok())
    }

    pub fn insert(&self, key: &str, value: impl Serialize) -> Result<(), serde_json::Error> {
        let v = serde_json::to_value(value)?;
        let mut st = self.lock();
        st.data.insert(key.to_owned(), v);
        st.changed = true;
        Ok(())
    }

    pub fn remove(&self, key: &str) -> Option<Value> {
        let mut st = self.lock();
        let v = st.data.remove(key);
        st.changed |= v.is_some();
        v
    }

    pub fn clear(&self) {
        let mut st = self.lock();
        st.changed |= !st.data.is_empty();
        st.data.clear();
    }

    /// Issue a new session id on this response, keeping the data. Do it whenever privileges
    /// change, so an id planted before login (session fixation) becomes worthless.
    pub fn cycle(&self) {
        self.lock().cycle = true;
    }

    /// Delete the session from the store and expire the cookie.
    pub fn destroy(&self) {
        let mut st = self.lock();
        st.destroyed = true;
        st.data.clear();
    }

    /// Stores `user` (an id or a small struct) and cycles the id. Read it back with [`Auth`].
    pub fn login(&self, user: impl Serialize) -> Result<(), serde_json::Error> {
        self.insert(USER_KEY, user)?;
        let mut st = self.lock();
        st.cycle = true;
        st.destroyed = false;
        Ok(())
    }

    pub fn logout(&self) {
        self.destroy();
    }

    pub fn user<T: DeserializeOwned>(&self) -> Option<T> {
        self.get(USER_KEY)
    }
}

impl<S: Send + Sync> FromRequestParts<S> for Session {
    type Rejection = (StatusCode, &'static str);

    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, Self::Rejection> {
        parts.extensions.get::<Session>().cloned().ok_or((StatusCode::INTERNAL_SERVER_ERROR, "sessions are not enabled: add App::sessions"))
    }
}

/// The logged-in user stored by [`Session::login`]; answers 401 when there is none.
/// `Option<Auth<T>>` lets a handler serve both.
#[derive(Clone, Debug)]
pub struct Auth<T>(pub T);

impl<S: Send + Sync, T: DeserializeOwned> FromRequestParts<S> for Auth<T> {
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let session = Session::from_request_parts(parts, state).await.map_err(IntoResponse::into_response)?;
        session.user().map(Auth).ok_or_else(|| (StatusCode::UNAUTHORIZED, "login required").into_response())
    }
}

impl<S: Send + Sync, T: DeserializeOwned> OptionalFromRequestParts<S> for Auth<T> {
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Option<Self>, Self::Rejection> {
        let session = Session::from_request_parts(parts, state).await.map_err(IntoResponse::into_response)?;
        Ok(session.user().map(Auth))
    }
}

impl<S> Layer<S> for Sessions {
    type Service = SessionService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        SessionService { inner, sessions: self.clone() }
    }
}

#[derive(Clone)]
pub struct SessionService<S> {
    inner: S,
    sessions: Sessions,
}

impl<S> Service<Request> for SessionService<S>
where
    S: Service<Request, Response = Response> + Clone + Send + 'static,
    S::Future: Send + 'static,
{
    type Response = Response;
    type Error = S::Error;
    type Future = BoxFuture<'static, Result<Response, S::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), S::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, mut req: Request) -> Self::Future {
        // The clone that was polled ready must be the one that is called.
        let clone = self.inner.clone();
        let mut inner = std::mem::replace(&mut self.inner, clone);
        let sessions = self.sessions.clone();
        Box::pin(async move {
            let now = unix_now();
            let cookie = cookie_value(req.headers(), sessions.name());
            let had_cookie = cookie.is_some();
            let key = cookie.filter(|t| plausible_token(t)).map(|t| sha256_hex(t.as_bytes()));
            let mut record = None;
            if let Some(k) = &key {
                match sessions.store.load(k).await {
                    Ok(r) => record = r.filter(|r| sessions.deadline(r) > now),
                    Err(e) => {
                        eprintln!("ngs_olezhek: session store: {e}");
                        return Ok((StatusCode::SERVICE_UNAVAILABLE, "session store unavailable").into_response());
                    }
                }
            }
            let key = key.filter(|_| record.is_some());
            let session = Session::new(record, now);
            req.extensions_mut().insert(session.clone());
            let mut res = inner.call(req).await?;
            if let Err(e) = sessions.finish(&session, key, had_cookie, now, &mut res).await {
                eprintln!("ngs_olezhek: session store: {e}");
                return Ok((StatusCode::SERVICE_UNAVAILABLE, "session store unavailable").into_response());
            }
            Ok(res)
        })
    }
}
