//! Compiles the text art sources into embedded data via `jnr-artkit`.
//!
//! Loads `art/palettes/main.palette`, every `art/sprites/**/*.sprite` and
//! the sprites generated from `art/generators/*.toml`, runs the art lints,
//! packs one atlas and writes to `$OUT_DIR`:
//! - `atlas.rgba`: raw RGBA8 pixels
//! - `atlas.rs`: `fn generated_atlas(pixels) -> Atlas` (metadata)
//! - `assets.rs`: small constants
//!
//! - `audio.rs`: `AUDIO_SOURCES`, every `audio/` sound source as
//!   `(path, include_str!(…))` (E58b), after `jnr_audio::bank::Bank::load`
//!   accepted them (parse, ids, music lint errors)
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

    println!("cargo:rerun-if-changed={}", root.join("audio").display());
    write(&out.join("audio.rs"), audio_sources(&root).as_bytes());
}

/// Audio source extensions (`README.md`, `targets.toml` are not sources).
const AUDIO_EXTENSIONS: [&str; 4] = ["sfx", "inst", "song", "quote"];

/// Every audio source under `audio/` (sorted), checked with
/// `Bank::load`; returns the generated `AUDIO_SOURCES` table.
fn audio_sources(root: &Path) -> String {
    let dir = root.join("audio");
    let mut files = Vec::new();
    let mut stack = vec![dir.clone()];
    while let Some(d) = stack.pop() {
        let Ok(entries) = fs::read_dir(&d) else {
            continue;
        };
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if p
                .extension()
                .and_then(|x| x.to_str())
                .is_some_and(|x| AUDIO_EXTENSIONS.contains(&x))
            {
                files.push(p);
            }
        }
    }
    files.sort();
    let texts: Vec<(String, String)> = files
        .iter()
        .map(|p| {
            let name = p
                .strip_prefix(root)
                .unwrap_or(p)
                .to_string_lossy()
                .replace('\\', "/");
            let text =
                fs::read_to_string(p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()));
            (name, text)
        })
        .collect();
    let refs: Vec<(&str, &str)> = texts
        .iter()
        .map(|(n, t)| (n.as_str(), t.as_str()))
        .collect();
    if let Err(errors) = jnr_audio::bank::Bank::load(&refs) {
        eprintln!("jnr-assets: the audio sources have errors:");
        for e in &errors {
            eprintln!("{e}");
        }
        eprintln!("(run `cargo xtask audio lint` for all findings)");
        process::exit(1);
    }
    let mut code = String::from(
        "/// Every sound source under `audio/` as (path from the workspace root,\n\
         /// text), sorted by path.\n\
         pub static AUDIO_SOURCES: &[(&str, &str)] = &[\n",
    );
    for (p, (name, _)) in files.iter().zip(&texts) {
        let abs = p.canonicalize().unwrap_or_else(|_| p.clone());
        code.push_str(&format!(
            "    ({name:?}, include_str!({:?})),\n",
            abs.display().to_string()
        ));
    }
    code.push_str("];\n");
    code
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
