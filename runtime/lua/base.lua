---@diagnostic disable: duplicate-set-field
local N, R = ...

local function join(...)
    local parts = {}
    for i = 1, select("#", ...) do
        parts[i] = tostring((select(i, ...)))
    end
    return table.concat(parts, "\t")
end

function print(...)
    N.log(join(...))
end

function ErrorNoHalt(...)
    N.error(join(...))
end

function PrintTable(t, indent, seen)
    indent = indent or 0
    seen = seen or {}
    seen[t] = true
    local keys = {}
    for k in pairs(t) do keys[#keys + 1] = k end
    table.sort(keys, function(a, b) return tostring(a) < tostring(b) end)
    local pad = string.rep("\t", indent)
    for _, k in ipairs(keys) do
        local v = t[k]
        if type(v) == "table" and not seen[v] then
            print(pad .. tostring(k) .. ":")
            PrintTable(v, indent + 1, seen)
        else
            print(pad .. tostring(k) .. "\t=\t" .. tostring(v))
        end
    end
end

-- Handlers run as coroutines so they can block on http.Get and friends.
-- A wait is `coroutine.yield(AWAIT, key)`; the marker keeps it apart from user coroutines.
-- `key` is a native op id or any table used as a future (R.op_done resolves both).
local AWAIT = {}
local waiting = {}
local owners = setmetatable({}, { __mode = "k" })

local function report(co, err)
    N.error(debug.traceback(co, tostring(err)))
end

local function park(key, w)
    local list = waiting[key]
    if list then
        list[#list + 1] = w
    else
        waiting[key] = { w }
    end
end

local function step(co, owner, ...)
    local res = table.pack(coroutine.resume(co, ...))
    if not res[1] then
        report(co, res[2])
        return false
    end
    if coroutine.status(co) == "dead" then
        return true, table.unpack(res, 2, res.n)
    end
    if res[2] == AWAIT then
        park(res[3], { co = co, owner = owner, gen = owner and rawget(owner, "__gen") })
    end
    return false
end

local function spawn(owner, fn, ...)
    local co = coroutine.create(fn)
    if owner then owners[co] = owner end
    return step(co, owner, ...)
end

--- Runs fn in a new coroutine. Returns true plus fn's results if it finished without waiting.
function R.spawn(fn, ...)
    return spawn(nil, fn, ...)
end

--- Same, but if `owner` (a panel) is removed or rebuilt while fn waits, fn is never resumed:
--- a page that awaits http.Get in Initialize won't touch its children after navigating away.
function R.spawn_for(owner, fn, ...)
    return spawn(owner, fn, ...)
end

--- The panel whose handler is running right now, if any.
function R.owner()
    local co = coroutine.running()
    return co and owners[co]
end

function R.await(key, level)
    if not coroutine.isyieldable() then
        error("this call waits for a result, so it only works inside handlers and Initialize; use the callback variant here", level or 3)
    end
    return coroutine.yield(AWAIT, key)
end

--- Callback-style alternative to R.await for code running outside a coroutine.
function R.on_op(key, fn)
    park(key, fn)
end

local function stale(w)
    local o = w.owner
    return o ~= nil and (o.__removed or rawget(o, "__gen") ~= w.gen)
end

function R.op_done(key, ...)
    local list = waiting[key]
    if not list then return end
    waiting[key] = nil
    for _, w in ipairs(list) do
        if type(w) == "function" then
            R.spawn(w, ...)
        elseif not stale(w) then
            step(w.co, w.owner, ...)
        end
    end
end

local function traceback(err)
    return debug.traceback(tostring(err), 2)
end

--- Synchronous protected call for Paint/PerformLayout/Think: errors are reported, not raised.
function R.pcall(fn, ...)
    local res = table.pack(xpcall(fn, traceback, ...))
    if not res[1] then
        N.error(res[2])
    end
    return table.unpack(res, 1, res.n)
end

function CurTime() return N.now() end
RealTime = CurTime
SysTime = CurTime

R.frame_time = 0
function FrameTime() return R.frame_time end

gui = {}

--- Cursor position in page (viewport) coordinates.
function gui.MousePos() return N.mouse() end
function gui.MouseX() return (N.mouse()) end
function gui.MouseY() return select(2, N.mouse()) end
function gui.OpenURL(url)
    url = tostring(url)
    if not R.check_url(url, "link", "gui.OpenURL") then return false end
    N.open(url)
    return true
end

local LINK_SCHEMES = { http = true, https = true, mailto = true }
local SRC_SCHEMES = { http = true, https = true, data = true, blob = true }

-- Same steps as the WHATWG URL parser before it reads the scheme: leading/trailing control
-- chars and spaces are dropped, tabs and newlines are removed anywhere. Without this
-- " javascript:" or "java\tscript:" would pass as relative URLs and still run on click.
local function normalize_url(url)
    return (url:gsub("^[%c ]+", ""):gsub("[%c ]+$", ""):gsub("[\t\n\r]", ""))
end

--- Lowercase scheme of an absolute URL, nil for a relative one.
function R.url_scheme(url)
    local scheme = normalize_url(url):match("^(%a[%w+.%-]*):")
    return scheme and scheme:lower()
end

--- kind "link": http, https, mailto and relative URLs; "src" (images, media): http, https, data, blob, relative.
function R.safe_url(url, kind)
    if type(url) ~= "string" then return false end
    local scheme = R.url_scheme(url)
    if scheme == nil then return true end
    return (kind == "src" and SRC_SCHEMES or LINK_SCHEMES)[scheme] == true
end

function R.check_url(url, kind, where)
    if R.safe_url(url, kind) then return true end
    N.error(string.format("%s: blocked URL with scheme '%s'", where, tostring(R.url_scheme(tostring(url)))))
    return false
end

--- Relative URLs (but not "//host") and absolute ones on this origin.
function R.same_origin(url)
    local s = normalize_url(url)
    if R.url_scheme(s) == nil then return s:match("^[/\\][/\\]") == nil end
    local origin = N.origin():lower()
    local rest = s:sub(#origin + 1, #origin + 1)
    return s:sub(1, #origin):lower() == origin and (rest == "" or rest:match("[/?#]") ~= nil)
end

function ScrW() return (N.screen()) end
function ScrH() return select(2, N.screen()) end

function Lerp(t, a, b) return a + (b - a) * t end

function math.Clamp(v, lo, hi) return math.min(math.max(v, lo), hi) end

function math.Round(v, decimals)
    local m = 10 ^ (decimals or 0)
    return math.floor(v * m + 0.5) / m
end

function math.Remap(v, a1, b1, a2, b2) return a2 + (v - a1) * (b2 - a2) / (b1 - a1) end

function string.Trim(s) return (s:gsub("^%s+", ""):gsub("%s+$", "")) end
function string.StartsWith(s, prefix) return s:sub(1, #prefix) == prefix end
function string.EndsWith(s, suffix) return suffix == "" or s:sub(-#suffix) == suffix end

function string.Split(s, sep)
    local out = {}
    local start = 1
    while true do
        local i, j = s:find(sep, start, true)
        if not i then
            out[#out + 1] = s:sub(start)
            return out
        end
        out[#out + 1] = s:sub(start, i - 1)
        start = j + 1
    end
end

function table.Count(t)
    local n = 0
    for _ in pairs(t) do n = n + 1 end
    return n
end

function table.HasValue(t, value)
    for _, v in pairs(t) do
        if v == value then return true end
    end
    return false
end

function table.IsEmpty(t) return next(t) == nil end

function table.Copy(t, seen)
    seen = seen or {}
    if seen[t] then return seen[t] end
    local out = {}
    seen[t] = out
    for k, v in pairs(t) do
        out[k] = type(v) == "table" and table.Copy(v, seen) or v
    end
    return setmetatable(out, getmetatable(t))
end

local COLOR = {}
COLOR.__index = COLOR

function Color(r, g, b, a)
    return setmetatable({ r = r or 255, g = g or 255, b = b or 255, a = a or 255 }, COLOR)
end

function IsColor(v) return getmetatable(v) == COLOR end

function COLOR:__tostring() return string.format("%d %d %d %d", self.r, self.g, self.b, self.a) end
function COLOR.__eq(a, b) return a.r == b.r and a.g == b.g and a.b == b.b and a.a == b.a end
function COLOR:Copy() return Color(self.r, self.g, self.b, self.a) end
function COLOR:Unpack() return self.r, self.g, self.b, self.a end

color_white = Color(255, 255, 255)
color_black = Color(0, 0, 0)
color_transparent = Color(0, 0, 0, 0)

function R.css_color(c)
    return string.format("rgba(%d,%d,%d,%.3f)", c.r, c.g, c.b, (c.a or 255) / 255)
end

util = {}
function util.TableToJSON(t, pretty) return N.json_encode(t, pretty) end

function util.JSONToTable(s)
    local ok, v = pcall(N.json_decode, s)
    if ok then return v end
    return nil
end

hook = {}
local hooks = {}

function hook.Add(event, id, fn)
    hooks[event] = hooks[event] or {}
    hooks[event][id] = fn
    if event == "Think" then R.wake_think() end
end

function hook.Remove(event, id)
    if hooks[event] then hooks[event][id] = nil end
end

--- Like GMod's hook.Run: the first hook returning a non-nil value stops the chain.
function hook.Run(event, ...)
    local list = hooks[event]
    if not list then return end
    for _, fn in pairs(list) do
        local a, b, c = fn(...)
        if a ~= nil then return a, b, c end
    end
end

function hook.GetTable() return hooks end

function R.has_think_hooks()
    return hooks.Think ~= nil and next(hooks.Think) ~= nil
end

timer = {}
local timers = {}

function timer.Simple(delay, fn)
    N.timeout(delay * 1000, function() R.spawn(fn) end)
end

--- `key` may be any value; panels use private tables so their timers never clash with named ones.
--- `owner` drops waits of a removed panel, `done` runs once the last repetition fired.
function R.start_timer(key, delay, reps, fn, owner, done)
    timer.Remove(key)
    local t = { delay = delay, left = reps > 0 and reps or math.huge }
    timers[key] = t
    local function tick()
        if timers[key] ~= t then return end
        t.left = t.left - 1
        if t.left <= 0 then
            timers[key] = nil
            if done then done() end
        end
        R.spawn_for(owner, fn)
        if timers[key] == t then
            t.handle = N.timeout(t.delay * 1000, tick)
        end
    end
    t.handle = N.timeout(delay * 1000, tick)
end

--- reps = 0 repeats forever.
function timer.Create(name, delay, reps, fn)
    R.start_timer(name, delay, reps, fn)
end

function timer.Remove(name)
    local t = timers[name]
    if t then
        N.clear_timeout(t.handle)
        timers[name] = nil
    end
end

function timer.Exists(name) return timers[name] ~= nil end

function timer.Adjust(name, delay)
    local t = timers[name]
    if t then t.delay = delay end
end
