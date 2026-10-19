//! Enemy behaviour (`docs/game-design.md` → "Entities and behaviour",
//! Stage 1–4): per-kind update functions for [`EntityKind::Enemy`], the
//! projectiles (thrown Mozartkugel [`EntityKind::Thrown`], Blasmusik note
//! [`EntityKind::Note`]), the Steinschlag spawner and its falling rocks
//! ([`EntityKind::Steinschlag`], [`EntityKind::Stein`]) and defeated
//! enemies ([`EntityKind::Defeated`]),
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
use crate::level::{TILE_PX, TileKind};
use crate::physics::{TILE, TileMap, YHit, tile_of};
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
    /// Stein: shattered (removed; the game spawns its debris).
    pub const SHATTERED: u8 = 1;

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
}

impl Entity {
    /// The pose of an enemy, defeated enemy or thrown Mozartkugel; `None`
    /// for every other kind.
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
            EntityKind::Enemy(EnemyKind::Gams) => match self.state {
                state::CROUCH => Pose::Crouch,
                state::HOP => Pose::Hop,
                _ => Pose::Idle,
            },
            // The niche is empty from the drop until the refill.
            EntityKind::Steinschlag => steinschlag_pose(self, t)?,
            EntityKind::Stein => Pose::Fall,
            EntityKind::Note => Pose::Fly,
            EntityKind::Defeated(EnemyKind::Taube | EnemyKind::TaubeSitting) => Pose::Tumble,
            EntityKind::Defeated(_) => Pose::Squished,
            EntityKind::Thrown => Pose::Spin,
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
    /// crashed E-Biker); true for every other enemy.
    pub fn harmful(&self) -> bool {
        !(self.kind == EntityKind::Enemy(EnemyKind::EBiker) && self.state == state::CRASH)
    }
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
    if e.dir != 0 && standing && !floor_at(map, ahead, feet) {
        e.dir = -e.dir;
    }
    e.state = state::STRIDE;
    e.vx = t.enemies.walker_speed.mul_int(i32::from(e.dir));
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
    if ctx.map.touches_lethal(&b) || e.right() <= ctx.screen_left {
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
        | EntityKind::Enemy(EnemyKind::Pilger) => true,
        EntityKind::Enemy(EnemyKind::Gams) => e.state != state::HOP,
        EntityKind::Enemy(EnemyKind::Fiaker) => e.state != state::CHARGE,
        EntityKind::Enemy(EnemyKind::EBiker) => matches!(e.state, state::PARK | state::RING),
        EntityKind::Enemy(EnemyKind::TaubeSitting) => e.state == state::SIT,
        _ => false,
    }
}

/// Selfie-Tourists, Nordic-Walkers and Pilger that overlap another enemy on foot
/// turn away from it (after everything moved; the direction is set, not
/// flipped, so two that overlap for several frames don't flip back and
/// forth).
pub(crate) fn turn_at_enemies(entities: &mut [Entity]) {
    for i in 0..entities.len() {
        let a = entities[i];
        let walks = matches!(
            a.kind,
            EntityKind::Enemy(
                EnemyKind::SelfieTourist | EnemyKind::NordicWalker | EnemyKind::Pilger
            )
        );
        // `dir == 0`: a `still` tourist (test levels) never turns.
        if !a.active || a.removed || a.dir == 0 || !walks {
            continue;
        }
        let ac = a.x + a.w.div_int(2);
        let other = entities.iter().enumerate().find(|(j, b)| {
            *j != i && b.active && !b.removed && on_foot(b) && b.hitbox().overlaps(&a.hitbox())
        });
        if let Some((_, b)) = other {
            let bc = b.x + b.w.div_int(2);
            let away = (ac - bc).signum() as i8;
            entities[i].dir = if away == 0 { -a.dir } else { away };
        }
    }
}

/// A block at (`col`, `row`) was bumped from below: every stompable enemy
/// standing on it (feet exactly on its top, overlapping the cell) is
/// knocked off (defeated, tumbling). Rolling items standing on it hop.
/// Returns the knocked enemies' kind and feet (for points and popups).
pub(crate) fn knock_off_block(
    entities: &mut [Entity],
    col: i32,
    row: i32,
    t: &Tuning,
) -> Vec<(EnemyKind, Fx, Fx)> {
    let (left, top) = (TILE.mul_int(col), TILE.mul_int(row));
    let mut knocked = Vec::new();
    for e in entities.iter_mut() {
        let standing = e.y + e.h == top && e.x < left + TILE && left < e.right();
        if !e.active || e.removed || !standing {
            continue;
        }
        match e.kind {
            EntityKind::Enemy(kind) if kind.stompable() && on_foot(e) => {
                defeat(e, kind, true, t);
                knocked.push((kind, e.x + e.w.div_int(2), e.y));
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
        let mut d = Entity::new(EntityKind::SteinDebris, cx + dx, cy + dy, half, half);
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

/// **Defeat by projectile** (the entry point for the thrown Hackl, D-056):
/// what a projectile does to entity `e`, for the M3 kinds. A Pilger or a
/// Gams is defeated (tumbling off the screen like a knocked enemy) for its
/// base points (`score.stomp_chain[chain_start]`: 100 / 200, outside the
/// stomp chain); a falling rock shatters for `enemies.stein_points` (50).
/// Returns the points, or `None` if the projectile has no effect on this
/// kind here (other kinds are the caller's). `Game::defeat_by_projectile`
/// adds the score, the popup and the rock's debris.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn hit_by_projectile(e: &mut Entity, t: &Tuning) -> Option<u32> {
    if !e.active || e.removed {
        return None;
    }
    match e.kind {
        EntityKind::Enemy(kind @ (EnemyKind::Pilger | EnemyKind::Gams)) => {
            defeat(e, kind, true, t);
            Some(t.score.stomp_chain[kind.chain_start()])
        }
        EntityKind::Stein => {
            e.state = state::SHATTERED;
            e.removed = true;
            Some(t.enemies.stein_points)
        }
        _ => None,
    }
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
