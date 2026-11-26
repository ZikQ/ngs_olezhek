local Page = { Title = "Компоненты", KeepAlive = true }

function Page.Loader()
    return { scheme = browser.ColorScheme() }
end

function Page:Initialize()
    local tabs = self.Content:Add("Tabs", {Dock = DOCK_FILL})
    local tableTab = tabs:AddTab("Таблица")
    tableTab:SetGrow(1)
    local treeTab = tabs:AddTab("Дерево")
    local browserTab = tabs:AddTab("Браузер")
    local grid = tableTab:Add("DataGrid", {Dock = DOCK_FILL})
    grid:SetColumns({{key = "id", title = "№", width = 80}, {key = "label", title = "Название"}})
    grid:SetDataSource(function(offset, limit)
        return http.Get(("/api/rows?offset=%d&limit=%d"):format(offset, limit))
    end, 64)

    local folders = treeTab:Add("Panel", {Gap = 16})
    local tree = folders:Add("Tree")
    tree:SetMaxSize(360)
    local node = tree:AddNode("Документы", {"Отчёты", "Письма"})
    node:AddNode("Отчёты", {"Итоги квартала", "Отчёт по проекту"})
    node:AddNode("Письма", {"Входящее письмо", "Ответ заказчику"})
    local heading = folders:Add("Label", {Text = "Содержимое папки", Font = "DefaultBold"})
    local contents = folders:Add("Column", {Gap = 6})
    tree.OnSelect = function(_, item, entries)
        heading:SetText("Содержимое папки «" .. item.Toggle:GetText() .. "»")
        contents:Clear()
        for _, entry in ipairs(entries) do contents:Add("Label", {Text = entry}) end
    end
    node:Select()

    local options = browserTab:Add("Dropdown", {Text = "Выбрать тему"})
    options:AddOption("Светлая", "light")
    options:AddOption("Тёмная", "dark")
    options.OnSelect = function(_, _, scheme) cookie.Set("theme", scheme); ui.Toast("Выбор сохранён") end

    browserTab:Add("Button", {Text = "Проверить WebSocket", DoClick = function()
        browser.WebSocket("/api/echo", {
            open = function(socket) socket:Send("WebSocket подключён") end,
            message = function(socket, text) socket:Close(); ui.Toast(text) end,
            error = function(socket) socket:Close(); ui.Toast("Ошибка WebSocket") end,
        }, self)
    end})
    browserTab:Add("Button", {Text = "Проверить SSE", DoClick = function()
        browser.EventSource("/api/events", {
            message = function(stream, text) stream:Close(); ui.Toast(text) end,
            error = function(stream) stream:Close(); ui.Toast("Ошибка SSE") end,
        }, self)
    end})
    local files = browserTab:Add("FileInput", {Multiple = true})
    files.OnChange = function(input, selected)
        if #selected == 0 then return end
        local names, err = http.Upload("/api/upload", input)
        ui.Toast(err and err.message or table.concat(names, ", "))
    end

    local details = browserTab:Add("Disclosure", {Text = "Системная тема"})
    local scheme = details.Body:Add("Label", {Text = self.Data.scheme})
    self:Hook("OnColorSchemeChanged", function(value) scheme:SetText(value) end)
    local copy = browserTab:Add("Button", {Text = "Скопировать путь"})
    copy:SetRichTooltip(function(tip) tip:Add("Label", {Text = router.GetPath()}) end)
    copy.DoClick = function()
        local ok, err = clipboard.WriteText(router.GetPath())
        ui.Toast(ok and "Скопировано" or err)
    end
    browserTab:Add("Button", {Text = "Сбросить выбор темы", DoClick = function()
        ui.Confirm("Удалить сохранённую тему?", function(ok)
            if ok then cookie.Remove("theme"); ui.Toast("Тема сброшена") end
        end, "Тема", self)
    end})
end
return ui.Register("LibraryPage", Page, "DefaultPage")
