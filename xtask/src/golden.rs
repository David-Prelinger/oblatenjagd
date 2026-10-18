//! Golden screenshots (L6): the `shots` of every replay, rendered headlessly
//! and compared pixel-exactly with `tests/golden/<replay>_<shot>.png`.
//!
//! - `cargo xtask golden` (and the `goldens` verify step): compare; on a
//!   mismatch or a missing golden write
//!   `$CARGO_TARGET_DIR/reports/golden/<name>_{actual,expected,diff}.png`.
//! - `cargo xtask golden --accept <name>|--all`: render the shots and write
//!   them to `tests/golden/` (only after an art-director APPROVED verdict).
//!   `<name>` is `<replay>_<shot>` or a replay name (all its shots).
//!   Rendering is deterministic, so this is exactly what was reviewed in
//!   `preview/replays/` (`cargo xtask replay <file> --shots`).

use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use jnr_artkit::Image;

use crate::content;
use crate::replay::{all_replays, play, write_png};

pub const HELP: &str = "\
cargo xtask golden                      compare every replay shot with tests/golden/
cargo xtask golden --accept <name>      write the current render of <replay>_<shot>
                                        (or all shots of a replay) to tests/golden/
cargo xtask golden --accept --all       write every shot to tests/golden/
Accept only after an art-director APPROVED verdict (docs/validation.md L6).
";

/// Colour of changed pixels in the diff image.
const DIFF_RED: [u8; 4] = [255, 0, 0, 255];
/// Unchanged pixels of the diff image are the expected image divided by this.
const DIFF_DARKEN: u8 = 3;

pub fn run(root: &Path, args: &[String]) -> ExitCode {
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let result = match args.as_slice() {
        [] => {
            let (ok, text) = check_report(root);
            print!("{text}");
            if ok { Ok(()) } else { Err(String::new()) }
        }
        ["--accept", "--all"] => accept(root, None),
        ["--accept", name] => accept(root, Some(name)),
        ["help" | "-h" | "--help"] => {
            print!("{HELP}");
            Ok(())
        }
        _ => Err(format!("unknown arguments: {}\n\n{HELP}", args.join(" "))),
    };
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

/// One rendered shot: golden name `<replay>_<shot>` and the image.
struct Rendered {
    replay: String,
    shot: String,
    name: String,
    image: Image,
    /// Expectation/determinism failures of the replay (empty = it passes).
    replay_failures: Vec<String>,
}

/// Renders the shots of every replay that has any. Two shots with the same
/// golden name (`a_b` + `c` and `a` + `b_c`) are an error.
fn render_all(root: &Path) -> Result<Vec<Rendered>, String> {
    let mut out = Vec::new();
    for path in all_replays(root)? {
        let replay = content::load_replay(&path)?;
        if replay.shots.is_empty() {
            continue;
        }
        let frames: Vec<u32> = replay.shots.iter().map(|s| s.frame).collect();
        let played = play(root, &path, &frames)?;
        for shot in &replay.shots {
            out.push(Rendered {
                replay: played.name.clone(),
                shot: shot.name.clone(),
                name: format!("{}_{}", played.name, shot.name),
                image: played.image(shot.frame).expect("shot rendered").clone(),
                replay_failures: played.failures.clone(),
            });
        }
    }
    let pairs: Vec<(&str, &str)> = out
        .iter()
        .map(|r| (r.replay.as_str(), r.shot.as_str()))
        .collect();
    match duplicate_name(&pairs) {
        Some(e) => Err(e),
        None => Ok(out),
    }
}

/// The first golden name `<replay>_<shot>` produced by two (replay, shot)
/// pairs, as an error message.
pub fn duplicate_name(pairs: &[(&str, &str)]) -> Option<String> {
    for (i, (replay, shot)) in pairs.iter().enumerate() {
        let name = format!("{replay}_{shot}");
        if let Some((r2, s2)) = pairs[..i]
            .iter()
            .find(|(r2, s2)| format!("{r2}_{s2}") == name)
        {
            return Some(format!(
                "golden name `{name}` comes from replay `{r2}` shot `{s2}` and from replay \
                 `{replay}` shot `{shot}`; rename one of the shots"
            ));
        }
    }
    None
}

/// Golden files that no current shot uses.
pub fn unused_goldens(golden_files: &[PathBuf], used_names: &[&str]) -> Vec<PathBuf> {
    golden_files
        .iter()
        .filter(|p| {
            let stem = p
                .file_stem()
                .map(|s| s.to_string_lossy())
                .unwrap_or_default();
            !used_names.contains(&stem.as_ref())
        })
        .cloned()
        .collect()
}

fn reports_golden_dir(root: &Path) -> PathBuf {
    crate::target_dir(root).join("reports/golden")
}

/// Compares every shot; returns (all equal, printable text).
pub fn check_report(root: &Path) -> (bool, String) {
    match check(root) {
        Ok((ok, text)) => (ok, text),
        Err(e) => (false, format!("error: {e}\n")),
    }
}

fn check(root: &Path) -> Result<(bool, String), String> {
    let report_dir = reports_golden_dir(root);
    if report_dir.exists() {
        fs::remove_dir_all(&report_dir)
            .map_err(|e| format!("clean {}: {e}", report_dir.display()))?;
    }
    let mut text = String::new();
    let mut failed = 0;
    let shots = render_all(root)?;
    for r in &shots {
        let golden = content::golden_dir(root).join(format!("{}.png", r.name));
        let problem = if !golden.is_file() {
            Some(format!(
                "missing golden {} (review preview/replays/{}.png from `cargo xtask replay {} --shots`; \
                 after art-director APPROVED: `cargo xtask golden --accept {}`)",
                golden.display(),
                r.name,
                r.replay,
                r.name
            ))
        } else {
            let expected = read_png(&golden)?;
            match compare(&r.image, &expected) {
                Comparison::Same => None,
                Comparison::SizeDiffers => Some(format!(
                    "size differs: actual {}x{}, golden {}x{}",
                    r.image.width, r.image.height, expected.width, expected.height
                )),
                Comparison::Differs {
                    pixels,
                    first,
                    diff,
                } => {
                    write_report(&report_dir, &r.name, "expected", &expected)?;
                    write_report(&report_dir, &r.name, "diff", &diff)?;
                    Some(format!(
                        "{pixels} pixel(s) differ, first at ({}, {})",
                        first.0, first.1
                    ))
                }
            }
        };
        match problem {
            None => text.push_str(&format!("ok   {}\n", r.name)),
            Some(p) => {
                failed += 1;
                let actual = write_report(&report_dir, &r.name, "actual", &r.image)?;
                text.push_str(&format!(
                    "FAIL {}: {p}\n     see {}\n",
                    r.name,
                    actual.display()
                ));
            }
        }
    }
    let golden_files = content::files_with_ext(&content::golden_dir(root), "png")?;
    let used: Vec<&str> = shots.iter().map(|r| r.name.as_str()).collect();
    for path in unused_goldens(&golden_files, &used) {
        failed += 1;
        text.push_str(&format!(
            "FAIL unused golden {}; delete it or re-add the shot\n",
            path.display()
        ));
    }
    text.push_str(&format!(
        "goldens: {} compared, {failed} failed\n",
        shots.len()
    ));
    Ok((failed == 0, text))
}

fn write_report(dir: &Path, name: &str, kind: &str, img: &Image) -> Result<PathBuf, String> {
    fs::create_dir_all(dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
    let path = dir.join(format!("{name}_{kind}.png"));
    write_png(&path, img)?;
    Ok(path)
}

/// `--accept <name>` (`None` = all): writes the current renders into tests/golden/.
fn accept(root: &Path, which: Option<&str>) -> Result<(), String> {
    let shots = render_all(root)?;
    let chosen: Vec<&Rendered> = shots
        .iter()
        .filter(|r| which.is_none_or(|w| r.name == w || r.replay == w))
        .collect();
    if chosen.is_empty() {
        let names: Vec<&str> = shots.iter().map(|r| r.name.as_str()).collect();
        return Err(format!(
            "no shot matches `{}` (known: {})",
            which.unwrap_or("--all"),
            names.join(", ")
        ));
    }
    refuse_failing(&chosen)?;
    let dir = content::golden_dir(root);
    fs::create_dir_all(&dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
    for r in chosen {
        let path = dir.join(format!("{}.png", r.name));
        write_png(&path, &r.image)?;
        println!("accepted {}", path.display());
    }
    Ok(())
}

/// Goldens are only accepted from replays that pass (expectations and determinism).
fn refuse_failing(chosen: &[&Rendered]) -> Result<(), String> {
    match chosen.iter().find(|r| !r.replay_failures.is_empty()) {
        None => Ok(()),
        Some(r) => Err(format!(
            "refusing to accept goldens of replay `{}`: it fails ({}); fix it first (`cargo xtask replay {}`)",
            r.replay,
            r.replay_failures.join("; "),
            r.replay
        )),
    }
}

/// Result of a pixel-exact comparison.
#[derive(Debug, PartialEq, Eq)]
pub enum Comparison {
    Same,
    SizeDiffers,
    /// `diff`: changed pixels red over the darkened expected image.
    Differs {
        pixels: usize,
        first: (u32, u32),
        diff: Image,
    },
}

/// Compares two images pixel by pixel (all four channels).
pub fn compare(actual: &Image, expected: &Image) -> Comparison {
    if (actual.width, actual.height) != (expected.width, expected.height) {
        return Comparison::SizeDiffers;
    }
    let mut diff = Image::new(expected.width, expected.height);
    let mut pixels = 0;
    let mut first = None;
    for y in 0..expected.height {
        for x in 0..expected.width {
            let e = expected.get(x, y);
            if actual.get(x, y) == e {
                diff.set(
                    x,
                    y,
                    [
                        e[0] / DIFF_DARKEN,
                        e[1] / DIFF_DARKEN,
                        e[2] / DIFF_DARKEN,
                        255,
                    ],
                );
            } else {
                pixels += 1;
                first.get_or_insert((x, y));
                diff.set(x, y, DIFF_RED);
            }
        }
    }
    match first {
        None => Comparison::Same,
        Some(first) => Comparison::Differs {
            pixels,
            first,
            diff,
        },
    }
}

/// Decodes a PNG into RGBA8 (RGB, grey and palette images are converted).
pub fn read_png(path: &Path) -> Result<Image, String> {
    let bytes = fs::read(path).map_err(|e| format!("read {}: {e}", path.display()))?;
    decode_png(&bytes).map_err(|e| format!("{}: {e}", path.display()))
}

pub fn decode_png(bytes: &[u8]) -> Result<Image, String> {
    let mut decoder = png::Decoder::new(bytes);
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().map_err(|e| format!("bad PNG: {e}"))?;
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader
        .next_frame(&mut buf)
        .map_err(|e| format!("bad PNG: {e}"))?;
    buf.truncate(info.buffer_size());
    let rgba: Vec<u8> = match info.color_type {
        png::ColorType::Rgba => buf,
        png::ColorType::Rgb => buf
            .chunks_exact(3)
            .flat_map(|p| [p[0], p[1], p[2], 255])
            .collect(),
        png::ColorType::GrayscaleAlpha => buf
            .chunks_exact(2)
            .flat_map(|p| [p[0], p[0], p[0], p[1]])
            .collect(),
        png::ColorType::Grayscale => buf.iter().flat_map(|&g| [g, g, g, 255]).collect(),
        png::ColorType::Indexed => return Err("unexpected indexed PNG after expansion".into()),
    };
    Ok(Image {
        width: info.width,
        height: info.height,
        rgba,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: [u8; 4] = [30, 60, 90, 255];
    const B: [u8; 4] = [200, 100, 0, 255];

    fn filled(w: u32, h: u32, px: [u8; 4]) -> Image {
        let mut img = Image::new(w, h);
        img.fill(px);
        img
    }

    #[test]
    fn equal_images_are_same() {
        assert_eq!(
            compare(&filled(3, 2, A), &filled(3, 2, A)),
            Comparison::Same
        );
        assert_eq!(
            compare(&filled(3, 2, A), &filled(2, 3, A)),
            Comparison::SizeDiffers
        );
    }

    #[test]
    fn diff_marks_changed_pixels_red_over_darkened_expected() {
        let expected = filled(3, 2, A);
        let mut actual = expected.clone();
        actual.set(2, 0, B);
        actual.set(1, 1, [30, 60, 90, 254]); // alpha alone counts too
        let Comparison::Differs {
            pixels,
            first,
            diff,
        } = compare(&actual, &expected)
        else {
            panic!("should differ");
        };
        assert_eq!((pixels, first), (2, (2, 0)));
        assert_eq!(diff.get(2, 0), DIFF_RED);
        assert_eq!(diff.get(1, 1), DIFF_RED);
        assert_eq!(diff.get(0, 0), [10, 20, 30, 255]);
    }

    #[test]
    fn duplicate_golden_names_are_found() {
        assert_eq!(duplicate_name(&[("a", "x"), ("a", "y"), ("b", "x")]), None);
        assert_eq!(
            duplicate_name(&[("a_b", "c"), ("a", "b_c")]).unwrap(),
            "golden name `a_b_c` comes from replay `a_b` shot `c` and from replay `a` shot `b_c`; \
             rename one of the shots"
        );
    }

    #[test]
    fn unused_goldens_are_listed() {
        let files = [PathBuf::from("g/a_x.png"), PathBuf::from("g/old_y.png")];
        assert_eq!(
            unused_goldens(&files, &["a_x", "a_z"]),
            vec![PathBuf::from("g/old_y.png")]
        );
        assert!(unused_goldens(&files, &["a_x", "old_y"]).is_empty());
    }

    #[test]
    fn accept_refuses_failing_replays() {
        let shot = |failures: Vec<String>| Rendered {
            replay: "r".into(),
            shot: "s".into(),
            name: "r_s".into(),
            image: Image::new(1, 1),
            replay_failures: failures,
        };
        let good = shot(vec![]);
        let bad = shot(vec!["not deterministic: …".into()]);
        assert_eq!(refuse_failing(&[&good]), Ok(()));
        let err = refuse_failing(&[&good, &bad]).unwrap_err();
        assert!(
            err.starts_with(
                "refusing to accept goldens of replay `r`: it fails (not deterministic"
            ),
            "{err}"
        );
    }

    #[test]
    fn png_round_trip() {
        let mut img = filled(4, 3, A);
        img.set(3, 2, [1, 2, 3, 0]);
        assert_eq!(decode_png(&img.encode_png()).unwrap(), img);
        assert!(decode_png(b"not a png").unwrap_err().starts_with("bad PNG"));
    }
}
