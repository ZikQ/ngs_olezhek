use std::cell::RefCell;

use wasm_bindgen::{JsCast, prelude::Closure};

use crate::dom;

const URL: &str = "/_ngs/errors";
/// Distinct reports per page load; an error in Think would otherwise send forever.
const MAX_PER_PAGE: usize = 50;
const MAX_MESSAGE: usize = 4000;
const DELAY_MS: i32 = 2000;

#[derive(Default)]
struct Queue {
    enabled: bool,
    items: Vec<(String, u32)>,
    sent: usize,
    timer: bool,
}

thread_local! {
    static QUEUE: RefCell<Queue> = RefCell::default();
}

/// Release builds report Lua errors to the server; dev mode shows them on the page instead.
pub fn enable() {
    QUEUE.with(|q| q.borrow_mut().enabled = true);
    // Whatever is still queued when the tab closes goes out with the beacon.
    let cb = Closure::<dyn FnMut()>::new(flush);
    let _ = dom::window().add_event_listener_with_callback("pagehide", cb.as_ref().unchecked_ref());
    cb.forget();
}

pub fn push(msg: &str) {
    // try_borrow: an error raised while a report is being sent must not panic here.
    let schedule = QUEUE.with(|q| {
        let Ok(mut q) = q.try_borrow_mut() else { return false };
        if !q.enabled {
            return false;
        }
        if let Some(item) = q.items.iter_mut().find(|(m, _)| m == msg) {
            item.1 += 1;
            return false;
        }
        if q.sent + q.items.len() >= MAX_PER_PAGE {
            return false;
        }
        q.items.push((truncate(msg), 1));
        !std::mem::replace(&mut q.timer, true)
    });
    if schedule {
        let cb = Closure::once_into_js(flush);
        let _ = dom::window().set_timeout_with_callback_and_timeout_and_arguments_0(cb.unchecked_ref(), DELAY_MS);
    }
}

/// A Rust panic ends the wasm instance (panic = "abort"), so it is sent at once.
pub fn panic(msg: &str) {
    push(msg);
    flush();
}

fn flush() {
    let items = QUEUE.with(|q| {
        let Ok(mut q) = q.try_borrow_mut() else { return Vec::new() };
        q.timer = false;
        q.sent += q.items.len();
        std::mem::take(&mut q.items)
    });
    if items.is_empty() {
        return;
    }
    let errors: Vec<_> =
        items.into_iter().map(|(message, count)| serde_json::json!({ "message": message, "count": count })).collect();
    // Only the path: query strings carry tokens (password reset links and the like).
    let page = dom::window().location().pathname().unwrap_or_default();
    let body = serde_json::json!({ "page": page, "errors": errors }).to_string();
    // A beacon survives the page unloading and needs no answer.
    let _ = dom::window().navigator().send_beacon_with_opt_str(URL, Some(&body));
}

fn truncate(msg: &str) -> String {
    if msg.len() <= MAX_MESSAGE {
        return msg.to_string();
    }
    let mut end = MAX_MESSAGE;
    while !msg.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}...", &msg[..end])
}
