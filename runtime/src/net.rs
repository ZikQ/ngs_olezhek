use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
};

use omnilua::{Function, Value};
use wasm_bindgen::{JsCast, JsValue, prelude::Closure};
use wasm_bindgen_futures::JsFuture;
use web_sys::{AbortController, AbortSignal, Headers, Request, RequestInit, Response};

use crate::{args::Args, dom};

type FrameCallback = Closure<dyn FnMut(f64)>;

thread_local! {
    static NEXT_OP: Cell<u32> = const { Cell::new(0) };
    static THINK: RefCell<Option<FrameCallback>> = const { RefCell::new(None) };
    static THINKING: Cell<bool> = const { Cell::new(false) };
    static ABORTS: RefCell<HashMap<u32, AbortController>> = RefCell::new(HashMap::new());
}

struct Reply {
    status: u32,
    text: String,
    content_type: String,
    /// The server's SPA fallback page rather than a real endpoint.
    shell: bool,
}

fn js_string(e: JsValue) -> String {
    e.as_string().unwrap_or_else(|| format!("{e:?}"))
}

pub async fn fetch_bytes(url: &str) -> Result<Vec<u8>, String> {
    let resp: Response = JsFuture::from(dom::window().fetch_with_str(url)).await.map_err(js_string)?.unchecked_into();
    if !resp.ok() {
        return Err(format!("GET {url}: HTTP {}", resp.status()));
    }
    let buf = JsFuture::from(resp.array_buffer().map_err(js_string)?).await.map_err(js_string)?;
    Ok(js_sys::Uint8Array::new(&buf).to_vec())
}

/// Starts a request and returns an op id; the result arrives later in `R.op_done`.
/// The id is returned before the request can complete, so Lua always registers the waiter first.
pub fn fetch(method: String, url: String, body: Option<String>, headers: HashMap<String, String>) -> u32 {
    let op = NEXT_OP.with(|n| {
        n.set(n.get() + 1);
        n.get()
    });
    let signal = AbortController::new().ok().map(|c| {
        let signal = c.signal();
        ABORTS.with(|a| a.borrow_mut().insert(op, c));
        signal
    });
    wasm_bindgen_futures::spawn_local(async move {
        let res = request(&method, &url, body, &headers, signal.as_ref()).await;
        ABORTS.with(|a| a.borrow_mut().remove(&op));
        match res {
            Ok(r) => crate::call::<()>("op_done", Args((op, r.status, r.text, r.content_type, Value::Nil, r.shell))),
            Err(e) => crate::call::<()>("op_done", Args((op, 0u32, "", "", e, false))),
        };
    });
    op
}

/// Lua has already resolved the request as cancelled/timed out; the fetch then fails with
/// AbortError and that late result is ignored.
pub fn abort(op: u32) {
    if let Some(c) = ABORTS.with(|a| a.borrow_mut().remove(&op)) {
        c.abort();
    }
}

async fn request(
    method: &str,
    url: &str,
    body: Option<String>,
    headers: &HashMap<String, String>,
    signal: Option<&AbortSignal>,
) -> Result<Reply, String> {
    let init = RequestInit::new();
    init.set_method(method);
    let h = Headers::new().map_err(js_string)?;
    h.set("Accept", "application/json, text/plain, */*").map_err(js_string)?;
    for (name, value) in headers {
        h.set(name, value).map_err(|e| format!("header {name}: {}", js_string(e)))?;
    }
    init.set_headers(&h);
    if let Some(b) = body {
        init.set_body(&JsValue::from_str(&b));
    }
    if let Some(s) = signal {
        init.set_signal(Some(s));
    }
    let req = Request::new_with_str_and_init(url, &init).map_err(js_string)?;
    let resp: Response =
        JsFuture::from(dom::window().fetch_with_request(&req)).await.map_err(js_string)?.unchecked_into();
    let content_type = resp.headers().get("content-type").ok().flatten().unwrap_or_default();
    let shell = resp.headers().has("x-ngs-shell").unwrap_or(false);
    let text = JsFuture::from(resp.text().map_err(js_string)?).await.map_err(js_string)?.as_string().unwrap_or_default();
    Ok(Reply { status: resp.status() as u32, text, content_type, shell })
}

pub fn timeout(ms: f64, f: Function) -> i32 {
    let cb = Closure::once_into_js(move || {
        if let Err(e) = f.call::<_, ()>(()) {
            dom::report_error(&e.to_string());
        }
    });
    dom::window()
        .set_timeout_with_callback_and_timeout_and_arguments_0(cb.unchecked_ref(), ms.max(0.0) as i32)
        .unwrap_or(0)
}

pub fn clear_timeout(handle: i32) {
    dom::window().clear_timeout_with_handle(handle);
}

/// Batches Dock/InvalidateLayout/InvalidatePaint into one pass before the browser renders.
pub fn schedule_flush() {
    let cb = Closure::once_into_js(|| {
        crate::call::<()>("flush", ());
    });
    dom::window().queue_microtask(cb.unchecked_ref());
}

/// Runs `R.think` every animation frame for as long as it returns true.
pub fn think(on: bool) {
    if on && !THINKING.with(|t| t.replace(true)) {
        request_frame();
    }
}

fn request_frame() {
    THINK.with(|t| {
        let mut t = t.borrow_mut();
        let cb = t.get_or_insert_with(|| {
            Closure::new(|now: f64| {
                if crate::call::<bool>("think", now / 1000.0).unwrap_or(false) {
                    request_frame();
                } else {
                    THINKING.with(|t| t.set(false));
                }
            })
        });
        let _ = dom::window().request_animation_frame(cb.as_ref().unchecked_ref());
    });
}
