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
//! 3. [`draw_entities`]: items, props, enemies (animation by
//!    `Entity::pose`, facing by `Entity::dir`), the Festungsbahn
//! 4. player (scene)
//! 5. [`draw_front`]: debris and popups

use jnr_artkit::atlas::{Atlas, AtlasAnim, AtlasSprite};
use jnr_core::entity::{EnemyKind, Entity, EntityKind, Popup};
use jnr_core::{Block, Camera, Game, Pose, SignStyle, Tile};

use crate::Framebuffer;
use crate::blit::draw_sprite;
use crate::font::Font;
use crate::palette::{LEATHER_DARK, OUTLINE, WHITE};

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

/// Sign board sprite per style.
pub const SIGN_SPRITES: [&str; 3] = ["sign_street", "sign_bus", "sign_ortsende"];
/// The red bar drawn over the Ortsende sign's text.
pub const ORTSENDE_BAR_SPRITE: &str = "sign_ortsende_bar";

/// Grey-box sign board (fits 2 lines of 12 characters).
const BOARD_W: i32 = 78;
const BOARD_H: i32 = 22;
/// Grey-box sign post below the board.
const POST_H: i32 = 10;

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
    signs: [Option<&'a AtlasSprite>; 3],
    ortsende_bar: Option<&'a AtlasSprite>,
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
            signs: SIGN_SPRITES.map(s),
            ortsende_bar: s(ORTSENDE_BAR_SPRITE),
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
            },
            EntityKind::Thrown => self.thrown,
            EntityKind::Popup(_) | EntityKind::Bump | EntityKind::Sign => None,
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
        },
        EntityKind::Thrown => LEATHER_DARK,
        EntityKind::Festungsbahn => LAKE_TEAL,
        EntityKind::Popup(_) | EntityKind::Bump => WHITE,
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
    for e in game.entities() {
        match e.kind {
            EntityKind::Debris | EntityKind::Popup(_) | EntityKind::Bump => {}
            EntityKind::Sign => draw_sign(game, looks, atlas, font, texts, fb, cam, e),
            EntityKind::Checkpoint => draw_checkpoint(game, looks, atlas, fb, cam, e),
            EntityKind::Enemy(_) | EntityKind::Defeated(_) | EntityKind::Thrown => {
                draw_enemy(game, looks, atlas, fb, cam, e)
            }
            _ if e.rising(game.tuning()) => {}
            _ => draw_body(game, looks, atlas, fb, cam, e),
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
            EntityKind::Debris => draw_body(game, looks, atlas, fb, cam, e),
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
    }
}

/// An enemy, a defeated enemy or a thrown Mozartkugel: the animation of
/// its pose (missing anims fall back to the first one), facing `dir`
/// (the Fiaker art faces left and is never mirrored). Without a sprite:
/// the hitbox as a grey box, flattened to its bottom quarter when squished.
fn draw_enemy(
    game: &Game,
    looks: &EntityLooks,
    atlas: &Atlas,
    fb: &mut Framebuffer,
    cam: Camera,
    e: &Entity,
) {
    let pose = e.pose(game.tuning()).unwrap_or(Pose::Idle);
    let (x, y) = feet_on_screen(e, cam);
    match looks.sprite(e.kind) {
        Some(s) => {
            let (name, frame) = pose_anim(pose);
            let anim = s.anim(name).unwrap_or(&s.anims[0]);
            let tick = match frame {
                Some(i) => i as u32 * anim.ticks,
                None => game.frame(),
            };
            let fiaker = matches!(
                e.kind,
                EntityKind::Enemy(EnemyKind::Fiaker) | EntityKind::Defeated(EnemyKind::Fiaker)
            );
            let left = e.facing_left() && !fiaker;
            draw_sprite(fb, atlas, s, anim, tick, x, y, left);
        }
        None => {
            let (w, full) = (e.w.to_px(), e.h.to_px());
            let h = if pose == Pose::Squished {
                (full / 4).max(1)
            } else {
                full
            };
            let bx = e.x.to_px() - cam.x;
            let by = e.y.to_px() - cam.y + full - h;
            fb.fill_rect(bx, by, w, h, grey_box_colour(e.kind));
            fb.outline_rect(bx, by, w, h, OUTLINE);
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

/// A sign: board sprite by style (else a cream board on a post), its text
/// centred line by line, and the red bar over an Ortsende sign.
#[allow(clippy::too_many_arguments)]
fn draw_sign(
    game: &Game,
    looks: &EntityLooks,
    atlas: &Atlas,
    font: &Font,
    texts: &jnr_core::Texts,
    fb: &mut Framebuffer,
    cam: Camera,
    e: &Entity,
) {
    let Some(spawn) = e
        .spawn
        .and_then(|i| game.level().entities.get(usize::from(i)))
    else {
        return;
    };
    let style = spawn.style;
    let (x, y) = feet_on_screen(e, cam);
    let board_top = match looks.signs[style_index(style)] {
        Some(s) => {
            draw_sprite(fb, atlas, s, &s.anims[0], game.frame(), x, y, false);
            y - s.origin.y as i32
        }
        None => {
            fb.fill_rect(x - 1, y - POST_H, 2, POST_H, OUTLINE);
            let (bx, by) = (x - BOARD_W / 2, y - POST_H - BOARD_H);
            let board = if style == SignStyle::Ortsende {
                WHITE
            } else {
                CREAM
            };
            fb.fill_rect(bx, by, BOARD_W, BOARD_H, board);
            fb.outline_rect(bx, by, BOARD_W, BOARD_H, OUTLINE);
            by
        }
    };
    if let Some(key) = &spawn.text {
        let text = texts.get(key);
        for (i, line) in text.split('\n').take(2).enumerate() {
            let w = font.measure(line);
            let ty = board_top + 3 + i as i32 * crate::font::LINE_HEIGHT;
            font.draw_text(fb, atlas, line, x - w / 2, ty, OUTLINE, None);
        }
    }
    if style == SignStyle::Ortsende {
        match looks.ortsende_bar {
            Some(s) => draw_sprite(fb, atlas, s, &s.anims[0], game.frame(), x, y, false),
            None => {
                // A 2 px red diagonal from bottom left to top right.
                let (bx, by) = (x - BOARD_W / 2, y - POST_H - BOARD_H);
                for i in 0..BOARD_W {
                    let py = by + BOARD_H - 1 - i * (BOARD_H - 1) / (BOARD_W - 1);
                    fb.fill_rect(bx + i, py - 1, 1, 2, DIRNDL_RED);
                }
            }
        }
    }
}

const fn style_index(style: SignStyle) -> usize {
    match style {
        SignStyle::Street => 0,
        SignStyle::Bus => 1,
        SignStyle::Ortsende => 2,
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
    font.draw_text(fb, atlas, &text, x - w / 2, y, WHITE, Some(OUTLINE));
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
        ];
        for (i, a) in kinds.iter().enumerate() {
            for b in &kinds[i + 1..] {
                assert_ne!(grey_box_colour(*a), grey_box_colour(*b), "{a:?} vs {b:?}");
            }
        }
    }
}
