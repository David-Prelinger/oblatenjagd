//! Full screens and text overlays around play (`docs/game-design.md` →
//! "Screens and HUD"): title, stage intro, pause, time up, stage clear,
//! „Fortsetzung folgt …" and game over. Every text comes from
//! `content/text/de.toml` by key; [`TEXT_KEYS`] lists every key the engine
//! itself uses (a test checks them against de.toml).
//!
//! Text is drawn in `w` with the `k` shadow unless noted (the title prompt
//! has the full `k` outline); lines are centred
//! horizontally. Blinking texts are on for [`BLINK_FRAMES`], then off as long.

use jnr_core::{ClearPhase, DeathCause, Game, Mode, Popup};

use crate::Framebuffer;
use crate::blit::draw_sprite;
use crate::palette::{DIRNDL_RED, MOZART_YELLOW, NAVY, OUTLINE, SILVER_LIGHT, SKY_BLUE, WHITE};
use crate::scene::{self, PlayerAnim, RenderContext};
use crate::{WIDTH, hud};

/// Blinking texts: this many frames on, then as many off.
pub const BLINK_FRAMES: u32 = 30;

/// `title.name`: the fallback when there is no `title_logo` sprite.
pub const TITLE_NAME_KEY: &str = "title.name";
/// `title.subtitle`.
pub const SUBTITLE_KEY: &str = "title.subtitle";
/// `title.press_start`.
pub const PRESS_START_KEY: &str = "title.press_start";
/// `title.cleared` („Durchgespielt! Mahlzeit!", the marked title, D-107).
pub const CLEARED_KEY: &str = "title.cleared";
/// `title.press_start_again` (the marked title's prompt, D-107).
pub const PRESS_START_AGAIN_KEY: &str = "title.press_start_again";
/// `ui.pause`.
pub const PAUSE_KEY: &str = "ui.pause";
/// `ui.game_over`.
pub const GAME_OVER_KEY: &str = "ui.game_over";
/// `ui.continue`.
pub const CONTINUE_KEY: &str = "ui.continue";
/// `ui.time_up`.
pub const TIME_UP_KEY: &str = "ui.time_up";
/// `ui.to_be_continued`.
pub const TO_BE_CONTINUED_KEY: &str = "ui.to_be_continued";
/// `stage.clear`.
pub const STAGE_CLEAR_KEY: &str = "stage.clear";
/// `stage.time_bonus`.
pub const TIME_BONUS_KEY: &str = "stage.time_bonus";
/// The stage whose texts are used when the level has none of its own
/// (`stage.<level id>.title` missing, e.g. the test levels; M1 has one stage).
pub const FALLBACK_STAGE: &str = "salzburg";
/// Per-stage text keys are `stage.<stage>.<suffix>` with these suffixes.
pub const STAGE_SUFFIXES: [&str; 3] = ["title", "tagline", "next"];

/// The sprite of the title logo (≤ 240×48, origin = centre).
pub const TITLE_LOGO_SPRITE: &str = "title_logo";

/// Every text key the engine uses by name (not counting level signs, which
/// `cargo xtask levels lint` checks). Per-stage keys are listed for the
/// fallback stage; the route map's keys are [`crate::map`]'s.
pub const TEXT_KEYS: [&str; 83] = [
    TITLE_NAME_KEY,
    SUBTITLE_KEY,
    PRESS_START_KEY,
    CLEARED_KEY,
    PRESS_START_AGAIN_KEY,
    match Popup::Stollen.text_key() {
        Some(k) => k,
        None => "",
    },
    crate::map::ENDING_TITLE_KEY,
    crate::map::ENDING_ROUTE_KEY,
    crate::map::ENDING_NOTE_KEY,
    crate::ending::SCENE_KEYS[0],
    crate::ending::SCENE_KEYS[1],
    crate::ending::SCENE_KEYS[2],
    crate::ending::BILL_TITLE_KEY,
    crate::ending::BILL_KEYS[0],
    crate::ending::BILL_KEYS[1],
    crate::ending::BILL_KEYS[2],
    crate::ending::BILL_KEYS[3],
    crate::ending::BILL_KEYS[4],
    crate::ending::END_KEYS[0],
    crate::ending::END_KEYS[1],
    crate::ending::END_KEYS[2],
    // The credits pages as shipped (`ending::credits_lines` looks every
    // `credits.pageN.{title,line1..4}` up and skips the missing ones).
    "credits.page1.title",
    "credits.page1.line1",
    "credits.page1.line2",
    "credits.page2.title",
    "credits.page2.line1",
    "credits.page2.line2",
    "credits.page2.line3",
    "credits.page2.line4",
    "credits.page3.title",
    "credits.page3.line1",
    "credits.page3.line2",
    "credits.page3.line3",
    "credits.page3.line4",
    "credits.page4.title",
    "credits.page4.line1",
    "credits.page4.line2",
    "credits.page4.line3",
    "credits.page4.line4",
    "credits.page5.title",
    "credits.page5.line1",
    "credits.page5.line2",
    "credits.page5.line3",
    "credits.page5.line4",
    "credits.page6.line1",
    "credits.page6.line2",
    "credits.page6.line3",
    "credits.page6.line4",
    hud::LABEL_KEYS[0],
    hud::LABEL_KEYS[1],
    hud::LABEL_KEYS[2],
    hud::LABEL_KEYS[3],
    PAUSE_KEY,
    GAME_OVER_KEY,
    CONTINUE_KEY,
    TIME_UP_KEY,
    TO_BE_CONTINUED_KEY,
    STAGE_CLEAR_KEY,
    TIME_BONUS_KEY,
    "stage.salzburg.title",
    "stage.salzburg.tagline",
    "stage.salzburg.next",
    match Popup::ExtraLife.text_key() {
        Some(k) => k,
        None => "",
    },
    match Popup::Checkpoint.text_key() {
        Some(k) => k,
        None => "",
    },
    match Popup::Wurstkranz.text_key() {
        Some(k) => k,
        None => "",
    },
    match Popup::Hackl.text_key() {
        Some(k) => k,
        None => "",
    },
    match Popup::Ballast.text_key() {
        Some(k) => k,
        None => "",
    },
    match Popup::Bravo.text_key() {
        Some(k) => k,
        None => "",
    },
    match Popup::Beleidigt.text_key() {
        Some(k) => k,
        None => "",
    },
    match Popup::Kurruhe.text_key() {
        Some(k) => k,
        None => "",
    },
    match Popup::Abtreten.text_key() {
        Some(k) => k,
        None => "",
    },
    match Popup::KommGleich.text_key() {
        Some(k) => k,
        None => "",
    },
    crate::map::NEXT_STOP_KEY,
    crate::map::CONTINUE_KEY,
    crate::map::STATION_KEYS[0],
    crate::map::STATION_KEYS[1],
    crate::map::STATION_KEYS[2],
    crate::map::STATION_KEYS[3],
    crate::map::STATION_KEYS[4],
    crate::map::STATION_KEYS[5],
    crate::map::STATION_KEYS[6],
    crate::map::STATION_KEYS[7],
    crate::map::STATION_KEYS[8],
];

// Layout (y = top of the text line).
const LOGO_Y: i32 = 24;
const TITLE_NAME_Y: i32 = 20;
/// In the plain sky band under the logo, clear of every station's start
/// tiles (the Brunnwinkl ?-boxes start at y 84; art review of E12).
const SUBTITLE_Y: i32 = 60;
/// Top of the title prompt line (also the band `cargo xtask wasm-smoke` watches).
pub const PRESS_START_Y: i32 = 128;
/// The marked title's line (D-107: y ≈ 100).
pub const CLEARED_Y: i32 = 100;
/// The served plate on the marked title: this far right of Resi's centre,
/// on her ground line.
const TELLER_DX: i32 = 16;
const INTRO_TITLE_Y: i32 = 64;
const INTRO_TAGLINE_Y: i32 = 80;
const INTRO_LIVES_Y: i32 = 112;
const GAME_OVER_Y: i32 = 72;
const CONTINUE_Y: i32 = 96;
const TIME_UP_Y: i32 = 72;
const CLEAR_Y: i32 = 74;
const NEXT_Y: i32 = 84;
const BONUS_Y: i32 = 96;
/// Stage-clear panel (D-037): top edge and height (y 69..=109, clear of
/// the Maibaum crown above y 42 and the Ortsende sign below); the width
/// follows the widest line. Inside the 1 px border: 4 px padding.
const CLEAR_PANEL_Y: i32 = 69;
const CLEAR_PANEL_H: i32 = 41;
const PANEL_PADDING: i32 = 4;
/// The time-bonus number is measured as this many digits (the maximum
/// bonus, 300 s × 50, has 5), so the panel doesn't change width while the
/// tally counts.
const BONUS_DIGITS: &str = "00000";
const CENTRE_Y: i32 = 86;
/// Pause panel `(x, y, w, h)`: x 112..=207, y 78..=101.
pub const PAUSE_PANEL: (i32, i32, i32, i32) = (112, 78, 96, 24);
/// Gap between the label and the number of the time-bonus line, and
/// between Resi and „× N" on the intro screen.
const GAP: i32 = 4;

/// Whether a blinking text is visible `frames` frames after it started.
pub fn blink_on(frames: u32) -> bool {
    (frames / BLINK_FRAMES).is_multiple_of(2)
}

/// Draws `text` centred horizontally with its top at `y`.
pub(crate) fn centred(ctx: &RenderContext, fb: &mut Framebuffer, text: &str, y: i32, colour: u32) {
    let x = (WIDTH as i32 - ctx.font().measure(text)) / 2;
    ctx.font()
        .draw_text(fb, ctx.atlas(), text, x, y, colour, Some(OUTLINE));
}

/// Title screen: the level's start view (static background, tiles, Resi
/// idle at the start), the logo (or `title.name`), subtitle and the
/// blinking START prompt.
pub fn draw_title(game: &Game, ctx: &RenderContext, fb: &mut Framebuffer, frames: u16) {
    scene::draw_backdrop(game, ctx, fb);
    match ctx.title_logo {
        Some(logo) => draw_sprite(
            fb,
            ctx.atlas(),
            logo,
            &logo.anims[0],
            0,
            WIDTH as i32 / 2,
            LOGO_Y,
            false,
        ),
        None => centred(
            ctx,
            fb,
            &ctx.texts().get(TITLE_NAME_KEY),
            TITLE_NAME_Y,
            DIRNDL_RED,
        ),
    }
    centred(ctx, fb, &ctx.texts().get(SUBTITLE_KEY), SUBTITLE_Y, WHITE);
    // The session's „durchgespielt" mark (D-107): the plate at Resi's
    // feet, the line at y 100 and the other prompt.
    let prompt_key = if game.cleared() {
        let p = game.player();
        let cam = game.camera();
        let feet_x = (p.x + p.w.div_int(2)).to_px() - cam.x;
        let feet_y = (p.y + p.h).to_px() - cam.y;
        scene::draw_teller(ctx, fb, (feet_x + TELLER_DX, feet_y), 0);
        let text = ctx.texts().get(CLEARED_KEY);
        let x = (WIDTH as i32 - ctx.font().measure(&text)) / 2;
        ctx.font()
            .draw_text_outlined(fb, ctx.atlas(), &text, x, CLEARED_Y, MOZART_YELLOW, OUTLINE);
        PRESS_START_AGAIN_KEY
    } else {
        PRESS_START_KEY
    };
    if blink_on(u32::from(frames)) {
        // Full `k` outline, not the drop shadow: the prompt crosses the
        // cream Dom façade of the Salzburg rooftops (D-035).
        let prompt = ctx.texts().get(prompt_key);
        let x = (WIDTH as i32 - ctx.font().measure(&prompt)) / 2;
        ctx.font()
            .draw_text_outlined(fb, ctx.atlas(), &prompt, x, PRESS_START_Y, WHITE, OUTLINE);
    }
}

/// Stage intro: navy, stage title, tagline in `i`, Resi idle and „× N".
pub fn draw_intro(game: &Game, ctx: &RenderContext, fb: &mut Framebuffer) {
    fb.clear(NAVY);
    let texts = ctx.texts();
    centred(
        ctx,
        fb,
        &texts.get(&ctx.stage_keys[0]),
        INTRO_TITLE_Y,
        WHITE,
    );
    let tagline = texts.get(&ctx.stage_keys[1]);
    centred(ctx, fb, &tagline, INTRO_TAGLINE_Y, SILVER_LIGHT);

    let mut buf = [0u8; 10];
    let lives = hud::padded(u32::from(game.lives()), 1, &mut buf);
    let (font, atlas) = (ctx.font(), ctx.atlas());
    let times_w = font.measure("×");
    let total = 16 + GAP + times_w + GAP + font.measure(lives);
    let left = (WIDTH as i32 - total) / 2;
    // The power state carried into the stage: `player_big` or
    // `player_hackl` (D-042, D-056).
    let feet = (left + 8, INTRO_LIVES_Y + 12);
    scene::draw_player_pose(
        ctx,
        fb,
        game.player().power,
        &game.tuning().player,
        PlayerAnim::Idle,
        feet,
        game.frame(),
        false,
    );
    let x = left + 16 + GAP;
    // The „×“ in `Y` like the number (art-director: a white × vanishes in haze).
    font.draw_text(
        fb,
        atlas,
        "×",
        x,
        INTRO_LIVES_Y,
        MOZART_YELLOW,
        Some(OUTLINE),
    );
    let x = x + times_w + GAP;
    font.draw_text(
        fb,
        atlas,
        lives,
        x,
        INTRO_LIVES_Y,
        MOZART_YELLOW,
        Some(OUTLINE),
    );
}

/// Game over: navy, `ui.game_over`; after `game.game_over_frames` the
/// blinking `ui.continue`.
pub fn draw_game_over(game: &Game, ctx: &RenderContext, fb: &mut Framebuffer, frames: u16) {
    fb.clear(NAVY);
    centred(ctx, fb, &ctx.texts().get(GAME_OVER_KEY), GAME_OVER_Y, WHITE);
    let wait = game.tuning().game.game_over_frames;
    if frames >= wait && blink_on(u32::from(frames - wait)) {
        centred(ctx, fb, &ctx.texts().get(CONTINUE_KEY), CONTINUE_Y, WHITE);
    }
}

/// „Fortsetzung folgt …" on navy.
pub fn draw_to_be_continued(ctx: &RenderContext, fb: &mut Framebuffer) {
    fb.clear(NAVY);
    let text = ctx.texts().get(TO_BE_CONTINUED_KEY);
    centred(ctx, fb, &text, CENTRE_Y, WHITE);
}

/// Pause: the frozen level stays at full colour; a `B` panel with a `k`
/// border and an `S` highlight (top and left) carries `ui.pause`.
pub fn draw_pause(ctx: &RenderContext, fb: &mut Framebuffer) {
    draw_panel(fb, PAUSE_PANEL);
    centred(ctx, fb, &ctx.texts().get(PAUSE_KEY), CENTRE_Y, WHITE);
}

/// An art-style §11 panel `(x, y, w, h)`: `B` fill, 1 px `k` border, 1 px
/// `S` highlight inside the border on the top and left edges.
pub(crate) fn draw_panel(fb: &mut Framebuffer, (x, y, w, h): (i32, i32, i32, i32)) {
    fb.fill_rect(x, y, w, h, NAVY);
    fb.fill_rect(x + 1, y + 1, w - 2, 1, SKY_BLUE);
    fb.fill_rect(x + 1, y + 1, 1, h - 2, SKY_BLUE);
    fb.outline_rect(x, y, w, h, OUTLINE);
}

/// The stage-clear panel `(x, y, w, h)`: centred, as wide as the widest of
/// its three lines (+1 px shadow) plus padding and border on both sides.
pub fn clear_panel(ctx: &RenderContext) -> (i32, i32, i32, i32) {
    let (font, texts) = (ctx.font(), ctx.texts());
    let bonus = font.measure(&texts.get(TIME_BONUS_KEY)) + GAP + font.measure(BONUS_DIGITS);
    let widest = font
        .measure(&texts.get(STAGE_CLEAR_KEY))
        .max(font.measure(&texts.get(&ctx.stage_keys[2])))
        .max(bonus);
    // Rounded up to even, so the panel is exactly centred.
    let w = (widest + 1 + 2 * (1 + PANEL_PADDING) + 1) / 2 * 2;
    ((WIDTH as i32 - w) / 2, CLEAR_PANEL_Y, w, CLEAR_PANEL_H)
}

/// Overlays drawn over the level for the current mode: „Zeit is' um!"
/// while dying of time-up, the stage-clear texts from the tally on.
pub fn draw_overlays(game: &Game, ctx: &RenderContext, fb: &mut Framebuffer) {
    match game.mode() {
        Mode::Dying { .. } if game.death_cause() == Some(DeathCause::TimeUp) => {
            centred(ctx, fb, &ctx.texts().get(TIME_UP_KEY), TIME_UP_Y, WHITE);
        }
        Mode::StageClear => {
            let Some(c) = game.stage_clear() else { return };
            if !matches!(c.phase, ClearPhase::Tally | ClearPhase::Clear) {
                return;
            }
            draw_panel(fb, clear_panel(ctx));
            let texts = ctx.texts();
            centred(ctx, fb, &texts.get(STAGE_CLEAR_KEY), CLEAR_Y, WHITE);
            centred(ctx, fb, &texts.get(&ctx.stage_keys[2]), NEXT_Y, WHITE);
            let (font, atlas) = (ctx.font(), ctx.atlas());
            let label = texts.get(TIME_BONUS_KEY);
            let mut buf = [0u8; 10];
            let number = hud::padded(c.bonus, 1, &mut buf);
            let (wl, wn) = (font.measure(&label), font.measure(number));
            let x = (WIDTH as i32 - (wl + GAP + wn)) / 2;
            font.draw_text(fb, atlas, &label, x, BONUS_Y, WHITE, Some(OUTLINE));
            let x = x + wl + GAP;
            font.draw_text(fb, atlas, number, x, BONUS_Y, MOZART_YELLOW, Some(OUTLINE));
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::font::{CHARSET, FONT_SPRITE, GLYPH_ANIM};
    use crate::scene::render;
    use crate::testutil::{self, image};
    use jnr_core::{InputFrame, Level, Start, Texts, Tuning};

    fn texts() -> Texts {
        Texts::parse(include_str!("../../../content/text/de.toml")).unwrap()
    }

    #[test]
    fn every_engine_text_key_exists_in_de_toml() {
        let t = texts();
        for key in TEXT_KEYS {
            assert!(
                t.contains(key),
                "missing text key `{key}` in content/text/de.toml"
            );
        }
        for suffix in STAGE_SUFFIXES {
            let key = format!("stage.{FALLBACK_STAGE}.{suffix}");
            assert!(t.contains(&key), "{key}");
        }
    }

    #[test]
    fn engine_texts_fit_the_ui_limits() {
        // UI lines <= 24 characters, only font characters (D-012).
        let t = texts();
        for key in TEXT_KEYS {
            let text = t.get(key);
            assert!(text.chars().count() <= 24, "{key}: `{text}` is too long");
            for ch in text.chars() {
                assert!(CHARSET.contains(ch), "{key}: `{ch}` is not in the font");
            }
        }
    }

    #[test]
    fn blinks_30_on_30_off() {
        assert!(blink_on(0) && blink_on(29));
        assert!(!blink_on(30) && !blink_on(59));
        assert!(blink_on(60));
    }

    /// Every glyph a full 8x8 block (advance 9, space 3).
    fn block_font() -> jnr_artkit::Atlas {
        let glyphs = CHARSET.chars().map(|_| image(&["aaaaaaaa"; 8])).collect();
        testutil::atlas(&[testutil::sprite(
            FONT_SPRITE,
            (0, 0),
            false,
            vec![(GLYPH_ANIM, 0, glyphs)],
        )])
    }

    /// Start at column 2, a spike block at column 8 (at the player's
    /// height), the goal at column 18.
    const LEVEL: &str = "[meta]\nid = \"t\"\ntime_limit = 300\n[legend]\n\"#\" = \"solid\"\n\
                         \"^\" = \"hazard\"\n\"P\" = { entity = \"player_start\" }\n\
                         \"G\" = { entity = \"goal\" }\n[map]\ngrid = '''\n\
                         ....................\n..P.....^.........G.\n####################\n'''\n";

    fn level() -> Level {
        Level::parse(LEVEL).unwrap()
    }

    fn tuning() -> Tuning {
        Tuning::parse(include_str!("../../../content/tuning.toml")).unwrap()
    }

    fn rendered(game: &Game, atlas: &jnr_artkit::Atlas) -> Framebuffer {
        let texts = texts();
        let ctx = RenderContext::new(atlas, &texts, game.level());
        let mut fb = testutil::fb();
        render(game, &ctx, &mut fb);
        fb
    }

    /// Left edge of `text` centred with the block font.
    fn centred_x(text: &str) -> i32 {
        let font = crate::Font::from_atlas(&block_font());
        (320 - font.measure(text)) / 2
    }

    #[test]
    fn title_shows_texts_and_blinks_the_prompt() {
        let atlas = block_font();
        let mut g = Game::new_at(&level(), &tuning(), 1, Start::Title);
        let fb = rendered(&g, &atlas);
        let t = texts();
        // No logo sprite: `title.name` in red at y 20.
        let name = t.get(TITLE_NAME_KEY);
        assert_eq!(fb.get(centred_x(&name), TITLE_NAME_Y), Some(DIRNDL_RED));
        let sub = t.get(SUBTITLE_KEY);
        assert_eq!(fb.get(centred_x(&sub), SUBTITLE_Y), Some(WHITE));
        let prompt = t.get(PRESS_START_KEY);
        let px = centred_x(&prompt);
        assert_eq!(fb.get(px, PRESS_START_Y), Some(WHITE));
        // Full outline (D-035), not the +1/+1 shadow: also left of and
        // above the first glyph.
        assert_eq!(fb.get(px - 1, PRESS_START_Y), Some(OUTLINE));
        assert_eq!(fb.get(px - 1, PRESS_START_Y - 1), Some(OUTLINE));
        assert_eq!(fb.get(px, PRESS_START_Y - 1), Some(OUTLINE));
        // The subtitle keeps the drop shadow.
        assert_ne!(fb.get(centred_x(&sub) - 1, SUBTITLE_Y), Some(OUTLINE));
        for _ in 0..30 {
            g.step(InputFrame::NONE);
        }
        let fb = rendered(&g, &atlas);
        assert_ne!(fb.get(px, PRESS_START_Y), Some(WHITE), "prompt off");
        // No HUD on the title screen.
        assert_ne!(fb.get(16, 4), Some(WHITE));
    }

    /// The subtitle (y 60..=68 with its shadow) sits in plain sky on every
    /// shipped stage's title view: no tile is drawn behind it.
    #[test]
    fn subtitle_band_is_clear_of_tiles_on_every_stage_title() {
        let line = crate::font::LINE_HEIGHT;
        // Every shipped stage: content/levels/0[1-9]_*.level.
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../content/levels");
        let mut stages: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| {
                let n = p.file_name().unwrap().to_string_lossy();
                let b = n.as_bytes();
                b.len() > 3
                    && b[0] == b'0'
                    && (b'1'..=b'9').contains(&b[1])
                    && b[2] == b'_'
                    && n.ends_with(".level")
            })
            .collect();
        stages.sort();
        assert!(stages.len() >= 3, "{stages:?}");
        for path in stages {
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            let level = Level::parse(&std::fs::read_to_string(&path).unwrap()).unwrap();
            let g = Game::new_at(&level, &tuning(), 1, Start::Title);
            let cam = g.camera();
            let tile = jnr_core::level::TILE_PX;
            for y in (cam.y + SUBTITLE_Y - 1)..=(cam.y + SUBTITLE_Y + line) {
                for x in (cam.x + 48)..(cam.x + WIDTH as i32 - 48) {
                    let t = level.tile(x.div_euclid(tile), y.div_euclid(tile));
                    assert!(
                        t.is_none_or(|t| t.name == 0),
                        "{name}: a tile is drawn behind the title subtitle at screen ({}, {})",
                        x - cam.x,
                        y - cam.y
                    );
                }
            }
        }
    }

    #[test]
    fn intro_is_navy_with_title_tagline_and_lives() {
        let atlas = block_font();
        let mut g = Game::new_at(&level(), &tuning(), 1, Start::Title);
        g.step(InputFrame::START);
        assert!(matches!(g.mode(), Mode::StageIntro { .. }));
        let fb = rendered(&g, &atlas);
        assert_eq!(fb.get(0, 0), Some(NAVY));
        let t = texts();
        // Level `t` has no texts of its own: the Salzburg ones.
        let title = t.get("stage.salzburg.title");
        assert_eq!(fb.get(centred_x(&title), INTRO_TITLE_Y), Some(WHITE));
        let tagline = t.get("stage.salzburg.tagline");
        let tx = centred_x(&tagline);
        assert_eq!(fb.get(tx, INTRO_TAGLINE_Y), Some(SILVER_LIGHT));
        // Resi (grey box) + "×" + "3", centred as a group: 16 + 4 + 8 + 4 + 8.
        let left = (320 - (16 + 4 + 8 + 4 + 8)) / 2;
        assert_eq!(fb.get(left + 20, INTRO_LIVES_Y), Some(MOZART_YELLOW), "×");
        assert_eq!(fb.get(left + 32, INTRO_LIVES_Y), Some(MOZART_YELLOW), "3");
        assert_ne!(fb.get(left + 8, INTRO_LIVES_Y + 6), Some(NAVY), "Resi");
        // No HUD on the intro.
        assert_eq!(fb.get(16, 4), Some(NAVY));
    }

    #[test]
    fn pause_shows_a_panel_over_the_full_colour_level() {
        let atlas = block_font();
        let mut g = Game::new_at(&level(), &tuning(), 1, Start::Title);
        g.step(InputFrame::START);
        for _ in 0..tuning().game.intro_frames {
            g.step(InputFrame::NONE);
        }
        let playing = rendered(&g, &atlas);
        g.step(InputFrame::START);
        assert_eq!(g.mode(), Mode::Paused);
        let fb = rendered(&g, &atlas);
        // Outside the panel the level is unchanged (no dimming).
        assert_eq!(fb.get(5, 170), playing.get(5, 170));
        assert_eq!(fb.get(111, 80), playing.get(111, 80));
        assert_eq!(fb.get(208, 80), playing.get(208, 80));
        // Panel: k border, S highlight top and left inside it, B fill.
        assert_eq!(fb.get(112, 78), Some(OUTLINE));
        assert_eq!(fb.get(207, 101), Some(OUTLINE));
        assert_eq!(fb.get(150, 79), Some(SKY_BLUE));
        assert_eq!(fb.get(113, 95), Some(SKY_BLUE));
        assert_eq!(fb.get(206, 100), Some(NAVY));
        assert_eq!(fb.get(150, 82), Some(NAVY));
        let pause = texts().get(PAUSE_KEY).into_owned();
        let x = centred_x(&pause);
        assert_eq!(fb.get(x, CENTRE_Y), Some(WHITE));
        assert_eq!(fb.get(x + 1, CENTRE_Y + 8), Some(OUTLINE), "shadow");
    }

    #[test]
    fn game_over_prompt_appears_after_60_frames() {
        let atlas = block_font();
        let mut g = Game::new(&level(), &tuning(), 1);
        while !matches!(g.mode(), Mode::GameOver { .. }) {
            g.step(InputFrame::RIGHT);
        }
        let fb = rendered(&g, &atlas);
        assert_eq!(fb.get(0, 0), Some(NAVY));
        let go = texts().get(GAME_OVER_KEY).into_owned();
        assert_eq!(fb.get(centred_x(&go), GAME_OVER_Y), Some(WHITE));
        let cont = texts().get(CONTINUE_KEY).into_owned();
        let cx = centred_x(&cont);
        assert_eq!(fb.get(cx, CONTINUE_Y), Some(NAVY));
        for _ in 0..60 {
            g.step(InputFrame::NONE);
        }
        assert_eq!(rendered(&g, &atlas).get(cx, CONTINUE_Y), Some(WHITE));
    }

    #[test]
    fn time_up_text_shows_while_dying() {
        let atlas = block_font();
        let lvl = Level::parse(
            "[meta]\nid = \"t\"\ntime_limit = 1\n[legend]\n\"#\" = \"solid\"\n\
             \"P\" = { entity = \"player_start\" }\n[map]\ngrid = '''\n.P..\n####\n'''\n",
        )
        .unwrap();
        let mut g = Game::new(&lvl, &tuning(), 1);
        for _ in 0..60 {
            g.step(InputFrame::NONE);
        }
        assert!(matches!(g.mode(), Mode::Dying { .. }));
        let fb = rendered(&g, &atlas);
        let text = texts().get(TIME_UP_KEY).into_owned();
        assert_eq!(fb.get(centred_x(&text), TIME_UP_Y), Some(WHITE));
        // No stage-clear panel while dying.
        assert_ne!(fb.get(150, CLEAR_PANEL_Y), Some(OUTLINE));
    }

    #[test]
    fn clear_panel_fits_the_real_texts_between_crown_and_sign() {
        let texts = texts();
        let lvl = level();
        let ctx = RenderContext::new(jnr_assets::atlas(), &texts, &lvl);
        let (x, y, w, h) = clear_panel(&ctx);
        // About x 106..=214, y 69..=109 (art-director ruling, D-037).
        assert!((100..=110).contains(&x), "left edge {x}, width {w}");
        assert_eq!(x * 2 + w, 320, "centred");
        assert_eq!((y, y + h - 1), (69, 109));
        // 4 px padding inside the 1 px border on top and bottom: the first
        // line starts at y + 5, the bonus line's shadow ends at y + h - 6.
        assert_eq!(CLEAR_Y, y + 1 + PANEL_PADDING);
        assert_eq!(BONUS_Y + 8, y + h - 1 - PANEL_PADDING - 1);
    }

    #[test]
    fn stage_clear_texts_and_time_bonus_then_to_be_continued() {
        let atlas = block_font();
        let no_spikes = Level::parse(&LEVEL.replace("..P.....^", "..P......")).unwrap();
        let mut g = Game::new(&no_spikes, &tuning(), 1);
        while g.mode() != Mode::StageClear {
            g.step(InputFrame::RIGHT);
            assert!(g.frame() < 600);
        }
        let fb = rendered(&g, &atlas);
        let clear = texts().get(STAGE_CLEAR_KEY).into_owned();
        let x = centred_x(&clear);
        assert_ne!(fb.get(x, CLEAR_Y), Some(WHITE), "not while sliding");
        assert_ne!(fb.get(160, CLEAR_PANEL_Y), Some(OUTLINE), "no panel yet");
        while g.stage_clear().unwrap().phase != ClearPhase::Tally {
            g.step(InputFrame::NONE);
        }
        g.step(InputFrame::NONE);
        let fb = rendered(&g, &atlas);
        assert_eq!(fb.get(x, CLEAR_Y), Some(WHITE));
        let next = texts().get("stage.salzburg.next").into_owned();
        assert_eq!(fb.get(centred_x(&next), NEXT_Y), Some(WHITE));
        // "Zeitbonus" (9 glyphs: 80 px) + 4 + "50" (17 px) centred.
        assert_eq!(g.stage_clear().unwrap().bonus, 50);
        let x = (320 - (80 + 4 + 17)) / 2;
        assert_eq!(fb.get(x, BONUS_Y), Some(WHITE));
        assert_eq!(fb.get(x + 84, BONUS_Y), Some(MOZART_YELLOW));
        assert_eq!(fb.get(x + 1, BONUS_Y + 8), Some(OUTLINE), "label shadow");
        // The panel: k border, S highlight top and left, B fill behind
        // the texts; the level is unchanged outside it.
        let ctx_texts = texts();
        let ctx = RenderContext::new(&atlas, &ctx_texts, g.level());
        let (px, py, pw, ph) = clear_panel(&ctx);
        assert_eq!(fb.get(px, py), Some(OUTLINE));
        assert_eq!(fb.get(px + pw - 1, py + ph - 1), Some(OUTLINE));
        assert_eq!(fb.get(px + 10, py + 1), Some(SKY_BLUE), "top highlight");
        assert_eq!(fb.get(px + 1, py + 20), Some(SKY_BLUE), "left highlight");
        assert_eq!(fb.get(px + pw - 2, py + ph - 2), Some(NAVY));
        assert_eq!(fb.get(px + 2, py + 2), Some(NAVY));
        assert_ne!(fb.get(px - 1, py + 20), Some(OUTLINE));
        assert_ne!(fb.get(px - 1, py + 20), Some(NAVY));
        // HUD still shown.
        assert_eq!(fb.get(16, 4), Some(WHITE));
        while g.stage_clear().unwrap().phase != ClearPhase::ToBeContinued {
            g.step(InputFrame::NONE);
        }
        let fb = rendered(&g, &atlas);
        assert_eq!(fb.get(0, 0), Some(NAVY));
        let tbc = texts().get(TO_BE_CONTINUED_KEY).into_owned();
        assert_eq!(fb.get(centred_x(&tbc), CENTRE_Y), Some(WHITE));
    }
}
