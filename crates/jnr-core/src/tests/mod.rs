//! Integration-style tests of the whole simulation (game feel, game flow,
//! determinism, the testbed level). Unit tests live next to their modules.

mod enemies;
mod feel;
mod flow;
mod goal;
mod hackl;
mod items;
mod m2;
mod m3;
mod modes;
mod route;
mod testbed;

use crate::{Event, Game, InputFrame, Level, Tuning};

/// The real tuning from `content/tuning.toml`.
fn tuning() -> Tuning {
    Tuning::parse(include_str!("../../../../content/tuning.toml")).unwrap()
}

/// Builds a level from grid rows of `.`, `#`, `-`, `^` (spikes), `~`
/// (Salzach water), `P`, `G`, `t`
/// (Selfie-Tourist), `v` (Mozartkugel-Verkäufer), `M` (Mozartkugel box).
fn level(rows: &[&str]) -> Level {
    let grid = rows.join("\n");
    let text = format!(
        "[meta]\nid = \"test\"\ntime_limit = 300\n[legend]\n\"#\" = \"solid\"\n\"-\" = \"oneway\"\n\"^\" = \"hazard\"\n\"~\" = \"salzach_water\"\n\"P\" = {{ entity = \"player_start\" }}\n\"G\" = {{ entity = \"goal\" }}\n\"t\" = {{ entity = \"selfie_tourist\" }}\n\"v\" = {{ entity = \"mozartkugel_verkaeufer\" }}\n\"M\" = {{ tile = \"oblaten_box\", contains = \"mozartkugel\" }}\n[map]\ngrid = '''\n{grid}\n'''\n"
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

/// Steps until `pred` holds (at most `max` frames); returns all events.
fn until(g: &mut Game, token: &str, max: u32, pred: impl Fn(&Game) -> bool) -> Vec<Event> {
    let mut events = Vec::new();
    for _ in 0..max {
        events.extend_from_slice(g.step(input(token)));
        if pred(g) {
            return events;
        }
    }
    panic!("condition not reached in {max} frames; events: {events:?}");
}
