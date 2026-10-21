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

    // Cargo scans a directory recursively for changed files, so a new or
    // edited file anywhere under `audio/` reruns this script.
    println!("cargo:rerun-if-changed={}", root.join("audio").display());
    write(&out.join("audio.rs"), audio_sources(&root).as_bytes());
}

/// The source directories under `audio/` and their extension (flat: as
/// `cargo xtask audio` reads them). `README.md` and `targets.toml` are not
/// sources.
const AUDIO_DIRS: [(&str, &str); 5] = [
    ("sfx", "sfx"),
    ("jingles", "sfx"),
    ("instruments", "inst"),
    ("music", "song"),
    ("quotes", "quote"),
];

/// Every audio source (sorted by its embedded name), checked with
/// `Bank::load`; returns the generated `AUDIO_SOURCES` table. A source file
/// anywhere else under `audio/` (nested, or in the wrong directory) is a
/// build error, so nothing is silently left out.
fn audio_sources(root: &Path) -> String {
    let dir = root.join("audio");
    let mut files: Vec<(String, PathBuf)> = Vec::new();
    let mut misplaced = Vec::new();
    let mut stack = vec![dir.clone()];
    while let Some(d) = stack.pop() {
        let entries =
            fs::read_dir(&d).unwrap_or_else(|e| panic!("read the audio dir {}: {e}", d.display()));
        for e in entries {
            let p = e
                .unwrap_or_else(|e| panic!("read the audio dir {}: {e}", d.display()))
                .path();
            if p.is_dir() {
                stack.push(p);
                continue;
            }
            let Some(ext) = p.extension().and_then(|x| x.to_str()) else {
                continue;
            };
            if !AUDIO_DIRS.iter().any(|(_, x)| *x == ext) {
                continue;
            }
            let rel = p.strip_prefix(root).unwrap_or(&p);
            let name = rel
                .iter()
                .map(|c| c.to_string_lossy())
                .collect::<Vec<_>>()
                .join("/");
            let parent = p.parent().filter(|par| par.parent() == Some(dir.as_path()));
            let known = parent
                .and_then(|par| par.file_name())
                .and_then(|n| n.to_str())
                .is_some_and(|n| AUDIO_DIRS.contains(&(n, ext)));
            if known {
                files.push((name, p));
            } else {
                misplaced.push(format!(
                    "{name}:1:1: error[id]: an audio source must sit directly in audio/sfx, audio/jingles (.sfx), audio/instruments (.inst), audio/music (.song) or audio/quotes (.quote)"
                ));
            }
        }
    }
    files.sort();
    let texts: Vec<String> = files
        .iter()
        .map(|(_, p)| fs::read_to_string(p).unwrap_or_else(|e| panic!("read {}: {e}", p.display())))
        .collect();
    let refs: Vec<(&str, &str)> = files
        .iter()
        .zip(&texts)
        .map(|((n, _), t)| (n.as_str(), t.as_str()))
        .collect();
    let errors = match jnr_audio::bank::Bank::load(&refs) {
        Ok(_) => misplaced,
        Err(e) => misplaced.into_iter().chain(e).collect(),
    };
    if !errors.is_empty() {
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
    for (name, p) in &files {
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
