---@diagnostic disable: duplicate-set-field, inject-field, undefined-field, need-check-nil
local N, R = ...

local Label = ui.GetClass("Label")

local function cls(p, name, on) N.class(p.__id, name, on ~= false) end

local function spawn(self, method, ...)
    local fn = self[method]
    if fn then return R.spawn_for(self, fn, self, ...) end
end

-- Dates are unix seconds of local midnight (date.GetOffset()).
local Calendar = { BindDefault = "Value" }
function Calendar:Initialize()
    cls(self, "ngs-calendar")
    self.Nav = self:Add("Row")
    cls(self.Nav, "ngs-calendar-nav")
    self.Prev = self.Nav:Add("Button", { Text = "‹" })
    cls(self.Prev, "ngs-calendar-navbtn")
    self.Prev:SetAttr("aria-label", self.PrevText)
    self.MonthLabel = self.Nav:Add("Label")
    cls(self.MonthLabel, "ngs-calendar-month")
    self.Next = self.Nav:Add("Button", { Text = "›" })
    cls(self.Next, "ngs-calendar-navbtn")
    self.Next:SetAttr("aria-label", self.NextText)
    self.Prev.DoClick = function() self:__shift(-1) end
    self.Next.DoClick = function() self:__shift(1) end
    self.Weekdays = self:Add("Grid", { Columns = 7 })
    cls(self.Weekdays, "ngs-calendar-weekdays")
    self.Days = self:Add("Grid", { Columns = 7 })
    cls(self.Days, "ngs-calendar-grid")
    local f = date.Fields(date.Now())
    self.__y, self.__m = f.year, f.month
    self:__render()
end

function Calendar:__shift(n)
    local f = date.Fields(date.Time(self.__y, self.__m + n, 1))
    self:SetMonth(f.year, f.month)
end

function Calendar:SetMonth(y, m)
    self.__y, self.__m = y, m
    self:__render()
end

function Calendar:GetMonth() return self.__y, self.__m end

--- Selected day (any time on it), or nil. Shows that day's month.
function Calendar:SetValue(unix)
    self.__value = unix and date.StartOfDay(unix) or nil
    if unix then
        local f = date.Fields(unix)
        self.__y, self.__m = f.year, f.month
    end
    self:__render()
end

function Calendar:GetValue() return self.__value end

--- Days strictly between from and to get the in-range look.
function Calendar:SetRange(from, to)
    self.__from = from and date.StartOfDay(from) or nil
    self.__to = to and date.StartOfDay(to) or nil
    self:__render()
end

function Calendar:__render()
    local y, m = self.__y, self.__m
    local first = date.Time(y, m, 1)
    self.MonthLabel:SetText(date.Format(first, "MMMM YYYY"))
    local fw = date.GetLocale("firstWeekday") or 1
    local names = date.GetLocale("weekdaysShort")
    self.Weekdays:Clear()
    for k = 0, 6 do
        self.Weekdays:Add("Label", { Text = names[(fw - 1 + k) % 7 + 1] })
    end
    self.Days:Clear()
    local blanks = (date.Fields(first).wday - fw) % 7
    for _ = 1, blanks do self.Days:Add("Panel") end
    local today = date.StartOfDay(date.Now())
    for d = 1, date.DaysInMonth(y, m) do
        local day = date.Time(y, m, d)
        local b = self.Days:Add("Button", { Text = d })
        cls(b, "ngs-calendar-day")
        cls(b, "ngs-calendar-today", day == today)
        local selected = day == self.__value
        cls(b, "ngs-calendar-selected", selected)
        b:SetAttr("aria-pressed", tostring(selected))
        cls(b, "ngs-calendar-inrange", self.__from ~= nil and self.__to ~= nil and day > self.__from and day < self.__to)
        b.DoClick = function()
            self.__value = day
            self:__render()
            spawn(self, "OnSelect", day)
        end
    end
end
Calendar.PrevText = "Previous month"
Calendar.NextText = "Next month"
ui.Register("Calendar", Calendar, "Panel")

local DatePicker = { BindDefault = "Value" }
function DatePicker:Initialize()
    cls(self, "ngs-datepicker")
    self:SetAttr("aria-haspopup", "dialog")
    self.__format = self.Format
    self:__refresh()
end
DatePicker.Format = "D MMM YYYY"

function DatePicker:__refresh()
    local v = self.__value
    cls(self, "ngs-datepicker-placeholder", v == nil)
    Label.SetText(self, v and date.Format(v, self.__format) or self.__placeholder or "")
end

function DatePicker:SetPlaceholder(text)
    self.__placeholder = text
    self:__refresh()
end

--- Pattern for date.Format, "D MMM YYYY" by default.
function DatePicker:SetFormat(pattern)
    self.__format = pattern
    self:__refresh()
end

--- A day as unix seconds (any time on it) or nil. Does not fire OnChange.
function DatePicker:SetValue(unix)
    self.__value = unix and date.StartOfDay(unix) or nil
    self:__refresh()
end

function DatePicker:GetValue() return self.__value end

function DatePicker:GetCalendar() return self.Calendar end

function DatePicker:DoClick()
    local pop = self.__pop
    if not IsValid(pop) then
        pop = ui.Create("Popover", self:GetParent())
        cls(pop, "ngs-datepicker-pop")
        self.Calendar = pop:Add("Calendar")
        self.Calendar.OnSelect = function(_, day)
            self:SetValue(day)
            pop:Hide()
            R.input_changed(self, "Value", day)
            spawn(self, "OnChange", day)
        end
        pop.OnClose = function() if IsValid(self) then cls(self, "ngs-datepicker-open", false) end end
        self:CallOnRemove("ngs-datepicker", function() if IsValid(pop) then pop:Remove() end end)
        self.__pop = pop
    end
    if not pop:IsOpen() then
        if self.__value then self.Calendar:SetValue(self.__value)
        else
            self.Calendar.__value = nil
            local f = date.Fields(date.Now())
            self.Calendar:SetMonth(f.year, f.month)
        end
    end
    pop:Toggle(self)
    cls(self, "ngs-datepicker-open", pop:IsOpen())
end
ui.Register("DatePicker", DatePicker, "Button")
