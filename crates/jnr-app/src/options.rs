//! Start-up options and the per-frame decisions derived from them.
//!
//! Pure and platform-independent: native builds fill [`Options`] from the
//! command line (`cli.rs`), wasm always uses [`Options::default`].

use std::path::{Path, PathBuf};

use jnr_core::{InputFrame, Level, Start};

/// The levels embedded in the binary (so wasm needs no file access), as
/// `(id, source)`: every file in `content/levels/`, in file name order.
/// The id must equal the level's `[meta] id`. To add a level: append a line
/// here (a test checks that this list matches the directory). Levels whose
/// id is a station (`st_gilgen`, …) join the journey on their own: the run
/// follows `jnr_core::route::Route::standard` over all of these (D-047).
pub const LEVELS: &[(&str, &str)] = &[
    (
        "climb_test",
        include_str!("../../../content/levels/00_climb_test.level"),
    ),
    (
        "enemies",
        include_str!("../../../content/levels/00_enemies.level"),
    ),
    (
        "hackl",
        include_str!("../../../content/levels/00_hackl.level"),
    ),
    (
        "items",
        include_str!("../../../content/levels/00_items.level"),
    ),
    (
        "m2_enemies",
        include_str!("../../../content/levels/00_m2_enemies.level"),
    ),
    (
        "m3_enemies",
        include_str!("../../../content/levels/00_m3_enemies.level"),
    ),
    (
        "m4_bus",
        include_str!("../../../content/levels/00_m4_bus.level"),
    ),
    (
        "m4_enemies",
        include_str!("../../../content/levels/00_m4_enemies.level"),
    ),
    (
        "m5_autoscroll",
        include_str!("../../../content/levels/00_m5_autoscroll.level"),
    ),
    (
        "m5_boats",
        include_str!("../../../content/levels/00_m5_boats.level"),
    ),
    (
        "m5_enemies",
        include_str!("../../../content/levels/00_m5_enemies.level"),
    ),
    (
        "m5_lake",
        include_str!("../../../content/levels/00_m5_lake.level"),
    ),
    (
        "m5_swim",
        include_str!("../../../content/levels/00_m5_swim.level"),
    ),
    (
        "m6_traffic",
        include_str!("../../../content/levels/00_m6_traffic.level"),
    ),
    (
        "route_a",
        include_str!("../../../content/levels/00_route_a.level"),
    ),
    (
        "route_b",
        include_str!("../../../content/levels/00_route_b.level"),
    ),
    (
        "schafbergbahn",
        include_str!("../../../content/levels/00_schafbergbahn.level"),
    ),
    (
        "testbed",
        include_str!("../../../content/levels/00_testbed.level"),
    ),
    (
        "salzburg",
        include_str!("../../../content/levels/01_salzburg.level"),
    ),
    (
        "st_gilgen",
        include_str!("../../../content/levels/02_st_gilgen.level"),
    ),
    (
        "brunnwinkl",
        include_str!("../../../content/levels/03_brunnwinkl.level"),
    ),
    (
        "falkenstein",
        include_str!("../../../content/levels/04_falkenstein.level"),
    ),
    (
        "st_wolfgang",
        include_str!("../../../content/levels/05_st_wolfgang.level"),
    ),
    (
        "wolfgangsee",
        include_str!("../../../content/levels/06_wolfgangsee.level"),
    ),
];

/// The level played when none is given: Stage 1.
pub const DEFAULT_LEVEL: &str = "salzburg";

/// Every embedded level, parsed (panics on a broken embedded level: the
/// tests parse them all).
pub fn embedded_levels() -> Vec<Level> {
    LEVELS
        .iter()
        .map(|(id, src)| Level::parse(src).unwrap_or_else(|e| panic!("level {id}: {e}")))
        .collect()
}

/// The `.level` source of the embedded level `id`.
#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
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
    /// Where the inputs come from (keyboard, or a replay via `--replay`).
    pub input: InputSource,
    /// `--record`: write the played inputs as a replay to this file on exit.
    pub record: Option<PathBuf>,
    /// How the game starts: the title screen, unless a `--replay` says
    /// otherwise (its `start`, default `playing`, D-025).
    pub start: Start,
    /// The level ids of the run, from a `--replay`'s `route` header (empty:
    /// the standard route if `level` is on it, else `level` alone; D-047).
    pub route: Vec<String>,
}

/// Where the per-frame input comes from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InputSource {
    /// The keys held right now.
    Keyboard,
    /// One input per step, from a replay file; after the last one, nothing
    /// is held. Game input comes only from the replay (Escape quits; F1/F12
    /// still work).
    /// Native only (`--replay`); wasm has no command line.
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    Replay(Vec<InputFrame>),
}

impl InputSource {
    /// The input for the step that turns frame `frame` into `frame + 1`;
    /// `keyboard` is what the keyboard holds right now.
    pub fn input_for(&self, frame: u32, keyboard: InputFrame) -> InputFrame {
        match self {
            InputSource::Keyboard => keyboard,
            InputSource::Replay(frames) => frames
                .get(frame as usize)
                .copied()
                .unwrap_or(InputFrame::NONE),
        }
    }
}

impl Default for Options {
    fn default() -> Options {
        Options {
            level: DEFAULT_LEVEL.to_owned(),
            seed: 1,
            scale: 4,
            exit_after: None,
            screenshots: Vec::new(),
            input: InputSource::Keyboard,
            record: None,
            start: Start::Title,
            route: Vec::new(),
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
    fn embedded_levels_match_the_content_directory() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../content/levels");
        let mut files: Vec<String> = std::fs::read_dir(&dir)
            .expect("content/levels")
            .map(|e| e.expect("dir entry").path())
            .filter(|p| p.extension().is_some_and(|ext| ext == "level"))
            .map(|p| std::fs::read_to_string(p).expect("level file"))
            .collect();
        files.sort();
        let mut embedded: Vec<String> = LEVELS.iter().map(|(_, s)| (*s).to_owned()).collect();
        embedded.sort();
        assert_eq!(
            files.len(),
            embedded.len(),
            "every content/levels/*.level must be in LEVELS (and nothing else)"
        );
        assert!(
            files == embedded,
            "LEVELS does not embed exactly the files in content/levels/"
        );
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
    fn keyboard_source_passes_the_keys_through() {
        let src = InputSource::Keyboard;
        assert_eq!(src.input_for(0, InputFrame::JUMP), InputFrame::JUMP);
        assert_eq!(src.input_for(999, InputFrame::NONE), InputFrame::NONE);
    }

    #[test]
    fn replay_source_ignores_the_keyboard_and_ends_empty() {
        let src = InputSource::Replay(vec![InputFrame::RIGHT, InputFrame::JUMP]);
        assert_eq!(src.input_for(0, InputFrame::LEFT), InputFrame::RIGHT);
        assert_eq!(src.input_for(1, InputFrame::LEFT), InputFrame::JUMP);
        assert_eq!(src.input_for(2, InputFrame::LEFT), InputFrame::NONE);
        assert_eq!(src.input_for(u32::MAX, InputFrame::LEFT), InputFrame::NONE);
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
