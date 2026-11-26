use std::{cell::RefCell, collections::HashMap, f64::consts::TAU};

use omnilua::{Result, Table};
use wasm_bindgen::{JsCast, JsValue, prelude::Closure};
use wasm_bindgen_futures::JsFuture;
use web_sys::{CanvasRenderingContext2d, FontFace, FontFaceDescriptors, HtmlCanvasElement, HtmlImageElement};

use crate::{dom, dom::js, err};

const WHITE: &str = "rgba(255,255,255,1)";

struct State {
    panel: u32,
    ctx: CanvasRenderingContext2d,
    color: String,
    text_color: String,
    font: String,
}

#[derive(Default)]
struct Images {
    list: Vec<HtmlImageElement>,
    /// Panels that tried to draw an image before it finished loading.
    waiting: HashMap<u32, Vec<u32>>,
}

thread_local! {
    static PAINT: RefCell<Option<State>> = const { RefCell::new(None) };
    static MEASURE: RefCell<Option<CanvasRenderingContext2d>> = const { RefCell::new(None) };
    static METRICS: RefCell<HashMap<String, (f64, f64)>> = RefCell::new(HashMap::new());
    static IMAGES: RefCell<Images> = RefCell::new(Images::default());
}

fn with<R>(f: impl FnOnce(&mut State) -> R) -> Result<R> {
    PAINT
        .with(|p| p.borrow_mut().as_mut().map(f))
        .ok_or_else(|| err("surface/draw functions can only be called inside Paint"))
}

pub fn rgba(r: f64, g: f64, b: f64, a: f64) -> String {
    format!("rgba({},{},{},{})", r.clamp(0.0, 255.0) as u8, g.clamp(0.0, 255.0) as u8, b.clamp(0.0, 255.0) as u8, (a / 255.0).clamp(0.0, 1.0))
}

pub fn begin(panel: u32, w: f64, h: f64, font: String) -> Result<()> {
    let ctx = dom::canvas(panel, w, h)?;
    ctx.set_fill_style_str(WHITE);
    ctx.set_stroke_style_str(WHITE);
    ctx.set_font(&font);
    PAINT.with(|p| *p.borrow_mut() = Some(State { panel, ctx, color: WHITE.into(), text_color: WHITE.into(), font }));
    Ok(())
}

pub fn end() {
    PAINT.with(|p| *p.borrow_mut() = None);
}

pub fn color(r: f64, g: f64, b: f64, a: f64) -> Result<()> {
    with(|s| {
        s.color = rgba(r, g, b, a);
        s.ctx.set_fill_style_str(&s.color);
        s.ctx.set_stroke_style_str(&s.color);
    })
}

pub fn text_color(r: f64, g: f64, b: f64, a: f64) -> Result<()> {
    with(|s| s.text_color = rgba(r, g, b, a))
}

pub fn font(css: String) -> Result<()> {
    with(|s| {
        s.ctx.set_font(&css);
        s.font = css;
    })
}

pub fn alpha(a: f64) -> Result<()> {
    with(|s| s.ctx.set_global_alpha(a.clamp(0.0, 1.0)))
}

pub fn rect(x: f64, y: f64, w: f64, h: f64) -> Result<()> {
    with(|s| s.ctx.fill_rect(x, y, w, h))
}

/// Border drawn inside the rect, like GMod's DrawOutlinedRect.
pub fn outlined(x: f64, y: f64, w: f64, h: f64, t: f64) -> Result<()> {
    with(|s| {
        s.ctx.fill_rect(x, y, w, t);
        s.ctx.fill_rect(x, y + h - t, w, t);
        s.ctx.fill_rect(x, y + t, t, h - 2.0 * t);
        s.ctx.fill_rect(x + w - t, y + t, t, h - 2.0 * t);
    })
}

pub fn line(x1: f64, y1: f64, x2: f64, y2: f64) -> Result<()> {
    with(|s| {
        s.ctx.begin_path();
        s.ctx.set_line_width(1.0);
        s.ctx.move_to(x1 + 0.5, y1 + 0.5);
        s.ctx.line_to(x2 + 0.5, y2 + 0.5);
        s.ctx.stroke();
    })
}

#[allow(clippy::too_many_arguments)]
pub fn rounded(r: f64, x: f64, y: f64, w: f64, h: f64, tl: bool, tr: bool, bl: bool, br: bool) -> Result<()> {
    let r = r.min(w / 2.0).min(h / 2.0).max(0.0);
    let pick = |on: bool| if on { r } else { 0.0 };
    let (rtl, rtr, rbl, rbr) = (pick(tl), pick(tr), pick(bl), pick(br));
    with(|s| -> std::result::Result<(), wasm_bindgen::JsValue> {
        let c = &s.ctx;
        c.begin_path();
        c.move_to(x + rtl, y);
        c.line_to(x + w - rtr, y);
        c.arc_to(x + w, y, x + w, y + rtr, rtr)?;
        c.line_to(x + w, y + h - rbr);
        c.arc_to(x + w, y + h, x + w - rbr, y + h, rbr)?;
        c.line_to(x + rbl, y + h);
        c.arc_to(x, y + h, x, y + h - rbl, rbl)?;
        c.line_to(x, y + rtl);
        c.arc_to(x, y, x + rtl, y, rtl)?;
        c.close_path();
        c.fill();
        Ok(())
    })?
    .map_err(js)
}

pub fn circle(x: f64, y: f64, r: f64, fill: bool) -> Result<()> {
    with(|s| -> std::result::Result<(), wasm_bindgen::JsValue> {
        s.ctx.begin_path();
        s.ctx.arc(x, y, r, 0.0, TAU)?;
        if fill { s.ctx.fill() } else { s.ctx.stroke() }
        Ok(())
    })?
    .map_err(js)
}

pub fn poly(verts: Table) -> Result<()> {
    let mut points = Vec::new();
    for i in 1..=verts.len()? {
        let v: Table = verts.get(i)?;
        points.push((v.get::<_, f64>("x")?, v.get::<_, f64>("y")?));
    }
    with(|s| {
        let Some(&(x0, y0)) = points.first() else { return };
        s.ctx.begin_path();
        s.ctx.move_to(x0, y0);
        for &(x, y) in &points[1..] {
            s.ctx.line_to(x, y);
        }
        s.ctx.close_path();
        s.ctx.fill();
    })
}

/// Text is placed by its top edge (as in GMod) using the font's ascent, so `y..y+h`
/// from `measure` is exactly the box it occupies. Returns the drawn width.
pub fn draw_text(text: &str, x: f64, y: f64) -> Result<f64> {
    let (ctx, font, text_color, color) = with(|s| (s.ctx.clone(), s.font.clone(), s.text_color.clone(), s.color.clone()))?;
    let (ascent, _) = metrics(&ctx, &font)?;
    ctx.set_fill_style_str(&text_color);
    ctx.set_text_baseline("alphabetic");
    ctx.fill_text(text, x, y + ascent).map_err(js)?;
    ctx.set_fill_style_str(&color);
    Ok(ctx.measure_text(text).map_err(js)?.width())
}

pub fn measure(text: &str, font: &str) -> Result<(f64, f64)> {
    let ctx = measure_ctx()?;
    ctx.set_font(font);
    let (ascent, descent) = metrics(&ctx, font)?;
    let w = ctx.measure_text(text).map_err(js)?.width();
    Ok((w, ascent + descent))
}

fn metrics(ctx: &CanvasRenderingContext2d, font: &str) -> Result<(f64, f64)> {
    if let Some(m) = METRICS.with(|m| m.borrow().get(font).copied()) {
        return Ok(m);
    }
    let tm = ctx.measure_text("Mg").map_err(js)?;
    let (mut a, mut d) = (tm.font_bounding_box_ascent(), tm.font_bounding_box_descent());
    if a.is_nan() || a <= 0.0 {
        let size = font.split_whitespace().find_map(|p| p.strip_suffix("px")?.parse::<f64>().ok()).unwrap_or(14.0);
        (a, d) = (size * 0.9, size * 0.3);
    }
    METRICS.with(|m| m.borrow_mut().insert(font.to_string(), (a, d)));
    Ok((a, d))
}

fn measure_ctx() -> Result<CanvasRenderingContext2d> {
    if let Some(c) = MEASURE.with(|m| m.borrow().clone()) {
        return Ok(c);
    }
    let canvas: HtmlCanvasElement = dom::document().create_element("canvas").map_err(js)?.unchecked_into();
    let ctx: CanvasRenderingContext2d =
        canvas.get_context("2d").map_err(js)?.ok_or_else(|| err("no 2d context"))?.unchecked_into();
    MEASURE.with(|m| *m.borrow_mut() = Some(ctx.clone()));
    Ok(ctx)
}

/// Registers a web font; `R.font_loaded(family, err)` fires when it is ready (or failed).
/// Cached metrics were measured with the fallback font, so they are dropped on success.
pub fn load_font(family: String, url: &str, weight: &str, style: &str) -> Result<()> {
    let desc = FontFaceDescriptors::new();
    desc.set_weight(weight);
    desc.set_style(style);
    let source = format!("url(\"{}\")", url.replace('"', "%22"));
    let face = FontFace::new_with_str_and_descriptors(&family, &source, &desc).map_err(js)?;
    wasm_bindgen_futures::spawn_local(async move {
        let loaded = match face.load() {
            Ok(p) => JsFuture::from(p).await.map(drop),
            Err(e) => Err(e),
        };
        let error = match loaded {
            Ok(()) => {
                let _ = dom::document().fonts().add(&face);
                METRICS.with(|m| m.borrow_mut().clear());
                None
            }
            Err(e) => Some(describe(e)),
        };
        crate::call::<()>("font_loaded", (family, error));
    });
    Ok(())
}

fn describe(e: JsValue) -> String {
    e.as_string()
        .or_else(|| js_sys::Reflect::get(&e, &"message".into()).ok().and_then(|m| m.as_string()))
        .unwrap_or_else(|| format!("{e:?}"))
}

pub fn image_load(url: &str) -> Result<u32> {
    let img = HtmlImageElement::new().map_err(js)?;
    let id = IMAGES.with(|i| {
        let mut i = i.borrow_mut();
        i.list.push(img.clone());
        i.list.len() as u32
    });
    let onload = Closure::<dyn FnMut()>::new(move || {
        let panels = IMAGES.with(|i| i.borrow_mut().waiting.remove(&id)).unwrap_or_default();
        for p in panels {
            crate::call::<()>("invalidate", p);
        }
    });
    img.set_onload(Some(onload.as_ref().unchecked_ref()));
    onload.forget();
    img.set_src(url);
    Ok(id)
}

fn ready_image(id: u32) -> Result<Option<HtmlImageElement>> {
    let img = IMAGES.with(|i| i.borrow().list.get(id as usize - 1).cloned()).ok_or_else(|| err("invalid material"))?;
    if img.complete() && img.natural_width() > 0 {
        return Ok(Some(img));
    }
    let panel = with(|s| s.panel)?;
    IMAGES.with(|i| {
        let mut i = i.borrow_mut();
        let list = i.waiting.entry(id).or_default();
        if !list.contains(&panel) {
            list.push(panel);
        }
    });
    Ok(None)
}

pub fn image(id: u32, x: f64, y: f64, w: f64, h: f64) -> Result<()> {
    let Some(img) = ready_image(id)? else { return Ok(()) };
    with(|s| s.ctx.draw_image_with_html_image_element_and_dw_and_dh(&img, x, y, w, h))?.map_err(js)
}

#[allow(clippy::too_many_arguments)]
pub fn image_uv(id: u32, x: f64, y: f64, w: f64, h: f64, u0: f64, v0: f64, u1: f64, v1: f64) -> Result<()> {
    let Some(img) = ready_image(id)? else { return Ok(()) };
    let (iw, ih) = (img.natural_width() as f64, img.natural_height() as f64);
    with(|s| {
        s.ctx.draw_image_with_html_image_element_and_sw_and_sh_and_dx_and_dy_and_dw_and_dh(
            &img,
            u0 * iw,
            v0 * ih,
            (u1 - u0) * iw,
            (v1 - v0) * ih,
            x,
            y,
            w,
            h,
        )
    })?
    .map_err(js)
}

pub fn image_size(id: u32) -> Result<(u32, u32)> {
    let img = IMAGES.with(|i| i.borrow().list.get(id as usize - 1).cloned()).ok_or_else(|| err("invalid material"))?;
    Ok((img.natural_width(), img.natural_height()))
}
