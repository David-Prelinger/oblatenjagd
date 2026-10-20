//! The solver bot (L4): `cargo xtask solve` and the `solver` verify step.
//!
//! **Search.** A frame-synchronous beam search over the real `jnr-core`
//! simulation: every search node is a cloned [`Game`] plus the macro-actions
//! that led to it. Nodes are kept in buckets by their frame and the buckets
//! are processed in frame order. Each bucket is ranked (furthest right
//! first, then fewer jumps; remaining ties keep their insertion order,
//! which is deterministic), de-duplicated by a coarse state key (see
//! [`StateKey`]) so the beam stays diverse, cut to the beam width plus a
//! grounded and a "high" reserve (see [`select_beam`]), and every survivor is expanded
//! with every macro-action in [`MACROS`]. A macro that makes the player die is pruned
//! and counted as a "search death"; a macro that touches the goal stops at
//! that frame. The search ends at the first bucket at or after the earliest
//! completion found, so the route is the earliest completion the beam sees.
//! It fails when no node survives, after the frame budget (3× the level
//! width in pixels; climb levels: 3× width + height; auto-scroll levels:
//! plus the frames the scroll takes, [`scroll_frames`], D-075) or after the
//! wall-clock limit (default 60 s).
//!
//! **Climb levels** (`[meta] camera = "climb"`, D-054): "furthest right"
//! can't see progress inside a vertical shaft, so nodes are ranked by
//! [`search_progress`]: x plus the height the feet gained above the start's feet
//! (both in subpixels; side levels: x alone). The jump-window recovery
//! then also requires the feet to be at most [`CLIMB_FEET_TOLERANCE_PX`]
//! lower than the original route's at the checkpoint, so a missed jump
//! in the shaft that lands on the plank below no longer counts as
//! recovered just because x is the same. If the route is in the air at
//! the checkpoint (often: a later jump within 2 s), its "feet" there are
//! the lower of the ground before and after (`Trace::goal_feet`), never
//! a point in the air.
//!
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
//! **Difficulty** (D-033: jump windows measure recoverability). For every
//! jump of the route (a JUMP press followed by a `Jumped`), the whole
//! held-JUMP segment is shifted by d ∈ [-12, +12] frames with the rest of
//! the inputs unchanged (padded with the last input past the end of the
//! route). Its checkpoint is the first landing after that `Jumped` + 2 s,
//! capped at the route's end; the progress there is the original x minus
//! one tile. A shift succeeds if the player survives the shifted jump and
//! then reaches that progress (or completes the level) without dying:
//! either the unchanged later inputs do it by the checkpoint, standing on
//! the ground (fast path), or a short re-planning beam search ([`recovers`], beam
//! [`REPLAN_BEAM`]) from the state right after the shifted jump's landing
//! (or, if the press made no jump, after its buffer ran out) reaches it
//! standing on the ground by the checkpoint + [`REPLAN_MARGIN`] frames. So
//! a later press that a 1-frame shift makes miss its buffer no longer
//! collapses the window: a player would simply press it a frame later. The
//! jump window is the contiguous successful range around d = 0. Each side
//! records why it stopped ([`Bound`]): a failed shift, the route (the
//! shift would start before frame 0 or touch a neighbouring JUMP hold) or
//! ±12. Windows with a route-limited side are only lower bounds and never
//! count as the narrowest. A recovery may take a small detour (skip an
//! item, another platform): the window asks whether a player can get on
//! with the stage, not whether the score stays the same (D-034).
//! Each shift starts from a snapshot of the original route at its earliest
//! changed frame, and the jumps are measured on parallel threads (results
//! in jump order, so independent of the thread count).

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use jnr_core::replay::{Expect, FrameRange, Shot, Source};
use jnr_core::{
    CameraMode, Event, Game, InputFrame, Level, Mode, Replay, TICKS_PER_SECOND, Tuning,
};
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
/// Search threads per level when `verify` searches several levels at once
/// (D-082): few levels at once with many threads each keeps every search
/// far below the wall-clock limit (measured on 14 cores: 2 × 7 threads,
/// slowest search 13.5 s idle, 16.5–18.2 s under load; 4 × 3: 22–25 s), at about the same total time.
const LEVEL_THREADS: usize = 7;
/// Buckets smaller than this are expanded on the calling thread.
const PARALLEL_MIN_NODES: usize = 16;

/// Jump shifts tested for the window: d ∈ [-MAX_SHIFT, +MAX_SHIFT].
pub const MAX_SHIFT: i32 = 12;
/// The progress checkpoint is this many frames after the original landing.
const CHECKPOINT_AFTER_LANDING: u32 = 2 * TICKS_PER_SECOND;
/// A shifted jump may end up this many pixels behind the original.
const PROGRESS_TOLERANCE_PX: i32 = 16;
/// Beam width of the re-planning search after a shifted jump (D-033).
pub const REPLAN_BEAM: usize = 12;
/// The re-planning search may reach the progress this many frames after
/// the jump's checkpoint (a shifted jump lands up to 12 frames later).
pub const REPLAN_MARGIN: u32 = 30;
/// Climb levels: the re-planning margin (D-054 addendum). A shifted jump
/// that lands on the plank below costs one more jump up (about 40 frames
/// up 3 tiles and down onto the plank), which a player simply makes; 30
/// frames would count it as a failure.
pub const CLIMB_REPLAN_MARGIN: u32 = 60;
/// Climb levels: a shifted jump may end with the feet this many px lower
/// than the original route's at the checkpoint (D-054).
pub const CLIMB_FEET_TOLERANCE_PX: i32 = 8;

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
    /// The defaults for `level`: beam 64, budget 3× its width in pixels
    /// (climb levels: width + height; auto-scroll levels: plus the frames
    /// the scroll takes, D-075), 60 s, up to 8 threads.
    pub fn for_level(level: &Level, tuning: &Tuning) -> SolveOptions {
        let threads = std::thread::available_parallelism().map_or(1, |n| n.get().min(MAX_THREADS));
        let tiles = match level.meta.camera {
            CameraMode::Side | CameraMode::Autoscroll => level.width.max(1),
            CameraMode::Climb => level.width.max(1) + level.height.max(0),
        };
        SolveOptions {
            beam_width: DEFAULT_BEAM,
            frame_budget: BUDGET_PER_PX * (tiles as u32) * jnr_core::level::TILE_PX as u32
                + scroll_frames(level, tuning),
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

    fn progress(&self) -> i32 {
        search_progress(&self.game)
    }
}

/// Frames the auto-scroll takes to cross its zone (D-075): the zone's
/// length in subpixels over `camera.autoscroll_speed`, rounded up; 0
/// without a zone.
pub fn scroll_frames(level: &Level, tuning: &Tuning) -> u32 {
    let Some((from, to)) = level.meta.autoscroll_zone() else {
        return 0;
    };
    let len = jnr_core::Fx::from_px((to - from) * jnr_core::level::TILE_PX).raw();
    let speed = tuning.camera.autoscroll_speed.raw().max(1);
    (len + speed - 1).div_euclid(speed).max(0) as u32
}

/// How far a state has come, in subpixels: the player's x; in climb levels
/// (D-054) plus the height her feet gained above the player start's feet
/// (negative below it). The search ranks by it.
pub fn search_progress(game: &Game) -> i32 {
    let p = game.player();
    match game.level().meta.camera {
        CameraMode::Side | CameraMode::Autoscroll => p.x.raw(),
        CameraMode::Climb => {
            let start = game.level().player_start();
            let start_feet = jnr_core::Fx::from_px((start.row + 1) * jnr_core::level::TILE_PX);
            p.x.raw() + (start_feet - p.feet().1).raw()
        }
    }
}

/// Coarse state for de-duplication within a frame bucket: position in 4 px
/// cells, speed in 1/4 px (x) and 1/2 px (y) per frame, grounded or not,
/// plus [`Game::world_hash`] (changed tiles, entities incl. enemy/platform state and pattern counters, checkpoint, power
/// state), so states that differ in collected items, broken bricks or the
/// power-up are kept apart. Of several nodes with the same key only the
/// best-ranked one is kept.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct StateKey(i32, i32, i32, i32, bool, u64);

impl StateKey {
    fn of(game: &Game) -> StateKey {
        let p = game.player();
        StateKey(
            p.x.raw() >> 10,
            p.y.raw() >> 10,
            p.vx.raw() >> 6,
            p.vy.raw() >> 7,
            p.on_ground,
            game.world_hash(),
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
        // Furthest right (climb levels: furthest right + highest) first,
        // then fewer jumps (no needless hops, so jump windows are not
        // coupled); the stable sort keeps insertion order on ties.
        nodes.sort_by_key(|n| (std::cmp::Reverse(n.progress()), n.jumps));
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
                    && (node.progress() > best.progress()
                        || (node.progress() == best.progress() && f < best.game.frame()))
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

/// The beam: the first `width` of the ranked `nodes`, plus, from the rest
/// in rank order:
/// - the best grounded nodes until at least a quarter of `width` is
///   grounded. Airborne nodes far right are often already doomed; the
///   grounded reserve keeps safe states alive (e.g. waiting at an edge);
/// - up to a quarter of `width` "high" nodes: a node whose feet are higher
///   than every kept node's in its [`HEIGHT_COLUMN_PX`] column. Furthest
///   right alone drops a player waiting above a pit for a moving platform
///   (or riding it) in favour of nodes further right down in the pit.
fn select_beam(mut nodes: Vec<Node>, width: usize) -> Vec<Node> {
    let width = width.max(1);
    if nodes.len() <= width {
        return nodes;
    }
    let rest = nodes.split_off(width);
    let grounded = nodes.iter().filter(|n| n.game.player().on_ground).count();
    let mut missing = (width / 4).saturating_sub(grounded);
    let mut high = width / 4;
    let column = |n: &Node| n.game.player().x.to_px().div_euclid(HEIGHT_COLUMN_PX);
    let feet = |n: &Node| n.game.player().feet().1;
    let mut highest: BTreeMap<i32, jnr_core::Fx> = BTreeMap::new();
    let keep = |n: &Node, highest: &mut BTreeMap<i32, jnr_core::Fx>| {
        let y = highest.entry(column(n)).or_insert(feet(n));
        *y = (*y).min(feet(n));
    };
    for n in &nodes {
        keep(n, &mut highest);
    }
    for n in rest {
        if missing > 0 && n.game.player().on_ground {
            missing -= 1;
        } else if high > 0 && highest.get(&column(&n)).is_none_or(|&y| feet(&n) < y) {
            high -= 1;
        } else {
            continue;
        }
        keep(&n, &mut highest);
        nodes.push(n);
    }
    nodes
}

/// Column width of the beam's "high node" reserve (see [`select_beam`]).
const HEIGHT_COLUMN_PX: i32 = 16;

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

/// Per-frame record of a run: x, the feet's y (px) and whether she
/// stands on the ground after each frame (index 0 = start) and events.
struct Trace {
    x: Vec<i32>,
    feet: Vec<i32>,
    ground: Vec<bool>,
    events: Vec<(u32, Event)>,
}

impl Trace {
    /// The climb height goal's feet at frame `f` (D-059 addendum, E17):
    /// her feet if she stands on the ground then, else the lower (larger
    /// y) of the last ground before `f` and the next ground after it. A
    /// checkpoint can fall into the air of a later jump; its feet there
    /// are a point no grounded run on the same floor ever reaches.
    fn goal_feet(&self, f: usize) -> i32 {
        if self.ground[f] {
            return self.feet[f];
        }
        let before = (0..f).rev().find(|&i| self.ground[i]);
        // The next ground may lie past the deadline; taking the lower of
        // the two grounds can only relax the goal, never tighten it.
        let after = (f + 1..self.feet.len()).find(|&i| self.ground[i]);
        match (before, after) {
            (Some(b), Some(a)) => self.feet[b].max(self.feet[a]),
            (Some(i), None) | (None, Some(i)) => self.feet[i],
            (None, None) => self.feet[f],
        }
    }
}

fn trace(level: &Level, tuning: &Tuning, inputs: &[InputFrame]) -> Trace {
    let mut g = Game::new(level, tuning, 1);
    let mut t = Trace {
        x: vec![g.player().x.to_px()],
        feet: vec![g.player().feet().1.to_px()],
        ground: vec![g.player().on_ground],
        events: Vec::new(),
    };
    for &i in inputs {
        g.step(i);
        t.events.extend(g.events().iter().map(|&e| (g.frame(), e)));
        t.x.push(g.player().x.to_px());
        t.feet.push(g.player().feet().1.to_px());
        t.ground.push(g.player().on_ground);
    }
    t
}

/// Where a shifted run stands at its checkpoint (see [`progress`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Progress {
    /// Player x (px) at the checkpoint, or where the level was completed.
    x: i32,
    /// The feet's y (px) then.
    feet: i32,
    /// The level was completed by the checkpoint.
    done: bool,
    /// The player stands on the ground at the checkpoint.
    on_ground: bool,
}

/// Runs `inputs` from `start` (a state of the same route) up to frame
/// `check` (or the end of `inputs`): `None` if the player died on the way,
/// else where it stands then (or where it completed the level).
fn progress(start: &Game, inputs: &[InputFrame], check: u32) -> Option<Progress> {
    let mut g = start.clone();
    let at = |g: &Game, done| Progress {
        x: g.player().x.to_px(),
        feet: g.player().feet().1.to_px(),
        done,
        on_ground: g.player().on_ground,
    };
    while g.frame() < check {
        let Some(&i) = inputs.get(g.frame() as usize) else {
            break;
        };
        if g.step(i).contains(&Event::Died) {
            return None;
        }
        if g.mode() == Mode::StageClear {
            return Some(at(&g, true));
        }
    }
    Some(at(&g, false))
}

/// Where a shifted jump leaves the player for re-planning, running
/// `inputs` from `start` (a state of the same route before the press):
/// after the step that lands it (the first `Landed` after its `Jumped`),
/// or, if the press made no jump, after the jump buffer ran out.
/// `Err(true)`: the level was completed on the way (success), `Err(false)`:
/// died or never landed before `deadline` (failure).
fn after_shifted_jump(
    start: &Game,
    tuning: &Tuning,
    inputs: &[InputFrame],
    press: u32,
    deadline: u32,
) -> Result<Game, bool> {
    let buffer = tuning.player.jump_buffer_frames as u32;
    let mut g = start.clone();
    let mut jumped = false;
    let pad = inputs
        .last()
        .map_or(InputFrame::NONE, |f| f.without(InputFrame::JUMP));
    while g.frame() < deadline {
        let input = inputs.get(g.frame() as usize).copied().unwrap_or(pad);
        let events = g.step(input);
        if events.contains(&Event::Died) {
            return Err(false);
        }
        if g.mode() == Mode::StageClear {
            return Err(true);
        }
        let f = g.frame();
        if f >= press && !jumped {
            jumped = g.events().contains(&Event::Jumped);
            if !jumped && f >= press + buffer {
                return Ok(g);
            }
        } else if jumped && g.events().contains(&Event::Landed) {
            return Ok(g);
        }
    }
    Err(false)
}

/// Where a shifted jump must get back to (D-033, D-054): standing on the
/// ground at x ≥ `x` px and, in climb levels, with the feet at y ≤ `feet`
/// px (no lower than that).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Goal {
    pub x: i32,
    pub feet: Option<i32>,
}

impl Goal {
    fn reached_by(&self, p: Progress) -> bool {
        p.done || (p.on_ground && p.x >= self.x && self.feet.is_none_or(|f| p.feet <= f))
    }
}

/// The re-planning search (D-033): a small beam search with the solver's
/// macro-actions from `game` that finds a node standing on the ground at
/// the `goal` (or completing the level) by frame `deadline`, without
/// dying. Deterministic like [`search`] (nodes in rank order, results
/// joined in node order).
pub fn recovers(game: Game, goal: Goal, deadline: u32, threads: usize) -> bool {
    let reached = |g: &Game| {
        goal.reached_by(Progress {
            x: g.player().x.to_px(),
            feet: g.player().feet().1.to_px(),
            done: g.mode() == Mode::StageClear,
            on_ground: g.player().on_ground,
        })
    };
    if reached(&game) {
        return true;
    }
    let mut buckets: BTreeMap<u32, Vec<Node>> = BTreeMap::new();
    buckets.insert(
        game.frame(),
        vec![Node {
            game,
            jumps: 0,
            trail: None,
        }],
    );
    while let Some((_, mut nodes)) = buckets.pop_first() {
        nodes.sort_by_key(|n| (std::cmp::Reverse(n.progress()), n.jumps));
        let mut seen = BTreeSet::new();
        nodes.retain(|n| seen.insert(StateKey::of(&n.game)));
        let nodes = select_beam(nodes, REPLAN_BEAM);
        for node in expand_all(&nodes, threads).into_iter().flatten().flatten() {
            let f = node.game.frame();
            if f > deadline {
                continue;
            }
            if reached(&node.game) {
                return true;
            }
            buckets.entry(f).or_default().push(node);
        }
    }
    false
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

/// A jump of the route before measuring: its JUMP hold, checkpoint and the
/// state every shift of it starts from.
struct JumpAt {
    hold: (usize, usize),
    check: u32,
    /// The original route run up to frame `hold.0 - MAX_SHIFT` (or 0), the
    /// last state no shift changes: `Err(true)` if it completed the level
    /// before, `Err(false)` if it died before.
    start: Result<Game, bool>,
}

/// The jump windows of a route (see the module docs). Jumps are measured on
/// up to `threads` threads; the result does not depend on it.
pub fn jump_windows(
    level: &Level,
    tuning: &Tuning,
    inputs: &[InputFrame],
    threads: usize,
) -> Vec<JumpWindow> {
    let orig = trace(level, tuning, inputs);
    let climb = level.meta.camera == CameraMode::Climb;
    let total = inputs.len() as u32;
    let buffer = tuning.player.jump_buffer_frames as u32;
    let first = |ev: Event, from: u32, to: u32| {
        orig.events
            .iter()
            .find(|&&(f, e)| e == ev && f >= from && f <= to)
            .map(|&(f, _)| f)
    };
    // Presses that make the player jump; checkpoint = the first landing
    // after that `Jumped` + 2 s, capped at the end of the route. The start
    // states come from one run of the original route (holds are in order).
    let mut run: Result<Game, bool> = Ok(Game::new(level, tuning, 1));
    let jumps: Vec<JumpAt> = jump_holds(inputs)
        .into_iter()
        .filter_map(|(s, e)| {
            let press = s as u32 + 1;
            let jumped = first(Event::Jumped, press, press + buffer)?;
            let landing = first(Event::Landed, jumped + 1, total).unwrap_or(total);
            let from = s.saturating_sub(MAX_SHIFT as usize) as u32;
            while let Ok(g) = &mut run
                && g.frame() < from
            {
                let events = g.step(inputs[g.frame() as usize]);
                if events.contains(&Event::Died) {
                    run = Err(false);
                } else if g.mode() == Mode::StageClear {
                    run = Err(true);
                }
            }
            Some(JumpAt {
                hold: (s, e),
                check: landing.saturating_add(CHECKPOINT_AFTER_LANDING).min(total),
                start: run.clone(),
            })
        })
        .collect();

    // The jumps are independent: a shared counter hands them to the
    // threads, the results are put back in jump order.
    let next = AtomicUsize::new(0);
    let worker = || {
        let mut measured = Vec::new();
        loop {
            let i = next.fetch_add(1, Ordering::Relaxed);
            let Some(jump) = jumps.get(i) else {
                return measured;
            };
            measured.push((i, window(tuning, climb, inputs, &orig, jump)));
        }
    };
    let mut out: Vec<(usize, JumpWindow)> = if threads <= 1 || jumps.len() < 2 {
        worker()
    } else {
        std::thread::scope(|s| {
            let handles: Vec<_> = (0..threads.min(jumps.len()))
                .map(|_| s.spawn(worker))
                .collect();
            handles
                .into_iter()
                .flat_map(|h| h.join().expect("jump window thread panicked"))
                .collect()
        })
    };
    out.sort_by_key(|&(i, _)| i);
    out.into_iter().map(|(_, w)| w).collect()
}

/// Measures the window of one jump of the route `inputs` (D-033).
fn window(
    tuning: &Tuning,
    climb: bool,
    inputs: &[InputFrame],
    orig: &Trace,
    jump: &JumpAt,
) -> JumpWindow {
    let (s, e) = jump.hold;
    let check = jump.check;
    let goal = Goal {
        x: orig.x[check as usize] - PROGRESS_TOLERANCE_PX,
        feet: climb.then(|| orig.goal_feet(check as usize) + CLIMB_FEET_TOLERANCE_PX),
    };
    let deadline = check
        + if climb {
            CLIMB_REPLAN_MARGIN
        } else {
            REPLAN_MARGIN
        };
    let (lo, lo_bound, hi, hi_bound) = window_range(|d| {
        let Some(shifted) = shift_hold(inputs, s, e, d) else {
            return Shifted::Route;
        };
        let start = match &jump.start {
            Ok(g) => g,
            Err(true) => return Shifted::Ok,
            Err(false) => return Shifted::Fail,
        };
        // Fast path: the unchanged later inputs already get there and she
        // stands on the ground (like the re-planning goal).
        if let Some(p) = progress(start, &shifted, check)
            && goal.reached_by(p)
        {
            return Shifted::Ok;
        }
        let press = (s as i64 + i64::from(d)) as u32 + 1;
        let ok = match after_shifted_jump(start, tuning, &shifted, press, deadline) {
            // A beam of REPLAN_BEAM nodes is too small to gain from threads;
            // the jumps run in parallel instead.
            Ok(game) => recovers(game, goal, deadline, 1),
            Err(done) => done,
        };
        if ok { Shifted::Ok } else { Shifted::Fail }
    });
    JumpWindow {
        index: s,
        hold: (e - s) as u32,
        x: orig.x[s],
        lo,
        hi,
        lo_bound,
        hi_bound,
    }
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
            start: jnr_core::Start::Playing,
            route: Vec::new(),
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
    measure(level, tuning, search(level, tuning, opts), opts.threads)
}

/// The part of [`solve`] after the search: if it is complete, cleans up
/// the route and measures the jump windows on `threads` threads. No clock
/// runs here.
pub fn measure(level: &Level, tuning: &Tuning, search: Search, threads: usize) -> Solution {
    let mut inputs = search.best.inputs();
    let windows = if search.complete() {
        inputs = clean_route(level, tuning, &inputs);
        jump_windows(level, tuning, &inputs, threads)
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
    /// Threads for one level's search and jump windows (default: up to
    /// `MAX_THREADS`); the result does not depend on it.
    threads: Option<usize>,
}

/// `cargo xtask solve <level-id|--all> [--write] [--shots] [--beam N] [--time-limit S]`.
pub fn run(root: &Path, args: &[String]) -> ExitCode {
    let mut a = Args {
        target: None,
        write: false,
        shots: false,
        beam: None,
        time_limit: None,
        threads: None,
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
    let sol = solve(level, &tuning, &options(level, &tuning, a));
    report(root, level, &tuning, sol, a, out)
}

/// [`SolveOptions::for_level`] with the overrides from `a`.
fn options(level: &Level, tuning: &Tuning, a: &Args) -> SolveOptions {
    let mut opts = SolveOptions::for_level(level, tuning);
    if let Some(b) = a.beam {
        opts.beam_width = b;
    }
    if let Some(t) = a.time_limit {
        opts.time_limit = t;
    }
    if let Some(n) = a.threads {
        opts.threads = n;
    }
    opts
}

/// Writes the image, report and (with `--write`) the replay of a solved
/// (or stuck) `level` into `out`; returns (solution, printable text).
fn report(
    root: &Path,
    level: &Level,
    tuning: &Tuning,
    sol: Solution,
    a: &Args,
    out: &OutDirs,
) -> Result<(Solution, String), String> {
    let id = &sol.level;
    let mut files = Files::default();

    let texts = content::texts(root)?;
    let ctx = RenderContext::for_level(jnr_assets::atlas(), &texts, level);
    let dir = &out.preview;
    fs::create_dir_all(dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
    if sol.search.complete() {
        if let Some(w) = narrowest(&sol.windows) {
            let mut g = Game::new(level, tuning, 1);
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

/// Why a solved route breaks the level's `min_jump_window` (D-030): its
/// narrowest jump window (see [`narrowest`]) is smaller. `None` if fine,
/// the level sets no minimum, or the search failed (reported elsewhere).
pub fn window_problem(level: &Level, sol: &Solution) -> Option<String> {
    let min = level.meta.min_jump_window?;
    if !sol.search.complete() {
        return None;
    }
    let w = narrowest(&sol.windows).filter(|w| w.size() < min)?;
    Some(format!(
        "FAIL {}: narrowest jump window {} frames < min_jump_window {min} (jump at frame {}, x={}, bounds {}/{}; see the {}_narrowest image)",
        level.meta.id,
        w.size(),
        w.frame(),
        w.x,
        w.lo_bound.as_str(),
        w.hi_bound.as_str(),
        level.meta.id
    ))
}

/// The `solver` verify step: every level must be solved, have its
/// committed `tests/replays/<id>_solver.replay`, and keep its narrowest
/// jump window at least `min_jump_window` (if set, [`window_problem`]).
/// Returns (ok, output, note with the narrowest jump window per level).
pub fn verify(root: &Path) -> (bool, String, String) {
    match all_levels(root) {
        Ok(l) if l.is_empty() => (false, "no levels in content/levels".into(), String::new()),
        Ok(levels) => verify_into(root, &OutDirs::of(root), &levels, crate::par::threads()),
        Err(e) => (false, e, String::new()),
    }
}

/// [`verify`] for `levels`, writing into `dirs`, with `threads` threads in
/// all. First every level's search (the only part with a clock) runs, in
/// parallel and largest level first, with the threads split between the
/// levels running at once and each search ([`split_threads`]), so no more
/// threads are busy than there are cores. Only then do the route clean-ups
/// and jump windows (no clock) run, every level at once with up to
/// `threads` threads each. Neither changes a route
/// (see the module doc), and the output is in `levels` order (D-082).
fn verify_into(
    root: &Path,
    dirs: &OutDirs,
    levels: &[Level],
    threads: usize,
) -> (bool, String, String) {
    let tuning = match content::tuning(root) {
        Ok(t) => t,
        Err(e) => return (false, e, String::new()),
    };
    let (outer, inner) = split_threads(threads, levels.len());
    let a = Args {
        target: None,
        write: false,
        shots: false,
        beam: None,
        time_limit: None,
        threads: Some(inner),
    };
    // Pass 1: every level's timed search, `outer` levels at a time with
    // `inner` threads each (about one thread per core), largest first.
    let order = largest_first(levels);
    let found = crate::par::map_ordered(&order, outer, |&i| {
        Mutex::new(Some(search(
            &levels[i],
            &tuning,
            &options(&levels[i], &tuning, &a),
        )))
    });
    // Pass 2 (no clock): route clean-up and jump windows, every level at
    // once with up to `threads` threads each (largest first again).
    let threads = threads.max(1);
    let jobs: Vec<(usize, Mutex<Option<Search>>)> = order.into_iter().zip(found).collect();
    let measured = crate::par::map_ordered(&jobs, threads, |(i, s)| {
        let s = s
            .lock()
            .expect("one job per level")
            .take()
            .expect("searched once");
        (*i, measure(&levels[*i], &tuning, s, threads))
    });
    let mut solutions: Vec<Option<Solution>> = levels.iter().map(|_| None).collect();
    for (i, sol) in measured {
        solutions[i] = Some(sol);
    }
    // The reports and checks, in level order.
    let (mut ok, mut out, mut notes) = (true, String::new(), Vec::new());
    for (level, sol) in levels.iter().zip(solutions) {
        let sol = sol.expect("every level is measured once");
        match report(root, level, &tuning, sol, &a, dirs) {
            Ok((sol, text)) => {
                ok &= sol.search.complete();
                out.push_str(&text);
                notes.push(sol.note());
                if let Some(p) = window_problem(level, &sol) {
                    ok = false;
                    out.push_str(&format!("{p}\n"));
                }
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

/// Splits `threads` into (levels solved at once, threads per level): about
/// `LEVEL_THREADS` per level, as many levels at once as that allows (at
/// most `levels`), the rest spread over them (at most `MAX_THREADS` each).
fn split_threads(threads: usize, levels: usize) -> (usize, usize) {
    let threads = threads.max(1);
    let outer = (threads / LEVEL_THREADS).clamp(1, levels.max(1));
    let inner = (threads / outer).clamp(1, MAX_THREADS);
    (outer, inner)
}

/// Indices of `levels`, the largest (width × height in tiles) first; ties
/// keep their order.
fn largest_first(levels: &[Level]) -> Vec<usize> {
    let mut order: Vec<usize> = (0..levels.len()).collect();
    order.sort_by_key(|&i| std::cmp::Reverse(levels[i].width.max(0) * levels[i].height.max(0)));
    order
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
            ..SolveOptions::for_level(level, &tuning())
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

    /// Fresh output directories under
    /// `$CARGO_TARGET_DIR/tmp/xtask-solver-test/<pid>/<name>`, so tests never
    /// touch the real preview/reports/replays folders, and two test runs at
    /// the same time (`check` and `verify`) never delete each other's files.
    fn tmp_dirs(name: &str) -> OutDirs {
        let all = crate::target_dir(&root()).join("tmp/xtask-solver-test");
        // Left over from finished test runs (no /proc/<pid> any more) or from
        // before the per-process folders (any non-numeric name). Without
        // /proc (not Linux) this removes nothing.
        if Path::new("/proc/self").exists() {
            for entry in fs::read_dir(&all).into_iter().flatten().flatten() {
                let name = entry.file_name();
                let is_pid = name.to_str().is_some_and(|p| p.parse::<u32>().is_ok());
                if !is_pid || !Path::new("/proc").join(&name).exists() {
                    let _ = fs::remove_dir_all(entry.path());
                }
            }
        }
        let base = all.join(std::process::id().to_string()).join(name);
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
            threads: None,
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
        assert_eq!(select_beam(vec![airborne.clone(); 3], 4).len(), 3);

        // High reserve: a node higher than every kept node in its column
        // survives the cut (one per quarter of the width), lower ones don't.
        let mut low_first = vec![grounded.clone(); 6];
        low_first.extend([airborne.clone(), airborne.clone(), grounded.clone()]);
        let beam = select_beam(low_first, 4);
        assert_eq!(on_ground(&beam), [true, true, true, true, false]);
        // In another column it counts on its own.
        let mut far = grounded.clone();
        let centre = far.game.player().center_px().0;
        far.game.teleport_player(centre + HEIGHT_COLUMN_PX);
        far.game.step(InputFrame::JUMP);
        let mut ranked = vec![far; 4];
        ranked.extend([airborne.clone(), grounded]);
        let beam = select_beam(ranked, 4);
        assert_eq!(
            beam.len(),
            6,
            "a higher node in the column and the grounded reserve"
        );
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
        let w = jump_windows(&level, &tuning(), &inputs, 1);
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

        let w = jump_windows(&level, &t, &inputs, 1);
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
        let w = jump_windows(&level, &t, &inputs, 1);
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

    /// D-033: jump 1 is a hop while still accelerating (air accel < ground
    /// accel), so its timing moves the player by a few pixels for good.
    /// Jump 2, before jump 1's checkpoint, is pressed on the last coyote
    /// frame before a pit. With every later press fixed, a later hop makes
    /// jump 2 too late and the player dies (the old metric: a narrow
    /// window). Re-planning after the hop's landing simply jumps a frame
    /// earlier, so jump 1's window is wide.
    #[test]
    fn a_fixed_later_press_no_longer_narrows_the_window() {
        let w = 60;
        let air = ".".repeat(w);
        let mut top = ".".repeat(w);
        top.replace_range(1..2, "P");
        top.replace_range(w - 2..w - 1, "G");
        let pit_col = 16;
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
        // The hop's landing, then the first airborne step at the pit edge.
        let mut probe = Game::new(&level, &t, 1);
        let (mut landed, mut left_ground) = (0, 0);
        for i in 0..400 {
            let input = prefix.get(i).copied().unwrap_or(RB);
            let events = probe.step(input).to_vec();
            if landed == 0 && events.contains(&Event::Landed) {
                landed = probe.frame();
            } else if landed > 0 && !probe.player().on_ground {
                left_ground = i;
                break;
            }
        }
        assert!(landed > 0 && left_ground > 0);
        let press2 = left_ground + t.player.coyote_frames as usize - 1;
        assert!(
            (press2 as u32) < landed + CHECKPOINT_AFTER_LANDING,
            "jump 2 comes before jump 1's checkpoint"
        );
        let mut inputs = prefix.clone();
        inputs.resize(press2, RB);
        inputs.extend([RBJ; 20]);
        inputs.extend([RB; 200]);
        let windows = jump_windows(&level, &t, &inputs, 1);
        assert_eq!(windows.len(), 2, "{windows:?}");
        let w1 = &windows[0];

        // With the later inputs fixed, some later hops die at the pit ...
        let (s, e) = (w1.index, w1.index + 1);
        let dies = |d: i32| {
            let shifted = shift_hold(&inputs, s, e, d).unwrap();
            let mut g = Game::new(&level, &t, 1);
            shifted.iter().any(|&i| g.step(i).contains(&Event::Died))
        };
        let fixed_deaths: Vec<i32> = (1..=MAX_SHIFT).filter(|&d| dies(d)).collect();
        assert!(
            !fixed_deaths.is_empty(),
            "scenario: a later hop ruins the fixed jump 2"
        );
        // ... before jump 1's checkpoint, so the fast path fails ...
        let shifted = shift_hold(&inputs, s, e, fixed_deaths[0]).unwrap();
        let check = landed + CHECKPOINT_AFTER_LANDING;
        assert_eq!(
            progress(&Game::new(&level, &t, 1), &shifted, check),
            None,
            "dies by {check}"
        );
        // ... but the window re-plans and reaches +12.
        assert_eq!(
            (w1.hi, w1.hi_bound),
            (MAX_SHIFT, Bound::Max),
            "{w1:?} vs fixed deaths at {fixed_deaths:?}"
        );
        assert_eq!(w1.lo_bound, Bound::Route, "{w1:?}");
        // Jump 2 itself is a real coyote-limited jump: still narrow on the
        // late side.
        let w2 = &windows[1];
        assert_eq!(w2.hi_bound, Bound::Fail, "{w2:?}");
        assert!(w2.hi <= 1, "{w2:?}");
    }

    /// A 70-tile level with three 3-tile pits: a route with several jumps.
    fn three_pit_level() -> Level {
        let w = 70;
        let air = ".".repeat(w);
        let mut top = ".".repeat(w);
        top.replace_range(1..2, "P");
        top.replace_range(w - 2..w - 1, "G");
        let mut ground = "#".repeat(w);
        for pit in [14, 30, 46] {
            ground.replace_range(pit..pit + 3, "...");
        }
        level([&air, &air, &air, &top], &ground)
    }

    #[test]
    fn jump_windows_do_not_depend_on_the_thread_count() {
        let level = three_pit_level();
        let t = tuning();
        let sol = solve(&level, &t, &opts(&level, 1));
        assert!(sol.search.complete(), "{}", sol.summary());
        let one = jump_windows(&level, &t, &sol.inputs, 1);
        assert!(one.len() >= 3, "a jump per pit: {one:?}");
        assert_eq!(one, sol.windows);
        for threads in [2, 3, 8] {
            assert_eq!(
                jump_windows(&level, &t, &sol.inputs, threads),
                one,
                "{threads} threads"
            );
        }
    }

    #[test]
    fn progress_reports_ground_and_does_not_depend_on_the_start_frame() {
        // Standing jump at input index 5 on flat ground.
        let level = gap_level(0);
        let t = tuning();
        let mut inputs = vec![NONE; 5];
        inputs.push(InputFrame::JUMP);
        inputs.extend([NONE; 100]);
        let start = Game::new(&level, &t, 1);
        let mid_air = progress(&start, &inputs, 12).unwrap();
        assert!(!mid_air.on_ground && !mid_air.done, "{mid_air:?}");
        let landed = progress(&start, &inputs, 100).unwrap();
        assert!(landed.on_ground && !landed.done, "{landed:?}");
        // From a later state of the same route: the same result.
        let mut later = start.clone();
        for &i in &inputs[..4] {
            later.step(i);
        }
        assert_eq!(progress(&later, &inputs, 12), Some(mid_air));
        assert_eq!(progress(&later, &inputs, 100), Some(landed));
        // Past the end of the inputs: where the route ends.
        assert_eq!(progress(&start, &inputs, 10_000), Some(landed));
    }

    #[test]
    fn after_shifted_jump_stops_at_the_landing_or_the_buffer_end() {
        let level = gap_level(0);
        let t = tuning();
        let buffer = t.player.jump_buffer_frames as u32;
        let start = Game::new(&level, &t, 1);
        // A standing jump pressed at input index 5 (frame 6): stops right
        // after the landing step.
        let mut inputs = vec![NONE; 5];
        inputs.push(InputFrame::JUMP);
        inputs.extend([NONE; 100]);
        let g = after_shifted_jump(&start, &t, &inputs, 6, 200).unwrap();
        assert!(g.events().contains(&Event::Landed), "{:?}", g.events());
        assert!(g.player().on_ground);
        assert!(g.frame() > 6 + 10, "landed at {}", g.frame());
        let landing = g.frame();
        // The same from a later start state of the route.
        let mut later = start.clone();
        for &i in &inputs[..3] {
            later.step(i);
        }
        let g = after_shifted_jump(&later, &t, &inputs, 6, 200).unwrap();
        assert_eq!(g.frame(), landing);
        // Deadline before the landing: failure.
        assert_eq!(
            after_shifted_jump(&start, &t, &inputs, 6, landing - 1).err(),
            Some(false)
        );
        // A press high in the air makes no jump: stops when its buffer ran
        // out (the second press, at index 15, is far from any landing).
        let mut air_press = inputs.clone();
        air_press[15] = InputFrame::JUMP;
        let g = after_shifted_jump(&start, &t, &air_press, 16, 200).unwrap();
        assert_eq!(g.frame(), 16 + buffer);
        assert!(!g.player().on_ground);
        // Running into a pit before the press: died.
        let pit = gap_level(4);
        let g0 = Game::new(&pit, &t, 1);
        let run = vec![RB; 200];
        assert_eq!(
            after_shifted_jump(&g0, &t, &run, 250, 300).err(),
            Some(false)
        );
        // Running to the goal before the press: completed (success).
        let run = vec![RB; 400];
        assert_eq!(
            after_shifted_jump(&start, &t, &run, 900, 1000).err(),
            Some(true)
        );
    }

    #[test]
    fn replanning_finds_a_way_or_reports_none() {
        // From the start of a level with a 3-tile gap the re-planning
        // search gets past the gap; across an 11-tile gap it can't.
        let t = tuning();
        let threads = 1;
        let ok = gap_level(3);
        let g = Game::new(&ok, &t, 1);
        assert!(recovers(
            g,
            Goal {
                x: 20 * 16,
                feet: None
            },
            400,
            threads
        ));
        let wide = gap_level(11);
        let g = Game::new(&wide, &t, 1);
        assert!(!recovers(
            g,
            Goal {
                x: 26 * 16,
                feet: None
            },
            400,
            threads
        ));
        // Already there: true without searching.
        let g = Game::new(&ok, &t, 1);
        let x = g.player().x.to_px();
        assert!(recovers(g, Goal { x, feet: None }, 0, threads));
    }

    fn climb_test_level() -> Level {
        content::find_level(&root(), "climb_test").unwrap()
    }

    #[test]
    fn climb_levels_rank_by_x_plus_the_height_gained() {
        let t = tuning();
        let side = Game::new(&gap_level(3), &t, 1);
        assert_eq!(search_progress(&side), side.player().x.raw());
        let level = climb_test_level();
        let mut g = Game::new(&level, &t, 1);
        assert_eq!(search_progress(&g), g.player().x.raw(), "at the start");
        // Teleported onto the row-21 plank in the shaft: 3 tiles above the
        // start terrace (row 24).
        g.teleport_player(37 * 16 + 8);
        assert_eq!(g.player().feet().1.to_px(), 21 * 16);
        assert_eq!(search_progress(&g), g.player().x.raw() + 48 * 256);
        // The budget covers the climb too.
        let budget = SolveOptions::for_level(&level, &t).frame_budget;
        assert_eq!(budget, 3 * (64 + 40) * 16);
        assert_eq!(
            SolveOptions::for_level(&gap_level(3), &t).frame_budget,
            3 * gap_level(3).width as u32 * 16
        );
    }

    #[test]
    fn the_budget_covers_the_auto_scroll() {
        let t = tuning();
        let level = content::find_level(&root(), "m5_autoscroll").unwrap();
        // Zone [8, 80]: 1152 px at 192 subpx per frame.
        assert_eq!(scroll_frames(&level, &t), 1536);
        assert_eq!(
            SolveOptions::for_level(&level, &t).frame_budget,
            3 * 112 * 16 + 1536
        );
        assert_eq!(scroll_frames(&gap_level(3), &t), 0);
        // Rounded up: 1 px more is 2 frames more (1 px = 256 subpx).
        let mut slow = t;
        slow.camera.autoscroll_speed = jnr_core::Fx(100);
        assert_eq!(scroll_frames(&level, &slow), (1152 * 256_u32).div_ceil(100));
    }

    #[test]
    fn climb_recovery_also_needs_the_height() {
        let level = climb_test_level();
        let mut g = Game::new(&level, &tuning(), 1);
        g.teleport_player(37 * 16 + 8);
        let (x, feet) = (g.player().x.to_px(), g.player().feet().1.to_px());
        let goal = |feet| Goal { x: x - 16, feet };
        let f = g.frame();
        // Where she stands: reached with or without the height check.
        assert!(recovers(g.clone(), goal(None), f, 1));
        assert!(recovers(g.clone(), goal(Some(feet + 8)), f, 1));
        // One plank up (row 18, 3 tiles): she has to jump for it.
        assert!(!recovers(g.clone(), goal(Some(feet - 48 + 8)), f, 1));
        assert!(recovers(g.clone(), goal(Some(feet - 48 + 8)), f + 150, 1));
        // Above the plateau: never (and the goal is too far for 60 frames).
        assert!(!recovers(g, goal(Some(15 * 16 - 64)), f + 60, 1));
        // The fast-path check: x alone is not enough in a climb level.
        let p = Progress {
            x,
            feet: feet + 48,
            done: false,
            on_ground: true,
        };
        assert!(goal(None).reached_by(p));
        assert!(!goal(Some(feet + 8)).reached_by(p));
        assert!(goal(Some(feet + 8)).reached_by(Progress { done: true, ..p }));
        assert!(!goal(None).reached_by(Progress {
            on_ground: false,
            ..p
        }));
    }

    /// A climb level with flat ground (14 rows, so the climb camera has a
    /// full screen): nothing can fail a shift.
    fn flat_climb_level() -> Level {
        let w = 80;
        let mut rows = vec![".".repeat(w); 12];
        rows[11].replace_range(1..2, "P");
        rows[11].replace_range(w - 2..w - 1, "G");
        rows.extend(["#".repeat(w), "#".repeat(w)]);
        let text = format!(
            "[meta]\nid = \"flat_climb\"\ntime_limit = 100\ncamera = \"climb\"\n[legend]\n\
             \"#\" = \"solid\"\n\"P\" = {{ entity = \"player_start\" }}\n\
             \"G\" = {{ entity = \"goal\" }}\n[map]\ngrid = '''\n{}\n'''\n",
            rows.join("\n")
        );
        Level::parse(&text).unwrap()
    }

    /// E17 bug: a jump followed by another one within 2 s has its
    /// checkpoint (landing + 120) in the second jump's air. The climb
    /// height goal used the feet there, a point in the air that no run on
    /// flat ground can stand at, so every shift failed (window 0..0).
    #[test]
    fn a_climb_checkpoint_in_the_air_uses_the_ground_around_it() {
        let level = flat_climb_level();
        let t = tuning();
        let r = InputFrame::RIGHT;
        let rj = r.with(InputFrame::JUMP);
        let mut inputs = vec![r; 20];
        inputs.extend([rj; 12]);
        // Run to jump 1's landing.
        let mut g = Game::new(&level, &t, 1);
        for &i in &inputs {
            g.step(i);
        }
        while !g.step(r).contains(&Event::Landed) {
            inputs.push(r);
        }
        inputs.push(r);
        let landing = inputs.len();
        // Jump 2 is pressed 110 frames after it, so the checkpoint is 10
        // frames into jump 2's air.
        inputs.extend(vec![r; 109]);
        inputs.extend([rj; 12]);
        inputs.extend(vec![r; 80]);
        let check = landing + CHECKPOINT_AFTER_LANDING as usize;
        let orig = trace(&level, &t, &inputs);
        assert!(!orig.ground[check], "the checkpoint is mid-air");
        assert!(orig.ground[landing] && orig.ground[check - 11]);
        assert!(orig.feet[check] < orig.feet[landing] - 8, "high in the air");
        assert_eq!(orig.goal_feet(check), orig.feet[landing], "the ground");
        let w = jump_windows(&level, &t, &inputs, 1);
        assert_eq!(w.len(), 2);
        assert_eq!((w[0].index, w[0].lo, w[0].hi), (20, -MAX_SHIFT, MAX_SHIFT));
        assert_eq!((w[0].lo_bound, w[0].hi_bound), (Bound::Max, Bound::Max));
    }

    /// Mid-air at the checkpoint, the height goal is the lower of the
    /// ground she left and the ground she lands on next (a jump up keeps
    /// the ledge she jumped from; a fall keeps the floor she falls to, so a
    /// shifted run that is already down there is not "too low").
    #[test]
    fn goal_feet_is_the_lower_ground_around_an_airborne_frame() {
        let t = |feet: &[i32], ground: &[bool]| Trace {
            x: vec![0; feet.len()],
            feet: feet.to_vec(),
            ground: ground.to_vec(),
            events: Vec::new(),
        };
        let (y, n) = (true, false);
        // Grounded: its own feet.
        assert_eq!(t(&[100, 90], &[y, y]).goal_feet(1), 90);
        // Jump up from 100 onto 52: the ledge she left.
        let up = t(&[100, 80, 60, 52], &[y, n, n, y]);
        assert_eq!(up.goal_feet(1), 100);
        assert_eq!(up.goal_feet(2), 100);
        // Walk off 52 down to 148: the floor she falls to.
        let down = t(&[52, 60, 100, 148], &[y, n, n, y]);
        assert_eq!(down.goal_feet(2), 148);
        // Only one side grounded, or none: what there is.
        assert_eq!(t(&[52, 60, 100], &[y, n, n]).goal_feet(2), 52);
        assert_eq!(t(&[60, 100, 148], &[n, n, y]).goal_feet(0), 148);
        assert_eq!(t(&[60, 100], &[n, n]).goal_feet(1), 100);
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

    /// Synthetic levels for the `verify_into` tests: quick to solve, no
    /// `min_jump_window`, of different sizes (so `largest_first` reorders).
    fn verify_levels() -> Vec<Level> {
        [(2, "syn_small"), (5, "syn_large"), (3, "syn_mid")]
            .into_iter()
            .map(|(gap, id)| {
                let mut l = gap_level(gap);
                l.meta.id = id.into();
                l
            })
            .collect()
    }

    /// The `SOLVED`/`STUCK`/`FAIL` lines of a verify output, without the
    /// runtimes (a summary's last item `, <digits>.<digit>s`).
    fn result_lines(out: &str) -> Vec<String> {
        out.lines()
            .filter(|l| {
                ["SOLVED ", "STUCK ", "FAIL "]
                    .iter()
                    .any(|p| l.starts_with(p))
            })
            .map(|l| match l.rsplit_once(", ") {
                Some((head, tail)) if is_runtime(tail) => head.to_owned(),
                _ => l.to_owned(),
            })
            .collect()
    }

    /// `\d+\.\ds` (the summary's `{:.1}s`).
    fn is_runtime(s: &str) -> bool {
        let Some((int, frac)) = s.strip_suffix('s').and_then(|n| n.split_once('.')) else {
            return false;
        };
        let digits = |t: &str| !t.is_empty() && t.bytes().all(|b| b.is_ascii_digit());
        digits(int) && frac.len() == 1 && digits(frac)
    }

    #[test]
    fn result_lines_strip_only_the_runtime() {
        let out = "SOLVED a: complete at frame 9, 1 jumps, narrowest 5, 0 search deaths, 12.3s\n  \
                   report x\nSTUCK b: dead end – furthest x=3 px at frame 1, 2 search deaths, 0.0s\n\
                   FAIL c: missing solver replay x, y\nFAIL d: ends in 1.5s";
        assert_eq!(
            result_lines(out),
            [
                "SOLVED a: complete at frame 9, 1 jumps, narrowest 5, 0 search deaths",
                "STUCK b: dead end – furthest x=3 px at frame 1, 2 search deaths",
                "FAIL c: missing solver replay x, y",
                "FAIL d: ends in 1.5s",
            ]
        );
    }

    /// The `solver` step itself runs over every real level in `verify`;
    /// this checks its bookkeeping on synthetic levels (it used to solve
    /// every real level twice inside `cargo test`, D-082).
    #[test]
    fn verify_fails_without_the_committed_solver_replay() {
        let dirs = tmp_dirs("verify_missing_replay");
        let levels = verify_levels();
        let (ok, out, note) = verify_into(&root(), &dirs, &levels, 4);
        assert!(!ok, "{out}");
        for l in &levels {
            let id = &l.meta.id;
            let replay = dirs.replays.join(format!("{id}_solver.replay"));
            assert!(
                out.contains(&format!(
                    "FAIL {id}: missing solver replay {} (create it with `cargo xtask solve {id} --write`",
                    replay.display()
                )),
                "{id}: {out}"
            );
            assert!(note.contains(&format!("{id}: ")), "{note}");
        }
        // With the replays present (any content: the `replays` step checks them).
        for l in &levels {
            let id = &l.meta.id;
            fs::write(dirs.replays.join(format!("{id}_solver.replay")), "").unwrap();
        }
        let (ok, out, _) = verify_into(&root(), &dirs, &levels, 4);
        assert!(ok, "{out}");
        assert!(!out.contains("FAIL "), "{out}");
    }

    #[test]
    fn verify_output_is_in_level_order_for_any_thread_count() {
        let dirs = tmp_dirs("verify_threads");
        let levels = verify_levels();
        let (ok1, out1, note1) = verify_into(&root(), &dirs, &levels, 1);
        let first = result_lines(&out1);
        let solved: Vec<&str> = first
            .iter()
            .filter_map(|l| l.strip_prefix("SOLVED "))
            .map(|l| l.split(':').next().unwrap())
            .collect();
        assert_eq!(solved, ["syn_small", "syn_large", "syn_mid"], "{out1}");
        for threads in [2, 3, 9, 14] {
            let (ok, out, note) = verify_into(&root(), &dirs, &levels, threads);
            assert_eq!(ok, ok1, "{threads} threads");
            assert_eq!(result_lines(&out), first, "{threads} threads");
            assert_eq!(note, note1, "{threads} threads");
        }
    }

    #[test]
    fn verify_reports_a_stuck_level_among_solved_ones() {
        let dirs = tmp_dirs("verify_stuck");
        let mut levels = verify_levels();
        let mut broken = gap_level(14);
        broken.meta.id = "syn_broken".into();
        levels.insert(1, broken);
        for l in &levels {
            let id = &l.meta.id;
            fs::write(dirs.replays.join(format!("{id}_solver.replay")), "").unwrap();
        }
        let (ok, out, _) = verify_into(&root(), &dirs, &levels, 8);
        assert!(!ok, "{out}");
        let lines = result_lines(&out);
        assert_eq!(lines.len(), 4, "{out}");
        assert!(lines[0].starts_with("SOLVED syn_small:"), "{out}");
        assert!(lines[1].starts_with("STUCK syn_broken:"), "{out}");
        assert!(lines[2].starts_with("SOLVED syn_large:"), "{out}");
        assert!(lines[3].starts_with("SOLVED syn_mid:"), "{out}");
    }

    #[test]
    fn threads_are_split_between_levels_and_searches() {
        // (threads, levels) -> (levels at once, threads per level)
        assert_eq!(split_threads(14, 17), (2, 7));
        assert_eq!(split_threads(14, 1), (1, MAX_THREADS));
        assert_eq!(split_threads(8, 17), (1, MAX_THREADS));
        assert_eq!(split_threads(32, 17), (4, MAX_THREADS));
        assert_eq!(split_threads(64, 1), (1, MAX_THREADS));
        assert_eq!(split_threads(2, 17), (1, 2));
        assert_eq!(split_threads(1, 5), (1, 1));
        assert_eq!(split_threads(0, 0), (1, 1));
    }

    #[test]
    fn largest_levels_are_solved_first() {
        assert_eq!(largest_first(&verify_levels()), vec![1, 2, 0]);
        let same = [gap_level(2), gap_level(2)];
        assert_eq!(largest_first(&same), vec![0, 1], "ties keep their order");
    }

    #[test]
    fn a_window_below_min_jump_window_fails() {
        let mut level = gap_level(3);
        let t = tuning();
        let sol = solve(&level, &t, &opts(&level, 4));
        let w = narrowest(&sol.windows)
            .expect("one jump over the gap")
            .clone();
        assert_eq!(window_problem(&level, &sol), None, "no minimum set");
        level.meta.min_jump_window = Some(w.size());
        assert_eq!(window_problem(&level, &sol), None, "exactly the minimum");
        level.meta.min_jump_window = Some(w.size() + 1);
        let p = window_problem(&level, &sol).expect("too narrow");
        let id = &level.meta.id;
        assert!(
            p.starts_with(&format!(
                "FAIL {id}: narrowest jump window {} frames < min_jump_window {} (jump at frame {}, x={}, ",
                w.size(),
                w.size() + 1,
                w.frame(),
                w.x
            )),
            "{p}"
        );
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
