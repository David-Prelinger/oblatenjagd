//! `cargo xtask levels lint [id]`: checks every `content/levels/*.level`
//! (part of `check`, see `docs/validation.md` L3).
//!
//! Checks: the file parses (legend, grid, exactly one player start — all
//! enforced by `Level::parse`), `meta.id` is unique, `meta.title_key` (if
//! set) and every sign's `text` key exist in `content/text/de.toml`, at
//! least one goal (`time_limit` is required by `Level::parse`), every
//! `prop` names a catalog prop sprite (`jnr_core::PROP_SPRITES`), and every
//! moving platform track (Festungsbahn, Gondel) has 3 tiles of free space
//! above the car (`jnr_core::platform::track_problems`) and a way back up
//! out of the pit under it: stair-stepped floor with steps up of at most 3
//! tiles to the upper end (`jnr_core::platform::recovery_problems`, D-027,
//! D-044; its doc comment lists what it does not check).

use std::path::Path;
use std::process::ExitCode;

use jnr_core::{PROP_SPRITES, SpawnKind, Texts, Tuning};

use crate::content::{self, LevelFile};

pub const HELP: &str = "\
cargo xtask levels <command>

Commands:
  lint [id]   lint every content/levels/*.level (or only the level with this
              meta.id): parses (incl. time_limit), unique id, title_key and
              sign text keys exist, a goal, prop sprites are catalog props,
              3 free tiles above every Festungsbahn and Gondel track, and
              a way back up (steps <= 3 tiles) from the pit under it to its
              upper end
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
    let loaded = (
        content::all_levels(root),
        content::texts(root),
        content::tuning(root),
    );
    let problems = match loaded {
        (Ok(levels), Ok(texts), Ok(tuning)) => lint(&levels, &texts, &tuning, only),
        (Err(e), _, _) | (_, Err(e), _) | (_, _, Err(e)) => vec![e],
    };
    let mut text: String = problems.iter().map(|p| format!("{p}\n")).collect();
    text.push_str(&format!("levels lint: {} problem(s)\n", problems.len()));
    (problems.is_empty(), text)
}

/// The lint over already loaded levels (path, parse result), texts and tuning.
pub fn lint(
    levels: &[LevelFile],
    texts: &Texts,
    tuning: &Tuning,
    only: Option<&str>,
) -> Vec<String> {
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
        for e in &level.entities {
            if let Some(key) = &e.text
                && !texts.contains(key)
            {
                problems.push(format!(
                    "{shown}: sign at row {}, column {}: text key `{key}` is missing in content/text/de.toml",
                    e.row + 1,
                    e.col + 1
                ));
            }
        }
        for e in &level.entities {
            if let Some(name) = &e.sprite
                && !PROP_SPRITES.contains(&name.as_str())
            {
                problems.push(format!(
                    "{shown}: prop at row {}, column {}: sprite `{name}` is not a catalog prop (one of {})",
                    e.row + 1,
                    e.col + 1,
                    PROP_SPRITES.join(", ")
                ));
            }
        }
        if !level.entities.iter().any(|e| e.kind == SpawnKind::Goal) {
            problems.push(format!("{shown}: no goal (add a `goal` entity)"));
        }
        for p in jnr_core::platform::track_problems(level, tuning)
            .into_iter()
            .chain(jnr_core::platform::recovery_problems(level, tuning))
        {
            problems.push(format!("{shown}: {p}"));
        }
        let test_level = path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.starts_with("00_"));
        for e in level.entities.iter().filter(|e| e.still && !test_level) {
            problems.push(format!(
                "{shown}: still enemy at row {}, column {}: `still = true` is for grey-box test levels (00_*.level) only",
                e.row + 1,
                e.col + 1
            ));
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
            "[meta]\n{meta}\n[legend]\n\"#\" = \"solid\"\n\"P\" = {{ entity = \"player_start\" }}\n\"G\" = {{ entity = \"goal\" }}\n\"s\" = {{ entity = \"sign\", text = \"stage.a.sign\" }}\n[map]\ngrid = '''\n{grid}\n'''\n"
        );
        Level::parse(&text).map_err(|e| e.to_string())
    }

    fn texts() -> Texts {
        Texts::parse("[stage.a]\ntitle = \"A\"\nsign = \"Servus\"\n").unwrap()
    }

    fn tuning() -> Tuning {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        content::tuning(&root).unwrap()
    }

    #[test]
    fn clean_levels_pass() {
        let levels = vec![
            (
                PathBuf::from("a.level"),
                level(
                    "id = \"a\"\ntitle_key = \"stage.a.title\"\ntime_limit = 300",
                    "PsG\n###",
                ),
            ),
            (
                PathBuf::from("b.level"),
                level("id = \"b\"\ntime_limit = 1", "PG"),
            ),
        ];
        assert_eq!(
            lint(&levels, &texts(), &tuning(), None),
            Vec::<String>::new()
        );
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
                level(
                    "id = \"a\"\ntime_limit = 5\ntitle_key = \"stage.nope\"",
                    "P.s",
                ),
            ),
            (
                PathBuf::from("c.level"),
                level("id = \"c\"\ntime_limit = 1", "P?"),
            ),
            (PathBuf::from("d.level"), level("id = \"d\"", "PG")),
        ];
        let texts = Texts::parse("[stage.a]\ntitle = \"A\"\n").unwrap();
        let problems = lint(&levels, &texts, &tuning(), None);
        assert_eq!(problems.len(), 6, "{problems:?}");
        assert_eq!(
            problems[..5],
            [
                "b.level: meta.id `a` is also used by a.level",
                "b.level: title_key `stage.nope` is missing in content/text/de.toml",
                "b.level: sign at row 1, column 3: text key `stage.a.sign` is missing in content/text/de.toml",
                "b.level: no goal (add a `goal` entity)",
                "c.level: unknown grid character '?' at row 1, column 2 (not in [legend])",
            ]
        );
        assert!(problems[5].starts_with("d.level: level: ") && problems[5].contains("time_limit"));
        // Only one level: the others' problems (except parse errors) are left out.
        let only_a = lint(&levels, &texts, &tuning(), Some("a"));
        assert_eq!(only_a.len(), 6, "{only_a:?}");
        assert_eq!(
            lint(&levels[..1], &texts, &tuning(), Some("zz")),
            vec!["no level with id `zz` in content/levels"]
        );
    }

    #[test]
    fn festungsbahn_needs_free_space_above_its_track() {
        let grid = |ceiling: &str| {
            format!(
                "[meta]\nid = \"f\"\ntime_limit = 1\n[legend]\n\"#\" = \"solid\"\n\"P\" = {{ entity = \"player_start\" }}\n\"G\" = {{ entity = \"goal\" }}\n\"F\" = {{ entity = \"festungsbahn\", to = [3, -2] }}\n[map]\ngrid = '''\n{ceiling}\n........\n........\n........\n........\nPF.....G\n########\n'''\n"
            )
        };
        let open = Level::parse(&grid("........")).map_err(|e| e.to_string());
        let low = Level::parse(&grid("....#...")).map_err(|e| e.to_string());
        let levels = vec![
            (PathBuf::from("open.level"), open),
            (PathBuf::from("low.level"), low),
        ];
        let texts = Texts::parse("").unwrap();
        let problems: Vec<String> = lint(&levels, &texts, &tuning(), None)
            .into_iter()
            .filter(|p| !p.contains("meta.id"))
            .collect();
        assert_eq!(problems.len(), 1, "{problems:?}");
        assert!(
            problems[0]
                .starts_with("low.level: festungsbahn at row 6, column 2: less than 48 px free"),
            "{}",
            problems[0]
        );
    }

    #[test]
    fn still_enemies_only_in_test_levels() {
        let text = "[meta]\nid = \"s\"\ntime_limit = 1\n[legend]\n\"P\" = { entity = \"player_start\" }\n\"G\" = { entity = \"goal\" }\n\"t\" = { entity = \"selfie_tourist\", still = true }\n[map]\ngrid = '''\nPtG\n'''\n";
        let level = || Level::parse(text).map_err(|e| e.to_string());
        let texts = Texts::parse("").unwrap();
        let test = vec![(PathBuf::from("00_s.level"), level())];
        assert_eq!(lint(&test, &texts, &tuning(), None), Vec::<String>::new());
        let stage = vec![(PathBuf::from("01_s.level"), level())];
        let problems = lint(&stage, &texts, &tuning(), None);
        assert_eq!(
            problems,
            [
                "01_s.level: still enemy at row 1, column 2: `still = true` is for grey-box test levels (00_*.level) only"
            ]
        );
    }

    #[test]
    fn props_must_name_a_catalog_prop_sprite() {
        let text = |sprite: &str| {
            format!(
                "[meta]\nid = \"p\"\ntime_limit = 1\n[legend]\n\"P\" = {{ entity = \"player_start\" }}\n\"G\" = {{ entity = \"goal\" }}\n\"s\" = {{ entity = \"prop\", sprite = \"{sprite}\", layer = \"back\" }}\n[map]\ngrid = '''\nPsG\n'''\n"
            )
        };
        let texts = Texts::parse("").unwrap();
        let check = |sprite: &str| {
            let levels = vec![(
                PathBuf::from("p.level"),
                Level::parse(&text(sprite)).map_err(|e| e.to_string()),
            )];
            lint(&levels, &texts, &tuning(), None)
        };
        for name in PROP_SPRITES {
            assert_eq!(check(name), Vec::<String>::new(), "{name}");
        }
        // PROP_SPRITES == the rows of the catalog's "Props and goal" table
        // and the M2 "Items and props" table that are props (not the
        // checkpoint, Maibaum, signs or the car, which have entities of
        // their own).
        let catalog = include_str!("../../docs/sprite-catalog.md");
        let section = |heading: &str, end: &str| {
            let start = catalog.find(heading).expect(heading);
            let s = &catalog[start..];
            &s[..s[3..].find(end).map_or(s.len(), |e| e + 3)]
        };
        let own_entity = |n: &str| {
            n == "checkpoint"
                || n == "festungsbahn"
                || n.starts_with("maibaum_")
                || n.starts_with("sign_")
        };
        let mut in_catalog: Vec<&str> = [
            section("## Props and goal", "\n## "),
            section("### Items and props (group `props`)", "\n### "),
        ]
        .into_iter()
        .flat_map(str::lines)
        .filter_map(|l| l.strip_prefix("| `")?.split('`').next())
        .filter(|n| !own_entity(n))
        .collect();
        in_catalog.sort_unstable();
        let mut listed = PROP_SPRITES.to_vec();
        listed.sort_unstable();
        assert_eq!(listed, in_catalog);
        let problems = check("sphere");
        assert_eq!(problems.len(), 1, "{problems:?}");
        assert!(
            problems[0].starts_with(
                "p.level: prop at row 1, column 2: sprite `sphere` is not a catalog prop (one of sphaera, "
            ),
            "{}",
            problems[0]
        );
    }

    #[test]
    fn real_levels_lint_clean() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let (ok, text) = lint_report(&root, None);
        assert!(ok, "{text}");
    }
}
