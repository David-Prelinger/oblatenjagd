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
//! Sources are enumerated per [`Kind`]; a new kind (E57: `.song` in
//! `audio/music/`) adds a variant, its directory, its render arm in
//! [`render`], its target in [`target_for`] and its extra review images in
//! [`write_review`] (the piano roll). Every command then handles it.

pub mod golden;
pub mod lint;
pub mod plot;
pub mod stats;
pub mod targets;

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
cargo xtask audio render <id>|<file.sfx>|--all
        render to $CARGO_TARGET_DIR/preview/audio/: <id>.wav, <id>_wave.png,
        <id>_spec.png, <id>.json (stats); --all also sheet_spectrograms.png
cargo xtask audio lint [id]
        signal lints against audio/targets.toml (check step `audio-lint`)
cargo xtask audio golden                compare PCM hashes with tests/golden/audio.toml
cargo xtask audio golden --accept <id>|--all
        write the current hashes; only after a game-designer APPROVED verdict (D-117)
cargo xtask audio golden --prune        drop lines of removed sounds and placeholders
";

/// Where a sound's source lives and how it is rendered.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// `audio/sfx/*.sfx`.
    Sfx,
    /// `audio/jingles/*.sfx`.
    Jingle,
}

impl Kind {
    pub const ALL: [Kind; 2] = [Kind::Sfx, Kind::Jingle];

    pub fn name(self) -> &'static str {
        match self {
            Kind::Sfx => "sfx",
            Kind::Jingle => "jingle",
        }
    }

    /// Directory (relative to the workspace root) and file extension.
    fn files(self) -> (&'static str, &'static str) {
        match self {
            Kind::Sfx => ("audio/sfx", "sfx"),
            Kind::Jingle => ("audio/jingles", "sfx"),
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
    let r = match source.kind {
        Kind::Sfx | Kind::Jingle => render_sfx_file(&source.path)?,
    };
    Ok(Rendered {
        source: source.clone(),
        stats: stats::analyse(&r.pcm, r.clamped),
        pcm: r.pcm,
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

/// The target a source is linted against (`.sfx` kinds: its entry in
/// `audio/targets.toml`).
pub fn target_for<'a>(source: &Source, targets: &'a Targets) -> Option<&'a Target> {
    match source.kind {
        Kind::Sfx | Kind::Jingle => targets.get(&source.id),
    }
}

/// Wiring problems: duplicate ids, sources without a target, targets
/// without a source, jingles that are not in class `jingle` and SFX that are.
pub fn wiring(srcs: &[Source], targets: &Targets) -> Vec<String> {
    let mut out = Vec::new();
    for (i, s) in srcs.iter().enumerate() {
        if let Some(other) = srcs[..i].iter().find(|o| o.id == s.id) {
            out.push(format!(
                "{}: id `{}` is also used by {} (ids are unique across audio/)",
                s.path.display(),
                s.id,
                other.path.display()
            ));
        }
        match (s.kind, target_for(s, targets)) {
            (Kind::Sfx | Kind::Jingle, None) => out.push(format!(
                "{}: no `[{}]` in audio/targets.toml (give it a class)",
                s.path.display(),
                s.id
            )),
            (Kind::Sfx, Some(t)) if t.class == Class::Jingle => out.push(format!(
                "{}: class `jingle` belongs in audio/jingles/",
                s.path.display()
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
        if !srcs
            .iter()
            .any(|s| &s.id == id && matches!(s.kind, Kind::Sfx | Kind::Jingle))
        {
            out.push(format!(
                "audio/targets.toml: `[{id}]` has no audio/sfx/{id}.sfx or audio/jingles/{id}.sfx"
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
        stats::to_json(id, r.source.kind.name(), &r.pcm, &r.stats).as_bytes(),
    )?;
    let wave = plot::waveform(&r.pcm, target.map(lint::peak_band));
    write(&dir.join(format!("{id}_wave.png")), &wave.encode_png())?;
    let spec = plot::spectrogram(&r.pcm);
    write(&dir.join(format!("{id}_spec.png")), &spec.encode_png())?;
    // Kind-specific extra images go here (E57: the piano roll of a song).
    match r.source.kind {
        Kind::Sfx | Kind::Jingle => {}
    }
    Ok(plot::cell(&wave, &spec))
}

/// `render <id>|<file.sfx>|--all` (`None` = all).
fn render_cmd(root: &Path, which: Option<&str>) -> Result<(), String> {
    let all = sources(root)?;
    let chosen: Vec<Source> = match which {
        None => all,
        Some(w) if w.ends_with(".sfx") => {
            // A file outside audio/ (a draft, a test fixture), rendered as an SFX.
            let path = PathBuf::from(w);
            let id = path
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();
            vec![Source {
                kind: Kind::Sfx,
                id,
                path,
            }]
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
        let cell = write_review(&dir, &r, target_for(&r.source, &targets))?;
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
        "wrote {}/<id>{{.wav,.json,_wave.png,_spec.png}}",
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
        for f in lint::check(&r.stats, target) {
            errors += 1;
            text.push_str(&format!(
                "error: {} [{}]: {}\n",
                src.path.strip_prefix(root).unwrap_or(&src.path).display(),
                f.lint.name(),
                f.message
            ));
        }
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
    match golden_check(root) {
        Ok(r) => r,
        Err(e) => (false, format!("error: {e}\n")),
    }
}

fn golden_check(root: &Path) -> Result<(bool, String), String> {
    let report_dir = reports_dir(root);
    if report_dir.exists() {
        fs::remove_dir_all(&report_dir)
            .map_err(|e| format!("clean {}: {e}", report_dir.display()))?;
    }
    let goldens = golden::load(root)?;
    let targets = load_targets(root)?;
    let srcs = sources(root)?;
    let mut text = String::new();
    let (mut failed, mut compared) = (0, 0);
    for r in render_all(&srcs) {
        let r = r?;
        let id = &r.source.id;
        let target = target_for(&r.source, &targets);
        let placeholder = target.is_some_and(|t| t.placeholder);
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
        "audio goldens: {compared} compared, {failed} failed ({} sound(s))\n",
        srcs.len()
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
        refuse(&src.id, target, &r.stats)?;
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
}
