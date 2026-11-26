---@class NotFoundPage: DefaultPage
local PAGE = {}
PAGE.Title = "Не найдено"

function PAGE:Initialize()
    local title = ui.Create("Label", self.Content)
    title:SetFont("Big")
    title:SetText("404")

    local text = ui.Create("Label", self.Content)
    text:SetText("Страницы " .. self.Path .. " нет")

    local back = ui.Create("Link", self.Content)
    back:SetText("На главную")
    back:SetURL("/")
end

ui.Register("NotFoundPage", PAGE, "DefaultPage")
