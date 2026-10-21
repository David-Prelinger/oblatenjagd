//! The integer synth: one [`Layer`] rendered as one voice into an `i32` mix
//! buffer.
//!
//! - Phase: a `u32` accumulator (a full period = 2^32); the per-sample
//!   increment is recomputed every [`CONTROL`] samples from the pitch in
//!   cents (slide + vibrato + arpeggio + detune) through
//!   [`tables::scale_by_cents`], and kept below Nyquist.
//! - Oscillators give Q15 samples (±32767): pulse (zero-mean, peak-normalised
//!   for every duty), triangle, saw, sine (table), noise (15-bit LFSR, taps
//!   0 and 1, seed 1, one step per half period, so it steps at 2 × the pitch).
//!   Every shape except pulse and noise starts at 0, so onsets don't click.
//! - Envelope: linear segments in Q16 (peak 65 536), boundaries from
//!   [`crate::ms_to_samples`] of the cumulative times, so lengths are exact
//!   to the sample.
//! - Filters: one-pole low-pass `y += a·(x − y)` and a high-pass `x − lp(x)`,
//!   with `a = w / (1 + w)`, `w = 2π·fc/fs` (π as 355/113), state in Q16;
//!   a band-pass: a Chamberlin state-variable filter run twice per sample
//!   (`f = 2·sin(π·fc/2fs)` from the sine table, damping `1/Q`), its band
//!   output scaled by the damping (unity gain at the centre).
//! - Tremolo: the level times `1 − depth·(1 + sin)/2`, updated with the pitch.

use crate::patch::{Envelope, Layer, Pitch, Wave};
use crate::{SAMPLE_RATE, ms_to_samples, tables};

/// Envelope peak (Q16).
pub const PEAK: i64 = 1 << 16;
/// Samples between pitch updates (0.73 ms).
pub const CONTROL: u32 = 32;
/// Largest phase increment (just below Nyquist).
pub const MAX_INC: u64 = (1 << 31) - 1;
/// Full-scale oscillator amplitude (Q15).
pub const FULL: i32 = 32_767;

/// Phase increment per sample of `millihertz` (Q32 turns).
pub fn inc_of_mhz(millihertz: u64) -> u64 {
    (millihertz << 32) / (SAMPLE_RATE as u64 * 1000)
}

/// Phase increment of MIDI note `midi` (`a4` = 69 = 440 Hz).
pub fn inc_of_note(midi: u8) -> u64 {
    let a4 = inc_of_mhz(440_000);
    tables::scale_by_cents(a4, (midi as i32 - crate::note::A4 as i32) * 100)
}

/// Cents from `from_hz` to `to_hz`, rounded to the nearest cent.
pub fn cents_between(from_hz: u32, to_hz: u32) -> i32 {
    let base = (from_hz as u64) << 20;
    let target = (to_hz as u64) << 20;
    // Smallest c in [-12000, 12000] with base · 2^(c/1200) >= target.
    let (mut lo, mut hi) = (-12_000i32, 12_000i32);
    while lo < hi {
        let mid = lo + (hi - lo) / 2;
        if tables::scale_by_cents(base, mid) >= target {
            hi = mid;
        } else {
            lo = mid + 1;
        }
    }
    let above = tables::scale_by_cents(base, lo).abs_diff(target);
    let below = tables::scale_by_cents(base, lo - 1).abs_diff(target);
    if below < above { lo - 1 } else { lo }
}

/// One linear envelope segment: `len` samples from `from` towards `to`
/// (`to` itself is the first value of the next segment).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Segment {
    pub len: u32,
    pub from: i64,
    pub to: i64,
}

/// The envelope's segments after the delay. `gate` is how long an `Adsr`
/// note is held (samples), counted from the voice start **including** the
/// delay, so a note sounds `gate + release` samples from its onset; `Ahd`
/// ignores it. A release from level 0 (e.g. a gate of 0) has no segment.
pub fn segments(env: &Envelope, delay_ms: u32, gate: u32) -> Vec<Segment> {
    let seg = |len: u32, from: i64, to: i64| Segment { len, from, to };
    match *env {
        Envelope::Ahd {
            attack_ms,
            hold_ms,
            decay_ms,
        } => {
            // Cumulative boundaries, so the total is exact.
            let b0 = ms_to_samples(delay_ms);
            let b1 = ms_to_samples(delay_ms + attack_ms);
            let b2 = ms_to_samples(delay_ms + attack_ms + hold_ms);
            let b3 = ms_to_samples(delay_ms + attack_ms + hold_ms + decay_ms);
            vec![
                seg(b1 - b0, 0, PEAK),
                seg(b2 - b1, PEAK, PEAK),
                seg(b3 - b2, PEAK, 0),
            ]
        }
        Envelope::Adsr {
            attack_ms,
            decay_ms,
            sustain_pct,
            release_ms,
        } => {
            let a = ms_to_samples(attack_ms);
            let d = ms_to_samples(decay_ms);
            let r = ms_to_samples(release_ms);
            let sustain = PEAK * sustain_pct as i64 / 100;
            let mut out = Vec::new();
            let mut level = 0;
            let mut left = gate.saturating_sub(ms_to_samples(delay_ms));
            // Attack (cut short by an early release).
            let len = a.min(left);
            if len > 0 {
                let to = if a == 0 {
                    PEAK
                } else {
                    PEAK * len as i64 / a as i64
                };
                out.push(seg(len, 0, to));
                level = to;
                left -= len;
            } else if a == 0 && left > 0 {
                level = PEAK;
            }
            // Decay towards sustain.
            if left > 0 {
                let len = d.min(left);
                if len > 0 {
                    let to = PEAK + (sustain - PEAK) * len as i64 / d as i64;
                    out.push(seg(len, PEAK, to));
                    left -= len;
                    level = to;
                } else {
                    level = sustain;
                }
            }
            // Sustain while held.
            if left > 0 {
                out.push(seg(left, level, level));
            }
            if level > 0 {
                out.push(seg(r, level, 0));
            }
            out
        }
    }
}

/// Everything needed to render one layer as a voice.
pub struct Voice<'a> {
    pub layer: &'a Layer,
    /// Phase increment of the unmodulated start pitch.
    pub base_inc: u64,
    /// Total glide (cents), its length (samples; 0 = jump at once) and
    /// when it starts (samples after the delay).
    pub slide_cents: i32,
    pub slide_samples: u32,
    pub slide_start: u32,
    /// Constant pitch offset (cents), e.g. an instrument's detune.
    pub offset_cents: i32,
    /// Envelope segments after the delay (see [`segments`]).
    pub segments: Vec<Segment>,
    /// Extra volume on top of the layer's (percent), e.g. a channel volume.
    pub gain_pct: u32,
}

impl Voice<'_> {
    /// The voice of an `.sfx` layer (fixed pitch).
    pub fn sfx(layer: &Layer, gain_pct: u32) -> Voice<'_> {
        let segments = segments(&layer.envelope, layer.delay_ms, 0);
        let sound_len: u32 = segments.iter().map(|s| s.len).sum();
        let (base_inc, slide_cents) = match layer.pitch {
            Pitch::Fixed {
                freq_hz,
                slide_to_hz,
            } => (
                inc_of_mhz(freq_hz as u64 * 1000),
                slide_to_hz.map_or(0, |to| cents_between(freq_hz, to)),
            ),
            // A melodic layer played as an effect: a4.
            Pitch::Note { slide_cents, .. } => (inc_of_note(crate::note::A4), slide_cents),
        };
        let offset_cents = match layer.pitch {
            Pitch::Fixed { .. } => 0,
            Pitch::Note { detune_cents, .. } => detune_cents,
        };
        let slide_start = ms_to_samples(layer.slide_delay_ms);
        Voice {
            layer,
            base_inc,
            slide_cents,
            slide_samples: layer
                .slide_ms
                .map_or(sound_len.saturating_sub(slide_start), ms_to_samples),
            slide_start,
            offset_cents,
            segments,
            gain_pct,
        }
    }

    /// The voice of a melodic layer playing MIDI note `midi` for `gate` samples.
    pub fn note(layer: &Layer, midi: u8, gate: u32, gain_pct: u32) -> Voice<'_> {
        let (offset_cents, slide_cents) = match layer.pitch {
            Pitch::Note {
                detune_cents,
                slide_cents,
            } => (detune_cents, slide_cents),
            Pitch::Fixed { .. } => (0, 0),
        };
        Voice {
            layer,
            base_inc: inc_of_note(midi),
            slide_cents,
            slide_samples: layer.slide_ms.map_or(0, ms_to_samples),
            slide_start: ms_to_samples(layer.slide_delay_ms),
            offset_cents,
            segments: segments(&layer.envelope, layer.delay_ms, gate),
            gain_pct,
        }
    }

    /// Samples from the voice start to its end (delay included).
    pub fn len(&self) -> usize {
        ms_to_samples(self.layer.delay_ms) as usize
            + self.segments.iter().map(|s| s.len as usize).sum::<usize>()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Adds the voice into `out` from sample `start` on (grows `out`).
    pub fn add_into(&self, out: &mut Vec<i32>, start: usize) {
        let end = start + self.len();
        if out.len() < end {
            out.resize(end, 0);
        }
        let layer = self.layer;
        let mut pos = start + ms_to_samples(layer.delay_ms) as usize;

        let (pulse_edge, pulse_hi, pulse_lo) = match layer.wave {
            Wave::Pulse { duty_pct } => {
                let d = duty_pct as i32;
                let m = d.max(100 - d);
                (
                    (((duty_pct as u64) << 32) / 100) as u32,
                    FULL * (100 - d) / m,
                    -FULL * d / m,
                )
            }
            _ => (0, 0, 0),
        };
        let bp = layer.bandpass.map(|b| {
            let base = ((b.hz as u64) << 30) / SAMPLE_RATE as u64;
            (base, cents_between(b.hz, b.to_hz))
        });
        let sound_len: u32 = self.segments.iter().map(|s| s.len).sum();
        let trem_inc = ((layer.tremolo.rate_hz as u64) << 32) / SAMPLE_RATE as u64;
        let trem_depth = PEAK * layer.tremolo.depth_pct as i64 / 100;
        let vib_inc = ((layer.vibrato.rate_hz as u64) << 32) / SAMPLE_RATE as u64;
        let arp_every = ms_to_samples(layer.arp.every_ms).max(1);

        let vol_q16 = (layer.volume_pct as i64 * self.gain_pct as i64 * PEAK) / 10_000;
        let mut chain = Chain {
            level_q32: 0,
            step: 0,
            gain: vol_q16,
            lp: layer.lowpass_hz.map(|hz| (coefficient(hz), 0)),
            hp: layer.highpass_hz.map(|hz| (coefficient(hz), 0)),
            bp: layer.bandpass.map(|b| BandState {
                f: 0,
                damping: PEAK * 100 / b.q_pct as i64,
                low: 0,
                band: 0,
            }),
        };
        let mut phase: u32 = 0;
        let mut lfsr: u32 = 1;
        let mut noise = FULL;
        let mut t: u32 = 0; // samples since the delay

        for seg in &self.segments {
            chain.step = if seg.len == 0 {
                0
            } else {
                ((seg.to - seg.from) << 32) / seg.len as i64
            };
            chain.level_q32 = seg.from << 32;
            let mut left = seg.len;
            while left > 0 {
                // One block: up to the next control point or the segment end.
                let n = (CONTROL - t % CONTROL).min(left);
                if t.is_multiple_of(CONTROL) {
                    if let (Some((base, cents)), Some(b)) = (bp, chain.bp.as_mut()) {
                        let c = if sound_len == 0 {
                            cents
                        } else {
                            (cents as i64 * t as i64 / sound_len as i64) as i32
                        };
                        // phase = fc / (4 fs) turns: sin(π fc / 2fs).
                        let phase = tables::scale_by_cents(base, c).min(1 << 30) as u32;
                        b.f = 4 * tables::sine(phase) as i64;
                    }
                    if trem_depth > 0 {
                        let lfo = tables::sine((trem_inc * t as u64) as u32) as i64;
                        let trem = PEAK - (trem_depth * (FULL as i64 + lfo) / (2 * FULL as i64));
                        chain.gain = (vol_q16 * trem) >> 16;
                    }
                }
                let inc = self.inc_at(t - t % CONTROL, vib_inc, arp_every) as u32;
                let dst = &mut out[pos..pos + n as usize];
                // One loop per wave, so the shape is not matched per sample.
                match layer.wave {
                    Wave::Pulse { .. } => {
                        for d in dst {
                            let osc = if phase < pulse_edge {
                                pulse_hi
                            } else {
                                pulse_lo
                            };
                            phase = phase.wrapping_add(inc);
                            *d += chain.sample(osc);
                        }
                    }
                    Wave::Triangle => {
                        for d in dst {
                            let p = (phase >> 16) as i32;
                            let v = if p < 16_384 {
                                2 * p
                            } else if p < 49_152 {
                                65_536 - 2 * p
                            } else {
                                2 * p - 131_072
                            };
                            phase = phase.wrapping_add(inc);
                            *d += chain.sample(v.clamp(-FULL, FULL));
                        }
                    }
                    Wave::Saw => {
                        for d in dst {
                            let v = ((phase.wrapping_add(1 << 31)) >> 16) as i32 - 32_768;
                            phase = phase.wrapping_add(inc);
                            *d += chain.sample(v.max(-FULL));
                        }
                    }
                    Wave::Sine => {
                        for d in dst {
                            let v = tables::sine(phase);
                            phase = phase.wrapping_add(inc);
                            *d += chain.sample(v);
                        }
                    }
                    Wave::Noise => {
                        for d in dst {
                            let osc = noise;
                            let next = phase.wrapping_add(inc);
                            if (next ^ phase) & (1 << 31) != 0 {
                                let bit = (lfsr ^ (lfsr >> 1)) & 1;
                                lfsr = (lfsr >> 1) | (bit << 14);
                                noise = if lfsr & 1 == 1 { FULL } else { -FULL };
                            }
                            phase = next;
                            *d += chain.sample(osc);
                        }
                    }
                }
                pos += n as usize;
                t += n;
                left -= n;
            }
        }
    }

    /// Phase increment at `t` samples after the delay.
    fn inc_at(&self, t: u32, vib_inc: u64, arp_every: u32) -> u64 {
        let layer = self.layer;
        let slide = if t < self.slide_start {
            0
        } else if t - self.slide_start >= self.slide_samples {
            self.slide_cents
        } else {
            let ts = t - self.slide_start;
            (self.slide_cents as i64 * ts as i64 / self.slide_samples as i64) as i32
        };
        let v = layer.vibrato;
        let vib_delay = ms_to_samples(v.delay_ms);
        let vibrato = if (v.depth_cents == 0 && v.to_depth_cents == 0) || t < vib_delay {
            0
        } else {
            let tv = t - vib_delay;
            let ramp = ms_to_samples(v.ramp_ms);
            let depth = if tv >= ramp {
                v.to_depth_cents as i64
            } else {
                let (from, to) = (v.depth_cents as i64, v.to_depth_cents as i64);
                from + (to - from) * tv as i64 / ramp as i64
            };
            let lfo_phase = (vib_inc * tv as u64) as u32;
            (depth * tables::sine(lfo_phase) as i64 / FULL as i64) as i32
        };
        let arp = match layer.arp.semitones.len() {
            0 => 0,
            n => layer.arp.semitones[(t / arp_every) as usize % n] * 100,
        };
        let cents = self.offset_cents + slide + vibrato + arp;
        tables::scale_by_cents(self.base_inc, cents).clamp(1, MAX_INC)
    }
}

/// Band-pass state (Q16 `f` and damping; `low` and `band` carry 16 extra bits).
struct BandState {
    f: i64,
    damping: i64,
    low: i64,
    band: i64,
}

/// Everything after the oscillator: envelope, volume, tremolo, filters.
struct Chain {
    level_q32: i64,
    step: i64,
    /// Volume × tremolo (Q16), updated per control block.
    gain: i64,
    /// (coefficient, state with 16 extra bits)
    lp: Option<(i64, i64)>,
    hp: Option<(i64, i64)>,
    bp: Option<BandState>,
}

impl Chain {
    #[inline(always)]
    fn sample(&mut self, osc: i32) -> i32 {
        let level = self.level_q32 >> 32;
        self.level_q32 += self.step;
        let mut x = (((osc as i64 * level) >> 16) * self.gain) >> 16;
        if let Some((a, y)) = &mut self.lp {
            *y += (((x << 16) - *y) * *a) >> 16;
            x = *y >> 16;
        }
        if let Some((a, y)) = &mut self.hp {
            *y += (((x << 16) - *y) * *a) >> 16;
            x -= *y >> 16;
        }
        if let Some(b) = &mut self.bp {
            // Run twice per sample (2x oversampling keeps it stable up to 10 kHz).
            for _ in 0..2 {
                b.low += (b.f * b.band) >> 16;
                let high = (x << 16) - b.low - ((b.damping * b.band) >> 16);
                b.band += (b.f * high) >> 16;
            }
            x = ((b.band >> 16) * b.damping) >> 16;
        }
        x as i32
    }
}

/// One-pole coefficient (Q16) for cutoff `hz`: `w / (1 + w)`, `w = 2π·hz/fs`.
pub fn coefficient(hz: u32) -> i64 {
    let w = (2 * 355 * hz as i64 * PEAK) / (113 * SAMPLE_RATE as i64);
    (w * PEAK) / (PEAK + w)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn note_increments_match_equal_temperament() {
        // a4 = 440 Hz, a5 = 880 Hz, c4 = 261.63 Hz.
        let hz = |inc: u64| inc as f64 * SAMPLE_RATE as f64 / 4_294_967_296.0;
        assert!((hz(inc_of_note(69)) - 440.0).abs() < 0.001);
        assert!((hz(inc_of_note(81)) - 880.0).abs() < 0.001);
        assert!((hz(inc_of_note(60)) - 261.6256).abs() < 0.001);
    }

    #[test]
    fn cents_between_octaves_and_fifths() {
        assert_eq!(cents_between(330, 660), 1200);
        assert_eq!(cents_between(660, 330), -1200);
        assert_eq!(cents_between(440, 440), 0);
        assert_eq!(cents_between(200, 300), 702);
        assert_eq!(cents_between(20, 20_000), 11_959);
    }

    #[test]
    fn ahd_segments_are_exact_to_the_sample() {
        let env = Envelope::Ahd {
            attack_ms: 3,
            hold_ms: 7,
            decay_ms: 11,
        };
        let segs = segments(&env, 1, 0);
        // Boundaries ms_to_samples(1), (4), (11), (22) = 44, 176, 485, 970.
        let lens: Vec<u32> = segs.iter().map(|s| s.len).collect();
        assert_eq!(lens, [132, 309, 485]);
    }

    #[test]
    fn adsr_released_early_starts_its_release_from_the_reached_level() {
        let env = Envelope::Adsr {
            attack_ms: 10,
            decay_ms: 10,
            sustain_pct: 50,
            release_ms: 10,
        };
        // Held 5 ms: half the attack, then release from half the peak.
        let segs = segments(&env, 0, ms_to_samples(5));
        assert_eq!(
            segs,
            [
                Segment {
                    len: 220,
                    from: 0,
                    to: PEAK * 220 / 441
                },
                Segment {
                    len: 441,
                    from: PEAK * 220 / 441,
                    to: 0
                },
            ]
        );
        // Held 100 ms: attack, decay, sustain at 50 %, release.
        let segs = segments(&env, 0, ms_to_samples(100));
        let lens: Vec<u32> = segs.iter().map(|s| s.len).collect();
        assert_eq!(lens, [441, 441, 4410 - 882, 441]);
        assert_eq!(segs[2].from, PEAK / 2);
        assert_eq!(segs[3].from, PEAK / 2);
    }

    #[test]
    fn filter_coefficients_rise_with_the_cutoff() {
        let mut last = 0;
        for hz in [20, 100, 1000, 5000, 20_000] {
            let a = coefficient(hz);
            assert!(a > last && a < PEAK, "{hz}: {a}");
            last = a;
        }
    }
}
