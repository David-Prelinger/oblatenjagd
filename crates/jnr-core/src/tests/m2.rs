//! The M2 enemies and the Gondel (game-design.md → "Stage 2" / "Stage 3",
//! "New mechanics"; D-044, D-046): Nordic-Walker, Blasmusiker and notes,
//! E-Biker, Bienenschwarm, Gondel, and the 200-point chain start.

use super::*;
use crate::enemies::{Pose, state, swarm_offset, swarm_period};
use crate::platform::{gondel_progress, gondel_travel};
use crate::{DeathCause, EnemyKind, Entity, EntityKind, Fx, Mode, Popup, Power, Tile};

/// Legend of the test worlds (`.` air, `P` start).
const LEGEND: &str = r##"
"#" = "solid"
"-" = "oneway"
"^" = "hazard"
"~" = "wall_village_bg"
"=" = "balcony"
"w" = { entity = "nordic_walker" }
"b" = { entity = "blasmusiker" }
"e" = { entity = "e_biker" }
"t" = { entity = "selfie_tourist" }
"T" = { entity = "selfie_tourist", still = true }
"s" = { entity = "bienenschwarm", dir = 0, len = 2 }
"S" = { entity = "bienenschwarm", dir = 0, len = 2, phase = 50 }
"g" = { entity = "gondel", to = [6, -1] }
"h" = { entity = "gondel", to = [6, -1], phase = 100 }
"v" = { entity = "gondel", to = [0, -4] }
"l" = { entity = "gondel", to = [-8, 0] }
"P" = { entity = "player_start" }
"G" = { entity = "goal" }
"##;

/// A 14-row world `width` columns wide: ground in rows 12–13, the player
/// start at column 2 (row 11) unless `marks` places a `P`, plus `marks` as
/// (row, col, char).
fn world(width: usize, marks: &[(usize, usize, char)]) -> Level {
    let mut rows: Vec<Vec<char>> = (0..14)
        .map(|r| vec![if r >= 12 { '#' } else { '.' }; width])
        .collect();
    if !marks.iter().any(|m| m.2 == 'P') {
        rows[11][2] = 'P';
    }
    for &(r, c, ch) in marks {
        rows[r][c] = ch;
    }
    let grid: Vec<String> = rows.into_iter().map(|r| r.into_iter().collect()).collect();
    let text = format!(
        "[meta]\nid = \"m2\"\ntime_limit = 300\n[legend]\n{LEGEND}\n[map]\ngrid = '''\n{}\n'''\n",
        grid.join("\n")
    );
    Level::parse(&text).unwrap()
}

fn game(level: &Level) -> Game {
    Game::new(level, &tuning(), 1)
}

fn find(g: &Game, kind: EntityKind) -> Option<Entity> {
    g.entities().iter().find(|e| e.kind == kind).copied()
}

fn first(g: &Game, kind: EntityKind) -> Entity {
    find(g, kind).unwrap_or_else(|| panic!("no {kind:?}"))
}

fn index(g: &Game, kind: EntityKind) -> usize {
    g.entities().iter().position(|e| e.kind == kind).unwrap()
}

fn count(g: &Game, kind: EntityKind) -> usize {
    g.entities().iter().filter(|e| e.kind == kind).count()
}

/// Steps once with the player kept invulnerable (projectiles may pass
/// through her without consequences).
fn safe_step(g: &mut Game, token: &str) -> Vec<Event> {
    g.player.invulnerable = u8::MAX;
    g.step(input(token)).to_vec()
}

/// Puts the player in the air, falling at `vy`, with the hitbox's left
/// edge at `x` and the feet at `feet`.
fn airborne(g: &mut Game, x: Fx, feet: Fx, vy: Fx) {
    g.player.x = x;
    g.player.y = feet - g.player.h;
    g.player.vx = Fx::ZERO;
    g.player.vy = vy;
    g.player.on_ground = false;
}

/// Drops the player onto the middle of `e` (falling, feet 2 px above its
/// top): a stomp if it is stompable.
fn drop_onto(g: &mut Game, e: Entity) {
    let x = e.x + e.w.div_int(2) - g.player.w.div_int(2);
    airborne(g, x, e.y - Fx::from_px(2), Fx::from_px(3));
}

fn centre_x(e: &Entity) -> Fx {
    e.x + e.w.div_int(2)
}

// ------------------------------------------------------------------ Tiles

#[test]
fn village_back_walls_never_collide_and_balconies_are_one_way() {
    // A full-height `wall_village_bg` house front at columns 6-7 with a
    // balcony at row 9 (top 144) in front of it at column 10.
    let mut marks: Vec<(usize, usize, char)> =
        (0..12).flat_map(|r| [(r, 6, '~'), (r, 7, '~')]).collect();
    marks.push((9, 10, '='));
    let mut g = game(&world(40, &marks));
    until(&mut g, "R", 120, |g| g.player().x > Fx::from_px(8 * 16));
    assert_eq!(g.player().feet().1, Fx::from_px(192), "walked through");
    // Dropped above the balcony: she lands on it.
    airborne(&mut g, Fx::from_px(162), Fx::from_px(120), Fx::ZERO);
    until(&mut g, "-", 60, |g| g.player().on_ground);
    assert_eq!(g.player().feet().1, Fx::from_px(144));
}

// ---------------------------------------------------------- Nordic-Walker

const WALKER: EntityKind = EntityKind::Enemy(EnemyKind::NordicWalker);

/// Runs `frames` frames with the player parked and returns the walker's
/// (min x, max right edge, distinct feet heights).
fn pace(g: &mut Game, frames: u32) -> (Fx, Fx, Vec<Fx>) {
    let (mut min_x, mut max_r, mut feet) = (Fx::MAX, Fx::MIN, Vec::new());
    for _ in 0..frames {
        safe_step(g, "-");
        let w = first(g, WALKER);
        min_x = min_x.min(w.x);
        max_r = max_r.max(w.right());
        if !feet.contains(&w.feet().1) {
            feet.push(w.feet().1);
        }
    }
    (min_x, max_r, feet)
}

#[test]
fn walker_walks_at_0_75_and_turns_at_the_ledges_of_a_planter() {
    // A 1-tile planter (solid, columns 10-15: x 160..256, top 176) with
    // the walker on it at column 12 (x 194..206).
    let mut marks: Vec<(usize, usize, char)> = (10..16).map(|c| (11, c, '#')).collect();
    marks.push((10, 12, 'w'));
    let mut g = game(&world(40, &marks));
    let start = first(&g, WALKER);
    safe_step(&mut g, "-");
    let w = first(&g, WALKER);
    assert_eq!(
        w.x,
        start.x - Fx::from_raw(192),
        "0.75 px/frame to the left"
    );
    assert_eq!(w.pose(g.tuning()), Some(Pose::Walk));
    let (min_x, max_r, feet) = pace(&mut g, 600);
    // It turns before its leading foot is 1 px from the edge: never over
    // the ledge, never falls.
    assert!(
        min_x >= Fx::from_px(160) && min_x < Fx::from_px(161),
        "{min_x:?}"
    );
    assert!(
        max_r <= Fx::from_px(256) && max_r > Fx::from_px(255),
        "{max_r:?}"
    );
    assert_eq!(feet, vec![Fx::from_px(176)], "stays on the planter");
}

#[test]
fn walker_turns_at_the_edges_of_a_one_way_platform() {
    let mut marks: Vec<(usize, usize, char)> = (10..16).map(|c| (8, c, '-')).collect();
    marks.push((7, 12, 'w'));
    let mut g = game(&world(40, &marks));
    let (min_x, max_r, feet) = pace(&mut g, 600);
    assert!(
        min_x >= Fx::from_px(160) && min_x < Fx::from_px(161),
        "{min_x:?}"
    );
    assert!(
        max_r <= Fx::from_px(256) && max_r > Fx::from_px(255),
        "{max_r:?}"
    );
    assert_eq!(feet, vec![Fx::from_px(128)], "never drops off");
}

#[test]
fn walker_turns_at_walls_and_other_enemies() {
    // Walls at column 8 (right edge 144) and 18; a still tourist at 22.
    let mut g = game(&world(
        40,
        &[(11, 8, '#'), (11, 18, '#'), (11, 12, 'w'), (11, 25, 'T')],
    ));
    let (min_x, max_r, _) = pace(&mut g, 600);
    assert_eq!(min_x, Fx::from_px(144), "stopped by the left wall");
    assert_eq!(max_r, Fx::from_px(288), "stopped by the right wall");
    // Without the right wall it meets the still tourist and turns there.
    let mut g = game(&world(40, &[(11, 8, '#'), (11, 12, 'w'), (11, 25, 'T')]));
    let tourist = first(&g, EntityKind::Enemy(EnemyKind::SelfieTourist));
    let (_, max_r, _) = pace(&mut g, 900);
    assert!(
        max_r > tourist.x && max_r <= tourist.x + Fx::ONE,
        "turns on touching the tourist: {max_r:?} vs {:?}",
        tourist.x
    );
}

#[test]
fn walker_falls_when_the_floor_under_it_disappears() {
    let mut g = game(&world(40, &[(11, 12, 'w')]));
    safe_step(&mut g, "-");
    // The ground under column 12 (where the whole walker stands) goes.
    for row in [12, 13] {
        g.set_tile(12, row, Tile::default());
    }
    let y0 = first(&g, WALKER).y;
    let mut fell = false;
    for _ in 0..60 {
        safe_step(&mut g, "-");
        match find(&g, WALKER) {
            Some(w) => fell |= w.y > y0,
            None => break,
        }
    }
    assert!(fell, "falls");
    assert!(find(&g, WALKER).is_none(), "gone below the level");
}

#[test]
fn stomping_a_walker_scores_100() {
    let mut g = game(&world(40, &[(11, 12, 'w')]));
    let w = first(&g, WALKER);
    drop_onto(&mut g, w);
    let events = g.step(input("-")).to_vec();
    assert!(events.contains(&Event::Stomped), "{events:?}");
    assert_eq!(g.score(), 100);
    assert!(find(&g, WALKER).is_none());
}

// ------------------------------------------------------------ Blasmusiker

const MUSICIAN: EntityKind = EntityKind::Enemy(EnemyKind::Blasmusiker);

#[test]
fn blasmusiker_bar_inhale_then_notes_at_30_54_78_every_180_frames_in_range() {
    let t = tuning();
    // Musician at column 13 (centre 216); the player (centre 40) is
    // 176 px away: out of range (160).
    let mut g = game(&world(40, &[(11, 13, 'b')]));
    for _ in 0..100 {
        safe_step(&mut g, "-");
    }
    let m = first(&g, MUSICIAN);
    assert_eq!((m.count, m.pose(&t)), (0, Some(Pose::Idle)), "waits");
    assert_eq!(m.dir, -1, "faces the player");
    assert_eq!(count(&g, EntityKind::Note), 0);
    // Into range: centre 120 (96 px away).
    g.player.x = Fx::from_px(114);
    let (mut notes, mut inhale, mut blow) = (Vec::new(), Vec::new(), Vec::new());
    let mut spawned = 0;
    for f in 1..=400u32 {
        safe_step(&mut g, "-");
        let n = g
            .entities()
            .iter()
            .filter(|e| e.kind == EntityKind::Note && e.timer == 0)
            .count();
        if n > 0 {
            notes.push(f);
            spawned += n;
        }
        match first(&g, MUSICIAN).pose(&t) {
            Some(Pose::Inhale) => inhale.push(f),
            Some(Pose::Blow) => blow.push(f),
            _ => {}
        }
    }
    // In-range frame f is bar frame f - 1.
    assert_eq!(notes, vec![31, 55, 79, 211, 235, 259, 391], "Hum-ta-ta");
    assert_eq!(spawned, notes.len(), "one note each");
    let first_inhale: Vec<u32> = inhale.iter().copied().filter(|&f| f <= 180).collect();
    assert_eq!(first_inhale, (1..=30).collect::<Vec<u32>>(), "inhale 0-29");
    assert!(
        inhale
            .iter()
            .filter(|&&f| f > 180 && f <= 360)
            .eq((181..=210).collect::<Vec<u32>>().iter())
    );
    let first_blow: Vec<u32> = blow.iter().copied().filter(|&f| f <= 180).collect();
    let want: Vec<u32> = [31, 55, 79].iter().flat_map(|&n| n..n + 8).collect();
    assert_eq!(first_blow, want, "8-frame blow pose per note");
    // Out of range again: the counter pauses with the pose kept.
    // (Centre 387: 171 px to his right, with him still on screen.)
    g.player.x = Fx::from_px(381);
    safe_step(&mut g, "-");
    let paused = first(&g, MUSICIAN);
    assert_eq!(paused.dir, 1, "turns to the player");
    for _ in 0..90 {
        safe_step(&mut g, "-");
        let m = first(&g, MUSICIAN);
        assert_eq!((m.count, m.state), (paused.count, paused.state));
    }
}

/// The first note of a musician at column 12 (centre 200, feet 192) facing
/// left, with the player in range; returns the game right after it spawned.
fn first_note(marks: &[(usize, usize, char)]) -> Game {
    let mut all = vec![(11, 12, 'b')];
    all.extend_from_slice(marks);
    let mut g = game(&world(40, &all));
    until(&mut g, "-", 200, |g| count(g, EntityKind::Note) > 0);
    g
}

#[test]
fn a_note_starts_at_the_bell_and_flies_straight_at_1_px_for_160_px() {
    let mut g = first_note(&[]);
    let n = first(&g, EntityKind::Note);
    // Centre 10 px in front of his centre and 10 px above his feet.
    assert_eq!((n.w, n.h), (Fx::from_px(6), Fx::from_px(6)));
    assert_eq!(centre_x(&n), Fx::from_px(190));
    assert_eq!(n.y + n.h.div_int(2), Fx::from_px(182));
    assert_eq!(n.pose(g.tuning()), Some(Pose::Fly));
    let start = n.x;
    // Later notes follow on the same line; this one is `k` px along.
    let mut k = 0;
    loop {
        safe_step(&mut g, "-");
        let want = start - Fx::from_px(k + 1);
        let here = g
            .entities()
            .iter()
            .any(|e| e.kind == EntityKind::Note && e.x == want && e.y == n.y);
        if !here {
            break;
        }
        k += 1;
        assert!(k < 400);
    }
    assert_eq!(k, 159, "gone after 160 px");
}

#[test]
fn a_note_vanishes_on_a_solid_tile_and_passes_one_way_tiles_and_enemies() {
    // A wall at column 8 (x 128..144) in the note's row (11).
    let mut g = first_note(&[(11, 8, '#')]);
    let mut lowest = Fx::MAX;
    for _ in 0..100 {
        safe_step(&mut g, "-");
        match find(&g, EntityKind::Note) {
            Some(n) => lowest = lowest.min(n.x),
            None => break,
        }
    }
    assert!(find(&g, EntityKind::Note).is_none(), "vanished");
    assert!(
        lowest >= Fx::from_px(144),
        "never inside the wall: {lowest:?}"
    );
    // A one-way tile and a (still) tourist in its path: it flies on.
    let mut g = first_note(&[(11, 8, '-'), (11, 6, 'T')]);
    let tourist = first(&g, EntityKind::Enemy(EnemyKind::SelfieTourist));
    let mut lowest = Fx::MAX;
    for _ in 0..140 {
        safe_step(&mut g, "-");
        if let Some(n) = find(&g, EntityKind::Note) {
            lowest = lowest.min(n.x);
        }
    }
    assert!(lowest < Fx::from_px(80), "passed both: {lowest:?}");
    let t2 = first(&g, EntityKind::Enemy(EnemyKind::SelfieTourist));
    assert_eq!((t2.x, t2.dir), (tourist.x, tourist.dir), "unaffected");
}

#[test]
fn a_note_hurts_and_cannot_be_stomped() {
    let t = tuning();
    let mut g = game(&world(40, &[]));
    g.player.set_power(Power::Big, &t.player);
    g.player.transform = 0;
    let mut n = Entity::at_feet(
        EntityKind::Note,
        g.player().x + g.player().w.div_int(2),
        g.player().y + Fx::from_px(5),
        t.enemies.note_hitbox,
    );
    n.vx = -t.enemies.note_speed;
    g.entities.push(n);
    g.player.vy = Fx::from_px(1);
    let events = g.step(input("-")).to_vec();
    assert!(events.contains(&Event::Hurt), "{events:?}");
    assert!(!events.contains(&Event::Stomped));
    assert_eq!((g.player().power, g.score()), (Power::Normal, 0));
}

#[test]
fn stomping_a_blasmusiker_starts_the_chain_at_200() {
    let mut g = game(&world(40, &[(11, 12, 'b')]));
    let m = first(&g, MUSICIAN);
    drop_onto(&mut g, m);
    let events = g.step(input("-")).to_vec();
    assert!(events.contains(&Event::Stomped), "{events:?}");
    assert_eq!(g.score(), 200);
    assert_eq!(
        first(&g, EntityKind::Defeated(EnemyKind::Blasmusiker)).pose(g.tuning()),
        Some(Pose::Squished)
    );
}

// ---------------------------------------------------------------- E-Biker

const BIKER: EntityKind = EntityKind::Enemy(EnemyKind::EBiker);

#[test]
fn e_biker_triggers_within_144_px() {
    // E-Biker at column 20 (centre 328).
    let mut g = game(&world(40, &[(11, 20, 'e')]));
    g.player.x = Fx::from_px(328 - 145 - 6);
    g.step(input("-"));
    assert_eq!(first(&g, BIKER).state, state::PARK, "145 px: parked");
    g.player.x = Fx::from_px(328 - 144 - 6);
    g.step(input("-"));
    let b = first(&g, BIKER);
    assert_eq!(
        (b.state, b.pose(g.tuning())),
        (state::RING, Some(Pose::Telegraph))
    );
}

#[test]
fn e_biker_telegraphs_40_frames_then_rides_left_at_2_px() {
    let t = tuning();
    let mut g = game(&world(60, &[(11, 20, 'e')]));
    let parked = first(&g, BIKER);
    until(&mut g, "R", 200, |g| first(g, BIKER).state == state::RING);
    let mut telegraph = 1;
    loop {
        g.step(input("L"));
        let b = first(&g, BIKER);
        if b.state != state::RING {
            break;
        }
        assert_eq!(b.x, parked.x, "stands while telegraphing");
        telegraph += 1;
    }
    assert_eq!(telegraph, 40);
    let b = first(&g, BIKER);
    assert_eq!(b.pose(&t), Some(Pose::Ride));
    assert_eq!(b.x, parked.x - Fx::from_px(2), "2 px/frame left");
    assert!(b.facing_left());
}

#[test]
fn a_riding_e_biker_follows_the_ground_down_and_is_lost_in_water() {
    // It rides off a 1-tile ledge (solid row 11, columns 14-25: top 176)
    // down to the ground (top 192), on to a water gap at columns 4-5.
    let mut marks: Vec<(usize, usize, char)> = (14..26).map(|c| (11, c, '#')).collect();
    marks.extend([(10, 20, 'e'), (12, 4, '^'), (12, 5, '^')]);
    let mut g = game(&world(40, &marks));
    let i = index(&g, BIKER);
    g.entities[i].active = true;
    g.entities[i].state = state::RIDE;
    let mut feet = Vec::new();
    let mut gone = false;
    for _ in 0..300 {
        safe_step(&mut g, "-");
        match find(&g, BIKER) {
            Some(b) => {
                assert_eq!(b.state, state::RIDE, "no crash");
                if !feet.contains(&b.feet().1) {
                    feet.push(b.feet().1);
                }
            }
            None => {
                gone = true;
                break;
            }
        }
    }
    assert!(gone, "lost in the water");
    assert_eq!(feet.first(), Some(&Fx::from_px(176)));
    assert!(feet.contains(&Fx::from_px(192)), "rode down: {feet:?}");
    assert_eq!(count(&g, EntityKind::Popup(Popup::Hoppala)), 0);
    assert_eq!(g.score(), 0);
}

#[test]
fn e_biker_crashes_into_a_wall_hoppala_no_points_then_gone() {
    let t = tuning();
    // A 2-tile boulder at column 10 (right edge 176).
    let mut g = game(&world(40, &[(11, 10, '#'), (10, 10, '#'), (11, 20, 'e')]));
    let i = index(&g, BIKER);
    g.entities[i].active = true;
    g.entities[i].state = state::RIDE;
    until(&mut g, "-", 200, |g| {
        find(g, BIKER).is_some_and(|b| b.state == state::CRASH)
    });
    let b = first(&g, BIKER);
    assert_eq!(b.x, Fx::from_px(176), "stopped at the boulder");
    assert_eq!(b.pose(&t), Some(Pose::Crash));
    assert_eq!(
        count(&g, EntityKind::Popup(Popup::Hoppala)),
        1,
        "„Hoppala!\""
    );
    // Harmless while crashed: the player stands in it.
    g.player.x = b.x + Fx::from_px(4);
    g.player.vx = Fx::ZERO;
    let mut frames = 1;
    loop {
        let events = g.step(input("-")).to_vec();
        assert!(!events.contains(&Event::Died), "{events:?}");
        if find(&g, BIKER).is_none() {
            break;
        }
        frames += 1;
        assert!(frames <= 40);
    }
    assert_eq!(frames, 30, "30 frames of the crash pose");
    assert_eq!((g.score(), g.deaths()), (0, 0), "no points");
}

#[test]
fn stomping_an_e_biker_scores_200_and_a_side_hit_kills() {
    let mut g = game(&world(40, &[(11, 12, 'e')]));
    let b = first(&g, BIKER);
    drop_onto(&mut g, b);
    let events = g.step(input("-")).to_vec();
    assert!(events.contains(&Event::Stomped), "{events:?}");
    assert_eq!(g.score(), 200, "chain start 200");
    assert_eq!(
        first(&g, EntityKind::Defeated(EnemyKind::EBiker)).pose(g.tuning()),
        Some(Pose::Squished)
    );
    // Walking into a parked one as Normal: death.
    let mut g = game(&world(40, &[(11, 12, 'e')]));
    until(&mut g, "R", 200, |g| g.mode() != Mode::Playing);
    assert_eq!(g.death_cause(), Some(DeathCause::Hit));
}

#[test]
fn a_tourist_then_a_200_point_enemy_continues_the_chain() {
    // Tourist (100) then, without landing, the musician: the chain's
    // second step (200), not a fresh start.
    let mut g = game(&world(40, &[(11, 12, 't'), (11, 20, 'b')]));
    let t = first(&g, EntityKind::Enemy(EnemyKind::SelfieTourist));
    drop_onto(&mut g, t);
    g.step(input("-"));
    assert_eq!(g.score(), 100);
    let m = first(&g, MUSICIAN);
    drop_onto(&mut g, m);
    g.player.stomp_chain = 1;
    g.step(input("-"));
    assert_eq!(g.score(), 300);
}

// ----------------------------------------------------------- Bienenschwarm

const SWARM: EntityKind = EntityKind::Enemy(EnemyKind::Bienenschwarm);

#[test]
fn swarm_period_is_twice_run_plus_return() {
    let t = tuning();
    assert_eq!(
        [2, 3, 4].map(|len| swarm_period(len, &t)),
        [192, 224, 256],
        "game-design.md: len 2/3/4"
    );
}

#[test]
fn swarm_dances_a_figure_eight() {
    let t = tuning();
    let px = Fx::from_px;
    let at = |dir, len, k| {
        let (x, y, run) = swarm_offset(dir, len, k, &t);
        (x, y, run)
    };
    // dir 0 (right), len 2: run 32 px, return over 64 frames.
    assert_eq!(at(0, 2, 0), (px(0), px(0), true), "at A");
    assert_eq!(at(0, 2, 16), (px(16), px(0), true), "1 px/frame");
    assert_eq!(at(0, 2, 32), (px(32), px(0), false), "at B, turning back");
    assert_eq!(at(0, 2, 64), (px(16), px(-16), false), "left loop (up)");
    assert_eq!(at(0, 2, 96), (px(0), px(0), true), "back at A, second run");
    assert_eq!(at(0, 2, 128), (px(32), px(0), false));
    assert_eq!(at(0, 2, 160), (px(16), px(16), false), "right loop (down)");
    assert_eq!(at(0, 2, 192), at(0, 2, 0), "periodic");
    // dir 2 (up), len 3: run 48 px up, left of it is the left.
    assert_eq!(at(2, 3, 20), (px(0), px(-20), true));
    assert_eq!(at(2, 3, 48 + 32), (px(-24), px(-24), false));
    assert_eq!(at(2, 3, 112 + 48 + 32), (px(24), px(-24), false));
    // dir 1 (45° up-right), len 4: 64 px along the diagonal.
    assert_eq!(at(1, 4, 10), (Fx(1810), Fx(-1810), true));
    assert_eq!(at(1, 4, 64 + 32), (px(0), Fx(-11584), false), "left loop");
    // Every return frame is on the circle over AB (radius L/2 around its
    // middle, within 1 px), every step at most 2 px: no jumps.
    for (dir, len) in [(0, 2), (1, 4), (2, 3), (5, 3)] {
        let period = swarm_period(len, &t);
        let r = i64::from(len) * 8 * 256;
        let (mx, my, _) = at(dir, len, u16::from(len) * 8);
        let mut prev = at(dir, len, period - 1);
        for k in 0..period {
            let p = at(dir, len, k);
            if !p.2 {
                let (dx, dy) = (i64::from((p.0 - mx).raw()), i64::from((p.1 - my).raw()));
                let d2 = dx * dx + dy * dy;
                assert!(
                    (d2 - r * r).abs() <= 2 * r * 256,
                    "dir {dir} len {len} k {k}"
                );
            }
            let step = (p.0 - prev.0).abs().max((p.1 - prev.1).abs());
            assert!(step <= px(2), "dir {dir} len {len} k {k}: {step:?}");
            prev = p;
        }
    }
}

#[test]
fn a_swarm_in_the_level_follows_its_path_from_its_phase() {
    let t = tuning();
    // Anchored at column 10, row 5 (centre 168, 88); `S` starts 50 frames
    // later in its cycle.
    let mut a = game(&world(40, &[(5, 10, 's')]));
    let mut b = game(&world(40, &[(5, 10, 'S')]));
    let s = first(&a, SWARM);
    assert_eq!(
        (centre_x(&s), s.y + s.h.div_int(2)),
        (Fx::from_px(168), Fx::from_px(88))
    );
    let at = |g: &Game| {
        let e = first(g, SWARM);
        (e.x, e.y)
    };
    let mut path = vec![at(&a)];
    for k in 1..=400u16 {
        safe_step(&mut a, "-");
        let e = first(&a, SWARM);
        let (ox, oy, run) = swarm_offset(0, 2, k, &t);
        assert_eq!((e.x - s.x, e.y - s.y), (ox, oy), "frame {k}");
        let pose = if run { Pose::Waggle } else { Pose::Buzz };
        assert_eq!(e.pose(&t), Some(pose));
        path.push(at(&a));
    }
    for k in 0..300 {
        assert_eq!(at(&b), path[k + 50], "phase 50, frame {k}");
        safe_step(&mut b, "-");
    }
}
#[test]
fn a_swarm_hurts_from_above_and_cannot_be_stomped() {
    let t = tuning();
    let mut g = game(&world(40, &[(10, 12, 's')]));
    g.player.set_power(Power::Big, &t.player);
    g.player.transform = 0;
    let s = first(&g, SWARM);
    drop_onto(&mut g, s);
    let events = g.step(input("-")).to_vec();
    assert!(events.contains(&Event::Hurt), "{events:?}");
    assert!(!events.contains(&Event::Stomped));
    assert_eq!(g.score(), 0);
    assert!(find(&g, SWARM).is_some(), "can't be defeated");
}

#[test]
fn a_swarm_shakes_across_its_run_only_in_the_renderer() {
    let t = tuning();
    let mut g = game(&world(40, &[(5, 10, 's')]));
    let mut seen = Vec::new();
    for _ in 0..40 {
        safe_step(&mut g, "-");
        let e = first(&g, SWARM);
        seen.push(e.shake_px(&t));
    }
    // count 1..=31 on the run (dir 0: across = vertical), ±2 every 4 frames.
    assert_eq!(
        &seen[..8],
        &[
            (0, -2),
            (0, -2),
            (0, -2),
            (0, 2),
            (0, 2),
            (0, 2),
            (0, 2),
            (0, -2)
        ]
    );
    assert_eq!(seen[32], (0, 0), "no shake on the return");
}

// ----------------------------------------------------------------- Gondel

const GONDEL: EntityKind = EntityKind::Gondel;

/// The cabin's position `k` frames into its cycle for a spawn at
/// (`x0`, `y0`) px going `to` tiles.
fn expected(x0: i32, y0: i32, to: [i32; 2], k: u16) -> (Fx, Fx) {
    let t = tuning();
    let travel = (to[0].abs().max(to[1].abs()) * 16) as u16;
    let p = i32::from(gondel_progress(k, travel, t.platforms.gondel_wait));
    let n = i32::from(travel);
    (
        Fx::from_px(x0) + Fx::from_px(to[0] * 16).mul_int(p).div_int(n),
        Fx::from_px(y0) + Fx::from_px(to[1] * 16).mul_int(p).div_int(n),
    )
}

#[test]
fn gondel_progress_waits_travels_and_comes_back() {
    let p = |k| gondel_progress(k, 96, 30);
    assert_eq!((p(0), p(29)), (0, 0), "waits 30 frames at the spawn end");
    assert_eq!((p(30), p(31), p(125)), (1, 2, 96), "1 px/frame, arrives");
    assert_eq!((p(155), p(156)), (96, 95), "waits, then comes back");
    assert_eq!(p(251), 0, "home on the last frame of the 252-frame cycle");
}

#[test]
fn gondel_runs_free_from_activation_and_never_drifts() {
    let t = tuning();
    // Spawn cell (col 4, row 11): x 64, top 168 (bottom on the cell's).
    for (ch, to) in [('g', [6i32, -1]), ('v', [0, -4]), ('l', [-8, 0])] {
        let col = if ch == 'l' { 12 } else { 4 };
        let mut g = game(&world(40, &[(11, col, ch)]));
        let c = first(&g, GONDEL);
        assert_eq!((c.w, c.h), (Fx::from_px(32), Fx::from_px(24)));
        assert_eq!((c.x, c.y), (Fx::from_px(col as i32 * 16), Fx::from_px(168)));
        let spawn = &g.level().entities[c.spawn.unwrap() as usize];
        let period = u32::from(crate::platform::gondel_period(spawn, &t));
        assert_eq!(
            gondel_travel(spawn, &t),
            (to[0].abs().max(to[1].abs()) * 16) as u16
        );
        // Nobody rides: it moves anyway, over three whole cycles.
        for f in 1..=3 * period {
            safe_step(&mut g, "-");
            let c = first(&g, GONDEL);
            let k = (f % period) as u16;
            assert_eq!(
                (c.x, c.y),
                expected(col as i32 * 16, 168, to, k),
                "{ch} frame {f}"
            );
        }
    }
    assert_eq!(
        crate::platform::gondel_period(&world(40, &[(11, 4, 'g')]).entities[1], &t),
        252,
        "[6, -1]: 96 frames of travel, 4.2 s"
    );
}

#[test]
fn gondel_phase_shifts_its_cycle() {
    let mut a = game(&world(40, &[(11, 4, 'g')]));
    let mut b = game(&world(40, &[(11, 4, 'h')]));
    let at = |g: &Game| {
        let c = first(g, GONDEL);
        (c.x, c.y)
    };
    let mut path = Vec::new();
    for _ in 0..600 {
        path.push(at(&a));
        safe_step(&mut a, "-");
    }
    for k in 0..400 {
        assert_eq!(at(&b), path[k + 100], "frame {k}");
        safe_step(&mut b, "-");
    }
}

/// Puts the player standing on the cabin, `dx` px right of its left edge.
fn board(g: &mut Game, dx: i32) {
    let c = first(g, GONDEL);
    g.player.x = c.x + Fx::from_px(dx);
    g.player.y = c.y - g.player.h;
    g.player.vx = Fx::ZERO;
    g.player.vy = Fx::ZERO;
    g.player.on_ground = true;
}

#[test]
fn gondel_carries_a_standing_player_exactly() {
    let mut g = game(&world(40, &[(11, 4, 'g')]));
    safe_step(&mut g, "-");
    board(&mut g, 10);
    let mut moved = 0;
    for _ in 0..504 {
        let before = (first(&g, GONDEL), *g.player());
        let events = g.step(input("-")).to_vec();
        let (c, p) = (first(&g, GONDEL), *g.player());
        assert_eq!(p.y + p.h, c.y, "feet exactly on the roof");
        assert_eq!(p.x - c.x, Fx::from_px(10), "no sliding");
        assert_eq!(p.x - before.1.x, c.x - before.0.x);
        assert!(p.on_ground && !events.contains(&Event::Landed));
        moved += u32::from(c.x != before.0.x);
    }
    assert_eq!(moved, 4 * 96, "two whole cycles of travel");
    assert_eq!(g.mode(), Mode::Playing);
}

#[test]
fn a_gondel_rider_is_held_at_the_screen_s_left_edge() {
    // A cabin at column 12 going 8 tiles left; the camera's left edge is
    // put at x 160 (as if the player had scrolled it there): the rider
    // stays on screen and slides back over the roof instead.
    let mut g = game(&world(60, &[(11, 12, 'l')]));
    safe_step(&mut g, "-");
    board(&mut g, 20);
    g.camera.x = 160;
    let mut held = 0;
    for _ in 0..300 {
        g.step(input("-"));
        let p = *g.player();
        assert!(p.x >= Fx::from_px(g.camera().x), "on screen");
        held += u32::from(p.x == Fx::from_px(160));
    }
    assert!(held > 0, "pushed against the edge");
}

#[test]
fn falling_onto_a_gondel_lands_on_its_roof() {
    let mut g = game(&world(40, &[(11, 4, 'g')]));
    safe_step(&mut g, "-");
    let c = first(&g, GONDEL);
    airborne(
        &mut g,
        c.x + Fx::from_px(8),
        c.y - Fx::from_px(20),
        Fx::from_px(2),
    );
    until(&mut g, "-", 30, |g| g.player().on_ground);
    let c = first(&g, GONDEL);
    assert_eq!(g.player().feet().1, c.y, "on the roof, not the ground");
}

#[test]
fn gondel_never_despawns_behind_the_camera() {
    let t = tuning();
    let level = world(120, &[(11, 4, 'g')]);
    let mut far = game(&level);
    let mut near = game(&level);
    safe_step(&mut far, "-");
    safe_step(&mut near, "-");
    far.teleport_player(1500);
    let despawn_x = Fx::from_px(far.camera().x - t.game.despawn_margin);
    assert!(first(&far, GONDEL).right() < despawn_x);
    for _ in 0..600 {
        safe_step(&mut far, "-");
        safe_step(&mut near, "-");
        let (a, b) = (first(&far, GONDEL), first(&near, GONDEL));
        assert_eq!((a.x, a.y), (b.x, b.y), "same cycle");
    }
}
