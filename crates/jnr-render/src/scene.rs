//! Drawing a whole [`Game`]: the full screens of the flow modes (title,
//! stage intro, game over, „Fortsetzung folgt …", see [`crate::screens`];
//! the route map, see [`crate::map`]),
//! else the level (background, tiles, Maibaum, entities, player), the HUD
//! ([`crate::hud`]) and the text overlays; plus the opt-in debug overlay.
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
use jnr_core::{
    Camera, ClearPhase, Facing, Fx, Game, Level, Mode, Player, Power, PropLayer, SpawnKind, Texts,
};

use crate::background::{self, BackgroundLayer};
use crate::blit::{draw_sprite, draw_sprite_frame};
use crate::entities::{self, EntityLooks};
use crate::font::Font;
use crate::palette::{DIRNDL_RED, MEADOW, MOZART_YELLOW, OUTLINE, WHITE};
use crate::tiles::{self, TileLooks};
use crate::{Framebuffer, HEIGHT, hud, map, screens};

/// Sprite name of the player.
pub const PLAYER_SPRITE: &str = "player";
/// Sprite name of the Zuckerschock player.
pub const PLAYER_BIG_SPRITE: &str = "player_big";
/// Sprite name of the Hackl player (D-056).
pub const PLAYER_HACKL_SPRITE: &str = "player_hackl";
/// **Art fallback** while the atlas has no [`PLAYER_HACKL_SPRITE`]: the
/// Hackl Resi is drawn as `player_big` (or `player`) with its dirndl red
/// (`r`) swapped for the loden green (`G`). It goes away by itself once
/// the pixel-artist's `player_hackl` exists (the lookup prefers it).
pub const HACKL_FALLBACK_SWAP: (u32, u32) = (DIRNDL_RED, entities::PINE_DARK);
/// The Maibaum pole, 16×16, repeated upwards from the goal cell's bottom.
pub const MAIBAUM_POLE_SPRITE: &str = "maibaum_pole";
/// The Maibaum top (wreath, ribbons, Wurstkranz), on top of the pole.
pub const MAIBAUM_TOP_SPRITE: &str = "maibaum_top";
/// Prefix of tile sprites: a tile named `ground` in the legend uses `tile_ground`.
pub const TILE_SPRITE_PREFIX: &str = "tile_";

/// The player blinks during `Dying` and while invulnerable, and alternates
/// sizes while transforming: this many frames per phase.
const DYING_FLASH_FRAMES: u32 = 4;

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
    /// On the ground, braking against the motion (holding the other way).
    Skid,
    /// Sliding down the Maibaum (goal sequence; never from [`PlayerAnim::of`]).
    Pole,
}

impl PlayerAnim {
    /// Every state, in the order of [`PlayerAnim::index`].
    pub const ALL: [PlayerAnim; 7] = [
        PlayerAnim::Idle,
        PlayerAnim::Walk,
        PlayerAnim::Run,
        PlayerAnim::Jump,
        PlayerAnim::Fall,
        PlayerAnim::Skid,
        PlayerAnim::Pole,
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
        } else if p.skidding {
            PlayerAnim::Skid
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
            PlayerAnim::Skid => 5,
            PlayerAnim::Pole => 6,
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
            PlayerAnim::Skid => &["skid", "walk"],
            PlayerAnim::Pole => &["pole", "fall"],
        }
    }
}

/// Whether the player sprite is drawn mirrored (facing left): the look
/// direction, except while skidding, when Resi still faces the direction
/// she slides in (sprite catalog: `skid` faces the motion).
pub fn player_faces_left(p: &Player, t: &PlayerTuning) -> bool {
    if PlayerAnim::of(p, t) == PlayerAnim::Skid {
        p.vx < Fx::ZERO
    } else {
        p.facing == Facing::Left
    }
}

/// The player sprite with one resolved anim per [`PlayerAnim`], plus the
/// M3 poses it may lack: `throw` (Hackl; without it the movement anim
/// shows) and `crawl` (without it Resi is hidden while crawling, the D-057
/// fallback).
#[derive(Clone, Debug)]
struct PlayerLook<'a> {
    sprite: &'a AtlasSprite,
    anims: [&'a AtlasAnim; 7],
    throw: Option<&'a AtlasAnim>,
    crawl: Option<&'a AtlasAnim>,
}

/// A level `prop` (D-030/D-031): its sprite (`None` = grey-box outline of
/// its cell), the bottom centre of its cell (moved by its `dx`) in level
/// pixels, and its layer.
#[derive(Clone, Copy, Debug)]
struct PropLook<'a> {
    sprite: Option<&'a AtlasSprite>,
    foot_x: i32,
    foot_y: i32,
    layer: PropLayer,
}

/// Everything the renderer looks up, resolved once: the atlas, the font,
/// the texts, the tile/player/goal sprites and the background layers.
/// Build it at startup (and again when the level changes, e.g. at a stage
/// change: tile sprites are indexed by the level's `tile_names`; see
/// [`RenderContext::is_for`]) and pass it to [`render`] and [`render_debug`].
#[derive(Clone, Debug)]
pub struct RenderContext<'a> {
    /// `meta.id` of the level it was built for.
    level_id: String,
    atlas: &'a Atlas,
    texts: &'a Texts,
    font: Font,
    /// `tile_<name>` look per [`Level::tile_names`] index (index 0 = no name).
    tiles: TileLooks<'a>,
    player: Option<PlayerLook<'a>>,
    player_big: Option<PlayerLook<'a>>,
    player_hackl: Option<PlayerLook<'a>>,
    maibaum_pole: Option<&'a AtlasSprite>,
    maibaum_top: Option<&'a AtlasSprite>,
    entities: EntityLooks<'a>,
    /// The level's props, in spawn order.
    props: Vec<PropLook<'a>>,
    /// `title_logo` (title screen).
    pub(crate) title_logo: Option<&'a AtlasSprite>,
    /// `hud_oblate` (HUD icon).
    pub(crate) hud_oblate: Option<&'a AtlasSprite>,
    /// `map_salzkammergut` (route-map backdrop).
    pub(crate) map_backdrop: Option<&'a AtlasSprite>,
    /// `map_station` (route-map station marker).
    pub(crate) map_station: Option<&'a AtlasSprite>,
    /// The stage's text keys, in [`screens::STAGE_SUFFIXES`] order:
    /// `stage.<level id>.<suffix>` if de.toml has the title, else the
    /// [`screens::FALLBACK_STAGE`]'s; `[meta].title_key` overrides the title.
    pub(crate) stage_keys: [String; 3],
    /// Parallax layers, back to front (empty = flat sky only).
    background: Vec<BackgroundLayer<'a>>,
    /// True if the first layer hides the whole screen (no flat sky fill).
    background_covers: bool,
    /// The camera's lowest y in this level (level height - screen height,
    /// at least 0): where `factor_y` layers sit unmoved (D-054).
    max_camera_y: i32,
}

impl<'a> RenderContext<'a> {
    /// Resolves all lookups for `level` in `atlas`. Sprites without anims
    /// count as missing (grey-box).
    pub fn new(atlas: &'a Atlas, texts: &'a Texts, level: &Level) -> RenderContext<'a> {
        let with_anims = |name: &str| atlas.sprite(name).filter(|s| !s.anims.is_empty());
        let tiles = TileLooks::resolve(atlas, &level.tile_names, TILE_SPRITE_PREFIX);
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
                throw: sprite.anim("throw"),
                crawl: sprite.anim("crawl"),
            })
        };
        let props = level
            .entities
            .iter()
            .filter(|e| e.kind == SpawnKind::Prop)
            .map(|e| PropLook {
                sprite: e.sprite.as_deref().and_then(with_anims),
                foot_x: e.col * TILE_PX + TILE_PX / 2 + e.dx,
                foot_y: (e.row + 1) * TILE_PX,
                layer: e.layer,
            })
            .collect();
        let id = &level.meta.id;
        let stage = if texts.contains(&format!("stage.{id}.title")) {
            id.as_str()
        } else {
            screens::FALLBACK_STAGE
        };
        let mut stage_keys = screens::STAGE_SUFFIXES.map(|s| format!("stage.{stage}.{s}"));
        if let Some(key) = &level.meta.title_key {
            stage_keys[0] = key.clone();
        }
        RenderContext {
            level_id: level.meta.id.clone(),
            atlas,
            texts,
            font: Font::from_atlas(atlas),
            tiles,
            player: look(PLAYER_SPRITE),
            player_big: look(PLAYER_BIG_SPRITE),
            player_hackl: look(PLAYER_HACKL_SPRITE),
            maibaum_pole: with_anims(MAIBAUM_POLE_SPRITE),
            maibaum_top: with_anims(MAIBAUM_TOP_SPRITE),
            entities: EntityLooks::new(atlas),
            props,
            title_logo: with_anims(screens::TITLE_LOGO_SPRITE),
            hud_oblate: with_anims(hud::OBLATE_ICON_SPRITE),
            map_backdrop: with_anims(map::BACKDROP_SPRITE),
            map_station: with_anims(map::STATION_SPRITE),
            stage_keys,
            background: Vec::new(),
            background_covers: false,
            max_camera_y: (level.height * TILE_PX - HEIGHT as i32).max(0),
        }
    }

    /// [`RenderContext::new`] plus the level's `[meta].background` layers.
    pub fn for_level(atlas: &'a Atlas, texts: &'a Texts, level: &Level) -> RenderContext<'a> {
        let layers: Vec<(&str, u32, u32, u32)> = level
            .meta
            .background
            .iter()
            .map(|l| (l.sprite.as_str(), l.factor, l.y, l.factor_y))
            .collect();
        RenderContext::new(atlas, texts, level).with_background(&layers)
    }

    /// Sets the parallax background: `(sprite name, scroll factor in
    /// sixteenths, y offset in px above the screen bottom, vertical factor
    /// in sixteenths)`, back to front (e.g. `[("bg_sky_salzburg", 0, 0, 0),
    /// ("bg_untersberg", 3, 32, 0)]`).
    /// Sprites the atlas lacks are skipped; without layers the flat sky is
    /// drawn. See [`crate::background`] for placement.
    pub fn with_background(mut self, layers: &[(&str, u32, u32, u32)]) -> RenderContext<'a> {
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

    /// Whether this context was built for `level` (by `meta.id`). After a
    /// stage change it isn't: build a new one with
    /// [`RenderContext::for_level`] for `game.level()`.
    pub fn is_for(&self, level: &Level) -> bool {
        self.level_id == level.meta.id
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

    /// The look of a power state and the colour swap to draw it with:
    /// `player`, `player_big` (falls back to `player`), `player_hackl`
    /// (until it exists: the big look with [`HACKL_FALLBACK_SWAP`]).
    fn player_look(&self, power: Power) -> (Option<&PlayerLook<'a>>, Option<(u32, u32)>) {
        let big = || self.player_big.as_ref().or(self.player.as_ref());
        match power {
            Power::Normal => (self.player.as_ref(), None),
            Power::Big => (big(), None),
            Power::Hackl => match &self.player_hackl {
                Some(look) => (Some(look), None),
                None => (big(), Some(HACKL_FALLBACK_SWAP)),
            },
        }
    }

    /// True if the atlas has the real `player_hackl` (no fallback).
    pub fn has_player_hackl(&self) -> bool {
        self.player_hackl.is_some()
    }
}

/// Grey-box player (no sprite at all): hitbox height and fill per state.
fn grey_box_player(power: Power, t: &PlayerTuning) -> (i32, u32) {
    match power {
        Power::Normal => (t.hitbox_h, DIRNDL_RED),
        Power::Big => (t.hitbox_h_big, entities::WINE_DARK),
        Power::Hackl => (t.hitbox_h_big, entities::PINE_DARK),
    }
}

/// Draws frame `index` of `anim` of `sprite` with the optional colour swap.
#[allow(clippy::too_many_arguments)]
fn draw_look(
    fb: &mut Framebuffer,
    atlas: &Atlas,
    sprite: &AtlasSprite,
    anim: &AtlasAnim,
    index: usize,
    (x, y): (i32, i32),
    left: bool,
    swap: Option<(u32, u32)>,
) {
    draw_sprite_frame(fb, atlas, sprite, anim, index, x, y, left, swap);
}

/// The frame of the 2-frame `throw` anim `elapsed` frames into the throw
/// pose: 0 (arm back) for the first [`THROW_WINDUP_FRAMES`], then 1 (sprite
/// catalog: the engine picks the frame).
pub fn throw_frame(elapsed: u32) -> usize {
    usize::from(elapsed >= THROW_WINDUP_FRAMES)
}

/// Frames of the throw pose that show `throw` frame 0 (arm back with the
/// Hackl raised); the rest of `hackl.throw_pose_frames` show frame 1.
pub const THROW_WINDUP_FRAMES: u32 = 4;

/// Draws one frame of the game. Deterministic: the same game state and
/// context give identical pixels.
///
/// Title, stage intro, game over and „Fortsetzung folgt …" are full
/// screens; otherwise the level is drawn, then the HUD, then the overlay
/// texts of the mode (pause: a panel over the frozen level).
pub fn render(game: &Game, ctx: &RenderContext, fb: &mut Framebuffer) {
    let to_be_continued = game
        .stage_clear()
        .is_some_and(|c| c.phase == ClearPhase::ToBeContinued);
    match game.mode() {
        Mode::Title { frames } => return screens::draw_title(game, ctx, fb, frames),
        Mode::StageIntro { .. } => return screens::draw_intro(game, ctx, fb),
        Mode::GameOver { frames } => return screens::draw_game_over(game, ctx, fb, frames),
        Mode::StageClear if to_be_continued => return screens::draw_to_be_continued(ctx, fb),
        Mode::RouteMap => return map::draw_route_map(game, ctx, fb),
        Mode::Playing | Mode::Paused | Mode::Dying { .. } | Mode::StageClear => {}
    }
    let cam = game.camera();
    let (atlas, tick) = (ctx.atlas, game.frame());
    background::draw(
        fb,
        atlas,
        &ctx.background,
        ctx.background_covers,
        (cam.x, cam.y),
        ctx.max_camera_y,
        tick,
    );
    draw_props(ctx, fb, cam, tick, PropLayer::Back);
    entities::draw_behind_tiles(game, &ctx.entities, ctx.atlas, fb);
    tiles::draw_tiles(fb, atlas, &ctx.tiles, game, cam, tick);
    draw_props(ctx, fb, cam, tick, PropLayer::Front);
    draw_goals(game, ctx, fb, cam);
    entities::draw_entities(game, &ctx.entities, ctx.atlas, &ctx.font, ctx.texts, fb);
    draw_player(game, ctx, fb, cam);
    entities::draw_front(game, &ctx.entities, ctx.atlas, &ctx.font, ctx.texts, fb);
    hud::draw_hud(game, ctx, fb);
    screens::draw_overlays(game, ctx, fb);
    if game.mode() == Mode::Paused {
        screens::draw_pause(ctx, fb);
    }
}

/// The title screen's backdrop: the level's start view without entities
/// (background, tiles, Maibaum) and Resi idle where she stands. Static:
/// every animation is drawn at tick 0.
pub(crate) fn draw_backdrop(game: &Game, ctx: &RenderContext, fb: &mut Framebuffer) {
    let cam = game.camera();
    let (atlas, tick) = (ctx.atlas, 0);
    background::draw(
        fb,
        atlas,
        &ctx.background,
        ctx.background_covers,
        (cam.x, cam.y),
        ctx.max_camera_y,
        tick,
    );
    draw_props(ctx, fb, cam, tick, PropLayer::Back);
    tiles::draw_tiles(fb, atlas, &ctx.tiles, game, cam, tick);
    draw_props(ctx, fb, cam, tick, PropLayer::Front);
    draw_goals(game, ctx, fb, cam);
    let p = game.player();
    let feet_x = (p.x + p.w.div_int(2)).to_px() - cam.x;
    let feet_y = (p.y + p.h).to_px() - cam.y;
    draw_player_pose(
        ctx,
        fb,
        p.power,
        PlayerAnim::Idle,
        (feet_x, feet_y),
        tick,
        false,
    );
}

/// Resi outside play (title, intro, route map) in her power state:
/// `player`, `player_big` or `player_hackl` (with their fallbacks, see
/// [`RenderContext`]) in `state` with her feet at `feet` on screen,
/// mirrored if `left`; else the grey-box player (a 12×22 `r` box, 12×30
/// wine when Zuckerschock, 12×30 pine green when Hackl).
pub(crate) fn draw_player_pose(
    ctx: &RenderContext,
    fb: &mut Framebuffer,
    power: Power,
    state: PlayerAnim,
    (x, y): (i32, i32),
    tick: u32,
    left: bool,
) {
    let (look, swap) = ctx.player_look(power);
    match look {
        Some(look) => {
            let anim = look.anims[state.index()];
            let i = anim.frame_index_at(tick);
            draw_look(fb, ctx.atlas, look.sprite, anim, i, (x, y), left, swap);
        }
        None => {
            let (h, fill) = match power {
                Power::Normal => (22, DIRNDL_RED),
                Power::Big => (30, entities::WINE_DARK),
                Power::Hackl => (30, entities::PINE_DARK),
            };
            fb.fill_rect(x - 6, y - h, 12, h, fill);
            fb.outline_rect(x - 6, y - h, 12, h, OUTLINE);
        }
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

/// The props of `layer`: their sprite (first anim) with its origin on the
/// bottom centre of the prop's cell; no collision. Missing sprite: the
/// outline of the cell (grey-box).
fn draw_props(ctx: &RenderContext, fb: &mut Framebuffer, cam: Camera, tick: u32, layer: PropLayer) {
    for p in ctx.props.iter().filter(|p| p.layer == layer) {
        let (x, y) = (p.foot_x - cam.x, p.foot_y - cam.y);
        match p.sprite {
            Some(s) => draw_sprite(fb, ctx.atlas, s, &s.anims[0], tick, x, y, false),
            None => fb.outline_rect(x - TILE_PX / 2, y - TILE_PX, TILE_PX, TILE_PX, OUTLINE),
        }
    }
}

/// Maibaum: `maibaum_pole` repeated up from the bottom of the goal cell as
/// high as the Wurstkranz zone (`goal.zone_heights`' last value), and
/// `maibaum_top` with its origin on the pole's top. Missing sprites: a
/// grey-box pole (2 px `Y`) and a small `r` flag at the top.
fn draw_goals(game: &Game, ctx: &RenderContext, fb: &mut Framebuffer, cam: Camera) {
    let height = game.tuning().goal.zone_heights[3];
    let tick = game.frame();
    for e in &game.level().entities {
        if e.kind != SpawnKind::Goal {
            continue;
        }
        let foot_x = e.col * TILE_PX + TILE_PX / 2 - cam.x;
        let foot_y = (e.row + 1) * TILE_PX - cam.y;
        let top = foot_y - height;
        match ctx.maibaum_pole {
            Some(s) => {
                let mut y = foot_y;
                while y > top {
                    draw_sprite(fb, ctx.atlas, s, &s.anims[0], tick, foot_x, y, false);
                    y -= TILE_PX;
                }
            }
            None => fb.fill_rect(foot_x - 1, top, 2, height, MOZART_YELLOW),
        }
        match ctx.maibaum_top {
            Some(s) => draw_sprite(fb, ctx.atlas, s, &s.anims[0], tick, foot_x, top, false),
            None => {
                fb.fill_rect(foot_x - 9, top + 2, 8, 6, DIRNDL_RED);
                fb.outline_rect(foot_x - 9, top + 2, 8, 6, OUTLINE);
            }
        }
    }
}

/// The power state the player is drawn in this frame: hers, except while
/// transforming, when the old (`prev_power`) and the new state's looks
/// alternate every 4 frames (starting with the old one), for every change
/// (growing, shrinking, Zuckerschock ↔ Hackl).
pub fn player_shown_power(p: &Player, t: &PlayerTuning) -> Power {
    if p.transform == 0 {
        return p.power;
    }
    let elapsed = u32::from(t.transform_frames.saturating_sub(p.transform));
    let show_new = (elapsed / DYING_FLASH_FRAMES) % 2 == 1;
    if show_new { p.power } else { p.prev_power }
}

/// Whether the player is drawn big this frame (see [`player_shown_power`]).
pub fn player_shown_big(p: &Player, t: &PlayerTuning) -> bool {
    player_shown_power(p, t).is_big()
}

/// The player: sprite `player` (Normal), `player_big` (Zuckerschock,
/// falls back to `player`) or `player_hackl` (Hackl, falls back to the
/// recoloured big look, [`HACKL_FALLBACK_SWAP`]) with its origin at the
/// feet (bottom centre of the hitbox, D-011), else the hitbox as a box in
/// the state's colour with a `k` outline. Blinks while dying (counted from
/// the moment of death) and while invulnerable; alternates looks while
/// transforming. During the throw pose (Hackl) anim `throw`, its frame by
/// the frames since the throw (without `throw`: the movement anim). While
/// crawling anim `crawl` facing right, or nothing if the look has no
/// `crawl` (the D-057 fallback: hidden inside the stone).
fn draw_player(game: &Game, ctx: &RenderContext, fb: &mut Framebuffer, cam: Camera) {
    if let Mode::Dying { timer } = game.mode() {
        let elapsed = u32::from(game.tuning().game.death_frames.saturating_sub(timer));
        if (elapsed / DYING_FLASH_FRAMES) % 2 == 1 {
            return;
        }
    }
    let p = game.player();
    if p.transform == 0 && (u32::from(p.invulnerable) / DYING_FLASH_FRAMES) % 2 == 1 {
        return;
    }
    let sliding = game
        .stage_clear()
        .is_some_and(|c| c.phase == ClearPhase::Slide);
    draw_resi(ctx, fb, cam, p, game.tuning(), sliding, game.frame());
}

/// [`draw_player`] without the blinking: Resi as `p` says, at `tick`.
fn draw_resi(
    ctx: &RenderContext,
    fb: &mut Framebuffer,
    cam: Camera,
    p: &Player,
    tuning: &jnr_core::Tuning,
    sliding: bool,
    tick: u32,
) {
    let t = &tuning.player;
    let power = player_shown_power(p, t);
    let (look, swap) = ctx.player_look(power);
    let feet = (
        (p.x + p.w.div_int(2)).to_px() - cam.x,
        (p.y + p.h).to_px() - cam.y,
    );
    let atlas = ctx.atlas;
    match look {
        Some(look) if p.crawling() => {
            if let Some(anim) = look.crawl {
                let i = anim.frame_index_at(tick);
                draw_look(fb, atlas, look.sprite, anim, i, feet, false, swap);
            }
        }
        Some(look) => {
            let throw = look
                .throw
                .filter(|_| p.throw_pose > 0 && power == Power::Hackl);
            let (anim, index) = match throw {
                Some(anim) => {
                    let total = tuning.hackl.throw_pose_frames;
                    let elapsed = total.saturating_sub(p.throw_pose);
                    (anim, throw_frame(u32::from(elapsed)))
                }
                None => {
                    let state = if sliding {
                        PlayerAnim::Pole
                    } else {
                        PlayerAnim::of(p, t)
                    };
                    let anim = look.anims[state.index()];
                    (anim, anim.frame_index_at(tick))
                }
            };
            let left = player_faces_left(p, t);
            draw_look(fb, atlas, look.sprite, anim, index, feet, left, swap);
        }
        None => {
            // Grey-box: the state's height (also while transforming), the
            // 12×12 hitbox while crawling.
            let (h, fill) = grey_box_player(power, t);
            let h = if p.crawling() { p.h.to_px() } else { h };
            let (x, y) = (p.x.to_px() - cam.x, feet.1 - h);
            let w = p.w.to_px();
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
        // Goal pole in column 16: x 263..265, from y 32 (7 tiles, D-038) down to 144.
        assert_eq!(fb.get(263, 140), Some(MOZART_YELLOW));
        assert_eq!(fb.get(264, 32), Some(MOZART_YELLOW));
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
            ("bg_sky", 0, 0, 0),
            ("bg_missing", 2, 0, 0),
            ("bg_hills", 8, 0, 0),
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
        let ctx = RenderContext::new(&atlas, &texts, game.level())
            .with_background(&[("bg_hills", 8, 0, 0)]);
        assert!(!ctx.background_covers);
        let mut fb = Framebuffer::new();
        render(&game, &ctx, &mut fb);
        assert_eq!(fb.get(0, 0), Some(SKY_LIGHT));
        assert_eq!(fb.get(0, 179), Some(A));

        // From the level: `[meta].background` with a `y` offset.
        let text = "[meta]\nid = \"bg\"\ntime_limit = 300\n\
                    background = [{ sprite = \"bg_hills\", factor = 8, y = 5 }]\n\
                    [legend]\n\"P\" = { entity = \"player_start\" }\n[map]\ngrid = '''\nP\n'''\n";
        let level = Level::parse(text).unwrap();
        let ctx = RenderContext::for_level(&atlas, &texts, &level);
        assert_eq!(ctx.background().len(), 1);
        assert_eq!(
            (ctx.background()[0].factor16, ctx.background()[0].y_offset),
            (8, 5)
        );
        let game = Game::new(&level, &tuning(), 1);
        let mut fb = Framebuffer::new();
        render(&game, &ctx, &mut fb);
        assert_eq!(fb.get(0, 179 - 5), Some(A), "raised by y = 5");
        assert_eq!(fb.get(0, 179), Some(SKY_LIGHT));
    }

    /// A 2-column, 21-row climb level with a `factor_y = 8` layer and the
    /// player standing on a ledge in `start_row`.
    fn tall_level(start_row: usize) -> Level {
        let mut rows = vec![".."; 20];
        rows[start_row] = "P.";
        rows[start_row + 1] = "##";
        rows.push("##");
        let text = format!(
            "[meta]\nid = \"tall\"\ntime_limit = 300\ncamera = \"climb\"\n\
             background = [{{ sprite = \"bg_hills\", factor = 0, factor_y = 8 }}]\n\
             [legend]\n\"P\" = {{ entity = \"player_start\" }}\n\"#\" = \"solid\"\n\
             [map]\ngrid = '''\n{}\n'''\n",
            rows.join("\n")
        );
        Level::parse(&text).unwrap()
    }

    #[test]
    fn factor_y_layers_sink_as_the_climb_camera_rises() {
        let atlas = testutil::atlas(&[testutil::sprite(
            "bg_hills",
            (0, 0),
            false,
            vec![("idle", 0, vec![image(&["ab..", "ab.."])])],
        )]);
        let texts = texts();
        let low = tall_level(18);
        let ctx = RenderContext::for_level(&atlas, &texts, &low);
        assert_eq!(ctx.max_camera_y, 21 * 16 - 180);
        assert_eq!(ctx.background()[0].factor_y16, 8);
        let game = Game::new(&low, &tuning(), 1);
        assert_eq!(game.camera().y, ctx.max_camera_y, "at the level bottom");
        let mut fb = Framebuffer::new();
        render(&game, &ctx, &mut fb);
        assert_eq!(
            fb.get(100, 179),
            Some(A),
            "unmoved at the bottom (right of the level)"
        );
        // Feet at 160 settle at screen y 148: camera y 12, 144 px climbed,
        // so the 2 px strip sinks 72 px, below the screen.
        let high = tall_level(9);
        let game = Game::new(&high, &tuning(), 1);
        assert_eq!(game.camera().y, 12);
        let mut fb = Framebuffer::new();
        render(&game, &ctx, &mut fb);
        assert_eq!(fb.get(100, 179), Some(SKY_LIGHT), "sunk off the screen");
        assert!(!fb.pixels.contains(&A));
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
        assert!(ctx.tiles.looks.iter().all(Option::is_none));
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
        assert!(!player_faces_left(&running, &t), "facing stays right");
        // Running right, holding left: skids, facing the motion (right).
        let skidding = Player {
            vx: t.run_speed,
            skidding: true,
            facing: Facing::Left,
            ..p
        };
        assert_eq!(PlayerAnim::of(&skidding, &t), PlayerAnim::Skid);
        assert!(!player_faces_left(&skidding, &t), "skid faces the motion");
        let skid_left = Player {
            vx: -t.walk_speed,
            facing: Facing::Right,
            ..skidding
        };
        assert!(player_faces_left(&skid_left, &t));
        // Once stopped it is idle again and faces the input direction.
        let stopped = Player {
            vx: Fx::ZERO,
            ..skidding
        };
        assert_eq!(PlayerAnim::of(&stopped, &t), PlayerAnim::Idle);
        assert!(player_faces_left(&stopped, &t));
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
    fn maibaum_sprites_and_the_pole_anim_while_sliding() {
        let atlas = testutil::atlas(&[
            testutil::sprite(
                PLAYER_SPRITE,
                (2, 4),
                true,
                vec![
                    ("idle", 0, vec![image(&["aaaa"; 4])]),
                    ("pole", 0, vec![image(&["bbbb"; 4])]),
                ],
            ),
            testutil::sprite(
                MAIBAUM_POLE_SPRITE,
                (8, 16),
                false,
                vec![("idle", 0, vec![image(&["......cccc......"; 16])])],
            ),
            testutil::sprite(
                MAIBAUM_TOP_SPRITE,
                (1, 2),
                false,
                vec![("idle", 0, vec![image(&["cc", "cc"])])],
            ),
        ]);
        // Goal at column 6, row 8: pole centre x 104, base y 144.
        let grid = "\
....................
....................
....................
....................
....................
....................
....................
....................
..P...G.............
####################
";
        let mut game = Game::new(&level(grid), &tuning(), 1);
        let fb = rendered(&game, &atlas);
        // 7 pole tiles from y 32 to 144 (D-038), 4 px wide (x 102..106); top above.
        assert_eq!(fb.get(102, 143), Some(C));
        assert_eq!(fb.get(105, 32), Some(C));
        assert_eq!(fb.get(106, 100), Some(SKY_LIGHT));
        assert_eq!(fb.get(103, 30), Some(C), "top at y 30..32");
        assert_eq!(fb.get(103, 28), Some(SKY_LIGHT));
        // Jump into the pole: `pole` anim while sliding, idle-or-walk after.
        game.step(InputFrame::RIGHT.with(InputFrame::JUMP));
        while game.mode() != Mode::StageClear {
            game.step(InputFrame::RIGHT.with(InputFrame::JUMP));
            assert!(game.frame() < 200);
        }
        game.step(InputFrame::NONE);
        let c = game.stage_clear().unwrap();
        assert_eq!(c.phase, jnr_core::ClearPhase::Slide);
        let p = game.player();
        let (fx, fy) = ((p.x + p.w.div_int(2)).to_px(), (p.y + p.h).to_px());
        assert_eq!(
            rendered(&game, &atlas).get(fx, fy - 1),
            Some(B),
            "pole anim"
        );
    }

    /// Props (D-030/D-031): front props after the tiles, back props before
    /// them, `dx` moves them, a missing sprite is a grey-box cell outline.
    #[test]
    fn props_draw_on_their_layer_with_their_offset() {
        let text = "[meta]\nid = \"tiny\"\ntime_limit = 300\n\n[legend]\n\"#\" = \"solid\"\n\
             \"P\" = { entity = \"player_start\" }\n\
             \"s\" = { entity = \"prop\", sprite = \"sphaera\", dx = 8 }\n\
             \"h\" = { entity = \"prop\", sprite = \"geburtshaus\", layer = \"back\" }\n\
             \"z\" = { entity = \"prop\", sprite = \"zwerg\" }\n\n[map]\ngrid = '''\n\
             ....................\n....................\n....................\n\
             ....................\n....................\n....................\n\
             ....................\n....................\n\
             ..P.s...h...z.......\n####################\n'''\n";
        let game = Game::new(&Level::parse(text).unwrap(), &tuning(), 1);
        // 4x8 sprites with the origin 4 px above their bottom: the lower
        // half reaches into the ground row (y 144..160).
        let prop = |name: &str, c: &str| {
            testutil::sprite(name, (2, 4), false, vec![("idle", 0, vec![image(&[c; 8])])])
        };
        let atlas = testutil::atlas(&[
            prop("sphaera", "aaaa"),
            prop("geburtshaus", "bbbb"),
            testutil::sprite(
                "tile_solid",
                (0, 0),
                false,
                vec![("tile", 0, vec![image(&["cccccccccccccccc"; 16])])],
            ),
        ]);
        let fb = rendered(&game, &atlas);
        // Front: column 4 (foot x 72) + dx 8 = 80 -> x 78..82, y 140..148,
        // over the ground tile.
        assert_eq!(fb.get(78, 140), Some(A));
        assert_eq!(fb.get(81, 147), Some(A), "in front of the tile");
        assert_eq!(fb.get(77, 140), Some(SKY_LIGHT));
        assert_eq!(fb.get(82, 140), Some(SKY_LIGHT), "moved by dx");
        // Back: column 8 (foot x 136) -> x 134..138; the tile covers its
        // lower half.
        assert_eq!(fb.get(134, 140), Some(B));
        assert_eq!(fb.get(134, 145), Some(C), "behind the tile");
        // No `zwerg` sprite: the outline of its cell (column 12, row 8).
        assert_eq!(fb.get(192, 128), Some(OUTLINE));
        assert_eq!(fb.get(207, 143), Some(OUTLINE));
        assert_eq!(fb.get(200, 136), Some(SKY_LIGHT));
        // No collision: the cells stay empty.
        assert_eq!(
            game.level().tile(4, 8).unwrap().kind,
            jnr_core::TileKind::Empty
        );
    }

    #[test]
    fn checkpoint_uses_idle_then_active() {
        let atlas = testutil::atlas(&[testutil::sprite(
            "checkpoint",
            (0, 1),
            false,
            vec![
                ("idle", 0, vec![image(&["a"])]),
                ("active", 0, vec![image(&["b"])]),
            ],
        )]);
        let text = "[meta]\nid = \"t\"\ntime_limit = 300\n[legend]\n\"#\" = \"solid\"\n\
                    \"P\" = { entity = \"player_start\" }\n\"C\" = { entity = \"checkpoint\" }\n\
                    [map]\ngrid = '''\n..........\n.P...C....\n##########\n'''\n";
        let mut game = Game::new(&Level::parse(text).unwrap(), &tuning(), 1);
        game.step(InputFrame::NONE);
        // Feet at (5 * 16 + 8, 32): the 1x1 sprite sits at (88, 31).
        assert_eq!(rendered(&game, &atlas).get(88, 31), Some(A));
        while game.checkpoint().is_none() {
            game.step(InputFrame::RIGHT);
            assert!(game.frame() < 200);
        }
        // Walk on so the player no longer covers it.
        for _ in 0..30 {
            game.step(InputFrame::RIGHT);
        }
        assert_eq!(rendered(&game, &atlas).get(88, 31), Some(B));
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

    /// A 2×2 frame of one colour.
    fn solid(rgba: u32) -> jnr_artkit::Image {
        let mut img = jnr_artkit::Image::new(2, 2);
        for (x, y) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
            img.set(x, y, rgba.to_be_bytes());
        }
        img
    }

    /// The colour drawn just above Resi's feet by [`draw_resi`].
    fn resi_colour(ctx: &RenderContext, p: &Player) -> Option<u32> {
        let mut fb = testutil::fb();
        let cam = Camera { x: 0, y: 0 };
        draw_resi(ctx, &mut fb, cam, p, &tuning(), false, 0);
        let (fx, fy) = p.feet();
        fb.get(fx.to_px() - 1, fy.to_px() - 1)
    }

    #[test]
    fn hackl_resi_falls_back_to_the_recoloured_big_look() {
        let t = tuning().player;
        let atlas = testutil::atlas(&[
            testutil::sprite(
                PLAYER_SPRITE,
                (1, 2),
                false,
                vec![("idle", 0, vec![solid(A)])],
            ),
            testutil::sprite(
                PLAYER_BIG_SPRITE,
                (1, 2),
                false,
                vec![("idle", 0, vec![solid(DIRNDL_RED)])],
            ),
        ]);
        let (texts, game) = (Texts::default(), tiny_game());
        let ctx = RenderContext::new(&atlas, &texts, game.level());
        assert!(!ctx.has_player_hackl());
        let mut p = *game.player();
        assert_eq!(resi_colour(&ctx, &p), Some(A));
        p.power = Power::Big;
        assert_eq!(resi_colour(&ctx, &p), Some(DIRNDL_RED));
        p.power = Power::Hackl;
        assert_eq!(resi_colour(&ctx, &p), Some(entities::PINE_DARK), "fallback");
        // Zuckerschock → Hackl: the old look first, the new one 4 frames on.
        p.prev_power = Power::Big;
        p.transform = t.transform_frames;
        assert_eq!(player_shown_power(&p, &t), Power::Big);
        assert_eq!(resi_colour(&ctx, &p), Some(DIRNDL_RED));
        p.transform = t.transform_frames - 4;
        assert_eq!(player_shown_power(&p, &t), Power::Hackl);
        assert_eq!(resi_colour(&ctx, &p), Some(entities::PINE_DARK));
        p.transform = t.transform_frames - 8;
        assert_eq!(player_shown_power(&p, &t), Power::Big);
        // Hackl → Zuckerschock (a hit) alternates the same way.
        p.power = Power::Big;
        p.prev_power = Power::Hackl;
        p.transform = t.transform_frames;
        assert_eq!(player_shown_power(&p, &t), Power::Hackl);
        // The throw pose without a `throw` anim: the movement anim.
        p.power = Power::Hackl;
        p.transform = 0;
        p.throw_pose = 10;
        assert_eq!(resi_colour(&ctx, &p), Some(entities::PINE_DARK));
        // Crawling without a `crawl` anim: hidden (D-057 fallback).
        p.throw_pose = 0;
        p.crawl_to = Some(p.x + Fx::from_px(64));
        p.y += p.h - Fx::from_px(12);
        p.h = Fx::from_px(12);
        assert_eq!(resi_colour(&ctx, &p), Some(testutil::BG));
        // Grey-box (no sprites at all): the 12×12 crawl hitbox in green.
        let none = empty_atlas();
        let ctx = RenderContext::new(&none, &texts, game.level());
        let mut fb = testutil::fb();
        draw_resi(
            &ctx,
            &mut fb,
            Camera { x: 0, y: 0 },
            &p,
            &tuning(),
            false,
            0,
        );
        let (x, y) = (p.x.to_px(), p.y.to_px());
        assert_eq!(fb.get(x + 6, y + 6), Some(entities::PINE_DARK));
        assert_eq!(fb.get(x + 6, y - 1), Some(testutil::BG), "12 px tall");
    }

    #[test]
    fn player_hackl_throws_and_crawls_with_its_own_anims() {
        let anims = |colour| {
            vec![
                ("idle", 0, vec![solid(colour)]),
                ("throw", 0, vec![solid(C), solid(A)]),
                ("crawl", 8, vec![solid(B), solid(B)]),
            ]
        };
        let atlas = testutil::atlas(&[
            testutil::sprite(
                PLAYER_SPRITE,
                (1, 2),
                false,
                vec![("idle", 0, vec![solid(A)])],
            ),
            testutil::sprite(PLAYER_HACKL_SPRITE, (1, 2), false, anims(STONE_GREY)),
        ]);
        let (texts, game) = (Texts::default(), tiny_game());
        let ctx = RenderContext::new(&atlas, &texts, game.level());
        assert!(ctx.has_player_hackl());
        let mut p = *game.player();
        p.power = Power::Hackl;
        assert_eq!(resi_colour(&ctx, &p), Some(STONE_GREY), "no swap");
        // Throw pose: frame 0 for 4 frames, then frame 1.
        for (left, colour) in [(10, C), (7, C), (6, A), (1, A)] {
            p.throw_pose = left;
            assert_eq!(resi_colour(&ctx, &p), Some(colour), "{left} frames left");
        }
        assert_eq!((throw_frame(3), throw_frame(4)), (0, 1));
        p.throw_pose = 0;
        p.crawl_to = Some(p.x + Fx::from_px(64));
        assert_eq!(resi_colour(&ctx, &p), Some(B), "crawl anim");
        // Normal Resi has no `crawl` here: hidden.
        p.power = Power::Normal;
        assert_eq!(resi_colour(&ctx, &p), Some(testutil::BG));
    }
}
