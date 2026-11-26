use std::cell::Cell;

use ngs_olezhek_bundle::{Bundle, Kind};
use wasm_bindgen::{JsCast, prelude::Closure};
use web_sys::{BinaryType, MessageEvent, WebSocket};

use crate::dom;

thread_local! {
    static WAS_CONNECTED: Cell<bool> = const { Cell::new(false) };
}

/// Dev-only link to the server. Changed Lua files arrive as one-entry bundles and are re-run
/// in place. If the socket drops and comes back, the server was restarted (likely rebuilt),
/// so the page reloads to pick up new Rust code and runtime.
pub fn connect() {
    let loc = dom::window().location();
    let scheme = if loc.protocol().as_deref() == Ok("https:") { "wss" } else { "ws" };
    let url = format!("{scheme}://{}/_ngs/live", loc.host().unwrap_or_default());
    let Ok(ws) = WebSocket::new(&url) else { return retry() };
    ws.set_binary_type(BinaryType::Arraybuffer);

    let on_open = Closure::<dyn FnMut()>::new(|| {
        if WAS_CONNECTED.with(|c| c.replace(true)) {
            let _ = dom::window().location().reload();
        }
    });
    let on_message = Closure::<dyn FnMut(MessageEvent)>::new(|e: MessageEvent| {
        if let Ok(buf) = e.data().dyn_into::<js_sys::ArrayBuffer>() {
            apply(&js_sys::Uint8Array::new(&buf).to_vec());
        }
    });
    let on_close = Closure::<dyn FnMut()>::new(retry);
    ws.set_onopen(Some(on_open.as_ref().unchecked_ref()));
    ws.set_onmessage(Some(on_message.as_ref().unchecked_ref()));
    ws.set_onclose(Some(on_close.as_ref().unchecked_ref()));
    on_open.forget();
    on_message.forget();
    on_close.forget();
}

fn retry() {
    let cb = Closure::once_into_js(connect);
    let _ = dom::window().set_timeout_with_callback_and_timeout_and_arguments_0(cb.unchecked_ref(), 1000);
}

fn apply(bytes: &[u8]) {
    let bundle = match Bundle::decode(bytes) {
        Ok(b) => b,
        Err(e) => return dom::report_error(&format!("live reload: {e}")),
    };
    for entry in bundle.entries {
        match entry.kind {
            Kind::Chunk => match crate::load_chunk(&entry.name, &entry.data) {
                Ok(f) => {
                    dom::clear_errors();
                    crate::call::<()>("reload", (entry.name, f));
                }
                Err(e) => dom::report_error(&e.to_string()),
            },
            Kind::Deferred => (),
            Kind::Error => dom::report_error(&String::from_utf8_lossy(&entry.data)),
        }
    }
}
