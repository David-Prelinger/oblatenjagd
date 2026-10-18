//! Drawing a whole [`Game`]: background, tiles, goal, player, stage text,
//! plus the opt-in debug overlay.
//!
//! Everything works in grey-box mode: whatever sprite the atlas lacks is
//! drawn as flat palette shapes, so levels are playable before the art
//! exists. Draw order follows `docs/architecture.md` (backgrounds → tiles →
//! entities → player → text).

use jnr_artkit::atlas::{Atlas, AtlasAnim, AtlasSprite};
use jnr_core::level::TILE_PX;
use jnr_core::physics::Aabb;
use jnr_core::tuning::PlayerTuning;
use jnr_core::{Camera, EntityKind, Facing, Fx, Game, Mode, Player, TileKind};

use crate::Framebuffer;
use crate::blit::{blit_frame, draw_sprite};
use crate::font::Font;
use crate::palette::{
    DIRNDL_RED, LEATHER_DARK, MEADOW, MOZART_YELLOW, OUTLINE, SKY_LIGHT, STONE_GREY, WHITE,
};

/// Shown centred on the screen in `StageClear`.
///
/// TODO(M1 HUD): player-visible text must come from `content/text/de.toml`
/// (keys only in code). This M0 placeholder moves there with the HUD task.
pub const STAGE_CLEAR_TEXT: &str = "ETAPPE GESCHAFFT!";

/// Sprite name of the player.
pub const PLAYER_SPRITE: &str = "player";
/// Sprite name of the goal.
pub const GOAL_SPRITE: &str = "goal";
/// Prefix of tile sprites: a tile named `ground` in the legend uses `tile_ground`.
pub const TILE_SPRITE_PREFIX: &str = "tile_";

/// The player blinks during `Dying`: visible for this many frames, then hidden as long.
const DYING_FLASH_FRAMES: u32 = 4;
/// Height of the grey-box goal pole in pixels.
const GOAL_POLE_PX: i32 = 8 * TILE_PX;
/// Height of a grey-box one-way platform in pixels (from the tile top).
const ONEWAY_PX: i32 = 4;

/// One background layer, far to near.
///
/// The hook for parallax: M0 only has the flat sky. Later variants (one per
/// id in `level.meta.background`, scrolled by a fraction of the camera) are
/// added here and to [`background_layers`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layer {
    /// Flat fill in palette `s`.
    Sky,
}

/// The background layers of a game, far to near.
pub fn background_layers(_game: &Game) -> &'static [Layer] {
    &[Layer::Sky]
}

/// Draws one frame of the game. Deterministic: the same game state and
/// atlas give identical pixels.
pub fn render(game: &Game, atlas: &Atlas, fb: &mut Framebuffer) {
    let cam = game.camera();
    for &layer in background_layers(game) {
        draw_layer(layer, fb);
    }
    draw_tiles(game, atlas, fb, cam);
    draw_goals(game, atlas, fb, cam);
    draw_player(game, atlas, fb, cam);
    if game.mode() == Mode::StageClear {
        let font = Font::from_atlas(atlas);
        let w = font.measure(STAGE_CLEAR_TEXT);
        let x = (fb.w as i32 - w) / 2;
        let y = fb.h as i32 / 2 - 4;
        font.draw_text(fb, atlas, STAGE_CLEAR_TEXT, x, y, WHITE, Some(OUTLINE));
    }
}

/// Debug overlay (opt-in, drawn after [`render`]): 1 px outlines of the
/// player hitbox (`Y`) and goal boxes (`g`), and a line of text with the
/// frame number and the player position in whole pixels.
pub fn render_debug(game: &Game, atlas: &Atlas, fb: &mut Framebuffer) {
    let cam = game.camera();
    for g in game.goals() {
        outline_aabb(fb, g, cam, MEADOW);
    }
    let p = game.player();
    outline_aabb(fb, &p.hitbox(), cam, MOZART_YELLOW);
    let text = format!("F:{} X:{} Y:{}", game.frame(), p.x.to_px(), p.y.to_px());
    Font::from_atlas(atlas).draw_text(fb, atlas, &text, 2, 2, WHITE, Some(OUTLINE));
}

fn draw_layer(layer: Layer, fb: &mut Framebuffer) {
    match layer {
        Layer::Sky => fb.clear(SKY_LIGHT),
    }
}

/// Tiles inside the camera view: the `tile_<name>` sprite if the atlas has
/// it, else a grey-box shape per [`TileKind`].
fn draw_tiles(game: &Game, atlas: &Atlas, fb: &mut Framebuffer, cam: Camera) {
    let level = game.level();
    // One lookup per tile name (index 0 is "no name").
    let sprites: Vec<Option<&AtlasSprite>> = level
        .tile_names
        .iter()
        .map(|n| {
            if n.is_empty() {
                None
            } else {
                atlas
                    .sprite(&format!("{TILE_SPRITE_PREFIX}{n}"))
                    .filter(|s| !s.anims.is_empty())
            }
        })
        .collect();

    let col0 = cam.x.div_euclid(TILE_PX).max(0);
    let col1 = (cam.x + fb.w as i32 - 1)
        .div_euclid(TILE_PX)
        .min(level.width - 1);
    let row0 = cam.y.div_euclid(TILE_PX).max(0);
    let row1 = (cam.y + fb.h as i32 - 1)
        .div_euclid(TILE_PX)
        .min(level.height - 1);
    for row in row0..=row1 {
        for col in col0..=col1 {
            let Some(tile) = level.tile(col, row) else {
                continue;
            };
            let x = col * TILE_PX - cam.x;
            let y = row * TILE_PX - cam.y;
            match sprites.get(tile.name as usize).copied().flatten() {
                Some(s) => {
                    let anim = &s.anims[0];
                    let rect = anim.frame(anim.frame_index_at(game.frame()), false);
                    blit_frame(fb, atlas, rect, x, y, false);
                }
                None => match tile.kind {
                    TileKind::Empty => {}
                    TileKind::Solid => fb.fill_rect(x, y, TILE_PX, TILE_PX, STONE_GREY),
                    TileKind::OneWay => fb.fill_rect(x, y, TILE_PX, ONEWAY_PX, LEATHER_DARK),
                    TileKind::Hazard => fb.fill_rect(x, y, TILE_PX, TILE_PX, DIRNDL_RED),
                },
            }
        }
    }
}

/// Goals: sprite `goal` with its origin at the bottom centre of the goal
/// cell, else a grey-box pole (2 px `Y`) with a small `r` flag.
fn draw_goals(game: &Game, atlas: &Atlas, fb: &mut Framebuffer, cam: Camera) {
    let sprite = atlas.sprite(GOAL_SPRITE).filter(|s| !s.anims.is_empty());
    for e in &game.level().entities {
        if e.kind != EntityKind::Goal {
            continue;
        }
        let foot_x = e.col * TILE_PX + TILE_PX / 2 - cam.x;
        let foot_y = (e.row + 1) * TILE_PX - cam.y;
        match sprite {
            Some(s) => draw_sprite(
                fb,
                atlas,
                s,
                &s.anims[0],
                game.frame(),
                foot_x,
                foot_y,
                false,
            ),
            None => {
                let top = foot_y - GOAL_POLE_PX;
                fb.fill_rect(foot_x - 1, top, 2, GOAL_POLE_PX, MOZART_YELLOW);
                fb.fill_rect(foot_x - 9, top + 2, 8, 6, DIRNDL_RED);
                fb.outline_rect(foot_x - 9, top + 2, 8, 6, OUTLINE);
            }
        }
    }
}

/// The player: sprite `player` with its origin at the feet (bottom centre
/// of the hitbox, D-011), else the hitbox as an `r` box with a `k` outline.
/// Blinks while dying.
fn draw_player(game: &Game, atlas: &Atlas, fb: &mut Framebuffer, cam: Camera) {
    if let Mode::Dying { .. } = game.mode() {
        if (game.frame() / DYING_FLASH_FRAMES) % 2 == 1 {
            return;
        }
    }
    let p = game.player();
    match atlas.sprite(PLAYER_SPRITE).filter(|s| !s.anims.is_empty()) {
        Some(s) => {
            let feet_x = (p.x + p.w.div_int(2)).to_px() - cam.x;
            let feet_y = (p.y + p.h).to_px() - cam.y;
            let anim = player_anim(s, p, &game.tuning().player);
            let left = p.facing == Facing::Left;
            draw_sprite(fb, atlas, s, anim, game.frame(), feet_x, feet_y, left);
        }
        None => {
            let (x, y) = (p.x.to_px() - cam.x, p.y.to_px() - cam.y);
            let (w, h) = (p.w.to_px(), p.h.to_px());
            fb.fill_rect(x, y, w, h, DIRNDL_RED);
            fb.outline_rect(x, y, w, h, OUTLINE);
        }
    }
}

/// The player animation for the current state (idle, walk, run, jump,
/// fall): the first of the wanted names the sprite has, else `idle`, else
/// its first anim. The sprite must have at least one anim.
fn player_anim<'a>(s: &'a AtlasSprite, p: &Player, t: &PlayerTuning) -> &'a AtlasAnim {
    let wanted: &[&str] = if !p.on_ground {
        if p.vy < Fx::ZERO {
            &["jump", "fall"]
        } else {
            &["fall", "jump"]
        }
    } else if p.vx == Fx::ZERO {
        &["idle"]
    } else if p.vx.abs() > t.walk_speed {
        &["run", "walk"]
    } else {
        &["walk", "run"]
    };
    wanted
        .iter()
        .chain(&["idle"])
        .find_map(|name| s.anim(name))
        .unwrap_or(&s.anims[0])
}

fn outline_aabb(fb: &mut Framebuffer, b: &Aabb, cam: Camera, rgba: u32) {
    fb.outline_rect(
        b.x.to_px().saturating_sub(cam.x),
        b.y.to_px().saturating_sub(cam.y),
        b.w.to_px(),
        b.h.to_px(),
        rgba,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::font::{CHARSET, FONT_SPRITE, GLYPH_ANIM};
    use crate::testutil::{self, A, B, image};
    use jnr_core::{InputFrame, Level, Tuning};

    fn tuning() -> Tuning {
        Tuning::parse(include_str!("../../../content/tuning.toml")).unwrap()
    }

    fn level(grid: &str) -> Level {
        let text = format!(
            "[meta]\nid = \"tiny\"\n\n[legend]\n\"#\" = \"solid\"\n\"-\" = \"oneway\"\n\
             \"^\" = \"hazard\"\n\"P\" = {{ entity = \"player_start\" }}\n\
             \"G\" = {{ entity = \"goal\" }}\n\n[map]\ngrid = '''\n{grid}'''\n"
        );
        Level::parse(&text).unwrap()
    }

    /// 20x10 tiles (320x160 px): the camera stays at (0, 0).
    fn tiny_game() -> Game {
        let grid = "\
....................
....................
....................
....................
....................
....................
....................
....................
..P.......-.....G...
########^###########
";
        Game::new(&level(grid), &tuning(), 1)
    }

    fn empty_atlas() -> Atlas {
        testutil::atlas(&[])
    }

    fn rendered(game: &Game, atlas: &Atlas) -> Framebuffer {
        let mut fb = Framebuffer::new();
        render(game, atlas, &mut fb);
        fb
    }

    #[test]
    fn grey_box_scene_pixels() {
        let game = tiny_game();
        assert_eq!(game.camera(), Camera { x: 0, y: 0 });
        let fb = rendered(&game, &empty_atlas());
        // Sky above everything and below the 160 px tall level.
        assert_eq!(fb.get(0, 0), Some(SKY_LIGHT));
        assert_eq!(fb.get(5, 170), Some(SKY_LIGHT));
        // Solid ground row 9 (y 144..160), hazard in column 8.
        assert_eq!(fb.get(0, 150), Some(STONE_GREY));
        assert_eq!(fb.get(8 * 16 + 5, 150), Some(DIRNDL_RED));
        // One-way in column 10, row 8 (y 128): top 4 px only.
        assert_eq!(fb.get(165, 128), Some(LEATHER_DARK));
        assert_eq!(fb.get(165, 131), Some(LEATHER_DARK));
        assert_eq!(fb.get(165, 132), Some(SKY_LIGHT));
        // Player hitbox 12x22 centred in column 2, feet on y 144: x 34..46, y 122..144.
        assert_eq!(fb.get(34, 122), Some(OUTLINE));
        assert_eq!(fb.get(45, 143), Some(OUTLINE));
        assert_eq!(fb.get(40, 130), Some(DIRNDL_RED));
        assert_eq!(fb.get(33, 130), Some(SKY_LIGHT));
        assert_eq!(fb.get(46, 130), Some(SKY_LIGHT));
        // Goal pole in column 16: x 263..265, from y 16 down to 144.
        assert_eq!(fb.get(263, 140), Some(MOZART_YELLOW));
        assert_eq!(fb.get(264, 16), Some(MOZART_YELLOW));
        assert_eq!(fb.get(265, 140), Some(SKY_LIGHT));
    }

    #[test]
    fn sprites_replace_grey_box_shapes() {
        let atlas = testutil::atlas(&[
            // 4x4 player, origin bottom centre (2, 4).
            testutil::sprite(
                PLAYER_SPRITE,
                (2, 4),
                true,
                vec![("idle", 0, vec![image(&["aaaa"; 4])])],
            ),
            testutil::sprite(
                "tile_solid",
                (0, 0),
                false,
                vec![("tile", 0, vec![image(&["bbbbbbbbbbbbbbbb"; 16])])],
            ),
        ]);
        let fb = rendered(&tiny_game(), &atlas);
        // Feet at (40, 144) -> sprite covers x 38..42, y 140..144.
        assert_eq!(fb.get(38, 140), Some(A));
        assert_eq!(fb.get(41, 143), Some(A));
        assert_eq!(fb.get(37, 140), Some(SKY_LIGHT));
        assert_eq!(fb.get(34, 122), Some(SKY_LIGHT), "no grey-box player");
        assert_eq!(fb.get(0, 150), Some(B));
        // Hazards have no sprite here and stay grey-box.
        assert_eq!(fb.get(8 * 16 + 5, 150), Some(DIRNDL_RED));
    }

    #[test]
    fn player_anim_follows_state() {
        let atlas = testutil::atlas(&[testutil::sprite(
            PLAYER_SPRITE,
            (0, 0),
            false,
            vec![
                ("idle", 0, vec![image(&["a"])]),
                ("walk", 0, vec![image(&["a"])]),
                ("fall", 0, vec![image(&["a"])]),
            ],
        )]);
        let s = atlas.sprite(PLAYER_SPRITE).unwrap();
        let t = tuning().player;
        let p = *tiny_game().player();
        let name = |p: &Player| player_anim(s, p, &t).name.clone();
        assert_eq!(name(&p), "idle");
        let walking = Player {
            vx: t.walk_speed,
            ..p
        };
        assert_eq!(name(&walking), "walk");
        let running = Player {
            vx: -t.run_speed,
            ..p
        };
        assert_eq!(name(&running), "walk", "no `run` anim: falls back to walk");
        let rising = Player {
            on_ground: false,
            vy: Fx(-100),
            ..p
        };
        assert_eq!(name(&rising), "fall", "no `jump` anim: falls back to fall");
        let only_first = testutil::atlas(&[testutil::sprite(
            PLAYER_SPRITE,
            (0, 0),
            false,
            vec![("stand", 0, vec![image(&["a"])])],
        )]);
        let s = only_first.sprite(PLAYER_SPRITE).unwrap();
        assert_eq!(player_anim(s, &rising, &t).name, "stand");
    }

    #[test]
    fn dying_player_flashes_every_four_frames() {
        let mut game = tiny_game();
        let atlas = empty_atlas();
        let mut guard = 0;
        while !matches!(game.mode(), Mode::Dying { .. }) {
            game.step(InputFrame::RIGHT);
            guard += 1;
            assert!(guard < 600, "the player should walk into the hazard pit");
        }
        let mut seen = [false; 2];
        for _ in 0..16 {
            let p = *game.player();
            let (x, y) = (p.x.to_px(), p.y.to_px());
            let visible = rendered(&game, &atlas).get(x, y) == Some(OUTLINE);
            assert_eq!(
                visible,
                (game.frame() / 4).is_multiple_of(2),
                "frame {}",
                game.frame()
            );
            seen[visible as usize] = true;
            game.step(InputFrame::NONE);
        }
        assert_eq!(seen, [true, true]);
    }

    #[test]
    fn stage_clear_shows_centred_text() {
        // Every glyph a full 8x8 block: advance 9, space 3.
        let glyphs = CHARSET.chars().map(|_| image(&["aaaaaaaa"; 8])).collect();
        let atlas = testutil::atlas(&[testutil::sprite(
            FONT_SPRITE,
            (0, 0),
            false,
            vec![(GLYPH_ANIM, 0, glyphs)],
        )]);
        let grid = "\
....................
..PG................
####################
";
        let mut game = Game::new(&level(grid), &tuning(), 1);
        let before = rendered(&game, &atlas);
        for _ in 0..120 {
            game.step(InputFrame::RIGHT);
        }
        assert_eq!(game.mode(), Mode::StageClear);
        let fb = rendered(&game, &atlas);
        // "ETAPPE GESCHAFFT!": 16 glyphs * 9 + 3 = 147 px -> x 86, y 86.
        assert_eq!(Font::from_atlas(&atlas).measure(STAGE_CLEAR_TEXT), 147);
        assert_eq!(fb.get(86, 86), Some(WHITE));
        assert_eq!(fb.get(85, 86), Some(SKY_LIGHT));
        assert_eq!(fb.get(94, 94), Some(OUTLINE), "shadow at +1,+1");
        assert_eq!(before.get(86, 86), Some(SKY_LIGHT));
    }

    #[test]
    fn debug_overlay_outlines_hitbox() {
        let game = tiny_game();
        let atlas = empty_atlas();
        let mut fb = rendered(&game, &atlas);
        render_debug(&game, &atlas, &mut fb);
        assert_eq!(fb.get(34, 122), Some(MOZART_YELLOW));
        assert_eq!(fb.get(40, 130), Some(DIRNDL_RED), "outline only");
        // Goal box column 16 (x 256..272): its left edge is visible.
        assert_eq!(fb.get(256, 100), Some(MEADOW));
    }

    #[test]
    fn render_is_deterministic() {
        let inputs = [InputFrame::RIGHT, InputFrame::RIGHT.with(InputFrame::JUMP)];
        let run = || {
            let mut game = tiny_game();
            let atlas = empty_atlas();
            let mut frames = Vec::new();
            for i in 0..90 {
                game.step(inputs[(i / 20) % 2]);
                let mut fb = rendered(&game, &atlas);
                render_debug(&game, &atlas, &mut fb);
                frames.push(fb);
            }
            frames
        };
        let (a, b) = (run(), run());
        assert!(a == b);
        let game = tiny_game();
        assert_eq!(
            rendered(&game, &empty_atlas()),
            rendered(&game, &empty_atlas())
        );
    }

    #[test]
    fn renders_the_testbed_with_the_real_atlas() {
        let level = Level::parse(include_str!("../../../content/levels/00_testbed.level")).unwrap();
        let mut game = Game::new(&level, &tuning(), 1);
        let atlas = jnr_assets::atlas();
        let mut fb = Framebuffer::new();
        for i in 0..240u32 {
            let input = if i % 40 < 30 {
                InputFrame::RIGHT.with(InputFrame::RUN)
            } else {
                InputFrame::RIGHT.with(InputFrame::JUMP)
            };
            game.step(input);
            render(&game, atlas, &mut fb);
            render_debug(&game, atlas, &mut fb);
        }
        assert!(fb.pixels.contains(&SKY_LIGHT));
    }
}
