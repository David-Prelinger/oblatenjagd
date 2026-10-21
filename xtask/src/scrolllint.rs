//! `levels lint` rules for auto-scroll levels (`camera = "autoscroll"`,
//! D-075). The zone `[from, to]` is seen from camera x `from × 16` to
//! `to × 16`, i.e. columns `from .. to + 20`.
//!
//! - **Goal**: every goal column is ≥ `to + 20`, so the Maibaum sequence
//!   always runs with the side camera (D-029, D-040 unchanged).
//! - **Squeeze faces**: a *face* is a vertical run of solid cells whose
//!   left neighbours are not solid, in a column whose left neighbour is on
//!   screen during the scroll. The *surface in front* of it is the first
//!   non-empty cell in the left neighbour column from the face's top down:
//!   a deck, a hazard or the ground, or the top of a body of water. A face
//!   with at least [`PASS_UNDER_ROWS`] empty rows between its bottom and
//!   that surface is overhead (Resi passes under it in every form) and is
//!   skipped; any other face's top must be ≤ [`MAX_FACE_TILES`] tiles above
//!   the surface, so the edge can only push her onto it, never pin her;
//!   over a **swim** surface ≤ [`MAX_FACE_TILES_OVER_SWIM`] (D-084: a
//!   swimmer climbs it with a held jump-out, and big Resi's reaches only
//!   ~45 px). A face with nothing below it in front (bottomless) is
//!   skipped.
//! - **Under-water faces** (D-085): a face whose front cell at its top is
//!   swim water lies wholly under the water line, so the rule above can't
//!   see it. A diver resting on the lake bed is pushed sideways into it
//!   with no step-up and squeezed, so it is an error unless at least
//!   [`PASS_UNDER_ROWS`] rows of swim water lie between its bottom and the
//!   bed in front (a block she passes under). In short: no raised steps in
//!   the lake bed where the edge pushes. A quay or pier face rises above
//!   the water line and stays under the swim-surface rule; faces under
//!   *lethal* water are not checked (there is no diver to squeeze). Only
//!   faces in columns ≤ `to + 1` are checked: the edge stops at camera x
//!   `to × 16` and pushes a hitbox ≤ 16 px wide, so it never reaches
//!   further, and a beach rising to the lead-out quay is fine (the faces
//!   above use the stricter `to + 20`, D-083).
//! - **Checkpoint**: a checkpoint in the zone's columns stands on a deck
//!   tile (solid or one-way), not on water.

use jnr_core::level::SCREEN_COLS;
use jnr_core::{Level, SpawnKind, TileKind};

/// Highest a face in an auto-scroll zone may rise above the surface in
/// front of it, tiles (D-075).
pub const MAX_FACE_TILES: i32 = 3;
/// The same over a swim surface (D-084, [`jnr_core::swim::MAX_FACE_TILES`]).
pub use jnr_core::swim::MAX_FACE_TILES as MAX_FACE_TILES_OVER_SWIM;
/// Empty rows under a face that Resi passes through in every form (the
/// tallest hitbox is 30 px).
pub const PASS_UNDER_ROWS: i32 = 2;

fn kind(level: &Level, c: i32, r: i32) -> Option<TileKind> {
    level.tile(c, r).map(|t| t.kind)
}

fn solid(level: &Level, c: i32, r: i32) -> bool {
    kind(level, c, r) == Some(TileKind::Solid)
}

/// The surface in front of a face whose top is at row `top`, in column
/// `c`: the first non-empty cell from `top` down (for water, the top row
/// of that water body), and whether it is swim water. `None` if the column
/// is empty down to the bottom.
fn surface(level: &Level, c: i32, top: i32) -> Option<(i32, bool)> {
    let r = (top..level.height).find(|&r| kind(level, c, r) != Some(TileKind::Empty))?;
    let water = |r: i32| match kind(level, c, r) {
        Some(TileKind::Water | TileKind::Swim) => true,
        Some(TileKind::Empty | TileKind::Solid | TileKind::OneWay | TileKind::Hazard) | None => {
            false
        }
    };
    let mut top = r;
    while top > 0 && water(top - 1) {
        top -= 1;
    }
    Some(if water(r) {
        (top, kind(level, c, top) == Some(TileKind::Swim))
    } else {
        (r, false)
    })
}

/// "row R, column C", 1-based like the other lint messages.
fn at(c: i32, r: i32) -> String {
    format!("row {}, column {}", r + 1, c + 1)
}

/// Every problem of `level` (empty if clean or not an auto-scroll level).
pub fn problems(level: &Level) -> Vec<String> {
    let Some((from, to)) = level.meta.autoscroll_zone() else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let seen_to = to + SCREEN_COLS;
    for e in &level.entities {
        match e.kind {
            SpawnKind::Goal | SpawnKind::GoalStollen if e.col < seen_to => out.push(format!(
                "goal at {}: in an auto-scroll level the goal must be at column >= {} (to + {SCREEN_COLS}, 1-based; the goal sequence uses the side camera, D-075)",
                at(e.col, e.row),
                seen_to + 1
            )),
            SpawnKind::Checkpoint
                if (from..seen_to).contains(&e.col)
                    && !matches!(
                        kind(level, e.col, e.row + 1),
                        Some(TileKind::Solid | TileKind::OneWay)
                    ) =>
            {
                out.push(format!(
                    "checkpoint at {}: in the auto-scroll zone a checkpoint stands on a deck tile (solid or one-way), not on water or air (D-075)",
                    at(e.col, e.row)
                ));
            }
            _ => {}
        }
    }
    for c in (from + 1).max(1)..=seen_to.min(level.width - 1) {
        let mut r = 0;
        while r < level.height {
            if !(solid(level, c, r) && !solid(level, c - 1, r)) {
                r += 1;
                continue;
            }
            let top = r;
            while r + 1 < level.height && solid(level, c, r + 1) && !solid(level, c - 1, r + 1) {
                r += 1;
            }
            let bottom = r;
            r += 1;
            if kind(level, c - 1, top) == Some(TileKind::Swim) {
                if c > to + 1 {
                    continue;
                }
                let below = (bottom + 1..level.height)
                    .take_while(|&r| kind(level, c - 1, r) == Some(TileKind::Swim))
                    .count() as i32;
                if below < PASS_UNDER_ROWS {
                    let bed = bottom + 1 + below;
                    out.push(format!(
                        "left-facing solid face at column {}, rows {}–{}: under water, {} tiles above the lake bed in front of it ({}); in an auto-scroll zone the lake bed has no raised steps (a block under water leaves >= {PASS_UNDER_ROWS} rows of water below it), since the edge's push has no step-up and squeezes a diver resting on the bed (D-085)",
                        c + 1,
                        top + 1,
                        bottom + 1,
                        bed - top,
                        at(c - 1, bed)
                    ));
                }
                continue;
            }
            let Some((s, swim)) = surface(level, c - 1, top) else {
                continue;
            };
            if s - bottom > PASS_UNDER_ROWS {
                continue;
            }
            let (max, what, why) = if swim {
                (
                    MAX_FACE_TILES_OVER_SWIM,
                    "swim surface",
                    "a held jump-out lands on it in every form (D-084)",
                )
            } else {
                (
                    MAX_FACE_TILES,
                    "surface",
                    "the screen's edge can push Resi onto it instead of squeezing her (D-075)",
                )
            };
            if s - top > max {
                out.push(format!(
                    "left-facing solid face at column {}, rows {}–{}: its top is {} tiles above the {what} in front of it ({}); in an auto-scroll zone at most {max}, so {why}",
                    c + 1,
                    top + 1,
                    bottom + 1,
                    s - top,
                    at(c - 1, s)
                ));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 14-row auto-scroll level 32 columns wide (zone [2, 10]: seen
    /// columns 2..30) from `rows` (bottom-aligned, padded with air), a goal
    /// at column 31 unless `rows` has one.
    fn grid(rows: &[&str]) -> Level {
        let mut all: Vec<String> = vec![".".repeat(32); 14 - rows.len()];
        all.extend(rows.iter().map(|r| format!("{r:.<32}")));
        if !rows.iter().any(|r| r.contains('G')) {
            all[0].replace_range(31..32, "G");
        }
        let text = format!(
            "[meta]\nid = \"s\"\ntime_limit = 1\ncamera = \"autoscroll\"\nautoscroll = [2, 10]\n[legend]\n\"#\" = \"solid\"\n\"-\" = \"oneway\"\n\"~\" = \"wolfgangsee_water\"\n\"s\" = \"see_wasser\"\n\"C\" = {{ entity = \"checkpoint\" }}\n\"P\" = {{ entity = \"player_start\" }}\n\"G\" = {{ entity = \"goal\" }}\n[map]\ngrid = '''\n{}\n'''\n",
            all.join("\n")
        );
        Level::parse(&text).unwrap()
    }

    #[test]
    fn a_wall_up_to_3_tiles_is_fine_4_is_not() {
        let three = grid(&[
            "..........#",
            "..........#",
            "P.........#",
            "##############################",
        ]);
        assert_eq!(problems(&three), Vec::<String>::new());
        let four = grid(&[
            "..........#",
            "..........#",
            "..........#",
            "P.........#",
            "##############################",
        ]);
        let p = problems(&four);
        assert_eq!(p.len(), 1, "{p:?}");
        assert!(
            p[0].starts_with("left-facing solid face at column 11, rows 10–13: its top is 4 tiles above the surface in front of it (row 14, column 10)"),
            "{p:?}"
        );
    }

    #[test]
    fn swim_water_is_a_surface_in_front_of_a_face() {
        // Swim water rows 11-12 in front of the wall: its surface is the
        // top of the water body (row 12, 1-based), like lethal water.
        let p = problems(&grid(&[
            "..........#",
            "..........#",
            "..........#",
            "P.........#",
            "#sssssssss#",
            "#sssssssss#",
            "##############################",
        ]));
        assert_eq!(p.len(), 1, "{p:?}");
        assert!(
            p[0].contains("4 tiles above the swim surface in front of it (row 12, column 10)"),
            "{p:?}"
        );
    }

    #[test]
    fn a_face_over_swim_water_is_at_most_2_tiles() {
        // D-084: 2 tiles above the swim surface is fine, 3 is not (it would
        // be fine over a deck or lethal water).
        let face = |rows: usize, water: char| {
            let mut g: Vec<String> = (0..rows).map(|_| "..........#".to_owned()).collect();
            g.push(format!("#{}#", water.to_string().repeat(9)));
            g.push(format!("#{}#", water.to_string().repeat(9)));
            g.push("##############################".to_owned());
            g[0].replace_range(0..1, "P");
            problems(&grid(&g.iter().map(String::as_str).collect::<Vec<_>>()))
        };
        assert_eq!(face(2, 's'), Vec::<String>::new());
        let p = face(3, 's');
        assert_eq!(p.len(), 1, "{p:?}");
        assert!(
            p[0].contains("3 tiles above the swim surface") && p[0].contains("at most 2"),
            "{p:?}"
        );
        assert_eq!(
            face(3, '~'),
            Vec::<String>::new(),
            "lethal water: 3 is fine"
        );
    }

    #[test]
    fn overhead_blocks_and_faces_outside_the_zone_are_skipped() {
        // A box 3 rows above the deck (2 empty rows under it): overhead.
        let boxes = grid(&[
            "..........#",
            "...........",
            "P..........",
            "##############################",
        ]);
        assert_eq!(problems(&boxes), Vec::<String>::new());
        // 1 empty row under it: Big Resi can't pass, and its top is 4 above.
        let low = grid(&[
            "..........#",
            "..........#",
            "..........#",
            "P..........",
            "##############################",
        ]);
        assert_eq!(problems(&low).len(), 1, "{:?}", problems(&low));
        // A tall wall at column 2: its front (column 1) is never on screen
        // during the scroll.
        let early = grid(&[
            "..#",
            "..#",
            "..#",
            "..#",
            "P.#",
            "##############################",
        ]);
        assert_eq!(problems(&early), Vec::<String>::new());
    }

    #[test]
    fn water_in_front_counts_from_its_surface() {
        // A pier wall 4 rows above the water's surface: too high. The lake
        // bed's step under water is fine.
        let wall = grid(&[
            "..........#",
            "..........#",
            "..........#",
            "P.........#",
            "##~~~~~~~~####################",
            "##~~~~~~~~####################",
        ]);
        let p = problems(&wall);
        assert_eq!(p.len(), 1, "{p:?}");
        assert!(p[0].contains("rows 9–14: its top is 4 tiles"), "{p:?}");
        let step = grid(&[
            "P.............................",
            "##~~~~~~~~~~~~~~~~~~~~~~~~~~~~",
            "##~~~~~~~~~~~~~~~~~~~~~~~~~~~~",
            "##~~~~~~~~#####~~~~~~~~~~~~~~~",
            "##############################",
        ]);
        assert_eq!(problems(&step), Vec::<String>::new());
    }

    #[test]
    fn a_raised_step_in_the_lake_bed_squeezes_a_resting_diver() {
        // Stage 6's former shallows (D-085): the bed rises 2 tiles at
        // column 11 (1-based), wholly under swim water.
        let p = problems(&grid(&[
            "P.............................",
            "#sssssssssssssssssssssssssssss",
            "#sssssssssssssssssssssssssssss",
            "#sssssssss#####sssssssssssssss",
            "#sssssssss#####sssssssssssssss",
            "##############################",
        ]));
        assert_eq!(p.len(), 1, "{p:?}");
        assert!(
            p[0].starts_with("left-facing solid face at column 11, rows 12–13: under water, 2 tiles above the lake bed in front of it (row 14, column 10)"),
            "{p:?}"
        );
        // A 1-tile step is a step too: the push has no step-up.
        let p = problems(&grid(&[
            "P.............................",
            "#sssssssssssssssssssssssssssss",
            "#sssssssssssssssssssssssssssss",
            "#sssssssssssssssssssssssssssss",
            "#sssssssss#####sssssssssssssss",
            "##############################",
        ]));
        assert_eq!(p.len(), 1, "{p:?}");
        assert!(p[0].contains("1 tiles above the lake bed"), "{p:?}");
        // Right of `to + 1` (column 12, 1-based) the edge never pushes: a
        // beach rising to the lead-out quay is fine.
        let beach = grid(&[
            "P.............................",
            "#ssssssssssssss###############",
            "#sssssssssssss################",
            "#ssssssssssss#################",
            "#sssssssssss##################",
            "##############################",
        ]);
        assert_eq!(problems(&beach), Vec::<String>::new());
    }

    #[test]
    fn a_quay_over_a_flat_lake_bed_is_fine() {
        // The quay wall rises from the bed to 2 tiles above the swim
        // surface: the swim-surface rule (D-084) covers it (a resting diver
        // surfaces and climbs out), and the bed in front of it is flat. A
        // one-way deck over the water changes nothing.
        let quay = grid(&[
            "..........###############",
            "P..--.....###############",
            "#sssssssss###############",
            "#sssssssss###############",
            "#sssssssss###############",
            "##############################",
        ]);
        assert_eq!(problems(&quay), Vec::<String>::new());
    }

    #[test]
    fn a_block_under_water_needs_2_rows_of_water_below_it() {
        let block = |gap: usize| {
            let mut g = vec!["P.............................".to_owned()];
            g.push("#sssssssssssssssssssssssssssss".to_owned());
            g.push("#sssssssss###sssssssssssssssss".to_owned());
            g.extend((0..gap).map(|_| "#sssssssssssssssssssssssssssss".to_owned()));
            g.push("##############################".to_owned());
            problems(&grid(&g.iter().map(String::as_str).collect::<Vec<_>>()))
        };
        assert_eq!(block(2), Vec::<String>::new());
        let p = block(1);
        assert_eq!(p.len(), 1, "{p:?}");
        assert!(p[0].contains("2 tiles above the lake bed"), "{p:?}");
    }

    #[test]
    fn the_goal_is_past_the_zone_and_a_zone_checkpoint_on_a_deck() {
        let early_goal = grid(&[
            "P...........................G.",
            "##############################",
        ]);
        // The goal at column index 28 is left of to + 20 = 30.
        let p = problems(&early_goal);
        assert_eq!(p.len(), 1, "{p:?}");
        assert!(
            p[0].contains("goal must be at column >= 31 (to + 20, 1-based"),
            "{p:?}"
        );
        let checkpoints = grid(&["P....C.......C", "#####-#######~################"]);
        let p = problems(&checkpoints);
        assert_eq!(p.len(), 1, "{p:?}");
        assert!(
            p[0].starts_with("checkpoint at row 13, column 14:"),
            "{p:?}"
        );
    }

    #[test]
    fn other_levels_are_not_checked() {
        let text = "[meta]\nid = \"s\"\ntime_limit = 1\n[legend]\n\"#\" = \"solid\"\n\"P\" = { entity = \"player_start\" }\n[map]\ngrid = '''\n.#\n.#\n.#\n.#\nP#\n##\n'''\n";
        assert_eq!(problems(&Level::parse(text).unwrap()), Vec::<String>::new());
    }

    #[test]
    fn the_autoscroll_test_level_is_clean() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let level = crate::content::find_level(&root, "m5_autoscroll").unwrap();
        assert_eq!(problems(&level), Vec::<String>::new());
    }
}
