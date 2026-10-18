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
  preview [filter] [--scale N]  one PNG per sprite (frames side by side, at most 16 or
                                ~1600 px per row; mirrored copies below), default scale 4
                                -> preview/sprites/<name>.png
  sheet [--group <name>]        contact sheet of all sprites (or one group) with names,
                                at most ~2000 px wide
                                -> preview/contact_sheet.png (contact_sheet_<group>.png)
  anim <name> [--scale N]       filmstrip (one row per animation) + one GIF per animation
                                -> preview/anim/<name>.png, preview/anim/<name>_<anim>.gif
";

/// Background of the contact sheet (neutral grey so both light and dark art read).
const SHEET_BACKGROUND: &str = "#9a9ea6";
/// Most frames side by side in one preview row.
const MAX_FRAMES_PER_ROW: u32 = 16;
/// Preview rows wrap before they get wider than this (scaled pixels).
const MAX_PREVIEW_ROW_PX: u32 = 1600;
/// The contact sheet's rows wrap before they get wider than this.
const MAX_SHEET_W: u32 = 2000;
/// Padding around each sheet cell.
const SHEET_PAD: u32 = 12;
/// Point size of the sheet labels.
const LABEL_POINTSIZE: &str = "14";
/// Palette colour behind sprites of [`DARK_GROUPS`] (white UI glyphs).
const DARK_BACKGROUND: char = 'B';
/// Sprite groups previewed on [`DARK_BACKGROUND`].
const DARK_GROUPS: [&str; 1] = ["ui"];

pub fn run(root: &Path, args: &[String]) -> ExitCode {
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let result = match args.as_slice() {
        ["lint"] => {
            let (ok, text) = lint_report(root);
            print!("{text}");
            if ok { Ok(()) } else { Err(String::new()) }
        }
        ["preview", rest @ ..] => parse_opts(rest).and_then(|(filter, scale)| {
            preview(root, filter.unwrap_or(""), None, scale.unwrap_or(4)).map(|_| ())
        }),
        ["sheet"] => sheet(root, None),
        ["sheet", "--group", group] => sheet(root, Some(group)),
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
        "sprites lint: {errors} error(s), {warnings} warning(s) in {sprites} sprite(s) (hand-made + generated)\n"
    ));
    (errors == 0, text)
}

/// A compiled sprite with what the previews need to know about it.
struct Sheet {
    sprite: CompiledSprite,
    /// Background for the preview, if the group needs one.
    background: Option<[u8; 4]>,
}

/// Every sprite (base + variants), sorted by name. Fails on any lint error.
fn compile(root: &Path) -> Result<Vec<Sheet>, String> {
    let p = Project::load(root).map_err(|errs| format_errors(&errs))?;
    // `Project::compile` fails on lint errors and sorts by name.
    let dark = p.palette.colour(DARK_BACKGROUND).flatten();
    let sprites = p.compile().map_err(|e| format_errors(&e))?;
    Ok(sprites
        .into_iter()
        .map(|sprite| Sheet {
            background: dark.filter(|_| DARK_GROUPS.contains(&sprite.group.as_str())),
            sprite,
        })
        .collect())
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

/// How many frames of `frame_w` px (plus a 1 px gap) fit in one preview
/// row at `scale`: at most [`MAX_FRAMES_PER_ROW`] and about
/// [`MAX_PREVIEW_ROW_PX`] scaled pixels, at least 1.
fn frames_per_row(frame_w: u32, scale: u32) -> u32 {
    let per_frame = (frame_w + 1) * scale.max(1);
    (MAX_PREVIEW_ROW_PX / per_frame).clamp(1, MAX_FRAMES_PER_ROW)
}

/// All frames of all animations, wrapped into rows of
/// [`frames_per_row`]; the mirrored copies (if any) follow in the same
/// layout. Unscaled, on `background` if given.
fn sprite_preview(s: &CompiledSprite, scale: u32, background: Option<[u8; 4]>) -> Image {
    let frames: Vec<Image> = s
        .anims
        .iter()
        .flat_map(|a| a.frames.iter().cloned())
        .collect();
    let per_row = frames_per_row(s.width, scale) as usize;
    let mut rows: Vec<Image> = frames.chunks(per_row).map(|c| strip(c, 1)).collect();
    if s.mirror {
        let mirrored: Vec<Image> = frames.iter().map(Image::flipped_h).collect();
        rows.extend(mirrored.chunks(per_row).map(|c| strip(c, 1)));
    }
    let img = stack(&rows, 1);
    match background {
        Some(px) => {
            let mut out = Image::new(img.width, img.height);
            out.fill(px);
            out.blit(&img, 0, 0);
            out
        }
        None => img,
    }
}

/// A written preview: sprite name, file and its (scaled) width.
struct Preview {
    name: String,
    path: PathBuf,
    width: u32,
}

/// Writes previews of the sprites whose name contains `filter` (and that
/// are in `group`, if given).
fn preview(
    root: &Path,
    filter: &str,
    group: Option<&str>,
    scale: u32,
) -> Result<Vec<Preview>, String> {
    let sprites = compile(root)?;
    let dir = preview_dir(root, "sprites")?;
    let mut written = Vec::new();
    for s in sprites
        .iter()
        .filter(|s| s.sprite.name.contains(filter) && group.is_none_or(|g| s.sprite.group == g))
    {
        let path = dir.join(format!("{}.png", s.sprite.name));
        let img = sprite_preview(&s.sprite, scale, s.background).scaled(scale);
        write_png(&path, &img)?;
        println!("{}", path.display());
        written.push(Preview {
            name: s.sprite.name.clone(),
            path,
            width: img.width,
        });
    }
    if written.is_empty() {
        return Err(match group {
            Some(g) => {
                let mut groups: Vec<&str> =
                    sprites.iter().map(|s| s.sprite.group.as_str()).collect();
                groups.sort();
                groups.dedup();
                format!("no sprite in group `{g}` (groups: {})", groups.join(", "))
            }
            None => format!("no sprite matches `{filter}`"),
        });
    }
    Ok(written)
}

/// Writes one contact-sheet cell to `out`: the preview at its own size
/// (no resampling of pixel art) above its name, centred, as wide as the
/// wider of the two (ImageMagick sizes the `label:`, so long names are never
/// cut), with [`SHEET_PAD`] around. Returns the cell's width.
fn sheet_cell(preview: &Path, name: &str, out: &Path) -> Result<u32, String> {
    let mut cmd = Command::new("convert");
    cmd.args(["-background", SHEET_BACKGROUND])
        .arg(preview)
        .args(["-fill", "black", "-pointsize", LABEL_POINTSIZE])
        .arg(format!("label:{name}"))
        .args([
            "-gravity",
            "center",
            "-append",
            "-bordercolor",
            SHEET_BACKGROUND,
        ])
        .args(["-border", &SHEET_PAD.to_string(), "+repage"])
        .arg(out);
    run_tool(cmd, "convert")?;
    let file = fs::File::open(out).map_err(|e| format!("{}: {e}", out.display()))?;
    let reader = png::Decoder::new(std::io::BufReader::new(file))
        .read_info()
        .map_err(|e| format!("{}: {e}", out.display()))?;
    Ok(reader.info().width)
}

/// Flows cells (by width) into rows, in order: a new row starts before
/// the row's total width would exceed `max_w`. A single cell wider than
/// `max_w` gets a row of its own. Returns the number of cells per row.
fn flow_rows(cell_ws: &[u32], max_w: u32) -> Vec<usize> {
    let mut rows: Vec<usize> = Vec::new();
    let (mut count, mut row_w) = (0usize, 0u32);
    for &w in cell_ws {
        if count > 0 && row_w + w > max_w {
            rows.push(count);
            (count, row_w) = (0, 0);
        }
        count += 1;
        row_w += w;
    }
    if count > 0 {
        rows.push(count);
    }
    rows
}

fn sheet(root: &Path, group: Option<&str>) -> Result<(), String> {
    let previews = preview(root, "", group, 4)?;
    let out_name = match group {
        Some(g) => format!("contact_sheet_{g}.png"),
        None => "contact_sheet.png".to_owned(),
    };
    let out = preview_dir(root, "")?.join(out_name);
    let cells_dir = preview_dir(root, "sheet_cells")?;
    // One labelled cell per sprite, at its own size.
    let mut cells = Vec::new();
    let mut cell_ws = Vec::new();
    for p in &previews {
        let cell = cells_dir.join(format!("{}.png", p.name));
        cell_ws.push(sheet_cell(&p.path, &p.name, &cell)?);
        cells.push(cell);
    }
    // Cells side by side (top-aligned) per row, rows stacked (left-aligned).
    let mut cmd = Command::new("convert");
    cmd.args(["-background", SHEET_BACKGROUND, "-gravity", "NorthWest"]);
    let mut rest = cells.as_slice();
    for count in flow_rows(&cell_ws, MAX_SHEET_W) {
        let (row, tail) = rest.split_at(count);
        rest = tail;
        cmd.arg("(").args(row).args(["+append", ")"]);
    }
    cmd.args(["-append", "+repage"]).arg(&out);
    run_tool(cmd, "convert")?;
    println!("{}", out.display());
    Ok(())
}

fn anim(root: &Path, name: &str, scale: u32) -> Result<(), String> {
    let sprites = compile(root)?;
    let s = sprites
        .iter()
        .map(|s| &s.sprite)
        .find(|s| s.name == name)
        .ok_or_else(|| {
            let names: Vec<&str> = sprites.iter().map(|s| s.sprite.name.as_str()).collect();
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
    fn frames_wrap_at_16_or_about_1600_px() {
        // Small frames: the 16-frame limit wins.
        assert_eq!(frames_per_row(5, 4), 16);
        // 16 px frames at x4: 17 * 4 = 68 px each -> 23 fit, capped at 16.
        assert_eq!(frames_per_row(16, 4), 16);
        // 32 px frames at x4: 132 px each -> 12 fit in 1600 px.
        assert_eq!(frames_per_row(32, 4), 12);
        // Huge frames still get one per row.
        assert_eq!(frames_per_row(1000, 4), 1);
        assert_eq!(frames_per_row(0, 0), 16);
    }

    fn sprite_with_frames(w: u32, h: u32, n: usize, mirror: bool) -> CompiledSprite {
        let mut frame = Image::new(w, h);
        frame.fill([255, 255, 255, 255]);
        CompiledSprite {
            name: "s".into(),
            group: "test".into(),
            width: w,
            height: h,
            origin: Default::default(),
            hitbox: Default::default(),
            mirror,
            anims: vec![jnr_artkit::CompiledAnim {
                name: "idle".into(),
                ticks: 0,
                looping: true,
                frames: vec![frame; n],
            }],
        }
    }

    #[test]
    fn preview_wraps_frames_and_mirrored_copies() {
        // 89 glyph-sized frames (like the font): 16 per row -> 6 rows.
        let img = sprite_preview(&sprite_with_frames(5, 7, 89, false), 4, None);
        assert_eq!((img.width, img.height), (16 * 6 - 1, 6 * 8 - 1));
        // Mirrored: the same rows again below.
        let img = sprite_preview(&sprite_with_frames(5, 7, 20, true), 4, None);
        assert_eq!((img.width, img.height), (16 * 6 - 1, 4 * 8 - 1));
        // Few frames stay in one row.
        let img = sprite_preview(&sprite_with_frames(5, 7, 3, false), 4, None);
        assert_eq!((img.width, img.height), (3 * 6 - 1, 7));
    }

    #[test]
    fn preview_background_fills_the_gaps() {
        let navy = [42, 56, 120, 255];
        let img = sprite_preview(&sprite_with_frames(2, 2, 2, false), 4, Some(navy));
        assert_eq!(img.get(2, 0), navy, "gap between frames");
        assert_eq!(img.get(0, 0), [255, 255, 255, 255], "frame pixel");
        let plain = sprite_preview(&sprite_with_frames(2, 2, 2, false), 4, None);
        assert_eq!(plain.get(2, 0)[3], 0, "no background: transparent");
    }

    #[test]
    fn sheet_rows_stay_under_the_width_limit() {
        assert_eq!(flow_rows(&[], 2000), Vec::<usize>::new());
        assert_eq!(flow_rows(&[100; 5], 2000), vec![5]);
        // 20 * 100 = 2000 fits, the 21st starts a new row.
        assert_eq!(flow_rows(&[100; 21], 2000), vec![20, 1]);
        // Widths add up: 900 + 900 fits, the next 300 does not.
        assert_eq!(flow_rows(&[900, 900, 300, 100], 2000), vec![2, 2]);
        // A cell wider than the limit gets its own row.
        assert_eq!(flow_rows(&[100, 2500, 100], 2000), vec![1, 1, 1]);
        // Every row fits (unless it is a single over-wide cell) and could
        // not have taken the next cell.
        let ws: Vec<u32> = (0..40).map(|i| 50 + (i * 37) % 400).collect();
        let mut i = 0;
        for n in flow_rows(&ws, 2000) {
            let row_w: u32 = ws[i..i + n].iter().sum();
            assert!(n == 1 || row_w <= 2000);
            if let Some(next) = ws.get(i + n) {
                assert!(row_w + next > 2000);
            }
            i += n;
        }
        assert_eq!(i, ws.len());
    }

    #[test]
    fn sheet_cells_fit_preview_or_label() {
        let dir = crate::target_dir(&Path::new(env!("CARGO_MANIFEST_DIR")).join(".."))
            .join("tmp/xtask-sheet-cell-test");
        fs::create_dir_all(&dir).unwrap();
        let png = |name: &str, w: u32| {
            let path = dir.join(name);
            fs::write(&path, Image::new(w, 8).encode_png()).unwrap();
            path
        };
        let label_w = |name: &str| {
            let out = Command::new("convert")
                .args(["-pointsize", LABEL_POINTSIZE])
                .arg(format!("label:{name}"))
                .args(["-format", "%w", "info:"])
                .output()
                .unwrap();
            String::from_utf8(out.stdout)
                .unwrap()
                .parse::<u32>()
                .unwrap()
        };
        // A wide preview: the cell is the preview plus padding.
        let wide = png("wide.png", 400);
        let w = sheet_cell(&wide, "font", &dir.join("cell_font.png")).unwrap();
        assert_eq!(w, 400 + 2 * SHEET_PAD);
        // A long name under a small preview: the whole label fits.
        let name = "zartkugel_thrown_long_name";
        let small = png("small.png", 4);
        let w = sheet_cell(&small, name, &dir.join("cell_long.png")).unwrap();
        assert!(label_w(name) > 4 + 2 * SHEET_PAD, "the label is the wider");
        assert_eq!(w, label_w(name) + 2 * SHEET_PAD);
    }

    #[test]
    fn real_art_lints_clean() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let (ok, text) = lint_report(&root);
        assert!(ok, "{text}");
    }
}
