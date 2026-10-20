//! The Offiziere of Stage 8 (game-design.md → "Stage 8: Bad Ischl",
//! entities and "New mechanics"; D-096, D-097): the drill on the stage
//! clock (march, „Habt Acht!", „Präsentiert!"), a file in step, the sabre
//! zone checked before the stomp, 200 points with „Abtreten!" on every
//! defeat, and the thrown Hackl.

use super::*;
use crate::enemies::{Pose, sabre_zone, state};
use crate::hackl::{AxeHit, axe};
use crate::player::Power;
use crate::{DeathCause, EnemyKind, Entity, EntityKind, Fx, Mode, Popup, Start};

/// Legend of the test worlds (`.` air, `P` start).
const LEGEND: &str = r##"
"#" = "solid"
"B" = "brick"
"o" = { entity = "offizier" }
"l" = { entity = "offizier", lead = true }
"1" = { entity = "offizier", phase = 100 }
"T" = { entity = "selfie_tourist", still = true }
"w" = { entity = "nordic_walker" }
"P" = { entity = "player_start" }
"G" = { entity = "goal" }
"##;

/// A 14-row world `width` columns wide: ground in rows 12–13 (feet at y
/// 192), the start at column 2 (row 11) unless `marks` has a `P`, plus
/// `marks` as (row, col, char) (`.` in rows 12–13 digs a hole).
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
        "[meta]\nid = \"m6\"\ntime_limit = 300\n[legend]\n{LEGEND}\n[map]\ngrid = '''\n{}\n'''\n",
        grid.join("\n")
    );
    Level::parse(&text).unwrap()
}

fn game(level: &Level) -> Game {
    Game::new(level, &tuning(), 1)
}

const OFFIZIER: EntityKind = EntityKind::Enemy(EnemyKind::Offizier);
const DEFEATED: EntityKind = EntityKind::Defeated(EnemyKind::Offizier);

fn all(g: &Game, kind: EntityKind) -> Vec<Entity> {
    g.entities()
        .iter()
        .filter(|e| e.kind == kind)
        .copied()
        .collect()
}

fn first(g: &Game, kind: EntityKind) -> Entity {
    all(g, kind)
        .first()
        .copied()
        .unwrap_or_else(|| panic!("no {kind:?}"))
}

fn has_popup(g: &Game, p: Popup) -> bool {
    g.entities().iter().any(|e| e.kind == EntityKind::Popup(p))
}

/// Steps once with `token` and the player kept invulnerable.
fn safe(g: &mut Game, token: &str) -> Vec<Event> {
    g.player.invulnerable = u8::MAX;
    g.step(input(token)).to_vec()
}

/// Steps until the first officer is in `st` (player kept safe).
fn until_state(g: &mut Game, st: u8) {
    for _ in 0..300 {
        safe(g, "-");
        if first(g, OFFIZIER).state == st {
            return;
        }
    }
    panic!("state {st} not reached");
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

/// An officer at (11, 20): feet (328, 192), far right of the start.
fn officer_world(marks: &[(usize, usize, char)]) -> Game {
    let mut m = vec![(11, 20, 'o')];
    m.extend_from_slice(marks);
    game(&world(40, &m))
}

// ------------------------------------------------------------ the drill

#[test]
fn the_drill_marches_168_halts_24_and_presents_48_on_the_stage_clock() {
    let t = tuning();
    let e = &t.enemies;
    let mut g = officer_world(&[]);
    assert_eq!(g.stage_clock(), 0);
    let mut states = Vec::new();
    for n in 1..=241u32 {
        safe(&mut g, "-");
        assert_eq!(g.stage_clock(), n, "one tick per frame of play");
        let o = first(&g, OFFIZIER);
        // Frame n of play sees the clock n − 1.
        assert_eq!(u32::from(o.count), (n - 1) % 240, "count = drill frame");
        states.push((o.state, o.pose(&t).unwrap()));
    }
    let (march, attention) = (
        usize::from(e.offizier_march),
        usize::from(e.offizier_attention),
    );
    assert_eq!((march, attention, 240 - march - attention), (168, 24, 48));
    assert!(
        states[..168]
            .iter()
            .all(|&s| s == (state::WALK, Pose::March))
    );
    assert!(
        states[168..192]
            .iter()
            .all(|&s| s == (state::ATTENTION, Pose::Habtacht))
    );
    assert!(
        states[192..240]
            .iter()
            .all(|&s| s == (state::PRESENT, Pose::Praesentiert))
    );
    assert_eq!(states[240].0, state::WALK, "the next drill");
}

#[test]
fn a_phase_shifts_the_drill() {
    let mut g = game(&world(40, &[(11, 20, 'o'), (11, 30, '1')]));
    for _ in 0..50 {
        safe(&mut g, "-");
        let o = all(&g, OFFIZIER);
        let (a, b) = (o[0], o[1]);
        assert!(a.active && b.active);
        assert_eq!((a.count + 100) % 240, b.count);
    }
}

#[test]
fn he_marches_at_half_a_pixel_and_halts_towards_the_player() {
    let t = tuning();
    // The player right of him: he faces right at activation.
    let mut g = game(&world(40, &[(11, 6, 'o'), (11, 14, 'P')]));
    safe(&mut g, "-");
    let mut prev = first(&g, OFFIZIER);
    assert_eq!(prev.dir, 1, "faces the player");
    for n in 2..=240 {
        safe(&mut g, "-");
        let o = first(&g, OFFIZIER);
        let moved = o.x - prev.x;
        if o.state == state::WALK {
            assert_eq!(moved, t.enemies.offizier_speed, "frame {n}");
            assert_eq!(o.vx, Fx(128));
        } else {
            assert_eq!((moved, o.vx), (Fx::ZERO, Fx::ZERO), "halted, frame {n}");
        }
        prev = o;
    }
    // 167 march frames after the first: 83.5 px; 84 px per drill.
    assert_eq!(
        first(&g, OFFIZIER).x - Entity::at_feet(OFFIZIER, Fx::from_px(104), Fx::ZERO, [12, 22]).x,
        Fx::from_px(84)
    );
}

#[test]
fn he_turns_at_walls_ledges_and_enemies_on_foot() {
    // A wall at column 16, 4 tiles left of him.
    let mut g = officer_world(&[(11, 16, '#')]);
    for _ in 0..120 {
        safe(&mut g, "-");
    }
    let o = first(&g, OFFIZIER);
    assert_eq!(o.dir, 1, "turned at the wall");
    assert!(o.x >= Fx::from_px(17 * 16));
    // A hole in the ground at column 17: he turns at its edge and never
    // falls.
    let mut g = officer_world(&[(12, 17, '.'), (13, 17, '.')]);
    for _ in 0..240 {
        safe(&mut g, "-");
        assert_eq!(first(&g, OFFIZIER).y + Fx::from_px(22), Fx::from_px(192));
    }
    let o = first(&g, OFFIZIER);
    assert_eq!(o.dir, 1, "turned at the ledge");
    // A still tourist at column 17 (an enemy on foot).
    let mut g = officer_world(&[(11, 17, 'T')]);
    for _ in 0..120 {
        safe(&mut g, "-");
    }
    assert_eq!(first(&g, OFFIZIER).dir, 1, "turned at the tourist");
    // And a Nordic-Walker turns at him (he is on foot).
    // Both face the player on the right; the faster walker behind him
    // runs into him and turns back.
    let mut g = game(&world(40, &[(11, 20, 'o'), (11, 19, 'w'), (11, 35, 'P')]));
    let mut turned = false;
    for _ in 0..120 {
        safe(&mut g, "-");
        // (Once turned it walks off the screen and is removed.)
        turned |= all(&g, EntityKind::Enemy(EnemyKind::NordicWalker))
            .iter()
            .any(|w| w.dir == -1);
    }
    assert!(turned, "the walker turned at the officer");
}

#[test]
fn a_file_stays_in_step_after_staggered_activation() {
    // Two officers far apart: the second wakes only when the camera gets
    // there. Both run on the stage clock, so they are at the same drill
    // frame from his first frame on; a file one cell apart keeps its
    // spacing.
    let level = world(100, &[(11, 30, 'o'), (11, 31, 'o'), (11, 55, 'o')]);
    let mut g = game(&level);
    let mut both = 0;
    for _ in 0..600 {
        safe(&mut g, "RB");
        let o: Vec<Entity> = g
            .entities()
            .iter()
            .filter(|e| e.kind == OFFIZIER && e.active)
            .copied()
            .collect();
        if o.len() == 3 {
            both += 1;
            assert!(o[2].timer < o[0].timer, "the far one woke later");
            assert!(
                o.iter()
                    .all(|e| (e.count, e.state) == (o[0].count, o[0].state))
            );
        }
        if o.len() >= 2 && o[0].spawn == Some(1) && o[1].spawn == Some(2) {
            assert_eq!(
                o[1].x - o[0].x,
                Fx::from_px(16),
                "the file keeps its spacing"
            );
        }
    }
    assert!(
        both > 60,
        "all three were active together for {both} frames"
    );
}

// -------------------------------------------------------- the sabre zone

#[test]
fn the_sabre_zone_is_8_by_16_above_him_only_while_presenting() {
    let t = tuning();
    let mut g = officer_world(&[]);
    for n in 1..=240 {
        safe(&mut g, "-");
        let o = first(&g, OFFIZIER);
        match sabre_zone(&o, &t) {
            Some(z) => {
                assert_eq!(o.state, state::PRESENT, "frame {n}");
                assert_eq!((z.w, z.h), (Fx::from_px(8), Fx::from_px(16)));
                assert_eq!(z.x + z.w.div_int(2), o.x + o.w.div_int(2), "centred");
                assert_eq!(z.y + z.h, o.y, "from his hitbox top up");
            }
            None => assert_ne!(o.state, state::PRESENT, "frame {n}"),
        }
    }
    assert_eq!(
        sabre_zone(&Entity::at_feet(OFFIZIER, Fx::ZERO, Fx::ZERO, [12, 22]), &t),
        None
    );
}

#[test]
fn falling_onto_a_presenting_officer_hurts_instead_of_stomping() {
    let mut g = officer_world(&[]);
    until_state(&mut g, state::PRESENT);
    let o = first(&g, OFFIZIER);
    drop_onto(&mut g, o);
    g.player.invulnerable = 0;
    let events = g.step(input("-")).to_vec();
    assert!(!events.contains(&Event::Stomped), "{events:?}");
    assert_eq!(g.death_cause(), Some(DeathCause::Hit), "Normal dies");
    assert_eq!(g.score(), 0);
    assert_eq!(all(&g, OFFIZIER).len(), 1, "not stomped");
}

#[test]
fn the_sabre_hurts_from_the_side() {
    let t = tuning();
    let mut g = officer_world(&[]);
    g.player.set_power(Power::Big, &t.player);
    g.player.transform = 0;
    until_state(&mut g, state::PRESENT);
    // Hang in the air left of the blade, touching it but not his hitbox.
    let z = sabre_zone(&first(&g, OFFIZIER), &t).unwrap();
    g.player.x = z.x - g.player.w + Fx::ONE;
    g.player.y = z.y - g.player.h + Fx::from_px(4);
    g.player.vy = Fx::ZERO;
    g.player.on_ground = false;
    g.player.invulnerable = 0;
    assert!(!g.player.hitbox().overlaps(&first(&g, OFFIZIER).hitbox()));
    let events = g.step(input("-")).to_vec();
    assert!(events.contains(&Event::Hurt), "{events:?}");
    assert_eq!(g.player().power, Power::Normal, "Zuckerschock");
}

#[test]
fn an_axe_passes_the_sabre_zone() {
    let t = tuning();
    let mut g = officer_world(&[]);
    until_state(&mut g, state::PRESENT);
    let z = sabre_zone(&first(&g, OFFIZIER), &t).unwrap();
    // A flying axe resting in the middle of the blade (no speed).
    let [w, h] = t.hackl.axe_hitbox;
    let mut a = Entity::at_feet(
        EntityKind::ThrownHackl,
        z.x + z.w.div_int(2),
        z.y + Fx::from_px(h),
        [w, h],
    );
    a.state = axe::FLY;
    g.entities.push(a);
    let events = safe(&mut g, "-");
    let axe = first(&g, EntityKind::ThrownHackl);
    assert!(axe.hitbox().overlaps(&z), "inside the zone");
    assert!(!axe.hitbox().overlaps(&first(&g, OFFIZIER).hitbox()));
    assert!(!events.contains(&Event::AxeHit), "{events:?}");
    assert_eq!(all(&g, OFFIZIER).len(), 1);
}

// ------------------------------------------------------------- defeats

/// A stomp in `st` (march or „Habt Acht!"): 200, „Abtreten!" above the
/// score popup.
fn stomp_in(st: u8) {
    let mut g = officer_world(&[]);
    until_state(&mut g, st);
    let o = first(&g, OFFIZIER);
    drop_onto(&mut g, o);
    let events = g.step(input("-")).to_vec();
    assert!(events.contains(&Event::Stomped), "{st}: {events:?}");
    assert!(!events.contains(&Event::Hurt));
    assert_eq!(g.score(), 200, "chain start 200");
    assert_eq!(first(&g, DEFEATED).state, state::SQUISHED);
    let p: Vec<Entity> = g
        .entities()
        .iter()
        .filter(|e| matches!(e.kind, EntityKind::Popup(_)))
        .copied()
        .collect();
    let score = p
        .iter()
        .find(|e| e.kind == EntityKind::Popup(Popup::Score(200)))
        .unwrap();
    let abtreten = p
        .iter()
        .find(|e| e.kind == EntityKind::Popup(Popup::Abtreten))
        .unwrap();
    assert_eq!(abtreten.x, score.x);
    assert_eq!(
        score.y - abtreten.y,
        Fx::from_px(crate::entity::POPUP_STACK_PX),
        "„Abtreten!“ above the score"
    );
}

#[test]
fn a_stomp_in_the_march_scores_200_and_abtreten() {
    assert_eq!(EnemyKind::Offizier.chain_start(), 1);
    stomp_in(state::WALK);
}

#[test]
fn a_stomp_during_habt_acht_works() {
    stomp_in(state::ATTENTION);
}

#[test]
fn a_file_of_three_chains_200_400_800() {
    let mut g = game(&world(40, &[(11, 18, 'l'), (11, 19, 'o'), (11, 20, 'o')]));
    safe(&mut g, "-");
    let mut events = Vec::new();
    for _ in 0..3 {
        let o = first(&g, OFFIZIER);
        drop_onto(&mut g, o);
        events.extend(g.step(input("-")).to_vec());
    }
    assert_eq!(events.iter().filter(|&&e| e == Event::Stomped).count(), 3);
    assert_eq!(g.score(), 200 + 400 + 800);
    assert_eq!(all(&g, DEFEATED).len(), 3);
}

#[test]
fn a_bumped_block_knocks_him_off_with_abtreten() {
    let mut g = game(&world(40, &[(9, 12, 'B'), (8, 12, 'o'), (11, 12, 'P')]));
    let events = until(&mut g, "J", 60, |g| !all(g, DEFEATED).is_empty());
    assert!(events.contains(&Event::BlockBumped));
    assert_eq!(first(&g, DEFEATED).state, state::TUMBLE);
    assert_eq!(g.score(), tuning().score.bump_enemy);
    assert!(has_popup(&g, Popup::Abtreten));
}

#[test]
fn an_axe_defeats_him_for_200_and_abtreten() {
    assert_eq!(EnemyKind::Offizier.axe_hit(), AxeHit::Defeat);
    let mut g = officer_world(&[]);
    g.player.set_power(Power::Hackl, &tuning().player);
    g.player.transform = 0;
    g.entities.retain(|e| e.kind != OFFIZIER);
    // About 6 tiles ahead, where the arc is back at hand height.
    let o = Entity::at_feet(OFFIZIER, Fx::from_px(150), Fx::from_px(192), [12, 22]);
    g.entities.push(o);
    let mut events = hold(&mut g, "B", 1);
    events.extend(hold(&mut g, "-", 40));
    assert!(events.contains(&Event::AxeHit), "{events:?}");
    assert_eq!(first(&g, DEFEATED).state, state::TUMBLE);
    assert_eq!(g.score(), 200);
    assert!(has_popup(&g, Popup::Abtreten));
}

#[test]
fn he_sinks_like_a_walker() {
    assert!(EnemyKind::Offizier.sinks_in_swim_water());
    assert_eq!(EnemyKind::Offizier.sprite(), "offizier");
    assert_eq!(Popup::Abtreten.text_key(), Some("popup.abtreten"));
}

// ------------------------------------------------------ the stage clock

#[test]
fn frozen_paused_and_dying_frames_dont_advance_the_clock() {
    let t = tuning();
    // Frozen: a transformation.
    let mut g = officer_world(&[]);
    safe(&mut g, "-");
    g.set_player_power(Power::Big);
    assert!(g.player.transform > 0);
    let clock = g.stage_clock();
    let count = first(&g, OFFIZIER).count;
    while g.player.transform > 0 {
        safe(&mut g, "-");
    }
    assert_eq!(g.stage_clock(), clock, "frozen");
    assert_eq!(first(&g, OFFIZIER).count, count, "the drill froze too");
    // Paused (the real flow: title, intro, then START pauses).
    let mut g = Game::new_at(&world(40, &[(11, 20, 'o')]), &t, 1, Start::Title);
    hold(&mut g, "S", 1);
    hold(&mut g, "-", u32::from(t.game.intro_frames) + 10);
    assert_eq!(g.mode(), Mode::Playing);
    // The intro's last step switches to play: 10 frames of play followed.
    assert_eq!(g.stage_clock(), 10, "intro frames don't count");
    hold(&mut g, "S", 1);
    hold(&mut g, "-", 1);
    assert_eq!(g.mode(), Mode::Paused);
    let clock = g.stage_clock();
    hold(&mut g, "-", 50);
    assert_eq!(g.stage_clock(), clock, "paused");
}

#[test]
fn a_respawn_restarts_the_clock_and_the_drill() {
    let mut g = officer_world(&[(12, 4, '.'), (13, 4, '.')]);
    for _ in 0..100 {
        safe(&mut g, "-");
    }
    assert_eq!(first(&g, OFFIZIER).count, 99);
    // Walk into the pit: dying frames don't count, the respawn restarts.
    until(&mut g, "R", 200, |g| matches!(g.mode(), Mode::Dying { .. }));
    let clock = g.stage_clock();
    hold(&mut g, "-", 10);
    assert_eq!(g.stage_clock(), clock, "dying");
    let events = until(&mut g, "-", 200, |g| g.mode() == Mode::Playing);
    assert!(events.contains(&Event::Respawned));
    assert_eq!(g.stage_clock(), 0);
    safe(&mut g, "-");
    let o = first(&g, OFFIZIER);
    assert_eq!((o.count, o.state), (0, state::WALK), "the drill restarted");
    assert_eq!(
        o.x,
        Entity::at_feet(OFFIZIER, Fx::from_px(328), Fx::ZERO, [12, 22]).x - Fx(128)
    );
}

// ------------------------------------------- the grey box and its replays

/// Plays `replay` (text) on `00_m6_offiziere` and calls `check` after
/// every step.
fn play_grey_box(replay: &str, mut check: impl FnMut(&Game)) {
    let level = Level::parse(include_str!(
        "../../../../content/levels/00_m6_offiziere.level"
    ))
    .unwrap();
    let r = crate::replay::Replay::parse(replay).unwrap();
    let outcome = r.run(&level, &tuning(), |g| check(g));
    assert!(r.check(&outcome).is_empty(), "{:?}", r.check(&outcome));
}

/// The active officers of `g` by spawn index.
fn active_officers(g: &Game) -> Vec<Entity> {
    g.entities()
        .iter()
        .filter(|e| e.kind == OFFIZIER && e.active)
        .copied()
        .collect()
}

#[test]
fn the_grey_box_files_stay_in_step() {
    // Every active officer (all with phase 0) is at the same drill frame
    // in every frame, file B's three wake on three different frames, and
    // file A presents sabres under Resi without touching her.
    let mut woke = std::collections::BTreeMap::new();
    let mut presented_under_her = false;
    play_grey_box(
        include_str!("../../../../tests/replays/m6_offiziere_file_in_step.replay"),
        |g| {
            let o = active_officers(g);
            assert!(
                o.iter()
                    .all(|e| (e.count, e.state) == (o[0].count, o[0].state))
            );
            for e in &o {
                woke.entry(e.spawn).or_insert(g.frame());
            }
            let feet = g.player().y + g.player().h;
            presented_under_her |= o.iter().any(|e| {
                sabre_zone(e, g.tuning()).is_some_and(|z| {
                    z.y > feet && feet == Fx::from_px(144) && e.x < Fx::from_px(400)
                })
            });
        },
    );
    let level = Level::parse(include_str!(
        "../../../../content/levels/00_m6_offiziere.level"
    ))
    .unwrap();
    let file_b: Vec<u32> = level
        .entities
        .iter()
        .enumerate()
        .filter(|(_, s)| s.kind == crate::level::SpawnKind::Offizier && (44..=46).contains(&s.col))
        .map(|(i, _)| woke[&Some(i as u16)])
        .collect();
    assert_eq!(file_b.len(), 3);
    assert!(
        file_b[0] < file_b[1] && file_b[1] < file_b[2],
        "staggered activation: {file_b:?}"
    );
    assert!(presented_under_her, "file A presents below the terrace");
}

#[test]
fn the_grey_box_habt_acht_stomp_lands_in_the_halt() {
    let mut stomped_in = Vec::new();
    play_grey_box(
        include_str!("../../../../tests/replays/m6_offiziere_habtacht_stomp.replay"),
        |g| {
            if g.events().contains(&Event::Stomped) {
                stomped_in.extend(active_officers(g).first().map(|e| e.state));
            }
        },
    );
    assert_eq!(stomped_in, vec![state::ATTENTION]);
}

#[test]
fn the_grey_box_sabre_hurt_comes_from_the_blade() {
    let mut from_blade = 0;
    play_grey_box(
        include_str!("../../../../tests/replays/m6_offiziere_sabre_hurt.replay"),
        |g| {
            if g.events().contains(&Event::Hurt) {
                // Her Big hitbox at the contact (the shrink keeps the
                // feet) met a presented blade and no officer's body.
                let p = g.player().hitbox();
                let big_h = Fx::from_px(g.tuning().player.hitbox_h_big);
                let big = crate::physics::Aabb {
                    y: p.y + p.h - big_h,
                    h: big_h,
                    ..p
                };
                let o = active_officers(g);
                assert!(o.iter().any(|e| {
                    e.state == state::PRESENT
                        && sabre_zone(e, g.tuning()).is_some_and(|z| z.overlaps(&big))
                }));
                assert!(!o.iter().any(|e| e.hitbox().overlaps(&big)), "not his body");
                from_blade += 1;
            }
        },
    );
    assert_eq!(from_blade, 1);
}
