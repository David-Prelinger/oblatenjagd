//! `.sprite` parser (syntax only).
//!
//! ```text
//! sprite  = selfie_tourist
//! size    = 16x24
//! origin  = 8,24          # optional, default bottom centre
//! hitbox  = 3,8,10,16     # optional x,y,w,h, default full size
//! mirror  = auto          # optional: auto | none
//! anim    = walk     frames=2 ticks=10 loop
//! anim    = squished frames=1
//! variant = tourist_b remap r->b l->g
//! ---
//! frame walk 0
//! <grid rows>
//! ```
//!
//! `frame <index>` is accepted when the sprite has exactly one animation.
//! Grid geometry, palette chars and frame counts are checked by the lints
//! (`crate::lint`), which need the palette; this module only checks syntax.

pub use crate::atlas::{Point, Rect};
use crate::diag::{Diagnostic, strip_comment, tokens};

/// Where an animation or frame was declared.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Pos {
    pub line: usize,
    pub col: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AnimDecl {
    pub name: String,
    pub frames: u32,
    /// Ticks per frame at 60 Hz; 0 = held (single-frame animations).
    pub ticks: u32,
    pub looping: bool,
    pub line: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Remap {
    pub from: char,
    pub to: char,
    pub pos: Pos,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VariantDecl {
    pub name: String,
    pub remap: Vec<Remap>,
    pub line: usize,
}

/// One grid row: its text and source line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub text: String,
    pub line: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FrameSource {
    pub anim: String,
    pub index: u32,
    pub line: usize,
    pub rows: Vec<Row>,
}

/// A parsed `.sprite` file, before palette checks and compilation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpriteSource {
    /// Path used in diagnostics (relative to the workspace root).
    pub file: String,
    /// Directory group below `art/sprites/` (`player`, `enemies`, `tiles`…).
    pub group: String,
    pub name: String,
    pub name_line: usize,
    pub width: u32,
    pub height: u32,
    pub size_line: usize,
    pub origin: Point,
    /// Where `origin` was set (line 0 = defaulted).
    pub origin_pos: Pos,
    pub hitbox: Rect,
    /// Where `hitbox` was set (line 0 = defaulted).
    pub hitbox_pos: Pos,
    pub mirror: bool,
    pub anims: Vec<AnimDecl>,
    pub variants: Vec<VariantDecl>,
    pub frames: Vec<FrameSource>,
}

impl SpriteSource {
    pub fn anim(&self, name: &str) -> Option<&AnimDecl> {
        self.anims.iter().find(|a| a.name == name)
    }

    pub fn frame(&self, anim: &str, index: u32) -> Option<&FrameSource> {
        self.frames
            .iter()
            .find(|f| f.anim == anim && f.index == index)
    }
}

/// Parses a `.sprite` file. `file` is used in diagnostics, `group` is the
/// directory group (see [`SpriteSource::group`]).
pub fn parse_sprite(file: &str, group: &str, src: &str) -> Result<SpriteSource, Diagnostic> {
    Parser { file }.parse(group, src)
}

struct Parser<'a> {
    file: &'a str,
}

impl Parser<'_> {
    fn err(&self, line: usize, col: usize, msg: impl Into<String>) -> Diagnostic {
        Diagnostic::error("syntax", self.file, line, col, msg)
    }

    fn parse(&self, group: &str, src: &str) -> Result<SpriteSource, Diagnostic> {
        let lines: Vec<&str> = src.lines().collect();
        let sep_found = lines.iter().position(|l| l.trim() == "---");
        let sep = sep_found.unwrap_or(lines.len());

        let mut name: Option<(String, usize)> = None;
        let mut size: Option<(u32, u32, usize)> = None;
        let mut origin: Option<(Point, Pos)> = None;
        let mut hitbox: Option<(Rect, Pos)> = None;
        let mut mirror: Option<bool> = None;
        let mut anims: Vec<AnimDecl> = Vec::new();
        let mut variants: Vec<VariantDecl> = Vec::new();

        for (i, raw) in lines[..sep].iter().enumerate() {
            let line = i + 1;
            let text = strip_comment(raw);
            if text.trim().is_empty() {
                continue;
            }
            let Some(eq) = text.find('=') else {
                let col = text.len() - text.trim_start().len() + 1;
                return Err(self.err(line, col, "expected `key = value` in the header"));
            };
            let key = text[..eq].trim();
            let key_col = text.chars().take_while(|c| c.is_whitespace()).count() + 1;
            let value_col = text[..eq + 1].chars().count() + 1;
            let value_toks: Vec<(usize, &str)> = tokens(&text[eq + 1..])
                .into_iter()
                .map(|(c, t)| (c + value_col - 1, t))
                .collect();
            let Some(&(vcol, first)) = value_toks.first() else {
                return Err(self.err(line, value_col, format!("`{key}` needs a value")));
            };
            let single = |what: &str| -> Result<(), Diagnostic> {
                match value_toks.get(1) {
                    Some(&(c, t)) => {
                        Err(self.err(line, c, format!("unexpected `{t}` after the {what}")))
                    }
                    None => Ok(()),
                }
            };
            let once = |seen: bool| -> Result<(), Diagnostic> {
                if seen {
                    Err(self.err(line, key_col, format!("`{key}` is set twice")))
                } else {
                    Ok(())
                }
            };
            match key {
                "sprite" => {
                    once(name.is_some())?;
                    single("sprite name")?;
                    self.check_ident(line, vcol, first, "sprite name")?;
                    name = Some((first.to_owned(), line));
                }
                "size" => {
                    once(size.is_some())?;
                    single("size")?;
                    let (w, h) = first.split_once('x').ok_or_else(|| {
                        self.err(
                            line,
                            vcol,
                            format!("bad size `{first}`: expected `WxH`, e.g. `16x24`"),
                        )
                    })?;
                    let h_col = vcol + w.chars().count() + 1;
                    let w = self.number(line, vcol, w, "width")?;
                    let h = self.number(line, h_col, h, "height")?;
                    if w == 0 || h == 0 {
                        return Err(self.err(line, vcol, "size must be at least 1x1"));
                    }
                    size = Some((w, h, line));
                }
                "origin" => {
                    once(origin.is_some())?;
                    single("origin")?;
                    let n = self.numbers(line, vcol, first, 2, "origin `x,y`")?;
                    origin = Some((Point { x: n[0], y: n[1] }, Pos { line, col: vcol }));
                }
                "hitbox" => {
                    once(hitbox.is_some())?;
                    single("hitbox")?;
                    let n = self.numbers(line, vcol, first, 4, "hitbox `x,y,w,h`")?;
                    hitbox = Some((
                        Rect {
                            x: n[0],
                            y: n[1],
                            w: n[2],
                            h: n[3],
                        },
                        Pos { line, col: vcol },
                    ));
                }
                "mirror" => {
                    once(mirror.is_some())?;
                    single("mirror mode")?;
                    mirror = match first {
                        "auto" => Some(true),
                        "none" => Some(false),
                        _ => {
                            return Err(self.err(
                                line,
                                vcol,
                                format!("bad mirror mode `{first}`: expected `auto` or `none`"),
                            ));
                        }
                    };
                }
                "anim" => {
                    let anim = self.anim(line, &value_toks)?;
                    if let Some(prev) = anims.iter().find(|a| a.name == anim.name) {
                        return Err(self.err(
                            line,
                            vcol,
                            format!(
                                "animation `{}` is declared twice (first on line {})",
                                anim.name, prev.line
                            ),
                        ));
                    }
                    anims.push(anim);
                }
                "variant" => variants.push(self.variant(line, &value_toks)?),
                "include" | "tileset" => {
                    return Err(self.err(
                        line,
                        key_col,
                        format!("`{key}` is not supported yet (planned, see docs/art-pipeline.md)"),
                    ));
                }
                _ => {
                    return Err(self.err(
                        line,
                        key_col,
                        format!(
                            "unknown header key `{key}` (expected sprite, size, origin, hitbox, mirror, anim, variant)"
                        ),
                    ));
                }
            }
        }

        if sep_found.is_none() {
            return Err(self.err(
                lines.len().max(1),
                0,
                "missing `---` line between the header and the frames",
            ));
        }
        let (name, name_line) =
            name.ok_or_else(|| self.err(1, 0, "missing `sprite = <name>` in the header"))?;
        let (width, height, size_line) =
            size.ok_or_else(|| self.err(1, 0, "missing `size = WxH` in the header"))?;
        if anims.is_empty() {
            return Err(self.err(
                sep + 1,
                0,
                "no `anim = <name> frames=N ...` declared before `---`",
            ));
        }
        let (origin, origin_pos) = origin.unwrap_or((
            Point {
                x: width / 2,
                y: height,
            },
            Pos::default(),
        ));
        let (hitbox, hitbox_pos) = hitbox.unwrap_or((
            Rect {
                x: 0,
                y: 0,
                w: width,
                h: height,
            },
            Pos::default(),
        ));

        let frames = self.frames(&lines, sep, &anims)?;
        Ok(SpriteSource {
            file: self.file.to_owned(),
            group: group.to_owned(),
            name,
            name_line,
            width,
            height,
            size_line,
            origin,
            origin_pos,
            hitbox,
            hitbox_pos,
            mirror: mirror.unwrap_or(false),
            anims,
            variants,
            frames,
        })
    }

    fn check_ident(&self, line: usize, col: usize, s: &str, what: &str) -> Result<(), Diagnostic> {
        let ok = s
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
            && s.chars().next().is_some_and(|c| c.is_ascii_lowercase());
        if ok {
            Ok(())
        } else {
            Err(self.err(
                line,
                col,
                format!("bad {what} `{s}`: use lowercase letters, digits and `_`, starting with a letter"),
            ))
        }
    }

    fn number(&self, line: usize, col: usize, s: &str, what: &str) -> Result<u32, Diagnostic> {
        s.parse::<u32>().map_err(|_| {
            self.err(
                line,
                col,
                format!("bad {what} `{s}`: expected a whole number"),
            )
        })
    }

    /// Parses `count` comma-separated numbers.
    fn numbers(
        &self,
        line: usize,
        col: usize,
        s: &str,
        count: usize,
        what: &str,
    ) -> Result<Vec<u32>, Diagnostic> {
        let parts: Vec<&str> = s.split(',').collect();
        if parts.len() != count {
            return Err(self.err(line, col, format!("bad {what}: found `{s}`")));
        }
        let mut out = Vec::new();
        let mut c = col;
        for p in parts {
            out.push(self.number(line, c, p, what)?);
            c += p.chars().count() + 1;
        }
        Ok(out)
    }

    /// `<name> frames=N [ticks=T] [loop]`
    fn anim(&self, line: usize, toks: &[(usize, &str)]) -> Result<AnimDecl, Diagnostic> {
        let (ncol, name) = toks[0];
        self.check_ident(line, ncol, name, "animation name")?;
        let mut frames = None;
        let mut ticks = None;
        let mut looping = false;
        for &(col, t) in &toks[1..] {
            if t == "loop" {
                looping = true;
            } else if let Some(v) = t.strip_prefix("frames=") {
                frames = Some(self.number(line, col + 7, v, "frame count")?);
            } else if let Some(v) = t.strip_prefix("ticks=") {
                ticks = Some(self.number(line, col + 6, v, "tick count")?);
            } else {
                return Err(self.err(
                    line,
                    col,
                    format!("unexpected `{t}` in anim (expected frames=N, ticks=T or loop)"),
                ));
            }
        }
        let frames = frames
            .ok_or_else(|| self.err(line, ncol, format!("anim `{name}` needs `frames=N`")))?;
        if frames == 0 {
            return Err(self.err(line, ncol, format!("anim `{name}` needs at least 1 frame")));
        }
        let ticks = match ticks {
            Some(0) => {
                return Err(self.err(
                    line,
                    ncol,
                    format!("anim `{name}`: ticks must be at least 1"),
                ));
            }
            Some(t) => t,
            None if frames > 1 => {
                return Err(self.err(
                    line,
                    ncol,
                    format!("anim `{name}` has {frames} frames and needs `ticks=T`"),
                ));
            }
            None => 0,
        };
        Ok(AnimDecl {
            name: name.to_owned(),
            frames,
            ticks,
            looping,
            line,
        })
    }

    /// `<name> remap a->b c->d`
    fn variant(&self, line: usize, toks: &[(usize, &str)]) -> Result<VariantDecl, Diagnostic> {
        let (ncol, name) = toks[0];
        self.check_ident(line, ncol, name, "variant name")?;
        match toks.get(1) {
            Some(&(_, "remap")) => {}
            Some(&(c, t)) => {
                return Err(self.err(
                    line,
                    c,
                    format!("expected `remap` after the variant name, found `{t}`"),
                ));
            }
            None => {
                return Err(self.err(
                    line,
                    ncol,
                    format!("variant `{name}` needs `remap a->b ...`"),
                ));
            }
        }
        let mut remap: Vec<Remap> = Vec::new();
        for &(col, t) in &toks[2..] {
            let chars: Vec<char> = t.chars().collect();
            let [from, '-', '>', to] = chars[..] else {
                return Err(self.err(line, col, format!("bad remap `{t}`: expected `a->b`")));
            };
            if remap.iter().any(|r| r.from == from) {
                return Err(self.err(line, col, format!("`{from}` is remapped twice")));
            }
            remap.push(Remap {
                from,
                to,
                pos: Pos { line, col },
            });
        }
        if remap.is_empty() {
            return Err(self.err(line, ncol, format!("variant `{name}` remaps nothing")));
        }
        Ok(VariantDecl {
            name: name.to_owned(),
            remap,
            line,
        })
    }

    fn frames(
        &self,
        lines: &[&str],
        sep: usize,
        anims: &[AnimDecl],
    ) -> Result<Vec<FrameSource>, Diagnostic> {
        let mut frames: Vec<FrameSource> = Vec::new();
        for (i, raw) in lines.iter().enumerate().skip(sep + 1) {
            let line = i + 1;
            let row = raw.trim_end();
            if row.is_empty() || row.starts_with('#') {
                continue;
            }
            if row == "frame" || row.starts_with("frame ") {
                let toks = tokens(strip_comment(row));
                let (anim, (icol, idx)) = match toks[1..] {
                    [(_, idx)] if anims.len() == 1 => (anims[0].name.as_str(), (toks[1].0, idx)),
                    [(_, idx)] => {
                        return Err(self.err(
                            line,
                            toks[1].0,
                            format!(
                                "`frame {idx}` is ambiguous with {} animations: write `frame <anim> {idx}`",
                                anims.len()
                            ),
                        ));
                    }
                    [(acol, anim), (icol, idx)] => {
                        if !anims.iter().any(|a| a.name == anim) {
                            let known: Vec<&str> = anims.iter().map(|a| a.name.as_str()).collect();
                            return Err(self.err(
                                line,
                                acol,
                                format!(
                                    "unknown animation `{anim}` (declared: {})",
                                    known.join(", ")
                                ),
                            ));
                        }
                        (anim, (icol, idx))
                    }
                    _ => {
                        return Err(self.err(line, 1, "expected `frame <anim> <index>`"));
                    }
                };
                let index = self.number(line, icol, idx, "frame index")?;
                if let Some(prev) = frames.iter().find(|f| f.anim == anim && f.index == index) {
                    return Err(self.err(
                        line,
                        1,
                        format!(
                            "duplicate frame `{anim} {index}` (first on line {})",
                            prev.line
                        ),
                    ));
                }
                frames.push(FrameSource {
                    anim: anim.to_owned(),
                    index,
                    line,
                    rows: Vec::new(),
                });
                continue;
            }
            let Some(frame) = frames.last_mut() else {
                return Err(self.err(
                    line,
                    1,
                    "grid row before the first `frame <anim> <index>` line",
                ));
            };
            frame.rows.push(Row {
                text: row.to_owned(),
                line,
            });
        }
        Ok(frames)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BOX: &str = "\
sprite = test_box   # a comment
size   = 4x3
origin = 2,3
hitbox = 1,0,2,3
mirror = auto
anim   = idle frames=2 ticks=30 loop
anim   = hit  frames=1
variant = test_box_red remap k->r w->p
---
frame idle 0
.kk.
kwwk
.kk.

frame idle 1
# comment inside the body
.kk.
kwwk
kkkk
frame hit 0
kkkk
kkkk
kkkk
";

    #[test]
    fn parses_full_sprite() {
        let s = parse_sprite("t.sprite", "test", BOX).unwrap();
        assert_eq!(s.name, "test_box");
        assert_eq!((s.width, s.height), (4, 3));
        assert_eq!(s.origin, Point { x: 2, y: 3 });
        assert_eq!(
            s.hitbox,
            Rect {
                x: 1,
                y: 0,
                w: 2,
                h: 3
            }
        );
        assert!(s.mirror);
        assert_eq!(s.anims.len(), 2);
        assert_eq!(s.anims[0].ticks, 30);
        assert!(s.anims[0].looping);
        assert_eq!(s.anims[1].ticks, 0);
        assert!(!s.anims[1].looping);
        assert_eq!(s.variants[0].name, "test_box_red");
        assert_eq!(s.variants[0].remap.len(), 2);
        assert_eq!(s.variants[0].remap[1].to, 'p');
        assert_eq!(s.frames.len(), 3);
        let f1 = s.frame("idle", 1).unwrap();
        assert_eq!(f1.line, 15);
        assert_eq!(f1.rows[0].line, 17);
        assert_eq!(f1.rows[2].text, "kkkk");
    }

    #[test]
    fn defaults_and_short_frame_header() {
        let s = parse_sprite(
            "t.sprite",
            "test",
            "sprite = a\nsize = 2x2\nanim = idle frames=1\n---\nframe 0\nkk\nkk\n",
        )
        .unwrap();
        assert_eq!(s.origin, Point { x: 1, y: 2 });
        assert_eq!(
            s.hitbox,
            Rect {
                x: 0,
                y: 0,
                w: 2,
                h: 2
            }
        );
        assert!(!s.mirror);
        assert_eq!(s.frame("idle", 0).unwrap().rows.len(), 2);
    }

    fn err(src: &str) -> String {
        parse_sprite("t.sprite", "test", src)
            .unwrap_err()
            .to_string()
    }

    const HEAD: &str = "sprite = a\nsize = 2x2\nanim = idle frames=2 ticks=8\n";

    #[test]
    fn syntax_errors_have_locations() {
        let cases: &[(&str, &str)] = &[
            ("sprite = a\n", "t.sprite:1: error[syntax]: missing `---`"),
            (
                "size = 2x2\nanim = i frames=1\n---\n",
                "t.sprite:1: error[syntax]: missing `sprite = <name>`",
            ),
            (
                "sprite = a\nanim = i frames=1\n---\n",
                "t.sprite:1: error[syntax]: missing `size = WxH`",
            ),
            (
                "sprite = a\nsize = 2x2\n---\n",
                "t.sprite:3: error[syntax]: no `anim",
            ),
            (
                "sprite = a\nsize = 2by2\n",
                "t.sprite:2:8: error[syntax]: bad size `2by2`",
            ),
            (
                "sprite = a\nsize = 2xq\n",
                "t.sprite:2:10: error[syntax]: bad height `q`",
            ),
            (
                "sprite = Abc\n",
                "t.sprite:1:10: error[syntax]: bad sprite name `Abc`",
            ),
            (
                "sprite = a\nsprite = b\n---\n",
                "t.sprite:2:1: error[syntax]: `sprite` is set twice",
            ),
            (
                "sprite = a\ncolour = red\n---\n",
                "t.sprite:2:1: error[syntax]: unknown header key `colour`",
            ),
            (
                "sprite = a\n  colour = red\n---\n",
                "t.sprite:2:3: error[syntax]: unknown header key `colour`",
            ),
            (
                "sprite = a\nmirror = auto\n   mirror = none\n---\n",
                "t.sprite:3:4: error[syntax]: `mirror` is set twice",
            ),
            (
                "sprite = a\ninclude = b\n---\n",
                "t.sprite:2:1: error[syntax]: `include` is not supported yet",
            ),
            (
                "sprite = a\ntileset = salzburg\n---\n",
                "t.sprite:2:1: error[syntax]: `tileset` is not supported yet",
            ),
            (
                "sprite = a\nhitbox = 1,2,3\n---\n",
                "t.sprite:2:10: error[syntax]: bad hitbox",
            ),
            (
                "sprite = a\norigin = 1,x\n---\n",
                "t.sprite:2:12: error[syntax]: bad origin",
            ),
            (
                "sprite = a\nmirror = left\n---\n",
                "t.sprite:2:10: error[syntax]: bad mirror mode `left`",
            ),
            (
                "sprite = a\nanim = walk frames=4\n---\n",
                "t.sprite:2:8: error[syntax]: anim `walk` has 4 frames and needs `ticks=T`",
            ),
            (
                "sprite = a\nanim = walk ticks=4\n---\n",
                "t.sprite:2:8: error[syntax]: anim `walk` needs `frames=N`",
            ),
            (
                "sprite = a\nanim = walk frames=x\n---\n",
                "t.sprite:2:20: error[syntax]: bad frame count `x`",
            ),
            (
                "sprite = a\nanim = walk frames=1 fast\n---\n",
                "t.sprite:2:22: error[syntax]: unexpected `fast`",
            ),
            (
                "sprite = a\nanim = w frames=1\nanim = w frames=1\n---\n",
                "t.sprite:3:8: error[syntax]: animation `w` is declared twice (first on line 2)",
            ),
            (
                "sprite = a\nvariant = b swap r->b\n---\n",
                "t.sprite:2:13: error[syntax]: expected `remap`",
            ),
            (
                "sprite = a\nvariant = b remap r-b\n---\n",
                "t.sprite:2:19: error[syntax]: bad remap `r-b`",
            ),
            (
                "sprite = a\nvariant = b remap r->b r->c\n---\n",
                "t.sprite:2:24: error[syntax]: `r` is remapped twice",
            ),
            (
                "sprite = a\nbogus line\n---\n",
                "t.sprite:2:1: error[syntax]: expected `key = value`",
            ),
        ];
        for (src, want) in cases {
            let got = err(src);
            assert!(
                got.starts_with(want),
                "source:\n{src}\nwant prefix: {want}\ngot: {got}"
            );
        }
    }

    #[test]
    fn frame_errors_have_locations() {
        let e = err(&format!("{HEAD}---\nkk\n"));
        assert!(
            e.starts_with("t.sprite:5:1: error[syntax]: grid row before the first"),
            "{e}"
        );
        let e = err(&format!("{HEAD}---\nframe idle 0\nkk\nkk\nframe idle 0\n"));
        assert!(
            e.starts_with(
                "t.sprite:8:1: error[syntax]: duplicate frame `idle 0` (first on line 5)"
            ),
            "{e}"
        );
        let e = err(&format!("{HEAD}---\nframe walk 0\n"));
        assert!(
            e.starts_with("t.sprite:5:7: error[syntax]: unknown animation `walk`"),
            "{e}"
        );
        let e = err(&format!("{HEAD}anim = b frames=1\n---\nframe 0\n"));
        assert!(
            e.starts_with("t.sprite:6:7: error[syntax]: `frame 0` is ambiguous"),
            "{e}"
        );
        let e = err(&format!("{HEAD}---\nframe idle x\n"));
        assert!(
            e.starts_with("t.sprite:5:12: error[syntax]: bad frame index `x`"),
            "{e}"
        );
    }
}
