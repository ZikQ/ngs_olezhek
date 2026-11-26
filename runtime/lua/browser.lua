---@diagnostic disable: duplicate-set-field, inject-field
local N, R = ...

cookie = {}
function cookie.Get(name, default)
    local key = http.Encode(name)
    for pair in N.cookies():gmatch("[^;]+") do
        local k, v = pair:match("^%s*([^=]+)=(.*)$")
        if k == key then
            return (v:gsub("%%(%x%x)", function(h) return string.char(tonumber(h, 16)) end))
        end
    end
    return default
end

--- First-party cookie scoped to the application root. maxAge is in seconds.
function cookie.Set(name, value, maxAge)
    assert(type(name) == "string" and name ~= "", "expected a cookie name")
    if value == nil then return cookie.Remove(name) end
    local attrs = "; Path=/; SameSite=Lax"
    if N.origin():match("^https:") then attrs = attrs .. "; Secure" end
    if maxAge ~= nil then attrs = attrs .. "; Max-Age=" .. string.format("%d", math.floor(maxAge)) end
    N.set_cookie(http.Encode(name) .. "=" .. http.Encode(value) .. attrs)
end
function cookie.Remove(name) cookie.Set(name, "", 0) end

clipboard = {}
local function clip(text)
    assert(coroutine.isyieldable(), "use clipboard in a handler")
    local ok, op = pcall(N.clipboard, text)
    if not ok then return nil, tostring(op) end
    return R.await(op)
end
function clipboard.ReadText() return clip(nil) end
function clipboard.WriteText(text)
    local result, err = clip(tostring(text))
    return result ~= nil, err
end

browser = {}
function browser.ColorScheme() return N.dark() and "dark" or "light" end
function R.scheme_changed(dark) hook.Run("OnColorSchemeChanged", dark and "dark" or "light") end
function browser.SetMeta(name, content) N.meta(name, content) end

--- Unix seconds from the browser clock, with milliseconds as the fraction.
function browser.Now() return N.clock() end
--- Seconds east of UTC in the browser's time zone at that moment (default: now).
function browser.TimezoneOffset(unix) return math.floor(N.tz_offset(unix or N.clock()) + 0.5) end
function browser.HasFocus() return N.focused() end
function browser.IsVisible() return N.visible() end
function R.window_focus(on) hook.Run("OnWindowFocusChanged", on) end
function R.visibility_changed(visible) hook.Run("OnVisibilityChanged", visible) end

local FileInput = { __native = { tag = "div", class = "ngs-file", control = "input", type = "file" } }
function FileInput:SetAccept(types) N.attr(self.__id, "accept", types, true) end
function FileInput:SetMultiple(on) N.prop(self.__id, "multiple", on == true) end
function FileInput:GetFiles() return N.files(self.__id) end
function FileInput:Clear() N.prop(self.__id, "value", "") end
function FileInput:__on_change()
    if self.OnChange then R.spawn_for(self, self.OnChange, self, self:GetFiles()) end
end
ui.Register("FileInput", FileInput, "Panel")

local streams = {}
local Stream = {}
Stream.__index = Stream
function Stream:Send(text) N.send(self.id, tostring(text)) end
function Stream:Close()
    if not streams[self.id] then return end
    streams[self.id] = nil
    if self.owner then self.owner:RemoveCallOnRemove(self) end
    N.disconnect(self.id)
end

local function open(url, handlers, owner, sse)
    local id = N.connect(url, sse)
    local s = setmetatable({ id = id, handlers = handlers or {}, owner = owner }, Stream)
    streams[id] = s
    if owner then owner:CallOnRemove(s, function() s:Close() end) end
    return s
end

function browser.WebSocket(url, handlers, owner) return open(url, handlers, owner, false) end
function browser.EventSource(url, handlers, owner) return open(url, handlers, owner, true) end
function R.stream_event(id, event, text)
    local s = streams[id]
    if not s then return end
    if event == "close" then s:Close() end
    local fn = s.handlers[event]
    if fn then R.spawn_for(s.owner, fn, s, text) end
end
