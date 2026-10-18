//! The player: SMB-style run and jump physics.

use crate::fx::Fx;
use crate::game::Event;
use crate::hash::Fnv;
use crate::input::InputFrame;
use crate::level::{TILE_PX, TileKind};
use crate::physics::{Aabb, TileMap, YHit, tile_of};
use crate::tuning::PlayerTuning;

/// Which way the player looks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Facing {
    /// Looking left.
    Left,
    /// Looking right.
    Right,
}

/// The player's power state (`docs/game-design.md` → "Power states").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Power {
    /// Small; dies on a hit.
    Normal,
    /// Zuckerschock: taller, breaks bricks, survives one hit.
    Big,
}

/// The player's simulation state. The renderer places the sprite with its
/// feet at the bottom centre of the hitbox.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Player {
    /// Hitbox left edge.
    pub x: Fx,
    /// Hitbox top edge.
    pub y: Fx,
    /// Hitbox width (`player.hitbox_w` from the tuning).
    pub w: Fx,
    /// Hitbox height (`player.hitbox_h` from the tuning).
    pub h: Fx,
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
    /// Normal or Zuckerschock.
    pub power: Power,
    /// Frames left of the grow/shrink freeze (the whole game stands still).
    pub transform: u8,
    /// Frames left in which hits do no harm (after shrinking).
    pub invulnerable: u8,
    /// Stomps since the player last stood on the ground (stomp chain).
    pub stomp_chain: u8,
}

impl Player {
    /// A Normal player standing at a grid cell: hitbox bottom on the
    /// cell's bottom edge, horizontally centred in the cell. Starts as
    /// grounded so the first step does not report `Landed`.
    pub fn spawn(col: i32, row: i32, t: &PlayerTuning) -> Player {
        let tile = Fx::from_px(TILE_PX);
        let w = Fx::from_px(t.hitbox_w);
        let h = Fx::from_px(t.hitbox_h);
        Player {
            x: tile.mul_int(col) + (tile - w).div_int(2),
            y: tile.mul_int(row + 1) - h,
            w,
            h,
            vx: Fx::ZERO,
            vy: Fx::ZERO,
            on_ground: true,
            facing: Facing::Right,
            skidding: false,
            jump_rising: false,
            jump_held: false,
            coyote: 0,
            jump_buffer: 0,
            power: Power::Normal,
            transform: 0,
            invulnerable: 0,
            stomp_chain: 0,
        }
    }

    /// Feet position (bottom centre of the hitbox) in subpixels.
    pub fn feet(&self) -> (Fx, Fx) {
        (self.x + self.w.div_int(2), self.y + self.h)
    }

    /// Changes the power state: sets the hitbox height (feet stay in
    /// place) and starts the transformation freeze. Nothing happens if the
    /// state does not change.
    pub fn set_power(&mut self, power: Power, t: &PlayerTuning) {
        if power == self.power {
            return;
        }
        let h = Fx::from_px(match power {
            Power::Normal => t.hitbox_h,
            Power::Big => t.hitbox_h_big,
        });
        self.y = self.y + self.h - h;
        self.h = h;
        self.power = power;
        self.transform = t.transform_frames;
    }

    /// After growing (feet kept in place) the new head may be inside solid
    /// tiles, e.g. when growing while rising just below a box. That counts
    /// as a head bump: upward speed stops and the hitbox moves down until
    /// its top is flush below the lowest solid tile it overlaps; it never
    /// moves up through the tile. There is always room below: the tuning
    /// keeps `hitbox_h_big` within the whole-tile gap any Normal hitbox
    /// needs. Should moving down still put the feet into solid ground, the
    /// hitbox stays where it is and [`TileMap::move_y`] keeps it from
    /// rising through the tile.
    /// Returns true if the head was inside a solid tile.
    pub fn fit_head(&mut self, map: &TileMap) -> bool {
        let b = self.hitbox();
        let (c0, c1) = (tile_of(b.x), tile_of(b.right() - Fx(1)));
        let (r0, r1) = (tile_of(b.y), tile_of(b.bottom() - Fx(1)));
        let solid_row = |r: i32| (c0..=c1).any(|c| map.kind_at(c, r) == TileKind::Solid);
        let Some(row) = (r0..=r1).rev().find(|&r| solid_row(r)) else {
            return false;
        };
        let pushed = Aabb {
            y: Fx::from_px(TILE_PX).mul_int(row + 1),
            ..b
        };
        if !map.touches(&pushed, TileKind::Solid) {
            self.y = pushed.y;
        }
        self.vy = self.vy.max(Fx::ZERO);
        self.jump_rising = false;
        true
    }

    /// Bounces off a stomped enemy: upward speed by whether JUMP is held
    /// (held: lower gravity while it stays held, like a jump).
    pub fn bounce(&mut self, jump_held: bool, t: &PlayerTuning) {
        self.vy = -if jump_held {
            t.stomp_bounce_held
        } else {
            t.stomp_bounce
        };
        self.jump_rising = jump_held;
        self.on_ground = false;
        self.coyote = 0;
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

    /// Centre of the hitbox in whole pixels (camera focus).
    pub fn center_px(&self) -> (i32, i32) {
        (
            (self.x + self.w.div_int(2)).to_px(),
            (self.y + self.h.div_int(2)).to_px(),
        )
    }

    /// Advances the player by one tick: horizontal speed, jump, gravity,
    /// then movement with tile collision (X, then Y). The hitbox never goes
    /// left of `min_x` (the camera's left edge). Returns the grid row of the
    /// solid tile the head bumped into, if any.
    pub fn update(
        &mut self,
        input: InputFrame,
        map: &TileMap,
        t: &PlayerTuning,
        min_x: Fx,
        events: &mut Vec<Event>,
    ) -> Option<i32> {
        self.update_horizontal_speed(input, t);
        let drop_through = self.update_jump(input, map, t, events);

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
        let mut b = self.hitbox();
        if map.move_x(&mut b, self.vx) {
            self.vx = Fx::ZERO;
        }
        if b.x < min_x {
            b.x = min_x;
            self.vx = self.vx.max(Fx::ZERO);
        }
        let was_on_ground = self.on_ground;
        let mut head_row = None;
        match map.move_y(&mut b, self.vy, drop_through) {
            YHit::Floor => {
                self.vy = Fx::ZERO;
                self.on_ground = true;
                self.jump_rising = false;
                self.coyote = t.coyote_frames;
                self.stomp_chain = 0;
                if !was_on_ground {
                    events.push(Event::Landed);
                }
            }
            YHit::Ceiling => {
                self.vy = Fx::ZERO;
                self.on_ground = false;
                self.jump_rising = false;
                head_row = Some(tile_of(b.y) - 1);
            }
            YHit::None => self.on_ground = false,
        }
        self.x = b.x;
        self.y = b.y;
        head_row
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

    /// Handles JUMP presses: jump buffer, coyote time, and DOWN + JUMP on a
    /// one-way platform, which drops through instead of jumping. Returns
    /// true if the player drops through this step.
    fn update_jump(
        &mut self,
        input: InputFrame,
        map: &TileMap,
        t: &PlayerTuning,
        events: &mut Vec<Event>,
    ) -> bool {
        let pressed = input.jump() && !self.jump_held;
        self.jump_held = input.jump();
        if pressed && input.down() && self.on_ground && map.on_one_way_only(&self.hitbox()) {
            self.on_ground = false;
            self.jump_buffer = 0;
            self.coyote = 0;
            return true;
        }
        if pressed {
            self.jump_buffer = t.jump_buffer_frames.saturating_add(1);
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
        false
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
        h.i32(self.w.raw());
        h.i32(self.h.raw());
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
        h.u8(match self.power {
            Power::Normal => 0,
            Power::Big => 1,
        });
        h.u8(self.transform);
        h.u8(self.invulnerable);
        h.u8(self.stomp_chain);
    }
}
