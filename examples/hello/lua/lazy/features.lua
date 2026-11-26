---@class AccentBadge: Label
local BADGE = {}

function BADGE:Initialize()
    self:DockPadding(12, 8, 12, 8)
    self:SetRadius(6)
    self:SetTextColor(color_black)
    self:Apply(self:Inject("accent", Color(92, 156, 255)))
end

function BADGE:Apply(c)
    self:SetBackgroundColor(c)
    self:SetText(string.format("Inject(\"accent\") = Color(%d, %d, %d)", c.r, c.g, c.b))
end

function BADGE:OnContextChanged(_, c) self:Apply(c) end

ui.Register("AccentBadge", BADGE, "Label")

---@class FeaturesPage: DefaultPage
local PAGE = {}
PAGE.Title = "Возможности"

local ACCENTS = {
    { "Синий", Color(92, 156, 255) },
    { "Зелёный", Color(80, 200, 140) },
    { "Оранжевый", Color(255, 160, 70) },
}

function PAGE:Initialize()
    ui.Create("Label", self.Content, { Font = "Big", Text = "Стили, props, контекст, storage" })

    local styles = ui.Create("Card", self.Content, { Title = "SetStyleHover/Active: CSS-классы, Lua на hover не вызывается" })
    local row = ui.Create("Row", styles, { Gap = 12 })
    ui.Create("Button", row, {
        Text = "Наведи и нажми",
        Radius = 20,
        Transition = 0.15,
        StyleHover = { background = Color(92, 156, 255), transform = { y = -2 }, shadow = { 0, 6, 16, Color(0, 0, 0, 120) } },
        StyleActive = { transform = { scale = 0.96 } },
    })
    ui.Create("Button", row, {
        Text = "Градиент",
        Border = false,
        Gradient = { Color(92, 156, 255), Color(170, 90, 255), 135 },
        Transition = 0.2,
        StyleHover = { filter = "brightness(1.2)", shadow = { 0, 0, 0, Color(170, 90, 255), 3 } },
    })
    ui.Create("Button", row, { Text = "Выключена", Enabled = false, StyleDisabled = { alpha = 90, cursor = "not-allowed" } })

    local saved = storage.Get("accent")
    self:Provide("accent", saved and Color(saved.r, saved.g, saved.b) or ACCENTS[1][2])
    local ctx = ui.Create("Card", self.Content, { Title = "Provide/Inject + storage: цвет переживает перезагрузку" })
    ui.Create("AccentBadge", ctx)
    local choices = ui.Create("Row", ctx, { Gap = 8, Slot = "footer" })
    for _, a in ipairs(ACCENTS) do
        ui.Create("Button", choices, {
            Text = a[1],
            DoClick = function()
                storage.Set("accent", a[2])
                ui.SetSkin({ accent = a[2] })
                self:Provide("accent", a[2])
            end,
        })
    end

    local links = ui.Create("Card", self.Content, { Title = "Link:SetURL из данных: javascript: в href не попадёт" })
    links:SetMaxSize(520)
    local input = ui.Create("TextEntry", links, { PlaceholderText = "javascript:alert(1) или https://example.com" })
    input:SetAttr("aria-label", "URL для проверки")
    input:SetAttr("spellcheck", "false")
    local preview = ui.Create("Link", links, { Text = "(ссылка появится здесь)", NewTab = true })
    local verdict = ui.Create("Label", links, { TextColor = Color(154, 160, 170), Text = " " })
    ui.Create("Button", links, {
        Slot = "footer",
        Text = "Поставить в href",
        DoClick = function()
            local url = input:GetValue()
            local ok = preview:SetURL(url)
            preview:SetText(url ~= "" and url or "(пусто)")
            verdict:SetText(ok and "Принято" or "Отклонено: схема не из http, https, mailto")
        end,
    })
end

return ui.Register("FeaturesPage", PAGE, "DefaultPage")
