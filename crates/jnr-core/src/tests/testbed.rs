//! The grey-box testbed level is completable with a hand-written script.

use super::*;
use crate::Mode;

fn testbed() -> Level {
    Level::parse(include_str!("../../../../content/levels/00_testbed.level")).unwrap()
}

/// Hand-written route through `00_testbed.level`, in the replay body format
/// (`<token> <frames>`, `#` comments).
const ROUTE: &str = "
RB 110    # run up to the steps
RBJ 35    # running jump over the steps
RB 60
RBJ 20    # gap of 3
RB 73
RBJ 35    # gap of 6
RB 38
RBJ 40    # running jump onto the 5-tile ledge (rises against the wall)
RB 29
RBJ 1     # hop off the ledge onto the low one-way platform
RB 35
RBJ 14    # up through the high one-way platform from below
RB 38
RBJ 30    # off the high platform over the hazard pit
RB 200    # to the goal
";

/// Parses replay body lines into one input per frame.
fn parse_route(text: &str) -> Vec<InputFrame> {
    let mut frames = Vec::new();
    for line in text.lines() {
        let line = line.split('#').next().unwrap().trim();
        if line.is_empty() {
            continue;
        }
        let (token, n) = line.split_once(' ').expect("<token> <frames>");
        let n: usize = n.trim().parse().unwrap();
        frames.extend(std::iter::repeat_n(input(token), n));
    }
    frames
}

#[test]
fn hand_written_route_reaches_the_goal() {
    let mut g = Game::new(&testbed(), &tuning(), 1);
    let mut events = Vec::new();
    for i in parse_route(ROUTE) {
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
/// `state_hash` at that frame.
const FINISH_HASH: u64 = 0xfcac_268c_3aa5_3c25;

#[test]
fn route_parser() {
    let r = parse_route("R 2 # comment\n\n- 1\nRBJ 1\n");
    assert_eq!(r, vec![input("R"), input("R"), input("-"), input("RBJ")]);
}
