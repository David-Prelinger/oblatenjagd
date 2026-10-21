//! The M5 lake enemies of Stage 6 (game-design.md → "Stage 6:
//! Wolfgangsee", entities and "New mechanics"; D-078 … D-081): the Schwan,
//! the Gelse and the Regattaboot, their contact rules (also while Resi
//! swims), the thrown Hackl against them and the regatta lane lint.
//!
//! The lake is swim water (`see_wasser`, D-076). Live dives against the
//! lake enemies are in `tests/lake.rs` (E30); here the contact rule
//! ([`contact`]) and hitbox geometry are checked directly.

use super::*;
use crate::enemies::{Contact, Pose, contact, regatta_parts, reisebus_lane_problems, state};
use crate::entity::UpdateCtx;
use crate::hackl::AxeHit;
use crate::physics::{Aabb, TileMap};
use crate::player::Power;
use crate::{EnemyKind, Entity, EntityKind, Fx, Popup, TileKind};

/// Legend of the test worlds (`.` air, `P` start).
const LEGEND: &str = r##"
"#" = "solid"
"-" = "oneway"
"~" = "see_wasser"
"s" = { entity = "schwan", tile = "see_wasser" }
"g" = { entity = "gelse" }
"b" = { entity = "regattaboot" }
"r" = { entity = "reisebus" }
"P" = { entity = "player_start" }
"G" = { entity = "goal" }
"##;

const SWAN: EntityKind = EntityKind::Enemy(EnemyKind::Schwan);
const GELSE: EntityKind = EntityKind::Enemy(EnemyKind::Gelse);
const BOAT: EntityKind = EntityKind::Enemy(EnemyKind::Regattaboot);

/// The water line (the top of the surface row 12) and the deck tops (row
/// 11), in px.
const WATER_LINE: i32 = 192;
const DECK: i32 = 176;

/// The level text of a 17-row lake world `width` columns wide (the Stage 6
/// layout): a solid quay in columns 0–3 (top row 11, feet y 176) with the
/// start at column 2, water in rows 12–15 (the water line at y 192) and a
/// solid lake bed in row 16, plus `marks` as (row, col, char).
fn world_text(width: usize, marks: &[(usize, usize, char)]) -> String {
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
    if !marks.iter().any(|m| m.2 == 'P') {
        rows[10][2] = 'P';
    }
    for &(r, c, ch) in marks {
        rows[r][c] = ch;
    }
    let grid: Vec<String> = rows.into_iter().map(|r| r.into_iter().collect()).collect();
    format!(
        "[meta]\nid = \"lake\"\ntime_limit = 300\n[legend]\n{LEGEND}\n[map]\ngrid = '''\n{}\n'''\n",
        grid.join("\n")
    )
}

fn world(width: usize, marks: &[(usize, usize, char)]) -> Level {
    Level::parse(&world_text(width, marks)).unwrap()
}

/// A one-way deck on row 11 over columns `cols`.
fn deck(cols: std::ops::Range<usize>) -> Vec<(usize, usize, char)> {
    cols.map(|c| (11, c, '-')).collect()
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

/// A player hitbox of the current form with its top `below` px under the
/// water line, centred on `cx` (a swimmer: D-076 puts the chest point,
/// 8 px under the top, on the water line at the surface).
fn swimmer_box(g: &Game, cx: Fx, below: i32) -> Aabb {
    Aabb {
        x: cx - g.player.w.div_int(2),
        y: Fx::from_px(WATER_LINE + below),
        w: g.player.w,
        h: g.player.h,
    }
}

// ------------------------------------------------------------ contacts

#[test]
fn while_swimming_every_contact_hurts() {
    // D-076/D-078/D-080: no stomps, no roof bounces while she swims.
    for kind in [
        EnemyKind::Schwan,
        EnemyKind::Gelse,
        EnemyKind::Regattaboot,
        EnemyKind::Reisebus,
        EnemyKind::Fiaker,
        EnemyKind::SelfieTourist,
    ] {
        assert_eq!(contact(kind, true, true), Contact::Hurt, "{kind:?}");
        assert_eq!(contact(kind, false, true), Contact::Hurt, "{kind:?}");
        assert_eq!(contact(kind, false, false), Contact::Hurt, "{kind:?}");
    }
    // From the air (also a jump-out, airborne from its next frame).
    assert_eq!(contact(EnemyKind::Schwan, true, false), Contact::Stomp);
    assert_eq!(contact(EnemyKind::Gelse, true, false), Contact::Stomp);
    assert_eq!(
        contact(EnemyKind::Regattaboot, true, false),
        Contact::Bounce
    );
    assert_eq!(contact(EnemyKind::Reisebus, true, false), Contact::Bounce);
    assert_eq!(
        contact(EnemyKind::Bienenschwarm, true, false),
        Contact::Hurt
    );
    // On the quay she doesn't swim (`Game::player_swimming` is
    // `Player::swimming`, E30).
    let g = game(&world(20, &[]));
    assert!(!g.player_swimming());
}

#[test]
fn axe_classification_of_the_lake_enemies() {
    // D-081: Schwan and Gelse Defeat, Regattaboot Bounce.
    assert_eq!(EnemyKind::Schwan.axe_hit(), AxeHit::Defeat);
    assert_eq!(EnemyKind::Gelse.axe_hit(), AxeHit::Defeat);
    assert_eq!(EnemyKind::Regattaboot.axe_hit(), AxeHit::Bounce);
    assert!(EnemyKind::Schwan.stompable() && EnemyKind::Gelse.stompable());
    assert!(!EnemyKind::Regattaboot.stompable());
    assert_eq!(EnemyKind::Schwan.chain_start(), 1, "200, chain start 200");
    assert_eq!(EnemyKind::Gelse.chain_start(), 0, "100");
    assert_eq!(Popup::Beleidigt.text_key(), Some("popup.beleidigt"));
    assert_eq!(EnemyKind::Schwan.sprite(), "schwan");
    assert_eq!(EnemyKind::Gelse.sprite(), "gelse");
    assert_eq!(EnemyKind::Regattaboot.sprite(), "regattaboot");
}

#[test]
fn any_entity_may_sit_on_a_swim_cell_none_on_lethal_water() {
    // D-076 extends D-049 to swim tiles for every entity; lethal water
    // stays a colliding tile for entities (the D-078 swan-on-water special
    // case is gone with swim water, E30).
    let ok = world_text(20, &[(12, 10, 's')]);
    for entity in ["schwan", "gelse", "regattaboot", "selfie_tourist"] {
        let text = ok.replace(
            "\"s\" = { entity = \"schwan\", tile",
            &format!("\"s\" = {{ entity = \"{entity}\", tile"),
        );
        assert!(Level::parse(&text).is_ok(), "{entity}");
        let lethal = text.replace("tile = \"see_wasser\" }", "tile = \"wolfgangsee_water\" }");
        let err = Level::parse(&lethal).unwrap_err().to_string();
        assert!(err.contains("must not collide"), "{entity}: {err}");
    }
}

#[test]
fn world_hash_covers_the_lake_enemies_states() {
    let mut g = game(&world(40, &[(12, 10, 's'), (5, 14, 'g'), (12, 20, 'b')]));
    safe_step(&mut g);
    for kind in [SWAN, GELSE, BOAT] {
        let before = g.world_hash();
        let i = index(&g, kind);
        g.entities[i].state = state::TELEGRAPH;
        g.entities[i].count = 7;
        assert_ne!(g.world_hash(), before, "{kind:?}");
    }
}

// ----------------------------------------------------------------- swan

/// A swan at (12, 20) under a deck over columns 4–59: hitbox x 320..336,
/// centre 328.
fn swan_under_deck() -> Game {
    let mut m = deck(4..60);
    m.push((12, 20, 's'));
    game(&world(60, &m))
}

#[test]
fn a_swan_floats_4_px_under_the_water_line_8_px_below_the_decks() {
    let g = swan_under_deck();
    let s = first(&g, SWAN);
    assert_eq!((s.x, s.w), (Fx::from_px(320), Fx::from_px(16)));
    assert_eq!(s.y + s.h, Fx::from_px(WATER_LINE + 4), "float height");
    assert_eq!(s.y, Fx::from_px(DECK + 8), "top 8 px below the decks");
    assert_eq!(s.base_x, s.x, "its home point");
    assert_eq!(
        (s.state, s.pose(g.tuning())),
        (state::HOME, Some(Pose::Idle))
    );
    assert_eq!(g.tile(20, 12).map(|t| t.kind), Some(TileKind::Swim));
}

#[test]
fn a_swan_triggers_within_96_px_only_fully_on_screen() {
    // 97 px: nothing; 96 px: the hiss.
    let mut g = swan_under_deck();
    stand(&mut g, 328 - 97, DECK);
    for _ in 0..5 {
        safe_step(&mut g);
    }
    assert_eq!(first(&g, SWAN).state, state::HOME, "97 px");
    stand(&mut g, 328 - 96, DECK);
    safe_step(&mut g);
    let s = first(&g, SWAN);
    assert_eq!((s.state, s.count), (state::TELEGRAPH, 1));
    assert_eq!(s.pose(g.tuning()), Some(Pose::Telegraph));
    // In range but its right edge 1 px past the screen: nothing.
    let mut g = swan_under_deck();
    for _ in 0..5 {
        stand(&mut g, 300, DECK);
        g.camera.x = 336 - 320 - 1;
        safe_step(&mut g);
    }
    assert_eq!(first(&g, SWAN).state, state::HOME, "cut off at the right");
    stand(&mut g, 300, DECK);
    g.camera.x = 336 - 320;
    safe_step(&mut g);
    assert_eq!(first(&g, SWAN).state, state::TELEGRAPH, "fully on screen");
}

#[test]
fn hiss_32_charge_1_5_for_64_paddle_back_0_5_rest_60() {
    // The player stands on the deck 50 px left of it: it charges under her
    // (a deck rider is out of its reach) and back, a 348-frame cycle.
    let t = tuning();
    let mut g = swan_under_deck();
    stand(&mut g, 278, DECK);
    let mut events = until(&mut g, "-", 10, |g| {
        first(g, SWAN).state == state::TELEGRAPH
    });
    let base = first(&g, SWAN).base_x;
    let mut log = vec![(state::TELEGRAPH, first(&g, SWAN).x)];
    for _ in 1..348 {
        events.extend_from_slice(g.step(input("-")));
        let s = first(&g, SWAN);
        log.push((s.state, s.x));
    }
    assert!(
        !events.contains(&Event::Hurt) && g.deaths() == 0,
        "the deck is a refuge"
    );
    let e = &t.enemies;
    let (tel, charge) = (
        usize::from(e.schwan_telegraph),
        usize::from(e.schwan_charge_frames),
    );
    assert_eq!((tel, charge), (32, 64));
    assert!(
        log[..tel]
            .iter()
            .all(|&(st, x)| st == state::TELEGRAPH && x == base)
    );
    for k in tel..tel + charge {
        assert_eq!(log[k].1 - log[k - 1].1, Fx(-384), "charge frame {k}");
    }
    assert_eq!(
        log[tel + charge - 1].1,
        base - Fx::from_px(96),
        "96 px left"
    );
    let back = tel + charge + 192;
    for k in tel + charge..back {
        assert_eq!(log[k].1 - log[k - 1].1, Fx(128), "return frame {k}");
    }
    assert_eq!(log[back - 1], (state::REST, base), "home again");
    assert!(
        log[back - 1..back + 59]
            .iter()
            .all(|&(st, _)| st == state::REST)
    );
    assert_eq!(log[back + 59].0, state::HOME, "rested 60 frames");
    g.step(input("-"));
    assert_eq!(first(&g, SWAN).state, state::TELEGRAPH, "348 frames later");
}

#[test]
fn a_swan_charges_towards_the_side_the_player_was_on() {
    let mut g = swan_under_deck();
    stand(&mut g, 378, DECK);
    until(&mut g, "-", 10, |g| {
        first(g, SWAN).state == state::TELEGRAPH
    });
    let base = first(&g, SWAN).base_x;
    // She crosses to its left during the hiss: it still charges right.
    stand(&mut g, 250, DECK);
    // The trigger frame, 31 more of the hiss and 64 of the charge.
    for _ in 0..95 {
        safe_step(&mut g);
    }
    let s = first(&g, SWAN);
    assert_eq!((s.state, s.x), (state::PADDLE_BACK, base + Fx::from_px(96)));
}

#[test]
fn a_swan_hurts_from_the_side() {
    // A beach (solid, top on the water line) at columns 4–8 and a swan at
    // column 10: it charges onto the beach and into Resi's side.
    let mut m: Vec<_> = (12..16)
        .flat_map(|r| (4..9).map(move |c| (r, c, '#')))
        .collect();
    m.push((12, 10, 's'));
    let mut g = with_power(game(&world(40, &m)), Power::Big);
    stand(&mut g, 110, WATER_LINE);
    let events = until(&mut g, "-", 200, |g| g.player().power == Power::Normal);
    assert!(events.contains(&Event::Hurt) && !events.contains(&Event::Stomped));
    assert_eq!(first(&g, SWAN).state, state::CHARGE);
}

#[test]
fn a_surface_swimmer_touches_a_swan_and_a_diver_passes_under() {
    let g = swan_under_deck();
    let s = first(&g, SWAN).hitbox();
    let cx = s.x + s.w.div_int(2);
    // At the surface the chest point (8 px under her top) is on the water
    // line: her top is 8 px above it.
    for power in [Power::Normal, Power::Big] {
        let g = with_power(swan_under_deck(), power);
        assert!(
            swimmer_box(&g, cx, -8).overlaps(&s),
            "{power:?} at the surface"
        );
        assert!(
            !swimmer_box(&g, cx, 8).overlaps(&s),
            "{power:?} 1 tile down"
        );
    }
    // A deck rider's feet are 8 px above its top.
    let rider = Aabb {
        y: Fx::from_px(DECK) - g.player.h,
        ..swimmer_box(&g, cx, 0)
    };
    assert!(!rider.overlaps(&s));
}

#[test]
fn a_stomped_swan_sulks_for_200_with_beleidigt_above_the_score() {
    let t = tuning();
    let mut g = game(&world(40, &[(12, 12, 's')]));
    safe_step(&mut g);
    let s = first(&g, SWAN);
    drop_onto(&mut g, s);
    let events = g.step(input("-")).to_vec();
    assert!(events.contains(&Event::Stomped), "{events:?}");
    assert!(!events.contains(&Event::Hurt));
    assert_eq!(g.score(), 200, "chain start 200");
    let d = first(&g, EntityKind::Defeated(EnemyKind::Schwan));
    assert_eq!(d.state, state::SQUISHED);
    assert_eq!(d.pose(&t), Some(Pose::Squished));
    let p = popups(&g);
    let score = p
        .iter()
        .find(|e| e.kind == EntityKind::Popup(Popup::Score(200)))
        .unwrap();
    let text = p
        .iter()
        .find(|e| e.kind == EntityKind::Popup(Popup::Beleidigt))
        .unwrap();
    assert_eq!(text.x, score.x, "centred together");
    assert_eq!(
        score.y - text.y,
        Fx::from_px(crate::entity::POPUP_STACK_PX),
        "„Beleidigt!“ above the score (the „Bravo!“ stacking)"
    );
    // A chain: the third stomp of a chain is worth 400.
    let mut g = game(&world(40, &[(12, 12, 's')]));
    safe_step(&mut g);
    g.player.stomp_chain = 2;
    let s = first(&g, SWAN);
    drop_onto(&mut g, s);
    g.step(input("-"));
    assert_eq!(g.score(), 400);
}

#[test]
fn an_axe_from_the_quay_defeats_a_swan_for_200_with_beleidigt() {
    // D-081: from a deck the arc meets the swan before the water.
    let mut g = with_power(game(&world(40, &[(12, 10, 's')])), Power::Hackl);
    safe_step(&mut g);
    let mut events = hold(&mut g, "B", 1);
    events.extend(hold(&mut g, "-", 60));
    assert!(events.contains(&Event::AxeHit), "{events:?}");
    let d = first(&g, EntityKind::Defeated(EnemyKind::Schwan));
    assert_eq!(d.state, state::TUMBLE);
    assert_eq!(g.score(), 200);
    assert_eq!(g.player().stomp_chain, 0, "outside the chain");
    assert!(
        popups(&g)
            .iter()
            .any(|e| e.kind == EntityKind::Popup(Popup::Beleidigt))
    );
}

// ---------------------------------------------------------------- Gelse

/// One update of a lone Gelse `e` on `level`'s map, with the player's
/// centre x and top at `target` (px) and the screen at `screen` (px).
fn gelse_update(e: &mut Entity, level: &Level, target: (i32, i32), screen: (i32, i32)) {
    let (t, map) = (tuning(), TileMap::from_level(level));
    let ctx = UpdateCtx {
        map: &map,
        tuning: &t,
        level,
        player_cx: Fx::from_px(target.0),
        player_top: Fx::from_px(target.1),
        player_box: crate::physics::Aabb {
            x: Fx::from_px(target.0),
            y: Fx::from_px(target.1),
            w: Fx::ZERO,
            h: Fx::ZERO,
        },
        player_standing: None,
        screen_left: Fx::from_px(screen.0),
        screen_top: Fx::from_px(screen.1),
        globes: &[],
        stage_clock: 0,
        cars: &[],
        bills: 0,
    };
    e.update(&ctx);
}

/// The Gelse spawned at (`row`, `col`) of `level`, active.
fn spawned_gelse(level: &Level) -> Entity {
    let i = level
        .entities
        .iter()
        .position(|s| s.kind == crate::SpawnKind::Gelse)
        .unwrap();
    let mut e = Entity::from_spawn(&level.entities[i], i, &tuning()).unwrap();
    e.active = true;
    e
}

#[test]
fn a_gelse_is_centred_in_its_cell() {
    let level = world(40, &[(5, 12, 'g')]);
    let e = spawned_gelse(&level);
    assert_eq!(
        (e.x, e.y, e.w, e.h),
        (
            Fx::from_px(196),
            Fx::from_px(84),
            Fx::from_px(8),
            Fx::from_px(8)
        )
    );
    assert_eq!(e.state, state::HOVER);
}

#[test]
fn the_wait_counts_only_frames_fully_on_screen() {
    let t = tuning();
    let level = world(60, &[(5, 22, 'g')]);
    let mut e = spawned_gelse(&level);
    let start = (e.x, e.y);
    // Its right edge (360) past the screen (camera x 43): no count.
    for _ in 0..30 {
        gelse_update(&mut e, &level, (40, 150), (43, 0));
    }
    assert_eq!((e.state, e.count), (state::HOVER, 0));
    // Fully on screen: 60 frames of hovering, then the hunt.
    for k in 1..t.enemies.gelse_wait {
        gelse_update(&mut e, &level, (40, 150), (44, 0));
        assert_eq!((e.state, e.count), (state::HOVER, k));
        assert_eq!(e.pose(&t), Some(Pose::Wait));
    }
    assert_eq!((e.x, e.y), start, "hovers in place");
    // Off screen again (above it): the count pauses.
    gelse_update(&mut e, &level, (40, 150), (44, 85));
    assert_eq!(e.count, t.enemies.gelse_wait - 1);
    gelse_update(&mut e, &level, (40, 150), (44, 0));
    assert_eq!(e.state, state::HUNT, "after 60 frames on screen");
    assert_eq!((e.x, e.y), start, "the hunt starts on the next frame");
}

#[test]
fn the_hunt_accelerates_1_32_per_axis_up_to_0_75() {
    let t = tuning();
    let level = world(60, &[(3, 30, 'g')]);
    let mut e = spawned_gelse(&level);
    e.state = state::HUNT;
    // Target far left and below (above the water line).
    let target = (40, 150);
    let mut prev = (e.x, e.y);
    for k in 1..=40 {
        gelse_update(&mut e, &level, target, (0, 0));
        let v = Fx(8 * k).min(Fx(192));
        assert_eq!((e.vx, e.vy), (-v, v), "frame {k}");
        assert_eq!((e.x - prev.0, e.y - prev.1), (-v, v), "frame {k}");
        prev = (e.x, e.y);
    }
    assert_eq!(e.pose(&t), Some(Pose::Fly));
    assert!(e.facing_left());
}

/// [`world`] with its lake (`~`) of `kind`: swim water (`see_wasser`) or
/// the lethal `wolfgangsee_water`; the Gelse avoids both (D-079, E30).
fn world_of(kind: TileKind, width: usize, marks: &[(usize, usize, char)]) -> Level {
    let text = world_text(width, marks);
    let text = match kind {
        TileKind::Swim => text,
        TileKind::Water => text.replace("\"~\" = \"see_wasser\"", "\"~\" = \"wolfgangsee_water\""),
        other => panic!("not water: {other:?}"),
    };
    let level = Level::parse(&text).unwrap();
    assert_eq!(level.tile(20, 13).map(|t| t.kind), Some(kind));
    level
}

#[test]
fn a_gelse_never_enters_water_and_waits_at_the_water_line_above_a_diver() {
    for kind in [TileKind::Swim, TileKind::Water] {
        let level = world_of(kind, 40, &[(8, 10, 'g')]);
        let map = TileMap::from_level(&level);
        let mut e = spawned_gelse(&level);
        e.state = state::HUNT;
        // A diver 3 tiles down, 2 tiles to the right.
        let diver = (168 + 32, WATER_LINE + 48);
        for k in 0..300 {
            gelse_update(&mut e, &level, diver, (0, 0));
            assert!(!map.touches(&e.hitbox(), kind), "{kind:?} frame {k}");
        }
        assert_eq!(
            e.y + e.h,
            Fx::from_px(WATER_LINE),
            "{kind:?}: waits on the water line"
        );
        assert_eq!(e.vy, Fx::ZERO, "{kind:?}: its fall cut");
        let cx = e.x + e.w.div_int(2);
        assert!(
            (cx - Fx::from_px(diver.0)).abs() <= Fx::from_px(24),
            "{kind:?}: above her"
        );
    }
}

#[test]
fn a_gelse_is_stopped_at_the_side_of_a_water_tile() {
    for kind in [TileKind::Swim, TileKind::Water] {
        // A column of water above the surface at column 20 (rows 5–11).
        let mut marks: Vec<_> = (5..12).map(|r| (r, 20, '~')).collect();
        marks.push((8, 14, 'g'));
        let level = world_of(kind, 40, &marks);
        let map = TileMap::from_level(&level);
        let mut e = spawned_gelse(&level);
        e.state = state::HUNT;
        let mut stopped = false;
        for k in 0..200 {
            gelse_update(&mut e, &level, (500, 130), (0, 0));
            assert!(!map.touches(&e.hitbox(), kind), "{kind:?} frame {k}");
            stopped |= e.right() == Fx::from_px(320) && e.vx == Fx::ZERO;
        }
        assert!(stopped, "{kind:?}: cut at the column's left edge");
    }
}

#[test]
fn after_480_hunting_frames_it_leaves_harmless_and_is_removed_above_the_screen() {
    let t = tuning();
    let level = world(60, &[(5, 12, 'g')]);
    let mut e = spawned_gelse(&level);
    e.state = state::HUNT;
    for _ in 1..t.enemies.gelse_hunt {
        gelse_update(&mut e, &level, (200, 100), (0, 0));
        assert_eq!(e.state, state::HUNT);
    }
    gelse_update(&mut e, &level, (200, 100), (0, 0));
    assert_eq!(e.state, state::LEAVE, "after 480 frames");
    assert!(!e.harmful());
    assert_eq!(e.axe_hit(), AxeHit::Pass);
    assert_eq!(e.pose(&t), Some(Pose::Leave));
    let y = e.y;
    gelse_update(&mut e, &level, (200, 100), (0, 0));
    assert_eq!(
        (e.y, e.vx),
        (y - Fx::from_px(1), Fx::ZERO),
        "straight up at 1"
    );
    let mut frames = 0;
    while !e.removed {
        gelse_update(&mut e, &level, (200, 100), (0, 0));
        frames += 1;
        assert!(frames < 400);
    }
    let bottom = e.y + e.h;
    assert!(
        bottom <= Fx::ZERO && bottom > -Fx::ONE,
        "removed once its bottom is above the screen: {bottom:?}"
    );
}

#[test]
fn a_leaving_gelse_does_no_harm() {
    let mut g = game(&world(40, &[(9, 3, 'g')]));
    g.step(input("-"));
    let i = index(&g, GELSE);
    g.entities[i].state = state::LEAVE;
    let p = g.player().hitbox();
    g.entities[i].x = p.x;
    g.entities[i].y = p.y + Fx::from_px(4);
    let events = g.step(input("-")).to_vec();
    assert!(
        !events.contains(&Event::Hurt) && g.deaths() == 0,
        "{events:?}"
    );
}

#[test]
fn a_gelse_hurts_on_contact_and_a_stomp_is_100() {
    // Side contact: Normal Resi dies.
    let mut g = game(&world(40, &[(9, 8, 'g')]));
    g.step(input("-"));
    let i = index(&g, GELSE);
    let p = g.player().hitbox();
    g.entities[i].x = p.right() - Fx::from_px(2);
    g.entities[i].y = p.y + Fx::from_px(6);
    g.step(input("-"));
    assert_eq!(g.deaths(), 1);
    // From above: 100, no „Beleidigt!“.
    let mut g = game(&world(40, &[(5, 12, 'g')]));
    safe_step(&mut g);
    let e = first(&g, GELSE);
    drop_onto(&mut g, e);
    let events = g.step(input("-")).to_vec();
    assert!(events.contains(&Event::Stomped), "{events:?}");
    assert_eq!(g.score(), 100);
    assert_eq!(
        first(&g, EntityKind::Defeated(EnemyKind::Gelse)).pose(g.tuning()),
        Some(Pose::Squished)
    );
    assert!(
        !popups(&g)
            .iter()
            .any(|e| e.kind == EntityKind::Popup(Popup::Beleidigt))
    );
}

#[test]
fn an_axe_defeats_a_gelse_for_100() {
    let mut g = game(&world(40, &[(5, 12, 'g')]));
    safe_step(&mut g);
    let i = index(&g, GELSE);
    assert!(g.defeat_by_projectile(i));
    assert_eq!(g.score(), 100);
    assert_eq!(
        first(&g, EntityKind::Defeated(EnemyKind::Gelse)).state,
        state::TUMBLE
    );
}

// ---------------------------------------------------------- Regattaboot

/// A boat at (12, 20) on a dry lane (rows 9–15 of columns 4–59 free, a
/// floor at row 13 under it: feet y 208, like a swimmer's) for the trigger
/// and contact tests: hitbox x 324..348, centre 336.
fn boat_world(marks: &[(usize, usize, char)]) -> Game {
    let mut m: Vec<_> = (12..16)
        .flat_map(|r| (4..60).map(move |c| (r, c, if r >= 13 { '#' } else { '.' })))
        .collect();
    m.push((12, 20, 'b'));
    m.extend_from_slice(marks);
    game(&world(60, &m))
}

#[test]
fn a_boat_runs_from_its_masthead_to_its_keel_over_two_cells() {
    let g = boat_world(&[]);
    let b = first(&g, BOAT);
    assert_eq!((b.x, b.w), (Fx::from_px(324), Fx::from_px(24)));
    assert_eq!(b.y + b.h, Fx::from_px(WATER_LINE + 12), "keel");
    assert_eq!(
        b.y,
        Fx::from_px(DECK - 28),
        "masthead 28 px above the decks"
    );
    assert_eq!(
        (b.state, b.pose(g.tuning())),
        (state::PARKED, Some(Pose::Idle))
    );
    assert!(b.facing_left());
}

#[test]
fn a_boat_triggers_within_160_px_only_fully_on_screen() {
    let mut g = boat_world(&[]);
    for _ in 0..5 {
        stand(&mut g, 336 - 161, 208);
        safe_step(&mut g);
    }
    assert_eq!(first(&g, BOAT).state, state::PARKED, "161 px");
    // 100 px, but its right edge (348) 1 px past the screen.
    for _ in 0..5 {
        stand(&mut g, 236, 208);
        g.camera.x = 348 - 320 - 1;
        safe_step(&mut g);
    }
    assert_eq!(first(&g, BOAT).state, state::PARKED, "cut off");
    stand(&mut g, 336 - 160, 208);
    g.camera.x = 348 - 320;
    safe_step(&mut g);
    let b = first(&g, BOAT);
    assert_eq!((b.state, b.count), (state::TELEGRAPH, 1));
    assert_eq!(b.pose(g.tuning()), Some(Pose::Telegraph));
}

#[test]
fn a_boat_telegraphs_60_then_races_left_at_1_5_through_walls_and_off_screen() {
    // A wall in its lane (it ignores tiles; the lint would reject it).
    let marks: Vec<_> = (9..13).map(|r| (r, 8, '#')).collect();
    let mut g = boat_world(&marks);
    stand(&mut g, 250, 208);
    until(&mut g, "-", 10, |g| {
        first(g, BOAT).state == state::TELEGRAPH
    });
    let parked = first(&g, BOAT).x;
    // She stays far left on the quay, out of its way.
    g.player.x = Fx::from_px(8);
    g.player.y = Fx::from_px(DECK) - g.player.h;
    let mut telegraph = 1;
    loop {
        safe_step(&mut g);
        let b = first(&g, BOAT);
        if b.state != state::TELEGRAPH {
            assert_eq!(b.x, parked - Fx(384), "moves on the first frame after");
            break;
        }
        telegraph += 1;
    }
    assert_eq!(telegraph, 60);
    let mut prev = first(&g, BOAT).x;
    for _ in 0..400 {
        safe_step(&mut g);
        let Some(b) = g.entities().iter().find(|e| e.kind == BOAT).copied() else {
            break;
        };
        assert_eq!(b.x - prev, Fx(-384), "never stops, through the wall");
        assert_eq!(b.state, state::DRIVE);
        prev = b.x;
    }
    // Removed once its right edge is left of the screen.
    assert!(g.entities().iter().all(|e| e.kind != BOAT));
    let left = Fx::from_px(g.camera().x);
    let right = prev + Fx::from_px(24) - Fx(384);
    assert!(
        right <= left && right > left - Fx(384),
        "{right:?} {left:?}"
    );
}

#[test]
fn only_the_narrow_masthead_bounces_the_body_below_hurts() {
    // D-080 refinement: the drawn masthead is ~6 px wide, so only an 8×20
    // part around the mast bounces; the 24 px body below it hurts.
    let t = tuning();
    let g = boat_world(&[]);
    let b = first(&g, BOAT);
    let (mast, body) = regatta_parts(&b, &t).unwrap();
    assert_eq!((mast.x, mast.y), (b.x + Fx::from_px(8), b.y));
    assert_eq!((mast.w, mast.h), (Fx::from_px(8), Fx::from_px(20)));
    assert_eq!((body.x, body.y), (b.x, b.y + Fx::from_px(20)));
    assert_eq!((body.w, body.y + body.h), (b.w, b.y + b.h));
    // Falling beside the masthead (her hitbox over the hull's left 7 px):
    // no bounce in mid-air, she meets the sail and is hurt.
    let mut g = with_power(boat_world(&[]), Power::Big);
    safe_step(&mut g);
    g.player.invulnerable = 0;
    let b = first(&g, BOAT);
    drop_onto(&mut g, b);
    g.player.x = b.x + Fx::from_px(7) - g.player.w;
    let mut events = Vec::new();
    for _ in 0..12 {
        events.extend_from_slice(g.step(input("-")));
        if events.contains(&Event::Hurt) {
            break;
        }
    }
    assert!(events.contains(&Event::Hurt), "{events:?}");
    assert!(
        !events.contains(&Event::Stomped),
        "no bounce beside the mast"
    );
    assert!(
        g.player().y + g.player().h > b.y + Fx::from_px(20),
        "at the sail"
    );
}

#[test]
fn the_masthead_bounces_without_damage_or_points() {
    let mut g = with_power(boat_world(&[]), Power::Normal);
    safe_step(&mut g);
    let b = first(&g, BOAT);
    drop_onto(&mut g, b);
    let events = g.step(input("-")).to_vec();
    assert!(events.contains(&Event::Stomped), "{events:?}");
    assert!(events.contains(&Event::Bounced), "{events:?}");
    assert!(!events.contains(&Event::Hurt) && g.deaths() == 0);
    assert_eq!(g.score(), 0, "no points");
    assert_eq!(first(&g, BOAT).kind, BOAT, "not defeated");
    assert_eq!(g.player().y + g.player().h, b.y, "feet put on its top");
}

#[test]
fn a_boat_hurts_from_the_side_also_parked() {
    let mut g = with_power(boat_world(&[]), Power::Big);
    safe_step(&mut g);
    g.player.invulnerable = 0;
    let b = first(&g, BOAT);
    assert_eq!(b.state, state::PARKED);
    stand(&mut g, (b.x - Fx::from_px(4)).to_px(), 208);
    let events = g.step(input("-")).to_vec();
    assert!(events.contains(&Event::Hurt), "{events:?}");
    assert_eq!(g.player().power, Power::Normal);
}

#[test]
fn a_diver_13_px_down_passes_under_a_boat() {
    let g = boat_world(&[]);
    let b = first(&g, BOAT).hitbox();
    let cx = b.x + b.w.div_int(2);
    assert!(!swimmer_box(&g, cx, 13).overlaps(&b), "13 px down");
    assert!(swimmer_box(&g, cx, 11).overlaps(&b), "11 px down");
    assert!(swimmer_box(&g, cx, -8).overlaps(&b), "at the surface");
}

#[test]
fn an_axe_bounces_off_a_boat() {
    let mut g = boat_world(&[]);
    safe_step(&mut g);
    let i = index(&g, BOAT);
    assert!(!g.defeat_by_projectile(i));
    assert_eq!(g.score(), 0);
    assert_eq!(first(&g, BOAT).kind, BOAT);
}

#[test]
fn a_boats_lane_must_be_clear_of_decks_for_24_tiles() {
    let t = tuning();
    // A clear lane (water and a lake bed are fine): no problem.
    let clear = world(60, &[(12, 40, 'b')]);
    assert!(reisebus_lane_problems(&clear, &t).is_empty());
    // A deck tile 24 tiles left of its hitbox (column 16) is in the lane;
    // one at column 15 is not.
    let blocked = world(60, &[(12, 40, 'b'), (11, 16, '-')]);
    let p = reisebus_lane_problems(&blocked, &t);
    assert_eq!(p.len(), 1, "{p:?}");
    assert_eq!(
        p[0],
        "regattaboot at row 13, column 41: its lane (rows 10-13, columns 17-42) has a solid or one-way tile at row 12, column 17 (D-080: keep it clear for 24 tiles to the left, or to the level's left edge)"
    );
    let outside = world(60, &[(12, 40, 'b'), (11, 15, '-')]);
    assert!(reisebus_lane_problems(&outside, &t).is_empty());
    // A Reisebus keeps its message.
    let bus = world(60, &[(10, 40, 'r'), (9, 30, '#'), (10, 41, '#')]);
    assert!(reisebus_lane_problems(&bus, &t)[0].starts_with("reisebus at row 11"));
}
