//! Strobl's Fensterläden and the stage clock (D-094, game-design.md →
//! "Stage 7: Strobl", "New mechanics"): a stationary one-way platform that
//! is there only while open, on a cycle f = (stage clock + `phase`) mod
//! `period`; the stage clock counts the frames the ZEIT timer counts since
//! the stage start or the last respawn.

use super::*;
use crate::platform::{
    ShutterLook, recovery_problems, shutter, shutter_frame, shutter_look, shutter_look_at,
    track_problems,
};
use crate::{DeathCause, Entity, EntityKind, Fx, Mode, Start};

/// A 14-row test world 40 columns wide: ground (`#`) in rows 12–13 (top
/// y 192), the player start at column 2, a spike (`^`) at column 30, the
/// goal at column 38. `marks` are (row, col, char): `f`/`g`/`h` are
/// Fensterläden with the legend fields `f`, `g`, `h`.
fn world(f: &str, g: &str, h: &str, marks: &[(usize, usize, char)]) -> Level {
    let mut rows: Vec<Vec<char>> = (0..14)
        .map(|r| (0..40).map(|_| if r >= 12 { '#' } else { '.' }).collect())
        .collect();
    rows[11][2] = 'P';
    rows[12][30] = '^';
    rows[11][38] = 'G';
    for &(r, c, ch) in marks {
        rows[r][c] = ch;
    }
    let grid: Vec<String> = rows.into_iter().map(|r| r.into_iter().collect()).collect();
    let text = format!(
        "[meta]\nid = \"shutters\"\ntime_limit = 300\n[legend]\n\"#\" = \"solid\"\n\"^\" = \"hazard\"\n\
         \"P\" = {{ entity = \"player_start\" }}\n\"G\" = {{ entity = \"goal\" }}\n\
         \"f\" = {{ entity = \"fensterladen\"{f} }}\n\"g\" = {{ entity = \"fensterladen\"{g} }}\n\
         \"h\" = {{ entity = \"fensterladen\"{h} }}\n[map]\ngrid = '''\n{}\n'''\n",
        grid.join("\n")
    );
    Level::parse(&text).unwrap()
}

/// [`world`] with one Fensterladen `f` (legend fields `fields`) in cell
/// (6, `row`): body x 96..128, top y `row` × 16.
fn one(fields: &str, row: usize) -> Level {
    world(fields, "", "", &[(row, 6, 'f')])
}

fn shutters(g: &Game) -> Vec<Entity> {
    g.entities()
        .iter()
        .filter(|e| e.kind == EntityKind::Fensterladen)
        .copied()
        .collect()
}

fn the_shutter(g: &Game) -> Entity {
    shutters(g)[0]
}

/// Puts the player standing on `e`'s top, `dx` px right of its left edge.
fn stand_on(g: &mut Game, e: Entity, dx: i32) {
    g.player.x = e.x + Fx::from_px(dx);
    g.player.y = e.y - g.player.h;
    g.player.vx = Fx::ZERO;
    g.player.vy = Fx::ZERO;
    g.player.on_ground = true;
}

/// Puts the player in the air with her feet at `feet` px, at `x` px, at
/// rest.
fn hang(g: &mut Game, x: i32, feet: i32) {
    g.player.x = Fx::from_px(x);
    g.player.y = Fx::from_px(feet) - g.player.h;
    g.player.vx = Fx::ZERO;
    g.player.vy = Fx::ZERO;
    g.player.on_ground = false;
}

fn feet(g: &Game) -> Fx {
    g.player().y + g.player().h
}

#[test]
fn a_shutter_is_a_32x8_body_left_aligned_with_its_top_on_its_cell_top() {
    let g = Game::new(&one("", 9), &tuning(), 1);
    let e = the_shutter(&g);
    assert_eq!((e.x, e.y), (Fx::from_px(96), Fx::from_px(144)));
    assert_eq!((e.w, e.h), (Fx::from_px(32), Fx::from_px(8)));
    // Defaults (D-094): period 240, open 150, phase 0 → open at clock 0.
    let s = &g.level().entities[usize::from(e.spawn.unwrap())];
    assert_eq!((s.period, s.open, s.phase), (240, 150, 0));
    assert_eq!((e.state, e.count), (shutter::OPEN, 0));
}

/// open → rattle → closed → warn over full periods for a few
/// `period`/`open`/`phase` values: f = (clock + phase) mod period, open
/// for f < open, the last 30 open frames rattle, the last 20 closed frames
/// warn; the same every cycle (no drift).
#[test]
fn the_cycle_runs_open_rattle_closed_warn_on_the_clock_without_drift() {
    let t = tuning();
    for (period, open, phase) in [
        (240u16, 150u16, 0u16),
        (240, 150, 60),
        (120, 60, 119),
        (480, 450, 7),
    ] {
        let lvl = one(
            &format!(", period = {period}, open = {open}, phase = {phase}"),
            9,
        );
        let spawn = lvl
            .entities
            .iter()
            .find(|s| s.kind == crate::SpawnKind::Fensterladen)
            .unwrap()
            .clone();
        for clock in 0..(5 * u32::from(period)) {
            let f = shutter_frame(&spawn, clock);
            assert_eq!(u32::from(f), (clock + u32::from(phase)) % u32::from(period));
            let want = if f < open - 30 {
                ShutterLook::Open
            } else if f < open {
                ShutterLook::Rattle
            } else if f < period - 20 {
                ShutterLook::Closed
            } else {
                ShutterLook::Warn
            };
            assert_eq!(shutter_look_at(f, period, open, &t), want, "f {f}");
        }
        // In a game: the entity follows the clock frame by frame.
        let mut g = Game::new(&lvl, &t, 1);
        for _ in 0..(2 * u32::from(period) + 17) {
            let e = the_shutter(&g);
            let f = shutter_frame(&spawn, g.stage_clock());
            assert_eq!(e.count, f);
            assert_eq!(e.state == shutter::OPEN, f < open);
            assert_eq!(
                shutter_look(&e, g.level(), &t),
                Some(shutter_look_at(f, period, open, &t))
            );
            g.step(input("-"));
        }
    }
}

/// The stage clock counts the frames the ZEIT timer counts: frames of play
/// only, not frozen (a transformation), paused, dying or goal-sequence
/// frames; a respawn restarts it at 0 (and the shutters with it); it is in
/// `state_hash`.
#[test]
fn the_stage_clock_counts_play_frames_only_and_restarts_on_respawn() {
    let t = tuning();
    let lvl = one(", period = 120, open = 60, phase = 10", 9);
    let mut g = Game::new_at(&lvl, &t, 1, Start::Title);
    hold(&mut g, "-", 30);
    assert_eq!(g.stage_clock(), 0, "title");
    g.step(input("S"));
    hold(&mut g, "-", u32::from(t.game.intro_frames));
    assert_eq!(g.mode(), Mode::Playing);
    assert_eq!(g.stage_clock(), 0, "the intro doesn't count");
    hold(&mut g, "-", 50);
    assert_eq!(g.stage_clock(), 50);
    assert_eq!(the_shutter(&g).count, 60);
    // Paused: START, 40 frames, START.
    g.step(input("S"));
    assert_eq!(g.mode(), Mode::Paused);
    hold(&mut g, "-", 40);
    g.step(input("S"));
    assert_eq!(g.mode(), Mode::Playing);
    assert_eq!(g.stage_clock(), 50, "paused frames don't count");
    assert_eq!(the_shutter(&g).count, 60);
    // Frozen by a transformation.
    g.player.transform = 30;
    hold(&mut g, "-", 30);
    assert_eq!(g.stage_clock(), 50, "frozen frames don't count");
    hold(&mut g, "-", 1);
    assert_eq!(g.stage_clock(), 51);
    // One more second of play is one second off the ZEIT timer.
    let time = g.time_left();
    hold(&mut g, "-", 60);
    assert_eq!((g.stage_clock(), g.time_left()), (111, time - 1));
    // Dying: onto the spike.
    hang(&mut g, 30 * 16 + 2, 196);
    let events = hold(&mut g, "-", 1);
    assert!(events.contains(&Event::Died), "{events:?}");
    assert_eq!(g.death_cause(), Some(DeathCause::Hazard));
    let at_death = g.stage_clock();
    hold(&mut g, "-", u32::from(t.game.death_frames) - 1);
    assert!(matches!(g.mode(), Mode::Dying { .. }));
    assert_eq!(g.stage_clock(), at_death, "dying frames don't count");
    // The respawn restarts it (and the shutters are back at their phase).
    let events = hold(&mut g, "-", 1);
    assert!(events.contains(&Event::Respawned));
    assert_eq!(g.stage_clock(), 0);
    assert_eq!(the_shutter(&g).count, 10);
    hold(&mut g, "-", u32::from(t.game.intro_frames));
    assert_eq!(g.stage_clock(), 0, "the intro after a death doesn't count");
    hold(&mut g, "-", 5);
    assert_eq!(g.stage_clock(), 5);
    // In the state hash: the same game with another clock hashes apart.
    let mut other = g.clone();
    other.stage_clock += 1;
    assert_ne!(g.state_hash(), other.state_hash());
}

#[test]
fn the_goal_sequence_stops_the_stage_clock() {
    let mut g = Game::new(&one("", 9), &tuning(), 1);
    hold(&mut g, "-", 10);
    g.player.x = Fx::from_px(34 * 16);
    until(&mut g, "R", 600, |g| g.mode() == Mode::StageClear);
    let at_goal = g.stage_clock();
    hold(&mut g, "-", 60);
    assert_eq!(g.stage_clock(), at_goal);
}

/// Falling onto an open shutter lands on it; onto a closed one falls
/// through to the ground.
#[test]
fn only_an_open_shutter_catches_a_fall() {
    let t = tuning();
    // Open at clock 0 (phase 0) and closed (phase 150: f = 150).
    for (phase, lands) in [(0, true), (150, false)] {
        let mut g = Game::new(&one(&format!(", phase = {phase}"), 9), &t, 1);
        hang(&mut g, 100, 120);
        let events = until(&mut g, "-", 120, |g| g.player().on_ground);
        assert!(events.contains(&Event::Landed));
        let want = if lands { 144 } else { 192 };
        assert_eq!(feet(&g), Fx::from_px(want), "phase {phase}");
        // Standing on it while it stays open: no drift, still grounded.
        hold(&mut g, "-", 60);
        assert_eq!(feet(&g), Fx::from_px(want));
        assert!(g.player().on_ground);
    }
}

/// From below, an open shutter is a one-way top: a jump passes through it
/// and lands on it on the way down.
#[test]
fn a_jump_from_below_passes_through_an_open_shutter_and_lands_on_it() {
    let mut g = Game::new(&one("", 9), &tuning(), 1);
    g.player.x = Fx::from_px(106);
    hold(&mut g, "-", 2);
    assert_eq!(feet(&g), Fx::from_px(192));
    assert_eq!(the_shutter(&g).state, shutter::OPEN);
    let events = hold(&mut g, "J", 20);
    assert!(events.contains(&Event::Jumped));
    assert!(feet(&g) < Fx::from_px(144), "above the shutter");
    until(&mut g, "-", 60, |g| g.player().on_ground);
    assert_eq!(feet(&g), Fx::from_px(144), "on the shutter");
    assert!(g.stage_clock() < 150, "still open");
}

/// A closing shutter drops its rider: on the frame it closes she is where
/// she was (nothing pushes her), then she falls; her coyote frames still
/// give a jump.
#[test]
fn a_closing_shutter_drops_its_rider_with_coyote_frames() {
    let t = tuning();
    let lvl = one(", period = 240, open = 150", 9);
    let mut g = Game::new(&lvl, &t, 1);
    hold(&mut g, "-", 140);
    let e = the_shutter(&g);
    stand_on(&mut g, e, 4);
    let at = (g.player().x, g.player().y);
    // Clock 140..150: still open, she stands.
    hold(&mut g, "-", 10);
    assert_eq!(g.stage_clock(), 150);
    assert_eq!(the_shutter(&g).state, shutter::CLOSED);
    assert_eq!((g.player().x, g.player().y), at, "not moved");
    assert!(g.player().on_ground);
    // The next frame she falls (no landing on the closed body).
    let events = hold(&mut g, "-", 1);
    assert!(!g.player().on_ground);
    assert!(!events.contains(&Event::Landed));
    assert_eq!(g.player().x, at.0);
    // Coyote: a jump a few frames after the drop still works.
    let mut late = g.clone();
    hold(&mut late, "-", u32::from(t.player.coyote_frames) - 2);
    let events = hold(&mut late, "J", 1);
    assert!(events.contains(&Event::Jumped), "coyote jump: {events:?}");
    // Without a jump she lands in the street, unhurt.
    until(&mut g, "-", 120, |g| g.player().on_ground);
    assert_eq!(feet(&g), Fx::from_px(192));
    assert_eq!(g.deaths(), 0);
}

/// A shutter that opens around a player inside its body neither lifts nor
/// crushes her: a Normal Resi standing under a low shutter (body y 176..184
/// across her head) and a falling one whose feet are already below its top
/// stay on their own course. Rising out of it later, she can land on top.
#[test]
fn an_opening_shutter_never_lifts_or_crushes_a_player_inside_it() {
    let t = tuning();
    // Closed at clock 0 (f = 150), opens at clock 90.
    let lvl = one(", period = 240, open = 150, phase = 150", 11);
    let mut g = Game::new(&lvl, &t, 1);
    g.player.x = Fx::from_px(104);
    hold(&mut g, "-", 2);
    let body = the_shutter(&g).hitbox();
    assert!(body.overlaps(&g.player().hitbox()), "inside its body");
    let at = (g.player().x, g.player().y);
    let events = hold(&mut g, "-", 120);
    assert_eq!(the_shutter(&g).state, shutter::OPEN);
    assert_eq!((g.player().x, g.player().y), at, "never lifted");
    assert!(!events.contains(&Event::Landed));
    assert_eq!(g.deaths(), 0);
    assert_eq!(g.mode(), Mode::Playing);
    // Big: the same.
    let mut big = Game::new(&lvl, &t, 1);
    big.set_player_power(crate::Power::Big);
    big.player.transform = 0;
    big.player.x = Fx::from_px(104);
    hold(&mut big, "-", 2);
    let at = (big.player().x, big.player().y);
    hold(&mut big, "-", 120);
    assert_eq!((big.player().x, big.player().y), at, "never lifted (big)");
    assert_eq!(big.mode(), Mode::Playing);
    // Falling with her feet 2 px below the top on the frame it opens: she
    // falls on through.
    let mut g = Game::new(&lvl, &t, 1);
    hold(&mut g, "-", 89);
    hang(&mut g, 100, 178);
    hold(&mut g, "-", 1);
    assert_eq!(the_shutter(&g).state, shutter::OPEN);
    until(&mut g, "-", 60, |g| g.player().on_ground);
    assert_eq!(feet(&g), Fx::from_px(192), "through it, onto the ground");
    // Jumping up out of it (it is open now): she lands on its top.
    hold(&mut g, "J", 20);
    until(&mut g, "-", 60, |g| g.player().on_ground);
    assert_eq!(feet(&g), Fx::from_px(176));
}

/// The pair from Stage 7 (D-094): `period` 240, `open` 150, `phase` 0 and
/// 60 are open together 90 frames per cycle, every cycle.
#[test]
fn a_shutter_pair_keeps_its_overlap_whatever_the_approach() {
    let t = tuning();
    let lvl = world(
        ", phase = 0",
        ", phase = 60",
        "",
        &[(9, 10, 'f'), (6, 14, 'g')],
    );
    for wait in [0, 37, 311] {
        let mut g = Game::new(&lvl, &t, 1);
        // Whatever she did before (here: walk around), the pair's relative
        // phase is the same.
        hold(&mut g, "L", wait);
        let mut both = 0;
        for _ in 0..480 {
            // Left to right: #2 (phase 0), then #3 (phase 60).
            let mut s = shutters(&g);
            s.sort_by_key(|e| e.x);
            if s[0].state == shutter::OPEN && s[1].state == shutter::OPEN {
                both += 1;
            }
            let d = (i32::from(s[1].count) - i32::from(s[0].count)).rem_euclid(240);
            assert_eq!(d, 60);
            g.step(input("-"));
        }
        assert_eq!(both, 180, "90 frames per cycle, two cycles");
    }
}

/// Before activation a shutter already follows the clock (it is drawn in
/// its state); it never despawns behind the camera.
#[test]
fn shutters_follow_the_clock_before_activation_and_never_despawn() {
    let t = tuning();
    let mut marks: Vec<(usize, usize, char)> = vec![(9, 4, 'f')];
    // A much wider world: a second shutter far beyond the activation margin.
    let mut rows: Vec<String> = Vec::new();
    for r in 0..14 {
        let mut row: Vec<char> = (0..120).map(|_| if r >= 12 { '#' } else { '.' }).collect();
        if r == 11 {
            row[2] = 'P';
            row[118] = 'G';
        }
        rows.push(row.into_iter().collect());
    }
    marks.push((9, 100, 'f'));
    for &(r, c, ch) in &marks {
        let mut row: Vec<char> = rows[r].chars().collect();
        row[c] = ch;
        rows[r] = row.into_iter().collect();
    }
    let text = format!(
        "[meta]\nid = \"far\"\ntime_limit = 300\n[legend]\n\"#\" = \"solid\"\n\
         \"P\" = {{ entity = \"player_start\" }}\n\"G\" = {{ entity = \"goal\" }}\n\
         \"f\" = {{ entity = \"fensterladen\", period = 120, open = 60 }}\n[map]\ngrid = '''\n{}\n'''\n",
        rows.join("\n")
    );
    let mut g = Game::new(&Level::parse(&text).unwrap(), &t, 1);
    hold(&mut g, "-", 77);
    let far = shutters(&g)[1];
    assert!(!far.active, "not active yet");
    assert_eq!(far.count, 77);
    assert_eq!(far.state, shutter::CLOSED);
    // Run right past the first shutter until it is far behind the camera.
    let gone = 6 * 16 + t.game.despawn_margin;
    until(&mut g, "RB", 1200, |g| g.camera().x > gone);
    let s = shutters(&g);
    assert_eq!(s.len(), 2, "never despawns");
    assert!(s[0].active, "passed long ago, still there");
}

/// The solver's coarse `world_hash` tells shutter states apart (only in
/// levels that have shutters).
#[test]
fn world_hash_sees_the_shutter_state() {
    let mut g = Game::new(&one(", period = 120, open = 60", 9), &tuning(), 1);
    g.step(input("-"));
    let a = g.world_hash();
    let mut later = g.clone();
    hold(&mut later, "-", 60);
    later.player = g.player;
    assert_ne!(a, later.world_hash());
}

/// The shared platform lints with a zero-length track: 3 tiles of air
/// above the body, and a way back up from under it.
#[test]
fn shutters_get_the_clearance_and_recovery_lints() {
    let t = tuning();
    let lvl = one("", 9);
    assert_eq!(track_problems(&lvl, &t), Vec::<String>::new());
    assert_eq!(recovery_problems(&lvl, &t), Vec::<String>::new());
    // A solid tile 2 tiles above the body.
    let low = world("", "", "", &[(9, 6, 'f'), (7, 7, '#')]);
    let p = track_problems(&low, &t);
    assert_eq!(p.len(), 1, "{p:?}");
    assert!(
        p[0].starts_with("fensterladen at row 10, column 7: less than 48 px free"),
        "{}",
        p[0]
    );
    // Over a 5-deep well between walls: no way back up on either side.
    let mut marks = vec![(9, 6, 'f')];
    for r in 7..12 {
        marks.extend([(r, 5, '#'), (r, 8, '#')]);
    }
    let well = world("", "", "", &marks);
    let p = recovery_problems(&well, &t);
    assert_eq!(p.len(), 1, "{p:?}");
    assert!(
        p[0].starts_with("fensterladen at row 10, column 7"),
        "{}",
        p[0]
    );
}
