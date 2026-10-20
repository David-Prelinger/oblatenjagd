//! Level signs (D-018, sprite catalog "Props and goal"): the board sprite
//! by style with its origin at the sign's feet (bottom centre of its cell),
//! the text from its key centred in the board's text area, and for an
//! Ortsende sign the red bar sprite over the text.
//!
//! Text area (relative to the board's top-left corner, catalog: ≥ 74×20):
//! x 3..=76, y 3..=22. One or two lines (`\n`), each centred horizontally;
//! the block of lines is centred vertically (one line at y 9, two at 4 and
//! 13). Colours: street `w` with a `k` shadow; bus `k` without shadow;
//! Ortsende `k`, a one-line name 1 px higher (the bar crosses its middle).
//! Without the sprite a grey-box board of the same size is drawn.

use jnr_artkit::atlas::{Atlas, AtlasSprite};
use jnr_core::entity::Entity;
use jnr_core::{Camera, Game, SignStyle, Texts};

use crate::Framebuffer;
use crate::blit::draw_sprite;
use crate::bounds::{PxRect, opaque_box};
use crate::entities::{CREAM, DIRNDL_RED, EntityLooks};
use crate::font::{Font, LINE_HEIGHT};
use crate::palette::{OUTLINE, WHITE};

/// Board sprite per style, in [`style_index`] order.
pub const SIGN_SPRITES: [&str; 3] = ["sign_street", "sign_bus", "sign_ortsende"];
/// The red bar drawn over the Ortsende sign's text.
pub const ORTSENDE_BAR_SPRITE: &str = "sign_ortsende_bar";

/// Text area: left edge, width, top edge and height inside the board.
pub const AREA_X: i32 = 3;
pub const AREA_W: i32 = 74;
pub const AREA_Y: i32 = 3;
pub const AREA_H: i32 = 20;
/// Most lines on a board.
pub const MAX_LINES: usize = 2;

/// Grey-box board (80×24) on a 2×4 px post: 28 px tall like `sign_street`.
const BOARD_W: i32 = 80;
const BOARD_H: i32 = 24;
const POST_H: i32 = 4;

/// Index of a style in [`SIGN_SPRITES`].
pub const fn style_index(style: SignStyle) -> usize {
    match style {
        SignStyle::Street => 0,
        SignStyle::Bus => 1,
        SignStyle::Ortsende => 2,
    }
}

/// Text colour and shadow of a style.
pub const fn text_colours(style: SignStyle) -> (u32, Option<u32>) {
    match style {
        SignStyle::Street => (WHITE, Some(OUTLINE)),
        SignStyle::Bus | SignStyle::Ortsende => (OUTLINE, None),
    }
}

/// Top-left corners of the lines (relative to the board's top-left) for
/// lines of the given pixel widths (at most [`MAX_LINES`] are used).
pub fn line_positions(widths: &[i32], style: SignStyle) -> [(i32, i32); MAX_LINES] {
    let n = widths.len().clamp(1, MAX_LINES) as i32;
    let block = n * LINE_HEIGHT - 1;
    let mut top = AREA_Y + (AREA_H - block) / 2;
    if n == 1 && style == SignStyle::Ortsende {
        top -= 1;
    }
    let mut out = [(0, 0); MAX_LINES];
    for (i, w) in widths.iter().take(MAX_LINES).enumerate() {
        out[i] = (AREA_X + (AREA_W - w) / 2, top + i as i32 * LINE_HEIGHT);
    }
    out
}

/// The board's top-left corner relative to the sign's feet: the sprite's
/// top-left (its origin is the feet), else the grey-box board's.
pub fn board_top_left(sprite: Option<&AtlasSprite>) -> (i32, i32) {
    match sprite {
        Some(s) => (-(s.origin.x as i32), -(s.origin.y as i32)),
        None => (-BOARD_W / 2, -POST_H - BOARD_H),
    }
}

/// The board face (frame included, posts and pole left out) relative to
/// the sign's feet. For a sprite: the opaque box of the leading rows of
/// its first frame that are at least half the sprite's width opaque (the
/// posts below are narrower); for the grey box its 80×24 board.
pub fn board_face(atlas: &Atlas, sprite: Option<&AtlasSprite>) -> PxRect {
    let (left, top) = board_top_left(sprite);
    let Some(frame) = sprite.and_then(|s| s.anims.first()?.frames.first().copied()) else {
        return PxRect::new(left, top, BOARD_W, BOARD_H);
    };
    let rows = (0..frame.h)
        .take_while(|&y| {
            let row = atlas.row(frame, y);
            let opaque = (0..frame.w).filter(|&x| row[x as usize * 4 + 3] != 0);
            2 * opaque.count() as u32 >= frame.w
        })
        .count() as u32;
    opaque_box(atlas, frame, (0, rows))
        .unwrap_or(PxRect::new(0, 0, frame.w as i32, frame.h as i32))
        .offset(left, top)
}

/// Pixel widths of the (at most [`MAX_LINES`]) lines of a sign text.
pub fn line_widths(font: &Font, text: &str) -> ([i32; MAX_LINES], usize) {
    let mut widths = [0; MAX_LINES];
    let mut n = 0;
    for line in text.split('\n').take(MAX_LINES) {
        widths[n] = font.measure(line);
        n += 1;
    }
    (widths, n)
}

/// The text as drawn (each line's glyph cells, 8 px tall, plus the
/// shadow at (+1, +1) for styles that have one) relative to the board's
/// top-left corner; `None` if nothing is drawn (empty text or no font).
pub fn text_box(font: &Font, text: &str, style: SignStyle) -> Option<PxRect> {
    let (widths, n) = line_widths(font, text);
    let shadow = i32::from(text_colours(style).1.is_some());
    line_positions(&widths[..n], style)
        .iter()
        .zip(&widths[..n])
        .filter(|&(_, &w)| w > 0)
        .map(|(&(x, y), &w)| PxRect::new(x, y, w + shadow, LINE_HEIGHT - 1 + shadow))
        .reduce(PxRect::union)
}

/// Everything of a sign that may hide what is drawn behind it, relative
/// to its feet: the board face ([`board_face`]) and, if its text overflows
/// the board, the text ([`text_box`]). The posts are left out.
pub fn board_box(
    atlas: &Atlas,
    sprite: Option<&AtlasSprite>,
    font: &Font,
    text: Option<&str>,
    style: SignStyle,
) -> PxRect {
    let face = board_face(atlas, sprite);
    let (left, top) = board_top_left(sprite);
    match text.and_then(|t| text_box(font, t, style)) {
        Some(t) => face.union(t.offset(left, top)),
        None => face,
    }
}

/// Draws one sign entity.
#[allow(clippy::too_many_arguments)]
pub fn draw_sign(
    game: &Game,
    looks: &EntityLooks,
    atlas: &Atlas,
    font: &Font,
    texts: &Texts,
    fb: &mut Framebuffer,
    cam: Camera,
    e: &Entity,
) {
    let Some(spawn) = e
        .spawn
        .and_then(|i| game.level().entities.get(usize::from(i)))
    else {
        return;
    };
    let style = spawn.style;
    let (fx, fy) = e.feet();
    let (x, y) = (fx.to_px() - cam.x, fy.to_px() - cam.y);
    let tick = game.frame();
    let sprite = looks.signs[style_index(style)];
    let (dx, dy) = board_top_left(sprite);
    let (left, top) = (x + dx, y + dy);
    match sprite {
        Some(s) => draw_sprite(fb, atlas, s, &s.anims[0], tick, x, y, false),
        None => {
            let (bx, by) = (left, top);
            fb.fill_rect(x - 1, y - POST_H, 2, POST_H, OUTLINE);
            let board = if style == SignStyle::Ortsende {
                WHITE
            } else {
                CREAM
            };
            fb.fill_rect(bx, by, BOARD_W, BOARD_H, board);
            fb.outline_rect(bx, by, BOARD_W, BOARD_H, OUTLINE);
        }
    }
    if let Some(key) = &spawn.text {
        let text = texts.get(key);
        let (widths, n) = line_widths(font, &text);
        let pos = line_positions(&widths[..n], style);
        let (colour, shadow) = text_colours(style);
        for (line, (lx, ly)) in text.split('\n').take(MAX_LINES).zip(pos) {
            font.draw_text(fb, atlas, line, left + lx, top + ly, colour, shadow);
        }
    }
    if style == SignStyle::Ortsende {
        draw_ortsende_bar(fb, atlas, looks.ortsende_bar, tick, x, y, left, top);
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_ortsende_bar(
    fb: &mut Framebuffer,
    atlas: &Atlas,
    bar: Option<&AtlasSprite>,
    tick: u32,
    x: i32,
    y: i32,
    left: i32,
    top: i32,
) {
    match bar {
        Some(s) => draw_sprite(fb, atlas, s, &s.anims[0], tick, x, y, false),
        None => {
            // A 2 px red diagonal across the text area, bottom left to top right.
            let (ax, ay) = (left + AREA_X, top + AREA_Y);
            for i in 0..AREA_W {
                let py = ay + AREA_H - 1 - i * (AREA_H - 1) / (AREA_W - 1);
                fb.fill_rect(ax + i, py - 1, 1, 2, DIRNDL_RED);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::font::{CHARSET, FONT_SPRITE, GLYPH_ANIM};
    use crate::scene::{RenderContext, render};
    use crate::testutil::{self, image};
    use jnr_core::{Level, Tuning};

    #[test]
    fn one_and_two_lines_are_centred_in_the_text_area() {
        // One line, 40 px: x 3 + 17, y 3 + (20 - 8) / 2.
        assert_eq!(line_positions(&[40], SignStyle::Street)[0], (20, 9));
        assert_eq!(line_positions(&[40], SignStyle::Bus)[0], (20, 9));
        assert_eq!(
            line_positions(&[40], SignStyle::Ortsende)[0],
            (20, 8),
            "1 px up"
        );
        // Two lines (17 px block): tops at 4 and 13, each centred on its own.
        let two = line_positions(&[74, 10], SignStyle::Street);
        assert_eq!(two, [(3, 4), (35, 13)]);
        assert_eq!(line_positions(&[74, 10], SignStyle::Ortsende), two);
        // The 2nd line's last pixel row (13 + 7) stays inside y 3..=22.
        assert!(two[1].1 + 7 < AREA_Y + AREA_H);
    }

    #[test]
    fn colours_by_style() {
        assert_eq!(text_colours(SignStyle::Street), (WHITE, Some(OUTLINE)));
        assert_eq!(text_colours(SignStyle::Bus), (OUTLINE, None));
        assert_eq!(text_colours(SignStyle::Ortsende), (OUTLINE, None));
    }

    fn sign_game(style: &str, key: &str) -> Game {
        let text = format!(
            "[meta]\nid = \"t\"\ntime_limit = 300\n[legend]\n\"#\" = \"solid\"\n\
             \"P\" = {{ entity = \"player_start\" }}\n\
             \"s\" = {{ entity = \"sign\", text = \"{key}\", style = \"{style}\" }}\n\
             [map]\ngrid = '''\n....................\n....................\n....................\n.P......s...........\n####################\n'''\n"
        );
        let tuning = Tuning::parse(include_str!("../../../content/tuning.toml")).unwrap();
        let mut g = Game::new(&Level::parse(&text).unwrap(), &tuning, 1);
        g.step(jnr_core::InputFrame::NONE); // activates the sign
        g
    }

    /// Board sprite 80x28 (`c`), origin bottom centre; font: every glyph a
    /// 5x7 block of `a` (advance 6).
    fn atlas(with_board: bool) -> jnr_artkit::Atlas {
        let glyph = image(&[
            "aaaaa...", "aaaaa...", "aaaaa...", "aaaaa...", "aaaaa...", "aaaaa...", "aaaaa...",
            "........",
        ]);
        let mut sprites = vec![testutil::sprite(
            FONT_SPRITE,
            (0, 0),
            false,
            vec![(
                GLYPH_ANIM,
                0,
                CHARSET.chars().map(|_| glyph.clone()).collect(),
            )],
        )];
        if with_board {
            let row = "c".repeat(80);
            let rows: Vec<&str> = vec![row.as_str(); 28];
            sprites.push(testutil::sprite(
                "sign_street",
                (40, 28),
                false,
                vec![("idle", 0, vec![image(&rows)])],
            ));
        }
        testutil::atlas(&sprites)
    }

    /// Sign sprite 80x28: a 26-row board face (the first and last rows
    /// with transparent corners) on two 2-wide posts.
    fn board_atlas() -> jnr_artkit::Atlas {
        let edge = format!(".{}.", "c".repeat(78));
        let full = "c".repeat(80);
        let post = format!("{}cc{}cc{}", ".".repeat(8), ".".repeat(60), ".".repeat(8));
        let mut rows: Vec<&str> = vec![edge.as_str()];
        rows.extend(std::iter::repeat_n(full.as_str(), 24));
        rows.push(edge.as_str());
        rows.extend([post.as_str(), post.as_str()]);
        testutil::atlas(&[testutil::sprite(
            "sign_street",
            (40, 28),
            false,
            vec![("idle", 0, vec![image(&rows)])],
        )])
    }

    #[test]
    fn board_face_leaves_out_the_posts() {
        let atlas = board_atlas();
        let s = atlas.sprite("sign_street");
        assert_eq!(board_top_left(s), (-40, -28));
        assert_eq!(board_face(&atlas, s), PxRect::new(-40, -28, 80, 26));
        // Grey box: the 80x24 board above its 4 px post.
        assert_eq!(board_top_left(None), (-40, -28));
        assert_eq!(board_face(&atlas, None), PxRect::new(-40, -28, 80, 24));
    }

    #[test]
    fn multi_line_text_box_is_as_wide_as_its_widest_line() {
        let font = Font::from_atlas(&atlas(false));
        // Glyphs advance 6, last gap 1: "ABC" 17 px, 15 letters 89 px.
        let wide = "ABCDEFGHIJKLMNO";
        assert_eq!(line_widths(&font, &format!("ABC\n{wide}")), ([17, 89], 2));
        // Street (shadow +1): line 1 at (31, 4), line 2 at (3 - 7, 13).
        let t = text_box(&font, &format!("ABC\n{wide}"), SignStyle::Street);
        assert_eq!(t, Some(PxRect::new(-4, 4, 90, 18)));
        // Bus (no shadow), the wide line first: same x, 1 px narrower.
        let t = text_box(&font, &format!("{wide}\nABC"), SignStyle::Bus);
        assert_eq!(t, Some(PxRect::new(-4, 4, 89, 17)));
        // Lines that fit stay inside the face; one that overflows widens it.
        let atlas = board_atlas();
        let s = atlas.sprite("sign_street");
        let fits = board_box(&atlas, s, &font, Some("ABC\nDEF"), SignStyle::Street);
        assert_eq!(fits, PxRect::new(-40, -28, 80, 26));
        let text = format!("ABC\n{wide}");
        let over = board_box(&atlas, s, &font, Some(&text), SignStyle::Street);
        assert_eq!(over, PxRect::new(-44, -28, 90, 26));
        assert_eq!(board_box(&atlas, s, &font, None, SignStyle::Street).w, 80);
    }

    #[test]
    fn street_sign_text_is_white_with_shadow_and_centred() {
        let texts = Texts::parse("[t]\ns = \"ABC\"\n").unwrap();
        let g = sign_game("street", "t.s");
        let atlas = atlas(true);
        let ctx = RenderContext::new(&atlas, &texts, g.level());
        let mut fb = testutil::fb();
        render(&g, &ctx, &mut fb);
        // Feet at (8 * 16 + 8, 64) = (136, 64): board x 96..176, y 36..64.
        assert_eq!(fb.get(96, 36), Some(testutil::C));
        // "ABC" = 3 * 6 - 1 = 17 px: x 96 + 3 + 28 = 127, y 36 + 9 = 45.
        assert_eq!(fb.get(127, 45), Some(WHITE));
        assert_eq!(fb.get(126, 45), Some(testutil::C));
        assert_eq!(fb.get(143, 51), Some(WHITE), "last column of C");
        assert_eq!(fb.get(144, 52), Some(OUTLINE), "shadow");
    }

    #[test]
    fn grey_box_ortsende_board_with_dark_text_and_bar() {
        let texts = Texts::parse("[t]\ns = \"AB\"\n").unwrap();
        let g = sign_game("ortsende", "t.s");
        let atlas = atlas(false);
        let ctx = RenderContext::new(&atlas, &texts, g.level());
        let mut fb = testutil::fb();
        render(&g, &ctx, &mut fb);
        // Grey box: board x 96..176, y 36..60 (white), post below.
        assert_eq!(fb.get(97, 37), Some(WHITE));
        assert_eq!(fb.get(136, 62), Some(OUTLINE), "post");
        // "AB" = 11 px: x 96 + 3 + 31 = 130, one line 1 px up: y 36 + 8 = 44.
        assert_eq!(fb.get(130, 44), Some(OUTLINE));
        assert_eq!(fb.get(130, 43), Some(WHITE));
        assert_eq!(fb.get(129, 44), Some(WHITE));
        // The red bar starts bottom left of the text area.
        assert_eq!(fb.get(99, 36 + 3 + 19), Some(DIRNDL_RED));
    }
}
