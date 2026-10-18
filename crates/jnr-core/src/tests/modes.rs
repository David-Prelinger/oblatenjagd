//! Flow modes of the real game (`Start::Title`): title, stage intro, pause,
//! intro after a death and after a continue (M1 task E4, D-025).

use super::*;
use crate::{DeathCause, Mode, Start};

fn spikes() -> Level {
    level(&["..........", "..P.......", "####^^####"])
}

fn titled(level: &Level) -> Game {
    Game::new_at(level, &tuning(), 1, Start::Title)
}

fn intro_frames() -> u32 {
    u32::from(tuning().game.intro_frames)
}

/// From the title screen into play: START, then the whole intro.
fn start_playing(g: &mut Game) {
    g.step(input("S"));
    hold(g, "-", intro_frames());
    assert_eq!(g.mode(), Mode::Playing);
}

#[test]
fn title_then_start_then_intro_then_playing() {
    let mut g = titled(&flat(30));
    assert_eq!(g.mode(), Mode::Title { frames: 0 });
    assert_eq!(g.start(), Start::Title);
    let p0 = *g.player();
    // Nothing moves on the title screen; it counts frames for blinking.
    hold(&mut g, "R", 30);
    assert_eq!(g.mode(), Mode::Title { frames: 30 });
    assert_eq!(*g.player(), p0);
    // A START held from before is not a press.
    hold(&mut g, "S", 1);
    assert_eq!(
        g.mode(),
        Mode::StageIntro {
            timer: tuning().game.intro_frames
        }
    );
    let time = g.time_left();
    hold(&mut g, "RS", intro_frames() - 1);
    assert_eq!(g.mode(), Mode::StageIntro { timer: 1 });
    assert_eq!(*g.player(), p0, "the intro stands still");
    assert_eq!(g.time_left(), time);
    hold(&mut g, "R", 1);
    assert_eq!(g.mode(), Mode::Playing);
    hold(&mut g, "R", 10);
    assert!(g.player().x > p0.x, "plays");
}

#[test]
fn death_shows_the_intro_before_play_resumes() {
    let t = tuning();
    let mut g = titled(&spikes());
    start_playing(&mut g);
    let mut died = false;
    for _ in 0..60 {
        if g.step(input("R")).contains(&Event::Died) {
            died = true;
            break;
        }
    }
    assert!(died);
    assert_eq!(g.death_cause(), Some(DeathCause::Hazard));
    let events = hold(&mut g, "-", u32::from(t.game.death_frames));
    assert!(events.contains(&Event::Respawned));
    assert!(matches!(g.mode(), Mode::StageIntro { .. }));
    assert_eq!(g.lives(), 2);
    hold(&mut g, "-", intro_frames() - 1);
    assert!(matches!(g.mode(), Mode::StageIntro { timer: 1 }));
    hold(&mut g, "-", 1);
    assert_eq!(g.mode(), Mode::Playing);
}

#[test]
fn jump_held_through_the_intro_does_not_jump() {
    let mut g = titled(&flat(30));
    g.step(input("S"));
    hold(&mut g, "J", intro_frames());
    assert_eq!(g.mode(), Mode::Playing);
    let events = hold(&mut g, "J", 10);
    assert!(!events.contains(&Event::Jumped), "{events:?}");
}

#[test]
fn game_over_then_continue_shows_the_intro() {
    let t = tuning();
    let mut g = titled(&spikes());
    start_playing(&mut g);
    for _ in 0..3 {
        let mut n = 0;
        while !g.step(input("R")).contains(&Event::Died) {
            n += 1;
            assert!(n < 400, "never died");
        }
        hold(&mut g, "-", u32::from(t.game.death_frames));
        if g.lives() > 0 {
            hold(&mut g, "-", intro_frames());
            assert_eq!(g.mode(), Mode::Playing);
        }
    }
    assert!(matches!(g.mode(), Mode::GameOver { .. }));
    hold(&mut g, "-", u32::from(t.game.game_over_frames));
    g.step(input("S"));
    assert!(matches!(g.mode(), Mode::StageIntro { .. }));
    assert_eq!((g.lives(), g.score()), (3, 0));
    hold(&mut g, "-", intro_frames());
    assert_eq!(g.mode(), Mode::Playing);
}

#[test]
fn start_pauses_and_resumes() {
    let mut g = titled(&flat(60));
    start_playing(&mut g);
    hold(&mut g, "R", 20);
    let before = g.clone();
    g.step(input("RS"));
    assert_eq!(g.mode(), Mode::Paused);
    // Paused: nothing moves, the timer stands still, held START is no press.
    hold(&mut g, "RS", 100);
    hold(&mut g, "R", 100);
    assert_eq!(g.mode(), Mode::Paused);
    assert_eq!(g.player(), before.player());
    assert_eq!(g.time_left(), before.time_left());
    g.step(input("RS"));
    assert_eq!(g.mode(), Mode::Playing);
    hold(&mut g, "R", 5);
    assert!(g.player().x > before.player().x);
}

#[test]
fn direct_start_has_no_pause_and_no_intro() {
    let mut g = Game::new(&spikes(), &tuning(), 1);
    assert_eq!(g.start(), Start::Playing);
    assert_eq!(g.mode(), Mode::Playing);
    g.step(input("S"));
    assert_eq!(g.mode(), Mode::Playing, "START does not pause");
    while !g.step(input("R")).contains(&Event::Died) {}
    hold(&mut g, "-", u32::from(tuning().game.death_frames));
    assert_eq!(g.mode(), Mode::Playing, "no intro after a death");
}

#[test]
fn flow_state_is_hashed() {
    let a = Game::new(&flat(30), &tuning(), 1);
    let b = titled(&flat(30));
    assert_ne!(a.state_hash(), b.state_hash());
}
