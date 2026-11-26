//! Runs the browser runtime's Lua prelude natively against a mocked `N` (tests/prelude/mock.lua).
//! Covers the Lua side only; the natives themselves need a browser.

use omnilua::{Lua, Table};

const PRELUDE: &[(&str, &str)] = &[
    ("base", include_str!("../runtime/lua/base.lua")),
    ("ui", include_str!("../runtime/lua/ui.lua")),
    ("style", include_str!("../runtime/lua/style.lua")),
    ("attrs", include_str!("../runtime/lua/attrs.lua")),
    ("controls", include_str!("../runtime/lua/controls.lua")),
    ("surface", include_str!("../runtime/lua/surface.lua")),
    ("http", include_str!("../runtime/lua/http.lua")),
    ("storage", include_str!("../runtime/lua/storage.lua")),
    ("router", include_str!("../runtime/lua/router.lua")),
];

fn runtime() -> Lua {
    let lua = Lua::new();
    let (n, m): (Table, Table) = lua.load(include_str!("prelude/mock.lua")).set_name("@mock.lua").eval().unwrap();
    let r = lua.create_table().unwrap();
    for (name, src) in PRELUDE {
        lua.load(*src)
            .set_name(format!("@ngs/{name}.lua"))
            .into_function()
            .and_then(|f| f.call::<_, ()>((n.clone(), r.clone())))
            .unwrap_or_else(|e| panic!("prelude {name}: {e}"));
    }
    m.set("R", r.clone()).unwrap();
    lua.globals().set("MOCK", m).unwrap();
    lua.globals().set("R", r).unwrap();
    lua.load("R.boot({}, true)").exec().unwrap();
    lua
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
