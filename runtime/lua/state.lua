---@diagnostic disable: duplicate-set-field, inject-field
local N, R = ...

-- Push-pull signals: Set marks dependents (computeds go dirty, effects get queued), then the
-- queue runs once per change. Computeds recompute lazily on Get, so a chain of them settles
-- before any effect reads it and an effect never sees half-updated values.

local WEAK = { __mode = "k" }
local STATE, COMPUTED = {}, {}
STATE.__index = STATE
COMPUTED.__index = COMPUTED

local current
local depth = 0
local queue, queued = {}, {}
local unowned = {}
local EFFECTS = {}

local function track(source)
    if current then
        source.__subs[current] = true
        current.__deps[source] = true
    end
end

local function untrack_all(node)
    for dep in pairs(node.__deps) do dep.__subs[node] = nil end
    node.__deps = {}
end

local mark

local function notify(source)
    local subs = {}
    for s in pairs(source.__subs) do subs[#subs + 1] = s end
    for _, s in ipairs(subs) do mark(s) end
end

function mark(node)
    if node.__compute then
        if not node.__dirty then
            node.__dirty = true
            notify(node)
        end
    elseif not node.__disposed and not queued[node] then
        queued[node] = true
        queue[#queue + 1] = node
    end
end

local function run(e)
    if e.__cleanup then
        local c = e.__cleanup
        e.__cleanup = nil
        R.pcall(c)
    end
    untrack_all(e)
    local prev = current
    current = e
    local ok, cleanup = R.pcall(e.__fn)
    current = prev
    if ok and type(cleanup) == "function" then e.__cleanup = cleanup end
end

local function flush()
    if depth > 0 then return end
    depth = depth + 1
    local rounds = 0
    while queue[1] do
        rounds = rounds + 1
        if rounds > 100 then
            queue, queued = {}, {}
            N.error("ui.State: effects keep changing the state they read (a cycle?)")
            break
        end
        local list = queue
        queue = {}
        for _, e in ipairs(list) do
            queued[e] = nil
            if not e.__disposed then run(e) end
        end
    end
    depth = depth - 1
end

local function changed(source)
    notify(source)
    flush()
end

local function dispose(e)
    if e.__disposed then return end
    e.__disposed = true
    untrack_all(e)
    unowned[e] = nil
    if e.__cleanup then R.pcall(e.__cleanup) end
end

local function effect(fn, owner)
    local e = { __fn = fn, __deps = {} }
    if owner then
        local list = rawget(owner, "__effects")
        if not list then
            list = {}
            owner.__effects = list
            owner:CallOnRemove(EFFECTS, function(p)
                for x in pairs(list) do dispose(x) end
                rawset(p, "__binds", nil)
                rawset(p, "__two_way", nil)
            end)
        end
        list[e] = true
    else
        unowned[e] = true
    end
    run(e)
    return function()
        dispose(e)
        local list = owner and rawget(owner, "__effects")
        if list then list[e] = nil end
    end
end
R.scoped[EFFECTS] = "__effects"

--- A value that panels and effects can follow: count:Get(), count:Set(5), count:Update(f).
function ui.State(value)
    return setmetatable({ __value = value, __subs = setmetatable({}, WEAK) }, STATE)
end

--- A read-only value derived from other states, recomputed only when one of them changed
--- and someone reads it.
function ui.Computed(fn)
    if type(fn) ~= "function" then error("ui.Computed: expected a function", 2) end
    return setmetatable({ __compute = fn, __dirty = true, __deps = {}, __subs = setmetatable({}, WEAK) }, COMPUTED)
end

function ui.IsState(v)
    local m = getmetatable(v)
    return m == STATE or m == COMPUTED
end

--- Runs fn now and again whenever a state it read changes. fn may return a cleanup function
--- (called before the next run and on dispose). With `owner` the effect dies with that panel;
--- otherwise keep the returned dispose function.
function ui.Effect(fn, owner)
    if type(fn) ~= "function" then error("ui.Effect: expected a function", 2) end
    if owner ~= nil and not ui.IsPanel(owner) then error("ui.Effect: owner must be a panel", 2) end
    return effect(fn, owner)
end

--- Groups several Set calls: effects run once, after fn returns.
function ui.Batch(fn)
    depth = depth + 1
    local ok, err = pcall(fn)
    depth = depth - 1
    flush()
    if not ok then error(err, 0) end
end

--- Reads states inside fn without subscribing the running effect to them.
function ui.Untrack(fn, ...)
    local prev = current
    current = nil
    local res = table.pack(pcall(fn, ...))
    current = prev
    if not res[1] then error(res[2], 0) end
    return table.unpack(res, 2, res.n)
end

function STATE:Get()
    track(self)
    return self.__value
end

function STATE:Peek() return self.__value end

function STATE:Set(v)
    if rawequal(self.__value, v) then return end
    self.__value = v
    changed(self)
end

function STATE:Update(fn) self:Set(fn(self.__value)) end

--- For tables changed in place, which Set cannot detect.
function STATE:Notify() changed(self) end

--- fn(value) after every change (not right away). Returns an unsubscribe function.
function STATE:Subscribe(fn, owner)
    local first = true
    return ui.Effect(function()
        local v = self:Get()
        if first then
            first = false
            return
        end
        ui.Untrack(fn, v)
    end, owner)
end

STATE.__call = STATE.Get
STATE.__tostring = function(self) return "State(" .. tostring(self.__value) .. ")" end

function COMPUTED:Get()
    track(self)
    if self.__dirty then
        untrack_all(self)
        local prev = current
        current = self
        local ok, v = pcall(self.__compute)
        current = prev
        if not ok then error(v, 0) end
        self.__value = v
        self.__dirty = false
    end
    return self.__value
end

function COMPUTED:Peek() return ui.Untrack(COMPUTED.Get, self) end
function COMPUTED:Set() error("ui.Computed is read-only", 2) end
COMPUTED.Subscribe = STATE.Subscribe
COMPUTED.__call = COMPUTED.Get
COMPUTED.__tostring = function(self) return "Computed(" .. tostring(self.__value) .. ")" end

local PANEL = ui.GetClass("Panel")
local NOTHING = {}

--- panel:Bind([prop,] source [, format]) keeps Set<prop> in sync with a state; only that one
--- setter runs on change. prop defaults to the class's BindDefault (Text for labels, Value
--- for inputs). source may be a function, which becomes a ui.Computed. Inputs bound to a
--- plain ui.State without format write user edits back into it (two-way).
function PANEL:Bind(prop, source, format)
    if type(prop) ~= "string" then prop, source, format = nil, prop, source end
    prop = prop or self.BindDefault
    if not prop then
        error("Bind: " .. tostring(self.__class) .. " has no default property, pass one: Bind(\"Text\", state)", 2)
    end
    if type(source) == "function" then source = ui.Computed(source) end
    if not ui.IsState(source) then error("Bind: expected a ui.State, a ui.Computed or a function", 2) end
    local method = "Set" .. prop
    if type(self[method]) ~= "function" then error("Bind: " .. tostring(self.__class) .. " has no " .. method, 2) end
    if format ~= nil and type(format) ~= "function" then error("Bind: format must be a function", 2) end

    local binds = rawget(self, "__binds")
    if not binds then
        binds = {}
        self.__binds = binds
    end
    if binds[prop] then binds[prop]() end

    local last = NOTHING
    local dispose = effect(function()
        local v = source:Get()
        if format then v = format(v) end
        if rawequal(v, last) or self.__removed then return end
        last = v
        -- The value came from this very input; writing it back would move the caret.
        if rawget(self, "__echo") == prop then return end
        if R.UNPACK[prop] and type(v) == "table" and getmetatable(v) == nil then
            self[method](self, table.unpack(v))
        else
            self[method](self, v)
        end
    end, self)
    binds[prop] = dispose

    local tw = rawget(self, "__two_way")
    if tw then tw[prop] = nil end
    if not format and getmetatable(source) == STATE then
        tw = tw or {}
        self.__two_way = tw
        tw[prop] = source
    end
    return dispose
end

function PANEL:Unbind(prop)
    prop = prop or self.BindDefault
    local binds = rawget(self, "__binds")
    if binds and binds[prop] then
        binds[prop]()
        binds[prop] = nil
    end
    local tw = rawget(self, "__two_way")
    if tw then tw[prop] = nil end
end

function PANEL:Effect(fn) return ui.Effect(fn, self) end

--- Controls call this from their input handlers with the value the user produced.
function R.input_changed(self, prop, value)
    local tw = rawget(self, "__two_way")
    local s = tw and tw[prop]
    if not s then return end
    rawset(self, "__echo", prop)
    local ok, err = pcall(s.Set, s, value)
    rawset(self, "__echo", nil)
    if not ok then N.error(tostring(err)) end
end
