//! wasm only: let exactly quad-snd's JavaScript imports stay undefined at
//! link time (D-116). quad-snd 0.2.8 declares them in a plain `extern "C"`
//! block without `#[link(wasm_import_module = "env")]`, and current rustc no
//! longer passes `--allow-undefined` for wasm32, so the link would fail with
//! `undefined symbol: audio_init`. The symbols are listed in
//! `wasm-audio-imports.txt` (checked against quad-snd's source and the
//! version in `Cargo.lock` by an xtask test); every other undefined symbol
//! still fails the link. `web/audio.js` provides them at runtime.

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=wasm-audio-imports.txt");
    if std::env::var("CARGO_CFG_TARGET_ARCH").as_deref() == Ok("wasm32") {
        let dir = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is set by cargo");
        println!("cargo:rustc-link-arg-bins=--allow-undefined-file={dir}/wasm-audio-imports.txt");
    }
}
