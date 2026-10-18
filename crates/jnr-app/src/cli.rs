//! Native-only command line and PNG output (not compiled for wasm).

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use jnr_render::Framebuffer;

use crate::options::{LEVELS, Options, level_source};

pub const HELP: &str = "\
jnr-app: Oblatenjagd

Usage: jnr-app [options]

Options:
  --level <id>                  embedded level to play (default: testbed)
  --seed <n>                    seed of the game RNG (default: 1)
  --scale <n>                   initial window size 320n x 180n, 1..=16 (default: 4)
  --exit-after <frames>         quit (exit code 0) after that many simulation frames
  --screenshot-at <frame>:<png> write the 320x180 framebuffer after that simulation
                                frame (0 = start); repeatable
  -h, --help                    show this help

Keys: arrows move, X/Space jump, Y/Z/Shift run, Enter start, F1 debug overlay,
F12 (debug builds) save the framebuffer to $CARGO_TARGET_DIR/preview/f12_<frame>.png.
";

/// Largest accepted `--scale`.
const MAX_SCALE: u32 = 16;

/// What the command line asks for.
#[derive(Debug, PartialEq, Eq)]
pub enum Command {
    Run(Options),
    Help,
}

/// Parses the arguments (without the program name).
pub fn parse_args(args: &[String]) -> Result<Command, String> {
    let mut opts = Options::default();
    let mut it = args.iter();
    while let Some(flag) = it.next() {
        let mut value = || {
            it.next()
                .map(String::as_str)
                .ok_or_else(|| format!("{flag} needs a value"))
        };
        match flag.as_str() {
            "-h" | "--help" => return Ok(Command::Help),
            "--level" => {
                let id = value()?;
                if level_source(id).is_none() {
                    return Err(format!(
                        "unknown level `{id}` (embedded levels: {})",
                        level_ids()
                    ));
                }
                opts.level = id.to_owned();
            }
            "--seed" => opts.seed = number(flag, value()?)?,
            "--scale" => {
                let scale: u32 = number(flag, value()?)?;
                if !(1..=MAX_SCALE).contains(&scale) {
                    return Err(format!("--scale must be 1..={MAX_SCALE}, got {scale}"));
                }
                opts.scale = scale;
            }
            "--exit-after" => opts.exit_after = Some(number(flag, value()?)?),
            "--screenshot-at" => {
                let spec = value()?;
                let (frame, path) = spec.split_once(':').ok_or_else(|| {
                    format!("--screenshot-at expects <frame>:<path>, got `{spec}`")
                })?;
                if path.is_empty() {
                    return Err(format!("--screenshot-at `{spec}`: path is empty"));
                }
                opts.screenshots
                    .push((number(flag, frame)?, PathBuf::from(path)));
            }
            other => return Err(format!("unknown argument `{other}` (see --help)")),
        }
    }
    if let Some(exit) = opts.exit_after {
        if let Some((frame, path)) = opts.screenshots.iter().find(|(f, _)| *f > exit) {
            return Err(format!(
                "--screenshot-at {frame}:{} is after --exit-after {exit} and would never be taken",
                path.display()
            ));
        }
    }
    Ok(Command::Run(opts))
}

/// The ids of all embedded levels, comma separated (for error messages).
fn level_ids() -> String {
    LEVELS
        .iter()
        .map(|(id, _)| *id)
        .collect::<Vec<_>>()
        .join(", ")
}

fn number<T: std::str::FromStr>(flag: &str, text: &str) -> Result<T, String> {
    text.parse()
        .map_err(|_| format!("{flag}: `{text}` is not a valid non-negative number"))
}

/// Where F12 saves a screenshot: `$CARGO_TARGET_DIR/preview/f12_<frame>.png`,
/// or `./f12_<frame>.png` if `CARGO_TARGET_DIR` is unset or empty.
pub fn f12_path(target_dir: Option<&OsStr>, frame: u32) -> PathBuf {
    let name = format!("f12_{frame}.png");
    match target_dir {
        Some(dir) if !dir.is_empty() => Path::new(dir).join("preview").join(name),
        _ => PathBuf::from(name),
    }
}

/// Encodes the framebuffer as an RGBA PNG (1x).
pub fn encode_png(fb: &Framebuffer) -> Vec<u8> {
    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut out, fb.w as u32, fb.h as u32);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        let mut writer = enc.write_header().expect("png header into Vec");
        writer
            .write_image_data(&fb.to_rgba_bytes())
            .expect("png data into Vec");
    }
    out
}

/// Writes the framebuffer as PNG to `path`, creating parent directories.
pub fn save_png(fb: &Framebuffer, path: &Path) -> Result<(), String> {
    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    std::fs::write(path, encode_png(fb)).map_err(|e| format!("{}: {e}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<Command, String> {
        let args: Vec<String> = args.iter().map(|s| (*s).to_owned()).collect();
        parse_args(&args)
    }

    fn run(args: &[&str]) -> Options {
        match parse(args) {
            Ok(Command::Run(opts)) => opts,
            other => panic!("{args:?}: {other:?}"),
        }
    }

    fn error(args: &[&str]) -> String {
        match parse(args) {
            Err(e) => e,
            other => panic!("{args:?} should fail: {other:?}"),
        }
    }

    #[test]
    fn no_args_give_defaults() {
        assert_eq!(run(&[]), Options::default());
    }

    #[test]
    fn all_flags_parse() {
        let opts = run(&[
            "--level",
            "testbed",
            "--seed",
            "42",
            "--scale",
            "2",
            "--exit-after",
            "120",
            "--screenshot-at",
            "60:/tmp/a.png",
            "--screenshot-at",
            "0:b.png",
        ]);
        assert_eq!(
            opts,
            Options {
                level: "testbed".to_owned(),
                seed: 42,
                scale: 2,
                exit_after: Some(120),
                screenshots: vec![(60, "/tmp/a.png".into()), (0, "b.png".into())],
            }
        );
    }

    #[test]
    fn help_wins() {
        assert_eq!(parse(&["--help"]), Ok(Command::Help));
        assert_eq!(parse(&["--seed", "3", "-h"]), Ok(Command::Help));
    }

    #[test]
    fn bad_input_is_reported() {
        assert!(error(&["--level", "atlantis"]).contains("embedded levels: testbed"));
        assert!(error(&["--seed"]).contains("needs a value"));
        assert!(error(&["--seed", "-1"]).contains("not a valid"));
        assert!(error(&["--scale", "0"]).contains("1..=16"));
        assert!(error(&["--scale", "17"]).contains("1..=16"));
        assert!(error(&["--screenshot-at", "60"]).contains("<frame>:<path>"));
        assert!(error(&["--screenshot-at", "60:"]).contains("path is empty"));
        assert!(error(&["--screenshot-at", "x:a.png"]).contains("not a valid"));
        assert!(error(&["--fullscreen"]).contains("unknown argument"));
        assert!(
            error(&["--exit-after", "10", "--screenshot-at", "11:a.png"])
                .contains("never be taken")
        );
    }

    #[test]
    fn screenshot_path_may_contain_colons() {
        let opts = run(&["--screenshot-at", "5:C:/shots/a.png"]);
        assert_eq!(opts.screenshots, vec![(5, "C:/shots/a.png".into())]);
    }

    #[test]
    fn f12_goes_to_target_preview() {
        assert_eq!(
            f12_path(Some(OsStr::new("/t")), 7),
            PathBuf::from("/t/preview/f12_7.png")
        );
        assert_eq!(f12_path(None, 7), PathBuf::from("f12_7.png"));
        assert_eq!(
            f12_path(Some(OsStr::new("")), 7),
            PathBuf::from("f12_7.png")
        );
    }

    #[test]
    fn png_roundtrips_the_framebuffer() {
        let mut fb = Framebuffer::new();
        fb.set(3, 2, 0x1122_33ff);
        let bytes = encode_png(&fb);
        let decoder = png::Decoder::new(bytes.as_slice());
        let mut reader = decoder.read_info().expect("valid png");
        let mut buf = vec![0; reader.output_buffer_size()];
        let info = reader.next_frame(&mut buf).expect("frame");
        assert_eq!((info.width, info.height), (320, 180));
        assert_eq!(buf, fb.to_rgba_bytes());
    }
}
