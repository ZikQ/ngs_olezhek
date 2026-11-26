//! Runs the browser runtime's Lua prelude natively against a mocked `N` (src/testing/mock.lua).
//! Covers the Lua side only; the natives themselves need a browser.

use ngs_olezhek::testing::Runtime;
use omnilua::Lua;

fn runtime() -> Lua {
    Runtime::new().lua().clone()
}

/// Runs a test script; it asserts on its own. Errors the runtime reported but the script did not
/// consume with MOCK.take_errors() fail the test.
fn run(script: &str) {
    let lua = runtime();
    lua.load(script).set_name("@test").exec().unwrap_or_else(|e| panic!("{e}"));
    let left: Vec<String> = lua.load("return MOCK.errors").eval().unwrap();
    assert!(left.is_empty(), "unexpected runtime errors: {left:#?}");
}

#[test]
fn link_rejects_dangerous_schemes() {
    run(r##"
        local l = ui.Create("Link")
        assert(l:SetURL("https://example.com/a") == true)
        assert(MOCK.el(l).attrs.href == "https://example.com/a")
        local bad = { "javascript:alert(1)", " JavaScript:alert(1)", "java\tscript:alert(1)", "\0javascript:x",
            "jav\nascript:x", "data:text/html,<b>", "vbscript:x" }
        for _, url in ipairs(bad) do
            assert(l:SetURL(url) == false, url)
            assert(MOCK.el(l).attrs.href == nil, url)
        end
        assert(#MOCK.take_errors() == #bad)
        for _, url in ipairs({ "/profile/1", "?q=1", "#top", "mailto:a@b.c", "rel/path", "//cdn.example.com/x", "HTTPS://X.COM" }) do
            assert(l:SetURL(url), url)
        end
    "##);
}

#[test]
fn link_routes_only_same_origin_paths() {
    run(r##"
        local l = ui.Create("Link")
        l:SetURL("/\\evil.com")
        assert(l:__on_click(true) == false, "browser handles it")
        assert(MOCK.path == "/")
        l:SetURL("/profile/2")
        assert(l:__on_click(true) == true)
        assert(MOCK.path == "/profile/2")
    "##);
}

#[test]
fn open_url_checks_scheme() {
    run(r##"
        assert(gui.OpenURL("javascript:alert(1)") == false)
        assert(router.Open("data:text/html,x") == false)
        assert(#MOCK.opened == 0)
        assert(#MOCK.take_errors() == 2)
        assert(router.Open("https://example.com") == true)
        assert(MOCK.opened[1] == "https://example.com")
    "##);
}

#[test]
fn image_src_allows_data_but_not_javascript() {
    run(r##"
        local img = ui.Create("Image")
        img:SetImage("data:image/png;base64,AAAA")
        assert(MOCK.el(img).attrs.src:sub(1, 5) == "data:")
        img:SetImage("javascript:alert(1)")
        assert(MOCK.el(img).attrs.src == nil)
        assert(#MOCK.take_errors() == 1)
    "##);
}

#[test]
fn attrs_and_props_are_allow_listed() {
    run(r##"
        local e = ui.Create("TextEntry")
        e:SetAttr("aria-label", "Name")
        e:SetAttr("autocomplete", "email")
        e:SetAttr("data-test", 5)
        e:SetAttr("required", true)
        local c = MOCK.el(e).control
        assert(c.attrs["aria-label"] == "Name" and c.attrs.autocomplete == "email")
        assert(c.attrs["data-test"] == "5" and c.attrs.required == "")
        assert(e:GetAttr("aria-label") == "Name")
        for _, name in ipairs({ "onclick", "ONERROR", "style", "srcdoc", "formaction", "data-ngs", "target", "class" }) do
            assert(not pcall(e.SetAttr, e, name, "x"), name)
        end
        e:SetAttr("href", "javascript:alert(1)")
        assert(c.attrs.href == nil)
        assert(#MOCK.take_errors() == 1)

        e:SetProp("value", "hi")
        assert(e:GetValue() == "hi")
        assert(not pcall(e.SetProp, e, "innerHTML", "<img src=x onerror=alert(1)>"))
        assert(not pcall(e.SetProp, e, "value", {}))
        assert(not pcall(e.Invoke, e, "remove"))
        e:Invoke("select")
        assert(c.invoked[1] == "select")
    "##);
}

#[test]
fn custom_native_tags() {
    run(r##"
        ui.Register("Video", { Tag = "video" }, "Panel")
        local v = ui.Create("Video")
        assert(MOCK.el(v).tag == "video")
        v:SetAttr("src", "/clip.mp4")
        v:SetProp("muted", true)
        v:Invoke("play")
        ui.Register("Evil", { Tag = "script" }, "Panel")
        assert(not pcall(ui.Create, "Evil"))
        ui.Register("Evil2", { __native = { tag = "iframe" } }, "Panel")
        assert(not pcall(ui.Create, "Evil2"))
        ui.Register("Evil3", { Tag = "img onerror" }, "Panel")
        assert(not pcall(ui.Create, "Evil3"))
    "##);
}

#[test]
fn typed_style_setters() {
    run(r##"
        local p = ui.Create("Panel")
        local s = MOCK.el(p).style
        p:SetRadius(8)
        assert(s["border-radius"] == "8px")
        p:SetRadius(8, 8, 0, 0)
        assert(s["border-radius"] == "8px 8px 0px 0px")
        p:SetBorder(2, Color(255, 0, 0), "dashed")
        assert(s.border == "2px dashed rgba(255,0,0,1.000)", s.border)
        p:SetBorder(false)
        assert(s.border == "none")
        p:SetBorder(nil)
        assert(s.border == nil)
        p:SetShadow(0, 4, 12, Color(0, 0, 0, 255))
        assert(s["box-shadow"] == "0px 4px 12px 0px rgba(0,0,0,1.000)", s["box-shadow"])
        p:SetGradient(Color(255, 0, 0), Color(0, 0, 255), 90)
        assert(s["background-image"] == "linear-gradient(90deg, rgba(255,0,0,1.000), rgba(0,0,255,1.000))")
        p:SetTransform({ x = 10, rotate = 45, scale = 1.5 })
        assert(s.transform == "translate(10px, 0px) rotate(45deg) scale(1.5)", s.transform)
        p:SetOverflow("hidden", "auto")
        assert(s["overflow-x"] == "hidden" and s["overflow-y"] == "auto")
        p:SetStyle("letterSpacing", 2)
        p:SetStyle({ ["font-weight"] = 600, opacity = 0.5 })
        assert(s["letter-spacing"] == "2px" and s["font-weight"] == "600" and s.opacity == "0.5")
        p:SetTransition(0.2)
        assert(s.transition == "all 0.2s ease")
        p:AlphaTo(100, 0.5)
        assert(s.transition:find("^all 0.2s ease, opacity 0.5s"), s.transition)
    "##);
}

#[test]
fn style_values_cannot_fetch_or_escape() {
    run(r##"
        local p = ui.Create("Panel")
        local bad = {
            { "background", "url(https://evil/?x)" },
            { "background-image", "image-set('a.png' 1x)" },
            { "color", "red; background: blue" },
            { "color", "u\\72l(x)" },
            { "position", "fixed" },
            { "--x", "1" },
            { "display", "none" },
        }
        for _, b in ipairs(bad) do
            assert(not pcall(p.SetStyle, p, b[1], b[2]), b[1] .. " " .. b[2])
        end
        assert(not pcall(p.SetStyleHover, p, { background = "url(x)" }))
        assert(not pcall(p.SetOverflow, p, "everywhere"))
        p:SetStyle("width", "calc(100% - 20px)")
        assert(MOCK.el(p).style.width == "calc(100% - 20px)")
    "##);
}

#[test]
fn typography_and_scrollbar_props_are_allowed() {
    run(r##"
        local p = ui.Create("Panel")
        local s = MOCK.el(p).style
        p:SetStyle({ fontVariantNumeric = "tabular-nums", fontFeatureSettings = '"tnum" 1, "cv11" 1' })
        assert(s["font-variant-numeric"] == "tabular-nums")
        assert(s["font-feature-settings"] == '"tnum" 1, "cv11" 1')
        p:SetStyle({ fontVariationSettings = '"opsz" 32', caretColor = Color(1, 2, 3), accentColor = Color(4, 5, 6) })
        assert(s["font-variation-settings"] == '"opsz" 32' and s["caret-color"] == "rgba(1,2,3,1.000)")
        p:SetStyle({ scrollbarWidth = "thin", scrollbarColor = "rgba(255,255,255,0.14) transparent" })
        assert(s["scrollbar-width"] == "thin" and s["scrollbar-color"] == "rgba(255,255,255,0.14) transparent")
    "##);
}

#[test]
fn app_vars_are_checked_and_usable_in_styles() {
    run(r##"
        local repainted = 0
        local canvas = ui.Create("Panel")
        canvas.Paint = function() end
        local orig = canvas.InvalidatePaint
        canvas.InvalidatePaint = function(self) repainted = repainted + 1; return orig(self) end
        local changed = 0
        hook.Add("OnSkinChanged", "t", function() changed = changed + 1 end)

        local surface2 = Color(30, 32, 36)
        ui.SetVars({ surface2 = surface2, radiusCard = 12, shadow = "0 1px 2px rgba(0,0,0,0.3)" })
        assert(MOCK.vars["--app-surface2"] == "rgba(30,32,36,1.000)")
        assert(MOCK.vars["--app-radiusCard"] == "12px")
        assert(MOCK.vars["--app-shadow"] == "0 1px 2px rgba(0,0,0,0.3)")
        assert(ui.GetVar("surface2") == surface2, "Paint gets the Color back")
        assert(ui.GetVar("missing", 7) == 7)
        assert(changed == 1 and repainted == 1, changed .. " " .. repainted)

        assert(ui.Var("surface2") == "var(--app-surface2)")
        local p = ui.Create("Panel")
        p:SetBackgroundColor(ui.Var("surface2"))
        p:SetTextColor(ui.Var("text"))
        assert(MOCK.el(p).style["background-color"] == "var(--app-surface2)")
        assert(MOCK.el(p).style.color == "var(--app-text)")
        p:SetStyleHover({ background = ui.Var("surface2") })

        ui.SetVars({ shadow = false })
        assert(MOCK.vars["--app-shadow"] == "")
        assert(ui.GetVar("shadow") == nil)

        assert(not pcall(ui.SetVars, { bg = "url(https://evil/x)" }), "a var value lands in styles")
        assert(not pcall(ui.SetVars, { bg = "red; x: y" }))
        assert(not pcall(ui.SetVars, { ["bad name"] = 1 }))
        assert(not pcall(ui.SetVars, { ["-x"] = 1 }))
        assert(not pcall(ui.Var, "a)b"))
        assert(not pcall(p.SetBackgroundColor, p, "url(x)"))

        assert(not pcall(ui.SetSkin, { background = "url(https://evil/x)" }), "SetSkin strings are checked too")
        ui.SetSkin({ accent = Color(245, 166, 35), radius = 8, text = "#e4e6eb" })
        assert(MOCK.vars["--ngs-radius"] == "8px" and MOCK.vars["--ngs-fg"] == "#e4e6eb")
        assert(ui.GetSkin("accent") == Color(245, 166, 35) and ui.GetSkin("danger") == nil)
    "##);
}

#[test]
fn surface_vector_api_checks_arguments() {
    run(r##"
        surface.SetLineWidth(2)
        surface.SetLineCap("round")
        surface.SetLineJoin("round")
        surface.SetLineDash({ 4, 2 })
        surface.SetLineDash()
        surface.SetDrawGradient(0, 0, 0, 100, { Color(255, 0, 0, 41), Color(255, 0, 0, 0) })
        surface.SetDrawGradient(0, 0, 100, 0, { { 0, Color(1, 1, 1) }, { 0.3, Color(2, 2, 2) }, { 1, Color(3, 3, 3) } })
        surface.DrawArc(50, 50, 42, -90, 30)
        surface.DrawPath("M7 5v14M17 5v14M7 12h10", "stroke", 0, 0, 0.75)
        surface.DrawPath("M0 0 L10 0 L10 10 Z")

        assert(not pcall(surface.SetLineCap, "pointy"))
        assert(not pcall(surface.SetLineJoin, "sharp"))
        assert(not pcall(surface.SetLineDash, { 4, -1 }))
        assert(not pcall(surface.SetDrawGradient, 0, 0, 1, 1, { Color(1, 1, 1) }))
        assert(not pcall(surface.SetDrawGradient, 0, 0, 1, 1, { Color(1, 1, 1), "red" }))
        assert(not pcall(surface.DrawPath, "M0 0", "outline"))
        assert(not pcall(surface.DrawPath, nil))
    "##);
}

#[test]
fn popover_holds_any_content_and_toggles_from_its_anchor() {
    run(r##"
        local page = ui.Create("Panel")
        local button = ui.Create("Button", page, { Text = "Дата" })
        local pop = ui.Create("Popover", page)
        local grid = ui.Create("Grid", pop, { Columns = 7 })
        for d = 1, 31 do ui.Create("Button", grid, { Text = d }) end
        assert(grid:GetParent() == pop and grid:ChildCount() == 31)
        assert(MOCK.el(pop).attrs.popover == "auto")

        local opened, closed = 0, 0
        pop.OnOpen = function() opened = opened + 1 end
        pop.OnClose = function() closed = closed + 1 end

        pop:Toggle(button)
        assert(pop:IsOpen() and pop:GetAnchor() == button)
        assert(MOCK.el(pop).invoked[1] == "showPopover")
        assert(MOCK.el(pop).style.position == "fixed", "placed in window coordinates")
        assert(opened == 1)

        -- Light dismiss on pointerup, then the anchor's click: the click must not reopen it.
        pop:__on_toggle(false)
        assert(not pop:IsOpen() and closed == 1)
        pop:Toggle(button)
        assert(not pop:IsOpen(), "click right after light dismiss is swallowed")
        MOCK.advance(1)
        pop:Toggle(button)
        assert(pop:IsOpen() and opened == 2)
        pop:Toggle(button)
        assert(not pop:IsOpen() and closed == 2)
        pop:Hide()
        assert(closed == 2, "OnClose fires once")
        assert(IsValid(pop), "kept for reuse by default")

        pop:SetDeleteSelf(true)
        pop:Open(10, 20)
        pop:Hide()
        assert(not IsValid(pop))

        local p2 = ui.Create("Popover")
        p2:SetAutoClose(false)
        assert(MOCK.el(p2).attrs.popover == "manual")
        assert(not pcall(p2.SetSide, p2, "under"))
        assert(not pcall(p2.SetAlign, p2, "middle"))
    "##);
}

#[test]
fn state_styles_compile_to_shared_classes() {
    run(r##"
        local a, b = ui.Create("Button"), ui.Create("Button")
        a:SetStyleHover({ background = Color(1, 2, 3), radius = 10 })
        b:SetStyleHover({ radius = 10, background = Color(1, 2, 3) })
        assert(#MOCK.rules == 1, "identical styles share one rule")
        local rule = MOCK.rules[1]
        assert(rule.selector == ".ngs-s1:hover:not(.ngs-disabled):not(:disabled)", rule.selector)
        assert(rule.decls[1] == "background-color" and rule.decls[2] == "rgba(1,2,3,1.000)")
        assert(rule.decls[3] == "border-radius" and rule.decls[4] == "10px")
        assert(MOCK.el(a).classes["ngs-s1"] and MOCK.el(b).classes["ngs-s1"])
        a:SetStyleDisabled({ alpha = 128 })
        assert(MOCK.rules[2].selector == ".ngs-s2.ngs-disabled, .ngs-s2:disabled")
        a:SetStyleHover({ background = Color(9, 9, 9) })
        assert(not MOCK.el(a).classes["ngs-s1"] and MOCK.el(a).classes["ngs-s3"])
        a:SetStyleHover(nil)
        assert(not MOCK.el(a).classes["ngs-s3"])
        assert(not pcall(a.SetStyleState, a, "visited", {}))
    "##);
}

#[test]
fn panel_hooks_and_timers_die_with_the_panel() {
    run(r##"
        local p = ui.Create("Panel")
        local fonts, ticks, once = 0, 0, 0
        p:Hook("OnFontLoaded", function(family) fonts = fonts + 1 end)
        p:Hook("OnFontLoaded", "extra", function() fonts = fonts + 10 end)
        function p:Grow() ticks = ticks + 1 end
        p:Timer("grow", 1, 0, "Grow")
        p:TimerSimple(5, function() once = once + 1 end)
        R.font_loaded("Inter")
        assert(fonts == 11)
        MOCK.advance(3.5)
        assert(ticks == 3)
        assert(p:TimerExists("grow"))
        p:Remove()
        R.font_loaded("Fira")
        MOCK.advance(10)
        assert(fonts == 11 and ticks == 3 and once == 0)
        assert(next(hook.GetTable().OnFontLoaded) == nil)
        assert(next(MOCK.timers) == nil, "timeouts cleared")
    "##);
}

#[test]
fn removed_page_hooks_are_gone_after_navigation() {
    run(r##"
        local PAGE = { Route = "/w" }
        function PAGE:Initialize()
            local label = ui.Create("Label", self)
            self:Hook("OnFontLoaded", function() label:SetText("loaded") end)
        end
        ui.Register("WPage", PAGE, "Page")
        router.Go("/w")
        router.Go("/")
        R.font_loaded("X")
        assert(next(hook.GetTable().OnFontLoaded) == nil)
    "##);
}

#[test]
fn props_setters_fields_and_children() {
    run(r##"
        local seen
        ui.Register("Card", {
            Initialize = function(self)
                seen = self.User
                self.Body = ui.Create("Panel", self)
                self.Footer = ui.Create("Row", self)
                self:SetSlot(self.Body)
                self:SetSlot("footer", self.Footer)
            end,
            SetTitle = function(self, t) self.TitleText = t end,
        }, "Panel")

        local card = ui.Create("Card", nil, {
            User = { name = "Олежек" },
            Title = "Профиль",
            Size = { 300, 200 },
            Dock = DOCK_TOP,
            ui.Create("Label", nil, { Text = "inside" }),
            function(c) ui.Create("Button", c, { Text = "ok", Slot = "footer" }) end,
        })
        assert(seen.name == "Олежек", "plain props are fields before Initialize")
        assert(card.TitleText == "Профиль", "setter props run after it")
        assert(card.Props.Title == "Профиль")
        assert(MOCK.el(card).style.width == nil and card:GetDock() == DOCK_TOP)
        assert(card.Body:ChildCount() == 1 and card.Body:GetChild(1):GetText() == "inside")
        assert(card.Footer:ChildCount() == 1 and card.Footer:GetChild(1):GetText() == "ok")
        assert(card:ChildCount() == 2)
        local direct = ui.Create("Panel", card, { Slot = false })
        assert(direct:GetParent() == card)
        assert(not pcall(ui.Create, "Panel", card, { Slot = "nope" }))

        local s = ui.Create("Slider", nil, { Min = 10, Max = 20, Value = 15 })
        assert(s:GetMin() == 10 and s:GetMax() == 20, "setters run after Initialize's SetMinMax(0, 100)")
        local b = ui.Create("Button", nil, { Text = "Сохранить", DoClick = function() end, StyleHover = { radius = 4 } })
        assert(b:GetText() == "Сохранить" and type(b.DoClick) == "function")
        assert(MOCK.el(b).classes["ngs-s1"])
    "##);
}

#[test]
fn provide_inject_and_change_notification() {
    run(r##"
        ui.Provide("theme", "dark")
        local outer = ui.Create("Panel")
        local inner = ui.Create("Panel", outer)
        local leaf = ui.Create("Panel", inner)
        assert(leaf:Inject("theme") == "dark")
        assert(leaf:Inject("missing", 42) == 42)
        assert(ui.Inject("theme") == "dark")
        outer:Provide("user", { name = "a" })
        local got
        function leaf:OnContextChanged(key, value) got = { key, value } end
        assert(leaf:Inject("user").name == "a")
        outer:Provide("user", { name = "b" })
        assert(got[1] == "user" and got[2].name == "b")
        inner:Provide("theme", "light")
        assert(leaf:Inject("theme") == "light", "nearest provider wins")
    "##);
}

#[test]
fn http_default_headers_stay_on_this_origin() {
    run(r##"
        http.SetHeader("Authorization", "Bearer t")
        http.SetHeader("X-CSRF-Token", "c")
        R.spawn(function() http.Get("/api/me") end)
        R.spawn(function() http.Get("http://localhost:3000/api/me") end)
        R.spawn(function() http.Get("https://other.example/api") end)
        R.spawn(function() http.Get("//other.example/api") end)
        R.spawn(function() http.Post("/api/x", { a = 1 }, { headers = { ["X-Extra"] = "1", Authorization = false } }) end)
        local f = MOCK.fetches
        assert(f[1].headers.Authorization == "Bearer t" and f[1].headers["X-CSRF-Token"] == "c")
        assert(f[2].headers.Authorization == "Bearer t")
        assert(f[3].headers.Authorization == nil and f[4].headers.Authorization == nil)
        assert(f[5].headers["Content-Type"] == "application/json" and f[5].headers["X-Extra"] == "1")
        assert(f[5].headers.Authorization == nil, "per-request false removes a default")
        assert(f[5].body == '{"a":1}')
    "##);
}

#[test]
fn http_timeout_cancel_and_owner_removal() {
    run(r##"
        local r1
        R.spawn(function() r1 = { http.Get("/slow", { timeout = 1 }) } end)
        MOCK.advance(1.5)
        assert(r1[1] == nil and r1[2].timeout and r1[2].status == 0)
        assert(MOCK.aborted[MOCK.fetches[1].op])
        MOCK.respond(1, 200, "late")
        assert(r1[2].timeout, "a late answer is ignored")

        local req = http.Start("GET", "/x")
        local r2
        R.spawn(function() r2 = { req:Wait() } end)
        req:Cancel()
        assert(r2[2].cancelled and MOCK.aborted[req.op])

        local page = ui.Create("Panel")
        local resumed = false
        R.spawn_for(page, function()
            http.Get("/api/data")
            resumed = true
        end)
        page:Remove()
        assert(MOCK.aborted[MOCK.fetches[3].op], "leaving the page aborts its requests")
        MOCK.respond(3, 200, "x")
        assert(not resumed)

        local cb
        http.Fetch("/cb", function(v) cb = v end)
        MOCK.respond(4, 200, "ok")
        assert(cb == "ok")
    "##);
}

#[test]
fn http_spa_shell_is_a_404() {
    run(r##"
        local v, err
        R.spawn(function() v, err = http.Get("/api/typo") end)
        MOCK.respond(1, 200, "<!doctype html>", "text/html", true)
        assert(v == nil and err.status == 404)
    "##);
}

#[test]
fn query_dedupes_caches_and_revalidates() {
    run(r##"
        local a, b
        R.spawn(function() a = http.Query("/api/users/1") end)
        R.spawn(function() b = http.Query("/api/users/1") end)
        assert(#MOCK.fetches == 1, "concurrent queries share a request")
        MOCK.json(1, { name = "a" })
        assert(a.name == "a" and b == a)

        local c
        R.spawn(function() c = http.Query("/api/users/1") end)
        assert(c == a and #MOCK.fetches == 1, "fresh answers come from the cache")

        local updates = 0
        local p = ui.Create("Panel")
        function p:OnQueryUpdate(key, value) updates = updates + 1; self.Last = value end
        MOCK.advance(5)
        local d
        R.spawn_for(p, function() d = p:UseQuery("/api/users/1") end)
        assert(d.name == "a", "stale data is returned at once")
        assert(#MOCK.fetches == 2, "and refreshed in the background")
        MOCK.json(2, { name = "a" })
        assert(updates == 0, "unchanged data does not notify")
        MOCK.advance(5)
        R.spawn_for(p, function() p:UseQuery("/api/users/1") end)
        MOCK.json(3, { name = "b" })
        assert(updates == 1 and p.Last.name == "b")
    "##);
}

#[test]
fn invalidate_refetches_subscribed_and_drops_the_rest() {
    run(r##"
        local builds = 0
        ui.Register("UserView", {
            Initialize = function(self)
                builds = builds + 1
                local u = self:UseQuery("/api/users/1")
                ui.Create("Label", self, { Text = u and u.name or "?" })
            end,
        }, "Panel")
        local v = ui.Create("UserView")
        MOCK.json(1, { name = "a" })
        assert(builds == 1)
        R.spawn(function() http.Query("/api/other") end)
        MOCK.json(2, { x = 1 })

        R.spawn(function() http.Post("/api/users/1/rename", { name = "b" }, { invalidate = "/api/users" }) end)
        MOCK.json(3, { ok = true })
        assert(#MOCK.fetches == 4 and MOCK.fetches[4].url == "/api/users/1", "subscribed query refetched")
        MOCK.json(4, { name = "b" })
        assert(builds == 2, "no OnQueryUpdate: the panel is rebuilt")
        assert(v:ChildCount() == 1 and v:GetChild(1):GetText() == "b")

        http.Invalidate("/api/other")
        assert(http.GetData("/api/other") == nil)
        assert(#MOCK.fetches == 4, "unsubscribed entries are only dropped")

        http.SetData("/api/users/1", { name = "c" })
        assert(builds == 3 and v:GetChild(1):GetText() == "c")
    "##);
}

#[test]
fn pages_use_cached_queries_across_navigation() {
    run(r##"
        local builds = 0
        local PAGE = { Route = "/u/{id}" }
        function PAGE:Initialize()
            builds = builds + 1
            local u = self:UseQuery("/api/users/" .. self.Params.id)
            self.Name = u and u.name
        end
        ui.Register("UPage", PAGE, "Page")
        router.Go("/u/1?tab=info")
        assert(router.GetPage().Query.tab == "info", "Page.Query is still the query string")
        MOCK.json(1, { name = "a" })
        assert(router.GetPage().Name == "a")
        router.Go("/")
        router.Go("/u/1")
        assert(#MOCK.fetches == 1 and router.GetPage().Name == "a", "cached page renders without waiting")
        router.Go("/")
        MOCK.advance(10)
        router.Go("/u/1")
        assert(router.GetPage().Name == "a" and #MOCK.fetches == 2, "stale: shown at once, refreshed")
        MOCK.json(2, { name = "b" })
        assert(router.GetPage().Name == "b", "the page rebuilt with the new data")
    "##);
}

#[test]
fn rebuild_drops_the_previous_initialize() {
    run(r##"
        local done = 0
        ui.Register("Slow", {
            Initialize = function(self)
                http.Get("/api/slow")
                done = done + 1
            end,
        }, "Panel")
        local s = ui.Create("Slow")
        s:Rebuild()
        MOCK.respond(1, 200, "x")
        assert(done == 0, "first Initialize is not resumed after Rebuild")
        MOCK.respond(2, 200, "x")
        assert(done == 1)

        local ticks, fonts = 0, 0
        ui.Register("Ticker", {
            Initialize = function(self)
                if not self.Second then
                    self.Second = true
                    self:Timer("old", 1, 0, function() ticks = ticks + 1 end)
                    self:Hook("OnFontLoaded", function() fonts = fonts + 1 end)
                end
            end,
        }, "Panel")
        local t = ui.Create("Ticker")
        t:Rebuild()
        MOCK.advance(3)
        R.font_loaded("X")
        assert(ticks == 0 and fonts == 0, "bindings of the previous Initialize are dropped")
        t:Hook("OnFontLoaded", function() fonts = fonts + 1 end)
        R.font_loaded("Y")
        assert(fonts == 1)
        t:Remove()
        R.font_loaded("Z")
        assert(fonts == 1, "and new ones still die with the panel")
    "##);
}

#[test]
fn storage_round_trip_and_fallback() {
    run(r##"
        storage.Set("theme", { dark = true, size = 14 })
        assert(storage.Get("theme").dark == true and storage.Get("theme").size == 14)
        assert(MOCK.storage[false]["ngs:theme"] ~= nil)
        storage.Set("token", "abc")
        assert(storage.Get("token") == "abc")
        assert(storage.Get("missing", "def") == "def")
        local keys = storage.Keys()
        assert(#keys == 2 and keys[1] == "theme" and keys[2] == "token")
        MOCK.storage[false]["other-app"] = "keep"
        storage.Clear()
        assert(storage.Get("token") == nil and MOCK.storage[false]["other-app"] == "keep")

        storage.Session.Set("step", 3)
        assert(storage.Session.Get("step") == 3 and storage.Get("step") == nil)

        MOCK.storage_broken = true
        assert(storage.Set("x", 1) == false)
        assert(storage.Get("x") == 1, "kept in memory")

        local changed
        hook.Add("OnStorageChanged", "t", function(k, v) changed = { k, v } end)
        MOCK.storage[false]["ngs:remote"] = '"v"'
        R.storage_changed("ngs:remote")
        assert(changed[1] == "remote" and changed[2] == "v")
    "##);
}

#[test]
fn guarded_navigation_keeps_layout_and_restores_the_page() {
    run(r##"
        local function layout(self) self:SetOutlet(self:Add("Panel")) end
        ui.Register("Shell", { Initialize = layout }, "Panel")
        ui.Register("Inner", { Initialize = layout }, "Panel")
        local allowed = true
        ui.Register("A", { Route="/a/{id}", Layout={"Shell","Inner"}, KeepAlive=true,
            Guard=function() return allowed end,
            Loader=function(ctx) return ctx.Params.id .. ctx.Query.q end,
            Initialize=function(self) self.input = self:Add("TextEntry") end }, "Page")
        ui.Register("B", {Route="/b", Layout="Shell"}, "Page")
        router.Go("/a/1?q=x")
        local a = router.GetPage()
        local inner = a:GetParent():GetParent()
        local shell = inner:GetParent():GetParent()
        assert(a.Data == "1x" and shell:GetClassName() == "Shell")
        a.input:SetValue("draft")
        a:SetProp("scrollTop", 75)
        router.Go("/b")
        assert(shell:IsValid() and not inner:IsValid() and a:IsValid())
        allowed = false
        router.Go("/a/1?q=x")
        assert(MOCK.path == "/b" and router.GetPage():GetClassName() == "B")
        local index = MOCK.historyIndex
        MOCK.path, MOCK.query, MOCK.historyIndex = "/a/1", "q=x", index - 1
        R.popstate(MOCK.path, MOCK.query)
        assert(MOCK.historyDelta == 1 and MOCK.path == "/a/1")
        MOCK.path, MOCK.query, MOCK.historyIndex = "/b", "", index
        R.popstate(MOCK.path, MOCK.query)
        allowed = true
        router.Go("/a/1?q=x")
        assert(router.GetPage() == a and a.input:GetValue() == "draft" and a:GetProp("scrollTop") == 75)
        router.Reload()
        assert(not a:IsValid() and router.GetPage() ~= a)
    "##);
}

#[test]
fn stale_loader_cannot_commit_and_failed_lazy_load_retries() {
    run(r##"
        ui.Register("Slow", {Route="/slow", Loader=function() return http.Get("/api/slow") end}, "Page")
        ui.Register("Fast", {Route="/fast"}, "Page")
        router.Go("/slow")
        router.Go("/fast")
        MOCK.respond(1, 200, "late")
        assert(MOCK.path == "/fast" and router.GetPage():GetClassName() == "Fast")
        router.Lazy("/later", "Later", "lazy/later.lua")
        router.Go("/later")
        R.op_done(MOCK.chunkOp, nil, "offline")
        assert(#MOCK.take_errors() == 1 and MOCK.path == "/fast")
        router.Go("/later")
        local op = MOCK.chunkOp
        router.Go("/later")
        assert(MOCK.chunkOp == op)
        R.op_done(op, function() ui.Register("Later", {}, "Page") end)
        assert(MOCK.path == "/later" and router.GetPage():GetClassName() == "Later")
    "##);
}

#[test]
fn virtual_grid_only_mounts_the_visible_rows() {
    run(r##"
        local grid = ui.Create("Table")
        grid:SetColumns({{key="id",title="Index"}})
        local rows = {}
        for i=1,10000 do rows[i] = {id=i} end
        grid:SetRows(rows)
        assert(grid.Canvas:ChildCount() <= 4)
        grid.View:SetProp("scrollTop", 32000)
        R.event(grid.View.__id, "scroll")
        assert(grid.visible[1] == nil and grid.visible[1001]:GetChild(1):GetText() == "1001")
        grid:SetRows({{id="only"}})
        assert(grid.Canvas:ChildCount() == 1 and grid.visible[1]:GetChild(1):GetText() == "only")
    "##);
}

#[test]
fn tabs_tree_confirm_and_rich_tooltip_lifetimes() {
    run(r##"
        local tabs = ui.Create("Tabs")
        local a, b = tabs:AddTab("A"), tabs:AddTab("B")
        tabs:Select(2)
        assert(not a:IsVisible() and b:IsVisible())
        local tree = ui.Create("Tree")
        local parent = tree:AddNode("A")
        local child = parent:AddNode("B", 42)
        parent.Toggle:DoClick()
        child.Toggle:DoClick()
        assert(parent.Body:IsVisible() and tree.selected.data == 42)
        local choices = {}
        local f = ui.Confirm("Continue?", function(yes) choices[#choices+1] = yes end)
        f:__on_cancel()
        assert(#choices == 1 and choices[1] == false)
        local button = ui.Create("Button")
        button:SetRichTooltip(function(panel) panel:Add("Label", {Text="tip"}) end)
        R.event(button.__id, "focus")
        local tip = button.__tip
        assert(tip:GetChild(1):GetText() == "tip")
        button:Remove()
        assert(not tip:IsValid())
    "##);
}

#[test]
fn storage_failed_overwrite_and_delete_override_the_old_value() {
    run(r##"
        storage.Set("key", 1)
        MOCK.storage_broken = true
        assert(not storage.Set("key", 2) and storage.Get("key") == 2)
        assert(not storage.Remove("key") and storage.Get("key") == nil and #storage.Keys() == 0)
    "##);
}

#[test]
fn navigation_inside_initialize_replaces_the_just_mounted_tree() {
    run(r##"
        ui.Register("Landing", {Route="/landing"}, "Page")
        ui.Register("Entry", {Route="/entry", Initialize=function() router.Replace("/landing") end}, "Page")
        router.Go("/entry")
        assert(MOCK.path == "/landing" and router.GetPage():GetClassName() == "Landing")
        assert(ui.GetRoot():ChildCount() == 1)
    "##);
}

#[test]
fn upload_preserves_http_errors_headers_and_cancellation() {
    run(r##"
        local input = ui.Create("FileInput", nil, {Accept="image/*", Multiple=true})
        local control = MOCK.el(input).control
        assert(control.attrs.type=="file" and control.attrs.accept=="image/*" and control.props.multiple)
        local err
        R.spawn(function() local _; _, err = http.Upload("/api/upload", input) end)
        assert(err.status == 0 and #MOCK.fetches == 0)
        MOCK.el(input).files = {{name="photo.jpg", size=7, type="image/jpeg"}}
        http.SetHeader("Authorization", "token")
        http.SetHeader("Content-Type", "application/json")
        R.spawn_for(input, function() http.Upload("/api/upload", input, "photos") end)
        local req = MOCK.fetches[1]
        assert(req.field=="photos" and req.headers.Authorization=="token" and req.headers["Content-Type"]==nil)
        input:Remove()
        assert(MOCK.aborted[req.op])
    "##);
}

#[test]
fn lazy_module_errors_keep_their_own_location() {
    run(r##"
        router.Lazy("/later", "Later", "lazy/later.lua")
        router.Go("/later")
        R.op_done(MOCK.chunkOp, load("error('boom')", "@lazy/later.lua"))
        local errors = MOCK.take_errors()
        assert(errors[1] == "lazy/later.lua:1: boom", errors[1])
    "##);
}

#[test]
fn second_navigation_waits_for_a_lazy_module_that_is_still_running() {
    run(r##"
        router.Lazy("/later", "Later", "lazy/later.lua")
        router.Go("/later")
        R.op_done(MOCK.chunkOp, function() http.Get("/api/x"); ui.Register("Later", {}, "Page") end)
        router.Go("/later")
        assert(#MOCK.errors == 0, MOCK.errors[1])
        MOCK.respond(1, 200, "x")
        assert(MOCK.path == "/later" and router.GetPage():GetClassName() == "Later")
    "##);
}

#[test]
fn keep_alive_page_closes_its_modal_before_parking() {
    run(r##"
        ui.Register("Keep", {Route="/keep", KeepAlive=true}, "Page")
        ui.Register("Other", {Route="/other"}, "Page")
        router.Go("/keep")
        local page = router.GetPage()
        local answers = {}
        ui.Confirm("?", function(ok) answers[#answers + 1] = ok end, nil, page)
        assert(#R.modals == 1)
        router.Go("/other")
        assert(#R.modals == 0 and answers[1] == false and page:IsValid())
    "##);
}

#[test]
fn cookie_set_handles_nil_and_fractional_max_age() {
    run(r##"
        cookie.Set("a b", "x;y", 90.7)
        assert(MOCK.cookies[1] == "a%20b=x%3By; Path=/; SameSite=Lax; Max-Age=90", MOCK.cookies[1])
        cookie.Set("a b", nil)
        assert(MOCK.cookies[2]:find("Max-Age=0", 1, true))
    "##);
}

#[test]
fn state_updates_only_the_bound_setter() {
    run(r##"
        local count = ui.State(0)
        local label = ui.Create("Label")
        local calls = 0
        local set = label.SetText
        label.SetText = function(self, s)
            calls = calls + 1
            return set(self, s)
        end
        label:Bind(count)
        assert(label:GetText() == "0" and calls == 1)
        count:Set(1)
        assert(label:GetText() == "1" and calls == 2)
        count:Set(1)
        assert(calls == 2, "same value, no update")

        local other = ui.Create("Label")
        other:Bind("Text", function() return "x" .. count() * 2 end)
        assert(other:GetText() == "x2")
        ui.Batch(function()
            count:Set(2)
            count:Set(3)
        end)
        assert(calls == 3 and label:GetText() == "3" and other:GetText() == "x6", "a batch updates once")

        local tip = ui.Create("Label", nil, { Text = count, Visible = ui.Computed(function() return count() > 3 end) })
        assert(tip:GetText() == "3" and not tip:IsVisible(), "states in props are bound")
        count:Set(4)
        assert(tip:IsVisible())

        assert(calls == 4)
        label:Remove()
        count:Set(5)
        assert(calls == 4, "a removed panel is unbound")
        assert(other:GetText() == "x10")
        assert(not pcall(ui.Create("Panel").Bind, ui.Create("Panel"), count), "Panel has no default property")
    "##);
}

#[test]
fn computed_is_lazy_and_glitch_free() {
    run(r##"
        local a = ui.State(1)
        local runs = 0
        local double = ui.Computed(function()
            runs = runs + 1
            return a:Get() * 2
        end)
        local sum = ui.Computed(function() return a:Get() + double:Get() end)
        local seen = {}
        local stop = ui.Effect(function() seen[#seen + 1] = sum:Get() end)
        assert(#seen == 1 and seen[1] == 3)
        a:Set(2)
        assert(#seen == 2 and seen[2] == 6, "the diamond settles before the effect runs, once")
        assert(runs == 2)
        stop()
        a:Set(3)
        assert(#seen == 2 and runs == 2, "nobody reads it, nothing recomputes")
        assert(double:Peek() == 6 and runs == 3)
        assert(not pcall(double.Set, double, 1))

        local got = {}
        local unsub = a:Subscribe(function(v) got[#got + 1] = v end)
        assert(#got == 0)
        a:Set(4)
        assert(got[1] == 4)
        unsub()
        a:Set(5)
        assert(#got == 1)

        local cleaned = 0
        local dispose = ui.Effect(function()
            a:Get()
            return function() cleaned = cleaned + 1 end
        end)
        a:Set(6)
        dispose()
        assert(cleaned == 2)
    "##);
}

#[test]
fn inputs_write_back_into_plain_state() {
    run(r##"
        local name = ui.State("Олежек")
        local e = ui.Create("TextEntry", nil, { Value = name })
        assert(e:GetValue() == "Олежек")
        local writes = 0
        local set = e.SetValue
        e.SetValue = function(self, v)
            writes = writes + 1
            return set(self, v)
        end
        MOCK.el(e).control.props.value = "Гарри"
        R.event(e.__id, "input")
        assert(name:Get() == "Гарри")
        assert(writes == 0, "the typed value is not written back into the same input")
        name:Set("Аликс")
        assert(e:GetValue() == "Аликс" and writes == 1)

        local on = ui.State(false)
        local cb = ui.Create("CheckBox", nil, { Checked = on })
        MOCK.el(cb).control.props.checked = true
        R.event(cb.__id, "change")
        assert(on:Get() == true)

        local n = ui.State(5)
        local f = ui.Create("TextEntry")
        f:Bind("Value", n, function(v) return "#" .. v end)
        assert(f:GetValue() == "#5")
        MOCK.el(f).control.props.value = "zzz"
        R.event(f.__id, "input")
        assert(n:Get() == 5, "a formatted binding is one-way")

        local s = ui.Create("Slider")
        local level = ui.State(30)
        s:Bind(level)
        MOCK.el(s).control.props.value = "70"
        R.event(s.__id, "input")
        assert(level:Get() == 70)
        s:Unbind()
        MOCK.el(s).control.props.value = "10"
        R.event(s.__id, "input")
        assert(level:Get() == 70)
    "##);
}

#[test]
fn rebuild_drops_old_bindings_and_effects() {
    run(r##"
        local s = ui.State("a")
        local runs = 0
        ui.Register("Bound", {
            Initialize = function(self)
                self.L = ui.Create("Label", self, { Text = s })
                self:Effect(function()
                    runs = runs + 1
                    self.Seen = s:Get()
                end)
            end,
        }, "Panel")
        local b = ui.Create("Bound", nil, { Title = s })
        b:Rebuild()
        b:Rebuild()
        runs = 0
        s:Set("b")
        assert(b.L:GetText() == "b" and b.Seen == "b")
        assert(runs == 1, "effects of earlier builds are gone")
        local n = 0
        for _ in pairs(rawget(b, "__effects")) do n = n + 1 end
        assert(n == 1)
        b:Remove()
        s:Set("c")
        assert(runs == 1)
    "##);
}

#[test]
fn effect_errors_and_cycles_are_reported() {
    run(r##"
        local x = ui.State(0)
        ui.Effect(function() if x:Get() == 1 then error("boom") end end)
        x:Set(1)
        local errs = MOCK.take_errors()
        assert(#errs == 1 and errs[1]:find("boom"), errs[1])
        x:Set(2)
        assert(#MOCK.take_errors() == 0, "the effect keeps working after an error")

        local y = ui.State(0)
        ui.Effect(function() y:Set(y:Get() + 1) end)
        errs = MOCK.take_errors()
        assert(#errs == 1 and errs[1]:find("cycle"), errs[1])
    "##);
}

#[test]
fn app_code_cannot_reach_the_debug_library_or_bytecode() {
    run(r##"
        assert(debug == nil and string.dump == nil and dofile == nil and loadfile == nil)
        assert(load("return 1 + 1")() == 2)
        assert(load("return x", "c", "t", { x = 5 })() == 5)
        assert(not pcall(load("return print", "c", "t", nil)), "an explicit nil env stays nil")
        local f, err = load("\27LuaT\0\25\147\r\n\26\n")
        assert(f == nil and err:find("binary"), err)
        assert(not pcall(load, "return 1", "c", "b"))
        assert(not pcall(load, "return 1", "c", "bt"))
        ErrorNoHalt("still reports")
        assert(#MOCK.take_errors() == 1)
    "##);
}

#[test]
fn sandbox_shares_nothing_with_the_app() {
    run(r##"
        local box = sandbox.New({ name = "plugin" })
        local ok, err = box:Run([[
            assert(http == nil and storage == nil and router == nil and hook == nil and N == nil and R == nil)
            assert(debug == nil and load == nil and require == nil and string.dump == nil and collectgarbage == nil)
            assert(not pcall(ui.Create, "Label"), "no root, no ui")
            string.upper = function() return "pwned" end
            getmetatable("").__index.lower = function() return "pwned" end
            secret = "plugin global"
        ]])
        assert(ok, err)
        assert(("a"):upper() == "A" and ("A"):lower() == "a", "the app's string library is untouched")
        assert(secret == nil)

        ok, err = box:Run("\27Lua")
        assert(not ok and err:find("binary"), err)
        ok, err = box:Run("error('nope')", "user.lua")
        assert(not ok and err:find("user.lua:1: nope"), err)
    "##);
}

#[test]
fn sandbox_limits_cpu_and_memory() {
    run(r##"
        local box = sandbox.New({ instructions = 100000 })
        local ok, err = box:Run("while true do end")
        assert(not ok and err:find("instruction limit"), err)
        ok, err = box:Run("local n = 0 for i = 1, 1000 do n = n + i end x = n")
        assert(ok, "the budget is refilled for the next run: " .. tostring(err))

        local fat = sandbox.New({ instructions = 100000000, memory = 512 * 1024 })
        ok, err = fat:Run("local t = {} for i = 1, 1e7 do t[i] = ('x'):rep(64) .. i end")
        assert(not ok and err:find("memory limit"), err)

        ok, err = sandbox.New({ instructions = 10000000 }):Run("for i = 1, 20000 do print(i) end")
        assert(not ok and err:find("too many host calls"), err)
    "##);
}

#[test]
fn sandbox_api_and_calls_copy_plain_data() {
    run(r##"
        local logged = {}
        local box = sandbox.New({
            api = {
                add = function(a, b) return a + b end,
                user = function() return { name = "Олежек", tags = { "admin" }, fn = print } end,
                hand = function() return print end,
            },
            on_print = function(text) logged[#logged + 1] = text end,
        })
        local ok, err = box:Run([[
            local u = api.user()
            assert(u.name == "Олежек" and u.tags[1] == "admin" and u.fn == nil, "functions do not cross")
            assert(api.hand() == nil)
            assert(api.add(2, 3) == 5)
            local ok, e = pcall(api.add, function() end, 1)
            assert(not ok and e:find("function cannot cross"), e)
            ok, e = pcall(api.missing)
            assert(not ok and e:find("no api function"), e)
            print("hi", 42)
            function double(x) return x * 2, { x = x } end
            function cyclic() local t = {} t.t = t return t end
        ]])
        assert(ok, err)
        assert(logged[1] == "hi\t42")
        local ok2, a, t = box:Call("double", 21)
        assert(ok2 and a == 42 and t.x == 21)
        local ok3, e3 = box:Call("cyclic")
        assert(not ok3 and e3:find("too deeply"), e3)
        local ok4, e4 = box:Call("nope")
        assert(not ok4 and e4:find("no function"), e4)
        local ok5, got = box:Call("double", ui.Create("Panel"))
        assert(not ok5, "a host panel arrives as nil")
    "##);
}

#[test]
fn sandbox_ui_goes_through_checked_proxies() {
    run(r##"
        local root = ui.Create("Panel")
        local errors = {}
        local box = sandbox.New({ root = root, max_panels = 4, on_error = function(m) errors[#errors + 1] = m end })
        assert(MOCK.el(root).style.contain == "layout paint" and MOCK.el(root).style.isolation == "isolate")
        local ok, err = box:Run([[
            local root = ui.Root()
            local label = ui.Create("Label", root, { Text = "0", Dock = TOP })
            local clicks = 0
            btn = ui.Create("Button", root, { Text = "+1", DoClick = function(self)
                clicks = clicks + 1
                label:SetText(tostring(clicks))
            end })
            assert(label:GetText() == "0" and label:GetParent() == root)
            assert(root:GetParent() == nil, "the host's tree above the root is invisible")
            for _, bad in ipairs({
                function() ui.Create("Frame", root) end,
                function() ui.Create("Link", root) end,
                function() label:SetParent(nil) end,
                function() label:SetSlot("x", label) end,
                function() root:SetStyle("contain", "none") end,
                function() root:Remove() end,
                function() root.DoClick = function() end end,
                function() label:__on_click() end,
                function() label:Hook("Think", function() end) end,
            }) do
                local ok = pcall(bad)
                assert(not ok, "bad call " .. _ .. " went through")
            end
            assert(not pcall(label.SetText, {}, "x"), "methods need a proxy")

            local cb = ui.Create("CheckBox", root)
            local seen
            cb.OnChange = function(self, on) seen = on end
            cb:Toggle()
            assert(seen == true, "events fire inside a host call")

            timer.Simple(1, function() label:SetText("later") end)
            local ok, e = pcall(ui.Create, "Panel", root)
            assert(ok, e)
            ok, e = pcall(ui.Create, "Panel", root)
            assert(not ok and e:find("too many panels"), e)
            labelRef = label
        ]])
        assert(ok, err)
        local kids = root:GetChildren()
        local label, btn = kids[1], kids[2]
        assert(label:GetText() == "0" and btn:GetText() == "+1")
        R.event(btn.__id, "click")
        R.event(btn.__id, "click")
        assert(label:GetText() == "2")
        MOCK.advance(1)
        assert(label:GetText() == "later")

        box:Run("btn.DoClick = function() error('handler broke') end")
        R.event(btn.__id, "click")
        assert(#errors == 1 and errors[1]:find("handler broke"), errors[1])

        box:Run("labelRef:Remove()")
        assert(label.__removed and root:ChildCount() == 3)
        box:Close()
        assert(root:ChildCount() == 0 and not root.__removed, "closing removes the plugin's panels, not the root")
        assert(box:IsClosed() and not box:Run("x = 1"))

        local root2 = ui.Create("Panel")
        local box2 = sandbox.New({ root = root2 })
        box2:Run("ui.Create('Label', ui.Root())")
        root2:Remove()
        assert(box2:IsClosed(), "removing the root closes the sandbox")
    "##);
}

#[test]
fn unauthorized_hook_fires_for_this_origin_only() {
    run(r##"
        local got = {}
        hook.Add("Unauthorized", "test", function(url, err) got[#got + 1] = url .. " " .. err.status end)
        http.Start("GET", "/api/me")
        MOCK.respond(1, 401, "login required")
        http.Start("GET", "https://other.example/me")
        MOCK.respond(2, 401, "no")
        http.Start("GET", "/api/forbidden")
        MOCK.respond(3, 403, "no")
        assert(#got == 1 and got[1] == "/api/me 401", got[1])
    "##);
}

#[test]
fn field_errors_from_422_land_in_err_fields() {
    run(r##"
        local r = http.Start("POST", "/api/signup", { name = "" })
        MOCK.json(1, { errors = { name = { "cannot be empty" }, age = { "too young" } } }, 422)
        local v, err = r.value, r.err
        assert(v == nil and err.status == 422)
        assert(err.fields.name[1] == "cannot be empty" and err.fields.age[1] == "too young")
        assert(err.message == "age: too young\nname: cannot be empty", err.message)

        r = http.Start("POST", "/api/signup", {})
        MOCK.json(2, { message = "Try again", errors = { name = { "taken" } } }, 422)
        assert(r.err.message == "Try again" and r.err.fields.name[1] == "taken")
    "##);
}

#[test]
fn form_checks_submits_and_highlights_server_errors() {
    run(r##"
        local form = ui.Create("Form")
        form:SetAction("POST", "/api/signup")
        local name = form:AddField("name", "TextEntry", { Label = "Name", Required = true, PlaceholderText = "Ann" })
        local age = form:AddField("age", "TextEntry", { Label = "Age", Validate = function(v)
            if v ~= "" and not tonumber(v) then return "not a number" end
        end })
        local role = form:AddField("role", "ComboBox")
        role:AddChoice("Player", "player", true)
        role:AddChoice("Admin", "admin")
        local agree = form:AddField("agree", "CheckBox", { Text = "I agree" })
        local submit = form:AddSubmit("Send")
        local done
        function form:OnSuccess(v) done = v end

        assert(MOCK.el(name).control.attrs.placeholder == "Ann")
        submit:__on_click()
        assert(#MOCK.fetches == 0, "Required stops the request")
        assert(name:GetError() == "Required" and MOCK.el(name).classes["ngs-invalid"])
        assert(MOCK.el(name).control.attrs["aria-invalid"] == "true")

        name:SetValue("Ann")
        name:__on_input()
        assert(name:GetError() == nil and not MOCK.el(name).classes["ngs-invalid"], "typing clears the error")
        age:SetValue("x")
        submit:__on_click()
        assert(age:GetError() == "not a number" and #MOCK.fetches == 0)

        age:SetValue("")
        role:ChooseOptionID(2)
        agree:SetChecked(true)
        submit:__on_click()
        assert(#MOCK.fetches == 1 and not submit:IsEnabled(), "the button waits for the answer")
        local body = MOCK.fetches[1].body
        assert(body:find('"role":"admin"') and body:find('"agree":true') and body:find('"name":"Ann"'), body)

        MOCK.json(1, { message = "Fix it", errors = { name = { "taken" }, coupon = { "expired" } } }, 422)
        assert(submit:IsEnabled())
        assert(name:GetError() == "taken")
        assert(form.Message:IsVisible() and form.Message:GetText() == "Fix it\ncoupon: expired", form.Message:GetText())

        submit:__on_click()
        MOCK.json(2, { id = 1 })
        assert(done.id == 1 and name:GetError() == nil and not form.Message:IsVisible())

        name:SetValue("Bob")
        name:__on_key("Enter")
        assert(#MOCK.fetches == 3, "Enter submits")
        MOCK.respond(3, 500, "server broke")
        assert(form.Message:GetText() == "server broke")
    "##);
}

#[test]
fn apply_errors_targets_named_panels() {
    run(r##"
        local root = ui.Create("Panel")
        local a = ui.Create("TextEntry", root)
        a:SetName("email")
        local rest = ui.ApplyErrors(root, { email = { "bad" }, other = "x" })
        assert(a:GetError() == "bad" and rest.other == "x" and rest.email == nil)
        ui.ApplyErrors(root, {})
        assert(a:GetError() == nil)
    "##);
}
