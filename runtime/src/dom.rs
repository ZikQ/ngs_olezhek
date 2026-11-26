use std::{cell::RefCell, collections::HashMap};

use js_sys::Reflect;
use omnilua::{Lua, Result, Value};
use wasm_bindgen::{JsCast, JsValue, prelude::Closure};
use web_sys::{
    CanvasRenderingContext2d, CssStyleRule, CssStyleSheet, Document, Element, EventTarget, HtmlCanvasElement,
    HtmlElement, HtmlLinkElement, ResizeObserver, ResizeObserverEntry, Text, Window,
};

use crate::err;

const ID_ATTR: &str = "data-ngs";

pub struct Entry {
    pub el: HtmlElement,
    /// The native input inside a wrapper (`<input>` in TextEntry, `<select>` in ComboBox...).
    pub control: Option<HtmlElement>,
    pub text: Option<Text>,
    pub canvas: Option<Canvas>,
}

impl Entry {
    fn target(&self) -> &HtmlElement {
        self.control.as_ref().unwrap_or(&self.el)
    }
}

pub struct Canvas {
    pub el: HtmlCanvasElement,
    pub ctx: CanvasRenderingContext2d,
    size: (u32, u32),
}

#[derive(Default)]
struct Dom {
    entries: HashMap<u32, Entry>,
    next: u32,
    dev: bool,
    observer: Option<ResizeObserver>,
    errors: Option<HtmlElement>,
    error_rows: Vec<(String, HtmlElement, u32)>,
    rules: Option<CssStyleSheet>,
}

thread_local! {
    static DOM: RefCell<Dom> = RefCell::new(Dom::default());
}

pub fn window() -> Window {
    web_sys::window().unwrap()
}

pub fn document() -> Document {
    window().document().unwrap()
}

pub fn js(e: JsValue) -> omnilua::Error {
    err(e.as_string().unwrap_or_else(|| format!("{e:?}")))
}

fn with<R>(id: u32, f: impl FnOnce(&mut Entry) -> R) -> Result<R> {
    DOM.with(|d| d.borrow_mut().entries.get_mut(&id).map(f))
        .ok_or_else(|| err(format!("panel {id} has been removed")))
}

fn element(id: u32) -> Result<HtmlElement> {
    with(id, |e| e.el.clone())
}

pub(crate) fn target(id: u32) -> Result<HtmlElement> {
    with(id, |e| e.target().clone())
}

pub fn set_dev(dev: bool) {
    DOM.with(|d| d.borrow_mut().dev = dev);
}

pub fn panel_id(target: &EventTarget) -> Option<u32> {
    let el = target.dyn_ref::<Element>()?;
    el.closest(&format!("[{ID_ATTR}]")).ok()??.get_attribute(ID_ATTR)?.parse().ok()
}

pub fn create(tag: &str, class: &str, control: Option<&str>, control_type: Option<&str>) -> Result<u32> {
    let doc = document();
    let el: HtmlElement = doc.create_element(tag).map_err(js)?.unchecked_into();
    el.set_class_name(&format!("ngs {class}"));
    if tag == "button" {
        el.set_attribute("type", "button").map_err(js)?;
    }
    let control = match control {
        Some(t) => {
            let c = make_control(&doc, t, control_type)?;
            el.append_child(&c).map_err(js)?;
            Some(c)
        }
        None => None,
    };
    let id = DOM.with(|d| {
        let mut d = d.borrow_mut();
        d.next += 1;
        let id = d.next;
        d.entries.insert(id, Entry { el: el.clone(), control, text: None, canvas: None });
        id
    });
    el.set_attribute(ID_ATTR, &id.to_string()).map_err(js)?;
    Ok(id)
}

fn make_control(doc: &Document, tag: &str, ty: Option<&str>) -> Result<HtmlElement> {
    let c: HtmlElement = doc.create_element(tag).map_err(js)?.unchecked_into();
    c.set_class_name("ngs-control");
    if let Some(ty) = ty {
        c.set_attribute("type", ty).map_err(js)?;
    }
    Ok(c)
}

/// Swaps the inner control (e.g. `<input>` to `<textarea>` for multiline), keeping its value.
pub fn set_control(id: u32, tag: &str, ty: Option<&str>) -> Result<()> {
    let old = with(id, |e| e.control.clone())?.ok_or_else(|| err("panel has no native control"))?;
    let new = make_control(&document(), tag, ty)?;
    let value = Reflect::get(&old, &"value".into()).map_err(js)?;
    old.replace_with_with_node_1(&new).map_err(js)?;
    Reflect::set(&new, &"value".into(), &value).map_err(js)?;
    with(id, |e| e.control = Some(new))
}

pub fn mount(id: u32) -> Result<()> {
    let el = element(id)?;
    document().body().unwrap().append_child(&el).map_err(js)?;
    Ok(())
}

pub fn append(parent: u32, child: u32) -> Result<()> {
    let child = element(child)?;
    element(parent)?.append_child(&child).map_err(js)?;
    Ok(())
}

pub fn remove(id: u32) -> Result<()> {
    let (entry, observer) = DOM.with(|d| {
        let mut d = d.borrow_mut();
        (d.entries.remove(&id), d.observer.clone())
    });
    if let Some(entry) = entry {
        if let Some(o) = observer {
            o.unobserve(&entry.el);
        }
        entry.el.remove();
    }
    crate::events::forget(id);
    Ok(())
}

pub fn style(id: u32, prop: &str, value: Option<&str>) -> Result<()> {
    let style = element(id)?.style();
    match value {
        Some(v) if !v.is_empty() => style.set_property(prop, v),
        _ => style.remove_property(prop).map(drop),
    }
    .map_err(js)
}

pub fn attr(id: u32, name: &str, value: Option<&str>, on_control: bool) -> Result<()> {
    let el = if on_control { target(id)? } else { element(id)? };
    match value {
        Some(v) => el.set_attribute(name, v),
        None => el.remove_attribute(name),
    }
    .map_err(js)
}

pub fn get_attr(id: u32, name: &str, on_control: bool) -> Result<Option<String>> {
    let el = if on_control { target(id)? } else { element(id)? };
    Ok(el.get_attribute(name))
}

pub fn prop(id: u32, name: &str, value: Value) -> Result<()> {
    let v = match value {
        Value::Nil => JsValue::UNDEFINED,
        Value::Boolean(b) => b.into(),
        Value::Integer(i) => (i as f64).into(),
        Value::Number(n) => n.into(),
        Value::String(s) => String::from_utf8_lossy(&s.as_bytes()?).as_ref().into(),
        _ => return Err(err("property value must be nil, boolean, number or string")),
    };
    let t = target(id)?;
    Reflect::set(&t, &name.into(), &v).map_err(js)?;
    Ok(())
}

pub fn get_prop(lua: &Lua, id: u32, name: &str) -> Result<Value> {
    let t = target(id)?;
    let v = Reflect::get(&t, &name.into()).map_err(js)?;
    Ok(if let Some(b) = v.as_bool() {
        Value::Boolean(b)
    } else if let Some(n) = v.as_f64() {
        Value::Number(n)
    } else if let Some(s) = v.as_string() {
        Value::String(lua.create_string(s)?)
    } else {
        Value::Nil
    })
}

pub fn class(id: u32, name: &str, on: bool) -> Result<()> {
    element(id)?.class_list().toggle_with_force(name, on).map_err(js)?;
    Ok(())
}

/// The panel's own text lives in one text node placed before child panels,
/// after the paint canvas and the native control.
pub fn text(id: u32, s: &str) -> Result<()> {
    let (el, anchor, node) = with(id, |e| {
        let anchor = e.control.as_ref().map(|c| c.clone().unchecked_into::<web_sys::Node>())
            .or_else(|| e.canvas.as_ref().map(|c| c.el.clone().unchecked_into()));
        (e.el.clone(), anchor, e.text.clone())
    })?;
    if let Some(node) = node {
        node.set_data(s);
        return Ok(());
    }
    let node = document().create_text_node(s);
    let before = match anchor {
        Some(a) => a.next_sibling(),
        None => el.first_child(),
    };
    el.insert_before(&node, before.as_ref()).map_err(js)?;
    with(id, |e| e.text = Some(node))
}

pub fn size(id: u32) -> Result<(i32, i32)> {
    let el = element(id)?;
    Ok((el.offset_width(), el.offset_height()))
}

pub fn offset(id: u32) -> Result<(i32, i32)> {
    let el = element(id)?;
    Ok((el.offset_left(), el.offset_top()))
}

/// Viewport rect, the same coordinate space as the cursor position.
pub fn rect(id: u32) -> Result<(f64, f64, f64, f64)> {
    let r = element(id)?.get_bounding_client_rect();
    Ok((r.x(), r.y(), r.width(), r.height()))
}

/// Calls a no-argument DOM method on the panel element (or its native control): showModal, close, play...
pub fn invoke(id: u32, method: &str) -> Result<()> {
    let el = target(id)?;
    let f: js_sys::Function = Reflect::get(&el, &method.into())
        .map_err(js)?
        .dyn_into()
        .map_err(|_| err(format!("<{}> has no method {method}", el.tag_name().to_lowercase())))?;
    f.call0(&el).map_err(js)?;
    Ok(())
}

/// Best effort: capture fails harmlessly when no pointer is down.
pub fn capture(id: u32, on: bool) -> Result<()> {
    let el = element(id)?;
    let pointer = crate::events::pointer_id();
    let _ = if on { el.set_pointer_capture(pointer) } else { el.release_pointer_capture(pointer) };
    Ok(())
}

pub fn clear_errors() {
    DOM.with(|d| {
        for (_, row, _) in d.borrow_mut().error_rows.drain(..) {
            row.remove();
        }
    });
}

pub fn focus(id: u32) -> Result<()> {
    target(id)?.focus().map_err(js)
}

pub fn blur(id: u32) -> Result<()> {
    target(id)?.blur().map_err(js)
}

pub fn has_focus(id: u32) -> Result<bool> {
    let t = target(id)?;
    let t: &Element = t.as_ref();
    Ok(document().active_element().is_some_and(|a| &a == t))
}

pub fn add_option(id: u32, text: &str) -> Result<()> {
    let opt = document().create_element("option").map_err(js)?;
    opt.set_text_content(Some(text));
    target(id)?.append_child(&opt).map_err(js)?;
    Ok(())
}

pub fn clear_options(id: u32) -> Result<()> {
    target(id)?.set_inner_html("");
    Ok(())
}

pub fn scroll_into_view(id: u32) -> Result<()> {
    element(id)?.scroll_into_view();
    Ok(())
}

pub fn css_var(name: &str, value: &str) -> Result<()> {
    let root: HtmlElement = document().document_element().unwrap().unchecked_into();
    root.style().set_property(name, value).map_err(js)
}

/// Adds `selector { prop: value !important; ... }` to the runtime's own stylesheet. Going through
/// CSSOM instead of stylesheet text means a value can never close the rule and start another.
pub fn css_rule(selector: &str, decls: &[String]) -> Result<()> {
    let sheet = rules_sheet()?;
    let rules = sheet.css_rules().map_err(js)?;
    let index = sheet.insert_rule_with_index(&format!("{selector} {{}}"), rules.length()).map_err(js)?;
    let rule: CssStyleRule = rules
        .get(index)
        .ok_or_else(|| err("css rule vanished"))?
        .dyn_into()
        .map_err(|_| err("not a style rule"))?;
    let style = rule.style();
    for pair in decls.chunks(2) {
        if let [prop, value] = pair {
            style.set_property_with_priority(prop, value, "important").map_err(js)?;
        }
    }
    Ok(())
}

fn rules_sheet() -> Result<CssStyleSheet> {
    if let Some(s) = DOM.with(|d| d.borrow().rules.clone()) {
        return Ok(s);
    }
    let doc = document();
    let el: HtmlLinkElement = doc.get_element_by_id("ngs-style")
        .ok_or_else(|| err("missing runtime stylesheet"))?.unchecked_into();
    let sheet: CssStyleSheet = el.sheet().ok_or_else(|| err("no stylesheet"))?.unchecked_into();
    DOM.with(|d| d.borrow_mut().rules = Some(sheet.clone()));
    Ok(sheet)
}

pub fn observe(id: u32, on: bool) -> Result<()> {
    let el = element(id)?;
    let observer = DOM.with(|d| d.borrow().observer.clone());
    let observer = match observer {
        Some(o) => o,
        None => {
            let o = new_observer()?;
            DOM.with(|d| d.borrow_mut().observer = Some(o.clone()));
            o
        }
    };
    if on { observer.observe(&el) } else { observer.unobserve(&el) }
    Ok(())
}

fn new_observer() -> Result<ResizeObserver> {
    let cb = Closure::<dyn FnMut(js_sys::Array)>::new(|entries: js_sys::Array| {
        let sizes: Vec<(u32, i32, i32)> = entries
            .iter()
            .filter_map(|e| {
                let target = e.unchecked_into::<ResizeObserverEntry>().target();
                let id = panel_id(&target)?;
                let el: HtmlElement = target.unchecked_into();
                Some((id, el.offset_width(), el.offset_height()))
            })
            .collect();
        for (id, w, h) in sizes {
            crate::call::<()>("resized", (id, w, h));
        }
    });
    let o = ResizeObserver::new(cb.as_ref().unchecked_ref()).map_err(js)?;
    cb.forget();
    Ok(o)
}

/// Returns the panel's paint canvas sized for `w`x`h` CSS pixels, creating it on first use.
pub fn canvas(id: u32, w: f64, h: f64) -> Result<CanvasRenderingContext2d> {
    let dpr = window().device_pixel_ratio();
    let size = (((w * dpr).ceil().max(1.0)) as u32, ((h * dpr).ceil().max(1.0)) as u32);
    let existing = with(id, |e| e.canvas.as_ref().map(|c| (c.el.clone(), c.ctx.clone(), c.size)))?;
    let (el, ctx) = match existing {
        Some((el, ctx, old)) => {
            if old != size {
                el.set_width(size.0);
                el.set_height(size.1);
                with(id, |e| e.canvas.as_mut().unwrap().size = size)?;
            }
            (el, ctx)
        }
        None => {
            let el: HtmlCanvasElement = document().create_element("canvas").map_err(js)?.unchecked_into();
            el.set_class_name("ngs-paint");
            el.set_width(size.0);
            el.set_height(size.1);
            let ctx: CanvasRenderingContext2d =
                el.get_context("2d").map_err(js)?.ok_or_else(|| err("no 2d context"))?.unchecked_into();
            let host = element(id)?;
            host.insert_before(&el, host.first_child().as_ref()).map_err(js)?;
            with(id, |e| e.canvas = Some(Canvas { el: el.clone(), ctx: ctx.clone(), size }))?;
            (el, ctx)
        }
    };
    drop(el);
    ctx.set_transform(1.0, 0.0, 0.0, 1.0, 0.0, 0.0).map_err(js)?;
    ctx.clear_rect(0.0, 0.0, size.0 as f64, size.1 as f64);
    ctx.set_transform(dpr, 0.0, 0.0, dpr, 0.0, 0.0).map_err(js)?;
    ctx.set_global_alpha(1.0);
    Ok(ctx)
}

/// Console always; in dev also an on-screen list, GMod-style, with repeats collapsed into a counter;
/// in release a report to the server unless the app turned that off.
pub fn report_error(msg: &str) {
    web_sys::console::error_1(&msg.into());
    crate::report::push(msg);
    let Some(list) = DOM.with(|d| {
        let mut d = d.borrow_mut();
        if !d.dev {
            return None;
        }
        if d.errors.is_none() {
            d.errors = Some(new_error_list());
        }
        d.errors.clone()
    }) else {
        return;
    };

    let doc = document();
    DOM.with(|d| {
        let mut d = d.borrow_mut();
        if let Some(row) = d.error_rows.iter_mut().find(|(m, _, _)| m == msg) {
            row.2 += 1;
            let _ = row.1.set_attribute("data-count", &format!("x{}", row.2));
            return;
        }
        let row: HtmlElement = doc.create_element("div").unwrap().unchecked_into();
        row.set_class_name("ngs-error");
        row.set_text_content(Some(msg));
        let _ = list.prepend_with_node_1(&row);
        d.error_rows.push((msg.to_string(), row, 1));
        while d.error_rows.len() > 8 {
            let (_, old, _) = d.error_rows.remove(0);
            old.remove();
        }
    });
}

fn new_error_list() -> HtmlElement {
    let list: HtmlElement = document().create_element("div").unwrap().unchecked_into();
    list.set_id("ngs-errors");
    let cb = Closure::<dyn FnMut(web_sys::Event)>::new(|e: web_sys::Event| {
        let Some(row) = e.target().and_then(|t| t.dyn_into::<Element>().ok()).and_then(|t| t.closest(".ngs-error").ok().flatten())
        else {
            return;
        };
        row.remove();
        DOM.with(|d| d.borrow_mut().error_rows.retain(|(_, r, _)| AsRef::<Element>::as_ref(r) != &row));
    });
    list.add_event_listener_with_callback("click", cb.as_ref().unchecked_ref()).unwrap();
    cb.forget();
    document().body().unwrap().append_child(&list).unwrap();
    list
}
