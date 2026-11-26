---@class HomePage: DefaultPage
local PAGE = {}
PAGE.Route = "/"
PAGE.Title = "Главная"

function PAGE:Initialize()
    local title = ui.Create("Label", self.Content)
    title:SetFont("Big")
    title:SetText("Фронтенд на Lua")

    local form = self:AddSection("Форма (нативные button / input / select)")
    form:SetMaxSize(460)

    local name = ui.Create("TextEntry", form)
    name:SetPlaceholderText("Новое имя для пользователя 1")

    local role = ui.Create("ComboBox", form)
    role:AddChoice("Игрок", "player", true)
    role:AddChoice("Админ", "admin")

    local remember = ui.Create("CheckBox", form)
    remember:SetText("Запомнить меня")

    local volume = ui.Create("Slider", form)
    local progress = ui.Create("ProgressBar", form)
    progress:SetFraction(0.5)
    volume:SetValue(50)
    function volume:OnValueChanged(v) progress:SetFraction(v / 100) end

    local status = ui.Create("Label", form)
    status:SetTextColor(Color(154, 160, 170))
    status:SetText(" ")

    local save = ui.Create("Button", form)
    save:SetText("Сохранить")
    save.DoClick = function()
        save:SetEnabled(false)
        local user, err = http.Post("/api/users/1/rename", { name = name:GetValue() }, { invalidate = "/api/users" })
        save:SetEnabled(true)
        if err then
            status:SetText("Ошибка " .. err.status .. ": " .. err.message)
            return
        end
        local _, role_id = role:GetSelected()
        status:SetText(string.format("Сохранено: %s, роль %s, запомнить: %s", user.name, role_id, tostring(remember:GetChecked())))
    end
    name.OnEnter = save.DoClick

    local search = self:AddSection("Поиск: запрос прямо в обработчике, предыдущий отменяется")
    search:SetMaxSize(460)
    local query = ui.Create("TextEntry", search)
    query:SetPlaceholderText("Начни печатать имя")
    local results = ui.Create("Column", search)
    results:SetGap(4)
    local pending
    function query:OnValueChange(text)
        if pending then pending:Cancel() end
        pending = http.Start("GET", "/api/search?q=" .. http.Encode(text))
        local found, err = pending:Wait()
        if err then return end
        results:Clear()
        for _, u in ipairs(found) do
            local link = ui.Create("Link", results)
            link:SetText(u.name .. "  (" .. u.role .. ")")
            link:SetURL("/profile/" .. u.id)
        end
    end
    query:OnValueChange("")

    local dock = self:AddSection("Dock: TOP, LEFT, BOTTOM, RIGHT, FILL в одном grid")
    local area = ui.Create("Panel", dock)
    area:SetTall(200)
    local colors = {
        { DOCK_TOP, "TOP", Color(92, 156, 255) },
        { DOCK_LEFT, "LEFT", Color(255, 170, 70) },
        { DOCK_BOTTOM, "BOTTOM", Color(90, 200, 140) },
        { DOCK_RIGHT, "RIGHT", Color(220, 100, 200) },
        { DOCK_FILL, "FILL", Color(70, 76, 86) },
    }
    for _, d in ipairs(colors) do
        local p = ui.Create("Label", area)
        p:Dock(d[1])
        p:SetSize(90, 36)
        p:DockMargin(3, 3, 3, 3)
        p:SetContentAlignment(5)
        p:SetBackgroundColor(d[3])
        p:SetTextColor(color_black)
        p:SetText(d[2])
    end
end

ui.Register("HomePage", PAGE, "DefaultPage")
