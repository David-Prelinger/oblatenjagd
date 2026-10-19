//! Entities: items, particles, popups, props and enemies, as one plain
//! `Vec<Entity>` in `Game` with an [`EntityKind`] enum and per-kind update
//! functions (architecture.md → "Entities"; no ECS).
//!
//! Rules:
//! - Order is deterministic: level spawns in grid order, then everything
//!   spawned later appended in the order it happened. Removal keeps order.
//! - Level-placed entities start inactive and activate when their left edge
//!   comes within `game.activate_margin` px of the right screen edge;
//!   spawned ones (items from boxes, debris, popups) start active. Only
//!   active entities update and interact with the player.
//! - An entity is removed when collected/finished, when it falls below the
//!   level, or when its right edge is more than `game.despawn_margin` px
//!   left of the screen.
//! - Positions are the hitbox in subpixels; the renderer puts the sprite's
//!   origin at the feet (bottom centre of the hitbox).
//!
//! The player interactions (collecting, power-ups, stomping, hurting) live
//! in `game.rs`, because they change the score, lives and player. Enemy
//! behaviour (patterns, projectiles, defeated enemies) is in
//! [`crate::enemies`], the moving platforms (Festungsbahn car, Gondel) in
//! [`crate::platform`].

use crate::fx::Fx;
use crate::hash::Fnv;
use crate::level::{EntitySpawn, SpawnKind, TILE_PX};
use crate::physics::{Aabb, TILE, TileMap, YHit};
use crate::tuning::Tuning;

/// What a popup shows. Score popups are numbers; the others are text keys
/// (see [`Popup::text_key`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Popup {
    /// Points, e.g. `1000`.
    Score(u32),
    /// `popup.extra_life` („+1 Leben").
    ExtraLife,
    /// `popup.checkpoint` („Zwischenstopp").
    Checkpoint,
    /// `popup.wurstkranz` („Wurstkranz!"), the top Maibaum zone.
    Wurstkranz,
    /// `popup.hoppala` („Hoppala!"), an E-Biker crashed into a wall.
    Hoppala,
}

impl Popup {
    /// The text key for text popups; `None` for score popups.
    pub const fn text_key(self) -> Option<&'static str> {
        match self {
            Popup::Score(_) => None,
            Popup::ExtraLife => Some("popup.extra_life"),
            Popup::Checkpoint => Some("popup.checkpoint"),
            Popup::Wurstkranz => Some("popup.wurstkranz"),
            Popup::Hoppala => Some("popup.hoppala"),
        }
    }
}

/// The enemy types of Stages 1–3.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnemyKind {
    /// `selfie_tourist`.
    SelfieTourist,
    /// `taube` (flying).
    Taube,
    /// `taube_sitting` (sits until the player comes close).
    TaubeSitting,
    /// `mozartkugel_verkaeufer`.
    MozartkugelVerkaeufer,
    /// `fiaker`: not stompable, landing on it only bounces.
    Fiaker,
    /// `nordic_walker`: walks, turns at walls, enemies and ledges.
    NordicWalker,
    /// `blasmusiker`: stands, blows note trains ([`EntityKind::Note`]).
    Blasmusiker,
    /// `e_biker`: parked, then rides left along the ground.
    EBiker,
    /// `bienenschwarm`: dances a figure-eight; hurts on any contact.
    Bienenschwarm,
}

impl EnemyKind {
    /// True if a stomp defeats it (the Fiaker only bounces the player, a
    /// swarm hurts from above too).
    pub const fn stompable(self) -> bool {
        !matches!(self, EnemyKind::Fiaker | EnemyKind::Bienenschwarm)
    }

    /// True if touching it from above hurts like any other contact (no
    /// stomp, no bounce): the Bienenschwarm.
    pub const fn hurts_from_above(self) -> bool {
        matches!(self, EnemyKind::Bienenschwarm)
    }

    /// Index into `score.stomp_chain` when it is the first stomp of a
    /// chain: 1 (200 points) for the enemies worth 200 (Verkäufer,
    /// Blasmusiker, E-Biker; D-046), else 0.
    pub const fn chain_start(self) -> usize {
        match self {
            EnemyKind::MozartkugelVerkaeufer | EnemyKind::Blasmusiker | EnemyKind::EBiker => 1,
            _ => 0,
        }
    }

    /// Catalog sprite name.
    pub const fn sprite(self) -> &'static str {
        match self {
            EnemyKind::SelfieTourist => "selfie_tourist",
            EnemyKind::Taube | EnemyKind::TaubeSitting => "taube",
            EnemyKind::MozartkugelVerkaeufer => "mozartkugel_verkaeufer",
            EnemyKind::Fiaker => "fiaker",
            EnemyKind::NordicWalker => "nordic_walker",
            EnemyKind::Blasmusiker => "blasmusiker",
            EnemyKind::EBiker => "e_biker",
            EnemyKind::Bienenschwarm => "bienenschwarm",
        }
    }

    const fn id(self) -> u8 {
        match self {
            EnemyKind::SelfieTourist => 0,
            EnemyKind::Taube => 1,
            EnemyKind::TaubeSitting => 2,
            EnemyKind::MozartkugelVerkaeufer => 3,
            EnemyKind::Fiaker => 4,
            EnemyKind::NordicWalker => 5,
            EnemyKind::Blasmusiker => 6,
            EnemyKind::EBiker => 7,
            EnemyKind::Bienenschwarm => 8,
        }
    }
}

/// Every kind of entity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntityKind {
    /// A static Oblate, collected on touch.
    Oblate,
    /// An Oblate popping out of a box (already collected; flies up and
    /// disappears, then shows its score popup).
    PoppedOblate,
    /// The Mozartkugel power-up: rises out of its box, then rolls.
    Mozartkugel,
    /// The Zaunerstollen 1-up: like the Mozartkugel, a bit faster.
    Zaunerstollen,
    /// A brick piece (no collision).
    Debris,
    /// A popup floating up (score or text).
    Popup(Popup),
    /// A block bumped from below; its hitbox is the tile cell, the renderer
    /// draws that tile raised by [`Entity::bump_offset_px`].
    Bump,
    /// The checkpoint (Haltestelle) sign; whether it is reached is
    /// `Game::checkpoint`.
    Checkpoint,
    /// A decorative sign; text and style come from its level spawn.
    Sign,
    /// An enemy.
    Enemy(EnemyKind),
    /// A stomped or knocked enemy (no collision): squished in place for
    /// `enemies.squish_frames`, or tumbling down off the screen.
    Defeated(EnemyKind),
    /// A Mozartkugel thrown by the Verkäufer (hurts, can't be stomped).
    Thrown,
    /// The Festungsbahn car (moving one-way platform, [`crate::platform`]).
    Festungsbahn,
    /// A Blasmusik note (flies straight, hurts, can't be stomped).
    Note,
    /// A Zwölferhorn cabin (free-running moving one-way platform,
    /// [`crate::platform`]).
    Gondel,
}

impl EntityKind {
    /// Catalog sprite name, if the kind has a sprite of its own (signs pick
    /// theirs by style, bumps are tiles, popups are text).
    pub const fn sprite(self) -> Option<&'static str> {
        match self {
            EntityKind::Oblate | EntityKind::PoppedOblate => Some("oblate"),
            EntityKind::Mozartkugel => Some("mozartkugel"),
            EntityKind::Zaunerstollen => Some("zaunerstollen"),
            EntityKind::Debris => Some("debris"),
            EntityKind::Checkpoint => Some("checkpoint"),
            EntityKind::Enemy(k) | EntityKind::Defeated(k) => Some(k.sprite()),
            EntityKind::Thrown => Some("mozartkugel_thrown"),
            EntityKind::Festungsbahn => Some("festungsbahn"),
            EntityKind::Note => Some("note"),
            EntityKind::Gondel => Some("gondel"),
            EntityKind::Popup(_) | EntityKind::Bump | EntityKind::Sign => None,
        }
    }

    /// True for moving platforms (Festungsbahn, Gondel): one-way tops that
    /// carry the player and never despawn (D-027).
    pub const fn is_platform(self) -> bool {
        matches!(self, EntityKind::Festungsbahn | EntityKind::Gondel)
    }

    fn hash(self, h: &mut Fnv) {
        match self {
            EntityKind::Oblate => h.u8(0),
            EntityKind::PoppedOblate => h.u8(1),
            EntityKind::Mozartkugel => h.u8(2),
            EntityKind::Zaunerstollen => h.u8(3),
            EntityKind::Debris => h.u8(4),
            EntityKind::Popup(p) => {
                h.u8(5);
                match p {
                    Popup::Score(n) => {
                        h.u8(0);
                        h.u32(n);
                    }
                    Popup::ExtraLife => h.u8(1),
                    Popup::Checkpoint => h.u8(2),
                    Popup::Wurstkranz => h.u8(3),
                    Popup::Hoppala => h.u8(4),
                }
            }
            EntityKind::Bump => h.u8(6),
            EntityKind::Checkpoint => h.u8(7),
            EntityKind::Sign => h.u8(8),
            EntityKind::Enemy(k) => {
                h.u8(9);
                h.u8(k.id());
            }
            EntityKind::Festungsbahn => h.u8(10),
            EntityKind::Defeated(k) => {
                h.u8(11);
                h.u8(k.id());
            }
            EntityKind::Thrown => h.u8(12),
            EntityKind::Note => h.u8(13),
            EntityKind::Gondel => h.u8(14),
        }
    }
}

/// One entity. Read-only for the renderer (`Game::entities`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Entity {
    /// What it is.
    pub kind: EntityKind,
    /// Hitbox left edge.
    pub x: Fx,
    /// Hitbox top edge.
    pub y: Fx,
    /// Hitbox width.
    pub w: Fx,
    /// Hitbox height.
    pub h: Fx,
    /// Horizontal speed per frame (+ = right).
    pub vx: Fx,
    /// Vertical speed per frame (+ = down).
    pub vy: Fx,
    /// Frames since it was spawned or activated (1 on the first update).
    pub timer: u16,
    /// Updating and interacting (see the module docs).
    pub active: bool,
    /// Index into `Level::entities` for level-placed entities.
    pub spawn: Option<u16>,
    /// Marked for removal at the end of the step.
    pub removed: bool,
    /// Hitbox top at the start of the current step (before its own
    /// movement): the stomp rule compares against this middle.
    pub prev_y: Fx,
    /// Per-kind state machine value (e.g. walking / selfie / wind-up);
    /// 0 = the kind's initial state. Free for the enemy task.
    pub state: u8,
    /// Per-kind direction: -1 left, 1 right, 0 none (enemies start
    /// facing left, towards the player).
    pub dir: i8,
    /// Per-kind reference height (e.g. the centre line of a flying Taube's
    /// sine wave); the spawn y by default.
    pub base_y: Fx,
    /// Per-kind reference x (the Festungsbahn's start point); the spawn x
    /// by default.
    pub base_x: Fx,
    /// Per-kind frame counter (e.g. the Selfie-Tourist's phase in its
    /// selfie period, frames into a state); 0 at spawn and activation.
    pub count: u16,
}

impl Entity {
    /// A new active entity with this hitbox, at rest.
    pub fn new(kind: EntityKind, x: Fx, y: Fx, w: Fx, h: Fx) -> Entity {
        Entity {
            kind,
            x,
            y,
            w,
            h,
            vx: Fx::ZERO,
            vy: Fx::ZERO,
            timer: 0,
            active: true,
            spawn: None,
            removed: false,
            prev_y: y,
            state: 0,
            dir: 0,
            base_y: y,
            base_x: x,
            count: 0,
        }
    }

    /// A new active entity of size `[w, h]` px with its feet (bottom
    /// centre) at `(fx, fy)`.
    pub fn at_feet(kind: EntityKind, fx: Fx, fy: Fx, size: [i32; 2]) -> Entity {
        let (w, h) = (Fx::from_px(size[0]), Fx::from_px(size[1]));
        Entity::new(kind, fx - w.div_int(2), fy - h, w, h)
    }

    /// The (inactive) entity for a level spawn, or `None` for spawns that
    /// are not entities (player start, goal). `index` is the spawn's index
    /// in `Level::entities`.
    pub fn from_spawn(s: &EntitySpawn, index: usize, t: &Tuning) -> Option<Entity> {
        let tile = TILE;
        let cell_x = tile.mul_int(s.col);
        let feet_x = cell_x + tile.div_int(2);
        let feet_y = tile.mul_int(s.row + 1);
        let e = &t.enemies;
        let mut entity = match s.kind {
            SpawnKind::PlayerStart | SpawnKind::Goal | SpawnKind::Prop => return None,
            SpawnKind::Oblate => {
                // Centred in its cell.
                let [w, h] = t.items.oblate_hitbox;
                let (w, h) = (Fx::from_px(w), Fx::from_px(h));
                Entity::new(
                    EntityKind::Oblate,
                    feet_x - w.div_int(2),
                    tile.mul_int(s.row) + (tile - h).div_int(2),
                    w,
                    h,
                )
            }
            SpawnKind::Checkpoint => {
                Entity::at_feet(EntityKind::Checkpoint, feet_x, feet_y, [16, 32])
            }
            SpawnKind::Sign => Entity::at_feet(EntityKind::Sign, feet_x, feet_y, [16, 16]),
            SpawnKind::SelfieTourist => Entity::at_feet(
                EntityKind::Enemy(EnemyKind::SelfieTourist),
                feet_x,
                feet_y,
                e.selfie_tourist_hitbox,
            ),
            SpawnKind::Taube => Entity::at_feet(
                EntityKind::Enemy(EnemyKind::Taube),
                feet_x,
                feet_y,
                e.taube_hitbox,
            ),
            SpawnKind::TaubeSitting => Entity::at_feet(
                EntityKind::Enemy(EnemyKind::TaubeSitting),
                feet_x,
                feet_y,
                e.taube_hitbox,
            ),
            SpawnKind::MozartkugelVerkaeufer => Entity::at_feet(
                EntityKind::Enemy(EnemyKind::MozartkugelVerkaeufer),
                feet_x,
                feet_y,
                e.mozartkugel_verkaeufer_hitbox,
            ),
            SpawnKind::Fiaker => Entity::at_feet(
                EntityKind::Enemy(EnemyKind::Fiaker),
                feet_x,
                feet_y,
                e.fiaker_hitbox,
            ),
            SpawnKind::Festungsbahn => {
                // Left-aligned in its cell: the car is 3 tiles wide; it
                // waits at its lower end or first travels down to it.
                let mut car = Entity::at_feet(
                    EntityKind::Festungsbahn,
                    feet_x,
                    feet_y,
                    t.platforms.festungsbahn_size,
                );
                car.x = cell_x;
                car.base_x = cell_x;
                car.state = crate::platform::initial_state(s);
                car
            }
            SpawnKind::Gondel => {
                // Left-aligned in its cell like the Festungsbahn, then at
                // `phase` frames into its cycle.
                let mut cab =
                    Entity::at_feet(EntityKind::Gondel, feet_x, feet_y, t.platforms.gondel_size);
                cab.x = cell_x;
                cab.base_x = cell_x;
                cab.count = s.phase % crate::platform::gondel_period(s, t);
                crate::platform::place_gondel(&mut cab, s, t);
                cab
            }
            SpawnKind::NordicWalker => Entity::at_feet(
                EntityKind::Enemy(EnemyKind::NordicWalker),
                feet_x,
                feet_y,
                e.nordic_walker_hitbox,
            ),
            SpawnKind::Blasmusiker => Entity::at_feet(
                EntityKind::Enemy(EnemyKind::Blasmusiker),
                feet_x,
                feet_y,
                e.blasmusiker_hitbox,
            ),
            SpawnKind::EBiker => Entity::at_feet(
                EntityKind::Enemy(EnemyKind::EBiker),
                feet_x,
                feet_y,
                e.e_biker_hitbox,
            ),
            SpawnKind::Bienenschwarm => {
                // Hitbox centred on the cell centre (the anchor A).
                let [w, h] = e.bienenschwarm_hitbox;
                let centre_y = tile.mul_int(s.row) + tile.div_int(2);
                Entity::at_feet(
                    EntityKind::Enemy(EnemyKind::Bienenschwarm),
                    feet_x,
                    centre_y + Fx::from_px(h).div_int(2),
                    [w, h],
                )
            }
        };
        entity.active = false;
        entity.spawn = Some(index as u16);
        if let EntityKind::Enemy(kind) = entity.kind {
            // A `still` tourist has no direction: it never walks (D-026).
            entity.dir = if s.still { 0 } else { -1 };
            entity.state = crate::enemies::initial_state(kind);
            if kind == EnemyKind::Bienenschwarm {
                // `dir` is its run direction (0..=7), placed at `phase`.
                entity.dir = (s.dir & 7) as i8;
                entity.count = s.phase % crate::enemies::swarm_period(s.len, t);
                crate::enemies::place_swarm(&mut entity, s.len, t);
            }
        }
        Some(entity)
    }

    /// The hitbox.
    pub fn hitbox(&self) -> Aabb {
        Aabb {
            x: self.x,
            y: self.y,
            w: self.w,
            h: self.h,
        }
    }

    /// Exclusive right edge.
    pub fn right(&self) -> Fx {
        self.x + self.w
    }

    /// Feet (bottom centre of the hitbox) in subpixels.
    pub fn feet(&self) -> (Fx, Fx) {
        (self.x + self.w.div_int(2), self.y + self.h)
    }

    /// For [`EntityKind::Popup`]: how far it has floated up, whole pixels.
    pub fn popup_rise_px(&self, t: &Tuning) -> i32 {
        let frames = i32::from(t.score.popup_frames.max(1));
        t.score.popup_rise * i32::from(self.timer).min(frames) / frames
    }

    /// For [`EntityKind::Bump`]: how far the tile is raised, whole pixels
    /// (up to `bump_px` at half time, back to 0 at the end).
    pub fn bump_offset_px(&self, t: &Tuning) -> i32 {
        bump_offset_px(self.timer, t)
    }

    /// For [`EntityKind::Mozartkugel`] / [`EntityKind::Zaunerstollen`]:
    /// still rising out of its box (drawn behind the tiles).
    pub fn rising(&self, t: &Tuning) -> bool {
        matches!(
            self.kind,
            EntityKind::Mozartkugel | EntityKind::Zaunerstollen
        ) && self.timer <= t.items.rise_frames
    }

    /// Grid cell of a [`EntityKind::Bump`].
    pub fn cell(&self) -> (i32, i32) {
        (
            self.x.to_px().div_euclid(TILE_PX),
            self.y.to_px().div_euclid(TILE_PX),
        )
    }

    /// Advances an active entity by one frame (its own movement only).
    /// Returns an entity to spawn (a popped Oblate's score popup, a thrown
    /// Mozartkugel).
    pub(crate) fn update(&mut self, ctx: &UpdateCtx) -> Option<Entity> {
        let (map, t) = (ctx.map, ctx.tuning);
        self.timer = self.timer.saturating_add(1);
        match self.kind {
            EntityKind::Oblate | EntityKind::Checkpoint | EntityKind::Sign => None,
            EntityKind::Enemy(kind) => crate::enemies::update_enemy(self, kind, ctx),
            EntityKind::Defeated(_) => {
                crate::enemies::update_defeated(self, t);
                None
            }
            EntityKind::Thrown => {
                crate::enemies::update_thrown(self, map, t);
                None
            }
            EntityKind::Festungsbahn => {
                crate::platform::update_car(self, ctx);
                None
            }
            EntityKind::Gondel => {
                crate::platform::update_gondel(self, ctx);
                None
            }
            EntityKind::Note => {
                crate::enemies::update_note(self, map, t);
                None
            }
            EntityKind::PoppedOblate => self.update_popped_oblate(t),
            EntityKind::Mozartkugel => {
                self.update_power_up(map, t, t.items.mozartkugel_speed);
                None
            }
            EntityKind::Zaunerstollen => {
                self.update_power_up(map, t, t.items.zaunerstollen_speed);
                None
            }
            EntityKind::Debris => {
                self.vy = (self.vy + t.player.gravity).min(t.player.max_fall_speed);
                self.x += self.vx;
                self.y += self.vy;
                None
            }
            EntityKind::Popup(_) => {
                if self.timer >= t.score.popup_frames {
                    self.removed = true;
                }
                None
            }
            EntityKind::Bump => {
                if self.timer >= t.items.bump_frames {
                    self.removed = true;
                }
                None
            }
        }
    }

    fn update_popped_oblate(&mut self, t: &Tuning) -> Option<Entity> {
        self.vy += t.player.gravity;
        self.y += self.vy;
        if self.timer < t.items.pop_frames {
            return None;
        }
        self.removed = true;
        let (fx, _) = self.feet();
        Some(popup(Popup::Score(t.score.oblate), fx, self.y))
    }

    /// Rise out of the box (no collision), then roll: gravity, turn at
    /// walls, fall off ledges. Lost in hazards (water).
    fn update_power_up(&mut self, map: &TileMap, t: &Tuning, speed: Fx) {
        if self.timer <= t.items.rise_frames {
            self.y -= t.items.rise_speed;
            if self.timer == t.items.rise_frames {
                self.vx = speed;
            }
            return;
        }
        self.vy = (self.vy + t.player.gravity).min(t.player.max_fall_speed);
        let mut b = self.hitbox();
        if map.move_x(&mut b, self.vx) {
            self.vx = -self.vx;
        }
        if map.move_y(&mut b, self.vy, false) != YHit::None {
            self.vy = Fx::ZERO;
        }
        self.x = b.x;
        self.y = b.y;
        if map.touches_lethal(&b) {
            self.removed = true;
        }
    }

    pub(crate) fn hash(&self, h: &mut Fnv) {
        self.kind.hash(h);
        for v in [self.x, self.y, self.w, self.h, self.vx, self.vy] {
            h.i32(v.raw());
        }
        h.u16(self.timer);
        h.bool(self.active);
        h.u16(self.spawn.unwrap_or(u16::MAX));
        h.bool(self.removed);
        h.i32(self.prev_y.raw());
        h.u8(self.state);
        h.u8(self.dir as u8);
        h.i32(self.base_y.raw());
        h.i32(self.base_x.raw());
        h.u16(self.count);
    }
}

/// What an entity update may look at besides the entity itself.
pub(crate) struct UpdateCtx<'a> {
    /// The current tile grid.
    pub map: &'a TileMap,
    /// The tuning.
    pub tuning: &'a Tuning,
    /// The level (spawn data such as a Festungsbahn's end point).
    pub level: &'a crate::Level,
    /// The player's hitbox centre x.
    pub player_cx: Fx,
    /// The player's hitbox while standing on the ground (a tile or a car),
    /// `None` in the air (a Festungsbahn waits for its rider).
    pub player_standing: Option<crate::physics::Aabb>,
    /// The left screen edge (camera x).
    pub screen_left: Fx,
}

/// A popup centred on `x` with its top at `y`.
pub fn popup(p: Popup, x: Fx, y: Fx) -> Entity {
    let w = Fx::from_px(16);
    Entity::new(EntityKind::Popup(p), x - w.div_int(2), y, w, Fx::from_px(8))
}

/// How far a bumped tile is raised `timer` frames into its bump, whole pixels.
pub fn bump_offset_px(timer: u16, t: &Tuning) -> i32 {
    let frames = i32::from(t.items.bump_frames.max(2));
    let tt = i32::from(timer).min(frames);
    t.items.bump_px * tt.min(frames - tt) / (frames / 2)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tuning() -> Tuning {
        Tuning::parse(include_str!("../../../content/tuning.toml")).unwrap()
    }

    #[test]
    fn bump_goes_up_and_down() {
        let t = tuning();
        let offsets: Vec<i32> = (0..=t.items.bump_frames)
            .map(|f| bump_offset_px(f, &t))
            .collect();
        assert_eq!(offsets, vec![0, 1, 2, 3, 4, 3, 2, 1, 0]);
    }

    #[test]
    fn popup_floats_16_px_over_30_frames() {
        let t = tuning();
        let mut p = popup(Popup::Score(200), Fx::from_px(100), Fx::from_px(50));
        assert_eq!(p.popup_rise_px(&t), 0);
        let level = crate::Level::parse(
            "[meta]\nid = \"t\"\ntime_limit = 1\n[legend]\n\"P\" = { entity = \"player_start\" }\n[map]\ngrid = '''\nP.\n'''\n",
        )
        .unwrap();
        let map = TileMap::from_level(&level);
        let ctx = UpdateCtx {
            map: &map,
            tuning: &t,
            level: &level,
            player_cx: Fx::ZERO,
            player_standing: None,
            screen_left: Fx::ZERO,
        };
        for _ in 0..15 {
            assert!(p.update(&ctx).is_none());
        }
        assert_eq!(p.popup_rise_px(&t), 8);
        for _ in 15..29 {
            p.update(&ctx);
        }
        assert!(!p.removed);
        p.update(&ctx);
        assert_eq!(p.popup_rise_px(&t), 16);
        assert!(p.removed, "gone after 30 frames");
        assert_eq!(p.x, Fx::from_px(92), "centred");
    }

    #[test]
    fn spawns_are_inactive_and_placed_at_their_cell() {
        let t = tuning();
        let s = |kind| EntitySpawn {
            kind,
            col: 2,
            row: 3,
            text: None,
            style: Default::default(),
            to: None,
            still: false,
            sprite: None,
            layer: Default::default(),
            dx: 0,
            phase: 0,
            dir: 0,
            len: crate::level::DEFAULT_SWARM_LEN,
        };
        assert!(Entity::from_spawn(&s(SpawnKind::PlayerStart), 0, &t).is_none());
        assert!(Entity::from_spawn(&s(SpawnKind::Prop), 0, &t).is_none());
        assert!(Entity::from_spawn(&s(SpawnKind::Goal), 0, &t).is_none());
        let o = Entity::from_spawn(&s(SpawnKind::Oblate), 5, &t).unwrap();
        assert!(!o.active);
        assert_eq!(o.spawn, Some(5));
        assert_eq!((o.x, o.y), (Fx::from_px(35), Fx::from_px(51)), "centred");
        let tourist = Entity::from_spawn(&s(SpawnKind::SelfieTourist), 0, &t).unwrap();
        assert_eq!(tourist.feet(), (Fx::from_px(40), Fx::from_px(64)));
        assert_eq!(tourist.kind, EntityKind::Enemy(EnemyKind::SelfieTourist));
        for kind in SpawnKind::ALL {
            if let Some(e) = Entity::from_spawn(&s(kind), 0, &t) {
                // Oblaten are centred in the cell; so is a swarm's hitbox
                // (its anchor is the cell centre).
                let above = match kind {
                    SpawnKind::Oblate => 3,
                    SpawnKind::Bienenschwarm => 1,
                    _ => 0,
                };
                assert_eq!(e.feet().1, Fx::from_px(64 - above), "{kind:?}");
            }
        }
    }

    #[test]
    fn enemy_rules() {
        assert!(EnemyKind::SelfieTourist.stompable());
        assert!(!EnemyKind::Fiaker.stompable());
        assert!(!EnemyKind::Bienenschwarm.stompable());
        assert!(EnemyKind::Bienenschwarm.hurts_from_above());
        assert!(!EnemyKind::Fiaker.hurts_from_above());
        for k in [
            EnemyKind::NordicWalker,
            EnemyKind::Blasmusiker,
            EnemyKind::EBiker,
        ] {
            assert!(k.stompable(), "{k:?}");
        }
        assert_eq!(EnemyKind::SelfieTourist.chain_start(), 0);
        assert_eq!(EnemyKind::NordicWalker.chain_start(), 0);
        assert_eq!(EnemyKind::MozartkugelVerkaeufer.chain_start(), 1);
        assert_eq!(EnemyKind::Blasmusiker.chain_start(), 1);
        assert_eq!(EnemyKind::EBiker.chain_start(), 1);
        assert_eq!(Popup::Hoppala.text_key(), Some("popup.hoppala"));
        assert!(EntityKind::Gondel.is_platform() && EntityKind::Festungsbahn.is_platform());
        assert!(!EntityKind::Note.is_platform());
        assert_eq!(EnemyKind::TaubeSitting.sprite(), "taube");
        assert_eq!(Popup::Score(5).text_key(), None);
        assert_eq!(Popup::ExtraLife.text_key(), Some("popup.extra_life"));
    }
}
