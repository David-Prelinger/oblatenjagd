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
    /// Enemy hitboxes and behaviour.
    pub enemies: EnemyTuning,
    /// Moving platforms.
    pub platforms: PlatformTuning,
    /// The Maibaum goal sequence.
    pub goal: GoalTuning,
    /// The route map between stages (D-043).
    pub map: MapTuning,
}

/// The route map between stages (`docs/game-design.md` → "Route map").
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MapTuning {
    /// Frames Resi stands on the cleared station before walking.
    pub arrive_frames: u16,
    /// Frame of the Arrive phase at which the cleared station's marker
    /// turns `done` (renderer only).
    pub done_frame: u16,
    /// Walking speed along the route, whole px per frame.
    pub walk_px: u16,
    /// Frames of the Wait phase before the next stage starts on its own.
    pub wait_frames: u16,
    /// Frames of „Fortsetzung folgt …" before the title (teaser).
    pub teaser_frames: u16,
}

/// The Maibaum goal sequence (`docs/game-design.md` → "Goal sequence").
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GoalTuning {
    /// Feet heights above the pole base (whole px) where the next score
    /// zone starts, strictly increasing.
    pub zone_heights: [i32; 4],
    /// Points per zone (below the first height, ..., at or above the last
    /// height = the Wurstkranz).
    pub zone_scores: [u32; 5],
    /// The player's hitbox right edge stays this many px left of the pole
    /// centre while sliding (and the left edge as far right of it when
    /// hopping off).
    pub pole_offset: i32,
    /// Slide speed down the pole.
    pub slide_speed: Fx,
    /// Upward speed of the hop off the pole base (player gravity applies).
    pub hop_speed: Fx,
    /// Frames the stage-clear texts stay after the time-bonus tally.
    pub clear_frames: u16,
    /// Frames of „Fortsetzung folgt …" before the title screen (M1).
    pub to_be_continued_frames: u16,
}

/// Moving platforms.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlatformTuning {
    /// Festungsbahn car `[w, h]` in whole pixels (feet at the bottom
    /// centre, left-aligned in its spawn cell).
    pub festungsbahn_size: [i32; 2],
    /// Frames from one end point to the other.
    pub festungsbahn_travel: u16,
    /// Frames standing at the upper end point (at the lower end the car
    /// waits for its rider, D-030).
    pub festungsbahn_wait: u16,
    /// Gondel cabin `[w, h]` in whole pixels: the hitbox is the cabin, its
    /// top the standing surface (left-aligned in its spawn cell, bottom on
    /// the cell's bottom edge, like the Festungsbahn).
    pub gondel_size: [i32; 2],
    /// Frames a cabin waits at each end of its track (it never waits for a
    /// rider, D-044).
    pub gondel_wait: u16,
    /// Travel frames per tile along the track's major axis (16 = 1 px per
    /// frame): travel = max(|dx|, |dy|) × this.
    pub gondel_frames_per_tile: u16,
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
    /// Whole pixels the player's feet sink below a water tile's top before
    /// water kills (D-050; spikes kill on touch).
    pub water_sink_px: i32,
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
    /// Climb camera (`[meta] camera = "climb"`, D-054): screen y the feet
    /// settle at while grounded.
    pub climb_feet_y: i32,
    /// Climb camera: most px per frame the camera moves while settling.
    pub climb_settle_speed: i32,
    /// Climb camera: in the air, the camera follows up at once when the
    /// hitbox top is above this screen y.
    pub climb_top: i32,
    /// Climb camera: in the air it follows the feet down at most this many
    /// px below the settle framing of the last ground.
    pub climb_fall_follow: i32,
    /// Climb camera: screen y the feet settle at during the goal sequence.
    pub climb_goal_feet_y: i32,
    /// Climb levels: a level entity activates only while its spawn cell is
    /// at most this many px above or below the screen.
    pub climb_activate_y: i32,
}

/// Game flow constants.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GameTuning {
    /// Frames between dying and respawning.
    pub death_frames: u16,
    /// Frames in `GameOver` before START restarts the stage.
    pub game_over_frames: u16,
    /// Frames of the stage intro screen (at the start and after each death,
    /// title flow only).
    pub intro_frames: u16,
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
    /// An enemy defeated by bumping the block it stands on.
    pub bump_enemy: u32,
    /// Time bonus per second left at the goal (tallied one second per frame).
    pub time_bonus: u32,
    /// Frames a score popup is shown.
    pub popup_frames: u16,
    /// How far a popup floats up over its lifetime, whole pixels.
    pub popup_rise: i32,
}

/// Enemies (`docs/game-design.md` → "Entities and behaviour"). Hitboxes
/// `[w, h]` are whole pixels (feet at the bottom centre); distances to the
/// player are horizontal, centre to centre, in whole pixels; durations are
/// frames, counted from activation (D-017).
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
    /// The thrown Mozartkugel.
    pub thrown_hitbox: [i32; 2],
    /// Nordic-Walker.
    pub nordic_walker_hitbox: [i32; 2],
    /// Blasmusiker.
    pub blasmusiker_hitbox: [i32; 2],
    /// A Blasmusik note.
    pub note_hitbox: [i32; 2],
    /// E-Biker.
    pub e_biker_hitbox: [i32; 2],
    /// Bienenschwarm (centred on its path).
    pub bienenschwarm_hitbox: [i32; 2],
    /// Selfie-Tourist walking speed.
    pub tourist_speed: Fx,
    /// A selfie starts every this many frames after activation ...
    pub tourist_selfie_period: u16,
    /// ... and lasts this many frames (standing still).
    pub tourist_selfie_frames: u16,
    /// Frame of the selfie at which the flash starts ...
    pub tourist_flash_at: u16,
    /// ... and how many frames it shows.
    pub tourist_flash_frames: u16,
    /// Taube flying speed (to the left).
    pub taube_speed: Fx,
    /// Taube sine wave amplitude, whole pixels.
    pub taube_amplitude: i32,
    /// Taube sine wave period.
    pub taube_period: u16,
    /// A sitting Taube takes off when the player is this close (px).
    pub taube_sitting_trigger: i32,
    /// How far it rises before flying (px) ...
    pub taube_rise: i32,
    /// ... over this many frames.
    pub taube_rise_frames: u16,
    /// The Verkäufer's throw counter runs while the player is this close (px).
    pub verkaeufer_range: i32,
    /// Frames of that counter per throw ...
    pub verkaeufer_period: u16,
    /// ... the last this many winding up.
    pub verkaeufer_windup: u16,
    /// Frames the throw pose shows after a throw.
    pub verkaeufer_throw_pose: u16,
    /// Horizontal speed of a thrown Mozartkugel.
    pub thrown_speed_x: Fx,
    /// Upward speed of a thrown Mozartkugel at the throw.
    pub thrown_speed_y: Fx,
    /// Gravity of a thrown Mozartkugel.
    pub thrown_gravity: Fx,
    /// The parked Fiaker telegraphs when the player is this close (px) ...
    pub fiaker_trigger: i32,
    /// ... for this many frames ...
    pub fiaker_telegraph: u16,
    /// ... then charges left at this speed.
    pub fiaker_speed: Fx,
    /// Nordic-Walker walking speed (it turns at walls, enemies and ledges).
    pub walker_speed: Fx,
    /// The Blasmusiker's bar counter runs while the player is this close (px).
    pub blasmusiker_range: i32,
    /// Frames of one bar of that counter ...
    pub blasmusiker_bar: u16,
    /// ... the first this many inhaling (the telegraph) ...
    pub blasmusiker_inhale: u16,
    /// ... and a note is blown at each of these bar frames (strictly
    /// increasing, after the inhale, inside the bar).
    pub blasmusiker_notes: [u16; 3],
    /// Frames the blow pose shows from each note on.
    pub blasmusiker_blow_pose: u16,
    /// Note speed (straight towards the musician's facing side).
    pub note_speed: Fx,
    /// A note vanishes after flying this far (px).
    pub note_range: i32,
    /// Note centre height above the musician's feet (px) ...
    pub note_height: i32,
    /// ... and how far in front of his centre it starts (px).
    pub note_ahead: i32,
    /// The parked E-Biker telegraphs when the player is this close (px) ...
    pub e_biker_trigger: i32,
    /// ... for this many frames ...
    pub e_biker_telegraph: u16,
    /// ... then rides left at this speed (player gravity and max fall speed).
    pub e_biker_speed: Fx,
    /// Frames the crashed E-Biker shows before it is removed.
    pub e_biker_crash_frames: u16,
    /// Frames of a swarm's half-circle return to its anchor (the waggle
    /// run itself is 1 px per frame).
    pub swarm_return_frames: u16,
    /// Renderer only: the swarm sprite shakes this many px across the run ...
    pub swarm_shake: i32,
    /// ... switching sides every this many frames.
    pub swarm_shake_frames: u16,
    /// Frames a stomped enemy stays visible (squished).
    pub squish_frames: u16,
    /// Upward speed of an enemy or item knocked by a bumped block.
    pub knock_speed: Fx,
    /// Frames in which non-stompable enemies do no harm after a bounce off
    /// one, when the player's feet could not be put on its top (a solid
    /// or hazard tile was in the way, D-027 fallback). At least 2 (it is
    /// counted down in the frame it is set).
    pub bounce_grace_frames: u16,
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
        // Every gap a Normal player fits in (whole tiles on the tile grid)
        // must fit the Zuckerschock too, so growing always finds room
        // (`Player::fit_head`).
        let gap = (p.hitbox_h + TILE_PX - 1) / TILE_PX * TILE_PX;
        if !(p.hitbox_h..=gap).contains(&p.hitbox_h_big) {
            return Err(ParseError::new(format!(
                "tuning.toml: need player.hitbox_h <= player.hitbox_h_big <= player.hitbox_h \
                 rounded up to whole tiles ({TILE_PX} px)"
            )));
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
        if !(1..=TILE_PX).contains(&p.water_sink_px) {
            return Err(ParseError::new(format!(
                "tuning.toml: player.water_sink_px must be 1..={TILE_PX}"
            )));
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
            ("enemies.thrown_hitbox", e.thrown_hitbox),
            ("enemies.nordic_walker_hitbox", e.nordic_walker_hitbox),
            ("enemies.blasmusiker_hitbox", e.blasmusiker_hitbox),
            ("enemies.note_hitbox", e.note_hitbox),
            ("enemies.e_biker_hitbox", e.e_biker_hitbox),
            ("enemies.bienenschwarm_hitbox", e.bienenschwarm_hitbox),
            (
                "platforms.festungsbahn_size",
                self.platforms.festungsbahn_size,
            ),
            ("platforms.gondel_size", self.platforms.gondel_size),
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
            ("enemies.tourist_speed", e.tourist_speed),
            ("enemies.taube_speed", e.taube_speed),
            ("enemies.thrown_speed_x", e.thrown_speed_x),
            ("enemies.thrown_speed_y", e.thrown_speed_y),
            ("enemies.thrown_gravity", e.thrown_gravity),
            ("enemies.fiaker_speed", e.fiaker_speed),
            ("enemies.knock_speed", e.knock_speed),
            ("enemies.walker_speed", e.walker_speed),
            ("enemies.note_speed", e.note_speed),
            ("enemies.e_biker_speed", e.e_biker_speed),
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
            ("enemies.tourist_selfie_period", e.tourist_selfie_period),
            ("enemies.taube_period", e.taube_period),
            ("enemies.taube_rise_frames", e.taube_rise_frames),
            ("enemies.verkaeufer_period", e.verkaeufer_period),
            ("enemies.fiaker_telegraph", e.fiaker_telegraph),
            ("enemies.squish_frames", e.squish_frames),
            ("enemies.blasmusiker_bar", e.blasmusiker_bar),
            ("enemies.blasmusiker_blow_pose", e.blasmusiker_blow_pose),
            ("enemies.e_biker_telegraph", e.e_biker_telegraph),
            ("enemies.e_biker_crash_frames", e.e_biker_crash_frames),
            ("enemies.swarm_shake_frames", e.swarm_shake_frames),
            (
                "platforms.gondel_frames_per_tile",
                self.platforms.gondel_frames_per_tile,
            ),
            (
                "platforms.festungsbahn_travel",
                self.platforms.festungsbahn_travel,
            ),
            ("game.intro_frames", g.intro_frames),
            ("goal.clear_frames", self.goal.clear_frames),
            (
                "goal.to_be_continued_frames",
                self.goal.to_be_continued_frames,
            ),
        ] {
            if v == 0 {
                return Err(ParseError::new(format!("tuning.toml: {name} must be >= 1")));
            }
        }
        let m = &self.map;
        for (name, v) in [
            ("map.arrive_frames", m.arrive_frames),
            ("map.wait_frames", m.wait_frames),
            ("map.teaser_frames", m.teaser_frames),
        ] {
            if v == 0 {
                return Err(ParseError::new(format!("tuning.toml: {name} must be >= 1")));
            }
        }
        if m.done_frame > m.arrive_frames {
            return Err(ParseError::new(
                "tuning.toml: map.done_frame must be <= map.arrive_frames",
            ));
        }
        if !(1..=8).contains(&m.walk_px) {
            return Err(ParseError::new(
                "tuning.toml: map.walk_px must be 1..=8 px/frame",
            ));
        }
        // Counted down in the frame it is set: 1 would protect no frame.
        if e.bounce_grace_frames < 2 {
            return Err(ParseError::new(
                "tuning.toml: enemies.bounce_grace_frames must be >= 2",
            ));
        }
        if e.tourist_selfie_frames >= e.tourist_selfie_period
            || e.tourist_flash_at.saturating_add(e.tourist_flash_frames) > e.tourist_selfie_frames
        {
            return Err(ParseError::new(
                "tuning.toml: need enemies.tourist_selfie_frames < tourist_selfie_period and tourist_flash_at + tourist_flash_frames <= tourist_selfie_frames",
            ));
        }
        let pl = &self.platforms;
        if u32::from(pl.festungsbahn_wait) + u32::from(pl.festungsbahn_travel) > 32_767 {
            return Err(ParseError::new(
                "tuning.toml: platforms.festungsbahn_wait + festungsbahn_travel must be <= 32767 frames",
            ));
        }
        // A cabin's whole cycle (longest track: 64 tiles) fits the u16
        // frame counter.
        let gondel_period = 2
            * (u32::from(pl.gondel_wait)
                + u32::from(pl.gondel_frames_per_tile) * crate::level::MAX_TRACK_TILES as u32);
        // (`gondel_frames_per_tile` <= 64 also keeps `place_gondel`'s
        // delta × progress product inside i32.)
        if gondel_period > u32::from(u16::MAX) || pl.gondel_frames_per_tile > 64 {
            return Err(ParseError::new(
                "tuning.toml: need platforms.gondel_frames_per_tile <= 64 and 2 × (gondel_wait + 64 × gondel_frames_per_tile) <= 65535 frames",
            ));
        }
        let n = e.blasmusiker_notes;
        if n[0] < e.blasmusiker_inhale
            || n.windows(2).any(|w| w[0] >= w[1])
            || n[2].saturating_add(e.blasmusiker_blow_pose) > e.blasmusiker_bar
        {
            return Err(ParseError::new(
                "tuning.toml: need enemies.blasmusiker_inhale <= blasmusiker_notes (strictly increasing) and the last note + blasmusiker_blow_pose <= blasmusiker_bar",
            ));
        }
        if !(2..=1024).contains(&e.swarm_return_frames) {
            return Err(ParseError::new(
                "tuning.toml: enemies.swarm_return_frames must be 2..=1024",
            ));
        }
        for (name, v) in [
            ("enemies.note_range", e.note_range),
            ("enemies.note_height", e.note_height),
            ("enemies.note_ahead", e.note_ahead),
            ("enemies.swarm_shake", e.swarm_shake),
        ] {
            if !(0..=crate::camera::SCREEN_W).contains(&v) {
                return Err(ParseError::new(format!(
                    "tuning.toml: {name} must be 0..=320 px"
                )));
            }
        }
        if e.verkaeufer_windup.saturating_add(e.verkaeufer_throw_pose) >= e.verkaeufer_period {
            return Err(ParseError::new(
                "tuning.toml: need enemies.verkaeufer_windup + verkaeufer_throw_pose < verkaeufer_period",
            ));
        }
        for (name, v) in [
            ("enemies.taube_amplitude", e.taube_amplitude),
            ("enemies.taube_rise", e.taube_rise),
        ] {
            if !(0..=TILE_PX * 8).contains(&v) {
                return Err(ParseError::new(format!(
                    "tuning.toml: {name} must be 0..=128 px"
                )));
            }
        }
        for (name, v) in [
            ("enemies.taube_sitting_trigger", e.taube_sitting_trigger),
            ("enemies.verkaeufer_range", e.verkaeufer_range),
            ("enemies.fiaker_trigger", e.fiaker_trigger),
            ("enemies.blasmusiker_range", e.blasmusiker_range),
            ("enemies.e_biker_trigger", e.e_biker_trigger),
        ] {
            if !(0..=crate::camera::SCREEN_W * 2).contains(&v) {
                return Err(ParseError::new(format!(
                    "tuning.toml: {name} must be 0..=640 px"
                )));
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
        let goal = &self.goal;
        if goal.zone_heights[0] <= 0 || goal.zone_heights.windows(2).any(|w| w[0] >= w[1]) {
            return Err(ParseError::new(
                "tuning.toml: goal.zone_heights must be > 0 and strictly increasing",
            ));
        }
        if !(0..=TILE_PX / 2).contains(&goal.pole_offset) {
            return Err(ParseError::new(
                "tuning.toml: goal.pole_offset must be 0..=8 px",
            ));
        }
        for (name, v) in [
            ("goal.slide_speed", goal.slide_speed),
            ("goal.hop_speed", goal.hop_speed),
        ] {
            if v <= Fx::ZERO || v >= max_step {
                return Err(ParseError::new(format!(
                    "tuning.toml: {name} must be > 0 and < 12 px/frame (3072)"
                )));
            }
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
        let h = crate::camera::SCREEN_H;
        if !(0 <= c.climb_top && c.climb_top < c.climb_feet_y && c.climb_feet_y <= h) {
            return Err(ParseError::new(
                "tuning.toml: need 0 <= camera.climb_top < camera.climb_feet_y <= 180",
            ));
        }
        if !(1..=h).contains(&c.climb_goal_feet_y) {
            return Err(ParseError::new(
                "tuning.toml: camera.climb_goal_feet_y must be 1..=180",
            ));
        }
        if !(1..=TILE_PX).contains(&c.climb_settle_speed) {
            return Err(ParseError::new(
                "tuning.toml: camera.climb_settle_speed must be 1..=16 px/frame",
            ));
        }
        for (name, v) in [
            ("camera.climb_fall_follow", c.climb_fall_follow),
            ("camera.climb_activate_y", c.climb_activate_y),
        ] {
            if !(0..=h).contains(&v) {
                return Err(ParseError::new(format!(
                    "tuning.toml: {name} must be 0..=180 px"
                )));
            }
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
    fn rejects_unordered_goal_zones() {
        let t = REAL.replace("zone_heights = [32, 64,", "zone_heights = [32, 32,");
        assert!(Tuning::parse(&t).is_err());
        let t = REAL.replace("zone_heights = [32,", "zone_heights = [0,");
        assert!(Tuning::parse(&t).is_err());
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
            ("intro_frames", 0),
            ("clear_frames", 0),
            ("to_be_continued_frames", 0),
            ("slide_speed", 0),
            ("hop_speed", 0),
            ("pole_offset", 9),
            ("bump_frames", 1),
            ("multi_oblaten", 0),
            ("popup_frames", 0),
            ("lives_start", 0),
            ("festungsbahn_size", -1),
            ("festungsbahn_travel", 0),
            ("tourist_selfie_period", 0),
            ("tourist_selfie_frames", 240),
            ("tourist_flash_at", 47),
            ("taube_period", 0),
            ("taube_amplitude", -1),
            ("verkaeufer_windup", 120),
            ("fiaker_trigger", 641),
            ("fiaker_speed", 0),
            ("thrown_gravity", 0),
            ("squish_frames", 0),
            ("bounce_grace_frames", 0),
            ("bounce_grace_frames", 1),
            ("festungsbahn_wait", 32700),
            ("gondel_size", 0),
            ("gondel_frames_per_tile", 0),
            ("gondel_frames_per_tile", 65),
            ("gondel_wait", 32000),
            ("nordic_walker_hitbox", 65),
            ("blasmusiker_hitbox", 0),
            ("note_hitbox", 0),
            ("e_biker_hitbox", 65),
            ("bienenschwarm_hitbox", 0),
            ("walker_speed", 0),
            ("note_speed", 0),
            ("e_biker_speed", 3072),
            ("blasmusiker_range", 641),
            ("e_biker_trigger", -1),
            ("blasmusiker_bar", 0),
            ("blasmusiker_bar", 85),
            ("blasmusiker_inhale", 31),
            ("blasmusiker_blow_pose", 0),
            ("note_range", 321),
            ("e_biker_telegraph", 0),
            ("e_biker_crash_frames", 0),
            ("swarm_return_frames", 1),
            ("swarm_return_frames", 1025),
            ("swarm_shake_frames", 0),
            ("hitbox_w", 3),
            ("hitbox_w", 17),
            ("hitbox_h", 3),
            ("hitbox_h", 33),
            ("hitbox_h", 16),
            ("hitbox_h_big", 21),
            ("hitbox_h_big", 33),
            ("water_sink_px", 0),
            ("water_sink_px", 17),
            ("arrive_frames", 0),
            ("done_frame", 41),
            ("walk_px", 0),
            ("walk_px", 9),
            ("wait_frames", 0),
            ("teaser_frames", 0),
            ("climb_feet_y", 181),
            ("climb_feet_y", 40),
            ("climb_top", -1),
            ("climb_settle_speed", 0),
            ("climb_settle_speed", 17),
            ("climb_fall_follow", -1),
            ("climb_fall_follow", 181),
            ("climb_goal_feet_y", 0),
            ("climb_goal_feet_y", 181),
            ("climb_activate_y", -1),
            ("climb_activate_y", 181),
        ]
        .map(|(f, v)| (f, v.to_string()))
        .into_iter()
        .chain([("blasmusiker_notes", "[30, 30, 78]".to_string())])
        {
            let t = REAL.replace(&format!("{field} = "), &format!("{field} = {bad} #"));
            assert!(Tuning::parse(&t).is_err(), "{field} = {bad}");
        }
    }
}
