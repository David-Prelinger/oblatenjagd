//! `cargo xtask audio`: validating sound without ears (docs/validation.md,
//! "Audio"; docs/proposals/ma-audio-architecture.md, "Validation without
//! ears").
//!
//! - `render <id>|<file.sfx>|--all`: WAV, waveform and spectrogram PNGs and
//!   JSON stats per sound under `$CARGO_TARGET_DIR/preview/audio/`; `--all`
//!   also writes the spectrogram overview `sheet_spectrograms.png`.
//! - `lint [id]` (`check` step `audio-lint`): the signal lints of `lint.rs`
//!   against `audio/targets.toml`, plus the wiring (every sound has a
//!   target, every target a sound, ids are unique).
//! - `golden [--accept <id>|--all] | --prune` (`verify` step
//!   `audio-goldens`): PCM hashes in `tests/golden/audio.toml`.
//!
//! Sources are enumerated per [`Kind`]; a kind has its directory, its
//! render arm in [`render`], its target in [`target_for`] and its extra
//! review images in [`write_review`]. Songs (`audio/music/*.song`, E57) are
//! rendered through `crate::audio_song`, get a default target by their kind
//! (loop: class `song`; jingle: class `jingle`), a piano roll, loop fields
//! in their JSON, and the music lints of `jnr_audio::music_lint` in `lint`.

pub mod golden;
pub mod lint;
pub mod plot;
pub mod stats;
pub mod targets;

use std::borrow::Cow;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use jnr_artkit::Image;
use jnr_audio::{Pcm, Render, Sfx};

use crate::content;
use golden::{Golden, Verdict};
use stats::Stats;
use targets::{Class, Target, Targets};

pub const HELP: &str = "\
cargo xtask audio render <id>|<file.sfx>|<file.song>|--all
        render to $CARGO_TARGET_DIR/preview/audio/: <id>.wav, <id>_wave.png,
        <id>_spec.png, <id>.json (stats), songs <id>_roll.png (piano roll);
        --all also sheet_spectrograms.png
cargo xtask audio lint [id]
        signal lints against audio/targets.toml (songs: their kind's defaults)
        and the music lints of every .song (check step `audio-lint`)
cargo xtask audio golden                compare PCM hashes with tests/golden/audio.toml
cargo xtask audio golden --accept <id>|--all
        write the current hashes; only after a game-designer APPROVED verdict (D-117)
cargo xtask audio golden --prune        drop lines of removed sounds and placeholders
cargo xtask audio mix
        mix every replay offline and lint the headroom (verify step `audio-mix`)
";

/// Where a sound's source lives and how it is rendered.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// `audio/sfx/*.sfx`.
    Sfx,
    /// `audio/jingles/*.sfx`.
    Jingle,
    /// `audio/music/*.song` (loops and music jingles, E57).
    Song,
}

impl Kind {
    pub const ALL: [Kind; 3] = [Kind::Sfx, Kind::Jingle, Kind::Song];

    pub fn name(self) -> &'static str {
        match self {
            Kind::Sfx => "sfx",
            Kind::Jingle => "jingle",
            Kind::Song => "song",
        }
    }

    /// Directory (relative to the workspace root) and file extension.
    fn files(self) -> (&'static str, &'static str) {
        match self {
            Kind::Sfx => ("audio/sfx", "sfx"),
            Kind::Jingle => ("audio/jingles", "sfx"),
            Kind::Song => ("audio/music", "song"),
        }
    }
}

/// One sound source file.
#[derive(Clone, Debug)]
pub struct Source {
    pub kind: Kind,
    pub id: String,
    pub path: PathBuf,
}

/// A rendered sound with its measurements.
pub struct Rendered {
    pub source: Source,
    pub pcm: Pcm,
    pub stats: Stats,
    /// A looping song's loop length in frames (its whole PCM).
    pub loop_frames: Option<u32>,
}

/// Every source of every kind, sorted by kind, then path. An empty or
/// missing `audio/` is an empty list.
pub fn sources(root: &Path) -> Result<Vec<Source>, String> {
    let mut out = Vec::new();
    for kind in Kind::ALL {
        let (dir, ext) = kind.files();
        for path in content::files_with_ext(&root.join(dir), ext)? {
            let id = path
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();
            out.push(Source { kind, id, path });
        }
    }
    Ok(out)
}

/// Parses and renders an `.sfx` file (errors are `file:line:col: …`).
pub fn render_sfx_file(path: &Path) -> Result<Render, String> {
    let text = content::read(path)?;
    let sfx = Sfx::parse(&path.to_string_lossy(), &text).map_err(|e| e.to_string())?;
    Ok(jnr_audio::render_sfx(&sfx))
}

pub fn render(source: &Source) -> Result<Rendered, String> {
    let (r, loop_frames) = match source.kind {
        Kind::Sfx | Kind::Jingle => (render_sfx_file(&source.path)?, None),
        Kind::Song => {
            let (song, r) = crate::audio_song::render_song_file(&source.path)?;
            let looped = song.kind == jnr_audio::song::SongKind::Loop;
            (r.render, looped.then_some(r.length_frames))
        }
    };
    Ok(Rendered {
        source: source.clone(),
        stats: stats::analyse(&r.pcm, r.clamped),
        pcm: r.pcm,
        loop_frames,
    })
}

/// Renders every source on all cores (results in source order).
fn render_all(srcs: &[Source]) -> Vec<Result<Rendered, String>> {
    crate::par::map_ordered(srcs, crate::par::threads(), render)
}

/// The targets file, `audio/targets.toml` (missing = no targets).
pub fn targets_path(root: &Path) -> PathBuf {
    root.join("audio/targets.toml")
}

pub fn load_targets(root: &Path) -> Result<Targets, String> {
    let p = targets_path(root);
    match fs::read_to_string(&p) {
        Ok(text) => targets::parse("audio/targets.toml", &text),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Targets::new()),
        Err(e) => Err(format!("read {}: {e}", p.display())),
    }
}

/// The class a song's target defaults to: `song` for a loop, `jingle` for
/// a music jingle (a song that doesn't parse: `song`; `render` reports it).
fn song_class(path: &Path) -> Class {
    match crate::audio_song::song_kind(path) {
        Some(jnr_audio::song::SongKind::Jingle) => Class::Jingle,
        _ => Class::Song,
    }
}

/// The target a source is linted against: `.sfx` kinds: its entry in
/// `audio/targets.toml`; songs: that entry if there is one (to mark a
/// placeholder or set a peak), else the default of their kind, so every
/// song is linted.
pub fn target_for<'a>(source: &Source, targets: &'a Targets) -> Option<Cow<'a, Target>> {
    match source.kind {
        Kind::Sfx | Kind::Jingle => targets.get(&source.id).map(Cow::Borrowed),
        Kind::Song => Some(match targets.get(&source.id) {
            Some(t) => Cow::Borrowed(t),
            None => Cow::Owned(Target {
                class: song_class(&source.path),
                peak_dbfs: None,
                duration_ms: None,
                centroid_hz: None,
                placeholder: false,
            }),
        }),
    }
}

/// Ids are file stems of `[a-z0-9_]+` (they become golden keys and asset names).
fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}

/// Wiring problems: duplicate ids, sources without a target, targets
/// without a source, jingles that are not in class `jingle` and SFX that are.
pub fn wiring(srcs: &[Source], targets: &Targets) -> Vec<String> {
    let mut out = Vec::new();
    for (i, s) in srcs.iter().enumerate() {
        if !valid_id(&s.id) {
            out.push(format!(
                "{}: bad id `{}`: use lowercase letters, digits and `_`",
                s.path.display(),
                s.id
            ));
        }
        if let Some(other) = srcs[..i].iter().find(|o| o.id == s.id) {
            out.push(format!(
                "{}: id `{}` is also used by {} (ids are unique across audio/)",
                s.path.display(),
                s.id,
                other.path.display()
            ));
        }
        match (s.kind, target_for(s, targets).as_deref()) {
            (Kind::Sfx | Kind::Jingle, None) => out.push(format!(
                "{}: no `[{}]` in audio/targets.toml (give it a class)",
                s.path.display(),
                s.id
            )),
            (Kind::Sfx, Some(t)) if t.class == Class::Jingle => out.push(format!(
                "{}: class `jingle` belongs in audio/jingles/",
                s.path.display()
            )),
            (Kind::Sfx | Kind::Jingle, Some(t)) if t.class == Class::Song => out.push(format!(
                "{}: class `song` is for loops in audio/music/",
                s.path.display()
            )),
            (Kind::Song, Some(t)) if t.class != song_class(&s.path) => out.push(format!(
                "{}: this song's class is `{}` (its `kind`), not `{}`",
                s.path.display(),
                song_class(&s.path).name(),
                t.class.name()
            )),
            (Kind::Jingle, Some(t)) if t.class != Class::Jingle => out.push(format!(
                "{}: a jingle needs class `jingle` (has `{}`)",
                s.path.display(),
                t.class.name()
            )),
            _ => {}
        }
    }
    for id in targets.keys() {
        if !srcs.iter().any(|s| &s.id == id) {
            out.push(format!(
                "audio/targets.toml: `[{id}]` has no audio/sfx/{id}.sfx, audio/jingles/{id}.sfx or audio/music/{id}.song"
            ));
        }
    }
    out
}

// --------------------------------------------------------------------- CLI

pub fn run(root: &Path, args: &[String]) -> ExitCode {
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let result = match args.as_slice() {
        ["render", "--all"] => render_cmd(root, None),
        ["render", which] => render_cmd(root, Some(which)),
        ["lint"] => print_result(lint_report(root, None)),
        ["lint", id] => print_result(lint_report(root, Some(id))),
        ["golden"] => print_result(golden_report(root)),
        ["golden", "--accept", "--all"] => accept(root, None),
        ["golden", "--accept", id] => accept(root, Some(id)),
        ["golden", "--prune"] => prune(root),
        ["mix"] => print_result(crate::replay_audio::check_report(root)),
        [] | ["help" | "-h" | "--help"] => {
            print!("{HELP}");
            Ok(())
        }
        _ => Err(format!("unknown arguments: {}\n\n{HELP}", args.join(" "))),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(msg) => {
            if !msg.is_empty() {
                eprintln!("error: {msg}");
            }
            ExitCode::FAILURE
        }
    }
}

fn print_result((ok, text): (bool, String)) -> Result<(), String> {
    print!("{text}");
    if ok { Ok(()) } else { Err(String::new()) }
}

fn preview_dir(root: &Path) -> Result<PathBuf, String> {
    let dir = crate::target_dir(root).join("preview/audio");
    fs::create_dir_all(&dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
    Ok(dir)
}

fn write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    fs::write(path, bytes).map_err(|e| format!("write {}: {e}", path.display()))
}

/// Writes the review set of `r` into `dir`: `<id>.wav`, `<id>.json`,
/// `<id>_wave.png`, `<id>_spec.png`. Returns the overview cell (waveform
/// above spectrogram) for the sheet.
pub fn write_review(dir: &Path, r: &Rendered, target: Option<&Target>) -> Result<Image, String> {
    let id = &r.source.id;
    write(
        &dir.join(format!("{id}.wav")),
        &jnr_audio::wav::write(&r.pcm),
    )?;
    write(
        &dir.join(format!("{id}.json")),
        stats::to_json(id, r.source.kind.name(), &r.pcm, &r.stats, r.loop_frames).as_bytes(),
    )?;
    let wave = plot::waveform(&r.pcm, target.map(lint::peak_band));
    write(&dir.join(format!("{id}_wave.png")), &wave.encode_png())?;
    let spec = plot::spectrogram(&r.pcm);
    write(&dir.join(format!("{id}_spec.png")), &spec.encode_png())?;
    // Kind-specific extra images.
    match r.source.kind {
        Kind::Sfx | Kind::Jingle => {}
        Kind::Song => {
            let roll = crate::audio_song::roll_of_file(&r.source.path)?;
            write(&dir.join(format!("{id}_roll.png")), &roll.encode_png())?;
        }
    }
    Ok(plot::cell(&wave, &spec))
}

/// `render <id>|<file.sfx>|--all` (`None` = all).
fn render_cmd(root: &Path, which: Option<&str>) -> Result<(), String> {
    let all = sources(root)?;
    let chosen: Vec<Source> = match which {
        None => all,
        Some(w) if w.ends_with(".sfx") || w.ends_with(".song") => {
            // A file outside audio/ (a draft, a test fixture), rendered as an
            // SFX or a song (with the instruments and quotes found under the
            // song directory's parent, like audio/ for audio/music/).
            let path = PathBuf::from(w);
            let id = path
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();
            let kind = if w.ends_with(".song") {
                Kind::Song
            } else {
                Kind::Sfx
            };
            vec![Source { kind, id, path }]
        }
        Some(w) => {
            let found: Vec<Source> = all.iter().filter(|s| s.id == w).cloned().collect();
            if found.is_empty() {
                let ids: Vec<&str> = all.iter().map(|s| s.id.as_str()).collect();
                return Err(format!(
                    "no sound `{w}` in audio/ (known: {})",
                    if ids.is_empty() {
                        "none".to_owned()
                    } else {
                        ids.join(", ")
                    }
                ));
            }
            found
        }
    };
    if chosen.is_empty() {
        println!("audio render: no sounds in audio/ (nothing to do)");
        return Ok(());
    }
    let targets = load_targets(root)?;
    let dir = preview_dir(root)?;
    let cells_dir = dir.join("sheet_cells");
    fs::create_dir_all(&cells_dir).map_err(|e| format!("create {}: {e}", cells_dir.display()))?;
    let mut cells = Vec::new();
    for r in render_all(&chosen) {
        let r = r?;
        let cell = write_review(&dir, &r, target_for(&r.source, &targets).as_deref())?;
        let st = &r.stats;
        println!(
            "{:<16} {:>7.1} ms  peak {:>6.2} dBFS  RMS {:>6.2} dBFS  centroid {:>5.0} Hz  clamped {}",
            r.source.id,
            st.duration_ms,
            st.peak_dbfs,
            st.rms_dbfs,
            st.spectral_centroid_hz,
            st.clamped_samples
        );
        if which.is_none() {
            let path = cells_dir.join(format!("{}.png", r.source.id));
            write(&path, &cell.encode_png())?;
            cells.push((r.source.id.clone(), path));
        }
    }
    println!(
        "wrote {}/<id>{{.wav,.json,_wave.png,_spec.png}} (songs also <id>_roll.png)",
        dir.display()
    );
    if which.is_none() {
        let sheet = dir.join("sheet_spectrograms.png");
        plot::sheet(&cells, &sheet)?;
        println!("overview: {}", sheet.display());
    }
    Ok(())
}

/// Lints every sound (or the one with `id`); returns (no errors, text).
pub fn lint_report(root: &Path, id: Option<&str>) -> (bool, String) {
    match lint_all(root, id) {
        Ok(r) => r,
        Err(e) => (false, format!("error: {e}\n")),
    }
}

fn lint_all(root: &Path, id: Option<&str>) -> Result<(bool, String), String> {
    let targets = load_targets(root)?;
    let all = sources(root)?;
    let mut text = String::new();
    let mut errors = 0;
    if id.is_none() {
        for w in wiring(&all, &targets) {
            errors += 1;
            text.push_str(&format!("error: {w}\n"));
        }
    }
    let chosen: Vec<Source> = all
        .iter()
        .filter(|s| id.is_none_or(|i| s.id == i))
        .cloned()
        .collect();
    if let (Some(i), true) = (id, chosen.is_empty()) {
        return Err(format!("no sound `{i}` in audio/"));
    }
    for (src, r) in chosen.iter().zip(render_all(&chosen)) {
        let r = match r {
            Ok(r) => r,
            Err(e) => {
                errors += 1;
                text.push_str(&format!("error: {e}\n"));
                continue;
            }
        };
        let Some(target) = target_for(src, &targets) else {
            continue; // reported by `wiring`
        };
        for f in lint::check(&r.stats, &target) {
            errors += 1;
            text.push_str(&format!(
                "error: {} [{}]: {}\n",
                src.path.strip_prefix(root).unwrap_or(&src.path).display(),
                f.lint.name(),
                f.message
            ));
        }
    }
    // The music lints read the song text (bars, keys, ranges, quotes).
    if chosen.iter().any(|s| s.kind == Kind::Song) {
        let (music_errors, music) = crate::audio_song::lint_songs(root, &root.join("audio"), id);
        errors += music_errors;
        text.push_str(&music);
    }
    text.push_str(&format!(
        "audio lint: {} sound(s), {errors} error(s)\n",
        chosen.len()
    ));
    Ok((errors == 0, text))
}

fn reports_dir(root: &Path) -> PathBuf {
    crate::target_dir(root).join("reports/audio-golden")
}

/// Compares every sound with `tests/golden/audio.toml`; returns (all
/// equal, text). Failures write the review set to `reports/audio-golden/`.
pub fn golden_report(root: &Path) -> (bool, String) {
    match golden_check(root, &reports_dir(root)) {
        Ok(r) => r,
        Err(e) => (false, format!("error: {e}\n")),
    }
}

/// [`golden_report`] with its failure reports in `report_dir` (emptied first).
fn golden_check(root: &Path, report_dir: &Path) -> Result<(bool, String), String> {
    let report_dir = report_dir.to_path_buf();
    if report_dir.exists() {
        fs::remove_dir_all(&report_dir)
            .map_err(|e| format!("clean {}: {e}", report_dir.display()))?;
    }
    let goldens = golden::load(root)?;
    let targets = load_targets(root)?;
    let srcs = sources(root)?;
    let mut text = String::new();
    let (mut failed, mut compared) = (0, 0);
    let mut placeholders: Vec<String> = Vec::new();
    for r in render_all(&srcs) {
        let r = r?;
        let id = &r.source.id;
        let target = target_for(&r.source, &targets);
        let target = target.as_deref();
        let placeholder = target.is_some_and(|t| t.placeholder);
        if placeholder {
            placeholders.push(id.clone());
        }
        let actual = Golden::of(&r.pcm, &r.stats);
        let expected = goldens.get(id);
        let problem = match golden::compare(placeholder, expected, &actual) {
            Verdict::Same => {
                if expected.is_some() {
                    compared += 1;
                    text.push_str(&format!("ok   {id}\n"));
                }
                None
            }
            Verdict::Placeholder => Some(format!(
                "`{id}` is a placeholder (audio/targets.toml) but has a golden; \
                 run `cargo xtask audio golden --prune`"
            )),
            Verdict::Missing => Some(format!(
                "no golden for `{id}`: review preview/audio/{id}_* from `cargo xtask audio render {id}`; \
                 after a game-designer APPROVED verdict: `cargo xtask audio golden --accept {id}`"
            )),
            Verdict::Differs(what) => {
                compared += 1;
                Some(format!("PCM differs: {what}"))
            }
        };
        if let Some(p) = problem {
            failed += 1;
            fs::create_dir_all(&report_dir)
                .map_err(|e| format!("create {}: {e}", report_dir.display()))?;
            write_review(&report_dir, &r, target)?;
            if let Some(g) = expected {
                let mut one = golden::Goldens::new();
                one.insert(id.clone(), g.clone());
                write(
                    &report_dir.join(format!("{id}_expected.toml")),
                    golden::format(&one).as_bytes(),
                )?;
            }
            text.push_str(&format!(
                "FAIL {id}: {p}\n     see {}/{id}{{.wav,.json,_wave.png,_spec.png}}\n",
                report_dir.display()
            ));
        }
    }
    for id in goldens.keys() {
        if !srcs.iter().any(|s| &s.id == id) {
            failed += 1;
            text.push_str(&format!(
                "FAIL unused golden `{id}` (no such sound); run `cargo xtask audio golden --prune`\n"
            ));
        }
    }
    text.push_str(&format!(
        "audio goldens: {compared} compared, {failed} failed ({} sound(s), {} placeholder(s){})\n",
        srcs.len(),
        placeholders.len(),
        if placeholders.is_empty() {
            String::new()
        } else {
            format!(": {}", placeholders.join(", "))
        }
    ));
    Ok((failed == 0, text))
}

/// `golden --accept <id>` (`None` = every non-placeholder sound).
fn accept(root: &Path, which: Option<&str>) -> Result<(), String> {
    let targets = load_targets(root)?;
    let srcs = sources(root)?;
    let chosen: Vec<Source> = srcs
        .iter()
        .filter(|s| match which {
            Some(w) => s.id == w,
            None => !target_for(s, &targets).is_some_and(|t| t.placeholder),
        })
        .cloned()
        .collect();
    if let (Some(w), true) = (which, chosen.is_empty()) {
        return Err(format!("no sound `{w}` in audio/"));
    }
    let mut goldens = golden::load(root)?;
    for (src, r) in chosen.iter().zip(render_all(&chosen)) {
        let r = r?;
        let target = target_for(src, &targets)
            .ok_or_else(|| format!("`{}` has no entry in audio/targets.toml", src.id))?;
        refuse(&src.id, &target, &r.stats)?;
        goldens.insert(src.id.clone(), Golden::of(&r.pcm, &r.stats));
        println!("accepted {} ({})", src.id, golden::fingerprint(&r.pcm));
    }
    golden::save(root, &goldens)?;
    println!("wrote {}", golden::path(root).display());
    Ok(())
}

/// Goldens are accepted only for real sounds that pass their lints.
fn refuse(id: &str, target: &Target, st: &Stats) -> Result<(), String> {
    if target.placeholder {
        return Err(format!(
            "refusing to accept `{id}`: it is a placeholder (audio/targets.toml); \
             placeholders never become goldens"
        ));
    }
    let found = lint::check(st, target);
    if let Some(f) = found.first() {
        return Err(format!(
            "refusing to accept `{id}`: it fails `audio lint` ([{}] {}); fix it first",
            f.lint.name(),
            f.message
        ));
    }
    Ok(())
}

/// `golden --prune`: removes lines of sounds that no longer exist and of
/// placeholders (only removes, so it needs no verdict).
fn prune(root: &Path) -> Result<(), String> {
    let targets = load_targets(root)?;
    let srcs = sources(root)?;
    let mut goldens = golden::load(root)?;
    let before: Vec<String> = goldens.keys().cloned().collect();
    goldens.retain(|id, _| {
        srcs.iter()
            .find(|s| &s.id == id)
            .is_some_and(|s| !target_for(s, &targets).is_some_and(|t| t.placeholder))
    });
    for id in before.iter().filter(|id| !goldens.contains_key(*id)) {
        println!("pruned {id}");
    }
    if before.len() != goldens.len() {
        golden::save(root, &goldens)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn src(kind: Kind, id: &str) -> Source {
        Source {
            kind,
            id: id.into(),
            path: PathBuf::from(format!("audio/{}/{id}.sfx", kind.name())),
        }
    }

    fn target(class: Class) -> Target {
        Target {
            class,
            peak_dbfs: None,
            duration_ms: None,
            centroid_hz: None,
            placeholder: false,
        }
    }

    #[test]
    fn wiring_finds_missing_stray_duplicate_and_misfiled() {
        let mut t = Targets::new();
        t.insert("jump".into(), target(Class::Normal));
        t.insert("ghost".into(), target(Class::Soft));
        t.insert("fanfare".into(), target(Class::Normal));
        let srcs = [
            src(Kind::Sfx, "jump"),
            src(Kind::Sfx, "land"),
            src(Kind::Jingle, "fanfare"),
            src(Kind::Jingle, "jump"),
        ];
        let w = wiring(&srcs, &t);
        assert_eq!(w.len(), 5, "{w:#?}");
        assert!(w[0].contains("no `[land]`"), "{}", w[0]);
        assert!(w[1].contains("a jingle needs class `jingle`"), "{}", w[1]);
        assert!(w[2].contains("id `jump` is also used by"), "{}", w[2]);
        assert!(w[3].contains("a jingle needs class `jingle`"), "{}", w[3]);
        assert!(w[4].contains("`[ghost]` has no"), "{}", w[4]);
        let ok = [src(Kind::Sfx, "jump")];
        let mut t2 = Targets::new();
        t2.insert("jump".into(), target(Class::Normal));
        assert!(wiring(&ok, &t2).is_empty());
    }

    #[test]
    fn accept_refuses_placeholders_and_failing_sounds() {
        let r = render_sfx_file(
            &Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/audio/peak_pass.sfx"),
        )
        .unwrap();
        let st = stats::analyse(&r.pcm, r.clamped);
        let good = target(Class::Normal);
        assert_eq!(refuse("x", &good, &st), Ok(()));
        let mut ph = good.clone();
        ph.placeholder = true;
        assert!(refuse("x", &ph, &st).unwrap_err().contains("placeholder"));
        let loud = target(Class::Loud);
        let e = refuse("x", &loud, &st).unwrap_err();
        assert!(e.contains("fails `audio lint` ([peak]"), "{e}");
    }

    /// A throwaway workspace with one sound `beep` (the `peak_pass` fixture)
    /// and its target.
    fn temp_root(target: &str) -> PathBuf {
        let root = crate::target_dir(Path::new(env!("CARGO_MANIFEST_DIR")))
            .join(format!("tmp/xtask-audio-test/{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("audio/sfx")).unwrap();
        fs::create_dir_all(root.join("tests/golden")).unwrap();
        fs::copy(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/audio/peak_pass.sfx"),
            root.join("audio/sfx/beep.sfx"),
        )
        .unwrap();
        fs::write(root.join("audio/targets.toml"), target).unwrap();
        root
    }

    #[test]
    fn lint_and_golden_on_a_workspace() {
        let root = temp_root("[beep]\nclass = \"normal\"\nplaceholder = true\n");
        let (ok, text) = lint_report(&root, None);
        assert!(ok, "{text}");
        assert!(
            text.ends_with("audio lint: 1 sound(s), 0 error(s)\n"),
            "{text}"
        );
        // A placeholder needs no golden and cannot get one.
        let (ok, text) = golden_check(&root, &root.join("reports/audio-golden")).unwrap();
        assert!(ok, "{text}");
        assert!(
            accept(&root, Some("beep"))
                .unwrap_err()
                .contains("placeholder")
        );

        // A real sound must have one; the failure names the report files.
        fs::write(
            root.join("audio/targets.toml"),
            "[beep]\nclass = \"normal\"\n",
        )
        .unwrap();
        let (ok, text) = golden_check(&root, &root.join("reports/audio-golden")).unwrap();
        assert!(!ok);
        assert!(
            text.starts_with("FAIL beep: no golden for `beep`"),
            "{text}"
        );
        assert!(
            text.contains("reports/audio-golden/beep{.wav,.json,_wave.png,_spec.png}"),
            "{text}"
        );
        accept(&root, Some("beep")).unwrap();
        let (ok, text) = golden_check(&root, &root.join("reports/audio-golden")).unwrap();
        assert!(ok, "{text}");
        assert!(text.starts_with("ok   beep\n"), "{text}");

        // One changed sample (a quieter layer) fails it, and lint flags a typo'd target.
        let src = root.join("audio/sfx/beep.sfx");
        let text = fs::read_to_string(&src)
            .unwrap()
            .replace("volume_pct = 35", "volume_pct = 34");
        fs::write(&src, text).unwrap();
        let (ok, text) = golden_check(&root, &root.join("reports/audio-golden")).unwrap();
        assert!(!ok);
        assert!(
            text.starts_with("FAIL beep: PCM differs: pcm fnv64:"),
            "{text}"
        );
        fs::write(
            root.join("audio/targets.toml"),
            "[beeep]\nclass = \"normal\"\n",
        )
        .unwrap();
        let (ok, text) = lint_report(&root, None);
        assert!(!ok);
        assert!(text.contains("no `[beep]` in audio/targets.toml"), "{text}");
        assert!(text.contains("`[beeep]` has no"), "{text}");
        let _ = fs::remove_dir_all(&root);
    }

    /// A throwaway workspace with the jnr-audio song fixtures in
    /// `audio/{instruments,music,quotes}/` and `targets` as targets.toml.
    fn song_root(targets: &str) -> PathBuf {
        let fx =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../crates/jnr-audio/tests/fixtures/music");
        let root = crate::target_dir(Path::new(env!("CARGO_MANIFEST_DIR")))
            .join(format!("tmp/xtask-audio-song-test/{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        for (ext, sub) in [
            ("inst", "instruments"),
            ("song", "music"),
            ("quote", "quotes"),
        ] {
            let dir = root.join("audio").join(sub);
            fs::create_dir_all(&dir).unwrap();
            for f in content::files_with_ext(&fx, ext).unwrap() {
                fs::copy(&f, dir.join(f.file_name().unwrap())).unwrap();
            }
        }
        fs::create_dir_all(root.join("tests/golden")).unwrap();
        fs::write(root.join("audio/targets.toml"), targets).unwrap();
        root
    }

    #[test]
    fn songs_are_rendered_linted_and_golden_checked() {
        let root = song_root("[placeholder_polka]\nclass = \"song\"\nplaceholder = true\n");
        let srcs = sources(&root).unwrap();
        assert_eq!(srcs.iter().filter(|s| s.kind == Kind::Song).count(), 2);
        // Default targets: a loop is class `song`, a music jingle `jingle`.
        let targets = load_targets(&root).unwrap();
        let fanfare = srcs.iter().find(|s| s.id == "placeholder_fanfare").unwrap();
        assert_eq!(target_for(fanfare, &targets).unwrap().class, Class::Jingle);

        let (ok, text) = lint_report(&root, None);
        assert!(ok, "{text}");
        assert!(text.contains("music lint: 2 song(s), 0 error(s)"), "{text}");
        assert!(
            text.ends_with("audio lint: 2 sound(s), 0 error(s)\n"),
            "{text}"
        );

        // The review set has a piano roll and the loop fields.
        let polka = srcs.iter().find(|s| s.id == "placeholder_polka").unwrap();
        let r = render(polka).unwrap();
        let dir = root.join("review");
        fs::create_dir_all(&dir).unwrap();
        write_review(&dir, &r, target_for(polka, &targets).as_deref()).unwrap();
        assert!(dir.join("placeholder_polka_roll.png").exists());
        let json = fs::read_to_string(dir.join("placeholder_polka.json")).unwrap();
        assert!(json.contains("\"loop_aligned\": true"), "{json}");
        assert_eq!(r.loop_frames, Some(r.pcm.samples.len() as u32));

        // Goldens: the placeholder is named, the jingle needs one.
        let (ok, text) = golden_check(&root, &root.join("reports")).unwrap();
        assert!(!ok);
        assert!(
            text.contains("FAIL placeholder_fanfare: no golden"),
            "{text}"
        );
        assert!(
            text.contains("1 placeholder(s): placeholder_polka"),
            "{text}"
        );

        // A music lint error fails `audio lint` with its file and line.
        let song = root.join("audio/music/placeholder_fanfare.song");
        let t = fs::read_to_string(&song)
            .unwrap()
            .replace("e4/2 |", "e4/3 |");
        fs::write(&song, t).unwrap();
        let (ok, text) = lint_report(&root, Some("placeholder_fanfare"));
        assert!(!ok);
        assert!(text.contains("placeholder_fanfare.song:21:"), "{text}");
        assert!(text.contains("error[score]"), "{text}");

        // A song's targets entry must match its kind; a bad id is reported.
        fs::write(
            root.join("audio/targets.toml"),
            "[placeholder_polka]\nclass = \"jingle\"\n",
        )
        .unwrap();
        fs::write(root.join("audio/music/Bad-Id.song"), "").unwrap();
        let w = wiring(&sources(&root).unwrap(), &load_targets(&root).unwrap());
        assert!(w.iter().any(|m| m.contains("class is `song`")), "{w:?}");
        assert!(w.iter().any(|m| m.contains("bad id `Bad-Id`")), "{w:?}");
        let _ = fs::remove_dir_all(&root);
    }
}
