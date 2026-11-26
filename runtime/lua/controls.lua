-- Prelude code touches internal panel fields (__id, __menu...) that the public definitions hide.
---@diagnostic disable: duplicate-set-field, inject-field, undefined-field, need-check-nil
local N, R = ...

local Panel = ui.GetClass("Panel")

local function spawn(self, method, ...)
    local fn = self[method]
    if fn then return R.spawn_for(self, fn, self, ...) end
end

local Label = { __native = { tag = "span", class = "ngs-label" }, BindDefault = "Text" }

function Label:SetText(s)
    s = s == nil and "" or tostring(s)
    self.__text = s
    N.text(self.__id, s)
    self:InvalidatePaint()
end

function Label:GetText() return self.__text or "" end
function Label:SetWrap(b) N.style(self.__id, "white-space", b and "pre-wrap" or nil) end

ui.Register("Label", Label, "Panel")

ui.Register("Button", { __native = { tag = "button", class = "ngs-button" } }, "Label")

local TextEntry = { __native = { tag = "div", class = "ngs-textentry", control = "input", type = "text" }, BindDefault = "Value" }

function TextEntry:SetValue(v) N.prop(self.__id, "value", v == nil and "" or tostring(v)) end
function TextEntry:GetValue() return N.get_prop(self.__id, "value") or "" end
TextEntry.SetText = TextEntry.SetValue
TextEntry.GetText = TextEntry.GetValue

function TextEntry:GetFloat() return tonumber(self:GetValue()) end

function TextEntry:GetInt()
    local n = tonumber(self:GetValue())
    return n and math.floor(n)
end

-- The native control can be swapped (input <-> textarea), so its attributes live here and get reapplied.
function TextEntry:__control_attrs()
    local id = self.__id
    N.attr(id, "placeholder", self.__placeholder, true)
    N.attr(id, "maxlength", self.__maxlength and tostring(self.__maxlength), true)
    N.prop(id, "readOnly", self.__readonly == true)
    N.prop(id, "disabled", self.__disabled == true)
end

function TextEntry:SetPlaceholderText(s)
    self.__placeholder = s
    self:__control_attrs()
end

function TextEntry:SetMaxLength(n)
    self.__maxlength = n
    self:__control_attrs()
end

function TextEntry:SetEditable(b)
    self.__readonly = not b
    self:__control_attrs()
end

function TextEntry:SetMultiline(b)
    b = b == true
    if b == (self.__multiline == true) then return end
    self.__multiline = b
    if b then
        N.set_control(self.__id, "textarea", nil)
    else
        N.set_control(self.__id, "input", self.__type or "text")
    end
    self:__control_attrs()
end

--- "text", "password", "email", "number", "search", "tel", "url".
function TextEntry:SetInputType(t)
    self.__type = t
    if not self.__multiline then N.attr(self.__id, "type", t, true) end
end

function TextEntry:SetNumeric(b) self:SetInputType(b and "number" or "text") end

function TextEntry:__on_input()
    R.input_changed(self, "Value", self:GetValue())
    spawn(self, "OnChange")
    spawn(self, "OnValueChange", self:GetValue())
end

function TextEntry:__on_key(key)
    if key == "Enter" and not self.__multiline then spawn(self, "OnEnter", self:GetValue()) end
    return Panel.__on_key(self, key)
end

ui.Register("TextEntry", TextEntry, "Panel")

local CheckBox = { __native = { tag = "label", class = "ngs-checkbox", control = "input", type = "checkbox" }, BindDefault = "Checked" }

function CheckBox:SetChecked(b) N.prop(self.__id, "checked", b == true) end
function CheckBox:GetChecked() return N.get_prop(self.__id, "checked") == true end
CheckBox.SetValue = CheckBox.SetChecked
CheckBox.GetValue = CheckBox.GetChecked

function CheckBox:Toggle()
    self:SetChecked(not self:GetChecked())
    self:__on_change()
end

-- A click on the label text also produces a synthetic click on the input; use OnChange instead.
function CheckBox:__on_click() end

function CheckBox:__on_change()
    local on = self:GetChecked()
    R.input_changed(self, "Checked", on)
    R.input_changed(self, "Value", on)
    spawn(self, "OnChange", on)
end

ui.Register("CheckBox", CheckBox, "Label")

local ComboBox = { __native = { tag = "div", class = "ngs-combobox", control = "select" }, BindDefault = "Value" }

function ComboBox:Initialize() self.__choices = {} end

function ComboBox:AddChoice(text, data, select)
    text = tostring(text)
    table.insert(self.__choices, { text = text, data = data })
    N.add_option(self.__id, text)
    if select then self:ChooseOptionID(#self.__choices) end
    return #self.__choices
end

function ComboBox:Clear()
    self.__choices = {}
    N.clear_options(self.__id)
end

function ComboBox:ChooseOptionID(i) N.prop(self.__id, "selectedIndex", i - 1) end

function ComboBox:ChooseOption(text)
    for i, c in ipairs(self.__choices) do
        if c.text == text then return self:ChooseOptionID(i) end
    end
end

ComboBox.SetValue = ComboBox.ChooseOption

function ComboBox:GetSelectedID()
    local i = N.get_prop(self.__id, "selectedIndex")
    if i and i >= 0 then return math.floor(i) + 1 end
end

function ComboBox:GetSelected()
    local c = self.__choices[self:GetSelectedID() or 0]
    if c then return c.text, c.data end
end

function ComboBox:GetValue() return (self:GetSelected()) end
function ComboBox:GetOptionText(i) return self.__choices[i] and self.__choices[i].text end
function ComboBox:GetOptionData(i) return self.__choices[i] and self.__choices[i].data end

function ComboBox:__on_change()
    local i = self:GetSelectedID()
    local c = i and self.__choices[i]
    if not c then return end
    R.input_changed(self, "Value", c.text)
    spawn(self, "OnSelect", i, c.text, c.data)
end

ui.Register("ComboBox", ComboBox, "Panel")

local function number_input(class, ty)
    local T = { __native = { tag = "div", class = class, control = "input", type = ty }, BindDefault = "Value" }

    function T:SetMin(v)
        self.__min = v
        N.prop(self.__id, "min", tostring(v))
    end

    function T:SetMax(v)
        self.__max = v
        N.prop(self.__id, "max", tostring(v))
    end

    function T:SetMinMax(lo, hi)
        self:SetMin(lo)
        self:SetMax(hi)
    end

    function T:GetMin() return self.__min end
    function T:GetMax() return self.__max end

    function T:SetDecimals(d)
        self.__decimals = d
        N.prop(self.__id, "step", d > 0 and tostring(10 ^ -d) or "1")
    end

    function T:SetValue(v) N.prop(self.__id, "value", tostring(v)) end
    function T:GetValue() return tonumber(N.get_prop(self.__id, "value")) or 0 end

    function T:__on_input()
        R.input_changed(self, "Value", self:GetValue())
        spawn(self, "OnValueChanged", self:GetValue())
    end

    return T
end

local Slider = number_input("ngs-slider", "range")

function Slider:Initialize()
    self:SetMinMax(0, 100)
    self:SetDecimals(0)
end

ui.Register("Slider", Slider, "Panel")
ui.Register("NumberWang", number_input("ngs-numberwang", "number"), "Panel")

local ProgressBar = { __native = { tag = "div", class = "ngs-progress", control = "progress" }, BindDefault = "Fraction" }

function ProgressBar:Initialize() N.prop(self.__id, "max", 1) end

function ProgressBar:SetFraction(f)
    self.__fraction = f
    N.prop(self.__id, "value", math.Clamp(f, 0, 1))
end

function ProgressBar:GetFraction() return self.__fraction or 0 end

ui.Register("ProgressBar", ProgressBar, "Panel")

local Image = { __native = { tag = "img", class = "ngs-image" }, BindDefault = "Image" }

function Image:SetImage(src)
    if src ~= nil then
        src = tostring(src)
        if not R.check_url(src, "src", "Image:SetImage") then src = nil end
    end
    self.__src = src
    N.attr(self.__id, "src", src)
end

function Image:GetImage() return self.__src end
function Image:SetAltText(s) N.attr(self.__id, "alt", s) end
function Image:SetKeepAspect(b) N.style(self.__id, "object-fit", b and "contain" or nil) end

ui.Register("Image", Image, "Panel")

local Link = { __native = { tag = "a", class = "ngs-link" } }

--- Only http, https, mailto and relative URLs get into href; anything else (javascript:,
--- data:...) leaves the link without one and reports an error. Returns false in that case.
function Link:SetURL(url)
    if url ~= nil then
        url = tostring(url)
        if not R.check_url(url, "link", "Link:SetURL") then url = nil end
    end
    self.__url = url
    N.attr(self.__id, "href", url)
    return url ~= nil
end

function Link:GetURL() return self.__url end

function Link:SetNewTab(b)
    self.__newtab = b
    N.attr(self.__id, "target", b and "_blank" or nil)
    N.attr(self.__id, "rel", b and "noopener" or nil)
end

-- "//host" and "/\host" are other origins to the browser.
local function internal(url)
    return url == "/" or url:match("^/[^/\\]") ~= nil
end

-- Same-origin links navigate through the router; modified clicks (ctrl/shift/middle) stay with the browser.
function Link:__on_click(plain)
    spawn(self, "DoClick")
    local url = self.__url
    if plain and url and not self.__newtab and internal(url) then
        router.Go(url)
        return true
    end
    return url == nil
end

ui.Register("Link", Link, "Label")

local ScrollPanel = { __native = { tag = "div", class = "ngs-scroll" } }

function ScrollPanel:AddItem(p) p:SetParent(self) end
function ScrollPanel:GetCanvas() return self end

ui.Register("ScrollPanel", ScrollPanel, "Panel")

ui.Register("Row", { Initialize = function(self) self:SetLayout(LAYOUT_ROW) end }, "Panel")
ui.Register("Column", { Initialize = function(self) self:SetLayout(LAYOUT_COLUMN) end }, "Panel")
ui.Register("Grid", { Initialize = function(self) self:SetLayout(LAYOUT_GRID) end }, "Panel")

local function unmodal(frame)
    frame.__modal = false
    for i = #R.modals, 1, -1 do
        if R.modals[i] == frame then table.remove(R.modals, i) end
    end
end

-- The header is positioned, not docked, so the frame's own children flow normally
-- (or dock) inside the padding below it, like DFrame.
local Frame = { __native = { tag = "dialog", class = "ngs-frame" }, BindDefault = "Title" }

function Frame:Initialize()
    self.__fixed = true
    self.__delete_on_close = true
    self.__draggable = true
    self:DockPadding(12, 50, 12, 12)
    self:SetSize(420, 300)

    local header = ui.Create("Panel", self)
    header:SetPos(0, 0)
    header:SetSize("100%", 38)
    header:SetLayout(LAYOUT_ROW)
    header:SetAlign(ALIGN_CENTER)
    header:DockPadding(14, 0, 5, 0)
    N.class(header.__id, "ngs-frame-header", true)
    self.Header = header

    self.TitleLabel = ui.Create("Label", header)
    self.TitleLabel:SetGrow(1)
    self.TitleLabel:SetFont("DefaultBold")

    self.CloseButton = ui.Create("Button", header)
    self.CloseButton:SetText("×")
    N.class(self.CloseButton.__id, "ngs-frame-close", true)
    self.CloseButton.DoClick = function() self:Close() end

    function header.OnMousePressed(h, code)
        if code ~= MOUSE_LEFT or not self.__draggable then return end
        local fx, fy = self:GetScreenBounds()
        local mx, my = gui.MousePos()
        self.__drag = { mx - fx, my - fy }
        h:MouseCapture(true)
    end

    function header.OnCursorMoved()
        local d = self.__drag
        if not d then return end
        local mx, my = gui.MousePos()
        self:SetPos(mx - d[1], my - d[2])
    end

    function header.OnMouseReleased(h)
        self.__drag = nil
        h:MouseCapture(false)
    end

    self:Center()
    N.invoke(self.__id, "show")
end

function Frame:SetTitle(t) self.TitleLabel:SetText(t) end
function Frame:GetTitle() return self.TitleLabel:GetText() end
function Frame:ShowCloseButton(b) self.CloseButton:SetVisible(b) end
function Frame:SetDraggable(b) self.__draggable = b end
function Frame:GetDraggable() return self.__draggable end
function Frame:SetDeleteOnClose(b) self.__delete_on_close = b end
function Frame:IsModal() return self.__modal == true end

function Frame:SetSizable(b)
    N.style(self.__id, "resize", b and "both" or nil)
    N.style(self.__id, "min-width", b and "160px" or nil)
    N.style(self.__id, "min-height", b and "90px" or nil)
end

function Frame:SetBackgroundBlur(b) N.class(self.__id, "ngs-frame-blur", b == true) end

--- Modal: dims the page, traps focus, Esc closes.
function Frame:MakePopup()
    if self.__modal then return end
    Panel.SetVisible(self, true)
    N.invoke(self.__id, "close")
    N.invoke(self.__id, "showModal")
    self.__modal = true
    table.insert(R.modals, self)
end

function Frame:SetVisible(b)
    Panel.SetVisible(self, b)
    if b then
        if not N.get_prop(self.__id, "open") then N.invoke(self.__id, "show") end
    else
        N.invoke(self.__id, "close")
        unmodal(self)
    end
end

function Frame:Close()
    if self.__removed then return end
    if self.OnClose then R.pcall(self.OnClose, self) end
    if self.__delete_on_close then
        self:Remove()
    else
        self:SetVisible(false)
    end
end

function Frame:Remove()
    unmodal(self)
    Panel.Remove(self)
end

function Frame:Clear()
    for i = #self.__children, 1, -1 do
        local c = self.__children[i]
        if c ~= self.Header then c:Remove() end
    end
end

-- Esc on a modal dialog: close through Close() so OnClose and DeleteOnClose apply.
function Frame:__on_cancel()
    self:Close()
    return true
end

ui.Register("Frame", Frame, "Panel")

-- Popover API: top layer, closes on outside click and Esc by itself.
-- Submenus are DOM children of their parent menu, which keeps the parent open while they show.
local Menu = { __native = { tag = "div", class = "ngs-menu" } }

function Menu:Initialize()
    self.__fixed = true
    N.attr(self.__id, "popover", "auto")
    N.attr(self.__id, "role", "menu")
end

function Menu:AddOption(text, fn)
    local opt = ui.Create("MenuOption", self)
    opt:SetText(text)
    opt.__menu = self
    opt.__fn = fn
    return opt
end

function Menu:AddSpacer()
    local s = ui.Create("Panel", self)
    N.class(s.__id, "ngs-menu-spacer", true)
    return s
end

function Menu:AddSubMenu(text, fn)
    local opt = self:AddOption(text, fn)
    N.class(opt.__id, "ngs-menu-parent", true)
    local sub = ui.Create("Menu", self)
    sub.__parent_menu = self
    opt.__submenu = sub
    return sub, opt
end

--- Menus are one-shot by default, like DermaMenu: removed once closed.
function Menu:SetDeleteSelf(b) self.__keep = not b end

function Menu:IsOpen() return self.__open == true end

--- Opens at the cursor, or at x, y (page coordinates), kept inside the window.
function Menu:Open(x, y)
    if not x then x, y = gui.MousePos() end
    -- Everything outside a modal dialog is inert, so a menu opened from one must live inside it.
    local modal = R.modals[#R.modals]
    if modal and not self.__parent_menu and self.__parent ~= modal then self:SetParent(modal) end
    self:SetPos(x, y)
    if not self.__open then
        N.invoke(self.__id, "showPopover")
        self.__open = true
    end
    local _, _, w, h = N.bounds(self.__id)
    local sw, sh = N.screen()
    if x + w > sw then x = math.max(0, sw - w - 4) end
    if y + h > sh then y = math.max(0, sh - h - 4) end
    self:SetPos(x, y)
end

function Menu:Hide()
    if self.__open then N.invoke(self.__id, "hidePopover") end
end

function Menu:CloseAll()
    local root = self
    while root.__parent_menu do root = root.__parent_menu end
    root:Hide()
end

function Menu:__on_toggle(open)
    self.__open = open
    if open then return end
    local parent = self.__parent_menu
    if parent then
        if parent.__open_sub == self then parent.__open_sub = nil end
    elseif not self.__keep then
        self:Remove()
    end
end

ui.Register("Menu", Menu, "Panel")

local MenuOption = { __native = { tag = "button", class = "ngs-menu-option" } }

function MenuOption:Initialize() N.attr(self.__id, "role", "menuitem") end

function MenuOption:GetMenu() return self.__menu end

function MenuOption:__open_submenu()
    local menu = self.__menu
    if menu.__open_sub and menu.__open_sub ~= self.__submenu then
        menu.__open_sub:Hide()
        menu.__open_sub = nil
    end
    local sub = self.__submenu
    if sub and not sub.__open then
        local x, y, w = self:GetScreenBounds()
        sub:Open(x + w, y - 5)
        menu.__open_sub = sub
    end
end

function MenuOption:__on_enter()
    Panel.__on_enter(self)
    self:__open_submenu()
end

function MenuOption:__on_click()
    if self.__disabled then return end
    if self.__submenu then return self:__open_submenu() end
    self.__menu:CloseAll()
    if self.__fn then R.spawn(self.__fn, self) end
    if self.DoClick then R.spawn(self.DoClick, self) end
end

ui.Register("MenuOption", MenuOption, "Label")

function ui.Menu(parent) return ui.Create("Menu", parent) end

-- A Menu takes only options; this is the same top-layer popover for any content.
local Popover = { __native = { tag = "div", class = "ngs-popover" } }

local SIDES = { bottom = true, top = true, left = true, right = true }
local ALIGNS = { start = true, center = true, ["end"] = true }

function Popover:Initialize()
    self.__fixed = true
    self.__side, self.__align, self.__gap = "bottom", "start", 6
    self.__keep = true
    N.attr(self.__id, "popover", "auto")
    N.attr(self.__id, "role", "dialog")
    self:Hook("OnScreenSizeChanged", "ngs-popover", function()
        if self.__open then self:__place() end
    end)
end

function Popover:SetSide(side)
    if not SIDES[side] then error("Popover:SetSide: expected bottom, top, left or right, got " .. tostring(side), 2) end
    self.__side = side
end

function Popover:SetAlign(align)
    if not ALIGNS[align] then error("Popover:SetAlign: expected start, center or end, got " .. tostring(align), 2) end
    self.__align = align
end

function Popover:SetGap(px) self.__gap = tonumber(px) or 0 end

--- Popover at least as wide as the anchor, for select-like lists.
function Popover:SetMatchAnchorWidth(b) self.__match = b == true end

--- false: Esc and outside clicks no longer close it, only Hide does.
function Popover:SetAutoClose(b)
    N.attr(self.__id, "popover", b == false and "manual" or "auto")
end

--- Popovers are reused by default (date pickers, dropdowns); true removes it after it closes.
function Popover:SetDeleteSelf(b) self.__keep = not b end

function Popover:IsOpen() return self.__open == true end
function Popover:GetAnchor() return self.__anchor end

local function inside(p, ancestor)
    while p do
        if p == ancestor then return true end
        p = p.__parent
    end
    return false
end

--- Open(anchorPanel) next to the anchor, or Open(x, y) at window coordinates.
function Popover:Open(a, y)
    if ui.IsPanel(a) then
        self.__anchor, self.__at = a, nil
    else
        if not a then a, y = gui.MousePos() end
        self.__anchor, self.__at = nil, { a, y }
    end
    -- Outside a modal dialog everything is inert, so the popover has to move into it.
    local modal = R.modals[#R.modals]
    if modal and not inside(self, modal) then self:SetParent(modal) end
    if not self.__open then
        self.__open = true
        N.invoke(self.__id, "showPopover")
        self:__place()
        if self.OnOpen then R.spawn(self.OnOpen, self) end
    else
        self:__place()
    end
end

--- Light dismiss closes the popover on pointerup, right before the anchor's own click would
--- reopen it; Toggle swallows that click so a button can open and close the same popover.
function Popover:Toggle(anchor)
    if self.__open then return self:Hide() end
    if anchor and anchor == self.__last_anchor and self.__closed_at and CurTime() - self.__closed_at < 0.3 then
        return
    end
    self:Open(anchor)
end

function Popover:Hide()
    if not self.__open then return end
    N.invoke(self.__id, "hidePopover")
    self:__closed()
end

function Popover:__closed()
    if not self.__open then return end
    self.__open = false
    self.__closed_at = CurTime()
    self.__last_anchor = self.__anchor
    if self.OnClose then R.pcall(self.OnClose, self) end
    if not self.__keep then self:Remove() end
end

function Popover:__on_toggle(open)
    if not open then self:__closed() end
end

function Popover:__place()
    local sw, sh = N.screen()
    local anchor = self.__anchor
    if anchor and not IsValid(anchor) then return self:Hide() end
    if anchor and self.__match then
        local _, _, aw = anchor:GetScreenBounds()
        N.style(self.__id, "min-width", aw .. "px")
    end
    local _, _, w, h = N.bounds(self.__id)
    local x, y
    if anchor then
        local ax, ay, aw, ah = anchor:GetScreenBounds()
        local side, align, gap = self.__side, self.__align, self.__gap
        if side == "bottom" and ay + ah + gap + h > sh and ay - gap - h >= 0 then side = "top"
        elseif side == "top" and ay - gap - h < 0 and ay + ah + gap + h <= sh then side = "bottom"
        elseif side == "right" and ax + aw + gap + w > sw and ax - gap - w >= 0 then side = "left"
        elseif side == "left" and ax - gap - w < 0 and ax + aw + gap + w <= sw then side = "right" end
        local function along(start, size, own)
            if align == "center" then return start + (size - own) / 2 end
            if align == "end" then return start + size - own end
            return start
        end
        if side == "bottom" then x, y = along(ax, aw, w), ay + ah + gap
        elseif side == "top" then x, y = along(ax, aw, w), ay - gap - h
        elseif side == "right" then x, y = ax + aw + gap, along(ay, ah, h)
        else x, y = ax - gap - w, along(ay, ah, h) end
    else
        x, y = self.__at[1], self.__at[2]
    end
    x = math.max(4, math.min(x, sw - w - 4))
    y = math.max(4, math.min(y, sh - h - 4))
    self:SetPos(x, y)
end

ui.Register("Popover", Popover, "Panel")

local Page = { __native = { tag = "main", class = "ngs-page" }, BindDefault = "Title" }

function Page:SetTitle(t)
    self.__title = t
    if R.page == self then N.title(t) end
end

function Page:GetTitle() return self.__title or self.Title end

ui.Register("Page", Page, "Panel")
