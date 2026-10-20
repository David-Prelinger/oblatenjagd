//! Swimming (D-076; game-design.md → "Stage 6: Wolfgangsee", "Swimming"
//! and the "New mechanics" row): the chest-point rule, the surface band and
//! rise cap, DOWN dives, the entry clamp, the bed as ground, contacts while
//! swimming, the jump-out, no throw, items, axes and walking enemies lost
//! in swim water, lethal water unchanged. The feel numbers (stroke rise,
//! max sink, swim speed, floating, jump-out onto a deck) are in `feel.rs`.

use super::*;
use crate::enemies::state;
use crate::hackl::axe;
use crate::player::Power;
use crate::{DeathCause, EnemyKind, EntityKind, Fx, Mode};

/// Surface y of [`pool`], whole px.
const SURFACE: i32 = 128;

fn game(level: &Level, power: Power) -> Game {
    let mut g = Game::new(level, &tuning(), 1);
    g.player.set_power(power, &tuning().player);
    g.player.transform = 0;
    g
}

fn chest_y(g: &Game) -> Fx {
    g.player().y + Fx::from_px(tuning().swim.chest_px)
}

fn count(g: &Game, kind: EntityKind) -> usize {
    g.entities().iter().filter(|e| e.kind == kind).count()
}

fn index(g: &Game, kind: EntityKind) -> usize {
    g.entities().iter().position(|e| e.kind == kind).unwrap()
}

/// Steps with no input until she floats (vy 0 at the surface, 5 frames).
fn settle(g: &mut Game) {
    let mut calm = 0;
    for _ in 0..200 {
        g.step(InputFrame::NONE);
        let p = g.player();
        if p.swimming && p.vy == Fx::ZERO && !p.on_ground {
            calm += 1;
            if calm == 5 {
                return;
            }
        } else {
            calm = 0;
        }
    }
    panic!("never floated");
}

const TAUBE: EntityKind = EntityKind::Enemy(EnemyKind::Taube);
const FIAKER: EntityKind = EntityKind::Enemy(EnemyKind::Fiaker);

/// Holds the Taube `i` still with its hitbox's top left at (`x`, `top`) px
/// for the next step (it flies a sine to the left otherwise).
fn pin_taube(g: &mut Game, i: usize, x: i32, top: i32) {
    let t = tuning().enemies;
    let e = &mut g.entities[i];
    e.x = Fx::from_px(x) - t.taube_speed.mul_int(i32::from(e.dir));
    e.count = t.taube_period - 1;
    e.base_y = Fx::from_px(top);
}

#[test]
fn she_swims_while_her_chest_point_is_in_swim_water() {
    // Walking off the quay (top 128) into the water at column 4: the chest
    // point (centre x, 8 px below the top) decides, in every form.
    for power in Power::ALL {
        let mut g = game(&pool(20, 4..16, &[]), power);
        let mut swam = false;
        for _ in 0..80 {
            let p = *g.player();
            let (cx, cy) = (p.x + p.w.div_int(2), p.y + Fx::from_px(8));
            let in_swim =
                cy >= Fx::from_px(SURFACE) && (4..16).contains(&crate::physics::tile_of(cx));
            g.step(input("R"));
            assert_eq!(
                g.player().swimming,
                in_swim,
                "{power:?} chest at {cx:?},{cy:?}"
            );
            swam |= in_swim;
        }
        assert!(swam, "{power:?}");
    }
}

#[test]
fn strokes_never_lift_the_chest_above_the_surface() {
    let mut g = game(&pool(20, 4..16, &[(8, 8, 'S')]), Power::Normal);
    settle(&mut g);
    hold(&mut g, "D", 60);
    let mut capped = 0;
    for i in 0..300 {
        let below_band = chest_y(&g) >= Fx::from_px(SURFACE + tuning().swim.surface_band);
        let token = if i % 6 == 0 && below_band { "J" } else { "-" };
        let events = g.step(input(token)).to_vec();
        assert!(chest_y(&g) >= Fx::from_px(SURFACE), "frame {i}");
        assert!(
            !events.contains(&Event::Jumped),
            "frame {i}: a stroke, not a jump"
        );
        if chest_y(&g) == Fx::from_px(SURFACE) {
            capped += 1;
            assert_eq!(g.player().vy, Fx::ZERO, "stops on the water line");
            break;
        }
    }
    assert_eq!(capped, 1, "reached the water line");
    // On the line, a press jumps out.
    assert!(g.step(input("J")).contains(&Event::Jumped));
}

#[test]
fn a_press_in_the_surface_band_jumps_out_and_below_it_strokes() {
    let band = tuning().swim.surface_band;
    for (depth, jumps) in [
        (0, true),
        (band - 1, true),
        (band, false),
        (band + 5, false),
    ] {
        let mut g = game(&pool(20, 4..16, &[(8, 8, 'S')]), Power::Normal);
        settle(&mut g);
        // Place the chest `depth` px below the line, at rest.
        g.player.y = Fx::from_px(SURFACE + depth - 8);
        g.player.vy = Fx::ZERO;
        let events = g.step(input("J")).to_vec();
        assert_eq!(events.contains(&Event::Jumped), jumps, "{depth} px");
        assert_eq!(events.contains(&Event::Stroke), !jumps, "{depth} px");
    }
}

#[test]
fn down_dives_and_without_it_she_floats() {
    let mut g = game(&pool(20, 4..16, &[(8, 8, 'S')]), Power::Big);
    settle(&mut g);
    let y = g.player().y;
    hold(&mut g, "-", 60);
    assert_eq!(g.player().y, y, "floats");
    hold(&mut g, "D", 30);
    let deeper = g.player().y;
    assert!(deeper > y + Fx::from_px(8), "dives below the band");
    // Below the band she keeps sinking without DOWN (no buoyancy) ...
    hold(&mut g, "-", 5);
    assert!(g.player().y > deeper && g.player().vy > Fx::ZERO);
    // ... down to the bed, the ground under water.
    let events = until(&mut g, "-", 200, |g| g.player().on_ground);
    assert!(events.contains(&Event::Landed));
    assert_eq!(g.player().feet().1, Fx::from_px(208));
    assert!(g.player().swimming);
}

/// A drop from a one-way ledge (row 3, top y 48) into the water, holding
/// `token`; returns the game after the first swimming step and that step's
/// number.
fn drop_in(token: &str, press_before_entry: Option<u32>) -> (Game, u32) {
    let marks = [(3, 7, '-'), (3, 8, '-'), (3, 9, '-'), (2, 8, 'P')];
    let mut g = game(&pool(20, 4..16, &marks), Power::Normal);
    hold(&mut g, "-", 2);
    g.step(input("DJ"));
    let entry = press_before_entry.map(|k| entry_step() - k);
    for n in 1..100 {
        let token = match entry {
            Some(press) if n == press => "J",
            _ => token,
        };
        let events = g.step(input(token)).to_vec();
        assert!(
            !events.contains(&Event::Jumped) && !events.contains(&Event::Stroke),
            "{n}"
        );
        if g.player().swimming {
            return (g, n);
        }
    }
    panic!("never entered the water");
}

/// The step of [`drop_in`] on which she enters the water (no presses).
fn entry_step() -> u32 {
    let marks = [(3, 7, '-'), (3, 8, '-'), (3, 9, '-'), (2, 8, 'P')];
    let mut g = game(&pool(20, 4..16, &marks), Power::Normal);
    hold(&mut g, "-", 2);
    g.step(input("DJ"));
    (1..100)
        .find(|_| {
            g.step(InputFrame::NONE);
            g.player().swimming
        })
        .unwrap()
}

#[test]
fn entering_clamps_the_fall_and_clears_buffer_and_coyote() {
    // Falling 5 tiles in, she hits the water at max fall speed (4 px).
    let (g, _) = drop_in("D", None);
    assert_eq!(g.player().vy, tuning().swim.max_sink, "clamped to max sink");
    let (g, _) = drop_in("-", None);
    assert_eq!(
        g.player().vy,
        Fx::ZERO,
        "the splash brakes the fall: floats"
    );
    // A JUMP press buffered in the air never becomes a stroke or jump-out.
    for k in 1..=3 {
        let (mut g, n) = drop_in("-", Some(k));
        assert_eq!(n, entry_step());
        assert_eq!((g.player().jump_buffer, g.player().coyote), (0, 0), "{k}");
        let events = hold(&mut g, "-", 20);
        assert!(!events.contains(&Event::Jumped) && !events.contains(&Event::Stroke));
    }
}

#[test]
fn the_lake_bed_is_ground_at_swim_speed_and_jump_strokes() {
    let mut g = game(&pool(40, 4..36, &[(12, 6, 'S')]), Power::Normal);
    hold(&mut g, "-", 5);
    assert!(g.player().on_ground && g.player().swimming);
    hold(&mut g, "RB", 60);
    assert_eq!(g.player().vx, Fx(320), "walks at swim speed, RUN no faster");
    assert!(g.player().on_ground);
    let events = g.step(input("RBJ")).to_vec();
    assert!(events.contains(&Event::Stroke) && !events.contains(&Event::Jumped));
    let events = until(&mut g, "R", 120, |g| g.player().on_ground);
    assert!(events.contains(&Event::Landed));
}

#[test]
fn swim_water_never_kills_and_lethal_water_still_does() {
    let mut g = game(&pool(20, 4..16, &[(8, 8, 'S')]), Power::Normal);
    let mut events = hold(&mut g, "-", 300);
    events.extend(hold(&mut g, "D", 300));
    assert!(!events.contains(&Event::Died));
    assert!(g.player().on_ground && g.player().swimming);
    // The same pool with `wolfgangsee_water` (D-050): death once sunk in.
    let lethal: Vec<(usize, usize, char)> = (4..16)
        .flat_map(|c| (8..13).map(move |r| (r, c, 'w')))
        .collect();
    let mut g = game(&pool(20, 4..16, &lethal), Power::Normal);
    until(&mut g, "R", 120, |g| g.mode() != Mode::Playing);
    assert_eq!(g.death_cause(), Some(DeathCause::Hazard));
    assert!(!g.player().swimming);
}

#[test]
fn every_enemy_contact_hurts_while_swimming() {
    // A Taube held still under water, a diver sinking onto it: a stomp in
    // the air, a hurt in the water (Zuckerschock → Normal, then Normal dies).
    for (power, hurt) in [(Power::Big, true), (Power::Normal, false)] {
        let mut g = game(&pool(20, 4..16, &[(8, 8, 'S'), (12, 12, 't')]), power);
        let i = index(&g, TAUBE);
        let x = g.player().x.to_px();
        pin_taube(&mut g, i, x, 176);
        settle(&mut g);
        let mut contact = None;
        for f in 0..120 {
            let prev_bottom = g.player().feet().1;
            pin_taube(&mut g, i, x, 176);
            let events = g.step(input("D")).to_vec();
            if events.contains(&Event::Hurt) || events.contains(&Event::Died) {
                // It was a stomp-shaped contact: falling, feet above its middle.
                assert!(prev_bottom <= Fx::from_px(181), "{power:?}");
                assert!(g.player().vy > Fx::ZERO || g.mode() != Mode::Playing);
                contact = Some((f, events));
                break;
            }
        }
        let (_, events) = contact.expect("touched the Taube");
        assert!(!events.contains(&Event::Stomped), "{power:?}: no stomp");
        assert_eq!(events.contains(&Event::Hurt), hurt, "{power:?}");
        assert_eq!(events.contains(&Event::Died), !hurt, "{power:?}");
        assert_eq!(count(&g, TAUBE), 1, "the Taube lives");
    }
}

#[test]
fn no_roof_bounce_while_swimming() {
    // A Fiaker parked on the lake bed (its roof 28 px up, y 180): diving
    // onto it hurts instead of bouncing.
    let mut g = game(&pool(20, 4..16, &[(8, 8, 'S'), (12, 7, 'f')]), Power::Big);
    let i = index(&g, FIAKER);
    let mut events = Vec::new();
    for _ in 0..200 {
        g.entities[i].state = state::PARKED;
        g.entities[i].count = 0;
        let e = g.step(input("D")).to_vec();
        let done = e.contains(&Event::Hurt) || e.contains(&Event::Stomped);
        events.extend(e);
        if done {
            break;
        }
    }
    assert!(events.contains(&Event::Hurt), "{events:?}");
    assert!(!events.contains(&Event::Stomped));
    assert_eq!(g.player().power, Power::Normal);
}

#[test]
fn a_jump_out_is_airborne_from_its_next_frame_and_can_stomp() {
    // A Taube held still above the water, its left edge 2 px right of the
    // floating swimmer: she jumps out straight up, drifts right at the top
    // and comes down on it.
    let mut g = game(
        &pool(20, 4..16, &[(8, 8, 'S'), (2, 12, 'u')]),
        Power::Normal,
    );
    let i = index(&g, TAUBE);
    let x = g.player().hitbox().right().to_px() + 2;
    pin_taube(&mut g, i, x, 104);
    settle(&mut g);
    pin_taube(&mut g, i, x, 104);
    assert!(g.step(input("J")).contains(&Event::Jumped));
    assert!(g.player().leaving_water && !g.player().on_ground);
    pin_taube(&mut g, i, x, 104);
    g.step(input("J"));
    assert!(!g.player().swimming, "airborne from the next frame");
    let mut events = Vec::new();
    for _ in 0..120 {
        if g.mode() != Mode::Playing || count(&g, TAUBE) == 0 {
            break;
        }
        pin_taube(&mut g, i, x, 104);
        let token = if g.player().vy < Fx::ZERO { "J" } else { "R" };
        events.extend_from_slice(g.step(input(token)));
    }
    assert!(events.contains(&Event::Stomped), "{events:?}");
    assert!(!events.contains(&Event::Died) && !events.contains(&Event::Stroke));
    assert_eq!(count(&g, TAUBE), 0, "stomped");
}

#[test]
fn run_throws_no_hackl_while_swimming() {
    let mut g = game(&pool(20, 4..16, &[(8, 8, 'S')]), Power::Hackl);
    settle(&mut g);
    for _ in 0..5 {
        assert!(!g.step(input("B")).contains(&Event::HacklThrown));
        g.step(InputFrame::NONE);
    }
    assert_eq!(count(&g, EntityKind::ThrownHackl), 0);
    // On the quay the same press throws.
    let mut g = game(&pool(20, 4..16, &[]), Power::Hackl);
    hold(&mut g, "-", 2);
    assert!(g.step(input("B")).contains(&Event::HacklThrown));
}

#[test]
fn axes_and_power_ups_are_lost_in_swim_water() {
    // An axe thrown from the quay lands in the water: removed, never stuck.
    let mut g = game(&pool(30, 4..26, &[]), Power::Hackl);
    hold(&mut g, "-", 2);
    g.step(input("B"));
    for _ in 0..80 {
        g.step(InputFrame::NONE);
        let stuck = g
            .entities()
            .iter()
            .any(|e| e.kind == EntityKind::ThrownHackl && e.state == axe::STUCK);
        assert!(!stuck);
    }
    assert_eq!(count(&g, EntityKind::ThrownHackl), 0, "lost in the water");
    // A Mozartkugel rolls off the quay into the water: lost at the touch.
    let mut g = game(&pool(20, 4..16, &[(5, 2, 'M')]), Power::Normal);
    until(&mut g, "J", 30, |g| {
        g.events().contains(&Event::BlockBumped)
    });
    let mut lowest = Fx::ZERO;
    for _ in 0..200 {
        g.step(InputFrame::NONE);
        match g
            .entities()
            .iter()
            .find(|e| e.kind == EntityKind::Mozartkugel)
        {
            Some(k) => lowest = lowest.max(k.y + k.h),
            None => break,
        }
    }
    assert_eq!(count(&g, EntityKind::Mozartkugel), 0);
    assert!(
        lowest < Fx::from_px(SURFACE + 16),
        "lost at the touch, not on the bed"
    );
    assert_eq!(g.player().power, Power::Normal);
}

#[test]
fn walking_enemies_are_lost_in_swim_water() {
    // A Selfie-Tourist walks left off the quay (columns 10+) into the water.
    let mut g = game(&pool(24, 4..10, &[(7, 13, 'W')]), Power::Normal);
    let tourist = EntityKind::Enemy(EnemyKind::SelfieTourist);
    let mut lowest = Fx::ZERO;
    for _ in 0..600 {
        g.step(InputFrame::NONE);
        match g.entities().iter().find(|e| e.kind == tourist) {
            Some(t) => lowest = lowest.max(t.y + t.h),
            None => break,
        }
    }
    assert_eq!(count(&g, tourist), 0, "lost in the water");
    assert!(
        lowest < Fx::from_px(SURFACE + 16),
        "at the touch, not on the bed"
    );
    assert_eq!(g.score(), 0, "no points");
}

#[test]
fn oblaten_in_swim_cells_are_collected_by_a_diver() {
    let mut g = game(
        &pool(20, 4..16, &[(8, 8, 'S'), (11, 8, 'o')]),
        Power::Normal,
    );
    settle(&mut g);
    assert_eq!(count(&g, EntityKind::Oblate), 1, "not lost in the water");
    let events = until(&mut g, "D", 120, |g| g.oblaten() == 1);
    assert!(events.contains(&Event::OblateCollected));
}

#[test]
fn head_bumps_under_water_work_as_in_the_air() {
    // A brick in the water two rows above her head on the bed.
    let mut g = game(
        &pool(20, 4..16, &[(12, 8, 'S'), (10, 8, 'B')]),
        Power::Normal,
    );
    hold(&mut g, "-", 5);
    let events = until(&mut g, "J", 60, |g| {
        g.events().contains(&Event::BlockBumped)
    });
    assert!(events.contains(&Event::Stroke));
}

#[test]
fn a_tap_jump_out_falls_back_in_and_floats_again() {
    let mut g = game(&pool(20, 4..16, &[(8, 8, 'S')]), Power::Normal);
    settle(&mut g);
    assert!(g.step(input("J")).contains(&Event::Jumped));
    // Presses in the air neither stroke nor buffer into the water.
    let mut events = Vec::new();
    for i in 0..60 {
        // Only while the next step starts out of the water (a press on the
        // step that enters it is a real press at the surface: a jump-out).
        let out = chest_y(&g) < Fx::from_px(SURFACE) || g.player().leaving_water;
        let press = i % 2 == 1 && out;
        events.extend_from_slice(g.step(input(if press { "J" } else { "-" })));
    }
    assert!(!events.contains(&Event::Stroke) && !events.contains(&Event::Jumped));
    settle(&mut g);
    assert!(chest_y(&g) >= Fx::from_px(SURFACE), "back in the water");
    assert!(!g.player().leaving_water);
}
// --------------------------- power changes while swimming (D-086, QA M5)

/// A floater's chest point stays in the surface band through a power change
/// and she keeps floating with no input (QA `qa_m5_shrink_while_floating`:
/// a hurt Zuckerschock kept her feet, so Normal's chest landed at surface
/// + 8, outside the `< 8` band, and she sank to the bed).
#[test]
fn a_power_change_while_floating_keeps_the_chest_point() {
    let band = Fx::from_px(tuning().swim.surface_band);
    for (from, to) in [
        (Power::Big, Power::Normal),
        (Power::Hackl, Power::Normal),
        (Power::Normal, Power::Big),
        (Power::Normal, Power::Hackl),
    ] {
        let mut g = game(&pool(20, 4..16, &[(8, 8, 'S')]), from);
        settle(&mut g);
        let chest = chest_y(&g);
        g.set_player_power(to);
        assert_eq!(g.player().power, to);
        assert_eq!(chest_y(&g), chest, "{from:?} -> {to:?}: the chest stays");
        g.player.transform = 0;
        hold(&mut g, "-", 120);
        let p = g.player();
        assert!(
            p.swimming && !p.on_ground,
            "{from:?} -> {to:?}: still floating"
        );
        assert!(
            chest_y(&g) - Fx::from_px(SURFACE) < band,
            "{from:?} -> {to:?}: in the surface band"
        );
        // ... so JUMP is a jump-out, not a stroke.
        let events = g.step(input("J")).to_vec();
        assert!(events.contains(&Event::Jumped), "{from:?} -> {to:?}");
    }
}

/// Standing on the lake bed, or with no room under her for a grown hitbox
/// just above it, a power change keeps the feet as on land.
#[test]
fn a_power_change_on_or_just_above_the_bed_keeps_the_feet() {
    let bed = Fx::from_px(208);
    // On the bed: both ways keep the feet on it.
    for (from, to) in [(Power::Big, Power::Normal), (Power::Normal, Power::Big)] {
        let mut g = game(&pool(20, 4..16, &[(8, 8, 'S')]), from);
        until(&mut g, "D", 300, |g| g.player().on_ground);
        g.set_player_power(to);
        assert_eq!(g.player().feet().1, bed, "{from:?} -> {to:?} on the bed");
    }
    // Sinking 4 px above the bed: growing by 8 px from the top would reach
    // into the bed, so the feet stay.
    let mut g = game(&pool(20, 4..16, &[(8, 8, 'S')]), Power::Normal);
    until(&mut g, "D", 300, |g| g.player().on_ground);
    g.player.y -= Fx::from_px(4);
    g.player.on_ground = false;
    g.set_player_power(Power::Big);
    assert_eq!(g.player().feet().1, bed - Fx::from_px(4));
    // On land nothing changed: the feet stay.
    let mut g = game(&pool(20, 4..16, &[]), Power::Big);
    hold(&mut g, "-", 5);
    let feet = g.player().feet().1;
    g.set_player_power(Power::Normal);
    assert_eq!(g.player().feet().1, feet);
}
