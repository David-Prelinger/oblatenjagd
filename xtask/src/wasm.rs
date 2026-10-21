//! `cargo xtask wasm` (build + assemble the web dir, optionally serve it) and
//! `cargo xtask wasm-smoke` (load it in headless Chromium and check the
//! screenshot shows the game).

use std::fs;
use std::net::{TcpListener, TcpStream};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitCode, ExitStatus, Stdio};
use std::time::{Duration, Instant};

use jnr_artkit::Palette;

use crate::target_dir;

pub const HELP: &str = "\
cargo xtask wasm [--release | --debug] [--serve] [--port N]
    builds jnr-app for wasm32-unknown-unknown (release by default) and
    assembles $CARGO_TARGET_DIR/web/ (index.html, gl.js, audio.js, jnr-app.wasm);
    --serve then serves it with `python3 -m http.server` (default port 8080)
cargo xtask wasm-smoke
    assembles the release build, loads it in headless Chromium (SwiftShader
    WebGL) and checks the screenshot $CARGO_TARGET_DIR/preview/wasm_smoke.png
    is not blank and shows the Salzburg title's parallax sky (palette `s` +
    `S` together at least 25 %, the upper `S` band at least 5 %); a probe
    page (smoke.html) checks that the blinking title prompt changes, so a
    frozen frame fails
";

/// The wasm file name the page loads (see `web/index.html`).
const WASM_NAME: &str = "jnr-app.wasm";
/// Palette character of the (lower, light) sky: the testbed's flat sky and
/// the haze band of the Salzburg parallax.
const SKY_CHAR: char = 's';
/// Palette character of the upper sky band of the Salzburg parallax.
const SKY_BLUE_CHAR: char = 'S';
/// Screenshot window size (the game fills it exactly at scale 4).
const WINDOW: (u32, u32) = (1280, 720);
/// Upper bound for one Chromium run.
const CHROMIUM_TIMEOUT: Duration = Duration::from_secs(90);
/// Per-channel tolerance when matching the sky colour.
const SKY_TOLERANCE: u8 = 2;
/// Minimum share of sky pixels (`s` + `S`) in the screenshot. The Salzburg
/// title screen the game starts on shows 36.4 % (20.4 % `s`, 16.0 % `S`;
/// the logo, rooftops, Festung and cobbles cover the rest).
const MIN_SKY_SHARE: f64 = 0.25;
/// Minimum share of the upper `S` sky band: it is only in the parallax
/// background, so a flat sky fill (or the testbed) fails.
const MIN_SKY_BLUE_SHARE: f64 = 0.05;
/// The liveness probe (see the file); served only as `smoke.js` next to
/// `smoke.html`, a copy of `index.html` that loads it.
const PROBE_JS: &str = include_str!("wasm_smoke_probe.js");
/// Game rows `[top, bottom)` of the band the probe watches: the title
/// prompt line with its outline and descenders.
const PROMPT_BAND: (i32, i32) = (
    jnr_render::screens::PRESS_START_Y - 3,
    jnr_render::screens::PRESS_START_Y + 13,
);
/// Changes of the prompt band the probe must see: at most one while the
/// title first appears, then the blink (every 30 frames, about 9 changes in
/// the 5 s virtual-time budget). A frozen game frame shows ≤ 1 (the probe drives the frame export itself, so it does not prove the page's requestAnimationFrame loop).
const MIN_PROMPT_CHANGES: u32 = 3;

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
    let dir = match build_and_assemble(root, opts.release, false) {
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

/// Builds `jnr-app` for wasm and copies page, loader and audio plugin (+
/// their licences) and module into `$CARGO_TARGET_DIR/web/`. Returns that directory.
/// `quiet` hides cargo's progress output (errors are still shown).
fn build_and_assemble(root: &Path, release: bool, quiet: bool) -> Result<PathBuf, String> {
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
    if quiet {
        cmd.arg("--quiet");
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
        (root.join("web/gl.js.LICENSE-MIT"), "gl.js.LICENSE-MIT"),
        (root.join("web/audio.js"), "audio.js"),
        (
            root.join("web/audio.js.LICENSE-MIT"),
            "audio.js.LICENSE-MIT",
        ),
        (wasm, WASM_NAME),
    ];
    for (from, name) in files {
        fs::copy(&from, dir.join(name)).map_err(|e| format!("copy {}: {e}", from.display()))?;
    }
    Ok(dir)
}

/// Kills the child process when dropped (HTTP server, Chromium). With
/// `group`, the child was started as leader of its own process group
/// (`process_group(0)`) and the whole group is killed, so Chromium's helper
/// processes do not outlive a timeout.
pub struct KillOnDrop {
    pub child: Child,
    pub group: bool,
}

impl Drop for KillOnDrop {
    fn drop(&mut self) {
        if self.group {
            let _ = Command::new("kill")
                .args(["-KILL", "--", &format!("-{}", self.child.id())])
                .stderr(Stdio::null())
                .status();
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Runs the whole smoke check; `Ok` carries a one-line summary.
/// Also the `wasm-smoke` step of `cargo xtask verify`.
pub fn smoke_check(root: &Path) -> Result<String, String> {
    let sky = [sky_colour(root)?, palette_colour(root, SKY_BLUE_CHAR)?];
    let dir = build_and_assemble(root, true, true)?;
    write_probe_page(&dir)?;
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
    let mut server = KillOnDrop {
        child: server,
        group: false,
    };
    wait_for_port(&mut server.child, port, Duration::from_secs(10))?;
    let url = format!("http://127.0.0.1:{port}/index.html");
    let probe_url = format!("http://127.0.0.1:{port}/smoke.html");

    // 1) Did the wasm module instantiate (DOM marker set by index.html), and
    //    is the game alive (the probe saw the title prompt blink)?
    let dom_log = preview.join("wasm_smoke_dom.log");
    let dom_out = preview.join("wasm_smoke_dom.html");
    run_chromium(
        &chromium,
        &preview,
        &["--dump-dom"],
        &probe_url,
        &dom_out,
        &dom_log,
    )?;
    let dom = fs::read_to_string(&dom_out).unwrap_or_default();
    let see = format!("see {} and {}", dom_out.display(), dom_log.display());
    if !dom.contains("data-wasm=\"loaded\"") {
        return Err(format!(
            "the wasm module did not instantiate (no data-wasm=\"loaded\" marker in the DOM); \
             {see}"
        ));
    }
    let liveness = check_liveness(&dom).map_err(|e| format!("{e}; {see}"))?;

    // 1b) The same without audio.js (E59): the audio loads time out and the
    //     game must keep running.
    let na_log = preview.join("wasm_smoke_no_audio.log");
    let na_out = preview.join("wasm_smoke_no_audio_dom.html");
    let na_url = format!("http://127.0.0.1:{port}/smoke_no_audio.html");
    run_chromium(
        &chromium,
        &preview,
        &["--dump-dom"],
        &na_url,
        &na_out,
        &na_log,
    )?;
    let na_dom = fs::read_to_string(&na_out).unwrap_or_default();
    let na_see = format!("see {} and {}", na_out.display(), na_log.display());
    if !na_dom.contains("data-wasm=\"loaded\"") {
        return Err(format!(
            "without audio.js the wasm module did not instantiate; {na_see}"
        ));
    }
    let na_liveness =
        check_liveness(&na_dom).map_err(|e| format!("without audio.js: {e}; {na_see}"))?;

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
        "module loaded, {liveness}, without audio.js {na_liveness}, {w}x{h} screenshot {}, {summary}",
        shot.display()
    ))
}

/// Writes `smoke.html` (`index.html` that also loads the probe) and
/// `smoke.js` (the probe with the prompt band) into the web dir `dir`.
fn write_probe_page(dir: &Path) -> Result<(), String> {
    let index = dir.join("index.html");
    let html = fs::read_to_string(&index).map_err(|e| format!("{}: {e}", index.display()))?;
    let loader = "<script src=\"gl.js\"></script>";
    if !html.contains(loader) {
        return Err(format!(
            "{} does not load gl.js with `{loader}`: update the wasm-smoke probe",
            index.display()
        ));
    }
    let page = html.replacen(
        loader,
        &format!("{loader}\n<script src=\"smoke.js\"></script>"),
        1,
    );
    let height = jnr_render::HEIGHT as f64;
    let js = format!(
        "var PROMPT_TOP = {};\nvar PROMPT_BOTTOM = {};\n{PROBE_JS}",
        f64::from(PROMPT_BAND.0) / height,
        f64::from(PROMPT_BAND.1) / height
    );
    // The same page without audio.js (D-115: the budgeted loads give up,
    // the game runs on silently).
    let audio_tag = "<script src=\"audio.js\"></script>";
    if !page.contains(audio_tag) {
        return Err(format!(
            "{} does not load audio.js with `{audio_tag}`: update the wasm-smoke probe",
            index.display()
        ));
    }
    let no_audio = page.replacen(audio_tag, "", 1);
    for (name, text) in [
        ("smoke.html", page),
        ("smoke_no_audio.html", no_audio),
        ("smoke.js", js),
    ] {
        let path = dir.join(name);
        fs::write(&path, text).map_err(|e| format!("{}: {e}", path.display()))?;
    }
    Ok(())
}

/// Reads the probe's counters from the dumped DOM and requires at least
/// [`MIN_PROMPT_CHANGES`] changes of the prompt band. Returns a summary.
fn check_liveness(dom: &str) -> Result<String, String> {
    let attr = |name: &str| -> Option<u32> {
        let start = dom.find(&format!("{name}=\""))? + name.len() + 2;
        dom[start..].split('"').next()?.parse().ok()
    };
    let Some(frames) = attr("data-smoke-frames") else {
        return Err("the liveness probe saw no game frame (no data-smoke-frames)".to_owned());
    };
    let changes = attr("data-smoke-prompt-changes").unwrap_or(0);
    if changes < MIN_PROMPT_CHANGES {
        return Err(format!(
            "frozen frame: the title prompt band changed {changes} times in {frames} frames \
             (need {MIN_PROMPT_CHANGES}: the prompt blinks every 30 frames)"
        ));
    }
    Ok(format!(
        "prompt blinked ({changes} changes in {frames} frames)"
    ))
}

/// The sky colour (`s`) from `art/palettes/main.palette`.
pub fn sky_colour(root: &Path) -> Result<[u8; 3], String> {
    palette_colour(root, SKY_CHAR)
}

/// The colour of palette character `ch` from `art/palettes/main.palette`.
fn palette_colour(root: &Path, ch: char) -> Result<[u8; 3], String> {
    let path = root.join("art/palettes/main.palette");
    let src = fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let palette = Palette::parse("main.palette", &src).map_err(|d| format!("{d:?}"))?;
    match palette.colour(ch) {
        Some(Some([r, g, b, _])) => Ok([r, g, b]),
        _ => Err(format!("palette has no colour `{ch}`")),
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
        .process_group(0)
        .spawn()
        .map_err(|e| format!("could not start {}: {e}", chromium.display()))?;
    let mut child = KillOnDrop { child, group: true };
    match wait_timeout(&mut child.child, CHROMIUM_TIMEOUT) {
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

pub fn wait_timeout(child: &mut Child, timeout: Duration) -> Option<ExitStatus> {
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

/// Waits until `server` accepts connections on `port`; fails fast if it died.
fn wait_for_port(server: &mut Child, port: u16, timeout: Duration) -> Result<(), String> {
    let started = Instant::now();
    while started.elapsed() < timeout {
        if let Ok(Some(status)) = server.try_wait() {
            return Err(format!(
                "python3 http.server exited ({status}) before listening on port {port}"
            ));
        }
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

/// Checks the screenshot is not blank (more than one colour), that at least
/// [`MIN_SKY_SHARE`] of it is one of the two sky colours `[s, S]` and at
/// least [`MIN_SKY_BLUE_SHARE`] the upper band `S`. Returns a summary.
fn analyse(rgba: &[u8], sky: [[u8; 3]; 2]) -> Result<String, String> {
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
    let share = |c: [u8; 3]| {
        let n = pixels
            .iter()
            .filter(|p| close(p[0], c[0]) && close(p[1], c[1]) && close(p[2], c[2]))
            .count();
        n as f64 / pixels.len() as f64
    };
    let (light, blue) = (share(sky[0]), share(sky[1]));
    let hex = |c: [u8; 3]| format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2]);
    let (light_hex, blue_hex) = (hex(sky[0]), hex(sky[1]));
    if light + blue < MIN_SKY_SHARE {
        return Err(format!(
            "only {:.1} % of the screenshot is sky {light_hex}/{blue_hex} (need {:.0} %)",
            (light + blue) * 100.0,
            MIN_SKY_SHARE * 100.0
        ));
    }
    if blue < MIN_SKY_BLUE_SHARE {
        return Err(format!(
            "only {:.1} % of the screenshot is the upper sky band {blue_hex} (need {:.0} %): \
             no parallax sky, not the Salzburg title?",
            blue * 100.0,
            MIN_SKY_BLUE_SHARE * 100.0
        ));
    }
    Ok(format!(
        "{:.1} % sky ({:.1} % {light_hex}, {:.1} % {blue_hex})",
        (light + blue) * 100.0,
        light * 100.0,
        blue * 100.0
    ))
}

/// The quad-snd version whose `js/audio.js` is in `web/` and whose JS
/// imports `crates/jnr-app/wasm-audio-imports.txt` lists (D-116).
#[cfg(test)]
const QUAD_SND_VERSION: &str = "0.2.8";

/// The version of `package` in a `Cargo.lock`.
#[cfg(test)]
fn locked_version(lock: &str, package: &str) -> Option<String> {
    let mut lines = lock.lines();
    while let Some(line) = lines.next() {
        if line.trim() == format!("name = \"{package}\"") {
            let version = lines.next()?.trim().strip_prefix("version = \"")?;
            return Some(version.trim_end_matches('"').to_owned());
        }
    }
    None
}

/// quad-snd's `src/web_snd.rs` from the cargo registry.
#[cfg(test)]
fn quad_snd_web_source(version: &str) -> Result<String, String> {
    let home = std::env::var_os("CARGO_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cargo")))
        .ok_or("neither CARGO_HOME nor HOME is set")?;
    let registry = home.join("registry/src");
    let entries = fs::read_dir(&registry).map_err(|e| format!("{}: {e}", registry.display()))?;
    for entry in entries.flatten() {
        let file = entry
            .path()
            .join(format!("quad-snd-{version}/src/web_snd.rs"));
        if let Ok(text) = fs::read_to_string(&file) {
            return Ok(text);
        }
    }
    Err(format!(
        "quad-snd-{version}/src/web_snd.rs not found under {} (run `cargo fetch`)",
        registry.display()
    ))
}

/// The function names declared in the first `extern "C" { ... }` block.
#[cfg(test)]
fn extern_fns(source: &str) -> Vec<String> {
    source
        .lines()
        .skip_while(|l| l.trim() != "extern \"C\" {")
        .skip(1)
        .take_while(|l| l.trim() != "}")
        .filter_map(|l| l.trim().strip_prefix("fn "))
        .filter_map(|rest| rest.split('(').next())
        .map(str::to_owned)
        .collect()
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
    const SKY_BLUE: [u8; 3] = [0x64, 0xae, 0xe6];
    const SKIES: [[u8; 3]; 2] = [SKY, SKY_BLUE];
    const GROUND: [u8; 3] = [0x5d, 0x66, 0x80];

    #[test]
    fn analyse_accepts_a_parallax_sky_scene() {
        let img = image(&[SKY, SKY, SKY_BLUE, GROUND]);
        assert_eq!(
            analyse(&img, SKIES),
            Ok("75.0 % sky (50.0 % #a6daf2, 25.0 % #64aee6)".to_owned())
        );
        // Small rounding differences still count as sky.
        let img = image(&[[0xa7, 0xd9, 0xf2], [0x65, 0xad, 0xe6], GROUND]);
        assert!(analyse(&img, SKIES).is_ok());
    }

    #[test]
    fn analyse_rejects_blank_and_skyless_screens() {
        assert!(
            analyse(&image(&[[0; 3]; 4]), SKIES)
                .unwrap_err()
                .contains("blank")
        );
        assert!(
            analyse(&image(&[SKY; 4]), SKIES)
                .unwrap_err()
                .contains("blank")
        );
        // Exactly the minimum share passes, less fails.
        assert!(analyse(&image(&[[0; 3], GROUND, GROUND, SKY_BLUE]), SKIES).is_ok());
        assert!(
            analyse(&image(&[[0; 3], GROUND, GROUND, GROUND, SKY_BLUE]), SKIES)
                .unwrap_err()
                .contains("only 20.0 %")
        );
        assert!(analyse(&[], SKIES).unwrap_err().contains("empty"));
    }

    #[test]
    fn analyse_needs_the_upper_sky_band() {
        // A flat light sky (the testbed) is not the Salzburg title.
        let mut flat = vec![SKY; 19];
        flat.push(GROUND);
        assert!(
            analyse(&image(&flat), SKIES)
                .unwrap_err()
                .contains("upper sky band")
        );
        // 1 of 20 pixels = exactly 5 % `S` passes.
        flat[0] = SKY_BLUE;
        assert!(analyse(&image(&flat), SKIES).is_ok());
    }

    #[test]
    fn sky_colour_comes_from_the_palette() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        assert_eq!(sky_colour(root), Ok(SKY));
        assert_eq!(palette_colour(root, SKY_BLUE_CHAR), Ok(SKY_BLUE));
    }

    #[test]
    fn liveness_needs_the_prompt_to_blink() {
        let dom = |attrs: &str| format!("<html><body data-wasm=\"loaded\" {attrs}></body></html>");
        assert_eq!(
            check_liveness(&dom(
                "data-smoke-frames=\"280\" data-smoke-prompt-changes=\"9\""
            )),
            Ok("prompt blinked (9 changes in 280 frames)".to_owned())
        );
        // A frozen frame: the title appeared once, then nothing changed.
        let frozen = check_liveness(&dom(
            "data-smoke-frames=\"280\" data-smoke-prompt-changes=\"1\"",
        ));
        assert!(frozen.unwrap_err().starts_with("frozen frame"));
        // No frame reached the probe at all.
        assert!(
            check_liveness(&dom(""))
                .unwrap_err()
                .contains("no game frame")
        );
    }

    #[test]
    fn probe_page_loads_the_probe_after_gl_js() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let dir = target_dir(root).join("tmp/xtask-wasm-probe-test");
        fs::create_dir_all(&dir).unwrap();
        fs::copy(root.join("web/index.html"), dir.join("index.html")).unwrap();
        write_probe_page(&dir).unwrap();
        let page = fs::read_to_string(dir.join("smoke.html")).unwrap();
        let gl = page.find("src=\"gl.js\"").unwrap();
        let probe = page.find("src=\"smoke.js\"").unwrap();
        let load = page.find("load(\"jnr-app.wasm\")").unwrap();
        assert!(gl < probe && probe < load, "{page}");
        let js = fs::read_to_string(dir.join("smoke.js")).unwrap();
        // Band 125..141 of 180 rows.
        assert!(js.starts_with("var PROMPT_TOP = 0.69"), "{js}");
        assert!(js.contains("var PROMPT_BOTTOM = 0.78"), "{js}");
        assert!(js.contains("miniquad_add_plugin"));
    }

    fn repo_root() -> &'static Path {
        Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap()
    }

    #[test]
    fn index_loads_audio_js_after_gl_js_and_before_the_module() {
        let html = fs::read_to_string(repo_root().join("web/index.html")).unwrap();
        let gl = html.find("<script src=\"gl.js\"></script>").unwrap();
        let audio = html.find("<script src=\"audio.js\"></script>").unwrap();
        let load = html.find("load(\"jnr-app.wasm\")").unwrap();
        assert!(gl < audio && audio < load, "{html}");
    }

    #[test]
    fn audio_js_matches_the_sha256_in_the_readme() {
        let root = repo_root();
        let out = Command::new("sha256sum")
            .arg(root.join("web/audio.js"))
            .output()
            .expect("sha256sum (coreutils)");
        assert!(out.status.success());
        let text = String::from_utf8(out.stdout).unwrap();
        let sha = text.split_whitespace().next().unwrap();
        let readme = fs::read_to_string(root.join("web/README.md")).unwrap();
        assert!(
            readme.contains(&format!("`{sha}`")),
            "web/audio.js has sha256 {sha}, which web/README.md does not record"
        );
        assert!(readme.contains(&format!("quad-snd {QUAD_SND_VERSION}")));
        assert!(root.join("web/audio.js.LICENSE-MIT").is_file());
    }

    #[test]
    fn audio_imports_list_matches_the_locked_quad_snd() {
        let root = repo_root();
        let lock = fs::read_to_string(root.join("Cargo.lock")).unwrap();
        assert_eq!(
            locked_version(&lock, "quad-snd").as_deref(),
            Some(QUAD_SND_VERSION),
            "quad-snd changed in Cargo.lock: re-check crates/jnr-app/wasm-audio-imports.txt \
             and web/audio.js against the new crate, then update QUAD_SND_VERSION"
        );
        let listed: Vec<String> =
            fs::read_to_string(root.join("crates/jnr-app/wasm-audio-imports.txt"))
                .unwrap()
                .lines()
                .map(str::trim)
                .filter(|l| !l.is_empty())
                .map(str::to_owned)
                .collect();
        let source = quad_snd_web_source(QUAD_SND_VERSION).unwrap_or_else(|e| panic!("{e}"));
        let declared = extern_fns(&source);
        assert_eq!(declared.len(), 9, "{declared:?}");
        assert_eq!(
            listed, declared,
            "wasm-audio-imports.txt vs quad-snd's web_snd.rs"
        );
    }

    #[test]
    fn extern_fns_reads_the_extern_block_only() {
        let src = "use x;\nextern \"C\" {\n    fn a(x: u32);\n    fn b() -> bool;\n}\n\
                   pub extern \"C\" fn c() -> u32 { 1 }\nfn d() {}\n";
        assert_eq!(extern_fns(src), ["a", "b"]);
        let lock = "[[package]]\nname = \"quad-snd\"\nversion = \"0.2.8\"\n";
        assert_eq!(locked_version(lock, "quad-snd").as_deref(), Some("0.2.8"));
        assert_eq!(locked_version(lock, "quad"), None);
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
