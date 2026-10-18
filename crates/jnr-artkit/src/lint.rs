//! Art lints. Table-driven: to add a rule, write a check function and add a
//! row to [`SPRITE_RULES`] (per sprite) or [`PROJECT_RULES`] (whole art set).
//! Each check returns [`Hit`]s; the table gives them rule id and severity.

use std::collections::BTreeMap;

use crate::diag::{Diagnostic, Severity};
use crate::palette::{MAX_COLOURS, Palette};
use crate::sprite::SpriteSource;

/// Sprite groups (directories below `art/sprites/`) that are characters or
/// interactive objects and therefore need a dark outline (`k`).
pub const OUTLINE_GROUPS: [&str; 3] = ["player", "enemies", "items"];
/// Frames that are intentionally fully transparent: (sprite, anim, frame index).
/// The font's space glyph is the only one so far.
pub const EMPTY_FRAME_ALLOWED: [(&str, &str, u32); 1] = [("font", "glyphs", 0)];
/// The outline palette char.
pub const OUTLINE_CHAR: char = 'k';
/// Sprite group of parallax background layers (`docs/sprite-catalog.md`).
pub const BG_GROUP: &str = "bg";
/// Largest background layer (`docs/sprite-catalog.md`).
pub const BG_MAX_SIZE: (u32, u32) = (640, 180);

/// One raw finding of a check, before rule id and severity are attached.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hit {
    pub file: String,
    pub line: usize,
    pub col: usize,
    pub message: String,
}

fn hit(file: &str, line: usize, col: usize, message: String) -> Hit {
    Hit {
        file: file.to_owned(),
        line,
        col,
        message,
    }
}

pub struct SpriteRule {
    pub id: &'static str,
    pub severity: Severity,
    pub summary: &'static str,
    pub check: fn(&SpriteSource, &Palette) -> Vec<Hit>,
}

pub struct ProjectRule {
    pub id: &'static str,
    pub severity: Severity,
    pub summary: &'static str,
    pub check: fn(&Palette, &[SpriteSource]) -> Vec<Hit>,
}

pub const SPRITE_RULES: &[SpriteRule] = &[
    SpriteRule {
        id: "palette-chars",
        severity: Severity::Error,
        summary: "grid and remap chars must be palette chars",
        check: palette_chars,
    },
    SpriteRule {
        id: "row-width",
        severity: Severity::Error,
        summary: "every grid row is exactly the declared width",
        check: row_width,
    },
    SpriteRule {
        id: "frame-height",
        severity: Severity::Error,
        summary: "every frame has exactly the declared number of rows (so all frames are the same size)",
        check: frame_height,
    },
    SpriteRule {
        id: "frame-count",
        severity: Severity::Error,
        summary: "frames match the `anim` declarations (none missing, none extra)",
        check: frame_count,
    },
    SpriteRule {
        id: "empty-frame",
        severity: Severity::Error,
        summary: "no fully transparent frames",
        check: empty_frame,
    },
    SpriteRule {
        id: "outline",
        severity: Severity::Error,
        summary: "character sprites (player, enemies, items) have outline pixels `k` in every frame",
        check: outline,
    },
    SpriteRule {
        id: "bounds",
        severity: Severity::Error,
        summary: "origin and hitbox lie inside the sprite (hitbox non-empty)",
        check: bounds,
    },
    SpriteRule {
        id: "bg-tileable",
        severity: Severity::Warning,
        summary: "background layers tile horizontally: the seam (last column -> column 0) differs in no more rows than the most different pair of neighbouring columns inside",
        check: bg_tileable,
    },
    SpriteRule {
        id: "bg-size",
        severity: Severity::Warning,
        summary: "background layers are at most 640x180",
        check: bg_size,
    },
    SpriteRule {
        id: "file-name",
        severity: Severity::Warning,
        summary: "sprite name matches the file name",
        check: file_name,
    },
];

pub const PROJECT_RULES: &[ProjectRule] = &[
    ProjectRule {
        id: "palette-size",
        severity: Severity::Error,
        summary: "the palette has at most 32 colours",
        check: palette_size,
    },
    ProjectRule {
        id: "duplicate-name",
        severity: Severity::Error,
        summary: "sprite and variant names are unique",
        check: duplicate_name,
    },
];

/// Runs all sprite rules on one sprite.
pub fn lint_sprite(s: &SpriteSource, palette: &Palette) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    for rule in SPRITE_RULES {
        out.extend(
            (rule.check)(s, palette)
                .into_iter()
                .map(|h| to_diag(rule.id, rule.severity, h)),
        );
    }
    out
}

/// Runs all sprite and project rules. Sorted by file, line, column.
pub fn lint_all(palette: &Palette, sprites: &[SpriteSource]) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    for rule in PROJECT_RULES {
        out.extend(
            (rule.check)(palette, sprites)
                .into_iter()
                .map(|h| to_diag(rule.id, rule.severity, h)),
        );
    }
    for s in sprites {
        out.extend(lint_sprite(s, palette));
    }
    out.sort_by(|a, b| (&a.file, a.line, a.col).cmp(&(&b.file, b.line, b.col)));
    out
}

fn to_diag(rule: &'static str, severity: Severity, h: Hit) -> Diagnostic {
    Diagnostic {
        severity,
        rule,
        file: h.file,
        line: h.line,
        col: h.col,
        message: h.message,
    }
}

// ----------------------------------------------------------------- sprite rules

fn palette_chars(s: &SpriteSource, p: &Palette) -> Vec<Hit> {
    let mut out = Vec::new();
    for f in &s.frames {
        for row in &f.rows {
            for (i, ch) in row.text.chars().enumerate() {
                if p.get(ch).is_none() {
                    out.push(hit(
                        &s.file,
                        row.line,
                        i + 1,
                        format!("`{ch}` is not a palette char (see {})", p.file),
                    ));
                }
            }
        }
    }
    for v in &s.variants {
        for r in &v.remap {
            for ch in [r.from, r.to] {
                if p.get(ch).is_none() {
                    out.push(hit(
                        &s.file,
                        r.pos.line,
                        r.pos.col,
                        format!(
                            "variant `{}` remaps `{ch}`, which is not a palette char",
                            v.name
                        ),
                    ));
                }
            }
        }
    }
    out
}

fn row_width(s: &SpriteSource, _: &Palette) -> Vec<Hit> {
    let mut out = Vec::new();
    for f in &s.frames {
        for row in &f.rows {
            let len = row.text.chars().count() as u32;
            if len != s.width {
                let col = (len.min(s.width) + 1) as usize;
                out.push(hit(
                    &s.file,
                    row.line,
                    col,
                    format!(
                        "row of frame `{} {}` is {len} wide, but `size` is {}x{}",
                        f.anim, f.index, s.width, s.height
                    ),
                ));
            }
        }
    }
    out
}

fn frame_height(s: &SpriteSource, _: &Palette) -> Vec<Hit> {
    s.frames
        .iter()
        .filter(|f| f.rows.len() as u32 != s.height)
        .map(|f| {
            hit(
                &s.file,
                f.line,
                0,
                format!(
                    "frame `{} {}` has {} rows, but `size` is {}x{}",
                    f.anim,
                    f.index,
                    f.rows.len(),
                    s.width,
                    s.height
                ),
            )
        })
        .collect()
}

fn frame_count(s: &SpriteSource, _: &Palette) -> Vec<Hit> {
    let mut out = Vec::new();
    for a in &s.anims {
        let missing: Vec<String> = (0..a.frames)
            .filter(|&i| s.frame(&a.name, i).is_none())
            .map(|i| i.to_string())
            .collect();
        if !missing.is_empty() {
            out.push(hit(
                &s.file,
                a.line,
                0,
                format!(
                    "anim `{}` declares {} frames, but frame(s) {} are missing",
                    a.name,
                    a.frames,
                    missing.join(", ")
                ),
            ));
        }
    }
    for f in &s.frames {
        if let Some(a) = s.anim(&f.anim)
            && f.index >= a.frames
        {
            out.push(hit(
                &s.file,
                f.line,
                0,
                format!(
                    "frame `{} {}` is beyond `frames={}` declared on line {}",
                    f.anim, f.index, a.frames, a.line
                ),
            ));
        }
    }
    out
}

fn empty_frame(s: &SpriteSource, p: &Palette) -> Vec<Hit> {
    s.frames
        .iter()
        .filter(|f| !EMPTY_FRAME_ALLOWED.contains(&(s.name.as_str(), f.anim.as_str(), f.index)))
        .filter(|f| {
            f.rows
                .iter()
                .flat_map(|r| r.text.chars())
                .all(|ch| p.colour(ch) == Some(None))
        })
        .map(|f| {
            hit(
                &s.file,
                f.line,
                0,
                format!("frame `{} {}` is fully transparent", f.anim, f.index),
            )
        })
        .collect()
}

fn outline(s: &SpriteSource, _: &Palette) -> Vec<Hit> {
    if !OUTLINE_GROUPS.contains(&s.group.as_str()) {
        return Vec::new();
    }
    s.frames
        .iter()
        .filter(|f| !f.rows.iter().any(|r| r.text.contains(OUTLINE_CHAR)))
        .map(|f| {
            hit(
                &s.file,
                f.line,
                0,
                format!(
                    "frame `{} {}` has no outline pixels `{OUTLINE_CHAR}` ({} sprites need a 1 px outline)",
                    f.anim, f.index, s.group
                ),
            )
        })
        .collect()
}

fn bounds(s: &SpriteSource, _: &Palette) -> Vec<Hit> {
    let mut out = Vec::new();
    if s.origin.x > s.width || s.origin.y > s.height {
        out.push(hit(
            &s.file,
            s.origin_pos.line,
            s.origin_pos.col,
            format!(
                "origin {},{} lies outside the {}x{} sprite",
                s.origin.x, s.origin.y, s.width, s.height
            ),
        ));
    }
    let h = s.hitbox;
    // u64: user-supplied values near u32::MAX must not overflow.
    let right = u64::from(h.x) + u64::from(h.w);
    let bottom = u64::from(h.y) + u64::from(h.h);
    if h.w == 0 || h.h == 0 || right > u64::from(s.width) || bottom > u64::from(s.height) {
        out.push(hit(
            &s.file,
            s.hitbox_pos.line,
            s.hitbox_pos.col,
            format!(
                "hitbox {},{},{},{} is empty or exceeds the {}x{} sprite",
                h.x, h.y, h.w, h.h, s.width, s.height
            ),
        ));
    }
    out
}

fn bg_tileable(s: &SpriteSource, _: &Palette) -> Vec<Hit> {
    if s.group != BG_GROUP || s.width < 2 {
        return Vec::new();
    }
    let mut out = Vec::new();
    for f in &s.frames {
        let rows: Vec<Vec<char>> = f.rows.iter().map(|r| r.text.chars().collect()).collect();
        if rows.iter().any(|r| r.len() != s.width as usize) {
            continue; // `row-width` reports it
        }
        let differ = |a: usize, b: usize| rows.iter().filter(|r| r[a] != r[b]).count();
        let w = s.width as usize;
        let inner = (0..w - 1).map(|x| differ(x, x + 1)).max().unwrap_or(0);
        let seam = differ(w - 1, 0);
        if seam > inner {
            out.push(hit(
                &s.file,
                f.line,
                0,
                format!(
                    "frame `{} {}` does not tile: the last column and column 0 differ in {seam} rows \
                     (neighbouring columns inside differ in at most {inner})",
                    f.anim, f.index
                ),
            ));
        }
    }
    out
}

fn bg_size(s: &SpriteSource, _: &Palette) -> Vec<Hit> {
    let (mw, mh) = BG_MAX_SIZE;
    if s.group == BG_GROUP && (s.width > mw || s.height > mh) {
        vec![hit(
            &s.file,
            s.size_line,
            0,
            format!(
                "background `{}` is {}x{}, larger than {mw}x{mh}",
                s.name, s.width, s.height
            ),
        )]
    } else {
        Vec::new()
    }
}

fn file_name(s: &SpriteSource, _: &Palette) -> Vec<Hit> {
    let stem = s
        .file
        .rsplit(['/', '\\'])
        .next()
        .and_then(|f| f.strip_suffix(".sprite"));
    match stem {
        Some(stem) if stem != s.name => vec![hit(
            &s.file,
            s.name_line,
            0,
            format!(
                "sprite `{}` lives in `{stem}.sprite`; name the file `{}.sprite`",
                s.name, s.name
            ),
        )],
        _ => Vec::new(),
    }
}

// ---------------------------------------------------------------- project rules

fn palette_size(p: &Palette, _: &[SpriteSource]) -> Vec<Hit> {
    let n = p.colour_count();
    if n > MAX_COLOURS {
        vec![hit(
            &p.file,
            0,
            0,
            format!("palette has {n} colours, the maximum is {MAX_COLOURS}"),
        )]
    } else {
        Vec::new()
    }
}

fn duplicate_name(_: &Palette, sprites: &[SpriteSource]) -> Vec<Hit> {
    // name -> (file, line) of the first definition
    let mut seen: BTreeMap<&str, (&str, usize)> = BTreeMap::new();
    let mut out = Vec::new();
    for s in sprites {
        let names = std::iter::once((s.name.as_str(), s.name_line))
            .chain(s.variants.iter().map(|v| (v.name.as_str(), v.line)));
        for (name, line) in names {
            match seen.get(name) {
                Some(&(file, first)) => out.push(hit(
                    &s.file,
                    line,
                    0,
                    format!("sprite name `{name}` is already used at {file}:{first}"),
                )),
                None => {
                    seen.insert(name, (&s.file, line));
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sprite::parse_sprite;

    const PAL: &str = ". none t\nk #000000 k\nw #ffffff w\nr #ff0000 r\n";

    fn pal() -> Palette {
        Palette::parse("p.palette", PAL).unwrap()
    }

    /// 3x2 sprite, two-frame idle; `file` = `<group>/a.sprite`.
    fn sprite(group: &str, body: &str) -> SpriteSource {
        let src = format!("sprite = a\nsize = 3x2\nanim = idle frames=2 ticks=8\n---\n{body}");
        parse_sprite(&format!("{group}/a.sprite"), group, &src).unwrap()
    }

    const GOOD: &str = "frame idle 0\nkwk\nkrk\nframe idle 1\nkwk\nkkk\n";

    fn rules(s: &SpriteSource) -> Vec<(&'static str, usize, usize)> {
        lint_sprite(s, &pal())
            .into_iter()
            .map(|d| (d.rule, d.line, d.col))
            .collect()
    }

    #[test]
    fn good_sprite_is_clean() {
        assert!(rules(&sprite("player", GOOD)).is_empty());
    }

    #[test]
    fn palette_chars_rule() {
        let s = sprite("test", &GOOD.replace("krk", "kXk"));
        assert_eq!(rules(&s), [("palette-chars", 7, 2)]);
    }

    #[test]
    fn row_width_rule() {
        let s = sprite("test", &GOOD.replace("krk", "krkk"));
        assert_eq!(rules(&s), [("row-width", 7, 4)]);
        let s = sprite("test", &GOOD.replace("krk", "k"));
        assert_eq!(rules(&s), [("row-width", 7, 2)]);
    }

    #[test]
    fn frame_height_rule() {
        let s = sprite("test", &GOOD.replace("kwk\nkkk\n", "kwk\nkkk\nkkk\n"));
        assert_eq!(rules(&s), [("frame-height", 8, 0)]);
    }

    #[test]
    fn frame_count_rule() {
        let s = sprite("test", "frame idle 0\nkwk\nkrk\n");
        assert_eq!(rules(&s), [("frame-count", 3, 0)]);
        let s = sprite("test", &format!("{GOOD}frame idle 2\nkkk\nkkk\n"));
        assert_eq!(rules(&s), [("frame-count", 11, 0)]);
    }

    #[test]
    fn empty_frame_rule() {
        let s = sprite("test", &GOOD.replace("kwk\nkkk", "...\n..."));
        assert_eq!(rules(&s), [("empty-frame", 8, 0)]);
    }

    #[test]
    fn empty_frame_allowed_only_for_font_space() {
        let src = "sprite = font\nsize = 3x2\nanim = glyphs frames=2 ticks=1\n---\n\
                   frame glyphs 0\n...\n...\nframe glyphs 1\n...\n...\n";
        let s = parse_sprite("ui/font.sprite", "ui", src).unwrap();
        // Frame 0 (space) is allowed; frame 1 (line 8) is still reported.
        assert_eq!(rules(&s), [("empty-frame", 8, 0)]);
    }

    #[test]
    fn bg_tileable_rule_compares_the_seam_with_inner_columns() {
        let bg = |rows: &str| {
            let src = format!("sprite = a\nsize = 4x2\nanim = idle frames=1\n---\nframe 0\n{rows}");
            parse_sprite("bg/a.sprite", "bg", &src).unwrap()
        };
        // Seam (col 3 -> col 0) differs in 1 row, inner pairs in up to 1: fine.
        assert!(rules(&bg("rwww\nwwww\n")).is_empty());
        // Seam differs in 2 rows, inner pairs in at most 1: warning.
        let s = bg("rrrw\nrwww\n");
        assert_eq!(rules(&s), [("bg-tileable", 5, 0)]);
        assert!(lint_sprite(&s, &pal())[0].severity == Severity::Warning);
        // Other groups are not checked.
        let src = "sprite = a\nsize = 4x2\nanim = idle frames=1\n---\nframe 0\nrrrw\nrwww\n";
        assert!(rules(&parse_sprite("tiles/a.sprite", "tiles", src).unwrap()).is_empty());
    }

    #[test]
    fn bg_size_rule() {
        let mut s = sprite("bg", GOOD);
        assert!(!rules(&s).iter().any(|r| r.0 == "bg-size"));
        s.width = 641;
        assert!(rules(&s).iter().any(|r| r.0 == "bg-size"));
    }

    #[test]
    fn outline_rule_only_for_character_groups() {
        let body = GOOD.replace("kwk\nkkk", "www\nrrr");
        assert_eq!(rules(&sprite("enemies", &body)), [("outline", 8, 0)]);
        assert_eq!(rules(&sprite("items", &body)), [("outline", 8, 0)]);
        assert!(rules(&sprite("tiles", &body)).is_empty());
    }

    #[test]
    fn bounds_rule_is_an_error() {
        let src = "sprite = b\nsize = 2x2\norigin = 3,1\nhitbox = 1,0,2,2\nanim = i frames=1\n---\nframe 0\nkk\nkk\n";
        let s = parse_sprite("test/b.sprite", "test", src).unwrap();
        let got: Vec<_> = lint_sprite(&s, &pal())
            .iter()
            .map(|d| (d.rule, d.line, d.col, d.severity))
            .collect();
        assert_eq!(
            got,
            [
                ("bounds", 3, 10, Severity::Error),
                ("bounds", 4, 10, Severity::Error),
            ]
        );
    }

    #[test]
    fn bounds_rule_survives_huge_values() {
        let src = "sprite = b\nsize = 2x2\norigin = 4294967295,4294967295\nhitbox = 4294967295,0,1,1\nanim = i frames=1\n---\nframe 0\nkk\nkk\n";
        let s = parse_sprite("test/b.sprite", "test", src).unwrap();
        let d = lint_sprite(&s, &pal());
        let shown: Vec<String> = d.iter().map(|d| d.to_string()).collect();
        assert_eq!(shown.len(), 2, "{shown:?}");
        assert!(shown[1].starts_with("test/b.sprite:4:10: error[bounds]: hitbox 4294967295,0,1,1"));
        let hw = "sprite = b\nsize = 2x2\nhitbox = 1,1,4294967295,4294967295\nanim = i frames=1\n---\nframe 0\nkk\nkk\n";
        let s = parse_sprite("test/b.sprite", "test", hw).unwrap();
        assert_eq!(lint_sprite(&s, &pal())[0].rule, "bounds");
    }

    #[test]
    fn file_name_rule_is_a_warning() {
        let src = "sprite = b\nsize = 2x2\nanim = i frames=1\n---\nframe 0\nkk\nkk\n";
        let s = parse_sprite("test/a.sprite", "test", src).unwrap();
        let got: Vec<_> = lint_sprite(&s, &pal())
            .iter()
            .map(|d| (d.rule, d.line, d.severity))
            .collect();
        assert_eq!(got, [("file-name", 1, Severity::Warning)]);
    }

    #[test]
    fn duplicate_name_rule_includes_variants() {
        let a = sprite("player", GOOD);
        let src =
            "sprite = c\nsize = 1x1\nanim = i frames=1\nvariant = a remap k->w\n---\nframe 0\nk\n";
        let c = parse_sprite("test/c.sprite", "test", src).unwrap();
        let d: Vec<_> = lint_all(&pal(), &[a, c])
            .into_iter()
            .filter(|d| d.rule == "duplicate-name")
            .collect();
        assert_eq!(d.len(), 1);
        assert_eq!(
            d[0].to_string(),
            "test/c.sprite:4: error[duplicate-name]: sprite name `a` is already used at player/a.sprite:1"
        );
    }

    #[test]
    fn palette_size_rule() {
        let mut src = String::new();
        for i in 0..33u8 {
            src.push_str(&format!("{} #0000{i:02x} c{i}\n", (b'A' + i) as char));
        }
        let big = Palette::parse("big.palette", &src).unwrap();
        let d = lint_all(&big, &[]);
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].rule, "palette-size");
        assert!(lint_all(&pal(), &[]).is_empty());
    }

    #[test]
    fn rule_ids_are_unique() {
        let mut ids: Vec<&str> = SPRITE_RULES.iter().map(|r| r.id).collect();
        ids.extend(PROJECT_RULES.iter().map(|r| r.id));
        let n = ids.len();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), n);
    }
}
