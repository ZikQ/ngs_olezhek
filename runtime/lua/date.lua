---@diagnostic disable: duplicate-set-field
local N, R = ...

-- Local time is the browser's time zone (DST included) unless the app fixes an offset with
-- date.SetOffset, e.g. to show everything in the server's zone. date.SetNow does the same for the clock.
date = {}

local now_base, now_at
local offset

local function off_at(unix, off)
    if off then return off end
    if offset then return offset end
    return browser.TimezoneOffset(unix)
end

local locale = {
    months = { "January", "February", "March", "April", "May", "June", "July", "August",
        "September", "October", "November", "December" },
    monthsShort = { "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec" },
    weekdaysShort = { "Mo", "Tu", "We", "Th", "Fr", "Sa", "Su" },
    firstWeekday = 1,
}

-- Howard Hinnant's days_from_civil / civil_from_days; Lua's // floors, which these need.
local function days_from_civil(y, m, d)
    y = m <= 2 and y - 1 or y
    local era = y // 400
    local yoe = y - era * 400
    local doy = (153 * (m > 2 and m - 3 or m + 9) + 2) // 5 + d - 1
    local doe = yoe * 365 + yoe // 4 - yoe // 100 + doy
    return era * 146097 + doe - 719468
end

local function civil_from_days(z)
    z = z + 719468
    local era = z // 146097
    local doe = z - era * 146097
    local yoe = (doe - doe // 1460 + doe // 36524 - doe // 146096) // 365
    local doy = doe - (365 * yoe + yoe // 4 - yoe // 100)
    local mp = (5 * doy + 2) // 153
    local d = doy - (153 * mp + 2) // 5 + 1
    local m = mp < 10 and mp + 3 or mp - 9
    return yoe + era * 400 + (m <= 2 and 1 or 0), m, d
end

local function int(v, name)
    local n = math.tointeger(v)
    if not n then error("date: " .. name .. " must be an integer, got " .. tostring(v), 3) end
    return n
end

function date.SetNow(unix)
    assert(type(unix) == "number", "date.SetNow: expected unix seconds")
    now_base, now_at = unix, CurTime()
end

--- Unix seconds (UTC): the browser clock, or the time given to date.SetNow carried forward.
function date.Now()
    if not now_base then return math.floor(browser.Now()) end
    return math.floor(now_base + CurTime() - now_at)
end

--- nil returns to the browser's time zone.
function date.SetOffset(seconds)
    assert(seconds == nil or type(seconds) == "number", "date.SetOffset: expected seconds east of UTC")
    offset = seconds and math.floor(seconds)
end

--- Offset in effect at a moment (default now): the fixed one or the browser's.
function date.GetOffset(unix) return off_at(unix or date.Now()) end

function date.SetLocale(t)
    for k, v in pairs(t) do
        if locale[k] == nil then error("date.SetLocale: unknown key '" .. tostring(k) .. "'", 2) end
        locale[k] = v
    end
end

function date.GetLocale(key) return locale[key] end

function date.IsLeap(y) return y % 4 == 0 and (y % 100 ~= 0 or y % 400 == 0) end

function date.DaysInMonth(y, m)
    if m == 2 then return date.IsLeap(y) and 29 or 28 end
    return (m == 4 or m == 6 or m == 9 or m == 11) and 30 or 31
end

--- Unix seconds for a wall-clock time at the given offset (default: date.GetOffset()).
--- Month and day may overflow: Time(2026, 13, 1) is January 2027.
function date.Time(y, m, d, hh, mm, ss, off)
    y, m, d = int(y, "year"), int(m or 1, "month"), int(d or 1, "day")
    local m0 = m - 1
    y = y + m0 // 12
    m = m0 % 12 + 1
    local days = days_from_civil(y, m, 1) + d - 1
    local wall = days * 86400 + (hh or 0) * 3600 + (mm or 0) * 60 + (ss or 0)
    -- The offset depends on the instant we are looking for; one refinement settles DST edges.
    local guess = wall - off_at(wall, off)
    return wall - off_at(guess, off)
end

--- { year, month, day, hour, min, sec, wday (1 = Monday), yday } at the offset.
function date.Fields(unix, off)
    local t = math.floor(unix) + off_at(unix, off)
    local days = t // 86400
    local secs = t - days * 86400
    local y, m, d = civil_from_days(days)
    return {
        year = y, month = m, day = d,
        hour = secs // 3600, min = secs % 3600 // 60, sec = secs % 60,
        wday = (days + 3) % 7 + 1,
        yday = days - days_from_civil(y, 1, 1) + 1,
    }
end

function date.StartOfDay(unix, off)
    local f = date.Fields(unix, off)
    return date.Time(f.year, f.month, f.day, 0, 0, 0, off)
end

function date.StartOfMonth(unix, off)
    local f = date.Fields(unix, off)
    return date.Time(f.year, f.month, 1, 0, 0, 0, off)
end

--- Calendar days, so a day stays a day across offset changes.
function date.AddDays(unix, n, off)
    local f = date.Fields(unix, off)
    return date.Time(f.year, f.month, f.day + n, f.hour, f.min, f.sec, off)
end

function date.AddMonths(unix, n, off)
    local f = date.Fields(unix, off)
    local first = date.Time(f.year, f.month + n, 1, 0, 0, 0, off)
    local nf = date.Fields(first, off)
    local day = math.min(f.day, date.DaysInMonth(nf.year, nf.month))
    return date.Time(nf.year, nf.month, day, f.hour, f.min, f.sec, off)
end

function date.SameDay(a, b, off)
    return date.StartOfDay(a, off) == date.StartOfDay(b, off)
end

--- ISO 8601: "2026-11-26", "2026-11-26T18:41:00", fractions, "Z" or "+03:00". No zone means UTC.
function date.Parse(s)
    if type(s) ~= "string" then return nil end
    local y, m, d, rest = s:match("^%s*(%d%d%d%d)%-(%d%d)%-(%d%d)(.*)$")
    if not y then return nil end
    local hh, mi, ss, frac = 0, 0, 0, 0
    local zone = 0
    if rest ~= "" and not rest:match("^%s*$") then
        local h2, m2, tail = rest:match("^[Tt ](%d%d):(%d%d)(.*)$")
        if not h2 then return nil end
        hh, mi = tonumber(h2), tonumber(m2)
        local s2, tail2 = tail:match("^:(%d%d)(.*)$")
        if s2 then ss, tail = tonumber(s2), tail2 end
        local f, tail3 = tail:match("^[%.,](%d+)(.*)$")
        if f then frac, tail = tonumber("0." .. f), tail3 end
        if tail:match("^[Zz]%s*$") or tail:match("^%s*$") then
            zone = 0
        else
            local sign, zh, zm = tail:match("^([%+%-])(%d%d):?(%d%d)%s*$")
            if not sign then return nil end
            zone = (tonumber(zh) * 3600 + tonumber(zm) * 60) * (sign == "-" and -1 or 1)
        end
    end
    y, m, d = tonumber(y), tonumber(m), tonumber(d)
    if m < 1 or m > 12 or d < 1 or d > date.DaysInMonth(y, m) or hh > 23 or mi > 59 or ss > 60 then return nil end
    return days_from_civil(y, m, d) * 86400 + hh * 3600 + mi * 60 + ss - zone + frac
end

--- "2026-11-26T15:41:00Z"
function date.ISO(unix)
    local f = date.Fields(unix, 0)
    return string.format("%04d-%02d-%02dT%02d:%02d:%02dZ", f.year, f.month, f.day, f.hour, f.min, f.sec)
end

local TOKENS = { "YYYY", "YY", "MMMM", "MMM", "MM", "M", "DD", "D", "HH", "H", "mm", "ss", "dd" }

local function token_value(tok, f)
    if tok == "YYYY" then return string.format("%04d", f.year) end
    if tok == "YY" then return string.format("%02d", f.year % 100) end
    if tok == "MMMM" then return locale.months[f.month] end
    if tok == "MMM" then return locale.monthsShort[f.month] end
    if tok == "MM" then return string.format("%02d", f.month) end
    if tok == "M" then return tostring(f.month) end
    if tok == "DD" then return string.format("%02d", f.day) end
    if tok == "D" then return tostring(f.day) end
    if tok == "HH" then return string.format("%02d", f.hour) end
    if tok == "H" then return tostring(f.hour) end
    if tok == "mm" then return string.format("%02d", f.min) end
    if tok == "ss" then return string.format("%02d", f.sec) end
    if tok == "dd" then return locale.weekdaysShort[f.wday] end
end

--- Tokens: YYYY YY MMMM MMM MM M DD D HH H mm ss dd (weekday). Text in [brackets] is literal.
function date.Format(unix, pattern, off)
    local f = date.Fields(unix, off)
    local out, i, n = {}, 1, #pattern
    while i <= n do
        local c = pattern:sub(i, i)
        if c == "[" then
            local close = pattern:find("]", i + 1, true) or n + 1
            out[#out + 1] = pattern:sub(i + 1, close - 1)
            i = close + 1
        else
            local hit
            for _, tok in ipairs(TOKENS) do
                if pattern:sub(i, i + #tok - 1) == tok then hit = tok; break end
            end
            if hit then
                out[#out + 1] = token_value(hit, f)
                i = i + #hit
            else
                out[#out + 1] = c
                i = i + 1
            end
        end
    end
    return table.concat(out)
end

R.days_from_civil = days_from_civil
