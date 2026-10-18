//! `jnr-artkit`: the text-based pixel-art pipeline of Oblatenjagd.
//!
//! Parses `.palette` and `.sprite` sources, runs the procedural
//! generators (`art/generators/*.toml`), compiles everything to RGBA frames,
//! packs atlases and runs art lints. It is used at build time by
//! `jnr-assets/build.rs` and by `cargo xtask sprites ...`.
//! See `docs/art-pipeline.md`.
//!
//! Modules:
//! - [`palette`], [`sprite`]: parsers (syntax errors as [`Diagnostic`]s)
//! - [`lint`]: table-driven art checks (errors and warnings)
//! - [`compile`]: sprite → RGBA frames (+ palette-swap variants)
//! - [`atlas`]: runtime atlas types (used by `jnr-assets`/`jnr-render`) and the packer
//! - [`codegen`]: atlas metadata → Rust source for `jnr-assets`
//! - [`generators`]: procedural backgrounds (`sky`, `mountains`, `skyline`) → sprite sources
//! - [`project`]: loads the whole `art/` tree from disk
//!
//! Feature `png` adds [`Image::encode_png`] (xtask only); the rest depends
//! only on `serde` + `toml` (generator params) and builds for wasm.

pub mod atlas;
pub mod codegen;
pub mod compile;
pub mod diag;
pub mod generators;
pub mod image;
pub mod lint;
pub mod palette;
pub mod project;
pub mod sprite;

pub use atlas::{Atlas, AtlasAnim, AtlasSprite, Point, Rect};
pub use compile::{CompiledAnim, CompiledSprite};
pub use diag::{Diagnostic, Severity};
pub use image::Image;
pub use palette::Palette;
pub use project::Project;
pub use sprite::SpriteSource;

/// File extensions of the art sources this crate understands.
pub const SOURCE_EXTENSIONS: [&str; 2] = ["palette", "sprite"];

/// Returns true if `path` names an art source file handled by artkit.
pub fn is_art_source(path: &str) -> bool {
    path.rsplit_once('.')
        .is_some_and(|(_, ext)| SOURCE_EXTENSIONS.contains(&ext))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognises_art_sources() {
        assert!(is_art_source("art/palettes/main.palette"));
        assert!(is_art_source("art/sprites/resi/idle.sprite"));
        assert!(!is_art_source("content/levels/01_salzburg.level"));
        assert!(!is_art_source("sprite"));
    }
}
