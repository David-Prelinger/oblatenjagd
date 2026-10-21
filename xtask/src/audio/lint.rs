//! Signal lints for a rendered sound against its [`Target`]. Every
//! threshold comes from docs/audio-design.md (sections 3, 6 and 9) or
//! docs/proposals/ma-audio-architecture.md ("Validation without ears");
//! each lint has a failing and a passing fixture in `xtask/fixtures/audio/`.

use super::stats::{Stats, dbfs_to_sample};
use super::targets::{Class, Target};

/// Hard ceiling of every render (master bus, section 3).
pub const CEILING_DBFS: f64 = -1.0;
/// A peak target (class or `peak_dbfs`) is met within ± this.
pub const PEAK_TOL_DB: f64 = 1.0;
/// |DC offset| must stay below this (% of full scale, section 3).
pub const DC_MAX_PCT: f64 = 0.5;
/// The sound must be audible (≥ −40 dBFS) within this (responsiveness).
pub const ONSET_MAX_MS: f64 = 10.0;
/// At most this much inaudible (< −40 dBFS) time at the end.
pub const TAIL_MAX_MS: f64 = 50.0;
/// The first and last sample stay at or below this, so nothing clicks
/// (section 3: "every cue starts and ends at zero").
pub const EDGE_MAX_DBFS: f64 = -60.0;
/// A `duration_ms` target is met within ± this (section 9: "150 ms ± 10").
pub const DURATION_TOL_MS: f64 = 10.0;

/// One lint (the names are what `audio lint` prints).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lint {
    Clamp,
    Ceiling,
    Peak,
    Rms,
    Dc,
    Onset,
    Tail,
    Edges,
    Length,
    Duration,
    Centroid,
}

impl Lint {
    #[cfg(test)]
    pub const ALL: [Lint; 11] = [
        Lint::Clamp,
        Lint::Ceiling,
        Lint::Peak,
        Lint::Rms,
        Lint::Dc,
        Lint::Onset,
        Lint::Tail,
        Lint::Edges,
        Lint::Length,
        Lint::Duration,
        Lint::Centroid,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Lint::Clamp => "clamp",
            Lint::Ceiling => "ceiling",
            Lint::Peak => "peak",
            Lint::Rms => "rms",
            Lint::Dc => "dc",
            Lint::Onset => "onset",
            Lint::Tail => "tail",
            Lint::Edges => "edges",
            Lint::Length => "length",
            Lint::Duration => "duration",
            Lint::Centroid => "centroid",
        }
    }
}

/// A failed lint.
#[derive(Clone, Debug, PartialEq)]
pub struct Finding {
    pub lint: Lint,
    pub message: String,
}

/// The class's peak band in dBFS (inclusive), docs/audio-design.md section 3.
pub fn class_peak(class: Class) -> (f64, f64) {
    match class {
        Class::Loud => (-6.0 - PEAK_TOL_DB, -6.0 + PEAK_TOL_DB),
        Class::Normal => (-9.0 - PEAK_TOL_DB, -9.0 + PEAK_TOL_DB),
        Class::Soft => (-16.0 - PEAK_TOL_DB, -14.0 + PEAK_TOL_DB),
        Class::Ambience => (-26.0 - PEAK_TOL_DB, -24.0 + PEAK_TOL_DB),
        // "peak ≤ −6 dBFS"; the RMS band sets the loudness.
        Class::Jingle => (-20.0, -6.0),
        // The music bus: "sample peak ≤ −8 dBFS".
        Class::Song => (-20.0, -8.0),
    }
}

/// The class's RMS band in dBFS, if it has one (jingles: −17 ± 2).
pub fn class_rms(class: Class) -> Option<(f64, f64)> {
    match class {
        Class::Jingle => Some((-19.0, -15.0)),
        // "RMS over one full loop −20 ± 2 dBFS".
        Class::Song => Some((-22.0, -18.0)),
        Class::Loud | Class::Normal | Class::Soft | Class::Ambience => None,
    }
}

/// The class's length bounds in ms (inclusive): an SFX ≤ 1.5 s (and at
/// least about a frame), a jingle ≤ 10 s (the proposal said 6 s, but the
/// design's `game_over` is 8 s).
pub fn class_length_ms(class: Class) -> (f64, f64) {
    match class {
        Class::Loud | Class::Normal | Class::Soft | Class::Ambience => (20.0, 1500.0),
        Class::Jingle => (200.0, 10_000.0),
        Class::Song => (20_000.0, 90_000.0),
    }
}

/// The peak band that applies: `peak_dbfs` ± [`PEAK_TOL_DB`] if set, else
/// the class's.
pub fn peak_band(t: &Target) -> (f64, f64) {
    match t.peak_dbfs {
        Some(p) => (p as f64 - PEAK_TOL_DB, p as f64 + PEAK_TOL_DB),
        None => class_peak(t.class),
    }
}

/// Whether `lint` applies to `class`. A loop (`song`) has no start or end
/// (it wraps sample-continuously, E57), so `onset`, `tail` and `edges`
/// don't apply to it; every other lint applies to every class.
pub fn applies(lint: Lint, class: Class) -> bool {
    !(class == Class::Song && matches!(lint, Lint::Onset | Lint::Tail | Lint::Edges))
}

/// Every lint that fails for `st` against `t`.
pub fn check(st: &Stats, t: &Target) -> Vec<Finding> {
    let mut found = check_all(st, t);
    found.retain(|f| applies(f.lint, t.class));
    found
}

fn check_all(st: &Stats, t: &Target) -> Vec<Finding> {
    let mut out = Vec::new();
    let mut fail = |lint, message: String| out.push(Finding { lint, message });

    if st.clamped_samples > 0 {
        fail(
            Lint::Clamp,
            format!(
                "{} sample(s) clamped to ±32767 (lower `volume_pct`)",
                st.clamped_samples
            ),
        );
    }
    if st.peak_dbfs > CEILING_DBFS {
        fail(
            Lint::Ceiling,
            format!(
                "peak {:.2} dBFS is above the {CEILING_DBFS} dBFS ceiling",
                st.peak_dbfs
            ),
        );
    }
    let (lo, hi) = peak_band(t);
    if st.peak_dbfs < lo || st.peak_dbfs > hi {
        let from = match t.peak_dbfs {
            Some(p) => format!("peak_dbfs = {p}"),
            None => format!("class {}", t.class.name()),
        };
        fail(
            Lint::Peak,
            format!(
                "peak {:.2} dBFS is outside {lo} … {hi} dBFS ({from})",
                st.peak_dbfs
            ),
        );
    }
    if let Some((lo, hi)) =
        class_rms(t.class).filter(|&(lo, hi)| st.rms_dbfs < lo || st.rms_dbfs > hi)
    {
        fail(
            Lint::Rms,
            format!(
                "RMS {:.2} dBFS is outside {lo} … {hi} dBFS (class {})",
                st.rms_dbfs,
                t.class.name()
            ),
        );
    }
    if st.dc_offset_pct.abs() >= DC_MAX_PCT {
        fail(
            Lint::Dc,
            format!(
                "DC offset {:.3} % of full scale (must stay below {DC_MAX_PCT} %)",
                st.dc_offset_pct
            ),
        );
    }
    if st.onset_ms > ONSET_MAX_MS {
        fail(
            Lint::Onset,
            format!(
                "first audible sample (≥ −40 dBFS) at {:.1} ms; at most {ONSET_MAX_MS} ms",
                st.onset_ms
            ),
        );
    }
    if st.silent_tail_ms > TAIL_MAX_MS {
        fail(
            Lint::Tail,
            format!(
                "{:.1} ms of inaudible (< −40 dBFS) tail; at most {TAIL_MAX_MS} ms",
                st.silent_tail_ms
            ),
        );
    }
    // Rounded (−60 dBFS = 32.8 → 33), so the message's bound is the real one.
    let edge = dbfs_to_sample(EDGE_MAX_DBFS).round();
    for (which, v) in [("first", st.first_sample), ("last", st.last_sample)] {
        if (v as f64).abs() > edge {
            fail(
                Lint::Edges,
                format!(
                    "{which} sample is {v}; must be within ±{edge:.0} ({EDGE_MAX_DBFS} dBFS) so it does not click"
                ),
            );
        }
    }
    let (lo, hi) = class_length_ms(t.class);
    if st.duration_ms < lo || st.duration_ms > hi {
        fail(
            Lint::Length,
            format!(
                "{:.1} ms long; class {} allows {lo} … {hi} ms",
                st.duration_ms,
                t.class.name()
            ),
        );
    }
    if let Some(want) = t
        .duration_ms
        .filter(|&want| (st.duration_ms - want as f64).abs() > DURATION_TOL_MS)
    {
        fail(
            Lint::Duration,
            format!(
                "{:.1} ms long; target duration_ms = {want} ± {DURATION_TOL_MS}",
                st.duration_ms
            ),
        );
    }
    if let Some([lo, hi]) = t.centroid_hz.filter(|&[lo, hi]| {
        st.spectral_centroid_hz < lo as f64 || st.spectral_centroid_hz > hi as f64
    }) {
        fail(
            Lint::Centroid,
            format!(
                "spectral centroid {:.0} Hz is outside the target {lo} … {hi} Hz",
                st.spectral_centroid_hz
            ),
        );
    }
    out
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::*;
    use crate::audio::{render_sfx_file, stats, targets};

    fn fixtures() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/audio")
    }

    /// The lints of fixture `<name>.sfx` against its entry in the
    /// fixtures' `targets.toml`.
    fn lint_fixture(name: &str) -> Vec<Finding> {
        let dir = fixtures();
        let targets = targets::parse(
            "fixtures/audio/targets.toml",
            &std::fs::read_to_string(dir.join("targets.toml")).unwrap(),
        )
        .unwrap();
        let target = targets
            .get(name)
            .unwrap_or_else(|| panic!("no target for fixture `{name}`"));
        let r = render_sfx_file(&dir.join(format!("{name}.sfx"))).unwrap();
        check(&stats::analyse(&r.pcm, r.clamped), target)
    }

    #[test]
    fn every_lint_has_a_failing_and_a_passing_fixture() {
        for lint in Lint::ALL {
            let pass = format!("{}_pass", lint.name());
            let found = lint_fixture(&pass);
            assert!(
                found.iter().all(|f| f.lint != lint),
                "{pass} should pass `{}`: {found:?}",
                lint.name()
            );
            let fail = format!("{}_fail", lint.name());
            let found = lint_fixture(&fail);
            assert!(
                found.iter().any(|f| f.lint == lint),
                "{fail} should fail `{}`: {found:?}",
                lint.name()
            );
            // A fixture shows one problem only, so it tests the lint it is
            // named after (a clamped sample is at 0 dBFS: over the ceiling too).
            assert!(
                found
                    .iter()
                    .all(|f| f.lint == lint || (lint, f.lint) == (Lint::Clamp, Lint::Ceiling)),
                "{fail} fails other lints too: {found:?}"
            );
        }
    }

    #[test]
    fn every_fixture_belongs_to_a_lint() {
        let names: Vec<String> = Lint::ALL
            .iter()
            .flat_map(|l| [format!("{}_pass", l.name()), format!("{}_fail", l.name())])
            .collect();
        for entry in std::fs::read_dir(fixtures()).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().is_some_and(|e| e == "sfx") {
                let stem = path.file_stem().unwrap().to_string_lossy().into_owned();
                assert!(names.contains(&stem), "stray fixture {}", path.display());
            }
        }
    }

    #[test]
    fn edges_allow_exactly_the_printed_bound() {
        let r = render_sfx_file(&fixtures().join("edges_pass.sfx")).unwrap();
        let mut st = stats::analyse(&r.pcm, r.clamped);
        let t = Target {
            class: Class::Normal,
            peak_dbfs: None,
            duration_ms: None,
            centroid_hz: None,
            placeholder: false,
        };
        let edges = |st: &Stats| {
            check_all(st, &t)
                .into_iter()
                .filter(|f| f.lint == Lint::Edges)
                .count()
        };
        st.first_sample = 33;
        st.last_sample = -33;
        assert_eq!(edges(&st), 0);
        st.first_sample = 34;
        assert_eq!(edges(&st), 1);
    }

    #[test]
    fn loops_skip_the_start_and_end_lints() {
        let r = render_sfx_file(&fixtures().join("edges_fail.sfx")).unwrap();
        let st = stats::analyse(&r.pcm, r.clamped);
        let t = Target {
            class: Class::Song,
            peak_dbfs: None,
            duration_ms: None,
            centroid_hz: None,
            placeholder: false,
        };
        let found = check(&st, &t);
        assert!(
            found
                .iter()
                .all(|f| !matches!(f.lint, Lint::Edges | Lint::Onset | Lint::Tail)),
            "{found:?}"
        );
        assert!(
            found.iter().any(|f| f.lint == Lint::Length),
            "a short sound is no loop: {found:?}"
        );
        assert_eq!(class_rms(Class::Song), Some((-22.0, -18.0)));
    }

    #[test]
    fn class_bands_follow_the_design() {
        assert_eq!(class_peak(Class::Normal), (-10.0, -8.0));
        assert_eq!(class_peak(Class::Loud), (-7.0, -5.0));
        assert_eq!(class_peak(Class::Soft), (-17.0, -13.0));
        let t = Target {
            class: Class::Normal,
            peak_dbfs: Some(-12),
            duration_ms: None,
            centroid_hz: None,
            placeholder: false,
        };
        assert_eq!(peak_band(&t), (-13.0, -11.0));
    }
}
