local Page = { Title = "Набор компонентов" }

local MONEY = function(v) return "$" .. math.floor(v + 0.5) end

local LOGOUT = { "M9 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h4", "M16 17l5-5-5-5", "M21 12H9" }
local MOON = "M21 12.8A9 9 0 1 1 11.2 3a7 7 0 0 0 9.8 9.8z"
local SUN = { "M16 12a4 4 0 1 1-8 0a4 4 0 1 1 8 0",
    "M12 2v2M12 20v2M4.9 4.9l1.4 1.4M17.7 17.7l1.4 1.4M2 12h2M20 12h2M4.9 19.1l1.4-1.4M17.7 6.3l1.4-1.4" }

local function row(parent)
    return parent:Add("Row", { Gap = 12, Align = ALIGN_CENTER, LayoutWrap = true })
end

function Page:Initialize()
    ui.SetVars({ series1 = Color(212, 137, 10), series2 = Color(226, 77, 139), series3 = Color(51, 140, 237),
        series4 = Color(0, 168, 136), positive = Color(61, 220, 151) })

    local first = self:AddSection("Кнопки, значки, статусы")
    local buttons = row(first)
    for _, v in ipairs({ "default", "primary", "outline", "danger", "ghost" }) do
        buttons:Add("Button", { Text = v, Variant = v })
    end
    buttons:Add("Button", { Text = "выключена", Variant = "primary", Enabled = false })
    local logout = buttons:Add("Button", { Variant = "ghost", Size = { 34, 34 }, Tooltip = "Выйти" })
    logout:DockPadding(0, 0, 0, 0)
    logout:Add("Icon", { Icon = LOGOUT, Color = "danger" })

    local badges = row(first)
    for _, v in ipairs({ "neutral", "accent", "success", "warning", "danger" }) do
        badges:Add("Badge", { Text = v, Variant = v })
    end
    badges:Add("Status", { Status = "online", Text = "на смене", Pulse = true })
    badges:Add("Status", { Status = "idle", Text = "отошёл" })
    badges:Add("Status", { Status = "offline", Text = "офлайн" })
    badges:Add("Spinner")

    local second = self:AddSection("Аватары и полосы")
    local people = row(second)
    for i, name in ipairs({ "Анна Волкова", "Дмитрий Орлов", "@lily_rose", "Мария" }) do
        people:Add("Avatar", { Name = name, AvatarSize = ({ "sm", "md", "lg", 40 })[i] })
    end
    local meters = second:Add("Column", { Gap = 10, MaxSize = 420 })
    for _, m in ipairs({ { 1, "success" }, { 0.75, "accent" }, { 0.5, "warning" }, { 0.2, "danger" } }) do
        meters:Add("Meter", { Fraction = m[1], Variant = m[2], ShowLabel = true })
    end

    local inputs = self:AddSection("Выбор")
    local line1 = row(inputs)
    local seg = line1:Add("Segmented")
    seg:AddOption("Месяц", "month")
    seg:AddOption("Всё время", "all")
    seg:SetValue("month")
    local theme = line1:Add("Segmented")
    for _, opt in ipairs({ { MOON, "dark" }, { SUN, "light" } }) do
        local b = theme:AddOption("", opt[2])
        b:SetSize(32, 28)
        b:DockPadding(0, 0, 0, 0)
        b:Add("Icon", { Icon = opt[1], Size = { 16, 16 } })
    end
    theme:SetValue("dark")

    local select = line1:Add("Select", { Placeholder = "Все модели", Wide = 200 })
    select:SetOptions({ "@lily_rose", "@mia_vibes", "@sophie_k", "@nika_live" })
    local from = line1:Add("DatePicker", { Placeholder = "Дата от", Wide = 150 })
    local to = line1:Add("DatePicker", { Placeholder = "Дата до", Wide = 150 })
    local picked = inputs:Add("Label", { Text = "Ничего не выбрано" })
    local function show()
        local parts = { "период: " .. tostring(seg:GetValue()), "модель: " .. tostring(select:GetValue() or "все") }
        if from:GetValue() then parts[#parts + 1] = "с " .. date.Format(from:GetValue(), "DD.MM.YYYY") end
        if to:GetValue() then parts[#parts + 1] = "по " .. date.Format(to:GetValue(), "DD.MM.YYYY") end
        picked:SetText(table.concat(parts, " · "))
    end
    seg.OnSelect, select.OnSelect, from.OnChange, to.OnChange = show, show, show, show
    local clock = inputs:Add("Label")
    local function tick() clock:SetText("Сейчас: " .. date.Format(date.Now(), "D MMMM YYYY, HH:mm:ss")) end
    tick()
    clock:Timer("clock", 1, 0, tick)
    local focus = inputs:Add("Label", { Text = "Вкладка: на экране" })
    self:Hook("OnVisibilityChanged", function(v) focus:SetText(v and "Вкладка: на экране" or "Вкладка: скрыта") end)
    self:Hook("OnWindowFocusChanged", function(f) focus:SetText(f and "Окно: в фокусе" or "Окно: не в фокусе") end)

    local dialogs = row(self:AddSection("Окна и уведомления"))
    dialogs:Add("Button", { Text = "Подтверждение", DoClick = function()
        ui.Confirm({ Title = "Завершить смену", Accept = "Да", Cancel = "Нет", CancelVariant = "danger",
            Build = function(body) body:Add("Label", { Text = "Завершить смену на @lily_rose?", Wrap = true }) end },
            function(ok) ui.Toast(ok and "Смена завершена" or "Отменено", 3, ok and "success" or "info") end)
    end })
    dialogs:Add("Button", { Text = "Dialog", DoClick = function()
        local d = ui.Create("Dialog")
        d:SetTitle("Добавить чаттера")
        d:Add("TextEntry", { PlaceholderText = "@username" })
        d:AddButton("Сохранить", "primary", function(dlg) ui.Toast("Сохранено", 3, "success"); dlg:Close() end)
        d:AddButton("Отмена", "outline")
        d:MakePopup()
    end })
    for _, k in ipairs({ "info", "success", "warning", "danger" }) do
        dialogs:Add("Button", { Text = "toast " .. k, Variant = "ghost", DoClick = function() ui.Toast("Уведомление " .. k, 3, k) end })
    end

    local list = self:AddSection("ListView")
    local lv = list:Add("ListView")
    lv:SetColumns({
        { key = "time", title = "Время", width = 90, sort = "ts", style = { color = Color(154, 160, 170) } },
        { key = "model", title = "Модель", sort = true },
        { title = "Тип", width = 130, build = function(cell, r)
            cell:Add("Badge", { Text = r.kind, Variant = ({ PPV = "success", Tip = "warning", Subscription = "accent" })[r.kind] or "neutral" })
        end },
        { key = "amount", title = "Сумма", width = 120, align = "right", sort = true, format = MONEY,
          style = { color = ui.Var("positive"), fontWeight = 500 } },
    })
    local rows, kinds, models = {}, { "PPV", "Tip", "Subscription", "Message" }, { "@lily_rose", "@mia_vibes", "@sophie_k" }
    for i = 1, 23 do
        rows[i] = { ts = 100 - i, time = string.format("18:%02d", 60 - i * 2), model = models[i % 3 + 1],
            kind = kinds[i % 4 + 1], amount = (i * 37) % 90 + 5 }
    end
    lv:SetRows(rows)
    lv:SetPageSize(8)
    lv.MoreButton:SetText("Загрузить ещё")
    lv.OnRowClick = function(_, r) ui.Toast(r.model .. ": " .. MONEY(r.amount)) end
    lv.OnRowRightClick = function(_, r)
        local menu = ui.Menu()
        menu:AddOption("Удалить " .. MONEY(r.amount), function() ui.Toast("Удаление " .. r.model, 3, "danger") end)
        menu:Open()
    end

    local charts = self:AddSection("Графики")
    local spark = charts:Add("Sparkline", { MaxSize = 320 })
    spark:SetValues({ 3, 5, 4, 8, 6, 9, 7, 12, 10, 11 })
    local days = {}
    for i = 1, 26 do days[i] = { label = string.format("%02d.11", i), value = 300 + math.floor(math.sin(i / 3) * 200 + i * 15) } end
    local lc = charts:Add("LineChart")
    lc:SetFormat(MONEY)
    lc:SetPoints(days)
    local grid = charts:Add("Grid", { Columns = 2, Gap = 16 })
    local donut = grid:Add("DonutChart", { CenterLabel = "всего" })
    donut:SetFormat(MONEY)
    donut:SetSlices({ { label = "Subscription", value = 4210 }, { label = "PPV", value = 3990 },
        { label = "Tip", value = 2140 }, { label = "Message", value = 1620 } })
    local hbar = grid:Add("BarChart")
    hbar:SetFormat(MONEY)
    hbar:SetBars({ { label = "lily_rose", value = 1978 }, { label = "mia_vibes", value = 1535 },
        { label = "eva_moon", value = 1415 }, { label = "sophie_k", value = 1237 } })
    local hours = {}
    for h = 0, 23 do hours[#hours + 1] = { label = string.format("%02d", h), value = 100 + (h * 53) % 470 } end
    local vbar = charts:Add("BarChart", { Orientation = "vertical" })
    vbar:SetFormat(MONEY)
    vbar:SetBars(hours)
end

return ui.Register("KitPage", Page, "DefaultPage")
