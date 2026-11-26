---@class FancyButton: Button
---@field Accent Color
---@field Fade number
local BTN = {}

function BTN:Initialize()
    self.Accent = Color(92, 156, 255)
    self.Fade = 0
    self:SetTall(40)
    self:SetFont("DefaultBold")
end

-- Think runs every frame, but Paint only when Fade actually changes.
function BTN:Think()
    local target = self:IsHovered() and 1 or 0
    if self.Fade == target then return end
    self.Fade = Lerp(math.min(FrameTime() * 10, 1), self.Fade, target)
    if math.abs(self.Fade - target) < 0.02 then self.Fade = target end
    self:InvalidatePaint()
end

function BTN:Paint(w, h)
    local a = self.Accent
    draw.RoundedBox(10, 0, 0, w, h, Color(a.r, a.g, a.b, 70 + 185 * self.Fade))
    if self:IsDown() then
        draw.RoundedBox(10, 0, 0, w, h, Color(0, 0, 0, 70))
    end
end

ui.Register("FancyButton", BTN, "Button")
