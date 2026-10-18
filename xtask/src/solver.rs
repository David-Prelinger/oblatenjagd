//! The solver bot (L4): `cargo xtask solve` and the `solver` verify step.
//!
//! **Search.** A frame-synchronous beam search over the real `jnr-core`
//! simulation: every search node is a cloned [`Game`] plus the macro-actions
//! that led to it. Nodes are kept in buckets by their frame and the buckets
//! are processed in frame order. Each bucket is ranked (furthest right
//! first, then fewer jumps; remaining ties keep their insertion order,
//! which is deterministic), de-duplicated by a coarse state key (see
//! [`StateKey`]) so the beam stays diverse, cut to the beam width plus a
//! grounded reserve (see [`select_beam`]), and every survivor is expanded
//! with every macro-action in [`MACROS`]. A macro that makes the player die is pruned
//! and counted as a "search death"; a macro that touches the goal stops at
//! that frame. The search ends at the first bucket at or after the earliest
//! completion found, so the route is the earliest completion the beam sees.
//! It fails when no node survives, after the frame budget (3× the level
//! width in pixels) or after the wall-clock limit (default 60 s).
//! A finished route is cleaned up ([`clean_route`]: JUMP removed where it
//! changes nothing) before it is measured and written.
//!
//! **Determinism.** The route never depends on the thread count (nodes are
//! expanded on scoped threads in chunks, results joined in node order) or
//! on the clock: the wall-clock limit is only checked while no completion
//! has been found. So whenever the search finishes, the same level and
//! tuning give the same route. Pass/fail itself can depend on machine speed
//! if a search runs into the limit, which is why the limit (60 s) is
//! generous: the testbed takes about 0.6 s.
//!
//! **Difficulty.** For every jump of the route (a JUMP press followed by a
//! `Jumped`), the whole held-JUMP segment is shifted by d ∈ [-12, +12]
//! frames with the rest of the inputs unchanged (padded with the last input
//! past the end of the route). Its checkpoint is the first landing after
//! that `Jumped` + 2 s, capped at the route's end. A shift succeeds if the
//! player survives up to the next jump's checkpoint (or the end of the
//! route) and, at its own checkpoint, is at most one tile behind the
//! original (or has completed the level). The jump window is the
//! contiguous successful range around d = 0. Each side records why it
//! stopped ([`Bound`]): a failed shift, the route (the shift would start
//! before frame 0 or touch a neighbouring JUMP hold) or ±12. Windows with
//! a route-limited side are only lower bounds and never count as the
//! narrowest.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;
use std::time::{Duration, Instant};

use jnr_core::replay::{Expect, FrameRange, Shot, Source};
use jnr_core::{Event, Game, InputFrame, Level, Mode, Replay, TICKS_PER_SECOND, Tuning};
use jnr_render::RenderContext;
use serde_json::{Value, json};

use crate::{content, replay};

pub const HELP: &str = "\
cargo xtask solve <level-id|--all> [--write] [--shots] [--beam N] [--time-limit S]
    Beam search over the real simulation until the level is complete.
    Prints the result and writes the difficulty report
      -> reports/solver/<level>.{md,json}
      -> preview/solver/<level>_narrowest.png (the narrowest jump)
      -> preview/solver/<level>_stuck.png     (furthest state, if it fails)
    --write          write tests/replays/<level>_solver.replay
                     (expect = complete frames=N..N deaths=0)
    --shots          with --write: add 3 shots spread over the route; they
                     need goldens (`cargo xtask golden --accept`, only after
                     art-director APPROVED), so they are off by default
    --beam N         beam width (default 64)
    --time-limit S   wall-clock limit in seconds (default 60)

Reports go to $CARGO_TARGET_DIR/reports/solver, images to
$CARGO_TARGET_DIR/preview/solver (or ./target/...).
";

// ------------------------------------------------------------- action set

const NONE: InputFrame = InputFrame::NONE;
const R: InputFrame = InputFrame::RIGHT;
const RB: InputFrame = R.with(InputFrame::RUN);
const RJ: InputFrame = R.with(InputFrame::JUMP);
const RBJ: InputFrame = RB.with(InputFrame::JUMP);
const L: InputFrame = InputFrame::LEFT;
const LB: InputFrame = L.with(InputFrame::RUN);
const LBJ: InputFrame = LB.with(InputFrame::JUMP);
const DJ: InputFrame = InputFrame::DOWN.with(InputFrame::JUMP);

/// One macro-action: inputs held for a number of frames, in order.
type Macro = &'static [(InputFrame, u32)];

/// The solver's action set. Order matters only for ties (earlier wins), so
/// plain running comes before jumping and the route has no needless jumps.
/// Every jump macro ends with two frames without JUMP, so the next jump
/// macro is always a fresh press.
///
/// - run right: `RB 4`, `RB 8`, `RB 16`; walk right: `R 8`
/// - wait / brake (also air control): `- 4`, `- 16`
/// - back off: `L 4`, `LB 8`
/// - running jump, JUMP held 1 (tap), 6, 12, 20 or 36 (full) frames, then `RB 2`
/// - walking jump, JUMP held 1, 12 or 36 frames, then `R 2`
/// - jump left: `LBJ 12` / `LBJ 36`, then `LB 2`
/// - drop through a one-way platform (straight jump elsewhere): `DJ 1`, `- 3`
pub const MACROS: &[Macro] = &[
    &[(RB, 4)],
    &[(RB, 8)],
    &[(RB, 16)],
    &[(R, 8)],
    &[(NONE, 4)],
    &[(NONE, 16)],
    &[(L, 4)],
    &[(LB, 8)],
    &[(RBJ, 1), (RB, 2)],
    &[(RBJ, 6), (RB, 2)],
    &[(RBJ, 12), (RB, 2)],
    &[(RBJ, 20), (RB, 2)],
    &[(RBJ, 36), (RB, 2)],
    &[(RJ, 1), (R, 2)],
    &[(RJ, 12), (R, 2)],
    &[(RJ, 36), (R, 2)],
    &[(LBJ, 12), (LB, 2)],
    &[(LBJ, 36), (LB, 2)],
    &[(DJ, 1), (NONE, 3)],
];

/// Default beam width (nodes kept per frame bucket).
pub const DEFAULT_BEAM: usize = 64;
/// Default wall-clock limit.
pub const DEFAULT_TIME_LIMIT: Duration = Duration::from_secs(60);
/// Frame budget = this × level width in pixels.
const BUDGET_PER_PX: u32 = 3;
/// Most threads used to expand a bucket.
const MAX_THREADS: usize = 8;
/// Buckets smaller than this are expanded on the calling thread.
const PARALLEL_MIN_NODES: usize = 16;

/// Jump shifts tested for the window: d ∈ [-MAX_SHIFT, +MAX_SHIFT].
pub const MAX_SHIFT: i32 = 12;
/// The progress checkpoint is this many frames after the original landing.
const CHECKPOINT_AFTER_LANDING: u32 = 2 * TICKS_PER_SECOND;
/// A shifted jump may end up this many pixels behind the original.
const PROGRESS_TOLERANCE_PX: i32 = 16;

// ------------------------------------------------------------------ search

/// Search settings.
#[derive(Clone, Copy, Debug)]
pub struct SolveOptions {
    pub beam_width: usize,
    /// No node goes beyond this frame.
    pub frame_budget: u32,
    /// Aborts the search (as a failure) after this much wall-clock time.
    pub time_limit: Duration,
    /// Threads for expanding nodes; the result does not depend on it.
    pub threads: usize,
}

impl SolveOptions {
    /// The defaults for `level`: beam 64, budget 3× its width in pixels,
    /// 60 s, up to 8 threads.
    pub fn for_level(level: &Level) -> SolveOptions {
        let threads = std::thread::available_parallelism().map_or(1, |n| n.get().min(MAX_THREADS));
        SolveOptions {
            beam_width: DEFAULT_BEAM,
            frame_budget: BUDGET_PER_PX
                * (level.width.max(1) as u32)
                * jnr_core::level::TILE_PX as u32,
            time_limit: DEFAULT_TIME_LIMIT,
            threads,
        }
    }
}

/// The macro-actions that led to a node, as a shared linked list (children
/// share their parent's trail instead of copying the route).
struct Trail {
    parent: Option<Arc<Trail>>,
    action: usize,
    /// Frames of the macro actually run (less than its length when the
    /// goal was touched inside it).
    frames: u32,
}

/// A search state.
#[derive(Clone)]
pub struct Node {
    pub game: Game,
    /// `Jumped` events so far (fewer wins ties).
    jumps: u32,
    trail: Option<Arc<Trail>>,
}

impl Node {
    /// The inputs from the start to this node, one per frame.
    pub fn inputs(&self) -> Vec<InputFrame> {
        let mut steps = Vec::new();
        let mut t = self.trail.as_ref();
        while let Some(s) = t {
            steps.push((s.action, s.frames));
            t = s.parent.as_ref();
        }
        let mut out = Vec::new();
        for (action, frames) in steps.into_iter().rev() {
            let all = MACROS[action]
                .iter()
                .flat_map(|&(i, n)| std::iter::repeat_n(i, n as usize));
            out.extend(all.take(frames as usize));
        }
        out
    }

    fn x(&self) -> i32 {
        self.game.player().x.raw()
    }
}

/// Coarse state for de-duplication within a frame bucket: position in 4 px
/// cells, speed in 1/4 px (x) and 1/2 px (y) per frame, grounded or not.
/// Of several nodes with the same key only the best-ranked one is kept.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct StateKey(i32, i32, i32, i32, bool);

impl StateKey {
    fn of(game: &Game) -> StateKey {
        let p = game.player();
        StateKey(
            p.x.raw() >> 10,
            p.y.raw() >> 10,
            p.vx.raw() >> 6,
            p.vy.raw() >> 7,
            p.on_ground,
        )
    }
}

/// Why the search stopped.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StopReason {
    Complete,
    /// Every surviving node ran into the frame budget.
    FrameBudget,
    /// Every node died.
    NoSurvivors,
    TimeLimit,
}

impl StopReason {
    pub fn as_str(self) -> &'static str {
        match self {
            StopReason::Complete => "complete",
            StopReason::FrameBudget => "frame budget exhausted",
            StopReason::NoSurvivors => "every branch died",
            StopReason::TimeLimit => "wall-clock limit reached",
        }
    }
}

/// The result of [`search`].
pub struct Search {
    pub reason: StopReason,
    /// The completing node, or the furthest-right node standing on the
    /// ground (airborne nodes may already be doomed).
    pub best: Node,
    /// Macro expansions that ended in a death.
    pub search_deaths: u64,
    /// Nodes expanded.
    pub expanded: u64,
    pub elapsed: Duration,
}

impl Search {
    pub fn complete(&self) -> bool {
        self.reason == StopReason::Complete
    }
}

/// Runs the beam search on `level`.
pub fn search(level: &Level, tuning: &Tuning, opts: &SolveOptions) -> Search {
    let started = Instant::now();
    let root = Node {
        game: Game::new(level, tuning, 1),
        jumps: 0,
        trail: None,
    };
    let mut best = root.clone();
    let mut completed: Option<Node> = None;
    let mut buckets: BTreeMap<u32, Vec<Node>> = BTreeMap::new();
    buckets.insert(0, vec![root]);
    let (mut search_deaths, mut expanded) = (0u64, 0u64);
    let mut hit_budget = false;
    let mut timed_out = false;

    while let Some((frame, mut nodes)) = buckets.pop_first() {
        if completed.as_ref().is_some_and(|c| c.game.frame() <= frame) {
            break;
        }
        // Once a completion is found, finish without the clock: the rest is
        // bounded by the longest macro, and the route must not depend on
        // machine speed.
        if completed.is_none() && started.elapsed() >= opts.time_limit {
            timed_out = true;
            break;
        }
        // Furthest right first, then fewer jumps (no needless hops, so jump
        // windows are not coupled); the stable sort keeps insertion order on ties.
        nodes.sort_by_key(|n| (std::cmp::Reverse(n.x()), n.jumps));
        let mut seen = BTreeSet::new();
        nodes.retain(|n| seen.insert(StateKey::of(&n.game)));
        let nodes = select_beam(nodes, opts.beam_width);
        expanded += nodes.len() as u64;

        for children in expand_all(&nodes, opts.threads) {
            for child in children {
                let Some(node) = child else {
                    search_deaths += 1;
                    continue;
                };
                let f = node.game.frame();
                if node.game.mode() == Mode::StageClear {
                    if completed
                        .as_ref()
                        .is_none_or(|c| (f, node.jumps) < (c.game.frame(), c.jumps))
                    {
                        completed = Some(node);
                    }
                    continue;
                }
                if node.game.player().on_ground
                    && (node.x() > best.x() || (node.x() == best.x() && f < best.game.frame()))
                {
                    best = node.clone();
                }
                if f > opts.frame_budget {
                    hit_budget = true;
                    continue;
                }
                buckets.entry(f).or_default().push(node);
            }
        }
    }

    let reason = match (&completed, timed_out, hit_budget) {
        (Some(_), _, _) => StopReason::Complete,
        (None, true, _) => StopReason::TimeLimit,
        (None, false, true) => StopReason::FrameBudget,
        (None, false, false) => StopReason::NoSurvivors,
    };
    Search {
        reason,
        best: completed.unwrap_or(best),
        search_deaths,
        expanded,
        elapsed: started.elapsed(),
    }
}

/// The beam: the first `width` of the ranked `nodes`, plus the best
/// grounded nodes after them until at least a quarter of `width` is
/// grounded. Airborne nodes far right are often already doomed; the
/// grounded reserve keeps safe states alive (e.g. waiting at an edge).
fn select_beam(mut nodes: Vec<Node>, width: usize) -> Vec<Node> {
    let width = width.max(1);
    if nodes.len() <= width {
        return nodes;
    }
    let rest = nodes.split_off(width);
    let grounded = nodes.iter().filter(|n| n.game.player().on_ground).count();
    let missing = (width / 4).saturating_sub(grounded);
    nodes.extend(
        rest.into_iter()
            .filter(|n| n.game.player().on_ground)
            .take(missing),
    );
    nodes
}

/// Expands every node (on up to `threads` threads); the result is in node order.
fn expand_all(nodes: &[Node], threads: usize) -> Vec<Vec<Option<Node>>> {
    if threads <= 1 || nodes.len() < PARALLEL_MIN_NODES {
        return nodes.iter().map(expand).collect();
    }
    let chunk = nodes.len().div_ceil(threads);
    std::thread::scope(|s| {
        let handles: Vec<_> = nodes
            .chunks(chunk)
            .map(|c| s.spawn(move || c.iter().map(expand).collect::<Vec<_>>()))
            .collect();
        handles
            .into_iter()
            .flat_map(|h| h.join().expect("solver thread panicked"))
            .collect()
    })
}

fn expand(node: &Node) -> Vec<Option<Node>> {
    (0..MACROS.len()).map(|a| run_macro(node, a)).collect()
}

/// Runs one macro-action from `node`: `None` if the player dies, else the
/// node at its end (or at the goal).
fn run_macro(node: &Node, action: usize) -> Option<Node> {
    let mut game = node.game.clone();
    let mut frames = 0;
    let mut jumps = node.jumps;
    'outer: for &(input, n) in MACROS[action] {
        for _ in 0..n {
            game.step(input);
            frames += 1;
            jumps += game.events().contains(&Event::Jumped) as u32;
            if game.events().contains(&Event::Died) {
                return None;
            }
            if game.mode() == Mode::StageClear {
                break 'outer;
            }
        }
    }
    Some(Node {
        game,
        jumps,
        trail: Some(Arc::new(Trail {
            parent: node.trail.clone(),
            action,
            frames,
        })),
    })
}

// --------------------------------------------------------------- difficulty

/// What stopped a jump window from growing on one side.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bound {
    /// The next shift failed in the simulation: a real limit of the level.
    Fail,
    /// The next shift is impossible because of the route itself (it would
    /// start before frame 0 or touch a neighbouring JUMP hold). Says nothing
    /// about the level, so such a side never counts toward the narrowest.
    Route,
    /// Reached ±[`MAX_SHIFT`].
    Max,
}

impl Bound {
    pub fn as_str(self) -> &'static str {
        match self {
            Bound::Fail => "fail",
            Bound::Route => "route",
            Bound::Max => "max",
        }
    }
}

/// The result of trying one shift.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Shifted {
    Ok,
    Fail,
    /// The shift cannot be made on this route.
    Route,
}

/// One jump of the route and its timing window.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JumpWindow {
    /// Input index of the JUMP press (the press happens in step
    /// `index + 1`, i.e. at `Game::frame() == index + 1`).
    pub index: usize,
    /// Frames JUMP is held.
    pub hold: u32,
    /// Player hitbox left edge (level px) just before the press.
    pub x: i32,
    /// Smallest and largest successful shift (lo ≤ 0 ≤ hi).
    pub lo: i32,
    pub hi: i32,
    /// Why the window ends below `lo` / above `hi`.
    pub lo_bound: Bound,
    pub hi_bound: Bound,
}

impl JumpWindow {
    /// The `Game::frame()` of the press step (replay frame numbering).
    pub fn frame(&self) -> u32 {
        self.index as u32 + 1
    }
    /// Number of frames in the window.
    pub fn size(&self) -> u32 {
        (self.hi - self.lo + 1) as u32
    }
    /// True if a side is limited only by the route (the window is then a
    /// lower bound and is left out of [`narrowest`]).
    pub fn route_limited(&self) -> bool {
        self.lo_bound == Bound::Route || self.hi_bound == Bound::Route
    }
}

/// Per-frame record of a run: x after each frame (index 0 = start) and events.
struct Trace {
    x: Vec<i32>,
    events: Vec<(u32, Event)>,
}

fn trace(level: &Level, tuning: &Tuning, inputs: &[InputFrame]) -> Trace {
    let mut g = Game::new(level, tuning, 1);
    let mut t = Trace {
        x: vec![g.player().x.to_px()],
        events: Vec::new(),
    };
    for &i in inputs {
        g.step(i);
        t.events.extend(g.events().iter().map(|&e| (g.frame(), e)));
        t.x.push(g.player().x.to_px());
    }
    t
}

/// Runs `inputs` up to frame `until` (or their end): `None` if the player
/// died on the way, else the x at frame `check` (≤ `until`) and whether the
/// level was completed by then.
fn progress(
    level: &Level,
    tuning: &Tuning,
    inputs: &[InputFrame],
    check: u32,
    until: u32,
) -> Option<(i32, bool)> {
    let mut g = Game::new(level, tuning, 1);
    let mut at_check = (g.player().x.to_px(), false);
    for &i in inputs.iter().take(until as usize) {
        if g.step(i).contains(&Event::Died) {
            return None;
        }
        if g.frame() <= check {
            at_check = (g.player().x.to_px(), g.mode() == Mode::StageClear);
        }
    }
    Some(at_check)
}

/// The JUMP-held segments `[start, end)` of `inputs`.
fn jump_holds(inputs: &[InputFrame]) -> Vec<(usize, usize)> {
    let mut holds = Vec::new();
    let mut i = 0;
    while i < inputs.len() {
        if inputs[i].jump() {
            let s = i;
            while i < inputs.len() && inputs[i].jump() {
                i += 1;
            }
            holds.push((s, i));
        } else {
            i += 1;
        }
    }
    holds
}

/// `inputs` with the JUMP hold `[s, e)` moved by `d` frames. Past the end
/// of the route the sequence is padded with its last input (without JUMP).
/// `None` if the shift would start before frame 0 or touch another JUMP
/// hold (no clean press).
fn shift_hold(inputs: &[InputFrame], s: usize, e: usize, d: i32) -> Option<Vec<InputFrame>> {
    let ns = s as i64 + d as i64;
    if ns < 0 {
        return None;
    }
    let (ns, ne) = (ns as usize, (e as i64 + d as i64) as usize);
    let mut out = inputs.to_vec();
    for f in &mut out[s..e] {
        *f = f.without(InputFrame::JUMP);
    }
    let pad = out
        .last()
        .map_or(InputFrame::NONE, |f| f.without(InputFrame::JUMP));
    if out.len() < ne {
        out.resize(ne, pad);
    }
    if (ns > 0 && out[ns - 1].jump())
        || (ne < out.len() && out[ne].jump())
        || out[ns..ne].iter().any(|f| f.jump())
    {
        return None;
    }
    for f in &mut out[ns..ne] {
        *f = f.with(InputFrame::JUMP);
    }
    Some(out)
}

/// Grows the range `lo..=hi` around 0 while `try_shift(d)` succeeds, within
/// ±MAX_SHIFT; returns `(lo, why lo stopped, hi, why hi stopped)`.
fn window_range(mut try_shift: impl FnMut(i32) -> Shifted) -> (i32, Bound, i32, Bound) {
    let mut side = |step: i32| -> (i32, Bound) {
        let mut d: i32 = 0;
        loop {
            if d.abs() == MAX_SHIFT {
                return (d, Bound::Max);
            }
            match try_shift(d + step) {
                Shifted::Ok => d += step,
                Shifted::Fail => return (d, Bound::Fail),
                Shifted::Route => return (d, Bound::Route),
            }
        }
    };
    let (lo, lo_bound) = side(-1);
    let (hi, hi_bound) = side(1);
    (lo, lo_bound, hi, hi_bound)
}

/// A jump of the route before measuring: its JUMP hold and checkpoint.
struct JumpAt {
    hold: (usize, usize),
    check: u32,
}

/// The jump windows of a route (see the module docs).
pub fn jump_windows(level: &Level, tuning: &Tuning, inputs: &[InputFrame]) -> Vec<JumpWindow> {
    let orig = trace(level, tuning, inputs);
    let total = inputs.len() as u32;
    let buffer = tuning.player.jump_buffer_frames as u32;
    let first = |ev: Event, from: u32, to: u32| {
        orig.events
            .iter()
            .find(|&&(f, e)| e == ev && f >= from && f <= to)
            .map(|&(f, _)| f)
    };
    // Presses that make the player jump; checkpoint = the first landing
    // after that `Jumped` + 2 s, capped at the end of the route.
    let jumps: Vec<JumpAt> = jump_holds(inputs)
        .into_iter()
        .filter_map(|(s, e)| {
            let press = s as u32 + 1;
            let jumped = first(Event::Jumped, press, press + buffer)?;
            let landing = first(Event::Landed, jumped + 1, total).unwrap_or(total);
            Some(JumpAt {
                hold: (s, e),
                check: landing.saturating_add(CHECKPOINT_AFTER_LANDING).min(total),
            })
        })
        .collect();

    let mut out = Vec::new();
    for (k, jump) in jumps.iter().enumerate() {
        let (s, e) = jump.hold;
        let check = jump.check;
        // Survive up to the next jump's checkpoint (or the end of the
        // route): a shift that ruins a later jump fails too.
        let until = jumps.get(k + 1).map_or(total, |n| n.check).max(check);
        let x_ref = orig.x[check as usize];
        let (lo, lo_bound, hi, hi_bound) = window_range(|d| match shift_hold(inputs, s, e, d) {
            None => Shifted::Route,
            Some(shifted) => match progress(level, tuning, &shifted, check, until) {
                Some((x, done)) if done || x >= x_ref - PROGRESS_TOLERANCE_PX => Shifted::Ok,
                _ => Shifted::Fail,
            },
        });
        out.push(JumpWindow {
            index: s,
            hold: (e - s) as u32,
            x: orig.x[s],
            lo,
            hi,
            lo_bound,
            hi_bound,
        });
    }
    out
}

/// The narrowest window (earliest on ties) among the jumps whose window is
/// not limited by the route.
pub fn narrowest(windows: &[JumpWindow]) -> Option<&JumpWindow> {
    windows
        .iter()
        .filter(|w| !w.route_limited())
        .min_by_key(|w| (w.size(), w.index))
}
// ------------------------------------------------------------------ solving

/// A solved (or failed) level with its difficulty numbers.
pub struct Solution {
    pub level: String,
    pub search: Search,
    /// The route (complete) or the inputs to the furthest state (failed).
    pub inputs: Vec<InputFrame>,
    /// Empty when the search failed.
    pub windows: Vec<JumpWindow>,
}

impl Solution {
    /// Frame of `LevelComplete` (= route length) when complete.
    pub fn frames(&self) -> u32 {
        self.inputs.len() as u32
    }

    /// "narrowest jump window 7 frames (frame 372, x=929, bounds fail/fail)".
    fn narrowest_text(&self) -> String {
        match narrowest(&self.windows) {
            Some(w) => format!(
                "narrowest jump window {} frames (frame {}, x={}, bounds {}/{})",
                w.size(),
                w.frame(),
                w.x,
                w.lo_bound.as_str(),
                w.hi_bound.as_str()
            ),
            None if self.windows.is_empty() => "no jumps".to_owned(),
            None => "every jump window is limited by the route".to_owned(),
        }
    }

    /// Short note for the verify step: "testbed: 731 frames, 5 jumps, …".
    pub fn note(&self) -> String {
        if self.search.complete() {
            format!(
                "{}: {} frames, {} jumps, {}",
                self.level,
                self.frames(),
                self.windows.len(),
                self.narrowest_text()
            )
        } else {
            format!(
                "{}: STUCK ({}) at x={}",
                self.level,
                self.search.reason.as_str(),
                self.search.best.game.player().x.to_px()
            )
        }
    }

    /// One-line result.
    pub fn summary(&self) -> String {
        let s = &self.search;
        if s.complete() {
            let narrow = self.narrowest_text();
            format!(
                "SOLVED {}: complete at frame {}, {} jumps, {narrow}, {} search deaths, {:.1}s",
                self.level,
                self.frames(),
                self.windows.len(),
                s.search_deaths,
                s.elapsed.as_secs_f64()
            )
        } else {
            format!(
                "STUCK {}: {} – furthest x={} px at frame {}, {} search deaths, {:.1}s",
                self.level,
                s.reason.as_str(),
                s.best.game.player().x.to_px(),
                s.best.game.frame(),
                s.search_deaths,
                s.elapsed.as_secs_f64()
            )
        }
    }

    /// The route as a solver replay (`complete frames=N..N deaths=0`).
    pub fn replay(&self, shots: bool) -> Replay {
        let n = self.frames();
        let shots = if shots {
            [(1, "route_25"), (2, "route_50"), (3, "route_75")]
                .into_iter()
                .map(|(q, name)| Shot {
                    frame: n * q / 4,
                    name: name.to_owned(),
                })
                .collect()
        } else {
            Vec::new()
        };
        Replay {
            level: self.level.clone(),
            seed: 1,
            source: Source::Solver,
            expects: vec![
                Expect::Complete(Some(FrameRange { first: n, last: n })),
                Expect::Deaths(0),
            ],
            shots,
            inputs: run_length(&self.inputs),
        }
    }

    fn to_json(&self, files: &Files) -> Value {
        let s = &self.search;
        let p = s.best.game.player();
        let jumps: Vec<Value> = self
            .windows
            .iter()
            .map(|w| {
                json!({
                    "frame": w.frame(),
                    "x": w.x,
                    "hold": w.hold,
                    "window": w.size(),
                    "shift_min": w.lo,
                    "shift_max": w.hi,
                    "bound_min": w.lo_bound.as_str(),
                    "bound_max": w.hi_bound.as_str(),
                    "route_limited": w.route_limited(),
                })
            })
            .collect();
        let narrow = narrowest(&self.windows).map(|w| {
            json!({
                "frame": w.frame(),
                "x": w.x,
                "window": w.size(),
                "bound_min": w.lo_bound.as_str(),
                "bound_max": w.hi_bound.as_str(),
            })
        });
        json!({
            "level": self.level,
            "solved": s.complete(),
            "reason": s.reason.as_str(),
            "frames": if s.complete() { Some(self.frames()) } else { None },
            "furthest_x": p.x.to_px(),
            "furthest_frame": s.best.game.frame(),
            "search_deaths": s.search_deaths,
            "expanded_nodes": s.expanded,
            "runtime_ms": s.elapsed.as_millis() as u64,
            "jumps": jumps,
            "jump_count": self.windows.len(),
            "narrowest": narrow,
            "max_shift": MAX_SHIFT,
            "image": files.image.as_ref().map(|p| p.display().to_string()),
            "replay": files.replay.as_ref().map(|p| p.display().to_string()),
        })
    }

    fn to_markdown(&self, files: &Files) -> String {
        let mut md = format!("# Solver report: {}\n\n{}\n\n", self.level, self.summary());
        if let Some(p) = &files.image {
            let what = if self.search.complete() {
                "Narrowest jump"
            } else {
                "Furthest state"
            };
            md.push_str(&format!("{what}: `{}`\n\n", p.display()));
        }
        if let Some(p) = &files.replay {
            md.push_str(&format!("Replay: `{}`\n\n", p.display()));
        }
        if !self.windows.is_empty() {
            md.push_str(&format!(
                "Jump windows (JUMP press shifted by -{MAX_SHIFT}..+{MAX_SHIFT} frames; \
                 window = successful shifts around 0). Bounds: `fail` = the next shift \
                 fails in the level, `route` = the route itself prevents the next shift \
                 (window is a lower bound, `≥`, not counted as narrowest), `max` = ±{MAX_SHIFT}.\n\n\
                 | frame | x px | hold | window | shifts | bounds |\n|---|---|---|---|---|---|\n"
            ));
            for w in &self.windows {
                let at_least = if w.route_limited() { "≥ " } else { "" };
                md.push_str(&format!(
                    "| {} | {} | {} | {at_least}{} | {}..{} | {}/{} |\n",
                    w.frame(),
                    w.x,
                    w.hold,
                    w.size(),
                    w.lo,
                    w.hi,
                    w.lo_bound.as_str(),
                    w.hi_bound.as_str()
                ));
            }
        }
        md
    }
}

/// Paths written for a solution.
#[derive(Default)]
struct Files {
    image: Option<PathBuf>,
    replay: Option<PathBuf>,
}

/// Searches and, if complete, cleans up the route and measures the jump
/// windows.
pub fn solve(level: &Level, tuning: &Tuning, opts: &SolveOptions) -> Solution {
    let search = search(level, tuning, opts);
    let mut inputs = search.best.inputs();
    let windows = if search.complete() {
        inputs = clean_route(level, tuning, &inputs);
        jump_windows(level, tuning, &inputs)
    } else {
        Vec::new()
    };
    Solution {
        level: level.meta.id.clone(),
        search,
        inputs,
        windows,
    }
}

/// Player position (raw x, raw y) after every frame of `inputs`.
fn positions(level: &Level, tuning: &Tuning, inputs: &[InputFrame]) -> Vec<(i32, i32)> {
    let mut g = Game::new(level, tuning, 1);
    inputs
        .iter()
        .map(|&i| {
            g.step(i);
            (g.player().x.raw(), g.player().y.raw())
        })
        .collect()
}

/// Removes JUMP where it changes nothing: every JUMP hold is dropped, or
/// else cut to the shortest length, for which the player's position on
/// every frame stays exactly the same (so deaths and the finish frame do
/// too). Macro-actions leave mid-air presses and over-long holds in the
/// route; without them the replay reads better and jump windows are not
/// cut short by a neighbouring no-op hold.
pub fn clean_route(level: &Level, tuning: &Tuning, inputs: &[InputFrame]) -> Vec<InputFrame> {
    let want = positions(level, tuning, inputs);
    let mut out = inputs.to_vec();
    for (s, e) in jump_holds(inputs) {
        for end in s..e {
            let mut trial = out.clone();
            for f in &mut trial[end..e] {
                *f = f.without(InputFrame::JUMP);
            }
            if positions(level, tuning, &trial) == want {
                out = trial;
                break;
            }
        }
    }
    out
}

/// Run-length encodes per-frame inputs.
pub fn run_length(inputs: &[InputFrame]) -> Vec<(InputFrame, u32)> {
    let mut out: Vec<(InputFrame, u32)> = Vec::new();
    for &i in inputs {
        match out.last_mut() {
            Some((last, n)) if *last == i => *n += 1,
            _ => out.push((i, 1)),
        }
    }
    out
}

// ---------------------------------------------------------------- commands

/// Settings from the command line.
struct Args {
    target: Option<String>,
    write: bool,
    shots: bool,
    beam: Option<usize>,
    time_limit: Option<Duration>,
}

/// `cargo xtask solve <level-id|--all> [--write] [--shots] [--beam N] [--time-limit S]`.
pub fn run(root: &Path, args: &[String]) -> ExitCode {
    let mut a = Args {
        target: None,
        write: false,
        shots: false,
        beam: None,
        time_limit: None,
    };
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--write" => a.write = true,
            "--shots" => a.shots = true,
            "--beam" => match it.next().and_then(|v| v.parse().ok()).filter(|&n| n >= 1) {
                Some(n) => a.beam = Some(n),
                None => return usage_err("--beam needs a whole number ≥ 1"),
            },
            "--time-limit" => match it.next().and_then(|v| v.parse().ok()).filter(|&n| n >= 1) {
                Some(n) => a.time_limit = Some(Duration::from_secs(n)),
                None => return usage_err("--time-limit needs whole seconds ≥ 1"),
            },
            "help" | "-h" | "--help" => {
                print!("{HELP}");
                return ExitCode::SUCCESS;
            }
            t if a.target.is_none() => a.target = Some(t.to_owned()),
            _ => return usage_err(&format!("unexpected argument `{arg}`")),
        }
    }
    if a.shots && !a.write {
        return usage_err("--shots only makes sense with --write");
    }
    let Some(target) = a.target.clone() else {
        return usage_err("missing level id or --all");
    };
    let levels = if target == "--all" {
        all_levels(root)
    } else {
        content::find_level(root, &target).map(|l| vec![l])
    };
    let levels = match levels {
        Ok(l) => l,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::FAILURE;
        }
    };
    let mut ok = true;
    let out = OutDirs::of(root);
    for level in &levels {
        match solve_and_report(root, level, &a, &out) {
            Ok((sol, text)) => {
                ok &= sol.search.complete();
                print!("{text}");
            }
            Err(e) => {
                ok = false;
                eprintln!("error: {}: {e}", level.meta.id);
            }
        }
    }
    if a.write && a.shots && ok {
        println!(
            "reminder: the new shots need goldens: review them (`cargo xtask replay <replay> --shots`, \
             art-director), then `cargo xtask golden --accept <replay>`"
        );
    }
    if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

/// Every level in `content/levels`; any unparsable one is an error.
fn all_levels(root: &Path) -> Result<Vec<Level>, String> {
    content::all_levels(root)?
        .into_iter()
        .map(|(p, l)| l.map_err(|e| format!("{}: {e}", p.display())))
        .collect()
}

/// Where `solve` writes: images, reports, `--write` replays.
struct OutDirs {
    preview: PathBuf,
    reports: PathBuf,
    replays: PathBuf,
}

impl OutDirs {
    /// The real locations: `$CARGO_TARGET_DIR/{preview,reports}/solver`
    /// and `tests/replays`.
    fn of(root: &Path) -> OutDirs {
        let target = crate::target_dir(root);
        OutDirs {
            preview: target.join("preview/solver"),
            reports: target.join("reports/solver"),
            replays: content::replays_dir(root),
        }
    }
}

/// Solves `level` with the real content (from `root`), writes report,
/// image and (with `--write`) the replay into `out`; returns (solution,
/// printable text).
fn solve_and_report(
    root: &Path,
    level: &Level,
    a: &Args,
    out: &OutDirs,
) -> Result<(Solution, String), String> {
    let tuning = content::tuning(root)?;
    let mut opts = SolveOptions::for_level(level);
    if let Some(b) = a.beam {
        opts.beam_width = b;
    }
    if let Some(t) = a.time_limit {
        opts.time_limit = t;
    }
    let sol = solve(level, &tuning, &opts);
    let id = &sol.level;
    let mut files = Files::default();

    let texts = content::texts(root)?;
    let ctx = RenderContext::new(jnr_assets::atlas(), &texts, level);
    let dir = &out.preview;
    fs::create_dir_all(dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
    if sol.search.complete() {
        if let Some(w) = narrowest(&sol.windows) {
            let mut g = Game::new(level, &tuning, 1);
            for &i in &sol.inputs[..w.index] {
                g.step(i);
            }
            let img = replay::render_image(&g, &ctx);
            files.image = Some(replay::write_with_view_copy(
                dir,
                &format!("{id}_narrowest"),
                &img,
            )?);
        }
        if a.write {
            let path = out.replays.join(format!("{id}_solver.replay"));
            let text = format!(
                "# Written by `cargo xtask solve {id} --write`; re-create it instead of editing.\n{}",
                sol.replay(a.shots).format()
            );
            fs::write(&path, text).map_err(|e| format!("write {}: {e}", path.display()))?;
            files.replay = Some(path);
        }
    } else {
        let img = replay::render_image(&sol.search.best.game, &ctx);
        files.image = Some(replay::write_with_view_copy(
            dir,
            &format!("{id}_stuck"),
            &img,
        )?);
    }

    let (json_path, md_path) = write_report(&out.reports, &sol, &files)?;
    let mut text = format!("{}\n", sol.summary());
    for p in [&files.image, &files.replay].into_iter().flatten() {
        text.push_str(&format!("  {}\n", p.display()));
    }
    text.push_str(&format!(
        "  report {}\n  report {}\n",
        md_path.display(),
        json_path.display()
    ));
    Ok((sol, text))
}

fn write_report(dir: &Path, sol: &Solution, files: &Files) -> Result<(PathBuf, PathBuf), String> {
    fs::create_dir_all(dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
    let json_path = dir.join(format!("{}.json", sol.level));
    let md_path = dir.join(format!("{}.md", sol.level));
    let json = serde_json::to_string_pretty(&sol.to_json(files)).expect("JSON values serialize");
    fs::write(&json_path, json + "\n")
        .map_err(|e| format!("write {}: {e}", json_path.display()))?;
    fs::write(&md_path, sol.to_markdown(files))
        .map_err(|e| format!("write {}: {e}", md_path.display()))?;
    Ok((json_path, md_path))
}

/// The `solver` verify step: every level must be solved and have its
/// committed `tests/replays/<id>_solver.replay`. Returns (ok, output, note
/// with the narrowest jump window per level).
pub fn verify(root: &Path) -> (bool, String, String) {
    verify_into(root, &OutDirs::of(root))
}

fn verify_into(root: &Path, dirs: &OutDirs) -> (bool, String, String) {
    let levels = match all_levels(root) {
        Ok(l) if l.is_empty() => {
            return (false, "no levels in content/levels".into(), String::new());
        }
        Ok(l) => l,
        Err(e) => return (false, e, String::new()),
    };
    let a = Args {
        target: None,
        write: false,
        shots: false,
        beam: None,
        time_limit: None,
    };
    let (mut ok, mut out, mut notes) = (true, String::new(), Vec::new());
    for level in &levels {
        match solve_and_report(root, level, &a, dirs) {
            Ok((sol, text)) => {
                ok &= sol.search.complete();
                out.push_str(&text);
                notes.push(sol.note());
                let id = &level.meta.id;
                let replay = dirs.replays.join(format!("{id}_solver.replay"));
                if !replay.is_file() {
                    ok = false;
                    out.push_str(&format!(
                        "FAIL {id}: missing solver replay {} (create it with \
                         `cargo xtask solve {id} --write` and commit it)\n",
                        replay.display()
                    ));
                }
            }
            Err(e) => {
                ok = false;
                out.push_str(&format!("FAIL {}: {e}\n", level.meta.id));
            }
        }
    }
    (ok, out, notes.join("; "))
}

fn usage_err(msg: &str) -> ExitCode {
    eprintln!("error: {msg}\n\n{HELP}");
    ExitCode::from(2)
}

// -------------------------------------------------------------------- tests

#[cfg(test)]
mod tests {
    use super::*;

    fn tuning() -> Tuning {
        content::tuning(&Path::new(env!("CARGO_MANIFEST_DIR")).join("..")).unwrap()
    }

    /// A 6-row level: `rows` are the top 4 rows (P, G placed by the caller),
    /// `ground` the bottom two rows.
    fn level(rows: [&str; 4], ground: &str) -> Level {
        let grid = format!(
            "{}\n{}\n{}\n{}\n{ground}\n{ground}\n",
            rows[0], rows[1], rows[2], rows[3]
        );
        let text = format!(
            "[meta]\nid = \"synthetic\"\ntime_limit = 100\n[legend]\n\"#\" = \"solid\"\n\
             \"P\" = {{ entity = \"player_start\" }}\n\"G\" = {{ entity = \"goal\" }}\n\
             [map]\ngrid = '''\n{grid}'''\n"
        );
        Level::parse(&text).unwrap()
    }

    fn gap_level(gap: usize) -> Level {
        let w = 30 + gap;
        let air = ".".repeat(w);
        let mut top = ".".repeat(w);
        top.replace_range(1..2, "P");
        top.replace_range(w - 2..w - 1, "G");
        let ground = format!(
            "{}{}{}",
            "#".repeat(12),
            ".".repeat(gap),
            "#".repeat(w - 12 - gap)
        );
        level([&air, &air, &air, &top], &ground)
    }

    fn opts(level: &Level, threads: usize) -> SolveOptions {
        SolveOptions {
            threads,
            ..SolveOptions::for_level(level)
        }
    }

    #[test]
    fn solves_a_gap_and_the_route_replays() {
        let level = gap_level(3);
        let t = tuning();
        let sol = solve(&level, &t, &opts(&level, 4));
        assert!(sol.search.complete(), "{}", sol.summary());
        assert_eq!(
            sol.windows.len(),
            1,
            "one jump over the gap: {:?}",
            sol.windows
        );
        let w = &sol.windows[0];
        assert!(w.lo <= 0 && w.hi >= 0 && w.size() >= 2, "{w:?}");
        // The written replay passes its own expectations.
        let replay = sol.replay(true);
        let parsed = Replay::parse(&replay.format()).unwrap();
        assert_eq!(parsed, replay);
        let outcome = parsed.run(&level, &t, |_| {});
        assert_eq!(parsed.check(&outcome), Vec::<String>::new());
        assert_eq!(outcome.completed_at, Some(sol.frames()));
        assert_eq!(parsed.shots.len(), 3);
    }

    fn root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("..")
    }

    /// Fresh output directories under `$CARGO_TARGET_DIR/tmp/xtask-solver-test/<name>`,
    /// so tests never touch the real preview/reports/replays folders.
    fn tmp_dirs(name: &str) -> OutDirs {
        let base = crate::target_dir(&root())
            .join("tmp/xtask-solver-test")
            .join(name);
        if base.exists() {
            fs::remove_dir_all(&base).unwrap();
        }
        let dirs = OutDirs {
            preview: base.join("preview"),
            reports: base.join("reports"),
            replays: base.join("replays"),
        };
        fs::create_dir_all(&dirs.replays).unwrap();
        dirs
    }

    fn no_args() -> Args {
        Args {
            target: None,
            write: false,
            shots: false,
            beam: None,
            time_limit: None,
        }
    }

    #[test]
    fn impossible_gap_fails_with_the_furthest_point_and_a_stuck_report() {
        // Through the command path: report, JSON and the stuck screenshot.
        let mut level = gap_level(14);
        level.meta.id = "synthetic_impossible".into();
        let dirs = tmp_dirs("impossible");
        let args = Args {
            write: true, // nothing to write when the search fails
            ..no_args()
        };
        let (sol, text) = solve_and_report(&root(), &level, &args, &dirs).unwrap();
        assert!(!sol.search.complete());
        assert!(
            text.starts_with("STUCK synthetic_impossible: frame budget exhausted – furthest x="),
            "{text}"
        );
        assert!(!text.contains(".replay"), "{text}");
        assert_eq!(fs::read_dir(&dirs.replays).unwrap().count(), 0);
        let png = dirs.preview.join("synthetic_impossible_stuck.png");
        assert!(text.contains(&png.display().to_string()), "{text}");
        assert!(png.is_file());
        let json = dirs.reports.join("synthetic_impossible.json");
        let v: Value = serde_json::from_str(&fs::read_to_string(json).unwrap()).unwrap();
        assert_eq!(v["solved"], false);
        assert_eq!(v["reason"], "frame budget exhausted");
        assert!(v["search_deaths"].as_u64().unwrap() > 0);
        // Standing at the edge of the gap (ground ends at 12 * 16 = 192 px).
        let x = v["furthest_x"].as_i64().unwrap();
        assert!((150..192).contains(&x), "furthest x {x}");
        assert_eq!(v["jump_count"], 0);
        assert!(
            sol.note()
                .starts_with("synthetic_impossible: STUCK (frame budget exhausted) at x=")
        );
    }

    #[test]
    fn time_limit_zero_stops_without_completing() {
        let level = gap_level(3);
        let opts = SolveOptions {
            time_limit: Duration::ZERO,
            ..opts(&level, 1)
        };
        let s = search(&level, &tuning(), &opts);
        assert_eq!(s.reason, StopReason::TimeLimit);
        assert!(!s.complete());
    }

    #[test]
    fn same_level_same_route_on_any_thread_count() {
        let level = gap_level(5);
        let t = tuning();
        let a = solve(&level, &t, &opts(&level, 1));
        assert!(a.search.complete());
        for threads in [2, 4, 4] {
            let b = solve(&level, &t, &opts(&level, threads));
            assert_eq!(a.inputs, b.inputs, "{threads} threads");
            assert_eq!(a.search.search_deaths, b.search.search_deaths);
            assert_eq!(a.windows, b.windows);
        }
    }

    #[test]
    fn select_beam_keeps_a_grounded_reserve() {
        let level = gap_level(0);
        let t = tuning();
        let grounded = Node {
            game: Game::new(&level, &t, 1),
            jumps: 0,
            trail: None,
        };
        let mut airborne = grounded.clone();
        airborne.game.step(InputFrame::JUMP);
        assert!(!airborne.game.player().on_ground && grounded.game.player().on_ground);
        let on_ground = |nodes: &[Node]| -> Vec<bool> {
            nodes.iter().map(|n| n.game.player().on_ground).collect()
        };
        let mut ranked = vec![airborne.clone(); 8];
        ranked.extend([grounded.clone(), grounded.clone()]);

        // Width 4: the 4 best plus 1 grounded (a quarter of 4).
        let beam = select_beam(ranked.clone(), 4);
        assert_eq!(on_ground(&beam), [false, false, false, false, true]);
        // Width 8: a quarter is 2 grounded.
        let beam = select_beam(ranked.clone(), 8);
        assert_eq!(beam.len(), 10);
        assert_eq!(on_ground(&beam[8..]), [true, true]);
        // Enough grounded nodes inside the width already: plain cut.
        let mut mixed = vec![grounded.clone(), airborne.clone()];
        mixed.extend(vec![airborne.clone(); 6]);
        assert_eq!(select_beam(mixed, 4).len(), 4);
        // Fewer nodes than the width: unchanged.
        assert_eq!(select_beam(vec![airborne; 3], 4).len(), 3);
    }

    #[test]
    fn window_sides_record_route_limits_and_padding() {
        // Standing jump on flat ground 5 frames into a short route: every
        // shift survives. Earlier than frame 0 is a route limit; later shifts
        // run past the route's end, which is padded, so they reach +12.
        let level = gap_level(0);
        let mut inputs = vec![NONE; 5];
        inputs.push(InputFrame::JUMP);
        inputs.extend([NONE; 5]);
        let w = jump_windows(&level, &tuning(), &inputs);
        assert_eq!(w.len(), 1);
        assert_eq!((w[0].lo, w[0].lo_bound), (-5, Bound::Route));
        assert_eq!((w[0].hi, w[0].hi_bound), (MAX_SHIFT, Bound::Max));
        assert_eq!((w[0].index, w[0].frame(), w[0].hold), (5, 6, 1));
        assert!(w[0].route_limited());
        assert_eq!(
            narrowest(&w),
            None,
            "route-limited windows are not the narrowest"
        );
    }

    #[test]
    fn neighbouring_holds_limit_by_route_not_by_level() {
        // Two jumps on flat ground with only 2 frames between their JUMP
        // holds: the first is held until shortly before landing, the second
        // pressed in the air just before landing (jump buffer). Shifting
        // into the other hold is a route limit, never a narrow window.
        let level = gap_level(0);
        let t = tuning();
        let mut probe = Game::new(&level, &t, 1);
        let mut landing = 0; // input index of the step that lands
        for i in 0..200 {
            let input = if i < 5 { NONE } else { InputFrame::JUMP };
            if probe.step(input).contains(&Event::Landed) {
                landing = i;
                break;
            }
        }
        assert!(landing > 20, "landing at {landing}");
        let mut inputs = vec![NONE; 5];
        inputs.extend(vec![InputFrame::JUMP; landing - 3 - 5]);
        inputs.extend([NONE; 2]);
        inputs.push(InputFrame::JUMP); // index landing - 1
        inputs.extend([NONE; 80]);
        assert_eq!(
            jump_holds(&inputs),
            vec![(5, landing - 3), (landing - 1, landing)]
        );

        let w = jump_windows(&level, &t, &inputs);
        assert_eq!(w.len(), 2, "both presses jump: {w:?}");
        assert_eq!((w[0].lo, w[0].lo_bound), (-5, Bound::Route));
        assert_eq!((w[0].hi, w[0].hi_bound), (1, Bound::Route), "{w:?}");
        assert_eq!((w[1].lo, w[1].lo_bound), (-1, Bound::Route), "{w:?}");
        assert_eq!(w[1].hi_bound, Bound::Max, "{w:?}");
        assert!(w.iter().all(JumpWindow::route_limited));
        assert_eq!(narrowest(&w), None);
    }

    #[test]
    fn window_limited_by_a_pit_and_coyote_time() {
        // Run right at a 4-tile gap and jump late: the latest working press
        // is the last coyote frame after leaving the edge.
        let level = gap_level(4);
        let t = tuning();
        let mut probe = Game::new(&level, &t, 1);
        let mut left_ground = 0;
        for i in 1..200 {
            probe.step(RB);
            if !probe.player().on_ground {
                left_ground = i; // input index i-1 is the first airborne step
                break;
            }
        }
        assert!(left_ground > 0);
        // Press with input index p: a jump works while coyote > 0, i.e. up
        // to coyote_frames steps after the first airborne step.
        let latest = left_ground - 1 + t.player.coyote_frames as usize;
        let press = latest - 2;
        let mut inputs = vec![RB; press];
        inputs.extend([RBJ; 20]);
        inputs.extend([RB; 150]);
        let w = jump_windows(&level, &t, &inputs);
        assert_eq!(w.len(), 1, "{w:?}");
        assert_eq!(
            (w[0].hi, w[0].hi_bound),
            (2, Bound::Fail),
            "late bound = coyote: {w:?}"
        );
        assert_eq!(
            (w[0].lo, w[0].lo_bound),
            (-MAX_SHIFT, Bound::Max),
            "early jumps still clear 4 tiles: {w:?}"
        );
        assert_eq!(w[0].size(), 15);
        assert_eq!(narrowest(&w), Some(&w[0]));
    }

    #[test]
    fn shifted_jump_that_ruins_a_later_jump_fails() {
        // Jump 1 is a hop while still accelerating (air accel < ground
        // accel), so its timing moves the player by a few pixels for good.
        // Jump 2, more than 2 s after jump 1's landing, is pressed on the
        // last coyote frame before a pit. A shift of jump 1 that is fine at
        // its own checkpoint can make jump 2 too late; it must count as a
        // failure of jump 1's window.
        let w = 70;
        let air = ".".repeat(w);
        let mut top = ".".repeat(w);
        top.replace_range(1..2, "P");
        top.replace_range(w - 2..w - 1, "G");
        let pit_col = 40;
        let ground = format!(
            "{}{}{}",
            "#".repeat(pit_col),
            ".".repeat(4),
            "#".repeat(w - pit_col - 4)
        );
        let level = level([&air, &air, &air, &top], &ground);
        let t = tuning();
        let hop = 3;
        let mut prefix = vec![RB; hop];
        prefix.push(RBJ);
        // Find the first airborne step at the pit edge (after the hop).
        let mut probe = Game::new(&level, &t, 1);
        let mut left_ground = 0;
        for i in 0..400 {
            let input = prefix.get(i).copied().unwrap_or(RB);
            probe.step(input);
            if i > 60 && !probe.player().on_ground {
                left_ground = i;
                break;
            }
        }
        assert!(left_ground > 0);
        let press2 = left_ground + t.player.coyote_frames as usize - 1;
        let mut inputs = prefix.clone();
        inputs.resize(press2, RB);
        inputs.extend([RBJ; 20]);
        inputs.extend([RB; 200]);
        let windows = jump_windows(&level, &t, &inputs);
        assert_eq!(windows.len(), 2, "{windows:?}");
        let w1 = &windows[0];
        assert!(
            windows[1].index > (w1.index + 150),
            "jump 2 is far after jump 1"
        );

        // Some shift of jump 1 dies only after jump 1's own checkpoint ...
        let (s, e) = (w1.index, w1.index + 1);
        let dies = |d: i32| {
            let shifted = shift_hold(&inputs, s, e, d).unwrap();
            let mut g = Game::new(&level, &t, 1);
            shifted.iter().any(|&i| g.step(i).contains(&Event::Died))
        };
        let late_deaths: Vec<i32> = (1..=MAX_SHIFT).filter(|&d| dies(d)).collect();
        assert!(
            !late_deaths.is_empty(),
            "scenario: a later hop ruins jump 2"
        );
        // ... and the window stops before it.
        assert!(
            w1.hi < late_deaths[0],
            "{w1:?} vs deaths at {late_deaths:?}"
        );
        assert_eq!(w1.hi_bound, Bound::Fail, "{w1:?}");
    }
    #[test]
    fn clean_route_drops_mid_air_presses_and_trims_long_holds() {
        let level = gap_level(0);
        let t = tuning();
        // Running jump held 45 frames (rising ends after ~37), released for
        // 2 frames, then a 1-frame press high in the air (no-op: landing is
        // more than the jump buffer away), then run to the goal.
        let mut dirty = vec![RB; 10];
        dirty.extend([RBJ; 45]);
        dirty.extend([RB; 2]);
        dirty.push(RBJ);
        dirty.extend([RB; 300]);
        let finish = |inputs: &[InputFrame]| {
            trace(&level, &t, inputs)
                .events
                .iter()
                .find(|&&(_, e)| e == Event::LevelComplete)
                .map(|&(f, _)| f)
        };
        let jumped = |inputs: &[InputFrame]| {
            trace(&level, &t, inputs)
                .events
                .iter()
                .filter(|&&(_, e)| e == Event::Jumped)
                .count()
        };
        assert!(finish(&dirty).is_some());
        assert_eq!(jumped(&dirty), 1, "the mid-air press must not jump");

        let clean = clean_route(&level, &t, &dirty);
        let holds = jump_holds(&clean);
        assert_eq!(holds.len(), 1, "mid-air press removed: {holds:?}");
        let (s, e) = holds[0];
        assert_eq!(s, 10);
        assert!(e - s < 45 && e - s >= 30, "hold trimmed to {}", e - s);
        assert_eq!(positions(&level, &t, &clean), positions(&level, &t, &dirty));
        assert_eq!(finish(&clean), finish(&dirty));
        assert_eq!(jumped(&clean), 1);
    }

    #[test]
    fn verify_fails_without_the_committed_solver_replay() {
        let dirs = tmp_dirs("verify_missing_replay");
        let (ok, out, note) = verify_into(&root(), &dirs);
        assert!(!ok, "{out}");
        assert!(out.contains("missing solver replay"), "{out}");
        assert!(out.contains("`cargo xtask solve testbed --write`"), "{out}");
        assert!(note.starts_with("testbed: "), "{note}");
        // With the replay present (any content: the `replays` step checks it).
        fs::write(dirs.replays.join("testbed_solver.replay"), "").unwrap();
        let (ok, out, _) = verify_into(&root(), &dirs);
        assert!(ok, "{out}");
    }

    #[test]
    fn shift_and_run_length_helpers() {
        let j = InputFrame::JUMP;
        let seq = [NONE, j, j, NONE, NONE, j];
        assert_eq!(jump_holds(&seq), vec![(1, 3), (5, 6)]);
        assert_eq!(
            shift_hold(&seq, 1, 3, -1).unwrap(),
            vec![j, j, NONE, NONE, NONE, j]
        );
        assert!(shift_hold(&seq, 1, 3, -2).is_none(), "before frame 0");
        assert_eq!(
            shift_hold(&seq, 1, 3, 1).unwrap(),
            vec![NONE, NONE, j, j, NONE, j]
        );
        assert!(shift_hold(&seq, 1, 3, 2).is_none(), "touches the next hold");
        // Past the end: padded with the last input without JUMP.
        let r = [RB, RBJ, RB];
        assert_eq!(shift_hold(&r, 1, 2, 3).unwrap(), vec![RB, RB, RB, RB, RBJ]);
        assert_eq!(run_length(&seq), vec![(NONE, 1), (j, 2), (NONE, 2), (j, 1)]);
        let ok_in = |range: std::ops::RangeInclusive<i32>| {
            move |d: i32| {
                if range.contains(&d) {
                    Shifted::Ok
                } else {
                    Shifted::Fail
                }
            }
        };
        assert_eq!(
            window_range(ok_in(-3..=1)),
            (-3, Bound::Fail, 1, Bound::Fail)
        );
        assert_eq!(
            window_range(|_| Shifted::Ok),
            (-MAX_SHIFT, Bound::Max, MAX_SHIFT, Bound::Max)
        );
        assert_eq!(
            window_range(|d| if d < -2 { Shifted::Route } else { Shifted::Ok }),
            (-2, Bound::Route, MAX_SHIFT, Bound::Max)
        );
    }
}
