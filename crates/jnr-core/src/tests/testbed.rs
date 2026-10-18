//! The grey-box testbed level is completable with a hand-written script.

use super::*;
use crate::{Mode, Replay};

fn testbed() -> Level {
    Level::parse(include_str!("../../../../content/levels/00_testbed.level")).unwrap()
}

/// The hand-written route through `00_testbed.level`; the same file is run
/// by `cargo xtask replay` (expectations, determinism, golden shots).
fn route() -> Replay {
    Replay::parse(include_str!(
        "../../../../tests/replays/testbed_route.replay"
    ))
    .unwrap()
}

#[test]
fn hand_written_route_reaches_the_goal() {
    let mut g = Game::new(&testbed(), &tuning(), 1);
    let mut events = Vec::new();
    for i in route().frames() {
        events.extend_from_slice(g.step(i));
        if g.mode() != Mode::Playing {
            break;
        }
    }
    assert_eq!(g.deaths(), 0, "{events:?}");
    assert_eq!(
        g.mode(),
        Mode::StageClear,
        "player at x={}",
        g.player().x.to_px()
    );
    assert!(events.contains(&Event::LevelComplete));
    assert!(events.contains(&Event::Landed));
    // Pinned on purpose: any change to physics, tuning, the testbed level or
    // the hash layout changes this value. Update it deliberately (and say so
    // in the commit message) after checking the route still makes sense.
    assert_eq!(g.frame(), FINISH_FRAME, "finish frame");
    assert_eq!(
        g.state_hash(),
        FINISH_HASH,
        "state hash at the goal: {:#018x}",
        g.state_hash()
    );
}

/// Frame on which the route touches the goal.
const FINISH_FRAME: u32 = 731;
/// `state_hash` at that frame (changed by the new `[map]` tuning section, E10).
const FINISH_HASH: u64 = 0x221d_4986_9d74_2e0d;
