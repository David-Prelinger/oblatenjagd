//! `jnr-render`: the software renderer of Oblatenjagd.
//!
//! Draws the game into a fixed-size RGBA [`Framebuffer`] with pure pixel
//! operations. No GPU and no macroquad: the same renderer feeds the real
//! window (via `jnr-app`) and headless screenshots, so a golden screenshot
//! is exactly what the player sees. See `docs/architecture.md`.

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

    /// Returns the pixels as RGBA bytes in row-major order (for texture upload or PNG export).
    pub fn to_rgba_bytes(&self) -> Vec<u8> {
        self.pixels.iter().flat_map(|p| p.to_be_bytes()).collect()
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
        let bytes = fb.to_rgba_bytes();
        assert_eq!(bytes.len(), 320 * 180 * 4);
        assert_eq!(&bytes[..4], &[0x11, 0x22, 0x33, 0xff]);
    }
}
