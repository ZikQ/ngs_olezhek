use std::cell::Cell;

use js_sys::Reflect;
use omnilua::{IntoLua, Value};
use wasm_bindgen::{JsCast, prelude::Closure};
use web_sys::{Event, EventTarget, KeyboardEvent, MouseEvent, PointerEvent, StorageEvent};

use crate::dom;

thread_local! {
    static HOVERED: Cell<Option<u32>> = const { Cell::new(None) };
    static PRESSED: Cell<Option<u32>> = const { Cell::new(None) };
    static MOUSE: Cell<(f64, f64)> = const { Cell::new((0.0, 0.0)) };
    static POINTER: Cell<i32> = const { Cell::new(1) };
}

/// Last known cursor position in viewport coordinates (GMod's gui.MousePos).
pub fn mouse() -> (f64, f64) {
    MOUSE.with(|m| m.get())
}

/// Pointer of the last press, needed for setPointerCapture.
pub fn pointer_id() -> i32 {
    POINTER.with(|p| p.get())
}

fn track_mouse(e: &Event) {
    let me: &MouseEvent = e.unchecked_ref();
    MOUSE.with(|m| m.set((me.client_x() as f64, me.client_y() as f64)));
}

pub const MOUSE_LEFT: i64 = 1;
pub const MOUSE_RIGHT: i64 = 2;
pub const MOUSE_MIDDLE: i64 = 3;

fn mouse_code(button: i16) -> i64 {
    match button {
        1 => MOUSE_MIDDLE,
        2 => MOUSE_RIGHT,
        _ => MOUSE_LEFT,
    }
}

/// Returns true when Lua asked to cancel the browser default.
fn dispatch(id: u32, name: &str, arg: impl IntoLua) -> bool {
    crate::call::<bool>("event", (id, name, arg)).unwrap_or(false)
}

fn target_id(e: &Event) -> Option<u32> {
    dom::panel_id(&e.target()?)
}

fn listen(target: &EventTarget, name: &str, f: impl FnMut(Event) + 'static) {
    let cb = Closure::<dyn FnMut(Event)>::new(f);
    target.add_event_listener_with_callback(name, cb.as_ref().unchecked_ref()).unwrap();
    cb.forget();
}

/// For events that don't bubble (dialog `cancel`, popover `toggle`): catch them on the way down.
fn listen_capture(target: &EventTarget, name: &str, f: impl FnMut(Event) + 'static) {
    let cb = Closure::<dyn FnMut(Event)>::new(f);
    target.add_event_listener_with_callback_and_bool(name, cb.as_ref().unchecked_ref(), true).unwrap();
    cb.forget();
}

/// Hover follows GMod: only the topmost panel under the cursor is hovered. Labels and images
/// have `pointer-events: none` by default, so hovering one hovers the panel underneath.
fn set_hovered(new: Option<u32>) {
    let old = HOVERED.with(|h| h.replace(new));
    if old == new {
        return;
    }
    if let Some(o) = old {
        dispatch(o, "exit", Value::Nil);
    }
    if let Some(n) = new {
        dispatch(n, "enter", Value::Nil);
    }
}

pub fn forget(id: u32) {
    HOVERED.with(|h| if h.get() == Some(id) { h.set(None) });
    PRESSED.with(|p| if p.get() == Some(id) { p.set(None) });
}

pub fn install() {
    let doc: EventTarget = dom::document().into();

    listen(&doc, "click", |e| {
        let me: &MouseEvent = e.unchecked_ref();
        if me.button() != 0 {
            return;
        }
        if let Some(id) = target_id(&e) {
            let plain = !(me.ctrl_key() || me.meta_key() || me.shift_key() || me.alt_key());
            if dispatch(id, "click", plain) {
                e.prevent_default();
            }
        }
    });
    for (dom_name, name) in [("contextmenu", "rclick"), ("dblclick", "dblclick")] {
        listen(&doc, dom_name, move |e| {
            if let Some(id) = target_id(&e)
                && dispatch(id, name, Value::Nil)
            {
                e.prevent_default();
            }
        });
    }
    listen(&doc, "pointerdown", |e| {
        track_mouse(&e);
        POINTER.with(|p| p.set(e.unchecked_ref::<PointerEvent>().pointer_id()));
        let code = mouse_code(e.unchecked_ref::<MouseEvent>().button());
        if let Some(id) = target_id(&e) {
            PRESSED.with(|p| p.set(Some(id)));
            dispatch(id, "down", code);
        }
    });
    // Released goes to the panel that got the press, even if the cursor left it, like GMod's mouse capture.
    listen(&doc, "pointerup", |e| {
        let code = mouse_code(e.unchecked_ref::<MouseEvent>().button());
        if let Some(id) = PRESSED.with(|p| p.take()) {
            dispatch(id, "up", code);
        }
    });
    // With MouseCapture the target stays the capturing panel, which is what dragging needs.
    listen(&doc, "pointermove", |e| {
        track_mouse(&e);
        if let Some(id) = target_id(&e) {
            dispatch(id, "move", Value::Nil);
        }
    });
    listen(&doc, "pointerover", |e| set_hovered(target_id(&e)));
    listen_capture(&doc, "scroll", |e| {
        if let Some(id) = target_id(&e) { dispatch(id, "scroll", Value::Nil); }
    });
    listen_capture(&doc, "cancel", |e| {
        if let Some(id) = target_id(&e)
            && dispatch(id, "cancel", Value::Nil)
        {
            e.prevent_default();
        }
    });
    listen_capture(&doc, "toggle", |e| {
        let open = Reflect::get(&e, &"newState".into()).ok().and_then(|v| v.as_string()).is_some_and(|s| s == "open");
        if let Some(id) = target_id(&e) {
            dispatch(id, "toggle", open);
        }
    });
    listen(&doc, "pointerout", |e| {
        if e.unchecked_ref::<MouseEvent>().related_target().is_none() {
            set_hovered(None);
        }
    });
    for (dom_name, name) in [("focusin", "focus"), ("focusout", "blur"), ("input", "input"), ("change", "change")] {
        listen(&doc, dom_name, move |e| {
            if let Some(id) = target_id(&e) {
                dispatch(id, name, Value::Nil);
            }
        });
    }
    listen(&doc, "keydown", |e| {
        let key = e.unchecked_ref::<KeyboardEvent>().key();
        if let Some(id) = target_id(&e)
            && dispatch(id, "key", key)
        {
            e.prevent_default();
        }
    });

    let win: EventTarget = dom::window().into();
    listen(&win, "popstate", |_| {
        let (path, query) = location();
        crate::call::<()>("popstate", (path, query));
    });
    listen(&win, "storage", |e| {
        let key = e.unchecked_ref::<StorageEvent>().key();
        crate::call::<()>("storage_changed", key);
    });
    listen(&win, "resize", |_| {
        let w = dom::window();
        let size = |v: Result<wasm_bindgen::JsValue, _>| v.ok().and_then(|v| v.as_f64()).unwrap_or(0.0);
        crate::call::<()>("screen", (size(w.inner_width()), size(w.inner_height())));
    });
}

const HISTORY_INDEX: &str = "ngsIndex";

pub fn history_index() -> omnilua::Result<Option<i32>> {
    let state = dom::window().history().map_err(dom::js)?.state().map_err(dom::js)?;
    Ok(Reflect::get(&state, &HISTORY_INDEX.into()).ok().and_then(|v| v.as_f64()).map(|v| v as i32))
}

pub fn set_history(url: Option<&str>, push: bool) -> omnilua::Result<()> {
    let history = dom::window().history().map_err(dom::js)?;
    let index = history_index()?.unwrap_or(0) + i32::from(push);
    let state = js_sys::Object::new();
    if !push {
        let previous = history.state().map_err(dom::js)?;
        if previous.is_object() {
            js_sys::Object::assign(&state, previous.unchecked_ref());
        }
    }
    Reflect::set(&state, &HISTORY_INDEX.into(), &index.into()).map_err(dom::js)?;
    if push { history.push_state_with_url(&state, "", url) }
    else { history.replace_state_with_url(&state, "", url) }.map_err(dom::js)
}

pub fn location() -> (String, String) {
    let loc = dom::window().location();
    let path = loc.pathname().unwrap_or_else(|_| "/".into());
    let query = loc.search().unwrap_or_default().trim_start_matches('?').to_string();
    (path, query)
}
