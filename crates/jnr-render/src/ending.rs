//! The ending after the last station (`docs/game-design.md` → "Finale and
//! ending", D-107), drawn from `Game::ending` alone (phase, frame, page)
//! and the run's totals; full screen, no HUD:
//!
//! - **Map**: [`crate::map::draw_ending_map`] (every station done, the
//!   whole route walked, Resi on the last dot, the three panel lines),
//! - **Scene**: navy `B`, `ending_cafe` at (16, 12) (missing: text only),
//!   three lines at y 140 / 150 / 160,
//! - **Bill**: `ending_zettel` at (72, 18) (missing: a D-037 panel of the
//!   same size) with the title, then one line per `ending.bill_line_frames`:
//!   Punkte, Oblaten gesammelt (the run's total), Leben übrig (label left,
//!   number right in `Y`), the tip and the thanks,
//! - **Credits**: page N from `credits.pageN.{title,line1..4}`, missing
//!   keys skipped, centred,
//! - **Ende**: „Ende", „Danke fürs Spielen!", „Pfiat di, Resi!".

use jnr_core::{EndingPhase, Game};

use crate::blit::draw_sprite;
use crate::palette::{MOZART_YELLOW, NAVY, OUTLINE, SLATE_DARK, WHITE, WINE_DARK};
use crate::scene::RenderContext;
use crate::screens::{self, centred};
use crate::{Framebuffer, HEIGHT, hud, map};

/// The Scene's framed picture: 288×120, origin 0,0, drawn at [`CAFE_POS`].
pub const CAFE_SPRITE: &str = "ending_cafe";
/// The bill: 176×144, origin 0,0, drawn at [`BILL_POS`].
pub const ZETTEL_SPRITE: &str = "ending_zettel";
/// Top-left corner of `ending_cafe`.
pub const CAFE_POS: (i32, i32) = (16, 12);
/// The Scene's three lines (`ending.scene_1..3`) and their tops.
pub const SCENE_KEYS: [&str; 3] = ["ending.scene_1", "ending.scene_2", "ending.scene_3"];
const SCENE_Y: [i32; 3] = [140, 150, 160];

/// The bill's box `(x, y, w, h)`: `ending_zettel`'s place, or the
/// fallback panel (centred: 72 + 176 + 72 = 320).
pub const BILL_BOX: (i32, i32, i32, i32) = (72, 18, 176, 144);
/// Inside the bill: the text area's left edge, the right edge numbers end
/// on (last pixel column), the first line's top and the line pitch
/// (pixel-artist's layout of `ending_zettel`: x 8–167, y 16 + 12k).
const BILL_LEFT: i32 = 8;
const BILL_RIGHT: i32 = 167;
const BILL_TOP: i32 = 16;
const BILL_PITCH: i32 = 12;
/// `ending.bill_title` and the bill's lines in order of appearance.
pub const BILL_TITLE_KEY: &str = "ending.bill_title";
/// `ending.bill_score`, `.bill_oblaten`, `.bill_lives`, `.bill_tip`,
/// `.bill_thanks`.
pub const BILL_KEYS: [&str; 5] = [
    "ending.bill_score",
    "ending.bill_oblaten",
    "ending.bill_lives",
    "ending.bill_tip",
    "ending.bill_thanks",
];
/// The bill line (0–8) each of [`BILL_KEYS`] sits on; the title is line 0.
const BILL_ROWS: [i32; 5] = [2, 3, 4, 6, 8];

/// `ending.end_title`, `.end_line1`, `.end_line2` and their tops.
pub const END_KEYS: [&str; 3] = ["ending.end_title", "ending.end_line1", "ending.end_line2"];
const END_Y: [i32; 3] = [70, 92, 104];

/// Credits: line pitch, and the extra gap below a page's title.
const CREDITS_PITCH: i32 = 12;
const CREDITS_TITLE_GAP: i32 = 6;

/// Draws the ending (navy if the game has none).
pub fn draw_ending(game: &Game, ctx: &RenderContext, fb: &mut Framebuffer) {
    let Some(e) = game.ending() else {
        fb.clear(NAVY);
        return;
    };
    match e.phase {
        EndingPhase::Map => map::draw_ending_map(game, ctx, fb),
        EndingPhase::Scene => draw_scene(ctx, fb, u32::from(e.timer)),
        EndingPhase::Bill => draw_bill(game, ctx, fb, e.timer),
        EndingPhase::Credits => draw_credits(ctx, fb, e.page),
        EndingPhase::Ende => {
            fb.clear(NAVY);
            let texts = ctx.texts();
            for (i, key) in END_KEYS.iter().enumerate() {
                let colour = if i == 0 { MOZART_YELLOW } else { WHITE };
                centred(ctx, fb, &texts.get(key), END_Y[i], colour);
            }
        }
    }
}

fn draw_scene(ctx: &RenderContext, fb: &mut Framebuffer, tick: u32) {
    fb.clear(NAVY);
    if let Some(s) = ctx.ending_cafe {
        draw_sprite(
            fb,
            ctx.atlas(),
            s,
            &s.anims[0],
            tick,
            CAFE_POS.0,
            CAFE_POS.1,
            false,
        );
    }
    for (key, y) in SCENE_KEYS.iter().zip(SCENE_Y) {
        centred(ctx, fb, &ctx.texts().get(key), y, WHITE);
    }
}

/// How many of [`BILL_KEYS`] show `timer` frames into the bill.
pub fn bill_lines_shown(timer: u16, line_frames: u16) -> usize {
    usize::from(timer / line_frames.max(1)).min(BILL_KEYS.len())
}

/// The top of bill line `k` (0–8) on screen.
pub fn bill_line_y(k: i32) -> i32 {
    BILL_BOX.1 + BILL_TOP + BILL_PITCH * k
}

fn draw_bill(game: &Game, ctx: &RenderContext, fb: &mut Framebuffer, timer: u16) {
    fb.clear(NAVY);
    let (x0, y0, _, _) = BILL_BOX;
    // On the paper: dark ink without shadow; on the panel: white with it.
    let (ink, accent, shadow) = match ctx.ending_zettel {
        Some(s) => {
            draw_sprite(fb, ctx.atlas(), s, &s.anims[0], 0, x0, y0, false);
            (SLATE_DARK, WINE_DARK, None)
        }
        None => {
            screens::draw_panel(fb, BILL_BOX);
            (WHITE, WHITE, Some(OUTLINE))
        }
    };
    let (font, atlas, texts) = (ctx.font(), ctx.atlas(), ctx.texts());
    let centre = |fb: &mut Framebuffer, text: &str, y: i32, colour: u32| {
        // Rounded down like `screens::centred` (also for text wider than the bill).
        let x = x0 + (BILL_BOX.2 - font.measure(text)).div_euclid(2);
        font.draw_text(fb, atlas, text, x, y, colour, shadow);
    };
    centre(fb, &texts.get(BILL_TITLE_KEY), bill_line_y(0), accent);
    let shown = bill_lines_shown(timer, game.tuning().ending.bill_line_frames);
    let numbers = [
        Some(game.score()),
        Some(game.oblaten_total()),
        Some(u32::from(game.lives())),
        None,
        None,
    ];
    for i in 0..shown {
        let y = bill_line_y(BILL_ROWS[i]);
        let text = texts.get(BILL_KEYS[i]);
        if i == BILL_KEYS.len() - 1 {
            centre(fb, &text, y, accent);
            continue;
        }
        font.draw_text(fb, atlas, &text, x0 + BILL_LEFT, y, ink, shadow);
        if let Some(n) = numbers[i] {
            let mut buf = [0u8; 10];
            let digits = hud::padded(n, 1, &mut buf);
            let x = x0 + BILL_RIGHT + 1 - font.measure(digits);
            font.draw_text_outlined(fb, atlas, digits, x, y, MOZART_YELLOW, OUTLINE);
        }
    }
}

/// The text keys of credits page `page` (0-based) that exist, title first:
/// `credits.page<N>.title`, `.line1` … `.line4`.
pub fn credits_lines(ctx: &RenderContext, page: u8) -> Vec<(String, bool)> {
    let n = u32::from(page) + 1;
    ["title", "line1", "line2", "line3", "line4"]
        .iter()
        .map(|part| (format!("credits.page{n}.{part}"), *part == "title"))
        .filter(|(key, _)| ctx.texts().contains(key))
        .collect()
}

fn draw_credits(ctx: &RenderContext, fb: &mut Framebuffer, page: u8) {
    fb.clear(NAVY);
    let lines = credits_lines(ctx, page);
    let titled = lines.first().is_some_and(|l| l.1);
    let n = lines.len() as i32;
    let height = n * CREDITS_PITCH + if titled { CREDITS_TITLE_GAP } else { 0 };
    let mut y = (HEIGHT as i32 - height) / 2;
    for (key, title) in &lines {
        let colour = if *title { MOZART_YELLOW } else { WHITE };
        centred(ctx, fb, &ctx.texts().get(key), y, colour);
        y += CREDITS_PITCH + if *title { CREDITS_TITLE_GAP } else { 0 };
    }
}
#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::font::{CHARSET, FONT_SPRITE, GLYPH_ANIM};
    use crate::palette::{DIRNDL_RED, LEATHER_DARK, SILVER_LIGHT, SKY_BLUE};
    use crate::scene::{self, render};
    use crate::testutil::{self, image};
    use jnr_core::route::STATIONS;
    use jnr_core::{ClearPhase, InputFrame, Level, Mode, Route, Start, Texts, Tuning};

    fn texts() -> Texts {
        Texts::parse(include_str!("../../../content/text/de.toml")).unwrap()
    }

    fn tuning() -> Tuning {
        Tuning::parse(include_str!("../../../content/tuning.toml")).unwrap()
    }

    fn glyphs() -> Vec<jnr_artkit::Image> {
        CHARSET.chars().map(|_| image(&["aaaaaaaa"; 8])).collect()
    }

    /// Every glyph a full 8x8 block (advance 9, space 3).
    fn block_font() -> jnr_artkit::Atlas {
        testutil::atlas(&[testutil::sprite(
            FONT_SPRITE,
            (0, 0),
            false,
            vec![(GLYPH_ANIM, 0, glyphs())],
        )])
    }

    /// The finale on the floor: the `goal_stollen` 4 columns right of
    /// Resi, an Oblate on the way.
    fn finale() -> Level {
        Level::parse(
            "[meta]\nid = \"fin\"\ntime_limit = 5\n[legend]\n\"#\" = \"solid\"\n\
             \"P\" = { entity = \"player_start\" }\n\"S\" = { entity = \"goal_stollen\" }\n\
             \"o\" = { entity = \"oblate\" }\n\
             [map]\ngrid = '''\n........\n........\n..Po..S.\n########\n'''\n",
        )
        .unwrap()
    }

    /// The real flow on the journey's last station (the finale level).
    fn last_station() -> Game {
        let levels: Vec<Level> = (0..9).map(|_| finale()).collect();
        let route = Arc::new(Route::new(levels).unwrap());
        Game::new_run(route, 8, &tuning(), 1, Start::Title)
    }

    fn step_until(g: &mut Game, input: InputFrame, pred: impl Fn(&Game) -> bool) {
        while !pred(g) {
            g.step(input);
            assert!(g.frame() < 6000, "never reached");
        }
    }

    /// The game on the first frame of the ending.
    fn in_ending() -> Game {
        let mut g = last_station();
        g.step(InputFrame::START);
        step_until(&mut g, InputFrame::RIGHT, |g| g.mode() == Mode::StageClear);
        step_until(&mut g, InputFrame::NONE, |g| g.mode() == Mode::Ending);
        g
    }

    /// Steps until the ending is in `phase` (and `page`), then `extra`
    /// frames more.
    fn at(g: &mut Game, phase: EndingPhase, page: u8, extra: u32) {
        step_until(g, InputFrame::NONE, |g| {
            g.ending()
                .is_some_and(|e| e.phase == phase && e.page == page)
        });
        for _ in 0..extra {
            g.step(InputFrame::NONE);
        }
    }

    fn rendered(game: &Game, atlas: &jnr_artkit::Atlas) -> Framebuffer {
        let texts = texts();
        let ctx = RenderContext::new(atlas, &texts, game.level());
        let mut fb = testutil::fb();
        render(game, &ctx, &mut fb);
        fb
    }

    fn width(text: &str) -> i32 {
        crate::Font::from_atlas(&block_font()).measure(text)
    }

    /// Left edge of `text` centred on the screen with the block font.
    fn centred_x(text: &str) -> i32 {
        (320 - width(text)) / 2
    }

    #[test]
    fn map_phase_shows_every_station_done_and_the_three_lines() {
        let atlas = block_font();
        let mut g = in_ending();
        let fb = rendered(&g, &atlas);
        let t = texts();
        // Fallback markers: every station `done` (red), none blinking or
        // to do (the block font's wide labels cover some dots).
        for st in &STATIONS[..2] {
            assert_eq!(fb.get(st.dot.0, st.dot.1), Some(DIRNDL_RED), "{}", st.id);
        }
        for st in &STATIONS {
            let (x, y) = st.dot;
            for colour in [MOZART_YELLOW, crate::palette::STONE_GREY] {
                assert_ne!(fb.get(x + 2, y + 2), Some(colour), "{}", st.id);
            }
        }
        let lines = [
            ("ending.map_title", 143, WHITE),
            ("ending.map_route", 153, MOZART_YELLOW),
            ("ending.map_note", 165, WHITE),
        ];
        for (key, y, colour) in lines {
            assert_eq!(fb.get(centred_x(&t.get(key)), y), Some(colour), "{key}");
        }
        // Steady: the note is still there 30 frames later.
        for _ in 0..30 {
            g.step(InputFrame::NONE);
        }
        let fb = rendered(&g, &atlas);
        let note = t.get("ending.map_note");
        assert_eq!(fb.get(centred_x(&note), 165), Some(WHITE));
    }

    #[test]
    fn scene_without_the_picture_is_text_on_navy() {
        let atlas = block_font();
        let mut g = in_ending();
        at(&mut g, EndingPhase::Scene, 0, 0);
        let fb = rendered(&g, &atlas);
        assert_eq!(fb.get(0, 0), Some(NAVY));
        assert_eq!(
            fb.get(CAFE_POS.0 + 10, CAFE_POS.1 + 10),
            Some(NAVY),
            "no picture"
        );
        for (key, y) in SCENE_KEYS.iter().zip(SCENE_Y) {
            assert_eq!(
                fb.get(centred_x(&texts().get(key)), y),
                Some(WHITE),
                "{key}"
            );
        }
        // With `ending_cafe`: drawn at (16, 12).
        let atlas = testutil::atlas(&[
            testutil::sprite(FONT_SPRITE, (0, 0), false, vec![(GLYPH_ANIM, 0, glyphs())]),
            testutil::sprite(
                CAFE_SPRITE,
                (0, 0),
                false,
                vec![("idle", 20, vec![image(&["bb"; 2])])],
            ),
        ]);
        let fb = rendered(&g, &atlas);
        assert_eq!(fb.get(16, 12), Some(testutil::B));
        assert_eq!(fb.get(15, 12), Some(NAVY));
    }

    #[test]
    fn bill_panel_shows_one_line_every_20_frames_with_the_run_totals() {
        let atlas = block_font();
        let mut g = in_ending();
        at(&mut g, EndingPhase::Bill, 0, 0);
        let fb = rendered(&g, &atlas);
        // The D-037 panel in the bill's box.
        let (x0, y0, w, h) = BILL_BOX;
        assert_eq!(fb.get(x0, y0), Some(OUTLINE));
        assert_eq!(fb.get(x0 + w - 1, y0 + h - 1), Some(OUTLINE));
        assert_eq!(fb.get(x0 + 10, y0 + 1), Some(SKY_BLUE));
        let t = texts();
        let title = t.get(BILL_TITLE_KEY);
        assert_eq!(fb.get(centred_x(&title), bill_line_y(0)), Some(WHITE));
        let score_y = bill_line_y(2);
        assert_eq!(fb.get(x0 + BILL_LEFT, score_y), Some(NAVY), "not yet");
        for _ in 0..20 {
            g.step(InputFrame::NONE);
        }
        let fb = rendered(&g, &atlas);
        assert_eq!(fb.get(x0 + BILL_LEFT, score_y), Some(WHITE), "Punkte");
        assert_eq!(fb.get(x0 + BILL_LEFT, bill_line_y(3)), Some(NAVY));
        // The number right-aligned to x 239 in `Y`.
        let mut buf = [0u8; 10];
        let score = hud::padded(g.score(), 1, &mut buf).to_owned();
        let nx = x0 + BILL_RIGHT + 1 - width(&score);
        assert_eq!(fb.get(nx, score_y), Some(MOZART_YELLOW));
        assert_eq!(bill_lines_shown(19, 20), 0);
        assert_eq!(bill_lines_shown(20, 20), 1);
        assert_eq!(bill_lines_shown(100, 20), 5);
        assert_eq!(bill_lines_shown(599, 20), 5);
        // All lines: the Oblaten total (1) and lives (3), the thanks centred.
        for _ in 0..80 {
            g.step(InputFrame::NONE);
        }
        assert_eq!((g.oblaten_total(), g.lives()), (1, 3));
        let fb = rendered(&g, &atlas);
        let one = x0 + BILL_RIGHT + 1 - width("1");
        assert_eq!(fb.get(one, bill_line_y(3)), Some(MOZART_YELLOW));
        let three = x0 + BILL_RIGHT + 1 - width("3");
        assert_eq!(fb.get(three, bill_line_y(4)), Some(MOZART_YELLOW));
        assert_eq!(fb.get(x0 + BILL_LEFT, bill_line_y(6)), Some(WHITE), "tip");
        let thanks = t.get("ending.bill_thanks");
        assert_eq!(fb.get(centred_x(&thanks), bill_line_y(8)), Some(WHITE));
    }

    #[test]
    fn bill_on_the_zettel_uses_ink_colours() {
        let atlas = testutil::atlas(&[
            testutil::sprite(FONT_SPRITE, (0, 0), false, vec![(GLYPH_ANIM, 0, glyphs())]),
            testutil::sprite(
                ZETTEL_SPRITE,
                (0, 0),
                false,
                vec![("idle", 1, vec![image(&["cc"; 2])])],
            ),
        ]);
        let mut g = in_ending();
        at(&mut g, EndingPhase::Bill, 0, 20);
        let fb = rendered(&g, &atlas);
        let (x0, y0, _, _) = BILL_BOX;
        assert_eq!(fb.get(x0, y0), Some(testutil::C), "the slip at (72, 18)");
        let title = texts().get(BILL_TITLE_KEY).into_owned();
        assert_eq!(fb.get(centred_x(&title), bill_line_y(0)), Some(WINE_DARK));
        assert_eq!(fb.get(x0 + BILL_LEFT, bill_line_y(2)), Some(SLATE_DARK));
        assert_eq!(
            fb.get(x0 + BILL_LEFT + 8, bill_line_y(2) + 8),
            Some(NAVY),
            "no shadow on paper"
        );
    }

    #[test]
    fn credits_pages_skip_missing_keys() {
        let atlas = block_font();
        let t = texts();
        let mut g = in_ending();
        at(&mut g, EndingPhase::Credits, 0, 0);
        let ctx = RenderContext::new(&atlas, &t, g.level());
        // Page 1: a title and two lines; page 6: four lines, no title.
        let p1 = credits_lines(&ctx, 0);
        assert_eq!(p1.len(), 3);
        assert!(p1[0].1 && !p1[1].1);
        let p6 = credits_lines(&ctx, 5);
        assert_eq!(p6.len(), 4);
        assert!(p6.iter().all(|l| !l.1));
        assert!(credits_lines(&ctx, 8).is_empty(), "no page 9");
        for page in 0..tuning().ending.credits_pages {
            assert!(!credits_lines(&ctx, page).is_empty(), "page {page}");
        }
        // Page 1 drawn: the title in `Y` at the top of the centred block.
        let fb = rendered(&g, &atlas);
        let h = 3 * CREDITS_PITCH + CREDITS_TITLE_GAP;
        let top = (180 - h) / 2;
        let title = t.get("credits.page1.title");
        assert_eq!(fb.get(centred_x(&title), top), Some(MOZART_YELLOW));
        let line1 = t.get("credits.page1.line1");
        let y1 = top + CREDITS_PITCH + CREDITS_TITLE_GAP;
        assert_eq!(fb.get(centred_x(&line1), y1), Some(WHITE));
    }

    #[test]
    fn ende_then_the_marked_title() {
        let atlas = block_font();
        let t = texts();
        let mut g = in_ending();
        at(&mut g, EndingPhase::Ende, 0, 0);
        let fb = rendered(&g, &atlas);
        assert_eq!(fb.get(0, 0), Some(NAVY));
        for (i, key) in END_KEYS.iter().enumerate() {
            let colour = if i == 0 { MOZART_YELLOW } else { WHITE };
            assert_eq!(
                fb.get(centred_x(&t.get(key)), END_Y[i]),
                Some(colour),
                "{key}"
            );
        }
        step_until(&mut g, InputFrame::NONE, |g| {
            matches!(g.mode(), Mode::Title { .. })
        });
        let fb = rendered(&g, &atlas);
        let cleared = t.get(screens::CLEARED_KEY);
        let cy = screens::CLEARED_Y;
        assert_eq!(fb.get(centred_x(&cleared), cy), Some(MOZART_YELLOW));
        let again = t.get(screens::PRESS_START_AGAIN_KEY);
        assert_eq!(
            fb.get(centred_x(&again), screens::PRESS_START_Y),
            Some(WHITE)
        );
        let normal = t.get(screens::PRESS_START_KEY);
        assert_ne!(width(&again), width(&normal), "a different prompt");
        // The plate at Resi's feet (grey box: a white plate line, the
        // Stollen on it).
        let p = g.player();
        let cam = g.camera();
        let fx = (p.x + p.w.div_int(2)).to_px() - cam.x + 16;
        let fy = (p.y + p.h).to_px() - cam.y;
        assert_eq!(fb.get(fx, fy - 1), Some(WHITE));
        assert_eq!(fb.get(fx, fy - 3), Some(LEATHER_DARK));
        // An unmarked title has none of it.
        let fb = rendered(&last_station(), &atlas);
        assert_ne!(fb.get(centred_x(&cleared), cy), Some(MOZART_YELLOW));
        assert_ne!(fb.get(fx, fy - 3), Some(LEATHER_DARK));
    }

    #[test]
    fn the_bell_and_the_serve() {
        let atlas = block_font();
        let mut g = Game::new(&finale(), &tuning(), 1);
        // Armed (no boss): `offen`, the dome lifted 6 px.
        assert_eq!(scene::glocke_anim(&g, 0), "offen");
        let fb = rendered(&g, &atlas);
        let cam = g.camera();
        let (bx, by) = (6 * 16 + 8 - cam.x, 3 * 16 - cam.y);
        assert_eq!(fb.get(bx - 7, by - 20), Some(SILVER_LIGHT), "lifted dome");
        assert_eq!(fb.get(bx, by - 3), Some(LEATHER_DARK), "the Stollen");
        // Taken: `leer`, and the plate above her head in the Serve.
        step_until(&mut g, InputFrame::RIGHT, |g| g.mode() == Mode::StageClear);
        assert_eq!(scene::glocke_anim(&g, 0), "leer");
        assert_eq!(g.stage_clear().unwrap().phase, ClearPhase::Serve);
        assert!(scene::serving(&g));
        // Past the popups („Zaunerstollen!" rises over her head first).
        for _ in 0..40 {
            g.step(InputFrame::NONE);
        }
        assert_eq!(g.stage_clear().unwrap().phase, ClearPhase::Serve);
        let fb = rendered(&g, &atlas);
        let p = g.player();
        let cam = g.camera();
        let hx = (p.x + p.w.div_int(2)).to_px() - cam.x;
        let hy = (p.y + p.h).to_px() - cam.y - map::resi_height(p.power) - 1;
        assert_eq!(fb.get(hx, hy - 1), Some(WHITE), "plate");
        assert_eq!(fb.get(hx, hy - 3), Some(LEATHER_DARK), "Stollen");
        // Held on through the tally and the clear panel, not after.
        step_until(&mut g, InputFrame::NONE, |g| {
            g.stage_clear()
                .is_some_and(|c| c.phase == ClearPhase::Clear)
        });
        assert!(scene::serving(&g));
        step_until(&mut g, InputFrame::NONE, |g| {
            g.stage_clear()
                .is_some_and(|c| c.phase == ClearPhase::ToBeContinued)
        });
        assert!(!scene::serving(&g));
    }

    #[test]
    fn a_maibaum_clear_beside_a_stollen_shows_no_plate() {
        let level = Level::parse(
            "[meta]\nid = \"both\"\ntime_limit = 5\n[legend]\n\"#\" = \"solid\"\n\
             \"P\" = { entity = \"player_start\" }\n\"S\" = { entity = \"goal_stollen\" }\n\
             \"G\" = { entity = \"goal\" }\n\
             [map]\ngrid = '''\n..........\n..........\nS.P...G...\n##########\n'''\n",
        )
        .unwrap();
        let mut g = Game::new(&level, &tuning(), 1);
        step_until(&mut g, InputFrame::RIGHT, |g| g.mode() == Mode::StageClear);
        // Goal 0 is the Stollen (left), goal 1 the Maibaum she touched.
        assert_eq!(scene::cleared_goal(&g), Some(1));
        assert_eq!(scene::glocke_anim(&g, 0), "offen");
        step_until(&mut g, InputFrame::NONE, |g| {
            g.stage_clear()
                .is_some_and(|c| c.phase == ClearPhase::Tally)
        });
        assert!(!scene::serving(&g));
    }
}
