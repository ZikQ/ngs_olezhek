use std::{fs, path::PathBuf};

use axum::{
    body::{Body, to_bytes},
    http::Request,
};
use ngs_olezhek::{App, EmbeddedLua, include_lua};
use ngs_olezhek_bundle::{Bundle, Kind};
use omnilua::Lua;
use tower::ServiceExt;

async fn fetch_bundle(app: App) -> Bundle {
    let req = Request::get("/_ngs/bundle").body(Body::empty()).unwrap();
    let res = app.into_router().unwrap().oneshot(req).await.unwrap();
    Bundle::decode(&to_bytes(res.into_body(), usize::MAX).await.unwrap()).unwrap()
}

fn names(b: &Bundle) -> Vec<&str> {
    b.entries.iter().map(|e| e.name.as_str()).collect()
}

fn contains(b: &Bundle, needle: &str) -> bool {
    b.entries.iter().any(|e| e.data.windows(needle.len()).any(|w| w == needle.as_bytes()))
}

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ngs_embed_{tag}_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[tokio::test]
async fn release_serves_embedded_stripped_bytecode() {
    let b = fetch_bundle(App::new().dev(false).lua(include_lua!("tests/fixtures/lua"))).await;
    assert!(!b.dev);
    assert_eq!(names(&b), ["autorun/init.lua", "pages/home.lua"]);
    assert!(b.entries.iter().all(|e| e.kind == Kind::Chunk));
    assert!(!contains(&b, "secret_local"), "local names must be stripped");
    assert!(!contains(&b, "function PAGE"), "no source text");
    let lua = Lua::new();
    for e in &b.entries {
        lua.load(&e.data[..]).into_function().unwrap();
    }
}

#[tokio::test]
async fn strip_false_keeps_debug_info() {
    let b = fetch_bundle(App::new().dev(false).strip(false).lua(include_lua!("tests/fixtures/lua"))).await;
    assert!(contains(&b, "secret_local"));
}

#[tokio::test]
async fn dev_reads_live_folder_instead_of_embedded() {
    let dir = temp_dir("live");
    fs::write(dir.join("a.lua"), "return 1").unwrap();
    let dir_str: &'static str = Box::leak(dir.to_string_lossy().into_owned().into_boxed_str());
    let embedded = EmbeddedLua::from_parts(dir_str, &[]);

    let router = App::new().dev(true).lua(embedded).into_router().unwrap();
    let get = || async {
        let req = Request::get("/_ngs/bundle").body(Body::empty()).unwrap();
        let res = router.clone().oneshot(req).await.unwrap();
        Bundle::decode(&to_bytes(res.into_body(), usize::MAX).await.unwrap()).unwrap()
    };
    assert_eq!(names(&get().await), ["a.lua"]);

    fs::write(dir.join("b.lua"), "return 2 +").unwrap();
    let b = get().await;
    assert_eq!(names(&b), ["a.lua", "b.lua"]);
    assert_eq!(b.entries[1].kind, Kind::Error, "dev ships compile errors to the client");
    fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn dev_falls_back_to_embedded_when_folder_is_gone() {
    let chunk = Lua::new().load("return 1").into_function().unwrap().dump(false).unwrap();
    let chunk: &'static [u8] = Box::leak(chunk.into_boxed_slice());
    let files: &'static [(&str, &[u8])] = Box::leak(Box::new([("only.lua", chunk)]));
    let b = fetch_bundle(App::new().dev(true).lua(EmbeddedLua::from_parts("Z:/no/such/dir", files))).await;
    assert!(b.dev);
    assert_eq!(names(&b), ["only.lua"]);
}
