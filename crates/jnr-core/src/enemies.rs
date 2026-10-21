//! Enemy behaviour (`docs/game-design.md` → "Entities and behaviour",
//! Stage 1–4): per-kind update functions for [`EntityKind::Enemy`], the
//! projectiles (thrown Mozartkugel [`EntityKind::Thrown`], Blasmusik note
//! [`EntityKind::Note`]), the Steinschlag spawner and its falling rocks
//! ([`EntityKind::Steinschlag`], [`EntityKind::Stein`]), the Souvenirstand
//! spawner and its snow globes ([`EntityKind::Souvenirstand`],
//! [`EnemyKind::Schneekugel`]), the Operettensänger's high-C column
//! ([`high_c_column`]), the lake enemies of Stage 6 (Schwan, Gelse,
//! Regattaboot), the Ampel spawner and its cars of Stage 7
//! ([`EntityKind::Ampel`], [`EnemyKind::Auto`]), the Offizier's drill and
//! sabre zone ([`sabre_zone`]) and defeated enemies
//! ([`EntityKind::Defeated`]),
//! plus the rules that need the whole entity list (turning at other
//! enemies, knocking enemies off a bumped block).
//!
//! Everything runs on frame counters from activation (`Entity::timer`,
//! `Entity::count`), never on the RNG (D-017). Numbers are in
//! `content/tuning.toml` `[enemies]`. Distances to the player are
//! horizontal, centre to centre.
//!
//! Per-kind `state` values are the constants in [`state`]; the renderer
//! reads them through [`Entity::pose`].

use crate::entity::{EnemyKind, Entity, EntityKind, Popup, UpdateCtx, popup};
use crate::fx::Fx;
use crate::level::{Level, SpawnKind, TILE_PX, TileKind};
use crate::physics::{Aabb, TILE, TileMap, YHit, tile_of};
use crate::tuning::Tuning;

/// `Entity::state` values per enemy kind.
pub mod state {
    /// Selfie-Tourist: walking.
    pub const WALK: u8 = 0;
    /// Selfie-Tourist: standing still for a selfie.
    pub const SELFIE: u8 = 1;

    /// Taube / sitting Taube: flying (the only state of a `taube`).
    pub const FLY: u8 = 0;
    /// Sitting Taube: waiting for the player.
    pub const SIT: u8 = 1;
    /// Sitting Taube: rising before it flies.
    pub const RISE: u8 = 2;

    /// Verkäufer: standing.
    pub const IDLE: u8 = 0;
    /// Verkäufer: winding up (arm raised).
    pub const WINDUP: u8 = 1;
    /// Verkäufer: just threw.
    pub const THROW: u8 = 2;

    /// Fiaker: parked.
    pub const PARKED: u8 = 0;
    /// Fiaker: telegraphing (horse rears).
    pub const TELEGRAPH: u8 = 1;
    /// Fiaker: charging left.
    pub const CHARGE: u8 = 2;

    /// Nordic-Walker: walking (its only state).
    pub const STRIDE: u8 = 0;

    /// Blasmusiker: standing (also between notes).
    pub const STAND: u8 = 0;
    /// Blasmusiker: inhaling (the telegraph, first frames of each bar).
    pub const INHALE: u8 = 1;
    /// Blasmusiker: just blew a note.
    pub const BLOW: u8 = 2;

    /// E-Biker: parked.
    pub const PARK: u8 = 0;
    /// E-Biker: telegraphing (bell, headlight).
    pub const RING: u8 = 1;
    /// E-Biker: riding left.
    pub const RIDE: u8 = 2;
    /// E-Biker: crashed into a wall (harmless, removed after the pose).
    pub const CRASH: u8 = 3;

    /// Bienenschwarm: on its straight waggle run.
    pub const RUN: u8 = 0;
    /// Bienenschwarm: on a half-circle return to its anchor.
    pub const RETURN: u8 = 1;

    /// Gams: crouching (the telegraph; `vx` holds the planned hop).
    pub const CROUCH: u8 = 1;
    /// Gams: in a hop.
    pub const HOP: u8 = 2;
    // Gams standing is `STAND`, Pilger walking is `WALK`.

    /// Steinschlag: the player was never in range (its counter hasn't
    /// started).
    pub const WAITING: u8 = 0;
    /// Steinschlag: counting (`count` = frame of its period).
    pub const COUNTING: u8 = 1;

    /// Stein: falling.
    pub const FALLING: u8 = 0;
    /// Stein / Schneekugel: shattered (removed; the game spawns its
    /// debris or shards).
    pub const SHATTERED: u8 = 1;

    /// Operettensänger: the high C sounds (the column hurts). Inhaling is
    /// `INHALE`, idle (also before the player first came in range) `STAND`.
    pub const SING: u8 = 2;

    /// Souvenirstand: counting, but this cycle started with
    /// `souvenirstand_max_alive` of its globes alive: no telegraph, no
    /// globe. Waiting is `WAITING`, a normal cycle `COUNTING`.
    pub const SKIPPING: u8 = 2;

    /// Schneekugel: rolling left (its only state until it shatters).
    pub const ROLL: u8 = 0;
    /// Reisebus: driving left (parked is `PARKED`, the first telegraph
    /// `TELEGRAPH`).
    pub const DRIVE: u8 = 2;
    /// Reisebus: halted for the photo stop (`count` = frame of the stop).
    pub const PHOTO_STOP: u8 = 3;
    /// Reisebus: telegraphing after the photo stop (`count` = frame).
    pub const RESTART: u8 = 4;
    // The Regattaboot uses the Reisebus's `PARKED`, `TELEGRAPH`, `DRIVE`.

    /// Schwan: at home, facing the player (telegraph is `TELEGRAPH`, the
    /// charge `CHARGE`, `count` = frame of the state).
    pub const HOME: u8 = 0;
    /// Schwan: paddling back to its home point after a charge.
    pub const PADDLE_BACK: u8 = 3;
    /// Schwan: resting at home after a return (`count` = frame of the rest).
    pub const REST: u8 = 4;

    /// Gelse: hovering in place (`count` = frames fully on screen so far).
    pub const HOVER: u8 = 0;
    /// Gelse: homing in on the player (`count` = frame of the hunt).
    pub const HUNT: u8 = 1;
    /// Gelse: leaving straight up, harmless (`satt`).
    pub const LEAVE: u8 = 2;

    /// Offizier: halted for „Habt Acht!" (the telegraph). Marching is
    /// `WALK`; `count` = the drill frame (D-096).
    pub const ATTENTION: u8 = 1;
    /// Offizier: halted for „Präsentiert!": the sabre zone is up
    /// ([`super::sabre_zone`]).
    pub const PRESENT: u8 = 2;
    // Kurgast (D-095): `WALK` (0) until his first stomp, which scores.
    /// Kurgast: lying in his deck chair (a one-way top platform, harmless;
    /// `count` = frames of the rest, paused while the player stands on it).
    pub const LIE: u8 = 1;
    /// Kurgast: the wake telegraph, still in the chair (`count` = frames of
    /// it, paused like the rest).
    pub const WAKE: u8 = 2;
    /// Kurgast: walking again after a rest; a stomp lays him down again but
    /// scores nothing.
    pub const RESTED: u8 = 3;

    /// Defeated: squished in place.
    pub const SQUISHED: u8 = 0;
    /// Defeated: tumbling down off the screen.
    pub const TUMBLE: u8 = 1;
}

/// What an enemy (or a defeated one, or a projectile) looks like right
/// now; the renderer maps it to the catalog animation
/// (`docs/sprite-catalog.md`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pose {
    /// Selfie-Tourist `walk`.
    Walk,
    /// Selfie-Tourist `selfie`: frame 0, or frame 1 while `flash`.
    Selfie {
        /// The 2-frame flash (frame 1 of `selfie`).
        flash: bool,
    },
    /// Taube `fly`.
    Fly,
    /// Taube `sit`.
    Sit,
    /// Taube `tumble` (stomped or knocked: falling off the screen).
    Tumble,
    /// Verkäufer `idle`, Fiaker `idle`.
    Idle,
    /// Verkäufer `windup`.
    Windup,
    /// Verkäufer `throw`.
    Throw,
    /// Selfie-Tourist / Verkäufer `squished`.
    Squished,
    /// Fiaker `telegraph`.
    Telegraph,
    /// Fiaker `charge`.
    Charge,
    /// Thrown Mozartkugel `spin`.
    Spin,
    /// Blasmusiker `inhale` (the telegraph).
    Inhale,
    /// Blasmusiker `blow`.
    Blow,
    /// E-Biker `ride`.
    Ride,
    /// E-Biker `crash`.
    Crash,
    /// Bienenschwarm `buzz` (returning to its anchor).
    Buzz,
    /// Bienenschwarm `waggle` (on its waggle run, shaking).
    Waggle,
    /// Thrown Hackl `stuck` (in a solid tile; `spin` while flying).
    Stuck,
    /// Gams `crouch` (the telegraph before a hop).
    Crouch,
    /// Gams `hop`.
    Hop,
    /// Stein `rest` (in its niche).
    Rest,
    /// Stein `wobble` (the telegraph; the renderer adds `kiesel` pebbles).
    Wobble,
    /// Stein `fall`.
    Fall,
    /// Operettensänger `sing` (the high C sounds).
    Sing,
    /// Schneekugel `roll`.
    Roll,
    /// Reisebus `drive` (parked: `Idle`, telegraphs: `Telegraph`).
    Drive,
    /// Reisebus `fotostopp`: frame 0, or frame 1 (camera flashes in the
    /// windows) while `flash`.
    Fotostopp {
        /// Frame 1 of `fotostopp`.
        flash: bool,
    },
    /// Gelse `wait` (hovering, the telegraph; hunting is `Fly`).
    Wait,
    /// Gelse `leave` (full, flying away).
    Leave,
    /// Offizier `march`.
    March,
    /// Offizier `habtacht` (halted, the telegraph).
    Habtacht,
    /// Offizier `praesentiert` (halted, sabre raised: the zone hurts).
    Praesentiert,
    /// Kurgast lying in his deck chair (`liegestuhl` `idle`, or the
    /// `kurgast` sprite's `liegen`).
    Liegen,
    /// Kurgast's wake telegraph in the chair (`wake`).
    Wake,
}

impl Entity {
    /// The pose of an enemy, defeated enemy or projectile (thrown
    /// Mozartkugel, note, thrown Hackl); `None` for every other kind.
    pub fn pose(&self, t: &Tuning) -> Option<Pose> {
        let e = &t.enemies;
        Some(match self.kind {
            EntityKind::Enemy(EnemyKind::SelfieTourist) => match self.state {
                state::SELFIE => {
                    let k = self.count;
                    Pose::Selfie {
                        flash: k >= e.tourist_flash_at
                            && k < e.tourist_flash_at + e.tourist_flash_frames,
                    }
                }
                _ => Pose::Walk,
            },
            EntityKind::Enemy(EnemyKind::Taube | EnemyKind::TaubeSitting) => match self.state {
                state::SIT => Pose::Sit,
                _ => Pose::Fly,
            },
            EntityKind::Enemy(EnemyKind::MozartkugelVerkaeufer) => match self.state {
                state::WINDUP => Pose::Windup,
                state::THROW => Pose::Throw,
                _ => Pose::Idle,
            },
            EntityKind::Enemy(EnemyKind::Fiaker) => match self.state {
                state::TELEGRAPH => Pose::Telegraph,
                state::CHARGE => Pose::Charge,
                _ => Pose::Idle,
            },
            EntityKind::Enemy(EnemyKind::NordicWalker) => Pose::Walk,
            EntityKind::Enemy(EnemyKind::Blasmusiker) => match self.state {
                state::INHALE => Pose::Inhale,
                state::BLOW => Pose::Blow,
                _ => Pose::Idle,
            },
            EntityKind::Enemy(EnemyKind::EBiker) => match self.state {
                state::RING => Pose::Telegraph,
                state::RIDE => Pose::Ride,
                state::CRASH => Pose::Crash,
                _ => Pose::Idle,
            },
            EntityKind::Enemy(EnemyKind::Bienenschwarm) => match self.state {
                state::RETURN => Pose::Buzz,
                _ => Pose::Waggle,
            },
            EntityKind::Enemy(EnemyKind::Pilger) => Pose::Walk,
            EntityKind::Enemy(EnemyKind::Operettensaenger) => match self.state {
                state::INHALE => Pose::Inhale,
                state::SING => Pose::Sing,
                _ => Pose::Idle,
            },
            EntityKind::Enemy(EnemyKind::Schneekugel) => Pose::Roll,
            // The vendor holds up a globe in a normal cycle's telegraph.
            EntityKind::Souvenirstand
                if self.state == state::COUNTING && self.count < e.souvenirstand_telegraph =>
            {
                Pose::Telegraph
            }
            EntityKind::Souvenirstand => Pose::Idle,
            EntityKind::Enemy(EnemyKind::Gams) => match self.state {
                state::CROUCH => Pose::Crouch,
                state::HOP => Pose::Hop,
                _ => Pose::Idle,
            },
            // A car always drives (D-093).
            EntityKind::Enemy(EnemyKind::Auto) => Pose::Drive,
            EntityKind::Enemy(EnemyKind::Reisebus | EnemyKind::Regattaboot) => match self.state {
                state::TELEGRAPH | state::RESTART => Pose::Telegraph,
                state::DRIVE => Pose::Drive,
                state::PHOTO_STOP => Pose::Fotostopp {
                    flash: (self.count.saturating_sub(1) / e.reisebus_flash_frames.max(1)) % 2 == 1,
                },
                _ => Pose::Idle,
            },
            EntityKind::Enemy(EnemyKind::Schwan) => match self.state {
                state::TELEGRAPH => Pose::Telegraph,
                state::CHARGE => Pose::Charge,
                _ => Pose::Idle,
            },
            EntityKind::Enemy(EnemyKind::Gelse) => match self.state {
                state::HUNT => Pose::Fly,
                state::LEAVE => Pose::Leave,
                _ => Pose::Wait,
            },
            EntityKind::Enemy(EnemyKind::Kurgast) => match self.state {
                state::LIE => Pose::Liegen,
                state::WAKE => Pose::Wake,
                _ => Pose::Walk,
            },
            EntityKind::Enemy(EnemyKind::Offizier) => match self.state {
                state::ATTENTION => Pose::Habtacht,
                state::PRESENT => Pose::Praesentiert,
                _ => Pose::March,
            },
            // The niche is empty from the drop until the refill.
            EntityKind::Steinschlag => steinschlag_pose(self, t)?,
            EntityKind::Stein => Pose::Fall,
            EntityKind::Note => Pose::Fly,
            EntityKind::Defeated(EnemyKind::Taube | EnemyKind::TaubeSitting) => Pose::Tumble,
            EntityKind::Defeated(_) => Pose::Squished,
            EntityKind::Thrown => Pose::Spin,
            EntityKind::ThrownHackl if self.state == crate::hackl::axe::STUCK => Pose::Stuck,
            EntityKind::ThrownHackl => Pose::Spin,
            _ => return None,
        })
    }

    /// True if it is drawn facing left (`dir < 0`).
    pub fn facing_left(&self) -> bool {
        self.dir < 0
    }

    /// Renderer only: the Bienenschwarm's sprite offset in whole px. On
    /// its waggle run it shakes `swarm_shake` px to either side of the run
    /// (across it, switching every `swarm_shake_frames` frames); the
    /// hitbox stays on the straight line. (0, 0) otherwise.
    pub fn shake_px(&self, t: &Tuning) -> (i32, i32) {
        let e = &t.enemies;
        if self.kind != EntityKind::Enemy(EnemyKind::Bienenschwarm) || self.state != state::RUN {
            return (0, 0);
        }
        let side = if (self.count / e.swarm_shake_frames.max(1)).is_multiple_of(2) {
            e.swarm_shake
        } else {
            -e.swarm_shake
        };
        let (nx, ny) = left_normal(self.dir as u8);
        (nx.signum() * side, ny.signum() * side)
    }

    /// False for an enemy that can't touch the player right now (a
    /// crashed E-Biker, a leaving Gelse, a Kurgast in his deck chair);
    /// true for every other enemy.
    pub fn harmful(&self) -> bool {
        match self.kind {
            EntityKind::Enemy(EnemyKind::EBiker) => self.state != state::CRASH,
            EntityKind::Enemy(EnemyKind::Gelse) => self.state != state::LEAVE,
            EntityKind::Enemy(EnemyKind::Kurgast) => !in_chair(self),
            _ => true,
        }
    }
}

/// True for a Kurgast in his deck chair (resting or waking, D-095).
fn in_chair(e: &Entity) -> bool {
    e.kind == EntityKind::Enemy(EnemyKind::Kurgast) && matches!(e.state, state::LIE | state::WAKE)
}

/// **The enemy-as-platform hook** (D-095): true for an enemy whose hitbox
/// is a one-way top platform right now, a Kurgast in his deck chair. Such
/// a platform never moves, so it needs only the shared landing code
/// ([`crate::platform::land`]), no carry; it despawns like any enemy.
pub(crate) fn is_platform_now(e: &Entity) -> bool {
    in_chair(e)
}

/// True if defeating `e` now scores its points: false for a Kurgast whose
/// first stomp has scored (lying, waking or walking again), so his points
/// total 100 at most (D-095: a bump or an axe after the stomp gives
/// nothing); true for every other enemy.
pub(crate) fn scores_on_defeat(e: &Entity) -> bool {
    e.kind != EntityKind::Enemy(EnemyKind::Kurgast) || e.state == state::WALK
}

/// The initial state of an enemy kind (at spawn and on level reset).
pub(crate) const fn initial_state(kind: EnemyKind) -> u8 {
    match kind {
        EnemyKind::TaubeSitting => state::SIT,
        _ => 0,
    }
}

/// Offset of a sine wave with `amplitude` and `period` (frames) at
/// `frame`, without floats: Bhaskara I's approximation
/// `sin ≈ 16p(H − p) / (5H² − 4p(H − p))` over each half wave (error
/// < 0.2 % of the amplitude), exact at 0, the peaks and the zero crossings.
/// Positive in the first half wave.
pub fn sine_offset(frame: u32, period: u16, amplitude: Fx) -> Fx {
    let h = i64::from(period.max(1));
    // Half-frame units: the half wave is `h` long, so odd periods work too.
    let x = 2 * i64::from(frame % u32::from(period.max(1)));
    let (p, sign) = if x < h { (x, 1) } else { (x - h, -1) };
    let q = p * (h - p);
    let num = i64::from(amplitude.raw()) * 16 * q;
    let den = 5 * h * h - 4 * q;
    let half = den / 2;
    Fx((sign * ((num + half) / den)) as i32)
}

/// Horizontal centre-to-centre distance to the player, whole pixels.
fn player_distance_px(e: &Entity, ctx: &UpdateCtx) -> i32 {
    (ctx.player_cx - (e.x + e.w.div_int(2))).abs().to_px()
}

/// One frame of an active enemy. Returns a thrown Mozartkugel to spawn.
pub(crate) fn update_enemy(e: &mut Entity, kind: EnemyKind, ctx: &UpdateCtx) -> Option<Entity> {
    match kind {
        EnemyKind::SelfieTourist => {
            update_tourist(e, ctx.map, ctx.tuning);
            None
        }
        EnemyKind::Taube | EnemyKind::TaubeSitting => {
            update_taube(e, ctx);
            None
        }
        EnemyKind::MozartkugelVerkaeufer => update_verkaeufer(e, ctx),
        EnemyKind::Fiaker => {
            update_fiaker(e, ctx);
            None
        }
        EnemyKind::NordicWalker => {
            update_walker(e, ctx);
            None
        }
        EnemyKind::Blasmusiker => update_blasmusiker(e, ctx),
        EnemyKind::EBiker => update_e_biker(e, ctx),
        EnemyKind::Pilger => {
            update_pilger(e, ctx);
            None
        }
        EnemyKind::Gams => {
            update_gams(e, ctx);
            None
        }
        EnemyKind::Operettensaenger => {
            update_singer(e, ctx);
            None
        }
        EnemyKind::Schneekugel => {
            update_globe(e, ctx);
            None
        }
        EnemyKind::Reisebus | EnemyKind::Regattaboot => {
            update_reisebus(e, ctx);
            None
        }
        EnemyKind::Schwan => {
            update_schwan(e, ctx);
            None
        }
        EnemyKind::Gelse => {
            update_gelse(e, ctx);
            None
        }
        EnemyKind::Auto => {
            update_auto(e, ctx);
            None
        }
        EnemyKind::Kurgast => {
            update_kurgast(e, ctx);
            None
        }
        EnemyKind::Offizier => {
            update_offizier(e, ctx);
            None
        }
        EnemyKind::Bienenschwarm => {
            let len = e
                .spawn
                .and_then(|i| ctx.level.entities.get(usize::from(i)))
                .map_or(crate::level::DEFAULT_SWARM_LEN, |s| s.len);
            e.count = (e.count + 1) % swarm_period(len, ctx.tuning);
            let (x, y) = (e.x, e.y);
            place_swarm(e, len, ctx.tuning);
            e.vx = e.x - x;
            e.vy = e.y - y;
            None
        }
    }
}

/// True if the cell under feet height `feet` at `x` is a floor (solid or
/// one-way) and `feet` is exactly on its top.
fn floor_at(map: &TileMap, x: Fx, feet: Fx) -> bool {
    feet.raw().rem_euclid(TILE.raw()) == 0
        && matches!(
            map.kind_at(tile_of(x), tile_of(feet)),
            TileKind::Solid | TileKind::OneWay
        )
}

/// Walks like a Selfie-Tourist without selfies (turning at walls), and
/// turns at ledges: while it stands on a floor, the cell under its leading
/// foot one px ahead must be a floor too, else it turns (and walks the
/// other way in the same frame). It only falls when the floor under it
/// disappears.
fn update_walker(e: &mut Entity, ctx: &UpdateCtx) {
    e.state = state::STRIDE;
    walk_turning_at_ledges(e, ctx, ctx.tuning.enemies.walker_speed);
}

/// The Nordic-Walker's walk at `speed` (see [`update_walker`]); also the
/// Kurgast's (D-095) and the Offizier's (D-096). At activation it faces
/// the player; then it turns at a ledge ahead (while on a floor), walks
/// with gravity and turns at a wall. A `speed` of 0 only falls (a halted
/// Offizier: no ledge turn). Leaves `state` alone.
fn walk_turning_at_ledges(e: &mut Entity, ctx: &UpdateCtx, speed: Fx) {
    let (map, t) = (ctx.map, ctx.tuning);
    // At activation it starts towards the player (game-design.md).
    let cx = e.x + e.w.div_int(2);
    if e.timer == 1 && ctx.player_cx != cx {
        e.dir = (ctx.player_cx - cx).signum() as i8;
    }
    let feet = e.y + e.h;
    let last = e.right() - Fx(1);
    let standing = floor_at(map, e.x, feet) || floor_at(map, last, feet);
    let ahead = if e.dir < 0 {
        e.x - Fx::ONE
    } else {
        last + Fx::ONE
    };
    if speed != Fx::ZERO && e.dir != 0 && standing && !floor_at(map, ahead, feet) {
        e.dir = -e.dir;
    }
    e.vx = speed.mul_int(i32::from(e.dir));
    e.vy = (e.vy + t.player.gravity).min(t.player.max_fall_speed);
    let mut b = e.hitbox();
    if map.move_x(&mut b, e.vx) {
        e.dir = -e.dir;
    }
    if map.move_y(&mut b, e.vy, false) != YHit::None {
        e.vy = Fx::ZERO;
    }
    e.x = b.x;
    e.y = b.y;
}

/// Faces the player. While the player is within `blasmusiker_range`, the
/// bar counter (`count`, frame of the bar) runs; outside it pauses and he
/// keeps his pose. Each bar: inhale for the first `blasmusiker_inhale`
/// frames, a note at each of `blasmusiker_notes` (blow pose for
/// `blasmusiker_blow_pose` frames), idle until the bar ends. The counter
/// starts at 0, so the first note train is preceded by an inhale.
fn update_blasmusiker(e: &mut Entity, ctx: &UpdateCtx) -> Option<Entity> {
    let et = &ctx.tuning.enemies;
    let cx = e.x + e.w.div_int(2);
    if ctx.player_cx != cx {
        e.dir = (ctx.player_cx - cx).signum() as i8;
    }
    if player_distance_px(e, ctx) > et.blasmusiker_range {
        return None;
    }
    let f = e.count;
    e.count = (e.count + 1) % et.blasmusiker_bar;
    let last_note = et.blasmusiker_notes.iter().rev().find(|&&n| n <= f);
    e.state = if f < et.blasmusiker_inhale {
        state::INHALE
    } else if last_note.is_some_and(|&n| f < n + et.blasmusiker_blow_pose) {
        state::BLOW
    } else {
        state::STAND
    };
    et.blasmusiker_notes
        .contains(&f)
        .then(|| note(e, ctx.tuning))
}

/// A note from the tuba bell: its centre `note_height` px above the
/// musician's feet and `note_ahead` px in front of his centre, flying
/// towards his facing side.
fn note(e: &Entity, t: &Tuning) -> Entity {
    let et = &t.enemies;
    let dir = i32::from(e.dir);
    let (cx, feet) = e.feet();
    let [w, h] = et.note_hitbox;
    let mut n = Entity::at_feet(
        EntityKind::Note,
        cx + Fx::from_px(et.note_ahead * dir),
        feet - Fx::from_px(et.note_height) + Fx::from_px(h).div_int(2),
        [w, h],
    );
    n.vx = et.note_speed.mul_int(dir);
    n.dir = e.dir;
    n
}

/// A note flies straight (ignoring gravity, one-way tiles and enemies) and
/// vanishes once it has flown `note_range` px from its start, touches a
/// solid tile or leaves the level sideways (left and right of the level
/// count as solid).
pub(crate) fn update_note(e: &mut Entity, map: &TileMap, t: &Tuning) {
    e.x += e.vx;
    let flown = (e.x - e.base_x).abs();
    if flown >= Fx::from_px(t.enemies.note_range) || map.touches(&e.hitbox(), TileKind::Solid) {
        e.removed = true;
    }
}

/// Parked until the player is within `e_biker_trigger`, then telegraphs
/// for `e_biker_telegraph` frames (`count` = frame of the telegraph), then
/// rides left with the player's gravity and max fall speed (off ledges,
/// into water: gone; through other enemies). A wall stops it: it crashes
/// (harmless, `count` = frame of the crash, removed after
/// `e_biker_crash_frames`) and returns the „Hoppala!" popup. Removed once
/// its right edge is left of the screen. No points except for a stomp.
fn update_e_biker(e: &mut Entity, ctx: &UpdateCtx) -> Option<Entity> {
    let et = &ctx.tuning.enemies;
    match e.state {
        state::PARK => {
            if player_distance_px(e, ctx) <= et.e_biker_trigger {
                e.state = state::RING;
                e.count = 1;
            }
        }
        state::RING => {
            e.count += 1;
            if e.count > et.e_biker_telegraph {
                e.state = state::RIDE;
                e.count = 0;
            }
        }
        state::CRASH => {
            e.count += 1;
            if e.count >= et.e_biker_crash_frames {
                e.removed = true;
            }
            return None;
        }
        _ => {}
    }
    if e.state != state::RIDE {
        return None;
    }
    let t = ctx.tuning;
    e.vx = -et.e_biker_speed;
    e.vy = (e.vy + t.player.gravity).min(t.player.max_fall_speed);
    let mut b = e.hitbox();
    let wall = ctx.map.move_x(&mut b, e.vx);
    if ctx.map.move_y(&mut b, e.vy, false) != YHit::None {
        e.vy = Fx::ZERO;
    }
    e.x = b.x;
    e.y = b.y;
    if wall {
        e.state = state::CRASH;
        e.count = 0;
        e.vx = Fx::ZERO;
        e.vy = Fx::ZERO;
        let (cx, _) = e.feet();
        return Some(popup(Popup::Hoppala, cx, e.y - TILE.div_int(2)));
    }
    if ctx.map.swallows(&b) || e.right() <= ctx.screen_left {
        e.removed = true;
    }
    None
}

/// Run direction (0..=7, × 45°, 0 = right, 2 = up) as a unit vector in
/// 1/256 px (diagonals 181/256 ≈ 1/√2), screen coordinates (y down).
const SWARM_DIRS: [(i32, i32); 8] = [
    (256, 0),
    (181, -181),
    (0, -256),
    (-181, -181),
    (-256, 0),
    (-181, 181),
    (0, 256),
    (181, 181),
];

/// The left side of a run in `dir` (the run rotated 90° counter-clockwise
/// on screen), in 1/256 px.
fn left_normal(dir: u8) -> (i32, i32) {
    let (ux, uy) = SWARM_DIRS[usize::from(dir & 7)];
    (uy, -ux)
}

/// Frames of a swarm's figure-eight: 2 × (run + return), the run being
/// `len` tiles at 1 px per frame.
pub fn swarm_period(len: u8, t: &Tuning) -> u16 {
    2 * (u16::from(len) * TILE_PX as u16 + t.enemies.swarm_return_frames)
}

/// Where a swarm is `k` frames into its figure-eight: the offset of its
/// hitbox centre from the anchor A (subpixels) and whether it is on a
/// waggle run. The cycle: run from A straight along `dir` for L = `len`
/// tiles at 1 px/frame (A → B), return to A on the half circle over AB on
/// the run's left side in `swarm_return_frames` frames, run A → B again,
/// return on the right side. Integer sine (D-026), no floats.
pub fn swarm_offset(dir: u8, len: u8, k: u16, t: &Tuning) -> (Fx, Fx, bool) {
    let (ux, uy) = SWARM_DIRS[usize::from(dir & 7)];
    let (nx, ny) = left_normal(dir);
    let run = i32::from(len) * TILE_PX;
    let back = i32::from(t.enemies.swarm_return_frames.max(1));
    let k = i32::from(k) % (2 * (run + back));
    let (j, side) = if k < run + back {
        (k, 1)
    } else {
        (k - run - back, -1)
    };
    if j < run {
        return (Fx(ux * j), Fx(uy * j), true);
    }
    // Half circle from B (angle 0) back to A (angle π) around the middle
    // of AB: sin and cos of π·j/back via the quarter-period trick.
    let j = j - run;
    let r = Fx::from_px(run).div_int(2);
    let p4 = (4 * back) as u16;
    let cos = sine_offset((2 * j + back) as u32, p4, r);
    let sin = sine_offset((2 * j) as u32, p4, r);
    let along = i64::from((r + cos).raw());
    let across = i64::from(sin.raw()) * i64::from(side);
    let ox = (i64::from(ux) * along + i64::from(nx) * across).div_euclid(256);
    let oy = (i64::from(uy) * along + i64::from(ny) * across).div_euclid(256);
    (Fx(ox as i32), Fx(oy as i32), false)
}

/// Puts a swarm where its `count` says (relative to its anchor hitbox at
/// `base_x`/`base_y`) and sets its state (run or return).
pub(crate) fn place_swarm(e: &mut Entity, len: u8, t: &Tuning) {
    let (ox, oy, run) = swarm_offset(e.dir as u8, len, e.count, t);
    e.x = e.base_x + ox;
    e.y = e.base_y + oy;
    e.state = if run { state::RUN } else { state::RETURN };
}

/// Walks (turning at walls, falling off ledges); every
/// `tourist_selfie_period` frames from activation it stands still for
/// `tourist_selfie_frames` frames. `count` is the phase in the period
/// (0 = a selfie starts, from the second period on).
fn update_tourist(e: &mut Entity, map: &TileMap, t: &Tuning) {
    let et = &t.enemies;
    e.count = (e.count + 1) % et.tourist_selfie_period;
    let selfie = e.timer >= et.tourist_selfie_period && e.count < et.tourist_selfie_frames;
    e.state = if selfie { state::SELFIE } else { state::WALK };
    e.vx = if selfie {
        Fx::ZERO
    } else {
        et.tourist_speed.mul_int(i32::from(e.dir))
    };
    e.vy = (e.vy + t.player.gravity).min(t.player.max_fall_speed);
    let mut b = e.hitbox();
    if map.move_x(&mut b, e.vx) {
        e.dir = -e.dir;
    }
    if map.move_y(&mut b, e.vy, false) != YHit::None {
        e.vy = Fx::ZERO;
    }
    e.x = b.x;
    e.y = b.y;
}

/// Flies left on a sine wave around `base_y` (ignoring tiles). A sitting
/// Taube waits until the player is close, rises, then flies from there;
/// `count` is the frame in the current state.
fn update_taube(e: &mut Entity, ctx: &UpdateCtx) {
    let et = &ctx.tuning.enemies;
    match e.state {
        state::SIT => {
            if player_distance_px(e, ctx) <= et.taube_sitting_trigger {
                e.state = state::RISE;
                e.count = 0;
            } else {
                return;
            }
        }
        state::RISE => {}
        _ => {
            e.count = (e.count + 1) % et.taube_period;
            e.x += et.taube_speed.mul_int(i32::from(e.dir));
            e.y = e.base_y
                - sine_offset(
                    u32::from(e.count),
                    et.taube_period,
                    Fx::from_px(et.taube_amplitude),
                );
            return;
        }
    }
    // Rising (also on the frame the player came close).
    e.count += 1;
    let frames = i32::from(et.taube_rise_frames);
    let k = i32::from(e.count).min(frames);
    e.y = e.base_y - Fx::from_px(et.taube_rise).mul_int(k).div_int(frames);
    if e.count >= et.taube_rise_frames {
        e.state = state::FLY;
        e.count = 0;
        e.base_y = e.y;
    }
}

/// Faces the player. While the player is within `verkaeufer_range`, the
/// throw counter (`count`) runs: wind-up for the last `verkaeufer_windup`
/// frames of each `verkaeufer_period`, then a throw.
fn update_verkaeufer(e: &mut Entity, ctx: &UpdateCtx) -> Option<Entity> {
    let et = &ctx.tuning.enemies;
    let cx = e.x + e.w.div_int(2);
    if ctx.player_cx != cx {
        e.dir = (ctx.player_cx - cx).signum() as i8;
    }
    if player_distance_px(e, ctx) > et.verkaeufer_range {
        if e.state == state::THROW {
            e.state = state::IDLE;
        }
        return None;
    }
    e.count += 1;
    if e.count >= et.verkaeufer_period {
        e.count = 0;
        e.state = state::THROW;
        return Some(throw(e, ctx.tuning));
    }
    if e.count >= et.verkaeufer_period - et.verkaeufer_windup {
        e.state = state::WINDUP;
    } else if e.state == state::THROW && e.count >= et.verkaeufer_throw_pose {
        e.state = state::IDLE;
    }
    None
}

/// The thrown Mozartkugel: from the raised hand (above the head, on the
/// facing side), up and towards the player's side.
fn throw(e: &Entity, t: &Tuning) -> Entity {
    let et = &t.enemies;
    let dir = i32::from(e.dir);
    let hand_x = e.x + e.w.div_int(2) + Fx::from_px(4).mul_int(dir);
    let mut k = Entity::at_feet(
        EntityKind::Thrown,
        hand_x,
        e.y + Fx::from_px(2),
        et.thrown_hitbox,
    );
    k.vx = et.thrown_speed_x.mul_int(dir);
    k.vy = -et.thrown_speed_y;
    k.dir = e.dir;
    k
}

/// Parked until the player is within `fiaker_trigger`, then telegraphs for
/// `fiaker_telegraph` frames (`count` = frame of the telegraph), then
/// charges left along its row (ignoring tiles, D-026) until it has left
/// the screen.
fn update_fiaker(e: &mut Entity, ctx: &UpdateCtx) {
    let et = &ctx.tuning.enemies;
    match e.state {
        state::PARKED => {
            if player_distance_px(e, ctx) <= et.fiaker_trigger {
                e.state = state::TELEGRAPH;
                e.count = 1;
            }
        }
        state::TELEGRAPH => {
            e.count += 1;
            if e.count > et.fiaker_telegraph {
                e.state = state::CHARGE;
            }
        }
        _ => {}
    }
    if e.state == state::CHARGE {
        e.vx = -et.fiaker_speed;
        e.x += e.vx;
        if e.right() <= ctx.screen_left {
            e.removed = true;
        }
    }
}

/// The Reisebus (D-070). Parked until the player is within
/// `reisebus_trigger` of its centre, then telegraphs for
/// `reisebus_telegraph` frames (`count` = frame of the telegraph, the
/// Fiaker's rule), then drives left at `reisebus_speed`, ignoring tiles and
/// enemies. With a `stop` it halts exactly at its parked x − `stop` tiles:
/// `reisebus_stop_frames` frames of photo stop (the arrival frame is the
/// first), `reisebus_restart` frames of telegraph, then it drives on.
/// Removed once its right edge is left of the screen.
///
/// The Regattaboot runs the same machine (D-080) with its own
/// `regattaboot_trigger`, `_telegraph` and `_speed`; its trigger also needs
/// it [`fully_on_screen`], and it has no `stop` (the level syntax has no
/// such field), so it never halts.
fn update_reisebus(e: &mut Entity, ctx: &UpdateCtx) {
    let et = &ctx.tuning.enemies;
    let boat = e.kind == EntityKind::Enemy(EnemyKind::Regattaboot);
    let (trigger, telegraph, speed) = if boat {
        (
            et.regattaboot_trigger,
            et.regattaboot_telegraph,
            et.regattaboot_speed,
        )
    } else {
        (
            et.reisebus_trigger,
            et.reisebus_telegraph,
            et.reisebus_speed,
        )
    };
    match e.state {
        state::PARKED => {
            if player_distance_px(e, ctx) <= trigger && (!boat || fully_on_screen(e, ctx)) {
                e.state = state::TELEGRAPH;
                e.count = 1;
            }
        }
        state::TELEGRAPH => {
            e.count += 1;
            if e.count > telegraph {
                e.state = state::DRIVE;
                e.count = 0;
            }
        }
        state::PHOTO_STOP => {
            e.count += 1;
            if e.count > et.reisebus_stop_frames {
                e.state = state::RESTART;
                e.count = 1;
            }
        }
        state::RESTART => {
            e.count += 1;
            if e.count > et.reisebus_restart {
                e.state = state::DRIVE;
                e.count = 0;
            }
        }
        _ => {}
    }
    if e.state != state::DRIVE {
        e.vx = Fx::ZERO;
        return;
    }
    e.vx = -speed;
    let stop = e
        .spawn
        .and_then(|i| ctx.level.entities.get(usize::from(i)))
        .map_or(0, |s| s.stop);
    let stop_x = e.base_x - TILE.mul_int(i32::from(stop));
    if stop > 0 && e.x > stop_x {
        e.x = (e.x + e.vx).max(stop_x);
        if e.x == stop_x {
            e.state = state::PHOTO_STOP;
            e.count = 1;
            e.vx = Fx::ZERO;
        }
    } else {
        e.x += e.vx;
    }
    if e.right() <= ctx.screen_left {
        e.removed = true;
    }
}

/// How many tiles left of a parked Reisebus (or Regattaboot) its lane
/// must be free (D-070, D-080).
pub const REISEBUS_LANE_TILES: i32 = 24;

/// `levels lint` (D-070, D-080, D-093): every Reisebus's and Regattaboot's
/// lane, the rows of its hitbox from its own cells to
/// [`REISEBUS_LANE_TILES`] tiles left of its parked hitbox (or the level's
/// left edge), holds no solid or one-way tile, so it is never seen driving
/// through a wall (one problem per vehicle, naming the first blocked cell
/// from the left); and a bus's photo stop (`stop`) must not lie left of the
/// level's left edge. Water tiles in a boat's lane are fine. An Ampel's
/// lane is its cars' (the hitbox of a car at its spawn point, through the
/// cell it pokes into, to 24 tiles left of the spawner).
pub fn reisebus_lane_problems(level: &Level, t: &Tuning) -> Vec<String> {
    let mut problems = Vec::new();
    for (i, s) in level.entities.iter().enumerate() {
        let decision = match s.kind {
            SpawnKind::Reisebus => "D-070",
            SpawnKind::Regattaboot => "D-080",
            SpawnKind::Ampel => "D-093",
            _ => continue,
        };
        let name = s.kind.name();
        let Some(mut bus) = Entity::from_spawn(s, i, t) else {
            continue;
        };
        if s.kind == SpawnKind::Ampel {
            bus = new_car(&bus, 0, t);
        }
        let (rows, last) = (tile_of(bus.y)..=s.row, tile_of(bus.right() - Fx(1)));
        let first = tile_of(bus.x - TILE.mul_int(REISEBUS_LANE_TILES)).max(0);
        let blocked = (first..=last).find_map(|c| {
            rows.clone().find_map(|r| {
                let solid = level
                    .tile(c, r)
                    .is_some_and(|tile| matches!(tile.kind, TileKind::Solid | TileKind::OneWay));
                solid.then_some((c, r))
            })
        });
        let stop_x = bus.x - TILE.mul_int(i32::from(s.stop));
        if stop_x < Fx::ZERO {
            problems.push(format!(
                "{name} at row {}, column {}: its photo stop (stop = {}) would be {} px left of the level's left edge",
                s.row + 1,
                s.col + 1,
                s.stop,
                (-stop_x).to_px()
            ));
        }
        if let Some((c, r)) = blocked {
            problems.push(format!(
                "{name} at row {}, column {}: its lane (rows {}-{}, columns {}-{}) has a solid or one-way tile at row {}, column {} ({decision}: keep it clear for {REISEBUS_LANE_TILES} tiles to the left, or to the level's left edge)",
                s.row + 1,
                s.col + 1,
                rows.start() + 1,
                rows.end() + 1,
                first + 1,
                last + 1,
                r + 1,
                c + 1
            ));
        }
    }
    problems
}

/// A thrown Mozartkugel: moves, then gravity; vanishes when it hits a
/// solid tile (one-way tiles don't stop it) or leaves the level.
pub(crate) fn update_thrown(e: &mut Entity, map: &TileMap, t: &Tuning) {
    let mut b = e.hitbox();
    let hit_x = map.move_x(&mut b, e.vx);
    let hit_y = map.move_y(&mut b, e.vy, true) != YHit::None;
    e.x = b.x;
    e.y = b.y;
    e.vy += t.enemies.thrown_gravity;
    if hit_x || hit_y || e.right() <= Fx::ZERO || e.x >= map.width_fx() {
        e.removed = true;
    }
}

/// A defeated enemy: squished ones vanish after `squish_frames`, tumbling
/// ones fall (player gravity) without collision until below the level.
pub(crate) fn update_defeated(e: &mut Entity, t: &Tuning) {
    if e.state == state::TUMBLE {
        e.vy = (e.vy + t.player.gravity).min(t.player.max_fall_speed);
        e.x += e.vx;
        e.y += e.vy;
    } else if e.timer >= t.enemies.squish_frames {
        e.removed = true;
    }
}

/// Turns an enemy into its defeated form: squished in place, or tumbling
/// (Tauben always tumble; `knocked` starts with an upward hop).
pub(crate) fn defeat(e: &mut Entity, kind: EnemyKind, knocked: bool, t: &Tuning) {
    e.kind = EntityKind::Defeated(kind);
    // A snow globe is never drawn defeated: it shatters at once (the game
    // spawns its shards, D-069).
    e.removed |= kind == EnemyKind::Schneekugel;
    e.timer = 0;
    e.count = 0;
    e.vx = Fx::ZERO;
    e.vy = Fx::ZERO;
    let flies = matches!(kind, EnemyKind::Taube | EnemyKind::TaubeSitting);
    e.state = if knocked || flies {
        state::TUMBLE
    } else {
        state::SQUISHED
    };
    if knocked {
        e.vy = -t.enemies.knock_speed;
    }
}

/// True for enemies that walk or stand on the ground (they block walking
/// Selfie-Tourists and Nordic-Walkers).
fn on_foot(e: &Entity) -> bool {
    match e.kind {
        EntityKind::Enemy(EnemyKind::SelfieTourist)
        | EntityKind::Enemy(EnemyKind::MozartkugelVerkaeufer)
        | EntityKind::Enemy(EnemyKind::NordicWalker)
        | EntityKind::Enemy(EnemyKind::Blasmusiker)
        | EntityKind::Enemy(EnemyKind::Pilger)
        | EntityKind::Enemy(EnemyKind::Operettensaenger)
        // Walking, or lying in his deck chair: walkers turn at it (D-095).
        | EntityKind::Enemy(EnemyKind::Kurgast)
        | EntityKind::Enemy(EnemyKind::Offizier) => true,
        EntityKind::Enemy(EnemyKind::Gams) => e.state != state::HOP,
        EntityKind::Enemy(EnemyKind::Fiaker) => e.state != state::CHARGE,
        EntityKind::Enemy(EnemyKind::Reisebus | EnemyKind::Regattaboot) => e.state != state::DRIVE,
        EntityKind::Enemy(EnemyKind::EBiker) => matches!(e.state, state::PARK | state::RING),
        EntityKind::Enemy(EnemyKind::TaubeSitting) => e.state == state::SIT,
        _ => false,
    }
}

/// Selfie-Tourists, Nordic-Walkers, Pilger, Offiziere and walking
/// Kurgäste that overlap another enemy on foot turn away from it (after
/// everything moved; the direction is set, not flipped, so two that
/// overlap for several frames don't flip back and forth). A lying Kurgast
/// counts with his chair widened by `kurgast_berth` px on each side
/// ([`turn_box`], D-102), so a walker turns before it reaches a rider
/// hanging past the chair.
pub(crate) fn turn_at_enemies(entities: &mut [Entity], t: &Tuning) {
    for i in 0..entities.len() {
        let a = entities[i];
        let walks = match a.kind {
            EntityKind::Enemy(
                EnemyKind::SelfieTourist
                | EnemyKind::NordicWalker
                | EnemyKind::Pilger
                | EnemyKind::Offizier,
            ) => true,
            EntityKind::Enemy(EnemyKind::Kurgast) => !in_chair(&a),
            _ => false,
        };
        // `dir == 0`: a `still` tourist (test levels) never turns.
        if !a.active || a.removed || a.dir == 0 || !walks {
            continue;
        }
        let ac = a.x + a.w.div_int(2);
        let other = entities.iter().enumerate().find(|(j, b)| {
            *j != i && b.active && !b.removed && on_foot(b) && turn_box(b, t).overlaps(&a.hitbox())
        });
        if let Some((_, b)) = other {
            let bc = b.x + b.w.div_int(2);
            let away = (ac - bc).signum() as i8;
            entities[i].dir = if away == 0 { -a.dir } else { away };
        }
    }
}

/// The box a walker turns at in [`turn_at_enemies`]: a lying Kurgast's
/// chair widened by `kurgast_berth` px on each side, same rows (D-102);
/// every other enemy's hitbox.
fn turn_box(e: &Entity, t: &Tuning) -> Aabb {
    let b = e.hitbox();
    if !in_chair(e) {
        return b;
    }
    let berth = Fx::from_px(t.enemies.kurgast_berth);
    Aabb {
        x: b.x - berth,
        w: b.w + berth.mul_int(2),
        ..b
    }
}

/// A block at (`col`, `row`) was bumped from below: every stompable enemy
/// standing on it (feet exactly on its top, overlapping the cell) is
/// knocked off (defeated, tumbling), a Kurgast also from his deck chair.
/// Rolling items standing on it hop. Returns the knocked enemies' kind,
/// feet (for points and popups) and whether they score
/// ([`scores_on_defeat`]).
pub(crate) fn knock_off_block(
    entities: &mut [Entity],
    col: i32,
    row: i32,
    t: &Tuning,
) -> Vec<(EnemyKind, Fx, Fx, bool)> {
    let (left, top) = (TILE.mul_int(col), TILE.mul_int(row));
    let mut knocked = Vec::new();
    for e in entities.iter_mut() {
        let standing = e.y + e.h == top && e.x < left + TILE && left < e.right();
        if !e.active || e.removed || !standing {
            continue;
        }
        match e.kind {
            EntityKind::Enemy(kind) if kind.stompable() && on_foot(e) => {
                let scores = scores_on_defeat(e);
                defeat(e, kind, true, t);
                knocked.push((kind, e.x + e.w.div_int(2), e.y, scores));
            }
            EntityKind::Mozartkugel | EntityKind::Zaunerstollen if !e.rising(t) => {
                e.vy = -t.enemies.knock_speed;
            }
            _ => {}
        }
    }
    knocked
}

// ------------------------------------------------------------- M3 (D-058)

/// Walks like a Selfie-Tourist without selfies: towards the player at
/// activation, turning at walls (and, in [`turn_at_enemies`], at enemies
/// on foot), and **off ledges** (gravity and max fall speed as the
/// player's), so pilgrims come down a mountain step by step. A group
/// placed one cell apart stays in file: same speed, same rules.
fn update_pilger(e: &mut Entity, ctx: &UpdateCtx) {
    let (map, t) = (ctx.map, ctx.tuning);
    let cx = e.x + e.w.div_int(2);
    if e.timer == 1 && ctx.player_cx != cx {
        e.dir = (ctx.player_cx - cx).signum() as i8;
    }
    e.state = state::WALK;
    e.vx = t.enemies.pilger_speed.mul_int(i32::from(e.dir));
    e.vy = (e.vy + t.player.gravity).min(t.player.max_fall_speed);
    let mut b = e.hitbox();
    if map.move_x(&mut b, e.vx) {
        e.dir = -e.dir;
    }
    if map.move_y(&mut b, e.vy, false) != YHit::None {
        e.vy = Fx::ZERO;
    }
    e.x = b.x;
    e.y = b.y;
}

/// The file a Pilger belongs to (D-061 addendum): Pilger spawned in
/// adjacent cells of the same row form one file, named by its row and its
/// leftmost column. `None` for every other entity (and for entities
/// without a spawn). The game activates a file together, so a group that
/// straddles the activation edge when the camera stops doesn't split.
pub(crate) fn pilger_file(level: &Level, e: &Entity) -> Option<(i32, i32)> {
    if e.kind != EntityKind::Enemy(EnemyKind::Pilger) {
        return None;
    }
    let s = level.entities.get(usize::from(e.spawn?))?;
    let is_pilger_at = |col: i32| {
        level
            .entities
            .iter()
            .any(|o| o.kind == SpawnKind::Pilger && o.row == s.row && o.col == col)
    };
    let mut col = s.col;
    while is_pilger_at(col - 1) {
        col -= 1;
    }
    Some((s.row, col))
}

/// The Gams cycle (`count` = frame in the current state, 1-based): stand
/// `gams_stand` frames, crouch `gams_crouch`, then hop until it lands
/// (32 frames with the content tuning: gravity is applied before the move,
/// so it is back at its height after 31 frames and lands on the 32nd,
/// `gams_hop_tiles` tiles further; apex 30 px). At the start of each
/// crouch it faces the player and plans the hop ([`plan_hop`]); during the
/// crouch `vx` holds the planned hop speed but it doesn't move. In a hop it
/// collides with tiles (lands on solid and one-way tops, a wall stops
/// `vx`). It always has gravity, so it falls when its floor disappears.
fn update_gams(e: &mut Entity, ctx: &UpdateCtx) {
    let (map, t) = (ctx.map, ctx.tuning);
    let et = &t.enemies;
    e.count = e.count.saturating_add(1);
    if e.state == state::STAND && e.count > et.gams_stand {
        e.state = state::CROUCH;
        e.count = 1;
        let hop = plan_hop(e, ctx);
        e.vx = et.gams_hop_speed_x.mul_int(i32::from(hop));
    }
    if e.state == state::CROUCH && e.count > et.gams_crouch {
        e.state = state::HOP;
        e.count = 1;
        e.vy = -et.gams_hop_speed_y;
    }
    e.vy = (e.vy + et.gams_gravity).min(t.player.max_fall_speed);
    let mut b = e.hitbox();
    if e.state == state::HOP && map.move_x(&mut b, e.vx) {
        e.vx = Fx::ZERO;
    }
    match map.move_y(&mut b, e.vy, false) {
        YHit::Floor => {
            e.vy = Fx::ZERO;
            if e.state == state::HOP {
                e.state = state::STAND;
                e.count = 0;
                e.vx = Fx::ZERO;
            }
        }
        YHit::Ceiling => e.vy = Fx::ZERO,
        YHit::None => {}
    }
    e.x = b.x;
    e.y = b.y;
}

/// At the start of a crouch: faces the player and returns the hop's
/// direction. The way towards the player if [`hop_is_clear`], else the
/// other way (it turns), else 0: a hop straight up. So it never leaves its
/// ledge.
fn plan_hop(e: &mut Entity, ctx: &UpdateCtx) -> i8 {
    let cx = e.x + e.w.div_int(2);
    if ctx.player_cx != cx {
        e.dir = (ctx.player_cx - cx).signum() as i8;
    }
    if e.dir == 0 {
        e.dir = -1;
    }
    let t = ctx.tuning;
    if hop_is_clear(e, ctx.map, e.dir, t) {
        e.dir
    } else if hop_is_clear(e, ctx.map, -e.dir, t) {
        e.dir = -e.dir;
        e.dir
    } else {
        0
    }
}

/// True if a Gams standing exactly on a floor can hop `gams_hop_tiles`
/// tiles towards `dir` without leaving its ledge: every column its hitbox
/// sweeps over (from where it stands to the landing spot) has a solid or
/// one-way top at its feet height, and no solid tile is in its body's rows
/// along the way (a wall). Checking the whole sweep, not only the landing
/// cell, keeps it off a neighbouring ledge across a gap.
pub fn hop_is_clear(e: &Entity, map: &TileMap, dir: i8, t: &Tuning) -> bool {
    let feet = e.y + e.h;
    if feet.raw().rem_euclid(TILE.raw()) != 0 {
        return false;
    }
    let d = TILE.mul_int(t.enemies.gams_hop_tiles * i32::from(dir));
    let (left, right) = (e.x.min(e.x + d), e.right().max(e.right() + d) - Fx(1));
    let floor_row = tile_of(feet);
    let (r0, r1) = (tile_of(e.y), floor_row - 1);
    (tile_of(left)..=tile_of(right)).all(|c| {
        matches!(
            map.kind_at(c, floor_row),
            TileKind::Solid | TileKind::OneWay
        ) && (r0..=r1).all(|r| map.kind_at(c, r) != TileKind::Solid)
    })
}

/// A rock at rest in the niche cell (`col`, `row`): the `stein_hitbox`
/// centred horizontally, its bottom 2 px above the cell's bottom (the
/// 16×16 sprite's origin is 8,14, so the sprite fills the cell). Used for
/// the spawner (`kind` [`EntityKind::Steinschlag`]) and for each falling
/// rock it drops ([`EntityKind::Stein`]).
pub(crate) fn resting_rock(kind: EntityKind, col: i32, row: i32, t: &Tuning) -> Entity {
    let feet_x = TILE.mul_int(col) + TILE.div_int(2);
    let feet_y = TILE.mul_int(row + 1) - Fx::from_px(2);
    Entity::at_feet(kind, feet_x, feet_y, t.enemies.stein_hitbox)
}

/// The niche's pose for its counter frame: the resting rock while waiting,
/// `wobble` in the telegraph, empty (`None`) from the drop until
/// `steinschlag_refill` frames later, then `rest` again.
fn steinschlag_pose(e: &Entity, t: &Tuning) -> Option<Pose> {
    let et = &t.enemies;
    if e.state == state::WAITING {
        return Some(Pose::Rest);
    }
    let f = e.count;
    if f < et.steinschlag_telegraph {
        Some(Pose::Wobble)
    } else if f < et.steinschlag_telegraph + et.steinschlag_refill {
        None
    } else {
        Some(Pose::Rest)
    }
}

/// The Steinschlag spawner. Its counter (`count`, frame of its `period`)
/// runs only while the player is within `range` tiles (horizontally,
/// centre to centre) **and** its cell is fully on screen with its top at
/// least `steinschlag_screen_top` px below the screen top (the HUD);
/// otherwise it pauses with its pose kept. On the first such frame it
/// starts at `phase` (0 or ≥ 41, so a first rock always follows a whole
/// telegraph); frames 0..`steinschlag_telegraph` are the telegraph, and at
/// frame `steinschlag_telegraph` it returns the falling rock to spawn.
pub(crate) fn update_steinschlag(e: &mut Entity, ctx: &UpdateCtx) -> Option<Entity> {
    let t = ctx.tuning;
    let s = e
        .spawn
        .and_then(|i| ctx.level.entities.get(usize::from(i)))?;
    let in_range = player_distance_px(e, ctx) <= i32::from(s.range) * TILE_PX;
    let (left, top) = (TILE.mul_int(s.col), TILE.mul_int(s.row));
    let screen_w = Fx::from_px(crate::camera::SCREEN_W);
    let screen_h = Fx::from_px(crate::camera::SCREEN_H);
    let on_screen = left >= ctx.screen_left
        && left + TILE <= ctx.screen_left + screen_w
        && top >= ctx.screen_top + Fx::from_px(t.enemies.steinschlag_screen_top)
        && top + TILE <= ctx.screen_top + screen_h;
    if !(in_range && on_screen) {
        return None;
    }
    let period = s.period.max(1);
    if e.state == state::WAITING {
        e.state = state::COUNTING;
        e.count = s.phase % period;
    } else {
        e.count = (e.count + 1) % period;
    }
    (e.count == t.enemies.steinschlag_telegraph)
        .then(|| resting_rock(EntityKind::Stein, s.col, s.row, t))
}

/// A falling rock: straight down from rest (gravity `stein_gravity`, max
/// `stein_max_fall`); it shatters (state [`state::SHATTERED`], removed; the
/// game spawns the debris) on the first solid or one-way tile its bottom
/// reaches. Below the level it is removed like any entity. It ignores
/// water and hazard tiles, enemies and items.
pub(crate) fn update_stein(e: &mut Entity, map: &TileMap, t: &Tuning) {
    let et = &t.enemies;
    e.vy = (e.vy + et.stein_gravity).min(et.stein_max_fall);
    let mut b = e.hitbox();
    if map.move_y(&mut b, e.vy, false) == YHit::Floor {
        e.state = state::SHATTERED;
        e.removed = true;
    }
    e.y = b.y;
}

/// The four `stein_debris` pieces of a shattered rock (8×8, like a broken
/// brick's: the upper pair flies higher), around its hitbox centre.
pub(crate) fn stein_debris(e: &Entity, t: &Tuning) -> [Entity; 4] {
    pieces(e, EntityKind::SteinDebris, t)
}

/// The four `schneekugel_scherben` shards of a shattered snow globe (on a
/// wall, stomped or hit by an axe), like a rock's debris.
pub(crate) fn globe_shards(e: &Entity, t: &Tuning) -> [Entity; 4] {
    pieces(e, EntityKind::SchneekugelScherben, t)
}

/// Four 8×8 pieces of `kind` flying apart from `e`'s hitbox centre.
fn pieces(e: &Entity, kind: EntityKind, t: &Tuning) -> [Entity; 4] {
    let it = &t.items;
    let half = TILE.div_int(2);
    let (cx, cy) = (e.x + e.w.div_int(2), e.y + e.h.div_int(2));
    [
        (-half, -half, -it.debris_speed_x, -it.debris_speed_high),
        (Fx::ZERO, -half, it.debris_speed_x, -it.debris_speed_high),
        (-half, Fx::ZERO, -it.debris_speed_x, -it.debris_speed_low),
        (Fx::ZERO, Fx::ZERO, it.debris_speed_x, -it.debris_speed_low),
    ]
    .map(|(dx, dy, vx, vy)| {
        let mut d = Entity::new(kind, cx + dx, cy + dy, half, half);
        d.vx = vx;
        d.vy = vy;
        d
    })
}

/// The column a Steinschlag's rocks fall down (renderer: the `kiesel`
/// trickle of the telegraph): the y of the first solid or one-way tile top
/// at or below the resting rock's bottom in the spawner's column, or the
/// level's bottom.
pub fn rock_column_bottom(e: &Entity, map: &TileMap) -> Fx {
    let col = tile_of(e.x + e.w.div_int(2));
    let start = tile_of(e.y + e.h);
    let height = tile_of(map.height_fx());
    (start..height)
        .find(|&r| matches!(map.kind_at(col, r), TileKind::Solid | TileKind::OneWay))
        .map_or(map.height_fx(), |r| TILE.mul_int(r))
}

// ------------------------------------------------------- M4 (D-068, D-069)

/// Faces the player. While the player is within `operettensaenger_range`
/// his cycle counter (`count`, frame of the cycle) runs; outside it pauses
/// and he keeps his pose (and a sounding column), like the Blasmusiker. It
/// starts at 0 on the first frame in range: inhale for the first
/// `operettensaenger_inhale` frames, the high C for the next
/// `operettensaenger_sing` ([`high_c_column`]), idle to the cycle's end.
fn update_singer(e: &mut Entity, ctx: &UpdateCtx) {
    let et = &ctx.tuning.enemies;
    let cx = e.x + e.w.div_int(2);
    if ctx.player_cx != cx {
        e.dir = (ctx.player_cx - cx).signum() as i8;
    }
    if player_distance_px(e, ctx) > et.operettensaenger_range {
        return;
    }
    let f = e.count;
    e.count = (f + 1) % et.operettensaenger_cycle;
    e.state = if f < et.operettensaenger_inhale {
        state::INHALE
    } else if f < et.operettensaenger_inhale + et.operettensaenger_sing {
        state::SING
    } else {
        state::STAND
    };
}

/// The high-C sound column of a singing Operettensänger (`None` for every
/// other entity and while he doesn't sing): `hohes_c_width` px wide,
/// centred on him, from his hitbox top straight up, `hohes_c_speed` px
/// taller per frame of the C up to `hohes_c_height`. It passes one-way
/// tiles and ends at the bottom of the first solid tile above him in its
/// columns (above the level, row 0's solid tiles count, as for the player).
/// Any contact hurts, checked before the stomp (D-068).
pub fn high_c_column(e: &Entity, map: &TileMap, t: &Tuning) -> Option<Aabb> {
    let et = &t.enemies;
    if e.kind != EntityKind::Enemy(EnemyKind::Operettensaenger) || e.state != state::SING {
        return None;
    }
    // `count` is already the next frame: the shown frame is one before.
    let cycle = et.operettensaenger_cycle.max(1);
    let f = (e.count + cycle - 1) % cycle;
    let k = i32::from(f.saturating_sub(et.operettensaenger_inhale)) + 1;
    let height = Fx::from_px((et.hohes_c_speed * k).min(et.hohes_c_height));
    let w = Fx::from_px(et.hohes_c_width);
    let x = e.x + e.w.div_int(2) - w.div_int(2);
    let bottom = e.y;
    let (c0, c1) = (tile_of(x), tile_of(x + w - Fx(1)));
    let mut top = bottom - height;
    // The lowest solid tile above his top within reach ends the column.
    let mut row = tile_of(bottom - Fx(1));
    while TILE.mul_int(row + 1) > top {
        if (c0..=c1).any(|c| map.kind_at(c, row) == TileKind::Solid) {
            top = top.max(TILE.mul_int(row + 1));
            break;
        }
        row -= 1;
    }
    (top < bottom).then_some(Aabb {
        x,
        y: top,
        w,
        h: bottom - top,
    })
}

/// Where stand `e`'s globes start (hitbox top left): the
/// `schneekugel_hitbox` with its bottom centre on the stand cell's bottom
/// centre. A globe keeps it as `base_x`/`base_y`, which tells its stand.
fn globe_spawn_point(e: &Entity, t: &Tuning) -> (Fx, Fx) {
    let [w, h] = t.enemies.schneekugel_hitbox;
    let (fx, fy) = e.feet();
    (fx - Fx::from_px(w).div_int(2), fy - Fx::from_px(h))
}

/// A new snow globe from stand `e`, rolling left.
fn new_globe(e: &Entity, t: &Tuning) -> Entity {
    let [w, h] = t.enemies.schneekugel_hitbox;
    let (x, y) = globe_spawn_point(e, t);
    let mut g = Entity::new(
        EntityKind::Enemy(EnemyKind::Schneekugel),
        x,
        y,
        Fx::from_px(w),
        Fx::from_px(h),
    );
    g.dir = -1;
    g.vx = -t.enemies.schneekugel_speed;
    g
}

/// The Souvenirstand spawner (D-069). Its counter (`count`, frame of its
/// `period`) runs only while the player's centre is left of the stand
/// cell's centre, within `range` tiles (horizontally, centre to centre),
/// and the stand cell is fully on screen; otherwise it pauses with its pose
/// kept. On the first such frame it starts at `phase` (0 or ≥ 31). At the
/// start of each cycle (and on that first frame) it counts its globes
/// alive (`UpdateCtx::globes`, by their spawn point): with
/// `souvenirstand_max_alive` of them the cycle is skipped (no telegraph, no
/// globe). Otherwise frames 0..`souvenirstand_telegraph` are the telegraph,
/// and at frame `souvenirstand_telegraph` it returns the globe to spawn.
pub(crate) fn update_souvenirstand(e: &mut Entity, ctx: &UpdateCtx) -> Option<Entity> {
    let t = ctx.tuning;
    let et = &t.enemies;
    let s = e
        .spawn
        .and_then(|i| ctx.level.entities.get(usize::from(i)))?;
    let left_of = ctx.player_cx < e.x + e.w.div_int(2);
    let in_range = player_distance_px(e, ctx) <= i32::from(s.range) * TILE_PX;
    let screen_w = Fx::from_px(crate::camera::SCREEN_W);
    let screen_h = Fx::from_px(crate::camera::SCREEN_H);
    let on_screen = e.x >= ctx.screen_left
        && e.right() <= ctx.screen_left + screen_w
        && e.y >= ctx.screen_top
        && e.y + e.h <= ctx.screen_top + screen_h;
    if !(left_of && in_range && on_screen) {
        return None;
    }
    let period = s.period.max(1);
    let first = e.state == state::WAITING;
    e.count = if first {
        s.phase % period
    } else {
        (e.count + 1) % period
    };
    if first || e.count == 0 {
        let origin = globe_spawn_point(e, t);
        let alive = ctx.globes.iter().filter(|&&g| g == origin).count();
        e.state = if alive >= usize::from(et.souvenirstand_max_alive) {
            state::SKIPPING
        } else {
            state::COUNTING
        };
    }
    (e.state == state::COUNTING && e.count == et.souvenirstand_telegraph).then(|| new_globe(e, t))
}

/// A snow globe rolls left at `schneekugel_speed` with the player's
/// gravity and max fall speed: off ledges, landing on solid and one-way
/// tops (passing one-way tiles from below), never turning. A solid wall
/// shatters it (state [`state::SHATTERED`], removed; the game spawns its
/// shards); water and hazard tiles remove it, and so does its right edge
/// leaving the screen on the left. It ignores enemies and items.
fn update_globe(e: &mut Entity, ctx: &UpdateCtx) {
    let t = ctx.tuning;
    e.vx = -t.enemies.schneekugel_speed;
    e.vy = (e.vy + t.player.gravity).min(t.player.max_fall_speed);
    let mut b = e.hitbox();
    let wall = ctx.map.move_x(&mut b, e.vx);
    if ctx.map.move_y(&mut b, e.vy, false) != YHit::None {
        e.vy = Fx::ZERO;
    }
    e.x = b.x;
    e.y = b.y;
    if wall {
        e.state = state::SHATTERED;
        e.removed = true;
    } else if ctx.map.swallows(&b) || e.right() <= ctx.screen_left {
        e.removed = true;
    }
}

/// The text popup of a defeated enemy, stacked above the score popup
/// whose top is `top` (centred on `x`): „Bravo!" for an Operettensänger
/// (D-068), „Beleidigt!" for a Schwan (D-078, the same mechanism),
/// „Abtreten!" for an Offizier (D-096); `None` for every other kind. Every
/// defeat shows it (stomp, axe, bumped block).
pub(crate) fn defeat_popup(kind: EnemyKind, x: Fx, top: Fx) -> Option<Entity> {
    let p = match kind {
        EnemyKind::Operettensaenger => Popup::Bravo,
        EnemyKind::Schwan => Popup::Beleidigt,
        EnemyKind::Offizier => Popup::Abtreten,
        _ => return None,
    };
    Some(popup(
        p,
        x,
        top - Fx::from_px(crate::entity::POPUP_STACK_PX),
    ))
}

/// A Regattaboot's two contact parts (D-080 refinement, E29): the
/// masthead part (`regattaboot_mast`, centred on the hitbox, from its top
/// down), the only part that bounces a falling player, and the body below
/// it over the whole hitbox width, which hurts on any contact. So a player
/// falling beside the narrow masthead passes it and meets the sail.
/// `None` for every other entity (their hitbox is one part).
pub fn regatta_parts(e: &Entity, t: &Tuning) -> Option<(Aabb, Aabb)> {
    if e.kind != EntityKind::Enemy(EnemyKind::Regattaboot) {
        return None;
    }
    let [w, h] = t.enemies.regattaboot_mast;
    let (w, h) = (Fx::from_px(w), Fx::from_px(h));
    let mast = Aabb {
        x: e.x + (e.w - w).div_int(2),
        y: e.y,
        w,
        h,
    };
    let body = Aabb {
        x: e.x,
        y: e.y + h,
        w: e.w,
        h: e.h - h,
    };
    Some((mast, body))
}

/// What touching an enemy does to the player (`Game::interact`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Contact {
    /// Defeated, the player bounces (points in the stomp chain).
    Stomp,
    /// The player bounces off without harm or points (the Fiaker's,
    /// Reisebus's and Regattaboot's roof, D-027).
    Bounce,
    /// The player is hurt.
    Hurt,
}

/// The contact rule for an enemy the player touches. `from_above`: she is
/// falling and her feet were above the enemy's middle on the previous
/// frame. From above a stompable enemy is stomped and any other bounces
/// her, except a swarm, which hurts on any contact; every other contact
/// hurts. While she swims every contact hurts: no stomps and no roof
/// bounces (D-076, D-078, D-080; a jump-out is airborne from its next
/// frame, so it can still end in a stomp).
pub fn contact(kind: EnemyKind, from_above: bool, swimming: bool) -> Contact {
    if !from_above || swimming || kind.hurts_from_above() {
        Contact::Hurt
    } else if kind.stompable() {
        Contact::Stomp
    } else {
        Contact::Bounce
    }
}

// ------------------------------------------------- M5 (D-078, D-079)

/// True if `e`'s hitbox lies fully inside the camera view (the D-075
/// visibility rule for triggers: nothing in the scroll zone starts off
/// screen; [`UpdateCtx::fully_on_screen`]).
fn fully_on_screen(e: &Entity, ctx: &UpdateCtx) -> bool {
    ctx.fully_on_screen(&e.hitbox())
}

/// The Schwan (D-078). At home (`base_x`) it faces the player; when the
/// player is within `schwan_range` (centre to centre) and it is
/// [`fully_on_screen`] it telegraphs (the hiss) for `schwan_telegraph`
/// frames (`count` = frame, the trigger frame is the first), then charges
/// towards the side the player was on at the telegraph's start at
/// `schwan_charge_speed` for `schwan_charge_frames` frames, paddles home at
/// `schwan_return_speed` (the last step clamped, so it arrives exactly) and
/// rests `schwan_rest` frames (the arrival frame not counted) before it
/// can trigger again. It ignores tiles: it stays at its float height.
fn update_schwan(e: &mut Entity, ctx: &UpdateCtx) {
    let et = &ctx.tuning.enemies;
    let cx = e.x + e.w.div_int(2);
    if matches!(e.state, state::HOME | state::REST) && ctx.player_cx != cx {
        e.dir = (ctx.player_cx - cx).signum() as i8;
    }
    match e.state {
        state::HOME => {
            if player_distance_px(e, ctx) <= et.schwan_range && fully_on_screen(e, ctx) {
                e.state = state::TELEGRAPH;
                e.count = 1;
            }
        }
        state::TELEGRAPH => {
            e.count += 1;
            if e.count > et.schwan_telegraph {
                e.state = state::CHARGE;
                e.count = 0;
            }
        }
        state::REST => {
            e.count += 1;
            if e.count >= et.schwan_rest {
                e.state = state::HOME;
                e.count = 0;
            }
        }
        _ => {}
    }
    e.vx = Fx::ZERO;
    match e.state {
        state::CHARGE => {
            e.vx = et.schwan_charge_speed.mul_int(i32::from(e.dir));
            e.x += e.vx;
            e.count += 1;
            if e.count >= et.schwan_charge_frames {
                e.state = state::PADDLE_BACK;
                e.count = 0;
            }
        }
        state::PADDLE_BACK => {
            let to_home = e.base_x - e.x;
            if to_home != Fx::ZERO {
                e.dir = to_home.signum() as i8;
            }
            let speed = et.schwan_return_speed;
            e.vx = to_home.max(-speed).min(speed);
            e.x += e.vx;
            if e.x == e.base_x {
                e.state = state::REST;
                e.count = 0;
            }
        }
        _ => {}
    }
}

/// The Gelse (D-079). It hovers in place (`count` = frames it has been
/// [`fully_on_screen`] so far) until that reaches `gelse_wait`, then hunts
/// for `gelse_hunt` frames: each frame its velocity changes by
/// `gelse_accel` per axis towards the player's hitbox centre x and her
/// hitbox top + `gelse_target_dy` (seen from its hitbox centre), clamped to
/// ±`gelse_speed`, and it moves clear of water ([`move_clear_of_water`]);
/// then it leaves: harmless, straight up at `gelse_leave_speed`, removed
/// once its bottom is above the screen.
fn update_gelse(e: &mut Entity, ctx: &UpdateCtx) {
    let et = &ctx.tuning.enemies;
    let cx = e.x + e.w.div_int(2);
    match e.state {
        state::HOVER => {
            if ctx.player_cx != cx {
                e.dir = (ctx.player_cx - cx).signum() as i8;
            }
            if fully_on_screen(e, ctx) {
                e.count += 1;
                if e.count >= et.gelse_wait {
                    e.state = state::HUNT;
                    e.count = 0;
                }
            }
        }
        state::HUNT => {
            let cy = e.y + e.h.div_int(2);
            let ty = ctx.player_top + Fx::from_px(et.gelse_target_dy);
            let (speed, accel) = (et.gelse_speed, et.gelse_accel);
            let towards = |v: Fx, d: Fx| (v + accel.mul_int(d.signum())).max(-speed).min(speed);
            e.vx = towards(e.vx, ctx.player_cx - cx);
            e.vy = towards(e.vy, ty - cy);
            move_clear_of_water(e, ctx.map);
            if e.vx != Fx::ZERO {
                e.dir = e.vx.signum() as i8;
            }
            e.count += 1;
            if e.count >= et.gelse_hunt {
                e.state = state::LEAVE;
                e.count = 0;
            }
        }
        _ => {
            e.vx = Fx::ZERO;
            e.vy = -et.gelse_leave_speed;
            e.y += e.vy;
            if e.y + e.h <= ctx.screen_top {
                e.removed = true;
            }
        }
    }
}

/// Moves a Gelse by its velocity through every tile except water, lethal
/// or swim (D-079): a move (per axis, x first) that would put any of its
/// hitbox into such a tile is cut at that tile's edge (a move down at the tile's top)
/// and that velocity set to 0. So it waits at the water line above a
/// diver. Its moves are shorter than a tile, so only the cells it newly
/// enters can be water.
fn move_clear_of_water(e: &mut Entity, map: &TileMap) {
    let wet = |b: &Aabb| map.touches(b, TileKind::Water) || map.touches(b, TileKind::Swim);
    let mut b = e.hitbox();
    b.x += e.vx;
    if e.vx != Fx::ZERO && wet(&b) {
        b.x = if e.vx > Fx::ZERO {
            TILE.mul_int(tile_of(b.right() - Fx(1))) - b.w
        } else {
            TILE.mul_int(tile_of(b.x) + 1)
        };
        e.vx = Fx::ZERO;
    }
    b.y += e.vy;
    if e.vy != Fx::ZERO && wet(&b) {
        b.y = if e.vy > Fx::ZERO {
            TILE.mul_int(tile_of(b.bottom() - Fx(1))) - b.h
        } else {
            TILE.mul_int(tile_of(b.y) + 1)
        };
        e.vy = Fx::ZERO;
    }
    e.x = b.x;
    e.y = b.y;
}

// ---------------------------------------------------------- M6 (D-095)

/// The Kurgast (D-095). Walking (`WALK`, `RESTED`) he is a Nordic-Walker
/// at `kurgast_speed`: towards the player at activation, turning at walls,
/// enemies on foot and ledges. Stomped he lies down ([`lie_down`], `LIE`):
/// his hitbox is the deck chair, a one-way top platform
/// ([`is_platform_now`]), harmless and still. He rests `kurgast_rest`
/// frames, then shows the wake telegraph (`WAKE`) for `kurgast_wake`;
/// **both counters pause while the player stands on the chair**, so he
/// never stands up under her. Then he stands up ([`stand_up`]: feet and
/// centre kept), faces the player and walks on (`RESTED`), harmful again;
/// he takes no step in the frame he stands up.
fn update_kurgast(e: &mut Entity, ctx: &UpdateCtx) {
    let et = &ctx.tuning.enemies;
    let (limit, next) = match e.state {
        state::LIE => (et.kurgast_rest, state::WAKE),
        state::WAKE => (et.kurgast_wake, state::RESTED),
        _ => return walk_turning_at_ledges(e, ctx, et.kurgast_speed),
    };
    let rider = ctx
        .player_standing
        .is_some_and(|p| crate::platform::stands_on(&p, &e.hitbox()));
    if !rider {
        e.count += 1;
    }
    if e.count < limit {
        return;
    }
    e.state = next;
    e.count = 0;
    if next == state::RESTED {
        stand_up(e, ctx.tuning);
        let cx = e.x + e.w.div_int(2);
        if ctx.player_cx != cx {
            e.dir = (ctx.player_cx - cx).signum() as i8;
        }
    }
}

/// Re-sizes `e` to `[w, h]` px around the same centre x, with the same
/// feet.
fn resize_at_feet(e: &mut Entity, [w, h]: [i32; 2]) {
    let (cx, feet) = e.feet();
    e.w = Fx::from_px(w);
    e.h = Fx::from_px(h);
    e.x = cx - e.w.div_int(2);
    e.y = feet - e.h;
}

/// A stomped Kurgast lies down in his deck chair where he stood (D-095):
/// the hitbox becomes `kurgast_chair`, centred on him with its bottom on
/// his feet; the rest starts. Also after a later stomp (no points then,
/// `Game::interact`). The chair never moves (no gravity): he only walks
/// where there is floor (ledge rule) and a bump knocks him off, so in play
/// it always stands on the ground.
pub(crate) fn lie_down(e: &mut Entity, t: &Tuning) {
    resize_at_feet(e, t.enemies.kurgast_chair);
    e.state = state::LIE;
    e.count = 0;
    e.vx = Fx::ZERO;
    e.vy = Fx::ZERO;
}

/// The Kurgast gets up from his chair: `kurgast_hitbox` again, feet and
/// centre kept.
fn stand_up(e: &mut Entity, t: &Tuning) {
    resize_at_feet(e, t.enemies.kurgast_hitbox);
    e.vx = Fx::ZERO;
    e.vy = Fx::ZERO;
}

// ------------------------------------------------------------ M6 (D-096)

/// The Offizier's drill (D-096) on the stage clock: frame f = (stage
/// clock plus his level `phase`) mod `offizier_cycle`, kept in `count`, so every
/// officer with the same `phase` is at the same frame whenever he was
/// activated (a file stays in step). f < `offizier_march`: he marches at
/// `offizier_speed` with the Nordic-Walker turns ([`walk_turning_at_ledges`]: walls,
/// ledges; enemies on foot in [`turn_at_enemies`]); the next
/// `offizier_attention` frames he halts for „Habt Acht!" (the telegraph);
/// the rest of the cycle he halts for „Präsentiert!" with the sabre zone
/// up ([`sabre_zone`]). At activation he faces the player. Halted, he
/// still falls if his floor disappears.
fn update_offizier(e: &mut Entity, ctx: &UpdateCtx) {
    let et = &ctx.tuning.enemies;
    let phase = e
        .spawn
        .and_then(|i| ctx.level.entities.get(usize::from(i)))
        .map_or(0, |s| u32::from(s.phase));
    let cycle = u32::from(et.offizier_cycle.max(1));
    let f = ((ctx.stage_clock % cycle + phase % cycle) % cycle) as u16;
    e.count = f;
    let (march, attention) = (et.offizier_march, et.offizier_attention);
    e.state = if f < march {
        state::WALK
    } else if f < march + attention {
        state::ATTENTION
    } else {
        state::PRESENT
    };
    let speed = if e.state == state::WALK {
        et.offizier_speed
    } else {
        Fx::ZERO
    };
    walk_turning_at_ledges(e, ctx, speed);
}

/// The sabre zone of a presenting Offizier (`None` for every other entity
/// and in the march and „Habt Acht!"): `offizier_sabre` [w, h] px, centred
/// on him, from his hitbox top straight up. Any contact hurts, checked in
/// `Game::interact` before his hitbox and the stomp (the high-C rule,
/// D-068); it is no entity, so axes pass it (D-097).
pub fn sabre_zone(e: &Entity, t: &Tuning) -> Option<Aabb> {
    if e.kind != EntityKind::Enemy(EnemyKind::Offizier) || e.state != state::PRESENT {
        return None;
    }
    let [w, h] = t.enemies.offizier_sabre;
    let (w, h) = (Fx::from_px(w), Fx::from_px(h));
    Some(Aabb {
        x: e.x + e.w.div_int(2) - w.div_int(2),
        y: e.y - h,
        w,
        h,
    })
}

/// **Defeat by projectile** (the one path for the thrown Hackl, D-056,
/// D-060): what a projectile does to entity `e`, by its
/// [`Entity::axe_hit`] class. Every `Defeat` enemy is knocked off (tumbling
/// off the screen) for its base points (`score.stomp_chain[chain_start]`:
/// 100 / 200, outside the stomp chain; 0 for a Kurgast who has scored,
/// [`scores_on_defeat`]); a falling rock (`Shatter`) breaks
/// for `enemies.stein_points` (50). Returns the points, or `None` for no
/// effect (`Bounce`, `Pass`). `Game::defeat_by_projectile` adds the score,
/// the popup (none for 0 points) and the rock's debris.
pub(crate) fn hit_by_projectile(e: &mut Entity, t: &Tuning) -> Option<u32> {
    if !e.active || e.removed {
        return None;
    }
    match (e.axe_hit(), e.kind) {
        (crate::hackl::AxeHit::Defeat, EntityKind::Enemy(kind)) => {
            let points = if scores_on_defeat(e) {
                t.score.stomp_chain[kind.chain_start()]
            } else {
                0
            };
            defeat(e, kind, true, t);
            Some(points)
        }
        (crate::hackl::AxeHit::Shatter, EntityKind::Stein) => {
            e.state = state::SHATTERED;
            e.removed = true;
            Some(t.enemies.stein_points)
        }
        _ => None,
    }
}
// ------------------------------------------------------------ M6 (D-093)

/// What an Ampel shows (D-093; the renderer's `ampel` anims).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AmpelLight {
    /// `rot`: red (also before the player first qualified).
    Red,
    /// `rotgelb`: red and amber together, the telegraph.
    RedAmber,
    /// `gruen`: green, the cars pull out.
    Green,
    /// `gruen_blink`: the four green blinks; `on` is frame 0 (lit).
    GreenBlink {
        /// The green lamp is lit (frame 0 of `gruen_blink`).
        on: bool,
    },
    /// `gelb`: amber.
    Amber,
}

/// The light of Ampel `e` at its counter frame f (`count`): red and amber
/// for f < `ampel_telegraph`, green to the last car's spawn plus a gap
/// (`ampel_telegraph + cars × ampel_gap`), then `ampel_blink` frames of
/// blinks (each an eighth of it on, then an eighth off), `ampel_amber` of
/// amber, and red for the rest of the period. Red while it never counted.
/// `None` for every other entity.
pub fn ampel_light(e: &Entity, level: &Level, t: &Tuning) -> Option<AmpelLight> {
    if e.kind != EntityKind::Ampel {
        return None;
    }
    if e.state == state::WAITING {
        return Some(AmpelLight::Red);
    }
    let et = &t.enemies;
    let cars = e
        .spawn
        .and_then(|i| level.entities.get(usize::from(i)))
        .map_or(0, |s| s.cars);
    let f = e.count;
    let green_end = et.ampel_telegraph + u16::from(cars) * et.ampel_gap;
    let blink_end = green_end + et.ampel_blink;
    Some(if f < et.ampel_telegraph {
        AmpelLight::RedAmber
    } else if f < green_end {
        AmpelLight::Green
    } else if f < blink_end {
        let half = (et.ampel_blink / 8).max(1);
        AmpelLight::GreenBlink {
            on: ((f - green_end) / half).is_multiple_of(2),
        }
    } else if f < blink_end + et.ampel_amber {
        AmpelLight::Amber
    } else {
        AmpelLight::Red
    })
}

/// Where Ampel `e`'s cars start (hitbox top left): `auto_spawn_dx` px into
/// the spawner cell, the `auto_hitbox` bottom on the cell's bottom (the
/// road). A car keeps it as `base_x`/`base_y`, which tells its Ampel.
fn car_spawn_point(e: &Entity, t: &Tuning) -> (Fx, Fx) {
    let et = &t.enemies;
    (
        e.x + Fx::from_px(et.auto_spawn_dx),
        e.y + e.h - Fx::from_px(et.auto_hitbox[1]),
    )
}

/// Car `k` (0-based) of a green from Ampel `e`, driving left. Its `count`
/// is `k`: the renderer's look (`drive_0` … `drive_2`, D-093).
fn new_car(e: &Entity, k: u16, t: &Tuning) -> Entity {
    let [w, h] = t.enemies.auto_hitbox;
    let (x, y) = car_spawn_point(e, t);
    let mut car = Entity::new(
        EntityKind::Enemy(EnemyKind::Auto),
        x,
        y,
        Fx::from_px(w),
        Fx::from_px(h),
    );
    car.dir = -1;
    car.vx = -t.enemies.auto_speed;
    car.count = k;
    car
}

/// The car (0-based, of `cars`) an Ampel sends at its counter frame `f`:
/// at f = `ampel_telegraph` + k × `ampel_gap` for k < `cars`.
fn car_at(f: u16, cars: u8, t: &Tuning) -> Option<u16> {
    let et = &t.enemies;
    let d = f.checked_sub(et.ampel_telegraph)?;
    let gap = et.ampel_gap.max(1);
    (d % gap == 0 && d / gap < u16::from(cars)).then_some(d / gap)
}

/// The lamp housing's top and bottom, px above the spawner cell's bottom
/// (D-102; the lamp housing, rows 0–27 of the 16x48 `ampel` sprite).
const AMPEL_HOUSING_TOP_PX: i32 = 48;
const AMPEL_HOUSING_BOTTOM_PX: i32 = 20;

/// The Ampel's lamp housing (D-102): a box 16 px wide in the spawner's
/// column, from the cell's bottom − 48 px to its bottom − 20 px. It
/// decides whether the Ampel is on screen ([`update_ampel`]), so a camera
/// raised above a refuge sees the light it can see, not the road cell.
pub(crate) fn ampel_housing(e: &Entity) -> Aabb {
    let bottom = e.y + e.h;
    Aabb {
        x: e.x + e.w.div_int(2) - Fx::from_px(TILE_PX / 2),
        y: bottom - Fx::from_px(AMPEL_HOUSING_TOP_PX),
        w: Fx::from_px(TILE_PX),
        h: Fx::from_px(AMPEL_HOUSING_TOP_PX - AMPEL_HOUSING_BOTTOM_PX),
    }
}

/// The Ampel spawner (D-093). Its counter (`count`, the frame f of its
/// `period`) runs only while the player's centre is left of the spawner
/// cell's centre, within `range` tiles (horizontally, centre to centre),
/// and its lamp housing ([`ampel_housing`], D-102) is fully on screen
/// ([`UpdateCtx::fully_on_screen`]); otherwise it pauses with its light
/// kept ([`ampel_light`]). On the first such frame it starts at `phase`. At
/// f = `ampel_telegraph` + k × `ampel_gap` (k < `cars`) it returns car k
/// to spawn, unless `ampel_max_alive` of its cars are alive
/// (`UpdateCtx::cars`, by their spawn point) or the car's hitbox would
/// overlap the player's: then that car is skipped.
pub(crate) fn update_ampel(e: &mut Entity, ctx: &UpdateCtx) -> Option<Entity> {
    let t = ctx.tuning;
    let s = e
        .spawn
        .and_then(|i| ctx.level.entities.get(usize::from(i)))?;
    let left_of = ctx.player_cx < e.x + e.w.div_int(2);
    let in_range = player_distance_px(e, ctx) <= i32::from(s.range) * TILE_PX;
    if !(left_of && in_range && ctx.fully_on_screen(&ampel_housing(e))) {
        return None;
    }
    let period = s.period.max(1);
    e.count = if e.state == state::WAITING {
        s.phase % period
    } else {
        (e.count + 1) % period
    };
    e.state = state::COUNTING;
    let k = car_at(e.count, s.cars, t)?;
    let origin = car_spawn_point(e, t);
    let alive = ctx.cars.iter().filter(|&&c| c == origin).count();
    if alive >= usize::from(t.enemies.ampel_max_alive) {
        return None;
    }
    let car = new_car(e, k, t);
    (!car.hitbox().overlaps(&ctx.player_box)).then_some(car)
}

/// A car (D-093) drives left at `auto_speed`, ignoring tiles, enemies and
/// platforms; it never stops or turns, and is removed once its right edge
/// is left of the screen.
fn update_auto(e: &mut Entity, ctx: &UpdateCtx) {
    e.vx = -ctx.tuning.enemies.auto_speed;
    e.x += e.vx;
    if e.right() <= ctx.screen_left {
        e.removed = true;
    }
}

/// True while an undefeated Oberkellner is in `entities` (D-106, D-107):
/// the `goal_stollen` is armed only when this is false.
// E46: stub until the Oberkellner lands (E46 replaces the body under this
// name; E49 removes this note). No boss exists yet, so a level is always
// armed (a level without a boss is armed from the start). E46/E49: add the
// game-level tests this stub can't have: an unarmed touch raises no
// `LevelComplete` and doesn't suppress a hurt in the same frame.
pub fn boss_undefeated(_entities: &[Entity]) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sine_offset_hits_peaks_and_zeros_exactly() {
        let amp = Fx::from_px(16);
        assert_eq!(sine_offset(0, 120, amp), Fx::ZERO);
        assert_eq!(sine_offset(30, 120, amp), amp);
        assert_eq!(sine_offset(60, 120, amp), Fx::ZERO);
        assert_eq!(sine_offset(90, 120, amp), -amp);
        assert_eq!(sine_offset(120, 120, amp), Fx::ZERO, "periodic");
        // Close to a real sine everywhere (table of 16·sin(2πf/120) in
        // subpixels, precomputed): within 0.2 % of the amplitude.
        for (f, want) in [(10, 2048), (15, 2896), (20, 3547), (45, 2896), (100, -3547)] {
            let got = sine_offset(f, 120, amp).raw();
            assert!((got - want).abs() <= 9, "frame {f}: {got} vs {want}");
        }
        // Odd periods work (half-frame units).
        assert_eq!(sine_offset(0, 7, amp), Fx::ZERO);
        assert!(sine_offset(2, 7, amp) > Fx::ZERO && sine_offset(5, 7, amp) < Fx::ZERO);
    }
}
