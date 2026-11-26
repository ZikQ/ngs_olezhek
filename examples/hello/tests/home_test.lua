local function save(t) t.Click({ Class = "Button", Text = "Сохранить" }) end

test("empty name is caught before the request", function(t)
    t.Serve("GET /api/search", {})
    t.Go("/")
    save(t)
    t.Eq(t.Get("#name"):GetError(), "Обязательное поле")
    t.Eq(#t.Requests("POST /api/users/1/rename"), 0)
end)

test("server field errors are shown next to the field", function(t)
    t.Serve("GET /api/search", {})
    t.Go("/")
    local name = t.Get("#name")
    t.Type(name, "admin")
    save(t)
    t.Eq(t.Request("POST /api/users/1/rename").json, { name = "admin", role = "player", remember = false })
    t.Respond("POST /api/users/1/rename", 422, { errors = { name = { "это имя зарезервировано" } } })
    t.Eq(name:GetError(), "это имя зарезервировано")
    assert(t.HasText("это имя зарезервировано"), t.Text())

    t.Type(name, "Гордон")
    t.Eq(name:GetError(), nil, "editing clears the error")
end)

test("successful rename reports the new name and role", function(t)
    t.Serve("GET /api/search", {})
    t.Serve("POST /api/users/{id}/rename", function(req, params)
        return { id = tonumber(params.id), name = req.json.name, role = req.json.role }
    end)
    t.Go("/")
    t.Type("#name", "Гордон")
    t.Select("#role", "Админ")
    save(t)
    assert(t.HasText("Сохранено: Гордон, роль admin"), t.Text())
end)

test("search results link to profiles", function(t)
    t.Serve("GET /api/search", function(req)
        if req.query.q == "" then return {} end
        return { { id = 2, name = "Гарри", role = "player" } }
    end)
    t.Serve("GET /api/users/{id}", function(_, params)
        return { id = tonumber(params.id), name = "Гарри", role = "player" }
    end)
    t.Go("/")
    t.Eq(t.Find({ Class = "Link", Contains = "Гарри" }), nil)
    t.Type("#search", "Гар")
    t.Click({ Class = "Link", Text = "Гарри  (player)" })
    t.Eq(t.Path(), "/profile/2")
    t.Eq(t.Page().ClassName, "ProfilePage")
    assert(t.HasText("Роль"), t.Text())
end)
