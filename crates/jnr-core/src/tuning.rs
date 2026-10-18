//! Gameplay constants from `content/tuning.toml`.
//!
//! Speeds are in subpixels (1/256 px) per frame, accelerations in subpixels
//! per frame², durations in frames, camera values in whole pixels.

use serde::Deserialize;

use crate::ParseError;
use crate::fx::Fx;

/// All tunable gameplay numbers. Parsed with [`Tuning::parse`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Tuning {
    /// Player movement.
    pub player: PlayerTuning,
    /// Camera follow behaviour.
    pub camera: CameraTuning,
    /// Game flow (timers).
    pub game: GameTuning,
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
    /// Hitbox height in whole pixels.
    pub hitbox_h: i32,
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
            ("hitbox_w", 3),
            ("hitbox_w", 17),
            ("hitbox_h", 3),
            ("hitbox_h", 33),
        ] {
            let t = REAL.replace(&format!("{field} = "), &format!("{field} = {bad} #"));
            assert!(Tuning::parse(&t).is_err(), "{field} = {bad}");
        }
    }
}
