//! Compiles the text art sources into embedded data via `jnr-artkit`.
//!
//! Skeleton: for now it only proves the build-dependency wiring by
//! generating `$OUT_DIR/assets.rs`. Later it walks `art/` and `content/`,
//! emits `cargo:rerun-if-changed` for every source and fails the build with
//! file/line on broken sprites.

use std::{env, fs, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    let out = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR is set by cargo"));
    let exts = jnr_artkit::SOURCE_EXTENSIONS
        .iter()
        .map(|e| format!("{e:?}"))
        .collect::<Vec<_>>()
        .join(", ");
    let generated = format!(
        "/// Art source extensions the build script compiled with.\n\
         pub const COMPILED_EXTENSIONS: &[&str] = &[{exts}];\n"
    );
    fs::write(out.join("assets.rs"), generated).expect("write generated assets.rs");
}
