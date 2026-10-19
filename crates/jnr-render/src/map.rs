//! The route map between stages (`docs/game-design.md` → "Route map",
//! D-043): the backdrop `map_salzkammergut` (fallback: a flat meadow fill),
//! the route drawn from the waypoint data of [`jnr_core::route`] (every
//! 4 px a 1 px dot, the walked part as 2×2 dots), the `map_station`
//! markers (`todo`/`done`/`next`; fallback: 5×5 squares), a name label for
//! every station (full `k` outline: reached `w`, next `Y`, future `i`),
//! Resi walking or standing (over the labels), and the D-037 panel with „Nächster Halt:",
//! the next station's name and the blinking prompt (or the teaser text).
//! Full screen, no HUD.

use jnr_core::Game;
use jnr_core::route::{self, LabelSide, MapPhase, RouteMap, STATIONS, Station};

use crate::palette::{
    DIRNDL_RED, MEADOW, MOZART_YELLOW, NAVY, OUTLINE, SILVER_LIGHT, STONE_GREY, WHITE,
};
use crate::scene::{self, PlayerAnim, RenderContext};
use crate::screens::{self, blink_on};
use crate::{Framebuffer, WIDTH, blit::draw_sprite};

/// The backdrop: 320×180, origin 0,0, no text in the art.
pub const BACKDROP_SPRITE: &str = "map_salzkammergut";
/// The station marker: 8×8, origin 4,4, anims `todo`, `done`, `next`.
pub const STATION_SPRITE: &str = "map_station";
/// `map.next_stop` („Nächster Halt:").
pub const NEXT_STOP_KEY: &str = "map.next_stop";
/// `map.continue` („Weiter mit START").
pub const CONTINUE_KEY: &str = "map.continue";
/// `map.station.<id>` for every station, in [`STATIONS`] order.
pub const STATION_KEYS: [&str; 9] = [
    "map.station.salzburg",
    "map.station.st_gilgen",
    "map.station.brunnwinkl",
    "map.station.falkenstein",
    "map.station.st_wolfgang",
    "map.station.wolfgangsee",
    "map.station.strobl",
    "map.station.bad_ischl",
    "map.station.cafe_zauner",
];

/// Route dots every this many px along the path.
pub const DOT_SPACING: i32 = 4;
/// Labels above and below: distance from the dot centre to the near edge.
const LABEL_GAP: i32 = 6;
/// Labels left and right: distance from the dot centre to the near edge
/// (clear of Resi's 16 px wide sprite standing on the dot).
const LABEL_GAP_X: i32 = 9;
/// Labels stay at least this far inside the screen.
const LABEL_MARGIN: i32 = 4;
/// Glyph height of the font (labels are one line).
const GLYPH_H: i32 = 8;
/// Resi's feet are this far below the station dot (she stands on it).
const RESI_DY: i32 = 2;
/// The panel (D-037 style): top edge and height (y 138..=175).
pub const PANEL_Y: i32 = 138;
const PANEL_H: i32 = 38;
const PANEL_PADDING: i32 = 4;
const NEXT_STOP_Y: i32 = 143;
const NAME_Y: i32 = 153;
const PROMPT_Y: i32 = 165;

/// How a station marker looks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Marker {
    /// Not reached yet.
    Todo,
    /// Cleared.
    Done,
    /// Resi's destination (blinking).
    Next,
}

impl Marker {
    fn anim(self) -> &'static str {
        match self {
            Marker::Todo => "todo",
            Marker::Done => "done",
            Marker::Next => "next",
        }
    }
}

/// The marker of station `i` on the map `m`: stations before the cleared
/// one are done, the cleared one turns done at `map.done_frame` of the
/// arrival (it was the destination until then), the next one blinks.
pub fn marker(m: &RouteMap, i: usize, done_frame: u16) -> Marker {
    let from = usize::from(m.from);
    if i < from || (i == from && (m.phase != MapPhase::Arrive || m.timer >= done_frame)) {
        Marker::Done
    } else if i == from || i == m.next() {
        Marker::Next
    } else {
        Marker::Todo
    }
}

/// Top-left corner of a label `w` px wide for a dot, kept inside the screen.
pub fn label_pos(dot: (i32, i32), side: LabelSide, w: i32) -> (i32, i32) {
    let (x, y) = match side {
        LabelSide::Right => (dot.0 + LABEL_GAP_X, dot.1 - GLYPH_H / 2),
        LabelSide::Left => (dot.0 - LABEL_GAP_X - w, dot.1 - GLYPH_H / 2),
        LabelSide::Above => (dot.0 - w / 2, dot.1 - LABEL_GAP - GLYPH_H),
        LabelSide::Below => (dot.0 - w / 2, dot.1 + LABEL_GAP),
    };
    let max_x = (WIDTH as i32 - LABEL_MARGIN - w).max(LABEL_MARGIN);
    (x.clamp(LABEL_MARGIN, max_x), y.max(LABEL_MARGIN))
}

/// [`label_pos`] for a station, with its fixed top edge and right limit
/// from the route data.
pub fn station_label_pos(st: &Station, w: i32) -> (i32, i32) {
    let (mut x, mut y) = label_pos(st.dot, st.label, w);
    if let Some(top) = st.label_top {
        y = top;
    }
    if let Some(right) = st.label_right {
        x = x.min(right - w).max(LABEL_MARGIN);
    }
    (x, y)
}

/// Draws the route map of `game` (navy if it has none).
pub fn draw_route_map(game: &Game, ctx: &RenderContext, fb: &mut Framebuffer) {
    let Some(m) = game.route_map() else {
        fb.clear(NAVY);
        return;
    };
    let (atlas, tick) = (ctx.atlas(), game.frame());
    fb.clear(MEADOW);
    if let Some(s) = ctx.map_backdrop {
        draw_sprite(fb, atlas, s, &s.anims[0], tick, 0, 0, false);
    }
    draw_route(fb, m);

    let done_frame = game.tuning().map.done_frame;
    for (i, st) in STATIONS.iter().enumerate() {
        let look = marker(m, i, done_frame);
        let anim = ctx
            .map_station
            .and_then(|s| s.anim(look.anim()).map(|a| (s, a)));
        match anim {
            Some((s, a)) => draw_sprite(fb, atlas, s, a, tick, st.dot.0, st.dot.1, false),
            None => {
                let fill = match look {
                    Marker::Done => DIRNDL_RED,
                    Marker::Next if blink_on(tick) => MOZART_YELLOW,
                    Marker::Next | Marker::Todo => STONE_GREY,
                };
                fb.fill_rect(st.dot.0 - 2, st.dot.1 - 2, 5, 5, fill);
                fb.outline_rect(st.dot.0 - 2, st.dot.1 - 2, 5, 5, OUTLINE);
            }
        }
    }

    let (font, texts) = (ctx.font(), ctx.texts());
    let from = usize::from(m.from);
    for (i, st) in STATIONS.iter().enumerate() {
        let colour = if i <= from {
            WHITE
        } else if i == m.next() {
            MOZART_YELLOW
        } else {
            SILVER_LIGHT
        };
        let text = texts.get(STATION_KEYS[i]);
        let (lx, ly) = station_label_pos(st, font.measure(&text));
        font.draw_text_outlined(fb, atlas, &text, lx, ly, colour, OUTLINE);
    }

    // Resi over the labels (she stands on a dot next to its label).
    let ((x, y), left) = m.resi();
    let state = if m.phase == MapPhase::Walk {
        PlayerAnim::Walk
    } else {
        PlayerAnim::Idle
    };
    scene::draw_player_pose(
        ctx,
        fb,
        game.player().power,
        state,
        (x, y + RESI_DY),
        tick,
        left,
    );

    if m.phase != MapPhase::Arrive {
        draw_map_panel(ctx, fb, m);
    }
}

/// The route: 1 px dots every [`DOT_SPACING`] px; the walked part (every
/// path before the cleared station, and this walk so far) as 2×2 dots.
fn draw_route(fb: &mut Framebuffer, m: &RouteMap) {
    let from = usize::from(m.from);
    for i in 0..STATIONS.len() - 1 {
        let Some(points) = route::path(i) else {
            continue;
        };
        let walked = match i.cmp(&from) {
            std::cmp::Ordering::Less => i32::MAX,
            std::cmp::Ordering::Equal => i32::from(m.walked),
            std::cmp::Ordering::Greater => -1,
        };
        let len = route::path_len(i);
        let mut d = 0;
        while d <= len {
            let ((x, y), _) = route::point_along(&points, d);
            if d <= walked {
                fb.fill_rect(x, y, 2, 2, DIRNDL_RED);
            } else {
                fb.set(x, y, WHITE);
            }
            d += DOT_SPACING;
        }
    }
}

/// The panel `(x, y, w, h)`: centred, as wide as the widest of its lines
/// (+1 px shadow) plus padding and border; line 3 is measured with the
/// text it will show (prompt or teaser), so the width never changes.
pub fn map_panel(ctx: &RenderContext, m: &RouteMap) -> (i32, i32, i32, i32) {
    let (font, texts) = (ctx.font(), ctx.texts());
    let name = texts.get(STATION_KEYS[m.next().min(STATIONS.len() - 1)]);
    let widest = font
        .measure(&texts.get(NEXT_STOP_KEY))
        .max(font.measure(&name))
        .max(font.measure(&texts.get(line3_key(m))));
    let w = (widest + 1 + 2 * (1 + PANEL_PADDING) + 1) / 2 * 2;
    ((WIDTH as i32 - w) / 2, PANEL_Y, w, PANEL_H)
}

fn line3_key(m: &RouteMap) -> &'static str {
    if m.teaser {
        screens::TO_BE_CONTINUED_KEY
    } else {
        CONTINUE_KEY
    }
}

fn draw_map_panel(ctx: &RenderContext, fb: &mut Framebuffer, m: &RouteMap) {
    screens::draw_panel(fb, map_panel(ctx, m));
    let texts = ctx.texts();
    screens::centred(ctx, fb, &texts.get(NEXT_STOP_KEY), NEXT_STOP_Y, WHITE);
    let name = texts.get(STATION_KEYS[m.next().min(STATIONS.len() - 1)]);
    screens::centred(ctx, fb, &name, NAME_Y, MOZART_YELLOW);
    let line3 = match m.phase {
        MapPhase::Teaser => true,
        MapPhase::Wait => blink_on(u32::from(m.timer)),
        MapPhase::Arrive | MapPhase::Walk => false,
    };
    if line3 {
        screens::centred(ctx, fb, &texts.get(line3_key(m)), PROMPT_Y, WHITE);
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::font::{CHARSET, FONT_SPRITE, GLYPH_ANIM};
    use crate::scene::render;
    use crate::testutil::{self, image};
    use jnr_core::{InputFrame, Level, Mode, Power, Route, Start, Texts, Tuning};

    fn texts() -> Texts {
        Texts::parse(include_str!("../../../content/text/de.toml")).unwrap()
    }

    fn tuning() -> Tuning {
        Tuning::parse(include_str!("../../../content/tuning.toml")).unwrap()
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

    fn stage(id: &str) -> Level {
        Level::parse(&format!(
            "[meta]\nid = \"{id}\"\ntime_limit = 5\n[legend]\n\"#\" = \"solid\"\n\
             \"P\" = {{ entity = \"player_start\" }}\n\"G\" = {{ entity = \"goal\" }}\n\
             [map]\ngrid = '''\n......G.\n..P.....\n########\n'''\n"
        ))
        .unwrap()
    }

    /// A two-stage run on the route map right after clearing stage `at`.
    fn on_map(at: usize) -> Game {
        let route = Route::new(vec![stage("a"), stage("b")]).unwrap();
        let mut g = Game::new_run(Arc::new(route), at, &tuning(), 1, Start::Title);
        let step = |g: &mut Game, i: InputFrame| {
            g.step(i);
            assert!(g.frame() < 3000, "never reached the map");
        };
        step(&mut g, InputFrame::START);
        while g.mode() != Mode::StageClear {
            step(&mut g, InputFrame::RIGHT);
        }
        while g.mode() != Mode::RouteMap {
            step(&mut g, InputFrame::NONE);
        }
        g
    }

    fn rendered(game: &Game, atlas: &jnr_artkit::Atlas) -> Framebuffer {
        let texts = texts();
        let ctx = RenderContext::new(atlas, &texts, game.level());
        let mut fb = testutil::fb();
        render(game, &ctx, &mut fb);
        fb
    }

    fn centred_x(text: &str) -> i32 {
        let font = crate::Font::from_atlas(&block_font());
        (320 - font.measure(text)) / 2
    }

    #[test]
    fn station_keys_match_the_stations_and_exist() {
        let t = texts();
        for (key, st) in STATION_KEYS.iter().zip(&STATIONS) {
            assert_eq!(*key, format!("map.station.{}", st.id));
            assert!(t.contains(key), "{key}");
            assert!(screens::TEXT_KEYS.contains(key), "{key} in TEXT_KEYS");
        }
        for key in [NEXT_STOP_KEY, CONTINUE_KEY] {
            assert!(screens::TEXT_KEYS.contains(&key), "{key}");
        }
    }

    #[test]
    fn labels_stay_inside_and_do_not_overlap_with_the_real_font() {
        let texts = texts();
        let font = crate::Font::from_atlas(jnr_assets::atlas());
        let boxes: Vec<(i32, i32, i32)> = STATIONS
            .iter()
            .zip(STATION_KEYS)
            .map(|(st, key)| {
                let w = font.measure(&texts.get(key));
                let (x, y) = station_label_pos(st, w);
                assert!(x >= 4 && x + w <= 316, "{key}: x {x}, w {w}");
                assert!(y >= 4 && y + GLYPH_H < PANEL_Y, "{key}: y {y}");
                (x, y, w)
            })
            .collect();
        for (i, a) in boxes.iter().enumerate() {
            for (j, b) in boxes.iter().enumerate().skip(i + 1) {
                // + 1 px outline around each label.
                let apart = a.0 + a.2 + 1 < b.0 - 1
                    || b.0 + b.2 + 1 < a.0 - 1
                    || a.1 + GLYPH_H + 1 < b.1 - 1
                    || b.1 + GLYPH_H + 1 < a.1 - 1;
                assert!(
                    apart,
                    "labels {} and {} overlap",
                    STATIONS[i].id, STATIONS[j].id
                );
            }
        }
    }

    #[test]
    fn label_positions_by_side_and_clamped() {
        assert_eq!(label_pos((28, 36), LabelSide::Right, 40), (37, 32));
        assert_eq!(label_pos((100, 70), LabelSide::Left, 40), (51, 66));
        assert_eq!(label_pos((124, 58), LabelSide::Above, 40), (104, 44));
        assert_eq!(label_pos((152, 68), LabelSide::Below, 40), (132, 74));
        // Café Zauner: above, pushed left to end at x 316.
        assert_eq!(label_pos((300, 76), LabelSide::Above, 60), (256, 62));
        assert_eq!(label_pos((2, 2), LabelSide::Left, 10), (4, 4));
        // Route-data overrides (art review): fixed rows, right limits.
        assert_eq!(station_label_pos(&STATIONS[2], 48), (66, 43), "Brunnwinkl");
        let (x, _) = station_label_pos(&STATIONS[3], 60);
        assert_eq!(x + 60, 172, "Falkenstein ends at x 172");
        assert_eq!(
            station_label_pos(&STATIONS[8], 58),
            (232, 69),
            "Café Zauner"
        );
    }

    #[test]
    fn markers_follow_the_phases() {
        let mut m = RouteMap {
            phase: MapPhase::Arrive,
            timer: 0,
            from: 1,
            walked: 0,
            teaser: false,
        };
        assert_eq!(marker(&m, 0, 20), Marker::Done);
        assert_eq!(marker(&m, 1, 20), Marker::Next, "until the done frame");
        assert_eq!(marker(&m, 2, 20), Marker::Next);
        assert_eq!(marker(&m, 3, 20), Marker::Todo);
        m.timer = 20;
        assert_eq!(marker(&m, 1, 20), Marker::Done);
        m.phase = MapPhase::Wait;
        m.timer = 0;
        assert_eq!(marker(&m, 1, 20), Marker::Done);
    }

    #[test]
    fn grey_box_map_with_route_markers_labels_and_resi() {
        let atlas = block_font();
        let g = on_map(0);
        let fb = rendered(&g, &atlas);
        // Flat meadow where nothing else is (no backdrop sprite).
        assert_eq!(fb.get(316, 4), Some(MEADOW));
        // No HUD, no panel while arriving.
        assert_ne!(fb.get(16, 4), Some(WHITE));
        assert_ne!(fb.get(160, PANEL_Y), Some(OUTLINE));
        // Resi (grey box) on Salzburg, feet 2 px below the dot.
        let (x, y) = STATIONS[0].dot;
        assert_eq!(fb.get(x, y + RESI_DY - 2), Some(DIRNDL_RED));
        assert_eq!(fb.get(x, y + RESI_DY - 22), Some(OUTLINE), "box top");
        // Brunnwinkl: a todo square (grey) with a k outline.
        let (bx, by) = STATIONS[2].dot;
        assert_eq!(fb.get(bx, by), Some(STONE_GREY));
        assert_eq!(fb.get(bx - 2, by - 2), Some(OUTLINE));
        // Labels: Salzburg reached (w), St. Gilgen next (Y), Falkenstein
        // future (i), each with the full outline.
        let font = crate::Font::from_atlas(&atlas);
        let t = texts();
        let w = |k: &str| font.measure(&t.get(k));
        let (sx, sy) = label_pos(STATIONS[0].dot, LabelSide::Right, w(STATION_KEYS[0]));
        assert_eq!(fb.get(sx + 2, sy + 2), Some(WHITE));
        assert_eq!(fb.get(sx - 1, sy - 1), Some(OUTLINE));
        let (gx, gy) = label_pos(STATIONS[1].dot, LabelSide::Left, w(STATION_KEYS[1]));
        assert_eq!(fb.get(gx + 2, gy + 2), Some(MOZART_YELLOW));
        let (fx, fy) = station_label_pos(&STATIONS[3], w(STATION_KEYS[3]));
        assert_eq!(fb.get(fx + 2, fy + 2), Some(SILVER_LIGHT));
        // The route: 1 px white dots ahead (St. Gilgen → Brunnwinkl starts
        // at the St. Gilgen dot, which the marker hides; the next one is
        // 4 px along).
        let p = route::path(1).unwrap();
        let ((dx, dy), _) = route::point_along(&p, 8);
        assert_eq!(fb.get(dx, dy), Some(WHITE));
    }

    #[test]
    fn walk_draws_bold_dots_and_the_panel_then_the_prompt_blinks() {
        let atlas = block_font();
        let t = tuning();
        let mut g = on_map(0);
        for _ in 0..t.map.arrive_frames + 30 {
            g.step(InputFrame::NONE);
        }
        let m = *g.route_map().unwrap();
        assert_eq!((m.phase, m.walked), (MapPhase::Walk, 30));
        let fb = rendered(&g, &atlas);
        let p = route::path(0).unwrap();
        // (Clear of the Salzburg label and of Resi.)
        let ((x, y), _) = route::point_along(&p, 28);
        assert_eq!(fb.get(x + 1, y + 1), Some(DIRNDL_RED), "walked: 2x2");
        let ((x, y), _) = route::point_along(&p, 40);
        assert_eq!(fb.get(x, y), Some(WHITE), "ahead: 1 px");
        assert_ne!(fb.get(x + 1, y + 1), Some(WHITE));
        // The panel from the walk on: border, highlight, texts; no prompt yet.
        let ctx_texts = texts();
        let ctx = RenderContext::new(&atlas, &ctx_texts, g.level());
        let (px, py, pw, ph) = map_panel(&ctx, &m);
        assert_eq!((py, py + ph - 1), (138, 175));
        assert_eq!(px * 2 + pw, 320, "centred");
        assert_eq!(fb.get(px, py), Some(OUTLINE));
        assert_eq!(fb.get(px + 1, py + 10), Some(crate::palette::SKY_BLUE));
        let tt = texts();
        let next_stop = tt.get(NEXT_STOP_KEY);
        assert_eq!(fb.get(centred_x(&next_stop), NEXT_STOP_Y), Some(WHITE));
        let name = tt.get(STATION_KEYS[1]);
        assert_eq!(fb.get(centred_x(&name), NAME_Y), Some(MOZART_YELLOW));
        let prompt = tt.get(CONTINUE_KEY);
        assert_eq!(fb.get(centred_x(&prompt), PROMPT_Y), Some(NAVY));
        // Wait: the prompt blinks 30 on, 30 off.
        g.step(InputFrame::START);
        assert_eq!(g.route_map().unwrap().phase, MapPhase::Wait);
        let fb = rendered(&g, &atlas);
        assert_eq!(fb.get(centred_x(&prompt), PROMPT_Y), Some(WHITE));
        let (x, y) = STATIONS[1].dot;
        assert_eq!(fb.get(x, y + RESI_DY - 2), Some(DIRNDL_RED), "Resi there");
        for _ in 0..30 {
            g.step(InputFrame::NONE);
        }
        let fb = rendered(&g, &atlas);
        assert_eq!(fb.get(centred_x(&prompt), PROMPT_Y), Some(NAVY));
    }

    #[test]
    fn teaser_shows_to_be_continued_steadily() {
        let atlas = block_font();
        let mut g = on_map(1);
        assert!(g.route_map().unwrap().teaser);
        g.step(InputFrame::START);
        assert_eq!(g.route_map().unwrap().phase, MapPhase::Teaser);
        let fb = rendered(&g, &atlas);
        let tbc = texts().get(screens::TO_BE_CONTINUED_KEY).into_owned();
        assert_eq!(fb.get(centred_x(&tbc), PROMPT_Y), Some(WHITE));
        let name = texts().get(STATION_KEYS[2]).into_owned();
        assert_eq!(fb.get(centred_x(&name), NAME_Y), Some(MOZART_YELLOW));
        // Teaser text: steady (no blinking).
        for _ in 0..30 {
            g.step(InputFrame::NONE);
        }
        let fb = rendered(&g, &atlas);
        assert_eq!(fb.get(centred_x(&tbc), PROMPT_Y), Some(WHITE));
    }

    #[test]
    fn big_resi_falls_back_to_a_taller_wine_box() {
        // (Map, intro and title draw Resi with `draw_player_pose`.)
        let atlas = block_font();
        let (t, lvl) = (texts(), stage("a"));
        let ctx = RenderContext::new(&atlas, &t, &lvl);
        let mut fb = testutil::fb();
        scene::draw_player_pose(
            &ctx,
            &mut fb,
            Power::Big,
            PlayerAnim::Idle,
            (50, 100),
            0,
            false,
        );
        assert_eq!(fb.get(50, 75), Some(crate::entities::WINE_DARK));
        assert_eq!(fb.get(50, 70), Some(OUTLINE), "30 px tall");
        let mut fb = testutil::fb();
        let hackl = Power::Hackl;
        scene::draw_player_pose(&ctx, &mut fb, hackl, PlayerAnim::Idle, (50, 100), 0, false);
        assert_eq!(
            fb.get(50, 75),
            Some(crate::entities::PINE_DARK),
            "Hackl: green"
        );
        assert_eq!(fb.get(50, 70), Some(OUTLINE), "30 px tall");
        let mut fb = testutil::fb();
        let normal = Power::Normal;
        scene::draw_player_pose(&ctx, &mut fb, normal, PlayerAnim::Idle, (50, 100), 0, false);
        assert_eq!(fb.get(50, 85), Some(DIRNDL_RED));
        assert_eq!(fb.get(50, 78), Some(OUTLINE), "22 px tall");
    }

    #[test]
    fn the_real_atlas_renders_the_map_without_panicking() {
        let g = on_map(0);
        let texts = texts();
        let ctx = RenderContext::for_level(jnr_assets::atlas(), &texts, g.level());
        let mut fb = testutil::fb();
        render(&g, &ctx, &mut fb);
        assert!(ctx.is_for(g.level()));
        assert!(!ctx.is_for(&stage("zz")));
        // Without the art: the meadow fill; with it: the backdrop covers it.
        let corner = fb.get(316, 4).unwrap();
        assert!(ctx.map_backdrop.is_some() || corner == MEADOW);
    }
}
