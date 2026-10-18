//! `.palette` parser.
//!
//! Format, one entry per line: `char  hex  name`, where `hex` is `#rrggbb`
//! or `none` (transparent). Lines starting with `#` are comments, and a
//! `#…` token after the name starts a trailing comment. The char `.` is
//! always transparent.

use crate::diag::{Diagnostic, tokens};

/// Maximum number of opaque colours (docs/art-pipeline.md).
pub const MAX_COLOURS: usize = 32;

/// One palette entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PaletteEntry {
    pub ch: char,
    /// RGBA; `None` means transparent.
    pub rgba: Option<[u8; 4]>,
    pub name: String,
    pub line: usize,
}

/// A parsed palette: chars map to colours.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Palette {
    pub file: String,
    pub entries: Vec<PaletteEntry>,
}

impl Palette {
    /// Parses palette source text. `file` is only used in diagnostics.
    pub fn parse(file: &str, src: &str) -> Result<Palette, Diagnostic> {
        let mut entries: Vec<PaletteEntry> = Vec::new();
        for (i, raw) in src.lines().enumerate() {
            let line = i + 1;
            let toks = tokens(raw);
            let Some(&(col, first)) = toks.first() else {
                continue;
            };
            if first.starts_with('#') {
                continue;
            }
            let err = |col: usize, msg: String| Diagnostic::error("syntax", file, line, col, msg);

            let mut chars = first.chars();
            let (Some(ch), None) = (chars.next(), chars.next()) else {
                return Err(err(
                    col,
                    format!("palette char must be a single character, found `{first}`"),
                ));
            };
            if !ch.is_ascii_graphic() {
                return Err(err(
                    col,
                    format!("palette char `{ch}` must be a printable ASCII character"),
                ));
            }
            if let Some(prev) = entries.iter().find(|e| e.ch == ch) {
                return Err(err(
                    col,
                    format!(
                        "duplicate palette char `{ch}` (first defined on line {})",
                        prev.line
                    ),
                ));
            }
            let Some(&(hex_col, hex)) = toks.get(1) else {
                return Err(err(
                    0,
                    format!("missing colour for `{ch}`: expected `{ch}  #rrggbb  name`"),
                ));
            };
            let rgba = parse_colour(hex).map_err(|msg| err(hex_col, msg))?;
            if ch == '.' && rgba.is_some() {
                return Err(err(hex_col, "`.` is always transparent; use `none`".into()));
            }
            let Some(&(_, name)) = toks.get(2) else {
                return Err(err(
                    0,
                    format!("missing name for `{ch}`: expected `{ch}  {hex}  name`"),
                ));
            };
            if let Some(&(extra_col, extra)) = toks.get(3)
                && !extra.starts_with('#')
            {
                return Err(err(
                    extra_col,
                    format!("unexpected `{extra}` after the name (comments start with `#`)"),
                ));
            }
            entries.push(PaletteEntry {
                ch,
                rgba,
                name: name.to_owned(),
                line,
            });
        }
        if !entries.iter().any(|e| e.ch == '.') {
            entries.insert(
                0,
                PaletteEntry {
                    ch: '.',
                    rgba: None,
                    name: "transparent".into(),
                    line: 0,
                },
            );
        }
        Ok(Palette {
            file: file.to_owned(),
            entries,
        })
    }

    /// The entry for `ch`, if the palette defines it.
    pub fn get(&self, ch: char) -> Option<&PaletteEntry> {
        self.entries.iter().find(|e| e.ch == ch)
    }

    /// RGBA for `ch`: `Some(None)` is transparent, `None` is unknown.
    pub fn colour(&self, ch: char) -> Option<Option<[u8; 4]>> {
        self.get(ch).map(|e| e.rgba)
    }

    /// Number of opaque colours.
    pub fn colour_count(&self) -> usize {
        self.entries.iter().filter(|e| e.rgba.is_some()).count()
    }
}

fn parse_colour(s: &str) -> Result<Option<[u8; 4]>, String> {
    if s == "none" {
        return Ok(None);
    }
    let hex = s
        .strip_prefix('#')
        .filter(|h| h.len() == 6 && h.chars().all(|c| c.is_ascii_hexdigit()))
        .ok_or_else(|| format!("bad colour `{s}`: expected `#rrggbb` or `none`"))?;
    let byte = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).expect("validated hex");
    Ok(Some([byte(0), byte(2), byte(4), 255]))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SRC: &str = "\
# comment
.       none      transparent

k       #1a1c2c   outline   # trailing comment
w       #FBF7EE   white
";

    #[test]
    fn parses_entries_and_comments() {
        let p = Palette::parse("p.palette", SRC).unwrap();
        assert_eq!(p.entries.len(), 3);
        assert_eq!(p.colour('.'), Some(None));
        assert_eq!(p.colour('k'), Some(Some([0x1a, 0x1c, 0x2c, 255])));
        assert_eq!(p.colour('w'), Some(Some([0xfb, 0xf7, 0xee, 255])));
        assert_eq!(p.colour('x'), None);
        assert_eq!(p.colour_count(), 2);
        assert_eq!(p.get('k').unwrap().line, 4);
    }

    #[test]
    fn dot_is_transparent_even_if_missing() {
        let p = Palette::parse("p", "k #000000 black\n").unwrap();
        assert_eq!(p.colour('.'), Some(None));
    }

    fn err(src: &str) -> String {
        Palette::parse("p.palette", src).unwrap_err().to_string()
    }

    #[test]
    fn errors_point_to_line_and_column() {
        assert_eq!(
            err("k #000000 a\nkk #000000 b\n"),
            "p.palette:2:1: error[syntax]: palette char must be a single character, found `kk`"
        );
        assert_eq!(
            err("k #000000 a\nk #ffffff b\n"),
            "p.palette:2:1: error[syntax]: duplicate palette char `k` (first defined on line 1)"
        );
        assert_eq!(
            err("\n  k  #12345 a\n"),
            "p.palette:2:6: error[syntax]: bad colour `#12345`: expected `#rrggbb` or `none`"
        );
        assert!(err("k #000000\n").starts_with("p.palette:1: error[syntax]: missing name"));
        assert!(err("k\n").starts_with("p.palette:1: error[syntax]: missing colour"));
        assert!(err(". #000000 x\n").contains("always transparent"));
        assert!(err("k #000000 a b\n").starts_with("p.palette:1:13:"));
    }

    #[test]
    fn real_palette_parses() {
        let src = include_str!("../../../art/palettes/main.palette");
        let p = Palette::parse("main.palette", src).unwrap();
        assert!(p.colour_count() <= MAX_COLOURS);
        assert!(p.get('k').is_some(), "outline colour `k` exists");
    }
}
