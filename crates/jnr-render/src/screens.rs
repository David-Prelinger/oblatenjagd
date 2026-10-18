//! Full screens and text overlays around play (`docs/game-design.md` →
//! "Screens and HUD"): title, stage intro, pause, time up, stage clear,
//! „Fortsetzung folgt …" and game over. Every text comes from
//! `content/text/de.toml` by key; [`TEXT_KEYS`] lists every key the engine
//! itself uses (a test checks them against de.toml).
//!
//! Text is drawn in `w` with the `k` shadow unless noted; lines are centred
//! horizontally. Blinking texts are on for [`BLINK_FRAMES`], then off as long.

use jnr_core::{ClearPhase, DeathCause, Game, Mode, Popup};

use crate::Framebuffer;
use crate::blit::draw_sprite;
use crate::palette::{DIRNDL_RED, MOZART_YELLOW, NAVY, OUTLINE, SILVER_LIGHT, WHITE};
use crate::scene::{self, RenderContext};
use crate::{WIDTH, hud};

/// Blinking texts: this many frames on, then as many off.
pub const BLINK_FRAMES: u32 = 30;

/// `title.name`: the fallback when there is no `title_logo` sprite.
pub const TITLE_NAME_KEY: &str = "title.name";
/// `title.subtitle`.
pub const SUBTITLE_KEY: &str = "title.subtitle";
/// `title.press_start`.
pub const PRESS_START_KEY: &str = "title.press_start";
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
/// fallback stage.
pub const TEXT_KEYS: [&str; 20] = [
    TITLE_NAME_KEY,
    SUBTITLE_KEY,
    PRESS_START_KEY,
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
];

// Layout (y = top of the text line).
const LOGO_Y: i32 = 24;
const TITLE_NAME_Y: i32 = 20;
const SUBTITLE_Y: i32 = 84;
const PRESS_START_Y: i32 = 140;
const INTRO_TITLE_Y: i32 = 64;
const INTRO_TAGLINE_Y: i32 = 80;
const INTRO_LIVES_Y: i32 = 112;
const GAME_OVER_Y: i32 = 72;
const CONTINUE_Y: i32 = 96;
const CLEAR_Y: i32 = 72;
const NEXT_Y: i32 = 84;
const BONUS_Y: i32 = 100;
const CENTRE_Y: i32 = 86;
/// Gap between the label and the number of the time-bonus line, and
/// between Resi and „× N" on the intro screen.
const GAP: i32 = 4;

/// Whether a blinking text is visible `frames` frames after it started.
pub fn blink_on(frames: u32) -> bool {
    (frames / BLINK_FRAMES).is_multiple_of(2)
}

/// Draws `text` centred horizontally with its top at `y`.
fn centred(ctx: &RenderContext, fb: &mut Framebuffer, text: &str, y: i32, colour: u32) {
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
            game.frame(),
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
    if blink_on(u32::from(frames)) {
        let prompt = ctx.texts().get(PRESS_START_KEY);
        centred(ctx, fb, &prompt, PRESS_START_Y, WHITE);
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
    scene::draw_player_idle(ctx, fb, left + 8, INTRO_LIVES_Y + 12, game.frame());
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

/// Pause: the frame drawn so far dimmed (every channel halved), then `ui.pause`.
pub fn draw_pause(ctx: &RenderContext, fb: &mut Framebuffer) {
    for p in &mut fb.pixels {
        *p = ((*p >> 1) & 0x7f7f_7f00) | (*p & 0xff);
    }
    centred(ctx, fb, &ctx.texts().get(PAUSE_KEY), CENTRE_Y, WHITE);
}

/// Overlays drawn over the level for the current mode: „Zeit is' um!"
/// while dying of time-up, the stage-clear texts from the tally on.
pub fn draw_overlays(game: &Game, ctx: &RenderContext, fb: &mut Framebuffer) {
    match game.mode() {
        Mode::Dying { .. } if game.death_cause() == Some(DeathCause::TimeUp) => {
            centred(ctx, fb, &ctx.texts().get(TIME_UP_KEY), CLEAR_Y, WHITE);
        }
        Mode::StageClear => {
            let Some(c) = game.stage_clear() else { return };
            if !matches!(c.phase, ClearPhase::Tally | ClearPhase::Clear) {
                return;
            }
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
        for _ in 0..30 {
            g.step(InputFrame::NONE);
        }
        let fb = rendered(&g, &atlas);
        assert_ne!(fb.get(px, PRESS_START_Y), Some(WHITE), "prompt off");
        // No HUD on the title screen.
        assert_ne!(fb.get(16, 4), Some(WHITE));
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
    fn pause_dims_the_level_and_shows_its_text() {
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
        let sky = playing.get(5, 170).unwrap();
        let dim = ((sky >> 1) & 0x7f7f_7f00) | 0xff;
        assert_eq!(fb.get(5, 170), Some(dim));
        let pause = texts().get(PAUSE_KEY).into_owned();
        assert_eq!(fb.get(centred_x(&pause), CENTRE_Y), Some(WHITE));
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
        assert_eq!(fb.get(centred_x(&text), CLEAR_Y), Some(WHITE));
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
