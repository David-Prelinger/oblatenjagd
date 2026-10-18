//! `skyline`: a mid/near parallax layer of simple building blocks on an
//! optional hill, e.g. the Festung on its hill or the old-town roofs.
//! Buildings are drawn in list order (later ones in front); every x wraps,
//! so the layer tiles horizontally.
//!
//! ```toml
//! [[skyline]]
//! name   = "bg_festung"
//! width  = 640
//! height = 140
//! seed   = 5
//! hill   = [[0, 20], [200, 70], [420, 24]]   # optional (x, height) points, linear between
//! hill_roughness = 4      # optional wobble (midpoint displacement, default 0)
//! hill_colour = "v"       # needed with `hill`
//! hill_shadow = "G"       # optional shadow side (split like `mountains`)
//!
//! [[skyline.building]]
//! x = 150
//! width = 60
//! height = 30
//! base = 60               # px from the bottom (default: lowest hill point under it, else 0)
//! roof = "gable"          # flat | gable | dome | tower (default flat)
//! roof_height = 12        # default: gable (width + 1) / 2, dome width / 2, tower width
//! wall = "i"
//! wall_shadow = "m"       # optional darker right edge ...
//! shadow_width = 6        # ... this many columns wide (default width / 5, at least 1)
//! roof_colour = "m"       # needed for gable, dome and tower
//! roof_shadow = "d"       # optional right half of the roof (default roof_colour)
//! eave = "R"              # optional 1 px line on top of the wall
//! window = "d"            # optional window colour (no windows without it)
//! window_size = [2, 3]    # w, h (default [2, 3])
//! window_gap = [3, 4]     # space between windows x, y (default [3, 4])
//! window_margin = [2, 3]  # wall margin x, y (default [2, 3])
//! window_chance = 80      # percent of windows drawn, from the seed (default 100)
//! ```

use serde::Deserialize;
use toml::Spanned;

use super::{
    Canvas, Colour, Common, Ctx, MAX_PARAM, Rng, midpoint_profile, seed_of, shade_silhouette,
};
use crate::diag::Diagnostic;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SkylineParams {
    name: Spanned<String>,
    #[serde(default)]
    group: Option<Spanned<String>>,
    #[serde(default)]
    seed: Option<u64>,
    width: Spanned<u32>,
    height: Spanned<u32>,
    #[serde(default)]
    hill: Vec<Spanned<[i32; 2]>>,
    #[serde(default)]
    hill_roughness: i32,
    #[serde(default)]
    hill_colour: Option<Colour>,
    #[serde(default)]
    hill_shadow: Option<Colour>,
    #[serde(default)]
    building: Vec<Spanned<Building>>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Building {
    x: i32,
    width: Spanned<u32>,
    height: u32,
    #[serde(default)]
    base: Option<i32>,
    #[serde(default)]
    roof: Option<Spanned<String>>,
    #[serde(default)]
    roof_height: Option<u32>,
    wall: Colour,
    #[serde(default)]
    wall_shadow: Option<Colour>,
    #[serde(default)]
    shadow_width: Option<u32>,
    #[serde(default)]
    roof_colour: Option<Colour>,
    #[serde(default)]
    roof_shadow: Option<Colour>,
    #[serde(default)]
    eave: Option<Colour>,
    #[serde(default)]
    window: Option<Colour>,
    #[serde(default = "default_window_size")]
    window_size: [u32; 2],
    #[serde(default = "default_window_gap")]
    window_gap: [u32; 2],
    #[serde(default = "default_window_margin")]
    window_margin: [u32; 2],
    #[serde(default = "default_window_chance")]
    window_chance: u32,
}

fn default_window_size() -> [u32; 2] {
    [2, 3]
}
fn default_window_gap() -> [u32; 2] {
    [3, 4]
}
fn default_window_margin() -> [u32; 2] {
    [2, 3]
}
fn default_window_chance() -> u32 {
    100
}

impl SkylineParams {
    pub(crate) fn common(&self) -> Common<'_> {
        Common {
            name: &self.name,
            group: &self.group,
            seed: self.seed,
            width: &self.width,
            height: &self.height,
        }
    }
}

/// Roof shapes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Roof {
    Flat,
    Gable,
    Dome,
    Tower,
}

impl Roof {
    fn parse(s: &str) -> Option<Roof> {
        match s {
            "flat" => Some(Roof::Flat),
            "gable" => Some(Roof::Gable),
            "dome" => Some(Roof::Dome),
            "tower" => Some(Roof::Tower),
            _ => None,
        }
    }
}

/// A building with its colours resolved.
struct Look {
    wall: char,
    wall_shadow: char,
    roof: char,
    roof_shadow: char,
}

pub(crate) fn generate(
    ctx: &Ctx,
    p: &SkylineParams,
    file_seed: Option<u64>,
) -> Result<Canvas, Diagnostic> {
    let (w, h) = ctx.size(&p.common())?;
    let (wi, hi) = (w as i32, h as i32);
    let mut rng = Rng::new(seed_of(&p.common(), file_seed));
    let mut canvas = Canvas::new(w, h);

    // The hill: column heights (px from the bottom), 0 = none.
    let mut hill = vec![0i32; w as usize];
    if !p.hill.is_empty() {
        let mut controls = Vec::new();
        for pt in &p.hill {
            let [x, y] = *pt.get_ref();
            if !(0..wi).contains(&x) || !(0..=hi).contains(&y) {
                return Err(ctx.err(
                    pt.span(),
                    format!("hill point [{x}, {y}] must have x in 0..{w} and height in 0..={h}"),
                ));
            }
            if controls.iter().any(|&(cx, _)| cx == x) {
                return Err(ctx.err(pt.span(), format!("two hill points at x {x}")));
            }
            controls.push((x, y));
        }
        let colour = ctx
            .opt_colour(&p.hill_colour)?
            .ok_or_else(|| ctx.err(p.hill[0].span(), "`hill` needs `hill_colour`"))?;
        let shadow = ctx.opt_colour(&p.hill_shadow)?.unwrap_or(colour);
        let rough = [i64::from(p.hill_roughness)];
        ctx.bounded(p.hill[0].span(), "hill_roughness", &rough, 0, 256)?;
        hill = midpoint_profile(w, &controls, p.hill_roughness, &mut rng)
            .into_iter()
            .map(|v| v.clamp(0, hi))
            .collect();
        shade_silhouette(&mut canvas, &hill, colour, shadow, 3, 8);
    }

    for b in &p.building {
        draw_building(ctx, &mut canvas, b, &hill, &mut rng)?;
    }
    Ok(canvas)
}

fn draw_building(
    ctx: &Ctx,
    canvas: &mut Canvas,
    sb: &Spanned<Building>,
    hill: &[i32],
    rng: &mut Rng,
) -> Result<(), Diagnostic> {
    let b = sb.get_ref();
    let (cw, ch) = (canvas.w as i32, canvas.h as i32);
    let bw = *b.width.get_ref() as i32;
    if bw < 2 || bw > cw {
        return Err(ctx.err(b.width.span(), format!("width must be 2..={cw}")));
    }
    let roof = match &b.roof {
        None => Roof::Flat,
        Some(r) => Roof::parse(r.get_ref()).ok_or_else(|| {
            ctx.err(
                r.span(),
                format!(
                    "unknown roof `{}` (expected flat, gable, dome or tower)",
                    r.get_ref()
                ),
            )
        })?,
    };
    let wall = ctx.colour(&b.wall)?;
    let roof_colour = ctx.opt_colour(&b.roof_colour)?;
    let roof_c = match (roof, roof_colour) {
        (_, Some(c)) => c,
        (Roof::Flat, None) => wall,
        (_, None) => {
            let span = b.roof.as_ref().expect("non-flat roof is set").span();
            return Err(ctx.err(span, "this roof needs `roof_colour`"));
        }
    };
    let look = Look {
        wall,
        wall_shadow: ctx.opt_colour(&b.wall_shadow)?.unwrap_or(wall),
        roof: roof_c,
        roof_shadow: ctx.opt_colour(&b.roof_shadow)?.unwrap_or(roof_c),
    };
    let eave = ctx.opt_colour(&b.eave)?;
    let window = ctx.opt_colour(&b.window)?;
    let numbers = [
        i64::from(b.x),
        i64::from(b.height),
        i64::from(b.base.unwrap_or(0)),
        i64::from(b.roof_height.unwrap_or(0)),
        i64::from(b.shadow_width.unwrap_or(0)),
    ]
    .into_iter()
    .chain(
        b.window_gap
            .iter()
            .chain(&b.window_margin)
            .map(|&v| i64::from(v)),
    )
    .collect::<Vec<_>>();
    ctx.bounded(
        sb.span(),
        "building x, height, base, roof_height, shadow_width and window gaps/margins",
        &numbers,
        -MAX_PARAM,
        MAX_PARAM,
    )?;
    let size = b.window_size.map(i64::from);
    ctx.bounded(sb.span(), "window_size", &size, 1, MAX_PARAM)?;
    if b.window_chance > 100 {
        return Err(ctx.err(sb.span(), "window_chance must be 0..=100"));
    }

    // Default base: the lowest hill point under the building, so it never floats.
    let base = b.base.unwrap_or_else(|| {
        (b.x..b.x + bw)
            .map(|x| hill[x.rem_euclid(cw) as usize])
            .min()
            .unwrap_or(0)
    });
    let bottom = ch - base; // first row below the building
    let top = bottom - b.height as i32; // first wall row
    let shadow_w = b.shadow_width.map_or((bw / 5).max(1), |s| s as i32);

    // Wall with a darker right edge.
    for x in 0..bw {
        let c = if x >= bw - shadow_w {
            look.wall_shadow
        } else {
            look.wall
        };
        for y in top..bottom {
            canvas.set(b.x + x, y, c);
        }
    }
    let mut wall_top = top;
    if let Some(e) = eave {
        canvas.fill_rect(b.x, top, bw, 1, e);
        wall_top += 1;
    }

    // Windows: a centred grid inside the wall margins.
    if let Some(win) = window {
        let [ww, wh] = b.window_size.map(|v| v as i32);
        let [gx, gy] = b.window_gap.map(|v| v as i32);
        let [mx, my] = b.window_margin.map(|v| v as i32);
        let (inner_w, inner_h) = (bw - 2 * mx, bottom - wall_top - 2 * my);
        let cols = (inner_w + gx) / (ww + gx);
        let rows = (inner_h + gy) / (wh + gy);
        if cols > 0 && rows > 0 {
            let x0 = b.x + mx + (inner_w - (cols * (ww + gx) - gx)) / 2;
            let y0 = wall_top + my;
            for r in 0..rows {
                for c in 0..cols {
                    if rng.range(0, 99) < b.window_chance as i32 {
                        canvas.fill_rect(x0 + c * (ww + gx), y0 + r * (wh + gy), ww, wh, win);
                    }
                }
            }
        }
    }

    // Roof above the wall.
    match roof {
        Roof::Flat => {}
        Roof::Gable => {
            let rh = b.roof_height.map_or((bw + 1) / 2, |v| v as i32);
            gable(canvas, b.x, top, bw, rh, &look);
        }
        Roof::Dome => {
            let rh = b.roof_height.map_or(bw / 2, |v| v as i32).max(1);
            dome(canvas, b.x, top, bw, rh, &look);
        }
        Roof::Tower => {
            let tw = (bw / 3).max(2);
            let th = b.roof_height.map_or(bw, |v| v as i32);
            let tx = b.x + (bw - tw) / 2;
            for x in 0..tw {
                let c = if x == tw - 1 {
                    look.wall_shadow
                } else {
                    look.wall
                };
                for y in top - th..top {
                    canvas.set(tx + x, y, c);
                }
            }
            gable(canvas, tx - 1, top - th, tw + 2, (tw + 3) / 2, &look);
        }
    }
    Ok(())
}

/// A gable (triangle) of `rh` rows sitting on row `top`, left half lit.
fn gable(canvas: &mut Canvas, x: i32, top: i32, w: i32, rh: i32, look: &Look) {
    for r in 0..rh {
        let inset = r * w / (2 * rh);
        for i in inset..w - inset {
            let c = if 2 * i < w {
                look.roof
            } else {
                look.roof_shadow
            };
            canvas.set(x + i, top - 1 - r, c);
        }
    }
}

/// A half-ellipse dome of `rh` rows on row `top`, plus a small lantern.
fn dome(canvas: &mut Canvas, x: i32, top: i32, w: i32, rh: i32, look: &Look) {
    // Pixel centres in doubled units: half-axes w (across) and 2 * rh (up).
    let (w64, rh64) = (i64::from(w), i64::from(rh));
    for r in 0..rh {
        for i in 0..w {
            let dx2 = i64::from(2 * i + 1 - w);
            let dy2 = i64::from(2 * r + 1);
            if dx2 * dx2 * 4 * rh64 * rh64 + dy2 * dy2 * w64 * w64 <= 4 * w64 * w64 * rh64 * rh64 {
                let c = if dx2 < 0 { look.roof } else { look.roof_shadow };
                canvas.set(x + i, top - 1 - r, c);
            }
        }
    }
    let lantern_h = (rh / 3).max(2);
    let cx = x + w / 2 - 1;
    canvas.fill_rect(cx, top - rh - lantern_h, 1, lantern_h + 1, look.roof);
    canvas.fill_rect(
        cx + 1,
        top - rh - lantern_h,
        1,
        lantern_h + 1,
        look.roof_shadow,
    );
}

#[cfg(test)]
mod tests {
    use super::super::tests::{check_common, gen_err, gen_one, grid};

    const SKYLINE: &str = r#"
[[skyline]]
name = "bg_town_test"
width = 96
height = 48
seed = 3
hill = [[0, 4], [48, 20]]
hill_colour = "v"
hill_shadow = "G"

[[skyline.building]]
x = 40
width = 16
height = 10
roof = "gable"
wall = "c"
wall_shadow = "o"
roof_colour = "E"
roof_shadow = "n"
eave = "R"
window = "d"

[[skyline.building]]
x = 88
width = 14
height = 8
base = 0
roof = "dome"
wall = "p"
roof_colour = "t"
roof_shadow = "T"

[[skyline.building]]
x = 20
width = 12
height = 6
roof = "tower"
wall = "i"
wall_shadow = "m"
roof_colour = "m"
"#;

    #[test]
    fn skyline_draws_hill_and_buildings_with_wrapping() {
        let s = check_common(SKYLINE, "vGcoEnRdptTim");
        let rows = grid(&s);
        let at = |x: usize, y: usize| rows[y].chars().nth(x).unwrap();
        // Hill: 20 px tall at x 48, 4 px at x 0.
        // Hill: about 7 px tall at x 10 (between [0, 4] and [48, 20]), lit side.
        assert_eq!(at(10, 47), 'v');
        assert_eq!(at(10, 47 - 5), 'v');
        assert_eq!(at(10, 47 - 9), '.');
        // The gable building sits on the lowest hill point under it.
        assert!(rows.iter().any(|r| r.contains('E')) && rows.iter().any(|r| r.contains('n')));
        assert!(rows.iter().any(|r| r.contains('R')), "eave");
        assert!(rows.iter().any(|r| r.contains('d')), "windows");
        // The dome building wraps: x 88..102 covers 0..6 on the bottom row.
        assert_eq!(at(2, 47), 'p');
        assert_eq!(at(90, 47), 'p');
        // Tower roof in `m` above the wall `i`.
        assert!(rows.iter().take(30).any(|r| r.contains('i')));
    }

    #[test]
    fn dome_is_a_half_ellipse_with_an_attached_lantern() {
        let src = "[[skyline]]\nname = \"s\"\nwidth = 16\nheight = 14\n\
                   [[skyline.building]]\nx = 0\nwidth = 10\nheight = 2\nroof = \"dome\"\n\
                   roof_height = 4\nwall = \"c\"\nroof_colour = \"t\"\nroof_shadow = \"T\"\n";
        let rows = grid(&gen_one(src));
        // Wall rows 12, 13; dome rows 11 (widest) up to 8; lantern rows 6..=8.
        assert_eq!(rows[13], "cccccccccc......");
        assert_eq!(rows[11], "tttttTTTTT......");
        assert!(
            rows[8].contains('t') && rows[8].starts_with("..."),
            "{}",
            rows[8]
        );
        for (y, row) in rows.iter().enumerate().take(12).skip(6) {
            assert_eq!(&row[4..6], "tT", "row {y}: lantern and dome touch");
        }
        assert_eq!(rows[5], "................");
        let widths: Vec<usize> = (8..=11)
            .map(|y| rows[y].matches(['t', 'T']).count())
            .collect();
        assert!(widths.windows(2).all(|w| w[0] <= w[1]), "{widths:?}");
    }

    #[test]
    fn gable_is_a_lit_left_half_and_a_shadow_right_half() {
        let src = "[[skyline]]\nname = \"s\"\nwidth = 16\nheight = 12\n\
                   [[skyline.building]]\nx = 0\nwidth = 8\nheight = 4\nroof = \"gable\"\n\
                   wall = \"c\"\nroof_colour = \"E\"\nroof_shadow = \"n\"\n";
        let rows = grid(&gen_one(src));
        assert_eq!(rows[11], "cccccccc........");
        assert_eq!(rows[7], "EEEEnnnn........");
        assert_eq!(rows[6], ".EEEnnn.........");
        assert_eq!(rows[4], "...En...........");
        assert_eq!(rows[3], "................");
    }

    #[test]
    fn skyline_param_errors() {
        let head = "[[skyline]]\nname = \"s\"\nwidth = 32\nheight = 16\n";
        let e = gen_err(&format!("{head}hill = [[0, 4]]\n"));
        assert!(e.contains("hill_colour"), "{e}");
        let e = gen_err(&format!("{head}hill = [[40, 4]]\nhill_colour = \"v\"\n"));
        assert!(e.starts_with("t.toml:5:9:"), "{e}");
        let b = "[[skyline.building]]\nx = 0\nheight = 4\nwall = \"c\"\n";
        let e = gen_err(&format!("{head}{b}width = 1\n"));
        assert!(e.contains("width must be 2..=32"), "{e}");
        let e = gen_err(&format!("{head}{b}width = 8\nroof = \"onion\"\n"));
        assert!(e.contains("unknown roof `onion`"), "{e}");
        let e = gen_err(&format!("{head}{b}width = 8\nroof = \"dome\"\n"));
        assert!(e.contains("roof_colour"), "{e}");
        let e = gen_err(&format!("{head}{b}width = 8\nwindow_chance = 101\n"));
        assert!(e.contains("window_chance"), "{e}");
        let e = gen_err(&format!("{head}{b}width = 8\nwindow_size = [0, 2]\n"));
        assert!(e.contains("window_size must be 1..="), "{e}");
        let e = gen_err(&format!("{head}{b}width = 8\nbase = -2147483648\n"));
        assert!(
            e.starts_with("t.toml:5:1:") && e.contains("must be -8192..=8192"),
            "{e}"
        );
        let e = gen_err(&format!(
            "{head}hill = [[0, 4]]\nhill_colour = \"v\"\nhill_roughness = -1\n"
        ));
        assert!(e.contains("hill_roughness must be 0..=256"), "{e}");
    }
}
