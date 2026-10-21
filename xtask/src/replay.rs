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

cargo xtask scene <level-id> --x <px> [--bg <sprite:factor,...>]
    Renders the level with the camera's left edge at x (clamped to the level);
    the player stands on the topmost floor where the camera would follow it
    -> preview/scenes/<level>_<px>.png (1x) and <level>_<px>_x4.png
    --bg sets parallax background layers back to front, each a sprite name and
    a scroll factor in sixteenths of the camera movement (0 = static sky,
    16 = with the playfield), e.g.
    --bg bg_sky_salzburg:0,bg_untersberg:3,bg_festung:8,bg_rooftops:13
    -> preview/scenes/<level>_<px>_bg.png (and _bg_x4.png)

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
    // (For the error message with the known ids.)
    content::find_level(root, &replay.level).map_err(|e| format!("{}: {e}", path.display()))?;
    // Every level must parse: a broken stage would silently cut the route.
    let levels: Vec<Level> = content::all_levels(root)?
        .into_iter()
        .map(|(p, l)| l.map_err(|e| format!("{}: {e}", p.display())))
        .collect::<Result<_, _>>()?;
    let tuning = content::tuning(root)?;
    let texts = content::texts(root)?;
    // The replay's level, on its route (D-047): a replay may change stages.
    let game = replay
        .new_game(&levels, &tuning)
        .map_err(|e| format!("{}: {e}", path.display()))?;
    let atlas = jnr_assets::atlas();
    let mut ctx = RenderContext::for_level(atlas, &texts, game.level());

    let mut hashes = Vec::new();
    let mut images = Vec::new();
    let outcome = replay.run_game(game, |g| {
        hashes.push(g.state_hash());
        if render_frames.contains(&g.frame()) {
            if !ctx.is_for(g.level()) {
                ctx = RenderContext::for_level(atlas, &texts, g.level());
            }
            images.push((g.frame(), render_image(g, &ctx)));
        }
    });
    // A freshly built game: building it must be deterministic too.
    let again_game = replay
        .new_game(&levels, &tuning)
        .map_err(|e| format!("{}: {e}", path.display()))?;
    let mut again = Vec::new();
    replay.run_game(again_game, |g| again.push(g.state_hash()));

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
    // Each replay is independent: play them on all cores (results in file order).
    let results = crate::par::map_ordered(paths, crate::par::threads(), |path| {
        play_and_shoot(root, path, shots)
    });
    for (path, result) in paths.iter().zip(results) {
        let line = match result {
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

/// Usage line of `cargo xtask scene`.
const SCENE_USAGE: &str = "usage: cargo xtask scene <level-id> --x <px> [--bg <sprite:factor,...>]";

/// Arguments of `cargo xtask scene`.
#[derive(Debug, PartialEq, Eq)]
pub struct SceneArgs {
    pub id: String,
    pub x: i32,
    /// Background layers (sprite, factor in sixteenths), back to front.
    pub bg: Vec<(String, u32)>,
}

/// `<level-id> --x <px> [--bg <list>]` in any order.
pub fn parse_scene_args(args: &[&str]) -> Result<SceneArgs, String> {
    let (mut id, mut x, mut bg) = (None, None, Vec::new());
    let mut it = args.iter();
    while let Some(&a) = it.next() {
        match a {
            "--x" => {
                let v = it.next().ok_or("--x needs a number")?;
                x = Some(
                    v.parse::<i32>()
                        .map_err(|_| format!("bad --x `{v}` (whole pixels)"))?,
                );
            }
            "--bg" => bg = parse_bg_list(it.next().ok_or("--bg needs a list")?)?,
            _ if id.is_none() && !a.starts_with('-') => id = Some(a.to_owned()),
            _ => return Err(format!("unexpected argument `{a}`")),
        }
    }
    match (id, x) {
        (Some(id), Some(x)) => Ok(SceneArgs { id, x, bg }),
        _ => Err(SCENE_USAGE.to_owned()),
    }
}

/// `sprite:factor,sprite:factor,...` (factor in sixteenths, 0..=64).
pub fn parse_bg_list(s: &str) -> Result<Vec<(String, u32)>, String> {
    s.split(',')
        .map(|item| {
            let (name, factor) = item
                .split_once(':')
                .ok_or_else(|| format!("bad --bg item `{item}`: expected sprite:factor"))?;
            let factor = factor
                .parse::<u32>()
                .ok()
                .filter(|f| *f <= 64)
                .ok_or_else(|| format!("bad factor in `{item}`: sixteenths 0..=64"))?;
            if name.is_empty() {
                return Err(format!("bad --bg item `{item}`: missing sprite name"));
            }
            Ok((name.to_owned(), factor))
        })
        .collect()
}

/// `cargo xtask scene <level-id> --x <px> [--bg <list>]`.
pub fn run_scene(root: &Path, args: &[String]) -> ExitCode {
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    if let ["help" | "-h" | "--help"] = args.as_slice() {
        return usage_ok();
    }
    match parse_scene_args(&args) {
        Ok(a) => exit(scene(root, &a)),
        Err(msg) => usage_err(&msg),
    }
}

/// A game on `level` with the camera's left edge at `camera_x` (clamped by
/// the camera) and the player standing where a following camera keeps it.
/// The player's centre is clamped so its hitbox stays inside the level;
/// the second value is true when that clamp changed it.
pub fn scene_game(level: &Level, tuning: &jnr_core::Tuning, camera_x: i32) -> (Game, bool) {
    let wanted = camera_x.saturating_add(tuning.camera.follow_x);
    // The hitbox reaches w/2 (x.5 px for an odd width) to each side of the
    // centre, so the centre stays (w + 1) / 2 whole pixels from both edges.
    let half_w = (tuning.player.hitbox_w + 1) / 2;
    let max = level.width * jnr_core::level::TILE_PX - half_w;
    let player_x = wanted.clamp(half_w, max);
    let mut game = Game::new(level, tuning, 1);
    game.teleport_player(player_x);
    (game, player_x != wanted)
}

fn scene(root: &Path, args: &SceneArgs) -> Result<(), String> {
    let (id, x) = (args.id.as_str(), args.x);
    let level = content::find_level(root, id)?;
    let tuning = content::tuning(root)?;
    let texts: Texts = content::texts(root)?;
    let atlas = jnr_assets::atlas();
    for (name, _) in &args.bg {
        if atlas.sprite(name).is_none() {
            eprintln!("warning: --bg sprite `{name}` is not in the atlas; layer skipped");
        }
    }
    // `--bg` replaces the level's own `[meta].background`.
    let ctx = if args.bg.is_empty() {
        RenderContext::for_level(atlas, &texts, &level)
    } else {
        let bg: Vec<(&str, u32, u32, u32)> = args
            .bg
            .iter()
            .map(|(n, f)| (n.as_str(), *f, 0, 0))
            .collect();
        RenderContext::new(atlas, &texts, &level).with_background(&bg)
    };
    let (game, clamped) = scene_game(&level, &tuning, x);
    if clamped {
        eprintln!(
            "warning: --x {x} puts the player outside the level ({} px wide); \
             player clamped to x={}",
            level.width * jnr_core::level::TILE_PX,
            game.player().x.to_px()
        );
    }
    let dir = preview_dir(root, "scenes")?;
    let suffix = if args.bg.is_empty() { "" } else { "_bg" };
    let name = format!("{id}_{x}{suffix}");
    let path = write_with_view_copy(&dir, &name, &render_image(&game, &ctx))?;
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
    use jnr_core::{Fx, InputFrame};

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

    /// D-036: each stage tour is the title intro, then the stage's solver
    /// route unchanged, then 800 idle frames for the Maibaum sequence (and
    /// the route map). A re-solve that changes a route must be copied into
    /// its tour.
    #[test]
    fn stage_tours_replay_the_solver_route() {
        let frames = |name: &str| -> Vec<u64> {
            let path = root().join("tests/replays").join(name);
            let replay = Replay::parse(&fs::read_to_string(&path).unwrap()).unwrap();
            replay.frames().map(|i| u64::from(i.bits())).collect()
        };
        let (none, start) = (0, u64::from(InputFrame::START.bits()));
        let intro: Vec<u64> = [vec![none; 20], vec![start], vec![none; 150]].concat();
        for stage in [
            "salzburg",
            "st_gilgen",
            "brunnwinkl",
            "falkenstein",
            "st_wolfgang",
            "wolfgangsee",
            "strobl",
            "bad_ischl",
        ] {
            let route = frames(&format!("{stage}_solver.replay"));
            let expected = [intro.clone(), route.clone(), vec![none; 800]].concat();
            let tour = frames(&format!("{stage}_tour.replay"));
            if let Some(i) = first_difference(&tour, &expected) {
                let part = if i < intro.len() {
                    "the title/intro prefix `- 20`, `S 1`, `- 150`"
                } else if i < intro.len() + route.len() {
                    "the solver route"
                } else {
                    "the Maibaum outro `- 800`"
                };
                panic!(
                    "tests/replays/{stage}_tour.replay differs at frame {i}, in {part} (tour {} \
                     frames, expected {}). The tour must be `- 20`, `S 1`, `- 150`, the inputs \
                     of tests/replays/{stage}_solver.replay unchanged, then `- 800` (D-036). \
                     If `cargo xtask solve {stage} --write` changed the route, re-copy its \
                     inputs into the tour, update its `expect` frames and re-pick the shot \
                     frames (new goldens need an art-director verdict).",
                    tour.len(),
                    expected.len()
                );
            }
        }
    }

    /// `<stage>_solver.replay` → (stage, None); `the first N frames of
    /// <stage>_solver.replay` → (stage, Some(N)).
    fn solver_route(s: &str) -> Option<(String, Option<usize>)> {
        let (limit, file) = match s.strip_prefix("the first ") {
            Some(rest) => {
                let (n, file) = rest.split_once(" frames of ")?;
                (Some(n.parse().ok()?), file)
            }
            None => (None, s),
        };
        let stage = file.strip_suffix("_solver.replay")?;
        (!stage.is_empty() && !stage.contains(' ')).then(|| (stage.to_owned(), limit))
    }

    #[test]
    fn solver_route_markers_parse() {
        assert_eq!(
            solver_route("brunnwinkl_solver.replay"),
            Some(("brunnwinkl".into(), None))
        );
        assert_eq!(
            solver_route("the first 375 frames of st_gilgen_solver.replay"),
            Some(("st_gilgen".into(), Some(375)))
        );
        assert_eq!(solver_route("the first x frames of a_solver.replay"), None);
        assert_eq!(solver_route("salzburg.replay"), None);
    }

    /// D-049: the `flow_m2_*` replays copy solver routes between marker
    /// comments, `# --- <stage>_solver.replay inputs, unchanged ---` (the
    /// whole route) or `# --- the first N frames of <stage>_solver.replay
    /// ---` (a prefix), each closed by `# --- end of ... ---`. Every block
    /// must still match its solver replay, so a re-solve says "re-copy".
    #[test]
    fn flow_m2_replays_copy_the_solver_routes() {
        let solver = |stage: &str| -> Vec<u64> {
            let name = format!("{stage}_solver.replay");
            let text = fs::read_to_string(root().join("tests/replays").join(&name)).unwrap();
            let replay = Replay::parse(&text).unwrap();
            replay.frames().map(|i| u64::from(i.bits())).collect()
        };
        let mut files: Vec<PathBuf> = fs::read_dir(root().join("tests/replays"))
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| {
                let name = p.file_name().unwrap().to_string_lossy();
                name.starts_with("flow_m2_") && name.ends_with(".replay")
            })
            .collect();
        files.sort();
        assert!(!files.is_empty(), "no flow_m2_* replays found");
        for path in files {
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            let text = fs::read_to_string(&path).unwrap();
            let mut blocks = 0;
            // The open block: (stage, frame limit, its marker line, inputs).
            let mut open: Option<(String, Option<usize>, String, String)> = None;
            for (i, line) in text.lines().enumerate() {
                let at = format!("tests/replays/{name}:{}", i + 1);
                let Some(m) = line
                    .strip_prefix("# --- ")
                    .and_then(|l| l.strip_suffix(" ---"))
                else {
                    if let Some((_, _, _, body)) = &mut open {
                        body.push_str(line);
                        body.push('\n');
                    }
                    continue;
                };
                match (m.strip_prefix("end of "), open.take()) {
                    (None, None) => {
                        let route = m
                            .strip_suffix(" inputs, unchanged")
                            .or_else(|| m.starts_with("the first ").then_some(m));
                        let (stage, limit) = route.and_then(solver_route).unwrap_or_else(|| {
                            panic!(
                                "{at}: unknown marker `{line}` (expected `<stage>_solver.replay \
                                 inputs, unchanged` or `the first N frames of \
                                 <stage>_solver.replay`)"
                            )
                        });
                        open = Some((stage, limit, line.to_owned(), String::new()));
                    }
                    (None, Some((_, _, start, _))) => {
                        panic!("{at}: `{line}` starts a block before `{start}` has ended")
                    }
                    (Some(_), None) => panic!("{at}: end without start: `{line}`"),
                    (Some(closes), Some((stage, limit, start, body))) => {
                        assert_eq!(
                            solver_route(closes),
                            Some((stage.clone(), limit)),
                            "{at}: `{line}` does not close `{start}` (same stage and frame count)"
                        );
                        let text = format!("level = x\nsource = handwritten\n---\n{body}");
                        let got: Vec<u64> = Replay::parse(&text)
                            .unwrap_or_else(|e| {
                                panic!("{at}: the block opened by `{start}` does not parse: {e}")
                            })
                            .frames()
                            .map(|i| u64::from(i.bits()))
                            .collect();
                        let route = solver(&stage);
                        let expected = match limit {
                            None => &route[..],
                            Some(n) => {
                                assert!(n <= route.len(), "{at}: {n} > the {stage} route");
                                &route[..n]
                            }
                        };
                        if let Some(i) = first_difference(&got, expected) {
                            panic!(
                                "tests/replays/{name}: the copy of {stage}_solver.replay differs at \
                                 its frame {i} ({} frames, expected {}). Re-copy the solver inputs \
                                 (D-049), update the expectations and re-pick the shot frames.",
                                got.len(),
                                expected.len()
                            );
                        }
                        blocks += 1;
                    }
                }
            }
            assert!(open.is_none(), "{name}: unclosed solver block");
            assert!(blocks > 0, "{name}: no solver block");
        }
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

    /// D-104, the Stage 9 "New mechanics" table: in a level whose
    /// `camera_top` is its bottom clamp (a fixed room camera: Café Zauner
    /// and its grey boxes) the camera's y is that value on every frame of
    /// every replay that plays it (by its `level` or `route` header), in
    /// every mode.
    #[test]
    fn fixed_room_cameras_never_move_vertically_in_any_replay() {
        let root = root();
        let levels: Vec<Level> = content::all_levels(&root)
            .unwrap()
            .into_iter()
            .map(|(_, l)| l.unwrap())
            .collect();
        let tuning = content::tuning(&root).unwrap();
        let fixed = |l: &Level| {
            l.meta
                .camera_top
                .filter(|&top| top == l.height * 16 - jnr_core::SCREEN_H)
        };
        let fixed_id = |id: &str| levels.iter().any(|l| l.meta.id == id && fixed(l).is_some());
        let mut checked = Vec::new();
        for path in all_replays(&root).unwrap() {
            let replay = content::load_replay(&path).unwrap();
            if !fixed_id(&replay.level) && !replay.route.iter().any(|id| fixed_id(id)) {
                continue;
            }
            let game = replay.new_game(&levels, &tuning).unwrap();
            let mut moved = None;
            replay.run_game(game, |g| {
                if let Some(top) = fixed(g.level())
                    && g.camera().y != top
                    && moved.is_none()
                {
                    moved = Some((g.frame(), g.camera().y));
                }
            });
            assert_eq!(moved, None, "{}: (frame, camera y)", path.display());
            checked.push(replay.level.clone());
        }
        for id in ["m7_boss", "m7_cafe"] {
            assert!(checked.iter().any(|l| l == id), "{id}: {checked:?}");
        }
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
        let (g, clamped) = scene_game(&level, &tuning, 600);
        assert!(!clamped);
        assert_eq!(g.camera().x, 600);
        assert!(g.player().on_ground);
        // Ground (row 12) under column (600 + follow_x) / 16.
        assert_eq!((g.player().y + g.player().h).to_px(), 12 * 16);
    }

    #[test]
    fn scene_args_with_and_without_backgrounds() {
        let a = parse_scene_args(&["testbed", "--x", "40"]).unwrap();
        assert_eq!((a.id.as_str(), a.x, a.bg.len()), ("testbed", 40, 0));
        let a = parse_scene_args(&["--bg", "bg_sky:0,bg_far:3", "--x", "-8", "t"]).unwrap();
        assert_eq!(a.id, "t");
        assert_eq!(a.x, -8);
        assert_eq!(a.bg, [("bg_sky".to_owned(), 0), ("bg_far".to_owned(), 3)]);
        assert!(parse_scene_args(&["t"]).unwrap_err().contains("usage"));
        assert!(
            parse_scene_args(&["t", "--x", "a"])
                .unwrap_err()
                .contains("bad --x")
        );
        assert!(parse_scene_args(&["t", "--x", "1", "u"]).is_err());
        assert!(
            parse_bg_list("bg_sky")
                .unwrap_err()
                .contains("sprite:factor")
        );
        assert!(
            parse_bg_list("bg_sky:x")
                .unwrap_err()
                .contains("sixteenths")
        );
        assert!(parse_bg_list("bg_sky:65").is_err());
        assert!(parse_bg_list(":3").unwrap_err().contains("missing sprite"));
    }

    #[test]
    fn scene_beyond_the_level_clamps_the_player_inside() {
        let root = root();
        let level = content::find_level(&root, "testbed").unwrap();
        let tuning = content::tuning(&root).unwrap();
        let level_px = level.width * jnr_core::level::TILE_PX;
        let mut odd = tuning;
        odd.player.hitbox_w = 13;
        // The real (even) hitbox and an odd one, whose edges are half pixels.
        for t in [&tuning, &odd] {
            for x in [5000, i32::MAX, -500, i32::MIN] {
                let (g, clamped) = scene_game(&level, t, x);
                assert!(clamped, "x={x}");
                let p = g.player();
                assert_eq!(p.w.to_px(), t.player.hitbox_w);
                assert!(p.x >= Fx::ZERO, "x={x}: player x={:?}", p.x);
                assert!(
                    p.x + p.w <= Fx::from_px(level_px),
                    "x={x} (w {}): player right edge {:?} > level width {level_px}",
                    t.player.hitbox_w,
                    p.x + p.w
                );
            }
        }
    }
}
