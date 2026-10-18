//! The HUD (`docs/game-design.md` → "Screens and HUD"): two rows at the
//! top, labels at y = 4 in `w`, values at y = 13 in `Y`, all with the
//! font's `k` shadow and no panel. Columns (left-aligned): PUNKTE x = 16
//! (6 digits, zero-padded), OBLATEN x = 96 (2 digits, after the optional
//! `hud_oblate` icon), LEBEN x = 184, ZEIT x = 256 (3 digits, `r` at ≤ 60 s).
//!
//! Shown in `Playing`, `Paused` (dimmed with the level), `Dying` and
//! `StageClear` (scene decides). Drawing allocates nothing.

use jnr_artkit::atlas::Atlas;
use jnr_core::Game;

use crate::Framebuffer;
use crate::blit::draw_sprite;
use crate::palette::{DIRNDL_RED, MOZART_YELLOW, OUTLINE, WHITE};
use crate::scene::RenderContext;

/// Label text keys, in column order.
pub const LABEL_KEYS: [&str; 4] = ["hud.score", "hud.oblaten", "hud.lives", "hud.time"];
/// Left edge of each column, in the order of [`LABEL_KEYS`].
pub const COLUMNS: [i32; 4] = [16, 96, 184, 256];
/// Top of the label row.
pub const LABEL_Y: i32 = 4;
/// Top of the value row.
pub const VALUE_Y: i32 = 13;
/// The small Oblate icon before the OBLATEN counter (optional sprite).
pub const OBLATE_ICON_SPRITE: &str = "hud_oblate";
/// Icon width plus gap: the Oblaten digits start this far right of the column.
const ICON_ADVANCE: i32 = 10;
/// ZEIT turns red at or below this many seconds.
pub const TIME_WARNING: u16 = 60;

/// `n` as decimal digits, zero-padded to at least `width` (≤ 10) digits,
/// written into `buf` (no allocation).
pub fn padded(mut n: u32, width: usize, buf: &mut [u8; 10]) -> &str {
    let width = width.min(buf.len());
    let mut i = buf.len();
    loop {
        i -= 1;
        buf[i] = b'0' + (n % 10) as u8;
        n /= 10;
        if n == 0 && buf.len() - i >= width {
            break;
        }
    }
    std::str::from_utf8(&buf[i..]).unwrap_or("?")
}

/// Draws the HUD over whatever is in the framebuffer.
pub fn draw_hud(game: &Game, ctx: &RenderContext, fb: &mut Framebuffer) {
    let (atlas, font, texts) = (ctx.atlas(), ctx.font(), ctx.texts());
    for (key, x) in LABEL_KEYS.iter().zip(COLUMNS) {
        font.draw_text(fb, atlas, &texts.get(key), x, LABEL_Y, WHITE, Some(OUTLINE));
    }
    let mut buf = [0u8; 10];
    let value = |fb: &mut Framebuffer, text: &str, x: i32, colour: u32| {
        font.draw_text(fb, atlas, text, x, VALUE_Y, colour, Some(OUTLINE));
    };
    value(
        fb,
        padded(game.score(), 6, &mut buf),
        COLUMNS[0],
        MOZART_YELLOW,
    );
    let oblaten_x = match ctx.hud_oblate {
        Some(icon) => {
            draw_icon(fb, atlas, icon, game.frame(), COLUMNS[1]);
            COLUMNS[1] + ICON_ADVANCE
        }
        None => COLUMNS[1],
    };
    let oblaten = u32::from(game.oblaten());
    value(fb, padded(oblaten, 2, &mut buf), oblaten_x, MOZART_YELLOW);
    let lives = u32::from(game.lives());
    value(fb, padded(lives, 1, &mut buf), COLUMNS[2], MOZART_YELLOW);
    let time = game.time_left();
    let colour = if time <= TIME_WARNING {
        DIRNDL_RED
    } else {
        MOZART_YELLOW
    };
    value(fb, padded(u32::from(time), 3, &mut buf), COLUMNS[3], colour);
}

/// The icon with its top-left corner at (`x`, [`VALUE_Y`]), whatever its origin.
fn draw_icon(
    fb: &mut Framebuffer,
    atlas: &Atlas,
    icon: &jnr_artkit::atlas::AtlasSprite,
    tick: u32,
    x: i32,
) {
    let (ox, oy) = (icon.origin.x as i32, icon.origin.y as i32);
    draw_sprite(
        fb,
        atlas,
        icon,
        &icon.anims[0],
        tick,
        x + ox,
        VALUE_Y + oy,
        false,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::font::{CHARSET, FONT_SPRITE, GLYPH_ANIM};
    use crate::testutil::{self, image};
    use jnr_core::{InputFrame, Level, Texts, Tuning};

    #[test]
    fn pads_numbers() {
        let mut b = [0u8; 10];
        assert_eq!(padded(0, 6, &mut b), "000000");
        assert_eq!(padded(4250, 6, &mut b), "004250");
        assert_eq!(padded(999_999, 6, &mut b), "999999");
        assert_eq!(padded(7, 1, &mut b), "7");
        assert_eq!(padded(12, 1, &mut b), "12");
        assert_eq!(padded(5, 3, &mut b), "005");
        assert_eq!(padded(u32::MAX, 20, &mut b), "4294967295");
    }

    /// A font whose digit `d` is a 1 px wide bar of height `d + 1` (digits up to 7
    /// can be read from the column height; 8 and 9 fill the column) and every
    /// other glyph a 1x1 dot at the top: advance 2 for all of them.
    fn digit_atlas() -> jnr_artkit::Atlas {
        digit_atlas_with(Vec::new())
    }

    /// [`digit_atlas`] plus more sprites.
    fn digit_atlas_with(mut sprites: Vec<jnr_artkit::CompiledSprite>) -> jnr_artkit::Atlas {
        let glyphs = CHARSET
            .chars()
            .map(|ch| {
                let rows: Vec<String> = (0..8)
                    .map(|r| {
                        let on = match ch.to_digit(10) {
                            Some(d) => r >= 7 - d as i32,
                            None => r == 0,
                        };
                        if on { "a......." } else { "........" }.to_owned()
                    })
                    .collect();
                let refs: Vec<&str> = rows.iter().map(String::as_str).collect();
                image(&refs)
            })
            .collect();
        sprites.push(testutil::sprite(
            FONT_SPRITE,
            (0, 0),
            false,
            vec![(GLYPH_ANIM, 0, glyphs)],
        ));
        testutil::atlas(&sprites)
    }

    /// Reads the digits drawn in `colour` from x onwards on the value row
    /// (one glyph every 2 px).
    fn read_digits(fb: &Framebuffer, x: i32, n: usize, colour: u32) -> String {
        (0..n as i32)
            .map(|i| {
                let gx = x + i * 2;
                let height = (0..8)
                    .filter(|r| fb.get(gx, VALUE_Y + r) == Some(colour))
                    .count();
                char::from_digit(height as u32 - 1, 10).unwrap()
            })
            .collect()
    }

    fn game(time_limit: u32) -> Game {
        let text = format!(
            "[meta]\nid = \"t\"\ntime_limit = {time_limit}\n[legend]\n\"#\" = \"solid\"\n\
             \"P\" = {{ entity = \"player_start\" }}\n[map]\ngrid = '''\n.P..\n####\n'''\n"
        );
        let tuning = Tuning::parse(include_str!("../../../content/tuning.toml")).unwrap();
        Game::new(&Level::parse(&text).unwrap(), &tuning, 1)
    }

    #[test]
    fn hud_shows_score_oblaten_lives_and_time() {
        let atlas = digit_atlas();
        let texts = Texts::parse(include_str!("../../../content/text/de.toml")).unwrap();
        let g = game(305);
        let ctx = RenderContext::new(&atlas, &texts, g.level());
        let mut fb = testutil::fb();
        draw_hud(&g, &ctx, &mut fb);
        assert_eq!(read_digits(&fb, 16, 6, MOZART_YELLOW), "000000");
        assert_eq!(read_digits(&fb, 96, 2, MOZART_YELLOW), "00");
        assert_eq!(read_digits(&fb, 184, 1, MOZART_YELLOW), "3");
        assert_eq!(read_digits(&fb, 256, 3, MOZART_YELLOW), "305");
        // Labels: white dots at y = 4 in every column, with a k shadow.
        for x in COLUMNS {
            assert_eq!(fb.get(x, LABEL_Y), Some(WHITE), "label at {x}");
            assert_eq!(fb.get(x + 1, LABEL_Y + 1), Some(OUTLINE), "shadow at {x}");
        }
        // Value shadow at +1,+1 below the last row of the 3 (rows 4..=7).
        assert_eq!(fb.get(185, VALUE_Y + 8), Some(OUTLINE));
    }

    #[test]
    fn time_turns_red_at_60_seconds() {
        let atlas = digit_atlas();
        let texts = Texts::default();
        let mut g = game(61);
        let ctx = RenderContext::new(&atlas, &texts, g.level());
        let mut fb = testutil::fb();
        draw_hud(&g, &ctx, &mut fb);
        assert_eq!(read_digits(&fb, 256, 3, MOZART_YELLOW), "061");
        for _ in 0..60 {
            g.step(InputFrame::NONE);
        }
        assert_eq!(g.time_left(), 60);
        let mut fb = testutil::fb();
        draw_hud(&g, &ctx, &mut fb);
        assert_eq!(read_digits(&fb, 256, 3, DIRNDL_RED), "060");
    }

    #[test]
    fn oblaten_digits_move_right_of_the_icon() {
        // An 8x8 icon with its origin at the bottom centre.
        let atlas = digit_atlas_with(vec![testutil::sprite(
            OBLATE_ICON_SPRITE,
            (4, 8),
            false,
            vec![("idle", 0, vec![image(&["bbbbbbbb"; 8])])],
        )]);
        let texts = Texts::default();
        let g = game(100);
        let ctx = RenderContext::new(&atlas, &texts, g.level());
        let mut fb = testutil::fb();
        draw_hud(&g, &ctx, &mut fb);
        assert_eq!(fb.get(96, VALUE_Y), Some(testutil::B), "icon top-left");
        assert_eq!(fb.get(103, VALUE_Y + 7), Some(testutil::B));
        assert_eq!(read_digits(&fb, 106, 2, MOZART_YELLOW), "00");
    }
}
