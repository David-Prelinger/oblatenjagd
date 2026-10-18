//! Enemy behaviour (`docs/game-design.md` → "Entities and behaviour"):
//! per-kind update functions for [`EntityKind::Enemy`], the thrown
//! Mozartkugel ([`EntityKind::Thrown`]) and defeated enemies
//! ([`EntityKind::Defeated`]), plus the rules that need the whole entity
//! list (turning at other enemies, knocking enemies off a bumped block).
//!
//! Everything runs on frame counters from activation (`Entity::timer`,
//! `Entity::count`), never on the RNG (D-017). Numbers are in
//! `content/tuning.toml` `[enemies]`. Distances to the player are
//! horizontal, centre to centre.
//!
//! Per-kind `state` values are the constants in [`state`]; the renderer
//! reads them through [`Entity::pose`].

use crate::entity::{EnemyKind, Entity, EntityKind, UpdateCtx};
use crate::fx::Fx;
use crate::physics::{TILE, TileMap, YHit};
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
    }
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
/// Selfie-Tourists).
fn on_foot(e: &Entity) -> bool {
    match e.kind {
        EntityKind::Enemy(EnemyKind::SelfieTourist)
        | EntityKind::Enemy(EnemyKind::MozartkugelVerkaeufer) => true,
        EntityKind::Enemy(EnemyKind::Fiaker) => e.state != state::CHARGE,
        EntityKind::Enemy(EnemyKind::TaubeSitting) => e.state == state::SIT,
        _ => false,
    }
}

/// Selfie-Tourists that overlap another enemy on foot turn away from it
/// (after everything moved; the direction is set, not flipped, so two
/// that overlap for several frames don't flip back and forth).
pub(crate) fn turn_at_enemies(entities: &mut [Entity]) {
    for i in 0..entities.len() {
        let a = entities[i];
        // `dir == 0`: a `still` tourist (test levels) never turns.
        if !a.active
            || a.removed
            || a.dir == 0
            || a.kind != EntityKind::Enemy(EnemyKind::SelfieTourist)
        {
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
