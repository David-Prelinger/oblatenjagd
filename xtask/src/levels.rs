//! `cargo xtask levels lint [id]`: checks every `content/levels/*.level`
//! (part of `check`, see `docs/validation.md` L3).
//!
//! Checks: the file parses (legend, grid, exactly one player start — all
//! enforced by `Level::parse`), `meta.id` is unique, `meta.title_key` (if
//! set) exists in `content/text/de.toml`, at least one goal, `time_limit` set.

use std::path::Path;
use std::process::ExitCode;

use jnr_core::{EntityKind, Texts};

use crate::content::{self, LevelFile};

pub const HELP: &str = "\
cargo xtask levels <command>

Commands:
  lint [id]   lint every content/levels/*.level (or only the level with this
              meta.id): parses, unique id, title_key exists, a goal, time_limit
";

pub fn run(root: &Path, args: &[String]) -> ExitCode {
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    match args.as_slice() {
        ["lint"] | ["lint", _] => {
            let (ok, text) = lint_report(root, args.get(1).copied());
            print!("{text}");
            if ok {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            }
        }
        [] | ["help" | "-h" | "--help"] => {
            print!("{HELP}");
            ExitCode::SUCCESS
        }
        _ => {
            eprintln!("error: unknown arguments: {}\n\n{HELP}", args.join(" "));
            ExitCode::from(2)
        }
    }
}

/// Runs the lint; returns (no problems, printable text). `only` limits the
/// per-level checks to the level with that id (ids stay unique-checked).
pub fn lint_report(root: &Path, only: Option<&str>) -> (bool, String) {
    let problems = match (content::all_levels(root), content::texts(root)) {
        (Ok(levels), Ok(texts)) => lint(&levels, &texts, only),
        (Err(e), _) | (_, Err(e)) => vec![e],
    };
    let mut text: String = problems.iter().map(|p| format!("{p}\n")).collect();
    text.push_str(&format!("levels lint: {} problem(s)\n", problems.len()));
    (problems.is_empty(), text)
}

/// The lint over already loaded levels (path, parse result) and texts.
pub fn lint(levels: &[LevelFile], texts: &Texts, only: Option<&str>) -> Vec<String> {
    let mut problems = Vec::new();
    let mut matched = false;
    for (i, (path, level)) in levels.iter().enumerate() {
        let shown = path.display();
        let level = match level {
            Ok(l) => l,
            Err(e) => {
                // Without an id we can't tell whether it is the one asked for.
                problems.push(format!("{shown}: {e}"));
                continue;
            }
        };
        let id = &level.meta.id;
        if let Some((other, _)) = levels[..i]
            .iter()
            .find(|(_, l)| l.as_ref().is_ok_and(|l| &l.meta.id == id))
        {
            problems.push(format!(
                "{shown}: meta.id `{id}` is also used by {}",
                other.display()
            ));
        }
        if only.is_some_and(|o| o != id) {
            continue;
        }
        matched = true;
        if let Some(key) = &level.meta.title_key
            && !texts.contains(key)
        {
            problems.push(format!(
                "{shown}: title_key `{key}` is missing in content/text/de.toml"
            ));
        }
        if !level.entities.iter().any(|e| e.kind == EntityKind::Goal) {
            problems.push(format!("{shown}: no goal (add a `goal` entity)"));
        }
        if level.meta.time_limit.is_none() {
            problems.push(format!("{shown}: meta.time_limit is not set"));
        }
    }
    if let Some(id) = only
        && !matched
    {
        problems.push(format!("no level with id `{id}` in content/levels"));
    }
    problems
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use jnr_core::Level;

    use super::*;

    fn level(meta: &str, grid: &str) -> Result<Level, String> {
        let text = format!(
            "[meta]\n{meta}\n[legend]\n\"#\" = \"solid\"\n\"P\" = {{ entity = \"player_start\" }}\n\"G\" = {{ entity = \"goal\" }}\n[map]\ngrid = '''\n{grid}\n'''\n"
        );
        Level::parse(&text).map_err(|e| e.to_string())
    }

    fn texts() -> Texts {
        Texts::parse("[stage.a]\ntitle = \"A\"\n").unwrap()
    }

    #[test]
    fn clean_levels_pass() {
        let levels = vec![
            (
                PathBuf::from("a.level"),
                level(
                    "id = \"a\"\ntitle_key = \"stage.a.title\"\ntime_limit = 300",
                    "P.G\n###",
                ),
            ),
            (
                PathBuf::from("b.level"),
                level("id = \"b\"\ntime_limit = 1", "PG"),
            ),
        ];
        assert_eq!(lint(&levels, &texts(), None), Vec::<String>::new());
    }

    #[test]
    fn finds_every_problem() {
        let levels = vec![
            (
                PathBuf::from("a.level"),
                level("id = \"a\"\ntime_limit = 1", "PG"),
            ),
            (
                PathBuf::from("b.level"),
                level("id = \"a\"\ntitle_key = \"stage.nope\"", "P.."),
            ),
            (PathBuf::from("c.level"), level("id = \"c\"", "P?")),
        ];
        let problems = lint(&levels, &texts(), None);
        assert_eq!(
            problems,
            vec![
                "b.level: meta.id `a` is also used by a.level",
                "b.level: title_key `stage.nope` is missing in content/text/de.toml",
                "b.level: no goal (add a `goal` entity)",
                "b.level: meta.time_limit is not set",
                "c.level: unknown grid character '?' at row 1, column 2 (not in [legend])",
            ]
        );
        // Only one level: the others' problems (except parse errors) are left out.
        let only_a = lint(&levels, &texts(), Some("a"));
        assert_eq!(only_a.len(), 5, "{only_a:?}");
        assert_eq!(
            lint(&levels[..1], &texts(), Some("zz")),
            vec!["no level with id `zz` in content/levels"]
        );
    }

    #[test]
    fn real_levels_lint_clean() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let (ok, text) = lint_report(&root, None);
        assert!(ok, "{text}");
    }
}
