---@class FormsPage: DefaultPage
local PAGE = {}
PAGE.Title = "Формы"

local MUTED = Color(154, 160, 170)

local function centered(section)
    section:SetWide("100%")
    section:SetMaxSize(520)
    section:SetAlignSelf("center")
    return section
end

local function note(parent, text)
    local label = ui.Create("Label", parent, { Text = text })
    label:SetTextColor(MUTED)
    label:SetWrap(true)
    return label
end

function PAGE:BuildSignup()
    local section = self:AddSection("Регистрация: проверки на клиенте, затем Valid<Signup> на сервере")
    centered(section)
    note(section, "Клиент проверяет обязательные поля, формат почты, длину пароля и совпадение паролей. "
        .. "Сервер дополнительно отвечает 422, если имя занято (попробуйте «Олежек»), почта на example.com, "
        .. "возраст меньше 14 или не принято соглашение.")

    local form = ui.Create("Form", section)
    form:SetAction("POST", "/api/forms/signup")
    form:AddField("name", "TextEntry", { Label = "Имя", Required = true, PlaceholderText = "Гордон" })
    form:AddField("email", "TextEntry", {
        Label = "Почта", Required = true, InputType = "email", PlaceholderText = "gordon@blackmesa.org",
        Validate = function(v) if not v:match("^[^@%s]+@[^@%s]+%.[^@%s]+$") then return "Это не похоже на адрес почты" end end,
    })
    form:AddField("password", "TextEntry", {
        Label = "Пароль", Required = true, InputType = "password", Hint = "Не короче 8 символов",
        Validate = function(v) if #v < 8 then return "Слишком короткий пароль" end end,
    })
    form:AddField("confirm", "TextEntry", { Label = "Пароль ещё раз", Required = true, InputType = "password" })
    form:AddField("age", "TextEntry", { Label = "Возраст", Numeric = true, Hint = "Необязательно" })
    local role = form:AddField("role", "ComboBox", { Label = "Роль" })
    role:AddChoice("Игрок", "player", true)
    role:AddChoice("Учёный", "scientist")
    role:AddChoice("Охрана", "security")
    form:AddField("agree", "CheckBox", { Text = "Принимаю правила Black Mesa" })
    form:AddSubmit("Зарегистрироваться")

    -- Проверка, которой нужны два поля сразу.
    function form:Validate(values)
        if values.password ~= values.confirm then return { confirm = "Пароли не совпадают" } end
    end

    -- Подтверждение пароля серверу не нужно.
    function form:OnBeforeSubmit(values)
        local body = {}
        for k, v in pairs(values) do body[k] = v end
        body.confirm = nil
        return body
    end

    local result = note(section, " ")
    ---@param account SignupResult
    function form:OnSuccess(account)
        result:SetText(string.format("Аккаунт #%d создан для %s", account.id, account.name))
        ui.Toast("Добро пожаловать, " .. account.name)
    end
end

function PAGE:BuildPromo()
    local section = self:AddSection("Ошибка всей формы: FieldErrors::message без конкретного поля")
    centered(section)
    note(section, "Рабочий код — OLEZHEK. Любой другой сервер отклоняет сообщением над полями, "
        .. "а свой OnFailure здесь не задан, поэтому Form показывает его сам.")

    local form = ui.Create("Form", section)
    form:SetAction("POST", "/api/forms/promo")
    local code = form:AddField("code", "TextEntry", { Label = "Промокод", PlaceholderText = "OLEZHEK" })
    form:AddSubmit("Применить")

    local result = note(section, " ")
    function form:OnSuccess(promo)
        result:SetText(string.format("Скидка %d%% по коду %s", promo.discount, code:GetValue():upper()))
    end
end

function PAGE:BuildManual()
    local section = self:AddSection("Без виджета Form: http.Post, err.fields и ui.ApplyErrors")
    centered(section)
    note(section, "Те же ответы 422 можно разобрать вручную: поля с SetName получают ошибки, "
        .. "а то, чему поле не нашлось, возвращается из ApplyErrors.")

    local row = ui.Create("Row", section)
    row:SetGap(8)
    local code = ui.Create("TextEntry", row)
    code:SetName("code")
    code:SetPlaceholderText("Промокод")
    code:SetWide(220)
    local apply = ui.Create("Button", row, { Text = "Проверить" })
    local status = note(section, " ")

    apply.DoClick = function()
        apply:SetEnabled(false)
        local promo, err = http.Post("/api/forms/promo", { code = code:GetValue() })
        apply:SetEnabled(true)
        if err then
            ui.ApplyErrors(section, err.fields)
            status:SetText(err.fields and err.fields.code and "" or err.message)
            return
        end
        ui.ApplyErrors(section, {})
        status:SetText("Подошёл: скидка " .. promo.discount .. "%")
    end
    code.OnEnter = apply.DoClick
end

function PAGE:Initialize()
    local title = ui.Create("Label", self.Content)
    title:SetFont("Big")
    title:SetText("Формы и валидация")

    self:BuildSignup()
    self:BuildPromo()
    self:BuildManual()
end

return ui.Register("FormsPage", PAGE, "DefaultPage")
