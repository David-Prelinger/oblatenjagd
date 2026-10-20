//! Drawing entities (items, particles, popups, props, enemies) and the
//! grey-box looks of block tiles.
//!
//! Every entity uses its catalog sprite (`docs/sprite-catalog.md`) with the
//! origin at its feet when the atlas has it, else a grey box in a colour of
//! its own per kind. Sprites are resolved once in [`EntityLooks::new`]
//! (part of the `RenderContext`); drawing allocates nothing.
//!
//! Draw passes (called from `scene::render`):
//! 1. [`draw_behind_tiles`]: Schafbergbahn tracks, power-ups still rising
//!    out of their box
//! 2. tiles (scene), raised by `Game::tile_bump_px` while bumped
//! 3. [`draw_entities`], in three layers ([`Layer`], D-086): Gondel cables
//!    and sign boards first (so Oblaten and Resi are always in front of a
//!    board); then items, props, the Steinschlag niches (resting/wobbling
//!    rock and the `kiesel` trickle), the Souvenirstand vendors, the
//!    Festungsbahn, the Gondel cabins, the Schafbergbahn with its steam
//!    puffs, the Traundl boats and the Dampfer with the `gischt` spray of
//!    its turning paddle wheel; then enemies and projectiles (animation by
//!    `Entity::pose`, facing by `Entity::dir`, the swarm's shake by
//!    `Entity::shake_px`, a `lead` Pilger as `pilger_lead`, a `lead`
//!    Offizier as `offizier_lead`), falling rocks, the Operettensänger's
//!    high-C column (`hohes_c` rings), the Offizier's grey-box sabre (drawn
//!    from `enemies::sabre_zone`) and snow globes, so a hazard never hides
//!    behind a hull or car
//! 4. player (scene)
//! 5. [`draw_front`]: debris (bricks, rocks, snow-globe shards), then all
//!    popups on top (the last world pass, under the HUD)

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
/// `h` sky_haze: Schneekugel (the glass dome).
pub const SKY_HAZE: u32 = 0xdff1f0ff;
/// `S` sky_blue: the Souvenirstand vendor (apron).
pub const SKY_BLUE: u32 = 0x64aee6ff;

/// Pebbles in a Steinschlag's telegraph trickle (renderer only) ...
pub const KIESEL_COUNT: i32 = 3;
/// ... falling this many px per frame down the rock's column.
pub const KIESEL_SPEED_PX: i32 = 3;

/// The Gondel's cable grip above its feet (sprite-catalog: the cable
/// meets the 32×52 sprite at (16, 0), origin (16, 52)).
pub const GONDEL_GRIP_PX: i32 = 52;

/// `O` oblate_brown: the Schafbergbahn track's sleepers.
pub const SLEEPER: u32 = OBLATE_BROWN;
/// A sleeper (2 px wide) every this many px along the track.
pub const SLEEPER_EVERY_PX: i32 = 8;
/// The Schafbergbahn's rails are this many px apart (the upper one runs
/// along the body's bottom line, the rack between them).
pub const RAIL_GAP_PX: i32 = 3;
/// A steam puff leaves the chimney every this many frames of travel ...
pub const DAMPF_EVERY: u16 = 20;
/// ... and lives this many frames (`dampf` `puff`: 3 frames × 8 ticks),
/// rising 1 px per frame.
pub const DAMPF_FRAMES: u16 = 24;
/// Puffs alive at once: one every 20 frames, each for 24.
pub const MAX_PUFFS: usize = DAMPF_FRAMES.div_ceil(DAMPF_EVERY) as usize;
/// Height of the funnel above the roof (the `schafbergbahn` sprite's funnel:
/// x 0–5, rows 0–7 above the roof row 8, at the loco's left, downhill end;
/// the grey box draws a stub there) ...
pub const FUNNEL_PX: i32 = 8;
/// ... and its width.
pub const FUNNEL_W: i32 = 6;
/// Where a steam puff leaves (its bottom centre) relative to the body's
/// top-left corner: over the funnel's middle, 2 px below its cap's top, so
/// the first puff touches the cap (art-director, M4).
pub const DAMPF_CHIMNEY: (i32, i32) = (FUNNEL_W / 2, -FUNNEL_PX + 2);

/// The Traundl's grey-box rower (until the `traundl` art lands): a
/// `[w, h]` stub standing on the deck at this x from the hull's left edge
/// (the sprite keeps him within x 34–47, at the stern).
pub const TRAUNDL_ROWER: (i32, [i32; 2]) = (38, [6, 14]);
/// The Dampfer's grey-box funnel (until the `dampfer` art lands): `[w, h]`
/// above the deck at this x from the hull's left edge (sprite: x ~70–77).
pub const DAMPFER_FUNNEL: (i32, [i32; 2]) = (70, [8, 20]);
/// The two `gischt` sprites stand on the water line this many px left and
/// right of the paddle wheel's centre.
pub const GISCHT_SPREAD_PX: i32 = 8;

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
    reisebus: Option<&'a AtlasSprite>,
    schwan: Option<&'a AtlasSprite>,
    gelse: Option<&'a AtlasSprite>,
    regattaboot: Option<&'a AtlasSprite>,
    offizier: Option<&'a AtlasSprite>,
    offizier_lead: Option<&'a AtlasSprite>,
    stein: Option<&'a AtlasSprite>,
    stein_debris: Option<&'a AtlasSprite>,
    kiesel: Option<&'a AtlasSprite>,
    hackl: Option<&'a AtlasSprite>,
    hackl_thrown: Option<&'a AtlasSprite>,
    operettensaenger: Option<&'a AtlasSprite>,
    hohes_c: Option<&'a AtlasSprite>,
    souvenirstand: Option<&'a AtlasSprite>,
    schneekugel: Option<&'a AtlasSprite>,
    schneekugel_scherben: Option<&'a AtlasSprite>,
    schafbergbahn: Option<&'a AtlasSprite>,
    dampf: Option<&'a AtlasSprite>,
    traundl: Option<&'a AtlasSprite>,
    dampfer: Option<&'a AtlasSprite>,
    gischt: Option<&'a AtlasSprite>,
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
            reisebus: s(EnemyKind::Reisebus.sprite()),
            schwan: s(EnemyKind::Schwan.sprite()),
            gelse: s(EnemyKind::Gelse.sprite()),
            regattaboot: s(EnemyKind::Regattaboot.sprite()),
            offizier: s(EnemyKind::Offizier.sprite()),
            offizier_lead: s("offizier_lead"),
            stein: s("stein"),
            stein_debris: s("stein_debris"),
            kiesel: s("kiesel"),
            hackl: s("hackl"),
            hackl_thrown: s("hackl_thrown"),
            operettensaenger: s(EnemyKind::Operettensaenger.sprite()),
            hohes_c: s("hohes_c"),
            souvenirstand: s("souvenirstand"),
            schneekugel: s(EnemyKind::Schneekugel.sprite()),
            schneekugel_scherben: s("schneekugel_scherben"),
            schafbergbahn: s("schafbergbahn"),
            dampf: s("dampf"),
            traundl: s("traundl"),
            dampfer: s("dampfer"),
            gischt: s("gischt"),
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
                EnemyKind::Operettensaenger => self.operettensaenger,
                EnemyKind::Schneekugel => self.schneekugel,
                EnemyKind::Reisebus => self.reisebus,
                EnemyKind::Schwan => self.schwan,
                EnemyKind::Gelse => self.gelse,
                EnemyKind::Regattaboot => self.regattaboot,
                EnemyKind::Offizier => self.offizier,
            },
            EntityKind::Thrown => self.thrown,
            EntityKind::Note => self.note,
            EntityKind::Gondel => self.gondel,
            EntityKind::Steinschlag | EntityKind::Stein => self.stein,
            EntityKind::SteinDebris => self.stein_debris,
            EntityKind::Hackl => self.hackl,
            EntityKind::ThrownHackl => self.hackl_thrown,
            EntityKind::Souvenirstand => self.souvenirstand,
            EntityKind::SchneekugelScherben => self.schneekugel_scherben,
            EntityKind::Schafbergbahn => self.schafbergbahn,
            EntityKind::Traundl => self.traundl,
            EntityKind::Dampfer => self.dampfer,
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
            EnemyKind::Operettensaenger => SLATE_DARK,
            EnemyKind::Schneekugel => SKY_HAZE,
            EnemyKind::Reisebus => WHITE,
            EnemyKind::Schwan => WHITE,
            EnemyKind::Gelse => OUTLINE,
            EnemyKind::Regattaboot => CREAM,
            EnemyKind::Offizier => STONE_GREY,
        },
        EntityKind::Steinschlag | EntityKind::Stein => FAR_DARK,
        EntityKind::SteinDebris => FAR_LIGHT,
        EntityKind::Thrown => LEATHER_DARK,
        EntityKind::Note => SLATE_DARK,
        EntityKind::Festungsbahn => LAKE_TEAL,
        EntityKind::Gondel => SKIN_LIGHT,
        EntityKind::Hackl => PINE_DARK,
        EntityKind::ThrownHackl => SILVER_LIGHT,
        EntityKind::Souvenirstand => SKY_BLUE,
        EntityKind::SchneekugelScherben => WHITE,
        EntityKind::Schafbergbahn => OUTLINE,
        EntityKind::Traundl => LEATHER_DARK,
        EntityKind::Dampfer => WHITE,
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

/// Schafbergbahn tracks and power-ups still rising out of their box (drawn
/// before the tiles, so the rock steps and the box cover them).
pub fn draw_behind_tiles(game: &Game, looks: &EntityLooks, atlas: &Atlas, fb: &mut Framebuffer) {
    let cam = game.camera();
    for e in game.entities() {
        if e.kind == EntityKind::Schafbergbahn {
            draw_track(game, fb, cam, e);
        }
    }
    for e in game.entities() {
        if e.rising(game.tuning()) {
            draw_body(game, looks, atlas, fb, cam, e);
        }
    }
}

/// The draw layers of [`draw_entities`], back to front (D-086).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layer {
    /// Sign boards: behind everything else, so no board hides an Oblate.
    Signs,
    /// Items, props, checkpoints and the moving platforms.
    Middle,
    /// Enemies and their projectiles: in front of every hull and car.
    Enemies,
}

impl Layer {
    /// The layer an entity of `kind` is drawn in.
    pub fn of(kind: EntityKind) -> Layer {
        match kind {
            EntityKind::Sign => Layer::Signs,
            EntityKind::Enemy(_)
            | EntityKind::Defeated(_)
            | EntityKind::Thrown
            | EntityKind::Stein
            | EntityKind::Note
            | EntityKind::ThrownHackl => Layer::Enemies,
            _ => Layer::Middle,
        }
    }
}

/// Items, props and enemies (not rising power-ups, debris or popups), layer
/// by layer ([`Layer`]); within a layer in entity order.
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
    for layer in [Layer::Signs, Layer::Middle, Layer::Enemies] {
        for e in game.entities() {
            if Layer::of(e.kind) == layer {
                draw_entity(game, looks, atlas, font, texts, fb, cam, e);
            }
        }
    }
}

/// One entity of [`draw_entities`].
#[allow(clippy::too_many_arguments)]
fn draw_entity(
    game: &Game,
    looks: &EntityLooks,
    atlas: &Atlas,
    font: &Font,
    texts: &jnr_core::Texts,
    fb: &mut Framebuffer,
    cam: Camera,
    e: &Entity,
) {
    match e.kind {
        // The Durchkriechstein is drawn by its prop (D-057).
        EntityKind::Debris
        | EntityKind::SteinDebris
        | EntityKind::SchneekugelScherben
        | EntityKind::Popup(_)
        | EntityKind::Bump
        | EntityKind::Durchkriechstein => {}
        EntityKind::Enemy(EnemyKind::Operettensaenger) => {
            draw_high_c(game, looks, atlas, fb, cam, e);
            draw_enemy(game, looks, atlas, fb, cam, e);
        }
        EntityKind::Souvenirstand => draw_souvenirstand(game, looks, atlas, fb, cam, e),
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
        EntityKind::Schafbergbahn => draw_schafbergbahn(game, looks, atlas, fb, cam, e),
        EntityKind::Traundl => draw_traundl(game, looks, atlas, fb, cam, e),
        EntityKind::Dampfer => draw_dampfer(game, looks, atlas, fb, cam, e),
        _ if e.rising(game.tuning()) => {}
        _ => draw_body(game, looks, atlas, fb, cam, e),
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

/// The Schafbergbahn's track (the telegraph of its ride, D-067; layout by
/// the pixel-artist, M4), drawn behind the tiles and the train. Per screen
/// column X from one tile before the body's leftmost position to one tile
/// past its rightmost, with u = X − L (L: the body's left edge at its
/// spawn) and the upper rail row Y = B + 1 + [`jnr_core::platform::rail_offset`]
/// (B: the body's bottom edge, the feet y, at the spawn; the sprite's
/// wheels stand on that row, sprite-catalog "as built", and the train
/// follows it pixel for pixel, `platform::track_offset`): `d` rail at Y, `m` rack
/// at Y+1 (even u only) and Y+2, `d` rail at Y+3; `O` sleepers over
/// Y+1..=Y+4 where u mod 8 is 0 or 1 (the rails stay on top); a `d`
/// girder at Y+5 under the whole track (art-director, M4: one viaduct
/// line, not a bare band like the Gondel cable). The top rail is plain
/// `d` with no light lip, so the diagonal never reads as a walkable slope.
fn draw_track(game: &Game, fb: &mut Framebuffer, cam: Camera, e: &Entity) {
    let Some([a, b]) = jnr_core::platform::track_ends(e, game.level()) else {
        return;
    };
    let (dx, dy) = ((b.0 - a.0).to_px(), (b.1 - a.1).to_px());
    if dx == 0 {
        return;
    }
    let (l, feet) = (e.base_x.to_px(), a.1.to_px());
    let tile = jnr_core::level::TILE_PX;
    let (x0, x1) = (l.min(l + dx) - tile, l.max(l + dx) + e.w.to_px() + tile);
    for x in x0..x1 {
        let u = x - l;
        let y = feet + 1 + jnr_core::platform::rail_offset(u, dx, dy);
        let (sx, sy) = (x - cam.x, y - cam.y);
        if u.rem_euclid(2) == 0 {
            fb.set(sx, sy + 1, STONE_GREY);
        }
        fb.set(sx, sy + 2, STONE_GREY);
        if u.rem_euclid(SLEEPER_EVERY_PX) < 2 {
            fb.fill_rect(sx, sy + 1, 1, RAIL_GAP_PX + 1, SLEEPER);
        }
        fb.set(sx, sy, SLATE_DARK);
        fb.set(sx, sy + RAIL_GAP_PX, SLATE_DARK);
        fb.set(sx, sy + RAIL_GAP_PX + 2, SLATE_DARK);
    }
}

/// The Schafbergbahn: sprite `schafbergbahn` at its feet, never mirrored,
/// anim `ride` while it travels and `idle` while it stands (falling back
/// to the first anim); else the body as a grey box with a chimney stub at
/// the loco's (left) end. Then its steam puffs ([`steam_puffs`]): `dampf`
/// `puff`, else small white boxes.
fn draw_schafbergbahn(
    game: &Game,
    looks: &EntityLooks,
    atlas: &Atlas,
    fb: &mut Framebuffer,
    cam: Camera,
    e: &Entity,
) {
    let moving = matches!(
        e.state,
        jnr_core::platform::car::UP | jnr_core::platform::car::DOWN
    );
    match looks.schafbergbahn {
        Some(s) => {
            let (x, y) = feet_on_screen(e, cam);
            let name = if moving { "ride" } else { "idle" };
            let anim = s.anim(name).unwrap_or(&s.anims[0]);
            draw_sprite(fb, atlas, s, anim, game.frame(), x, y, false);
        }
        None => {
            let (left, top) = (e.x.to_px() - cam.x, e.y.to_px() - cam.y);
            fb.fill_rect(left, top - FUNNEL_PX, FUNNEL_W, FUNNEL_PX, SLATE_DARK);
            draw_body(game, looks, atlas, fb, cam, e);
        }
    }
    for (x, y, age) in steam_puffs(game, e).into_iter().flatten() {
        let (x, y) = (x - cam.x, y - cam.y);
        match looks.dampf {
            Some(s) => {
                let anim = s.anim("puff").unwrap_or(&s.anims[0]);
                draw_sprite(fb, atlas, s, anim, u32::from(age), x, y, false);
            }
            None => fb.fill_rect(x - 3, y - 6, 6, 6, WHITE),
        }
    }
}

/// A Traundl: sprite `traundl` at its feet, never mirrored (it rows bow
/// first both ways, sprite-catalog), anim `ride` while it moves and `idle`
/// while it waits at an end (falling back to the first anim); else the
/// hull as a grey box with a stub for the standing rower.
fn draw_traundl(
    game: &Game,
    looks: &EntityLooks,
    atlas: &Atlas,
    fb: &mut Framebuffer,
    cam: Camera,
    e: &Entity,
) {
    match looks.traundl {
        Some(s) => {
            let (x, y) = feet_on_screen(e, cam);
            let name = if e.vx != jnr_core::Fx::ZERO {
                "ride"
            } else {
                "idle"
            };
            let anim = s.anim(name).unwrap_or(&s.anims[0]);
            draw_sprite(fb, atlas, s, anim, game.frame(), x, y, false);
        }
        None => {
            let (left, top) = (e.x.to_px() - cam.x, e.y.to_px() - cam.y);
            let (dx, [w, h]) = TRAUNDL_ROWER;
            fb.fill_rect(left + dx, top - h, w, h, PINE_DARK);
            draw_body(game, looks, atlas, fb, cam, e);
        }
    }
}

/// The Dampfer: sprite `dampfer` at its feet, never mirrored, anim `ride`
/// while it travels and `idle` while it waits or is moored; else the hull as
/// a grey box with a funnel stub (no smoke: diesel since 1954). While it
/// travels, the paddle wheel's hurt zone (`platform::paddle_wheel`) gets
/// two `gischt` `spray` sprites on the water line (else white spray boxes
/// just under it), and in the grey box also the zone's outline.
fn draw_dampfer(
    game: &Game,
    looks: &EntityLooks,
    atlas: &Atlas,
    fb: &mut Framebuffer,
    cam: Camera,
    e: &Entity,
) {
    let wheel = jnr_core::platform::paddle_wheel(e, game.tuning());
    match looks.dampfer {
        Some(s) => {
            let (x, y) = feet_on_screen(e, cam);
            let name = if wheel.is_some() { "ride" } else { "idle" };
            let anim = s.anim(name).unwrap_or(&s.anims[0]);
            draw_sprite(fb, atlas, s, anim, game.frame(), x, y, false);
        }
        None => {
            let (left, top) = (e.x.to_px() - cam.x, e.y.to_px() - cam.y);
            let (dx, [w, h]) = DAMPFER_FUNNEL;
            fb.fill_rect(left + dx, top - h, w, h, OBLATE_GOLD);
            draw_body(game, looks, atlas, fb, cam, e);
            if let Some(z) = wheel {
                let (zx, zy) = (z.x.to_px() - cam.x, z.y.to_px() - cam.y);
                fb.outline_rect(zx, zy, z.w.to_px(), z.h.to_px(), SLATE_DARK);
            }
        }
    }
    let Some(z) = wheel else {
        return;
    };
    let (cx, water) = (
        (z.x + z.w.div_int(2)).to_px() - cam.x,
        e.feet().1.to_px() - cam.y,
    );
    for x in [cx - GISCHT_SPREAD_PX, cx + GISCHT_SPREAD_PX] {
        match looks.gischt {
            Some(s) => {
                let anim = s.anim("spray").unwrap_or(&s.anims[0]);
                draw_sprite(fb, atlas, s, anim, u32::from(e.count), x, water, false);
            }
            None => {
                // Droplets just under the water line (the grey hull covers
                // the rows above it), bobbing 0–2 px with the wheel.
                let down = i32::from(e.count / 4 % 3);
                fb.fill_rect(x - 2, water + 1 + down, 4, 3, WHITE);
            }
        }
    }
}

/// The train's live steam puffs as (bottom centre x, y in level px, age in
/// frames), oldest first: while it travels, one leaves the chimney top
/// ([`DAMPF_CHIMNEY`]) on travel frame 1 and every [`DAMPF_EVERY`] frames
/// after, stays where it left the chimney (the train moves on) and rises
/// 1 px per frame for [`DAMPF_FRAMES`]. Computed from the train's state and
/// counter alone, so the renderer keeps no particles; a standing train has
/// none. At most [`MAX_PUFFS`] live at once (no allocation).
pub fn steam_puffs(game: &Game, e: &Entity) -> [Option<(i32, i32, u16)>; MAX_PUFFS] {
    use jnr_core::platform::{car, car_position, car_timing};
    let mut puffs = [None; MAX_PUFFS];
    if !matches!(e.state, car::UP | car::DOWN) {
        return puffs;
    }
    let spawn = e
        .spawn
        .and_then(|i| game.level().entities.get(usize::from(i)));
    let timing = car_timing(e.kind, spawn, game.tuning());
    let first = e.count.saturating_sub(DAMPF_FRAMES - 1).max(1);
    let emits = (first..=e.count).filter(|c| (c - 1) % DAMPF_EVERY == 0);
    for (slot, emit) in puffs.iter_mut().zip(emits) {
        let (x, y) = car_position(e.kind, (e.base_x, e.base_y), spawn, e.state, emit, timing);
        let age = e.count - emit;
        let (cx, cy) = DAMPF_CHIMNEY;
        *slot = Some((x.to_px() + cx, y.to_px() + cy - i32::from(age), age));
    }
    puffs
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

/// Debris and popups, in front of the player: first all debris, then all
/// popups, so a score or text popup is never cut by a shard's outline
/// (the last world pass; the HUD draws over it).
pub fn draw_front(
    game: &Game,
    looks: &EntityLooks,
    atlas: &Atlas,
    font: &Font,
    texts: &jnr_core::Texts,
    fb: &mut Framebuffer,
) {
    draw_front_of(game, game.entities(), looks, atlas, font, texts, fb);
}

/// [`draw_front`] for the given entities (any order).
fn draw_front_of(
    game: &Game,
    entities: &[Entity],
    looks: &EntityLooks,
    atlas: &Atlas,
    font: &Font,
    texts: &jnr_core::Texts,
    fb: &mut Framebuffer,
) {
    let cam = game.camera();
    for e in entities {
        if let EntityKind::Debris | EntityKind::SteinDebris | EntityKind::SchneekugelScherben =
            e.kind
        {
            draw_body(game, looks, atlas, fb, cam, e);
        }
    }
    for e in entities {
        if let EntityKind::Popup(p) = e.kind {
            draw_popup(game, atlas, font, texts, fb, cam, e, p);
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
        Pose::Sing => ("sing", None),
        Pose::Roll => ("roll", None),
        Pose::Drive => ("drive", None),
        Pose::Fotostopp { flash } => ("fotostopp", Some(usize::from(flash))),
        Pose::Wait => ("wait", None),
        Pose::Leave => ("leave", None),
        Pose::March => ("march", None),
        Pose::Habtacht => ("habtacht", None),
        Pose::Praesentiert => ("praesentiert", None),
    }
}

/// True for a Pilger or an Offizier (or a defeated one) placed with
/// `lead = true`.
fn is_lead(game: &Game, e: &Entity) -> bool {
    matches!(
        e.kind,
        EntityKind::Enemy(EnemyKind::Pilger | EnemyKind::Offizier)
            | EntityKind::Defeated(EnemyKind::Pilger | EnemyKind::Offizier)
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
    // A hovering Gelse bobs (renderer only, D-079).
    let sy = if pose == Pose::Wait {
        sy + gelse_bob_px(game.frame())
    } else {
        sy
    };
    let (x, y) = feet_on_screen(e, cam);
    let (x, y) = (x + sx, y + sy);
    let lead = is_lead(game, e);
    let lead_sprite = match e.kind {
        EntityKind::Enemy(EnemyKind::Offizier) | EntityKind::Defeated(EnemyKind::Offizier) => {
            looks.offizier_lead
        }
        _ => looks.pilger_lead,
    };
    let sprite = match lead_sprite {
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
            // These only ever move left: their art faces left.
            let fixed = matches!(
                e.kind,
                EntityKind::Enemy(
                    EnemyKind::Fiaker
                        | EnemyKind::EBiker
                        | EnemyKind::Schneekugel
                        | EnemyKind::Reisebus
                        | EnemyKind::Regattaboot
                ) | EntityKind::Defeated(
                    EnemyKind::Fiaker | EnemyKind::EBiker | EnemyKind::Schneekugel
                )
            );
            let left = e.facing_left() && !fixed;
            // The Regattaboot's origin is its sprite's bottom (its cell's
            // bottom), below the keel (the hitbox bottom).
            let y = if e.kind == EntityKind::Enemy(EnemyKind::Regattaboot) {
                y + jnr_core::level::TILE_PX - game.tuning().enemies.regattaboot_keel_px
            } else {
                y
            };
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
            if e.kind == EntityKind::Enemy(EnemyKind::Reisebus) {
                draw_bus_marks(fb, bx, by, w, h, pose, game.frame());
            }
            if e.kind == EntityKind::Enemy(EnemyKind::Regattaboot) {
                draw_boat_marks(fb, bx, by, w, h, pose, game.frame());
            }
            if e.kind == EntityKind::Enemy(EnemyKind::Schwan) && pose == Pose::Telegraph {
                // The hiss: raised wings as two white posts over the box.
                fb.fill_rect(bx + 1, by - 4, 3, 4, WHITE);
                fb.fill_rect(bx + w - 4, by - 4, 3, 4, WHITE);
            }
            let officer = e.kind == EntityKind::Enemy(EnemyKind::Offizier);
            if officer {
                draw_officer_marks(game, fb, cam, e, pose, lead);
            } else if lead && pose != Pose::Squished {
                // The leader's pennant: a 1 px stick above the box with a
                // small red flag.
                fb.fill_rect(bx + w - 2, by - 8, 1, 8, OUTLINE);
                fb.fill_rect(bx + w - 1, by - 8, 3, 2, DIRNDL_RED);
            }
        }
    }
}

/// Grey-box marks of an Offizier (no `offizier` sprite yet): in
/// „Habt Acht!" a yellow hilt at his side (the telegraph); in
/// „Präsentiert!" the sabre drawn from [`jnr_core::enemies::sabre_zone`]
/// (a white blade with a silver outline, exactly the hurting pixels); the
/// flag bearer (`lead`) carries a 1 px staff with a black-and-yellow flag
/// at the box's far side, dipped in „Habt Acht!", which stays ≥ 2 px clear
/// of the blade's column.
fn draw_officer_marks(
    game: &Game,
    fb: &mut Framebuffer,
    cam: Camera,
    e: &Entity,
    pose: Pose,
    lead: bool,
) {
    let (bx, by) = (e.x.to_px() - cam.x, e.y.to_px() - cam.y);
    let w = e.w.to_px();
    let front = if e.facing_left() { bx - 1 } else { bx + w - 1 };
    match pose {
        Pose::Habtacht => fb.fill_rect(front, by + 10, 2, 2, MOZART_YELLOW),
        Pose::Praesentiert => {
            if let Some(z) = jnr_core::enemies::sabre_zone(e, game.tuning()) {
                let (zx, zy) = (z.x.to_px() - cam.x, z.y.to_px() - cam.y);
                let (zw, zh) = (z.w.to_px(), z.h.to_px());
                fb.fill_rect(zx, zy, zw, zh, WHITE);
                fb.outline_rect(zx, zy, zw, zh, SILVER_LIGHT);
            }
        }
        _ => {}
    }
    if lead {
        // The staff on the side away from his front, outside the box.
        let staff = if e.facing_left() { bx + w + 1 } else { bx - 2 };
        let flag_x = if e.facing_left() {
            staff + 1
        } else {
            staff - 4
        };
        let top = if pose == Pose::Habtacht {
            by - 2
        } else {
            by - 10
        };
        fb.fill_rect(staff, top, 1, by + 8 - top, OUTLINE);
        fb.fill_rect(flag_x, top, 4, 2, OUTLINE);
        fb.fill_rect(flag_x, top + 2, 4, 2, MOZART_YELLOW);
    }
}

/// An Operettensänger's high-C column ([`jnr_core::enemies::high_c_column`])
/// while it sounds: `hohes_c` rings (anim `ring`, 8 px slices) stacked
/// every 8 px from his hitbox top up, alternate rings half a period apart.
/// A column clipped by a solid tile at a height that is no multiple of 8
/// gets one more ring hung from its top (overlapping the ring below), so
/// every hurting pixel lies in a ring's slice and none pokes into the tile.
/// Without the sprite: the column as a white box with an `i` outline.
fn draw_high_c(
    game: &Game,
    looks: &EntityLooks,
    atlas: &Atlas,
    fb: &mut Framebuffer,
    cam: Camera,
    e: &Entity,
) {
    let Some(c) = jnr_core::enemies::high_c_column(e, game.map(), game.tuning()) else {
        return;
    };
    let (x, y) = (c.x.to_px() - cam.x, c.y.to_px() - cam.y);
    let (w, h) = (c.w.to_px(), c.h.to_px());
    match looks.hohes_c {
        Some(s) => {
            let anim = s.anim("ring").unwrap_or(&s.anims[0]);
            let cx = x + w / 2;
            let full = h / 8;
            for i in 0..full {
                let tick = game.frame() + i as u32 * anim.ticks;
                draw_sprite(fb, atlas, s, anim, tick, cx, y + h - 8 * i, false);
            }
            if h % 8 != 0 {
                // The partial top: one more ring whose slice starts at the
                // clipped top (a column under 8 px keeps it on his head).
                let tick = game.frame() + full as u32 * anim.ticks;
                let feet = if full > 0 { y + 8 } else { y + h };
                draw_sprite(fb, atlas, s, anim, tick, cx, feet, false);
            }
        }
        None => {
            fb.fill_rect(x, y, w, h, WHITE);
            fb.outline_rect(x, y, w, h, SILVER_LIGHT);
        }
    }
}

/// A Souvenirstand's vendor: `souvenirstand` (anim `idle`, or `telegraph`
/// while it holds up a globe) with its origin on the top edge of the
/// counter's first tile (one column right of the spawner cell, the spawner
/// row's top), never mirrored; without the sprite a 12×12 `S` box there,
/// with a 4×4 white globe above it in the telegraph.
fn draw_souvenirstand(
    game: &Game,
    looks: &EntityLooks,
    atlas: &Atlas,
    fb: &mut Framebuffer,
    cam: Camera,
    e: &Entity,
) {
    let pose = e.pose(game.tuning()).unwrap_or(Pose::Idle);
    let x = e.right().to_px() + 8 - cam.x;
    let y = e.y.to_px() - cam.y;
    match looks.souvenirstand {
        Some(s) => {
            let (name, _) = pose_anim(pose);
            let anim = s.anim(name).unwrap_or(&s.anims[0]);
            draw_sprite(fb, atlas, s, anim, game.frame(), x, y, false);
        }
        None => {
            fb.fill_rect(x - 6, y - 12, 12, 12, SKY_BLUE);
            fb.outline_rect(x - 6, y - 12, 12, 12, OUTLINE);
            if pose == Pose::Telegraph {
                fb.fill_rect(x - 2, y - 17, 4, 4, WHITE);
                fb.outline_rect(x - 2, y - 17, 4, 4, OUTLINE);
            }
        }
    }
}

/// The Reisebus grey box's marks (until its art arrives, D-070): a dark
/// window band along the top half, hazard lights (`Y`) at both lower
/// corners that blink every [`BUS_HAZARD_TICKS`] frames while it
/// telegraphs, and three white camera flashes in the windows on the photo
/// stop's flash frames.
fn draw_bus_marks(fb: &mut Framebuffer, x: i32, y: i32, w: i32, h: i32, pose: Pose, tick: u32) {
    fb.fill_rect(x + 2, y + 3, w - 4, h / 3, SLATE_DARK);
    match pose {
        Pose::Telegraph if (tick / BUS_HAZARD_TICKS).is_multiple_of(2) => {
            fb.fill_rect(x + 1, y + h - 5, 3, 3, MOZART_YELLOW);
            fb.fill_rect(x + w - 4, y + h - 5, 3, 3, MOZART_YELLOW);
        }
        Pose::Fotostopp { flash: true } => {
            for i in 1..=3 {
                fb.fill_rect(x + i * w / 4 - 1, y + 4, 3, 3, WHITE);
            }
        }
        _ => {}
    }
}

/// The Regattaboot grey box's marks (until its art arrives, D-080): a
/// dark mast line up the middle of the sail and, while it telegraphs, the
/// start flag (`r`) at the stern blinking every [`BUS_HAZARD_TICKS`]
/// frames; a `Y` bow wave on the left while it races.
fn draw_boat_marks(fb: &mut Framebuffer, x: i32, y: i32, w: i32, h: i32, pose: Pose, tick: u32) {
    fb.fill_rect(x + w / 2, y + 1, 1, h - 2, SLATE_DARK);
    match pose {
        Pose::Telegraph if (tick / BUS_HAZARD_TICKS).is_multiple_of(2) => {
            fb.fill_rect(x + w - 5, y + h - 20, 4, 3, DIRNDL_RED);
        }
        Pose::Drive => fb.fill_rect(x - 2, y + h - 14, 2, 3, MOZART_YELLOW),
        _ => {}
    }
}

/// A hovering Gelse's bob (renderer only, D-079): −2..=2 px, one step
/// every 4 frames.
pub fn gelse_bob_px(frame: u32) -> i32 {
    const BOB: [i32; 8] = [0, 1, 2, 1, 0, -1, -2, -1];
    BOB[(frame / 4 % 8) as usize]
}

/// Frames per blink of the grey-box bus's hazard lights (the catalog's
/// `telegraph` anim: 2 frames of 6 ticks).
pub const BUS_HAZARD_TICKS: u32 = 6;

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
    fn popups_draw_over_debris() {
        use crate::font::{CHARSET, FONT_SPRITE, GLYPH_ANIM};
        use crate::testutil::{self, image};
        // Every glyph a full 8x8 block; the shards a 32x16 block of `b`
        // with the origin at its bottom centre.
        let glyphs = CHARSET.chars().map(|_| image(&["aaaaaaaa"; 8])).collect();
        let atlas = testutil::atlas(&[
            testutil::sprite(FONT_SPRITE, (0, 0), false, vec![(GLYPH_ANIM, 0, glyphs)]),
            testutil::sprite(
                "schneekugel_scherben",
                (16, 16),
                false,
                vec![("idle", 0, vec![image(&[&*"b".repeat(32); 16])])],
            ),
        ]);
        let font = Font::from_atlas(&atlas);
        let texts = Texts::default();
        let looks = EntityLooks::new(&atlas);
        let g = game();
        let popup = jnr_core::entity::popup(Popup::Score(200), Fx::from_px(160), Fx::from_px(80));
        // A shard after the popup in entity order, its feet below the
        // popup so its block covers the digits.
        let (fx, fy) = (Fx::from_px(160), Fx::from_px(88));
        let mut shard = popup;
        shard.kind = EntityKind::SchneekugelScherben;
        shard.x = fx - shard.w.div_int(2);
        shard.y = fy - shard.h;
        let mut fb = testutil::fb();
        draw_front_of(&g, &[popup, shard], &looks, &atlas, &font, &texts, &mut fb);
        let (left, top) = (160 - 13, 80 - popup.popup_rise_px(g.tuning()));
        assert_eq!(fb.get(left, top), Some(WHITE), "digit");
        assert_eq!(fb.get(left - 1, top), Some(OUTLINE), "digit outline");
        assert_eq!(fb.get(left - 2, top), Some(testutil::B), "shard beside it");
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
            ('h', SKY_HAZE),
            ('S', SKY_BLUE),
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

    /// A column clipped by a solid tile 26 px above the singer's head (not
    /// a multiple of 8): with a ring sprite that fills its whole 8 px slice,
    /// every hurting row is covered, and nothing is drawn above the column
    /// (in the tile) or below it (on his head).
    #[test]
    fn a_clipped_high_c_column_is_covered_up_to_its_top() {
        use crate::testutil::A;
        let full = "a".repeat(16);
        let ring = crate::testutil::sprite(
            "hohes_c",
            (8, 8),
            false,
            vec![("ring", 4, vec![crate::testutil::image(&[full.as_str(); 8])])],
        );
        let atlas = crate::testutil::atlas(&[ring]);
        let texts = Texts::default();
        let rows = [
            "....................",
            "....................",
            "....................",
            "....................",
            "....................",
            "....................",
            "....................",
            "....................",
            "..........#.........",
            "....................",
            "....................",
            "..P.......o.........",
            "####################",
            "####################",
        ];
        let text = format!(
            "[meta]\nid = \"t\"\ntime_limit = 300\n[legend]\n\"#\" = \"solid\"\n\
             \"o\" = {{ entity = \"operettensaenger\" }}\n\
             \"P\" = {{ entity = \"player_start\" }}\n[map]\ngrid = '''\n{}\n'''\n",
            rows.join("\n")
        );
        let mut g = Game::new(&Level::parse(&text).unwrap(), &tuning(), 1);
        for _ in 0..60 {
            g.step(InputFrame::NONE);
        }
        let singer = *g
            .entities()
            .iter()
            .find(|e| e.kind == EntityKind::Enemy(EnemyKind::Operettensaenger))
            .unwrap();
        let c = jnr_core::enemies::high_c_column(&singer, g.map(), g.tuning()).unwrap();
        assert_eq!(c.h.to_px(), 26, "clipped at the tile, 26 % 8 = 2");
        let ctx = RenderContext::new(&atlas, &texts, g.level());
        let mut fb = Framebuffer::new();
        render(&g, &ctx, &mut fb);
        let cam = g.camera();
        let cx = (c.x + c.w.div_int(2)).to_px() - cam.x;
        let top = c.y.to_px() - cam.y;
        let bottom = top + c.h.to_px();
        for y in top..bottom {
            assert_eq!(fb.get(cx, y), Some(A), "row {} of the column", y - top);
        }
        assert_ne!(fb.get(cx, top - 1), Some(A), "nothing in the tile");
        assert_ne!(fb.get(cx, bottom), Some(A), "nothing on his head");
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
            Pose::Sing,
            Pose::Roll,
            Pose::Wait,
            Pose::Leave,
            Pose::March,
            Pose::Habtacht,
            Pose::Praesentiert,
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

    #[test]
    fn the_grey_box_bus_shows_hazard_lights_and_photo_flashes() {
        use jnr_core::Pose;
        let catalog = include_str!("../../../docs/sprite-catalog.md");
        for p in [Pose::Drive, Pose::Fotostopp { flash: false }] {
            let n = pose_anim(p).0;
            assert!(
                catalog.contains(&format!("`{n}`")),
                "`{n}` not in the catalog"
            );
        }
        assert_eq!(
            pose_anim(Pose::Fotostopp { flash: true }),
            ("fotostopp", Some(1))
        );
        assert_eq!(
            grey_box_colour(EntityKind::Enemy(EnemyKind::Reisebus)),
            WHITE
        );
        // A 60×28 box at (100, 100): lights at its lower corners, flashes
        // in the window band.
        let marks = |pose: Pose, tick: u32| {
            let mut fb = Framebuffer::new();
            draw_bus_marks(&mut fb, 100, 100, 60, 28, pose, tick);
            (
                fb.get(102, 124),
                fb.get(157, 124),
                fb.get(115, 105),
                fb.get(130, 105),
            )
        };
        let (l, r, w1, w2) = marks(Pose::Telegraph, 0);
        assert_eq!((l, r), (Some(MOZART_YELLOW), Some(MOZART_YELLOW)));
        assert_eq!((w1, w2), (Some(SLATE_DARK), Some(SLATE_DARK)));
        let (l, _, _, _) = marks(Pose::Telegraph, BUS_HAZARD_TICKS);
        assert_ne!(l, Some(MOZART_YELLOW), "blinks");
        let (l, _, w1, w2) = marks(Pose::Fotostopp { flash: true }, 0);
        assert_ne!(l, Some(MOZART_YELLOW));
        assert_eq!((w1, w2), (Some(WHITE), Some(WHITE)), "flashes");
        let (_, _, w1, _) = marks(Pose::Fotostopp { flash: false }, 0);
        assert_eq!(w1, Some(SLATE_DARK));
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
            Popup::Abtreten,
        ] {
            let key = p.text_key().unwrap();
            assert!(texts.contains(key), "{key}");
        }
    }

    /// Two grey-box officers on a 20-column strip (the lead one at column
    /// 12, a plain one at 15), stepped `frames` frames.
    fn officer_game(frames: u32) -> Game {
        let text = "[meta]\nid = \"t\"\ntime_limit = 300\n[legend]\n\"#\" = \"solid\"\n\
             \"L\" = { entity = \"offizier\", lead = true }\n\"o\" = { entity = \"offizier\" }\n\
             \"P\" = { entity = \"player_start\" }\n[map]\ngrid = '''\n\
             ....................\n....................\n....................\n\
             ....................\n....................\n....................\n\
             ....................\n....................\n..P.........L..o....\n\
             ####################\n'''\n";
        let mut g = Game::new(&Level::parse(text).unwrap(), &tuning(), 1);
        for _ in 0..frames {
            g.step(InputFrame::NONE);
        }
        g
    }

    fn officers(g: &Game) -> Vec<Entity> {
        g.entities()
            .iter()
            .filter(|e| e.kind == EntityKind::Enemy(EnemyKind::Offizier))
            .copied()
            .collect()
    }

    /// The grey-box officer: a hilt in „Habt Acht!", the sabre drawn
    /// exactly over the hurt zone in „Präsentiert!", nothing above his head
    /// in the march; the flag bearer's black-and-yellow flag up in the
    /// march and dipped in „Habt Acht!", clear of the blade's column.
    #[test]
    fn the_grey_box_officer_shows_the_drill_and_the_flag() {
        let t = tuning();
        // March (frame 10).
        let g = officer_game(10);
        let fb = rendered(&g);
        let cam = g.camera();
        let o = officers(&g);
        assert_eq!(o[0].pose(&t), Some(Pose::March));
        let (cx, top) = (
            (o[1].x + o[1].w.div_int(2)).to_px() - cam.x,
            o[1].y.to_px() - cam.y,
        );
        assert_ne!(fb.get(cx, top - 8), Some(WHITE), "no blade in the march");
        let flag_px = |fb: &Framebuffer, e: &Entity| {
            let (l, r) = (e.x.to_px() - cam.x - 8, e.right().to_px() - cam.x + 8);
            (e.y.to_px() - cam.y - 12..e.y.to_px() - cam.y + 2)
                .flat_map(|y| (l..r).map(move |x| (x, y)))
                .filter(|&(x, y)| fb.get(x, y) == Some(MOZART_YELLOW))
                .map(|(_, y)| y)
                .min()
        };
        let up = flag_px(&fb, &o[0]).expect("the lead officer's flag");
        assert_eq!(flag_px(&fb, &o[1]), None, "a plain officer has none");
        // „Habt Acht!" (frame 170): the hilt shows, the flag dips.
        let g = officer_game(170);
        let fb = rendered(&g);
        let o = officers(&g);
        assert_eq!(o[1].pose(&t), Some(Pose::Habtacht));
        let dipped = flag_px(&fb, &o[0]).unwrap();
        assert!(dipped > up, "dipped: {dipped} vs {up}");
        let (x, y, r) = (
            o[1].x.to_px() - cam.x,
            o[1].y.to_px() - cam.y,
            o[1].right().to_px() - cam.x,
        );
        let hilt = (x - 1..=r).any(|hx| fb.get(hx, y + 10) == Some(MOZART_YELLOW));
        assert!(hilt, "the hilt");
        // „Präsentiert!" (frame 200): the blade covers the zone exactly.
        let g = officer_game(200);
        let fb = rendered(&g);
        for e in officers(&g) {
            let z = jnr_core::enemies::sabre_zone(&e, &t).unwrap();
            let (zx, zy) = (z.x.to_px() - cam.x, z.y.to_px() - cam.y);
            for y in zy..zy + 16 {
                for x in zx..zx + 8 {
                    let c = fb.get(x, y);
                    assert!(c == Some(WHITE) || c == Some(SILVER_LIGHT), "({x}, {y})");
                }
            }
            for x in [zx - 2, zx - 1, zx + 8, zx + 9] {
                assert_ne!(fb.get(x, zy + 1), Some(WHITE), "flag clear of the blade");
            }
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
            EntityKind::Schafbergbahn,
            EntityKind::Traundl,
            EntityKind::Dampfer,
            EntityKind::Enemy(EnemyKind::Offizier),
        ];
        for (i, a) in kinds.iter().enumerate() {
            for b in &kinds[i + 1..] {
                assert_ne!(grey_box_colour(*a), grey_box_colour(*b), "{a:?} vs {b:?}");
            }
        }
    }

    /// A 24×10 grey-box level, ground in row 9, with a Schafbergbahn
    /// spawned at its upper end in cell (10, 6) (body x 160..224, feet
    /// (192, 112)) going [−8, +2] down to feet (64, 144) on the ground: it
    /// travels down at once (no rider needed), 192 frames. A 2-column gap
    /// in the ground at columns 12–13 (under the track's middle).
    fn train_game() -> Game {
        let mut rows: Vec<Vec<char>> = (0..10)
            .map(|r| vec![if r == 9 { '#' } else { '.' }; 24])
            .collect();
        rows[8][0] = 'P';
        rows[6][10] = 'z';
        rows[9][12] = '.';
        rows[9][13] = '.';
        let grid: String = rows
            .into_iter()
            .map(|r| r.into_iter().collect::<String>() + "\n")
            .collect();
        let text = format!(
            "[meta]\nid = \"t\"\ntime_limit = 300\n[legend]\n\"#\" = \"solid\"\n\
             \"z\" = {{ entity = \"schafbergbahn\", to = [-8, 2] }}\n\
             \"P\" = {{ entity = \"player_start\" }}\n[map]\ngrid = '''\n{grid}'''\n"
        );
        Game::new(&Level::parse(&text).unwrap(), &tuning(), 1)
    }

    fn puffs(g: &Game, e: &Entity) -> Vec<(i32, i32, u16)> {
        steam_puffs(g, e).into_iter().flatten().collect()
    }

    fn the_train(g: &Game) -> Entity {
        *g.entities()
            .iter()
            .find(|e| e.kind == EntityKind::Schafbergbahn)
            .unwrap()
    }

    /// The pixel-artist's track layout: per column, u = x − 160 (the body's
    /// left edge at the spawn), upper rail Y = 113 (one below the feet y at
    /// the spawn, where the sprite's wheels stand) − floor(u / 4); rails at
    /// Y and Y+3, rack at Y+1 (even u) and
    /// Y+2, 2 px sleepers every 8 px over Y+1..=Y+4 under the rails, a
    /// girder at Y+5 under the whole track; from a tile
    /// before the lower end to a tile past the upper end; behind the tiles.
    #[test]
    fn the_schafbergbahn_track_shows_the_whole_ride() {
        let g = train_game();
        let fb = rendered(&g);
        // 1 px up per 4 px right: u −4..−1 on row 114, −8..−5 on row 115.
        for x in 156..=159 {
            assert_eq!(fb.get(x, 114), Some(SLATE_DARK), "x {x}");
        }
        for x in 152..=155 {
            assert_eq!(fb.get(x, 115), Some(SLATE_DARK), "x {x}");
        }
        // u −3 (odd, no sleeper): no rack at Y+1, rack at Y+2, rail at Y+3.
        assert_ne!(fb.get(157, 115), Some(STONE_GREY));
        assert_eq!(fb.get(158, 115), Some(STONE_GREY), "even u: rack at Y+1");
        assert_eq!(fb.get(157, 116), Some(STONE_GREY));
        assert_eq!(fb.get(157, 117), Some(SLATE_DARK), "lower rail");
        // u −8 (a sleeper, x 152–153): over Y+1..=Y+4, the rails on top.
        assert_eq!(fb.get(152, 115), Some(SLATE_DARK));
        assert_eq!(fb.get(153, 116), Some(SLEEPER));
        assert_eq!(fb.get(152, 118), Some(SLATE_DARK));
        assert_eq!(fb.get(152, 119), Some(SLEEPER));
        assert_ne!(fb.get(154, 119), Some(SLEEPER), "2 px wide");
        // The girder under the whole track: over the gap (x 200: u 40,
        // Y 103) and over the ground (x 150: u −10, Y 116).
        // (once the train has gone down: at the spawn it covers the gap).
        let mut down = train_game();
        for _ in 0..200 {
            down.step(InputFrame::NONE);
        }
        assert_eq!(rendered(&down).get(200, 108), Some(SLATE_DARK), "girder");
        assert_eq!(fb.get(150, 121), Some(SLATE_DARK), "over the ground too");
        // Ends: a tile past the upper end (x 239: u 79, Y 94) and no further.
        assert_eq!(fb.get(239, 94), Some(SLATE_DARK), "far end");
        assert_ne!(fb.get(240, 94), Some(SLATE_DARK));
        // Behind the tiles: under the ground at the lower end it is hidden.
        assert_eq!(fb.get(20, 147), Some(STONE_GREY), "the ground tile");
        assert_ne!(fb.get(20, 149), Some(SLATE_DARK));
        // Grey-box train over it, a chimney stub at its left end.
        assert_eq!(fb.get(192, 100), Some(OUTLINE), "body");
        assert_eq!(fb.get(165, 76), Some(SLATE_DARK), "chimney");
    }

    #[test]
    fn steam_puffs_leave_every_20_frames_and_rise_for_24() {
        let mut g = train_game();
        assert!(puffs(&g, &the_train(&g)).is_empty(), "not moving yet");
        g.step(InputFrame::NONE);
        let t = the_train(&g);
        assert_eq!(t.count, 1);
        let chimney = (t.x.to_px() + 3, t.y.to_px() - 6);
        assert_eq!(puffs(&g, &t), [(chimney.0, chimney.1, 0)]);
        let fb = rendered(&g);
        assert_eq!(fb.get(chimney.0, chimney.1 - 3), Some(WHITE), "grey box");
        for _ in 0..20 {
            g.step(InputFrame::NONE);
        }
        let t = the_train(&g);
        let next = (t.x.to_px() + 3, t.y.to_px() - 6);
        assert_eq!(
            puffs(&g, &t),
            [(chimney.0, chimney.1 - 20, 20), (next.0, next.1, 0)],
            "the first stays behind and rises"
        );
        for _ in 0..4 {
            g.step(InputFrame::NONE);
        }
        assert_eq!(
            puffs(&g, &the_train(&g)),
            [(next.0, next.1 - 4, 4)],
            "gone after 24 frames"
        );
        // Arrived at its lower end and waiting: no steam.
        for _ in 0..200 {
            g.step(InputFrame::NONE);
        }
        let t = the_train(&g);
        assert_eq!(t.state, jnr_core::platform::car::WAIT_LOWER);
        assert!(puffs(&g, &t).is_empty());
    }

    /// A 30×10 grey-box level, ground in row 9, with `boat` (a legend
    /// entry) in cell (2, 7): hull bottom (the water line) y 128, deck
    /// y 112. The player starts at (4, 4) and falls onto a Dampfer's deck.
    fn boat_game(boat: &str) -> Game {
        let mut rows: Vec<Vec<char>> = (0..10)
            .map(|r| vec![if r == 9 { '#' } else { '.' }; 30])
            .collect();
        rows[4][4] = 'P';
        rows[7][2] = 'b';
        let grid: String = rows
            .into_iter()
            .map(|r| r.into_iter().collect::<String>() + "\n")
            .collect();
        let text = format!(
            "[meta]\nid = \"t\"\ntime_limit = 300\n[legend]\n\"#\" = \"solid\"\n\
             \"b\" = {boat}\n\
             \"P\" = {{ entity = \"player_start\" }}\n[map]\ngrid = '''\n{grid}'''\n"
        );
        Game::new(&Level::parse(&text).unwrap(), &tuning(), 1)
    }

    fn the_boat(g: &Game, kind: EntityKind) -> Entity {
        *g.entities().iter().find(|e| e.kind == kind).unwrap()
    }

    /// Grey box: a white hull with a gold funnel stub; while it travels the
    /// paddle wheel's zone is outlined and white spray shows under the water
    /// line on both sides of the wheel; waiting and moored: neither.
    #[test]
    fn the_grey_box_dampfer_shows_its_wheel_and_spray_only_while_travelling() {
        use jnr_core::platform::{car, paddle_wheel};
        let mut g = boat_game("{ entity = \"dampfer\", to = [8, 0] }");
        let spray = |g: &Game| {
            let fb = rendered(g);
            let e = the_boat(g, EntityKind::Dampfer);
            let (cx, water) = feet_on_screen(&e, g.camera());
            let (left, top) = (e.x.to_px() - g.camera().x, e.y.to_px() - g.camera().y);
            assert_eq!(fb.get(left + 74, top - 10), Some(OBLATE_GOLD), "funnel");
            assert_eq!(fb.get(cx, top + 8), Some(WHITE), "hull");
            let zone = paddle_wheel(&e, g.tuning()).map(|z| {
                let (zx, zy) = (z.x.to_px() - g.camera().x, z.y.to_px() - g.camera().y);
                fb.get(zx, zy + 10) == Some(SLATE_DARK)
            });
            let both = [cx - GISCHT_SPREAD_PX, cx + GISCHT_SPREAD_PX]
                .map(|x| fb.get(x, water + 3) == Some(WHITE));
            (zone, both)
        };
        assert_eq!(spray(&g), (None, [false, false]), "waiting");
        until_state(&mut g, car::UP);
        for _ in 0..12 {
            assert_eq!(spray(&g), (Some(true), [true, true]), "travelling");
            g.step(InputFrame::NONE);
        }
        until_state(&mut g, car::MOORED);
        assert_eq!(spray(&g), (None, [false, false]), "moored");
    }

    fn until_state(g: &mut Game, state: u8) {
        for _ in 0..400 {
            if the_boat(g, EntityKind::Dampfer).state == state {
                return;
            }
            g.step(InputFrame::NONE);
        }
        panic!("the Dampfer never reached state {state}");
    }

    /// With its sprites: `dampfer` `ride` while it travels, `idle` while it
    /// waits and once moored, never mirrored; `gischt` `spray` on the water
    /// line beside the wheel's centre only while it travels.
    #[test]
    fn the_dampfer_and_its_spray_use_their_sprites() {
        use crate::testutil::{A, B, C, atlas, image, sprite};
        use jnr_core::platform::car;
        let frame = |c: &str, w: usize, h: usize| image(&vec![c.repeat(w).as_str(); h]);
        let ship = sprite(
            "dampfer",
            (80, 48),
            false,
            vec![
                ("idle", 0, vec![frame("a", 160, 48)]),
                ("ride", 0, vec![frame("b", 160, 48)]),
            ],
        );
        let gischt = sprite(
            "gischt",
            (8, 16),
            false,
            vec![("spray", 4, vec![frame("c", 16, 16)])],
        );
        let atlas = atlas(&[ship, gischt]);
        let texts = Texts::default();
        let mut g = boat_game("{ entity = \"dampfer\", to = [8, 0] }");
        let shown = |g: &Game| {
            let ctx = RenderContext::new(&atlas, &texts, g.level());
            let mut fb = Framebuffer::new();
            render(g, &ctx, &mut fb);
            let (x, y) = feet_on_screen(&the_boat(g, EntityKind::Dampfer), g.camera());
            (fb.get(x + 40, y - 30), fb.get(x - GISCHT_SPREAD_PX, y - 8))
        };
        assert_eq!(shown(&g), (Some(A), Some(A)), "waiting");
        until_state(&mut g, car::UP);
        assert_eq!(shown(&g), (Some(B), Some(C)), "travelling");
        until_state(&mut g, car::MOORED);
        assert_eq!(shown(&g), (Some(A), Some(A)), "moored");
    }

    /// The Traundl: grey box with the rower stub at the stern; with its
    /// sprite `idle` while it waits at an end and `ride` while it moves.
    #[test]
    fn the_traundl_waits_idle_and_rows_with_ride() {
        use crate::testutil::{A, B, atlas, image, sprite};
        let mut g = boat_game("{ entity = \"traundl\", to = [4, 0] }");
        let fb = rendered(&g);
        let e = the_boat(&g, EntityKind::Traundl);
        let (left, top) = (e.x.to_px() - g.camera().x, e.y.to_px() - g.camera().y);
        assert_eq!(fb.get(left + 5, top + 8), Some(LEATHER_DARK), "hull");
        let (rx, [_, rh]) = TRAUNDL_ROWER;
        assert_eq!(fb.get(left + rx + 1, top - rh), Some(PINE_DARK), "rower");
        let frame = |c: &str| image(&vec![c.repeat(48).as_str(); 32]);
        let boat = sprite(
            "traundl",
            (24, 32),
            false,
            vec![("idle", 0, vec![frame("a")]), ("ride", 0, vec![frame("b")])],
        );
        let atlas = atlas(&[boat]);
        let texts = Texts::default();
        let shown = |g: &Game| {
            let ctx = RenderContext::new(&atlas, &texts, g.level());
            let mut fb = Framebuffer::new();
            render(g, &ctx, &mut fb);
            let (x, y) = feet_on_screen(&the_boat(g, EntityKind::Traundl), g.camera());
            fb.get(x, y - 8)
        };
        // Wait 30 frames at the spawn end, then 64 frames of travel.
        let mut seen = Vec::new();
        for _ in 0..40 {
            g.step(InputFrame::NONE);
            seen.push(shown(&g));
        }
        assert!(seen[..29].iter().all(|&c| c == Some(A)), "{seen:?}");
        assert!(seen[31..].iter().all(|&c| c == Some(B)), "{seen:?}");
    }

    /// With its sprite: `ride` while it travels, `idle` while it stands,
    /// never mirrored (it travels left here).
    #[test]
    fn the_schafbergbahn_rides_and_idles_with_its_sprite() {
        use crate::testutil::{A, B, atlas, image, sprite};
        let frame = |c: &str| image(&vec![c.repeat(64).as_str(); 40]);
        let train = sprite(
            "schafbergbahn",
            (32, 40),
            false,
            vec![("idle", 0, vec![frame("a")]), ("ride", 0, vec![frame("b")])],
        );
        let atlas = atlas(&[train]);
        let texts = Texts::default();
        let mut g = train_game();
        let shown = |g: &Game| {
            let ctx = RenderContext::new(&atlas, &texts, g.level());
            let mut fb = Framebuffer::new();
            render(g, &ctx, &mut fb);
            let (x, y) = feet_on_screen(&the_train(g), g.camera());
            fb.get(x, y - 20)
        };
        assert_eq!(shown(&g), Some(B), "spawned at the upper end: riding down");
        for _ in 0..200 {
            g.step(InputFrame::NONE);
        }
        assert_eq!(shown(&g), Some(A), "waiting at the lower end");
    }

    #[test]
    fn the_grey_box_boat_shows_its_start_flag_and_bow_wave() {
        use jnr_core::Pose;
        // A 24×56 box at (100, 100): the mast up the middle, the flag at
        // the stern while it telegraphs (blinking), the bow wave racing.
        let marks = |pose: Pose, tick: u32| {
            let mut fb = Framebuffer::new();
            draw_boat_marks(&mut fb, 100, 100, 24, 56, pose, tick);
            (fb.get(112, 120), fb.get(120, 137), fb.get(98, 143))
        };
        let (mast, flag, wave) = marks(Pose::Idle, 0);
        assert_eq!(mast, Some(SLATE_DARK));
        assert_ne!(flag, Some(DIRNDL_RED));
        assert_ne!(wave, Some(MOZART_YELLOW));
        let (_, flag, _) = marks(Pose::Telegraph, 0);
        assert_eq!(flag, Some(DIRNDL_RED));
        let (_, flag, _) = marks(Pose::Telegraph, BUS_HAZARD_TICKS);
        assert_ne!(flag, Some(DIRNDL_RED), "blinks");
        let (_, _, wave) = marks(Pose::Drive, 0);
        assert_eq!(wave, Some(MOZART_YELLOW));
        assert_eq!(
            grey_box_colour(EntityKind::Enemy(EnemyKind::Regattaboot)),
            CREAM
        );
    }

    #[test]
    fn a_hovering_gelse_bobs_2_px() {
        let bob: Vec<i32> = (0..32).step_by(4).map(gelse_bob_px).collect();
        assert_eq!(bob, vec![0, 1, 2, 1, 0, -1, -2, -1]);
        assert_eq!(gelse_bob_px(32), 0, "periodic");
    }

    #[test]
    fn a_regattaboot_sprite_stands_on_its_cell_bottom_below_the_keel() {
        use crate::testutil::{A, atlas, image, sprite};
        // The catalog's 32×64 sprite with its origin at the bottom centre:
        // its bottom row is the cell's bottom, 4 px below the hitbox.
        let rows = vec!["a".repeat(32); 64];
        let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
        let boat = sprite(
            "regattaboot",
            (16, 64),
            false,
            vec![("idle", 30, vec![image(&rows)])],
        );
        let atlas = atlas(&[boat]);
        let texts = Texts::default();
        let text = "[meta]\nid = \"t\"\ntime_limit = 300\n[legend]\n\"#\" = \"solid\"\n\
             \"b\" = { entity = \"regattaboot\" }\n\
             \"P\" = { entity = \"player_start\" }\n[map]\ngrid = '''\n\
             ....................\n....................\n....................\n\
             ....................\n....................\n........b...........\n\
             ....................\n....................\n..P.................\n\
             ####################\n'''\n";
        let mut g = Game::new(&Level::parse(text).unwrap(), &tuning(), 1);
        g.step(InputFrame::NONE);
        let b = *g
            .entities()
            .iter()
            .find(|e| e.kind == EntityKind::Enemy(EnemyKind::Regattaboot))
            .unwrap();
        assert_eq!((b.y + b.h).to_px(), 92, "keel 12 px under the cell top");
        let ctx = RenderContext::new(&atlas, &texts, g.level());
        let mut fb = crate::testutil::fb();
        render(&g, &ctx, &mut fb);
        let cam = g.camera();
        let (x, bottom) = (144 - cam.x, 96 - cam.y);
        assert_eq!(fb.get(x, bottom - 1), Some(A), "down to the cell bottom");
        assert_ne!(fb.get(x, bottom), Some(A));
        assert_eq!(fb.get(x, bottom - 64), Some(A), "64 px tall");
        assert_ne!(fb.get(x, bottom - 65), Some(A));
    }

    // ------------------------------------------------ draw layers (D-086)

    /// A 20×10 grey-box level from `grid` (10 rows) with `legend` entries
    /// added to solid ground and the player start; stepped once so every
    /// entity is active.
    fn layered_game(grid: &str, legend: &str) -> Game {
        let text = format!(
            "[meta]\nid = \"t\"\ntime_limit = 300\n[legend]\n\"#\" = \"solid\"\n\
             \"P\" = {{ entity = \"player_start\" }}\n{legend}\n[map]\ngrid = '''\n{grid}'''\n"
        );
        let mut g = Game::new(&Level::parse(&text).unwrap(), &tuning(), 1);
        g.step(InputFrame::NONE);
        g
    }

    #[test]
    fn layers_put_signs_first_and_enemies_last() {
        assert_eq!(Layer::of(EntityKind::Sign), Layer::Signs);
        for kind in [
            EntityKind::Oblate,
            EntityKind::Checkpoint,
            EntityKind::Traundl,
            EntityKind::Dampfer,
            EntityKind::Gondel,
            EntityKind::Festungsbahn,
            EntityKind::Schafbergbahn,
        ] {
            assert_eq!(Layer::of(kind), Layer::Middle, "{kind:?}");
        }
        for kind in [
            EntityKind::Enemy(EnemyKind::Gelse),
            EntityKind::Enemy(EnemyKind::Schwan),
            EntityKind::Enemy(EnemyKind::Operettensaenger),
            EntityKind::Defeated(EnemyKind::Gelse),
            EntityKind::Thrown,
            EntityKind::Stein,
            EntityKind::Note,
            EntityKind::ThrownHackl,
        ] {
            assert_eq!(Layer::of(kind), Layer::Enemies, "{kind:?}");
        }
    }

    #[test]
    fn an_oblate_is_drawn_in_front_of_a_sign_board_listed_after_it() {
        // The Oblate (row 7) comes before the sign (row 8) in the entity
        // list, and its box lies inside the grey-box board (x 104..184,
        // y 116..140): drawn in list order, the board would cover it.
        let grid = "\
....................
....................
....................
....................
....................
....................
....................
.........o..........
.P.......s..........
####################
";
        let g = layered_game(
            grid,
            "\"o\" = { entity = \"oblate\" }\n\"s\" = { entity = \"sign\", text = \"t.s\", style = \"street\" }",
        );
        let kinds: Vec<_> = g.entities().iter().map(|e| e.kind).collect();
        let (o, s) = (
            kinds.iter().position(|&k| k == EntityKind::Oblate).unwrap(),
            kinds.iter().position(|&k| k == EntityKind::Sign).unwrap(),
        );
        assert!(o < s, "the Oblate is listed first: {kinds:?}");
        let fb = rendered(&g);
        let oblate = g.entities()[o];
        let (cx, cy) = (
            (oblate.x + oblate.w.div_int(2)).to_px(),
            (oblate.y + oblate.h.div_int(2)).to_px(),
        );
        assert!((116..140).contains(&cy), "over the board: y {cy}");
        assert_eq!(fb.get(cx, cy), Some(OBLATE_GOLD), "the Oblate in front");
        assert_eq!(fb.get(170, 130), Some(CREAM), "the board is drawn");
    }

    #[test]
    fn an_enemy_is_drawn_in_front_of_a_boat_listed_after_it() {
        // The Gelse (row 6, hitbox x 68..76, y 100..108) comes before the
        // Traundl (row 7, hull x 32..80) in the entity list and overlaps the
        // grey-box rower (x 70..76, y 98..112): drawn in list order, the
        // rower would hide it (the Gelse under Traundl B, E32).
        let grid = "\
....................
....................
....................
....................
....................
....................
....g...............
..b.................
.P..................
####################
";
        let g = layered_game(
            grid,
            "\"g\" = { entity = \"gelse\" }\n\"b\" = { entity = \"traundl\", to = [4, 0] }",
        );
        let kinds: Vec<_> = g.entities().iter().map(|e| e.kind).collect();
        let gelse = EntityKind::Enemy(EnemyKind::Gelse);
        let (i, j) = (
            kinds.iter().position(|&k| k == gelse).unwrap(),
            kinds
                .iter()
                .position(|&k| k == EntityKind::Traundl)
                .unwrap(),
        );
        assert!(i < j, "the Gelse is listed first: {kinds:?}");
        let boat = g.entities()[j];
        let rower_x = boat.x.to_px() + TRAUNDL_ROWER.0;
        let fb = rendered(&g);
        let e = g.entities()[i];
        let (cx, cy) = (
            (e.x + e.w.div_int(2)).to_px(),
            (e.y + e.h.div_int(2)).to_px(),
        );
        assert!(
            (rower_x..rower_x + TRAUNDL_ROWER.1[0]).contains(&cx),
            "over the rower: x {cx}, rower {rower_x}"
        );
        assert_eq!(
            fb.get(cx, cy),
            Some(grey_box_colour(gelse)),
            "the Gelse in front"
        );
        assert_eq!(
            fb.get(rower_x, boat.y.to_px() - 1),
            Some(PINE_DARK),
            "the rower is drawn"
        );
    }
}
