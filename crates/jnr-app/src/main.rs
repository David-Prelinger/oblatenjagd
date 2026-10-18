//! `jnr-app`: the macroquad shell of Oblatenjagd (native + wasm).
//!
//! The only crate that knows about macroquad. It polls input, steps the
//! simulation at a fixed 60 Hz, lets `jnr-render` draw into the 320x180
//! framebuffer and uploads that framebuffer as a texture, scaled by an
//! integer factor with a black letterbox.
//!
//! Skeleton: shows a solid-colour framebuffer placeholder.

use jnr_render::{Framebuffer, HEIGHT, WIDTH};
use macroquad::prelude::*;

/// Placeholder fill colour (Salzkammergut sky blue), `0xRRGGBBAA`.
const PLACEHOLDER_SKY: u32 = 0x5f_a8_e8_ff;

fn window_conf() -> Conf {
    Conf {
        window_title: "Oblatenjagd".to_owned(),
        window_width: (WIDTH * 4) as i32,
        window_height: (HEIGHT * 4) as i32,
        ..Default::default()
    }
}

/// Largest integer scale at which the framebuffer fits the screen (at least 1).
fn integer_scale(screen_w: f32, screen_h: f32) -> f32 {
    let sx = (screen_w / WIDTH as f32).floor();
    let sy = (screen_h / HEIGHT as f32).floor();
    sx.min(sy).max(1.0)
}

#[macroquad::main(window_conf)]
async fn main() {
    let mut fb = Framebuffer::new();
    fb.clear(PLACEHOLDER_SKY);

    let texture = Texture2D::from_rgba8(WIDTH as u16, HEIGHT as u16, &fb.to_rgba_bytes());
    texture.set_filter(FilterMode::Nearest);

    loop {
        clear_background(BLACK);

        let scale = integer_scale(screen_width(), screen_height());
        let (w, h) = (WIDTH as f32 * scale, HEIGHT as f32 * scale);
        let x = ((screen_width() - w) / 2.0).floor();
        let y = ((screen_height() - h) / 2.0).floor();
        texture.update_from_bytes(WIDTH as u32, HEIGHT as u32, &fb.to_rgba_bytes());
        draw_texture_ex(
            &texture,
            x,
            y,
            WHITE,
            DrawTextureParams {
                dest_size: Some(vec2(w, h)),
                ..Default::default()
            },
        );

        next_frame().await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn integer_scale_fits_and_letterboxes() {
        assert_eq!(integer_scale(1280.0, 720.0), 4.0);
        assert_eq!(integer_scale(1300.0, 700.0), 3.0);
        assert_eq!(integer_scale(100.0, 100.0), 1.0);
    }
}
