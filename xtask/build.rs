//! Hashes the code that decides a solver result (E35, D-091) into the
//! `XTASK_SOLVER_CODE_HASH` compile-time variable: every file of
//! `crates/jnr-core` (the simulation, level and tuning parsing) and of
//! `xtask` (the solver), `Cargo.lock`, the workspace `Cargo.toml`, the
//! compiler version and the build profile. The solver cache
//! (`xtask/src/solver/cache.rs`) keys on it, so a cached result always comes
//! from the code that is compiled into this binary, never from sources
//! edited after the build. Art, render and app crates are left out: they
//! cannot change a route or a jump window.

use std::path::{Path, PathBuf};
use std::{env, fs, process};

/// FNV-1a, 64 bit.
struct Fnv(u64);

impl Fnv {
    fn bytes(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.0 ^= u64::from(b);
            self.0 = self.0.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
}

/// Every file under `dir` (recursively), sorted by path.
fn files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let mut paths: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
    paths.sort();
    for p in paths {
        if p.is_dir() {
            files(&p, out);
        } else {
            out.push(p);
        }
    }
}

fn main() {
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("set by cargo"));
    let root = manifest.join("..");
    let inputs = [
        root.join("crates/jnr-core"),
        manifest.join("src"),
        manifest.join("build.rs"),
        manifest.join("Cargo.toml"),
        root.join("Cargo.toml"),
        root.join("Cargo.lock"),
    ];
    let mut all = Vec::new();
    for p in &inputs {
        // Cargo scans a directory recursively for changed files.
        println!("cargo:rerun-if-changed={}", p.display());
        if p.is_dir() {
            files(p, &mut all);
        } else {
            all.push(p.clone());
        }
    }
    let mut h = Fnv(0xcbf2_9ce4_8422_2325);
    for p in &all {
        let rel = p.strip_prefix(&root).unwrap_or(p);
        h.bytes(rel.to_string_lossy().as_bytes());
        h.bytes(&[0]);
        let data = fs::read(p).unwrap_or_else(|e| {
            eprintln!("xtask build: read {}: {e}", p.display());
            process::exit(1);
        });
        h.bytes(&(data.len() as u64).to_le_bytes());
        h.bytes(&data);
    }
    let rustc = env::var("RUSTC").unwrap_or_else(|_| "rustc".into());
    let version = process::Command::new(rustc)
        .arg("-vV")
        .output()
        .map(|o| o.stdout)
        .unwrap_or_default();
    h.bytes(&version);
    for var in ["PROFILE", "OPT_LEVEL", "DEBUG", "TARGET"] {
        h.bytes(env::var(var).unwrap_or_default().as_bytes());
        h.bytes(&[0]);
    }
    println!("cargo:rustc-env=XTASK_SOLVER_CODE_HASH={:016x}", h.0);
}
