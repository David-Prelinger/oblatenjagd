//! The M3 enemies (game-design.md → "Stage 4: Falkenstein", entities and
//! "New mechanics"; D-058): Pilger, Gams, the Steinschlag spawner and its
//! falling rocks, and the defeat-by-projectile entry point for the thrown
//! Hackl (D-056).

use super::*;
use crate::enemies::{Pose, hop_is_clear, state};
use crate::{DeathCause, EnemyKind, Entity, EntityKind, Fx, Popup};

/// Legend of the test worlds (`.` air, `P` start).
const LEGEND: &str = r##"
"#" = "solid"
"-" = "oneway"
"?" = { tile = "oblaten_box" }
"p" = { entity = "pilger" }
"L" = { entity = "pilger", lead = true }
"a" = { entity = "gams" }
"r" = { entity = "steinschlag" }
"R" = { entity = "steinschlag", phase = 100 }
"T" = { entity = "selfie_tourist", still = true }
"^" = "hazard"
"~" = "salzach_water"
"P" = { entity = "player_start" }
"G" = { entity = "goal" }
"##;

/// A 14-row world `width` columns wide: ground in rows 12–13 (top at
/// 192 px), the player start at column 2 (row 11) unless `marks` places a
/// `P`, plus `marks` as (row, col, char). The camera starts at y 44, so
/// the HUD line (screen y 24) is level y 68: niches from row 5 on count as
/// on screen.
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
        "[meta]\nid = \"m3\"\ntime_limit = 300\n[legend]\n{LEGEND}\n[map]\ngrid = '''\n{}\n'''\n",
        grid.join("\n")
    );
    Level::parse(&text).unwrap()
}

/// `marks` for a solid block filling rows `rows` × columns `cols`.
fn block(
    rows: std::ops::RangeInclusive<usize>,
    cols: std::ops::RangeInclusive<usize>,
) -> Vec<(usize, usize, char)> {
    rows.flat_map(|r| cols.clone().map(move |c| (r, c, '#')))
        .collect()
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

fn all(g: &Game, kind: EntityKind) -> Vec<Entity> {
    g.entities()
        .iter()
        .filter(|e| e.kind == kind)
        .copied()
        .collect()
}

/// Steps once with the player kept invulnerable.
fn safe_step(g: &mut Game) -> Vec<Event> {
    g.player.invulnerable = u8::MAX;
    g.step(input("-")).to_vec()
}

/// Puts the player in the air above the middle of `e`, falling at 3 px
/// per frame with the feet 2 px above its top.
fn drop_onto(g: &mut Game, e: Entity) {
    g.player.x = e.x + e.w.div_int(2) - g.player.w.div_int(2);
    g.player.y = e.y - Fx::from_px(2) - g.player.h;
    g.player.vx = Fx::ZERO;
    g.player.vy = Fx::from_px(3);
    g.player.on_ground = false;
}

/// Puts the standing player's hitbox centre at `cx` px (no physics).
fn place_player(g: &mut Game, cx: i32) {
    g.player.x = Fx::from_px(cx) - g.player.w.div_int(2);
}

const PILGER: EntityKind = EntityKind::Enemy(EnemyKind::Pilger);
const GAMS: EntityKind = EntityKind::Enemy(EnemyKind::Gams);

// ----------------------------------------------------------------- Pilger

#[test]
fn a_pilger_walks_down_the_steps_towards_the_player() {
    let t = tuning();
    // Steps up to the right: 1 tile (cols 8-9), 2 (10-11), 3 (12-19); the
    // pilgrim starts on the top step at column 16.
    let mut marks = block(11..=11, 8..=19);
    marks.extend(block(10..=10, 10..=19));
    marks.extend(block(9..=9, 12..=19));
    marks.push((8, 16, 'p'));
    let mut g = game(&world(40, &marks));
    safe_step(&mut g);
    let p0 = first(&g, PILGER);
    assert_eq!(p0.dir, -1, "towards the player (left)");
    safe_step(&mut g);
    let p1 = first(&g, PILGER);
    assert_eq!(p0.x - p1.x, t.enemies.pilger_speed, "0.5 px/frame");
    let mut heights = vec![p1.feet().1];
    for _ in 0..600 {
        safe_step(&mut g);
        let p = first(&g, PILGER);
        let feet = p.feet().1;
        if p.vy == Fx::ZERO && heights.last() != Some(&feet) {
            heights.push(feet);
        }
        if feet == Fx::from_px(192) {
            break;
        }
    }
    let px = Fx::from_px;
    assert_eq!(
        heights,
        vec![px(144), px(160), px(176), px(192)],
        "step by step down to the ground"
    );
}

#[test]
fn a_pilger_faces_the_player_at_activation() {
    // Player at column 10, pilgrim at column 3: he walks right.
    let mut g = game(&world(30, &[(11, 10, 'P'), (11, 3, 'p')]));
    safe_step(&mut g);
    assert_eq!(first(&g, PILGER).dir, 1);
}

#[test]
fn a_pilger_turns_at_walls_and_at_enemies_on_foot() {
    // A wall at column 9; a pilgrim at column 12 walks left into it.
    let mut marks = block(10..=11, 9..=9);
    marks.push((11, 12, 'p'));
    let mut g = game(&world(30, &marks));
    for _ in 0..120 {
        safe_step(&mut g);
    }
    let p = first(&g, PILGER);
    assert_eq!(p.dir, 1, "turned at the wall");
    assert!(p.x > Fx::from_px(160), "walking away from it");
    // A still tourist at column 10, a pilgrim at column 14.
    let mut g = game(&world(30, &[(11, 10, 'T'), (11, 14, 'p')]));
    for _ in 0..150 {
        safe_step(&mut g);
    }
    let p = first(&g, PILGER);
    assert_eq!(p.dir, 1, "turned at the tourist");
}

#[test]
fn a_pilger_group_stays_in_file() {
    let mut g = game(&world(40, &[(11, 14, 'L'), (11, 15, 'p'), (11, 16, 'p')]));
    for _ in 0..160 {
        safe_step(&mut g);
    }
    let group = all(&g, PILGER);
    assert_eq!(group.len(), 3);
    assert!(group.iter().all(|p| p.dir == -1));
    for w in group.windows(2) {
        assert_eq!(w[1].x - w[0].x, Fx::from_px(16), "one cell apart");
    }
}

/// A 60-column world, the camera parked at x 0 (the player stands
/// still), so entities activate left of x 640: a file of pilgrims in row
/// 11 at columns 38–41 straddles that edge.
fn straddling(marks: &[(usize, usize, char)]) -> Game {
    let g = game(&world(60, marks));
    assert_eq!(g.camera().x, 0);
    g
}

#[test]
fn a_pilger_file_straddling_the_activation_edge_wakes_together() {
    // QA (E19): the camera stopped with the file across the edge and the
    // leader (column 39, x < 640) walked off alone; 40 and 41 are past it.
    let mut g = straddling(&[(11, 39, 'L'), (11, 40, 'p'), (11, 41, 'p')]);
    let edge = Fx::from_px(640);
    let before = all(&g, PILGER);
    assert!(before[0].x < edge && before[1].x >= edge && before[2].x >= edge);
    safe_step(&mut g);
    assert!(
        all(&g, PILGER).iter().all(|p| p.active),
        "all three at once"
    );
    for _ in 0..160 {
        safe_step(&mut g);
    }
    let group = all(&g, PILGER);
    assert_eq!(group.len(), 3);
    assert!(group.iter().all(|p| p.dir == -1));
    for w in group.windows(2) {
        assert_eq!(w[1].x - w[0].x, Fx::from_px(16), "still in file");
    }
    // The same file entered from its other end: the rule is per file,
    // whichever member wakes first.
    let mut g = straddling(&[(11, 38, 'p'), (11, 39, 'p'), (11, 40, 'L')]);
    safe_step(&mut g);
    assert!(all(&g, PILGER).iter().all(|p| p.active));
}

#[test]
fn only_adjacent_pilger_in_the_same_row_form_a_file() {
    // A gap of one cell, another row, another kind in between: separate.
    for marks in [
        vec![(11, 39, 'p'), (11, 41, 'p')],
        vec![(11, 39, 'p'), (10, 40, 'p')],
        vec![(11, 39, 'p'), (11, 40, 'a'), (11, 41, 'p')],
    ] {
        let mut g = straddling(&marks);
        safe_step(&mut g);
        // Left to right (the entity list is in row order).
        let mut pilgrims = all(&g, PILGER);
        pilgrims.sort_by_key(|p| p.x);
        let woke: Vec<bool> = pilgrims.iter().map(|p| p.active).collect();
        assert_eq!(woke, vec![true, false], "{marks:?}");
    }
}

#[test]
fn lead_only_changes_the_look() {
    let run = |ch: char| {
        let mut g = game(&world(30, &[(11, 14, ch)]));
        for _ in 0..200 {
            safe_step(&mut g);
        }
        let p = first(&g, PILGER);
        (p.x, p.y, p.dir, p.w, p.h)
    };
    assert_eq!(run('p'), run('L'));
    let level = world(30, &[(11, 14, 'L')]);
    assert!(level.entities.iter().any(|s| s.lead));
}

#[test]
fn stomping_a_pilger_scores_100() {
    let mut g = game(&world(30, &[(11, 14, 'p')]));
    safe_step(&mut g);
    let p = first(&g, PILGER);
    drop_onto(&mut g, p);
    let events = g.step(input("-")).to_vec();
    assert!(events.contains(&Event::Stomped));
    assert_eq!(g.score(), 100);
    assert!(find(&g, EntityKind::Defeated(EnemyKind::Pilger)).is_some());
    assert_eq!(EnemyKind::Pilger.chain_start(), 0);
    assert!(EnemyKind::Pilger.stompable() && !EnemyKind::Pilger.hurts_from_above());
}

// ------------------------------------------------------------------- Gams

/// Runs a Gams on flat ground at column 12 (the player at column 2, to
/// the left) and returns (state, count, x, feet y, pose) for frames
/// 1..=`frames`.
fn gams_frames(frames: u32) -> Vec<(u8, u16, Fx, Fx, Pose)> {
    let t = tuning();
    let mut g = game(&world(30, &[(11, 12, 'a')]));
    (0..frames)
        .map(|_| {
            safe_step(&mut g);
            let e = first(&g, GAMS);
            (e.state, e.count, e.x, e.feet().1, e.pose(&t).unwrap())
        })
        .collect()
}

#[test]
fn the_gams_cycle_is_stand_60_crouch_16_hop_32() {
    let f = gams_frames(220);
    let x0 = f[0].2;
    let ground = Fx::from_px(192);
    for (i, &(st, _, x, feet, pose)) in f.iter().enumerate() {
        let frame = i + 1;
        let want = match frame {
            1..=60 | 109..=168 => (state::STAND, Pose::Idle),
            61..=76 | 169..=184 => (state::CROUCH, Pose::Crouch),
            77..=107 | 185..=215 => (state::HOP, Pose::Hop),
            _ => (state::STAND, Pose::Idle),
        };
        assert_eq!((st, pose), want, "frame {frame}");
        if frame <= 76 {
            assert_eq!(
                (x, feet),
                (x0, ground),
                "still until the hop, frame {frame}"
            );
        }
    }
    // The hop: 2 tiles towards the player, back on the ground on frame 108
    // (its 32nd), apex 30 px.
    assert_eq!(f[107].2, x0 - Fx::from_px(32));
    assert_eq!(f[107].3, ground);
    assert_eq!(
        f[106].3, ground,
        "level again after 31 frames, landing on the 32nd"
    );
    let apex = f[76..108].iter().map(|s| s.3).min().unwrap();
    assert_eq!(ground - apex, Fx::from_px(30), "apex ~2 tiles");
    assert_eq!(f[215].2, x0 - Fx::from_px(64), "second hop, period 108");
}

#[test]
fn the_gams_turns_at_ledge_ends_and_walls() {
    // A floating ledge (row 9, top 144) at columns 10-16, the Gams at its
    // left end; the player (left) is off the ledge: it turns right.
    let mut marks = block(9..=9, 10..=16);
    marks.push((8, 10, 'a'));
    let mut g = game(&world(30, &marks));
    for _ in 0..108 {
        safe_step(&mut g);
    }
    let e = first(&g, GAMS);
    assert_eq!((e.dir, e.x), (1, Fx::from_px(160 + 32)), "hopped right");
    assert_eq!(e.feet().1, Fx::from_px(144));
    // A wall (2 tiles high) at column 10 next to it on the ground.
    let mut marks = block(10..=11, 10..=10);
    marks.push((11, 11, 'a'));
    let mut g = game(&world(30, &marks));
    for _ in 0..108 {
        safe_step(&mut g);
    }
    let e = first(&g, GAMS);
    assert_eq!(
        (e.dir, e.x),
        (1, Fx::from_px(176 + 32)),
        "turned at the wall"
    );
}

#[test]
fn a_boxed_in_gams_hops_in_place() {
    // A single-tile pillar top: no room either side.
    let mut marks = block(9..=9, 10..=10);
    marks.push((8, 10, 'a'));
    let mut g = game(&world(30, &marks));
    let mut apex = Fx::MAX;
    for frame in 1..=108 {
        safe_step(&mut g);
        let e = first(&g, GAMS);
        assert_eq!(e.x, Fx::from_px(160), "frame {frame}");
        apex = apex.min(e.feet().1);
    }
    let e = first(&g, GAMS);
    assert_eq!(e.dir, -1, "still facing the player");
    assert_eq!(e.feet().1, Fx::from_px(144));
    assert_eq!(apex, Fx::from_px(144 - 30));
}

#[test]
fn the_gams_never_leaves_its_ledge() {
    // Ledge A (columns 10-14) and, across a 1-tile gap, ledge B (16-20):
    // landing 2 tiles further would be on B, but the sweep has no floor.
    for player_col in [2, 26] {
        let mut marks = block(9..=9, 10..=14);
        marks.extend(block(9..=9, 16..=20));
        marks.push((8, 12, 'a'));
        marks.push((11, player_col, 'P'));
        let mut g = game(&world(30, &marks));
        for frame in 0..2000 {
            safe_step(&mut g);
            let e = first(&g, GAMS);
            assert!(
                e.x >= Fx::from_px(160) && e.right() <= Fx::from_px(240),
                "player at {player_col}, frame {frame}: x {:?}",
                e.x
            );
            assert!(e.feet().1 <= Fx::from_px(144), "never below its ledge");
        }
    }
}

#[test]
fn the_hop_check_needs_floor_along_the_whole_hop() {
    let t = tuning();
    let mut marks = block(9..=9, 10..=12);
    marks.extend(block(9..=9, 14..=16));
    marks.push((8, 12, 'a'));
    let g = game(&world(30, &marks));
    let e = Entity::from_spawn(
        g.level()
            .entities
            .iter()
            .find(|s| s.kind == crate::SpawnKind::Gams)
            .unwrap(),
        0,
        &t,
    )
    .unwrap();
    assert!(hop_is_clear(&e, g.map(), -1, &t), "columns 10-12 are floor");
    assert!(!hop_is_clear(&e, g.map(), 1, &t), "gap at column 13");
    let mut air = e;
    air.y -= Fx::from_px(1);
    assert!(!hop_is_clear(&air, g.map(), -1, &t), "only from a floor");
}

#[test]
fn stomping_a_gams_scores_200_and_starts_the_chain_at_200() {
    let mut g = game(&world(30, &[(11, 12, 'a')]));
    safe_step(&mut g);
    let a = first(&g, GAMS);
    drop_onto(&mut g, a);
    let events = g.step(input("-")).to_vec();
    assert!(events.contains(&Event::Stomped));
    assert_eq!(g.score(), 200);
    assert_eq!(g.player().stomp_chain, 2, "the next stomp is worth 400");
    assert_eq!(EnemyKind::Gams.chain_start(), 1);
}

#[test]
fn a_bumped_block_knocks_the_gams_off() {
    let t = tuning();
    // An Oblaten box at (8, 2) right above the player, the Gams on it.
    let mut g = game(&world(20, &[(8, 2, '?'), (7, 2, 'a')]));
    until(&mut g, "J", 40, |g| {
        g.events().contains(&Event::BlockBumped)
    });
    let e = first(&g, EntityKind::Defeated(EnemyKind::Gams));
    assert_eq!(e.state, state::TUMBLE);
    assert_eq!(g.score(), t.score.oblate + t.score.bump_enemy);
}

// ------------------------------------------------------------ Steinschlag

const NICHE: EntityKind = EntityKind::Steinschlag;
const STEIN: EntityKind = EntityKind::Stein;

#[test]
fn the_rock_counter_runs_only_while_the_player_is_in_range() {
    // Niche at (5, 12): centre x 200; range 6 tiles = 96 px.
    let mut g = game(&world(30, &[(5, 12, 'r')]));
    for _ in 0..100 {
        safe_step(&mut g);
    }
    assert_eq!(first(&g, NICHE).state, state::WAITING, "player 162 px away");
    place_player(&mut g, 200 - 96);
    safe_step(&mut g);
    let n = first(&g, NICHE);
    assert_eq!(
        (n.state, n.count),
        (state::COUNTING, 0),
        "starts at phase 0"
    );
    safe_step(&mut g);
    assert_eq!(first(&g, NICHE).count, 1);
    place_player(&mut g, 200 - 97);
    for _ in 0..30 {
        safe_step(&mut g);
    }
    assert_eq!(first(&g, NICHE).count, 1, "paused out of range");
    assert_eq!(
        first(&g, NICHE).pose(&tuning()),
        Some(Pose::Wobble),
        "pose kept"
    );
    place_player(&mut g, 200 + 96);
    safe_step(&mut g);
    assert_eq!(first(&g, NICHE).count, 2, "in range on the other side");
}

#[test]
fn the_rock_counter_runs_only_with_the_niche_on_screen_below_the_hud() {
    // Camera y 44: the HUD line is level y 68. A niche at row 4 (top 64)
    // is under the HUD, one at row 5 (top 80) is on screen.
    for (row, counts) in [(1, false), (4, false), (5, true)] {
        let mut g = game(&world(30, &[(row, 12, 'r'), (11, 9, 'P')]));
        assert_eq!(g.camera().y, 44);
        for _ in 0..10 {
            safe_step(&mut g);
        }
        let n = first(&g, NICHE);
        assert_eq!(n.state == state::COUNTING, counts, "row {row}");
    }
}

#[test]
fn the_telegraph_is_40_frames_then_the_rock_drops_and_the_niche_refills() {
    let t = tuning();
    // The player stands in range from the start (column 9, 48 px away).
    let mut g = game(&world(30, &[(5, 12, 'r'), (11, 9, 'P')]));
    for k in 1..=160u32 {
        safe_step(&mut g);
        let n = first(&g, NICHE);
        assert_eq!(u32::from(n.count), (k - 1) % 150, "frame {k}");
        let want = match k {
            1..=40 | 151..=160 => Some(Pose::Wobble),
            41..=70 => None,
            _ => Some(Pose::Rest),
        };
        assert_eq!(n.pose(&t), want, "frame {k}");
        let rocks = all(&g, STEIN).len();
        if k < 41 {
            assert_eq!(rocks, 0, "frame {k}");
        }
        if k == 41 {
            assert_eq!(rocks, 1, "the drop at counter frame 40");
            let r = first(&g, STEIN);
            assert_eq!((r.x, r.y), (n.x, n.y), "from the resting position");
        }
    }
}

#[test]
fn a_phase_starts_the_counter_later_in_its_period() {
    let mut g = game(&world(30, &[(5, 12, 'R'), (11, 9, 'P')]));
    for k in 1..=91 {
        safe_step(&mut g);
        if k == 1 {
            assert_eq!(first(&g, NICHE).count, 100);
            assert_eq!(first(&g, NICHE).pose(&tuning()), Some(Pose::Rest));
        }
        assert_eq!(find(&g, STEIN).is_some(), k == 91, "frame {k}");
    }
}

#[test]
fn a_rock_falls_with_gravity_and_shatters_on_the_ground() {
    let t = tuning();
    let mut g = game(&world(30, &[(5, 12, 'r'), (11, 9, 'P')]));
    until(&mut g, "-", 60, |g| find(g, STEIN).is_some());
    let mut speeds = Vec::new();
    loop {
        safe_step(&mut g);
        match find(&g, STEIN) {
            Some(r) => speeds.push(r.vy),
            None => break,
        }
    }
    let g64 = t.enemies.stein_gravity;
    assert_eq!(&speeds[..3], &[g64, g64.mul_int(2), g64.mul_int(3)]);
    assert!(speeds.iter().all(|&v| v <= t.enemies.stein_max_fall));
    let debris = all(&g, EntityKind::SteinDebris);
    assert_eq!(debris.len(), 4, "shattered");
    // Shattered with its bottom on the ground (row 12, top 192): the
    // lower pieces start at the rock's centre, 6 px above.
    assert_eq!(debris[2].y, Fx::from_px(192 - 6));
}

#[test]
fn a_rock_reaches_max_fall_speed() {
    let t = tuning();
    // A tall column: niche at row 31 of a 40-row world (ground at row 38;
    // the camera is at y 460, so the HUD line is level y 484).
    let rows = 40;
    let mut grid: Vec<Vec<char>> = (0..rows)
        .map(|r| vec![if r >= rows - 2 { '#' } else { '.' }; 30])
        .collect();
    grid[rows - 3][9] = 'P';
    grid[rows - 9][12] = 'r';
    let text = format!(
        "[meta]\nid = \"m3\"\ntime_limit = 300\n[legend]\n{LEGEND}\n[map]\ngrid = '''\n{}\n'''\n",
        grid.into_iter()
            .map(|r| r.into_iter().collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
    );
    let mut g = game(&Level::parse(&text).unwrap());
    until(&mut g, "-", 60, |g| find(g, STEIN).is_some());
    let mut top = Fx::ZERO;
    while let Some(r) = find(&g, STEIN) {
        top = top.max(r.vy);
        safe_step(&mut g);
    }
    assert_eq!(top, t.enemies.stein_max_fall);
}

#[test]
fn a_rock_shatters_on_a_one_way_net() {
    // A one-way tile 3 tiles above the ground under the column.
    let mut g = game(&world(30, &[(5, 12, 'r'), (9, 12, '-'), (11, 9, 'P')]));
    until(&mut g, "-", 60, |g| find(g, STEIN).is_some());
    until(&mut g, "-", 60, |g| find(g, STEIN).is_none());
    let debris = all(&g, EntityKind::SteinDebris);
    assert_eq!(debris.len(), 4);
    assert_eq!(debris[2].y, Fx::from_px(144 - 6), "on the net (top 144)");
}

#[test]
fn a_rock_hurts_from_above_and_from_below_and_cannot_be_stomped() {
    let t = tuning();
    // Stomping it: a hurt (Normal → death), never a stomp.
    let mut g = game(&world(30, &[(11, 9, 'P')]));
    let rock = crate::enemies::resting_rock(EntityKind::Stein, 12, 8, &t);
    g.entities.push(rock);
    drop_onto(&mut g, rock);
    let events = g.step(input("-")).to_vec();
    assert!(!events.contains(&Event::Stomped));
    assert!(events.contains(&Event::Died), "{events:?}");
    assert_eq!(g.death_cause(), Some(DeathCause::Hit));
    // Standing under the column: the falling rock hits her head.
    let mut g = game(&world(30, &[(5, 12, 'r'), (11, 11, 'P')]));
    place_player(&mut g, 200);
    let events = until(&mut g, "-", 200, |g| g.mode() != crate::Mode::Playing);
    assert!(events.contains(&Event::Died));
    assert_eq!(g.death_cause(), Some(DeathCause::Hit));
}

#[test]
fn a_rock_falls_through_water_and_hazard_tiles() {
    // Spikes at row 8 and water at row 10 in the column: the rock passes
    // both and shatters on the ground (top 192).
    let mut g = game(&world(
        30,
        &[(5, 12, 'r'), (8, 12, '^'), (10, 12, '~'), (11, 9, 'P')],
    ));
    until(&mut g, "-", 60, |g| find(g, STEIN).is_some());
    until(&mut g, "-", 80, |g| find(g, STEIN).is_none());
    let debris = all(&g, EntityKind::SteinDebris);
    assert_eq!(debris.len(), 4);
    assert_eq!(debris[2].y, Fx::from_px(192 - 6), "on the ground");
}

#[test]
fn a_rock_falling_out_of_the_level_is_removed_without_debris() {
    // No ground under the column (a pit at column 12).
    let mut g = game(&world(
        30,
        &[(5, 12, 'r'), (12, 12, '.'), (13, 12, '.'), (11, 9, 'P')],
    ));
    until(&mut g, "-", 60, |g| find(g, STEIN).is_some());
    until(&mut g, "-", 120, |g| find(g, STEIN).is_none());
    assert!(find(&g, EntityKind::SteinDebris).is_none(), "no debris");
    assert_eq!(g.mode(), crate::Mode::Playing);
}

#[test]
fn the_niche_must_be_fully_on_screen_left_and_bottom() {
    // Left edge: niche and player in column 9 (distance 0). With the
    // camera 2 px right of the niche's left edge it doesn't count; with the
    // edge exactly on screen it does.
    for (cam_x, counts) in [(146, false), (144, true)] {
        let mut g = game(&world(40, &[(5, 9, 'r'), (11, 9, 'P')]));
        g.camera.x = cam_x;
        safe_step(&mut g);
        assert_eq!(g.camera().x, cam_x, "camera kept");
        let n = first(&g, NICHE);
        assert_eq!(n.state == state::COUNTING, counts, "camera x {cam_x}");
    }
    // Bottom edge: ground at rows 12-13 of a 24-row level with air below,
    // so the camera rests at y 89 (screen bottom 269): a niche at row 15
    // (bottom 256) is on screen, one at row 16 (bottom 272) is not.
    for (row, counts) in [(15, true), (16, false)] {
        let mut rows: Vec<Vec<char>> = (0..24)
            .map(|r| vec![if (12..=13).contains(&r) { '#' } else { '.' }; 30])
            .collect();
        rows[11][9] = 'P';
        rows[row][12] = 'r';
        let text = format!(
            "[meta]\nid = \"m3\"\ntime_limit = 300\n[legend]\n{LEGEND}\n[map]\ngrid = '''\n{}\n'''\n",
            rows.into_iter()
                .map(|r| r.into_iter().collect::<String>())
                .collect::<Vec<_>>()
                .join("\n")
        );
        let mut g = game(&Level::parse(&text).unwrap());
        for _ in 0..10 {
            safe_step(&mut g);
        }
        assert_eq!(g.camera().y, 89);
        let n = first(&g, NICHE);
        assert_eq!(n.state == state::COUNTING, counts, "row {row}");
    }
}

#[test]
fn a_rock_passes_through_enemies() {
    // A still tourist right under the column: the rock falls through him
    // and shatters on the ground; he is unharmed.
    let mut g = game(&world(30, &[(5, 12, 'r'), (11, 12, 'T'), (11, 9, 'P')]));
    until(&mut g, "-", 60, |g| find(g, STEIN).is_some());
    until(&mut g, "-", 60, |g| find(g, STEIN).is_none());
    assert_eq!(all(&g, EntityKind::SteinDebris).len(), 4);
    assert!(find(&g, EntityKind::Enemy(EnemyKind::SelfieTourist)).is_some());
    assert_eq!(g.score(), 0);
}

#[test]
fn the_niche_itself_is_harmless() {
    // The player jumps through the resting rock's niche cell.
    let mut g = game(&world(30, &[(10, 4, 'r')]));
    place_player(&mut g, 4 * 16 + 8);
    let events = hold(&mut g, "J", 40);
    assert!(!events.contains(&Event::Hurt) && !events.contains(&Event::Died));
}

// ------------------------------------------------- defeat by projectile

#[test]
fn defeat_by_projectile_pays_base_points_and_shatters_rocks() {
    let t = tuning();
    let mut g = game(&world(30, &[(11, 10, 'p'), (11, 14, 'a'), (11, 18, 'T')]));
    safe_step(&mut g);
    let rock = crate::enemies::resting_rock(EntityKind::Stein, 22, 6, &t);
    g.entities.push(rock);
    let at = |g: &Game, kind| g.entities().iter().position(|e| e.kind == kind).unwrap();
    let i = at(&g, PILGER);
    assert!(g.defeat_by_projectile(i));
    assert_eq!(g.score(), 100);
    let i = at(&g, GAMS);
    assert!(g.defeat_by_projectile(i));
    assert_eq!(g.score(), 300, "Gams 200 (not in the stomp chain)");
    assert_eq!(g.player().stomp_chain, 0);
    for kind in [EnemyKind::Pilger, EnemyKind::Gams] {
        let d = first(&g, EntityKind::Defeated(kind));
        assert_eq!(d.state, state::TUMBLE, "{kind:?} tumbles off the screen");
    }
    let i = at(&g, STEIN);
    assert!(g.defeat_by_projectile(i));
    assert_eq!(g.score(), 350, "rock 50");
    assert_eq!(all(&g, EntityKind::SteinDebris).len(), 4);
    assert!(g.entities()[i].removed);
    assert!(!g.defeat_by_projectile(i), "already gone");
    for p in [100, 200, 50] {
        assert!(
            g.entities()
                .iter()
                .any(|e| e.kind == EntityKind::Popup(Popup::Score(p)))
        );
    }
    // One path for every kind (D-060): the M1/M2 stompable enemies too.
    let i = at(&g, EntityKind::Enemy(EnemyKind::SelfieTourist));
    assert!(g.defeat_by_projectile(i));
    assert_eq!(g.score(), 450);
}
