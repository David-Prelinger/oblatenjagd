//! Runtime atlas data types plus the shelf packer that builds them.
//!
//! The types here are plain data (no I/O) and are what `jnr-assets` embeds
//! and `jnr-render` draws from. Pixels are RGBA8, row-major.

use std::borrow::Cow;

use crate::compile::CompiledSprite;
use crate::image::Image;

/// Padding in pixels around every packed frame.
pub const PADDING: u32 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Hash)]
pub struct Point {
    pub x: u32,
    pub y: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Hash)]
pub struct Rect {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

impl Rect {
    pub fn overlaps(&self, o: &Rect) -> bool {
        self.x < o.x + o.w && o.x < self.x + self.w && self.y < o.y + o.h && o.y < self.y + self.h
    }
}

/// All sprites packed into one RGBA image.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Atlas {
    pub width: u32,
    pub height: u32,
    /// RGBA8, `width * height * 4` bytes.
    /// Borrowed from `include_bytes!` in `jnr-assets` (no copy), owned at build time.
    pub pixels: Cow<'static, [u8]>,
    /// Sorted by name (use [`Atlas::sprite`]).
    pub sprites: Vec<AtlasSprite>,
}

/// One sprite (or palette-swap variant) in the atlas.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AtlasSprite {
    pub name: String,
    pub width: u32,
    pub height: u32,
    /// Anchor in sprite pixels for the right-facing frames (e.g. bottom centre).
    pub origin: Point,
    /// Hitbox in sprite pixels for the right-facing frames.
    pub hitbox: Rect,
    /// True if left-facing copies exist (`mirror = auto`).
    pub mirrored: bool,
    /// In declaration order.
    pub anims: Vec<AtlasAnim>,
}

/// One animation of a sprite.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AtlasAnim {
    pub name: String,
    /// Ticks per frame at 60 Hz; 0 = held single frame.
    pub ticks: u32,
    pub looping: bool,
    /// Right-facing frames (as authored), as atlas rects.
    pub frames: Vec<Rect>,
    /// Left-facing (horizontally flipped) frames; empty unless the sprite is mirrored.
    pub mirrored_frames: Vec<Rect>,
}

impl Atlas {
    /// Looks up a sprite by name (binary search; `sprites` is sorted).
    pub fn sprite(&self, name: &str) -> Option<&AtlasSprite> {
        self.sprites
            .binary_search_by(|s| s.name.as_str().cmp(name))
            .ok()
            .map(|i| &self.sprites[i])
    }

    /// RGBA of one atlas pixel.
    pub fn pixel(&self, x: u32, y: u32) -> [u8; 4] {
        let i = ((y * self.width + x) * 4) as usize;
        [
            self.pixels[i],
            self.pixels[i + 1],
            self.pixels[i + 2],
            self.pixels[i + 3],
        ]
    }

    /// RGBA bytes of row `y` (relative to `r`) of an atlas region:
    /// `r.w * 4` bytes, for slice-based blitting.
    pub fn row(&self, r: Rect, y: u32) -> &[u8] {
        let start = (((r.y + y) * self.width + r.x) * 4) as usize;
        &self.pixels[start..start + (r.w * 4) as usize]
    }

    /// Copies one atlas region into a standalone image.
    pub fn region(&self, r: Rect) -> Image {
        let mut img = Image::new(r.w, r.h);
        for y in 0..r.h {
            for x in 0..r.w {
                img.set(x, y, self.pixel(r.x + x, r.y + y));
            }
        }
        img
    }
}

impl AtlasSprite {
    pub fn anim(&self, name: &str) -> Option<&AtlasAnim> {
        self.anims.iter().find(|a| a.name == name)
    }

    /// Origin for the given facing (left-facing mirrors around the width).
    pub fn origin_facing(&self, left: bool) -> Point {
        if left {
            Point {
                x: self.width - self.origin.x,
                y: self.origin.y,
            }
        } else {
            self.origin
        }
    }

    /// Hitbox for the given facing. The `bounds` lint guarantees the hitbox
    /// lies inside the sprite, so this cannot underflow.
    pub fn hitbox_facing(&self, left: bool) -> Rect {
        if left {
            Rect {
                x: self.width - self.hitbox.x - self.hitbox.w,
                ..self.hitbox
            }
        } else {
            self.hitbox
        }
    }
}

impl AtlasAnim {
    /// Frame index shown `tick` ticks after the animation started.
    pub fn frame_index_at(&self, tick: u32) -> usize {
        let n = self.frames.len();
        if self.ticks == 0 || n <= 1 {
            return 0;
        }
        let i = (tick / self.ticks) as usize;
        if self.looping { i % n } else { i.min(n - 1) }
    }

    /// Atlas rect of frame `index` for the given facing. For sprites without
    /// `mirror = auto` there are no flipped copies: `left = true` falls back
    /// to the unflipped (authored) frame.
    pub fn frame(&self, index: usize, left: bool) -> Rect {
        if left && !self.mirrored_frames.is_empty() {
            self.mirrored_frames[index]
        } else {
            self.frames[index]
        }
    }
}

/// Shelf packer: places `sizes` (w, h) with [`PADDING`] between rects and
/// around the border. Returns (atlas width, atlas height, rect per input).
/// Deterministic: tall rects first, ties keep input order.
pub fn pack(sizes: &[(u32, u32)]) -> (u32, u32, Vec<Rect>) {
    let area: u64 = sizes
        .iter()
        .map(|&(w, h)| u64::from(w + PADDING) * u64::from(h + PADDING))
        .sum();
    let widest = sizes.iter().map(|s| s.0).max().unwrap_or(0) + 2 * PADDING;
    let mut width = 64;
    while u64::from(width) * u64::from(width) < area || width < widest {
        width *= 2;
    }

    let mut order: Vec<usize> = (0..sizes.len()).collect();
    order.sort_by_key(|&i| std::cmp::Reverse(sizes[i].1)); // stable

    let mut rects = vec![Rect::default(); sizes.len()];
    let (mut x, mut y, mut shelf_h) = (PADDING, PADDING, 0);
    for i in order {
        let (w, h) = sizes[i];
        if x + w + PADDING > width {
            x = PADDING;
            y += shelf_h + PADDING;
            shelf_h = 0;
        }
        rects[i] = Rect { x, y, w, h };
        x += w + PADDING;
        shelf_h = shelf_h.max(h);
    }
    let height = if sizes.is_empty() {
        1
    } else {
        y + shelf_h + PADDING
    };
    (width, height, rects)
}

/// Packs compiled sprites (all frames, plus mirrored copies) into one atlas.
/// Output is sorted by sprite name and independent of input order.
pub fn build_atlas(sprites: &[CompiledSprite]) -> Atlas {
    let mut sorted: Vec<&CompiledSprite> = sprites.iter().collect();
    sorted.sort_by(|a, b| a.name.cmp(&b.name));

    // Collect images in a fixed order: sprite -> anim -> frames, then mirrored.
    let mut images: Vec<Image> = Vec::new();
    for s in &sorted {
        for a in &s.anims {
            images.extend(a.frames.iter().cloned());
            if s.mirror {
                images.extend(a.frames.iter().map(Image::flipped_h));
            }
        }
    }
    let sizes: Vec<(u32, u32)> = images.iter().map(|i| (i.width, i.height)).collect();
    let (width, height, rects) = pack(&sizes);

    let mut sheet = Image::new(width, height);
    for (img, r) in images.iter().zip(&rects) {
        sheet.blit(img, r.x, r.y);
    }

    let mut next = rects.into_iter();
    let out = sorted
        .iter()
        .map(|s| AtlasSprite {
            name: s.name.clone(),
            width: s.width,
            height: s.height,
            origin: s.origin,
            hitbox: s.hitbox,
            mirrored: s.mirror,
            anims: s
                .anims
                .iter()
                .map(|a| {
                    let n = a.frames.len();
                    let frames: Vec<Rect> = next.by_ref().take(n).collect();
                    let mirrored_frames: Vec<Rect> = if s.mirror {
                        next.by_ref().take(n).collect()
                    } else {
                        Vec::new()
                    };
                    AtlasAnim {
                        name: a.name.clone(),
                        ticks: a.ticks,
                        looping: a.looping,
                        frames,
                        mirrored_frames,
                    }
                })
                .collect(),
        })
        .collect();

    Atlas {
        width,
        height,
        pixels: Cow::Owned(sheet.rgba),
        sprites: out,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pack_has_no_overlaps_and_respects_padding() {
        let sizes: Vec<(u32, u32)> = (0..40u32).map(|i| (3 + i % 13, 2 + (i * 7) % 17)).collect();
        let (w, h, rects) = pack(&sizes);
        for (i, a) in rects.iter().enumerate() {
            assert_eq!((a.w, a.h), sizes[i]);
            assert!(a.x >= PADDING && a.y >= PADDING);
            assert!(a.x + a.w + PADDING <= w && a.y + a.h + PADDING <= h);
            for b in &rects[i + 1..] {
                assert!(!a.overlaps(b), "{a:?} overlaps {b:?}");
            }
        }
        // Every pair has at least one empty pixel column or row between them.
        for (i, a) in rects.iter().enumerate() {
            for b in &rects[i + 1..] {
                let gap_x = b.x >= a.x + a.w + PADDING || a.x >= b.x + b.w + PADDING;
                let gap_y = b.y >= a.y + a.h + PADDING || a.y >= b.y + b.h + PADDING;
                assert!(gap_x || gap_y, "{a:?} and {b:?} touch");
            }
        }
    }

    #[test]
    fn pack_is_deterministic_and_handles_wide_inputs() {
        let sizes = [(300, 4), (5, 5), (5, 9)];
        assert_eq!(pack(&sizes), pack(&sizes));
        let (w, _, _) = pack(&sizes);
        assert!(w >= 302);
        assert_eq!(pack(&[]).2, Vec::<Rect>::new());
    }

    #[test]
    fn frame_index_follows_ticks() {
        let a = AtlasAnim {
            name: "walk".into(),
            ticks: 8,
            looping: true,
            frames: vec![Rect::default(); 4],
            mirrored_frames: Vec::new(),
        };
        let idx: Vec<usize> = [0, 7, 8, 31, 32]
            .iter()
            .map(|&t| a.frame_index_at(t))
            .collect();
        assert_eq!(idx, [0, 0, 1, 3, 0]);
        let once = AtlasAnim {
            looping: false,
            ..a.clone()
        };
        assert_eq!(once.frame_index_at(1000), 3);
        let held = AtlasAnim { ticks: 0, ..a };
        assert_eq!(held.frame_index_at(1000), 0);
    }

    #[test]
    fn facing_mirrors_origin_and_hitbox() {
        let s = AtlasSprite {
            name: "a".into(),
            width: 16,
            height: 24,
            origin: Point { x: 7, y: 24 },
            hitbox: Rect {
                x: 3,
                y: 8,
                w: 9,
                h: 16,
            },
            mirrored: true,
            anims: Vec::new(),
        };
        assert_eq!(s.origin_facing(false), Point { x: 7, y: 24 });
        assert_eq!(s.origin_facing(true), Point { x: 9, y: 24 });
        assert_eq!(
            s.hitbox_facing(true),
            Rect {
                x: 4,
                y: 8,
                w: 9,
                h: 16
            }
        );
    }

    fn sprite(name: &str, colour: u8, h: u32, mirror: bool) -> CompiledSprite {
        let mut img = Image::new(3, h);
        img.fill([colour, 0, 0, 255]);
        img.set(0, 0, [0, colour, 0, 255]); // asymmetric, so mirroring shows
        CompiledSprite {
            name: name.into(),
            group: "test".into(),
            width: 3,
            height: h,
            origin: Point { x: 1, y: h },
            hitbox: Rect {
                x: 0,
                y: 0,
                w: 3,
                h,
            },
            mirror,
            anims: vec![crate::compile::CompiledAnim {
                name: "idle".into(),
                ticks: 8,
                looping: true,
                frames: vec![img.clone(), img.flipped_h()],
            }],
        }
    }

    #[test]
    fn build_atlas_is_independent_of_input_order() {
        let a = vec![
            sprite("c", 10, 4, true),
            sprite("a", 20, 2, true),
            sprite("b", 31, 5, false),
        ];
        let mut b = a.clone();
        b.reverse();
        let (x, y) = (build_atlas(&a), build_atlas(&b));
        assert_eq!(x, y);
        let names: Vec<&str> = x.sprites.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, ["a", "b", "c"]);
    }

    #[test]
    fn rows_and_regions_read_packed_pixels() {
        let atlas = build_atlas(&[sprite("a", 20, 2, true)]);
        let anim = &atlas.sprite("a").unwrap().anims[0];
        let r = anim.frame(0, false);
        assert_eq!(
            atlas.row(r, 0),
            &[0, 20, 0, 255, 20, 0, 0, 255, 20, 0, 0, 255]
        );
        // The mirrored copy has the marker pixel on the right.
        let l = anim.frame(0, true);
        assert_eq!(atlas.row(l, 0)[8..12], [0, 20, 0, 255]);
        assert_eq!(atlas.region(l), atlas.region(r).flipped_h());
        // Not mirrored: left falls back to the authored frame.
        let odd = build_atlas(&[sprite("b", 31, 2, false)]);
        let anim = &odd.sprites[0].anims[0];
        assert_eq!(anim.frame(1, true), anim.frame(1, false));
    }
}
