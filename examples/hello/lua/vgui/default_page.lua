---@class DefaultPage: Page
---@field Content ScrollPanel
local PAGE = {}

local NAV = {
    { "Главная", "/" },
    { "Профиль", "/profile/1" },
    { "Формы", "/forms" },
    { "Paint", "/paint" },
    { "Виджеты", "/widgets" },
    { "Возможности", "/features" },
    { "Компоненты", "/library" },
    { "Набор", "/kit" },
    { "Безопасность", "/security" },
    { "Нет такой", "/nowhere" },
}

local Layout = {}
function Layout:Initialize()
    local header = ui.Create("Row", self)
    header:Dock(DOCK_TOP)
    header:SetTall(54)
    header:SetAlign(ALIGN_CENTER)
    header:SetGap(20)
    header:DockPadding(24, 0, 24, 0)
    header:SetBackgroundColor(Color(37, 40, 45))

    local logo = ui.Create("Label", header)
    logo:SetFont("Header")
    logo:SetText("ngs_olezhek")

    for _, item in ipairs(NAV) do
        local link = ui.Create("Link", header)
        link:SetText(item[1])
        link:SetURL(item[2])
    end

    local outlet = self:Add("Panel")
    outlet:Dock(DOCK_FILL)
    self:SetOutlet(outlet)
end
ui.Register("AppLayout", Layout, "Panel")

PAGE.Layout = "AppLayout"
function PAGE:Initialize()
    self.Content = ui.Create("ScrollPanel", self)
    self.Content:Dock(DOCK_FILL)
    self.Content:DockPadding(24, 20, 24, 24)
    self.Content:SetGap(14)
end

function PAGE:AddSection(title)
    local section = ui.Create("Panel", self.Content)
    section:SetGap(10)
    section:DockPadding(18, 16, 18, 18)
    section:SetBackgroundColor(Color(37, 40, 45))

    local label = ui.Create("Label", section)
    label:SetFont("DefaultBold")
    label:SetTextColor(Color(154, 160, 170))
    label:SetText(title)
    return section
end

ui.Register("DefaultPage", PAGE, "Page")
