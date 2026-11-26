---@class Card: Panel
---@field TitleLabel Label
---@field Body Column
---@field Footer Row
local CARD = {}

-- Children created with the card as parent land in Body; { Slot = "footer" } puts them in Footer.
function CARD:Initialize()
    self:SetGap(10)
    self:DockPadding(18, 16, 18, 18)
    self:SetRadius(10)
    self:SetBorder(1, Color(255, 255, 255, 14))
    self:SetBackgroundColor(Color(37, 40, 45))

    self.TitleLabel = ui.Create("Label", self, { Font = "DefaultBold", TextColor = Color(154, 160, 170) })
    self.Body = ui.Create("Column", self, { Gap = 10 })
    self.Footer = ui.Create("Row", self, { Gap = 8 })
    self:SetSlot(self.Body)
    self:SetSlot("footer", self.Footer)
end

function CARD:SetTitle(text) self.TitleLabel:SetText(text) end

ui.Register("Card", CARD, "Panel")
