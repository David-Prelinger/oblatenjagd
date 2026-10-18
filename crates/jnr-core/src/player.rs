//! The player: SMB-style run and jump physics.

use crate::fx::Fx;
use crate::game::Event;
use crate::hash::Fnv;
use crate::input::InputFrame;
use crate::level::{EntitySpawn, TILE_PX};
use crate::physics::{Aabb, TileMap, YHit};
use crate::tuning::PlayerTuning;

/// Hitbox width (smaller than the sprite, forgiving per "Difficulty: medium").
pub const PLAYER_W: Fx = Fx::from_px(12);
/// Hitbox height.
pub const PLAYER_H: Fx = Fx::from_px(22);
/// Sprite size in pixels; the hitbox sits bottom-centred in it.
pub const SPRITE_W: i32 = 16;
/// Sprite height in pixels.
pub const SPRITE_H: i32 = 24;
/// Sprite top-left relative to the hitbox top-left, in pixels.
pub const SPRITE_OFFSET: (i32, i32) = (-2, -2);

/// Which way the player looks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Facing {
    /// Looking left.
    Left,
    /// Looking right.
    Right,
}

/// The player's simulation state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Player {
    /// Hitbox left edge.
    pub x: Fx,
    /// Hitbox top edge.
    pub y: Fx,
    /// Horizontal speed per frame (+ = right).
    pub vx: Fx,
    /// Vertical speed per frame (+ = down).
    pub vy: Fx,
    /// Standing on a floor after the last step.
    pub on_ground: bool,
    /// Look direction.
    pub facing: Facing,
    /// Holding against the motion on the ground (for the skid animation).
    pub skidding: bool,
    /// In the rising part of a jump with JUMP still held (lower gravity).
    pub jump_rising: bool,
    /// JUMP was held in the previous step (to detect presses).
    pub jump_held: bool,
    /// Airborne frames left in which a jump still works (coyote time).
    pub coyote: u8,
    /// Frames left in which a buffered JUMP press is still honoured; 0 = none.
    pub jump_buffer: u8,
}

/// What happened to the player in one step (besides events).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PlayerStep {
    /// A wall stopped horizontal motion.
    pub hit_wall: bool,
    /// The head hit a ceiling.
    pub hit_ceiling: bool,
}

impl Player {
    /// A player standing at a spawn cell: hitbox bottom on the cell's
    /// bottom edge, horizontally centred in the cell. Starts as grounded so
    /// the first step does not report `Landed`.
    pub fn spawn(at: EntitySpawn) -> Player {
        let tile = Fx::from_px(TILE_PX);
        Player {
            x: tile.mul_int(at.col) + (tile - PLAYER_W).div_int(2),
            y: tile.mul_int(at.row + 1) - PLAYER_H,
            vx: Fx::ZERO,
            vy: Fx::ZERO,
            on_ground: true,
            facing: Facing::Right,
            skidding: false,
            jump_rising: false,
            jump_held: false,
            coyote: 0,
            jump_buffer: 0,
        }
    }

    /// The hitbox.
    pub fn hitbox(&self) -> Aabb {
        Aabb {
            x: self.x,
            y: self.y,
            w: PLAYER_W,
            h: PLAYER_H,
        }
    }

    /// Centre of the hitbox in whole pixels (camera focus).
    pub fn center_px(&self) -> (i32, i32) {
        (
            (self.x + PLAYER_W.div_int(2)).to_px(),
            (self.y + PLAYER_H.div_int(2)).to_px(),
        )
    }

    /// Advances the player by one tick: horizontal speed, jump, gravity,
    /// then movement with tile collision (X, then Y). The hitbox never goes
    /// left of `min_x` (the camera's left edge).
    pub fn update(
        &mut self,
        input: InputFrame,
        map: &TileMap,
        t: &PlayerTuning,
        min_x: Fx,
        events: &mut Vec<Event>,
    ) -> PlayerStep {
        self.update_horizontal_speed(input, t);
        self.update_jump(input, t, events);

        // Gravity: lighter while rising with JUMP held (variable jump height).
        if !input.jump() || self.vy >= Fx::ZERO {
            self.jump_rising = false;
        }
        let g = if self.jump_rising {
            t.gravity_hold
        } else {
            t.gravity
        };
        self.vy = (self.vy + g).min(t.max_fall_speed);

        // Move X, then Y.
        let mut step = PlayerStep::default();
        let mut b = self.hitbox();
        if map.move_x(&mut b, self.vx) {
            self.vx = Fx::ZERO;
            step.hit_wall = true;
        }
        if b.x < min_x {
            b.x = min_x;
            self.vx = self.vx.max(Fx::ZERO);
        }
        let was_on_ground = self.on_ground;
        match map.move_y(&mut b, self.vy, input.down()) {
            YHit::Floor => {
                self.vy = Fx::ZERO;
                self.on_ground = true;
                self.jump_rising = false;
                self.coyote = t.coyote_frames;
                if !was_on_ground {
                    events.push(Event::Landed);
                }
            }
            YHit::Ceiling => {
                self.vy = Fx::ZERO;
                self.on_ground = false;
                self.jump_rising = false;
                step.hit_ceiling = true;
            }
            YHit::None => self.on_ground = false,
        }
        self.x = b.x;
        self.y = b.y;
        step
    }

    fn update_horizontal_speed(&mut self, input: InputFrame, t: &PlayerTuning) {
        let dir = input.right() as i32 - input.left() as i32;
        let max = if input.run() {
            t.run_speed
        } else {
            t.walk_speed
        };
        let against = dir != 0 && self.vx.signum() == -dir;
        self.skidding = self.on_ground && against;
        if self.on_ground {
            if dir == 0 {
                self.vx = self.vx.approach(Fx::ZERO, t.friction);
            } else if against {
                self.vx = self.vx.approach(Fx::ZERO, t.skid_decel);
            } else if self.vx.abs() > max {
                // Above the current top speed (RUN released): slow down.
                self.vx = self.vx.approach(max.mul_int(dir), t.friction);
            } else {
                self.vx = self.vx.approach(max.mul_int(dir), t.ground_accel);
            }
            if dir > 0 {
                self.facing = Facing::Right;
            } else if dir < 0 {
                self.facing = Facing::Left;
            }
        } else if dir != 0 && (against || self.vx.abs() < max) {
            // Air control: steer and turn, but never drag momentum down.
            self.vx = self.vx.approach(max.mul_int(dir), t.air_accel);
        }
    }

    fn update_jump(&mut self, input: InputFrame, t: &PlayerTuning, events: &mut Vec<Event>) {
        let pressed = input.jump() && !self.jump_held;
        self.jump_held = input.jump();
        if pressed {
            self.jump_buffer = t.jump_buffer_frames + 1;
        } else {
            self.jump_buffer = self.jump_buffer.saturating_sub(1);
        }
        let can_jump = self.on_ground || self.coyote > 0;
        if self.jump_buffer > 0 && can_jump {
            self.vy = -self.jump_speed(t);
            self.on_ground = false;
            self.jump_rising = true;
            self.jump_buffer = 0;
            self.coyote = 0;
            events.push(Event::Jumped);
        } else if !self.on_ground {
            self.coyote = self.coyote.saturating_sub(1);
        }
    }

    /// Jump speed for the current horizontal speed: `jump_speed_walk` up to
    /// walk speed, rising linearly to `jump_speed_run` at run speed.
    fn jump_speed(&self, t: &PlayerTuning) -> Fx {
        let range = t.run_speed - t.walk_speed;
        if range <= Fx::ZERO {
            return t.jump_speed_walk;
        }
        let over = (self.vx.abs() - t.walk_speed).max(Fx::ZERO).min(range);
        let extra = t.jump_speed_run - t.jump_speed_walk;
        t.jump_speed_walk + extra.mul_int(over.raw()).div_int(range.raw())
    }

    pub(crate) fn hash(&self, h: &mut Fnv) {
        h.i32(self.x.raw());
        h.i32(self.y.raw());
        h.i32(self.vx.raw());
        h.i32(self.vy.raw());
        h.bool(self.on_ground);
        h.u8(match self.facing {
            Facing::Left => 0,
            Facing::Right => 1,
        });
        h.bool(self.skidding);
        h.bool(self.jump_rising);
        h.bool(self.jump_held);
        h.u8(self.coyote);
        h.u8(self.jump_buffer);
    }
}
