use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

const WASM_BINDGEN_VERSION: &str = "0.2.129";

fn main() {
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    for p in ["runtime/src", "runtime/lua", "runtime/style.css", "runtime/Cargo.toml", "bundle/src", "bundle/Cargo.toml"] {
        println!("cargo:rerun-if-changed={}", manifest.join(p).display());
    }
    println!("cargo:rerun-if-env-changed=NGS_RUNTIME_DIR");

    let out = PathBuf::from(env::var("OUT_DIR").unwrap()).join("runtime");
    fs::create_dir_all(&out).unwrap();

    if let Ok(prebuilt) = env::var("NGS_RUNTIME_DIR") {
        for f in ["runtime.js", "runtime_bg.wasm"] {
            fs::copy(Path::new(&prebuilt).join(f), out.join(f))
                .unwrap_or_else(|e| panic!("NGS_RUNTIME_DIR: cannot copy {f}: {e}"));
        }
        return;
    }

    let target_dir = manifest.join("target").join("ngs-runtime");
    build_wasm(&manifest, &target_dir);

    let wasm = target_dir.join("wasm32-unknown-unknown/wasm-release/ngs_olezhek_runtime.wasm");
    run(
        Command::new("wasm-bindgen")
            .args(["--target", "web", "--no-typescript", "--out-name", "runtime", "--out-dir"])
            .arg(&out)
            .arg(&wasm),
        &format!("wasm-bindgen not found; install it with `cargo install wasm-bindgen-cli --version {WASM_BINDGEN_VERSION}`"),
    );

    if env::var("PROFILE").as_deref() == Ok("release") {
        optimize(&out.join("runtime_bg.wasm"));
    }
}

fn build_wasm(manifest: &Path, target_dir: &Path) {
    let cargo = env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let mut cmd = Command::new(cargo);
    cmd.current_dir(manifest)
        .args(["build", "-p", "ngs_olezhek_runtime", "--target", "wasm32-unknown-unknown", "--profile", "wasm-release"])
        .arg("--target-dir")
        .arg(target_dir);

    // The outer cargo leaks host-specific flags and a jobserver into build scripts;
    // a nested wasm build must not inherit them, and clippy must not wrap it.
    for (key, _) in env::vars_os() {
        let key = key.to_string_lossy();
        if key.starts_with("CARGO_FEATURE_")
            || key.starts_with("CARGO_CFG_")
            || matches!(
                key.as_ref(),
                "CARGO_ENCODED_RUSTFLAGS"
                    | "RUSTFLAGS"
                    | "CARGO_TARGET_DIR"
                    | "CARGO_BUILD_TARGET"
                    | "CARGO_MAKEFLAGS"
                    | "CARGO_PRIMARY_PACKAGE"
                    | "RUSTC_WORKSPACE_WRAPPER"
            )
        {
            cmd.env_remove(key.as_ref());
        }
    }
    run(&mut cmd, "failed to start cargo for the wasm runtime");
}

fn optimize(wasm: &Path) {
    let tmp = wasm.with_extension("opt.wasm");
    let status = Command::new("wasm-opt")
        .args([
            "-Oz",
            "--enable-bulk-memory",
            "--enable-nontrapping-float-to-int",
            "--enable-sign-ext",
            "--enable-mutable-globals",
            "--enable-reference-types",
            "--enable-multivalue",
            "-o",
        ])
        .arg(&tmp)
        .arg(wasm)
        .status();
    match status {
        Ok(s) if s.success() => fs::rename(&tmp, wasm).unwrap(),
        _ => println!("cargo:warning=wasm-opt unavailable or failed, shipping unoptimized runtime"),
    }
}

fn run(cmd: &mut Command, spawn_error: &str) {
    let status = cmd.status().unwrap_or_else(|e| panic!("{spawn_error}: {e}"));
    if !status.success() {
        panic!("command failed: {cmd:?}");
    }
}
