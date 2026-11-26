---@class CursorPad: Panel
local PAD = {}

function PAD:Initialize()
    self:SetTall(170)
    self:SetCursor("crosshair")
    self.X, self.Y = -1, -1
end

function PAD:OnCursorMoved(x, y)
    self.X, self.Y = x, y
    self:InvalidatePaint()
end

function PAD:OnCursorExited()
    self.X, self.Y = -1, -1
    self:InvalidatePaint()
end

function PAD:Paint(w, h)
    draw.RoundedBox(10, 0, 0, w, h, Color(37, 40, 45))
    if self.X < 0 then
        draw.SimpleText("Наведи курсор", "Default", w / 2, h / 2, Color(154, 160, 170), TEXT_ALIGN_CENTER, TEXT_ALIGN_CENTER)
        return
    end
    surface.SetDrawColor(92, 156, 255, 120)
    surface.DrawLine(self.X, 0, self.X, h)
    surface.DrawLine(0, self.Y, w, self.Y)
    draw.Circle(self.X, self.Y, 5, Color(255, 170, 70))
    draw.SimpleText(string.format("LocalCursorPos: %d, %d", self.X, self.Y), "Mono", 14, 12, color_white)
end

ui.Register("CursorPad", PAD, "Panel")

---@class WidgetsPage: DefaultPage
local PAGE = {}
PAGE.Title = "Виджеты"

function PAGE:Initialize()
    local title = ui.Create("Label", self.Content)
    title:SetFont("Big")
    title:SetText("Окна, меню, курсор, шрифты")

    local windows = self:AddSection("Frame на <dialog>")
    local row = ui.Create("Row", windows)
    row:SetGap(10)
    local result = ui.Create("Label", windows)
    result:SetTextColor(Color(154, 160, 170))
    result:SetText(" ")

    local open = ui.Create("Button", row)
    open:SetText("Обычное окно")
    open.DoClick = function()
        local frame = ui.Create("Frame")
        frame:SetTitle("Перетаскивай за шапку")
        frame:SetSize(360, 220)
        frame:SetSizable(true)
        frame:SetGap(8)
        local text = ui.Create("Label", frame)
        text:SetWrap(true)
        text:SetText("Окно не модальное: страница под ним работает. Угол справа внизу меняет размер.")
        local close = ui.Create("Button", frame)
        close:SetText("Закрыть")
        close.DoClick = function() frame:Close() end
        frame.OnClose = function() result:SetText("Обычное окно закрыто") end
    end

    local modal = ui.Create("Button", row)
    modal:SetText("Модальное окно")
    modal.DoClick = function()
        local frame = ui.Create("Frame")
        frame:SetTitle("Подтверждение")
        frame:SetSize(340, 170)
        frame:SetBackgroundBlur(true)
        frame:MakePopup()
        local text = ui.Create("Label", frame)
        text:SetText("Удалить всё? Esc закрывает окно.")
        local buttons = ui.Create("Row", frame)
        buttons:Dock(DOCK_BOTTOM)
        buttons:SetGap(8)
        buttons:SetJustify(JUSTIFY_END)
        local no = ui.Create("Button", buttons)
        no:SetText("Отмена")
        no.DoClick = function() frame:Close() end
        local yes = ui.Create("Button", buttons)
        yes:SetText("Удалить")
        yes.DoClick = function()
            result:SetText("Подтверждено")
            frame:Close()
        end
        -- Right click inside a modal: the menu is moved into the dialog so it stays clickable.
        function frame:DoRightClick()
            local menu = ui.Menu()
            menu:AddOption("Меню внутри модального окна", function() result:SetText("Меню из модального окна") end)
            menu:Open()
        end
    end

    local menus = self:AddSection("Menu на Popover API: правый клик по полю ниже")
    local target = ui.Create("Label", menus)
    target:SetTall(90)
    target:SetMouseInputEnabled(true)
    target:SetContentAlignment(5)
    target:SetBackgroundColor(Color(30, 33, 38))
    target:SetText("Правый клик сюда")
    local chosen = ui.Create("Label", menus)
    chosen:SetTextColor(Color(154, 160, 170))
    chosen:SetText(" ")

    function target:DoRightClick()
        local menu = ui.Menu()
        menu:AddOption("Копировать", function() chosen:SetText("Выбрано: Копировать") end)
        menu:AddOption("Вставить", function() chosen:SetText("Выбрано: Вставить") end):SetEnabled(false)
        menu:AddSpacer()
        local sub = menu:AddSubMenu("Цвет")
        for _, c in ipairs({ { "Синий", Color(40, 70, 130) }, { "Зелёный", Color(35, 95, 60) }, { "Красный", Color(120, 40, 45) } }) do
            sub:AddOption(c[1], function()
                target:SetBackgroundColor(c[2])
                chosen:SetText("Цвет: " .. c[1])
            end)
        end
        menu:AddSpacer()
        menu:AddOption("Открыть окно", function() open:DoClick() end)
        menu:Open()
    end

    local pops = self:AddSection("Popover: любое содержимое рядом с кнопкой")
    local prow = ui.Create("Row", pops, { Gap = 10, Align = ALIGN_CENTER })
    local picked = ui.Create("Label", pops, { Text = " ", TextColor = Color(154, 160, 170) })

    local pick = ui.Create("Button", prow, { Text = "Выбрать день" })
    local calendar = ui.Create("Popover", prow)
    calendar:SetGap(8)
    ui.Create("Label", calendar, { Text = "Ноябрь", Font = "DefaultBold" })
    local days = ui.Create("Grid", calendar, { Columns = { 32, 32, 32, 32, 32, 32, 32 }, Gap = 4 })
    for d = 1, 30 do
        ui.Create("Button", days, {
            Text = d,
            Size = { 32, 32 },
            Style = { fontVariantNumeric = "tabular-nums", padding = 0 },
            DoClick = function()
                picked:SetText("Выбран день: " .. d)
                calendar:Hide()
            end,
        })
    end
    pick.DoClick = function() calendar:Toggle(pick) end

    local filter = ui.Create("Button", prow, { Text = "Фильтр справа" })
    local checks = ui.Create("Popover", prow)
    checks:SetSide("right")
    checks:SetGap(10)
    for _, name in ipairs({ "Tip", "Subscription", "PPV", "Message" }) do
        ui.Create("CheckBox", checks, { Text = name, Checked = true })
    end
    filter.DoClick = function() checks:Toggle(filter) end
    local cursor = self:AddSection("LocalCursorPos и OnCursorMoved")
    ui.Create("CursorPad", cursor)

    local fonts = self:AddSection("Веб-шрифты: surface.CreateFont({ url = ... })")
    local serif = ui.Create("Label", fonts)
    serif:SetFont("Big")
    serif:SetText("Source Serif 4 — Съешь же ещё этих мягких булок")
    local mono = ui.Create("Label", fonts)
    mono:SetFont("Mono")
    mono:SetText("Fira Mono: local x = ui.Create(\"Button\", self)")
    local status = ui.Create("Label", fonts)
    status:SetTextColor(Color(154, 160, 170))
    local function refresh()
        if not IsValid(status) then return end
        local ready = {}
        for _, family in ipairs({ "Source Serif 4", "Fira Mono" }) do
            if surface.IsFontLoaded(family) then ready[#ready + 1] = family end
        end
        status:SetText(#ready == 0 and "Шрифты грузятся..." or "Загружено: " .. table.concat(ready, ", "))
    end
    refresh()
    self:Hook("OnFontLoaded", refresh)
end

return ui.Register("WidgetsPage", PAGE, "DefaultPage")
