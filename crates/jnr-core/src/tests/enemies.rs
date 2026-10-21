//! Enemy behaviours, thrown Mozartkugeln, knocking enemies off bumped
//! blocks, and the Festungsbahn car (M1 task E3).

use super::*;
use crate::enemies::{Pose, sine_offset, state};
use crate::{DeathCause, EnemyKind, Entity, EntityKind, Fx, Mode, Power};

/// Legend of the test worlds (`.` air, `P` start, `G` goal).
const LEGEND: &str = r##"
"#" = "solid"
"-" = "oneway"
"^" = "hazard"
"B" = "brick"
"M" = { tile = "oblaten_box", contains = "mozartkugel" }
"t" = { entity = "selfie_tourist" }
"u" = { entity = "taube" }
"s" = { entity = "taube_sitting" }
"S" = { entity = "taube_sitting", dy = -8 }
"v" = { entity = "mozartkugel_verkaeufer" }
"f" = { entity = "fiaker" }
"F" = { entity = "festungsbahn", to = [8, -5] }
"D" = { entity = "festungsbahn", to = [8, 5] }
"T" = { entity = "selfie_tourist", still = true }
"P" = { entity = "player_start" }
"G" = { entity = "goal" }
"##;

/// A 14-row world `width` columns wide: ground in rows 12–13, the player
/// start at column 2 (row 11) unless `marks` places a `P`, plus `marks`
/// as (row, col, char).
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
        "[meta]\nid = \"enemies\"\ntime_limit = 300\n[legend]\n{LEGEND}\n[map]\ngrid = '''\n{}\n'''\n",
        grid.join("\n")
    );
    Level::parse(&text).unwrap()
}

fn game(level: &Level) -> Game {
    Game::new(level, &tuning(), 1)
}

/// The first entity of `kind` (enemies keep their spawn order).
fn first(g: &Game, kind: EntityKind) -> Entity {
    *g.entities()
        .iter()
        .find(|e| e.kind == kind)
        .unwrap_or_else(|| panic!("no {kind:?}"))
}

fn tourist(g: &Game) -> Entity {
    first(g, EntityKind::Enemy(EnemyKind::SelfieTourist))
}

/// Keeps the player out of the way: invulnerable and parked at the left.
fn park_player(g: &mut Game) {
    g.player.invulnerable = u8::MAX;
}

// ------------------------------------------------------- Selfie-Tourist

#[test]
fn tourist_walks_left_at_half_a_pixel_and_turns_at_walls() {
    // Wall at column 10; the tourist starts at column 14 (x 466) and a
    // wall at column 18 stops it on the way back.
    let mut g = game(&world(30, &[(11, 10, '#'), (11, 14, 't'), (11, 18, '#')]));
    park_player(&mut g);
    let x0 = tourist(&g).x;
    hold(&mut g, "-", 1);
    assert_eq!(tourist(&g).x, x0 - Fx::from_raw(128), "0.5 px/frame left");
    assert_eq!(tourist(&g).dir, -1);
    // 466 - 176 = 290 px to the wall: hits it and turns right.
    hold(&mut g, "-", 120);
    let t = tourist(&g);
    assert_eq!(t.dir, 1, "turned at the wall");
    assert!(t.x >= Fx::from_px(176), "never inside the wall");
    // Walks right until the wall at column 18, turns again.
    hold(&mut g, "-", 200);
    let t = tourist(&g);
    assert!(t.right() <= Fx::from_px(18 * 16));
    assert_eq!(t.feet().1, Fx::from_px(192), "stays on the ground");
}

#[test]
fn tourist_turns_at_other_enemies() {
    // Two tourists walking towards each other (the right one turned by a
    // wall first is not needed: the left one walks into a standing
    // Verkäufer and turns, then the other tourist walks into it).
    let mut g = game(&world(40, &[(11, 8, 'v'), (11, 12, 't'), (11, 30, 't')]));
    park_player(&mut g);
    let v = first(&g, EntityKind::Enemy(EnemyKind::MozartkugelVerkaeufer));
    let mut turned = false;
    for _ in 0..200 {
        g.step(input("-"));
        let t = tourist(&g);
        assert!(t.x + Fx::from_px(1) >= v.right(), "never walks through");
        if t.dir == 1 {
            turned = true;
            break;
        }
    }
    assert!(turned, "turned at the Verkäufer");
    // The two tourists meet and both turn away from each other.
    let mut met = false;
    for _ in 0..800 {
        g.step(input("-"));
        let ts: Vec<Entity> = g
            .entities()
            .iter()
            .filter(|e| e.kind == EntityKind::Enemy(EnemyKind::SelfieTourist))
            .copied()
            .collect();
        if ts[0].dir == -1 && ts[1].dir == 1 {
            met = true;
            assert!(ts[1].x - ts[0].right() >= Fx::from_px(-1), "barely touched");
            break;
        }
    }
    assert!(met, "the tourists turned away from each other");
}

#[test]
fn tourist_walks_off_ledges_and_falls_with_gravity() {
    // On a 1-tile-high ledge (columns 0-11 at row 11) the tourist at
    // column 11 walks left... so put the ledge to its left edge: a block
    // row at row 8 from column 8 to 12, tourist on it at column 9.
    let mut marks: Vec<(usize, usize, char)> = (8..=12).map(|c| (8, c, '#')).collect();
    marks.push((7, 9, 't'));
    let mut g = game(&world(30, &marks));
    park_player(&mut g);
    let t = tuning();
    let y0 = tourist(&g).y;
    let mut fell = false;
    let mut max_vy = Fx::ZERO;
    for _ in 0..200 {
        g.step(input("-"));
        let e = tourist(&g);
        max_vy = max_vy.max(e.vy);
        if e.y > y0 {
            fell = true;
        }
        if fell && e.feet().1 == Fx::from_px(192) {
            break;
        }
    }
    assert!(fell, "walked off the ledge");
    assert_eq!(
        tourist(&g).feet().1,
        Fx::from_px(192),
        "landed on the ground"
    );
    assert!(max_vy <= t.player.max_fall_speed);
    assert!(max_vy > Fx::ZERO);
}

#[test]
fn tourist_takes_a_selfie_every_240_frames_for_48_with_a_2_frame_flash() {
    let t = tuning();
    let mut g = game(&world(60, &[(11, 30, 't')]));
    park_player(&mut g);
    let mut poses = Vec::new();
    let mut xs = Vec::new();
    for _ in 0..600 {
        g.step(input("-"));
        let e = tourist(&g);
        poses.push(e.pose(&t).unwrap());
        xs.push(e.x);
    }
    // poses[i] is after update i + 1 (timer = i + 1).
    let selfie: Vec<usize> = (0..600)
        .filter(|&i| matches!(poses[i], Pose::Selfie { .. }))
        .map(|i| i + 1)
        .collect();
    let expected: Vec<usize> = (240..288).chain(480..528).collect();
    assert_eq!(selfie, expected, "selfie frames (timer)");
    let flash: Vec<usize> = (0..600)
        .filter(|&i| poses[i] == Pose::Selfie { flash: true })
        .map(|i| i + 1)
        .collect();
    assert_eq!(
        flash,
        vec![264, 265, 504, 505],
        "flash at frame 24 of the stop, 2 frames"
    );
    // Standing still during the selfie, walking otherwise.
    assert_eq!(xs[239], xs[287 - 1], "no movement during the selfie");
    assert_ne!(xs[288], xs[287], "walks again");
    assert_eq!(poses[0], Pose::Walk);
}

/// `still = true` (grey-box test levels): the tourist never walks or
/// turns, even when another tourist walks into it, but still hurts.
#[test]
fn a_still_tourist_never_moves() {
    let mut g = game(&world(30, &[(11, 12, 'T'), (11, 20, 't')]));
    park_player(&mut g);
    let still = first(&g, EntityKind::Enemy(EnemyKind::SelfieTourist));
    assert_eq!(still.dir, 0);
    for _ in 0..400 {
        g.step(input("-"));
        let e = first(&g, EntityKind::Enemy(EnemyKind::SelfieTourist));
        assert_eq!((e.x, e.y, e.dir), (still.x, still.y, 0));
    }
    let walker = g.entities()[1];
    assert_eq!(walker.dir, 1, "the walking one turned at the still one");
    g.player.invulnerable = 0;
    until(&mut g, "R", 200, |g| g.mode() != Mode::Playing);
    assert_eq!(g.death_cause(), Some(DeathCause::Hit));
}

#[test]
fn a_stomped_tourist_stays_squished_for_30_frames_without_collision() {
    let t = tuning();
    let mut g = game(&world(20, &[(9, 2, 'P'), (11, 2, 't')]));
    until(&mut g, "-", 60, |g| g.events().contains(&Event::Stomped));
    let squished = first(&g, EntityKind::Defeated(EnemyKind::SelfieTourist));
    assert_eq!(squished.pose(&t), Some(Pose::Squished));
    let mut frames = 0;
    while g
        .entities()
        .iter()
        .any(|e| matches!(e.kind, EntityKind::Defeated(_)))
    {
        let e = g.step(input("-")).to_vec();
        assert!(
            !e.contains(&Event::Died) && !e.contains(&Event::Stomped),
            "{e:?}"
        );
        let d = g
            .entities()
            .iter()
            .find(|e| matches!(e.kind, EntityKind::Defeated(_)));
        if let Some(d) = d {
            assert_eq!((d.x, d.y), (squished.x, squished.y), "stays in place");
        }
        frames += 1;
        assert!(frames <= 30);
    }
    assert_eq!(frames, u32::from(t.enemies.squish_frames));
}

// ----------------------------------------------------------------- Taube

#[test]
fn taube_flies_left_on_a_sine_wave_through_walls() {
    let t = tuning();
    // Taube at row 6, a wall column right in its way.
    let mut marks: Vec<(usize, usize, char)> = (2..=11).map(|r| (r, 12, '#')).collect();
    marks.push((6, 16, 'u'));
    let mut g = game(&world(40, &marks));
    park_player(&mut g);
    let start = first(&g, EntityKind::Enemy(EnemyKind::Taube));
    let base = start.y;
    for f in 1..=240u32 {
        g.step(input("-"));
        let e = first(&g, EntityKind::Enemy(EnemyKind::Taube));
        assert_eq!(
            e.x,
            start.x - Fx::from_raw(192).mul_int(f as i32),
            "0.75 px/frame"
        );
        let off = sine_offset(f, t.enemies.taube_period, Fx::from_px(16));
        assert_eq!(e.y, base - off, "frame {f}");
        assert_eq!(e.pose(&t), Some(Pose::Fly));
    }
    // Peaks 16 px above and below its start height, one wave per 120 frames.
    assert_eq!(sine_offset(30, 120, Fx::from_px(16)), Fx::from_px(16));
    assert_eq!(sine_offset(90, 120, Fx::from_px(16)), Fx::from_px(-16));
    // It flew through the wall at column 12 (x 192..208): now left of it.
    assert!(first(&g, EntityKind::Enemy(EnemyKind::Taube)).right() < Fx::from_px(192));
}

#[test]
fn taube_despawns_off_the_left() {
    let mut g = game(&world(40, &[(6, 10, 'u')]));
    park_player(&mut g);
    hold(&mut g, "-", 400);
    assert!(
        !g.entities()
            .iter()
            .any(|e| matches!(e.kind, EntityKind::Enemy(_))),
        "flew off the left and was removed"
    );
}

#[test]
fn sitting_taube_waits_rises_16_px_over_16_frames_then_flies() {
    let t = tuning();
    // Sitting Taube on the ground at column 14 (centre x 232); the player
    // (centre x 40) walks right until within 96 px.
    let mut g = game(&world(40, &[(11, 14, 's')]));
    let sit = first(&g, EntityKind::Enemy(EnemyKind::TaubeSitting));
    assert_eq!(sit.state, state::SIT);
    hold(&mut g, "-", 60);
    let e = first(&g, EntityKind::Enemy(EnemyKind::TaubeSitting));
    assert_eq!(
        (e.x, e.y, e.pose(&t)),
        (sit.x, sit.y, Some(Pose::Sit)),
        "sits"
    );
    // Walk until it notices the player.
    let mut frames = 0;
    while first(&g, EntityKind::Enemy(EnemyKind::TaubeSitting)).state == state::SIT {
        g.step(input("R"));
        frames += 1;
        assert!(frames < 300);
    }
    let cx = g.player().x + g.player().w.div_int(2);
    let e = first(&g, EntityKind::Enemy(EnemyKind::TaubeSitting));
    let dist = (e.x + e.w.div_int(2) - cx).to_px();
    assert!(dist <= 96 && dist > 96 - 3, "takes off at 6 tiles: {dist}");
    // Rises 1 px per frame for 16 frames (the first one in the take-off frame).
    let mut ys = vec![e.y];
    for _ in 0..15 {
        g.step(input("-"));
        let e = first(&g, EntityKind::Enemy(EnemyKind::TaubeSitting));
        assert_eq!(e.x, sit.x, "no horizontal movement while rising");
        ys.push(e.y);
    }
    let rise: Vec<i32> = ys.iter().map(|y| (sit.y - *y).to_px()).collect();
    assert_eq!(rise, (1..=16).collect::<Vec<i32>>());
    let e = first(&g, EntityKind::Enemy(EnemyKind::TaubeSitting));
    assert_eq!(e.state, state::FLY);
    assert_eq!(e.base_y, sit.y - Fx::from_px(16));
    // Then flies like a Taube from there.
    g.step(input("-"));
    let e = first(&g, EntityKind::Enemy(EnemyKind::TaubeSitting));
    assert_eq!(e.x, sit.x - Fx::from_raw(192));
    assert_eq!(e.pose(&t), Some(Pose::Fly));
}

#[test]
fn sitting_taube_dy_moves_its_perch_and_it_rises_from_there() {
    // `dy = -8`: feet 8 px above its cell bottom (a perch between two cell
    // bottoms, like St. Wolfgang's mitre); x and behaviour are unchanged.
    let at = |c| {
        let g = game(&world(40, &[(11, 14, c)]));
        first(&g, EntityKind::Enemy(EnemyKind::TaubeSitting))
    };
    let (ground, perch) = (at('s'), at('S'));
    assert_eq!(
        ground.y + ground.h,
        Fx::from_px(12 * 16),
        "feet on the cell bottom"
    );
    assert_eq!(perch.x, ground.x);
    assert_eq!(perch.y, ground.y - Fx::from_px(8));
    assert_eq!(perch.base_y, perch.y);
    // Sits there until the player comes close, then rises 16 px from the perch.
    let mut g = game(&world(40, &[(11, 14, 'S')]));
    hold(&mut g, "-", 30);
    let e = first(&g, EntityKind::Enemy(EnemyKind::TaubeSitting));
    assert_eq!((e.x, e.y, e.state), (perch.x, perch.y, state::SIT));
    until(&mut g, "R", 300, |g| {
        first(g, EntityKind::Enemy(EnemyKind::TaubeSitting)).state == state::FLY
    });
    let e = first(&g, EntityKind::Enemy(EnemyKind::TaubeSitting));
    assert_eq!(e.base_y, perch.y - Fx::from_px(16));
}

#[test]
fn stomped_taube_tumbles_off_screen_without_collision() {
    let t = tuning();
    // A sitting Taube on a 1-tile block at column 2, row 10; the player
    // falls onto it from row 6 (it takes off at once: the player is close,
    // but the stomp comes first or right after).
    let mut g = game(&world(20, &[(6, 2, 'P'), (10, 2, '#'), (9, 2, 's')]));
    until(&mut g, "-", 60, |g| g.events().contains(&Event::Stomped));
    let e = first(&g, EntityKind::Defeated(EnemyKind::TaubeSitting));
    assert_eq!(e.pose(&t), Some(Pose::Tumble));
    assert_eq!(g.score(), 100);
    // Falls through the block and the ground, then is removed below the level.
    let mut prev = e.y;
    let mut gone = false;
    for _ in 0..120 {
        g.step(input("-"));
        match g
            .entities()
            .iter()
            .find(|e| matches!(e.kind, EntityKind::Defeated(_)))
        {
            Some(e) => {
                assert!(e.y >= prev, "only falls");
                prev = e.y;
            }
            None => {
                gone = true;
                break;
            }
        }
    }
    assert!(gone, "removed below the level");
    assert!(prev > Fx::from_px(12 * 16), "fell through the ground");
    assert_eq!(g.mode(), Mode::Playing);
}

/// The stomp rule uses the enemy's position before this frame's movement
/// (`prev_y`): a Taube rising this frame is still stomped when the
/// player's previous feet were exactly at its previous middle, and hurts
/// one subpixel lower.
#[test]
fn stomping_a_moving_taube_uses_its_previous_position() {
    let run = |below: i32| {
        let mut g = game(&world(20, &[(6, 6, 'u')]));
        g.step(input("-"));
        let e = first(&g, EntityKind::Enemy(EnemyKind::Taube));
        // Next frame it rises (sine, first quarter wave).
        let middle = e.y + e.h.div_int(2);
        g.player.x = e.x + Fx::from_px(2) - Fx::from_raw(192);
        g.player.y = middle + Fx::from_raw(below) - g.player.h;
        g.player.vy = Fx::from_px(2);
        g.player.on_ground = false;
        let events = g.step(input("-")).to_vec();
        let now = first(&g, EntityKind::Defeated(EnemyKind::Taube));
        (events, now)
    };
    let (events, e) = run(0);
    assert!(events.contains(&Event::Stomped), "{events:?}");
    assert!(
        !events.contains(&Event::Bounced),
        "a real stomp: {events:?}"
    );
    assert!(e.y < e.prev_y, "it moved up in the stomp frame");
    let mut g = game(&world(20, &[(6, 6, 'u')]));
    g.step(input("-"));
    let e = first(&g, EntityKind::Enemy(EnemyKind::Taube));
    let middle = e.y + e.h.div_int(2);
    g.player.x = e.x + Fx::from_px(2) - Fx::from_raw(192);
    g.player.y = middle + Fx::from_raw(1) - g.player.h;
    g.player.vy = Fx::from_px(2);
    g.player.on_ground = false;
    let events = g.step(input("-")).to_vec();
    assert!(!events.contains(&Event::Stomped), "{events:?}");
    assert!(events.contains(&Event::Died), "{events:?}");
    // The Taube really moved up in that frame.
    let e2 = first(&g, EntityKind::Enemy(EnemyKind::Taube));
    assert!(e2.y < e.y && e2.prev_y == e.y);
}

// ------------------------------------------------- Mozartkugel-Verkäufer

#[test]
fn verkaeufer_faces_the_player_and_throws_every_120_frames_in_range() {
    let t = tuning();
    // Verkäufer at column 13 (centre 216); the player at column 2 (centre
    // 40) is 176 px away: out of range (160).
    let mut g = game(&world(40, &[(11, 13, 'v')]));
    let v = |g: &Game| first(g, EntityKind::Enemy(EnemyKind::MozartkugelVerkaeufer));
    let thrown = |g: &Game| {
        g.entities()
            .iter()
            .filter(|e| e.kind == EntityKind::Thrown)
            .count()
    };
    hold(&mut g, "-", 300);
    assert_eq!(v(&g).dir, -1, "faces the player (left)");
    assert_eq!(thrown(&g), 0, "out of range: no throws");
    assert_eq!(v(&g).pose(&t), Some(Pose::Idle));
    // Step into range (centre ≥ 56): 16 px to walk.
    until(&mut g, "R", 60, |g| {
        (g.player().x + g.player().w.div_int(2)).to_px() >= 56
    });
    g.player.vx = Fx::ZERO;
    let mut windup = Vec::new();
    let mut throws = Vec::new();
    for f in 1..=250 {
        let before = thrown(&g);
        g.step(input("-"));
        if v(&g).pose(&t) == Some(Pose::Windup) {
            windup.push(f);
        }
        if thrown(&g) > before {
            throws.push(f);
            assert_eq!(v(&g).pose(&t), Some(Pose::Throw));
        }
    }
    // The counter started in the frame the player came into range (frame 0
    // here counted 1), so the throws are at counter 120 and 240.
    assert_eq!(throws.len(), 2, "{throws:?}");
    assert_eq!(throws[1] - throws[0], 120);
    let first_windup: Vec<i32> = windup.iter().copied().filter(|&f| f < throws[0]).collect();
    assert_eq!(first_windup.len(), 20, "20-frame wind-up");
    assert_eq!(*first_windup.last().unwrap(), throws[0] - 1);
    // Facing flips when the player passes it.
    g.player.x = Fx::from_px(300);
    g.step(input("-"));
    assert_eq!(v(&g).dir, 1);
}

#[test]
fn thrown_mozartkugel_flies_a_fixed_arc_and_vanishes_on_solid_tiles() {
    let t = tuning();
    let mut g = game(&world(40, &[(11, 6, 'v')]));
    park_player(&mut g);
    // Wait for the first throw (the player is in range).
    until(&mut g, "-", 200, |g| {
        g.entities().iter().any(|e| e.kind == EntityKind::Thrown)
    });
    let k = first(&g, EntityKind::Thrown);
    assert_eq!(k.vx, -t.enemies.thrown_speed_x, "towards the player (left)");
    assert_eq!(k.vy, -t.enemies.thrown_speed_y);
    assert_eq!((k.w, k.h), (Fx::from_px(6), Fx::from_px(6)));
    let start = k;
    let mut top = k.y;
    let mut prev = k;
    let mut landed_at = None;
    for _ in 0..200 {
        g.step(input("-"));
        match g.entities().iter().find(|e| e.kind == EntityKind::Thrown) {
            Some(e) => {
                assert_eq!(e.x, prev.x + prev.vx, "constant horizontal speed");
                assert_eq!(e.vy, prev.vy + t.enemies.thrown_gravity);
                top = top.min(e.y);
                prev = *e;
            }
            None => {
                landed_at = Some(prev);
                break;
            }
        }
    }
    let apex = (start.y - top).to_px();
    assert!((34..=38).contains(&apex), "apex ~36 px: {apex}");
    let last = landed_at.expect("vanished");
    // It hit the ground (row 12 top = 192) and vanished there.
    assert!(last.y + last.h + last.vy >= Fx::from_px(192));
    // Range at throw height ~4 tiles (48 frames · 1.25 px = 60 px).
    let back_at_start = (start.x - last.x).to_px();
    assert!(back_at_start >= 56, "range {back_at_start}");
}

#[test]
fn thrown_mozartkugel_hurts_and_cannot_be_stomped() {
    let t = tuning();
    let mut g = game(&world(40, &[(11, 6, 'v')]));
    g.player.set_power(Power::Big, &t.player);
    g.player.transform = 0;
    // A projectile right above the player, falling onto it: contact hurts
    // even from above.
    let mut k = Entity::at_feet(
        EntityKind::Thrown,
        g.player().x + g.player().w.div_int(2),
        g.player().y - Fx::from_px(1),
        t.enemies.thrown_hitbox,
    );
    k.vy = Fx::from_px(2);
    g.entities.push(k);
    g.player.vy = Fx::from_px(1);
    let events = g.step(input("-")).to_vec();
    assert!(events.contains(&Event::Hurt), "{events:?}");
    assert!(!events.contains(&Event::Stomped));
    assert_eq!(g.player().power, Power::Normal);
    assert_eq!(g.score(), 0);
}

// ----------------------------------------------------------------- Fiaker

#[test]
fn fiaker_telegraphs_40_frames_then_charges_left_off_the_screen() {
    let t = tuning();
    // Fiaker at column 14 (centre x 232); the player (centre 40) walks
    // right until within 128 px.
    let mut g = game(&world(40, &[(11, 14, 'f')]));
    let f = |g: &Game| first(g, EntityKind::Enemy(EnemyKind::Fiaker));
    let parked = f(&g);
    hold(&mut g, "-", 30);
    assert_eq!((f(&g).x, f(&g).pose(&t)), (parked.x, Some(Pose::Idle)));
    until(&mut g, "R", 200, |g| f(g).state == state::TELEGRAPH);
    let dist = (f(&g).x + f(&g).w.div_int(2) - g.player().x - g.player().w.div_int(2)).to_px();
    assert!(dist <= 128 && dist > 125, "triggers at 8 tiles: {dist}");
    // Back off out of reach: it charges anyway (frame counters, not aim).
    let mut telegraph = 1;
    loop {
        g.step(input("L"));
        let e = f(&g);
        if e.state != state::TELEGRAPH {
            break;
        }
        assert_eq!(e.x, parked.x, "stands while telegraphing");
        assert_eq!(e.pose(&t), Some(Pose::Telegraph));
        telegraph += 1;
    }
    assert_eq!(telegraph, 40, "40-frame telegraph");
    let e = f(&g);
    assert_eq!(e.pose(&t), Some(Pose::Charge));
    assert_eq!(e.x, parked.x - Fx::from_px(2), "2 px/frame left");
    // Charges until it has fully left the screen (camera x 0), then is gone.
    park_player(&mut g);
    g.player.x = Fx::from_px(400);
    g.player.y = Fx::from_px(40);
    let mut frames = 0;
    while g
        .entities()
        .iter()
        .any(|e| e.kind == EntityKind::Enemy(EnemyKind::Fiaker))
    {
        let before = f(&g);
        g.step(input("-"));
        frames += 1;
        assert!(frames < 200);
        if let Some(e) = g
            .entities()
            .iter()
            .find(|e| e.kind == EntityKind::Enemy(EnemyKind::Fiaker))
        {
            assert_eq!(e.x, before.x - Fx::from_px(2));
            assert!(e.right() > Fx::ZERO);
        }
    }
}

#[test]
fn fiaker_hurts_from_the_side() {
    let mut g = game(&world(40, &[(11, 6, 'f')]));
    until(&mut g, "R", 200, |g| g.mode() != Mode::Playing);
    assert_eq!(g.death_cause(), Some(DeathCause::Hit));
}

fn fiaker_index(g: &Game) -> usize {
    g.entities()
        .iter()
        .position(|e| e.kind == EntityKind::Enemy(EnemyKind::Fiaker))
        .unwrap()
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

#[test]
fn a_charging_fiaker_reaching_a_falling_player_bounces_her_onto_its_top() {
    // The Fiaker (top 164, middle 178) is already charging; the player
    // hangs 1 px left of its front, falling slowly with her feet 10 px
    // below its top (the QA bug replay: feet 174). Its front drives into
    // her: the stomp rule holds (falling, feet above its middle on the
    // previous frame), so she bounces - feet exactly on its top - and is
    // never hurt afterwards (D-027).
    let mut g = game(&world(40, &[(11, 14, 'f')]));
    let i = fiaker_index(&g);
    g.entities[i].active = true;
    g.entities[i].state = state::CHARGE;
    let f = g.entities()[i];
    assert_eq!(f.y, Fx::from_px(164));
    let x = f.x - g.player.w - Fx::from_px(1);
    airborne(&mut g, x, f.y + Fx::from_px(10), Fx::from_raw(64));
    let events = g.step(input("-")).to_vec();
    assert!(events.contains(&Event::Stomped), "{events:?}");
    assert_eq!(g.player().feet().1, f.y, "feet put on its top");
    assert!(g.player().vy < Fx::ZERO, "bouncing up");
    assert_eq!(g.player().bounce_grace, 0, "no fallback needed");
    let events = hold(&mut g, "-", 120);
    assert!(!events.contains(&Event::Hurt), "{events:?}");
    assert_eq!((g.mode(), g.deaths()), (Mode::Playing, 0));
}

#[test]
fn landing_straight_down_on_the_fiaker_with_up_to_4_px_penetration_bounces() {
    // Parked Fiaker; the player falls at max fall speed (4 px/frame)
    // straight down onto its middle and ends the frame 1..=4 px inside it.
    let max_fall = tuning().player.max_fall_speed;
    for pen in 1..=4 {
        let mut g = game(&world(40, &[(11, 14, 'f')]));
        let f = g.entities()[fiaker_index(&g)];
        let x = f.x + f.w.div_int(2) - g.player.w.div_int(2);
        airborne(&mut g, x, f.y + Fx::from_px(pen) - max_fall, max_fall);
        let events = g.step(input("-")).to_vec();
        assert!(events.contains(&Event::Stomped), "pen {pen}: {events:?}");
        // D-113: a roof bounce only, so also `Bounced`.
        assert!(events.contains(&Event::Bounced), "pen {pen}: {events:?}");
        assert_eq!(g.player().feet().1, f.y, "pen {pen}: feet on its top");
        let events = hold(&mut g, "-", 30);
        assert!(!events.contains(&Event::Hurt), "pen {pen}: {events:?}");
        assert_eq!((g.mode(), g.deaths()), (Mode::Playing, 0), "pen {pen}");
        assert_eq!(g.score(), 0, "pen {pen}: no points for the Fiaker");
    }
}

/// D-113: a step that stomps an enemy and bounces off a Fiaker's top at
/// once is a real stomp: `Stomped` without `Bounced`.
#[test]
fn a_stomp_and_a_roof_bounce_in_one_step_is_no_bounced() {
    let max_fall = tuning().player.max_fall_speed;
    let mut g = game(&world(40, &[(11, 14, 'f'), (11, 24, 'T')]));
    g.step(input("-"));
    let f = g.entities()[fiaker_index(&g)];
    // The still tourist moved flush right of the Fiaker, its top level
    // with the Fiaker's.
    let ti = g
        .entities()
        .iter()
        .position(|e| e.kind == EntityKind::Enemy(EnemyKind::SelfieTourist))
        .unwrap();
    let t = &mut g.entities[ti];
    t.x = f.x + f.w;
    t.y = f.y;
    t.prev_y = f.y;
    // Her hitbox straddles the seam.
    let x = f.x + f.w - g.player.w.div_int(2);
    airborne(&mut g, x, f.y + Fx::from_px(2) - max_fall, max_fall);
    let events = g.step(input("-")).to_vec();
    assert!(events.contains(&Event::Stomped), "{events:?}");
    assert!(!events.contains(&Event::Bounced), "{events:?}");
    assert!(
        g.entities()
            .iter()
            .any(|e| e.kind == EntityKind::Defeated(EnemyKind::SelfieTourist)),
        "the tourist was stomped"
    );
    assert_eq!(
        first(&g, EntityKind::Enemy(EnemyKind::Fiaker)).kind,
        EntityKind::Enemy(EnemyKind::Fiaker)
    );
}

#[test]
fn a_fiaker_bounce_under_a_ceiling_falls_back_to_grace_frames() {
    // A solid (or hazard) tile at row 8 (y 128..144) over the parked
    // Fiaker (top 164): feet on its top (hitbox y 142) would overlap it, so
    // the player stays where she is, bounces, and non-stompable enemies do
    // no harm for `bounce_grace_frames` (every further bounce re-arms it).
    let t = tuning();
    for ceiling in ['#', '^'] {
        let mut g = game(&world(
            40,
            &[(11, 14, 'f'), (8, 14, ceiling), (8, 15, ceiling)],
        ));
        let f = g.entities()[fiaker_index(&g)];
        let x = f.x + f.w.div_int(2) - g.player.w.div_int(2);
        airborne(&mut g, x, f.y + Fx::from_px(6), Fx::from_px(2));
        let y0 = g.player().y;
        let events = g.step(input("-")).to_vec();
        assert!(events.contains(&Event::Stomped), "{ceiling}: {events:?}");
        assert!(g.player().y > y0, "{ceiling}: not pushed up into it");
        assert_eq!(g.mode(), Mode::Playing, "{ceiling}");
        // Counted down from this frame on (like `invulnerable`).
        assert_eq!(
            g.player().bounce_grace,
            t.enemies.bounce_grace_frames - 1,
            "{ceiling}"
        );
        let hb = g.player().hitbox();
        assert!(!g.map().touches(&hb, crate::level::TileKind::Solid));
        assert!(!g.map().touches(&hb, crate::level::TileKind::Hazard));
        let events = hold(&mut g, "-", 60);
        assert!(!events.contains(&Event::Hurt), "{ceiling}: {events:?}");
        if ceiling == '#' {
            assert_eq!((g.mode(), g.deaths()), (Mode::Playing, 0));
        } else {
            // Rising into the hazard itself still kills - but never the
            // Fiaker.
            assert_ne!(g.death_cause(), Some(DeathCause::Hit));
        }
    }
}

#[test]
fn an_enemy_hit_in_the_frame_of_the_goal_touch_is_ignored() {
    // Goal column 10 (x 160..176); a still tourist moved to x 160 overlaps
    // it. The running player's right edge goes from 159 to 161.5 in one
    // step: she touches the tourist and the goal in the same frame, and
    // the goal wins (D-027). Without the goal the same step kills her.
    for with_goal in [true, false] {
        let mut marks = vec![(11, 20, 'T')];
        if with_goal {
            marks.push((11, 10, 'G'));
        }
        let mut g = game(&world(30, &marks));
        let i = g
            .entities()
            .iter()
            .position(|e| e.kind == EntityKind::Enemy(EnemyKind::SelfieTourist))
            .unwrap();
        g.entities[i].x = Fx::from_px(160);
        g.entities[i].active = true;
        g.player.x = Fx::from_px(159) - g.player.w;
        g.player.vx = tuning().player.run_speed;
        let events = g.step(input("RB")).to_vec();
        if with_goal {
            assert!(events.contains(&Event::LevelComplete), "{events:?}");
            assert_eq!((g.mode(), g.deaths()), (Mode::StageClear, 0));
            assert_eq!(g.death_cause(), None);
        } else {
            assert!(
                matches!(g.mode(), Mode::Dying { .. }),
                "control: {events:?}"
            );
            assert_eq!(g.death_cause(), Some(DeathCause::Hit));
        }
    }
}

// --------------------------------------------------- bumped blocks

#[test]
fn bumping_a_block_knocks_off_the_enemy_standing_on_it() {
    let t = tuning();
    // Bricks at row 8, columns 1-3; a tourist stands on them at column 2
    // (it walks left: the bump comes within a few frames). The player
    // jumps from column 2 into the brick above.
    let mut g = game(&world(
        20,
        &[(8, 1, 'B'), (8, 2, 'B'), (8, 3, 'B'), (7, 2, 't')],
    ));
    let events = until(&mut g, "J", 40, |g| {
        g.events().contains(&Event::BlockBumped)
    });
    assert!(!events.contains(&Event::Died));
    let e = first(&g, EntityKind::Defeated(EnemyKind::SelfieTourist));
    assert_eq!(e.state, state::TUMBLE);
    assert_eq!(
        e.vy,
        -t.enemies.knock_speed + t.player.gravity,
        "knocked up (one frame of gravity)"
    );
    assert_eq!(g.score(), 100);
    assert!(
        g.entities()
            .iter()
            .any(|e| e.kind == EntityKind::Popup(crate::Popup::Score(100)))
    );
    // It tumbles down through everything and is removed.
    until(&mut g, "-", 200, |g| {
        !g.entities()
            .iter()
            .any(|e| matches!(e.kind, EntityKind::Defeated(_)))
    });
}

#[test]
fn bumping_a_block_lets_a_rolling_item_on_it_hop() {
    let t = tuning();
    let mut g = game(&world(20, &[(8, 2, 'B'), (8, 3, 'B'), (8, 4, 'B')]));
    // A rolling Mozartkugel standing on the brick above the player.
    let mut k = Entity::at_feet(
        EntityKind::Mozartkugel,
        Fx::from_px(40),
        Fx::from_px(8 * 16),
        t.items.mozartkugel_hitbox,
    );
    k.timer = 100;
    g.entities.push(k);
    until(&mut g, "J", 40, |g| {
        g.events().contains(&Event::BlockBumped)
    });
    let m = first(&g, EntityKind::Mozartkugel);
    assert_eq!(m.vy, -t.enemies.knock_speed + t.player.gravity, "hops");
}

// -------------------------------------------------------- Festungsbahn

/// Car spawn cell (col 4, row 11): x 64, bottom 192, top 168; it travels
/// to [+8, −5] tiles (x 192, top 88). A rock floor below; nothing above.
fn car_world() -> Level {
    world(40, &[(11, 4, 'F')])
}

fn car(g: &Game) -> Entity {
    first(g, EntityKind::Festungsbahn)
}

/// Puts the player standing on the car, `dx` px right of its left edge.
fn board(g: &mut Game, dx: i32) {
    let c = car(g);
    g.player.x = c.x + Fx::from_px(dx);
    g.player.y = c.y - g.player.h;
    g.player.vx = Fx::ZERO;
    g.player.vy = Fx::ZERO;
    g.player.on_ground = true;
}

#[test]
fn festungsbahn_waits_for_its_rider_then_travels_and_comes_back() {
    let mut g = game(&car_world());
    park_player(&mut g);
    let start = car(&g);
    assert_eq!((start.x, start.y), (Fx::from_px(64), Fx::from_px(168)));
    // Nobody on it: it waits at its lower end, however long.
    for _ in 0..600 {
        g.step(input("-"));
        assert_eq!((car(&g).x, car(&g).y), (start.x, start.y));
    }
    board(&mut g, 10);
    let mut path = Vec::new();
    for _ in 0..420 {
        g.step(input("-"));
        let c = car(&g);
        path.push((c.x, c.y));
    }
    // path[i] is after update i + 1.
    assert_ne!(path[0], (start.x, start.y), "leaves with its rider");
    let top = (Fx::from_px(64 + 128), Fx::from_px(168 - 80));
    assert_eq!(path[149], top, "arrives after 150 frames");
    assert_ne!(path[148], top);
    assert_eq!(path[209], top, "waits 60 frames at the top");
    assert_ne!(path[210], top);
    assert_eq!(path[359], (start.x, start.y), "back at the bottom");
    assert_eq!(path[360], (start.x, start.y), "stops there for a frame");
    assert_ne!(
        path[361],
        (start.x, start.y),
        "the rider is still on: up again"
    );
    // A straight line: every point on the segment, monotonic.
    for w in path[..150].windows(2) {
        assert!(w[1].0 > w[0].0 || w[1].1 < w[0].1, "moves every frame");
        assert!(w[1].0 >= w[0].0 && w[1].1 <= w[0].1);
    }
    // Off the car (on the floor left of it): back down, it waits again.
    hold(&mut g, "-", 360);
    g.player.x = Fx::from_px(8);
    g.player.y = Fx::from_px(192) - g.player.h;
    let mut away = 0;
    for _ in 0..800 {
        g.step(input("-"));
        away += u32::from((car(&g).x, car(&g).y) != (start.x, start.y));
    }
    assert!(away < 360, "came down and stayed ({away} frames away)");
    assert_eq!((car(&g).x, car(&g).y), (start.x, start.y));
}

/// A car placed at the upper end of its track (`to` goes down) travels
/// down first, then waits there for a rider and takes them back up.
/// Jumping off the waiting car in the frame JUMP is pressed: she is in the
/// air before the car looks for its rider, so it stays.
#[test]
fn a_jump_off_the_waiting_car_does_not_call_it() {
    let mut g = game(&car_world());
    park_player(&mut g);
    hold(&mut g, "-", 10);
    board(&mut g, 10);
    let events = g.step(input("J")).to_vec();
    assert!(events.contains(&Event::Jumped), "{events:?}");
    let c = car(&g);
    assert_eq!(c.state, crate::platform::car::WAIT_LOWER);
    assert_eq!((c.x, c.y), (Fx::from_px(64), Fx::from_px(168)));
    // She lands on it again: now it leaves.
    until(&mut g, "-", 120, |g| {
        car(g).state == crate::platform::car::UP
    });
    assert_eq!(g.player().y + g.player().h, car(&g).y);
}

/// Feet at the car's top height but only touching its left edge (her right
/// edge on `car.x`): the waiting car isn't called, a moving car doesn't
/// carry her.
#[test]
fn an_edge_only_touch_neither_calls_nor_carries() {
    let mut g = game(&car_world());
    park_player(&mut g);
    hold(&mut g, "-", 10);
    let place = |g: &mut Game| {
        let c = car(g);
        g.player.x = c.x - g.player.w;
        g.player.y = c.y - g.player.h;
        g.player.vx = Fx::ZERO;
        g.player.vy = Fx::ZERO;
        g.player.on_ground = true;
    };
    place(&mut g);
    g.step(input("-"));
    assert_eq!(
        car(&g).state,
        crate::platform::car::WAIT_LOWER,
        "not called"
    );
    // A car that is already rising: she isn't carried along.
    let i = g
        .entities
        .iter()
        .position(|e| e.kind == EntityKind::Festungsbahn)
        .unwrap();
    g.entities[i].state = crate::platform::car::UP;
    g.entities[i].count = 20;
    g.step(input("-"));
    place(&mut g);
    let before = (g.player().x, g.player().y);
    g.step(input("-"));
    let c = car(&g);
    assert!(c.vy < Fx::ZERO, "the car rose");
    assert_eq!(g.player().x, before.0, "not carried sideways");
    assert!(g.player().y >= before.1, "not lifted");
}

#[test]
fn festungsbahn_spawned_at_its_upper_end_comes_down_first() {
    // Spawn cell (col 4, row 6): top 88; the lower end is [+8, +5] tiles
    // away (x 192, top 168).
    let mut g = game(&world(40, &[(6, 4, 'D')]));
    park_player(&mut g);
    let top = car(&g);
    assert_eq!((top.x, top.y), (Fx::from_px(64), Fx::from_px(88)));
    let bottom = (Fx::from_px(192), Fx::from_px(168));
    let mut ys = Vec::new();
    for _ in 0..400 {
        g.step(input("-"));
        ys.push(car(&g).y);
    }
    assert!(ys[0] > top.y, "goes down at once");
    assert_eq!((car(&g).x, car(&g).y), bottom, "and waits at the bottom");
    assert!(
        ys[149..].iter().all(|&y| y == bottom.1),
        "{:?}",
        &ys[145..155]
    );
    board(&mut g, 10);
    g.step(input("-"));
    assert!(car(&g).y < bottom.1, "a rider takes it up");
    assert_eq!(g.player().y + g.player().h, car(&g).y, "carried");
}

#[test]
fn festungsbahn_never_despawns_behind_the_camera() {
    // The car (x 64..112) is activated and boarded, then the player of one
    // game jumps far right: the camera's left edge minus `despawn_margin`
    // is far past the car's right edge. The car stays and finishes its
    // ride exactly like in a game where the player rides along (D-027: a
    // player who fell off can always wait for it); back at the bottom it
    // waits for the next rider.
    let t = tuning();
    let level = world(80, &[(11, 4, 'F')]);
    let mut far = game(&level);
    let mut near = game(&level);
    park_player(&mut far);
    park_player(&mut near);
    far.step(input("-"));
    near.step(input("-"));
    board(&mut far, 10);
    board(&mut near, 10);
    far.step(input("-"));
    near.step(input("-"));
    far.teleport_player(1000);
    let despawn_x = Fx::from_px(far.camera().x - t.game.despawn_margin);
    assert!(car(&far).right() < despawn_x, "far behind the despawn line");
    let y0 = car(&far).y;
    let mut moved = false;
    for _ in 0..358 {
        far.step(input("-"));
        near.step(input("-"));
        let (a, b) = (car(&far), car(&near));
        assert_eq!((a.x, a.y), (b.x, b.y), "same ride");
        moved |= a.y != y0;
    }
    assert!(moved, "still travelling");
    hold(&mut far, "-", 300);
    let c = car(&far);
    assert_eq!((c.x, c.y, c.state), (Fx::from_px(64), Fx::from_px(168), 0));
}

#[test]
fn festungsbahn_carries_a_standing_player_exactly() {
    let mut g = game(&car_world());
    hold(&mut g, "-", 1);
    board(&mut g, 10);
    let mut moved = 0;
    for _ in 0..420 {
        let before = (car(&g), *g.player());
        let events = g.step(input("-")).to_vec();
        let (c, p) = (car(&g), *g.player());
        assert_eq!(p.y + p.h, c.y, "feet exactly on the car top");
        assert_eq!(p.x - c.x, Fx::from_px(10), "no sliding, no jitter");
        assert_eq!(
            p.x - before.1.x,
            c.x - before.0.x,
            "moved by the car's movement"
        );
        assert!(p.on_ground);
        assert!(!events.contains(&Event::Landed), "never airborne");
        moved += (c.x != before.0.x) as u32;
    }
    assert!(moved >= 290, "the car moved ({moved} frames)");
    assert_eq!(g.mode(), Mode::Playing);
}

#[test]
fn walking_on_the_moving_car_is_exact_and_the_player_can_jump_off() {
    let mut g = game(&car_world());
    hold(&mut g, "-", 70);
    board(&mut g, 2);
    // Walk right along the moving car: feet stay on its top, and the
    // horizontal movement is the car's plus the player's own speed.
    for _ in 0..20 {
        let before = (car(&g), *g.player());
        g.step(input("R"));
        let (c, p) = (car(&g), *g.player());
        assert_eq!(p.y + p.h, c.y);
        assert_eq!(p.x - before.1.x, (c.x - before.0.x) + p.vx);
    }
    // Jump off: leaves the car upwards.
    let events = g.step(input("RJ")).to_vec();
    assert!(events.contains(&Event::Jumped), "{events:?}");
    let p = *g.player();
    assert!(p.vy < Fx::ZERO && !p.on_ground);
    assert!(p.y + p.h < car(&g).y);
}

#[test]
fn falling_onto_the_car_lands_on_its_top_only_from_above() {
    let mut g = game(&car_world());
    hold(&mut g, "-", 100);
    let c = car(&g);
    g.player.x = c.x + Fx::from_px(16);
    g.player.y = c.y - g.player.h - Fx::from_px(30);
    g.player.vy = Fx::ZERO;
    g.player.on_ground = false;
    let events = until(&mut g, "-", 60, |g| g.player().on_ground);
    assert!(events.contains(&Event::Landed));
    assert_eq!(g.player().y + g.player().h, car(&g).y);
    // From below it is passed through (one-way): a jump from the ground
    // under the car's path goes up through it.
    let mut g = game(&car_world());
    hold(&mut g, "-", 100);
    let c = car(&g);
    g.player.x = c.x + Fx::from_px(16);
    g.player.y = c.y + Fx::from_px(4);
    g.player.vy = Fx::from_px(-4);
    g.player.on_ground = false;
    g.step(input("J"));
    assert!(!g.player().on_ground);
    assert!(
        g.player().y + g.player().h > car(&g).y,
        "inside, not snapped up"
    );
}

/// The car rises into a solid ceiling with the player on it: the ceiling
/// holds the player, the car moves on and the player drops through it
/// (one-way) to the floor below instead of being crushed.
#[test]
fn festungsbahn_under_a_ceiling_drops_the_player_instead_of_crushing() {
    // A ceiling at row 6 over columns 6-9: the car's top passes 96 px
    // (row 6 bottom) on its way to 88.
    let mut marks: Vec<(usize, usize, char)> = (6..=9).map(|c| (6, c, '#')).collect();
    marks.push((11, 4, 'F'));
    let level = world(40, &marks);
    let problems = crate::platform::track_problems(&level, &tuning());
    assert_eq!(problems.len(), 1, "the lint finds it: {problems:?}");
    let mut g = game(&level);
    hold(&mut g, "-", 1);
    board(&mut g, 18);
    let mut dropped = false;
    for _ in 0..420 {
        let events = g.step(input("-")).to_vec();
        assert!(!events.contains(&Event::Died), "never crushed");
        let p = *g.player();
        let hb = p.hitbox();
        assert!(
            !g.map().touches(&hb, crate::TileKind::Solid),
            "never inside a tile"
        );
        if p.y + p.h > car(&g).y + Fx::from_px(2) {
            dropped = true;
        }
    }
    assert!(dropped, "fell through the car");
    assert_eq!(g.mode(), Mode::Playing);
    assert!(g.player().on_ground);
    // Without the ceiling the track is clean.
    assert!(crate::platform::track_problems(&car_world(), &tuning()).is_empty());
}

/// A car rising into a player who stands on a one-way tile above its
/// track lifts them onto its top (they ride on from there).
#[test]
fn a_rising_car_lifts_a_player_standing_in_its_way() {
    let mut g = game(&world(40, &[(11, 4, 'F'), (10, 5, '-'), (10, 6, '-')]));
    hold(&mut g, "-", 1);
    g.player.x = Fx::from_px(82);
    g.player.y = Fx::from_px(160) - g.player.h;
    g.player.on_ground = true;
    // Someone else's ride: the car leaves its lower end.
    let car_index = g
        .entities
        .iter()
        .position(|e| e.kind == EntityKind::Festungsbahn)
        .unwrap();
    g.entities[car_index].state = crate::platform::car::UP;
    let mut lifted = false;
    for _ in 0..120 {
        let events = g.step(input("-")).to_vec();
        assert!(!events.contains(&Event::Died));
        let (p, c) = (*g.player(), car(&g));
        if p.y + p.h < Fx::from_px(160) {
            lifted = true;
            assert_eq!(p.y + p.h, c.y, "on the car top");
            assert!(p.on_ground);
        }
    }
    assert!(lifted, "lifted by the car");
}

/// Breaking a brick as Zuckerschock also knocks off the enemy on it.
#[test]
fn breaking_a_brick_knocks_off_the_enemy_standing_on_it() {
    let t = tuning();
    let mut g = game(&world(20, &[(8, 2, 'B'), (7, 2, 'v')]));
    g.player.set_power(Power::Big, &t.player);
    g.player.transform = 0;
    until(&mut g, "J", 40, |g| {
        g.events().contains(&Event::BrickBroken)
    });
    let e = first(&g, EntityKind::Defeated(EnemyKind::MozartkugelVerkaeufer));
    assert_eq!(e.state, state::TUMBLE);
    assert_eq!(g.score(), t.score.brick + t.score.bump_enemy);
}
