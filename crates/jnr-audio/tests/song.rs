//! `.song`: parser errors, rendering (loop alignment, seamless wrap, cache,
//! render options), music lints (a failing and a passing fixture per lint),
//! melody fixtures and the render-time budget.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

use jnr_audio::music_lint::Level;
use jnr_audio::song::Sound;
use jnr_audio::song_render::{LOOP_ALIGN, natural_frames};
use jnr_audio::{
    ErrorKind, Instrument, LintContext, ParseError, Quote, RenderOptions, Song, lint_song,
    render_song,
};

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/music")
}

/// Every file under `dir` (recursively) with extension `ext`: (path, text).
fn files_in(dir: &Path, ext: &str) -> Vec<(String, String)> {
    let mut paths = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in fs::read_dir(&d).into_iter().flatten().flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|x| x == ext) {
                paths.push(p);
            }
        }
    }
    let mut out: Vec<(String, String)> = paths
        .into_iter()
        .map(|p| {
            let text = fs::read_to_string(&p).unwrap();
            (p.strip_prefix(dir).unwrap().display().to_string(), text)
        })
        .collect();
    out.sort();
    out
}

struct Bank {
    instruments: BTreeMap<String, Instrument>,
    quotes: BTreeMap<String, Quote>,
    songs: BTreeMap<String, Song>,
}

fn bank() -> Bank {
    bank_in(&fixtures())
}

fn bank_in(dir: &Path) -> Bank {
    let files = |ext: &str| files_in(dir, ext);
    let instruments = files("inst")
        .iter()
        .map(|(f, t)| {
            let i = Instrument::parse(f, t).unwrap();
            (i.id.clone(), i)
        })
        .collect();
    let quotes = files("quote")
        .iter()
        .map(|(f, t)| {
            let q = Quote::parse(f, t).unwrap_or_else(|e| panic!("{e}"));
            (q.id.clone(), q)
        })
        .collect();
    let songs = files("song")
        .iter()
        .map(|(f, t)| {
            let s = Song::parse(f, t).unwrap_or_else(|e| panic!("{e}"));
            (s.id.clone(), s)
        })
        .collect();
    Bank {
        instruments,
        quotes,
        songs,
    }
}

impl Bank {
    fn ctx(&self) -> LintContext<'_> {
        LintContext {
            instruments: &self.instruments,
            songs: &self.songs,
            quotes: &self.quotes,
        }
    }
}

// ------------------------------------------------------------------ fixtures

#[test]
fn fixture_songs_parse_lint_clean_and_render() {
    let b = bank();
    assert!(b.songs.len() >= 2);
    for song in b.songs.values() {
        let lints = lint_song(song, &b.ctx());
        assert!(
            lints.is_empty(),
            "{}:\n{}",
            song.id,
            lints
                .iter()
                .map(|l| l.to_string())
                .collect::<Vec<_>>()
                .join("\n")
        );
        let r = render_song(song, &b.instruments, &RenderOptions::default()).unwrap();
        assert_eq!(r.render.clamped, 0, "{}", song.id);
        // Peaks of docs/audio-design.md §3: pieces -8 dBFS, jingles -6 dBFS.
        let limit = match song.kind {
            jnr_audio::song::SongKind::Loop => 13_045,
            jnr_audio::song::SongKind::Jingle => 16_423,
        };
        let peak = r
            .render
            .pcm
            .samples
            .iter()
            .map(|s| s.unsigned_abs())
            .max()
            .unwrap();
        assert!(peak <= limit, "{}: peak {peak}", song.id);
        assert!(
            r.render.pcm.samples.iter().any(|&s| s.abs() > 1000),
            "{} is silent",
            song.id
        );
    }
}

/// The game's own songs (`audio/`): lint-clean, and loops aligned.
#[test]
fn the_real_songs_lint_clean_and_loops_are_aligned() {
    let b = bank_in(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../audio"));
    for song in b.songs.values() {
        let errors: Vec<String> = lint_song(song, &b.ctx())
            .iter()
            .filter(|l| l.level == Level::Error)
            .map(|l| l.to_string())
            .collect();
        assert!(errors.is_empty(), "{}", errors.join("\n"));
        if song.kind == jnr_audio::song::SongKind::Loop {
            let r = render_song(song, &b.instruments, &RenderOptions::default()).unwrap();
            assert_eq!(
                r.render.pcm.samples.len() % LOOP_ALIGN as usize,
                0,
                "{}",
                song.id
            );
        }
    }
}

#[test]
fn every_loop_is_a_multiple_of_4096_frames() {
    let b = bank();
    for song in b
        .songs
        .values()
        .filter(|s| s.kind == jnr_audio::song::SongKind::Loop)
    {
        for tempo_pct in [100, 112] {
            let opts = RenderOptions {
                tempo_pct,
                length_frames: None,
            };
            let r = render_song(song, &b.instruments, &opts).unwrap();
            let len = r.render.pcm.samples.len() as u32;
            assert_eq!(len % LOOP_ALIGN, 0, "{} at {tempo_pct} %", song.id);
            assert_eq!(len, r.length_frames);
            // At most half an alignment block away from the written tempo.
            assert!(r.natural_frames.abs_diff(len as u64) <= LOOP_ALIGN as u64 / 2);
        }
    }
}

#[test]
fn the_hurry_render_is_12_percent_faster() {
    let b = bank();
    let song = &b.songs["placeholder_polka"];
    let normal = natural_frames(song, 100);
    let hurry = natural_frames(song, 112);
    assert_eq!(hurry, (normal * 100 + 56) / 112);
}

#[test]
fn length_frames_renders_exactly_that_long() {
    let b = bank();
    let song = &b.songs["placeholder_polka"];
    let opts = RenderOptions {
        tempo_pct: 100,
        length_frames: Some(260 * LOOP_ALIGN),
    };
    let r = render_song(song, &b.instruments, &opts).unwrap();
    assert_eq!(r.render.pcm.samples.len(), 260 * LOOP_ALIGN as usize);
    assert!(r.stretch_ppm > 0);
    let bad = RenderOptions {
        tempo_pct: 100,
        length_frames: Some(1000),
    };
    let e = render_song(song, &b.instruments, &bad).unwrap_err();
    assert!(e.to_string().contains("multiple of 4096"), "{e}");
}

/// The rendered loop equals the middle pass of the song played three times
/// in a row, so the wrap is the steady state: no click at the seam.
#[test]
fn the_loop_wrap_is_seamless() {
    let b = bank();
    let text = fs::read_to_string(fixtures().join("placeholder_polka.song")).unwrap();
    let song = &b.songs["placeholder_polka"];
    let looped = render_song(song, &b.instruments, &RenderOptions::default()).unwrap();
    let l = looped.length_frames as usize;

    let three = text
        .replace("kind = \"loop\"", "kind = \"jingle\"")
        .replace("bars = 24", "bars = 72")
        .replace(
            "order = [\"A\", \"B\", \"A\"]",
            "order = [\"A\", \"B\", \"A\", \"A\", \"B\", \"A\", \"A\", \"B\", \"A\"]",
        )
        .replace("id = \"placeholder_polka\"", "id = \"three\"");
    let three = Song::parse("three.song", &three).unwrap();
    let opts = RenderOptions {
        tempo_pct: 100,
        length_frames: Some(3 * l as u32),
    };
    let unrolled = render_song(&three, &b.instruments, &opts).unwrap();
    let mid = &unrolled.render.pcm.samples[l..2 * l];
    let lp = &looped.render.pcm.samples;
    let diff = mid.iter().zip(lp).filter(|(a, b)| a != b).count();
    assert_eq!(diff, 0, "the folded loop differs from the steady state");

    // And the seam itself jumps no more than the waveform does elsewhere.
    let max_step = lp
        .windows(2)
        .map(|w| (w[1] as i32 - w[0] as i32).abs())
        .max()
        .unwrap();
    let seam = (lp[0] as i32 - lp[l - 1] as i32).abs();
    assert!(seam <= max_step + 1, "seam {seam} > {max_step} + 1");
    // Something really wraps: the last bar's notes ring into sample 0.
    assert_ne!(lp[0], 0);
}

#[test]
fn identical_voices_are_rendered_once() {
    let b = bank();
    let r = render_song(
        &b.songs["placeholder_polka"],
        &b.instruments,
        &RenderOptions::default(),
    )
    .unwrap();
    assert!(
        r.voices > 3 * r.rendered_voices,
        "{} / {}",
        r.rendered_voices,
        r.voices
    );
}

#[test]
fn a_jingle_rings_out_past_its_last_bar() {
    let b = bank();
    let song = &b.songs["placeholder_fanfare"];
    let r = render_song(song, &b.instruments, &RenderOptions::default()).unwrap();
    // 2 bars of 2/4 at 160 bpm = 1.5 s = 66 150 frames, plus the release.
    assert_eq!(r.length_frames, 66_150);
    assert!(r.render.pcm.samples.len() > 66_150);
    assert!(r.render.pcm.samples.len() < 66_150 + 44_100 / 5);
}

#[test]
fn ties_merge_and_staccato_is_kept() {
    let b = bank();
    let song = &b.songs["placeholder_polka"];
    let lead = &song.events[0];
    // Bar 8: `bb4/4 _` is one 8-step note.
    let e = lead.iter().find(|e| e.bar == 7 && e.step == 7 * 8).unwrap();
    assert_eq!(e.len, 8);
    let chords = &song.events[3];
    assert!(
        chords
            .iter()
            .filter(|e| matches!(e.sound, Sound::Notes(_)))
            .all(|e| e.staccato)
    );
}

// ---------------------------------------------------------------- parse errors

#[track_caller]
fn check(e: ParseError, kind: ErrorKind, line: usize, needle: &str) {
    let shown = e.to_string();
    assert_eq!(e.kind, kind, "{shown}");
    assert_eq!(e.line, line, "{shown}");
    assert!(shown.contains(needle), "`{needle}` not in: {shown}");
    assert!(shown.starts_with(&format!("t.song:{line}:")), "{shown}");
}

/// A clean 20-bar loop (20 s). Line numbers: `[meta]` 1, `bars` 8,
/// pattern A's lines 19-21, pattern B's 26-28.
const T: &str = r#"[meta]
id = "t"
kind = "loop"
tempo_bpm = 120
meter = "2/4"
steps_per_beat = 2
key = "c major"
bars = 20
order = ["A", "B", "A", "B", "A"]
source = "original"

[channels]
lead = { instrument = "lead", volume_pct = 60 }
bass = { instrument = "bass", volume_pct = 60 }
drums = { instrument = "drums", volume_pct = 60 }

[pattern.A]
score = '''
lead  | c5/2 e5   | g5/2 e5   | f5/2 d5   | c5/4      |
bass  | c3/2 g2   | c3/2 g2   | g2/2 g2   | c3/4      |
drums | K/1 h K h | K/1 h K h | K/1 h K h | K/1 h S h |
'''

[pattern.B]
score = '''
lead  | e5/2 g5   | c6/2 g5   | d5/2 b4   | c5/4      |
bass  | c3/2 e3   | a2/2 e3   | g2/2 g2   | c3/4      |
drums | K/1 h K h | K/1 h K h | K/1 h K h | K/1 h S h |
'''
"#;

fn t(from: &str, to: &str) -> String {
    assert!(T.contains(from), "template lacks `{from}`");
    T.replacen(from, to, 1)
}

fn err(text: &str) -> ParseError {
    Song::parse("t.song", text).expect_err("should fail")
}

#[test]
fn the_template_is_clean() {
    let b = bank();
    let song = Song::parse("t.song", T).unwrap();
    assert_eq!(lint_song(&song, &b.ctx()), []);
    assert_eq!(song.total_steps(), 80);
}

#[test]
fn parse_errors_carry_file_and_line() {
    check(
        err(&t("c5/2 e5   | g5", "c5/2 x5   | g5")),
        ErrorKind::Score,
        19,
        "`x5` is not a note",
    );
    check(
        err(&t("c5/2 e5   | g5", "c5/0 e5   | g5")),
        ErrorKind::Score,
        19,
        "1..=4096",
    );
    check(
        err(&t("lead  | e5/2", "leed  | e5/2")),
        ErrorKind::Channel,
        26,
        "`leed` is not a channel",
    );
    check(
        err(&t("bass  | c3/2 e3   |", "bass    c3/2 e3   |")),
        ErrorKind::Score,
        27,
        "start with `|`",
    );
    check(
        err(&t("| c3/4      |\ndrums", "|\ndrums")),
        ErrorKind::Pattern,
        20,
        "3 bars",
    );
    check(
        err(&t("g5/2 e5   | f5", "g5/2 e5   |  | f5")),
        ErrorKind::Score,
        19,
        "empty bar",
    );
    check(
        err(&t("| c5/2 e5 ", "| _/2 e5 ")),
        ErrorKind::Score,
        19,
        "a tie",
    );
    check(
        err(&t("meter = \"2/4\"", "meter = \"4/4\"")),
        ErrorKind::Music,
        5,
        "2/4, 3/4 and 6/8",
    );
    check(
        err(&t("key = \"c major\"", "key = \"c dorian\"")),
        ErrorKind::Music,
        7,
        "bad key",
    );
    check(
        err(&t("kind = \"loop\"", "kind = \"once\"")),
        ErrorKind::Music,
        3,
        "loop | jingle",
    );
    check(
        err(&t("\"B\", \"A\"]", "\"C\", \"A\"]")),
        ErrorKind::Pattern,
        9,
        "`C`",
    );
    check(
        err(&t("id = \"t\"", "id = \"u\"")),
        ErrorKind::Id,
        2,
        "file stem",
    );
    check(
        err(&t("source = \"original\"", "source = \"Mozart\"")),
        ErrorKind::Music,
        10,
        "edition",
    );
    check(
        err(&t("tempo_bpm = 120", "tempo_bpm = 1.5")),
        ErrorKind::Schema,
        4,
        "",
    );
    check(
        err(&t("volume_pct = 60 }\nbass", "volume_pct = 160 }\nbass")),
        ErrorKind::Range,
        13,
        "volume_pct",
    );
    let many: String = (0..7)
        .map(|i| format!("c{i} = {{ instrument = \"lead\", volume_pct = 1 }}\n"))
        .collect();
    check(
        err(&t(
            "drums = { instrument = \"drums\", volume_pct = 60 }\n",
            &many,
        )),
        ErrorKind::Channel,
        12,
        "1..=6",
    );
}

// ------------------------------------------------------------------- lints

/// The lints of `text` with `code`, as `(line, level)`.
fn lints(text: &str, code: &str) -> Vec<(usize, Level)> {
    let b = bank();
    let song = Song::parse("t.song", text).unwrap_or_else(|e| panic!("{e}"));
    let all = lint_song(&song, &b.ctx());
    for l in &all {
        println!("{l}");
    }
    all.iter()
        .filter(|l| l.code == code)
        .map(|l| (l.at.line, l.level))
        .collect()
}

use Level::{Error, Warning};

#[test]
fn lint_bar_length() {
    assert_eq!(
        lints(&t("| c5/2 e5   |", "| c5/2 e5/1 |"), "bar-length"),
        [(19, Error)]
    );
}

#[test]
fn lint_loop_point() {
    // Pattern A is last in the order: its last bar rings over the end.
    let text = t(
        "| c5/4      |\nbass  | c3/2 g2",
        "| c5/6      |\nbass  | c3/2 g2",
    );
    assert_eq!(lints(&text, "loop-point"), [(19, Error)]);
}

#[test]
fn lint_bars() {
    assert_eq!(lints(&t("bars = 20", "bars = 21"), "bars"), [(8, Error)]);
}

#[test]
fn lint_range() {
    assert_eq!(lints(&t("c6/2 g5", "d7/2 g5"), "range"), [(26, Error)]);
}

#[test]
fn lint_key_and_accidental() {
    assert_eq!(lints(&t("f5/2 d5", "f#5/2 d5"), "key"), [(19, Error)]);
    assert_eq!(lints(&t("f5/2 d5", "f#5!/2 d5"), "key"), []);
    assert_eq!(
        lints(&t("f5/2 d5", "f5!/2 d5"), "accidental"),
        [(19, Warning)]
    );
    // A pattern's own key.
    let text = t("[pattern.B]\n", "[pattern.B]\nkey = \"g major\"\n");
    assert_eq!(lints(&text, "key"), []);
    let text = t("[pattern.B]\n", "[pattern.B]\nkey = \"f major\"\n");
    assert_eq!(lints(&text, "key").len(), 1, "b4 is not in f major");
}

#[test]
fn lint_length() {
    assert_eq!(
        lints(&t("tempo_bpm = 120", "tempo_bpm = 240"), "length"),
        [(4, Error)]
    );
    assert_eq!(
        lints(&t("tempo_bpm = 120", "tempo_bpm = 25"), "length"),
        [(4, Error)]
    );
    let jingle = t("kind = \"loop\"", "kind = \"jingle\"");
    assert_eq!(lints(&jingle, "length"), [(4, Error)], "a 20 s jingle");
    let short = jingle
        .replace("tempo_bpm = 120", "tempo_bpm = 400")
        .replace(
            "order = [\"A\", \"B\", \"A\", \"B\", \"A\"]",
            "order = [\"A\", \"B\"]",
        )
        .replace("bars = 20", "bars = 8");
    assert_eq!(lints(&short, "length"), []);
}

#[test]
fn lint_length_of() {
    let with = |target: &str| {
        t(
            "source = \"original\"\n",
            &format!("source = \"original\"\nlength_of = \"{target}\"\n"),
        )
    };
    assert_eq!(lints(&with("nope"), "length-of"), [(11, Error)]);
    assert_eq!(
        lints(&with("placeholder_fanfare"), "length-of"),
        [(11, Error)],
        "a jingle"
    );
    // The polka is 24 s, this one 20 s.
    assert_eq!(
        lints(&with("placeholder_polka"), "length-of"),
        [(11, Error)]
    );
    let fits = with("placeholder_polka").replace("tempo_bpm = 120", "tempo_bpm = 100");
    assert_eq!(lints(&fits, "length-of"), []);
    let b = bank();
    let song = Song::parse("t.song", &fits).unwrap();
    let target = render_song(
        &b.songs["placeholder_polka"],
        &b.instruments,
        &RenderOptions::default(),
    )
    .unwrap();
    let opts = RenderOptions {
        tempo_pct: 100,
        length_frames: Some(target.length_frames),
    };
    let r = render_song(&song, &b.instruments, &opts).unwrap();
    assert_eq!(r.render.pcm.samples.len(), target.render.pcm.samples.len());
}

const HAYDN: &str = "\n[[quote]]\nfixture = \"haydn_94_andante\"\nchannel = \"lead\"\nbar = 1\n";

#[test]
fn lint_quote_and_provenance() {
    // The template doesn't play Haydn: the first difference is reported.
    let text = format!("{T}{HAYDN}");
    let b = bank();
    let song = Song::parse("t.song", &text).unwrap();
    let all = lint_song(&song, &b.ctx());
    let q: Vec<String> = all
        .iter()
        .filter(|l| l.code == "quote")
        .map(|l| l.to_string())
        .collect();
    assert_eq!(q.len(), 1, "{q:?}");
    assert!(
        q[0].contains("expected c4 for 1/2 beat(s), found c5 for 1 beat(s)"),
        "{}",
        q[0]
    );
    assert!(
        q[0].starts_with("t.song:31:1: error[music-quote]: t.song:19:"),
        "{}",
        q[0]
    );
    assert_eq!(lints(&text, "provenance"), [(10, Error)]);

    let missing = format!("{T}{}", HAYDN.replace("haydn_94_andante", "nope"));
    assert_eq!(lints(&missing, "quote"), [(31, Error)]);

    // Playing it (an octave up, transposed) passes.
    let plays = t(
        "| c5/2 e5   | g5/2 e5   |",
        "| c5/1 c5 e5 e5 | g5 g5 e5/2 |",
    )
    .replace(
        "source = \"original\"",
        "source = \"Haydn\"\nedition = \"Rietz\"",
    );
    let ok = format!("{plays}{HAYDN}transpose = 12\n");
    assert_eq!(lints(&ok, "quote"), []);
    assert_eq!(lints(&ok, "provenance"), []);
    // Pitches only: other lengths pass, other notes don't.
    let squeezed = t(
        "| c5/2 e5   | g5/2 e5   |",
        "| c5/2 c5 | e5/1 e5 g5 g5 | e5/2 |",
    )
    .replace("| f5/2 d5   | c5/4      |\nbass", "| c5/4      |\nbass")
    .replace(
        "source = \"original\"",
        "source = \"Haydn\"\nedition = \"Rietz\"",
    );
    let pitches = format!("{squeezed}{HAYDN}transpose = 12\ncompare = \"pitches\"\n");
    assert_eq!(lints(&pitches, "quote"), []);
    let exact = format!("{squeezed}{HAYDN}transpose = 12\n");
    assert_eq!(lints(&exact, "quote").len(), 1);
}

#[test]
fn lint_instrument() {
    assert_eq!(
        lints(&t("\"bass\", volume", "\"tuba9\", volume"), "instrument"),
        [(14, Error)]
    );
    assert_eq!(
        lints(&t("| c3/2 g2   | c3/2", "| K/2 g2    | c3/2"), "instrument"),
        [(20, Error)]
    );
    assert_eq!(
        lints(
            &t(
                "| K/1 h K h | K/1 h S h |\n'''\n\n[pattern.B]",
                "| K/1 h K h | K/1 h X h |\n'''\n\n[pattern.B]"
            ),
            "instrument"
        ),
        [(21, Error)]
    );
}

#[test]
fn lint_clash() {
    // c5 over b2 on the first beat.
    assert_eq!(
        lints(
            &t("| c3/2 g2   | c3/2 g2", "| b2/2 g2   | c3/2 g2"),
            "clash"
        ),
        [(19, Warning)]
    );
}

#[test]
fn lint_consonance() {
    let text = t(
        "| c3/2 g2   | c3/2 g2   | g2/2 g2   | c3/4      |",
        "| b2/4      | b2/4      | b2/4      | b2/4      |",
    )
    .replace(
        "| c3/2 e3   | a2/2 e3   | g2/2 g2   | c3/4      |",
        "| b2/4      | b2/4      | b2/4      | b2/4      |",
    );
    assert_eq!(lints(&text, "consonance"), [(13, Warning)]);
}

#[test]
fn lint_melody_range() {
    let text = t("c6/2 g5", "c7/2 g5").replace("d5/2 b4", "d5/2 f3");
    assert_eq!(lints(&text, "melody-range"), [(13, Warning)]);
}

#[test]
fn lint_repeat_and_unused_pattern() {
    let text = t(
        "order = [\"A\", \"B\", \"A\", \"B\", \"A\"]",
        "order = [\"A\", \"A\", \"A\", \"A\", \"A\"]",
    );
    assert_eq!(lints(&text, "repeat"), [(9, Warning)]);
    assert_eq!(lints(&text, "unused-pattern"), [(24, Warning)]);
}

// --------------------------------------------------------------- render time

/// A 90 s, 6-channel song (3/4 at 128 bpm, 64 bars): lead, second voice,
/// bass, arpeggiated chords, a counter-melody and drums, 16th-note runs.
fn big_song() -> String {
    let lead = [
        "| c5/1 d5 e5 f5 g5/2 e5 c5/4 |",
        "| a5/2 g5 f5 e5 d5/4 |",
        "| b4/1 c5 d5 e5 f5/2 d5 b4/4 |",
        "| c5/12 |",
    ];
    let second = [
        "| e4/4 g4 e4 |",
        "| f4/4 a4 f4 |",
        "| d4/4 g4 f4 |",
        "| e4/12 |",
    ];
    let bass = [
        "| c3/4 g2 g2 |",
        "| f2/4 c3 c3 |",
        "| g2/4 d3 d3 |",
        "| c3/4 g2 c3 |",
    ];
    let chords = [
        "| r/4 [e4,g4,c5]/4' [e4,g4,c5]' |",
        "| r/4 [f4,a4,c5]/4' [f4,a4,c5]' |",
        "| r/4 [f4,g4,b4]/4' [f4,g4,b4]' |",
        "| r/4 [e4,g4,c5]/4' [e4,g4,c5]' |",
    ];
    let counter = [
        "| g4/2 a4 g4 f4 e4/4 |",
        "| a4/2 b4 c5 a4 f4/4 |",
        "| b4/2 a4 g4 f4 d4/4 |",
        "| c4/12 |",
    ];
    let drums = [
        "| K/4 h h |",
        "| K/4 [h,S] h |",
        "| K/4 h h |",
        "| [K,S]/4 S/2 S S/4 |",
    ];
    let mut score = String::new();
    for (name, bars) in [
        ("lead", lead),
        ("second", second),
        ("bass", bass),
        ("chords", chords),
        ("counter", counter),
        ("drums", drums),
    ] {
        score.push_str(&format!("{name} {}\n", bars.join(" ").replace("| |", "|")));
    }
    format!(
        r#"[meta]
id = "big"
kind = "loop"
tempo_bpm = 128
meter = "3/4"
steps_per_beat = 4
key = "c major"
bars = 64
order = ["A", "A", "A", "A", "B", "A", "A", "A", "A", "B", "A", "A", "A", "A", "B", "A"]
source = "original"

[channels]
lead = {{ instrument = "lead", volume_pct = 50 }}
second = {{ instrument = "lead", volume_pct = 30 }}
bass = {{ instrument = "bass", volume_pct = 60 }}
chords = {{ instrument = "chords", volume_pct = 40 }}
counter = {{ instrument = "lead", volume_pct = 25 }}
drums = {{ instrument = "drums", volume_pct = 50 }}

[pattern.A]
score = '''
{score}'''

[pattern.B]
key = "g major"
score = '''
{}'''
"#,
        score
            .replace("f4", "f#4")
            .replace("f5", "f#5")
            .replace("f2", "f#2")
    )
}

#[test]
fn render_time_of_a_90_s_six_channel_song() {
    let b = bank();
    let song = Song::parse("big.song", &big_song()).unwrap_or_else(|e| panic!("{e}"));
    let lints: Vec<_> = lint_song(&song, &b.ctx())
        .into_iter()
        .filter(|l| l.level == Error)
        .collect();
    assert_eq!(lints, []);
    let opts = RenderOptions::default();
    render_song(&song, &b.instruments, &opts).unwrap(); // warm-up
    let mut best = u128::MAX;
    let mut r = None;
    for _ in 0..3 {
        let t0 = Instant::now();
        r = Some(render_song(&song, &b.instruments, &opts).unwrap());
        best = best.min(t0.elapsed().as_micros());
    }
    let r = r.unwrap();
    let secs = r.render.pcm.samples.len() as u64 * 1000 / 44_100;
    println!(
        "render time: {} ms for {secs} ms of 6-channel music ({} voices, {} synthesised)",
        best / 1000,
        r.voices,
        r.rendered_voices
    );
    assert!((87_000..=93_000).contains(&secs), "{secs} ms");
    let budget_ms = if cfg!(debug_assertions) { 1000 } else { 50 };
    assert!(
        best / 1000 < budget_ms,
        "{} ms >= {budget_ms} ms",
        best / 1000
    );
}
