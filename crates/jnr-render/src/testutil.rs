//! Small hand-built atlases for the renderer tests.

use jnr_artkit::atlas::build_atlas;
use jnr_artkit::{Atlas, CompiledAnim, CompiledSprite, Image, Point, Rect};

/// Test colours (`0xRRGGBBAA`) for the grid characters used in test images.
pub const A: u32 = 0x1111_11ff;
pub const B: u32 = 0x2222_22ff;
pub const C: u32 = 0x3333_33ff;
/// Background used to see untouched pixels.
pub const BG: u32 = 0x0000_00ff;

/// An image from text rows: `.` transparent, `a`/`b`/`c` the colours above.
pub fn image(rows: &[&str]) -> Image {
    let h = rows.len() as u32;
    let w = rows.first().map_or(0, |r| r.chars().count()) as u32;
    let mut img = Image::new(w, h);
    for (y, row) in rows.iter().enumerate() {
        for (x, ch) in row.chars().enumerate() {
            let rgba = match ch {
                'a' => A,
                'b' => B,
                'c' => C,
                _ => continue,
            };
            img.set(x as u32, y as u32, rgba.to_be_bytes());
        }
    }
    img
}

/// A sprite with one anim per `(name, ticks, frames)`; all frames share the size of the first.
pub fn sprite(
    name: &str,
    origin: (u32, u32),
    mirror: bool,
    anims: Vec<(&str, u32, Vec<Image>)>,
) -> CompiledSprite {
    let first = &anims[0].2[0];
    let (width, height) = (first.width, first.height);
    CompiledSprite {
        name: name.into(),
        width,
        height,
        origin: Point {
            x: origin.0,
            y: origin.1,
        },
        hitbox: Rect {
            x: 0,
            y: 0,
            w: width,
            h: height,
        },
        mirror,
        anims: anims
            .into_iter()
            .map(|(name, ticks, frames)| CompiledAnim {
                name: name.into(),
                ticks,
                looping: true,
                frames,
            })
            .collect(),
    }
}

/// Packs the sprites into an atlas.
pub fn atlas(sprites: &[CompiledSprite]) -> Atlas {
    build_atlas(sprites)
}

/// A framebuffer cleared to [`BG`].
pub fn fb() -> crate::Framebuffer {
    let mut fb = crate::Framebuffer::new();
    fb.clear(BG);
    fb
}
