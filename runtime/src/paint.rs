use std::{cell::RefCell, collections::HashMap, f64::consts::TAU};

use omnilua::{Result, Table};
use wasm_bindgen::{JsCast, JsValue, prelude::Closure};
use wasm_bindgen_futures::JsFuture;
use web_sys::{
    CanvasRenderingContext2d, FontFace, FontFaceDescriptors, HtmlCanvasElement, HtmlImageElement, Path2d,
};

use crate::{dom, dom::js, err};

const WHITE: &str = "rgba(255,255,255,1)";

struct State {
    panel: u32,
    ctx: CanvasRenderingContext2d,
    text_color: String,
    font: String,
    line_width: f64,
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
    // The context outlives a single Paint, so everything a previous Paint could have changed is reset.
    ctx.set_fill_style_str(WHITE);
    ctx.set_stroke_style_str(WHITE);
    ctx.set_font(&font);
    ctx.set_line_width(1.0);
    ctx.set_line_cap("butt");
    ctx.set_line_join("miter");
    ctx.set_line_dash(&js_sys::Array::new()).map_err(js)?;
    PAINT.with(|p| *p.borrow_mut() = Some(State { panel, ctx, text_color: WHITE.into(), font, line_width: 1.0 }));
    Ok(())
}

pub fn end() {
    PAINT.with(|p| *p.borrow_mut() = None);
}

pub fn color(r: f64, g: f64, b: f64, a: f64) -> Result<()> {
    with(|s| {
        let color = rgba(r, g, b, a);
        s.ctx.set_fill_style_str(&color);
        s.ctx.set_stroke_style_str(&color);
    })
}

/// `stops` is flat: offset, r, g, b, a for every stop.
pub fn gradient(x1: f64, y1: f64, x2: f64, y2: f64, stops: Vec<f64>) -> Result<()> {
    with(|s| -> std::result::Result<(), JsValue> {
        let g = s.ctx.create_linear_gradient(x1, y1, x2, y2);
        for st in stops.chunks_exact(5) {
            g.add_color_stop(st[0].clamp(0.0, 1.0) as f32, &rgba(st[1], st[2], st[3], st[4]))?;
        }
        s.ctx.set_fill_style_canvas_gradient(&g);
        s.ctx.set_stroke_style_canvas_gradient(&g);
        Ok(())
    })?
    .map_err(js)
}

pub fn line_width(w: f64) -> Result<()> {
    with(|s| {
        s.line_width = w.max(0.0);
        s.ctx.set_line_width(s.line_width);
    })
}

pub fn line_cap(cap: &str) -> Result<()> {
    with(|s| s.ctx.set_line_cap(cap))
}

pub fn line_join(join: &str) -> Result<()> {
    with(|s| s.ctx.set_line_join(join))
}

pub fn line_dash(pattern: Vec<f64>) -> Result<()> {
    let arr: js_sys::Array = pattern.into_iter().map(JsValue::from_f64).collect();
    with(|s| s.ctx.set_line_dash(&arr))?.map_err(js)
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
        // Odd whole widths centered on a pixel edge would smear over two pixel rows.
        let w = s.line_width;
        let off = if w.fract() == 0.0 && (w as i64) % 2 == 1 { 0.5 } else { 0.0 };
        s.ctx.begin_path();
        s.ctx.move_to(x1 + off, y1 + off);
        s.ctx.line_to(x2 + off, y2 + off);
        s.ctx.stroke();
    })
}

/// Angles in degrees, 0 pointing right, growing clockwise (screen y goes down).
pub fn arc(x: f64, y: f64, r: f64, from: f64, to: f64, fill: bool) -> Result<()> {
    let (a0, a1) = (from.to_radians(), to.to_radians());
    with(|s| -> std::result::Result<(), JsValue> {
        s.ctx.begin_path();
        if fill {
            s.ctx.move_to(x, y);
        }
        s.ctx.arc_with_anticlockwise(x, y, r.max(0.0), a0, a1, a1 < a0)?;
        if fill {
            s.ctx.close_path();
            s.ctx.fill();
        } else {
            s.ctx.stroke();
        }
        Ok(())
    })?
    .map_err(js)
}

/// SVG path data (`M 2 3 L 10 4 Z`, arcs and curves included). The offset and scale move the
/// geometry only, so line width stays in panel pixels whatever the scale.
pub fn path(d: &str, mode: &str, x: f64, y: f64, sx: f64, sy: f64) -> Result<()> {
    let parsed = Path2d::new_with_path_string(d).map_err(js)?;
    let shape = if (x, y, sx, sy) == (0.0, 0.0, 1.0, 1.0) {
        parsed
    } else {
        // web-sys types addPath's matrix as SVGMatrix, but the browser takes a plain DOMMatrix2DInit dict.
        let m = js_sys::Object::new();
        for (k, v) in [("a", sx), ("b", 0.0), ("c", 0.0), ("d", sy), ("e", x), ("f", y)] {
            js_sys::Reflect::set(&m, &k.into(), &v.into()).map_err(js)?;
        }
        let moved = Path2d::new().map_err(js)?;
        let add: js_sys::Function = js_sys::Reflect::get(&moved, &"addPath".into()).map_err(js)?.unchecked_into();
        add.call2(&moved, &parsed, &m).map_err(js)?;
        moved
    };
    with(|s| {
        if mode != "stroke" {
            s.ctx.fill_with_path_2d(&shape);
        }
        if mode != "fill" {
            s.ctx.stroke_with_path(&shape);
        }
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
    let (ctx, font, text_color) = with(|s| (s.ctx.clone(), s.font.clone(), s.text_color.clone()))?;
    let (ascent, _) = metrics(&ctx, &font)?;
    // save/restore rather than re-setting the draw color: the fill may be a gradient.
    ctx.save();
    ctx.set_fill_style_str(&text_color);
    ctx.set_text_baseline("alphabetic");
    let drawn = ctx.fill_text(text, x, y + ascent).map_err(js);
    ctx.restore();
    drawn?;
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
