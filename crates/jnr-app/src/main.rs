//! `jnr-app`: the macroquad shell of Oblatenjagd (native + wasm).
//!
//! The only crate that knows about macroquad. It polls input, steps the
//! simulation at a fixed 60 Hz, lets `jnr-render` draw into the 320x180
//! framebuffer and uploads that framebuffer as a texture, scaled by an
//! integer factor with a black letterbox. Floats are used here only for
//! timing and scaling, never in the simulation.
//!
//! Native builds take command line flags (`--help`, see `cli.rs`); wasm
//! always starts the default level with default options.
//! Keys: arrows move, X/Space jump, Y/Z/Shift run, Enter start, M sound
//! on/off, F1 debug overlay, F12 (native debug builds) save the framebuffer as PNG, Escape
//! (native) quit. `--replay` feeds inputs from a replay file instead of the
//! keyboard; `--record` writes the played inputs as a replay on exit.

mod audio;
#[cfg(not(target_arch = "wasm32"))]
mod cli;
mod options;
#[cfg(not(target_arch = "wasm32"))]
mod record;

use jnr_core::{Game, InputFrame, TICKS_PER_SECOND, Texts, Tuning};
use jnr_render::{Framebuffer, HEIGHT, RenderContext, WIDTH, render, render_debug};
use macroquad::prelude::*;
use options::{Options, embedded_levels};

/// Gameplay constants (embedded).
const TUNING_SRC: &str = include_str!("../../../content/tuning.toml");
/// All player-visible German text (embedded).
const TEXTS_SRC: &str = include_str!("../../../content/text/de.toml");
/// Longest real time one frame may account for; more is dropped (e.g. after
/// the window was hidden), so the game never fast-forwards through a burst.
const MAX_FRAME_SECONDS: f64 = 0.25;

fn window_conf(scale: u32) -> Conf {
    Conf {
        window_title: "Oblatenjagd".to_owned(),
        window_width: (WIDTH as u32 * scale) as i32,
        window_height: (HEIGHT as u32 * scale) as i32,
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

/// Toggles the sound (both platforms, outside the game input).
const MUTE_KEY: KeyCode = KeyCode::M;

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

/// The game the options ask for: a run along the route (D-042, D-047) when
/// the level is on it (`--level st_gilgen` starts the run there), else the
/// level alone.
fn new_game(opts: &Options) -> Game {
    let levels = embedded_levels();
    let tuning = Tuning::parse(TUNING_SRC).unwrap_or_else(|e| panic!("tuning.toml: {e}"));
    match jnr_core::route::select(&levels, &opts.level, &opts.route) {
        Ok(Some((route, stage))) => Game::new_run(
            std::sync::Arc::new(route),
            stage,
            &tuning,
            opts.seed,
            opts.start,
        ),
        Ok(None) => {
            let level = levels
                .iter()
                .find(|l| l.meta.id == opts.level)
                .unwrap_or_else(|| panic!("level `{}` is not embedded", opts.level));
            Game::new_at(level, &tuning, opts.seed, opts.start)
        }
        Err(e) => panic!("route: {e}"),
    }
}

fn texts() -> Texts {
    Texts::parse(TEXTS_SRC).unwrap_or_else(|e| panic!("de.toml: {e}"))
}

/// Native: options from the command line (prints help or an error and exits).
#[cfg(not(target_arch = "wasm32"))]
fn startup_options() -> Options {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match cli::parse_args(&args) {
        Ok(cli::Command::Run(opts)) => opts,
        Ok(cli::Command::Help) => {
            print!("{}", cli::HELP);
            std::process::exit(0);
        }
        Err(e) => {
            eprintln!("jnr-app: {e}");
            std::process::exit(2);
        }
    }
}

/// Wasm: no command line, always the defaults.
#[cfg(target_arch = "wasm32")]
fn startup_options() -> Options {
    Options::default()
}

/// Writes the framebuffer to `path` and prints it; a failure ends the app
/// with exit code 1 (automation relies on the file being there).
#[cfg(not(target_arch = "wasm32"))]
fn save_screenshot(fb: &Framebuffer, path: &std::path::Path) {
    match cli::save_png(fb, path) {
        Ok(()) => println!("screenshot: {}", path.display()),
        Err(e) => {
            eprintln!("jnr-app: screenshot failed: {e}");
            std::process::exit(1);
        }
    }
}

/// Wasm never has screenshot options (no command line).
#[cfg(target_arch = "wasm32")]
fn save_screenshot(_fb: &Framebuffer, _path: &std::path::Path) {}

/// Native `--record`: the recorder, if recording.
#[cfg(not(target_arch = "wasm32"))]
type Recording = Option<record::Recorder>;
/// Wasm never records.
#[cfg(target_arch = "wasm32")]
type Recording = ();

#[cfg(not(target_arch = "wasm32"))]
fn start_recording(opts: &Options, game: &Game) -> Recording {
    opts.record
        .as_ref()
        .map(|_| record::Recorder::new(game, opts.seed, &opts.route))
}

#[cfg(target_arch = "wasm32")]
fn start_recording(_opts: &Options, _game: &Game) -> Recording {}

#[cfg(not(target_arch = "wasm32"))]
fn record_step(rec: &mut Recording, input: InputFrame, game: &Game) {
    if let Some(rec) = rec {
        let was_full = rec.is_full();
        rec.record(input, game);
        if !was_full && rec.is_full() {
            eprintln!(
                "jnr-app: warning: recording stopped at frame {} (the replay limit, {} frames = 1 h); \
                 the game goes on, the file gets the recorded part on exit",
                game.frame(),
                jnr_core::replay::MAX_TOTAL_FRAMES
            );
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn record_step(_rec: &mut Recording, _input: InputFrame, _game: &Game) {}

/// Writes the recording (if any) and prints its path; a failure ends the
/// app with exit code 1.
#[cfg(not(target_arch = "wasm32"))]
fn finish_recording(opts: &Options, rec: &Recording) {
    if let (Some(path), Some(rec)) = (&opts.record, rec) {
        match record::save(&rec.replay(), path) {
            Ok(()) => println!("recorded: {}", path.display()),
            Err(e) => {
                eprintln!("jnr-app: writing the recording failed: {e}");
                std::process::exit(1);
            }
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn finish_recording(_opts: &Options, _rec: &Recording) {}

/// Native: Escape or closing the window ends the session (so `--record`
/// can still write its file). Wasm: the page owns the lifetime.
#[cfg(not(target_arch = "wasm32"))]
fn quit_requested() -> bool {
    is_key_pressed(KeyCode::Escape) || is_quit_requested()
}

#[cfg(target_arch = "wasm32")]
fn quit_requested() -> bool {
    false
}

/// Runs the actions due after the game's current simulation frame: takes the
/// requested screenshots (of the plain frame, no overlay) and returns whether
/// the app should quit now.
fn after_frame(opts: &Options, game: &Game, ctx: &RenderContext, fb: &mut Framebuffer) -> bool {
    let actions = opts.actions_after(game.frame());
    if !actions.screenshots.is_empty() {
        render(game, ctx, fb);
        for path in actions.screenshots {
            save_screenshot(fb, path);
        }
    }
    actions.exit
}

fn main() {
    let opts = startup_options();
    // Before the window: macroquad creates the audio context (and quad-snd
    // its thread) while it sets the window up.
    #[cfg(not(target_arch = "wasm32"))]
    audio::install_panic_guard();
    macroquad::Window::from_config(window_conf(opts.scale), run(opts));
}

async fn run(opts: Options) {
    let mut game = new_game(&opts);
    let texts = texts();
    // All renderer lookups are resolved once per level (rebuilt when the
    // run moves on to the next stage).
    let atlas = jnr_assets::atlas();
    let mut ctx = RenderContext::for_level(atlas, &texts, game.level());
    let mut fb = Framebuffer::new();
    let mut bytes = Vec::with_capacity(WIDTH * HEIGHT * 4);
    let mut accumulator = 0.0;
    let mut debug_overlay = false;
    let mut recording = start_recording(&opts, &game);
    let mut audio = audio::Audio::new(opts.mute);
    if opts.test_tone {
        audio.queue_test_tone();
    }
    // Closing the window only sets a flag; the loop below ends cleanly.
    #[cfg(not(target_arch = "wasm32"))]
    prevent_quit();

    if after_frame(&opts, &game, &ctx, &mut fb) {
        finish_recording(&opts, &recording);
        return;
    }
    render(&game, &ctx, &mut fb);
    fb.write_rgba_bytes(&mut bytes);
    let texture = Texture2D::from_rgba8(WIDTH as u16, HEIGHT as u16, &bytes);
    texture.set_filter(FilterMode::Nearest);

    'frames: loop {
        if quit_requested() {
            break;
        }
        if is_key_pressed(KeyCode::F1) {
            debug_overlay = !debug_overlay;
        }
        if is_key_pressed(MUTE_KEY) {
            audio.toggle_mute();
        }
        audio.update();
        let keyboard = input_from(is_key_down);
        for _ in 0..ticks_due(&mut accumulator, f64::from(get_frame_time())) {
            let input = opts.input.input_for(game.frame(), keyboard);
            game.step(input);
            record_step(&mut recording, input, &game);
            if !ctx.is_for(game.level()) {
                ctx = RenderContext::for_level(atlas, &texts, game.level());
            }
            if after_frame(&opts, &game, &ctx, &mut fb) {
                break 'frames;
            }
        }

        render(&game, &ctx, &mut fb);
        if debug_overlay {
            render_debug(&game, &ctx, &mut fb);
        }
        // F12 saves what is on screen (including the overlay, if shown).
        #[cfg(all(debug_assertions, not(target_arch = "wasm32")))]
        if is_key_pressed(KeyCode::F12) {
            let target = std::env::var_os("CARGO_TARGET_DIR");
            save_screenshot(&fb, &cli::f12_path(target.as_deref(), game.frame()));
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
    // Returning ends the macroquad loop; the process exits with 0.
    finish_recording(&opts, &recording);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_size_follows_scale() {
        let conf = window_conf(2);
        assert_eq!((conf.window_width, conf.window_height), (640, 360));
        let conf = window_conf(Options::default().scale);
        assert_eq!((conf.window_width, conf.window_height), (1280, 720));
    }

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
    fn mute_key_is_not_a_game_key() {
        assert!(KEY_MAP.iter().all(|(key, _)| *key != MUTE_KEY));
        assert!(![KeyCode::F1, KeyCode::F12, KeyCode::Escape].contains(&MUTE_KEY));
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
    fn embedded_content_parses() {
        let game = new_game(&Options::default());
        assert_eq!(game.level().meta.id, "salzburg");
        assert_eq!(game.frame(), 0);
        assert_eq!(
            game.mode(),
            jnr_core::Mode::Title {
                frames: 0,
                from_ending: false
            },
            "starts at the title"
        );
        assert_eq!(texts().get("stage.clear"), "Etappe geschafft!");
    }

    #[test]
    fn journey_levels_start_a_run_and_test_levels_play_alone() {
        let game = new_game(&Options::default());
        assert_eq!(game.station(), Some(0), "Salzburg starts the route");
        let route: Vec<&str> = game.route().map(|r| r.ids().collect()).unwrap_or_default();
        assert_eq!(route[0], "salzburg");
        let testbed = new_game(&Options {
            level: "testbed".to_owned(),
            ..Options::default()
        });
        assert_eq!(testbed.station(), None);
    }

    #[test]
    fn seed_option_reaches_the_game() {
        let a = new_game(&Options::default());
        let b = new_game(&Options {
            seed: 99,
            ..Options::default()
        });
        assert_ne!(a.state_hash(), b.state_hash());
    }
}
