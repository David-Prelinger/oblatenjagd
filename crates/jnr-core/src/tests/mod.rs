//! Integration-style tests of the whole simulation (game feel, game flow,
//! determinism, the testbed level). Unit tests live next to their modules.

mod autoscroll;
mod bad_ischl;
mod boats;
mod climb;
mod enemies;
mod feel;
mod finale;
mod flow;
mod goal;
mod hackl;
mod items;
mod kellner;
mod kurgast;
mod lake;
mod lake_enemies;
mod m2;
mod m3;
mod m4;
mod m6_mix;
mod m7_cafe;
mod modes;
mod oberkellner;
mod offiziere;
mod reisebus;
mod route;
mod schafbergbahn;
mod shutters;
mod swim;
mod testbed;
mod traffic;

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

/// Legend of [`pool`] (`.` air).
const POOL_LEGEND: &str = r##"
"#" = "solid"
"-" = "oneway"
"~" = "see_wasser"
"w" = "wolfgangsee_water"
"B" = "brick"
"M" = { tile = "oblaten_box", contains = "mozartkugel" }
"P" = { entity = "player_start" }
"S" = { entity = "player_start", tile = "see_wasser" }
"o" = { entity = "oblate", tile = "see_wasser" }
"t" = { entity = "taube", tile = "see_wasser" }
"u" = { entity = "taube" }
"f" = { entity = "fiaker", tile = "see_wasser" }
"T" = { entity = "selfie_tourist", still = true }
"W" = { entity = "selfie_tourist" }
"G" = { entity = "goal" }
"##;

/// A 14-row swimming pool (D-076) `width` columns wide: solid quay in rows
/// 8–13 (top y 128), except in the columns `water`, which hold swim water
/// (`see_wasser`) in rows 8–12 (surface y 128, flush with the quay) over a
/// solid lake bed in row 13 (top y 208). The player starts on the quay at
/// column 2 (feet 128) unless `marks` places a `P` or `S` (start in the
/// water); `marks` are (row, col, char) in [`POOL_LEGEND`].
fn pool(width: usize, water: std::ops::Range<usize>, marks: &[(usize, usize, char)]) -> Level {
    let mut rows: Vec<Vec<char>> = (0..14)
        .map(|r| {
            (0..width)
                .map(|c| match r {
                    0..=7 => '.',
                    8..=12 if water.contains(&c) => '~',
                    _ => '#',
                })
                .collect()
        })
        .collect();
    if !marks.iter().any(|m| m.2 == 'P' || m.2 == 'S') {
        rows[7][2] = 'P';
    }
    for &(r, c, ch) in marks {
        rows[r][c] = ch;
    }
    let grid: Vec<String> = rows.into_iter().map(|r| r.into_iter().collect()).collect();
    let text = format!(
        "[meta]\nid = \"pool\"\ntime_limit = 300\n[legend]\n{POOL_LEGEND}\n[map]\ngrid = '''\n{}\n'''\n",
        grid.join("\n")
    );
    Level::parse(&text).unwrap()
}
