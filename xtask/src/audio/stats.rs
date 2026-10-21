//! Measurements of a rendered sound (f64 analysis lives here in xtask, never
//! in `jnr-audio`): level, DC offset, silent head and tail, spectral
//! centroid, plus the short-time spectrum that the spectrogram PNG draws.

use jnr_audio::Pcm;

/// Full scale of an `i16` sample (the synth clamps symmetrically).
pub const FULL_SCALE: f64 = 32_767.0;
/// "Audible" for the head and tail measurements: −40 dBFS (docs/proposals/
/// ma-audio-architecture.md, "onset_ms: first sample above −40 dBFS").
pub const AUDIBLE_DBFS: f64 = -40.0;
/// The dBFS reported for digital silence (instead of −∞).
pub const SILENCE_DBFS: f64 = -120.0;
/// FFT size for the centroid and the spectrogram (23 ms at 44.1 kHz).
pub const FFT_SIZE: usize = 1024;

/// What `audio render` writes to `<id>.json` and the lints read.
#[derive(Clone, Debug, PartialEq)]
pub struct Stats {
    pub frames: usize,
    pub duration_ms: f64,
    pub peak_dbfs: f64,
    pub rms_dbfs: f64,
    /// Samples the renderer had to clamp to ±32767.
    pub clamped_samples: u32,
    /// Mean sample in % of full scale (signed).
    pub dc_offset_pct: f64,
    /// Time before the first sample at or above [`AUDIBLE_DBFS`].
    pub onset_ms: f64,
    /// Time after the last sample at or above [`AUDIBLE_DBFS`].
    pub silent_tail_ms: f64,
    /// Magnitude-weighted mean frequency over the whole sound.
    pub spectral_centroid_hz: f64,
    /// The first and last sample (a cue must start and end at zero).
    pub first_sample: i16,
    pub last_sample: i16,
}

/// Amplitude (0..=1 of full scale) to dBFS, floored at [`SILENCE_DBFS`].
pub fn dbfs(amplitude: f64) -> f64 {
    if amplitude <= 0.0 {
        SILENCE_DBFS
    } else {
        (20.0 * amplitude.log10()).max(SILENCE_DBFS)
    }
}

/// dBFS to a sample magnitude.
pub fn dbfs_to_sample(db: f64) -> f64 {
    FULL_SCALE * 10f64.powf(db / 20.0)
}

pub fn samples_to_ms(n: usize, rate: u32) -> f64 {
    n as f64 * 1000.0 / rate.max(1) as f64
}

pub fn analyse(pcm: &Pcm, clamped: u32) -> Stats {
    let s = &pcm.samples;
    let n = s.len();
    let peak = s.iter().map(|&v| (v as f64).abs()).fold(0.0, f64::max);
    let (sum, sum_sq) = s.iter().fold((0.0, 0.0), |(a, b), &v| {
        let v = v as f64;
        (a + v, b + v * v)
    });
    let mean = if n == 0 { 0.0 } else { sum / n as f64 };
    let rms = if n == 0 {
        0.0
    } else {
        (sum_sq / n as f64).sqrt()
    };
    let audible = dbfs_to_sample(AUDIBLE_DBFS);
    let loud = |v: &i16| (*v as f64).abs() >= audible;
    let first_loud = s.iter().position(loud).unwrap_or(n);
    let tail = match s.iter().rposition(loud) {
        Some(last) => n - 1 - last,
        None => n,
    };
    Stats {
        frames: n,
        duration_ms: samples_to_ms(n, pcm.rate),
        peak_dbfs: dbfs(peak / FULL_SCALE),
        rms_dbfs: dbfs(rms / FULL_SCALE),
        clamped_samples: clamped,
        dc_offset_pct: mean / FULL_SCALE * 100.0,
        onset_ms: samples_to_ms(first_loud, pcm.rate),
        silent_tail_ms: samples_to_ms(tail, pcm.rate),
        spectral_centroid_hz: centroid(pcm),
        first_sample: s.first().copied().unwrap_or(0),
        last_sample: s.last().copied().unwrap_or(0),
    }
}

/// The stats as pretty JSON (the `<id>.json` file), with the golden
/// fingerprint and the source kind.
pub fn to_json(id: &str, kind: &str, pcm: &Pcm, st: &Stats) -> String {
    let round = |v: f64| (v * 100.0).round() / 100.0;
    let value = serde_json::json!({
        "id": id,
        "kind": kind,
        "pcm": super::golden::fingerprint(pcm),
        "frames": st.frames,
        "duration_ms": round(st.duration_ms),
        "peak_dbfs": round(st.peak_dbfs),
        "rms_dbfs": round(st.rms_dbfs),
        "clipped_samples": st.clamped_samples,
        "dc_offset_pct": (st.dc_offset_pct * 1000.0).round() / 1000.0,
        "onset_ms": round(st.onset_ms),
        "silent_tail_ms": round(st.silent_tail_ms),
        "spectral_centroid_hz": st.spectral_centroid_hz.round(),
        "first_sample": st.first_sample,
        "last_sample": st.last_sample,
    });
    serde_json::to_string_pretty(&value).expect("json") + "\n"
}

/// The spectral centroid: Σ f·|X(f)| / Σ |X(f)| over every Hann-windowed
/// [`FFT_SIZE`] frame (hop half a frame), so louder frames weigh more.
/// 0 for silence.
pub fn centroid(pcm: &Pcm) -> f64 {
    let hop = FFT_SIZE / 2;
    let bin_hz = pcm.rate as f64 / FFT_SIZE as f64;
    let (mut num, mut den) = (0.0, 0.0);
    let mut start = 0isize - hop as isize;
    while start < pcm.samples.len() as isize {
        let mags = spectrum(&pcm.samples, start);
        for (k, m) in mags.iter().enumerate().skip(1) {
            num += k as f64 * bin_hz * m;
            den += m;
        }
        start += hop as isize;
    }
    if den == 0.0 { 0.0 } else { num / den }
}

/// Magnitudes of bins `0..FFT_SIZE/2` of the Hann-windowed frame that
/// starts at sample `start` (samples outside the sound are zero),
/// normalised so a full-scale sine's peak bin is about 1.
pub fn spectrum(samples: &[i16], start: isize) -> Vec<f64> {
    let n = FFT_SIZE;
    let mut re = vec![0.0; n];
    let mut im = vec![0.0; n];
    for (i, r) in re.iter_mut().enumerate() {
        let at = start + i as isize;
        if at >= 0 && (at as usize) < samples.len() {
            let w = 0.5 - 0.5 * (2.0 * std::f64::consts::PI * i as f64 / n as f64).cos();
            *r = samples[at as usize] as f64 / FULL_SCALE * w;
        }
    }
    fft(&mut re, &mut im);
    // A Hann-windowed sine of amplitude 1 peaks at N/4.
    let scale = 4.0 / n as f64;
    (0..n / 2)
        .map(|k| (re[k] * re[k] + im[k] * im[k]).sqrt() * scale)
        .collect()
}

/// In-place iterative radix-2 FFT (`re.len()` a power of two).
fn fft(re: &mut [f64], im: &mut [f64]) {
    let n = re.len();
    debug_assert!(n.is_power_of_two() && im.len() == n);
    let mut j = 0;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j |= bit;
        if i < j {
            re.swap(i, j);
            im.swap(i, j);
        }
    }
    let mut len = 2;
    while len <= n {
        let ang = -2.0 * std::f64::consts::PI / len as f64;
        for start in (0..n).step_by(len) {
            for k in 0..len / 2 {
                let (wr, wi) = ((ang * k as f64).cos(), (ang * k as f64).sin());
                let (a, b) = (start + k, start + k + len / 2);
                let (xr, xi) = (re[b] * wr - im[b] * wi, re[b] * wi + im[b] * wr);
                re[b] = re[a] - xr;
                im[b] = im[a] - xi;
                re[a] += xr;
                im[a] += xi;
            }
        }
        len <<= 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(hz: f64, amp: f64, n: usize) -> Pcm {
        let samples = (0..n)
            .map(|i| {
                (amp * FULL_SCALE * (2.0 * std::f64::consts::PI * hz * i as f64 / 44_100.0).sin())
                    .round() as i16
            })
            .collect();
        Pcm {
            rate: 44_100,
            samples,
        }
    }

    #[test]
    fn sine_levels_and_centroid() {
        let st = analyse(&sine(1000.0, 0.5, 44_100), 0);
        assert_eq!(st.frames, 44_100);
        assert!((st.duration_ms - 1000.0).abs() < 1e-9);
        assert!((st.peak_dbfs - -6.02).abs() < 0.01, "{}", st.peak_dbfs);
        // RMS of a sine is 3.01 dB under its peak.
        assert!((st.rms_dbfs - -9.03).abs() < 0.01, "{}", st.rms_dbfs);
        assert!(st.dc_offset_pct.abs() < 0.01);
        assert!(st.onset_ms < 0.1);
        assert!(st.silent_tail_ms < 0.1);
        assert!(
            (st.spectral_centroid_hz - 1000.0).abs() < 60.0,
            "{}",
            st.spectral_centroid_hz
        );
        let high = analyse(&sine(5000.0, 0.5, 44_100), 0);
        assert!((high.spectral_centroid_hz - 5000.0).abs() < 150.0);
    }

    #[test]
    fn head_tail_and_silence() {
        let mut pcm = sine(440.0, 0.5, 4410);
        let mut samples = vec![0i16; 441]; // 10 ms of silence before
        samples.extend(&pcm.samples);
        samples.extend(vec![0i16; 882]); // 20 ms after
        pcm.samples = samples;
        let st = analyse(&pcm, 3);
        assert_eq!(st.clamped_samples, 3);
        assert!((st.onset_ms - 10.0).abs() < 0.1, "{}", st.onset_ms);
        assert!(
            (st.silent_tail_ms - 20.0).abs() < 0.1,
            "{}",
            st.silent_tail_ms
        );
        assert_eq!((st.first_sample, st.last_sample), (0, 0));

        let silent = analyse(
            &Pcm {
                rate: 44_100,
                samples: vec![0; 100],
            },
            0,
        );
        assert_eq!(silent.peak_dbfs, SILENCE_DBFS);
        assert_eq!(silent.spectral_centroid_hz, 0.0);
        assert_eq!(silent.onset_ms, silent.duration_ms);
    }

    #[test]
    fn dc_offset_is_signed_percent() {
        let pcm = Pcm {
            rate: 44_100,
            samples: vec![3277; 1000],
        };
        let st = analyse(&pcm, 0);
        assert!((st.dc_offset_pct - 10.0).abs() < 0.01);
    }
}
