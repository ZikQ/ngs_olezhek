---@diagnostic disable: duplicate-set-field, redundant-parameter
local N, R = ...

surface = {}
draw = {}

TEXT_ALIGN_LEFT, TEXT_ALIGN_CENTER, TEXT_ALIGN_RIGHT, TEXT_ALIGN_TOP, TEXT_ALIGN_BOTTOM = 0, 1, 2, 3, 4

local FALLBACK = 'system-ui, -apple-system, "Segoe UI", Roboto, sans-serif'
local fonts = {}

local requested = {}
local loaded = {}

--- OnFontLoaded fires once per font, so a panel created later has to ask instead of waiting for it.
function surface.IsFontLoaded(family) return loaded[family] == true end

--- Downloads a font file (woff2/ttf, e.g. from App::public) and registers it under `family`.
function surface.LoadFont(family, url, data)
    data = data or {}
    local weight = tostring(data.weight or 400)
    local style = data.italic and "italic" or "normal"
    local key = table.concat({ family, url, weight, style }, "|")
    if requested[key] then return end
    requested[key] = true
    N.load_font(family, url, weight, style)
end

--- data: { font = "Roboto", size = 16, weight = 500, italic = false, url = "/fonts/Roboto.woff2" }
--- With `url` the font file is loaded too; until then text uses the fallback font.
function surface.CreateFont(name, data)
    data = data or {}
    local family = data.font or data.family
    local stack = family and ('"' .. family .. '", ' .. FALLBACK) or FALLBACK
    fonts[name] = string.format("%s%d %gpx %s", data.italic and "italic " or "", data.weight or 400, data.size or 14, stack)
    if family and data.url then surface.LoadFont(family, data.url, data) end
end

-- DOM text switches fonts by itself; canvases drew with the fallback and need a repaint.
function R.font_loaded(family, err)
    if err then
        N.error("surface.LoadFont('" .. family .. "'): " .. err)
        return
    end
    loaded[family] = true
    for _, p in pairs(R.panels) do p:InvalidatePaint() end
    hook.Run("OnFontLoaded", family)
end

function R.font_css(name) return fonts[name] or fonts.Default end

surface.CreateFont("Default", { size = 14 })
surface.CreateFont("DefaultBold", { size = 14, weight = 700 })
surface.CreateFont("Small", { size = 12 })
surface.CreateFont("Large", { size = 20, weight = 500 })
surface.CreateFont("Title", { size = 28, weight = 700 })

local cur_font = "Default"
local text_x, text_y = 0, 0
local material

function R.surface_begin()
    cur_font = "Default"
    text_x, text_y = 0, 0
    material = nil
end

local function rgba(r, g, b, a)
    if type(r) == "table" then return r.r, r.g, r.b, r.a or 255 end
    return r or 255, g or 255, b or 255, a or 255
end

function surface.SetDrawColor(r, g, b, a) N.color(rgba(r, g, b, a)) end
function surface.SetTextColor(r, g, b, a) N.text_color(rgba(r, g, b, a)) end

-- Also usable outside Paint (before GetTextSize in PerformLayout), so only touch the canvas while painting.
function surface.SetFont(name)
    cur_font = fonts[name] and name or "Default"
    if R.painting then N.font(fonts[cur_font]) end
end

function surface.GetFont() return cur_font end
function surface.GetTextSize(text) return N.measure(tostring(text), fonts[cur_font]) end

function surface.SetTextPos(x, y) text_x, text_y = x, y end
function surface.GetTextPos() return text_x, text_y end

function surface.DrawText(text)
    text_x = text_x + N.draw_text(tostring(text), text_x, text_y)
end

function surface.DrawRect(x, y, w, h) N.rect(x, y, w, h) end
function surface.DrawOutlinedRect(x, y, w, h, thickness) N.outlined(x, y, w, h, thickness or 1) end
function surface.DrawLine(x1, y1, x2, y2) N.line(x1, y1, x2, y2) end

function surface.DrawCircle(x, y, radius, r, g, b, a)
    if r then surface.SetDrawColor(r, g, b, a) end
    N.circle(x, y, radius, false)
end

--- verts: { {x = 0, y = 0}, {x = 10, y = 0}, ... }
function surface.DrawPoly(verts) N.poly(verts) end

--- Linear gradient for the following fills and strokes until the next SetDrawColor.
--- stops: { Color, Color, ... } evenly spaced, or { {0, Color}, {0.7, Color}, ... } with offsets 0..1.
function surface.SetDrawGradient(x1, y1, x2, y2, stops)
    if type(stops) ~= "table" or #stops < 2 then error("SetDrawGradient: need at least two stops", 2) end
    local flat, n = {}, #stops
    for i, s in ipairs(stops) do
        local offset, c
        if IsColor(s) then
            offset, c = (i - 1) / (n - 1), s
        else
            offset, c = s[1], s[2]
        end
        if type(offset) ~= "number" or not IsColor(c) then
            error("SetDrawGradient: stop " .. i .. " must be a Color or { offset, Color }", 2)
        end
        local base = #flat
        flat[base + 1], flat[base + 2], flat[base + 3], flat[base + 4], flat[base + 5] = offset, c.r, c.g, c.b, c.a or 255
    end
    N.gradient(x1, y1, x2, y2, flat)
end

local CAPS = { butt = true, round = true, square = true }
local JOINS = { miter = true, round = true, bevel = true }

function surface.SetLineWidth(w) N.line_width(tonumber(w) or 1) end

function surface.SetLineCap(cap)
    cap = cap or "butt"
    if not CAPS[cap] then error("SetLineCap: expected 'butt', 'round' or 'square', got " .. tostring(cap), 2) end
    N.line_cap(cap)
end

function surface.SetLineJoin(join)
    join = join or "miter"
    if not JOINS[join] then error("SetLineJoin: expected 'miter', 'round' or 'bevel', got " .. tostring(join), 2) end
    N.line_join(join)
end

--- pattern: { dash, gap, ... } in pixels; nil or {} draws solid lines again.
function surface.SetLineDash(pattern)
    local list = {}
    for i, v in ipairs(pattern or {}) do
        if type(v) ~= "number" or v < 0 then error("SetLineDash: entries must be non-negative numbers", 2) end
        list[i] = v
    end
    N.line_dash(list)
end

--- Degrees, 0 points right and angles grow clockwise. fill = true draws a pie slice instead of a stroke.
function surface.DrawArc(x, y, radius, from, to, fill) N.arc(x, y, radius, from, to, fill == true) end

local PATH_MODES = { fill = true, stroke = true, both = true }

--- d: SVG path data. The geometry is scaled and then moved by x, y; line width is not scaled.
function surface.DrawPath(d, mode, x, y, sx, sy)
    if type(d) ~= "string" then error("DrawPath: path data must be a string", 2) end
    mode = mode or "fill"
    if not PATH_MODES[mode] then error("DrawPath: mode must be 'fill', 'stroke' or 'both', got " .. tostring(mode), 2) end
    sx = sx or 1
    N.path(d, mode, x or 0, y or 0, sx, sy or sx)
end

function surface.SetMaterial(mat) material = mat end

function surface.DrawTexturedRect(x, y, w, h)
    if material then N.image(material.__img, x, y, w, h) end
end

function surface.DrawTexturedRectUV(x, y, w, h, u0, v0, u1, v1)
    if material then N.image_uv(material.__img, x, y, w, h, u0, v0, u1, v1) end
end

function surface.SetAlphaMultiplier(a) N.alpha(a) end

local MATERIAL = {}
MATERIAL.__index = MATERIAL

function MATERIAL:Width() return (N.image_size(self.__img)) end
function MATERIAL:Height() return select(2, N.image_size(self.__img)) end
function MATERIAL:GetName() return self.__path end

local materials = {}

--- Image by URL (e.g. "/static/logo.png"). Panels that draw it before it loads get repainted on load.
function Material(path)
    local m = materials[path]
    if not m then
        m = setmetatable({ __img = N.image_load(path), __path = path }, MATERIAL)
        materials[path] = m
    end
    return m
end

function draw.RoundedBox(radius, x, y, w, h, col)
    surface.SetDrawColor(col)
    N.rounded(radius, x, y, w, h, true, true, true, true)
end

function draw.RoundedBoxEx(radius, x, y, w, h, col, tl, tr, bl, br)
    surface.SetDrawColor(col)
    N.rounded(radius, x, y, w, h, tl == true, tr == true, bl == true, br == true)
end

function draw.Circle(x, y, radius, col)
    surface.SetDrawColor(col)
    N.circle(x, y, radius, true)
end

function draw.SimpleText(text, font, x, y, col, xalign, yalign)
    text = tostring(text)
    surface.SetFont(font or "Default")
    local w, h = surface.GetTextSize(text)
    x, y = x or 0, y or 0
    if xalign == TEXT_ALIGN_CENTER then
        x = x - w / 2
    elseif xalign == TEXT_ALIGN_RIGHT then
        x = x - w
    end
    if yalign == TEXT_ALIGN_CENTER then
        y = y - h / 2
    elseif yalign == TEXT_ALIGN_BOTTOM then
        y = y - h
    end
    surface.SetTextColor(col or color_white)
    surface.SetTextPos(x, y)
    surface.DrawText(text)
    return w, h
end

function draw.DrawText(text, font, x, y, col, xalign)
    local width, cy = 0, y
    for line in (tostring(text) .. "\n"):gmatch("(.-)\n") do
        local w, h = draw.SimpleText(line, font, x, cy, col, xalign)
        width = math.max(width, w)
        cy = cy + h
    end
    return width, cy - y
end

function draw.NoTexture() material = nil end

function draw.GetFontHeight(font)
    return select(2, N.measure("Mg", fonts[font] or fonts.Default))
end
