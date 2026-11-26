mod client_errors;
mod form;
mod live;
mod luals;
pub mod security;
#[cfg(feature = "testing")]
pub mod testing;

use std::{
    collections::{BTreeMap, HashMap},
    fmt,
    hash::{DefaultHasher, Hash, Hasher},
    io,
    net::SocketAddr,
    path::{Path, PathBuf},
    sync::{Arc, OnceLock},
};

use axum::{
    Router,
    body::{Body, Bytes},
    extract::{Path as RoutePath, ws::WebSocketUpgrade},
    middleware::map_response,
    http::{HeaderMap, HeaderName, HeaderValue, Method, StatusCode, Uri, header},
    response::{Html, IntoResponse, Response},
    routing::{any, get},
};
use ngs_olezhek_bundle::{Bundle, Entry, Kind};
use omnilua::Lua;
use security::{Cors, CrossOrigin, RateLimit, Sessions};
use tower_http::{compression::CompressionLayer, services::ServeDir};

pub use axum;
pub use client_errors::ClientError;
pub use form::{FieldErrors, Valid, Validate};
pub use luals::lua_types;
pub use ngs_olezhek_macros::{LuaType, delete, get, head, include_lua, options, patch, post, put, route, routes, trace};

#[doc(hidden)]
pub mod __private {
    pub use crate::luals::LuaClass;
    pub use inventory;
}

const RUNTIME_JS: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/runtime/runtime.js"));
const RUNTIME_WASM: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/runtime/runtime_bg.wasm"));

pub struct App {
    lua: LuaSource,
    public_dir: Option<PathBuf>,
    api: Router,
    title: String,
    metadata: BTreeMap<String, String>,
    dev: bool,
    strip: Option<bool>,
    api_prefix: String,
    csp: HeaderValue,
    cross_origin: Option<CrossOrigin>,
    sessions: Option<Sessions>,
    cors: Option<Cors>,
    client_errors: Option<client_errors::Handler>,
    client_error_limit: RateLimit,
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

impl App {
    pub fn new() -> Self {
        Self {
            lua: LuaSource::Dir("lua".into()),
            public_dir: None,
            api: Router::new(),
            title: "App".into(),
            metadata: Default::default(),
            dev: cfg!(debug_assertions),
            strip: None,
            api_prefix: "/api".into(),
            csp: HeaderValue::from_static(DEFAULT_CSP),
            cross_origin: Some(CrossOrigin::new()),
            sessions: None,
            cors: None,
            client_errors: Some(Arc::new(client_errors::print)),
            client_error_limit: RateLimit::per_minute(30).burst(10),
        }
    }

    /// A directory read at runtime, or `include_lua!("lua")` to ship the compiled bundle inside the binary.
    pub fn lua(mut self, source: impl Into<LuaSource>) -> Self {
        self.lua = source.into();
        self
    }

    /// Static files (images, fonts) served as-is; whatever is not found there falls through to the app.
    pub fn public(mut self, dir: impl Into<PathBuf>) -> Self {
        self.public_dir = Some(dir.into());
        self
    }

    pub fn api(mut self, router: Router) -> Self {
        self.api = self.api.merge(router);
        self
    }

    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }

    /// Set shell metadata visible before WebAssembly starts, including description and og:* properties.
    pub fn meta(mut self, name: impl Into<String>, content: impl Into<String>) -> Self {
        self.metadata.insert(name.into(), content.into());
        self
    }

    /// Dev mode rebuilds the Lua bundle on every page load, pushes changed files to open pages
    /// (hot reload), shows Lua errors on screen and writes LuaLS definitions next to the Lua folder.
    /// Defaults to `cfg!(debug_assertions)`.
    pub fn dev(mut self, dev: bool) -> Self {
        self.dev = dev;
        self
    }

    /// Strip debug info (line numbers, local names) from shipped bytecode. Defaults to `!dev`.
    pub fn strip(mut self, strip: bool) -> Self {
        self.strip = Some(strip);
        self
    }

    /// Unknown paths under this prefix answer 404 instead of the app page. Defaults to `/api`;
    /// an empty string turns the check off (the Sec-Fetch-Mode check still applies).
    pub fn api_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.api_prefix = prefix.into().trim_end_matches('/').to_string();
        self
    }

    /// Override the default same-origin policy for applications that use an external service.
    pub fn content_security_policy(mut self, policy: HeaderValue) -> Self {
        self.csp = policy;
        self
    }

    /// Cross-origin (CSRF) protection for every route, on by default. `false` turns it off,
    /// which only makes sense if something in front of the app does the same job.
    pub fn csrf(mut self, on: bool) -> Self {
        self.cross_origin = on.then(|| self.cross_origin.take().unwrap_or_default());
        self
    }

    /// Accept state-changing requests from pages on another origin (`https://admin.example.com`).
    pub fn trust_origin(mut self, origin: impl AsRef<str>) -> Self {
        if let Some(policy) = self.cross_origin.take() {
            self.cross_origin = Some(policy.trust(origin));
        }
        self
    }

    /// Cookie sessions for the API routes; handlers take `Session` or `Auth<T>`.
    /// Cookies are Secure outside dev mode unless `Sessions::secure` says otherwise.
    pub fn sessions(mut self, sessions: Sessions) -> Self {
        self.sessions = Some(sessions);
        self
    }

    /// CORS for pages on other origins; those origins are trusted by the CSRF check too.
    pub fn cors(mut self, cors: Cors) -> Self {
        for origin in cors.origin_list() {
            self = self.trust_origin(origin);
        }
        self.cors = Some(cors);
        self
    }

    /// Lua errors from browsers (release builds) are sent to `/_ngs/errors` and passed here.
    /// Default: printed to stderr. The handler runs on the request task, so hand slow work
    /// (a database, an HTTP call to an error tracker) to `tokio::spawn`.
    pub fn on_client_error(mut self, handler: impl Fn(ClientError) + Send + Sync + 'static) -> Self {
        self.client_errors = Some(Arc::new(handler));
        self
    }

    /// `false` stops browsers from sending errors and removes `/_ngs/errors`.
    pub fn client_errors(mut self, on: bool) -> Self {
        if !on {
            self.client_errors = None;
        } else if self.client_errors.is_none() {
            self.client_errors = Some(Arc::new(client_errors::print));
        }
        self
    }

    /// Rate limit for `/_ngs/errors`; default 30 per minute per client address. Behind a
    /// reverse proxy pass one keyed by `ClientKey::Forwarded`, or all visitors share one budget.
    pub fn client_error_limit(mut self, limit: RateLimit) -> Self {
        self.client_error_limit = limit;
        self
    }

    pub fn into_router(self) -> Result<Router, Error> {
        let strip = self.strip.unwrap_or(!self.dev);
        let live_dir = self.lua.live_dir();
        let report_errors = self.client_errors.is_some() && !self.dev;
        let source = BundleSource { lua: self.lua, dev: self.dev, strip, report_errors };

        // Fail at startup, not on the first request: in release the bundle is built once and
        // a syntax error should stop the deploy; in dev a missing directory is still worth catching.
        let prebuilt = source.build(|_| true)?;
        let errors = prebuilt.errors();
        let bundle = if self.dev {
            None
        } else if !errors.is_empty() {
            return Err(Error::Lua(errors));
        } else {
            Some(Arc::new(Assets::split(prebuilt)))
        };

        let shell = Arc::new(shell_html(&self.title, self.metadata));
        let js = Asset::new(RUNTIME_JS, "text/javascript; charset=utf-8");
        let wasm = Asset::new(RUNTIME_WASM, "application/wasm");

        let css = Asset::new(include_bytes!("../runtime/style.css"), "text/css; charset=utf-8");
        let start_source = source.clone();
        let start_bundle = bundle.clone();
        let internal = Router::new()
            .route("/_ngs/runtime.js", get(move |h: HeaderMap| async move { js.respond(&h) }))
            .route("/_ngs/runtime_bg.wasm", get(move |h: HeaderMap| async move { wasm.respond(&h) }))
            .route("/_ngs/style.css", get(move |h: HeaderMap| async move { css.respond(&h) }))
            .route("/_ngs/bundle", get(move || async move {
                bundle_response(&start_source, start_bundle.as_deref(), None)
            }))
            .route("/_ngs/page/{*name}", get(move |RoutePath(name): RoutePath<String>| async move {
                bundle_response(&source, bundle.as_deref(), Some(&name))
            }))
            .layer(CompressionLayer::new());

        // Merged before the session layer, so a report knows the logged-in user.
        let api = match &self.client_errors {
            Some(handler) if report_errors => self.api.merge(client_errors::router(handler.clone(), self.client_error_limit)),
            _ => self.api,
        };
        let api = match self.sessions {
            Some(sessions) => api.layer(sessions.default_secure(!self.dev)),
            None => api,
        };
        let mut router = api.merge(internal);
        if self.dev {
            // The watcher starts with the first connected page, so building a router (tests) costs nothing.
            let updates: Arc<OnceLock<Option<live::Updates>>> = Arc::default();
            router = router.route(
                "/_ngs/live",
                get(move |ws: WebSocketUpgrade| async move {
                    let tx = updates.get_or_init(|| live_dir.map(live::watch));
                    let rx = tx.as_ref().map(|t| t.subscribe());
                    ws.on_upgrade(move |socket| live::serve(socket, rx))
                }),
            );
        }

        let api_prefix = self.api_prefix;
        let page = any(move |method: Method, uri: Uri, headers: HeaderMap| async move {
            if !is_navigation(&method, uri.path(), &headers, &api_prefix) {
                return (StatusCode::NOT_FOUND, "Not Found").into_response();
            }
            ([(HeaderName::from_static(SHELL_HEADER), "1")], Html(shell.as_ref().clone())).into_response()
        });
        let router = match self.public_dir {
            Some(dir) => {
                router.fallback_service(ServeDir::new(dir).call_fallback_on_method_not_allowed(true).fallback(page))
            }
            None => router.fallback(page),
        };
        let router = match self.cross_origin {
            Some(policy) => router.layer(policy),
            None => router,
        };
        let router = match &self.cors {
            Some(cors) => router.layer(cors.layer()),
            None => router,
        };
        let csp = self.csp;
        let dev = self.dev;
        Ok(router.layer(map_response(move |mut response: Response| {
            let h = response.headers_mut();
            h.entry(header::CONTENT_SECURITY_POLICY).or_insert(csp.clone());
            h.entry(header::X_CONTENT_TYPE_OPTIONS).or_insert(HeaderValue::from_static("nosniff"));
            h.entry(header::REFERRER_POLICY).or_insert(HeaderValue::from_static("strict-origin-when-cross-origin"));
            h.entry(header::X_FRAME_OPTIONS).or_insert(HeaderValue::from_static("DENY"));
            if !dev {
                h.entry(header::STRICT_TRANSPORT_SECURITY).or_insert(HeaderValue::from_static("max-age=31536000"));
            }
            std::future::ready(response)
        })))
    }

    pub async fn serve(self, addr: &str) -> Result<(), Error> {
        let dev = self.dev;
        if dev && let Some(dir) = self.lua.live_dir() {
            luals::write(&dir);
        }
        let router = self.into_router()?;
        let listener = tokio::net::TcpListener::bind(addr).await?;
        println!("ngs_olezhek: http://{} ({})", listener.local_addr()?, if dev { "dev" } else { "release" });
        axum::serve(listener, router.into_make_service_with_connect_info::<SocketAddr>()).await?;
        Ok(())
    }
}

#[derive(Clone)]
pub enum LuaSource {
    Dir(PathBuf),
    Embedded(EmbeddedLua),
}

impl LuaSource {
    /// The folder with live sources, if it exists on this machine.
    fn live_dir(&self) -> Option<PathBuf> {
        let dir = match self {
            LuaSource::Dir(d) => d.as_path(),
            LuaSource::Embedded(e) => Path::new(e.dir),
        };
        dir.is_dir().then(|| std::path::absolute(dir).unwrap_or_else(|_| dir.to_path_buf()))
    }
}

impl From<&str> for LuaSource {
    fn from(dir: &str) -> Self {
        LuaSource::Dir(dir.into())
    }
}

impl From<String> for LuaSource {
    fn from(dir: String) -> Self {
        LuaSource::Dir(dir.into())
    }
}

impl From<&Path> for LuaSource {
    fn from(dir: &Path) -> Self {
        LuaSource::Dir(dir.into())
    }
}

impl From<PathBuf> for LuaSource {
    fn from(dir: PathBuf) -> Self {
        LuaSource::Dir(dir)
    }
}

impl From<EmbeddedLua> for LuaSource {
    fn from(lua: EmbeddedLua) -> Self {
        LuaSource::Embedded(lua)
    }
}

/// Produced by `include_lua!`: bytecode compiled at build time (with debug info; stripped at
/// startup unless `App::strip(false)`), plus the source dir so dev mode can still read live files.
#[derive(Clone, Copy)]
pub struct EmbeddedLua {
    dir: &'static str,
    files: &'static [(&'static str, &'static [u8])],
}

impl EmbeddedLua {
    #[doc(hidden)]
    pub const fn from_parts(dir: &'static str, files: &'static [(&'static str, &'static [u8])]) -> Self {
        Self { dir, files }
    }

    fn bundle(&self, dev: bool, strip: bool, include: impl Fn(&str) -> bool) -> Bundle {
        let lua = Lua::new();
        let entries = self
            .files
            .iter()
            .filter(|(name, _)| include(name))
            .map(|(name, data)| {
                let entry = Entry { name: name.to_string(), kind: Kind::Chunk, data: data.to_vec() };
                if strip { entry.strip(&lua) } else { entry }
            })
            .collect();
        Bundle { dev, report_errors: false, entries }
    }
}

#[derive(Clone)]
struct BundleSource {
    lua: LuaSource,
    dev: bool,
    strip: bool,
    report_errors: bool,
}

impl BundleSource {
    fn startup(&self) -> Result<Bundle, Error> {
        let mut bundle = self.build(|name| !name.starts_with("lazy/"))?;
        let names = match self.lua.live_dir() {
            Some(dir) => ngs_olezhek_bundle::collect(&dir)?.into_iter().map(|(name, _)| name).collect::<Vec<_>>(),
            None => match &self.lua {
                LuaSource::Embedded(e) => e.files.iter().map(|(name, _)| name.to_string()).collect(),
                LuaSource::Dir(_) => return Ok(bundle),
            },
        };
        bundle.entries.extend(names.iter().filter(|n| n.starts_with("lazy/")).map(|n| Entry::deferred(n)));
        Ok(bundle)
    }

    fn build(&self, include: impl Fn(&str) -> bool) -> Result<Bundle, Error> {
        let dir = match &self.lua {
            LuaSource::Dir(dir) => dir.as_path(),
            LuaSource::Embedded(e) if self.dev && Path::new(e.dir).is_dir() => Path::new(e.dir),
            LuaSource::Embedded(e) => {
                let mut bundle = e.bundle(self.dev, self.strip, include);
                bundle.report_errors = self.report_errors;
                return Ok(bundle);
            }
        };
        let mut bundle = Bundle::from_dir_matching(dir, self.dev, self.strip, include)
            .map_err(|e| Error::Io(io::Error::new(e.kind(), format!("lua dir {}: {e}", dir.display()))))?;
        bundle.report_errors = self.report_errors;
        Ok(bundle)
    }
}

struct Assets {
    startup: Bytes,
    pages: HashMap<String, Bytes>,
}

impl Assets {
    fn split(mut bundle: Bundle) -> Self {
        let mut pages = HashMap::new();
        let mut eager = Vec::new();
        for entry in bundle.entries {
            if let Some(name) = entry.name.strip_prefix("lazy/") {
                eager.push(Entry::deferred(&entry.name));
                pages.insert(name.to_owned(), Bundle { dev: bundle.dev, report_errors: false, entries: vec![entry] }.encode().into());
            } else {
                eager.push(entry);
            }
        }
        bundle.entries = eager;
        Self { startup: bundle.encode().into(), pages }
    }
}

fn bundle_response(source: &BundleSource, built: Option<&Assets>, page: Option<&str>) -> Response {
    let bytes = match built {
        Some(assets) => match page {
            None => assets.startup.clone(),
            Some(name) => match assets.pages.get(name) {
                Some(bytes) => bytes.clone(),
                None => return StatusCode::NOT_FOUND.into_response(),
            },
        },
        None => match page.map_or_else(|| source.startup(), |page| source.build(|name| name.strip_prefix("lazy/") == Some(page))) {
            Ok(b) if page.is_some() && b.entries.is_empty() => return StatusCode::NOT_FOUND.into_response(),
            Ok(b) => b.encode().into(),
            Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
        },
    };
    ([(header::CONTENT_TYPE, "application/octet-stream"), (header::CACHE_CONTROL, "no-cache")], bytes).into_response()
}

#[derive(Clone, Copy)]
struct Asset {
    bytes: &'static [u8],
    content_type: &'static str,
    etag: u64,
}

impl Asset {
    fn new(bytes: &'static [u8], content_type: &'static str) -> Self {
        let mut h = DefaultHasher::new();
        bytes.hash(&mut h);
        Self { bytes, content_type, etag: h.finish() }
    }

    fn respond(&self, headers: &HeaderMap) -> Response {
        let etag = format!("\"{:x}\"", self.etag);
        let fresh = headers.get(header::IF_NONE_MATCH).is_some_and(|v| v.as_bytes() == etag.as_bytes());
        let mut res = if fresh {
            StatusCode::NOT_MODIFIED.into_response()
        } else {
            let mut r = Response::new(Body::from(self.bytes));
            r.headers_mut().insert(header::CONTENT_TYPE, HeaderValue::from_static(self.content_type));
            r
        };
        let h = res.headers_mut();
        h.insert(header::ETAG, HeaderValue::from_str(&etag).unwrap());
        h.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
        res
    }
}

/// Marks the app page so the client can tell "this endpoint does not exist" from a real answer
/// even when the fallback did serve it (a navigation-looking request to a missing endpoint).
const SHELL_HEADER: &str = "x-ngs-shell";

/// The SPA fallback answers only page loads: a typo in `/api/...`, a missing font or a fetch()
/// to an unknown URL gets a 404 rather than 200 with HTML.
fn is_navigation(method: &Method, path: &str, headers: &HeaderMap, api_prefix: &str) -> bool {
    if method != Method::GET && method != Method::HEAD {
        return false;
    }
    let under = |prefix: &str| path == prefix || path.strip_prefix(prefix).is_some_and(|rest| rest.starts_with('/'));
    if under("/_ngs") || (!api_prefix.is_empty() && under(api_prefix)) {
        return false;
    }
    let header = |name: header::HeaderName| headers.get(name).and_then(|v| v.to_str().ok());
    // Every current browser sends it, and only page loads are "navigate".
    if let Some(mode) = header(HeaderName::from_static("sec-fetch-mode")) {
        return mode == "navigate";
    }
    header(header::ACCEPT).is_none_or(|a| a.contains("text/html") || a.contains("*/*"))
}

const BOOT_SCRIPT: &str = include_str!(concat!(env!("OUT_DIR"), "/bootstrap.js"));
/// Default policy including the build-specific SHA-256 of the shell bootstrap.
pub const DEFAULT_CSP: &str = concat!("default-src 'self'; script-src 'self' 'wasm-unsafe-eval' 'sha256-", env!("NGS_BOOT_HASH"),
    "'; style-src 'self'; img-src 'self' data: blob:; connect-src 'self'; object-src 'none'; base-uri 'none'; frame-ancestors 'none'; form-action 'self'");

fn shell_html(title: &str, mut metadata: BTreeMap<String, String>) -> String {
    fn escape(value: &str) -> String {
        value.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
    }
    metadata.entry("viewport".into()).or_insert_with(|| "width=device-width, initial-scale=1".into());
    let mut tags = String::new();
    for (name, content) in metadata {
        use std::fmt::Write;
        let attribute = if name.starts_with("og:") { "property" } else { "name" };
        writeln!(tags, r#"<meta {attribute}="{}" content="{}">"#, escape(&name), escape(&content)).unwrap();
    }
    let title = escape(title);
    format!(
        r#"<!doctype html>
<html>
<head>
<meta charset="utf-8">
{tags}<link id="ngs-style" rel="stylesheet" href="/_ngs/style.css">
<title>{title}</title>
</head>
<body>
<script type="module">{BOOT_SCRIPT}</script>
</body>
</html>
"#
    )
}

#[derive(Debug)]
pub enum Error {
    Io(io::Error),
    /// Compile errors, one per broken file.
    Lua(Vec<String>),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(e) => write!(f, "{e}"),
            Error::Lua(errors) => write!(f, "lua compile errors:\n{}", errors.join("\n")),
        }
    }
}

impl std::error::Error for Error {}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Error::Io(e)
    }
}
