---@diagnostic disable: duplicate-set-field
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

function R.on_register(name, tbl)
    for i = #routes, 1, -1 do
        if routes[i].class == name then table.remove(routes, i) end
    end
    local pattern = rawget(tbl, "Route")
    if pattern then
        routes[#routes + 1] = compile(pattern, name)
        table.sort(routes, function(a, b) return a.score < b.score end)
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

function R.render(path, query)
    local parts = {}
    for p in path:gmatch("[^/]+") do parts[#parts + 1] = p end
    local class, params
    for _, r in ipairs(routes) do
        params = match(r, parts)
        if params then
            class = r.class
            break
        end
    end
    if not class then
        class = ui.Exists("NotFoundPage") and "NotFoundPage" or "__NotFound"
        params = {}
    end
    if R.page then R.page:Remove() end
    -- Params/Query/Path are set before Initialize runs.
    local page = R.create(class, R.root, { Params = params, Query = parse_query(query), Path = path })
    R.page = page
    page:Dock(DOCK_FILL)
    if page.Title then N.title(page.Title) end
    hook.Run("OnPageChanged", path, page)
end

function router.Go(url)
    N.push_state(url)
    R.render(N.location())
end

function router.Replace(url)
    N.replace_state(url)
    R.render(N.location())
end

function router.Back() N.back() end
function router.Open(url) return gui.OpenURL(url) end
function router.Reload() R.render(N.location()) end
function router.GetPath() return (N.location()) end
function router.GetPage() return R.page end

function R.popstate(path, query) R.render(path, query) end

local files, loaded = {}, {}

--- Runs a file from the lua folder once and returns what it returned; later calls reuse the result.
--- Every file is run at startup anyway, include just controls the order and passes values around.
function include(path)
    local res = loaded[path]
    if res then return table.unpack(res, 1, res.n) end
    local fn = files[path]
    if not fn then error("include: no such file '" .. tostring(path) .. "'", 2) end
    loaded[path] = { n = 0 }
    res = table.pack(fn())
    loaded[path] = res
    return table.unpack(res, 1, res.n)
end

-- Hot reload. Re-running the file re-registers its classes, and since instances share one
-- metatable per class, live panels get the new methods at once and repaint. Initialize does
-- not run again on its own, so if a live panel uses one of the re-registered classes (or the
-- file registers nothing, e.g. fonts/skin in autorun) the current page is rebuilt.
function R.reload(name, fn)
    files[name] = fn
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

function R.boot(list, dev)
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
        elseif not loaded[e.name] then
            R.spawn(include, e.name)
        end
    end
    R.render(N.location())
end
