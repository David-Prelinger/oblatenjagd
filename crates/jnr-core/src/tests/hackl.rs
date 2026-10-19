//! The Hackl power-up, the thrown axes (D-056) and the Durchkriechstein
//! crawl (D-057): every rule of the Stage 4 "New mechanics" table for them.

use std::sync::Arc;

use super::*;
use crate::crawl::stone;
use crate::hackl::axe;
use crate::player::{Facing, Power};
use crate::route::Route;
use crate::{EnemyKind, Entity, EntityKind, Fx, Mode, Popup, Start};

/// Legend of the test worlds (`.` air, `P` start, `G` goal).
const LEGEND: &str = r##"
"#" = "solid"
"-" = "oneway"
"~" = "salzach_water"
"^" = "hazard"
"B" = "brick"
"H" = { tile = "oblaten_box", contains = "hackl" }
"K" = { entity = "durchkriechstein" }
"P" = { entity = "player_start" }
"G" = { entity = "goal" }
"##;

/// A 14-row world `width` columns wide: ground in rows 12–13 (feet at y
/// 192), the start at column 2 unless `marks` has a `P`, plus `marks` as
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
        "[meta]\nid = \"hackl_test\"\ntime_limit = 300\n[legend]\n{LEGEND}\n[map]\ngrid = '''\n{}\n'''\n",
        grid.join("\n")
    );
    Level::parse(&text).unwrap()
}

fn game(level: &Level) -> Game {
    Game::new(level, &tuning(), 1)
}

/// `g` with the player as Hackl at once (no freeze).
fn as_hackl(mut g: Game) -> Game {
    g.player.set_power(Power::Hackl, &tuning().player);
    g.player.transform = 0;
    g
}

fn axes(g: &Game) -> Vec<Entity> {
    g.entities()
        .iter()
        .filter(|e| e.kind == EntityKind::ThrownHackl)
        .copied()
        .collect()
}

fn feet_y(g: &Game) -> Fx {
    g.player().y + g.player().h
}

fn popups(g: &Game) -> Vec<Popup> {
    g.entities()
        .iter()
        .filter_map(|e| match e.kind {
            EntityKind::Popup(p) => Some(p),
            _ => None,
        })
        .collect()
}

/// An active enemy standing on the ground with its feet centre at `x` px.
fn enemy(kind: EnemyKind, x: i32) -> Entity {
    let t = tuning();
    let size = match kind {
        EnemyKind::Fiaker => t.enemies.fiaker_hitbox,
        EnemyKind::MozartkugelVerkaeufer => t.enemies.mozartkugel_verkaeufer_hitbox,
        EnemyKind::Bienenschwarm => t.enemies.bienenschwarm_hitbox,
        _ => t.enemies.selfie_tourist_hitbox,
    };
    Entity::at_feet(
        EntityKind::Enemy(kind),
        Fx::from_px(x),
        Fx::from_px(192),
        size,
    )
}

// ------------------------------------------------------------ the item

#[test]
fn hackl_rises_out_of_its_box_and_stays() {
    let t = tuning();
    let mut g = game(&world(20, &[(8, 2, 'H')]));
    until(&mut g, "J", 30, |g| {
        g.events().contains(&Event::BlockBumped)
    });
    let item = |g: &Game| {
        *g.entities()
            .iter()
            .find(|e| e.kind == EntityKind::Hackl)
            .expect("the Hackl came out")
    };
    let e = item(&g);
    assert_eq!((e.w, e.h), (Fx::from_px(12), Fx::from_px(12)));
    assert_eq!(
        e.feet(),
        (Fx::from_px(40), Fx::from_px(144) - t.items.rise_speed)
    );
    assert!(e.rising(&t));
    hold(&mut g, "-", 200);
    let e = item(&g);
    assert_eq!(
        e.feet(),
        (Fx::from_px(40), Fx::from_px(128)),
        "on top of the box"
    );
    assert!(!e.rising(&t));
    assert_eq!((e.vx, e.vy), (Fx::ZERO, Fx::ZERO), "it stays put");
    assert_eq!(g.player().power, Power::Normal, "out of reach from below");
}

fn hackl_item_at_player(g: &mut Game) {
    let (fx, fy) = g.player().feet();
    let mut e = Entity::at_feet(EntityKind::Hackl, fx, fy, [12, 12]);
    e.timer = 100;
    g.entities.push(e);
}

#[test]
fn normal_and_zuckerschock_become_hackl_with_the_freeze() {
    let t = tuning();
    for from in [Power::Normal, Power::Big] {
        let mut g = game(&world(20, &[]));
        g.player.set_power(from, &t.player);
        g.player.transform = 0;
        let feet = feet_y(&g);
        hackl_item_at_player(&mut g);
        let events = hold(&mut g, "-", 1);
        assert!(events.contains(&Event::PowerUp), "{from:?}: {events:?}");
        let p = *g.player();
        assert_eq!((p.power, p.prev_power), (Power::Hackl, from));
        assert_eq!(p.h, Fx::from_px(t.player.hitbox_h_big), "big");
        assert_eq!(feet_y(&g), feet, "feet kept");
        assert_eq!(p.transform, t.player.transform_frames);
        assert_eq!(g.score(), t.score.power_up);
        let shown = popups(&g);
        assert!(
            shown.contains(&Popup::Hackl) && shown.contains(&Popup::Score(1000)),
            "{shown:?}"
        );
        // The whole game freezes for 30 frames (the timer too).
        let time = (g.time_left(), g.frame());
        hold(&mut g, "R", u32::from(t.player.transform_frames));
        assert_eq!(g.player().x, p.x, "frozen");
        assert_eq!(g.time_left(), time.0);
        hold(&mut g, "R", 1);
        assert!(g.player().x > p.x, "moves again");
    }
}

#[test]
fn hackl_items_and_mozartkugeln_as_hackl_give_points_only() {
    let t = tuning();
    let mut g = as_hackl(game(&world(20, &[])));
    hackl_item_at_player(&mut g);
    let events = hold(&mut g, "-", 1);
    assert!(!events.contains(&Event::PowerUp));
    assert_eq!((g.player().power, g.player().transform), (Power::Hackl, 0));
    assert_eq!(g.score(), 1000);
    assert!(!popups(&g).contains(&Popup::Hackl));
    let (fx, fy) = g.player().feet();
    let mut k = Entity::at_feet(EntityKind::Mozartkugel, fx, fy, [10, 10]);
    k.timer = 100;
    g.entities.push(k);
    hold(&mut g, "-", 1);
    assert_eq!((g.player().power, g.player().transform), (Power::Hackl, 0));
    assert_eq!(g.score(), 2000);
    let _ = t;
}

#[test]
fn hackl_breaks_bricks() {
    let mut g = as_hackl(game(&world(20, &[(8, 2, 'B')])));
    let events = until(&mut g, "J", 30, |g| {
        g.events().contains(&Event::BrickBroken)
    });
    assert!(events.contains(&Event::BrickBroken));
    assert_eq!(g.tile(2, 8).unwrap().kind, crate::TileKind::Empty);
}

#[test]
fn a_hit_drops_hackl_to_zuckerschock_with_invulnerability() {
    let t = tuning();
    let mut g = as_hackl(game(&world(20, &[])));
    let mut e = enemy(EnemyKind::SelfieTourist, 44);
    e.dir = 0;
    g.entities.push(e);
    let events = hold(&mut g, "-", 1);
    assert!(events.contains(&Event::Hurt), "{events:?}");
    let p = *g.player();
    assert_eq!((p.power, p.prev_power), (Power::Big, Power::Hackl));
    assert_eq!(p.h, Fx::from_px(t.player.hitbox_h_big), "still big");
    assert_eq!(p.transform, t.player.transform_frames, "the freeze");
    assert_eq!(p.invulnerable, t.player.invulnerable_frames);
    // During the i-frames the tourist does nothing; after them it hurts
    // again (Zuckerschock → Normal).
    hold(&mut g, "-", u32::from(t.player.transform_frames) + 119);
    assert_eq!(g.player().power, Power::Big);
    let events = hold(&mut g, "-", 2);
    assert!(events.contains(&Event::Hurt));
    assert_eq!(g.player().power, Power::Normal);
}

#[test]
fn death_respawns_as_normal() {
    let mut g = as_hackl(game(&world(20, &[(12, 5, '^')])));
    until(&mut g, "R", 120, |g| g.events().contains(&Event::Respawned));
    assert_eq!(g.player().power, Power::Normal);
}

#[test]
fn hackl_carries_over_into_the_next_stage() {
    let t = tuning();
    let mk = |id: &str| {
        let mut l = world(12, &[(11, 6, 'G')]);
        l.meta.id = id.to_owned();
        l
    };
    let route = Arc::new(Route::new(vec![mk("a"), mk("b")]).unwrap());
    let mut g = Game::new_run(route, 0, &t, 1, Start::Title);
    hold(&mut g, "S", 1);
    hold(&mut g, "-", u32::from(t.game.intro_frames));
    g.player.set_power(Power::Hackl, &t.player);
    g.player.transform = 0;
    until(&mut g, "R", 600, |g| g.mode() == Mode::StageClear);
    until(&mut g, "S", 3000, |g| g.level().meta.id == "b");
    let p = *g.player();
    assert_eq!((p.power, p.transform), (Power::Hackl, 0), "no freeze");
    assert_eq!(p.h, Fx::from_px(t.player.hitbox_h_big));
    assert_eq!(feet_y(&g), Fx::from_px(192));
}

// ------------------------------------------------------------- throwing

#[test]
fn a_run_press_throws_once_and_holding_run_keeps_running() {
    let t = tuning();
    let mut g = as_hackl(game(&world(40, &[])));
    let events = hold(&mut g, "B", 1);
    assert!(events.contains(&Event::HacklThrown));
    assert_eq!(axes(&g).len(), 1);
    assert_eq!(g.player().throw_pose, t.hackl.throw_pose_frames);
    let events = hold(&mut g, "RB", 20);
    assert!(!events.contains(&Event::HacklThrown), "held: no new throw");
    assert!(g.player().vx > t.player.walk_speed, "RUN still runs");
    assert_eq!(g.player().throw_pose, 0, "the pose lasted 10 frames");
    // Release and press again: a second one.
    hold(&mut g, "R", 1);
    assert!(hold(&mut g, "RB", 1).contains(&Event::HacklThrown));
    assert_eq!(axes(&g).len(), 2);
}

#[test]
fn at_most_two_axes_are_alive() {
    let mut g = as_hackl(game(&world(40, &[])));
    for _ in 0..2 {
        assert!(hold(&mut g, "B", 1).contains(&Event::HacklThrown));
        hold(&mut g, "-", 1);
    }
    assert!(!hold(&mut g, "B", 1).contains(&Event::HacklThrown));
    assert_eq!(axes(&g).len(), 2);
    // Once one is gone (stuck 20 frames in the ground), a press throws again.
    until(&mut g, "-", 200, |g| axes(g).len() < 2);
    assert!(hold(&mut g, "B", 1).contains(&Event::HacklThrown));
}

#[test]
fn only_hackl_throws() {
    for power in [Power::Normal, Power::Big] {
        let mut g = game(&world(40, &[]));
        g.player.set_power(power, &tuning().player);
        g.player.transform = 0;
        let events = hold(&mut g, "B", 1);
        assert!(!events.contains(&Event::HacklThrown), "{power:?}");
        assert!(axes(&g).is_empty());
    }
}

#[test]
fn no_throw_while_frozen_and_a_held_run_is_no_press_after_it() {
    let mut g = game(&world(40, &[]));
    g.player.set_power(Power::Hackl, &tuning().player);
    assert!(g.player().transform > 0);
    let events = hold(&mut g, "B", 40);
    assert!(!events.contains(&Event::HacklThrown), "{events:?}");
    assert!(axes(&g).is_empty());
}

#[test]
fn no_throw_in_the_goal_sequence() {
    let mut g = as_hackl(game(&world(20, &[(11, 4, 'G')])));
    until(&mut g, "R", 120, |g| g.mode() == Mode::StageClear);
    for _ in 0..30 {
        assert!(!hold(&mut g, "B", 1).contains(&Event::HacklThrown));
        hold(&mut g, "-", 1);
    }
    assert!(axes(&g).is_empty());
}

#[test]
fn the_arc_is_fixed() {
    let t = tuning();
    let mut g = as_hackl(game(&world(60, &[])));
    // Running fast does not change it.
    hold(&mut g, "RB", 30);
    g.entities.retain(|e| e.kind != EntityKind::ThrownHackl);
    hold(&mut g, "R", 1);
    let p = *g.player();
    hold(&mut g, "RB", 1);
    let a = axes(&g)[0];
    assert_eq!((a.w, a.h), (Fx::from_px(8), Fx::from_px(8)));
    // Spawned centred 8 px ahead of her centre (moved by her own step
    // first), 10 px below her hitbox top, then moved once.
    let q = *g.player();
    let (cx, cy) = (q.x + q.w.div_int(2) + Fx::from_px(8), q.y + Fx::from_px(10));
    assert_eq!(a.x + a.w.div_int(2), cx + t.hackl.throw_speed_x);
    assert_eq!(a.y + a.h.div_int(2), cy - t.hackl.throw_speed_y);
    assert!(q.x > p.x);
    let start = (a.x, a.y);
    // 16 frames: apex 25.5 px up; 32 frames: 96 px ahead and back at
    // hand height (3 px above).
    let mut ys = vec![a.y];
    for _ in 1..32 {
        g.step(input("-"));
        ys.push(axes(&g)[0].y);
    }
    let a = axes(&g)[0];
    assert_eq!(a.x - start.0, Fx::from_px(93), "3 px per frame");
    let top = *ys.iter().min().unwrap();
    assert_eq!(
        start.1 - top,
        Fx::from_px(25) + Fx::ONE.div_int(2) - Fx::from_px(3)
    );
    assert_eq!(ys.iter().position(|&y| y == top), Some(15));
    g.step(input("-"));
    let a = axes(&g)[0];
    // 33 moves (32 frames after the throw frame): back at hand height,
    // 6 tiles + 3 px ahead.
    assert_eq!(a.y + a.h.div_int(2), cy, "hand height");
    assert_eq!(a.x + a.w.div_int(2) - cx, Fx::from_px(99));
    assert_eq!(a.state, axe::FLY);
}

#[test]
fn axes_stick_in_solid_tiles_harmlessly_then_vanish() {
    let t = tuning();
    // A wall 3 tiles ahead.
    let wall: Vec<(usize, usize, char)> = (5..12).map(|r| (r, 5, '#')).collect();
    let mut g = as_hackl(game(&world(40, &wall)));
    hold(&mut g, "B", 1);
    until(&mut g, "-", 30, |g| axes(g)[0].state == axe::STUCK);
    let a = axes(&g)[0];
    assert_eq!(a.right(), Fx::from_px(80), "flush with the wall");
    assert_eq!((a.vx, a.vy), (Fx::ZERO, Fx::ZERO));
    hold(&mut g, "-", u32::from(t.hackl.stuck_frames) - 1);
    assert_eq!(axes(&g).len(), 1);
    hold(&mut g, "-", 1);
    assert!(axes(&g).is_empty(), "removed after 20 frames");
    // Thrown against the wall it is stuck at once.
    let mut g = as_hackl(game(&world(40, &[(10, 3, '#'), (11, 3, '#')])));
    hold(&mut g, "B", 1);
    assert_eq!(axes(&g)[0].state, axe::STUCK);
    // A stuck axe does not hit anything.
    let mut g = as_hackl(game(&world(40, &wall)));
    hold(&mut g, "B", 1);
    until(&mut g, "-", 30, |g| axes(g)[0].state == axe::STUCK);
    let a = axes(&g)[0];
    let mut e = enemy(EnemyKind::SelfieTourist, (a.x.to_px()) + 4);
    e.dir = 0;
    e.y = a.y;
    g.entities.push(e);
    let events = hold(&mut g, "-", 1);
    assert!(!events.contains(&Event::AxeHit));
}

#[test]
fn axes_pass_one_way_tiles_and_are_lost_in_water() {
    // One-way tiles over the whole arc: it flies through them and sticks
    // in the ground.
    let marks: Vec<(usize, usize, char)> = (3..20).map(|c| (9, c, '-')).collect();
    let mut g = as_hackl(game(&world(40, &marks)));
    hold(&mut g, "B", 1);
    until(&mut g, "-", 80, |g| axes(g)[0].state == axe::STUCK);
    assert_eq!(
        axes(&g)[0].y + axes(&g)[0].h,
        Fx::from_px(192),
        "in the ground"
    );
    // Water where it lands: removed, never stuck.
    let marks: Vec<(usize, usize, char)> =
        (6..14).flat_map(|c| [(12, c, '~'), (13, c, '~')]).collect();
    let mut g = as_hackl(game(&world(40, &marks)));
    hold(&mut g, "B", 1);
    for _ in 0..80 {
        g.step(input("-"));
        assert!(axes(&g).iter().all(|a| a.state == axe::FLY));
    }
    assert!(axes(&g).is_empty(), "lost in the water");
}

#[test]
fn axes_off_screen_are_removed() {
    let mut g = as_hackl(game(&world(60, &[])));
    let mut a = Entity::at_feet(
        EntityKind::ThrownHackl,
        Fx::from_px(330),
        Fx::from_px(100),
        [8, 8],
    );
    a.vx = Fx::from_px(3);
    g.entities.push(a);
    hold(&mut g, "-", 1);
    assert!(axes(&g).is_empty());
}

/// Throws one axe at `targets` (placed at their feet x) and runs 40 frames.
fn throw_at(targets: &[Entity]) -> (Game, Vec<Event>) {
    let mut g = as_hackl(game(&world(40, &[])));
    g.entities.extend_from_slice(targets);
    let mut events = hold(&mut g, "B", 1);
    events.extend(hold(&mut g, "-", 40));
    (g, events)
}

#[test]
fn stompable_enemies_are_defeated_for_their_base_points() {
    for (kind, points) in [
        (EnemyKind::SelfieTourist, 100),
        (EnemyKind::NordicWalker, 100),
        (EnemyKind::Taube, 100),
        (EnemyKind::MozartkugelVerkaeufer, 200),
        (EnemyKind::Blasmusiker, 200),
        (EnemyKind::EBiker, 200),
    ] {
        // About 6 tiles ahead, where the arc is back at hand height.
        let mut e = enemy(kind, 150);
        e.dir = 0;
        e.timer = 5;
        if kind == EnemyKind::Taube {
            e.y = Fx::from_px(168);
            e.base_y = e.y;
        }
        let (g, events) = throw_at(&[e]);
        assert!(events.contains(&Event::AxeHit), "{kind:?}: {events:?}");
        let d = g
            .entities()
            .iter()
            .find(|e| e.kind == EntityKind::Defeated(kind))
            .unwrap_or_else(|| panic!("{kind:?} defeated"));
        assert_eq!(d.state, crate::enemies::state::TUMBLE, "{kind:?} tumbles");
        assert_eq!(g.score(), points, "{kind:?}");
        assert!(axes(&g).is_empty(), "{kind:?}: the axe is gone");
        assert_eq!(g.player().stomp_chain, 0, "outside the stomp chain");
        assert!(popups(&g).contains(&Popup::Score(points)));
    }
}

#[test]
fn the_fiaker_is_immune() {
    let (g, events) = throw_at(&[enemy(EnemyKind::Fiaker, 150)]);
    assert!(events.contains(&Event::AxeHit));
    assert!(axes(&g).is_empty(), "bounced off");
    assert_eq!(g.score(), 0);
    assert!(
        g.entities()
            .iter()
            .any(|e| e.kind == EntityKind::Enemy(EnemyKind::Fiaker))
    );
}

#[test]
fn swarms_and_projectiles_let_it_pass() {
    // A note standing in the axe's path (vx 0) and a swarm around it.
    let t = tuning();
    let mut note = Entity::at_feet(
        EntityKind::Note,
        Fx::from_px(62),
        Fx::from_px(172),
        t.enemies.note_hitbox,
    );
    note.dir = -1;
    let mut g = as_hackl(game(&world(40, &[])));
    g.entities.push(note);
    hold(&mut g, "B", 1);
    let mut met = false;
    for _ in 0..12 {
        let events = g.step(input("-")).to_vec();
        assert!(!events.contains(&Event::AxeHit));
        let a = axes(&g)[0];
        met |= g
            .entities()
            .iter()
            .any(|e| e.kind == EntityKind::Note && e.hitbox().overlaps(&a.hitbox()));
    }
    assert!(met, "the axe flew through the note");
    assert_eq!(axes(&g)[0].state, axe::FLY);
    // The swarm: hitbox placed on the axe every frame.
    let mut g = as_hackl(game(&world(40, &[])));
    hold(&mut g, "B", 1);
    let a = axes(&g)[0];
    let mut s = Entity::new(
        EntityKind::Enemy(EnemyKind::Bienenschwarm),
        a.x,
        a.y,
        Fx::from_px(14),
        Fx::from_px(14),
    );
    s.base_x = a.x + Fx::from_px(3);
    s.base_y = a.y - Fx::from_px(3);
    g.entities.push(s);
    let events = hold(&mut g, "-", 1);
    assert!(!events.contains(&Event::AxeHit), "{events:?}");
    assert_eq!(axes(&g).len(), 1);
}

#[test]
fn one_target_per_axe() {
    let mut a = enemy(EnemyKind::SelfieTourist, 150);
    a.dir = 0;
    let b = a;
    let (g, _) = throw_at(&[a, b]);
    let defeated = g
        .entities()
        .iter()
        .filter(|e| e.kind == EntityKind::Defeated(EnemyKind::SelfieTourist))
        .count();
    assert_eq!(defeated, 1);
    assert_eq!(g.score(), 100);
}

#[test]
fn axes_do_not_touch_blocks_or_items() {
    let mut g = as_hackl(game(&world(40, &[(10, 4, 'H')])));
    let tiles = g.tile(4, 10);
    hold(&mut g, "B", 1);
    hold(&mut g, "-", 40);
    assert_eq!(g.tile(4, 10), tiles, "the box is untouched");
}

// ------------------------------------------------------- Durchkriechstein

/// The stone: tunnel row 11, columns 8–11, boulder rows 9–10; mouth at
/// x 128, exit end at x 192.
fn stone_world() -> Level {
    let mut marks = vec![(11, 8, 'K')];
    for c in 8..12 {
        marks.push((9, c, '#'));
        marks.push((10, c, '#'));
    }
    world(30, &marks)
}

/// Walks right into the boulder's face (stops at the mouth).
fn at_mouth(power: Power) -> Game {
    let t = tuning();
    let mut g = game(&stone_world());
    g.player.set_power(power, &t.player);
    g.player.transform = 0;
    hold(&mut g, "R", 80);
    assert_eq!(g.player().x + g.player().w, Fx::from_px(128), "at the face");
    hold(&mut g, "-", 30);
    g
}

#[test]
fn down_at_the_mouth_crawls_through_as_normal_for_a_life() {
    let t = tuning();
    let mut g = at_mouth(Power::Normal);
    let lives = g.lives();
    let time = g.time_left();
    let events = hold(&mut g, "D", 1);
    assert!(events.contains(&Event::CrawlStarted));
    let p = *g.player();
    assert!(p.crawling());
    assert_eq!(
        (p.w, p.h),
        (Fx::from_px(12), Fx::from_px(12)),
        "crawl hitbox"
    );
    assert_eq!(p.x, Fx::from_px(117), "flush at the mouth, then 1 px");
    assert_eq!(feet_y(&g), Fx::from_px(192));
    // Input is ignored: holding LEFT + JUMP still crawls right.
    let events = hold(&mut g, "LJ", 74);
    assert!(!events.contains(&Event::Jumped));
    assert!(g.player().crawling());
    assert_eq!(g.player().x, Fx::from_px(191));
    // Frame 76: out, standing, +1 life.
    let events = hold(&mut g, "-", 1);
    assert!(events.contains(&Event::ExtraLife), "{events:?}");
    let p = *g.player();
    assert!(!p.crawling());
    assert_eq!(p.x, Fx::from_px(192), "left edge at the tunnel's end");
    assert_eq!(
        (p.w, p.h),
        (Fx::from_px(12), Fx::from_px(t.player.hitbox_h))
    );
    assert_eq!(feet_y(&g), Fx::from_px(192));
    assert_eq!(g.lives(), lives + 1);
    assert!(popups(&g).contains(&Popup::ExtraLife));
    assert!(g.time_left() < time, "the timer ran");
    let s = g
        .entities()
        .iter()
        .find(|e| e.kind == EntityKind::Durchkriechstein)
        .unwrap();
    assert_eq!(s.state, stone::USED);
}

#[test]
fn a_normal_crawl_at_the_lives_cap_gives_1000_points() {
    let t = tuning();
    let mut g = at_mouth(Power::Normal);
    g.lives = t.game.lives_max;
    hold(&mut g, "D", 1);
    hold(&mut g, "-", 75);
    assert_eq!(g.lives(), t.game.lives_max);
    assert_eq!(g.score(), t.crawl.cap_points);
    assert!(popups(&g).contains(&Popup::Score(1000)));
}

#[test]
fn a_big_crawl_sheds_to_normal_with_ballast_weg() {
    let t = tuning();
    for power in [Power::Big, Power::Hackl] {
        let mut g = at_mouth(power);
        let lives = g.lives();
        assert!(hold(&mut g, "D", 1).contains(&Event::CrawlStarted));
        assert_eq!(g.player().h, Fx::from_px(12), "{power:?} squeezes in");
        hold(&mut g, "-", 75);
        let p = *g.player();
        assert_eq!((p.power, p.prev_power), (Power::Normal, power));
        assert_eq!(p.transform, t.player.transform_frames, "the freeze");
        assert_eq!(p.invulnerable, 0, "no i-frames");
        assert_eq!(feet_y(&g), Fx::from_px(192));
        assert_eq!(g.lives(), lives);
        assert_eq!(g.score(), 0, "no points");
        assert!(popups(&g).contains(&Popup::Ballast), "{power:?}");
    }
}

#[test]
fn nothing_hurts_while_crawling_and_enemies_keep_moving() {
    let mut g = at_mouth(Power::Normal);
    hold(&mut g, "D", 1);
    let mut e = enemy(EnemyKind::SelfieTourist, 140);
    e.dir = -1;
    g.entities.push(e);
    let x0 = e.x;
    let events = hold(&mut g, "-", 30);
    assert!(!events.contains(&Event::Died), "{events:?}");
    assert_eq!(g.mode(), Mode::Playing);
    let t = g
        .entities()
        .iter()
        .find(|e| e.kind == EntityKind::Enemy(EnemyKind::SelfieTourist))
        .unwrap();
    assert!(t.x < x0, "it walked");
}

#[test]
fn crawl_start_conditions() {
    let t = tuning();
    // DOWN + JUMP: a normal jump, no crawl.
    let mut g = at_mouth(Power::Normal);
    let events = hold(&mut g, "DJ", 1);
    assert!(!events.contains(&Event::CrawlStarted) && events.contains(&Event::Jumped));
    // In the air: no crawl.
    let mut g = at_mouth(Power::Normal);
    hold(&mut g, "J", 3);
    assert!(!hold(&mut g, "D", 5).contains(&Event::CrawlStarted));
    // Facing left: no crawl.
    let mut g = at_mouth(Power::Normal);
    g.player.facing = Facing::Left;
    assert!(!hold(&mut g, "D", 1).contains(&Event::CrawlStarted));
    // 4 px before the mouth: crawl (snapped flush); 5 px: none.
    for (gap, crawls) in [(4, true), (5, false)] {
        let mut g = at_mouth(Power::Normal);
        g.player.x -= Fx::from_px(gap);
        let events = hold(&mut g, "D", 1);
        assert_eq!(events.contains(&Event::CrawlStarted), crawls, "{gap} px");
    }
    // DOWN alone elsewhere does nothing.
    let mut g = game(&stone_world());
    assert!(!hold(&mut g, "D", 10).contains(&Event::CrawlStarted));
    let _ = t;
}

#[test]
fn the_stone_is_used_once_per_attempt_and_resets_on_death() {
    let mut g = at_mouth(Power::Normal);
    hold(&mut g, "D", 76);
    assert_eq!(g.lives(), 4);
    // Back at the mouth: the stone is used.
    g.player.x = Fx::from_px(116);
    assert!(!hold(&mut g, "D", 5).contains(&Event::CrawlStarted));
    // A death resets it (like the boxes).
    g.time = 1;
    until(&mut g, "-", 61, |g| g.events().contains(&Event::Died));
    until(&mut g, "-", 200, |g| g.mode() == Mode::Playing);
    assert_eq!(g.lives(), 3, "dying cost the life the stone gave");
    hold(&mut g, "R", 80);
    hold(&mut g, "-", 30);
    assert!(hold(&mut g, "D", 1).contains(&Event::CrawlStarted));
}

#[test]
fn no_throw_while_crawling() {
    let mut g = at_mouth(Power::Hackl);
    hold(&mut g, "D", 1);
    for _ in 0..20 {
        assert!(!hold(&mut g, "B", 1).contains(&Event::HacklThrown));
        hold(&mut g, "-", 1);
    }
    assert!(axes(&g).is_empty());
}

#[test]
fn big_resi_at_the_mouth_without_down_is_never_stuck() {
    for power in [Power::Big, Power::Hackl] {
        let mut g = at_mouth(power);
        // Holding against the face: a wall.
        hold(&mut g, "R", 60);
        assert_eq!(g.player().x + g.player().w, Fx::from_px(128));
        assert!(!g.player().crawling());
        // Walks back, then climbs over the 3-tile boulder.
        hold(&mut g, "L", 20);
        hold(&mut g, "-", 30);
        assert!(g.player().x + g.player().w < Fx::from_px(120));
        until(&mut g, "RJ", 120, |g| g.player().x > Fx::from_px(192));
        assert_eq!(g.player().power, power, "{power:?} kept");
    }
}
