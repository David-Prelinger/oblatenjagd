//! The game state machine and the per-tick `step`.
//!
//! Besides the player and camera, `Game` owns everything that changes
//! while playing: the current tile grid (bricks break, boxes empty), the
//! entities, score, Oblaten, lives, timer and the checkpoint. A death
//! resets the level part (tiles, entities, timer) to the start state;
//! score, Oblaten, lives and the checkpoint survive (design: "Checkpoint
//! respawn"). Game over → `Mode::GameOver`; START restarts the stage.
//!
//! The screens around play (title, stage intro, pause), the Maibaum goal
//! sequence and the route map between stages live in the child module
//! [`flow`]; which of them a game has is set by its [`Start`] (D-025).
//!
//! A game plays one level at a time. A game made with [`Game::new_run`]
//! also has a [`Route`]: after a stage clear in the real flow it shows the
//! route map and then plays the next stage's level (D-042, D-047).

mod ending;
mod flow;
mod power;

pub use ending::{BILL_LINES, Ending, EndingPhase};
pub use flow::{ClearPhase, StageClear, Start, maibaum_points};

use std::sync::Arc;

use crate::camera::{Autoscroll, Camera, ClimbFocus, Rest, SCREEN_H, SCREEN_W, ScrollPhase};
use crate::enemies::Contact;
use crate::entity::{EnemyKind, Entity, EntityKind, Popup, UpdateCtx, popup};
use crate::fx::Fx;
use crate::hash::{Fnv, WordHash};
use crate::input::InputFrame;
use crate::level::{Block, CameraMode, Contents, Level, SpawnKind, TILE_PX, Tile, TileKind};
use crate::physics::{Aabb, TILE, TileMap, hash_tile, tile_of};
use crate::player::{Bounds, Player, Power};
use crate::rng::Rng;
use crate::route::{Route, RouteMap};
use crate::swim;
use crate::tuning::Tuning;

/// How many columns left and right [`Game::teleport_player`] searches for a floor.
pub const TELEPORT_SEARCH_COLS: i32 = 8;

/// The „Zwischenstopp" popup spawns with its text top this many px above
/// the checkpoint's ground. Its lowest drawn row (glyph rows 0–7 incl. the
/// descender row, then the 1 px outline at row 8) is then 44 px above the
/// ground: 3 px of air above the 40 px bus board beside the checkpoint
/// (the art review asks for at least 2), and it only rises from there (E12
/// art review: it used to spawn at 40 and cover the board).
pub const CHECKPOINT_POPUP_ABOVE_GROUND_PX: i32 = 52;

/// Highest score (the HUD shows 6 digits).
pub const MAX_SCORE: u32 = 999_999;

/// Something that happened during a step (for audio, effects, tests).
///
/// Adding a variant: also add it to [`Event::ALL`] and [`Event::name`]
/// (replay files name events, `no_event=Died`); the `event_names_round_trip`
/// test checks that every variant in `ALL` round-trips.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    /// The player started a jump.
    Jumped,
    /// The player touched down after being airborne.
    Landed,
    /// The player died (hazard, pit, hit as Normal, time up).
    Died,
    /// The player reappeared after dying (or after a game-over restart).
    Respawned,
    /// The player touched the goal.
    LevelComplete,
    /// An Oblate was collected (static or from a box).
    OblateCollected,
    /// The player ate a Mozartkugel and grows (Normal → Zuckerschock).
    PowerUp,
    /// The player was hit as Zuckerschock and shrinks.
    Hurt,
    /// +1 life (100 Oblaten, Zaunerstollen, long stomp chain).
    ExtraLife,
    /// A brick or box was bumped from below.
    BlockBumped,
    /// A brick was broken (Zuckerschock).
    BrickBroken,
    /// The player stomped (or bounced off) an enemy.
    Stomped,
    /// The player passed a checkpoint.
    CheckpointReached,
    /// The last life was lost.
    GameOver,
    /// The player threw a Hackl (D-056).
    HacklThrown,
    /// A thrown Hackl hit a target (defeated, bounced off or shattered it).
    AxeHit,
    /// The player started crawling through a Durchkriechstein (D-057).
    CrawlStarted,
    /// The player swam a stroke (a JUMP press under water, D-076); a
    /// jump-out from the surface is a `Jumped` instead.
    Stroke,
}

impl Event {
    /// Every event, in declaration order. Keep in sync with the enum: a new
    /// variant fails the build in [`Event::name`] (exhaustive `match`), which
    /// is the reminder to list it here too.
    pub const ALL: [Event; 18] = [
        Event::Jumped,
        Event::Landed,
        Event::Died,
        Event::Respawned,
        Event::LevelComplete,
        Event::OblateCollected,
        Event::PowerUp,
        Event::Hurt,
        Event::ExtraLife,
        Event::BlockBumped,
        Event::BrickBroken,
        Event::Stomped,
        Event::CheckpointReached,
        Event::GameOver,
        Event::HacklThrown,
        Event::AxeHit,
        Event::CrawlStarted,
        Event::Stroke,
    ];

    /// The variant name (`"Died"`), as used in replay files (`no_event=Died`).
    pub const fn name(self) -> &'static str {
        match self {
            Event::Jumped => "Jumped",
            Event::Landed => "Landed",
            Event::Died => "Died",
            Event::Respawned => "Respawned",
            Event::LevelComplete => "LevelComplete",
            Event::OblateCollected => "OblateCollected",
            Event::PowerUp => "PowerUp",
            Event::Hurt => "Hurt",
            Event::ExtraLife => "ExtraLife",
            Event::BlockBumped => "BlockBumped",
            Event::BrickBroken => "BrickBroken",
            Event::Stomped => "Stomped",
            Event::CheckpointReached => "CheckpointReached",
            Event::GameOver => "GameOver",
            Event::HacklThrown => "HacklThrown",
            Event::AxeHit => "AxeHit",
            Event::CrawlStarted => "CrawlStarted",
            Event::Stroke => "Stroke",
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
    /// The title screen (only with [`Start::Title`]): START starts the stage.
    Title {
        /// Frames since the title screen appeared (saturating; for blinking).
        frames: u16,
    },
    /// The stage intro screen before play, also after every death and after
    /// a continue (only with [`Start::Title`]). Nothing moves.
    StageIntro {
        /// Frames left until play starts.
        timer: u16,
    },
    /// Normal play.
    Playing,
    /// START during play (only with [`Start::Title`]); START again resumes.
    /// Nothing moves, the timer stands still.
    Paused,
    /// The player died; respawn (or game over) when `timer` runs out.
    Dying {
        /// Frames left until respawn.
        timer: u16,
    },
    /// The goal was reached: the Maibaum sequence runs (slide, hop, walk
    /// off, time bonus, texts), then the route map (real flow with a
    /// route) or „Fortsetzung folgt …" and the title screen. Enemies and
    /// the timer stand still; [`Game::stage_clear`] has the details.
    StageClear,
    /// The route map between two stages (only with [`Start::Title`] and a
    /// route): Resi walks to the next station, then its stage intro (or
    /// the teaser and the title). [`Game::route_map`] has the details.
    RouteMap,
    /// The ending after the journey's last station (only with
    /// [`Start::Title`] and a route, D-107): map, scene, bill, credits,
    /// „Ende", then the title with the „durchgespielt" mark.
    /// [`Game::ending`] has the details.
    Ending,
    /// No lives left. START (after `game.game_over_frames`) restarts the
    /// stage from its start with full lives, score 0 and Oblaten 0.
    GameOver {
        /// Frames since the game over (saturating).
        frames: u16,
    },
}

/// Why the player died (the renderer shows „Zeit is' um!" for `TimeUp`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeathCause {
    /// Touched a hazard tile (spikes, Salzach water).
    Hazard,
    /// Fell out of the bottom of the level.
    Pit,
    /// Hit by an enemy as Normal.
    Hit,
    /// The timer reached 0.
    TimeUp,
    /// Pushed by the auto-scroll camera's left edge against a solid tile
    /// (D-075); drawn as the normal hit death.
    Squeezed,
}

/// Progress of an `oblaten_10` brick that has been hit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct MultiBlock {
    /// Row-major cell index.
    cell: u32,
    /// Oblaten paid so far.
    hits: u8,
    /// Frame after which the next hit is the last one.
    deadline: u32,
}

/// The complete simulation state. Cheap to clone: the static level data is
/// shared via `Arc`, and so is the tile grid until a tile changes (copy on
/// write). `Game` is `Send + Sync` (the solver searches on several threads).
#[derive(Clone, Debug)]
pub struct Game {
    frame: u32,
    mode: Mode,
    level: Arc<Level>,
    /// The current grid (copy on write).
    map: Arc<TileMap>,
    /// The level's start grid (for level resets).
    start_map: Arc<TileMap>,
    /// Sorted cell indices where `map` differs from `start_map`.
    changed_cells: Vec<u32>,
    goals: Arc<[Aabb]>,
    pub(crate) player: Player,
    pub(crate) camera: Camera,
    /// Climb camera (D-054): the feet height (whole px) of the last
    /// grounded frame; unused (0) in side levels.
    ground_feet: i32,
    /// Climb camera (D-065): the feet height (whole px) the ground framing
    /// is anchored on ([`ClimbFocus::settle_feet`]); unused (0) in side
    /// levels.
    settle_feet: i32,
    /// Climb camera (D-065): [`ClimbFocus::slack_x`]; 0 in side levels.
    slack_x: i32,
    /// Auto-scroll camera (D-075): phase, subpixel position and hold; the
    /// default (unused) in other levels.
    pub(crate) scroll: Autoscroll,
    rng: Rng,
    deaths: u32,
    tuning: Tuning,
    pub(crate) entities: Vec<Entity>,
    multi: Vec<MultiBlock>,
    pub(crate) score: u32,
    pub(crate) oblaten: u8,
    /// Every Oblate collected since the run started or the last continue
    /// (D-107); unlike `oblaten` it never wraps. Shown on the ending's bill.
    pub(crate) oblaten_total: u32,
    pub(crate) lives: u8,
    /// Seconds left.
    pub(crate) time: u16,
    /// Frames into the current second.
    time_frames: u8,
    /// The stage clock (D-094): frames of play since the stage started or
    /// since the last respawn, i.e. the frames the ZEIT timer counts
    /// (frozen, paused, dying and goal-sequence frames don't count).
    pub(crate) stage_clock: u32,
    /// Index into `Level::entities` of the furthest checkpoint passed.
    checkpoint: Option<u16>,
    death_cause: Option<DeathCause>,
    /// START was held in the previous step (to detect presses).
    start_held: bool,
    /// Which flow screens this game has.
    start: Start,
    /// The goal sequence while `Mode::StageClear`.
    clear: Option<StageClear>,
    /// The levels of the run (`None`: a lone level, no route map).
    route: Option<Arc<Route>>,
    /// The current stage (index into the route = station index; 0 without
    /// a route).
    stage: u8,
    /// The stage the run starts at (title → START, after the teaser).
    first_stage: u8,
    /// The route map while `Mode::RouteMap`.
    route_map: Option<RouteMap>,
    /// The ending while `Mode::Ending` (D-107).
    ending: Option<Ending>,
    /// The session's „durchgespielt" mark (D-107): set when the ending
    /// ends, never cleared (it survives `end_run` and every new run).
    cleared: bool,
    /// Hash of everything that never changes during a game (start grid,
    /// goals, spawns, tuning), computed per level (`level_parts`).
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
    ///
    /// Same as [`Game::new_at`] with [`Start::Playing`]: play starts at once,
    /// without stage intros or pause (tests, tools and replays by default).
    pub fn new(level: &Level, tuning: &Tuning, seed: u64) -> Game {
        Game::new_at(level, tuning, seed, Start::Playing)
    }

    /// Starts a level with the given flow: [`Start::Title`] is the real
    /// game (title screen, stage intros, pause), [`Start::Playing`] starts
    /// in play without those screens.
    pub fn new_at(level: &Level, tuning: &Tuning, seed: u64, start: Start) -> Game {
        Game::build(Arc::new(level.clone()), None, 0, tuning, seed, start)
    }

    /// Starts a run along `route` at stage `stage` (clamped to the route):
    /// like [`Game::new_at`] with that stage's level, and with
    /// [`Start::Title`] a stage clear leads over the route map to the next
    /// stage (D-042). The title screen shows (and START starts) `stage`.
    pub fn new_run(
        route: Arc<Route>,
        stage: usize,
        tuning: &Tuning,
        seed: u64,
        start: Start,
    ) -> Game {
        let stage = stage.min(route.len().saturating_sub(1));
        let level = Arc::clone(route.level(stage).expect("a route has at least one level"));
        Game::build(level, Some(route), stage as u8, tuning, seed, start)
    }

    fn build(
        level: Arc<Level>,
        route: Option<Arc<Route>>,
        stage: u8,
        tuning: &Tuning,
        seed: u64,
        start: Start,
    ) -> Game {
        let (map, goals, static_hash) = level_parts(&level, tuning);
        let at = level.player_start();
        let player = Player::spawn(at.col, at.row, &tuning.player);
        let mut game = Game {
            frame: 0,
            mode: Mode::Playing,
            level,
            map: Arc::clone(&map),
            start_map: map,
            changed_cells: Vec::new(),
            goals,
            player,
            camera: Camera { x: 0, y: 0 },
            ground_feet: 0,
            settle_feet: 0,
            slack_x: 0,
            scroll: Autoscroll::default(),
            rng: Rng::new(seed),
            deaths: 0,
            tuning: *tuning,
            entities: Vec::new(),
            multi: Vec::new(),
            score: 0,
            oblaten: 0,
            oblaten_total: 0,
            lives: tuning.game.lives_start,
            time: 0,
            time_frames: 0,
            stage_clock: 0,
            checkpoint: None,
            death_cause: None,
            start_held: false,
            start,
            clear: None,
            route,
            stage,
            first_stage: stage,
            route_map: None,
            ending: None,
            cleared: false,
            static_hash,
            events: Vec::new(),
        };
        game.reset_level();
        game.place_camera();
        if start == Start::Title {
            game.mode = Mode::Title { frames: 0 };
            game.frame_title();
        }
        game
    }

    /// Advances exactly one 1/60 s tick and returns what happened.
    pub fn step(&mut self, input: InputFrame) -> &[Event] {
        self.events.clear();
        self.frame = self.frame.wrapping_add(1);
        let start_pressed = input.contains(InputFrame::START) && !self.start_held;
        match self.mode {
            Mode::Title { frames } => {
                self.player.jump_held = input.jump();
                self.mode = Mode::Title {
                    frames: frames.saturating_add(1),
                };
                if start_pressed {
                    self.restart();
                }
            }
            Mode::StageIntro { timer } => {
                self.player.jump_held = input.jump();
                self.mode = if timer > 1 {
                    Mode::StageIntro { timer: timer - 1 }
                } else {
                    Mode::Playing
                };
            }
            Mode::Playing if start_pressed && self.start == Start::Title => {
                self.player.jump_held = input.jump();
                self.mode = Mode::Paused;
            }
            Mode::Playing => self.step_playing(input),
            Mode::Paused => {
                self.player.jump_held = input.jump();
                if start_pressed {
                    self.mode = Mode::Playing;
                }
            }
            Mode::Dying { timer } => {
                // Keep tracking JUMP so a button held across the respawn
                // is not a new press (and presses here never buffer).
                self.player.jump_held = input.jump();
                if timer > 1 {
                    self.mode = Mode::Dying { timer: timer - 1 };
                } else if self.lives == 0 {
                    self.mode = Mode::GameOver { frames: 0 };
                    self.events.push(Event::GameOver);
                } else {
                    self.reset_level();
                    self.respawn();
                }
            }
            Mode::StageClear => {
                self.player.jump_held = input.jump();
                self.step_stage_clear();
            }
            Mode::RouteMap => {
                self.player.jump_held = input.jump();
                self.step_route_map(start_pressed);
            }
            Mode::Ending => {
                self.player.jump_held = input.jump();
                self.step_ending(start_pressed);
            }
            Mode::GameOver { frames } => {
                self.player.jump_held = input.jump();
                let frames = frames.saturating_add(1);
                self.mode = Mode::GameOver { frames };
                if start_pressed && frames >= self.tuning.game.game_over_frames {
                    self.restart();
                }
            }
        }
        self.start_held = input.contains(InputFrame::START);
        // In every mode, so RUN held across screens is not a new press.
        self.player.run_held = input.run();
        &self.events
    }

    fn step_playing(&mut self, input: InputFrame) {
        // Growing or shrinking freezes everything (timer included).
        if self.player.transform > 0 {
            self.player.transform -= 1;
            self.player.jump_held = input.jump();
            return;
        }

        self.player.throw_pose = self.player.throw_pose.saturating_sub(1);
        let prev_bottom = self.player.y + self.player.h;
        // The Durchkriechstein crawl replaces the player's own movement and
        // every contact with entities (input ignored, can't be hurt, D-057).
        let crawling = self.player.crawling() || self.start_crawl(input);
        if crawling {
            self.player.jump_held = input.jump();
            self.step_crawl();
        } else {
            let was_on_ground = self.player.on_ground;
            let bounds = self.player_bounds();
            let head_row = self.player.update(
                input,
                &self.map,
                &self.tuning.player,
                &self.tuning.swim,
                bounds,
                &mut self.events,
            );
            crate::platform::land(
                &mut self.player,
                &self.entities,
                prev_bottom,
                was_on_ground,
                &self.tuning.player,
                &mut self.events,
            );
            if let Some(row) = head_row {
                self.hit_from_below(row);
            }
            self.throw_hackl(input);
        }
        self.update_entities();
        self.follow_carried_left();
        self.axe_hits();
        if !crawling {
            self.interact(prev_bottom, input);
        }
        // Counted after the hits of this frame: exactly
        // `invulnerable_frames` frames without harm.
        if self.mode == Mode::Playing && self.player.transform == 0 {
            self.player.invulnerable = self.player.invulnerable.saturating_sub(1);
            self.player.bounce_grace = self.player.bounce_grace.saturating_sub(1);
        }
        self.entities.retain(|e| !e.removed);
        if self.mode != Mode::Playing {
            return;
        }

        let hb = self.player.hitbox();
        // Spikes kill on touch; water only once the feet have sunk in
        // (D-050), so coyote jumps work at water edges.
        if self.map.touches(&hb, TileKind::Hazard)
            || self
                .map
                .sunk_in_water(&hb, self.tuning.player.water_sink_px)
        {
            self.die(DeathCause::Hazard);
            return;
        }
        if hb.y >= self.map.height_fx() {
            self.die(DeathCause::Pit);
            return;
        }
        self.check_checkpoints();
        if let Some(goal) = self.touched_goal(&hb) {
            self.touch_goal(goal);
        }
        // Also on the frame the scroll hands over to the lead-out.
        let was_scrolling = self.scrolling();
        self.follow_camera();
        // The auto-scroll camera's left edge pushes (D-075); against a
        // solid tile she is squeezed.
        if self.mode == Mode::Playing
            && (was_scrolling || self.scrolling())
            && self.push_from_left_edge()
        {
            self.die(DeathCause::Squeezed);
            return;
        }
        // The climb camera's death line (D-054): the hitbox top below the
        // screen is a fall out of the level.
        if self.mode == Mode::Playing
            && self.climb()
            && hb.y > Fx::from_px(self.camera.y + SCREEN_H)
        {
            self.die(DeathCause::Pit);
            return;
        }
        if self.mode == Mode::Playing {
            self.tick_timer();
        }
    }

    /// True in a `camera = "climb"` level (D-054).
    fn climb(&self) -> bool {
        self.level.meta.camera == CameraMode::Climb
    }

    /// The auto-scroll zone `[from, to)` in level pixels (D-075), if the
    /// level has one.
    fn autoscroll_zone(&self) -> Option<(i32, i32)> {
        let (from, to) = self.level.meta.autoscroll_zone()?;
        Some((from * TILE_PX, to * TILE_PX))
    }

    /// A side level's top clamp (`[meta] camera_top`, D-103): the camera's
    /// y never above it. Without the key (and in climb and auto-scroll
    /// levels, which reject it) nothing changes.
    pub(crate) fn clamp_camera_top(&mut self) {
        if let Some(top) = self.level.meta.camera_top {
            let level_h = self.map.height_fx().to_px();
            self.camera.clamp_top(top, level_h);
        }
    }

    /// True while the auto-scroll camera scrolls (also during its hold):
    /// the left edge pushes, the right clamp holds and D-051 is off.
    fn scrolling(&self) -> bool {
        self.scroll.phase == ScrollPhase::Scroll && self.autoscroll_zone().is_some()
    }

    /// The scroll's right clamp (D-075): the player's hitbox right edge
    /// stays at most `autoscroll_right_margin` px left of the screen's
    /// right edge, for her own movement and any carry. No limit outside
    /// the scroll.
    fn right_clamp(&self) -> Fx {
        if !self.scrolling() {
            return Fx::MAX;
        }
        let margin = self.tuning.camera.autoscroll_right_margin;
        Fx::from_px(self.camera.x + SCREEN_W - margin)
    }

    /// The limits of the player's own movement this frame: the screen's
    /// left edge, and in the scroll the right clamp, which moves at the
    /// scroll speed (not during the hold).
    fn player_bounds(&self) -> Bounds {
        let mut bounds = Bounds::left(Fx::from_px(self.camera.x));
        if self.scrolling() {
            bounds.max_x = self.right_clamp();
            if self.scroll.hold == 0 {
                bounds.max_x_speed = self.tuning.camera.autoscroll_speed;
            }
        }
        bounds
    }

    /// The scroll's left edge (D-075, SMB3): a hitbox left of the camera's
    /// x is moved right by the difference with tile collision. Returns true
    /// if a solid tile stopped that move (she is squeezed).
    fn push_from_left_edge(&mut self) -> bool {
        let edge = Fx::from_px(self.camera.x);
        if self.player.x >= edge {
            return false;
        }
        let mut b = self.player.hitbox();
        self.map.move_x(&mut b, edge - self.player.x);
        self.player.x = b.x;
        b.x < edge
    }

    /// One frame of the level's camera (side: dead zone; climb: D-054;
    /// autoscroll: D-075).
    fn follow_camera(&mut self) {
        let (w, h) = (self.map.width_fx().to_px(), self.map.height_fx().to_px());
        let t = &self.tuning.camera;
        if let Some(zone) = self.autoscroll_zone() {
            let focus = self.player.center_px();
            let rest = self.camera_rest();
            self.camera
                .follow_autoscroll(&mut self.scroll, zone, focus, rest, w, h, t);
            return;
        }
        if !self.climb() {
            self.camera.follow(self.player.center_px(), w, h, t);
            self.clamp_camera_top();
            return;
        }
        let p = &self.player;
        let mut focus = ClimbFocus {
            cx: p.center_px().0,
            top: p.y.to_px(),
            feet: (p.y + p.h).to_px(),
            grounded: p.on_ground,
            ground_feet: self.ground_feet,
            settle_feet: self.settle_feet,
            slack_x: self.slack_x,
        };
        self.camera.follow_climb(&mut focus, w, h, t);
        self.ground_feet = focus.ground_feet;
        self.settle_feet = focus.settle_feet;
        self.slack_x = focus.slack_x;
    }

    /// The auto-scroll camera settles towards a rest (D-086, D-090) while
    /// she stands (a tile, a deck, a boat or the lake bed: framed by the
    /// top of that ground, her feet) or swims at the surface; not in the
    /// air and not diving.
    pub(crate) fn camera_rest(&self) -> Rest {
        let p = &self.player;
        if p.on_ground {
            return Rest::Ground((p.y + p.h).to_px());
        }
        let st = &self.tuning.swim;
        let floating = p.swimming
            && !p.leaving_water
            && swim::swim_surface(p, &self.map, st)
                .is_some_and(|surface| swim::at_surface(swim::chest(p, st).1, surface, st));
        if floating { Rest::Surface } else { Rest::None }
    }

    /// True if the auto-scroll camera settles this frame ([`Game::camera_rest`]).
    #[cfg(test)]
    pub(crate) fn camera_rests(&self) -> bool {
        self.camera_rest() != Rest::None
    }

    /// D-051: the player's own movement stops at the screen's left edge,
    /// so only a moving platform's carry can take her past it; the camera
    /// then scrolls left by the same amount (the one exception to "never
    /// scrolls left"), and she stays on the cabin. Off in the auto-scroll
    /// (D-075): there the left edge pushes her across the deck instead.
    fn follow_carried_left(&mut self) {
        if self.scrolling() {
            return;
        }
        let left = self.player.x.to_px();
        if left < self.camera.x {
            self.camera.x = left.max(0);
        }
    }

    /// One frame of the countdown and of the stage clock (every
    /// Fensterladen follows it at once, D-094); at 0 the player dies.
    fn tick_timer(&mut self) {
        self.stage_clock = self.stage_clock.saturating_add(1);
        self.place_shutters();
        self.time_frames += 1;
        if u32::from(self.time_frames) < crate::TICKS_PER_SECOND {
            return;
        }
        self.time_frames = 0;
        self.time = self.time.saturating_sub(1);
        if self.time == 0 {
            self.die(DeathCause::TimeUp);
        }
    }

    fn die(&mut self, cause: DeathCause) {
        self.mode = Mode::Dying {
            timer: self.tuning.game.death_frames.max(1),
        };
        self.player.vx = Fx::ZERO;
        self.player.vy = Fx::ZERO;
        self.player.transform = 0;
        self.player.invulnerable = 0;
        self.deaths = self.deaths.saturating_add(1);
        self.lives = self.lives.saturating_sub(1);
        self.death_cause = Some(cause);
        self.events.push(Event::Died);
    }

    /// Every Fensterladen (active or not: it is drawn before activation)
    /// in its state at the current stage clock (D-094).
    fn place_shutters(&mut self) {
        let clock = self.stage_clock;
        for e in &mut self.entities {
            if e.kind != EntityKind::Fensterladen {
                continue;
            }
            if let Some(spawn) = e
                .spawn
                .and_then(|i| self.level.entities.get(usize::from(i)))
            {
                crate::platform::place_shutter(e, spawn, clock);
            }
        }
    }

    /// Back to the level's start state: tiles, entities, timer, and the
    /// stage clock at 0 (the entities spawn in their state at clock 0).
    fn reset_level(&mut self) {
        self.map = Arc::clone(&self.start_map);
        self.changed_cells.clear();
        self.multi.clear();
        self.entities = self
            .level
            .entities
            .iter()
            .enumerate()
            .filter_map(|(i, s)| Entity::from_spawn(s, i, &self.tuning))
            .collect();
        self.time = self.level.meta.time_limit.min(u32::from(u16::MAX)) as u16;
        self.time_frames = 0;
        self.stage_clock = 0;
    }

    /// Puts a Normal player at the checkpoint (or the start) and the camera
    /// on it; play goes on (after the stage intro with [`Start::Title`]).
    fn respawn(&mut self) {
        self.place_player();
        self.death_cause = None;
        self.events.push(Event::Respawned);
        self.enter_stage();
    }

    /// Play starts (after the stage intro with [`Start::Title`]).
    fn enter_stage(&mut self) {
        self.mode = match self.start {
            Start::Title => Mode::StageIntro {
                timer: self.tuning.game.intro_frames.max(1),
            },
            Start::Playing => Mode::Playing,
        };
    }

    /// The title screen's view of the stage: an auto-scroll stage keeps the
    /// side camera's framing there (D-086), so the title's subtitle stays in
    /// the sky (D-035); START re-places the camera at its rest behind the
    /// navy intro card, so play still starts at the rest. Other levels:
    /// unchanged (`place_camera` already framed them so).
    pub(super) fn frame_title(&mut self) {
        if self.autoscroll_zone().is_some() {
            let (w, h) = (self.map.width_fx().to_px(), self.map.height_fx().to_px());
            self.camera = Camera::new(self.player.center_px(), w, h, &self.tuning.camera);
        }
    }

    /// A Normal player at the checkpoint (or the start), the camera on it.
    /// Keeps `jump_held`, so a held JUMP is not a new press.
    fn place_player(&mut self) {
        let jump_held = self.player.jump_held;
        let at = match self.checkpoint {
            Some(i) => &self.level.entities[i as usize],
            None => self.level.player_start(),
        };
        self.player = Player::spawn(at.col, at.row, &self.tuning.player);
        self.player.jump_held = jump_held;
        self.place_camera();
    }

    /// The stage from its start with full lives, no score, no Oblaten, no
    /// checkpoint (no mode change, no event).
    fn reset_run(&mut self) {
        self.lives = self.tuning.game.lives_start;
        self.score = 0;
        self.oblaten = 0;
        self.oblaten_total = 0;
        self.checkpoint = None;
        self.clear = None;
        self.death_cause = None;
        self.reset_level();
        self.place_player();
    }

    /// Game over → START, title → START: the stage from its start, full
    /// lives, no score. No `Respawned` event: that is only for a death.
    fn restart(&mut self) {
        self.reset_run();
        self.enter_stage();
    }

    /// Switches to another level (stage change): grid, goals and the
    /// static hash; the caller resets the level state and the player.
    fn set_level(&mut self, level: Arc<Level>) {
        let (map, goals, static_hash) = level_parts(&level, &self.tuning);
        self.level = level;
        self.map = Arc::clone(&map);
        self.start_map = map;
        self.goals = goals;
        self.static_hash = static_hash;
    }

    /// The camera on the player at the start, on respawn and on a stage
    /// change (climb: at the settle target at once, D-054; auto-scroll: at
    /// its rest, D-086; side: never above `camera_top`, D-103).
    fn place_camera(&mut self) {
        let (w, h) = (self.map.width_fx().to_px(), self.map.height_fx().to_px());
        let t = &self.tuning.camera;
        if self.climb() {
            let feet = (self.player.y + self.player.h).to_px();
            self.camera = Camera::new_climb(self.player.center_px().0, feet, w, h, t);
            self.ground_feet = feet;
            self.settle_feet = feet;
            self.slack_x = self.player.center_px().0;
        } else {
            self.camera = if self.autoscroll_zone().is_some() {
                // D-086: at its rest, the level's bottom clamp.
                Camera::new_autoscroll(self.player.center_px(), w, h, t)
            } else {
                Camera::new(self.player.center_px(), w, h, t)
            };
            self.ground_feet = 0;
            self.settle_feet = 0;
            self.slack_x = 0;
        }
        // D-103: start, respawn, stage change and the side title framing.
        self.clamp_camera_top();
        // Auto-scroll (D-075): framed as in side levels; inside the zone
        // the scroll waits `autoscroll_hold` frames.
        self.scroll = match self.autoscroll_zone() {
            Some(zone) => Autoscroll::placed(self.camera.x, zone, &self.tuning.camera),
            None => Autoscroll::default(),
        };
    }

    // ------------------------------------------------------------ blocks

    /// The head bumped the solid tiles of `row`: hits the one nearest the
    /// player's centre (one block per bump, like SMB).
    fn hit_from_below(&mut self, row: i32) {
        let p = &self.player;
        let centre = tile_of(p.x + p.w.div_int(2));
        let (c0, c1) = (tile_of(p.x), tile_of(p.x + p.w - Fx(1)));
        let other = if centre == c0 { c1 } else { c0 };
        let Some(col) = [centre, other].into_iter().find(|&c| {
            self.map
                .tile(c, row)
                .is_some_and(|t| t.kind == TileKind::Solid)
        }) else {
            return;
        };
        self.hit_block(col, row);
    }

    fn hit_block(&mut self, col: i32, row: i32) {
        let Some(tile) = self.map.tile(col, row) else {
            return;
        };
        if tile.block == Block::None || self.bump_at(col, row).is_some() {
            return;
        }
        match (tile.block, tile.contains) {
            (Block::Brick, Contents::None) => {
                if self.player.power.is_big() {
                    self.break_brick(col, row);
                    self.knock_enemies_on(col, row);
                    return;
                }
            }
            (_, Contents::Oblaten10) => {
                let cell = self.map.index(col, row).unwrap_or(0) as u32;
                let deadline = self
                    .frame
                    .saturating_add(u32::from(self.tuning.items.multi_frames));
                let i = match self.multi.iter().position(|m| m.cell == cell) {
                    Some(i) => i,
                    None => {
                        self.multi.push(MultiBlock {
                            cell,
                            hits: 0,
                            deadline,
                        });
                        self.multi.len() - 1
                    }
                };
                self.multi[i].hits = self.multi[i].hits.saturating_add(1);
                let m = self.multi[i];
                self.release(Contents::Oblate, col, row);
                if m.hits >= self.tuning.items.multi_oblaten || self.frame >= m.deadline {
                    self.multi.remove(i);
                    self.empty_box(col, row);
                }
            }
            (_, contents) => {
                self.release(contents, col, row);
                self.empty_box(col, row);
            }
        }
        let bump = Entity::new(
            EntityKind::Bump,
            TILE.mul_int(col),
            TILE.mul_int(row),
            TILE,
            TILE,
        );
        self.entities.push(bump);
        self.events.push(Event::BlockBumped);
        self.knock_enemies_on(col, row);
    }

    /// A block was hit from below (bumped or broken): enemies standing on
    /// it are knocked off for `score.bump_enemy` each (nothing for a
    /// Kurgast who has scored, D-095), items on it hop.
    fn knock_enemies_on(&mut self, col: i32, row: i32) {
        let knocked = crate::enemies::knock_off_block(&mut self.entities, col, row, &self.tuning);
        for (kind, x, y, scores) in knocked {
            if scores {
                let points = self.tuning.score.bump_enemy;
                self.add_score(points);
                self.entities.push(popup(Popup::Score(points), x, y));
            }
            self.entities
                .extend(crate::enemies::defeat_popup(kind, x, y));
        }
    }

    /// The bump entity on a cell, if one is running.
    fn bump_at(&self, col: i32, row: i32) -> Option<&Entity> {
        self.entities
            .iter()
            .find(|e| e.kind == EntityKind::Bump && !e.removed && e.cell() == (col, row))
    }

    /// Spawns what a block releases above (Oblate) or inside (items) it.
    fn release(&mut self, contents: Contents, col: i32, row: i32) {
        let feet_x = TILE.mul_int(col) + TILE.div_int(2);
        let it = self.tuning.items;
        match contents {
            Contents::None => {}
            Contents::Oblate | Contents::Oblaten10 => {
                let mut o = Entity::at_feet(
                    EntityKind::PoppedOblate,
                    feet_x,
                    TILE.mul_int(row),
                    it.oblate_hitbox,
                );
                o.vy = -it.pop_speed;
                self.entities.push(o);
                let life = self.collect_oblate(feet_x, TILE.mul_int(row - 1));
                self.entities.extend(life);
            }
            Contents::Mozartkugel => self.entities.push(Entity::at_feet(
                EntityKind::Mozartkugel,
                feet_x,
                TILE.mul_int(row + 1),
                it.mozartkugel_hitbox,
            )),
            Contents::Zaunerstollen => self.entities.push(Entity::at_feet(
                EntityKind::Zaunerstollen,
                feet_x,
                TILE.mul_int(row + 1),
                it.zaunerstollen_hitbox,
            )),
            Contents::Hackl => self.entities.push(Entity::at_feet(
                EntityKind::Hackl,
                feet_x,
                TILE.mul_int(row + 1),
                self.tuning.hackl.item_hitbox,
            )),
        }
    }

    pub(crate) fn empty_box(&mut self, col: i32, row: i32) {
        let name = self.level.empty_box_name();
        self.set_tile(
            col,
            row,
            Tile {
                kind: TileKind::Solid,
                name,
                block: Block::None,
                contains: Contents::None,
            },
        );
    }

    fn break_brick(&mut self, col: i32, row: i32) {
        self.set_tile(col, row, Tile::default());
        let it = self.tuning.items;
        let (x, y) = (TILE.mul_int(col), TILE.mul_int(row));
        let half = TILE.div_int(2);
        // Upper pieces first (left, right), then the lower ones.
        for (dx, dy, vx, vy) in [
            (
                Fx::ZERO,
                Fx::ZERO,
                -it.debris_speed_x,
                -it.debris_speed_high,
            ),
            (half, Fx::ZERO, it.debris_speed_x, -it.debris_speed_high),
            (Fx::ZERO, half, -it.debris_speed_x, -it.debris_speed_low),
            (half, half, it.debris_speed_x, -it.debris_speed_low),
        ] {
            let mut d = Entity::new(EntityKind::Debris, x + dx, y + dy, half, half);
            d.vx = vx;
            d.vy = vy;
            self.entities.push(d);
        }
        self.add_score(self.tuning.score.brick);
        let centre = x + half;
        self.entities
            .push(popup(Popup::Score(self.tuning.score.brick), centre, y));
        self.events.push(Event::BrickBroken);
    }

    pub(crate) fn set_tile(&mut self, col: i32, row: i32, tile: Tile) {
        let Some(i) = self.map.index(col, row) else {
            return;
        };
        Arc::make_mut(&mut self.map).set_tile(col, row, tile);
        let i = i as u32;
        if let Err(pos) = self.changed_cells.binary_search(&i) {
            self.changed_cells.insert(pos, i);
        }
    }

    // ---------------------------------------------------------- entities

    /// Activation, per-kind movement and despawning (module docs of
    /// [`crate::entity`]).
    fn update_entities(&mut self) {
        let g = &self.tuning.game;
        let activate_x = Fx::from_px(self.camera.x + SCREEN_W + g.activate_margin);
        // Climb levels: also only near the screen vertically (D-054).
        let climb = self.climb();
        let (camera, margin_y) = (self.camera, self.tuning.camera.climb_activate_y);
        let level = &self.level;
        let despawn_x = Fx::from_px(self.camera.x - g.despawn_margin);
        let bottom = self.map.height_fx();
        let max_x = self.right_clamp();
        // Every Souvenirstand counts its own globes by their spawn point.
        let globes: Vec<(Fx, Fx)> = self
            .entities
            .iter()
            .filter(|e| e.kind == EntityKind::Enemy(EnemyKind::Schneekugel) && !e.removed)
            .map(|e| (e.base_x, e.base_y))
            .collect();
        // Every Ampel counts its own cars by their spawn point (D-093).
        let cars: Vec<(Fx, Fx)> = self
            .entities
            .iter()
            .filter(|e| e.kind == EntityKind::Enemy(EnemyKind::Auto) && !e.removed)
            .map(|e| (e.base_x, e.base_y))
            .collect();
        let ctx = UpdateCtx {
            map: &self.map,
            tuning: &self.tuning,
            level: &self.level,
            player_cx: self.player.x + self.player.w.div_int(2),
            player_top: self.player.y,
            player_box: self.player.hitbox(),
            player_standing: self.player.on_ground.then(|| self.player.hitbox()),
            screen_left: Fx::from_px(self.camera.x),
            screen_top: Fx::from_px(self.camera.y),
            globes: &globes,
            stage_clock: self.stage_clock(),
            cars: &cars,
        };
        let wakes = |e: &Entity| {
            !e.active && e.x < activate_x && (!climb || spawn_row_near(level, e, camera, margin_y))
        };
        // A file of Pilger wakes together (D-061 addendum): every file one
        // of whose pilgrims wakes by the rule above, before anyone moves.
        let files: Vec<(i32, i32)> = self
            .entities
            .iter()
            .filter(|e| wakes(e))
            .filter_map(|e| crate::enemies::pilger_file(level, e))
            .collect();
        let mut spawned = Vec::new();
        for e in &mut self.entities {
            // Snapshot for the stomp rule (previous-frame position).
            e.prev_y = e.y;
            let in_woken_file = || {
                !files.is_empty()
                    && crate::enemies::pilger_file(level, e).is_some_and(|f| files.contains(&f))
            };
            if !e.active && (wakes(e) || in_woken_file()) {
                e.active = true;
                e.timer = 0;
            }
            if !e.active || e.removed {
                continue;
            }
            let old = e.hitbox();
            // `ctx.player_cx` is the player's x before any car carried
            // them this frame (same for every entity: order-independent).
            if let Some(new) = e.update(&ctx) {
                spawned.push(new);
            }
            if e.kind == EntityKind::Stein && e.state == crate::enemies::state::SHATTERED {
                spawned.extend(crate::enemies::stein_debris(e, ctx.tuning));
            }
            if e.kind == EntityKind::Enemy(EnemyKind::Schneekugel)
                && e.state == crate::enemies::state::SHATTERED
            {
                spawned.extend(crate::enemies::globe_shards(e, ctx.tuning));
            }
            if e.kind.is_platform() {
                crate::platform::carry(
                    &mut self.player,
                    old,
                    e,
                    ctx.map,
                    max_x,
                    &ctx.tuning.player,
                    &mut self.events,
                );
            }
            // Walking enemies are lost in swim water (D-076).
            if let EntityKind::Enemy(kind) = e.kind
                && kind.sinks_in_swim_water()
                && ctx.map.touches(&e.hitbox(), TileKind::Swim)
            {
                e.removed = true;
            }
            // Moving platforms never despawn (D-027): a player who fell
            // off must be able to wait for the car.
            if !e.kind.is_platform() && (e.right() < despawn_x || e.y >= bottom) {
                e.removed = true;
            }
        }
        crate::enemies::turn_at_enemies(&mut self.entities, &self.tuning);
        self.entities.extend(spawned);
    }

    /// The player against every active entity, in order.
    ///
    /// Two passes, so the result does not depend on the entity order:
    /// first items and stomps (every enemy whose stomp rule holds is
    /// stomped, the chain counts each one), then hurts from the enemies
    /// that were touched but not stomped. A frame with any stomp bounces
    /// the player once and reports one `Stomped` event (however many
    /// enemies it hit).
    ///
    /// Every contact of the frame uses the player's hitbox from before the
    /// pass: a Mozartkugel grows her (feet kept) at once, but the new head is
    /// fitted under the tiles (`Player::fit_head`) only after the pass, and
    /// later entities in the list still meet the old hitbox.
    ///
    /// A bounce off a non-stompable enemy (the Fiaker) puts the player's
    /// feet on its top, so the next frame (rising) can't be a side hit
    /// (D-027). If a solid tile is in the way there, she stays where she is
    /// and non-stompable enemies do no harm for `enemies.bounce_grace_frames`
    /// instead. An enemy hit in the frame the goal is touched does nothing:
    /// the goal wins (D-027).
    fn interact(&mut self, prev_bottom: Fx, input: InputFrame) {
        let mut bounced = false;
        let mut hurt = false;
        // Top of the highest non-stompable enemy bounced off this frame.
        let mut rest_on: Option<Fx> = None;
        let mut spawned = Vec::new();
        let mut grew = false;
        let hitbox = self.player.hitbox();
        let swimming = self.player_swimming();
        for i in 0..self.entities.len() {
            let e = self.entities[i];
            if !e.active || e.removed {
                continue;
            }
            // A sounding high C hurts on any contact, before the stomp rule
            // (falling onto a singing Operettensänger hurts, D-068).
            if crate::enemies::high_c_column(&e, &self.map, &self.tuning)
                .is_some_and(|c| c.overlaps(&hitbox))
            {
                hurt = true;
                continue;
            }
            // A presented sabre hurts on any contact, before the stomp rule
            // (falling onto a saluting Offizier hurts: the high-C rule,
            // D-068, D-096).
            if crate::enemies::sabre_zone(&e, &self.tuning).is_some_and(|z| z.overlaps(&hitbox)) {
                hurt = true;
                continue;
            }
            // A turning paddle wheel hurts like an enemy that can't be
            // stomped (D-077); a rider on the deck never reaches it.
            if crate::platform::paddle_wheel(&e, &self.tuning).is_some_and(|z| z.overlaps(&hitbox))
            {
                hurt = true;
                continue;
            }
            if !e.hitbox().overlaps(&hitbox) {
                continue;
            }
            let (fx, _) = e.feet();
            match e.kind {
                EntityKind::Oblate => {
                    self.entities[i].removed = true;
                    spawned.extend(self.collect_oblate(fx, e.y));
                }
                EntityKind::Mozartkugel => {
                    self.entities[i].removed = true;
                    if self.player.power == Power::Normal {
                        self.set_player_power(Power::Big);
                        grew = true;
                        self.events.push(Event::PowerUp);
                    }
                    self.add_score(self.tuning.score.power_up);
                    spawned.push(popup(Popup::Score(self.tuning.score.power_up), fx, e.y));
                }
                EntityKind::Zaunerstollen => {
                    self.entities[i].removed = true;
                    if self.add_life() {
                        spawned.push(popup(Popup::ExtraLife, fx, e.y));
                    }
                }
                EntityKind::Hackl => {
                    self.entities[i].removed = true;
                    grew |= self.player.power == Power::Normal;
                    spawned.extend(self.pick_up_hackl(fx, e.y));
                }
                EntityKind::Enemy(_) if !e.harmful() => {}
                EntityKind::Enemy(kind) => {
                    // The part she touches: a Regattaboot bounces only off
                    // its masthead part, its body below hurts (D-080
                    // refinement); every other enemy is its hitbox.
                    let (part, bounces) = match crate::enemies::regatta_parts(&e, &self.tuning) {
                        Some((mast, _)) if mast.overlaps(&hitbox) => (mast, true),
                        Some((_, body)) if body.overlaps(&hitbox) => (body, false),
                        // The strips beside the mast are air.
                        Some(_) => continue,
                        None => (e.hitbox(), true),
                    };
                    // From above: falling, and the feet were above the
                    // part's middle on the previous frame (both positions
                    // from before this frame's movement).
                    let middle = e.prev_y + (part.y - e.y) + part.h.div_int(2);
                    let from_above = bounces && self.player.vy > Fx::ZERO && prev_bottom <= middle;
                    match crate::enemies::contact(kind, from_above, swimming) {
                        // A stomped Kurgast lies down in his deck chair
                        // instead (D-095): 100 in the chain on his first
                        // stomp only; a later one only bounces her (no
                        // points, the chain neither advances nor resets).
                        // „Kurruhe!" stacks above the score popup.
                        Contact::Stomp if kind == EnemyKind::Kurgast => {
                            bounced = true;
                            let scores = crate::enemies::scores_on_defeat(&e);
                            crate::enemies::lie_down(&mut self.entities[i], &self.tuning);
                            let mut top = e.y;
                            if scores {
                                spawned.extend(self.stomp_reward(kind, fx, e.y));
                                top -= Fx::from_px(crate::entity::POPUP_STACK_PX);
                            }
                            spawned.push(popup(Popup::Kurruhe, fx, top));
                        }
                        Contact::Stomp => {
                            bounced = true;
                            crate::enemies::defeat(
                                &mut self.entities[i],
                                kind,
                                false,
                                &self.tuning,
                            );
                            spawned.extend(self.stomp_reward(kind, fx, e.y));
                            spawned.extend(crate::enemies::defeat_popup(kind, fx, e.y));
                            if kind == EnemyKind::Schneekugel {
                                spawned.extend(crate::enemies::globe_shards(&e, &self.tuning));
                            }
                        }
                        Contact::Bounce => {
                            bounced = true;
                            rest_on = Some(rest_on.map_or(e.y, |top| top.min(e.y)));
                            // A Kellner's tray: „Komm' gleich!" on every
                            // bounce, and his `bounce` pose (D-105).
                            spawned.extend(crate::enemies::tray_bounce(&mut self.entities[i]));
                        }
                        // A swarm (and anything while she swims) hurts
                        // even in the grace after a bounce; a non-stompable
                        // enemy's side doesn't (D-027 fallback).
                        Contact::Hurt => {
                            if kind.stompable()
                                || kind.hurts_from_above()
                                || swimming
                                || self.player.bounce_grace == 0
                            {
                                hurt = true;
                            }
                        }
                    }
                }
                // A falling rock hurts on any contact, also from above.
                EntityKind::Thrown | EntityKind::Note | EntityKind::Stein => hurt = true,
                EntityKind::PoppedOblate
                | EntityKind::Steinschlag
                | EntityKind::Souvenirstand
                | EntityKind::Ampel
                | EntityKind::SchneekugelScherben
                | EntityKind::SteinDebris
                | EntityKind::Defeated(_)
                | EntityKind::Debris
                | EntityKind::Popup(_)
                | EntityKind::Bump
                | EntityKind::Checkpoint
                | EntityKind::Sign
                | EntityKind::Festungsbahn
                | EntityKind::Gondel
                | EntityKind::Schafbergbahn
                | EntityKind::Traundl
                | EntityKind::Dampfer
                | EntityKind::Fensterladen
                | EntityKind::ThrownHackl
                | EntityKind::Durchkriechstein => {}
            }
        }
        if grew {
            self.player.fit_head(&self.map);
        }
        if hurt && !self.touches_goal() {
            self.hurt_player();
        }
        if bounced && self.mode == Mode::Playing {
            self.player.bounce(input.jump(), &self.tuning.player);
            if let Some(top) = rest_on {
                self.rest_feet_on(top);
            }
            self.events.push(Event::Stomped);
        }
        self.entities.extend(spawned);
    }

    /// True while the player swims (D-076, [`Player::swimming`]: her chest
    /// point was in swim water when this step began): no stomps and no
    /// roof bounces then, every enemy contact hurts
    /// ([`crate::enemies::contact`]). A jump-out is airborne from its next
    /// step, so it can end in a stomp.
    pub(crate) fn player_swimming(&self) -> bool {
        self.player.swimming
    }

    /// Puts the player's feet on `top` (a bounce off a non-stompable
    /// enemy). The move is upwards and shorter than the hitbox, so checking
    /// the new hitbox covers the whole way: if it overlaps a solid or a
    /// hazard tile she stays and gets `bounce_grace_frames` without harm from
    /// non-stompable enemies instead.
    fn rest_feet_on(&mut self, top: Fx) {
        let placed = Aabb {
            y: top - self.player.h,
            ..self.player.hitbox()
        };
        if self.map.touches(&placed, TileKind::Solid) || self.map.touches_lethal(&placed) {
            self.player.bounce_grace = self.tuning.enemies.bounce_grace_frames;
        } else {
            self.player.y = placed.y;
        }
    }

    /// True if the player's hitbox overlaps an armed goal.
    fn touches_goal(&self) -> bool {
        self.touched_goal(&self.player.hitbox()).is_some()
    }

    /// Points (or a life) for a stomp in the current chain; returns its
    /// popup (none for a life at the lives cap). A chain started by an
    /// enemy worth 200 starts at 200 (D-046, [`EnemyKind::chain_start`]).
    pub(crate) fn stomp_reward(&mut self, kind: EnemyKind, x: Fx, y: Fx) -> Option<Entity> {
        let chain = &self.tuning.score.stomp_chain;
        let mut idx = usize::from(self.player.stomp_chain);
        if idx == 0 {
            idx = kind.chain_start();
        }
        self.player.stomp_chain = (idx + 1).min(usize::from(u8::MAX)) as u8;
        match chain.get(idx) {
            Some(&points) => {
                self.add_score(points);
                Some(popup(Popup::Score(points), x, y))
            }
            None => self.add_life().then(|| popup(Popup::ExtraLife, x, y)),
        }
    }

    /// A projectile (the thrown Hackl, D-056) hit entity `i`: the outcome
    /// from [`crate::enemies::hit_by_projectile`] (`Defeat` enemies knocked
    /// off for 100 / 200, a falling rock shattered for 50) with the score,
    /// its popup and the rock's debris. Returns false if nothing happened
    /// (the Fiaker, pass-through kinds).
    pub(crate) fn defeat_by_projectile(&mut self, i: usize) -> bool {
        let Some(e) = self.entities.get_mut(i) else {
            return false;
        };
        let Some(points) = crate::enemies::hit_by_projectile(e, &self.tuning) else {
            return false;
        };
        let e = *e;
        if e.kind == EntityKind::Stein {
            self.entities
                .extend(crate::enemies::stein_debris(&e, &self.tuning));
        }
        if e.kind == EntityKind::Defeated(EnemyKind::Schneekugel) {
            self.entities
                .extend(crate::enemies::globe_shards(&e, &self.tuning));
        }
        let (fx, _) = e.feet();
        // A Kurgast who has scored is knocked off for nothing (D-095).
        if points > 0 {
            self.add_score(points);
            self.entities.push(popup(Popup::Score(points), fx, e.y));
        }
        if let EntityKind::Defeated(kind) = e.kind {
            self.entities
                .extend(crate::enemies::defeat_popup(kind, fx, e.y));
        }
        true
    }

    /// A power change during play. On land (and standing on the lake bed)
    /// the feet stay in place ([`Player::set_power`]). Floating or diving
    /// (swimming, not on the ground) the top stays, and with it the chest
    /// point (D-076: `chest_px` below the top in every form), so a floater
    /// stays in the surface band after a shrink or a grow (D-086). If the
    /// grown hitbox would reach into a solid tile (just above the lake
    /// bed), the feet stay instead.
    pub(crate) fn set_player_power(&mut self, power: Power) {
        let floating = self.player.swimming && !self.player.on_ground;
        let top = self.player.y;
        self.player.set_power(power, &self.tuning.player);
        if floating {
            let mut b = self.player.hitbox();
            b.y = top;
            if !self.map.touches(&b, TileKind::Solid) {
                self.player.y = top;
            }
        }
    }

    /// A hit steps down one power state (Hackl → Zuckerschock →
    /// Normal, with invulnerability); Normal dies. Nothing happens while
    /// invulnerable or transforming.
    fn hurt_player(&mut self) {
        if self.player.invulnerable > 0 || self.player.transform > 0 {
            return;
        }
        match self.player.power.after_hit() {
            Some(power) => {
                self.set_player_power(power);
                self.player.invulnerable = self.tuning.player.invulnerable_frames;
                self.events.push(Event::Hurt);
            }
            None => self.die(DeathCause::Hit),
        }
    }

    /// One Oblate at feet position (`x`, `y`): score, counter, and every
    /// 100th a life; returns the „+1 Leben" popup for that life (none at the
    /// lives cap, D-023).
    fn collect_oblate(&mut self, x: Fx, y: Fx) -> Option<Entity> {
        self.add_score(self.tuning.score.oblate);
        self.oblaten = self.oblaten.saturating_add(1);
        self.oblaten_total = self.oblaten_total.saturating_add(1);
        self.events.push(Event::OblateCollected);
        if self.oblaten >= self.tuning.game.oblaten_per_life {
            self.oblaten = 0;
            return self.add_life().then(|| popup(Popup::ExtraLife, x, y));
        }
        None
    }

    /// +1 life with an `ExtraLife` event; at the cap nothing happens and
    /// it returns false (callers then show no „+1 Leben" popup).
    fn add_life(&mut self) -> bool {
        if self.lives >= self.tuning.game.lives_max {
            return false;
        }
        self.lives += 1;
        self.events.push(Event::ExtraLife);
        true
    }

    fn add_score(&mut self, points: u32) {
        self.score = self.score.saturating_add(points).min(MAX_SCORE);
    }

    /// Activates every checkpoint whose x the player's centre has passed
    /// (the furthest one counts).
    fn check_checkpoints(&mut self) {
        let (cx, _) = self.player.center_px();
        for (i, s) in self.level.entities.iter().enumerate() {
            if s.kind != SpawnKind::Checkpoint || cx < s.col * TILE_PX + TILE_PX / 2 {
                continue;
            }
            let further = match self.checkpoint {
                None => true,
                Some(c) => s.col > self.level.entities[c as usize].col,
            };
            if further {
                self.checkpoint = Some(i as u16);
                self.events.push(Event::CheckpointReached);
                let x = TILE.mul_int(s.col) + TILE.div_int(2);
                let y = TILE.mul_int(s.row + 1) - Fx::from_px(CHECKPOINT_POPUP_ABOVE_GROUND_PX);
                self.entities.push(popup(Popup::Checkpoint, x, y));
            }
        }
    }

    // ------------------------------------------------------------ tooling

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
        let start = self.level.player_start();
        let mut p = match found {
            Some((c, r)) => Player::spawn(c, r - 1, t),
            None => Player::spawn(start.col, start.row, t),
        };
        if found.is_none_or(|(c, _)| c == col) {
            p.x = Fx::from_px(x_px) - p.w.div_int(2);
        }
        if found.is_none() {
            p.y = self.player.y;
            p.on_ground = false;
        }
        self.player = p;
        self.place_camera();
        self.mode = Mode::Playing;
    }

    // ---------------------------------------------------------- hashing

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
            Mode::GameOver { frames } => {
                h.u8(3);
                h.u16(frames);
            }
            Mode::Title { frames } => {
                h.u8(4);
                h.u16(frames);
            }
            Mode::StageIntro { timer } => {
                h.u8(5);
                h.u16(timer);
            }
            Mode::Paused => h.u8(6),
            Mode::RouteMap => h.u8(7),
            Mode::Ending => h.u8(8),
        }
        // Only runs with a route have these (lone-level hashes stay as
        // they were before routes existed).
        if self.route.is_some() {
            h.u8(self.stage);
            h.u8(self.first_stage);
            match &self.route_map {
                None => h.u8(0),
                Some(m) => {
                    h.u8(1);
                    m.hash(&mut h);
                }
            }
        }
        h.u8(match self.start {
            Start::Playing => 0,
            Start::Title => 1,
        });
        match &self.clear {
            None => h.u8(0),
            Some(c) => {
                h.u8(1);
                c.hash(&mut h);
            }
        }
        self.player.hash(&mut h);
        self.camera.hash(&mut h);
        h.i32(self.ground_feet);
        h.i32(self.settle_feet);
        h.i32(self.slack_x);
        // Only auto-scroll levels have it (other hashes stay as they were).
        if self.autoscroll_zone().is_some() {
            self.scroll.hash(&mut h);
        }
        h.u64(self.rng.state());
        h.u32(self.deaths);
        self.hash_tiles(&mut h);
        h.u32(self.entities.len() as u32);
        for e in &self.entities {
            e.hash(&mut h);
        }
        h.u16(self.checkpoint.unwrap_or(u16::MAX));
        h.u32(self.score);
        h.u8(self.oblaten);
        // D-107: the run's Oblaten total, the ending and the title mark
        // (E47 re-pinned the testbed's FINISH_HASH with them).
        h.u32(self.oblaten_total);
        match &self.ending {
            None => h.u8(0),
            Some(e) => {
                h.u8(1);
                e.hash(&mut h);
            }
        }
        h.bool(self.cleared);
        h.u8(self.lives);
        h.u16(self.time);
        h.u8(self.time_frames);
        h.u32(self.stage_clock);
        h.u8(match self.death_cause {
            None => 0,
            Some(DeathCause::Hazard) => 1,
            Some(DeathCause::Pit) => 2,
            Some(DeathCause::Hit) => 3,
            Some(DeathCause::TimeUp) => 4,
            Some(DeathCause::Squeezed) => 5,
        });
        h.bool(self.start_held);
        h.finish()
    }

    /// Coarse hash of the level progress, for the solver's de-duplication
    /// key: changed tiles, `oblaten_10` progress, the checkpoint, the
    /// player's power state and every gameplay entity (spawn, position in
    /// 4 px cells, direction); for enemies, projectiles and the moving
    /// platforms also their state, facing and pattern counters (moving
    /// enemies at the same spot can be at different points of their
    /// pattern). Cosmetic entities (popups, debris, bumps, defeated
    /// enemies) and item timers are left out, so states that only differ
    /// in those count as the same. `state_hash` covers everything exactly.
    /// While an Offizier is in the entity list, also the stage clock (E41,
    /// D-099): an officer not yet woken starts his drill at the clock, and
    /// two nodes of one frame can differ in it (a power-up freeze), so they
    /// are different states. Shutters need no clock (their frame is their
    /// `count`); other levels hash as before.
    pub fn world_hash(&self) -> u64 {
        let mut h = Fnv::new();
        self.hash_tiles(&mut h);
        h.u16(self.checkpoint.unwrap_or(u16::MAX));
        h.u8(self.player.power.id());
        // The scroll (phase, position, hold): with the same player state a
        // later scroll start is a different state (D-075). Only auto-scroll
        // levels, so the other levels' search is unchanged.
        if self.autoscroll_zone().is_some() {
            self.scroll.hash(&mut h);
        }
        // The entities (most of the work) a word at a time (E35): the same
        // fields as before, packed into words, through `WordHash`.
        let mut w = WordHash::new();
        let mut on_the_clock = false;
        for e in &self.entities {
            on_the_clock |= e.kind == EntityKind::Enemy(EnemyKind::Offizier);
            if matches!(
                e.kind,
                EntityKind::Popup(_)
                    | EntityKind::Debris
                    | EntityKind::SteinDebris
                    | EntityKind::SchneekugelScherben
                    | EntityKind::Bump
                    | EntityKind::Defeated(_)
            ) {
                continue;
            }
            let spawn = u64::from(e.spawn.unwrap_or(u16::MAX));
            let dir_x = (e.vx.signum() + 1) as u64; // 0, 1 or 2
            let rising = u64::from(e.rising(&self.tuning));
            w.word(spawn | dir_x << 16 | rising << 18);
            let cell = |v: i32| u64::from((v >> 10) as u32);
            w.word(cell(e.x.raw()) | cell(e.y.raw()) << 32);
            if matches!(
                e.kind,
                EntityKind::Enemy(_)
                    | EntityKind::Thrown
                    | EntityKind::Note
                    | EntityKind::Steinschlag
                    | EntityKind::Souvenirstand
                    | EntityKind::Ampel
                    | EntityKind::Stein
                    | EntityKind::Festungsbahn
                    | EntityKind::Gondel
                    | EntityKind::Schafbergbahn
                    | EntityKind::Traundl
                    | EntityKind::Dampfer
                    | EntityKind::Fensterladen
                    | EntityKind::ThrownHackl
                    | EntityKind::Durchkriechstein
            ) {
                w.word(
                    u64::from(e.active)
                        | u64::from(e.state) << 8
                        | u64::from(e.dir as u8) << 16
                        | u64::from(e.count) << 32,
                );
            }
        }
        if on_the_clock {
            w.word(u64::from(self.stage_clock));
        }
        h.u64(w.finish());
        h.finish()
    }

    /// Changed tiles and `oblaten_10` progress.
    fn hash_tiles(&self, h: &mut Fnv) {
        h.u32(self.changed_cells.len() as u32);
        for &c in &self.changed_cells {
            h.u32(c);
            hash_tile(&self.map.tile_at_index(c as usize), h);
        }
        h.u32(self.multi.len() as u32);
        for m in &self.multi {
            h.u32(m.cell);
            h.u8(m.hits);
            h.u32(m.deadline);
        }
    }

    // --------------------------------------------------------- accessors

    /// Ticks since the start.
    pub fn frame(&self) -> u32 {
        self.frame
    }
    /// The current mode.
    pub fn mode(&self) -> Mode {
        self.mode
    }
    /// The level being played (start state: tile names, spawns, meta).
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
    /// The auto-scroll camera's state (D-075; the default in levels
    /// without a zone).
    pub fn autoscroll(&self) -> Autoscroll {
        self.scroll
    }
    /// The current tile grid (collision and looks; bricks break, boxes empty).
    pub fn map(&self) -> &TileMap {
        &self.map
    }
    /// The current tile at a grid cell, or `None` outside the grid.
    pub fn tile(&self, col: i32, row: i32) -> Option<Tile> {
        self.map.tile(col, row)
    }
    /// How far the tile at a cell is raised by a running bump, whole pixels.
    pub fn tile_bump_px(&self, col: i32, row: i32) -> i32 {
        self.bump_at(col, row)
            .map_or(0, |b| b.bump_offset_px(&self.tuning))
    }
    /// Goal trigger boxes.
    pub fn goals(&self) -> &[Aabb] {
        &self.goals
    }
    /// The entities, in update order.
    pub fn entities(&self) -> &[Entity] {
        &self.entities
    }
    /// How often the player died so far.
    pub fn deaths(&self) -> u32 {
        self.deaths
    }
    /// Score (0..=999 999).
    pub fn score(&self) -> u32 {
        self.score
    }
    /// The Oblaten counter (0..100; at 100 it gives a life and restarts at 0).
    pub fn oblaten(&self) -> u8 {
        self.oblaten
    }

    /// Every Oblate collected since the run started or the last continue
    /// (never wraps, D-107).
    pub fn oblaten_total(&self) -> u32 {
        self.oblaten_total
    }

    /// The ending (while `Mode::Ending`, D-107).
    pub fn ending(&self) -> Option<&Ending> {
        self.ending.as_ref()
    }

    /// The session's „durchgespielt" mark: the ending has been seen (D-107).
    pub fn cleared(&self) -> bool {
        self.cleared
    }

    /// The level spawn of goal number `goal` (index into [`Game::goals`]).
    pub fn goal_spawn(&self, goal: usize) -> Option<&crate::level::EntitySpawn> {
        self.level
            .entities
            .iter()
            .filter(|e| e.kind.is_goal())
            .nth(goal)
    }

    /// Whether goal number `goal` can be touched: a Maibaum always, a
    /// `goal_stollen` once no undefeated Oberkellner is in the level
    /// (D-107; unarmed it is decoration).
    pub fn goal_armed(&self, goal: usize) -> bool {
        self.goal_spawn(goal).is_some_and(|s| {
            goal_kind_armed(s.kind, crate::enemies::boss_undefeated(&self.entities))
        })
    }

    /// The first armed goal `hb` overlaps.
    fn touched_goal(&self, hb: &Aabb) -> Option<usize> {
        (0..self.goals.len()).find(|&i| self.goals[i].overlaps(hb) && self.goal_armed(i))
    }
    /// Lives left (counting the current one).
    pub fn lives(&self) -> u8 {
        self.lives
    }
    /// The stage clock (D-094, shared with the Offiziere's drill, D-096):
    /// frames of play since the stage started or since the last respawn,
    /// exactly the frames the ZEIT timer counts (frozen, paused, dying and
    /// goal-sequence frames don't count). Fensterläden run on it.
    pub fn stage_clock(&self) -> u32 {
        self.stage_clock
    }

    /// Seconds left on the timer.
    pub fn time_left(&self) -> u16 {
        self.time
    }
    /// Index into `Level::entities` of the furthest checkpoint passed.
    pub fn checkpoint(&self) -> Option<usize> {
        self.checkpoint.map(usize::from)
    }
    /// Why the player died (set while `Dying` and in `GameOver`).
    pub fn death_cause(&self) -> Option<DeathCause> {
        self.death_cause
    }
    /// The goal sequence (while `Mode::StageClear`).
    pub fn stage_clear(&self) -> Option<&StageClear> {
        self.clear.as_ref()
    }
    /// Which flow screens this game has.
    pub fn start(&self) -> Start {
        self.start
    }
    /// The run's route (`None` for a lone level).
    pub fn route(&self) -> Option<&Route> {
        self.route.as_deref()
    }
    /// The current station (index into [`crate::route::STATIONS`]), if the
    /// game has a route.
    pub fn station(&self) -> Option<usize> {
        self.route.as_ref().map(|_| usize::from(self.stage))
    }
    /// The route map (while `Mode::RouteMap`).
    pub fn route_map(&self) -> Option<&RouteMap> {
        self.route_map.as_ref()
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

/// Whether a goal of `kind` can be touched (D-107): a Maibaum always, a
/// `goal_stollen` only while no Oberkellner is undefeated.
pub(crate) fn goal_kind_armed(kind: SpawnKind, boss_undefeated: bool) -> bool {
    kind != SpawnKind::GoalStollen || !boss_undefeated
}

/// D-054: in a climb level an entity activates only while its spawn cell
/// (or, for entities without a spawn, its hitbox's top row) is within
/// `margin` px of the screen vertically ([`Camera::row_near`]).
pub fn spawn_row_near(level: &Level, e: &Entity, camera: Camera, margin: i32) -> bool {
    let row = match e.spawn.and_then(|i| level.entities.get(usize::from(i))) {
        Some(s) => s.row,
        None => tile_of(e.y),
    };
    camera.row_near(row, margin)
}

/// The parts of a game derived from its level alone: the start grid, the
/// goal boxes and the hash of everything that never changes while playing
/// it (grid, goals, spawns, tuning).
///
/// A Maibaum occupies its cell's column from one level height above the
/// top of the level down to the cell's bottom edge; a `goal_stollen` only
/// its cell (D-107).
fn level_parts(level: &Level, tuning: &Tuning) -> (Arc<TileMap>, Arc<[Aabb]>, u64) {
    let map = Arc::new(TileMap::from_level(level));
    let goals: Vec<Aabb> = level
        .entities
        .iter()
        .filter(|e| e.kind.is_goal())
        .map(|e| match e.kind {
            SpawnKind::GoalStollen => Aabb {
                x: TILE.mul_int(e.col),
                y: TILE.mul_int(e.row),
                w: TILE,
                h: TILE,
            },
            _ => Aabb {
                x: TILE.mul_int(e.col),
                y: -TILE.mul_int(level.height),
                w: TILE,
                h: TILE.mul_int(level.height + e.row + 1),
            },
        })
        .collect();

    let mut h = Fnv::new();
    map.hash(&mut h);
    // D-103: only when present, so a level without it keeps its hash.
    if let Some(top) = level.meta.camera_top {
        h.bytes(b"camera_top");
        h.i32(top);
    }
    h.u32(goals.len() as u32);
    for g in &goals {
        for v in [g.x, g.y, g.w, g.h] {
            h.i32(v.raw());
        }
    }
    h.u32(level.entities.len() as u32);
    for e in &level.entities {
        h.bytes(e.kind.name().as_bytes());
        h.i32(e.col);
        h.i32(e.row);
    }
    // Debug formatting of plain integers is stable, and it covers every
    // tuning field without a hand-written list.
    h.bytes(format!("{tuning:?}").as_bytes());
    (map, goals.into(), h.finish())
}
