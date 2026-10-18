//! Replay files (`tests/replays/*.replay`): parsing, formatting, running a
//! replay against a level and checking its expectations.
//!
//! ```text
//! level  = testbed
//! seed   = 1
//! source = handwritten          # solver | human | handwritten
//! expect = complete frames=731..731 deaths=0
//! shots  = 0:start 760:clear    # frame:name; frame 0 = before the first step
//! ---
//! RB 110    # <input token> <frames>; "-" = nothing held
//! RBJ 35
//! ```
//!
//! Header lines are `key = value`; `expect` and `shots` may repeat and each
//! hold several space-separated items. `#` starts a comment anywhere. Frame
//! numbers are [`Game::frame`] values: the state after that many steps.
//! Ranges `a..b` are inclusive. All expectations are checked after the last
//! input (the replay always runs to its end).
//! A line holds at most [`MAX_FRAMES_PER_LINE`] frames and the whole
//! replay at most [`MAX_TOTAL_FRAMES`] (1 h at 60 Hz).

use std::fmt;

use crate::ParseError;
use crate::game::{Event, Game, Mode};
use crate::input::InputFrame;
use crate::level::Level;
use crate::tuning::Tuning;

/// Most frames one input line may hold (1 h at 60 Hz).
pub const MAX_FRAMES_PER_LINE: u32 = 216_000;
/// Most frames a whole replay may hold (1 h at 60 Hz); keeps a bad file
/// from making the runner allocate or loop for ever.
pub const MAX_TOTAL_FRAMES: u32 = 216_000;

/// Where a replay came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    /// Written by `cargo xtask solve`.
    Solver,
    /// Recorded by a person playing (`jnr-app --record`).
    Human,
    /// Typed by hand (bug reproductions, scripted routes).
    Handwritten,
}

impl Source {
    /// The name used in replay files.
    pub const fn name(self) -> &'static str {
        match self {
            Source::Solver => "solver",
            Source::Human => "human",
            Source::Handwritten => "handwritten",
        }
    }

    fn parse(s: &str) -> Option<Source> {
        [Source::Solver, Source::Human, Source::Handwritten]
            .into_iter()
            .find(|v| v.name() == s)
    }
}

/// An inclusive frame range `first..last`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameRange {
    /// First frame in the range.
    pub first: u32,
    /// Last frame in the range (inclusive).
    pub last: u32,
}

impl FrameRange {
    /// True if `frame` is inside the range.
    pub const fn contains(self, frame: u32) -> bool {
        frame >= self.first && frame <= self.last
    }

    fn parse(s: &str) -> Result<FrameRange, String> {
        let (a, b) = s
            .split_once("..")
            .ok_or_else(|| format!("bad frame range `{s}` (expected `first..last`)"))?;
        let first = parse_u32(a, "frame")?;
        let last = parse_u32(b, "frame")?;
        if first > last {
            return Err(format!("frame range `{s}` is empty (first > last)"));
        }
        Ok(FrameRange { first, last })
    }
}

impl fmt::Display for FrameRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}..{}", self.first, self.last)
    }
}

/// The game mode at the end of a replay, as named in `state=<…>`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EndState {
    /// [`Mode::Playing`].
    Playing,
    /// [`Mode::Dying`].
    Dying,
    /// [`Mode::StageClear`].
    StageClear,
    /// [`Mode::GameOver`].
    GameOver,
}

impl EndState {
    /// The name used in replay files.
    pub const fn name(self) -> &'static str {
        match self {
            EndState::Playing => "playing",
            EndState::Dying => "dying",
            EndState::StageClear => "stage_clear",
            EndState::GameOver => "game_over",
        }
    }

    /// The state of a game mode.
    pub const fn of(mode: Mode) -> EndState {
        match mode {
            Mode::Playing => EndState::Playing,
            Mode::Dying { .. } => EndState::Dying,
            Mode::StageClear => EndState::StageClear,
            Mode::GameOver { .. } => EndState::GameOver,
        }
    }

    fn parse(s: &str) -> Option<EndState> {
        [
            EndState::Playing,
            EndState::Dying,
            EndState::StageClear,
            EndState::GameOver,
        ]
        .into_iter()
        .find(|v| v.name() == s)
    }
}

/// One expectation clause of an `expect` line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Expect {
    /// `complete [frames=a..b]`: `LevelComplete` happened (in the range).
    Complete(Option<FrameRange>),
    /// `deaths=N`: exactly N deaths.
    Deaths(u32),
    /// `deaths<=N`: at most N deaths.
    DeathsAtMost(u32),
    /// `dies_at frames=a..b`: at least one death in the range.
    DiesAt(FrameRange),
    /// `player_x>=PX`: the player's hitbox left edge (whole level pixels)
    /// is at least PX at the end.
    PlayerXAtLeast(i32),
    /// `state=<playing|dying|stage_clear|game_over>` at the end.
    State(EndState),
    /// `no_event=<Event>`: the event never happened.
    NoEvent(Event),
    /// `oblaten>=N`: the Oblaten counter (HUD value, 0..100) is at least N
    /// at the end.
    OblatenAtLeast(u32),
    /// `score>=N`: the score is at least N at the end.
    ScoreAtLeast(u32),
    /// `lives=N`: exactly N lives at the end.
    Lives(u32),
}

/// What the expectation grammar supports (shown in "not supported yet" errors).
const SUPPORTED: &str = "complete [frames=a..b], deaths=N, deaths<=N, dies_at frames=a..b, \
                         player_x>=PX, state=<playing|dying|stage_clear|game_over>, \
                         no_event=<Event>, oblaten>=N, score>=N, lives=N";

impl Expect {
    /// Parses the space-separated clauses of one `expect` value.
    fn parse_clauses(value: &str) -> Result<Vec<Expect>, String> {
        let mut out = Vec::new();
        let mut words = value.split_whitespace().peekable();
        while let Some(w) = words.next() {
            let mut frames_arg = || -> Result<Option<FrameRange>, String> {
                match words.next_if(|n| n.starts_with("frames=")) {
                    Some(n) => FrameRange::parse(&n["frames=".len()..]).map(Some),
                    None => Ok(None),
                }
            };
            let e = if w == "complete" {
                Expect::Complete(frames_arg()?)
            } else if w == "dies_at" {
                Expect::DiesAt(frames_arg()?.ok_or("`dies_at` needs `frames=a..b` after it")?)
            } else if w.starts_with("frames=") {
                return Err(format!("`{w}` must follow `complete` or `dies_at`"));
            } else if let Some(n) = w.strip_prefix("deaths<=") {
                Expect::DeathsAtMost(parse_u32(n, "deaths")?)
            } else if let Some(n) = w.strip_prefix("deaths=") {
                Expect::Deaths(parse_u32(n, "deaths")?)
            } else if let Some(n) = w.strip_prefix("player_x>=") {
                Expect::PlayerXAtLeast(
                    n.parse()
                        .map_err(|_| format!("bad player_x `{n}` (expected whole pixels)"))?,
                )
            } else if let Some(s) = w.strip_prefix("state=") {
                Expect::State(EndState::parse(s).ok_or_else(|| {
                    format!(
                        "unknown state `{s}` (expected playing, dying, stage_clear or game_over)"
                    )
                })?)
            } else if let Some(n) = w.strip_prefix("oblaten>=") {
                Expect::OblatenAtLeast(parse_u32(n, "oblaten")?)
            } else if let Some(n) = w.strip_prefix("score>=") {
                Expect::ScoreAtLeast(parse_u32(n, "score")?)
            } else if let Some(n) = w.strip_prefix("lives=") {
                Expect::Lives(parse_u32(n, "lives")?)
            } else if let Some(name) = w.strip_prefix("no_event=") {
                Expect::NoEvent(Event::from_name(name).ok_or_else(|| {
                    let names: Vec<&str> = Event::ALL.iter().map(|e| e.name()).collect();
                    format!("unknown event `{name}` (known: {})", names.join(", "))
                })?)
            } else {
                return Err(format!("not supported yet: `{w}` (supported: {SUPPORTED})"));
            };
            out.push(e);
        }
        Ok(out)
    }

    /// Checks the expectation against what happened; `Err` says why it failed.
    pub fn check(&self, o: &Outcome) -> Result<(), String> {
        let ok = match *self {
            Expect::Complete(range) => match (o.completed_at, range) {
                (Some(_), None) => return Ok(()),
                (Some(f), Some(r)) if r.contains(f) => return Ok(()),
                (Some(f), Some(r)) => {
                    return Err(format!("expected complete in {r}, completed at {f}"));
                }
                (None, _) => false,
            },
            Expect::Deaths(n) => o.deaths == n,
            Expect::DeathsAtMost(n) => o.deaths <= n,
            Expect::DiesAt(r) => o.deaths_at.iter().any(|&f| r.contains(f)),
            Expect::PlayerXAtLeast(px) => o.player_x >= px,
            Expect::State(s) => o.state == s,
            Expect::NoEvent(e) => !o.events.iter().any(|&(seen, _)| seen == e),
            Expect::OblatenAtLeast(n) => o.oblaten >= n,
            Expect::ScoreAtLeast(n) => o.score >= n,
            Expect::Lives(n) => o.lives == n,
        };
        if ok {
            return Ok(());
        }
        Err(match *self {
            Expect::Complete(_) => format!(
                "expected {self}, never completed (state={}, player_x={} after {} frames)",
                o.state.name(),
                o.player_x,
                o.frames
            ),
            Expect::Deaths(_) | Expect::DeathsAtMost(_) => {
                format!("expected {self}, got {} {}", o.deaths, o.death_frames())
            }
            Expect::DiesAt(r) => format!("expected a death in {r}, {}", o.death_frames()),
            Expect::PlayerXAtLeast(_) => {
                format!("expected {self} at the end, got player_x={}", o.player_x)
            }
            Expect::State(_) => format!("expected {self} at the end, got {}", o.state.name()),
            Expect::NoEvent(e) => {
                let first = o.events.iter().find(|(seen, _)| *seen == e).map(|x| x.1);
                format!("expected {self}, first at frame {}", first.unwrap_or(0))
            }
            Expect::OblatenAtLeast(_) => {
                format!("expected {self} at the end, got oblaten={}", o.oblaten)
            }
            Expect::ScoreAtLeast(_) => format!("expected {self} at the end, got score={}", o.score),
            Expect::Lives(_) => format!("expected {self} at the end, got lives={}", o.lives),
        })
    }
}

impl fmt::Display for Expect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Expect::Complete(None) => f.write_str("complete"),
            Expect::Complete(Some(r)) => write!(f, "complete frames={r}"),
            Expect::Deaths(n) => write!(f, "deaths={n}"),
            Expect::DeathsAtMost(n) => write!(f, "deaths<={n}"),
            Expect::DiesAt(r) => write!(f, "dies_at frames={r}"),
            Expect::PlayerXAtLeast(px) => write!(f, "player_x>={px}"),
            Expect::State(s) => write!(f, "state={}", s.name()),
            Expect::NoEvent(e) => write!(f, "no_event={}", e.name()),
            Expect::OblatenAtLeast(n) => write!(f, "oblaten>={n}"),
            Expect::ScoreAtLeast(n) => write!(f, "score>={n}"),
            Expect::Lives(n) => write!(f, "lives={n}"),
        }
    }
}

/// A screenshot request: render the state at `frame` as `<replay>_<name>.png`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Shot {
    /// The [`Game::frame`] to render (0 = before the first step).
    pub frame: u32,
    /// Lower-case name (`a-z`, `0-9`, `_`), unique within the replay.
    pub name: String,
}

/// A parsed replay file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Replay {
    /// Level id (`meta.id` of a `.level` file).
    pub level: String,
    /// Seed for [`Game::new`]; default 1.
    pub seed: u64,
    /// Where the replay came from.
    pub source: Source,
    /// All expectations, in file order.
    pub expects: Vec<Expect>,
    /// Frames to render, in file order.
    pub shots: Vec<Shot>,
    /// Run-length encoded inputs: (input, frames held), each count ≥ 1.
    pub inputs: Vec<(InputFrame, u32)>,
}

impl Replay {
    /// Parses replay text. Errors name the line (`line 4: …`).
    pub fn parse(text: &str) -> Result<Replay, ParseError> {
        let mut level: Option<String> = None;
        let mut seed: Option<u64> = None;
        let mut source: Option<Source> = None;
        let mut expects = Vec::new();
        let mut shots: Vec<Shot> = Vec::new();
        // Line of each shot (parallel to `shots`) for errors found after the body.
        let mut shot_lines: Vec<usize> = Vec::new();
        let mut inputs = Vec::new();
        let mut total_frames: u32 = 0;
        let mut in_body = false;

        for (i, raw) in text.lines().enumerate() {
            let line_no = i + 1;
            let err = |msg: String| ParseError::new(format!("line {line_no}: {msg}"));
            let line = raw.split('#').next().unwrap_or("").trim();
            if line.is_empty() {
                continue;
            }
            if in_body {
                let mut parts = line.split_whitespace();
                let (Some(token), Some(n), None) = (parts.next(), parts.next(), parts.next())
                else {
                    return Err(err(format!(
                        "expected `<input token> <frames>`, got `{line}`"
                    )));
                };
                let input = InputFrame::parse_token(token).map_err(|e| err(e.to_string()))?;
                let n = parse_u32(n, "frame count").map_err(err)?;
                if n == 0 {
                    return Err(err("frame count must be at least 1".into()));
                }
                if n > MAX_FRAMES_PER_LINE {
                    return Err(err(format!(
                        "frame count {n} is above the per-line limit of {MAX_FRAMES_PER_LINE}"
                    )));
                }
                total_frames += n;
                if total_frames > MAX_TOTAL_FRAMES {
                    return Err(err(format!(
                        "replay is longer than {MAX_TOTAL_FRAMES} frames in total (1 h at 60 Hz)"
                    )));
                }
                inputs.push((input, n));
                continue;
            }
            if line == "---" {
                in_body = true;
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                return Err(err(format!(
                    "expected `key = value` or `---`, got `{line}`"
                )));
            };
            let (key, value) = (key.trim(), value.trim());
            let once = |seen: bool| {
                if seen {
                    Err(err(format!("`{key}` given twice")))
                } else {
                    Ok(())
                }
            };
            match key {
                "level" => {
                    once(level.is_some())?;
                    if value.is_empty() {
                        return Err(err("`level` is empty".into()));
                    }
                    level = Some(value.to_owned());
                }
                "seed" => {
                    once(seed.is_some())?;
                    seed = Some(
                        value
                            .parse()
                            .map_err(|_| err(format!("bad seed `{value}`")))?,
                    );
                }
                "source" => {
                    once(source.is_some())?;
                    source = Some(Source::parse(value).ok_or_else(|| {
                        err(format!(
                            "unknown source `{value}` (expected solver, human or handwritten)"
                        ))
                    })?);
                }
                "expect" => expects.extend(Expect::parse_clauses(value).map_err(err)?),
                "shots" => {
                    for item in value.split_whitespace() {
                        let shot = parse_shot(item).map_err(err)?;
                        if shots.iter().any(|s| s.name == shot.name) {
                            return Err(err(format!("shot name `{}` used twice", shot.name)));
                        }
                        shots.push(shot);
                        shot_lines.push(line_no);
                    }
                }
                _ => {
                    return Err(err(format!(
                        "unknown header key `{key}` (known: level, seed, source, expect, shots)"
                    )));
                }
            }
        }

        if !in_body {
            return Err(ParseError::new(
                "missing the `---` line between header and inputs",
            ));
        }
        let level = level.ok_or_else(|| ParseError::new("header is missing `level`"))?;
        let source = source.ok_or_else(|| ParseError::new("header is missing `source`"))?;
        let replay = Replay {
            level,
            seed: seed.unwrap_or(1),
            source,
            expects,
            shots,
            inputs,
        };
        let total = replay.frame_count();
        if let Some((s, line)) = replay
            .shots
            .iter()
            .zip(&shot_lines)
            .find(|(s, _)| s.frame > total)
        {
            return Err(ParseError::new(format!(
                "line {line}: shot `{}` at frame {} is after the last frame ({total})",
                s.name, s.frame
            )));
        }
        Ok(replay)
    }

    /// The canonical text form; `Replay::parse(&r.format()) == Ok(r)`.
    /// Comments are not kept.
    pub fn format(&self) -> String {
        let mut s = format!(
            "level  = {}\nseed   = {}\nsource = {}\n",
            self.level,
            self.seed,
            self.source.name()
        );
        if !self.expects.is_empty() {
            let clauses: Vec<String> = self.expects.iter().map(Expect::to_string).collect();
            s.push_str(&format!("expect = {}\n", clauses.join(" ")));
        }
        if !self.shots.is_empty() {
            let shots: Vec<String> = self
                .shots
                .iter()
                .map(|sh| format!("{}:{}", sh.frame, sh.name))
                .collect();
            s.push_str(&format!("shots  = {}\n", shots.join(" ")));
        }
        s.push_str("---\n");
        for (input, n) in &self.inputs {
            s.push_str(&format!("{input} {n}\n"));
        }
        s
    }

    /// One input per frame (the run-length encoding expanded).
    pub fn frames(&self) -> impl Iterator<Item = InputFrame> + '_ {
        self.inputs
            .iter()
            .flat_map(|&(input, n)| std::iter::repeat_n(input, n as usize))
    }

    /// Total number of frames (steps) in the replay.
    pub fn frame_count(&self) -> u32 {
        self.inputs
            .iter()
            .fold(0u32, |acc, &(_, n)| acc.saturating_add(n))
    }

    /// Plays the replay on a new [`Game`] and returns what happened.
    /// `on_frame` sees the game before the first step (frame 0) and after
    /// every step (for hashes and screenshots). The caller checks that
    /// `level` is the replay's level.
    pub fn run(&self, level: &Level, tuning: &Tuning, mut on_frame: impl FnMut(&Game)) -> Outcome {
        let mut game = Game::new(level, tuning, self.seed);
        let mut outcome = Outcome::new(&game);
        on_frame(&game);
        for input in self.frames() {
            game.step(input);
            outcome.record(&game, game.events());
            on_frame(&game);
        }
        outcome
    }

    /// Checks every expectation; returns the failure messages (empty = pass).
    pub fn check(&self, outcome: &Outcome) -> Vec<String> {
        self.expects
            .iter()
            .filter_map(|e| e.check(outcome).err())
            .collect()
    }
}

/// What happened during a replay: everything the expectations look at.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Outcome {
    /// Steps run.
    pub frames: u32,
    /// Frame of the first `LevelComplete`.
    pub completed_at: Option<u32>,
    /// Frames of every `Died`.
    pub deaths_at: Vec<u32>,
    /// Every event kind seen, with the frame it first happened.
    pub events: Vec<(Event, u32)>,
    /// Deaths at the end.
    pub deaths: u32,
    /// Player hitbox left edge at the end, whole level pixels.
    pub player_x: i32,
    /// Mode at the end.
    pub state: EndState,
    /// Oblaten counter at the end.
    pub oblaten: u32,
    /// Score at the end.
    pub score: u32,
    /// Lives at the end.
    pub lives: u32,
}

impl Outcome {
    /// The outcome of a game that has not stepped yet.
    pub fn new(game: &Game) -> Outcome {
        Outcome {
            frames: game.frame(),
            completed_at: None,
            deaths_at: Vec::new(),
            events: Vec::new(),
            deaths: game.deaths(),
            player_x: game.player().x.to_px(),
            state: EndState::of(game.mode()),
            oblaten: u32::from(game.oblaten()),
            score: game.score(),
            lives: u32::from(game.lives()),
        }
    }

    /// Records one step: `events` are what that step returned.
    pub fn record(&mut self, game: &Game, events: &[Event]) {
        let frame = game.frame();
        for &e in events {
            if !self.events.iter().any(|&(seen, _)| seen == e) {
                self.events.push((e, frame));
            }
            match e {
                Event::LevelComplete if self.completed_at.is_none() => {
                    self.completed_at = Some(frame);
                }
                Event::Died => self.deaths_at.push(frame),
                _ => {}
            }
        }
        self.frames = frame;
        self.deaths = game.deaths();
        self.player_x = game.player().x.to_px();
        self.state = EndState::of(game.mode());
        self.oblaten = u32::from(game.oblaten());
        self.score = game.score();
        self.lives = u32::from(game.lives());
    }

    /// "deaths at frames 10, 20" or "no deaths".
    fn death_frames(&self) -> String {
        if self.deaths_at.is_empty() {
            return "(no deaths)".to_owned();
        }
        let frames: Vec<String> = self.deaths_at.iter().map(u32::to_string).collect();
        format!("(deaths at frames {})", frames.join(", "))
    }
}

fn parse_u32(s: &str, what: &str) -> Result<u32, String> {
    s.parse()
        .map_err(|_| format!("bad {what} `{s}` (expected a whole number ≥ 0)"))
}

fn parse_shot(item: &str) -> Result<Shot, String> {
    let (frame, name) = item
        .split_once(':')
        .ok_or_else(|| format!("bad shot `{item}` (expected `frame:name`)"))?;
    let frame = parse_u32(frame, "shot frame")?;
    let valid = !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
    if !valid {
        return Err(format!("bad shot name `{name}` (use a-z, 0-9 and _ only)"));
    }
    Ok(Shot {
        frame,
        name: name.to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXAMPLE: &str = "\
# a comment line
level  = testbed
seed   = 7
source = handwritten   # trailing comment
expect = complete frames=10..20 deaths=0
expect = no_event=Died state=stage_clear
shots  = 0:start 5:jump
---
R 3       # walk
RBJ 2
- 1
";

    fn input(t: &str) -> InputFrame {
        InputFrame::parse_token(t).unwrap()
    }

    fn err(text: &str) -> String {
        Replay::parse(text).unwrap_err().to_string()
    }

    #[test]
    fn parses_header_and_body() {
        let r = Replay::parse(EXAMPLE).unwrap();
        assert_eq!(r.level, "testbed");
        assert_eq!(r.seed, 7);
        assert_eq!(r.source, Source::Handwritten);
        assert_eq!(
            r.expects,
            vec![
                Expect::Complete(Some(FrameRange {
                    first: 10,
                    last: 20
                })),
                Expect::Deaths(0),
                Expect::NoEvent(Event::Died),
                Expect::State(EndState::StageClear),
            ]
        );
        assert_eq!(
            r.shots[1],
            Shot {
                frame: 5,
                name: "jump".into()
            }
        );
        assert_eq!(
            r.inputs,
            vec![(input("R"), 3), (input("RBJ"), 2), (input("-"), 1)]
        );
        assert_eq!(r.frame_count(), 6);
        let frames: Vec<InputFrame> = r.frames().collect();
        assert_eq!(
            frames,
            ["R", "R", "R", "RBJ", "RBJ", "-"].map(input).to_vec()
        );
    }

    #[test]
    fn format_round_trips() {
        let r = Replay::parse(EXAMPLE).unwrap();
        let text = r.format();
        assert_eq!(Replay::parse(&text).unwrap(), r);
        assert!(text.contains(
            "expect = complete frames=10..20 deaths=0 no_event=Died state=stage_clear\n"
        ));
        let all = "level=x\nsource=solver\nexpect=complete dies_at frames=1..2 deaths<=3 \
                   player_x>=-4 state=dying state=playing state=game_over oblaten>=3 \
                   score>=100 lives=0\n---\nLRUDBJS 1\n";
        let r = Replay::parse(all).unwrap();
        assert_eq!(r.seed, 1, "default seed");
        assert_eq!(r.expects.len(), 10);
        assert_eq!(Replay::parse(&r.format()).unwrap(), r);
    }

    #[test]
    fn defaults_and_empty_body() {
        let r = Replay::parse("level = a\nsource = human\n---\n").unwrap();
        assert_eq!((r.seed, r.frame_count()), (1, 0));
        assert!(r.expects.is_empty() && r.shots.is_empty());
    }

    #[test]
    fn frame_counts_are_bounded() {
        let base = "level = a\nsource = solver\n---\n";
        // Exactly at the limits is fine.
        let ok = Replay::parse(&format!("{base}R {MAX_FRAMES_PER_LINE}\n")).unwrap();
        assert_eq!(ok.frame_count(), MAX_TOTAL_FRAMES);
        assert_eq!(
            err(&format!("{base}R 4000000000\nR 4000000000\n")),
            format!(
                "line 4: frame count 4000000000 is above the per-line limit of {MAX_FRAMES_PER_LINE}"
            )
        );
        let half = MAX_TOTAL_FRAMES / 2;
        assert_eq!(
            err(&format!("{base}R {half}\n- {half}\nB 1\n")),
            format!(
                "line 6: replay is longer than {MAX_TOTAL_FRAMES} frames in total (1 h at 60 Hz)"
            )
        );
    }

    #[test]
    fn errors_have_line_numbers() {
        let base = "level = a\nsource = solver\n";
        assert_eq!(
            err(&format!("{base}expect = complete coins>=30\n---\n")),
            format!("line 3: not supported yet: `coins>=30` (supported: {SUPPORTED})")
        );
        assert!(
            err(&format!("{base}---\nR\n"))
                .starts_with("line 4: expected `<input token> <frames>`")
        );
        assert!(err(&format!("{base}---\nRX 3\n")).starts_with("line 4: unknown input letter 'X'"));
        assert_eq!(
            err(&format!("{base}---\nR 0\n")),
            "line 4: frame count must be at least 1"
        );
        assert!(err(&format!("{base}---\nR -2\n")).starts_with("line 4: bad frame count `-2`"));
        assert_eq!(
            err("level = a\nlevel = b\n---\n"),
            "line 2: `level` given twice"
        );
        assert!(err("leve = a\n---\n").starts_with("line 1: unknown header key `leve`"));
        assert!(err("level a\n---\n").starts_with("line 1: expected `key = value`"));
        assert!(err("source = bot\n").starts_with("line 1: unknown source `bot`"));
        assert_eq!(
            err(base),
            "missing the `---` line between header and inputs"
        );
        assert_eq!(err("source = human\n---\n"), "header is missing `level`");
        assert_eq!(err("level = a\n---\n"), "header is missing `source`");
        assert!(err(&format!("{base}seed = -1\n---\n")).starts_with("line 3: bad seed"));
    }

    #[test]
    fn expectation_and_shot_errors() {
        let e = |exp: &str| {
            err(&format!(
                "level = a\nsource = solver\nexpect = {exp}\n---\n"
            ))
        };
        assert_eq!(
            e("dies_at"),
            "line 3: `dies_at` needs `frames=a..b` after it"
        );
        assert_eq!(
            e("frames=1..2"),
            "line 3: `frames=1..2` must follow `complete` or `dies_at`"
        );
        assert_eq!(
            e("complete frames=5..4"),
            "line 3: frame range `5..4` is empty (first > last)"
        );
        assert_eq!(
            e("complete frames=5"),
            "line 3: bad frame range `5` (expected `first..last`)"
        );
        assert!(e("state=won").starts_with("line 3: unknown state `won`"));
        assert!(
            e("no_event=Exploded").starts_with("line 3: unknown event `Exploded` (known: Jumped,")
        );
        assert!(e("deaths=x").starts_with("line 3: bad deaths `x`"));
        assert!(e("oblaten>=-1").starts_with("line 3: bad oblaten `-1`"));
        assert!(e("score>=x").starts_with("line 3: bad score `x`"));
        assert!(e("lives=").starts_with("line 3: bad lives ``"));

        let s = |shots: &str| {
            err(&format!(
                "level = a\nsource = solver\nshots = {shots}\n---\nR 10\n"
            ))
        };
        assert_eq!(s("1:a 2:a"), "line 3: shot name `a` used twice");
        assert!(s("1-a").starts_with("line 3: bad shot `1-a`"));
        assert!(s("1:Big").starts_with("line 3: bad shot name `Big`"));
        assert_eq!(
            s("11:late"),
            "line 3: shot `late` at frame 11 is after the last frame (10)"
        );
        assert!(Replay::parse("level = a\nsource = solver\nshots = 10:end\n---\nR 10\n").is_ok());
        // The error names the `shots` line of the late shot, not the last `shots` line.
        assert_eq!(
            err("level = a\nsource = solver\nshots = 11:late\nshots = 1:early\n---\nR 10\n"),
            "line 3: shot `late` at frame 11 is after the last frame (10)"
        );
    }

    fn outcome() -> Outcome {
        Outcome {
            frames: 800,
            completed_at: Some(731),
            deaths_at: vec![100, 300],
            events: vec![
                (Event::Jumped, 5),
                (Event::Died, 100),
                (Event::LevelComplete, 731),
            ],
            deaths: 2,
            player_x: 1850,
            state: EndState::StageClear,
            oblaten: 12,
            score: 4600,
            lives: 2,
        }
    }

    fn check(clause: &str, o: &Outcome) -> Result<(), String> {
        let e = Expect::parse_clauses(clause).unwrap();
        assert_eq!(e.len(), 1);
        e[0].check(o)
    }

    #[test]
    fn expectations_pass() {
        let o = outcome();
        for c in [
            "complete",
            "complete frames=731..731",
            "complete frames=700..800",
            "deaths=2",
            "deaths<=2",
            "dies_at frames=250..350",
            "player_x>=1850",
            "state=stage_clear",
            "no_event=Respawned",
            "oblaten>=12",
            "score>=4600",
            "lives=2",
        ] {
            assert_eq!(check(c, &o), Ok(()), "{c}");
        }
    }

    #[test]
    fn expectations_fail_with_reasons() {
        let mut o = outcome();
        let f = |c: &str, o: &Outcome| check(c, o).unwrap_err();
        assert_eq!(
            f("complete frames=700..720", &o),
            "expected complete in 700..720, completed at 731"
        );
        assert_eq!(
            f("deaths=0", &o),
            "expected deaths=0, got 2 (deaths at frames 100, 300)"
        );
        assert_eq!(
            f("deaths<=1", &o),
            "expected deaths<=1, got 2 (deaths at frames 100, 300)"
        );
        assert_eq!(
            f("dies_at frames=101..299", &o),
            "expected a death in 101..299, (deaths at frames 100, 300)"
        );
        assert_eq!(
            f("player_x>=1851", &o),
            "expected player_x>=1851 at the end, got player_x=1850"
        );
        assert_eq!(
            f("state=playing", &o),
            "expected state=playing at the end, got stage_clear"
        );
        assert_eq!(
            f("no_event=Died", &o),
            "expected no_event=Died, first at frame 100"
        );
        assert_eq!(
            f("oblaten>=13", &o),
            "expected oblaten>=13 at the end, got oblaten=12"
        );
        assert_eq!(
            f("score>=5000", &o),
            "expected score>=5000 at the end, got score=4600"
        );
        assert_eq!(f("lives=3", &o), "expected lives=3 at the end, got lives=2");
        assert_eq!(
            f("state=game_over", &o),
            "expected state=game_over at the end, got stage_clear"
        );
        o.completed_at = None;
        o.state = EndState::Playing;
        o.deaths_at.clear();
        assert_eq!(
            f("complete frames=1..2", &o),
            "expected complete frames=1..2, never completed (state=playing, player_x=1850 after 800 frames)"
        );
        assert_eq!(
            f("dies_at frames=1..2", &o),
            "expected a death in 1..2, (no deaths)"
        );
    }

    /// A replay that runs right into a hazard on a tiny level.
    #[test]
    fn run_records_the_outcome_and_every_frame() {
        let level = Level::parse(
            "[meta]\nid = \"t\"\ntime_limit = 100\n[legend]\n\"#\" = \"solid\"\n\"^\" = \"hazard\"\n\
             \"P\" = { entity = \"player_start\" }\n[map]\ngrid = '''\n\
             ..........\n.P........\n###^^#####\n'''\n",
        )
        .unwrap();
        let tuning = Tuning::parse(include_str!("../../../content/tuning.toml")).unwrap();
        let r = Replay::parse(
            "level = t\nsource = handwritten\nexpect = dies_at frames=1..60 deaths=1 no_event=LevelComplete\n---\nR 60\n",
        )
        .unwrap();
        let mut seen = Vec::new();
        let o = r.run(&level, &tuning, |g| seen.push((g.frame(), g.state_hash())));
        assert_eq!(seen.len(), 61, "frame 0 plus one per step");
        assert_eq!(seen[60].0, 60);
        assert_eq!(o.frames, 60);
        assert_eq!(o.deaths, 1);
        assert_eq!(r.check(&o), Vec::<String>::new());
        let mut again = Vec::new();
        r.run(&level, &tuning, |g| again.push((g.frame(), g.state_hash())));
        assert_eq!(seen, again, "deterministic");
    }
}
