//! Items, blocks, power states, scoring, lives, timer, checkpoint, level
//! reset and game over (M1 task E1).

use super::*;
use crate::{
    Block, Contents, DeathCause, EnemyKind, EntityKind, Fx, Mode, Popup, Power, SpawnKind, TileKind,
};

/// Legend of the test worlds (`.` air, `P` start, `G` goal).
const LEGEND: &str = r##"
"#" = "solid"
"^" = "hazard"
"-" = "oneway"
"B" = "brick"
"?" = { tile = "oblaten_box" }
"M" = { tile = "oblaten_box", contains = "mozartkugel" }
"Z" = { tile = "oblaten_box", contains = "zaunerstollen" }
"T" = { tile = "brick", contains = "oblaten_10" }
"o" = { entity = "oblate" }
"C" = { entity = "checkpoint" }
"t" = { entity = "selfie_tourist" }
"v" = { entity = "mozartkugel_verkaeufer" }
"f" = { entity = "fiaker" }
"s" = { entity = "sign", text = "stage.salzburg.sign_bus", style = "bus" }
"P" = { entity = "player_start" }
"G" = { entity = "goal" }
"##;

/// A 14-row world `width` columns wide: ground in rows 12–13, the player
/// start at column 2 (row 11) unless `marks` places a `P`, plus `marks`
/// as (row, col, char).
fn world_with(width: usize, time_limit: u32, marks: &[(usize, usize, char)]) -> Level {
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
        "[meta]\nid = \"items\"\ntime_limit = {time_limit}\n[legend]\n{LEGEND}\n[map]\ngrid = '''\n{}\n'''\n",
        grid.join("\n")
    );
    Level::parse(&text).unwrap()
}

fn world(width: usize, marks: &[(usize, usize, char)]) -> Level {
    world_with(width, 300, marks)
}

fn game(level: &Level) -> Game {
    Game::new(level, &tuning(), 1)
}

fn count(g: &Game, kind: EntityKind) -> usize {
    g.entities().iter().filter(|e| e.kind == kind).count()
}

fn feet_y(g: &Game) -> Fx {
    g.player().y + g.player().h
}

// ----------------------------------------------------------------- boxes

#[test]
fn box_releases_its_oblate_once_and_turns_empty() {
    let mut g = game(&world(20, &[(9, 2, '?')]));
    let empty = g.level().empty_box_name();
    assert_ne!(empty, 0);
    let events = hold(&mut g, "J", 20);
    assert!(events.contains(&Event::BlockBumped), "{events:?}");
    assert!(events.contains(&Event::OblateCollected));
    assert_eq!((g.oblaten(), g.score()), (1, 200));
    let t = g.tile(2, 9).unwrap();
    assert_eq!(
        (t.kind, t.block, t.name),
        (TileKind::Solid, Block::None, empty)
    );
    assert_eq!(
        g.level().tile(2, 9).unwrap().block,
        Block::Box,
        "level unchanged"
    );
    // The popped Oblate disappears and leaves a 200 popup.
    hold(&mut g, "-", 60);
    assert_eq!(count(&g, EntityKind::PoppedOblate), 0);
    // A second hit does nothing.
    let events = hold(&mut g, "J", 20);
    assert!(!events.contains(&Event::BlockBumped), "{events:?}");
    assert_eq!(g.oblaten(), 1);
}

#[test]
fn bump_raises_the_tile_4_px_over_8_frames() {
    let mut g = game(&world(20, &[(9, 2, 'B')]));
    until(&mut g, "J", 30, |g| {
        g.events().contains(&Event::BlockBumped)
    });
    let mut offsets = vec![g.tile_bump_px(2, 9)];
    for _ in 0..8 {
        g.step(input("-"));
        offsets.push(g.tile_bump_px(2, 9));
    }
    assert_eq!(offsets, vec![1, 2, 3, 4, 3, 2, 1, 0, 0]);
    assert_eq!(count(&g, EntityKind::Bump), 0, "bump entity gone");
    assert_eq!(
        g.tile(2, 9).unwrap().block,
        Block::Brick,
        "Normal does not break"
    );
    assert_eq!(g.score(), 0);
}

#[test]
fn only_the_block_nearest_the_centre_is_hit() {
    // Player x 34..46 (centre 40, column 2); bricks in columns 1 and 2.
    let mut g = game(&world(20, &[(9, 1, '?'), (9, 2, '?')]));
    hold(&mut g, "J", 20);
    assert_eq!(
        g.tile(2, 9).unwrap().block,
        Block::None,
        "centre column hit"
    );
    assert_eq!(
        g.tile(1, 9).unwrap().block,
        Block::Box,
        "neighbour untouched"
    );
}

#[test]
fn zuckerschock_breaks_bricks_into_four_pieces() {
    let mut g = game(&world(20, &[(9, 2, 'B')]));
    g.player.set_power(Power::Big, &tuning().player);
    g.player.transform = 0;
    let events = until(&mut g, "J", 30, |g| {
        g.events().contains(&Event::BrickBroken)
    });
    assert!(
        !events.contains(&Event::BlockBumped),
        "breaking is not a bump"
    );
    assert_eq!(g.tile(2, 9).unwrap().kind, TileKind::Empty);
    assert_eq!(g.score(), 50);
    assert_eq!(count(&g, EntityKind::Debris), 4);
    let debris: Vec<_> = g
        .entities()
        .iter()
        .filter(|e| e.kind == EntityKind::Debris)
        .map(|e| (e.vx.signum(), e.vy))
        .collect();
    // Spawned and moved once in the same frame (gravity applied once).
    let (it, g1) = (tuning().items, tuning().player.gravity);
    assert_eq!(
        debris,
        vec![
            (-1, g1 - it.debris_speed_high),
            (1, g1 - it.debris_speed_high),
            (-1, g1 - it.debris_speed_low),
            (1, g1 - it.debris_speed_low),
        ]
    );
    // The player passes through the hole; the debris falls out of the level.
    until(&mut g, "-", 60, |g| g.player().on_ground);
    let mut highest = feet_y(&g);
    for _ in 0..40 {
        g.step(input("J"));
        highest = highest.min(feet_y(&g));
    }
    assert!(highest < Fx::from_px(9 * 16), "jumped up through the hole");
    hold(&mut g, "-", 120);
    assert_eq!(count(&g, EntityKind::Debris), 0);
}

#[test]
fn oblaten_10_brick_pays_up_to_ten_then_empties() {
    let mut g = game(&world(20, &[(9, 2, 'T')]));
    for n in 1..=10u8 {
        until(&mut g, "J", 40, |g| {
            g.events().contains(&Event::BlockBumped)
        });
        assert_eq!(g.oblaten(), n);
        until(&mut g, "-", 80, |g| g.player().on_ground);
        // Quick re-hits stay inside the 240-frame window.
    }
    assert_eq!(g.tile(2, 9).unwrap().block, Block::None, "empty after 10");
    let events = hold(&mut g, "J", 30);
    assert!(!events.contains(&Event::OblateCollected));
}

#[test]
fn oblaten_10_brick_stops_after_its_time_window() {
    let mut g = game(&world(20, &[(9, 2, 'T')]));
    until(&mut g, "J", 40, |g| {
        g.events().contains(&Event::BlockBumped)
    });
    hold(&mut g, "-", tuning().items.multi_frames as u32);
    // The first hit after the window pays the last Oblate.
    until(&mut g, "J", 40, |g| {
        g.events().contains(&Event::BlockBumped)
    });
    assert_eq!(g.oblaten(), 2);
    assert_eq!(g.tile(2, 9).unwrap().contains, Contents::None);
}

// -------------------------------------------------------------- power-up

/// A Mozartkugel box above the start, a wall at column 8.
fn mozart_world() -> Level {
    world(20, &[(9, 2, 'M'), (10, 8, '#'), (11, 8, '#')])
}

#[test]
fn mozartkugel_rises_rolls_and_turns_at_walls() {
    let mut g = game(&mozart_world());
    until(&mut g, "J", 30, |g| {
        g.events().contains(&Event::BlockBumped)
    });
    let ball = |g: &Game| {
        *g.entities()
            .iter()
            .find(|e| e.kind == EntityKind::Mozartkugel)
            .unwrap()
    };
    let start = ball(&g);
    // Spawned inside the box, already one frame into its rise.
    let rise = tuning().items.rise_speed;
    assert_eq!(
        start.feet().1,
        Fx::from_px(10 * 16) - rise,
        "inside the box"
    );
    assert!(start.rising(g.tuning()));
    // Walk away left so the player does not touch it.
    hold(&mut g, "L", 31);
    let risen = ball(&g);
    assert_eq!(
        risen.feet().1,
        Fx::from_px(9 * 16),
        "rose 16 px in 32 frames"
    );
    assert_eq!(risen.x, start.x);
    assert_eq!(risen.vx, tuning().items.mozartkugel_speed);
    g.step(input("-"));
    assert_eq!(ball(&g).x, risen.x + Fx::ONE, "rolls right at 1 px/frame");
    // Falls off the box, rolls to the wall and comes back.
    let mut turned = false;
    for _ in 0..200 {
        g.step(input("-"));
        if ball(&g).vx < Fx::ZERO {
            turned = true;
            break;
        }
    }
    assert!(turned, "turned at the wall");
    assert_eq!(ball(&g).right(), Fx::from_px(8 * 16));
    assert_eq!(ball(&g).feet().1, Fx::from_px(12 * 16), "on the ground");
}

#[test]
fn eating_the_mozartkugel_grows_and_freezes_30_frames() {
    let t = tuning();
    let mut g = game(&mozart_world());
    until(&mut g, "J", 30, |g| {
        g.events().contains(&Event::BlockBumped)
    });
    // Wait on the ground until the rolling ball comes back to the player.
    let events = until(&mut g, "-", 400, |g| g.events().contains(&Event::PowerUp));
    assert!(events.contains(&Event::PowerUp));
    let p = *g.player();
    assert_eq!(p.power, Power::Big);
    assert_eq!(p.h, Fx::from_px(t.player.hitbox_h_big));
    assert_eq!(p.w, Fx::from_px(t.player.hitbox_w));
    assert_eq!(feet_y(&g), Fx::from_px(12 * 16), "feet stay in place");
    assert_eq!(g.score(), 1000);
    assert!(
        g.entities()
            .iter()
            .any(|e| e.kind == EntityKind::Popup(Popup::Score(1000)))
    );
    // Frozen: nothing moves, the timer stands still.
    let (time, before) = (g.time_left(), g.clone());
    for _ in 0..t.player.transform_frames {
        assert!(g.step(input("RB")).is_empty());
        assert_eq!(g.player().x, before.player().x);
        assert_eq!(g.entities(), before.entities());
    }
    assert_eq!(g.player().transform, 0);
    g.step(input("RB"));
    assert_ne!(
        g.player().x,
        before.player().x,
        "moves again after 30 frames"
    );
    assert_eq!(g.time_left(), time);
}

#[test]
fn mozartkugel_is_lost_in_water() {
    let mut g = game(&world(20, &[(9, 2, 'M'), (12, 6, '^'), (13, 6, '^')]));
    until(&mut g, "J", 30, |g| {
        g.events().contains(&Event::BlockBumped)
    });
    hold(&mut g, "L", 40);
    until(&mut g, "-", 200, |g| count(g, EntityKind::Mozartkugel) == 0);
    assert_eq!(g.player().power, Power::Normal);
}

#[test]
fn power_up_when_big_only_scores() {
    let mut g = game(&mozart_world());
    g.player.set_power(Power::Big, &tuning().player);
    g.player.transform = 0;
    until(&mut g, "J", 30, |g| {
        g.events().contains(&Event::BlockBumped)
    });
    until(&mut g, "-", 400, |g| count(g, EntityKind::Mozartkugel) == 0);
    assert_eq!(g.score(), 1000);
    assert_eq!(g.player().transform, 0, "no freeze");
}

#[test]
fn zaunerstollen_gives_a_life() {
    let mut g = game(&world(20, &[(9, 2, 'Z'), (10, 8, '#'), (11, 8, '#')]));
    until(&mut g, "J", 30, |g| {
        g.events().contains(&Event::BlockBumped)
    });
    let events = until(&mut g, "-", 400, |g| g.events().contains(&Event::ExtraLife));
    assert!(!events.contains(&Event::PowerUp));
    assert_eq!(g.lives(), 4);
    assert_eq!(g.player().power, Power::Normal);
    assert!(
        g.entities()
            .iter()
            .any(|e| e.kind == EntityKind::Popup(Popup::ExtraLife))
    );
}

// ------------------------------------------------- hurt, stomp, enemies

#[test]
fn enemy_kills_a_normal_player_from_the_side() {
    let mut g = game(&world(20, &[(11, 6, 't')]));
    until(&mut g, "R", 120, |g| g.mode() != Mode::Playing);
    assert_eq!(g.death_cause(), Some(DeathCause::Hit));
    assert_eq!(g.lives(), 2);
}

#[test]
fn hurt_shrinks_with_120_invulnerable_frames() {
    let t = tuning();
    // Verkäufer stand still (walking tourists would leave the screen).
    let mut g = game(&world(30, &[(11, 6, 'v'), (11, 12, 'v')]));
    g.player.set_power(Power::Big, &t.player);
    g.player.transform = 0;
    let events = until(&mut g, "R", 120, |g| g.events().contains(&Event::Hurt));
    assert!(!events.contains(&Event::Died));
    assert_eq!(g.player().power, Power::Normal);
    assert_eq!(g.player().h, Fx::from_px(t.player.hitbox_h));
    assert_eq!(feet_y(&g), Fx::from_px(12 * 16), "feet stay in place");
    assert_eq!(g.player().invulnerable, t.player.invulnerable_frames);
    // Shrinking freezes too, then 120 frames without harm: walk through
    // both enemies.
    hold(&mut g, "-", t.player.transform_frames as u32);
    let mut frames = 0;
    while g.player().invulnerable > 0 {
        assert!(!g.step(input("R")).contains(&Event::Died));
        frames += 1;
    }
    assert_eq!(frames, 120);
    assert_eq!(g.mode(), Mode::Playing);
    // Afterwards the next touch kills.
    let mut g2 = g.clone();
    until(&mut g2, "L", 200, |g| g.mode() != Mode::Playing);
}

#[test]
fn hazards_kill_even_when_invulnerable() {
    let mut g = game(&world(20, &[(12, 5, '^'), (13, 5, '^')]));
    g.player.invulnerable = 120;
    until(&mut g, "R", 120, |g| g.mode() != Mode::Playing);
    assert_eq!(g.death_cause(), Some(DeathCause::Hazard));
}

/// The player falls from row 9 (a short fall, so a walking enemy is
/// still below) onto an enemy at column 2.
fn stomp_world(enemy: char) -> Level {
    world(20, &[(9, 2, 'P'), (11, 2, enemy)])
}

/// Falls onto the enemy holding `token`; returns the game at the stomp.
fn stomped(enemy: char, token: &str) -> Game {
    let mut g = game(&stomp_world(enemy));
    until(&mut g, token, 120, |g| g.events().contains(&Event::Stomped));
    g
}

#[test]
fn stomp_defeats_and_scores_100() {
    let g = stomped('t', "-");
    assert_eq!(count(&g, EntityKind::Enemy(EnemyKind::SelfieTourist)), 0);
    assert_eq!(
        count(&g, EntityKind::Defeated(EnemyKind::SelfieTourist)),
        1,
        "squished for a while"
    );
    assert_eq!(g.score(), 100);
    assert!(g.player().vy < Fx::ZERO, "bounces");
    assert_eq!(g.player().stomp_chain, 1);
}

#[test]
fn verkaeufer_chain_starts_at_200() {
    let g = stomped('v', "-");
    assert_eq!(g.score(), 200);
    assert_eq!(g.player().stomp_chain, 2);
}

#[test]
fn fiaker_bounces_without_damage_or_points() {
    let g = stomped('f', "-");
    assert_eq!(count(&g, EntityKind::Enemy(EnemyKind::Fiaker)), 1);
    assert_eq!(g.score(), 0);
    assert!(g.player().vy < Fx::ZERO);
    assert_eq!(g.mode(), Mode::Playing);
}

#[test]
fn stomp_chain_scores_double_then_lives() {
    let t = tuning();
    let mut g = game(&world(20, &[(11, 5, 't')]));
    for i in 0..10u8 {
        g.player.stomp_chain = i;
        let before = (g.score(), g.lives());
        let e = crate::Entity::at_feet(
            EntityKind::Enemy(EnemyKind::SelfieTourist),
            Fx::ZERO,
            Fx::ZERO,
            [12, 20],
        );
        let p = g
            .stomp_reward(EnemyKind::SelfieTourist, e.x, e.y)
            .expect("a popup below the lives cap");
        match t.score.stomp_chain.get(usize::from(i)) {
            Some(&points) => {
                assert_eq!(g.score(), before.0 + points);
                assert_eq!(p.kind, EntityKind::Popup(Popup::Score(points)));
            }
            None => {
                assert_eq!(g.lives(), before.1 + 1, "stomp {i}");
                assert_eq!(p.kind, EntityKind::Popup(Popup::ExtraLife));
            }
        }
    }
    assert_eq!(g.score(), 100 + 200 + 400 + 800 + 1000 + 2000 + 4000 + 8000);
    // At the lives cap a chain life gives nothing: no event, no popup.
    g.lives = t.game.lives_max;
    let seen = g.events().len();
    assert!(
        g.stomp_reward(EnemyKind::SelfieTourist, Fx::ZERO, Fx::ZERO)
            .is_none()
    );
    assert_eq!(g.lives(), t.game.lives_max);
    assert!(!g.events()[seen..].contains(&Event::ExtraLife));
}

/// Falls (from row 9, 8 frames) onto two Selfie-Tourists starting in
/// columns 2 and 3; they walk 4 px left meanwhile, so the player's hitbox
/// (x 38..50) overlaps both (x 30..42 and 46..58).
#[test]
fn two_enemies_stomped_in_the_same_frame() {
    let t = tuning();
    let mut g = game(&world(20, &[(9, 2, 'P'), (11, 2, 't'), (11, 3, 't')]));
    g.player.x = Fx::from_px(38);
    let tourists = |g: &Game| count(g, EntityKind::Enemy(EnemyKind::SelfieTourist));
    assert_eq!(tourists(&g), 2);
    let events = until(&mut g, "-", 120, |g| g.events().contains(&Event::Stomped));
    assert!(!events.contains(&Event::Hurt) && !events.contains(&Event::Died));
    assert_eq!(tourists(&g), 0, "both stomped");
    assert_eq!(g.score(), 100 + 200, "chain: 100, then 200");
    assert_eq!(g.player().stomp_chain, 2);
    // One bounce and one `Stomped` event per frame, however many enemies.
    let stomped = g.events().iter().filter(|e| **e == Event::Stomped).count();
    assert_eq!(stomped, 1);
    assert_eq!(g.player().vy, -t.player.stomp_bounce);
    let popups: Vec<EntityKind> = g
        .entities()
        .iter()
        .map(|e| e.kind)
        .filter(|k| matches!(k, EntityKind::Popup(_)))
        .collect();
    assert_eq!(
        popups,
        vec![
            EntityKind::Popup(Popup::Score(100)),
            EntityKind::Popup(Popup::Score(200))
        ]
    );
}

/// A Selfie-Tourist on the ground (hitbox y 172..192, middle 182) and a
/// player falling onto it with its feet at `feet` on the previous frame.
fn fall_onto_tourist_from(feet: Fx) -> Vec<Event> {
    let mut g = game(&world(20, &[(11, 6, 't')]));
    g.player.x = Fx::from_px(6 * 16 + 2);
    g.player.y = feet - g.player.h;
    g.player.vy = Fx::from_px(1);
    g.player.on_ground = false;
    g.step(input("-")).to_vec()
}

#[test]
fn stomp_needs_the_previous_feet_above_the_enemy_middle() {
    let middle = Fx::from_px(182);
    // Exactly at the middle: stomp.
    let events = fall_onto_tourist_from(middle);
    assert!(events.contains(&Event::Stomped), "{events:?}");
    // One subpixel below the middle on the previous frame: hurt (death as
    // Normal), not a stomp.
    let events = fall_onto_tourist_from(middle + Fx(1));
    assert!(!events.contains(&Event::Stomped), "{events:?}");
    assert!(events.contains(&Event::Died), "{events:?}");
}

/// One frame with a stomp on tourist A (on the ground, middle 182) and a
/// side touch of tourist B (y 150..170, middle 160, so the previous feet at
/// 180 are below it): A is stomped, B hurts (Big: shrink), whichever comes
/// first in the entity list.
fn stomp_and_side_touch(b_first: bool) -> (Game, Vec<Event>) {
    let t = tuning();
    let mut g = game(&world(20, &[(3, 10, 'P'), (8, 5, 't'), (11, 2, 't')]));
    // entities: [B (row 8), A (row 11)]; move B next to A.
    let b = &mut g.entities[0];
    b.x = Fx::from_px(44);
    b.y = Fx::from_px(150);
    if !b_first {
        g.entities.swap(0, 1);
    }
    g.player.set_power(Power::Big, &t.player);
    g.player.transform = 0;
    g.player.x = Fx::from_px(36);
    g.player.y = Fx::from_px(180) - g.player.h;
    g.player.vy = Fx::from_px(1);
    g.player.on_ground = false;
    let events = g.step(input("-")).to_vec();
    (g, events)
}

#[test]
fn stomps_and_hurts_do_not_depend_on_entity_order() {
    let (a, a_events) = stomp_and_side_touch(true);
    let (b, b_events) = stomp_and_side_touch(false);
    for (g, events) in [(&a, &a_events), (&b, &b_events)] {
        assert!(events.contains(&Event::Stomped), "{events:?}");
        assert!(events.contains(&Event::Hurt), "{events:?}");
        assert_eq!(g.score(), 100);
        assert_eq!(g.player().power, Power::Normal);
        assert_eq!(count(g, EntityKind::Enemy(EnemyKind::SelfieTourist)), 1);
    }
    assert_eq!(a.player(), b.player(), "same player state in both orders");
    let sorted = |e: &Vec<Event>| {
        let mut n: Vec<&str> = e.iter().map(|e| e.name()).collect();
        n.sort();
        n
    };
    assert_eq!(sorted(&a_events), sorted(&b_events));
}

/// The per-kind fields the enemy task builds on; `prev_y` is the
/// position the stomp rule uses (snapshot before the entity moves).
#[test]
fn enemies_have_per_kind_state_and_a_previous_position() {
    let mut g = game(&world(20, &[(9, 2, 'P'), (11, 2, 't')]));
    let e = &mut g.entities[0];
    assert_eq!(e.prev_y, e.y);
    assert_eq!(e.dir, -1, "enemies face the player");
    assert_eq!(e.base_y, e.y);
    until(&mut g, "-", 120, |g| g.events().contains(&Event::Stomped));
}

#[test]
fn landing_resets_the_stomp_chain() {
    let mut g = stomped('t', "-");
    until(&mut g, "-", 120, |g| g.player().on_ground);
    assert_eq!(g.player().stomp_chain, 0);
}

// ---------------------------------------------- Oblaten, lives, score

#[test]
fn static_oblaten_are_collected_and_100_give_a_life() {
    let mut g = game(&world(20, &[(11, 4, 'o'), (11, 5, 'o')]));
    g.oblaten = 98;
    let events = hold(&mut g, "R", 60);
    let collected = events
        .iter()
        .filter(|e| **e == Event::OblateCollected)
        .count();
    assert_eq!(collected, 2);
    assert!(events.contains(&Event::ExtraLife));
    assert_eq!((g.oblaten(), g.lives(), g.score()), (0, 4, 400));
    assert_eq!(count(&g, EntityKind::Oblate), 0);
}

#[test]
fn lives_and_score_are_capped() {
    let mut g = game(&world(20, &[(11, 4, 'o')]));
    g.lives = 99;
    g.oblaten = 99;
    g.score = crate::game::MAX_SCORE - 100;
    let events = hold(&mut g, "R", 60);
    assert_eq!(g.lives(), 99);
    assert_eq!(g.oblaten(), 0, "the counter still restarts");
    assert!(!events.contains(&Event::ExtraLife), "no life at the cap");
    assert_eq!(g.score(), crate::game::MAX_SCORE);
}

#[test]
fn zaunerstollen_at_the_lives_cap_shows_no_popup() {
    let mut g = game(&world(20, &[(9, 2, 'Z'), (10, 8, '#'), (11, 8, '#')]));
    g.lives = tuning().game.lives_max;
    until(&mut g, "J", 30, |g| {
        g.events().contains(&Event::BlockBumped)
    });
    let events = until(&mut g, "-", 400, |g| {
        count(g, EntityKind::Zaunerstollen) == 0
    });
    assert!(!events.contains(&Event::ExtraLife));
    assert!(
        !g.entities()
            .iter()
            .any(|e| e.kind == EntityKind::Popup(Popup::ExtraLife))
    );
}

// ------------------------------------------------------------ timer

#[test]
fn timer_counts_down_every_60_frames_and_kills_at_0() {
    let mut g = game(&world_with(20, 3, &[]));
    assert_eq!(g.time_left(), 3);
    hold(&mut g, "-", 59);
    assert_eq!(g.time_left(), 3);
    hold(&mut g, "-", 1);
    assert_eq!(g.time_left(), 2);
    let events = hold(&mut g, "-", 120);
    assert_eq!(g.time_left(), 0);
    assert_eq!(events.last(), Some(&Event::Died));
    assert_eq!(g.death_cause(), Some(DeathCause::TimeUp));
    assert_eq!(g.frame(), 180);
    // Respawn resets the timer.
    until(&mut g, "-", 120, |g| g.mode() == Mode::Playing);
    assert_eq!(g.time_left(), 3);
}

#[test]
fn timer_stops_while_dying_and_at_the_goal() {
    let mut g = game(&world_with(20, 100, &[(11, 4, 'G')]));
    until(&mut g, "R", 120, |g| g.mode() == Mode::StageClear);
    let t = g.time_left();
    // It stands still while sliding, hopping and walking off; only the
    // time-bonus tally counts it down (tests/goal.rs).
    until(&mut g, "-", 400, |g| {
        g.stage_clear().unwrap().phase == crate::ClearPhase::Tally
    });
    assert_eq!(g.time_left(), t);
}

// --------------------------------------- checkpoint, reset, game over

/// Box above the start, an Oblate, a checkpoint at column 8, a brick and
/// a pit at columns 14–15.
fn checkpoint_world() -> Level {
    world(
        30,
        &[
            (9, 2, '?'),
            (11, 4, 'o'),
            (11, 8, 'C'),
            (12, 14, '^'),
            (13, 14, '^'),
            (12, 15, '^'),
            (13, 15, '^'),
        ],
    )
}

#[test]
fn checkpoint_activates_when_the_centre_passes_its_x() {
    let mut g = game(&checkpoint_world());
    let cp = g
        .level()
        .entities
        .iter()
        .position(|e| e.kind == SpawnKind::Checkpoint)
        .unwrap();
    let events = until(&mut g, "R", 200, |g| g.checkpoint().is_some());
    assert!(events.contains(&Event::CheckpointReached));
    assert_eq!(g.checkpoint(), Some(cp));
    assert_eq!(g.player().center_px().0, 8 * 16 + 8, "exactly at its x");
    let popup = g
        .entities()
        .iter()
        .find(|e| e.kind == EntityKind::Popup(Popup::Checkpoint))
        .expect("checkpoint popup");
    // Its text top 52 px above the checkpoint's ground: glyph rows 0–7
    // (incl. the descender row) and the outline row 8 end 44 px above the
    // ground, leaving 3 px of air above the 40 px bus board (at least 2
    // required), and the popup only rises.
    let ground = (g.level().entities[cp].row + 1) * 16;
    assert_eq!(popup.y.to_px(), ground - 52);
    let lowest_row = popup.y.to_px() + 8;
    let board_top = ground - 40;
    assert_eq!(board_top - lowest_row - 1, 3, "px of air above the board");
    // Only once.
    let events = hold(&mut g, "R", 20);
    assert!(!events.contains(&Event::CheckpointReached));
}

#[test]
fn death_resets_the_level_and_respawns_at_the_checkpoint() {
    let t = tuning();
    let level = checkpoint_world();
    let fresh = game(&level);
    let mut g = game(&level);
    until(&mut g, "J", 30, |g| {
        g.events().contains(&Event::BlockBumped)
    });
    hold(&mut g, "-", 40);
    let events = until(&mut g, "R", 300, |g| g.mode() != Mode::Playing);
    assert!(events.contains(&Event::CheckpointReached));
    assert_eq!(g.death_cause(), Some(DeathCause::Hazard));
    assert_eq!((g.oblaten(), g.score(), g.lives()), (2, 400, 2));
    assert_eq!(count(&g, EntityKind::Oblate), 0);
    assert_eq!(g.tile(2, 9).unwrap().block, Block::None);
    hold(&mut g, "-", 30);
    let time_before = g.time_left();

    let events = until(&mut g, "-", 120, |g| g.mode() == Mode::Playing);
    assert!(events.contains(&Event::Respawned));
    // Level back to the start state; score, Oblaten, lives, checkpoint kept.
    assert_eq!(g.tile(2, 9), fresh.tile(2, 9));
    assert_eq!(g.entities().len(), fresh.entities().len());
    assert_eq!(count(&g, EntityKind::Oblate), 1);
    assert_eq!((g.oblaten(), g.score(), g.lives()), (2, 400, 2));
    assert!(g.checkpoint().is_some());
    assert_eq!(g.time_left(), level.meta.time_limit as u16);
    assert!(time_before <= g.time_left());
    // Respawned on the ground at the checkpoint, as Normal.
    let p = g.player();
    assert_eq!(p.center_px().0, 8 * 16 + 8);
    assert_eq!(feet_y(&g), Fx::from_px(12 * 16));
    assert_eq!(p.power, Power::Normal);
    assert!(p.on_ground);
    assert!(g.camera().x <= 8 * 16);
    let _ = t;
}

#[test]
fn respawn_without_checkpoint_is_at_the_start_and_as_normal() {
    let level = world(30, &[(12, 6, '^'), (13, 6, '^')]);
    let mut g = game(&level);
    g.player.set_power(Power::Big, &tuning().player);
    g.player.transform = 0;
    until(&mut g, "R", 200, |g| g.mode() != Mode::Playing);
    until(&mut g, "-", 120, |g| g.mode() == Mode::Playing);
    assert_eq!(*g.player(), *game(&level).player());
}

#[test]
fn losing_the_last_life_is_game_over_and_start_restarts_the_stage() {
    let t = tuning();
    let level = checkpoint_world();
    let mut g = game(&level);
    until(&mut g, "J", 30, |g| {
        g.events().contains(&Event::BlockBumped)
    });
    hold(&mut g, "-", 40);
    for lives in (0..3).rev() {
        until(&mut g, "R", 400, |g| g.mode() != Mode::Playing);
        assert_eq!(g.lives(), lives);
        if lives > 0 {
            until(&mut g, "-", 120, |g| g.mode() == Mode::Playing);
        }
    }
    let events = until(&mut g, "-", 120, |g| {
        matches!(g.mode(), Mode::GameOver { .. })
    });
    assert!(events.contains(&Event::GameOver));
    assert_eq!(g.mode(), Mode::GameOver { frames: 0 });
    assert!(g.score() > 0);
    // START before `game_over_frames` does nothing.
    hold(&mut g, "S", 10);
    hold(&mut g, "-", 1);
    assert!(matches!(g.mode(), Mode::GameOver { .. }));
    // Holding START from before does not count either: it needs a press.
    hold(&mut g, "S", t.game.game_over_frames as u32);
    assert!(matches!(g.mode(), Mode::GameOver { .. }));
    hold(&mut g, "-", 1);
    let events = g.step(input("S")).to_vec();
    assert!(
        !events.contains(&Event::Respawned),
        "a restart is no respawn"
    );
    assert_eq!(g.mode(), Mode::Playing);
    assert_eq!((g.lives(), g.score(), g.oblaten()), (3, 0, 0));
    assert_eq!(g.checkpoint(), None);
    assert_eq!(*g.player(), *game(&level).player(), "back at the start");
    assert_eq!(g.entities(), game(&level).entities());
}

// ------------------------------------------- activation and despawn

#[test]
fn entities_activate_within_one_screen_and_despawn_behind() {
    let t = tuning();
    // Oblaten at columns 0 (behind the start), 30 (x 483) and 60 (x 963).
    let mut g = game(&world(120, &[(5, 0, 'o'), (5, 30, 'o'), (5, 60, 'o')]));
    g.step(input("-"));
    let active: Vec<bool> = g.entities().iter().map(|e| e.active).collect();
    assert_eq!(active, vec![true, true, false]);
    // Activates once its left edge is within `activate_margin` of the
    // right screen edge.
    let limit = 960 + 3 - 320 - t.game.activate_margin;
    until(&mut g, "RB", 400, |g| g.camera().x > limit);
    assert!(!g.entities().last().unwrap().active, "not yet");
    g.step(input("RB"));
    assert!(g.entities().iter().all(|e| e.active));
    // The first one is gone once it is far enough left of the screen.
    assert_eq!(g.entities().len(), 2, "column 0 despawned");
    assert!(g.entities()[0].x > Fx::from_px(g.camera().x - t.game.despawn_margin));
}

#[test]
fn signs_and_checkpoints_are_entities_without_collision() {
    let mut g = game(&world(20, &[(11, 4, 's'), (11, 6, 'C')]));
    let kinds: Vec<EntityKind> = g.entities().iter().map(|e| e.kind).collect();
    assert_eq!(kinds, vec![EntityKind::Sign, EntityKind::Checkpoint]);
    assert_eq!(
        g.entities()[0].spawn,
        Some(1),
        "spawn 0 is the player start"
    );
    let events = hold(&mut g, "R", 60);
    assert!(!events.contains(&Event::Died), "{events:?}");
}

#[test]
fn world_state_is_part_of_the_hash() {
    let level = world(20, &[(9, 2, '?'), (11, 6, 'o')]);
    let mut a = game(&level);
    let mut b = game(&level);
    assert_eq!(a.state_hash(), b.state_hash());
    assert_eq!(a.world_hash(), b.world_hash());
    // Same player state, different tiles.
    b.empty_box(2, 9);
    assert_ne!(a.world_hash(), b.world_hash());
    let mut c = game(&level);
    c.entities[0].removed = true;
    c.entities.retain(|e| !e.removed);
    assert_ne!(a.state_hash(), c.state_hash());
    // Score, lives, Oblaten and the timer are hashed too.
    let base = a.state_hash();
    a.score += 1;
    assert_ne!(a.state_hash(), base);
    a.score -= 1;
    a.lives += 1;
    assert_ne!(a.state_hash(), base);
    a.lives -= 1;
    a.time -= 1;
    assert_ne!(a.state_hash(), base);
    a.time += 1;
    assert_eq!(a.state_hash(), base);
}

// ------------------------------------------------- growing under blocks

/// Puts a Mozartkugel (done rising) on the player's centre, so the next
/// step's `interact` makes her grow.
fn feed_mozartkugel(g: &mut Game) {
    let t = tuning();
    let p = *g.player();
    let mut m = crate::entity::Entity::at_feet(
        EntityKind::Mozartkugel,
        p.x + p.w.div_int(2),
        p.y + p.h.div_int(2),
        t.items.mozartkugel_hitbox,
    );
    m.timer = t.items.rise_frames + 1;
    g.entities.push(m);
}

#[test]
fn growing_while_rising_under_a_box_is_a_head_bump() {
    let t = tuning();
    // A solid block at column 2, row 9 (y 144..160), right above the start.
    let mut g = game(&world(20, &[(9, 2, '#')]));
    // Rising, head 4 px below the block: growing by 8 px puts it inside.
    g.player.y = Fx::from_px(164);
    g.player.vy = Fx::from_px(-2);
    g.player.on_ground = false;
    feed_mozartkugel(&mut g);
    let events = g.step(input("-")).to_vec();
    assert!(events.contains(&Event::PowerUp));
    let p = *g.player();
    assert_eq!(p.power, Power::Big);
    assert_eq!(p.y, Fx::from_px(160), "pushed down, flush below the block");
    assert_eq!(p.vy, Fx::ZERO, "upward speed stopped");
    assert!(!p.jump_rising);
    // After the freeze she falls back to the ground and never gets above
    // the block's bottom.
    for _ in 0..t.player.transform_frames + 60 {
        g.step(input("-"));
        assert!(g.player().y >= Fx::from_px(160), "never into the block");
    }
    assert!(g.player().on_ground);
    assert_eq!(feet_y(&g), Fx::from_px(12 * 16));
    assert_eq!(g.tile(2, 9).unwrap().kind, TileKind::Solid);
}

#[test]
fn growing_while_rising_with_jump_held_does_not_pass_the_box() {
    // Same, but JUMP stays held the whole time (lower gravity while rising).
    let mut g = game(&world(20, &[(9, 2, '#')]));
    g.player.y = Fx::from_px(163);
    g.player.vy = Fx::from_px(-3);
    g.player.on_ground = false;
    g.player.jump_rising = true;
    g.player.jump_held = true;
    feed_mozartkugel(&mut g);
    for _ in 0..120 {
        g.step(input("J"));
        assert!(g.player().y >= Fx::from_px(160));
    }
    assert_eq!(g.player().power, Power::Big);
    assert_eq!(feet_y(&g), Fx::from_px(12 * 16));
}

#[test]
fn growing_on_the_ground_under_the_lowest_ceiling_fits_and_walks_out() {
    let t = tuning();
    // A ceiling at row 9 over columns 0..8 (y 144..160): two tiles of air
    // above the ground, the lowest a Normal Resi can stand under.
    let marks: Vec<_> = (0..8).map(|c| (9, c, '#')).collect();
    let mut g = game(&world(20, &marks));
    feed_mozartkugel(&mut g);
    g.step(input("-"));
    let p = *g.player();
    assert_eq!(p.power, Power::Big);
    assert_eq!(feet_y(&g), Fx::from_px(12 * 16), "feet stay on the ground");
    assert!(p.y >= Fx::from_px(160), "the head fits under the ceiling");
    assert!(p.on_ground);
    hold(&mut g, "-", u32::from(t.player.transform_frames));
    // A jump under the ceiling bumps, it does not pass.
    for _ in 0..40 {
        g.step(input("J"));
        assert!(g.player().y >= Fx::from_px(160));
    }
    hold(&mut g, "-", 30);
    // She walks out from under it.
    until(&mut g, "R", 200, |g| g.player().x >= Fx::from_px(8 * 16));
    assert_eq!(feet_y(&g), Fx::from_px(12 * 16));
}

#[test]
fn fit_head_only_acts_on_overlap() {
    let level = world(20, &[(9, 2, '#')]);
    let map = crate::physics::TileMap::from_level(&level);
    let mut p = *game(&level).player();
    p.set_power(Power::Big, &tuning().player);
    // Standing on the ground: the big head (y 162) is below the block.
    let before = p;
    assert!(!p.fit_head(&map));
    assert_eq!(p, before);
    // In the air with the head inside the block: pushed down, never up.
    p.y = Fx::from_px(150);
    p.vy = Fx::from_px(-2);
    assert!(p.fit_head(&map));
    assert_eq!((p.y, p.vy), (Fx::from_px(160), Fx::ZERO));
    // Falling speed is kept.
    p.y = Fx::from_px(150);
    p.vy = Fx::from_px(2);
    assert!(p.fit_head(&map));
    assert_eq!((p.y, p.vy), (Fx::from_px(160), Fx::from_px(2)));
}

/// The player stands on the ground and touches a Mozartkugel; an Oblate
/// sits right above her Normal head, inside the Zuckerschock hitbox she
/// grows into this frame. `ball_first`: the Mozartkugel comes first in the
/// entity list.
fn grow_under_an_oblate(ball_first: bool) -> Game {
    let mut g = game(&world(20, &[(5, 10, 'o'), (5, 12, 'o')]));
    let p = *g.player();
    let ball = &mut g.entities[0];
    ball.kind = EntityKind::Mozartkugel;
    ball.timer = 1000; // risen long ago
    ball.vx = Fx::ZERO;
    ball.x = p.x;
    ball.y = p.y + p.h - ball.h;
    let oblate = &mut g.entities[1];
    oblate.x = p.x;
    oblate.y = p.y - oblate.h; // bottom flush with the Normal head
    if !ball_first {
        g.entities.swap(0, 1);
    }
    g.step(input("-"));
    g
}

/// All contacts of a frame use the hitbox from the start of the contact
/// pass: growing (and `fit_head`) happens after it, so the Oblate above the
/// Normal head is not collected in the frame she grows, in either order.
#[test]
fn contacts_in_the_growing_frame_use_the_hitbox_before_growing() {
    let a = grow_under_an_oblate(true);
    let b = grow_under_an_oblate(false);
    for g in [&a, &b] {
        assert_eq!(g.player().power, Power::Big);
        assert!(g.events().contains(&Event::PowerUp));
        assert_eq!(g.oblaten(), 0, "the Oblate above the old head stays");
        assert_eq!(count(g, EntityKind::Oblate), 1);
    }
    assert_eq!(a.player(), b.player(), "same player in both orders");
    assert_eq!(a.score(), b.score());
}

// ------------------------------------------------------------- popups

#[test]
fn breaking_a_brick_shows_a_50_popup() {
    let mut g = game(&world(20, &[(9, 2, 'B')]));
    g.player.set_power(Power::Big, &tuning().player);
    g.player.transform = 0;
    until(&mut g, "J", 30, |g| {
        g.events().contains(&Event::BrickBroken)
    });
    let pops: Vec<_> = g
        .entities()
        .iter()
        .filter(|e| matches!(e.kind, EntityKind::Popup(_)))
        .collect();
    assert_eq!(pops.len(), 1);
    assert_eq!(pops[0].kind, EntityKind::Popup(Popup::Score(50)));
    assert_eq!(pops[0].feet().0, Fx::from_px(2 * 16 + 8), "above the brick");
}

#[test]
fn the_100th_oblate_shows_the_extra_life_popup() {
    let mut g = game(&world(20, &[(11, 4, 'o')]));
    g.oblaten = 99;
    let events = until(&mut g, "R", 60, |g| {
        g.events().contains(&Event::OblateCollected)
    });
    assert!(events.contains(&Event::ExtraLife));
    assert_eq!(count(&g, EntityKind::Popup(Popup::ExtraLife)), 1);
}

#[test]
fn the_100th_oblate_from_a_box_shows_the_extra_life_popup() {
    let mut g = game(&world(20, &[(9, 2, '?')]));
    g.oblaten = 99;
    until(&mut g, "J", 30, |g| {
        g.events().contains(&Event::OblateCollected)
    });
    assert_eq!(g.lives(), 4);
    assert_eq!(count(&g, EntityKind::Popup(Popup::ExtraLife)), 1);
}

#[test]
fn the_100th_oblate_at_the_lives_cap_shows_no_popup() {
    let mut g = game(&world(20, &[(11, 4, 'o')]));
    g.oblaten = 99;
    g.lives = tuning().game.lives_max;
    until(&mut g, "R", 60, |g| {
        g.events().contains(&Event::OblateCollected)
    });
    assert_eq!(g.oblaten(), 0);
    assert_eq!(count(&g, EntityKind::Popup(Popup::ExtraLife)), 0);
}
