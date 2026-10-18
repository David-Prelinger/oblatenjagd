//! `jnr-core`: the deterministic game simulation of Oblatenjagd.
//!
//! This crate owns all game state and advances it in fixed 1/60 s ticks.
//! It is pure: no macroquad, no floats in simulation state or logic
//! (fixed-point only), no `HashMap`/`HashSet`, no time, filesystem, threads
//! or global RNG. These rules are checked by the architecture lint in
//! `cargo xtask check`. See `docs/architecture.md`.
//!
//! Modules:
//! - [`fx`]: fixed-point numbers (1/256 px)
//! - [`input`]: per-frame input bitflags and replay tokens
//! - [`tuning`], [`level`]: content parsing (`tuning.toml`, `.level`)
//! - [`physics`]: boxes and tile collision
//! - [`player`]: run/jump physics, power states
//! - [`entity`]: items, particles, popups, props, enemies (`Vec<Entity>`)
//! - [`enemies`]: enemy patterns, thrown Mozartkugeln, defeated enemies
//! - [`platform`]: the Festungsbahn car (moving one-way platform)
//! - [`camera`]: SMB-style camera
//! - [`game`]: the state machine, `Game::step`, `Game::state_hash`
//! - [`text`]: the keyed text table (`content/text/de.toml`)
//! - [`replay`]: replay files (parse, format, run, check expectations)
//! - [`route`]: the stations of the journey, the [`Route`] of levels a run
//!   plays and the route-map state

use std::fmt;

pub mod camera;
pub mod enemies;
pub mod entity;
pub mod fx;
pub mod game;
pub mod hash;
pub mod input;
pub mod level;
pub mod physics;
pub mod platform;
pub mod player;
pub mod replay;
pub mod rng;
pub mod route;
pub mod text;
pub mod tuning;

#[cfg(test)]
mod tests;

pub use camera::{Camera, SCREEN_H, SCREEN_W};
pub use enemies::Pose;
pub use entity::{EnemyKind, Entity, EntityKind, Popup};
pub use fx::Fx;
pub use game::{ClearPhase, DeathCause, Event, Game, Mode, StageClear, Start};
pub use input::InputFrame;
pub use level::{
    Block, Contents, EntitySpawn, Level, PROP_SPRITES, PropLayer, SignStyle, SpawnKind, Tile,
    TileKind,
};
pub use player::{Facing, Player, Power};
pub use replay::Replay;
pub use route::{MapPhase, Route, RouteMap, STATIONS, Station};
pub use text::Texts;
pub use tuning::Tuning;

/// Simulation ticks per second. `Game::step` advances exactly one tick.
pub const TICKS_PER_SECOND: u32 = 60;

/// A content or replay-token parse error with a human-readable message.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseError {
    message: String,
}

impl ParseError {
    /// An error with this message.
    pub fn new(message: impl Into<String>) -> ParseError {
        ParseError {
            message: message.into(),
        }
    }

    /// The message.
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for ParseError {}
