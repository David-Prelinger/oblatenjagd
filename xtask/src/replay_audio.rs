//! `cargo xtask replay <file> --audio [out.wav]` (E60): the offline
//! soundtrack of a run. The replay runs headless, its cues come from
//! `jnr_audio::cues::Director` (as in the cue log), the sounds are rendered
//! from `audio/` and `jnr_audio::mix` plays the cues into one buffer the way
//! the app does (D-114: no master bus, so this is where the headroom is
//! proven). The analysis here uses f64 (xtask only).
//!
//! Outputs (default `$CARGO_TARGET_DIR/preview/replays/`): `<name>.wav`,
//! `<name>_audio.png` (cue labels, waveform and spectrogram over the run on
//! one time axis) and `<name>_audio.json` (mix stats, lint findings, the cue
//! timeline). The lints ([`lint`]: `clamp`, `ceiling`) name the sounds that
//! play where the mix is too loud. The `verify` step `audio-mix` mixes every
//! replay with a cue-log golden and runs the lints.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use jnr_artkit::Image;
use jnr_audio::bank::{Bank, Variant};
use jnr_audio::cues::{Cue, Director, Music, PENDING_MUSIC, Voice};
use jnr_audio::mix::{self, Mix, Sound, Sounds};
use jnr_audio::{FRAMES_PER_TICK, Pcm};
use jnr_core::Level;

use crate::audio::lint::CEILING_DBFS;
use crate::audio::plot;
use crate::audio::stats::{FULL_SCALE, dbfs, dbfs_to_sample, samples_to_ms};
use crate::audio_song::{rect, text};
use crate::content;

/// A replay's cues: (game frame, cue) in order, and its last frame.
pub struct Timeline {
    pub cues: Vec<(u32, Cue)>,
    pub end_frame: u32,
}

/// Runs the replay at `path` and feeds the cue director (frame 0 = `begin`).
pub fn timeline(root: &Path, path: &Path) -> Result<Timeline, String> {
    let replay = content::load_replay(path)?;
    let levels: Vec<Level> = content::all_levels(root)?
        .into_iter()
        .map(|(p, l)| l.map_err(|e| format!("{}: {e}", p.display())))
        .collect::<Result<_, _>>()?;
    let tuning = content::tuning(root)?;
    let mut game = replay
        .new_game(&levels, &tuning)
        .map_err(|e| format!("{}: {e}", path.display()))?;
    let mut director = Director::new();
    let mut cues: Vec<(u32, Cue)> = director
        .begin(&game)
        .into_iter()
        .map(|c| (game.frame(), c))
        .collect();
    for input in replay.frames() {
        game.step(input);
        let f = game.frame();
        cues.extend(director.step(&game, input).into_iter().map(|c| (f, c)));
    }
    Ok(Timeline {
        cues,
        end_frame: game.frame(),
    })
}

/// The sounds `cues` start (SFX variants, songs with their twins, jingles).
pub fn needed(cues: &[(u32, Cue)]) -> BTreeSet<Sound> {
    let mut out = BTreeSet::new();
    for &(_, c) in cues {
        match c {
            Cue::Sfx(p) => {
                out.insert(Sound::Sfx(p));
            }
            Cue::Song(m) => {
                out.insert(Sound::Music(m));
                if let Some(t) = m.underwater_twin() {
                    out.insert(Sound::Music(t));
                }
            }
            Cue::Jingle(m) => {
                out.insert(Sound::Music(m));
            }
            _ => {}
        }
    }
    out
}

/// The sources under `<root>/audio/` as (path from `root`, text), the way
/// `jnr-assets` embeds them, parsed and checked by `bank::Bank::load` (the
/// app's bank: one source of truth for parsing and pitched variants).
pub fn load_bank(root: &Path) -> Result<Bank, String> {
    let mut files = Vec::new();
    let mut stack = vec![root.join("audio")];
    while let Some(dir) = stack.pop() {
        let Ok(rd) = fs::read_dir(&dir) else { continue };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if p
                .extension()
                .is_some_and(|x| ["sfx", "inst", "song", "quote"].contains(&&*x.to_string_lossy()))
            {
                let name = p
                    .strip_prefix(root)
                    .unwrap_or(&p)
                    .to_string_lossy()
                    .replace('\\', "/");
                files.push((name, content::read(&p)?));
            }
        }
    }
    files.sort();
    let refs: Vec<(&str, &str)> = files
        .iter()
        .map(|(n, t)| (n.as_str(), t.as_str()))
        .collect();
    Bank::load(&refs).map_err(|e| e.join("\n"))
}

/// Renders the `needed` sounds with the bank's `render_sfx` (pitched
/// variants included) and `render_music`. A sound the bank doesn't have
/// stays out of the table (silent in the mix, see [`missing_lint`]).
pub fn sounds(bank: &Bank, needed: &BTreeSet<Sound>) -> Result<Sounds, String> {
    let mut out = Sounds::default();
    for &sound in needed {
        let pcm = match sound {
            Sound::Sfx(p) => bank.render_sfx(p),
            Sound::Music(m) => bank.render_music(m, Variant::Normal)?.map(|r| r.pcm),
        };
        if let Some(pcm) = pcm {
            out.sounds.insert(sound, pcm);
        }
    }
    Ok(out)
}

// ---------------------------------------------------------------- analysis

/// Numbers of a mix (`<name>_audio.json`).
#[derive(Clone, Debug, PartialEq)]
pub struct MixStats {
    pub frames: u32,
    pub duration_ms: f64,
    pub peak_dbfs: f64,
    /// The first sample at the peak.
    pub peak_sample: usize,
    pub rms_dbfs: f64,
    pub clamped_samples: u32,
    /// Samples whose magnitude is above the −1 dBFS ceiling (clamped ones too).
    pub samples_over_ceiling: usize,
    pub ms_over_ceiling: f64,
    /// RMS of every whole or partial second of the mix.
    pub rms_dbfs_per_second: Vec<f64>,
}

/// The ceiling as a sample magnitude.
fn ceiling() -> f64 {
    dbfs_to_sample(CEILING_DBFS)
}

pub fn analyse(m: &Mix, end_frame: u32) -> MixStats {
    let s = &m.render.pcm.samples;
    let rate = m.render.pcm.rate;
    let peak = s.iter().map(|&v| (v as f64).abs()).fold(0.0, f64::max);
    let peak_sample = s
        .iter()
        .position(|&v| (v as f64).abs() == peak)
        .unwrap_or(0);
    let rms = |chunk: &[i16]| {
        if chunk.is_empty() {
            return 0.0;
        }
        (chunk.iter().map(|&v| (v as f64) * (v as f64)).sum::<f64>() / chunk.len() as f64).sqrt()
    };
    let over = s.iter().filter(|&&v| (v as f64).abs() > ceiling()).count();
    MixStats {
        frames: end_frame + 1,
        duration_ms: samples_to_ms(s.len(), rate),
        peak_dbfs: dbfs(peak / FULL_SCALE),
        peak_sample,
        rms_dbfs: dbfs(rms(s) / FULL_SCALE),
        clamped_samples: m.render.clamped,
        samples_over_ceiling: over,
        ms_over_ceiling: samples_to_ms(over, rate),
        rms_dbfs_per_second: s
            .chunks(rate as usize)
            .map(|c| dbfs(rms(c) / FULL_SCALE))
            .collect(),
    }
}

/// The sounds heard somewhere in samples `[a, b)` (ids, sorted).
pub fn sounds_at(m: &Mix, (a, b): (usize, usize)) -> Vec<String> {
    let names: BTreeSet<String> = m
        .spans
        .iter()
        .filter(|sp| sp.start < b && sp.end > a)
        .map(|sp| sp.sound.to_string())
        .collect();
    names.into_iter().collect()
}

/// A mix lint finding.
#[derive(Clone, Debug, PartialEq)]
pub struct Finding {
    /// `clamp` or `ceiling`.
    pub lint: &'static str,
    pub message: String,
    /// The ids of the sounds playing where it happens (SFX, songs, jingles).
    pub culprits: Vec<String>,
}

/// Stretches of samples above the ceiling closer than this are one place.
const REGION_GAP: usize = FRAMES_PER_TICK as usize;
/// At most this many places are listed per lint.
const MAX_REGIONS: usize = 8;

/// The mix lints: `clamp` (no sample clamped) and `ceiling` (no sample above
/// −1 dBFS, audio-design §3), each naming the places (time, frame) and the
/// sounds that play there.
pub fn lint(m: &Mix) -> Vec<Finding> {
    let s = &m.render.pcm.samples;
    let limit = ceiling();
    // Places: [start, end) runs of over-ceiling samples, gaps < REGION_GAP merged.
    let mut regions: Vec<(usize, usize)> = Vec::new();
    let mut over = 0;
    for (i, &v) in s.iter().enumerate() {
        if (v as f64).abs() > limit {
            over += 1;
            match regions.last_mut() {
                Some(r) if i - r.1 < REGION_GAP => r.1 = i + 1,
                _ => regions.push((i, i + 1)),
            }
        }
    }
    if regions.is_empty() {
        return Vec::new();
    }
    let at = |r: (usize, usize)| sounds_at(m, r);
    let rate = m.render.pcm.rate;
    let describe = |r: (usize, usize)| {
        let peak = s[r.0..r.1]
            .iter()
            .map(|&v| (v as f64).abs())
            .fold(0.0, f64::max);
        format!(
            "{:.2} s (frame {}), {:.1} ms, peak {:.2} dBFS: {}",
            r.0 as f64 / rate as f64,
            r.0 / FRAMES_PER_TICK as usize,
            samples_to_ms(r.1 - r.0, rate),
            dbfs(peak / FULL_SCALE),
            at(r).join(" + ")
        )
    };
    let culprits = |rs: &[(usize, usize)]| {
        let all: BTreeSet<String> = rs.iter().flat_map(|&r| at(r)).collect();
        all.into_iter().collect::<Vec<_>>()
    };
    let listing = |rs: &[(usize, usize)]| {
        let mut t: String = rs
            .iter()
            .take(MAX_REGIONS)
            .map(|&r| format!("\n    at {}", describe(r)))
            .collect();
        if rs.len() > MAX_REGIONS {
            t.push_str(&format!("\n    … and {} more", rs.len() - MAX_REGIONS));
        }
        t
    };
    let mut out = Vec::new();
    if m.render.clamped > 0 {
        let clamped: Vec<(usize, usize)> = regions
            .iter()
            .copied()
            .filter(|&(a, b)| s[a..b].iter().any(|&v| v == i16::MAX || v == -i16::MAX))
            .collect();
        out.push(Finding {
            lint: "clamp",
            message: format!(
                "{} sample(s) clamped in {} place(s){}",
                m.render.clamped,
                clamped.len(),
                listing(&clamped)
            ),
            culprits: culprits(&clamped),
        });
    }
    out.push(Finding {
        lint: "ceiling",
        message: format!(
            "{over} sample(s) above {CEILING_DBFS} dBFS in {} place(s){}",
            regions.len(),
            listing(&regions)
        ),
        culprits: culprits(&regions),
    });
    out
}

/// The `missing` lint: every SFX must exist, and a music cue may be silent
/// only while it is in `cues::PENDING_MUSIC` (D-122; E58b's pending list).
pub fn missing_lint(m: &Mix) -> Option<Finding> {
    missing_lint_with(m, PENDING_MUSIC)
}

/// [`missing_lint`] against a given pending list (tests pass their own, as
/// `cues::PENDING_MUSIC` is empty since MA).
fn missing_lint_with(m: &Mix, pending: &[Music]) -> Option<Finding> {
    let bad: Vec<String> = m
        .missing
        .iter()
        .filter(|s| match s {
            Sound::Sfx(_) => true,
            Sound::Music(x) => !pending.contains(x),
        })
        .map(Sound::to_string)
        .collect();
    (!bad.is_empty()).then(|| Finding {
        lint: "missing",
        message: format!(
            "silent for lack of a source (only music in cues::PENDING_MUSIC may be): {}",
            bad.join(", ")
        ),
        culprits: bad,
    })
}

/// [`lint`] and [`missing_lint`].
pub fn findings(m: &Mix) -> Vec<Finding> {
    let mut f = lint(m);
    f.extend(missing_lint(m));
    f
}

/// Sample positions where a sound starts in `pcm`: a 64-sample block at or
/// above −40 dBFS whose peak is ≥ 12 dB over the loudest of the 8 blocks
/// before it (~12 ms), at least 20 ms after the previous onset. Used to
/// check the mix's timing against the cue timeline.
pub fn onsets(pcm: &Pcm) -> Vec<usize> {
    const BLOCK: usize = 64;
    const BEFORE: usize = 8;
    let env: Vec<f64> = pcm
        .samples
        .chunks(BLOCK)
        .map(|c| c.iter().map(|&v| (v as f64).abs()).fold(0.0, f64::max))
        .collect();
    let audible = dbfs_to_sample(crate::audio::stats::AUDIBLE_DBFS);
    let debounce = (pcm.rate as usize / 50).div_ceil(BLOCK);
    let mut out: Vec<usize> = Vec::new();
    let mut last: Option<usize> = None;
    for b in 0..env.len() {
        let before = env[b.saturating_sub(BEFORE)..b]
            .iter()
            .fold(0.0, |a: f64, &v| a.max(v));
        let rise = env[b] >= audible && env[b] >= 4.0 * before;
        if rise && last.is_none_or(|l| b >= l + debounce) {
            out.push(b * BLOCK);
            last = Some(b);
        }
    }
    out
}

// ----------------------------------------------------------------- picture

const BG: [u8; 4] = [16, 16, 24, 255];
const AXIS: [u8; 4] = [150, 150, 170, 255];
const DIM: [u8; 4] = [90, 90, 110, 255];
const LABEL_ROWS: u32 = 8;
const ROW_H: u32 = 7;
/// A label repeats only this many columns after the same one.
const REPEAT_LABEL_PX: u32 = 40;

/// Colour of a cue's marker: SFX by voice, music green, the rest grey.
fn colour(c: &Cue) -> [u8; 4] {
    match c {
        Cue::Sfx(p) => match p.sfx.voice() {
            Voice::P => [250, 150, 60, 255],
            Voice::W => [90, 210, 230, 255],
            Voice::I => [240, 220, 80, 255],
            Voice::A => [200, 120, 220, 255],
        },
        Cue::Song(_) | Cue::Jingle(_) => [90, 220, 110, 255],
        _ => DIM,
    }
}

/// A cue's label (SFX and music only; the rest are unlabelled ticks).
fn label(c: &Cue) -> Option<String> {
    match c {
        Cue::Sfx(p) if p.semitones != 0 => Some(format!("{}{:+}", p.sfx.id(), p.semitones)),
        Cue::Sfx(p) => Some(p.sfx.id().to_owned()),
        Cue::Song(m) | Cue::Jingle(m) => Some(m.id().to_owned()),
        _ => None,
    }
}

/// The review image (1x; written 2x): cue labels in rows (a label that
/// finds no free row is left out, its tick stays), a tick strip, the
/// waveform (red: −1 dBFS ceiling) and the spectrogram with every cue as a
/// dotted line, and a time axis in seconds. Silent cues (no source yet)
/// are labelled in grey.
pub fn picture(m: &Mix, cues: &[(u32, Cue)]) -> Image {
    let pcm = &m.render.pcm;
    let wave = plot::waveform(pcm, None);
    let spec = plot::spectrogram(pcm);
    let w = wave.width;
    let ms_col = plot::ms_per_column(pcm.duration_ms() as f64) as f64;
    let x_of = |frame: u32| {
        ((frame as f64 * FRAMES_PER_TICK as f64 * 1000.0 / pcm.rate as f64) / ms_col) as u32
    };
    let labels_h = LABEL_ROWS * ROW_H;
    let ticks_y = labels_h + 1;
    let wave_y = ticks_y + 6;
    let spec_y = wave_y + wave.height + 2;
    let axis_y = spec_y + spec.height + 1;
    let h = axis_y + 10;
    let mut img = Image::new(w, h);
    img.fill(BG);
    img.blit(&wave, 0, wave_y);
    img.blit(&spec, 0, spec_y);
    let missing = |c: &Cue| match c {
        Cue::Sfx(p) => m.missing.contains(&Sound::Sfx(*p)),
        Cue::Song(x) | Cue::Jingle(x) => m.missing.contains(&Sound::Music(*x)),
        _ => false,
    };
    let mut row_free = [0u32; LABEL_ROWS as usize];
    let mut last_label: BTreeMap<String, u32> = BTreeMap::new();
    for (frame, c) in cues {
        let x = x_of(*frame).min(w - 1);
        let col = if missing(c) { DIM } else { colour(c) };
        rect(&mut img, x, ticks_y, 1, 5, col);
        if !matches!(c, Cue::Sfx(_) | Cue::Song(_) | Cue::Jingle(_)) {
            continue;
        }
        for y in (wave_y..axis_y).step_by(4) {
            img.set(x, y, col);
        }
        if let Some(l) = label(c) {
            // A run of the same sound (tally ticks, an Oblaten row) is labelled once.
            if last_label
                .get(&l)
                .is_some_and(|&lx| x < lx + REPEAT_LABEL_PX)
            {
                continue;
            }
            last_label.insert(l.clone(), x);
            let lw = 4 * l.len() as u32;
            if let Some(r) = (0..LABEL_ROWS as usize).find(|&r| row_free[r] <= x) {
                let y = r as u32 * ROW_H + 1;
                rect(&mut img, x, y, 1, ROW_H - 1, col);
                text(&mut img, x + 2, y, &l, col);
                row_free[r] = x + 2 + lw + 2;
            }
        }
    }
    // Time axis: a tick every second, a label every `step` seconds.
    let px_per_s = 1000.0 / ms_col;
    let step = [1u32, 2, 5, 10, 20, 30, 60]
        .into_iter()
        .find(|&s| s as f64 * px_per_s >= 24.0)
        .unwrap_or(60);
    let secs = (pcm.duration_ms() / 1000) as u32;
    for s in 0..=secs {
        let x = (s as f64 * px_per_s) as u32;
        if x >= w {
            break;
        }
        rect(&mut img, x, axis_y, 1, 2, AXIS);
        if s % step == 0 {
            text(&mut img, x + 1, axis_y + 3, &format!("{s}s"), AXIS);
        }
    }
    img.scaled(2)
}

// ------------------------------------------------------------------ output

/// The run's sounds that have no source yet (silent), as ids.
fn missing_ids(m: &Mix) -> Vec<String> {
    m.missing.iter().map(Sound::to_string).collect()
}

pub fn to_json(
    name: &str,
    m: &Mix,
    st: &MixStats,
    findings: &[Finding],
    cues: &[(u32, Cue)],
) -> String {
    let r2 = |v: f64| (v * 100.0).round() / 100.0;
    let ms = |f: u32| f as u64 * 1000 / 60;
    let value = serde_json::json!({
        "replay": name,
        "frames": st.frames,
        "duration_ms": r2(st.duration_ms),
        "run_ms": r2(samples_to_ms(m.run_samples, m.render.pcm.rate)),
        "peak_dbfs": r2(st.peak_dbfs),
        "peak_ms": r2(samples_to_ms(st.peak_sample, m.render.pcm.rate)),
        "peak_frame": st.peak_sample / FRAMES_PER_TICK as usize,
        "peak_sounds": sounds_at(m, (st.peak_sample, st.peak_sample + 1)),
        "rms_dbfs": r2(st.rms_dbfs),
        "clamped_samples": st.clamped_samples,
        "ceiling_dbfs": CEILING_DBFS,
        "samples_over_ceiling": st.samples_over_ceiling,
        "ms_over_ceiling": r2(st.ms_over_ceiling),
        "rms_dbfs_per_second": st.rms_dbfs_per_second.iter().map(|&v| r2(v)).collect::<Vec<_>>(),
        "silent_no_source": missing_ids(m),
        "onsets_ms": onsets(&m.render.pcm)
            .into_iter()
            .map(|i| r2(samples_to_ms(i, m.render.pcm.rate)))
            .collect::<Vec<_>>(),
        "findings": findings.iter().map(|f| serde_json::json!({
            "lint": f.lint,
            "message": f.message,
            "sounds": f.culprits,
        })).collect::<Vec<_>>(),
        "cues": cues.iter().map(|(f, c)| serde_json::json!({
            "frame": f,
            "ms": ms(*f),
            "sample": *f as u64 * FRAMES_PER_TICK as u64,
            "cue": c.to_string(),
        })).collect::<Vec<_>>(),
    });
    let mut s = serde_json::to_string_pretty(&value).expect("json");
    s.push('\n');
    s
}

/// One mixed replay.
pub struct Mixed {
    pub name: String,
    pub timeline: Timeline,
    pub mix: Mix,
    pub stats: MixStats,
    pub findings: Vec<Finding>,
}

/// Mixes, measures and lints one replay's timeline.
fn mix_timeline(name: String, timeline: Timeline, sounds: &Sounds) -> Mixed {
    let mix = mix::mix(sounds, &timeline.cues, timeline.end_frame);
    let stats = analyse(&mix, timeline.end_frame);
    let findings = findings(&mix);
    Mixed {
        name,
        timeline,
        mix,
        stats,
        findings,
    }
}

/// Runs, mixes, measures and lints the replay at `path`.
pub fn mix_replay(root: &Path, path: &Path) -> Result<Mixed, String> {
    let timeline = timeline(root, path)?;
    let bank = load_bank(root)?;
    let sounds = sounds(&bank, &needed(&timeline.cues))?;
    Ok(mix_timeline(content::replay_name(path), timeline, &sounds))
}

/// A short report: the stats, the silent sounds and the findings.
pub fn summary(x: &Mixed) -> String {
    let st = &x.stats;
    let mut t = format!(
        "{}: {} frames, {:.1} s mixed; peak {:.2} dBFS, RMS {:.2} dBFS, {} clamped, \
         {} sample(s) ({:.2} ms) above {CEILING_DBFS} dBFS\n",
        x.name,
        st.frames,
        st.duration_ms / 1000.0,
        st.peak_dbfs,
        st.rms_dbfs,
        st.clamped_samples,
        st.samples_over_ceiling,
        st.ms_over_ceiling,
    );
    t.push_str(&format!(
        "  peak at {:.2} s (frame {}): {}\n",
        samples_to_ms(st.peak_sample, x.mix.render.pcm.rate) / 1000.0,
        st.peak_sample / FRAMES_PER_TICK as usize,
        sounds_at(&x.mix, (st.peak_sample, st.peak_sample + 1)).join(" + ")
    ));
    let missing = missing_ids(&x.mix);
    if !missing.is_empty() {
        t.push_str(&format!(
            "  silent (no source in audio/ yet): {}\n",
            missing.join(", ")
        ));
    }
    for f in &x.findings {
        t.push_str(&format!("  error[{}]: {}\n", f.lint, f.message));
    }
    t
}

/// `cargo xtask replay <file> --audio [out.wav]`: writes the WAV, the
/// picture and the JSON; `Err` when a lint fails (after writing them).
pub fn run(root: &Path, path: &Path, out: Option<&Path>) -> Result<String, String> {
    let x = mix_replay(root, path)?;
    let wav: PathBuf = match out {
        Some(p) => p.to_path_buf(),
        None => crate::replay::preview_dir(root, "replays")?.join(format!("{}.wav", x.name)),
    };
    let dir = wav.parent().unwrap_or(Path::new("."));
    fs::create_dir_all(dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
    let stem = wav
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| x.name.clone());
    let png = dir.join(format!("{stem}_audio.png"));
    let json = dir.join(format!("{stem}_audio.json"));
    let write =
        |p: &Path, b: &[u8]| fs::write(p, b).map_err(|e| format!("write {}: {e}", p.display()));
    write(&wav, &jnr_audio::wav::write(&x.mix.render.pcm))?;
    write(&png, &picture(&x.mix, &x.timeline.cues).encode_png())?;
    write(
        &json,
        to_json(&x.name, &x.mix, &x.stats, &x.findings, &x.timeline.cues).as_bytes(),
    )?;
    let mut text = summary(&x);
    for p in [&wav, &png, &json] {
        text.push_str(&format!("  -> {}\n", p.display()));
    }
    if x.findings.is_empty() {
        Ok(text)
    } else {
        Err(text)
    }
}

/// The `audio-mix` verify step: every replay with a cue-log golden is
/// mixed and linted (no files written).
pub fn check_report(root: &Path) -> (bool, String) {
    let names = match content::files_with_ext(&crate::cues::cues_dir(root), "cues") {
        Ok(p) => p
            .iter()
            .map(|p| content::replay_name(p))
            .collect::<Vec<_>>(),
        Err(e) => return (false, format!("error: {e}\n")),
    };
    if names.is_empty() {
        return (
            false,
            "FAIL: no replays to mix (no cue-log goldens)\n".into(),
        );
    }
    // Every timeline first, then each needed sound rendered once.
    let timelines = crate::par::map_ordered(&names, crate::par::threads(), |name| {
        content::resolve_replay(root, name).and_then(|p| timeline(root, &p))
    });
    let all: BTreeSet<Sound> = timelines
        .iter()
        .flatten()
        .flat_map(|t| needed(&t.cues))
        .collect();
    let sounds = match load_bank(root).and_then(|b| sounds(&b, &all)) {
        Ok(s) => s,
        Err(e) => return (false, format!("error: {e}\n")),
    };
    let jobs: Vec<(String, Result<Timeline, String>)> =
        names.iter().cloned().zip(timelines).collect();
    let results = crate::par::map_ordered(&jobs, crate::par::threads(), |(name, t)| match t {
        Ok(t) => Ok(mix_timeline(
            name.clone(),
            Timeline {
                cues: t.cues.clone(),
                end_frame: t.end_frame,
            },
            &sounds,
        )),
        Err(e) => Err(e.clone()),
    });
    let mut text = String::new();
    let mut failed = 0;
    let mut silent: BTreeSet<String> = BTreeSet::new();
    for (name, r) in names.iter().zip(results) {
        match r {
            Err(e) => {
                failed += 1;
                text.push_str(&format!("FAIL {name}: {e}\n"));
            }
            Ok(x) => {
                silent.extend(missing_ids(&x.mix));
                let st = &x.stats;
                let head = if x.findings.is_empty() {
                    "ok  "
                } else {
                    "FAIL"
                };
                failed += usize::from(!x.findings.is_empty());
                text.push_str(&format!(
                    "{head} {name}: peak {:.2} dBFS, {} clamped, {:.2} ms above {CEILING_DBFS} dBFS\n",
                    st.peak_dbfs, st.clamped_samples, st.ms_over_ceiling
                ));
                for f in &x.findings {
                    text.push_str(&format!("  error[{}]: {}\n", f.lint, f.message));
                }
            }
        }
    }
    if !silent.is_empty() {
        let list: Vec<String> = silent.into_iter().collect();
        text.push_str(&format!(
            "note: silent (no source in audio/ yet; pending songs are allowed): {}\n",
            list.join(", ")
        ));
    }
    text.push_str(&format!(
        "audio mix: {} replay(s), {failed} failed\n",
        names.len()
    ));
    (failed == 0, text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use jnr_audio::SAMPLE_RATE;
    use jnr_audio::cues::{Music, Sfx};

    fn root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("..")
    }

    const T: usize = FRAMES_PER_TICK as usize;

    /// E60 criterion: on `testbed_route` every sound the WAV starts is a
    /// cue of the timeline, within one frame (735 samples), and every
    /// sounding cue starts a sound. Since the music landed (MA) the run ends
    /// with the `fanfare_clear` jingle, whose own notes are onsets too: up to
    /// the jingle the onsets are exactly the 7 jumps, and the jingle's first
    /// onset is on its cue.
    #[test]
    fn testbed_route_onsets_match_the_cue_timeline() {
        let root = root();
        let path = content::resolve_replay(&root, "testbed_route").unwrap();
        let x = mix_replay(&root, &path).unwrap();
        assert!(x.mix.missing.is_empty(), "{:?}", x.mix.missing);
        let at = |pick: fn(&Cue) -> bool| -> Vec<usize> {
            x.timeline
                .cues
                .iter()
                .filter(|(_, c)| pick(c))
                .map(|(f, _)| *f as usize * T)
                .collect()
        };
        let sfx = at(|c| matches!(c, Cue::Sfx(_)));
        let music = at(|c| matches!(c, Cue::Song(_) | Cue::Jingle(_)));
        assert_eq!(sfx.len(), 7, "the 7 jumps: {:?}", x.timeline.cues);
        assert_eq!(music.len(), 1, "the clear jingle: {:?}", x.timeline.cues);
        let jingle = music[0];
        assert_eq!(jingle, 731 * T);
        let found = onsets(&x.mix.render.pcm);
        let (before, after): (Vec<usize>, Vec<usize>) =
            found.iter().partition(|&&f| f + T < jingle);
        assert_eq!(before.len(), sfx.len(), "{before:?} vs {sfx:?}");
        for (f, e) in before.iter().zip(&sfx) {
            assert!(f.abs_diff(*e) <= T, "onset at {f}, cue at {e}");
        }
        assert!(
            after.first().is_some_and(|f| f.abs_diff(jingle) <= T),
            "the jingle starts on its cue: {after:?} vs {jingle}"
        );
        assert!(x.findings.is_empty(), "{:?}", x.findings);
        assert_eq!(x.stats.clamped_samples, 0);
        assert_eq!(x.timeline.end_frame, 758);
    }

    fn table(entries: &[(Sfx, i16)]) -> Sounds {
        Sounds {
            sounds: entries
                .iter()
                .map(|&(s, v)| {
                    let pcm = Pcm {
                        rate: SAMPLE_RATE,
                        samples: vec![v; 2 * T],
                    };
                    (Sound::Sfx(s.play()), pcm)
                })
                .collect(),
        }
    }

    fn mixed(b: &Sounds) -> Mix {
        let cues = [
            (1, Cue::Sfx(Sfx::Jump.play())),
            (1, Cue::Sfx(Sfx::Oblate.play())),
        ];
        mix::mix(b, &cues, 4)
    }

    #[test]
    fn a_clipping_mix_is_detected_and_names_the_sounds() {
        let m = mixed(&table(&[(Sfx::Jump, 20_000), (Sfx::Oblate, 20_000)]));
        let f = lint(&m);
        assert_eq!(
            f.iter().map(|f| f.lint).collect::<Vec<_>>(),
            ["clamp", "ceiling"]
        );
        for finding in &f {
            assert_eq!(finding.culprits, ["jump", "oblate"]);
            assert!(finding.message.contains("frame 1"), "{}", finding.message);
        }
        assert!(
            f[0].message
                .starts_with(&format!("{} sample(s) clamped", 2 * T))
        );
        let st = analyse(&m, 4);
        assert_eq!(st.clamped_samples, 2 * T as u32);
        assert_eq!(st.samples_over_ceiling, 2 * T);
        assert!(st.peak_dbfs > -0.01);
    }

    #[test]
    fn a_mix_over_the_ceiling_without_clamping_is_detected() {
        // 2 × 15 000 = 30 000 > 29 204 (−1 dBFS), < 32 767.
        let m = mixed(&table(&[(Sfx::Jump, 15_000), (Sfx::Oblate, 15_000)]));
        let f = lint(&m);
        assert_eq!(f.len(), 1, "{f:?}");
        assert_eq!(f[0].lint, "ceiling");
        assert_eq!(f[0].culprits, ["jump", "oblate"]);
        let st = analyse(&m, 4);
        assert_eq!(st.clamped_samples, 0);
        assert!((st.ms_over_ceiling - 2.0 * 1000.0 / 60.0).abs() < 0.01);
        // Under the ceiling: no finding.
        let m = mixed(&table(&[(Sfx::Jump, 10_000), (Sfx::Oblate, 10_000)]));
        assert!(lint(&m).is_empty());
        // One second of mix per RMS value.
        assert_eq!(analyse(&m, 4).rms_dbfs_per_second.len(), 1);
    }

    #[test]
    fn audio_run_writes_the_wav_the_picture_and_the_json() {
        let root = root();
        let dir = crate::target_dir(&root).join("tmp/replay-audio-test");
        let _ = fs::remove_dir_all(&dir);
        let path = content::resolve_replay(&root, "testbed_route").unwrap();
        let wav = dir.join("tb.wav");
        let text = run(&root, &path, Some(&wav)).unwrap();
        assert!(!text.contains("silent"), "every cue has a source: {text}");
        assert!(text.contains(": fanfare_clear"), "{text}");
        // The run is 759 frames; the clear jingle (cued on frame 731) is a
        // one-shot and rings out past the end.
        let bank = load_bank(&root).unwrap();
        let fanfare = bank
            .render_music(Music::FanfareClear, Variant::Normal)
            .unwrap()
            .unwrap()
            .pcm;
        let pcm = jnr_audio::wav::read(&fs::read(&wav).unwrap()).unwrap();
        assert_eq!(
            pcm.samples.len(),
            (759 * T).max(731 * T + fanfare.samples.len())
        );
        assert!(dir.join("tb_audio.png").exists());
        let json: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(dir.join("tb_audio.json")).unwrap()).unwrap();
        assert_eq!(json["clamped_samples"], 0);
        assert_eq!(json["cues"].as_array().unwrap().len(), 9);
        assert_eq!(json["cues"][0]["cue"], "sfx jump P4");
        assert_eq!(json["cues"][0]["sample"], 111 * T as u64);
        // The 7 jumps, then the jingle's notes from its cue (12 183 ms) on.
        let onsets: Vec<f64> = json["onsets_ms"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_f64().unwrap())
            .collect();
        let jingle_ms = 731.0 * 1000.0 / 60.0;
        let jumps = onsets.iter().filter(|&&ms| ms < jingle_ms - 17.0).count();
        assert_eq!(jumps, 7, "{onsets:?}");
        assert!(onsets.len() > 7, "the jingle sounds: {onsets:?}");
    }

    /// The pitched variants of `cues::PITCHED` are rendered, not missing.
    #[test]
    fn pitched_variants_render_from_their_patch() {
        let root = root();
        let needed: BTreeSet<Sound> = jnr_audio::cues::PITCHED
            .iter()
            .map(|&p| Sound::Sfx(p))
            .collect();
        let bank = load_bank(&root).unwrap();
        let b = sounds(&bank, &needed).unwrap();
        assert_eq!(b.sounds.len(), needed.len());
        let plain = sounds(&bank, &[Sound::Sfx(Sfx::Jump.play())].into()).unwrap();
        let low = &b.sounds[&Sound::Sfx(Sfx::Jump.pitched(-5))];
        let jump = &plain.sounds[&Sound::Sfx(Sfx::Jump.play())];
        assert_eq!(low.samples.len(), jump.samples.len());
        assert_ne!(low.samples, jump.samples);
    }

    /// D-122: an SFX without a source fails the gate, a pending song doesn't,
    /// a song that left the pending list does.
    #[test]
    fn missing_sounds_fail_unless_the_music_is_pending() {
        let empty = Sounds::default();
        let song = |m: Music| mix::mix(&empty, &[(0, Cue::Song(m))], 1);
        let sfx = mix::mix(&empty, &[(0, Cue::Sfx(Sfx::Jump.play()))], 1);
        let f = missing_lint(&sfx).expect("a missing SFX fails");
        assert_eq!(
            (f.lint, f.culprits.clone()),
            ("missing", vec!["jump".to_owned()])
        );
        assert_eq!(findings(&sfx), [f]);
        // The real list is empty (MA): every song without a source fails.
        for &m in Music::ALL.iter().filter(|m| m.is_song()) {
            assert!(!song(m).missing.is_empty());
            assert!(
                missing_lint(&song(m)).is_some(),
                "{m:?} has no pending excuse"
            );
        }
        // A pending song is excused, one that left the list is not, and a
        // pending list never excuses an SFX.
        let pending = [Music::Salzburg];
        assert!(missing_lint_with(&song(Music::Salzburg), &pending).is_none());
        assert!(missing_lint_with(&song(Music::StGilgen), &pending).is_some());
        assert!(missing_lint_with(&sfx, &pending).is_some());
    }

    /// A throwaway workspace whose `audio/music/` holds E57's placeholder
    /// polka as `salzburg` and its fanfare as `fanfare_clear`.
    fn placeholder_music_root(name: &str) -> PathBuf {
        let tmp = crate::target_dir(&root()).join("tmp").join(name);
        let _ = fs::remove_dir_all(&tmp);
        let audio = tmp.join("audio");
        for d in ["music", "instruments", "quotes"] {
            fs::create_dir_all(audio.join(d)).unwrap();
        }
        let fixtures = root().join("crates/jnr-audio/tests/fixtures/music");
        for e in fs::read_dir(&fixtures).unwrap().flatten() {
            let p = e.path();
            let name = p.file_name().unwrap().to_string_lossy().into_owned();
            let (to, text) = match name.as_str() {
                "placeholder_polka.song" => {
                    ("salzburg.song", Some(("placeholder_polka", "salzburg")))
                }
                "placeholder_fanfare.song" => (
                    "fanfare_clear.song",
                    Some(("placeholder_fanfare", "fanfare_clear")),
                ),
                n => (n, None),
            };
            let mut body = fs::read_to_string(&p).unwrap();
            if let Some((a, b)) = text {
                body = body.replace(&format!("id = \"{a}\""), &format!("id = \"{b}\""));
            }
            let dir = match p.extension().unwrap().to_string_lossy().as_ref() {
                "inst" => "instruments",
                "quote" => "quotes",
                _ => "music",
            };
            fs::write(audio.join(dir).join(to), body).unwrap();
        }
        tmp
    }

    /// Songs loop from their cue and jingles play once: the Stage 1 tour
    /// with placeholder music has the song from its `song salzburg` frame on
    /// and the fanfare at the clear.
    #[test]
    fn songs_and_jingles_from_audio_music_are_mixed() {
        let root = root();
        let path = content::resolve_replay(&root, "salzburg_tour").unwrap();
        let tl = timeline(&root, &path).unwrap();
        let music_root = placeholder_music_root("replay-audio-music");
        let b = sounds(&load_bank(&music_root).unwrap(), &needed(&tl.cues)).unwrap();
        assert!(b.sounds.contains_key(&Sound::Music(Music::Salzburg)));
        assert!(b.sounds.contains_key(&Sound::Music(Music::FanfareClear)));
        let m = mix::mix(&b, &tl.cues, tl.end_frame);
        let missing: Vec<String> = m.missing.iter().map(Sound::to_string).collect();
        // The SFX come from the throwaway root's (empty) audio/sfx.
        assert!(missing.contains(&"title".to_owned()) && missing.contains(&"jump".to_owned()));
        let frame_of = |cue: &str| {
            tl.cues
                .iter()
                .find(|(_, c)| c.to_string() == cue)
                .map(|(f, _)| *f as usize)
                .unwrap()
        };
        let (song, clear) = (frame_of("song salzburg"), frame_of("jingle fanfare_clear"));
        let s = &m.render.pcm.samples;
        let loud = |a: usize, b: usize| s[a..b].iter().any(|&v| v.unsigned_abs() > 300);
        assert!(!loud(0, song * T), "silence before the song");
        assert!(
            loud(song * T, (song + 30) * T),
            "the song starts on its cue"
        );
        let song_len = b.sounds[&Sound::Music(Music::Salzburg)].samples.len();
        assert!(song_len < (clear - song) * T, "the run outlasts one loop");
        assert!(
            loud((song * T) + song_len, (song * T) + song_len + 30 * T),
            "it loops"
        );
        assert!(loud(clear * T, (clear + 30) * T), "the fanfare");
        assert!(lint(&m).is_empty(), "{:?}", lint(&m));
    }
}
