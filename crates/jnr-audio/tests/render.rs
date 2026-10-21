//! Rendering: determinism, pitch, envelope timing, clamping, WAV round trip.

use jnr_audio::note::Note;
use jnr_audio::patch::InstrumentKind;
use jnr_audio::render::{CUT_FADE, add_sfx, add_sfx_shifted, add_sfx_until, to_pcm};
use jnr_audio::{Instrument, Sfx, ms_to_samples, render_note, render_sfx, wav};

fn sfx(text: &str) -> Sfx {
    Sfx::parse("test/fixture.sfx", text).unwrap()
}

/// Sign changes between consecutive non-zero samples.
fn zero_crossings(s: &[i16]) -> usize {
    let signs: Vec<bool> = s.iter().filter(|&&v| v != 0).map(|&v| v > 0).collect();
    signs.windows(2).filter(|w| w[0] != w[1]).count()
}

fn tone(wave: &str, hz: u32, extra: &str) -> Sfx {
    sfx(&format!(
        "[[layer]]\nwave = \"{wave}\"\nfreq_hz = {hz}\nhold_ms = 1000\nvolume_pct = 50\n{extra}"
    ))
}

#[test]
fn same_patch_renders_the_same_bits() {
    let text = include_str!("fixtures/oblate.sfx");
    let a = render_sfx(&Sfx::parse("tests/fixtures/oblate.sfx", text).unwrap());
    let b = render_sfx(&Sfx::parse("tests/fixtures/oblate.sfx", text).unwrap());
    assert_eq!(a, b);
    assert_eq!(a.pcm.fnv64(), b.pcm.fnv64());
}

#[test]
fn pinned_hash_of_a_patch_using_every_feature() {
    // Integer-only maths: this hash must be the same on every platform
    // (native and wasm). A change here means the synth changed.
    let s = sfx("\
[[layer]]
wave = \"pulse\"
duty_pct = 30
freq_hz = 300
slide_to_hz = 500
attack_ms = 3
hold_ms = 40
decay_ms = 60
volume_pct = 30
vibrato = { depth_cents = 25, rate_hz = 7, delay_ms = 20, to_depth_cents = 5, ramp_ms = 40 }
slide_delay_ms = 10
arp = { every_ms = 20, semitones = [0, 4, 7] }
lowpass_hz = 5000
highpass_hz = 100
tremolo = { depth_pct = 40, rate_hz = 12 }

[[layer]]
wave = \"noise\"
freq_hz = 15000
attack_ms = 1
decay_ms = 30
volume_pct = 20
bandpass = { hz = 3000, to_hz = 800, q_pct = 300 }
repeat = { count = 3, every_ms = 15 }

[[layer]]
wave = \"triangle\"
freq_hz = 150
delay_ms = 10
decay_ms = 80
volume_pct = 20

[[layer]]
wave = \"saw\"
freq_hz = 220
decay_ms = 50
volume_pct = 10

[[layer]]
wave = \"sine\"
freq_hz = 880
decay_ms = 50
volume_pct = 10
");
    let r = render_sfx(&s);
    assert_eq!(r.clamped, 0);
    assert_eq!(r.pcm.samples.len(), ms_to_samples(103) as usize);
    assert_eq!(format!("{:016x}", r.pcm.fnv64()), PINNED);
}
const PINNED: &str = "d22bc56b3d2bf335";

#[test]
fn pulse_pitch_is_correct() {
    for (hz, duty) in [(440, 50), (440, 12), (1000, 75), (55, 50)] {
        let r = render_sfx(&tone("pulse", hz, &format!("duty_pct = {duty}\n")));
        let zc = zero_crossings(&r.pcm.samples) as i64;
        assert!(
            (zc - 2 * hz as i64).abs() <= 1,
            "{hz} Hz duty {duty}: {zc} crossings"
        );
    }
}

/// A pitched variant (`cues::PITCHED`, E60): the whole sound moves by the
/// offset, its length stays.
#[test]
fn shifted_effects_move_the_pitch_and_keep_the_length() {
    let patch = tone("sine", 440, "");
    let plain = render_sfx(&patch).pcm.samples;
    for (cents, hz) in [(0, 440), (1200, 880), (-500, 330)] {
        let mut mix = Vec::new();
        add_sfx_shifted(&mut mix, 0, &patch, 100, cents);
        let r = to_pcm(&mix).pcm.samples;
        assert_eq!(r.len(), plain.len());
        let zc = zero_crossings(&r) as i64;
        assert!((zc - 2 * hz).abs() <= 2, "{cents} cents: {zc} crossings");
    }
    let mut mix = Vec::new();
    add_sfx_shifted(&mut mix, 0, &patch, 100, 0);
    assert_eq!(to_pcm(&mix).pcm.samples, plain);
}

#[test]
fn other_waves_have_the_right_pitch_too() {
    for wave in ["triangle", "saw", "sine"] {
        let r = render_sfx(&tone(wave, 330, ""));
        let zc = zero_crossings(&r.pcm.samples) as i64;
        assert!((zc - 660).abs() <= 2, "{wave}: {zc} crossings");
    }
}

#[test]
fn envelope_length_is_exact_to_the_sample() {
    let s = sfx(
        "[[layer]]\nwave = \"pulse\"\nfreq_hz = 440\ndelay_ms = 5\nattack_ms = 10\nhold_ms = 20\ndecay_ms = 30\nvolume_pct = 50\n",
    );
    let pcm = render_sfx(&s).pcm;
    assert_eq!(pcm.samples.len(), ms_to_samples(65) as usize); // 2866
    let onset = ms_to_samples(5) as usize; // 220
    assert!(
        pcm.samples[..=onset].iter().all(|&v| v == 0),
        "silent through the first attack sample"
    );
    assert_ne!(pcm.samples[onset + 1], 0);
    assert_ne!(
        *pcm.samples.last().unwrap(),
        0,
        "the decay ends on its last sample"
    );
    // Hold: full level (50 % of 32767) on every hold sample.
    let hold = ms_to_samples(15) as usize..ms_to_samples(35) as usize;
    // (The negative half rounds down: -16384.)
    assert!(pcm.samples[hold].iter().all(|&v| v == 16383 || v == -16384));
}

#[test]
fn clamps_are_counted() {
    let r = to_pcm(&[40_000, -40_000, 32_767, -32_767, -32_768, 0]);
    assert_eq!(r.clamped, 3);
    assert_eq!(
        r.pcm.samples,
        [32_767, -32_767, 32_767, -32_767, -32_767, 0]
    );

    let one = "[[layer]]\nwave = \"pulse\"\nfreq_hz = 440\nhold_ms = 10\nvolume_pct = 100\n";
    assert_eq!(
        render_sfx(&sfx(one)).clamped,
        0,
        "one full-scale layer fits"
    );
    let two = render_sfx(&sfx(&one.repeat(2)));
    assert_eq!(
        two.clamped as usize,
        two.pcm.samples.len(),
        "two in phase clamp everywhere"
    );
}

#[test]
fn wav_round_trips_a_render() {
    let pcm = render_sfx(
        &Sfx::parse("tests/fixtures/jump.sfx", include_str!("fixtures/jump.sfx")).unwrap(),
    )
    .pcm;
    let bytes = wav::write(&pcm);
    assert_eq!(&bytes[..4], b"RIFF");
    assert_eq!(wav::read(&bytes), Ok(pcm));
}

#[test]
fn slides_and_arpeggios_move_the_pitch() {
    // 200 -> 800 Hz over the second: the last 100 ms cross far more often.
    let r = render_sfx(&tone("sine", 200, "slide_to_hz = 800\n"))
        .pcm
        .samples;
    let tenth = r.len() / 10;
    let (first, last) = (
        zero_crossings(&r[..tenth]),
        zero_crossings(&r[r.len() - tenth..]),
    );
    assert!(
        (38..=44).contains(&first) && (140..=162).contains(&last),
        "{first} {last}"
    );
    // Arp of an octave every 100 ms: alternate windows differ by 2x.
    let r = render_sfx(&tone(
        "sine",
        300,
        "arp = { every_ms = 100, semitones = [0, 12] }\n",
    ))
    .pcm
    .samples;
    let w = ms_to_samples(100) as usize;
    let (a, b) = (zero_crossings(&r[..w]), zero_crossings(&r[w..2 * w]));
    assert!(
        (59..=61).contains(&a) && (119..=121).contains(&b),
        "{a} {b}"
    );
}

#[test]
fn noise_and_filters_stay_bounded_and_centred() {
    for extra in [
        "",
        "bandpass = { hz = 10000, q_pct = 100 }\n",
        "bandpass = { hz = 20, q_pct = 2000 }\n",
        "bandpass = { hz = 300, to_hz = 9000, q_pct = 2000 }\n",
        "lowpass_hz = 20\n",
        "highpass_hz = 20000\n",
    ] {
        let r = render_sfx(&tone("noise", 20_000, extra));
        assert_eq!(r.clamped, 0, "{extra}");
        let s = &r.pcm.samples;
        let dc: i64 = s.iter().map(|&v| v as i64).sum::<i64>() / s.len() as i64;
        assert!(dc.abs() < 165, "{extra}: DC {dc}"); // < 0.5 % of full scale
        assert!(s.iter().any(|&v| v != 0), "{extra}: silent");
    }
}

#[test]
fn band_pass_passes_its_centre_and_damps_the_rest() {
    let rms = |hz: u32| {
        let r = render_sfx(&tone("sine", hz, "bandpass = { hz = 1000, q_pct = 400 }\n"));
        let s = &r.pcm.samples[4410..];
        ((s.iter().map(|&v| v as i64 * v as i64).sum::<i64>() / s.len() as i64) as f64).sqrt()
    };
    let (centre, low, high) = (rms(1000), rms(200), rms(5000));
    // Input RMS: 16383 / sqrt(2) = 11585.
    assert!((10_500.0..12_000.0).contains(&centre), "{centre}");
    assert!(low < centre / 4.0 && high < centre / 4.0, "{low} {high}");
}

#[test]
fn repeat_plays_a_layer_again() {
    let once = render_sfx(&tone("noise", 9000, "")).pcm.samples.len();
    let r = render_sfx(&tone(
        "noise",
        9000,
        "repeat = { count = 3, every_ms = 500 }\n",
    ))
    .pcm
    .samples;
    assert_eq!(r.len(), once + ms_to_samples(1000) as usize);
}

#[test]
fn instrument_notes_have_pitch_and_release() {
    let inst = Instrument::parse(
        "x/organ.inst",
        "range = [\"c2\", \"c6\"]\n[[layer]]\nwave = \"sine\"\nattack_ms = 5\ndecay_ms = 50\nsustain_pct = 60\nrelease_ms = 100\nvolume_pct = 50\n",
    )
    .unwrap();
    let InstrumentKind::Melodic { layers, .. } = &inst.kind else {
        panic!()
    };
    let gate = ms_to_samples(1000);
    let r = render_note(layers, Note::parse("a4").unwrap(), gate);
    assert_eq!(r.pcm.samples.len(), (gate + ms_to_samples(100)) as usize);
    let held = &r.pcm.samples[..gate as usize];
    assert!((zero_crossings(held) as i64 - 880).abs() <= 2);
    // Sustain at 60 % of 50 %: peak 9830 (± interpolation).
    let peak = held[gate as usize / 2..]
        .iter()
        .map(|v| v.abs())
        .max()
        .unwrap();
    assert!((9800..=9831).contains(&peak), "{peak}");
}

#[test]
fn the_fixture_kit_renders_every_hit() {
    let kit =
        Instrument::parse("tests/fixtures/kit.inst", include_str!("fixtures/kit.inst")).unwrap();
    let InstrumentKind::Kit { hits } = &kit.kind else {
        panic!()
    };
    for (letter, hit) in hits {
        let r = render_sfx(hit);
        assert_eq!(r.clamped, 0, "{letter}");
        assert!(r.pcm.samples.iter().any(|&v| v.abs() > 1000), "{letter}");
    }
}

/// Max − min of the distances between rising zero crossings.
fn period_spread(s: &[i16]) -> usize {
    let ups: Vec<usize> = (1..s.len())
        .filter(|&i| s[i - 1] < 0 && s[i] >= 0)
        .collect();
    let d: Vec<usize> = ups.windows(2).map(|w| w[1] - w[0]).collect();
    d.iter().max().unwrap() - d.iter().min().unwrap()
}

#[test]
fn vibrato_waits_for_its_delay_and_ramps_its_depth() {
    let plain = render_sfx(&tone("sine", 440, "")).pcm.samples;
    let vib = render_sfx(&tone(
        "sine",
        440,
        "vibrato = { depth_cents = 100, rate_hz = 6, delay_ms = 300 }\n",
    ))
    .pcm
    .samples;
    let delay = ms_to_samples(300) as usize; // 13230
    // No jitter before the delay (the next control point after it is 13248).
    assert_eq!(plain[..delay], vib[..delay]);
    assert!(period_spread(&vib[..delay]) <= 1);
    assert!(
        period_spread(&vib[delay + 4410..]) >= 8,
        "jitter after the delay"
    );

    // Fading vibrato (roof_bounce style): ±100 -> 0 over 200 ms.
    let fade = render_sfx(&tone(
        "sine",
        440,
        "vibrato = { depth_cents = 100, rate_hz = 8, to_depth_cents = 0, ramp_ms = 200 }\n",
    ))
    .pcm
    .samples;
    let w = ms_to_samples(200) as usize;
    assert!(period_spread(&fade[..w]) >= 4);
    assert!(period_spread(&fade[w + 64..]) <= 1, "steady after the ramp");

    // Growing vibrato (zucker style): 0 -> ±50 over 500 ms.
    let grow = render_sfx(&tone(
        "sine",
        440,
        "vibrato = { depth_cents = 0, rate_hz = 8, to_depth_cents = 50, ramp_ms = 500 }\n",
    ))
    .pcm
    .samples;
    let early = period_spread(&grow[..ms_to_samples(60) as usize]);
    let late = period_spread(&grow[ms_to_samples(600) as usize..]);
    assert!(early <= 1 && late >= 4, "{early} {late}");
}

#[test]
fn a_slide_can_start_late() {
    let plain = render_sfx(&tone("sine", 200, "")).pcm.samples;
    let late = render_sfx(&tone(
        "sine",
        200,
        "slide_to_hz = 400\nslide_delay_ms = 500\nslide_ms = 100\n",
    ))
    .pcm
    .samples;
    let at = ms_to_samples(500) as usize; // 22050, a control point is at 22048
    assert_eq!(plain[..22_048], late[..22_048]);
    let tail = &late[ms_to_samples(700) as usize..];
    let zc = zero_crossings(tail) as i64;
    assert!((zc - 240).abs() <= 2, "400 Hz over the last 300 ms: {zc}");
    assert!(at > 22_048);
}

#[test]
fn notes_count_their_gate_from_the_onset_and_gate_zero_is_silent() {
    let inst = Instrument::parse(
        "x/pad.inst",
        "range = [\"c2\", \"c6\"]\n[[layer]]\nwave = \"sine\"\ndelay_ms = 10\nattack_ms = 5\nrelease_ms = 20\nvolume_pct = 50\n",
    )
    .unwrap();
    let InstrumentKind::Melodic { layers, .. } = &inst.kind else {
        panic!()
    };
    let a4 = Note::parse("a4").unwrap();
    let gate = ms_to_samples(50);
    let r = render_note(layers, a4, gate);
    assert_eq!(r.pcm.samples.len(), (gate + ms_to_samples(20)) as usize);
    // Gate 0: no attack, no release, nothing after the delay.
    let r = render_note(layers, a4, 0);
    assert!(r.pcm.samples.iter().all(|&v| v == 0));
    assert_eq!(r.pcm.samples.len(), ms_to_samples(10) as usize);
    // A gate shorter than the delay: silent too.
    let r = render_note(layers, a4, ms_to_samples(5));
    assert!(r.pcm.samples.iter().all(|&v| v == 0));
}

#[test]
fn a_cut_voice_fades_out_in_2_ms() {
    let s = tone("pulse", 440, "");
    let mut full = Vec::new();
    add_sfx(&mut full, 100, &s, 100);
    let mut cut = Vec::new();
    let until = 100 + ms_to_samples(250) as usize;
    add_sfx_until(&mut cut, 100, &s, 100, until);
    assert_eq!(cut.len(), until + CUT_FADE);
    assert_eq!(cut[..until], full[..until]);
    for (i, &v) in cut[until..].iter().enumerate() {
        assert!(v.abs() <= full[until + i].abs());
    }
    assert!(cut.last().unwrap().abs() < full[until].abs() / 40);
    // A voice that starts at the cut adds nothing.
    let mut none = vec![0; 10];
    add_sfx_until(&mut none, 50, &s, 100, 50);
    assert_eq!(none, vec![0; 10]);
}
