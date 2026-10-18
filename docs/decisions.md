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

- (added in M1) The orchestrator deletes `/workspaces/.cargo-target/wt-<task>` when it merges or abandons that worktree; reviewers use one shared `wt-review` dir (deleted after each review). Each worktree target dir is 1–3 GB, and the 59 GB disk ran full once.

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
- (after review) Contact is order-independent: items and stomps are resolved for every touched entity first, hurts afterwards (one hurt per frame). Every enemy whose stomp rule holds in a frame is stomped (each counts in the chain); the player bounces once and gets **one `Stomped` event per frame**. The stomp rule uses the enemy's `prev_y` (snapshot before entities move) and the player's previous feet. `Entity` has per-kind `state: u8`, `dir: i8` (enemies start at -1), `base_y` for the enemy task; all hashed.
- (after review) At the lives cap an extra life does nothing: no `ExtraLife` event, no „+1 Leben" popup (points still count). Levels have at most 65 535 entity spawns (parse error). The Festungsbahn size is `platforms.festungsbahn_size` in tuning; frame counts where 0 breaks behaviour are validated ≥ 1.
- A head bump hits only the solid tile nearest the player's centre column. A brick with contents behaves like a box. Static Oblaten give no popup (SMB); everything else that scores shows one.
- `Game::world_hash` is a coarse progress hash (no timers, no cosmetic entities) used by the solver's de-duplication key; `state_hash` hashes everything exactly.
- Replay expectations `oblaten>=N` (HUD counter), `score>=N`, `lives=N`, `state=game_over`.
- Renderer: Zuckerschock uses `player_big` (fallback `player`) with the same anim selection; `PlayerAnim::Skid` (anim `skid`, fallback `walk`) while braking against the motion on the ground, drawn facing the direction of motion (sprite catalog), decided in the renderer so core's `facing` stays the input direction.

## D-023 QA rulings on items and stomping (orchestrator, M1)
- Stomp boundary stays as specified (D-017): a fall whose previous-frame feet are below the enemy's middle is a side hit, even near the corner. Generous enough for medium difficulty; revisit only if playtests disagree.
- Reaching 100 Oblaten shows the „+1 Leben" popup like the Zaunerstollen (unless at the lives cap).
- Goal and time-up in the same frame: the goal wins.
- Popups freeze during the 30-frame transformation freeze like everything else.

## D-024 Growing into a solid is a head bump (engine, M1 QA fixes)
- Growing keeps the feet (D-017). If the new Zuckerschock head then overlaps solid tiles (e.g. eating the Mozartkugel while rising just below a box), it counts as a head bump: upward speed stops and the hitbox is pushed down flush below the lowest overlapped solid tile (`Player::fit_head`). The block is not hit (no bump, no brick break).
- Tuning guarantees there is always room below: `hitbox_h_big` must not exceed `hitbox_h` rounded up to whole tiles (16 px), so every gap a Normal Resi fits in also fits Zuckerschock (validated in `tuning.rs`; 22 → 32 ≥ 30). Growing on the ground therefore never overlaps a ceiling.
- `TileMap::move_y` moving up never tunnels: a box whose top edge already is inside a solid row does not move up and reports `Ceiling`.
- Score popups: breaking a brick shows „50"; the 100th Oblate shows „+1 Leben" (D-023, none at the lives cap).

## D-025 Game flow, replay start, goal sequence (M1, task E4)
- `Game::new_at(level, tuning, seed, Start)`: `Start::Title` is the real game (the app's default): title screen → START → stage intro (`game.intro_frames` = 150) → play; after every death `Dying` → level reset + respawn → intro → play; game over → START (after `game_over_frames`) → intro; START during play pauses (`Mode::Paused`, nothing moves, timer stands still), START again resumes. `Start::Playing` (`Game::new`) starts in play and keeps the M0/E1 behaviour (no title, no intros, START does not pause; the goal sequence still ends in `Mode::Title`, and START there restarts the stage directly), so tests, the solver and all existing replays and goldens keep their frame numbers. JUMP is tracked in every non-play mode, so a held JUMP is never a new press afterwards.
- Replays opt into the real flow with the header `start = title` (default `playing`; written only when not the default). `--replay` uses the replay's `start`; `--record` writes the start it recorded with. New end states `state=title|stage_intro|paused`.
- Maibaum sequence (`Mode::StageClear` + `Game::stage_clear()`, phases `Slide → Hop → Walk → Tally → Clear → ToBeContinued`, then `Title` with the run reset to the stage start): at the first touch the height of the feet above the goal cell's bottom picks the zone (`goal.zone_heights` / `goal.zone_scores`; top zone = Wurstkranz + popup „Wurstkranz!"), the player is placed with her hitbox `goal.pole_offset` px left of the pole centre and slides at `goal.slide_speed`; at the base she is put `pole_offset` right of the pole and hops (`goal.hop_speed`, player gravity), then walks at walk speed with no collision until her hitbox is 16 px past the right screen edge (the camera stays); tally one second per frame (`score.time_bonus` each, skipped at 0 s); texts `goal.clear_frames` after the tally; „Fortsetzung folgt …" `goal.to_be_continued_frames`. Enemies, items and the timer freeze; popups, debris and bumps keep animating. Input is ignored (except JUMP tracking). Edge cases: a touch from the right side of the pole teleports her to its left side; a touch from below the base (feet lower than the goal cell's bottom) snaps her up to the base; the hop and the walk ignore terrain (no collision, a constant ground line at the base).
- Replay `complete` keeps its meaning (the frame of the first touch, `LevelComplete`); `state=stage_clear` holds for the whole sequence. The renderer shows `pole` while sliding; the stage-clear texts and the time-bonus line from the tally on.
- Stage text keys are `stage.<level id>.{title,tagline,next}`; when de.toml has no `stage.<id>.title` (test levels) the renderer uses `stage.salzburg.*` (M1 has one stage); `[meta].title_key` overrides the title.
- The Maibaum is drawn `goal.zone_heights[3]` (= Wurstkranz height, 128 px) tall: `maibaum_pole` tiles up from the goal cell's bottom, `maibaum_top` with its origin on the pole top. The old `goal` sprite name is gone.
- Pause (art-director ruling, art-style §11: no dimming, no dithering): the frozen level stays at full colour behind a palette panel, `B` fill at x 112–207, y 78–101, 1 px `k` border, 1 px `S` highlight on the top and left inner edges, `ui.pause` in `w` with the `k` shadow centred at y = 86.
- `Event::Respawned` comes only from the Dying path (a death → respawn); title → START and game over → START restart the stage without it.
- The HUD is drawn in `Playing`, `Paused`, `Dying` and `StageClear`, so every golden of a playing state shows it. With the `hud_oblate` icon the OBLATEN digits start at x = 106 (icon at x = 96).
- The title screen is static: its backdrop (background, tiles, Maibaum, Resi idle) and the logo are drawn at animation tick 0; only the START prompt blinks (y = 128).

## D-026 Enemy behaviours and the Festungsbahn (M1, task E3)
- Enemy patterns run on frame counters from activation (`Entity::timer`, per-kind `Entity::count`), never on the RNG (D-017). Distances to the player ("within 6/8/10 tiles") are horizontal, centre to centre. All numbers are `tuning.toml` `[enemies]` / `[platforms]` keys, validated in `Tuning::parse`.
- Selfie-Tourist: a selfie starts every `tourist_selfie_period` (240) frames counted from activation, so the first one is at frame 240 and it walks 192 frames between stops; the flash is frames 24–25 of the stop (renderer only, harmless). It turns at walls and at other enemies on foot (tourists, Verkäufer, parked Fiaker, a sitting Taube) by setting its direction away from the one it overlaps; it falls into hazards like into pits.
- Taube: integer sine via Bhaskara I's approximation (`enemies::sine_offset`, < 0.2 % error, exact at peaks and zeros, any period) around its start height, rising first; no table. The sitting Taube rises `taube_rise` px over `taube_rise_frames` from the frame the player comes close, then flies with the wave starting at its new height.
- Verkäufer: the throw counter only runs while the player is in range (it pauses outside, keeping a started wind-up pose until the player returns); the last `verkaeufer_windup` frames of each `verkaeufer_period` are the wind-up, the throw pose shows `verkaeufer_throw_pose` frames. The thrown Mozartkugel moves, then accelerates (apex 37.5 px with the design numbers, "~36"), passes through one-way tiles and vanishes on solid tiles or outside the level; it hurts on any contact and is never stomped.
- Fiaker: drives along its spawn row and **ignores tiles** while charging (it passes the Residenzbrunnen the design uses as a hiding spot; no gravity, so it never falls into pits); it is removed once its right edge is left of the screen. The generic top-contact bounce (no damage, no points) is unchanged.
- Bumping or breaking (Zuckerschock) a block knocks off enemies on it.
- Stomped enemies become `EntityKind::Defeated(kind)`: squished in place for `squish_frames` (Tauben tumble instead), no collision, left out of `world_hash`. Bumping a block knocks off stompable enemies standing on it (feet exactly on its top): `score.bump_enemy` (100, not part of the stomp chain), they tumble down off the screen; rolling items on it hop (`knock_speed`). The Fiaker can't be knocked.
- Festungsbahn: level syntax `{ entity = "festungsbahn", to = [dx, dy] }` (other end point in tiles relative to the spawn cell, required). Wait at the start, travel, wait, travel back; positions recomputed from the start point every frame. Only the top edge collides (from above); a standing player is carried by exactly the car's movement (with tile collision), so standing and walking on it are sub-pixel exact. Crush rule: the player is never crushed; a ceiling holds them and they drop through the car. Levels keep 3 tiles free above the car along the whole track (`levels lint`).
- The solver's beam keeps a "high" reserve (up to a quarter of the width: the highest node per 16 px column when higher than every kept one), because furthest-right ranking alone drops players waiting above a pit for the car (or riding it) in favour of nodes further right in the pit below. The testbed route is unchanged by it.
- Replays can assert `event=<Event>` (at least once).
- (merge with E1 QA / E4) Grey-box fixture: `{ entity = "selfie_tourist", still = true }` never walks or turns (`dir = 0`; selfie poses and contact rules unchanged). `00_items.level` uses it, so the E1/E4 regression replays built around a fixed target at column 34 (stomp boundaries, i-frames, 100-Oblaten loops, flow/goal goldens) stay valid unchanged. `levels lint` allows `still` only in `00_*.level` files, so real stages always have walking tourists.

## D-027 QA rulings on enemies, platforms and flow (orchestrator, M1)
- Trigger distances (sitting Taube, Fiaker, Verkäufer range) compare whole pixels with truncation; „within 6 tiles" means < 97 px. Accepted as specified.
- Enemy hit and goal touch in the same frame: the goal wins (consistent with D-023 goal vs time-up).
- A sitting Taube that rises and later hits a standing player is acceptable: the rise is the visible telegraph.
- Moving platforms never despawn. Every pit under a moving platform must have a way back up (stair-stepped rock as in the Stage 1 spec) — "falling off means waiting for the car or climbing back, never a softlock". A bounce off a non-stompable enemy (Fiaker) puts the player's feet on its top, so the bounce can never be followed by a side hit.

## D-028 Implementing D-027: Fiaker bounce, goal vs hit, pit recovery lint (engine, M1 QA fixes)
- Fiaker bounce: in a frame with a bounce off a non-stompable enemy, after the bounce speed is set, the player's feet are put on the top of the highest such enemy (`Game::rest_feet_on`). The move is upwards and shorter than the hitbox, so checking the new hitbox against solid tiles covers the whole way. If it would overlap a solid or hazard tile, she stays where she is and gets `enemies.bounce_grace_frames` (8, validated ≥ 2 since it counts down in the frame it is set) in which non-stompable enemies do no harm (`Player::bounce_grace`, hashed, counted down like `invulnerable`; every further bounce re-arms it). Stompable enemies, thrown Mozartkugeln and every other hurt are unaffected.
- Goal vs enemy hit: the hurt from `interact` is skipped when the player's hitbox overlaps a goal in that frame; the goal check later in the same step starts the Maibaum sequence. Hazards and pits still kill first (unchanged).
- Moving platforms (the Festungsbahn) are never removed by the despawn margin or the level bottom.
- `levels lint` pit-recovery check (`platform::recovery_problems`, `MAX_STEP_UP_TILES` = 3): per Festungsbahn, floor per column under the track = topmost solid/one-way tile at or below the highest point of the car's top over that column; walking from the far end of the track to the first column beyond its upper end (its floor: the topmost one at most `TRACK_CLEARANCE_PX` above the car's top there, so a ledge slightly above the car counts), no step up may exceed 3 tiles; a vertical track passes if either side works. Limits (documented in the doc comment): no headroom check, no jump-distance check across floorless columns (skipped: deaths, not softlocks), no camera check when the climb leads left, enemies ignored. Cheap and catches the 00_enemies class (a wall from the pit floor up to the upper end).
- `00_enemies.level`: stair-stepped rock (steps up 1–2 tiles) under the track; the plateau ends at column 107 and the Maibaum stands on the meadow at the level's bottom row (cell bottom y 208, camera y 44: base at screen y 164). The full Maibaum is 176 px tall (128 px pole + 48 px `maibaum_top`), so with a 180 px screen its tip can never stay below the HUD (y ≈ 22); at the lowest possible placement the pole top (the `maibaum_top` origin, wreath) is at screen y 36 and the spruce tip at y −12. Stage levels should place the Maibaum on the lowest ground too.
- The stairs under a Festungsbahn make the car optional: it's the comfortable ride, climbing is a valid alternative (accepted).

## D-029 Maibaum vs HUD (orchestrator, M1)
- The Maibaum is 176 px tall (128 px pole + 48 px top) and can't fit fully below the HUD rows on a 180 px screen. Rule: the goal stands on the lowest ground of its screen, and at the goal-touch camera position the `maibaum_top` sprite may reach into the HUD rows (y 0–21) only in the gap between the OBLATEN and LEBEN columns (screen x 139–182: the OBLATEN outline ends at 138, LEBEN's outline starts at 183 since D-037) and must not touch any HUD pixel; inside the HUD rows Stage 1's crown spans 152–181, never behind HUD text. (Numbers corrected by D-037.) With the camera's follow position the pole naturally lands there; the level-designer verifies it with a shot at the touch. Test levels are exempt.

## D-030 Salzburg level follow-ups (orchestrator, M1)
- **Festungsbahn waits for its rider**: the car waits at its lower end until the player stands on it, then travels up, waits at the top, travels back down and waits again for the next rider. (Replaces the free-running cycle from activation; makes boarding a calm, readable moment and removes the 2-frame boarding window the solver measured.)
- **Decorative props**: a generic `{ entity = "prop", sprite = "<name>" }` draws a catalog sprite (bottom-centre at the cell) with no collision; collision stays with tiles. Used for the Sphaera (gold ball statue) and other landmarks.
- New solid tile names `facade` (house wall with windows, autotiled, for Erker/house blocks) and `arch_under` (arch underside, autotiled left/right).
- `min_jump_window` in level meta is enforced by the `solver` verify step.
- Stage 1 is 264 × 17 tiles (3 extra empty rows on top so the camera can follow the Wurstkranz popup clear of the HUD, D-029). A good run is ~30–60 s; explorers take longer — game-design.md's "90–150 s" was too high for the level length.
- The checkpoint's own board text replaces a separate checkpoint sign (no duplicate „Bus 150 / auch voll").

## D-031 Landmark props and back-layer props (orchestrator, M1)
- Salzburg landmarks get prop sprites instead of plain stone blocks: `pegasus` (Pegasus statue on the Mirabell fountain plinth), `fountain_water` (decorative basin water surface, not a hazard), `zwerg` (Zwergerlgarten dwarf statue, 16×16), `residenzbrunnen` (Residenzbrunnen tiers), `geburtshaus` (yellow `Y` façade of Mozart's birthplace, Getreidegasse 9). Collision stays with tiles.
- Props take an optional `layer = "back"` (drawn after the parallax, before tiles — for façades behind the street) or the default front layer (after tiles, behind the player).
- Stage 1 shrinks to 262 columns so the camera clamps at the goal and the Maibaum crown sits clear of the HUD (D-029).

## D-032 Stage 1 review: Tauben away from the Festungsbahn ride (game-designer, M1)
- Section 5's two Tauben no longer sit or fly "on the ramparts": one `taube_sitting` sits on the head of the Sphaera statue (Kapitelplatz), one `taube` flies in at grandstand head height (spawn ~col 250, row 9). Reason: Tauben activate a screen ahead and drift left at 0.75 px/frame, so rampart pigeons arrive exactly at the car's top station and hit the rider (a Normal Resi dies on the ride that is meant to introduce the moving platform with "no death possible"). Rule: no enemy may reach the Festungsbahn track or the jump to the Maibaum.
- Taube placement rule for all stages: a walking or running player meets a Taube ~7–11 columns left of its spawn; levels place first encounters by that meeting point, not by the spawn column.
- Two Mozartkugel-Verkäufer in one stretch keep ≥ 10 columns apart so their landing zones don't overlap into a crossfire.
- The Wurstkranz stays reachable with any held jump off the rampart or the car (first stage, medium difficulty: the finale is a celebration, not a skill wall); walking off scores 2000.

## D-033 Jump windows measure recoverability, not a fixed press chain (orchestrator, M1)
- Shifting one jump while keeping every later press fixed measured artefacts: a 1-frame shift could make a later fixed press miss its buffer, so the window collapsed although a player would simply press the next jump a frame later (Salzburg showed 1–2 frame windows that way).
- New rule: a shift d succeeds if the player survives the shifted jump AND a short re-planning beam search from the state right after the shifted jump's landing (small beam, horizon up to the next jump's checkpoint + margin) reaches at least the original progress at that checkpoint without dying. The window is still the contiguous range of successful shifts around 0 with `fail`/`route`/`max` bounds; `min_jump_window` keeps meaning "a human has at least N frames of slack for this jump".

## D-034 Implementing D-030/D-031 (engine, M1 task E5)
- Festungsbahn state machine (`platform::car`, `Entity::state` + `count`): `WAIT_LOWER` (count 0) → with a rider `UP` 1..=travel → `WAIT_UPPER` 1..=wait → `DOWN` 1..=travel → `WAIT_LOWER`. A rider = the player is grounded with feet exactly on the car's top and overlapping it, seen by the car's update after the player's own movement and landing (so the car leaves in the very frame she lands on it; JUMP pressed on the waiting car puts her in the air first, so it stays). An edge-only touch (her right edge on the car's left edge, or the reverse) is no overlap: it neither calls nor carries. A player standing on an adjacent tile whose top is exactly at the car's top height and who overlaps the car horizontally does call it (and is carried): accepted, since the car's top is 8 px off the tile grid at its ends, so this only happens with tiles placed along a moving car's path. The frame it arrives at the bottom it stops; a rider still on it takes it up again from the next frame. The lower end is the end with the larger y (`to` dy > 0: the spawn cell is the upper end, the car starts in `DOWN`); a level track's lower end is its spawn cell. Carrying, crush and landing rules are unchanged; positions are still recomputed from the spawn point each frame.
- Consequence: a player who fell into the pit after the car came down cannot call it while its lower end is behind the camera's left edge (the camera never scrolls back); the stairs required by the D-028 recovery lint are the way out. Not a softlock.
- Props are not core entities: `SpawnKind::Prop` spawns no `Entity`, has no collision and no hash; the renderer resolves them once (`RenderContext`) and draws them from the spawn list (front layer after tiles and before the Maibaum and entities; back layer after the parallax, before tiles; both also on the title backdrop). Missing sprite: the outline of the cell. `prop` takes `sprite` (required, checked by `levels lint` against `jnr_core::PROP_SPRITES`, which a test keeps equal to the prop rows of the catalog's „Props and goal" table, excluding the sprites of other entities: checkpoint, maibaum_*, sign_*, festungsbahn), `layer = "front|back"` and `dx` (px, ±64).
- Tiles: `facade`, `arch_under` and `invisible` are solid; `invisible` is never drawn (no sprite, no grey box) — it sits under props such as the Sphaera. Autotile families (`autotile_family` in `jnr-render/src/tiles.rs`, an explicit table): `arch_under` → `arch`, so the two count as the same neighbour in both directions.
- D-033 in the solver (`jump_windows`, `recovers`):
  - **Fast path:** the shifted inputs with the later presses unchanged reach the checkpoint progress (x − 16 px, or the finish) without dying. This is the old rule minus "survive up to the next jump's checkpoint".
  - **Otherwise, re-plan:** simulate up to the shifted jump's landing (died → fail, finished → ok; a press that made no jump is re-planned from the end of its buffer), then run a beam search with the solver's macros, beam `REPLAN_BEAM` = 12, from that state. It succeeds as soon as a node stands on the ground at x ≥ the goal (or finishes) by the checkpoint + `REPLAN_MARGIN` (30) frames.
  - **Horizon:** it is the jump's own checkpoint + margin, not the next jump's. Success is measured at that checkpoint, and the later route is re-planned anyway.
  - **Runtime:** the fast path covers most shifts. Debug-build `solve` wall time: testbed 1.6 s, items 1.4 s, enemies 3.4 s, Salzburg 27 s (its search is 7.5 s).
  - **Routes:** unchanged, because the window metric doesn't steer the search.
- `solver` verify step: fails when a level's narrowest jump window (route-limited windows excluded, as in the report) is below its `min_jump_window`, naming the jump frame and x (`solver::window_problem`).

## D-035 Stage 1 level rulings (orchestrator, M1)
- The second flying Taube spawns at col 244 (not 250): from 250 it crossed the rising Festungsbahn rider; 244 keeps D-032's "no enemy reaches the track or rider".
- Walking off the rampart edge scores 100 (she lands on the meadow before the pole column); any held jump off the rampart or car reaches the Wurstkranz. This corrects the "walking off scores 2000" note in D-032.
- The Fiaker stands at col 200 so that standing on the Residenzbrunnen triggers it (it charges through underneath).
- The title prompt crosses the cream Dom façade of `bg_rooftops`: the prompt gets the full `k` outline (art-style §11 allows it) — polish task.

## D-036 The app ships every level and starts on Stage 1 (engine, M1 task E6)
- `jnr-app` embeds every `content/levels/*.level` through an explicit `include_str!` list (`options::LEVELS`) with a unit test that it matches the directory (no build script). `DEFAULT_LEVEL = "salzburg"`: native without flags and wasm start on the Salzburg title. Test levels stay reachable with `--level` and `--replay`.
- `vnc-smoke` keeps the testbed route (its replay names the level); `wasm-smoke` now checks the Salzburg title: `s` + `S` sky ≥ 25 % and the parallax-only `S` band ≥ 5 % (measured 36.4 % / 16.0 %).
- The title prompt `title.press_start` is drawn with the full 1 px `k` outline (all 8 neighbours, `Font::draw_text_outlined`) instead of the drop shadow (D-035); every other text keeps the shadow.
- `tests/replays/salzburg_tour.replay` is the Stage 1 golden tour: `start = title`, START at frame 21, the intro, then the `salzburg_solver.replay` inputs unchanged from frame 171. After `cargo xtask solve salzburg --write` changes the route, copy its inputs into the tour and re-pick the shot frames.

## D-037 HUD and popup outline, stage-clear panel (art-director ruling, M1 task E6)
- HUD labels (`w`) and numbers (`Y`) and every popup (score digits and the text popups such as „Wurstkranz!") are drawn with the full 1 px `k` outline of D-036 (`Font::draw_text_outlined`, all 8 neighbours) instead of the drop shadow, so they read over light sky, clouds, façades and the Maibaum crown. The outline reaches 1 px left of and above each column: LEBEN's outline starts at x 183, the HUD still fits y 3–21. D-029's gap is now x 139–182.
- Stage-clear texts sit on an art-style §11 panel (like pause): `B` fill, 1 px `k` border, 1 px `S` highlight inside the border on top and left, 4 px padding inside the border. y 69–109 (lines at 74, 84, 96), clear of the crown (y ≤ 41) and the Ortsende sign; the width is the widest line (time-bonus line measured with 5 digits, so it doesn't grow during the tally) + 1 px shadow + padding and border, rounded up to even and centred (Stage 1: about x 106–214). Texts in `w` with the `k` shadow, the bonus number in `Y`. The panel appears with the texts (tally and clear phases). The time-up text keeps y 72 without a panel.
- The salzburg_tour busbahnhof shot is frame 216, the first frame where Resi's hitbox is right of the bus board (screen x ≥ 113).


## D-038 Stage 1 QA rulings (orchestrator, M1)
- **Wurstkranz at 7 tiles**: the Maibaum's top zone starts at a feet height of 7 tiles (112 px) instead of 8: 6–7 tiles = 2000, ≥ 7 tiles = 5000. A good running jump off the rampart or the Festungsbahn car reaches it; a walking/standing jump off the car does not; walking off scores 100. (Corrects D-032/D-035 "any held jump reaches it", which QA measured as false.)
  - Implementation (engine): `goal.zone_heights = [32, 64, 96, 112]` in `tuning.toml`. The pole is still drawn `zone_heights[3]` tall (D-025), so the Wurstkranz keeps hanging exactly at the 5000 threshold and the whole Maibaum is now 160 px (112 px pole + 48 px top) instead of 176; the D-029 HUD numbers for the crown shift 16 px down accordingly. Goldens showing the Maibaum change and need an art-director review.
- **What is drawn as solid must be solid**: a prop drawn over tiles must not suggest a higher surface than the collision (Residenzbrunnen: all five columns solid at the drawn 3-tile height).
- Walking into the waiting car at the lower station drops Resi into the station trench (car top is above street level): accepted — jumping on is the natural move and the trench stairs lead back up.
- Level notes taken on for Stage 1: Makartsteg high Tauben one tile higher (they must stay above every jump arc, D-032); ≥ 1 tile headroom over the burger guild sign; stagger the two Tauben at the Sphaera/grandstand; signs must not hide play elements (Do-Re-Mi, „Liebe max.", Jedermann vs the Oblate at 210); Sphaera `dx` so the man stands over his collision; the Mirabell hedge tourist must be visible on the hedge.
- (art-director ruling) A Taube's sine wave may enter the HUD band, but must never be hidden behind HUD text for more than a few pixels; for later stages prefer keeping the top of the wave at or below row 13 (under the labels).

## D-039 „Bus 150 / auch voll" is a level sign beside the checkpoint (game-designer, M1 acceptance)
- Supersedes the last point of D-030 („the checkpoint's own board text replaces a separate checkpoint sign"): the checkpoint sprite is 16 px wide and draws no text, so `stage.salzburg.sign_checkpoint` was never shown and the stage's running gag (start: „Bus 150 / leider voll" → Hanuschplatz: „auch voll") lost its punchline.
- Fix without code: the level places `{ entity = "sign", text = "stage.salzburg.sign_checkpoint", style = "bus" }` on the street right of the checkpoint (≈ col 130–134), not hiding the checkpoint pole, the `M`-box at col 132 or the Mozartkugel's path (signs are background; D-038 "signs must not hide play elements"). The checkpoint itself keeps its disc + „Zwischenstopp" popup.
- Rule for later stages: checkpoint jokes go on an adjacent sign, not on the checkpoint sprite.

## D-040 Goal camera follows the slide down (orchestrator, M1 acceptance)
- During the goal sequence the camera stays horizontally (as before) but follows Resi vertically down to the goal's ground during the slide, so after a high (Wurstkranz) touch the hop, the walk past the Ortsende sign and the exit are on screen. Found by the M1 acceptance QA pass.
- Implementation (engine): in the slide, hop and walk phases the camera applies only the vertical part of the play camera (`Camera::follow_y`: the `camera.deadzone_top/bottom` dead zone on the player's centre, clamped to the level) after each frame; x never changes. So the slide ends at exactly the framing the play camera has on the goal's ground, and a low touch (already inside the dead zone) leaves the camera untouched (`salzburg_tour_maibaum`, `flow_title_tally`, `testbed_route_stage_clear` unchanged). The tour's 400-zone touch ends 16 px lower than before (the old framing had the walk-off with the feet on the bottom screen edge and the meadow out of view), so `salzburg_tour_tally` changes and needs an art-director verdict before it is accepted. No new tuning keys. Tests: `tests/goal.rs` (low touch: camera still; high touch: x fixed, y only down, Resi on screen every frame, the slide ends at the ground framing), replay `qa_m1_goal_camera_high_touch` (sequence completes to the title).

## D-041 Stages 2 and 3: scope and ramp rules (game-designer, M2 planning)
- **St. Gilgen** (`02_st_gilgen`, 280 columns, 6 sections) introduces the Zwölferhorn **Gondel** (free-running moving platform), the **Nordic-Walker** (turns at ledges) and the **Blasmusiker** with straight note trains. **Brunnwinkl** (`03_brunnwinkl`, 240 columns, 5 sections) introduces the **Bienenschwarm** (waggle-dance figure-eight, not stompable; its run points to a bonus) and the **E-Biker** (stompable charger that follows the ground). Reused: Selfie-Tourist, Taube (+ sitting), one Mozartkugel-Verkäufer (St. Gilgen), water death with a new Wolfgangsee look. Not in M2: Fiaker (Salzburg only), the Sound-of-Music tour group (backlog). Reason: one or two genuinely new mechanics per stage, each a small extension of an M1 system (Festungsbahn → Gondel, Selfie-Tourist → Nordic-Walker, Verkäufer → Blasmusiker, Fiaker → E-Biker, Taube's integer sine → swarm), so M2 stays one milestone.
- Ramp rules for stages 2–3 (in `game-design.md`): `min_jump_window = 6` and the stage 1–3 reach rules stay; required landings ≥ 2 tiles in stage 2, 1-tile landings in stage 3 only over non-lethal ground; at most one moving hazard per required jump; note trains never cross a required gap in stage 2; no enemy may reach a moving platform's track while it can be ridden (D-032 extended to gondolas) or the jump to the Maibaum; the D-032 Taube meeting rule and D-038 "drawn = solid" apply from the first draft. Each stage keeps Stage 1's vertical layout (17 rows, street top row 15, goal meadow one row lower, Maibaum ~4–5 columns right of a launch 6 tiles above the meadow, level end 9 columns after the pole).
- Running gags: two Postbus 150 signs per stage, at the start and beside the checkpoint (D-039): Salzburg „leider voll / auch voll" → St. Gilgen „4x stündlich / alle 4 voll" → Brunnwinkl „anderes Ufer / winkt rüber" (the bus runs along the south shore). Pigeon on a monument per stage where there is one (Mozartbrunnen). The Mozart chain (Geburtshaus → „Hier wohnte / nie Mozart" → „Mozarts Mama / geboren 1720"). Brunnwinkl has exactly five houses („Fünf Häuser am See").
- Village streets use a non-solid decorative back tile `wall_village_bg` so houses reach the ground and only balconies (`balcony`, one-way) are platforms; solid house blocks (`facade_village` + `roof_shingle`) are climbed over, never left floating (M1 art note on the Getreidegasse blocks).

## D-042 Multi-stage flow and carry-over (game-designer, M2 planning)
- Stage order = the route: `salzburg`, `st_gilgen`, `brunnwinkl`, `falkenstein`, `st_wolfgang`, `wolfgangsee`, `strobl`, `bad_ischl`, `cafe_zauner`; a station is playable when a level with that id ships.
- Into the next stage carry: score, lives, Oblaten counter and the **power state** (Zuckerschock stays, as in SMB: a reward for a clean stage; the stage intro draws `player_big` then). The timer is the new stage's `time_limit`; the checkpoint is cleared.
- Game over in any stage → „Weiter?" restarts **that** stage from its start with 3 lives, score 0, Oblaten 0, Normal (D-017 unchanged, now per stage). Title → START always starts at Salzburg; no stage select and no saves in M2.
- Stage clear → route map → next stage's intro. When the next station has no level yet, the map plays a teaser walk to it and shows „Fortsetzung folgt …" for 180 frames, then the title with the run reset. This replaces the M1 `ToBeContinued` phase and needs no special case when later stages ship.
- `--level <id>` starts the run at that stage (default carry-over values) and continues along the route. Recommended for the engine: `start = playing` runs keep ending in the title after the goal sequence (M1 behaviour), so solver replays and M1 replays keep their meaning; only the title flow shows the map.

## D-043 Route map screen (game-designer, M2 planning)
- Full screen, no HUD: hand-drawn backdrop `map_salzkammergut` (no text in the art), the route drawn by the engine from waypoint data (faint dots ahead, bold dots walked), `map_station` markers (`todo`/`done`/`next`), a name label for **every** station (reached `w`, next `Y`, future `i`, `k` outline, positions per station in the route data so labels never overlap), Resi as her normal player sprite, and a D-037 panel with „Nächster Halt:" + the station name + the prompt.
- Phases: Arrive 40 frames → Walk (1 px/frame along the path) → Wait (START, or automatically after 300 frames) → next stage intro; Teaser 180 frames instead of Wait when the next station has no level. START (new press) skips Arrive/Walk to Wait; START in Wait continues. The auto-continue keeps an unattended game moving and is deterministic.
- Shown only after a stage clear in the real flow, never after deaths or continues.
- Labels in the map need „Café Zauner": the font gets `é` appended (D-012 append rule; indices stay stable). Until the glyph lands the label would draw `?`, so the glyph is part of the M2 art package.

## D-044 Gondel: free-running ping-pong cabins with a drawn cable (game-designer, M2 planning)
- The real Zwölferhornbahn is a circulating gondola; in the game each cabin shuttles back and forth on its own cable segment (`to = [dx, dy]`, optional `phase`), waiting 30 frames at each end and travelling at 1 px/frame along the major axis. Reason: it reuses the Festungsbahn's straight-line motion, collision and carrying (D-026/D-034) with only the "wait for a rider" removed, and cabins that come back are readable and fair; a true circulating loop needs a hand-off at the top station that can drop a rider.
- Unlike the Festungsbahn it never waits for the rider: timing a moving platform is the new skill. The intro cabin runs over a 2-deep trench with stairs (no death).
- The renderer draws the cable between both end positions (+1 tile) as the telegraph of the cabin's path. `|dx| ≤ 8` so a rider carried back left never reaches the screen's left edge (the camera doesn't scroll back). `levels lint` applies the 3-tile clearance and the pit-recovery check to `gondel` as to the Festungsbahn (recovery skipped where the pit is lethal, as for other floorless columns).
- Resi rides on the cabin roof next to the hanger; the „Dachfahrt / verboten" sign makes that the joke instead of a problem.

## D-045 Time limits from measured runs (game-designer, M2 planning)
- From Stage 2 on: `time_limit` = 3 × the measured relaxed full run, rounded up to 10 s, clamped to 200–300 s, set by the level-designer at level acceptance (QA measures the relaxed run). Provisional values: St. Gilgen 240 (relaxed ≈ 80 s expected, gondola waits included), Brunnwinkl 200 (≈ 60 s). Stage 1 keeps 300 (first stage, generous on purpose).
- Reason: with 300 s a 65 s Stage 1 run earns ~12 000 time bonus against ~8 000–9 000 from play (backlog note); about 3× keeps the timer relaxed for explorers while Oblaten, stomps and the Wurstkranz matter as much as speed.

## D-046 Rules for the M2 enemies (game-designer, M2 planning)
- **Nordic-Walker**: like the Selfie-Tourist but 0.75 px/frame, no selfie stops, and it turns at ledges (no solid or one-way tile under its leading foot one px ahead); it only falls when its floor disappears. 100 points.
- **Blasmusiker**: stands, faces the player; while the player is within 10 tiles a 180-frame bar runs (inhale telegraph 0–29, notes at 30/54/78); notes fly straight at 1.0 px/frame for ≤ 160 px at 10 px above his feet, pass one-way tiles and enemies, vanish on solid tiles, hurt on contact, can't be stomped. Stompable, 200 points.
- **E-Biker**: parked, triggers at < 145 px, 40-frame telegraph, rides left at 2.0 px/frame with gravity (falls into pits/water), never turns; hitting a wall makes it crash (30-frame pose, popup „Hoppala!", removed, **no points**) – so walls and boulders are hiding spots, like the Residenzbrunnen for the Fiaker. Stompable, 200 points.
- **Bienenschwarm**: anchored figure-eight (straight run of `len` tiles along `dir`, half-circle returns over 64 frames alternating sides; period 2 × (len × 16 + 64)); not stompable, hurts on any contact, can't be defeated in M2. Every swarm's run points to a bonus within one screen (level rule); longer run = farther bonus.
- Every enemy worth 200 starts a stomp chain at 200 (generalises the Verkäufer rule of D-017).
- All patterns are frame counters from activation or from entering range, never the RNG (D-017).
