//! Copying atlas frames onto the framebuffer.
//!
//! Atlas pixels are RGBA8 with alpha 0 or 255: alpha 0 is skipped, anything
//! else is drawn opaque. Every function clips at all four framebuffer edges,
//! so any position (including negative and far off-screen) is safe.

use jnr_artkit::atlas::{Atlas, AtlasAnim, AtlasSprite, Rect};

use crate::Framebuffer;

/// Draws the atlas region `rect` with its top-left corner at (`dst_x`, `dst_y`),
/// mirrored horizontally if `flip_h`.
pub fn blit_frame(
    fb: &mut Framebuffer,
    atlas: &Atlas,
    rect: Rect,
    dst_x: i32,
    dst_y: i32,
    flip_h: bool,
) {
    blit(fb, atlas, rect, dst_x, dst_y, flip_h, Paint::Plain);
}

/// How opaque atlas pixels are written.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Paint {
    /// As they are.
    Plain,
    /// Every one in this colour.
    Tint(u32),
    /// Pixels of the first colour in the second, the rest as they are.
    Swap(u32, u32),
}

/// Like [`blit_frame`], but every opaque pixel is drawn in `tint`
/// (used for text and its shadow).
pub fn blit_frame_tinted(
    fb: &mut Framebuffer,
    atlas: &Atlas,
    rect: Rect,
    dst_x: i32,
    dst_y: i32,
    flip_h: bool,
    tint: u32,
) {
    blit(fb, atlas, rect, dst_x, dst_y, flip_h, Paint::Tint(tint));
}

/// Draws frame `anim.frame_index_at(tick)` of a sprite so that the sprite's
/// origin (for the facing, see [`AtlasSprite::origin_facing`]) lands on
/// (`x`, `y`). Facing left uses the atlas's mirrored frames if the sprite
/// has them, otherwise the authored frame is flipped while blitting.
/// An anim without frames draws nothing.
#[allow(clippy::too_many_arguments)]
pub fn draw_sprite(
    fb: &mut Framebuffer,
    atlas: &Atlas,
    sprite: &AtlasSprite,
    anim: &AtlasAnim,
    tick: u32,
    x: i32,
    y: i32,
    facing_left: bool,
) {
    let i = anim.frame_index_at(tick);
    sprite_impl(fb, atlas, sprite, anim, i, x, y, facing_left, Paint::Plain);
}

/// Like [`draw_sprite`], but every opaque pixel is drawn in `tint`.
#[allow(clippy::too_many_arguments)]
pub fn draw_sprite_tinted(
    fb: &mut Framebuffer,
    atlas: &Atlas,
    sprite: &AtlasSprite,
    anim: &AtlasAnim,
    tick: u32,
    x: i32,
    y: i32,
    facing_left: bool,
    tint: u32,
) {
    let i = anim.frame_index_at(tick);
    sprite_impl(
        fb,
        atlas,
        sprite,
        anim,
        i,
        x,
        y,
        facing_left,
        Paint::Tint(tint),
    );
}

/// Like [`draw_sprite`], but pixels of colour `from` (`0xRRGGBBAA`) are
/// drawn in `to` (a palette swap of one colour; used by documented art
/// fallbacks such as `player_hackl`'s).
#[allow(clippy::too_many_arguments)]
pub fn draw_sprite_swapped(
    fb: &mut Framebuffer,
    atlas: &Atlas,
    sprite: &AtlasSprite,
    anim: &AtlasAnim,
    tick: u32,
    x: i32,
    y: i32,
    facing_left: bool,
    (from, to): (u32, u32),
) {
    let i = anim.frame_index_at(tick);
    let paint = Paint::Swap(from, to);
    sprite_impl(fb, atlas, sprite, anim, i, x, y, facing_left, paint);
}

/// Like [`draw_sprite`], but frame `index` of the anim (clamped to its
/// last frame) instead of the frame at a tick, optionally with a colour
/// `swap` as in [`draw_sprite_swapped`].
#[allow(clippy::too_many_arguments)]
pub fn draw_sprite_frame(
    fb: &mut Framebuffer,
    atlas: &Atlas,
    sprite: &AtlasSprite,
    anim: &AtlasAnim,
    index: usize,
    x: i32,
    y: i32,
    facing_left: bool,
    swap: Option<(u32, u32)>,
) {
    let paint = swap.map_or(Paint::Plain, |(from, to)| Paint::Swap(from, to));
    let i = index.min(anim.frames.len().saturating_sub(1));
    sprite_impl(fb, atlas, sprite, anim, i, x, y, facing_left, paint);
}

#[allow(clippy::too_many_arguments)]
fn sprite_impl(
    fb: &mut Framebuffer,
    atlas: &Atlas,
    sprite: &AtlasSprite,
    anim: &AtlasAnim,
    index: usize,
    x: i32,
    y: i32,
    facing_left: bool,
    paint: Paint,
) {
    if anim.frames.is_empty() {
        return;
    }
    let rect = anim.frame(index, facing_left);
    let flip = facing_left && anim.mirrored_frames.is_empty();
    let o = sprite.origin_facing(facing_left);
    let dst_x = x.saturating_sub(o.x as i32);
    let dst_y = y.saturating_sub(o.y as i32);
    blit(fb, atlas, rect, dst_x, dst_y, flip, paint);
}

fn blit(
    fb: &mut Framebuffer,
    atlas: &Atlas,
    rect: Rect,
    dst_x: i32,
    dst_y: i32,
    flip_h: bool,
    paint: Paint,
) {
    let (w, h) = (rect.w as i32, rect.h as i32);
    let (fw, fh) = (fb.w as i32, fb.h as i32);
    // Visible part in source coordinates: 0 <= dst + i < screen size.
    let x0 = dst_x.saturating_neg().clamp(0, w);
    let x1 = fw.saturating_sub(dst_x).clamp(0, w);
    let y0 = dst_y.saturating_neg().clamp(0, h);
    let y1 = fh.saturating_sub(dst_y).clamp(0, h);
    if x0 >= x1 || y0 >= y1 {
        return;
    }
    for sy in y0..y1 {
        let src = atlas.row(rect, sy as u32);
        let dst_row = ((dst_y + sy) * fw + dst_x) as isize;
        for i in x0..x1 {
            let sx = if flip_h { w - 1 - i } else { i } as usize;
            let p = &src[sx * 4..sx * 4 + 4];
            if p[3] == 0 {
                continue;
            }
            let own = u32::from_be_bytes([p[0], p[1], p[2], 0xff]);
            let rgba = match paint {
                Paint::Plain => own,
                Paint::Tint(t) => t,
                Paint::Swap(from, to) if own == from => to,
                Paint::Swap(..) => own,
            };
            fb.pixels[(dst_row + i as isize) as usize] = rgba;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::{self, A, B, BG, C, image};

    /// A 3x2 asymmetric frame: `ab.` / `..c`.
    fn small_atlas() -> Atlas {
        testutil::atlas(&[testutil::sprite(
            "s",
            (1, 2),
            false,
            vec![("idle", 0, vec![image(&["ab.", "..c"])])],
        )])
    }

    fn rect(atlas: &Atlas) -> Rect {
        atlas.sprite("s").unwrap().anims[0].frames[0]
    }

    /// Non-background pixels as (x, y, colour), row-major.
    fn drawn(fb: &Framebuffer) -> Vec<(i32, i32, u32)> {
        let mut out = Vec::new();
        for y in 0..fb.h as i32 {
            for x in 0..fb.w as i32 {
                let p = fb.get(x, y).unwrap();
                if p != BG {
                    out.push((x, y, p));
                }
            }
        }
        out
    }

    #[test]
    fn blit_copies_opaque_pixels_only() {
        let atlas = small_atlas();
        let mut fb = testutil::fb();
        blit_frame(&mut fb, &atlas, rect(&atlas), 10, 20, false);
        assert_eq!(drawn(&fb), [(10, 20, A), (11, 20, B), (12, 21, C)]);
    }

    #[test]
    fn blit_flip_mirrors_columns() {
        let atlas = small_atlas();
        let mut fb = testutil::fb();
        blit_frame(&mut fb, &atlas, rect(&atlas), 10, 20, true);
        assert_eq!(drawn(&fb), [(11, 20, B), (12, 20, A), (10, 21, C)]);
    }

    #[test]
    fn blit_clips_left_and_top_with_negative_coords() {
        let atlas = small_atlas();
        let mut fb = testutil::fb();
        blit_frame(&mut fb, &atlas, rect(&atlas), -1, 0, false);
        assert_eq!(drawn(&fb), [(0, 0, B), (1, 1, C)]);
        let mut fb = testutil::fb();
        blit_frame(&mut fb, &atlas, rect(&atlas), 0, -1, false);
        assert_eq!(drawn(&fb), [(2, 0, C)]);
        let mut fb = testutil::fb();
        blit_frame(&mut fb, &atlas, rect(&atlas), -2, -1, true);
        assert_eq!(drawn(&fb), [], "the flipped `c` lands at x = -2");
        let mut fb = testutil::fb();
        blit_frame(&mut fb, &atlas, rect(&atlas), -1, 0, true);
        assert_eq!(drawn(&fb), [(0, 0, B), (1, 0, A)]);
    }

    #[test]
    fn blit_clips_right_and_bottom() {
        let atlas = small_atlas();
        let mut fb = testutil::fb();
        blit_frame(&mut fb, &atlas, rect(&atlas), 318, 179, false);
        assert_eq!(drawn(&fb), [(318, 179, A), (319, 179, B)]);
        let mut fb = testutil::fb();
        blit_frame(&mut fb, &atlas, rect(&atlas), 319, 178, true);
        assert_eq!(drawn(&fb), [(319, 179, C)]);
    }

    #[test]
    fn blit_far_off_screen_draws_nothing() {
        let atlas = small_atlas();
        let mut fb = testutil::fb();
        for (x, y) in [
            (i32::MIN, 0),
            (i32::MAX, 0),
            (0, i32::MIN),
            (0, i32::MAX),
            (-3, 0),
            (320, 0),
            (0, -2),
            (0, 180),
        ] {
            blit_frame(&mut fb, &atlas, rect(&atlas), x, y, false);
            blit_frame(&mut fb, &atlas, rect(&atlas), x, y, true);
        }
        assert_eq!(drawn(&fb), []);
    }

    #[test]
    fn tint_replaces_every_opaque_pixel() {
        let atlas = small_atlas();
        let mut fb = testutil::fb();
        blit_frame_tinted(&mut fb, &atlas, rect(&atlas), 0, 0, false, 0x1234_56ff);
        assert_eq!(
            drawn(&fb),
            [
                (0, 0, 0x1234_56ff),
                (1, 0, 0x1234_56ff),
                (2, 1, 0x1234_56ff)
            ]
        );
        let s = atlas.sprite("s").unwrap();
        let mut fb = testutil::fb();
        draw_sprite_tinted(&mut fb, &atlas, s, &s.anims[0], 0, 1, 2, false, 7);
        assert_eq!(drawn(&fb), [(0, 0, 7), (1, 0, 7), (2, 1, 7)]);
    }

    #[test]
    fn swap_replaces_one_colour_only() {
        let atlas = small_atlas();
        let s = atlas.sprite("s").unwrap();
        let mut fb = testutil::fb();
        draw_sprite_swapped(&mut fb, &atlas, s, &s.anims[0], 0, 1, 2, false, (B, 7));
        assert_eq!(drawn(&fb), [(0, 0, A), (1, 0, 7), (2, 1, C)]);
        let mut fb = testutil::fb();
        draw_sprite_frame(
            &mut fb,
            &atlas,
            s,
            &s.anims[0],
            5,
            1,
            2,
            false,
            Some((A, 9)),
        );
        assert_eq!(
            drawn(&fb),
            [(0, 0, 9), (1, 0, B), (2, 1, C)],
            "clamped index"
        );
    }

    #[test]
    fn sprite_origin_lands_on_position() {
        // 3x2 frame with origin (1, 2): the top-left lands at (x - 1, y - 2).
        let atlas = small_atlas();
        let s = atlas.sprite("s").unwrap();
        let mut fb = testutil::fb();
        draw_sprite(&mut fb, &atlas, s, &s.anims[0], 0, 50, 60, false);
        assert_eq!(drawn(&fb), [(49, 58, A), (50, 58, B), (51, 59, C)]);
    }

    #[test]
    fn sprite_facing_left_mirrors_around_origin() {
        // Not mirrored in the atlas: flipped while blitting. Left origin is
        // (3 - 1, 2), so the top-left lands at (x - 2, y - 2).
        let atlas = small_atlas();
        let s = atlas.sprite("s").unwrap();
        let mut fb = testutil::fb();
        draw_sprite(&mut fb, &atlas, s, &s.anims[0], 0, 50, 60, true);
        assert_eq!(drawn(&fb), [(49, 58, B), (50, 58, A), (48, 59, C)]);

        // Mirrored in the atlas (`mirror = auto`): same pixels on screen.
        let mirrored = testutil::atlas(&[testutil::sprite(
            "s",
            (1, 2),
            true,
            vec![("idle", 0, vec![image(&["ab.", "..c"])])],
        )]);
        let s = mirrored.sprite("s").unwrap();
        assert!(!s.anims[0].mirrored_frames.is_empty());
        let mut fb2 = testutil::fb();
        draw_sprite(&mut fb2, &mirrored, s, &s.anims[0], 0, 50, 60, true);
        assert_eq!(fb2, fb);
    }

    #[test]
    fn sprite_picks_frame_from_tick() {
        let atlas = testutil::atlas(&[testutil::sprite(
            "s",
            (0, 0),
            false,
            vec![("walk", 4, vec![image(&["a"]), image(&["b"])])],
        )]);
        let s = atlas.sprite("s").unwrap();
        let colour_at = |tick| {
            let mut fb = testutil::fb();
            draw_sprite(&mut fb, &atlas, s, &s.anims[0], tick, 5, 5, false);
            fb.get(5, 5).unwrap()
        };
        assert_eq!(
            [colour_at(0), colour_at(3), colour_at(4), colour_at(8)],
            [A, A, B, A]
        );
    }
}
