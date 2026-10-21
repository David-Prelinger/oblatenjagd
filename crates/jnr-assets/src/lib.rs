//! `jnr-assets`: all art, levels, text and sound sources of Oblatenjagd,
//! embedded at build time.
//!
//! `build.rs` uses `jnr-artkit` to compile `art/` into one packed atlas
//! (raw RGBA + generated metadata) and embeds it, so the game needs no file
//! access at runtime (important for wasm). Generated code is never committed.
//!
//! ```ignore
//! let box_ = jnr_assets::sprite("test_box").unwrap();
//! let rect = box_.anim("idle").unwrap().frame(0, false);
//! let rgba = jnr_assets::atlas().pixel(rect.x, rect.y);
//! ```

use std::sync::OnceLock;

pub use jnr_artkit::atlas::{Atlas, AtlasAnim, AtlasSprite, Point, Rect};

include!(concat!(env!("OUT_DIR"), "/assets.rs"));
include!(concat!(env!("OUT_DIR"), "/atlas.rs"));
include!(concat!(env!("OUT_DIR"), "/audio.rs"));

static ATLAS_RGBA: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/atlas.rgba"));

/// The sprite atlas with every sprite from `art/sprites/` (built on first use).
pub fn atlas() -> &'static Atlas {
    static ATLAS: OnceLock<Atlas> = OnceLock::new();
    ATLAS.get_or_init(|| generated_atlas(ATLAS_RGBA))
}

/// Every sound source under `audio/` as `(path, text)` (the build script
/// already checked them with `jnr_audio::bank::Bank::load`).
pub fn audio_sources() -> &'static [(&'static str, &'static str)] {
    AUDIO_SOURCES
}

/// The sound bank, parsed from [`audio_sources`] on first use (a few ms).
pub fn audio_bank() -> &'static jnr_audio::bank::Bank {
    static BANK: OnceLock<jnr_audio::bank::Bank> = OnceLock::new();
    BANK.get_or_init(|| {
        jnr_audio::bank::Bank::load(AUDIO_SOURCES)
            .expect("the build script checked the audio sources")
    })
}

/// Looks up a sprite (or palette-swap variant) by name.
pub fn sprite(name: &str) -> Option<&'static AtlasSprite> {
    atlas().sprite(name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use jnr_audio::cues::{Music, PENDING_MUSIC, PITCHED, Sfx};

    /// Every SFX the cues can play is in the embedded bank (no pending
    /// SFX), and so is every pitched variant's id.
    #[test]
    fn every_cue_sfx_is_embedded() {
        let bank = audio_bank();
        for s in Sfx::ALL {
            assert!(
                bank.sfx.contains_key(s.id()),
                "audio/sfx/{}.sfx is missing",
                s.id()
            );
        }
        let mut ids: Vec<&str> = bank.sfx.keys().map(String::as_str).collect();
        ids.sort_unstable();
        let mut ours: Vec<&str> = Sfx::ALL.iter().map(|s| s.id()).collect();
        ours.sort_unstable();
        assert_eq!(ids, ours, "audio/sfx has exactly the cues' ids");
        assert_eq!(bank.startup_sfx().len(), Sfx::ALL.len() + PITCHED.len());
    }

    /// Every music cue exists as a song or jingle, or is pending; a pending
    /// one that exists must leave `PENDING_MUSIC` (so the list only shrinks).
    #[test]
    fn every_music_cue_is_embedded_or_pending() {
        let bank = audio_bank();
        for m in Music::ALL {
            let pending = PENDING_MUSIC.contains(&m);
            assert!(
                bank.has_music(m) != pending,
                "`{}`: {}",
                m.id(),
                if pending {
                    "exists now: remove it from jnr_audio::cues::PENDING_MUSIC"
                } else {
                    "no audio/music/<id>.song or audio/jingles/<id>.sfx (and not pending)"
                }
            );
        }
    }

    #[test]
    fn embedded_audio_sources_are_sorted_sources() {
        let src = audio_sources();
        assert!(src.windows(2).all(|w| w[0].0 < w[1].0));
        assert!(src.iter().all(|(p, _)| p.starts_with("audio/")));
        assert!(src.iter().any(|(p, _)| *p == "audio/sfx/jump.sfx"));
        assert!(
            !src.iter()
                .any(|(p, _)| p.ends_with(".md") || p.ends_with(".toml"))
        );
    }

    #[test]
    fn build_script_ran_with_artkit() {
        assert!(COMPILED_EXTENSIONS.contains(&"sprite"));
        assert!(COMPILED_EXTENSIONS.contains(&"palette"));
    }

    #[test]
    fn embedded_atlas_is_consistent() {
        let a = atlas();
        assert_eq!(a.pixels.len(), (a.width * a.height * 4) as usize);
        assert!(a.sprites.windows(2).all(|w| w[0].name < w[1].name));
        for s in &a.sprites {
            for anim in &s.anims {
                assert!(!anim.frames.is_empty(), "{}.{}", s.name, anim.name);
                for r in anim.frames.iter().chain(&anim.mirrored_frames) {
                    assert_eq!((r.w, r.h), (s.width, s.height));
                    assert!(r.x + r.w <= a.width && r.y + r.h <= a.height);
                }
                let mirrored = if s.mirrored { anim.frames.len() } else { 0 };
                assert_eq!(anim.mirrored_frames.len(), mirrored);
            }
        }
    }

    #[test]
    fn test_box_fixture_is_embedded_and_mirrored() {
        let s = sprite("test_box").expect("art/sprites/test/test_box.sprite");
        assert_eq!((s.width, s.height), (16, 24));
        assert_eq!(s.origin, Point { x: 8, y: 24 });
        assert_eq!(
            s.hitbox,
            Rect {
                x: 3,
                y: 4,
                w: 10,
                h: 20
            }
        );
        let idle = s.anim("idle").unwrap();
        assert_eq!((idle.ticks, idle.looping, idle.frames.len()), (30, true, 2));
        // The left-facing frame is the right-facing one flipped.
        let a = atlas();
        let right = a.region(idle.frame(0, false));
        let left = a.region(idle.frame(0, true));
        assert_eq!(left, right.flipped_h());
        assert_ne!(left, right);
        // Outline colour `k` (#1a1c2c) at the box's top-left corner.
        assert_eq!(right.get(3, 4), [0x1a, 0x1c, 0x2c, 255]);
        assert_eq!(right.get(0, 0), [0, 0, 0, 0]);
    }

    #[test]
    fn coin_variant_is_palette_swapped() {
        let base = sprite("test_coin").unwrap();
        let gold = sprite("test_coin_gold").unwrap();
        let a = atlas();
        let b = a.region(base.anims[0].frames[0]);
        let g = a.region(gold.anims[0].frames[0]);
        // (3,3) is `m` stone_grey in the base and `o` oblate_gold in the variant.
        assert_eq!(b.get(3, 3), [0x5d, 0x66, 0x80, 255]);
        assert_eq!(g.get(3, 3), [0xd9, 0x91, 0x3a, 255]);
        // The outline is not remapped.
        assert_eq!(b.get(0, 2), g.get(0, 2));
        assert!(!gold.mirrored);
    }
}
