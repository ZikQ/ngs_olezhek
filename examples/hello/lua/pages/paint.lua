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

---@class PaintPage: DefaultPage
local PAGE = {}
PAGE.Route = "/paint"
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
end

ui.Register("PaintPage", PAGE, "DefaultPage")
