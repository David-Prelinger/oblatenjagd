//! The Kellner and the Wartende of Stage 9 (game-design.md → "Stage 9:
//! Café Zauner", entities and "New mechanics"; D-105): a fast walker with
//! the Nordic-Walker's turn rules whose tray is a bounce pad („Komm'
//! gleich!", no damage, no points) and whose every other contact hurts; and
//! the queue's Wartende, Selfie-Tourists that never walk, placed under their
//! own name.

use super::*;
use crate::enemies::{Pose, state};
use crate::hackl::AxeHit;
use crate::level::SpawnKind;
use crate::player::Power;
use crate::{EnemyKind, Entity, EntityKind, Fx, Popup};

/// Legend of the test worlds (`.` air, `P` start).
const LEGEND: &str = r##"
"#" = "solid"
"B" = "brick"
"K" = { entity = "kellner" }
"W" = { entity = "wartender" }
"w" = { entity = "nordic_walker" }
"P" = { entity = "player_start" }
"G" = { entity = "goal" }
"##;

const KELLNER: EntityKind = EntityKind::Enemy(EnemyKind::Kellner);
/// A Wartender in the simulation (D-105).
const TOURIST: EntityKind = EntityKind::Enemy(EnemyKind::SelfieTourist);

/// The floor's top (row 12), px.
const FLOOR: i32 = 192;

/// A 14-row world `width` columns wide: solid floor in rows 12–13 (top y
/// 192), the start at column 2 unless `marks` place a `P`; `marks` are
/// (row, col, char) in [`LEGEND`], a `.` digs a hole.
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
        "[meta]\nid = \"cafe\"\ntime_limit = 300\n[legend]\n{LEGEND}\n[map]\ngrid = '''\n{}\n'''\n",
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

fn index(g: &Game, kind: EntityKind) -> usize {
    g.entities().iter().position(|e| e.kind == kind).unwrap()
}

fn popups(g: &Game, p: Popup) -> Vec<Entity> {
    all(g, EntityKind::Popup(p))
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
    g.player.coyote = 0;
}

/// `g` with the player in `power` at once (no freeze).
fn with_power(mut g: Game, power: Power) -> Game {
    g.player.set_power(power, &tuning().player);
    g.player.transform = 0;
    g
}

fn centre(e: &Entity) -> Fx {
    e.x + e.w.div_int(2)
}

/// A Kellner at column 20, active (one step taken), with the player
/// dropped onto his tray and the bounce frame stepped with `token`.
/// Returns the game and the bounce frame's events.
fn bounced(token: &str) -> (Game, Vec<Event>) {
    let mut g = game(&world(40, &[(11, 20, 'K')]));
    safe_step(&mut g);
    let k = first(&g, KELLNER);
    drop_onto(&mut g, k);
    let events = g.step(input(token)).to_vec();
    (g, events)
}

// ------------------------------------------------------------- Kellner

#[test]
fn a_kellner_spawns_12_by_26_on_his_cell_floor_and_classifies() {
    let g = game(&world(40, &[(11, 20, 'K')]));
    let k = first(&g, KELLNER);
    assert!(!k.active);
    assert_eq!((k.w, k.h), (Fx::from_px(12), Fx::from_px(26)));
    assert_eq!(k.feet(), (Fx::from_px(20 * 16 + 8), Fx::from_px(FLOOR)));
    assert_eq!(k.state, state::WALK);
    assert!(!EnemyKind::Kellner.stompable(), "the tray");
    assert!(!EnemyKind::Kellner.hurts_from_above());
    assert_eq!(EnemyKind::Kellner.chain_start(), 0, "100 (bump, D-105)");
    assert!(EnemyKind::Kellner.sinks_in_swim_water(), "on foot");
    assert_eq!(EnemyKind::Kellner.axe_hit(), AxeHit::Bounce, "the tray");
    assert_eq!(EnemyKind::Kellner.sprite(), "kellner");
    assert_eq!(Popup::KommGleich.text_key(), Some("popup.komm_gleich"));
}

#[test]
fn he_walks_1_px_per_frame_towards_the_player_and_turns_at_walls() {
    // The player at column 2 (left); a wall at column 14.
    let mut g = game(&world(40, &[(11, 20, 'K'), (11, 14, '#'), (10, 14, '#')]));
    safe_step(&mut g);
    let k0 = first(&g, KELLNER);
    assert_eq!(k0.dir, -1, "towards the player");
    for _ in 0..16 {
        safe_step(&mut g);
    }
    let k = first(&g, KELLNER);
    assert_eq!(k0.x - k.x, Fx::from_px(16), "1.0 px/frame");
    assert_eq!(k.pose(&tuning()), Some(Pose::Walk));
    // At the wall (its right face at x 240) he turns and walks back; he
    // never stops (only the frame he meets the wall is spent turning).
    let mut turned = false;
    let (mut x, mut dir) = (k.x, k.dir);
    for _ in 0..300 {
        safe_step(&mut g);
        let k = first(&g, KELLNER);
        assert!(k.x >= Fx::from_px(240), "never in the wall");
        assert!(k.x != x || k.dir != dir, "never stops");
        (x, dir) = (k.x, k.dir);
        turned |= k.dir == 1;
    }
    assert!(turned);
}

#[test]
fn he_turns_at_ledges_and_never_drops_his_tray() {
    // A hole at column 16: his ledge is column 17's left edge (x 272).
    let mut g = game(&world(40, &[(11, 20, 'K'), (12, 16, '.'), (13, 16, '.')]));
    let mut turned = false;
    for _ in 0..300 {
        safe_step(&mut g);
        let k = first(&g, KELLNER);
        assert!(k.x >= Fx::from_px(272), "x {:?}", k.x);
        assert_eq!(k.feet().1, Fx::from_px(FLOOR), "on the floor");
        turned |= k.dir == 1;
    }
    assert!(turned);
}

#[test]
fn he_turns_at_a_wartender_and_a_walker_turns_at_him() {
    // A Wartender at 16 to his left; a Nordic-Walker at 28 to his right.
    let mut g = game(&world(40, &[(11, 20, 'K'), (11, 16, 'W'), (11, 28, 'w')]));
    let w = first(&g, TOURIST);
    let mut turned = false;
    let mut walker_turned = false;
    for _ in 0..400 {
        safe_step(&mut g);
        let k = first(&g, KELLNER);
        let walker = first(&g, EntityKind::Enemy(EnemyKind::NordicWalker));
        assert!(k.x > w.x, "never walks through the queue");
        assert!(walker.x > k.x, "the walker never passes him");
        turned |= k.dir == 1;
        walker_turned |= walker.dir == 1;
    }
    assert!(turned && walker_turned);
}

#[test]
fn the_tray_bounces_her_without_damage_or_points_with_komm_gleich() {
    let t = tuning();
    let (g, events) = bounced("-");
    assert!(events.contains(&Event::Stomped), "the bounce: {events:?}");
    assert!(!events.contains(&Event::Hurt));
    let k = first(&g, KELLNER);
    assert!(k.active && !k.removed, "he walks on");
    assert_eq!(g.score(), 0, "no points");
    assert_eq!(g.player().stomp_chain, 0, "no chain step");
    assert_eq!(g.player().power, Power::Normal);
    // A normal stomp bounce, her feet put on the tray.
    assert_eq!(g.player().vy, -t.player.stomp_bounce);
    assert_eq!(g.player().y + g.player().h, k.y);
    // „Komm' gleich!“ centred on him, where „Bravo!“ stands over a score
    // popup.
    let p = popups(&g, Popup::KommGleich);
    assert_eq!(p.len(), 1);
    assert_eq!(centre(&p[0]), centre(&k));
    assert_eq!(
        k.y - p[0].y,
        Fx::from_px(crate::entity::POPUP_STACK_PX),
        "stacked above the tray"
    );
    assert!(
        !g.entities()
            .iter()
            .any(|e| matches!(e.kind, EntityKind::Popup(Popup::Score(_)))),
        "no score popup"
    );
}

#[test]
fn jump_held_gives_the_full_stomp_bounce_and_the_chain_is_kept() {
    let t = tuning();
    let mut g = game(&world(40, &[(11, 20, 'K')]));
    safe_step(&mut g);
    g.player.stomp_chain = 3;
    let k = first(&g, KELLNER);
    drop_onto(&mut g, k);
    let events = g.step(input("J")).to_vec();
    assert!(events.contains(&Event::Stomped));
    assert_eq!(g.player().vy, -t.player.stomp_bounce_held, "~3 tiles");
    assert_eq!(g.player().stomp_chain, 3, "neither advances nor resets");
    assert_eq!(g.score(), 0);
}

#[test]
fn every_bounce_shows_komm_gleich_and_the_bounce_pose_for_8_frames() {
    let t = tuning();
    assert_eq!(t.enemies.kellner_bounce_pose, 8);
    let (mut g, _) = bounced("-");
    // Frame 1 of 8 is the bounce frame itself.
    for frame in 1..=8 {
        let k = first(&g, KELLNER);
        assert_eq!(k.state, state::TRAY_BOUNCE, "frame {frame}");
        assert_eq!(k.pose(&t), Some(Pose::Bounce), "frame {frame}");
        safe_step(&mut g);
    }
    let k = first(&g, KELLNER);
    assert_eq!((k.state, k.count), (state::WALK, 0));
    assert_eq!(k.pose(&t), Some(Pose::Walk));
    // A second bounce: a second popup (the first one has floated away by
    // then or not: each bounce shows one).
    let before = popups(&g, Popup::KommGleich).len();
    drop_onto(&mut g, k);
    let events = g.step(input("-")).to_vec();
    assert!(events.contains(&Event::Stomped));
    assert_eq!(popups(&g, Popup::KommGleich).len(), before + 1);
    assert_eq!(first(&g, KELLNER).state, state::TRAY_BOUNCE);
    assert_eq!(g.score(), 0);
}

#[test]
fn the_bounce_pose_changes_nothing_but_his_looks() {
    let (mut a, _) = bounced("-");
    let mut b = a.clone();
    let i = index(&b, KELLNER);
    b.entities[i].state = state::WALK;
    b.entities[i].count = 0;
    for _ in 0..30 {
        safe_step(&mut a);
        safe_step(&mut b);
        let (ka, kb) = (first(&a, KELLNER), first(&b, KELLNER));
        assert_eq!((ka.x, ka.y, ka.dir), (kb.x, kb.y, kb.dir));
    }
}

#[test]
fn his_side_hurts() {
    // Zuckerschock walks into him: hurt, Normal, no points, he walks on.
    let mut g = with_power(game(&world(40, &[(11, 8, 'K')])), Power::Big);
    let events = until(&mut g, "R", 120, |g| g.player().power == Power::Normal);
    assert!(events.contains(&Event::Hurt), "{events:?}");
    assert!(!events.contains(&Event::Stomped));
    assert_eq!(g.score(), 0);
    assert!(!all(&g, KELLNER).is_empty(), "he stays");
    // Normal: a side hit is a death.
    let mut g = game(&world(40, &[(11, 8, 'K')]));
    let events = until(&mut g, "R", 120, |g| g.deaths() > 0);
    assert!(events.contains(&Event::Died), "{events:?}");
}

#[test]
fn rising_into_his_tray_hurts() {
    let mut g = with_power(game(&world(40, &[(11, 20, 'K')])), Power::Big);
    safe_step(&mut g);
    let k = first(&g, KELLNER);
    // Jumping up into his tray from beside him (rising: no bounce), her
    // hitbox 2 px into his.
    g.player.x = k.x - g.player.w + Fx::from_px(2);
    g.player.y = k.y - Fx::from_px(8);
    g.player.vy = -Fx::from_px(2);
    g.player.on_ground = false;
    g.player.invulnerable = 0;
    let events = g.step(input("-")).to_vec();
    assert!(events.contains(&Event::Hurt), "{events:?}");
    assert!(!events.contains(&Event::Stomped));
}

#[test]
fn a_bumped_block_knocks_him_off_for_100() {
    // Bricks at row 8, columns 1–3, the Kellner on them at column 2; the
    // player jumps from column 2 into the brick above.
    let mut g = game(&world(
        20,
        &[(8, 1, 'B'), (8, 2, 'B'), (8, 3, 'B'), (7, 2, 'K')],
    ));
    safe_step(&mut g);
    let events = until(&mut g, "J", 40, |g| {
        g.events().contains(&Event::BlockBumped)
    });
    assert!(!events.contains(&Event::Died));
    let d = first(&g, EntityKind::Defeated(EnemyKind::Kellner));
    assert_eq!(d.state, state::TUMBLE);
    assert_eq!(d.pose(&tuning()), Some(Pose::Squished));
    assert_eq!(g.score(), tuning().score.bump_enemy);
    assert_eq!(popups(&g, Popup::Score(100)).len(), 1);
    assert!(popups(&g, Popup::KommGleich).is_empty(), "no tray bounce");
}

#[test]
fn the_hackl_bounces_off_his_tray() {
    let t = tuning();
    let mut k = first(&game(&world(40, &[(11, 20, 'K')])), KELLNER);
    assert_eq!(k.axe_hit(), AxeHit::Bounce);
    k.state = state::TRAY_BOUNCE;
    assert_eq!(k.axe_hit(), AxeHit::Bounce, "in his bounce pose too");
    k.active = true;
    let before = k;
    assert_eq!(crate::enemies::hit_by_projectile(&mut k, &t), None);
    assert_eq!(k, before, "nothing happens to him");
    // In play: an axe thrown at him as he comes closer is gone, he walks
    // on, no points.
    let mut g = with_power(game(&world(40, &[(11, 10, 'K')])), Power::Hackl);
    safe_step(&mut g);
    g.player.facing = crate::player::Facing::Right;
    let mut events = hold(&mut g, "B", 1);
    for _ in 0..30 {
        events.extend(safe_step(&mut g));
    }
    assert!(events.contains(&Event::HacklThrown), "{events:?}");
    assert!(events.contains(&Event::AxeHit), "{events:?}");
    assert!(
        !g.entities()
            .iter()
            .any(|e| e.kind == EntityKind::ThrownHackl),
        "bounced off"
    );
    assert_eq!(all(&g, KELLNER).len(), 1);
    assert_eq!(g.score(), 0);
}

#[test]
fn world_hash_covers_the_bounce_pose() {
    let (mut g, _) = bounced("-");
    let i = index(&g, KELLNER);
    let before = g.world_hash();
    g.entities[i].count += 1;
    assert_ne!(g.world_hash(), before, "the pose counter");
    let before = g.world_hash();
    g.entities[i].state = state::WALK;
    assert_ne!(g.world_hash(), before, "the state");
}

// ------------------------------------------------------------ Wartende

#[test]
fn a_wartender_is_a_still_selfie_tourist_under_his_own_name() {
    let t = tuning();
    let level = world(40, &[(11, 20, 'W')]);
    let spawn = level
        .entities
        .iter()
        .find(|s| s.kind == SpawnKind::Wartender)
        .unwrap();
    assert!(!spawn.still, "no `still` field: the name says it");
    let g = game(&level);
    let w = first(&g, TOURIST);
    assert_eq!(w.dir, 0, "no direction: never walks");
    assert!(!w.facing_left(), "faces right, towards the counter");
    let [hw, hh] = t.enemies.selfie_tourist_hitbox;
    assert_eq!((w.w, w.h), (Fx::from_px(hw), Fx::from_px(hh)));
    assert_eq!(w.feet(), (Fx::from_px(20 * 16 + 8), Fx::from_px(FLOOR)));
    assert_eq!(
        g.level().entities[usize::from(w.spawn.unwrap())].kind,
        SpawnKind::Wartender
    );
}

#[test]
fn wartende_never_move_always_face_right_and_take_harmless_selfies() {
    let t = tuning();
    // The player on either side, 2 px clear of his hitbox, through two
    // selfie periods: he never moves or turns, poses for his selfies on
    // the Selfie-Tourist schedule, and never hurts her.
    for side in [-1, 1] {
        let mut g = game(&world(40, &[(11, 20, 'W')]));
        let w0 = first(&g, TOURIST);
        let mut events = Vec::new();
        let mut flashed = false;
        for _ in 0..2 * u32::from(t.enemies.tourist_selfie_period) {
            let px = if side < 0 {
                w0.x - Fx::from_px(2) - g.player.w
            } else {
                w0.right() + Fx::from_px(2)
            };
            g.player.x = px;
            g.player.vx = Fx::ZERO;
            events.extend(g.step(input("-")).to_vec());
            let w = first(&g, TOURIST);
            assert_eq!((w.x, w.y, w.dir), (w0.x, w0.y, 0), "side {side}");
            assert!(!w.facing_left());
            flashed |= w.pose(&t) == Some(Pose::Selfie { flash: true });
        }
        assert!(flashed, "the selfie with its flash");
        assert!(!events.contains(&Event::Hurt), "side {side}: {events:?}");
    }
}

#[test]
fn a_chain_over_the_queue_scores_100_200_400() {
    let mut g = game(&world(40, &[(11, 12, 'W'), (11, 15, 'W'), (11, 18, 'W')]));
    safe_step(&mut g);
    let mut events = Vec::new();
    for _ in 0..3 {
        let w = first(&g, TOURIST);
        drop_onto(&mut g, w);
        events.extend(g.step(input("-")).to_vec());
    }
    assert_eq!(events.iter().filter(|&&e| e == Event::Stomped).count(), 3);
    assert!(!events.contains(&Event::Hurt));
    assert_eq!(g.score(), 100 + 200 + 400);
    assert_eq!(
        all(&g, EntityKind::Defeated(EnemyKind::SelfieTourist)).len(),
        3
    );
}

#[test]
fn a_wartenders_side_hurts() {
    let mut g = with_power(game(&world(40, &[(11, 8, 'W')])), Power::Big);
    let events = until(&mut g, "R", 120, |g| g.player().power == Power::Normal);
    assert!(events.contains(&Event::Hurt), "{events:?}");
    assert_eq!(g.score(), 0);
}

#[test]
fn an_axe_defeats_a_wartender_for_100() {
    assert_eq!(EnemyKind::SelfieTourist.axe_hit(), AxeHit::Defeat);
    let mut g = with_power(game(&world(40, &[(11, 8, 'W')])), Power::Hackl);
    safe_step(&mut g);
    g.player.facing = crate::player::Facing::Right;
    let mut events = hold(&mut g, "B", 1);
    for _ in 0..40 {
        events.extend(safe_step(&mut g));
    }
    assert!(events.contains(&Event::AxeHit), "{events:?}");
    let d = first(&g, EntityKind::Defeated(EnemyKind::SelfieTourist));
    assert_eq!(d.state, state::TUMBLE);
    assert_eq!(g.score(), 100);
}

#[test]
fn a_bumped_block_knocks_a_wartender_off() {
    let mut g = game(&world(
        20,
        &[(8, 1, 'B'), (8, 2, 'B'), (8, 3, 'B'), (7, 2, 'W')],
    ));
    let events = until(&mut g, "J", 40, |g| {
        g.events().contains(&Event::BlockBumped)
    });
    assert!(!events.contains(&Event::Died));
    let d = first(&g, EntityKind::Defeated(EnemyKind::SelfieTourist));
    assert_eq!(d.state, state::TUMBLE);
    assert_eq!(g.score(), tuning().score.bump_enemy);
}
