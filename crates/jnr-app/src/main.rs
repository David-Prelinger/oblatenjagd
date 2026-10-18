//! `jnr-app`: the macroquad shell of Oblatenjagd (native + wasm).
//!
//! The only crate that knows about macroquad. It polls input, steps the
//! simulation at a fixed 60 Hz, lets `jnr-render` draw into the 320x180
//! framebuffer and uploads that framebuffer as a texture, scaled by an
//! integer factor with a black letterbox. Floats are used here only for
//! timing and scaling, never in the simulation.
//!
//! M0: always plays the grey-box testbed (`content/levels/00_testbed.level`).
//! Keys: arrows move, X/Space jump, Y/Z/Shift run, Enter start, F1 debug overlay.

use jnr_core::{Game, InputFrame, Level, TICKS_PER_SECOND, Tuning};
use jnr_render::{Framebuffer, HEIGHT, WIDTH, render, render_debug};
use macroquad::prelude::*;

/// The level played in M0 (embedded, so wasm needs no file access).
const LEVEL_SRC: &str = include_str!("../../../content/levels/00_testbed.level");
/// Gameplay constants (embedded).
const TUNING_SRC: &str = include_str!("../../../content/tuning.toml");
/// Seed of the game RNG.
const SEED: u64 = 1;
/// Longest real time one frame may account for; more is dropped (e.g. after
/// the window was hidden), so the game never fast-forwards through a burst.
const MAX_FRAME_SECONDS: f64 = 0.25;

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

/// Which keys feed which input bit. Y and Z both run (German and English layouts).
const KEY_MAP: [(KeyCode, InputFrame); 12] = [
    (KeyCode::Left, InputFrame::LEFT),
    (KeyCode::Right, InputFrame::RIGHT),
    (KeyCode::Up, InputFrame::UP),
    (KeyCode::Down, InputFrame::DOWN),
    (KeyCode::X, InputFrame::JUMP),
    (KeyCode::Space, InputFrame::JUMP),
    (KeyCode::Y, InputFrame::RUN),
    (KeyCode::Z, InputFrame::RUN),
    (KeyCode::LeftShift, InputFrame::RUN),
    (KeyCode::RightShift, InputFrame::RUN),
    (KeyCode::Enter, InputFrame::START),
    (KeyCode::KpEnter, InputFrame::START),
];

/// The input bits for the keys held right now.
fn input_from(held: impl Fn(KeyCode) -> bool) -> InputFrame {
    KEY_MAP
        .iter()
        .filter(|(key, _)| held(*key))
        .fold(InputFrame::NONE, |acc, &(_, bit)| acc.with(bit))
}

/// Adds this frame's time to the accumulator and returns how many fixed
/// ticks to run now (the remainder stays in the accumulator).
fn ticks_due(accumulator: &mut f64, frame_seconds: f64) -> u32 {
    let tick = 1.0 / f64::from(TICKS_PER_SECOND);
    *accumulator += frame_seconds.clamp(0.0, MAX_FRAME_SECONDS);
    let n = (*accumulator / tick).floor();
    *accumulator -= n * tick;
    n as u32
}

fn new_game() -> Game {
    let level = Level::parse(LEVEL_SRC).unwrap_or_else(|e| panic!("00_testbed.level: {e}"));
    let tuning = Tuning::parse(TUNING_SRC).unwrap_or_else(|e| panic!("tuning.toml: {e}"));
    Game::new(&level, &tuning, SEED)
}

#[macroquad::main(window_conf)]
async fn main() {
    let atlas = jnr_assets::atlas();
    let mut game = new_game();
    let mut fb = Framebuffer::new();
    let mut bytes = Vec::with_capacity(WIDTH * HEIGHT * 4);
    let mut accumulator = 0.0;
    let mut debug_overlay = false;

    render(&game, atlas, &mut fb);
    fb.write_rgba_bytes(&mut bytes);
    let texture = Texture2D::from_rgba8(WIDTH as u16, HEIGHT as u16, &bytes);
    texture.set_filter(FilterMode::Nearest);

    loop {
        if is_key_pressed(KeyCode::F1) {
            debug_overlay = !debug_overlay;
        }
        let input = input_from(is_key_down);
        for _ in 0..ticks_due(&mut accumulator, f64::from(get_frame_time())) {
            game.step(input);
        }

        render(&game, atlas, &mut fb);
        if debug_overlay {
            render_debug(&game, atlas, &mut fb);
        }
        fb.write_rgba_bytes(&mut bytes);
        texture.update_from_bytes(WIDTH as u32, HEIGHT as u32, &bytes);

        clear_background(BLACK);
        let scale = integer_scale(screen_width(), screen_height());
        let (w, h) = (WIDTH as f32 * scale, HEIGHT as f32 * scale);
        let x = ((screen_width() - w) / 2.0).floor();
        let y = ((screen_height() - h) / 2.0).floor();
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

    #[test]
    fn keys_map_to_input_bits() {
        assert_eq!(input_from(|_| false), InputFrame::NONE);
        let held = [KeyCode::Right, KeyCode::Space, KeyCode::Y];
        let input = input_from(|k| held.contains(&k));
        assert_eq!(
            input,
            InputFrame::RIGHT
                .with(InputFrame::JUMP)
                .with(InputFrame::RUN)
        );
        assert_eq!(input_from(|k| k == KeyCode::Enter), InputFrame::START);
    }

    #[test]
    fn accumulator_steps_at_60_hz() {
        let mut acc = 0.0;
        assert_eq!(ticks_due(&mut acc, 1.0 / 120.0), 0);
        assert_eq!(ticks_due(&mut acc, 1.0 / 120.0 + 1e-9), 1);
        // One second of 60 fps frames = 60 ticks.
        let mut acc = 0.0;
        let total: u32 = (0..60)
            .map(|_| ticks_due(&mut acc, 1.0 / 60.0 + 1e-12))
            .sum();
        assert_eq!(total, 60);
        // A long hitch is capped.
        let mut acc = 0.0;
        assert_eq!(ticks_due(&mut acc, 5.0), 15);
    }

    #[test]
    fn embedded_level_and_tuning_parse() {
        let game = new_game();
        assert_eq!(game.level().meta.id, "testbed");
    }
}
