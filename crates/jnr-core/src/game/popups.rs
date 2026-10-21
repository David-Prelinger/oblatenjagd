//! Popups that keep their place on screen (E53, D-106/D-107 "As built
//! (E53)"): the Oberkellner's hit labels never cover Resi, and they and
//! the `goal_stollen` pair never rise into the HUD.
//!
//! A popup is tagged at spawn by its `state` ([`PopupPlace`]); plain
//! popups (state 0, every popup of Stages 1–8) are never touched here.
//! All popups with the same tag move as one group (a label over its
//! score), by whole pixels of their `x`/`y`; the renderer draws them at
//! `y` minus their rise as before.
//!
//! - **Clear of the HUD** (both tags): if the group's highest drawn row
//!   (its text's outline, `y − 1 − rise`) is above screen row
//!   [`HUD_CLEAR_Y`], the group moves down by the difference: its rise
//!   stops under the HUD (a Big or Hackl Resi holds the plate 8 px
//!   higher than a Normal one).
//! - **Clear of Resi** ([`PopupPlace::ClearOfResi`], the boss's labels):
//!   the group's box (centre ± [`LABEL_HALF_W_PX`], the drawn rows) is
//!   kept out of her drawn look (16 px wide around her feet, her hitbox
//!   plus 2 px on top) widened by [`DODGE_MARGIN_X_PX`] / `_Y_PX`. While
//!   that zone overlaps it, the group slides at most [`DODGE_SPEED_PX`]
//!   per frame the shortest way out of it that goes away from her: towards
//!   the room (its `dir`) while she is on its wall side, back towards the
//!   wall while she is on its room side (only as far as where it spawned,
//!   so never over his face), or up while she is below its middle (only
//!   while its top stays below the HUD). The slide is faster than she runs, so she shoos it ahead of
//!   her. If her look itself still touches it (she came at it faster than
//!   it slides), it jumps clear at once, the shortest of the three ways,
//!   past her towards the room at the latest.

use super::{Game, Mode};
use crate::entity::EntityKind;
use crate::fx::Fx;

/// The first screen row below the HUD (its rows 0–21, D-029): no popup
/// that keeps its place is drawn above it.
pub const HUD_CLEAR_Y: i32 = 22;

/// Half the drawn width of the boss's widest label, outline included
/// („Bitte sehr!": 45 px + 2): its columns are `centre − 24 ..
/// centre + 24` (exclusive). `jnr-render` checks every label with the
/// real font.
pub const LABEL_HALF_W_PX: i32 = 24;

/// A popup's drawn rows, outline included, from `y − 1` (its top
/// outline) for this many rows (`jnr-render` checks it with the font).
pub const POPUP_DRAWN_H_PX: i32 = 10;

/// The zone the boss's labels keep clear of: her look widened by this
/// many px left and right ...
pub const DODGE_MARGIN_X_PX: i32 = 4;

/// ... and above and below, so they see her coming as she falls back
/// (up to 4 px per frame) or jumps.
pub const DODGE_MARGIN_Y_PX: i32 = 24;

/// How far the labels slide per frame while she is in the zone: faster
/// than her run (2.5 px per frame), so they keep ahead of her.
pub const DODGE_SPEED_PX: i32 = 3;

/// How a popup keeps its place (its `state`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum PopupPlace {
    /// Clear of the HUD (the `goal_stollen` pair).
    ClearOfHud = 1,
    /// Clear of the HUD and of Resi (the Oberkellner's hit labels).
    ClearOfResi = 2,
}

/// Where a group of popups is and may go.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Group {
    /// Its drawn box, with the rise.
    b: PxBox,
    /// The room's side: 1 right, -1 left.
    room: i32,
    /// How far it may still go towards the wall: back to where it
    /// spawned (beside his head), never over his face.
    back: i32,
}

/// The shortest move that takes group `g` clear of `z` (whole px, `dy`
/// up is negative), if any: up (only while its top stays at or below the
/// level row `hud`), towards the room, or back towards the wall (at most
/// `g.back`). With `slide`, only moves that go away from `her` (her
/// centre) count: towards the room while she is on its wall side, back
/// while she is on its room side, up while she is below its middle; a
/// move past her is a jump.
fn clearance(g: &Group, z: &PxBox, hud: i32, her: (i32, i32), slide: bool) -> Option<(i32, i32)> {
    let (her_cx, her_cy) = her;
    let b = &g.b;
    // Room coordinates: x grows towards the room.
    let (bl, br) = if g.room > 0 {
        (b.left, b.right)
    } else {
        (-b.right, -b.left)
    };
    let (zl, zr) = if g.room > 0 {
        (z.left, z.right)
    } else {
        (-z.right, -z.left)
    };
    let her_room_side = her_cx * g.room >= (bl + br) / 2;
    let up = b.bottom - z.top;
    let to_room = zr - bl;
    let back = br - zl;
    let mut best: Option<(i32, (i32, i32))> = None;
    let mut consider = |cost: i32, ok: bool, mv: (i32, i32)| {
        if ok && best.is_none_or(|(c, _)| cost < c) {
            best = Some((cost, mv));
        }
    };
    let her_below = her_cy >= (b.top + b.bottom) / 2;
    consider(up, b.top - up >= hud && (!slide || her_below), (0, -up));
    consider(to_room, !slide || !her_room_side, (g.room * to_room, 0));
    consider(
        back,
        back <= g.back && (!slide || her_room_side),
        (-g.room * back, 0),
    );
    best.map(|(_, mv)| mv)
}

/// A box in whole level px, exclusive right and bottom.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PxBox {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

impl PxBox {
    fn overlaps(&self, o: &PxBox) -> bool {
        self.left < o.right && o.left < self.right && self.top < o.bottom && o.top < self.bottom
    }

    fn grown(&self, x: i32, y: i32) -> PxBox {
        PxBox {
            left: self.left - x,
            top: self.top - y,
            right: self.right + x,
            bottom: self.bottom + y,
        }
    }
}

impl Game {
    /// Resi's drawn look in level px: 16 px wide around her feet, 24 px
    /// tall (32 big, also while a transformation from or to big flickers
    /// between the two), the feet row last (as `jnr-render` draws her).
    fn resi_look_px(&self) -> PxBox {
        let p = &self.player;
        let (fx, fy) = p.feet();
        let (fx, fy) = (fx.to_px(), fy.to_px());
        let big = p.power.is_big() || (p.transform > 0 && p.prev_power.is_big());
        let h = if big { 32 } else { 24 };
        PxBox {
            left: fx - 8,
            top: fy - h,
            right: fx + 8,
            bottom: fy,
        }
    }

    /// The popups tagged `place` as a group (level px, with their rise);
    /// None if there are none.
    fn popup_group(&self, place: PopupPlace) -> Option<Group> {
        let mut found: Option<Group> = None;
        for e in self.popups(place) {
            let half = e.w.div_int(2);
            let cx = (e.x + half).to_px();
            let room = if e.dir < 0 { -1 } else { 1 };
            let back = (cx - (e.base_x + half).to_px()) * room;
            let top = e.y.to_px() - e.popup_rise_px(&self.tuning) - 1;
            let b = PxBox {
                left: cx - LABEL_HALF_W_PX,
                top,
                right: cx + LABEL_HALF_W_PX,
                bottom: top + POPUP_DRAWN_H_PX,
            };
            found = Some(match found {
                None => Group { b, room, back },
                Some(g) => Group {
                    b: PxBox {
                        left: g.b.left.min(b.left),
                        top: g.b.top.min(b.top),
                        right: g.b.right.max(b.right),
                        bottom: g.b.bottom.max(b.bottom),
                    },
                    back: g.back.min(back),
                    ..g
                },
            });
        }
        found
    }

    fn popups(&self, place: PopupPlace) -> impl Iterator<Item = &crate::entity::Entity> {
        self.entities.iter().filter(move |e| {
            matches!(e.kind, EntityKind::Popup(_)) && !e.removed && e.state == place as u8
        })
    }

    /// Moves every popup tagged `place` by (`dx`, `dy`) whole px.
    fn move_popups(&mut self, place: PopupPlace, dx: i32, dy: i32) {
        for e in &mut self.entities {
            if matches!(e.kind, EntityKind::Popup(_)) && !e.removed && e.state == place as u8 {
                e.x += Fx::from_px(dx);
                e.y += Fx::from_px(dy);
            }
        }
    }

    /// Keeps the tagged popups in their place (see the module docs); in
    /// play and in the goal sequence, after everything else of the frame
    /// moved (so what is drawn is what was checked).
    pub(super) fn place_popups(&mut self) {
        if !matches!(self.mode, Mode::Playing | Mode::StageClear) {
            return;
        }
        let hud = self.camera.y + HUD_CLEAR_Y;
        for place in [PopupPlace::ClearOfHud, PopupPlace::ClearOfResi] {
            if let Some(g) = self.popup_group(place)
                && g.b.top < hud
            {
                self.move_popups(place, 0, hud - g.b.top);
            }
        }
        let Some(g) = self.popup_group(PopupPlace::ClearOfResi) else {
            return;
        };
        let look = self.resi_look_px();
        let her = ((look.left + look.right) / 2, (look.top + look.bottom) / 2);
        let zone = look.grown(DODGE_MARGIN_X_PX, DODGE_MARGIN_Y_PX);
        if g.b.overlaps(&zone)
            && let Some((dx, dy)) = clearance(&g, &zone, hud, her, true)
        {
            let max = DODGE_SPEED_PX;
            self.move_popups(PopupPlace::ClearOfResi, dx.clamp(-max, max), dy.max(-max));
        }
        // Her look itself is never covered: if it still is, jump clear
        // (towards the room at the latest, which is always possible).
        if let Some(g) = self.popup_group(PopupPlace::ClearOfResi)
            && g.b.overlaps(&look)
        {
            let (dx, dy) = clearance(&g, &look, hud, her, false).unwrap_or((0, 0));
            self.move_popups(PopupPlace::ClearOfResi, dx, dy);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pxbox(left: i32, top: i32, right: i32, bottom: i32) -> PxBox {
        PxBox {
            left,
            top,
            right,
            bottom,
        }
    }

    /// A label pair at the left wall (the room to the right), 48 × 21,
    /// where it spawned (no way back).
    fn left_wall_pair() -> Group {
        Group {
            b: pxbox(100, 50, 148, 71),
            room: 1,
            back: 0,
        }
    }

    #[test]
    fn she_shoos_the_labels_towards_the_room_from_the_wall_side() {
        let g = left_wall_pair();
        // Her zone reaches 6 px into the pair from the wall side.
        let z = pxbox(80, 30, 106, 80);
        assert_eq!(clearance(&g, &z, 0, (90, 55), true), Some((6, 0)));
        // Mirrored at the right wall.
        let g = Group {
            b: pxbox(-148, 50, -100, 71),
            room: -1,
            back: 0,
        };
        let z = pxbox(-106, 30, -80, 80);
        assert_eq!(clearance(&g, &z, 0, (-90, 55), true), Some((-6, 0)));
    }

    #[test]
    fn from_the_room_side_they_never_slide_into_her_or_over_his_face() {
        let g = left_wall_pair();
        // She is on the room side, below them (rising): no slide towards
        // the room (into her) and none back (they are where they
        // spawned), so up.
        let z = pxbox(140, 60, 160, 100);
        assert_eq!(clearance(&g, &z, -100, (150, 80), true), Some((0, -11)));
        // With the HUD in the way of up, nothing slides.
        assert_eq!(clearance(&g, &z, 45, (150, 80), true), None);
        // Above them (falling), up would go towards her: nothing slides ...
        let z = pxbox(140, 20, 160, 55);
        assert_eq!(clearance(&g, &z, -100, (150, 37), true), None);
        // ... unless they can go back, which is shorter.
        let back = Group { back: 10, ..g };
        assert_eq!(clearance(&back, &z, -100, (150, 37), true), Some((-8, 0)));
        // The last resort takes the shortest way: up over her head ...
        assert_eq!(clearance(&g, &z, -100, (150, 37), false), Some((0, -51)));
        // ... or, with the HUD in the way, past her towards the room.
        assert_eq!(clearance(&g, &z, 40, (150, 37), false), Some((60, 0)));
    }
}
