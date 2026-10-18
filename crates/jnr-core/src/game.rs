//! The game state machine and the per-tick `step`.

use std::rc::Rc;

use crate::camera::Camera;
use crate::fx::Fx;
use crate::hash::Fnv;
use crate::input::InputFrame;

use crate::level::{EntityKind, EntitySpawn, Level, TileKind};
use crate::physics::{Aabb, TILE, TileMap};
use crate::player::Player;
use crate::rng::Rng;
use crate::tuning::Tuning;

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

/// The complete simulation state. Cheap to clone (the static level data is
/// shared via `Rc`).
#[derive(Clone, Debug)]
pub struct Game {
    frame: u32,
    mode: Mode,
    map: Rc<TileMap>,
    goals: Rc<[Aabb]>,
    start: EntitySpawn,
    player: Player,
    camera: Camera,
    rng: Rng,
    deaths: u32,
    tuning: Tuning,
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
        let player = Player::spawn(start);
        let camera = Camera::new(
            player.center_px(),
            map.width_fx().to_px(),
            map.height_fx().to_px(),
            &tuning.camera,
        );
        Game {
            frame: 0,
            mode: Mode::Playing,
            map: Rc::new(map),
            goals: goals.into(),
            start,
            player,
            camera,
            rng: Rng::new(seed),
            deaths: 0,
            tuning: *tuning,
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
        self.player = Player::spawn(self.start);
        self.camera = Camera::new(
            self.player.center_px(),
            self.map.width_fx().to_px(),
            self.map.height_fx().to_px(),
            &self.tuning.camera,
        );
        self.mode = Mode::Playing;
        self.events.push(Event::Respawned);
    }

    /// FNV-1a hash over all simulation state, field by field. Equal hashes
    /// after equal inputs prove determinism.
    pub fn state_hash(&self) -> u64 {
        let mut h = Fnv::new();
        h.u32(self.frame);
        match self.mode {
            Mode::Playing => h.u8(0),
            Mode::Dying { timer } => {
                h.u8(1);
                h.u16(timer);
            }
            Mode::StageClear => h.u8(2),
        }
        h.u64(self.map.content_hash());
        h.u32(self.goals.len() as u32);
        for g in self.goals.iter() {
            for v in [g.x, g.y, g.w, g.h] {
                h.i32(v.raw());
            }
        }
        h.i32(self.start.col);
        h.i32(self.start.row);
        self.player.hash(&mut h);
        self.camera.hash(&mut h);
        h.u64(self.rng.state());
        h.u32(self.deaths);
        hash_tuning(&self.tuning, &mut h);
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
    h.i32(t.camera.follow_x);
    h.i32(t.camera.deadzone_top);
    h.i32(t.camera.deadzone_bottom);
    h.u16(t.game.death_frames);
}
