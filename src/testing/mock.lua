-- Browser stand-in for the runtime prelude: returns the N table the prelude expects and MOCK,
-- which tests use to inspect "DOM" state, drive the clock and answer fetches.
local M = {
    els = {}, rules = {}, errors = {}, opened = {}, fetches = {}, aborted = {}, timers = {},
    storage = { [false] = {}, [true] = {} },
    now = 0, next_id = 0, next_timer = 0, next_op = 0,
    origin = "http://localhost:3000", path = "/", query = "",
}
local N = {}

local function el(id)
    local e = M.els[id]
    if not e then error("panel " .. tostring(id) .. " has been removed") end
    return e
end

local function target(e) return e.control or e end

function N.create(tag, class, control, ty)
    M.next_id = M.next_id + 1
    local e = { id = M.next_id, tag = tag, classes = {}, attrs = {}, props = {}, style = {} }
    if control then e.control = { tag = control, attrs = { type = ty }, props = {} } end
    for c in ("ngs " .. class):gmatch("%S+") do e.classes[c] = true end
    M.els[e.id] = e
    return e.id
end

function N.set_control(id, tag, ty)
    local e = el(id)
    e.control = { tag = tag, attrs = { type = ty }, props = { value = e.control and e.control.props.value } }
end

function N.mount(id) el(id).mounted = true end
function N.append(parent, child) el(child).parent = el(parent).id end
function N.remove(id) M.els[id] = nil end

function N.style(id, prop, value)
    if value == "" then value = nil end
    el(id).style[prop] = value
end

function N.attr(id, name, value, on_control)
    local e = el(id)
    if on_control then e = target(e) end
    e.attrs[name] = value
end

function N.get_attr(id, name, on_control)
    local e = el(id)
    if on_control then e = target(e) end
    return e.attrs[name]
end

function N.prop(id, name, value) target(el(id)).props[name] = value end
function N.get_prop(id, name) return target(el(id)).props[name] end
function N.class(id, name, on) el(id).classes[name] = on or nil end
function N.text(id, s) el(id).text = s end
function N.size(id) el(id) return 100, 50 end
function N.offset(id) el(id) return 0, 0 end
function N.bounds(id) el(id) return 0, 0, 100, 50 end

function N.invoke(id, method)
    local e = target(el(id))
    e.invoked = e.invoked or {}
    e.invoked[#e.invoked + 1] = method
end

function N.add_option(id, text)
    local t = target(el(id))
    t.options = t.options or {}
    t.options[#t.options + 1] = text
end

function N.clear_options(id) target(el(id)).options = {} end
function N.css_var(name, value) M.vars = M.vars or {}; M.vars[name] = value end
function N.css_rule(selector, decls) M.rules[#M.rules + 1] = { selector = selector, decls = decls } end

for _, name in ipairs({
    "capture", "focus", "blur", "scroll_into_view", "observe", "canvas_begin", "canvas_end", "color",
    "text_color", "font", "alpha", "rect", "outlined", "line", "rounded", "circle", "poly",
    "gradient", "line_width", "line_cap", "line_join", "line_dash", "arc", "path",
    "load_font", "image", "image_uv", "think", "log", "back",
}) do
    N[name] = function() end
end

function N.has_focus() return false end
function N.measure(s) return #s * 7, 14 end
-- Like the canvas native: draws nothing here, but returns the width the text pen advances by.
function N.draw_text(s) return #s * 7 end
function N.image_load() return 1 end
function N.image_size() return 0, 0 end
function N.now() return M.now end
function N.mouse() return 0, 0 end
function N.screen() return 1280, 720 end
function N.schedule() M.scheduled = true end

function N.timeout(ms, fn)
    M.next_timer = M.next_timer + 1
    M.timers[M.next_timer] = { at = M.now + ms / 1000, fn = fn }
    return M.next_timer
end

function N.clear_timeout(h) M.timers[h] = nil end

function N.fetch(method, url, body, headers)
    M.next_op = M.next_op + 1
    M.fetches[#M.fetches + 1] = { op = M.next_op, method = method, url = url, body = body, headers = headers }
    return M.next_op
end

function N.files(id) return el(id).files or {} end
function N.upload(id, url, field, headers)
    if #N.files(id) == 0 then error("no files selected") end
    local op = N.fetch("POST", url, nil, headers)
    M.fetches[#M.fetches].field = field
    return op
end

function N.load_page(file)
    M.next_op = M.next_op + 1
    M.chunkOp = M.next_op
    return M.next_op
end

M.cookies = {}
function N.set_cookie(s) M.cookies[#M.cookies + 1] = s end
function N.cookies() return "" end

function N.abort(op) M.aborted[op] = true end

M.meta, M.dark, M.clipboard, M.clip_ops, M.streams = {}, false, nil, {}, {}
function N.meta(name, content) M.meta[name] = content end
function N.dark() return M.dark end

-- Browser wall clock: M.clock at page start plus the fake page time. M.tz is seconds east of UTC
-- or a function(unix) for zones with DST.
M.clock, M.tz, M.focused, M.visible = 1795718460, 0, true, true
function N.clock() return M.clock + M.now end
function N.tz_offset(unix)
    if type(M.tz) == "function" then return M.tz(unix) end
    return M.tz
end
function N.focused() return M.focused end
function N.visible() return M.visible end

function N.clipboard(text)
    M.next_op = M.next_op + 1
    M.clip_ops[#M.clip_ops + 1] = { op = M.next_op, text = text }
    return M.next_op
end

--- Settles clipboard reads and writes started so far.
function M.flush_clipboard()
    local ops = M.clip_ops
    M.clip_ops = {}
    for _, c in ipairs(ops) do
        if c.text ~= nil then M.clipboard = c.text end
        M.R.op_done(c.op, c.text or M.clipboard or "")
    end
    return #ops > 0
end

function N.connect(url, sse)
    M.next_op = M.next_op + 1
    M.streams[M.next_op] = { id = M.next_op, url = url, sse = sse, sent = {}, closed = false }
    return M.next_op
end

function N.send(id, text)
    local s = M.streams[id]
    if not s or s.closed or s.sse then error("expected an open WebSocket") end
    s.sent[#s.sent + 1] = text
end

function N.disconnect(id)
    if M.streams[id] then M.streams[id].closed = true end
end
function N.error(s) M.errors[#M.errors + 1] = s end
function N.location() return M.path, M.query end
function N.origin() return M.origin end

local function change_url(url)
    M.path = url:match("^[^?#]*")
    M.query = url:match("%?([^#]*)") or ""
end

function N.resolve_url(url)
    if url == "" then url = M.path .. (M.query ~= "" and "?" .. M.query or "") end
    return url:match("^[^?#]*"), url:match("%?([^#]*)") or "", url
end
M.historyIndex = 0
N.replace_state = change_url
function N.push_state(url)
    M.historyIndex = M.historyIndex + 1
    change_url(url)
end
function N.history_index() return M.historyIndex end
function N.history_go(delta) M.historyDelta = delta end
function N.document_title() return M.title or "" end
function N.title(s) M.title = s end
function N.open(url) M.opened[#M.opened + 1] = url end

function N.storage_get(key, session) return M.storage[session][key] end

function N.storage_set(key, value, session)
    if M.storage_broken then return false end
    M.storage[session][key] = value
    return true
end

function N.storage_remove(key, session)
    if M.storage_broken then return false end
    M.storage[session][key] = nil
    return true
end

function N.storage_keys(session)
    local out = {}
    for k in pairs(M.storage[session]) do out[#out + 1] = k end
    return out
end

local function encode(v)
    local t = type(v)
    if t == "nil" then return "null" end
    if t == "boolean" then return tostring(v) end
    if t == "number" then
        if math.type(v) == "float" and v == math.floor(v) then return string.format("%d", v) end
        return tostring(v)
    end
    if t == "string" then
        return '"' .. v:gsub('[%c"\\]', function(c) return string.format("\\u%04x", c:byte()) end) .. '"'
    end
    if t == "table" then
        local parts = {}
        if next(v) == nil or v[1] ~= nil then
            for i, x in ipairs(v) do parts[i] = encode(x) end
            return "[" .. table.concat(parts, ",") .. "]"
        end
        local keys = {}
        for k in pairs(v) do keys[#keys + 1] = k end
        table.sort(keys)
        for i, k in ipairs(keys) do parts[i] = encode(tostring(k)) .. ":" .. encode(v[k]) end
        return "{" .. table.concat(parts, ",") .. "}"
    end
    error("cannot encode " .. t)
end

local function decode(s)
    local pos = 1
    local function ws() pos = s:find("[^ \t\r\n]", pos) or #s + 1 end
    local function str()
        local out = {}
        pos = pos + 1
        while true do
            local c = s:sub(pos, pos)
            if c == '"' then
                pos = pos + 1
                return table.concat(out)
            elseif c == "\\" then
                local n = s:sub(pos + 1, pos + 1)
                if n == "u" then
                    out[#out + 1] = utf8.char(tonumber(s:sub(pos + 2, pos + 5), 16))
                    pos = pos + 6
                else
                    out[#out + 1] = ({ n = "\n", t = "\t", r = "\r" })[n] or n
                    pos = pos + 2
                end
            else
                out[#out + 1] = c
                pos = pos + 1
            end
        end
    end
    local value
    function value()
        ws()
        local c = s:sub(pos, pos)
        if c == "{" or c == "[" then
            local t, close = {}, c == "{" and "}" or "]"
            pos = pos + 1
            ws()
            if s:sub(pos, pos) == close then
                pos = pos + 1
                return t
            end
            while true do
                if close == "}" then
                    ws()
                    local k = str()
                    ws()
                    pos = pos + 1
                    t[k] = value()
                else
                    t[#t + 1] = value()
                end
                ws()
                local d = s:sub(pos, pos)
                pos = pos + 1
                if d == close then return t end
            end
        elseif c == '"' then
            return str()
        end
        for word, v in pairs({ ["true"] = true, ["false"] = false }) do
            if s:sub(pos, pos + #word - 1) == word then
                pos = pos + #word
                return v
            end
        end
        if s:sub(pos, pos + 3) == "null" then
            pos = pos + 4
            return nil
        end
        local num = s:match("^-?[%d.eE+-]+", pos)
        if not num then error("bad json at " .. pos .. ": " .. s) end
        pos = pos + #num
        return math.tointeger(tonumber(num)) or tonumber(num)
    end
    return value()
end

function N.json_encode(v) return encode(v) end
function N.json_decode(s) return decode(s) end

function M.el(panel) return M.els[panel.__id] end

function M.take_errors()
    local e = M.errors
    M.errors = {}
    return e
end

--- Moves the clock forward, firing due timeouts in order.
function M.advance(dt)
    local stop = M.now + dt
    while true do
        local best, handle
        for h, t in pairs(M.timers) do
            if t.at <= stop and (best == nil or t.at < best.at or (t.at == best.at and h < handle)) then
                best, handle = t, h
            end
        end
        if not best then break end
        M.timers[handle] = nil
        M.now = best.at
        best.fn()
    end
    M.now = stop
end

--- Answers fetch number i (1-based, in call order).
function M.respond(i, status, text, ctype, shell)
    local f = M.fetches[i]
    M.R.op_done(f.op, status, text, ctype or "text/plain", nil, shell == true)
end

function M.json(i, value, status)
    M.respond(i, status or 200, encode(value), "application/json")
end

return N, M
