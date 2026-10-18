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
    let (left, top) = match looks.signs[style_index(style)] {
        Some(s) => {
            draw_sprite(fb, atlas, s, &s.anims[0], tick, x, y, false);
            (x - s.origin.x as i32, y - s.origin.y as i32)
        }
        None => {
            let (bx, by) = (x - BOARD_W / 2, y - POST_H - BOARD_H);
            fb.fill_rect(x - 1, y - POST_H, 2, POST_H, OUTLINE);
            let board = if style == SignStyle::Ortsende {
                WHITE
            } else {
                CREAM
            };
            fb.fill_rect(bx, by, BOARD_W, BOARD_H, board);
            fb.outline_rect(bx, by, BOARD_W, BOARD_H, OUTLINE);
            (bx, by)
        }
    };
    if let Some(key) = &spawn.text {
        let text = texts.get(key);
        let mut widths = [0; MAX_LINES];
        let mut n = 0;
        for line in text.split('\n').take(MAX_LINES) {
            widths[n] = font.measure(line);
            n += 1;
        }
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
