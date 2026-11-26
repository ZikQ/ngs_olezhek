---@diagnostic disable: duplicate-set-field, inject-field
local N, R = ...

http = {}

local default_headers = {}
local default_timeout = 30

local Request = {}
Request.__index = Request

local function encode_body(body)
    if type(body) == "table" then return N.json_encode(body), "application/json" end
    if body ~= nil then return tostring(body), "text/plain;charset=utf-8" end
end

-- JSON responses come back as tables, anything else as a string.
-- Non-2xx and network failures give nil plus { status, message, body, fields? }.
local function result(status, body, ctype, neterr, shell)
    if neterr then return nil, { status = 0, message = neterr } end
    -- The server's SPA fallback answered: the endpoint does not exist.
    if shell then return nil, { status = 404, message = "no such endpoint", body = "" } end
    local value = body
    if ctype and ctype:find("json", 1, true) and body ~= "" then
        local ok, v = pcall(N.json_decode, body)
        if ok then value = v end
    end
    if status >= 200 and status < 300 then return value end
    local err = { status = status, message = type(value) == "string" and value or ("HTTP " .. status), body = value }
    -- FieldErrors from the server: { message?, errors = { field = { "msg", ... } } }.
    if type(value) == "table" then
        if type(value.message) == "string" then err.message = value.message end
        if type(value.errors) == "table" then
            err.fields = value.errors
            if type(value.message) ~= "string" then
                local lines = {}
                for name, list in pairs(value.errors) do
                    lines[#lines + 1] = name .. ": " .. tostring(type(list) == "table" and list[1] or list)
                end
                table.sort(lines)
                err.message = table.concat(lines, "\n")
            end
        end
    end
    return nil, err
end

local function header_table(url, ctype, extra)
    local merged = {}
    local function put(name, value)
        name = tostring(name)
        if value == nil or value == false then
            merged[name:lower()] = nil
        else
            merged[name:lower()] = { name, tostring(value) }
        end
    end
    if ctype then put("Content-Type", ctype) end
    -- Defaults usually carry credentials (Authorization, CSRF token): never send them to another origin.
    if R.same_origin(url) then
        for k, h in pairs(default_headers) do merged[k] = h end
    end
    if extra then
        for k, v in pairs(extra) do put(k, v) end
    end
    local out = {}
    for _, h in pairs(merged) do out[h[1]] = h[2] end
    return out
end

local function start(method, url, body, opts, owner, input, field)
    opts = opts or {}
    url = tostring(url)
    local payload, ctype = encode_body(body)
    local req = setmetatable({ method = method, url = url, done = false, invalidate = opts.invalidate }, Request)
    local headers = header_table(url, ctype, opts.headers)
    if input then
        for name in pairs(headers) do
            if name:lower() == "content-type" then headers[name] = nil end
        end
        req.op = N.upload(input.__id, url, field or "file", headers)
    else
        req.op = N.fetch(method, url, payload, headers)
    end
    R.on_op(req.op, function(status, text, rctype, neterr, shell)
        req.status, req.text = status, text
        req:__finish(result(status, text, rctype, neterr, shell))
    end)
    local timeout = opts.timeout
    if timeout == nil then timeout = default_timeout end
    if timeout and timeout > 0 then
        req.timer = N.timeout(timeout * 1000, function()
            req:__abort("timeout", string.format("timed out after %gs", timeout))
        end)
    end
    if owner then
        req.owner = owner
        owner:CallOnRemove(req, function() req:Cancel() end)
    end
    return req
end

function Request:__finish(value, err)
    if self.done then return end
    self.done = true
    if self.timer then N.clear_timeout(self.timer) end
    if self.owner then self.owner:RemoveCallOnRemove(self) end
    self.value, self.err = value, err
    if not err and self.invalidate then http.Invalidate(self.invalidate) end
    -- One place to send the user to a login page when the server's Auth<T> says no.
    if err and err.status == 401 and R.same_origin(self.url) then hook.Run("Unauthorized", self.url, err) end
    R.op_done(self, value, err)
end

function Request:__abort(kind, message)
    if self.done then return end
    N.abort(self.op)
    self:__finish(nil, { status = 0, message = message, [kind] = true })
end

--- The waiting code gets nil, { status = 0, message = "cancelled", cancelled = true }.
function Request:Cancel() self:__abort("cancelled", "cancelled") end
function Request:IsDone() return self.done end

--- Waits for the result (inside handlers and Initialize); returns value, err like http.Get.
function Request:Wait()
    if self.done then return self.value, self.err end
    return R.await(self)
end

local function wait(req)
    if not coroutine.isyieldable() then
        error("this call waits for a result, so it only works inside handlers and Initialize; use http.Fetch or http.Start here", 2)
    end
    return req:Wait()
end

--- Starts a request without waiting. opts: headers, timeout (seconds, 0 = none), invalidate.
--- Not tied to a panel: cancel it yourself if it outlives the caller.
function http.Start(method, url, body, opts)
    return start(method, url, body, opts)
end

--- Waits for the response: local user, err = http.Get("/api/users/1")
--- Works inside handlers (DoClick, OnChange...) and Initialize; use http.Fetch elsewhere.
--- The request is cancelled if the panel running the handler is removed meanwhile.
function http.Request(method, url, body, opts)
    return wait(start(method, url, body, opts, R.owner()))
end

function http.Get(url, opts) return wait(start("GET", url, nil, opts, R.owner())) end
function http.Post(url, body, opts) return wait(start("POST", url, body, opts, R.owner())) end
function http.Put(url, body, opts) return wait(start("PUT", url, body, opts, R.owner())) end
function http.Patch(url, body, opts) return wait(start("PATCH", url, body, opts, R.owner())) end
function http.Delete(url, body, opts) return wait(start("DELETE", url, body, opts, R.owner())) end

--- Upload the current selection as multipart/form-data. Shares timeout, cancellation and response handling.
function http.Upload(url, input, field, opts)
    assert(coroutine.isyieldable(), "use http.Upload in a handler")
    if not IsValid(input) then return nil, {status = 0, message = "expected a file input"} end
    local ok, req = pcall(start, "POST", url, nil, opts, R.owner(), input, field)
    if not ok then return nil, {status = 0, message = tostring(req)} end
    return req:Wait()
end

--- Callback style, usable anywhere: onSuccess(value), onFailure(message, err). Returns the request.
function http.Fetch(url, onSuccess, onFailure, method, body, opts)
    local req = start(method or "GET", url, body, opts)
    R.on_op(req, function(value, err)
        if err then
            if onFailure then onFailure(err.message, err) end
        elseif onSuccess then
            onSuccess(value)
        end
    end)
    return req
end

--- Sent with every request to this origin; nil removes it. http.SetHeader("Authorization", "Bearer " .. token)
function http.SetHeader(name, value)
    name = tostring(name)
    default_headers[name:lower()] = value ~= nil and { name, tostring(value) } or nil
end

--- Default timeout in seconds for requests without opts.timeout; 0 or nil disables it.
function http.SetTimeout(seconds) default_timeout = seconds end

function http.Encode(s)
    return (tostring(s):gsub("[^%w%-_%.~]", function(c) return string.format("%%%02X", c:byte()) end))
end

-- Query cache: one entry per key (the URL unless opts.key is given).
-- Requests made within DEDUPE seconds of each other share one response.
local DEDUPE = 2
local KEEP = 300
local cache = {}
local last_sweep = 0

local function sweep(now)
    if now - last_sweep < 60 then return end
    last_sweep = now
    for key, e in pairs(cache) do
        local live = false
        for p in pairs(e.subs) do
            if p.__removed then e.subs[p] = nil else live = true end
        end
        if not live and not e.inflight and (e.time == nil or now - e.time > KEEP) then cache[key] = nil end
    end
end

local function notify(key, e)
    local list = {}
    for p in pairs(e.subs) do list[#list + 1] = p end
    for _, p in ipairs(list) do
        if p.__removed then
            e.subs[p] = nil
        elseif p.OnQueryUpdate then
            R.spawn_for(p, p.OnQueryUpdate, p, key, e.value)
        else
            p:Rebuild()
        end
    end
end

local function revalidate(key, e)
    e.seq = (e.seq or 0) + 1
    local seq = e.seq
    local req = start("GET", e.url, nil, e.opts)
    e.inflight = req
    R.on_op(req, function(value, err)
        if e.seq ~= seq then return end
        e.inflight = nil
        if err then return end
        local had = e.loaded
        local changed = e.text ~= req.text
        e.value, e.text, e.time, e.loaded = value, req.text, N.now(), true
        if had and changed then notify(key, e) end
    end)
    return req
end

local function query(url, opts, sub)
    opts = opts or {}
    url = tostring(url)
    local key = opts.key or url
    local now = N.now()
    sweep(now)
    local e = cache[key]
    if not e then
        e = { subs = setmetatable({}, { __mode = "k" }) }
        cache[key] = e
    end
    e.url, e.opts = url, opts
    if sub then e.subs[sub] = true end
    if e.time and now - e.time < math.max(opts.ttl or 0, DEDUPE) then return e.value end
    if e.time then
        -- Stale: answer with what we have, refresh in the background and notify on change.
        if not e.inflight then revalidate(key, e) end
        return e.value
    end
    return wait(e.inflight or revalidate(key, e))
end

--- Cached GET: concurrent calls share one request, a fresh answer (younger than opts.ttl seconds,
--- at least 2) comes from the cache, a stale one is returned at once and refreshed in the background.
--- The returned table is shared with other callers; copy it before changing it.
function http.Query(url, opts) return query(url, opts) end

--- Peeks into the query cache without a request.
function http.GetData(key)
    local e = cache[key]
    return e and e.value
end

--- Puts `value` into the cache (optimistic update) and notifies the panels subscribed to `key`.
function http.SetData(key, value)
    local e = cache[key]
    if not e then
        e = { subs = setmetatable({}, { __mode = "k" }), url = key, opts = {} }
        cache[key] = e
    end
    e.seq = (e.seq or 0) + 1
    e.inflight = nil
    e.value, e.text, e.time, e.loaded = value, N.json_encode(value), N.now(), true
    notify(key, e)
end

local function matches(key, prefix)
    if key == prefix then return true end
    if key:sub(1, #prefix) ~= prefix then return false end
    local c = key:sub(#prefix + 1, #prefix + 1)
    return prefix:sub(-1) == "/" or c == "/" or c == "?"
end

--- Marks cached queries outdated: "/api/users" matches "/api/users", "/api/users/1", "/api/users?q=a",
--- but not "/api/usersettings". No argument matches everything. Queries that panels are subscribed
--- to are fetched again at once, the rest are dropped.
function http.Invalidate(prefix)
    if type(prefix) == "table" then
        for _, p in ipairs(prefix) do http.Invalidate(p) end
        return
    end
    local hit = {}
    for key in pairs(cache) do
        if prefix == nil or matches(key, prefix) then hit[#hit + 1] = key end
    end
    for _, key in ipairs(hit) do
        local e = cache[key]
        local live = false
        for p in pairs(e.subs) do
            if not p.__removed then live = true end
        end
        if live then
            e.time = nil
            revalidate(key, e)
        else
            cache[key] = nil
        end
    end
end

local Panel = ui.GetClass("Panel")

--- http.Query that subscribes the panel: when the data changes later (background refresh,
--- http.Invalidate, http.SetData) it calls self:OnQueryUpdate(key, value) or, without that
--- method, self:Rebuild().
function Panel:UseQuery(url, opts)
    return query(url, opts, self)
end
