//! `cargo xtask sprites {lint,preview,sheet,anim}`: art checks and review
//! images. Images go to `$CARGO_TARGET_DIR/preview/...`; paths are printed.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use jnr_artkit::{CompiledSprite, Diagnostic, Image, Project};

pub const HELP: &str = "\
cargo xtask sprites <command>

Commands:
  lint                          run the art lints (file:line findings), exit 1 on errors
  preview [filter] [--scale N]  one PNG per sprite (all frames side by side; mirrored
                                copies in a second row), default scale 4
                                -> preview/sprites/<name>.png
  sheet                         contact sheet of all sprites with names
                                -> preview/contact_sheet.png
  anim <name> [--scale N]       filmstrip (one row per animation) + one GIF per animation
                                -> preview/anim/<name>.png, preview/anim/<name>_<anim>.gif
";

/// Background of the contact sheet (neutral grey so both light and dark art read).
const SHEET_BACKGROUND: &str = "#9a9ea6";

pub fn run(root: &Path, args: &[String]) -> ExitCode {
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let result = match args.as_slice() {
        ["lint"] => {
            let (ok, text) = lint_report(root);
            print!("{text}");
            if ok { Ok(()) } else { Err(String::new()) }
        }
        ["preview", rest @ ..] => parse_opts(rest).and_then(|(filter, scale)| {
            preview(root, filter.unwrap_or(""), scale.unwrap_or(4)).map(|_| ())
        }),
        ["sheet"] => sheet(root),
        ["anim", rest @ ..] => parse_opts(rest).and_then(|opts| match opts {
            (Some(name), scale) => anim(root, name, scale.unwrap_or(4)),
            (None, _) => Err("usage: cargo xtask sprites anim <name> [--scale N]".into()),
        }),
        [] | ["help" | "-h" | "--help"] => {
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

/// `[positional] [--scale N]` in any order.
fn parse_opts<'a>(args: &[&'a str]) -> Result<(Option<&'a str>, Option<u32>), String> {
    let mut positional = None;
    let mut scale = None;
    let mut it = args.iter();
    while let Some(&a) = it.next() {
        if a == "--scale" {
            let v = it.next().ok_or("--scale needs a number")?;
            scale = Some(
                v.parse::<u32>()
                    .ok()
                    .filter(|&n| (1..=32).contains(&n))
                    .ok_or_else(|| format!("bad --scale `{v}` (1..=32)"))?,
            );
        } else if positional.is_none() && !a.starts_with('-') {
            positional = Some(a);
        } else {
            return Err(format!("unexpected argument `{a}`"));
        }
    }
    Ok((positional, scale))
}

/// Runs all lints; returns (no errors, printable text). Used by the
/// `sprites-lint` step of `check` and by `sprites lint`.
pub fn lint_report(root: &Path) -> (bool, String) {
    let (findings, sprites) = match Project::load(root) {
        Ok(p) => (p.lint(), p.sprites.len()),
        Err(errors) => (errors, 0),
    };
    let errors = findings.iter().filter(|d| d.is_error()).count();
    let warnings = findings.len() - errors;
    let mut text: String = findings.iter().map(|d| format!("{d}\n")).collect();
    text.push_str(&format!(
        "sprites lint: {errors} error(s), {warnings} warning(s) in {sprites} sprite file(s)\n"
    ));
    (errors == 0, text)
}

fn compile(root: &Path) -> Result<Vec<CompiledSprite>, String> {
    Project::load(root)
        .and_then(|p| p.compile())
        .map_err(|errs| format_errors(&errs))
}

fn format_errors(errs: &[Diagnostic]) -> String {
    let mut s = String::from("the art sources have errors:\n");
    for e in errs {
        s.push_str(&format!("{e}\n"));
    }
    s
}

/// `$CARGO_TARGET_DIR/preview/<sub>`, created.
fn preview_dir(root: &Path, sub: &str) -> Result<PathBuf, String> {
    let dir = crate::target_dir(root).join("preview").join(sub);
    fs::create_dir_all(&dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
    Ok(dir)
}

fn write_png(path: &Path, img: &Image) -> Result<(), String> {
    fs::write(path, img.encode_png()).map_err(|e| format!("write {}: {e}", path.display()))
}

/// Lays images out left to right with `gap` transparent pixels between.
fn strip(images: &[Image], gap: u32) -> Image {
    let w = images
        .iter()
        .map(|i| i.width + gap)
        .sum::<u32>()
        .saturating_sub(gap);
    let h = images.iter().map(|i| i.height).max().unwrap_or(0);
    let mut out = Image::new(w.max(1), h.max(1));
    let mut x = 0;
    for img in images {
        out.blit(img, x, 0);
        x += img.width + gap;
    }
    out
}

/// Stacks images top to bottom with `gap` transparent pixels between.
fn stack(images: &[Image], gap: u32) -> Image {
    let w = images.iter().map(|i| i.width).max().unwrap_or(0);
    let h = images
        .iter()
        .map(|i| i.height + gap)
        .sum::<u32>()
        .saturating_sub(gap);
    let mut out = Image::new(w.max(1), h.max(1));
    let mut y = 0;
    for img in images {
        out.blit(img, 0, y);
        y += img.height + gap;
    }
    out
}

/// All frames of all animations in one row; mirrored copies in a second row.
fn sprite_preview(s: &CompiledSprite) -> Image {
    let frames: Vec<Image> = s
        .anims
        .iter()
        .flat_map(|a| a.frames.iter().cloned())
        .collect();
    let mut rows = vec![strip(&frames, 1)];
    if s.mirror {
        let mirrored: Vec<Image> = frames.iter().map(Image::flipped_h).collect();
        rows.push(strip(&mirrored, 1));
    }
    stack(&rows, 1)
}

fn preview(root: &Path, filter: &str, scale: u32) -> Result<Vec<(String, PathBuf)>, String> {
    let sprites = compile(root)?;
    let dir = preview_dir(root, "sprites")?;
    let mut written = Vec::new();
    for s in sprites.iter().filter(|s| s.name.contains(filter)) {
        let path = dir.join(format!("{}.png", s.name));
        write_png(&path, &sprite_preview(s).scaled(scale))?;
        println!("{}", path.display());
        written.push((s.name.clone(), path));
    }
    if written.is_empty() {
        return Err(format!("no sprite matches `{filter}`"));
    }
    Ok(written)
}

fn sheet(root: &Path) -> Result<(), String> {
    let previews = preview(root, "", 4)?;
    let out = preview_dir(root, "")?.join("contact_sheet.png");
    let mut cmd = Command::new("montage");
    cmd.args([
        "-background",
        SHEET_BACKGROUND,
        "-fill",
        "black",
        "-pointsize",
        "14",
    ]);
    for (name, path) in &previews {
        cmd.arg("-label").arg(name).arg(path);
    }
    let cols = (previews.len() as f64).sqrt().ceil().max(1.0) as u32;
    // `1x1<` keeps every preview at its own size (no resampling of pixel art).
    cmd.args(["-tile", &format!("{cols}x"), "-geometry", "1x1<+12+12"])
        .arg(&out);
    run_tool(cmd, "montage")?;
    println!("{}", out.display());
    Ok(())
}

fn anim(root: &Path, name: &str, scale: u32) -> Result<(), String> {
    let sprites = compile(root)?;
    let s = sprites.iter().find(|s| s.name == name).ok_or_else(|| {
        let names: Vec<&str> = sprites.iter().map(|s| s.name.as_str()).collect();
        format!("unknown sprite `{name}` (known: {})", names.join(", "))
    })?;
    let dir = preview_dir(root, "anim")?;
    let frames_dir = dir.join(format!("{name}.frames"));
    fs::create_dir_all(&frames_dir).map_err(|e| format!("create {}: {e}", frames_dir.display()))?;

    let rows: Vec<Image> = s.anims.iter().map(|a| strip(&a.frames, 1)).collect();
    let strip_path = dir.join(format!("{name}.png"));
    write_png(&strip_path, &stack(&rows, 2).scaled(scale))?;
    println!("{}", strip_path.display());

    for a in &s.anims {
        let mut cmd = Command::new("convert");
        // ImageMagick accepts `ticks x ticks-per-second`; the timing is
        // approximate because GIF delays are stored in 1/100 s.
        // Held single frames (ticks = 0) are shown for one second.
        let ticks = if a.ticks == 0 { 60 } else { a.ticks };
        let delay = format!("{ticks}x60");
        cmd.args(["-dispose", "Background", "-loop", "0", "-delay", &delay]);
        for (i, f) in a.frames.iter().enumerate() {
            let p = frames_dir.join(format!("{}_{i}.png", a.name));
            write_png(&p, &f.scaled(scale))?;
            cmd.arg(&p);
        }
        let gif = dir.join(format!("{name}_{}.gif", a.name));
        cmd.arg(&gif);
        run_tool(cmd, "convert")?;
        println!(
            "{}  ({} frames, {} ticks/frame)",
            gif.display(),
            a.frames.len(),
            a.ticks
        );
    }
    Ok(())
}

fn run_tool(mut cmd: Command, name: &str) -> Result<(), String> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opts_parse_in_any_order() {
        assert_eq!(
            parse_opts(&["box", "--scale", "2"]),
            Ok((Some("box"), Some(2)))
        );
        assert_eq!(
            parse_opts(&["--scale", "3", "box"]),
            Ok((Some("box"), Some(3)))
        );
        assert_eq!(parse_opts(&[]), Ok((None, None)));
        assert!(parse_opts(&["--scale", "0"]).is_err());
        assert!(parse_opts(&["a", "b"]).is_err());
    }

    #[test]
    fn strip_and_stack_sizes() {
        let a = Image::new(2, 3);
        let b = Image::new(4, 1);
        let s = strip(&[a.clone(), b.clone()], 1);
        assert_eq!((s.width, s.height), (7, 3));
        let t = stack(&[a, b], 2);
        assert_eq!((t.width, t.height), (4, 6));
    }

    #[test]
    fn real_art_lints_clean() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let (ok, text) = lint_report(&root);
        assert!(ok, "{text}");
    }
}
