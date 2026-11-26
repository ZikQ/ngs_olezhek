---@meta ngs_olezhek
-- LuaLS definitions for the ngs_olezhek runtime. Generated into <lua>/.ngs/ by the dev server.

---@type boolean
DEV = false

---@class Color
---@field r number
---@field g number
---@field b number
---@field a number
local Color_ = {}
---@return Color
function Color_:Copy() end
---@return number r, number g, number b, number a
function Color_:Unpack() end

---@param r? number
---@param g? number
---@param b? number
---@param a? number
---@return Color
function Color(r, g, b, a) end

---@param v any
---@return boolean
function IsColor(v) end

---@type Color
color_white = nil
---@type Color
color_black = nil
---@type Color
color_transparent = nil

---@param ... any
function ErrorNoHalt(...) end
---@param t table
function PrintTable(t) end
---Seconds since the page loaded.
---@return number
function CurTime() end
---@return number
function RealTime() end
---@return number
function SysTime() end
---Seconds since the previous frame (only meaningful inside Think).
---@return number
function FrameTime() end
---@return number
function ScrW() end
---@return number
function ScrH() end
---@param t number
---@param from number
---@param to number
---@return number
function Lerp(t, from, to) end
---@param v any
---@return boolean
function IsValid(v) end
---Runs a file from the lua folder once and returns its results (cached).
---@param path string e.g. "shared/config.lua"
---@return any ...
function include(path) end

---@param v number
---@param lo number
---@param hi number
---@return number
function math.Clamp(v, lo, hi) end
---@param v number
---@param decimals? integer
---@return number
function math.Round(v, decimals) end
---@return number
function math.Remap(v, inMin, inMax, outMin, outMax) end

---@param s string
---@return string
function string.Trim(s) end
---@return boolean
function string.StartsWith(s, prefix) end
---@return boolean
function string.EndsWith(s, suffix) end
---@param s string
---@param sep string plain text, not a pattern
---@return string[]
function string.Split(s, sep) end

---@return integer
function table.Count(t) end
---@return boolean
function table.HasValue(t, value) end
---@return boolean
function table.IsEmpty(t) end
---Deep copy.
---@generic T: table
---@param t T
---@return T
function table.Copy(t) end

DOCK_NONE = 0
DOCK_FILL = 1
DOCK_LEFT = 2
DOCK_RIGHT = 3
DOCK_TOP = 4
DOCK_BOTTOM = 5

---@alias Dock 0|1|2|3|4|5

LAYOUT_COLUMN = "column"
LAYOUT_ROW = "row"
LAYOUT_GRID = "grid"

---@alias Layout "column"|"row"|"grid"

ALIGN_START = "flex-start"
ALIGN_CENTER = "center"
ALIGN_END = "flex-end"
ALIGN_STRETCH = "stretch"

JUSTIFY_START = "flex-start"
JUSTIFY_CENTER = "center"
JUSTIFY_END = "flex-end"
JUSTIFY_BETWEEN = "space-between"
JUSTIFY_AROUND = "space-around"
JUSTIFY_EVENLY = "space-evenly"

MOUSE_LEFT = 1
MOUSE_RIGHT = 2
MOUSE_MIDDLE = 3

TEXT_ALIGN_LEFT = 0
TEXT_ALIGN_CENTER = 1
TEXT_ALIGN_RIGHT = 2
TEXT_ALIGN_TOP = 3
TEXT_ALIGN_BOTTOM = 4

---Pixels as a number, or a CSS length such as "50%" or "fit-content".
---@alias Length number|string

---@alias StyleValue number|string|Color|false

---@class Shadow
---@field x? number
---@field y? number
---@field blur? number
---@field spread? number
---@field color? Color
---@field inset? boolean

---Colors (or { Color, percent } stops) in the array part.
---@class Gradient
---@field angle? number
---@field radial? boolean

---@class Transform
---@field x? Length
---@field y? Length
---@field rotate? number degrees
---@field scale? number
---@field scaleX? number
---@field scaleY? number
---@field skewX? number
---@field skewY? number

---Any allowed CSS property as a key (kebab or camelCase), plus the shortcuts below.
---@class StyleTable
---@field background? Color|string
---@field textColor? Color
---@field color? Color|string
---@field alpha? number 0..255
---@field border? number|Color|false|{ [1]: number, [2]: Color?, [3]: string? }
---@field radius? Length|Length[]
---@field shadow? Shadow|Shadow[]|false
---@field gradient? Gradient|false
---@field transform? Transform|false
---@field overflow? string|string[]
---@field font? string font name from surface.CreateFont
---@field padding? Length|Length[] left, top, right, bottom
---@field margin? Length|Length[] left, top, right, bottom
---@field opacity? number
---@field cursor? string
---@field [string] StyleValue

util = {}
---@param t table
---@param pretty? boolean
---@return string
function util.TableToJSON(t, pretty) end
---@param s string
---@return table|nil
function util.JSONToTable(s) end

hook = {}
---Built-in events: Think, OnPageChanged(path, page), OnScreenSizeChanged(w, h), OnFontLoaded(family), OnReload(file),
---OnStorageChanged(key, value) (another tab changed storage). Hooks added here live forever: inside a panel use Panel:Hook.
---@param event string
---@param id any
---@param fn function
function hook.Add(event, id, fn) end
---@param event string
---@param id any
function hook.Remove(event, id) end
---First hook returning a non-nil value stops the chain and that value is returned.
---@param event string
---@return any ...
function hook.Run(event, ...) end
---@return table<string, table<any, function>>
function hook.GetTable() end

timer = {}
---@param delay number seconds
---@param fn function
function timer.Simple(delay, fn) end
---@param name string
---@param delay number seconds
---@param reps integer 0 = forever
---@param fn function
function timer.Create(name, delay, reps, fn) end
function timer.Remove(name) end
---@return boolean
function timer.Exists(name) end
---@param name string
---@param delay number
function timer.Adjust(name, delay) end

gui = {}
---Cursor position in page coordinates.
---@return number x, number y
function gui.MousePos() end
---@return number
function gui.MouseX() end
---@return number
function gui.MouseY() end
---New tab with noopener. Only http, https, mailto and relative URLs; returns false otherwise.
---@param url string
---@return boolean
function gui.OpenURL(url) end

---@class FontData
---@field font? string family name
---@field size? number px, default 14
---@field weight? number default 400
---@field italic? boolean
---@field url? string font file to load, e.g. "/fonts/Inter.woff2"

surface = {}
---Built-in fonts: Default, DefaultBold, Small, Large, Title.
---@param name string
---@param data FontData
function surface.CreateFont(name, data) end
---@param family string
---@param url string
---@param data? { weight?: number, italic?: boolean }
function surface.LoadFont(family, url, data) end
---True once the font file is downloaded; OnFontLoaded fires only once, so check this when a panel is created.
---@param family string
---@return boolean
function surface.IsFontLoaded(family) end
---@overload fun(color: Color)
---@param r number
---@param g number
---@param b number
---@param a? number
function surface.SetDrawColor(r, g, b, a) end
---@overload fun(color: Color)
---@param r number
---@param g number
---@param b number
---@param a? number
function surface.SetTextColor(r, g, b, a) end
---@param name string
function surface.SetFont(name) end
---@return string
function surface.GetFont() end
---@param text string
---@return number w, number h
function surface.GetTextSize(text) end
function surface.SetTextPos(x, y) end
---@return number x, number y
function surface.GetTextPos() end
---Draws at the text position and advances it by the text width.
---@param text string
function surface.DrawText(text) end
function surface.DrawRect(x, y, w, h) end
---@param thickness? number
function surface.DrawOutlinedRect(x, y, w, h, thickness) end
function surface.DrawLine(x1, y1, x2, y2) end
---Outline only; use draw.Circle for a filled one.
---@overload fun(x: number, y: number, radius: number, color?: Color)
function surface.DrawCircle(x, y, radius, r, g, b, a) end
---@param verts { x: number, y: number }[]
function surface.DrawPoly(verts) end
---@param mat IMaterial|nil
function surface.SetMaterial(mat) end
function surface.DrawTexturedRect(x, y, w, h) end
function surface.DrawTexturedRectUV(x, y, w, h, u0, v0, u1, v1) end
---@param a number 0..1
function surface.SetAlphaMultiplier(a) end

---@class IMaterial
local IMaterial = {}
---@return integer
function IMaterial:Width() end
---@return integer
function IMaterial:Height() end
---@return string
function IMaterial:GetName() end

---Image by URL; panels that draw it before it loads are repainted when it arrives.
---@param path string
---@return IMaterial
function Material(path) end

draw = {}
---@param radius number
---@param color Color
function draw.RoundedBox(radius, x, y, w, h, color) end
---@param color Color
---@param tl? boolean
---@param tr? boolean
---@param bl? boolean
---@param br? boolean
function draw.RoundedBoxEx(radius, x, y, w, h, color, tl, tr, bl, br) end
---Filled circle.
---@param color Color
function draw.Circle(x, y, radius, color) end
---@param text any
---@param font? string
---@param x? number
---@param y? number
---@param color? Color
---@param xalign? integer TEXT_ALIGN_*
---@param yalign? integer TEXT_ALIGN_*
---@return number w, number h
function draw.SimpleText(text, font, x, y, color, xalign, yalign) end
---Multi-line text.
---@return number w, number h
function draw.DrawText(text, font, x, y, color, xalign) end
function draw.NoTexture() end
---@param font string
---@return number
function draw.GetFontHeight(font) end

---@class HttpError
---@field status integer 0 for network errors, timeouts and cancellation
---@field message string
---@field body any decoded JSON or text
---@field timeout? boolean
---@field cancelled? boolean

---@class HttpOptions
---@field headers? table<string, string|false> false removes a default header
---@field timeout? number seconds, 0 = none; default from http.SetTimeout (30)
---@field invalidate? string|string[] http.Invalidate these prefixes after a successful response

---@class QueryOptions: HttpOptions
---@field ttl? number seconds the answer counts as fresh (at least 2)
---@field key? string cache key, the URL by default

---@class HttpRequest
local HttpRequest = {}
---Waits for the result (only inside handlers and Initialize).
---@return any value, HttpError? err
function HttpRequest:Wait() end
function HttpRequest:Cancel() end
---@return boolean
function HttpRequest:IsDone() end

http = {}
---Waits for the response (only inside handlers and Initialize). JSON comes back as a table.
---Cancelled when the panel running the handler is removed.
---@param method string
---@param url string
---@param body? table|string tables are sent as JSON
---@param opts? HttpOptions
---@return any value, HttpError? err
function http.Request(method, url, body, opts) end
---@param url string
---@param opts? HttpOptions
---@return any value, HttpError? err
function http.Get(url, opts) end
---@param opts? HttpOptions
---@return any value, HttpError? err
function http.Post(url, body, opts) end
---@param opts? HttpOptions
---@return any value, HttpError? err
function http.Put(url, body, opts) end
---@param opts? HttpOptions
---@return any value, HttpError? err
function http.Patch(url, body, opts) end
---@param opts? HttpOptions
---@return any value, HttpError? err
function http.Delete(url, body, opts) end
---Starts a request without waiting; not tied to any panel.
---@param method string
---@param url string
---@param body? table|string
---@param opts? HttpOptions
---@return HttpRequest
function http.Start(method, url, body, opts) end
---Callback style, usable anywhere.
---@param url string
---@param onSuccess? fun(value: any)
---@param onFailure? fun(message: string, err: HttpError)
---@param method? string
---@param body? table|string
---@param opts? HttpOptions
---@return HttpRequest
function http.Fetch(url, onSuccess, onFailure, method, body, opts) end
---Default header for requests to this origin (never sent elsewhere); nil removes it.
---@param name string
---@param value? string
function http.SetHeader(name, value) end
---Default timeout in seconds; 0 or nil disables it.
---@param seconds? number
function http.SetTimeout(seconds) end
---Cached GET with request sharing: fresh answers come from the cache, stale ones are returned
---at once and refreshed in the background. The table is shared between callers.
---@param url string
---@param opts? QueryOptions
---@return any value, HttpError? err
function http.Query(url, opts) end
---@param key string
---@return any
function http.GetData(key) end
---Optimistic update: replaces the cached value and notifies subscribed panels.
---@param key string
---@param value any
function http.SetData(key, value) end
---"/api/users" matches "/api/users", "/api/users/1", "/api/users?q=a". No argument = everything.
---Subscribed queries are fetched again, the rest dropped.
---@param prefix? string|string[]
function http.Invalidate(prefix) end
---Percent-encodes a query value.
---@param s any
---@return string
function http.Encode(s) end

---@class Storage
local Storage = {}
---@param key string
---@param default? any
---@return any
function Storage.Get(key, default) end
---JSON-encoded; nil removes. Returns false if the browser refused (the value is kept in memory).
---@param key string
---@param value any
---@return boolean
function Storage.Set(key, value) end
---@param key string
function Storage.Remove(key) end
---@return string[]
function Storage.Keys() end
---Removes this app's keys only.
function Storage.Clear() end

---@class StorageModule: Storage
---@field Session Storage sessionStorage, cleared when the tab closes

---localStorage under the "ngs:" prefix. Any script on the page can read it, including injected
---ones, so an HttpOnly cookie is the safer place for a session token.
---@type StorageModule
storage = nil

router = {}
---@param url string
function router.Go(url) end
---@param url string
function router.Replace(url) end
function router.Back() end
---Opens in a new tab (noopener). Same scheme check as gui.OpenURL.
---@param url string
---@return boolean
function router.Open(url) end
---Rebuilds the current page.
function router.Reload() end
---@return string
function router.GetPath() end
---@return Page
function router.GetPage() end

---@class Panel
---@field ClassName string
---@field Base? string
---@field BaseClass? table
---@field Hovered boolean
---@field Depressed boolean
---@field Initialize? fun(self: Panel)
---@field Paint? fun(self: Panel, w: number, h: number)
---@field PerformLayout? fun(self: Panel, w: number, h: number)
---@field Think? fun(self: Panel)
---@field OnRemove? fun(self: Panel)
---@field DoClick? fun(self: Panel)
---@field DoRightClick? fun(self: Panel)
---@field DoDoubleClick? fun(self: Panel)
---@field OnMousePressed? fun(self: Panel, code: integer)
---@field OnMouseReleased? fun(self: Panel, code: integer)
---@field OnCursorEntered? fun(self: Panel)
---@field OnCursorExited? fun(self: Panel)
---@field OnCursorMoved? fun(self: Panel, x: number, y: number)
---@field OnFocusChanged? fun(self: Panel, gained: boolean)
---@field OnKeyCodePressed? fun(self: Panel, key: string): boolean? return true to cancel the browser default
---@field OnQueryUpdate? fun(self: Panel, key: string, value: any) data from Panel:UseQuery changed; without it the panel is rebuilt
---@field OnContextChanged? fun(self: Panel, key: any, value: any) a value this panel injected was provided again
---@field Props table props passed to ui.Create
---@field Tag? string native tag for a custom control class, e.g. "video"
local Panel = {}
---@return boolean
function Panel:IsValid() end
---@return string
function Panel:GetClassName() end
function Panel:SetName(name) end
---@return string
function Panel:GetName() end
---@generic T: Panel
---@param class `T`
---@param props? table
---@return T
function Panel:Add(class, props) end
---@param parent? Panel
---@param slot? string|false a named slot of the parent, false to skip its slots
function Panel:SetParent(parent, slot) end
---Children added later go into `panel`. Without a name it is the default slot.
---@overload fun(self: Panel, panel: Panel)
---@param name string
---@param panel? Panel
function Panel:SetSlot(name, panel) end
---@param name? string
---@return Panel
function Panel:GetSlot(name) end
---@param name any
---@param fn fun(self: Panel)
function Panel:CallOnRemove(name, fn) end
---@param name any
function Panel:RemoveCallOnRemove(name) end
---hook.Add removed together with the panel. A string calls that method of the panel.
---@overload fun(self: Panel, event: string, fn: function|string)
---@param event string
---@param name any
---@param fn function|string
function Panel:Hook(event, name, fn) end
---@param event string
---@param name? any
function Panel:Unhook(event, name) end
---timer.Create removed together with the panel.
---@param name any
---@param delay number seconds
---@param reps? integer 0 = forever, default 1
---@param fn function|string
function Panel:Timer(name, delay, reps, fn) end
---@param delay number seconds
---@param fn function|string
function Panel:TimerSimple(delay, fn) end
---@param name any
function Panel:RemoveTimer(name) end
---@return boolean
function Panel:TimerExists(name) end
---Visible to Inject in all descendants.
---@param key any
---@param value any
function Panel:Provide(key, value) end
---@param key any
---@param default? any
---@return any
function Panel:Inject(key, default) end
---Removes the children and runs Initialize again; hooks and timers of the panel are dropped first.
function Panel:Rebuild() end
---http.Query that subscribes the panel to later changes (OnQueryUpdate, or Rebuild).
---@param url string
---@param opts? QueryOptions
---@return any value, HttpError? err
function Panel:UseQuery(url, opts) end
---Attribute on the native element (the control of a TextEntry/ComboBox). Allowed: aria-*, data-*,
---role, title, alt, placeholder, autocomplete, inputmode, tabindex, min/max/step, src/href (scheme-checked)...
---Event handlers, style and similar are rejected. true = empty attribute, false/nil removes it.
---@param name string
---@param value? string|number|boolean
function Panel:SetAttr(name, value) end
---@param name string
---@return string?
function Panel:GetAttr(name) end
---DOM property from an allow-list (value, checked, muted, currentTime, scrollTop...).
---@param name string
---@param value? string|number|boolean
function Panel:SetProp(name, value) end
---@param name string
---@return string|number|boolean|nil
function Panel:GetProp(name) end
---@param method "play"|"pause"|"load"|"select"|"showPicker"|"stepUp"|"stepDown"|"reportValidity"|"requestFullscreen"
function Panel:Invoke(method) end
---One CSS property (kebab or camelCase) or a table of them. Numbers are px, Colors become
---rgba; url() and other fetching functions are rejected. nil clears.
---@overload fun(self: Panel, styles: StyleTable)
---@param prop string
---@param value? StyleValue
function Panel:SetStyle(prop, value) end
---@overload fun(self: Panel, border: false|string|nil)
---@param width number
---@param color? Color
---@param style? "solid"|"dashed"|"dotted"|"double"
function Panel:SetBorder(width, color, style) end
---One radius, or top-left, top-right, bottom-right, bottom-left.
---@param r Length|nil
---@param tr? Length
---@param br? Length
---@param bl? Length
function Panel:SetRadius(r, tr, br, bl) end
---@overload fun(self: Panel, shadow: Shadow|Shadow[]|false|nil)
---@param x number
---@param y number
---@param blur? number
---@param color? Color
---@param spread? number
---@param inset? boolean
function Panel:SetShadow(x, y, blur, color, spread, inset) end
---@overload fun(self: Panel, gradient: Gradient|false|nil)
---@param from Color
---@param to Color
---@param angle? number degrees, 180 = top to bottom
function Panel:SetGradient(from, to, angle) end
---@param t Transform|false|nil
function Panel:SetTransform(t) end
---@param x "visible"|"hidden"|"scroll"|"auto"|"clip"
---@param y? "visible"|"hidden"|"scroll"|"auto"|"clip"
function Panel:SetOverflow(x, y) end
---Animates every style change, state styles included. nil turns it off.
---@param duration? number seconds
---@param ease? string "ease", "linear", "ease-in-out", "cubic-bezier(...)"
function Panel:SetTransition(duration, ease) end
---Compiled into a CSS class: applied by the browser, no Lua runs on hover. nil removes it.
---@param state "hover"|"active"|"focus"|"disabled"
---@param styles? StyleTable
function Panel:SetStyleState(state, styles) end
---@param styles? StyleTable
function Panel:SetStyleHover(styles) end
---While pressed.
---@param styles? StyleTable
function Panel:SetStyleActive(styles) end
---While the panel or anything inside it has focus.
---@param styles? StyleTable
function Panel:SetStyleFocus(styles) end
---@param styles? StyleTable
function Panel:SetStyleDisabled(styles) end
---@return Panel?
function Panel:GetParent() end
---@return Panel[]
function Panel:GetChildren() end
---@param i integer
---@return Panel?
function Panel:GetChild(i) end
---@return integer
function Panel:ChildCount() end
function Panel:Clear() end
function Panel:Remove() end
---@param dock Dock
function Panel:Dock(dock) end
---@return Dock
function Panel:GetDock() end
function Panel:DockMargin(left, top, right, bottom) end
function Panel:DockPadding(left, top, right, bottom) end
---@param w? Length
---@param h? Length
function Panel:SetSize(w, h) end
---@param w Length
function Panel:SetWide(w) end
---@param h Length
function Panel:SetTall(h) end
---@param w Length
function Panel:SetWidth(w) end
---@param h Length
function Panel:SetHeight(h) end
---@param w? Length
---@param h? Length
function Panel:SetMinSize(w, h) end
---@param w? Length
---@param h? Length
function Panel:SetMaxSize(w, h) end
function Panel:SizeToContents() end
function Panel:SizeToContentsX() end
function Panel:SizeToContentsY() end
---@return integer w, integer h
function Panel:GetSize() end
---@return integer
function Panel:GetWide() end
---@return integer
function Panel:GetTall() end
---@return integer
function Panel:GetWidth() end
---@return integer
function Panel:GetHeight() end
---Absolute position inside the parent; ignored while docked.
---@param x Length
---@param y Length
function Panel:SetPos(x, y) end
function Panel:SetX(x) end
function Panel:SetY(y) end
---@return integer x, integer y
function Panel:GetPos() end
---@return integer
function Panel:GetX() end
---@return integer
function Panel:GetY() end
---@return number x, number y, number w, number h
function Panel:GetScreenBounds() end
---@return number x, number y
function Panel:LocalToScreen(x, y) end
---@return number x, number y
function Panel:ScreenToLocal(x, y) end
---@return integer x, integer y
function Panel:LocalCursorPos() end
---@return integer x, integer y
function Panel:CursorPos() end
---While captured the panel keeps receiving OnCursorMoved/OnMouseReleased outside its bounds.
---@param capture boolean
function Panel:MouseCapture(capture) end
function Panel:Center() end
---How children without Dock are arranged.
---@param mode Layout
function Panel:SetLayout(mode) end
---@param gap number
---@param columnGap? number
function Panel:SetGap(gap, columnGap) end
---Cross axis: ALIGN_*.
function Panel:SetAlign(align) end
---Main axis: JUSTIFY_*.
function Panel:SetJustify(justify) end
function Panel:SetAlignSelf(align) end
---@param wrap boolean
function Panel:SetLayoutWrap(wrap) end
---Number: equal columns. Table: per column, numbers are px, strings like "1fr"/"auto".
---@param cols integer|(number|string)[]
function Panel:SetColumns(cols) end
---Share of the free space in a row/column parent.
---@param n number
function Panel:SetGrow(n) end
---@param visible boolean
function Panel:SetVisible(visible) end
---@return boolean
function Panel:IsVisible() end
function Panel:Show() end
function Panel:Hide() end
---@param enabled boolean
function Panel:SetEnabled(enabled) end
---@return boolean
function Panel:IsEnabled() end
function Panel:SetDisabled(disabled) end
---@return boolean
function Panel:GetDisabled() end
---@param text? string
function Panel:SetTooltip(text) end
---@param cursor "arrow"|"hand"|"beam"|"sizeall"|"sizewe"|"sizens"|"sizenesw"|"sizenwse"|"no"|"waiting"|"crosshair"|"blank"|string
function Panel:SetCursor(cursor) end
---@param alpha number 0..255
function Panel:SetAlpha(alpha) end
---@return number
function Panel:GetAlpha() end
function Panel:SetZPos(z) end
---@param color? Color
function Panel:SetBackgroundColor(color) end
---@return Color?
function Panel:GetBackgroundColor() end
function Panel:SetPaintBackground(paint) end
function Panel:SetMouseInputEnabled(enabled) end
---@param name string font from surface.CreateFont
function Panel:SetFont(name) end
---@return string
function Panel:GetFont() end
---@param color? Color
function Panel:SetTextColor(color) end
---@return Color?
function Panel:GetTextColor() end
---Numpad layout: 7 8 9 top, 4 5 6 middle, 1 2 3 bottom.
---@param n integer
function Panel:SetContentAlignment(n) end
---@return boolean
function Panel:IsHovered() end
---@return boolean
function Panel:IsDown() end
---@return boolean
function Panel:IsChildHovered() end
---@return boolean
function Panel:HasFocus() end
function Panel:RequestFocus() end
function Panel:KillFocus() end
---@param now? boolean call PerformLayout right away
function Panel:InvalidateLayout(now) end
function Panel:InvalidateParent() end
function Panel:InvalidateChildren(recursive) end
---Schedules Paint for this frame.
function Panel:InvalidatePaint() end
---@param every boolean
function Panel:SetPaintEveryFrame(every) end
---@param child Panel
function Panel:ScrollToChild(child) end
---@param callback? fun(self: Panel)
function Panel:AlphaTo(alpha, duration, delay, callback) end
---@param callback? fun(self: Panel)
function Panel:MoveTo(x, y, duration, delay, ease, callback) end
---@param callback? fun(self: Panel)
function Panel:SizeTo(w, h, duration, delay, ease, callback) end

---@class Label: Panel
local Label = {}
function Label:SetText(text) end
---@return string
function Label:GetText() end
---@param wrap boolean
function Label:SetWrap(wrap) end

---@class Button: Label

---@class TextEntry: Panel
---@field OnChange? fun(self: TextEntry)
---@field OnValueChange? fun(self: TextEntry, value: string)
---@field OnEnter? fun(self: TextEntry, value: string)
local TextEntry = {}
function TextEntry:SetValue(value) end
---@return string
function TextEntry:GetValue() end
function TextEntry:SetText(value) end
---@return string
function TextEntry:GetText() end
---@return number?
function TextEntry:GetFloat() end
---@return integer?
function TextEntry:GetInt() end
---@param text? string
function TextEntry:SetPlaceholderText(text) end
---@param n? integer
function TextEntry:SetMaxLength(n) end
---@param editable boolean
function TextEntry:SetEditable(editable) end
---@param multiline boolean
function TextEntry:SetMultiline(multiline) end
---@param t "text"|"password"|"email"|"number"|"search"|"tel"|"url"
function TextEntry:SetInputType(t) end
---@param numeric boolean
function TextEntry:SetNumeric(numeric) end

---@class CheckBox: Label
---@field OnChange? fun(self: CheckBox, checked: boolean)
local CheckBox = {}
---@param checked boolean
function CheckBox:SetChecked(checked) end
---@return boolean
function CheckBox:GetChecked() end
---@param checked boolean
function CheckBox:SetValue(checked) end
---@return boolean
function CheckBox:GetValue() end
function CheckBox:Toggle() end

---@class ComboBox: Panel
---@field OnSelect? fun(self: ComboBox, index: integer, text: string, data: any)
local ComboBox = {}
---@param text string
---@param data? any
---@param select? boolean
---@return integer index
function ComboBox:AddChoice(text, data, select) end
function ComboBox:Clear() end
---@param index integer
function ComboBox:ChooseOptionID(index) end
---@param text string
function ComboBox:ChooseOption(text) end
---@param text string
function ComboBox:SetValue(text) end
---@return integer?
function ComboBox:GetSelectedID() end
---@return string? text, any data
function ComboBox:GetSelected() end
---@return string?
function ComboBox:GetValue() end
---@return string?
function ComboBox:GetOptionText(index) end
---@return any
function ComboBox:GetOptionData(index) end

---@class Slider: Panel
---@field OnValueChanged? fun(self: Slider, value: number)
local Slider = {}
function Slider:SetMin(v) end
function Slider:SetMax(v) end
function Slider:SetMinMax(lo, hi) end
---@return number?
function Slider:GetMin() end
---@return number?
function Slider:GetMax() end
---@param decimals integer
function Slider:SetDecimals(decimals) end
function Slider:SetValue(v) end
---@return number
function Slider:GetValue() end

---@class NumberWang: Slider

---@class ProgressBar: Panel
local ProgressBar = {}
---@param fraction number 0..1
function ProgressBar:SetFraction(fraction) end
---@return number
function ProgressBar:GetFraction() end

---@class Image: Panel
local Image = {}
---@param url string
function Image:SetImage(url) end
---@return string?
function Image:GetImage() end
function Image:SetAltText(text) end
---@param keep boolean
function Image:SetKeepAspect(keep) end

---@class Link: Label
local Link = {}
---Same-origin URLs ("/users/1") navigate through the router. Only http, https, mailto and
---relative URLs are accepted; anything else (javascript:, data:) leaves the link without href.
---@param url string
---@return boolean accepted
function Link:SetURL(url) end
---@return string?
function Link:GetURL() end
---@param newTab boolean
function Link:SetNewTab(newTab) end

---@class ScrollPanel: Panel
local ScrollPanel = {}
---@param panel Panel
function ScrollPanel:AddItem(panel) end
---@return ScrollPanel
function ScrollPanel:GetCanvas() end

---@class Row: Panel
---@class Column: Panel
---@class Grid: Panel

---@class Page: Panel
---@field Route? string e.g. "/users/{id}" or "/files/{*path}"
---@field Title? string
---@field Params table<string, string> route parameters
---@field Query table<string, string> query string
---@field Path string
local Page = {}
---@param title string
function Page:SetTitle(title) end
---@return string?
function Page:GetTitle() end

---@class Frame: Panel
---@field Header Panel
---@field TitleLabel Label
---@field CloseButton Button
---@field OnClose? fun(self: Frame)
local Frame = {}
function Frame:SetTitle(title) end
---@return string
function Frame:GetTitle() end
---@param show boolean
function Frame:ShowCloseButton(show) end
---@param draggable boolean
function Frame:SetDraggable(draggable) end
---@return boolean
function Frame:GetDraggable() end
---Default true: Close() removes the frame instead of hiding it.
---@param delete boolean
function Frame:SetDeleteOnClose(delete) end
---@return boolean
function Frame:IsModal() end
---@param sizable boolean
function Frame:SetSizable(sizable) end
---@param blur boolean
function Frame:SetBackgroundBlur(blur) end
---Modal: dims the page, traps focus, Esc closes.
function Frame:MakePopup() end
function Frame:Close() end

---@class Menu: Panel
local Menu = {}
---@param text string
---@param fn? fun(option: MenuOption)
---@return MenuOption
function Menu:AddOption(text, fn) end
---@return Panel
function Menu:AddSpacer() end
---@param text string
---@param fn? fun(option: MenuOption)
---@return Menu submenu, MenuOption option
function Menu:AddSubMenu(text, fn) end
---Default true: the menu is removed once closed.
---@param delete boolean
function Menu:SetDeleteSelf(delete) end
---@return boolean
function Menu:IsOpen() end
---At the cursor, or at x, y in page coordinates.
---@param x? number
---@param y? number
function Menu:Open(x, y) end
function Menu:Hide() end
function Menu:CloseAll() end

---@class MenuOption: Label
local MenuOption = {}
---@return Menu
function MenuOption:GetMenu() end

ui = {}
---@generic T: table
---@param name string
---@param tbl T
---@param base? string
---@return T
function ui.Register(name, tbl, base) end
---Parent defaults to the root panel (above the page). The class name picks the return type,
---your own classes too if they are declared with ---@class.
---props: keys with a Set<Key> method are applied after Initialize ({ Text = "Hi" }; list values
---are unpacked for Size, Pos, Border, Radius, Shadow, Gradient...; Dock/DockMargin/DockPadding
---work too), other keys become fields before Initialize, the array part holds children (panels
---or functions(panel)), Slot = "name" picks a slot of the parent (false = the parent itself).
---@generic T: Panel
---@param name `T`
---@param parent? Panel
---@param props? table
---@return T
function ui.Create(name, parent, props) end
---App-wide context, visible to every panel's Inject.
---@param key any
---@param value any
function ui.Provide(key, value) end
---@param key any
---@param default? any
---@return any
function ui.Inject(key, default) end
---@param parent? Panel
---@return Menu
function ui.Menu(parent) end
---@param name string
---@return table?
function ui.GetClass(name) end
---@return boolean
function ui.Exists(name) end
---@return boolean
function ui.IsPanel(v) end
---@return Panel
function ui.GetRoot() end
---@param skin { background?: Color, text?: Color, textMuted?: Color, panel?: Color, accent?: Color, border?: Color, input?: Color, button?: Color, buttonHover?: Color, radius?: number }
function ui.SetSkin(skin) end
