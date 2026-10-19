//! The Schafbergbahn (D-067, game-design.md → "Stage 5: St. Wolfgang",
//! "New mechanics"): the Festungsbahn's waiting state machine with a 64×32
//! body, 24 frames per tile and a 90-frame top wait; roofs flush with the
//! station platforms, so Resi walks on and off.

use super::*;
use crate::platform::{CarTiming, car, car_timing, schafbergbahn_travel};
use crate::{Entity, EntityKind, Fx};

const TRAIN: EntityKind = EntityKind::Schafbergbahn;

/// The lower platform (columns 0–5, top row 9 = y 144), the train in cell
/// (6, 10) (body x 96..160, roof y 144: flush) going `to` = [16, −4] (a
/// 1:4 track: upper end x 352, roof y 80), a bottomless gorge under the
/// track (columns 6–25) and the upper platform from column 26 (top row 5
/// = y 80: flush with the roof at the upper end). 14 rows, 48 columns.
fn station_world() -> Level {
    let mut rows: Vec<String> = Vec::new();
    for r in 0..14 {
        let mut row: Vec<char> = vec!['.'; 48];
        for (c, cell) in row.iter_mut().enumerate() {
            if (c < 6 && r >= 9) || (c >= 26 && r >= 5) {
                *cell = '#';
            }
        }
        if r == 8 {
            row[2] = 'P';
        }
        if r == 10 {
            row[6] = 'Z';
        }
        if r == 4 {
            row[44] = 'G';
        }
        rows.push(row.into_iter().collect());
    }
    let text = format!(
        "[meta]\nid = \"z\"\ntime_limit = 300\n[legend]\n\"#\" = \"solid\"\n\"P\" = {{ entity = \"player_start\" }}\n\"G\" = {{ entity = \"goal\" }}\n\"Z\" = {{ entity = \"schafbergbahn\", to = [16, -4] }}\n[map]\ngrid = '''\n{}\n'''\n",
        rows.join("\n")
    );
    Level::parse(&text).unwrap()
}

fn train(g: &Game) -> Entity {
    *g.entities()
        .iter()
        .find(|e| e.kind == TRAIN)
        .expect("a Schafbergbahn")
}

fn timing(g: &Game) -> CarTiming {
    let t = train(g);
    let spawn = &g.level().entities[usize::from(t.spawn.unwrap())];
    car_timing(TRAIN, Some(spawn), g.tuning())
}

/// Puts the player standing on the roof, `dx` px right of its left edge.
fn board(g: &mut Game, dx: i32) {
    let c = train(g);
    g.player.x = c.x + Fx::from_px(dx);
    g.player.y = c.y - g.player.h;
    g.player.vx = Fx::ZERO;
    g.player.vy = Fx::ZERO;
    g.player.on_ground = true;
}

/// The train's hitbox top-left when `k` of `travel` frames up the track:
/// x on the straight line, the roof 1 px higher for every whole 4 px of
/// it (on the drawn rail, never between two rail rows).
fn expected(k: i32, travel: i32) -> (Fx, Fx) {
    let dx = Fx::from_px(256).mul_int(k).div_int(travel);
    (
        Fx::from_px(96) + dx,
        Fx::from_px(144 - dx.to_px().div_euclid(4)),
    )
}

#[test]
fn travel_is_24_frames_per_tile_along_the_major_axis() {
    let t = tuning();
    let level = station_world();
    let spawn = level
        .entities
        .iter()
        .find(|s| s.kind == crate::SpawnKind::Schafbergbahn)
        .unwrap();
    assert_eq!(schafbergbahn_travel(spawn, &t), 16 * 24);
    assert_eq!(
        car_timing(TRAIN, Some(spawn), &t),
        CarTiming {
            travel: 384,
            wait: 90
        }
    );
    // Stage 5's track (D-067): 24 tiles = 576 frames (9.6 s).
    let mut stage = spawn.clone();
    stage.to = Some([24, -6]);
    assert_eq!(schafbergbahn_travel(&stage, &t), 576);
    stage.to = Some([-4, -8]);
    assert_eq!(schafbergbahn_travel(&stage, &t), 8 * 24, "major axis");
    let g = Game::new(&level, &t, 1);
    let c = train(&g);
    assert_eq!((c.w, c.h), (Fx::from_px(64), Fx::from_px(32)));
    assert_eq!((c.x, c.y), (Fx::from_px(96), Fx::from_px(144)));
    assert_eq!(c.state, car::WAIT_LOWER);
}

#[test]
fn the_train_waits_for_its_rider_rides_up_waits_90_and_comes_back_without_drift() {
    let mut g = Game::new(&station_world(), &tuning(), 1);
    let start = train(&g);
    // Nobody on it: it waits at its lower end, however long.
    for _ in 0..600 {
        g.step(input("-"));
        assert_eq!((train(&g).x, train(&g).y), (start.x, start.y));
    }
    assert_eq!(timing(&g).travel, 384);
    board(&mut g, 26);
    // A whole cycle with the rider on board, plus a second departure.
    let mut path = Vec::new();
    for _ in 0..(384 + 90 + 384 + 3) {
        g.step(input("-"));
        let c = train(&g);
        path.push((c.x, c.y, c.state));
    }
    // path[i] is after update i + 1; every frame exactly on the line.
    for (i, &(x, y, _)) in path[..384].iter().enumerate() {
        assert_eq!((x, y), expected(i as i32 + 1, 384), "up, frame {i}");
    }
    let top = expected(384, 384);
    assert_eq!(top, (Fx::from_px(352), Fx::from_px(80)));
    assert_eq!((path[383].0, path[383].1), top, "arrives after 384 frames");
    assert_eq!(path[384].2, car::WAIT_UPPER);
    for (i, &(x, y, _)) in path[383..384 + 90].iter().enumerate() {
        assert_eq!((x, y), top, "waits 90 frames at the top ({i})");
    }
    assert_eq!(path[384 + 90].2, car::DOWN, "then comes back");
    for (i, &(x, y, _)) in path[384 + 90..384 + 90 + 384].iter().enumerate() {
        assert_eq!((x, y), expected(383 - i as i32, 384), "down, frame {i}");
    }
    let n = 384 + 90 + 383;
    assert_eq!((path[n].0, path[n].1), (start.x, start.y), "back, no drift");
    assert_eq!(path[n].2, car::DOWN, "arrives");
    assert_eq!(
        (path[n + 1].0, path[n + 1].1, path[n + 1].2),
        (start.x, start.y, car::WAIT_LOWER)
    );
    assert_eq!(path[n + 2].2, car::UP, "the rider is still on: up again");
    // Both roofs on the tile grid (whole-tile body and `to`).
    for (x, y) in [(start.x, start.y), top] {
        assert_eq!(x.to_px() % 16, 0);
        assert_eq!(y.to_px() % 16, 0);
        assert_eq!(Fx::from_px(x.to_px()), x, "whole px");
        assert_eq!(Fx::from_px(y.to_px()), y, "whole px");
    }
}

#[test]
fn the_rider_is_carried_exactly_over_the_whole_ride() {
    let mut g = Game::new(&station_world(), &tuning(), 1);
    g.step(input("-"));
    board(&mut g, 20);
    for f in 0..(384 + 90 + 384) {
        let events = g.step(input("-")).to_vec();
        let (c, p) = (train(&g), *g.player());
        assert_eq!(p.y + p.h, c.y, "feet exactly on the roof, frame {f}");
        assert_eq!(p.x - c.x, Fx::from_px(20), "no sliding, frame {f}");
        assert!(p.on_ground, "frame {f}");
        assert!(!events.contains(&Event::Landed), "frame {f}");
        assert!(!events.contains(&Event::Died), "frame {f}");
    }
}

#[test]
fn resi_walks_on_at_the_lower_station_and_off_at_the_upper_one() {
    let mut g = Game::new(&station_world(), &tuning(), 1);
    // Settle on the lower platform (feet y 144, the roof's height).
    hold(&mut g, "-", 30);
    assert!(g.player().on_ground);
    assert_eq!(g.player().y + g.player().h, Fx::from_px(144));
    // Walk right onto the roof: she never leaves the ground, and the
    // train leaves with her.
    for f in 0..200 {
        let events = g.step(input("R")).to_vec();
        assert!(g.player().on_ground, "walking on, frame {f}");
        assert!(!events.contains(&Event::Landed), "no fall, frame {f}");
        if train(&g).state == car::UP {
            break;
        }
    }
    assert_eq!(train(&g).state, car::UP, "called by walking on");
    // Stand still for the ride.
    until(&mut g, "-", 400, |g| train(g).state == car::WAIT_UPPER);
    assert_eq!(g.player().y + g.player().h, Fx::from_px(80));
    // Walk off onto the upper platform during the wait, on the ground all
    // the way.
    for f in 0..80 {
        let events = g.step(input("R")).to_vec();
        assert!(g.player().on_ground, "walking off, frame {f}");
        assert!(!events.contains(&Event::Landed), "no fall, frame {f}");
    }
    assert!(g.player().x > Fx::from_px(416), "on the upper platform");
    assert_eq!(g.player().y + g.player().h, Fx::from_px(80));
    // The train goes back down without her and waits there.
    hold(&mut g, "-", 90 + 384 + 60);
    assert_eq!(train(&g).state, car::WAIT_LOWER);
    assert_eq!((train(&g).x, train(&g).y), expected(0, 384));
}

/// D-067 risk 2: jumps don't inherit the platform's speed, so a hop on the
/// moving roof drifts against it (2/3 px/frame across on a 1:4 track at 24
/// frames per tile). Measured: riding up, a tap hop (24 frames in the air)
/// lands 15.3 px further back on the roof, a fully held standing jump (51
/// frames) 33.3 px (D-067 estimated ~20 px). From the roof's middle (26 px
/// in) the full hop still lands on the 64 px roof with 4 px of her 12 px
/// hitbox; from the uphill half (32 px in or more) with 10 px or more.
/// Riding down (the roof sinks away, so she is longer in the air) a tap
/// drifts 16.7 px forward (26 frames), a full hop 36 px (55 frames): from
/// the middle it lands with 2 px on the downhill edge.
#[test]
fn a_hop_on_the_moving_roof_drifts_back_and_lands_on_it() {
    let measure = |hold_frames: u32, dx: i32, ride_frames: u32| {
        let mut g = Game::new(&station_world(), &tuning(), 1);
        g.step(input("-"));
        board(&mut g, dx);
        hold(&mut g, "-", ride_frames);
        assert!(train(&g).vx != Fx::ZERO, "moving");
        let before = g.player().x - train(&g).x;
        let events = hold(&mut g, "J", hold_frames);
        assert!(events.contains(&Event::Jumped), "{events:?}");
        let mut air = hold_frames;
        while !g.player().on_ground {
            g.step(input("-"));
            air += 1;
            assert!(air < 200, "never landed");
        }
        let (c, p) = (train(&g), *g.player());
        assert_eq!(p.y + p.h, c.y, "landed on the roof ({hold_frames}, {dx})");
        assert!(p.x + p.w > c.x && p.x < c.x + c.w);
        ((p.x - c.x - before).raw(), air, (p.x - c.x).to_px())
    };
    // Riding up (100 frames after boarding).
    assert_eq!(measure(1, 26, 100), (-3925, 24, 10), "tap");
    assert_eq!(measure(30, 26, 100), (-8533, 51, -8), "full hop");
    assert_eq!(measure(30, 32, 100).2, -2, "full hop from the front half");
    // Riding down (the rider stayed on through the top wait).
    let down = 384 + 90 + 100;
    assert_eq!(measure(1, 26, down), (4266, 26, 42), "tap, forward");
    assert_eq!(measure(30, 26, down), (9216, 55, 62), "full hop, forward");
}
