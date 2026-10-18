//! Gameplay constants from `content/tuning.toml`.
//!
//! Speeds are in subpixels (1/256 px) per frame, accelerations in subpixels
//! per frame², durations in frames, camera values in whole pixels.

use serde::Deserialize;

use crate::ParseError;
use crate::fx::Fx;
use crate::level::TILE_PX;

/// All tunable gameplay numbers. Parsed with [`Tuning::parse`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Tuning {
    /// Player movement.
    pub player: PlayerTuning,
    /// Camera follow behaviour.
    pub camera: CameraTuning,
    /// Game flow (timers, lives, entity activation).
    pub game: GameTuning,
    /// Items and blocks.
    pub items: ItemTuning,
    /// Points.
    pub score: ScoreTuning,
    /// Enemy hitboxes (behaviour comes with the enemy task).
    pub enemies: EnemyTuning,
    /// Moving platforms.
    pub platforms: PlatformTuning,
}

/// Moving platforms (the Festungsbahn is inert until its own task).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlatformTuning {
    /// Festungsbahn car `[w, h]` in whole pixels (feet at the bottom
    /// centre, left-aligned in its spawn cell).
    pub festungsbahn_size: [i32; 2],
}

/// Player movement constants (SMB-style).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlayerTuning {
    /// Top ground speed without RUN.
    pub walk_speed: Fx,
    /// Top ground speed with RUN held.
    pub run_speed: Fx,
    /// Ground acceleration towards the held direction.
    pub ground_accel: Fx,
    /// Ground deceleration with no direction held, and when above the
    /// current top speed (e.g. RUN released).
    pub friction: Fx,
    /// Ground deceleration while holding against the current motion (skid).
    pub skid_decel: Fx,
    /// Air acceleration towards the held direction (also used to turn).
    pub air_accel: Fx,
    /// Upward jump speed when standing or walking.
    pub jump_speed_walk: Fx,
    /// Upward jump speed at full run speed; interpolated linearly between
    /// walk and run speed.
    pub jump_speed_run: Fx,
    /// Gravity while rising with JUMP held (variable jump height).
    pub gravity_hold: Fx,
    /// Gravity otherwise.
    pub gravity: Fx,
    /// Maximum downward speed.
    pub max_fall_speed: Fx,
    /// Frames after walking off a ledge in which a jump still works.
    pub coyote_frames: u8,
    /// Frames before landing in which a JUMP press is remembered.
    pub jump_buffer_frames: u8,
    /// Hitbox width in whole pixels (feet at its bottom centre).
    pub hitbox_w: i32,
    /// Hitbox height in whole pixels (Normal).
    pub hitbox_h: i32,
    /// Hitbox height in whole pixels as Zuckerschock (same width).
    pub hitbox_h_big: i32,
    /// Upward speed after a stomp with JUMP not held.
    pub stomp_bounce: Fx,
    /// Upward speed after a stomp with JUMP held (then `gravity_hold`
    /// applies while JUMP stays held, like a jump).
    pub stomp_bounce_held: Fx,
    /// Frames the player can't be hurt after shrinking (counted after the
    /// transformation freeze).
    pub invulnerable_frames: u8,
    /// Frames the game freezes while the player grows or shrinks.
    pub transform_frames: u8,
}

/// Camera constants, in whole pixels of the 320×180 screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CameraTuning {
    /// Screen x of the player's centre beyond which the camera scrolls right.
    pub follow_x: i32,
    /// Upper edge of the vertical dead zone (screen y of the player's centre).
    pub deadzone_top: i32,
    /// Lower edge of the vertical dead zone (screen y of the player's centre).
    pub deadzone_bottom: i32,
}

/// Game flow constants.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GameTuning {
    /// Frames between dying and respawning.
    pub death_frames: u16,
    /// Frames in `GameOver` before START restarts the stage.
    pub game_over_frames: u16,
    /// Lives at the start and after a game over.
    pub lives_start: u8,
    /// Most lives.
    pub lives_max: u8,
    /// Oblaten for an extra life (the counter then starts at 0 again).
    pub oblaten_per_life: u8,
    /// Entities activate when their left edge is less than this many
    /// pixels beyond the right screen edge.
    pub activate_margin: i32,
    /// Entities are removed when their right edge is more than this many
    /// pixels left of the screen (the camera never scrolls back).
    pub despawn_margin: i32,
}

/// Items, blocks and particles. Item gravity and max fall speed are the
/// player's.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ItemTuning {
    /// Static Oblate hitbox `[w, h]`, whole pixels.
    pub oblate_hitbox: [i32; 2],
    /// Mozartkugel power-up hitbox `[w, h]`.
    pub mozartkugel_hitbox: [i32; 2],
    /// Zaunerstollen 1-up hitbox `[w, h]`.
    pub zaunerstollen_hitbox: [i32; 2],
    /// Frames a power-up rises out of its box.
    pub rise_frames: u16,
    /// Rise speed out of the box.
    pub rise_speed: Fx,
    /// Rolling speed of the Mozartkugel.
    pub mozartkugel_speed: Fx,
    /// Rolling speed of the Zaunerstollen.
    pub zaunerstollen_speed: Fx,
    /// Upward speed of an Oblate popping out of a box.
    pub pop_speed: Fx,
    /// Frames until a popped Oblate disappears.
    pub pop_frames: u16,
    /// Frames of a block bump (up and down again).
    pub bump_frames: u16,
    /// Height of a block bump in whole pixels.
    pub bump_px: i32,
    /// Horizontal speed of brick debris.
    pub debris_speed_x: Fx,
    /// Upward speed of the upper two debris pieces.
    pub debris_speed_high: Fx,
    /// Upward speed of the lower two debris pieces.
    pub debris_speed_low: Fx,
    /// Most Oblaten from an `oblaten_10` brick.
    pub multi_oblaten: u8,
    /// Frames after the first hit in which an `oblaten_10` brick still pays.
    pub multi_frames: u16,
}

/// Points (`docs/game-design.md` → "Scoring, lives, timer, checkpoint").
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScoreTuning {
    /// Per Oblate.
    pub oblate: u32,
    /// Per power-up eaten (Mozartkugel).
    pub power_up: u32,
    /// Per broken brick.
    pub brick: u32,
    /// Stomps without touching the ground; past the end each stomp is +1 life.
    pub stomp_chain: [u32; 8],
    /// Frames a score popup is shown.
    pub popup_frames: u16,
    /// How far a popup floats up over its lifetime, whole pixels.
    pub popup_rise: i32,
}

/// Enemy hitboxes `[w, h]` in whole pixels (feet at the bottom centre).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EnemyTuning {
    /// Selfie-Tourist.
    pub selfie_tourist_hitbox: [i32; 2],
    /// Taube (both variants).
    pub taube_hitbox: [i32; 2],
    /// Mozartkugel-Verkäufer.
    pub mozartkugel_verkaeufer_hitbox: [i32; 2],
    /// Fiaker.
    pub fiaker_hitbox: [i32; 2],
}

impl Tuning {
    /// Parses and validates `tuning.toml` text.
    pub fn parse(text: &str) -> Result<Tuning, ParseError> {
        let t: Tuning = toml::from_str(text)
            .map_err(|e| ParseError::new(format!("tuning.toml: {}", e.to_string().trim())))?;
        t.validate()?;
        Ok(t)
    }

    fn validate(&self) -> Result<(), ParseError> {
        let p = &self.player;
        let positive = [
            ("player.walk_speed", p.walk_speed),
            ("player.run_speed", p.run_speed),
            ("player.ground_accel", p.ground_accel),
            ("player.friction", p.friction),
            ("player.skid_decel", p.skid_decel),
            ("player.air_accel", p.air_accel),
            ("player.jump_speed_walk", p.jump_speed_walk),
            ("player.jump_speed_run", p.jump_speed_run),
            ("player.gravity_hold", p.gravity_hold),
            ("player.gravity", p.gravity),
            ("player.max_fall_speed", p.max_fall_speed),
        ];
        for (name, v) in positive {
            if v <= Fx::ZERO {
                return Err(ParseError::new(format!("tuning.toml: {name} must be > 0")));
            }
        }
        // Content sanity bound, not a collision limit (tile collision checks
        // every tile crossed): anything this fast is a typo in tuning.toml.
        let max_step = Fx::from_px(12);
        for (name, v) in [
            ("player.run_speed", p.run_speed),
            ("player.jump_speed_run", p.jump_speed_run),
            ("player.jump_speed_walk", p.jump_speed_walk),
            ("player.max_fall_speed", p.max_fall_speed),
        ] {
            if v >= max_step {
                return Err(ParseError::new(format!(
                    "tuning.toml: {name} must be < 12 px/frame (3072)"
                )));
            }
        }
        if p.walk_speed > p.run_speed {
            return Err(ParseError::new(
                "tuning.toml: player.walk_speed must be <= player.run_speed",
            ));
        }
        if p.gravity_hold > p.gravity {
            return Err(ParseError::new(
                "tuning.toml: player.gravity_hold must be <= player.gravity",
            ));
        }
        for (name, v) in [
            ("player.coyote_frames", p.coyote_frames),
            ("player.jump_buffer_frames", p.jump_buffer_frames),
        ] {
            if v > 60 {
                return Err(ParseError::new(format!(
                    "tuning.toml: {name} must be <= 60"
                )));
            }
        }
        if !(4..=16).contains(&p.hitbox_w) || !(4..=32).contains(&p.hitbox_h) {
            return Err(ParseError::new(
                "tuning.toml: need 4 <= player.hitbox_w <= 16 and 4 <= player.hitbox_h <= 32",
            ));
        }
        // Every gap a Normal player fits in (whole tiles on the 16 px grid)
        // must fit the Zuckerschock too, so growing always finds room
        // (`Player::fit_head`).
        let gap = (p.hitbox_h + 15) / 16 * 16;
        if !(p.hitbox_h..=gap).contains(&p.hitbox_h_big) {
            return Err(ParseError::new(
                "tuning.toml: need player.hitbox_h <= player.hitbox_h_big <= player.hitbox_h \
                 rounded up to whole tiles (16 px)",
            ));
        }
        for (name, v) in [
            ("player.stomp_bounce", p.stomp_bounce),
            ("player.stomp_bounce_held", p.stomp_bounce_held),
        ] {
            if v <= Fx::ZERO || v >= max_step {
                return Err(ParseError::new(format!(
                    "tuning.toml: {name} must be > 0 and < 12 px/frame (3072)"
                )));
            }
        }
        if p.transform_frames > 120 {
            return Err(ParseError::new(
                "tuning.toml: player.transform_frames must be <= 120",
            ));
        }
        let g = &self.game;
        if g.lives_start == 0 || g.lives_start > g.lives_max || g.lives_max > 99 {
            return Err(ParseError::new(
                "tuning.toml: need 1 <= game.lives_start <= game.lives_max <= 99",
            ));
        }
        if g.oblaten_per_life == 0 || g.oblaten_per_life > 100 {
            return Err(ParseError::new(
                "tuning.toml: game.oblaten_per_life must be 1..=100",
            ));
        }
        if g.activate_margin < 0 || g.despawn_margin < 0 {
            return Err(ParseError::new(
                "tuning.toml: game.activate_margin and game.despawn_margin must be >= 0",
            ));
        }
        let i = &self.items;
        let e = &self.enemies;
        for (name, [w, h]) in [
            ("items.oblate_hitbox", i.oblate_hitbox),
            ("items.mozartkugel_hitbox", i.mozartkugel_hitbox),
            ("items.zaunerstollen_hitbox", i.zaunerstollen_hitbox),
            ("enemies.selfie_tourist_hitbox", e.selfie_tourist_hitbox),
            ("enemies.taube_hitbox", e.taube_hitbox),
            (
                "enemies.mozartkugel_verkaeufer_hitbox",
                e.mozartkugel_verkaeufer_hitbox,
            ),
            ("enemies.fiaker_hitbox", e.fiaker_hitbox),
            (
                "platforms.festungsbahn_size",
                self.platforms.festungsbahn_size,
            ),
        ] {
            if !(1..=64).contains(&w) || !(1..=64).contains(&h) {
                return Err(ParseError::new(format!(
                    "tuning.toml: {name} must be 1..=64 px in both directions"
                )));
            }
        }
        for (name, v) in [
            ("items.rise_speed", i.rise_speed),
            ("items.mozartkugel_speed", i.mozartkugel_speed),
            ("items.zaunerstollen_speed", i.zaunerstollen_speed),
            ("items.pop_speed", i.pop_speed),
            ("items.debris_speed_high", i.debris_speed_high),
            ("items.debris_speed_low", i.debris_speed_low),
        ] {
            if v <= Fx::ZERO || v >= max_step {
                return Err(ParseError::new(format!(
                    "tuning.toml: {name} must be > 0 and < 12 px/frame (3072)"
                )));
            }
        }
        if i.bump_frames < 2 || !(0..=TILE_PX).contains(&i.bump_px) {
            return Err(ParseError::new(
                "tuning.toml: need items.bump_frames >= 2 and 0 <= items.bump_px <= 16",
            ));
        }
        if i.multi_oblaten == 0 {
            return Err(ParseError::new(
                "tuning.toml: items.multi_oblaten must be >= 1",
            ));
        }
        // Frame counts where 0 would break behaviour (a power-up that never
        // starts rolling, an Oblate that never pops, a window that is over
        // before it starts, a death without a dying phase).
        for (name, v) in [
            ("items.rise_frames", i.rise_frames),
            ("items.pop_frames", i.pop_frames),
            ("items.multi_frames", i.multi_frames),
            ("game.death_frames", g.death_frames),
        ] {
            if v == 0 {
                return Err(ParseError::new(format!("tuning.toml: {name} must be >= 1")));
            }
        }
        if !(0..=TILE_PX * 4).contains(&self.score.popup_rise) {
            return Err(ParseError::new(
                "tuning.toml: score.popup_rise must be 0..=64 px",
            ));
        }
        if self.score.popup_frames == 0 {
            return Err(ParseError::new(
                "tuning.toml: score.popup_frames must be >= 1",
            ));
        }
        let c = &self.camera;
        if !(0..crate::camera::SCREEN_W).contains(&c.follow_x) {
            return Err(ParseError::new(
                "tuning.toml: camera.follow_x must be on screen",
            ));
        }
        if !(0 <= c.deadzone_top
            && c.deadzone_top <= c.deadzone_bottom
            && c.deadzone_bottom < crate::camera::SCREEN_H)
        {
            return Err(ParseError::new(
                "tuning.toml: need 0 <= camera.deadzone_top <= camera.deadzone_bottom < 180",
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const REAL: &str = include_str!("../../../content/tuning.toml");

    #[test]
    fn parses_content_tuning() {
        let t = Tuning::parse(REAL).unwrap();
        assert!(t.player.walk_speed < t.player.run_speed);
        assert_eq!(t.player.coyote_frames, 6);
        assert_eq!(t.player.jump_buffer_frames, 6);
    }

    #[test]
    fn rejects_unknown_and_missing_fields() {
        let unknown = REAL.replace("[game]", "[game]\nbogus = 1");
        let err = Tuning::parse(&unknown).unwrap_err().to_string();
        assert!(err.contains("bogus") && err.contains("line"), "{err}");

        let syntax = REAL.replace("[game]", "[game");
        let err = Tuning::parse(&syntax).unwrap_err().to_string();
        assert!(err.contains("line"), "{err}");

        let missing = REAL.replace("gravity_hold", "#gravity_hold");
        let err = Tuning::parse(&missing).unwrap_err().to_string();
        assert!(err.contains("gravity_hold"), "{err}");
    }

    #[test]
    fn rejects_bad_values() {
        let neg = REAL.replace("friction = ", "friction = -");
        assert!(Tuning::parse(&neg).is_err());
        let float = REAL.replace("friction = ", "friction = 0.5 #");
        assert!(Tuning::parse(&float).is_err(), "floats are not allowed");
        let fast = REAL.replace("max_fall_speed = ", "max_fall_speed = 9999 #");
        assert!(Tuning::parse(&fast).is_err());
        let gravity = REAL.replace("gravity_hold = ", "gravity_hold = 500 #");
        assert!(Tuning::parse(&gravity).is_err(), "gravity_hold > gravity");
        for field in ["coyote_frames", "jump_buffer_frames"] {
            let at_limit = REAL.replace(&format!("{field} = "), &format!("{field} = 60 #"));
            assert!(Tuning::parse(&at_limit).is_ok(), "{field} = 60");
            let long = REAL.replace(&format!("{field} = "), &format!("{field} = 61 #"));
            assert!(Tuning::parse(&long).is_err(), "{field} = 61");
            let overflow = REAL.replace(&format!("{field} = "), &format!("{field} = 255 #"));
            assert!(Tuning::parse(&overflow).is_err(), "{field} = 255");
        }
        for (field, bad) in [
            ("rise_frames", 0),
            ("pop_frames", 0),
            ("multi_frames", 0),
            ("death_frames", 0),
            ("bump_frames", 1),
            ("multi_oblaten", 0),
            ("popup_frames", 0),
            ("lives_start", 0),
            ("festungsbahn_size", -1),
            ("hitbox_w", 3),
            ("hitbox_w", 17),
            ("hitbox_h", 3),
            ("hitbox_h", 33),
            ("hitbox_h", 16),
            ("hitbox_h_big", 21),
            ("hitbox_h_big", 33),
        ] {
            let t = REAL.replace(&format!("{field} = "), &format!("{field} = {bad} #"));
            assert!(Tuning::parse(&t).is_err(), "{field} = {bad}");
        }
    }
}
