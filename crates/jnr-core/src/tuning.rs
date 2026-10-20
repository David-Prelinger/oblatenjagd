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
    /// The Hackl power-up and the thrown axes (D-056).
    pub hackl: HacklTuning,
    /// The Durchkriechstein crawl (D-057).
    pub crawl: CrawlTuning,
}

/// The Hackl power-up and its thrown axes (`docs/game-design.md` →
/// "Stage 4: Falkenstein", D-056). The item's rise is the other power-ups'
/// (`items.rise_frames`, `items.rise_speed`), its points `score.power_up`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HacklTuning {
    /// The `hackl` item's hitbox `[w, h]`, whole pixels.
    pub item_hitbox: [i32; 2],
    /// A thrown axe's hitbox `[w, h]`, whole pixels.
    pub axe_hitbox: [i32; 2],
    /// Most axes alive at once; a RUN press with this many alive does nothing.
    pub max_alive: u8,
    /// Horizontal speed of a thrown axe (towards the facing side).
    pub throw_speed_x: Fx,
    /// Upward speed of a thrown axe at the throw.
    pub throw_speed_y: Fx,
    /// Gravity on a thrown axe.
    pub gravity: Fx,
    /// Maximum downward speed of a thrown axe.
    pub max_fall_speed: Fx,
    /// The axe's centre at the throw: this many px in front of the
    /// player's centre ...
    pub throw_ahead: i32,
    /// ... and this many px below her hitbox top.
    pub throw_below_top: i32,
    /// Frames of the throw pose.
    pub throw_pose_frames: u8,
    /// Frames an axe stays stuck in a solid tile (harmless) before it is removed.
    pub stuck_frames: u16,
}

/// The Durchkriechstein crawl (`docs/game-design.md` → "Stage 4:
/// Falkenstein", D-057).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CrawlTuning {
    /// The player's hitbox `[w, h]` while crawling, whole pixels (fits the
    /// 1-tile tunnel).
    pub hitbox: [i32; 2],
    /// Crawl speed to the right.
    pub speed: Fx,
    /// The crawl starts when the hitbox's right edge is at most this many
    /// px left of the tunnel mouth.
    pub mouth_px: i32,
    /// Points instead of the +1 life when a Normal Resi crawls through at
    /// the lives cap.
    pub cap_points: u32,
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
    /// Schafbergbahn train body `[w, h]` in whole pixels, whole tiles
    /// (so with whole-tile `to` both roofs lie on the tile grid, flush with
    /// the station platforms; D-067). Left-aligned in its spawn cell, bottom
    /// on the cell's bottom edge, like the Festungsbahn.
    pub schafbergbahn_size: [i32; 2],
    /// Travel frames per tile along the track's major axis: travel =
    /// max(|dx|, |dy|) × this (24: 0.67 px/frame).
    pub schafbergbahn_frames_per_tile: u16,
    /// Frames standing at the upper end (at the lower end the train waits
    /// for its rider, like the Festungsbahn).
    pub schafbergbahn_wait: u16,
    /// Traundl hull `[w, h]` in whole pixels: its top is the deck
    /// (left-aligned in its spawn cell, bottom on the cell's bottom edge =
    /// the water line, like the Gondel; D-077).
    pub traundl_size: [i32; 2],
    /// Frames a Traundl waits at each end of its track (it never waits for
    /// a rider: the Gondel's free-running machine, D-077).
    pub traundl_wait: u16,
    /// Traundl travel frames per tile: travel = |dx| × this (16 = 1 px per
    /// frame).
    pub traundl_frames_per_tile: u16,
    /// Dampfer hull `[w, h]` in whole pixels, whole tiles (the deck flush
    /// with the quays; left-aligned in its spawn cell, bottom on the cell's
    /// bottom edge = the water line; D-077). Up to a screen wide.
    pub dampfer_size: [i32; 2],
    /// Dampfer travel frames per tile: travel = dx × this (20: 0.8 px per
    /// frame). It waits for its rider, travels once and stays moored.
    pub dampfer_frames_per_tile: u16,
    /// The paddle wheel's hurt zone `[w, h]` in px while the Dampfer
    /// travels, centred on the hull's centre x (D-077).
    pub dampfer_wheel_zone: [i32; 2],
    /// Px the wheel zone's top lies above the hull's bottom (the water
    /// line); the rest of its height is below it.
    pub dampfer_wheel_rise: i32,
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
    /// Climb camera (D-065): grounded, the framing stays while the feet are
    /// at most this many px above the ground it last settled on; higher, it
    /// settles up by the excess only.
    pub climb_up_slack: i32,
    /// Climb camera (D-065): the slack holds only while she stays within
    /// this many px (horizontally) of where she stepped up.
    pub climb_up_reach: i32,
    /// Climb camera: in the air, the camera follows up at once when the
    /// hitbox top is above this screen y.
    pub climb_top: i32,
    /// Climb camera (D-065): while the slack holds, the up-follow line is
    /// `climb_top` minus the slack's lag, but never above this screen y.
    pub climb_top_min: i32,
    /// Climb camera (D-065): grounded with the hitbox top above
    /// `climb_top`, the camera moves up this many px per frame more than
    /// the settle until it is below.
    pub climb_catch_up_speed: i32,
    /// Climb camera: in the air it follows the feet down at most this many
    /// px below the settle framing of the last ground.
    pub climb_fall_follow: i32,
    /// Climb camera: screen y the feet settle at during the goal sequence.
    pub climb_goal_feet_y: i32,
    /// Climb levels: a level entity activates only while its spawn cell is
    /// at most this many px above or below the screen.
    pub climb_activate_y: i32,
    /// Auto-scroll camera (`[meta] camera = "autoscroll"`, D-075): how far
    /// the scroll moves the camera every frame of play, subpixels.
    pub autoscroll_speed: Fx,
    /// Auto-scroll camera: frames of play the scroll waits after a start or
    /// respawn framed inside the zone.
    pub autoscroll_hold: u16,
    /// Auto-scroll camera: in the scroll the player's hitbox right edge
    /// stays at most at screen x 320 minus this.
    pub autoscroll_right_margin: i32,
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
    /// Pilger (both looks; `lead` only changes the sprite).
    pub pilger_hitbox: [i32; 2],
    /// Pilger walking speed (towards the player at activation).
    pub pilger_speed: Fx,
    /// Gams.
    pub gams_hitbox: [i32; 2],
    /// Frames a Gams stands before each crouch ...
    pub gams_stand: u16,
    /// ... and crouches (the telegraph) before each hop.
    pub gams_crouch: u16,
    /// Horizontal speed of a hop.
    pub gams_hop_speed_x: Fx,
    /// Upward speed at the start of a hop.
    pub gams_hop_speed_y: Fx,
    /// Gravity of a Gams (in a hop and when it falls).
    pub gams_gravity: Fx,
    /// How many tiles ahead the landing check looks (the hop's length).
    pub gams_hop_tiles: i32,
    /// The falling rock (`stein`).
    pub stein_hitbox: [i32; 2],
    /// Gravity of a falling rock ...
    pub stein_gravity: Fx,
    /// ... and its max fall speed.
    pub stein_max_fall: Fx,
    /// Steinschlag counter frames of the telegraph; the rock drops at this
    /// frame of each period. At most [`crate::level::STEINSCHLAG_MIN_PHASE`]
    /// − 1, so a level `phase` never skips into a telegraph.
    pub steinschlag_telegraph: u16,
    /// Frames after a drop until the niche shows a resting rock again.
    pub steinschlag_refill: u16,
    /// The niche counts as on screen only when its top is at least this
    /// many px below the screen top (the HUD).
    pub steinschlag_screen_top: i32,
    /// Points for a rock shattered by a projectile (the thrown Hackl).
    pub stein_points: u32,
    /// Operettensänger (D-068).
    pub operettensaenger_hitbox: [i32; 2],
    /// His counter runs while the player is this close (px) ...
    pub operettensaenger_range: i32,
    /// ... frames per cycle ...
    pub operettensaenger_cycle: u16,
    /// ... the first this many frames inhale (the telegraph) ...
    pub operettensaenger_inhale: u16,
    /// ... then the high C sounds for this many frames; idle to the end.
    pub operettensaenger_sing: u16,
    /// The high-C column: width (px, centred on him) ...
    pub hohes_c_width: i32,
    /// ... growth per frame from his hitbox top (px) ...
    pub hohes_c_speed: i32,
    /// ... and full height (px), unless a solid tile stops it first.
    pub hohes_c_height: i32,
    /// Souvenirstand counter frames of the telegraph; the globe spawns at
    /// this frame of each period. At most
    /// [`crate::level::SOUVENIRSTAND_MIN_PHASE`] − 1 (D-069).
    pub souvenirstand_telegraph: u16,
    /// At most this many of a stand's globes are alive at once.
    pub souvenirstand_max_alive: u16,
    /// The rolling snow globe.
    pub schneekugel_hitbox: [i32; 2],
    /// Its rolling speed (always to the left; gravity is the player's).
    pub schneekugel_speed: Fx,
    /// Reisebus (D-070).
    pub reisebus_hitbox: [i32; 2],
    /// The parked bus telegraphs when the player is this close to its
    /// centre (px) ...
    pub reisebus_trigger: i32,
    /// ... for this many frames ...
    pub reisebus_telegraph: u16,
    /// ... then drives left at this speed, ignoring tiles.
    pub reisebus_speed: Fx,
    /// Frames of the photo stop (a bus with `stop`) ...
    pub reisebus_stop_frames: u16,
    /// ... then this many frames of telegraph before it drives on.
    pub reisebus_restart: u16,
    /// Renderer: the camera flashes in its windows switch on and off every
    /// this many frames of the photo stop (off first).
    pub reisebus_flash_frames: u16,
    /// Schwan (D-078).
    pub schwan_hitbox: [i32; 2],
    /// Its hitbox bottom floats this many px below the water line.
    pub schwan_float_px: i32,
    /// A rested swan at home that is fully on screen triggers when the
    /// player is this close (px) ...
    pub schwan_range: i32,
    /// ... telegraphs (the hiss) for this many frames ...
    pub schwan_telegraph: u16,
    /// ... charges towards the player's side at this speed ...
    pub schwan_charge_speed: Fx,
    /// ... for this many frames ...
    pub schwan_charge_frames: u16,
    /// ... returns home at this speed ...
    pub schwan_return_speed: Fx,
    /// ... and rests this many frames before it can trigger again.
    pub schwan_rest: u16,
    /// Gelse (D-079).
    pub gelse_hitbox: [i32; 2],
    /// Frames fully on screen before it hunts (the telegraph) ...
    pub gelse_wait: u16,
    /// ... frames of the hunt, then it leaves (harmless) ...
    pub gelse_hunt: u16,
    /// ... per-axis velocity change per frame towards its target ...
    pub gelse_accel: Fx,
    /// ... per-axis speed limit ...
    pub gelse_speed: Fx,
    /// ... the target: the player's hitbox centre x and her hitbox top +
    /// this many px.
    pub gelse_target_dy: i32,
    /// A leaving Gelse flies straight up at this speed.
    pub gelse_leave_speed: Fx,
    /// Regattaboot (D-080).
    pub regattaboot_hitbox: [i32; 2],
    /// Its hitbox bottom (the keel) lies this many px below the water line.
    pub regattaboot_keel_px: i32,
    /// The masthead part `[w, h]` px: centred on the hitbox, from its top
    /// down; the only part that bounces a falling player. Below it the body
    /// spans the whole hitbox width (D-080 refinement, E29).
    pub regattaboot_mast: [i32; 2],
    /// The parked boat telegraphs when the player is this close to its
    /// centre (px) and it is fully on screen ...
    pub regattaboot_trigger: i32,
    /// ... for this many frames ...
    pub regattaboot_telegraph: u16,
    /// ... then races left at this speed, ignoring tiles.
    pub regattaboot_speed: Fx,
}

/// Frames of a Gams hop on flat ground until it lands (the frame whose
/// move would take it below its start height), moving like
/// `enemies::update_gams`: vy starts at −`up`, gravity is added before each
/// move, fall speed capped at `max_fall`. `None` if it takes more than 1000
/// frames.
pub fn gams_airtime(up: Fx, gravity: Fx, max_fall: Fx) -> Option<u32> {
    let (mut y, mut vy) = (0i64, -i64::from(up.raw()));
    for n in 1..=1000 {
        vy = (vy + i64::from(gravity.raw())).min(i64::from(max_fall.raw()));
        y += vy;
        if y > 0 {
            return Some(n);
        }
    }
    None
}

impl PlatformTuning {
    /// The M5 boat keys (Traundl, Dampfer; D-077).
    fn validate_boats(&self, scroll_speed: Fx) -> Result<(), ParseError> {
        // A Traundl's whole cycle fits the u16 frame counter (as the
        // Gondel's; `frames_per_tile` <= 64 keeps the position product in
        // i32).
        let period = 2
            * (u32::from(self.traundl_wait)
                + u32::from(self.traundl_frames_per_tile) * crate::level::MAX_TRACK_TILES as u32);
        if period > u32::from(u16::MAX) || self.traundl_frames_per_tile > 64 {
            return Err(ParseError::new(
                "tuning.toml: need platforms.traundl_frames_per_tile <= 64 and 2 × (traundl_wait + 64 × traundl_frames_per_tile) <= 65535 frames",
            ));
        }
        // The steamer's hull is whole tiles (its deck on the tile grid,
        // flush with the quays) and at most a screen wide.
        let [w, h] = self.dampfer_size;
        if !(TILE_PX..=crate::camera::SCREEN_W).contains(&w)
            || !(TILE_PX..=64).contains(&h)
            || w % TILE_PX != 0
            || h % TILE_PX != 0
        {
            return Err(ParseError::new(
                "tuning.toml: platforms.dampfer_size must be whole tiles, 16..=320 px wide and 16..=64 px high",
            ));
        }
        // At least as fast as the auto-scroll (a standing rider is never
        // pushed, D-077): 16 px × 256 / frames per tile >= the scroll speed.
        // <= 64 frames per tile keeps the travel (64 tiles) in the counter
        // and the position product in i32.
        let speed = TILE_PX * 256 / i32::from(self.dampfer_frames_per_tile.max(1));
        if self.dampfer_frames_per_tile > 64 || speed < scroll_speed.raw() {
            return Err(ParseError::new(format!(
                "tuning.toml: platforms.dampfer_frames_per_tile must be 1..=64 and keep the Dampfer at least as fast as the auto-scroll (camera.autoscroll_speed = {} subpx/frame: at most {} frames per tile)",
                scroll_speed.raw(),
                TILE_PX * 256 / scroll_speed.raw().max(1)
            )));
        }
        let [zw, zh] = self.dampfer_wheel_zone;
        if !(1..=w).contains(&zw)
            || !(1..=64).contains(&zh)
            || !(0..=zh).contains(&self.dampfer_wheel_rise)
        {
            return Err(ParseError::new(
                "tuning.toml: need platforms.dampfer_wheel_zone 1..=dampfer width × 1..=64 px and 0 <= dampfer_wheel_rise <= its height",
            ));
        }
        Ok(())
    }
}

impl EnemyTuning {
    /// The M3 enemy keys (Pilger, Gams, Steinschlag; D-058).
    /// `max_fall` is the player's max fall speed (the Gams falls with it).
    fn validate_m3(&self, max_fall: Fx) -> Result<(), ParseError> {
        for (name, [w, h]) in [
            ("enemies.pilger_hitbox", self.pilger_hitbox),
            ("enemies.gams_hitbox", self.gams_hitbox),
            ("enemies.stein_hitbox", self.stein_hitbox),
        ] {
            if !(1..=64).contains(&w) || !(1..=64).contains(&h) {
                return Err(ParseError::new(format!(
                    "tuning.toml: {name} must be 1..=64 px in both directions"
                )));
            }
        }
        for (name, v) in [
            ("enemies.pilger_speed", self.pilger_speed),
            ("enemies.gams_hop_speed_x", self.gams_hop_speed_x),
            ("enemies.gams_hop_speed_y", self.gams_hop_speed_y),
            ("enemies.gams_gravity", self.gams_gravity),
            ("enemies.stein_gravity", self.stein_gravity),
            ("enemies.stein_max_fall", self.stein_max_fall),
        ] {
            if v <= Fx::ZERO || v >= Fx::from_px(12) {
                return Err(ParseError::new(format!(
                    "tuning.toml: {name} must be > 0 and < 12 px/frame (3072)"
                )));
            }
        }
        for (name, v) in [
            ("enemies.gams_stand", self.gams_stand),
            ("enemies.gams_crouch", self.gams_crouch),
            ("enemies.steinschlag_refill", self.steinschlag_refill),
        ] {
            if v == 0 {
                return Err(ParseError::new(format!("tuning.toml: {name} must be >= 1")));
            }
        }
        if !(1..=4).contains(&self.gams_hop_tiles) {
            return Err(ParseError::new(
                "tuning.toml: enemies.gams_hop_tiles must be 1..=4",
            ));
        }
        // The landing check sweeps `gams_hop_tiles` tiles, so the hop must
        // not carry it further: simulate the hop on flat ground exactly as
        // `enemies::update_gams` moves (gravity before the move, `vx` every
        // frame including the landing one).
        let airtime = gams_airtime(self.gams_hop_speed_y, self.gams_gravity, max_fall);
        let across = airtime.map(|n| i64::from(self.gams_hop_speed_x.raw()) * i64::from(n));
        let swept = i64::from(Fx::from_px(self.gams_hop_tiles * TILE_PX).raw());
        if across.is_none_or(|a| a > swept) {
            return Err(ParseError::new(
                "tuning.toml: need enemies.gams_hop_speed_x × the hop's airtime (from gams_hop_speed_y and gams_gravity) <= gams_hop_tiles × 16 px, so the Gams never lands past the floor it checked",
            ));
        }
        // The level's `phase` is 0 or STEINSCHLAG_MIN_PHASE.., so the first
        // rock of an approach always shows the whole telegraph; drop and
        // refill fit the shortest period.
        let min_phase = crate::level::STEINSCHLAG_MIN_PHASE;
        let min_period = *crate::level::STEINSCHLAG_PERIOD_RANGE.start();
        if self.steinschlag_telegraph == 0
            || self.steinschlag_telegraph >= min_phase
            || self.steinschlag_telegraph + self.steinschlag_refill >= min_period
        {
            return Err(ParseError::new(format!(
                "tuning.toml: need 1 <= enemies.steinschlag_telegraph < {min_phase} and steinschlag_telegraph + steinschlag_refill < {min_period}"
            )));
        }
        if !(0..crate::camera::SCREEN_H).contains(&self.steinschlag_screen_top) {
            return Err(ParseError::new(
                "tuning.toml: enemies.steinschlag_screen_top must be 0..180 px",
            ));
        }
        Ok(())
    }

    /// The Reisebus keys (D-070).
    fn validate_reisebus(&self) -> Result<(), ParseError> {
        let [w, h] = self.reisebus_hitbox;
        if !(1..=64).contains(&w) || !(1..=64).contains(&h) {
            return Err(ParseError::new(
                "tuning.toml: enemies.reisebus_hitbox must be 1..=64 px in both directions",
            ));
        }
        if self.reisebus_speed <= Fx::ZERO || self.reisebus_speed >= Fx::from_px(12) {
            return Err(ParseError::new(
                "tuning.toml: enemies.reisebus_speed must be > 0 and < 12 px/frame (3072)",
            ));
        }
        if !(0..=crate::camera::SCREEN_W * 2).contains(&self.reisebus_trigger) {
            return Err(ParseError::new(
                "tuning.toml: enemies.reisebus_trigger must be 0..=640 px",
            ));
        }
        for (name, v) in [
            ("enemies.reisebus_telegraph", self.reisebus_telegraph),
            ("enemies.reisebus_stop_frames", self.reisebus_stop_frames),
            ("enemies.reisebus_restart", self.reisebus_restart),
            ("enemies.reisebus_flash_frames", self.reisebus_flash_frames),
        ] {
            if v == 0 {
                return Err(ParseError::new(format!("tuning.toml: {name} must be >= 1")));
            }
        }
        Ok(())
    }
}

impl EnemyTuning {
    /// The M5 lake enemies' keys (Schwan, Gelse, Regattaboot; D-078 …
    /// D-080).
    fn validate_lake(&self) -> Result<(), ParseError> {
        for (name, [w, h], max_w, max_h) in [
            ("enemies.schwan_hitbox", self.schwan_hitbox, 24, 24),
            ("enemies.gelse_hitbox", self.gelse_hitbox, TILE_PX, TILE_PX),
            (
                "enemies.regattaboot_hitbox",
                self.regattaboot_hitbox,
                32,
                64,
            ),
        ] {
            if !(1..=max_w).contains(&w) || !(1..=max_h).contains(&h) {
                return Err(ParseError::new(format!(
                    "tuning.toml: {name} must be 1..={max_w} px wide and 1..={max_h} px high"
                )));
            }
        }
        let max = Fx::from_px(12);
        for (name, v) in [
            ("enemies.schwan_charge_speed", self.schwan_charge_speed),
            ("enemies.schwan_return_speed", self.schwan_return_speed),
            ("enemies.gelse_speed", self.gelse_speed),
            ("enemies.gelse_leave_speed", self.gelse_leave_speed),
            ("enemies.regattaboot_speed", self.regattaboot_speed),
        ] {
            if v <= Fx::ZERO || v >= max {
                return Err(ParseError::new(format!(
                    "tuning.toml: {name} must be > 0 and < 12 px/frame (3072)"
                )));
            }
        }
        let [mw, mh] = self.regattaboot_mast;
        let [bw, bh] = self.regattaboot_hitbox;
        if !(1..=bw).contains(&mw) || !(1..bh).contains(&mh) {
            return Err(ParseError::new(
                "tuning.toml: enemies.regattaboot_mast must be 1..=hitbox wide and 1..hitbox high",
            ));
        }
        if self.gelse_accel <= Fx::ZERO || self.gelse_accel > self.gelse_speed {
            return Err(ParseError::new(
                "tuning.toml: need 0 < enemies.gelse_accel <= gelse_speed",
            ));
        }
        for (name, v, range) in [
            (
                "enemies.schwan_float_px",
                self.schwan_float_px,
                0..=self.schwan_hitbox[1],
            ),
            (
                "enemies.regattaboot_keel_px",
                self.regattaboot_keel_px,
                0..=TILE_PX.min(self.regattaboot_hitbox[1]),
            ),
            ("enemies.gelse_target_dy", self.gelse_target_dy, 0..=32),
            (
                "enemies.schwan_range",
                self.schwan_range,
                TILE_PX..=crate::camera::SCREEN_W,
            ),
            (
                "enemies.regattaboot_trigger",
                self.regattaboot_trigger,
                0..=crate::camera::SCREEN_W * 2,
            ),
        ] {
            if !range.contains(&v) {
                return Err(ParseError::new(format!(
                    "tuning.toml: {name} must be {}..={} px",
                    range.start(),
                    range.end()
                )));
            }
        }
        for (name, v) in [
            ("enemies.schwan_telegraph", self.schwan_telegraph),
            ("enemies.schwan_charge_frames", self.schwan_charge_frames),
            ("enemies.schwan_rest", self.schwan_rest),
            ("enemies.gelse_wait", self.gelse_wait),
            ("enemies.gelse_hunt", self.gelse_hunt),
            ("enemies.regattaboot_telegraph", self.regattaboot_telegraph),
        ] {
            if v == 0 {
                return Err(ParseError::new(format!("tuning.toml: {name} must be >= 1")));
            }
        }
        Ok(())
    }

    /// The M4 keys of the Operettensänger and the Souvenirstand (D-068,
    /// D-069).
    fn validate_singer_and_stand(&self) -> Result<(), ParseError> {
        for (name, [w, h]) in [
            (
                "enemies.operettensaenger_hitbox",
                self.operettensaenger_hitbox,
            ),
            ("enemies.schneekugel_hitbox", self.schneekugel_hitbox),
        ] {
            if !(1..=TILE_PX).contains(&w) || !(1..=32).contains(&h) {
                return Err(ParseError::new(format!(
                    "tuning.toml: {name} must be 1..=16 px wide and 1..=32 px high"
                )));
            }
        }
        if !(TILE_PX..=crate::camera::SCREEN_W).contains(&self.operettensaenger_range) {
            return Err(ParseError::new(
                "tuning.toml: enemies.operettensaenger_range must be 16..=320 px",
            ));
        }
        // Inhale, then the C, then at least one idle frame.
        if self.operettensaenger_inhale == 0
            || self.operettensaenger_sing == 0
            || self.operettensaenger_inhale + self.operettensaenger_sing
                >= self.operettensaenger_cycle
        {
            return Err(ParseError::new(
                "tuning.toml: need enemies.operettensaenger_inhale >= 1, operettensaenger_sing >= 1 and inhale + sing < operettensaenger_cycle",
            ));
        }
        if !(1..=32).contains(&self.hohes_c_width)
            || !(1..=TILE_PX).contains(&self.hohes_c_speed)
            || !(1..=160).contains(&self.hohes_c_height)
        {
            return Err(ParseError::new(
                "tuning.toml: need enemies.hohes_c_width 1..=32, hohes_c_speed 1..=16 and hohes_c_height 1..=160 px",
            ));
        }
        // The level's `phase` is 0 or SOUVENIRSTAND_MIN_PHASE.., so the first
        // globe of an approach always follows the whole telegraph.
        let min_phase = crate::level::SOUVENIRSTAND_MIN_PHASE;
        if self.souvenirstand_telegraph == 0 || self.souvenirstand_telegraph >= min_phase {
            return Err(ParseError::new(format!(
                "tuning.toml: need 1 <= enemies.souvenirstand_telegraph < {min_phase}"
            )));
        }
        if !(1..=8).contains(&self.souvenirstand_max_alive) {
            return Err(ParseError::new(
                "tuning.toml: enemies.souvenirstand_max_alive must be 1..=8",
            ));
        }
        if self.schneekugel_speed <= Fx::ZERO || self.schneekugel_speed >= Fx::from_px(12) {
            return Err(ParseError::new(
                "tuning.toml: enemies.schneekugel_speed must be > 0 and < 12 px/frame (3072)",
            ));
        }
        Ok(())
    }
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
        self.enemies.validate_m3(self.player.max_fall_speed)?;
        self.enemies.validate_singer_and_stand()?;
        self.enemies.validate_reisebus()?;
        self.enemies.validate_lake()?;
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
            (
                "platforms.schafbergbahn_size",
                self.platforms.schafbergbahn_size,
            ),
            ("platforms.traundl_size", self.platforms.traundl_size),
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
            (
                "platforms.schafbergbahn_frames_per_tile",
                self.platforms.schafbergbahn_frames_per_tile,
            ),
            (
                "platforms.traundl_frames_per_tile",
                self.platforms.traundl_frames_per_tile,
            ),
            (
                "platforms.dampfer_frames_per_tile",
                self.platforms.dampfer_frames_per_tile,
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
        // The train's body is whole tiles (both roofs on the tile grid,
        // D-067); its travel (longest `to`: 64 tiles) plus the top wait fit
        // the counter like the Festungsbahn's, and `frames_per_tile` <= 64
        // keeps the delta × progress product inside i32 (as for the Gondel).
        let [sw, sh] = pl.schafbergbahn_size;
        if sw % TILE_PX != 0 || sh % TILE_PX != 0 {
            return Err(ParseError::new(
                "tuning.toml: platforms.schafbergbahn_size must be whole tiles (multiples of 16 px)",
            ));
        }
        if pl.schafbergbahn_frames_per_tile > 64
            || u32::from(pl.schafbergbahn_wait)
                + u32::from(pl.schafbergbahn_frames_per_tile) * crate::level::MAX_TRACK_TILES as u32
                > 32_767
        {
            return Err(ParseError::new(
                "tuning.toml: need platforms.schafbergbahn_frames_per_tile <= 64 and schafbergbahn_wait + 64 × schafbergbahn_frames_per_tile <= 32767 frames",
            ));
        }
        self.platforms
            .validate_boats(self.camera.autoscroll_speed)?;
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
        self.validate_hackl()?;
        self.validate_autoscroll()?;
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
        if !(0..=TILE_PX).contains(&c.climb_catch_up_speed) {
            return Err(ParseError::new(
                "tuning.toml: camera.climb_catch_up_speed must be 0..=16 px/frame",
            ));
        }
        if !(0..=c.climb_top).contains(&c.climb_top_min) {
            return Err(ParseError::new(
                "tuning.toml: need 0 <= camera.climb_top_min <= climb_top",
            ));
        }
        if c.climb_up_slack > c.climb_fall_follow {
            return Err(ParseError::new(
                "tuning.toml: need camera.climb_up_slack <= climb_fall_follow",
            ));
        }
        if !(0..=c.climb_feet_y - c.climb_top).contains(&c.climb_up_slack) {
            return Err(ParseError::new(
                "tuning.toml: need 0 <= camera.climb_up_slack <= climb_feet_y - climb_top",
            ));
        }
        if !(0..=crate::camera::SCREEN_W).contains(&c.climb_up_reach) {
            return Err(ParseError::new(
                "tuning.toml: camera.climb_up_reach must be 0..=320 px",
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

    /// The auto-scroll camera's keys (D-075): `0 < autoscroll_speed <
    /// player.walk_speed` (walking always gains on the screen; E27 adds
    /// `< swim.speed`), `autoscroll_hold` 0..=600 frames,
    /// `autoscroll_right_margin` 0..=160 px.
    fn validate_autoscroll(&self) -> Result<(), ParseError> {
        let c = &self.camera;
        if c.autoscroll_speed <= Fx::ZERO || c.autoscroll_speed >= self.player.walk_speed {
            return Err(ParseError::new(
                "tuning.toml: need 0 < camera.autoscroll_speed < player.walk_speed",
            ));
        }
        if c.autoscroll_hold > 600 {
            return Err(ParseError::new(
                "tuning.toml: camera.autoscroll_hold must be 0..=600 frames",
            ));
        }
        if !(0..=160).contains(&c.autoscroll_right_margin) {
            return Err(ParseError::new(
                "tuning.toml: camera.autoscroll_right_margin must be 0..=160 px",
            ));
        }
        Ok(())
    }

    /// The `[hackl]` and `[crawl]` sections (D-056, D-057).
    fn validate_hackl(&self) -> Result<(), ParseError> {
        let (h, c) = (&self.hackl, &self.crawl);
        for (name, [w, ht]) in [
            ("hackl.item_hitbox", h.item_hitbox),
            ("hackl.axe_hitbox", h.axe_hitbox),
        ] {
            if !(1..=TILE_PX).contains(&w) || !(1..=TILE_PX).contains(&ht) {
                return Err(ParseError::new(format!(
                    "tuning.toml: {name} must be 1..=16 px in both directions"
                )));
            }
        }
        // The crawl hitbox fits the 1-tile tunnel and is no wider than the
        // player (she stands up in place at the exit).
        let [cw, ch] = c.hitbox;
        if !(1..=self.player.hitbox_w).contains(&cw) || !(1..=TILE_PX).contains(&ch) {
            return Err(ParseError::new(
                "tuning.toml: need 1 <= crawl.hitbox[0] <= player.hitbox_w and 1 <= crawl.hitbox[1] <= 16",
            ));
        }
        if !(1..=8).contains(&h.max_alive) {
            return Err(ParseError::new(
                "tuning.toml: hackl.max_alive must be 1..=8",
            ));
        }
        let max_step = Fx::from_px(12);
        for (name, v) in [
            ("hackl.throw_speed_x", h.throw_speed_x),
            ("hackl.throw_speed_y", h.throw_speed_y),
            ("hackl.gravity", h.gravity),
            ("hackl.max_fall_speed", h.max_fall_speed),
            ("crawl.speed", c.speed),
        ] {
            if v <= Fx::ZERO || v >= max_step {
                return Err(ParseError::new(format!(
                    "tuning.toml: {name} must be > 0 and < 12 px/frame (3072)"
                )));
            }
        }
        for (name, v) in [
            ("hackl.throw_ahead", h.throw_ahead),
            ("hackl.throw_below_top", h.throw_below_top),
            ("crawl.mouth_px", c.mouth_px),
        ] {
            if !(0..=TILE_PX * 2).contains(&v) {
                return Err(ParseError::new(format!(
                    "tuning.toml: {name} must be 0..=32 px"
                )));
            }
        }
        if !(1..=60).contains(&h.throw_pose_frames) || h.stuck_frames == 0 {
            return Err(ParseError::new(
                "tuning.toml: need 1 <= hackl.throw_pose_frames <= 60 and hackl.stuck_frames >= 1",
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
        let e = t.enemies;
        let airtime = gams_airtime(e.gams_hop_speed_y, e.gams_gravity, t.player.max_fall_speed);
        assert_eq!(airtime, Some(32), "a Gams hop lands on its 32nd frame");
        assert_eq!(
            e.gams_hop_speed_x.mul_int(32),
            Fx::from_px(e.gams_hop_tiles * TILE_PX),
            "exactly the swept tiles"
        );
        let c = t.camera;
        assert_eq!(c.autoscroll_speed, Fx(192), "0.75 px/frame (D-075)");
        assert_eq!((c.autoscroll_hold, c.autoscroll_right_margin), (60, 64));
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
    fn schafbergbahn_numbers_are_whole_tiles_and_fit_the_counter() {
        let t = Tuning::parse(REAL).unwrap();
        let pl = &t.platforms;
        assert_eq!(pl.schafbergbahn_size, [64, 32]);
        assert_eq!(pl.schafbergbahn_frames_per_tile, 24);
        assert_eq!(pl.schafbergbahn_wait, 90);
        let size = |s: &str| {
            Tuning::parse(&REAL.replace(
                "schafbergbahn_size = [64, 32]",
                &format!("schafbergbahn_size = {s}"),
            ))
        };
        assert!(size("[48, 32]").is_ok());
        assert!(size("[64, 64]").is_ok());
        for bad in ["[60, 32]", "[64, 24]", "[80, 32]", "[0, 32]"] {
            assert!(size(bad).is_err(), "{bad}");
        }
        let wait = |n: u32| {
            Tuning::parse(&REAL.replace(
                "schafbergbahn_wait = 90",
                &format!("schafbergbahn_wait = {n}"),
            ))
        };
        // 64 tiles × 24 frames = 1536 frames of travel at most.
        assert!(wait(0).is_ok());
        assert!(wait(32_767 - 1536).is_ok());
        assert!(wait(32_767 - 1535).is_err());
    }

    #[test]
    fn boat_numbers_are_validated() {
        let t = Tuning::parse(REAL).unwrap();
        let pl = &t.platforms;
        assert_eq!(pl.traundl_size, [48, 16]);
        assert_eq!((pl.traundl_wait, pl.traundl_frames_per_tile), (30, 16));
        assert_eq!(pl.dampfer_size, [160, 16]);
        assert_eq!(pl.dampfer_frames_per_tile, 20);
        assert_eq!(pl.dampfer_wheel_zone, [32, 20]);
        assert_eq!(pl.dampfer_wheel_rise, 4);
        let with = |old: &str, new: &str| {
            assert!(REAL.contains(old), "{old}");
            Tuning::parse(&REAL.replace(old, new))
        };
        // The Dampfer at least as fast as the scroll: 21 frames per tile
        // (195 subpx/frame) is still >= 192, 22 (186) is not.
        let fpt = "dampfer_frames_per_tile = 20";
        assert!(with(fpt, "dampfer_frames_per_tile = 21").is_ok());
        let e = with(fpt, "dampfer_frames_per_tile = 22")
            .unwrap_err()
            .to_string();
        assert!(e.contains("at least as fast as the auto-scroll"), "{e}");
        assert!(with(fpt, "dampfer_frames_per_tile = 0").is_err());
        assert!(with(fpt, "dampfer_frames_per_tile = 1").is_ok());
        let size = "dampfer_size = [160, 16]";
        assert!(with(size, "dampfer_size = [320, 32]").is_ok());
        for bad in [
            "[336, 16]",
            "[150, 16]",
            "[160, 20]",
            "[0, 16]",
            "[160, 80]",
        ] {
            assert!(
                with(size, &format!("dampfer_size = {bad}")).is_err(),
                "{bad}"
            );
        }
        let zone = "dampfer_wheel_zone = [32, 20]";
        assert!(with(zone, "dampfer_wheel_zone = [160, 64]").is_ok());
        for bad in ["[161, 20]", "[0, 20]", "[32, 0]", "[32, 65]"] {
            assert!(
                with(zone, &format!("dampfer_wheel_zone = {bad}")).is_err(),
                "{bad}"
            );
        }
        let rise = "dampfer_wheel_rise = 4";
        assert!(with(rise, "dampfer_wheel_rise = 20").is_ok());
        assert!(with(rise, "dampfer_wheel_rise = 21").is_err());
        assert!(with(rise, "dampfer_wheel_rise = -1").is_err());
        let size = "traundl_size = [48, 16]";
        assert!(with(size, "traundl_size = [64, 16]").is_ok());
        assert!(with(size, "traundl_size = [65, 16]").is_err());
        let tf = "traundl_frames_per_tile = 16";
        assert!(with(tf, "traundl_frames_per_tile = 0").is_err());
        assert!(with(tf, "traundl_frames_per_tile = 65").is_err());
        // 2 × (wait + 64 × 16) <= 65535.
        let wait = "traundl_wait = 30";
        assert!(with(wait, "traundl_wait = 31743").is_ok());
        assert!(with(wait, "traundl_wait = 31744").is_err());
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
            ("schafbergbahn_size", 0),
            ("schafbergbahn_frames_per_tile", 0),
            ("schafbergbahn_frames_per_tile", 65),
            ("schafbergbahn_wait", 31300),
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
            ("pilger_hitbox", 0),
            ("gams_hitbox", 65),
            ("stein_hitbox", 0),
            ("pilger_speed", 0),
            ("gams_hop_speed_x", 0),
            ("gams_hop_speed_y", 3072),
            ("gams_gravity", 0),
            ("stein_gravity", 0),
            ("stein_max_fall", 3072),
            ("gams_stand", 0),
            ("gams_crouch", 0),
            ("gams_hop_tiles", 0),
            ("gams_hop_tiles", 5),
            ("gams_hop_tiles", 1),
            ("gams_hop_speed_x", 257),
            ("gams_gravity", 60),
            ("gams_hop_speed_y", 1100),
            ("steinschlag_telegraph", 0),
            ("steinschlag_telegraph", 41),
            ("steinschlag_refill", 0),
            ("steinschlag_refill", 50),
            ("steinschlag_screen_top", -1),
            ("steinschlag_screen_top", 180),
            ("operettensaenger_hitbox", 0),
            ("operettensaenger_hitbox", 17),
            ("operettensaenger_range", 15),
            ("operettensaenger_range", 321),
            ("operettensaenger_cycle", 88),
            ("operettensaenger_inhale", 0),
            ("operettensaenger_sing", 0),
            ("operettensaenger_sing", 140),
            ("hohes_c_width", 0),
            ("hohes_c_speed", 0),
            ("hohes_c_speed", 17),
            ("hohes_c_height", 0),
            ("hohes_c_height", 161),
            ("souvenirstand_telegraph", 0),
            ("souvenirstand_telegraph", 31),
            ("souvenirstand_max_alive", 0),
            ("souvenirstand_max_alive", 9),
            ("schneekugel_hitbox", 0),
            ("schneekugel_hitbox", 33),
            ("schneekugel_speed", 0),
            ("schneekugel_speed", 3072),
            ("climb_feet_y", 181),
            ("climb_feet_y", 40),
            ("climb_top", -1),
            ("climb_settle_speed", 0),
            ("climb_settle_speed", 17),
            ("climb_up_slack", -1),
            ("climb_up_slack", 109),
            ("climb_up_reach", -1),
            ("climb_top_min", -1),
            ("climb_top_min", 41),
            ("climb_catch_up_speed", -1),
            ("climb_catch_up_speed", 17),
            ("climb_up_reach", 321),
            ("climb_fall_follow", -1),
            ("climb_fall_follow", 181),
            ("climb_fall_follow", 47), // < climb_up_slack
            ("climb_goal_feet_y", 0),
            ("climb_goal_feet_y", 181),
            ("climb_activate_y", -1),
            ("climb_activate_y", 181),
            ("autoscroll_speed", 0),
            ("autoscroll_speed", 384), // = walk_speed
            ("autoscroll_hold", 601),
            ("autoscroll_right_margin", -1),
            ("autoscroll_right_margin", 161),
            ("item_hitbox", 0),
            ("axe_hitbox", 17),
            ("max_alive", 0),
            ("max_alive", 9),
            ("throw_speed_x", 0),
            ("throw_speed_y", 3072),
            ("throw_ahead", 33),
            ("throw_below_top", -1),
            ("throw_pose_frames", 0),
            ("stuck_frames", 0),
            ("mouth_px", 33),
        ]
        .map(|(f, v)| (f, v.to_string()))
        .into_iter()
        .chain([("blasmusiker_notes", "[30, 30, 78]".to_string())])
        {
            let t = REAL.replace(&format!("{field} = "), &format!("{field} = {bad} #"));
            assert!(Tuning::parse(&t).is_err(), "{field} = {bad}");
        }
    }

    #[test]
    fn reisebus_keys_are_validated() {
        let t = Tuning::parse(REAL).unwrap().enemies;
        assert_eq!(t.reisebus_hitbox, [60, 28]);
        assert_eq!(t.reisebus_trigger, 160, "< 161 px");
        assert_eq!(t.reisebus_speed, Fx(384), "1.5 px/frame");
        assert_eq!(
            (
                t.reisebus_telegraph,
                t.reisebus_stop_frames,
                t.reisebus_restart
            ),
            (60, 120, 30)
        );
        for (field, bad) in [
            ("reisebus_hitbox", "[65, 28]"),
            ("reisebus_hitbox", "[60, 0]"),
            ("reisebus_trigger", "-1"),
            ("reisebus_trigger", "641"),
            ("reisebus_telegraph", "0"),
            ("reisebus_speed", "0"),
            ("reisebus_speed", "3072"),
            ("reisebus_stop_frames", "0"),
            ("reisebus_restart", "0"),
            ("reisebus_flash_frames", "0"),
        ] {
            let text = REAL.replace(&format!("{field} = "), &format!("{field} = {bad} #"));
            let err = Tuning::parse(&text).unwrap_err().to_string();
            assert!(err.contains(field), "{field} = {bad}: {err}");
        }
        let missing = REAL.replace("reisebus_restart = ", "#reisebus_restart = ");
        let err = Tuning::parse(&missing).unwrap_err().to_string();
        assert!(err.contains("reisebus_restart"), "{err}");
    }

    #[test]
    fn lake_enemy_keys_are_validated() {
        let t = Tuning::parse(REAL).unwrap().enemies;
        // D-078: 16×12, 4 px under the water line, 96 px, 32 / 1.5 × 64 /
        // 0.5 / 60.
        assert_eq!((t.schwan_hitbox, t.schwan_float_px), ([16, 12], 4));
        assert_eq!(t.schwan_range, 96, "< 97 px, 6 tiles");
        assert_eq!(
            (
                t.schwan_telegraph,
                t.schwan_charge_speed,
                t.schwan_charge_frames,
                t.schwan_return_speed,
                t.schwan_rest
            ),
            (32, Fx(384), 64, Fx(128), 60)
        );
        // D-079: 8×8, wait 60, hunt 480, 1/32 px/frame², 0.75 px/frame,
        // her top + 4, leaves at 1 px/frame.
        assert_eq!(t.gelse_hitbox, [8, 8]);
        assert_eq!((t.gelse_wait, t.gelse_hunt), (60, 480));
        assert_eq!((t.gelse_accel, t.gelse_speed), (Fx(8), Fx(192)));
        assert_eq!((t.gelse_target_dy, t.gelse_leave_speed), (4, Fx(256)));
        // D-080: 24×56 with the keel 12 px under the water line, < 161 px,
        // 60 frames, 1.5 px/frame.
        assert_eq!(
            (t.regattaboot_hitbox, t.regattaboot_keel_px),
            ([24, 56], 12)
        );
        assert_eq!(t.regattaboot_mast, [8, 20], "the masthead part");
        assert_eq!(
            (
                t.regattaboot_trigger,
                t.regattaboot_telegraph,
                t.regattaboot_speed
            ),
            (160, 60, Fx(384))
        );
        for (field, bad, named) in [
            ("schwan_hitbox", "[25, 12]", "schwan_hitbox"),
            ("schwan_hitbox", "[16, 0]", "schwan_hitbox"),
            ("schwan_float_px", "-1", "schwan_float_px"),
            ("schwan_float_px", "13", "schwan_float_px"),
            ("schwan_range", "15", "schwan_range"),
            ("schwan_range", "321", "schwan_range"),
            ("schwan_telegraph", "0", "schwan_telegraph"),
            ("schwan_charge_speed", "0", "schwan_charge_speed"),
            ("schwan_charge_speed", "3072", "schwan_charge_speed"),
            ("schwan_charge_frames", "0", "schwan_charge_frames"),
            ("schwan_return_speed", "0", "schwan_return_speed"),
            ("schwan_rest", "0", "schwan_rest"),
            ("gelse_hitbox", "[17, 8]", "gelse_hitbox"),
            ("gelse_wait", "0", "gelse_wait"),
            ("gelse_hunt", "0", "gelse_hunt"),
            ("gelse_accel", "0", "gelse_accel"),
            ("gelse_accel", "193", "gelse_accel"),
            ("gelse_speed", "0", "gelse_speed"),
            ("gelse_speed", "3072", "gelse_speed"),
            ("gelse_target_dy", "-1", "gelse_target_dy"),
            ("gelse_target_dy", "33", "gelse_target_dy"),
            ("gelse_leave_speed", "0", "gelse_leave_speed"),
            ("regattaboot_hitbox", "[33, 56]", "regattaboot_hitbox"),
            ("regattaboot_hitbox", "[24, 65]", "regattaboot_hitbox"),
            ("regattaboot_keel_px", "-1", "regattaboot_keel_px"),
            ("regattaboot_mast", "[0, 20]", "regattaboot_mast"),
            ("regattaboot_mast", "[25, 20]", "regattaboot_mast"),
            ("regattaboot_mast", "[8, 56]", "regattaboot_mast"),
            ("regattaboot_keel_px", "17", "regattaboot_keel_px"),
            ("regattaboot_trigger", "-1", "regattaboot_trigger"),
            ("regattaboot_trigger", "641", "regattaboot_trigger"),
            ("regattaboot_telegraph", "0", "regattaboot_telegraph"),
            ("regattaboot_speed", "0", "regattaboot_speed"),
            ("regattaboot_speed", "3072", "regattaboot_speed"),
        ] {
            let text = REAL.replace(&format!("{field} = "), &format!("{field} = {bad} #"));
            let err = Tuning::parse(&text).unwrap_err().to_string();
            assert!(err.contains(named), "{field} = {bad}: {err}");
        }
    }
}
