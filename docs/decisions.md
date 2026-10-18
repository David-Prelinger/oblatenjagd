# Decision log

Short records of decisions and why. Newest at the bottom. Agents append here when they make a decision that affects more than one file or crate; to change an existing decision, add a new entry that supersedes it and explains why.

## D-001 Rust + macroquad (not SDL2, not Bevy)
- macroquad targets `wasm32-unknown-unknown` directly; SDL2 from Rust needs Emscripten for the web, which adds a C toolchain, bigger bundles and breakage.
- Bevy is powerful but heavy (compile times, ECS ceremony) for a 2D pixel game where we render in software anyway.
- Risk is contained: only `jnr-app` uses macroquad.

## D-002 Software renderer into a 320×180 framebuffer
- Pixel-perfect by construction, no GPU dependency (the container has none).
- The same code produces headless screenshots, so golden-image tests show exactly what the player sees.
- 320×180 is 16:9 and scales by whole numbers to 720p (×4), 1080p (×6) and 1440p (×8).

## D-003 Fixed-point simulation at fixed 60 Hz
- Bit-identical results native vs wasm → replays and solver bots stay valid everywhere.
- Classic platformers (SMB) use sub-pixel integer math too; it suits tight pixel-art movement.

## D-004 All assets as text sources
- `.sprite` text grids, procedural generators, ASCII levels, TOML text/tuning.
- Agents can author and diff them; no binary blobs in git except approved golden screenshots.
- Pixel art is produced inside the dev container with no external editor. **No LibreSprite / Aseprite** (human decision).

## D-005 Headless-first validation
- Every feature must be verifiable without a window (`cargo xtask verify`). The VNC window is for humans and a final smoke test only.

## D-006 Documentation in English, game text in German
- Docs, code, comments, commit messages: English.
- All player-visible text: German (Austrian flavour), only in `content/text/de.toml`.

## D-007 Fully autonomous agents (human, 2026-10-18)
- No human involvement: agents decide, commit, merge to `main` and accept milestones themselves (see `docs/milestones.md`).
- Web research and installing tools in the container are allowed; installed tools go into the dev container config.

## D-008 Game decisions (human, 2026-10-18)
- Title „Oblatenjagd"; hero **Resi** in a Dirndl; real names of places, businesses and historical people may be used; difficulty **medium** (see `game-design.md`).

## D-009 Parallel code tasks run in git worktrees
- Two engineers editing the same tree break each other's `cargo xtask check` (and block the orchestrator's Stop hook).
- Parallel code tasks therefore run in isolated git worktrees, each with its own `CARGO_TARGET_DIR` (`/workspaces/.cargo-target/wt-<task>`), and commit on their own branch; the orchestrator merges them into the milestone branch.

## D-010 Atlas data is owned and built once at startup
- The atlas types (`Atlas`, `AtlasSprite`, `AtlasAnim`) are plain owned data (`Vec`/`String`) shared by build time and runtime; `jnr_assets::atlas()` builds the atlas once via `OnceLock` from embedded RGBA bytes and generated metadata.
- One type family instead of separate `&'static` statics; the cost is one pixel copy at startup.

## D-011 Level semantics (jnr-core, M0)
- `.` is always empty air and can't be redefined in the legend; tile names map to collision kinds in core.
- Outside the grid: left/right are solid walls, solid tiles in row 0 extend upwards, below the level is empty and falling out kills.
- A goal is a pole covering its whole column above the goal cell, so it can't be jumped over.
- The player spawns grounded; `Dying` freezes the player; respawn resets the camera.
- No air drag; turning in the air uses `air_accel`. On the ground, speed above the current max (RUN released) decays by `friction`.
- (added after review) One-way platforms are dropped through only with DOWN + JUMP on the ground; DOWN alone does nothing (so walking over Zunftschilder while ducking is safe).
- The player hitbox size is a gameplay number in `content/tuning.toml`; sprite placement comes from the atlas origin, not from constants in core.

## D-012 Bitmap font convention
- Sprite `font` in `art/sprites/ui/font.sprite`, 8×8 cells, one anim `glyphs`; frame i = i-th char of `jnr_render::font::CHARSET` (89 glyphs: ASCII subset, ÄÖÜäöüß„“–…×%+). New glyphs are only ever appended, so indices stay stable.
- Glyphs are drawn in palette `w` only; the renderer tints them and draws the `k` shadow at (+1,+1).
- Advance = rightmost opaque column + 2; space (frame 0, intentionally empty, lint-exempt) advances 3 px; line height 9.

## D-013 Replay semantics and golden naming
- Replay frame numbers are `Game::frame()` values: frame N is the state after N steps, frame 0 the state before the first step. Shots and `frames=a..b` ranges (inclusive) use this numbering.
- A replay always runs all its inputs; end-state expectations (`state=`, `deaths`, `player_x>=`) are checked after the last input. Unknown expectation clauses are parse errors ("not supported yet"), so a replay never silently ignores a check.
- Replay parsing and expectation checks live in `jnr_core::replay` (pure, unit-tested); loading files, rendering and determinism runs live in xtask.
- Goldens are named after the replay, not the level: `tests/golden/<replay>_<shot>.png` (several replays may share a level). `golden --accept` re-renders instead of copying earlier actuals; rendering is deterministic, so the result equals the reviewed preview.
- `levels lint` runs in `check` (L3), not only in `verify`.
- The hand-written testbed route exists once, as `tests/replays/testbed_route.replay`; jnr-core's testbed test includes that file.

