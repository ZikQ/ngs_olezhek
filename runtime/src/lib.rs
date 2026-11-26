mod args;
mod dom;
mod events;
mod json;
mod live;
mod natives;
mod net;
mod paint;

use std::{cell::RefCell, fmt::Display};

use ngs_olezhek_bundle::{Bundle, Kind};
use omnilua::{FromLuaMulti, Function, IntoLuaMulti, Lua, LuaError, Table};
use wasm_bindgen::prelude::*;

const PRELUDE: &[(&str, &str)] = &[
    ("base", include_str!("../lua/base.lua")),
    ("ui", include_str!("../lua/ui.lua")),
    ("style", include_str!("../lua/style.lua")),
    ("attrs", include_str!("../lua/attrs.lua")),
    ("controls", include_str!("../lua/controls.lua")),
    ("surface", include_str!("../lua/surface.lua")),
    ("http", include_str!("../lua/http.lua")),
    ("storage", include_str!("../lua/storage.lua")),
    ("router", include_str!("../lua/router.lua")),
];

thread_local! {
    /// The Lua state and the prelude's private `R` table with entry points (event, flush, resized...).
    static RT: RefCell<Option<(Lua, Table)>> = const { RefCell::new(None) };
}

pub(crate) fn err(msg: impl Display) -> omnilua::Error {
    LuaError::runtime(format_args!("{msg}")).into()
}

pub(crate) fn load_chunk(name: &str, data: &[u8]) -> omnilua::Result<Function> {
    let lua = RT.with(|rt| rt.borrow().as_ref().map(|(l, _)| l.clone())).ok_or_else(|| err("runtime not booted"))?;
    lua.load(data).set_name(format!("@{name}")).into_function()
}

/// Calls a prelude entry point `R.<name>`. Never call this while holding a `RefCell` borrow:
/// Lua code calls back into natives that borrow the same state.
pub(crate) fn call<R: FromLuaMulti>(name: &str, args: impl IntoLuaMulti) -> Option<R> {
    let r = RT.with(|rt| rt.borrow().as_ref().map(|(_, r)| r.clone()))?;
    let result = r.get::<_, Function>(name).and_then(|f| f.call(args));
    match result {
        Ok(v) => Some(v),
        Err(e) => {
            dom::report_error(&format!("R.{name}: {e}"));
            None
        }
    }
}

#[wasm_bindgen(start)]
pub fn start() {
    std::panic::set_hook(Box::new(|info| web_sys::console::error_1(&info.to_string().into())));
    wasm_bindgen_futures::spawn_local(async {
        if let Err(e) = boot().await {
            dom::report_error(&format!("ngs runtime: {e}"));
        }
    });
}

async fn boot() -> Result<(), String> {
    dom::install_style();
    let bytes = net::fetch_bytes("/_ngs/bundle").await?;
    let bundle = Bundle::decode(&bytes)?;
    dom::set_dev(bundle.dev);

    let lua = Lua::new();
    let s = |e: omnilua::Error| e.to_string();
    let n = natives::create(&lua).map_err(s)?;
    let r = lua.create_table().map_err(s)?;
    for (name, src) in PRELUDE {
        lua.load(*src)
            .set_name(format!("@ngs/{name}.lua"))
            .into_function()
            .and_then(|f| f.call::<_, ()>((n.clone(), r.clone())))
            .map_err(s)?;
    }
    RT.with(|rt| *rt.borrow_mut() = Some((lua.clone(), r)));
    events::install();

    let files = lua.create_table().map_err(s)?;
    for (i, entry) in bundle.entries.into_iter().enumerate() {
        let t = lua.create_table().map_err(s)?;
        t.set("name", entry.name.as_str()).map_err(s)?;
        match entry.kind {
            Kind::Chunk => match load_chunk(&entry.name, &entry.data) {
                Ok(f) => t.set("fn", f),
                Err(e) => t.set("err", e.to_string()),
            },
            Kind::Error => t.set("err", String::from_utf8_lossy(&entry.data).into_owned()),
        }
        .map_err(s)?;
        files.set(i + 1, t).map_err(s)?;
    }
    call::<()>("boot", (files, bundle.dev));
    if bundle.dev {
        live::connect();
    }
    Ok(())
}
