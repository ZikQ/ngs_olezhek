---@diagnostic disable: duplicate-set-field
local N, R = ...

local Panel = ui.GetClass("Panel")

local function set_of(words)
    local t = {}
    for w in words:gmatch("%S+") do t[w] = true end
    return t
end

-- Allow-list rather than a deny-list: on*, style, srcdoc, formaction, target and friends stay out
-- even if the browser grows new dangerous attributes.
local ATTRS = set_of([[
role title alt placeholder name id tabindex lang dir autocomplete autofocus inputmode enterkeyhint
spellcheck maxlength minlength min max step pattern required readonly disabled checked selected multiple
accept value rows cols wrap draggable width height loading decoding label download colspan rowspan scope
headers translate inert controls autoplay loop muted playsinline preload type for list datetime reversed
start open size crossorigin referrerpolicy
]])

local URL_ATTRS = { href = "link", cite = "link", src = "src", poster = "src" }

local PROPS = set_of([[
value checked indeterminate selectedIndex disabled readOnly required placeholder min max step multiple
tabIndex title scrollTop scrollLeft muted volume currentTime playbackRate loop autoplay controls open
selectionStart selectionEnd spellcheck maxLength minLength rows cols defaultValue defaultChecked valueAsNumber
]])

local READ_ONLY = set_of([[
duration paused ended readyState naturalWidth naturalHeight complete currentSrc scrollWidth scrollHeight
clientWidth clientHeight validationMessage willValidate
]])

local METHODS = set_of("play pause load select showPicker stepUp stepDown reportValidity requestFullscreen")

local function attr_name(name)
    if type(name) ~= "string" then error("attribute name must be a string", 3) end
    name = name:lower()
    if ATTRS[name] or URL_ATTRS[name] or name:match("^aria%-%l+$") then return name end
    if name:match("^data%-[%l%d][%l%d%-]*$") and not name:match("^data%-ngs") then return name end
    if name:match("^on") then
        error("attribute '" .. name .. "' is an event handler; use panel callbacks instead", 3)
    end
    error("attribute '" .. name .. "' is not allowed", 3)
end

--- Attributes go on the native control when the panel wraps one (the <input> of a TextEntry).
--- true sets an empty (boolean) attribute, false or nil removes it. URL attributes are checked
--- like Link:SetURL and dropped when unsafe.
function Panel:SetAttr(name, value)
    name = attr_name(name)
    if value == false then
        value = nil
    elseif value == true then
        value = ""
    elseif value ~= nil then
        value = tostring(value)
    end
    local kind = URL_ATTRS[name]
    if kind and value ~= nil and not R.check_url(value, kind, "SetAttr('" .. name .. "')") then value = nil end
    N.attr(self.__id, name, value, true)
end

function Panel:GetAttr(name)
    return N.get_attr(self.__id, attr_name(name), true)
end

function Panel:SetProp(name, value)
    if not PROPS[name] then error("property '" .. tostring(name) .. "' is not allowed", 2) end
    local t = type(value)
    if t ~= "nil" and t ~= "boolean" and t ~= "number" and t ~= "string" then
        error("property values must be nil, boolean, number or string", 2)
    end
    N.prop(self.__id, name, value)
end

function Panel:GetProp(name)
    if not (PROPS[name] or READ_ONLY[name]) then error("property '" .. tostring(name) .. "' is not allowed", 2) end
    return N.get_prop(self.__id, name)
end

--- Calls a no-argument method of the native element: play, pause, select, showPicker...
function Panel:Invoke(method)
    if not METHODS[method] then error("method '" .. tostring(method) .. "' is not allowed", 2) end
    N.invoke(self.__id, method)
end
