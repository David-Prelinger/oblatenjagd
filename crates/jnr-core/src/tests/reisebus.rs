//! The Reisebus (D-070; game-design.md → "Stage 5", "New mechanics"):
//! trigger, telegraph, driving through tiles, the photo stop, roof bounce
//! and side hits in every state, removal off screen, the axe bounce, its
//! `stop` field and the lane lint.

use super::*;
use crate::enemies::{Pose, REISEBUS_LANE_TILES, reisebus_lane_problems, state};
use crate::hackl::AxeHit;
use crate::player::Power;
use crate::{EnemyKind, Entity, EntityKind, Fx, SpawnKind};

/// Legend of the test worlds (`.` air, `P` start).
const LEGEND: &str = r##"
"#" = "solid"
"-" = "oneway"
"r" = { entity = "reisebus" }
"s" = { entity = "reisebus", stop = 4 }
"w" = { entity = "nordic_walker" }
"H" = { tile = "oblaten_box", contains = "hackl" }
"P" = { entity = "player_start" }
"G" = { entity = "goal" }
"##;

const BUS: EntityKind = EntityKind::Enemy(EnemyKind::Reisebus);

/// The level text of a 14-row world `width` columns wide: ground in rows
/// 12–13 (feet at y 192), the player start at column 2 unless `marks`
/// places a `P`, plus `marks` as (row, col, char).
fn world_text(width: usize, marks: &[(usize, usize, char)]) -> String {
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
    format!(
        "[meta]\nid = \"bus\"\ntime_limit = 300\n[legend]\n{LEGEND}\n[map]\ngrid = '''\n{}\n'''\n",
        grid.join("\n")
    )
}

fn world(width: usize, marks: &[(usize, usize, char)]) -> Level {
    Level::parse(&world_text(width, marks)).unwrap()
}

fn game(level: &Level) -> Game {
    Game::new(level, &tuning(), 1)
}

fn find(g: &Game) -> Option<Entity> {
    g.entities().iter().find(|e| e.kind == BUS).copied()
}

fn bus(g: &Game) -> Entity {
    find(g).expect("a Reisebus")
}

fn index(g: &Game) -> usize {
    g.entities().iter().position(|e| e.kind == BUS).unwrap()
}

/// Steps once with the player kept invulnerable.
fn safe_step(g: &mut Game, token: &str) -> Vec<Event> {
    g.player.invulnerable = u8::MAX;
    g.step(input(token)).to_vec()
}

/// Puts the player's hitbox centre `d` px left of the bus's centre,
/// standing still on the ground.
fn stand_left_of(g: &mut Game, d: i32) {
    let b = bus(g);
    let centre = b.x + b.w.div_int(2);
    g.player.x = centre - Fx::from_px(d) - g.player.w.div_int(2);
    g.player.vx = Fx::ZERO;
}

/// A bus (spawned at column 20, active) put into `st` with its counter at 1.
fn bus_in_state(st: u8) -> Game {
    let mut g = game(&world(60, &[(11, 20, 'r')]));
    let i = index(&g);
    g.entities[i].active = true;
    g.entities[i].state = st;
    g.entities[i].count = 1;
    g
}

#[test]
fn a_parked_bus_covers_four_cells_from_its_spawn() {
    let g = game(&world(40, &[(11, 20, 'r')]));
    let b = bus(&g);
    // Hitbox 60×28 centred under the 64 px sprite over columns 20–23.
    assert_eq!((b.x, b.w), (Fx::from_px(322), Fx::from_px(60)));
    assert_eq!((b.y, b.h), (Fx::from_px(164), Fx::from_px(28)));
    assert_eq!(b.base_x, b.x, "remembers where it was parked");
    assert_eq!(b.state, state::PARKED);
    assert_eq!(b.pose(g.tuning()), Some(Pose::Idle));
    assert!(b.facing_left());
    assert!(!EnemyKind::Reisebus.stompable());
    assert!(!EnemyKind::Reisebus.hurts_from_above());
}

#[test]
fn it_triggers_within_160_px_of_its_centre() {
    let mut g = game(&world(40, &[(11, 20, 'r')]));
    g.step(input("-"));
    stand_left_of(&mut g, 161);
    g.step(input("-"));
    assert_eq!(bus(&g).state, state::PARKED, "161 px: parked");
    stand_left_of(&mut g, 160);
    g.step(input("-"));
    let b = bus(&g);
    assert_eq!(
        (b.state, b.pose(g.tuning())),
        (state::TELEGRAPH, Some(Pose::Telegraph))
    );
}

#[test]
fn it_telegraphs_60_frames_then_drives_left_at_1_5_through_walls() {
    // A 2-tile wall in its lane at columns 14–15: it drives straight on.
    let marks = [(10, 14, '#'), (11, 14, '#'), (10, 15, '#'), (11, 15, '#')];
    let mut marks = marks.to_vec();
    marks.push((11, 20, 'r'));
    let mut g = game(&world(60, &marks));
    g.step(input("-"));
    let parked = bus(&g);
    // Trigger from 150 px, then the player stays at the far left.
    stand_left_of(&mut g, 150);
    safe_step(&mut g, "-");
    assert_eq!(bus(&g).state, state::TELEGRAPH);
    g.player.x = Fx::from_px(8);
    let mut telegraph = 1;
    loop {
        safe_step(&mut g, "-");
        let b = bus(&g);
        if b.state != state::TELEGRAPH {
            break;
        }
        assert_eq!(b.x, parked.x, "stands while telegraphing");
        telegraph += 1;
    }
    assert_eq!(telegraph, 60);
    let mut b = bus(&g);
    assert_eq!(b.pose(g.tuning()), Some(Pose::Drive));
    assert_eq!(b.x, parked.x - Fx(384), "1.5 px/frame left");
    assert_eq!(b.y, parked.y);
    // Through the wall (right edge 256) without slowing down.
    while b.x > Fx::from_px(200) {
        let x = b.x;
        safe_step(&mut g, "-");
        b = bus(&g);
        assert_eq!(b.x, x - Fx(384));
        assert_eq!((b.y, b.state), (parked.y, state::DRIVE));
    }
}

#[test]
fn it_is_removed_once_its_right_edge_is_left_of_the_screen() {
    // The camera scrolled to the player at 600 px and stays there; the
    // driving bus starts 10 px inside the screen's left edge.
    let mut g = game(&world(80, &[(11, 50, 'r')]));
    g.player.x = Fx::from_px(600);
    hold(&mut g, "-", 2);
    let left = Fx::from_px(g.camera().x);
    assert!(left > Fx::ZERO);
    let i = index(&g);
    g.entities[i].state = state::DRIVE;
    g.entities[i].x = left + Fx::from_px(10);
    let mut frames = 0;
    while find(&g).is_some() {
        let before = bus(&g);
        safe_step(&mut g, "-");
        assert_eq!(Fx::from_px(g.camera().x), left);
        frames += 1;
        assert!(frames < 100);
        if find(&g).is_none() {
            assert!(before.right() - Fx(384) <= left, "only off screen");
            assert!(before.right() > left);
        }
    }
}

#[test]
fn a_photo_stop_halts_it_120_frames_with_flashes_then_30_telegraph() {
    let t = tuning();
    let mut g = game(&world(60, &[(11, 20, 's')]));
    let i = index(&g);
    g.entities[i].active = true;
    g.entities[i].state = state::DRIVE;
    let stop_x = bus(&g).base_x - Fx::from_px(64);
    g.player.x = Fx::from_px(8);
    until(&mut g, "-", 100, |g| bus(g).state != state::DRIVE);
    // Exactly 4 tiles left of where it was parked (64 px is not a whole
    // number of 1.5 px steps: it is clamped there).
    let b = bus(&g);
    assert_eq!((b.state, b.x, b.count), (state::PHOTO_STOP, stop_x, 1));
    let mut flashes = Vec::new();
    let mut stop = 1;
    while let Some(Pose::Fotostopp { flash }) = bus(&g).pose(&t) {
        flashes.push(flash);
        safe_step(&mut g, "-");
        assert_eq!(bus(&g).x, stop_x, "stands");
        if bus(&g).state == state::PHOTO_STOP {
            stop += 1;
        }
    }
    assert_eq!(stop, 120);
    // Off for 16 frames, on for 16, … (the catalog's 2f/16t `fotostopp`).
    assert!(flashes[..16].iter().all(|f| !f) && flashes[16..32].iter().all(|f| *f));
    assert!(!flashes[32]);
    let b = bus(&g);
    assert_eq!(
        (b.state, b.pose(&t), b.count),
        (state::RESTART, Some(Pose::Telegraph), 1)
    );
    let mut restart = 1;
    loop {
        safe_step(&mut g, "-");
        if bus(&g).state != state::RESTART {
            break;
        }
        assert_eq!(bus(&g).x, stop_x);
        restart += 1;
    }
    assert_eq!(restart, 30);
    // On again, and it never stops a second time.
    assert_eq!(bus(&g).x, stop_x - Fx(384));
    for _ in 0..60 {
        safe_step(&mut g, "-");
        assert_eq!(bus(&g).state, state::DRIVE);
    }
}

#[test]
fn a_bus_without_stop_never_halts() {
    let mut g = bus_in_state(state::DRIVE);
    g.player.x = Fx::from_px(8);
    for _ in 0..150 {
        safe_step(&mut g, "-");
        assert_eq!(bus(&g).state, state::DRIVE);
    }
}

#[test]
fn its_roof_bounces_without_damage_or_points_in_every_state() {
    for st in [
        state::PARKED,
        state::TELEGRAPH,
        state::DRIVE,
        state::PHOTO_STOP,
        state::RESTART,
    ] {
        let mut g = bus_in_state(st);
        let b = bus(&g);
        let x = b.x + b.w.div_int(2) - g.player.w.div_int(2);
        g.player.x = x;
        g.player.y = b.y - Fx::from_px(2) - g.player.h;
        g.player.vx = Fx::ZERO;
        g.player.vy = Fx::from_px(3);
        g.player.on_ground = false;
        let events = g.step(input("-")).to_vec();
        assert!(events.contains(&Event::Stomped), "{st}: {events:?}");
        assert!(!events.contains(&Event::Hurt) && !events.contains(&Event::Died));
        assert_eq!(g.score(), 0, "{st}: no points");
        assert!(g.player().vy < Fx::ZERO, "{st}: bounced up");
        assert_eq!(
            g.player().y + g.player().h,
            bus(&g).y,
            "{st}: feet on its roof"
        );
        assert_eq!(bus(&g).kind, BUS, "{st}: not defeated");
        // The rising frames after the bounce are no side hit.
        for _ in 0..20 {
            let events = g.step(input("-")).to_vec();
            assert!(!events.contains(&Event::Died), "{st}: {events:?}");
        }
    }
}

#[test]
fn a_side_hit_hurts_in_every_state() {
    for st in [
        state::PARKED,
        state::TELEGRAPH,
        state::DRIVE,
        state::PHOTO_STOP,
        state::RESTART,
    ] {
        let mut g = bus_in_state(st);
        g.player.set_power(Power::Big, &tuning().player);
        g.player.transform = 0;
        let b = bus(&g);
        // Standing on the ground, overlapping its left side.
        g.player.x = b.x - g.player.w + Fx::from_px(3);
        g.player.vx = Fx::ZERO;
        let events = g.step(input("-")).to_vec();
        assert!(events.contains(&Event::Hurt), "{st}: {events:?}");
        assert_eq!(g.player().power, Power::Normal, "{st}");
        assert_eq!(g.score(), 0);
    }
}

#[test]
fn walkers_turn_at_a_standing_bus_but_not_at_a_driving_one() {
    for (st, turns) in [
        (state::PARKED, true),
        (state::PHOTO_STOP, true),
        (state::DRIVE, false),
    ] {
        // A Nordic-Walker walking right into the bus's left side.
        let mut g = bus_in_state(st);
        let b = bus(&g);
        let t = tuning();
        let mut w = Entity::at_feet(
            EntityKind::Enemy(EnemyKind::NordicWalker),
            b.x - Fx::from_px(5),
            Fx::from_px(192),
            t.enemies.nordic_walker_hitbox,
        );
        w.dir = 1;
        w.timer = 5;
        g.entities.push(w);
        g.player.x = Fx::from_px(8);
        safe_step(&mut g, "-");
        let walker = g
            .entities()
            .iter()
            .find(|e| e.kind == EntityKind::Enemy(EnemyKind::NordicWalker))
            .unwrap();
        assert_eq!(walker.dir == -1, turns, "state {st}");
    }
}

#[test]
fn the_hackl_bounces_off_a_bus() {
    let t = tuning();
    let mut b = bus(&game(&world(40, &[(11, 20, 'r')])));
    assert_eq!(b.axe_hit(), AxeHit::Bounce);
    for st in [state::DRIVE, state::PHOTO_STOP] {
        b.state = st;
        assert_eq!(b.axe_hit(), AxeHit::Bounce, "immune in state {st}");
    }
    b.active = true;
    let before = b;
    assert_eq!(crate::enemies::hit_by_projectile(&mut b, &t), None);
    assert_eq!(b, before, "nothing happens to it");
    // In play: the axe thrown at a parked bus 2 tiles ahead is gone, the
    // bus stays, no points.
    let mut g = game(&world(40, &[(11, 7, 'r')]));
    g.player.set_power(Power::Hackl, &t.player);
    g.player.transform = 0;
    let i = index(&g);
    g.entities[i].active = true;
    g.entities[i].timer = 1;
    let mut events = safe_step(&mut g, "B");
    for _ in 0..40 {
        events.extend(safe_step(&mut g, "-"));
    }
    assert!(events.contains(&Event::HacklThrown), "{events:?}");
    assert!(events.contains(&Event::AxeHit), "{events:?}");
    assert!(
        !g.entities()
            .iter()
            .any(|e| e.kind == EntityKind::ThrownHackl),
        "bounced off"
    );
    assert_eq!(bus(&g).kind, BUS);
    assert_eq!(g.score(), 0);
}

#[test]
fn the_stop_field_parses_and_is_validated() {
    let stop = |field: &str| {
        let text = world_text(30, &[(11, 20, 'x')]).replace(
            "[legend]\n",
            &format!("[legend]\n\"x\" = {{ entity = \"reisebus\"{field} }}\n"),
        );
        Level::parse(&text).map(|l| {
            let s = l.entities.iter().find(|s| s.kind == SpawnKind::Reisebus);
            s.unwrap().stop
        })
    };
    assert_eq!(stop(""), Ok(0), "no stop by default");
    assert_eq!(stop(", stop = 1"), Ok(1));
    assert_eq!(stop(", stop = 16"), Ok(16));
    for bad in [", stop = 0", ", stop = 17", ", stop = -4", ", stop = \"4\""] {
        let e = stop(bad).unwrap_err().to_string();
        assert!(
            e.contains("field \"stop\" must be a whole number in 1..=16"),
            "{bad}: {e}"
        );
    }
    let e = stop(", phase = 4").unwrap_err().to_string();
    assert!(e.contains("field \"phase\" is not allowed"), "{e}");
    let on_fiaker = world_text(30, &[]).replace(
        "[legend]\n",
        "[legend]\n\"x\" = { entity = \"fiaker\", stop = 4 }\n",
    );
    let e = Level::parse(&on_fiaker).unwrap_err().to_string();
    assert!(e.contains("field \"stop\" is not allowed"), "{e}");
}

#[test]
fn the_lane_lint_wants_24_clear_tiles_left_of_a_bus() {
    let t = tuning();
    let lint = |marks: &[(usize, usize, char)]| reisebus_lane_problems(&world(60, marks), &t);
    assert_eq!(REISEBUS_LANE_TILES, 24);
    // Bus at column 30: its lane is rows 10–11, columns 6–33.
    assert!(lint(&[(11, 30, 'r')]).is_empty());
    // Just outside the lane: 25 tiles left, a row above, the ground below.
    assert!(lint(&[(11, 30, 'r'), (11, 5, '#'), (9, 20, '#'), (9, 31, '-')]).is_empty());
    // Closer to the level's left edge than 24 tiles: up to the edge.
    assert!(lint(&[(11, 10, 'r')]).is_empty());
    let p = lint(&[(11, 30, 'r'), (10, 6, '-'), (11, 12, '#')]);
    assert_eq!(p.len(), 1, "one problem per bus: {p:?}");
    assert!(
        p[0].starts_with(
            "reisebus at row 12, column 31: its lane (rows 11-12, columns 7-34) has a solid or one-way tile at row 11, column 7"
        ),
        "{p:?}"
    );
    // Its own cells count too, and the left edge clamps the lane.
    let p = lint(&[(11, 10, 'r'), (10, 13, '#')]);
    assert!(
        p[0].contains("columns 1-14") && p[0].contains("row 11, column 14"),
        "{p:?}"
    );
    let p = lint(&[(11, 10, 'r'), (11, 0, '#'), (11, 40, 's'), (11, 30, '#')]);
    assert_eq!(p.len(), 2, "{p:?}");
}
