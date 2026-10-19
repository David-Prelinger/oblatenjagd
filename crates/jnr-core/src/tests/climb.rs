//! The climb camera (D-054, M3 task E14): start and respawn framing, the
//! ground settle, the fall-follow limit and the death line, the goal
//! framing and the vertical activation rule. The camera's own arithmetic
//! is unit-tested in `camera.rs`; side levels are covered by every M1/M2
//! replay and golden, which must not change.

use super::*;
use crate::{CameraMode, ClearPhase, DeathCause, Mode, Power, SCREEN_H};

/// A level from grid rows with `camera = "<camera>"`: `#` solid, `-`
/// one-way, `P` start, `G` goal, `C` checkpoint, `o` Oblate.
fn level_with(camera: &str, rows: &[&str]) -> Level {
    let grid = rows.join("\n");
    let text = format!(
        "[meta]\nid = \"climb\"\ntime_limit = 300\ncamera = \"{camera}\"\n[legend]\n\"#\" = \"solid\"\n\"-\" = \"oneway\"\n\"P\" = {{ entity = \"player_start\" }}\n\"G\" = {{ entity = \"goal\" }}\n\"C\" = {{ entity = \"checkpoint\" }}\n\"o\" = {{ entity = \"oblate\" }}\n[map]\ngrid = '''\n{grid}\n'''\n"
    );
    Level::parse(&text).unwrap()
}

/// 40 rows, 40 columns: a ledge (surface row `ledge`) over columns 0..`edge`,
/// then open air down to a floor `drop` tiles lower (`None`: bottomless),
/// and the player at column 2 on the ledge.
fn drop_level(camera: &str, ledge: usize, edge: usize, drop: Option<usize>) -> Level {
    let mut rows: Vec<String> = Vec::new();
    for r in 0..40 {
        let row: String = (0..40)
            .map(|c| {
                let floor = drop.is_some_and(|d| r >= ledge + d);
                if (c < edge && r >= ledge) || floor {
                    '#'
                } else if c == 2 && r + 1 == ledge {
                    'P'
                } else {
                    '.'
                }
            })
            .collect();
        rows.push(row);
    }
    let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
    level_with(camera, &rows)
}

fn feet(g: &Game) -> i32 {
    (g.player().y + g.player().h).to_px()
}

fn top(g: &Game) -> i32 {
    g.player().y.to_px()
}

/// Walks right until airborne, then keeps walking until landed or dead.
fn walk_off(g: &mut Game) -> Vec<Event> {
    let mut events = until(g, "R", 300, |g| !g.player().on_ground);
    events.extend(until(g, "R", 300, |g| {
        g.player().on_ground || g.mode() != Mode::Playing
    }));
    events
}

#[test]
fn a_climb_level_starts_with_the_feet_at_148() {
    let g = Game::new(&drop_level("climb", 20, 40, None), &tuning(), 1);
    assert_eq!(g.level().meta.camera, CameraMode::Climb);
    assert_eq!(feet(&g), 320);
    assert_eq!(g.camera().y, 320 - 148);
    // The same level with the side camera: the dead zone's middle.
    let side = Game::new(&drop_level("side", 20, 40, None), &tuning(), 1);
    let t = tuning().camera;
    let centre = g.player().center_px().1;
    assert_eq!(
        side.camera().y,
        centre - (t.deadzone_top + t.deadzone_bottom) / 2
    );
}

#[test]
fn the_camera_settles_on_a_higher_ledge_at_2_px_per_frame() {
    // Floor at row 30, a 3-tile step up from column 4.
    let mut rows: Vec<String> = Vec::new();
    for r in 0..40 {
        let row: String = (0..40)
            .map(|c| {
                if r >= 30 || (c >= 4 && r >= 27) {
                    '#'
                } else if c == 2 && r == 29 {
                    'P'
                } else {
                    '.'
                }
            })
            .collect();
        rows.push(row);
    }
    let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
    let mut g = Game::new(&level_with("climb", &rows), &tuning(), 1);
    let start_y = g.camera().y;
    assert_eq!(start_y, 480 - 148);
    // A held jump up the step: the camera holds still in the air.
    until(&mut g, "RJ", 60, |g| !g.player().on_ground);
    let mut ys = vec![g.camera().y];
    for _ in 0..24 {
        hold(&mut g, "RJ", 1);
        ys.push(g.camera().y);
    }
    until(&mut g, "R", 120, |g| {
        g.player().on_ground && feet(g) == 27 * 16
    });
    ys.push(g.camera().y);
    assert!(
        ys.iter().all(|&y| (start_y - 2..=start_y).contains(&y)),
        "still in the air: {ys:?}"
    );
    // Grounded on the step: 2 px per frame up to feet − 148.
    let mut prev = g.camera().y;
    while g.camera().y != 432 - 148 {
        hold(&mut g, "-", 1);
        assert_eq!(prev - g.camera().y, 2.min(prev - (432 - 148)));
        prev = g.camera().y;
        assert!(g.frame() < 400);
    }
    hold(&mut g, "-", 10);
    assert_eq!(g.camera().y, 432 - 148, "stays at the target");
}

#[test]
fn a_6_tile_drop_survives_on_screen() {
    let mut g = Game::new(&drop_level("climb", 20, 6, Some(6)), &tuning(), 1);
    let events = walk_off(&mut g);
    assert!(!events.contains(&Event::Died), "{events:?}");
    assert_eq!(feet(&g), 26 * 16);
    let screen_feet = feet(&g) - g.camera().y;
    assert!(screen_feet <= 164, "feet at screen y {screen_feet}");
    // The fall-follow limit: 80 px below the ledge's framing, plus the
    // first 2 px settle step in the landing frame.
    assert_eq!(g.camera().y, 320 - 148 + 80 + 2);
    // Then it settles on the new ground.
    hold(&mut g, "-", 60);
    assert_eq!(g.camera().y, 26 * 16 - 148);
}

#[test]
fn a_10_tile_drop_dies_at_the_death_line_before_landing() {
    for power in [Power::Normal, Power::Big] {
        let mut g = Game::new(&drop_level("climb", 20, 6, Some(10)), &tuning(), 1);
        if power == Power::Big {
            g.player.set_power(Power::Big, &tuning().player);
            g.player.transform = 0;
        }
        let events = walk_off(&mut g);
        assert!(events.contains(&Event::Died), "{power:?}: {events:?}");
        assert_eq!(g.death_cause(), Some(DeathCause::Pit));
        // Died with the hitbox top just below the screen (its whole px may
        // still be the screen bottom row's 180), in the air above the floor
        // 10 tiles down.
        let cam = g.camera().y;
        assert!(
            top(&g) >= cam + SCREEN_H && top(&g) < cam + SCREEN_H + 4,
            "{power:?}: top {} camera {cam}",
            top(&g)
        );
        assert!(feet(&g) < 30 * 16, "{power:?}: died before landing");
        assert_eq!(cam, 320 - 148 + 80, "at the fall-follow limit");
    }
}

#[test]
fn a_cleft_kills_long_before_the_level_bottom() {
    let mut g = Game::new(&drop_level("climb", 10, 6, None), &tuning(), 1);
    let left_at = {
        until(&mut g, "R", 300, |g| !g.player().on_ground);
        g.frame()
    };
    let events = until(&mut g, "-", 300, |g| g.mode() != Mode::Playing);
    assert!(events.contains(&Event::Died));
    assert_eq!(g.death_cause(), Some(DeathCause::Pit));
    let fell = g.frame() - left_at;
    assert!(fell <= 45, "died {fell} frames after leaving the ledge");
    assert!(feet(&g) < 30 * 16, "far above the level bottom (640)");
}

#[test]
fn side_levels_have_no_death_line() {
    // The 10-tile drop in a side level: she lands.
    let mut g = Game::new(&drop_level("side", 20, 6, Some(10)), &tuning(), 1);
    let events = walk_off(&mut g);
    assert!(!events.contains(&Event::Died), "{events:?}");
    assert_eq!(feet(&g), 30 * 16);
}

#[test]
fn respawn_frames_the_checkpoint_at_the_settle_target() {
    // A ledge (surface row 20) with a checkpoint at column 6, then a
    // bottomless cleft from column 10.
    let mut rows: Vec<String> = Vec::new();
    for r in 0..40 {
        let row: String = (0..40)
            .map(|c| {
                if c < 10 && r >= 20 {
                    '#'
                } else if c == 2 && r == 19 {
                    'P'
                } else if c == 6 && r == 19 {
                    'C'
                } else {
                    '.'
                }
            })
            .collect();
        rows.push(row);
    }
    let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
    let mut g = Game::new(&level_with("climb", &rows), &tuning(), 1);
    let events = until(&mut g, "R", 400, |g| g.mode() != Mode::Playing);
    assert!(events.contains(&Event::CheckpointReached));
    assert!(events.contains(&Event::Died));
    // While dying the camera stays at the fall-follow limit ...
    assert_eq!(g.camera().y, 320 - 148 + 80);
    // ... and the respawn frames the checkpoint at once.
    until(&mut g, "-", 120, |g| g.mode() == Mode::Playing);
    assert_eq!(feet(&g), 20 * 16, "at the checkpoint");
    assert_eq!(g.player().center_px().0, 6 * 16 + 8);
    assert_eq!(g.camera().y, 20 * 16 - 148, "no settling animation");
    hold(&mut g, "-", 30);
    assert_eq!(g.camera().y, 20 * 16 - 148);
}

#[test]
fn the_goal_sequence_settles_the_feet_at_164() {
    // Goal on a floor at row 30; a step 4 tiles higher before it, so the
    // touch is high and the camera has to come down.
    let mut rows: Vec<String> = Vec::new();
    for r in 0..40 {
        let row: String = (0..40)
            .map(|c| {
                if r >= 30 || (c <= 9 && r >= 26) {
                    '#'
                } else if c == 2 && r == 25 {
                    'P'
                } else if c == 12 && r == 29 {
                    'G'
                } else {
                    '.'
                }
            })
            .collect();
        rows.push(row);
    }
    let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
    let mut g = Game::new(&level_with("climb", &rows), &tuning(), 1);
    until(&mut g, "RJ", 200, |g| g.mode() == Mode::StageClear);
    let x = g.camera().x;
    let mut prev = g.camera().y;
    while g.stage_clear().is_some_and(|c| {
        matches!(
            c.phase,
            ClearPhase::Slide | ClearPhase::Hop | ClearPhase::Walk
        )
    }) {
        hold(&mut g, "-", 1);
        assert_eq!(g.camera().x, x, "x fixed (D-040)");
        assert!((g.camera().y - prev).abs() <= 2, "settles at 2 px/frame");
        prev = g.camera().y;
    }
    assert_eq!(g.camera().y, 30 * 16 - 164, "pole base at screen y 164");
}

#[test]
fn climb_levels_activate_entities_only_near_the_screen_vertically() {
    // Player on the floor (row 38); Oblaten straight above at rows 2 and 20.
    let mut rows = vec!["........".to_owned(); 40];
    rows[2] = "...o....".to_owned();
    rows[20] = "...o....".to_owned();
    rows[38] = "..P.....".to_owned();
    rows[39] = "########".to_owned();
    let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
    let active = |camera: &str| {
        let mut g = Game::new(&level_with(camera, &rows), &tuning(), 1);
        hold(&mut g, "-", 1);
        g.entities()
            .iter()
            .filter(|e| e.kind == crate::EntityKind::Oblate)
            .map(|e| e.active)
            .collect::<Vec<_>>()
    };
    // Camera 624 − 148 = 476, clamped to the level bottom 640 − 180 = 460:
    // rows 2 and 20 end above 460 − 64 = 396.
    assert_eq!(active("side"), [true, true], "side: horizontal rule only");
    assert_eq!(active("climb"), [false, false]);
    // Closer: a ledge whose cell bottom is just inside the margin.
    let mut rows2: Vec<String> = rows.iter().map(|s| (*s).to_owned()).collect();
    rows2[20] = "........".to_owned();
    rows2[23] = "...o....".to_owned(); // 368..384: bottom 384 < 396, out
    rows2[24] = "...o....".to_owned(); // 384..400: bottom 400 >= 396, in
    let rows2: Vec<&str> = rows2.iter().map(String::as_str).collect();
    let mut g = Game::new(&level_with("climb", &rows2), &tuning(), 1);
    assert_eq!(g.camera().y, 460);
    hold(&mut g, "-", 1);
    let oblaten: Vec<bool> = g
        .entities()
        .iter()
        .filter(|e| e.kind == crate::EntityKind::Oblate)
        .map(|e| e.active)
        .collect();
    assert_eq!(oblaten, [false, false, true]);
    // As the camera never moves here, the far ones stay inactive.
    hold(&mut g, "-", 60);
    assert!(!g.entities()[0].active);
}
