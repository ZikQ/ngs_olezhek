---@diagnostic disable: duplicate-set-field, inject-field, undefined-field
local N, R = ...

sandbox = {}

-- Runs inside the sandbox VM before any plugin code and turns the bare __host bridge into
-- print, api, ui, timer and Color. Panels there are proxies holding a handle; every method
-- call goes back to the host, which checks it.
local SHIM = [==[
local host = __host
__host = nil
local type, select, error, pairs, rawget, rawset, setmetatable, getmetatable, tostring, tonumber =
    type, select, error, pairs, rawget, rawset, setmetatable, getmetatable, tostring, tonumber
local pack, unpack, concat = table.pack, table.unpack, table.concat

local function raise(level, ok, ...)
    if not ok then error((...), level + 1) end
    return ...
end

function print(...)
    local parts = {}
    for i = 1, select("#", ...) do parts[i] = tostring((select(i, ...))) end
    raise(2, host("print", concat(parts, "\t")))
end

api = setmetatable({}, {
    __index = function(_, name)
        return function(...) return raise(2, host("api", name, ...)) end
    end,
    __newindex = function() error("api is read-only", 2) end,
})

function Color(r, g, b, a)
    return { __color = true, r = r or 255, g = g or 255, b = b or 255, a = a or 255 }
end

local PROXY = {}
local proxies, handlers, methods = {}, {}, {}

local function wrap(h)
    local p = proxies[h]
    if not p then
        p = setmetatable({ __h = h }, PROXY)
        proxies[h] = p
    end
    return p
end

local function import(v, depth)
    if type(v) ~= "table" then return v end
    if v.__h then return wrap(v.__h) end
    if depth < 16 then
        for k, x in pairs(v) do v[k] = import(x, depth + 1) end
    end
    return v
end

local function export(v, depth)
    if type(v) ~= "table" then return v end
    if getmetatable(v) == PROXY then return { __h = rawget(v, "__h") } end
    if depth >= 16 then error("table nested too deeply", 0) end
    local out = {}
    for k, x in pairs(v) do out[k] = export(x, depth + 1) end
    return out
end

local function map_all(f, ...)
    local t = pack(...)
    for i = 1, t.n do t[i] = f(t[i], 0) end
    return unpack(t, 1, t.n)
end

local function method(name)
    local m = methods[name]
    if not m then
        m = function(self, ...)
            if getmetatable(self) ~= PROXY then error("call " .. name .. " with ':'", 2) end
            return map_all(import, raise(2, host("call", rawget(self, "__h"), name, map_all(export, ...))))
        end
        methods[name] = m
    end
    return m
end

local function is_handler(k)
    return type(k) == "string" and (k:match("^On%u") or k:match("^Do%u")) ~= nil
end

PROXY.__index = function(self, k)
    local hs = handlers[rawget(self, "__h")]
    if hs and hs[k] then return hs[k] end
    if type(k) == "string" and k:match("^%u") then return method(k) end
end

PROXY.__newindex = function(self, k, v)
    local h = rawget(self, "__h")
    if is_handler(k) and (v == nil or type(v) == "function") then
        handlers[h] = handlers[h] or {}
        handlers[h][k] = v
        raise(2, host(v and "listen" or "unlisten", h, k))
    else
        rawset(self, k, v)
    end
end

PROXY.__tostring = function(self) return "Panel [" .. tostring(rawget(self, "__h")) .. "]" end

ui = {}

function ui.Create(class, parent, props)
    local ph
    if parent ~= nil then
        if getmetatable(parent) ~= PROXY then error("ui.Create: parent must be a panel", 2) end
        ph = rawget(parent, "__h")
    end
    local data, fns = {}, {}
    if props ~= nil then
        if type(props) ~= "table" then error("ui.Create: props must be a table", 2) end
        for k, v in pairs(props) do
            if type(k) == "string" then
                if type(v) == "function" then fns[k] = v else data[k] = export(v, 0) end
            end
        end
    end
    local p = wrap(raise(2, host("create", class, ph, data)))
    for k, fn in pairs(fns) do p[k] = fn end
    return p
end

function ui.Root() return wrap(raise(2, host("root"))) end
function ui.IsPanel(v) return getmetatable(v) == PROXY end

function IsValid(v)
    return getmetatable(v) == PROXY and raise(2, host("valid", rawget(v, "__h"))) == true
end

timer = {}
local timers, next_timer = {}, 0

function timer.Simple(delay, fn)
    if type(fn) ~= "function" then error("timer.Simple: expected a function", 2) end
    next_timer = next_timer + 1
    timers[next_timer] = fn
    raise(2, host("timer", tonumber(delay) or 0, next_timer))
end

function __ngs_dispatch(kind, a, b, ...)
    if kind == "event" then
        local hs = handlers[a]
        local fn = hs and hs[b]
        if fn then
            local r = fn(wrap(a), map_all(import, ...))
            if type(r) == "boolean" then return r end
        end
    elseif kind == "timer" then
        local fn = timers[a]
        timers[a] = nil
        if fn then fn() end
    elseif kind == "removed" then
        proxies[a], handlers[a] = nil, nil
    end
end
]==]

local function set_of(list)
    local t = {}
    for _, v in ipairs(list) do t[v] = true end
    return t
end

local DEFAULT_CLASSES = {
    "Panel", "Label", "Button", "TextEntry", "CheckBox", "ComboBox", "Slider", "NumberWang", "ProgressBar",
    "Row", "Column", "Grid", "ScrollPanel",
}
local EXTRA_METHODS = set_of({
    "Dock", "DockMargin", "DockPadding", "Center", "Clear", "Remove", "Show", "Hide", "Toggle", "AddChoice",
    "ChooseOption", "ChooseOptionID", "RequestFocus", "KillFocus", "SizeToContents", "SizeToContentsX",
    "SizeToContentsY", "InvalidateLayout", "ChildCount", "MoveTo", "SizeTo", "AlphaTo", "ScrollToChild",
})
-- Methods that take panels or functions, or cost a frame every frame.
local DENIED = set_of({ "SetParent", "SetSlot", "SetOutlet", "SetRichTooltip", "SetPaintEveryFrame", "SetDataSource" })
-- The container belongs to the host: a plugin may read it and empty it, nothing else.
local ROOT_METHODS = set_of({
    "GetSize", "GetWide", "GetTall", "GetWidth", "GetHeight", "GetChildren", "GetChild", "GetParent", "ChildCount",
    "IsValid", "IsVisible", "Clear",
})
local PLAIN_PROPS = { Dock = true, DockMargin = true, DockPadding = true }
local MAX_OPS = 10000

local function allowed(method)
    if type(method) ~= "string" or DENIED[method] then return false end
    return EXTRA_METHODS[method] or method:match("^Set%u") or method:match("^Get%u") or method:match("^Is%u")
        or method:match("^Has%u")
end

local function is_handler(name)
    return type(name) == "string" and name ~= "OnContextChanged"
        and (name:match("^On%u%a*$") or name:match("^Do%u%a*$")) ~= nil
end

local function export(box, v, depth)
    local t = type(v)
    if t == "table" then
        if ui.IsPanel(v) then
            local h = box.__ids[v]
            return h and { __h = h } or nil
        end
        if IsColor(v) then return { __color = true, r = v.r, g = v.g, b = v.b, a = v.a } end
        if ui.IsState(v) then return export(box, v:Peek(), depth) end
        if depth >= 16 then return nil end
        local out = {}
        for k, x in next, v do
            local kt = type(k)
            if kt == "string" or kt == "number" or kt == "boolean" then out[k] = export(box, x, depth + 1) end
        end
        return out
    elseif t == "function" or t == "thread" or t == "userdata" then
        return nil
    end
    return v
end

-- strict: a handle that is not ours is an error (arguments); otherwise it becomes nil (results).
local function import(box, v, depth, strict)
    if type(v) ~= "table" then return v end
    if v.__h ~= nil then
        local p = box.__handles[v.__h]
        if not p and strict then error("not a panel of this sandbox", 0) end
        return p
    end
    if v.__color then
        return Color(tonumber(v.r) or 255, tonumber(v.g) or 255, tonumber(v.b) or 255, tonumber(v.a) or 255)
    end
    if depth < 16 then
        for k, x in next, v do v[k] = import(box, x, depth + 1, strict) end
    end
    return v
end

local function map_all(box, f, strict, ...)
    local t = table.pack(...)
    for i = 1, t.n do t[i] = f(box, t[i], 0, strict) end
    return table.unpack(t, 1, t.n)
end

local Box = {}
Box.__index = Box

local function own(box, panel)
    local h = box.__next
    box.__next = h + 1
    box.__count = box.__count + 1
    box.__handles[h] = panel
    box.__ids[panel] = h
    panel:CallOnRemove(box, function()
        box.__handles[h] = nil
        box.__ids[panel] = nil
        box.__count = box.__count - 1
        if not box.__closed then box:__dispatch("removed", h) end
    end)
    return h
end

local function invoke(box, h, p, method, ...)
    if not allowed(method) or (h == 1 and not ROOT_METHODS[method]) then
        error(tostring(p.__class) .. ":" .. tostring(method) .. " is not available in the sandbox", 0)
    end
    local fn = p[method]
    if type(fn) ~= "function" then error(tostring(p.__class) .. " has no method " .. method, 0) end
    return map_all(box, export, nil, fn(p, map_all(box, import, true, ...)))
end

local OPS = {}

function OPS.print(box, text) box.__print(tostring(text)) end

function OPS.api(box, name, ...)
    local fn = box.__api[name]
    if type(fn) ~= "function" then error("no api function '" .. tostring(name) .. "'", 0) end
    return map_all(box, export, nil, fn(map_all(box, import, true, ...)))
end

function OPS.root(box)
    if not box.__root then error("this sandbox has no ui root", 0) end
    return 1
end

function OPS.valid(box, h)
    local p = box.__handles[h]
    return p ~= nil and IsValid(p)
end

function OPS.create(box, class, parent_h, props)
    if not box.__root then error("this sandbox has no ui root", 0) end
    if type(class) ~= "string" or not box.__classes[class] then
        error("class '" .. tostring(class) .. "' is not available in the sandbox", 0)
    end
    local parent = box.__root
    if parent_h ~= nil then
        parent = box.__handles[parent_h]
        if not parent then error("ui.Create: parent is not a panel of this sandbox", 0) end
    end
    if box.__count >= box.__max_panels then error("too many panels (limit " .. box.__max_panels .. ")", 0) end
    local p = ui.Create(class, parent)
    local h = own(box, p)
    for k, v in pairs(props or {}) do
        local m = PLAIN_PROPS[k] and k or "Set" .. tostring(k)
        if R.UNPACK[k] and type(v) == "table" then
            invoke(box, h, p, m, table.unpack(v))
        else
            invoke(box, h, p, m, v)
        end
    end
    return h
end

function OPS.call(box, h, method, ...)
    local p = box.__handles[h]
    if not p then error("the panel has been removed", 0) end
    return invoke(box, h, p, method, ...)
end

function OPS.listen(box, h, name)
    local p = box.__handles[h]
    if not p or h == 1 then error("cannot listen to this panel", 0) end
    if not is_handler(name) then error("'" .. tostring(name) .. "' is not an event", 0) end
    p[name] = function(_, ...) return box:__event(h, name, ...) end
end

function OPS.unlisten(box, h, name)
    local p = box.__handles[h]
    if p and h ~= 1 and is_handler(name) then p[name] = nil end
end

function OPS.timer(box, delay, id)
    if box.__timers >= 64 then error("too many pending timers", 0) end
    box.__timers = box.__timers + 1
    local function fire()
        box.__timers = box.__timers - 1
        if not box.__closed then box:__dispatch("timer", id) end
    end
    delay = math.max(tonumber(delay) or 0, 0)
    if box.__root then box.__root:TimerSimple(delay, fire) else timer.Simple(delay, fire) end
end

local function bridge(box)
    return function(op, ...)
        if box.__closed then return false, "sandbox is closed" end
        box.__ops = box.__ops + 1
        if box.__ops > MAX_OPS then return false, "too many host calls in one run" end
        local fn = OPS[op]
        if not fn then return false, "unknown host call" end
        return pcall(fn, box, ...)
    end
end

--- Untrusted Lua in a VM of its own. opts:
---   root         panel the plugin builds its UI in (dedicated, empty); without it there is no ui
---   api          { name = function } callable from the plugin as api.name(...); data in, data out
---   classes      panel classes the plugin may create (default: basic controls and layouts)
---   instructions budget per Run, Call and event (default 2 000 000)
---   memory       byte cap for the VM (default 8 MiB)
---   max_panels   default 500
---   on_error     function(message) for errors in events and timers (default: reported)
---   on_print     function(text) for the plugin's print
function sandbox.New(opts)
    opts = opts or {}
    if type(opts) ~= "table" then error("sandbox.New: opts must be a table", 2) end
    local box = setmetatable({
        __name = tostring(opts.name or "sandbox"),
        __api = {},
        __handles = {},
        __ids = setmetatable({}, { __mode = "k" }),
        __next = 2,
        __count = 0,
        __timers = 0,
        __ops = 0,
        __depth = 0,
        __max_panels = opts.max_panels or 500,
        __classes = set_of(opts.classes or DEFAULT_CLASSES),
    }, Box)
    box.__print = opts.on_print or function(text) print("[" .. box.__name .. "] " .. text) end
    box.__on_error = opts.on_error or function(msg) N.error("[" .. box.__name .. "] " .. msg) end
    for k, fn in pairs(opts.api or {}) do box:Expose(k, fn) end
    box.__id = N.sandbox_new(opts.instructions or 2000000, opts.memory or 8 * 1024 * 1024, bridge(box))
    if opts.root ~= nil then
        local root = opts.root
        if not ui.IsPanel(root) then error("sandbox.New: root must be a panel", 2) end
        box.__root = root
        box.__handles[1] = root
        box.__ids[root] = 1
        -- Keeps absolutely positioned and fixed children (and their z-index) inside the box,
        -- so a plugin cannot draw over the app's own UI.
        for prop, v in pairs({ position = "relative", overflow = "hidden", isolation = "isolate", contain = "layout paint" }) do
            N.style(root.__id, prop, v)
        end
        root:CallOnRemove(box, function() box:Close() end)
    end
    local ok, err = box:__enter(N.sandbox_run, box.__id, SHIM, "ngs/sandbox")
    if not ok then
        box:Close()
        error("sandbox.New: " .. tostring(err), 2)
    end
    return box
end

function Box:__enter(fn, ...)
    if self.__closed then return false, "sandbox is closed" end
    if self.__depth == 0 then self.__ops = 0 end
    self.__depth = self.__depth + 1
    local res = table.pack(pcall(fn, ...))
    self.__depth = self.__depth - 1
    if not res[1] then return false, tostring(res[2]) end
    return table.unpack(res, 2, res.n)
end

--- Runs plugin source (text, never bytecode). Returns true, or false and the message.
function Box:Run(source, name)
    if type(source) ~= "string" then error("Run: source must be a string", 2) end
    return self:__enter(N.sandbox_run, self.__id, source, tostring(name or self.__name))
end

--- Calls a global function the plugin defined: true plus its results, or false and the message.
function Box:Call(name, ...)
    if type(name) ~= "string" then error("Call: name must be a string", 2) end
    local res = table.pack(self:__enter(N.sandbox_call, self.__id, name, map_all(self, export, nil, ...)))
    if not res[1] then return false, res[2] end
    return true, map_all(self, import, false, table.unpack(res, 2, res.n))
end

function Box:Expose(name, fn)
    if type(name) ~= "string" or type(fn) ~= "function" then error("Expose: expected a name and a function", 2) end
    self.__api[name] = fn
end

function Box:__dispatch(kind, ...)
    local ok, err = self:__enter(N.sandbox_call, self.__id, "__ngs_dispatch", kind, ...)
    if not ok then self.__on_error(tostring(err)) end
end

function Box:__event(h, name, ...)
    local ok, r = self:__enter(N.sandbox_call, self.__id, "__ngs_dispatch", "event", h, name, map_all(self, export, nil, ...))
    if not ok then
        self.__on_error(tostring(r))
        return
    end
    return r
end

--- Removes the plugin's panels and frees the VM. Also happens when the root is removed.
function Box:Close()
    if self.__closed then return end
    self.__closed = true
    for h, p in pairs(self.__handles) do
        if h ~= 1 and not p.__removed then p:Remove() end
    end
    if self.__root then self.__root:RemoveCallOnRemove(self) end
    N.sandbox_close(self.__id)
end

function Box:IsClosed() return self.__closed == true end
