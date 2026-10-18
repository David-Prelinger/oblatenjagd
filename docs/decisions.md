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
- (after review) A replay must have at least one expectation (runner failure, not a parse error); `replays` fails on an empty `tests/replays/`; goldens without a shot fail; goldens are only accepted from passing replays.
- The hand-written testbed route exists once, as `tests/replays/testbed_route.replay`; jnr-core's testbed test includes that file.

## D-014 Live smoke test and human recordings
- `vnc-smoke` plays `tests/replays/testbed_route.replay` in the real native window and requires its `--screenshot-at` framebuffers to be pixel-identical to the existing goldens (no separate VNC goldens). The X root screenshot is only checked for a sizable sky area (±8 per channel, the desktop is 16-bit RGB565), never compared exactly.
- `jnr-app --record` writes a replay with `source = human` and an `expect` line derived from the outcome (`complete frames=N..N` or `state=… player_x>=…`, plus `deaths=`), so every recording is a passing replay test the moment it is saved.
- Native Escape and window close end the session cleanly (macroquad `prevent_quit`) so the recording is always written; wasm keeps running until the page closes.

## D-015 Solver bot: search, jump window and solver replays
- The solver (`cargo xtask solve`, L4) is a frame-synchronous beam search over the real `Game` with a small fixed macro-action set; ranking is furthest right, then fewer jumps, then a stable insertion order, so the same level + tuning always gives the same route (threads only split work). The wall-clock limit is checked only while no completion has been found, so it can never change a route; but whether a search passes or fails can depend on machine speed if it runs into the limit, so the limit (60 s) is generous on purpose (the testbed takes ~0.6 s). jnr-core and xtask are built with `opt-level = 2` in the dev profile for speed.
- The jump window, the difficulty number level designers use (`game-design.md`: ≥ 6 frames in stages 1–3), is defined as: shift the whole held-JUMP segment by d ∈ [-12, +12], rest of the inputs unchanged; past the route's end the inputs are padded with the last input. Checkpoint = the first landing after the matching `Jumped` + 2 s; success = alive up to the next jump's checkpoint (or the end of the route) and at most 16 px behind the original at the own checkpoint (or finished). Window = contiguous successful shifts around 0, with the reason each side stopped: `fail` (the level), `route` (the shift would start before frame 0 or touch a neighbouring JUMP hold) or `max` (±12). Route-limited windows are only lower bounds and never count as the narrowest. It is measured on the cleaned route (no-op JUMP removed).
- Solver replays are `tests/replays/<level>_solver.replay`, written only with `--write`, and carry no `shots` by default (shots need art-director-approved goldens; `--shots` opts in).
- The `solver` verify step also requires the committed `tests/replays/<level>_solver.replay` for every level.
- A per-level `min_jump_window` waits for an optional `LevelMeta` field (backlog, M1); until then the `solver` step only reports the narrowest window.

## D-016 M0 movement tuning accepted unchanged (game-designer, M0 acceptance)
- The measured M0 movement (walk 1.5 / run 2.5 px/frame, jumps ~25 / 65 / 81 px, max fall 4 px/frame, coyote and buffer 6 frames, running jump ~9 tiles) is close to Super Mario Bros. and fits medium difficulty, so `content/tuning.toml` keeps its values in M1 (no replay or golden churn).
- The ~9-tile running jump is not "too far": it's a ceiling for optional routes. Required jumps in stages 1–3 follow the reach rules in `game-design.md` (gaps ≤ 4 tiles, step-ups ≤ 3; 5–6-tile gaps only with ≥ 8 tiles of run-up). The solver's ≥ 6-frame window stays the final check.
- New feel targets for M1 (new keys, not changes): walking jump reach ~5 tiles, stomp bounce ~1 tile (tap) / ~3 tiles (held), hurt invulnerability 120 frames.

## D-017 Stage 1 gameplay rules (game-designer, before M1)
- No ducking: DOWN is only used for one-way drop-through (D-011). Zuckerschock hitbox 12×30 px with the same physics; required passages are ≥ 2 tiles high.
- Stomp = falling and the feet above the enemy hitbox's middle on the previous frame. SMB-style stomp chain and scores (Oblate 200, power-up 1000, brick 50, time bonus 50/s). Maibaum scores by feet height in 2-tile zones up to 5000 for the Wurstkranz (≥ 8 tiles).
- The Fiaker can't be stomped, but landing on it bounces without damage (medium difficulty: no punishment for the obvious "jump on it" idea).
- A death resets the whole level (enemies, boxes, Oblaten) like SMB; checkpoint flag, score, Oblaten counter and lives survive; the timer is reset; the player is Normal again. Game over → „Weiter?" restarts from the stage start.
- Thrown Mozartkugeln are unwrapped chocolate balls, visually distinct from the wrapped power-up.
- Enemy patterns use frame counters from activation, never the RNG, so the solver and replays see fixed patterns.

## D-018 Level signs (game-designer, before M1)
- Signs are a decorative `sign` entity referencing a `de.toml` key, ≤ 2 lines of ≤ 12 characters each (`\n` separates the lines); this is how the skill's "signs ≤ 12 characters" applies to two-line boards. The level format for it (e.g. `{ entity = "sign", text = "stage.salzburg.sign_bus", style = "bus" }`) is the engine-engineer's choice.

## D-019 Input across respawn, replay limits, scene clamp
- While `Dying`, `Game::step` keeps updating `Player::jump_held` from the input, and `respawn` carries it into the new player. A JUMP held across a respawn is therefore not a new press (no auto-jump); presses during `Dying` never reach the jump buffer (it is not updated while dying and is reset on spawn).
- Replay files hold at most `MAX_FRAMES_PER_LINE` frames per input line and `MAX_TOTAL_FRAMES` in total (both 216 000 = 1 h at 60 Hz); larger files are parse errors with a line number, so a malformed replay cannot exhaust memory or run for ever.
- `cargo xtask scene` clamps the player's centre so its hitbox stays inside the level and prints a warning when it did; the camera clamps itself as before.

## D-020 Sprite catalog and autotiling (M1)
- `docs/sprite-catalog.md` is the contract between art and engine: sprite names, sizes, origins, animation names. The engine falls back to grey boxes for missing sprites, so art and code proceed in parallel.
- Autotiling without a new artkit header: a tile sprite with a 16-frame animation `auto` is drawn with frame = 4-neighbour mask (N=1, E=2, S=4, W=8; level edge counts as same). The `tileset` header stays unsupported.

## D-021 Parallax layers and procedural backgrounds (M1, engine)
- Background layers are `(sprite, factor in sixteenths)` pairs, back to front, set on the `RenderContext` (`with_background`); the level format provides the list later. Each layer is tiled horizontally from `-((camera.x * factor / 16) mod width)` and **bottom-aligned to the screen bottom**, with no vertical parallax for now (simple and predictable for art; revisit if levels scroll a lot vertically). Integer sixteenths keep the renderer free of floats.
- The flat sky fill stays behind the layers unless the first layer is fully opaque and at least screen-tall (precomputed once), so a partial layer list never shows stale framebuffer pixels.
- Generators live in `jnr-artkit::generators` (the requested `gen` name is a reserved keyword in Rust 2024) and turn `art/generators/*.toml` entries into ordinary `SpriteSource`s, so lints, compile, atlas and previews treat generated and hand-made sprites the same. Params are parsed with `serde` + `toml` (`Spanned` values give `file:line:col` errors); this makes `serde`/`toml` default dependencies of `jnr-artkit` (pure, wasm-safe, already used by `jnr-core`).
- Generated sprites are compiled into the game atlas like all art (the four Stage 1 layers add ~0.9 MB of raw RGBA to the embedded atlas).
- Autotile masks are computed from the live level while drawing (4 lookups per visible tile), not cached in the `RenderContext`, so tiles that change during play (bricks) stay correct; only the per-tile-name look is precomputed.

## D-022 Level format v2, entities, power states, level reset (M1, task E1)
- Legend values: a tile name, `{ tile = "...", contains = "oblate|mozartkugel|zaunerstollen|oblaten_10" }` (only `brick` and `oblaten_box`; a box defaults to `oblate`), or `{ entity = "..." }`; signs take `text = "<key>"` (required) and `style = "street|bus|ortsende"`. Unknown fields are errors. `[meta]`: `time_limit` is required (1..=999), `background = [{ sprite, factor, y }]` (factor in 1/16 of the camera speed, 0..=32; optional `y` = px above the screen bottom, 0..=180, default 0; orchestrator decision), optional `min_jump_window`. `RenderContext::for_level` wires it into `with_background(&[(sprite, factor, y)])`; the app, `replay --shots`/goldens, the solver images and `xtask scene` (unless `--bg` overrides it) use it.
- Level spawns are `SpawnKind` (level.rs); runtime entities are `entity::EntityKind` in one `Vec<Entity>` (enum + match, per-kind update). Level entities activate when their left edge is within `game.activate_margin` (320 px, one screen) of the right screen edge, and are removed `game.despawn_margin` px behind the camera or below the level. Order: spawns in grid order, then spawned entities appended.
- The current tile grid lives in `Game` (copy-on-write `Arc<TileMap>`), with a sorted list of changed cells for the hash; `Level` stays the start state. A death resets tiles, entities and the timer; score, Oblaten, lives and the checkpoint survive. Lives 0 → `Mode::GameOver`; START (after `game_over_frames`) restarts from the stage start with 3 lives, score 0, Oblaten 0.
- Power states: Normal/Big hitbox heights from tuning (feet stay in place), a 30-frame freeze for growing and shrinking (the whole game stands still, timer included), `invulnerable_frames` counted after the freeze (so 120 frames of play without harm). Hazards, pits and time-up kill regardless.
- Enemies are parsed and placed now as standing placeholders with the generic contact rules (stomp = falling and feet above the enemy's middle on the previous frame; otherwise hurt; Fiaker bounces only; SMB chain with the Verkäufer starting at 200), so hurt/stomp are testable; movement comes with the enemy task.
- A head bump hits only the solid tile nearest the player's centre column. A brick with contents behaves like a box. Static Oblaten give no popup (SMB); everything else that scores shows one.
- `Game::world_hash` is a coarse progress hash (no timers, no cosmetic entities) used by the solver's de-duplication key; `state_hash` hashes everything exactly.
- Replay expectations `oblaten>=N` (HUD counter), `score>=N`, `lives=N`, `state=game_over`.
- Renderer: Zuckerschock uses `player_big` (fallback `player`) with the same anim selection; `PlayerAnim::Skid` (anim `skid`, fallback `walk`) while braking against the motion on the ground, drawn facing the direction of motion (sprite catalog), decided in the renderer so core's `facing` stays the input direction.
