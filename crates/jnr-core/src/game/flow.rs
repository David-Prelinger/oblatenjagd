//! The game flow around play: how a game starts ([`Start`]) and the Maibaum
//! goal sequence ([`StageClear`], `docs/game-design.md` → "Goal sequence").
//!
//! Title, stage intro and pause are small mode switches in `Game::step`;
//! the goal sequence is a phase machine stepped once per frame while
//! `Mode::StageClear`:
//!
//! 1. touch: enemies freeze, the timer stops, height score (+ „Wurstkranz!"),
//! 2. [`ClearPhase::Slide`] down the pole at `goal.slide_speed`, just left of it,
//! 3. [`ClearPhase::Hop`] off to the right of the pole (player gravity),
//! 4. [`ClearPhase::Walk`] right at walk speed until off screen (during 2-4
//!    the camera's x stays and its y follows her with the play dead zone, so
//!    a high touch ends at the ground framing, D-040; in climb levels it
//!    settles one way only to the pole base at `camera.climb_goal_feet_y`
//!    instead, D-054 and the D-059 addendum),
//! 5. [`ClearPhase::Tally`]: one second of time per frame, `score.time_bonus` each,
//! 6. [`ClearPhase::Clear`]: the stage-clear texts for `goal.clear_frames`,
//! 7. in the real flow of a run with a route (D-042): the route map
//!    (`Mode::RouteMap`, below); otherwise [`ClearPhase::ToBeContinued`] for
//!    `goal.to_be_continued_frames`, then the title screen (the run is reset
//!    to the stage start; M1 behaviour, kept for `Start::Playing` and lone
//!    levels).
//!
//! A `goal_stollen` (the finale, D-107) has its own start instead of 1–4:
//! 10 000 points (`score.stollen`) and „Zaunerstollen!", then
//! [`ClearPhase::Serve`] where she stands (if she is in the air she first
//! falls with the player's gravity) for `goal.serve_frames` frames from
//! the frame she stands; then 5 and 6 as above, and after them, in the
//! real flow on the journey's last station, the ending (`Mode::Ending`,
//! module `ending`) instead of 7.
//!
//! The route map (D-043) is a phase machine too ([`MapPhase`]): Arrive
//! (`map.arrive_frames`) → Walk (`map.walk_px` px per frame along the path
//! to the next station) → Wait (until START, at most `map.wait_frames`) →
//! the next stage's intro with score, lives, Oblaten and the power state
//! carried over (timer reset, checkpoint cleared). If the next station has
//! no level, Teaser (`map.teaser_frames`, START ignored) replaces Wait and
//! the run ends on the title screen, reset to its first stage. START during
//! Arrive or Walk skips to Wait (or Teaser) with Resi on the next station.

use super::{Event, Game, Mode};
use crate::camera::SCREEN_W;
use crate::entity::{EntityKind, Popup, popup};
use crate::fx::Fx;
use crate::hash::Fnv;
use crate::level::{SpawnKind, TILE_PX};
use crate::physics::YHit;
use crate::player::Facing;
use crate::route::{self, MapPhase, RouteMap};
use crate::tuning::GoalTuning;

/// How a game starts, and with it which flow screens it has (D-025).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Start {
    /// Directly in `Playing`, without stage intros or pause; a death
    /// respawns right after `Dying` (tests, tools, the solver and replays
    /// by default). The goal sequence still ends in `Mode::Title`; START
    /// there restarts the stage directly in play.
    Playing,
    /// The real game: the title screen first; START → stage intro → play;
    /// an intro after every death and continue; START pauses.
    Title,
}

impl Start {
    /// Every value.
    pub const ALL: [Start; 2] = [Start::Playing, Start::Title];

    /// The name used in replay files (`start = title`).
    pub const fn name(self) -> &'static str {
        match self {
            Start::Playing => "playing",
            Start::Title => "title",
        }
    }

    /// The value with this name.
    pub fn from_name(name: &str) -> Option<Start> {
        Start::ALL.into_iter().find(|s| s.name() == name)
    }
}

/// The phases of the goal sequence, in order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClearPhase {
    /// Sliding down the pole (anim `pole`).
    Slide,
    /// Hopping off at the base, to the right of the pole.
    Hop,
    /// Walking right until off screen.
    Walk,
    /// Counting the time left into the score.
    Tally,
    /// The stage-clear texts.
    Clear,
    /// „Fortsetzung folgt …" (M1: one stage only).
    ToBeContinued,
    /// The finale (`goal_stollen`, D-107): she holds the plate up where
    /// she stands (falling there first if she is in the air).
    Serve,
}

/// The state of the goal sequence (`Game::stage_clear`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StageClear {
    /// The current phase.
    pub phase: ClearPhase,
    /// Frames spent in the current phase (0 on the frame it began).
    pub timer: u16,
    /// Pole centre, level px.
    pub pole_x: i32,
    /// Pole base (bottom edge of the goal cell), level px.
    pub base_y: i32,
    /// Feet height above the base at the touch, whole px (≥ 0).
    pub height_px: i32,
    /// Points for that height.
    pub height_points: u32,
    /// Time bonus counted so far.
    pub bonus: u32,
}

impl StageClear {
    pub(super) fn hash(&self, h: &mut Fnv) {
        h.u8(match self.phase {
            ClearPhase::Slide => 0,
            ClearPhase::Hop => 1,
            ClearPhase::Walk => 2,
            ClearPhase::Tally => 3,
            ClearPhase::Clear => 4,
            ClearPhase::ToBeContinued => 5,
            ClearPhase::Serve => 6,
        });
        h.u16(self.timer);
        h.i32(self.pole_x);
        h.i32(self.base_y);
        h.i32(self.height_px);
        h.u32(self.height_points);
        h.u32(self.bonus);
    }
}

/// Maibaum points for a feet height (whole px above the pole base) and
/// whether it is the top zone (the Wurstkranz).
pub fn maibaum_points(height_px: i32, g: &GoalTuning) -> (u32, bool) {
    let zone = g.zone_heights.iter().filter(|&&h| height_px >= h).count();
    (g.zone_scores[zone], zone == g.zone_heights.len())
}

impl Game {
    /// The player touched goal number `goal` (index into `goals`).
    pub(super) fn touch_goal(&mut self, goal: usize) {
        let Some(&crate::level::EntitySpawn { kind, col, row, .. }) = self.goal_spawn(goal) else {
            return;
        };
        if kind == SpawnKind::GoalStollen {
            self.touch_stollen(col, row);
            return;
        }
        let pole_x = col * TILE_PX + TILE_PX / 2;
        let base_y = (row + 1) * TILE_PX;
        let feet = self.player.y + self.player.h;
        let height_px = (Fx::from_px(base_y) - feet).to_px().max(0);
        let (points, wurstkranz) = maibaum_points(height_px, &self.tuning.goal);
        self.add_score(points);
        let (fx, _) = self.player.feet();
        self.entities
            .push(popup(Popup::Score(points), fx, self.player.y));
        if wurstkranz {
            self.entities.push(popup(
                Popup::Wurstkranz,
                fx,
                self.player.y - Fx::from_px(10),
            ));
        }

        let p = &mut self.player;
        p.x = Fx::from_px(pole_x - self.tuning.goal.pole_offset) - p.w;
        p.vx = Fx::ZERO;
        p.vy = Fx::ZERO;
        p.on_ground = false;
        p.facing = Facing::Right;
        p.skidding = false;
        p.jump_rising = false;
        p.invulnerable = 0;
        self.mode = Mode::StageClear;
        self.clear = Some(StageClear {
            phase: ClearPhase::Slide,
            timer: 0,
            pole_x,
            base_y,
            height_px,
            height_points: points,
            bonus: 0,
        });
        self.events.push(Event::LevelComplete);
    }

    /// The player touched the armed `goal_stollen` in cell (`col`, `row`)
    /// (D-107): 10 000 points and „Zaunerstollen!" above her, enemies and
    /// the timer stop, and the Serve starts where she is (no horizontal
    /// speed; in the air she keeps her vertical speed and falls).
    fn touch_stollen(&mut self, col: i32, row: i32) {
        let points = self.tuning.score.stollen;
        self.add_score(points);
        let (fx, _) = self.player.feet();
        let top = self.player.y;
        self.entities.push(popup(Popup::Score(points), fx, top));
        self.entities
            .push(popup(Popup::Stollen, fx, top - Fx::from_px(10)));
        let p = &mut self.player;
        p.vx = Fx::ZERO;
        p.skidding = false;
        p.jump_rising = false;
        p.invulnerable = 0;
        self.mode = Mode::StageClear;
        self.clear = Some(StageClear {
            phase: ClearPhase::Serve,
            timer: 0,
            pole_x: col * TILE_PX + TILE_PX / 2,
            base_y: (row + 1) * TILE_PX,
            height_px: 0,
            height_points: points,
            bonus: 0,
        });
        self.events.push(Event::LevelComplete);
    }

    /// One frame of falling in the Serve (player gravity, stopped by solid
    /// and one-way tiles; the level's bottom edge holds her too, a café
    /// has no pits). Returns true once she stands.
    fn serve_fall(&mut self) -> bool {
        let t = self.tuning.player;
        let bottom = self.map.height_fx();
        let p = &mut self.player;
        p.vy = (p.vy + t.gravity).min(t.max_fall_speed);
        let mut b = p.hitbox();
        let hit = self.map.move_y(&mut b, p.vy, false);
        p.y = b.y;
        if hit != YHit::None {
            p.vy = Fx::ZERO;
        }
        if p.y + p.h >= bottom {
            p.y = bottom - p.h;
            p.vy = Fx::ZERO;
            p.on_ground = true;
        }
        if hit == YHit::Floor {
            p.on_ground = true;
        }
        p.on_ground
    }

    /// One frame of the goal sequence.
    pub(super) fn step_stage_clear(&mut self) {
        self.update_cosmetic_entities();
        let Some(mut c) = self.clear else {
            return;
        };
        c.timer = c.timer.saturating_add(1);
        let g = self.tuning.goal;
        let base = Fx::from_px(c.base_y);
        let next = match c.phase {
            ClearPhase::Slide => {
                let p = &mut self.player;
                let feet = (p.y + p.h + g.slide_speed).min(base);
                p.y = feet - p.h;
                (feet >= base).then(|| {
                    p.x = Fx::from_px(c.pole_x + g.pole_offset);
                    p.vx = self.tuning.player.walk_speed;
                    p.vy = -g.hop_speed;
                    ClearPhase::Hop
                })
            }
            ClearPhase::Hop => {
                let p = &mut self.player;
                p.vy = (p.vy + self.tuning.player.gravity).min(self.tuning.player.max_fall_speed);
                p.x += p.vx;
                p.y += p.vy;
                (p.vy > Fx::ZERO && p.y + p.h >= base).then(|| {
                    p.y = base - p.h;
                    p.vy = Fx::ZERO;
                    p.on_ground = true;
                    ClearPhase::Walk
                })
            }
            ClearPhase::Walk => {
                let p = &mut self.player;
                p.vx = self.tuning.player.walk_speed;
                p.x += p.vx;
                // Off screen with the whole sprite (16 px wide around the
                // hitbox centre), whatever the hitbox width.
                let off = Fx::from_px(self.camera.x + SCREEN_W + TILE_PX);
                (p.x >= off).then(|| {
                    p.vx = Fx::ZERO;
                    if self.time > 0 {
                        ClearPhase::Tally
                    } else {
                        ClearPhase::Clear
                    }
                })
            }
            ClearPhase::Tally => {
                if self.time > 0 {
                    self.time -= 1;
                    let bonus = self.tuning.score.time_bonus;
                    c.bonus = c.bonus.saturating_add(bonus);
                    self.add_score(bonus);
                }
                (self.time == 0).then_some(ClearPhase::Clear)
            }
            ClearPhase::Serve => {
                // Counted from the frame she stands.
                if !self.player.on_ground {
                    self.serve_fall();
                    c.timer = 0;
                }
                (self.player.on_ground && c.timer >= g.serve_frames).then_some(if self.time > 0 {
                    ClearPhase::Tally
                } else {
                    ClearPhase::Clear
                })
            }
            ClearPhase::Clear => {
                if c.timer >= g.clear_frames && (self.enter_ending() || self.enter_route_map()) {
                    return;
                }
                (c.timer >= g.clear_frames).then_some(ClearPhase::ToBeContinued)
            }
            ClearPhase::ToBeContinued => {
                if c.timer >= g.to_be_continued_frames {
                    // Title with the run reset to its first stage (a no-op
                    // switch for lone levels).
                    self.end_run();
                    return;
                }
                None
            }
        };
        if matches!(
            c.phase,
            ClearPhase::Slide | ClearPhase::Hop | ClearPhase::Walk | ClearPhase::Serve
        ) {
            let level_h = self.map.height_fx().to_px();
            let t = &self.tuning.camera;
            if self.climb() {
                // D-054: feet at `climb_goal_feet_y` on the walk ground
                // line (the pole base), which frames the meadow like
                // Stages 1–3. Calm (D-059 addendum, E19): the target is
                // the final framing from the touch on, so the camera
                // moves one way only and ignores the hop; it never lets
                // her feet sink below that framing while she slides.
                let feet = (self.player.y + self.player.h).to_px();
                self.camera.settle_y_towards(
                    c.base_y - t.climb_goal_feet_y,
                    feet - t.climb_goal_feet_y,
                    level_h,
                    t,
                );
            } else {
                let (_, cy) = self.player.center_px();
                self.camera.follow_y(cy, level_h, t);
                if self.autoscroll_zone().is_some() {
                    // D-090: the top clamp holds in the goal sequence too.
                    self.camera.clamp_autoscroll_top(level_h, t);
                }
                // D-103: so does a side level's `camera_top` (validation
                // keeps the two clamps apart: only one ever applies).
                self.clamp_camera_top();
            }
        }
        if let Some(phase) = next {
            c.phase = phase;
            c.timer = 0;
        }
        self.clear = Some(c);
    }

    /// Popups, debris, bumps and popped Oblaten keep moving during the goal
    /// sequence; enemies and items stand still.
    fn update_cosmetic_entities(&mut self) {
        let mut spawned = Vec::new();
        let bottom = self.map.height_fx();
        let ctx = crate::entity::UpdateCtx {
            map: &self.map,
            tuning: &self.tuning,
            level: &self.level,
            player_cx: self.player.x + self.player.w.div_int(2),
            player_top: self.player.y,
            player_box: self.player.hitbox(),
            player_standing: self.player.on_ground.then(|| self.player.hitbox()),
            screen_left: Fx::from_px(self.camera.x),
            screen_top: Fx::from_px(self.camera.y),
            globes: &[],
            stage_clock: self.stage_clock(),
            cars: &[],
            bills: 0,
        };
        for e in &mut self.entities {
            let cosmetic = matches!(
                e.kind,
                EntityKind::Popup(_)
                    | EntityKind::Debris
                    | EntityKind::SteinDebris
                    | EntityKind::SchneekugelScherben
                    | EntityKind::Bump
                    | EntityKind::PoppedOblate
                    | EntityKind::Defeated(_)
            );
            if cosmetic && e.active && !e.removed {
                spawned.extend(e.update(&ctx));
                if e.y >= bottom {
                    e.removed = true;
                }
            }
        }
        self.entities.extend(spawned);
        self.entities.retain(|e| !e.removed);
    }

    /// After the stage-clear texts: the route map, if this is the real flow
    /// of a run with a route and a next station exists. Returns whether
    /// the map started.
    fn enter_route_map(&mut self) -> bool {
        let Some(route) = &self.route else {
            return false;
        };
        let from = usize::from(self.stage);
        if self.start != Start::Title || from + 1 >= route::STATIONS.len() {
            return false;
        }
        self.route_map = Some(RouteMap {
            phase: MapPhase::Arrive,
            timer: 0,
            from: self.stage,
            walked: 0,
            teaser: route.level(from + 1).is_none(),
        });
        self.clear = None;
        self.mode = Mode::RouteMap;
        true
    }

    /// One frame of the route map; `start_pressed` is a new START press.
    pub(super) fn step_route_map(&mut self, start_pressed: bool) {
        let Some(mut m) = self.route_map else {
            return;
        };
        m.timer = m.timer.saturating_add(1);
        let t = self.tuning.map;
        let len = route::path_len(usize::from(m.from)).clamp(0, i32::from(u16::MAX)) as u16;
        let arrived = if m.teaser {
            MapPhase::Teaser
        } else {
            MapPhase::Wait
        };
        let next = match m.phase {
            MapPhase::Arrive | MapPhase::Walk if start_pressed => {
                m.walked = len;
                Some(arrived)
            }
            MapPhase::Arrive => (m.timer >= t.arrive_frames).then_some(MapPhase::Walk),
            MapPhase::Walk => {
                m.walked = m.walked.saturating_add(t.walk_px).min(len);
                (m.walked >= len).then_some(arrived)
            }
            MapPhase::Wait => {
                if start_pressed || m.timer >= t.wait_frames {
                    self.next_stage();
                    return;
                }
                None
            }
            MapPhase::Teaser => {
                if m.timer >= t.teaser_frames {
                    self.end_run();
                    return;
                }
                None
            }
        };
        if let Some(phase) = next {
            m.phase = phase;
            m.timer = 0;
        }
        self.route_map = Some(m);
    }

    /// The next stage's intro: its level from the start, with score,
    /// lives, Oblaten and the power state carried over; the timer is the
    /// new level's, the checkpoint is cleared (D-042).
    fn next_stage(&mut self) {
        self.route_map = None;
        let next = usize::from(self.stage) + 1;
        let Some(level) = self.route.as_ref().and_then(|r| r.level(next)).cloned() else {
            self.end_run();
            return;
        };
        let power = self.player.power;
        self.stage = next as u8;
        self.set_level(level);
        self.checkpoint = None;
        self.clear = None;
        self.death_cause = None;
        self.reset_level();
        self.place_player();
        if power.is_big() {
            // Zuckerschock and Hackl carry over (D-042, D-056).
            self.player.set_power(power, &self.tuning.player);
            // Carried over, not eaten: no growing freeze.
            self.player.transform = 0;
            self.player.fit_head(&self.map);
        }
        self.enter_stage();
    }

    /// After the teaser: the title screen, the run reset to its first
    /// stage (score 0, full lives, Oblaten 0, Normal).
    pub(super) fn end_run(&mut self) {
        self.route_map = None;
        if self.stage != self.first_stage {
            if let Some(level) = self
                .route
                .as_ref()
                .and_then(|r| r.level(usize::from(self.first_stage)))
                .cloned()
            {
                self.stage = self.first_stage;
                self.set_level(level);
            }
        }
        self.reset_run();
        self.mode = Mode::Title { frames: 0 };
        self.frame_title();
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    /// E47's reserved tag (D-107): `ClearPhase::Serve` hashes as 6.
    #[test]
    fn serve_hashes_with_its_reserved_tag() {
        let c = StageClear {
            phase: ClearPhase::Serve,
            timer: 3,
            pole_x: 1,
            base_y: 2,
            height_px: 0,
            height_points: 10_000,
            bonus: 0,
        };
        let mut h = Fnv::new();
        c.hash(&mut h);
        let mut six = Fnv::new();
        six.u8(6);
        six.u16(3);
        six.i32(1);
        six.i32(2);
        six.i32(0);
        six.u32(10_000);
        six.u32(0);
        assert_eq!(h.finish(), six.finish());
    }
}
