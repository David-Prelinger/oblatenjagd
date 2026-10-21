//! `jnr-audio`: the game's sound, made from text (see `docs/architecture.md`,
//! section "jnr-audio").
//!
//! - [`patch`]: the `.sfx` (one sound effect) and `.inst` (a note-driven
//!   instrument or a drum kit) formats, parsed with `file:line:col` errors.
//! - [`synth`]: integer oscillators (pulse with duty, triangle, saw, sine
//!   from a table, LFSR noise), envelopes, pitch slide, vibrato, arpeggio,
//!   one-pole low/high-pass, volume.
//! - [`render`]: patches to 44 100 Hz mono `i16` [`Pcm`], with clamp counting.
//! - [`wav`]: RIFF/WAVE bytes from and to [`Pcm`].
<<<<<<< HEAD
//! - [`cues`]: game events and state to sound cues (the [`cues::Director`]).
=======
//! - [`song`]: the `.song` music format (TOML header + tracker-style
//!   scores), [`song_render`]: songs to PCM (loop alignment, wrapped tails,
//!   a voice cache), [`music_lint`]: what can be checked from the text,
//!   [`quote`]: melody fixtures of quoted tunes.
>>>>>>> ma-audio
//!
//! The crate follows the `jnr-core` determinism rules (integers only, no
//! `HashMap` iteration, no time/fs/threads/global RNG; enforced by
//! `cargo xtask arch-lint`), so a render is bit-identical on native and wasm
//! and its hash can serve as a golden.

pub mod cues;
pub mod error;
pub mod music_lint;
pub mod note;
pub mod patch;
pub mod quote;
pub mod render;
pub mod song;
pub mod song_render;
pub mod synth;
pub mod tables;
pub mod wav;

pub use error::{ErrorKind, ParseError};
pub use music_lint::{Lint, LintContext, lint_song};
pub use patch::{Instrument, InstrumentKind, Layer, Sfx, Wave};
pub use quote::Quote;
pub use render::{Pcm, Render, render_note, render_sfx};
pub use song::Song;
pub use song_render::{RenderOptions, SongRender, render_song};

/// Output sample rate (Hz).
pub const SAMPLE_RATE: u32 = 44_100;
/// Samples per 60 Hz game tick (44 100 / 60, exact).
pub const FRAMES_PER_TICK: u32 = 735;

/// Milliseconds to samples, rounded down (`ms · 44.1`).
pub fn ms_to_samples(ms: u32) -> u32 {
    (ms as u64 * SAMPLE_RATE as u64 / 1000) as u32
}
