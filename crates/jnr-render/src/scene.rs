//! Drawing a whole [`Game`]: background, tiles, goal, player, stage text,
//! plus the opt-in debug overlay.
//!
//! Everything works in grey-box mode: whatever sprite the atlas lacks is
//! drawn as flat palette shapes, so levels are playable before the art
//! exists. Draw order follows `docs/architecture.md` (backgrounds → tiles →
//! entities → player → text).
//!
//! All lookups (font, texts, tile/player/goal sprites, background layers)
//! are resolved once in a [`RenderContext`]; drawing a frame allocates
//! nothing (the debug overlay formats its one line of text). Backgrounds
//! live in [`crate::background`], tiles (incl. autotiling) in
//! [`crate::tiles`].

use jnr_artkit::atlas::{Atlas, AtlasAnim, AtlasSprite};
use jnr_core::level::TILE_PX;
use jnr_core::physics::Aabb;
use jnr_core::tuning::PlayerTuning;
use jnr_core::{Camera, Facing, Fx, Game, Level, Mode, Player, Power, SpawnKind, Texts};

use crate::background::{self, BackgroundLayer};
use crate::blit::draw_sprite;
use crate::entities::{self, EntityLooks};
use crate::font::Font;
use crate::palette::{DIRNDL_RED, MEADOW, MOZART_YELLOW, OUTLINE, WHITE};
use crate::tiles::{self, TileLook};
use crate::{Framebuffer, HEIGHT};

/// Text key shown centred on the screen in `StageClear` (`content/text/de.toml`).
pub const STAGE_CLEAR_KEY: &str = "stage.clear";

/// Sprite name of the player.
pub const PLAYER_SPRITE: &str = "player";
/// Sprite name of the Zuckerschock player.
pub const PLAYER_BIG_SPRITE: &str = "player_big";
/// Sprite name of the goal.
pub const GOAL_SPRITE: &str = "goal";
/// Prefix of tile sprites: a tile named `ground` in the legend uses `tile_ground`.
pub const TILE_SPRITE_PREFIX: &str = "tile_";

/// The player blinks during `Dying` and while invulnerable, and alternates
/// sizes while transforming: this many frames per phase.
const DYING_FLASH_FRAMES: u32 = 4;
/// Height of the grey-box goal pole in pixels.
const GOAL_POLE_PX: i32 = 8 * TILE_PX;

/// Which player animation a state shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlayerAnim {
    /// Standing still on the ground.
    Idle,
    /// Moving on the ground up to walk speed.
    Walk,
    /// Moving on the ground faster than walk speed.
    Run,
    /// In the air, moving up.
    Jump,
    /// In the air, moving down.
    Fall,
}

impl PlayerAnim {
    /// Every state, in the order of [`PlayerAnim::index`].
    pub const ALL: [PlayerAnim; 5] = [
        PlayerAnim::Idle,
        PlayerAnim::Walk,
        PlayerAnim::Run,
        PlayerAnim::Jump,
        PlayerAnim::Fall,
    ];

    /// The state of a player.
    pub fn of(p: &Player, t: &PlayerTuning) -> PlayerAnim {
        if !p.on_ground {
            if p.vy < Fx::ZERO {
                PlayerAnim::Jump
            } else {
                PlayerAnim::Fall
            }
        } else if p.vx == Fx::ZERO {
            PlayerAnim::Idle
        } else if p.vx.abs() > t.walk_speed {
            PlayerAnim::Run
        } else {
            PlayerAnim::Walk
        }
    }

    /// Position in [`PlayerAnim::ALL`].
    pub const fn index(self) -> usize {
        match self {
            PlayerAnim::Idle => 0,
            PlayerAnim::Walk => 1,
            PlayerAnim::Run => 2,
            PlayerAnim::Jump => 3,
            PlayerAnim::Fall => 4,
        }
    }

    /// Anim names to try, best first (then `idle`, then the sprite's first anim).
    fn names(self) -> &'static [&'static str] {
        match self {
            PlayerAnim::Idle => &["idle"],
            PlayerAnim::Walk => &["walk", "run"],
            PlayerAnim::Run => &["run", "walk"],
            PlayerAnim::Jump => &["jump", "fall"],
            PlayerAnim::Fall => &["fall", "jump"],
        }
    }
}

/// The player sprite with one resolved anim per [`PlayerAnim`].
#[derive(Clone, Debug)]
struct PlayerLook<'a> {
    sprite: &'a AtlasSprite,
    anims: [&'a AtlasAnim; 5],
}

/// Everything the renderer looks up, resolved once: the atlas, the font,
/// the texts, the tile/player/goal sprites and the background layers.
/// Build it at startup (and again when the level changes: tile sprites are
/// indexed by the level's `tile_names`) and pass it to [`render`] and
/// [`render_debug`].
#[derive(Clone, Debug)]
pub struct RenderContext<'a> {
    atlas: &'a Atlas,
    texts: &'a Texts,
    font: Font,
    /// `tile_<name>` look per [`Level::tile_names`] index (index 0 = no name).
    tiles: Vec<Option<TileLook<'a>>>,
    player: Option<PlayerLook<'a>>,
    player_big: Option<PlayerLook<'a>>,
    goal: Option<&'a AtlasSprite>,
    entities: EntityLooks<'a>,
    /// Parallax layers, back to front (empty = flat sky only).
    background: Vec<BackgroundLayer<'a>>,
    /// True if the first layer hides the whole screen (no flat sky fill).
    background_covers: bool,
}

impl<'a> RenderContext<'a> {
    /// Resolves all lookups for `level` in `atlas`. Sprites without anims
    /// count as missing (grey-box).
    pub fn new(atlas: &'a Atlas, texts: &'a Texts, level: &Level) -> RenderContext<'a> {
        let with_anims = |name: &str| atlas.sprite(name).filter(|s| !s.anims.is_empty());
        let tiles = level
            .tile_names
            .iter()
            .map(|n| {
                if n.is_empty() {
                    None
                } else {
                    TileLook::resolve(atlas, &format!("{TILE_SPRITE_PREFIX}{n}"))
                }
            })
            .collect();
        let look = |name: &str| {
            with_anims(name).map(|sprite| PlayerLook {
                sprite,
                anims: PlayerAnim::ALL.map(|state| {
                    state
                        .names()
                        .iter()
                        .chain(&["idle"])
                        .find_map(|name| sprite.anim(name))
                        .unwrap_or(&sprite.anims[0])
                }),
            })
        };
        RenderContext {
            atlas,
            texts,
            font: Font::from_atlas(atlas),
            tiles,
            player: look(PLAYER_SPRITE),
            player_big: look(PLAYER_BIG_SPRITE),
            goal: with_anims(GOAL_SPRITE),
            entities: EntityLooks::new(atlas),
            background: Vec::new(),
            background_covers: false,
        }
    }

    /// Sets the parallax background: `(sprite name, scroll factor in
    /// sixteenths)` pairs, back to front (e.g. `[("bg_sky_salzburg", 0),
    /// ("bg_untersberg", 3)]`). Sprites the atlas lacks are skipped; without
    /// layers the flat sky is drawn. See [`crate::background`] for placement.
    pub fn with_background(mut self, layers: &[(&str, u32)]) -> RenderContext<'a> {
        self.background = background::resolve(self.atlas, layers);
        self.background_covers = self
            .background
            .first()
            .is_some_and(|l| background::covers_screen(self.atlas, l, HEIGHT as u32));
        self
    }

    /// The resolved background layers, back to front.
    pub fn background(&self) -> &[BackgroundLayer<'a>] {
        &self.background
    }

    /// The atlas everything is drawn from.
    pub fn atlas(&self) -> &'a Atlas {
        self.atlas
    }

    /// The font (empty if the atlas has none).
    pub fn font(&self) -> &Font {
        &self.font
    }

    /// The player-visible texts.
    pub fn texts(&self) -> &'a Texts {
        self.texts
    }

    /// The resolved player anim for a state, if the atlas has a player sprite.
    pub fn player_anim(&self, state: PlayerAnim) -> Option<&'a AtlasAnim> {
        self.player.as_ref().map(|look| look.anims[state.index()])
    }
}

/// Draws one frame of the game. Deterministic: the same game state and
/// context give identical pixels.
pub fn render(game: &Game, ctx: &RenderContext, fb: &mut Framebuffer) {
    let cam = game.camera();
    let (atlas, tick) = (ctx.atlas, game.frame());
    background::draw(
        fb,
        atlas,
        &ctx.background,
        ctx.background_covers,
        cam.x,
        tick,
    );
    entities::draw_behind_tiles(game, &ctx.entities, ctx.atlas, fb);
    tiles::draw_tiles(fb, atlas, &ctx.tiles, game, cam, tick);
    draw_goals(game, ctx, fb, cam);
    entities::draw_entities(game, &ctx.entities, ctx.atlas, &ctx.font, ctx.texts, fb);
    draw_player(game, ctx, fb, cam);
    entities::draw_front(game, &ctx.entities, ctx.atlas, &ctx.font, ctx.texts, fb);
    if game.mode() == Mode::StageClear {
        let text = ctx.texts.get(STAGE_CLEAR_KEY);
        let w = ctx.font.measure(&text);
        let x = (fb.w as i32 - w) / 2;
        let y = fb.h as i32 / 2 - 4;
        ctx.font
            .draw_text(fb, ctx.atlas, &text, x, y, WHITE, Some(OUTLINE));
    }
}

/// Debug overlay (opt-in, drawn after [`render`]): 1 px outlines of the
/// player hitbox (`Y`) and goal boxes (`g`), and a line of text with the
/// frame number and the player position in whole pixels.
pub fn render_debug(game: &Game, ctx: &RenderContext, fb: &mut Framebuffer) {
    let cam = game.camera();
    for g in game.goals() {
        outline_aabb(fb, g, cam, MEADOW);
    }
    let p = game.player();
    outline_aabb(fb, &p.hitbox(), cam, MOZART_YELLOW);
    let text = format!("F:{} X:{} Y:{}", game.frame(), p.x.to_px(), p.y.to_px());
    ctx.font
        .draw_text(fb, ctx.atlas, &text, 2, 2, WHITE, Some(OUTLINE));
}

/// Goals: sprite `goal` with its origin at the bottom centre of the goal
/// cell, else a grey-box pole (2 px `Y`) with a small `r` flag.
fn draw_goals(game: &Game, ctx: &RenderContext, fb: &mut Framebuffer, cam: Camera) {
    for e in &game.level().entities {
        if e.kind != SpawnKind::Goal {
            continue;
        }
        let foot_x = e.col * TILE_PX + TILE_PX / 2 - cam.x;
        let foot_y = (e.row + 1) * TILE_PX - cam.y;
        match ctx.goal {
            Some(s) => {
                let anim = &s.anims[0];
                draw_sprite(fb, ctx.atlas, s, anim, game.frame(), foot_x, foot_y, false);
            }
            None => {
                let top = foot_y - GOAL_POLE_PX;
                fb.fill_rect(foot_x - 1, top, 2, GOAL_POLE_PX, MOZART_YELLOW);
                fb.fill_rect(foot_x - 9, top + 2, 8, 6, DIRNDL_RED);
                fb.outline_rect(foot_x - 9, top + 2, 8, 6, OUTLINE);
            }
        }
    }
}

/// Whether the player is drawn big this frame: the power state, except
/// while transforming, when the old and new size alternate every 4 frames
/// (starting with the old one).
pub fn player_shown_big(p: &Player, t: &PlayerTuning) -> bool {
    let big = p.power == Power::Big;
    if p.transform == 0 {
        return big;
    }
    let elapsed = u32::from(t.transform_frames.saturating_sub(p.transform));
    let show_new = (elapsed / DYING_FLASH_FRAMES) % 2 == 1;
    if show_new { big } else { !big }
}

/// The player: sprite `player` (Normal) or `player_big` (Zuckerschock,
/// falls back to `player`) with its origin at the feet (bottom centre of
/// the hitbox, D-011), else the hitbox as an `r` box (`R` when big) with a
/// `k` outline. Blinks while dying (counted from the moment of death) and
/// while invulnerable; alternates sizes while transforming.
fn draw_player(game: &Game, ctx: &RenderContext, fb: &mut Framebuffer, cam: Camera) {
    if let Mode::Dying { timer } = game.mode() {
        let elapsed = u32::from(game.tuning().game.death_frames.saturating_sub(timer));
        if (elapsed / DYING_FLASH_FRAMES) % 2 == 1 {
            return;
        }
    }
    let p = game.player();
    let t = &game.tuning().player;
    if p.transform == 0 && (u32::from(p.invulnerable) / DYING_FLASH_FRAMES) % 2 == 1 {
        return;
    }
    let big = player_shown_big(p, t);
    let look = if big {
        ctx.player_big.as_ref().or(ctx.player.as_ref())
    } else {
        ctx.player.as_ref()
    };
    let feet_x = (p.x + p.w.div_int(2)).to_px() - cam.x;
    let feet_y = (p.y + p.h).to_px() - cam.y;
    match look {
        Some(look) => {
            let anim = look.anims[PlayerAnim::of(p, t).index()];
            let left = p.facing == Facing::Left;
            let (atlas, tick) = (ctx.atlas, game.frame());
            draw_sprite(fb, atlas, look.sprite, anim, tick, feet_x, feet_y, left);
        }
        None => {
            let h = if big { t.hitbox_h_big } else { t.hitbox_h };
            let (x, y) = (p.x.to_px() - cam.x, feet_y - h);
            let w = p.w.to_px();
            let fill = if big { entities::WINE_DARK } else { DIRNDL_RED };
            fb.fill_rect(x, y, w, h, fill);
            fb.outline_rect(x, y, w, h, OUTLINE);
        }
    }
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
    use crate::palette::{LEATHER_DARK, SKY_LIGHT, STONE_GREY};
    use crate::testutil::{self, A, B, C, image};
    use jnr_core::{InputFrame, Tuning};

    fn tuning() -> Tuning {
        Tuning::parse(include_str!("../../../content/tuning.toml")).unwrap()
    }

    fn texts() -> Texts {
        Texts::parse(include_str!("../../../content/text/de.toml")).unwrap()
    }

    fn level(grid: &str) -> Level {
        let text = format!(
            "[meta]\nid = \"tiny\"\ntime_limit = 300\n\n[legend]\n\"#\" = \"solid\"\n\"-\" = \"oneway\"\n\
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
        let texts = texts();
        let ctx = RenderContext::new(atlas, &texts, game.level());
        let mut fb = Framebuffer::new();
        render(game, &ctx, &mut fb);
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
    fn background_layers_replace_the_flat_sky_behind_the_tiles() {
        // 8x180 opaque sky column `c`, a 4x2 strip `ab..` on top of it.
        let sky: Vec<&str> = vec!["cccccccc"; HEIGHT];
        let atlas = testutil::atlas(&[
            testutil::sprite(
                "bg_sky",
                (0, 0),
                false,
                vec![("idle", 0, vec![image(&sky)])],
            ),
            testutil::sprite(
                "bg_hills",
                (0, 0),
                false,
                vec![("idle", 0, vec![image(&["ab..", "ab.."])])],
            ),
        ]);
        let texts = texts();
        let game = tiny_game();
        let ctx = RenderContext::new(&atlas, &texts, game.level()).with_background(&[
            ("bg_sky", 0),
            ("bg_missing", 2),
            ("bg_hills", 8),
        ]);
        assert_eq!(ctx.background().len(), 2);
        assert!(ctx.background_covers);
        let mut fb = Framebuffer::new();
        render(&game, &ctx, &mut fb);
        assert_eq!(fb.get(0, 0), Some(C), "sky layer instead of the flat sky");
        assert!(!fb.pixels.contains(&SKY_LIGHT));
        // The strip is bottom-aligned (rows 178, 179) and below the level's
        // 160 px, so it shows; its transparent columns show the sky layer.
        assert_eq!(fb.get(4, 179), Some(A));
        assert_eq!(fb.get(5, 178), Some(B));
        assert_eq!(fb.get(6, 179), Some(C));
        // Tiles are drawn over the background (solid ground row y 144..160).
        assert_eq!(fb.get(0, 150), Some(STONE_GREY));

        // Only a partial layer: the flat sky stays behind it.
        let ctx =
            RenderContext::new(&atlas, &texts, game.level()).with_background(&[("bg_hills", 8)]);
        assert!(!ctx.background_covers);
        let mut fb = Framebuffer::new();
        render(&game, &ctx, &mut fb);
        assert_eq!(fb.get(0, 0), Some(SKY_LIGHT));
        assert_eq!(fb.get(0, 179), Some(A));
    }

    #[test]
    fn context_resolves_player_anims_with_fallbacks() {
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
        let texts = Texts::default();
        let game = tiny_game();
        let ctx = RenderContext::new(&atlas, &texts, game.level());
        let name = |state| ctx.player_anim(state).unwrap().name.as_str();
        assert_eq!(name(PlayerAnim::Idle), "idle");
        assert_eq!(name(PlayerAnim::Walk), "walk");
        assert_eq!(name(PlayerAnim::Run), "walk", "no `run` anim");
        assert_eq!(name(PlayerAnim::Jump), "fall", "no `jump` anim");
        assert_eq!(name(PlayerAnim::Fall), "fall");

        let only_first = testutil::atlas(&[testutil::sprite(
            PLAYER_SPRITE,
            (0, 0),
            false,
            vec![("stand", 0, vec![image(&["a"])])],
        )]);
        let ctx = RenderContext::new(&only_first, &texts, game.level());
        assert_eq!(ctx.player_anim(PlayerAnim::Jump).unwrap().name, "stand");

        let ctx = RenderContext::new(&atlas, &texts, game.level());
        assert!(ctx.tiles.iter().all(Option::is_none));
        assert!(ctx.font().is_empty());
        let none = empty_atlas();
        assert!(
            RenderContext::new(&none, &texts, game.level())
                .player_anim(PlayerAnim::Idle)
                .is_none()
        );
    }

    #[test]
    fn player_state_picks_anim() {
        let t = tuning().player;
        let p = *tiny_game().player();
        assert_eq!(PlayerAnim::of(&p, &t), PlayerAnim::Idle);
        let walking = Player {
            vx: t.walk_speed,
            ..p
        };
        assert_eq!(PlayerAnim::of(&walking, &t), PlayerAnim::Walk);
        let running = Player {
            vx: -t.run_speed,
            ..p
        };
        assert_eq!(PlayerAnim::of(&running, &t), PlayerAnim::Run);
        let rising = Player {
            on_ground: false,
            vy: Fx(-100),
            ..p
        };
        assert_eq!(PlayerAnim::of(&rising, &t), PlayerAnim::Jump);
        let falling = Player {
            on_ground: false,
            vy: Fx(0),
            ..p
        };
        assert_eq!(PlayerAnim::of(&falling, &t), PlayerAnim::Fall);
        for (i, s) in PlayerAnim::ALL.iter().enumerate() {
            assert_eq!(s.index(), i);
        }
    }

    #[test]
    fn dying_player_blinks_from_the_moment_of_death() {
        let mut game = tiny_game();
        let atlas = empty_atlas();
        let mut guard = 0;
        while !matches!(game.mode(), Mode::Dying { .. }) {
            game.step(InputFrame::RIGHT);
            guard += 1;
            assert!(guard < 600, "the player should walk into the hazard pit");
        }
        let mut seen = Vec::new();
        for _ in 0..16 {
            let p = *game.player();
            let (x, y) = (p.x.to_px(), p.y.to_px());
            seen.push(rendered(&game, &atlas).get(x, y) == Some(OUTLINE));
            game.step(InputFrame::NONE);
        }
        let on = [true; 4];
        let off = [false; 4];
        assert_eq!(seen, [on, off, on, off].concat());
    }

    #[test]
    fn stage_clear_shows_centred_text_from_texts() {
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
        // "Etappe geschafft!": 16 glyphs * 9 + 3 - 1 (no trailing gap) = 146 px
        // -> x (320 - 146) / 2 = 87, y 86.
        let text = texts().get(STAGE_CLEAR_KEY).into_owned();
        assert_eq!(text, "Etappe geschafft!");
        assert_eq!(Font::from_atlas(&atlas).measure(&text), 146);
        assert_eq!(fb.get(87, 86), Some(WHITE));
        assert_eq!(fb.get(86, 86), Some(SKY_LIGHT));
        assert_eq!(fb.get(95, 94), Some(OUTLINE), "shadow at +1,+1");
        assert_eq!(before.get(87, 86), Some(SKY_LIGHT));

        // A missing key shows the bracketed key instead of panicking.
        let empty = Texts::default();
        let ctx = RenderContext::new(&atlas, &empty, game.level());
        let mut fb = Framebuffer::new();
        render(&game, &ctx, &mut fb);
        let w = ctx.font().measure("[stage.clear]");
        assert_eq!(fb.get((320 - w) / 2, 86), Some(WHITE));
    }

    #[test]
    fn debug_overlay_outlines_hitbox() {
        let game = tiny_game();
        let atlas = empty_atlas();
        let texts = texts();
        let ctx = RenderContext::new(&atlas, &texts, game.level());
        let mut fb = Framebuffer::new();
        render(&game, &ctx, &mut fb);
        render_debug(&game, &ctx, &mut fb);
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
            let texts = texts();
            let ctx = RenderContext::new(&atlas, &texts, game.level());
            let mut frames = Vec::new();
            for i in 0..90 {
                game.step(inputs[(i / 20) % 2]);
                let mut fb = Framebuffer::new();
                render(&game, &ctx, &mut fb);
                render_debug(&game, &ctx, &mut fb);
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

    /// Smoke test only (no pixel asserts): `render` and `render_debug` run
    /// with the real embedded atlas (incl. the real font) without panicking
    /// while the player runs and jumps through the testbed.
    #[test]
    fn renders_the_testbed_with_the_real_atlas() {
        let level = Level::parse(include_str!("../../../content/levels/00_testbed.level")).unwrap();
        let mut game = Game::new(&level, &tuning(), 1);
        let texts = texts();
        let ctx = RenderContext::new(jnr_assets::atlas(), &texts, game.level());
        assert!(
            !ctx.font().is_empty(),
            "art/sprites/ui/font.sprite is embedded"
        );
        let mut fb = Framebuffer::new();
        for i in 0..240u32 {
            let input = if i % 40 < 30 {
                InputFrame::RIGHT.with(InputFrame::RUN)
            } else {
                InputFrame::RIGHT.with(InputFrame::JUMP)
            };
            game.step(input);
            render(&game, &ctx, &mut fb);
            render_debug(&game, &ctx, &mut fb);
        }
        assert!(fb.pixels.contains(&SKY_LIGHT));
    }
}
