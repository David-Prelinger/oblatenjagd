//! `audio/targets.toml`: the review targets of every `.sfx` (its loudness
//! class from docs/audio-design.md section 3, optional per-id numbers from
//! the SFX table in section 6, and the placeholder mark). The lints read it;
//! the synth never does.
//!
//! ```toml
//! [jump]
//! class = "normal"             # loud | normal | soft | ambience | jingle | song
//! peak_dbfs = -9               # optional: overrides the class peak (±1 dB)
//! duration_ms = 150            # optional: the rendered length (±10 ms)
//! centroid_hz = [800, 3000]    # optional: spectral-centroid band
//! placeholder = true           # a stand-in: linted, but never a golden
//! ```

use std::collections::BTreeMap;

use serde::Deserialize;

/// Loudness and length class (docs/audio-design.md, section 3).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Class {
    /// 1-up, power-ups, hurt, brick, boss hit: peak −6 dBFS.
    Loud,
    /// jump, Oblate, stomp, bump, bounce, checkpoint, START: peak −9 dBFS.
    Normal,
    /// stroke, tally tick, selfie click, footsteps: peak −14 … −16 dBFS.
    Soft,
    /// Ambience loops (nice): peak −24 … −26 dBFS.
    Ambience,
    /// Longer one-shots (`audio/jingles/`, and `.song` jingles in
    /// `audio/music/`): peak ≤ −6, RMS −17 ± 2 dBFS.
    Jingle,
    /// A looping `.song` (`audio/music/`): peak ≤ −8, RMS −20 ± 2 dBFS,
    /// 20–90 s. The default target of every loop (no `targets.toml` entry
    /// needed; one may add `placeholder` or a `peak_dbfs`).
    Song,
}

impl Class {
    pub fn name(self) -> &'static str {
        match self {
            Class::Loud => "loud",
            Class::Normal => "normal",
            Class::Soft => "soft",
            Class::Ambience => "ambience",
            Class::Jingle => "jingle",
            Class::Song => "song",
        }
    }
}

/// One sound's targets.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Target {
    pub class: Class,
    #[serde(default)]
    pub peak_dbfs: Option<i32>,
    #[serde(default)]
    pub duration_ms: Option<u32>,
    #[serde(default)]
    pub centroid_hz: Option<[u32; 2]>,
    #[serde(default)]
    pub placeholder: bool,
}

/// Every target by id.
pub type Targets = BTreeMap<String, Target>;

/// Parses a targets file; errors carry `file: message`.
pub fn parse(file: &str, text: &str) -> Result<Targets, String> {
    let targets: Targets = toml::from_str(text).map_err(|e| {
        let line = e
            .span()
            .map(|s| text[..s.start].matches('\n').count() + 1)
            .unwrap_or(1);
        format!("{file}:{line}: {}", e.message().trim())
    })?;
    for (id, t) in &targets {
        if let Some([lo, hi]) = t.centroid_hz.filter(|&[lo, hi]| lo >= hi) {
            return Err(format!(
                "{file}: [{id}] centroid_hz = [{lo}, {hi}]: the low bound must be below the high one"
            ));
        }
    }
    Ok(targets)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_targets_and_rejects_typos() {
        let t = parse(
            "t.toml",
            "[jump]\nclass = \"normal\"\nduration_ms = 150\n\n\
             [oneup]\nclass = \"loud\"\npeak_dbfs = -6\ncentroid_hz = [800, 4000]\nplaceholder = true\n",
        )
        .unwrap();
        assert_eq!(t["jump"].class, Class::Normal);
        assert_eq!(t["jump"].duration_ms, Some(150));
        assert!(!t["jump"].placeholder);
        assert_eq!(t["oneup"].centroid_hz, Some([800, 4000]));
        assert!(t["oneup"].placeholder);

        let e = parse("t.toml", "[jump]\nclass = \"normal\"\nduraton_ms = 1\n").unwrap_err();
        assert!(e.starts_with("t.toml:3: unknown field `duraton_ms`"), "{e}");
        let e = parse("t.toml", "[jump]\nclass = \"medium\"\n").unwrap_err();
        assert!(e.starts_with("t.toml:2: unknown variant `medium`"), "{e}");
        let e = parse("t.toml", "[jump]\nduration_ms = 1\n").unwrap_err();
        assert!(e.contains("missing field `class`"), "{e}");
        let e = parse(
            "t.toml",
            "[x]\nclass = \"soft\"\ncentroid_hz = [900, 100]\n",
        )
        .unwrap_err();
        assert!(e.contains("low bound"), "{e}");
    }
}
