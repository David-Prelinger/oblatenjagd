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

/// 40 rows, 40 columns: floor at row 30, a step `height` tiles up from
/// column 8, the player at column 2 on the floor.
fn step_level(height: usize) -> Level {
    let mut rows: Vec<String> = Vec::new();
    for r in 0..40 {
        let row: String = (0..40)
            .map(|c| {
                if r >= 30 || (c >= 8 && r >= 30 - height) {
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
    level_with("climb", &rows)
}

/// A held jump up the step of [`step_level`]; returns the camera y of every
/// frame from the take-off until she lands on the step (the landing frame
/// last).
fn jump_onto_step(g: &mut Game, height: i32) -> Vec<i32> {
    until(g, "R", 60, |g| g.player().x.to_px() >= 5 * 16);
    until(g, "RJ", 60, |g| !g.player().on_ground);
    let surface = (30 - height) * 16;
    let mut ys = vec![g.camera().y];
    while !g.player().on_ground {
        hold(g, "RJ", 1);
        ys.push(g.camera().y);
        assert!(ys.len() < 120, "never landed");
    }
    assert_eq!(feet(g), surface, "landed on the step");
    ys
}

#[test]
fn a_3_tile_step_up_keeps_the_ground_below_on_screen() {
    // D-065: a box she can walk under is 3 tiles high; standing on it the
    // camera keeps the floor's framing.
    let mut g = Game::new(&step_level(3), &tuning(), 1);
    let start_y = g.camera().y;
    assert_eq!(start_y, 480 - 148);
    let ys = jump_onto_step(&mut g, 3);
    hold(&mut g, "-", 120);
    assert!(
        ys.iter().all(|&y| y == start_y),
        "never moves in the air: {ys:?}"
    );
    assert_eq!(g.camera().y, start_y, "stays on the step");
    assert_eq!(480 - g.camera().y, 148, "the floor stays at screen y 148");
    // Back down: still nothing to do.
    until(&mut g, "L", 120, |g| g.player().on_ground && feet(g) == 480);
    hold(&mut g, "-", 30);
    assert_eq!(g.camera().y, start_y);
}

#[test]
fn walking_on_along_higher_ground_settles_on_it() {
    // D-065: the slack keeps the floor's framing only near the step-up;
    // walking on along a 2-tile terrace, the camera settles on it as before
    // (so a long staircase doesn't leave the camera low for good).
    let mut g = Game::new(&step_level(2), &tuning(), 1);
    let start_y = g.camera().y;
    jump_onto_step(&mut g, 2);
    let landed_x = g.player().center_px().0;
    hold(&mut g, "-", 30);
    assert_eq!(g.camera().y, start_y, "held near the step-up");
    until(&mut g, "R", 120, |g| {
        g.player().center_px().0 > landed_x + 48
    });
    hold(&mut g, "-", 30);
    assert_eq!(g.camera().y, 448 - 148, "settled on the terrace");
}

#[test]
fn a_walk_jump_from_a_held_box_leaves_the_camera_still() {
    // D-065, no bob: the up-follow line moves up by the slack's lag, so a
    // held walk jump from the 3-tile box never passes it.
    let mut g = Game::new(&step_level(3), &tuning(), 1);
    let start_y = g.camera().y;
    jump_onto_step(&mut g, 3);
    hold(&mut g, "-", 30);
    until(&mut g, "J", 60, |g| !g.player().on_ground);
    let mut min_top = i32::MAX;
    while !g.player().on_ground {
        min_top = min_top.min(top(&g) - g.camera().y);
        assert_eq!(g.camera().y, start_y, "moved in the air");
        hold(&mut g, "J", 1);
        assert!(g.frame() < 600);
    }
    assert!(min_top < 40, "the apex passes climb_top: {min_top}");
    assert!(min_top >= tuning().camera.climb_top_min, "{min_top}");
    hold(&mut g, "-", 60);
    assert_eq!(g.camera().y, start_y, "never moved");
}

#[test]
fn a_run_jump_from_a_held_box_follows_only_to_climb_top_min() {
    // A wide 3-tile terrace and a long reach, so a run-up keeps the slack:
    // the run jump's apex passes `climb_top_min` (11, so the top stays at
    // least 8 px on screen) and the camera follows only as far as that needs.
    let mut t = tuning();
    t.camera.climb_up_reach = 640;
    let mut g = Game::new(&step_level(3), &t, 1);
    let start_y = g.camera().y;
    jump_onto_step(&mut g, 3);
    hold(&mut g, "-", 30);
    until(&mut g, "RB", 120, |g| g.player().center_px().0 >= 20 * 16);
    assert_eq!(g.camera().y, start_y, "held during the run-up");
    until(&mut g, "RBJ", 60, |g| !g.player().on_ground);
    let (mut tops, mut highest) = (Vec::new(), start_y);
    while !g.player().on_ground {
        tops.push(top(&g) - g.camera().y);
        highest = highest.min(g.camera().y);
        hold(&mut g, "RBJ", 1);
        assert!(tops.len() < 120);
    }
    let min_top = *tops.iter().min().unwrap();
    assert_eq!(
        min_top, t.camera.climb_top_min,
        "the top at screen y: {tops:?}"
    );
    assert!(min_top >= 8);
    assert!(highest < start_y, "it had to follow");
    hold(&mut g, "-", 60);
    assert_eq!(g.camera().y, start_y, "settled back on the held framing");
}

#[test]
fn a_4_tile_step_up_settles_by_16_px_at_2_px_per_frame() {
    let mut g = Game::new(&step_level(4), &tuning(), 1);
    let start_y = g.camera().y;
    let ys = jump_onto_step(&mut g, 4);
    let (landing, air) = ys.split_last().unwrap();
    assert!(air.iter().all(|&y| y == start_y), "in the air: {ys:?}");
    assert_eq!(*landing, start_y - 2, "the landing frame settles 2 px");
    // Grounded on the step: 2 px per frame up to the excess over the slack.
    let target = start_y - 16;
    let mut prev = g.camera().y;
    while g.camera().y != target {
        hold(&mut g, "-", 1);
        assert_eq!(prev - g.camera().y, 2.min(prev - target));
        prev = g.camera().y;
        assert!(g.frame() < 400);
    }
    hold(&mut g, "-", 10);
    assert_eq!(g.camera().y, target, "stays at the target");
}

#[test]
fn a_shaft_of_planks_3_tiles_apart_keeps_resi_on_screen() {
    // Floor at row 38, one-way planks every 3 tiles up to row 14. With the
    // slack (D-065) the first plank keeps the framing and every further one
    // moves it by 3 tiles. In the air (the slack lowers the up-follow line)
    // her top stays at least `climb_top_min` on screen; landed, the
    // catch-up brings it below `climb_top` within 4 frames.
    let planks = [35, 32, 29, 26, 23, 20, 17, 14];
    let mut rows: Vec<String> = Vec::new();
    for r in 0..40 {
        let row: String = (0..20)
            .map(|c| {
                if r >= 38 {
                    '#'
                } else if planks.contains(&r) {
                    '-'
                } else if c == 2 && r == 37 {
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
    let t = tuning().camera;
    assert_eq!(g.camera().y, 608 - 148);
    for (i, &plank) in planks.iter().enumerate() {
        let surface = plank * 16;
        until(&mut g, "J", 120, |g| {
            let screen_top = top(g) - g.camera().y;
            assert!(
                screen_top >= t.climb_top_min,
                "plank {plank}: top {screen_top}"
            );
            assert!(
                feet(g) <= g.camera().y + SCREEN_H,
                "plank {plank}: off screen"
            );
            g.player().on_ground && feet(g) == surface
        });
        for frame in 0..60 {
            hold(&mut g, "-", 1);
            let line = if frame < 4 {
                t.climb_top_min
            } else {
                t.climb_top
            };
            assert!(
                top(&g) - g.camera().y >= line,
                "plank {plank}, frame {frame}: top at screen y {}",
                top(&g) - g.camera().y
            );
        }
        let want = if i == 0 { 608 } else { surface + 48 };
        assert_eq!(g.camera().y, want - 148, "plank {plank}");
    }
}

#[test]
fn a_cleft_beside_a_box_still_kills_at_the_death_line() {
    // Floor at row 30 (columns 0..12), a 3-tile box on it at columns 8..12,
    // then bottomless. She jumps onto the box (the camera keeps the floor's
    // framing, D-065) and walks off its far side.
    let mut rows: Vec<String> = Vec::new();
    for r in 0..40 {
        let row: String = (0..40)
            .map(|c| {
                if c < 12 && (r >= 30 || (c >= 8 && r >= 27)) {
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
    jump_onto_step(&mut g, 3);
    assert_eq!(g.camera().y, 480 - 148);
    let events = walk_off(&mut g);
    assert!(events.contains(&Event::Died), "{events:?}");
    assert_eq!(g.death_cause(), Some(DeathCause::Pit));
    let cam = g.camera().y;
    // The fall-follow limit is measured from the box (her last ground),
    // as without the slack, so the death line is where it always was.
    assert_eq!(cam, 432 - 148 + 80, "the box's framing + the fall limit");
    assert!(top(&g) >= cam + SCREEN_H && top(&g) < cam + SCREEN_H + 4);
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
        let d = g.camera().y - prev;
        assert!((0..=2).contains(&d), "down only, at most 2 px/frame: {d}");
        prev = g.camera().y;
    }
    assert_eq!(g.camera().y, 30 * 16 - 164, "pole base at screen y 164");
}

/// The camera y of every frame of Slide, Hop and Walk, after touching
/// the goal on a flat climb level (floor row 30, goal column 20) with a
/// jump pressed `delay` frames after setting off (`None`: walking in).
fn goal_camera_track(delay: Option<u32>) -> (Vec<i32>, Vec<i32>) {
    let mut rows: Vec<String> = Vec::new();
    for r in 0..40 {
        let row: String = (0..40)
            .map(|c| match (r, c) {
                (30.., _) => '#',
                (29, 2) => 'P',
                (29, 20) => 'G',
                _ => '.',
            })
            .collect();
        rows.push(row);
    }
    let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
    let mut g = Game::new(&level_with("climb", &rows), &tuning(), 1);
    match delay {
        Some(d) => {
            hold(&mut g, "R", d);
            until(&mut g, "RJ", 300, |g| g.mode() == Mode::StageClear);
        }
        None => {
            until(&mut g, "R", 300, |g| g.mode() == Mode::StageClear);
        }
    }
    let (mut ys, mut feet_on_screen) = (vec![g.camera().y], Vec::new());
    while g.stage_clear().is_some_and(|c| {
        matches!(
            c.phase,
            ClearPhase::Slide | ClearPhase::Hop | ClearPhase::Walk
        )
    }) {
        hold(&mut g, "-", 1);
        ys.push(g.camera().y);
        feet_on_screen.push(feet(&g) - g.camera().y);
    }
    (ys, feet_on_screen)
}

#[test]
fn the_climb_goal_camera_moves_one_way_and_ignores_the_hop() {
    // QA (E19): with the touch framed at feet 148 the camera went up while
    // she slid down, then reversed, then followed the 5 px hop.
    // On flat ground the touch is always framed at feet 148 or higher on
    // screen, so the camera goes up 16 px; coming down is covered by
    // `the_goal_sequence_settles_the_feet_at_164`.
    for delay in [
        None,
        Some(0),
        Some(40),
        Some(80),
        Some(100),
        Some(110),
        Some(120),
    ] {
        let (ys, feet_on_screen) = goal_camera_track(delay);
        let steps: Vec<i32> = ys.windows(2).map(|w| w[1] - w[0]).collect();
        assert!(
            steps.iter().all(|&d| d >= 0) || steps.iter().all(|&d| d <= 0),
            "{delay:?}: one way only, got {ys:?}"
        );
        assert!(
            steps.iter().all(|d| d.abs() <= 2),
            "{delay:?}: calm, {ys:?}"
        );
        assert_eq!(*ys.last().unwrap(), 30 * 16 - 164, "{delay:?}: base at 164");
        assert!(
            feet_on_screen.iter().all(|&f| f <= 164),
            "{delay:?}: her feet never sink below the final framing"
        );
        assert!(steps.iter().any(|&d| d < 0), "{delay:?}: up to 164");
    }
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

#[test]
fn falkenstein_h_box_top_keeps_the_axe_lane_on_screen() {
    // The original E20 bug (backlog "Falkenstein H-box framing"): standing
    // on the H-box (col 164, row 8) the lane (row 12) was below the screen.
    // `falkenstein_hackl_axes` stands there for ~400 frames throwing axes.
    let level = Level::parse(include_str!(
        "../../../../content/levels/04_falkenstein.level"
    ))
    .unwrap();
    let replay = crate::Replay::parse(include_str!(
        "../../../../tests/replays/falkenstein_hackl_axes.replay"
    ))
    .unwrap();
    let mut g = replay.new_game(&[level], &tuning()).unwrap();
    let mut on_box = 0;
    for i in replay.frames() {
        g.step(i);
        let col = g.player().center_px().0.div_euclid(16);
        if g.player().on_ground && feet(&g) == 8 * 16 && col == 164 {
            on_box += 1;
            let lane_bottom = 13 * 16;
            assert!(
                lane_bottom <= g.camera().y + SCREEN_H,
                "frame {}: camera y {} hides row 12",
                g.frame(),
                g.camera().y
            );
        }
    }
    assert!(on_box > 300, "stood on the box for {on_box} frames");
}
