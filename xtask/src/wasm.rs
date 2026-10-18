//! `cargo xtask wasm` (build + assemble the web dir, optionally serve it) and
//! `cargo xtask wasm-smoke` (load it in headless Chromium and check the
//! screenshot shows the game).

use std::fs;
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitCode, ExitStatus, Stdio};
use std::time::{Duration, Instant};

use jnr_artkit::Palette;

use crate::target_dir;

pub const HELP: &str = "\
cargo xtask wasm [--release | --debug] [--serve] [--port N]
    builds jnr-app for wasm32-unknown-unknown (release by default) and
    assembles $CARGO_TARGET_DIR/web/ (index.html, gl.js, jnr-app.wasm);
    --serve then serves it with `python3 -m http.server` (default port 8080)
cargo xtask wasm-smoke
    assembles the release build, loads it in headless Chromium (SwiftShader
    WebGL) and checks the screenshot $CARGO_TARGET_DIR/preview/wasm_smoke.png
    is not blank and shows the sky colour (palette `s`)
";

/// The wasm file name the page loads (see `web/index.html`).
const WASM_NAME: &str = "jnr-app.wasm";
/// Palette character of the sky the testbed is drawn on.
const SKY_CHAR: char = 's';
/// Screenshot window size (the game fills it exactly at scale 4).
const WINDOW: (u32, u32) = (1280, 720);
/// Upper bound for one Chromium run.
const CHROMIUM_TIMEOUT: Duration = Duration::from_secs(90);
/// Per-channel tolerance when matching the sky colour.
const SKY_TOLERANCE: u8 = 2;
/// Minimum share of sky pixels in the screenshot (the testbed start is ~80 %).
const MIN_SKY_SHARE: f64 = 0.25;

#[derive(Debug, PartialEq, Eq)]
struct WasmOpts {
    release: bool,
    serve: bool,
    port: u16,
}

fn parse_opts(args: &[String]) -> Result<WasmOpts, String> {
    let mut opts = WasmOpts {
        release: true,
        serve: false,
        port: 8080,
    };
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--release" => opts.release = true,
            "--debug" => opts.release = false,
            "--serve" => opts.serve = true,
            "--port" => {
                let value = it.next().ok_or("--port needs a value")?;
                opts.port = value
                    .parse()
                    .map_err(|_| format!("--port: `{value}` is not a port number"))?;
            }
            other => return Err(format!("unknown argument `{other}`")),
        }
    }
    Ok(opts)
}

/// `cargo xtask wasm ...`
pub fn run(root: &Path, args: &[String]) -> ExitCode {
    let opts = match parse_opts(args) {
        Ok(opts) => opts,
        Err(e) => {
            eprintln!("error: {e}\n\n{HELP}");
            return ExitCode::from(2);
        }
    };
    let dir = match build_and_assemble(root, opts.release) {
        Ok(dir) => dir,
        Err(e) => {
            eprintln!("xtask wasm: {e}");
            return ExitCode::FAILURE;
        }
    };
    println!("xtask wasm: assembled {}", dir.display());
    if !opts.serve {
        return ExitCode::SUCCESS;
    }
    println!(
        "xtask wasm: serving on http://localhost:{}/ (Ctrl+C to stop)",
        opts.port
    );
    let status = Command::new("python3")
        .args(["-m", "http.server", &opts.port.to_string(), "--directory"])
        .arg(&dir)
        .status();
    match status {
        Ok(s) if s.success() => ExitCode::SUCCESS,
        Ok(s) => {
            eprintln!("xtask wasm: http.server exited with {s}");
            ExitCode::FAILURE
        }
        Err(e) => {
            eprintln!("xtask wasm: could not start python3: {e}");
            ExitCode::FAILURE
        }
    }
}

/// `cargo xtask wasm-smoke`
pub fn smoke(root: &Path, args: &[String]) -> ExitCode {
    if !args.is_empty() {
        eprintln!("error: wasm-smoke takes no arguments\n\n{HELP}");
        return ExitCode::from(2);
    }
    match smoke_check(root) {
        Ok(summary) => {
            println!("xtask wasm-smoke: PASS ({summary})");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("xtask wasm-smoke: FAIL: {e}");
            ExitCode::FAILURE
        }
    }
}

/// Builds `jnr-app` for wasm and copies page, loader and module into
/// `$CARGO_TARGET_DIR/web/`. Returns that directory.
fn build_and_assemble(root: &Path, release: bool) -> Result<PathBuf, String> {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned());
    let mut cmd = Command::new(cargo);
    cmd.args([
        "build",
        "-p",
        "jnr-app",
        "--target",
        "wasm32-unknown-unknown",
    ])
    .current_dir(root);
    if release {
        cmd.arg("--release");
    }
    let status = cmd
        .status()
        .map_err(|e| format!("could not start cargo: {e}"))?;
    if !status.success() {
        return Err(format!("wasm build failed ({status})"));
    }

    let target = target_dir(root);
    let profile = if release { "release" } else { "debug" };
    let wasm = target
        .join("wasm32-unknown-unknown")
        .join(profile)
        .join(WASM_NAME);
    let dir = target.join("web");
    fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let files = [
        (root.join("web/index.html"), "index.html"),
        (root.join("web/gl.js"), "gl.js"),
        (wasm, WASM_NAME),
    ];
    for (from, name) in files {
        fs::copy(&from, dir.join(name)).map_err(|e| format!("copy {}: {e}", from.display()))?;
    }
    Ok(dir)
}

/// Kills the child process when dropped (the background HTTP server).
struct KillOnDrop(Child);

impl Drop for KillOnDrop {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn smoke_check(root: &Path) -> Result<String, String> {
    let sky = sky_colour(root)?;
    let dir = build_and_assemble(root, true)?;
    let preview = target_dir(root).join("preview");
    fs::create_dir_all(&preview).map_err(|e| format!("{}: {e}", preview.display()))?;
    let shot = preview.join("wasm_smoke.png");
    let _ = fs::remove_file(&shot);
    let chromium = find_chromium().ok_or(
        "no chromium binary found (install with `sudo apt-get install chromium`, \
         see .devcontainer/Dockerfile)",
    )?;

    let port = free_port()?;
    let server = Command::new("python3")
        .args([
            "-m",
            "http.server",
            &port.to_string(),
            "--bind",
            "127.0.0.1",
            "--directory",
        ])
        .arg(&dir)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("could not start python3 http.server: {e}"))?;
    let _server = KillOnDrop(server);
    wait_for_port(port, Duration::from_secs(10))?;
    let url = format!("http://127.0.0.1:{port}/index.html");

    // 1) Did the wasm module instantiate? (DOM marker set by index.html.)
    let dom_log = preview.join("wasm_smoke_dom.log");
    let dom_out = preview.join("wasm_smoke_dom.html");
    run_chromium(
        &chromium,
        &preview,
        &["--dump-dom"],
        &url,
        &dom_out,
        &dom_log,
    )?;
    let dom = fs::read_to_string(&dom_out).unwrap_or_default();
    if !dom.contains("data-wasm=\"loaded\"") {
        return Err(format!(
            "the wasm module did not instantiate (no data-wasm=\"loaded\" marker in the DOM); \
             see {} and {}",
            dom_out.display(),
            dom_log.display()
        ));
    }

    // 2) Did it draw the game? (Screenshot.)
    let shot_log = preview.join("wasm_smoke.log");
    let shot_arg = format!("--screenshot={}", shot.display());
    run_chromium(
        &chromium,
        &preview,
        &[&shot_arg],
        &url,
        &preview.join("wasm_smoke_stdout.txt"),
        &shot_log,
    )?;
    let bytes = fs::read(&shot).map_err(|e| {
        format!(
            "no screenshot at {} ({e}); see {}",
            shot.display(),
            shot_log.display()
        )
    })?;
    let (w, h, rgba) = decode_png(&bytes)?;
    let summary = analyse(&rgba, sky).map_err(|e| {
        format!(
            "{e} (screenshot {}, log {})",
            shot.display(),
            shot_log.display()
        )
    })?;
    Ok(format!(
        "module loaded, {w}x{h} screenshot {}, {summary}",
        shot.display()
    ))
}

/// The sky colour from `art/palettes/main.palette`.
fn sky_colour(root: &Path) -> Result<[u8; 3], String> {
    let path = root.join("art/palettes/main.palette");
    let src = fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let palette = Palette::parse("main.palette", &src).map_err(|d| format!("{d:?}"))?;
    match palette.colour(SKY_CHAR) {
        Some(Some([r, g, b, _])) => Ok([r, g, b]),
        _ => Err(format!("palette has no colour `{SKY_CHAR}`")),
    }
}

fn find_chromium() -> Option<PathBuf> {
    ["chromium", "chromium-browser", "google-chrome"]
        .iter()
        .find_map(|name| {
            std::env::split_paths(&std::env::var_os("PATH")?)
                .map(|dir| dir.join(name))
                .find(|p| p.is_file())
        })
}

/// Runs headless Chromium (software WebGL via SwiftShader) on `url` with
/// `extra` flags; stdout goes to `stdout_file`, stderr to `log`.
fn run_chromium(
    chromium: &Path,
    preview: &Path,
    extra: &[&str],
    url: &str,
    stdout_file: &Path,
    log: &Path,
) -> Result<(), String> {
    let out =
        fs::File::create(stdout_file).map_err(|e| format!("{}: {e}", stdout_file.display()))?;
    let err = fs::File::create(log).map_err(|e| format!("{}: {e}", log.display()))?;
    let profile = preview.join("wasm_smoke_profile");
    let child = Command::new(chromium)
        .args([
            "--headless",
            "--no-sandbox",
            "--no-first-run",
            "--hide-scrollbars",
            "--use-gl=angle",
            "--use-angle=swiftshader",
            "--enable-unsafe-swiftshader",
            "--virtual-time-budget=5000",
            &format!("--window-size={},{}", WINDOW.0, WINDOW.1),
        ])
        .arg(format!("--user-data-dir={}", profile.display()))
        .args(extra)
        .arg(url)
        .stdin(Stdio::null())
        .stdout(out)
        .stderr(err)
        .spawn()
        .map_err(|e| format!("could not start {}: {e}", chromium.display()))?;
    let mut child = KillOnDrop(child);
    match wait_timeout(&mut child.0, CHROMIUM_TIMEOUT) {
        Some(status) if status.success() => Ok(()),
        Some(status) => Err(format!(
            "chromium exited with {status}; see {}",
            log.display()
        )),
        None => Err(format!(
            "chromium did not finish within {}s; see {}",
            CHROMIUM_TIMEOUT.as_secs(),
            log.display()
        )),
    }
}

fn wait_timeout(child: &mut Child, timeout: Duration) -> Option<ExitStatus> {
    let started = Instant::now();
    while started.elapsed() < timeout {
        match child.try_wait() {
            Ok(Some(status)) => return Some(status),
            Ok(None) => std::thread::sleep(Duration::from_millis(100)),
            Err(_) => return None,
        }
    }
    None
}

/// A currently free local TCP port.
fn free_port() -> Result<u16, String> {
    TcpListener::bind("127.0.0.1:0")
        .and_then(|l| l.local_addr())
        .map(|a| a.port())
        .map_err(|e| format!("no free port: {e}"))
}

fn wait_for_port(port: u16, timeout: Duration) -> Result<(), String> {
    let started = Instant::now();
    while started.elapsed() < timeout {
        if TcpStream::connect(("127.0.0.1", port)).is_ok() {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    Err(format!("http.server did not listen on port {port}"))
}

/// Decodes a PNG into (width, height, RGBA bytes).
fn decode_png(bytes: &[u8]) -> Result<(u32, u32, Vec<u8>), String> {
    let mut decoder = png::Decoder::new(bytes);
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info().map_err(|e| format!("bad png: {e}"))?;
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader
        .next_frame(&mut buf)
        .map_err(|e| format!("bad png: {e}"))?;
    buf.truncate(info.buffer_size());
    let rgba = match info.color_type {
        png::ColorType::Rgba => buf,
        png::ColorType::Rgb => buf
            .chunks_exact(3)
            .flat_map(|p| [p[0], p[1], p[2], 255])
            .collect(),
        other => return Err(format!("unexpected png colour type {other:?}")),
    };
    Ok((info.width, info.height, rgba))
}

/// Checks the screenshot is not blank (more than one colour) and that at
/// least [`MIN_SKY_SHARE`] of it is the sky colour. Returns a summary.
fn analyse(rgba: &[u8], sky: [u8; 3]) -> Result<String, String> {
    let pixels: Vec<&[u8]> = rgba.chunks_exact(4).collect();
    if pixels.is_empty() {
        return Err("screenshot is empty".to_owned());
    }
    let first = pixels[0];
    if pixels.iter().all(|p| p[..3] == first[..3]) {
        return Err(format!(
            "screenshot is blank (every pixel is #{:02x}{:02x}{:02x})",
            first[0], first[1], first[2]
        ));
    }
    let close = |a: u8, b: u8| a.abs_diff(b) <= SKY_TOLERANCE;
    let sky_pixels = pixels
        .iter()
        .filter(|p| close(p[0], sky[0]) && close(p[1], sky[1]) && close(p[2], sky[2]))
        .count();
    let share = sky_pixels as f64 / pixels.len() as f64;
    let hex = format!("#{:02x}{:02x}{:02x}", sky[0], sky[1], sky[2]);
    if share < MIN_SKY_SHARE {
        return Err(format!(
            "only {:.1} % of the screenshot is sky {hex} (need {:.0} %)",
            share * 100.0,
            MIN_SKY_SHARE * 100.0
        ));
    }
    Ok(format!("{:.1} % sky {hex}", share * 100.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn opts_default_to_release_without_serving() {
        assert_eq!(
            parse_opts(&[]),
            Ok(WasmOpts {
                release: true,
                serve: false,
                port: 8080
            })
        );
        assert_eq!(
            parse_opts(&args(&["--debug", "--serve", "--port", "9000"])),
            Ok(WasmOpts {
                release: false,
                serve: true,
                port: 9000
            })
        );
        assert!(parse_opts(&args(&["--release"])).unwrap().release);
        assert!(parse_opts(&args(&["--port"])).is_err());
        assert!(parse_opts(&args(&["--port", "99999"])).is_err());
        assert!(parse_opts(&args(&["--open"])).is_err());
    }

    fn image(colours: &[[u8; 3]]) -> Vec<u8> {
        colours
            .iter()
            .flat_map(|c| [c[0], c[1], c[2], 255])
            .collect()
    }

    const SKY: [u8; 3] = [0xa6, 0xda, 0xf2];
    const GROUND: [u8; 3] = [0x5d, 0x66, 0x80];

    #[test]
    fn analyse_accepts_a_sky_scene() {
        let img = image(&[SKY, SKY, SKY, GROUND]);
        assert_eq!(analyse(&img, SKY), Ok("75.0 % sky #a6daf2".to_owned()));
        // Small rounding differences still count as sky.
        let img = image(&[[0xa7, 0xd9, 0xf2], GROUND]);
        assert!(analyse(&img, SKY).is_ok());
    }

    #[test]
    fn analyse_rejects_blank_and_skyless_screens() {
        assert!(
            analyse(&image(&[[0; 3]; 4]), SKY)
                .unwrap_err()
                .contains("blank")
        );
        assert!(
            analyse(&image(&[SKY; 4]), SKY)
                .unwrap_err()
                .contains("blank")
        );
        // Exactly the minimum share passes, less fails.
        assert!(analyse(&image(&[[0; 3], GROUND, GROUND, SKY]), SKY).is_ok());
        assert!(
            analyse(&image(&[[0; 3], GROUND, GROUND, GROUND, SKY]), SKY)
                .unwrap_err()
                .contains("only 20.0 %")
        );
        assert!(analyse(&[], SKY).unwrap_err().contains("empty"));
    }

    #[test]
    fn sky_colour_comes_from_the_palette() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        assert_eq!(sky_colour(root), Ok(SKY));
    }

    #[test]
    fn png_decoding_roundtrips() {
        let mut img = jnr_artkit::Image::new(2, 1);
        img.set(0, 0, [1, 2, 3, 255]);
        let (w, h, rgba) = decode_png(&img.encode_png()).unwrap();
        assert_eq!((w, h), (2, 1));
        assert_eq!(rgba, vec![1, 2, 3, 255, 0, 0, 0, 0]);
    }
}
