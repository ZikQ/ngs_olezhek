-- Runs last. App code is trusted, but nothing in it should reach N or R: without the debug
-- library upvalues are opaque. The prelude keeps its own reference to debug.traceback.
-- Binary chunks are refused because a crafted one breaks the VM's assumptions about valid
-- bytecode; the bundle itself is loaded from Rust, not through `load`.
debug = nil
string.dump = nil
dofile, loadfile = nil, nil

local raw_load = load
function load(chunk, name, mode, ...)
    if mode ~= nil and mode ~= "t" then error("load: only text chunks are allowed", 2) end
    -- Forwarded as varargs: an explicit nil env differs from no env at all.
    return raw_load(chunk, name, "t", ...)
end
