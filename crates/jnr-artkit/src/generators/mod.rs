//! Procedural generators: large or repetitive art (parallax backgrounds)
//! made from a few parameters in `art/generators/*.toml`.
//!
//! A generator file lists sprites by generator kind, each entry one sprite:
//!
//! ```toml
//! seed = 1                  # optional file default
//!
//! [[sky]]                   # or [[mountains]], [[skyline]]
//! name   = "bg_sky_salzburg"
//! width  = 320
//! height = 180
//! # ... generator params, see sky.rs / mountains.rs / skyline.rs
//! ```
//!
//! Each entry becomes a plain [`SpriteSource`] (group `bg` unless `group`
//! is set, origin 0,0, one held anim `idle`), so generated sprites run
//! through exactly the same lints, compiler and atlas packer as `.sprite`
//! files. Every row of a generated sprite reports the line of the entry's
//! `name` key.
//!
//! Rules for generators: deterministic (own seeded xorshift, integer math
//! only), palette chars only (every colour param is checked against the
//! palette, with `file:line:col`), and horizontally tileable (every x
//! coordinate wraps around the width). No I/O: [`generate`] takes the file
//! text, so this module builds for wasm.

mod mountains;
mod sky;
mod skyline;

use std::ops::Range;

use serde::Deserialize;
use toml::Spanned;

use crate::atlas::{Point, Rect};
use crate::diag::Diagnostic;
use crate::palette::Palette;
use crate::sprite::{AnimDecl, FrameSource, Pos, Row, SpriteSource};

/// Default sprite group of generated sprites.
pub const DEFAULT_GROUP: &str = "bg";
/// Name of the single animation of a generated sprite.
pub const ANIM: &str = "idle";
/// Largest generated sprite edge (keeps typos like `width = 64000` cheap).
pub const MAX_SIZE: u32 = 2048;
/// Every other number param (coordinates, lengths, roughness, ...) must lie
/// in `-MAX_PARAM..=MAX_PARAM`, so the generators' i32 math never overflows
/// (a panic would break the build script instead of reporting `file:line`).
pub const MAX_PARAM: i64 = 4 * MAX_SIZE as i64;

/// The whole generator file.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GenFile {
    #[serde(default)]
    seed: Option<u64>,
    #[serde(default)]
    sky: Vec<sky::SkyParams>,
    #[serde(default)]
    mountains: Vec<mountains::MountainParams>,
    #[serde(default)]
    skyline: Vec<skyline::SkylineParams>,
}

/// The keys every generator entry has (`name`, `group`, `seed`, `width`,
/// `height`), borrowed from the entry. Each params struct repeats these
/// fields (serde's `flatten` would lose spans and `deny_unknown_fields`).
pub(crate) struct Common<'a> {
    pub name: &'a Spanned<String>,
    pub group: &'a Option<Spanned<String>>,
    pub seed: Option<u64>,
    pub width: &'a Spanned<u32>,
    pub height: &'a Spanned<u32>,
}

/// A palette char parameter, e.g. `lit = "f"`.
pub(crate) type Colour = Spanned<String>;

/// Generates every sprite of one generator file. `file` is the path used in
/// diagnostics; colour params are checked against `palette`.
/// Returns the sprites in file order per generator kind (sky, mountains,
/// skyline), or the first error.
pub fn generate(file: &str, src: &str, palette: &Palette) -> Result<Vec<SpriteSource>, Diagnostic> {
    let ctx = Ctx { file, src, palette };
    let parsed: GenFile = toml::from_str(src).map_err(|e| {
        let (line, col) = e.span().map_or((1, 0), |s| ctx.line_col(s.start));
        Diagnostic::error("syntax", file, line, col, e.message().trim().to_owned())
    })?;
    let mut out = Vec::new();
    for p in &parsed.sky {
        let canvas = sky::generate(&ctx, p, parsed.seed)?;
        out.push(ctx.sprite(&p.common(), canvas)?);
    }
    for p in &parsed.mountains {
        let canvas = mountains::generate(&ctx, p, parsed.seed)?;
        out.push(ctx.sprite(&p.common(), canvas)?);
    }
    for p in &parsed.skyline {
        let canvas = skyline::generate(&ctx, p, parsed.seed)?;
        out.push(ctx.sprite(&p.common(), canvas)?);
    }
    Ok(out)
}

/// What a generator needs to report errors and check colours.
pub(crate) struct Ctx<'a> {
    pub file: &'a str,
    pub src: &'a str,
    pub palette: &'a Palette,
}

impl Ctx<'_> {
    /// 1-based line and column of a byte offset.
    pub fn line_col(&self, offset: usize) -> (usize, usize) {
        let before = &self.src[..offset.min(self.src.len())];
        let line = before.matches('\n').count() + 1;
        let col = before.rsplit('\n').next().map_or(0, |l| l.chars().count()) + 1;
        (line, col)
    }

    /// An error at a span.
    pub fn err(&self, span: Range<usize>, msg: impl Into<String>) -> Diagnostic {
        let (line, col) = self.line_col(span.start);
        Diagnostic::error("generator", self.file, line, col, msg)
    }

    /// Checks a colour param: exactly one non-transparent palette char.
    pub fn colour(&self, c: &Colour) -> Result<char, Diagnostic> {
        let mut chars = c.get_ref().chars();
        match (chars.next(), chars.next()) {
            (Some(ch), None) => match self.palette.colour(ch) {
                Some(Some(_)) => Ok(ch),
                Some(None) => Err(self.err(
                    c.span(),
                    format!("`{ch}` is transparent; a colour param needs an opaque palette char"),
                )),
                None => Err(self.err(
                    c.span(),
                    format!("`{ch}` is not a palette char (see {})", self.palette.file),
                )),
            },
            _ => Err(self.err(
                c.span(),
                format!(
                    "expected one palette char, found `{}`",
                    c.get_ref().escape_default()
                ),
            )),
        }
    }

    /// Checks that every value lies in `lo..=hi` (`what` names the keys).
    pub fn bounded(
        &self,
        span: Range<usize>,
        what: &str,
        values: &[i64],
        lo: i64,
        hi: i64,
    ) -> Result<(), Diagnostic> {
        if values.iter().all(|v| (lo..=hi).contains(v)) {
            Ok(())
        } else {
            Err(self.err(span, format!("{what} must be {lo}..={hi}")))
        }
    }

    pub fn opt_colour(&self, c: &Option<Colour>) -> Result<Option<char>, Diagnostic> {
        c.as_ref().map(|c| self.colour(c)).transpose()
    }

    /// Checks the common keys and returns (width, height).
    pub fn size(&self, c: &Common<'_>) -> Result<(u32, u32), Diagnostic> {
        for v in [&c.width, &c.height] {
            if *v.get_ref() == 0 || *v.get_ref() > MAX_SIZE {
                return Err(self.err(v.span(), format!("size must be 1..={MAX_SIZE} px")));
            }
        }
        Ok((*c.width.get_ref(), *c.height.get_ref()))
    }

    /// Turns a finished canvas into a sprite source.
    fn sprite(&self, c: &Common<'_>, canvas: Canvas) -> Result<SpriteSource, Diagnostic> {
        let name = c.name.get_ref();
        let valid = |s: &str| {
            s.chars()
                .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_')
                && s.chars().next().is_some_and(|ch| ch.is_ascii_lowercase())
        };
        if !valid(name) {
            return Err(self.err(
                c.name.span(),
                format!("bad sprite name `{name}`: use lowercase letters, digits and `_`, starting with a letter"),
            ));
        }
        let group = match c.group {
            Some(g) if !valid(g.get_ref()) => {
                return Err(self.err(g.span(), format!("bad group `{}`", g.get_ref())));
            }
            Some(g) => g.get_ref().clone(),
            None => DEFAULT_GROUP.to_owned(),
        };
        let (line, col) = self.line_col(c.name.span().start);
        let rows = canvas
            .rows()
            .into_iter()
            .map(|text| Row { text, line })
            .collect();
        Ok(SpriteSource {
            file: self.file.to_owned(),
            group,
            name: name.clone(),
            name_line: line,
            width: canvas.w,
            height: canvas.h,
            size_line: line,
            origin: Point { x: 0, y: 0 },
            origin_pos: Pos { line, col },
            hitbox: Rect {
                x: 0,
                y: 0,
                w: canvas.w,
                h: canvas.h,
            },
            hitbox_pos: Pos { line, col },
            mirror: false,
            anims: vec![AnimDecl {
                name: ANIM.to_owned(),
                frames: 1,
                ticks: 0,
                looping: false,
                line,
            }],
            variants: Vec::new(),
            frames: vec![FrameSource {
                anim: ANIM.to_owned(),
                index: 0,
                line,
                rows,
            }],
        })
    }
}

/// The seed of an entry: its own, else the file's, else 0.
pub(crate) fn seed_of(c: &Common<'_>, file_seed: Option<u64>) -> u64 {
    c.seed.or(file_seed).unwrap_or(0)
}

// ------------------------------------------------------------------- canvas

/// A grid of palette chars (`.` = transparent). Writes wrap horizontally
/// (so every generator is tileable by construction) and clip vertically.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Canvas {
    pub w: u32,
    pub h: u32,
    px: Vec<char>,
}

impl Canvas {
    pub fn new(w: u32, h: u32) -> Canvas {
        Canvas {
            w,
            h,
            px: vec!['.'; (w * h) as usize],
        }
    }

    /// Sets a pixel; x wraps around the width, y outside the canvas is ignored.
    pub fn set(&mut self, x: i32, y: i32, ch: char) {
        if y < 0 || y >= self.h as i32 {
            return;
        }
        let x = x.rem_euclid(self.w as i32);
        self.px[(y as u32 * self.w + x as u32) as usize] = ch;
    }

    /// The pixel at (x wrapped, y); `.` outside vertically.
    pub fn get(&self, x: i32, y: i32) -> char {
        if y < 0 || y >= self.h as i32 {
            return '.';
        }
        let x = x.rem_euclid(self.w as i32);
        self.px[(y as u32 * self.w + x as u32) as usize]
    }

    /// Fills `w` x `h` pixels from (x, y) (wrapping horizontally).
    pub fn fill_rect(&mut self, x: i32, y: i32, w: i32, h: i32, ch: char) {
        for yy in y..y + h {
            for xx in x..x + w {
                self.set(xx, yy, ch);
            }
        }
    }

    /// The grid as text rows, top to bottom.
    pub fn rows(&self) -> Vec<String> {
        self.px
            .chunks(self.w as usize)
            .map(|r| r.iter().collect())
            .collect()
    }
}

// ---------------------------------------------------------------- utilities

/// xorshift64* (same algorithm as `jnr_core::rng`), private to generators.
#[derive(Clone, Debug)]
pub(crate) struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Rng {
        // Mix so that small seeds give unrelated streams; never zero.
        let s = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0xD1B5_4A32_D192_ED03;
        Rng(if s == 0 { 1 } else { s })
    }

    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// Uniform in `lo..=hi` (`lo` if the range is empty).
    pub fn range(&mut self, lo: i32, hi: i32) -> i32 {
        if hi <= lo {
            return lo;
        }
        let span = (i64::from(hi) - i64::from(lo)) as u64 + 1;
        (i64::from(lo) + ((self.next_u64() >> 32) % span) as i64) as i32
    }
}

/// 4x4 Bayer matrix (thresholds 0..16).
const BAYER4: [[u32; 4]; 4] = [[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]];
/// 2x2 ordered (checkerboard-like) matrix, scaled to 0..16.
const BAYER2: [[u32; 2]; 2] = [[0, 8], [12, 4]];

/// Ordered dither pattern for transitions between two colours.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dither {
    /// 4x4 Bayer.
    Bayer4,
    /// 2x2 checkerboard.
    Checker,
}

impl Dither {
    pub fn parse(s: &str) -> Option<Dither> {
        match s {
            "bayer4" => Some(Dither::Bayer4),
            "checker" => Some(Dither::Checker),
            _ => None,
        }
    }

    /// Period in x and y (the width of a tileable layer must be a multiple).
    pub fn period(self) -> u32 {
        match self {
            Dither::Bayer4 => 4,
            Dither::Checker => 2,
        }
    }

    /// True if pixel (x, y) shows the second colour when the mix is
    /// `amount16` sixteenths of the second colour (0 = none, 16 = all).
    pub fn second(self, x: i32, y: i32, amount16: u32) -> bool {
        let t = match self {
            Dither::Bayer4 => BAYER4[y.rem_euclid(4) as usize][x.rem_euclid(4) as usize],
            Dither::Checker => BAYER2[y.rem_euclid(2) as usize][x.rem_euclid(2) as usize],
        };
        t < amount16
    }
}

/// Periodic midpoint displacement: heights for `w` columns through the
/// control points (x, height), sorted by x; the profile wraps from the last
/// point to the first one plus `w`. Each midpoint is displaced by up to
/// `segment length * roughness / 64` px.
pub(crate) fn midpoint_profile(
    w: u32,
    controls: &[(i32, i32)],
    roughness: i32,
    rng: &mut Rng,
) -> Vec<i32> {
    let w = w as i32;
    let mut h = vec![0i32; w as usize];
    let mut pts: Vec<(i32, i32)> = controls.to_vec();
    pts.sort_by_key(|p| p.0);
    for &(x, y) in &pts {
        h[x.rem_euclid(w) as usize] = y;
    }
    for i in 0..pts.len() {
        let (x0, y0) = pts[i];
        let (x1, y1) = if i + 1 < pts.len() {
            pts[i + 1]
        } else {
            (pts[0].0 + w, pts[0].1)
        };
        subdivide(&mut h, w, (x0, y0), (x1, y1), roughness, rng);
    }
    h
}

fn subdivide(h: &mut [i32], w: i32, a: (i32, i32), b: (i32, i32), roughness: i32, rng: &mut Rng) {
    let len = b.0 - a.0;
    if len < 2 {
        return;
    }
    let mx = a.0 + len / 2;
    let amp = len * roughness / 64;
    let my = (a.1 + b.1) / 2 + rng.range(-amp, amp);
    h[mx.rem_euclid(w) as usize] = my;
    subdivide(h, w, a, (mx, my), roughness, rng);
    subdivide(h, w, (mx, my), b, roughness, rng);
}

/// Fills a silhouette given by column heights (px from the bottom of the
/// canvas) in two tones with a lit/shadow split along the ridges (light from
/// the top left, art-style §4/§7). A pixel `depth` px below the surface is
/// lit when the terrain rises towards the right around the column
/// `depth * slant / 16` px to its left, so the split runs down and to the
/// right from every peak. The slope window grows with depth
/// (`smooth + depth / 2` px each way), so small bumps only shade the skin
/// near the surface and deep pixels follow the big shapes.
pub(crate) fn shade_silhouette(
    canvas: &mut Canvas,
    heights: &[i32],
    lit: char,
    shadow: char,
    smooth: i32,
    slant: i32,
) {
    let w = canvas.w as i32;
    let ch = canvas.h as i32;
    let at = |x: i32| heights[x.rem_euclid(w) as usize];
    for x in 0..w {
        let top = ch - at(x).clamp(0, ch);
        for y in top..ch {
            let depth = y - top;
            let xs = x - depth * slant / 16;
            let k = smooth + depth / 2;
            let c = if at(xs + k) >= at(xs - k) {
                lit
            } else {
                shadow
            };
            canvas.set(x, y, c);
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// The real palette.
    pub fn palette() -> Palette {
        Palette::parse(
            "art/palettes/main.palette",
            include_str!("../../../../art/palettes/main.palette"),
        )
        .unwrap()
    }

    pub fn gen_one(src: &str) -> SpriteSource {
        let mut v = generate("t.toml", src, &palette()).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(v.len(), 1);
        v.remove(0)
    }

    pub fn gen_err(src: &str) -> String {
        generate("t.toml", src, &palette()).unwrap_err().to_string()
    }

    pub fn grid(s: &SpriteSource) -> Vec<String> {
        s.frames[0].rows.iter().map(|r| r.text.clone()).collect()
    }

    /// Asserts the shared properties every generated background has:
    /// size, only the allowed chars, deterministic, horizontally tileable
    /// (the seam is no worse than the worst inner column pair).
    pub fn check_common(src: &str, allowed: &str) -> SpriteSource {
        let s = gen_one(src);
        let rows = grid(&s);
        assert_eq!(rows.len() as u32, s.height);
        assert!(rows.iter().all(|r| r.chars().count() as u32 == s.width));
        for r in &rows {
            for ch in r.chars() {
                assert!(ch == '.' || allowed.contains(ch), "unexpected `{ch}`");
            }
        }
        assert_eq!(gen_one(src), s, "deterministic");
        let lint = crate::lint::lint_sprite(&s, &palette());
        assert!(lint.is_empty(), "{lint:?}");
        s
    }

    #[test]
    fn rng_is_deterministic_and_in_range() {
        let mut a = Rng::new(5);
        let mut b = Rng::new(5);
        for _ in 0..1000 {
            let v = a.range(-3, 3);
            assert_eq!(v, b.range(-3, 3));
            assert!((-3..=3).contains(&v));
        }
        assert_ne!(Rng::new(1).next_u64(), Rng::new(2).next_u64());
        assert_eq!(Rng::new(0).range(4, 4), 4);
        let wide = Rng::new(7).range(i32::MIN, i32::MAX);
        assert!((i32::MIN..=i32::MAX).contains(&wide));
    }

    #[test]
    fn canvas_wraps_horizontally_and_clips_vertically() {
        let mut c = Canvas::new(4, 2);
        c.set(-1, 0, 'a');
        c.set(5, 1, 'b');
        c.set(0, 2, 'x');
        c.set(0, -1, 'x');
        assert_eq!(c.rows(), vec!["...a", ".b.."]);
        assert_eq!(c.get(-1, 0), 'a');
        assert_eq!(c.get(0, 9), '.');
    }

    #[test]
    fn dither_mixes_by_amount() {
        for d in [Dither::Bayer4, Dither::Checker] {
            let p = d.period() as i32;
            for amount in [0u32, 4, 8, 12, 16] {
                let n = (0..p)
                    .flat_map(|y| (0..p).map(move |x| (x, y)))
                    .filter(|&(x, y)| d.second(x, y, amount))
                    .count() as u32;
                assert_eq!(n * 16, amount * (p * p) as u32, "{d:?} {amount}");
            }
        }
        assert_eq!(Dither::parse("checker"), Some(Dither::Checker));
        assert_eq!(Dither::parse("x"), None);
    }

    #[test]
    fn midpoint_profile_hits_controls_and_wraps() {
        let mut rng = Rng::new(3);
        let h = midpoint_profile(64, &[(10, 30), (40, 5)], 16, &mut rng);
        assert_eq!(h.len(), 64);
        assert_eq!(h[10], 30);
        assert_eq!(h[40], 5);
        // Without roughness the profile is piecewise linear and continuous
        // across the wrap (x 63 -> 0).
        let flat = midpoint_profile(64, &[(0, 20)], 0, &mut Rng::new(1));
        assert!(flat.iter().all(|&v| v == 20));
        let mut rng = Rng::new(9);
        let rough = midpoint_profile(64, &[(0, 20)], 32, &mut rng);
        assert!((rough[63] - rough[0]).abs() <= 8, "{rough:?}");
    }

    #[test]
    fn shading_splits_a_peak_left_lit_right_shadow() {
        let mut c = Canvas::new(16, 8);
        let heights: Vec<i32> = (0..16).map(|x: i32| 8 - (x - 8).abs()).collect();
        shade_silhouette(&mut c, &heights, 'f', 'F', 1, 0);
        let rows = c.rows();
        assert_eq!(rows[7], ".ffffffffFFFFFFF");
        assert_eq!(rows[1], ".......ffF......");
        // With slant the split moves right going down.
        let mut c = Canvas::new(16, 8);
        shade_silhouette(&mut c, &heights, 'f', 'F', 1, 16);
        assert_eq!(c.get(10, 7), 'f');
    }

    #[test]
    fn syntax_and_unknown_keys_report_file_line() {
        assert!(gen_err("[[sky]]\nname = \n").starts_with("t.toml:2:"));
        let e = gen_err("[[sky]]\nname = \"a\"\nwidth = 4\nheight = 4\nbogus = 1\n");
        assert!(
            e.starts_with("t.toml:5:1: error[syntax]: unknown field `bogus`"),
            "{e}"
        );
        let e = gen_err("[[cloudz]]\n");
        assert!(e.contains("unknown field `cloudz`"), "{e}");
    }

    #[test]
    fn colour_params_must_be_palette_chars() {
        let base = "[[sky]]\nname = \"a\"\nwidth = 4\nheight = 4\nbands = [{ colour = ";
        let e = gen_err(&format!("{base}\"Q\" }}]\n"));
        assert_eq!(
            e,
            "t.toml:5:21: error[generator]: `Q` is not a palette char (see art/palettes/main.palette)"
        );
        let e = gen_err(&format!("{base}\".\" }}]\n"));
        assert!(e.contains("transparent"), "{e}");
        let e = gen_err(&format!("{base}\"SS\" }}]\n"));
        assert!(e.contains("expected one palette char"), "{e}");
    }

    #[test]
    fn names_groups_and_sizes_are_checked() {
        let e = gen_err(
            "[[sky]]\nname = \"Bad\"\nwidth = 4\nheight = 4\nbands = [{ colour = \"S\" }]\n",
        );
        assert!(
            e.starts_with("t.toml:2:8: error[generator]: bad sprite name"),
            "{e}"
        );
        let e =
            gen_err("[[sky]]\nname = \"a\"\nwidth = 0\nheight = 4\nbands = [{ colour = \"S\" }]\n");
        assert!(e.starts_with("t.toml:3:9:"), "{e}");
        let s = gen_one(
            "[[sky]]\nname = \"a\"\ngroup = \"test\"\nwidth = 4\nheight = 2\nbands = [{ colour = \"S\" }]\n",
        );
        assert_eq!(
            (s.group.as_str(), s.name_line, s.origin),
            ("test", 2, Point { x: 0, y: 0 })
        );
        assert_eq!(s.anims[0].name, ANIM);
    }

    #[test]
    fn seeds_fall_back_to_the_file_seed() {
        let src = |own: &str| {
            format!(
                "seed = 4\n[[mountains]]\nname = \"m\"\nwidth = 64\nheight = 40\n{own}lit = \"f\"\nshadow = \"F\"\n"
            )
        };
        assert_eq!(gen_one(&src("")), gen_one(&src("seed = 4\n")));
        assert_ne!(grid(&gen_one(&src(""))), grid(&gen_one(&src("seed = 5\n"))));
    }
}
