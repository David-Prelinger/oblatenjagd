//! Review images of a rendered sound: the waveform (envelope view) and a
//! log-frequency spectrogram on the same time axis, plus the labelled
//! overview sheet of every spectrogram (ImageMagick `montage`).
//!
//! Reading them (docs/validation.md, "Audio"):
//! - time runs left to right at [`ms_per_column`] (1 ms per pixel for
//!   anything up to 1 s, so SFX are comparable side by side); dotted
//!   vertical lines every 50 ms (every second for long sounds);
//! - waveform: min/max per column (light) over the RMS per column (bright),
//!   the red lines are the −1 dBFS ceiling, the green ones the target
//!   peak band;
//! - spectrogram: 40 Hz (bottom) to 20.48 kHz (top), 24 rows per octave,
//!   dotted lines at 100 Hz, 1 kHz and 10 kHz; colour from black (−80 dB
//!   or quieter, relative to a full-scale sine) over blue and red to
//!   yellow-white (0 dB).

use std::path::{Path, PathBuf};
use std::process::Command;

use jnr_artkit::Image;
use jnr_audio::Pcm;

use super::stats::{FFT_SIZE, FULL_SCALE, dbfs_to_sample, spectrum};

/// Rows of the spectrogram per octave, and its octaves above [`SPEC_LOW_HZ`].
const ROWS_PER_OCTAVE: u32 = 24;
const OCTAVES: u32 = 9;
const SPEC_LOW_HZ: f64 = 40.0;
/// Height of the waveform view (odd: a centre row).
const WAVE_H: u32 = 129;
/// The spectrogram's floor (dB under a full-scale sine).
const SPEC_FLOOR_DB: f64 = -80.0;

const BG: [u8; 4] = [16, 16, 24, 255];
const GRID: [u8; 4] = [90, 90, 110, 255];
const CEILING: [u8; 4] = [220, 40, 40, 255];
const TARGET: [u8; 4] = [60, 200, 90, 255];
const WAVE_ENV: [u8; 4] = [70, 120, 190, 255];
const WAVE_RMS: [u8; 4] = [170, 210, 255, 255];

/// Time resolution: 1 ms per column up to 1 s, then as many ms as needed
/// to stay within about 1000 columns.
pub fn ms_per_column(duration_ms: f64) -> u32 {
    (duration_ms / 1000.0).ceil().max(1.0) as u32
}

fn columns(pcm: &Pcm, ms_col: u32) -> u32 {
    let ms = pcm.duration_ms() as f64 + 1.0;
    ((ms / ms_col as f64).ceil() as u32).max(1)
}

/// Sample range `[start, end)` of column `x`.
fn column_span(pcm: &Pcm, ms_col: u32, x: u32) -> (usize, usize) {
    let per = pcm.rate as f64 * ms_col as f64 / 1000.0;
    let start = (x as f64 * per) as usize;
    let end = (((x + 1) as f64 * per) as usize).min(pcm.samples.len());
    (start.min(end), end)
}

/// Dotted vertical grid every 50 ms (every 1000 ms when a column is more than 5 ms).
fn time_grid(img: &mut Image, ms_col: u32) {
    let every = if ms_col <= 5 { 50 } else { 1000 };
    for x in 0..img.width {
        let (t0, t1) = (x * ms_col, (x + 1) * ms_col);
        // A multiple of `every` falls inside this column.
        if x > 0 && t0.div_ceil(every) * every < t1 {
            for y in (0..img.height).step_by(3) {
                if img.get(x, y) == BG {
                    img.set(x, y, GRID);
                }
            }
        }
    }
}

/// The waveform view; `band` is the target peak band in dBFS (green lines).
pub fn waveform(pcm: &Pcm, band: Option<(f64, f64)>) -> Image {
    let ms_col = ms_per_column(pcm.duration_ms() as f64);
    let w = columns(pcm, ms_col);
    let mut img = Image::new(w, WAVE_H);
    img.fill(BG);
    let mid = (WAVE_H / 2) as i32;
    let half = mid as f64;
    let row =
        |v: f64| (mid - (v / FULL_SCALE * half).round() as i32).clamp(0, WAVE_H as i32 - 1) as u32;
    time_grid(&mut img, ms_col);
    for x in 0..w {
        img.set(x, mid as u32, GRID);
        let (a, b) = column_span(pcm, ms_col, x);
        let chunk = &pcm.samples[a..b];
        if chunk.is_empty() {
            continue;
        }
        let lo = *chunk.iter().min().unwrap() as f64;
        let hi = *chunk.iter().max().unwrap() as f64;
        for y in row(hi)..=row(lo) {
            img.set(x, y, WAVE_ENV);
        }
        let rms = (chunk.iter().map(|&s| (s as f64) * (s as f64)).sum::<f64>()
            / chunk.len() as f64)
            .sqrt();
        for y in row(rms)..=row(-rms) {
            img.set(x, y, WAVE_RMS);
        }
    }
    let mut hline = |db: f64, colour: [u8; 4], dotted: bool| {
        let v = dbfs_to_sample(db);
        for y in [row(v), row(-v)] {
            for x in (0..w).filter(|x| !dotted || x % 4 < 2) {
                img.set(x, y, colour);
            }
        }
    };
    if let Some((lo, hi)) = band {
        hline(lo, TARGET, true);
        hline(hi, TARGET, true);
    }
    hline(-1.0, CEILING, false);
    img
}

/// Frequency (Hz) at the centre of spectrogram row `r` (0 = bottom).
fn row_hz(r: f64) -> f64 {
    SPEC_LOW_HZ * 2f64.powf(r / ROWS_PER_OCTAVE as f64)
}

/// The log-frequency spectrogram.
pub fn spectrogram(pcm: &Pcm) -> Image {
    let ms_col = ms_per_column(pcm.duration_ms() as f64);
    let w = columns(pcm, ms_col);
    let h = ROWS_PER_OCTAVE * OCTAVES;
    let mut img = Image::new(w, h);
    img.fill(BG);
    let bin_hz = pcm.rate as f64 / FFT_SIZE as f64;
    for x in 0..w {
        let (a, b) = column_span(pcm, ms_col, x);
        let centre = (a + b) / 2;
        let mags = spectrum(&pcm.samples, centre as isize - FFT_SIZE as isize / 2);
        for r in 0..h {
            let lo = row_hz(r as f64 - 0.5) / bin_hz;
            let hi = row_hz(r as f64 + 0.5) / bin_hz;
            // The strongest bin in the row's band (or the nearest one).
            let (k0, k1) = (
                lo.round() as usize,
                (hi.round() as usize).max(lo.round() as usize),
            );
            let m = (k0..=k1)
                .filter_map(|k| mags.get(k))
                .fold(0.0f64, |a, &b| a.max(b));
            let db = if m > 0.0 {
                20.0 * m.log10()
            } else {
                SPEC_FLOOR_DB
            };
            let t = ((db - SPEC_FLOOR_DB) / -SPEC_FLOOR_DB).clamp(0.0, 1.0);
            if t > 0.0 {
                img.set(x, h - 1 - r, heat(t));
            }
        }
    }
    for hz in [100.0, 1000.0, 10_000.0] {
        let r = (ROWS_PER_OCTAVE as f64 * (hz / SPEC_LOW_HZ).log2()).round() as u32;
        for x in (0..w).step_by(3) {
            img.set(x, h - 1 - r, GRID);
        }
    }
    time_grid(&mut img, ms_col);
    img
}

/// Black → blue → red → yellow → white for `t` in 0..=1.
fn heat(t: f64) -> [u8; 4] {
    const STOPS: [[f64; 3]; 5] = [
        [16.0, 16.0, 24.0],
        [40.0, 40.0, 160.0],
        [200.0, 40.0, 90.0],
        [250.0, 180.0, 40.0],
        [255.0, 255.0, 230.0],
    ];
    let pos = t * (STOPS.len() - 1) as f64;
    let i = (pos.floor() as usize).min(STOPS.len() - 2);
    let f = pos - i as f64;
    let c = |k: usize| (STOPS[i][k] + (STOPS[i + 1][k] - STOPS[i][k]) * f).round() as u8;
    [c(0), c(1), c(2), 255]
}

/// One overview cell: the waveform above the spectrogram.
pub fn cell(wave: &Image, spec: &Image) -> Image {
    let mut img = Image::new(wave.width.max(spec.width), wave.height + 2 + spec.height);
    img.fill(BG);
    img.blit(wave, 0, 0);
    img.blit(spec, 0, wave.height + 2);
    img
}

/// The overview: every cell (already written as PNGs) with its id as the
/// label, four per row, left-aligned in equal tiles, at the same time scale.
pub fn sheet(cells: &[(String, PathBuf)], out: &Path) -> Result<(), String> {
    let mut cmd = Command::new("montage");
    cmd.args([
        "-background",
        "#202030",
        "-fill",
        "white",
        "-pointsize",
        "14",
    ]);
    for (label, path) in cells {
        cmd.arg("-label").arg(label).arg(path);
    }
    cmd.args(["-tile", "4x", "-gravity", "West", "-geometry", "+6+4"])
        .arg(out);
    let output = cmd
        .output()
        .map_err(|e| format!("cannot run `montage` (ImageMagick installed?): {e}"))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "`montage` failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(hz: f64, ms: usize) -> Pcm {
        let n = ms * 441 / 10;
        Pcm {
            rate: 44_100,
            samples: (0..n)
                .map(|i| {
                    (16_000.0 * (2.0 * std::f64::consts::PI * hz * i as f64 / 44_100.0).sin())
                        as i16
                })
                .collect(),
        }
    }

    #[test]
    fn time_scale_is_shared_by_both_views() {
        assert_eq!(ms_per_column(150.0), 1);
        assert_eq!(ms_per_column(1000.0), 1);
        assert_eq!(ms_per_column(1500.0), 2);
        assert_eq!(ms_per_column(6000.0), 6);
        let pcm = tone(1000.0, 150);
        let (w, s) = (waveform(&pcm, Some((-10.0, -8.0))), spectrogram(&pcm));
        assert_eq!(w.width, s.width);
        assert_eq!(w.width, 151);
        assert_eq!((w.height, s.height), (WAVE_H, 216));
    }

    #[test]
    fn a_tone_lights_its_row() {
        let pcm = tone(1000.0, 200);
        let s = spectrogram(&pcm);
        let r = (ROWS_PER_OCTAVE as f64 * (1000.0 / SPEC_LOW_HZ).log2()).round() as u32;
        let x = s.width / 2;
        let at = |row: u32| s.get(x, s.height - 1 - row);
        // The tone's row is bright, two octaves below it is background.
        assert!(at(r)[0] > 200, "{:?}", at(r));
        assert_eq!(at(r - 2 * ROWS_PER_OCTAVE), BG);
    }
}
