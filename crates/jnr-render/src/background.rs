//! Parallax background layers.
//!
//! A level lists its layers back to front as (sprite name, scroll factor in
//! sixteenths of the camera movement, y offset, vertical factor);
//! [`RenderContext::with_background`] resolves them once. Each layer is drawn
//! - tiled horizontally, starting at x = [`parallax_offset`] (in `-(width-1)..=0`),
//! - **bottom-aligned** to the screen bottom (y = screen height − sprite
//!   height − `y`), plus the vertical parallax [`sink_px`]: `(max camera y −
//!   camera y) × factor_y / 16` px lower (D-054). With `factor_y` 0 (the
//!   default) or at the level bottom it is fixed to the screen; layers only
//!   ever sink, so the sky behind them never shows a gap,
//! - with its first animation, animated by the game frame like tiles.
//!
//! Factor 0 is a static sky, 16 moves with the playfield. The flat sky fill
//! (palette `s`) is drawn first unless the first layer covers the whole
//! screen (fully opaque, at least as tall as the screen and fixed to its
//! bottom: [`covers_screen`]), so partial
//! layer lists never show stale pixels.
//!
//! [`RenderContext::with_background`]: crate::RenderContext::with_background

use jnr_artkit::atlas::{Atlas, AtlasAnim, AtlasSprite};

use crate::Framebuffer;
use crate::blit::blit_frame;
use crate::palette::SKY_LIGHT;

/// One resolved background layer.
#[derive(Clone, Debug)]
pub struct BackgroundLayer<'a> {
    pub sprite: &'a AtlasSprite,
    pub anim: &'a AtlasAnim,
    /// Scroll factor in sixteenths (0 = static, 16 = like the playfield).
    pub factor16: u32,
    /// Raised this many pixels above the screen bottom (level `y`, default 0).
    pub y_offset: u32,
    /// Vertical parallax in sixteenths of the camera's climb (level
    /// `factor_y`, default 0; see [`sink_px`]).
    pub factor_y16: u32,
}

/// Resolves `(sprite name, factor in sixteenths, y offset, factor_y in
/// sixteenths)` against the atlas, back to front. Names the atlas lacks (or
/// sprites without frames) are skipped, like every missing sprite in the
/// renderer.
pub fn resolve<'a>(atlas: &'a Atlas, layers: &[(&str, u32, u32, u32)]) -> Vec<BackgroundLayer<'a>> {
    layers
        .iter()
        .filter_map(|&(name, factor16, y_offset, factor_y16)| {
            let sprite = atlas.sprite(name)?;
            let anim = sprite.anims.first().filter(|a| !a.frames.is_empty())?;
            Some(BackgroundLayer {
                sprite,
                anim,
                factor16,
                y_offset,
                factor_y16,
            })
        })
        .collect()
}

/// How many px lower than its bottom-aligned place a layer with vertical
/// factor `factor_y16` is drawn for a camera at `camera_y` in a level whose
/// lowest camera position is `max_camera_y` (D-054): `(max_camera_y -
/// camera_y) × factor_y16 / 16`, rounded down, never negative. 0 at the
/// level bottom and for `factor_y16` 0.
pub fn sink_px(camera_y: i32, max_camera_y: i32, factor_y16: u32) -> i32 {
    let climbed = (i64::from(max_camera_y) - i64::from(camera_y)).max(0);
    (climbed * i64::from(factor_y16) / 16).min(i64::from(i32::MAX)) as i32
}

/// True if `layer` hides everything behind it on a `screen_h` tall screen:
/// at least that tall, every pixel of every frame opaque, and fixed to the
/// screen bottom (`y` 0 and `factor_y` 0: a raised or sinking layer would
/// leave a strip uncovered). Any scroll factor qualifies, since the copies
/// tile the whole width (e.g. M7's `bg_zauner_salon` at factor 16, the
/// café's back wall moving with the room).
pub fn covers_screen(atlas: &Atlas, layer: &BackgroundLayer, screen_h: u32) -> bool {
    layer.sprite.height >= screen_h
        && layer.y_offset == 0
        && layer.factor_y16 == 0
        && layer
            .anim
            .frames
            .iter()
            .all(|&r| (0..r.h).all(|y| atlas.row(r, y).chunks_exact(4).all(|p| p[3] != 0)))
}

/// Left edge of the first copy of a `width` px wide layer for a camera at
/// `camera_x` and a scroll factor of `factor16` sixteenths:
/// `-((camera_x * factor16 / 16) mod width)`, rounded towards minus
/// infinity, always in `-(width - 1)..=0`. Copies follow every `width` px.
pub fn parallax_offset(camera_x: i32, factor16: u32, width: u32) -> i32 {
    if width == 0 {
        return 0;
    }
    let scroll = (i64::from(camera_x) * i64::from(factor16)).div_euclid(16);
    -(scroll.rem_euclid(i64::from(width)) as i32)
}

/// Draws the flat sky (unless `covered`) and every layer for a camera at
/// `camera` (x, y) in a level whose lowest camera y is `max_camera_y`,
/// animated at `tick`.
pub fn draw(
    fb: &mut Framebuffer,
    atlas: &Atlas,
    layers: &[BackgroundLayer],
    covered: bool,
    camera: (i32, i32),
    max_camera_y: i32,
    tick: u32,
) {
    let (camera_x, camera_y) = camera;
    if !covered {
        fb.clear(SKY_LIGHT);
    }
    for layer in layers {
        let w = layer.sprite.width as i32;
        if w == 0 {
            continue;
        }
        let rect = layer.anim.frame(layer.anim.frame_index_at(tick), false);
        let sink = sink_px(camera_y, max_camera_y, layer.factor_y16).min(10_000);
        let y = fb.h as i32 - layer.sprite.height as i32 - layer.y_offset.min(10_000) as i32 + sink;
        let mut x = parallax_offset(camera_x, layer.factor16, layer.sprite.width);
        while x < fb.w as i32 {
            blit_frame(fb, atlas, rect, x, y, false);
            x += w;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::{self, A, B, C, image};
    use crate::{HEIGHT, WIDTH};

    #[test]
    fn offset_wraps_and_stays_in_range() {
        // factor 16: moves with the camera, wraps every width.
        assert_eq!(parallax_offset(0, 16, 100), 0);
        assert_eq!(parallax_offset(30, 16, 100), -30);
        assert_eq!(parallax_offset(130, 16, 100), -30);
        assert_eq!(parallax_offset(100, 16, 100), 0);
        // factor 0: static.
        assert_eq!(parallax_offset(12345, 0, 100), 0);
        // factor 8: half speed; 3/16 rounds the scroll down.
        assert_eq!(parallax_offset(30, 8, 100), -15);
        assert_eq!(parallax_offset(10, 3, 100), -1); // 30 / 16 = 1.875 -> 1
        // Negative camera positions wrap the other way and stay in range.
        assert_eq!(parallax_offset(-30, 16, 100), -70);
        assert_eq!(parallax_offset(-1, 3, 100), -99); // -3 / 16 -> -1
        for cam in -500..500 {
            for f in [0, 1, 3, 8, 13, 16, 24] {
                let o = parallax_offset(cam, f, 37);
                assert!((-36..=0).contains(&o), "{cam} {f} -> {o}");
            }
        }
        // Huge values do not overflow.
        assert!(parallax_offset(i32::MAX, 32, 640) <= 0);
        assert_eq!(parallax_offset(5, 16, 0), 0);
    }

    /// A 3x2 layer `abc` / `abc` (columns a, b, c) and a full-screen layer.
    fn layer_atlas() -> Atlas {
        let full: Vec<String> = (0..HEIGHT).map(|_| "c".repeat(WIDTH)).collect();
        let full: Vec<&str> = full.iter().map(String::as_str).collect();
        testutil::atlas(&[
            testutil::sprite(
                "bg_strip",
                (0, 0),
                false,
                vec![("idle", 0, vec![image(&["abc", "abc"])])],
            ),
            testutil::sprite(
                "bg_full",
                (0, 0),
                false,
                vec![("idle", 0, vec![image(&full)])],
            ),
            testutil::sprite(
                "bg_holey",
                (0, 0),
                false,
                vec![("idle", 0, vec![image(&["a.", "aa"])])],
            ),
        ])
    }

    #[test]
    fn layers_tile_horizontally_bottom_aligned() {
        let atlas = layer_atlas();
        let layers = resolve(&atlas, &[("bg_strip", 16, 0, 0)]);
        let mut fb = testutil::fb();
        draw(&mut fb, &atlas, &layers, false, (1, 0), 0, 0);
        // Camera 1 at factor 16: offset -1, so the screen starts with `b c a b c ...`.
        let y = HEIGHT as i32 - 1;
        let row: Vec<u32> = (0..6).map(|x| fb.get(x, y).unwrap()).collect();
        assert_eq!(row, [B, C, A, B, C, A]);
        // The right edge is covered too.
        assert_eq!(
            fb.get(WIDTH as i32 - 1, y),
            Some([A, B, C][WIDTH % 3]) // strip column (x + 1) mod 3 at x = WIDTH - 1
        );
        // Two rows tall, bottom-aligned; above it the flat sky.
        assert_eq!(fb.get(0, HEIGHT as i32 - 2), Some(B));
        assert_eq!(fb.get(0, HEIGHT as i32 - 3), Some(SKY_LIGHT));
        // Factor 0 ignores the camera.
        let still = resolve(&atlas, &[("bg_strip", 0, 0, 0)]);
        let mut fb = testutil::fb();
        draw(&mut fb, &atlas, &still, false, (1000, 0), 0, 0);
        assert_eq!(fb.get(0, y), Some(A));
    }

    #[test]
    fn y_offset_raises_a_layer_above_the_screen_bottom() {
        let atlas = layer_atlas();
        let layers = resolve(&atlas, &[("bg_strip", 0, 10, 0)]);
        assert_eq!(layers[0].y_offset, 10);
        let mut fb = testutil::fb();
        draw(&mut fb, &atlas, &layers, false, (0, 0), 0, 0);
        let bottom = HEIGHT as i32 - 1;
        assert_eq!(fb.get(0, bottom), Some(SKY_LIGHT));
        assert_eq!(fb.get(0, bottom - 10), Some(A));
        assert_eq!(fb.get(0, bottom - 11), Some(A));
        assert_eq!(fb.get(0, bottom - 12), Some(SKY_LIGHT));
    }

    #[test]
    fn sink_is_the_climb_times_factor_y_over_16() {
        assert_eq!(sink_px(492, 492, 8), 0, "at the level bottom");
        assert_eq!(sink_px(0, 492, 0), 0, "factor_y 0");
        assert_eq!(sink_px(0, 492, 8), 246);
        assert_eq!(
            sink_px(100, 492, 3),
            73,
            "392 × 3 / 16 = 73.5, rounded down"
        );
        assert_eq!(sink_px(0, 492, 32), 984);
        assert_eq!(sink_px(600, 492, 8), 0, "never up");
        assert_eq!(sink_px(i32::MIN, i32::MAX, 32), i32::MAX, "no overflow");
    }

    #[test]
    fn factor_y_sinks_a_layer_as_the_camera_climbs() {
        let atlas = layer_atlas();
        let layers = resolve(&atlas, &[("bg_strip", 0, 10, 8)]);
        assert_eq!(layers[0].factor_y16, 8);
        let bottom = HEIGHT as i32 - 1;
        // At the level bottom: exactly where `y` puts it.
        let mut fb = testutil::fb();
        draw(&mut fb, &atlas, &layers, false, (0, 100), 100, 0);
        assert_eq!(fb.get(0, bottom - 10), Some(A));
        assert_eq!(fb.get(0, bottom - 12), Some(SKY_LIGHT));
        // 6 px climbed at factor 8: 3 px lower.
        let mut fb = testutil::fb();
        draw(&mut fb, &atlas, &layers, false, (0, 94), 100, 0);
        assert_eq!(fb.get(0, bottom - 7), Some(A));
        assert_eq!(fb.get(0, bottom - 8), Some(A));
        assert_eq!(fb.get(0, bottom - 9), Some(SKY_LIGHT));
        assert_eq!(fb.get(0, bottom - 6), Some(SKY_LIGHT));
        // Far up it has sunk off the screen.
        let mut fb = testutil::fb();
        draw(&mut fb, &atlas, &layers, false, (0, 0), 100, 0);
        assert!(fb.pixels.iter().all(|&p| p == SKY_LIGHT));
    }

    #[test]
    fn later_layers_draw_over_earlier_ones_and_missing_ones_are_skipped() {
        let atlas = layer_atlas();
        let layers = resolve(
            &atlas,
            &[
                ("bg_full", 0, 0, 0),
                ("nope", 4, 0, 0),
                ("bg_holey", 16, 0, 0),
            ],
        );
        assert_eq!(layers.len(), 2);
        assert!(covers_screen(&atlas, &layers[0], HEIGHT as u32));
        assert!(
            !covers_screen(&atlas, &layers[1], HEIGHT as u32),
            "transparent pixel"
        );
        let mut fb = testutil::fb();
        draw(&mut fb, &atlas, &layers, true, (0, 0), 0, 0);
        let bottom = HEIGHT as i32 - 1;
        assert_eq!(fb.get(0, bottom), Some(A));
        assert_eq!(
            fb.get(1, bottom - 1),
            Some(C),
            "transparent pixel shows the layer behind"
        );
        assert_eq!(fb.get(5, 0), Some(C));
    }

    #[test]
    fn an_opaque_full_height_layer_at_factor_16_covers_every_column() {
        // M7 (`bg_zauner_salon`): the only layer, moving with the room.
        // A 7 px wide opaque column pattern (7 does not divide the screen
        // width) tiles the whole screen for every camera x, so the flat sky
        // is never needed and never shows.
        let cols = ["a", "b", "c", "a", "b", "c", "c"];
        let rows: Vec<String> = (0..HEIGHT).map(|_| cols.concat()).collect();
        let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
        let atlas = testutil::atlas(&[testutil::sprite(
            "bg_salon",
            (0, 0),
            false,
            vec![("idle", 0, vec![image(&rows)])],
        )]);
        let layers = resolve(&atlas, &[("bg_salon", 16, 0, 0)]);
        assert!(covers_screen(&atlas, &layers[0], HEIGHT as u32));
        let colour = |i: i32| match cols[i.rem_euclid(7) as usize] {
            "a" => A,
            "b" => B,
            _ => C,
        };
        for camera_x in [0, 1, 6, 7, 100, 333, 2000] {
            let mut fb = testutil::fb();
            fb.clear(0x0102_0304);
            draw(&mut fb, &atlas, &layers, true, (camera_x, 92), 92, 0);
            for x in 0..WIDTH as i32 {
                for y in [0, HEIGHT as i32 / 2, HEIGHT as i32 - 1] {
                    assert_eq!(
                        fb.get(x, y),
                        Some(colour(x + camera_x)),
                        "{camera_x} {x} {y}"
                    );
                }
            }
        }
        // Raised (`y`) or sinking (`factor_y`), the same layer leaves a strip
        // free, so the flat sky stays behind it.
        for (y, fy) in [(1, 0), (0, 8)] {
            let moved = resolve(&atlas, &[("bg_salon", 16, y, fy)]);
            assert!(!covers_screen(&atlas, &moved[0], HEIGHT as u32), "{y} {fy}");
        }
    }

    #[test]
    fn partial_layers_keep_the_flat_sky_behind_them() {
        let atlas = layer_atlas();
        let strip = resolve(&atlas, &[("bg_strip", 16, 0, 0)]);
        assert!(
            !covers_screen(&atlas, &strip[0], HEIGHT as u32),
            "too short"
        );
        let mut fb = testutil::fb();
        draw(&mut fb, &atlas, &[], false, (0, 0), 0, 0);
        assert!(fb.pixels.iter().all(|&p| p == SKY_LIGHT));
    }
}
