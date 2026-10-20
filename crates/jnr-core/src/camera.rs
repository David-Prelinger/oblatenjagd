//! The camera: follows the player like SMB (never scrolls left, except to follow a rider carried left by a platform, D-051).
//!
//! Horizontally every level is the same. Vertically a level picks
//! (`[meta] camera`, D-054):
//! - `side` (default, Stages 1–3): [`Camera::follow`] keeps the player's
//!   centre inside the dead zone `camera.deadzone_top..=deadzone_bottom`;
//! - `climb`: [`Camera::follow_climb`], SMW style. Grounded, the camera
//!   settles by at most `climb_settle_speed` px per frame towards the
//!   framing with the *settle feet* ([`ClimbFocus::settle_feet`]) at
//!   screen y `climb_feet_y`. The settle feet follow the ground down at
//!   once, but up only by what a step exceeds `climb_up_slack` (D-065), so
//!   standing on a box or plank up to 3 tiles high keeps the ground she
//!   came from on screen; once she walks more than `climb_up_reach` px
//!   away from where she stepped up, they catch up with her feet. In the
//!   air it follows up at once when the hitbox top rises above screen y
//!   `climb_top` (while the slack holds, that line moves up by the slack's
//!   lag, but stays at least `climb_top_min` on screen; grounded again
//!   above `climb_top`, it catches up `climb_catch_up_speed` px per frame
//!   faster), and down while the feet are below `climb_feet_y`, but
//!   never more than `climb_fall_follow` px below the framing of the last
//!   ground itself ([`ClimbFocus::ground_feet`], so the death line is
//!   where it was without the slack). `Game` kills the player when her
//!   hitbox top is below the screen (the death line), and level entities
//!   activate only near the screen vertically ([`Camera::row_near`]).
//! - `autoscroll` (D-075): the side camera, except while its left edge
//!   lies in the level's zone `[from × 16, to × 16)`: there
//!   [`Camera::follow_autoscroll`] moves it right by
//!   `camera.autoscroll_speed` subpixels every frame of play, independent
//!   of the player ([`Autoscroll`] keeps the phase, the subpixel position
//!   and the respawn hold). `Game` pushes the player at the left edge
//!   (squeezing her against a solid tile kills) and stops her
//!   `autoscroll_right_margin` px before the right edge. Vertically it is
//!   the side camera's dead zone around a rest (D-086, D-090): it starts at
//!   the level's bottom clamp; while she stands it settles towards the
//!   framing with the ground she stands on at screen y
//!   `autoscroll_ground_y` (no higher than `autoscroll_top`, no lower than
//!   the bottom clamp), and while she floats at the surface towards the
//!   bottom clamp, by at most `autoscroll_settle_speed` px per frame. In
//!   every phase y never goes above `autoscroll_top` (the top clamp).

use crate::fx::Fx;
use crate::hash::Fnv;
use crate::level::TILE_PX;
use crate::physics::Aabb;
use crate::tuning::CameraTuning;

/// Screen width in pixels.
pub const SCREEN_W: i32 = 320;
/// Screen height in pixels.
pub const SCREEN_H: i32 = 180;

/// Top-left corner of the visible 320×180 area, in whole level pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Camera {
    /// Left edge in level pixels.
    pub x: i32,
    /// Top edge in level pixels.
    pub y: i32,
}

/// What the climb camera needs to know about the player, in whole level
/// pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClimbFocus {
    /// Hitbox centre x.
    pub cx: i32,
    /// Hitbox top.
    pub top: i32,
    /// Hitbox bottom (the feet).
    pub feet: i32,
    /// Standing on a tile, a one-way tile or a moving platform.
    pub grounded: bool,
    /// Feet height of the last grounded frame (set by
    /// [`Camera::follow_climb`] while grounded): the fall-follow limit's
    /// reference.
    pub ground_feet: i32,
    /// The feet height the ground framing is anchored on (D-065): the
    /// camera settles with these feet at `climb_feet_y`. Updated by
    /// [`Camera::follow_climb`] while grounded; start and respawn set it to
    /// the feet.
    pub settle_feet: i32,
    /// Hitbox centre x where she stepped up onto the ground she stands on
    /// while `settle_feet` is held below her feet (D-065); start and
    /// respawn set it to `cx`.
    pub slack_x: i32,
}

/// Where the auto-scroll camera is relative to its zone (D-075).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ScrollPhase {
    /// Before the zone: the side camera.
    #[default]
    LeadIn,
    /// In the zone: the camera scrolls on its own.
    Scroll,
    /// After the zone: the side camera again (never back to the scroll).
    LeadOut,
}

/// The auto-scroll camera's state (D-075); unused (the default) in other
/// levels.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Autoscroll {
    /// Lead-in, scroll or lead-out.
    pub phase: ScrollPhase,
    /// In the scroll: the camera's left edge in subpixels (the camera's x
    /// is its whole-px floor).
    pub pos: Fx,
    /// In the scroll: frames of play left before it moves (after a start or
    /// respawn framed inside the zone).
    pub hold: u16,
}

/// What the auto-scroll camera's vertical rest knows about the player
/// this frame (D-086, D-090).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rest {
    /// In the air, diving, or on a jump-out's way up: no settle.
    None,
    /// Standing on ground (a tile, deck, boat, quay or the lake bed) whose
    /// top is at this level y (her feet).
    Ground(i32),
    /// Floating with the chest in the surface band.
    Surface,
}

impl Autoscroll {
    /// The state for a camera just framed at `x` (start, respawn, stage
    /// change) with the zone `[from, to)` in level pixels: inside the
    /// zone the scroll waits `camera.autoscroll_hold` frames.
    pub fn placed(x: i32, zone: (i32, i32), t: &CameraTuning) -> Autoscroll {
        let (from, to) = zone;
        let phase = if x < from {
            ScrollPhase::LeadIn
        } else if x < to {
            ScrollPhase::Scroll
        } else {
            ScrollPhase::LeadOut
        };
        let in_scroll = phase == ScrollPhase::Scroll;
        Autoscroll {
            phase,
            pos: if in_scroll { Fx::from_px(x) } else { Fx::ZERO },
            hold: if in_scroll { t.autoscroll_hold } else { 0 },
        }
    }

    pub(crate) fn hash(&self, h: &mut Fnv) {
        h.u8(match self.phase {
            ScrollPhase::LeadIn => 0,
            ScrollPhase::Scroll => 1,
            ScrollPhase::LeadOut => 2,
        });
        h.i32(self.pos.raw());
        h.u16(self.hold);
    }
}

impl Camera {
    /// A camera looking at `focus` (level pixels, usually the player's
    /// centre): horizontally as if it had just followed it, vertically
    /// centred on the dead zone; clamped to the level (`level_w` × `level_h`
    /// pixels).
    pub fn new(focus: (i32, i32), level_w: i32, level_h: i32, t: &CameraTuning) -> Camera {
        let mid = (t.deadzone_top + t.deadzone_bottom) / 2;
        Camera {
            x: (focus.0 - t.follow_x).clamp(0, (level_w - SCREEN_W).max(0)),
            y: clamp_y(focus.1 - mid, level_h),
        }
    }

    /// The auto-scroll camera's start (and respawn) framing (D-086):
    /// horizontally like [`Camera::new`], vertically at its rest, the
    /// level's bottom clamp, unless that leaves `focus` outside the dead
    /// zone (then the dead zone's nearest edge); never above the top clamp
    /// `autoscroll_top` (D-090).
    pub fn new_autoscroll(
        focus: (i32, i32),
        level_w: i32,
        level_h: i32,
        t: &CameraTuning,
    ) -> Camera {
        let mut c = Camera::new(focus, level_w, level_h, t);
        c.y = rest_y(level_h);
        c.follow_y(focus.1, level_h, t);
        c.clamp_autoscroll_top(level_h, t);
        c
    }

    /// The auto-scroll camera's top clamp (D-090): y at least
    /// `autoscroll_top` (or the bottom clamp, if that is higher), so the
    /// water stays on screen however high she jumps.
    pub fn clamp_autoscroll_top(&mut self, level_h: i32, t: &CameraTuning) {
        self.y = self.y.max(t.autoscroll_top.min(rest_y(level_h)));
    }

    /// Follows `focus`: scrolls right once it passes `follow_x` on screen
    /// (never left), keeps it inside the vertical dead zone, and clamps to
    /// the level.
    pub fn follow(&mut self, focus: (i32, i32), level_w: i32, level_h: i32, t: &CameraTuning) {
        let (fx, fy) = focus;
        self.follow_x(fx, level_w, t);
        self.follow_y(fy, level_h, t);
    }

    /// Only the vertical part of [`Camera::follow`]: keeps `fy` inside the
    /// dead zone, clamped to the level; x stays (the goal sequence, D-040).
    pub fn follow_y(&mut self, fy: i32, level_h: i32, t: &CameraTuning) {
        if fy - self.y < t.deadzone_top {
            self.y = fy - t.deadzone_top;
        } else if fy - self.y > t.deadzone_bottom {
            self.y = fy - t.deadzone_bottom;
        }
        self.y = clamp_y(self.y, level_h);
    }

    /// The horizontal part of every camera: scrolls right once `fx` passes
    /// `follow_x` on screen (never left), clamped to the level.
    fn follow_x(&mut self, fx: i32, level_w: i32, t: &CameraTuning) {
        if fx - self.x > t.follow_x {
            self.x = fx - t.follow_x;
        }
        self.x = self.x.clamp(0, (level_w - SCREEN_W).max(0));
    }

    /// One frame of the auto-scroll camera (D-075) with the zone
    /// `[from, to)` in level pixels. Lead-in: the side camera, until
    /// following would put x at or past `from`; then x is exactly `from`
    /// and the scroll starts (no hold). Scroll: after the hold, the
    /// position grows by `autoscroll_speed` subpixels; x is its whole-px
    /// floor, and at `to` the lead-out starts. Lead-out: the side camera,
    /// moving right at most `autoscroll_catch_up_speed` px per frame, so
    /// the hand-over never snaps (D-086).
    /// Vertically in every phase (D-086, D-090): while she rests, y first
    /// settles by at most `autoscroll_settle_speed` px towards the rest
    /// ([`Rest`]: standing, the framing with her ground at screen y
    /// `autoscroll_ground_y`; floating, the bottom clamp); then the side
    /// camera's dead zone; then the top clamp (`autoscroll_top`).
    #[allow(clippy::too_many_arguments)]
    pub fn follow_autoscroll(
        &mut self,
        s: &mut Autoscroll,
        zone: (i32, i32),
        focus: (i32, i32),
        rest: Rest,
        level_w: i32,
        level_h: i32,
        t: &CameraTuning,
    ) {
        let (from, to) = zone;
        match s.phase {
            ScrollPhase::LeadIn => {
                self.follow_x(focus.0, level_w, t);
                if self.x >= from {
                    self.x = from;
                    *s = Autoscroll {
                        phase: ScrollPhase::Scroll,
                        pos: Fx::from_px(from),
                        hold: 0,
                    };
                }
            }
            ScrollPhase::Scroll => {
                if s.hold > 0 {
                    s.hold -= 1;
                } else {
                    s.pos += t.autoscroll_speed;
                }
                self.x = s.pos.to_px();
                if self.x >= to {
                    self.x = to;
                    s.phase = ScrollPhase::LeadOut;
                }
            }
            ScrollPhase::LeadOut => {
                // From where the scroll ended, the side camera catches up
                // with her at most `autoscroll_catch_up_speed` px a frame
                // (faster than she runs), instead of snapping (D-086).
                let x = self.x;
                self.follow_x(focus.0, level_w, t);
                self.x = self.x.min(x + t.autoscroll_catch_up_speed);
            }
        }
        if let Some(target) = rest_target(rest, level_h, t) {
            self.settle_y_by(target, t.autoscroll_settle_speed, level_h);
        }
        self.follow_y(focus.1, level_h, t);
        self.clamp_autoscroll_top(level_h, t);
    }

    /// True if `b` lies completely inside the screen (touching its edges
    /// counts as inside). Enemy triggers in an auto-scroll zone need it, so
    /// nothing starts off screen (D-075).
    pub fn fully_on_screen(&self, b: &Aabb) -> bool {
        let (left, top) = (Fx::from_px(self.x), Fx::from_px(self.y));
        b.x >= left
            && b.right() <= left + Fx::from_px(SCREEN_W)
            && b.y >= top
            && b.bottom() <= top + Fx::from_px(SCREEN_H)
    }

    /// The climb camera's start (and respawn) framing: horizontally like
    /// [`Camera::new`], vertically at the settle target (feet at screen y
    /// `climb_feet_y`) at once; clamped to the level.
    pub fn new_climb(cx: i32, feet: i32, level_w: i32, level_h: i32, t: &CameraTuning) -> Camera {
        Camera {
            x: (cx - t.follow_x).clamp(0, (level_w - SCREEN_W).max(0)),
            y: clamp_y(feet - t.climb_feet_y, level_h),
        }
    }

    /// One frame of the climb camera (module docs, D-054, D-065).
    /// Horizontally like [`Camera::follow`]. Grounded: remembers the feet
    /// in `f.ground_feet`, moves `f.settle_feet` down to the feet at once
    /// but up only once the feet are more than `climb_up_slack` px above it
    /// (and then by the excess) or she is more than `climb_up_reach` px
    /// (horizontally) away from where she stepped up (`f.slack_x`), and
    /// settles towards it. In the air: up at once when the top is above
    /// `climb_top` (while the slack holds: `climb_top` minus the lag of the
    /// settle feet below the last ground, at least `climb_top_min`), down
    /// with the feet below `climb_feet_y` but at most
    /// `climb_fall_follow` px below the last ground's own framing (never
    /// up because of that limit). Clamped to the level.
    pub fn follow_climb(
        &mut self,
        f: &mut ClimbFocus,
        level_w: i32,
        level_h: i32,
        t: &CameraTuning,
    ) {
        self.follow_x(f.cx, level_w, t);
        if f.grounded {
            let new_ground = f.feet != f.ground_feet;
            f.ground_feet = f.feet;
            if f.feet >= f.settle_feet || f.settle_feet - f.feet > t.climb_up_slack {
                // On or below the settle ground, or more than the slack
                // above it: max = settle down onto lower ground at once;
                // min = keep the settle feet at most the slack below her.
                f.settle_feet = f.settle_feet.max(f.feet).min(f.feet + t.climb_up_slack);
                f.slack_x = f.cx;
            } else if new_ground {
                f.slack_x = f.cx;
            } else if (f.cx - f.slack_x).abs() > t.climb_up_reach {
                f.settle_feet = f.feet;
            }
            self.settle_y(f.settle_feet - t.climb_feet_y, level_h, t);
            // A landing after a jump the lowered up-follow line didn't
            // follow can leave the top above `climb_top`: catch up faster
            // (`climb_catch_up_speed` more px per frame) until it is below.
            if f.top - self.y < t.climb_top {
                let up = (self.y - t.climb_catch_up_speed).max(f.top - t.climb_top);
                self.y = clamp_y(up, level_h);
            }
            return;
        }
        // While the slack holds (D-065) the framing is `lag` px lower than
        // the last ground's own, so the up-follow line moves up by as much
        // (no bob when she jumps from a held box), but never above
        // `climb_top_min`: her top always stays that far on screen.
        let lag = (f.settle_feet - f.ground_feet).max(0);
        let top_line = (t.climb_top - lag).max(t.climb_top_min);
        if f.top - self.y < top_line {
            self.y = f.top - top_line;
        } else if f.feet - self.y > t.climb_feet_y {
            let limit = f.ground_feet - t.climb_feet_y + t.climb_fall_follow;
            self.y = self.y.max((f.feet - t.climb_feet_y).min(limit));
        }
        self.y = clamp_y(self.y, level_h);
    }

    /// Moves y towards `target` (clamped to the level) by at most
    /// `climb_settle_speed` px; x stays. The climb camera's ground settle
    /// and its goal-sequence framing.
    pub fn settle_y(&mut self, target: i32, level_h: i32, t: &CameraTuning) {
        self.settle_y_by(target, t.climb_settle_speed, level_h);
    }

    /// Moves y towards `target` (clamped to the level) by at most `step`
    /// px; x stays.
    fn settle_y_by(&mut self, target: i32, step: i32, level_h: i32) {
        let target = clamp_y(target, level_h);
        self.y = clamp_y(self.y + (target - self.y).clamp(-step, step), level_h);
    }

    /// The climb goal sequence's camera (D-059 addendum): like
    /// [`Camera::settle_y`] towards a fixed `target`, but never less than
    /// `floor` (the camera y that keeps the player's feet at the goal
    /// framing, so a slide faster than the settle can't drop her out of
    /// view). With `floor ≤ target`, as when her feet never go below the
    /// pole base, y moves monotonically towards `target` and never past it.
    pub fn settle_y_towards(&mut self, target: i32, floor: i32, level_h: i32, t: &CameraTuning) {
        self.settle_y(target, level_h, t);
        let target = clamp_y(target, level_h);
        if self.y < target {
            self.y = self.y.max(clamp_y(floor, level_h).min(target));
        }
    }

    /// True if grid row `row` is at most `margin` px above or below the
    /// screen (touching or overlapping it counts). Climb levels activate a
    /// level entity only while its spawn row is that near
    /// (`camera.climb_activate_y`, D-054); entity code with a vertical
    /// trigger of its own can use it too.
    pub fn row_near(&self, row: i32, margin: i32) -> bool {
        let top = row * TILE_PX;
        top + TILE_PX >= self.y - margin && top <= self.y + SCREEN_H + margin
    }

    pub(crate) fn hash(&self, h: &mut Fnv) {
        h.i32(self.x);
        h.i32(self.y);
    }
}

/// The auto-scroll camera's vertical rest (D-086): the bottom clamp of a
/// `level_h` px tall level.
fn rest_y(level_h: i32) -> i32 {
    clamp_y(level_h - SCREEN_H, level_h)
}

/// The camera y the auto-scroll camera settles towards (D-090), if she
/// rests: standing, her ground at screen y `autoscroll_ground_y`, between
/// the top clamp and the bottom clamp; floating, the bottom clamp.
fn rest_target(rest: Rest, level_h: i32, t: &CameraTuning) -> Option<i32> {
    let bottom = rest_y(level_h);
    match rest {
        Rest::None => None,
        Rest::Ground(top) => Some(
            (top - t.autoscroll_ground_y)
                .max(t.autoscroll_top)
                .min(bottom),
        ),
        Rest::Surface => Some(bottom),
    }
}

/// `y` clamped to the camera's range in a `level_h` px tall level.
fn clamp_y(y: i32, level_h: i32) -> i32 {
    y.clamp(0, (level_h - SCREEN_H).max(0))
}

#[cfg(test)]
mod tests {
    use super::*;

    const T: CameraTuning = CameraTuning {
        follow_x: 144,
        deadzone_top: 64,
        deadzone_bottom: 120,
        climb_feet_y: 148,
        climb_settle_speed: 2,
        climb_up_slack: 48,
        climb_up_reach: 48,
        climb_top: 40,
        climb_top_min: 11,
        climb_catch_up_speed: 6,
        climb_fall_follow: 80,
        climb_goal_feet_y: 164,
        climb_activate_y: 64,
        autoscroll_speed: Fx(192),
        autoscroll_hold: 60,
        autoscroll_right_margin: 64,
        autoscroll_settle_speed: 2,
        autoscroll_catch_up_speed: 4,
        autoscroll_ground_y: 84,
        autoscroll_top: 24,
    };
    /// An auto-scroll zone [8, 40) in columns, in px.
    const ZONE: (i32, i32) = (128, 640);
    const W: i32 = 1000;
    const H: i32 = 224;
    /// A tall (climb) level: 42 rows.
    const TALL: i32 = 672;

    #[test]
    fn starts_clamped() {
        let c = Camera::new((40, 180), W, H, &T);
        assert_eq!(c, Camera { x: 0, y: 44 });
        let c = Camera::new((990, 0), W, H, &T);
        assert_eq!(
            c,
            Camera {
                x: W - SCREEN_W,
                y: 0
            }
        );
    }

    #[test]
    fn scrolls_right_never_left() {
        let mut c = Camera { x: 0, y: 0 };
        c.follow((100, 90), W, H, &T);
        assert_eq!(c.x, 0, "inside the dead zone");
        c.follow((200, 90), W, H, &T);
        assert_eq!(c.x, 56);
        c.follow((150, 90), W, H, &T);
        assert_eq!(c.x, 56, "never scrolls left");
        c.follow((5000, 90), W, H, &T);
        assert_eq!(c.x, W - SCREEN_W, "clamped to the level");
    }

    #[test]
    fn vertical_dead_zone() {
        let mut c = Camera { x: 0, y: 44 };
        c.follow((10, 150), W, H, &T);
        assert_eq!(c.y, 44, "inside: 150 - 44 = 106");
        c.follow((10, 100), W, H, &T);
        assert_eq!(c.y, 36, "above the dead zone");
        c.follow((10, 10), W, H, &T);
        assert_eq!(c.y, 0, "clamped at the top");
        c.follow((10, 220), W, H, &T);
        assert_eq!(c.y, 44, "clamped at the bottom");
        let mut short = Camera { x: 0, y: 0 };
        short.follow((10, 170), 300, 100, &T);
        assert_eq!(
            short,
            Camera { x: 0, y: 0 },
            "level smaller than the screen"
        );
    }

    #[test]
    fn follow_y_keeps_x() {
        let mut c = Camera { x: 300, y: 0 };
        c.follow_y(200, H, &T);
        assert_eq!(c, Camera { x: 300, y: 44 });
        c.follow_y(80, H, &T);
        assert_eq!(c, Camera { x: 300, y: 16 });
    }

    /// The focus with `ground` as both the last ground and the settle feet.
    fn focus(feet: i32, grounded: bool, ground: i32) -> ClimbFocus {
        ClimbFocus {
            cx: 100,
            top: feet - 22,
            feet,
            grounded,
            ground_feet: ground,
            settle_feet: ground,
            slack_x: 100,
        }
    }

    #[test]
    fn climb_starts_at_the_settle_target_clamped() {
        assert_eq!(
            Camera::new_climb(100, 400, W, TALL, &T),
            Camera { x: 0, y: 252 },
            "feet at screen y 148"
        );
        assert_eq!(Camera::new_climb(400, 648, W, TALL, &T).y, TALL - SCREEN_H);
        assert_eq!(Camera::new_climb(400, 648, W, TALL, &T).x, 256);
        assert_eq!(Camera::new_climb(100, 60, W, TALL, &T).y, 0);
    }

    #[test]
    fn climb_settles_on_the_ground_at_2_px_per_frame() {
        let mut c = Camera { x: 0, y: 300 };
        let mut f = focus(400, true, 0);
        c.follow_climb(&mut f, W, TALL, &T);
        assert_eq!((f.ground_feet, f.settle_feet), (400, 400), "the ground");
        assert_eq!(c.y, 298, "moves up by 2");
        for _ in 0..30 {
            c.follow_climb(&mut f, W, TALL, &T);
        }
        assert_eq!(c.y, 252, "reached the target and stays");
        assert_eq!(400 - c.y, T.climb_feet_y);
        // Downwards as well, and only 1 px when 1 px is left.
        let mut c = Camera { x: 0, y: 249 };
        c.follow_climb(&mut f, W, TALL, &T);
        assert_eq!(c.y, 251);
        c.follow_climb(&mut f, W, TALL, &T);
        assert_eq!(c.y, 252);
    }

    #[test]
    fn climb_keeps_the_framing_on_a_step_up_of_up_to_3_tiles() {
        // Settled on the ground (feet 400), she stands on a box 3 tiles up:
        // the camera stays, so the ground below stays on screen.
        let mut c = Camera { x: 0, y: 252 };
        let mut f = focus(352, true, 400);
        for _ in 0..60 {
            c.follow_climb(&mut f, W, TALL, &T);
        }
        assert_eq!(f.settle_feet, 400, "still anchored on the ground");
        assert_eq!(f.ground_feet, 352, "but stands on the box");
        assert_eq!(c.y, 252);
        assert_eq!(400 - c.y, T.climb_feet_y, "the ground's framing");
        // Back down on the ground: nothing to do.
        let mut f = focus(400, true, f.settle_feet);
        c.follow_climb(&mut f, W, TALL, &T);
        assert_eq!((c.y, f.settle_feet), (252, 400));
    }

    #[test]
    fn climb_settles_up_only_by_the_excess_over_the_slack() {
        // A 4-tile step-up: 16 px more than the slack.
        let mut c = Camera { x: 0, y: 252 };
        let mut f = focus(336, true, 400);
        for _ in 0..60 {
            c.follow_climb(&mut f, W, TALL, &T);
        }
        assert_eq!(f.settle_feet, 384);
        assert_eq!(c.y, 252 - 16, "settled up by 16 px");
        // A shaft of planks 3 tiles apart: the first keeps the framing,
        // every further one moves it by 3 tiles.
        let mut c = Camera { x: 0, y: 252 };
        let mut settle = 400;
        for (feet, want) in [(352, 252), (304, 204), (256, 156)] {
            let mut f = focus(feet, true, settle);
            for _ in 0..60 {
                c.follow_climb(&mut f, W, TALL, &T);
            }
            settle = f.settle_feet;
            assert_eq!(c.y, want, "plank at feet {feet}");
            assert!(f.top - c.y >= T.climb_top, "the top stays below climb_top");
        }
    }

    #[test]
    fn climb_slack_holds_only_within_the_reach_of_the_step_up() {
        // Settled on the ground (feet 400), she steps up 2 tiles at x 100
        // and walks on along the higher ground.
        let mut c = Camera { x: 0, y: 252 };
        let mut f = focus(400, true, 400);
        f.feet = 368;
        f.top = 346;
        c.follow_climb(&mut f, W, TALL, &T);
        assert_eq!((f.slack_x, f.settle_feet), (100, 400), "stepped up");
        f.cx = 148;
        c.follow_climb(&mut f, W, TALL, &T);
        assert_eq!((f.settle_feet, c.y), (400, 252), "48 px on: still held");
        f.cx = 149;
        c.follow_climb(&mut f, W, TALL, &T);
        assert_eq!(f.settle_feet, 368, "49 px on: catches up");
        assert_eq!(c.y, 250, "settling at 2 px per frame");
        // Back the other way it counts too, and every new ground (here a
        // jump onto a step 1 tile higher) starts the reach again.
        let mut f = focus(400, true, 400);
        f.feet = 384;
        f.cx = 60;
        c.follow_climb(&mut f, W, TALL, &T);
        assert_eq!(f.slack_x, 60);
        f.feet = 368;
        f.cx = 70;
        c.follow_climb(&mut f, W, TALL, &T);
        assert_eq!((f.slack_x, f.settle_feet), (70, 400), "new ground");
        f.cx = 22;
        c.follow_climb(&mut f, W, TALL, &T);
        assert_eq!(f.settle_feet, 400, "48 px to the left of the step");
        f.cx = 21;
        c.follow_climb(&mut f, W, TALL, &T);
        assert_eq!(f.settle_feet, 368, "49 px to the left of the step");
    }

    #[test]
    fn climb_settles_down_at_once_below_the_settle_feet() {
        // Anchored on the ground below a box, she steps down 1 tile: the
        // framing follows down.
        let mut c = Camera { x: 0, y: 252 };
        let mut f = focus(416, true, 400);
        c.follow_climb(&mut f, W, TALL, &T);
        assert_eq!(f.settle_feet, 416);
        assert_eq!(c.y, 254);
    }

    #[test]
    fn climb_fall_follow_limit_is_measured_from_the_last_ground() {
        // On a box 3 tiles above the settle ground (camera at the ground's
        // framing) she walks off into a deep hole: the limit is 80 px below
        // the box's own framing, as without the slack (D-065), so the death
        // line doesn't move.
        let mut c = Camera { x: 0, y: 252 };
        let mut f = ClimbFocus {
            ground_feet: 352,
            ..focus(352 + 400, false, 400)
        };
        c.follow_climb(&mut f, W, TALL, &T);
        assert_eq!(c.y, 352 - 148 + 80);
    }

    #[test]
    fn climb_holds_still_for_a_jump_that_stays_inside_the_band() {
        let mut c = Camera { x: 0, y: 252 };
        // A 3-tile jump: feet 48 px up, top at screen y 78 > 40.
        let mut f = focus(352, false, 400);
        c.follow_climb(&mut f, W, TALL, &T);
        assert_eq!(c.y, 252);
        assert_eq!(
            (f.ground_feet, f.settle_feet),
            (400, 400),
            "only grounded frames update them"
        );
    }

    #[test]
    fn climb_up_follow_line_moves_up_by_the_slack_lag() {
        // On a box 3 tiles up, held at the ground's framing (lag 48): the
        // line is 40 - 48, floored at `climb_top_min` 11.
        let mut c = Camera { x: 0, y: 252 };
        let mut f = ClimbFocus {
            ground_feet: 352,
            ..focus(352 - 60, false, 400)
        };
        // Top at screen y 270 - 252 = 18: above climb_top, but no follow.
        c.follow_climb(&mut f, W, TALL, &T);
        assert_eq!(c.y, 252);
        f.top = 252 + 11;
        c.follow_climb(&mut f, W, TALL, &T);
        assert_eq!(c.y, 252, "11 px: still none");
        f.top = 252 + 3;
        c.follow_climb(&mut f, W, TALL, &T);
        assert_eq!(c.y, 244, "follows only to keep the top at 11");
        // A lag of 16 (a 1-tile step) moves the line to 24.
        let mut c = Camera { x: 0, y: 252 };
        let mut f = ClimbFocus {
            ground_feet: 384,
            top: 252 + 20,
            ..focus(330, false, 400)
        };
        c.follow_climb(&mut f, W, TALL, &T);
        assert_eq!(c.y, 248);
    }

    #[test]
    fn climb_grounded_top_above_climb_top_catches_up_at_8_px_per_frame() {
        // Landing on a plank 6 tiles above the settle feet with the camera
        // still low (top at screen y 12): the excess settle moves 2 px and
        // the catch-up 6 more per frame until the top is at climb_top.
        let mut c = Camera { x: 0, y: 252 };
        let mut f = focus(304, true, 400);
        f.ground_feet = 352;
        f.top = 252 + 12;
        let mut ys = Vec::new();
        for _ in 0..5 {
            c.follow_climb(&mut f, W, TALL, &T);
            ys.push(c.y);
        }
        assert_eq!(f.settle_feet, 352);
        assert_eq!(ys, [244, 236, 228, 224, 222], "8 px, then the settle");
        assert_eq!(f.top - 224, 40);
    }

    #[test]
    fn climb_follows_up_at_once_above_climb_top() {
        let mut c = Camera { x: 0, y: 252 };
        let mut f = focus(300, false, 400); // top 278: screen y 26 < 40
        c.follow_climb(&mut f, W, TALL, &T);
        assert_eq!(c.y, 278 - 40);
        let mut f = focus(30, false, 400);
        c.follow_climb(&mut f, W, TALL, &T);
        assert_eq!(c.y, 0, "clamped at the top");
    }

    #[test]
    fn climb_follows_a_fall_at_most_80_px_below_the_last_ground() {
        let mut c = Camera { x: 0, y: 252 };
        let mut f = focus(400 + 50, false, 400);
        c.follow_climb(&mut f, W, TALL, &T);
        assert_eq!(c.y, 302, "feet back at 148");
        let mut f = focus(400 + 200, false, 400);
        c.follow_climb(&mut f, W, TALL, &T);
        assert_eq!(c.y, 252 + 80, "limited");
        // The limit never pulls the camera up.
        let mut c = Camera { x: 0, y: 340 };
        c.follow_climb(&mut f, W, TALL, &T);
        assert_eq!(c.y, 340);
        // Clamped at the level bottom.
        let mut c = Camera { x: 0, y: 480 };
        let mut f = focus(700, false, 640);
        c.follow_climb(&mut f, W, TALL, &T);
        assert_eq!(c.y, TALL - SCREEN_H);
    }

    #[test]
    fn climb_keeps_the_horizontal_follow() {
        let mut c = Camera { x: 0, y: 252 };
        let mut f = ClimbFocus {
            cx: 200,
            ..focus(400, true, 400)
        };
        c.follow_climb(&mut f, W, TALL, &T);
        assert_eq!(c.x, 56);
        f.cx = 150;
        c.follow_climb(&mut f, W, TALL, &T);
        assert_eq!(c.x, 56, "never scrolls left");
    }

    #[test]
    fn settle_y_is_clamped_and_keeps_x() {
        let mut c = Camera { x: 77, y: 100 };
        c.settle_y(-50, TALL, &T);
        assert_eq!(c, Camera { x: 77, y: 98 });
        let mut c = Camera { x: 77, y: 1 };
        c.settle_y(-50, TALL, &T);
        assert_eq!(c.y, 0);
        let mut c = Camera {
            x: 77,
            y: TALL - SCREEN_H - 1,
        };
        c.settle_y(10_000, TALL, &T);
        assert_eq!(c.y, TALL - SCREEN_H);
    }

    #[test]
    fn settle_y_towards_is_monotonic_and_keeps_up_with_the_floor() {
        // Down towards 300: 2 px a frame, or as far as the floor demands,
        // never past the target.
        let mut c = Camera { x: 5, y: 280 };
        c.settle_y_towards(300, 270, TALL, &T);
        assert_eq!(c, Camera { x: 5, y: 282 });
        c.settle_y_towards(300, 290, TALL, &T);
        assert_eq!(c.y, 290, "the floor pulls it further");
        c.settle_y_towards(300, 300, TALL, &T);
        assert_eq!(c.y, 300, "at most to the target");
        c.settle_y_towards(300, 250, TALL, &T);
        assert_eq!(c.y, 300, "a lower floor (the hop) never pulls it up");
        // Up towards 300: only the settle.
        let mut c = Camera { x: 5, y: 310 };
        c.settle_y_towards(300, 290, TALL, &T);
        assert_eq!(c.y, 308);
        // Clamped to the level.
        let mut c = Camera { x: 5, y: 0 };
        c.settle_y_towards(-40, -60, TALL, &T);
        assert_eq!(c.y, 0);
    }

    #[test]
    fn autoscroll_starts_at_exactly_from_and_moves_192_subpx_per_frame() {
        let mut c = Camera { x: 0, y: 44 };
        let mut s = Autoscroll::default();
        // Lead-in: the side camera.
        c.follow_autoscroll(&mut s, ZONE, (200, 150), Rest::None, W, H, &T);
        assert_eq!((c.x, s.phase), (56, ScrollPhase::LeadIn));
        c.follow_autoscroll(&mut s, ZONE, (150, 150), Rest::None, W, H, &T);
        assert_eq!(c.x, 56, "never left");
        // Following would put x at 131: set to exactly 128, no hold.
        c.follow_autoscroll(&mut s, ZONE, (275, 150), Rest::None, W, H, &T);
        assert_eq!(c.x, 128);
        assert_eq!(
            s,
            Autoscroll {
                phase: ScrollPhase::Scroll,
                pos: Fx::from_px(128),
                hold: 0
            }
        );
        // From then on 0.75 px per frame, whatever the player does; x is
        // the floor of the subpixel position.
        let mut xs = Vec::new();
        for focus_x in [0, 900, 275, 275] {
            c.follow_autoscroll(&mut s, ZONE, (focus_x, 150), Rest::None, W, H, &T);
            xs.push(c.x);
        }
        assert_eq!(xs, [128, 129, 130, 131]);
        assert_eq!(s.pos, Fx::from_px(131));
        assert_eq!(c.y, 44, "vertically the side camera");
        c.follow_autoscroll(&mut s, ZONE, (275, 100), Rest::None, W, H, &T);
        assert_eq!(c.y, 100 - T.deadzone_top);
    }

    #[test]
    fn autoscroll_hands_over_to_the_side_camera_at_to() {
        let mut c = Camera { x: 638, y: 0 };
        let mut s = Autoscroll {
            phase: ScrollPhase::Scroll,
            pos: Fx::from_px(638) + Fx(128),
            hold: 0,
        };
        c.follow_autoscroll(&mut s, ZONE, (0, 90), Rest::None, W, H, &T);
        assert_eq!((c.x, s.phase), (639, ScrollPhase::Scroll));
        c.follow_autoscroll(&mut s, ZONE, (0, 90), Rest::None, W, H, &T);
        assert_eq!((c.x, s.phase), (640, ScrollPhase::LeadOut));
        c.follow_autoscroll(&mut s, ZONE, (0, 90), Rest::None, W, H, &T);
        assert_eq!(c.x, 640, "the side camera: no scroll, never left");
        c.follow_autoscroll(&mut s, ZONE, (640 + 144 + 3, 90), Rest::None, W, H, &T);
        assert_eq!((c.x, s.phase), (643, ScrollPhase::LeadOut), "follows");
        // Far right of the follow line (the right clamp allows up to 99
        // px): no snap, it catches up at 4 px per frame (D-086).
        let mut xs = Vec::new();
        for _ in 0..4 {
            c.follow_autoscroll(&mut s, ZONE, (643 + 144 + 10, 90), Rest::None, W, H, &T);
            xs.push(c.x);
        }
        assert_eq!(xs, [647, 651, 653, 653]);
        // The lead-out never goes back to the scroll.
        c.x = 300;
        c.follow_autoscroll(&mut s, ZONE, (0, 90), Rest::None, W, H, &T);
        assert_eq!((c.x, s.phase), (300, ScrollPhase::LeadOut));
    }

    #[test]
    fn autoscroll_placed_inside_the_zone_holds_60_frames() {
        assert_eq!(Autoscroll::placed(127, ZONE, &T), Autoscroll::default());
        assert_eq!(
            Autoscroll::placed(640, ZONE, &T).phase,
            ScrollPhase::LeadOut
        );
        let mut s = Autoscroll::placed(300, ZONE, &T);
        assert_eq!(
            s,
            Autoscroll {
                phase: ScrollPhase::Scroll,
                pos: Fx::from_px(300),
                hold: 60
            }
        );
        let mut c = Camera { x: 300, y: 0 };
        for _ in 0..60 {
            c.follow_autoscroll(&mut s, ZONE, (0, 90), Rest::None, W, H, &T);
            assert_eq!(c.x, 300);
        }
        assert_eq!(s.hold, 0);
        c.follow_autoscroll(&mut s, ZONE, (0, 90), Rest::None, W, H, &T);
        assert_eq!(s.pos, Fx::from_px(300) + Fx(192), "moves on frame 61");
    }

    #[test]
    fn autoscroll_starts_at_its_rest_the_bottom_clamp() {
        // D-086: H = 224 px, so the rest is y 44; the side camera centres
        // the focus in the dead zone instead (120 - 92 = 28).
        let c = Camera::new_autoscroll((40, 120), W, H, &T);
        assert_eq!(c, Camera { x: 0, y: 44 });
        assert_eq!(Camera::new((40, 120), W, H, &T).y, 28, "side: dead zone");
        // A focus above the dead zone at the rest: the dead zone's top wins.
        let c = Camera::new_autoscroll((40, 100), W, H, &T);
        assert_eq!(c.y, 100 - T.deadzone_top);
        // A level no taller than the screen: y 0.
        assert_eq!(Camera::new_autoscroll((40, 60), W, 180, &T).y, 0);
    }

    #[test]
    fn autoscroll_settles_back_to_its_rest_at_2_px_per_frame_only_at_rest() {
        let mut s = Autoscroll::default();
        // Up at y 30 after a jump, focus inside the dead zone throughout.
        let mut c = Camera { x: 0, y: 30 };
        c.follow_autoscroll(&mut s, ZONE, (40, 140), Rest::None, W, H, &T);
        assert_eq!(c.y, 30, "in the air (no rest): the dead zone only");
        let mut ys = Vec::new();
        for _ in 0..9 {
            c.follow_autoscroll(&mut s, ZONE, (40, 140), Rest::Surface, W, H, &T);
            ys.push(c.y);
        }
        assert_eq!(
            ys,
            [32, 34, 36, 38, 40, 42, 44, 44, 44],
            "settles, stops at 44"
        );
        // In every phase: the scroll and the lead-out settle too.
        for phase in [ScrollPhase::Scroll, ScrollPhase::LeadOut] {
            let mut s = Autoscroll {
                phase,
                pos: Fx::from_px(300),
                hold: 0,
            };
            let mut c = Camera { x: 300, y: 30 };
            c.follow_autoscroll(&mut s, ZONE, (340, 140), Rest::Surface, W, H, &T);
            assert_eq!(c.y, 32, "{phase:?}");
        }
    }

    /// Stage 6's height: 17 rows, bottom clamp 92.
    const LAKE_H: i32 = 272;

    /// The camera y after `frames` frames of `rest` with the focus at
    /// `fy`, from `y`, in each phase (they must agree).
    fn settled(rest: Rest, fy: i32, y: i32, frames: usize) -> i32 {
        let mut ys = Vec::new();
        for phase in [
            ScrollPhase::LeadIn,
            ScrollPhase::Scroll,
            ScrollPhase::LeadOut,
        ] {
            let mut s = Autoscroll {
                phase,
                pos: Fx::from_px(300),
                hold: 0,
            };
            let mut c = Camera { x: 300, y };
            for _ in 0..frames {
                c.follow_autoscroll(&mut s, ZONE, (340, fy), rest, W, LAKE_H, &T);
            }
            ys.push(c.y);
        }
        assert!(ys.iter().all(|&y| y == ys[0]), "{rest:?}: {ys:?}");
        ys[0]
    }

    #[test]
    fn autoscroll_high_ground_rest_frames_the_ground_at_screen_y_84() {
        // D-090: standing, the rest is clamp(ground top - 84, 24, 92); the
        // focus is her centre, 8 (small) to 15 (tallest form) px above the
        // feet, always inside the dead zone at the rest.
        for (top, rest) in [
            (128, 44), // the Zaunerstollen perch (row 8)
            (160, 76), // a bollard on a deck (row 10)
            (112, 28), // a box top (row 7)
            (176, 92), // a deck or a boat (row 11)
            (256, 92), // the lake bed (row 16): the bottom clamp
            (80, 24),  // a box top on row 5: the top clamp
        ] {
            for half in [8, 12, 15] {
                assert_eq!(
                    settled(Rest::Ground(top), top - half, 92, 120),
                    rest,
                    "ground top {top}, centre {half} px above the feet"
                );
            }
        }
        // Floating at the surface (water line 192): the bottom clamp, from
        // anywhere above.
        assert_eq!(settled(Rest::Surface, 186, 40, 120), 92);
        // Back down from the perch rest to a deck at 2 px per frame.
        assert_eq!(settled(Rest::Ground(176), 161, 44, 10), 64);
    }

    #[test]
    fn autoscroll_dead_zone_still_applies_after_the_high_ground_rest() {
        // Landing on the perch (top 128, centre 113) with the camera at the
        // deck rest 92: the centre is at screen y 21, above the dead zone,
        // which moves the camera to 49 at once; the rest then settles on to
        // 44 at 2 px per frame and stays there, no creep, no jitter.
        let mut s = Autoscroll::default();
        let mut c = Camera { x: 0, y: 92 };
        let mut ys = Vec::new();
        for _ in 0..8 {
            c.follow_autoscroll(&mut s, ZONE, (40, 113), Rest::Ground(128), W, LAKE_H, &T);
            ys.push(c.y);
        }
        assert_eq!(ys, [49, 47, 45, 44, 44, 44, 44, 44]);
        // Standing on a row-5 box top (80): the rest is clamped at 24 and
        // the dead zone (which wants y 1) loses to the top clamp; she may
        // stand close to the screen's top, as in SMB3.
        let mut c = Camera { x: 0, y: 24 };
        c.follow_autoscroll(&mut s, ZONE, (40, 65), Rest::Ground(80), W, LAKE_H, &T);
        assert_eq!(c.y, 24);
        // Below the dead zone (a dive), the camera follows down as before.
        let mut c = Camera { x: 0, y: 24 };
        c.follow_autoscroll(&mut s, ZONE, (40, 160), Rest::None, W, LAKE_H, &T);
        assert_eq!(c.y, 160 - T.deadzone_bottom);
    }

    #[test]
    fn autoscroll_top_clamp_holds_in_every_phase() {
        // D-090: a jump far above the decks pulls the side camera's dead
        // zone up to y -104; the auto-scroll camera stops at 24 in the
        // lead-in, the scroll (with and without the hold) and the lead-out,
        // and its start/respawn framing too.
        for (phase, hold) in [
            (ScrollPhase::LeadIn, 0),
            (ScrollPhase::Scroll, 0),
            (ScrollPhase::Scroll, 60),
            (ScrollPhase::LeadOut, 0),
        ] {
            let mut s = Autoscroll {
                phase,
                pos: Fx::from_px(300),
                hold,
            };
            let mut c = Camera { x: 300, y: 92 };
            for fy in [100, 60, 20, -40, 20, 60] {
                c.follow_autoscroll(&mut s, ZONE, (340, fy), Rest::None, W, LAKE_H, &T);
                assert_eq!(c.y, (fy - T.deadzone_top).max(24), "{phase:?} focus {fy}");
            }
        }
        assert_eq!(Camera::new_autoscroll((40, 20), W, LAKE_H, &T).y, 24);
        let mut c = Camera { x: 0, y: 3 };
        c.clamp_autoscroll_top(LAKE_H, &T);
        assert_eq!(c.y, 24);
        // A level whose bottom clamp is above 24 is clamped there instead.
        let mut c = Camera { x: 0, y: 0 };
        c.clamp_autoscroll_top(190, &T);
        assert_eq!(c.y, 10);
        // The side camera is not clamped.
        let mut c = Camera { x: 0, y: 92 };
        c.follow((40, 20), W, LAKE_H, &T);
        assert_eq!(c.y, 0);
    }

    #[test]
    fn fully_on_screen_needs_the_whole_box_inside() {
        let c = Camera { x: 100, y: 50 };
        let b = |x: i32, y: i32, w: i32, h: i32| Aabb {
            x: Fx::from_px(x),
            y: Fx::from_px(y),
            w: Fx::from_px(w),
            h: Fx::from_px(h),
        };
        assert!(
            c.fully_on_screen(&b(100, 50, 320, 180)),
            "touching the edges"
        );
        assert!(!c.fully_on_screen(&b(99, 60, 16, 16)), "1 px left of it");
        assert!(
            !c.fully_on_screen(&b(405, 60, 16, 16)),
            "1 px past the right"
        );
        assert!(!c.fully_on_screen(&b(200, 49, 16, 16)), "above");
        assert!(!c.fully_on_screen(&b(200, 215, 16, 16)), "below");
        let mut sub = b(100, 60, 16, 16);
        sub.x -= Fx(1);
        assert!(!c.fully_on_screen(&sub), "one subpixel out");
    }

    #[test]
    fn row_near_is_64_px_around_the_screen() {
        let c = Camera { x: 0, y: 160 };
        // Screen rows 160..340: row 6 ends at 112 = 160 - 48 (near),
        // row 5 ends at 96 = 160 - 64 (touching the margin), row 4 at 80.
        assert!(c.row_near(6, 64));
        assert!(c.row_near(5, 64));
        assert!(!c.row_near(4, 64));
        // Below: row 25 starts at 400 = 340 + 60, row 26 at 416.
        assert!(c.row_near(25, 64));
        assert!(!c.row_near(26, 64));
        assert!(c.row_near(15, 0), "on screen");
        assert!(!c.row_near(26, 0));
    }
}
