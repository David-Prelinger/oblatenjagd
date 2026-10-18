//! `sky`: horizontal colour bands top to bottom with ordered-dither
//! transitions between neighbouring bands, plus optional clouds.
//!
//! ```toml
//! [[sky]]
//! name   = "bg_sky_salzburg"
//! width  = 320
//! height = 180
//! seed   = 3
//! bands  = [
//!   { colour = "S", until = 70 },   # rows 0..70 (until = first row of the next band)
//!   { colour = "s", until = 130 },
//!   { colour = "h" },               # the last band reaches the bottom
//! ]
//! dither  = 6           # transition height in px, centred on each boundary (0..=8, default 4)
//! pattern = "bayer4"    # bayer4 | checker
//! cloud_colour = "w"    # needed when there are clouds
//! cloud_shadow = "h"    # optional: the bottom 2 rows of every cloud
//! random_clouds = 3     # extra clouds placed from the seed (default 0) ...
//! random_cloud_rows = [10, 80]   # ... with their tops in these rows
//!
//! [[sky.cloud]]         # hand-placed clouds: box of puffs on a flat base
//! x = 40
//! y = 20
//! width = 48
//! height = 12
//! ```

use serde::Deserialize;
use toml::Spanned;

use super::{Canvas, Colour, Common, Ctx, Dither, MAX_PARAM, Rng, seed_of};
use crate::diag::Diagnostic;

/// Largest dither transition (art-style §6: bands are 2-8 px tall).
pub const MAX_DITHER: u32 = 8;
/// Rows of cloud shadow at the bottom of a cloud.
const CLOUD_SHADOW_ROWS: i32 = 2;
/// Most random clouds per sky.
const MAX_RANDOM_CLOUDS: i64 = 64;
/// Placement attempts per random cloud.
const RANDOM_CLOUD_TRIES: u32 = 16;
/// Free space kept around random clouds.
const CLOUD_GAP: i32 = 6;
/// Smallest cloud box.
const MIN_CLOUD: (u32, u32) = (8, 4);

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SkyParams {
    name: Spanned<String>,
    #[serde(default)]
    group: Option<Spanned<String>>,
    #[serde(default)]
    seed: Option<u64>,
    width: Spanned<u32>,
    height: Spanned<u32>,
    bands: Spanned<Vec<Band>>,
    #[serde(default)]
    dither: Option<Spanned<u32>>,
    #[serde(default)]
    pattern: Option<Spanned<String>>,
    #[serde(default)]
    cloud_colour: Option<Colour>,
    #[serde(default)]
    cloud_shadow: Option<Colour>,
    #[serde(default)]
    cloud: Vec<Spanned<Cloud>>,
    #[serde(default)]
    random_clouds: u32,
    #[serde(default)]
    random_cloud_rows: Option<Spanned<[i32; 2]>>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Band {
    colour: Colour,
    #[serde(default)]
    until: Option<Spanned<u32>>,
}

#[derive(Deserialize, Clone, Copy)]
#[serde(deny_unknown_fields)]
struct Cloud {
    x: i32,
    y: i32,
    width: u32,
    height: u32,
}

impl SkyParams {
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

pub(crate) fn generate(
    ctx: &Ctx,
    p: &SkyParams,
    file_seed: Option<u64>,
) -> Result<Canvas, Diagnostic> {
    let (w, h) = ctx.size(&p.common())?;
    let mut rng = Rng::new(seed_of(&p.common(), file_seed));

    let dither = p.dither.as_ref().map_or(4, |d| *d.get_ref());
    if dither > MAX_DITHER {
        let span = p.dither.as_ref().expect("set").span();
        return Err(ctx.err(span, format!("dither must be 0..={MAX_DITHER} px")));
    }
    let pattern = match &p.pattern {
        None => Dither::Bayer4,
        Some(s) => Dither::parse(s.get_ref()).ok_or_else(|| {
            ctx.err(
                s.span(),
                format!(
                    "unknown pattern `{}` (expected bayer4 or checker)",
                    s.get_ref()
                ),
            )
        })?,
    };
    if dither > 0 && w % pattern.period() != 0 {
        return Err(ctx.err(
            p.width.span(),
            format!(
                "width must be a multiple of {} so the dither pattern tiles",
                pattern.period()
            ),
        ));
    }

    // (colour, first row of the next band) per band; the last ends at h.
    let mut bands: Vec<(char, i32)> = Vec::new();
    let n = p.bands.get_ref().len();
    if n == 0 {
        return Err(ctx.err(p.bands.span(), "needs at least one band"));
    }
    let mut prev = 0i32;
    for (i, b) in p.bands.get_ref().iter().enumerate() {
        let colour = ctx.colour(&b.colour)?;
        let until = match (&b.until, i + 1 == n) {
            (None, true) => h as i32,
            (Some(u), true) => {
                return Err(ctx.err(u.span(), "the last band reaches the bottom; remove `until`"));
            }
            (None, false) => {
                return Err(ctx.err(b.colour.span(), "every band but the last needs `until`"));
            }
            (Some(u), false) => {
                let v = *u.get_ref() as i32;
                if v <= prev || v >= h as i32 {
                    return Err(ctx.err(
                        u.span(),
                        format!(
                            "`until` must grow and lie inside the sky (found {v} after {prev})"
                        ),
                    ));
                }
                if v - prev < dither as i32 {
                    return Err(ctx.err(
                        u.span(),
                        format!(
                            "band is {} px tall, less than the {dither} px dither",
                            v - prev
                        ),
                    ));
                }
                v
            }
        };
        bands.push((colour, until));
        prev = until;
    }

    let mut canvas = Canvas::new(w, h);
    for y in 0..h as i32 {
        let (upper, lower, amount) = row_mix(&bands, y, dither as i32);
        for x in 0..w as i32 {
            let ch = if pattern.second(x, y, amount) {
                lower
            } else {
                upper
            };
            canvas.set(x, y, ch);
        }
    }

    // Clouds: hand-placed first, then random ones (drawn in that order).
    let has_clouds = !p.cloud.is_empty() || p.random_clouds > 0;
    let colour = match (ctx.opt_colour(&p.cloud_colour)?, has_clouds) {
        (Some(c), _) => c,
        (None, false) => return Ok(canvas),
        (None, true) => {
            return Err(ctx.err(p.name.span(), "clouds need `cloud_colour`"));
        }
    };
    let shadow = ctx.opt_colour(&p.cloud_shadow)?.unwrap_or(colour);
    let mut clouds = Vec::new();
    for c in &p.cloud {
        let v = *c.get_ref();
        let xy = [i64::from(v.x), i64::from(v.y)];
        ctx.bounded(c.span(), "cloud x and y", &xy, -MAX_PARAM, MAX_PARAM)?;
        if v.width < MIN_CLOUD.0
            || v.height < MIN_CLOUD.1
            || v.width > w
            || v.height > h
            || v.height > v.width
        {
            return Err(ctx.err(
                c.span(),
                format!(
                    "cloud must be at least {}x{} px, no taller than wide, and fit the sky",
                    MIN_CLOUD.0, MIN_CLOUD.1
                ),
            ));
        }
        clouds.push(v);
    }
    let [top0, top1] = match &p.random_cloud_rows {
        Some(r) => {
            let [a, b] = *r.get_ref();
            let rows = [i64::from(a), i64::from(b)];
            ctx.bounded(r.span(), "random_cloud_rows", &rows, -MAX_PARAM, MAX_PARAM)?;
            if a > b {
                return Err(ctx.err(r.span(), "expected [first row, last row]"));
            }
            [a, b]
        }
        None => [0, h as i32 / 2],
    };
    // Random clouds keep a gap to every earlier cloud (a few tries each,
    // skipped if none fits), so overlapping shadows never cut through one.
    ctx.bounded(
        p.name.span(),
        "random_clouds",
        &[i64::from(p.random_clouds)],
        0,
        MAX_RANDOM_CLOUDS,
    )?;
    for _ in 0..p.random_clouds {
        for _ in 0..RANDOM_CLOUD_TRIES {
            let cw = rng.range(24, 56).min(w as i32);
            let cand = Cloud {
                width: cw as u32,
                height: rng.range(8, 14).min(h as i32).min(cw) as u32,
                x: rng.range(0, w as i32 - 1),
                y: rng.range(top0, top1),
            };
            // A sky too small for a minimal cloud gets none.
            let big_enough = cand.width >= MIN_CLOUD.0 && cand.height >= MIN_CLOUD.1;
            if big_enough && clouds.iter().all(|c| !near(c, &cand, w as i32)) {
                clouds.push(cand);
                break;
            }
        }
    }
    for c in clouds {
        draw_cloud(&mut canvas, c, colour, shadow, &mut rng);
    }
    Ok(canvas)
}

/// (upper colour, lower colour, sixteenths of lower) for row `y`. Outside
/// transitions the amount is 0 and `upper` is the band's colour.
fn row_mix(bands: &[(char, i32)], y: i32, dither: i32) -> (char, char, u32) {
    for i in 0..bands.len() {
        let (colour, until) = bands[i];
        if i + 1 < bands.len() && dither > 0 {
            let start = until - dither / 2;
            if (start..start + dither).contains(&y) {
                let amount = (((y - start) * 2 + 1) * 16 / (2 * dither)) as u32;
                return (colour, bands[i + 1].0, amount);
            }
        }
        if y < until {
            return (colour, colour, 0);
        }
    }
    let last = bands[bands.len() - 1].0;
    (last, last, 0)
}

/// True if two cloud boxes (plus [`CLOUD_GAP`]) overlap, with x wrapping
/// around the sky width.
fn near(a: &Cloud, b: &Cloud, sky_w: i32) -> bool {
    let (aw, ah, bw, bh) = (
        a.width as i32,
        a.height as i32,
        b.width as i32,
        b.height as i32,
    );
    let rows = a.y - CLOUD_GAP < b.y + bh && b.y - CLOUD_GAP < a.y + ah;
    // Horizontal distance between the box starts, both ways round the sky.
    let dx = (b.x - a.x).rem_euclid(sky_w);
    let cols = dx < aw + CLOUD_GAP || sky_w - dx < bw + CLOUD_GAP;
    rows && cols
}

/// A cloud: overlapping round puffs (spaced 2/3 of the height apart, the
/// outer ones touching the box edges) on a flat base half the height tall;
/// the bottom rows in `shadow`. Wraps horizontally like the whole canvas.
fn draw_cloud(canvas: &mut Canvas, c: Cloud, colour: char, shadow: char, rng: &mut Rng) {
    let (w, h) = (c.width as i32, c.height as i32);
    let bottom = h - 1;
    let spacing = (h * 2 / 3).max(3);
    let (first, last) = (h / 2, w - 1 - h / 2);
    let n = ((last - first) / spacing + 1).max(2);
    let mut inside = vec![false; (w * h) as usize];
    for i in 0..n {
        let cx = first + (last - first) * i / (n - 1);
        let edge = i == 0 || i == n - 1;
        let r = if edge {
            rng.range(h / 3, h * 2 / 5)
        } else {
            rng.range(h * 2 / 5, h / 2)
        }
        .max(1);
        let cy = bottom - r;
        for y in 0..h {
            for x in 0..w {
                let (dx, dy) = (x - cx, y - cy);
                if dx * dx + dy * dy <= r * r + r {
                    inside[(y * w + x) as usize] = true;
                }
            }
        }
    }
    let base = (h / 2).max(2);
    for y in h - base..h {
        for x in first.min(w - 1)..=last.clamp(0, w - 1) {
            inside[(y * w + x) as usize] = true;
        }
    }
    for y in 0..h {
        for x in 0..w {
            if inside[(y * w + x) as usize] {
                let ch = if y > bottom - CLOUD_SHADOW_ROWS {
                    shadow
                } else {
                    colour
                };
                canvas.set(c.x + x, c.y + y, ch);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::{check_common, gen_err, grid};

    const SKY: &str = r#"
[[sky]]
name = "bg_sky_test"
width = 64
height = 40
seed = 2
bands = [{ colour = "S", until = 12 }, { colour = "s", until = 28 }, { colour = "h" }]
dither = 4
cloud_colour = "w"
cloud_shadow = "h"
random_clouds = 2
random_cloud_rows = [26, 26]

[[sky.cloud]]
x = 50
y = 4
width = 30
height = 10
"#;

    #[test]
    fn sky_bands_dither_and_clouds() {
        let s = check_common(SKY, "Sshw");
        assert_eq!((s.width, s.height, s.group.as_str()), (64, 40, "bg"));
        let rows = grid(&s);
        // Pure bands away from boundaries and clouds.
        assert!(rows[0].chars().all(|c| c == 'S'));
        assert!(rows[20].chars().all(|c| c == 's'));
        // Random clouds have their tops in row 26.
        assert!(rows[26].contains('w') && !rows[25].contains('w'));
        // The transition around row 12 (rows 10..14) mixes S and s only.
        let mixed: String = rows[11]
            .chars()
            .filter(|c| *c != 'w' && *c != 'h')
            .collect();
        assert!(mixed.contains('S') && mixed.contains('s'), "{}", rows[11]);
        // The hand-placed cloud wraps around the right edge (x 50..80).
        assert_eq!(rows[10].chars().nth(55), Some('w'));
        assert_eq!(rows[10].chars().nth(72 - 64), Some('w'));
        // Cloud undersides (the bottom 2 rows, 12 and 13) use the shadow colour.
        assert_eq!(rows[13].chars().nth(70 - 64), Some('h'));
        assert_eq!(rows[12].chars().nth(55), Some('h'));
    }

    #[test]
    fn transition_mixes_more_of_the_lower_colour_further_down() {
        let src = "[[sky]]\nname = \"a\"\nwidth = 16\nheight = 16\nbands = [{ colour = \"S\", until = 8 }, { colour = \"s\" }]\ndither = 8\n";
        let rows = grid(&check_common(src, "Ss"));
        assert!(rows[..4].iter().all(|r| r == "SSSSSSSSSSSSSSSS"));
        assert!(rows[12..].iter().all(|r| r == "ssssssssssssssss"));
        assert!(
            rows[4..12]
                .iter()
                .all(|r| r.contains('S') || r.contains('s'))
        );
        let mix: Vec<(char, char, u32)> = (0..16)
            .map(|y| super::row_mix(&[('S', 8), ('s', 16)], y, 8))
            .collect();
        let amounts: Vec<u32> = mix.iter().map(|m| m.2).collect();
        assert_eq!(amounts, [0, 0, 0, 0, 1, 3, 5, 7, 9, 11, 13, 15, 0, 0, 0, 0]);
        assert_eq!((mix[4].0, mix[4].1), ('S', 's'));
        assert_eq!(mix[12], ('s', 's', 0));
    }

    #[test]
    fn clouds_near_each_other_across_the_wrap() {
        let c = |x, y| super::Cloud {
            x,
            y,
            width: 20,
            height: 10,
        };
        assert!(super::near(&c(0, 0), &c(10, 5), 100));
        assert!(!super::near(&c(0, 0), &c(40, 0), 100));
        assert!(!super::near(&c(0, 0), &c(0, 30), 100));
        // 90..110 wraps onto 0..10.
        assert!(super::near(&c(0, 0), &c(90, 0), 100));
        assert!(super::near(&c(90, 0), &c(0, 0), 100));
    }

    #[test]
    fn tall_clouds_are_rejected_and_tiny_skies_get_no_random_clouds() {
        let head = "[[sky]]\nname = \"a\"\nwidth = 4\nheight = 16\nbands = [{ colour = \"S\" }]\ncloud_colour = \"w\"\n";
        let e = gen_err(&format!(
            "{head}[[sky.cloud]]\nx = 0\ny = 0\nwidth = 8\nheight = 16\n"
        ));
        assert!(e.contains("no taller than wide"), "{e}");
        let s = check_common(&format!("{head}random_clouds = 3\n"), "S");
        assert!(grid(&s).iter().all(|r| r == "SSSS"));
        // The flat base of the narrowest allowed cloud stays inside its box.
        let wide = "[[sky]]\nname = \"a\"\nwidth = 16\nheight = 8\nbands = [{ colour = \"S\" }]\ncloud_colour = \"w\"\n[[sky.cloud]]\nx = 0\ny = 0\nwidth = 8\nheight = 8\n";
        let rows = grid(&check_common(wide, "Sw"));
        assert!(rows.iter().all(|r| !r[8..].contains('w')), "{rows:?}");
    }

    #[test]
    fn plain_sky_without_clouds() {
        let src = "[[sky]]\nname = \"a\"\nwidth = 8\nheight = 8\nbands = [{ colour = \"S\", until = 4 }, { colour = \"h\" }]\ndither = 0\n";
        let s = check_common(src, "Sh");
        assert_eq!(grid(&s)[3], "SSSSSSSS");
        assert_eq!(grid(&s)[4], "hhhhhhhh");
    }

    #[test]
    fn sky_param_errors() {
        let head = "[[sky]]\nname = \"a\"\nwidth = 8\nheight = 8\n";
        let e = gen_err(&format!(
            "{head}bands = [{{ colour = \"S\" }}, {{ colour = \"h\" }}]\n"
        ));
        assert!(e.contains("every band but the last needs `until`"), "{e}");
        let e = gen_err(&format!(
            "{head}bands = [{{ colour = \"S\", until = 4 }}]\n"
        ));
        assert!(e.contains("remove `until`"), "{e}");
        let e = gen_err(&format!(
            "{head}bands = [{{ colour = \"S\", until = 9 }}, {{ colour = \"h\" }}]\n"
        ));
        assert!(e.contains("must grow"), "{e}");
        let e = gen_err(&format!("{head}bands = []\n"));
        assert!(e.contains("at least one band"), "{e}");
        let e = gen_err(&format!(
            "{head}bands = [{{ colour = \"S\" }}]\ndither = 9\n"
        ));
        assert!(e.starts_with("t.toml:6:10:"), "{e}");
        let e = gen_err(&format!(
            "{head}bands = [{{ colour = \"S\" }}]\npattern = \"x\"\n"
        ));
        assert!(e.contains("unknown pattern"), "{e}");
        let e =
            gen_err("[[sky]]\nname = \"a\"\nwidth = 6\nheight = 8\nbands = [{ colour = \"S\" }]\n");
        assert!(e.contains("multiple of 4"), "{e}");
        let e = gen_err(&format!(
            "{head}bands = [{{ colour = \"S\" }}]\nrandom_clouds = 1\n"
        ));
        assert!(e.contains("cloud_colour"), "{e}");
        let e = gen_err(&format!(
            "{head}bands = [{{ colour = \"S\" }}]\ncloud_colour = \"w\"\n[[sky.cloud]]\nx = 0\ny = 0\nwidth = 2\nheight = 2\n"
        ));
        assert!(e.starts_with("t.toml:7:1:"), "{e}");
    }
}
