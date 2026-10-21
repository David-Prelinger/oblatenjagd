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
//!    Offizier as `offizier_lead`, a car's look by its spawn index, a
//!    `look = "postbus"` Reisebus as `postbus`), falling rocks, the
//!    Operettensänger's high-C column (`hohes_c` rings), the Offizier's
//!    grey-box sabre (drawn from `enemies::sabre_zone`) and snow globes,
//!    so a hazard never hides behind a hull or car; the Ampel's light
//!    (from its counter) is a middle-layer prop, so its cars drive in
//!    front of it
//! 4. player (scene)
//! 5. [`draw_front`]: debris (bricks, rocks, snow-globe shards), then all
//!    popups on top (the last world pass, under the HUD)

use jnr_artkit::atlas::{Atlas, AtlasAnim, AtlasSprite};
use jnr_core::entity::{EnemyKind, Entity, EntityKind, Popup};
use jnr_core::{AmpelLight, Block, BusLook, Camera, Game, Pose, SpawnKind, Tile};

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

/// A car's looks (D-093): anims `drive_0` … `drive_2` (a red hatchback, a
/// white estate, a camper van), picked by its spawn index in its green
/// (`Entity::count`) modulo this.
pub const AUTO_LOOKS: u16 = 3;
/// The cars' grey-box colours by look (fallback without the `auto` sprite): `r`
/// red, `w` white, `h` pale.
pub const AUTO_GREY_BOX: [u32; AUTO_LOOKS as usize] = [DIRNDL_RED, WHITE, SKY_HAZE];
/// The `ampel` sprite is this tall (origin on its cell's bottom centre).
pub const AMPEL_H: i32 = 48;

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

/// The Kurgast in his deck chair (D-095): 24×16, origin at the bottom
/// centre of the 16×16 chair; anims `idle` (lying) and `wake`. Without it
/// the `kurgast` sprite's `liegen`/`wake` anims are used.
pub const LIEGESTUHL_SPRITE: &str = "liegestuhl";

/// A Wartender (D-105, a still Selfie-Tourist placed as `wartender`):
/// 16×24, origin at the feet, always facing right; three looks picked by
/// the spawn column mod [`WARTENDER_LOOKS`], each with its own anims
/// ([`WARTENDER_IDLE`], [`WARTENDER_SELFIE`], [`WARTENDER_SQUISHED`]).
/// Without it: the Selfie-Tourist's grey box.
pub const WARTENDER_SPRITE: &str = "wartender";
/// The number of Wartender looks.
pub const WARTENDER_LOOKS: i32 = 3;
/// A Wartender's standing anim by look (a Selfie-Tourist's `walk`).
pub const WARTENDER_IDLE: [&str; WARTENDER_LOOKS as usize] = ["idle_0", "idle_1", "idle_2"];
/// A Wartender's selfie anim by look (frame 1 = the flash, like `selfie`).
pub const WARTENDER_SELFIE: [&str; WARTENDER_LOOKS as usize] = ["selfie_0", "selfie_1", "selfie_2"];
/// A defeated Wartender's anim by look.
pub const WARTENDER_SQUISHED: [&str; WARTENDER_LOOKS as usize] =
    ["squished_0", "squished_1", "squished_2"];

/// The Kellner's grey box (until the `kellner` art lands, D-105): the tray
/// line at the box top is this many px high ...
pub const KELLNER_TRAY_PX: i32 = 2;
/// ... and drawn this many px lower in the `bounce` pose (the tray dips).
pub const KELLNER_TRAY_DIP_PX: i32 = 2;

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
    postbus: Option<&'a AtlasSprite>,
    auto: Option<&'a AtlasSprite>,
    ampel: Option<&'a AtlasSprite>,
    schwan: Option<&'a AtlasSprite>,
    gelse: Option<&'a AtlasSprite>,
    regattaboot: Option<&'a AtlasSprite>,
    kurgast: Option<&'a AtlasSprite>,
    liegestuhl: Option<&'a AtlasSprite>,
    offizier: Option<&'a AtlasSprite>,
    offizier_lead: Option<&'a AtlasSprite>,
    kellner: Option<&'a AtlasSprite>,
    wartender: Option<&'a AtlasSprite>,
    oberkellner: Option<&'a AtlasSprite>,
    rechnung: Option<&'a AtlasSprite>,
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
    fensterladen: Option<&'a AtlasSprite>,
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
            postbus: s("postbus"),
            auto: s(EnemyKind::Auto.sprite()),
            ampel: s("ampel"),
            schwan: s(EnemyKind::Schwan.sprite()),
            gelse: s(EnemyKind::Gelse.sprite()),
            regattaboot: s(EnemyKind::Regattaboot.sprite()),
            kurgast: s(EnemyKind::Kurgast.sprite()),
            liegestuhl: s(LIEGESTUHL_SPRITE),
            offizier: s(EnemyKind::Offizier.sprite()),
            offizier_lead: s("offizier_lead"),
            kellner: s(EnemyKind::Kellner.sprite()),
            wartender: s(WARTENDER_SPRITE),
            oberkellner: s(EnemyKind::Oberkellner.sprite()),
            rechnung: s("rechnung"),
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
            fensterladen: s("fensterladen"),
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
                EnemyKind::Auto => self.auto,
                EnemyKind::Kurgast => self.kurgast,
                EnemyKind::Offizier => self.offizier,
                EnemyKind::Kellner => self.kellner,
                EnemyKind::Oberkellner => self.oberkellner,
            },
            EntityKind::Rechnung => self.rechnung,
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
            EntityKind::Ampel => self.ampel,
            EntityKind::Fensterladen => self.fensterladen,
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
            EnemyKind::Auto => DIRNDL_RED,
            EnemyKind::Kurgast => STONE_GREY,
            EnemyKind::Offizier => STONE_GREY,
            // `d` like the Operettensänger (the palette has no colour
            // left); his box differs by its white apron and tray line.
            EnemyKind::Kellner => SLATE_DARK,
            // `d` tails (D-106 grey box: a `d` box with an `i` tray line).
            EnemyKind::Oberkellner => SLATE_DARK,
        },
        // `w` paper.
        EntityKind::Rechnung => WHITE,
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
        EntityKind::Ampel => STONE_GREY,
        EntityKind::Fensterladen => DIRNDL_RED,
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
            | EntityKind::ThrownHackl
            | EntityKind::Rechnung => Layer::Enemies,
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
        EntityKind::Ampel => draw_ampel(game, looks, atlas, fb, cam, e),
        EntityKind::Sign => signs::draw_sign(game, looks, atlas, font, texts, fb, cam, e),
        EntityKind::Checkpoint => draw_checkpoint(game, looks, atlas, fb, cam, e),
        EntityKind::Enemy(_)
        | EntityKind::Defeated(_)
        | EntityKind::Thrown
        | EntityKind::Stein
        | EntityKind::Note
        | EntityKind::ThrownHackl
        | EntityKind::Rechnung => draw_enemy(game, looks, atlas, fb, cam, e),
        EntityKind::Steinschlag => draw_steinschlag(game, looks, atlas, fb, cam, e),
        EntityKind::Festungsbahn => draw_festungsbahn(game, looks, atlas, fb, cam, e),
        EntityKind::Gondel => draw_gondel(game, looks, atlas, fb, cam, e),
        EntityKind::Schafbergbahn => draw_schafbergbahn(game, looks, atlas, fb, cam, e),
        EntityKind::Traundl => draw_traundl(game, looks, atlas, fb, cam, e),
        EntityKind::Dampfer => draw_dampfer(game, looks, atlas, fb, cam, e),
        EntityKind::Fensterladen => draw_fensterladen(game, looks, atlas, fb, cam, e),
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

/// A Fensterladen's sprite origin lies this many px below its body's top
/// (sprite-catalog: 32×24, origin 16,24, the body's top on sprite row 0).
pub const FENSTERLADEN_SPRITE_H: i32 = 24;
/// Ticks per step of the grey box's rattle (1 px down every other step).
pub const FENSTERLADEN_RATTLE_TICKS: u32 = 4;
/// The grey box's folded-up leaves: this wide, this tall, from the body's
/// top down at both of its ends.
pub const FENSTERLADEN_LEAF: [i32; 2] = [4, 20];

/// A Fensterladen (D-094) in its look from the stage clock
/// (`platform::shutter_look`): sprite `fensterladen` with the anim of that
/// name (`open`, `rattle`, `closed`, `warn`; the rattle's shake is in its
/// frames) at the body's top + [`FENSTERLADEN_SPRITE_H`], never mirrored.
/// Else the grey box: open, the body; rattling, the body shaken 1 px down
/// every other [`FENSTERLADEN_RATTLE_TICKS`] ticks (drawn only, the body
/// never moves); closed, two leaves folded up at its ends and no ledge;
/// warning, the leaves and the landlady's face (a `SKIN_LIGHT` square)
/// between them.
fn draw_fensterladen(
    game: &Game,
    looks: &EntityLooks,
    atlas: &Atlas,
    fb: &mut Framebuffer,
    cam: Camera,
    e: &Entity,
) {
    use jnr_core::platform::{ShutterLook, shutter_look};
    let Some(look) = shutter_look(e, game.level(), game.tuning()) else {
        return;
    };
    let (left, top) = (e.x.to_px() - cam.x, e.y.to_px() - cam.y);
    let (w, h) = (e.w.to_px(), e.h.to_px());
    if let Some(s) = looks.fensterladen {
        let name = match look {
            ShutterLook::Open => "open",
            ShutterLook::Rattle => "rattle",
            ShutterLook::Closed => "closed",
            ShutterLook::Warn => "warn",
        };
        let anim = s.anim(name).unwrap_or(&s.anims[0]);
        let x = left + w / 2;
        draw_sprite(
            fb,
            atlas,
            s,
            anim,
            game.frame(),
            x,
            top + FENSTERLADEN_SPRITE_H,
            false,
        );
        return;
    }
    match look {
        ShutterLook::Open | ShutterLook::Rattle => {
            let shake =
                look == ShutterLook::Rattle && (game.frame() / FENSTERLADEN_RATTLE_TICKS) % 2 == 1;
            let y = top + i32::from(shake);
            fb.fill_rect(left, y, w, h, DIRNDL_RED);
            fb.outline_rect(left, y, w, h, OUTLINE);
        }
        ShutterLook::Closed | ShutterLook::Warn => {
            let [lw, lh] = FENSTERLADEN_LEAF;
            for x in [left, left + w - lw] {
                fb.fill_rect(x, top, lw, lh, DIRNDL_RED);
                fb.outline_rect(x, top, lw, lh, OUTLINE);
            }
            if look == ShutterLook::Warn {
                fb.fill_rect(left + w / 2 - 3, top + 4, 6, 6, SKIN_LIGHT);
            }
        }
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
        Pose::Liegen => ("liegen", None),
        Pose::Wake => ("wake", None),
        Pose::March => ("march", None),
        Pose::Habtacht => ("habtacht", None),
        Pose::Praesentiert => ("praesentiert", None),
        Pose::Anstellen => ("anstellen", None),
        Pose::Dash => ("dash", None),
        Pose::Verschnaufen => ("verschnaufen", None),
        Pose::Zahlen => ("zahlen", None),
        Pose::Stun => ("stun", None),
        Pose::Verbeugen => ("verbeugen", None),
        Pose::Hang => ("hang", None),
        Pose::Flutter => ("flutter", None),
        Pose::Bounce => ("bounce", None),
    }
}

/// A Wartender's look (0..[`WARTENDER_LOOKS`]: its spawn column mod 3,
/// D-105) if `e` (an enemy or a defeated one) was placed as `wartender`;
/// `None` for everything else, a plain Selfie-Tourist included.
pub fn wartender_look(game: &Game, e: &Entity) -> Option<usize> {
    if !matches!(
        e.kind,
        EntityKind::Enemy(EnemyKind::SelfieTourist)
            | EntityKind::Defeated(EnemyKind::SelfieTourist)
    ) {
        return None;
    }
    let s = e
        .spawn
        .and_then(|i| game.level().entities.get(usize::from(i)))?;
    (s.kind == SpawnKind::Wartender).then(|| s.col.rem_euclid(WARTENDER_LOOKS) as usize)
}

/// A Wartender's anim for `pose` in `look` (`idle_N`, `selfie_N` with its
/// fixed frame, `squished_N`).
pub fn wartender_anim(pose: Pose, look: usize) -> (&'static str, Option<usize>) {
    let look = look.min(WARTENDER_LOOKS as usize - 1);
    match pose {
        Pose::Selfie { flash } => (WARTENDER_SELFIE[look], Some(usize::from(flash))),
        Pose::Squished | Pose::Tumble => (WARTENDER_SQUISHED[look], None),
        _ => (WARTENDER_IDLE[look], None),
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
    // A `lead` Pilger or Offizier: `pilger_lead` / `offizier_lead`.
    let lead = is_lead(game, e);
    let lead_sprite = match e.kind {
        EntityKind::Enemy(EnemyKind::Offizier) | EntityKind::Defeated(EnemyKind::Offizier) => {
            looks.offizier_lead
        }
        _ => looks.pilger_lead,
    };
    let postbus = is_postbus(game, e);
    // A Kurgast in his deck chair is the `liegestuhl` sprite (its lying
    // anim is `idle`), else his own sprite's `liegen`/`wake`.
    let chair = matches!(pose, Pose::Liegen | Pose::Wake);
    // A Wartender (D-105): the `wartender` sprite in its look (without it
    // the Selfie-Tourist's grey box, not its sprite), always facing right.
    let look = wartender_look(game, e);
    let (sprite, (name, frame)) = match (look, lead_sprite, looks.liegestuhl) {
        (Some(look), _, _) => (looks.wartender, wartender_anim(pose, look)),
        (_, Some(s), _) if lead => (Some(s), pose_anim(pose)),
        // Postbus 150 (D-097): its own sprite (without it the yellow grey
        // box); at its stop `halt`, doors shut, faces at the windows.
        _ if postbus => match pose {
            Pose::Fotostopp { .. } => (looks.postbus, ("halt", None)),
            _ => (looks.postbus, pose_anim(pose)),
        },
        (_, _, Some(s)) if chair => {
            let anim = if pose == Pose::Liegen {
                ("idle", None)
            } else {
                pose_anim(pose)
            };
            (Some(s), anim)
        }
        _ => (looks.sprite(e.kind), pose_anim(pose)),
    };
    let car = e.kind == EntityKind::Enemy(EnemyKind::Auto);
    match sprite {
        Some(s) => {
            // A car's look by its index in its green (`drive_0` …), else
            // `drive`.
            let anim = match AUTO_DRIVE.get(usize::from(e.count % AUTO_LOOKS)) {
                Some(look) if car => s.anim(look).or_else(|| s.anim(name)),
                _ => s.anim(name),
            }
            .unwrap_or(&s.anims[0]);
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
                        | EnemyKind::Auto
                ) | EntityKind::Defeated(
                    EnemyKind::Fiaker | EnemyKind::EBiker | EnemyKind::Schneekugel
                )
            );
            // A Wartender always faces right, towards the counter (D-105).
            let left = e.facing_left() && !fixed && look.is_none();
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
            let colour = if postbus {
                MOZART_YELLOW
            } else if car {
                AUTO_GREY_BOX[usize::from(e.count % AUTO_LOOKS)]
            } else {
                grey_box_colour(e.kind)
            };
            fb.fill_rect(bx, by, w, h, colour);
            fb.outline_rect(bx, by, w, h, OUTLINE);
            if postbus {
                draw_postbus_marks(fb, bx, by, w, h, pose, game.frame());
            } else if e.kind == EntityKind::Enemy(EnemyKind::Reisebus) {
                draw_bus_marks(fb, bx, by, w, h, pose, game.frame());
            }
            if car {
                draw_car_marks(fb, bx, by, w, h);
            }
            if e.kind == EntityKind::Enemy(EnemyKind::Regattaboot) {
                draw_boat_marks(fb, bx, by, w, h, pose, game.frame());
            }
            if chair {
                // The deck chair: a wine canvas stripe across the box; in
                // the wake telegraph two white arms stretch above it.
                fb.fill_rect(bx + 1, by + h / 2 - 1, w - 2, 2, WINE_DARK);
                if pose == Pose::Wake {
                    fb.fill_rect(bx + 3, by - 5, 2, 5, WHITE);
                    fb.fill_rect(bx + w - 5, by - 5, 2, 5, WHITE);
                }
            }
            if e.kind == EntityKind::Enemy(EnemyKind::Kellner) {
                draw_kellner_marks(fb, bx, by, w, h, pose);
            }
            if e.kind == EntityKind::Enemy(EnemyKind::Schwan) && pose == Pose::Telegraph {
                // The hiss: raised wings as two white posts over the box.
                fb.fill_rect(bx + 1, by - 4, 3, 4, WHITE);
                fb.fill_rect(bx + w - 4, by - 4, 3, 4, WHITE);
            }
            if e.kind == EntityKind::Enemy(EnemyKind::Oberkellner) {
                draw_tray_marks(fb, bx, by, w, pose, e.facing_left());
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

/// The Oberkellner grey box's tray (no `oberkellner` sprite yet, D-106):
/// an `i` line 2 px high and 4 px wider than the box on its top while the
/// tray is up (intro, telegraph, dash, volley, stun: the top of his
/// hitbox is the tray); in his breather the tray hangs lowered at his
/// front side, a 2×10 `i` bar beside the box from its middle down. No
/// tray while he waits (it stands on the counter) or bows.
fn draw_tray_marks(fb: &mut Framebuffer, bx: i32, by: i32, w: i32, pose: Pose, left: bool) {
    match pose {
        Pose::Anstellen | Pose::Telegraph | Pose::Dash | Pose::Zahlen | Pose::Stun => {
            fb.fill_rect(bx - 2, by, w + 4, 2, SILVER_LIGHT);
        }
        Pose::Verschnaufen => {
            let x = if left { bx - 2 } else { bx + w };
            fb.fill_rect(x, by + 15, 2, 10, SILVER_LIGHT);
        }
        _ => {}
    }
}

/// Grey-box marks of a walking Kellner (no `kellner` sprite yet, D-105) on
/// his `d` box at (`x`, `y`), `w`×`h`: a white `w` apron over the lower
/// half (1 px in from the sides) and the `i` tray line,
/// [`KELLNER_TRAY_PX`] high across the whole top (the bounce surface), or
/// [`KELLNER_TRAY_DIP_PX`] lower in the `bounce` pose.
fn draw_kellner_marks(fb: &mut Framebuffer, x: i32, y: i32, w: i32, h: i32, pose: Pose) {
    fb.fill_rect(x + 1, y + h / 2, w - 2, h - h / 2 - 1, WHITE);
    let dip = if pose == Pose::Bounce {
        KELLNER_TRAY_DIP_PX
    } else {
        0
    };
    fb.fill_rect(x, y + dip, w, KELLNER_TRAY_PX, SILVER_LIGHT);
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

/// The anims of a car's looks (D-093), by its spawn index modulo
/// [`AUTO_LOOKS`]; a missing one falls back to `drive`.
pub const AUTO_DRIVE: [&str; AUTO_LOOKS as usize] = ["drive_0", "drive_1", "drive_2"];

/// True for a Reisebus placed with `look = "postbus"` (D-097).
fn is_postbus(game: &Game, e: &Entity) -> bool {
    e.kind == EntityKind::Enemy(EnemyKind::Reisebus)
        && e.spawn
            .and_then(|i| game.level().entities.get(usize::from(i)))
            .is_some_and(|s| s.look == BusLook::Postbus)
}

/// The `ampel` anim (and fixed frame) for a light: `rot`, `rotgelb`,
/// `gruen`, `gruen_blink` (frame 0 lit, 1 dark, from the counter so the
/// four blinks stay in step with the cars) and `gelb`.
pub fn ampel_anim(light: AmpelLight) -> (&'static str, Option<usize>) {
    match light {
        AmpelLight::Red => ("rot", None),
        AmpelLight::RedAmber => ("rotgelb", None),
        AmpelLight::Green => ("gruen", None),
        AmpelLight::GreenBlink { on } => ("gruen_blink", Some(usize::from(!on))),
        AmpelLight::Amber => ("gelb", None),
    }
}

/// An Ampel (D-093): the `ampel` light (16×48, its origin on the spawner
/// cell's bottom centre, the kerb) in the anim of its light
/// ([`jnr_core::enemies::ampel_light`]), never mirrored. Without the
/// sprite a grey box: a 2 px `m` pole and a `k` housing with three lamps
/// (red, amber, green; unlit `d`). In the red-and-amber telegraph two
/// headlights (`Y`) flash in the mouth (every [`BUS_HAZARD_TICKS`]).
fn draw_ampel(
    game: &Game,
    looks: &EntityLooks,
    atlas: &Atlas,
    fb: &mut Framebuffer,
    cam: Camera,
    e: &Entity,
) {
    let Some(light) = jnr_core::enemies::ampel_light(e, game.level(), game.tuning()) else {
        return;
    };
    let (x, y) = feet_on_screen(e, cam);
    match looks.ampel {
        Some(s) => {
            let (name, frame) = ampel_anim(light);
            let anim = s.anim(name).unwrap_or(&s.anims[0]);
            let tick = frame.map_or(game.frame(), |i| i as u32 * anim.ticks);
            draw_sprite(fb, atlas, s, anim, tick, x, y, false);
        }
        None => {
            let top = y - AMPEL_H;
            fb.fill_rect(x - 1, top + 18, 2, AMPEL_H - 18, STONE_GREY);
            fb.fill_rect(x - 4, top, 8, 18, OUTLINE);
            let lit = match light {
                AmpelLight::Red => [true, false, false],
                AmpelLight::RedAmber => [true, true, false],
                AmpelLight::Green | AmpelLight::GreenBlink { on: true } => [false, false, true],
                AmpelLight::GreenBlink { on: false } => [false, false, false],
                AmpelLight::Amber => [false, true, false],
            };
            for (i, (on, colour)) in lit
                .into_iter()
                .zip([DIRNDL_RED, MOZART_YELLOW, MEADOW])
                .enumerate()
            {
                let c = if on { colour } else { SLATE_DARK };
                fb.fill_rect(x - 2, top + 1 + 6 * i as i32, 4, 4, c);
            }
        }
    }
    if light == AmpelLight::RedAmber && (game.frame() / BUS_HAZARD_TICKS).is_multiple_of(2) {
        fb.fill_rect(x - 6, y - 7, 2, 2, MOZART_YELLOW);
        fb.fill_rect(x + 4, y - 7, 2, 2, MOZART_YELLOW);
    }
}

/// A car grey box's marks (fallback without the `auto` sprite): a `d` window band
/// over the upper half (the cabin) and a `Y` headlight at the front (left).
fn draw_car_marks(fb: &mut Framebuffer, x: i32, y: i32, w: i32, h: i32) {
    fb.fill_rect(x + 4, y + 2, w - 8, h / 2 - 2, SLATE_DARK);
    fb.fill_rect(x + 1, y + h - 5, 2, 2, MOZART_YELLOW);
}

/// The Postbus grey box's marks (fallback without the `postbus` sprite, D-097):
/// the Reisebus's window band and blinking hazard lights, and at its halt
/// faces (`e`) pressed to the windows instead of photo flashes.
fn draw_postbus_marks(fb: &mut Framebuffer, x: i32, y: i32, w: i32, h: i32, pose: Pose, tick: u32) {
    match pose {
        Pose::Fotostopp { .. } => {
            draw_bus_marks(fb, x, y, w, h, Pose::Idle, tick);
            for i in 1..=5 {
                fb.fill_rect(x + i * w / 6 - 1, y + 5, 3, 3, SKIN_LIGHT);
            }
        }
        _ => draw_bus_marks(fb, x, y, w, h, pose, tick),
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
            Pose::Liegen,
            Pose::Wake,
            Pose::March,
            Pose::Habtacht,
            Pose::Praesentiert,
            Pose::Bounce,
            Pose::Anstellen,
            Pose::Dash,
            Pose::Verschnaufen,
            Pose::Zahlen,
            Pose::Stun,
            Pose::Verbeugen,
            Pose::Hang,
            Pose::Flutter,
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

    #[test]
    fn the_grey_box_oberkellner_shows_his_tray_up_or_lowered() {
        use jnr_core::Pose;
        // A 16×36 box at (100, 100); facing left or right.
        let tray = |pose: Pose, left: bool| {
            let mut fb = Framebuffer::new();
            draw_tray_marks(&mut fb, 100, 100, 16, pose, left);
            fb
        };
        // Tray up: an `i` line on the box's top, 2 px wider on each side.
        for pose in [
            Pose::Anstellen,
            Pose::Telegraph,
            Pose::Dash,
            Pose::Zahlen,
            Pose::Stun,
        ] {
            let fb = tray(pose, true);
            for x in 98..118 {
                assert_eq!(fb.get(x, 100), Some(SILVER_LIGHT), "{pose:?} x {x}");
            }
            assert_ne!(fb.get(98, 115), Some(SILVER_LIGHT));
        }
        // The breather: lowered to his front side, from his middle down.
        let fb = tray(Pose::Verschnaufen, true);
        assert_ne!(fb.get(105, 100), Some(SILVER_LIGHT), "no tray on top");
        assert_eq!(fb.get(98, 115), Some(SILVER_LIGHT), "facing left");
        assert_eq!(fb.get(99, 124), Some(SILVER_LIGHT));
        let fb = tray(Pose::Verschnaufen, false);
        assert_eq!(fb.get(116, 115), Some(SILVER_LIGHT), "facing right");
        // Waiting (the tray is on the counter) and bowing: none.
        for pose in [Pose::Idle, Pose::Verbeugen] {
            let fb = tray(pose, true);
            assert_ne!(fb.get(105, 100), Some(SILVER_LIGHT), "{pose:?}");
            assert_ne!(fb.get(98, 115), Some(SILVER_LIGHT), "{pose:?}");
        }
        assert_eq!(
            grey_box_colour(EntityKind::Enemy(EnemyKind::Oberkellner)),
            SLATE_DARK
        );
        assert_eq!(grey_box_colour(EntityKind::Rechnung), WHITE);
        assert_eq!(Layer::of(EntityKind::Rechnung), Layer::Enemies);
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
            Popup::Hackl,
            Popup::Ballast,
            Popup::Bravo,
            Popup::Beleidigt,
            Popup::Kurruhe,
            Popup::Abtreten,
            Popup::Stollen,
            Popup::KommGleich,
            Popup::Anstellen,
            Popup::Unerhoert,
            Popup::BitteSehr,
        ] {
            let key = p.text_key().unwrap();
            assert!(texts.contains(key), "{key}");
            // Every text popup's key is listed as an engine key.
            assert!(crate::screens::TEXT_KEYS.contains(&key), "{key}");
        }
        // Exhaustive on purpose (no `_` arm): a new popup kind fails to
        // compile here until it is added to the list above.
        let listed = |p: Popup| match p {
            Popup::Score(_) => false,
            Popup::ExtraLife
            | Popup::Checkpoint
            | Popup::Wurstkranz
            | Popup::Hoppala
            | Popup::Hackl
            | Popup::Ballast
            | Popup::Bravo
            | Popup::Beleidigt
            | Popup::Kurruhe
            | Popup::Abtreten
            | Popup::Stollen
            | Popup::KommGleich
            | Popup::Anstellen
            | Popup::Unerhoert
            | Popup::BitteSehr => true,
        };
        assert!(listed(Popup::BitteSehr) && !listed(Popup::Score(100)));
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
            for y in zy..zy + z.h.to_px() {
                for x in zx..zx + z.w.to_px() {
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
            EntityKind::Enemy(EnemyKind::Kurgast),
            EntityKind::Fensterladen,
            // The Offizier shares the Kurgast's STONE_GREY (the palette has no
            // colour left; both draw their sprites, the grey box is a fallback).
            // His box differs by its hilt, flag and sabre marks.
            // The Kellner shares the Operettensänger's SLATE_DARK (D-105);
            // his box differs by its white apron and silver tray line.
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

    /// A 20×10 grey-box level, ground in row 9, the player at (2, 8) and a
    /// Fensterladen `{ entity = "fensterladen", period = 120, open = 60 }`
    /// in cell (8, 5): body x 128..160, y 80..88. Its cycle on the stage
    /// clock: open 0..30, rattle 30..60, closed 60..100, warn 100..120.
    fn shutter_game() -> Game {
        let mut rows: Vec<Vec<char>> = (0..10)
            .map(|r| vec![if r == 9 { '#' } else { '.' }; 20])
            .collect();
        rows[8][2] = 'P';
        rows[5][8] = 'f';
        let grid: String = rows
            .into_iter()
            .map(|r| r.into_iter().collect::<String>() + "\n")
            .collect();
        let text = format!(
            "[meta]\nid = \"t\"\ntime_limit = 300\n[legend]\n\"#\" = \"solid\"\n\
             \"f\" = {{ entity = \"fensterladen\", period = 120, open = 60 }}\n\
             \"P\" = {{ entity = \"player_start\" }}\n[map]\ngrid = '''\n{grid}'''\n"
        );
        Game::new(&Level::parse(&text).unwrap(), &tuning(), 1)
    }

    fn the_shutter(g: &Game) -> Entity {
        *g.entities()
            .iter()
            .find(|e| e.kind == EntityKind::Fensterladen)
            .unwrap()
    }

    /// Grey box (D-094): open, the red body; rattling, the body shaken 1 px
    /// down every other 4 ticks while the core body stays put; closed, only
    /// the two folded-up leaves (no ledge); warning, the leaves and the
    /// landlady's face. The look follows the stage clock.
    #[test]
    fn the_grey_box_fensterladen_shows_open_rattle_closed_and_warn() {
        use jnr_core::platform::{ShutterLook, shutter_look};
        let mut g = shutter_game();
        let e = the_shutter(&g);
        let (left, top) = (e.x.to_px() - g.camera().x, e.y.to_px() - g.camera().y);
        assert_eq!((e.w.to_px(), e.h.to_px()), (32, 8));
        // (the ledge 2 px below the top, the row just under the ledge is
        // its shaken outline, a folded-up leaf, the landlady's face)
        let probe = |g: &Game| {
            let fb = rendered(g);
            (
                fb.get(left + 16, top + 2) == Some(DIRNDL_RED),
                fb.get(left + 16, top + 8) == Some(OUTLINE),
                fb.get(left + 1, top + 12) == Some(DIRNDL_RED),
                fb.get(left + 16, top + 6) == Some(SKIN_LIGHT),
            )
        };
        let mut seen = Vec::new();
        for clock in 0..120 {
            assert_eq!(g.stage_clock(), clock);
            let look = shutter_look(&the_shutter(&g), g.level(), g.tuning()).unwrap();
            seen.push((look, probe(&g), g.frame()));
            g.step(InputFrame::NONE);
        }
        for (clock, &(look, probe, frame)) in seen.iter().enumerate() {
            let shaken = (frame / FENSTERLADEN_RATTLE_TICKS) % 2 == 1;
            let want = match clock {
                0..30 => (ShutterLook::Open, (true, false, false, false)),
                30..60 => (ShutterLook::Rattle, (true, shaken, false, false)),
                60..100 => (ShutterLook::Closed, (false, false, true, false)),
                _ => (ShutterLook::Warn, (false, false, true, true)),
            };
            assert_eq!((look, probe), want, "clock {clock}");
        }
        assert!(seen[30..60].iter().any(|s| s.1.1), "it shakes");
        // The body itself never moves.
        let now = the_shutter(&g);
        assert_eq!((now.x, now.y), (e.x, e.y));
    }

    /// With its sprite: the anim named after the look, drawn with its
    /// origin 24 px below the body's top (the sprite spans the body's top
    /// row down to 23 px below it), never mirrored.
    #[test]
    fn the_fensterladen_picks_its_anim_from_the_stage_clock() {
        use crate::testutil::{A, B, C, atlas, image, sprite};
        let frame = |c: &str| image(&vec![c.repeat(32).as_str(); 24]);
        let shutter = sprite(
            "fensterladen",
            (16, 24),
            false,
            vec![
                ("open", 0, vec![frame("a")]),
                ("rattle", 0, vec![frame("b")]),
                ("closed", 0, vec![frame("c")]),
                ("warn", 0, vec![frame("b")]),
            ],
        );
        let atlas = atlas(&[shutter]);
        let texts = Texts::default();
        let mut g = shutter_game();
        let e = the_shutter(&g);
        let (left, top) = (e.x.to_px() - g.camera().x, e.y.to_px() - g.camera().y);
        let shown = |g: &Game| {
            let ctx = RenderContext::new(&atlas, &texts, g.level());
            let mut fb = Framebuffer::new();
            render(g, &ctx, &mut fb);
            let inside = (fb.get(left, top), fb.get(left + 31, top + 23));
            let outside = [fb.get(left + 16, top - 1), fb.get(left + 16, top + 24)];
            assert!(!outside.contains(&inside.0), "{outside:?} vs {inside:?}");
            inside
        };
        let mut seen = Vec::new();
        for _ in 0..120 {
            seen.push(shown(&g));
            g.step(InputFrame::NONE);
        }
        assert!(seen[..30].iter().all(|&c| c == (Some(A), Some(A))));
        assert!(seen[30..60].iter().all(|&c| c == (Some(B), Some(B))));
        assert!(seen[60..100].iter().all(|&c| c == (Some(C), Some(C))));
        assert!(seen[100..].iter().all(|&c| c == (Some(B), Some(B))));
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

    // ------------------------------------------------------ Kurgast (D-095)

    /// A grey-box level with a Kurgast at (10, 8) on the ground (feet y
    /// 144), stepped once; returns the game and the Kurgast in `st`
    /// (`LIE`/`WAKE`: resized to the 16×16 chair at his feet).
    fn kurgast_in(st: u8) -> (Game, Entity) {
        let g = layered_game(
            "....................\n....................\n....................\n\
             ....................\n....................\n....................\n\
             ....................\n....................\n..P.......k.........\n\
             ####################\n",
            "\"k\" = { entity = \"kurgast\" }",
        );
        let mut k = *g
            .entities()
            .iter()
            .find(|e| e.kind == EntityKind::Enemy(EnemyKind::Kurgast))
            .unwrap();
        k.state = st;
        if st != jnr_core::enemies::state::WALK {
            let (cx, feet) = k.feet();
            k.w = Fx::from_px(16);
            k.h = Fx::from_px(16);
            k.x = cx - Fx::from_px(8);
            k.y = feet - k.h;
        }
        (g, k)
    }

    fn drawn(g: &Game, atlas: &Atlas, k: &Entity) -> Framebuffer {
        let looks = EntityLooks::new(atlas);
        let mut fb = crate::testutil::fb();
        draw_enemy(g, &looks, atlas, &mut fb, g.camera(), k);
        fb
    }

    #[test]
    fn the_grey_box_kurgast_shows_his_chair_and_the_wake_arms() {
        use jnr_core::enemies::state;
        let atlas = crate::testutil::atlas(&[]);
        let (g, walking) = kurgast_in(state::WALK);
        let fb = drawn(&g, &atlas, &walking);
        let (x, y) = (walking.x.to_px() + 6, walking.y.to_px() + 11);
        assert_eq!(fb.get(x, y), Some(STONE_GREY), "walking: a grey box");
        let (g, lying) = kurgast_in(state::LIE);
        let fb = drawn(&g, &atlas, &lying);
        let (bx, by) = (lying.x.to_px(), lying.y.to_px());
        assert_eq!(fb.get(bx + 8, by + 3), Some(STONE_GREY), "the chair");
        assert_eq!(fb.get(bx + 8, by + 7), Some(WINE_DARK), "its canvas stripe");
        assert_eq!(fb.get(bx + 3, by - 3), Some(crate::testutil::BG), "no arms");
        let (g, waking) = kurgast_in(state::WAKE);
        let fb = drawn(&g, &atlas, &waking);
        assert_eq!(
            fb.get(bx + 3, by - 3),
            Some(WHITE),
            "arms up: the telegraph"
        );
        assert_eq!(fb.get(bx + 12, by - 3), Some(WHITE));
    }

    #[test]
    fn a_lying_kurgast_uses_the_liegestuhl_sprite() {
        use crate::testutil::{A, B, C, image, sprite};
        use jnr_core::enemies::state;
        let one = |c: &str| vec![image(&[c; 4])];
        let atlas = crate::testutil::atlas(&[
            sprite("kurgast", (2, 4), true, vec![("walk", 0, one("cccc"))]),
            sprite(
                LIEGESTUHL_SPRITE,
                (2, 4),
                false,
                vec![("idle", 0, one("aaaa")), ("wake", 0, one("bbbb"))],
            ),
        ]);
        for (st, colour) in [(state::WALK, C), (state::LIE, A), (state::WAKE, B)] {
            let (g, k) = kurgast_in(st);
            let fb = drawn(&g, &atlas, &k);
            let (fx, fy) = k.feet();
            assert_eq!(
                fb.get(fx.to_px(), fy.to_px() - 1),
                Some(colour),
                "state {st}"
            );
        }
    }

    // ------------------------------------------- Kellner, Wartende (D-105)

    /// A grey-box level with a Kellner at (6, 8) and Wartende at (9, 8),
    /// (10, 8) and (11, 8) (looks 0, 1, 2) on the ground, stepped once.
    fn cafe_game() -> Game {
        layered_game(
            "....................\n....................\n....................\n\
             ....................\n....................\n....................\n\
             ....................\n....................\n..P...K..WWW........\n\
             ####################\n",
            "\"K\" = { entity = \"kellner\" }\n\"W\" = { entity = \"wartender\" }",
        )
    }

    fn of(g: &Game, kind: EntityKind) -> Vec<Entity> {
        g.entities()
            .iter()
            .filter(|e| e.kind == kind)
            .copied()
            .collect()
    }

    #[test]
    fn the_grey_box_kellner_shows_his_apron_and_tray_line_dipping_on_a_bounce() {
        use jnr_core::enemies::state;
        let atlas = crate::testutil::atlas(&[]);
        let g = cafe_game();
        let mut k = of(&g, EntityKind::Enemy(EnemyKind::Kellner))[0];
        let fb = drawn(&g, &atlas, &k);
        let (bx, by) = (k.x.to_px() - g.camera().x, k.y.to_px() - g.camera().y);
        let (w, h) = (k.w.to_px(), k.h.to_px());
        assert_eq!(fb.get(bx + w / 2, by), Some(SILVER_LIGHT), "the tray line");
        assert_eq!(fb.get(bx + w / 2, by + KELLNER_TRAY_PX), Some(SLATE_DARK));
        assert_eq!(fb.get(bx + w / 2, by + h - 3), Some(WHITE), "the apron");
        assert_eq!(
            fb.get(bx + w / 2, by + h / 4),
            Some(SLATE_DARK),
            "waistcoat"
        );
        k.state = state::TRAY_BOUNCE;
        let fb = drawn(&g, &atlas, &k);
        assert_eq!(
            fb.get(bx + w / 2, by),
            Some(OUTLINE),
            "dipped: the box outline"
        );
        assert_eq!(
            fb.get(bx + w / 2, by + KELLNER_TRAY_DIP_PX),
            Some(SILVER_LIGHT)
        );
    }

    #[test]
    fn the_kellner_sprite_shows_bounce_after_a_bounce_else_walk() {
        use crate::testutil::{A, B, image, sprite};
        use jnr_core::enemies::state;
        let one = |c: &str| vec![image(&[c; 4])];
        let atlas = crate::testutil::atlas(&[sprite(
            "kellner",
            (2, 4),
            true,
            vec![("walk", 0, one("aaaa")), ("bounce", 0, one("bbbb"))],
        )]);
        let g = cafe_game();
        let mut k = of(&g, EntityKind::Enemy(EnemyKind::Kellner))[0];
        for (st, colour) in [(state::WALK, A), (state::TRAY_BOUNCE, B)] {
            k.state = st;
            let fb = drawn(&g, &atlas, &k);
            let (fx, fy) = k.feet();
            let (x, y) = (fx.to_px() - g.camera().x, fy.to_px() - 1 - g.camera().y);
            assert_eq!(fb.get(x, y), Some(colour), "state {st}");
        }
        assert_eq!(pose_anim(Pose::Bounce), ("bounce", None));
    }

    #[test]
    fn wartende_pick_their_look_by_column_and_always_face_right() {
        use crate::testutil::{A, B, C, image, sprite};
        // Each look's anims in its own colour, a `.` column on the right
        // half: mirrored, the colour would show right of the feet instead.
        let half = |c: char| {
            let row = format!("{c}{c}..");
            vec![image(&[row.as_str(); 4])]
        };
        let mut anims = Vec::new();
        for (i, c) in ['a', 'b', 'c'].into_iter().enumerate() {
            anims.push((WARTENDER_IDLE[i], 0, half(c)));
            anims.push((WARTENDER_SELFIE[i], 0, half(c)));
            anims.push((WARTENDER_SQUISHED[i], 0, half(c)));
        }
        let atlas = crate::testutil::atlas(&[sprite(WARTENDER_SPRITE, (2, 4), true, anims)]);
        let g = cafe_game();
        let ws = of(&g, EntityKind::Enemy(EnemyKind::SelfieTourist));
        assert_eq!(ws.len(), 3);
        for (mut w, colour) in ws.into_iter().zip([A, B, C]) {
            let look = wartender_look(&g, &w).unwrap();
            assert_eq!(look, (w.feet().0.to_px() / 16) as usize % 3);
            // Facing left in core would mirror any other enemy.
            w.dir = -1;
            let fb = drawn(&g, &atlas, &w);
            let (fx, fy) = w.feet();
            let (x, y) = (fx.to_px() - g.camera().x, fy.to_px() - 1 - g.camera().y);
            assert_eq!(fb.get(x - 1, y), Some(colour), "look {look}");
            assert_eq!(fb.get(x + 1, y), Some(crate::testutil::BG), "not mirrored");
        }
        assert_eq!(WARTENDER_LOOKS, 3);
    }

    #[test]
    fn wartender_anims_follow_the_selfie_tourist_poses() {
        assert_eq!(wartender_anim(Pose::Walk, 0), ("idle_0", None));
        assert_eq!(
            wartender_anim(Pose::Selfie { flash: false }, 1),
            ("selfie_1", Some(0))
        );
        assert_eq!(
            wartender_anim(Pose::Selfie { flash: true }, 2),
            ("selfie_2", Some(1))
        );
        assert_eq!(wartender_anim(Pose::Squished, 2), ("squished_2", None));
        assert_eq!(wartender_anim(Pose::Tumble, 0), ("squished_0", None));
        let catalog = include_str!("../../../docs/sprite-catalog.md");
        for n in [
            "idle_0",
            "selfie_0",
            "squished_0",
            "`wartender`",
            "`kellner`",
        ] {
            assert!(catalog.contains(n), "{n} not in the catalog");
        }
    }

    #[test]
    fn a_wartender_without_his_sprite_is_the_selfie_tourist_grey_box() {
        use crate::testutil::{A, image, sprite};
        // The Selfie-Tourist's sprite exists, the Wartender's doesn't: a
        // plain tourist draws its sprite, a Wartender the grey box.
        let one = |c: &str| vec![image(&[c; 4])];
        let atlas = crate::testutil::atlas(&[sprite(
            "selfie_tourist",
            (2, 4),
            true,
            vec![("walk", 0, one("aaaa"))],
        )]);
        let g = cafe_game();
        let w = of(&g, EntityKind::Enemy(EnemyKind::SelfieTourist))[0];
        let fb = drawn(&g, &atlas, &w);
        let (fx, fy) = w.feet();
        let (x, y) = (fx.to_px() - g.camera().x, fy.to_px() - 1 - g.camera().y);
        assert_eq!(fb.get(x, y - 4), Some(LAKE_DEEP));
        assert_ne!(fb.get(x, y), Some(A));
        // A plain Selfie-Tourist is not a Wartender.
        let plain = Entity { spawn: None, ..w };
        assert_eq!(wartender_look(&g, &plain), None);
        let fb = drawn(&g, &atlas, &plain);
        assert_eq!(fb.get(x, y), Some(A));
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

    // ------------------------------------------------- traffic (M6, E36)

    /// A 24x10 traffic grey box (ground in row 9): Resi on a one-way
    /// shelter (row 6, columns 1–4) at column 2, above the lane; plus
    /// `marks` (an Ampel in cell (12, 8) has its lamps at x 198–201 from
    /// y 97).
    fn traffic_game(marks: &[(usize, usize, char)]) -> Game {
        let mut rows: Vec<Vec<char>> = (0..10)
            .map(|r| vec![if r == 9 { '#' } else { '.' }; 24])
            .collect();
        rows[6][1..=4].fill('-');
        rows[5][2] = 'P';
        for &(r, c, ch) in marks {
            rows[r][c] = ch;
        }
        let grid: String = rows
            .into_iter()
            .map(|r| r.into_iter().collect::<String>() + "\n")
            .collect();
        let text = format!(
            "[meta]\nid = \"t\"\ntime_limit = 300\n[legend]\n\"#\" = \"solid\"\n\"-\" = \"oneway\"\n\
             \"a\" = {{ entity = \"ampel\" }}\n\
             \"y\" = {{ entity = \"reisebus\", stop = 2, look = \"postbus\" }}\n\
             \"r\" = {{ entity = \"reisebus\", stop = 2 }}\n\
             \"P\" = {{ entity = \"player_start\" }}\n[map]\ngrid = '''\n{grid}'''\n"
        );
        Game::new(&Level::parse(&text).unwrap(), &tuning(), 1)
    }

    /// The grey-box Ampel's three lamps (top red, middle amber, bottom
    /// green) at the counter frames of D-093's cycle, the headlights in
    /// the mouth during red and amber, and its cars in front of it in their
    /// looks' colours.
    #[test]
    fn the_grey_box_ampel_shows_its_cycle_and_the_cars_their_looks() {
        let mut g = traffic_game(&[(8, 12, 'a')]);
        let lamps = |fb: &Framebuffer| [98, 104, 110].map(|y| fb.get(200, y).unwrap());
        let dark = SLATE_DARK;
        g.step(InputFrame::NONE);
        let fb = rendered(&g);
        assert_eq!(
            lamps(&fb),
            [DIRNDL_RED, MOZART_YELLOW, dark],
            "red and amber"
        );
        assert_eq!(fb.get(194, 137), Some(MOZART_YELLOW), "headlights");
        assert_eq!(fb.get(204, 137), Some(MOZART_YELLOW));
        assert_eq!(fb.get(200, 120), Some(STONE_GREY), "the pole");
        for _ in 1..41 {
            g.step(InputFrame::NONE);
        }
        let fb = rendered(&g);
        assert_eq!(lamps(&fb), [dark, dark, MEADOW], "green");
        assert_ne!(fb.get(194, 137), Some(MOZART_YELLOW), "no headlights");
        for _ in 41..112 {
            g.step(InputFrame::NONE);
        }
        // Cars spawned at 30, 70, 110 (2 px/frame left from x 194).
        let fb = rendered(&g);
        for (x, colour) in [(32, DIRNDL_RED), (112, WHITE), (192, SKY_HAZE)] {
            assert_eq!(fb.get(x + 16, 141), Some(colour), "car at {x}");
            assert_eq!(fb.get(x + 10, 134), Some(SLATE_DARK), "its windows");
        }
        assert_eq!(fb.get(200, 141), Some(SKY_HAZE), "in front of the pole");
        for _ in 112..159 {
            g.step(InputFrame::NONE);
        }
        let a = g.entities().iter().find(|e| e.kind == EntityKind::Ampel);
        assert_eq!(
            jnr_core::enemies::ampel_light(a.unwrap(), g.level(), g.tuning()),
            Some(AmpelLight::GreenBlink { on: false })
        );
        assert_eq!(lamps(&rendered(&g)), [dark, dark, dark], "a blink off");
    }

    #[test]
    fn ampel_anims_follow_the_light() {
        assert_eq!(ampel_anim(AmpelLight::Red), ("rot", None));
        assert_eq!(ampel_anim(AmpelLight::RedAmber), ("rotgelb", None));
        assert_eq!(ampel_anim(AmpelLight::Green), ("gruen", None));
        assert_eq!(
            ampel_anim(AmpelLight::GreenBlink { on: true }),
            ("gruen_blink", Some(0))
        );
        assert_eq!(
            ampel_anim(AmpelLight::GreenBlink { on: false }),
            ("gruen_blink", Some(1))
        );
        assert_eq!(ampel_anim(AmpelLight::Amber), ("gelb", None));
        let catalog = include_str!("../../../docs/sprite-catalog.md");
        let names = [
            "rot",
            "rotgelb",
            "gruen",
            "gruen_blink",
            "gelb",
            "halt",
            "postbus",
            "ampel",
        ];
        for n in names.into_iter().chain(AUTO_DRIVE) {
            assert!(
                catalog.contains(&format!("`{n}`")),
                "`{n}` not in the catalog"
            );
        }
    }

    /// With the sprites: the Ampel in its light's anim at the cell's
    /// bottom centre; a car's look by its spawn index (`drive_2` missing:
    /// `drive`); the Postbus's `drive` and its `halt` at its stop.
    #[test]
    fn traffic_sprites_pick_their_anims() {
        use crate::testutil::{A, B, C};
        let s = |name: &str, anims: Vec<(&str, u32, Vec<jnr_artkit::Image>)>| {
            crate::testutil::sprite(name, (8, 24), true, anims)
        };
        let atlas = crate::testutil::atlas(&[
            s(
                "ampel",
                vec![
                    ("rot", 1, vec![solid('c')]),
                    ("rotgelb", 1, vec![solid('a')]),
                    ("gruen", 1, vec![solid('b')]),
                ],
            ),
            s(
                "auto",
                vec![
                    ("drive", 4, vec![solid('c')]),
                    ("drive_0", 4, vec![solid('a')]),
                    ("drive_1", 4, vec![solid('b')]),
                ],
            ),
            s(
                "postbus",
                vec![
                    ("drive", 4, vec![solid('b')]),
                    ("halt", 16, vec![solid('a')]),
                ],
            ),
        ]);
        let texts = Texts::default();
        let shown = |g: &Game, x: i32, y: i32| {
            let ctx = RenderContext::new(&atlas, &texts, g.level());
            let mut fb = Framebuffer::new();
            render(g, &ctx, &mut fb);
            fb.get(x, y)
        };
        let mut g = traffic_game(&[(8, 12, 'a')]);
        g.step(InputFrame::NONE);
        assert_eq!(shown(&g, 200, 130), Some(A), "rotgelb");
        for _ in 1..112 {
            g.step(InputFrame::NONE);
        }
        assert_eq!(shown(&g, 194, 125), Some(B), "gruen (left of the car)");
        // Car feet at x + 14 (the 16 px frames centred there).
        assert_eq!(shown(&g, 46, 140), Some(A), "drive_0");
        assert_eq!(shown(&g, 126, 140), Some(B), "drive_1");
        assert_eq!(shown(&g, 206, 140), Some(C), "drive_2 falls back to drive");
        // The Postbus (stop 2) at column 10 triggers at once (152 px) and
        // halts after 60 + 22 frames at column 8.
        let mut g = traffic_game(&[(8, 10, 'y')]);
        for _ in 0..70 {
            g.step(InputFrame::NONE);
        }
        let bus = |g: &Game| {
            let b = g
                .entities()
                .iter()
                .find(|e| e.kind == EntityKind::Enemy(EnemyKind::Reisebus));
            (b.unwrap().feet().0.to_px(), b.unwrap().pose(g.tuning()))
        };
        let (x, pose) = bus(&g);
        assert_eq!(pose, Some(Pose::Drive));
        assert_eq!(shown(&g, x, 140), Some(B), "drives");
        for _ in 70..100 {
            g.step(InputFrame::NONE);
        }
        let (x, pose) = bus(&g);
        assert!(matches!(pose, Some(Pose::Fotostopp { .. })));
        assert_eq!(shown(&g, x, 140), Some(A), "halt");
    }

    /// The approved traffic art is embedded and has every anim the renderer
    /// asks for, so the grey boxes are only fallbacks.
    #[test]
    fn the_real_atlas_has_the_traffic_art() {
        let looks = EntityLooks::new(jnr_assets::atlas());
        let ampel = looks.ampel.expect("art/sprites/props/ampel.sprite");
        for light in [
            AmpelLight::Red,
            AmpelLight::RedAmber,
            AmpelLight::Green,
            AmpelLight::GreenBlink { on: true },
            AmpelLight::Amber,
        ] {
            let (name, frame) = ampel_anim(light);
            let anim = ampel.anim(name).unwrap_or_else(|| panic!("ampel `{name}`"));
            assert!(frame.unwrap_or(0) < anim.frames.len(), "ampel `{name}`");
        }
        let auto = looks.auto.expect("art/sprites/enemies/auto.sprite");
        for look in AUTO_DRIVE {
            assert!(auto.anim(look).is_some(), "auto `{look}`");
        }
        let postbus = looks.postbus.expect("art/sprites/enemies/postbus.sprite");
        for name in ["idle", "telegraph", "drive", "halt"] {
            assert!(postbus.anim(name).is_some(), "postbus `{name}`");
        }
    }

    /// The approved Offizier art is embedded with every pose anim, and drawn
    /// (not the grey box): in „Präsentiert!" everything above his hitbox
    /// top is the blade, inside the sabre zone (cols 8–11, its tip at
    /// sprite row 0); the flag bearer's flag above row 10 stays in cols
    /// 0–2 (mirrored when he faces left), clear of the zone, in every pose.
    #[test]
    fn the_real_offizier_art_draws_the_sabre_inside_the_zone() {
        let atlas = jnr_assets::atlas();
        let looks = EntityLooks::new(atlas);
        for (name, s) in [
            ("offizier", looks.offizier),
            ("offizier_lead", looks.offizier_lead),
        ] {
            let s = s.unwrap_or_else(|| panic!("art/sprites/enemies/{name}.sprite"));
            for pose in [
                Pose::March,
                Pose::Habtacht,
                Pose::Praesentiert,
                Pose::Squished,
            ] {
                let (anim, _) = pose_anim(pose);
                assert!(s.anim(anim).is_some(), "{name} `{anim}`");
            }
        }
        let t = tuning();
        for frames in [10, 170, 200] {
            let g = officer_game(frames);
            for e in officers(&g) {
                let fb = drawn(&g, atlas, &e);
                let cam = g.camera();
                let top = e.y.to_px() - cam.y;
                let left = (e.x + e.w.div_int(2)).to_px() - cam.x - 8;
                // Sprite-local column (as authored, facing right).
                let col = |x: i32| {
                    if e.facing_left() {
                        15 - (x - left)
                    } else {
                        x - left
                    }
                };
                let above: Vec<(i32, i32)> = (top - 10..top)
                    .flat_map(|y| (left..left + 16).map(move |x| (x, y)))
                    .filter(|&(x, y)| fb.get(x, y) != Some(crate::testutil::BG))
                    .collect();
                let zone = jnr_core::enemies::sabre_zone(&e, &t);
                let lead = is_lead(&g, &e);
                if let Some(z) = zone {
                    let (zx, zy) = (z.x.to_px() - cam.x, z.y.to_px() - cam.y);
                    assert!(above.iter().any(|&(_, y)| y == top - 10), "tip at row 0");
                    for &(x, y) in &above {
                        let in_zone = (zx..zx + z.w.to_px()).contains(&x)
                            && (zy..zy + z.h.to_px()).contains(&y);
                        assert!(
                            in_zone || (lead && col(x) <= 2),
                            "frame {frames}: ({x}, {y}) col {} outside the zone",
                            col(x)
                        );
                        if in_zone {
                            assert!((8..=11).contains(&col(x)), "blade col {}", col(x));
                        }
                    }
                } else {
                    for &(x, _) in &above {
                        assert!(lead && col(x) <= 2, "frame {frames}: col {}", col(x));
                    }
                }
                // Up in the march and „Präsentiert!", dipped below row 10 in
                // „Habt Acht!".
                if lead && e.pose(&t) != Some(Pose::Habtacht) {
                    assert!(above.iter().any(|&(x, _)| col(x) <= 2), "the flag");
                }
            }
        }
    }

    /// Without the `postbus` art its grey box is yellow, and at its halt
    /// faces fill the windows instead of photo flashes.
    #[test]
    fn the_grey_box_postbus_is_yellow_with_faces_at_its_halt() {
        let body = |g: &Game| rendered(g).get(150, 140);
        let face = |g: &Game| rendered(g).get(140, 122);
        let mut post = traffic_game(&[(8, 10, 'y')]);
        let mut plain = traffic_game(&[(8, 10, 'r')]);
        for _ in 0..100 {
            post.step(InputFrame::NONE);
            plain.step(InputFrame::NONE);
        }
        assert_eq!(body(&post), Some(MOZART_YELLOW));
        assert_eq!(face(&post), Some(SKIN_LIGHT), "faces at the halt");
        assert_eq!(body(&plain), Some(WHITE), "a plain bus stays white");
        assert_ne!(face(&plain), Some(SKIN_LIGHT));
        for _ in 100..215 {
            post.step(InputFrame::NONE);
        }
        assert_ne!(face(&post), Some(SKIN_LIGHT), "driving on: no faces");
    }

    /// E51: the Oberkellner's hit labels, centred
    /// `OBERKELLNER_POPUP_SIDE_PX` beside him, keep clear of his sprite
    /// cell (either facing) with the real font: each label's outline
    /// starts at least 1 px beyond the cell.
    #[test]
    fn boss_hit_labels_clear_his_sprite() {
        let atlas = jnr_assets::atlas();
        let font = crate::Font::from_atlas(atlas);
        let texts = Texts::parse(include_str!("../../../content/text/de.toml")).unwrap();
        let s = atlas.sprite("oberkellner").expect("the sprite");
        let half = s.origin.x.max(s.width - s.origin.x) as i32;
        let t = tuning();
        let mut labels: Vec<String> = [Popup::Unerhoert, Popup::BitteSehr]
            .iter()
            .map(|p| texts.get(p.text_key().unwrap()).into_owned())
            .collect();
        labels.push(t.enemies.oberkellner_hit_points.to_string());
        labels.push(t.enemies.oberkellner_final_points.to_string());
        for label in labels {
            let w = font.measure(&label);
            assert!(w > 0, "{label}");
            // His cell around x 0: columns -half..half-1 (either facing).
            // A label is drawn from `centre - w / 2` over w columns, with
            // its outline 1 px around: columns left-1..=left+w.
            let side = jnr_core::game::OBERKELLNER_POPUP_SIDE_PX;
            for centre in [side, -side] {
                let left = centre - w / 2;
                let (first, last) = (left - 1, left + w);
                assert!(
                    first > half - 1 || last < -half,
                    "{label} ({w} px) at {centre}: columns {first}..={last} touch the cell"
                );
            }
        }
    }
}
