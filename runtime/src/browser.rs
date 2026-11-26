use std::{cell::RefCell, collections::HashMap};

use omnilua::{Lua, Result, Table};
use wasm_bindgen::{JsCast, JsValue, prelude::Closure};
use wasm_bindgen_futures::{JsFuture, spawn_local};
use web_sys::{Event, EventSource, EventTarget, FormData, HtmlDocument, HtmlInputElement, MessageEvent, WebSocket};

use crate::{args::Args, dom, err, natives::reg, net};

enum Stream {
    Socket(WebSocket),
    Events(EventSource),
}

thread_local! {
    static STREAMS: RefCell<HashMap<u32, Stream>> = RefCell::new(HashMap::new());
}

fn close(id: u32) {
    match STREAMS.with(|s| s.borrow_mut().remove(&id)) {
        Some(Stream::Socket(ws)) => {
            let _ = ws.close();
        }
        Some(Stream::Events(source)) => source.close(),
        None => (),
    }
}

fn connect(url: &str, events: bool) -> Result<u32> {
    let id = net::next_op();
    let (target, stream): (EventTarget, Stream) = if events {
        let source = EventSource::new(url).map_err(dom::js)?;
        (source.clone().into(), Stream::Events(source))
    } else {
        let ws = WebSocket::new(url).map_err(dom::js)?;
        (ws.clone().into(), Stream::Socket(ws))
    };
    for name in ["open", "message", "error", "close"] {
        let callback = Closure::<dyn FnMut(Event)>::new(move |e: Event| {
            let text = e.dyn_ref::<MessageEvent>().and_then(|m| m.data().as_string());
            crate::call::<()>("stream_event", (id, name, text));
            let ended = name == "error"
                && e.target()
                    .and_then(|t| t.dyn_into::<EventSource>().ok())
                    .is_some_and(|s| s.ready_state() == EventSource::CLOSED);
            if ended {
                crate::call::<()>("stream_event", (id, "close", Option::<String>::None));
            }
            if name == "close" || ended {
                close(id);
            }
        })
        .into_js_value();
        target
            .add_event_listener_with_callback(name, callback.unchecked_ref())
            .map_err(dom::js)?;
    }
    STREAMS.with(|s| s.borrow_mut().insert(id, stream));
    Ok(id)
}

fn files(id: u32) -> Result<web_sys::FileList> {
    dom::target(id)?
        .dyn_into::<HtmlInputElement>()
        .ok()
        .and_then(|el| el.files())
        .ok_or_else(|| err("expected a file input"))
}

fn clipboard(text: Option<String>) -> Result<u32> {
    let navigator = dom::window().navigator();
    let clip = navigator.clipboard();
    if clip.is_undefined() {
        return Err(err("clipboard requires a secure browser context"));
    }
    // Start during the click handler so the browser sees user activation.
    let promise = match text {
        Some(s) => clip.write_text(&s),
        None => clip.read_text(),
    };
    let id = net::next_op();
    spawn_local(async move {
        match JsFuture::from(promise).await {
            Ok(v) => crate::call::<()>("op_done", (id, v.as_string().unwrap_or_default())),
            Err(e) => crate::call::<()>("op_done", (id, Option::<String>::None, dom::js(e).to_string())),
        };
    });
    Ok(id)
}

fn load_page(name: String) -> u32 {
    let id = net::next_op();
    spawn_local(async move {
        let result = async {
            let path = name.strip_prefix("lazy/").ok_or("expected lazy/ module")?;
            let path = path
                .split('/')
                .map(|p| String::from(js_sys::encode_uri_component(p)))
                .collect::<Vec<_>>()
                .join("/");
            let bytes = net::fetch_bytes(&format!("/_ngs/page/{path}")).await?;
            let bundle = ngs_olezhek_bundle::Bundle::decode(&bytes)?;
            let entry = bundle
                .entries
                .first()
                .filter(|e| e.name == name)
                .ok_or("unexpected page module")?;
            if let Some(error) = entry.error() {
                return Err(error);
            }
            crate::load_chunk(&name, &entry.data).map_err(|e| e.to_string())
        }
        .await;
        match result {
            Ok(f) => crate::call::<()>("op_done", (id, f)),
            Err(e) => crate::call::<()>("op_done", (id, Option::<String>::None, e)),
        };
    });
    id
}

pub fn register(lua: &Lua, n: &Table) -> Result<()> {
    reg(lua, n, "load_page", |_, name: String| Ok(load_page(name)))?;
    reg(lua, n, "resolve_url", |_, url: String| {
        let loc = dom::window().location();
        let u = web_sys::Url::new_with_base(&url, &loc.href().map_err(dom::js)?).map_err(dom::js)?;
        if u.origin() != loc.origin().map_err(dom::js)? {
            return Err(err("route must be same-origin"));
        }
        Ok((u.pathname(), u.search().trim_start_matches('?').to_string(), u.href()))
    })?;
    reg(lua, n, "cookies", |_, ()| {
        dom::document()
            .unchecked_into::<HtmlDocument>()
            .cookie()
            .map_err(dom::js)
    })?;
    reg(lua, n, "set_cookie", |_, s: String| {
        dom::document()
            .unchecked_into::<HtmlDocument>()
            .set_cookie(&s)
            .map_err(dom::js)
    })?;
    reg(lua, n, "clipboard", |_, text: Option<String>| clipboard(text))?;
    reg(lua, n, "meta", |_, (name, value): (String, Option<String>)| {
        if name.is_empty() || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b"-_:.".contains(&b)) {
            return Err(err("invalid meta name"));
        }
        let doc = dom::document();
        let attr = if name.starts_with("og:") { "property" } else { "name" };
        let head = doc.head().ok_or_else(|| err("missing head"))?;
        let existing = head
            .query_selector(&format!("meta[{attr}=\"{name}\"]"))
            .map_err(dom::js)?;
        if let Some(value) = value {
            let el = match existing {
                Some(el) => el,
                None => {
                    let el = doc.create_element("meta").map_err(dom::js)?;
                    el.set_attribute(attr, &name).map_err(dom::js)?;
                    head.append_child(&el).map_err(dom::js)?;
                    el
                }
            };
            el.set_attribute("content", &value).map_err(dom::js)?;
        } else if let Some(el) = existing {
            el.remove();
        }
        Ok(())
    })?;
    let scheme = dom::window()
        .match_media("(prefers-color-scheme: dark)")
        .map_err(dom::js)?
        .ok_or_else(|| err("matchMedia unavailable"))?;
    let media = scheme.clone();
    let cb = Closure::<dyn FnMut(Event)>::new(move |_| {
        crate::call::<()>("scheme_changed", media.matches());
    })
    .into_js_value();
    scheme
        .add_event_listener_with_callback("change", cb.unchecked_ref())
        .map_err(dom::js)?;
    reg(lua, n, "dark", move |_, ()| Ok(scheme.matches()))?;
    // Offset east of UTC at the given moment, so DST changes land on the right dates.
    reg(lua, n, "clock", |_, ()| Ok(js_sys::Date::now() / 1000.0))?;
    reg(lua, n, "tz_offset", |_, unix: f64| {
        Ok(-js_sys::Date::new(&JsValue::from_f64(unix * 1000.0)).get_timezone_offset() * 60.0)
    })?;
    let win: EventTarget = dom::window().into();
    for (name, on) in [("focus", true), ("blur", false)] {
        let cb = Closure::<dyn FnMut(Event)>::new(move |_| {
            crate::call::<()>("window_focus", on);
        })
        .into_js_value();
        win.add_event_listener_with_callback(name, cb.unchecked_ref()).map_err(dom::js)?;
    }
    let cb = Closure::<dyn FnMut(Event)>::new(move |_| {
        crate::call::<()>("visibility_changed", dom::document().visibility_state() == web_sys::VisibilityState::Visible);
    })
    .into_js_value();
    let doc: EventTarget = dom::document().into();
    doc.add_event_listener_with_callback("visibilitychange", cb.unchecked_ref()).map_err(dom::js)?;
    reg(lua, n, "focused", |_, ()| dom::document().has_focus().map_err(dom::js))?;
    reg(lua, n, "visible", |_, ()| Ok(dom::document().visibility_state() == web_sys::VisibilityState::Visible))?;
    reg(lua, n, "connect", |_, (url, events): (String, bool)| {
        connect(&url, events)
    })?;
    reg(lua, n, "disconnect", |_, id: u32| {
        close(id);
        Ok(())
    })?;
    reg(lua, n, "send", |_, (id, text): (u32, String)| {
        STREAMS.with(|s| match s.borrow().get(&id) {
            Some(Stream::Socket(ws)) => ws.send_with_str(&text).map_err(dom::js),
            _ => Err(err("expected an open WebSocket")),
        })
    })?;
    reg(lua, n, "files", |lua, id: u32| {
        let list = files(id)?;
        let out = lua.create_table()?;
        for i in 0..list.length() {
            let file = list.get(i).ok_or_else(|| err("file selection changed"))?;
            let t = lua.create_table()?;
            t.set("name", file.name())?;
            t.set("size", file.size())?;
            t.set("type", file.type_())?;
            out.set(i + 1, t)?;
        }
        Ok(out)
    })?;
    type UploadArgs = Args<(u32, String, String, HashMap<String, String>)>;
    reg(lua, n, "upload", |_, Args((id, url, field, headers)): UploadArgs| {
        let form = FormData::new().map_err(dom::js)?;
        let list = files(id)?;
        if list.length() == 0 {
            return Err(err("no files selected"));
        }
        for i in 0..list.length() {
            let file = list.get(i).ok_or_else(|| err("file selection changed"))?;
            form.append_with_blob_and_filename(&field, &file, &file.name())
                .map_err(dom::js)?;
        }
        Ok(net::fetch("POST".into(), url, Some(JsValue::from(form)), headers))
    })?;
    Ok(())
}
