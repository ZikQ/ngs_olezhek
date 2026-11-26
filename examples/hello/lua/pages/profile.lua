---@class ProfilePage: DefaultPage
local PAGE = {}
PAGE.Route = "/profile/{id}"

function PAGE:Initialize()
    local title = ui.Create("Label", self.Content)
    title:SetFont("Big")
    title:SetText("Загрузка...")

    local btn = ui.Create("Button", self)
    btn:Dock(DOCK_BOTTOM)
    btn:DockMargin(24, 0, 24, 16)
    btn:SetText("Сохранить")
    btn.DoClick = function() print("saved") end

    -- Initialize can wait: if the user leaves the page meanwhile, this coroutine is dropped.
    -- UseQuery caches the answer: coming back shows it at once, and if the background refresh
    -- brings something new (say, after a rename on the home page) the page is rebuilt.
    local user, err = self:UseQuery("/api/users/" .. self.Params.id)
    if err then
        title:SetText("Ошибка " .. err.status)
        local msg = ui.Create("Label", self.Content)
        msg:SetText(err.message)
        return
    end

    title:SetText(user.name)
    self:SetTitle("Профиль: " .. user.name)

    local info = self:AddSection("Данные с сервера")
    for _, row in ipairs({ { "id", user.id }, { "Имя", user.name }, { "Роль", user.role } }) do
        local line = ui.Create("Row", info)
        line:SetGap(8)
        local k = ui.Create("Label", line)
        k:SetWide(60)
        k:SetTextColor(Color(154, 160, 170))
        k:SetText(row[1])
        local v = ui.Create("Label", line)
        v:SetText(tostring(row[2]))
    end

    local nav = ui.Create("Row", self.Content)
    nav:SetGap(16)
    local prev = ui.Create("Link", nav)
    prev:SetText("← Предыдущий")
    prev:SetURL("/profile/" .. (user.id - 1))
    local next_link = ui.Create("Link", nav)
    next_link:SetText("Следующий →")
    next_link:SetURL("/profile/" .. (user.id + 1))
end

ui.Register("ProfilePage", PAGE, "DefaultPage")
