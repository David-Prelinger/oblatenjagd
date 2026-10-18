//! `jnr-assets`: all art, levels and text of Oblatenjagd, embedded at build time.
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

static ATLAS_RGBA: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/atlas.rgba"));

/// The sprite atlas with every sprite from `art/sprites/` (built on first use).
pub fn atlas() -> &'static Atlas {
    static ATLAS: OnceLock<Atlas> = OnceLock::new();
    ATLAS.get_or_init(|| generated_atlas(ATLAS_RGBA))
}

/// Looks up a sprite (or palette-swap variant) by name.
pub fn sprite(name: &str) -> Option<&'static AtlasSprite> {
    atlas().sprite(name)
}

#[cfg(test)]
mod tests {
    use super::*;

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
