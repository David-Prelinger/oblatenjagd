//! The Festungsbahn car: a moving one-way platform (`docs/game-design.md`
//! → "Festungsbahn car", D-026).
//!
//! - Level syntax: `{ entity = "festungsbahn", to = [dx, dy] }`; the car
//!   starts left-aligned in its spawn cell with its bottom on the cell's
//!   bottom edge, and `to` is the other end point in tiles relative to that.
//! - Cycle (frame counter `count` from activation, wrapping): wait
//!   `platforms.festungsbahn_wait` frames at the start, travel
//!   `festungsbahn_travel` frames in a straight line to the other end, wait,
//!   travel back. Positions are computed from the start point each frame
//!   (`start + delta · k / travel`), so the car never drifts; `vx`/`vy`
//!   hold its movement in the last frame.
//! - Only its top edge collides, from above (like a one-way tile).
//!   [`land`] runs right after the player's own movement: a player whose
//!   feet crossed the top while not moving up lands on it. [`carry`] runs
//!   when the car moves: a player standing on it moves by exactly the
//!   car's movement this frame (with tile collision), so standing and
//!   walking on it are sub-pixel exact.
//! - Squeezed against a ceiling (the car rises, a solid tile stops the
//!   player), the player stays where the ceiling holds them, the car moves
//!   on, and the player drops through it (one-way) instead of being
//!   crushed. Levels keep [`TRACK_CLEARANCE_PX`] free above the track
//!   (checked by `cargo xtask levels lint` via [`track_problems`]).

use crate::entity::{Entity, EntityKind, UpdateCtx};
use crate::fx::Fx;
use crate::game::Event;
use crate::level::{EntitySpawn, Level, SpawnKind, TileKind};
use crate::physics::{Aabb, TILE, TileMap};
use crate::player::Player;
use crate::tuning::{PlayerTuning, Tuning};

/// Free space above the car's top along the whole track (3 tiles: the
/// Zuckerschock hitbox plus room to jump off).
pub const TRACK_CLEARANCE_PX: i32 = 48;

/// How far along the track the car is `timer` frames after activation:
/// `(k, travel)` means `k / travel` of the way from the start point to
/// the other end.
pub fn track_fraction(timer: u32, t: &Tuning) -> (i32, i32) {
    let wait = u32::from(t.platforms.festungsbahn_wait);
    let travel = u32::from(t.platforms.festungsbahn_travel.max(1));
    let u = timer % (2 * (wait + travel));
    let k = if u < wait {
        0
    } else if u < wait + travel {
        u - wait
    } else if u < 2 * wait + travel {
        travel
    } else {
        travel - (u - 2 * wait - travel)
    };
    (k as i32, travel as i32)
}

/// The car's other end point relative to its start, in subpixels.
pub fn track_delta(spawn: &EntitySpawn) -> (Fx, Fx) {
    let [dx, dy] = spawn.to.unwrap_or([0, 0]);
    (TILE.mul_int(dx), TILE.mul_int(dy))
}

/// The car's hitbox top-left `timer` frames after activation.
fn car_position(start: (Fx, Fx), delta: (Fx, Fx), timer: u32, t: &Tuning) -> (Fx, Fx) {
    let (k, n) = track_fraction(timer, t);
    (
        start.0 + delta.0.mul_int(k).div_int(n),
        start.1 + delta.1.mul_int(k).div_int(n),
    )
}

/// One frame of the car (called from `Entity::update`).
pub(crate) fn update_car(e: &mut Entity, ctx: &UpdateCtx) {
    let delta = e
        .spawn
        .and_then(|i| ctx.level.entities.get(usize::from(i)))
        .map_or((Fx::ZERO, Fx::ZERO), track_delta);
    // `count` is the frame in the cycle (`timer` saturates after ~18 min).
    let p = &ctx.tuning.platforms;
    let cycle = 2 * (u32::from(p.festungsbahn_wait) + u32::from(p.festungsbahn_travel));
    e.count = ((u32::from(e.count) + 1) % cycle) as u16;
    let (x, y) = car_position((e.base_x, e.base_y), delta, u32::from(e.count), ctx.tuning);
    e.vx = x - e.x;
    e.vy = y - e.y;
    e.x = x;
    e.y = y;
}

fn overlaps_x(p: &Aabb, car: &Aabb) -> bool {
    p.x < car.right() && car.x < p.right()
}

/// Puts the player on a car top: no vertical speed, grounded, coyote time,
/// stomp chain reset, `Landed` if airborne before.
fn stand(p: &mut Player, top: Fx, was_on_ground: bool, t: &PlayerTuning, events: &mut Vec<Event>) {
    p.y = top - p.h;
    p.vy = Fx::ZERO;
    p.on_ground = true;
    p.jump_rising = false;
    p.coyote = t.coyote_frames;
    p.stomp_chain = 0;
    if !was_on_ground {
        events.push(Event::Landed);
    }
}

/// After the player's own movement: if the feet crossed (or reached) a
/// car's top this frame while not moving up, the player lands on it.
/// `prev_bottom` is the feet height before the player's movement,
/// `was_on_ground` the grounded flag before it. The first car in entity
/// order wins.
pub(crate) fn land(
    p: &mut Player,
    entities: &[Entity],
    prev_bottom: Fx,
    was_on_ground: bool,
    t: &PlayerTuning,
    events: &mut Vec<Event>,
) {
    if p.vy < Fx::ZERO {
        return;
    }
    let hb = p.hitbox();
    let car = entities.iter().find(|c| {
        c.kind == EntityKind::Festungsbahn
            && c.active
            && !c.removed
            && prev_bottom <= c.y
            && hb.bottom() >= c.y
            && overlaps_x(&hb, &c.hitbox())
    });
    if let Some(c) = car {
        // Already on the ground (a tile caught the feet this frame): its
        // `Landed` was reported, don't report a second one.
        let grounded = was_on_ground || p.on_ground;
        stand(p, c.y, grounded, t, events);
    }
}

/// The car `car` just moved from `old`: a player standing on it moves by
/// exactly the same amount (X then Y, with tile collision). A car rising
/// into a player who is not moving up (feet at or above the old top, below
/// the new one) lifts the player onto its top.
pub(crate) fn carry(
    p: &mut Player,
    old: Aabb,
    car: &Entity,
    map: &TileMap,
    screen_left: Fx,
    t: &PlayerTuning,
    events: &mut Vec<Event>,
) {
    let mut b = p.hitbox();
    let feet = b.bottom();
    if p.on_ground && feet == old.y && overlaps_x(&b, &old) {
        map.move_x(&mut b, car.x - old.x);
        // Like the player's own movement: never left of the screen.
        b.x = b.x.max(screen_left);
        map.move_y(&mut b, car.y - old.y, false);
        p.x = b.x;
        p.y = b.y;
    } else if p.vy >= Fx::ZERO
        && feet <= old.y
        && feet > car.y
        && overlaps_x(&b, &car.hitbox())
        && map.move_y(&mut b, car.y - feet, false) == crate::physics::YHit::None
    {
        let was_on_ground = p.on_ground;
        stand(p, car.y, was_on_ground, t, events);
    }
}

/// Problems of every Festungsbahn's track in `level`: the car's top must
/// have [`TRACK_CLEARANCE_PX`] free of solid tiles above it at every
/// frame of its travel. Empty if fine.
pub fn track_problems(level: &Level, t: &Tuning) -> Vec<String> {
    let map = TileMap::from_level(level);
    let mut problems = Vec::new();
    for (i, s) in level.entities.iter().enumerate() {
        if s.kind != SpawnKind::Festungsbahn {
            continue;
        }
        let Some(car) = Entity::from_spawn(s, i, t) else {
            continue;
        };
        let delta = track_delta(s);
        let travel = i32::from(t.platforms.festungsbahn_travel.max(1));
        for k in 0..=travel {
            let x = car.x + delta.0.mul_int(k).div_int(travel);
            let y = car.y + delta.1.mul_int(k).div_int(travel);
            let above = Aabb {
                x,
                y: y - Fx::from_px(TRACK_CLEARANCE_PX),
                w: car.w,
                h: Fx::from_px(TRACK_CLEARANCE_PX),
            };
            if map.touches(&above, TileKind::Solid) {
                problems.push(format!(
                    "festungsbahn at row {}, column {}: less than {} px free above the car at ({}, {}) px ({} of {} frames along the track); keep 3 tiles above the whole track free",
                    s.row + 1,
                    s.col + 1,
                    TRACK_CLEARANCE_PX,
                    x.to_px(),
                    y.to_px(),
                    k,
                    travel
                ));
                break;
            }
        }
    }
    problems
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tuning() -> Tuning {
        Tuning::parse(include_str!("../../../content/tuning.toml")).unwrap()
    }

    #[test]
    fn track_cycle_waits_travels_waits_and_returns() {
        let t = tuning();
        let f = |timer| track_fraction(timer, &t).0;
        assert_eq!(track_fraction(0, &t), (0, 150));
        assert_eq!(f(59), 0, "waits 60 frames at the start");
        assert_eq!(f(60), 0);
        assert_eq!(f(61), 1, "then moves");
        assert_eq!(f(210), 150, "arrives after 150 frames of travel");
        assert_eq!(f(270), 150, "waits 60 frames at the end");
        assert_eq!(f(271), 149, "comes back");
        assert_eq!(f(420), 0, "home after 420 frames");
        assert_eq!(f(421), 0, "the cycle repeats");
        assert_eq!(f(420 + 211), 150);
    }
}
