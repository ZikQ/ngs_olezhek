---@diagnostic disable: duplicate-set-field, inject-field, undefined-field, need-check-nil
local N, R = ...

local Tabs = {}
function Tabs:Initialize()
    self.items = {}
    self.Bar = self:Add("Row")
    self.Bar:SetAttr("role", "tablist")
    self:SetGap(12)
    self.Bar:SetGap(6)
end

function Tabs:Select(index)
    local item = self.items[index]
    if not item or index == self.selected then return end
    for i, tab in ipairs(self.items) do
        local active = i == index
        tab.body:SetVisible(active)
        tab.button:SetAttr("aria-selected", tostring(active))
        tab.button:SetAttr("tabindex", active and 0 or -1)
    end
    self.selected = index
    if self.OnSelect then R.spawn_for(self, self.OnSelect, self, index, item.body) end
end

--- Returns the panel that holds this tab's content.
function Tabs:AddTab(title)
    local i = #self.items + 1
    local button = self.Bar:Add("Button", {Text = title})
    local body = self:Add("Panel")
    local bid, pid = "ngs-tab-" .. button.__id, "ngs-tabpanel-" .. body.__id
    button:SetAttr("id", bid)
    button:SetAttr("role", "tab")
    button:SetAttr("aria-controls", pid)
    button:SetAttr("aria-selected", "false")
    button:SetAttr("tabindex", -1)
    body:SetAttr("id", pid)
    body:SetAttr("role", "tabpanel")
    body:SetAttr("aria-labelledby", bid)
    N.style(body.__id, "flex", "1 0 auto")
    body:Hide()
    self.items[i] = {button = button, body = body}
    button.DoClick = function() self:Select(i) end
    button.OnKeyCodePressed = function(_, key)
        local count = #self.items
        local dest = ({ArrowLeft = (i-2) % count+1, ArrowRight = i % count+1, Home = 1, End = count})[key]
        if dest then
            self:Select(dest)
            self.items[dest].button:RequestFocus()
            return true
        end
    end
    if i == 1 then self:Select(i) end
    return body
end
ui.Register("Tabs", Tabs, "Panel")

local Grid = {}
function Grid:Initialize()
    self.columns, self.rows, self.visible = {}, {}, {}
    self.rowHeight = 32
    self:SetAttr("role", "table")
    self:SetTall(256)
    self.Header = self:Add("Grid")
    self.Header:SetAttr("role", "row")
    self.View = self:Add("ScrollPanel")
    self.View:SetGrow(1)
    self.Canvas = self.View:Add("Panel")
    self.Canvas:SetAttr("role", "rowgroup")
    self.View.__on_scroll = function() self:RefreshRows() end
    self.View.PerformLayout = function() self:RefreshRows() end
end

function Grid:SetColumns(columns)
    self.columns = columns
    self.Header:Clear()
    self.Canvas:Clear()
    self.visible = {}
    local widths = {}
    for i, column in ipairs(columns) do
        widths[i] = column.width or "minmax(0, 1fr)"
        local cell = self.Header:Add("Label", {Text = column.title or column.key})
        cell:SetAttr("role", "columnheader")
    end
    self.widths = widths
    self.Header:SetColumns(widths)
    self:RefreshRows(true)
end

function Grid:SetRows(rows)
    self.source = nil
    self:SetAttr("aria-busy", "false")
    if self.retry then self.retry:Hide() end
    self.rows = rows
    self:SetAttr("aria-rowcount", #rows + 1)
    self:RefreshRows(true)
end

--- loader(offset, limit) may await http.Get. Return {total=N, rows={...}} or nil, error.
--- Offsets are zero based. Only nearby batches are retained; at most two requests run at once.
function Grid:SetDataSource(loader, pageSize)
    pageSize = pageSize or 64
    assert(type(loader) == "function" and math.tointeger(pageSize) and pageSize > 0, "expected loader and positive page size")
    self.rows = {}
    self.source = {load = loader, size = pageSize, pages = {}, active = 0}
    self:SetAttr("aria-rowcount", -1)
    self.View:SetProp("scrollTop", 0)
    self:RefreshRows(true)
end

local function load_batch(grid, source, offset)
    local batch = {pending = true}
    source.pages[offset] = batch
    source.active = source.active + 1
    R.spawn_for(grid, function()
        local ok, result, err = pcall(source.load, offset, source.size)
        if grid.source ~= source then return end
        source.active = source.active - 1
        batch.pending = false
        local total = ok and type(result) == "table" and math.tointeger(result.total)
        if total and total >= 0 and type(result.rows) == "table"
            and #result.rows == math.min(source.size, math.max(total - offset, 0)) then
            source.total, batch.rows = total, result.rows
            grid:SetAttr("aria-rowcount", total + 1)
        else
            batch.error = (not ok and tostring(result)) or (type(err) == "table" and err.message)
                or tostring(err or "invalid row batch")
            if grid.OnLoadError then R.spawn_for(grid, grid.OnLoadError, grid, batch.error, offset) end
        end
        grid.View:InvalidateLayout()
    end)
end

function Grid:Retry()
    if not self.source then return end
    for offset, batch in pairs(self.source.pages) do
        if batch.error then self.source.pages[offset] = nil end
    end
    self:RefreshRows(true)
end

local function request_range(grid, source, first, last)
    local size = source.size
    local start = ((first - 1) // size) * size
    local finish = ((last - 1) // size) * size
    for offset, batch in pairs(source.pages) do
        if not batch.pending and (offset < start - size or offset > finish + size) then
            source.pages[offset] = nil
        end
    end
    local failed, loading = false, false
    for offset = start, finish, size do
        if not source.pages[offset] and source.active < 2 then load_batch(grid, source, offset) end
        local batch = source.pages[offset]
        failed = failed or (batch and batch.error ~= nil)
        loading = loading or not batch or batch.pending
    end
    grid:SetAttr("aria-busy", tostring(loading))
    if failed and not grid.retry then
        grid.retry = grid:Add("Button", {Text = "Retry", DoClick = function() grid:Retry() end})
    end
    if grid.retry then grid.retry:SetVisible(failed) end
end

function Grid:SetRowHeight(height)
    assert(type(height) == "number" and height >= 1, "expected a positive row height")
    self.rowHeight = height
    self:RefreshRows(true)
end

function Grid:RefreshRows(force)
    local source = self.source
    local count = source and source.total or #self.rows
    local h = self.rowHeight
    if self.Canvas.__h ~= count * h then self.Canvas:SetTall(count * h) end
    local viewport = self.View:GetTall()
    local top = math.Clamp(self.View:GetProp("scrollTop") or 0, 0, math.max(0, count*h-viewport))
    local first = math.max(1, math.floor(top/h))
    local last = math.min(count, math.ceil((top+viewport)/h)+1)
    if source and viewport > 0 then
        request_range(self, source, first, source.total == nil and 1 or last)
    end
    for i, row in pairs(self.visible) do
        if i < first or i > last then row:Remove(); self.visible[i] = nil end
    end
    for i = first, last do
        local row = self.visible[i]
        if not row then
            row = self.Canvas:Add("Grid")
            row:SetAttr("role", "row")
            row:SetAttr("aria-rowindex", i+1)
            row:SetColumns(self.widths or {})
            row:SetWide("100%")
            for _ in ipairs(self.columns) do row:Add("Label"):SetAttr("role", "cell") end
            self.visible[i] = row
        end
        local record = self.rows[i]
        if source then
            local offset = ((i - 1) // source.size) * source.size
            local batch = source.pages[offset]
            record = batch and batch.rows and batch.rows[i - offset]
        end
        if not force and row.__bound and row.__record == record then goto continue end
        row.__bound, row.__record = true, record
        row:SetPos(0, (i-1)*h)
        row:SetTall(h)
        for j, col in ipairs(self.columns) do
            local value
            if record then
                value = record[col.key]
                if col.format then value = col.format(value, record, i) end
            elseif j == 1 then
                value = "…"
            end
            row:GetChild(j):SetText(value)
        end
        ::continue::
    end
end
ui.Register("DataGrid", Grid, "Panel")
ui.Register("Table", {}, "DataGrid")

local Disclosure = {}
function Disclosure:Initialize()
    self.Toggle = self:Add("Button")
    self.Body = self:Add("Panel")
    self:SetExpanded(false)
    self.Toggle.DoClick = function() self:SetExpanded(not self.expanded) end
end
function Disclosure:SetText(s) self.Toggle:SetText(s) end
function Disclosure:SetExpanded(on)
    self.expanded = on == true
    self.Body:SetVisible(self.expanded)
    self.Toggle:SetAttr("aria-expanded", tostring(self.expanded))
end
ui.Register("Disclosure", Disclosure, "Panel")

local Tree = {}
local function add_node(parent, tree, label, data)
    local node = parent:Add("TreeNode", {Text = label})
    node.tree, node.data = tree, data
    return node
end
function Tree:AddNode(label, data)
    return add_node(self, self, label, data)
end
local Node = {}
function Node:AddNode(label, data)
    self.hasChildren = true
    self.Toggle:SetAttr("aria-expanded", tostring(self.expanded))
    return add_node(self.Body, self.tree, label, data)
end
function Node:Initialize()
    self.Toggle:SetAttr("role", "treeitem")
    self.Toggle:SetAttr("aria-expanded", nil)
    self.Body:SetAttr("role", "group")
    self.Body:DockPadding(16, 0, 0, 0)
    self.Toggle.DoClick = function()
        if self.hasChildren then self:SetExpanded(not self.expanded) end
        self:Select()
    end
end

function Node:Select()
    local tree = self.tree
    if IsValid(tree.selected) then tree.selected.Toggle:SetAttr("aria-selected", "false") end
    tree.selected = self
    self.Toggle:SetAttr("aria-selected", "true")
    if tree.OnSelect then R.spawn_for(tree, tree.OnSelect, tree, self, self.data) end
end
function Tree:Initialize() self:SetAttr("role", "tree") end
ui.Register("TreeNode", Node, "Disclosure")
ui.Register("Tree", Tree, "Panel")

local Dropdown = {}
function Dropdown:Initialize()
    self.options = {}
    self:SetAttr("aria-haspopup", "menu")
end
function Dropdown:AddOption(text, data)
    self.options[#self.options+1] = {text, data}
end
function Dropdown:DoClick()
    if IsValid(self.menu) then self.menu:Remove() end
    local menu = ui.Menu(self:GetParent())
    self.menu = menu
    self:SetAttr("aria-expanded", "true")
    menu:CallOnRemove(self, function()
        if self:IsValid() then self:SetAttr("aria-expanded", "false") end
    end)
    self:CallOnRemove("dropdown", function() if menu:IsValid() then menu:Remove() end end)
    for i, opt in ipairs(self.options) do
        menu:AddOption(opt[1], function()
            self.selected = i
            self:SetText(opt[1])
            if self.OnSelect then R.spawn_for(self, self.OnSelect, self, i, opt[2]) end
        end)
    end
    local x, y, _, h = self:GetScreenBounds()
    menu:Open(x, y+h)
    if menu:GetChild(1) then menu:GetChild(1):RequestFocus() end
end
ui.Register("Dropdown", Dropdown, "Button")

local stack
local TOAST_KINDS = { info = true, success = true, warning = true, danger = true }

--- kind: "info", "success", "warning" or "danger" adds a colored stripe.
function ui.Toast(message, seconds, kind)
    if kind ~= nil and not TOAST_KINDS[kind] then
        error("ui.Toast: kind must be info, success, warning or danger, got " .. tostring(kind), 2)
    end
    if not IsValid(stack) then
        stack = ui.Create("Panel")
        N.class(stack.__id, "ngs-toasts", true)
        stack:SetAttr("aria-live", "polite")
    end
    local toast = stack:Add("Label", {Text = message, Wrap = true})
    N.class(toast.__id, "ngs-toast", true)
    if kind then N.class(toast.__id, "ngs-toast-" .. kind, true) end
    toast:TimerSimple(seconds or 4, function() toast:Remove() end)
    return toast
end

ui.ConfirmText = { title = "Confirm", accept = "OK", cancel = "Cancel" }

--- The callback receives false for Cancel, Escape, the close button or owner removal.
--- message may be a table: { Message or Build = function(body), Title, Accept, Cancel,
--- AcceptVariant, CancelVariant, Width }.
function ui.Confirm(message, callback, title, parent)
    local opts = type(message) == "table" and message or { Message = message }
    title = opts.Title or title or ui.ConfirmText.title
    local frame = ui.Create("Dialog", parent)
    frame:SetTitle(title)
    frame:SetAttr("aria-label", title)
    if opts.Width then frame:SetWide(math.min(opts.Width, ScrW() - 32)) end
    if opts.Build then
        opts.Build(frame.Body)
    else
        frame:Add("Label", {Text = opts.Message, Wrap = true})
    end
    local accepted = false
    frame:AddButton(opts.Accept or ui.ConfirmText.accept, opts.AcceptVariant or "primary", function()
        accepted = true
        frame:Close()
    end)
    local cancel = frame:AddButton(opts.Cancel or ui.ConfirmText.cancel, opts.CancelVariant or "outline")
    frame:CallOnRemove("confirm", function() if callback then R.spawn(callback, accepted) end end)
    frame:MakePopup()
    cancel:RequestFocus()
    return frame
end

local Panel = ui.GetClass("Panel")
local function hide_tip(owner)
    if IsValid(owner.__tip) then owner.__tip:Remove() end
    owner.__tip = nil
end
function Panel:SetRichTooltip(build)
    hide_tip(self)
    self.__tipBuild = build
end
function R.tooltip_event(owner, event, key)
    if not owner.__tipBuild then return end
    if event == "exit" or event == "blur" or event == "key" and key == "Escape" then
        hide_tip(owner)
    elseif (event == "enter" or event == "focus") and not IsValid(owner.__tip) then
        local tip = ui.Create("Panel", R.modals[#R.modals] or R.root)
        owner.__tip = tip
        N.class(tip.__id, "ngs-rich-tip", true)
        tip:SetAttr("role", "tooltip")
        local id = "ngs-tip-" .. tip.__id
        tip:SetAttr("id", id)
        local previous = owner:GetAttr("aria-describedby")
        owner:SetAttr("aria-describedby", previous and previous .. " " .. id or id)
        tip:CallOnRemove("description", function()
            if IsValid(owner) then owner:SetAttr("aria-describedby", previous) end
        end)
        owner:CallOnRemove("tooltip", hide_tip)
        if type(owner.__tipBuild) == "function" then owner.__tipBuild(tip)
        else tip:Add("Label", {Text = owner.__tipBuild, Wrap = true}) end
        tip.__fixed = true
        local x, y, _, h = owner:GetScreenBounds()
        local w, ht = tip:GetSize()
        tip:SetPos(math.max(0, math.min(x, ScrW()-w)), math.max(0, math.min(y+h+4, ScrH()-ht)))
    end
end
