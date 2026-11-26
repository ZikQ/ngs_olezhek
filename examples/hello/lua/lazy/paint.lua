---@class BarChart: Panel
---@field Values number[]
---@field Paints integer
local CHART = {}

function CHART:Initialize()
    self:SetTall(240)
    self.Paints = 0
    self:Shuffle()
end

function CHART:Shuffle()
    self.Values = {}
    for i = 1, 20 do self.Values[i] = math.random(8, 100) end
    self:InvalidatePaint()
end

function CHART:Paint(w, h)
    self.Paints = self.Paints + 1
    draw.RoundedBox(10, 0, 0, w, h, Color(37, 40, 45))

    local n = #self.Values
    local bw = (w - 40) / n
    for i, v in ipairs(self.Values) do
        local bh = (h - 70) * v / 100
        local col = v > 70 and Color(255, 170, 70) or Color(92, 156, 255)
        draw.RoundedBoxEx(4, 20 + (i - 1) * bw + 3, h - 24 - bh, bw - 6, bh, col, true, true, false, false)
    end

    surface.SetDrawColor(255, 255, 255, 30)
    surface.DrawLine(20, h - 24, w - 20, h - 24)

    draw.SimpleText("Paint вызван " .. self.Paints .. " раз", "DefaultBold", 20, 14, color_white)
    draw.SimpleText(w .. " x " .. h, "Small", w - 20, 16, Color(154, 160, 170), TEXT_ALIGN_RIGHT)
end

ui.Register("BarChart", CHART, "Panel")

-- Lucide-style 24x24 icons: SVG path data drawn with DrawPath, colored like any other stroke.
local ICONS = {
    { "M3 17l6-6 4 4 8-8", "M14 7h7v7" },
    { "M16 21v-2a4 4 0 0 0-4-4H6a4 4 0 0 0-4 4v2", "M22 21v-2a4 4 0 0 0-3-3.87", "M16 3.13a4 4 0 0 1 0 7.75",
        "M13 7a4 4 0 1 1-8 0a4 4 0 1 1 8 0" },
    { "M21 12a9 9 0 1 1-18 0a9 9 0 1 1 18 0", "M12 7v5l3 2" },
    { "M12 3l8 3v6c0 4.5-3.2 8.2-8 9-4.8-.8-8-4.5-8-9V6z", "M9 12l2 2 4-4" },
    { "M21 12.8A9 9 0 1 1 11.2 3a7 7 0 0 0 9.8 9.8z" },
}

---@class VectorDemo: Panel
local VECTOR = {}

function VECTOR:Initialize()
    self:SetTall(220)
    self.Points = {}
    for i = 1, 26 do self.Points[i] = math.random(20, 95) end
end

function VECTOR:Paint(w, h)
    draw.RoundedBox(10, 0, 0, w, h, ui.GetVar("card", Color(37, 40, 45)))
    local accent = ui.GetVar("series", Color(245, 166, 35))

    surface.SetDrawColor(ui.GetVar("muted", Color(154, 160, 170)))
    surface.SetLineWidth(1.8)
    surface.SetLineCap("round")
    surface.SetLineJoin("round")
    for i, icon in ipairs(ICONS) do
        for _, d in ipairs(icon) do surface.DrawPath(d, "stroke", 20 + (i - 1) * 40, 18, 1) end
    end

    local x0, y0, cw, ch = 20, 64, w - 200, h - 84
    surface.SetLineWidth(1)
    surface.SetLineDash({ 4, 4 })
    surface.SetDrawColor(255, 255, 255, 25)
    for k = 0, 3 do surface.DrawLine(x0, y0 + ch * k / 3, x0 + cw, y0 + ch * k / 3) end
    surface.SetLineDash()

    local line, n = {}, #self.Points
    for i, v in ipairs(self.Points) do
        line[i] = string.format("%s%.1f %.1f", i == 1 and "M" or "L", x0 + cw * (i - 1) / (n - 1), y0 + ch * (1 - v / 100))
    end
    local stroke = table.concat(line, " ")
    surface.SetDrawGradient(0, y0, 0, y0 + ch, { Color(accent.r, accent.g, accent.b, 41), Color(accent.r, accent.g, accent.b, 0) })
    surface.DrawPath(stroke .. string.format(" L%.1f %.1f L%.1f %.1f Z", x0 + cw, y0 + ch, x0, y0 + ch), "fill")
    surface.SetDrawColor(accent)
    surface.SetLineWidth(2)
    surface.DrawPath(stroke, "stroke")
    local lx, ly = x0 + cw, y0 + ch * (1 - self.Points[n] / 100)
    draw.Circle(lx, ly, 7.5, ui.GetVar("card", Color(37, 40, 45)))
    draw.Circle(lx, ly, 4.5, accent)

    local cx, cy, r = w - 90, h / 2 + 10, 52
    local parts, colors = { 35, 33, 18, 14 }, { accent, Color(92, 156, 255), Color(90, 200, 140), Color(200, 120, 255) }
    surface.SetLineWidth(16)
    surface.SetLineCap("butt")
    local a = -90
    for i, p in ipairs(parts) do
        local sweep = 360 * p / 100
        surface.SetDrawColor(colors[i])
        surface.DrawArc(cx, cy, r, a, a + sweep - 2)
        a = a + sweep
    end
    draw.SimpleText("$11,960", "DefaultBold", cx, cy, ui.GetVar("text", color_white), TEXT_ALIGN_CENTER, TEXT_ALIGN_CENTER)
end

ui.Register("VectorDemo", VECTOR, "Panel")

local THEMES = {
    graphite = { card = Color(29, 31, 36), text = Color(236, 237, 240), muted = Color(150, 156, 168), series = Color(232, 156, 30) },
    porcelain = { card = Color(255, 255, 255), text = Color(24, 28, 38), muted = Color(96, 104, 120), series = Color(184, 110, 0) },
}

---@class PaintPage: DefaultPage
local PAGE = {}
PAGE.Title = "Paint"

function PAGE:Initialize()
    local intro = ui.Create("Label", self.Content)
    intro:SetWrap(true)
    intro:SetTextColor(Color(154, 160, 170))
    intro:SetText("Paint рисует на canvas и вызывается только при инвалидации: создание, изменение размера, hover/нажатие, InvalidatePaint(). Поменяй ширину окна или нажми кнопку.")

    local chart = ui.Create("BarChart", self.Content)

    local row = ui.Create("Row", self.Content)
    row:SetGap(10)

    local shuffle = ui.Create("FancyButton", row)
    shuffle:SetWide(200)
    shuffle:SetText("Перемешать")
    shuffle.DoClick = function() chart:Shuffle() end

    local inline = ui.Create("Button", row)
    inline:SetWide(200)
    inline:SetText("Paint через поле")
    inline.Paint = function(s, w, h)
        local c = s:IsHovered() and Color(90, 200, 140) or Color(60, 130, 95)
        draw.RoundedBox(h / 2, 0, 0, w, h, c)
    end

    local shapes = ui.Create("Panel", self.Content)
    shapes:SetTall(120)
    function shapes:Paint(w, h)
        draw.RoundedBox(10, 0, 0, w, h, Color(37, 40, 45))
        draw.Circle(60, h / 2, 34, Color(255, 120, 120))
        surface.SetDrawColor(120, 220, 255)
        surface.DrawPoly({ { x = 130, y = h - 26 }, { x = 170, y = 26 }, { x = 210, y = h - 26 } })
        surface.SetDrawColor(255, 255, 255, 120)
        surface.DrawOutlinedRect(240, 26, 120, h - 52, 2)
        draw.SimpleText("center", "Large", 300, h / 2, color_white, TEXT_ALIGN_CENTER, TEXT_ALIGN_CENTER)
    end

    local vector = self:AddSection("Векторы: DrawPath, DrawArc, SetLineWidth, SetDrawGradient, ui.SetVars")
    ui.SetVars(THEMES.graphite)
    local card = ui.Create("Panel", vector, { DockPadding = { 14, 12, 14, 12 }, Gap = 4, Radius = 10 })
    card:SetBackgroundColor(ui.Var("card"))
    ui.Create("Label", card, { Text = "Карточка на CSS-переменных", TextColor = ui.Var("text") })
    ui.Create("Label", card, { Text = "$ 1 111.11  $ 8 888.88", TextColor = ui.Var("muted"),
        Style = { fontVariantNumeric = "tabular-nums" } })
    ui.Create("VectorDemo", vector)
    local current = "graphite"
    ui.Create("Button", vector, {
        Text = "Сменить тему (ui.SetVars)",
        AlignSelf = ALIGN_START,
        DoClick = function()
            current = current == "graphite" and "porcelain" or "graphite"
            ui.SetVars(THEMES[current])
        end,
    })
end

return ui.Register("PaintPage", PAGE, "DefaultPage")
