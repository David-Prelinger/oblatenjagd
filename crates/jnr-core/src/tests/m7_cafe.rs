//! M7 integration (E49): the Stage 9 kinds together (D-104 … D-107) in the
//! grey box `00_m7_cafe` and small test worlds: a Kellner turning at a
//! Wartender and at a lying Kurgast, a Mozartkugel turning at the step
//! before the arena, the `goal_stollen` arming exactly on the third hit
//! and never on a respawn, a respawn during a volley rebuilding the fight,
//! a thrown Hackl passing through the stun, the fixed room camera in every
//! phase (the ending included), the solver's route through the whole grey
//! box, and the axe and swim-water classification of every new kind.

use std::sync::Arc;

use super::*;
use crate::enemies::state::{self as st, oberkellner as boss_st, rechnung};
use crate::enemies::{arena_problems, boss_hits, lie_down};
use crate::game::EndingPhase;
use crate::hackl::{AxeHit, axe};
use crate::level::SpawnKind;
use crate::player::{Facing, Power};
use crate::route::Route;
use crate::{EnemyKind, Entity, EntityKind, Fx, Mode, Replay, Start};

/// The grey box: 88 × 17, floor top y 240, the arena columns 68–87.
fn cafe() -> Level {
    Level::parse(include_str!("../../../../content/levels/00_m7_cafe.level")).unwrap()
}

fn game() -> Game {
    Game::new(&cafe(), &tuning(), 1)
}

const KELLNER: EntityKind = EntityKind::Enemy(EnemyKind::Kellner);
/// A Wartender in the simulation (D-105).
const TOURIST: EntityKind = EntityKind::Enemy(EnemyKind::SelfieTourist);
const KURGAST: EntityKind = EntityKind::Enemy(EnemyKind::Kurgast);
const BOSS: EntityKind = EntityKind::Enemy(EnemyKind::Oberkellner);

/// The fixed camera's y (`camera_top` = 17 × 16 − 180).
const CAMERA_Y: i32 = 92;
/// The camera's right clamp (88 × 16 − 320) = the arena's left edge.
const MAX_CAMERA_X: i32 = 1088;
/// The floor's top.
const FLOOR_Y: i32 = 240;
/// The 1-high step before the arena (columns 66–67): its left face.
const STEP_X: i32 = 66 * 16;
/// The Oberkellner's spawn x (column 82, his 16 px box on the cell).
const BOSS_X: i32 = 82 * 16;

fn all(g: &Game, kind: EntityKind) -> Vec<Entity> {
    g.entities()
        .iter()
        .filter(|e| e.kind == kind && !e.removed)
        .copied()
        .collect()
}

fn first(g: &Game, kind: EntityKind) -> Entity {
    all(g, kind)
        .first()
        .copied()
        .unwrap_or_else(|| panic!("no {kind:?}"))
}

fn boss_index(g: &Game) -> usize {
    g.entities.iter().position(|e| e.kind == BOSS).unwrap()
}

fn boss(g: &Game) -> Entity {
    g.entities[boss_index(g)]
}

fn bills(g: &Game) -> Vec<Entity> {
    all(g, EntityKind::Rechnung)
}

/// Steps once with `token` and the player kept invulnerable.
fn safe(g: &mut Game, token: &str) -> Vec<Event> {
    g.player.invulnerable = u8::MAX;
    g.step(input(token)).to_vec()
}

/// The player standing on the left refuge (columns 68–69, top y 192).
fn on_refuge(g: &mut Game) {
    g.player.x = Fx::from_px(1090);
    g.player.y = Fx::from_px(192) - g.player.h;
    g.player.vx = Fx::ZERO;
    g.player.vy = Fx::ZERO;
    g.player.on_ground = true;
}

/// Locks the arena: the camera at its right clamp, the player on the
/// left refuge, one frame stepped (the boss's intro frame 1).
fn lock(g: &mut Game) {
    g.camera.x = MAX_CAMERA_X;
    on_refuge(g);
    safe(g, "-");
    assert_eq!(
        (boss(g).state, boss(g).count),
        (boss_st::INTRO, 1),
        "locked"
    );
}

/// Steps (the player safe on the refuge) until the boss is in `state`.
fn until_boss(g: &mut Game, state: u8, max: u32) {
    for _ in 0..max {
        on_refuge(g);
        safe(g, "-");
        if boss(g).state == state {
            return;
        }
    }
    panic!("boss state {state} not reached in {max} frames");
}

/// Puts the player in the air above the middle of `e`, falling at 3 px per
/// frame with the feet 2 px above its top.
fn drop_onto(g: &mut Game, e: Entity) {
    g.player.x = e.x + e.w.div_int(2) - g.player.w.div_int(2);
    g.player.y = e.y - Fx::from_px(2) - g.player.h;
    g.player.vx = Fx::ZERO;
    g.player.vy = Fx::from_px(3);
    g.player.on_ground = false;
}

/// Passes the checkpoint at column 59 (the player stands at 60 for one
/// frame).
fn pass_checkpoint(g: &mut Game) {
    g.teleport_player(60 * 16 + 8);
    safe(g, "-");
    assert!(g.checkpoint().is_some());
}

// ------------------------------------------------------------- the level

/// The grey box holds every M7 kind and tile: a Kellner pen beside a still
/// queue, a Kurgast beside a vitrine, the arena with the Oberkellner and
/// the `goal_stollen`, the café's tiles and props; the arena lint passes.
#[test]
fn the_grey_box_holds_every_m7_kind() {
    let level = cafe();
    let count = |k: SpawnKind| level.entities.iter().filter(|s| s.kind == k).count();
    assert_eq!(count(SpawnKind::Kellner), 1);
    assert_eq!(count(SpawnKind::Wartender), 5);
    assert_eq!(count(SpawnKind::Kurgast), 1);
    assert_eq!(count(SpawnKind::Oberkellner), 1);
    assert_eq!(count(SpawnKind::GoalStollen), 1);
    assert_eq!(count(SpawnKind::Checkpoint), 1);
    for prop in ["zauner_tuer", "garderobe", "kuchentheke", "luster_deko"] {
        assert!(
            level
                .entities
                .iter()
                .any(|s| s.kind == SpawnKind::Prop && s.sprite.as_deref() == Some(prop)),
            "{prop}"
        );
    }
    for tile in [
        "stuck",
        "marmor",
        "marmor_punkt",
        "vitrine",
        "theke",
        "luster",
    ] {
        assert!(level.tile_names.iter().any(|n| n == tile), "{tile}");
    }
    assert_eq!(level.meta.camera_top, Some(CAMERA_Y));
    assert_eq!(arena_problems(&level, &tuning()), Vec::<String>::new());
    // The pen's right end is the queue's first Wartender (column 24).
    let col = |k: SpawnKind| level.entities.iter().find(|s| s.kind == k).unwrap().col;
    assert_eq!(
        (col(SpawnKind::Kellner), col(SpawnKind::Wartender)),
        (21, 24)
    );
}

/// The committed solver route beats the boss (three hits) and finishes
/// without a death, and the camera never leaves y 92 on the way.
#[test]
fn the_solver_route_beats_the_boss_with_the_camera_fixed() {
    let replay = Replay::parse(include_str!(
        "../../../../tests/replays/m7_cafe_solver.replay"
    ))
    .unwrap();
    let mut ys = Vec::new();
    let mut locked_at = None;
    let o = replay.run(&cafe(), &tuning(), |g| {
        ys.push(g.camera().y);
        let b = g.entities().iter().find(|e| e.kind == BOSS);
        if locked_at.is_none() && b.is_some_and(|b| b.state != boss_st::WAIT) {
            locked_at = Some(g.frame());
        }
    });
    assert!(o.completed_at.is_some());
    assert_eq!((o.deaths, o.boss_hits), (0, Some(3)));
    assert!(ys.iter().all(|&y| y == CAMERA_Y), "camera y {ys:?}");
    assert!(locked_at.is_some(), "the arena locked");
}

// ---------------------------------------------- the Kellner and his pen

/// The Kellner in his pen beside the queue walks to the vitrine, turns,
/// walks back and turns at the first Wartender, never into the queue; the
/// Wartender never moves.
#[test]
fn a_kellner_turns_at_a_wartender() {
    let mut g = game();
    safe(&mut g, "-");
    let w = first(&g, TOURIST);
    let k = first(&g, KELLNER);
    assert!(k.active && k.dir == -1, "walks towards her");
    let (mut at_vitrine, mut at_queue) = (false, false);
    let mut prev = k;
    for _ in 0..600 {
        safe(&mut g, "-");
        let k = first(&g, KELLNER);
        // At most a frame's step into him: he turns on the overlap.
        assert!(k.right() <= w.x + Fx::from_px(2), "never into the queue");
        let now = first(&g, TOURIST);
        assert_eq!((now.x, now.y, now.dir), (w.x, w.y, 0), "never moves");
        if prev.dir == -1 && k.dir == 1 {
            assert!(k.x <= Fx::from_px(12 * 16 + 1), "turns at the vitrine");
            at_vitrine = true;
        }
        if prev.dir == 1 && k.dir == -1 {
            assert!(k.hitbox().overlaps(&w.hitbox()), "turns at the Wartender");
            at_queue = true;
        }
        prev = k;
    }
    assert!(at_vitrine && at_queue);
}

/// Legend of the small test worlds (`.` air).
const LEGEND: &str = r##"
"#" = "solid"
"K" = { entity = "kellner" }
"k" = { entity = "kurgast" }
"W" = { entity = "wartender", tile = "see_wasser" }
"S" = { entity = "kellner", tile = "see_wasser" }
"O" = { entity = "oberkellner", tile = "see_wasser" }
"P" = { entity = "player_start" }
"##;

/// A 14-row world `width` columns wide: solid floor in rows 12–13 (top y
/// 192), the start at column 2; `marks` are (row, col, char) in
/// [`LEGEND`].
fn world(width: usize, marks: &[(usize, usize, char)]) -> Level {
    let mut rows: Vec<Vec<char>> = (0..14)
        .map(|r| vec![if r >= 12 { '#' } else { '.' }; width])
        .collect();
    rows[11][2] = 'P';
    for &(r, c, ch) in marks {
        rows[r][c] = ch;
    }
    let grid: Vec<String> = rows.into_iter().map(|r| r.into_iter().collect()).collect();
    let text = format!(
        "[meta]\nid = \"m7_mix_test\"\ntime_limit = 300\n[legend]\n{LEGEND}\n[map]\ngrid = '''\n{}\n'''\n",
        grid.join("\n")
    );
    Level::parse(&text).unwrap()
}

/// A Kellner walking at a Kurgast lying in his deck chair turns at the
/// chair (widened by the berth, D-102) and never walks through it; the
/// chair stays.
#[test]
fn a_kellner_turns_at_a_lying_kurgast() {
    let mut g = Game::new(&world(40, &[(11, 12, 'k'), (11, 20, 'K')]), &tuning(), 1);
    safe(&mut g, "-");
    let i = g.entities.iter().position(|e| e.kind == KURGAST).unwrap();
    lie_down(&mut g.entities[i], &tuning());
    let chair = g.entities[i];
    assert_eq!(first(&g, KELLNER).dir, -1);
    let mut turned = false;
    for _ in 0..250 {
        safe(&mut g, "-");
        let k = first(&g, KELLNER);
        assert!(k.x > chair.right(), "never reaches the chair");
        turned |= k.dir == 1;
        let lying = first(&g, KURGAST);
        assert_eq!((lying.x, lying.state), (chair.x, st::LIE));
    }
    assert!(turned);
}

/// The player standing on the floor under the `M`-box at (62, row 11)
/// (a teleport would put her on its top).
fn under_the_box(g: &mut Game) {
    g.teleport_player(61 * 16 + 8);
    g.player.x = Fx::from_px(62 * 16 + 8) - g.player.w.div_int(2);
    g.player.y = Fx::from_px(FLOOR_Y) - g.player.h;
    g.player.on_ground = true;
}

// ------------------------------------------- the approach to the arena

/// The `M`-box's Mozartkugel lands on the floor, rolls right, turns at the
/// 1-high step before the arena (flush with its face) and never enters
/// the arena; a big Resi fits under the box (no squeeze, D-106 finding).
#[test]
fn a_mozartkugel_rolling_towards_the_arena_turns_at_the_step() {
    let mut g = game();
    under_the_box(&mut g);
    until(&mut g, "J", 40, |g| {
        g.entities()
            .iter()
            .any(|e| e.kind == EntityKind::Mozartkugel)
    });
    // Out of its way: on top of the 3-high vitrine at 56–57.
    g.teleport_player(56 * 16 + 16);
    let mut max_right = Fx::ZERO;
    let mut turned = false;
    for _ in 0..600 {
        safe(&mut g, "-");
        let m = first(&g, EntityKind::Mozartkugel);
        max_right = max_right.max(m.right());
        turned |= m.vx < Fx::ZERO && max_right == Fx::from_px(STEP_X);
        assert!(m.right() <= Fx::from_px(STEP_X), "never on the step");
    }
    assert_eq!(max_right, Fx::from_px(STEP_X));
    assert!(turned, "rolls back");
    // Stage 9's layout (the box directly left of the step, 117/118): the
    // ball falls onto the step, turns at the arena's 3-high refuge and
    // never reaches the arena's floor either.
    let text = include_str!("../../../../content/levels/00_m7_cafe.level").replace(
        "..............................................................M.......................G.",
        ".................................................................M....................G.",
    );
    let mut g = Game::new(&Level::parse(&text).unwrap(), &tuning(), 1);
    g.teleport_player(64 * 16 + 8);
    g.player.x = Fx::from_px(65 * 16 + 8) - g.player.w.div_int(2);
    g.player.y = Fx::from_px(FLOOR_Y) - g.player.h;
    until(&mut g, "J", 40, |g| {
        g.entities()
            .iter()
            .any(|e| e.kind == EntityKind::Mozartkugel)
    });
    g.teleport_player(56 * 16 + 16);
    let refuge = Fx::from_px(MAX_CAMERA_X);
    let mut max_right = Fx::ZERO;
    for _ in 0..300 {
        safe(&mut g, "-");
        let m = first(&g, EntityKind::Mozartkugel);
        max_right = max_right.max(m.right());
        assert!(m.right() <= refuge, "never in the arena");
    }
    assert_eq!(max_right, refuge, "turned at the refuge");
    // A big Resi standing under the box (row 11, bottom y 192) has room.
    let mut g = game();
    under_the_box(&mut g);
    g.set_player_power(Power::Big);
    g.player.transform = 0;
    safe(&mut g, "-");
    let p = *g.player();
    assert_eq!((p.power, p.y + p.h), (Power::Big, Fx::from_px(FLOOR_Y)));
    assert!(p.y > Fx::from_px(192), "under the box with room to spare");
}

// --------------------------------------------------------- the finale

/// The bell arms on exactly the frame of the third hit: not before, not on
/// the first two hits.
#[test]
fn the_goal_stollen_arms_exactly_on_the_third_hit() {
    let mut g = game();
    lock(&mut g);
    for hit in 1..=3u8 {
        until_boss(&mut g, boss_st::BREATHE, 900);
        assert!(!g.goal_armed(0), "before hit {hit}");
        let b = boss(&g);
        drop_onto(&mut g, b);
        safe(&mut g, "-");
        assert_eq!(boss(&g).hits, hit);
        assert_eq!(g.goal_armed(0), hit == 3, "after hit {hit}");
    }
    assert_eq!(boss(&g).state, boss_st::DONE);
}

/// A respawn never arms the bell: with two hits taken she dies (the fight
/// is reset, one more hit doesn't arm it), and after he is beaten a death
/// on the timer brings him back unbeaten, the bell closed again.
#[test]
fn a_respawn_never_arms_the_goal_stollen() {
    let mut g = game();
    pass_checkpoint(&mut g);
    lock(&mut g);
    for _ in 0..2 {
        until_boss(&mut g, boss_st::BREATHE, 900);
        let b = boss(&g);
        drop_onto(&mut g, b);
        safe(&mut g, "-");
    }
    assert_eq!(boss(&g).hits, 2);
    // Killed by the timer (no hurt needed), then respawned.
    let respawn = |g: &mut Game| {
        g.time = 1;
        until(g, "-", 61, |g| matches!(g.mode(), Mode::Dying { .. }));
        until(g, "-", 400, |g| g.mode() == Mode::Playing);
    };
    respawn(&mut g);
    let b = boss(&g);
    assert_eq!(
        (b.state, b.hits, b.x),
        (boss_st::WAIT, 0, Fx::from_px(BOSS_X))
    );
    assert!(!g.goal_armed(0));
    lock(&mut g);
    until_boss(&mut g, boss_st::BREATHE, 900);
    let b = boss(&g);
    drop_onto(&mut g, b);
    safe(&mut g, "-");
    assert_eq!(boss(&g).hits, 1);
    assert!(!g.goal_armed(0), "one hit after the reset");
    // Beaten, then a death: he is back unbeaten and the bell is closed.
    let i = boss_index(&g);
    g.entities[i].state = boss_st::DONE;
    g.entities[i].hits = 3;
    assert!(g.goal_armed(0));
    respawn(&mut g);
    assert_eq!((boss(&g).state, boss(&g).hits), (boss_st::WAIT, 0));
    for _ in 0..300 {
        assert!(!g.goal_armed(0));
        safe(&mut g, "-");
    }
}

/// What the boss and his bills do from the lock for `frames` frames with
/// the player safe on the left refuge.
type Trace = Vec<(u8, u16, Fx, i8, u8, Vec<(Fx, Fx, u8)>)>;

fn trace(g: &mut Game, frames: u32) -> Trace {
    (0..frames)
        .map(|_| {
            on_refuge(g);
            safe(g, "-");
            let b = boss(g);
            let bills = bills(g).iter().map(|e| (e.x, e.y, e.state)).collect();
            (b.state, b.count, b.x, b.dir, b.hits, bills)
        })
        .collect()
}

/// A death in a volley respawns her at the checkpoint with the level
/// reset (the boss at his spawn, waiting, no hits, no bills, the `M`-box
/// whole), and the next lock rebuilds the same fight frame by frame as a
/// first lock does.
#[test]
fn a_respawn_during_a_volley_rebuilds_the_fight() {
    let fresh = {
        let mut g = game();
        pass_checkpoint(&mut g);
        lock(&mut g);
        trace(&mut g, 900)
    };
    let mut g = game();
    let box_tile = g.tile(62, 11);
    // The `M`-box bumped (its Mozartkugel out) before the fight.
    under_the_box(&mut g);
    until(&mut g, "J", 40, |g| g.tile(62, 11) != box_tile);
    pass_checkpoint(&mut g);
    lock(&mut g);
    // Hit 1 in the first breather, then round 2's breather passes: the
    // volley.
    until_boss(&mut g, boss_st::BREATHE, 900);
    let b = boss(&g);
    drop_onto(&mut g, b);
    safe(&mut g, "-");
    until_boss(&mut g, boss_st::VOLLEY, 900);
    while !bills(&g).iter().any(|b| b.state == rechnung::FALL) {
        on_refuge(&mut g);
        safe(&mut g, "-");
    }
    // A Normal Resi under a falling bill dies.
    let bill = *bills(&g)
        .iter()
        .find(|b| b.state == rechnung::FALL)
        .unwrap();
    assert_eq!(g.player().power, Power::Normal);
    g.player.x = bill.x;
    g.player.y = bill.y;
    g.player.on_ground = false;
    g.player.invulnerable = 0;
    let events = g.step(input("-")).to_vec();
    assert!(events.contains(&Event::Died), "{events:?}");
    until(&mut g, "-", 400, |g| g.mode() == Mode::Playing);
    // Respawned at the checkpoint (column 59) with the level reset.
    assert_eq!(g.player().center_px().0 / 16, 59);
    let b = boss(&g);
    assert_eq!(
        (b.state, b.count, b.hits, b.x, b.active),
        (boss_st::WAIT, 0, 0, Fx::from_px(BOSS_X), false)
    );
    assert!(bills(&g).is_empty());
    assert_eq!(g.tile(62, 11), box_tile, "the M-box whole");
    assert_eq!(g.stage_clock(), 0);
    lock(&mut g);
    assert_eq!(trace(&mut g, 900), fresh, "the same fight");
}

/// A thrown Hackl passes through the stunned boss: no hit, no bounce, the
/// axe flies on into the wall behind him.
#[test]
fn a_hackl_axe_through_the_stun_passes() {
    let mut g = game();
    lock(&mut g);
    g.set_player_power(Power::Hackl);
    g.player.transform = 0;
    until_boss(&mut g, boss_st::BREATHE, 900);
    let b = boss(&g);
    drop_onto(&mut g, b);
    safe(&mut g, "-");
    assert_eq!((boss(&g).state, boss(&g).hits), (boss_st::STUN, 1));
    assert_eq!(g.player().power, Power::Hackl);
    // On the floor right beside him, facing him: RUN throws.
    let b = boss(&g);
    g.player.x = b.right() + Fx::ONE;
    g.player.y = Fx::from_px(FLOOR_Y) - g.player.h;
    g.player.vx = Fx::ZERO;
    g.player.vy = Fx::ZERO;
    g.player.on_ground = true;
    g.player.facing = Facing::Left;
    let events = safe(&mut g, "B");
    assert!(events.contains(&Event::HacklThrown));
    let mut overlapped = false;
    for _ in 0..12 {
        assert_eq!(boss(&g).state, boss_st::STUN);
        let a = first(&g, EntityKind::ThrownHackl);
        overlapped |= a.state == axe::FLY && a.hitbox().overlaps(&boss(&g).hitbox());
        assert_eq!(boss(&g).axe_hit(), AxeHit::Pass);
        // Not blinking: she stands beside him, unhurt (no contact).
        g.player.invulnerable = 0;
        let events = g.step(input("-")).to_vec();
        assert!(!events.contains(&Event::AxeHit), "passes");
        assert!(!events.contains(&Event::Hurt), "no contact in the stun");
    }
    assert!(overlapped, "the axe flew through him");
    assert_eq!(boss(&g).hits, 1, "one hit per breather");
    assert_eq!(first(&g, EntityKind::ThrownHackl).state, axe::STUCK);
}

// ---------------------------------------------------------- the camera

/// A run whose last station (Café Zauner, station 9) is the grey box, in
/// the real flow from the title.
fn cafe_run() -> Game {
    let mut levels: Vec<Level> = (0..8)
        .map(|i| {
            let mut l = level(&["......G.", "..P.....", "########"]);
            l.meta.id = format!("s{i}");
            l
        })
        .collect();
    levels.push(cafe());
    let route = Arc::new(Route::new(levels).unwrap());
    Game::new_run(route, 8, &tuning(), 1, Start::Title)
}

fn mode_name(g: &Game) -> &'static str {
    match g.mode() {
        Mode::Title { .. } => "title",
        Mode::StageIntro { .. } => "intro",
        Mode::Playing => "playing",
        Mode::Paused => "paused",
        Mode::Dying { .. } => "dying",
        Mode::StageClear => "stage_clear",
        Mode::RouteMap => "route_map",
        Mode::Ending => "ending",
        Mode::GameOver { .. } => "game_over",
    }
}

/// Steps once and checks the camera's y; records the mode.
fn fixed_step(g: &mut Game, token: &str, seen: &mut Vec<&'static str>) -> Vec<Event> {
    let events = g.step(input(token)).to_vec();
    assert_eq!(g.level().meta.id, "m7_cafe");
    assert_eq!(
        g.camera().y,
        CAMERA_Y,
        "frame {} in {}",
        g.frame(),
        mode_name(g)
    );
    let m = mode_name(g);
    if !seen.contains(&m) {
        seen.push(m);
    }
    events
}

/// D-104: in a `camera_top = 92` level the camera stays at y 92 in every
/// phase: the title, the intro, play (the whole solver route, the arena
/// lock and the fight), the pause, the goal sequence (Serve, Tally,
/// Clear), every phase of the ending and the marked title after it.
#[test]
fn the_camera_stays_at_92_through_the_run_and_the_ending() {
    let route = Replay::parse(include_str!(
        "../../../../tests/replays/m7_cafe_solver.replay"
    ))
    .unwrap();
    let mut g = cafe_run();
    let mut seen = Vec::new();
    assert_eq!(g.camera().y, CAMERA_Y, "the title's framing");
    for _ in 0..10 {
        fixed_step(&mut g, "-", &mut seen);
    }
    fixed_step(&mut g, "S", &mut seen);
    while g.mode() != Mode::Playing {
        fixed_step(&mut g, "-", &mut seen);
    }
    // A pause before the first step of play.
    fixed_step(&mut g, "S", &mut seen);
    assert_eq!(g.mode(), Mode::Paused);
    for _ in 0..10 {
        fixed_step(&mut g, "-", &mut seen);
    }
    fixed_step(&mut g, "S", &mut seen);
    assert_eq!(g.mode(), Mode::Playing);
    let mut completed = false;
    for i in route.frames() {
        let events = fixed_step(&mut g, &i.to_token(), &mut seen);
        completed |= events.contains(&Event::LevelComplete);
    }
    assert!(completed, "the route completes in the real flow");
    assert_eq!(boss_hits(g.entities()), Some(3));
    let mut phases = Vec::new();
    while !matches!(g.mode(), Mode::Title { .. }) {
        fixed_step(&mut g, "-", &mut seen);
        if let Some(e) = g.ending()
            && !phases.contains(&e.phase)
        {
            phases.push(e.phase);
        }
        assert!(g.frame() < 20_000, "the ending ends");
    }
    assert_eq!(
        phases,
        [
            EndingPhase::Map,
            EndingPhase::Scene,
            EndingPhase::Bill,
            EndingPhase::Credits,
            EndingPhase::Ende
        ]
    );
    assert!(g.cleared());
    for _ in 0..30 {
        fixed_step(&mut g, "-", &mut seen);
    }
    assert_eq!(
        seen,
        [
            "title",
            "intro",
            "playing",
            "paused",
            "stage_clear",
            "ending"
        ]
    );
}

/// The same through deaths: dying in the arena (camera at its right
/// clamp), the intro after the respawn, the game over and the continue.
#[test]
fn the_camera_stays_at_92_through_deaths_and_the_game_over() {
    let mut g = cafe_run();
    let mut seen = Vec::new();
    fixed_step(&mut g, "S", &mut seen);
    while g.mode() != Mode::Playing {
        fixed_step(&mut g, "-", &mut seen);
    }
    // Into the arena: the camera at its right clamp.
    g.teleport_player(80 * 16 + 8);
    assert_eq!(g.camera().x, MAX_CAMERA_X);
    assert_eq!(g.camera().y, CAMERA_Y);
    for round in 0..2 {
        if round == 1 {
            g.lives = 1;
        }
        g.time = 1;
        while !matches!(g.mode(), Mode::Dying { .. }) {
            fixed_step(&mut g, "-", &mut seen);
        }
        let mut n = 0;
        while g.mode() != Mode::Playing {
            // START presses only where they are wanted: the continue.
            let token = if matches!(g.mode(), Mode::GameOver { .. }) && n % 2 == 0 {
                "S"
            } else {
                "-"
            };
            fixed_step(&mut g, token, &mut seen);
            n += 1;
            assert!(n < 2000);
        }
        for _ in 0..30 {
            fixed_step(&mut g, "R", &mut seen);
        }
    }
    assert_eq!(
        seen,
        ["intro", "playing", "dying", "game_over"],
        "every phase seen"
    );
}

// ----------------------------------------------------- classification

/// D-105/D-106's tables, by kind: axes bounce off the Kellner's and the
/// boss's trays, defeat a Wartender (a Selfie-Tourist), pass a bill; on
/// foot, all of them are lost in swim water; the bill is not an enemy.
#[test]
fn every_new_kind_is_classified_for_axes_and_swim_water() {
    assert_eq!(EnemyKind::Kellner.axe_hit(), AxeHit::Bounce);
    assert_eq!(EnemyKind::Oberkellner.axe_hit(), AxeHit::Bounce);
    assert_eq!(EnemyKind::SelfieTourist.axe_hit(), AxeHit::Defeat);
    assert_eq!(
        crate::enemies::new_bill(70, &tuning()).axe_hit(),
        AxeHit::Pass
    );
    for k in [
        EnemyKind::Kellner,
        EnemyKind::Oberkellner,
        EnemyKind::SelfieTourist,
    ] {
        assert!(k.sinks_in_swim_water(), "{k:?}");
    }
}

/// In play: a Kellner, a Wartender and an Oberkellner placed in swim water
/// are gone on their first frame (D-076: walkers are lost in it).
#[test]
fn kellner_wartender_and_oberkellner_are_lost_in_swim_water() {
    // Each stands in a swim-water cell on the floor (its lake bed).
    let l = world(40, &[(11, 10, 'S'), (11, 14, 'W'), (11, 18, 'O')]);
    let mut g = Game::new(&l, &tuning(), 1);
    assert_eq!(
        g.entities()
            .iter()
            .filter(|e| matches!(e.kind, EntityKind::Enemy(_)))
            .count(),
        3
    );
    safe(&mut g, "-");
    assert!(
        !g.entities()
            .iter()
            .any(|e| matches!(e.kind, EntityKind::Enemy(_))),
        "all lost"
    );
}

/// E51/E53: in Stage 9's arena (camera y 92) the Serve's popups, spawned
/// over the plate as she stands at the bell on the counter, stay below the
/// HUD (rows 0–21) on every frame, for every form: a Normal Resi's pair
/// rises freely (its highest drawn row, the text's outline, is 25 on its
/// last frame, risen 15 px); a Big or Hackl Resi holds the plate 8 px
/// higher, so her pair stops rising at row 22 (E53: in QA's
/// `qa_m7a_stollen_popup_in_hud_big` it reached row 16).
#[test]
fn the_stollen_popups_stay_clear_of_the_hud_in_the_stage_9_arena() {
    let t = tuning();
    let level = Level::parse(include_str!(
        "../../../../content/levels/09_cafe_zauner.level"
    ))
    .unwrap();
    for (power, want_highest) in [(Power::Normal, 25), (Power::Big, 22), (Power::Hackl, 22)] {
        let mut g = Game::new(&level, &t, 1);
        let i = boss_index(&g);
        g.entities[i].state = boss_st::DONE;
        g.entities[i].hits = 3;
        assert!(g.goal_armed(0));
        g.set_player_power(power);
        g.player.transform = 0;
        let (col, row) = g.goal_spawn(0).map(|s| (s.col, s.row)).unwrap();
        g.camera.x = level.width * 16 - crate::camera::SCREEN_W;
        g.player.x = Fx::from_px(col * 16 + 2);
        g.player.y = Fx::from_px((row + 1) * 16) - g.player.h;
        g.player.vx = Fx::ZERO;
        g.player.vy = Fx::ZERO;
        g.player.on_ground = true;
        let events = safe(&mut g, "-");
        assert!(
            events.contains(&Event::LevelComplete),
            "{power:?}: {events:?}"
        );
        assert_eq!(g.camera().y, CAMERA_Y);
        let (mut highest, mut frames) = (i32::MAX, 0);
        while let Some(text) = g
            .entities()
            .iter()
            .find(|e| e.kind == EntityKind::Popup(crate::Popup::Stollen))
            .copied()
        {
            frames += 1;
            let top = text.y.to_px() - text.popup_rise_px(&t) - 1 - g.camera().y;
            assert!(
                top >= crate::game::HUD_CLEAR_Y,
                "{power:?}: drawn from row {top}"
            );
            highest = highest.min(top);
            g.step(crate::InputFrame::NONE);
        }
        assert!(
            frames >= t.score.popup_frames,
            "{power:?}: up {frames} frames"
        );
        assert_eq!(highest, want_highest, "{power:?}");
    }
}
