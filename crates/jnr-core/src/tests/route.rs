//! The multi-stage flow (D-042) and the route map (D-043): stage clear →
//! map → next stage with carry-over, START skips, the teaser after the last
//! playable stage, continue and title reset per stage.

use std::sync::Arc;

use super::*;
use crate::game::ClearPhase;
use crate::player::Power;
use crate::route::{self, MapPhase, Route, STATIONS};
use crate::{Fx, Mode, Start};

/// A short stage with id `id`: the goal 4 columns right of the start,
/// a spike pit further left is out of reach (the start faces right).
fn stage(id: &str, time_limit: u32) -> Level {
    let mut l = level(&["......G.", "..P.....", "########"]);
    l.meta.id = id.to_owned();
    l.meta.time_limit = time_limit;
    l
}

/// A stage that kills: spikes right of the start.
fn deadly(id: &str) -> Level {
    let mut l = level(&["..........", "..P.....G.", "####^^####"]);
    l.meta.id = id.to_owned();
    l
}

fn run(levels: Vec<Level>, stage: usize, start: Start) -> Game {
    Game::new_run(
        Arc::new(Route::new(levels).unwrap()),
        stage,
        &tuning(),
        1,
        start,
    )
}

fn intro_frames() -> u32 {
    u32::from(tuning().game.intro_frames)
}

/// Title → START → the whole intro → play.
fn start_playing(g: &mut Game) {
    hold(g, "S", 1);
    hold(g, "-", intro_frames());
    assert_eq!(g.mode(), Mode::Playing);
}

/// Runs right into the goal and waits until the goal sequence is over.
fn clear_stage(g: &mut Game) {
    until(g, "R", 600, |g| g.mode() == Mode::StageClear);
    until(g, "-", 2000, |g| g.mode() != Mode::StageClear);
}

fn map(g: &Game) -> route::RouteMap {
    *g.route_map().expect("on the route map")
}

#[test]
fn stage_clear_shows_the_map_then_the_next_stage_with_carry_over() {
    let t = tuning();
    let mut g = run(vec![stage("a", 300), stage("b", 120)], 0, Start::Title);
    assert_eq!(g.station(), Some(0));
    start_playing(&mut g);
    // Zuckerschock and some score/Oblaten to carry over.
    g.player.set_power(Power::Big, &t.player);
    g.player.transform = 0;
    g.oblaten = 7;
    clear_stage(&mut g);
    assert_eq!(g.mode(), Mode::RouteMap);
    assert!(g.stage_clear().is_none());
    let m = map(&g);
    assert_eq!(
        (m.phase, m.timer, m.from, m.walked, m.teaser),
        (MapPhase::Arrive, 0, 0, 0, false)
    );
    let (score, lives) = (g.score(), g.lives());
    assert!(score > 0);

    // Arrive: `map.arrive_frames`, Resi on the cleared station.
    hold(&mut g, "-", u32::from(t.map.arrive_frames) - 1);
    assert_eq!(map(&g).phase, MapPhase::Arrive);
    assert_eq!(map(&g).resi().0, STATIONS[0].dot);
    hold(&mut g, "-", 1);
    assert_eq!(map(&g).phase, MapPhase::Walk);
    // Walk: 1 px per frame along the path.
    let len = route::path_len(0) as u32;
    hold(&mut g, "-", 10);
    assert_eq!(map(&g).walked, 10);
    hold(&mut g, "-", len - 11);
    assert_eq!(map(&g).phase, MapPhase::Walk);
    hold(&mut g, "-", 1);
    assert_eq!(map(&g).phase, MapPhase::Wait);
    assert_eq!(map(&g).resi().0, STATIONS[1].dot);
    // Wait: on its own after `map.wait_frames`.
    hold(&mut g, "-", u32::from(t.map.wait_frames) - 1);
    assert_eq!(g.mode(), Mode::RouteMap);
    let events = hold(&mut g, "-", 1);
    assert!(events.is_empty(), "no Respawned: nobody died ({events:?})");
    assert_eq!(
        g.mode(),
        Mode::StageIntro {
            timer: t.game.intro_frames
        }
    );

    // Stage b: carried over score, lives, Oblaten, power; new timer.
    assert_eq!(g.level().meta.id, "b");
    assert_eq!(g.station(), Some(1));
    assert_eq!((g.score(), g.lives(), g.oblaten()), (score, lives, 7));
    assert_eq!(g.player().power, Power::Big);
    assert_eq!(g.player().transform, 0, "no growing freeze");
    assert_eq!(g.time_left(), 120);
    assert_eq!(g.checkpoint(), None);
    assert!(g.route_map().is_none());
    let start = g.level().player_start();
    assert_eq!(
        g.player().y + g.player().h,
        Fx::from_px((start.row + 1) * 16)
    );
    hold(&mut g, "-", intro_frames());
    assert_eq!(g.mode(), Mode::Playing);
    hold(&mut g, "R", 10);
    assert!(g.player().x > Fx::from_px(2 * 16 + 2), "plays stage b");
}

#[test]
fn start_skips_arrive_and_walk_and_continues_from_wait() {
    let mut g = run(vec![stage("a", 300), stage("b", 300)], 0, Start::Title);
    start_playing(&mut g);
    clear_stage(&mut g);
    hold(&mut g, "-", 5);
    // A START press in Arrive: Resi is on the next station at once.
    hold(&mut g, "S", 1);
    let m = map(&g);
    assert_eq!((m.phase, m.timer), (MapPhase::Wait, 0));
    assert_eq!(i32::from(m.walked), route::path_len(0));
    assert_eq!(m.resi().0, STATIONS[1].dot);
    // Held START is no new press.
    hold(&mut g, "S", 20);
    assert_eq!(g.mode(), Mode::RouteMap);
    hold(&mut g, "-", 1);
    hold(&mut g, "S", 1);
    assert!(matches!(g.mode(), Mode::StageIntro { .. }));
    assert_eq!(g.level().meta.id, "b");

    // START during the walk skips too.
    let mut g = run(vec![stage("a", 300), stage("b", 300)], 0, Start::Title);
    start_playing(&mut g);
    clear_stage(&mut g);
    until(&mut g, "-", 100, |g| map(g).phase == MapPhase::Walk);
    hold(&mut g, "-", 3);
    hold(&mut g, "S", 1);
    assert_eq!(map(&g).phase, MapPhase::Wait);
}

#[test]
fn last_playable_stage_ends_with_the_teaser_and_a_reset_title() {
    let t = tuning();
    let mut g = run(vec![stage("a", 300), stage("b", 300)], 1, Start::Title);
    // `--level b`: the title starts the run at b.
    assert_eq!(g.level().meta.id, "b");
    start_playing(&mut g);
    g.player.set_power(Power::Big, &t.player);
    g.player.transform = 0;
    clear_stage(&mut g);
    let m = map(&g);
    assert_eq!(
        (m.from, m.teaser),
        (1, true),
        "St. Gilgen → Brunnwinkl: no level"
    );
    // START skips the walk to the teaser, where it is ignored.
    hold(&mut g, "S", 1);
    assert_eq!(map(&g).phase, MapPhase::Teaser);
    hold(&mut g, "-", 1);
    hold(&mut g, "S", 1);
    hold(&mut g, "-", u32::from(t.map.teaser_frames) - 3);
    assert_eq!(map(&g).phase, MapPhase::Teaser);
    hold(&mut g, "-", 1);
    assert_eq!(g.mode(), Mode::Title { frames: 0 });
    // The run is reset: its first stage, score 0, full lives, Normal.
    assert_eq!(g.level().meta.id, "b");
    assert_eq!(
        (g.score(), g.lives(), g.oblaten()),
        (0, t.game.lives_start, 0)
    );
    assert_eq!(g.player().power, Power::Normal);
    assert!(g.route_map().is_none() && g.stage_clear().is_none());
}

#[test]
fn title_after_the_teaser_starts_the_route_again() {
    let t = tuning();
    let mut g = run(vec![stage("a", 300), stage("b", 300)], 0, Start::Title);
    start_playing(&mut g);
    clear_stage(&mut g);
    hold(&mut g, "S", 1);
    hold(&mut g, "-", 1);
    hold(&mut g, "S", 1);
    hold(&mut g, "-", intro_frames());
    assert_eq!((g.level().meta.id.as_str(), g.mode()), ("b", Mode::Playing));
    clear_stage(&mut g);
    until(&mut g, "-", 2000, |g| {
        matches!(g.mode(), Mode::Title { .. })
    });
    assert_eq!(g.level().meta.id, "a", "back to the first stage");
    assert_eq!(g.station(), Some(0));
    assert_eq!(g.score(), 0);
    assert_eq!(g.lives(), t.game.lives_start);
    hold(&mut g, "S", 1);
    assert!(matches!(g.mode(), Mode::StageIntro { .. }));
    assert_eq!(g.level().meta.id, "a");
}

#[test]
fn continue_restarts_the_current_stage() {
    let t = tuning();
    let mut g = run(vec![stage("a", 300), deadly("b")], 0, Start::Title);
    start_playing(&mut g);
    clear_stage(&mut g);
    hold(&mut g, "S", 1);
    hold(&mut g, "-", 1);
    hold(&mut g, "S", 1);
    assert_eq!(g.level().meta.id, "b");
    // Die until the game is over.
    until(&mut g, "R", 5000, |g| {
        matches!(g.mode(), Mode::GameOver { .. })
    });
    hold(&mut g, "-", u32::from(t.game.game_over_frames));
    hold(&mut g, "S", 1);
    assert!(matches!(g.mode(), Mode::StageIntro { .. }));
    assert_eq!(g.level().meta.id, "b", "same stage, not Salzburg");
    assert_eq!(
        (g.score(), g.lives(), g.oblaten()),
        (0, t.game.lives_start, 0)
    );
    assert_eq!(g.player().power, Power::Normal);
    assert_eq!(g.time_left(), 300);
}

#[test]
fn death_in_a_later_stage_respawns_in_it_as_normal() {
    let t = tuning();
    let mut g = run(vec![stage("a", 300), deadly("b")], 0, Start::Title);
    start_playing(&mut g);
    g.player.set_power(Power::Big, &t.player);
    g.player.transform = 0;
    clear_stage(&mut g);
    hold(&mut g, "S", 1);
    hold(&mut g, "-", 1);
    hold(&mut g, "S", 1);
    hold(&mut g, "-", intro_frames());
    let lives = g.lives();
    until(&mut g, "R", 600, |g| {
        matches!(g.mode(), Mode::StageIntro { .. })
    });
    assert_eq!(g.level().meta.id, "b");
    assert_eq!(g.lives(), lives - 1);
    assert_eq!(g.player().power, Power::Normal);
}

#[test]
fn playing_start_and_lone_levels_keep_the_m1_ending() {
    // `Start::Playing` with a route: no map, „Fortsetzung folgt …", title.
    let mut g = run(vec![stage("a", 300), stage("b", 300)], 0, Start::Playing);
    until(&mut g, "R", 600, |g| g.mode() == Mode::StageClear);
    until(&mut g, "-", 2000, |g| {
        g.stage_clear()
            .is_some_and(|c| c.phase == ClearPhase::ToBeContinued)
    });
    until(&mut g, "-", 400, |g| matches!(g.mode(), Mode::Title { .. }));
    assert_eq!(g.level().meta.id, "a");

    // Started at a later stage, the M1 ending is the same.
    let mut g = run(vec![stage("a", 300), stage("b", 300)], 1, Start::Playing);
    until(&mut g, "R", 600, |g| g.mode() == Mode::StageClear);
    until(&mut g, "-", 2400, |g| {
        matches!(g.mode(), Mode::Title { .. })
    });
    assert_eq!((g.level().meta.id.as_str(), g.station()), ("b", Some(1)));

    // A lone level in the real flow: the same.
    let mut g = Game::new_at(&stage("a", 300), &tuning(), 1, Start::Title);
    assert_eq!((g.station(), g.route().is_none()), (None, true));
    start_playing(&mut g);
    until(&mut g, "R", 600, |g| g.mode() == Mode::StageClear);
    until(&mut g, "-", 2000, |g| {
        g.stage_clear()
            .is_some_and(|c| c.phase == ClearPhase::ToBeContinued)
    });
}

#[test]
fn the_last_station_has_no_map_but_the_ending() {
    let levels: Vec<Level> = (0..9).map(|i| stage(&format!("s{i}"), 300)).collect();
    let mut g = run(levels, 8, Start::Title);
    assert_eq!(g.station(), Some(8));
    start_playing(&mut g);
    until(&mut g, "R", 600, |g| g.mode() == Mode::StageClear);
    until(&mut g, "-", 2000, |g| g.mode() != Mode::StageClear);
    // D-107: no map and no „Fortsetzung folgt …" after the last station.
    assert_eq!(g.mode(), Mode::Ending);
    assert!(g.route_map().is_none() && g.stage_clear().is_none());
}

#[test]
fn after_the_finale_the_title_starts_the_route_again() {
    let levels: Vec<Level> = (0..9).map(|i| stage(&format!("s{i}"), 5)).collect();
    let mut g = run(levels, 0, Start::Title);
    hold(&mut g, "S", 1);
    for i in 0..8 {
        assert_eq!(g.station(), Some(i));
        hold(&mut g, "-", intro_frames());
        clear_stage(&mut g);
        assert_eq!(g.mode(), Mode::RouteMap);
        hold(&mut g, "-", 1);
        hold(&mut g, "S", 1); // skip to Wait
        hold(&mut g, "-", 1);
        hold(&mut g, "S", 1); // next stage
    }
    assert_eq!(g.station(), Some(8));
    hold(&mut g, "-", intro_frames());
    clear_stage(&mut g);
    assert_eq!(g.mode(), Mode::Ending, "no map after the finale (D-107)");
    until(&mut g, "-", 4000, |g| g.mode() != Mode::Ending);
    assert!(matches!(g.mode(), Mode::Title { .. }));
    assert_eq!((g.station(), g.level().meta.id.as_str()), (Some(0), "s0"));
    assert_eq!(g.score(), 0);
    assert!(g.cleared());
}

#[test]
fn jump_held_through_the_map_is_no_press_in_the_next_stage() {
    let mut g = run(vec![stage("a", 300), stage("b", 300)], 0, Start::Title);
    start_playing(&mut g);
    clear_stage(&mut g);
    hold(&mut g, "J", 1);
    hold(&mut g, "JS", 1);
    hold(&mut g, "J", 1);
    hold(&mut g, "JS", 1);
    assert!(matches!(g.mode(), Mode::StageIntro { .. }));
    let events = hold(&mut g, "J", intro_frames() + 20);
    assert!(!events.contains(&Event::Jumped), "{events:?}");
}

#[test]
fn route_runs_are_deterministic_and_hash_the_map() {
    let make = || run(vec![stage("a", 300), stage("b", 300)], 0, Start::Title);
    let mut a = make();
    start_playing(&mut a);
    clear_stage(&mut a);
    let mut b = a.clone();
    hold(&mut a, "-", 50);
    hold(&mut b, "-", 50);
    assert_eq!(a.state_hash(), b.state_hash());
    let mut c = a.clone();
    hold(&mut c, "S", 1);
    hold(&mut a, "-", 1);
    assert_ne!(a.state_hash(), c.state_hash(), "the skip is in the hash");
    // Same frame, same everything, but another stage: different hashes.
    let x = run(vec![stage("a", 300), stage("b", 300)], 0, Start::Title);
    let y = run(vec![stage("a", 300), stage("b", 300)], 1, Start::Title);
    assert_ne!(x.state_hash(), y.state_hash());
    // The stage is clamped to the route.
    let z = run(vec![stage("a", 300)], 5, Start::Title);
    assert_eq!(z.station(), Some(0));
}
