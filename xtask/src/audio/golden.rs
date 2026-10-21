//! Audio goldens: `tests/golden/audio.toml`, one line per sound id with the
//! FNV-1a 64 hash of its rendered PCM (`jnr_audio::Pcm::fnv64`) and, for the
//! failure report, the stats it had when it was accepted:
//!
//! ```toml
//! jump = { pcm = "fnv64:0123456789abcdef", frames = 6615, peak_dbfs = -9.12, rms_dbfs = -15.3, centroid_hz = 1450 }
//! ```
//!
//! Every sound that is not a placeholder must have a golden; placeholders
//! must not. A line is written only by `cargo xtask audio golden --accept`,
//! after a game-designer APPROVED verdict on the rendered review set
//! (D-117); the `guard-golden.sh` hook blocks direct edits of
//! `tests/golden/`.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use jnr_audio::Pcm;
use serde::Deserialize;

use super::stats::Stats;

/// `fnv64:<16 hex digits>`.
pub fn fingerprint(pcm: &Pcm) -> String {
    format!("fnv64:{:016x}", pcm.fnv64())
}

/// One golden line.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Golden {
    pub pcm: String,
    pub frames: usize,
    pub peak_dbfs: f64,
    pub rms_dbfs: f64,
    pub centroid_hz: f64,
}

impl Golden {
    pub fn of(pcm: &Pcm, st: &Stats) -> Golden {
        let round = |v: f64| (v * 100.0).round() / 100.0;
        Golden {
            pcm: fingerprint(pcm),
            frames: st.frames,
            peak_dbfs: round(st.peak_dbfs),
            rms_dbfs: round(st.rms_dbfs),
            centroid_hz: st.spectral_centroid_hz.round(),
        }
    }

    fn line(&self, id: &str) -> String {
        format!(
            "{id} = {{ pcm = \"{}\", frames = {}, peak_dbfs = {:.2}, rms_dbfs = {:.2}, centroid_hz = {:.0} }}\n",
            self.pcm, self.frames, self.peak_dbfs, self.rms_dbfs, self.centroid_hz
        )
    }
}

pub type Goldens = BTreeMap<String, Golden>;

const HEADER: &str = "\
# Audio goldens (docs/validation.md, \"Audio\"; D-117): the FNV-1a 64 hash of
# each sound's rendered PCM, plus its stats at acceptance (for the report).
# Written only by `cargo xtask audio golden --accept <id>` after a
# game-designer APPROVED verdict. Do not edit by hand.
";

pub fn path(root: &Path) -> PathBuf {
    crate::content::golden_dir(root).join("audio.toml")
}

/// Reads the goldens; a missing file is an empty list.
pub fn load(root: &Path) -> Result<Goldens, String> {
    let p = path(root);
    match fs::read_to_string(&p) {
        Ok(text) => parse(&text).map_err(|e| format!("{}: {e}", p.display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Goldens::new()),
        Err(e) => Err(format!("read {}: {e}", p.display())),
    }
}

pub fn parse(text: &str) -> Result<Goldens, String> {
    toml::from_str(text).map_err(|e| e.message().trim().to_owned())
}

/// The file text: the header and one line per id, sorted.
pub fn format(goldens: &Goldens) -> String {
    let mut out = HEADER.to_owned();
    for (id, g) in goldens {
        out.push_str(&g.line(id));
    }
    out
}

pub fn save(root: &Path, goldens: &Goldens) -> Result<(), String> {
    let p = path(root);
    fs::write(&p, format(goldens)).map_err(|e| format!("write {}: {e}", p.display()))
}

/// What a sound's render says about its golden.
#[derive(Debug, PartialEq)]
pub enum Verdict {
    Same,
    /// A sound that needs a golden has none.
    Missing,
    /// A placeholder has a golden (it must not).
    Placeholder,
    /// The hash differs; a readable summary of what changed.
    Differs(String),
}

/// Compares one render with its golden line (if any).
pub fn compare(placeholder: bool, golden: Option<&Golden>, actual: &Golden) -> Verdict {
    match (placeholder, golden) {
        (true, None) => Verdict::Same,
        (true, Some(_)) => Verdict::Placeholder,
        (false, None) => Verdict::Missing,
        (false, Some(g)) if g.pcm == actual.pcm => Verdict::Same,
        (false, Some(g)) => Verdict::Differs(format!(
            "pcm {} → {}; frames {} → {}; peak {:.2} → {:.2} dBFS; RMS {:.2} → {:.2} dBFS; centroid {:.0} → {:.0} Hz",
            g.pcm,
            actual.pcm,
            g.frames,
            actual.frames,
            g.peak_dbfs,
            actual.peak_dbfs,
            g.rms_dbfs,
            actual.rms_dbfs,
            g.centroid_hz,
            actual.centroid_hz
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::stats::analyse;

    fn pcm(samples: Vec<i16>) -> Pcm {
        Pcm {
            rate: 44_100,
            samples,
        }
    }

    fn golden_of(p: &Pcm) -> Golden {
        Golden::of(p, &analyse(p, 0))
    }

    #[test]
    fn a_one_sample_change_fails_the_golden() {
        let a = pcm((0..2000).map(|i| ((i * 37) % 2000 - 1000) as i16).collect());
        let mut b = a.clone();
        b.samples[1234] += 1;
        let (ga, gb) = (golden_of(&a), golden_of(&b));
        assert_eq!(compare(false, Some(&ga), &ga), Verdict::Same);
        let Verdict::Differs(msg) = compare(false, Some(&ga), &gb) else {
            panic!("a one-sample change must differ");
        };
        assert!(
            msg.starts_with(&format!("pcm {} → {}", ga.pcm, gb.pcm)),
            "{msg}"
        );
        // Stats rounded to 0.01 dB do not see it; the hash does.
        assert_eq!((ga.frames, ga.peak_dbfs), (gb.frames, gb.peak_dbfs));
    }

    #[test]
    fn placeholders_never_have_goldens_and_real_sounds_always() {
        let g = golden_of(&pcm(vec![0, 5, -5, 0]));
        assert_eq!(compare(true, None, &g), Verdict::Same);
        assert_eq!(compare(true, Some(&g), &g), Verdict::Placeholder);
        assert_eq!(compare(false, None, &g), Verdict::Missing);
    }

    #[test]
    fn file_round_trips() {
        let mut goldens = Goldens::new();
        goldens.insert("zap".into(), golden_of(&pcm(vec![0, 300, -300, 0])));
        goldens.insert("jump".into(), golden_of(&pcm(vec![0, 9000, -9000, 0])));
        let text = format(&goldens);
        assert!(text.starts_with("# Audio goldens"));
        let lines: Vec<&str> = text.lines().filter(|l| !l.starts_with('#')).collect();
        assert_eq!(lines.len(), 2);
        assert!(
            lines[0].starts_with("jump = { pcm = \"fnv64:"),
            "{}",
            lines[0]
        );
        assert_eq!(parse(&text).unwrap(), goldens);
        assert!(parse("jump = { pcm = \"x\" }").is_err());
    }
}
