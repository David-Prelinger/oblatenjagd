//! `jnr-assets`: all art, levels and text of Oblatenjagd, embedded at build time.
//!
//! `build.rs` uses `jnr-artkit` to compile `art/` into packed atlases and
//! embeds levels and text as strings, so the game needs no file access at
//! runtime (important for wasm). Generated code is never committed.

include!(concat!(env!("OUT_DIR"), "/assets.rs"));

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_script_ran_with_artkit() {
        assert!(COMPILED_EXTENSIONS.contains(&"sprite"));
        assert!(COMPILED_EXTENSIONS.contains(&"palette"));
    }
}
