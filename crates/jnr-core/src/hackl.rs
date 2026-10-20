//! The thrown Hackl (D-056): spawning an axe, its flight, and what it does
//! to the entity it hits ([`AxeHit`], the hook every entity kind answers).
//!
//! A new press of RUN while Hackl throws one axe (the game decides when:
//! `game/power.rs`). The arc is fixed and independent of Resi's speed
//! (`[hackl]` in `tuning.toml`): it moves, then gravity applies; it sticks
//! in solid tiles (harmless, removed after `hackl.stuck_frames`), passes
//! one-way tiles, and is removed in water or a hazard tile. Leaving the
//! screen removes it too (the game checks that, it knows the camera).
//!
//! **The axe-hit hook** (for every entity kind): the matches in
//! [`EnemyKind::axe_hit`] and [`Entity::axe_hit`] are exhaustive on
//! purpose, so a new kind must be classified there:
//! - a stompable enemy (`EntityKind::Enemy(kind)`) → [`AxeHit::Defeat`]:
//!   knocked off (tumbling, like a bumped block) for its base value
//!   `score.stomp_chain[kind.chain_start()]` (100 or 200) outside the stomp
//!   chain;
//! - an immune one (the Fiaker) → [`AxeHit::Bounce`] (the axe is gone,
//!   nothing else);
//! - swarms, projectiles, items, spawners → [`AxeHit::Pass`] (flies on);
//! - the falling Stein → [`AxeHit::Shatter`] (both gone, 50 points).
//!
//! `Game::axe_hits` removes the axe and applies every hit through
//! `Game::defeat_by_projectile` / `enemies::hit_by_projectile`, which act on
//! this classification (points, popup, the rock's debris). A new kind with
//! special effects goes into `enemies::hit_by_projectile`.

use crate::entity::{EnemyKind, Entity, EntityKind};
use crate::fx::Fx;
use crate::level::TileKind;
use crate::physics::{TILE, TileMap, YHit, tile_of};
use crate::player::{Facing, Player};
use crate::tuning::Tuning;

/// `Entity::state` of a thrown Hackl.
pub mod axe {
    /// Flying (spinning); hits targets.
    pub const FLY: u8 = 0;
    /// Stuck in a solid tile: harmless, removed after `hackl.stuck_frames`
    /// (`Entity::count` counts the frames).
    pub const STUCK: u8 = 1;
}

/// What a flying axe does to an entity it touches (one target per axe).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AxeHit {
    /// Not a target: the axe flies on.
    Pass,
    /// The target is defeated (tumbles off the screen) for its base points;
    /// the axe is gone.
    Defeat,
    /// The target is immune: the axe bounces off (removed), nothing else.
    Bounce,
    /// Both are destroyed for `hackl.rock_points` (the falling Stein).
    Shatter,
}

impl EnemyKind {
    /// What a thrown Hackl does to this enemy (D-056): every stompable
    /// enemy is defeated, the Fiaker is immune, the Bienenschwarm lets it
    /// pass. Exhaustive on purpose: classify every new kind here.
    pub const fn axe_hit(self) -> AxeHit {
        match self {
            EnemyKind::SelfieTourist
            | EnemyKind::Taube
            | EnemyKind::TaubeSitting
            | EnemyKind::MozartkugelVerkaeufer
            | EnemyKind::NordicWalker
            | EnemyKind::Blasmusiker
            | EnemyKind::EBiker
            | EnemyKind::Pilger
            | EnemyKind::Gams
            | EnemyKind::Operettensaenger
            | EnemyKind::Schneekugel
            | EnemyKind::Schwan
            | EnemyKind::Gelse => AxeHit::Defeat,
            EnemyKind::Fiaker | EnemyKind::Reisebus | EnemyKind::Regattaboot => AxeHit::Bounce,
            EnemyKind::Bienenschwarm => AxeHit::Pass,
        }
    }
}

impl Entity {
    /// What a thrown Hackl does to this entity: enemies per
    /// [`EnemyKind::axe_hit`] (a harmless one, e.g. a crashed E-Biker, lets
    /// it pass); projectiles, items, blocks and everything else let it
    /// pass (D-056). Exhaustive on purpose: classify every new kind here.
    pub fn axe_hit(&self) -> AxeHit {
        match self.kind {
            EntityKind::Enemy(_) if !self.harmful() => AxeHit::Pass,
            EntityKind::Enemy(kind) => kind.axe_hit(),
            EntityKind::Thrown
            | EntityKind::Note
            | EntityKind::Oblate
            | EntityKind::PoppedOblate
            | EntityKind::Mozartkugel
            | EntityKind::Zaunerstollen
            | EntityKind::Hackl
            | EntityKind::ThrownHackl
            | EntityKind::Debris
            | EntityKind::Popup(_)
            | EntityKind::Bump
            | EntityKind::Checkpoint
            | EntityKind::Sign
            | EntityKind::Defeated(_)
            | EntityKind::Festungsbahn
            | EntityKind::Gondel
            | EntityKind::Schafbergbahn
            | EntityKind::Traundl
            | EntityKind::Dampfer
            | EntityKind::Durchkriechstein
            | EntityKind::Steinschlag
            | EntityKind::SteinDebris
            | EntityKind::Souvenirstand
            | EntityKind::SchneekugelScherben => AxeHit::Pass,
            EntityKind::Stein => AxeHit::Shatter,
        }
    }
}

/// A new axe thrown by `p`: its centre `hackl.throw_ahead` px in front of
/// her centre (facing side) and `hackl.throw_below_top` px below her
/// hitbox top, flying at (±`throw_speed_x`, −`throw_speed_y`). Spawned
/// inside a solid tile (thrown against a wall) it is stuck at once.
pub(crate) fn throw_axe(p: &Player, map: &TileMap, t: &Tuning) -> Entity {
    let h = &t.hackl;
    let dir = match p.facing {
        Facing::Left => -1,
        Facing::Right => 1,
    };
    let cx = p.x + p.w.div_int(2) + Fx::from_px(h.throw_ahead * dir);
    let cy = p.y + Fx::from_px(h.throw_below_top);
    let [w, ht] = h.axe_hitbox;
    let mut e = Entity::at_feet(
        EntityKind::ThrownHackl,
        cx,
        cy + Fx::from_px(ht).div_int(2),
        [w, ht],
    );
    e.vx = h.throw_speed_x.mul_int(dir);
    e.vy = -h.throw_speed_y;
    e.dir = dir as i8;
    if map.touches(&e.hitbox(), TileKind::Solid) {
        // Flush against the wall face on the throwing side (the `stuck`
        // art has the face just beyond the hitbox), not inside it.
        let (c0, c1) = (tile_of(e.x), tile_of(e.right() - Fx(1)));
        let (r0, r1) = (tile_of(e.y), tile_of(e.y + e.h - Fx(1)));
        let solid = |c: i32| (r0..=r1).any(|r| map.kind_at(c, r) == TileKind::Solid);
        if dir > 0 {
            if let Some(c) = (c0..=c1).find(|&c| solid(c)) {
                e.x = TILE.mul_int(c) - e.w;
            }
        } else if let Some(c) = (c0..=c1).rev().find(|&c| solid(c)) {
            e.x = TILE.mul_int(c + 1);
        }
        stick(&mut e);
    }
    e
}

/// One frame of a thrown axe: stuck ones count down; flying ones move
/// (X, then Y; one-way tiles don't stop them), then gravity applies. A
/// solid tile makes it stick, water or a hazard removes it.
pub(crate) fn update_axe(e: &mut Entity, map: &TileMap, t: &Tuning) {
    let h = &t.hackl;
    if e.state == axe::STUCK {
        e.count = e.count.saturating_add(1);
        if e.count >= h.stuck_frames {
            e.removed = true;
        }
        return;
    }
    let mut b = e.hitbox();
    let hit_x = map.move_x(&mut b, e.vx);
    let hit_y = map.move_y(&mut b, e.vy, true) != YHit::None;
    e.x = b.x;
    e.y = b.y;
    e.vy = (e.vy + h.gravity).min(h.max_fall_speed);
    if map.touches_lethal(&b) {
        e.removed = true;
    } else if hit_x || hit_y {
        stick(e);
    }
}

fn stick(e: &mut Entity) {
    e.state = axe::STUCK;
    e.vx = Fx::ZERO;
    e.vy = Fx::ZERO;
    e.count = 0;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn axe_hits_per_kind() {
        use EnemyKind::*;
        for k in [
            SelfieTourist,
            Taube,
            TaubeSitting,
            MozartkugelVerkaeufer,
            NordicWalker,
            Blasmusiker,
            EBiker,
        ] {
            assert!(k.stompable(), "{k:?}");
            assert_eq!(k.axe_hit(), AxeHit::Defeat, "{k:?}");
        }
        assert_eq!(Fiaker.axe_hit(), AxeHit::Bounce);
        assert_eq!(Bienenschwarm.axe_hit(), AxeHit::Pass);
        let e = |kind| Entity::new(kind, Fx::ZERO, Fx::ZERO, Fx::ONE, Fx::ONE);
        for kind in [
            EntityKind::Thrown,
            EntityKind::Note,
            EntityKind::Mozartkugel,
            EntityKind::Hackl,
            EntityKind::Oblate,
            EntityKind::Defeated(SelfieTourist),
            EntityKind::Gondel,
            EntityKind::Schafbergbahn,
            EntityKind::Traundl,
            EntityKind::Dampfer,
        ] {
            assert_eq!(e(kind).axe_hit(), AxeHit::Pass, "{kind:?}");
        }
        assert_eq!(e(EntityKind::Enemy(Fiaker)).axe_hit(), AxeHit::Bounce);
        assert_eq!(e(EntityKind::Stein).axe_hit(), AxeHit::Shatter);
        assert_eq!(e(EntityKind::Steinschlag).axe_hit(), AxeHit::Pass);
        assert_eq!(Pilger.axe_hit(), AxeHit::Defeat);
        assert_eq!(Gams.axe_hit(), AxeHit::Defeat);
        let mut crashed = e(EntityKind::Enemy(EBiker));
        assert_eq!(crashed.axe_hit(), AxeHit::Defeat);
        crashed.state = crate::enemies::state::CRASH;
        assert_eq!(crashed.axe_hit(), AxeHit::Pass, "harmless");
    }
}
