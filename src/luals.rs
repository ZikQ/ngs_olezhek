use std::{fs, path::Path};

const TYPES: &str = include_str!("../runtime/types/ngs.lua");
const TESTING: &str = include_str!("../runtime/types/testing.lua");

/// One `#[derive(LuaType)]`, collected at link time.
#[doc(hidden)]
pub struct LuaClass {
    pub name: &'static str,
    pub source: &'static str,
}

inventory::collect!(LuaClass);

/// LuaLS annotations for every `#[derive(LuaType)]` type in the binary. Dev mode writes them
/// to `<lua>/.ngs/api.lua`; call this to write them elsewhere (CI, a pre-commit check).
pub fn lua_types() -> String {
    let mut classes: Vec<&LuaClass> = inventory::iter::<LuaClass>.into_iter().collect();
    classes.sort_by_key(|c| c.name);
    classes.dedup_by_key(|c| c.name);
    let mut out = String::from("---@meta\n-- Generated from Rust types with #[derive(LuaType)]. Do not edit.\n");
    for c in classes {
        out.push('\n');
        out.push_str(c.source);
    }
    out
}

fn write_if_changed(path: &Path, text: &str) -> std::io::Result<()> {
    if fs::read_to_string(path).ok().as_deref() == Some(text) {
        return Ok(());
    }
    fs::create_dir_all(path.parent().expect("file in a folder"))?;
    fs::write(path, text)
}

/// Keeps `<lua>/.ngs/ngs.lua` in sync with this runtime version and `api.lua` with the app's
/// Rust types, and creates `.luarc.json` in the project root if there is none: LuaLS reads it
/// only from the workspace root.
pub fn write(lua_dir: &Path) {
    let types_dir = lua_dir.join(".ngs");
    for (file, text) in [("ngs.lua", TYPES.to_string()), ("testing.lua", TESTING.to_string()), ("api.lua", lua_types())] {
        let path = types_dir.join(file);
        if let Err(e) = write_if_changed(&path, &text) {
            return eprintln!("ngs_olezhek: cannot write LuaLS types to {}: {e}", path.display());
        }
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
