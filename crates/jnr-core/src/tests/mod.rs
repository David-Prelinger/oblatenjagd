//! Integration-style tests of the whole simulation (game feel, game flow,
//! determinism, the testbed level). Unit tests live next to their modules.

mod feel;
mod flow;
mod testbed;

use crate::{Event, Game, InputFrame, Level, Tuning};

/// The real tuning from `content/tuning.toml`.
fn tuning() -> Tuning {
    Tuning::parse(include_str!("../../../../content/tuning.toml")).unwrap()
}

/// Builds a level from grid rows of `.`, `#`, `-`, `^`, `P`, `G`.
fn level(rows: &[&str]) -> Level {
    let grid = rows.join("\n");
    let text = format!(
        "[meta]\nid = \"test\"\n[legend]\n\"#\" = \"solid\"\n\"-\" = \"oneway\"\n\"^\" = \"hazard\"\n\"P\" = {{ entity = \"player_start\" }}\n\"G\" = {{ entity = \"goal\" }}\n[map]\ngrid = '''\n{grid}\n'''\n"
    );
    Level::parse(&text).unwrap()
}

/// A flat 14-row test map, `width` columns wide: ground at rows 12–13, the
/// player starts at column 2.
fn flat(width: usize) -> Level {
    let air = ".".repeat(width);
    let ground = "#".repeat(width);
    let start = format!("..P{}", ".".repeat(width - 3));
    let mut rows: Vec<&str> = vec![&air; 11];
    rows.push(&start);
    rows.push(&ground);
    rows.push(&ground);
    level(&rows)
}

fn input(token: &str) -> InputFrame {
    InputFrame::parse_token(token).unwrap()
}

/// Steps `frames` times with the same input and collects all events.
fn hold(game: &mut Game, token: &str, frames: u32) -> Vec<Event> {
    let i = input(token);
    let mut events = Vec::new();
    for _ in 0..frames {
        events.extend_from_slice(game.step(i));
    }
    events
}
