//! `--record`: collects the inputs of a played session and turns them into a
//! replay file (native only; pure apart from [`save`]).
//!
//! The replay gets an `expect` line derived from what happened, so it passes
//! `cargo xtask replay` right away: `complete frames=N..N` if the stage was
//! completed, else `state=<mode> player_x>=<x>`; always `deaths=<n>`.

use std::path::Path;

use jnr_core::replay::{Expect, FrameRange, Outcome, Source};
use jnr_core::{Game, InputFrame, Replay};

/// Records one input per simulation step plus what happened.
pub struct Recorder {
    level: String,
    seed: u64,
    /// Run-length encoded: (input, frames held).
    inputs: Vec<(InputFrame, u32)>,
    outcome: Outcome,
}

impl Recorder {
    /// Starts recording a game that has not stepped yet; `seed` is the seed
    /// it was created with.
    pub fn new(game: &Game, seed: u64) -> Recorder {
        Recorder {
            level: game.level().meta.id.clone(),
            seed,
            inputs: Vec::new(),
            outcome: Outcome::new(game),
        }
    }

    /// Records one step: `input` was fed to `game.step`, `game` is the state after it.
    pub fn record(&mut self, input: InputFrame, game: &Game) {
        match self.inputs.last_mut() {
            Some((last, n)) if *last == input => *n += 1,
            _ => self.inputs.push((input, 1)),
        }
        self.outcome.record(game, game.events());
    }

    /// The recording as a replay (`source = human`, expectations derived).
    pub fn replay(&self) -> Replay {
        Replay {
            level: self.level.clone(),
            seed: self.seed,
            source: Source::Human,
            expects: expects_for(&self.outcome),
            shots: Vec::new(),
            inputs: self.inputs.clone(),
        }
    }
}

/// The expectations a recording with this outcome passes.
pub fn expects_for(o: &Outcome) -> Vec<Expect> {
    let mut out = match o.completed_at {
        Some(f) => vec![Expect::Complete(Some(FrameRange { first: f, last: f }))],
        None => vec![Expect::State(o.state), Expect::PlayerXAtLeast(o.player_x)],
    };
    out.push(Expect::Deaths(o.deaths));
    out
}

/// Writes the replay to `path`, creating missing parent directories.
pub fn save(replay: &Replay, path: &Path) -> Result<(), String> {
    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    std::fs::write(path, replay.format()).map_err(|e| format!("{}: {e}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::options::{Options, level_source};
    use jnr_core::replay::EndState;
    use jnr_core::{Level, Tuning};

    fn level() -> Level {
        Level::parse(level_source("testbed").expect("embedded")).expect("parses")
    }

    fn tuning() -> Tuning {
        Tuning::parse(crate::TUNING_SRC).expect("parses")
    }

    /// Plays `inputs` like the app does and returns the recorded replay.
    fn record(inputs: &[InputFrame]) -> Replay {
        let opts = Options::default();
        let mut game = crate::new_game(&opts);
        let mut rec = Recorder::new(&game, opts.seed);
        for &input in inputs {
            game.step(input);
            rec.record(input, &game);
        }
        rec.replay()
    }

    /// The recorded replay, written and parsed back, passes its own expectations.
    fn assert_round_trip_passes(replay: &Replay) {
        let parsed = Replay::parse(&replay.format()).expect("recorded replay parses");
        assert_eq!(&parsed, replay);
        let outcome = parsed.run(&level(), &tuning(), |_| {});
        assert_eq!(parsed.check(&outcome), Vec::<String>::new());
    }

    #[test]
    fn inputs_are_run_length_encoded() {
        let r = InputFrame::RIGHT;
        let rj = r.with(InputFrame::JUMP);
        let replay = record(&[r, r, r, rj, rj, InputFrame::NONE, r]);
        assert_eq!(
            replay.inputs,
            vec![(r, 3), (rj, 2), (InputFrame::NONE, 1), (r, 1)]
        );
        assert_eq!(replay.level, "testbed");
        assert_eq!(replay.seed, 1);
        assert_eq!(replay.source, Source::Human);
        assert!(replay.shots.is_empty());
    }

    #[test]
    fn unfinished_run_expects_state_position_and_deaths() {
        let mut inputs = vec![InputFrame::RIGHT; 40];
        inputs.extend([InputFrame::RIGHT.with(InputFrame::JUMP); 10]);
        let replay = record(&inputs);
        let [
            Expect::State(EndState::Playing),
            Expect::PlayerXAtLeast(x),
            Expect::Deaths(0),
        ] = replay.expects[..]
        else {
            panic!("unexpected expects {:?}", replay.expects);
        };
        assert!(x > 0, "player moved right, x={x}");
        assert_round_trip_passes(&replay);
    }

    #[test]
    fn completed_run_expects_the_exact_finish_frame() {
        let route = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/replays/testbed_route.replay"
        ))
        .expect("testbed route replay");
        let route = Replay::parse(&route).expect("parses");
        let inputs: Vec<InputFrame> = route.frames().collect();
        let replay = record(&inputs);
        let first = FrameRange {
            first: 731,
            last: 731,
        };
        assert_eq!(
            replay.expects,
            vec![Expect::Complete(Some(first)), Expect::Deaths(0)]
        );
        assert_eq!(replay.inputs, route.inputs);
        assert_round_trip_passes(&replay);
    }

    #[test]
    fn empty_recording_is_a_valid_replay() {
        let replay = record(&[]);
        assert!(replay.inputs.is_empty());
        assert_round_trip_passes(&replay);
    }

    #[test]
    fn save_creates_missing_directories() {
        let dir = std::env::temp_dir().join(format!("jnr-app-record-{}", std::process::id()));
        let path = dir.join("a/b/run.replay");
        let replay = record(&[InputFrame::RIGHT; 5]);
        save(&replay, &path).expect("saved");
        let text = std::fs::read_to_string(&path).expect("written");
        assert_eq!(Replay::parse(&text).expect("parses"), replay);
        std::fs::remove_dir_all(&dir).expect("cleanup");
    }
}
