//! `jnr-render`: the software renderer of Oblatenjagd.
//!
//! Draws the game into a fixed-size RGBA [`Framebuffer`] with pure pixel
//! operations. No GPU and no macroquad: the same renderer feeds the real
//! window (via `jnr-app`) and headless screenshots, so a golden screenshot
//! is exactly what the player sees. See `docs/architecture.md`.
//!
//! The sprite atlas is always passed in as `&Atlas` (normally
//! `jnr_assets::atlas()`), so tests can use small hand-built atlases.
//! Everything is integer math: the same inputs give identical pixels.
//!
//! Modules:
//! - [`background`]: parallax background layers (tiled, bottom-aligned)
//! - [`blit`]: atlas frames and sprites onto the framebuffer (clipping, flip, tint)
//! - [`font`]: the variable-width bitmap font (sprite `font`, anim `glyphs`)
//! - [`scene`]: [`render`] a whole `Game` through a [`RenderContext`]
//!   (font, texts and sprite lookups resolved once), plus the opt-in
//!   [`render_debug`] overlay
//! - [`tiles`]: the tile grid, incl. autotiling (D-020)
//! - [`palette`]: the few palette colours used for grey-box drawing and text

pub mod background;
pub mod blit;
pub mod font;
pub mod palette;
pub mod scene;
pub mod tiles;

#[cfg(test)]
mod testutil;

pub use background::{BackgroundLayer, parallax_offset};
pub use blit::{blit_frame, blit_frame_tinted, draw_sprite, draw_sprite_tinted};
pub use font::{CHARSET, Font};
pub use scene::{PlayerAnim, RenderContext, render, render_debug};
pub use tiles::{TileLook, autotile_mask};

/// Framebuffer width in pixels.
pub const WIDTH: usize = 320;
/// Framebuffer height in pixels.
pub const HEIGHT: usize = 180;

/// A 320x180 image. Each pixel is `0xRRGGBBAA`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Framebuffer {
    pub w: usize,
    pub h: usize,
    pub pixels: Vec<u32>,
}

impl Framebuffer {
    /// Creates a framebuffer of the game's native size, filled with opaque black.
    pub fn new() -> Framebuffer {
        Framebuffer {
            w: WIDTH,
            h: HEIGHT,
            pixels: vec![0x0000_00ff; WIDTH * HEIGHT],
        }
    }

    /// Fills every pixel with `rgba`.
    pub fn clear(&mut self, rgba: u32) {
        self.pixels.fill(rgba);
    }

    /// The pixel at (`x`, `y`), or `None` outside the framebuffer.
    pub fn get(&self, x: i32, y: i32) -> Option<u32> {
        self.index(x, y).map(|i| self.pixels[i])
    }

    /// Sets one pixel; outside the framebuffer nothing happens.
    pub fn set(&mut self, x: i32, y: i32, rgba: u32) {
        if let Some(i) = self.index(x, y) {
            self.pixels[i] = rgba;
        }
    }

    /// Fills a rectangle, clipped to the framebuffer (any position and size;
    /// zero or negative sizes draw nothing).
    pub fn fill_rect(&mut self, x: i32, y: i32, w: i32, h: i32, rgba: u32) {
        let (fw, fh) = (self.w as i32, self.h as i32);
        let x0 = x.clamp(0, fw);
        let y0 = y.clamp(0, fh);
        let x1 = x.saturating_add(w.max(0)).clamp(0, fw);
        let y1 = y.saturating_add(h.max(0)).clamp(0, fh);
        if x0 >= x1 {
            return;
        }
        for row in y0..y1 {
            let start = (row * fw) as usize;
            self.pixels[start + x0 as usize..start + x1 as usize].fill(rgba);
        }
    }

    /// Draws a 1 px rectangle outline (inside the given rectangle), clipped.
    pub fn outline_rect(&mut self, x: i32, y: i32, w: i32, h: i32, rgba: u32) {
        if w <= 0 || h <= 0 {
            return;
        }
        let right = x.saturating_add(w - 1);
        let bottom = y.saturating_add(h - 1);
        self.fill_rect(x, y, w, 1, rgba);
        self.fill_rect(x, bottom, w, 1, rgba);
        self.fill_rect(x, y, 1, h, rgba);
        self.fill_rect(right, y, 1, h, rgba);
    }

    /// Returns the pixels as RGBA bytes in row-major order (for texture upload or PNG export).
    pub fn to_rgba_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        self.write_rgba_bytes(&mut out);
        out
    }

    /// Like [`Framebuffer::to_rgba_bytes`], but reuses `out` (cleared first),
    /// so a per-frame texture upload does not allocate.
    pub fn write_rgba_bytes(&self, out: &mut Vec<u8>) {
        out.clear();
        out.reserve(self.pixels.len() * 4);
        for p in &self.pixels {
            out.extend_from_slice(&p.to_be_bytes());
        }
    }

    fn index(&self, x: i32, y: i32) -> Option<usize> {
        if x < 0 || y < 0 || x as usize >= self.w || y as usize >= self.h {
            return None;
        }
        Some(y as usize * self.w + x as usize)
    }
}

impl Default for Framebuffer {
    fn default() -> Self {
        Framebuffer::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_framebuffer_is_320x180_black() {
        let fb = Framebuffer::new();
        assert_eq!((fb.w, fb.h), (320, 180));
        assert_eq!(fb.pixels.len(), 320 * 180);
        assert!(fb.pixels.iter().all(|&p| p == 0x0000_00ff));
    }

    #[test]
    fn clear_and_rgba_bytes() {
        let mut fb = Framebuffer::new();
        fb.clear(0x1122_33ff);
        fb.set(1, 0, 0xaabb_ccdd);
        let bytes = fb.to_rgba_bytes();
        assert_eq!(bytes.len(), 320 * 180 * 4);
        assert_eq!(
            &bytes[..8],
            &[0x11, 0x22, 0x33, 0xff, 0xaa, 0xbb, 0xcc, 0xdd]
        );
        let mut reused = vec![9; 3];
        fb.write_rgba_bytes(&mut reused);
        assert_eq!(reused, bytes);
    }

    #[test]
    fn get_and_set_ignore_outside() {
        let mut fb = Framebuffer::new();
        fb.set(-1, 0, 1);
        fb.set(320, 0, 1);
        fb.set(0, 180, 1);
        assert!(fb.pixels.iter().all(|&p| p == 0x0000_00ff));
        fb.set(319, 179, 7);
        assert_eq!(fb.get(319, 179), Some(7));
        assert_eq!(fb.get(320, 0), None);
        assert_eq!(fb.get(0, -1), None);
    }

    #[test]
    fn fill_rect_clips_at_every_edge() {
        let mut fb = Framebuffer::new();
        fb.clear(0);
        fb.fill_rect(-5, -5, 7, 7, 1); // top-left: covers 0..2
        fb.fill_rect(318, 178, 10, 10, 2); // bottom-right: covers 318..320
        fb.fill_rect(i32::MIN, 50, i32::MAX, 1, 3); // huge, entirely left
        fb.fill_rect(100, 100, -3, 4, 4); // negative size: nothing
        assert_eq!(fb.get(1, 1), Some(1));
        assert_eq!(fb.get(2, 1), Some(0));
        assert_eq!(fb.get(318, 178), Some(2));
        assert_eq!(fb.get(319, 179), Some(2));
        assert_eq!(fb.get(317, 179), Some(0));
        let count = |c: u32| fb.pixels.iter().filter(|&&p| p == c).count();
        assert_eq!((count(1), count(2), count(3), count(4)), (4, 4, 0, 0));
    }

    #[test]
    fn outline_rect_draws_only_the_border() {
        let mut fb = Framebuffer::new();
        fb.clear(0);
        fb.outline_rect(10, 20, 4, 3, 5);
        assert_eq!(fb.pixels.iter().filter(|&&p| p == 5).count(), 10);
        assert_eq!(fb.get(11, 21), Some(0));
        assert_eq!(fb.get(13, 22), Some(5));
    }
}
