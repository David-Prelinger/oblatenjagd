//! Parser errors: one test per `ErrorKind`, each checking `file:line`.

use jnr_audio::{ErrorKind, Instrument, InstrumentKind, ParseError, Sfx, Wave};

const GOOD_LAYER: &str = "wave = \"pulse\"\nfreq_hz = 440\ndecay_ms = 100\nvolume_pct = 50\n";

fn sfx_err(text: &str) -> ParseError {
    Sfx::parse("audio/sfx/test.sfx", text).expect_err("should fail")
}

fn inst_err(text: &str) -> ParseError {
    Instrument::parse("audio/instruments/test.inst", text).expect_err("should fail")
}

#[track_caller]
fn check(e: ParseError, kind: ErrorKind, line: usize, needle: &str) {
    let shown = e.to_string();
    assert_eq!(e.kind, kind, "{shown}");
    assert_eq!(e.line, line, "{shown}");
    assert!(shown.contains(needle), "`{needle}` not in: {shown}");
    let prefix = format!("{}:{line}:", e.file);
    assert!(shown.starts_with(&prefix), "{shown}");
    assert!(
        shown.contains(&format!("error[{}]", kind.as_str())),
        "{shown}"
    );
}

#[test]
fn the_fixtures_parse() {
    let jump = Sfx::parse(
        "audio/sfx/jump.sfx",
        include_str!("../../../audio/sfx/jump.sfx"),
    )
    .unwrap();
    assert_eq!(jump.id, "jump");
    assert_eq!(jump.layers[0].wave, Wave::Pulse { duty_pct: 25 });
    for (file, text) in [
        (
            "audio/sfx/oblate.sfx",
            include_str!("../../../audio/sfx/oblate.sfx"),
        ),
        (
            "audio/sfx/klatschen.sfx",
            include_str!("../../../audio/sfx/klatschen.sfx"),
        ),
    ] {
        Sfx::parse(file, text).unwrap();
    }
    let tuba = Instrument::parse(
        "audio/instruments/tuba.inst",
        include_str!("../../../audio/instruments/tuba.inst"),
    )
    .unwrap();
    assert!(matches!(tuba.kind, InstrumentKind::Melodic { ref layers, .. } if layers.len() == 2));
    let kit = Instrument::parse(
        "audio/instruments/kit.inst",
        include_str!("../../../audio/instruments/kit.inst"),
    )
    .unwrap();
    let InstrumentKind::Kit { hits } = kit.kind else {
        panic!("kit expected")
    };
    assert_eq!(hits.keys().collect::<String>(), "KSh");
    assert_eq!(hits[&'S'].id, "snare");
}

#[test]
fn syntax() {
    check(
        sfx_err("[[layer]]\nwave = \"pulse\nfreq_hz = 1\n"),
        ErrorKind::Syntax,
        2,
        "",
    );
    check(sfx_err("[[layer]\n"), ErrorKind::Syntax, 1, "");
}

#[test]
fn schema_unknown_key_float_and_missing_key() {
    check(
        sfx_err(&format!("[[layer]]\n{GOOD_LAYER}colour = 3\n")),
        ErrorKind::Schema,
        6,
        "unknown field `colour`",
    );
    check(
        sfx_err("[[layer]]\nwave = \"pulse\"\nfreq_hz = 330.5\ndecay_ms = 9\nvolume_pct = 5\n"),
        ErrorKind::Schema,
        3,
        "floating point",
    );
    check(
        sfx_err("\n[[layer]]\nwave = \"pulse\"\ndecay_ms = 9\nvolume_pct = 5\n"),
        ErrorKind::Schema,
        2,
        "missing `freq_hz`",
    );
    check(
        sfx_err("[[layer]]\nwave = \"pulse\"\nfreq_hz = 9000\ndecay_ms = 9\n"),
        ErrorKind::Schema,
        1,
        "missing `volume_pct`",
    );
    check(
        inst_err("[[layer]]\nwave = \"saw\"\nvolume_pct = 5\n"),
        ErrorKind::Schema,
        1,
        "missing `range",
    );
}

#[test]
fn key_not_allowed_here() {
    let inst = |extra: &str| {
        format!("range = [\"c2\", \"c4\"]\n[[layer]]\nwave = \"saw\"\nvolume_pct = 50\n{extra}")
    };
    check(
        inst_err(&inst("hold_ms = 10\n")),
        ErrorKind::Key,
        5,
        "`hold_ms` is not allowed",
    );
    check(
        inst_err(&inst("freq_hz = 10\n")),
        ErrorKind::Key,
        5,
        "`freq_hz` is not allowed",
    );
    check(
        inst_err(&inst("repeat = { count = 2, every_ms = 5 }\n")),
        ErrorKind::Key,
        5,
        "`repeat` is not allowed",
    );
    check(
        sfx_err(&format!("[[layer]]\n{GOOD_LAYER}release_ms = 10\n")),
        ErrorKind::Key,
        6,
        "`release_ms` is not allowed",
    );
    check(
        sfx_err("[[layer]]\nwave = \"saw\"\nduty_pct = 25\nfreq_hz = 9\n"),
        ErrorKind::Key,
        3,
        "pulse only",
    );
    check(
        sfx_err(&format!("[[layer]]\n{GOOD_LAYER}slide_ms = 10\n")),
        ErrorKind::Key,
        6,
        "without `slide_to_hz`",
    );
}

#[test]
fn range() {
    check(
        sfx_err("[[layer]]\nwave = \"pulse\"\nfreq_hz = 5\n"),
        ErrorKind::Range,
        3,
        "`freq_hz` = 5 is outside 20..=20000",
    );
    check(
        sfx_err(&format!("[[layer]]\n{GOOD_LAYER}").replace("volume_pct = 50", "volume_pct = 101")),
        ErrorKind::Range,
        5,
        "volume_pct",
    );
    check(
        sfx_err(&format!(
            "[[layer]]\n{GOOD_LAYER}lowpass_hz = 500\nhighpass_hz = 500\n"
        )),
        ErrorKind::Range,
        7,
        "must be below",
    );
    check(
        sfx_err("[[layer]]\nwave = \"pulse\"\nfreq_hz = 440\nvolume_pct = 50\n"),
        ErrorKind::Range,
        1,
        "no length",
    );
    check(
        sfx_err(&format!(
            "[[layer]]\n{GOOD_LAYER}vibrato = {{ depth_cents = 30, rate_hz = 0 }}\n"
        )),
        ErrorKind::Range,
        6,
        "rate_hz",
    );
    check(
        sfx_err(&format!(
            "[[layer]]\n{GOOD_LAYER}bandpass = {{ hz = 15000 }}\n"
        )),
        ErrorKind::Range,
        6,
        "outside 20..=10000",
    );
}

#[test]
fn wave() {
    check(
        sfx_err("[[layer]]\nfreq_hz = 440\nwave = \"square\"\n"),
        ErrorKind::Wave,
        3,
        "unknown wave `square`",
    );
}

#[test]
fn layers() {
    check(
        sfx_err("# nothing\n"),
        ErrorKind::Layers,
        1,
        "no `[[layer]]`",
    );
    let nine = format!("[[layer]]\n{GOOD_LAYER}").repeat(9);
    check(sfx_err(&nine), ErrorKind::Layers, 41, "9 layers; at most 8");
}

#[test]
fn note() {
    let with = |r: &str| format!("range = {r}\n[[layer]]\nwave = \"saw\"\nvolume_pct = 5\n");
    check(
        inst_err(&with("[\"h1\", \"c4\"]")),
        ErrorKind::Note,
        1,
        "bad note `h1`",
    );
    check(
        inst_err(&with("[\"c4\", \"c2\"]")),
        ErrorKind::Note,
        1,
        "goes down",
    );
    check(inst_err(&with("[\"c2\"]")), ErrorKind::Note, 1, "two notes");
}

#[test]
fn kit() {
    let patch = |name: &str| format!("[[patch.{name}.layer]]\n{GOOD_LAYER}");
    check(
        inst_err(&format!("[kit]\nK = \"kick\"\n{}", patch("snare"))),
        ErrorKind::Kit,
        2,
        "no `[patch.kick]`",
    );
    check(
        inst_err(&format!(
            "[kit]\nK = \"kick\"\n{}{}",
            patch("kick"),
            patch("snare")
        )),
        ErrorKind::Kit,
        8,
        "`[patch.snare]` is not used",
    );
    check(
        inst_err(&format!("[kit]\nKK = \"kick\"\n{}", patch("kick"))),
        ErrorKind::Kit,
        2,
        "single letter",
    );
    check(
        inst_err(&patch("kick")),
        ErrorKind::Kit,
        1,
        "without a `[kit]`",
    );
    check(inst_err("[kit]\n"), ErrorKind::Kit, 1, "maps no letters");
    check(
        inst_err(&format!(
            "range = [\"c1\", \"c2\"]\n[kit]\nK = \"kick\"\n{}",
            patch("kick")
        )),
        ErrorKind::Key,
        1,
        "a kit has no `range`",
    );
}

#[test]
fn id() {
    let e = Sfx::parse("audio/sfx/Jump.sfx", GOOD_LAYER).unwrap_err();
    check(e, ErrorKind::Id, 1, "bad id `Jump`");
    let e = Sfx::parse("audio/sfx/jump.txt", GOOD_LAYER).unwrap_err();
    check(e, ErrorKind::Id, 1, "must end in `.sfx`");
    let e = Instrument::parse("x/2tuba.inst", GOOD_LAYER).unwrap_err();
    check(e, ErrorKind::Id, 1, "bad id `2tuba`");
}
