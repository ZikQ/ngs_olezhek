// Long Args<(..)> tuples are the whole point here: they mirror the Lua call signature.
#![allow(clippy::type_complexity)]

use std::collections::HashMap;

use omnilua::{FromLuaMulti, Function, IntoLuaMulti, Lua, Result, Table, Value};
use wasm_bindgen::JsValue;

use crate::{args::Args, dom, events, json, net, paint};

pub(crate) fn reg<A, R, F>(lua: &Lua, t: &Table, name: &str, f: F) -> Result<()>
where
    A: FromLuaMulti + 'static,
    R: IntoLuaMulti + 'static,
    F: Fn(&Lua, A) -> Result<R> + 'static,
{
    // A second registration would silently replace the first (this happened with "rect").
    if !matches!(t.raw_get::<_, Value>(name)?, Value::Nil) {
        return Err(crate::err(format!("native `{name}` registered twice")));
    }
    t.set(name, lua.create_function(f)?)
}

/// The `N` table handed to the prelude chunks. User code never sees it.
pub fn create(lua: &Lua) -> Result<Table> {
    let n = lua.create_table()?;
    dom_natives(lua, &n)?;
    paint_natives(lua, &n)?;
    platform_natives(lua, &n)?;
    crate::browser::register(lua, &n)?;
    crate::sandbox::register(lua, &n)?;
    Ok(n)
}

type Opt = Option<String>;

fn dom_natives(lua: &Lua, n: &Table) -> Result<()> {
    reg(lua, n, "create", |_, Args((tag, class, control, ty)): Args<(String, String, Opt, Opt)>| {
        dom::create(&tag, &class, control.as_deref(), ty.as_deref())
    })?;
    reg(lua, n, "set_control", |_, (id, tag, ty): (u32, String, Opt)| dom::set_control(id, &tag, ty.as_deref()))?;
    reg(lua, n, "mount", |_, id: u32| dom::mount(id))?;
    reg(lua, n, "append", |_, (parent, child): (u32, u32)| dom::append(parent, child))?;
    reg(lua, n, "remove", |_, id: u32| dom::remove(id))?;
    reg(lua, n, "style", |_, (id, prop, value): (u32, String, Opt)| dom::style(id, &prop, value.as_deref()))?;
    reg(lua, n, "attr", |_, Args((id, name, value, on_control)): Args<(u32, String, Opt, Option<bool>)>| {
        dom::attr(id, &name, value.as_deref(), on_control.unwrap_or(false))
    })?;
    reg(lua, n, "get_attr", |_, (id, name, on_control): (u32, String, Option<bool>)| {
        dom::get_attr(id, &name, on_control.unwrap_or(false))
    })?;
    reg(lua, n, "prop", |_, (id, name, value): (u32, String, Value)| dom::prop(id, &name, value))?;
    reg(lua, n, "get_prop", |lua, (id, name): (u32, String)| dom::get_prop(lua, id, &name))?;
    reg(lua, n, "class", |_, (id, name, on): (u32, String, bool)| dom::class(id, &name, on))?;
    reg(lua, n, "text", |_, (id, s): (u32, String)| dom::text(id, &s))?;
    reg(lua, n, "size", |_, id: u32| dom::size(id))?;
    reg(lua, n, "offset", |_, id: u32| dom::offset(id))?;
    reg(lua, n, "bounds", |_, id: u32| dom::rect(id).map(Args))?;
    reg(lua, n, "invoke", |_, (id, method): (u32, String)| dom::invoke(id, &method))?;
    reg(lua, n, "capture", |_, (id, on): (u32, bool)| dom::capture(id, on))?;
    reg(lua, n, "focus", |_, id: u32| dom::focus(id))?;
    reg(lua, n, "blur", |_, id: u32| dom::blur(id))?;
    reg(lua, n, "has_focus", |_, id: u32| dom::has_focus(id))?;
    reg(lua, n, "add_option", |_, (id, text): (u32, String)| dom::add_option(id, &text))?;
    reg(lua, n, "clear_options", |_, id: u32| dom::clear_options(id))?;
    reg(lua, n, "scroll_into_view", |_, id: u32| dom::scroll_into_view(id))?;
    reg(lua, n, "observe", |_, (id, on): (u32, bool)| dom::observe(id, on))?;
    reg(lua, n, "css_var", |_, (name, value): (String, String)| dom::css_var(&name, &value))?;
    reg(lua, n, "css_rule", |_, (selector, decls): (String, Vec<String>)| dom::css_rule(&selector, &decls))?;
    Ok(())
}

fn paint_natives(lua: &Lua, n: &Table) -> Result<()> {
    reg(lua, n, "canvas_begin", |_, Args((id, w, h, font)): Args<(u32, f64, f64, String)>| paint::begin(id, w, h, font))?;
    reg(lua, n, "canvas_end", |_, ()| {
        paint::end();
        Ok(())
    })?;
    reg(lua, n, "color", |_, Args((r, g, b, a)): Args<(f64, f64, f64, Option<f64>)>| paint::color(r, g, b, a.unwrap_or(255.0)))?;
    reg(lua, n, "text_color", |_, Args((r, g, b, a)): Args<(f64, f64, f64, Option<f64>)>| {
        paint::text_color(r, g, b, a.unwrap_or(255.0))
    })?;
    reg(lua, n, "font", |_, css: String| paint::font(css))?;
    reg(lua, n, "alpha", |_, a: f64| paint::alpha(a))?;
    reg(lua, n, "rect", |_, Args((x, y, w, h)): Args<(f64, f64, f64, f64)>| paint::rect(x, y, w, h))?;
    reg(lua, n, "outlined", |_, Args((x, y, w, h, t)): Args<(f64, f64, f64, f64, f64)>| paint::outlined(x, y, w, h, t))?;
    reg(lua, n, "line", |_, Args((x1, y1, x2, y2)): Args<(f64, f64, f64, f64)>| paint::line(x1, y1, x2, y2))?;
    reg(
        lua,
        n,
        "rounded",
        |_, Args((r, x, y, w, h, tl, tr, bl, br)): Args<(f64, f64, f64, f64, f64, bool, bool, bool, bool)>| {
            paint::rounded(r, x, y, w, h, tl, tr, bl, br)
        },
    )?;
    reg(lua, n, "circle", |_, Args((x, y, r, fill)): Args<(f64, f64, f64, bool)>| paint::circle(x, y, r, fill))?;
    reg(lua, n, "poly", |_, verts: Table| paint::poly(verts))?;
    reg(lua, n, "gradient", |_, Args((x1, y1, x2, y2, stops)): Args<(f64, f64, f64, f64, Vec<f64>)>| {
        paint::gradient(x1, y1, x2, y2, stops)
    })?;
    reg(lua, n, "line_width", |_, w: f64| paint::line_width(w))?;
    reg(lua, n, "line_cap", |_, cap: String| paint::line_cap(&cap))?;
    reg(lua, n, "line_join", |_, join: String| paint::line_join(&join))?;
    reg(lua, n, "line_dash", |_, pattern: Vec<f64>| paint::line_dash(pattern))?;
    reg(lua, n, "arc", |_, Args((x, y, r, from, to, fill)): Args<(f64, f64, f64, f64, f64, bool)>| {
        paint::arc(x, y, r, from, to, fill)
    })?;
    reg(lua, n, "path", |_, Args((d, mode, x, y, sx, sy)): Args<(String, String, f64, f64, f64, f64)>| {
        paint::path(&d, &mode, x, y, sx, sy)
    })?;
    reg(lua, n, "draw_text", |_, (s, x, y): (String, f64, f64)| paint::draw_text(&s, x, y))?;
    reg(lua, n, "measure", |_, (s, font): (String, String)| paint::measure(&s, &font))?;
    reg(lua, n, "image_load", |_, url: String| paint::image_load(&url))?;
    reg(lua, n, "image", |_, Args((img, x, y, w, h)): Args<(u32, f64, f64, f64, f64)>| paint::image(img, x, y, w, h))?;
    reg(
        lua,
        n,
        "image_uv",
        |_, Args((img, x, y, w, h, u0, v0, u1, v1)): Args<(u32, f64, f64, f64, f64, f64, f64, f64, f64)>| {
            paint::image_uv(img, x, y, w, h, u0, v0, u1, v1)
        },
    )?;
    reg(lua, n, "image_size", |_, img: u32| paint::image_size(img))?;
    reg(lua, n, "load_font", |_, Args((family, url, weight, style)): Args<(String, String, String, String)>| {
        paint::load_font(family, &url, &weight, &style)
    })?;
    Ok(())
}

fn platform_natives(lua: &Lua, n: &Table) -> Result<()> {
    let window = dom::window();
    let perf = window.performance().expect("performance API");
    reg(lua, n, "now", move |_, ()| Ok(perf.now() / 1000.0))?;
    reg(lua, n, "mouse", |_, ()| Ok(events::mouse()))?;
    reg(lua, n, "screen", |_, ()| {
        let w = dom::window();
        let get = |v: std::result::Result<JsValue, JsValue>| v.ok().and_then(|v| v.as_f64()).unwrap_or(0.0);
        Ok((get(w.inner_width()), get(w.inner_height())))
    })?;
    reg(lua, n, "schedule", |_, ()| {
        net::schedule_flush();
        Ok(())
    })?;
    reg(lua, n, "think", |_, on: bool| {
        net::think(on);
        Ok(())
    })?;
    reg(lua, n, "timeout", |_, (ms, f): (f64, Function)| Ok(net::timeout(ms, f)))?;
    reg(lua, n, "clear_timeout", |_, handle: i32| {
        net::clear_timeout(handle);
        Ok(())
    })?;
    type FetchArgs = Args<(String, String, Opt, Option<HashMap<String, String>>)>;
    reg(lua, n, "fetch", |_, Args((method, url, body, headers)): FetchArgs| {
        Ok(net::fetch(method, url, body.map(JsValue::from), headers.unwrap_or_default()))
    })?;
    reg(lua, n, "abort", |_, op: u32| {
        net::abort(op);
        Ok(())
    })?;
    reg(lua, n, "json_encode", |_, (v, pretty): (Value, Option<bool>)| json::encode(v, pretty.unwrap_or(false)))?;
    reg(lua, n, "json_decode", |lua, s: String| json::decode(lua, &s))?;
    reg(lua, n, "log", |_, s: String| {
        web_sys::console::log_1(&s.into());
        Ok(())
    })?;
    reg(lua, n, "error", |_, s: String| {
        dom::report_error(&s);
        Ok(())
    })?;
    reg(lua, n, "location", |_, ()| Ok(events::location()))?;
    reg(lua, n, "origin", |_, ()| dom::window().location().origin().map_err(dom::js))?;
    if events::history_index()?.is_none() { events::set_history(None, false)?; }
    reg(lua, n, "history_index", |_, ()| events::history_index())?;
    reg(lua, n, "history_go", |_, delta: i32| dom::window().history().map_err(dom::js)?.go_with_delta(delta).map_err(dom::js))?;
    reg(lua, n, "push_state", |_, url: String| events::set_history(Some(&url), true))?;
    reg(lua, n, "replace_state", |_, url: String| events::set_history(Some(&url), false))?;
    reg(lua, n, "back", |_, ()| dom::window().history().map_err(dom::js)?.back().map_err(dom::js))?;
    reg(lua, n, "document_title", |_, ()| Ok(dom::document().title()))?;
    reg(lua, n, "title", |_, s: String| {
        dom::document().set_title(&s);
        Ok(())
    })?;
    // Lua checks the scheme before calling this; noopener keeps the new tab away from window.opener.
    reg(lua, n, "open", |_, url: String| {
        dom::window().open_with_url_and_target_and_features(&url, "_blank", "noopener").map_err(dom::js)?;
        Ok(())
    })?;
    storage_natives(lua, n)
}

/// Storage failures (private mode, quota, blocked cookies) come back as nil/false, never as errors.
fn storage_natives(lua: &Lua, n: &Table) -> Result<()> {
    fn storage(session: bool) -> Option<web_sys::Storage> {
        let w = dom::window();
        if session { w.session_storage() } else { w.local_storage() }.ok().flatten()
    }
    reg(lua, n, "storage_get", |_, (key, session): (String, bool)| {
        Ok(storage(session).and_then(|s| s.get_item(&key).ok().flatten()))
    })?;
    reg(lua, n, "storage_set", |_, (key, value, session): (String, String, bool)| {
        Ok(storage(session).is_some_and(|s| s.set_item(&key, &value).is_ok()))
    })?;
    reg(lua, n, "storage_remove", |_, (key, session): (String, bool)| {
        Ok(storage(session).is_some_and(|s| s.remove_item(&key).is_ok()))
    })?;
    reg(lua, n, "storage_keys", |_, session: bool| {
        let Some(s) = storage(session) else { return Ok(Vec::new()) };
        let len = s.length().unwrap_or(0);
        Ok((0..len).filter_map(|i| s.key(i).ok().flatten()).collect::<Vec<String>>())
    })?;
    Ok(())
}
