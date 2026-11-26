//! Lua bundle shared by the server (build from disk), `include_lua!` (build at compile time)
//! and the wasm runtime (decode). Keeping it in one place guarantees the same chunk names,
//! file order and wire format everywhere.

use std::{
    fs, io,
    path::{Path, PathBuf},
};

use omnilua::{Function, Lua};

const MAGIC: &[u8; 4] = b"NGSB";
const VERSION: u8 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Compiled bytecode.
    Chunk,
    /// The file failed to compile; `data` holds the message. Shipped so the client can show it.
    Error,
}

#[derive(Debug, Clone)]
pub struct Entry {
    pub name: String,
    pub kind: Kind,
    pub data: Vec<u8>,
}

impl Entry {
    pub fn compile(lua: &Lua, name: &str, source: &[u8], strip: bool) -> Entry {
        let compiled = lua
            .load(source)
            .set_name(format!("@{name}"))
            .into_function()
            .and_then(|f: Function| f.dump(strip));
        match compiled {
            Ok(data) => Entry { name: name.into(), kind: Kind::Chunk, data },
            Err(e) => Entry { name: name.into(), kind: Kind::Error, data: e.to_string().into_bytes() },
        }
    }

    /// Drops line numbers and local names from an already compiled chunk.
    pub fn strip(self, lua: &Lua) -> Entry {
        if self.kind != Kind::Chunk {
            return self;
        }
        match lua.load(&self.data[..]).into_function().and_then(|f: Function| f.dump(true)) {
            Ok(data) => Entry { data, ..self },
            Err(e) => Entry { kind: Kind::Error, data: e.to_string().into_bytes(), ..self },
        }
    }

    pub fn error(&self) -> Option<String> {
        (self.kind == Kind::Error).then(|| String::from_utf8_lossy(&self.data).into_owned())
    }
}

pub struct Bundle {
    pub dev: bool,
    pub entries: Vec<Entry>,
}

impl Bundle {
    pub fn from_dir(dir: &Path, dev: bool, strip: bool) -> io::Result<Bundle> {
        let lua = Lua::new();
        let mut entries = Vec::new();
        for (name, path) in collect(dir)? {
            entries.push(Entry::compile(&lua, &name, &fs::read(path)?, strip));
        }
        Ok(Bundle { dev, entries })
    }

    pub fn errors(&self) -> Vec<String> {
        self.entries.iter().filter_map(Entry::error).collect()
    }

    /// `magic | version u8 | flags u8 | count u32 | (kind u8 | name u16+bytes | data u32+bytes)*`, little endian.
    pub fn encode(&self) -> Vec<u8> {
        let size = self.entries.iter().map(|e| 7 + e.name.len() + e.data.len()).sum::<usize>();
        let mut out = Vec::with_capacity(10 + size);
        out.extend_from_slice(MAGIC);
        out.push(VERSION);
        out.push(self.dev as u8);
        out.extend_from_slice(&(self.entries.len() as u32).to_le_bytes());
        for e in &self.entries {
            out.push(match e.kind {
                Kind::Chunk => 0,
                Kind::Error => 1,
            });
            out.extend_from_slice(&(e.name.len() as u16).to_le_bytes());
            out.extend_from_slice(e.name.as_bytes());
            out.extend_from_slice(&(e.data.len() as u32).to_le_bytes());
            out.extend_from_slice(&e.data);
        }
        out
    }

    pub fn decode(bytes: &[u8]) -> Result<Bundle, String> {
        let mut r = Reader { bytes, pos: 0 };
        if r.take(4)? != MAGIC {
            return Err("bad bundle magic".into());
        }
        let version = r.u8()?;
        if version != VERSION {
            return Err(format!("unsupported bundle version {version}"));
        }
        let dev = r.u8()? & 1 != 0;
        let count = r.u32()? as usize;
        let mut entries = Vec::with_capacity(count.min(4096));
        for _ in 0..count {
            let kind = if r.u8()? == 0 { Kind::Chunk } else { Kind::Error };
            let name_len = r.u16()? as usize;
            let name = String::from_utf8_lossy(r.take(name_len)?).into_owned();
            let data_len = r.u32()? as usize;
            let data = r.take(data_len)?.to_vec();
            entries.push(Entry { name, kind, data });
        }
        Ok(Bundle { dev, entries })
    }
}

/// `.lua` files under `dir` as (path relative to `dir` with `/`, full path).
/// `autorun/` comes first, everything else by name.
pub fn collect(dir: &Path) -> io::Result<Vec<(String, PathBuf)>> {
    let mut files = Vec::new();
    walk(dir, dir, &mut files)?;
    files.sort_by(|(a, _), (b, _)| (!a.starts_with("autorun/"), a).cmp(&(!b.starts_with("autorun/"), b)));
    Ok(files)
}

/// Chunk name for a file under `root` ("pages/home.lua"), or None if it is not a bundled
/// Lua file: wrong extension, outside `root`, or inside a hidden folder like `.ngs`.
pub fn lua_name(root: &Path, path: &Path) -> Option<String> {
    if path.extension()? != "lua" {
        return None;
    }
    let parts: Vec<_> = path.strip_prefix(root).ok()?.components().map(|c| c.as_os_str().to_string_lossy()).collect();
    if parts.is_empty() || parts.iter().any(|p| p.starts_with('.')) {
        return None;
    }
    Some(parts.join("/"))
}

fn walk(root: &Path, dir: &Path, out: &mut Vec<(String, PathBuf)>) -> io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if entry.file_name().to_string_lossy().starts_with('.') {
            continue;
        }
        if entry.file_type()?.is_dir() {
            walk(root, &path, out)?;
        } else if let Some(name) = lua_name(root, &path) {
            out.push((name, path));
        }
    }
    Ok(())
}

struct Reader<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], String> {
        let end = self.pos.checked_add(n).filter(|&e| e <= self.bytes.len()).ok_or("truncated bundle")?;
        let s = &self.bytes[self.pos..end];
        self.pos = end;
        Ok(s)
    }

    fn u8(&mut self) -> Result<u8, String> {
        Ok(self.take(1)?[0])
    }

    fn u16(&mut self) -> Result<u16, String> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }

    fn u32(&mut self) -> Result<u32, String> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_decode_round_trip() {
        let lua = Lua::new();
        let bundle = Bundle {
            dev: true,
            entries: vec![
                Entry::compile(&lua, "autorun/a.lua", b"return 1", false),
                Entry::compile(&lua, "pages/broken.lua", b"return 1 +", false),
            ],
        };
        let decoded = Bundle::decode(&bundle.encode()).unwrap();
        assert!(decoded.dev);
        assert_eq!(decoded.entries.len(), 2);
        assert_eq!(decoded.entries[0].kind, Kind::Chunk);
        assert_eq!(decoded.entries[0].data, bundle.entries[0].data);
        assert_eq!(decoded.entries[1].kind, Kind::Error);
        assert!(decoded.errors()[0].contains("pages/broken.lua:1:"), "{:?}", decoded.errors());
    }

    #[test]
    fn strip_keeps_chunk_runnable_and_drops_names() {
        let lua = Lua::new();
        let full = Entry::compile(&lua, "x.lua", b"local secret_name = 41 return secret_name + 1", false);
        let stripped = full.clone().strip(&lua);
        assert!(stripped.data.len() < full.data.len());
        assert!(!stripped.data.windows(11).any(|w| w == b"secret_name"));
        let n: i64 = lua.load(&stripped.data[..]).eval().unwrap();
        assert_eq!(n, 42);
    }

    #[test]
    fn truncated_bundle_is_rejected() {
        let lua = Lua::new();
        let bytes = Bundle { dev: false, entries: vec![Entry::compile(&lua, "a.lua", b"return 1", true)] }.encode();
        assert!(Bundle::decode(&bytes[..bytes.len() - 1]).is_err());
    }
}
