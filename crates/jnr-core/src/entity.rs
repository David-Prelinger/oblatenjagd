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
    /// `popup.hackl` („Wirf, Resi, wirf!"), picking up the Hackl.
    Hackl,
    /// `popup.ballast` („Ballast weg!"), a big Resi shed to Normal by the
    /// Durchkriechstein.
    Ballast,
    /// `popup.bravo` („Bravo!"), a defeated Operettensänger (D-068).
    Bravo,
    /// `popup.beleidigt` („Beleidigt!"), a defeated Schwan (D-078).
    Beleidigt,
    /// `popup.abtreten` („Abtreten!"), a defeated Offizier (D-096).
    Abtreten,
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
            Popup::Hackl => Some("popup.hackl"),
            Popup::Ballast => Some("popup.ballast"),
            Popup::Bravo => Some("popup.bravo"),
            Popup::Beleidigt => Some("popup.beleidigt"),
            Popup::Abtreten => Some("popup.abtreten"),
        }
    }
}

/// The enemy types of Stages 1–8.
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
    /// `pilger`: walks towards the player, off ledges (`lead` is a look).
    Pilger,
    /// `gams`: stands, crouches, hops; never leaves its ledge.
    Gams,
    /// `operettensaenger`: stands, sings a vertical high-C column
    /// ([`crate::enemies::high_c_column`]) that hurts on any contact.
    Operettensaenger,
    /// `schneekugel`: a snow globe from a Souvenirstand, rolls left and
    /// shatters on walls.
    Schneekugel,
    /// `reisebus`: parked, then drives left (optional photo stop); not
    /// stompable, its roof only bounces (D-070).
    Reisebus,
    /// `schwan`: floats on the surface, charges a close player from its
    /// home point and returns (D-078).
    Schwan,
    /// `gelse`: hovers until it has been on screen a while, then homes in
    /// on the player for a limited time; never enters water (D-079).
    Gelse,
    /// `regattaboot`: the Reisebus's machine on the water: parked, then
    /// races left; not stompable, its masthead only bounces (D-080).
    Regattaboot,
    /// `offizier`: marches, halts and presents sabres on the stage clock;
    /// the sabre zone ([`crate::enemies::sabre_zone`]) hurts (D-096).
    Offizier,
}

impl EnemyKind {
    /// True if a stomp defeats it (the Fiaker only bounces the player, a
    /// swarm hurts from above too).
    pub const fn stompable(self) -> bool {
        !matches!(
            self,
            EnemyKind::Fiaker
                | EnemyKind::Bienenschwarm
                | EnemyKind::Reisebus
                | EnemyKind::Regattaboot
        )
    }

    /// True if touching it from above hurts like any other contact (no
    /// stomp, no bounce): the Bienenschwarm.
    pub const fn hurts_from_above(self) -> bool {
        matches!(self, EnemyKind::Bienenschwarm)
    }

    /// True for the kinds that walk, stand or roll on the ground and are
    /// lost in swim water like in water (D-076): in lethal water they sink
    /// out of the level, swim water has a lake bed, so they are removed as
    /// soon as they touch it. Fliers, the kinds that ignore tiles (the
    /// Fiaker's and the Reisebus' charges) and the lake's kinds (Schwan,
    /// Gelse, Regattaboot) are not.
    pub const fn sinks_in_swim_water(self) -> bool {
        match self {
            EnemyKind::SelfieTourist
            | EnemyKind::MozartkugelVerkaeufer
            | EnemyKind::NordicWalker
            | EnemyKind::Blasmusiker
            | EnemyKind::EBiker
            | EnemyKind::Pilger
            | EnemyKind::Gams
            | EnemyKind::Operettensaenger
            | EnemyKind::Schneekugel
            // D-097: an officer sinks like a walker.
            | EnemyKind::Offizier => true,
            EnemyKind::Taube
            | EnemyKind::TaubeSitting
            | EnemyKind::Fiaker
            | EnemyKind::Bienenschwarm
            | EnemyKind::Reisebus
            // The lake's own kinds (game-design "Stage 6"): the Schwan
            // floats on the water, the Gelse flies over it (never entering
            // it), the Regattaboot sails in the surface row ignoring tiles.
            | EnemyKind::Schwan
            | EnemyKind::Gelse
            | EnemyKind::Regattaboot => false,
        }
    }

    /// Index into `score.stomp_chain` when it is the first stomp of a
    /// chain: 1 (200 points) for the enemies worth 200 (Verkäufer,
    /// Blasmusiker, E-Biker, Gams, Operettensänger, Schwan, Offizier; D-046,
    /// D-058, D-068, D-078, D-096),
    /// else 0. Also the index of
    /// its base value (the points outside a chain, e.g. for a thrown Hackl).
    pub const fn chain_start(self) -> usize {
        match self {
            EnemyKind::MozartkugelVerkaeufer
            | EnemyKind::Blasmusiker
            | EnemyKind::EBiker
            | EnemyKind::Gams
            | EnemyKind::Operettensaenger
            | EnemyKind::Schwan
            | EnemyKind::Offizier => 1,
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
            EnemyKind::Pilger => "pilger",
            EnemyKind::Gams => "gams",
            EnemyKind::Operettensaenger => "operettensaenger",
            EnemyKind::Schneekugel => "schneekugel",
            EnemyKind::Reisebus => "reisebus",
            EnemyKind::Schwan => "schwan",
            EnemyKind::Gelse => "gelse",
            EnemyKind::Regattaboot => "regattaboot",
            EnemyKind::Offizier => "offizier",
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
            EnemyKind::Pilger => 9,
            EnemyKind::Gams => 10,
            EnemyKind::Operettensaenger => 11,
            EnemyKind::Schneekugel => 12,
            EnemyKind::Reisebus => 13,
            EnemyKind::Schwan => 14,
            EnemyKind::Gelse => 15,
            EnemyKind::Regattaboot => 16,
            // M6: 21–22 are reserved for the Offizier (E39).
            EnemyKind::Offizier => 21,
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
    /// A Steinschlag rock spawner: the resting rock in its niche (no
    /// collision; its hitbox is the resting rock's), see
    /// [`crate::enemies::update_steinschlag`].
    Steinschlag,
    /// A falling rock (hurts on any contact, can't be stomped; shatters on
    /// solid and one-way tiles).
    Stein,
    /// A piece of a shattered rock (no collision).
    SteinDebris,
    /// St. Wolfgang's Hackl power-up: rises out of its box, then stays.
    Hackl,
    /// A Hackl thrown by the player ([`crate::hackl`]): flies in a fixed
    /// arc, hits one target, sticks in solid tiles.
    ThrownHackl,
    /// A Durchkriechstein's tunnel (hitbox = the tunnel cells; not drawn,
    /// the boulder is a prop): `state` is [`crate::crawl::stone::UNUSED`]
    /// until crawled through ([`crate::crawl::stone::USED`]).
    Durchkriechstein,
    /// A Souvenirstand: the snow-globe spawner in the air cell in front of
    /// its counter (no collision; its hitbox is that cell), see
    /// [`crate::enemies::update_souvenirstand`].
    Souvenirstand,
    /// A shard of a shattered snow globe (no collision).
    SchneekugelScherben,
    /// The Schafbergbahn steam train: the Festungsbahn's waiting state
    /// machine with its own body and timing ([`crate::platform`], D-067).
    Schafbergbahn,
    /// A Traundl: the Gondel's free-running ping-pong on the lake with its
    /// own hull and timing ([`crate::platform`], D-077).
    Traundl,
    /// The Dampfer paddle steamer: the Festungsbahn's waiting machine, one
    /// way (moored for good at the far end), with a paddle-wheel hurt zone
    /// while it travels ([`crate::platform::paddle_wheel`], D-077).
    Dampfer,
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
            EntityKind::Steinschlag | EntityKind::Stein => Some("stein"),
            EntityKind::SteinDebris => Some("stein_debris"),
            EntityKind::Hackl => Some("hackl"),
            EntityKind::ThrownHackl => Some("hackl_thrown"),
            EntityKind::Souvenirstand => Some("souvenirstand"),
            EntityKind::SchneekugelScherben => Some("schneekugel_scherben"),
            EntityKind::Schafbergbahn => Some("schafbergbahn"),
            EntityKind::Traundl => Some("traundl"),
            EntityKind::Dampfer => Some("dampfer"),
            EntityKind::Popup(_)
            | EntityKind::Bump
            | EntityKind::Sign
            | EntityKind::Durchkriechstein => None,
        }
    }

    /// True for moving platforms (Festungsbahn, Gondel, Schafbergbahn,
    /// Traundl, Dampfer): one-way tops that carry the player and never
    /// despawn (D-027).
    pub const fn is_platform(self) -> bool {
        matches!(
            self,
            EntityKind::Festungsbahn
                | EntityKind::Gondel
                | EntityKind::Schafbergbahn
                | EntityKind::Traundl
                | EntityKind::Dampfer
        )
    }

    /// The kind's tag in `state_hash` (unique per variant: the test
    /// `hash_tags_are_distinct` guards it; enemies and popups add their own
    /// sub-tag).
    pub(crate) const fn hash_tag(self) -> u8 {
        match self {
            EntityKind::Oblate => 0,
            EntityKind::PoppedOblate => 1,
            EntityKind::Mozartkugel => 2,
            EntityKind::Zaunerstollen => 3,
            EntityKind::Debris => 4,
            EntityKind::Popup(_) => 5,
            EntityKind::Bump => 6,
            EntityKind::Checkpoint => 7,
            EntityKind::Sign => 8,
            EntityKind::Enemy(_) => 9,
            EntityKind::Festungsbahn => 10,
            EntityKind::Defeated(_) => 11,
            EntityKind::Thrown => 12,
            EntityKind::Note => 13,
            EntityKind::Gondel => 14,
            EntityKind::Steinschlag => 15,
            EntityKind::Stein => 16,
            EntityKind::SteinDebris => 17,
            EntityKind::Hackl => 18,
            EntityKind::ThrownHackl => 19,
            EntityKind::Durchkriechstein => 20,
            EntityKind::Souvenirstand => 21,
            EntityKind::SchneekugelScherben => 22,
            EntityKind::Schafbergbahn => 23,
            EntityKind::Traundl => 24,
            EntityKind::Dampfer => 25,
        }
    }

    fn hash(self, h: &mut Fnv) {
        h.u8(self.hash_tag());
        match self {
            EntityKind::Popup(p) => match p {
                Popup::Score(n) => {
                    h.u8(0);
                    h.u32(n);
                }
                Popup::ExtraLife => h.u8(1),
                Popup::Checkpoint => h.u8(2),
                Popup::Wurstkranz => h.u8(3),
                Popup::Hoppala => h.u8(4),
                Popup::Hackl => h.u8(5),
                Popup::Ballast => h.u8(6),
                Popup::Bravo => h.u8(7),
                Popup::Beleidigt => h.u8(8),
                Popup::Abtreten => h.u8(10),
            },
            EntityKind::Enemy(k) | EntityKind::Defeated(k) => h.u8(k.id()),
            _ => {}
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
            // Feet on its cell bottom, moved `dy` px (level field, e.g. -8
            // to sit on a perch between two cell bottoms).
            SpawnKind::TaubeSitting => Entity::at_feet(
                EntityKind::Enemy(EnemyKind::TaubeSitting),
                feet_x,
                feet_y + Fx::from_px(s.dy),
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
            SpawnKind::Schafbergbahn => {
                // Left-aligned in its cell like the Festungsbahn (4 tiles
                // wide), waiting at its lower end or travelling down to it.
                let mut train = Entity::at_feet(
                    EntityKind::Schafbergbahn,
                    feet_x,
                    feet_y,
                    t.platforms.schafbergbahn_size,
                );
                train.x = cell_x;
                train.base_x = cell_x;
                train.state = crate::platform::initial_state(s);
                train
            }
            SpawnKind::Dampfer => {
                // Left-aligned in its cell like the Festungsbahn (10 tiles
                // wide), bottom on the cell's bottom (the water line),
                // waiting for its rider.
                let mut ship = Entity::at_feet(
                    EntityKind::Dampfer,
                    feet_x,
                    feet_y,
                    t.platforms.dampfer_size,
                );
                ship.x = cell_x;
                ship.base_x = cell_x;
                ship.state = crate::platform::initial_state(s);
                ship
            }
            SpawnKind::Traundl => {
                // Left-aligned in its cell like the Gondel, bottom on the
                // cell's bottom (the water line), at `phase` frames into
                // its cycle.
                let mut boat = Entity::at_feet(
                    EntityKind::Traundl,
                    feet_x,
                    feet_y,
                    t.platforms.traundl_size,
                );
                boat.x = cell_x;
                boat.base_x = cell_x;
                boat.count = s.phase % crate::platform::free_period(EntityKind::Traundl, s, t);
                crate::platform::place_free(&mut boat, s, t);
                boat
            }
            SpawnKind::Gondel => {
                // Left-aligned in its cell like the Festungsbahn, then at
                // `phase` frames into its cycle.
                let mut cab =
                    Entity::at_feet(EntityKind::Gondel, feet_x, feet_y, t.platforms.gondel_size);
                cab.x = cell_x;
                cab.base_x = cell_x;
                cab.count = s.phase % crate::platform::free_period(EntityKind::Gondel, s, t);
                crate::platform::place_free(&mut cab, s, t);
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
                // Hitbox centred on the anchor A: the cell centre, moved
                // `dy` px (level field, e.g. -8 above the ground).
                let [w, h] = e.bienenschwarm_hitbox;
                let centre_y = tile.mul_int(s.row) + tile.div_int(2) + Fx::from_px(s.dy);
                Entity::at_feet(
                    EntityKind::Enemy(EnemyKind::Bienenschwarm),
                    feet_x,
                    centre_y + Fx::from_px(h).div_int(2),
                    [w, h],
                )
            }
            SpawnKind::Pilger => Entity::at_feet(
                EntityKind::Enemy(EnemyKind::Pilger),
                feet_x,
                feet_y,
                e.pilger_hitbox,
            ),
            SpawnKind::Gams => Entity::at_feet(
                EntityKind::Enemy(EnemyKind::Gams),
                feet_x,
                feet_y,
                e.gams_hitbox,
            ),
            SpawnKind::Steinschlag => {
                crate::enemies::resting_rock(EntityKind::Steinschlag, s.col, s.row, t)
            }
            SpawnKind::Operettensaenger => Entity::at_feet(
                EntityKind::Enemy(EnemyKind::Operettensaenger),
                feet_x,
                feet_y,
                e.operettensaenger_hitbox,
            ),
            // The spawner cell (the globes start at its bottom centre).
            SpawnKind::Souvenirstand => {
                Entity::at_feet(EntityKind::Souvenirstand, feet_x, feet_y, [16, 16])
            }
            // The 4-tile (64 px) bus starts in its cell and covers the 3
            // to its right; the hitbox is centred under the sprite and
            // `base_x` keeps where it was parked (its photo stop, D-070).
            SpawnKind::Reisebus => Entity::at_feet(
                EntityKind::Enemy(EnemyKind::Reisebus),
                cell_x + tile.mul_int(2),
                feet_y,
                e.reisebus_hitbox,
            ),
            // Floats on the surface (D-078): spawned in a surface-row water
            // cell, its hitbox bottom `schwan_float_px` below the water
            // line (the cell's top). `base_x` is its home point.
            SpawnKind::Schwan => Entity::at_feet(
                EntityKind::Enemy(EnemyKind::Schwan),
                feet_x,
                tile.mul_int(s.row) + Fx::from_px(e.schwan_float_px),
                e.schwan_hitbox,
            ),
            // Hitbox centred in its cell (D-079).
            SpawnKind::Gelse => {
                let [_, h] = e.gelse_hitbox;
                let centre_y = tile.mul_int(s.row) + tile.div_int(2);
                Entity::at_feet(
                    EntityKind::Enemy(EnemyKind::Gelse),
                    feet_x,
                    centre_y + Fx::from_px(h).div_int(2),
                    e.gelse_hitbox,
                )
            }
            // The 2-tile (32 px) boat starts in its cell and covers the one
            // to its right, in the surface row (D-080): the hitbox centred
            // under the sprite, its bottom (the keel) `regattaboot_keel_px`
            // below the water line (the cell's top).
            SpawnKind::Regattaboot => Entity::at_feet(
                EntityKind::Enemy(EnemyKind::Regattaboot),
                cell_x + tile,
                tile.mul_int(s.row) + Fx::from_px(e.regattaboot_keel_px),
                e.regattaboot_hitbox,
            ),
            SpawnKind::Offizier => Entity::at_feet(
                EntityKind::Enemy(EnemyKind::Offizier),
                feet_x,
                feet_y,
                e.offizier_hitbox,
            ),
            SpawnKind::Durchkriechstein => {
                // The tunnel: `len` cells from the spawn cell to the right.
                let len = tile.mul_int(i32::from(s.len));
                Entity::new(
                    EntityKind::Durchkriechstein,
                    cell_x,
                    tile.mul_int(s.row),
                    len,
                    tile,
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

    /// For [`EntityKind::Mozartkugel`] / [`EntityKind::Zaunerstollen`] /
    /// [`EntityKind::Hackl`]: still rising out of its box (drawn behind
    /// the tiles).
    pub fn rising(&self, t: &Tuning) -> bool {
        matches!(
            self.kind,
            EntityKind::Mozartkugel | EntityKind::Zaunerstollen | EntityKind::Hackl
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
            EntityKind::Oblate
            | EntityKind::Checkpoint
            | EntityKind::Sign
            | EntityKind::Durchkriechstein => None,
            EntityKind::Hackl => {
                // Rises out of its box, then stays where it is (D-056).
                if self.timer <= t.items.rise_frames {
                    self.y -= t.items.rise_speed;
                }
                None
            }
            EntityKind::ThrownHackl => {
                crate::hackl::update_axe(self, map, t);
                None
            }
            EntityKind::Enemy(kind) => crate::enemies::update_enemy(self, kind, ctx),
            EntityKind::Defeated(_) => {
                crate::enemies::update_defeated(self, t);
                None
            }
            EntityKind::Thrown => {
                crate::enemies::update_thrown(self, map, t);
                None
            }
            EntityKind::Festungsbahn | EntityKind::Schafbergbahn | EntityKind::Dampfer => {
                crate::platform::update_car(self, ctx);
                None
            }
            EntityKind::Gondel | EntityKind::Traundl => {
                crate::platform::update_free(self, ctx);
                None
            }
            EntityKind::Note => {
                crate::enemies::update_note(self, map, t);
                None
            }
            EntityKind::Steinschlag => crate::enemies::update_steinschlag(self, ctx),
            EntityKind::Souvenirstand => crate::enemies::update_souvenirstand(self, ctx),
            EntityKind::Stein => {
                crate::enemies::update_stein(self, map, t);
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
            EntityKind::Debris | EntityKind::SteinDebris | EntityKind::SchneekugelScherben => {
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
    /// walls, fall off ledges. Lost in hazards (water, swim water).
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
        if map.swallows(&b) {
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
    /// The player's hitbox top (a Gelse aims just below it, D-079).
    pub player_top: Fx,
    /// The player's hitbox while standing on the ground (a tile or a car),
    /// `None` in the air (a Festungsbahn waits for its rider).
    pub player_standing: Option<crate::physics::Aabb>,
    /// The left screen edge (camera x).
    pub screen_left: Fx,
    /// The top screen edge (camera y).
    pub screen_top: Fx,
    /// Where every snow globe alive at the start of the update started
    /// (`base_x`, `base_y`): a Souvenirstand counts its own by its spawn
    /// point (D-069: at most `souvenirstand_max_alive`).
    pub globes: &'a [(Fx, Fx)],
    /// The stage clock ([`crate::Game::stage_clock`]) at the start of this
    /// frame: frames of play since the stage started or the last respawn.
    /// An Offizier's drill runs on it (D-096).
    pub stage_clock: u32,
}

impl UpdateCtx<'_> {
    /// True if `b` lies completely inside the screen
    /// ([`crate::Camera::fully_on_screen`]): enemy triggers in an
    /// auto-scroll zone need it, so no threat starts off screen (D-075).
    pub(crate) fn fully_on_screen(&self, b: &Aabb) -> bool {
        let camera = crate::Camera {
            x: self.screen_left.to_px(),
            y: self.screen_top.to_px(),
        };
        camera.fully_on_screen(b)
    }
}

/// A popup centred on `x` with its top at `y`.
pub fn popup(p: Popup, x: Fx, y: Fx) -> Entity {
    let w = Fx::from_px(16);
    Entity::new(EntityKind::Popup(p), x - w.div_int(2), y, w, Fx::from_px(8))
}

/// Vertical pitch of popups spawned together ([`stacked_popups`]), whole
/// pixels: a popup's text is drawn from its top down over a 9 px line with
/// the 1 px outline above and below (10 px), plus 1 px of air.
pub const POPUP_STACK_PX: i32 = 11;

/// Popups that spawn together (e.g. a score and a text), stacked upwards
/// so they never overlap: `popups[0]` lowest, with its drawn bottom (incl.
/// the outline) 1 px above `bottom`, each next one [`POPUP_STACK_PX`]
/// higher. All centred on `x`; they float up together.
pub fn stacked_popups(popups: &[Popup], x: Fx, bottom: Fx) -> Vec<Entity> {
    popups
        .iter()
        .zip(1..)
        .map(|(&p, i)| popup(p, x, bottom - Fx::from_px(POPUP_STACK_PX * i) + Fx::ONE))
        .collect()
}

/// How far a bumped tile is raised `timer` frames into its bump, whole pixels.
pub fn bump_offset_px(timer: u16, t: &Tuning) -> i32 {
    let frames = i32::from(t.items.bump_frames.max(2));
    let tt = i32::from(timer).min(frames);
    t.items.bump_px * tt.min(frames - tt) / (frames / 2)
}

#[cfg(test)]
mod tests {
    /// Every `EntityKind` variant has its own hash tag and every
    /// `EnemyKind` its own id (guards merges of parallel tasks that add
    /// kinds). The exhaustive matches make a new variant a compile error
    /// here until it is listed.
    #[test]
    fn hash_tags_are_distinct() {
        use super::{EnemyKind, EntityKind, Popup};
        const fn listed(k: EntityKind) -> usize {
            match k {
                EntityKind::Oblate => 0,
                EntityKind::PoppedOblate => 1,
                EntityKind::Mozartkugel => 2,
                EntityKind::Zaunerstollen => 3,
                EntityKind::Debris => 4,
                EntityKind::Popup(_) => 5,
                EntityKind::Bump => 6,
                EntityKind::Checkpoint => 7,
                EntityKind::Sign => 8,
                EntityKind::Enemy(_) => 9,
                EntityKind::Festungsbahn => 10,
                EntityKind::Defeated(_) => 11,
                EntityKind::Thrown => 12,
                EntityKind::Note => 13,
                EntityKind::Gondel => 14,
                EntityKind::Steinschlag => 15,
                EntityKind::Stein => 16,
                EntityKind::SteinDebris => 17,
                EntityKind::Hackl => 18,
                EntityKind::ThrownHackl => 19,
                EntityKind::Durchkriechstein => 20,
                EntityKind::Schafbergbahn => 21,
                EntityKind::Souvenirstand => 22,
                EntityKind::SchneekugelScherben => 23,
                EntityKind::Traundl => 24,
                EntityKind::Dampfer => 25,
            }
        }
        let e = EnemyKind::SelfieTourist;
        let kinds = [
            EntityKind::Oblate,
            EntityKind::PoppedOblate,
            EntityKind::Mozartkugel,
            EntityKind::Zaunerstollen,
            EntityKind::Debris,
            EntityKind::Popup(Popup::ExtraLife),
            EntityKind::Bump,
            EntityKind::Checkpoint,
            EntityKind::Sign,
            EntityKind::Enemy(e),
            EntityKind::Festungsbahn,
            EntityKind::Defeated(e),
            EntityKind::Thrown,
            EntityKind::Note,
            EntityKind::Gondel,
            EntityKind::Steinschlag,
            EntityKind::Stein,
            EntityKind::SteinDebris,
            EntityKind::Hackl,
            EntityKind::ThrownHackl,
            EntityKind::Durchkriechstein,
            EntityKind::Schafbergbahn,
            EntityKind::Souvenirstand,
            EntityKind::SchneekugelScherben,
            EntityKind::Traundl,
            EntityKind::Dampfer,
        ];
        for (i, k) in kinds.iter().enumerate() {
            assert_eq!(listed(*k), i, "{k:?} listed in order");
        }
        let mut tags: Vec<u8> = kinds.iter().map(|k| k.hash_tag()).collect();
        tags.sort_unstable();
        tags.dedup();
        assert_eq!(
            tags.len(),
            kinds.len(),
            "an EntityKind hash tag is used twice"
        );
        const fn enemy_listed(k: EnemyKind) -> usize {
            match k {
                EnemyKind::SelfieTourist => 0,
                EnemyKind::Taube => 1,
                EnemyKind::TaubeSitting => 2,
                EnemyKind::MozartkugelVerkaeufer => 3,
                EnemyKind::Fiaker => 4,
                EnemyKind::NordicWalker => 5,
                EnemyKind::Blasmusiker => 6,
                EnemyKind::EBiker => 7,
                EnemyKind::Bienenschwarm => 8,
                EnemyKind::Pilger => 9,
                EnemyKind::Gams => 10,
                EnemyKind::Operettensaenger => 11,
                EnemyKind::Schneekugel => 12,
                EnemyKind::Reisebus => 13,
                EnemyKind::Schwan => 14,
                EnemyKind::Gelse => 15,
                EnemyKind::Regattaboot => 16,
                EnemyKind::Offizier => 17,
            }
        }
        let enemies = [
            EnemyKind::SelfieTourist,
            EnemyKind::Taube,
            EnemyKind::TaubeSitting,
            EnemyKind::MozartkugelVerkaeufer,
            EnemyKind::Fiaker,
            EnemyKind::NordicWalker,
            EnemyKind::Blasmusiker,
            EnemyKind::EBiker,
            EnemyKind::Bienenschwarm,
            EnemyKind::Pilger,
            EnemyKind::Gams,
            EnemyKind::Operettensaenger,
            EnemyKind::Schneekugel,
            EnemyKind::Reisebus,
            EnemyKind::Schwan,
            EnemyKind::Gelse,
            EnemyKind::Regattaboot,
            EnemyKind::Offizier,
        ];
        for (i, k) in enemies.iter().enumerate() {
            assert_eq!(enemy_listed(*k), i, "{k:?} listed in order");
        }
        let mut ids: Vec<u8> = enemies.iter().map(|k| k.id()).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), enemies.len(), "an EnemyKind id is used twice");
    }

    use super::*;

    fn tuning() -> Tuning {
        Tuning::parse(include_str!("../../../content/tuning.toml")).unwrap()
    }

    #[test]
    fn stacked_popups_never_overlap() {
        let bottom = Fx::from_px(100);
        let s = stacked_popups(&[Popup::Score(1000), Popup::Hackl], Fx::from_px(50), bottom);
        assert_eq!(s.len(), 2);
        assert_eq!(s[0].kind, EntityKind::Popup(Popup::Score(1000)));
        // Drawn rows: top − 1 (outline) .. top + 9 (descender + outline).
        let rows = |e: &Entity| (e.y.to_px() - 1, e.y.to_px() + 9);
        let (a, b) = (rows(&s[0]), rows(&s[1]));
        assert_eq!(a.1, 99, "1 px above `bottom`");
        assert!(
            b.1 < a.0,
            "the text sits clear above the score: {a:?} {b:?}"
        );
        assert_eq!(a.0 - b.1, 1, "1 px of air");
        assert!(s.iter().all(|e| e.x + e.w.div_int(2) == Fx::from_px(50)));
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
            player_top: Fx::ZERO,
            player_standing: None,
            screen_left: Fx::ZERO,
            screen_top: Fx::ZERO,
            globes: &[],
            stage_clock: 0,
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
            dy: 0,
            lead: false,
            period: crate::level::DEFAULT_STEINSCHLAG_PERIOD,
            range: crate::level::DEFAULT_STEINSCHLAG_RANGE,
            stop: 0,
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
                    // The resting rock sits 2 px above the niche floor.
                    SpawnKind::Steinschlag => 2,
                    // A swan floats 4 px under its cell's top (the water
                    // line), a boat's keel 12 px; a Gelse is centred.
                    SpawnKind::Schwan => 12,
                    SpawnKind::Regattaboot | SpawnKind::Gelse => 4,
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
        assert!(EntityKind::Schafbergbahn.is_platform());
        assert!(EntityKind::Traundl.is_platform() && EntityKind::Dampfer.is_platform());
        assert!(!EntityKind::Note.is_platform());
        assert_eq!(EnemyKind::TaubeSitting.sprite(), "taube");
        assert_eq!(Popup::Score(5).text_key(), None);
        assert_eq!(Popup::ExtraLife.text_key(), Some("popup.extra_life"));
    }
}
