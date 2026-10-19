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

mod flow;

pub use flow::{ClearPhase, StageClear, Start, maibaum_points};

use std::sync::Arc;

use crate::camera::{Camera, ClimbFocus, SCREEN_H, SCREEN_W};
use crate::entity::{EnemyKind, Entity, EntityKind, Popup, UpdateCtx, popup};
use crate::fx::Fx;
use crate::hash::Fnv;
use crate::input::InputFrame;
use crate::level::{Block, CameraMode, Contents, Level, SpawnKind, TILE_PX, Tile, TileKind};
use crate::physics::{Aabb, TILE, TileMap, hash_tile, tile_of};
use crate::player::{Player, Power};
use crate::rng::Rng;
use crate::route::{Route, RouteMap};
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
}

impl Event {
    /// Every event, in declaration order. Keep in sync with the enum: a new
    /// variant fails the build in [`Event::name`] (exhaustive `match`), which
    /// is the reminder to list it here too.
    pub const ALL: [Event; 14] = [
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
    rng: Rng,
    deaths: u32,
    tuning: Tuning,
    pub(crate) entities: Vec<Entity>,
    multi: Vec<MultiBlock>,
    pub(crate) score: u32,
    pub(crate) oblaten: u8,
    pub(crate) lives: u8,
    /// Seconds left.
    pub(crate) time: u16,
    /// Frames into the current second.
    time_frames: u8,
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
            rng: Rng::new(seed),
            deaths: 0,
            tuning: *tuning,
            entities: Vec::new(),
            multi: Vec::new(),
            score: 0,
            oblaten: 0,
            lives: tuning.game.lives_start,
            time: 0,
            time_frames: 0,
            checkpoint: None,
            death_cause: None,
            start_held: false,
            start,
            clear: None,
            route,
            stage,
            first_stage: stage,
            route_map: None,
            static_hash,
            events: Vec::new(),
        };
        game.reset_level();
        game.place_camera();
        if start == Start::Title {
            game.mode = Mode::Title { frames: 0 };
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
        &self.events
    }

    fn step_playing(&mut self, input: InputFrame) {
        // Growing or shrinking freezes everything (timer included).
        if self.player.transform > 0 {
            self.player.transform -= 1;
            self.player.jump_held = input.jump();
            return;
        }

        let prev_bottom = self.player.y + self.player.h;
        let was_on_ground = self.player.on_ground;
        let min_x = Fx::from_px(self.camera.x);
        let head_row = self.player.update(
            input,
            &self.map,
            &self.tuning.player,
            min_x,
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
        self.update_entities();
        self.follow_carried_left();
        self.interact(prev_bottom, input);
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
        if let Some(goal) = self.goals.iter().position(|g| g.overlaps(&hb)) {
            self.touch_goal(goal);
        }
        self.follow_camera();
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

    /// One frame of the level's camera (side: dead zone; climb: D-054).
    fn follow_camera(&mut self) {
        let (w, h) = (self.map.width_fx().to_px(), self.map.height_fx().to_px());
        let t = &self.tuning.camera;
        if !self.climb() {
            self.camera.follow(self.player.center_px(), w, h, t);
            return;
        }
        let p = &self.player;
        let mut focus = ClimbFocus {
            cx: p.center_px().0,
            top: p.y.to_px(),
            feet: (p.y + p.h).to_px(),
            grounded: p.on_ground,
            ground_feet: self.ground_feet,
        };
        self.camera.follow_climb(&mut focus, w, h, t);
        self.ground_feet = focus.ground_feet;
    }

    /// D-051: the player's own movement stops at the screen's left edge,
    /// so only a moving platform's carry can take her past it; the camera
    /// then scrolls left by the same amount (the one exception to "never
    /// scrolls left"), and she stays on the cabin.
    fn follow_carried_left(&mut self) {
        let left = self.player.x.to_px();
        if left < self.camera.x {
            self.camera.x = left.max(0);
        }
    }

    /// One frame of the countdown; at 0 the player dies.
    fn tick_timer(&mut self) {
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

    /// Back to the level's start state: tiles, entities, timer.
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
    /// change (climb: at the settle target at once, D-054).
    fn place_camera(&mut self) {
        let (w, h) = (self.map.width_fx().to_px(), self.map.height_fx().to_px());
        let t = &self.tuning.camera;
        if self.climb() {
            let feet = (self.player.y + self.player.h).to_px();
            self.camera = Camera::new_climb(self.player.center_px().0, feet, w, h, t);
            self.ground_feet = feet;
        } else {
            self.camera = Camera::new(self.player.center_px(), w, h, t);
            self.ground_feet = 0;
        }
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
                if self.player.power == Power::Big {
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
    /// it are knocked off for `score.bump_enemy` each, items on it hop.
    fn knock_enemies_on(&mut self, col: i32, row: i32) {
        let knocked = crate::enemies::knock_off_block(&mut self.entities, col, row, &self.tuning);
        for (_, x, y) in knocked {
            let points = self.tuning.score.bump_enemy;
            self.add_score(points);
            self.entities.push(popup(Popup::Score(points), x, y));
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
        let ctx = UpdateCtx {
            map: &self.map,
            tuning: &self.tuning,
            level: &self.level,
            player_cx: self.player.x + self.player.w.div_int(2),
            player_standing: self.player.on_ground.then(|| self.player.hitbox()),
            screen_left: Fx::from_px(self.camera.x),
        };
        let mut spawned = Vec::new();
        for e in &mut self.entities {
            // Snapshot for the stomp rule (previous-frame position).
            e.prev_y = e.y;
            if !e.active
                && e.x < activate_x
                && (!climb || spawn_row_near(level, e, camera, margin_y))
            {
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
            if e.kind.is_platform() {
                crate::platform::carry(
                    &mut self.player,
                    old,
                    e,
                    ctx.map,
                    &ctx.tuning.player,
                    &mut self.events,
                );
            }
            // Moving platforms never despawn (D-027): a player who fell
            // off must be able to wait for the car.
            if !e.kind.is_platform() && (e.right() < despawn_x || e.y >= bottom) {
                e.removed = true;
            }
        }
        crate::enemies::turn_at_enemies(&mut self.entities);
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
        for i in 0..self.entities.len() {
            let e = self.entities[i];
            if !e.active || e.removed || !e.hitbox().overlaps(&hitbox) {
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
                        self.player.set_power(Power::Big, &self.tuning.player);
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
                EntityKind::Enemy(_) if !e.harmful() => {}
                // A swarm hurts on any contact, also from above.
                EntityKind::Enemy(kind) if kind.hurts_from_above() => hurt = true,
                EntityKind::Enemy(kind) => {
                    // Stomp: falling, and the feet were above the enemy's
                    // middle on the previous frame (both positions from
                    // before this frame's movement).
                    let middle = e.prev_y + e.h.div_int(2);
                    if self.player.vy > Fx::ZERO && prev_bottom <= middle {
                        bounced = true;
                        if !kind.stompable() {
                            rest_on = Some(rest_on.map_or(e.y, |top| top.min(e.y)));
                        } else {
                            crate::enemies::defeat(
                                &mut self.entities[i],
                                kind,
                                false,
                                &self.tuning,
                            );
                            spawned.extend(self.stomp_reward(kind, fx, e.y));
                        }
                    } else if kind.stompable() || self.player.bounce_grace == 0 {
                        hurt = true;
                    }
                }
                EntityKind::Thrown | EntityKind::Note => hurt = true,
                EntityKind::PoppedOblate
                | EntityKind::Defeated(_)
                | EntityKind::Debris
                | EntityKind::Popup(_)
                | EntityKind::Bump
                | EntityKind::Checkpoint
                | EntityKind::Sign
                | EntityKind::Festungsbahn
                | EntityKind::Gondel => {}
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

    /// True if the player's hitbox overlaps a goal.
    fn touches_goal(&self) -> bool {
        let hb = self.player.hitbox();
        self.goals.iter().any(|g| g.overlaps(&hb))
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

    /// A hit: Zuckerschock shrinks (with invulnerability), Normal dies.
    /// Nothing happens while invulnerable or transforming.
    fn hurt_player(&mut self) {
        if self.player.invulnerable > 0 || self.player.transform > 0 {
            return;
        }
        match self.player.power {
            Power::Big => {
                self.player.set_power(Power::Normal, &self.tuning.player);
                self.player.invulnerable = self.tuning.player.invulnerable_frames;
                self.events.push(Event::Hurt);
            }
            Power::Normal => self.die(DeathCause::Hit),
        }
    }

    /// One Oblate at feet position (`x`, `y`): score, counter, and every
    /// 100th a life; returns the „+1 Leben" popup for that life (none at the
    /// lives cap, D-023).
    fn collect_oblate(&mut self, x: Fx, y: Fx) -> Option<Entity> {
        self.add_score(self.tuning.score.oblate);
        self.oblaten = self.oblaten.saturating_add(1);
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
        h.u8(self.lives);
        h.u16(self.time);
        h.u8(self.time_frames);
        h.u8(match self.death_cause {
            None => 0,
            Some(DeathCause::Hazard) => 1,
            Some(DeathCause::Pit) => 2,
            Some(DeathCause::Hit) => 3,
            Some(DeathCause::TimeUp) => 4,
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
    pub fn world_hash(&self) -> u64 {
        let mut h = Fnv::new();
        self.hash_tiles(&mut h);
        h.u16(self.checkpoint.unwrap_or(u16::MAX));
        h.u8(match self.player.power {
            Power::Normal => 0,
            Power::Big => 1,
        });
        for e in &self.entities {
            if matches!(
                e.kind,
                EntityKind::Popup(_)
                    | EntityKind::Debris
                    | EntityKind::Bump
                    | EntityKind::Defeated(_)
            ) {
                continue;
            }
            h.u16(e.spawn.unwrap_or(u16::MAX));
            h.i32(e.x.raw() >> 10);
            h.i32(e.y.raw() >> 10);
            h.i32(e.vx.signum());
            h.bool(e.rising(&self.tuning));
            if matches!(
                e.kind,
                EntityKind::Enemy(_)
                    | EntityKind::Thrown
                    | EntityKind::Note
                    | EntityKind::Festungsbahn
                    | EntityKind::Gondel
            ) {
                h.bool(e.active);
                h.u8(e.state);
                h.u8(e.dir as u8);
                h.u16(e.count);
            }
        }
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
    /// Lives left (counting the current one).
    pub fn lives(&self) -> u8 {
        self.lives
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
/// A goal occupies its cell's column from one level height above the top
/// of the level down to the cell's bottom edge.
fn level_parts(level: &Level, tuning: &Tuning) -> (Arc<TileMap>, Arc<[Aabb]>, u64) {
    let map = Arc::new(TileMap::from_level(level));
    let goals: Vec<Aabb> = level
        .entities
        .iter()
        .filter(|e| e.kind == SpawnKind::Goal)
        .map(|e| Aabb {
            x: TILE.mul_int(e.col),
            y: -TILE.mul_int(level.height),
            w: TILE,
            h: TILE.mul_int(level.height + e.row + 1),
        })
        .collect();

    let mut h = Fnv::new();
    map.hash(&mut h);
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
