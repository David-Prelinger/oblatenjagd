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
/// `state_hash` at that frame (changes whenever the hashed tuning gains keys
/// or the player gains hashed fields; M3 E15: `[hackl]`, `[crawl]` and the
/// Hackl/crawl player fields; M3 E20: `camera.climb_up_slack`/`climb_up_reach`/`climb_top_min`/`climb_catch_up_speed` and the hashed
/// `settle_feet`/`slack_x`; M4 E21: `platforms.schafbergbahn_*`; M4 E22:
/// the Operettensänger and Souvenirstand `[enemies]` keys; M4 E23: the
/// `enemies.reisebus_*` keys; M5 E28: `platforms.traundl_*`/`dampfer_*`;
/// M5 E29: the `enemies.schwan_*`, `gelse_*` and `regattaboot_*` keys;
/// M5 E26: `camera.autoscroll_*`; M5 E27: the `[swim]` keys and the hashed
/// `swimming`/`leaving_water`/`swim_frames`; M5 E32:
/// `camera.autoscroll_settle_speed`/`autoscroll_catch_up_speed`; M5 E34:
/// `camera.autoscroll_ground_y`/`autoscroll_top`; M6 E38: the
/// `enemies.kurgast_*` keys; M6 E37: `platforms.fensterladen_*` and the
/// hashed stage clock; M6 E36: the `enemies.ampel_*` and `auto_*` keys;
/// M6 E39: the `enemies.offizier_*` keys; route and frame unchanged).
const FINISH_HASH: u64 = 0x3fe6_a50c_1d43_3f3f;
