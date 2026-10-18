//! Loading game content from the workspace for headless tools: levels by
//! id, the tuning, the texts, and replay files.

use std::fs;
use std::path::{Path, PathBuf};

use jnr_core::{Level, Replay, Texts, Tuning};

/// `content/levels`.
pub fn levels_dir(root: &Path) -> PathBuf {
    root.join("content/levels")
}

/// `tests/replays`.
pub fn replays_dir(root: &Path) -> PathBuf {
    root.join("tests/replays")
}

/// `tests/golden`.
pub fn golden_dir(root: &Path) -> PathBuf {
    root.join("tests/golden")
}

/// Files in `dir` with extension `ext`, sorted by path. A missing directory
/// is an empty list.
pub fn files_with_ext(dir: &Path, ext: &str) -> Result<Vec<PathBuf>, String> {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(format!("read {}: {e}", dir.display())),
    };
    let mut files: Vec<PathBuf> = entries
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == ext))
        .collect();
    files.sort();
    Ok(files)
}

pub fn read(path: &Path) -> Result<String, String> {
    fs::read_to_string(path).map_err(|e| format!("read {}: {e}", path.display()))
}

/// `content/tuning.toml`, parsed.
pub fn tuning(root: &Path) -> Result<Tuning, String> {
    let path = root.join("content/tuning.toml");
    Tuning::parse(&read(&path)?).map_err(|e| format!("{}: {e}", path.display()))
}

/// `content/text/de.toml`, parsed.
pub fn texts(root: &Path) -> Result<Texts, String> {
    let path = root.join("content/text/de.toml");
    Texts::parse(&read(&path)?).map_err(|e| format!("{}: {e}", path.display()))
}

/// A level file and its parse result.
pub type LevelFile = (PathBuf, Result<Level, String>);

/// Every `content/levels/*.level` with its parse result, sorted by path.
pub fn all_levels(root: &Path) -> Result<Vec<LevelFile>, String> {
    files_with_ext(&levels_dir(root), "level")?
        .into_iter()
        .map(|p| {
            let level = read(&p).and_then(|t| Level::parse(&t).map_err(|e| e.to_string()));
            Ok((p, level))
        })
        .collect()
}

/// The level whose `meta.id` is `id`. Levels that fail to parse are skipped
/// here (`levels lint` reports them).
pub fn find_level(root: &Path, id: &str) -> Result<Level, String> {
    let levels: Vec<Level> = all_levels(root)?
        .into_iter()
        .filter_map(|(_, l)| l.ok())
        .collect();
    find_by_id(levels, id)
}

/// Picks the level with `meta.id == id`; the error lists the known ids.
pub fn find_by_id(levels: Vec<Level>, id: &str) -> Result<Level, String> {
    let known: Vec<String> = levels.iter().map(|l| l.meta.id.clone()).collect();
    levels.into_iter().find(|l| l.meta.id == id).ok_or_else(|| {
        format!(
            "no level with id `{id}` in content/levels (known: {})",
            known.join(", ")
        )
    })
}

/// Reads and parses a replay file; errors start with the path.
pub fn load_replay(path: &Path) -> Result<Replay, String> {
    Replay::parse(&read(path)?).map_err(|e| format!("{}: {e}", path.display()))
}

/// The replay's name: its file stem (`tests/replays/x.replay` → `x`).
pub fn replay_name(path: &Path) -> String {
    path.file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// A replay argument: a path (relative to the current directory or the
/// workspace root) or a bare name in `tests/replays/`.
pub fn resolve_replay(root: &Path, arg: &str) -> Result<PathBuf, String> {
    let candidates = [
        PathBuf::from(arg),
        root.join(arg),
        replays_dir(root).join(format!("{arg}.replay")),
    ];
    candidates
        .into_iter()
        .find(|p| p.is_file())
        .ok_or_else(|| format!("no replay file `{arg}` (try a path or a name in tests/replays/)"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("..")
    }

    fn level(id: &str) -> Level {
        let text = format!(
            "[meta]\nid = \"{id}\"\n[legend]\n\"P\" = {{ entity = \"player_start\" }}\n[map]\ngrid = '''\nP\n'''\n"
        );
        Level::parse(&text).unwrap()
    }

    #[test]
    fn find_by_id_picks_the_matching_level() {
        let found = find_by_id(vec![level("a"), level("b")], "b").unwrap();
        assert_eq!(found.meta.id, "b");
        assert_eq!(
            find_by_id(vec![level("a"), level("b")], "c").unwrap_err(),
            "no level with id `c` in content/levels (known: a, b)"
        );
    }

    #[test]
    fn real_content_loads() {
        let root = root();
        assert_eq!(find_level(&root, "testbed").unwrap().meta.id, "testbed");
        tuning(&root).unwrap();
        assert!(texts(&root).unwrap().contains("stage.clear"));
        let p = resolve_replay(&root, "testbed_route").unwrap();
        assert_eq!(replay_name(&p), "testbed_route");
        assert!(resolve_replay(&root, "nope").is_err());
    }
}
