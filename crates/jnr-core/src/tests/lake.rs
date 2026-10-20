//! The M5 systems together (M5 task E30): swimming (D-076) against the
//! lake enemies (D-078 … D-080), the boats (D-077) and the auto-scroll
//! camera (D-075), live in swim water. The single systems are tested in
//! `swim.rs`, `lake_enemies.rs`, `boats.rs` and `autoscroll.rs`; these are
//! the cross-task rows of the Stage 6 "New mechanics" table.

use super::*;
use crate::camera::{Autoscroll, ScrollPhase};
use crate::enemies::state;
use crate::platform::car;
use crate::player::Power;
use crate::{EnemyKind, Entity, EntityKind, Fx, Mode};

/// Legend of the lake worlds (`.` air).
const LEGEND: &str = r##"
"#" = "solid"
"-" = "oneway"
"~" = "see_wasser"
"S" = { entity = "player_start", tile = "see_wasser" }
"P" = { entity = "player_start" }
"s" = { entity = "schwan", tile = "see_wasser" }
"g" = { entity = "gelse" }
"b" = { entity = "regattaboot", tile = "see_wasser" }
"D" = { entity = "dampfer", to = [16, 0] }
"T" = { entity = "traundl", to = [-6, 0] }
"C" = { entity = "checkpoint" }
"G" = { entity = "goal" }
"##;

/// The water line (the top of the surface row 12), the deck tops (row 11)
/// and the lake bed (row 16), px.
const WATER_LINE: i32 = 192;
const BED: i32 = 256;

/// A 17-row lake world `width` columns wide (the Stage 6 rows): a solid
/// quay in columns 0–3 (top row 11), swim water in rows 12–15 elsewhere,
/// the lake bed in row 16; `marks` as (row, col, char) in [`LEGEND`]
/// (with a `P` or `S` among them, else the start is on the quay at 2);
/// `zone`: an auto-scroll zone.
fn lake(width: usize, zone: Option<(i32, i32)>, marks: &[(usize, usize, char)]) -> Level {
    let mut rows: Vec<Vec<char>> = (0..17)
        .map(|r| {
            (0..width)
                .map(|c| match r {
                    16 => '#',
                    11..=15 if c < 4 => '#',
                    12..=15 => '~',
                    _ => '.',
                })
                .collect()
        })
        .collect();
    if !marks.iter().any(|m| m.2 == 'P' || m.2 == 'S') {
        rows[10][2] = 'P';
    }
    for &(r, c, ch) in marks {
        rows[r][c] = ch;
    }
    let grid: Vec<String> = rows.into_iter().map(|r| r.into_iter().collect()).collect();
    let camera = zone.map_or(String::new(), |(f, t)| {
        format!("camera = \"autoscroll\"\nautoscroll = [{f}, {t}]\n")
    });
    let text = format!(
        "[meta]\nid = \"lake\"\ntime_limit = 300\n{camera}[legend]\n{LEGEND}\n[map]\ngrid = '''\n{}\n'''\n",
        grid.join("\n")
    );
    Level::parse(&text).unwrap()
}

fn game(level: &Level, power: Power) -> Game {
    let mut g = Game::new(level, &tuning(), 1);
    g.player.set_power(power, &tuning().player);
    g.player.transform = 0;
    g
}

fn find(g: &Game, kind: EntityKind) -> Option<Entity> {
    g.entities().iter().find(|e| e.kind == kind).copied()
}

fn index(g: &Game, kind: EntityKind) -> usize {
    g.entities().iter().position(|e| e.kind == kind).unwrap()
}

const SWAN: EntityKind = EntityKind::Enemy(EnemyKind::Schwan);
const GELSE: EntityKind = EntityKind::Enemy(EnemyKind::Gelse);
const BOAT: EntityKind = EntityKind::Enemy(EnemyKind::Regattaboot);

/// Steps until she floats (vy 0 at the surface) or stands on the bed.
fn settle(g: &mut Game, token: &str) {
    for _ in 0..200 {
        g.step(input(token));
        let p = g.player();
        if p.swimming && p.vy == Fx::ZERO {
            return;
        }
    }
    panic!("never settled");
}

// ------------------------------------------------------------- the swan

#[test]
fn a_charging_swan_hurts_a_surface_swimmer_and_passes_over_a_diver() {
    // Swan #2 of Stage 6: Resi in open water at column 8, the swan at 12.
    let marks = [(12, 8, 'S'), (12, 12, 's')];
    // At the surface (Zuckerschock): it triggers, hisses, charges: Hurt.
    let mut g = game(&lake(40, None, &marks), Power::Big);
    settle(&mut g, "-");
    let events = hold(&mut g, "-", 200);
    assert!(events.contains(&Event::Hurt), "{events:?}");
    assert!(!events.contains(&Event::Stomped));
    // A diver on the bed: the same charge passes over her.
    let mut g = game(&lake(40, None, &marks), Power::Big);
    settle(&mut g, "D");
    assert_eq!(g.player().feet().1, Fx::from_px(BED), "on the bed");
    let mut charged = false;
    for _ in 0..200 {
        let events = g.step(input("-")).to_vec();
        assert!(!events.contains(&Event::Hurt) && !events.contains(&Event::Died));
        charged |= find(&g, SWAN).is_some_and(|s| s.state == state::CHARGE);
    }
    assert!(charged, "it charged");
    assert_eq!(g.player().power, Power::Big);
}

#[test]
fn a_jump_out_beside_a_resting_swan_ends_in_a_stomp() {
    // D-076: a jump-out is airborne from its next frame, so it can stomp
    // (the „Delfinsprung"). The swan is held at rest (it never triggers);
    // Resi floats with her right edge 2 px left of its hitbox.
    let mut g = game(
        &lake(40, None, &[(12, 8, 'S'), (12, 12, 's')]),
        Power::Normal,
    );
    let i = index(&g, SWAN);
    let rest = |g: &mut Game| {
        g.entities[i].state = state::REST;
        g.entities[i].count = 0;
    };
    rest(&mut g);
    settle(&mut g, "-");
    let swan = find(&g, SWAN).unwrap();
    g.player.x = swan.x - Fx::from_px(2) - g.player.w;
    rest(&mut g);
    g.step(input("-"));
    assert!(g.player().swimming);
    rest(&mut g);
    let events = g.step(input("J")).to_vec();
    assert!(events.contains(&Event::Jumped));
    let mut events = Vec::new();
    for _ in 0..120 {
        if find(&g, SWAN).is_none() {
            break;
        }
        rest(&mut g);
        let token = if g.player().vy < Fx::ZERO { "J" } else { "R" };
        events.extend_from_slice(g.step(input(token)));
    }
    assert!(events.contains(&Event::Stomped), "{events:?}");
    assert!(!events.contains(&Event::Hurt) && !events.contains(&Event::Died));
    assert_eq!(g.score(), 200, "chain start 200");
}

// ------------------------------------------------------------- the Gelse

#[test]
fn a_gelse_waits_at_the_water_line_over_a_diver_and_stings_a_swimmer() {
    let marks = [(12, 10, 'S'), (6, 10, 'g')];
    // A diver on the bed: it hunts down to the water line and waits there,
    // never in the water, until its 480 hunting frames are over.
    let mut g = game(&lake(40, None, &marks), Power::Normal);
    settle(&mut g, "D");
    let mut lowest = Fx::ZERO;
    for _ in 0..(60 + 480 + 10) {
        let events = g.step(input("-")).to_vec();
        assert!(!events.contains(&Event::Died));
        if let Some(e) = find(&g, GELSE) {
            assert!(!g.map().touches(&e.hitbox(), crate::TileKind::Swim));
            lowest = lowest.max(e.y + e.h);
        }
    }
    assert_eq!(lowest, Fx::from_px(WATER_LINE), "down to the water line");
    assert!(
        find(&g, GELSE).is_none_or(|e| e.state == state::LEAVE),
        "it leaves"
    );
    // A swimmer at the surface: her head is above the water line, so it
    // stings her (Zuckerschock → Normal).
    let mut g = game(&lake(40, None, &marks), Power::Big);
    settle(&mut g, "-");
    let events = hold(&mut g, "-", 300);
    assert!(events.contains(&Event::Hurt), "{events:?}");
}

// ------------------------------------------------------- the regatta boat

#[test]
fn a_regatta_boat_hurts_a_surface_swimmer_and_passes_over_a_diver() {
    let marks = [(12, 10, 'S'), (12, 16, 'b')];
    let mut g = game(&lake(40, None, &marks), Power::Big);
    settle(&mut g, "-");
    let events = hold(&mut g, "-", 200);
    assert!(events.contains(&Event::Hurt) && !events.contains(&Event::Stomped));
    let mut g = game(&lake(40, None, &marks), Power::Big);
    settle(&mut g, "D");
    let mut passed = false;
    for _ in 0..300 {
        let events = g.step(input("-")).to_vec();
        assert!(!events.contains(&Event::Hurt) && !events.contains(&Event::Died));
        passed |= find(&g, BOAT).is_none_or(|b| b.right() < g.player().x);
    }
    assert!(passed, "it raced over her");
}

// ---------------------------------------------------- the paddle wheel

#[test]
fn the_paddle_wheel_hurts_a_swimmer_but_never_its_rider() {
    // The Dampfer in cell (4, 11) going 16 tiles right over the water,
    // flush with the quay. Resi walks on (that calls it) and on off its bow
    // into the water: the hull passes over her and the wheel hurts her.
    let level = lake(60, None, &[(11, 4, 'D')]);
    let ship = |g: &Game| find(g, EntityKind::Dampfer).unwrap();
    let mut g = game(&level, Power::Big);
    until(&mut g, "R", 200, |g| ship(g).state == car::UP);
    let mut events = until(&mut g, "R", 300, |g| g.player().swimming);
    events.extend(hold(&mut g, "-", 300));
    assert!(events.contains(&Event::Hurt), "{events:?}");
    assert_eq!(g.player().power, Power::Normal);
    // A rider standing over the wheel the whole way: never hurt.
    let mut g = game(&level, Power::Big);
    let mut events = until(&mut g, "R", 200, |g| ship(g).state == car::UP);
    let wheel_x = ship(&g).x + ship(&g).w.div_int(2);
    events.extend(until(&mut g, "R", 200, |g| {
        g.player().x + g.player().w.div_int(2) >= wheel_x
    }));
    events.extend(hold(&mut g, "-", 300));
    assert!(!events.contains(&Event::Hurt) && !g.player().swimming);
    assert_eq!(ship(&g).state, car::MOORED);
}

// ------------------------------------------------------- the scroll edge

/// Puts the camera at `x` inside the zone, scrolling without a hold.
fn scrolling_at(g: &mut Game, x: i32) {
    g.camera.x = x;
    g.scroll = Autoscroll {
        phase: ScrollPhase::Scroll,
        pos: Fx::from_px(x),
        hold: 0,
    };
}

#[test]
fn the_scroll_edge_pushes_a_swimmer_along_the_water() {
    let mut g = game(&lake(80, Some((2, 40)), &[(12, 10, 'S')]), Power::Normal);
    settle(&mut g, "-");
    let x = g.player().x.to_px();
    scrolling_at(&mut g, x - 4);
    for _ in 0..200 {
        let events = g.step(input("-")).to_vec();
        assert!(!events.contains(&Event::Died));
        assert!(
            g.player().x >= Fx::from_px(g.camera().x),
            "never left of the edge"
        );
    }
    assert!(g.player().swimming, "still swimming");
    assert!(g.player().x > Fx::from_px(x + 100), "pushed along");
}

#[test]
fn the_scroll_edge_pushes_a_traundl_rider_across_the_deck_into_the_water() {
    // D-051 is off in the scroll (D-075): a Traundl going left carries its
    // rider into the edge, which pushes her across the deck until she drops
    // off its right end into the water, where she swims on.
    let mut g = game(
        &lake(80, Some((2, 40)), &[(11, 20, 'T'), (10, 21, 'P')]),
        Power::Normal,
    );
    hold(&mut g, "-", 2);
    let boat = find(&g, EntityKind::Traundl).unwrap();
    scrolling_at(&mut g, boat.x.to_px() - 40);
    let mut swam = false;
    for _ in 0..300 {
        let events = g.step(input("-")).to_vec();
        assert!(!events.contains(&Event::Died));
        swam |= g.player().swimming;
    }
    assert!(swam, "dropped into the water");
    assert!(g.player().x >= Fx::from_px(g.camera().x));
}

// ------------------------------------------- a respawn inside the zone

#[test]
fn a_respawn_on_a_raft_in_the_zone_holds_the_scroll_then_the_edge_pushes_her_into_the_water() {
    // A raft (one-way, row 11) at columns 30–33 over the water with a
    // checkpoint at 31, inside the zone [2, 60].
    let mut marks: Vec<_> = (30..34).map(|c| (11, c, '-')).collect();
    marks.push((10, 31, 'C'));
    let level = lake(100, Some((2, 60)), &marks);
    let mut g = game(&level, Power::Normal);
    g.player.x = Fx::from_px(31 * 16 + 2);
    g.player.y = Fx::from_px(176) - g.player.h;
    g.player.on_ground = true;
    let events = g.step(input("-")).to_vec();
    assert!(events.contains(&Event::CheckpointReached));
    // A death: out of the level's bottom.
    g.player.y = Fx::from_px(17 * 16);
    until(&mut g, "-", 200, |g| g.events().contains(&Event::Respawned));
    assert_eq!(g.mode(), Mode::Playing);
    assert_eq!(g.player().feet().1, Fx::from_px(176), "on the raft");
    assert!(!g.player().swimming);
    let cam = g.camera().x;
    let hold_frames = tuning().camera.autoscroll_hold;
    hold(&mut g, "-", u32::from(hold_frames) - 1);
    assert_eq!(g.camera().x, cam, "held");
    // Then it scrolls; standing still, she ends up pushed off the raft
    // into the water and swims (no death: the lake has no lethal pit).
    let events = until(&mut g, "-", 2000, |g| g.player().swimming);
    assert!(!events.contains(&Event::Died));
    assert!(g.camera().x > cam);
    assert!(g.player().x >= Fx::from_px(g.camera().x));
}
