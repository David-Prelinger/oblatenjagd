//! Oblaten clear of sign boards (M5, E33, D-086 addendum): signs are drawn
//! first, so an Oblate placed on a board's rectangle covers its text.
//!
//! For every `sign` the board is computed as the renderer draws it
//! (`jnr_render::signs::board_box`: the board face with its frame, the
//! posts left out, widened by any text line that overflows it), with the
//! real atlas, font and `de.toml` text. For every placed `oblate` entity
//! (`?`-box and brick contents are not placed) its drawn sprite box
//! (`jnr_render::bounds::body_box`, every spin frame) at its spawn feet.
//! An overlap of at least 1 px, after growing the board by [`MARGIN_PX`],
//! is an error, unless the Oblate is in [`KNOWN`]: then it is a warning
//! (and a stale [`KNOWN`] entry is an error).

use jnr_core::entity::Entity;
use jnr_core::{Level, SpawnKind, Texts, Tuning};
use jnr_render::Font;
use jnr_render::bounds::{PxRect, body_box};
use jnr_render::signs::{SIGN_SPRITES, board_box, style_index};

/// Clear pixels required between an Oblate and a board (0: they may
/// touch, but share no pixel).
pub const MARGIN_PX: i32 = 0;

/// Known overlaps that only warn: `(level id, Oblate row, Oblate column)`,
/// 1-based as in the messages. The findings in Stages 2–5 when the lint
/// came (E33); they are the level-designer's to fix, then the entry goes
/// (a stale entry fails). Never add an entry for a new placement.
pub const KNOWN: &[(&str, i32, i32)] = &[
    // The bus-stop board's bottom frame row on the Oblaten's top row (1 px).
    ("st_gilgen", 15, 7),
    ("st_gilgen", 15, 8),
    ("brunnwinkl", 15, 7),
    ("brunnwinkl", 15, 8),
    ("falkenstein", 40, 8),
    ("st_wolfgang", 15, 8),
    // Wholly or largely on the board's face (12x12 / 12x9 px).
    ("falkenstein", 11, 166),
    ("falkenstein", 11, 167),
    ("falkenstein", 38, 31),
    ("falkenstein", 38, 32),
    ("st_wolfgang", 13, 47),
    ("st_wolfgang", 13, 51),
];

/// One Oblate on one board.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Finding {
    /// The Oblate's cell, 1-based `(row, column)`.
    pub oblate: (i32, i32),
    /// The sign's cell, 1-based `(row, column)`.
    pub sign: (i32, i32),
    /// The sign's text, lines joined by ` / `.
    pub text: String,
    /// The shared pixels (level px), after the margin.
    pub overlap: PxRect,
}

impl std::fmt::Display for Finding {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let o = self.overlap;
        write!(
            f,
            "Oblate at row {}, column {} overlaps the sign board at row {}, column {} (\"{}\") by {}x{} px at ({}, {}) (E33: keep Oblaten clear of sign boards)",
            self.oblate.0, self.oblate.1, self.sign.0, self.sign.1, self.text, o.w, o.h, o.x, o.y
        )
    }
}

/// Every Oblate–board overlap in a level, in entity order (signs outer).
pub fn findings(level: &Level, texts: &Texts, tuning: &Tuning, margin: i32) -> Vec<Finding> {
    let atlas = jnr_assets::atlas();
    let font = Font::from_atlas(atlas);
    let oblate_sprite = atlas.sprite("oblate");
    let oblaten: Vec<((i32, i32), PxRect)> = level
        .entities
        .iter()
        .enumerate()
        .filter(|(_, s)| s.kind == SpawnKind::Oblate)
        .filter_map(|(i, s)| {
            let e = Entity::from_spawn(s, i, tuning)?;
            let (fx, fy) = e.feet();
            let hitbox = PxRect::new(e.x.to_px(), e.y.to_px(), e.w.to_px(), e.h.to_px());
            let drawn = oblate_sprite
                .and_then(|sp| body_box(atlas, sp))
                .map_or(hitbox, |b| b.offset(fx.to_px(), fy.to_px()));
            Some(((s.row + 1, s.col + 1), drawn))
        })
        .collect();
    let mut out = Vec::new();
    for s in level.entities.iter().filter(|s| s.kind == SpawnKind::Sign) {
        let sprite = atlas.sprite(SIGN_SPRITES[style_index(s.style)]);
        let text = s.text.as_deref().map(|k| texts.get(k));
        let feet = (s.col * 16 + 8, (s.row + 1) * 16);
        let board = board_box(atlas, sprite, &font, text.as_deref(), s.style)
            .offset(feet.0, feet.1)
            .grow(margin);
        for &(cell, rect) in &oblaten {
            if let Some(overlap) = intersection(&board, &rect) {
                out.push(Finding {
                    oblate: cell,
                    sign: (s.row + 1, s.col + 1),
                    text: text.as_deref().unwrap_or("").replace('\n', " / "),
                    overlap,
                });
            }
        }
    }
    out
}

/// The shared pixels of two rectangles, if any.
fn intersection(a: &PxRect, b: &PxRect) -> Option<PxRect> {
    if !a.overlaps(b) {
        return None;
    }
    let (x, y) = (a.x.max(b.x), a.y.max(b.y));
    let right = (a.x + a.w).min(b.x + b.w);
    let bottom = (a.y + a.h).min(b.y + b.h);
    Some(PxRect::new(x, y, right - x, bottom - y))
}

fn is_known(known: &[(&str, i32, i32)], id: &str, f: &Finding) -> bool {
    known.contains(&(id, f.oblate.0, f.oblate.1))
}

/// Errors: findings not in `known`, and `known` entries of this level
/// that no longer overlap.
pub fn problems(
    level: &Level,
    texts: &Texts,
    tuning: &Tuning,
    known: &[(&str, i32, i32)],
) -> Vec<String> {
    let id = level.meta.id.as_str();
    let found = findings(level, texts, tuning, MARGIN_PX);
    let mut out: Vec<String> = found
        .iter()
        .filter(|f| !is_known(known, id, f))
        .map(Finding::to_string)
        .collect();
    for &(_, row, col) in known.iter().filter(|k| k.0 == id) {
        if !found.iter().any(|f| f.oblate == (row, col)) {
            out.push(format!(
                "Oblate at row {row}, column {col} is listed in signlint::KNOWN but overlaps no sign board: remove the entry"
            ));
        }
    }
    out
}

/// Warnings: findings listed in `known`.
pub fn warnings(
    level: &Level,
    texts: &Texts,
    tuning: &Tuning,
    known: &[(&str, i32, i32)],
) -> Vec<String> {
    let id = level.meta.id.as_str();
    findings(level, texts, tuning, MARGIN_PX)
        .iter()
        .filter(|f| is_known(known, id, f))
        .map(|f| format!("{f} (known, see signlint::KNOWN)"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tuning() -> Tuning {
        Tuning::parse(include_str!("../../content/tuning.toml")).unwrap()
    }

    fn texts() -> Texts {
        Texts::parse("[t]\nshort = \"Hallo\"\ntwo = \"Bad Ischl\\nCafé Zauner\"\n").unwrap()
    }

    /// Four rows above the ground; a street sign `s` at row 3, column 9
    /// (0-based) has its feet at (152, 64): with the real 80x28 sprite
    /// its face (26 rows, the 2 post rows left out) is x 112..192, y 36..62.
    /// `z` is an Ortsende sign with a two-line text.
    fn level(rows: [&str; 4]) -> Level {
        let grid = rows.join("\n");
        let legend = r##""#" = "solid"
"P" = { entity = "player_start" }
"G" = { entity = "goal" }
"o" = { entity = "oblate" }
"?" = { tile = "oblaten_box" }
"s" = { entity = "sign", text = "t.short" }
"z" = { entity = "sign", text = "t.two", style = "ortsende" }"##;
        Level::parse(&format!(
            "[meta]\nid = \"t\"\ntime_limit = 300\n[legend]\n{legend}\n\
             [map]\ngrid = '''\n{grid}\n####################\n'''\n"
        ))
        .unwrap()
    }

    /// The real Oblate's drawn box in a cell (0-based column, row).
    fn oblate_at(col: i32, row: i32) -> PxRect {
        let atlas = jnr_assets::atlas();
        body_box(atlas, atlas.sprite("oblate").unwrap())
            .unwrap()
            .offset(col * 16 + 8, row * 16 + 13)
    }

    #[test]
    fn real_sign_sprites_have_80_wide_faces_above_their_posts() {
        let atlas = jnr_assets::atlas();
        for name in SIGN_SPRITES {
            let s = atlas.sprite(name).unwrap();
            let face = jnr_render::signs::board_face(atlas, Some(s));
            let top = -(s.height as i32);
            assert_eq!((face.x, face.y, face.w), (-40, top, 80), "{name}");
            assert_eq!(face.h, 26, "{name}: the posts are left out");
        }
    }

    #[test]
    fn an_oblate_on_the_board_is_found() {
        let l = level([
            "....................",
            "....................",
            ".........o..........",
            ".P.......s.........G",
        ]);
        let f = findings(&l, &texts(), &tuning(), 0);
        assert_eq!(f.len(), 1, "{f:?}");
        assert_eq!((f[0].oblate, f[0].sign), ((3, 10), (4, 10)));
        assert_eq!(f[0].text, "Hallo");
        // The Oblate's bottom rows are on the face's top rows.
        let o = oblate_at(9, 2);
        assert_eq!((f[0].overlap.y, f[0].overlap.h), (36, o.y + o.h - 36));
        let p = problems(&l, &texts(), &tuning(), &[]);
        assert_eq!(p.len(), 1);
        assert!(
            p[0].contains("row 3, column 10") && p[0].contains("Hallo"),
            "{p:?}"
        );
        // Listed as known: a warning, not an error.
        let known = [("t", 3, 10)];
        assert!(problems(&l, &texts(), &tuning(), &known).is_empty());
        assert_eq!(warnings(&l, &texts(), &tuning(), &known).len(), 1);
    }

    #[test]
    fn adjacent_oblaten_and_box_contents_are_not_flagged() {
        // Beside the face (x 112..192): column 6 ends left of 112, column
        // 12 starts right of 192; row 0 ends above y 36. A `?` box on the
        // board is no placed Oblate.
        let (left, right, above) = (oblate_at(6, 2), oblate_at(12, 2), oblate_at(9, 0));
        assert!(left.x + left.w <= 112 && right.x >= 192 && above.y + above.h <= 36);
        let l = level([
            ".........o..........",
            "....................",
            "......o...?.o.......",
            ".P.......s.........G",
        ]);
        let f = findings(&l, &texts(), &tuning(), 0);
        assert!(f.is_empty(), "{f:?}");
        assert!(problems(&l, &texts(), &tuning(), &[]).is_empty());
        // A known entry that no longer overlaps is an error.
        let p = problems(&l, &texts(), &tuning(), &[("t", 3, 7)]);
        assert_eq!(p.len(), 1);
        assert!(p[0].contains("remove the entry"), "{p:?}");
        // With a margin the neighbours beside the board are flagged.
        let gap = (112 - left.x - left.w).max(right.x - 192);
        let near = findings(&l, &texts(), &tuning(), gap + 1);
        let cells: Vec<_> = near.iter().map(|f| f.oblate).collect();
        assert_eq!(cells, [(3, 7), (3, 13)]);
    }

    #[test]
    fn two_line_text_is_reported_and_measured_with_the_real_font() {
        let l = level([
            "....................",
            "....................",
            ".........o..........",
            ".P.......z.........G",
        ]);
        let f = findings(&l, &texts(), &tuning(), 0);
        assert_eq!(f.len(), 1, "{f:?}");
        assert_eq!(f[0].text, "Bad Ischl / Café Zauner");
        // Both lines fit the 74 px text area: the box is the face.
        let atlas = jnr_assets::atlas();
        let font = Font::from_atlas(atlas);
        let sprite = atlas.sprite("sign_ortsende");
        let style = jnr_core::SignStyle::Ortsende;
        let face = jnr_render::signs::board_face(atlas, sprite);
        let text = "Bad Ischl\nCafé Zauner";
        assert_eq!(board_box(atlas, sprite, &font, Some(text), style), face);
    }
}
