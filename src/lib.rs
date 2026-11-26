mod live;
mod luals;

use std::{
    fmt,
    hash::{DefaultHasher, Hash, Hasher},
    io,
    path::{Path, PathBuf},
    sync::{Arc, OnceLock},
};

use axum::{
    Router,
    body::Body,
    extract::ws::WebSocketUpgrade,
    http::{HeaderMap, HeaderName, HeaderValue, Method, StatusCode, Uri, header},
    response::{Html, IntoResponse, Response},
    routing::{any, get},
};
use ngs_olezhek_bundle::{Bundle, Entry, Kind};
use omnilua::Lua;
use tower_http::{compression::CompressionLayer, services::ServeDir};

pub use axum;
pub use ngs_olezhek_macros::{delete, get, head, include_lua, options, patch, post, put, route, routes, trace};

const RUNTIME_JS: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/runtime/runtime.js"));
const RUNTIME_WASM: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/runtime/runtime_bg.wasm"));

pub struct App {
    lua: LuaSource,
    public_dir: Option<PathBuf>,
    api: Router,
    title: String,
    dev: bool,
    strip: Option<bool>,
    api_prefix: String,
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
            dev: cfg!(debug_assertions),
            strip: None,
            api_prefix: "/api".into(),
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

    pub fn into_router(self) -> Result<Router, Error> {
        let strip = self.strip.unwrap_or(!self.dev);
        let live_dir = self.lua.live_dir();
        let source = BundleSource { lua: self.lua, dev: self.dev, strip };

        // Fail at startup, not on the first request: in release the bundle is built once and
        // a syntax error should stop the deploy; in dev a missing directory is still worth catching.
        let prebuilt = source.build()?;
        let errors = prebuilt.errors();
        let bundle = if self.dev {
            None
        } else if !errors.is_empty() {
            return Err(Error::Lua(errors));
        } else {
            Some(Arc::new(prebuilt.encode()))
        };

        let shell = Arc::new(shell_html(&self.title));
        let js = Asset::new(RUNTIME_JS, "text/javascript; charset=utf-8");
        let wasm = Asset::new(RUNTIME_WASM, "application/wasm");

        let internal = Router::new()
            .route("/_ngs/runtime.js", get(move |h: HeaderMap| async move { js.respond(&h) }))
            .route("/_ngs/runtime_bg.wasm", get(move |h: HeaderMap| async move { wasm.respond(&h) }))
            .route(
                "/_ngs/bundle",
                get(move || async move {
                    let bytes = match &bundle {
                        Some(b) => b.as_ref().clone(),
                        None => match source.build() {
                            Ok(b) => b.encode(),
                            Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
                        },
                    };
                    ([(header::CONTENT_TYPE, "application/octet-stream"), (header::CACHE_CONTROL, "no-cache")], bytes)
                        .into_response()
                }),
            )
            .layer(CompressionLayer::new());

        let mut router = self.api.merge(internal);
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
        Ok(match self.public_dir {
            Some(dir) => {
                router.fallback_service(ServeDir::new(dir).call_fallback_on_method_not_allowed(true).fallback(page))
            }
            None => router.fallback(page),
        })
    }

    pub async fn serve(self, addr: &str) -> Result<(), Error> {
        let dev = self.dev;
        if dev && let Some(dir) = self.lua.live_dir() {
            luals::write(&dir);
        }
        let router = self.into_router()?;
        let listener = tokio::net::TcpListener::bind(addr).await?;
        println!("ngs_olezhek: http://{} ({})", listener.local_addr()?, if dev { "dev" } else { "release" });
        axum::serve(listener, router).await?;
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

    fn bundle(&self, dev: bool, strip: bool) -> Bundle {
        let lua = Lua::new();
        let entries = self
            .files
            .iter()
            .map(|(name, data)| {
                let entry = Entry { name: name.to_string(), kind: Kind::Chunk, data: data.to_vec() };
                if strip { entry.strip(&lua) } else { entry }
            })
            .collect();
        Bundle { dev, entries }
    }
}

#[derive(Clone)]
struct BundleSource {
    lua: LuaSource,
    dev: bool,
    strip: bool,
}

impl BundleSource {
    fn build(&self) -> Result<Bundle, Error> {
        match &self.lua {
            LuaSource::Dir(dir) => self.read_dir(dir),
            // Dev reads the live folder so edits show up on refresh without recompiling;
            // if the binary runs somewhere the folder doesn't exist, fall back to what's embedded.
            LuaSource::Embedded(e) if self.dev && Path::new(e.dir).is_dir() => self.read_dir(Path::new(e.dir)),
            LuaSource::Embedded(e) => Ok(e.bundle(self.dev, self.strip)),
        }
    }

    fn read_dir(&self, dir: &Path) -> Result<Bundle, Error> {
        Bundle::from_dir(dir, self.dev, self.strip)
            .map_err(|e| Error::Io(io::Error::new(e.kind(), format!("lua dir {}: {e}", dir.display()))))
    }
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

fn shell_html(title: &str) -> String {
    let title = title.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;");
    format!(
        r#"<!doctype html>
<html>
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{title}</title>
</head>
<body>
<script type="module">import init from "/_ngs/runtime.js"; init({{ module_or_path: "/_ngs/runtime_bg.wasm" }});</script>
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
