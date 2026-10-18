//! Compiles the text art sources into embedded data via `jnr-artkit`.
//!
//! Loads `art/palettes/main.palette` and every `art/sprites/**/*.sprite`,
//! runs the art lints, packs one atlas and writes to `$OUT_DIR`:
//! - `atlas.rgba`: raw RGBA8 pixels
//! - `atlas.rs`: `fn generated_atlas(pixels) -> Atlas` (metadata)
//! - `assets.rs`: small constants
//!
//! Any lint error or syntax error fails the build with `file:line:col`.

use std::path::{Path, PathBuf};
use std::{env, fs, process};

use jnr_artkit::{Project, codegen};

fn main() {
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("set by cargo"));
    let root = manifest.join("../..");
    let out = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR is set by cargo"));

    println!("cargo:rerun-if-changed=build.rs");
    // Cargo scans a directory recursively for changed files.
    println!("cargo:rerun-if-changed={}", root.join("art").display());

    let atlas = Project::load(&root)
        .and_then(|p| {
            for w in p.lint().iter().filter(|d| !d.is_error()) {
                println!("cargo:warning={w}");
            }
            p.atlas()
        })
        .unwrap_or_else(|errors| fail(&errors));

    write(&out.join("atlas.rgba"), &atlas.pixels);
    write(
        &out.join("atlas.rs"),
        codegen::atlas_rust_source(&atlas).as_bytes(),
    );

    let exts = jnr_artkit::SOURCE_EXTENSIONS
        .iter()
        .map(|e| format!("{e:?}"))
        .collect::<Vec<_>>()
        .join(", ");
    let generated = format!(
        "/// Art source extensions the build script compiled with.\n\
         pub const COMPILED_EXTENSIONS: &[&str] = &[{exts}];\n"
    );
    write(&out.join("assets.rs"), generated.as_bytes());
}

fn fail(errors: &[jnr_artkit::Diagnostic]) -> ! {
    eprintln!("jnr-assets: the art sources have errors:");
    for e in errors {
        eprintln!("{e}");
    }
    eprintln!("(run `cargo xtask sprites lint` for all findings)");
    process::exit(1);
}

fn write(path: &Path, bytes: &[u8]) {
    // Only rewrite on change so dependants are not rebuilt needlessly.
    if fs::read(path).ok().as_deref() != Some(bytes) {
        fs::write(path, bytes).unwrap_or_else(|e| panic!("write {}: {e}", path.display()));
    }
}
