//! The Wolfgangsee boats (D-077, game-design.md → "Stage 6: Wolfgangsee",
//! "New mechanics"): the free-running Traundl (the Gondel's machine with a
//! 48×16 hull) and the Dampfer (the Festungsbahn's waiting machine, one
//! way, 20 frames per tile) with its paddle wheel.
//!
//! Swim water (E27) doesn't exist yet: the lake here is the lethal
//! `wolfgangsee_water` (// E27: Swim), and the wheel is tested against a
//! player standing in a dry dock under the hull (the spec: any player
//! overlapping it).

use super::*;
use crate::platform::{CarTiming, car, car_timing, free_period, free_timing, gondel_progress};
use crate::player::Power;
use crate::{Entity, EntityKind, Fx};

/// A 17-row lake world (decks on row 11 = y 176, the water line y 192):
/// the quay (`#`, columns 0–3), the Dampfer `D` in cell (4, 11) going
/// `to = [8, 0]` (hull columns 4–13 → 12–21), a dry dock under its track
/// (columns 4–21: air on row 12, its floor row 13 = y 208), the quay again
/// from column 22 (deck flush with the moored ship), a Traundl `T` in cell
/// (26, 11) going `to = [6, 0]` (hull columns 26–28 → 32–34) over water
/// (`~`, rows 12–15, the lake bed row 16) and the far quay from 35.
fn lake_world() -> Level {
    let rows = [
        "........................................", // row 0
        "........................................",
        "........................................",
        "........................................",
        "........................................",
        "........................................",
        "........................................",
        "........................................",
        "........................................",
        "........................................",
        ".P...................................G..", // row 10
        "####D.................####T........#####", // row 11: decks
        "####..................####~~~~~~~~~#####", // row 12: the water line
        "##########################~~~~~~~~~#####", // row 13: the dock floor
        "##########################~~~~~~~~~#####",
        "##########################~~~~~~~~~#####",
        "########################################", // row 16: the lake bed
    ];
    let text = format!(
        "[meta]\nid = \"boats\"\ntime_limit = 300\n[legend]\n\"#\" = \"solid\"\n\"~\" = \"wolfgangsee_water\"\n\"P\" = {{ entity = \"player_start\" }}\n\"G\" = {{ entity = \"goal\" }}\n\"D\" = {{ entity = \"dampfer\", to = [8, 0] }}\n\"T\" = {{ entity = \"traundl\", to = [6, 0], phase = {PHASE} }}\n[map]\ngrid = '''\n{}\n'''\n",
        rows.join("\n")
    );
    Level::parse(&text).unwrap()
}

/// The Traundl's `phase` in [`lake_world`].
const PHASE: u16 = 100;

fn find(g: &Game, kind: EntityKind) -> Entity {
    *g.entities()
        .iter()
        .find(|e| e.kind == kind)
        .unwrap_or_else(|| panic!("a {kind:?}"))
}

fn ship(g: &Game) -> Entity {
    find(g, EntityKind::Dampfer)
}

fn boat(g: &Game) -> Entity {
    find(g, EntityKind::Traundl)
}

/// Puts the player standing on `e`'s deck, `dx` px right of its left edge.
fn board(g: &mut Game, e: Entity, dx: i32) {
    g.player.x = e.x + Fx::from_px(dx);
    g.player.y = e.y - g.player.h;
    g.player.vx = Fx::ZERO;
    g.player.vy = Fx::ZERO;
    g.player.on_ground = true;
}

/// Puts the player standing on the dock floor (y 208) at `x` px.
fn stand_in_dock(g: &mut Game, x: i32) {
    g.player.x = Fx::from_px(x);
    g.player.y = Fx::from_px(208) - g.player.h;
    g.player.vx = Fx::ZERO;
    g.player.vy = Fx::ZERO;
    g.player.on_ground = true;
}

#[test]
fn boats_spawn_left_aligned_with_their_decks_flush_on_row_11() {
    let t = tuning();
    let g = Game::new(&lake_world(), &t, 1);
    let (s, b) = (ship(&g), boat(&g));
    assert_eq!((s.w, s.h), (Fx::from_px(160), Fx::from_px(16)));
    assert_eq!((s.x, s.y), (Fx::from_px(64), Fx::from_px(176)));
    assert_eq!(s.state, car::WAIT_LOWER);
    assert_eq!((b.w, b.h), (Fx::from_px(48), Fx::from_px(16)));
    assert_eq!(b.y, Fx::from_px(176), "deck flush with the quay");
    assert_eq!(b.base_x, Fx::from_px(26 * 16));
    assert_eq!(b.count, PHASE, "placed at its phase");
}

#[test]
fn the_traundl_runs_the_gondel_cycle_with_its_own_size_and_never_waits() {
    let t = tuning();
    let level = lake_world();
    let spawn = level
        .entities
        .iter()
        .find(|s| s.kind == crate::SpawnKind::Traundl)
        .unwrap();
    assert_eq!(
        free_timing(EntityKind::Traundl, spawn, &t),
        CarTiming {
            travel: 96,
            wait: 30,
            moor: false
        }
    );
    // game-design.md: a Traundl with to = [6, 0] has a period of 252.
    assert_eq!(free_period(EntityKind::Traundl, spawn, &t), 252);
    let base = Fx::from_px(26 * 16);
    let at = |k: u16| base + Fx::from_px(i32::from(gondel_progress(k % 252, 96, 30)));
    // Without a rider and with one on board from the start: the same path.
    let mut alone = Game::new(&level, &t, 1);
    let mut ridden = Game::new(&level, &t, 1);
    ridden.step(input("-"));
    alone.step(input("-"));
    let b = boat(&ridden);
    board(&mut ridden, b, 16);
    for i in 0..(2 * 252) {
        alone.step(input("-"));
        ridden.step(input("-"));
        let k = PHASE + 2 + i as u16;
        let (a, r) = (boat(&alone), boat(&ridden));
        assert_eq!(a.x, at(k), "frame {i}");
        assert_eq!(a.y, Fx::from_px(176), "never bobs, frame {i}");
        assert_eq!((r.x, r.y), (a.x, a.y), "a rider changes nothing, frame {i}");
    }
}

#[test]
fn a_traundl_carries_its_rider_exactly_over_a_whole_period() {
    let mut g = Game::new(&lake_world(), &tuning(), 1);
    g.step(input("-"));
    let b = boat(&g);
    board(&mut g, b, 12);
    for f in 0..252 {
        let events = g.step(input("-")).to_vec();
        let (b, p) = (boat(&g), *g.player());
        assert_eq!(p.y + p.h, b.y, "feet on the deck, frame {f}");
        assert_eq!(p.x - b.x, Fx::from_px(12), "no sliding, frame {f}");
        assert!(p.on_ground, "frame {f}");
        assert!(!events.contains(&Event::Landed), "frame {f}");
        assert!(!events.contains(&Event::Died), "frame {f}");
    }
}

#[test]
fn the_dampfer_waits_for_its_rider_travels_20_frames_per_tile_and_stays_moored() {
    let t = tuning();
    let mut g = Game::new(&lake_world(), &t, 1);
    let start = ship(&g);
    let spawn = &g.level().entities[usize::from(start.spawn.unwrap())];
    let timing = car_timing(EntityKind::Dampfer, Some(spawn), &t);
    assert_eq!(
        timing,
        CarTiming {
            travel: 160,
            wait: 0,
            moor: true
        }
    );
    // Nobody on board: it waits, however long.
    for _ in 0..600 {
        g.step(input("-"));
        assert_eq!((ship(&g).x, ship(&g).state), (start.x, car::WAIT_LOWER));
    }
    board(&mut g, start, 70);
    let mut path = Vec::new();
    for _ in 0..(160 + 2000) {
        g.step(input("-"));
        let s = ship(&g);
        path.push((s.x, s.y, s.state));
    }
    // path[i] is after update i + 1: 0.8 px per frame on the straight line.
    for (i, &(x, y, state)) in path[..160].iter().enumerate() {
        let k = i as i32 + 1;
        assert_eq!(x, start.x + Fx::from_px(128).mul_int(k).div_int(160), "{i}");
        assert_eq!((y, state), (start.y, car::UP), "{i}");
    }
    let end = start.x + Fx::from_px(128);
    assert_eq!(path[159].0, end, "arrives after 8 × 20 frames");
    // Moored for good: no wait-and-return, rider or not.
    for (i, &(x, y, state)) in path[160..].iter().enumerate() {
        assert_eq!((x, y, state), (end, start.y, car::MOORED), "{i}");
    }
    assert_eq!(
        g.player().y + g.player().h,
        start.y,
        "the rider is still on"
    );
    g.player.x = Fx::from_px(8);
    hold(&mut g, "-", 300);
    assert_eq!((ship(&g).x, ship(&g).state), (end, car::MOORED));
}

#[test]
fn the_paddle_wheel_hurts_a_player_under_the_hull_while_it_travels() {
    let t = tuning();
    let mut g = Game::new(&lake_world(), &t, 1);
    g.player.set_power(Power::Big, &t.player);
    g.player.transform = 0;
    g.step(input("-"));
    let s = ship(&g);
    board(&mut g, s, 4);
    g.step(input("-"));
    assert_eq!(ship(&g).state, car::UP, "left with the rider");
    // The wheel zone starts at x 128..160 (centred on the hull's 144);
    // she waits in the dock just ahead of it, at x 170.
    stand_in_dock(&mut g, 170);
    let events = until(&mut g, "-", 40, |g| g.player().power == Power::Normal);
    assert!(events.contains(&Event::Hurt), "{events:?}");
    let zone = crate::platform::paddle_wheel(&ship(&g), &t).unwrap();
    assert!(zone.overlaps(&g.player().hitbox()) || g.player().invulnerable > 0);
    // Normal: the next touch (after the invulnerable frames) kills.
    let mut g = Game::new(&lake_world(), &t, 1);
    g.step(input("-"));
    let s = ship(&g);
    board(&mut g, s, 4);
    g.step(input("-"));
    stand_in_dock(&mut g, 170);
    let events = until(&mut g, "-", 40, |g| g.mode() != crate::Mode::Playing);
    assert!(events.contains(&Event::Died), "{events:?}");
    assert_eq!(g.death_cause(), Some(crate::DeathCause::Hit));
}

#[test]
fn the_wheel_never_touches_a_rider_and_is_still_while_waiting_or_moored() {
    let t = tuning();
    // A Normal rider stands over the wheel for the whole trip.
    let mut g = Game::new(&lake_world(), &t, 1);
    g.step(input("-"));
    let s = ship(&g);
    board(&mut g, s, 74);
    let events = hold(&mut g, "-", 400);
    assert!(!events.contains(&Event::Hurt) && !events.contains(&Event::Died));
    assert_eq!(ship(&g).state, car::MOORED);
    // In the dock under the waiting ship's wheel: nothing.
    let mut g = Game::new(&lake_world(), &t, 1);
    g.step(input("-"));
    stand_in_dock(&mut g, 138);
    let events = hold(&mut g, "-", 120);
    assert!(!events.contains(&Event::Hurt) && !events.contains(&Event::Died));
    assert_eq!(ship(&g).state, car::WAIT_LOWER);
    // Under the moored ship's wheel (centre 144 + 128): nothing either.
    let s = ship(&g);
    board(&mut g, s, 4);
    hold(&mut g, "-", 170);
    assert_eq!(ship(&g).state, car::MOORED);
    stand_in_dock(&mut g, 266);
    let events = hold(&mut g, "-", 120);
    assert!(!events.contains(&Event::Hurt) && !events.contains(&Event::Died));
}

#[test]
fn boats_never_despawn_and_reset_with_the_level() {
    let t = tuning();
    let mut g = Game::new(&lake_world(), &t, 1);
    g.step(input("-"));
    let s = ship(&g);
    board(&mut g, s, 4);
    hold(&mut g, "-", 200);
    assert_eq!(ship(&g).state, car::MOORED);
    // Walk into the lake (lethal until E27's swim water): the level resets.
    g.player.x = Fx::from_px(27 * 16);
    g.player.y = Fx::from_px(200);
    g.player.on_ground = false;
    until(&mut g, "-", 400, |g| {
        g.mode() == crate::Mode::Playing && g.deaths() == 1
    });
    assert_eq!(
        (ship(&g).x, ship(&g).state),
        (s.x, car::WAIT_LOWER),
        "back at its spawn"
    );
    assert!(g.entities().iter().any(|e| e.kind == EntityKind::Traundl));
}
