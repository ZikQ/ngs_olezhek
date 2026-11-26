---@meta
-- Lua test files run by ngs_olezhek::testing::LuaTests (feature "testing").

---@alias PanelQuery string|{ Class?: string, Name?: string, Text?: string, Contains?: string, Visible?: boolean }|fun(p: Panel): boolean
---"GET /api/users/{id}" or "/api/users/1"; the method is optional and the query string is ignored
---unless the pattern has one.
---@alias RequestPattern string|fun(req: table): table?

---@class TestRequest
---@field method string
---@field url string
---@field body? string
---@field json? any the body decoded, if it was JSON
---@field query table<string, string> raw (percent-encoded) query values
---@field headers table<string, string>

---@class TestStream
---@field url string
---@field sse boolean
---@field sent string[] what the app sent with Stream:Send
---@field closed boolean
---@field Emit fun(event: "open"|"message"|"error"|"close", text?: string)

---@class TestContext
local t = {}
---Fails unless the values are equal; tables are compared by content.
function t.Eq(actual, expected, message) end
---Opens a page: the first call renders the app, later ones navigate like router.Go.
---@return Page
function t.Go(url) end
---@return Page
function t.Page() end
---@return string path, string query
function t.Path() end
---Delivers loaded modules, runs scheduled flushes and served requests.
function t.Settle() end
---"Button" matches the class and its subclasses, "#email" a SetName.
---@param query PanelQuery
---@param root? Panel
---@return Panel?
function t.Find(query, root) end
---@param query PanelQuery
---@param root? Panel
---@return Panel[]
function t.FindAll(query, root) end
---Like Find, but fails the test (listing the texts on screen) when nothing matches.
---@param query PanelQuery
---@param root? Panel
---@return Panel
function t.Get(query, root) end
---Every non-empty text under root, one per line.
---@param root? Panel
---@return string
function t.Text(root) end
---@param root? Panel
---@return boolean
function t.HasText(text, root) end
---@param panel Panel|PanelQuery
function t.Click(panel) end
---Replaces a TextEntry's text as typed, firing OnChange, OnValueChange and bindings.
---@param panel Panel|PanelQuery
---@param text string
function t.Type(panel, text) end
---@param panel Panel|PanelQuery
---@param key string "Enter", "Escape", ...
function t.Press(panel, key) end
---@param panel Panel|PanelQuery a ComboBox
---@param option string|integer text or 1-based index
function t.Select(panel, option) end
---@param panel Panel|PanelQuery a CheckBox
---@param on? boolean default true
function t.Check(panel, on) end
---Moves the fake clock, firing timers that come due.
---@param seconds number
function t.Advance(seconds) end
---A fake backend route answered automatically from now on. `answer` is sent as JSON with 200,
---or a function returning body or status, body.
---@param pattern RequestPattern
---@param answer any|fun(req: TestRequest, params: table<string, string>): any, any?
function t.Serve(pattern, answer) end
---Requests made so far that match (all of them without a pattern).
---@param pattern? RequestPattern
---@return TestRequest[]
function t.Requests(pattern) end
---The oldest unanswered matching request; fails the test if there is none.
---@param pattern RequestPattern
---@return TestRequest
function t.Request(pattern) end
---Answers the oldest unanswered matching request: t.Respond(p, value) or t.Respond(p, status, body).
---@param pattern RequestPattern
---@overload fun(pattern: RequestPattern, body: any)
---@param status integer
---@param body any
---@param content_type? string
function t.Respond(pattern, status, body, content_type) end
---Fails the oldest matching request as a network error.
---@param pattern RequestPattern
---@param message? string
function t.Fail(pattern, message) end
---@param pattern RequestPattern
---@return TestStream
function t.Stream(pattern) end
---@return string?
function t.Clipboard() end
---Takes the runtime errors reported so far; untaken ones fail the test.
---@return string[]
function t.Errors() end
---@param session? boolean sessionStorage instead of localStorage
---@return table<string, string>
function t.Storage(session) end
---The fake browser state (elements, fetches, timers) for checks the helpers do not cover.
t.MOCK = {}

---Registers a test case; every case runs in a fresh runtime with autorun already done.
---@param name string
---@param fn fun(t: TestContext)
function test(name, fn) end
