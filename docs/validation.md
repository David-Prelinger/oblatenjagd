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
- `cargo xtask levels lint [id]`: unknown legend chars, exactly one player start and at least one goal, entities inside bounds, all text keys exist in `de.toml`, all referenced sprites exist, time limit set; Festungsbahn tracks: 3 free tiles above the car and a way back up (steps ≤ 3 tiles) from the pit under the track (D-028).
  - Implemented (M0): every `content/levels/*.level` parses (legend, grid, one player start via `Level::parse`), unique `meta.id`, `title_key` exists in `de.toml`, at least one goal, `time_limit` set. Sprite references and other text keys come with M1 (see `backlog.md`).

### L4 Level completability (verify)
- **Solver bot** (`cargo xtask solve <level>`): a beam search over the real `jnr-core` simulation. It clones `Game`, tries macro-actions (e.g. run right 8 frames, run+jump held N frames, wait), keeps the most promising states (furthest right, alive, highest score) and stops when `LevelComplete` happens.
- Output: a replay file that proves the level can be finished. A level the bot can't finish fails verification, with a report of the furthest point reached and a screenshot there.
- The bot also reports **difficulty signals**: number of frames, how narrow the successful jumps were (frame windows), and deaths during the search. These go into the level report for the `level-designer`.
- Implemented (M0, `xtask/src/solver.rs`, D-015):
  - **Search**: frame-synchronous beam search (default width 64): nodes are bucketed by frame and buckets processed in frame order; each bucket is ranked (furthest right, then fewer jumps, then insertion order), de-duplicated by a coarse state key (x/y in 4 px cells, vx in 1/4 px, vy in 1/2 px, grounded, plus `Game::world_hash`: changed tiles, gameplay entities in 4 px cells, for enemies, projectiles and the Festungsbahn also activation, state, facing and pattern counter, checkpoint, power state), cut to the beam width plus a reserve that keeps at least a quarter of the beam grounded and a "high" reserve (up to a quarter of the beam: a node whose feet are higher than every kept node's in its 16 px column, so a player waiting above a pit for the Festungsbahn, or riding it, survives against nodes further right down in the pit), and expanded with every macro-action. Macro-actions that die are pruned and counted as **search deaths**; touching the goal stops the macro. The first completion (earliest frame, then fewest jumps) wins. Fails when every branch dies, after the frame budget (3× level width in px) or the wall-clock limit (default 60 s, `--time-limit`). The route is deterministic: it does not depend on the thread count, and the clock is checked only while no completion has been found. Pass/fail can depend on machine speed if a search hits the limit, so the limit is generous (testbed: ~0.6 s with `opt-level = 2` for jnr-core and xtask in the dev profile).
  - **Action set**: `RB 4/8/16`, `R 8`, `- 4/16`, `L 4`, `LB 8`; running jump `RBJ k` (k = 1, 6, 12, 20, 36) + `RB 2`; walking jump `RJ k` (k = 1, 12, 36) + `R 2`; `LBJ 12/36` + `LB 2`; drop-through `DJ 1` + `- 3`.
  - **Route clean-up**: JUMP holds are removed or shortened where the player's position stays identical on every frame (mid-air presses, holds past the apex).
  - **Jump window**: for each JUMP press that makes the player jump, the whole held-JUMP segment is shifted by d ∈ [-12, +12] with the rest of the inputs unchanged (padded with the last input past the route's end). Checkpoint = first landing after the matching `Jumped` + 120 frames, capped at the finish. A shift succeeds (D-033) if the player survives the shifted jump and then reaches the original x at that checkpoint minus 16 px (or finishes) without dying. Either the unchanged later inputs get there by the checkpoint, standing on the ground (fast path), or a re-planning beam search (`solver::recovers`: the solver's macros, beam 12) does: it starts right after the shifted jump's landing (or after the jump buffer ran out if the press made no jump), and a node must stand on the ground at that x by the checkpoint + 30 frames. The window is the contiguous successful range around d = 0 (max 25). Each side records why it stopped: `fail` (a shift failed: the level's limit), `route` (the shift would start before frame 0 or touch a neighbouring JUMP hold) or `max`. Windows with a `route` side are lower bounds (`≥` in the report) and never count as the narrowest. A recovery may take a small detour (skip an item, take another platform; D-034). Each shift starts from a snapshot of the original route at frame press − 12, and the jumps are measured in parallel (the result does not depend on the thread count).
  - **Outputs**: `$CARGO_TARGET_DIR/reports/solver/<level>.{md,json}` (frames, search deaths, every jump with frame, x, hold, window, shift range and bounds; the narrowest; runtime), `preview/solver/<level>_narrowest.png` (state just before the narrowest jump's press) or `preview/solver/<level>_stuck.png` (furthest grounded state when it fails). `--write` writes `tests/replays/<level>_solver.replay` (`expect = complete frames=N..N deaths=0`, no shots; `--shots` adds `route_25/50/75`, which then need goldens after art-director approval).
  - **Verify step `solver`**: `cargo xtask solve --all`; fails if any level is not finished (the output names the stuck report and image) or has no committed `tests/replays/<level>_solver.replay` (create it with `cargo xtask solve <level> --write`); the step note lists the narrowest window per level. A per-level `min_jump_window` in `[meta]` is in `backlog.md` (the meta table rejects unknown keys).

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
- Implemented (M0): `jnr_core::replay` parses/formats the format and checks expectations; `cargo xtask replay <file|name|--all> [--shots]` runs replays on the real content (level by `meta.id`). Supported expectations: `complete [frames=a..b]`, `deaths=N`, `deaths<=N`, `dies_at frames=a..b`, `player_x>=PX`, `state=<playing|dying|stage_clear|game_over|title|stage_intro|paused>`, `no_event=<Event>`, `event=<Event>` (happened at least once; M1, E3), and (M1) `oblaten>=N` (HUD counter at the end), `score>=N`, `lives=N`; an optional header `start = playing|title` picks the flow (default `playing`: in play at once, no intros or pause; `title`: the real game from its title screen, D-025); `complete` is the first touch of the Maibaum and `state=stage_clear` holds for the whole goal sequence, which ends on the title screen; anything else is a parse error "not supported yet". A replay without any `expect` fails in the runner ("no expectations"), and the `replays` step fails if `tests/replays/` has no replay files. Frame numbers are `Game::frame()` (frame 0 = before the first step), ranges are inclusive, and every replay runs to its last input before end-state expectations are checked (D-013).
- Later: the headless runner is also built for `wasm32-wasip1` and run with `wasmtime`, and its hash is compared to the native one → native/web determinism.
- Physics changes may break replays on purpose; then `cargo xtask solve` re-creates the solver replays, and the change must be explained in the commit message. Human-recorded replays (recorded in VNC with `jnr-app --record <file>`, which writes `source = human` and a derived `expect` line so the replay passes right away) are kept as "feel" references.

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
- `cargo xtask vnc-smoke` (the last `verify` step): needs an X server on `$DISPLAY` (default `:1`; fails with "no X server answers on DISPLAY=…" otherwise). It builds the native app (debug) and runs it with `--replay tests/replays/testbed_route.replay --scale 4 --exit-after <last shot + 1>` and one `--screenshot-at <frame>:<file>` per shot of that replay, with a 60 s timeout (killed on timeout).
  - The app must exit 0, and every framebuffer screenshot (`$CARGO_TARGET_DIR/preview/vnc_smoke/<shot>.png`) must be **pixel-identical** to the golden `tests/golden/testbed_route_<shot>.png`: the real window path (replay input, fixed-step loop) produces exactly what the goldens show. On a mismatch `$CARGO_TARGET_DIR/reports/vnc-smoke/<name>_{actual,expected,diff}.png`; the app's output is in `preview/vnc_smoke/app.log`.
  - It refuses to start while a window titled `Oblatenjagd` is already open ("close the running game first"). The 60 s timeout counts from the app's start, not including the build.
  - While it runs (after `xdotool search` finds the new visible `Oblatenjagd` window by title, since miniquad sets no `_NET_WM_PID`, plus 1.5 s), `import -window root` writes `$CARGO_TARGET_DIR/preview/vnc_smoke_root.png`. The screenshot is cropped to the window's geometry from `xwininfo` (`xdotool getwindowgeometry` counts the title bar twice), and at least 25 % of that area must be the palette sky colour `s` (±8 per channel: the desktop is 16-bit RGB565).
  - Takes about 13 s (the replay plays in real time).
- This checks the macroquad path (window, texture upload, scaling) that headless tests can't reach.
- `cargo xtask wasm-smoke` (part of `verify`, before `vnc-smoke`): builds the release wasm, serves `$CARGO_TARGET_DIR/web/` locally and loads it in headless Chromium with SwiftShader WebGL. It checks the DOM marker `<body data-wasm="loaded">` (module instantiated) and the 1280x720 screenshot `$CARGO_TARGET_DIR/preview/wasm_smoke.png` of the game's start, the **Salzburg title screen** (M1, E6): not blank, at least 25 % sky (palette `s` sky_light + `S` sky_blue together) and at least 5 % of the upper `S` band, which only the parallax sky has (a flat sky fill or the testbed fails). Measured: 36.4 % sky (20.4 % `s`, 16.0 % `S`; logo, rooftops, Festung and cobbles cover the rest), identical over repeated runs (the title backdrop is static; only the prompt blinks, which doesn't touch the sky).
- `vnc-smoke` stays on the testbed: it passes `--replay tests/replays/testbed_route.replay`, whose `level = testbed` overrides the app's default level (`salzburg`), and compares against the testbed goldens.

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
