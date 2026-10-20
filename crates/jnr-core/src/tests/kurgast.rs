//! The Kurgast of Stage 8 (game-design.md → "Stage 8: Bad Ischl",
//! entities and "New mechanics"; D-095): a slow walker with the
//! Nordic-Walker's turn rules who, stomped, lies down in his deck chair, a
//! one-way top platform (the enemy-as-platform hook in
//! [`crate::platform::land`]), rests and gets up again; the rest and the
//! wake telegraph pause while Resi stands on him, and only his first stomp
//! scores.

use super::*;
use crate::enemies::{Pose, is_platform_now, lie_down, state};
use crate::player::Power;
use crate::{EnemyKind, Entity, EntityKind, Fx, Popup};

/// Legend of the test worlds (`.` air, `P` start).
const LEGEND: &str = r##"
"#" = "solid"
"-" = "oneway"
"B" = "brick"
"k" = { entity = "kurgast" }
"w" = { entity = "nordic_walker" }
"T" = { entity = "selfie_tourist", still = true }
"f" = { entity = "fiaker" }
"P" = { entity = "player_start" }
"G" = { entity = "goal" }
"##;

const KURGAST: EntityKind = EntityKind::Enemy(EnemyKind::Kurgast);

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
        "[meta]\nid = \"kurpark\"\ntime_limit = 300\n[legend]\n{LEGEND}\n[map]\ngrid = '''\n{}\n'''\n",
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

fn index(g: &Game, kind: EntityKind) -> usize {
    g.entities().iter().position(|e| e.kind == kind).unwrap()
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

/// Puts the player standing with her hitbox centre at `cx` px and her
/// feet at `feet` px (no physics).
fn stand(g: &mut Game, cx: i32, feet: i32) {
    g.player.x = Fx::from_px(cx) - g.player.w.div_int(2);
    g.player.y = Fx::from_px(feet) - g.player.h;
    g.player.vx = Fx::ZERO;
    g.player.vy = Fx::ZERO;
    g.player.on_ground = true;
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

/// `g` with the player in `power` at once (no freeze).
fn with_power(mut g: Game, power: Power) -> Game {
    g.player.set_power(power, &tuning().player);
    g.player.transform = 0;
    g
}

/// A Kurgast at column 20 stomped once (from the frame after activation);
/// returns the game right after the stomp frame and the events of it.
fn stomped() -> (Game, Vec<Event>) {
    let mut g = game(&world(40, &[(11, 20, 'k')]));
    safe_step(&mut g);
    let k = first(&g, KURGAST);
    drop_onto(&mut g, k);
    let events = g.step(input("-")).to_vec();
    (g, events)
}

/// The Kurgast's centre x (px).
fn centre(e: &Entity) -> Fx {
    e.x + e.w.div_int(2)
}

// --------------------------------------------------------------- walking

#[test]
fn a_kurgast_spawns_12_by_22_on_his_cell_floor() {
    let g = game(&world(40, &[(11, 20, 'k')]));
    let k = first(&g, KURGAST);
    assert!(!k.active);
    assert_eq!((k.w, k.h), (Fx::from_px(12), Fx::from_px(22)));
    assert_eq!(k.feet(), (Fx::from_px(20 * 16 + 8), Fx::from_px(FLOOR)));
    assert!(EnemyKind::Kurgast.stompable());
    assert_eq!(EnemyKind::Kurgast.chain_start(), 0, "100 points");
    assert!(EnemyKind::Kurgast.sinks_in_swim_water(), "D-097");
    assert_eq!(EnemyKind::Kurgast.sprite(), "kurgast");
    assert_eq!(Popup::Kurruhe.text_key(), Some("popup.kurruhe"));
}

#[test]
fn he_walks_0_375_towards_the_player_and_turns_at_walls() {
    // The player at column 2 (left); a wall at column 14.
    let mut g = game(&world(40, &[(11, 20, 'k'), (11, 14, '#'), (10, 14, '#')]));
    safe_step(&mut g);
    let k0 = first(&g, KURGAST);
    assert_eq!(k0.dir, -1, "towards the player");
    for _ in 0..16 {
        safe_step(&mut g);
    }
    let k = first(&g, KURGAST);
    assert_eq!(k0.x - k.x, Fx(96 * 16), "0.375 px/frame");
    assert_eq!(k.pose(&tuning()), Some(Pose::Walk));
    // At the wall (its right face at x 240) he turns and walks back.
    let mut turned = false;
    for _ in 0..400 {
        safe_step(&mut g);
        let k = first(&g, KURGAST);
        assert!(k.x >= Fx::from_px(240), "never in the wall");
        turned |= k.dir == 1;
    }
    assert!(turned);
}

#[test]
fn he_turns_at_ledges_and_never_steps_off() {
    // A hole in the floor at column 16: his ledge is column 17's left
    // edge (x 272).
    let mut g = game(&world(40, &[(11, 20, 'k'), (12, 16, '.'), (13, 16, '.')]));
    let mut turned = false;
    for _ in 0..600 {
        safe_step(&mut g);
        let k = first(&g, KURGAST);
        assert!(k.x >= Fx::from_px(272), "x {:?}", k.x);
        assert_eq!(k.feet().1, Fx::from_px(FLOOR), "on the floor");
        turned |= k.dir == 1;
    }
    assert!(turned);
}

#[test]
fn he_turns_at_enemies_on_foot() {
    let mut g = game(&world(40, &[(11, 20, 'k'), (11, 16, 'T')]));
    let tourist = first(&g, EntityKind::Enemy(EnemyKind::SelfieTourist));
    let mut turned = false;
    for _ in 0..400 {
        safe_step(&mut g);
        let k = first(&g, KURGAST);
        assert!(k.x > tourist.x, "never walks through him");
        turned |= k.dir == 1;
    }
    assert!(turned);
}

// --------------------------------------------------------------- stomped

#[test]
fn a_stomp_lays_him_in_his_deck_chair_for_100_with_kurruhe_above() {
    let t = tuning();
    let mut g = game(&world(40, &[(11, 20, 'k')]));
    safe_step(&mut g);
    let k = first(&g, KURGAST);
    drop_onto(&mut g, k);
    let events = g.step(input("-")).to_vec();
    assert!(events.contains(&Event::Stomped), "{events:?}");
    assert!(!events.contains(&Event::Hurt));
    assert!(g.player().vy < Fx::ZERO, "she bounces");
    assert_eq!(g.score(), 100);
    assert_eq!(g.player().stomp_chain, 1, "in the chain");
    let c = first(&g, KURGAST);
    assert_eq!(c.state, state::LIE);
    assert_eq!(c.pose(&t), Some(Pose::Liegen));
    assert!(!c.harmful(), "harmless in the chair");
    assert!(is_platform_now(&c), "the enemy-as-platform hook");
    assert!(!c.kind.is_platform(), "not a moving platform");
    // The 16×16 chair, centred on him, its bottom on his feet.
    assert_eq!((c.w, c.h), (Fx::from_px(16), Fx::from_px(16)));
    // (He took his step of the frame before the stomp.)
    assert_eq!(centre(&c), centre(&k) - Fx(96));
    assert_eq!(c.feet().1, k.feet().1);
    // „Kurruhe!“ stacked above the score popup, like „Bravo!“.
    let p = popups(&g);
    let score = p
        .iter()
        .find(|e| e.kind == EntityKind::Popup(Popup::Score(100)))
        .unwrap();
    let text = p
        .iter()
        .find(|e| e.kind == EntityKind::Popup(Popup::Kurruhe))
        .unwrap();
    assert_eq!(text.x, score.x, "centred together");
    assert_eq!(score.y - text.y, Fx::from_px(crate::entity::POPUP_STACK_PX));
}

#[test]
fn the_chair_is_a_one_way_top_she_lands_and_stands_on() {
    let (mut g, _) = stomped();
    let chair = first(&g, KURGAST);
    let top = chair.y;
    // She falls back from the bounce onto the chair (she is right above).
    let events = until(&mut g, "-", 120, |g| g.player().on_ground);
    assert!(events.contains(&Event::Landed));
    assert!(!events.contains(&Event::Hurt) && !events.contains(&Event::Stomped));
    assert_eq!(g.player().y + g.player().h, top, "feet on the chair top");
    // She stays standing on it, frame after frame.
    for _ in 0..60 {
        let events = g.step(input("-")).to_vec();
        assert!(!events.contains(&Event::Landed), "no landing every frame");
        assert!(g.player().on_ground);
        assert_eq!(g.player().y + g.player().h, top);
    }
    assert_eq!(g.score(), 100, "standing on him scores nothing");
    // A jump from the chair is a normal jump (also DOWN + JUMP).
    let events = hold(&mut g, "DJ", 1);
    assert!(events.contains(&Event::Jumped));
}

#[test]
fn she_walks_through_the_chair_and_jumps_up_through_it_onto_it() {
    let (mut g, _) = stomped();
    let chair = first(&g, KURGAST);
    // On the floor left of the chair, walking right through it: no harm.
    stand(&mut g, centre(&chair).to_px() - 20, FLOOR);
    let mut inside = false;
    for _ in 0..40 {
        let events = g.step(input("R")).to_vec();
        assert!(!events.contains(&Event::Hurt) && !events.contains(&Event::Stomped));
        inside |= g.player().hitbox().overlaps(&chair.hitbox());
        assert_eq!(
            g.player().y + g.player().h,
            Fx::from_px(FLOOR),
            "on the floor"
        );
    }
    assert!(inside, "she walked through it");
    // Inside the chair on the floor, a jump goes up through its top and
    // she lands on it on the way down.
    stand(&mut g, centre(&chair).to_px(), FLOOR);
    hold(&mut g, "J", 1);
    until(&mut g, "J", 120, |g| g.player().on_ground);
    assert_eq!(g.player().y + g.player().h, chair.y, "on the chair top");
}

#[test]
fn he_rests_300_then_wakes_40_then_stands_up_with_his_feet_kept() {
    let t = tuning();
    let (mut g, _) = stomped();
    let chair = first(&g, KURGAST);
    // The player far away to his left, standing on the floor.
    stand(&mut g, 40, FLOOR);
    for n in 1..=340 {
        safe_step(&mut g);
        let k = first(&g, KURGAST);
        let expected = match n {
            1..=299 => Pose::Liegen,
            300..=339 => Pose::Wake,
            _ => Pose::Walk,
        };
        assert_eq!(k.pose(&t), Some(expected), "frame {n} after the stomp");
        if n < 340 {
            assert!(!k.harmful() && is_platform_now(&k), "frame {n}");
            assert_eq!(k.hitbox(), chair.hitbox(), "the chair never moves");
        }
    }
    let k = first(&g, KURGAST);
    assert_eq!(k.state, state::RESTED);
    assert!(k.harmful() && !is_platform_now(&k));
    assert_eq!((k.w, k.h), (Fx::from_px(12), Fx::from_px(22)));
    assert_eq!(k.feet(), chair.feet(), "feet and centre kept");
    assert_eq!(k.dir, -1, "facing the player");
    // He walks on (no step in the frame he stood up).
    let x = k.x;
    safe_step(&mut g);
    assert_eq!(x - first(&g, KURGAST).x, Fx(96));
}

#[test]
fn both_counters_pause_while_she_stands_on_him_so_he_never_stands_up_under_her() {
    let (mut g, _) = stomped();
    // She lands on the chair and stays there: the rest doesn't run.
    until(&mut g, "-", 120, |g| g.player().on_ground);
    let count = first(&g, KURGAST).count;
    for _ in 0..1000 {
        let events = g.step(input("-")).to_vec();
        assert!(!events.contains(&Event::Hurt), "never hurt by him");
    }
    let k = first(&g, KURGAST);
    assert_eq!((k.state, k.count), (state::LIE, count), "the rest paused");
    // Off the chair the rest runs on; in the wake telegraph she steps back
    // on: it pauses too.
    let chair = k;
    stand(&mut g, 40, FLOOR);
    until(&mut g, "-", 400, |g| first(g, KURGAST).state == state::WAKE);
    hold(&mut g, "-", 10);
    assert_eq!(first(&g, KURGAST).count, 10);
    stand(&mut g, centre(&chair).to_px(), chair.y.to_px());
    for _ in 0..500 {
        let events = g.step(input("-")).to_vec();
        assert!(!events.contains(&Event::Hurt));
        assert!(g.player().on_ground);
    }
    let k = first(&g, KURGAST);
    assert_eq!((k.state, k.count), (state::WAKE, 10), "the wake paused");
    // Off again: he stands up 30 frames later.
    stand(&mut g, 40, FLOOR);
    hold(&mut g, "-", 29);
    assert_eq!(first(&g, KURGAST).state, state::WAKE);
    hold(&mut g, "-", 1);
    assert_eq!(first(&g, KURGAST).state, state::RESTED);
}

#[test]
fn a_waking_kurgast_hurts_her_when_he_stands_up_beside_her() {
    let (g, _) = stomped();
    let mut g = with_power(g, Power::Big);
    let chair = first(&g, KURGAST);
    // On the floor overlapping the chair's edge: not standing on him.
    stand(&mut g, centre(&chair).to_px() + 9, FLOOR);
    let mut events = Vec::new();
    for n in 1..=340 {
        events = g.step(input("-")).to_vec();
        if n < 340 {
            assert!(
                !events.contains(&Event::Hurt),
                "harmless in the chair, frame {n}"
            );
        }
    }
    assert!(events.contains(&Event::Hurt), "{events:?}");
    assert_eq!(g.player().power, Power::Normal);
}

#[test]
fn only_his_first_stomp_scores_later_ones_bounce_without_points_or_chain_change() {
    let (mut g, _) = stomped();
    assert_eq!(g.score(), 100);
    stand(&mut g, 40, FLOOR);
    until(&mut g, "-", 400, |g| {
        first(g, KURGAST).state == state::RESTED
    });
    hold(&mut g, "-", 40);
    // A chain is running (two stomps so far).
    let k = first(&g, KURGAST);
    drop_onto(&mut g, k);
    g.player.stomp_chain = 2;
    let events = g.step(input("-")).to_vec();
    assert!(events.contains(&Event::Stomped), "{events:?}");
    assert!(g.player().vy < Fx::ZERO, "she bounces");
    assert_eq!(g.score(), 100, "no points");
    assert_eq!(
        g.player().stomp_chain,
        2,
        "the chain neither advances nor resets"
    );
    let c = first(&g, KURGAST);
    assert_eq!(c.state, state::LIE, "lying again");
    assert_eq!(
        c.feet(),
        (k.feet().0 - Fx(96), k.feet().1),
        "where he walked"
    );
    let p = popups(&g);
    assert!(
        !p.iter()
            .any(|e| matches!(e.kind, EntityKind::Popup(Popup::Score(_)))),
        "no score popup"
    );
    let text = p
        .iter()
        .find(|e| e.kind == EntityKind::Popup(Popup::Kurruhe))
        .expect("„Kurruhe!“ again");
    assert_eq!(text.y, k.y, "where the score popup would be");
}

// ---------------------------------------------- the other walkers, charges

#[test]
fn walkers_turn_at_his_chair() {
    let mut g = game(&world(40, &[(11, 14, 'k'), (11, 22, 'w')]));
    safe_step(&mut g);
    let i = index(&g, KURGAST);
    lie_down(&mut g.entities[i], &tuning());
    let chair = first(&g, KURGAST);
    let mut turned = false;
    for _ in 0..300 {
        safe_step(&mut g);
        let w = first(&g, EntityKind::Enemy(EnemyKind::NordicWalker));
        assert!(w.x > chair.x, "never walks through the chair");
        turned |= w.dir == 1;
    }
    assert!(turned);
}

#[test]
fn a_charging_fiaker_passes_his_chair() {
    // The Fiaker at 10 charges left through the chair at 7.
    let mut g = game(&world(40, &[(11, 7, 'k'), (11, 10, 'f')]));
    safe_step(&mut g);
    let i = index(&g, KURGAST);
    lie_down(&mut g.entities[i], &tuning());
    let chair = first(&g, KURGAST);
    let fiaker = EntityKind::Enemy(EnemyKind::Fiaker);
    let mut passed = false;
    for _ in 0..300 {
        safe_step(&mut g);
        passed |= g
            .entities()
            .iter()
            .find(|e| e.kind == fiaker)
            .is_none_or(|f| f.right() < chair.x);
    }
    assert!(passed, "the Fiaker drove through the chair");
    assert!(is_platform_now(&first(&g, KURGAST)), "still in his chair");
}

// --------------------------------------------------------- bump and axe

#[test]
fn a_bump_knocks_him_off_for_100_and_off_his_chair_for_nothing() {
    for (lying, points) in [(false, 100), (true, 0)] {
        // Bricks at row 8, columns 1–3, the Kurgast on them at column 2;
        // the player jumps from column 2 into the brick above.
        let mut g = game(&world(
            20,
            &[(8, 1, 'B'), (8, 2, 'B'), (8, 3, 'B'), (7, 2, 'k')],
        ));
        safe_step(&mut g);
        if lying {
            let i = index(&g, KURGAST);
            lie_down(&mut g.entities[i], &tuning());
        }
        let events = until(&mut g, "J", 40, |g| {
            g.events().contains(&Event::BlockBumped)
        });
        assert!(!events.contains(&Event::Died));
        let d = first(&g, EntityKind::Defeated(EnemyKind::Kurgast));
        assert_eq!(d.state, state::TUMBLE, "lying {lying}");
        assert_eq!(g.score(), points, "lying {lying}");
        assert_eq!(
            popups(&g)
                .iter()
                .any(|e| matches!(e.kind, EntityKind::Popup(Popup::Score(_)))),
            points > 0,
            "lying {lying}"
        );
    }
}

/// Where a Kurgast stands to meet an axe thrown from the start (column 2)
/// on its way down (it flies 6 tiles until it is back at hand height).
const AXE_COL: usize = 10;

#[test]
fn an_axe_defeats_him_walking_and_passes_his_chair() {
    let t = tuning();
    for st in [state::WALK, state::LIE, state::RESTED] {
        let mut g = with_power(game(&world(40, &[(11, AXE_COL, 'k')])), Power::Hackl);
        safe_step(&mut g);
        let i = index(&g, KURGAST);
        if st == state::LIE {
            lie_down(&mut g.entities[i], &t);
        } else {
            g.entities[i].state = st;
        }
        assert_eq!(
            g.entities[i].axe_hit(),
            if st == state::LIE {
                crate::hackl::AxeHit::Pass
            } else {
                crate::hackl::AxeHit::Defeat
            }
        );
        g.player.facing = crate::player::Facing::Right;
        let mut events = hold(&mut g, "B", 1);
        for _ in 0..60 {
            events.extend(safe_step(&mut g));
        }
        if st == state::LIE {
            assert!(!events.contains(&Event::AxeHit), "the axe passes");
            assert_eq!(first(&g, KURGAST).state, state::LIE);
            assert_eq!(g.score(), 0);
        } else {
            assert!(events.contains(&Event::AxeHit), "{st}: {events:?}");
            assert_eq!(
                first(&g, EntityKind::Defeated(EnemyKind::Kurgast)).state,
                state::TUMBLE
            );
            // 100 if he hasn't scored yet, else nothing (D-095).
            let points = if st == state::WALK { 100 } else { 0 };
            assert_eq!(g.score(), points, "{st}");
            assert_eq!(
                popups(&g)
                    .iter()
                    .any(|e| e.kind == EntityKind::Popup(Popup::Score(100))),
                st == state::WALK
            );
        }
    }
}

#[test]
fn world_hash_covers_the_rest_and_the_wake() {
    let (mut g, _) = stomped();
    let i = index(&g, KURGAST);
    let before = g.world_hash();
    g.entities[i].count += 1;
    assert_ne!(g.world_hash(), before, "the rest counter");
    let before = g.world_hash();
    g.entities[i].state = state::WAKE;
    assert_ne!(g.world_hash(), before, "the state");
}
