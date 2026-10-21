//! The finale and the ending (D-107, `docs/game-design.md` → "Finale and
//! ending"): the `goal_stollen` (its cell, armed, 10 000 + „Zaunerstollen!",
//! the Serve), the flow after the clear (the ending on the journey's last
//! station in the real flow, else „Fortsetzung folgt …"), the ending's
//! phases with their frame counts and START rules, the run's Oblaten total
//! and the title's „durchgespielt" mark.

use std::sync::Arc;

use super::*;
use crate::entity::{EntityKind, Popup};
use crate::game::{ClearPhase, EndingPhase, goal_kind_armed};
use crate::level::{SpawnKind, TILE_PX};
use crate::physics::Aabb;
use crate::route::Route;
use crate::{Fx, Mode, Start};

/// A 20 × 12 side level: Resi at column 2, Oblaten at 5 and 7, a walking
/// Selfie-Tourist at column 0 behind her, the `goal_stollen` on the floor
/// at column 15 (row 10).
const FINALE: &str = r##"[meta]
id = "finale_test"
time_limit = 100

[legend]
"#" = "solid"
"P" = { entity = "player_start" }
"S" = { entity = "goal_stollen" }
"o" = { entity = "oblate" }
"t" = { entity = "selfie_tourist" }

[map]
grid = '''
....................
....................
....................
....................
....................
....................
....................
....................
....................
....................
t.P..o.o.......S....
####################
'''
"##;

const GOAL_COL: i32 = 15;
const GOAL_ROW: i32 = 10;

fn finale() -> Level {
    Level::parse(FINALE).unwrap()
}

/// A plain stage `id` (the goal 4 columns right of the start).
fn stage(id: &str) -> Level {
    let mut l = level(&["......G.", "..P.....", "########"]);
    l.meta.id = id.to_owned();
    l
}

/// A run whose last station (Café Zauner, station 9) is the finale level,
/// started there.
fn last_station_run(start: Start) -> Game {
    let mut levels: Vec<Level> = (0..8).map(|i| stage(&format!("s{i}"))).collect();
    levels.push(finale());
    let route = Arc::new(Route::new(levels).unwrap());
    Game::new_run(route, 8, &tuning(), 1, start)
}

/// Title → START → the whole intro → play.
fn start_playing(g: &mut Game) {
    hold(g, "S", 1);
    hold(g, "-", u32::from(tuning().game.intro_frames));
    assert_eq!(g.mode(), Mode::Playing);
}

/// Runs right into the `goal_stollen` (the goal sequence starts).
fn touch(g: &mut Game) -> Vec<Event> {
    until(g, "R", 600, |g| g.mode() == Mode::StageClear)
}

fn phase(g: &Game) -> ClearPhase {
    g.stage_clear().expect("in the goal sequence").phase
}

/// The Serve of a touch on the ground, then Tally and Clear, up to the
/// first frame after the goal sequence.
fn finish_goal_sequence(g: &mut Game) {
    until(g, "-", 2000, |g| g.mode() != Mode::StageClear);
}

fn ending_phase(g: &Game) -> (EndingPhase, u16, u8) {
    let e = g.ending().expect("in the ending");
    (e.phase, e.timer, e.page)
}

#[test]
fn the_trigger_is_the_goal_cell() {
    let g = Game::new(&finale(), &tuning(), 1);
    let t = Fx::from_px(TILE_PX);
    assert_eq!(
        g.goals(),
        &[Aabb {
            x: t.mul_int(GOAL_COL),
            y: t.mul_int(GOAL_ROW),
            w: t,
            h: t,
        }],
        "16×16, not a Maibaum's column"
    );
    assert_eq!(g.goal_spawn(0).unwrap().kind, SpawnKind::GoalStollen);
    // Passing right over the cell (her feet above its top) does nothing.
    let mut g = g;
    g.player.x = t.mul_int(GOAL_COL);
    g.player.y = t.mul_int(GOAL_ROW) - g.player.h - Fx::from_px(2);
    g.player.vy = -Fx::from_px(4);
    g.player.on_ground = false;
    let events = hold(&mut g, "-", 1);
    assert!(!events.contains(&Event::LevelComplete));
    assert_eq!(g.mode(), Mode::Playing);
}

#[test]
fn armed_only_without_an_undefeated_boss() {
    // The game asks `enemies::boss_undefeated` (E46); a level without an
    // Oberkellner is armed from the start.
    assert!(goal_kind_armed(SpawnKind::GoalStollen, false));
    assert!(!goal_kind_armed(SpawnKind::GoalStollen, true), "unarmed");
    assert!(goal_kind_armed(SpawnKind::Goal, true), "a Maibaum always");
    let g = Game::new(&finale(), &tuning(), 1);
    assert!(g.goal_armed(0));
    assert!(!g.goal_armed(1), "no such goal");
}

#[test]
fn touching_it_scores_10000_with_the_popup_and_starts_the_serve() {
    let t = tuning();
    let mut g = Game::new(&finale(), &t, 1);
    let events = touch(&mut g);
    assert!(events.contains(&Event::LevelComplete));
    assert_eq!(phase(&g), ClearPhase::Serve);
    // Two Oblaten on the way, then the Stollen.
    assert_eq!(g.score(), 2 * t.score.oblate + t.score.stollen);
    let popups: Vec<Popup> = g
        .entities()
        .iter()
        .filter_map(|e| match e.kind {
            EntityKind::Popup(p) => Some(p),
            _ => None,
        })
        .collect();
    assert!(popups.contains(&Popup::Score(10_000)), "{popups:?}");
    assert!(popups.contains(&Popup::Stollen), "{popups:?}");
    assert_eq!(Popup::Stollen.text_key(), Some("popup.stollen"));
    // Where she is: no move to a pole, no horizontal speed.
    let x = g.player().x;
    assert_eq!(g.player().vx, Fx::ZERO);
    // Enemies freeze and the timer stops.
    let tourist = |g: &Game| {
        g.entities()
            .iter()
            .find(|e| matches!(e.kind, EntityKind::Enemy(_)))
            .map(|e| (e.x, e.y))
    };
    let (before, time) = (tourist(&g), g.time_left());
    assert!(before.is_some());
    hold(&mut g, "R", 60);
    assert_eq!((tourist(&g), g.time_left()), (before, time));
    assert_eq!(g.player().x, x, "input ignored in the Serve");
}

#[test]
fn the_serve_lasts_90_frames_then_tally_and_clear() {
    let t = tuning();
    let mut g = Game::new(&finale(), &t, 1);
    touch(&mut g);
    assert!(g.player().on_ground);
    let time = g.time_left();
    hold(&mut g, "-", u32::from(t.goal.serve_frames) - 1);
    assert_eq!(phase(&g), ClearPhase::Serve);
    hold(&mut g, "-", 1);
    assert_eq!(phase(&g), ClearPhase::Tally, "after 90 frames");
    let score = g.score();
    until(&mut g, "-", 1000, |g| phase(g) == ClearPhase::Clear);
    assert_eq!(
        g.score(),
        score + u32::from(time) * t.score.time_bonus,
        "the unchanged tally"
    );
    assert_eq!(
        g.stage_clear().unwrap().bonus,
        u32::from(time) * t.score.time_bonus
    );
}

#[test]
fn in_the_air_she_falls_first_and_the_serve_counts_from_the_landing() {
    let t = tuning();
    let mut g = Game::new(&finale(), &t, 1);
    // Rising into the goal cell from beside it.
    let tile = Fx::from_px(TILE_PX);
    g.player.x = tile.mul_int(GOAL_COL);
    g.player.y = tile.mul_int(GOAL_ROW + 1) - g.player.h - Fx::from_px(12);
    g.player.vy = -Fx::from_px(2);
    g.player.on_ground = false;
    let events = hold(&mut g, "-", 1);
    assert!(events.contains(&Event::LevelComplete));
    assert_eq!(phase(&g), ClearPhase::Serve);
    assert!(!g.player().on_ground);
    let x = g.player().x;
    let mut airborne = 0;
    while !g.player().on_ground {
        hold(&mut g, "-", 1);
        airborne += 1;
        assert_eq!(g.stage_clear().unwrap().timer, 0, "not counted in the air");
        assert!(airborne < 120);
    }
    assert!(airborne > 10, "rose first, then fell: {airborne}");
    assert_eq!(g.player().x, x, "straight down");
    let floor = tile.mul_int(GOAL_ROW + 1);
    assert_eq!(g.player().y + g.player().h, floor, "on the floor");
    hold(&mut g, "-", u32::from(t.goal.serve_frames) - 1);
    assert_eq!(phase(&g), ClearPhase::Serve);
    hold(&mut g, "-", 1);
    assert_eq!(phase(&g), ClearPhase::Tally);
}

#[test]
fn playing_games_and_lone_levels_keep_the_m1_ending() {
    // `Start::Playing` on the last station: „Fortsetzung folgt …".
    let mut g = last_station_run(Start::Playing);
    touch(&mut g);
    until(&mut g, "-", 2000, |g| {
        g.stage_clear()
            .is_none_or(|c| c.phase == ClearPhase::ToBeContinued)
    });
    assert_eq!(phase(&g), ClearPhase::ToBeContinued);
    finish_goal_sequence(&mut g);
    assert!(matches!(g.mode(), Mode::Title { .. }));
    assert!(!g.cleared(), "set only by the ending");
    // A lone level in the real flow: the same.
    let mut g = Game::new_at(&finale(), &tuning(), 1, Start::Title);
    start_playing(&mut g);
    touch(&mut g);
    until(&mut g, "-", 2000, |g| {
        g.stage_clear()
            .is_none_or(|c| c.phase == ClearPhase::ToBeContinued)
    });
    assert_eq!(phase(&g), ClearPhase::ToBeContinued);
    finish_goal_sequence(&mut g);
    assert!(matches!(g.mode(), Mode::Title { .. }) && !g.cleared());
}

#[test]
fn a_finale_level_before_the_last_station_leads_on_to_the_map() {
    // As built (E47): the ending is tied to the journey's last station,
    // not to the goal kind; elsewhere the route flow is unchanged.
    let route = Arc::new(Route::new(vec![finale(), stage("b")]).unwrap());
    let mut g = Game::new_run(route, 0, &tuning(), 1, Start::Title);
    start_playing(&mut g);
    touch(&mut g);
    finish_goal_sequence(&mut g);
    assert_eq!(g.mode(), Mode::RouteMap);
}

#[test]
fn the_real_flow_on_the_last_station_runs_the_ending_with_its_frame_counts() {
    let e = tuning().ending;
    let mut g = last_station_run(Start::Title);
    start_playing(&mut g);
    touch(&mut g);
    finish_goal_sequence(&mut g);
    assert_eq!(g.mode(), Mode::Ending);
    assert!(g.stage_clear().is_none() && g.route_map().is_none());
    assert_eq!(ending_phase(&g), (EndingPhase::Map, 0, 0));
    let phases = [
        (EndingPhase::Map, e.map_frames),
        (EndingPhase::Scene, e.scene_frames),
        (EndingPhase::Bill, e.bill_frames),
    ];
    for (i, &(p, frames)) in phases.iter().enumerate() {
        hold(&mut g, "-", u32::from(frames) - 1);
        assert_eq!(ending_phase(&g), (p, frames - 1, 0), "{p:?}");
        hold(&mut g, "-", 1);
        let next = phases.get(i + 1).map_or(EndingPhase::Credits, |n| n.0);
        assert_eq!(ending_phase(&g), (next, 0, 0), "after {p:?}");
    }
    for page in 0..e.credits_pages {
        assert_eq!(ending_phase(&g), (EndingPhase::Credits, 0, page));
        hold(&mut g, "-", u32::from(e.credits_page_frames));
    }
    assert_eq!(ending_phase(&g), (EndingPhase::Ende, 0, 0));
    let score = g.score();
    assert!(score > 10_000);
    hold(&mut g, "-", u32::from(e.end_frames) - 1);
    assert_eq!(g.mode(), Mode::Ending);
    assert_eq!(g.score(), score, "the run stays until the title");
    hold(&mut g, "-", 1);
    assert!(matches!(g.mode(), Mode::Title { frames: 0 }));
    assert!(g.ending().is_none());
    // The run reset, the mark set.
    assert_eq!((g.score(), g.oblaten(), g.oblaten_total()), (0, 0, 0));
    assert_eq!(g.lives(), tuning().game.lives_start);
    assert_eq!(g.station(), Some(8), "the run's first stage");
    assert!(g.cleared());
}

#[test]
fn start_skips_each_phase_after_its_delay_and_a_page_at_a_time() {
    let e = tuning().ending;
    let mut g = last_station_run(Start::Title);
    start_playing(&mut g);
    touch(&mut g);
    finish_goal_sequence(&mut g);
    // A START held from the clear is no new press.
    let mut expect = vec![(EndingPhase::Scene, 0)];
    expect.push((EndingPhase::Bill, 0));
    expect.push((EndingPhase::Credits, 0));
    for page in 1..e.credits_pages {
        expect.push((EndingPhase::Credits, page));
    }
    expect.push((EndingPhase::Ende, 0));
    for (next_phase, next_page) in expect {
        let (p, _, page) = ending_phase(&g);
        let delay = match p {
            EndingPhase::Bill => e.bill_start_frames,
            EndingPhase::Ende => e.end_start_frames,
            _ => e.start_ignore_frames,
        };
        // Presses before the phase is `delay` frames old are ignored
        // (released on the frame before, so the press at `delay` is new).
        for f in 1..delay {
            let press = f % 2 == 1 && f + 1 < delay;
            hold(&mut g, if press { "S" } else { "-" }, 1);
            let (now, timer, now_page) = ending_phase(&g);
            assert_eq!((now, timer, now_page), (p, f, page), "frame {f}");
        }
        hold(&mut g, "S", 1);
        assert_eq!(ending_phase(&g), (next_phase, 0, next_page), "skip {p:?}");
    }
    // Ende: START from frame 60, then the title with the mark.
    hold(&mut g, "-", u32::from(e.end_start_frames) - 2);
    hold(&mut g, "S", 1);
    assert_eq!(
        ending_phase(&g),
        (EndingPhase::Ende, 59, 0),
        "one frame early"
    );
    hold(&mut g, "-", 1);
    hold(&mut g, "S", 1);
    assert!(matches!(g.mode(), Mode::Title { .. }));
    assert!(g.cleared());
}

#[test]
fn a_held_start_is_no_press_and_jump_is_tracked() {
    let e = tuning().ending;
    let mut g = last_station_run(Start::Title);
    start_playing(&mut g);
    touch(&mut g);
    until(&mut g, "-", 2000, |g| g.mode() == Mode::Ending);
    // START held across many frames: one press at most, and that one
    // came before the delay.
    hold(&mut g, "S", u32::from(e.map_frames) - 1);
    assert_eq!(ending_phase(&g).0, EndingPhase::Map);
    hold(&mut g, "J", 1);
    assert!(g.player().jump_held, "JUMP tracked (D-025)");
    hold(&mut g, "-", 1);
    assert!(!g.player().jump_held);
    assert_eq!(ending_phase(&g).0, EndingPhase::Scene);
}

#[test]
fn the_title_mark_survives_new_runs_and_start_plays_again() {
    let mut g = last_station_run(Start::Title);
    start_playing(&mut g);
    touch(&mut g);
    until(&mut g, "-", 6000, |g| {
        matches!(g.mode(), Mode::Title { .. })
    });
    assert!(g.cleared());
    let marked = g.state_hash();
    // START starts a new run exactly as before.
    hold(&mut g, "S", 1);
    assert!(matches!(g.mode(), Mode::StageIntro { .. }));
    assert_eq!(g.station(), Some(8));
    assert!(g.cleared(), "for the session");
    // Game over and continue keep it too.
    hold(&mut g, "-", u32::from(tuning().game.intro_frames));
    g.lives = 1;
    g.player.y = g.map().height_fx();
    until(&mut g, "-", 400, |g| {
        matches!(g.mode(), Mode::GameOver { .. })
    });
    hold(&mut g, "-", u32::from(tuning().game.game_over_frames));
    hold(&mut g, "S", 1);
    assert!(g.cleared());
    // Hashed.
    let mut fresh = last_station_run(Start::Title);
    hold(&mut fresh, "-", 1);
    assert_ne!(marked, fresh.state_hash());
}

#[test]
fn the_oblaten_total_never_wraps_and_resets_with_the_score_on_a_continue() {
    let t = tuning();
    let mut g = Game::new(&finale(), &t, 1);
    g.oblaten = 99;
    g.oblaten_total = 99;
    until(&mut g, "R", 200, |g| g.oblaten_total() == 100);
    assert_eq!(g.oblaten(), 0, "the HUD counter wraps at 100");
    assert_eq!(g.oblaten_total(), 100, "the total doesn't");
    let h = g.state_hash();
    g.oblaten_total += 1;
    assert_ne!(g.state_hash(), h, "hashed");
    g.oblaten_total -= 1;
    // A death keeps it (like the score).
    g.player.y = g.map().height_fx();
    until(&mut g, "-", 400, |g| {
        g.mode() == Mode::Playing && g.deaths() == 1
    });
    assert_eq!(g.oblaten_total(), 100);
    // Game over → continue: reset with the score.
    g.lives = 1;
    g.player.y = g.map().height_fx();
    until(&mut g, "-", 400, |g| {
        matches!(g.mode(), Mode::GameOver { .. })
    });
    assert_eq!(g.oblaten_total(), 100);
    hold(&mut g, "-", u32::from(t.game.game_over_frames));
    hold(&mut g, "S", 1);
    assert_eq!((g.score(), g.oblaten_total()), (0, 0));
}

#[test]
fn the_oblaten_total_carries_over_to_the_next_stage() {
    let route = Arc::new(Route::new(vec![finale(), stage("b")]).unwrap());
    let mut g = Game::new_run(route, 0, &tuning(), 1, Start::Title);
    start_playing(&mut g);
    touch(&mut g);
    assert_eq!(g.oblaten_total(), 2);
    finish_goal_sequence(&mut g);
    assert_eq!(g.mode(), Mode::RouteMap);
    hold(&mut g, "-", 1);
    hold(&mut g, "S", 1); // skip to Wait
    hold(&mut g, "-", 1);
    hold(&mut g, "S", 1); // the next stage
    assert_eq!(g.station(), Some(1));
    assert_eq!(g.oblaten_total(), 2);
}

#[test]
fn the_armed_goal_wins_over_a_hurt_in_the_same_frame() {
    let t = tuning();
    let mut g = Game::new(&finale(), &t, 1);
    hold(&mut g, "-", 1);
    let tile = Fx::from_px(TILE_PX);
    let goal_x = tile.mul_int(GOAL_COL);
    // The tourist stands across the goal cell's left edge, Resi just left
    // of the cell, walking in: she meets both in one frame.
    let i = g
        .entities
        .iter()
        .position(|e| matches!(e.kind, EntityKind::Enemy(_)))
        .unwrap();
    let e = &mut g.entities[i];
    e.x = goal_x - Fx::from_px(4);
    e.y = tile.mul_int(GOAL_ROW + 1) - e.h;
    g.player.x = goal_x - g.player.w;
    g.player.vx = Fx::from_px(1);
    let events = hold(&mut g, "R", 1);
    assert!(events.contains(&Event::LevelComplete), "{events:?}");
    assert!(!events.contains(&Event::Died) && !events.contains(&Event::Hurt));
    assert_eq!(g.mode(), Mode::StageClear);
    assert_eq!(g.deaths(), 0);
}
