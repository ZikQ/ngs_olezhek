use std::{fs, path::Path};

const TYPES: &str = include_str!("../runtime/types/ngs.lua");

/// Keeps `<lua>/.ngs/ngs.lua` in sync with this runtime version, and creates `.luarc.json`
/// in the project root if there is none: LuaLS reads it only from the workspace root.
pub fn write(lua_dir: &Path) {
    let types_dir = lua_dir.join(".ngs");
    let types = types_dir.join("ngs.lua");
    if fs::read_to_string(&types).ok().as_deref() != Some(TYPES)
        && let Err(e) = fs::create_dir_all(&types_dir).and_then(|_| fs::write(&types, TYPES))
    {
        return eprintln!("ngs_olezhek: cannot write LuaLS types to {}: {e}", types.display());
    }

    let Ok(root) = std::env::current_dir() else { return };
    let rc = root.join(".luarc.json");
    if rc.exists() || !root.join("Cargo.toml").exists() {
        return;
    }
    let library = types_dir.strip_prefix(&root).unwrap_or(&types_dir).to_string_lossy().replace('\\', "/");
    let json = format!(
        "{{\n  \"runtime.version\": \"Lua 5.4\",\n  \"workspace.library\": [\"{library}\"],\n  \"workspace.checkThirdParty\": false\n}}\n"
    );
    match fs::write(&rc, json) {
        Ok(()) => println!("ngs_olezhek: created {} for LuaLS autocompletion", rc.display()),
        Err(e) => eprintln!("ngs_olezhek: cannot create {}: {e}", rc.display()),
    }
}
