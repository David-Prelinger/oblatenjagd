//! Drawing entities (items, particles, popups, props, enemies) and the
//! grey-box looks of block tiles.
//!
//! Every entity uses its catalog sprite (`docs/sprite-catalog.md`) with the
//! origin at its feet when the atlas has it, else a grey box in a colour of
//! its own per kind. Sprites are resolved once in [`EntityLooks::new`]
//! (part of the `RenderContext`); drawing allocates nothing.
//!
//! Draw passes (called from `scene::render`):
//! 1. [`draw_behind_tiles`]: power-ups still rising out of their box
//! 2. tiles (scene), raised by `Game::tile_bump_px` while bumped
//! 3. [`draw_entities`]: Gondel cables first, then items, props, enemies
//!    (animation by `Entity::pose`, facing by `Entity::dir`, the swarm's
//!    shake by `Entity::shake_px`, a `lead` Pilger as `pilger_lead`), the
//!    Steinschlag niches (resting/wobbling rock and the `kiesel` trickle),
//!    falling rocks, the Festungsbahn and the Gondel cabins
//! 4. player (scene)
//! 5. [`draw_front`]: debris (bricks and rocks) and popups

use jnr_artkit::atlas::{Atlas, AtlasAnim, AtlasSprite};
use jnr_core::entity::{EnemyKind, Entity, EntityKind, Popup};
use jnr_core::{Block, Camera, Game, Pose, Tile};

use crate::Framebuffer;
use crate::blit::draw_sprite;
use crate::font::Font;
use crate::palette::{LEATHER_DARK, OUTLINE, WHITE};
use crate::signs;

/// `o` oblate_gold: Oblaten.
pub const OBLATE_GOLD: u32 = 0xd9913aff;
/// `p` pastel_pink: Mozartkugel power-up.
pub const PASTEL_PINK: u32 = 0xf2aeb0ff;
/// `O` oblate_brown: Zaunerstollen, empty boxes.
pub const OBLATE_BROWN: u32 = 0x9e5a2eff;
/// `n` skin_dark (terracotta): bricks and their debris.
pub const TERRACOTTA: u32 = 0xa85f40ff;
/// `Y` mozart_yellow: Oblatenkisten, checkpoint disc.
pub const MOZART_YELLOW: u32 = 0xf5c547ff;
/// `G` pine_dark: checkpoint pole/rim (green „H").
pub const PINE_DARK: u32 = 0x2a7050ff;
/// `c` cream: sign boards.
pub const CREAM: u32 = 0xf9e5b8ff;
/// `b` lake_deep: Selfie-Tourist.
pub const LAKE_DEEP: u32 = 0x2e64aeff;
/// `i` silver_light: Taube.
pub const SILVER_LIGHT: u32 = 0xa2abbeff;
/// `R` wine_dark: Mozartkugel-Verkäufer, Zuckerschock player box.
pub const WINE_DARK: u32 = 0x8a2340ff;
/// `j` pine_shadow: Fiaker.
pub const PINE_SHADOW: u32 = 0x1b4640ff;
/// `T` lake_teal: Festungsbahn car.
pub const LAKE_TEAL: u32 = 0x1e8e9cff;
/// `r` dirndl_red: the Ortsende bar.
pub const DIRNDL_RED: u32 = 0xcf3a3eff;
/// `v` mid_sage: Nordic-Walker.
pub const MID_SAGE: u32 = 0x7aa294ff;
/// `E` skin_shade: Blasmusiker.
pub const SKIN_SHADE: u32 = 0xdc9068ff;
/// `d` slate_dark: Blasmusik notes and the Gondel cable.
pub const SLATE_DARK: u32 = 0x343a50ff;
/// `t` lake_turquoise: E-Biker.
pub const LAKE_TURQUOISE: u32 = 0x46cabeff;
/// `y` meadow_light: Bienenschwarm.
pub const MEADOW_LIGHT: u32 = 0xb0d65aff;
/// `e` skin_light: Gondel cabin.
pub const SKIN_LIGHT: u32 = 0xf9cfa6ff;
/// `g` meadow: Pilger.
pub const MEADOW: u32 = 0x56a845ff;
/// `B` navy: Gams.
pub const NAVY: u32 = 0x2a3878ff;
/// `F` far_dark: Stein (falling and in its niche).
pub const FAR_DARK: u32 = 0x8ea4ccff;
/// `f` far_light: Stein debris.
pub const FAR_LIGHT: u32 = 0xa4bad8ff;
/// `m` stone_grey: `kiesel` pebbles without their sprite.
pub const STONE_GREY: u32 = 0x5d6680ff;

/// Pebbles in a Steinschlag's telegraph trickle (renderer only) ...
pub const KIESEL_COUNT: i32 = 3;
/// ... falling this many px per frame down the rock's column.
pub const KIESEL_SPEED_PX: i32 = 3;

/// The Gondel's cable grip above its feet (sprite-catalog: the cable
/// meets the 32×52 sprite at (16, 0), origin (16, 52)).
pub const GONDEL_GRIP_PX: i32 = 52;

/// The entity sprites, resolved once.
#[derive(Clone, Debug, Default)]
pub struct EntityLooks<'a> {
    oblate: Option<&'a AtlasSprite>,
    mozartkugel: Option<&'a AtlasSprite>,
    zaunerstollen: Option<&'a AtlasSprite>,
    debris: Option<&'a AtlasSprite>,
    checkpoint: Option<&'a AtlasSprite>,
    festungsbahn: Option<&'a AtlasSprite>,
    selfie_tourist: Option<&'a AtlasSprite>,
    taube: Option<&'a AtlasSprite>,
    verkaeufer: Option<&'a AtlasSprite>,
    fiaker: Option<&'a AtlasSprite>,
    thrown: Option<&'a AtlasSprite>,
    nordic_walker: Option<&'a AtlasSprite>,
    blasmusiker: Option<&'a AtlasSprite>,
    e_biker: Option<&'a AtlasSprite>,
    bienenschwarm: Option<&'a AtlasSprite>,
    note: Option<&'a AtlasSprite>,
    gondel: Option<&'a AtlasSprite>,
    pilger: Option<&'a AtlasSprite>,
    pilger_lead: Option<&'a AtlasSprite>,
    gams: Option<&'a AtlasSprite>,
    stein: Option<&'a AtlasSprite>,
    stein_debris: Option<&'a AtlasSprite>,
    kiesel: Option<&'a AtlasSprite>,
    hackl: Option<&'a AtlasSprite>,
    hackl_thrown: Option<&'a AtlasSprite>,
    /// Sign boards by style (`signs::SIGN_SPRITES`).
    pub(crate) signs: [Option<&'a AtlasSprite>; 3],
    /// `sign_ortsende_bar`.
    pub(crate) ortsende_bar: Option<&'a AtlasSprite>,
}

impl<'a> EntityLooks<'a> {
    /// Looks up every entity sprite; sprites without anims count as missing.
    pub fn new(atlas: &'a Atlas) -> EntityLooks<'a> {
        let s = |name: &str| atlas.sprite(name).filter(|s| !s.anims.is_empty());
        EntityLooks {
            oblate: s("oblate"),
            mozartkugel: s("mozartkugel"),
            zaunerstollen: s("zaunerstollen"),
            debris: s("debris"),
            checkpoint: s("checkpoint"),
            festungsbahn: s("festungsbahn"),
            selfie_tourist: s(EnemyKind::SelfieTourist.sprite()),
            taube: s(EnemyKind::Taube.sprite()),
            verkaeufer: s(EnemyKind::MozartkugelVerkaeufer.sprite()),
            fiaker: s(EnemyKind::Fiaker.sprite()),
            thrown: s("mozartkugel_thrown"),
            nordic_walker: s(EnemyKind::NordicWalker.sprite()),
            blasmusiker: s(EnemyKind::Blasmusiker.sprite()),
            e_biker: s(EnemyKind::EBiker.sprite()),
            bienenschwarm: s(EnemyKind::Bienenschwarm.sprite()),
            note: s("note"),
            gondel: s("gondel"),
            pilger: s(EnemyKind::Pilger.sprite()),
            pilger_lead: s("pilger_lead"),
            gams: s(EnemyKind::Gams.sprite()),
            stein: s("stein"),
            stein_debris: s("stein_debris"),
            kiesel: s("kiesel"),
            hackl: s("hackl"),
            hackl_thrown: s("hackl_thrown"),
            signs: signs::SIGN_SPRITES.map(s),
            ortsende_bar: s(signs::ORTSENDE_BAR_SPRITE),
        }
    }

    /// The sprite of an entity kind, if the atlas has it.
    pub fn sprite(&self, kind: EntityKind) -> Option<&'a AtlasSprite> {
        match kind {
            EntityKind::Oblate | EntityKind::PoppedOblate => self.oblate,
            EntityKind::Mozartkugel => self.mozartkugel,
            EntityKind::Zaunerstollen => self.zaunerstollen,
            EntityKind::Debris => self.debris,
            EntityKind::Checkpoint => self.checkpoint,
            EntityKind::Festungsbahn => self.festungsbahn,
            EntityKind::Enemy(k) | EntityKind::Defeated(k) => match k {
                EnemyKind::SelfieTourist => self.selfie_tourist,
                EnemyKind::Taube | EnemyKind::TaubeSitting => self.taube,
                EnemyKind::MozartkugelVerkaeufer => self.verkaeufer,
                EnemyKind::Fiaker => self.fiaker,
                EnemyKind::NordicWalker => self.nordic_walker,
                EnemyKind::Blasmusiker => self.blasmusiker,
                EnemyKind::EBiker => self.e_biker,
                EnemyKind::Bienenschwarm => self.bienenschwarm,
                EnemyKind::Pilger => self.pilger,
                EnemyKind::Gams => self.gams,
            },
            EntityKind::Thrown => self.thrown,
            EntityKind::Note => self.note,
            EntityKind::Gondel => self.gondel,
            EntityKind::Steinschlag | EntityKind::Stein => self.stein,
            EntityKind::SteinDebris => self.stein_debris,
            EntityKind::Hackl => self.hackl,
            EntityKind::ThrownHackl => self.hackl_thrown,
            EntityKind::Popup(_)
            | EntityKind::Bump
            | EntityKind::Sign
            | EntityKind::Durchkriechstein => None,
        }
    }
}

/// Grey-box fill colour of an entity kind (also its look in the debug view).
pub fn grey_box_colour(kind: EntityKind) -> u32 {
    match kind {
        EntityKind::Oblate | EntityKind::PoppedOblate => OBLATE_GOLD,
        EntityKind::Mozartkugel => PASTEL_PINK,
        EntityKind::Zaunerstollen => OBLATE_BROWN,
        EntityKind::Debris => TERRACOTTA,
        EntityKind::Checkpoint => PINE_DARK,
        EntityKind::Sign => CREAM,
        EntityKind::Enemy(k) | EntityKind::Defeated(k) => match k {
            EnemyKind::SelfieTourist => LAKE_DEEP,
            EnemyKind::Taube | EnemyKind::TaubeSitting => SILVER_LIGHT,
            EnemyKind::MozartkugelVerkaeufer => WINE_DARK,
            EnemyKind::Fiaker => PINE_SHADOW,
            EnemyKind::NordicWalker => MID_SAGE,
            EnemyKind::Blasmusiker => SKIN_SHADE,
            EnemyKind::EBiker => LAKE_TURQUOISE,
            EnemyKind::Bienenschwarm => MEADOW_LIGHT,
            EnemyKind::Pilger => MEADOW,
            EnemyKind::Gams => NAVY,
        },
        EntityKind::Steinschlag | EntityKind::Stein => FAR_DARK,
        EntityKind::SteinDebris => FAR_LIGHT,
        EntityKind::Thrown => LEATHER_DARK,
        EntityKind::Note => SLATE_DARK,
        EntityKind::Festungsbahn => LAKE_TEAL,
        EntityKind::Gondel => SKIN_LIGHT,
        EntityKind::Hackl => PINE_DARK,
        EntityKind::ThrownHackl => SILVER_LIGHT,
        EntityKind::Popup(_) | EntityKind::Bump | EntityKind::Durchkriechstein => WHITE,
    }
}

/// Grey-box colour of a solid tile without a sprite: bricks, boxes and
/// empty boxes get their own colours, everything else `default`.
pub fn solid_tile_colour(game: &Game, tile: Tile, default: u32) -> u32 {
    match tile.block {
        Block::Brick => TERRACOTTA,
        Block::Box => MOZART_YELLOW,
        Block::None if tile.name != 0 && tile.name == game.level().empty_box_name() => OBLATE_BROWN,
        Block::None => default,
    }
}

/// Power-ups still rising out of their box (drawn before the tiles, so the
/// box covers them).
pub fn draw_behind_tiles(game: &Game, looks: &EntityLooks, atlas: &Atlas, fb: &mut Framebuffer) {
    let cam = game.camera();
    for e in game.entities() {
        if e.rising(game.tuning()) {
            draw_body(game, looks, atlas, fb, cam, e);
        }
    }
}

/// Items, props and enemies (not rising power-ups, debris or popups).
pub fn draw_entities(
    game: &Game,
    looks: &EntityLooks,
    atlas: &Atlas,
    font: &Font,
    texts: &jnr_core::Texts,
    fb: &mut Framebuffer,
) {
    let cam = game.camera();
    // Cables behind every cabin.
    for e in game.entities() {
        if e.kind == EntityKind::Gondel {
            draw_cable(game, fb, cam, e);
        }
    }
    for e in game.entities() {
        match e.kind {
            // The Durchkriechstein is drawn by its prop (D-057).
            EntityKind::Debris
            | EntityKind::SteinDebris
            | EntityKind::Popup(_)
            | EntityKind::Bump
            | EntityKind::Durchkriechstein => {}
            EntityKind::Sign => signs::draw_sign(game, looks, atlas, font, texts, fb, cam, e),
            EntityKind::Checkpoint => draw_checkpoint(game, looks, atlas, fb, cam, e),
            EntityKind::Enemy(_)
            | EntityKind::Defeated(_)
            | EntityKind::Thrown
            | EntityKind::Stein
            | EntityKind::Note
            | EntityKind::ThrownHackl => draw_enemy(game, looks, atlas, fb, cam, e),
            EntityKind::Steinschlag => draw_steinschlag(game, looks, atlas, fb, cam, e),
            EntityKind::Festungsbahn => draw_festungsbahn(game, looks, atlas, fb, cam, e),
            EntityKind::Gondel => draw_gondel(game, looks, atlas, fb, cam, e),
            _ if e.rising(game.tuning()) => {}
            _ => draw_body(game, looks, atlas, fb, cam, e),
        }
    }
}

/// The Festungsbahn car: sprite `festungsbahn` (anim `idle`) at its feet,
/// never mirrored (it moves both ways on its track), else a grey box.
fn draw_festungsbahn(
    game: &Game,
    looks: &EntityLooks,
    atlas: &Atlas,
    fb: &mut Framebuffer,
    cam: Camera,
    e: &Entity,
) {
    match looks.festungsbahn {
        Some(s) => {
            let (x, y) = feet_on_screen(e, cam);
            let anim = s.anim("idle").unwrap_or(&s.anims[0]);
            draw_sprite(fb, atlas, s, anim, game.frame(), x, y, false);
        }
        None => draw_body(game, looks, atlas, fb, cam, e),
    }
}

/// A Gondel cabin: sprite `gondel` (anim `idle`) at its feet, never
/// mirrored; else the cabin as a grey box with a 1 px hanger up to the
/// cable grip.
fn draw_gondel(
    game: &Game,
    looks: &EntityLooks,
    atlas: &Atlas,
    fb: &mut Framebuffer,
    cam: Camera,
    e: &Entity,
) {
    let (x, y) = feet_on_screen(e, cam);
    match looks.gondel {
        Some(s) => {
            let anim = s.anim("idle").unwrap_or(&s.anims[0]);
            draw_sprite(fb, atlas, s, anim, game.frame(), x, y, false);
        }
        None => {
            let top = e.y.to_px() - cam.y;
            fb.fill_rect(
                x,
                y - GONDEL_GRIP_PX,
                1,
                top - (y - GONDEL_GRIP_PX),
                OUTLINE,
            );
            draw_body(game, looks, atlas, fb, cam, e);
        }
    }
}

/// The Gondel's cable (the telegraph of its path, D-044): a 1 px `d` line
/// through the cable grips at both ends of its track, extended one tile
/// (along the major axis) beyond each end.
fn draw_cable(game: &Game, fb: &mut Framebuffer, cam: Camera, e: &Entity) {
    let Some([a, b]) = jnr_core::platform::track_ends(e, game.level()) else {
        return;
    };
    let grip = |(x, y): (jnr_core::Fx, jnr_core::Fx)| {
        (x.to_px() - cam.x, y.to_px() - GONDEL_GRIP_PX - cam.y)
    };
    let ((x0, y0), (x1, y1)) = (grip(a), grip(b));
    let (dx, dy) = (x1 - x0, y1 - y0);
    let major = dx.abs().max(dy.abs()).max(1);
    let tile = jnr_core::level::TILE_PX;
    let (ex, ey) = (dx * tile / major, dy * tile / major);
    draw_line(fb, x0 - ex, y0 - ey, x1 + ex, y1 + ey, SLATE_DARK);
}

/// A 1 px line from (x0, y0) to (x1, y1), both ends included (Bresenham).
fn draw_line(fb: &mut Framebuffer, x0: i32, y0: i32, x1: i32, y1: i32, rgba: u32) {
    let (dx, dy) = ((x1 - x0).abs(), -(y1 - y0).abs());
    let (sx, sy) = ((x1 - x0).signum(), (y1 - y0).signum());
    let (mut x, mut y, mut err) = (x0, y0, dx + dy);
    loop {
        fb.set(x, y, rgba);
        if x == x1 && y == y1 {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x += sx;
        }
        if e2 <= dx {
            err += dx;
            y += sy;
        }
    }
}

/// Debris and popups, in front of the player.
pub fn draw_front(
    game: &Game,
    looks: &EntityLooks,
    atlas: &Atlas,
    font: &Font,
    texts: &jnr_core::Texts,
    fb: &mut Framebuffer,
) {
    let cam = game.camera();
    for e in game.entities() {
        match e.kind {
            EntityKind::Debris | EntityKind::SteinDebris => {
                draw_body(game, looks, atlas, fb, cam, e)
            }
            EntityKind::Popup(p) => draw_popup(game, atlas, font, texts, fb, cam, e, p),
            _ => {}
        }
    }
}

/// Screen position of an entity's feet.
fn feet_on_screen(e: &Entity, cam: Camera) -> (i32, i32) {
    let (fx, fy) = e.feet();
    (fx.to_px() - cam.x, fy.to_px() - cam.y)
}

/// The sprite (first anim) at the feet, else the hitbox as a grey box.
fn draw_body(
    game: &Game,
    looks: &EntityLooks,
    atlas: &Atlas,
    fb: &mut Framebuffer,
    cam: Camera,
    e: &Entity,
) {
    let (x, y) = feet_on_screen(e, cam);
    match looks.sprite(e.kind) {
        Some(s) => {
            let left = e.vx < jnr_core::Fx::ZERO;
            draw_sprite(fb, atlas, s, &s.anims[0], game.frame(), x, y, left);
        }
        None => {
            let (bx, by) = (e.x.to_px() - cam.x, e.y.to_px() - cam.y);
            let (w, h) = (e.w.to_px(), e.h.to_px());
            fb.fill_rect(bx, by, w, h, grey_box_colour(e.kind));
            fb.outline_rect(bx, by, w, h, OUTLINE);
        }
    }
}

/// The catalog animation (and fixed frame, if any) for a pose: `(anim
/// name, frame)`; `None` = animate with the game frame.
pub fn pose_anim(pose: Pose) -> (&'static str, Option<usize>) {
    match pose {
        Pose::Walk => ("walk", None),
        Pose::Selfie { flash } => ("selfie", Some(usize::from(flash))),
        Pose::Fly => ("fly", None),
        Pose::Sit => ("sit", None),
        Pose::Tumble => ("tumble", None),
        Pose::Idle => ("idle", None),
        Pose::Windup => ("windup", None),
        Pose::Throw => ("throw", None),
        Pose::Squished => ("squished", None),
        Pose::Telegraph => ("telegraph", None),
        Pose::Charge => ("charge", None),
        Pose::Spin => ("spin", None),
        Pose::Inhale => ("inhale", None),
        Pose::Blow => ("blow", None),
        Pose::Ride => ("ride", None),
        Pose::Crash => ("crash", None),
        Pose::Buzz => ("buzz", None),
        Pose::Waggle => ("waggle", None),
        Pose::Stuck => ("stuck", None),
        Pose::Crouch => ("crouch", None),
        Pose::Hop => ("hop", None),
        Pose::Rest => ("rest", None),
        Pose::Wobble => ("wobble", None),
        Pose::Fall => ("fall", None),
    }
}

/// True for a Pilger (or a defeated one) placed with `lead = true`.
fn is_lead_pilger(game: &Game, e: &Entity) -> bool {
    matches!(
        e.kind,
        EntityKind::Enemy(EnemyKind::Pilger) | EntityKind::Defeated(EnemyKind::Pilger)
    ) && e
        .spawn
        .and_then(|i| game.level().entities.get(usize::from(i)))
        .is_some_and(|s| s.lead)
}

/// A Steinschlag niche: the rock (`stein` anim `rest` or `wobble`, nothing
/// while the niche is empty; grey box without the sprite) and, in the
/// telegraph, [`KIESEL_COUNT`] `kiesel` pebbles trickling down the rock's
/// column to the first solid or one-way tile (`kiesel` anim `fall` at their
/// feet, else 2×2 `m` dots), evenly spaced and falling [`KIESEL_SPEED_PX`]
/// px per frame.
fn draw_steinschlag(
    game: &Game,
    looks: &EntityLooks,
    atlas: &Atlas,
    fb: &mut Framebuffer,
    cam: Camera,
    e: &Entity,
) {
    let Some(pose) = e.pose(game.tuning()) else {
        return;
    };
    draw_enemy(game, looks, atlas, fb, cam, e);
    if pose != Pose::Wobble {
        return;
    }
    let (fx, fy) = e.feet();
    let top = fy.to_px();
    let len = jnr_core::enemies::rock_column_bottom(e, game.map()).to_px() - top;
    if len <= 0 {
        return;
    }
    let x = fx.to_px() - cam.x;
    let tick = game.frame();
    for i in 0..KIESEL_COUNT {
        let dy = ((tick as i32 % 1024) * KIESEL_SPEED_PX + i * len / KIESEL_COUNT) % len;
        let (px, py) = (x - 3 + 3 * i, top + dy - cam.y);
        match looks.kiesel {
            Some(s) => {
                let anim = s.anim("fall").unwrap_or(&s.anims[0]);
                draw_sprite(fb, atlas, s, anim, tick, px, py + 1, false);
            }
            None => fb.fill_rect(px - 1, py - 1, 2, 2, STONE_GREY),
        }
    }
}

/// An enemy, a defeated enemy or a projectile (thrown Mozartkugel, note):
/// the animation of its pose (missing anims fall back to the first one),
/// facing `dir` (the Fiaker and E-Biker art faces left and is never
/// mirrored), shifted by `Entity::shake_px` (the swarm's waggle). Without
/// a sprite: the hitbox as a grey box, flattened to its bottom quarter
/// when squished.
fn draw_enemy(
    game: &Game,
    looks: &EntityLooks,
    atlas: &Atlas,
    fb: &mut Framebuffer,
    cam: Camera,
    e: &Entity,
) {
    let pose = e.pose(game.tuning()).unwrap_or(Pose::Idle);
    let (sx, sy) = e.shake_px(game.tuning());
    let (x, y) = feet_on_screen(e, cam);
    let (x, y) = (x + sx, y + sy);
    let lead = is_lead_pilger(game, e);
    let sprite = match looks.pilger_lead {
        Some(s) if lead => Some(s),
        _ => looks.sprite(e.kind),
    };
    match sprite {
        Some(s) => {
            let (name, frame) = pose_anim(pose);
            let anim = s.anim(name).unwrap_or(&s.anims[0]);
            let tick = match frame {
                Some(i) => i as u32 * anim.ticks,
                None => game.frame(),
            };
            let fixed = matches!(
                e.kind,
                EntityKind::Enemy(EnemyKind::Fiaker | EnemyKind::EBiker)
                    | EntityKind::Defeated(EnemyKind::Fiaker | EnemyKind::EBiker)
            );
            let left = e.facing_left() && !fixed;
            draw_sprite(fb, atlas, s, anim, tick, x, y, left);
        }
        None => {
            let (w, full) = (e.w.to_px(), e.h.to_px());
            let h = if pose == Pose::Squished {
                (full / 4).max(1)
            } else {
                full
            };
            let bx = e.x.to_px() - cam.x + sx;
            let by = e.y.to_px() - cam.y + full - h + sy;
            fb.fill_rect(bx, by, w, h, grey_box_colour(e.kind));
            fb.outline_rect(bx, by, w, h, OUTLINE);
            if lead && pose != Pose::Squished {
                // The leader's pennant: a 1 px stick above the box with a
                // small red flag.
                fb.fill_rect(bx + w - 2, by - 8, 1, 8, OUTLINE);
                fb.fill_rect(bx + w - 1, by - 8, 3, 2, DIRNDL_RED);
            }
        }
    }
}

/// Haltestelle: anim `active` once reached (`idle` before), else a pole
/// with a yellow disc (filled green once reached).
fn draw_checkpoint(
    game: &Game,
    looks: &EntityLooks,
    atlas: &Atlas,
    fb: &mut Framebuffer,
    cam: Camera,
    e: &Entity,
) {
    let level = game.level();
    let reached = match (game.checkpoint(), e.spawn) {
        (Some(c), Some(s)) => level.entities[usize::from(s)].col <= level.entities[c].col,
        _ => false,
    };
    let (x, y) = feet_on_screen(e, cam);
    match looks.checkpoint {
        Some(s) => {
            let name = if reached { "active" } else { "idle" };
            let anim: &AtlasAnim = s.anim(name).unwrap_or(&s.anims[0]);
            draw_sprite(fb, atlas, s, anim, game.frame(), x, y, false);
        }
        None => {
            fb.fill_rect(x - 1, y - 32, 2, 32, PINE_DARK);
            let disc = if reached { PINE_DARK } else { MOZART_YELLOW };
            fb.fill_rect(x - 5, y - 32, 10, 10, disc);
            fb.outline_rect(x - 5, y - 32, 10, 10, PINE_DARK);
        }
    }
}

/// A popup: the score as digits, or its text, centred and floating up.
#[allow(clippy::too_many_arguments)]
fn draw_popup(
    game: &Game,
    atlas: &Atlas,
    font: &Font,
    texts: &jnr_core::Texts,
    fb: &mut Framebuffer,
    cam: Camera,
    e: &Entity,
    p: Popup,
) {
    let (x, _) = feet_on_screen(e, cam);
    let y = e.y.to_px() - cam.y - e.popup_rise_px(game.tuning());
    let mut digits = [0u8; 10];
    let text = match p.text_key() {
        Some(key) => texts.get(key),
        None => {
            let Popup::Score(n) = p else { return };
            std::borrow::Cow::Borrowed(format_u32(n, &mut digits))
        }
    };
    let w = font.measure(&text);
    if font.is_empty() {
        // No font: a small white marker so popups stay visible in grey-box.
        fb.fill_rect(x - 4, y, 8, 3, WHITE);
        return;
    }
    // Full `k` outline like the HUD (D-037).
    font.draw_text_outlined(fb, atlas, &text, x - w / 2, y, WHITE, OUTLINE);
}

/// Decimal digits of `n` in `buf` without allocating.
fn format_u32(mut n: u32, buf: &mut [u8; 10]) -> &str {
    let mut i = buf.len();
    loop {
        i -= 1;
        buf[i] = b'0' + (n % 10) as u8;
        n /= 10;
        if n == 0 {
            break;
        }
    }
    std::str::from_utf8(&buf[i..]).unwrap_or("?")
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::palette::SKY_LIGHT;
    use crate::scene::{RenderContext, player_shown_big, render};
    use jnr_core::{Fx, InputFrame, Level, Power, Texts, Tuning};

    fn tuning() -> Tuning {
        Tuning::parse(include_str!("../../../content/tuning.toml")).unwrap()
    }

    const GRID: &str = "\
....................
....................
....................
....................
....................
....................
..?.B...............
....................
..P.....o...t.......
####################
";

    /// 20x10 tiles: box at (2, 6), brick at (4, 6), Oblate at (8, 8),
    /// Selfie-Tourist at (12, 8); the camera stays at (0, 0).
    fn game() -> Game {
        let text = format!(
            "[meta]\nid = \"t\"\ntime_limit = 300\n[legend]\n\"#\" = \"solid\"\n\
             \"B\" = \"brick\"\n\"?\" = {{ tile = \"oblaten_box\" }}\n\
             \"o\" = {{ entity = \"oblate\" }}\n\"t\" = {{ entity = \"selfie_tourist\" }}\n\
             \"P\" = {{ entity = \"player_start\" }}\n[map]\ngrid = '''\n{GRID}'''\n"
        );
        Game::new(&Level::parse(&text).unwrap(), &tuning(), 1)
    }

    fn rendered(game: &Game) -> Framebuffer {
        let atlas = crate::testutil::atlas(&[]);
        let texts = Texts::default();
        let ctx = RenderContext::new(&atlas, &texts, game.level());
        let mut fb = Framebuffer::new();
        render(game, &ctx, &mut fb);
        fb
    }

    #[test]
    fn popups_have_the_full_outline() {
        use crate::font::{CHARSET, FONT_SPRITE, GLYPH_ANIM};
        use crate::testutil::{self, image};
        // Every glyph a full 8x8 block (advance 9).
        let glyphs = CHARSET.chars().map(|_| image(&["aaaaaaaa"; 8])).collect();
        let atlas = testutil::atlas(&[testutil::sprite(
            FONT_SPRITE,
            (0, 0),
            false,
            vec![(GLYPH_ANIM, 0, glyphs)],
        )]);
        let font = Font::from_atlas(&atlas);
        let g = game();
        let e = jnr_core::entity::popup(Popup::Score(100), Fx::from_px(160), Fx::from_px(80));
        let mut fb = testutil::fb();
        let cam = Camera { x: 0, y: 0 };
        let texts = Texts::default();
        draw_popup(
            &g,
            &atlas,
            &font,
            &texts,
            &mut fb,
            cam,
            &e,
            Popup::Score(100),
        );
        // „100" is 26 px wide, centred on x 160; top at y 80 minus the rise.
        let (left, top) = (160 - 13, 80 - e.popup_rise_px(g.tuning()));
        assert_eq!(fb.get(left, top), Some(WHITE));
        assert_eq!(fb.get(left - 1, top), Some(OUTLINE), "left");
        assert_eq!(fb.get(left - 1, top - 1), Some(OUTLINE), "top-left corner");
        assert_eq!(fb.get(left, top - 1), Some(OUTLINE), "top");
        assert_eq!(fb.get(left + 25, top + 8), Some(OUTLINE), "below");
        assert_eq!(fb.get(left - 2, top), Some(testutil::BG));
    }

    #[test]
    fn grey_box_blocks_entities_and_bumps() {
        let mut g = game();
        let fb = rendered(&g);
        assert_eq!(fb.get(2 * 16 + 8, 6 * 16 + 8), Some(MOZART_YELLOW), "box");
        assert_eq!(fb.get(4 * 16 + 8, 6 * 16 + 8), Some(TERRACOTTA), "brick");
        assert_eq!(fb.get(8 * 16 + 8, 8 * 16 + 8), Some(OBLATE_GOLD), "Oblate");
        assert_eq!(fb.get(12 * 16 + 8, 8 * 16 + 8), Some(LAKE_DEEP), "tourist");
        // Jump into the box: it empties and is drawn raised while bumped.
        let mut bumped = false;
        for _ in 0..30 {
            g.step(InputFrame::JUMP);
            if g.tile_bump_px(2, 6) == 4 {
                bumped = true;
                break;
            }
        }
        assert!(bumped);
        let fb = rendered(&g);
        assert_eq!(
            fb.get(2 * 16 + 8, 6 * 16 - 2),
            Some(OBLATE_BROWN),
            "raised 4 px"
        );
        assert_eq!(fb.get(2 * 16 + 8, 6 * 16 + 14), Some(SKY_LIGHT));
    }

    #[test]
    fn transformation_alternates_sizes_every_4_frames() {
        let t = tuning().player;
        let mut p = *game().player();
        assert!(!player_shown_big(&p, &t));
        p.power = Power::Big;
        p.h = Fx::from_px(t.hitbox_h_big);
        assert!(player_shown_big(&p, &t));
        // Growing: the old (small) size first, the new one 4 frames later.
        let shown: Vec<bool> = (0..t.transform_frames)
            .rev()
            .map(|left| {
                p.transform = left + 1;
                player_shown_big(&p, &t)
            })
            .collect();
        assert_eq!(
            &shown[..9],
            &[false, false, false, false, true, true, true, true, false]
        );
    }

    #[test]
    fn formats_numbers_without_allocating() {
        let mut b = [0u8; 10];
        assert_eq!(format_u32(0, &mut b), "0");
        assert_eq!(format_u32(200, &mut b), "200");
        assert_eq!(format_u32(u32::MAX, &mut b), "4294967295");
    }

    /// `0xRRGGBBAA` of palette character `ch` in `main.palette`.
    fn palette_colour(src: &str, ch: char) -> u32 {
        let line = src
            .lines()
            .find(|l| l.split_whitespace().next() == Some(ch.to_string().as_str()))
            .unwrap_or_else(|| panic!("`{ch}` is not in main.palette"));
        let hex = line.split_whitespace().nth(1).unwrap();
        (u32::from_str_radix(hex.trim_start_matches('#'), 16).unwrap() << 8) | 0xff
    }

    #[test]
    fn colours_match_main_palette() {
        let src = include_str!("../../../art/palettes/main.palette");
        for (ch, value) in [
            ('o', OBLATE_GOLD),
            ('p', PASTEL_PINK),
            ('O', OBLATE_BROWN),
            ('n', TERRACOTTA),
            ('Y', MOZART_YELLOW),
            ('G', PINE_DARK),
            ('c', CREAM),
            ('b', LAKE_DEEP),
            ('i', SILVER_LIGHT),
            ('R', WINE_DARK),
            ('j', PINE_SHADOW),
            ('T', LAKE_TEAL),
            ('r', DIRNDL_RED),
            ('v', MID_SAGE),
            ('E', SKIN_SHADE),
            ('d', SLATE_DARK),
            ('t', LAKE_TURQUOISE),
            ('y', MEADOW_LIGHT),
            ('e', SKIN_LIGHT),
            ('g', MEADOW),
            ('B', NAVY),
            ('F', FAR_DARK),
            ('f', FAR_LIGHT),
            ('m', STONE_GREY),
        ] {
            assert_eq!(value, palette_colour(src, ch), "palette `{ch}`");
        }
    }

    /// A 16x24 frame filled with test colour `c` (`a`, `b` or `c`).
    fn solid(c: char) -> jnr_artkit::Image {
        let row: String = std::iter::repeat_n(c, 16).collect();
        crate::testutil::image(&vec![row.as_str(); 24])
    }

    /// The Selfie-Tourist's selfie: frame 0 (stick up) for the whole stop,
    /// frame 1 (flash) only in the 2 frames from frame 24 of the stop; the
    /// walk anim otherwise; facing left (mirrored) while walking left.
    #[test]
    fn selfie_shows_the_flash_frame_for_two_frames() {
        use crate::testutil::{A, B, C};
        let tourist = crate::testutil::sprite(
            "selfie_tourist",
            (8, 24),
            true,
            vec![
                ("walk", 10, vec![solid('a'), solid('a')]),
                ("selfie", 24, vec![solid('b'), solid('c')]),
                ("squished", 0, vec![solid('a')]),
            ],
        );
        let atlas = crate::testutil::atlas(&[tourist]);
        let texts = Texts::default();
        let mut g = game();
        let shown = |g: &Game| {
            let ctx = RenderContext::new(&atlas, &texts, g.level());
            let mut fb = Framebuffer::new();
            render(g, &ctx, &mut fb);
            let e = g
                .entities()
                .iter()
                .find(|e| e.kind == EntityKind::Enemy(EnemyKind::SelfieTourist))
                .unwrap();
            let (fx, fy) = e.feet();
            fb.get(fx.to_px() - g.camera().x, fy.to_px() - 4 - g.camera().y)
        };
        let t = tuning().enemies;
        let mut seen = Vec::new();
        for f in 1..=300u32 {
            g.step(InputFrame::NONE);
            seen.push((f, shown(&g)));
        }
        let start = u32::from(t.tourist_selfie_period);
        let flash = start + u32::from(t.tourist_flash_at);
        for (f, colour) in seen {
            let want = if f >= flash && f < flash + 2 {
                C
            } else if f >= start && f < start + u32::from(t.tourist_selfie_frames) {
                B
            } else {
                A
            };
            assert_eq!(colour, Some(want), "frame {f}");
        }
    }

    /// The car uses its sprite at its feet and is never mirrored, also
    /// while it moves left.
    #[test]
    fn festungsbahn_is_drawn_with_its_sprite_unmirrored() {
        use crate::testutil::{A, B};
        let half: String = "a".repeat(24) + &"b".repeat(24);
        let car = crate::testutil::sprite(
            "festungsbahn",
            (24, 24),
            true,
            vec![(
                "idle",
                0,
                vec![crate::testutil::image(&vec![half.as_str(); 24])],
            )],
        );
        let atlas = crate::testutil::atlas(&[car]);
        let texts = Texts::default();
        let text = "[meta]\nid = \"t\"\ntime_limit = 300\n[legend]\n\"#\" = \"solid\"\n\
             \"F\" = { entity = \"festungsbahn\", to = [3, 0] }\n\
             \"P\" = { entity = \"player_start\" }\n[map]\ngrid = '''\n\
             ....................\n.....P..............\n....................\n\
             ....F...............\n####################\n'''\n";
        // The player starts above the car and falls onto it: it waits for
        // its rider (D-030), then moves right and back.
        let mut g = Game::new(&Level::parse(text).unwrap(), &tuning(), 1);
        let mut checked = 0;
        for _ in 0..420 {
            g.step(InputFrame::NONE);
            let c = *g
                .entities()
                .iter()
                .find(|e| e.kind == EntityKind::Festungsbahn)
                .unwrap();
            if c.vx == jnr_core::Fx::ZERO && checked > 0 {
                continue;
            }
            let ctx = RenderContext::new(&atlas, &texts, g.level());
            let mut fb = Framebuffer::new();
            render(&g, &ctx, &mut fb);
            let (x, y) = (c.x.to_px() - g.camera().x, c.y.to_px() - g.camera().y + 12);
            assert_eq!(fb.get(x + 2, y), Some(A), "left half, vx {:?}", c.vx);
            assert_eq!(fb.get(x + 45, y), Some(B), "right half");
            checked += 1;
        }
        assert!(checked > 200, "checked while moving both ways");
    }

    #[test]
    fn every_pose_has_a_catalog_animation() {
        use jnr_core::Pose;
        let names: Vec<&str> = [
            Pose::Walk,
            Pose::Selfie { flash: false },
            Pose::Selfie { flash: true },
            Pose::Fly,
            Pose::Sit,
            Pose::Tumble,
            Pose::Idle,
            Pose::Windup,
            Pose::Throw,
            Pose::Squished,
            Pose::Telegraph,
            Pose::Charge,
            Pose::Spin,
            Pose::Inhale,
            Pose::Blow,
            Pose::Ride,
            Pose::Crash,
            Pose::Buzz,
            Pose::Waggle,
            Pose::Crouch,
            Pose::Hop,
            Pose::Rest,
            Pose::Wobble,
            Pose::Fall,
        ]
        .into_iter()
        .map(|p| pose_anim(p).0)
        .collect();
        let catalog = include_str!("../../../docs/sprite-catalog.md");
        for n in names {
            assert!(
                catalog.contains(&format!("`{n}`")),
                "`{n}` not in the catalog"
            );
        }
        assert_eq!(pose_anim(Pose::Selfie { flash: true }), ("selfie", Some(1)));
        assert_eq!(
            pose_anim(Pose::Selfie { flash: false }),
            ("selfie", Some(0))
        );
    }

    /// A grey-box level with `marks` (row, col, legend char) on a 20x10
    /// map with ground in row 9 and the player at column 2.
    fn m2_game(marks: &[(usize, usize, char)]) -> Game {
        let mut rows: Vec<Vec<char>> = (0..10)
            .map(|r| vec![if r == 9 { '#' } else { '.' }; 20])
            .collect();
        rows[8][2] = 'P';
        for &(r, c, ch) in marks {
            rows[r][c] = ch;
        }
        let grid: String = rows
            .into_iter()
            .map(|r| r.into_iter().collect::<String>() + "\n")
            .collect();
        let text = format!(
            "[meta]\nid = \"t\"\ntime_limit = 300\n[legend]\n\"#\" = \"solid\"\n\
             \"g\" = {{ entity = \"gondel\", to = [4, -2] }}\n\
             \"s\" = {{ entity = \"bienenschwarm\", dir = 0, len = 2 }}\n\
             \"P\" = {{ entity = \"player_start\" }}\n[map]\ngrid = '''\n{grid}'''\n"
        );
        Game::new(&Level::parse(&text).unwrap(), &tuning(), 1)
    }

    /// The cable runs through both grips (52 px above the cabin's feet at
    /// each end of the track), one tile beyond each end; the grey-box
    /// cabin hangs from it on a 1 px hanger.
    #[test]
    fn the_gondel_cable_shows_the_whole_track() {
        // Cabin at column 6, row 7 (x 96..128, feet (112, 128)); the other
        // end is [4, -2] tiles away: feet (176, 96).
        let g = m2_game(&[(7, 6, 'g')]);
        let fb = rendered(&g);
        let (gx0, gy0) = (112, 128 - GONDEL_GRIP_PX);
        let (gx1, gy1) = (176, 96 - GONDEL_GRIP_PX);
        assert_eq!(fb.get(gx1, gy1), Some(SLATE_DARK), "far grip");
        assert_eq!(fb.get(gx1 + 16, gy1 - 8), Some(SLATE_DARK), "a tile beyond");
        assert_eq!(fb.get(gx0 - 16, gy0 + 8), Some(SLATE_DARK), "a tile before");
        assert_eq!(fb.get(gx0 + 32, gy0 - 16), Some(SLATE_DARK), "middle");
        assert_ne!(
            fb.get(gx1 + 18, gy1 - 9),
            Some(SLATE_DARK),
            "and no further"
        );
        assert_eq!(fb.get(gx0, gy0 + 10), Some(OUTLINE), "hanger");
        assert_eq!(fb.get(gx0, 128 - 12), Some(SKIN_LIGHT), "cabin");
    }

    /// The swarm's grey box shakes across its run (renderer only).
    #[test]
    fn the_swarm_grey_box_shakes_on_its_run() {
        let mut g = m2_game(&[(4, 6, 's')]);
        let shown_top = |g: &Game| {
            let fb = rendered(g);
            let e = g
                .entities()
                .iter()
                .find(|e| e.kind == EntityKind::Enemy(EnemyKind::Bienenschwarm))
                .unwrap();
            let cx = (e.x + e.w.div_int(2)).to_px();
            (0..80)
                .find(|&y| fb.get(cx, y) == Some(MEADOW_LIGHT))
                .map(|y| y - 1 - e.y.to_px())
        };
        g.step(InputFrame::NONE);
        // Frames 1-3 of the run: 2 px up (the run's left side), then down.
        assert_eq!(shown_top(&g), Some(-2));
        for _ in 0..3 {
            g.step(InputFrame::NONE);
        }
        assert_eq!(shown_top(&g), Some(2));
    }

    /// A grey-box M3 level (20x10, ground in row 9, player at column 2):
    /// a Steinschlag niche at (3, 6) (in range), a `lead` Pilger at
    /// (8, 12) and a plain one at (8, 14).
    fn m3_game() -> Game {
        let text = "[meta]\nid = \"t\"\ntime_limit = 300\n[legend]\n\"#\" = \"solid\"\n\
             \"r\" = { entity = \"steinschlag\" }\n\
             \"L\" = { entity = \"pilger\", lead = true }\n\"p\" = { entity = \"pilger\" }\n\
             \"P\" = { entity = \"player_start\" }\n[map]\ngrid = '''\n\
             ....................\n....................\n....................\n\
             ......r.............\n....................\n....................\n\
             ....................\n....................\n..P.........L.p.....\n\
             ####################\n'''\n";
        Game::new(&Level::parse(text).unwrap(), &tuning(), 1)
    }

    /// The niche draws its resting rock, pebbles trickle down its column
    /// during the telegraph, the niche is empty after the drop; the lead
    /// Pilger's grey box carries a red pennant, the plain one doesn't.
    #[test]
    fn steinschlag_telegraph_pebbles_and_the_lead_pennant() {
        let mut g = m3_game();
        let col_x = 6 * 16 + 8;
        let pebbles = |fb: &Framebuffer| {
            (64..144)
                .filter(|&y| (col_x - 4..col_x + 4).any(|x| fb.get(x, y) == Some(STONE_GREY)))
                .count()
        };
        g.step(InputFrame::NONE);
        let fb = rendered(&g);
        assert_eq!(
            fb.get(col_x, 3 * 16 + 8),
            Some(FAR_DARK),
            "rock in the niche"
        );
        assert!(pebbles(&fb) >= 4, "telegraph pebbles");
        let pilgers: Vec<Entity> = g
            .entities()
            .iter()
            .filter(|e| e.kind == EntityKind::Enemy(EnemyKind::Pilger))
            .copied()
            .collect();
        let pennant = |e: &Entity| {
            let (x, y) = (e.right().to_px() - g.camera().x, e.y.to_px() - g.camera().y);
            fb.get(x, y - 8) == Some(DIRNDL_RED)
        };
        assert!(pennant(&pilgers[0]) && !pennant(&pilgers[1]));
        // Frame 56: the rock dropped at 41 and has fallen past the niche.
        for _ in 0..55 {
            g.step(InputFrame::NONE);
        }
        let fb = rendered(&g);
        assert_ne!(fb.get(col_x, 3 * 16 + 8), Some(FAR_DARK), "niche empty");
        assert_eq!(pebbles(&fb), 0, "no pebbles after the drop");
        assert!(
            g.entities().iter().any(|e| e.kind == EntityKind::Stein),
            "the falling rock"
        );
    }

    #[test]
    fn every_popup_text_key_exists() {
        let texts = Texts::parse(include_str!("../../../content/text/de.toml")).unwrap();
        for p in [
            Popup::ExtraLife,
            Popup::Checkpoint,
            Popup::Wurstkranz,
            Popup::Hoppala,
        ] {
            let key = p.text_key().unwrap();
            assert!(texts.contains(key), "{key}");
        }
    }

    #[test]
    fn every_kind_has_a_distinct_grey_box_colour() {
        let kinds = [
            EntityKind::Oblate,
            EntityKind::Mozartkugel,
            EntityKind::Zaunerstollen,
            EntityKind::Debris,
            EntityKind::Checkpoint,
            EntityKind::Sign,
            EntityKind::Enemy(EnemyKind::SelfieTourist),
            EntityKind::Enemy(EnemyKind::Taube),
            EntityKind::Enemy(EnemyKind::MozartkugelVerkaeufer),
            EntityKind::Enemy(EnemyKind::Fiaker),
            EntityKind::Festungsbahn,
            EntityKind::Enemy(EnemyKind::NordicWalker),
            EntityKind::Enemy(EnemyKind::Blasmusiker),
            EntityKind::Enemy(EnemyKind::EBiker),
            EntityKind::Enemy(EnemyKind::Bienenschwarm),
            EntityKind::Note,
            EntityKind::Gondel,
            EntityKind::Enemy(EnemyKind::Pilger),
            EntityKind::Enemy(EnemyKind::Gams),
            EntityKind::Stein,
            EntityKind::SteinDebris,
        ];
        for (i, a) in kinds.iter().enumerate() {
            for b in &kinds[i + 1..] {
                assert_ne!(grey_box_colour(*a), grey_box_colour(*b), "{a:?} vs {b:?}");
            }
        }
    }
}
