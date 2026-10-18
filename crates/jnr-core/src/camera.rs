//! The camera: follows the player like SMB (never scrolls left).

use crate::hash::Fnv;
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

impl Camera {
    /// A camera looking at `focus` (level pixels, usually the player's
    /// centre): horizontally as if it had just followed it, vertically
    /// centred on the dead zone; clamped to the level (`level_w` × `level_h`
    /// pixels).
    pub fn new(focus: (i32, i32), level_w: i32, level_h: i32, t: &CameraTuning) -> Camera {
        let mid = (t.deadzone_top + t.deadzone_bottom) / 2;
        Camera {
            x: (focus.0 - t.follow_x).clamp(0, (level_w - SCREEN_W).max(0)),
            y: (focus.1 - mid).clamp(0, (level_h - SCREEN_H).max(0)),
        }
    }

    /// Follows `focus`: scrolls right once it passes `follow_x` on screen
    /// (never left), keeps it inside the vertical dead zone, and clamps to
    /// the level.
    pub fn follow(&mut self, focus: (i32, i32), level_w: i32, level_h: i32, t: &CameraTuning) {
        let (fx, fy) = focus;
        if fx - self.x > t.follow_x {
            self.x = fx - t.follow_x;
        }
        if fy - self.y < t.deadzone_top {
            self.y = fy - t.deadzone_top;
        } else if fy - self.y > t.deadzone_bottom {
            self.y = fy - t.deadzone_bottom;
        }
        self.x = self.x.clamp(0, (level_w - SCREEN_W).max(0));
        self.y = self.y.clamp(0, (level_h - SCREEN_H).max(0));
    }

    pub(crate) fn hash(&self, h: &mut Fnv) {
        h.i32(self.x);
        h.i32(self.y);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const T: CameraTuning = CameraTuning {
        follow_x: 144,
        deadzone_top: 64,
        deadzone_bottom: 120,
    };
    const W: i32 = 1000;
    const H: i32 = 224;

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
}
