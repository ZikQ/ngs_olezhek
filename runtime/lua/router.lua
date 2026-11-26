---@diagnostic disable: duplicate-set-field, need-check-nil
local N, R = ...

router = {}

local routes = {}

-- Same syntax as the Rust #[get] macros: "/users/{id}", "/files/{*path}".
-- Fewer parameters wins, so "/users/new" beats "/users/{id}".
local function compile(pattern, class)
    local segs, score = {}, 0
    for seg in pattern:gmatch("[^/]+") do
        local wild = seg:match("^{%*(.+)}$")
        local param = not wild and seg:match("^{(.+)}$")
        if wild then
            segs[#segs + 1] = { kind = "wild", name = wild }
            score = score + 100
        elseif param then
            segs[#segs + 1] = { kind = "param", name = param }
            score = score + 1
        else
            segs[#segs + 1] = { kind = "lit", value = seg }
        end
    end
    return { pattern = pattern, segs = segs, class = class, score = score }
end

local lazy, explicitFiles = {}, {}
local function add_route(pattern, name)
    routes[#routes + 1] = compile(pattern, name)
    table.sort(routes, function(a, b) return a.score < b.score end)
end

--- Register in autorun. Modules under lazy/ are shipped separately, including in embedded builds.
function router.Lazy(pattern, class, file)
    assert(file:match("^lazy/.+%.lua$"), "expected a module under lazy/")
    explicitFiles[file] = true
    for i = #routes, 1, -1 do
        if routes[i].class == file then table.remove(routes, i) end
    end
    lazy[class] = { file = file, pattern = pattern }
    R.on_register(class, {})
end

function R.on_register(name, tbl)
    for i = #routes, 1, -1 do
        if routes[i].class == name then table.remove(routes, i) end
    end
    local pattern = lazy[name] and lazy[name].pattern or rawget(tbl, "Route")
    if pattern then
        add_route(pattern, name)
    end
end

local function pct_decode(s)
    return (s:gsub("%%(%x%x)", function(h) return string.char(tonumber(h, 16)) end))
end

local function form_decode(s) return pct_decode((s:gsub("+", " "))) end

local function match(route, parts)
    local params = {}
    for i, s in ipairs(route.segs) do
        if s.kind == "wild" then
            params[s.name] = pct_decode(table.concat(parts, "/", i))
            return params
        end
        local part = parts[i]
        if part == nil then return nil end
        if s.kind == "param" then
            params[s.name] = pct_decode(part)
        elseif s.value ~= part then
            return nil
        end
    end
    if #parts ~= #route.segs then return nil end
    return params
end

local function parse_query(q)
    local out = {}
    for pair in (q or ""):gmatch("[^&]+") do
        local k, v = pair:match("^([^=]*)=?(.*)$")
        out[form_decode(k)] = form_decode(v)
    end
    return out
end

local NotFound = {}

function NotFound:Initialize()
    self:SetAlign(ALIGN_CENTER)
    self:SetJustify(JUSTIFY_CENTER)
    self:SetGap(8)
    local code = ui.Create("Label", self)
    code:SetFont("Title")
    code:SetText("404")
    local text = ui.Create("Label", self)
    text:SetText("Nothing at " .. self.Path)
end

ui.Register("__NotFound", NotFound, "Page")

local files, loaded, pending = {}, {}, {}
local layouts, cached = {}, {}
local activeKey, activeURL, activeIndex, restoring, parking
local defaultTitle = N.document_title()
local sequence = 0

local function ensure_module(file)
    -- Checked before `loaded`: while the module's top level waits, `loaded` holds an empty placeholder.
    if pending[file] then
        local ok, message = R.await(pending[file])
        if not ok then error(message, 0) end
        return
    end
    if loaded[file] then return end
    local key = {}
    pending[file] = key
    local ok, message = pcall(function()
        if not files[file] then
            local fn, failure = R.await(N.load_page(file))
            if not fn then error(failure, 0) end
            files[file] = files[file] or fn
        end
        include(file)
    end)
    pending[file] = nil
    R.op_done(key, ok, message)
    if not ok then error(message, 0) end
end

-- A conventional module returns its page class or a plain definition with an optional Base.
local function discover(file)
    if explicitFiles[file] or lazy[file] then return end
    local path = "/" .. file:sub(6, -5)
    path = path:gsub("/index$", "")
    path = path == "" and "/" or path
    lazy[file] = {file = file, pattern = path, automatic = true}
    add_route(path, file)
end

local function page_class(class)
    local route = lazy[class]
    if not route then return class end
    ensure_module(route.file)
    if not route.automatic then return class end
    local def = include(route.file)
    assert(type(def) == "table", route.file .. " must return its page definition")
    if not rawget(def, "ClassName") then ui.Register(class, def, def.Base or "Page") end
    return def.ClassName
end

local function resolve(path)
    local parts = {}
    for p in path:gmatch("[^/]+") do parts[#parts + 1] = p end
    for _, route in ipairs(routes) do
        local params = match(route, parts)
        if params then return route.class, params end
    end
    return ui.Exists("NotFoundPage") and "NotFoundPage" or "__NotFound", {}
end

local function find_cached(key)
    for i, entry in ipairs(cached) do
        if entry.key == key then return entry, i end
    end
end

local function save_scroll(panel, saved)
    local x, y = panel:GetProp("scrollLeft") or 0, panel:GetProp("scrollTop") or 0
    if x ~= 0 or y ~= 0 then saved[panel] = {x, y} end
    for _, child in ipairs(panel.__children) do save_scroll(child, saved) end
end

local function is_inside(panel, ancestor)
    while panel do
        if panel == ancestor then return true end
        panel = panel.__parent
    end
    return false
end

local function leave(keep)
    local page = R.page
    if not IsValid(page) then return end
    if keep and page.KeepAlive then
        -- A modal dialog left open under the hidden parking panel would keep the whole document inert.
        for i = #R.modals, 1, -1 do
            local frame = R.modals[i]
            if frame and is_inside(frame, page) then frame:Close() end
        end
        if not parking then
            parking = ui.Create("Panel", R.root)
            parking:Hide()
        end
        local scroll = {}
        save_scroll(page, scroll)
        page:SetParent(parking)
        cached[#cached + 1] = {key = activeKey, page = page, scroll = scroll}
        if #cached > 8 then table.remove(cached, 1).page:Remove() end
    else
        page:Remove()
    end
    R.page = nil
end

--- Layouts create their outlet during Initialize. Without an explicit outlet, use the default slot.
local Panel = ui.GetClass("Panel")
function Panel:SetOutlet(panel)
    self:SetSlot("outlet", panel)
end

local function mount_layouts(names, ctx, reset)
    local common = 0
    while not reset and layouts[common+1] and layouts[common+1].__class == names[common+1] do
        common = common + 1
    end
    for i = #layouts, common + 1, -1 do
        layouts[i]:Remove()
        layouts[i] = nil
    end
    local parent = R.root
    for i, name in ipairs(names) do
        local layout = layouts[i]
        if not layout then
            layout = R.create(name, parent, ctx)
            layout:Dock(DOCK_FILL)
            layouts[i] = layout
        else
            layout.Path, layout.Params, layout.Query, layout.Data = ctx.Path, ctx.Params, ctx.Query, ctx.Data
        end
        parent = layout:GetSlot("outlet")
    end
    return parent
end

local function update_url(mode, url)
    if mode == "push" then N.push_state(url)
    elseif mode == "replace" then N.replace_state(url) end
    activeURL, activeIndex = url, N.history_index()
end

-- Recover the old history position after a denied Back/Forward without overwriting its destination.
local function rollback_pop()
    if not activeURL then return end
    local index = N.history_index()
    if activeIndex and index and index ~= activeIndex then
        restoring = activeIndex
        N.history_go(activeIndex - index)
    else
        N.replace_state(activeURL)
    end
end

local function mount(class, ctx, key, url, mode, names, hit, index)
    update_url(mode, url)
    if hit then table.remove(cached, index) end
    local reset = mode == "reload"
    leave(not reset and key ~= activeKey)
    local parent = mount_layouts(names, ctx, reset)
    local page
    if hit then
        page = hit.page
        page:SetParent(parent)
        R.flush_docks()
        for panel, pos in pairs(hit.scroll) do
            if IsValid(panel) then
                panel:SetProp("scrollLeft", pos[1])
                panel:SetProp("scrollTop", pos[2])
            end
        end
    else
        page = R.create(class, parent, ctx)
        page:Dock(DOCK_FILL)
    end
    R.page, activeKey = page, key
    N.title((page.GetTitle and page:GetTitle()) or page.Title or defaultTitle)
    hook.Run("OnPageChanged", ctx.Path, page)
end

-- Initialize, OnRemove and OnPageChanged can request another navigation synchronously.
-- Finish attaching the current tree before letting that request replace it.
local mounting, queued
local function commit(...)
    mounting = true
    local ok, message = pcall(mount, ...)
    mounting = false
    local next_visit = queued
    queued = nil
    if next_visit then next_visit() end
    if not ok then error(message, 0) end
end

local Failure = { Title = "Navigation" }
function Failure:Initialize()
    self:Add("Label", {Text = self.Message})
    self:Add("Button", {Text = "Retry", DoClick = router.Reload})
end
ui.Register("__NavigationFailure", Failure, "Page")

local function initial_failure(path, query, url, message)
    if not R.page then
        commit("__NavigationFailure", {Path = path, Query = parse_query(query), Params = {}, Message = message},
            nil, url, nil, {})
    end
end

local function transition(id, path, query, url, mode)
    for _ = 1, 12 do
        local class, params = resolve(path)
        class = page_class(class)
        if id ~= sequence then return end
        local def = assert(ui.GetClass(class), "route module must register " .. class)
        local ctx = { Params = params, Query = parse_query(query), Path = path }
        local key = class .. ":" .. path .. "?" .. query
        local guard = def.Guard and def.Guard(ctx)
        if id ~= sequence then return end
        if guard == false then
            if mode == "pop" then rollback_pop() end
            initial_failure(path, query, url, "Access denied")
            hook.Run("OnNavigationDenied", path)
            return
        elseif type(guard) == "string" then
            path, query, url = N.resolve_url(guard)
            if mode ~= "push" then mode = "replace" end
        else
            if key == activeKey and IsValid(R.page) and R.page.KeepAlive and mode ~= "reload" then
                update_url(mode, url)
                return
            end
            local hit, index = find_cached(key)
            if hit and not IsValid(hit.page) then table.remove(cached, index); hit = nil end
            local names = def.Layout or {}
            if type(names) == "string" then names = {names} end
            for _, name in ipairs(names) do assert(ui.Exists(name), "unknown layout " .. name) end
            if def.Loader and not hit then
                local data, failure = def.Loader(ctx)
                if failure then error(type(failure) == "table" and failure.message or tostring(failure), 0) end
                ctx.Data = data
            end
            if id == sequence then commit(class, ctx, key, url, mode, names, hit, index) end
            return
        end
    end
    error("too many route redirects")
end

local function navigate(path, query, url, mode)
    if mounting then
        queued = function() navigate(path, query, url, mode) end
        return
    end
    sequence = sequence + 1
    local id = sequence
    R.spawn(function()
        local ok, message = pcall(transition, id, path, query or "", url, mode)
        if not ok and id == sequence then
            if mode == "pop" then rollback_pop() end
            initial_failure(path, query, url, "Could not load this page")
            N.error(tostring(message))
            hook.Run("OnNavigationError", path, message)
        end
    end)
end

function R.render(path, query, mode)
    local _, _, url = N.resolve_url("")
    navigate(path, query, url, mode)
end

local function go(url, mode)
    local path, query, absolute = N.resolve_url(url)
    navigate(path, query, absolute, mode)
end

function router.Go(url) go(url, "push") end
function router.Replace(url) go(url, "replace") end
function router.Back() N.back() end
function router.Open(url) return gui.OpenURL(url) end
function router.Reload()
    for _, e in ipairs(cached) do e.page:Remove() end
    cached = {}
    local path, query = N.location()
    R.render(path, query, "reload")
end
function router.GetPath() return (N.location()) end
function router.GetPage() return R.page end
function R.popstate(path, query)
    local expected = restoring
    restoring = nil
    if expected and N.history_index() == expected then return end
    R.render(path, query, "pop")
end


--- Runs a file from the lua folder once and returns what it returned; later calls reuse the result.
--- Eager modules run at startup. Lazy modules become available after their first navigation.
function include(path)
    local res = loaded[path]
    if res then return table.unpack(res, 1, res.n) end
    local fn = files[path]
    if not fn then error("include: no such file '" .. tostring(path) .. "'", 2) end
    loaded[path] = { n = 0 }
    local ok, value = pcall(function() return table.pack(fn()) end)
    if not ok then loaded[path] = nil; error(value, 0) end
    res = value
    loaded[path] = res
    return table.unpack(res, 1, res.n)
end

-- Hot reload. Re-running the file re-registers its classes, and since instances share one
-- metatable per class, live panels get the new methods at once and repaint. Initialize does
-- not run again on its own, so if a live panel uses one of the re-registered classes (or the
-- file registers nothing, e.g. fonts/skin in autorun) the current page is rebuilt.
function R.reload(name, fn)
    files[name] = fn
    if name:match("^lazy/") then
        discover(name)
        if not loaded[name] then return end
    end
    loaded[name] = nil
    local registered = {}
    R.on_class_registered = function(cls) registered[cls] = true end
    R.spawn(include, name)
    R.on_class_registered = nil
    print("[ngs] reloaded " .. name)
    for _, p in pairs(R.panels) do p:InvalidatePaint() end
    if next(registered) == nil or R.uses_classes(registered) then router.Reload() end
    hook.Run("OnReload", name)
end

--- `defer_render`: the test harness shows the first page itself, on the URL the test asks for.
function R.boot(list, dev, defer_render)
    DEV = dev
    R.root = R.create("Panel", false)
    N.class(R.root.__id, "ngs-root", true)
    N.mount(R.root.__id)
    for _, e in ipairs(list) do
        if e.fn then files[e.name] = e.fn end
    end
    for _, e in ipairs(list) do
        if e.err then
            N.error(e.err)
        elseif not e.deferred and not loaded[e.name] then
            R.spawn(include, e.name)
        end
    end
    for _, e in ipairs(list) do
        if e.deferred then discover(e.name) end
    end
    if not defer_render then R.render(N.location()) end
end
