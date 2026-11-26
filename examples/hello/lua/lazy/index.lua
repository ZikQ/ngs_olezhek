---@class HomePage: DefaultPage
local PAGE = {}
PAGE.KeepAlive = true
PAGE.Title = "Главная"

function PAGE:Initialize()
    local title = ui.Create("Label", self.Content)
    title:SetFont("Big")
    title:SetText("Фронтенд на Lua")

    local section = self:AddSection("Форма: проверка на клиенте и ошибки полей от сервера (422)")
    section:SetMaxSize(460)

    local form = ui.Create("Form", section)
    form:SetAction("POST", "/api/users/1/rename", { invalidate = "/api/users" })
    form:AddField("name", "TextEntry", { Label = "Имя", Required = true, PlaceholderText = "Новое имя для пользователя 1",
        Hint = "Сервер не примет «admin» и имена длиннее 20 символов" })
    local role = form:AddField("role", "ComboBox", { Label = "Роль" })
    role:AddChoice("Игрок", "player", true)
    role:AddChoice("Админ", "admin")
    form:AddField("remember", "CheckBox", { Text = "Запомнить меня" })
    form:AddSubmit("Сохранить")

    local volume = ui.Create("Slider", section)
    local progress = ui.Create("ProgressBar", section)
    progress:SetFraction(0.5)
    volume:SetValue(50)
    function volume:OnValueChanged(v) progress:SetFraction(v / 100) end

    local status = ui.Create("Label", section)
    status:SetTextColor(Color(154, 160, 170))
    status:SetText(" ")

    ---@param user User
    function form:OnSuccess(user)
        local values = self:GetValues()
        status:SetText(string.format("Сохранено: %s, роль %s, запомнить: %s", user.name, user.role, tostring(values.remember)))
    end

    local search = self:AddSection("Поиск: запрос прямо в обработчике, предыдущий отменяется")
    search:SetMaxSize(460)
    local query = ui.Create("TextEntry", search)
    query:SetName("search")
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

return ui.Register("HomePage", PAGE, "DefaultPage")
