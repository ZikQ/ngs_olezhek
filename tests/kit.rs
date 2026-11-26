//! Component kit (kit.lua, charts.lua, calendar.lua) and the date module, on the mocked browser.

use ngs_olezhek::testing::Runtime;

fn run(script: &str) {
    let lua = Runtime::new().lua().clone();
    lua.load(script).set_name("@test").exec().unwrap_or_else(|e| panic!("{e}"));
    let left: Vec<String> = lua.load("return MOCK.errors").eval().unwrap();
    assert!(left.is_empty(), "unexpected runtime errors: {left:#?}");
}

#[test]
fn date_civil_math_parse_and_format() {
    run(r##"
        assert(date.Time(1970, 1, 1, 0, 0, 0, 0) == 0)
        assert(date.Time(2026, 11, 26, 18, 41, 0, 0) == 1795718460)
        local f = date.Fields(1795718460, 0)
        assert(f.year == 2026 and f.month == 11 and f.day == 26 and f.hour == 18 and f.min == 41)
        assert(f.wday == 4, "26.11.2026 is a Thursday")
        assert(date.Fields(-1, 0).year == 1969)
        assert(date.Time(2026, 13, 1, 0, 0, 0, 0) == date.Time(2027, 1, 1, 0, 0, 0, 0), "month overflow")
        assert(date.DaysInMonth(2024, 2) == 29 and date.DaysInMonth(2100, 2) == 28)

        assert(date.Parse("2026-11-26T18:41:00Z") == 1795718460)
        assert(date.Parse("2026-11-26T18:41:00") == 1795718460, "no zone is UTC")
        assert(date.Parse("2026-11-26T21:41:00+03:00") == 1795718460)
        assert(math.abs(date.Parse("2026-11-26T18:41:00.250Z") - 1795718460.25) < 1e-6)
        assert(date.Parse("2026-11-26") == date.Time(2026, 11, 26, 0, 0, 0, 0))
        assert(date.Parse("2026-02-30") == nil and date.Parse("nope") == nil and date.Parse(nil) == nil)

        date.SetOffset(3 * 3600)
        assert(date.Format(1795718460, "DD.MM HH:mm") == "26.11 21:41")
        assert(date.Format(1795718460, "D MMM YYYY [г.]") == "26 Nov 2026 г.")
        assert(date.ISO(1795718460) == "2026-11-26T18:41:00Z")
        local midnight = date.StartOfDay(1795718460)
        assert(date.Format(midnight, "HH:mm") == "00:00")
        assert(date.Format(date.StartOfMonth(1795718460), "D") == "1")
        assert(date.Format(date.AddMonths(date.Time(2026, 1, 31), 1), "DD.MM") == "28.02")
        assert(date.SameDay(midnight, midnight + 86399) and not date.SameDay(midnight, midnight + 86400))

        date.SetNow(1000)
        MOCK.advance(5)
        assert(date.Now() == 1005)

        date.SetLocale({ monthsShort = { "янв.", "февр.", "мар.", "апр.", "мая", "июн.", "июл.", "авг.", "сент.", "окт.", "нояб.", "дек." } })
        assert(date.Format(1795718460, "D MMM YYYY") == "26 нояб. 2026")
        assert(not pcall(date.SetLocale, { nope = 1 }))
    "##);
}

#[test]
fn date_follows_the_browser_clock_and_zone() {
    run(r##"
        assert(date.Now() == math.floor(MOCK.clock + MOCK.now))
        MOCK.advance(90)
        assert(date.Now() == math.floor(MOCK.clock + MOCK.now))
        assert(browser.Now() == MOCK.clock + MOCK.now)

        -- A zone that moves from +2 to +3 at 2026-03-29 01:00 UTC, like Eastern Europe.
        local switch = date.Time(2026, 3, 29, 1, 0, 0, 0)
        MOCK.tz = function(u) return u >= switch and 3 * 3600 or 2 * 3600 end
        assert(date.GetOffset(switch - 1) == 7200 and date.GetOffset(switch) == 10800)
        assert(date.Format(date.Time(2026, 3, 28, 12, 0, 0, 0), "HH:mm") == "14:00")
        assert(date.Format(date.Time(2026, 3, 30, 12, 0, 0, 0), "HH:mm") == "15:00")
        local before, after = date.Time(2026, 3, 28), date.Time(2026, 3, 30)
        assert(date.Fields(before).hour == 0 and date.Fields(after).hour == 0, "local midnights on both sides")
        assert(after - before == 2 * 86400 - 3600, "the night of the switch is an hour short")

        date.SetOffset(0)
        assert(date.GetOffset() == 0 and date.Format(switch, "HH:mm") == "01:00")
        date.SetOffset(nil)
        assert(date.GetOffset(switch) == 10800)
        assert(browser.TimezoneOffset(switch) == 10800)
    "##);
}

#[test]
fn window_focus_and_tab_visibility() {
    run(r##"
        assert(browser.HasFocus() == true and browser.IsVisible() == true)
        local seen = {}
        hook.Add("OnWindowFocusChanged", "t", function(on) seen[#seen + 1] = "focus " .. tostring(on) end)
        local page = ui.Create("Panel")
        page:Hook("OnVisibilityChanged", function(v) seen[#seen + 1] = "visible " .. tostring(v) end)
        R.window_focus(false)
        R.visibility_changed(false)
        R.visibility_changed(true)
        R.window_focus(true)
        assert(table.concat(seen, ",") == "focus false,visible false,visible true,focus true", table.concat(seen, ","))
        page:Remove()
        R.visibility_changed(false)
        assert(#seen == 4, "the panel hook is gone with the panel")
        MOCK.focused, MOCK.visible = false, false
        assert(browser.HasFocus() == false and browser.IsVisible() == false)
    "##);
}

#[test]
fn badge_status_avatar_meter() {
    run(r##"
        local b = ui.Create("Badge", nil, { Text = "PPV", Variant = "success" })
        assert(MOCK.el(b).classes["ngs-badge-success"] and not MOCK.el(b).classes["ngs-badge-neutral"])
        assert(not pcall(b.SetVariant, b, "pink"))

        local s = ui.Create("Status", nil, { Status = "online", Text = "на смене", Pulse = true })
        assert(MOCK.el(s).classes["ngs-status-online"] and MOCK.el(s).classes["ngs-status-pulse"])
        assert(s:GetText() == "на смене")

        local a = ui.Create("Avatar", nil, { Name = "Анна Волкова", AvatarSize = "sm" })
        assert(MOCK.el(a.Initials).text == "АВ")
        assert(MOCK.el(a).style.width == "26px")
        a:SetName("@lily_rose")
        assert(MOCK.el(a.Initials).text == "LR")
        a:SetName("ёж")
        assert(MOCK.el(a.Initials).text == "Ё")
        a:SetName("")
        assert(MOCK.el(a.Initials).text == "?")
        assert(R.name_hue("Анна Волкова") == R.name_hue("Анна Волкова"))
        a:SetImage("/a.png")
        assert(IsValid(a.Picture))
        a:SetImage(nil)
        assert(a.Picture == nil)

        local m = ui.Create("Meter", nil, { Fraction = 1.4, ShowLabel = true, Variant = "success" })
        assert(m:GetFraction() == 1 and MOCK.el(m.Fill).style.width == "100%")
        assert(MOCK.el(m.ValueLabel).text == "100%")
        m:SetFraction(0.756)
        assert(MOCK.el(m.ValueLabel).text == "76%" and tostring(MOCK.el(m).attrs["aria-valuenow"]) == "76")
        m:SetLabel("3 из 4")
        m:SetFraction(0.1)
        assert(MOCK.el(m.ValueLabel).text == "3 из 4")
        local st = ui.State(0.5)
        local bound = ui.Create("Meter", nil, { Fraction = st })
        st:Set(0.25)
        assert(bound:GetFraction() == 0.25)
    "##);
}

#[test]
fn buttons_segmented_and_select() {
    run(r##"
        local btn = ui.Create("Button", nil, { Text = "Войти", Variant = "primary" })
        assert(MOCK.el(btn).classes["ngs-button-primary"] and btn:GetVariant() == "primary")
        btn:SetVariant("default")
        assert(not MOCK.el(btn).classes["ngs-button-primary"])
        assert(not pcall(btn.SetVariant, btn, "huge"))

        local seg = ui.Create("Segmented")
        local picked
        seg.OnSelect = function(_, i, data) picked = data end
        seg:AddOption("Месяц", "month")
        local all = seg:AddOption("Всё время", "all")
        seg:SetValue("month")
        assert(seg:GetValue() == "month" and picked == nil, "SetValue is silent")
        all:DoClick()
        assert(picked == "all" and seg:GetValue() == "all")
        assert(MOCK.el(all).attrs["aria-checked"] == "true")

        local page = ui.Create("Panel")
        local sel = ui.Create("Select", page, { Placeholder = "Все модели" })
        assert(sel:GetText() == "Все модели" and MOCK.el(sel).classes["ngs-select-placeholder"])
        sel:SetOptions({ "@lily", { "Mia", "mia" } })
        local chosen
        sel.OnSelect = function(_, i, text, data) chosen = data end
        sel:DoClick()
        assert(sel.__pop:IsOpen())
        local opts = sel.__pop:GetChildren()
        assert(#opts == 2)
        opts[2]:DoClick()
        assert(chosen == "mia" and sel:GetValue() == "mia" and sel:GetText() == "Mia")
        assert(not sel.__pop:IsOpen())
        sel:SetOptions({ "x", { "Mia", "mia" } })
        assert(sel:GetValue() == "mia", "selection survives new options with the same data")
        sel:SetValue(nil)
        assert(sel:GetSelected() == nil and sel:GetText() == "Все модели")
        page:Remove()
        assert(not IsValid(sel.__pop), "popover goes with the select")
    "##);
}

#[test]
fn listview_sorts_pages_and_clicks() {
    run(r##"
        local lv = ui.Create("ListView")
        local clicked, right
        lv.OnRowClick = function(_, row) clicked = row end
        lv.OnRowRightClick = function(_, row) right = row end
        lv:SetColumns({
            { key = "name", title = "Имя", sort = true },
            { key = "amount", title = "Сумма", align = "right", sort = true, format = function(v) return "$" .. v end },
            { title = "Своя", build = function(cell, row) cell:Add("Badge", { Text = row.name }) end },
        })
        lv:SetRows({ { name = "b", amount = 5 }, { name = "a", amount = 30 }, { name = "c", amount = 10 } })
        assert(lv.Body:ChildCount() == 3)
        local first = lv.Body:GetChild(1)
        assert(first:GetChild(2):GetChild(1):GetText() == "$5")
        assert(MOCK.el(first).classes["ngs-listview-clickable"])

        lv.Header:GetChild(2):DoClick()
        local order = lv:GetDisplayRows()
        assert(order[1].amount == 30 and order[3].amount == 5, "numbers sort descending first")
        lv.Header:GetChild(2):DoClick()
        assert(lv:GetDisplayRows()[1].amount == 5)
        lv.Header:GetChild(1):DoClick()
        lv:SetSort(1, false)
        assert(lv:GetDisplayRows()[1].name == "a")
        assert(lv.Header:GetChild(1).Arrow:GetText() == "↑")

        lv.Body:GetChild(1):DoClick()
        assert(clicked.name == "a")
        lv.Body:GetChild(2):DoRightClick()
        assert(right.name == "b")

        lv:SetPageSize(2)
        assert(lv.Body:ChildCount() == 2 and lv.Footer:IsVisible())
        lv.MoreButton:DoClick()
        assert(lv.Body:ChildCount() == 3 and not lv.Footer:IsVisible())

        lv:SetRows({})
        assert(lv.Empty:IsVisible())
    "##);
}

#[test]
fn charts_build_and_paint_without_errors() {
    run(r##"
        ui.SetVars({ series1 = Color(212, 137, 10), surface = Color(17, 20, 27) })
        local sp = ui.Create("Sparkline", nil, { Values = { 1, 4, 2, 8 }, RingColor = "surface" })
        sp:Paint(300, 56)
        ui.Create("Sparkline"):Paint(300, 56)

        local lc = ui.Create("LineChart")
        lc:SetFormat(function(v) return "$" .. math.floor(v) end)
        lc:Paint(600, 256)
        lc:SetPoints({ { label = "01.11", value = 10 }, { label = "02.11", value = 30 }, { label = "03.11", value = 20 } })
        lc:OnCursorMoved(400)
        assert(lc.__hover ~= nil)
        lc:Paint(600, 256)
        lc:OnCursorExited()
        assert(lc.__hover == nil)

        local h = ui.Create("BarChart", nil, { Bars = { { label = "@lily", value = 100 }, { label = "@mia", value = 50 } } })
        assert(h:ChildCount() == 2)
        local fill = h:GetChild(2):GetChild(2):GetChild(1)
        assert(MOCK.el(fill).style.width == "50%")
        assert(MOCK.el(fill).style["background-color"] == "var(--app-series1)")
        h:SetOrientation("vertical")
        assert(h:GetChild(1):ChildCount() == 2)
        h:SetBars({})
        assert(h:GetChild(1):GetText() == "No data")
        assert(not pcall(h.SetOrientation, h, "diagonal"))

        local d = ui.Create("DonutChart", nil, { CenterLabel = "всего" })
        assert(d.EmptyLabel:IsVisible() and not d.Chart:IsVisible())
        d:SetSlices({ { label = "PPV", value = 3 }, { label = "Tip", value = 1 }, { label = "Zero", value = 0 } })
        assert(d.Legend:ChildCount() == 3 and d.Chart:IsVisible())
        assert(d.Legend:GetChild(1):GetChild(3):GetText() == "75%")
        assert(d.Total:GetText() == "4")
        d.Chart:Paint(172, 172)
        assert(R.percent_text(0.352) == "35.2%")
    "##);
}

#[test]
fn calendar_and_datepicker() {
    run(r##"
        date.SetOffset(0)
        date.SetNow(date.Time(2026, 11, 26, 12, 0, 0))
        local cal = ui.Create("Calendar")
        assert(cal.MonthLabel:GetText() == "November 2026")
        assert(cal.Days:ChildCount() == 6 + 30, "November 2026 starts on Sunday: six blanks before day 1")
        local today = cal.Days:GetChild(6 + 26)
        assert(MOCK.el(today).classes["ngs-calendar-today"])
        local picked
        cal.OnSelect = function(_, day) picked = day end
        cal.Days:GetChild(6 + 3):DoClick()
        assert(picked == date.Time(2026, 11, 3))
        cal.Next:DoClick()
        assert(cal.MonthLabel:GetText() == "December 2026")
        cal:SetValue(date.Time(2024, 2, 10, 15))
        assert(cal:GetValue() == date.Time(2024, 2, 10) and cal.MonthLabel:GetText() == "February 2024")

        local page = ui.Create("Panel")
        local dp = ui.Create("DatePicker", page, { Placeholder = "Дата от" })
        assert(dp:GetText() == "Дата от")
        local changed
        dp.OnChange = function(_, v) changed = v end
        dp:DoClick()
        assert(dp.__pop:IsOpen())
        dp.Calendar.Days:GetChild(6 + 1):DoClick()
        assert(changed == date.Time(2026, 11, 1) and dp:GetText() == "1 Nov 2026")
        assert(not dp.__pop:IsOpen())
        dp:SetValue(nil)
        assert(dp:GetText() == "Дата от" and dp:GetValue() == nil)
    "##);
}

#[test]
fn dialog_confirm_options_and_toast_kinds() {
    run(r##"
        local result
        local f = ui.Confirm({ Title = "Завершить смену", Accept = "Да", Cancel = "Нет",
            Build = function(body) body:Add("Label", { Text = "Точно?" }) end }, function(ok) result = ok end)
        assert(f:GetTitle() == "Завершить смену")
        assert(f.Body:GetChild(1):GetText() == "Точно?")
        local accept = f.Footer:GetChild(1)
        assert(accept:GetText() == "Да" and accept:GetVariant() == "primary")
        accept:DoClick()
        assert(result == true and not IsValid(f))

        local d = ui.Create("Dialog")
        d:Add("Label", { Text = "body" })
        assert(d.Body:ChildCount() == 1, "children go to the body slot")
        assert(not d.Footer:IsVisible())
        d:AddButton("Закрыть")
        assert(d.Footer:IsVisible())
        d.Footer:GetChild(1):DoClick()
        assert(not IsValid(d))

        local t = ui.Toast("Готово", 1, "success")
        assert(MOCK.el(t).classes["ngs-toast-success"])
        assert(not pcall(ui.Toast, "x", 1, "loud"))

        local card = ui.Create("Panel")
        card:SetChildrenMouseInputEnabled(false)
        assert(MOCK.el(card).classes["ngs-children-passive"])
        ui.SetSkin({ success = Color(61, 220, 151), warning = "#ff7a3d", onAccent = Color(26, 17, 0) })
        assert(MOCK.vars["--ngs-warning"] == "#ff7a3d")
    "##);
}
