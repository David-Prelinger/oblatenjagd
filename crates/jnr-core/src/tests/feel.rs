//! Game-feel tests: the "Movement feel" targets of `docs/game-design.md`,
//! measured by simulating scripted inputs with the real `tuning.toml`.
//!
//! Tolerances: speeds ±0.1 px/frame, jump heights ±0.25 tiles (4 px) and
//! never below the target (a jump of "4 tiles" must land on a 4-tile ledge),
//! coyote time and jump buffer exact.

use super::*;
use crate::Fx;

const TILE_PX: i32 = 16;

/// Asserts `measured` (subpixels) is within `tol` (subpixels) of `target`.
fn assert_near(what: &str, measured: Fx, target: Fx, tol: Fx) {
    assert!(
        (measured - target).abs() <= tol,
        "{what}: measured {} subpx ({}.{:03} px), target {} ± {} subpx",
        measured.raw(),
        measured.to_px(),
        (measured.raw() & 255) * 1000 / 256,
        target.raw(),
        tol.raw()
    );
}

/// Runs `pre`, then presses `jump` and holds it until landing (or releases
/// all buttons after one frame if `tap`); returns (rise of the feet in
/// subpixels, airborne frames).
fn jump_rise(width: usize, pre: &[(&str, u32)], jump: &str, tap: bool) -> (Fx, u32) {
    let mut g = Game::new(&flat(width), &tuning(), 1);
    hold(&mut g, "-", 2);
    for &(t, n) in pre {
        hold(&mut g, t, n);
    }
    assert!(g.player().on_ground);
    let ground_y = g.player().y;
    let mut min_y = ground_y;
    let mut frames = 0;
    let first = input(jump);
    let rest = if tap { InputFrame::NONE } else { first };
    let events = g.step(first).to_vec();
    assert!(events.contains(&Event::Jumped), "jump did not start");
    loop {
        frames += 1;
        min_y = min_y.min(g.player().y);
        if g.player().on_ground {
            break;
        }
        g.step(rest);
        assert!(frames < 200, "never landed");
    }
    (ground_y - min_y, frames)
}

fn top_speed(token: &str) -> (Fx, Fx) {
    let mut g = Game::new(&flat(120), &tuning(), 1);
    hold(&mut g, "-", 2);
    hold(&mut g, token, 60);
    let x0 = g.player().x;
    hold(&mut g, token, 10);
    (g.player().vx, (g.player().x - x0).div_int(10))
}

#[test]
fn walk_speed_is_1_5_px_per_frame() {
    let (v, avg) = top_speed("R");
    let target = Fx(384);
    assert_near("walk speed", v, target, Fx(26));
    assert_eq!(avg, v, "moves at top speed every frame");
}

#[test]
fn run_speed_is_2_5_px_per_frame() {
    let (v, avg) = top_speed("RB");
    assert_near("run speed", v, Fx(640), Fx(26));
    assert_eq!(avg, v);
}

#[test]
fn tap_jump_is_1_5_tiles() {
    let (rise, _) = jump_rise(20, &[], "J", true);
    let target = Fx::from_px(TILE_PX * 3 / 2);
    assert_near("tap jump", rise, target, Fx::from_px(4));
}

#[test]
fn held_walking_jump_is_4_tiles() {
    let (rise, _) = jump_rise(60, &[("R", 40)], "RJ", false);
    let target = Fx::from_px(4 * TILE_PX);
    assert!(rise >= target, "must clear a 4-tile ledge: {rise:?}");
    assert_near("walking jump", rise, target, Fx::from_px(4));
    // Standing jumps use the walking jump speed too.
    let (standing, _) = jump_rise(20, &[], "J", false);
    assert!(standing.raw() < rise.raw() + 64 && standing >= target);
}

#[test]
fn held_running_jump_is_5_tiles() {
    let (rise, _) = jump_rise(100, &[("RB", 60)], "RBJ", false);
    let target = Fx::from_px(5 * TILE_PX);
    assert!(rise >= target, "must clear a 5-tile ledge: {rise:?}");
    assert_near("running jump", rise, target, Fx::from_px(4));
}

#[test]
fn max_fall_speed_is_4_px_per_frame() {
    let air = ".".repeat(8);
    let mut rows = vec!["..P....."];
    rows.extend(std::iter::repeat_n(air.as_str(), 13));
    let mut g = Game::new(&level(&rows), &tuning(), 1);
    let mut max_vy = Fx::ZERO;
    for _ in 0..80 {
        if g.step(InputFrame::NONE).contains(&Event::Died) {
            break;
        }
        max_vy = max_vy.max(g.player().vy);
    }
    assert_near("max fall speed", max_vy, Fx(1024), Fx(26));
}

/// Walks off a ledge and presses JUMP `j` steps after the first airborne
/// step (step `j` is the `j`-th step that starts in the air).
fn coyote_jump_after(j: u32) -> bool {
    let air = ".".repeat(30);
    let ledge = format!("{}{}", "#".repeat(8), ".".repeat(22));
    let start = format!("..P{}", ".".repeat(27));
    let mut rows: Vec<&str> = vec![&air; 6];
    rows.push(&start);
    rows.push(&ledge);
    rows.extend(std::iter::repeat_n(air.as_str(), 6));
    let mut g = Game::new(&level(&rows), &tuning(), 1);
    hold(&mut g, "-", 2);
    let mut steps = 0;
    while g.player().on_ground {
        g.step(input("R"));
        steps += 1;
        assert!(steps < 200);
    }
    hold(&mut g, "R", j - 1);
    assert!(!g.player().on_ground);
    g.step(input("RJ")).contains(&Event::Jumped)
}

#[test]
fn coyote_time_is_6_frames() {
    let frames = tuning().player.coyote_frames as u32;
    assert_eq!(frames, 6);
    for j in 1..=frames {
        assert!(
            coyote_jump_after(j),
            "jump {j} frames after the edge must work"
        );
    }
    assert!(!coyote_jump_after(frames + 1), "too late");
}

/// Tap-jumps, then presses JUMP again `d` frames before the first frame
/// that starts on the ground; returns whether a second jump happened.
fn buffered_jump(d: u32) -> bool {
    // First measure when the player lands.
    let mut g = Game::new(&flat(20), &tuning(), 1);
    hold(&mut g, "-", 2);
    g.step(input("J"));
    let mut land = 0;
    while !g.player().on_ground {
        g.step(InputFrame::NONE);
        land += 1;
    }
    // Step `land` (0-based after the jump frame) landed, so step `land + 1`
    // starts on the ground. Press `d` steps earlier and keep holding.
    let mut g = Game::new(&flat(20), &tuning(), 1);
    hold(&mut g, "-", 2);
    g.step(input("J"));
    let press = land + 1 - d;
    hold(&mut g, "-", press - 1);
    let events = hold(&mut g, "J", d + 2);
    events.contains(&Event::Jumped)
}

#[test]
fn jump_buffer_is_6_frames() {
    let frames = tuning().player.jump_buffer_frames as u32;
    assert_eq!(frames, 6);
    for d in 1..=frames {
        assert!(buffered_jump(d), "press {d} frames early must jump");
    }
    assert!(!buffered_jump(frames + 1), "press too early must not jump");
}

#[test]
fn variable_and_speed_dependent_jump_heights() {
    let (tap, tap_air) = jump_rise(20, &[], "J", true);
    let (walk, walk_air) = jump_rise(60, &[("R", 40)], "RJ", false);
    let (run, _) = jump_rise(100, &[("RB", 60)], "RBJ", false);
    assert!(tap < walk && walk < run, "{tap:?} {walk:?} {run:?}");
    assert!(tap_air < walk_air);
}

#[test]
fn skid_turn_and_friction() {
    let t = tuning().player;
    let mut g = Game::new(&flat(120), &tuning(), 1);
    hold(&mut g, "-", 2);
    hold(&mut g, "RB", 60);
    assert_eq!(g.player().vx, t.run_speed);
    g.step(input("L"));
    assert!(g.player().skidding);
    assert_eq!(g.player().vx, t.run_speed - t.skid_decel);
    let mut frames = 1;
    while g.player().vx > Fx::ZERO {
        g.step(input("L"));
        frames += 1;
    }
    assert!(frames <= 17, "skid stop takes {frames} frames");
    hold(&mut g, "L", 30);
    assert!(g.player().vx < Fx::ZERO && !g.player().skidding);
    assert_eq!(g.player().facing, crate::Facing::Left);

    // Releasing all buttons at walk speed stops within 24 frames.
    let mut g = Game::new(&flat(120), &tuning(), 1);
    hold(&mut g, "-", 2);
    hold(&mut g, "R", 40);
    hold(&mut g, "-", 24);
    assert_eq!(g.player().vx, Fx::ZERO);
}

#[test]
fn air_control_steers_but_keeps_momentum() {
    let t = tuning().player;
    let mut g = Game::new(&flat(120), &tuning(), 1);
    hold(&mut g, "-", 2);
    hold(&mut g, "RB", 60);
    g.step(input("RBJ"));
    // No direction held in the air: no drag.
    hold(&mut g, "J", 5);
    assert_eq!(g.player().vx, t.run_speed);
    // Releasing RUN in the air keeps the run speed too.
    hold(&mut g, "RJ", 5);
    assert_eq!(g.player().vx, t.run_speed);
    // Holding against the motion turns with air_accel.
    g.step(input("LJ"));
    assert_eq!(g.player().vx, t.run_speed - t.air_accel);
}

#[test]
fn head_bump_stops_the_jump() {
    // Ceiling at row 8: its bottom edge is 64 px above the ground top (192).
    let air = ".".repeat(20);
    let ceil = "#".repeat(20);
    let start = format!("..P{}", ".".repeat(17));
    let mut rows: Vec<&str> = vec![&air; 8];
    rows.push(&ceil);
    rows.extend([air.as_str(), air.as_str()]);
    rows.push(&start);
    rows.extend([ceil.as_str(), ceil.as_str()]);
    let mut g = Game::new(&level(&rows), &tuning(), 1);
    hold(&mut g, "-", 2);
    g.step(input("J"));
    let mut bumped = false;
    for _ in 0..30 {
        g.step(input("J"));
        if g.player().y == Fx::from_px(9 * 16) {
            bumped = true;
            assert!(g.player().vy >= Fx::ZERO, "upward speed is cancelled");
            break;
        }
        assert!(g.player().y >= Fx::from_px(9 * 16), "inside the ceiling");
    }
    assert!(bumped);
    // Falls back down even though JUMP is still held.
    hold(&mut g, "J", 30);
    assert!(g.player().on_ground);
}

/// Not an assertion: prints the measured numbers
/// (`cargo test -p jnr-core print_measured -- --ignored --nocapture`).
#[test]
#[ignore]
fn print_measured_feel() {
    let px = |f: Fx| format!("{}.{:03}", f.to_px(), (f.raw() & 255) * 1000 / 256);
    let (walk, _) = top_speed("R");
    let (run, _) = top_speed("RB");
    let (tap, tap_air) = jump_rise(20, &[], "J", true);
    let (wj, wj_air) = jump_rise(60, &[("R", 40)], "RJ", false);
    let (rj, rj_air) = jump_rise(100, &[("RB", 60)], "RBJ", false);
    println!("walk speed {} px/f, run speed {} px/f", px(walk), px(run));
    println!("tap jump {} px ({tap_air} f air)", px(tap));
    println!("walking jump {} px ({wj_air} f air)", px(wj));
    println!("running jump {} px ({rj_air} f air)", px(rj));
}
