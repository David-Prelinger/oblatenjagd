//! Drawn rectangles of sprites, for tooling that checks what the renderer
//! would draw where (e.g. `levels lint`: no Oblate on a sign board, E33)
//! without rendering a frame.

use jnr_artkit::atlas::{Atlas, AtlasSprite, Rect};

/// A pixel rectangle: left, top, width, height (may be relative to an
/// entity's feet or in level pixels, as documented where it is used).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PxRect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl PxRect {
    pub const fn new(x: i32, y: i32, w: i32, h: i32) -> PxRect {
        PxRect { x, y, w, h }
    }

    /// The same rectangle moved by `(dx, dy)`.
    pub const fn offset(self, dx: i32, dy: i32) -> PxRect {
        PxRect::new(self.x + dx, self.y + dy, self.w, self.h)
    }

    /// The smallest rectangle containing both.
    pub fn union(self, o: PxRect) -> PxRect {
        let (x, y) = (self.x.min(o.x), self.y.min(o.y));
        let right = (self.x + self.w).max(o.x + o.w);
        let bottom = (self.y + self.h).max(o.y + o.h);
        PxRect::new(x, y, right - x, bottom - y)
    }

    /// The rectangle grown by `m` px on every side.
    pub const fn grow(self, m: i32) -> PxRect {
        PxRect::new(self.x - m, self.y - m, self.w + 2 * m, self.h + 2 * m)
    }

    /// True if the two share at least one pixel.
    pub const fn overlaps(&self, o: &PxRect) -> bool {
        self.x < o.x + o.w && o.x < self.x + self.w && self.y < o.y + o.h && o.y < self.y + self.h
    }
}

/// Bounding box of the opaque pixels of an atlas frame within its rows
/// `rows.0 .. rows.1`, relative to the frame's top-left; `None` if none.
pub fn opaque_box(atlas: &Atlas, frame: Rect, rows: (u32, u32)) -> Option<PxRect> {
    let mut out: Option<PxRect> = None;
    for y in rows.0..rows.1.min(frame.h) {
        let row = atlas.row(frame, y);
        let opaque = |x: &u32| row[*x as usize * 4 + 3] != 0;
        let (Some(l), Some(r)) = ((0..frame.w).find(opaque), (0..frame.w).rev().find(opaque))
        else {
            continue;
        };
        let line = PxRect::new(l as i32, y as i32, (r - l + 1) as i32, 1);
        out = Some(out.map_or(line, |o| o.union(line)));
    }
    out
}

/// What `draw_body` draws for a sprite at its feet (facing right):
/// the union of the opaque boxes of every frame of its first anim,
/// relative to the feet. `None` for a sprite with no opaque pixel.
pub fn body_box(atlas: &Atlas, sprite: &AtlasSprite) -> Option<PxRect> {
    let anim = sprite.anims.first()?;
    let (ox, oy) = (sprite.origin.x as i32, sprite.origin.y as i32);
    anim.frames
        .iter()
        .filter_map(|&f| opaque_box(atlas, f, (0, f.h)))
        .reduce(PxRect::union)
        .map(|r| r.offset(-ox, -oy))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::{self, image};

    #[test]
    fn rect_union_grow_and_overlap() {
        let a = PxRect::new(0, 0, 10, 10);
        let b = PxRect::new(10, 0, 4, 4);
        assert!(!a.overlaps(&b), "touching edges share no pixel");
        assert!(a.grow(1).overlaps(&b));
        assert!(a.overlaps(&PxRect::new(9, 9, 1, 1)));
        assert_eq!(a.union(b.offset(0, 20)), PxRect::new(0, 0, 14, 24));
    }

    #[test]
    fn body_box_is_the_opaque_union_of_all_frames_at_the_feet() {
        let f0 = image(&["......", "..a...", "......"]);
        let f1 = image(&["......", "......", "...aa."]);
        let s = testutil::sprite("x", (3, 3), false, vec![("idle", 0, vec![f0, f1])]);
        let atlas = testutil::atlas(&[s]);
        let sprite = atlas.sprite("x").unwrap();
        // Opaque x 2..=4, y 1..=2; origin (3, 3).
        assert_eq!(body_box(&atlas, sprite), Some(PxRect::new(-1, -2, 3, 2)));
    }
}
