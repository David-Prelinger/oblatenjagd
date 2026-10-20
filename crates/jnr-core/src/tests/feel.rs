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
    coyote_jump_after_over(j, '.')
}

/// A ledge at row 7 (columns 0–7) followed by `beyond` tiles in the same
/// row (`.` a pit, `~` water flush with the walking surface, `^` spikes).
fn ledge_level(beyond: char) -> Level {
    let air = ".".repeat(30);
    let ledge = format!("{}{}", "#".repeat(8), beyond.to_string().repeat(22));
    let start = format!("..P{}", ".".repeat(27));
    let mut rows: Vec<&str> = vec![&air; 6];
    rows.push(&start);
    rows.push(&ledge);
    rows.extend(std::iter::repeat_n(air.as_str(), 6));
    level(&rows)
}

/// [`coyote_jump_after`] with `beyond` (see [`ledge_level`]) after the
/// ledge.
fn coyote_jump_after_over(j: u32, beyond: char) -> bool {
    let mut g = Game::new(&ledge_level(beyond), &tuning(), 1);
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

#[test]
fn coyote_time_works_over_water() {
    // D-050: water flush with the walking surface kills only once the feet
    // have sunk in, so every coyote frame still jumps.
    let frames = tuning().player.coyote_frames as u32;
    for j in 1..=frames {
        assert!(
            coyote_jump_after_over(j, '~'),
            "jump {j} frames after the water edge must work"
        );
    }
}

/// Walks right off the ledge of [`ledge_level`] until she dies; returns
/// (steps from the first airborne step to the death step, 1 = the same
/// step, feet below the ledge top in subpixels at the death).
fn walk_off_until_death(beyond: char) -> (u32, Fx) {
    let mut g = Game::new(&ledge_level(beyond), &tuning(), 1);
    hold(&mut g, "-", 2);
    let top = Fx::from_px(7 * TILE_PX);
    let mut airborne = 0;
    for _ in 0..300 {
        let died = g.step(input("R")).contains(&Event::Died);
        if !g.player().on_ground || died {
            airborne += 1;
        }
        if died {
            return (airborne, g.player().feet().1 - top);
        }
        if airborne > 0 {
            assert!(
                g.player().feet().1 - top < Fx::from_px(tuning().player.water_sink_px),
                "alive with the feet sunk less than water_sink_px"
            );
        }
    }
    panic!("no death");
}

#[test]
fn water_kills_once_the_feet_have_sunk_in() {
    let sink = Fx::from_px(tuning().player.water_sink_px);
    assert_eq!(sink, Fx::from_px(8));
    let (steps, depth) = walk_off_until_death('~');
    assert!(depth >= sink, "sunk {} subpx", depth.raw());
    assert!(steps > 1, "not on the first airborne step");
    assert_eq!(g_cause('~'), Some(crate::DeathCause::Hazard));
}

#[test]
fn spikes_still_kill_on_touch() {
    // D-050 leaves spikes alone: the first airborne step over spikes flush
    // with the walking surface kills.
    let (steps, depth) = walk_off_until_death('^');
    assert_eq!(steps, 1);
    assert!(depth > Fx::ZERO && depth < Fx::from_px(1));
    assert_eq!(g_cause('^'), Some(crate::DeathCause::Hazard));
}

/// The death cause after walking off the ledge into `beyond`.
fn g_cause(beyond: char) -> Option<crate::DeathCause> {
    let mut g = Game::new(&ledge_level(beyond), &tuning(), 1);
    hold(&mut g, "-", 2);
    until(&mut g, "R", 300, |g| g.death_cause().is_some());
    g.death_cause()
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

/// Horizontal distance (subpixels) of a held jump at full speed from the
/// take-off to the landing on flat ground.
fn jump_reach(token_run: &str, token_jump: &str) -> Fx {
    let mut g = Game::new(&flat(120), &tuning(), 1);
    hold(&mut g, "-", 2);
    hold(&mut g, token_run, 60);
    let x0 = g.player().x;
    assert!(g.step(input(token_jump)).contains(&Event::Jumped));
    for _ in 0..200 {
        if g.step(input(token_jump)).contains(&Event::Landed) {
            return g.player().x - x0;
        }
    }
    panic!("never landed");
}

#[test]
fn walking_jump_reaches_5_tiles() {
    let reach = jump_reach("R", "RJ");
    assert_near(
        "walking jump reach",
        reach,
        Fx::from_px(5 * TILE_PX),
        Fx::from_px(8),
    );
}

/// "~9 tiles" (game-design.md) is the M0 measurement of 157 px airborne
/// distance; this pins it to 9–10 tiles (a ceiling for optional routes).
#[test]
fn running_jump_reaches_9_to_10_tiles() {
    let reach = jump_reach("RB", "RBJ");
    assert!(
        (Fx::from_px(9 * TILE_PX)..=Fx::from_px(10 * TILE_PX)).contains(&reach),
        "running jump reach {} px",
        reach.to_px()
    );
}

/// Falls from high above onto a (standing) Mozartkugel-Verkäufer holding
/// `token`; returns how far the feet rise after the stomp (subpixels).
fn stomp_rise(token: &str) -> Fx {
    let air = "..........";
    let mut rows = vec![air; 14];
    rows[2] = "..P.......";
    rows[11] = "..v.......";
    rows[12] = "##########";
    rows[13] = "##########";
    let mut g = Game::new(&level(&rows), &tuning(), 1);
    for _ in 0..120 {
        if g.step(input(token)).contains(&Event::Stomped) {
            let stomp_feet = g.player().y + g.player().h;
            let mut top = stomp_feet;
            while g.player().vy < Fx::ZERO {
                g.step(input(token));
                top = top.min(g.player().y + g.player().h);
            }
            return stomp_feet - top;
        }
    }
    panic!("no stomp");
}

#[test]
fn stomp_bounce_is_1_tile_without_jump() {
    let rise = stomp_rise("-");
    assert_near("stomp bounce", rise, Fx::from_px(16), Fx::from_px(4));
}

#[test]
fn stomp_bounce_is_3_tiles_with_jump_held() {
    let rise = stomp_rise("J");
    assert!(
        (Fx::from_px(48)..=Fx::from_px(56)).contains(&rise),
        "held stomp bounce: {} px (target 48..56)",
        rise.to_px()
    );
}

#[test]
fn zuckerschock_hitbox_is_12_by_30_and_growing_keeps_the_feet() {
    let t = tuning();
    let mut g = Game::new(&flat(20), &t, 1);
    let feet = g.player().feet();
    assert_eq!(
        (g.player().w, g.player().h),
        (Fx::from_px(12), Fx::from_px(22))
    );
    g.player.set_power(crate::Power::Big, &t.player);
    assert_eq!(
        (g.player().w, g.player().h),
        (Fx::from_px(12), Fx::from_px(30))
    );
    assert_eq!(g.player().feet(), feet);
    assert_eq!(g.player().transform, 30, "transformation freeze");
}

#[test]
fn transformation_freezes_30_frames_and_hurt_gives_120_safe_frames() {
    // A (standing) Verkäufer next to a Zuckerschock player: hurt, freeze,
    // then exactly 120 frames in which touching it does nothing.
    let rows = ["..........", "..P.v.....", "##########"];
    let t = tuning();
    let mut g = Game::new(&level(&rows), &t, 1);
    g.player.set_power(crate::Power::Big, &t.player);
    g.player.transform = 0;
    let mut frames = 0;
    while !g.step(input("R")).contains(&Event::Hurt) {
        frames += 1;
        assert!(frames < 60);
    }
    let mut frozen = 0;
    let x = g.player().x;
    while g.player().transform > 0 {
        g.step(input("R"));
        assert_eq!(g.player().x, x);
        frozen += 1;
    }
    assert_eq!(frozen, 30);
    let mut safe = 0;
    loop {
        let events = g.step(input("-")).to_vec();
        if events.contains(&Event::Died) {
            break;
        }
        safe += 1;
        assert!(safe < 200, "never hurt again");
    }
    assert_eq!(safe, 120, "invulnerable frames");
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
    println!(
        "jump reach walking {} px, running {} px",
        px(jump_reach("R", "RJ")),
        px(jump_reach("RB", "RBJ"))
    );
    println!(
        "stomp bounce {} px, held {} px",
        px(stomp_rise("-")),
        px(stomp_rise("J"))
    );
}

// ------------------------------------------------------- swimming (D-076)

/// Swim-water surface of [`pool`], whole px.
const SURFACE_PX: i32 = 128;

/// A game in a pool level with the player as `power` (no freeze).
fn pool_game(level: &Level, power: crate::Power) -> Game {
    let mut g = Game::new(level, &tuning(), 1);
    g.player.set_power(power, &tuning().player);
    g.player.transform = 0;
    g
}

/// The chest point's y (`swim.chest_px` below the hitbox top).
fn chest_y(g: &Game) -> Fx {
    g.player().y + Fx::from_px(tuning().swim.chest_px)
}

/// Lets a player who starts in or above the water settle: floating at the
/// surface (vy 0, chest in the band) for 5 frames in a row.
fn settle_floating(g: &mut Game) {
    let band = Fx::from_px(SURFACE_PX + tuning().swim.surface_band);
    let mut calm = 0;
    for _ in 0..200 {
        g.step(InputFrame::NONE);
        let p = g.player();
        if p.swimming && p.vy == Fx::ZERO && !p.on_ground && chest_y(g) < band {
            calm += 1;
            if calm == 5 {
                return;
            }
        } else {
            calm = 0;
        }
    }
    panic!("never floated");
}

/// Floats her with the chest exactly on the water line: dives just below
/// the surface band and strokes up into the rise cap.
fn float_on_the_line(g: &mut Game) {
    settle_floating(g);
    let below = Fx::from_px(SURFACE_PX + tuning().swim.surface_band);
    until(g, "D", 100, |g| chest_y(g) >= below);
    assert!(g.step(input("J")).contains(&Event::Stroke));
    until(g, "-", 100, |g| g.player().vy == Fx::ZERO);
    assert_eq!(chest_y(g), Fx::from_px(SURFACE_PX), "capped on the line");
}

#[test]
fn swim_stroke_rises_24_px() {
    // Standing on the lake bed (feet 208), one tap of JUMP: a stroke.
    for power in crate::Power::ALL {
        let mut g = pool_game(&pool(20, 4..16, &[(12, 8, 'S')]), power);
        hold(&mut g, "-", 20);
        let p = g.player();
        assert!(p.on_ground && p.swimming, "{power:?} on the bed");
        let y0 = p.y;
        let events = g.step(input("J")).to_vec();
        assert!(events.contains(&Event::Stroke) && !events.contains(&Event::Jumped));
        let mut min_y = g.player().y;
        for _ in 0..60 {
            g.step(InputFrame::NONE);
            min_y = min_y.min(g.player().y);
        }
        assert_near("stroke rise", y0 - min_y, Fx::from_px(24), Fx::from_px(4));
        assert!(g.player().on_ground, "{power:?} sank back to the bed");
    }
}

#[test]
fn swim_max_sink_is_1_px_per_frame() {
    let mut g = pool_game(&pool(20, 4..16, &[(8, 8, 'S')]), crate::Power::Normal);
    settle_floating(&mut g);
    let y0 = g.player().y;
    let mut max_vy = Fx::ZERO;
    let mut frames_to_12 = None;
    for f in 1..=40 {
        g.step(input("D"));
        max_vy = max_vy.max(g.player().vy);
        if frames_to_12.is_none() && g.player().y - y0 >= Fx::from_px(12) {
            frames_to_12 = Some(f);
        }
    }
    assert_eq!(max_vy, Fx(256), "max sink 1.0 px/frame");
    let y = g.player().y;
    g.step(input("D"));
    assert_eq!(g.player().y - y, Fx(256), "sinks at max speed");
    // From rest: 12 px in about 20 frames (game-design.md "Sinking").
    let f = frames_to_12.unwrap();
    assert!((18..=22).contains(&f), "{f} frames to 12 px");
}

#[test]
fn swim_speed_is_1_25_px_per_frame_and_run_is_no_faster() {
    for token in ["R", "RB"] {
        let mut g = pool_game(&pool(80, 4..76, &[(8, 6, 'S')]), crate::Power::Normal);
        settle_floating(&mut g);
        let y = g.player().y;
        hold(&mut g, token, 60);
        let x0 = g.player().x;
        hold(&mut g, token, 10);
        let p = g.player();
        assert!(p.swimming, "{token}");
        assert_eq!(p.vx, Fx(320), "{token}: 1.25 px/frame");
        assert_eq!((p.x - x0).div_int(10), Fx(320), "{token}: every frame");
        assert_eq!(p.y, y, "{token}: still floating");
    }
}

#[test]
fn swim_floats_at_the_surface_without_input() {
    // Walking off the quay into the water: she floats, no input needed.
    for power in crate::Power::ALL {
        let mut g = pool_game(&pool(30, 6..24, &[]), power);
        until(&mut g, "R", 120, |g| g.player().swimming);
        settle_floating(&mut g);
        let y = g.player().y;
        assert!(
            chest_y(&g) >= Fx::from_px(SURFACE_PX),
            "{power:?}: chest in the water"
        );
        for _ in 0..300 {
            let events = g.step(InputFrame::NONE).to_vec();
            assert!(!events.contains(&Event::Died));
            assert_eq!(g.player().y, y, "{power:?}: floats");
        }
        assert!(g.player().swimming && !g.player().on_ground);
    }
}

/// Floats a player of `power` on the water line at the `S` of `marks`,
/// presses `jump` once and then holds `then` for 120 frames. Returns the
/// feet's y (whole px) if she ended standing on solid ground or a deck,
/// `None` if she ended in the water.
fn jump_out(
    power: crate::Power,
    marks: &[(usize, usize, char)],
    jump: &str,
    then: &str,
) -> Option<i32> {
    let mut g = pool_game(&pool(30, 4..26, marks), power);
    float_on_the_line(&mut g);
    let events = g.step(input(jump)).to_vec();
    assert!(events.contains(&Event::Jumped), "{power:?}: a jump-out");
    assert!(!events.contains(&Event::Stroke));
    hold(&mut g, then, 120);
    let p = g.player();
    if p.swimming {
        return None;
    }
    assert!(p.on_ground, "{power:?}: neither in the water nor standing");
    Some(p.feet().1.to_px())
}

#[test]
fn held_jump_out_lands_on_a_one_tile_deck_in_every_form_and_a_tap_does_not() {
    // A one-way deck 1 tile above the water (row 7, top y 112) over the
    // swimmer: she jumps up through it.
    let deck: Vec<(usize, usize, char)> =
        (8..12).map(|c| (7, c, '-')).chain([(8, 10, 'S')]).collect();
    // A solid quay 1 tile above the water right of her (columns 15+).
    let mut quay: Vec<(usize, usize, char)> = (15..30)
        .flat_map(|c| (7..13).map(move |r| (r, c, '#')))
        .collect();
    quay.push((8, 14, 'S'));
    for power in crate::Power::ALL {
        assert_eq!(
            jump_out(power, &deck, "J", "J"),
            Some(112),
            "{power:?} held, deck"
        );
        assert_eq!(
            jump_out(power, &deck, "J", "-"),
            None,
            "{power:?} tap, deck"
        );
        assert_eq!(
            jump_out(power, &quay, "RJ", "RJ"),
            Some(112),
            "{power:?} held, quay"
        );
        assert_eq!(
            jump_out(power, &quay, "RJ", "R"),
            None,
            "{power:?} tap, quay"
        );
    }
}

/// Feet above the water line at the top of a jump-out from the line
/// (`held`: JUMP held), whole px.
fn jump_out_height(power: crate::Power, held: bool) -> Fx {
    let mut g = pool_game(&pool(20, 4..16, &[(8, 8, 'S')]), power);
    float_on_the_line(&mut g);
    assert!(g.step(input("J")).contains(&Event::Jumped));
    let mut min_feet = g.player().feet().1;
    for _ in 0..60 {
        g.step(input(if held { "J" } else { "-" }));
        min_feet = min_feet.min(g.player().feet().1);
    }
    Fx::from_px(SURFACE_PX) - min_feet
}

#[test]
fn jump_out_heights_bound_the_squeeze_face_over_swim() {
    // D-084: every form's held jump-out clears a face of
    // `swim::MAX_FACE_TILES` above the water line; the weakest one would
    // not clear one tile more.
    let max = crate::swim::MAX_FACE_TILES * TILE_PX;
    let weakest = crate::Power::ALL
        .into_iter()
        .map(|p| jump_out_height(p, true))
        .min()
        .unwrap();
    assert!(weakest >= Fx::from_px(max), "{weakest:?}");
    assert!(
        weakest < Fx::from_px(max + TILE_PX),
        "{weakest:?}: the limit could be higher"
    );
}

#[test]
fn jump_out_heights_match_the_design() {
    // Held, Normal Resi's feet rise ~53 px above the water (big ~45): a
    // deck 1 tile up is in reach; a tap only ~11 (big ~3).
    for (power, held, tap) in [
        (crate::Power::Normal, 53, 11),
        (crate::Power::Big, 45, 3),
        (crate::Power::Hackl, 45, 3),
    ] {
        let name = format!("{power:?}");
        assert_near(
            &name,
            jump_out_height(power, true),
            Fx::from_px(held),
            Fx::from_px(4),
        );
        assert_near(
            &name,
            jump_out_height(power, false),
            Fx::from_px(tap),
            Fx::from_px(4),
        );
    }
}
