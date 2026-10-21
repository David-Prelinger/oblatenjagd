//! `cargo xtask levels lint [id]`: checks every `content/levels/*.level`
//! (part of `check`, see `docs/validation.md` L3).
//!
//! Checks: the file parses (legend, grid, exactly one player start — all
//! enforced by `Level::parse`), `meta.id` is unique, `meta.title_key` (if
//! set) and every sign's `text` key exist in `content/text/de.toml`, at
//! least one goal (`time_limit` is required by `Level::parse`), every
//! `prop` names a catalog prop sprite (`jnr_core::PROP_SPRITES`), and every
//! platform track (Festungsbahn, Gondel, Schafbergbahn, Traundl,
//! Dampfer; a Fensterladen is a zero-length track, D-094) has 3 tiles
//! of free space above the car (`jnr_core::platform::track_problems`; for
//! the Schafbergbahn its whole swept body and the 3 tiles above its roof
//! are free of solid and one-way tiles, D-067) and a way back up
//! out of the pit under it: stair-stepped floor with steps up of at most 3
//! tiles to the upper end (`jnr_core::platform::recovery_problems`, D-027,
//! D-044; its doc comment lists what it does not check; columns over water
//! are skipped, D-077).
//!
//! Swimming (M5, D-076, `jnr_core::swim::problems`): every column of swim
//! water ends on a solid tile inside the level (a lake bed); inside an
//! auto-scroll zone (the columns its camera shows while scrolling) no solid
//! tile sits on a swim surface.
//!
//! Climbing (M3, [`crate::climblint`]): no 7–9 tile drops in climb levels
//! (D-054), rock spawner niches 4–6 tiles above their ground (D-058), the
//! Durchkriechstein's tunnel, mouth and exit (D-057), and a warning (not a
//! failure) for climb-level shafts wider than 9 columns (D-055).
//!
//! Auto-scroll (M5, [`crate::scrolllint`], D-075): the goal at column
//! ≥ `to + 20`, squeeze faces in the zone at most 3 tiles above the surface
//! in front of them (2 over swim water), no raised steps in the lake bed
//! where the edge pushes (E31, D-085), and a checkpoint in the zone on a
//! deck tile.
//!
//! Music (MA, D-113): a journey stage (`01_`…`99_`) has `[meta] music`,
//! and every `music` names a known stage song.
//!
//! Signs (M5, [`crate::signlint`], E33, D-088): no placed Oblate
//! overlaps a sign board's drawn rectangle (known findings only warn).
//!
//! The boss arena (M7, D-106, `jnr_core::enemies::arena_problems`): a
//! level with an `oberkellner` has exactly one, in its last 20 columns on
//! the arena floor; a 3-high solid refuge at each end, one flat solid
//! floor between them with only air above it from row 8 (1-based);
//! no other enemy, sign, platform, box or brick in the arena; every bill
//! column (with its sway) over the floor.

use std::path::Path;
use std::process::ExitCode;

use jnr_core::{PROP_SPRITES, Texts, Tuning};

use crate::content::{self, LevelFile};

pub const HELP: &str = "\
cargo xtask levels <command>

Commands:
  lint [id]   lint every content/levels/*.level (or only the level with this
              meta.id): parses (incl. time_limit), unique id, title_key and
              sign text keys exist, a goal, prop sprites are catalog props,
              3 free tiles above every Festungsbahn, Gondel, Traundl,
              Dampfer and Fensterladen (the Schafbergbahn: its swept body and 3 tiles
              above its roof free of solid and one-way tiles), and a way
              back up (steps <= 3 tiles) from the pit under it to its upper
              end (columns over water skipped); swim water ends on a solid
              lake bed in every column (in an auto-scroll zone no solid tile
              sits on a swim surface); climbing: no 7-9 tile drops in climb levels, rock
              spawner niches 4-6 tiles above their ground, Durchkriechstein
              tunnel/mouth/exit; warns about climb shafts wider than 9
              columns (warnings never fail); no placed Oblate on a sign
              board (face and frame as drawn, posts left out); a level
              with an oberkellner: his arena (last 20 columns: refuges,
              flat floor, nothing above it, no other enemy/sign/box, bill
              columns over the floor); a journey stage (01_..99_) has
              [meta] music, and every music id is a known stage song
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
    let (problems, warned) = match loaded {
        (Ok(levels), Ok(texts), Ok(tuning)) => (
            lint(&levels, &texts, &tuning, only),
            warnings(&levels, &texts, &tuning, only),
        ),
        (Err(e), _, _) | (_, Err(e), _) | (_, _, Err(e)) => (vec![e], Vec::new()),
    };
    let mut text: String = problems.iter().map(|p| format!("{p}\n")).collect();
    text.extend(warned.iter().map(|w| format!("warning: {w}\n")));
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
        problems.extend(
            music_problems(path, level)
                .into_iter()
                .map(|p| format!("{shown}: {p}")),
        );
        if !level.entities.iter().any(|e| e.kind.is_goal()) {
            problems.push(format!(
                "{shown}: no goal (add a `goal` or `goal_stollen` entity)"
            ));
        }
        for p in jnr_core::platform::track_problems(level, tuning)
            .into_iter()
            .chain(jnr_core::platform::recovery_problems(level, tuning))
            .chain(jnr_core::enemies::reisebus_lane_problems(level, tuning))
            // The Oberkellner's arena (D-106).
            .chain(jnr_core::enemies::arena_problems(level, tuning))
            // No solid on a swim surface in the columns the scroll shows.
            .chain(jnr_core::swim::problems(level, scroll_columns(level)))
        {
            problems.push(format!("{shown}: {p}"));
        }
        let test_level = path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.starts_with("00_"));
        for p in crate::climblint::problems(level)
            .into_iter()
            .chain(crate::scrolllint::problems(level))
            .chain(crate::signlint::problems(
                level,
                texts,
                tuning,
                crate::signlint::KNOWN,
            ))
        {
            problems.push(format!("{shown}: {p}"));
        }
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

/// A journey stage's file: `NN_<id>.level` with `NN` 01–99 (`00_` are the
/// grey-box test levels).
fn journey_level(path: &Path) -> bool {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    let b = name.as_bytes();
    b.len() > 3
        && b[0].is_ascii_digit()
        && b[1].is_ascii_digit()
        && b[2] == b'_'
        && !name.starts_with("00_")
}

/// `[meta] music` (MA, D-113): a journey stage names its piece, and any
/// `music` is a known stage song (`jnr_audio::cues::Music::level_song`;
/// E58b checks it against the embedded `.song` files).
fn music_problems(path: &Path, level: &jnr_core::Level) -> Vec<String> {
    match &level.meta.music {
        Some(id) if jnr_audio::cues::Music::level_song(id).is_none() => {
            let known: Vec<&str> = jnr_audio::cues::Music::ALL
                .into_iter()
                .map(|m| m.id())
                .filter(|id| jnr_audio::cues::Music::level_song(id).is_some())
                .collect();
            vec![format!(
                "music `{id}` is not a known stage song (one of {})",
                known.join(", ")
            )]
        }
        None if journey_level(path) => {
            vec!["no music (a journey stage needs `music = \"<id>\"` in [meta], docs/audio-design.md §5)".to_owned()]
        }
        _ => Vec::new(),
    }
}

/// The columns an auto-scroll camera shows while it scrolls (D-075): its
/// left edge lies in `[from × 16, to × 16)`, so columns `from` to
/// `to − 1 + 20` (both inclusive); `None` without a zone.
fn scroll_columns(level: &jnr_core::Level) -> Option<(i32, i32)> {
    let screen_cols = jnr_core::level::SCREEN_COLS;
    level
        .meta
        .autoscroll_zone()
        .map(|(from, to)| (from, to - 1 + screen_cols))
}

/// Warnings (printed, never failing): climb shafts wider than the shaft
/// rule allows ([`crate::climblint::warnings`]) and known Oblaten on sign
/// boards ([`crate::signlint::warnings`]).
pub fn warnings(
    levels: &[LevelFile],
    texts: &Texts,
    tuning: &Tuning,
    only: Option<&str>,
) -> Vec<String> {
    levels
        .iter()
        .filter_map(|(path, level)| Some((path, level.as_ref().ok()?)))
        .filter(|(_, l)| only.is_none_or(|o| o == l.meta.id))
        .flat_map(|(path, l)| {
            crate::climblint::warnings(l)
                .into_iter()
                .chain(crate::signlint::warnings(
                    l,
                    texts,
                    tuning,
                    crate::signlint::KNOWN,
                ))
                .map(move |w| format!("{}: {w}", path.display()))
        })
        .collect()
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
                "b.level: no goal (add a `goal` or `goal_stollen` entity)",
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
    fn music_must_be_a_known_song_and_journey_stages_need_one() {
        let levels = vec![
            (
                PathBuf::from("00_test.level"),
                level("id = \"t\"\ntime_limit = 1", "PG"),
            ),
            (
                PathBuf::from("01_a.level"),
                level("id = \"a\"\ntime_limit = 1", "PG"),
            ),
            (
                PathBuf::from("02_b.level"),
                level("id = \"b\"\ntime_limit = 1\nmusic = \"salzburg\"", "PG"),
            ),
            (
                PathBuf::from("03_c.level"),
                level("id = \"c\"\ntime_limit = 1\nmusic = \"death\"", "PG"),
            ),
            (
                PathBuf::from("00_d.level"),
                level("id = \"d\"\ntime_limit = 1\nmusic = \"polka\"", "PG"),
            ),
        ];
        let problems = lint(&levels, &texts(), &tuning(), None);
        assert_eq!(problems.len(), 3, "{problems:?}");
        assert!(
            problems[0].starts_with("01_a.level: no music"),
            "{problems:?}"
        );
        assert!(
            problems[1].starts_with("03_c.level: music `death` is not a known stage song (one of title, route_map, salzburg,"),
            "{problems:?}"
        );
        assert!(
            problems[2].starts_with("00_d.level: music `polka`"),
            "{problems:?}"
        );
        assert!(!journey_level(Path::new("a.level")));
        assert!(journey_level(Path::new(
            "content/levels/09_cafe_zauner.level"
        )));
    }

    #[test]
    fn swim_water_needs_a_solid_bed() {
        let swim = |bed: &str| {
            let text = format!(
                "[meta]\nid = \"s\"\ntime_limit = 1\n[legend]\n\"#\" = \"solid\"\n\"~\" = \"see_wasser\"\n\
                 \"P\" = {{ entity = \"player_start\" }}\n\"G\" = {{ entity = \"goal\" }}\n[map]\ngrid = '''\n\
                 P...G\n#~~~#\n#~~~#\n{bed}\n'''\n"
            );
            vec![(
                PathBuf::from("s.level"),
                Level::parse(&text).map_err(|e| e.to_string()),
            )]
        };
        assert!(lint(&swim("#####"), &texts(), &tuning(), None).is_empty());
        let problems = lint(&swim("##.##"), &texts(), &tuning(), None);
        assert_eq!(problems.len(), 1, "{problems:?}");
        assert!(
            problems[0].starts_with("s.level: swim water at column 3, rows 2–3")
                && problems[0].contains("lake bed"),
            "{problems:?}"
        );
    }

    #[test]
    fn no_solid_on_a_swim_surface_in_the_scroll_columns() {
        // 50 columns, zone [2, 10]: the scroll shows columns 2..=29.
        let level = |solid_col: usize| {
            let mut top = ['.'; 50];
            top[solid_col] = '#';
            top[0] = 'P';
            top[49] = 'G';
            let water: String = "~".repeat(50);
            let text = format!(
                "[meta]\nid = \"z\"\ntime_limit = 1\ncamera = \"autoscroll\"\nautoscroll = [2, 10]\n\
                 [legend]\n\"#\" = \"solid\"\n\"~\" = \"see_wasser\"\n\
                 \"P\" = {{ entity = \"player_start\" }}\n\"G\" = {{ entity = \"goal\" }}\n[map]\ngrid = '''\n\
                 {}\n{water}\n{}\n'''\n",
                top.iter().collect::<String>(),
                "#".repeat(50)
            );
            let l = Level::parse(&text).map_err(|e| e.to_string());
            vec![(PathBuf::from("z.level"), l)]
        };
        let on_surface = |col: usize| {
            lint(&level(col), &texts(), &tuning(), None)
                .iter()
                .any(|p| p.contains("sits on a swim surface"))
        };
        assert_eq!(
            scroll_columns(level(5)[0].1.as_ref().unwrap()),
            Some((2, 29))
        );
        assert!(on_surface(2) && on_surface(29));
        assert!(!on_surface(1) && !on_surface(30), "outside the scroll");
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
    fn schafbergbahn_needs_its_swept_body_free() {
        // A [4, -1] track from cell (1, 5): the body's lower right corner
        // enters column 5 at once, its bottom at 96 px, so a step at row 5
        // (top 80) there cuts it; row 6 (top 96) clears it.
        let grid = |row5: &str, row6: &str| {
            format!(
                "[meta]\nid = \"z\"\ntime_limit = 1\n[legend]\n\"#\" = \"solid\"\n\"P\" = {{ entity = \"player_start\" }}\n\"G\" = {{ entity = \"goal\" }}\n\"Z\" = {{ entity = \"schafbergbahn\", to = [4, -1] }}\n[map]\ngrid = '''\nP.........\n..........\n..........\n..........\n.........G\n{row5}\n{row6}\n##########\n'''\n"
            )
        };
        let level =
            |row5: &str, row6: &str| Level::parse(&grid(row5, row6)).map_err(|e| e.to_string());
        let levels = vec![
            (
                PathBuf::from("clear.level"),
                level(".Z........", ".....#####"),
            ),
            (
                PathBuf::from("cut.level"),
                level(".Z...#....", ".....#####"),
            ),
        ];
        let texts = Texts::parse("").unwrap();
        let problems: Vec<String> = lint(&levels, &texts, &tuning(), None)
            .into_iter()
            .filter(|p| !p.contains("meta.id"))
            .collect();
        assert_eq!(problems.len(), 1, "{problems:?}");
        assert!(
            problems[0].starts_with(
                "cut.level: schafbergbahn at row 6, column 2: a tile overlaps the train's body"
            ),
            "{}",
            problems[0]
        );
    }

    #[test]
    fn a_reisebus_lane_must_be_clear() {
        // A bus at column 31 (1-based) on the street row 3: its lane is
        // rows 2-3, columns 7-34. A solid tile in row 2 at `wall` (0-based).
        let text = |wall: usize| {
            let mut above = vec!['.'; 39];
            above[wall] = '#';
            let above: String = above.into_iter().collect();
            let street = format!("P{}r.......G", ".".repeat(29));
            format!(
                "[meta]\nid = \"b\"\ntime_limit = 1\n[legend]\n\"#\" = \"solid\"\n\"P\" = {{ entity = \"player_start\" }}\n\"G\" = {{ entity = \"goal\" }}\n\"r\" = {{ entity = \"reisebus\", stop = 4 }}\n[map]\ngrid = '''\n{}\n{above}\n{street}\n{}\n'''\n",
                ".".repeat(39),
                "#".repeat(39)
            )
        };
        let texts = Texts::parse("").unwrap();
        let check = |wall: usize| {
            let levels = vec![(
                PathBuf::from("b.level"),
                Level::parse(&text(wall)).map_err(|e| e.to_string()),
            )];
            lint(&levels, &texts, &tuning(), None)
        };
        // 25 tiles left of the bus: outside its lane.
        assert_eq!(check(5), Vec::<String>::new());
        let problems = check(6);
        assert_eq!(problems.len(), 1, "{problems:?}");
        assert!(
            problems[0].starts_with(
                "b.level: reisebus at row 3, column 31: its lane (rows 2-3, columns 7-34) has a solid or one-way tile at row 2, column 7"
            ),
            "{problems:?}"
        );
    }

    #[test]
    fn still_enemies_only_in_test_levels() {
        let text = "[meta]\nid = \"s\"\ntime_limit = 1\nmusic = \"salzburg\"\n[legend]\n\"P\" = { entity = \"player_start\" }\n\"G\" = { entity = \"goal\" }\n\"t\" = { entity = \"selfie_tourist\", still = true }\n[map]\ngrid = '''\nPtG\n'''\n";
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
        // A Wartender is still by its kind and legal in stages (D-105).
        let queue = text.replace("selfie_tourist\", still = true", "wartender\"");
        let stage = vec![(
            PathBuf::from("09_s.level"),
            Level::parse(&queue).map_err(|e| e.to_string()),
        )];
        assert_eq!(lint(&stage, &texts, &tuning(), None), Vec::<String>::new());
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
        // PROP_SPRITES == the rows of the catalog's "Props and goal" table,
        // the M2 "Items and props", the M3 "Props", the M4 "Props", the M5
        // "Props", the M6 "Props" and the M7 "Props" tables that are props
        // (not the checkpoint, Maibaum, signs or the car, which have
        // entities of their own; the M4 train and its steam are in the
        // "Platforms" table, not read here).
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
            section("### Props (group `props`)", "\n### "),
            section("### Props (group `props`, M4)", "\n### "),
            section("### Props (group `props`, M5)", "\n### "),
            section("### Props (group `props`, M6)", "\n### "),
            section("### Props (group `props`, M7)", "\n### "),
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
    fn climb_problems_are_lint_problems_and_wide_shafts_only_warn() {
        // A ledge (columns 0–3, its right column the shaft's left wall)
        // beside a 10-column shaft (walls at columns 3 and 14) with a
        // plank; walking off the ledge into the shaft drops 9 tiles.
        let mut rows = vec!["P.............#", "###############"];
        rows[1] = "####..........#";
        rows.extend(["####..........#"; 7]);
        rows.push("####....--....#");
        rows.push("###############");
        rows.push("G..............");
        let text = format!(
            "[meta]\nid = \"c\"\ntime_limit = 1\ncamera = \"climb\"\n[legend]\n\"#\" = \"solid\"\n\"-\" = \"oneway\"\n\"P\" = {{ entity = \"player_start\" }}\n\"G\" = {{ entity = \"goal\" }}\n[map]\ngrid = '''\n{}\n'''\n",
            rows.join("\n")
        );
        let levels = vec![(
            PathBuf::from("c.level"),
            Level::parse(&text).map_err(|e| e.to_string()),
        )];
        let texts = Texts::parse("").unwrap();
        let problems = lint(&levels, &texts, &tuning(), None);
        assert_eq!(problems.len(), 1, "{problems:?}");
        assert!(
            problems[0].starts_with(
                "c.level: fall depth: walking off the right end of the surface at row 2, column 4 drops 9 tiles"
            ),
            "{problems:?}"
        );
        let warned = warnings(&levels, &texts, &tuning(), None);
        assert_eq!(
            warned,
            [
                "c.level: shaft between the walls at columns 4 and 15 (rows 2–10) is 10 columns wide inside; the shaft rule allows 9 (D-055)"
            ]
        );
        assert_eq!(
            warnings(&levels, &texts, &tuning(), Some("other")),
            Vec::<String>::new()
        );
    }

    #[test]
    fn real_levels_lint_clean() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let (ok, text) = lint_report(&root, None);
        assert!(ok, "{text}");
    }
}
