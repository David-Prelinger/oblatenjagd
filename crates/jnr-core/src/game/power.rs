//! The M3 power-ups in play: picking up the Hackl, throwing it and what
//! the axes hit (D-056), and the Durchkriechstein crawl (D-057).
//!
//! - **Hackl item**: Normal or Zuckerschock → Hackl (the transformation
//!   freeze, feet kept), the „Wirf, Resi, wirf!" popup plus the score popup
//!   (`score.power_up`); as Hackl the points only.
//! - **Throw**: a new press of RUN while Hackl, in play (not frozen, not
//!   crawling; the goal sequence and dying are other modes) throws an axe
//!   if fewer than `hackl.max_alive` are alive, and shows the throw pose
//!   for `hackl.throw_pose_frames`. Holding RUN keeps running.
//! - **Axe hits** ([`Game::axe_hits`], after every entity moved): each
//!   flying axe hits the first entity in list order that it overlaps and
//!   that does not let it pass ([`crate::hackl::AxeHit`]). An axe off
//!   screen is removed.
//! - **Crawl**: DOWN at the mouth starts it ([`crate::crawl::starts`]); the
//!   hitbox becomes `crawl.hitbox` flush left of the mouth, then moves right
//!   by `crawl.speed` per frame (no tile collision, the tunnel is empty by
//!   lint) until its left edge reaches the tunnel's right end; then she
//!   stands up with her feet on the floor. Outcome: Normal → +1 life
//!   (`crawl.cap_points` at the cap), big → Normal with „Ballast weg!" (the
//!   freeze, no invulnerability). The stone is then used until the level
//!   resets.

use super::{Event, Game};
use crate::crawl::{self, stone};
use crate::entity::{EnemyKind, EntityKind, Popup, popup};
use crate::fx::Fx;
use crate::hackl::{self, AxeHit, axe};
use crate::physics::Aabb;
use crate::player::{Facing, Player, Power};
use crate::{SCREEN_H, SCREEN_W};

/// The „Wirf, Resi, wirf!" popup starts this many px above the score popup.
const HACKL_POPUP_ABOVE_PX: i32 = 10;

impl Game {
    /// The player touched the Hackl item at feet (`x`, `y`): Normal or
    /// Zuckerschock become Hackl; points in any state. Returns the popups.
    pub(super) fn pick_up_hackl(&mut self, x: Fx, y: Fx) -> Vec<crate::Entity> {
        let mut popups = Vec::new();
        if self.player.power != Power::Hackl {
            self.player.set_power(Power::Hackl, &self.tuning.player);
            self.events.push(Event::PowerUp);
            popups.push(popup(
                Popup::Hackl,
                x,
                y - Fx::from_px(HACKL_POPUP_ABOVE_PX),
            ));
        }
        let points = self.tuning.score.power_up;
        self.add_score(points);
        popups.push(popup(Popup::Score(points), x, y));
        popups
    }

    /// A new RUN press while Hackl throws an axe (at most
    /// `hackl.max_alive` alive).
    pub(super) fn throw_hackl(&mut self, input: crate::InputFrame) {
        let p = &self.player;
        let pressed = input.run() && !p.run_held;
        if !pressed || p.power != Power::Hackl || p.crawling() {
            return;
        }
        let alive = self
            .entities
            .iter()
            .filter(|e| e.kind == EntityKind::ThrownHackl && !e.removed)
            .count();
        if alive >= usize::from(self.tuning.hackl.max_alive) {
            return;
        }
        let axe = hackl::throw_axe(p, &self.map, &self.tuning);
        self.entities.push(axe);
        self.player.throw_pose = self.tuning.hackl.throw_pose_frames;
        self.events.push(Event::HacklThrown);
    }

    /// Every flying axe against the other entities (one target each), and
    /// axes that left the screen are removed.
    pub(super) fn axe_hits(&mut self) {
        let screen = Aabb {
            x: Fx::from_px(self.camera.x),
            y: Fx::from_px(self.camera.y),
            w: Fx::from_px(SCREEN_W),
            h: Fx::from_px(SCREEN_H),
        };
        for i in 0..self.entities.len() {
            let a = self.entities[i];
            if a.kind != EntityKind::ThrownHackl || a.removed {
                continue;
            }
            if !a.hitbox().overlaps(&screen) {
                self.entities[i].removed = true;
                continue;
            }
            if a.state != axe::FLY {
                continue;
            }
            let target = self.entities.iter().position(|e| {
                e.active
                    && !e.removed
                    && e.axe_hit() != AxeHit::Pass
                    && e.hitbox().overlaps(&a.hitbox())
            });
            if let Some(j) = target {
                self.axe_hit(i, j);
            }
        }
    }

    /// Axe `i` hits entity `j` (its reaction is not `Pass`).
    fn axe_hit(&mut self, i: usize, j: usize) {
        self.entities[i].removed = true;
        self.events.push(Event::AxeHit);
        let target = self.entities[j];
        let (x, _) = target.feet();
        let points = match (target.axe_hit(), target.kind) {
            (AxeHit::Defeat, EntityKind::Enemy(kind)) => {
                crate::enemies::defeat(&mut self.entities[j], kind, true, &self.tuning);
                base_points(kind, &self.tuning)
            }
            (AxeHit::Shatter, _) => {
                self.entities[j].removed = true;
                self.tuning.hackl.rock_points
            }
            // Bounce (the Fiaker): the axe is gone, nothing else.
            _ => return,
        };
        self.add_score(points);
        self.entities.push(popup(Popup::Score(points), x, target.y));
    }

    /// DOWN at the mouth of an unused Durchkriechstein starts the crawl:
    /// the crawl hitbox flush left of the mouth on the tunnel floor, the
    /// stone used. Returns whether it started.
    pub(super) fn start_crawl(&mut self, input: crate::InputFrame) -> bool {
        let Some(j) = self
            .entities
            .iter()
            .position(|e| crawl::starts(&self.player, e, input, &self.tuning))
        else {
            return false;
        };
        let s = &mut self.entities[j];
        s.state = stone::USED;
        let (mouth, end, floor) = (s.x, s.right(), s.y + s.h);
        let [w, h] = self.tuning.crawl.hitbox;
        let p = &mut self.player;
        p.w = Fx::from_px(w);
        p.h = Fx::from_px(h);
        p.x = mouth - p.w;
        p.y = floor - p.h;
        p.vx = Fx::ZERO;
        p.vy = Fx::ZERO;
        p.skidding = false;
        p.jump_rising = false;
        p.throw_pose = 0;
        p.crawl_to = Some(end);
        self.events.push(Event::CrawlStarted);
        true
    }

    /// One frame of the crawl: move right; at the end stand up and get the
    /// stone's outcome.
    pub(super) fn step_crawl(&mut self) {
        let Some(end) = self.player.crawl_to else {
            return;
        };
        let p = &mut self.player;
        p.x = (p.x + self.tuning.crawl.speed).min(end);
        p.on_ground = true;
        p.facing = Facing::Right;
        if p.x < end {
            return;
        }
        // Stand up in place, feet on the floor.
        let feet = p.y + p.h;
        p.w = Fx::from_px(self.tuning.player.hitbox_w);
        p.h = Fx::from_px(Player::height(p.power, &self.tuning.player));
        p.y = feet - p.h;
        p.crawl_to = None;
        let (x, top) = (p.x + p.w.div_int(2), p.y);
        let shown = match p.power {
            Power::Normal => {
                if self.add_life() {
                    Popup::ExtraLife
                } else {
                    let points = self.tuning.crawl.cap_points;
                    self.add_score(points);
                    Popup::Score(points)
                }
            }
            Power::Big | Power::Hackl => {
                // Shed to Normal: the freeze, no invulnerability frames.
                self.player.set_power(Power::Normal, &self.tuning.player);
                Popup::Ballast
            }
        };
        self.entities.push(popup(shown, x, top));
    }
}

/// An enemy's base value: the first step of its stomp chain (100, or 200
/// for the enemies that start a chain at 200, D-046).
fn base_points(kind: EnemyKind, t: &crate::Tuning) -> u32 {
    let chain = &t.score.stomp_chain;
    chain[kind.chain_start().min(chain.len() - 1)]
}
