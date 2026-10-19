//! The camera: follows the player like SMB (never scrolls left, except to follow a rider carried left by a platform, D-051).
//!
//! Horizontally every level is the same. Vertically a level picks
//! (`[meta] camera`, D-054):
//! - `side` (default, Stages 1–3): [`Camera::follow`] keeps the player's
//!   centre inside the dead zone `camera.deadzone_top..=deadzone_bottom`;
//! - `climb`: [`Camera::follow_climb`], SMW style. Grounded, the camera
//!   settles towards the feet at screen y `climb_feet_y` by at most
//!   `climb_settle_speed` px per frame; in the air it follows up at once
//!   when the hitbox top rises above screen y `climb_top`, and down while
//!   the feet are below `climb_feet_y`, but never more than
//!   `climb_fall_follow` px below the settle framing of the last ground
//!   ([`ClimbFocus::ground_feet`]). `Game` kills the player when her hitbox
//!   top is below the screen (the death line), and level entities activate
//!   only near the screen vertically ([`Camera::row_near`]).

use crate::hash::Fnv;
use crate::level::TILE_PX;
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
    /// [`Camera::follow_climb`] while grounded).
    pub ground_feet: i32,
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

    /// The climb camera's start (and respawn) framing: horizontally like
    /// [`Camera::new`], vertically at the settle target (feet at screen y
    /// `climb_feet_y`) at once; clamped to the level.
    pub fn new_climb(cx: i32, feet: i32, level_w: i32, level_h: i32, t: &CameraTuning) -> Camera {
        Camera {
            x: (cx - t.follow_x).clamp(0, (level_w - SCREEN_W).max(0)),
            y: clamp_y(feet - t.climb_feet_y, level_h),
        }
    }

    /// One frame of the climb camera (module docs, D-054). Horizontally
    /// like [`Camera::follow`]. Grounded: remembers the feet in
    /// `f.ground_feet` and settles towards them. In the air: up at once
    /// when the top is above `climb_top`, down with the feet below
    /// `climb_feet_y` but at most `climb_fall_follow` px below the last
    /// ground's settle target (never up because of that limit). Clamped
    /// to the level.
    pub fn follow_climb(
        &mut self,
        f: &mut ClimbFocus,
        level_w: i32,
        level_h: i32,
        t: &CameraTuning,
    ) {
        self.follow_x(f.cx, level_w, t);
        if f.grounded {
            f.ground_feet = f.feet;
            self.settle_y(f.feet - t.climb_feet_y, level_h, t);
            return;
        }
        if f.top - self.y < t.climb_top {
            self.y = f.top - t.climb_top;
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
        let target = clamp_y(target, level_h);
        let step = t.climb_settle_speed;
        self.y = clamp_y(self.y + (target - self.y).clamp(-step, step), level_h);
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
        climb_top: 40,
        climb_fall_follow: 80,
        climb_goal_feet_y: 164,
        climb_activate_y: 64,
    };
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

    fn focus(feet: i32, grounded: bool, ground_feet: i32) -> ClimbFocus {
        ClimbFocus {
            cx: 100,
            top: feet - 22,
            feet,
            grounded,
            ground_feet,
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
        assert_eq!(f.ground_feet, 400, "remembers the ground");
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
    fn climb_holds_still_for_a_jump_that_stays_inside_the_band() {
        let mut c = Camera { x: 0, y: 252 };
        // A 3-tile jump: feet 48 px up, top at screen y 78 > 40.
        let mut f = focus(352, false, 400);
        c.follow_climb(&mut f, W, TALL, &T);
        assert_eq!(c.y, 252);
        assert_eq!(f.ground_feet, 400, "only grounded frames update it");
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
