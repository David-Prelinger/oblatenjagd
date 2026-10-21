//! Tests of the cue rules: every event decided, every id from the design,
//! the voice policy, and the director's music rules on small games.

use super::*;
use jnr_core::{Level, Start, Tuning};

const DESIGN: &str = include_str!("../../../../docs/audio-design.md");

fn tuning() -> Tuning {
    Tuning::parse(include_str!("../../../../content/tuning.toml")).unwrap()
}

/// A flat 40 × 14 level (ground rows 12–13, start at column 2, a goal at
/// column 39) with these extra `[meta]` lines.
fn flat(meta: &str) -> Level {
    let air = ".".repeat(40);
    let ground = "#".repeat(40);
    let start = format!("..P{}G", ".".repeat(36));
    let mut rows: Vec<&str> = vec![&air; 11];
    rows.push(&start);
    rows.push(&ground);
    rows.push(&ground);
    let text = format!(
        "[meta]\nid = \"t\"\n{meta}\n[legend]\n\"#\" = \"solid\"\n\"P\" = {{ entity = \"player_start\" }}\n\"G\" = {{ entity = \"goal\" }}\n[map]\ngrid = '''\n{}\n'''\n",
        rows.join("\n")
    );
    Level::parse(&text).unwrap()
}

fn input(token: &str) -> InputFrame {
    InputFrame::parse_token(token).unwrap()
}

/// Steps `frames` times with `token`; returns (frame, cue) of every cue.
fn run(g: &mut Game, d: &mut Director, token: &str, frames: u32) -> Vec<(u32, Cue)> {
    let mut out = Vec::new();
    for _ in 0..frames {
        g.step(input(token));
        out.extend(d.step(g, input(token)).into_iter().map(|c| (g.frame(), c)));
    }
    out
}

fn cues(log: &[(u32, Cue)]) -> Vec<Cue> {
    log.iter().map(|(_, c)| *c).collect()
}

// ------------------------------------------------------------- the tables

/// Every core event has a decided sound, as in `docs/audio-design.md`
/// §6.2 (the `match` in `sfx_for` is exhaustive: a new event fails the
/// build; this pins the decisions).
#[test]
fn every_event_decided() {
    use EventSound::{Music as M, Sfx as S, Silent};
    let expected = [
        (Event::Jumped, S(Sfx::Jump.play())),
        (Event::Landed, Silent),
        (Event::Died, M),
        (Event::Respawned, Silent),
        (Event::LevelComplete, M),
        (Event::OblateCollected, S(Sfx::Oblate.play())),
        (Event::PowerUp, Silent),
        (Event::Hurt, S(Sfx::Hurt.play())),
        (Event::ExtraLife, S(Sfx::OneUp.play())),
        (Event::BlockBumped, S(Sfx::Bump.play())),
        (Event::BrickBroken, S(Sfx::Brick.play())),
        (Event::Stomped, S(Sfx::Stomp.play())),
        (Event::CheckpointReached, S(Sfx::Checkpoint.play())),
        (Event::GameOver, M),
        (Event::HacklThrown, S(Sfx::Throw.play())),
        (Event::AxeHit, S(Sfx::AxeHit.play())),
        (Event::CrawlStarted, Silent),
        (Event::Stroke, S(Sfx::Stroke.play())),
        (Event::Bounced, S(Sfx::RoofBounce.play())),
    ];
    assert_eq!(expected.len(), Event::ALL.len());
    for (e, (want_e, want)) in Event::ALL.into_iter().zip(expected) {
        assert_eq!(e, want_e);
        assert_eq!(sfx_for(e, Power::Normal), want, "{e:?}");
    }
    // The power decides the jump's pitch and the power-up's sound.
    assert_eq!(
        sfx_for(Event::Jumped, Power::Big),
        S(Sfx::Jump.pitched(JUMP_BIG_SEMITONES))
    );
    assert_eq!(
        sfx_for(Event::Jumped, Power::Hackl),
        S(Sfx::Jump.pitched(JUMP_BIG_SEMITONES))
    );
    assert_eq!(sfx_for(Event::PowerUp, Power::Big), S(Sfx::Zucker.play()));
    assert_eq!(sfx_for(Event::PowerUp, Power::Hackl), S(Sfx::Hackl.play()));
}

/// The words in backticks in `text`.
fn ticked(text: &str) -> Vec<&str> {
    text.split('`').skip(1).step_by(2).collect()
}

/// Every cue id is one of the design's: SFX ids in §6, music ids exactly
/// the first column of §5's overview table (pitched variants are the
/// same ids, [`PITCHED`]).
#[test]
fn every_cue_id_is_in_the_design() {
    let sfx_section = DESIGN
        .split("## 6. SFX")
        .nth(1)
        .and_then(|s| s.split("## 7.").next())
        .unwrap();
    let sfx_ids = ticked(sfx_section);
    for s in Sfx::ALL {
        assert!(
            sfx_ids.contains(&s.id()),
            "{}: not in audio-design §6",
            s.id()
        );
    }
    let overview = DESIGN
        .split("### Overview")
        .nth(1)
        .and_then(|s| s.split("### ").next())
        .unwrap();
    let mut theirs: Vec<&str> = overview
        .lines()
        .filter_map(|l| l.strip_prefix("| `"))
        .filter_map(|l| l.split('`').next())
        .collect();
    theirs.sort_unstable();
    let mut ours: Vec<&str> = Music::ALL.iter().map(|m| m.id()).collect();
    ours.sort_unstable();
    assert_eq!(ours, theirs, "Music::ALL is §5's overview table");
}

/// The design's counts (§6 "Count"): 25 must SFX, 25 music cues (with
/// the nice `unterwasser`); ids unique and found by `from_id`.
#[test]
fn the_tables_are_complete_and_unique() {
    assert_eq!(Sfx::ALL.len(), 25);
    assert_eq!(Music::ALL.len(), 25);
    for s in Sfx::ALL {
        assert_eq!(Sfx::from_id(s.id()), Some(s));
        assert!((1..=9).contains(&s.prio()) && s.ms() > 0, "{s:?}");
        assert_ne!(s.voice(), Voice::A, "no must SFX is ambience");
    }
    for m in Music::ALL {
        assert_eq!(Music::from_id(m.id()), Some(m));
    }
    // A level may name a stage piece, not a jingle or the underwater twin.
    assert_eq!(Music::level_song("salzburg"), Some(Music::Salzburg));
    assert_eq!(Music::level_song("death"), None);
    assert_eq!(Music::level_song("unterwasser"), None);
    assert_eq!(Music::level_song("polka"), None);
    assert_eq!(
        Music::Wolfgangsee.underwater_twin(),
        Some(Music::Unterwasser)
    );
    assert_eq!(Music::Salzburg.underwater_twin(), None);
}

#[test]
fn cue_lines_read_as_text() {
    assert_eq!(
        log_line(7, &Cue::Sfx(Sfx::Jump.play())),
        "     7 sfx jump P4"
    );
    assert_eq!(
        log_line(1, &Cue::CutSfx(Cut::KeepItems)),
        "     1 cut-sfx P W A"
    );
    assert_eq!(log_line(1, &Cue::Song(Music::Title)), "     1 song title");
    assert_eq!(log_line(1, &Cue::Hold(true)), "     1 hold on");
    assert_eq!(ms_to_frames(150), 9);
    assert_eq!(ms_to_frames(1000), 60);
    assert_eq!(ms_to_frames(25), 2);
}

// ---------------------------------------------------------- voice policy

/// These SFX at their own pitch.
fn p(sfx: &[Sfx]) -> Vec<Play> {
    sfx.iter().map(|s| s.play()).collect()
}

#[test]
fn a_pitched_variant_is_its_id_to_the_voice_rules() {
    let mut v = Voices::default();
    let big = Sfx::Jump.pitched(JUMP_BIG_SEMITONES);
    assert_eq!(v.resolve(0, &[big, Sfx::Jump.play()]), vec![big]);
    assert_eq!(v.playing(Voice::P, 1), Some(Sfx::Jump));
    assert_eq!(log_line(3, &Cue::Sfx(big)), "     3 sfx jump P4 -5");
    assert_eq!(PITCHED.len(), 2);
}

#[test]
fn one_trigger_per_id_and_one_sound_per_voice_per_step() {
    let mut v = Voices::default();
    let out = v.resolve(10, &p(&[Sfx::Oblate, Sfx::Oblate, Sfx::Jump, Sfx::Stroke]));
    // Two Oblaten: one sound; jump (4) beats stroke (2) on voice P.
    assert_eq!(out, p(&[Sfx::Jump, Sfx::Oblate]));
    assert_eq!(v.playing(Voice::P, 10), Some(Sfx::Jump));
    // A tie on one voice: the first request wins.
    let mut v = Voices::default();
    assert_eq!(
        v.resolve(0, &p(&[Sfx::Stomp, Sfx::RoofBounce])),
        p(&[Sfx::Stomp])
    );
}

#[test]
fn a_retrigger_restarts_and_stealing_goes_by_priority() {
    let mut v = Voices::default();
    assert_eq!(v.resolve(0, &p(&[Sfx::Hurt])), p(&[Sfx::Hurt])); // P8, 30 frames
    // Lower priority on the busy voice: dropped.
    assert_eq!(v.resolve(5, &p(&[Sfx::Jump])), p(&[]));
    assert_eq!(v.playing(Voice::P, 5), Some(Sfx::Hurt));
    // Equal priority replaces (the same id restarts).
    assert_eq!(v.resolve(6, &p(&[Sfx::Hurt])), p(&[Sfx::Hurt]));
    assert_eq!(v.playing(Voice::P, 6 + 29), Some(Sfx::Hurt));
    assert_eq!(v.playing(Voice::P, 6 + 30), None, "free after its length");
    assert_eq!(v.resolve(36, &p(&[Sfx::Jump])), p(&[Sfx::Jump]));
    // Higher priority replaces.
    assert_eq!(v.resolve(37, &p(&[Sfx::Zucker])), p(&[Sfx::Zucker]));
    // Other voices are independent.
    assert_eq!(v.resolve(38, &p(&[Sfx::Bump])), p(&[Sfx::Bump]));
}

#[test]
fn cuts_free_voices_and_priority_7_ducks() {
    let mut v = Voices::default();
    v.resolve(0, &p(&[Sfx::Checkpoint, Sfx::Stomp, Sfx::Jump]));
    assert!(v.ducking(0), "checkpoint is priority 7");
    assert!(v.ducking(26) && !v.ducking(27), "27 frames (450 ms)");
    v.cut(Cut::KeepItems);
    assert_eq!(v.playing(Voice::I, 1), Some(Sfx::Checkpoint));
    assert_eq!(v.playing(Voice::P, 1), None);
    assert_eq!(v.playing(Voice::W, 1), None);
    v.cut(Cut::All);
    assert_eq!(v.playing(Voice::I, 1), None);
    assert!(!v.ducking(1));
    v.resolve(2, &p(&[Sfx::HohesC]));
    assert!(v.stop(Sfx::HohesC, 3));
    assert!(!v.stop(Sfx::HohesC, 3), "stopped already");
}

// --------------------------------------------------------------- director

#[test]
fn the_title_start_intro_and_stage_piece() {
    let level = flat("time_limit = 300\nmusic = \"salzburg\"");
    let mut g = Game::new_at(&level, &tuning(), 1, Start::Title);
    let mut d = Director::new();
    assert_eq!(d.begin(&g), vec![Cue::Song(Music::Title)]);
    let log = run(&mut g, &mut d, "S", 1);
    assert_eq!(
        cues(&log),
        vec![Cue::StopSong, Cue::Sfx(Sfx::UiStart.play())],
        "START: the title stops, `ui_start`"
    );
    // The intro is silent; play starts the stage piece.
    let log = run(&mut g, &mut d, "-", 150);
    assert_eq!(log, vec![(151, Cue::Song(Music::Salzburg))]);
}

#[test]
fn pause_cuts_sfx_holds_the_song_and_resumes_in_place() {
    let level = flat("time_limit = 300\nmusic = \"salzburg\"");
    let mut g = Game::new_at(&level, &tuning(), 1, Start::Title);
    let mut d = Director::new();
    d.begin(&g);
    run(&mut g, &mut d, "S", 1);
    run(&mut g, &mut d, "-", 150);
    let log = run(&mut g, &mut d, "J", 1);
    assert_eq!(cues(&log), vec![Cue::Sfx(Sfx::Jump.play())]);
    let log = run(&mut g, &mut d, "S", 1);
    assert_eq!(
        cues(&log),
        vec![
            Cue::CutSfx(Cut::All),
            Cue::Sfx(Sfx::Pause.play()),
            Cue::Hold(true),
            Cue::Duck(true)
        ]
    );
    let log = run(&mut g, &mut d, "-", 40);
    assert_eq!(cues(&log), vec![Cue::Duck(false)]);
    let log = run(&mut g, &mut d, "S", 1);
    assert_eq!(
        cues(&log),
        vec![
            Cue::Sfx(Sfx::Pause.pitched(PAUSE_OUT_SEMITONES)),
            Cue::Hold(false),
            Cue::Duck(true)
        ],
        "no new song: the loop resumes where it was"
    );
}

#[test]
fn zeit_60_rings_the_bells_and_holds_the_piece_for_their_length() {
    let level = flat("time_limit = 61\nmusic = \"salzburg\"");
    let mut g = Game::new(&level, &tuning(), 1);
    let mut d = Director::new();
    assert_eq!(d.begin(&g), vec![Cue::Song(Music::Salzburg)]);
    let log = run(&mut g, &mut d, "-", 200);
    assert_eq!(
        log,
        vec![
            (60, Cue::Jingle(Music::Hurry)),
            (60, Cue::Hold(true)),
            (60 + 96, Cue::Hold(false)),
        ]
    );
    assert_eq!(g.time_left(), 61 - 3);
}

#[test]
fn a_death_cuts_everything_and_the_respawn_restarts_the_piece() {
    // A 1 s timer: time-up at frame 60, respawn after the dying phase.
    let level = flat("time_limit = 1\nmusic = \"strobl\"");
    let mut g = Game::new(&level, &tuning(), 1);
    let mut d = Director::new();
    d.begin(&g);
    let log = run(&mut g, &mut d, "-", 60);
    assert_eq!(
        log,
        vec![
            (60, Cue::CutSfx(Cut::All)),
            (60, Cue::StopSong),
            (60, Cue::Jingle(Music::Death)),
        ]
    );
    // The respawn (after the dying phase) starts the piece again.
    let log = run(&mut g, &mut d, "-", 100);
    assert_eq!(cues(&log), vec![Cue::Song(Music::Strobl)]);
    assert_eq!((g.deaths(), g.mode()), (1, Mode::Playing));
}

#[test]
fn begin_starts_fresh_with_the_level_piece_or_silence() {
    let level = flat("time_limit = 300\nmusic = \"salzburg\"");
    let g = Game::new(&level, &tuning(), 1);
    let mut d = Director::new();
    d.begin(&g);
    assert_eq!(d.begin(&g), vec![Cue::Song(Music::Salzburg)]);
    let quiet = Game::new(&flat("time_limit = 300"), &tuning(), 1);
    assert_eq!(Director::new().begin(&quiet), vec![]);
}
