//! The patch formats: `.sfx` (one sound effect or jingle) and `.inst` (a
//! note-driven instrument or a drum kit). Both are TOML with integer,
//! unit-suffixed keys; see `docs/architecture.md` ("jnr-audio") for the
//! full key list.
//!
//! ```toml
//! # audio/sfx/jump.sfx
//! [[layer]]                # 1..=8 layers, summed
//! wave = "pulse"           # pulse | triangle | saw | sine | noise
//! duty_pct = 25            # pulse only (1..=99, default 50)
//! freq_hz = 330            # sfx: the start pitch (noise: the LFSR steps at 2 × freq_hz)
//! slide_to_hz = 660        # optional: glide (in cents) to this pitch ...
//! slide_ms = 60            # ... over this time (default: the rest of the sound)
//! slide_delay_ms = 0       # ... starting this long after the layer's start
//! attack_ms = 2            # linear 0 -> peak
//! hold_ms = 30             # peak
//! decay_ms = 90            # linear peak -> silence
//! volume_pct = 60          # 0..=100 of full scale
//! vibrato = { depth_cents = 20, rate_hz = 6 }   # optional; also delay_ms, to_depth_cents, ramp_ms
//! arp = { every_ms = 50, semitones = [0, 7] }   # optional
//! tremolo = { depth_pct = 60, rate_hz = 25 }   # optional amplitude wobble
//! lowpass_hz = 6000        # optional one-pole filters
//! highpass_hz = 80
//! bandpass = { hz = 2500, to_hz = 700, q_pct = 200 }  # optional 2-pole band-pass
//! delay_ms = 0             # start later than the sound
//! repeat = { count = 4, every_ms = 10 }        # optional: the layer again and again
//! ```
//!
//! An `.inst` file is either **melodic** (`range = ["e2", "c5"]` plus
//! `[[layer]]`s whose pitch comes from the note: `detune_cents` and
//! `slide_cents` instead of `freq_hz`/`slide_to_hz`, and an ADSR envelope
//! `attack_ms`, `decay_ms`, `sustain_pct`, `release_ms` instead of
//! `hold_ms`/`decay_ms`-to-silence), or a **kit**: `[kit]` maps single
//! letters to `[patch.<name>]` tables, each with `.sfx`-style
//! `[[patch.<name>.layer]]`s.

use std::collections::BTreeMap;
use std::ops::Range;

use serde::Deserialize;
use toml::Spanned;

use crate::error::{ErrorKind, ParseError, Src};
use crate::note::Note;

/// Most layers per patch.
pub const MAX_LAYERS: usize = 8;
/// Longest single time value (attack, hold, decay, release, delay, slide).
pub const MAX_MS: i64 = 10_000;
/// Most arpeggio steps.
pub const MAX_ARP_STEPS: usize = 8;
/// Pitch range of `freq_hz`, `slide_to_hz` and the one-pole filters (Hz).
pub const FREQ_RANGE: (i64, i64) = (20, 20_000);
/// Centre range of the band-pass (Hz; the filter stays stable below it).
pub const BANDPASS_RANGE: (i64, i64) = (20, 10_000);

/// A sound effect or jingle (`audio/sfx/*.sfx`, `audio/jingles/*.sfx`), or
/// one hit of a drum kit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sfx {
    /// The file stem (or the kit patch name).
    pub id: String,
    pub layers: Vec<Layer>,
}

/// An instrument (`audio/instruments/*.inst`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Instrument {
    pub id: String,
    pub kind: InstrumentKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InstrumentKind {
    /// Played by notes; `range` is the lowest and highest playable note.
    Melodic {
        range: (Note, Note),
        layers: Vec<Layer>,
    },
    /// Letters (as used in drum patterns) to fixed-pitch patches.
    Kit { hits: BTreeMap<char, Sfx> },
}

/// Oscillator shape.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Wave {
    Pulse { duty_pct: u32 },
    Triangle,
    Saw,
    Sine,
    Noise,
}

/// Where a layer's pitch comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pitch {
    /// `.sfx`: a fixed start pitch, optionally gliding to `slide_to_hz`.
    Fixed {
        freq_hz: u32,
        slide_to_hz: Option<u32>,
    },
    /// `.inst`: the played note, detuned, optionally gliding by `slide_cents`.
    Note { detune_cents: i32, slide_cents: i32 },
}

/// Amplitude envelope (linear segments).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Envelope {
    /// `.sfx`: 0 -> peak over `attack_ms`, peak for `hold_ms`, peak -> 0 over `decay_ms`.
    Ahd {
        attack_ms: u32,
        hold_ms: u32,
        decay_ms: u32,
    },
    /// `.inst`: 0 -> peak over `attack_ms`, peak -> sustain over `decay_ms`
    /// while the note is held, then current level -> 0 over `release_ms`.
    Adsr {
        attack_ms: u32,
        decay_ms: u32,
        sustain_pct: u32,
        release_ms: u32,
    },
}

/// Pitch wobble: a sine of `rate_hz`. Off for the first `delay_ms` (the
/// LFO starts at phase 0 there), then ± `depth_cents`, ramping linearly to
/// ± `to_depth_cents` over `ramp_ms` (a growing or fading vibrato).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Vibrato {
    pub depth_cents: u32,
    pub rate_hz: u32,
    pub delay_ms: u32,
    pub to_depth_cents: u32,
    pub ramp_ms: u32,
}

/// Arpeggio: every `every_ms` the next semitone offset of the list (cycling).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Arp {
    pub every_ms: u32,
    pub semitones: Vec<i32>,
}

/// Amplitude wobble: a sine of `rate_hz` pulling the level down by up to `depth_pct`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Tremolo {
    pub depth_pct: u32,
    pub rate_hz: u32,
}

/// Two-pole band-pass (state-variable filter), its centre gliding (in cents)
/// from `hz` to `to_hz` over the whole sound; unity gain at the centre.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bandpass {
    pub hz: u32,
    pub to_hz: u32,
    /// Quality × 100 (100 = Q 1, wide; 1000 = Q 10, narrow).
    pub q_pct: u32,
}

/// The layer played `count` times, `every_ms` apart (e.g. a hand clap's bursts).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Repeat {
    pub count: u32,
    pub every_ms: u32,
}

/// One oscillator with its envelope, modulation and filters.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Layer {
    pub wave: Wave,
    pub pitch: Pitch,
    /// Glide time; `None` = the whole sound (`.sfx`) or no glide (`.inst`
    /// without `slide_cents`).
    pub slide_ms: Option<u32>,
    /// The glide starts this long after the layer's start (0 = at once).
    pub slide_delay_ms: u32,
    pub envelope: Envelope,
    pub volume_pct: u32,
    pub vibrato: Vibrato,
    pub arp: Arp,
    pub tremolo: Tremolo,
    pub lowpass_hz: Option<u32>,
    pub highpass_hz: Option<u32>,
    pub bandpass: Option<Bandpass>,
    pub delay_ms: u32,
    pub repeat: Option<Repeat>,
}

impl Sfx {
    /// Parses an `.sfx` file; the id is the file stem of `file`.
    pub fn parse(file: &str, text: &str) -> Result<Sfx, ParseError> {
        let src = Src { file, text };
        let id = file_id(&src, ".sfx")?;
        let raw: RawSfx = from_toml(&src)?;
        let layers = layers(&src, &raw.layer, 0..0, Ctx::Sfx)?;
        Ok(Sfx { id, layers })
    }
}

impl Instrument {
    /// Parses an `.inst` file; the id is the file stem of `file`.
    pub fn parse(file: &str, text: &str) -> Result<Instrument, ParseError> {
        let src = Src { file, text };
        let id = file_id(&src, ".inst")?;
        let raw: RawInst = from_toml(&src)?;
        let kind = match raw.kit {
            Some(kit) => {
                if let Some(range) = &raw.range {
                    return Err(src.err(
                        range.span(),
                        ErrorKind::Key,
                        "a kit has no `range` (its patches have fixed pitches)",
                    ));
                }
                if let Some(first) = raw.layer.first() {
                    return Err(src.err(
                        first.span(),
                        ErrorKind::Key,
                        "a kit has no `[[layer]]`; put layers in `[[patch.<name>.layer]]`",
                    ));
                }
                InstrumentKind::Kit {
                    hits: kit_hits(&src, &kit, &raw.patch)?,
                }
            }
            None => {
                if let Some((name, patch)) = raw.patch.iter().next() {
                    return Err(src.err(
                        patch.span(),
                        ErrorKind::Kit,
                        format!("`[patch.{name}]` without a `[kit]` table"),
                    ));
                }
                let Some(range) = &raw.range else {
                    return Err(src.err(
                        0..0,
                        ErrorKind::Schema,
                        "missing `range = [\"<lowest>\", \"<highest>\"]` (or a `[kit]` table)",
                    ));
                };
                InstrumentKind::Melodic {
                    range: note_range(&src, range)?,
                    layers: layers(&src, &raw.layer, 0..0, Ctx::Inst)?,
                }
            }
        };
        Ok(Instrument { id, kind })
    }
}

// ------------------------------------------------------------- raw TOML shape

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSfx {
    #[serde(default)]
    layer: Vec<Spanned<RawLayer>>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawInst {
    #[serde(default)]
    range: Option<Spanned<Vec<Spanned<String>>>>,
    #[serde(default)]
    layer: Vec<Spanned<RawLayer>>,
    #[serde(default)]
    kit: Option<Spanned<BTreeMap<String, Spanned<String>>>>,
    #[serde(default)]
    patch: BTreeMap<String, Spanned<RawPatch>>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPatch {
    #[serde(default)]
    layer: Vec<Spanned<RawLayer>>,
}

type Num = Option<Spanned<i64>>;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawLayer {
    wave: Option<Spanned<String>>,
    duty_pct: Num,
    freq_hz: Num,
    slide_to_hz: Num,
    detune_cents: Num,
    slide_cents: Num,
    slide_ms: Num,
    slide_delay_ms: Num,
    attack_ms: Num,
    hold_ms: Num,
    decay_ms: Num,
    sustain_pct: Num,
    release_ms: Num,
    volume_pct: Num,
    vibrato: Option<Spanned<RawVibrato>>,
    arp: Option<Spanned<RawArp>>,
    lowpass_hz: Num,
    highpass_hz: Num,
    delay_ms: Num,
    tremolo: Option<Spanned<RawTremolo>>,
    bandpass: Option<Spanned<RawBandpass>>,
    repeat: Option<Spanned<RawRepeat>>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawTremolo {
    depth_pct: Spanned<i64>,
    rate_hz: Spanned<i64>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawBandpass {
    hz: Spanned<i64>,
    to_hz: Option<Spanned<i64>>,
    q_pct: Option<Spanned<i64>>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRepeat {
    count: Spanned<i64>,
    every_ms: Spanned<i64>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawVibrato {
    depth_cents: Spanned<i64>,
    rate_hz: Spanned<i64>,
    delay_ms: Option<Spanned<i64>>,
    to_depth_cents: Option<Spanned<i64>>,
    ramp_ms: Option<Spanned<i64>>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawArp {
    every_ms: Spanned<i64>,
    semitones: Vec<Spanned<i64>>,
}

// ------------------------------------------------------------------ checking

/// Which file kind a layer belongs to (decides the allowed keys).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Ctx {
    /// `.sfx` files and kit patches.
    Sfx,
    /// Melodic `.inst` layers.
    Inst,
}

pub(crate) fn from_toml<T: for<'de> Deserialize<'de>>(src: &Src) -> Result<T, ParseError> {
    toml::from_str(src.text).map_err(|e| {
        let span = e.span().unwrap_or(0..0);
        let msg = e.message().trim().to_owned();
        // toml reports both syntax and shape errors; shape errors come from
        // serde ("unknown field", "missing field", "invalid type").
        let kind = if [
            "unknown field",
            "missing field",
            "invalid type",
            "invalid length",
        ]
        .iter()
        .any(|p| msg.starts_with(p))
        {
            ErrorKind::Schema
        } else {
            ErrorKind::Syntax
        };
        src.err(span, kind, msg)
    })
}

/// The id from the file name: the stem after the last `/`, which must be
/// `[a-z][a-z0-9_]*`, with extension `ext`.
pub(crate) fn file_id(src: &Src, ext: &str) -> Result<String, ParseError> {
    let name = src.file.rsplit(['/', '\\']).next().unwrap_or(src.file);
    let Some(stem) = name.strip_suffix(ext) else {
        return Err(src.err(
            0..0,
            ErrorKind::Id,
            format!("file name must end in `{ext}`"),
        ));
    };
    if !valid_id(stem) {
        return Err(src.err(
            0..0,
            ErrorKind::Id,
            format!(
                "bad id `{stem}`: use lowercase letters, digits and `_`, starting with a letter"
            ),
        ));
    }
    Ok(stem.to_owned())
}

pub(crate) fn valid_id(s: &str) -> bool {
    s.chars().next().is_some_and(|c| c.is_ascii_lowercase())
        && s.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

/// Checks the layer count and every layer. `at` locates a "no layers"
/// error (the file start or the kit patch).
fn layers(
    src: &Src,
    raw: &[Spanned<RawLayer>],
    at: Range<usize>,
    ctx: Ctx,
) -> Result<Vec<Layer>, ParseError> {
    if raw.is_empty() {
        return Err(src.err(
            at,
            ErrorKind::Layers,
            format!("no `[[layer]]`: a patch needs 1..={MAX_LAYERS}"),
        ));
    }
    if raw.len() > MAX_LAYERS {
        return Err(src.err(
            raw[MAX_LAYERS].span(),
            ErrorKind::Layers,
            format!("{} layers; at most {MAX_LAYERS}", raw.len()),
        ));
    }
    raw.iter().map(|l| layer(src, l, ctx)).collect()
}

/// A number in `lo..=hi`, or `default` if the key is missing.
fn num(src: &Src, v: &Num, key: &str, lo: i64, hi: i64, default: i64) -> Result<i64, ParseError> {
    match v {
        None => Ok(default),
        Some(v) => bounded(src, v, key, lo, hi),
    }
}

fn bounded(src: &Src, v: &Spanned<i64>, key: &str, lo: i64, hi: i64) -> Result<i64, ParseError> {
    let n = *v.get_ref();
    if (lo..=hi).contains(&n) {
        Ok(n)
    } else {
        Err(src.err(
            v.span(),
            ErrorKind::Range,
            format!("`{key}` = {n} is outside {lo}..={hi}"),
        ))
    }
}

/// Errors if a key that `ctx` doesn't allow is present.
fn forbid(src: &Src, v: &Num, key: &str, why: &str) -> Result<(), ParseError> {
    match v {
        Some(v) => Err(src.err(
            v.span(),
            ErrorKind::Key,
            format!("`{key}` is not allowed {why}"),
        )),
        None => Ok(()),
    }
}

fn layer(src: &Src, sl: &Spanned<RawLayer>, ctx: Ctx) -> Result<Layer, ParseError> {
    let l = sl.get_ref();
    let at = sl.span();
    let missing = |key: &str| {
        src.err(
            at.clone(),
            ErrorKind::Schema,
            format!("missing `{key}` in this layer"),
        )
    };

    // Keys of the other file kind.
    match ctx {
        Ctx::Sfx => {
            let why = "in an .sfx layer (instrument key)";
            forbid(src, &l.detune_cents, "detune_cents", why)?;
            forbid(src, &l.slide_cents, "slide_cents", why)?;
            forbid(src, &l.sustain_pct, "sustain_pct", why)?;
            forbid(src, &l.release_ms, "release_ms", why)?;
        }
        Ctx::Inst => {
            let why = "in an .inst layer (the note sets the pitch; use detune_cents/slide_cents)";
            forbid(src, &l.freq_hz, "freq_hz", why)?;
            forbid(src, &l.slide_to_hz, "slide_to_hz", why)?;
            let why = "in an .inst layer (use sustain_pct/release_ms)";
            forbid(src, &l.hold_ms, "hold_ms", why)?;
        }
    }

    let wave_name = l.wave.as_ref().ok_or_else(|| missing("wave"))?;
    let wave = match wave_name.get_ref().as_str() {
        "pulse" => Wave::Pulse {
            duty_pct: num(src, &l.duty_pct, "duty_pct", 1, 99, 50)? as u32,
        },
        "triangle" => Wave::Triangle,
        "saw" => Wave::Saw,
        "sine" => Wave::Sine,
        "noise" => Wave::Noise,
        other => {
            return Err(src.err(
                wave_name.span(),
                ErrorKind::Wave,
                format!("unknown wave `{other}` (pulse | triangle | saw | sine | noise)"),
            ));
        }
    };
    if !matches!(wave, Wave::Pulse { .. }) {
        forbid(src, &l.duty_pct, "duty_pct", "on this wave (pulse only)")?;
    }

    let (flo, fhi) = FREQ_RANGE;
    let pitch = match ctx {
        Ctx::Sfx => Pitch::Fixed {
            freq_hz: bounded(
                src,
                l.freq_hz.as_ref().ok_or_else(|| missing("freq_hz"))?,
                "freq_hz",
                flo,
                fhi,
            )? as u32,
            slide_to_hz: match &l.slide_to_hz {
                Some(v) => Some(bounded(src, v, "slide_to_hz", flo, fhi)? as u32),
                None => None,
            },
        },
        Ctx::Inst => Pitch::Note {
            detune_cents: num(src, &l.detune_cents, "detune_cents", -2400, 2400, 0)? as i32,
            slide_cents: num(src, &l.slide_cents, "slide_cents", -4800, 4800, 0)? as i32,
        },
    };
    let slides = l.slide_to_hz.is_some() || l.slide_cents.is_some();
    if !slides {
        let why = "without `slide_to_hz`/`slide_cents`";
        forbid(src, &l.slide_ms, "slide_ms", why)?;
        forbid(src, &l.slide_delay_ms, "slide_delay_ms", why)?;
    }
    let slide_delay_ms = num(src, &l.slide_delay_ms, "slide_delay_ms", 0, MAX_MS, 0)? as u32;
    let slide_ms = match &l.slide_ms {
        Some(v) => Some(bounded(src, v, "slide_ms", 0, MAX_MS)? as u32),
        None => None,
    };
    if ctx == Ctx::Inst && l.slide_cents.is_some() && slide_ms.is_none() {
        return Err(missing("slide_ms"));
    }

    let ms = |v: &Num, key: &str| num(src, v, key, 0, MAX_MS, 0).map(|n| n as u32);
    let envelope = match ctx {
        Ctx::Sfx => {
            let env = Envelope::Ahd {
                attack_ms: ms(&l.attack_ms, "attack_ms")?,
                hold_ms: ms(&l.hold_ms, "hold_ms")?,
                decay_ms: ms(&l.decay_ms, "decay_ms")?,
            };
            if let Envelope::Ahd {
                attack_ms: 0,
                hold_ms: 0,
                decay_ms: 0,
            } = env
            {
                return Err(src.err(
                    at,
                    ErrorKind::Range,
                    "the layer has no length: set attack_ms, hold_ms or decay_ms",
                ));
            }
            env
        }
        Ctx::Inst => Envelope::Adsr {
            attack_ms: ms(&l.attack_ms, "attack_ms")?,
            decay_ms: ms(&l.decay_ms, "decay_ms")?,
            sustain_pct: num(src, &l.sustain_pct, "sustain_pct", 0, 100, 100)? as u32,
            release_ms: ms(&l.release_ms, "release_ms")?,
        },
    };

    let volume_pct = bounded(
        src,
        l.volume_pct.as_ref().ok_or_else(|| missing("volume_pct"))?,
        "volume_pct",
        0,
        100,
    )? as u32;

    let vibrato = match &l.vibrato {
        None => Vibrato::default(),
        Some(v) => {
            let r = v.get_ref();
            let depth = bounded(src, &r.depth_cents, "depth_cents", 0, 1200)?;
            let rate = bounded(src, &r.rate_hz, "rate_hz", 0, 50)?;
            let to_depth = match &r.to_depth_cents {
                Some(v) => bounded(src, v, "to_depth_cents", 0, 1200)?,
                None => depth,
            };
            let opt_ms = |v: &Option<Spanned<i64>>, key: &str| match v {
                Some(v) => bounded(src, v, key, 0, MAX_MS),
                None => Ok(0),
            };
            let delay = opt_ms(&r.delay_ms, "delay_ms")?;
            let ramp = opt_ms(&r.ramp_ms, "ramp_ms")?;
            if (depth > 0 || to_depth > 0) && rate == 0 {
                return Err(src.err(
                    r.rate_hz.span(),
                    ErrorKind::Range,
                    "`rate_hz` must be 1..=50 when `depth_cents` or `to_depth_cents` > 0",
                ));
            }
            Vibrato {
                depth_cents: depth as u32,
                rate_hz: rate as u32,
                delay_ms: delay as u32,
                to_depth_cents: to_depth as u32,
                ramp_ms: ramp as u32,
            }
        }
    };

    let arp = match &l.arp {
        None => Arp::default(),
        Some(a) => {
            let r = a.get_ref();
            if r.semitones.len() > MAX_ARP_STEPS {
                return Err(src.err(
                    a.span(),
                    ErrorKind::Range,
                    format!("{} arp steps; at most {MAX_ARP_STEPS}", r.semitones.len()),
                ));
            }
            let lo = if r.semitones.is_empty() { 0 } else { 1 };
            let every_ms = bounded(src, &r.every_ms, "every_ms", lo, 1000)? as u32;
            let semitones = r
                .semitones
                .iter()
                .map(|s| bounded(src, s, "semitones", -24, 24).map(|n| n as i32))
                .collect::<Result<Vec<_>, _>>()?;
            Arp {
                every_ms,
                semitones,
            }
        }
    };

    let tremolo = match &l.tremolo {
        None => Tremolo::default(),
        Some(t) => {
            let r = t.get_ref();
            Tremolo {
                depth_pct: bounded(src, &r.depth_pct, "depth_pct", 0, 100)? as u32,
                rate_hz: bounded(src, &r.rate_hz, "rate_hz", 1, 50)? as u32,
            }
        }
    };

    let (blo, bhi) = BANDPASS_RANGE;
    let bandpass = match &l.bandpass {
        None => None,
        Some(b) => {
            let r = b.get_ref();
            let hz = bounded(src, &r.hz, "hz", blo, bhi)? as u32;
            Some(Bandpass {
                hz,
                to_hz: match &r.to_hz {
                    Some(v) => bounded(src, v, "to_hz", blo, bhi)? as u32,
                    None => hz,
                },
                q_pct: match &r.q_pct {
                    Some(v) => bounded(src, v, "q_pct", 100, 2000)? as u32,
                    None => 100,
                },
            })
        }
    };

    let repeat = match &l.repeat {
        None => None,
        Some(rp) => {
            let r = rp.get_ref();
            Some(Repeat {
                count: bounded(src, &r.count, "count", 1, 64)? as u32,
                every_ms: bounded(src, &r.every_ms, "every_ms", 1, MAX_MS)? as u32,
            })
        }
    };
    if ctx == Ctx::Inst {
        if let Some(rp) = &l.repeat {
            return Err(src.err(
                rp.span(),
                ErrorKind::Key,
                "`repeat` is not allowed in an .inst layer (the song repeats notes)",
            ));
        }
    }

    let hz = |v: &Num, key: &str| -> Result<Option<u32>, ParseError> {
        match v {
            Some(v) => Ok(Some(bounded(src, v, key, flo, fhi)? as u32)),
            None => Ok(None),
        }
    };
    let lowpass_hz = hz(&l.lowpass_hz, "lowpass_hz")?;
    let highpass_hz = hz(&l.highpass_hz, "highpass_hz")?;
    if let (Some(lp), Some(hp), Some(span)) = (lowpass_hz, highpass_hz, l.highpass_hz.as_ref()) {
        if hp >= lp {
            return Err(src.err(
                span.span(),
                ErrorKind::Range,
                format!("`highpass_hz` = {hp} must be below `lowpass_hz` = {lp}"),
            ));
        }
    }

    Ok(Layer {
        wave,
        pitch,
        slide_ms,
        slide_delay_ms,
        envelope,
        volume_pct,
        vibrato,
        arp,
        lowpass_hz,
        highpass_hz,
        delay_ms: ms(&l.delay_ms, "delay_ms")?,
        tremolo,
        bandpass,
        repeat,
    })
}

fn note_range(
    src: &Src,
    range: &Spanned<Vec<Spanned<String>>>,
) -> Result<(Note, Note), ParseError> {
    let notes = range.get_ref();
    if notes.len() != 2 {
        return Err(src.err(
            range.span(),
            ErrorKind::Note,
            "`range` needs two notes: [\"<lowest>\", \"<highest>\"]",
        ));
    }
    let parse = |s: &Spanned<String>| {
        Note::parse(s.get_ref()).ok_or_else(|| {
            src.err(
                s.span(),
                ErrorKind::Note,
                format!(
                    "bad note `{}` (e.g. c4, f#3, bb2; octaves 0..=8)",
                    s.get_ref()
                ),
            )
        })
    };
    let (lo, hi) = (parse(&notes[0])?, parse(&notes[1])?);
    if lo > hi {
        return Err(src.err(
            range.span(),
            ErrorKind::Note,
            format!("`range` goes down: {lo} is above {hi}"),
        ));
    }
    Ok((lo, hi))
}

fn kit_hits(
    src: &Src,
    kit: &Spanned<BTreeMap<String, Spanned<String>>>,
    patches: &BTreeMap<String, Spanned<RawPatch>>,
) -> Result<BTreeMap<char, Sfx>, ParseError> {
    if kit.get_ref().is_empty() {
        return Err(src.err(kit.span(), ErrorKind::Kit, "`[kit]` maps no letters"));
    }
    let mut hits = BTreeMap::new();
    for (letter, name) in kit.get_ref() {
        let mut chars = letter.chars();
        let ch = match (chars.next(), chars.next()) {
            (Some(c), None) if c.is_ascii_alphabetic() => c,
            _ => {
                return Err(src.err(
                    name.span(),
                    ErrorKind::Kit,
                    format!("kit key `{letter}` must be a single letter A-Z or a-z"),
                ));
            }
        };
        let Some(patch) = patches.get(name.get_ref()) else {
            return Err(src.err(
                name.span(),
                ErrorKind::Kit,
                format!("no `[patch.{}]` in this file", name.get_ref()),
            ));
        };
        let layers = layers(src, &patch.get_ref().layer, patch.span(), Ctx::Sfx)?;
        hits.insert(
            ch,
            Sfx {
                id: name.get_ref().clone(),
                layers,
            },
        );
    }
    for (name, patch) in patches {
        if !valid_id(name) {
            return Err(src.err(
                patch.span(),
                ErrorKind::Kit,
                format!("bad patch name `{name}`"),
            ));
        }
        if !kit.get_ref().values().any(|n| n.get_ref() == name) {
            return Err(src.err(
                patch.span(),
                ErrorKind::Kit,
                format!("`[patch.{name}]` is not used by `[kit]`"),
            ));
        }
    }
    Ok(hits)
}
