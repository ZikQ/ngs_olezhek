surface.CreateFont("Header", { size = 20, weight = 700 })
surface.CreateFont("Big", { font = "Source Serif 4", size = 36, weight = 700, url = "/fonts/SourceSerif4-Bold.woff2" })
surface.CreateFont("Mono", { font = "Fira Mono", size = 13, url = "/fonts/FiraMono-Regular.woff2" })

-- The features page saves the chosen accent; storage keeps it across reloads.
local saved = storage.Get("accent")
ui.SetSkin({
    accent = saved and Color(saved.r, saved.g, saved.b) or Color(92, 156, 255),
    radius = 8,
})
