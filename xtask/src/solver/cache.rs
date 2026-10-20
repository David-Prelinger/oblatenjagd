//! The solver result cache (E35, D-091).
//!
//! A level's solve (route, search numbers, jump windows) depends only on
//! the level, the tuning, the solver options and the code of `jnr-core`
//! and `xtask`. The cache keeps the last complete solve of each level in
//! `$CARGO_TARGET_DIR/cache/solver/<level>.json` under a key over all of
//! them; a solve whose key matches is reused instead of searched and
//! measured again. The key:
//! - [`FORMAT`] (the entry format),
//! - `XTASK_SOLVER_CODE_HASH`, hashed at build time by `xtask/build.rs`
//!   over every file of `crates/jnr-core` and `xtask`, `Cargo.lock`, the
//!   workspace `Cargo.toml`, the compiler version and the build profile, so
//!   it always describes the code compiled into this binary,
//! - the parsed level and tuning (their `Debug` text: every field),
//! - the beam width, the frame budget and the wall-clock limit.
//!
//! Only complete solves are stored (a failed search may depend on machine
//! speed). A fresh target directory (CI) has no cache, so everything is
//! solved; `--no-cache` (`cargo xtask solve`, `cargo xtask verify`) ignores
//! and overwrites it.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use jnr_core::hash::Fnv;
use jnr_core::{Game, InputFrame, Level, Tuning};
use serde_json::{Value, json};

use super::{Bound, JumpWindow, Node, Search, Solution, SolveOptions, StopReason};

/// Version of the entry format; part of the key.
const FORMAT: &str = "solver-cache-1";

/// `$CARGO_TARGET_DIR/cache/solver`.
pub fn dir(root: &Path) -> PathBuf {
    crate::target_dir(root).join("cache/solver")
}

/// The cache key of solving `level` with `tuning` and `opts` (see the
/// module docs), as 16 hex digits.
pub fn key(level: &Level, tuning: &Tuning, opts: &SolveOptions) -> String {
    let mut h = Fnv::new();
    for part in [
        FORMAT.to_owned(),
        env!("XTASK_SOLVER_CODE_HASH").to_owned(),
        format!("{level:?}"),
        format!("{tuning:?}"),
        format!(
            "beam={} budget={} limit_ms={}",
            opts.beam_width,
            opts.frame_budget,
            opts.time_limit.as_millis()
        ),
    ] {
        h.u64(part.len() as u64);
        h.bytes(part.as_bytes());
    }
    format!("{:016x}", h.finish())
}

fn path(dir: &Path, id: &str) -> PathBuf {
    dir.join(format!("{id}.json"))
}

/// The cached solve of `level` if its entry has `key`, else `None` (no
/// entry, another key, or an entry that doesn't read back).
pub fn load(dir: &Path, level: &Level, tuning: &Tuning, key: &str) -> Option<Solution> {
    let text = fs::read_to_string(path(dir, &level.meta.id)).ok()?;
    let v: Value = serde_json::from_str(&text).ok()?;
    if v["key"].as_str() != Some(key) || v["level"].as_str() != Some(&level.meta.id) {
        return None;
    }
    let mut inputs = Vec::new();
    for run in v["inputs"].as_array()? {
        let bits = u8::try_from(run[0].as_u64()?).ok()?;
        let n = usize::try_from(run[1].as_u64()?).ok()?;
        inputs.extend(std::iter::repeat_n(InputFrame::from_bits(bits), n));
    }
    let bound = |s: &Value| match s.as_str()? {
        "fail" => Some(Bound::Fail),
        "route" => Some(Bound::Route),
        "max" => Some(Bound::Max),
        _ => None,
    };
    let int = |v: &Value| i32::try_from(v.as_i64()?).ok();
    let mut windows = Vec::new();
    for w in v["windows"].as_array()? {
        windows.push(JumpWindow {
            index: usize::try_from(w["index"].as_u64()?).ok()?,
            hold: u32::try_from(w["hold"].as_u64()?).ok()?,
            x: int(&w["x"])?,
            lo: int(&w["lo"])?,
            hi: int(&w["hi"])?,
            lo_bound: bound(&w["lo_bound"])?,
            hi_bound: bound(&w["hi_bound"])?,
        });
    }
    // The completing state, for the report's "furthest" numbers: the
    // cleaned route keeps every position of the search's route
    // (`clean_route`), so x and frame are the search's completing node's.
    let mut game = Game::new(level, tuning, 1);
    for &i in &inputs {
        game.step(i);
    }
    let search = Search {
        reason: StopReason::Complete,
        best: Node::new(game, 0, None),
        search_deaths: v["search_deaths"].as_u64()?,
        expanded: v["expanded"].as_u64()?,
        elapsed: Duration::from_millis(v["runtime_ms"].as_u64()?),
    };
    Some(Solution {
        level: level.meta.id.clone(),
        search,
        inputs,
        windows,
        cached: true,
    })
}

/// Stores a complete `sol` under `key` (an incomplete one is not stored).
pub fn store(dir: &Path, sol: &Solution, key: &str) -> Result<(), String> {
    if !sol.search.complete() {
        return Ok(());
    }
    let inputs: Vec<Value> = super::run_length(&sol.inputs)
        .into_iter()
        .map(|(i, n)| json!([i.bits(), n]))
        .collect();
    let windows: Vec<Value> = sol
        .windows
        .iter()
        .map(|w| {
            json!({
                "index": w.index,
                "hold": w.hold,
                "x": w.x,
                "lo": w.lo,
                "hi": w.hi,
                "lo_bound": w.lo_bound.as_str(),
                "hi_bound": w.hi_bound.as_str(),
            })
        })
        .collect();
    let entry = json!({
        "key": key,
        "level": sol.level,
        "search_deaths": sol.search.search_deaths,
        "expanded": sol.search.expanded,
        "runtime_ms": sol.search.elapsed.as_millis() as u64,
        "inputs": inputs,
        "windows": windows,
    });
    fs::create_dir_all(dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
    let p = path(dir, &sol.level);
    // Written next to it and renamed, so a reader never sees half an entry.
    let tmp = dir.join(format!("{}.json.{}.tmp", sol.level, std::process::id()));
    fs::write(&tmp, entry.to_string()).map_err(|e| format!("write {}: {e}", tmp.display()))?;
    fs::rename(&tmp, &p).map_err(|e| format!("rename to {}: {e}", p.display()))
}
