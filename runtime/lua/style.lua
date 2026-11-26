---@diagnostic disable: duplicate-set-field, inject-field
local N, R = ...

local Panel = ui.GetClass("Panel")

local function set_of(words)
    local t = {}
    for w in words:gmatch("%S+") do t[w] = true end
    return t
end

-- Position, display and grid stay with SetPos/Dock/SetLayout, which own them.
local PROPS = set_of([[
background background-color background-image background-position background-size background-repeat
color opacity visibility cursor
border border-width border-style border-color border-top border-right border-bottom border-left
border-radius border-top-left-radius border-top-right-radius border-bottom-left-radius border-bottom-right-radius
outline outline-width outline-style outline-color outline-offset
box-shadow text-shadow filter backdrop-filter mix-blend-mode
transform transform-origin translate rotate scale transition
overflow overflow-x overflow-y text-overflow
padding padding-top padding-right padding-bottom padding-left
margin margin-top margin-right margin-bottom margin-left
width height min-width min-height max-width max-height aspect-ratio
gap row-gap column-gap flex flex-grow flex-shrink flex-basis order align-self justify-self z-index
font-size font-weight font-style font-family letter-spacing line-height word-spacing
text-align text-decoration text-transform text-indent white-space word-break overflow-wrap
user-select pointer-events object-fit object-position
]])

local UNITLESS = set_of("opacity z-index font-weight line-height flex flex-grow flex-shrink order scale aspect-ratio")

-- No url(), image() or image-set(): a style value must not make the browser fetch anything.
local FUNCS = set_of([[
rgb rgba hsl hsla hwb lab lch oklab oklch color color-mix calc min max clamp var env
linear-gradient radial-gradient conic-gradient repeating-linear-gradient repeating-radial-gradient
repeating-conic-gradient translate translatex translatey translatez translate3d rotate rotatex rotatey
rotatez rotate3d scale scalex scaley scalez scale3d skew skewx skewy matrix matrix3d perspective
cubic-bezier steps linear blur brightness contrast drop-shadow grayscale hue-rotate invert opacity
saturate sepia minmax fit-content repeat
]])

local function fail(msg) error(msg, 0) end

-- Backslash is banned too: CSS escapes can spell "u\72l(".
local function check(v)
    local lower = v:lower()
    if lower:find("[\\;{}<>]") then fail("style value '" .. v .. "' contains a forbidden character") end
    for name in lower:gmatch("([%a%-][%w%-]*)%s*%(") do
        if not FUNCS[name] then fail("style value '" .. v .. "' uses " .. name .. "(), which is not allowed") end
    end
    return v
end

local function px(v)
    if type(v) == "number" then return v .. "px" end
    if type(v) == "string" then return check(v) end
    fail("expected a number or a CSS length, got " .. type(v))
end

local function color(v)
    if IsColor(v) then return R.css_color(v) end
    if type(v) == "string" then return check(v) end
    fail("expected a Color, got " .. type(v))
end

local function value_for(prop, v)
    if v == false then return "none" end
    if IsColor(v) then return R.css_color(v) end
    if type(v) == "number" then
        if UNITLESS[prop] then return tostring(v) end
        if prop == "rotate" then return v .. "deg" end
        return v .. "px"
    end
    if type(v) == "string" then return check(v) end
    fail("style '" .. prop .. "': unsupported value of type " .. type(v))
end

local BORDER_STYLES = set_of("solid dashed dotted double groove ridge inset outset none hidden")

local function border(v)
    if v == false then return "none" end
    if type(v) == "number" then return v .. "px solid var(--ngs-border)" end
    if IsColor(v) then return "1px solid " .. R.css_color(v) end
    if type(v) == "string" then return check(v) end
    local w, c, style = v[1] or v.width or 1, v[2] or v.color, v[3] or v.style or "solid"
    if not BORDER_STYLES[style] then fail("unknown border style '" .. tostring(style) .. "'") end
    return string.format("%s %s %s", px(w), style, c and color(c) or "var(--ngs-border)")
end

local function radius(v)
    if type(v) == "table" then
        local parts = {}
        for i = 1, 4 do parts[i] = px(v[i] or 0) end
        return table.concat(parts, " ")
    end
    return px(v)
end

local function one_shadow(s)
    local x, y, blur, c, spread = s[1] or s.x or 0, s[2] or s.y or 0, s[3] or s.blur or 0, s[4] or s.color, s[5] or s.spread or 0
    return string.format("%s%s %s %s %s %s", s.inset and "inset " or "", px(x), px(y), px(blur), px(spread),
        c and color(c) or "rgba(0,0,0,0.35)")
end

local function shadow(v)
    if v == false then return "none" end
    if type(v) == "string" then return check(v) end
    if type(v[1]) ~= "table" then return one_shadow(v) end
    local parts = {}
    for i, s in ipairs(v) do parts[i] = one_shadow(s) end
    return table.concat(parts, ", ")
end

local function gradient(v)
    if v == false then return "none" end
    if type(v) == "string" then return check(v) end
    local stops = {}
    for _, s in ipairs(v) do
        if type(s) == "table" and not IsColor(s) then
            stops[#stops + 1] = color(s[1]) .. " " .. tostring(tonumber(s[2]) or 0) .. "%"
        else
            stops[#stops + 1] = color(s)
        end
    end
    if #stops < 2 then fail("gradient needs at least two colors") end
    if v.radial then return "radial-gradient(circle, " .. table.concat(stops, ", ") .. ")" end
    return string.format("linear-gradient(%gdeg, %s)", tonumber(v.angle) or 180, table.concat(stops, ", "))
end

local function transform(v)
    if v == false then return "none" end
    if type(v) == "string" then return check(v) end
    local parts = {}
    if v.x or v.y then parts[#parts + 1] = string.format("translate(%s, %s)", px(v.x or 0), px(v.y or 0)) end
    if v.rotate then parts[#parts + 1] = string.format("rotate(%gdeg)", v.rotate) end
    if v.scale then parts[#parts + 1] = string.format("scale(%g)", v.scale) end
    if v.scaleX or v.scaleY then parts[#parts + 1] = string.format("scale(%g, %g)", v.scaleX or 1, v.scaleY or 1) end
    if v.skewX or v.skewY then parts[#parts + 1] = string.format("skew(%gdeg, %gdeg)", v.skewX or 0, v.skewY or 0) end
    return #parts > 0 and table.concat(parts, " ") or "none"
end

local OVERFLOW = set_of("visible hidden scroll auto clip")

local function overflow_mode(m)
    if not OVERFLOW[m] then fail("overflow must be visible, hidden, scroll, auto or clip, got '" .. tostring(m) .. "'") end
    return m
end

local function box4(v)
    if type(v) ~= "table" then return px(v) end
    return string.format("%s %s %s %s", px(v[2] or 0), px(v[3] or 0), px(v[4] or 0), px(v[1] or 0))
end

-- Keys beyond plain CSS names. `out(prop, value)` collects declarations; `props` lists what a nil clears.
local COMPOSITE = {
    border = { props = { "border" }, fn = function(v, out) out("border", border(v)) end },
    radius = { props = { "border-radius" }, fn = function(v, out) out("border-radius", radius(v)) end },
    shadow = { props = { "box-shadow" }, fn = function(v, out) out("box-shadow", shadow(v)) end },
    gradient = { props = { "background-image" }, fn = function(v, out) out("background-image", gradient(v)) end },
    transform = { props = { "transform" }, fn = function(v, out) out("transform", transform(v)) end },
    alpha = { props = { "opacity" }, fn = function(v, out) out("opacity", tostring(v / 255)) end },
    font = { props = { "font" }, fn = function(v, out) out("font", R.font_css(v)) end },
    padding = { props = { "padding" }, fn = function(v, out) out("padding", box4(v)) end },
    margin = { props = { "margin" }, fn = function(v, out) out("margin", box4(v)) end },
    textColor = { props = { "color" }, fn = function(v, out) out("color", color(v)) end },
    background = {
        props = { "background", "background-color" },
        fn = function(v, out)
            if IsColor(v) then out("background-color", R.css_color(v)) else out("background", value_for("background", v)) end
        end,
    },
    overflow = {
        props = { "overflow", "overflow-x", "overflow-y" },
        fn = function(v, out)
            if type(v) == "table" then
                out("overflow-x", overflow_mode(v[1]))
                out("overflow-y", overflow_mode(v[2] or v[1]))
            else
                out("overflow", overflow_mode(v))
            end
        end,
    },
}

local function kebab(k) return (k:gsub("%u", function(c) return "-" .. c:lower() end)) end

--- Declarations for one style key; a nil value yields { prop, nil } pairs that clear it.
local function resolve(key, value, out)
    if type(key) ~= "string" then fail("style keys must be strings") end
    local comp = COMPOSITE[key]
    if comp then
        if value == nil then
            for _, p in ipairs(comp.props) do out(p, nil) end
        else
            comp.fn(value, out)
        end
        return
    end
    local prop = kebab(key)
    if not PROPS[prop] then fail("style property '" .. key .. "' is not allowed") end
    out(prop, value ~= nil and value_for(prop, value) or nil)
end

local function apply(self, key, value)
    resolve(key, value, function(p, v) N.style(self.__id, p, v) end)
end

--- SetStyle("border-radius", 8), SetStyle("borderRadius", 8) or SetStyle{ radius = 8, color = c }.
--- nil clears a property.
function Panel:SetStyle(prop, value)
    if type(prop) == "table" then
        for k, v in pairs(prop) do apply(self, k, v) end
    else
        apply(self, prop, value)
    end
end

--- SetBorder(1, color, "dashed"), SetBorder(false) for none, SetBorder(nil) back to the default.
function Panel:SetBorder(width, c, style)
    if type(width) == "number" then width = { width, c, style } end
    apply(self, "border", width)
end

--- One radius, or four: top-left, top-right, bottom-right, bottom-left.
function Panel:SetRadius(r, tr, br, bl)
    apply(self, "radius", tr ~= nil and { r, tr, br, bl } or r)
end

function Panel:SetShadow(x, y, blur, c, spread, inset)
    if type(x) == "number" then x = { x, y, blur, c, spread, inset = inset } end
    apply(self, "shadow", x)
end

--- SetGradient(from, to, angle) or a table: { c1, c2, { c3, 80 }, angle = 90, radial = false }.
function Panel:SetGradient(from, to, angle)
    if IsColor(from) then from = { from, to, angle = angle } end
    apply(self, "gradient", from)
end

--- { x, y, rotate (deg), scale, scaleX, scaleY, skewX, skewY }; independent of SetPos/Center.
function Panel:SetTransform(t) apply(self, "transform", t) end

function Panel:SetOverflow(x, y)
    apply(self, "overflow", y ~= nil and { x, y } or x)
end

--- Animates every style change (hover styles included) over `duration` seconds.
function Panel:SetTransition(duration, ease)
    self.__transition = duration and string.format("all %gs %s", duration, check(ease or "ease")) or nil
    R.apply_transition(self)
end

-- State styles live in a stylesheet, so hover/press/focus never go through Lua. The rules are
-- !important to beat inline styles (SetBackgroundColor & co) while the state lasts.
local STATES = {
    hover = { ":hover:not(.ngs-disabled):not(:disabled)" },
    active = { ":active:not(.ngs-disabled):not(:disabled)" },
    focus = { ":focus-within" },
    disabled = { ".ngs-disabled", ":disabled" },
}

local compiled = {}
local next_class = 0

local function compile(state, tbl)
    local decls = {}
    for k, v in pairs(tbl) do
        resolve(k, v, function(p, val) decls[#decls + 1] = { p, val } end)
    end
    table.sort(decls, function(a, b) return a[1] < b[1] end)
    local parts = {}
    for i, d in ipairs(decls) do parts[i] = d[1] .. ":" .. d[2] end
    local key = state .. "|" .. table.concat(parts, ";")
    local cls = compiled[key]
    if cls then return cls end
    next_class = next_class + 1
    cls = "ngs-s" .. next_class
    local selectors = {}
    for i, suffix in ipairs(STATES[state]) do selectors[i] = "." .. cls .. suffix end
    local flat = {}
    for _, d in ipairs(decls) do
        flat[#flat + 1] = d[1]
        flat[#flat + 1] = d[2]
    end
    N.css_rule(table.concat(selectors, ", "), flat)
    compiled[key] = cls
    return cls
end

--- state: "hover", "active", "focus" (anything inside has focus) or "disabled".
--- Same keys as SetStyle; nil or an empty table removes the state style.
function Panel:SetStyleState(state, tbl)
    if not STATES[state] then fail("unknown style state '" .. tostring(state) .. "'") end
    local cls = (tbl and next(tbl) ~= nil) and compile(state, tbl) or nil
    local current = rawget(self, "__state_cls") or {}
    self.__state_cls = current
    local old = current[state]
    if old == cls then return end
    if old then N.class(self.__id, old, false) end
    if cls then N.class(self.__id, cls, true) end
    current[state] = cls
end

function Panel:SetStyleHover(tbl) self:SetStyleState("hover", tbl) end
function Panel:SetStyleActive(tbl) self:SetStyleState("active", tbl) end
function Panel:SetStyleFocus(tbl) self:SetStyleState("focus", tbl) end
function Panel:SetStyleDisabled(tbl) self:SetStyleState("disabled", tbl) end
