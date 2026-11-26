//! Runs app Lua natively in `cargo test`: the real runtime prelude on top of a browser stand-in
//! (fake DOM, clock, fetch and storage). Needs the `testing` feature.
//!
//! ```ignore
//! // tests/ui.rs
//! #[test]
//! fn lua() {
//!     ngs_olezhek::testing::LuaTests::new("lua").run_dir("tests/lua");
//! }
//! ```
//!
//! A test file calls `test(name, function(t) ... end)` for each case; every case gets a fresh
//! runtime. `t` drives the app: `t.Go`, `t.Click`, `t.Type`, `t.Serve`, `t.Respond`, `t.Advance`...
//! (see `.ngs/ngs.lua` for the full list).

use std::{
    fmt::Write as _,
    fs, io,
    path::{Path, PathBuf},
};

use omnilua::{Function, Lua, Table};

#[path = "../../runtime/src/sandbox.rs"]
#[allow(dead_code)]
mod sandbox;

const PRELUDE: &[(&str, &str)] = &[
    ("base", include_str!("../../runtime/lua/base.lua")),
    ("ui", include_str!("../../runtime/lua/ui.lua")),
    ("state", include_str!("../../runtime/lua/state.lua")),
    ("style", include_str!("../../runtime/lua/style.lua")),
    ("attrs", include_str!("../../runtime/lua/attrs.lua")),
    ("controls", include_str!("../../runtime/lua/controls.lua")),
    ("surface", include_str!("../../runtime/lua/surface.lua")),
    ("http", include_str!("../../runtime/lua/http.lua")),
    ("storage", include_str!("../../runtime/lua/storage.lua")),
    ("browser", include_str!("../../runtime/lua/browser.lua")),
    ("date", include_str!("../../runtime/lua/date.lua")),
    ("widgets", include_str!("../../runtime/lua/widgets.lua")),
    ("kit", include_str!("../../runtime/lua/kit.lua")),
    ("charts", include_str!("../../runtime/lua/charts.lua")),
    ("calendar", include_str!("../../runtime/lua/calendar.lua")),
    ("form", include_str!("../../runtime/lua/form.lua")),
    ("router", include_str!("../../runtime/lua/router.lua")),
    ("sandbox", include_str!("../../runtime/lua/sandbox.lua")),
];

/// Runs after the harness: it needs `debug.traceback`, which lockdown removes.
const LOCKDOWN: &str = include_str!("../../runtime/lua/lockdown.lua");

const MOCK: &str = include_str!("mock.lua");
const HARNESS: &str = include_str!("harness.lua");

/// One booted runtime: prelude, browser stand-in and (optionally) the app's Lua files.
/// Globals `MOCK` (the fake browser state) and `R` (prelude internals) are set for tests.
pub struct Runtime {
    lua: Lua,
}

impl Runtime {
    /// The prelude alone, booted on `/` with no app files.
    pub fn new() -> Self {
        let rt = Self::bare(&[]).expect("prelude boots");
        rt.lua.load("R.boot({}, true)").exec().expect("boot");
        rt
    }

    /// The prelude plus the app files from `lua_dir`. Autorun has run; no page is shown until
    /// `t.Go` (or `TEST.go`) is called.
    pub fn with_app(lua_dir: impl AsRef<Path>) -> io::Result<Self> {
        Self::from_files(&read_app(lua_dir.as_ref())?)
    }

    fn from_files(files: &[(String, String)]) -> io::Result<Self> {
        let rt = Self::bare(files).map_err(io::Error::other)?;
        rt.lua.load("TEST.boot()").set_name("@ngs/testing").exec().map_err(|e| io::Error::other(e.to_string()))?;
        Ok(rt)
    }

    fn bare(files: &[(String, String)]) -> Result<Self, String> {
        let lua = Lua::new();
        let s = |e: omnilua::Error| e.to_string();
        let (n, m): (Table, Table) = lua.load(MOCK).set_name("@ngs/mock.lua").eval().map_err(s)?;
        sandbox::register(&lua, &n).map_err(s)?;
        let r = lua.create_table().map_err(s)?;
        for (name, src) in PRELUDE {
            lua.load(*src)
                .set_name(format!("@ngs/{name}.lua"))
                .into_function()
                .and_then(|f| f.call::<_, ()>((n.clone(), r.clone())))
                .map_err(|e| format!("prelude {name}: {e}"))?;
        }
        m.set("R", r.clone()).map_err(s)?;
        lua.globals().set("MOCK", m.clone()).map_err(s)?;
        lua.globals().set("R", r.clone()).map_err(s)?;

        let list = lua.create_table().map_err(s)?;
        let sources = lua.create_table().map_err(s)?;
        for (name, src) in files {
            let entry = lua.create_table().map_err(s)?;
            entry.set("name", name.as_str()).map_err(s)?;
            if name.starts_with("lazy/") {
                entry.set("deferred", true).map_err(s)?;
            } else {
                match lua.load(src.as_str()).set_name(format!("@{name}")).into_function() {
                    Ok(f) => entry.set("fn", f),
                    Err(e) => entry.set("err", e.to_string()),
                }
                .map_err(s)?;
            }
            sources.set(name.as_str(), src.as_str()).map_err(s)?;
            list.push(entry).map_err(s)?;
        }
        // Lazy modules are compiled when the router asks for them, like the real loader does.
        let compile = {
            let sources = sources.clone();
            lua.create_function(move |lua, name: String| -> omnilua::Result<(Option<Function>, Option<String>)> {
                let Some(src) = sources.get::<_, Option<String>>(name.as_str())? else {
                    return Ok((None, Some(format!("no such module {name}"))));
                };
                match lua.load(src).set_name(format!("@{name}")).into_function() {
                    Ok(f) => Ok((Some(f), None)),
                    Err(e) => Ok((None, Some(e.to_string()))),
                }
            })
            .map_err(s)?
        };
        let harness: Function = lua.load(HARNESS).set_name("@ngs/harness.lua").into_function().map_err(s)?;
        // omnilua converts tuples of at most three values, hence the context table.
        let ctx = lua.create_table().map_err(s)?;
        ctx.set("mock", m).map_err(s)?;
        ctx.set("files", list).map_err(s)?;
        ctx.set("compile", compile).map_err(s)?;
        harness.call::<_, ()>((n.clone(), r.clone(), ctx)).map_err(s)?;
        lua.load(LOCKDOWN).set_name("@ngs/lockdown.lua").into_function().and_then(|f| f.call::<_, ()>((n, r))).map_err(s)?;
        Ok(Self { lua })
    }

    pub fn lua(&self) -> &Lua {
        &self.lua
    }

    /// Runs a chunk; the error carries the Lua traceback.
    pub fn exec(&self, name: &str, source: &str) -> Result<(), String> {
        self.lua.load(source).set_name(format!("@{name}")).exec().map_err(|e| e.to_string())
    }

    /// Errors the runtime reported (`N.error`) that the test did not take with `t.Errors()`.
    pub fn take_errors(&self) -> Vec<String> {
        self.lua.load("return MOCK.take_errors()").eval().unwrap_or_default()
    }
}

impl Default for Runtime {
    fn default() -> Self {
        Self::new()
    }
}

/// Runs Lua test files against an app's Lua folder. Each `test(name, fn)` case runs in its own
/// runtime; a failing case does not stop the others, and the run panics at the end with a report.
pub struct LuaTests {
    lua_dir: PathBuf,
    filter: Option<String>,
}

#[derive(Debug)]
pub struct Failure {
    pub file: String,
    pub test: String,
    pub message: String,
}

impl LuaTests {
    /// `lua_dir` is the folder passed to `App::lua`, relative to the crate root.
    pub fn new(lua_dir: impl Into<PathBuf>) -> Self {
        Self { lua_dir: lua_dir.into(), filter: None }
    }

    /// Only cases whose name contains `text`. `NGS_TEST=<text>` in the environment does the same.
    pub fn filter(mut self, text: impl Into<String>) -> Self {
        self.filter = Some(text.into());
        self
    }

    /// Every `.lua` file under `dir`, recursively.
    pub fn run_dir(&self, dir: impl AsRef<Path>) {
        let dir = dir.as_ref();
        let mut files: Vec<_> = ngs_olezhek_bundle::collect(dir)
            .unwrap_or_else(|e| panic!("lua tests {}: {e}", dir.display()))
            .into_iter()
            .map(|(_, path)| path)
            .collect();
        files.sort();
        self.report(self.check_files(&files));
    }

    pub fn run_file(&self, path: impl AsRef<Path>) {
        self.report(self.check_files(&[path.as_ref().to_path_buf()]));
    }

    /// A test script given inline; `name` shows up in error messages.
    pub fn run(&self, name: &str, source: &str) {
        let app = read_app(&self.lua_dir).unwrap_or_else(|e| panic!("lua dir {}: {e}", self.lua_dir.display()));
        self.report(self.check(&app, name, source));
    }

    /// Runs the files and returns the failures instead of panicking.
    pub fn check_files(&self, files: &[PathBuf]) -> Vec<Failure> {
        let app = read_app(&self.lua_dir).unwrap_or_else(|e| panic!("lua dir {}: {e}", self.lua_dir.display()));
        let mut failures = Vec::new();
        for path in files {
            let name = path.to_string_lossy().replace('\\', "/");
            match fs::read_to_string(path) {
                Ok(src) => failures.extend(self.check(&app, &name, &src)),
                Err(e) => failures.push(Failure { file: name, test: String::new(), message: e.to_string() }),
            }
        }
        failures
    }

    fn check(&self, app: &[(String, String)], file: &str, source: &str) -> Vec<Failure> {
        let fail = |test: &str, message: String| Failure { file: file.into(), test: test.into(), message };
        let names: Vec<String> = match Runtime::from_files(app).and_then(|rt| {
            rt.exec(file, source).map_err(io::Error::other)?;
            rt.lua.load("return TEST.names()").eval::<Vec<String>>().map_err(|e| io::Error::other(e.to_string()))
        }) {
            Ok(n) => n,
            Err(e) => return vec![fail("", e.to_string())],
        };
        let env_filter = std::env::var("NGS_TEST").ok();
        let wanted = |n: &str| [&self.filter, &env_filter].iter().all(|f| f.as_ref().is_none_or(|f| n.contains(f.as_str())));
        let mut failures = Vec::new();
        for name in names.iter().filter(|n| wanted(n)) {
            match run_case(app, file, source, name) {
                Ok(()) => println!("lua test {file} :: {name} ... ok"),
                Err(message) => {
                    println!("lua test {file} :: {name} ... FAILED");
                    failures.push(fail(name, message));
                }
            }
        }
        failures
    }

    fn report(&self, failures: Vec<Failure>) {
        if failures.is_empty() {
            return;
        }
        let mut out = format!("{} lua test(s) failed:\n", failures.len());
        for f in &failures {
            let _ = writeln!(out, "\n--- {} :: {}\n{}", f.file, f.test, f.message);
        }
        panic!("{out}");
    }
}

fn run_case(app: &[(String, String)], file: &str, source: &str, name: &str) -> Result<(), String> {
    let rt = Runtime::from_files(app).map_err(|e| e.to_string())?;
    rt.exec(file, source)?;
    let run: Function = rt.lua.load("return TEST.run").eval().map_err(|e| e.to_string())?;
    let (ok, message): (bool, Option<String>) = run.call(name).map_err(|e| e.to_string())?;
    if !ok {
        return Err(message.unwrap_or_default());
    }
    let left = rt.take_errors();
    if !left.is_empty() {
        return Err(format!("runtime errors the test did not take with t.Errors():\n{}", left.join("\n")));
    }
    Ok(())
}

fn read_app(dir: &Path) -> io::Result<Vec<(String, String)>> {
    ngs_olezhek_bundle::collect(dir)?
        .into_iter()
        .map(|(name, path)| Ok((name, fs::read_to_string(path)?)))
        .collect()
}
