---@class SecurityPage: DefaultPage
local PAGE = {}
PAGE.Title = "Безопасность"

local MUTED = Color(154, 160, 170)
local WARN = Color(255, 160, 70)

local PLUGIN = [[
-- Отдельная VM: http, storage, cookie и глобалы приложения сюда не попадают.
print("http =", http, "storage =", storage)

local root = ui.Root()
local label = ui.Create("Label", root, { Text = "Нажато: 0" })
local n = 0
ui.Create("Button", root, {
    Text = "Кликни",
    DoClick = function()
        n = n + 1
        label:SetText("Нажато: " .. n)
    end,
})
-- Попробуйте: while true do end, ui.Create("Frame", root), label:SetParent(nil)
]]

function PAGE:BuildState()
    local count = ui.State(0)
    local name = ui.State("Олежек")
    local card = ui.Create("Card", self.Content, { Title = "ui.State: при изменении вызывается только привязанный сеттер" })
    ui.Create("Label", card, { Text = ui.Computed(function() return "Счётчик: " .. count() end) })
    ui.Create("Label", card, {
        TextColor = MUTED,
        Text = ui.Computed(function() return count() % 2 == 0 and "чётный" or "нечётный" end),
    })
    ui.Create("TextEntry", card, { Value = name, PlaceholderText = "Имя (двусторонняя привязка)" })
    ui.Create("Label", card, { Text = ui.Computed(function() return "Привет, " .. name() .. "!" end) })
    local row = ui.Create("Row", card, { Gap = 8, Slot = "footer" })
    ui.Create("Button", row, { Text = "+1", DoClick = function() count:Update(function(n) return n + 1 end) end })
    ui.Create("Button", row, { Text = "Сброс", DoClick = function() count:Set(0) end })
end

function PAGE:BuildSession()
    local me = ui.State(nil)
    local status = ui.State(" ")
    local card = ui.Create("Card", self.Content, { Title = "Сессии: cookie HttpOnly, CSRF-проверка, 5 попыток входа в минуту" })
    card:SetMaxSize(520)
    ui.Create("Label", card, {
        Text = ui.Computed(function()
            local u = me()
            return u and ("Вошли как " .. u.name .. " (" .. u.role .. ")") or "Не вошли. Пароль у всех: lambda"
        end),
    })
    local login = ui.Create("TextEntry", card, { PlaceholderText = "Имя: Олежек, Гарри или Кляйнер" })
    local pass = ui.Create("TextEntry", card, { PlaceholderText = "Пароль", InputType = "password" })
    ui.Create("Label", card, { Text = status, TextColor = WARN })
    local buttons = ui.Create("Row", card, { Gap = 8, Slot = "footer" })
    ui.Create("Button", buttons, {
        Text = "Войти",
        DoClick = function()
            local user, err = http.Post("/api/login", { name = login:GetValue(), password = pass:GetValue() })
            if user then
                me:Set(user)
                status:Set(" ")
                pass:SetValue("")
            else
                status:Set(err.status == 429 and "Слишком много попыток, подождите минуту" or err.message)
            end
        end,
    })
    ui.Create("Button", buttons, {
        Text = "Выйти",
        Enabled = ui.Computed(function() return me() ~= nil end),
        DoClick = function()
            http.Post("/api/logout")
            me:Set(nil)
        end,
    })
    me:Set((http.Get("/api/me")))
end

function PAGE:BuildSandbox()
    local card = ui.Create("Card", self.Content, { Title = "sandbox.New: чужой Lua в отдельной VM с лимитами" })
    local code = ui.Create("TextEntry", card)
    code:SetMultiline(true)
    code:SetFont("Mono")
    code:SetTall(260)
    code:SetValue(PLUGIN)
    local out = ui.Create("Label", card, { Font = "Mono", Wrap = true, TextColor = MUTED, Text = " " })
    local stage = ui.Create("Row", card, { Gap = 8, Align = ALIGN_CENTER })
    stage:SetMinSize(nil, 40)

    local box
    local lines = {}
    local function log(text)
        lines[#lines + 1] = text
        out:SetText(table.concat(lines, "\n"))
    end
    ui.Create("Button", card, {
        Slot = "footer",
        Text = "Запустить",
        DoClick = function()
            if box then box:Close() end
            lines = {}
            out:SetText(" ")
            box = sandbox.New({
                name = "playground",
                root = stage,
                instructions = 1000000,
                memory = 4 * 1024 * 1024,
                on_print = log,
                on_error = function(msg) log("ошибка: " .. msg) end,
            })
            local ok, err = box:Run(code:GetValue(), "playground")
            log(ok and "готово" or ("ошибка: " .. err))
        end,
    })
end

function PAGE:Initialize()
    ui.Create("Label", self.Content, { Font = "Big", Text = "Состояние, сессии, песочница" })
    self:BuildState()
    self:BuildSandbox()
    self:BuildSession()
end

return ui.Register("SecurityPage", PAGE, "DefaultPage")
