local PAGE = {}
PAGE.Route = "/"

function PAGE:Initialize()
    ui.Create("Label", self):SetText(GREETING)
end

ui.Register("HomePage", PAGE, "Page")
