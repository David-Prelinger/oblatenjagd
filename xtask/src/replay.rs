//! The headless runner: `cargo xtask replay`, `filmstrip` and `scene`.
//!
//! Replays run on the real content (`content/levels`, tuning, texts) with
//! `jnr_core::Game::step`; images are rendered with `jnr-render` and the
//! real atlas (`jnr-assets`), exactly like the game. Every replay runs twice
//! and the `state_hash` of every frame is compared (determinism, L5).

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use jnr_artkit::Image;
use jnr_core::replay::Outcome;
use jnr_core::{Game, Level, Replay, Texts};
use jnr_render::{Framebuffer, RenderContext, render};

use crate::content;

pub const HELP: &str = "\
cargo xtask replay <file|name|--all> [--shots]
    Runs replays (tests/replays/*.replay with --all): checks the expectations
    and determinism (two runs, state_hash compared on every frame).
    --shots renders the `shots` frames
    -> preview/replays/<replay>_<shot>.png (1x) and <replay>_<shot>_x4.png

cargo xtask filmstrip <file|name> [--every N]
    Contact sheet of every Nth frame (default 30, plus the last frame),
    labelled with the frame number -> preview/filmstrips/<replay>.png

cargo xtask scene <level-id> --x <px>
    Renders the level with the camera's left edge at x (clamped to the level);
    the player stands on the topmost floor where the camera would follow it
    -> preview/scenes/<level>_<px>.png (1x) and <level>_<px>_x4.png

Images go to $CARGO_TARGET_DIR/preview/ (or ./target/preview).
";

/// Scale of the extra viewing copy of shots and scenes.
const VIEW_SCALE: u32 = 4;
/// Default `--every` of `filmstrip`.
const FILMSTRIP_EVERY: u32 = 30;
/// Frames per row of a filmstrip.
const FILMSTRIP_COLUMNS: u32 = 6;

/// Failure of a replay without any `expect` clause.
pub const NO_EXPECTATIONS: &str = "no expectations (add an `expect` line)";

/// A replay that was played (once for the outcome, once more for determinism).
pub struct Played {
    pub name: String,
    pub replay: Replay,
    pub outcome: Outcome,
    /// Expectation and determinism failures; empty = pass.
    pub failures: Vec<String>,
    /// Rendered frames, in frame order (only the requested ones).
    pub images: Vec<(u32, Image)>,
}

impl Played {
    /// One-line result: `PASS name: …` or `FAIL name: reason; reason`.
    pub fn summary(&self) -> String {
        if self.failures.is_empty() {
            let complete = match self.outcome.completed_at {
                Some(f) => format!("complete at {f}"),
                None => "not complete".to_owned(),
            };
            format!(
                "PASS {}: {} frames, {complete}, deaths {}, deterministic",
                self.name, self.outcome.frames, self.outcome.deaths
            )
        } else {
            format!("FAIL {}: {}", self.name, self.failures.join("; "))
        }
    }

    /// The rendered image of `frame`.
    pub fn image(&self, frame: u32) -> Option<&Image> {
        self.images
            .iter()
            .find(|(f, _)| *f == frame)
            .map(|(_, i)| i)
    }
}

/// Plays the replay at `path` twice, checks expectations and determinism,
/// and renders the frames in `render_frames`.
pub fn play(root: &Path, path: &Path, render_frames: &[u32]) -> Result<Played, String> {
    let replay = content::load_replay(path)?;
    let level =
        content::find_level(root, &replay.level).map_err(|e| format!("{}: {e}", path.display()))?;
    let tuning = content::tuning(root)?;
    let texts = content::texts(root)?;
    let ctx = RenderContext::new(jnr_assets::atlas(), &texts, &level);

    let mut hashes = Vec::new();
    let mut images = Vec::new();
    let outcome = replay.run(&level, &tuning, |g| {
        hashes.push(g.state_hash());
        if render_frames.contains(&g.frame()) {
            images.push((g.frame(), render_image(g, &ctx)));
        }
    });
    let mut again = Vec::new();
    replay.run(&level, &tuning, |g| again.push(g.state_hash()));

    let mut failures = replay.check(&outcome);
    if replay.expects.is_empty() {
        // `Replay::parse` accepts it, but a replay that checks nothing proves nothing.
        failures.push(NO_EXPECTATIONS.to_owned());
    }
    if let Some(f) = first_difference(&hashes, &again) {
        failures.push(format!(
            "not deterministic: state_hash differs between two runs first at frame {f}"
        ));
    }
    Ok(Played {
        name: content::replay_name(path),
        replay,
        outcome,
        failures,
        images,
    })
}

/// Index of the first differing element (or length mismatch).
pub fn first_difference(a: &[u64], b: &[u64]) -> Option<usize> {
    match a.iter().zip(b).position(|(x, y)| x != y) {
        Some(i) => Some(i),
        None if a.len() != b.len() => Some(a.len().min(b.len())),
        None => None,
    }
}

/// Renders one frame at 1x.
pub fn render_image(game: &Game, ctx: &RenderContext) -> Image {
    let mut fb = Framebuffer::new();
    render(game, ctx, &mut fb);
    Image {
        width: fb.w as u32,
        height: fb.h as u32,
        rgba: fb.to_rgba_bytes(),
    }
}

/// `$CARGO_TARGET_DIR/preview/<sub>`, created.
pub fn preview_dir(root: &Path, sub: &str) -> Result<PathBuf, String> {
    let dir = crate::target_dir(root).join("preview").join(sub);
    fs::create_dir_all(&dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
    Ok(dir)
}

pub fn write_png(path: &Path, img: &Image) -> Result<(), String> {
    fs::write(path, img.encode_png()).map_err(|e| format!("write {}: {e}", path.display()))
}

/// Writes `<dir>/<stem>.png` (1x) and `<dir>/<stem>_x4.png`; returns the 1x path.
pub fn write_with_view_copy(dir: &Path, stem: &str, img: &Image) -> Result<PathBuf, String> {
    let path = dir.join(format!("{stem}.png"));
    write_png(&path, img)?;
    write_png(
        &dir.join(format!("{stem}_x{VIEW_SCALE}.png")),
        &img.scaled(VIEW_SCALE),
    )?;
    Ok(path)
}

/// Every `tests/replays/*.replay`.
pub fn all_replays(root: &Path) -> Result<Vec<PathBuf>, String> {
    content::files_with_ext(&content::replays_dir(root), "replay")
}

// ------------------------------------------------------------------- commands

/// `cargo xtask replay <file|name|--all> [--shots]`.
pub fn run_replay(root: &Path, args: &[String]) -> ExitCode {
    let mut target = None;
    let mut shots = false;
    for a in args {
        match a.as_str() {
            "--shots" => shots = true,
            "help" | "-h" | "--help" => return usage_ok(),
            _ if target.is_none() => target = Some(a.as_str()),
            _ => return usage_err(&format!("unexpected argument `{a}`")),
        }
    }
    let paths = match target {
        Some("--all") => all_replays(root),
        Some(t) => content::resolve_replay(root, t).map(|p| vec![p]),
        None => return usage_err("missing replay file, name or --all"),
    };
    let result = paths.and_then(|paths| {
        let (ok, text) = run_replays(root, &paths, shots);
        print!("{text}");
        if ok { Ok(()) } else { Err(String::new()) }
    });
    exit(result)
}

/// Runs the replays; returns (all passed, one line per replay plus written shots).
/// No replay files at all is a failure (the verify step would prove nothing).
pub fn run_replays(root: &Path, paths: &[PathBuf], shots: bool) -> (bool, String) {
    if paths.is_empty() {
        return (
            false,
            "FAIL: no replay files in tests/replays (expected at least one *.replay)\n".to_owned(),
        );
    }
    let mut ok = true;
    let mut text = String::new();
    for path in paths {
        let line = match play_and_shoot(root, path, shots) {
            Ok((played, written)) => {
                ok &= played.failures.is_empty();
                let mut s = format!("{}\n", played.summary());
                for p in written {
                    s.push_str(&format!("  shot {}\n", p.display()));
                }
                s
            }
            Err(e) => {
                ok = false;
                format!("FAIL {}: {e}\n", content::replay_name(path))
            }
        };
        text.push_str(&line);
    }
    text.push_str(&format!(
        "replays: {} run, {}\n",
        paths.len(),
        if ok { "all passed" } else { "FAILED" }
    ));
    (ok, text)
}

fn play_and_shoot(root: &Path, path: &Path, shots: bool) -> Result<(Played, Vec<PathBuf>), String> {
    let frames = if shots {
        let replay = content::load_replay(path)?;
        replay.shots.iter().map(|s| s.frame).collect()
    } else {
        Vec::new()
    };
    let played = play(root, path, &frames)?;
    let mut written = Vec::new();
    if shots {
        let dir = preview_dir(root, "replays")?;
        for shot in &played.replay.shots {
            let img = played.image(shot.frame).expect("shot frames are rendered");
            let stem = format!("{}_{}", played.name, shot.name);
            written.push(write_with_view_copy(&dir, &stem, img)?);
        }
    }
    Ok((played, written))
}

/// `cargo xtask filmstrip <file|name> [--every N]`.
pub fn run_filmstrip(root: &Path, args: &[String]) -> ExitCode {
    let mut target = None;
    let mut every = FILMSTRIP_EVERY;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--every" => match it
                .next()
                .and_then(|v| v.parse::<u32>().ok())
                .filter(|&n| n >= 1)
            {
                Some(n) => every = n,
                None => return usage_err("--every needs a whole number ≥ 1"),
            },
            "help" | "-h" | "--help" => return usage_ok(),
            _ if target.is_none() => target = Some(a.as_str()),
            _ => return usage_err(&format!("unexpected argument `{a}`")),
        }
    }
    let Some(target) = target else {
        return usage_err("missing replay file or name");
    };
    exit(content::resolve_replay(root, target).and_then(|p| filmstrip(root, &p, every)))
}

/// Frames 0, N, 2N, … plus the last frame.
pub fn filmstrip_frames(total: u32, every: u32) -> Vec<u32> {
    let mut frames: Vec<u32> = (0..=total).step_by(every.max(1) as usize).collect();
    if frames.last() != Some(&total) {
        frames.push(total);
    }
    frames
}

fn filmstrip(root: &Path, path: &Path, every: u32) -> Result<(), String> {
    let replay = content::load_replay(path)?;
    let frames = filmstrip_frames(replay.frame_count(), every);
    let played = play(root, path, &frames)?;
    println!("{}", played.summary());

    let dir = preview_dir(root, "filmstrips")?;
    let frames_dir = dir.join(format!("{}.frames", played.name));
    if frames_dir.exists() {
        fs::remove_dir_all(&frames_dir)
            .map_err(|e| format!("clean {}: {e}", frames_dir.display()))?;
    }
    fs::create_dir_all(&frames_dir).map_err(|e| format!("create {}: {e}", frames_dir.display()))?;

    let out = dir.join(format!("{}.png", played.name));
    let mut cmd = Command::new("montage");
    cmd.args([
        "-background",
        "#333333",
        "-fill",
        "white",
        "-pointsize",
        "12",
    ]);
    for (frame, img) in &played.images {
        let p = frames_dir.join(format!("{frame:06}.png"));
        write_png(&p, img)?;
        cmd.arg("-label").arg(format!("frame {frame}")).arg(&p);
    }
    // `1x1<` keeps every frame at 1x (no resampling of pixel art).
    cmd.args([
        "-tile",
        &format!("{FILMSTRIP_COLUMNS}x"),
        "-geometry",
        "1x1<+4+4",
    ])
    .arg(&out);
    run_tool(cmd, "montage")?;
    println!("{}", out.display());
    Ok(())
}

/// `cargo xtask scene <level-id> --x <px>`.
pub fn run_scene(root: &Path, args: &[String]) -> ExitCode {
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let (id, x) = match args.as_slice() {
        [id, "--x", x] | ["--x", x, id] => match x.parse::<i32>() {
            Ok(x) => (*id, x),
            Err(_) => return usage_err(&format!("bad --x `{x}` (whole pixels)")),
        },
        ["help" | "-h" | "--help"] => return usage_ok(),
        _ => return usage_err("usage: cargo xtask scene <level-id> --x <px>"),
    };
    exit(scene(root, id, x))
}

/// A game on `level` with the camera's left edge at `camera_x` (clamped by
/// the camera) and the player standing where a following camera keeps it.
pub fn scene_game(level: &Level, tuning: &jnr_core::Tuning, camera_x: i32) -> Game {
    let mut game = Game::new(level, tuning, 1);
    game.teleport_player(camera_x + tuning.camera.follow_x);
    game
}

fn scene(root: &Path, id: &str, x: i32) -> Result<(), String> {
    let level = content::find_level(root, id)?;
    let tuning = content::tuning(root)?;
    let texts: Texts = content::texts(root)?;
    let ctx = RenderContext::new(jnr_assets::atlas(), &texts, &level);
    let game = scene_game(&level, &tuning, x);
    let dir = preview_dir(root, "scenes")?;
    let path = write_with_view_copy(&dir, &format!("{id}_{x}"), &render_image(&game, &ctx))?;
    println!(
        "camera x={} (player x={}) -> {}",
        game.camera().x,
        game.player().x.to_px(),
        path.display()
    );
    Ok(())
}

// -------------------------------------------------------------------- helpers

pub fn run_tool(mut cmd: Command, name: &str) -> Result<(), String> {
    let out = cmd
        .output()
        .map_err(|e| format!("cannot run `{name}` (ImageMagick installed?): {e}"))?;
    if out.status.success() {
        Ok(())
    } else {
        Err(format!(
            "`{name}` failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ))
    }
}

fn usage_ok() -> ExitCode {
    print!("{HELP}");
    ExitCode::SUCCESS
}

fn usage_err(msg: &str) -> ExitCode {
    eprintln!("error: {msg}\n\n{HELP}");
    ExitCode::from(2)
}

fn exit(result: Result<(), String>) -> ExitCode {
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(msg) => {
            if !msg.is_empty() {
                eprintln!("error: {msg}");
            }
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("..")
    }

    #[test]
    fn filmstrip_frames_include_start_and_end() {
        assert_eq!(filmstrip_frames(90, 30), vec![0, 30, 60, 90]);
        assert_eq!(filmstrip_frames(95, 30), vec![0, 30, 60, 90, 95]);
        assert_eq!(filmstrip_frames(0, 30), vec![0]);
    }

    #[test]
    fn first_difference_finds_divergence() {
        assert_eq!(first_difference(&[1, 2, 3], &[1, 2, 3]), None);
        assert_eq!(first_difference(&[1, 2, 3], &[1, 9, 3]), Some(1));
        assert_eq!(first_difference(&[1, 2], &[1, 2, 3]), Some(2));
    }

    #[test]
    fn real_replays_pass_and_render_shots() {
        let root = root();
        let path = root.join("tests/replays/testbed_route.replay");
        let played = play(&root, &path, &[0, 731]).unwrap();
        assert_eq!(played.failures, Vec::<String>::new());
        assert_eq!(played.outcome.completed_at, Some(731));
        let img = played.image(731).unwrap();
        assert_eq!((img.width, img.height), (320, 180));
        assert!(played.image(1).is_none());
        let (ok, text) = run_replays(&root, &all_replays(&root).unwrap(), false);
        assert!(ok, "{text}");
    }

    #[test]
    fn broken_expectation_fails_with_reason() {
        let root = root();
        let dir = crate::target_dir(&root).join("tmp/xtask-replay-test");
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("bad.replay");
        fs::write(
            &path,
            "level = testbed\nsource = handwritten\nexpect = complete frames=1..2 state=stage_clear\n---\nR 30\n",
        )
        .unwrap();
        let played = play(&root, &path, &[]).unwrap();
        assert_eq!(played.failures.len(), 2, "{:?}", played.failures);
        assert!(
            played
                .summary()
                .starts_with("FAIL bad: expected complete frames=1..2, never completed")
        );
        let (ok, text) = run_replays(&root, &[path], false);
        assert!(!ok);
        assert!(text.contains("FAILED"), "{text}");
    }

    #[test]
    fn replay_without_expectations_fails() {
        let root = root();
        let dir = crate::target_dir(&root).join("tmp/xtask-replay-test");
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("no_expect.replay");
        fs::write(&path, "level = testbed\nsource = handwritten\n---\nR 10\n").unwrap();
        let played = play(&root, &path, &[]).unwrap();
        assert_eq!(played.failures, vec![NO_EXPECTATIONS.to_owned()]);
        assert_eq!(
            played.summary(),
            "FAIL no_expect: no expectations (add an `expect` line)"
        );
    }

    #[test]
    fn no_replay_files_fails() {
        let (ok, text) = run_replays(&root(), &[], false);
        assert!(!ok);
        assert!(text.contains("no replay files"), "{text}");
    }

    #[test]
    fn scene_puts_camera_at_x_and_player_on_the_ground() {
        let root = root();
        let level = content::find_level(&root, "testbed").unwrap();
        let tuning = content::tuning(&root).unwrap();
        let g = scene_game(&level, &tuning, 600);
        assert_eq!(g.camera().x, 600);
        assert!(g.player().on_ground);
        // Ground (row 12) under column (600 + follow_x) / 16.
        assert_eq!((g.player().y + g.player().h).to_px(), 12 * 16);
    }
}
