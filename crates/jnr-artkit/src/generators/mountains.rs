//! `mountains`: a horizontally tileable mountain silhouette from seeded
//! periodic midpoint displacement through optional hand-placed peaks, in two
//! tones split along the ridges (left lit), with optional snow caps.
//!
//! ```toml
//! [[mountains]]
//! name   = "bg_untersberg"
//! width  = 640
//! height = 110
//! seed   = 11
//! peaks  = [[150, 96], [260, 70], [470, 84]]  # (x, height in px from the bottom)
//! base   = 40          # height at x 0 when there are no peaks (default height / 2)
//! roughness = 20       # midpoint displacement in px per 64 px of segment (default 20)
//! smooth = 3           # slope window for the lit/shadow split (default 3)
//! ridge_slant = 8      # the split moves right by depth * slant / 16 (default 8)
//! lit    = "f"
//! shadow = "F"
//! snow   = "h"         # optional snow caps above snow_line (px from the bottom)
//! snow_line = 80
//! snow_depth = 8       # snow covers at most this many px below the surface (default 8)
//! snow_shadow = "f"    # optional snow colour on the shadow side (default: snow)
//! ```

use serde::Deserialize;
use toml::Spanned;

use super::{
    Canvas, Colour, Common, Ctx, MAX_PARAM, Rng, midpoint_profile, seed_of, shade_silhouette,
};
use crate::diag::Diagnostic;

/// Largest up/down wobble of the snow line in px.
const SNOW_JITTER: i32 = 3;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct MountainParams {
    name: Spanned<String>,
    #[serde(default)]
    group: Option<Spanned<String>>,
    #[serde(default)]
    seed: Option<u64>,
    width: Spanned<u32>,
    height: Spanned<u32>,
    #[serde(default)]
    peaks: Vec<Spanned<[i32; 2]>>,
    #[serde(default)]
    base: Option<Spanned<i32>>,
    #[serde(default = "default_roughness")]
    roughness: i32,
    #[serde(default = "default_smooth")]
    smooth: i32,
    #[serde(default = "default_slant")]
    ridge_slant: i32,
    lit: Colour,
    shadow: Colour,
    #[serde(default)]
    snow: Option<Colour>,
    #[serde(default)]
    snow_line: Option<Spanned<i32>>,
    #[serde(default = "default_snow_depth")]
    snow_depth: i32,
    #[serde(default)]
    snow_shadow: Option<Colour>,
}

fn default_snow_depth() -> i32 {
    8
}

fn default_roughness() -> i32 {
    20
}
fn default_smooth() -> i32 {
    3
}
fn default_slant() -> i32 {
    8
}

impl MountainParams {
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
    p: &MountainParams,
    file_seed: Option<u64>,
) -> Result<Canvas, Diagnostic> {
    let (w, h) = ctx.size(&p.common())?;
    let (wi, hi) = (w as i32, h as i32);
    let mut rng = Rng::new(seed_of(&p.common(), file_seed));
    let lit = ctx.colour(&p.lit)?;
    let shadow = ctx.colour(&p.shadow)?;
    let snow = ctx.opt_colour(&p.snow)?;
    let snow_shadow = ctx.opt_colour(&p.snow_shadow)?;

    let mut controls = Vec::new();
    for peak in &p.peaks {
        let [x, y] = *peak.get_ref();
        if !(0..wi).contains(&x) || !(1..=hi).contains(&y) {
            return Err(ctx.err(
                peak.span(),
                format!("peak [{x}, {y}] must have x in 0..{w} and height in 1..={h}"),
            ));
        }
        if controls.iter().any(|&(cx, _)| cx == x) {
            return Err(ctx.err(peak.span(), format!("two peaks at x {x}")));
        }
        controls.push((x, y));
    }
    if controls.is_empty() {
        let base = match &p.base {
            Some(b) if !(1..=hi).contains(b.get_ref()) => {
                return Err(ctx.err(b.span(), format!("base must be 1..={h}")));
            }
            Some(b) => *b.get_ref(),
            None => hi / 2,
        };
        controls.push((0, base));
    }
    let span = p.name.span();
    ctx.bounded(span.clone(), "roughness", &[i64::from(p.roughness)], 0, 256)?;
    ctx.bounded(span.clone(), "smooth", &[i64::from(p.smooth)], 1, 256)?;
    ctx.bounded(
        span.clone(),
        "ridge_slant",
        &[i64::from(p.ridge_slant)],
        0,
        256,
    )?;
    ctx.bounded(span, "snow_depth", &[i64::from(p.snow_depth)], 0, MAX_PARAM)?;

    let heights: Vec<i32> = midpoint_profile(w, &controls, p.roughness, &mut rng)
        .into_iter()
        .map(|v| v.clamp(1, hi))
        .collect();
    let mut canvas = Canvas::new(w, h);
    shade_silhouette(&mut canvas, &heights, lit, shadow, p.smooth, p.ridge_slant);

    if let Some(snow) = snow {
        let line = p.snow_line.as_ref().ok_or_else(|| {
            ctx.err(
                p.snow.as_ref().expect("set").span(),
                "`snow` needs `snow_line` (px from the bottom)",
            )
        })?;
        ctx.bounded(
            line.span(),
            "snow_line",
            &[i64::from(*line.get_ref())],
            0,
            i64::from(hi),
        )?;
        let line = *line.get_ref();
        let jitter = midpoint_profile(w, &[(0, 0)], 6, &mut rng);
        for x in 0..wi {
            let j = jitter[x as usize].clamp(-SNOW_JITTER, SNOW_JITTER);
            let limit = line + j;
            let top = hi - heights[x as usize];
            for y in top..(top + p.snow_depth - j).clamp(top, hi) {
                let c = canvas.get(x, y);
                if hi - y > limit {
                    let s = if c == shadow {
                        snow_shadow.unwrap_or(snow)
                    } else {
                        snow
                    };
                    canvas.set(x, y, s);
                }
            }
        }
    }
    Ok(canvas)
}

#[cfg(test)]
mod tests {
    use super::super::tests::{check_common, gen_err, gen_one, grid};

    const MOUNTAINS: &str = r#"
[[mountains]]
name = "bg_far_test"
width = 128
height = 60
seed = 7
peaks = [[30, 55], [90, 40]]
lit = "f"
shadow = "F"
snow = "h"
snow_line = 45
"#;

    /// Column heights (px from the bottom) of a generated silhouette.
    fn heights(rows: &[String]) -> Vec<usize> {
        let w = rows[0].chars().count();
        (0..w)
            .map(|x| {
                rows.iter()
                    .filter(|r| r.chars().nth(x) != Some('.'))
                    .count()
            })
            .collect()
    }

    #[test]
    fn mountains_are_a_solid_tileable_silhouette() {
        let s = check_common(MOUNTAINS, "fFh");
        let rows = grid(&s);
        let hs = heights(&rows);
        assert_eq!(hs[30], 55, "peak control point");
        assert_eq!(hs[90], 40);
        assert!(hs.iter().all(|&v| v >= 1));
        // Solid below the surface: no holes in any column.
        for x in 0..128 {
            let col: String = rows.iter().map(|r| r.chars().nth(x).unwrap()).collect();
            assert!(!col.trim_start_matches('.').contains('.'), "column {x}");
        }
        // The seam is continuous: neighbouring heights across the wrap
        // differ no more than the steepest inner step.
        let steepest = hs.windows(2).map(|w| w[0].abs_diff(w[1])).max().unwrap();
        assert!(hs[0].abs_diff(hs[127]) <= steepest);
        // Snow only above the snow line (+ jitter), on the peak.
        assert_eq!(rows[5].chars().nth(30), Some('h'));
        assert!(rows[59 - 45 + 4..].iter().all(|r| !r.contains('h')));
    }

    #[test]
    fn left_of_a_peak_is_lit_and_right_is_shadow() {
        let src = "[[mountains]]\nname = \"m\"\nwidth = 64\nheight = 40\npeaks = [[0, 2], [32, 38]]\nroughness = 0\nridge_slant = 0\nlit = \"f\"\nshadow = \"F\"\n";
        let rows = grid(&gen_one(src));
        assert_eq!(rows[39].chars().nth(20), Some('f'));
        assert_eq!(rows[39].chars().nth(44), Some('F'));
        assert!(
            rows.iter().all(|r| !r.contains('h')),
            "no snow without `snow`"
        );
    }

    #[test]
    fn snow_follows_the_surface_down_to_snow_depth() {
        let src = "[[mountains]]\nname = \"m\"\nwidth = 64\nheight = 40\nseed = 2\n\
                   lit = \"f\"\nshadow = \"F\"\nsnow = \"h\"\nsnow_line = 0\nsnow_depth = 4\n";
        let rows = grid(&gen_one(src));
        for x in 0..64 {
            let col: Vec<char> = rows.iter().map(|r| r.chars().nth(x).unwrap()).collect();
            let top = col.iter().position(|&c| c != '.').unwrap();
            let snow = col.iter().filter(|&&c| c == 'h').count();
            // Snow starts at the surface and is at most snow_depth + jitter deep.
            assert_eq!(col[top], 'h', "column {x}");
            assert!(
                snow <= 4 + super::SNOW_JITTER as usize,
                "column {x}: {snow}"
            );
            assert!(col[top..top + snow].iter().all(|&c| c == 'h'), "column {x}");
        }
    }

    #[test]
    fn mountain_param_errors() {
        let head =
            "[[mountains]]\nname = \"m\"\nwidth = 64\nheight = 40\nlit = \"f\"\nshadow = \"F\"\n";
        let e = gen_err(&format!("{head}peaks = [[70, 10]]\n"));
        assert!(
            e.starts_with("t.toml:7:10: error[generator]: peak [70, 10]"),
            "{e}"
        );
        let e = gen_err(&format!("{head}peaks = [[7, 10], [7, 12]]\n"));
        assert!(e.contains("two peaks at x 7"), "{e}");
        let e = gen_err(&format!("{head}snow = \"h\"\n"));
        assert!(e.contains("snow_line"), "{e}");
        let e = gen_err(&format!("{head}base = 0\n"));
        assert!(e.contains("base must be"), "{e}");
        let e = gen_err(&format!("{head}smooth = 0\n"));
        assert!(e.contains("smooth must be 1..=256"), "{e}");
        let e = gen_err(&format!("{head}roughness = 2147483647\n"));
        assert!(e.contains("roughness must be 0..=256"), "{e}");
        let e = gen_err(&format!("{head}snow = \"h\"\nsnow_line = -2147483648\n"));
        assert!(e.contains("snow_line must be 0..=40"), "{e}");
    }
}
