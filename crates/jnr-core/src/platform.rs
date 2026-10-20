//! One-way platforms: the Festungsbahn car (`docs/game-design.md`
//! → "Festungsbahn car", D-026), the Zwölferhorn Gondel (D-044), the
//! Schafbergbahn (D-067), the Wolfgangsee boats (D-077) and Strobl's
//! Fensterläden (D-094).
//!
//! The **Fensterladen** (M6, `{ entity = "fensterladen", period, open,
//! phase }`) never moves: a `platforms.fensterladen_size` body (32×8)
//! left-aligned in its cell with its top on the cell's top edge. Its
//! cycle runs on **the stage clock** (`Game::stage_clock`, the frames the
//! ZEIT timer counts since the stage start or the last respawn): frame
//! f = (clock + `phase`) mod `period` ([`shutter_frame`]); it is open for
//! f < `open` and closed for the rest ([`shutter`]). `Game` sets every
//! shutter's `count` = f and `state` when the clock ticks
//! ([`place_shutter`]), so all shutters keep an exact relative phase however
//! the player approaches (the Gondel B/C lesson, D-052). Only an open
//! shutter's top lands and carries the player ([`carries`]); a closing one
//! simply stops holding her (she falls with her coyote frames, nothing is
//! pushed or crushed), and an opening one never lifts a player whose feet
//! are already below its top (the landing rule needs her feet to cross the
//! top from above). The renderer's looks come from [`shutter_look`] (open,
//! rattle, closed, warn; the rattle shake is drawn only). It shares
//! never-despawn and the clearance and recovery lints (a zero-length track).
//!
//! The boats (M5): the **Traundl** (`{ entity = "traundl", to = [dx, 0],
//! phase = N }`, 2 ≤ |dx| ≤ 8) is the Gondel's free-running ping-pong with
//! its own hull and timing ([`free_timing`]: `traundl_size`,
//! `traundl_wait`, `traundl_frames_per_tile`). The **Dampfer**
//! (`{ entity = "dampfer", to = [dx, 0] }`, 8 ≤ dx ≤ 64) is the
//! Festungsbahn's waiting machine **one way** ([`CarTiming::moor`]): it
//! waits for its rider, travels dx × `dampfer_frames_per_tile` frames and
//! then stays [`car::MOORED`] until the level resets. While it travels its
//! paddle wheel hurts the player ([`paddle_wheel`]). Both are placed like
//! the car (left-aligned in the spawn cell, bottom on the cell's bottom =
//! the water line) and share landing, carrying, crush, never-despawn and
//! the lints; the pit-recovery lint skips columns over water (a fall is a
//! swim, [`recovery_problems`]).
//!
//! The Schafbergbahn (`{ entity = "schafbergbahn", to = [dx, dy] }`,
//! 4 ≤ |dx| ≤ 32) is the Festungsbahn's waiting state machine with its own
//! numbers ([`car_timing`]): a `platforms.schafbergbahn_size` body (whole
//! tiles, so with whole-tile `to` both roofs lie on the tile grid and sit
//! flush with the station platforms), travel = max(|dx|, |dy|) ×
//! `schafbergbahn_frames_per_tile`, `schafbergbahn_wait` frames at the top.
//! Its lint covers the **swept body** ([`track_problems`]): no solid or
//! one-way tile may overlap the body anywhere along the track, nor the
//! [`TRACK_CLEARANCE_PX`] above its roof.
//!
//! The Gondel (`{ entity = "gondel", to = [dx, dy], phase = N }`) is placed
//! like the car (left-aligned in its spawn cell, bottom on the cell's
//! bottom edge; `|dx|` ≤ 8) and free-running: it never waits for a rider
//! ([`gondel_progress`]: wait `gondel_wait` frames at the spawn end, travel
//! to the other end at 1 px/frame along the major axis, wait, travel back,
//! repeat; `count` = frame in that cycle, starting at `phase`). Landing,
//! carrying and the crush rule below are the same code for both
//! ([`crate::entity::EntityKind::is_platform`]); so are the lints.
//!
//! The Festungsbahn car:
//!
//! - Level syntax: `{ entity = "festungsbahn", to = [dx, dy] }`; the car
//!   starts left-aligned in its spawn cell with its bottom on the cell's
//!   bottom edge, and `to` is the other end point in tiles relative to that.
//! - It waits for its rider (D-030), a state machine in `Entity::state`
//!   ([`car`]) with the frame counter `count`: at its lower end it waits,
//!   for as long as it takes, until the player stands on it; then it
//!   travels up (`platforms.festungsbahn_travel` frames, a straight line),
//!   waits `festungsbahn_wait` frames at the upper end, travels back down
//!   and waits for the next rider. The lower end is the one with the
//!   larger y (`to`'s dy sign; a level track's lower end is its spawn
//!   cell). A car spawned at its upper end first travels down. Positions
//!   are computed from the start point each frame
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
use crate::level::{EntitySpawn, Level, SpawnKind, TILE_PX, TileKind};
use crate::physics::{Aabb, TILE, TileMap, tile_of};
use crate::player::Player;
use crate::tuning::{PlayerTuning, Tuning};

/// Free space above the car's top along the whole track (3 tiles: the
/// Zuckerschock hitbox plus room to jump off).
pub const TRACK_CLEARANCE_PX: i32 = 48;

/// The car's states (`Entity::state`); `count` is the frame in the state.
pub mod car {
    /// At the lower end, waiting for a rider (`count` stays 0).
    pub const WAIT_LOWER: u8 = 0;
    /// Travelling up: `count` frames (1..=travel) from the lower end.
    pub const UP: u8 = 1;
    /// At the upper end: `count` frames (1..=wait) of waiting.
    pub const WAIT_UPPER: u8 = 2;
    /// Travelling down: `count` frames (1..=travel) from the upper end.
    pub const DOWN: u8 = 3;
    /// Arrived at the far end for good (a one-way car, the Dampfer, D-077):
    /// it stays there until the level resets (`count` stays 0).
    pub const MOORED: u8 = 4;
}

/// A Fensterladen's states (`Entity::state`, D-094); `count` is its frame
/// f in its cycle on the stage clock ([`shutter_frame`]).
pub mod shutter {
    /// Folded up against the wall: no collision.
    pub const CLOSED: u8 = 0;
    /// Folded out: a one-way top.
    pub const OPEN: u8 = 1;
}

/// A Fensterladen's frame in its cycle at stage clock `clock`:
/// (clock + `phase`) mod `period` (D-094).
pub fn shutter_frame(spawn: &EntitySpawn, clock: u32) -> u16 {
    let period = u32::from(spawn.period.max(1));
    ((clock % period + u32::from(spawn.phase)) % period) as u16
}

/// Sets a Fensterladen's `count` (its frame f) and `state` (open for
/// f < `open`) for stage clock `clock`. The body never moves.
pub(crate) fn place_shutter(e: &mut Entity, spawn: &EntitySpawn, clock: u32) {
    e.count = shutter_frame(spawn, clock);
    e.state = if e.count < spawn.open {
        shutter::OPEN
    } else {
        shutter::CLOSED
    };
}

/// What the renderer shows of a Fensterladen (D-094, sprite-catalog anims
/// `open`, `rattle`, `closed`, `warn`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShutterLook {
    /// Open, a ledge.
    Open,
    /// The last `platforms.fensterladen_rattle` open frames: the closing
    /// telegraph (the body still carries her).
    Rattle,
    /// Closed, no ledge.
    Closed,
    /// The last `platforms.fensterladen_warn` closed frames: the opening
    /// telegraph (still no collision).
    Warn,
}

/// The look of a Fensterladen at frame `f` of its cycle (`period`,
/// `open` from its spawn): open → rattle → closed → warn.
pub fn shutter_look_at(f: u16, period: u16, open: u16, t: &Tuning) -> ShutterLook {
    let p = &t.platforms;
    if f < open {
        if f + p.fensterladen_rattle >= open {
            ShutterLook::Rattle
        } else {
            ShutterLook::Open
        }
    } else if f + p.fensterladen_warn >= period {
        ShutterLook::Warn
    } else {
        ShutterLook::Closed
    }
}

/// The look of the Fensterladen `e` (from its `count`, which follows the
/// stage clock); `None` for other entities.
pub fn shutter_look(e: &Entity, level: &Level, t: &Tuning) -> Option<ShutterLook> {
    if e.kind != EntityKind::Fensterladen {
        return None;
    }
    let spawn = level.entities.get(usize::from(e.spawn?))?;
    Some(shutter_look_at(e.count, spawn.period, spawn.open, t))
}

/// True if `e`'s top holds the player now: every active platform (a
/// Fensterladen only while open, D-094) and a Kurgast in his deck chair
/// (`enemies::is_platform_now`, D-095).
pub fn carries(e: &Entity) -> bool {
    (e.kind.is_platform() || crate::enemies::is_platform_now(e))
        && e.active
        && !e.removed
        && (e.kind != EntityKind::Fensterladen || e.state == shutter::OPEN)
}

/// Whether the car's spawn cell is the upper end of its track (`to` goes
/// down). A level track starts at its lower end.
pub fn starts_at_upper_end(spawn: &EntitySpawn) -> bool {
    spawn.to.is_some_and(|[_, dy]| dy > 0)
}

/// The car's state at spawn: waiting at the lower end, or travelling
/// down when it was placed at the upper end.
pub(crate) fn initial_state(spawn: &EntitySpawn) -> u8 {
    if starts_at_upper_end(spawn) {
        car::DOWN
    } else {
        car::WAIT_LOWER
    }
}

/// The timing of a moving platform: frames from one end to the other, and
/// frames standing at an end (a waiting car: at its upper end; a
/// free-running one: at each end).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CarTiming {
    /// Frames of travel between the ends (at least 1).
    pub travel: u16,
    /// Frames standing at the upper end (free-running: at each end).
    pub wait: u16,
    /// One way: a waiting car that stays [`car::MOORED`] at the far end
    /// instead of waiting and coming back (the Dampfer, D-077).
    pub moor: bool,
}

/// The timing of a waiting car of `kind` spawned from `spawn`: the
/// Festungsbahn's fixed `festungsbahn_travel` / `festungsbahn_wait`, the
/// Schafbergbahn's [`schafbergbahn_travel`] / `schafbergbahn_wait`, or the
/// Dampfer's [`dampfer_travel`], one way.
pub fn car_timing(kind: EntityKind, spawn: Option<&EntitySpawn>, t: &Tuning) -> CarTiming {
    let p = &t.platforms;
    match kind {
        EntityKind::Schafbergbahn => CarTiming {
            travel: spawn.map_or(1, |s| schafbergbahn_travel(s, t)),
            wait: p.schafbergbahn_wait,
            moor: false,
        },
        EntityKind::Dampfer => CarTiming {
            travel: spawn.map_or(1, |s| dampfer_travel(s, t)),
            wait: 0,
            moor: true,
        },
        _ => CarTiming {
            travel: p.festungsbahn_travel.max(1),
            wait: p.festungsbahn_wait,
            moor: false,
        },
    }
}

/// The state machine's next `(state, count)` after `(state, count)`;
/// `rider` = the player stands on the car this frame.
pub fn next_state(state: u8, count: u16, rider: bool, timing: CarTiming) -> (u8, u16) {
    let CarTiming { travel, wait, moor } = timing;
    let travel = travel.max(1);
    match state {
        car::MOORED => (car::MOORED, 0),
        car::UP if count >= travel && moor => (car::MOORED, 0),
        car::UP if count >= travel => {
            if wait == 0 {
                (car::DOWN, 1)
            } else {
                (car::WAIT_UPPER, 1)
            }
        }
        car::UP => (car::UP, count + 1),
        car::WAIT_UPPER if count >= wait => (car::DOWN, 1),
        car::WAIT_UPPER => (car::WAIT_UPPER, count + 1),
        car::DOWN if count >= travel => (car::WAIT_LOWER, 0),
        car::DOWN => (car::DOWN, count + 1),
        _ if rider => (car::UP, 1),
        _ => (car::WAIT_LOWER, 0),
    }
}

/// How far up the track the car is in `(state, count)`: `(k, travel)`
/// means `k / travel` of the way from the lower end to the upper end.
pub fn height_fraction(state: u8, count: u16, timing: CarTiming) -> (i32, i32) {
    let travel = i32::from(timing.travel.max(1));
    let count = i32::from(count).min(travel);
    let k = match state {
        car::UP => count,
        car::WAIT_UPPER | car::MOORED => travel,
        car::DOWN => travel - count,
        _ => 0,
    };
    (k, travel)
}

/// The car's other end point relative to its start, in subpixels.
pub fn track_delta(spawn: &EntitySpawn) -> (Fx, Fx) {
    let [dx, dy] = spawn.to.unwrap_or([0, 0]);
    (TILE.mul_int(dx), TILE.mul_int(dy))
}

/// Travel frames along a track of `spawn` at `frames_per_tile`:
/// max(|dx|, |dy|) tiles × that (at least 1).
fn per_tile_travel(spawn: &EntitySpawn, frames_per_tile: u16) -> u16 {
    let [dx, dy] = spawn.to.unwrap_or([0, 0]);
    let tiles = dx.unsigned_abs().max(dy.unsigned_abs()) as u16;
    (tiles * frames_per_tile).max(1)
}

/// Travel frames of a Gondel from one end to the other: max(|dx|, |dy|)
/// tiles × `gondel_frames_per_tile` (at least 1).
pub fn gondel_travel(spawn: &EntitySpawn, t: &Tuning) -> u16 {
    per_tile_travel(spawn, t.platforms.gondel_frames_per_tile)
}

/// Travel frames of the Dampfer from its spawn to its mooring: dx tiles ×
/// `dampfer_frames_per_tile` (at least 1; Stage 6's 40 tiles: 800 frames).
pub fn dampfer_travel(spawn: &EntitySpawn, t: &Tuning) -> u16 {
    per_tile_travel(spawn, t.platforms.dampfer_frames_per_tile)
}

/// The timing of a free-running platform of `kind` (Gondel, Traundl):
/// travel = max(|dx|, |dy|) tiles × its frames per tile, and its wait at
/// each end (it never waits for a rider).
pub fn free_timing(kind: EntityKind, spawn: &EntitySpawn, t: &Tuning) -> CarTiming {
    let p = &t.platforms;
    let (wait, frames_per_tile) = match kind {
        EntityKind::Traundl => (p.traundl_wait, p.traundl_frames_per_tile),
        _ => (p.gondel_wait, p.gondel_frames_per_tile),
    };
    CarTiming {
        travel: per_tile_travel(spawn, frames_per_tile),
        wait,
        moor: false,
    }
}

/// Frames of a free-running platform's whole cycle: 2 × (wait + travel).
pub fn free_period(kind: EntityKind, spawn: &EntitySpawn, t: &Tuning) -> u16 {
    let CarTiming { travel, wait, .. } = free_timing(kind, spawn, t);
    2 * (wait + travel)
}

/// Travel frames of a Schafbergbahn from one end to the other:
/// max(|dx|, |dy|) tiles × `schafbergbahn_frames_per_tile` (at least 1;
/// Stage 5's 24-tile track: 576 frames).
pub fn schafbergbahn_travel(spawn: &EntitySpawn, t: &Tuning) -> u16 {
    per_tile_travel(spawn, t.platforms.schafbergbahn_frames_per_tile)
}

/// Frames of a Gondel's whole cycle: 2 × (wait + travel).
pub fn gondel_period(spawn: &EntitySpawn, t: &Tuning) -> u16 {
    free_period(EntityKind::Gondel, spawn, t)
}

/// How far along its track (frames of travel from the spawn end, 0..=
/// `travel`) a Gondel is `k` frames into its cycle: frames 0..wait at the
/// spawn end, then one frame of travel per frame up to `travel` (arriving
/// on frame wait + travel − 1), wait frames at the other end, then back;
/// it arrives at the spawn end on the cycle's last frame. So each end
/// shows for wait + 1 frames, like the Festungsbahn's upper end.
pub fn gondel_progress(k: u16, travel: u16, wait: u16) -> u16 {
    let travel = travel.max(1);
    if k < wait {
        0
    } else if k < wait + travel {
        k - wait + 1
    } else if k < 2 * wait + travel {
        travel
    } else {
        travel.saturating_sub(k - 2 * wait - travel + 1)
    }
}

/// Puts a free-running platform (Gondel, Traundl) where its `count` says,
/// recomputed from the spawn end (`base_x`/`base_y`), so it never drifts;
/// `vx`/`vy` hold the movement.
pub(crate) fn place_free(e: &mut Entity, spawn: &EntitySpawn, t: &Tuning) {
    let CarTiming { travel, wait, .. } = free_timing(e.kind, spawn, t);
    let k = i32::from(gondel_progress(e.count, travel, wait));
    let n = i32::from(travel);
    let (dx, dy) = track_delta(spawn);
    let x = e.base_x + dx.mul_int(k).div_int(n);
    let y = e.base_y + dy.mul_int(k).div_int(n);
    e.vx = x - e.x;
    e.vy = y - e.y;
    e.x = x;
    e.y = y;
}

/// One frame of a free-running platform (Gondel, Traundl; called from
/// `Entity::update`).
pub(crate) fn update_free(e: &mut Entity, ctx: &UpdateCtx) {
    let Some(spawn) = e.spawn.and_then(|i| ctx.level.entities.get(usize::from(i))) else {
        return;
    };
    e.count = (e.count + 1) % free_period(e.kind, spawn, ctx.tuning);
    place_free(e, spawn, ctx.tuning);
}

/// The Dampfer's paddle wheel (D-077): while it travels, a
/// `dampfer_wheel_zone` rectangle centred on the hull's centre x, its top
/// `dampfer_wheel_rise` px above the hull's bottom (the water line), the
/// rest below it. It hurts the player on contact like an enemy that can't
/// be stomped (`Game::interact`); a rider on the deck never reaches it.
/// `None` while the Dampfer waits or is moored (the wheel is still) and for
/// every other entity. Not an entity: the axe can't meet it.
pub fn paddle_wheel(e: &Entity, t: &Tuning) -> Option<Aabb> {
    if e.kind != EntityKind::Dampfer || e.state != car::UP || !e.active || e.removed {
        return None;
    }
    let p = &t.platforms;
    let [w, h] = p.dampfer_wheel_zone;
    let (w, h) = (Fx::from_px(w), Fx::from_px(h));
    let (cx, bottom) = e.feet();
    Some(Aabb {
        x: cx - w.div_int(2),
        y: bottom - Fx::from_px(p.dampfer_wheel_rise),
        w,
        h,
    })
}

/// Both end points of a moving platform's track as the feet (bottom centre
/// of the hitbox) in subpixels: the spawn end first. `None` for other
/// entities. A Fensterladen gets two equal ends (a zero-length track).
/// The renderer draws the Gondel's cable from them.
pub fn track_ends(e: &Entity, level: &Level) -> Option<[(Fx, Fx); 2]> {
    if !e.kind.is_platform() {
        return None;
    }
    let spawn = level.entities.get(usize::from(e.spawn?))?;
    let (dx, dy) = track_delta(spawn);
    let start = (e.base_x + e.w.div_int(2), e.base_y + e.h);
    Some([start, (start.0 + dx, start.1 + dy)])
}

/// The rail's rise in whole px `u` px along a track of `dx`, `dy` px
/// (`dx` ≠ 0), measured from the spawn end: −floor(u · (−dy) / dx), so a
/// 1:4 track rising to the right steps up 1 px every 4 px and reaches `dy`
/// exactly at `u = dx`. The Schafbergbahn's roof follows it pixel for pixel
/// ([`track_offset`]) and the renderer draws the rails with it, so the
/// wheels never leave the rail (pixel-artist, M4).
pub fn rail_offset(u: i32, dx: i32, dy: i32) -> i32 {
    let (mut num, mut den) = (u * -dy, dx);
    if den < 0 {
        num = -num;
        den = -den;
    }
    -num.div_euclid(den.max(1))
}

/// The offset of a moving platform of `kind` from its spawn point `k` of
/// `n` frames along its track `delta`: the straight line delta · k / n;
/// for the Schafbergbahn (when it moves across) the height is snapped to
/// whole pixels on the rail ([`rail_offset`] of the whole-px x offset), so
/// it stays exactly on the track's 4:1 ratio on every frame.
pub fn track_offset(kind: SpawnKind, delta: (Fx, Fx), k: i32, n: i32) -> (Fx, Fx) {
    let x = delta.0.mul_int(k).div_int(n);
    if kind == SpawnKind::Schafbergbahn && delta.0 != Fx::ZERO {
        let rise = rail_offset(x.to_px(), delta.0.to_px(), delta.1.to_px());
        return (x, Fx::from_px(rise));
    }
    (x, delta.1.mul_int(k).div_int(n))
}

/// Where a waiting car (Festungsbahn, Schafbergbahn, Dampfer) spawned from `spawn`
/// at `(base_x, base_y)` is in `(state, count)`: its hitbox top-left,
/// recomputed from the spawn point ([`track_offset`]), so it never drifts.
/// The renderer uses it for the steam puffs left behind.
pub fn car_position(
    kind: EntityKind,
    base: (Fx, Fx),
    spawn: Option<&EntitySpawn>,
    state: u8,
    count: u16,
    timing: CarTiming,
) -> (Fx, Fx) {
    let delta = spawn.map_or((Fx::ZERO, Fx::ZERO), track_delta);
    let from_upper = spawn.is_some_and(starts_at_upper_end);
    let (k, n) = height_fraction(state, count, timing);
    // `k` counts from the lower end, the position from the spawn point.
    let k = if from_upper { n - k } else { k };
    let kind = match kind {
        EntityKind::Schafbergbahn => SpawnKind::Schafbergbahn,
        _ => SpawnKind::Festungsbahn,
    };
    let (dx, dy) = track_offset(kind, delta, k, n);
    (base.0 + dx, base.1 + dy)
}

/// One frame of the car (called from `Entity::update`).
pub(crate) fn update_car(e: &mut Entity, ctx: &UpdateCtx) {
    let spawn = e.spawn.and_then(|i| ctx.level.entities.get(usize::from(i)));
    let timing = car_timing(e.kind, spawn, ctx.tuning);
    let rider = ctx
        .player_standing
        .is_some_and(|p| stands_on(&p, &e.hitbox()));
    (e.state, e.count) = next_state(e.state, e.count, rider, timing);
    let (x, y) = car_position(
        e.kind,
        (e.base_x, e.base_y),
        spawn,
        e.state,
        e.count,
        timing,
    );
    e.vx = x - e.x;
    e.vy = y - e.y;
    e.x = x;
    e.y = y;
}

/// A grounded player with hitbox `p` stands on the car with hitbox `c`:
/// feet exactly on its top, overlapping it horizontally.
pub(crate) fn stands_on(p: &Aabb, c: &Aabb) -> bool {
    p.bottom() == c.y && overlaps_x(p, c)
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
///
/// "Car" is anything with a one-way top: the moving platforms
/// ([`EntityKind::is_platform`]) and, through the **enemy-as-platform
/// hook** [`crate::enemies::is_platform_now`], an enemy that is a platform
/// right now (a Kurgast in his deck chair, D-095). Standing on a still
/// platform is this landing every frame (gravity pulls her 1 step down,
/// the top catches her), so an enemy platform, which never moves, needs
/// no [`carry`].
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
        carries(c) && prev_bottom <= c.y && hb.bottom() >= c.y && overlaps_x(&hb, &c.hitbox())
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
///
/// Unlike the player's own movement the carry is not stopped at the
/// screen's left edge: the camera follows a rider carried past it
/// (D-051, `Game::follow_carried_left`); in an auto-scroll the left edge
/// pushes her back instead (D-075). Moving right, the carry stops like at
/// a wall where her hitbox's right edge reaches `max_x` (the scroll's
/// right clamp; `Fx::MAX` elsewhere), and never moves her left for it.
pub(crate) fn carry(
    p: &mut Player,
    old: Aabb,
    car: &Entity,
    map: &TileMap,
    max_x: Fx,
    t: &PlayerTuning,
    events: &mut Vec<Event>,
) {
    let mut b = p.hitbox();
    let feet = b.bottom();
    if p.on_ground && stands_on(&b, &old) {
        map.move_x(&mut b, car.x - old.x);
        if b.right() > max_x && b.x > p.x {
            b.x = (max_x - b.w).max(p.x);
        }
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

/// A platform's track for the lints: its hitbox at the spawn end, the
/// delta to the other end and the travel frames (a Fensterladen: a
/// zero-length track of one frame). `None` for other spawns.
fn track(s: &EntitySpawn, i: usize, t: &Tuning) -> Option<(Aabb, (Fx, Fx), i32)> {
    let travel = match s.kind {
        SpawnKind::Fensterladen => 1,
        SpawnKind::Festungsbahn => t.platforms.festungsbahn_travel.max(1),
        SpawnKind::Gondel => gondel_travel(s, t),
        SpawnKind::Schafbergbahn => schafbergbahn_travel(s, t),
        SpawnKind::Traundl => free_timing(EntityKind::Traundl, s, t).travel,
        SpawnKind::Dampfer => dampfer_travel(s, t),
        _ => return None,
    };
    let car = Entity::from_spawn(s, i, t)?;
    let start = Aabb {
        x: car.base_x,
        y: car.base_y,
        w: car.w,
        h: car.h,
    };
    Some((start, track_delta(s), i32::from(travel)))
}

/// Problems of every moving platform's track in `level`: at every frame of
/// its travel, the Festungsbahn's and the Gondel's top must have
/// [`TRACK_CLEARANCE_PX`] free of solid tiles above it; the Schafbergbahn's
/// **swept body** and the [`TRACK_CLEARANCE_PX`] above its roof must be
/// free of solid and one-way tiles (D-067: a 64×32 box moving up a slope
/// sweeps a parallelogram that stepped ground under the track could cut).
/// A Fensterladen is a zero-length track: 3 tiles of air above its body
/// (D-094). Empty if fine.
pub fn track_problems(level: &Level, t: &Tuning) -> Vec<String> {
    let map = TileMap::from_level(level);
    let mut problems = Vec::new();
    for (i, s) in level.entities.iter().enumerate() {
        let Some((car, delta, travel)) = track(s, i, t) else {
            continue;
        };
        let swept_body = s.kind == SpawnKind::Schafbergbahn;
        for k in 0..=travel {
            let (ox, oy) = track_offset(s.kind, delta, k, travel);
            let (x, y) = (car.x + ox, car.y + oy);
            let clearance = Fx::from_px(TRACK_CLEARANCE_PX);
            let mut free = Aabb {
                x,
                y: y - clearance,
                w: car.w,
                h: clearance,
            };
            if swept_body {
                free.h = clearance + car.h;
                let (x_px, y_px) = (x.to_px(), y.to_px());
                if map.touches(&free, TileKind::Solid) || map.touches(&free, TileKind::OneWay) {
                    problems.push(format!(
                        "{} at row {}, column {}: a tile overlaps the train's body or the {} px above its roof at ({}, {}) px ({} of {} frames along the track); keep the swept body and 3 tiles above it free of solid and one-way tiles",
                        s.kind.name(),
                        s.row + 1,
                        s.col + 1,
                        TRACK_CLEARANCE_PX,
                        x_px,
                        y_px,
                        k,
                        travel
                    ));
                    break;
                }
                continue;
            }
            let above = free;
            if map.touches(&above, TileKind::Solid) {
                problems.push(format!(
                    "{} at row {}, column {}: less than {} px free above the car at ({}, {}) px ({} of {} frames along the track); keep 3 tiles above the whole track free",
                    s.kind.name(),
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

/// Highest step-up (tiles) the way back out of the pit under a track may
/// need: a walking jump reaches 4 tiles, minus one tile of safety margin.
pub const MAX_STEP_UP_TILES: i32 = 3;

/// Problems of every platform (Festungsbahn, Gondel, Schafbergbahn,
/// Traundl, Dampfer, Fensterladen: a zero-length vertical track, so either
/// side may be the way out) whose pit has no way back up (D-027, D-044: falling
/// off means waiting for the car or climbing back, never a softlock).
/// Empty if fine.
///
/// The check (cheap, deliberately simple): for each column the car passes
/// over, the floor is the topmost solid or one-way tile at or below the
/// highest point the car's top reaches over that column (anything higher
/// would be a ceiling, which [`track_problems`] forbids anyway). The exit
/// is the first column beyond the track at its upper end (for a level
/// track: in the direction of `to`; a vertical track passes if either
/// side works), its floor the topmost one whose top is at most
/// [`TRACK_CLEARANCE_PX`] above the car's top there (a ledge a little
/// above the car still counts as the step it is). Walking from the far
/// end of the track to the exit, no step up from one floor to the next
/// may exceed [`MAX_STEP_UP_TILES`]. Columns without a floor (bottomless
/// or hazard pits: deaths, not softlocks) are skipped, and so are columns
/// over swim water (D-077, D-076: the first tile below the track's top is
/// swim water; under a boat a fall is a swim, not a softlock); an exit over
/// swim water passes. Lethal water is looked through, as before D-077
/// (D-084).
///
/// Limits: it does not check headroom above a step, jump distances across
/// skipped columns, that the camera (which never scrolls back) leaves the
/// stairs reachable when the climb leads left, enemies, or one-way tiles
/// that need a drop-through.
pub fn recovery_problems(level: &Level, t: &Tuning) -> Vec<String> {
    let map = TileMap::from_level(level);
    let mut problems = Vec::new();
    for (i, s) in level.entities.iter().enumerate() {
        let Some((car, delta, travel)) = track(s, i, t) else {
            continue;
        };
        let c0 = tile_of(car.x.min(car.x + delta.0));
        let c1 = tile_of(car.x.max(car.x + delta.0) + car.w - Fx(1));
        // Highest car top (px) over each column c0..=c1.
        let mut highest = vec![i32::MAX; (c1 - c0 + 1) as usize];
        for k in 0..=travel {
            let (ox, oy) = track_offset(s.kind, delta, k, travel);
            let (x, y) = (car.x + ox, (car.y + oy).to_px());
            for c in tile_of(x)..=tile_of(x + car.w - Fx(1)) {
                let h = &mut highest[(c - c0) as usize];
                *h = (*h).min(y);
            }
        }
        // The floor row; `None` when there is none or swim water comes
        // first. Lethal water is no floor and doesn't stop the search (as
        // before D-077: a fall into it is a death, not a softlock).
        let floor = |col: i32, y_px: i32| {
            (0..map.height)
                .filter(|&r| r * TILE_PX >= y_px)
                .find(|&r| {
                    matches!(
                        map.kind_at(col, r),
                        TileKind::Solid | TileKind::OneWay | TileKind::Swim
                    )
                })
                .filter(|&r| map.kind_at(col, r) != TileKind::Swim)
        };
        let over_water = |col: i32, y_px: i32| {
            (0..map.height)
                .filter(|&r| r * TILE_PX >= y_px)
                .find(|&r| map.kind_at(col, r) != TileKind::Empty)
                .is_some_and(|r| map.kind_at(col, r) == TileKind::Swim)
        };
        let upper_top = (car.y + delta.1.min(Fx::ZERO)).to_px();
        let at = format!(
            "{} at row {}, column {}",
            s.kind.name(),
            s.row + 1,
            s.col + 1
        );
        // The way out towards one side: `Err` describes why it fails.
        let climb = |right: bool| -> Result<(), String> {
            let exit = if right { c1 + 1 } else { c0 - 1 };
            if over_water(exit, upper_top - TRACK_CLEARANCE_PX) {
                return Ok(());
            }
            let Some(exit_floor) = floor(exit, upper_top - TRACK_CLEARANCE_PX) else {
                return Err(format!(
                    "{at}: no floor in column {} beyond the upper end of its track",
                    exit + 1
                ));
            };
            let mut cols: Vec<i32> = (c0..=c1).collect();
            if !right {
                cols.reverse();
            }
            let walk = cols
                .into_iter()
                .map(|c| (c, floor(c, highest[(c - c0) as usize])))
                .chain([(exit, Some(exit_floor))]);
            let mut prev: Option<i32> = None;
            for (col, row) in walk {
                let Some(row) = row else { continue };
                if let Some(p) = prev
                    && p - row > MAX_STEP_UP_TILES
                {
                    return Err(format!(
                        "{at}: no way back up from the pit under its track: a {}-tile step up to column {} (max {MAX_STEP_UP_TILES}); add stair-stepped rock",
                        p - row,
                        col + 1
                    ));
                }
                prev = Some(row);
            }
            Ok(())
        };
        let result = if delta.0 == Fx::ZERO {
            // Vertical: either side will do.
            climb(true).or_else(|e| climb(false).map_err(|_| e))
        } else {
            // The upper end is `to` unless the track goes down.
            let to_is_upper = delta.1 <= Fx::ZERO;
            climb((delta.0 > Fx::ZERO) == to_is_upper)
        };
        if let Err(p) = result {
            problems.push(p);
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

    /// Runs the state machine `frames` frames from `(state, count)` with a
    /// rider on every frame where `rider(i)` holds; the height fraction `k`
    /// after each frame.
    fn heights(start: (u8, u16), frames: usize, rider: impl Fn(usize) -> bool) -> Vec<i32> {
        heights_with(
            car_timing(EntityKind::Festungsbahn, None, &tuning()),
            start,
            frames,
            rider,
        )
    }

    fn heights_with(
        timing: CarTiming,
        start: (u8, u16),
        frames: usize,
        rider: impl Fn(usize) -> bool,
    ) -> Vec<i32> {
        let (mut s, mut c) = start;
        (0..frames)
            .map(|i| {
                (s, c) = next_state(s, c, rider(i), timing);
                height_fraction(s, c, timing).0
            })
            .collect()
    }

    #[test]
    fn the_car_waits_for_a_rider_then_travels_up_waits_and_comes_back() {
        let t = tuning();
        let timing = car_timing(EntityKind::Festungsbahn, None, &t);
        assert_eq!(
            timing,
            CarTiming {
                travel: 150,
                wait: 60,
                moor: false
            }
        );
        assert_eq!(height_fraction(car::WAIT_LOWER, 0, timing), (0, 150));
        // Nobody boards: it waits forever.
        assert!(
            heights((car::WAIT_LOWER, 0), 1000, |_| false)
                .iter()
                .all(|&k| k == 0)
        );
        // A rider from frame 100 (index 99) on.
        let k = heights((car::WAIT_LOWER, 0), 1000, |i| i >= 99);
        assert_eq!(k[98], 0, "waits until the rider stands on it");
        assert_eq!(k[99], 1, "leaves in the frame the rider is on it");
        assert_eq!(k[99 + 149], 150, "arrives after 150 frames of travel");
        assert_eq!(k[99 + 149 + 60], 150, "waits 60 frames at the top");
        assert_eq!(k[99 + 149 + 61], 149, "comes back");
        assert_eq!(k[99 + 149 + 61 + 149], 0, "down after 150 frames");
        assert_eq!(k[99 + 149 + 61 + 150], 0, "stops at the bottom");
        assert_eq!(k[99 + 149 + 61 + 151], 1, "the rider is still on: up again");
        // Without a rider at the bottom it stays there.
        let k = heights((car::WAIT_LOWER, 0), 600, |i| i == 0);
        assert_eq!(k[359], 0, "back at the bottom");
        assert!(k[359..].iter().all(|&k| k == 0), "waits for the next rider");
        // Spawned at the upper end: down first, then it waits.
        let k = heights((car::DOWN, 0), 400, |_| false);
        assert_eq!((k[0], k[148], k[149]), (149, 1, 0));
        assert!(k[149..].iter().all(|&k| k == 0));
    }

    #[test]
    fn standing_on_needs_feet_on_the_top_and_a_real_overlap() {
        let px = Fx::from_px;
        let car = Aabb {
            x: px(64),
            y: px(168),
            w: px(48),
            h: px(24),
        };
        let p = |x: Fx, bottom: Fx| Aabb {
            x,
            y: bottom - px(22),
            w: px(12),
            h: px(22),
        };
        assert!(stands_on(&p(px(70), px(168)), &car));
        // Edge-only touches on either side: not on it.
        assert!(!stands_on(&p(px(52), px(168)), &car), "right edge == car.x");
        assert!(
            !stands_on(&p(px(112), px(168)), &car),
            "left edge == car.right()"
        );
        // One subpixel of overlap is enough.
        assert!(stands_on(&p(px(52) + Fx(1), px(168)), &car));
        assert!(stands_on(&p(px(112) - Fx(1), px(168)), &car));
        // Feet not exactly on the top.
        assert!(!stands_on(&p(px(70), px(168) - Fx(1)), &car));
        assert!(!stands_on(&p(px(70), px(168) + Fx(1)), &car));
    }

    #[test]
    fn the_lower_end_comes_from_the_track_direction() {
        let spawn = |to: [i32; 2]| EntitySpawn {
            kind: SpawnKind::Festungsbahn,
            col: 0,
            row: 0,
            text: None,
            style: Default::default(),
            to: Some(to),
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
            open: crate::level::DEFAULT_FENSTERLADEN_OPEN,
            range: crate::level::DEFAULT_STEINSCHLAG_RANGE,
            stop: 0,
            cars: 0,
            look: Default::default(),
        };
        assert_eq!(initial_state(&spawn([8, -5])), car::WAIT_LOWER);
        assert_eq!(initial_state(&spawn([8, 0])), car::WAIT_LOWER);
        assert_eq!(initial_state(&spawn([-3, 4])), car::DOWN);
        assert!(starts_at_upper_end(&spawn([0, 2])));
    }

    /// A 14-row level from `rows` (row 0 at the top), padded with air
    /// above; `F` rises [8, -5] (or `to` as given).
    fn track_level(to: &str, rows: &[&str]) -> Level {
        let width = rows[0].len();
        let mut grid: Vec<String> = vec![".".repeat(width); 14 - rows.len()];
        grid.extend(rows.iter().map(|r| r.to_string()));
        let text = format!(
            "[meta]\nid = \"t\"\ntime_limit = 1\n[legend]\n\"#\" = \"solid\"\n\"-\" = \"oneway\"\n\"P\" = {{ entity = \"player_start\" }}\n\"G\" = {{ entity = \"goal\" }}\n\"F\" = {{ entity = \"festungsbahn\", to = {to} }}\n[map]\ngrid = '''\n{}\n'''\n",
            grid.join("\n")
        );
        Level::parse(&text).unwrap()
    }

    #[test]
    fn a_pit_under_a_track_needs_steps_of_at_most_3_tiles_back_up() {
        let t = tuning();
        // 00_enemies before D-027: the car rises from column 3 onto the
        // plateau (columns 14-16, top row 8) over a pit with its floor at
        // row 14: a 6-tile wall.
        let wall = track_level(
            "[8, -5]",
            &[
                ".............###", // row 7
                ".............###",
                ".............###",
                ".............###",
                "P............###",
                "#.F..........###", // row 12
                "################",
            ],
        );
        let p = recovery_problems(&wall, &t);
        assert_eq!(p.len(), 1, "{p:?}");
        assert!(
            p[0].starts_with(
                "festungsbahn at row 13, column 3: no way back up from the pit under its track: a 6-tile step up to column 14"
            ),
            "{}",
            p[0]
        );
        // Stair-stepped rock under the track (as in 00_enemies): steps up
        // of 1, 1, 1, 1 and 2 tiles.
        let stairs = track_level(
            "[8, -5]",
            &[
                ".............###", // row 7
                ".............###",
                "............####",
                "..........######",
                "P........#######",
                "#.F....#########", // row 12
                "################",
            ],
        );
        assert_eq!(recovery_problems(&stairs, &t), Vec::<String>::new());
        // A single 4-tile step is too high; one-way tiles count as floor.
        let high = track_level(
            "[8, -5]",
            &[
                ".............###", // row 7
                ".............###",
                "............-###",
                ".............###",
                "P............###",
                "#.F..........###", // row 12
                "################",
            ],
        );
        let p = recovery_problems(&high, &t);
        assert_eq!(p.len(), 1, "{p:?}");
        assert!(p[0].contains("a 4-tile step up to column 13"), "{}", p[0]);
    }

    /// [`track_level`] with the `F` spawn as a `gondel` instead.
    fn gondel_level(to: &str, rows: &[&str]) -> Level {
        let mut level = track_level(to, rows);
        for e in &mut level.entities {
            if e.kind == SpawnKind::Festungsbahn {
                e.kind = SpawnKind::Gondel;
            }
        }
        level
    }

    #[test]
    fn gondel_tracks_get_the_clearance_and_recovery_lints() {
        let t = tuning();
        // The intro cabin over a 2-deep trench with stairs at both ends:
        // rises 1 tile over 6 to a knoll (columns 10-13, top row 9).
        let trench = [
            "..........####", // row 9: the knoll
            "P.F.......####", // row 10: the cabin's spawn cell
            "##.......#####", // row 11
            "###.....######", // row 12 (2 deep)
            "##############",
        ];
        let lvl = gondel_level("[6, -1]", &trench[..]);
        assert_eq!(recovery_problems(&lvl, &t), Vec::<String>::new());
        assert_eq!(track_problems(&lvl, &t), Vec::<String>::new());
        // A 5-tile wall at the upper end: no way back up.
        let wall = [
            "..........####", // row 8
            "..........####",
            "..........####",
            "P.F.......####", // row 11
            "##........####",
            "##############",
        ];
        let p = recovery_problems(&gondel_level("[6, -1]", &wall[..]), &t);
        assert_eq!(p.len(), 1, "{p:?}");
        assert!(
            p[0].starts_with("gondel at row 12, column 3: no way back up"),
            "{}",
            p[0]
        );
        // A ceiling 2 tiles above the roof somewhere along the track.
        let mut low = trench.to_vec();
        low.insert(0, "......#.......");
        low.insert(1, "..............");
        let p = track_problems(&gondel_level("[6, -1]", &low), &t);
        assert_eq!(p.len(), 1, "{p:?}");
        assert!(
            p[0].starts_with("gondel at row 11, column 3: less than 48 px free"),
            "{}",
            p[0]
        );
        // Over a death ravine (no floor at all) recovery is skipped.
        let ravine = [
            "..........####", // row 9
            "P.F.......####",
            "##........####",
            "##........####",
            "##........####",
        ];
        let lvl = gondel_level("[6, -1]", &ravine[..]);
        assert_eq!(recovery_problems(&lvl, &t), Vec::<String>::new());
    }

    /// [`track_level`] with the `F` spawn as a `schafbergbahn` instead.
    fn train_level(to: &str, rows: &[&str]) -> Level {
        let mut level = track_level(to, rows);
        for e in &mut level.entities {
            if e.kind == SpawnKind::Festungsbahn {
                e.kind = SpawnKind::Schafbergbahn;
            }
        }
        level
    }

    /// A 1:4 track [8, −2] from cell (2, 12) (body columns 2–5, rows 11–12,
    /// roof flush with the lower platform at columns 0–1) to cell (10, 10)
    /// (roof flush with the upper platform from column 14, top row 9), over
    /// steps that stay below the swept body: row 13, then row 12 at 10–13.
    /// The body's lowest point over a column is where its lower right
    /// corner enters it (it rises to the right): bottom 232 − 4c px over
    /// column c ≥ 6, so 192 (row 12's top) over column 10.
    const TRAIN_ROWS: [&str; 7] = [
        "P................", // row 7
        ".................",
        "..............###", // row 9
        "..............###",
        "##............###", // row 11
        "##F.......#######",
        "#################", // row 13
    ];

    #[test]
    fn the_rail_steps_1_px_per_4_and_reaches_the_end_exactly() {
        // Rising right (Stage 5's [24, −6] = 384 × −96 px).
        let r = |u| rail_offset(u, 384, -96);
        assert_eq!((r(0), r(3), r(4), r(7), r(8)), (0, 0, -1, -1, -2));
        assert_eq!((r(-1), r(-4), r(-5)), (1, 1, 2), "beyond the lower end");
        assert_eq!(r(384), -96);
        // Spawned at the upper end going down-left ([−8, 2]): the same
        // rail, seen from the other end.
        let d = |u| rail_offset(u, -128, 32);
        assert_eq!((d(0), d(-1), d(-4), d(-128)), (0, 1, 1, 32));
        assert_eq!((d(4), d(5)), (-1, -1));
        // The train's offsets: the height follows the whole-px x offset.
        let delta = (Fx::from_px(384), Fx::from_px(-96));
        for k in 0..=576 {
            let (x, y) = track_offset(SpawnKind::Schafbergbahn, delta, k, 576);
            assert_eq!(y, Fx::from_px(rail_offset(x.to_px(), 384, -96)), "{k}");
        }
        assert_eq!(
            track_offset(SpawnKind::Schafbergbahn, delta, 576, 576),
            delta
        );
        // Other platforms keep the plain straight line.
        assert_eq!(
            track_offset(SpawnKind::Festungsbahn, delta, 3, 576).1,
            Fx::from_px(-96).mul_int(3).div_int(576)
        );
    }

    #[test]
    fn the_train_lint_covers_its_swept_body_and_3_tiles_above_it() {
        let t = tuning();
        let clean = train_level("[8, -2]", &TRAIN_ROWS);
        assert_eq!(track_problems(&clean, &t), Vec::<String>::new());
        assert_eq!(recovery_problems(&clean, &t), Vec::<String>::new());
        // The step one column earlier (column 9, top 192) cuts the body as
        // it passes (its bottom enters column 9 at 196 px), although nothing
        // is above the roof: the Festungsbahn's lint (above the top only)
        // would let it pass.
        let mut cut = TRAIN_ROWS;
        cut[5] = "##F......########";
        let cut_level = train_level("[8, -2]", &cut);
        let p = track_problems(&cut_level, &t);
        assert_eq!(p.len(), 1, "{p:?}");
        assert!(
            p[0].starts_with(
                "schafbergbahn at row 13, column 3: a tile overlaps the train's body or the 48 px above its roof"
            ),
            "{}",
            p[0]
        );
        assert_eq!(
            track_problems(&track_level("[8, -2]", &cut), &t),
            Vec::<String>::new(),
            "the car's lint looks above its top only"
        );
        // A one-way tile 2 tiles above the roof at the upper end: one-way
        // tiles count for the train (a rider would be scraped off).
        let mut low = TRAIN_ROWS;
        low[0] = "P..........-.....";
        let p = track_problems(&train_level("[8, -2]", &low), &t);
        assert_eq!(p.len(), 1, "{p:?}");
        assert!(
            p[0].contains("a tile overlaps the train's body"),
            "{}",
            p[0]
        );
        assert_eq!(
            track_problems(&track_level("[8, -2]", &low), &t),
            Vec::<String>::new(),
            "the car's lint counts solid tiles only"
        );
    }

    #[test]
    fn the_train_pit_recovery_is_skipped_over_a_lethal_gorge() {
        let t = tuning();
        // A bottomless gorge under the middle of the track (columns 6–9):
        // no floor there, so those columns are skipped; the steps on both
        // sides still lead up.
        let gorge = [
            "P................", // row 7
            ".................",
            "..............###", // row 9
            "..............###",
            "##............###", // row 11
            "##F.......#######",
            "######....#######", // row 13
        ];
        let lvl = train_level("[8, -2]", &gorge);
        assert_eq!(track_problems(&lvl, &t), Vec::<String>::new());
        assert_eq!(recovery_problems(&lvl, &t), Vec::<String>::new());
        // Without the steps at the upper end: a 4-tile step up from the
        // pit floor (row 13) to the upper platform (row 9).
        let wall = [
            "P................", // row 7
            ".................",
            "..............###", // row 9
            "..............###",
            "##............###", // row 11
            "##F...........###",
            "#################", // row 13
        ];
        let p = recovery_problems(&train_level("[8, -2]", &wall), &t);
        assert_eq!(p.len(), 1, "{p:?}");
        assert!(
            p[0].starts_with(
                "schafbergbahn at row 13, column 3: no way back up from the pit under its track: a 4-tile step up to column 15"
            ),
            "{}",
            p[0]
        );
    }

    #[test]
    fn a_one_way_car_travels_once_and_stays_moored() {
        let timing = CarTiming {
            travel: 320,
            wait: 0,
            moor: true,
        };
        // Nobody boards: it waits at its spawn.
        assert!(
            heights_with(timing, (car::WAIT_LOWER, 0), 500, |_| false)
                .iter()
                .all(|&k| k == 0)
        );
        // A rider from frame 10 (index 9) to 400: leaves, arrives after 320
        // frames and stays at the far end for good, rider or not.
        let k = heights_with(timing, (car::WAIT_LOWER, 0), 5000, |i| {
            (9..400).contains(&i)
        });
        assert_eq!((k[8], k[9], k[9 + 319]), (0, 1, 320));
        assert!(k[9 + 319..].iter().all(|&k| k == 320), "never comes back");
        let (mut s, mut c) = (car::UP, 319);
        (s, c) = next_state(s, c, false, timing);
        assert_eq!((s, c), (car::UP, 320));
        (s, c) = next_state(s, c, false, timing);
        assert_eq!((s, c), (car::MOORED, 0));
        assert_eq!(next_state(s, c, true, timing), (car::MOORED, 0));
        assert_eq!(height_fraction(car::MOORED, 0, timing), (320, 320));
    }

    /// A 17-row lake level (decks on row 11, water rows 12–15, bed row 16)
    /// from `rows` for rows 10–16, with a boat `B` (`entity` with `to`).
    fn lake_level(entity: &str, to: &str, rows: &[&str]) -> Level {
        let width = rows[0].len();
        let mut grid: Vec<String> = vec![".".repeat(width); 17 - rows.len()];
        grid.extend(rows.iter().map(|r| r.to_string()));
        let text = format!(
            "[meta]\nid = \"t\"\ntime_limit = 1\n[legend]\n\"#\" = \"solid\"\n\"=\" = \"steg\"\n\"~\" = \"see_wasser\"\n\"w\" = \"wolfgangsee_water\"\n\"P\" = {{ entity = \"player_start\" }}\n\"G\" = {{ entity = \"goal\" }}\n\"B\" = {{ entity = \"{entity}\", to = {to} }}\n[map]\ngrid = '''\n{}\n'''\n",
            grid.join("\n")
        );
        Level::parse(&text).unwrap()
    }

    #[test]
    fn boats_skip_the_pit_recovery_over_water() {
        let t = tuning();
        // A Dampfer (10 tiles) from the quay (columns 0–1) 8 tiles right to
        // a pier (columns 20–23) over open water: every column under it is
        // swim water, so a fall is a swim, never a softlock (D-077).
        let lake = [
            "P.....................G.", // row 10
            "##B.................====", // row 11: quay, pier
            "##~~~~~~~~~~~~~~~~~~~~~~",
            "##~~~~~~~~~~~~~~~~~~~~~~",
            "##~~~~~~~~~~~~~~~~~~~~~~",
            "##~~~~~~~~~~~~~~~~~~~~~~",
            "########################", // row 16: the lake bed
        ];
        let lvl = lake_level("dampfer", "[8, 0]", &lake);
        assert_eq!(recovery_problems(&lvl, &t), Vec::<String>::new());
        assert_eq!(track_problems(&lvl, &t), Vec::<String>::new());
        // The same basin dry, or filled with lethal water (looked through
        // as before D-077, D-084): a 5-tile step up from its floor
        // to the pier.
        let lethal: Vec<String> = lake.iter().map(|r| r.replace('~', "w")).collect();
        let lethal: Vec<&str> = lethal.iter().map(String::as_str).collect();
        let p = recovery_problems(&lake_level("dampfer", "[8, 0]", &lethal), &t);
        assert_eq!(p.len(), 1, "{p:?}");
        assert!(p[0].contains("a 5-tile step up to column 21"), "{p:?}");
        let dry: Vec<String> = lake.iter().map(|r| r.replace('~', ".")).collect();
        let dry: Vec<&str> = dry.iter().map(String::as_str).collect();
        let p = recovery_problems(&lake_level("dampfer", "[8, 0]", &dry), &t);
        assert_eq!(p.len(), 1, "{p:?}");
        assert!(
            p[0].starts_with("dampfer at row 12, column 3: no way back up")
                && p[0].contains("a 5-tile step up to column 21"),
            "{}",
            p[0]
        );
        // A Traundl whose far end is open water (the exit is a swim too).
        let open = [
            "P....................G", // row 10
            "##B...................",
            "##~~~~~~~~~~~~~~~~~~~~",
            "##~~~~~~~~~~~~~~~~~~~~",
            "##~~~~~~~~~~~~~~~~~~~~",
            "##~~~~~~~~~~~~~~~~~~~~",
            "######################",
        ];
        let lvl = lake_level("traundl", "[6, 0]", &open);
        assert_eq!(recovery_problems(&lvl, &t), Vec::<String>::new());
        // Its clearance: a solid tile 2 tiles above the deck on its way.
        let mut low = open.to_vec();
        low.insert(0, "..........#...........");
        low.insert(1, "......................");
        let p = track_problems(&lake_level("traundl", "[6, 0]", &low), &t);
        assert_eq!(p.len(), 1, "{p:?}");
        assert!(
            p[0].starts_with("traundl at row 12, column 3: less than 48 px free"),
            "{}",
            p[0]
        );
    }

    #[test]
    fn the_paddle_wheel_turns_only_while_the_dampfer_travels() {
        let t = tuning();
        let mut ship = Entity::at_feet(
            EntityKind::Dampfer,
            Fx::from_px(80),
            Fx::from_px(192),
            t.platforms.dampfer_size,
        );
        assert_eq!(ship.x, Fx::ZERO);
        for state in [car::WAIT_LOWER, car::MOORED] {
            ship.state = state;
            assert_eq!(paddle_wheel(&ship, &t), None, "{state}");
        }
        ship.state = car::UP;
        // 32×20, centred on the hull's centre x (80), from 4 px above the
        // water line (192) to 16 px below it.
        assert_eq!(
            paddle_wheel(&ship, &t),
            Some(Aabb {
                x: Fx::from_px(64),
                y: Fx::from_px(188),
                w: Fx::from_px(32),
                h: Fx::from_px(20),
            })
        );
        ship.active = false;
        assert_eq!(paddle_wheel(&ship, &t), None, "not before activation");
        let mut other = ship;
        other.active = true;
        other.kind = EntityKind::Festungsbahn;
        assert_eq!(paddle_wheel(&other, &t), None, "only the Dampfer");
    }

    #[test]
    fn the_climb_leads_to_the_upper_end_on_either_side() {
        let t = tuning();
        let check = |to: &str, rows: &[&str]| recovery_problems(&track_level(to, rows), &t);
        // Mirrored: the car starts at column 14 and rises to the left onto
        // the plateau (columns 1-5): the climb leads left.
        let wall = [
            "#####.............", // row 7
            "#####.............",
            "#####.............",
            "#####.............",
            "#####............P",
            "#####........F...#", // row 12
            "##################",
        ];
        let p = check("[-8, -5]", &wall);
        assert_eq!(p.len(), 1, "{p:?}");
        assert!(p[0].contains("a 6-tile step up to column 5"), "{}", p[0]);
        let mut stairs = [
            "#####.............", // row 7
            "#####.............",
            "######............",
            "########..........",
            "#########........P",
            "###########..F...#", // row 12
            "##################",
        ];
        assert_eq!(check("[-8, -5]", &stairs), Vec::<String>::new());
        // The same track described from its upper end (spawned on the
        // plateau's edge, going down to the right): the same exit.
        stairs[0] = "#####F............";
        stairs[5] = "###########......#";
        assert_eq!(check("[8, 5]", &stairs), Vec::<String>::new());
        let mut wall = wall;
        wall[0] = "#####F............";
        wall[5] = "#####............#";
        let p = check("[8, 5]", &wall);
        assert_eq!(p.len(), 1, "{p:?}");
        assert!(p[0].contains("a 6-tile step up to column 5"), "{}", p[0]);
    }

    #[test]
    fn a_vertical_track_passes_if_either_side_leads_out() {
        let t = tuning();
        let check = |rows: &[&str]| recovery_problems(&track_level("[0, -5]", rows), &t);
        // Car in columns 6-8; a 6-tile wall right, a 1-tile step left.
        let open_left = [
            "P...........", // row 6
            "........####", // row 7
            "........####",
            "........####",
            "........####",
            "........####",
            "####.F..####", // row 12
            "############",
        ];
        assert_eq!(check(&open_left), Vec::<String>::new());
        // Walls on both sides: reported for the right side.
        let mut walled = open_left;
        for r in &mut walled[1..7] {
            *r = if r.contains('F') {
                "#####F..####"
            } else {
                "#####...####"
            };
        }
        let p = check(&walled);
        assert_eq!(p.len(), 1, "{p:?}");
        assert!(p[0].contains("a 6-tile step up to column 9"), "{}", p[0]);
    }

    #[test]
    fn the_upper_end_needs_a_floor_and_ledges_above_the_car_count() {
        let t = tuning();
        // Nothing at all in column 14, beyond the upper end.
        let gap = track_level(
            "[8, -5]",
            &[
                "................", // row 7
                "................",
                "............#...",
                "..........###...",
                "P........####...",
                "#.F....######...", // row 12
                "#############...",
            ],
        );
        let p = recovery_problems(&gap, &t);
        assert_eq!(p.len(), 1, "{p:?}");
        assert!(
            p[0].ends_with("no floor in column 14 beyond the upper end of its track"),
            "{}",
            p[0]
        );
        // The ledge's top (row 6, y 96) is 8 px above the car's top at the
        // upper end (y 104): it is still the step out, 4 tiles above the
        // last stair (row 10), not the tile below it.
        let ledge = track_level(
            "[8, -5]",
            &[
                ".............###", // row 6
                ".............###",
                ".............###",
                ".............###",
                "..........######", // row 10
                "P........#######",
                "#.F....#########", // row 12
                "################",
            ],
        );
        let p = recovery_problems(&ledge, &t);
        assert_eq!(p.len(), 1, "{p:?}");
        assert!(p[0].contains("a 4-tile step up to column 14"), "{}", p[0]);
    }
}
