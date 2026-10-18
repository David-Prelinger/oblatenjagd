//! The Festungsbahn car: a moving one-way platform (`docs/game-design.md`
//! → "Festungsbahn car", D-026).
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

/// The state machine's next `(state, count)` after `(state, count)`;
/// `rider` = the player stands on the car this frame.
pub fn next_state(state: u8, count: u16, rider: bool, t: &Tuning) -> (u8, u16) {
    let wait = t.platforms.festungsbahn_wait;
    let travel = t.platforms.festungsbahn_travel.max(1);
    match state {
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
pub fn height_fraction(state: u8, count: u16, t: &Tuning) -> (i32, i32) {
    let travel = i32::from(t.platforms.festungsbahn_travel.max(1));
    let count = i32::from(count).min(travel);
    let k = match state {
        car::UP => count,
        car::WAIT_UPPER => travel,
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

/// One frame of the car (called from `Entity::update`).
pub(crate) fn update_car(e: &mut Entity, ctx: &UpdateCtx) {
    let spawn = e.spawn.and_then(|i| ctx.level.entities.get(usize::from(i)));
    let delta = spawn.map_or((Fx::ZERO, Fx::ZERO), track_delta);
    let from_upper = spawn.is_some_and(starts_at_upper_end);
    let rider = ctx
        .player_standing
        .is_some_and(|p| stands_on(&p, &e.hitbox()));
    (e.state, e.count) = next_state(e.state, e.count, rider, ctx.tuning);
    let (k, n) = height_fraction(e.state, e.count, ctx.tuning);
    // `k` counts from the lower end, the position from the spawn point.
    let k = if from_upper { n - k } else { k };
    let x = e.base_x + delta.0.mul_int(k).div_int(n);
    let y = e.base_y + delta.1.mul_int(k).div_int(n);
    e.vx = x - e.x;
    e.vy = y - e.y;
    e.x = x;
    e.y = y;
}

/// A grounded player with hitbox `p` stands on the car with hitbox `c`:
/// feet exactly on its top, overlapping it horizontally.
fn stands_on(p: &Aabb, c: &Aabb) -> bool {
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
    if p.on_ground && stands_on(&b, &old) {
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

/// Highest step-up (tiles) the way back out of the pit under a track may
/// need: a walking jump reaches 4 tiles, minus one tile of safety margin.
pub const MAX_STEP_UP_TILES: i32 = 3;

/// Problems of every Festungsbahn whose pit has no way back up (D-027:
/// falling off means waiting for the car or climbing back, never a
/// softlock). Empty if fine.
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
/// or hazard pits: deaths, not softlocks) are skipped.
///
/// Limits: it does not check headroom above a step, jump distances across
/// skipped columns, that the camera (which never scrolls back) leaves the
/// stairs reachable when the climb leads left, enemies, or one-way tiles
/// that need a drop-through. Platforms other than the Festungsbahn don't
/// exist yet.
pub fn recovery_problems(level: &Level, t: &Tuning) -> Vec<String> {
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
        let c0 = tile_of(car.x.min(car.x + delta.0));
        let c1 = tile_of(car.x.max(car.x + delta.0) + car.w - Fx(1));
        // Highest car top (px) over each column c0..=c1.
        let mut highest = vec![i32::MAX; (c1 - c0 + 1) as usize];
        for k in 0..=travel {
            let x = car.x + delta.0.mul_int(k).div_int(travel);
            let y = (car.y + delta.1.mul_int(k).div_int(travel)).to_px();
            for c in tile_of(x)..=tile_of(x + car.w - Fx(1)) {
                let h = &mut highest[(c - c0) as usize];
                *h = (*h).min(y);
            }
        }
        let floor = |col: i32, y_px: i32| {
            (0..map.height)
                .filter(|&r| r * TILE_PX >= y_px)
                .find(|&r| matches!(map.kind_at(col, r), TileKind::Solid | TileKind::OneWay))
        };
        let upper_top = (car.y + delta.1.min(Fx::ZERO)).to_px();
        let at = format!("festungsbahn at row {}, column {}", s.row + 1, s.col + 1);
        // The way out towards one side: `Err` describes why it fails.
        let climb = |right: bool| -> Result<(), String> {
            let exit = if right { c1 + 1 } else { c0 - 1 };
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
        let t = tuning();
        let (mut s, mut c) = start;
        (0..frames)
            .map(|i| {
                (s, c) = next_state(s, c, rider(i), &t);
                height_fraction(s, c, &t).0
            })
            .collect()
    }

    #[test]
    fn the_car_waits_for_a_rider_then_travels_up_waits_and_comes_back() {
        let t = tuning();
        assert_eq!(height_fraction(car::WAIT_LOWER, 0, &t), (0, 150));
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
