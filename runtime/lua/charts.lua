---@diagnostic disable: duplicate-set-field, inject-field, undefined-field, need-check-nil
local N, R = ...

local function cls(p, name, on) N.class(p.__id, name, on ~= false) end

-- Used when the app has not declared series1..series6 with ui.SetVars.
local FALLBACK = {
    Color(212, 137, 10), Color(226, 77, 139), Color(51, 140, 237),
    Color(0, 168, 136), Color(152, 135, 239), Color(10, 172, 35),
}
local MUTED = Color(154, 160, 170)
local LINE = Color(255, 255, 255, 24)
local SURFACE = Color(37, 40, 45)

local function series_name(i) return "series" .. ((i - 1) % 6 + 1) end

--- Canvas color of series i: the app variable seriesN when it is a Color, else a built-in palette.
function R.series_paint(i, c)
    if c ~= nil then return R.paint_color(c, FALLBACK[(i - 1) % 6 + 1]) end
    return R.paint_color(series_name(i), FALLBACK[(i - 1) % 6 + 1])
end

--- The same for DOM charts: a CSS value for SetBackgroundColor.
function R.series_css(i, c)
    if c ~= nil then return R.css_color_of(c) end
    local name = series_name(i)
    if ui.GetVar(name) ~= nil then return ui.Var(name) end
    return FALLBACK[(i - 1) % 6 + 1]
end

local function alpha(c, a) return Color(c.r, c.g, c.b, a) end

local function fmt_default(v)
    if math.type(v) == "float" and v ~= math.floor(v) then return string.format("%.1f", v) end
    return tostring(math.floor(v + 0.5))
end

--- "35.2%", "40%".
function R.percent_text(share)
    local s = string.format("%.1f", share * 100):gsub("%.0$", "")
    return s .. "%"
end

local function path_of(pts)
    local parts = {}
    for i, p in ipairs(pts) do
        parts[i] = string.format("%s%.2f %.2f", i == 1 and "M" or "L", p[1], p[2])
    end
    return table.concat(parts, " ")
end

local function draw_line(pts, c, ring, bottom, fill)
    local line = path_of(pts)
    if fill and #pts > 1 then
        surface.SetDrawGradient(0, 0, 0, bottom, { alpha(c, 41), alpha(c, 0) })
        surface.DrawPath(string.format("%s L%.2f %.2f L%.2f %.2f Z", line, pts[#pts][1], bottom, pts[1][1], bottom), "fill")
    end
    surface.SetDrawColor(c)
    surface.SetLineWidth(2)
    surface.SetLineJoin("round")
    surface.SetLineCap("round")
    if #pts > 1 then surface.DrawPath(line, "stroke") end
    local last = pts[#pts]
    draw.Circle(last[1], last[2], 7.5, ring)
    draw.Circle(last[1], last[2], 4.5, c)
end

local Sparkline = {}
function Sparkline:Initialize()
    self.__values = {}
    self.__fill = true
    self:SetTall(56)
    self:SetAttr("role", "img")
end

function Sparkline:SetValues(values)
    self.__values = values or {}
    self:InvalidatePaint()
end

function Sparkline:GetValues() return self.__values end

--- Color, app variable or skin key (default series1).
function Sparkline:SetColor(c) self.__color = c; self:InvalidatePaint() end
--- The ring around the end dot: the color of whatever is behind the chart.
function Sparkline:SetRingColor(c) self.__ring = c; self:InvalidatePaint() end
function Sparkline:SetFill(b) self.__fill = b ~= false; self:InvalidatePaint() end

function Sparkline:Paint(w, h)
    local vals = self.__values
    local n = #vals
    if n == 0 then return end
    local lo, hi = math.huge, -math.huge
    for _, v in ipairs(vals) do lo, hi = math.min(lo, v), math.max(hi, v) end
    if hi == lo then lo, hi = lo - 1, hi + 1 end
    local padx = 8
    local top, bottom = h * 2 / 32, h * 30 / 32
    local pts = {}
    for i, v in ipairs(vals) do
        local x = n == 1 and w / 2 or padx + (w - 2 * padx) * (i - 1) / (n - 1)
        pts[i] = { x, bottom - (v - lo) / (hi - lo) * (bottom - top) }
    end
    local c = R.series_paint(1, self.__color)
    draw_line(pts, c, R.paint_color(self.__ring or self.RingColor, SURFACE), h, self.__fill)
end
Sparkline.RingColor = "panel"
ui.Register("Sparkline", Sparkline, "Panel")

local LineChart = {}
function LineChart:Initialize()
    self.__series = {}
    self.__format = fmt_default
    self:SetTall(256)
    self:SetAttr("role", "img")
end

--- { { name = "...", color = ..., points = { { label = "01.11", value = 12 }, ... } }, ... }
function LineChart:SetSeries(series)
    self.__series = series or {}
    self.__hover = nil
    self:InvalidatePaint()
end

--- One series: points = { { label = ..., value = ... }, ... }.
function LineChart:SetPoints(points, color)
    self:SetSeries({ { points = points or {}, color = color } })
end

--- Formats axis labels and the hover value: fn(number) -> string.
function LineChart:SetFormat(fn) self.__format = fn or fmt_default; self:InvalidatePaint() end
function LineChart:SetEmptyText(text) self.__empty = text; self:InvalidatePaint() end

function LineChart:__count()
    local n = 0
    for _, s in ipairs(self.__series) do n = math.max(n, #s.points) end
    return n
end

local PL, PT, PB = 48, 10, 26

function LineChart:__x(i, n, w)
    local pw = w - PL
    if n == 1 then return PL + pw / 2 end
    return PL + pw * (0.02 + 0.96 * (i - 1) / (n - 1))
end

function LineChart:OnCursorMoved(x)
    local n = self:__count()
    if n == 0 then return end
    local w = self:GetWide()
    local best, dist = nil, math.huge
    for i = 1, n do
        local d = math.abs(self:__x(i, n, w) - x)
        if d < dist then best, dist = i, d end
    end
    if best ~= self.__hover then
        self.__hover = best
        self:InvalidatePaint()
    end
end

function LineChart:OnCursorExited()
    if self.__hover then
        self.__hover = nil
        self:InvalidatePaint()
    end
end

function LineChart:Paint(w, h)
    local font = self.Font
    local label_c = R.paint_color(self.LabelColor, MUTED)
    local n = self:__count()
    if n == 0 then
        draw.SimpleText(self.__empty or self.EmptyText, font, w / 2, h / 2, label_c, TEXT_ALIGN_CENTER, TEXT_ALIGN_CENTER)
        return
    end
    local max = 0
    for _, s in ipairs(self.__series) do
        for _, p in ipairs(s.points) do max = math.max(max, tonumber(p.value) or 0) end
    end
    if max <= 0 then max = 1 end
    local gt, gb = PT, h - PB
    local grid_c = R.paint_color(self.GridColor)
    if not grid_c then
        local b = R.paint_color("border")
        grid_c = b and alpha(b, math.floor(b.a / 2)) or LINE
    end
    local fmt = self.__format
    surface.SetLineWidth(1)
    for k = 0, 4 do
        local y = math.floor(gt + (gb - gt) * k / 4) + 0.5
        surface.SetDrawColor(grid_c)
        surface.DrawLine(PL, y, w, y)
        draw.SimpleText(fmt(max * (1 - k / 4)), font, PL - 8, y, label_c, TEXT_ALIGN_RIGHT, TEXT_ALIGN_CENTER)
    end

    local labels = self.__series[1].points
    local count = math.min(n, 6)
    for j = 0, count - 1 do
        local i = count == 1 and 1 or math.floor((n - 1) * j / (count - 1) + 0.5) + 1
        local p = labels[i]
        if p then
            local align = (j == 0 and count > 1) and TEXT_ALIGN_LEFT or (j == count - 1 and count > 1) and TEXT_ALIGN_RIGHT or TEXT_ALIGN_CENTER
            local x = self:__x(i, n, w)
            draw.SimpleText(p.label or "", font, x, h - PB / 2 + 2, label_c, align, TEXT_ALIGN_CENTER)
        end
    end

    local ring = R.paint_color(self.RingColor, SURFACE)
    local placed = {}
    for si, s in ipairs(self.__series) do
        local pts = {}
        for i, p in ipairs(s.points) do
            pts[i] = { self:__x(i, n, w), gb - (gb - gt) * (tonumber(p.value) or 0) / max }
        end
        placed[si] = pts
        if #pts > 0 then draw_line(pts, R.series_paint(si, s.color), ring, gb, #self.__series == 1) end
    end

    local hi = self.__hover
    if hi then
        local x = self:__x(hi, n, w)
        surface.SetDrawColor(alpha(label_c, 110))
        surface.SetLineWidth(1)
        surface.SetLineDash({ 3, 3 })
        surface.DrawLine(math.floor(x) + 0.5, gt, math.floor(x) + 0.5, gb)
        surface.SetLineDash(nil)
        local texts = {}
        for si, s in ipairs(self.__series) do
            local p, pt = s.points[hi], placed[si][hi]
            if p and pt then
                local c = R.series_paint(si, s.color)
                draw.Circle(pt[1], pt[2], 6, ring)
                draw.Circle(pt[1], pt[2], 4, c)
                texts[#texts + 1] = (s.name and (s.name .. ": ") or "") .. fmt(tonumber(p.value) or 0)
            end
        end
        local head = (labels[hi] and labels[hi].label) or ""
        surface.SetFont(font)
        local tw = surface.GetTextSize(head)
        for _, t in ipairs(texts) do tw = math.max(tw, (surface.GetTextSize(t))) end
        local lh = draw.GetFontHeight(font)
        local bw, bh = tw + 20, lh * (#texts + 1) + 14
        local bx = math.Clamp(x + 12, PL, w - bw)
        if x + 12 + bw > w then bx = math.max(PL, x - 12 - bw) end
        local by = gt
        draw.RoundedBox(6, bx, by, bw, bh, R.paint_color(self.TooltipColor, SURFACE))
        local text_c = R.paint_color("text", color_white)
        draw.SimpleText(head, font, bx + 10, by + 7, label_c)
        for k, t in ipairs(texts) do
            draw.SimpleText(t, font, bx + 10, by + 7 + lh * k, text_c)
        end
    end
end
LineChart.Font = "Small"
LineChart.EmptyText = "No data"
LineChart.LabelColor = "textMuted"
LineChart.GridColor = nil
LineChart.RingColor = "panel"
LineChart.TooltipColor = "panel"
ui.Register("LineChart", LineChart, "Panel")

local BarChart = {}
function BarChart:Initialize()
    cls(self, "ngs-barchart")
    self.__bars = {}
    self.__orientation = "horizontal"
    self.__format = fmt_default
    self:__render()
end

--- { { label = "...", value = 12, color = ? }, ... }
function BarChart:SetBars(bars)
    self.__bars = bars or {}
    self:__render()
end

--- "horizontal" (rows with a label, a track and the value) or "vertical" (columns, value on hover).
function BarChart:SetOrientation(o)
    if o ~= "horizontal" and o ~= "vertical" then
        error("BarChart:SetOrientation: expected horizontal or vertical, got " .. tostring(o), 2)
    end
    self.__orientation = o
    self:__render()
end

function BarChart:SetFormat(fn) self.__format = fn or fmt_default; self:__render() end
function BarChart:SetColor(c) self.__color = c; self:__render() end
function BarChart:SetEmptyText(text) self.__empty = text; self:__render() end
--- Fixed scale instead of the largest value.
function BarChart:SetMax(v) self.__max = v; self:__render() end
--- Height of the vertical chart.
function BarChart:SetPlotHeight(px) self.__plot = px; self:__render() end

function BarChart:__render()
    self:Clear()
    local bars = self.__bars
    local vertical = self.__orientation == "vertical"
    cls(self, "ngs-barchart-h", not vertical)
    cls(self, "ngs-barchart-v", vertical)
    if #bars == 0 then
        local e = self:Add("Label", { Text = self.__empty or self.EmptyText })
        cls(e, "ngs-barchart-empty")
        return
    end
    local max = self.__max or 0
    if not self.__max then
        for _, b in ipairs(bars) do max = math.max(max, tonumber(b.value) or 0) end
    end
    if max <= 0 then max = 1 end
    local fmt = self.__format
    local fill_c = R.series_css(1, self.__color)
    if vertical then
        local cols = self:Add("Row")
        cls(cols, "ngs-barchart-cols")
        if self.__plot then cols:SetTall(self.__plot) end
        for _, b in ipairs(bars) do
            local v = tonumber(b.value) or 0
            local text = fmt(v)
            local col = cols:Add("Panel")
            cls(col, "ngs-barchart-col")
            col:SetTooltip((b.label or "") .. ": " .. text)
            local bubble = col:Add("Label", { Text = text })
            cls(bubble, "ngs-barchart-bubble")
            local track = col:Add("Panel")
            cls(track, "ngs-barchart-coltrack")
            local fill = track:Add("Panel")
            cls(fill, "ngs-barchart-colfill")
            N.style(fill.__id, "height", string.format("%g%%", math.Clamp(v / max, 0, 1) * 100))
            fill:SetBackgroundColor(b.color and R.css_color_of(b.color) or fill_c)
            local label = col:Add("Label", { Text = b.label or "" })
            cls(label, "ngs-barchart-collabel")
        end
    else
        for _, b in ipairs(bars) do
            local v = tonumber(b.value) or 0
            local row = self:Add("Grid")
            cls(row, "ngs-barchart-row")
            row:SetColumns({ "minmax(80px, 128px)", "1fr", "auto" })
            local label = row:Add("Label", { Text = b.label or "" })
            cls(label, "ngs-barchart-label")
            label:SetTooltip(b.label)
            label:SetMouseInputEnabled(true)
            local track = row:Add("Panel")
            cls(track, "ngs-barchart-track")
            local fill = track:Add("Panel")
            cls(fill, "ngs-barchart-fill")
            N.style(fill.__id, "width", string.format("%g%%", math.Clamp(v / max, 0, 1) * 100))
            fill:SetBackgroundColor(b.color and R.css_color_of(b.color) or fill_c)
            local value = row:Add("Label", { Text = fmt(v) })
            cls(value, "ngs-barchart-value")
        end
    end
end
BarChart.EmptyText = "No data"
ui.Register("BarChart", BarChart, "Panel")

local DonutChart = {}
function DonutChart:Initialize()
    cls(self, "ngs-donut")
    self.__slices = {}
    self.__format = fmt_default
    self.Chart = self:Add("Panel")
    cls(self.Chart, "ngs-donut-chart")
    self.Chart:SetSize(172, 172)
    self.Chart:SetAttr("role", "img")
    self.Chart.Paint = function(_, w, h) self:__paint(w, h) end
    self.Center = self.Chart:Add("Column")
    cls(self.Center, "ngs-donut-center")
    self.Total = self.Center:Add("Label")
    cls(self.Total, "ngs-donut-total")
    self.Caption = self.Center:Add("Label")
    cls(self.Caption, "ngs-donut-caption")
    self.Legend = self:Add("Column")
    cls(self.Legend, "ngs-donut-legend")
    self.EmptyLabel = self:Add("Label", { Text = self.EmptyText })
    cls(self.EmptyLabel, "ngs-donut-empty")
    self:__render()
end

--- { { label = "PPV", value = 120, color = ? }, ... }; negative values count as zero.
function DonutChart:SetSlices(slices)
    self.__slices = slices or {}
    self:__render()
end

function DonutChart:SetCenterLabel(text) self.Caption:SetText(text) end
--- Formats the total in the middle.
function DonutChart:SetFormat(fn) self.__format = fn or fmt_default; self:__render() end
function DonutChart:SetEmptyText(text) self.EmptyLabel:SetText(text) end

function DonutChart:SetDiameter(px)
    self.Chart:SetSize(px, px)
    self.Chart:InvalidatePaint()
end

--- Ring width in pixels; by default a tenth of the diameter.
function DonutChart:SetThickness(px) self.__thick = px; self.Chart:InvalidatePaint() end

function DonutChart:__render()
    local total = 0
    for _, s in ipairs(self.__slices) do total = total + math.max(tonumber(s.value) or 0, 0) end
    self.__total = total
    local empty = total <= 0
    self.Chart:SetVisible(not empty)
    self.Legend:SetVisible(not empty)
    self.EmptyLabel:SetVisible(empty)
    self.Legend:Clear()
    self.Total:SetText(self.__format(total))
    if empty then return end
    for i, s in ipairs(self.__slices) do
        local item = self.Legend:Add("Row")
        cls(item, "ngs-donut-item")
        local dot = item:Add("Panel")
        cls(dot, "ngs-donut-dot")
        dot:SetBackgroundColor(R.series_css(i, s.color))
        local name = item:Add("Label", { Text = s.label or "" })
        cls(name, "ngs-donut-name")
        local share = math.max(tonumber(s.value) or 0, 0) / total
        local val = item:Add("Label", { Text = R.percent_text(share) })
        cls(val, "ngs-donut-value")
    end
    self.Chart:InvalidatePaint()
end

function DonutChart:__paint(w, h)
    local total = self.__total or 0
    if total <= 0 then return end
    local d = math.min(w, h)
    local thick = self.__thick or d * 0.1
    local r = d * 0.42
    local cx, cy = w / 2, h / 2
    local n = 0
    for _, s in ipairs(self.__slices) do if (tonumber(s.value) or 0) > 0 then n = n + 1 end end
    local gap = n > 1 and 1.4 / (2 * math.pi * 42) * 360 or 0
    local angle = -90
    surface.SetLineWidth(thick)
    surface.SetLineCap("butt")
    for i, s in ipairs(self.__slices) do
        local v = math.max(tonumber(s.value) or 0, 0)
        if v > 0 then
            local sweep = 360 * v / total
            surface.SetDrawColor(R.series_paint(i, s.color))
            surface.DrawArc(cx, cy, r, angle, angle + math.max(sweep - gap, 0.5))
            angle = angle + sweep
        end
    end
end
DonutChart.EmptyText = "No data"
ui.Register("DonutChart", DonutChart, "Panel")
