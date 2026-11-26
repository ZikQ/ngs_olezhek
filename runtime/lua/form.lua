---@diagnostic disable: duplicate-set-field, inject-field
local N, R = ...

local Panel = ui.GetClass("Panel")

--- Marks the panel invalid (red border, aria-invalid) and shows `message` under it when the panel
--- was added with Form:AddField. nil or false clears it.
function Panel:SetError(message)
    if message == false then message = nil end
    if message ~= nil then message = tostring(message) end
    if rawget(self, "__error") == message then return end
    rawset(self, "__error", message)
    N.class(self.__id, "ngs-invalid", message ~= nil)
    self:SetAttr("aria-invalid", message ~= nil and "true" or nil)
    local label = rawget(self, "__error_label")
    if label and IsValid(label) then
        label:SetText(message or "")
        label:SetVisible(message ~= nil)
    end
    if self.OnErrorChanged then R.pcall(self.OnErrorChanged, self, message) end
end

function Panel:GetError() return rawget(self, "__error") end

local function walk(p, fn)
    for _, c in ipairs(p.__children) do
        if not c.__removed then
            fn(c)
            walk(c, fn)
        end
    end
end

--- Puts server field errors (err.fields from a 422, or any { name = "msg" | {"msg", ...} }) on the
--- panels under `root` whose SetName matches. Named panels without an error are cleared.
--- Returns the errors no panel took, as { name = "msg" }.
function ui.ApplyErrors(root, fields)
    fields = fields or {}
    local taken = {}
    walk(root, function(p)
        local name = rawget(p, "__name")
        if name == nil then return end
        local e = fields[name]
        if type(e) == "table" then e = e[1] end
        if e ~= nil or rawget(p, "__error") ~= nil then p:SetError(e) end
        if e ~= nil then taken[name] = true end
    end)
    local rest = {}
    for name, e in pairs(fields) do
        if not taken[name] then rest[name] = type(e) == "table" and e[1] or e end
    end
    return rest
end

-- Editing a field clears its error: the message described the old value.
local function clears_error(class, method)
    local cls = ui.GetClass(class)
    local original = cls[method]
    cls[method] = function(self, ...)
        if rawget(self, "__error") ~= nil then self:SetError(nil) end
        return original(self, ...)
    end
end
clears_error("TextEntry", "__on_input")
clears_error("CheckBox", "__on_change")
clears_error("ComboBox", "__on_change")
clears_error("Slider", "__on_input")
clears_error("NumberWang", "__on_input")

local TextEntry = ui.GetClass("TextEntry")
local entry_key = TextEntry.__on_key
function TextEntry:__on_key(key)
    local form = rawget(self, "__form")
    if key == "Enter" and form and not self.__multiline and IsValid(form) then
        R.spawn_for(form, form.Submit, form)
    end
    return entry_key(self, key)
end

local function get_value(p)
    if p.GetFormValue then return p:GetFormValue() end
    if p.GetChecked then return p:GetChecked() end
    if p.GetSelected then
        local text, data = p:GetSelected()
        if data ~= nil then return data end
        return text
    end
    if p.GetValue then
        local v = p:GetValue()
        if p.__type == "number" then return tonumber(v) end
        return v
    end
end

local function set_value(p, v)
    if p.SetFormValue then return p:SetFormValue(v) end
    if p.SetChecked then return p:SetChecked(v == true) end
    if p.GetSelected and p.GetOptionData then
        for i = 1, #p.__choices do
            if p:GetOptionData(i) == v or p:GetOptionText(i) == v then return p:ChooseOptionID(i) end
        end
        return
    end
    if p.SetValue then p:SetValue(v) end
end

local function blank(v)
    return v == nil or v == false or (type(v) == "string" and v:match("^%s*$") ~= nil)
end

local FIELD_OPTS = { Label = true, Required = true, RequiredMessage = true, Validate = true, Value = true, Hint = true }

local Form = {}

--- What Required fields say when empty. Set it once in autorun for your language.
Form.RequiredMessage = "Required"

function Form:Initialize()
    self:SetLayout(LAYOUT_COLUMN)
    self:SetGap(12)
    self.__fields = {}
    self.Message = ui.Create("Label", self)
    N.class(self.Message.__id, "ngs-form-message", true)
    self.Message:SetAttr("role", "alert")
    self.Message:Hide()
end

--- A labelled input whose value goes into the request under `name`. opts: Label, Hint,
--- Required, Value, Validate = function(value, values) return "message" or nil end, plus any
--- props for ui.Create (PlaceholderText = "..." calls SetPlaceholderText, and so on). Returns the input.
function Form:AddField(name, class, opts)
    opts = opts or {}
    local box = ui.Create("Panel", self)
    box:SetLayout(LAYOUT_COLUMN)
    box:SetGap(4)
    N.class(box.__id, "ngs-field", true)
    if opts.Label then
        local label = ui.Create("Label", box)
        label:SetText(opts.Label)
        N.class(label.__id, "ngs-field-label", true)
    end
    local props = {}
    for k, v in pairs(opts) do
        if not FIELD_OPTS[k] then props[k] = v end
    end
    local input = ui.Create(class or "TextEntry", box, props)
    input:SetName(name)
    if opts.Label then input:SetAttr("aria-label", opts.Label) end
    if opts.Required then input:SetAttr("aria-required", "true") end
    if opts.Hint then
        local hint = ui.Create("Label", box)
        hint:SetText(opts.Hint)
        N.class(hint.__id, "ngs-field-hint", true)
    end
    local err = ui.Create("Label", box)
    N.class(err.__id, "ngs-field-error", true)
    err:Hide()
    rawset(input, "__error_label", err)
    rawset(input, "__form", self)
    if opts.Value ~= nil then set_value(input, opts.Value) end
    self.__fields[#self.__fields + 1] = { name = name, input = input, opts = opts }
    return input
end

--- The button that submits the form. Enter in a single-line field submits too.
function Form:AddSubmit(text)
    local button = ui.Create("Button", self)
    button:SetText(text or "Submit")
    N.class(button.__id, "ngs-form-submit", true)
    local form = self
    button.DoClick = function() form:Submit() end
    self.SubmitButton = button
    return button
end

--- Where Submit sends the values: form:SetAction("POST", "/api/users"). opts as for http.Request.
function Form:SetAction(method, url, opts)
    self.__method, self.__url, self.__opts = method, url, opts
end

function Form:GetField(name)
    for _, f in ipairs(self.__fields) do
        if f.name == name then return f.input end
    end
end

--- { name = value } for every field. Nested names ("address.city") stay flat keys.
function Form:GetValues()
    local out = {}
    for _, f in ipairs(self.__fields) do
        if IsValid(f.input) then out[f.name] = get_value(f.input) end
    end
    return out
end

function Form:SetValues(values)
    for _, f in ipairs(self.__fields) do
        if values[f.name] ~= nil and IsValid(f.input) then set_value(f.input, values[f.name]) end
    end
end

--- Shows errors: fields = { name = "message" or {"message", ...} }, message for the whole form.
--- Errors for names the form has no field for are added to the message.
function Form:SetErrors(fields, message)
    local rest = ui.ApplyErrors(self, fields)
    local lines = { message }
    local names = {}
    for name in pairs(rest) do names[#names + 1] = name end
    table.sort(names)
    for _, name in ipairs(names) do lines[#lines + 1] = name .. ": " .. tostring(rest[name]) end
    local text = table.concat(lines, "\n")
    self.Message:SetText(text)
    self.Message:SetVisible(text ~= "")
end

function Form:ClearErrors() self:SetErrors({}) end

--- Client-side checks: Required and Validate from AddField, then self:Validate(values) if defined.
--- Returns true when the values pass; otherwise shows the errors and returns false.
function Form:Check(values)
    values = values or self:GetValues()
    local errors, any = {}, false
    for _, f in ipairs(self.__fields) do
        local v = values[f.name]
        local e
        if f.opts.Required and blank(v) then
            e = f.opts.RequiredMessage or self.RequiredMessage
        elseif f.opts.Validate then
            e = f.opts.Validate(v, values)
        end
        if e then errors[f.name], any = e, true end
    end
    if self.Validate then
        local extra = self:Validate(values)
        if type(extra) == "table" then
            for k, v in pairs(extra) do errors[k], any = v, true end
        elseif type(extra) == "string" then
            self:SetErrors(errors, extra)
            return false
        end
    end
    self:SetErrors(errors)
    return not any
end

--- Checks the values, sends them with SetAction's method and URL and shows a 422's field errors.
--- Calls self:OnSuccess(value) or self:OnFailure(err); without OnFailure the error message is shown
--- above the fields. Waits for the answer, so call it from a handler (DoClick does). Returns
--- value, err like http.Post.
function Form:Submit()
    if self.__submitting then return nil, { status = 0, message = "already submitting" } end
    local values = self:GetValues()
    if not self:Check(values) then return nil, { status = 0, message = "invalid", fields = {} } end
    local body = values
    if self.OnBeforeSubmit then
        body = self:OnBeforeSubmit(values)
        if body == false then return nil, { status = 0, message = "cancelled", cancelled = true } end
        body = body or values
    end
    if not self.__url then error("Form:Submit: call SetAction first", 2) end
    self.__submitting = true
    local button = self.SubmitButton
    if IsValid(button) then button:SetEnabled(false) end
    local value, err = http.Request(self.__method or "POST", self.__url, body, self.__opts)
    self.__submitting = false
    if IsValid(button) then button:SetEnabled(true) end
    if self.__removed then return value, err end
    if err then
        if err.fields then
            self:SetErrors(err.fields, err.body and type(err.body) == "table" and err.body.message or nil)
        elseif not self.OnFailure then
            self:SetErrors({}, err.message)
        end
        if self.OnFailure then self:OnFailure(err) end
        return nil, err
    end
    self:ClearErrors()
    if self.OnSuccess then self:OnSuccess(value) end
    return value, err
end

ui.Register("Form", Form, "Panel")
