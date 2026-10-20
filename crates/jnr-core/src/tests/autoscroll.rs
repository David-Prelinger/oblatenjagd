//! The auto-scroll camera (D-075, M5 task E26): lead-in → scroll at
//! exactly `from × 16`, 192 subpx per frame with a whole-px camera, the
//! lead-out at `to × 16`, standing still while frozen, paused, dying or in
//! the goal sequence, the push with tile collision and the squeeze death,
//! the right clamp for walking and carrying, D-051 off in the scroll only,
//! and the 60-frame hold after a start or respawn framed inside the zone.
//! The camera's own arithmetic is unit-tested in `camera.rs`; side and
//! climb levels are covered by every earlier replay and golden, which
//! must not change.

use super::*;
use crate::camera::{Autoscroll, ScrollPhase};
use crate::{DeathCause, EntityKind, Fx, Mode, SCREEN_W, Start};

/// Legend of the test worlds (`.` air, `P` start).
const LEGEND: &str = r##"
"#" = "solid"
"-" = "oneway"
"^" = "hazard"
"C" = { entity = "checkpoint" }
"l" = { entity = "gondel", to = [-8, 0] }
"r" = { entity = "gondel", to = [8, 0] }
"P" = { entity = "player_start" }
"G" = { entity = "goal" }
"##;

/// A 14-row auto-scroll world `width` columns wide with the zone
/// `[from, to]`: ground in rows 12–13, the player start at column 2 (row
/// 11) unless `marks` places a `P`, plus `marks` as (row, col, char).
fn world(width: usize, zone: (i32, i32), marks: &[(usize, usize, char)]) -> Level {
    let mut rows: Vec<Vec<char>> = (0..14)
        .map(|r| vec![if r >= 12 { '#' } else { '.' }; width])
        .collect();
    if !marks.iter().any(|m| m.2 == 'P') {
        rows[11][2] = 'P';
    }
    for &(r, c, ch) in marks {
        rows[r][c] = ch;
    }
    let grid: Vec<String> = rows.into_iter().map(|r| r.into_iter().collect()).collect();
    let text = format!(
        "[meta]\nid = \"scroll\"\ntime_limit = 300\ncamera = \"autoscroll\"\nautoscroll = [{}, {}]\n[legend]\n{LEGEND}\n[map]\ngrid = '''\n{}\n'''\n",
        zone.0,
        zone.1,
        grid.join("\n")
    );
    Level::parse(&text).unwrap()
}

fn game(level: &Level) -> Game {
    Game::new(level, &tuning(), 1)
}

fn gondel(g: &Game) -> crate::Entity {
    *g.entities()
        .iter()
        .find(|e| e.kind == EntityKind::Gondel)
        .expect("a gondel")
}

/// Puts the camera at `x` inside the zone, scrolling without a hold.
fn scrolling_at(g: &mut Game, x: i32) {
    g.camera.x = x;
    g.scroll = Autoscroll {
        phase: ScrollPhase::Scroll,
        pos: Fx::from_px(x),
        hold: 0,
    };
}

/// Puts the player standing on the gondel's roof, `dx` px from its left.
fn board(g: &mut Game, dx: i32) {
    let c = gondel(g);
    g.player.x = c.x + Fx::from_px(dx);
    g.player.y = c.y - g.player.h;
    g.player.vx = Fx::ZERO;
    g.player.vy = Fx::ZERO;
    g.player.on_ground = true;
}

#[test]
fn the_scroll_starts_at_exactly_from_and_moves_192_subpx_per_frame() {
    let mut g = game(&world(80, (4, 40), &[]));
    assert_eq!(g.autoscroll(), Autoscroll::default(), "lead-in");
    until(&mut g, "R", 300, |g| g.camera().x > 0);
    let mut prev = g.camera().x;
    while g.autoscroll().phase == ScrollPhase::LeadIn {
        prev = g.camera().x;
        g.step(input("R"));
    }
    assert!(prev < 64, "the side camera until then");
    assert_eq!(g.camera().x, 64, "exactly from × 16");
    assert!(
        g.player().center_px().0 - 144 >= 64,
        "following would go past"
    );
    assert_eq!(
        g.autoscroll(),
        Autoscroll {
            phase: ScrollPhase::Scroll,
            pos: Fx::from_px(64),
            hold: 0
        },
        "no hold"
    );
    // Independent of the player: idle, walking or running, 0.75 px per
    // frame, the camera at the whole-px floor.
    for (k, token) in (1..=120).zip(["-", "R", "RB", "L"].iter().cycle()) {
        g.step(input(token));
        let pos = Fx::from_px(64) + Fx(192).mul_int(k);
        assert_eq!(g.autoscroll().pos, pos, "frame {k}");
        assert_eq!(g.camera().x, pos.to_px(), "frame {k}");
    }
    assert_eq!(g.camera().x, 64 + 90);
}

#[test]
fn the_side_camera_takes_over_at_to_and_never_scrolls_back() {
    let mut g = game(&world(60, (4, 10), &[]));
    until(&mut g, "R", 300, |g| {
        g.autoscroll().phase == ScrollPhase::Scroll
    });
    // 96 px at 0.75 px per frame: 128 frames.
    let events = hold(&mut g, "-", 127);
    assert!(!events.contains(&Event::Died));
    assert_eq!(
        (g.camera().x, g.autoscroll().phase),
        (159, ScrollPhase::Scroll)
    );
    g.step(input("-"));
    assert_eq!(g.camera().x, 160);
    assert_eq!(g.autoscroll().phase, ScrollPhase::LeadOut);
    hold(&mut g, "-", 60);
    assert_eq!(g.camera().x, 160, "idle: the side camera stands");
    hold(&mut g, "L", 60);
    assert_eq!(g.camera().x, 160, "never left");
    assert_eq!(
        g.player().x,
        Fx::from_px(160),
        "held at the edge, not pushed"
    );
    until(&mut g, "R", 300, |g| g.camera().x > 170);
    assert_eq!(g.autoscroll().phase, ScrollPhase::LeadOut, "never back");
    assert_eq!(g.camera().x, g.player().center_px().0 - 144, "following");
}

#[test]
fn the_left_edge_pushes_an_idle_player_along() {
    let mut g = game(&world(80, (4, 40), &[]));
    until(&mut g, "R", 300, |g| {
        g.autoscroll().phase == ScrollPhase::Scroll
    });
    let mut pushed = 0;
    for _ in 0..400 {
        g.step(input("-"));
        assert_eq!(g.mode(), Mode::Playing);
        let edge = Fx::from_px(g.camera().x);
        assert!(g.player().x >= edge, "never left of the screen");
        if g.player().x == edge {
            pushed += 1;
        }
        assert!(g.player().on_ground, "pushed along the ground");
    }
    assert!(pushed > 150, "pushed for {pushed} frames");
}

#[test]
fn a_solid_tile_in_the_way_of_the_push_squeezes_her() {
    // A 3-tile wall (rows 9–11) at column 14; she stands flush against it.
    let marks: Vec<(usize, usize, char)> = (9..12).map(|r| (r, 14, '#')).collect();
    let mut g = game(&world(80, (4, 40), &marks));
    until(&mut g, "R", 400, |g| {
        g.player().x + g.player().w == Fx::from_px(14 * 16)
    });
    assert_eq!(g.autoscroll().phase, ScrollPhase::Scroll);
    let mut events = Vec::new();
    for _ in 0..400 {
        events.extend_from_slice(g.step(input("R")));
        if g.mode() != Mode::Playing {
            break;
        }
    }
    assert!(events.contains(&Event::Died));
    assert_eq!(g.death_cause(), Some(DeathCause::Squeezed));
    assert!(matches!(g.mode(), Mode::Dying { .. }));
    assert_eq!(
        g.player().x + g.player().w,
        Fx::from_px(14 * 16),
        "the wall stopped the push"
    );
    assert!(g.player().x < Fx::from_px(g.camera().x));
    // Dying: the scroll stands still.
    let (camera, scroll) = (g.camera(), g.autoscroll());
    hold(&mut g, "-", 59);
    assert!(matches!(g.mode(), Mode::Dying { .. }));
    assert_eq!((g.camera(), g.autoscroll()), (camera, scroll));
}

#[test]
fn jumping_onto_the_wall_escapes_the_squeeze() {
    let marks: Vec<(usize, usize, char)> = (9..12).map(|r| (r, 14, '#')).collect();
    let mut g = game(&world(80, (4, 40), &marks));
    until(&mut g, "R", 400, |g| {
        g.player().x + g.player().w == Fx::from_px(14 * 16)
    });
    hold(&mut g, "RJ", 30);
    let events = hold(&mut g, "R", 300);
    assert!(!events.contains(&Event::Died), "{events:?}");
    assert!(g.player().x > Fx::from_px(14 * 16));
}

#[test]
fn the_right_clamp_stops_her_own_movement() {
    let mut g = game(&world(80, (4, 40), &[]));
    until(&mut g, "R", 300, |g| {
        g.autoscroll().phase == ScrollPhase::Scroll
    });
    let margin = tuning().camera.autoscroll_right_margin;
    let mut at_clamp = 0;
    for k in 0..300 {
        let clamp = Fx::from_px(g.camera().x + SCREEN_W - margin);
        g.step(input("RB"));
        let right = g.player().x + g.player().w;
        assert!(right <= clamp, "past the clamp");
        if right == clamp {
            at_clamp += 1;
            let speed = tuning().camera.autoscroll_speed;
            assert!(g.player().vx <= speed, "slowed to the edge's speed");
        }
        // Running 2.5 px per frame she reaches it within 100 frames, then
        // keeps pace with the screen less than 1 px behind it (no stutter).
        if k >= 100 {
            assert!(clamp - right < Fx::ONE, "frame {k}");
        }
    }
    assert!(at_clamp > 50, "{at_clamp}");
    // Jumping doesn't get past it either.
    for _ in 0..60 {
        let clamp = Fx::from_px(g.camera().x + SCREEN_W - margin);
        g.step(input("RBJ"));
        assert!(g.player().x + g.player().w <= clamp);
    }
}

#[test]
fn the_right_clamp_stops_a_carry_and_she_slides_on_the_deck() {
    // A cabin at column 20 going 8 tiles right at 1 px per frame, faster
    // than the scroll: a standing rider reaches the clamp and stays there
    // while the cabin moves on under her.
    let mut g = game(&world(80, (4, 40), &[(11, 20, 'r')]));
    g.step(input("-"));
    board(&mut g, 18);
    let margin = tuning().camera.autoscroll_right_margin;
    let right = (g.player().x + g.player().w).to_px();
    scrolling_at(&mut g, right - (SCREEN_W - margin) + 10);
    let mut slid = false;
    for _ in 0..158 {
        let clamp = Fx::from_px(g.camera().x + SCREEN_W - margin);
        let before = g.player().x - gondel(&g).x;
        g.step(input("-"));
        let (p, c) = (*g.player(), gondel(&g));
        assert!(p.x + p.w <= clamp, "past the clamp");
        assert_eq!(p.y + p.h, c.y, "still on the roof");
        slid |= p.x - c.x < before;
    }
    assert!(slid, "the cabin moved on under her");
    assert_eq!(g.mode(), Mode::Playing);
}

#[test]
fn in_the_scroll_a_rider_carried_left_is_pushed_across_the_deck() {
    // D-051 is off in the scroll: the camera keeps scrolling right and
    // the edge pushes her over the roof of a cabin going left until she
    // drops off its right end.
    let mut g = game(&world(80, (4, 40), &[(11, 20, 'l')]));
    g.step(input("-"));
    board(&mut g, 2);
    let start = (g.player().x - Fx::from_px(40)).to_px();
    scrolling_at(&mut g, start);
    let mut pushed = false;
    let mut dropped = false;
    for _ in 0..200 {
        let (cam, offset) = (g.camera().x, g.player().x - gondel(&g).x);
        g.step(input("-"));
        assert!(g.camera().x >= cam, "the camera never follows left");
        assert!(g.player().x >= Fx::from_px(g.camera().x), "on screen");
        let p = *g.player();
        if p.y + p.h == gondel(&g).y && p.on_ground {
            pushed |= p.x - gondel(&g).x > offset;
        } else {
            dropped = true;
            break;
        }
    }
    assert!(pushed, "slid over the roof");
    assert!(dropped, "and off its end");
}

#[test]
fn in_the_lead_in_the_camera_follows_a_rider_carried_left() {
    // D-051 as before outside the zone: the zone starts at column 30, the
    // camera at x 160 is in the lead-in.
    let mut g = game(&world(80, (30, 50), &[(11, 12, 'l')]));
    g.step(input("-"));
    board(&mut g, 20);
    g.camera.x = 160;
    let mut followed = false;
    for _ in 0..300 {
        let cam = g.camera().x;
        g.step(input("-"));
        let (p, c) = (*g.player(), gondel(&g));
        assert_eq!(p.y + p.h, c.y, "still on the roof");
        assert_eq!(p.x - c.x, Fx::from_px(20), "no sliding");
        followed |= g.camera().x < cam;
    }
    assert!(followed, "the camera scrolled left");
    assert_eq!(g.autoscroll().phase, ScrollPhase::LeadIn);
}

#[test]
fn a_respawn_inside_the_zone_holds_the_scroll_for_60_frames() {
    // Checkpoint at column 22 (respawn framing: camera x 360 - 144 = 216,
    // inside [64, 640)), spikes at column 27.
    let mut g = game(&world(80, (4, 40), &[(11, 22, 'C'), (11, 27, '^')]));
    let events = until(&mut g, "R", 600, |g| g.mode() != Mode::Playing);
    assert!(events.contains(&Event::CheckpointReached));
    let events = until(&mut g, "-", 120, |g| g.mode() == Mode::Playing);
    assert!(events.contains(&Event::Respawned));
    assert_eq!(g.camera().x, 216);
    assert_eq!(
        g.autoscroll(),
        Autoscroll {
            phase: ScrollPhase::Scroll,
            pos: Fx::from_px(216),
            hold: 60
        }
    );
    for k in 1..=60 {
        g.step(input("-"));
        assert_eq!(g.camera().x, 216, "frame {k} of the hold");
    }
    assert_eq!(g.autoscroll().hold, 0);
    g.step(input("-"));
    assert_eq!(
        g.autoscroll().pos,
        Fx::from_px(216) + Fx(192),
        "moves on frame 61"
    );
    // The hold counts frames of play only: a transformation freeze
    // doesn't use it up.
    g.scroll.hold = 5;
    g.player.transform = 30;
    hold(&mut g, "-", 30);
    assert_eq!(g.autoscroll().hold, 5);
}

#[test]
fn a_respawn_before_the_zone_and_the_lead_in_never_hold() {
    // Checkpoint at column 5, zone from column 20: the respawn frames the
    // lead-in, and walking into the zone starts the scroll at once.
    let mut g = game(&world(80, (20, 40), &[(11, 5, 'C'), (11, 9, '^')]));
    let events = until(&mut g, "R", 600, |g| g.mode() != Mode::Playing);
    assert!(events.contains(&Event::CheckpointReached));
    until(&mut g, "-", 120, |g| g.mode() == Mode::Playing);
    assert_eq!(g.camera().x, 0);
    assert_eq!(g.autoscroll(), Autoscroll::default(), "lead-in, no hold");
    // Past the spikes (still the lead-in) and on into the zone: the scroll
    // starts at once.
    g.teleport_player(12 * 16);
    assert_eq!(g.autoscroll(), Autoscroll::default());
    until(&mut g, "R", 600, |g| {
        g.autoscroll().phase == ScrollPhase::Scroll
    });
    assert_eq!(g.autoscroll().hold, 0);
    g.step(input("R"));
    assert_eq!(g.autoscroll().pos, Fx::from_px(320) + Fx(192));
}

#[test]
fn a_start_framed_inside_the_zone_holds_too() {
    let g = game(&world(80, (4, 40), &[(11, 30, 'P')]));
    assert_eq!(g.camera().x, 30 * 16 + 8 - 144);
    assert_eq!(g.autoscroll().phase, ScrollPhase::Scroll);
    assert_eq!(g.autoscroll().hold, 60);
    let g = game(&world(80, (4, 20), &[(11, 50, 'P')]));
    assert_eq!(g.autoscroll().phase, ScrollPhase::LeadOut, "past the zone");
}

#[test]
fn the_scroll_stands_still_while_frozen_and_paused() {
    // Frozen: the transformation freeze.
    let mut g = game(&world(80, (4, 40), &[(11, 30, 'P')]));
    g.scroll.hold = 0;
    g.step(input("-"));
    let scroll = g.autoscroll();
    g.player.transform = 30;
    hold(&mut g, "-", 30);
    assert_eq!(g.autoscroll(), scroll, "frozen");
    g.step(input("-"));
    assert_eq!(g.autoscroll().pos, scroll.pos + Fx(192));
    // Paused (the real flow).
    let level = world(80, (4, 40), &[(11, 30, 'P')]);
    let mut g = Game::new_at(&level, &tuning(), 1, Start::Title);
    g.step(input("S"));
    until(&mut g, "-", 400, |g| g.mode() == Mode::Playing);
    hold(&mut g, "-", 70);
    let scroll = g.autoscroll();
    assert_eq!(scroll.hold, 0);
    g.step(input("S"));
    assert_eq!(g.mode(), Mode::Paused);
    hold(&mut g, "-", 50);
    assert_eq!(g.autoscroll(), scroll, "paused");
}

#[test]
fn the_scroll_stands_still_in_the_goal_sequence() {
    // A goal inside the zone (the lint forbids it in real levels: goal ≥
    // to + 20), so the scroll is still running at the touch.
    let mut g = game(&world(80, (4, 40), &[(11, 30, 'P'), (11, 34, 'G')]));
    until(&mut g, "R", 200, |g| g.mode() == Mode::StageClear);
    assert_eq!(g.autoscroll().phase, ScrollPhase::Scroll);
    let (x, scroll) = (g.camera().x, g.autoscroll());
    hold(&mut g, "-", 200);
    assert_eq!(g.mode(), Mode::StageClear);
    assert_eq!((g.camera().x, g.autoscroll()), (x, scroll));
}

#[test]
fn the_hashes_cover_the_scroll() {
    let level = world(80, (4, 40), &[(11, 30, 'P')]);
    let a = game(&level);
    let mut b = game(&level);
    assert_eq!(a.state_hash(), b.state_hash());
    assert_eq!(a.world_hash(), b.world_hash());
    b.scroll.hold -= 1;
    assert_ne!(a.state_hash(), b.state_hash(), "hold");
    assert_ne!(a.world_hash(), b.world_hash(), "hold");
    let mut c = game(&level);
    c.scroll.pos += Fx(1);
    assert_ne!(a.state_hash(), c.state_hash(), "position");
    assert_ne!(a.world_hash(), c.world_hash(), "position");
}
