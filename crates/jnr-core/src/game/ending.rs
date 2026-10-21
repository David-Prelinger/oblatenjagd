//! The ending after the last station (`docs/game-design.md` → "Finale and
//! ending", D-107): `Mode::Ending`, a phase machine stepped once per frame
//! with START as its only input (a new press; JUMP is tracked by
//! `Game::step` as in every non-play mode, D-025):
//!
//! 1. [`EndingPhase::Map`] (`ending.map_frames`): the route map with every
//!    station done and Resi on the last dot,
//! 2. [`EndingPhase::Scene`] (`ending.scene_frames`): the café scene,
//! 3. [`EndingPhase::Bill`] (until START from `ending.bill_start_frames`,
//!    at most `ending.bill_frames`): the run's tally as a café bill,
//! 4. [`EndingPhase::Credits`] (`ending.credits_pages` pages of
//!    `ending.credits_page_frames`): START skips a page,
//! 5. [`EndingPhase::Ende`] (until START from `ending.end_start_frames`,
//!    at most `ending.end_frames`),
//!
//! then the title with the run reset and the session's „durchgespielt"
//! mark set (`Game::cleared`). START is ignored for the first
//! `ending.start_ignore_frames` frames of the Map, the Scene and every
//! credits page, and for the first `ending.end_start_frames` frames of
//! that marked title (D-111). Everything has fixed frame counts, so
//! `start = title` replays run through it.

use super::{Game, Mode};
use crate::hash::Fnv;

/// Lines of the bill after its title (Punkte, Oblaten gesammelt, Leben
/// übrig, Trinkgeld, the thanks); the renderer shows one more every
/// `ending.bill_line_frames` frames.
pub const BILL_LINES: u16 = 5;

/// The phases of the ending, in order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EndingPhase {
    /// The route map: every station done, the whole route walked.
    Map,
    /// The café scene (`ending_cafe`) and three lines.
    Scene,
    /// The bill: the run's score, Oblaten total and lives.
    Bill,
    /// The credits pages (`Ending::page`).
    Credits,
    /// „Ende".
    Ende,
}

/// The state of the ending (`Game::ending`, while `Mode::Ending`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ending {
    /// The current phase.
    pub phase: EndingPhase,
    /// Frames spent in the current phase, or on the current credits page
    /// (0 on the frame it began).
    pub timer: u16,
    /// The credits page (0-based; 0 outside the credits).
    pub page: u8,
}

impl Ending {
    /// The ending's first frame: the Map phase.
    pub const START: Ending = Ending {
        phase: EndingPhase::Map,
        timer: 0,
        page: 0,
    };

    pub(super) fn hash(&self, h: &mut Fnv) {
        h.u8(match self.phase {
            EndingPhase::Map => 0,
            EndingPhase::Scene => 1,
            EndingPhase::Bill => 2,
            EndingPhase::Credits => 3,
            EndingPhase::Ende => 4,
        });
        h.u16(self.timer);
        h.u8(self.page);
    }
}

impl Game {
    /// After the stage-clear texts: the ending, if this is the real flow
    /// of a run on the journey's last station (D-107). Returns whether it
    /// started.
    pub(super) fn enter_ending(&mut self) -> bool {
        let last = usize::from(self.stage) + 1 == crate::route::STATIONS.len();
        if self.route.is_none() || self.start != super::Start::Title || !last {
            return false;
        }
        self.clear = None;
        self.ending = Some(Ending::START);
        self.mode = Mode::Ending;
        true
    }

    /// One frame of the ending; `start_pressed` is a new START press.
    pub(super) fn step_ending(&mut self, start_pressed: bool) {
        let Some(mut e) = self.ending else {
            return;
        };
        let t = self.tuning.ending;
        e.timer = e.timer.saturating_add(1);
        let (frames, delay) = match e.phase {
            EndingPhase::Map => (t.map_frames, t.start_ignore_frames),
            EndingPhase::Scene => (t.scene_frames, t.start_ignore_frames),
            EndingPhase::Bill => (t.bill_frames, t.bill_start_frames),
            EndingPhase::Credits => (t.credits_page_frames, t.start_ignore_frames),
            EndingPhase::Ende => (t.end_frames, t.end_start_frames),
        };
        // Like the game over's prompt: a press counts once the phase (or
        // page) is `delay` frames old.
        let skip = start_pressed && e.timer >= delay;
        if skip || e.timer >= frames {
            match e.phase {
                EndingPhase::Map => e.phase = EndingPhase::Scene,
                EndingPhase::Scene => e.phase = EndingPhase::Bill,
                EndingPhase::Bill => e.phase = EndingPhase::Credits,
                EndingPhase::Credits if e.page + 1 < t.credits_pages => e.page += 1,
                EndingPhase::Credits => {
                    e.phase = EndingPhase::Ende;
                    e.page = 0;
                }
                EndingPhase::Ende => {
                    self.ending = None;
                    // Set only here; it survives `end_run` (D-107).
                    self.cleared = true;
                    self.end_run(true);
                    return;
                }
            }
            e.timer = 0;
        }
        self.ending = Some(e);
    }
}
