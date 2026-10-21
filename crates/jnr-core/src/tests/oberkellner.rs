//! The Oberkellner, the boss of Stage 9, and his bills (game-design.md →
//! "Stage 9: Café Zauner" → "The Oberkellner", D-106): the arena lock, his
//! machine and its numbers by round, the dash's far wall and its exact
//! stop, contact and axes by state, the points, the bills, the reset on a
//! respawn, the query for the finale, the arena lint and the measured
//! windows of the Normal-form win (D-100 method).

use super::*;
use crate::enemies::state::{oberkellner as st, rechnung};
use crate::enemies::{
    BILL_COLUMNS, BILL_ROW, Pose, arena_problems, boss_hits, boss_undefeated, sine_offset,
};
use crate::hackl::{AxeHit, axe};
use crate::player::Power;
use crate::{EnemyKind, Entity, EntityKind, Fx, Mode, Popup, Replay};

/// The grey box: the arena alone (columns 20–39), floor top y 240, the
/// left wall face x 352, the right wall face x 592, the boss at x 544.
fn arena() -> Level {
    Level::parse(include_str!("../../../../content/levels/00_m7_boss.level")).unwrap()
}

fn game() -> Game {
    Game::new(&arena(), &tuning(), 1)
}

const BOSS: EntityKind = EntityKind::Enemy(EnemyKind::Oberkellner);
/// The camera's right clamp in the grey box (40 × 16 − 320).
const MAX_CAMERA_X: i32 = 320;
/// The boss's x flush at the left wall face (x 352) and at the right one
/// (592 − 16).
const LEFT_WALL_X: i32 = 352;
const RIGHT_WALL_X: i32 = 576;
/// The floor's top.
const FLOOR_Y: i32 = 240;

fn boss_index(g: &Game) -> usize {
    g.entities
        .iter()
        .position(|e| e.kind == BOSS)
        .expect("an Oberkellner")
}

fn boss(g: &Game) -> Entity {
    g.entities[boss_index(g)]
}

fn boss_mut(g: &mut Game) -> &mut Entity {
    let i = boss_index(g);
    &mut g.entities[i]
}

fn bills(g: &Game) -> Vec<Entity> {
    g.entities
        .iter()
        .filter(|e| e.kind == EntityKind::Rechnung)
        .copied()
        .collect()
}

fn has_popup(g: &Game, p: Popup) -> bool {
    g.entities.iter().any(|e| e.kind == EntityKind::Popup(p))
}

/// Steps once with `token`, the player kept invulnerable and out of the
/// way (on the left refuge, x 330).
fn tick(g: &mut Game, token: &str) -> Vec<Event> {
    g.player.invulnerable = u8::MAX;
    g.step(input(token)).to_vec()
}

/// The player standing on the left refuge (top y 192) at `x`.
fn on_refuge(g: &mut Game, x: i32) {
    g.player.x = Fx::from_px(x);
    g.player.y = Fx::from_px(192) - g.player.h;
    g.player.vx = Fx::ZERO;
    g.player.vy = Fx::ZERO;
    g.player.on_ground = true;
}

/// A game whose arena is locked: the camera at its right clamp, the
/// player on the left refuge, one frame stepped (the boss's intro frame 1).
fn locked() -> Game {
    let mut g = game();
    g.camera.x = MAX_CAMERA_X;
    on_refuge(&mut g, 330);
    tick(&mut g, "-");
    assert_eq!(boss(&g).state, st::INTRO, "locked");
    assert_eq!(boss(&g).count, 1);
    g
}

/// Steps (player safe on the refuge) until the boss is in `state`.
fn until_boss(g: &mut Game, state: u8, max: u32) -> u32 {
    for n in 1..=max {
        on_refuge(g, 330);
        tick(g, "-");
        if boss(g).state == state {
            return n;
        }
    }
    panic!(
        "state {state} not reached in {max} frames (now {} c{})",
        boss(g).state,
        boss(g).count
    );
}

/// Frames the boss stays in his current state (player safe).
fn frames_in_state(g: &mut Game) -> u32 {
    let s = boss(g).state;
    let mut n = 1;
    loop {
        on_refuge(g, 330);
        tick(g, "-");
        if boss(g).state != s {
            return n;
        }
        n += 1;
    }
}

/// Puts the player in the air above the middle of `e`, falling at 3 px per
/// frame with the feet `above` px above its top.
fn drop_onto(g: &mut Game, e: Entity, above: i32) {
    g.player.x = e.x + e.w.div_int(2) - g.player.w.div_int(2);
    g.player.y = e.y - Fx::from_px(above) - g.player.h;
    g.player.vx = Fx::ZERO;
    g.player.vy = Fx::from_px(3);
    g.player.on_ground = false;
}

/// The player standing on the floor right beside `e` (touching its right
/// side after one frame).
fn beside(g: &mut Game, e: Entity) {
    g.player.x = e.right() - Fx::from_px(2);
    g.player.y = Fx::from_px(FLOOR_Y) - g.player.h;
    g.player.vx = Fx::ZERO;
    g.player.vy = Fx::ZERO;
    g.player.on_ground = true;
}

// ------------------------------------------------------------- the lock

#[test]
fn he_waits_until_the_first_frame_that_starts_with_the_camera_at_its_right_clamp() {
    let mut g = game();
    let mut events = Vec::new();
    // Walk right (onto the step and the refuge as in the replays) until the
    // camera reaches its clamp at the end of a frame.
    for token in ["R"; 155].into_iter().chain(["RJ"]).chain(["R"; 40]) {
        events.extend_from_slice(g.step(input(token)));
    }
    for _ in 0..20 {
        events.extend_from_slice(g.step(input("RJ")));
    }
    let mut reached = None;
    for _ in 0..200 {
        let before = g.camera().x;
        events.extend_from_slice(g.step(input("R")));
        let b = boss(&g);
        if g.camera().x < MAX_CAMERA_X {
            assert_eq!(b.state, st::WAIT, "no lock before the clamp");
            assert!(!has_popup(&g, Popup::Anstellen));
        }
        if before < MAX_CAMERA_X && g.camera().x == MAX_CAMERA_X {
            // The frame the camera arrives: he is active, still waiting
            // (the lock is the next frame, which starts at the clamp).
            assert!(b.active, "active one screen ahead");
            assert_eq!(b.state, st::WAIT);
            reached = Some(g.frame());
            break;
        }
    }
    let reached = reached.expect("the camera reached its clamp");
    g.step(input("-"));
    let b = boss(&g);
    assert_eq!((b.state, b.count), (st::INTRO, 1), "frame {}", reached + 1);
    assert!(has_popup(&g, Popup::Anstellen), "„Hinten anstellen!“");
    assert!(b.dir < 0, "he turns to her (she is left of him)");
    assert_eq!(b.pose(&tuning()), Some(Pose::Anstellen));
    assert!(!events.contains(&Event::Hurt));
}

#[test]
fn the_lock_also_comes_with_her_in_the_air_and_closes_the_arena_behind_her() {
    let mut g = game();
    // In the air, mid-jump, as the camera stands at its clamp.
    g.camera.x = MAX_CAMERA_X;
    g.player.x = Fx::from_px(400);
    g.player.y = Fx::from_px(150);
    g.player.vy = Fx::from_px(-2);
    g.player.on_ground = false;
    g.step(input("-"));
    assert_eq!(boss(&g).state, st::INTRO, "locked in the air");
    // Walking left on the refuge she stops at the screen's left edge, the
    // arena's left edge; the camera never moves (x at its clamp, y at the
    // café's camera_top 92).
    on_refuge(&mut g, 330);
    for _ in 0..120 {
        g.player.invulnerable = u8::MAX;
        g.step(input("L"));
        assert_eq!(g.camera().x, MAX_CAMERA_X);
        assert_eq!(g.camera().y, 92);
        assert!(g.player.x >= Fx::from_px(MAX_CAMERA_X));
    }
    assert_eq!(g.player.x, Fx::from_px(MAX_CAMERA_X), "pinned at the edge");
}

#[test]
fn the_intro_counts_frames_of_play_only_and_is_harmless() {
    let t = tuning();
    let mut g = locked();
    // A power-up freeze stops everything, his counter included.
    g.player.transform = 20;
    for _ in 0..20 {
        g.step(input("-"));
    }
    assert_eq!(boss(&g).count, 1, "frozen frames don't count");
    // She can stand inside him: no contact in the intro.
    let b = boss(&g);
    g.player.invulnerable = 0;
    beside(&mut g, b);
    g.player.x = b.x;
    let events = g.step(input("-")).to_vec();
    assert!(!events.contains(&Event::Hurt) && !events.contains(&Event::Died));
    assert_eq!(g.player.power, Power::Normal);
    assert_eq!(boss(&g).count, 2);
    // 90 frames in all (count 1..=90), then the telegraph.
    let rest = frames_in_state(&mut g);
    assert_eq!(rest + 2, u32::from(t.enemies.oberkellner_intro) + 1);
    assert_eq!(boss(&g).state, st::TELEGRAPH);
}

// ------------------------------------------------------- rounds and dash

#[test]
fn every_round_has_its_telegraph_dash_and_breather() {
    let t = tuning();
    let e = &t.enemies;
    for hits in 0..3u8 {
        let mut g = locked();
        boss_mut(&mut g).hits = hits;
        until_boss(&mut g, st::TELEGRAPH, 200);
        let r = usize::from(hits);
        assert_eq!(
            frames_in_state(&mut g),
            u32::from(e.oberkellner_telegraph[r])
        );
        assert_eq!(boss(&g).state, st::DASH);
        assert_eq!(boss(&g).vx, -e.oberkellner_dash[r], "round {r} speed");
        assert_eq!(boss(&g).x, Fx::from_px(544) - e.oberkellner_dash[r]);
        until_boss(&mut g, st::BREATHE, 200);
        assert_eq!(frames_in_state(&mut g), u32::from(e.oberkellner_breathe[r]));
        // Round 1 repeats its telegraph; rounds 2 and 3 throw a volley.
        let next = if hits == 0 { st::TELEGRAPH } else { st::VOLLEY };
        assert_eq!(boss(&g).state, next, "round {r}");
    }
    // Across the floor (224 px) the dashes take 112 / 90 / 75 frames.
    for (r, frames) in [(0usize, 112u32), (1, 90), (2, 75)] {
        let speed = e.oberkellner_dash[r].raw();
        assert_eq!((224 * 256 + speed - 1) / speed, frames as i32);
    }
}

#[test]
fn the_dash_goes_to_the_far_wall_stops_at_its_face_and_alternates() {
    let t = tuning();
    let mut g = locked();
    // From his spawn (x 544: 2 columns from the right wall) the far wall is
    // the left one.
    until_boss(&mut g, st::TELEGRAPH, 200);
    assert!(boss(&g).dir < 0, "faces the far (left) wall");
    until_boss(&mut g, st::BREATHE, 300);
    let b = boss(&g);
    assert_eq!(b.x, Fx::from_px(LEFT_WALL_X), "flush with the wall face");
    assert_eq!(b.dir, -1);
    // The next dash goes right and stops flush with the counter's face.
    until_boss(&mut g, st::TELEGRAPH, 200);
    assert!(boss(&g).dir > 0, "faces the far (right) wall");
    until_boss(&mut g, st::BREATHE, 300);
    assert_eq!(boss(&g).x, Fx::from_px(RIGHT_WALL_X));
    until_boss(&mut g, st::TELEGRAPH, 200);
    assert!(boss(&g).dir < 0, "and back");
    // At 2.5 px/frame the 224 px are no multiple of the speed: the last
    // step is cut at the face, never past it.
    let mut g = locked();
    boss_mut(&mut g).hits = 1;
    until_boss(&mut g, st::DASH, 300);
    let mut last = boss(&g).x;
    while boss(&g).state == st::DASH {
        on_refuge(&mut g, 330);
        tick(&mut g, "-");
        let x = boss(&g).x;
        assert!(x >= Fx::from_px(LEFT_WALL_X), "never past the face");
        assert!(last - x <= t.enemies.oberkellner_dash[1]);
        last = x;
    }
    assert_eq!(boss(&g).x, Fx::from_px(LEFT_WALL_X));
}

#[test]
fn the_breather_hitbox_is_16_by_30_with_the_feet_kept() {
    let mut g = locked();
    let feet = |e: &Entity| (e.x + e.w.div_int(2), e.y + e.h);
    let b = boss(&g);
    assert_eq!((b.w.to_px(), b.h.to_px()), (16, 36));
    until_boss(&mut g, st::BREATHE, 400);
    let b = boss(&g);
    assert_eq!((b.w.to_px(), b.h.to_px()), (16, 30));
    assert_eq!(
        feet(&b),
        (Fx::from_px(LEFT_WALL_X + 8), Fx::from_px(FLOOR_Y))
    );
    assert_eq!(b.prev_y, b.y, "prev_y moved with the top");
    assert_eq!(b.pose(&tuning()), Some(Pose::Verschnaufen));
    until_boss(&mut g, st::TELEGRAPH, 200);
    let b = boss(&g);
    assert_eq!((b.w.to_px(), b.h.to_px()), (16, 36));
    assert_eq!(
        feet(&b),
        (Fx::from_px(LEFT_WALL_X + 8), Fx::from_px(FLOOR_Y))
    );
}

// ------------------------------------------------------------- contact

#[test]
fn there_is_no_contact_in_wait_intro_stun_and_done() {
    for state in [st::WAIT, st::INTRO, st::STUN, st::DONE] {
        let mut g = game();
        g.camera.x = 0;
        let b = boss_mut(&mut g);
        b.state = state;
        b.active = true;
        let b = boss(&g);
        assert!(!b.harmful(), "{state}");
        assert_eq!(b.axe_hit(), AxeHit::Pass, "{state}");
        // Side contact: nothing.
        beside(&mut g, b);
        g.player.x = b.x + Fx::from_px(2);
        let events = g.step(input("-")).to_vec();
        assert!(!events.contains(&Event::Died), "{state}: {events:?}");
        // From above: she passes through, no bounce, no hit.
        let mut g2 = game();
        let b2 = boss_mut(&mut g2);
        b2.state = state;
        b2.active = true;
        let b2 = boss(&g2);
        drop_onto(&mut g2, b2, 2);
        let events = g2.step(input("-")).to_vec();
        assert!(!events.contains(&Event::Stomped), "{state}");
        assert_eq!(boss(&g2).hits, 0);
    }
}

#[test]
fn the_raised_tray_bounces_her_without_damage_or_points_and_its_side_hurts() {
    for state in [st::TELEGRAPH, st::DASH, st::VOLLEY] {
        let mut g = locked();
        until_boss(&mut g, st::TELEGRAPH, 200);
        boss_mut(&mut g).state = state;
        let b = boss(&g);
        assert!(b.harmful());
        assert_eq!(
            b.axe_hit(),
            AxeHit::Bounce,
            "{state}: the tray deflects an axe"
        );
        g.player.stomp_chain = 2;
        drop_onto(&mut g, b, 2);
        g.player.invulnerable = 0;
        let events = g.step(input("-")).to_vec();
        assert!(events.contains(&Event::Stomped), "{state}: a bounce");
        assert!(!events.contains(&Event::Hurt) && !events.contains(&Event::Died));
        assert_eq!(g.score(), 0, "{state}: no points");
        assert_eq!(boss(&g).hits, 0);
        assert_eq!(
            g.player.stomp_chain, 2,
            "the chain neither advances nor resets"
        );
        assert!(g.player.vy < Fx::ZERO, "she bounces");
        // Her feet on the tray (the Fiaker's rule, D-027).
        assert_eq!(g.player.y + g.player.h, boss(&g).y);
        // From the side: hurt (Big → Normal).
        let mut g = locked();
        until_boss(&mut g, st::TELEGRAPH, 200);
        boss_mut(&mut g).state = state;
        let b = boss(&g);
        g.set_player_power(Power::Big);
        g.player.transform = 0;
        g.player.invulnerable = 0;
        beside(&mut g, b);
        let events = g.step(input("L")).to_vec();
        assert!(events.contains(&Event::Hurt), "{state}: side hurt");
        assert_eq!(g.player.power, Power::Normal);
    }
}

/// E51: a hit's labels hang beside his head on the room's side, the text
/// starting just below her feet as she bounces off, the score under it;
/// at both walls (the breathers alternate: left, right, left), for all
/// three hits. That they keep clear of his sprite is checked with the real
/// font in `jnr-render` (`boss_hit_labels_clear_his_sprite`).
#[test]
fn the_hit_labels_hang_beside_his_head_on_the_rooms_side() {
    let t = tuning();
    let side = Fx::from_px(crate::game::OBERKELLNER_POPUP_SIDE_PX);
    let stack = Fx::from_px(crate::entity::POPUP_STACK_PX);
    let mut g = locked();
    for (hit, wall_x, room, label, points) in [
        (1u8, LEFT_WALL_X, 1, Popup::Unerhoert, 1000),
        (2, RIGHT_WALL_X, -1, Popup::Unerhoert, 1000),
        (3, LEFT_WALL_X, 1, Popup::BitteSehr, 5000),
    ] {
        until_boss(&mut g, st::BREATHE, 900);
        let b = boss(&g);
        assert_eq!(b.x, Fx::from_px(wall_x), "hit {hit}: at the wall");
        assert_eq!(i32::from(b.dir), -room, "facing the wall");
        let head = b.y;
        assert_eq!(
            head,
            Fx::from_px(FLOOR_Y - t.enemies.oberkellner_breathe_hitbox[1])
        );
        let (cx, _) = b.feet();
        drop_onto(&mut g, b, 2);
        g.step(input("-"));
        assert_eq!(boss(&g).hits, hit);
        let popup = |p: Popup| {
            *g.entities
                .iter()
                .find(|e| e.kind == EntityKind::Popup(p))
                .unwrap_or_else(|| panic!("hit {hit}: no {p:?}"))
        };
        let (text, score) = (popup(label), popup(Popup::Score(points)));
        // The text's outline row is his head top; the score one line under.
        assert_eq!(text.y, head + Fx::ONE, "hit {hit}");
        assert_eq!(score.y, text.y + stack, "hit {hit}");
        let room_x = cx + side.mul_int(room);
        assert_eq!(
            (text.feet().0, score.feet().0),
            (room_x, room_x),
            "hit {hit}"
        );
        if hit < 3 {
            until_boss(&mut g, st::TELEGRAPH, 200);
            g.entities
                .retain(|e| !matches!(e.kind, EntityKind::Popup(_)));
        }
    }
}

#[test]
fn a_stomp_in_the_breather_is_a_hit_and_its_side_hurts() {
    let t = tuning();
    let mut g = locked();
    until_boss(&mut g, st::BREATHE, 400);
    let b = boss(&g);
    assert_eq!(b.axe_hit(), AxeHit::Hit);
    g.player.stomp_chain = 3;
    drop_onto(&mut g, b, 2);
    g.player.invulnerable = 0;
    let events = g.step(input("-")).to_vec();
    assert!(events.contains(&Event::Stomped));
    let b = boss(&g);
    assert_eq!((b.hits, b.state, b.count), (1, st::STUN, 1));
    assert_eq!(g.score(), t.enemies.oberkellner_hit_points);
    assert!(has_popup(&g, Popup::Score(1000)) && has_popup(&g, Popup::Unerhoert));
    assert_eq!(g.player.stomp_chain, 3, "boss points are outside the chain");
    assert!(g.player.vy < Fx::ZERO, "she bounces as from a stomp");
    // The stun: no contact, 60 frames, then the next round's telegraph.
    assert_eq!(
        frames_in_state(&mut g),
        u32::from(t.enemies.oberkellner_stun)
    );
    assert_eq!(boss(&g).state, st::TELEGRAPH);
    // Side contact in the breather hurts.
    let mut g = locked();
    until_boss(&mut g, st::BREATHE, 400);
    let b = boss(&g);
    g.player.invulnerable = 0;
    beside(&mut g, b);
    let events = g.step(input("L")).to_vec();
    assert!(events.contains(&Event::Died), "Normal, side contact");
    assert_eq!(boss(&g).hits, 0);
}

/// Open risk 4 of the plan: the frame his hitbox changes size. The stomp
/// rule compares her previous feet with the middle of the box she touches
/// at its previous position; his box shrinks (36 → 30, the breather) or
/// grows (30 → 36, the next telegraph) with the feet kept, and `prev_y`
/// moves with its top, so the middle is the new box's.
#[test]
fn the_frame_his_hitbox_changes_size_is_judged_on_the_new_box() {
    // The breather's first frame: her feet were 12 px below the new (30 px)
    // top (y 210) on the previous frame, above its middle (225). Measured
    // with the old top (204) and the new half height the middle would be
    // 219, above her feet: a side hurt instead of the hit.
    let mut g = locked();
    until_boss(&mut g, st::DASH, 300);
    while boss(&g).state == st::DASH {
        let b = boss(&g);
        // Keep her clear until his last dash frame.
        on_refuge(&mut g, 330);
        if b.x <= Fx::from_px(LEFT_WALL_X + 2) {
            break;
        }
        tick(&mut g, "-");
    }
    let b = boss(&g);
    assert_eq!(b.state, st::DASH);
    // Next frame he arrives (x 352) and breathes: his top goes 204 → 210.
    let new_top = Fx::from_px(FLOOR_Y - 30);
    g.player.x = Fx::from_px(LEFT_WALL_X + 2);
    g.player.y = new_top + Fx::from_px(12) - g.player.h;
    g.player.vy = Fx::from_px(3);
    g.player.vx = Fx::ZERO;
    g.player.on_ground = false;
    g.player.invulnerable = 0;
    let events = g.step(input("-")).to_vec();
    let b = boss(&g);
    assert_eq!(b.x, Fx::from_px(LEFT_WALL_X));
    assert!(events.contains(&Event::Stomped), "{events:?}");
    assert!(!events.contains(&Event::Died));
    assert_eq!(
        (b.hits, b.state),
        (1, st::STUN),
        "a hit on the breather's first frame"
    );

    // The telegraph's first frame after a breather: his top rises 6 px into
    // her falling feet. From above: a bounce off the tray, not a hurt.
    let mut g = locked();
    until_boss(&mut g, st::BREATHE, 400);
    while boss(&g).count < tuning().enemies.oberkellner_breathe[0] {
        on_refuge(&mut g, 330);
        tick(&mut g, "-");
    }
    let b = boss(&g);
    assert_eq!(b.state, st::BREATHE);
    // Feet 1 px above the breather's top (210), falling 3 px: next frame
    // the tray-up box (top 204) overlaps her.
    g.player.x = b.x + Fx::from_px(2);
    g.player.y = b.y - Fx::from_px(1) - g.player.h;
    g.player.vy = Fx::from_px(3);
    g.player.vx = Fx::ZERO;
    g.player.on_ground = false;
    g.player.invulnerable = 0;
    let events = g.step(input("-")).to_vec();
    assert_eq!(boss(&g).state, st::TELEGRAPH);
    assert!(events.contains(&Event::Stomped), "{events:?}");
    assert!(!events.contains(&Event::Died));
    assert_eq!(boss(&g).hits, 0, "a bounce, no hit");
}

// ----------------------------------------------------------------- axes

/// A flying axe placed on him.
fn axe_on(g: &mut Game, e: Entity) {
    let mut a = Entity::new(
        EntityKind::ThrownHackl,
        e.x + Fx::from_px(4),
        e.y + Fx::from_px(8),
        Fx::from_px(8),
        Fx::from_px(8),
    );
    a.state = axe::FLY;
    g.entities.push(a);
}

#[test]
fn an_axe_hits_once_per_breather_bounces_off_the_tray_and_passes_otherwise() {
    let t = tuning();
    // Breather: a hit; a second axe in the stun passes.
    let mut g = locked();
    until_boss(&mut g, st::BREATHE, 400);
    let b = boss(&g);
    axe_on(&mut g, b);
    let events = tick(&mut g, "-");
    assert!(events.contains(&Event::AxeHit));
    assert_eq!((boss(&g).hits, boss(&g).state), (1, st::STUN));
    assert_eq!(g.score(), t.enemies.oberkellner_hit_points);
    let b = boss(&g);
    axe_on(&mut g, b);
    let events = tick(&mut g, "-");
    assert!(!events.contains(&Event::AxeHit), "the stun lets it pass");
    assert_eq!(boss(&g).hits, 1, "one hit per breather");
    assert!(
        g.entities
            .iter()
            .any(|e| e.kind == EntityKind::ThrownHackl && !e.removed),
        "the axe flies on"
    );
    // Tray up: the axe bounces (gone, AxeHit, nothing else).
    let mut g = locked();
    until_boss(&mut g, st::TELEGRAPH, 200);
    let b = boss(&g);
    axe_on(&mut g, b);
    let events = tick(&mut g, "-");
    assert!(events.contains(&Event::AxeHit));
    assert_eq!((boss(&g).hits, g.score()), (0, 0));
    assert!(!g.entities.iter().any(|e| e.kind == EntityKind::ThrownHackl));
    // Intro: passes.
    let mut g = locked();
    let b = boss(&g);
    axe_on(&mut g, b);
    let events = tick(&mut g, "-");
    assert!(!events.contains(&Event::AxeHit));
    // A bill: paper, the axe passes.
    assert_eq!(crate::enemies::new_bill(25, &t).axe_hit(), AxeHit::Pass);
}

// --------------------------------------------------------------- points

#[test]
fn the_third_hit_gives_5000_bitte_sehr_removes_the_bills_and_he_bows() {
    let t = tuning();
    let mut g = locked();
    // Round 3 (two hits taken) with a volley in the air.
    boss_mut(&mut g).hits = 2;
    until_boss(&mut g, st::VOLLEY, 900);
    tick(&mut g, "-");
    assert_eq!(bills(&g).len(), 4);
    assert!(boss_undefeated(&g.entities));
    // Straight into the breather again for the third hit.
    let b = boss_mut(&mut g);
    b.state = st::BREATHE;
    b.count = 1;
    let i = boss_index(&g);
    let popups = g.hit_oberkellner(i);
    assert_eq!(popups.len(), 2);
    assert_eq!(popups[0].kind, EntityKind::Popup(Popup::Score(5000)));
    assert_eq!(popups[1].kind, EntityKind::Popup(Popup::BitteSehr));
    assert!(popups[1].y < popups[0].y, "stacked above the score");
    assert_eq!(g.score(), t.enemies.oberkellner_final_points);
    let b2 = boss(&g);
    assert_eq!((b2.hits, b2.state), (3, st::DONE));
    assert_eq!(b2.pose(&t), Some(Pose::Verbeugen));
    assert!(
        g.entities
            .iter()
            .filter(|e| e.kind == EntityKind::Rechnung)
            .all(|e| e.removed)
    );
    assert!(!boss_undefeated(&g.entities), "the finale's query (E47)");
    assert_eq!(boss_hits(&g.entities), Some(3));
    // Done for good: harmless, no further hit, no movement.
    tick(&mut g, "-");
    assert!(bills(&g).is_empty());
    for _ in 0..400 {
        tick(&mut g, "-");
    }
    let b3 = boss(&g);
    assert_eq!((b3.state, b3.x, b3.hits), (st::DONE, b2.x, 3));
    assert!(!b3.harmful());
    assert_eq!(g.hit_oberkellner(boss_index(&g)), Vec::new());
}

/// The player standing on the counter in the `goal_stollen`'s cell (38,
/// row 11: x 608, feet 192).
fn on_the_bell(g: &mut Game) {
    g.player.x = Fx::from_px(610);
    g.player.y = Fx::from_px(192) - g.player.h;
    g.player.vx = Fx::ZERO;
    g.player.vy = Fx::ZERO;
    g.player.on_ground = true;
}

#[test]
fn the_bell_is_armed_only_once_he_is_beaten() {
    // E47's stub note: an unarmed touch raises no `LevelComplete`.
    let mut g = locked();
    assert!(!g.goal_armed(0));
    for _ in 0..30 {
        on_the_bell(&mut g);
        let events = tick(&mut g, "-");
        assert!(!events.contains(&Event::LevelComplete));
        assert_eq!(g.mode(), Mode::Playing);
    }
    // Beaten (DONE): the next touch completes the stage.
    let b = boss_mut(&mut g);
    b.state = st::DONE;
    b.hits = 3;
    assert!(g.goal_armed(0));
    on_the_bell(&mut g);
    let events = tick(&mut g, "-");
    assert!(events.contains(&Event::LevelComplete), "{events:?}");
}

#[test]
fn an_unarmed_bell_does_not_suppress_a_hurt() {
    // E47's stub note: a hurt in the frame she touches the unarmed bell
    // still happens (only an armed goal wins over a hit, D-027).
    let t = tuning();
    let mut g = locked();
    on_the_bell(&mut g);
    let mut bill = crate::enemies::new_bill(38, &t);
    bill.state = rechnung::FALL;
    bill.base_x = g.player.x;
    bill.x = g.player.x;
    bill.y = g.player.y;
    g.entities.push(bill);
    g.player.invulnerable = 0;
    let events = g.step(input("-")).to_vec();
    assert!(events.contains(&Event::Died), "{events:?}");
    assert!(!events.contains(&Event::LevelComplete));
}

#[test]
fn boss_undefeated_is_false_without_an_oberkellner() {
    let g = Game::new(&flat(30), &tuning(), 1);
    assert!(!boss_undefeated(g.entities()));
    assert_eq!(boss_hits(g.entities()), None);
    let g = game();
    assert!(
        boss_undefeated(g.entities()),
        "waiting counts as undefeated"
    );
}

// ---------------------------------------------------------------- bills

#[test]
fn volleys_come_only_after_a_missed_breather_in_rounds_2_and_3() {
    let t = tuning();
    let left = 40 - crate::enemies::ARENA_COLS;
    for (hits, columns) in [(1u8, BILL_COLUMNS[0]), (2, BILL_COLUMNS[1])] {
        let mut g = locked();
        boss_mut(&mut g).hits = hits;
        until_boss(&mut g, st::VOLLEY, 900);
        let b = boss(&g);
        assert_eq!(b.count, 1);
        assert_eq!(b.h.to_px(), 36, "tray up again");
        assert_eq!(b.pose(&t), Some(Pose::Zahlen));
        let v = bills(&g);
        assert_eq!(v.len(), columns.len());
        for (bill, &c) in v.iter().zip(columns) {
            let centre = bill.x + bill.w.div_int(2);
            assert_eq!(centre, Fx::from_px((left + c) * 16 + 8), "column {c}");
            assert_eq!(bill.y, Fx::from_px(BILL_ROW * 16), "hanging in row 8");
            assert_eq!(bill.state, rechnung::HANG);
            assert_eq!((bill.w.to_px(), bill.h.to_px()), (8, 8));
        }
        // The volley ends on the first frame without a bill.
        let mut n = 0;
        while boss(&g).state == st::VOLLEY {
            tick(&mut g, "-");
            n += 1;
            assert!(n < 200, "the volley ends");
        }
        assert!(bills(&g).is_empty());
        assert_eq!(boss(&g).state, st::TELEGRAPH);
        assert!(n <= 140, "≤ 140 frames: {n}");
    }
    // Round 1 has no volley (checked in every_round_has_its_telegraph_…).
    assert_eq!(BILL_COLUMNS[0], &[4, 9, 14]);
    assert_eq!(BILL_COLUMNS[1], &[3, 7, 11, 15]);
}

#[test]
fn a_bill_hangs_harmless_for_30_frames_then_falls_1_px_with_the_sway() {
    let t = tuning();
    let e = &t.enemies;
    let mut g = locked();
    boss_mut(&mut g).hits = 1;
    until_boss(&mut g, st::VOLLEY, 900);
    let start = bills(&g)[0];
    // The spawn frame is the first of the hang; 29 updates more.
    for k in 1..u32::from(e.oberkellner_bill_hang) {
        tick(&mut g, "-");
        let b = bills(&g)[0];
        assert_eq!(
            (b.state, b.x, b.y),
            (rechnung::HANG, start.x, start.y),
            "hang {k}"
        );
        assert_eq!(b.pose(&t), Some(Pose::Hang));
    }
    // Harmless while hanging: she stands in it on a hang frame.
    let b = bills(&g)[0];
    let put_in = |g: &mut Game, b: Entity| {
        g.player.x = b.x - Fx::from_px(2);
        g.player.y = b.y - Fx::from_px(4);
        g.player.vx = Fx::ZERO;
        g.player.vy = Fx::ZERO;
        g.player.on_ground = false;
        g.player.invulnerable = 0;
    };
    let mut hanging = g.clone();
    let i = hanging
        .entities
        .iter()
        .position(|e| e.kind == EntityKind::Rechnung)
        .unwrap();
    hanging.entities[i].count = 0;
    put_in(&mut hanging, b);
    let events = hanging.step(input("-")).to_vec();
    assert_eq!(bills(&hanging)[0].state, rechnung::HANG);
    assert!(!events.contains(&Event::Died), "a hanging bill is harmless");
    // While she blinks a falling bill passes her (no hurt, not removed).
    let mut blinking = g.clone();
    put_in(&mut blinking, b);
    blinking.player.invulnerable = 60;
    let events = blinking.step(input("-")).to_vec();
    assert!(!events.contains(&Event::Hurt) && !events.contains(&Event::Died));
    assert!(
        bills(&blinking).iter().any(|x| x.base_x == b.base_x),
        "passes"
    );
    // Falling, it hurts on any contact (also from above) and is gone.
    put_in(&mut g, b);
    let events = g.step(input("-")).to_vec();
    assert!(events.contains(&Event::Died), "{events:?}");
    assert!(!bills(&g).iter().any(|x| x.base_x == b.base_x), "gone");
    assert_eq!(bills(&g).len(), 2, "the others fall on");
    // A fresh volley: watch the fall.
    let mut g = locked();
    boss_mut(&mut g).hits = 1;
    until_boss(&mut g, st::VOLLEY, 900);
    for _ in 1..e.oberkellner_bill_hang {
        tick(&mut g, "-");
    }
    let base = bills(&g)[0];
    let mut seen = Vec::new();
    for k in 1..=64u32 {
        tick(&mut g, "-");
        let b = bills(&g)[0];
        assert_eq!(b.state, rechnung::FALL);
        assert_eq!(b.pose(&t), Some(Pose::Flutter));
        assert_eq!(
            b.y,
            base.y + e.oberkellner_bill_fall.mul_int(k as i32),
            "1 px per frame"
        );
        let sway = sine_offset(
            k,
            e.oberkellner_bill_sway_period,
            Fx::from_px(e.oberkellner_bill_sway),
        );
        assert_eq!(b.x, base.x + sway, "integer sine, frame {k}");
        seen.push(b.x - base.x);
    }
    assert_eq!(*seen.iter().max().unwrap(), Fx::from_px(12), "+12 px");
    assert_eq!(*seen.iter().min().unwrap(), Fx::from_px(-12), "-12 px");
    // It reaches the floor (top 240, from 136: 104 px) and is gone there.
    let mut gone = None;
    for k in 65..=120u32 {
        tick(&mut g, "-");
        if !bills(&g).iter().any(|b| b.base_x == base.base_x) {
            gone = Some(k);
            break;
        }
    }
    assert_eq!(
        gone,
        Some(105),
        "gone on the frame it would enter the floor"
    );
}

#[test]
fn a_bill_is_gone_on_one_way_tiles_in_water_and_hazards() {
    let t = tuning();
    let level = Level::parse(
        "[meta]\nid = \"b\"\ntime_limit = 1\n[legend]\n\"#\" = \"solid\"\n\"-\" = \"oneway\"\n\"~\" = \"salzach_water\"\n\"^\" = \"hazard\"\n\"P\" = { entity = \"player_start\" }\n\"G\" = { entity = \"goal\" }\n[map]\ngrid = '''\n......\n......\n......\n......\n......\n......\n......\n......\n......\n......\n.-.~.^\n.#.#.#\nP#G#.#\n######\n'''\n",
    )
    .unwrap();
    let map = crate::physics::TileMap::from_level(&level);
    for (col, why) in [(1, "one-way tile"), (3, "water"), (5, "hazard")] {
        let mut b = crate::enemies::new_bill(col, &t);
        b.state = rechnung::FALL;
        let mut gone = false;
        for _ in 0..80 {
            crate::enemies::update_rechnung(&mut b, &map, &t);
            if b.removed {
                gone = true;
                break;
            }
        }
        assert!(gone, "{why}");
        assert!(b.y + b.h <= Fx::from_px(160 + 8), "{why}: at row 10");
    }
}

// -------------------------------------------------------------- respawn

#[test]
fn a_respawn_resets_the_fight() {
    let mut g = locked();
    boss_mut(&mut g).hits = 1;
    until_boss(&mut g, st::VOLLEY, 900);
    tick(&mut g, "-");
    assert!(!bills(&g).is_empty());
    // She dies as Normal against his side (tray up in the volley).
    let b = boss(&g);
    g.player.x = b.x - g.player.w + Fx::from_px(2);
    g.player.y = Fx::from_px(FLOOR_Y) - g.player.h;
    g.player.on_ground = true;
    g.player.invulnerable = 0;
    g.step(input("R"));
    assert!(matches!(g.mode(), Mode::Dying { .. }));
    for _ in 0..200 {
        g.step(input("-"));
        if g.mode() == Mode::Playing {
            break;
        }
    }
    let b = boss(&g);
    assert_eq!((b.state, b.hits, b.x), (st::WAIT, 0, Fx::from_px(544)));
    assert!(bills(&g).is_empty());
    // Not locked again until the camera is back at its clamp.
    for _ in 0..30 {
        g.step(input("-"));
    }
    assert_eq!(boss(&g).state, st::WAIT);
}

// ------------------------------------------------------------- the kind

#[test]
fn the_kind_is_on_foot_not_stompable_and_sinks() {
    assert!(!EnemyKind::Oberkellner.stompable());
    assert!(!EnemyKind::Oberkellner.hurts_from_above());
    assert!(EnemyKind::Oberkellner.sinks_in_swim_water());
    assert_eq!(EnemyKind::Oberkellner.sprite(), "oberkellner");
    assert_eq!(EnemyKind::Oberkellner.axe_hit(), AxeHit::Bounce);
    assert_eq!(EntityKind::Rechnung.sprite(), Some("rechnung"));
    assert_eq!(Popup::Anstellen.text_key(), Some("popup.anstellen"));
    assert_eq!(Popup::Unerhoert.text_key(), Some("popup.unerhoert"));
    assert_eq!(Popup::BitteSehr.text_key(), Some("popup.bitte_sehr"));
    let g = game();
    let b = boss(&g);
    assert_eq!((b.state, b.dir, b.hits), (st::WAIT, -1, 0));
    assert_eq!(b.pose(&tuning()), Some(Pose::Idle));
    assert_eq!((b.x, b.y), (Fx::from_px(544), Fx::from_px(FLOOR_Y - 36)));
}

#[test]
fn world_hash_tells_his_hits_and_his_bills_apart() {
    let a = locked();
    let mut b = a.clone();
    assert_eq!(a.world_hash(), b.world_hash());
    boss_mut(&mut b).hits = 1;
    assert_ne!(a.world_hash(), b.world_hash(), "hits");
    let mut c = a.clone();
    boss_mut(&mut c).count += 1;
    assert_ne!(a.world_hash(), c.world_hash(), "counter");
    let mut d = a.clone();
    boss_mut(&mut d).dir = 1;
    assert_ne!(a.world_hash(), d.world_hash(), "facing");
    let mut e = a.clone();
    e.entities.push(crate::enemies::new_bill(25, &tuning()));
    assert_ne!(a.world_hash(), e.world_hash(), "a bill");
    let mut f = e.clone();
    let last = f.entities.len() - 1;
    f.entities[last].state = rechnung::FALL;
    assert_ne!(e.world_hash(), f.world_hash(), "a bill's state");
}

// ----------------------------------------------------------------- lint

/// A 40 × 17 arena level from `rows` (row 0 first); `#` solid, `-` one-way,
/// `~` water, `K` the Oberkellner, `t` a Selfie-Tourist, `s` a sign, `?` a box, `P`,
/// `G`.
fn arena_level(rows: &[String]) -> Level {
    let text = format!(
        "[meta]\nid = \"a\"\ntime_limit = 1\n[legend]\n\"#\" = \"solid\"\n\"-\" = \"oneway\"\n\"~\" = \"salzach_water\"\n\"?\" = {{ tile = \"oblaten_box\", contains = \"oblate\" }}\n\"K\" = {{ entity = \"oberkellner\" }}\n\"t\" = {{ entity = \"selfie_tourist\" }}\n\"s\" = {{ entity = \"sign\", text = \"x\" }}\n\"P\" = {{ entity = \"player_start\" }}\n\"G\" = {{ entity = \"goal\" }}\n[map]\ngrid = '''\n{}\n'''\n",
        rows.join("\n")
    );
    Level::parse(&text).unwrap()
}

/// The grey box's grid with `edit` applied (row, col, char).
fn grey_box_rows(edits: &[(usize, usize, char)]) -> Vec<String> {
    let text = include_str!("../../../../content/levels/00_m7_boss.level");
    let grid = text.split("grid = '''\n").nth(1).unwrap();
    let mut rows: Vec<Vec<char>> = grid
        .lines()
        .take_while(|l| !l.starts_with("'''"))
        .map(|l| {
            l.chars()
                .map(|c| match c {
                    'H' | 'M' => '?',
                    'C' => '.',
                    c => c,
                })
                .collect()
        })
        .collect();
    for &(r, c, ch) in edits {
        rows[r][c] = ch;
    }
    rows.into_iter().map(|r| r.into_iter().collect()).collect()
}

#[test]
fn the_arena_lint_passes_the_grey_box_and_names_every_broken_rule() {
    let t = tuning();
    assert_eq!(arena_problems(&arena(), &t), Vec::<String>::new());
    let ok = arena_level(&grey_box_rows(&[]));
    assert_eq!(arena_problems(&ok, &t), Vec::<String>::new());
    let check = |edits: &[(usize, usize, char)], want: &str| {
        let p = arena_problems(&arena_level(&grey_box_rows(edits)), &t);
        assert!(
            p.iter().any(|m| m.contains(want)),
            "{edits:?}: expected `{want}` in {p:?}"
        );
    };
    // Two bosses.
    check(&[(14, 30, 'K')], "exactly one");
    // Outside the arena (and the one in the arena removed).
    check(&[(14, 34, '.'), (14, 10, 'K')], "not in the arena");
    // Something solid or one-way over the floor (a ledge, a shelf).
    check(&[(10, 28, '-')], "3-high solid refuge");
    check(&[(7, 28, '#')], "3-high solid refuge");
    // Water over the floor (he is on foot; only air above it).
    check(&[(14, 28, '~')], "3-high solid refuge");
    // A refuge only 2 high.
    check(&[(12, 20, '.'), (12, 21, '.')], "3-high solid refuge");
    // A hole in the floor.
    check(&[(15, 28, '.'), (16, 28, '.')], "3-high solid refuge");
    // No refuge on the right: the floor runs to the level's edge.
    check(
        &[
            (12, 37, '.'),
            (13, 37, '.'),
            (14, 37, '.'),
            (12, 38, '.'),
            (13, 38, '.'),
            (14, 38, '.'),
            (12, 39, '.'),
            (13, 39, '.'),
            (14, 39, '.'),
            (11, 38, '.'),
            (14, 4, 'G'),
        ],
        "3-high solid refuge",
    );
    // Another enemy, a sign, a box in the arena.
    check(&[(14, 25, 't')], "selfie_tourist at row 15, column 26");
    check(&[(14, 25, 's')], "sign at row 15, column 26");
    check(&[(10, 25, '?')], "box or brick at row 11, column 26");
    // A narrower floor: the outer bill columns fall over a refuge.
    let narrow = &[(12, 22, '#'), (13, 22, '#'), (14, 22, '#')];
    check(narrow, "would fall outside the floor");
    // Not on the floor: in the left refuge's bottom cell, or in the air.
    check(&[(14, 34, '.'), (14, 20, 'K')], "stand on the arena floor");
    check(
        &[(14, 34, '.'), (10, 30, 'K')],
        "the arena has no floor on row 12",
    );
    // Levels without one: nothing.
    assert!(arena_problems(&flat(30), &t).is_empty());
}

// ---------------------------------------------------------- the windows

/// D-106: every required press of the Normal-form win has a window of at
/// least 8 frames, measured the D-100 way: each press (or press group:
/// the step off the refuge with its braking and the hop back) shifted
/// alone by d frames, every other input at its frame (its neighbours
/// absorb the shift), judged on the replay's outcome (complete, no
/// death, no hurt, three hits). Measured over ±40 (see the replay's
/// header for the values): the jump onto the refuge after the lock 106
/// (route-limited), the step-offs 29 / 26 / 46, the hops back 29 / 29 /
/// 85, the jump onto the counter 64. Checked here over ±8 only.
#[test]
fn normal_win_presses_have_windows_of_at_least_8() {
    let base = Replay::parse(include_str!(
        "../../../../tests/replays/m7_boss_win_normal.replay"
    ))
    .unwrap();
    let level = arena();
    let t = tuning();
    let wins = |r: &Replay| {
        let o = r.run(&level, &t, |_| {});
        o.completed_at.is_some()
            && o.deaths == 0
            && !o.events.iter().any(|&(e, _)| e == Event::Hurt)
            && o.boss_hits == Some(3)
    };
    assert!(wins(&base));
    // (first, last) input runs of each press (indices into `inputs`).
    let presses: [(usize, usize, &str); 8] = [
        (6, 6, "onto the refuge after the lock"),
        (9, 9, "step-off 1"),
        (11, 11, "hop back 1"),
        (14, 14, "step-off 2"),
        (16, 16, "hop back 2"),
        (19, 19, "step-off 3"),
        (21, 21, "hop back 3"),
        (25, 25, "onto the counter"),
    ];
    for (a, b, what) in presses {
        let shifted = |d: i64| {
            let mut r = base.clone();
            let before = i64::from(r.inputs[a - 1].1) + d;
            let after = i64::from(r.inputs[b + 1].1) - d;
            if before < 1 || after < 1 {
                return false;
            }
            r.inputs[a - 1].1 = before as u32;
            r.inputs[b + 1].1 = after as u32;
            wins(&r)
        };
        let lo = (1..=8).take_while(|&m| shifted(-m)).count() as i64;
        let hi = (1..=8).take_while(|&m| shifted(m)).count() as i64;
        assert!(lo + hi + 1 >= 8, "{what}: window -{lo}..+{hi}");
    }
}
