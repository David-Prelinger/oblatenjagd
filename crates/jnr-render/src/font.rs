//! The variable-width bitmap font.
//!
//! Convention (shared with the pixel-artist, see `docs/art-style.md` §11):
//! sprite [`FONT_SPRITE`] (`art/sprites/ui/font.sprite`), 8x8 frames, one
//! anim [`GLYPH_ANIM`] with one frame per character of [`CHARSET`], in that
//! order. Glyphs are drawn in `w` on transparent and tinted when drawn.
//! Rows: 0 accents/umlaut dots, 1–6 cap height (baseline row 6), 7 descenders.
//!
//! Advance per glyph = rightmost opaque column + 2 (1 px gap); space is
//! [`SPACE_ADVANCE`]. Characters not in the charset (or not drawn yet)
//! render as `?`. If the atlas has no font, drawing text is a no-op.

use jnr_artkit::atlas::{Atlas, Rect};

use crate::Framebuffer;
use crate::blit::blit_frame_tinted;

/// Every character the font has, in glyph-frame order (89 glyphs).
pub const CHARSET: &str =
    " !\"'(),-./0123456789:;?ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyzÄÖÜäöüß„“–…×%+";
/// Name of the font sprite in the atlas.
pub const FONT_SPRITE: &str = "font";
/// Name of the font sprite's only anim.
pub const GLYPH_ANIM: &str = "glyphs";
/// Pixels between the tops of two text lines (`\n`).
pub const LINE_HEIGHT: i32 = 9;
/// Advance of the space character in pixels.
pub const SPACE_ADVANCE: i32 = 3;

/// Drawn for characters the font does not have.
const FALLBACK: char = '?';

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Glyph {
    rect: Rect,
    advance: i32,
}

/// Glyph rects and advances, measured once from an atlas.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Font {
    /// `(char, glyph)` sorted by char, for binary search.
    glyphs: Vec<(char, Glyph)>,
}

impl Font {
    /// Reads the font from the atlas. Without a `font` sprite (or its
    /// `glyphs` anim) the font is empty and draws nothing. Frames beyond
    /// [`CHARSET`] are ignored; missing frames make their characters
    /// unknown (drawn as `?`).
    pub fn from_atlas(atlas: &Atlas) -> Font {
        let Some(anim) = atlas.sprite(FONT_SPRITE).and_then(|s| s.anim(GLYPH_ANIM)) else {
            return Font::default();
        };
        let mut glyphs: Vec<(char, Glyph)> = CHARSET
            .chars()
            .zip(&anim.frames)
            .map(|(ch, &rect)| {
                let advance = if ch == ' ' {
                    SPACE_ADVANCE
                } else {
                    rightmost_opaque(atlas, rect).map_or(SPACE_ADVANCE, |c| c + 2)
                };
                (ch, Glyph { rect, advance })
            })
            .collect();
        glyphs.sort_by_key(|&(ch, _)| ch);
        Font { glyphs }
    }

    /// True if the atlas had no font: drawing and measuring do nothing.
    pub fn is_empty(&self) -> bool {
        self.glyphs.is_empty()
    }

    /// Width in pixels of the widest line (sum of advances, so it includes
    /// the 1 px gap after the last glyph). 0 for an empty font.
    pub fn measure(&self, text: &str) -> i32 {
        if self.is_empty() {
            return 0;
        }
        text.split('\n')
            .map(|line| line.chars().map(|ch| self.advance(ch)).sum())
            .max()
            .unwrap_or(0)
    }

    /// Draws `text` with its top-left corner at (`x`, `y`) in `colour`.
    /// With a `shadow` colour, the whole text is first drawn in that colour
    /// at (+1, +1) (HUD style, art-style §11). `\n` starts a new line
    /// [`LINE_HEIGHT`] px lower. Never panics; clips like any blit.
    #[allow(clippy::too_many_arguments)]
    pub fn draw_text(
        &self,
        fb: &mut Framebuffer,
        atlas: &Atlas,
        text: &str,
        x: i32,
        y: i32,
        colour: u32,
        shadow: Option<u32>,
    ) {
        if self.is_empty() {
            return;
        }
        if let Some(s) = shadow {
            self.draw_pass(fb, atlas, text, x.saturating_add(1), y.saturating_add(1), s);
        }
        self.draw_pass(fb, atlas, text, x, y, colour);
    }

    fn draw_pass(&self, fb: &mut Framebuffer, atlas: &Atlas, text: &str, x: i32, y: i32, c: u32) {
        let (mut pen_x, mut pen_y) = (x, y);
        for ch in text.chars() {
            if ch == '\n' {
                pen_x = x;
                pen_y = pen_y.saturating_add(LINE_HEIGHT);
                continue;
            }
            match self.glyph(ch) {
                Some(g) => {
                    if ch != ' ' {
                        blit_frame_tinted(fb, atlas, g.rect, pen_x, pen_y, false, c);
                    }
                    pen_x = pen_x.saturating_add(g.advance);
                }
                None => pen_x = pen_x.saturating_add(SPACE_ADVANCE),
            }
        }
    }

    fn advance(&self, ch: char) -> i32 {
        self.glyph(ch).map_or(SPACE_ADVANCE, |g| g.advance)
    }

    /// The glyph for `ch`, else the `?` glyph, else `None`.
    fn glyph(&self, ch: char) -> Option<Glyph> {
        self.find(ch).or_else(|| self.find(FALLBACK))
    }

    fn find(&self, ch: char) -> Option<Glyph> {
        self.glyphs
            .binary_search_by_key(&ch, |&(c, _)| c)
            .ok()
            .map(|i| self.glyphs[i].1)
    }
}

/// Index of the rightmost column of `rect` with an opaque pixel.
fn rightmost_opaque(atlas: &Atlas, rect: Rect) -> Option<i32> {
    (0..rect.h)
        .filter_map(|y| {
            let row = atlas.row(rect, y);
            (0..rect.w).rev().find(|&x| row[x as usize * 4 + 3] != 0)
        })
        .max()
        .map(|x| x as i32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::{self, A, BG, image};
    use jnr_artkit::Image;

    const W: u32 = 0xfbf7_eeff;
    const K: u32 = 0x1a1c_2cff;

    /// Glyphs for a test font: an 8x8 frame per charset entry, only a few drawn.
    fn glyph_art(ch: char) -> Option<[&'static str; 8]> {
        Some(match ch {
            'A' => [
                "........", ".aaa....", "a...a...", "a...a...", "aaaaa...", "a...a...", "a...a...",
                "........",
            ],
            'I' => [
                "........", "a.......", "a.......", "a.......", "a.......", "a.......", "a.......",
                "........",
            ],
            'Ä' => [
                "a...a...", ".aaa....", "a...a...", "a...a...", "aaaaa...", "a...a...", "a...a...",
                "........",
            ],
            '?' => [
                "........", "aaa.....", "..a.....", ".aa.....", ".a......", "........", ".a......",
                "........",
            ],
            // Drawn in the source but always advances by SPACE_ADVANCE.
            ' ' => [
                "aaaaaaaa", "........", "........", "........", "........", "........", "........",
                "........",
            ],
            _ => return None,
        })
    }

    /// A font atlas with the first `n` glyph frames.
    fn font_atlas(n: usize) -> Atlas {
        let frames: Vec<Image> = CHARSET
            .chars()
            .take(n)
            .map(|ch| image(&glyph_art(ch).unwrap_or(["........"; 8])))
            .collect();
        testutil::atlas(&[testutil::sprite(
            FONT_SPRITE,
            (0, 0),
            false,
            vec![(GLYPH_ANIM, 0, frames)],
        )])
    }

    fn full_font() -> (Atlas, Font) {
        let atlas = font_atlas(CHARSET.chars().count());
        let font = Font::from_atlas(&atlas);
        (atlas, font)
    }

    #[test]
    fn charset_has_89_unique_glyphs() {
        let chars: Vec<char> = CHARSET.chars().collect();
        assert_eq!(chars.len(), 89);
        let mut sorted = chars.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), 89);
        for ch in "ÄÖÜäöüß„“–…×%+".chars() {
            assert!(chars.contains(&ch), "{ch}");
        }
    }

    #[test]
    fn advance_is_rightmost_column_plus_two() {
        let (_, font) = full_font();
        assert_eq!(font.measure("A"), 6);
        assert_eq!(font.measure("I"), 2);
        assert_eq!(font.measure("Ä"), 6);
        assert_eq!(font.measure(" "), SPACE_ADVANCE);
        // Empty glyph frames (not drawn yet) advance like a space.
        assert_eq!(font.measure("B"), SPACE_ADVANCE);
        assert_eq!(font.measure("AI A"), 6 + 2 + SPACE_ADVANCE + 6);
        // Widest line wins.
        assert_eq!(font.measure("I\nAA\nA"), 12);
        assert_eq!(font.measure(""), 0);
    }

    #[test]
    fn draws_tinted_glyphs_with_advance() {
        let (atlas, font) = full_font();
        let mut fb = testutil::fb();
        font.draw_text(&mut fb, &atlas, "IA", 10, 20, W, None);
        // `I`: column 0, rows 1-6.
        assert_eq!(fb.get(10, 21), Some(W));
        assert_eq!(fb.get(10, 26), Some(W));
        assert_eq!(fb.get(10, 20), Some(BG));
        // `A` starts 2 px later (x 12): top bar at x 13..=15 in row 1, sides from row 2.
        assert_eq!(fb.get(12, 21), Some(BG));
        assert_eq!(fb.get(13, 21), Some(W));
        assert_eq!(fb.get(15, 21), Some(W));
        assert_eq!(fb.get(16, 21), Some(BG));
        assert_eq!(fb.get(12, 22), Some(W));
        assert!(!fb.pixels.contains(&A), "glyphs are tinted, not copied");
    }

    #[test]
    fn umlaut_dots_sit_in_row_zero() {
        let (atlas, font) = full_font();
        let mut fb = testutil::fb();
        font.draw_text(&mut fb, &atlas, "Ä", 0, 10, W, None);
        assert_eq!(fb.get(0, 10), Some(W));
        assert_eq!(fb.get(4, 10), Some(W));
        assert_eq!(fb.get(2, 10), Some(BG));
        assert_eq!(fb.get(1, 11), Some(W));
    }

    #[test]
    fn shadow_is_drawn_first_at_plus_one() {
        let (atlas, font) = full_font();
        let mut fb = testutil::fb();
        font.draw_text(&mut fb, &atlas, "I", 0, 0, W, Some(K));
        // Text column 0 rows 1-6, shadow column 1 rows 2-7; text wins overlaps.
        assert_eq!(fb.get(0, 1), Some(W));
        assert_eq!(fb.get(0, 6), Some(W));
        assert_eq!(fb.get(1, 2), Some(K));
        assert_eq!(fb.get(1, 7), Some(K));
        assert_eq!(fb.get(0, 7), Some(BG));
        assert_eq!(fb.get(1, 1), Some(BG));
    }

    #[test]
    fn space_is_never_drawn_and_newline_moves_down() {
        let (atlas, font) = full_font();
        let mut fb = testutil::fb();
        font.draw_text(&mut fb, &atlas, " I\nI", 0, 0, W, None);
        assert_eq!(fb.get(0, 0), Some(BG), "space glyph pixels are skipped");
        assert_eq!(fb.get(SPACE_ADVANCE, 1), Some(W));
        assert_eq!(fb.get(0, LINE_HEIGHT + 1), Some(W));
    }

    #[test]
    fn unknown_chars_draw_as_question_mark() {
        let (atlas, font) = full_font();
        let mut a = testutil::fb();
        let mut b = testutil::fb();
        font.draw_text(&mut a, &atlas, "€\t\u{1F600}", 0, 0, W, None);
        font.draw_text(&mut b, &atlas, "???", 0, 0, W, None);
        assert_eq!(a, b);
        assert_ne!(a, testutil::fb());
        assert_eq!(font.measure("€"), font.measure("?"));
    }

    #[test]
    fn partial_and_missing_fonts_never_panic() {
        // Only the first 10 glyphs (no `?`): unknown chars just advance.
        let atlas = font_atlas(10);
        let font = Font::from_atlas(&atlas);
        assert!(!font.is_empty());
        let mut fb = testutil::fb();
        font.draw_text(&mut fb, &atlas, "AÄ€", 0, 0, W, Some(K));
        assert_eq!(fb, testutil::fb());
        assert_eq!(font.measure("AB"), 2 * SPACE_ADVANCE);

        // No font sprite at all: no-op.
        let other = testutil::atlas(&[testutil::sprite(
            "x",
            (0, 0),
            false,
            vec![("idle", 0, vec![image(&["a"])])],
        )]);
        let font = Font::from_atlas(&other);
        assert!(font.is_empty());
        font.draw_text(&mut fb, &other, "Hallo", 0, 0, W, Some(K));
        assert_eq!(fb, testutil::fb());
        assert_eq!(font.measure("Hallo"), 0);
    }

    #[test]
    fn text_clips_at_screen_edges() {
        let (atlas, font) = full_font();
        let mut fb = testutil::fb();
        font.draw_text(&mut fb, &atlas, "AAAA", -3, -2, W, Some(K));
        font.draw_text(&mut fb, &atlas, "AAAA", 315, 176, W, Some(K));
        font.draw_text(&mut fb, &atlas, "A", i32::MAX, i32::MAX, W, Some(K));
        // (0, 0) is glyph pixel (3, 2) of the text (empty) and (2, 1) of the shadow (set).
        assert_eq!(fb.get(0, 0), Some(K));
        assert_eq!(
            fb.get(319, 179),
            Some(W),
            "glyph pixel (4, 3) of the `A` at x 315"
        );
    }
}
