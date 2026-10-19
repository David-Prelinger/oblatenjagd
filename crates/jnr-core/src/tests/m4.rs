//! The M4 enemies of Stage 5 (game-design.md → "Stage 5: St. Wolfgang",
//! entities and "New mechanics"; D-068, D-069, D-071): the Operettensänger
//! with his high-C column, the Souvenirstand spawner and its rolling
//! snow globes, and how the thrown Hackl treats them.

use super::*;
use crate::enemies::{Pose, high_c_column, state};
use crate::hackl::AxeHit;
use crate::player::Power;
use crate::{DeathCause, EnemyKind, Entity, EntityKind, Fx, Popup};

/// Legend of the test worlds (`.` air, `P` start).
const LEGEND: &str = r##"
"#" = "solid"
"-" = "oneway"
"B" = "brick"
"~" = "salzach_water"
"o" = { entity = "operettensaenger" }
"s" = { entity = "souvenirstand" }
"q" = { entity = "souvenirstand", period = 90 }
"Q" = { entity = "souvenirstand", phase = 100 }
"T" = { entity = "selfie_tourist", still = true }
"P" = { entity = "player_start" }
"G" = { entity = "goal" }
"##;

/// A 14-row world `width` columns wide: ground in rows 12–13 (feet at y
/// 192), the start at column 2 (row 11) unless `marks` has a `P`, plus
/// `marks` as (row, col, char). The camera starts at y 44.
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
        "[meta]\nid = \"m4\"\ntime_limit = 300\n[legend]\n{LEGEND}\n[map]\ngrid = '''\n{}\n'''\n",
        grid.join("\n")
    );
    Level::parse(&text).unwrap()
}

fn game(level: &Level) -> Game {
    Game::new(level, &tuning(), 1)
}

fn first(g: &Game, kind: EntityKind) -> Entity {
    g.entities()
        .iter()
        .find(|e| e.kind == kind)
        .copied()
        .unwrap_or_else(|| panic!("no {kind:?}"))
}

fn all(g: &Game, kind: EntityKind) -> Vec<Entity> {
    g.entities()
        .iter()
        .filter(|e| e.kind == kind)
        .copied()
        .collect()
}

fn popups(g: &Game) -> Vec<Entity> {
    g.entities()
        .iter()
        .filter(|e| matches!(e.kind, EntityKind::Popup(_)))
        .copied()
        .collect()
}

/// Steps once with the player kept invulnerable.
fn safe_step(g: &mut Game) -> Vec<Event> {
    g.player.invulnerable = u8::MAX;
    g.step(input("-")).to_vec()
}

/// Puts the standing player's hitbox centre at `cx` px (no physics).
fn place_player(g: &mut Game, cx: i32) {
    g.player.x = Fx::from_px(cx) - g.player.w.div_int(2);
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

/// `g` with the player as Hackl at once (no freeze).
fn as_hackl(mut g: Game) -> Game {
    g.player.set_power(Power::Hackl, &tuning().player);
    g.player.transform = 0;
    g
}

const SINGER: EntityKind = EntityKind::Enemy(EnemyKind::Operettensaenger);
const GLOBE: EntityKind = EntityKind::Enemy(EnemyKind::Schneekugel);
const STAND: EntityKind = EntityKind::Souvenirstand;
const SHARD: EntityKind = EntityKind::SchneekugelScherben;

/// A snow globe rolling left with its feet at (`x`, `feet`) px.
fn globe(x: i32, feet: i32) -> Entity {
    let t = tuning();
    let mut g = Entity::at_feet(
        GLOBE,
        Fx::from_px(x),
        Fx::from_px(feet),
        t.enemies.schneekugel_hitbox,
    );
    g.dir = -1;
    g
}

// ---------------------------------------------------------- the singer

/// The singer at (11, 12): feet (200, 192), hitbox 194..206 × 170..192.
fn singer_world(marks: &[(usize, usize, char)]) -> Game {
    let mut m = vec![(11, 12, 'o')];
    m.extend_from_slice(marks);
    game(&world(40, &m))
}

#[test]
fn the_singer_cycle_runs_only_in_range_and_starts_at_zero() {
    let t = tuning();
    let e = &t.enemies;
    let mut g = singer_world(&[]);
    // 161 px away: out of range (< 161 px is in range).
    place_player(&mut g, 200 - 161);
    for _ in 0..50 {
        safe_step(&mut g);
    }
    let s = first(&g, SINGER);
    assert_eq!((s.state, s.count), (state::STAND, 0), "waits, idle");
    assert_eq!(s.pose(&t), Some(Pose::Idle));
    assert_eq!(s.dir, -1, "faces the player");
    place_player(&mut g, 200 - 160);
    let mut states = Vec::new();
    for _ in 0..e.operettensaenger_cycle + 1 {
        safe_step(&mut g);
        states.push(first(&g, SINGER).state);
    }
    // Frame f of the cycle is shown after the (f + 1)-th step in range.
    let inhale = usize::from(e.operettensaenger_inhale);
    let sing = usize::from(e.operettensaenger_sing);
    assert!(states[..inhale].iter().all(|&s| s == state::INHALE), "0-39");
    assert!(
        states[inhale..inhale + sing]
            .iter()
            .all(|&s| s == state::SING),
        "40-87"
    );
    assert!(
        states[inhale + sing..180]
            .iter()
            .all(|&s| s == state::STAND),
        "88-179"
    );
    assert_eq!(states[180], state::INHALE, "the next cycle");
    assert_eq!((inhale, sing, 180 - inhale - sing), (40, 48, 92));
}

#[test]
fn out_of_range_the_singer_pauses_and_keeps_his_column() {
    let t = tuning();
    let mut g = singer_world(&[]);
    place_player(&mut g, 120);
    for _ in 0..45 {
        safe_step(&mut g);
    }
    let s = first(&g, SINGER);
    assert_eq!(s.state, state::SING);
    let column = high_c_column(&s, g.map(), &t).unwrap();
    place_player(&mut g, 200 + 161);
    for _ in 0..60 {
        safe_step(&mut g);
    }
    let paused = first(&g, SINGER);
    assert_eq!((paused.state, paused.count), (s.state, s.count), "paused");
    assert_eq!(paused.dir, 1, "still faces the player");
    assert_eq!(high_c_column(&paused, g.map(), &t), Some(column), "kept");
}

#[test]
fn the_column_grows_8_px_per_frame_to_80_and_is_12_px_wide() {
    let t = tuning();
    let mut g = singer_world(&[]);
    place_player(&mut g, 120);
    for _ in 0..40 {
        safe_step(&mut g);
        assert_eq!(
            high_c_column(&first(&g, SINGER), g.map(), &t),
            None,
            "inhale"
        );
    }
    let mut heights = Vec::new();
    for _ in 0..48 {
        safe_step(&mut g);
        let s = first(&g, SINGER);
        let c = high_c_column(&s, g.map(), &t).unwrap();
        assert_eq!(c.w, Fx::from_px(12));
        assert_eq!(c.x + c.w.div_int(2), s.x + s.w.div_int(2), "centred");
        assert_eq!(c.y + c.h, s.y, "from his hitbox top");
        heights.push(c.h.to_px());
    }
    assert_eq!(&heights[..11], &[8, 16, 24, 32, 40, 48, 56, 64, 72, 80, 80]);
    assert!(heights.iter().skip(9).all(|&h| h == 80));
    safe_step(&mut g);
    assert_eq!(high_c_column(&first(&g, SINGER), g.map(), &t), None, "idle");
}

#[test]
fn the_column_passes_one_way_tiles_and_stops_at_solid() {
    let t = tuning();
    // His top is y 170; row 8 is y 128..144, row 9 y 144..160.
    for (tile, top) in [('-', 90), ('#', 144)] {
        let mut g = singer_world(&[(8, 12, tile)]);
        place_player(&mut g, 120);
        for _ in 0..60 {
            safe_step(&mut g);
        }
        let c = high_c_column(&first(&g, SINGER), g.map(), &t).unwrap();
        assert_eq!(c.y.to_px(), top, "{tile}");
    }
    // Through a one-way tile up to the solid one above it.
    let mut g = singer_world(&[(9, 12, '-'), (7, 12, '#')]);
    place_player(&mut g, 120);
    for _ in 0..60 {
        safe_step(&mut g);
    }
    let c = high_c_column(&first(&g, SINGER), g.map(), &t).unwrap();
    assert_eq!(c.y.to_px(), 128);
}

#[test]
fn the_column_hurts_on_contact_from_the_side() {
    let mut g = singer_world(&[]);
    g.player.set_power(Power::Big, &tuning().player);
    g.player.transform = 0;
    place_player(&mut g, 120);
    for _ in 0..55 {
        g.step(input("-"));
    }
    // Hang in the air next to the column, then touch it.
    let c = high_c_column(&first(&g, SINGER), g.map(), &tuning()).unwrap();
    g.player.x = c.x - g.player.w + Fx::ONE;
    g.player.y = c.y;
    g.player.vy = Fx::ZERO;
    g.player.on_ground = false;
    let events = g.step(input("-")).to_vec();
    assert!(events.contains(&Event::Hurt), "{events:?}");
    assert_eq!(g.player().power, Power::Normal);
    assert_eq!(first(&g, SINGER).kind, SINGER, "not stomped");
}

#[test]
fn falling_onto_a_singing_singer_hurts_instead_of_stomping() {
    let mut g = singer_world(&[]);
    place_player(&mut g, 120);
    for _ in 0..42 {
        safe_step(&mut g);
    }
    let s = first(&g, SINGER);
    assert_eq!(s.state, state::SING, "column 24 px high");
    drop_onto(&mut g, s);
    g.player.invulnerable = 0;
    let events = g.step(input("-")).to_vec();
    assert!(!events.contains(&Event::Stomped), "{events:?}");
    assert_eq!(g.death_cause(), Some(DeathCause::Hit), "Normal dies");
    assert_eq!(g.score(), 0);
}

#[test]
fn a_stomp_in_the_inhale_cancels_the_c_for_200_and_bravo() {
    let t = tuning();
    assert_eq!(EnemyKind::Operettensaenger.chain_start(), 1);
    for frames in [10, 120] {
        let mut g = singer_world(&[]);
        place_player(&mut g, 120);
        for _ in 0..frames {
            safe_step(&mut g);
        }
        let s = first(&g, SINGER);
        assert_ne!(s.state, state::SING, "{frames}");
        drop_onto(&mut g, s);
        let events = g.step(input("-")).to_vec();
        assert!(events.contains(&Event::Stomped), "{frames}: {events:?}");
        assert!(!events.contains(&Event::Hurt));
        assert_eq!(g.score(), 200, "chain start 200");
        assert_eq!(all(&g, SINGER).len(), 0);
        let d = first(&g, EntityKind::Defeated(EnemyKind::Operettensaenger));
        assert_eq!(high_c_column(&d, g.map(), &t), None, "no C");
        let p = popups(&g);
        let score = p
            .iter()
            .find(|e| e.kind == EntityKind::Popup(Popup::Score(200)))
            .unwrap();
        let bravo = p
            .iter()
            .find(|e| e.kind == EntityKind::Popup(Popup::Bravo))
            .unwrap();
        assert_eq!(bravo.x, score.x, "centred together");
        assert_eq!(
            score.y - bravo.y,
            Fx::from_px(crate::entity::POPUP_STACK_PX),
            "„Bravo!“ above the score"
        );
    }
}

#[test]
fn a_singer_continues_a_stomp_chain() {
    let mut g = singer_world(&[]);
    place_player(&mut g, 120);
    safe_step(&mut g);
    g.player.stomp_chain = 2;
    let target = first(&g, SINGER);
    drop_onto(&mut g, target);
    g.step(input("-"));
    assert_eq!(g.score(), 400, "the chain's third stomp");
}

#[test]
fn a_bumped_block_knocks_the_singer_off_with_bravo() {
    // A brick at (9, 12) with the singer on it (feet y 144); the player
    // below jumps into the brick.
    let mut g = game(&world(40, &[(9, 12, 'B'), (8, 12, 'o'), (11, 12, 'P')]));
    let events = until(&mut g, "J", 60, |g| {
        g.entities()
            .iter()
            .any(|e| e.kind == EntityKind::Defeated(EnemyKind::Operettensaenger))
    });
    assert!(events.contains(&Event::BlockBumped));
    let d = first(&g, EntityKind::Defeated(EnemyKind::Operettensaenger));
    assert_eq!(d.state, state::TUMBLE);
    assert_eq!(g.score(), tuning().score.bump_enemy);
    assert!(
        popups(&g)
            .iter()
            .any(|e| e.kind == EntityKind::Popup(Popup::Bravo))
    );
}

#[test]
fn an_axe_defeats_the_singer_for_200_and_bravo() {
    let mut g = as_hackl(singer_world(&[]));
    g.entities.retain(|e| e.kind != SINGER);
    // About 6 tiles ahead, where the arc is back at hand height.
    let mut s = Entity::at_feet(SINGER, Fx::from_px(150), Fx::from_px(192), [12, 22]);
    s.dir = -1;
    g.entities.push(s);
    let mut events = hold(&mut g, "B", 1);
    events.extend(hold(&mut g, "-", 40));
    assert!(events.contains(&Event::AxeHit), "{events:?}");
    let d = first(&g, EntityKind::Defeated(EnemyKind::Operettensaenger));
    assert_eq!(d.state, state::TUMBLE);
    assert_eq!(g.score(), 200);
    assert_eq!(g.player().stomp_chain, 0, "outside the chain");
    assert!(
        popups(&g)
            .iter()
            .any(|e| e.kind == EntityKind::Popup(Popup::Bravo))
    );
}

#[test]
fn an_axe_flies_through_the_column() {
    let t = tuning();
    let mut g = as_hackl(singer_world(&[]));
    g.entities.retain(|e| e.kind != SINGER);
    // A singer at x 96 in his full C: the axe's arc passes over his head
    // through the column.
    let mut s = Entity::at_feet(SINGER, Fx::from_px(96), Fx::from_px(192), [12, 22]);
    s.dir = -1;
    s.state = state::SING;
    s.count = 50;
    g.entities.push(s);
    let mut events = hold(&mut g, "B", 1);
    let mut crossed = false;
    for _ in 0..30 {
        events.extend(hold(&mut g, "-", 1));
        let c = high_c_column(&first(&g, SINGER), g.map(), &t).unwrap();
        let axe = first(&g, EntityKind::ThrownHackl);
        crossed |= axe.hitbox().overlaps(&c);
    }
    assert!(crossed, "the axe went through the column");
    assert!(!events.contains(&Event::AxeHit), "{events:?}");
    assert_eq!(first(&g, SINGER).kind, SINGER);
    assert_eq!(g.score(), 0);
}

// ------------------------------------------------------ the Souvenirstand

/// A stand at (11, 15): its cell is x 240..256 (centre 248), the counter
/// at columns 16–18 (row 11).
fn stand_world(stand: char, marks: &[(usize, usize, char)]) -> Game {
    let mut m = vec![(11, 15, stand), (11, 16, '#'), (11, 17, '#'), (11, 18, '#')];
    m.extend_from_slice(marks);
    game(&world(40, &m))
}

#[test]
fn the_stand_counts_only_while_the_player_is_left_of_it_and_in_range() {
    let t = tuning();
    let mut g = stand_world('s', &[]);
    // Range 8 tiles (128 px) by default.
    place_player(&mut g, 248 - 129);
    for _ in 0..20 {
        safe_step(&mut g);
    }
    assert_eq!(first(&g, STAND).state, state::WAITING, "out of range");
    place_player(&mut g, 248 - 128);
    safe_step(&mut g);
    let s = first(&g, STAND);
    assert_eq!(
        (s.state, s.count),
        (state::COUNTING, 0),
        "starts at phase 0"
    );
    assert_eq!(s.pose(&t), Some(Pose::Telegraph));
    safe_step(&mut g);
    assert_eq!(first(&g, STAND).count, 1);
    // Right of the stand (on the counter, in range): paused, pose kept.
    place_player(&mut g, 264);
    for _ in 0..40 {
        safe_step(&mut g);
    }
    let s = first(&g, STAND);
    assert_eq!(s.count, 1, "paused right of it");
    assert_eq!(s.pose(&t), Some(Pose::Telegraph));
    assert!(all(&g, GLOBE).is_empty());
    place_player(&mut g, 248);
    safe_step(&mut g);
    assert_eq!(first(&g, STAND).count, 1, "paused at its centre");
    place_player(&mut g, 247);
    safe_step(&mut g);
    assert_eq!(first(&g, STAND).count, 2, "left of it again");
}

#[test]
fn the_stand_counts_only_on_screen() {
    // The camera starts at y 44: a stand at row 1 (on a ledge) is above
    // the screen, one at row 3 (y 48..64) is on it.
    for (row, counts) in [(1, false), (3, true)] {
        let mut g = game(&world(
            40,
            &[(row, 15, 's'), (row + 1, 15, '#'), (11, 9, 'P')],
        ));
        assert_eq!(g.camera().y, 44);
        for _ in 0..10 {
            safe_step(&mut g);
        }
        assert_eq!(
            first(&g, STAND).state == state::COUNTING,
            counts,
            "row {row}"
        );
    }
}

#[test]
fn telegraph_30_frames_then_a_globe_spawns_in_the_stand_cell() {
    let t = tuning();
    let mut g = stand_world('s', &[(11, 9, 'P')]);
    for k in 0..30u16 {
        safe_step(&mut g);
        let s = first(&g, STAND);
        assert_eq!(s.count, k);
        assert_eq!(s.pose(&t), Some(Pose::Telegraph), "frame {k}");
        assert!(all(&g, GLOBE).is_empty(), "frame {k}");
    }
    safe_step(&mut g);
    assert_eq!(first(&g, STAND).pose(&t), Some(Pose::Idle), "frame 30");
    let globes = all(&g, GLOBE);
    assert_eq!(globes.len(), 1, "spawned at frame 30");
    let gl = globes[0];
    assert_eq!(
        gl.feet(),
        (Fx::from_px(248), Fx::from_px(192)),
        "cell bottom centre"
    );
    assert_eq!((gl.w, gl.h), (Fx::from_px(10), Fx::from_px(10)));
    assert_eq!(gl.pose(&t), Some(Pose::Roll));
    assert!(gl.facing_left());
    // The next globe follows a period (150) later.
    for _ in 0..149 {
        safe_step(&mut g);
    }
    assert_eq!(all(&g, GLOBE).len(), 1);
    safe_step(&mut g);
    assert_eq!(all(&g, GLOBE).len(), 2);
}

#[test]
fn with_two_globes_alive_a_cycle_shows_nothing_and_spawns_nothing() {
    let t = tuning();
    // Period 90: globes at frames 30 and 120 (counter frames); both are
    // still rolling (to the level's left wall) when the third cycle starts.
    let mut g = stand_world('q', &[(11, 9, 'P')]);
    for _ in 0..181 {
        safe_step(&mut g);
    }
    assert_eq!(all(&g, GLOBE).len(), 2);
    let s = first(&g, STAND);
    assert_eq!((s.state, s.count), (state::SKIPPING, 0));
    for _ in 0..40 {
        assert_eq!(first(&g, STAND).pose(&t), Some(Pose::Idle), "no telegraph");
        safe_step(&mut g);
    }
    assert_eq!(all(&g, GLOBE).len(), 2, "no third globe");
    // Once one is gone, the next cycle telegraphs and spawns again.
    g.entities
        .retain(|e| e.kind != GLOBE || e.x > Fx::from_px(100));
    assert_eq!(all(&g, GLOBE).len(), 1);
    until(&mut g, "-", 120, |g| first(g, STAND).count == 0);
    let s = first(&g, STAND);
    assert_eq!(
        (s.state, s.pose(&t)),
        (state::COUNTING, Some(Pose::Telegraph))
    );
}

#[test]
fn a_stand_starts_at_its_phase() {
    let mut g = stand_world('Q', &[(11, 9, 'P')]);
    safe_step(&mut g);
    assert_eq!(first(&g, STAND).count, 100);
    // Counter 100..149, then the telegraph from 0: the globe at 30.
    for _ in 0..79 {
        safe_step(&mut g);
        assert!(all(&g, GLOBE).is_empty());
    }
    safe_step(&mut g);
    assert_eq!(all(&g, GLOBE).len(), 1, "50 + 30 frames after the start");
}

#[test]
fn the_stand_is_harmless_and_lets_axes_pass() {
    let mut g = stand_world('s', &[]);
    safe_step(&mut g);
    let s = first(&g, STAND);
    assert_eq!(s.axe_hit(), AxeHit::Pass);
    // The player standing in its cell is not hurt.
    g.player.invulnerable = 0;
    place_player(&mut g, 248);
    let events = hold(&mut g, "-", 5);
    assert!(!events.contains(&Event::Hurt));
    assert_eq!(g.death_cause(), None);
}

// ------------------------------------------------------------ the globe

#[test]
fn a_globe_rolls_left_at_1_px_and_falls_off_ledges() {
    let t = tuning();
    // A 2-tile ledge (rows 10-11) at columns 8-12; the globe on it at 190.
    let marks: Vec<_> = (8..=12)
        .flat_map(|c| [(10, c, '#'), (11, c, '#')])
        .collect();
    let mut g = game(&world(40, &marks));
    g.entities.push(globe(190, 160));
    safe_step(&mut g);
    let a = first(&g, GLOBE);
    safe_step(&mut g);
    let b = first(&g, GLOBE);
    assert_eq!(a.x - b.x, t.enemies.schneekugel_speed);
    assert_eq!(t.enemies.schneekugel_speed, Fx::from_px(1), "1.0 px/frame");
    assert_eq!(b.feet().1, Fx::from_px(160), "rolls on the ledge");
    until(&mut g, "-", 120, |g| {
        first(g, GLOBE).feet().1 == Fx::from_px(192)
    });
    let landed = first(&g, GLOBE);
    assert!(landed.x < Fx::from_px(128), "off the ledge's left end");
    safe_step(&mut g);
    assert_eq!(first(&g, GLOBE).x, landed.x - Fx::from_px(1), "rolls on");
}

#[test]
fn a_globe_shatters_on_a_wall_into_shards() {
    let mut g = game(&world(40, &[(11, 5, '#'), (10, 5, '#')]));
    g.entities.push(globe(150, 192));
    let events = until(&mut g, "-", 80, |g| all(g, GLOBE).is_empty());
    assert!(!events.contains(&Event::Hurt));
    let shards = all(&g, SHARD);
    assert_eq!(shards.len(), 4, "schneekugel_scherben");
    assert!(shards.iter().all(|s| s.x >= Fx::from_px(90)), "at the wall");
    assert_eq!(g.score(), 0);
    // Shards are cosmetic: no harm, and they fall away.
    for _ in 0..120 {
        safe_step(&mut g);
    }
    assert!(all(&g, SHARD).is_empty());
}

#[test]
fn a_globe_is_lost_in_water_without_shards() {
    let marks: Vec<_> = (5..=6).flat_map(|c| [(12, c, '~'), (13, c, '~')]).collect();
    let mut g = game(&world(40, &marks));
    g.entities.push(globe(150, 192));
    until(&mut g, "-", 120, |g| all(g, GLOBE).is_empty());
    assert!(all(&g, SHARD).is_empty());
}

#[test]
fn a_globe_is_removed_once_it_leaves_the_screen_on_the_left() {
    let mut g = game(&world(80, &[]));
    place_player(&mut g, 600);
    for _ in 0..5 {
        safe_step(&mut g);
    }
    let left = g.camera().x;
    assert!(left > 200);
    g.entities.push(globe(left + 8, 192));
    for _ in 0..2 {
        safe_step(&mut g);
    }
    assert_eq!(all(&g, GLOBE).len(), 1);
    until(&mut g, "-", 30, |g| all(g, GLOBE).is_empty());
    assert_eq!(g.camera().x, left);
    assert!(all(&g, SHARD).is_empty());
}

#[test]
fn a_globe_passes_enemies_and_lands_on_one_way_tops() {
    let mut g = game(&world(40, &[(11, 6, 'T'), (8, 3, '-'), (8, 4, '-')]));
    g.entities.push(globe(150, 192));
    for _ in 0..80 {
        safe_step(&mut g);
    }
    let gl = first(&g, GLOBE);
    assert!(gl.x < Fx::from_px(90), "through the tourist");
    assert_eq!(
        first(&g, EntityKind::Enemy(EnemyKind::SelfieTourist)).dir,
        0
    );
    // A globe falling onto a one-way platform stays on it.
    let mut g = game(&world(40, &[(8, 6, '-'), (8, 7, '-'), (8, 8, '-')]));
    g.entities.push(globe(120, 100));
    for _ in 0..20 {
        safe_step(&mut g);
    }
    assert_eq!(
        first(&g, GLOBE).feet().1,
        Fx::from_px(128),
        "on the one-way top"
    );
}

#[test]
fn a_globe_hurts_from_the_side() {
    let mut g = game(&world(40, &[]));
    g.entities.push(globe(80, 192));
    g.player.invulnerable = 0;
    until(&mut g, "-", 60, |g| g.death_cause().is_some());
    assert_eq!(g.death_cause(), Some(DeathCause::Hit));
}

#[test]
fn a_stomped_globe_shatters_for_100_in_the_chain() {
    let mut g = game(&world(40, &[]));
    g.entities.push(globe(150, 192));
    safe_step(&mut g);
    let target = first(&g, GLOBE);
    drop_onto(&mut g, target);
    let events = g.step(input("-")).to_vec();
    assert!(events.contains(&Event::Stomped), "{events:?}");
    assert!(!events.contains(&Event::Hurt));
    assert_eq!(g.score(), 100);
    assert!(all(&g, GLOBE).is_empty());
    assert!(all(&g, EntityKind::Defeated(EnemyKind::Schneekugel)).is_empty());
    assert_eq!(all(&g, SHARD).len(), 4, "shatters");
    assert!(g.player().vy < Fx::ZERO, "bounces");
    assert_eq!(g.player().stomp_chain, 1);
    // Inside a chain it pays the chain's next value.
    let mut g = game(&world(40, &[]));
    g.entities.push(globe(150, 192));
    safe_step(&mut g);
    g.player.stomp_chain = 1;
    let target = first(&g, GLOBE);
    drop_onto(&mut g, target);
    g.step(input("-"));
    assert_eq!(g.score(), 200);
}

#[test]
fn an_axe_shatters_a_globe_for_100() {
    let mut g = as_hackl(game(&world(40, &[])));
    g.entities.push(globe(185, 192));
    let mut events = hold(&mut g, "B", 1);
    events.extend(hold(&mut g, "-", 50));
    assert!(events.contains(&Event::AxeHit), "{events:?}");
    assert!(all(&g, GLOBE).is_empty());
    assert_eq!(all(&g, SHARD).len(), 4, "drawn as a shatter");
    assert_eq!(g.score(), 100);
    assert_eq!(g.player().stomp_chain, 0, "outside the chain");
    assert_eq!(EnemyKind::Schneekugel.axe_hit(), AxeHit::Defeat);
    assert_eq!(EnemyKind::Operettensaenger.axe_hit(), AxeHit::Defeat);
}

#[test]
fn stands_and_globes_reset_with_the_level() {
    let mut g = stand_world('s', &[(11, 9, 'P')]);
    for _ in 0..40 {
        safe_step(&mut g);
    }
    assert_eq!(all(&g, GLOBE).len(), 1);
    // Dying respawns everything as placed.
    g.player.invulnerable = 0;
    g.player.y = Fx::from_px(400);
    until(&mut g, "-", 200, |g| {
        g.mode() == crate::Mode::Playing && g.deaths() == 1
    });
    assert!(all(&g, GLOBE).is_empty());
    let s = first(&g, STAND);
    assert_eq!(s.state, state::WAITING);
}
