//! Patches to PCM: voices are summed into an `i32` mix buffer, which is then
//! clamped to `i16` (every clamped sample is counted; the lints treat any
//! clamp as an error).

use crate::note::Note;
use crate::patch::{Layer, Sfx};
use crate::synth::Voice;
use crate::{SAMPLE_RATE, ms_to_samples};

const _: () = assert!(CUT_FADE == (2 * SAMPLE_RATE / 1000) as usize);

/// Mono 16-bit PCM.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pcm {
    /// Samples per second (always [`SAMPLE_RATE`] for renders).
    pub rate: u32,
    pub samples: Vec<i16>,
}

impl Pcm {
    /// Length in milliseconds, rounded down.
    pub fn duration_ms(&self) -> u64 {
        self.samples.len() as u64 * 1000 / self.rate.max(1) as u64
    }

    /// FNV-1a 64 over the rate and the little-endian samples: the
    /// golden fingerprint of a render (`fnv64:<16 hex digits>`).
    pub fn fnv64(&self) -> u64 {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        let bytes = self
            .rate
            .to_le_bytes()
            .into_iter()
            .chain(self.samples.iter().flat_map(|s| s.to_le_bytes()));
        for b in bytes {
            h ^= b as u64;
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
        h
    }
}

/// A finished render and how many samples had to be clamped.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Render {
    pub pcm: Pcm,
    pub clamped: u32,
}

/// Clamps a mix buffer to `i16`, counting the clamped samples.
pub fn to_pcm(mix: &[i32]) -> Render {
    let mut clamped = 0;
    let samples = mix
        .iter()
        .map(|&v| {
            if v > i16::MAX as i32 || v < -(i16::MAX as i32) {
                clamped += 1;
            }
            v.clamp(-(i16::MAX as i32), i16::MAX as i32) as i16
        })
        .collect();
    Render {
        pcm: Pcm {
            rate: SAMPLE_RATE,
            samples,
        },
        clamped,
    }
}

/// Length of the fade when a voice is cut early (2 ms).
pub const CUT_FADE: usize = 88;

/// Adds `voice` at `at`, cut at the absolute sample `until` if given: it
/// fades out linearly over [`CUT_FADE`] samples from `until` on and adds
/// nothing after that (a voice starting at or after `until` adds nothing).
fn add_cut(out: &mut Vec<i32>, at: usize, voice: &Voice, until: Option<usize>) {
    let Some(until) = until else {
        voice.add_into(out, at);
        return;
    };
    if at >= until {
        return;
    }
    let mut tmp = Vec::new();
    voice.add_into(&mut tmp, 0);
    let keep = tmp.len().min(until + CUT_FADE - at);
    if out.len() < at + keep {
        out.resize(at + keep, 0);
    }
    for (i, &v) in tmp[..keep].iter().enumerate() {
        let pos = at + i;
        out[pos] += if pos < until {
            v
        } else {
            // Fade: (CUT_FADE - k) / CUT_FADE for the k-th sample past `until`.
            (v as i64 * (CUT_FADE - (pos - until)) as i64 / CUT_FADE as i64) as i32
        };
    }
}

/// Adds every repetition of `voice` (see `repeat`) from sample `start` on.
fn add_voice(out: &mut Vec<i32>, start: usize, voice: &Voice, layer: &Layer, until: Option<usize>) {
    match layer.repeat {
        None => add_cut(out, start, voice, until),
        Some(r) => {
            for i in 0..r.count {
                let at = start + ms_to_samples(i * r.every_ms) as usize;
                add_cut(out, at, voice, until);
            }
        }
    }
}

/// Adds an effect (every layer, at `volume_pct`) into `out` from `start` on.
pub fn add_sfx(out: &mut Vec<i32>, start: usize, sfx: &Sfx, volume_pct: u32) {
    for layer in &sfx.layers {
        add_voice(out, start, &Voice::sfx(layer, volume_pct), layer, None);
    }
}

/// Like [`add_sfx`], but the effect is cut at the absolute sample `until`
/// (a 2 ms fade, see [`CUT_FADE`]), e.g. when it is retriggered.
pub fn add_sfx_until(out: &mut Vec<i32>, start: usize, sfx: &Sfx, volume_pct: u32, until: usize) {
    for layer in &sfx.layers {
        add_voice(
            out,
            start,
            &Voice::sfx(layer, volume_pct),
            layer,
            Some(until),
        );
    }
}

/// Adds one note of a melodic instrument's `layers`, held for `gate`
/// samples from its onset (the release follows), at `volume_pct`.
pub fn add_note(
    out: &mut Vec<i32>,
    start: usize,
    layers: &[Layer],
    note: Note,
    gate: u32,
    volume_pct: u32,
) {
    for layer in layers {
        let voice = Voice::note(layer, note.0, gate, volume_pct);
        add_voice(out, start, &voice, layer, None);
    }
}

/// Like [`add_note`], but the note (release included) is cut at the
/// absolute sample `until` with a 2 ms fade (see [`CUT_FADE`]).
pub fn add_note_until(
    out: &mut Vec<i32>,
    start: usize,
    layers: &[Layer],
    note: Note,
    gate: u32,
    volume_pct: u32,
    until: usize,
) {
    for layer in layers {
        let voice = Voice::note(layer, note.0, gate, volume_pct);
        add_voice(out, start, &voice, layer, Some(until));
    }
}

/// Renders an effect at full volume.
pub fn render_sfx(sfx: &Sfx) -> Render {
    let mut mix = Vec::new();
    add_sfx(&mut mix, 0, sfx, 100);
    to_pcm(&mix)
}

/// Renders one note of a melodic instrument at full volume.
pub fn render_note(layers: &[Layer], note: Note, gate: u32) -> Render {
    let mut mix = Vec::new();
    add_note(&mut mix, 0, layers, note, gate, 100);
    to_pcm(&mix)
}
