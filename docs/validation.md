# Validation: closing the agentic loop

Status: **approved v1**.

Goal: an agent can implement a feature and **prove** it works without a human looking at it. The project runs with **no human involvement**; milestones are accepted by the agents themselves (see `milestones.md`).

## Two entry points

| Command | When | Target time |
|---|---|---|
| `cargo xtask check` | after every change, enforced by a hook before an agent may finish its turn | < 60 s incremental |
| `cargo xtask verify` | before every commit / task completion | < 10 min |

Both write a machine-readable report to `$CARGO_TARGET_DIR/reports/{check,verify}.json` (here: `/workspaces/.cargo-target/reports/`) plus a short Markdown summary that agents read. Every failure includes the exact command to reproduce it.

## Layers

### L1 Static (check)
- `cargo fmt --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- Native build + `cargo build -p jnr-app --target wasm32-unknown-unknown`
- **Architecture lint**: `jnr-core` and `jnr-render` must not depend on macroquad; no `f32`/`f64` in `jnr-core` (except in tests); no `HashMap` in `jnr-core`; no `std::time`/`std::fs` in core; tooling-only functions (`Game::teleport_player`) are called only from xtask and tests. Implemented as a simple dependency-tree + grep check in xtask.

### L2 Unit tests (check)
- `cargo test --workspace`: physics, collision, entity behaviour, level parsing, sprite parsing.
- **Game-feel tests**: simulate the player with scripted inputs and assert the targets in `game-design.md` (e.g. "holding JUMP while running reaches 5 ± 0.25 tiles", "coyote jump works 6 frames after the edge").

### L3 Asset lints (check)
- `cargo xtask sprites lint` (see `art-pipeline.md`).
- `cargo xtask levels lint [id]`: unknown legend chars, exactly one player start and at least one goal, entities inside bounds, all text keys exist in `de.toml`, all referenced sprites exist, time limit set.
  - Implemented (M0): every `content/levels/*.level` parses (legend, grid, one player start via `Level::parse`), unique `meta.id`, `title_key` exists in `de.toml`, at least one goal, `time_limit` set. Sprite references and other text keys come with M1 (see `backlog.md`).

### L4 Level completability (verify)
- **Solver bot** (`cargo xtask solve <level>`): a beam search over the real `jnr-core` simulation. It clones `Game`, tries macro-actions (e.g. run right 8 frames, run+jump held N frames, wait), keeps the most promising states (furthest right, alive, highest score) and stops when `LevelComplete` happens.
- Output: a replay file that proves the level can be finished. A level the bot can't finish fails verification, with a report of the furthest point reached and a screenshot there.
- The bot also reports **difficulty signals**: number of frames, how narrow the successful jumps were (frame windows), and deaths during the search. These go into the level report for the `level-designer`.

### L5 Replay regression tests (verify)
Replay files in `tests/replays/` contain the inputs per frame (run-length encoded) and expectations:

```
level    = salzburg
seed     = 1
source   = solver           # or: human (recorded in VNC)
expect   = complete frames=2400..3600 oblaten>=30 deaths=0
shots    = 120:intro 900:getreidegasse 2000:fiaker
---
R 60
RJ 18
R 40
...
```

- Running a replay asserts the expected outcome.
- Every replay runs twice and compares `state_hash()` → determinism.
- Implemented (M0): `jnr_core::replay` parses/formats the format and checks expectations; `cargo xtask replay <file|name|--all> [--shots]` runs replays on the real content (level by `meta.id`). Supported expectations: `complete [frames=a..b]`, `deaths=N`, `deaths<=N`, `dies_at frames=a..b`, `player_x>=PX`, `state=<playing|dying|stage_clear>`, `no_event=<Event>`; anything else (e.g. `oblaten>=30` until M1) is a parse error "not supported yet". A replay without any `expect` fails in the runner ("no expectations"), and the `replays` step fails if `tests/replays/` has no replay files. Frame numbers are `Game::frame()` (frame 0 = before the first step), ranges are inclusive, and every replay runs to its last input before end-state expectations are checked (D-013).
- Later: the headless runner is also built for `wasm32-wasip1` and run with `wasmtime`, and its hash is compared to the native one → native/web determinism.
- Physics changes may break replays on purpose; then `cargo xtask solve` re-creates the solver replays, and the change must be explained in the commit message. Human-recorded replays (recorded in VNC with `--record`) are kept as "feel" references.

### L6 Golden screenshots (verify)
- The `shots` frames of each replay are rendered headlessly and compared **pixel-exactly** to `tests/golden/<replay>_<shot>.png` (`cargo xtask golden`; the `goldens` verify step).
- On mismatch: `$CARGO_TARGET_DIR/reports/golden/<name>_{actual,expected,diff}.png` (diff: changed pixels red over the darkened expected image). A missing golden fails too and writes the actual; so does a file in `tests/golden/` that no current shot uses, and two shots with the same golden name (`a_b` + `c` vs `a` + `b_c`).
- Review images before accepting: `cargo xtask replay <file> --shots` → `$CARGO_TARGET_DIR/preview/replays/<replay>_<shot>.png` (+ `_x4` copy).
- `cargo xtask golden --accept <replay>_<shot>|<replay>|--all` re-renders the shots (deterministic, so identical to the reviewed previews) and writes them into `tests/golden/`; it refuses replays that fail their expectations or are not deterministic.
- Updating goldens: `cargo xtask golden --accept <name>` is allowed only after an `art-director` APPROVED verdict for that change, and the commit message must state it.

### L7 Visual review (per visual change)
- The `art-director` agent reviews contact sheets, filmstrips and replay screenshots with a fixed rubric (readability, palette, silhouette, light direction, scale consistency, tile seams, HUD clipping, German spelling, "is the joke visible?").
- `cargo xtask filmstrip <replay>` makes a contact sheet of every 30th frame of a replay, so motion problems (jitter, clipping into walls) can be seen in one image.

### L8 Live smoke test (verify, last step)
- `cargo xtask vnc-smoke`: starts the real native binary on `DISPLAY=:1` with `--replay tests/replays/salzburg_solver.replay --exit-after N`, takes an X screenshot with ImageMagick `import -window root`, and checks that the process exited 0 and the screenshot isn't blank.
- This checks the macroquad path (window, texture upload, scaling) that headless tests can't reach.
- Later (web milestone): headless Chromium loads the wasm build and takes a screenshot.

## Hooks (configured in `.claude/settings.json` in Milestone 0)

| Hook | Action |
|---|---|
| `PostToolUse` on Edit/Write of `*.rs` | `rustfmt` on that file |
| `Stop` | runs `cargo xtask check`; on failure it blocks the stop and returns the summary, so the agent must fix the problem first. Guarded with `stop_hook_active` to avoid endless loops. |
| `PreToolUse` on Edit/Write in `tests/golden/` | blocked; goldens change only through `cargo xtask golden --accept` |

## Definition of done (per task)

1. `cargo xtask verify` is green.
2. New behaviour has a test (unit, game-feel, or replay).
3. Visual changes are APPROVED by `art-director`.
4. Code changes are reviewed by `code-reviewer` (architecture + determinism rules).
5. `docs/milestones.md` is updated; relevant docs and `docs/decisions.md` are updated.

## When agents are stuck

There is no human in the loop. Instead:
- After 3 failed rounds of the same fix or review, the orchestrator simplifies the task (smaller scope, simpler asset or mechanic), records the decision in `docs/decisions.md` and moves on. Deferred work goes into `docs/backlog.md`.
- Design questions are decided by the `game-designer` within the rules of `game-design.md` and recorded.
- Only permissively licensed dependencies (MIT, Apache-2.0, BSD, Zlib, CC0, CC-BY with attribution in `assets/LICENSES.md`). Anything else is not used.
- The only reasons to stop and wait for the human: something needs credentials, payment, or would delete/rewrite git history on `main`.
