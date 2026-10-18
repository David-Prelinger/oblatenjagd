//! The game state machine and the per-tick `step`.

use std::sync::Arc;

use crate::camera::Camera;
use crate::fx::Fx;
use crate::hash::Fnv;
use crate::input::InputFrame;
use crate::level::{EntityKind, EntitySpawn, Level, TILE_PX, TileKind};
use crate::physics::{Aabb, TILE, TileMap};
use crate::player::Player;
use crate::rng::Rng;
use crate::tuning::Tuning;

/// How many columns left and right [`Game::teleport_player`] searches for a floor.
pub const TELEPORT_SEARCH_COLS: i32 = 8;

/// Something that happened during a step (for audio, effects, tests).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    /// The player started a jump.
    Jumped,
    /// The player touched down after being airborne.
    Landed,
    /// The player died (hazard or fell out of the level).
    Died,
    /// The player reappeared at the player start after dying.
    Respawned,
    /// The player touched the goal.
    LevelComplete,
}

impl Event {
    /// Every event, in declaration order.
    pub const ALL: [Event; 5] = [
        Event::Jumped,
        Event::Landed,
        Event::Died,
        Event::Respawned,
        Event::LevelComplete,
    ];

    /// The variant name (`"Died"`), as used in replay files (`no_event=Died`).
    pub const fn name(self) -> &'static str {
        match self {
            Event::Jumped => "Jumped",
            Event::Landed => "Landed",
            Event::Died => "Died",
            Event::Respawned => "Respawned",
            Event::LevelComplete => "LevelComplete",
        }
    }

    /// The event with this variant name, if any.
    pub fn from_name(name: &str) -> Option<Event> {
        Event::ALL.into_iter().find(|e| e.name() == name)
    }
}

/// The game's current mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// Normal play.
    Playing,
    /// The player died; respawn when `timer` runs out.
    Dying {
        /// Frames left until respawn.
        timer: u16,
    },
    /// The goal was reached; the simulation stands still.
    StageClear,
}

/// The complete simulation state. Cheap to clone: the static level data
/// (level, collision grid, goals) is shared via `Arc`, so `Game` is also
/// `Send + Sync` (the solver may search on several threads).
#[derive(Clone, Debug)]
pub struct Game {
    frame: u32,
    mode: Mode,
    level: Arc<Level>,
    map: Arc<TileMap>,
    goals: Arc<[Aabb]>,
    start: EntitySpawn,
    player: Player,
    camera: Camera,
    rng: Rng,
    deaths: u32,
    tuning: Tuning,
    /// Hash of everything above that never changes during a game (level
    /// grid, goals, start, tuning), computed once in `new`. Anything that
    /// becomes mutable later (e.g. breakable tiles) must move out of it.
    static_hash: u64,
    events: Vec<Event>,
}

impl Game {
    /// Starts a level: the player stands at the player start, the camera
    /// looks at the player.
    ///
    /// A goal occupies its cell's column from one level height above the
    /// top of the level down to the cell's bottom edge (like a pole: it
    /// cannot be jumped over, even above the screen).
    pub fn new(level: &Level, tuning: &Tuning, seed: u64) -> Game {
        let map = TileMap::from_level(level);
        let goals: Vec<Aabb> = level
            .entities
            .iter()
            .filter(|e| e.kind == EntityKind::Goal)
            .map(|e| Aabb {
                x: TILE.mul_int(e.col),
                y: -TILE.mul_int(level.height),
                w: TILE,
                h: TILE.mul_int(level.height + e.row + 1),
            })
            .collect();
        let start = level.player_start();
        let player = Player::spawn(start, &tuning.player);
        let camera = Camera::new(
            player.center_px(),
            map.width_fx().to_px(),
            map.height_fx().to_px(),
            &tuning.camera,
        );

        let mut h = Fnv::new();
        map.hash(&mut h);
        h.u32(goals.len() as u32);
        for g in &goals {
            for v in [g.x, g.y, g.w, g.h] {
                h.i32(v.raw());
            }
        }
        h.i32(start.col);
        h.i32(start.row);
        hash_tuning(tuning, &mut h);

        Game {
            frame: 0,
            mode: Mode::Playing,
            level: Arc::new(level.clone()),
            map: Arc::new(map),
            goals: goals.into(),
            start,
            player,
            camera,
            rng: Rng::new(seed),
            deaths: 0,
            tuning: *tuning,
            static_hash: h.finish(),
            events: Vec::new(),
        }
    }

    /// Advances exactly one 1/60 s tick and returns what happened.
    pub fn step(&mut self, input: InputFrame) -> &[Event] {
        self.events.clear();
        self.frame = self.frame.wrapping_add(1);
        match self.mode {
            Mode::Playing => self.step_playing(input),
            Mode::Dying { timer } => {
                if timer <= 1 {
                    self.respawn();
                } else {
                    self.mode = Mode::Dying { timer: timer - 1 };
                }
            }
            Mode::StageClear => {}
        }
        &self.events
    }

    fn step_playing(&mut self, input: InputFrame) {
        let min_x = Fx::from_px(self.camera.x);
        self.player.update(
            input,
            &self.map,
            &self.tuning.player,
            min_x,
            &mut self.events,
        );
        let hb = self.player.hitbox();
        if self.map.touches(&hb, TileKind::Hazard) || hb.y >= self.map.height_fx() {
            self.mode = Mode::Dying {
                timer: self.tuning.game.death_frames.max(1),
            };
            self.player.vx = Fx::ZERO;
            self.player.vy = Fx::ZERO;
            self.deaths = self.deaths.saturating_add(1);
            self.events.push(Event::Died);
            return;
        }
        if self.goals.iter().any(|g| g.overlaps(&hb)) {
            self.mode = Mode::StageClear;
            self.player.vx = Fx::ZERO;
            self.events.push(Event::LevelComplete);
        }
        self.camera.follow(
            self.player.center_px(),
            self.map.width_fx().to_px(),
            self.map.height_fx().to_px(),
            &self.tuning.camera,
        );
    }

    fn respawn(&mut self) {
        self.player = Player::spawn(self.start, &self.tuning.player);
        self.camera = Camera::new(
            self.player.center_px(),
            self.map.width_fx().to_px(),
            self.map.height_fx().to_px(),
            &self.tuning.camera,
        );
        self.mode = Mode::Playing;
        self.events.push(Event::Respawned);
    }

    /// **Tooling only** (`cargo xtask scene`); the game, replays and the
    /// solver never call it, and it bypasses physics.
    ///
    /// Puts the player with its hitbox centre at level pixel `x_px`,
    /// standing on the topmost floor (a solid or one-way tile with air
    /// above it) of that column. Without a floor there, the nearest column
    /// within [`TELEPORT_SEARCH_COLS`] that has one is used (player centred
    /// in it); without any, the player keeps its height. Speeds are reset,
    /// the mode becomes `Playing` and the camera is placed like on respawn.
    pub fn teleport_player(&mut self, x_px: i32) {
        let col = x_px.div_euclid(TILE_PX);
        let floor_row = |c: i32| -> Option<i32> {
            if c < 0 || c >= self.level.width {
                return None;
            }
            (1..self.level.height).find(|&r| {
                matches!(self.map.kind_at(c, r), TileKind::Solid | TileKind::OneWay)
                    && self.map.kind_at(c, r - 1) == TileKind::Empty
            })
        };
        let found = (0..=TELEPORT_SEARCH_COLS)
            .flat_map(|d| [col + d, col - d])
            .find_map(|c| floor_row(c).map(|r| (c, r)));
        let t = &self.tuning.player;
        let mut p = match found {
            Some((c, r)) => Player::spawn(
                EntitySpawn {
                    kind: EntityKind::PlayerStart,
                    col: c,
                    row: r - 1,
                },
                t,
            ),
            None => Player::spawn(self.start, t),
        };
        if found.is_none_or(|(c, _)| c == col) {
            p.x = Fx::from_px(x_px) - p.w.div_int(2);
        }
        if found.is_none() {
            p.y = self.player.y;
            p.on_ground = false;
        }
        self.player = p;
        self.camera = Camera::new(
            self.player.center_px(),
            self.map.width_fx().to_px(),
            self.map.height_fx().to_px(),
            &self.tuning.camera,
        );
        self.mode = Mode::Playing;
    }

    /// FNV-1a hash over all simulation state, field by field (the static
    /// part is precomputed in `new`). Equal hashes after equal inputs prove
    /// determinism.
    pub fn state_hash(&self) -> u64 {
        let mut h = Fnv::new();
        h.u64(self.static_hash);
        h.u32(self.frame);
        match self.mode {
            Mode::Playing => h.u8(0),
            Mode::Dying { timer } => {
                h.u8(1);
                h.u16(timer);
            }
            Mode::StageClear => h.u8(2),
        }
        self.player.hash(&mut h);
        self.camera.hash(&mut h);
        h.u64(self.rng.state());
        h.u32(self.deaths);
        h.finish()
    }

    /// Ticks since the start.
    pub fn frame(&self) -> u32 {
        self.frame
    }
    /// The current mode.
    pub fn mode(&self) -> Mode {
        self.mode
    }
    /// The level being played (tile names, entity spawns, meta).
    pub fn level(&self) -> &Level {
        &self.level
    }
    /// The player.
    pub fn player(&self) -> &Player {
        &self.player
    }
    /// The camera.
    pub fn camera(&self) -> Camera {
        self.camera
    }
    /// The collision grid.
    pub fn map(&self) -> &TileMap {
        &self.map
    }
    /// Goal trigger boxes.
    pub fn goals(&self) -> &[Aabb] {
        &self.goals
    }
    /// How often the player died so far.
    pub fn deaths(&self) -> u32 {
        self.deaths
    }
    /// Events of the last step.
    pub fn events(&self) -> &[Event] {
        &self.events
    }
    /// The tuning in use.
    pub fn tuning(&self) -> &Tuning {
        &self.tuning
    }
}

fn hash_tuning(t: &Tuning, h: &mut Fnv) {
    let p = &t.player;
    for v in [
        p.walk_speed,
        p.run_speed,
        p.ground_accel,
        p.friction,
        p.skid_decel,
        p.air_accel,
        p.jump_speed_walk,
        p.jump_speed_run,
        p.gravity_hold,
        p.gravity,
        p.max_fall_speed,
    ] {
        h.i32(v.raw());
    }
    h.u8(p.coyote_frames);
    h.u8(p.jump_buffer_frames);
    h.i32(p.hitbox_w);
    h.i32(p.hitbox_h);
    h.i32(t.camera.follow_x);
    h.i32(t.camera.deadzone_top);
    h.i32(t.camera.deadzone_bottom);
    h.u16(t.game.death_frames);
}
