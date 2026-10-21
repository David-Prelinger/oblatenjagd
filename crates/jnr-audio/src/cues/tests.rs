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

/// Every cue id is one of the design's: each SFX with its voice, priority
/// and length as in its §6 row, the music ids exactly
/// the first column of §5's overview table (pitched variants are the
/// same ids, [`PITCHED`]).
#[test]
fn every_cue_id_is_in_the_design() {
    let sfx_section = DESIGN
        .split("## 6. SFX")
        .nth(1)
        .and_then(|s| s.split("## 7.").next())
        .expect("audio-design.md has `## 6. SFX` before `## 7.`");
    // Every table row of §6 whose SFX column starts with one of our ids:
    // `| trigger | `id` … | V / prio | N ms … | …`.
    for sfx in Sfx::ALL {
        let row = sfx_section
            .lines()
            .filter(|l| l.starts_with('|'))
            .map(|l| l.split('|').map(str::trim).collect::<Vec<_>>())
            .find(|cells| cells.len() > 4 && ticked(cells[2]).first() == Some(&sfx.id()))
            .unwrap_or_else(|| panic!("`{}`: no row in audio-design §6", sfx.id()));
        let (voice, prio) = row[3]
            .split_once(" / ")
            .unwrap_or_else(|| panic!("`{}`: voice/prio cell `{}`", sfx.id(), row[3]));
        assert_eq!(voice, sfx.voice().name(), "`{}`: voice", sfx.id());
        let prio: u8 = prio
            .parse()
            .unwrap_or_else(|_| panic!("`{}`: priority `{prio}`", sfx.id()));
        assert_eq!(prio, sfx.prio(), "`{}`: priority", sfx.id());
        let ms: u32 = row[4]
            .split(|c: char| !c.is_ascii_digit())
            .find(|w| !w.is_empty())
            .and_then(|w| w.parse().ok())
            .unwrap_or_else(|| panic!("`{}`: duration cell `{}`", sfx.id(), row[4]));
        assert_eq!(ms, sfx.ms(), "`{}`: length (ms)", sfx.id());
    }
    let overview = DESIGN
        .split("### Overview")
        .nth(1)
        .and_then(|s| s.split("### ").next())
        .expect("audio-design.md §5 has a `### Overview` table");
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
    assert_eq!(log_line(1, &Cue::HoldSong(true)), "     1 hold-song on");
    assert_eq!(
        log_line(1, &Cue::HoldJingle(false)),
        "     1 hold-jingle off"
    );
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
    let mut d = Director::default();
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
    let mut d = Director::default();
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
            Cue::HoldSong(true),
            Cue::HoldJingle(true),
        ],
        "no duck in the pause: everything is held"
    );
    let log = run(&mut g, &mut d, "-", 40);
    assert_eq!(cues(&log), vec![]);
    let log = run(&mut g, &mut d, "S", 1);
    assert_eq!(
        cues(&log),
        vec![
            Cue::Sfx(Sfx::Pause.pitched(PAUSE_OUT_SEMITONES)),
            Cue::HoldSong(false),
            Cue::HoldJingle(false),
            Cue::Duck(true)
        ],
        "no new song: the loop resumes where it was"
    );
}

#[test]
fn zeit_60_rings_the_bells_and_holds_the_piece_for_their_length() {
    let level = flat("time_limit = 61\nmusic = \"salzburg\"");
    let mut g = Game::new(&level, &tuning(), 1);
    let mut d = Director::default();
    assert_eq!(d.begin(&g), vec![Cue::Song(Music::Salzburg)]);
    let log = run(&mut g, &mut d, "-", 200);
    assert_eq!(
        log,
        vec![
            (60, Cue::Jingle(Music::Hurry)),
            (60, Cue::HoldSong(true)),
            (60 + 96, Cue::HoldSong(false)),
        ]
    );
    assert_eq!(g.time_left(), 61 - 3);
}

/// A pause during the ZEIT-60 bells holds the jingle (it isn't stopped:
/// it runs on at volume 0) and the song stays held until the bells' 96
/// frames are over; the paused frames count, as the bells run on (E63).
#[test]
fn a_pause_holds_a_running_jingle_in_place() {
    let level = flat("time_limit = 61\nmusic = \"salzburg\"");
    let mut g = Game::new_at(&level, &tuning(), 1, Start::Title);
    let mut d = Director::default();
    d.begin(&g);
    run(&mut g, &mut d, "S", 1);
    run(&mut g, &mut d, "-", 150);
    let log = run(&mut g, &mut d, "-", 60);
    assert_eq!(
        cues(&log),
        vec![Cue::Jingle(Music::Hurry), Cue::HoldSong(true)]
    );
    let bells = g.frame();
    let log = run(&mut g, &mut d, "S", 1);
    assert_eq!(
        cues(&log),
        vec![
            Cue::CutSfx(Cut::All),
            Cue::Sfx(Sfx::Pause.play()),
            Cue::HoldJingle(true)
        ],
        "the bells are held, not stopped"
    );
    run(&mut g, &mut d, "-", 9);
    let log = run(&mut g, &mut d, "S", 1);
    assert!(!cues(&log).contains(&Cue::HoldSong(false)), "{log:?}");
    assert!(cues(&log).contains(&Cue::HoldJingle(false)), "{log:?}");
    let log = run(&mut g, &mut d, "-", 100);
    assert_eq!(
        log.iter()
            .find(|(_, c)| *c == Cue::HoldSong(false))
            .map(|(f, _)| *f),
        Some(bells + 96)
    );
}

#[test]
fn a_death_cuts_everything_and_the_respawn_restarts_the_piece() {
    // A 1 s timer: time-up at frame 60, respawn after the dying phase.
    let level = flat("time_limit = 1\nmusic = \"strobl\"");
    let mut g = Game::new(&level, &tuning(), 1);
    let mut d = Director::default();
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
    let mut d = Director::default();
    d.begin(&g);
    assert_eq!(d.begin(&g), vec![Cue::Song(Music::Salzburg)]);
    let quiet = Game::new(&flat("time_limit = 300"), &tuning(), 1);
    assert_eq!(Director::default().begin(&quiet), vec![]);
}

#[test]
fn pending_music_is_empty() {
    // Every music cue has its source since MA (milestone acceptance): the
    // list may never grow again.
    assert_eq!(
        PENDING_MUSIC,
        &[] as &[Music],
        "PENDING_MUSIC must stay empty: every music cue needs its .song"
    );
}

/// E63: jingles are timed by the bank's rendered lengths; a jingle the
/// table lacks falls back to its design length.
#[test]
fn jingle_frames_prefer_the_rendered_length() {
    let mut lengths = JingleFrames::default();
    assert_eq!(lengths.of(Music::Hurry), 96, "1600 ms by design");
    lengths.frames.insert(Music::Hurry, 101);
    assert_eq!(lengths.of(Music::Hurry), 101);
    assert_eq!(lengths.of(Music::BossDefeat), 90);
}

/// E63: the beaten Oberkellner still counts as the fight until the stage
/// piece is back (the bells don't hold a song that isn't playing).
#[test]
fn done_counts_as_the_fight_until_the_piece_resumes() {
    use state::oberkellner as st;
    assert!(!boss_fighting(None, false));
    assert!(!boss_fighting(Some(st::WAIT), true));
    assert!(boss_fighting(Some(st::TELEGRAPH), false));
    assert!(boss_fighting(Some(st::DONE), true));
    assert!(!boss_fighting(Some(st::DONE), false));
}

/// E63: the ZEIT-60 bells hold the song for the jingle's rendered length
/// (from the bank's table), not its design length.
#[test]
fn the_bells_hold_counts_rendered_frames() {
    let level = flat("time_limit = 61\nmusic = \"salzburg\"");
    let mut g = Game::new_at(&level, &tuning(), 1, Start::Title);
    let mut lengths = JingleFrames::default();
    lengths.frames.insert(Music::Hurry, 50);
    let mut d = Director::new(lengths);
    d.begin(&g);
    run(&mut g, &mut d, "S", 1);
    run(&mut g, &mut d, "-", 150);
    let log = run(&mut g, &mut d, "-", 60);
    assert!(cues(&log).contains(&Cue::HoldSong(true)));
    let bells = g.frame();
    let log = run(&mut g, &mut d, "-", 60);
    assert_eq!(log, vec![(bells + 50, Cue::HoldSong(false))]);
}

/// A director with short jingles: the Tusch 30 frames, the bells 20.
fn short_director() -> Director {
    let mut lengths = JingleFrames::default();
    lengths.frames.insert(Music::BossDefeat, 30);
    lengths.frames.insert(Music::Hurry, 20);
    lengths.frames.insert(Music::FanfareClear, 50);
    Director::new(lengths)
}

/// E63 review: a pause doesn't move what is tied to the jingle's sound
/// (it runs on at volume 0); only the silent wait after it counts play
/// frames. Tusch ends at 100, the café piece is due at 160.
#[test]
fn a_pause_owes_only_the_silence_after_the_jingle() {
    let tusch = |paused_at: u32, resume: u32| {
        let mut d = short_director();
        d.jingle = Some(Music::BossDefeat);
        d.jingle_until = Some(100);
        d.scheduled = Some((160, Music::CafeZauner));
        d.paused = true;
        d.paused_at = Some(paused_at);
        d.resume(resume);
        assert!(!d.paused);
        assert_eq!(d.jingle_until, Some(100), "the jingle's end doesn't move");
        d.scheduled.map(|s| s.0)
    };
    // Inside the Tusch, resumed before it ends: nothing owed.
    assert_eq!(tusch(80, 95), Some(160));
    // Spanning the Tusch's end: the whole second of silence after resume.
    assert_eq!(tusch(90, 130), Some(190));
    // Spanning the scheduled frame, inside the silence: its rest (40).
    assert_eq!(tusch(120, 200), Some(240));
    // A song due at the jingle's end (the boss loop after the chord) owes
    // nothing: it starts on the resume frame if its time passed.
    let mut d = short_director();
    d.jingle = Some(Music::BossSurprise);
    d.jingle_until = Some(100);
    d.scheduled = Some((100, Music::Boss));
    d.hurry_until = Some(110);
    d.paused = true;
    d.paused_at = Some(90);
    d.resume(130);
    assert_eq!(d.scheduled, Some((130, Music::Boss)));
    assert_eq!(d.hurry_until, Some(110), "the bells' hold doesn't move");
}

/// E63: the bells wait for a boss jingle (`bells_due`), hold nothing once
/// the Oberkellner is beaten, and the café piece waits for them
/// (game-designer review: start = max(Tusch end + 1 s, bells' end)).
#[test]
fn the_bells_wait_for_the_tusch_and_the_cafe_waits_for_the_bells() {
    use state::oberkellner as st;
    let mut d = short_director();
    d.jingle = Some(Music::BossDefeat);
    d.jingle_until = Some(100);
    assert!(d.boss_jingle_playing(99));
    assert!(!d.boss_jingle_playing(100));
    // Bells at the Tusch's end (100): the café, due at 160, stays there.
    d.scheduled = Some((160, Music::CafeZauner));
    let mut out = Vec::new();
    d.now = 100;
    d.bells(100, Some(st::DONE), &mut out);
    assert_eq!(out, vec![Cue::Jingle(Music::Hurry)]);
    assert_eq!(d.hurry_until, None, "DONE is still the fight: no hold");
    assert_eq!(d.scheduled, Some((160, Music::CafeZauner)));
    // Bells late in the silence (150): the café waits for their end (170).
    d.scheduled = Some((160, Music::CafeZauner));
    d.now = 150;
    d.bells(150, Some(st::DONE), &mut Vec::new());
    assert_eq!(d.scheduled, Some((170, Music::CafeZauner)));
    // Once the café piece is back, the bells hold it as anywhere else.
    d.scheduled = None;
    d.now = 400;
    d.bells(400, Some(st::DONE), &mut Vec::new());
    assert_eq!(d.hurry_until, Some(420));
}

/// E63: the route map's piece waits for the fanfare's end.
#[test]
fn the_route_map_waits_for_the_fanfare() {
    let level = flat("time_limit = 300");
    let g = Game::new(&level, &tuning(), 1);
    let now = g.frame();
    let map = |jingle_until: Option<u32>| {
        let mut d = short_director();
        d.jingle = Some(Music::FanfareClear);
        d.jingle_until = jingle_until;
        let (mut out, mut req) = (Vec::new(), Vec::new());
        let from = Some(Phase::Clear(ClearPhase::Clear));
        d.enter(
            &g,
            from,
            Phase::Map(MapPhase::Arrive),
            false,
            &mut out,
            &mut req,
        );
        (out, d.scheduled)
    };
    assert_eq!(
        map(Some(now + 30)),
        (vec![], Some((now + 30, Music::RouteMap)))
    );
    assert_eq!(map(Some(now)), (vec![Cue::Song(Music::RouteMap)], None));
    assert_eq!(map(None), (vec![Cue::Song(Music::RouteMap)], None));
}

/// The rendered lengths of the boss jingles and the bells (as the bank
/// renders them at E64), so the logs match the cue goldens.
fn boss_lengths() -> JingleFrames {
    let mut l = JingleFrames::default();
    for (m, f) in [
        (Music::BossIntro, 93),
        (Music::BossSurprise, 38),
        (Music::BossDefeat, 95),
        (Music::Hurry, 96),
    ] {
        l.frames.insert(m, f);
    }
    l
}

/// The music cues (songs, jingles) of a replay on `00_m7_boss.level`.
fn m7_boss_music(replay: &str) -> Vec<(u32, Cue)> {
    let level = Level::parse(include_str!("../../../../content/levels/00_m7_boss.level")).unwrap();
    let replay = jnr_core::Replay::parse(replay).unwrap();
    let mut g = replay.new_game(&[level], &tuning()).unwrap();
    let mut d = Director::new(boss_lengths());
    let mut out: Vec<(u32, Cue)> = d.begin(&g).into_iter().map(|c| (0, c)).collect();
    for input in replay.frames() {
        g.step(input);
        let f = g.frame();
        out.extend(d.step(&g, input).into_iter().map(|c| (f, c)));
    }
    out.retain(|(_, c)| matches!(c, Cue::Song(_) | Cue::Jingle(_)));
    out
}

/// E64: ZEIT 60 on the arena-lock frame: the lock's `boss_intro` comes
/// first and the bells wait for it and the chord after it.
#[test]
fn zeit60_on_the_lock_frame_waits_for_the_boss_jingles() {
    let log = m7_boss_music(include_str!(
        "../../../../tests/replays/qa_ma_cues_zeit60_on_lock_frame.replay"
    ));
    let at = |f: u32| -> Vec<Cue> { log.iter().filter(|(g, _)| *g == f).map(|x| x.1).collect() };
    assert_eq!(at(14400), vec![Cue::Jingle(Music::BossIntro)]);
    assert_eq!(
        at(14528),
        vec![Cue::Song(Music::Boss), Cue::Jingle(Music::Hurry)]
    );
}

/// E64: ZEIT 60 on the DONE frame: the bells wait for the Tusch.
#[test]
fn zeit60_on_the_done_frame_waits_for_the_tusch() {
    let log = m7_boss_music(include_str!(
        "../../../../tests/replays/qa_ma_cues_zeit60_on_done_frame.replay"
    ));
    let at = |f: u32| -> Vec<Cue> { log.iter().filter(|(g, _)| *g == f).map(|x| x.1).collect() };
    assert_eq!(at(14400), vec![Cue::Jingle(Music::BossDefeat)]);
    assert_eq!(at(14495), vec![Cue::Jingle(Music::Hurry)]);
}

/// E64: ZEIT 60 on the frame the boss loop is due: the loop starts on
/// time, the bells play over it (only the café piece waits for them).
#[test]
fn zeit60_on_the_boss_loop_frame_starts_the_loop_on_time() {
    let log = m7_boss_music(include_str!(
        "../../../../tests/replays/qa_ma_cues_zeit60_on_boss_loop_due.replay"
    ));
    let at = |f: u32| -> Vec<Cue> { log.iter().filter(|(g, _)| *g == f).map(|x| x.1).collect() };
    assert_eq!(
        at(14400),
        vec![Cue::Jingle(Music::Hurry), Cue::Song(Music::Boss)]
    );
}

/// E64: the bells move only the stage piece after the Tusch, never the
/// boss loop.
#[test]
fn the_bells_never_move_the_boss_loop() {
    use state::oberkellner as st;
    let mut d = short_director();
    d.scheduled = Some((100, Music::Boss));
    d.now = 100;
    d.bells(100, Some(st::TELEGRAPH), &mut Vec::new());
    assert_eq!(d.scheduled, Some((100, Music::Boss)));
}
