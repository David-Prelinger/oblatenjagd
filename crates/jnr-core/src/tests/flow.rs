//! Game flow: events, modes, death and respawn, goal, camera, determinism.

use super::*;
use crate::rng::Rng;
use crate::{Fx, Mode};

#[test]
fn jump_and_land_events() {
    let mut g = Game::new(&flat(20), &tuning(), 1);
    assert!(hold(&mut g, "-", 3).is_empty(), "standing still is quiet");
    assert_eq!(g.step(input("J")), &[Event::Jumped]);
    let events = hold(&mut g, "-", 60);
    assert_eq!(events, vec![Event::Landed]);
    assert!(g.player().on_ground);
}

#[test]
fn hazard_kills_and_player_respawns() {
    let t = tuning();
    let mut g = Game::new(&level(&["..........", "..P.......", "####^^####"]), &t, 1);
    let start = *g.player();
    let mut events = Vec::new();
    for _ in 0..60 {
        events.extend_from_slice(g.step(input("R")));
        if g.mode() != Mode::Playing {
            break;
        }
    }
    assert!(events.contains(&Event::Died), "{events:?}");
    assert_eq!(g.deaths(), 1);
    let died_at = g.player().x;
    // Frozen while dying; respawn after `death_frames`.
    let frames = t.game.death_frames as u32;
    assert!(hold(&mut g, "R", frames - 1).is_empty());
    assert_eq!(g.player().x, died_at);
    assert!(matches!(g.mode(), Mode::Dying { timer: 1 }));
    assert_eq!(g.step(input("R")), &[Event::Respawned]);
    assert_eq!(g.mode(), Mode::Playing);
    assert_eq!(*g.player(), start);
}

#[test]
fn falling_out_of_the_level_kills() {
    let mut g = Game::new(&level(&["..P...", "##....", "......"]), &tuning(), 1);
    let events = hold(&mut g, "R", 120);
    assert!(events.contains(&Event::Died), "{events:?}");
}

#[test]
fn goal_completes_the_stage() {
    let mut g = Game::new(&level(&["......G.", "..P.....", "########"]), &tuning(), 1);
    let events = hold(&mut g, "R", 120);
    assert_eq!(
        events
            .iter()
            .filter(|e| **e == Event::LevelComplete)
            .count(),
        1
    );
    assert_eq!(g.mode(), Mode::StageClear);
    let x = g.player().x;
    let h = g.state_hash();
    hold(&mut g, "RJ", 10);
    assert_eq!(g.player().x, x, "simulation stands still");
    assert_ne!(g.state_hash(), h, "the frame counter still advances");
}

#[test]
fn goal_pole_cannot_be_jumped_over() {
    // The goal cell is at the bottom; its column reaches up to the top.
    let mut g = Game::new(
        &level(&["........", "........", "..P...G.", "########"]),
        &tuning(),
        1,
    );
    hold(&mut g, "R", 20);
    hold(&mut g, "RJ", 40);
    assert_eq!(g.mode(), Mode::StageClear);
}

#[test]
fn camera_follows_right_and_blocks_walking_back() {
    let mut g = Game::new(&flat(100), &tuning(), 1);
    assert_eq!(g.camera().x, 0);
    hold(&mut g, "RB", 180);
    hold(&mut g, "-", 60);
    let cam = g.camera();
    assert!(cam.x > 0);
    let (px, _) = g.player().center_px();
    assert_eq!(
        px - cam.x,
        tuning().camera.follow_x,
        "player at the follow line"
    );
    hold(&mut g, "L", 300);
    assert_eq!(g.camera().x, cam.x, "never scrolls left");
    assert_eq!(g.player().x, Fx::from_px(cam.x), "stopped at the left edge");
    assert_eq!(g.player().vx, Fx::ZERO);
}

#[test]
fn camera_clamps_vertically_to_level() {
    let g = Game::new(&flat(30), &tuning(), 1);
    // 14 rows = 224 px, screen 180 px: the start near the ground shows the
    // bottom of the level.
    assert_eq!(g.camera().y, 224 - 180);
}

#[test]
fn one_way_platform_jump_through_and_drop_down() {
    // One-way platform 3 tiles above the ground top.
    let rows = [
        "..........",
        "..........",
        "..........",
        "..........",
        "..........",
        "..----....",
        "..........",
        "..P.......",
        "##########",
    ];
    let mut g = Game::new(&level(&rows), &tuning(), 1);
    let platform_top = Fx::from_px(5 * 16);
    g.step(input("J"));
    let landed_on_top = (0..60).any(|_| {
        g.step(input("J"));
        g.player().on_ground && g.player().y + crate::player::PLAYER_H == platform_top
    });
    assert!(landed_on_top, "jumped through from below and landed on top");
    // Standing still keeps the player on the platform.
    hold(&mut g, "-", 30);
    assert_eq!(g.player().y + crate::player::PLAYER_H, platform_top);
    // Holding DOWN drops through.
    hold(&mut g, "D", 40);
    assert_eq!(g.player().y + crate::player::PLAYER_H, Fx::from_px(8 * 16));
}

/// Pseudo-random inputs from a fixed seed (biased to the right).
fn noisy_inputs(n: usize) -> Vec<InputFrame> {
    let mut r = Rng::new(99);
    let mut held = InputFrame::RIGHT;
    (0..n)
        .map(|_| {
            if r.below(8) == 0 {
                held = InputFrame((r.next_u64() & 0x3f) as u8).with(InputFrame::RIGHT);
                if r.below(3) == 0 {
                    held = held.without(InputFrame::RIGHT);
                }
            }
            held
        })
        .collect()
}

fn hash_sequence(level: &Level, seed: u64, inputs: &[InputFrame]) -> Vec<u64> {
    let mut g = Game::new(level, &tuning(), seed);
    inputs
        .iter()
        .map(|&i| {
            g.step(i);
            g.state_hash()
        })
        .collect()
}

#[test]
fn determinism_same_inputs_same_hashes() {
    let level = Level::parse(include_str!("../../../../content/levels/00_testbed.level")).unwrap();
    let inputs = noisy_inputs(3000);
    let a = hash_sequence(&level, 1, &inputs);
    let b = hash_sequence(&level, 1, &inputs);
    assert_eq!(a, b);
    let distinct: std::collections::BTreeSet<u64> = a.iter().copied().collect();
    assert!(distinct.len() > 1000, "hash changes with the state");
    let other_seed = hash_sequence(&level, 2, &inputs);
    assert_ne!(a[0], other_seed[0], "the rng seed is part of the state");
}

#[test]
fn clones_continue_identically() {
    let level = Level::parse(include_str!("../../../../content/levels/00_testbed.level")).unwrap();
    let inputs = noisy_inputs(1200);
    let mut g = Game::new(&level, &tuning(), 1);
    for &i in &inputs[..600] {
        g.step(i);
    }
    let mut c = g.clone();
    assert_eq!(c.state_hash(), g.state_hash());
    for &i in &inputs[600..] {
        assert_eq!(g.step(i), c.step(i));
        assert_eq!(g.state_hash(), c.state_hash());
    }
}

#[test]
fn state_hash_sees_small_changes() {
    let mut a = Game::new(&flat(30), &tuning(), 1);
    let mut b = a.clone();
    a.step(input("R"));
    b.step(input("-"));
    assert_ne!(a.state_hash(), b.state_hash());
}
