//! Start-up options and the per-frame decisions derived from them.
//!
//! Pure and platform-independent: native builds fill [`Options`] from the
//! command line (`cli.rs`), wasm always uses [`Options::default`].

use std::path::{Path, PathBuf};

/// The levels embedded in the binary (so wasm needs no file access), as
/// `(id, source)`. The id must equal the level's `[meta] id`.
/// To add a level: append a line here.
pub const LEVELS: &[(&str, &str)] = &[(
    "testbed",
    include_str!("../../../content/levels/00_testbed.level"),
)];

/// The level played when none is given.
pub const DEFAULT_LEVEL: &str = "testbed";

/// The `.level` source of the embedded level `id`.
pub fn level_source(id: &str) -> Option<&'static str> {
    LEVELS
        .iter()
        .find(|(level_id, _)| *level_id == id)
        .map(|(_, src)| *src)
}

/// Everything the app needs to know at start-up.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Options {
    /// Id of an embedded level (see [`LEVELS`]).
    pub level: String,
    /// Seed of the game RNG.
    pub seed: u64,
    /// Initial window size = 320*scale x 180*scale.
    pub scale: u32,
    /// Quit after this many simulation frames.
    pub exit_after: Option<u32>,
    /// `(frame, path)`: write the framebuffer as PNG after that simulation frame.
    pub screenshots: Vec<(u32, PathBuf)>,
}

impl Default for Options {
    fn default() -> Options {
        Options {
            level: DEFAULT_LEVEL.to_owned(),
            seed: 1,
            scale: 4,
            exit_after: None,
            screenshots: Vec::new(),
        }
    }
}

/// What the app has to do right after simulation frame `frame`
/// (frame 0 = the initial state, before the first step).
#[derive(Debug, PartialEq, Eq)]
pub struct FrameActions<'a> {
    /// Write the framebuffer of this frame to these files.
    pub screenshots: Vec<&'a Path>,
    /// Quit (after taking the screenshots).
    pub exit: bool,
}

impl Options {
    /// The actions due after simulation frame `frame`.
    pub fn actions_after(&self, frame: u32) -> FrameActions<'_> {
        FrameActions {
            screenshots: self
                .screenshots
                .iter()
                .filter(|(at, _)| *at == frame)
                .map(|(_, path)| path.as_path())
                .collect(),
            exit: self.exit_after.is_some_and(|n| frame >= n),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_level_ids_match_their_meta() {
        for (id, src) in LEVELS {
            let level = jnr_core::Level::parse(src).unwrap_or_else(|e| panic!("{id}: {e}"));
            assert_eq!(level.meta.id, *id);
        }
        assert!(level_source(DEFAULT_LEVEL).is_some());
        assert!(level_source("atlantis").is_none());
    }

    #[test]
    fn default_options_never_act() {
        let opts = Options::default();
        for frame in [0, 1, 60, u32::MAX] {
            let actions = opts.actions_after(frame);
            assert!(actions.screenshots.is_empty());
            assert!(!actions.exit);
        }
    }

    #[test]
    fn screenshots_and_exit_fire_on_their_frame() {
        let opts = Options {
            exit_after: Some(120),
            screenshots: vec![
                (60, PathBuf::from("a.png")),
                (0, PathBuf::from("start.png")),
                (60, PathBuf::from("b.png")),
            ],
            ..Options::default()
        };
        assert_eq!(
            opts.actions_after(0),
            FrameActions {
                screenshots: vec![Path::new("start.png")],
                exit: false
            }
        );
        assert!(opts.actions_after(59).screenshots.is_empty());
        assert_eq!(
            opts.actions_after(60).screenshots,
            vec![Path::new("a.png"), Path::new("b.png")]
        );
        assert!(!opts.actions_after(119).exit);
        assert!(opts.actions_after(120).exit);
    }

    #[test]
    fn exit_after_zero_exits_before_the_first_step() {
        let opts = Options {
            exit_after: Some(0),
            ..Options::default()
        };
        assert!(opts.actions_after(0).exit);
    }
}
