//! `levels lint` rules for climbing (Stage 4, D-054, D-055, D-057, D-058).
//!
//! - **Fall depth** (`camera = "climb"` levels): walking off any walkable
//!   edge into the open neighbouring column must drop ≤ 6 or ≥ 10 tiles to
//!   the next standable tile (or be bottomless, or end in a lethal tile).
//!   Drops of 7–9 tiles are survivable but land off screen at the climb
//!   camera's fall-follow limit.
//! - **Rock spawner niche** (every level): a `steinschlag` spawn's cell
//!   bottom is 4–6 tiles above the first **solid** tile below it (the path
//!   the player stands on; one-way tiles such as a `netz` shelter or a
//!   `steig` plank in between are skipped, D-062), so the whole niche is on
//!   screen below the HUD for a player standing there.
//! - **Durchkriechstein geometry** (every level): the rule lives in core,
//!   `jnr_core::crawl::geometry_problems` (D-057, D-060): the `len` tunnel
//!   cells from the spawn cell to the right are empty with solid tiles above
//!   and below; the mouth and the exit each have 2 empty rows over a floor
//!   at the tunnel floor's height.
//! - **Shaft width** (climb levels, a warning only): a shaft is a run of
//!   non-solid cells between two solid walls that keeps exactly those
//!   walls for at least [`SHAFT_MIN_ROWS`] rows and has a one-way tile in
//!   it. Its inner width should be ≤ [`MAX_SHAFT_WIDTH`] (the shaft rule:
//!   then the camera never strands a plank behind the screen's left edge).
//!   Shafts with jagged walls are not detected.
//!
//! "Walkable" means a solid or one-way tile with two empty cells above it
//! (Resi is 22–30 px tall); rows above the level count as empty.

use jnr_core::{CameraMode, Level, SpawnKind, TileKind};

/// Drops of this many tiles are rejected in climb levels (D-054).
pub const FORBIDDEN_DROPS: std::ops::RangeInclusive<i32> = 7..=9;
/// Allowed height of a rock spawner's niche bottom above its ground, tiles (D-058).
pub const NICHE_HEIGHT: std::ops::RangeInclusive<i32> = 4..=6;
/// The widest shaft the shaft rule allows, inner columns (D-055).
pub const MAX_SHAFT_WIDTH: i32 = 9;
/// A walled run must be at least this many rows tall to count as a shaft.
pub const SHAFT_MIN_ROWS: i32 = 6;

fn kind(level: &Level, c: i32, r: i32) -> Option<TileKind> {
    if r < 0 && (0..level.width).contains(&c) {
        return Some(TileKind::Empty);
    }
    level.tile(c, r).map(|t| t.kind)
}

fn empty(level: &Level, c: i32, r: i32) -> bool {
    kind(level, c, r) == Some(TileKind::Empty)
}

fn standable(level: &Level, c: i32, r: i32) -> bool {
    matches!(kind(level, c, r), Some(TileKind::Solid | TileKind::OneWay))
}

fn solid(level: &Level, c: i32, r: i32) -> bool {
    kind(level, c, r) == Some(TileKind::Solid)
}

/// Two empty cells above `(c, r)`: Resi fits standing on it.
fn headroom(level: &Level, c: i32, r: i32) -> bool {
    empty(level, c, r - 1) && empty(level, c, r - 2)
}

/// "row R, column C", 1-based like the other lint messages.
fn at(c: i32, r: i32) -> String {
    format!("row {}, column {}", r + 1, c + 1)
}

/// Every problem of `level` (empty if clean).
pub fn problems(level: &Level) -> Vec<String> {
    let mut out = Vec::new();
    if level.meta.camera == CameraMode::Climb {
        out.extend(fall_depth_problems(level));
    }
    for e in &level.entities {
        match e.kind {
            SpawnKind::Steinschlag => out.extend(niche_problem(level, e.col, e.row)),
            SpawnKind::Durchkriechstein => {
                out.extend(jnr_core::crawl::geometry_problems(level, e));
            }
            _ => {}
        }
    }
    out
}

/// Every warning of `level` (empty if clean).
pub fn warnings(level: &Level) -> Vec<String> {
    if level.meta.camera != CameraMode::Climb {
        return Vec::new();
    }
    shafts(level)
        .into_iter()
        .filter(|s| s.width() > MAX_SHAFT_WIDTH)
        .map(|s| {
            format!(
                "shaft between the walls at columns {} and {} (rows {}–{}) is {} columns wide inside; the shaft rule allows {MAX_SHAFT_WIDTH} (D-055)",
                s.left + 1,
                s.right + 1,
                s.top + 1,
                s.bottom + 1,
                s.width()
            )
        })
        .collect()
}

/// How far a fall down column `c` from row `r` (the row of the surface she
/// walked off) goes: `Some(tiles)` to the first standable tile, `None` if
/// bottomless or ending in a lethal tile.
fn drop_tiles(level: &Level, c: i32, r: i32) -> Option<i32> {
    (r..level.height).find_map(|rr| match kind(level, c, rr) {
        Some(TileKind::Solid | TileKind::OneWay) => Some(Some(rr - r)),
        Some(TileKind::Hazard | TileKind::Water) => Some(None),
        _ => None,
    })?
}

/// The fall-depth rule (module docs).
pub fn fall_depth_problems(level: &Level) -> Vec<String> {
    let mut out = Vec::new();
    for r in 0..level.height {
        for c in 0..level.width {
            if !standable(level, c, r) || !headroom(level, c, r) {
                continue;
            }
            for (dc, side) in [(-1, "left"), (1, "right")] {
                let n = c + dc;
                if !(0..level.width).contains(&n)
                    || standable(level, n, r)
                    || !headroom(level, n, r)
                {
                    continue;
                }
                if let Some(tiles) = drop_tiles(level, n, r)
                    && FORBIDDEN_DROPS.contains(&tiles)
                {
                    out.push(format!(
                        "fall depth: walking off the {side} end of the surface at {} drops {tiles} tiles; 7–9 tiles land off screen in a climb level, make it ≤ 6 or ≥ 10 (D-054)",
                        at(c, r)
                    ));
                }
            }
        }
    }
    out
}

/// The niche rule for a rock spawner in cell (`col`, `row`): measured to
/// the first solid tile below, skipping one-way tiles (D-062).
pub fn niche_problem(level: &Level, col: i32, row: i32) -> Option<String> {
    let ground = (row + 1..level.height).find(|&r| solid(level, col, r));
    let Some(ground) = ground else {
        return Some(format!(
            "steinschlag at {}: no solid tile below the niche (the path under the rock)",
            at(col, row)
        ));
    };
    let height = ground - row - 1;
    (!NICHE_HEIGHT.contains(&height)).then(|| {
        format!(
            "steinschlag at {}: the niche bottom is {height} tiles above the solid ground below it (row {}); it must be 4–6 (D-058, D-062)",
            at(col, row),
            ground + 1
        )
    })
}

/// A walled vertical run (see the module docs).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Shaft {
    /// Column of the left and right wall.
    pub left: i32,
    pub right: i32,
    /// First and last row.
    pub top: i32,
    pub bottom: i32,
}

impl Shaft {
    /// Inner width in columns.
    pub fn width(&self) -> i32 {
        self.right - self.left - 1
    }
}

/// The shafts of `level` (module docs), top to bottom, left to right.
pub fn shafts(level: &Level) -> Vec<Shaft> {
    // Bounded runs per row: (left wall, right wall).
    let runs = |r: i32| -> Vec<(i32, i32)> {
        let mut out = Vec::new();
        let mut c = 0;
        while c < level.width {
            if solid(level, c, r) && c + 1 < level.width && !solid(level, c + 1, r) {
                let right = (c + 1..level.width).find(|&x| solid(level, x, r));
                if let Some(right) = right {
                    out.push((c, right));
                    c = right;
                    continue;
                }
                break;
            }
            c += 1;
        }
        out
    };
    let rows: Vec<Vec<(i32, i32)>> = (0..level.height).map(runs).collect();
    let mut out = Vec::new();
    for (r, row) in rows.iter().enumerate() {
        let r = r as i32;
        for &(left, right) in row {
            // Only start at the top of a run of rows with the same walls.
            if r > 0 && rows[(r - 1) as usize].contains(&(left, right)) {
                continue;
            }
            let bottom = (r..level.height)
                .take_while(|&rr| rows[rr as usize].contains(&(left, right)))
                .last()
                .unwrap_or(r);
            let one_way = (r..=bottom)
                .any(|rr| (left + 1..right).any(|c| kind(level, c, rr) == Some(TileKind::OneWay)));
            if bottom - r + 1 >= SHAFT_MIN_ROWS && one_way {
                out.push(Shaft {
                    left,
                    right,
                    top: r,
                    bottom,
                });
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A level from grid rows: `#` solid, `-` one-way, `=` `steig`, `w`
    /// `steig_wall`, `^` hazard, `P` start.
    fn grid(camera: &str, rows: &[&str]) -> Level {
        let text = format!(
            "[meta]\nid = \"g\"\ntime_limit = 1\ncamera = \"{camera}\"\n[legend]\n\"#\" = \"solid\"\n\"-\" = \"oneway\"\n\"=\" = \"steig\"\n\"w\" = \"steig_wall\"\n\"^\" = \"hazard\"\n\"P\" = {{ entity = \"player_start\" }}\n[map]\ngrid = '''\n{}\n'''\n",
            rows.join("\n")
        );
        Level::parse(&text).unwrap()
    }

    /// A ledge (surface row 2, columns 0–2) and a floor `drop` tiles lower
    /// on the right (none: bottomless), 16 rows.
    fn ledge(camera: &str, drop: Option<usize>) -> Level {
        let rows: Vec<&str> = (0..16)
            .map(|r| match r {
                0 => "......",
                1 => "P.....",
                _ if drop.is_some_and(|d| r >= 2 + d) => "######",
                _ => "###...",
            })
            .collect();
        grid(camera, &rows)
    }

    #[test]
    fn drops_of_7_to_9_tiles_are_rejected_in_climb_levels() {
        for (drop, bad) in [
            (Some(1), false),
            (Some(6), false),
            (Some(7), true),
            (Some(8), true),
            (Some(9), true),
            (Some(10), false),
            (Some(13), false),
            (None, false),
        ] {
            let p = fall_depth_problems(&ledge("climb", drop));
            assert_eq!(p.len(), usize::from(bad), "{drop:?}: {p:?}");
            if bad {
                assert_eq!(
                    p[0],
                    format!(
                        "fall depth: walking off the right end of the surface at row 3, column 3 drops {} tiles; 7–9 tiles land off screen in a climb level, make it ≤ 6 or ≥ 10 (D-054)",
                        drop.unwrap()
                    )
                );
            }
        }
        // Only in climb levels.
        assert_eq!(problems(&ledge("side", Some(8))), Vec::<String>::new());
        assert_eq!(problems(&ledge("climb", Some(8))).len(), 1);
    }

    #[test]
    fn falls_into_hazards_walls_and_low_gaps_are_not_drops() {
        // Spikes 8 tiles down: a death anyway, not a fall-depth problem.
        let mut rows = vec!["P.....", "###...", "###...", "###...", "###..."];
        rows.extend(["###...", "###...", "###...", "###..."]);
        rows.extend(["######", "######"]);
        rows[9] = "###^^^";
        assert_eq!(
            fall_depth_problems(&grid("climb", &rows)),
            Vec::<String>::new()
        );
        // A gap under a low ceiling (one empty row) can't be walked into,
        // although the floor is 8 tiles below the ledge.
        let mut rows = vec!["P..###"];
        rows.extend(["###..."; 8]);
        rows.push("######");
        assert_eq!(
            fall_depth_problems(&grid("climb", &rows)),
            Vec::<String>::new()
        );
    }

    #[test]
    fn one_way_planks_count_as_surfaces() {
        // A plank 8 tiles above the floor: walking off its end is an 8-tile drop.
        let mut rows = vec!["......"; 12];
        rows[0] = "P.....";
        rows[2] = "..---.";
        rows[10] = "######";
        rows[11] = "######";
        let p = fall_depth_problems(&grid("climb", &rows));
        assert_eq!(p.len(), 2, "both ends: {p:?}");
        assert!(p[0].contains("left end of the surface at row 3, column 3 drops 8 tiles"));
        assert!(p[1].contains("right end of the surface at row 3, column 5 drops 8 tiles"));
        // The shaft planks `steig` and `steig_wall` are one-way surfaces
        // like any other: the same drop, and a plank catches a fall.
        for plank in ["..===.", "..www."] {
            rows[2] = plank;
            assert_eq!(fall_depth_problems(&grid("climb", &rows)), p, "{plank}");
        }
        rows[2] = "..---.";
        rows[5] = "wwwwww";
        assert_eq!(
            fall_depth_problems(&grid("climb", &rows)),
            Vec::<String>::new(),
            "3 tiles onto the steig_wall plank below"
        );
    }

    #[test]
    fn rock_niches_are_4_to_6_tiles_above_their_solid_ground() {
        let level = |h: usize| {
            let mut rows = vec!["......"; 12];
            rows[0] = "P.....";
            rows[1] = "######";
            rows[2 + h] = "######";
            grid("climb", &rows)
        };
        // The niche cell (2, 1): its bottom is row 2's top, the ground in
        // row 2 + h is `h` tiles lower.
        for (h, ok) in [(3, false), (4, true), (5, true), (6, true), (7, false)] {
            let p = niche_problem(&level(h), 2, 1);
            assert_eq!(p.is_none(), ok, "{h}: {p:?}");
        }
        let p = niche_problem(&level(3), 2, 1).unwrap();
        assert_eq!(
            p,
            "steinschlag at row 2, column 3: the niche bottom is 3 tiles above the solid ground below it (row 6); it must be 4–6 (D-058, D-062)"
        );
        let open = grid("climb", &["P..", "...", "..."]);
        assert!(
            niche_problem(&open, 1, 0)
                .unwrap()
                .contains("no solid tile below the niche")
        );
    }

    #[test]
    fn rock_niches_skip_one_way_shelters_and_planks() {
        // Falkenstein rock #1 (D-062): niche 6 tiles above the path, a net
        // (or a plank) 3 tiles above the path in between.
        for net in ["--", "==", "ww"] {
            let mut rows = vec!["......"; 12];
            rows[0] = "P.....";
            rows[1] = "######";
            let shelter = format!("..{net}..");
            rows[5] = &shelter;
            rows[8] = "######";
            assert_eq!(niche_problem(&grid("climb", &rows), 2, 1), None, "{net}");
        }
        // Only one-way tiles below: no ground.
        let mut rows = vec!["......"; 8];
        rows[0] = "P.....";
        rows[5] = "..--..";
        assert!(
            niche_problem(&grid("climb", &rows), 2, 1)
                .unwrap()
                .contains("no solid tile below the niche")
        );
    }

    #[test]
    fn durchkriechstein_geometry_comes_from_core() {
        // Boulder over columns 2–5, tunnel row 3 (rows 1–2 solid), no
        // floor under the exit.
        let text = "[meta]\nid = \"k\"\ntime_limit = 10\n[legend]\n\"#\" = \"solid\"\n\"P\" = { entity = \"player_start\" }\n\"K\" = { entity = \"durchkriechstein\" }\n[map]\ngrid = '''\nP.......\n..####..\n..####..\n..K.....\n######.#\n'''\n";
        let level = Level::parse(text).unwrap();
        let p = problems(&level);
        assert_eq!(p.len(), 1, "{p:?}");
        assert!(
            p[0].starts_with(
                "durchkriechstein at row 4, column 3: the exit (column 7) needs a floor"
            ),
            "{p:?}"
        );
    }

    /// A shaft `inner` columns wide, 8 rows tall, with planks.
    fn shaft(inner: usize) -> Level {
        let wall = |fill: &str| format!("#{}#", fill.repeat(inner));
        let mut rows: Vec<String> = vec!["P".to_owned() + &".".repeat(inner + 1)];
        for r in 0..8 {
            rows.push(if r % 3 == 1 {
                format!("#--{}#", ".".repeat(inner - 2))
            } else {
                wall(".")
            });
        }
        rows.push(wall("#"));
        let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
        grid("climb", &rows)
    }

    #[test]
    fn shafts_wider_than_9_columns_are_warned_about() {
        assert_eq!(
            shafts(&shaft(9)),
            [Shaft {
                left: 0,
                right: 10,
                top: 1,
                bottom: 8
            }]
        );
        assert_eq!(warnings(&shaft(9)), Vec::<String>::new());
        assert_eq!(
            warnings(&shaft(10)),
            [
                "shaft between the walls at columns 1 and 12 (rows 2–9) is 10 columns wide inside; the shaft rule allows 9 (D-055)"
            ]
        );
        // Not a shaft: too short, or without one-way tiles (a cave).
        let cave = grid(
            "climb",
            &[
                "P....", "#...#", "#...#", "#...#", "#...#", "#...#", "#...#", "#####",
            ],
        );
        assert_eq!(shafts(&cave), []);
        let short = grid("climb", &["P....", "#-..#", "#...#", "#####"]);
        assert_eq!(shafts(&short), []);
        // Side levels never warn.
        let side = Level {
            meta: jnr_core::level::LevelMeta {
                camera: CameraMode::Side,
                ..shaft(10).meta
            },
            ..shaft(10)
        };
        assert_eq!(warnings(&side), Vec::<String>::new());
    }

    #[test]
    fn the_climb_test_level_is_clean() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let level = crate::content::find_level(&root, "climb_test").unwrap();
        assert_eq!(problems(&level), Vec::<String>::new());
        assert_eq!(warnings(&level), Vec::<String>::new());
        assert_eq!(
            shafts(&level),
            [Shaft {
                left: 35,
                right: 45,
                top: 15,
                bottom: 26
            }]
        );
    }
}
