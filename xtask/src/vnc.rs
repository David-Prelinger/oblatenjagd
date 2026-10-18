//! `cargo xtask vnc-smoke` (L8, native): runs the real `jnr-app` window on
//! the X display (`$DISPLAY`, default `:1`) with a replay and checks
//!
//! 0. no other "Oblatenjagd" window is open (else: close the running game first),
//! 1. the app exits with code 0 within [`APP_TIMEOUT`] of its start (the
//!    build before it does not count),
//! 2. every `--screenshot-at` framebuffer it wrote is pixel-identical to the
//!    golden `tests/golden/<replay>_<shot>.png` (same renderer, real window
//!    path), mismatches go to `$CARGO_TARGET_DIR/reports/vnc-smoke/`,
//! 3. an X root-window screenshot taken while it runs
//!    (`$CARGO_TARGET_DIR/preview/vnc_smoke_root.png`) shows the game window:
//!    cropped to the window's geometry (`xwininfo`), at least
//!    [`MIN_SKY_SHARE`] of it is the palette sky colour.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitCode, ExitStatus, Stdio};
use std::time::{Duration, Instant};

use jnr_artkit::Image;
use jnr_core::replay::Shot;

use crate::golden::{Comparison, compare, read_png, write_report};
use crate::wasm::{KillOnDrop, sky_colour};
use crate::{content, target_dir};

/// The replay the smoke test plays (its shots must have goldens).
const REPLAY: &str = "tests/replays/testbed_route.replay";
/// Display used when `$DISPLAY` is unset or empty.
const DEFAULT_DISPLAY: &str = ":1";
/// Window scale passed to the app (window = 320*SCALE x 180*SCALE).
const SCALE: u32 = 4;
/// The whole app run (window, replay, exit) must finish within this.
const APP_TIMEOUT: Duration = Duration::from_secs(60);
/// How long to wait for the window to appear.
const WINDOW_TIMEOUT: Duration = Duration::from_secs(20);
/// Wait after the window appeared before the root screenshot (first frames drawn).
const SETTLE: Duration = Duration::from_millis(1500);
/// Share of the game window's area that must be sky in the root screenshot.
const MIN_SKY_SHARE: f64 = 0.25;
/// Per-channel tolerance when matching the sky colour: the noVNC desktop's
/// root window is 16 bit (RGB565), which moves 5-bit channels by up to 8.
const SKY_TOLERANCE: u8 = 8;

/// `cargo xtask vnc-smoke`
pub fn run(root: &Path, args: &[String]) -> ExitCode {
    if !args.is_empty() {
        eprintln!(
            "error: vnc-smoke takes no arguments (display: $DISPLAY, default {DEFAULT_DISPLAY})"
        );
        return ExitCode::from(2);
    }
    match smoke_check(root) {
        Ok(summary) => {
            println!("xtask vnc-smoke: PASS ({summary})");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("xtask vnc-smoke: FAIL: {e}");
            ExitCode::FAILURE
        }
    }
}

/// `$DISPLAY`, or [`DEFAULT_DISPLAY`] if unset or empty.
pub fn display(env: Option<&str>) -> String {
    match env {
        Some(d) if !d.is_empty() => d.to_owned(),
        _ => DEFAULT_DISPLAY.to_owned(),
    }
}

/// The app's command line: play `replay`, screenshot every shot into
/// `out_dir/<shot>.png`, quit one frame after the last shot.
pub fn app_args(replay: &Path, shots: &[Shot], out_dir: &Path) -> Vec<String> {
    let last = shots.iter().map(|s| s.frame).max().unwrap_or(0);
    let mut args = vec![
        "--replay".to_owned(),
        replay.display().to_string(),
        "--scale".to_owned(),
        SCALE.to_string(),
        "--exit-after".to_owned(),
        (last + 1).to_string(),
    ];
    for s in shots {
        args.push("--screenshot-at".to_owned());
        args.push(format!(
            "{}:{}",
            s.frame,
            out_dir.join(format!("{}.png", s.name)).display()
        ));
    }
    args
}

/// Runs the whole smoke test; `Ok` carries a one-line summary.
/// Also the `vnc-smoke` step of `cargo xtask verify`.
pub fn smoke_check(root: &Path) -> Result<String, String> {
    let started = Instant::now();
    let display = display(std::env::var("DISPLAY").ok().as_deref());
    check_x_server(&display)?;
    let sky = sky_colour(root)?;
    let replay_path = root.join(REPLAY);
    let replay = content::load_replay(&replay_path)?;
    if replay.shots.is_empty() {
        return Err(format!("{REPLAY} has no shots to compare"));
    }
    let name = content::replay_name(&replay_path);
    let app = build_app(root)?;

    let target = target_dir(root);
    let preview = target.join("preview");
    let out_dir = preview.join("vnc_smoke");
    let reports = target.join("reports/vnc-smoke");
    for dir in [&out_dir, &reports] {
        if dir.exists() {
            fs::remove_dir_all(dir).map_err(|e| format!("clean {}: {e}", dir.display()))?;
        }
    }
    fs::create_dir_all(&out_dir).map_err(|e| format!("{}: {e}", out_dir.display()))?;
    let root_shot = preview.join("vnc_smoke_root.png");
    let _ = fs::remove_file(&root_shot);
    let log = out_dir.join("app.log");

    // 1) Run the app; while it runs, screenshot the X root window.
    let existing = game_windows(&display)?;
    if !existing.is_empty() {
        return Err(format!(
            "a window titled Oblatenjagd is already open on {display} (id {}); \
             close the running game first",
            existing.join(", ")
        ));
    }
    let log_file = fs::File::create(&log).map_err(|e| format!("{}: {e}", log.display()))?;
    let child = Command::new(&app)
        .args(app_args(&replay_path, &replay.shots, &out_dir))
        .current_dir(root)
        .env("DISPLAY", &display)
        .stdin(Stdio::null())
        .stdout(log_file.try_clone().map_err(|e| e.to_string())?)
        .stderr(log_file)
        .spawn()
        .map_err(|e| format!("could not start {}: {e}", app.display()))?;
    // The app's own clock: the timeout must not include the build above.
    let app_started = Instant::now();
    let mut app_proc = KillOnDrop {
        child,
        group: false,
    };
    let see_log = || format!("see {}", log.display());
    let window = wait_for_window(&mut app_proc.child, &display)
        .map_err(|e| format!("{e}; {}", see_log()))?;
    std::thread::sleep(SETTLE);
    let geometry = window_geometry(&display, &window);
    let capture = capture_root(&display, &root_shot);
    let remaining = APP_TIMEOUT.saturating_sub(app_started.elapsed());
    let status =
        wait_exit(&mut app_proc.child, remaining).map_err(|e| format!("{e}; {}", see_log()))?;
    if !status.success() {
        return Err(format!("the app exited with {status}; {}", see_log()));
    }
    capture?;
    let geometry = geometry?;

    // 2) The window shows the game (sky colour inside the window's area of
    //    the root screenshot).
    let desktop = read_png(&root_shot)?;
    let in_window = crop(&desktop, geometry);
    let window_area = u64::from(geometry.width) * u64::from(geometry.height);
    let sky_summary = check_sky(&in_window, sky, window_area).map_err(|e| {
        format!(
            "{e} (window {window} at {geometry:?}, root screenshot {})",
            root_shot.display()
        )
    })?;

    // 3) The framebuffer screenshots equal the goldens.
    let mut problems = Vec::new();
    for shot in &replay.shots {
        let golden_name = format!("{name}_{}", shot.name);
        let actual_path = out_dir.join(format!("{}.png", shot.name));
        let golden = content::golden_dir(root).join(format!("{golden_name}.png"));
        if !actual_path.is_file() {
            problems.push(format!(
                "{golden_name}: the app wrote no {}",
                actual_path.display()
            ));
            continue;
        }
        let actual = read_png(&actual_path)?;
        let expected = read_png(&golden)?;
        let problem = match compare(&actual, &expected) {
            Comparison::Same => continue,
            Comparison::SizeDiffers => format!(
                "size differs: actual {}x{}, golden {}x{}",
                actual.width, actual.height, expected.width, expected.height
            ),
            Comparison::Differs {
                pixels,
                first,
                diff,
            } => {
                write_report(&reports, &golden_name, "expected", &expected)?;
                write_report(&reports, &golden_name, "diff", &diff)?;
                format!(
                    "{pixels} pixel(s) differ, first at ({}, {})",
                    first.0, first.1
                )
            }
        };
        let actual_report = write_report(&reports, &golden_name, "actual", &actual)?;
        problems.push(format!(
            "{golden_name}: {problem}; see {}",
            actual_report.display()
        ));
    }
    if !problems.is_empty() {
        return Err(format!(
            "window screenshots differ from the goldens:\n{}",
            problems.join("\n")
        ));
    }
    Ok(format!(
        "DISPLAY={display}, {} shots equal the goldens, {sky_summary}, root screenshot {}, {:.1}s",
        replay.shots.len(),
        root_shot.display(),
        started.elapsed().as_secs_f64()
    ))
}

/// Fails with a clear message if no X server answers on `display`.
fn check_x_server(display: &str) -> Result<(), String> {
    let out = Command::new("xdotool")
        .arg("getdisplaygeometry")
        .env("DISPLAY", display)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| {
            format!("could not run xdotool ({e}); install it with `sudo apt-get install xdotool`")
        })?;
    if out.status.success() {
        Ok(())
    } else {
        Err(format!(
            "no X server answers on DISPLAY={display} (is the noVNC desktop running?): {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ))
    }
}

/// Builds the native app (debug) and returns the binary.
fn build_app(root: &Path) -> Result<PathBuf, String> {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned());
    let out = Command::new(cargo)
        .args(["build", "-p", "jnr-app", "--quiet"])
        .current_dir(root)
        .env("CARGO_TERM_COLOR", "never")
        .output()
        .map_err(|e| format!("could not start cargo: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "native build of jnr-app failed:\n{}",
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    Ok(target_dir(root).join("debug/jnr-app"))
}

/// Ids of the visible windows titled exactly "Oblatenjagd". Found by title:
/// miniquad does not set `_NET_WM_PID`, so `xdotool search --pid` finds nothing.
fn game_windows(display: &str) -> Result<Vec<String>, String> {
    let out = Command::new("xdotool")
        .args(["search", "--onlyvisible", "--name", "^Oblatenjagd$"])
        .env("DISPLAY", display)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .map_err(|e| format!("could not run xdotool search: {e}"))?;
    // Exit code 1 = no match; the ids are on stdout either way.
    Ok(String::from_utf8_lossy(&out.stdout)
        .split_whitespace()
        .map(str::to_owned)
        .collect())
}

fn poll_error(e: std::io::Error) -> String {
    format!("could not poll the app process: {e}")
}

/// Polls until the app's window is visible and returns its id. Only windows
/// that appear after the start count ([`smoke_check`] refuses to run while
/// another game window is open). Fails if the app exits first or
/// [`WINDOW_TIMEOUT`] passes.
fn wait_for_window(app: &mut Child, display: &str) -> Result<String, String> {
    let started = Instant::now();
    while started.elapsed() < WINDOW_TIMEOUT {
        if let Some(status) = app.try_wait().map_err(poll_error)? {
            return Err(format!(
                "the app exited ({status}) before its window appeared"
            ));
        }
        if let Some(id) = game_windows(display)?.into_iter().next() {
            return Ok(id);
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    Err(format!(
        "no visible window named Oblatenjagd on {display} after {}s",
        WINDOW_TIMEOUT.as_secs()
    ))
}

/// Waits for the app to exit; kills it (via [`KillOnDrop`]) after `timeout`.
fn wait_exit(app: &mut Child, timeout: Duration) -> Result<ExitStatus, String> {
    let started = Instant::now();
    loop {
        if let Some(status) = app.try_wait().map_err(poll_error)? {
            return Ok(status);
        }
        if started.elapsed() >= timeout {
            return Err(format!(
                "the app did not exit within {}s of its start (killed)",
                APP_TIMEOUT.as_secs()
            ));
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// A window's client area in root-window coordinates.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Geometry {
    pub x: i64,
    pub y: i64,
    pub width: u32,
    pub height: u32,
}

/// The window's absolute geometry from `xwininfo -id`. (`xdotool
/// getwindowgeometry` adds the window manager's title bar a second time.)
fn window_geometry(display: &str, window: &str) -> Result<Geometry, String> {
    let out = Command::new("xwininfo")
        .args(["-display", display, "-id", window])
        .stdin(Stdio::null())
        .output()
        .map_err(|e| {
            format!(
                "could not run xwininfo ({e}); install it with `sudo apt-get install x11-utils`"
            )
        })?;
    if !out.status.success() {
        return Err(format!(
            "xwininfo -id {window} failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    parse_xwininfo(&String::from_utf8_lossy(&out.stdout))
}

/// Reads "Absolute upper-left X/Y", "Width" and "Height" from `xwininfo` output.
pub fn parse_xwininfo(text: &str) -> Result<Geometry, String> {
    let field = |label: &str| -> Result<i64, String> {
        text.lines()
            .find_map(|l| l.trim().strip_prefix(label))
            .and_then(|v| v.trim().parse().ok())
            .ok_or_else(|| format!("xwininfo output has no `{label}`"))
    };
    let size = |label: &str| -> Result<u32, String> {
        u32::try_from(field(label)?).map_err(|_| format!("xwininfo: bad `{label}`"))
    };
    Ok(Geometry {
        x: field("Absolute upper-left X:")?,
        y: field("Absolute upper-left Y:")?,
        width: size("Width:")?,
        height: size("Height:")?,
    })
}

/// The part of `img` covered by `g` (clipped to the image).
pub fn crop(img: &Image, g: Geometry) -> Image {
    let clip = |start: i64, len: u32, max: u32| -> (u32, u32) {
        let a = start.clamp(0, i64::from(max));
        let b = (start + i64::from(len)).clamp(0, i64::from(max));
        (a as u32, (b - a) as u32)
    };
    let (x0, w) = clip(g.x, g.width, img.width);
    let (y0, h) = clip(g.y, g.height, img.height);
    let mut out = Image::new(w, h);
    for y in 0..h {
        for x in 0..w {
            out.set(x, y, img.get(x0 + x, y0 + y));
        }
    }
    out
}

/// `import -window root` into `path`.
fn capture_root(display: &str, path: &Path) -> Result<(), String> {
    let out = Command::new("import")
        .args(["-window", "root", "-display", display])
        .arg(path)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| {
            format!("could not run import ({e}); install ImageMagick (`sudo apt-get install imagemagick`)")
        })?;
    if out.status.success() {
        Ok(())
    } else {
        Err(format!(
            "import -window root failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ))
    }
}

/// Checks that at least [`MIN_SKY_SHARE`] of `window_area` pixels of the
/// screenshot (cropped to the window) are the sky colour; returns a summary.
pub fn check_sky(window: &Image, sky: [u8; 3], window_area: u64) -> Result<String, String> {
    let close = |a: u8, b: u8| a.abs_diff(b) <= SKY_TOLERANCE;
    let sky_pixels = window
        .rgba
        .chunks_exact(4)
        .filter(|p| close(p[0], sky[0]) && close(p[1], sky[1]) && close(p[2], sky[2]))
        .count() as u64;
    let share = sky_pixels as f64 / window_area as f64;
    let hex = format!("#{:02x}{:02x}{:02x}", sky[0], sky[1], sky[2]);
    if share < MIN_SKY_SHARE {
        return Err(format!(
            "the game window is not visible: only {sky_pixels} sky {hex} pixels in the window area \
             ({:.1} % of the {window_area}-pixel window, need {:.0} %)",
            share * 100.0,
            MIN_SKY_SHARE * 100.0
        ));
    }
    Ok(format!(
        "{sky_pixels} sky {hex} pixels in the window ({:.0} % of the window)",
        share * 100.0
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_defaults_to_one() {
        assert_eq!(display(None), ":1");
        assert_eq!(display(Some("")), ":1");
        assert_eq!(display(Some(":7")), ":7");
    }

    #[test]
    fn app_args_shoot_every_shot_and_exit_after_the_last() {
        let shots = vec![
            Shot {
                frame: 0,
                name: "start".into(),
            },
            Shot {
                frame: 750,
                name: "end".into(),
            },
            Shot {
                frame: 315,
                name: "mid".into(),
            },
        ];
        let args = app_args(Path::new("r.replay"), &shots, Path::new("/o"));
        assert_eq!(
            args,
            [
                "--replay",
                "r.replay",
                "--scale",
                "4",
                "--exit-after",
                "751",
                "--screenshot-at",
                "0:/o/start.png",
                "--screenshot-at",
                "750:/o/end.png",
                "--screenshot-at",
                "315:/o/mid.png",
            ]
        );
    }

    fn desktop(sky_pixels: usize, total: usize, sky: [u8; 3]) -> Image {
        let mut img = Image::new(total as u32, 1);
        for x in 0..total {
            let px = if x < sky_pixels {
                [sky[0], sky[1].saturating_add(1), sky[2], 255]
            } else {
                [0, 0, 0, 255]
            };
            img.set(x as u32, 0, px);
        }
        img
    }

    #[test]
    fn sky_check_needs_a_quarter_of_the_window() {
        let sky = [0xa6, 0xda, 0xf2];
        assert!(check_sky(&desktop(30, 200, sky), sky, 100).is_ok());
        assert!(check_sky(&desktop(25, 200, sky), sky, 100).is_ok());
        let e = check_sky(&desktop(24, 200, sky), sky, 100).unwrap_err();
        assert!(e.contains("not visible"), "{e}");
        assert!(check_sky(&desktop(0, 200, sky), sky, 100).is_err());
    }

    #[test]
    fn xwininfo_geometry_is_parsed() {
        let text = "\nxwininfo: Window id: 0x200005 \"Oblatenjagd\"\n\n  \
                    Absolute upper-left X:  1\n  Absolute upper-left Y:  16\n  \
                    Relative upper-left X:  0\n  Relative upper-left Y:  0\n  \
                    Width: 1280\n  Height: 720\n  Depth: 16\n";
        assert_eq!(
            parse_xwininfo(text),
            Ok(Geometry {
                x: 1,
                y: 16,
                width: 1280,
                height: 720
            })
        );
        assert!(
            parse_xwininfo("Width: 3\n")
                .unwrap_err()
                .contains("Absolute")
        );
    }

    #[test]
    fn crop_takes_the_window_and_clips_to_the_screen() {
        let mut img = Image::new(4, 3);
        img.set(1, 1, [9, 9, 9, 255]);
        let inside = crop(
            &img,
            Geometry {
                x: 1,
                y: 1,
                width: 2,
                height: 2,
            },
        );
        assert_eq!((inside.width, inside.height), (2, 2));
        assert_eq!(inside.get(0, 0), [9, 9, 9, 255]);
        let clipped = crop(
            &img,
            Geometry {
                x: -2,
                y: 2,
                width: 10,
                height: 10,
            },
        );
        assert_eq!((clipped.width, clipped.height), (4, 1));
        let outside = crop(
            &img,
            Geometry {
                x: 10,
                y: 0,
                width: 5,
                height: 5,
            },
        );
        assert_eq!((outside.width, outside.height), (0, 3));
    }
}
