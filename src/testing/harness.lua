---@diagnostic disable: lowercase-global
-- Test driver loaded between the prelude and lockdown.lua. It owns the fake page loader and the
-- `t` helpers handed to every `test(name, function(t) ... end)`.
local N, R, ctx = ...
local M, files, compile = ctx.mock, ctx.files, ctx.compile

local traceback = debug.traceback
local cases, order = {}, {}
local loads = {}
local booted, shown = false, false
local served = {}

function N.load_page(file)
    M.next_op = M.next_op + 1
    M.chunkOp = M.next_op
    loads[#loads + 1] = { op = M.next_op, file = file }
    return M.next_op
end

-- What the browser would do between tasks: deliver loaded modules, run the scheduled flush and
-- answer requests that a t.Serve route covers. Repeats until nothing changes.
local function settle()
    for _ = 1, 1000 do
        local progress = false
        if #loads > 0 then
            local batch = loads
            loads = {}
            for _, l in ipairs(batch) do
                local fn, err = compile(l.file)
                R.op_done(l.op, fn, err)
            end
            progress = true
        end
        for i, f in ipairs(M.fetches) do
            if not f.answered and not M.aborted[f.op] then
                for _, route in ipairs(served) do
                    local params = route.match(f)
                    if params then
                        local status, body, ctype = route.answer(f, params)
                        TEST.answer(i, status, body, ctype)
                        progress = true
                        break
                    end
                end
            end
        end
        if M.flush_clipboard() then progress = true end
        if M.scheduled then
            M.scheduled = false
            R.flush()
            progress = true
        end
        if not progress then return end
    end
    error("the app keeps scheduling work: settle gave up after 1000 rounds", 3)
end

TEST = {}

function TEST.boot()
    if booted then return end
    booted = true
    R.boot(files, true, true)
    settle()
end

function TEST.names() return order end

function TEST.run(name)
    local fn = cases[name]
    if not fn then return false, "no test named " .. name end
    local ok, err = xpcall(fn, function(e) return traceback(tostring(e), 2) end, TEST.t)
    if ok then
        ok, err = xpcall(settle, function(e) return traceback(tostring(e), 2) end)
    end
    return ok, err
end

function TEST.answer(i, status, body, ctype)
    local f = M.fetches[i]
    f.answered = true
    if type(body) == "table" then
        body, ctype = N.json_encode(body), ctype or "application/json"
    elseif body == nil then
        body = ""
    end
    R.op_done(f.op, status or 200, tostring(body), ctype or "text/plain", nil, false)
end

--- Registers a test case. The body runs in a fresh runtime with the app's autorun already done.
function test(name, fn)
    assert(type(name) == "string" and type(fn) == "function", "usage: test(name, function(t) ... end)")
    if cases[name] then error("duplicate test name: " .. name, 2) end
    cases[name] = fn
    order[#order + 1] = name
end

local t = {}
TEST.t = t

local function describe(v)
    if type(v) == "string" then return string.format("%q", v) end
    if type(v) ~= "table" or ui.IsPanel(v) then return tostring(v) end
    local ok, s = pcall(N.json_encode, v)
    return ok and s or tostring(v)
end

local function deep_equal(a, b)
    if a == b then return true end
    if type(a) ~= "table" or type(b) ~= "table" or ui.IsPanel(a) or ui.IsPanel(b) then return false end
    for k, v in pairs(a) do
        if not deep_equal(v, b[k]) then return false end
    end
    for k in pairs(b) do
        if a[k] == nil then return false end
    end
    return true
end

--- Fails unless the values are equal; tables are compared by content.
function t.Eq(actual, expected, message)
    if not deep_equal(actual, expected) then
        error(string.format("%sexpected %s, got %s", message and message .. ": " or "", describe(expected), describe(actual)), 2)
    end
end

--- Opens a page. The first call renders the app for the first time, later ones are router.Go.
function t.Go(url)
    if shown then
        router.Go(url)
    else
        shown = true
        N.replace_state(url)
        R.render(N.location())
    end
    settle()
    return R.page
end

function t.Page() return R.page end
function t.Path() return M.path, M.query end

--- Runs pending work (module loads, flushes, served requests) without doing anything else.
t.Settle = settle

local function isa(p, class)
    local name = p.__class
    while name do
        if name == class then return true end
        local c = ui.GetClass(name)
        name = c and rawget(c, "Base")
    end
    return false
end

local function text_of(p)
    if p.GetText then
        local ok, s = pcall(p.GetText, p)
        if ok and type(s) == "string" then return s end
    end
end

-- Queries: "Button" (class, bases included), "#name" (SetName), a table of fields that must all
-- hold ({ Class = "Button", Text = "Save", Name = "email", Contains = "Sav" }) or a predicate.
local function matcher(q)
    if type(q) == "function" then return q end
    if type(q) == "string" then
        local name = q:match("^#(.+)$")
        if name then return function(p) return rawget(p, "__name") == name end end
        return function(p) return isa(p, q) end
    end
    assert(type(q) == "table", "query must be a class name, '#name', a table or a function")
    return function(p)
        if q.Class and not isa(p, q.Class) then return false end
        if q.Name and rawget(p, "__name") ~= q.Name then return false end
        if q.Text and text_of(p) ~= q.Text then return false end
        if q.Contains and not (text_of(p) or ""):find(q.Contains, 1, true) then return false end
        if q.Visible ~= nil and (p:IsVisible() ~= q.Visible) then return false end
        return true
    end
end

local function walk(p, fn)
    for _, c in ipairs(p.__children) do
        if not c.__removed then
            if fn(c) then return true end
            if walk(c, fn) then return true end
        end
    end
end

function t.FindAll(query, root)
    local match, out = matcher(query), {}
    walk(root or R.root, function(p)
        if match(p) then out[#out + 1] = p end
    end)
    return out
end

function t.Find(query, root)
    local match, found = matcher(query), nil
    walk(root or R.root, function(p)
        if match(p) then found = p; return true end
    end)
    return found
end

--- Like Find, but a missing panel fails the test and lists the texts on screen.
function t.Get(query, root)
    local p = t.Find(query, root)
    if not p then
        error(string.format("no panel matches %s; texts on screen:\n%s", describe(query), t.Text(root)), 2)
    end
    return p
end

--- Every non-empty text under root (labels, buttons, links), one per line.
function t.Text(root)
    local out = {}
    walk(root or R.root, function(p)
        local s = text_of(p)
        if s and s ~= "" then out[#out + 1] = s end
    end)
    return table.concat(out, "\n")
end

function t.HasText(s, root)
    return t.Text(root):find(s, 1, true) ~= nil
end

local function panel(p, what)
    if not ui.IsPanel(p) then p = t.Get(p) end
    if p.__removed then error(what .. ": the panel was removed", 3) end
    return p
end

--- A user click: nothing happens on a disabled panel, like in the browser.
function t.Click(p)
    p = panel(p, "Click")
    R.event(p.__id, "click", true)
    settle()
end

--- Replaces the text of a TextEntry as if typed, firing OnChange / OnValueChange / bindings.
function t.Type(p, text)
    p = panel(p, "Type")
    p:SetValue(text)
    R.event(p.__id, "input")
    settle()
end

--- Presses a key on a panel ("Enter", "Escape", ...).
function t.Press(p, key)
    p = panel(p, "Press")
    R.event(p.__id, "key", key)
    settle()
end

--- Picks a ComboBox option by text or 1-based index.
function t.Select(p, option)
    p = panel(p, "Select")
    if type(option) == "number" then p:ChooseOptionID(option) else p:ChooseOption(option) end
    R.event(p.__id, "change")
    settle()
end

function t.Check(p, on)
    p = panel(p, "Check")
    p:SetChecked(on ~= false)
    R.event(p.__id, "change")
    settle()
end

--- Moves the fake clock, firing timers (SetTimeout, Timer, debounce, request timeouts).
function t.Advance(seconds)
    M.advance(seconds)
    settle()
end

local function split_pattern(pattern)
    local method, rest = pattern:match("^(%u+)%s+(.+)$")
    return method, rest or pattern
end

local function path_of(url)
    url = url:gsub("^https?://[^/]+", "")
    return url:match("^[^?#]*"), url:match("%?([^#]*)") or ""
end

-- "GET /api/users/{id}" or "/api/users/1": the method is optional, the query string is ignored
-- unless the pattern has one.
local function route_matcher(pattern)
    if type(pattern) == "function" then return pattern end
    local method, url = split_pattern(pattern)
    local want_path, want_query = path_of(url)
    local segs = {}
    for s in want_path:gmatch("[^/]+") do segs[#segs + 1] = s end
    return function(f)
        if method and f.method ~= method then return nil end
        local path, query = path_of(f.url)
        if url:find("?", 1, true) and query ~= want_query then return nil end
        local parts = {}
        for s in path:gmatch("[^/]+") do parts[#parts + 1] = s end
        local params = {}
        for i, s in ipairs(segs) do
            local wild = s:match("^{%*(.+)}$")
            if wild then
                params[wild] = table.concat(parts, "/", i)
                return params
            end
            local name = s:match("^{(.+)}$")
            if parts[i] == nil then return nil end
            if name then params[name] = parts[i] elseif parts[i] ~= s then return nil end
        end
        if #parts ~= #segs then return nil end
        return params
    end
end

local function request_view(f)
    local view = { method = f.method, url = f.url, body = f.body, headers = f.headers or {} }
    if type(f.body) == "string" then
        local ok, v = pcall(N.json_decode, f.body)
        if ok then view.json = v end
    end
    view.query = {}
    for pair in (select(2, path_of(f.url))):gmatch("[^&]+") do
        local k, v = pair:match("^([^=]*)=?(.*)$")
        view.query[k] = v
    end
    return view
end

--- A fake backend route, answered automatically from now on. `answer` is a value sent as JSON
--- with 200, or function(req, params) returning body or status, body.
function t.Serve(pattern, answer)
    local match = route_matcher(pattern)
    served[#served + 1] = {
        match = match,
        answer = function(f, params)
            if type(answer) ~= "function" then return 200, answer end
            local a, b, c = answer(request_view(f), params)
            if type(a) == "number" then return a, b, c end
            return 200, a
        end,
    }
    settle()
end

--- Requests made so far (answered or not) that match the pattern; all of them without one.
function t.Requests(pattern)
    local match = pattern and route_matcher(pattern)
    local out = {}
    for _, f in ipairs(M.fetches) do
        if not match or match(f) then out[#out + 1] = request_view(f) end
    end
    return out
end

local function pending_index(pattern)
    local match = route_matcher(pattern)
    for i, f in ipairs(M.fetches) do
        if not f.answered and not M.aborted[f.op] and match(f) then return i end
    end
    local list = {}
    for _, f in ipairs(M.fetches) do
        if not f.answered and not M.aborted[f.op] then list[#list + 1] = f.method .. " " .. f.url end
    end
    error(string.format("no pending request matches %s; pending: %s", describe(pattern),
        #list > 0 and table.concat(list, ", ") or "none"), 3)
end

--- The oldest unanswered request matching the pattern; fails the test if there is none.
function t.Request(pattern)
    return request_view(M.fetches[pending_index(pattern)])
end

--- Answers the oldest unanswered matching request: t.Respond("/api/me", { id = 1 }) or
--- t.Respond("POST /api/login", 401, "wrong password").
function t.Respond(pattern, status, body, ctype)
    if type(status) ~= "number" then status, body, ctype = 200, status, body end
    TEST.answer(pending_index(pattern), status, body, ctype)
    settle()
end

--- Fails the oldest matching request as a network error.
function t.Fail(pattern, message)
    local i = pending_index(pattern)
    local f = M.fetches[i]
    f.answered = true
    R.op_done(f.op, 0, "", "", message or "network error", false)
    settle()
end

--- Runtime errors reported so far (they would show up on screen in dev). Taking them keeps the
--- test from failing on them.
function t.Errors() return M.take_errors() end

--- The newest WebSocket / EventSource opened to a URL matching the pattern (see t.Request):
--- { url, sent = {...}, closed, Emit = function(event, text) } where event is "open", "message",
--- "error" or "close".
function t.Stream(pattern)
    local match = route_matcher(pattern)
    local found
    for _, s in pairs(M.streams) do
        if match({ method = "GET", url = s.url }) and (not found or s.id > found.id) then found = s end
    end
    if not found then error("no stream matches " .. describe(pattern), 2) end
    return setmetatable({
        Emit = function(event, text)
            R.stream_event(found.id, event, text)
            settle()
        end,
    }, { __index = found })
end

--- Text written with clipboard.WriteText (and what clipboard.ReadText will return).
function t.Clipboard() return M.clipboard end

--- localStorage / sessionStorage contents as stored strings.
function t.Storage(session) return M.storage[session == true] end

t.MOCK = M
