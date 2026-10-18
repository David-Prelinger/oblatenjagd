//! `jnr-artkit`: the text-based pixel-art pipeline of Oblatenjagd.
//!
//! Parses `.palette` and `.sprite` sources, runs procedural generators,
//! packs atlases and runs art lints. It is used at build time by
//! `jnr-assets/build.rs` and by `cargo xtask sprites ...`.
//! See `docs/art-pipeline.md`.

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
