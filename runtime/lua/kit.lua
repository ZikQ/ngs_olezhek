-- Prelude code touches internal panel fields (__id, __menu...) that the public definitions hide.
---@diagnostic disable: duplicate-set-field, inject-field, undefined-field, need-check-nil
local N, R = ...

local Panel = ui.GetClass("Panel")
local Label = ui.GetClass("Label")

local function cls(p, name, on) N.class(p.__id, name, on ~= false) end

local function spawn(self, method, ...)
    local fn = self[method]
    if fn then return R.spawn_for(self, fn, self, ...) end
end

local function variant_setter(prefix, allowed, fn_name)
    return function(self, v)
        if v ~= nil and not allowed[v] then
            local list = {}
            for k in pairs(allowed) do list[#list + 1] = k end
            table.sort(list)
            error(fn_name .. ": expected " .. table.concat(list, ", ") .. ", got " .. tostring(v), 2)
        end
        if self.__variant then cls(self, prefix .. self.__variant, false) end
        self.__variant = v
        if v then cls(self, prefix .. v) end
    end
end

-- Library colors are a Color, an app variable (ui.SetVars), a skin key or plain CSS.
function R.css_color_of(c)
    if c == nil or IsColor(c) then return c end
    if type(c) == "string" then
        if ui.GetVar(c) ~= nil then return ui.Var(c) end
        if R.SKIN[c] then return "var(" .. R.SKIN[c] .. ")" end
    end
    return c
end

-- Canvases need a real Color: the variable or skin key has to be set to one.
function R.paint_color(c, fallback)
    if IsColor(c) then return c end
    if type(c) == "string" then
        local v = ui.GetVar(c)
        if IsColor(v) then return v end
        v = ui.GetSkin(c)
        if IsColor(v) then return v end
    end
    return fallback
end

--- Children, including ones created later, ignore the mouse: a click anywhere lands on this panel.
--- A child can opt back in with SetMouseInputEnabled(true).
function Panel:SetChildrenMouseInputEnabled(b) cls(self, "ngs-children-passive", b == false) end

local Button = ui.GetClass("Button")
local BUTTON_VARIANTS = { primary = true, outline = true, danger = true, ghost = true }
local set_button_variant = variant_setter("ngs-button-", BUTTON_VARIANTS, "Button:SetVariant")

function Button:SetVariant(v)
    if v == "default" then v = nil end
    set_button_variant(self, v)
end

function Button:GetVariant() return self.__variant or "default" end

local TONES = { accent = true, success = true, warning = true, danger = true, neutral = true }

local Badge = {}
function Badge:Initialize()
    cls(self, "ngs-badge")
    self:SetVariant("neutral")
end
Badge.SetVariant = variant_setter("ngs-badge-", TONES, "Badge:SetVariant")
function Badge:GetVariant() return self.__variant end
ui.Register("Badge", Badge, "Label")

local STATUSES = { online = true, busy = true, idle = true, offline = true }

local Status = { BindDefault = "Text" }
function Status:Initialize()
    cls(self, "ngs-status")
    self.Dot = self:Add("Panel")
    cls(self.Dot, "ngs-status-dot")
    self.TextLabel = self:Add("Label")
    self:SetStatus("offline")
end
local set_status = variant_setter("ngs-status-", STATUSES, "Status:SetStatus")
function Status:SetStatus(s) set_status(self, s or "offline") end
function Status:GetStatus() return self.__variant end
function Status:SetText(t) self.TextLabel:SetText(t) end
function Status:GetText() return self.TextLabel:GetText() end
--- Pulsing ring around the dot; offline never pulses.
function Status:SetPulse(b) cls(self, "ngs-status-pulse", b == true) end
ui.Register("Status", Status, "Panel")

local CYRILLIC_UPPER = { ["ё"] = "Ё" }
local function upper_char(ch)
    if #ch == 1 then return ch:upper() end
    local cp = utf8.codepoint(ch)
    if cp >= 0x430 and cp <= 0x44F then return utf8.char(cp - 0x20) end
    if cp >= 0x450 and cp <= 0x45F then return utf8.char(cp - 0x50) end
    return CYRILLIC_UPPER[ch] or ch
end

local function first_char(s) return s:match("^[%z\1-\127\194-\244][\128-\191]*") end

--- Up to two initials: "Анна Волкова" -> "АВ", "@lily_rose" -> "LR", "" -> "?".
function R.initials(name)
    if type(name) ~= "string" then return "?" end
    name = name:Trim():gsub("^@+", "")
    local parts = {}
    for part in name:gmatch("[^%s_%-%.]+") do parts[#parts + 1] = part end
    if #parts == 0 then return "?" end
    local a = upper_char(first_char(parts[1]) or "?")
    if #parts == 1 then return a end
    return a .. upper_char(first_char(parts[2]) or "")
end

--- Stable hue 0..359 for a name, so a person keeps one color everywhere.
function R.name_hue(name)
    local hash = 0
    for _, cp in utf8.codes(name or "") do
        hash = (hash * 31 + cp) & 0x7fffffff
    end
    return hash % 360
end

local AVATAR_SIZES = { sm = { 26, 10 }, md = { 34, 12 }, lg = { 52, 17 } }

local Avatar = { BindDefault = "Name" }
function Avatar:Initialize()
    cls(self, "ngs-avatar")
    self.Initials = self:Add("Label")
    self:SetAvatarSize("md")
    self:SetName(nil)
end

function Avatar:SetName(name)
    self.__name = name
    self.Initials:SetText(R.initials(name))
    self:SetTooltip(name)
    local h = R.name_hue(name)
    N.style(self.__id, "background-color", ("hsl(%d 62%% 55%% / 0.18)"):format(h))
    N.style(self.__id, "color", ("color-mix(in srgb, hsl(%d 72%% 58%%) 72%%, var(--ngs-fg))"):format(h))
    N.style(self.__id, "box-shadow", ("inset 0 0 0 1px hsl(%d 62%% 60%% / 0.3)"):format(h))
end

function Avatar:GetName() return self.__name end

--- nil removes the picture and the initials show again.
function Avatar:SetImage(url)
    if url == nil or url == "" then
        if IsValid(self.Picture) then self.Picture:Remove() end
        self.Picture = nil
        return
    end
    if not IsValid(self.Picture) then
        self.Picture = self:Add("Image")
        cls(self.Picture, "ngs-avatar-img")
    end
    self.Picture:SetImage(url)
    self.Picture:SetAltText(self.__name)
end

--- "sm" (26), "md" (34), "lg" (52) or a diameter in pixels.
function Avatar:SetAvatarSize(s)
    local spec = AVATAR_SIZES[s]
    if not spec then
        if type(s) ~= "number" then error("Avatar:SetAvatarSize: expected sm, md, lg or pixels, got " .. tostring(s), 2) end
        spec = { s, math.max(9, math.floor(s / 3)) }
    end
    self:SetSize(spec[1], spec[1])
    N.style(self.__id, "font-size", spec[2] .. "px")
end
ui.Register("Avatar", Avatar, "Panel")

local Meter = { BindDefault = "Fraction" }
function Meter:Initialize()
    cls(self, "ngs-meter")
    self:SetAttr("role", "progressbar")
    self:SetAttr("aria-valuemin", 0)
    self:SetAttr("aria-valuemax", 100)
    self.Track = self:Add("Panel")
    cls(self.Track, "ngs-meter-track")
    self.Fill = self.Track:Add("Panel")
    cls(self.Fill, "ngs-meter-fill")
    self.ValueLabel = self:Add("Label")
    cls(self.ValueLabel, "ngs-meter-label")
    self.ValueLabel:Hide()
    self:SetVariant("accent")
    self:SetFraction(0)
end

function Meter:SetFraction(f)
    f = math.Clamp(tonumber(f) or 0, 0, 1)
    self.__fraction = f
    local pct = math.floor(f * 100 + 0.5)
    N.style(self.Fill.__id, "width", string.format("%g%%", f * 100))
    self:SetAttr("aria-valuenow", pct)
    if not self.__label then self.ValueLabel:SetText(pct .. "%") end
end

function Meter:GetFraction() return self.__fraction or 0 end

local set_meter_variant = variant_setter("ngs-meter-", { accent = true, success = true, warning = true, danger = true }, "Meter:SetVariant")
function Meter:SetVariant(v) set_meter_variant(self, v or "accent") end
function Meter:GetVariant() return self.__variant end
function Meter:SetShowLabel(b) self.ValueLabel:SetVisible(b == true) end

--- Text instead of the percentage; nil returns to the percentage.
function Meter:SetLabel(text)
    self.__label = text
    if text then self.ValueLabel:SetText(text) else self:SetFraction(self:GetFraction()) end
end

function Meter:SetThickness(px) self.Track:SetTall(px) end
ui.Register("Meter", Meter, "Panel")

local Segmented = { BindDefault = "Value" }
function Segmented:Initialize()
    cls(self, "ngs-segmented")
    self:SetAttr("role", "radiogroup")
    self.__items = {}
end

--- Returns the option's Button: add an Icon to it, set a tooltip...
function Segmented:AddOption(text, data)
    local i = #self.__items + 1
    local button = self:Add("Button", { Text = text })
    cls(button, "ngs-segmented-item")
    button:SetAttr("role", "radio")
    button:SetAttr("aria-checked", "false")
    self.__items[i] = { button = button, data = data, text = text }
    button.DoClick = function()
        if self:Select(i) then
            R.input_changed(self, "Value", data)
            spawn(self, "OnSelect", i, data)
        end
    end
    return button
end

--- Returns true when the selection changed. OnSelect fires only for user clicks.
function Segmented:Select(index)
    if index == self.__selected then return false end
    for i, item in ipairs(self.__items) do
        item.button:SetAttr("aria-checked", tostring(i == index))
    end
    self.__selected = self.__items[index] and index or nil
    return true
end

function Segmented:SetValue(data)
    for i, item in ipairs(self.__items) do
        if item.data == data then return self:Select(i) end
    end
    self:Select(nil)
end

function Segmented:GetValue()
    local item = self.__items[self.__selected or 0]
    return item and item.data
end

function Segmented:GetSelectedID() return self.__selected end
ui.Register("Segmented", Segmented, "Panel")

local Select = { BindDefault = "Value" }
function Select:Initialize()
    cls(self, "ngs-select")
    self:SetAttr("aria-haspopup", "listbox")
    self:SetAttr("aria-expanded", "false")
    self.__options = {}
    self:__refresh()
end

function Select:__refresh()
    local opt = self.__options[self.__selected or 0]
    cls(self, "ngs-select-placeholder", opt == nil)
    Label.SetText(self, opt and opt.text or self.__placeholder or "")
end

function Select:SetPlaceholder(text)
    self.__placeholder = text
    self:__refresh()
end

function Select:AddOption(text, data)
    local i = #self.__options + 1
    self.__options[i] = { text = tostring(text), data = data }
    return i
end

--- Strings, { text, data } or { text = ..., data = ... }. Keeps the selection when its data is still there.
function Select:SetOptions(list)
    local keep = self:GetValue()
    local had = self.__selected ~= nil
    self.__options = {}
    self.__selected = nil
    for _, o in ipairs(list) do
        if type(o) == "table" then self:AddOption(o.text or o[1], o.data == nil and o[2] or o.data)
        else self:AddOption(o, o) end
    end
    if had then
        for i, o in ipairs(self.__options) do
            if o.data == keep then self.__selected = i break end
        end
    end
    self:__refresh()
end

function Select:Clear()
    self.__options = {}
    self.__selected = nil
    self:__refresh()
end

--- Selects without firing OnSelect; nil clears the selection and shows the placeholder.
function Select:ChooseIndex(i)
    self.__selected = self.__options[i or 0] and i or nil
    self:__refresh()
end

function Select:SetValue(data)
    for i, o in ipairs(self.__options) do
        if o.data == data then return self:ChooseIndex(i) end
    end
    self:ChooseIndex(nil)
end

function Select:GetValue()
    local o = self.__options[self.__selected or 0]
    return o and o.data
end

function Select:GetSelected()
    local o = self.__options[self.__selected or 0]
    if o then return o.text, o.data end
end

function Select:GetSelectedID() return self.__selected end
function Select:GetOptions() return self.__options end

function Select:__choose(i)
    local o = self.__options[i]
    if IsValid(self.__pop) then self.__pop:Hide() end
    if not o then return end
    local changed = self.__selected ~= i
    self:ChooseIndex(i)
    self:RequestFocus()
    if changed then
        R.input_changed(self, "Value", o.data)
        spawn(self, "OnSelect", i, o.text, o.data)
    end
end

function Select:DoClick()
    local pop = self.__pop
    if not IsValid(pop) then
        pop = ui.Create("Popover", self:GetParent())
        cls(pop, "ngs-select-list")
        pop:SetMatchAnchorWidth(true)
        pop:SetAttr("role", "listbox")
        pop.OnClose = function()
            if IsValid(self) then self:SetAttr("aria-expanded", "false") end
        end
        self:CallOnRemove("ngs-select", function() if IsValid(pop) then pop:Remove() end end)
        self.__pop = pop
    end
    if pop:IsOpen() then return pop:Hide() end
    pop:Clear()
    local items = {}
    for i, o in ipairs(self.__options) do
        local b = pop:Add("Button", { Text = o.text })
        cls(b, "ngs-select-option")
        b:SetAttr("role", "option")
        b:SetAttr("aria-selected", tostring(i == self.__selected))
        b.DoClick = function() self:__choose(i) end
        b.OnKeyCodePressed = function(_, key)
            local dest = ({ ArrowDown = i + 1, ArrowUp = i - 1, Home = 1, End = #items })[key]
            if dest and items[dest] then items[dest]:RequestFocus() return true end
        end
        items[i] = b
    end
    if #items == 0 then
        local empty = pop:Add("Label", { Text = self.EmptyText })
        cls(empty, "ngs-select-empty")
    end
    pop:Toggle(self)
    if pop:IsOpen() then
        self:SetAttr("aria-expanded", "true")
        local focus = items[self.__selected or 1]
        if focus then focus:RequestFocus() end
    end
end
Select.EmptyText = "No options"
ui.Register("Select", Select, "Button")

local Icon = {}
function Icon:Initialize()
    self.__paths = {}
    self.__stroke = 1.8
    self.__box = 24
    self:SetSize(18, 18)
    self:SetMouseInputEnabled(false)
    self:SetAttr("aria-hidden", "true")
end

--- SVG path data on a 24x24 grid (Lucide, Feather): one string or a list of them.
function Icon:SetIcon(paths)
    if type(paths) == "string" then paths = { paths } end
    self.__paths = paths or {}
    self:InvalidatePaint()
end

--- Color, app variable name or skin key; default is the muted text color.
function Icon:SetColor(c)
    self.__color = c
    self:InvalidatePaint()
end

function Icon:GetColor() return self.__color end

function Icon:SetStrokeWidth(w)
    self.__stroke = w
    self:InvalidatePaint()
end

function Icon:SetFilled(b)
    self.__filled = b == true
    self:InvalidatePaint()
end

function Icon:SetViewBox(size)
    self.__box = size
    self:InvalidatePaint()
end

local MUTED = Color(154, 160, 170)
function Icon:Paint(w, h)
    local s = math.min(w, h) / self.__box
    local ox, oy = (w - self.__box * s) / 2, (h - self.__box * s) / 2
    surface.SetDrawColor(R.paint_color(self.__color, R.paint_color("textMuted", MUTED)))
    surface.SetLineWidth(self.__stroke * s)
    surface.SetLineCap("round")
    surface.SetLineJoin("round")
    local mode = self.__filled and "fill" or "stroke"
    for _, d in ipairs(self.__paths) do surface.DrawPath(d, mode, ox, oy, s) end
end
ui.Register("Icon", Icon, "Panel")

local Spinner = {}
function Spinner:Initialize()
    cls(self, "ngs-spinner")
    self:SetSize(22, 22)
    self:SetAttr("role", "status")
    self:SetAttr("aria-label", self.LoadingText)
end
Spinner.LoadingText = "Loading"
ui.Register("Spinner", Spinner, "Panel")

-- Plain table: every row is in the DOM, no inner scroll, so the page scrolls as one.
-- For thousands of rows use DataGrid, or SetPageSize here.
local ListView = {}
function ListView:Initialize()
    cls(self, "ngs-listview")
    self:SetAttr("role", "table")
    self.__columns, self.__rows, self.__order = {}, {}, {}
    self.__shown = 0
    self.Inner = self:Add("Panel")
    cls(self.Inner, "ngs-listview-inner")
    self.Header = self.Inner:Add("Grid")
    cls(self.Header, "ngs-listview-header")
    self.Header:SetAttr("role", "row")
    self.Body = self.Inner:Add("Panel")
    cls(self.Body, "ngs-listview-body")
    self.Body:SetAttr("role", "rowgroup")
    self.Empty = self.Inner:Add("Label", { Text = self.EmptyText })
    cls(self.Empty, "ngs-listview-empty")
    self.Footer = self:Add("Row")
    cls(self.Footer, "ngs-listview-footer")
    self.MoreButton = self.Footer:Add("Button", { Text = self.MoreText })
    self.MoreButton:SetVariant("outline")
    self.MoreButton.DoClick = function() self:ShowMore() end
    self.Footer:Hide()
end
ListView.EmptyText = "No data"
ListView.MoreText = "Show more"

local function widths_of(columns)
    local w = {}
    for i, c in ipairs(columns) do w[i] = c.width or "minmax(0, 1fr)" end
    return w
end

local function sort_value(col, row)
    local s = col.sort
    local v
    if type(s) == "function" then v = s(row)
    elseif type(s) == "string" then v = row[s]
    else v = row[col.key] end
    if type(v) == "string" then v = v:lower() end
    return v
end

--- Columns: key, title, width (px or "1fr"), align ("right" for numbers), format(value, row),
--- style (table or function(row) for SetStyle), build(cell, row) for custom cells,
--- sort (true, a key or function(row)).
function ListView:SetColumns(columns)
    self.__columns = columns
    self.__widths = widths_of(columns)
    self.Header:Clear()
    self.Header:SetColumns(self.__widths)
    for i, col in ipairs(columns) do
        local cell = self.Header:Add("Row")
        cls(cell, "ngs-listview-th")
        cell:SetAttr("role", "columnheader")
        cls(cell, "ngs-listview-right", col.align == "right")
        local text = cell:Add("Label", { Text = col.title or col.key or "" })
        cls(text, "ngs-listview-text")
        if col.sort then
            cls(cell, "ngs-listview-sortable")
            cell.Arrow = cell:Add("Label", { Text = "↕" })
            cls(cell.Arrow, "ngs-listview-arrow")
            cell.DoClick = function()
                local desc = self.__sortCol == i and not self.__sortDesc
                if self.__sortCol ~= i then desc = col.sortDesc ~= false end
                self:SetSort(i, desc)
                spawn(self, "OnSort", i, desc)
            end
        end
    end
    if self.__sortCol and not (columns[self.__sortCol] and columns[self.__sortCol].sort) then self.__sortCol = nil end
    self:__render()
end

function ListView:SetRows(rows)
    self.__rows = rows or {}
    self.__shown = self.__pageSize or #self.__rows
    self:__render()
end

function ListView:GetRows() return self.__rows end

--- Rows in display order (after sorting).
function ListView:GetDisplayRows() return self.__order end

--- Column index and direction; nil returns to the order given to SetRows.
function ListView:SetSort(index, desc)
    self.__sortCol, self.__sortDesc = index, desc == true
    self:__render()
end

function ListView:GetSort() return self.__sortCol, self.__sortDesc end

--- Show only the first n rows and a "Show more" button for the next n.
function ListView:SetPageSize(n)
    self.__pageSize = n
    self.__shown = n or #self.__rows
    self:__render()
end

function ListView:ShowMore()
    self.__shown = self.__shown + (self.__pageSize or #self.__rows)
    self:__render()
end

--- Narrower than this, the table scrolls sideways inside itself.
function ListView:SetMinWidth(px) N.style(self.Inner.__id, "min-width", px and (px .. "px") or nil) end

function ListView:SetEmptyText(text) self.Empty:SetText(text) end

--- Horizontal and vertical cell padding of header and rows.
function ListView:SetRowPadding(x, y)
    self.__pad = { x, y }
    N.style(self.Header.__id, "padding", y .. "px " .. x .. "px")
    self:__render()
end

function ListView:SetRowHeight(px)
    self.__rowh = px
    self:__render()
end

function ListView:__render()
    local rows = self.__rows
    local order = {}
    for i, r in ipairs(rows) do order[i] = r end
    local col = self.__sortCol and self.__columns[self.__sortCol]
    if col and col.sort then
        local desc = self.__sortDesc
        local keyed = {}
        for i, r in ipairs(order) do keyed[i] = { r, sort_value(col, r), i } end
        table.sort(keyed, function(a, b)
            local x, y = a[2], b[2]
            if x == nil or y == nil then
                if x == y then return a[3] < b[3] end
                return y == nil
            end
            if x == y then return a[3] < b[3] end
            if type(x) ~= type(y) then x, y = tostring(x), tostring(y) end
            if desc then return x > y end
            return x < y
        end)
        for i, k in ipairs(keyed) do order[i] = k[1] end
    end
    self.__order = order

    for i, c in ipairs(self.Header:GetChildren()) do
        if c.Arrow then
            local on = self.__sortCol == i
            c.Arrow:SetText(on and (self.__sortDesc and "↓" or "↑") or "↕")
            cls(c, "ngs-listview-sorted", on)
            c:SetAttr("aria-sort", on and (self.__sortDesc and "descending" or "ascending") or "none")
        end
    end

    self.Body:Clear()
    local limit = math.min(#order, self.__shown or #order)
    local clickable = self.OnRowClick ~= nil
    for i = 1, limit do
        local record = order[i]
        local row = self.Body:Add("Grid")
        cls(row, "ngs-listview-row")
        cls(row, "ngs-listview-clickable", clickable)
        row:SetAttr("role", "row")
        row:SetColumns(self.__widths or {})
        row:SetChildrenMouseInputEnabled(false)
        if self.__pad then N.style(row.__id, "padding", self.__pad[2] .. "px " .. self.__pad[1] .. "px") end
        if self.__rowh then N.style(row.__id, "min-height", self.__rowh .. "px") end
        if clickable then row.DoClick = function() spawn(self, "OnRowClick", record, i) end end
        if self.OnRowRightClick then row.DoRightClick = function() spawn(self, "OnRowRightClick", record, i) end end
        for _, c in ipairs(self.__columns) do
            local cell = row:Add("Row")
            cls(cell, "ngs-listview-cell")
            cls(cell, "ngs-listview-right", c.align == "right")
            cell:SetAttr("role", "cell")
            if c.build then
                c.build(cell, record, i)
            else
                local v = record[c.key]
                if c.format then v = c.format(v, record, i) end
                local text = cell:Add("Label", { Text = v })
                cls(text, "ngs-listview-text")
                local st = c.style
                if type(st) == "function" then st = st(record, i) end
                if st then text:SetStyle(st) end
            end
        end
    end
    self.Empty:SetVisible(#order == 0)
    self.Footer:SetVisible(limit < #order)
    if self.OnRendered then R.pcall(self.OnRendered, self) end
end
ui.Register("ListView", ListView, "Panel")

-- Frame with an in-flow layout: header, scrolling body (the default slot), button footer.
local Dialog = {}
function Dialog:Initialize()
    cls(self, "ngs-dialog")
    self:SetDraggable(false)
    self:DockPadding(0, 0, 0, 0)
    self:SetSize(math.min(480, ScrW() - 32), nil)
    self.Header.__pos = nil
    self.Header:__apply_pos()
    self.Header:SetSize(nil, nil)
    self.Header:DockPadding(24, 22, 18, 0)
    N.style(self.TitleLabel.__id, "font", nil)
    cls(self.TitleLabel, "ngs-dialog-title")
    self.Body = self:Add("Panel")
    cls(self.Body, "ngs-dialog-body")
    self.Footer = self:Add("Row")
    cls(self.Footer, "ngs-dialog-footer")
    self.Footer:Hide()
    self:SetSlot(self.Body)
    self:Center()
end

--- variant: "primary", "outline", "danger"... fn(dialog) runs on click; without fn the dialog closes.
function Dialog:AddButton(text, variant, fn)
    self.Footer:Show()
    local b = self.Footer:Add("Button", { Text = text })
    if variant then b:SetVariant(variant) end
    b.DoClick = function()
        if fn then fn(self) else self:Close() end
    end
    return b
end
ui.Register("Dialog", Dialog, "Frame")
