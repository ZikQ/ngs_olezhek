---@diagnostic disable: duplicate-set-field
local N, R = ...

ui = {}

DOCK_NONE, DOCK_FILL, DOCK_LEFT, DOCK_RIGHT, DOCK_TOP, DOCK_BOTTOM = 0, 1, 2, 3, 4, 5
LAYOUT_COLUMN, LAYOUT_ROW, LAYOUT_GRID = "column", "row", "grid"
ALIGN_START, ALIGN_CENTER, ALIGN_END, ALIGN_STRETCH = "flex-start", "center", "flex-end", "stretch"
JUSTIFY_START, JUSTIFY_CENTER, JUSTIFY_END = "flex-start", "center", "flex-end"
JUSTIFY_BETWEEN, JUSTIFY_AROUND, JUSTIFY_EVENLY = "space-between", "space-around", "space-evenly"
MOUSE_LEFT, MOUSE_RIGHT, MOUSE_MIDDLE = 1, 2, 3

local classes = {}
-- One metatable per class name, shared by all instances: re-registering a class only swaps
-- its __index, so live panels pick up new methods (hot reload).
local metas = {}
local panels = {}
local thinkers = {}
local painters = {}
R.panels = panels
R.modals = {}

local FEATURES = { Paint = true, Think = true, PerformLayout = true }

local function len(v)
    if v == nil then return nil end
    if type(v) == "number" then return v .. "px" end
    return tostring(v)
end

local function box(l, t, r, b)
    return string.format("%spx %spx %spx %spx", t or 0, r or 0, b or 0, l or 0)
end

local function feature(self, key)
    if key == "Think" then
        thinkers[self] = true
        R.wake_think()
        return
    end
    if key == "Paint" then N.class(self.__id, "ngs-painted", true) end
    if not self.__observed then
        -- The observer fires once right after observe(), which does the first layout/paint.
        self.__observed = true
        N.observe(self.__id, true)
    elseif key == "Paint" then
        self:InvalidatePaint()
    else
        self:InvalidateLayout()
    end
end

local function newindex(self, k, v)
    rawset(self, k, v)
    if FEATURES[k] and v ~= nil then feature(self, k) end
end

local function class_meta(tbl)
    return {
        __index = function(_, k)
            local base = rawget(tbl, "Base")
            local bc = base and classes[base]
            if not bc then return nil end
            if k == "BaseClass" then return bc end
            return bc[k]
        end,
    }
end

function ui.Register(name, tbl, base)
    tbl = tbl or {}
    if base ~= nil then rawset(tbl, "Base", base) end
    rawset(tbl, "ClassName", name)
    setmetatable(tbl, class_meta(tbl))
    classes[name] = tbl
    local m = metas[name]
    if m then
        m.__index = tbl
    else
        metas[name] = {
            __index = tbl,
            __newindex = newindex,
            __tostring = function(self) return "Panel: " .. name .. " [" .. tostring(rawget(self, "__id")) .. "]" end,
        }
    end
    if R.on_register then R.on_register(name, tbl) end
    if R.on_class_registered then R.on_class_registered(name) end
    return tbl
end

--- True if any live panel is an instance of (or inherits from) one of the class names in `set`.
function R.uses_classes(set)
    for _, p in pairs(panels) do
        local cls = p.__class
        while cls do
            if set[cls] then return true end
            local c = classes[cls]
            cls = c and rawget(c, "Base")
        end
    end
    return false
end

function ui.GetClass(name) return classes[name] end
function ui.Exists(name) return classes[name] ~= nil end
function ui.IsPanel(v) return type(v) == "table" and rawget(v, "__id") ~= nil end
function ui.GetRoot() return R.root end

function IsValid(v)
    return v ~= nil and type(v) == "table" and type(v.IsValid) == "function" and v:IsValid() == true
end

local function chain(name)
    local list = {}
    local seen = {}
    local cur = name
    while cur do
        local c = classes[cur]
        if not c then error("unknown panel class '" .. tostring(cur) .. "'", 4) end
        if seen[cur] then error("panel class '" .. name .. "' inherits from itself", 4) end
        seen[cur] = true
        table.insert(list, 1, c)
        cur = rawget(c, "Base")
    end
    return list
end

-- Tags that run code or load documents. Setting text on a <script> panel would execute it.
local BANNED_TAGS = {
    script = true, style = true, link = true, meta = true, base = true, iframe = true, frame = true,
    frameset = true, object = true, embed = true, applet = true, template = true, noscript = true,
    svg = true, math = true, portal = true, form = true,
}
local checked_tags = {}

local function native_of(list)
    for i = #list, 1, -1 do
        local c = list[i]
        local native = rawget(c, "__native")
        if native then return native end
        local tag = rawget(c, "Tag")
        if tag then
            native = { tag = tag, class = "ngs-panel" }
            rawset(c, "__native", native)
            return native
        end
    end
end

local function check_tag(tag)
    if checked_tags[tag] then return end
    if type(tag) ~= "string" or not (tag:match("^%l[%l%d]*$") or tag:match("^%l[%l%d]*%-[%l%d%-]+$")) then
        error("invalid native tag '" .. tostring(tag) .. "'", 4)
    end
    if BANNED_TAGS[tag] then error("native tag <" .. tag .. "> is not allowed", 4) end
    checked_tags[tag] = true
end

-- Props whose value is a list of arguments, and methods usable as props without the Set prefix.
local UNPACK = {
    Size = true, Pos = true, MinSize = true, MaxSize = true, Border = true, Radius = true, Shadow = true,
    Gradient = true, Overflow = true, Gap = true, DockMargin = true, DockPadding = true, Transition = true,
}
local PLAIN_METHODS = { Dock = true, DockMargin = true, DockPadding = true }
R.UNPACK = UNPACK

local function apply_props(self, props)
    for k, v in pairs(props) do
        if type(k) == "string" and k ~= "Slot" then
            local setter = PLAIN_METHODS[k] and self[k] or self["Set" .. k]
            if type(setter) == "function" then
                if not PLAIN_METHODS[k] and ui.IsState(v) then
                    self:Bind(k, v)
                elseif UNPACK[k] and type(v) == "table" and getmetatable(v) == nil then
                    setter(self, table.unpack(v))
                else
                    setter(self, v)
                end
            end
        end
    end
end

local function add_children(self, props)
    for _, child in ipairs(props) do
        if type(child) == "function" then
            child(self)
        elseif ui.IsPanel(child) then
            -- A Rebuild removed it together with the other children.
            if not child.__removed then child:SetParent(self) end
        else
            error("ui.Create: children must be panels or functions, got " .. type(child), 4)
        end
    end
end

-- Base-first, like vgui. Runs as a coroutine so Initialize may await http.Get; setter props and
-- children go in once Initialize returns or starts waiting.
local function initialize(self, list)
    R.spawn_for(self, function()
        for _, c in ipairs(list) do
            local init = rawget(c, "Initialize")
            if init then init(self) end
        end
    end)
    local props = rawget(self, "Props")
    if props and not self.__removed then
        apply_props(self, props)
        add_children(self, props)
    end
end

local function create(name, parent, fields, props)
    local list = chain(name)
    local native = native_of(list)
    check_tag(native.tag)
    local id = N.create(native.tag, native.class or "", native.control, native.type)
    local self = { __id = id, __children = {}, __class = name }
    if fields then
        for k, v in pairs(fields) do self[k] = v end
    end
    setmetatable(self, metas[name])
    -- Props without a matching setter become plain fields, so Initialize can read them.
    if props then
        rawset(self, "Props", props)
        for k, v in pairs(props) do
            if type(k) == "string" and k ~= "Slot" and not PLAIN_METHODS[k] and type(self["Set" .. k]) ~= "function" then
                rawset(self, k, v)
            end
        end
    end
    panels[id] = self
    if parent ~= false then self:SetParent(parent or R.root, props and props.Slot) end
    if self.Paint then feature(self, "Paint") end
    if self.PerformLayout then feature(self, "PerformLayout") end
    if self.Think then feature(self, "Think") end
    initialize(self, list)
    return self
end
R.create = create

--- props: setters by name ({ Text = "Hi" } calls SetText after Initialize), other keys become
--- fields before it, the array part holds children (panels or functions building them), and
--- Slot picks the parent's slot (false = the parent itself).
function ui.Create(name, parent, props)
    if parent ~= nil and not ui.IsPanel(parent) then error("ui.Create: parent must be a panel", 2) end
    if props ~= nil and type(props) ~= "table" then error("ui.Create: props must be a table", 2) end
    return create(name, parent, nil, props)
end

local dock_q, layout_q, paint_q = {}, {}, {}
local scheduled = false

local function schedule()
    if not scheduled then
        scheduled = true
        N.schedule()
    end
end

function R.dock_dirty(p)
    if p and not p.__removed then
        dock_q[p] = true
        schedule()
    end
end

local function clear_cell(c)
    local id = c.__id
    N.style(id, "grid-row", nil)
    N.style(id, "grid-column", nil)
    N.style(id, "justify-self", nil)
    N.style(id, "align-self", nil)
end

function R.apply_flow(p)
    if p.__grid then return end
    local id, mode = p.__id, p.__layout
    if mode == nil then
        N.style(id, "display", nil)
    elseif mode == LAYOUT_GRID then
        N.style(id, "display", "grid")
        N.style(id, "flex-direction", nil)
        N.style(id, "grid-template-columns", p.__columns or "repeat(auto-fill, minmax(160px, 1fr))")
    else
        N.style(id, "display", "flex")
        N.style(id, "flex-direction", mode)
        N.style(id, "grid-template-columns", nil)
    end
end

-- Dock maps onto one CSS grid on the parent. Each TOP/BOTTOM dock adds a row track and each
-- LEFT/RIGHT dock a column track; a child spans whatever area was still free when it docked,
-- which is exactly vgui's "carve from the remaining rect in child order". Bottom and right
-- tracks are addressed with negative lines so they don't shift when more docks are added.
-- FILL (and undocked, unpositioned children) take the center 1fr cell.
local function apply_dock(p)
    local kids = p.__children
    local docked = false
    for i = 1, #kids do
        if (kids[i].__dock or 0) ~= DOCK_NONE then
            docked = true
            break
        end
    end
    local id = p.__id
    if not docked then
        if p.__grid then
            p.__grid = false
            N.style(id, "grid-template-rows", nil)
            N.style(id, "grid-template-columns", nil)
            for _, c in ipairs(kids) do clear_cell(c) end
            R.apply_flow(p)
        end
        return
    end

    p.__grid = true
    N.style(id, "display", "grid")
    local T, B, L, Rt = 0, 0, 0, 0
    local cells = {}
    for _, c in ipairs(kids) do
        local d = c.__dock or 0
        if d == DOCK_TOP then
            T = T + 1
            cells[c] = { T .. "/" .. T + 1, L + 1 .. "/" .. -(Rt + 1) }
        elseif d == DOCK_BOTTOM then
            cells[c] = { -(B + 2) .. "/" .. -(B + 1), L + 1 .. "/" .. -(Rt + 1) }
            B = B + 1
        elseif d == DOCK_LEFT then
            L = L + 1
            cells[c] = { T + 1 .. "/" .. -(B + 1), L .. "/" .. L + 1 }
        elseif d == DOCK_RIGHT then
            cells[c] = { T + 1 .. "/" .. -(B + 1), -(Rt + 2) .. "/" .. -(Rt + 1) }
            Rt = Rt + 1
        end
    end
    N.style(id, "grid-template-rows", string.rep("auto ", T) .. "minmax(0,1fr)" .. string.rep(" auto", B))
    N.style(id, "grid-template-columns", string.rep("auto ", L) .. "minmax(0,1fr)" .. string.rep(" auto", Rt))

    local center = { T + 1 .. "/" .. -(B + 1), L + 1 .. "/" .. -(Rt + 1) }
    for _, c in ipairs(kids) do
        local cell = cells[c]
        local d = c.__dock or 0
        if not cell and d == DOCK_NONE and c.__pos then
            clear_cell(c)
        else
            local loose = not cell and d ~= DOCK_FILL
            cell = cell or center
            N.style(c.__id, "grid-row", cell[1])
            N.style(c.__id, "grid-column", cell[2])
            N.style(c.__id, "justify-self", loose and "start" or nil)
            N.style(c.__id, "align-self", loose and "start" or nil)
        end
    end
end

function R.flush_docks()
    while next(dock_q) do
        local q = dock_q
        dock_q = {}
        for p in pairs(q) do
            if not p.__removed then apply_dock(p) end
        end
    end
end

local function depth(p)
    local d = 0
    while p.__parent do
        d = d + 1
        p = p.__parent
    end
    return d
end

function R.paint(p, w, h)
    if p.__removed or not p.Paint then return end
    if not w then
        R.flush_docks()
        w, h = N.size(p.__id)
    end
    N.canvas_begin(p.__id, w, h, R.font_css("Default"))
    R.painting = true
    R.surface_begin()
    R.pcall(p.Paint, p, w, h)
    R.painting = false
    N.canvas_end()
end

function R.flush()
    scheduled = false
    for _ = 1, 10 do
        R.flush_docks()
        if next(layout_q) == nil and next(paint_q) == nil then return end
        local list = {}
        for p in pairs(layout_q) do list[#list + 1] = p end
        layout_q = {}
        table.sort(list, function(a, b) return depth(a) < depth(b) end)
        for _, p in ipairs(list) do
            if not p.__removed and p.PerformLayout then
                R.pcall(p.PerformLayout, p, N.size(p.__id))
            end
        end
        local q = paint_q
        paint_q = {}
        for p in pairs(q) do R.paint(p) end
    end
end

function R.resized(id, w, h)
    local p = panels[id]
    if not p then return end
    p.__sized = true
    R.flush_docks()
    if p.PerformLayout then R.pcall(p.PerformLayout, p, w, h) end
    if p.Paint then
        paint_q[p] = nil
        R.paint(p, w, h)
    end
end

function R.invalidate(id)
    local p = panels[id]
    if p then p:InvalidatePaint() end
end

local last_frame
function R.wake_think() N.think(true) end

function R.think(now)
    R.frame_time = last_frame and math.min(now - last_frame, 0.25) or 0
    last_frame = now
    hook.Run("Think")
    local list = {}
    for p in pairs(thinkers) do list[#list + 1] = p end
    for _, p in ipairs(list) do
        if p.__removed or not p.Think then
            thinkers[p] = nil
        else
            R.pcall(p.Think, p)
        end
    end
    list = {}
    for p in pairs(painters) do list[#list + 1] = p end
    for _, p in ipairs(list) do R.paint(p) end
    R.flush()
    local more = next(thinkers) ~= nil or next(painters) ~= nil or R.has_think_hooks()
    if not more then last_frame = nil end
    return more
end

--- Rust calls this for DOM events; true means "cancel the browser default".
function R.event(id, name, arg)
    local p = panels[id]
    if not p then return false end
    if p.__tipBuild then R.tooltip_event(p, name, arg) end
    local h = p["__on_" .. name]
    if not h then return false end
    return h(p, arg) == true
end

function R.screen(w, h)
    hook.Run("OnScreenSizeChanged", w, h)
end

local SKIN = {
    background = "--ngs-bg",
    text = "--ngs-fg",
    textMuted = "--ngs-muted",
    panel = "--ngs-panel",
    accent = "--ngs-accent",
    danger = "--ngs-danger",
    border = "--ngs-border",
    input = "--ngs-input",
    button = "--ngs-button",
    buttonHover = "--ngs-button-hover",
    radius = "--ngs-radius",
    success = "--ngs-success",
    warning = "--ngs-warning",
    onAccent = "--ngs-on-accent",
}
R.SKIN = SKIN

local function css_value(fn, k, v)
    if IsColor(v) then return R.css_color(v) end
    if type(v) == "number" then return v .. "px" end
    if type(v) ~= "string" then error(fn .. ": '" .. tostring(k) .. "' must be a Color, number or string", 3) end
    local ok, err = pcall(R.css_check, v)
    if not ok then error(fn .. ": '" .. tostring(k) .. "': " .. err, 3) end
    return v
end

-- Canvases do not follow CSS variables, so every Paint panel has to redraw with the new colors.
local function skin_changed()
    for _, p in pairs(R.panels) do
        if p.Paint then p:InvalidatePaint() end
    end
    hook.Run("OnSkinChanged")
end

local skin_values = {}

function ui.SetSkin(t)
    for k, v in pairs(t) do
        local var = SKIN[k]
        if not var then error("ui.SetSkin: unknown key '" .. tostring(k) .. "'", 2) end
        N.css_var(var, css_value("ui.SetSkin", k, v))
        skin_values[k] = v
    end
    skin_changed()
end

--- What ui.SetSkin last set for the key; nil while the stylesheet default is in effect.
function ui.GetSkin(key) return skin_values[key] end

local vars = {}

local function var_name(fn, name)
    if type(name) ~= "string" or not name:match("^%a[%w%-]*$") then
        error(fn .. ": variable name must be letters, digits and '-', starting with a letter, got " .. tostring(name), 3)
    end
    return "--app-" .. name
end

--- Declares app CSS variables: ui.SetVars({ surface2 = Color(...), radiusCard = 12 }). false removes one.
--- Styles refer to them through ui.Var(name), so a theme switch is one SetVars call.
function ui.SetVars(t)
    for k, v in pairs(t) do
        local var = var_name("ui.SetVars", k)
        if v == false then
            N.css_var(var, "")
            vars[k] = nil
        else
            N.css_var(var, css_value("ui.SetVars", k, v))
            vars[k] = v
        end
    end
    skin_changed()
end

--- The value given to ui.SetVars (a Color stays a Color, so Paint can draw with it).
function ui.GetVar(name, default)
    local v = vars[name]
    if v == nil then return default end
    return v
end

--- "var(--app-<name>)" for SetStyle, SetBackgroundColor, state styles and gradients.
function ui.Var(name) return "var(" .. var_name("ui.Var", name) .. ")" end

local PANEL = { __native = { tag = "div", class = "ngs-panel" } }

function PANEL:IsValid() return not self.__removed end
function PANEL:GetClassName() return self.__class end
function PANEL:SetName(name) self.__name = name end
function PANEL:GetName() return self.__name or self.__class end

function PANEL:Add(class, props) return ui.Create(class, self, props) end

local function resolve_slot(parent, slot)
    if slot == false then return parent end
    local name = slot or "default"
    for _ = 1, 16 do
        local slots = rawget(parent, "__slots")
        local s = slots and slots[name]
        if s == nil or s == parent or s.__removed then
            if name ~= "default" then
                error("panel " .. parent.__class .. " has no slot '" .. tostring(name) .. "'", 4)
            end
            break
        end
        parent, name = s, "default"
    end
    return parent
end

--- Children added to this panel later go into `panel` instead. Without a name it is the
--- default slot; named ones are picked with ui.Create(..., { Slot = name }).
function PANEL:SetSlot(name, panel)
    if panel == nil and type(name) ~= "string" then name, panel = "default", name end
    local slots = rawget(self, "__slots") or {}
    self.__slots = slots
    slots[name] = panel
end

function PANEL:GetSlot(name)
    local slots = rawget(self, "__slots")
    local s = slots and slots[name or "default"]
    if s and not s.__removed then return s end
    return self
end

--- slot: name of the parent's slot, false to skip the parent's slots.
function PANEL:SetParent(parent, slot)
    parent = resolve_slot(parent or R.root, slot)
    local old = self.__parent
    if old == parent then return end
    if old then
        for i, c in ipairs(old.__children) do
            if c == self then
                table.remove(old.__children, i)
                break
            end
        end
        R.dock_dirty(old)
    end
    self.__parent = parent
    table.insert(parent.__children, self)
    N.append(parent.__id, self.__id)
    R.dock_dirty(parent)
    if parent.PerformLayout then parent:InvalidateLayout() end
end

function PANEL:GetParent() return self.__parent end

function PANEL:GetChildren()
    local out = {}
    for i, c in ipairs(self.__children) do out[i] = c end
    return out
end

function PANEL:GetChild(i) return self.__children[i] end
function PANEL:ChildCount() return #self.__children end

function PANEL:Clear()
    for i = #self.__children, 1, -1 do self.__children[i]:Remove() end
end

--- fn(self) runs when the panel is removed; the same name replaces the previous callback.
function PANEL:CallOnRemove(name, fn)
    local list = rawget(self, "__on_remove") or {}
    self.__on_remove = list
    list[name] = fn
end

function PANEL:RemoveCallOnRemove(name)
    local list = rawget(self, "__on_remove")
    if list then list[name] = nil end
end

local HOOKS, TIMERS = {}, {}
-- Panel-owned resources a Rebuild drops with the old Initialize: CallOnRemove key -> field.
R.scoped = { [HOOKS] = "__hooks", [TIMERS] = "__timers" }

--- hook.Add that lives as long as the panel. fn gets the hook arguments; a string calls that
--- method on the panel. One hook per event and name; the name is optional.
function PANEL:Hook(event, name, fn)
    if fn == nil then name, fn = "", name end
    if type(fn) ~= "function" and type(fn) ~= "string" then error("Hook: expected a function or a method name", 2) end
    local hooks = rawget(self, "__hooks")
    if not hooks then
        hooks = {}
        self.__hooks = hooks
        self:CallOnRemove(HOOKS, function()
            for _, id in pairs(hooks) do hook.Remove(id.event, id) end
        end)
    end
    local key = event .. "\0" .. tostring(name)
    local id = hooks[key] or { event = event }
    hooks[key] = id
    if type(fn) == "string" then
        local method = fn
        fn = function(...) return self[method](self, ...) end
    end
    hook.Add(event, id, fn)
end

function PANEL:Unhook(event, name)
    local hooks = rawget(self, "__hooks")
    local key = event .. "\0" .. tostring(name or "")
    local id = hooks and hooks[key]
    if id then
        hook.Remove(event, id)
        hooks[key] = nil
    end
end

local function timer_keys(self)
    local keys = rawget(self, "__timers")
    if not keys then
        keys = {}
        self.__timers = keys
        self:CallOnRemove(TIMERS, function()
            for _, k in pairs(keys) do timer.Remove(k) end
        end)
    end
    return keys
end

local function timer_fn(self, fn)
    if type(fn) == "string" then return function() return self[fn](self) end end
    return fn
end

--- timer.Create scoped to the panel: removed together with it. reps = 0 repeats forever.
function PANEL:Timer(name, delay, reps, fn)
    local keys = timer_keys(self)
    local k = keys[name] or {}
    keys[name] = k
    R.start_timer(k, delay, reps or 1, timer_fn(self, fn), self, function()
        if keys[name] == k then keys[name] = nil end
    end)
end

--- timer.Simple that never fires after the panel is gone.
function PANEL:TimerSimple(delay, fn)
    local keys = timer_keys(self)
    local k = {}
    keys[k] = k
    R.start_timer(k, delay, 1, timer_fn(self, fn), self, function() keys[k] = nil end)
end

function PANEL:RemoveTimer(name)
    local keys = rawget(self, "__timers")
    local k = keys and keys[name]
    if k then
        timer.Remove(k)
        keys[name] = nil
    end
end

function PANEL:TimerExists(name)
    local keys = rawget(self, "__timers")
    return keys ~= nil and keys[name] ~= nil and timer.Exists(keys[name])
end

local WEAK_KEYS = { __mode = "k" }

--- Makes `value` visible to Inject in every descendant. Changing it later calls
--- OnContextChanged(key, value) on the panels that injected it from here.
function PANEL:Provide(key, value)
    local ctx = rawget(self, "__provide") or {}
    self.__provide = ctx
    if ctx[key] == value then return end
    ctx[key] = value
    local subs = rawget(self, "__ctx_subs")
    local set = subs and subs[key]
    if not set then return end
    local list = {}
    for p in pairs(set) do list[#list + 1] = p end
    for _, p in ipairs(list) do
        if p.__removed then
            set[p] = nil
        elseif p.OnContextChanged then
            R.spawn_for(p, p.OnContextChanged, p, key, value)
        end
    end
end

--- Nearest ancestor's Provide(key, ...), then ui.Provide, then `default`.
function PANEL:Inject(key, default)
    local p = self.__parent
    while p do
        local ctx = rawget(p, "__provide")
        if ctx and ctx[key] ~= nil then
            local subs = rawget(p, "__ctx_subs") or {}
            p.__ctx_subs = subs
            subs[key] = subs[key] or setmetatable({}, WEAK_KEYS)
            subs[key][self] = true
            return ctx[key]
        end
        p = p.__parent
    end
    return default
end

function ui.Provide(key, value) R.root:Provide(key, value) end

function ui.Inject(key, default)
    local ctx = rawget(R.root, "__provide")
    local v = ctx and ctx[key]
    if v == nil then return default end
    return v
end

--- Removes the children and runs Initialize again (base-first), then reapplies setter props.
--- Whatever the previous Initialize left behind goes too: its waiting coroutines, panel hooks
--- and panel timers.
function PANEL:Rebuild()
    if self.__removed then return end
    self.__gen = (rawget(self, "__gen") or 0) + 1
    for i = #self.__children, 1, -1 do self.__children[i]:Remove() end
    local on_remove = rawget(self, "__on_remove")
    for key, field in pairs(R.scoped) do
        local fn = on_remove and on_remove[key]
        if fn then
            on_remove[key] = nil
            fn(self)
        end
        rawset(self, field, nil)
    end
    initialize(self, chain(self.__class))
end

function PANEL:Remove()
    if self.__removed then return end
    self.__removed = true
    for i = #self.__children, 1, -1 do self.__children[i]:Remove() end
    if self.OnRemove then R.pcall(self.OnRemove, self) end
    local on_remove = rawget(self, "__on_remove")
    if on_remove then
        for _, fn in pairs(on_remove) do R.pcall(fn, self) end
    end
    local parent = self.__parent
    if parent then
        for i, c in ipairs(parent.__children) do
            if c == self then
                table.remove(parent.__children, i)
                break
            end
        end
        R.dock_dirty(parent)
        if parent.PerformLayout then parent:InvalidateLayout() end
    end
    panels[self.__id] = nil
    thinkers[self] = nil
    painters[self] = nil
    N.remove(self.__id)
end

function PANEL:__apply_size()
    local d = self.__dock or 0
    local w, h = self.__w, self.__h
    if d == DOCK_FILL then
        w, h = nil, nil
    elseif d == DOCK_TOP or d == DOCK_BOTTOM then
        w = nil
    elseif d == DOCK_LEFT or d == DOCK_RIGHT then
        h = nil
    end
    N.style(self.__id, "width", len(w))
    N.style(self.__id, "height", len(h))
end

function PANEL:__apply_pos()
    local id = self.__id
    if self.__pos and (self.__dock or 0) == DOCK_NONE then
        -- Frames and menus live in the browser's top layer and are placed relative to the page.
        N.style(id, "position", self.__fixed and "fixed" or "absolute")
        N.style(id, "left", len(self.__pos[1]))
        N.style(id, "top", len(self.__pos[2]))
        N.style(id, "translate", self.__pos[3])
    else
        N.style(id, "position", nil)
        N.style(id, "left", nil)
        N.style(id, "top", nil)
        N.style(id, "translate", nil)
    end
end

function PANEL:Dock(d)
    self.__dock = d or DOCK_NONE
    self:__apply_size()
    self:__apply_pos()
    R.dock_dirty(self.__parent)
end

function PANEL:GetDock() return self.__dock or DOCK_NONE end

function PANEL:DockMargin(l, t, r, b) N.style(self.__id, "margin", box(l, t, r, b)) end
function PANEL:DockPadding(l, t, r, b) N.style(self.__id, "padding", box(l, t, r, b)) end

function PANEL:SetSize(w, h)
    self.__w, self.__h = w, h
    self:__apply_size()
end

function PANEL:SetWide(w)
    self.__w = w
    self:__apply_size()
end

function PANEL:SetTall(h)
    self.__h = h
    self:__apply_size()
end

PANEL.SetWidth = PANEL.SetWide
PANEL.SetHeight = PANEL.SetTall

function PANEL:SetMinSize(w, h)
    N.style(self.__id, "min-width", len(w))
    N.style(self.__id, "min-height", len(h))
end

function PANEL:SetMaxSize(w, h)
    N.style(self.__id, "max-width", len(w))
    N.style(self.__id, "max-height", len(h))
end

function PANEL:SizeToContents()
    self.__w, self.__h = "fit-content", "fit-content"
    self:__apply_size()
end

function PANEL:SizeToContentsX()
    self.__w = "fit-content"
    self:__apply_size()
end

function PANEL:SizeToContentsY()
    self.__h = "fit-content"
    self:__apply_size()
end

function PANEL:GetSize()
    R.flush_docks()
    return N.size(self.__id)
end

function PANEL:GetWide() return (self:GetSize()) end
function PANEL:GetTall() return select(2, self:GetSize()) end
PANEL.GetWidth = PANEL.GetWide
PANEL.GetHeight = PANEL.GetTall

function PANEL:SetPos(x, y)
    self.__pos = { x, y }
    self:__apply_pos()
    R.dock_dirty(self.__parent)
end

function PANEL:SetX(x) self:SetPos(x, self.__pos and self.__pos[2] or 0) end
function PANEL:SetY(y) self:SetPos(self.__pos and self.__pos[1] or 0, y) end

function PANEL:GetPos()
    R.flush_docks()
    return N.offset(self.__id)
end

function PANEL:GetX() return (self:GetPos()) end
function PANEL:GetY() return select(2, self:GetPos()) end

--- Position and size in page coordinates (the same space as gui.MousePos).
function PANEL:GetScreenBounds()
    R.flush_docks()
    return N.bounds(self.__id)
end

function PANEL:LocalToScreen(x, y)
    local px, py = self:GetScreenBounds()
    return px + (x or 0), py + (y or 0)
end

function PANEL:ScreenToLocal(x, y)
    local px, py = self:GetScreenBounds()
    return x - px, y - py
end

-- Whole pixels like GMod: panel bounds are fractional, so the raw difference would be too.
function PANEL:LocalCursorPos()
    local x, y = self:ScreenToLocal(N.mouse())
    return math.floor(x), math.floor(y)
end
PANEL.CursorPos = PANEL.LocalCursorPos

--- While captured, the panel keeps getting OnCursorMoved/OnMouseReleased even outside its bounds.
function PANEL:MouseCapture(b) N.capture(self.__id, b == true) end

function PANEL:Center()
    self.__pos = { "50%", "50%", "-50% -50%" }
    self:__apply_pos()
    R.dock_dirty(self.__parent)
end

function PANEL:SetLayout(mode)
    self.__layout = mode
    R.apply_flow(self)
end

function PANEL:SetGap(gap, column_gap)
    N.style(self.__id, "gap", column_gap and (len(gap) .. " " .. len(column_gap)) or len(gap))
end

function PANEL:SetAlign(a) N.style(self.__id, "align-items", a) end
function PANEL:SetJustify(j) N.style(self.__id, "justify-content", j) end
function PANEL:SetAlignSelf(a) N.style(self.__id, "align-self", a) end
function PANEL:SetLayoutWrap(b) N.style(self.__id, "flex-wrap", b and "wrap" or nil) end

--- Number: that many equal columns. Table: one entry per column, numbers are pixels,
--- strings are fractions or keywords ("1fr", "2fr", "auto").
function PANEL:SetColumns(cols)
    if type(cols) == "number" then
        self.__columns = "repeat(" .. cols .. ", minmax(0, 1fr))"
    else
        local parts = {}
        for i, c in ipairs(cols) do parts[i] = len(c) end
        self.__columns = table.concat(parts, " ")
    end
    if self.__layout ~= LAYOUT_GRID then self.__layout = LAYOUT_GRID end
    R.apply_flow(self)
end

--- Share of free space along the parent's row/column (flex-grow with zero basis).
function PANEL:SetGrow(n)
    N.style(self.__id, "flex", (n and n > 0) and (n .. " 1 0px") or nil)
end

function PANEL:SetVisible(b)
    self.__hidden = not b
    N.attr(self.__id, "hidden", (not b) and "" or nil)
end

function PANEL:IsVisible() return not self.__hidden end
function PANEL:Show() self:SetVisible(true) end
function PANEL:Hide() self:SetVisible(false) end

function PANEL:SetEnabled(b)
    self.__disabled = not b
    N.prop(self.__id, "disabled", not b)
    N.class(self.__id, "ngs-disabled", not b)
    self:InvalidatePaint()
end

function PANEL:IsEnabled() return not self.__disabled end
function PANEL:SetDisabled(b) self:SetEnabled(not b) end
function PANEL:GetDisabled() return self.__disabled == true end

function PANEL:SetTooltip(text) N.attr(self.__id, "title", text and tostring(text) or nil) end

local CURSORS = {
    arrow = "default", hand = "pointer", beam = "text", sizeall = "move", sizewe = "ew-resize",
    sizens = "ns-resize", sizenesw = "nesw-resize", sizenwse = "nwse-resize", no = "not-allowed",
    waiting = "wait", hourglass = "wait", crosshair = "crosshair", blank = "none", last = "auto",
}

function PANEL:SetCursor(name) N.style(self.__id, "cursor", CURSORS[name] or name) end

function PANEL:SetAlpha(a)
    self.__alpha = a
    N.style(self.__id, "opacity", tostring(a / 255))
end

function PANEL:GetAlpha() return self.__alpha or 255 end
function PANEL:SetZPos(z) N.style(self.__id, "z-index", tostring(z)) end

-- A string here is a CSS color such as ui.Var("surface"), checked like any SetStyle value.
local function color_css(c)
    if c == nil then return nil end
    if type(c) == "string" then return R.css_check(c) end
    return R.css_color(c)
end

function PANEL:SetBackgroundColor(c)
    self.__bg = c
    N.style(self.__id, "background-color", color_css(c))
end

function PANEL:GetBackgroundColor() return self.__bg end

function PANEL:SetPaintBackground(b)
    N.style(self.__id, "background", (b == false) and "none" or nil)
end

function PANEL:SetMouseInputEnabled(b) N.style(self.__id, "pointer-events", b and "auto" or "none") end

function PANEL:SetFont(name)
    self.__font = name
    N.style(self.__id, "font", R.font_css(name))
end

function PANEL:GetFont() return self.__font or "Default" end

function PANEL:SetTextColor(c)
    self.__fg = c
    N.style(self.__id, "color", color_css(c))
end

function PANEL:GetTextColor() return self.__fg end

local HALIGN = { "flex-start", "center", "flex-end" }
local TEXTALIGN = { "left", "center", "right" }

--- Numpad layout like GMod: 7 8 9 top, 4 5 6 middle, 1 2 3 bottom.
function PANEL:SetContentAlignment(n)
    local col = (n - 1) % 3 + 1
    local row = n >= 7 and "flex-start" or (n >= 4 and "center" or "flex-end")
    N.style(self.__id, "justify-content", HALIGN[col])
    N.style(self.__id, "align-items", row)
    N.style(self.__id, "text-align", TEXTALIGN[col])
end

function PANEL:IsHovered() return self.Hovered == true end
function PANEL:IsDown() return self.Depressed == true end

function PANEL:IsChildHovered()
    for _, c in ipairs(self.__children) do
        if c.Hovered or c:IsChildHovered() then return true end
    end
    return false
end

function PANEL:HasFocus() return N.has_focus(self.__id) end
function PANEL:RequestFocus() N.focus(self.__id) end
function PANEL:KillFocus() N.blur(self.__id) end

function PANEL:InvalidateLayout(now)
    if now then
        if self.PerformLayout then R.pcall(self.PerformLayout, self, self:GetSize()) end
        return
    end
    layout_q[self] = true
    schedule()
end

function PANEL:InvalidateParent()
    if self.__parent then self.__parent:InvalidateLayout() end
end

function PANEL:InvalidateChildren(recursive)
    for _, c in ipairs(self.__children) do
        c:InvalidateLayout()
        if recursive then c:InvalidateChildren(true) end
    end
end

-- Until the observer reports the first size there is nothing to paint yet; that report paints anyway.
function PANEL:InvalidatePaint()
    if self.Paint and not self.__removed and self.__sized then
        paint_q[self] = true
        schedule()
    end
end

function PANEL:SetPaintEveryFrame(b)
    painters[self] = b and true or nil
    if b then R.wake_think() end
end

function PANEL:ScrollToChild(child) N.scroll_into_view(child.__id) end

--- SetTransition's "all ..." entry goes first so the per-property ones from AlphaTo/MoveTo win.
function R.apply_transition(self)
    local parts = {}
    if self.__transition then parts[1] = self.__transition end
    for _, t in pairs(self.__transitions or {}) do parts[#parts + 1] = t end
    N.style(self.__id, "transition", #parts > 0 and table.concat(parts, ", ") or nil)
end

-- Animations are CSS transitions; GMod's callback fires when the transition should be over.
local function animate(self, prop, values, duration, delay, callback)
    self.__transitions = self.__transitions or {}
    self.__transitions[prop] = string.format("%s %gs ease %gs", prop, duration or 0.2, delay or 0)
    R.apply_transition(self)
    for k, v in pairs(values) do N.style(self.__id, k, v) end
    if callback then
        self:TimerSimple((duration or 0.2) + (delay or 0), function() callback(self) end)
    end
end

function PANEL:AlphaTo(alpha, duration, delay, callback)
    self.__alpha = alpha
    animate(self, "opacity", { opacity = tostring(alpha / 255) }, duration, delay, callback)
end

function PANEL:MoveTo(x, y, duration, delay, _, callback)
    self.__pos = { x, y }
    N.style(self.__id, "position", "absolute")
    animate(self, "left", { left = len(x) }, duration, delay)
    animate(self, "top", { top = len(y) }, duration, delay, callback)
end

function PANEL:SizeTo(w, h, duration, delay, _, callback)
    if w and w >= 0 then self.__w = w end
    if h and h >= 0 then self.__h = h end
    animate(self, "width", { width = len(self.__w) }, duration, delay)
    animate(self, "height", { height = len(self.__h) }, duration, delay, callback)
end

local function call(self, method, ...)
    local fn = self[method]
    if fn then return R.spawn_for(self, fn, self, ...) end
end

function PANEL:__on_down(code)
    self.Depressed = true
    self:InvalidatePaint()
    call(self, "OnMousePressed", code)
end

function PANEL:__on_up(code)
    self.Depressed = false
    self:InvalidatePaint()
    call(self, "OnMouseReleased", code)
end

function PANEL:__on_click()
    if self.__disabled then return end
    call(self, "DoClick")
end

function PANEL:__on_rclick()
    if self.DoRightClick and not self.__disabled then
        call(self, "DoRightClick")
        return true
    end
end

function PANEL:__on_dblclick()
    if not self.__disabled then call(self, "DoDoubleClick") end
end

function PANEL:__on_enter()
    self.Hovered = true
    self:InvalidatePaint()
    call(self, "OnCursorEntered")
end

function PANEL:__on_exit()
    self.Hovered = false
    self:InvalidatePaint()
    call(self, "OnCursorExited")
end

function PANEL:__on_move()
    if self.OnCursorMoved then call(self, "OnCursorMoved", self:LocalCursorPos()) end
end

function PANEL:__on_focus()
    self:InvalidatePaint()
    call(self, "OnFocusChanged", true)
end

function PANEL:__on_blur()
    self:InvalidatePaint()
    call(self, "OnFocusChanged", false)
end

--- Return true from OnKeyCodePressed to cancel the browser default for that key.
function PANEL:__on_key(key)
    if self.OnKeyCodePressed then
        local done, handled = R.spawn_for(self, self.OnKeyCodePressed, self, key)
        return done and handled == true
    end
end

ui.Register("Panel", PANEL)
