---@diagnostic disable: duplicate-set-field
local N, R = ...

-- Values are stored as JSON under "ngs:" so tables, numbers and booleans come back as they went in.
-- When the browser refuses (private mode, quota, disabled storage) the data lives in memory
-- until the page is closed.
local PREFIX = "ngs:"

local function make(session)
    local memory = {}
    local s = {}

    function s.Get(key, default)
        key = PREFIX .. tostring(key)
        local raw = N.storage_get(key, session)
        if raw == nil then raw = memory[key] end
        if raw == nil then return default end
        local ok, v = pcall(N.json_decode, raw)
        if ok and v ~= nil then return v end
        return default
    end

    --- nil removes the key. Returns false if the browser did not keep it.
    function s.Set(key, value)
        key = PREFIX .. tostring(key)
        if value == nil then
            memory[key] = nil
            N.storage_remove(key, session)
            return true
        end
        local raw = N.json_encode(value)
        if N.storage_set(key, raw, session) then
            memory[key] = nil
            return true
        end
        memory[key] = raw
        return false
    end

    function s.Remove(key) s.Set(key, nil) end

    function s.Keys()
        local out, seen = {}, {}
        for _, k in ipairs(N.storage_keys(session)) do
            if k:sub(1, #PREFIX) == PREFIX then
                seen[k] = true
                out[#out + 1] = k:sub(#PREFIX + 1)
            end
        end
        for k in pairs(memory) do
            if not seen[k] then out[#out + 1] = k:sub(#PREFIX + 1) end
        end
        table.sort(out)
        return out
    end

    --- Only this app's keys; other data on the origin stays.
    function s.Clear()
        for _, k in ipairs(s.Keys()) do s.Remove(k) end
    end

    return s
end

storage = make(false)
storage.Session = make(true)

-- Another tab changed localStorage.
function R.storage_changed(key)
    if key == nil then
        hook.Run("OnStorageChanged", nil)
    elseif key:sub(1, #PREFIX) == PREFIX then
        local name = key:sub(#PREFIX + 1)
        hook.Run("OnStorageChanged", name, storage.Get(name))
    end
end
