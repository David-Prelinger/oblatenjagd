# Architecture

Status: **approved v1** — M0 in progress (see docs/milestones.md for what is implemented).

## Goals

1. Runs natively in the dev container (window shown via noVNC on `:1`).
2. Runs in the browser as `wasm32-unknown-unknown` with no code changes in the game logic.
3. Fully testable **without a window**: agents can simulate, screenshot and verify the game headlessly.
4. Deterministic: the same inputs always produce the same game, bit for bit, on every platform.
5. All art, levels, text and (later) audio are **plain-text sources** that agents can author and diff.

## Key decisions (see `decisions.md` for rationale)

| Topic | Decision |
|---|---|
| Language | Rust (edition 2024), Cargo workspace |
| Platform layer | macroquad (window, input, audio, texture upload) |
| Rendering | Own **software renderer** into a 320×180 RGBA framebuffer; macroquad only uploads and scales it |
| Simulation | Fixed timestep 60 Hz, **fixed-point math** (no `f32` in the simulation) |
| Assets | Text sources (`.palette`, `.sprite`, `.level`, `.toml`), compiled at build time and embedded |
| Tooling | `cargo xtask <cmd>` — one entry point for building assets, validation, headless runs |

## Workspace layout

```
jumpnrun/
├─ Cargo.toml                 # workspace root
├─ CLAUDE.md
├─ crates/
│  ├─ jnr-core/               # game simulation. NO I/O, NO macroquad, NO floats in sim state
│  ├─ jnr-render/             # software renderer: (&Game, &Assets) -> Framebuffer (RGBA, 320x180)
│  ├─ jnr-artkit/             # palette + .sprite parser, procedural generators, art lints
│  ├─ jnr-assets/             # build.rs compiles art/ + content/ into embedded data
│  ├─ jnr-audio/              # (later milestone) synth for SFX + music, renders to PCM/WAV
│  └─ jnr-app/                # macroquad binary: native + wasm. Thin shell.
├─ xtask/                     # dev tool: check, verify, sprites, levels, solve, replay, golden, wasm, vnc
├─ art/
│  ├─ palettes/main.palette
│  ├─ sprites/**/*.sprite
│  ├─ generators/*.toml       # parameters for procedural art (parallax, water, tiles)
│  └─ sprites/ui/font.sprite  # bitmap font incl. ÄÖÜäöüß„“– (see jnr-render)
├─ content/
│  ├─ levels/01_salzburg.level
│  ├─ text/de.toml            # all player-visible German text, keyed
│  └─ tuning.toml             # physics + gameplay constants
├─ tests/
│  ├─ replays/*.replay        # recorded inputs + expected outcomes
│  └─ golden/*.png            # approved reference screenshots
├─ web/index.html             # wasm host page
└─ docs/
```

### Dependency rules (enforced by review + a `cargo xtask check` lint)

```
jnr-app ──► jnr-render ──► jnr-core
   │            └──────────► jnr-artkit   (atlas data types; the atlas is passed in as &Atlas)
   └──► jnr-assets ──► jnr-artkit   (runtime: atlas data types only)
            ▲              │
            └─ (build.rs) ◄┘          (build time: parse, lint, pack, codegen)
xtask ──► everything (never the other way round)
```

- `jnr-artkit` depends by default only on `serde` + `toml` (generator params; pure and wasm-safe, like jnr-core) and builds for wasm; PNG encoding is behind its `png` feature, which only xtask enables.

- `jnr-core` depends on **nothing** except tiny pure crates (e.g. `serde`/`toml` for parsing tuning, if needed). Never on macroquad, std::time, rand, threads, filesystem.
- `jnr-render` depends only on `jnr-core` and asset data types. Pure Rust, no GPU.
- Only `jnr-app` knows about macroquad. Swapping macroquad (e.g. for SDL2) touches only this crate.

## jnr-core: the simulation

```rust
pub struct Game { /* all state: mode, level, player, entities, camera, rng, score, timer */ }

impl Game {
    pub fn new(level: &Level, tuning: &Tuning, seed: u64) -> Game;  // Level::parse(&str), Tuning::parse(&str)
    pub fn step(&mut self, input: InputFrame) -> &[Event];  // exactly one 1/60 s tick
    pub fn state_hash(&self) -> u64;                         // FNV-1a, for determinism checks
}

#[derive(Clone, Copy)] pub struct InputFrame(u8);  // bitflags: LEFT RIGHT UP DOWN JUMP RUN START
pub enum Event { Jumped, Landed, Died, Respawned, LevelComplete, /* later: Stomped, OblateCollected, PowerUp, Hurt, ... */ }
```

Modules (M0, + `entity` in M1): `fx` (fixed-point), `input`, `tuning`, `level`, `physics` (boxes + tile collision), `player`, `camera`, `game`, `text` (the `Texts` table), `replay` (replay file parse/format, `Replay::run` / `new_game` + `run_game`, expectation checks; see `validation.md` L5), `route` (the journey and the run's levels, M2), plus `hash` (FNV-1a) and `rng` (xorshift64*).

`Game::teleport_player(x_px)` is a tooling-only entry point (`cargo xtask scene`): it places the player on the floor at x without physics; the game, replays and the solver never call it (enforced by the architecture lint: only xtask and test code may call it).

- **Game modes** as a state machine inside core: `Title → StageIntro → Playing → (Dying | StageClear) → ...`. Menus are part of core so they are testable too.
  - Implemented (M1, E4, `game.rs` + `game/flow.rs`, D-025): `Mode::{Title, StageIntro, Playing, Paused, Dying, StageClear, GameOver}`. `Game::new_at(.., Start::Title)` is the real game (title, a 150-frame intro at the start and after every death and continue, START pauses); `Game::new` (= `Start::Playing`) starts in play without those screens (tests, solver, replays by default). `StageClear` runs the Maibaum sequence (`Game::stage_clear()`: phase `Slide → Hop → Walk → Tally → Clear → ToBeContinued`), then `Title`. The renderer draws the full screens in `jnr-render::screens`, the HUD in `hud`, signs in `signs`; `screens::TEXT_KEYS` lists every text key the engine uses (unit-tested against de.toml).
  - Multi-stage runs (M2, E10, `route.rs` + `game/flow.rs`, D-047): `route::STATIONS` is the journey (ids, map dots, label sides, waypoints); a `Route` is the run's parsed levels (stage i = station i), built outside core (`route::select`: a replay's `route` header, else the standard route of the levels named after stations, else a lone level) and handed to `Game::new_run(route, stage, ..)`. In the real flow, after the `Clear` phase `Mode::RouteMap` (`Game::route_map()`: `Arrive → Walk → Wait | Teaser`) leads to the next stage's intro (the game swaps its own level; score, lives, Oblaten, power carried over) or, after the last playable stage, to the title with the run reset to its first stage. `Start::Playing` games and lone levels keep the M1 ending (`ToBeContinued` → title). The renderer draws the map in `jnr-render::map`; callers rebuild their `RenderContext` when `!ctx.is_for(game.level())`.
- **Fixed-point**: positions/velocities are `i32` in 1/256 pixel (`Fx` newtype). Identical results native vs wasm.
- **Entities**: a plain `Vec<Entity>` with an `EntityKind` enum and per-kind update functions. No ECS framework — the game is small and an enum is easier for agents to reason about.
  - Implemented (M1, E1, `entity.rs` + `game.rs`): level spawns (`SpawnKind`) become inactive entities on every level reset; activation within `game.activate_margin` of the right screen edge, removal `game.despawn_margin` px behind the camera, below the level, or when finished (moving platforms are never removed, D-027). Kinds: `Oblate`, `PoppedOblate`, `Mozartkugel`, `Zaunerstollen`, `Debris`, `Popup(Score|ExtraLife|Checkpoint)`, `Bump` (a raised tile), `Checkpoint`, `Sign`, `Enemy(..)`, `Defeated(..)` (a stomped or knocked enemy: squished in place for `enemies.squish_frames`, or tumbling down without collision), `Thrown` (the Verkäufer's Mozartkugel: hurts, can't be stomped), `Festungsbahn` (moving one-way platform). The renderer reads `Game::entities()`, `Game::tile(col, row)` (the current grid), `Game::tile_bump_px`, `Game::checkpoint()`, `score()`, `oblaten()`, `lives()`, `time_left()`, `death_cause()`.
  - Enemy behaviour (M1, E3, `enemies.rs`, D-026): per-kind update functions driven by frame counters from activation (`Entity::timer`, `Entity::count`; never the RNG), numbers in `tuning.toml` `[enemies]`. Selfie-Tourist walks (turns at walls and at other enemies on foot, walks off ledges) with a selfie stop every `tourist_selfie_period` frames; Taube flies left on an integer sine (`enemies::sine_offset`, Bhaskara approximation, no floats) ignoring tiles; the sitting Taube waits, rises, then flies; the Verkäufer faces the player and throws while the player is in range; the Fiaker telegraphs, then charges left along its row ignoring tiles until off screen. Distances to the player are horizontal, centre to centre. Bumping a block knocks off stompable enemies standing on it (`score.bump_enemy`) and makes rolling items on it hop. Per-kind `state` values are `enemies::state::*`; the renderer reads `Entity::pose(&Tuning) -> Pose` (animation incl. the selfie flash frame) and `Entity::facing_left()` (`dir < 0`).
  - Festungsbahn (M1, E3, `platform.rs`): legend `{ entity = "festungsbahn", to = [dx, dy] }` (required; the other end point in tiles relative to the spawn cell, the car starts left-aligned in its spawn cell with its bottom on the cell's bottom edge). It waits for its rider (D-030), a state machine in `Entity::state` (`platform::car::{WAIT_LOWER, UP, WAIT_UPPER, DOWN}`, frame counter `count`): at its lower end (the larger y; `to`'s dy sign, a level track's lower end is its spawn cell) it waits until the player stands on it (`UpdateCtx::player_standing`: grounded, feet on its top), then travels up `festungsbahn_travel` frames in a straight line (position recomputed from the start point every frame, so no drift), waits `platforms.festungsbahn_wait` at the upper end, travels down and waits again. A car spawned at its upper end first travels down. Only its top edge collides, from above: `platform::land` after the player's own movement (feet crossed the top while not moving up), `platform::carry` when the car moves (a standing player moves by exactly the car's movement, with tile collision; a car rising into a player who isn't moving up lifts them). Squeezed against a ceiling, the player is held by the ceiling and drops through the car (never crushed); `cargo xtask levels lint` requires `platform::TRACK_CLEARANCE_PX` (3 tiles) of free space above the car along the whole track, and a way back up out of the pit under it (`platform::recovery_problems`: steps up of at most 3 tiles from the floor under the track to its upper end, D-028). DOWN + JUMP on the car is a normal jump (drop-through is for one-way tiles only).
  - Gondel and M2 enemies (M2, E11, D-047): `gondel` is a free-running ping-pong platform sharing the Festungsbahn's landing/carry/crush code and lints (`EntityKind::is_platform`); new kinds `Enemy(NordicWalker | Blasmusiker | EBiker | Bienenschwarm)`, `Note` (projectile) and `Gondel`; the renderer reads poses via `Entity::pose`, the swarm's shake via `Entity::shake_px` and the cable via `platform::track_ends`.
  - M3 enemies (M3, E16, D-061): `Enemy(Pilger | Gams)` (Pilger walks off ledges; Gams stand/crouch/hop with the ledge check `enemies::hop_is_clear`), `Steinschlag` (rock spawner: counter only while the player is in `range` and the niche is on screen below the HUD; `UpdateCtx::screen_top`), `Stein` (falling rock, hurts on any contact, shatters on solid/one-way tiles into `SteinDebris`). The thrown Hackl calls `Game::defeat_by_projectile(i)` (Pilger 100, Gams 200, Stein 50; `false` for other kinds). The renderer draws a `lead` Pilger as `pilger_lead` (from `EntitySpawn::lead`), the niche with `Entity::pose` (`None` = empty) and the `kiesel` trickle down `enemies::rock_column_bottom`.
  - Player power states (`Power::Normal|Big`), hurt/i-frames, 30-frame transformation freeze, score/Oblaten/lives/timer, checkpoint, level reset on death and `Mode::GameOver` live in `game.rs` (D-022).
- **Collision**: AABB vs tile grid (solid, one-way, hazard = kills on touch, water = kills once the feet sank `player.water_sink_px` in, D-050), AABB vs AABB for entities. No slopes in M1.
- **One-way platforms** block only from above; the player drops through by pressing DOWN + JUMP while standing on one (DOWN alone does nothing).
- **Camera** lives in core (it affects enemy activation like in SMB and must be deterministic).
  - Climb camera (M3, E14, D-054, `camera.rs`): a level's `[meta] camera = "side|climb"` (default `side`: the dead-zone `Camera::follow`, unchanged). `climb` uses `Camera::follow_climb` with a `ClimbFocus` (hitbox centre x, top, feet, grounded, and `Game`'s remembered `ground_feet`, the feet of the last grounded frame): horizontally the same; grounded it settles towards feet − `camera.climb_feet_y` by ≤ `climb_settle_speed` px per frame; in the air it follows up at once when the top is above screen y `climb_top`, and down while the feet are below `climb_feet_y`, at most `climb_fall_follow` px below the last ground's settle target (the limit never pulls it up). Start, respawn and stage changes frame the feet at `climb_feet_y` at once (`Camera::new_climb`); the goal sequence settles the feet at `climb_goal_feet_y` (`Camera::settle_y`). **Death line**: after the camera's follow, a hitbox top below the screen bottom (`player.y > camera.y + 180`, subpixels) dies with `DeathCause::Pit`.
- **Spawning**: enemies activate when they come within one screen of the camera; despawn when far behind. In climb levels a level entity also needs its spawn row within `camera.climb_activate_y` px of the screen vertically (`Camera::row_near`, `game::spawn_row_near`; entities without a spawn use their top row).
- **RNG**: own xorshift in state, seeded. No other randomness.
- **Text**: core only references string keys (`"hud.oblaten"`). `jnr_core::Texts::parse(&str)` flattens `de.toml` into dotted keys (`BTreeMap`); `get(key)` falls back to `[key]` for missing keys, never panics. The app embeds `de.toml`; the renderer resolves keys through its `RenderContext`.
- `Game: Clone` and cheap to clone (needed by the level-solver bot, see `validation.md`).

## jnr-render: software renderer

- Draws into `Framebuffer { w: 320, h: 180, pixels: Vec<u32> }`.
- Layers: parallax backgrounds → tiles → entities → player → foreground tiles → particles → HUD / text.
- Pure pixel ops: blit with transparency, horizontal flip, palette swap, bitmap text. No rotation/scaling (pixel-art purity).
- The **same renderer** is used by the real game and by headless screenshots, so a golden screenshot is exactly what the player sees.
- Cost: 320×180 = 57 600 pixels, trivially fast on CPU, also in wasm.

Implemented (M0):
- `jnr-render` never depends on `jnr-assets` (only as a dev-dependency for a sanity test): every draw call takes `&Atlas`, so tests use small hand-built atlases.
- `Framebuffer` (`fill_rect`, `outline_rect`, `get`/`set`, `to_rgba_bytes`/`write_rgba_bytes`); `blit_frame` and `draw_sprite` (origin placement via `origin_facing`, mirrored frames or flip fallback, optional tint). Atlas alpha 0 is skipped, everything else drawn opaque; all blits clip at every edge.
- Font (D-012): sprite `font`, 8×8 frames, one anim `glyphs`, one frame per character of `jnr_render::CHARSET` (89 glyphs, order fixed). Advance = rightmost opaque column + 2, space = 3 px, line height 9 (room for the +1 shadow); `measure` leaves out the gap after a line's last glyph. Unknown characters draw as `?`. No `font` sprite → text is a no-op.
- Implemented (M1): parallax backgrounds (`background.rs`): `RenderContext::with_background(&[(sprite, factor16, y, factor_y16)])`, layers back to front, tiled horizontally at `-((camera.x * factor / 16) mod width)`, bottom-aligned to the screen (raised by `y`); vertical parallax (M3, D-054): a layer is drawn `background::sink_px` = `(max camera y − camera.y) × factor_y / 16` px lower (0 at the level bottom, never up; `RenderContext` keeps the level's max camera y); the flat sky `s` stays behind them unless the first layer is opaque and full-height; missing sprites are skipped. Autotiling (`tiles.rs`, D-020): `TileLook` per tile name resolved once; `auto` (16 frames) picks the frame by `autotile_mask` (computed from the live level while drawing), other tiles animate by the game frame.
- `RenderContext::new(&Atlas, &Texts, &Level)` is built once (by `jnr-app` at startup, again on level change) and resolves the font, the texts and the tile/player/goal sprite lookups (incl. the player anim per state), so drawing a frame does no lookups or allocations.
- `render(&Game, &RenderContext, &mut Framebuffer)`: backgrounds → tiles (`tile_<name>` sprite, else grey-box per `TileKind`) → goal (`goal` sprite, else a pole) → player (`player` sprite at the feet, anim idle/walk/run/jump/fall with fallbacks, else the hitbox box; blinks every 4 frames counted from the death) → stage-clear text (key `stage.clear`). `render_debug` is the opt-in F1 overlay (hitboxes, frame, position).

## jnr-app: macroquad shell

```
loop {
    poll keyboard (later: gamepad) → InputFrame
    accumulator += frame_time; while accumulator >= 1/60 { events = game.step(input); audio.play(events) }
    renderer.draw(&game, &mut fb); texture.update(fb); draw_texture scaled by integer factor, letterboxed
}
```

- `FilterMode::Nearest`, integer scale, black letterbox.
- CLI flags (native; `--help` lists them): `--level <id>`, `--seed <n>`, `--scale <n>`, `--exit-after <frames>`, `--screenshot-at <frame>:<file>` (repeatable), `--replay <file>`, `--record <file>`. Wasm always uses the defaults: Stage 1 (`salzburg`) from its title screen.
- App levels (M1, E6): `options::LEVELS` is an explicit `include_str!` list of every `content/levels/*.level` (id = `[meta] id`); a unit test fails when it differs from the directory, so a new level file must be added there. `DEFAULT_LEVEL = "salzburg"`; the real game (native without flags, and wasm) starts at the Salzburg title (`Start::Title`). The test levels (`testbed`, `items`, `enemies`) stay reachable with `--level <id>` and via `--replay`.
  - `--replay`: parsed with `jnr_core::replay`; its `level` must be embedded and its `level`/`seed` are used (a conflicting `--level`/`--seed` is an error). The input of step N is the replay's frame N (`InputSource::input_for`), nothing after the last one; game input comes only from the replay (Escape quits; F1/F12 still work).
  - `--record`: `record::Recorder` RLE-encodes the input of every step and tracks a `replay::Outcome`; on exit (window close, Escape, `--exit-after`) it writes a replay with `source = human` and a derived `expect` (`complete frames=N..N`, else `state=<mode> player_x>=<x>`; always `deaths=<n>`), creating directories, and prints `recorded: <path>`. Not combinable with `--replay`.
- Escape and closing the window (native) end the loop cleanly (`prevent_quit`), so the recording can be written; the process exits 0.
- Debug keys (debug builds): F1 hitboxes, F2 frame-step, F3 slow-mo, F12 save framebuffer PNG.
- Audio: maps core `Event`s to sounds (macroquad audio). Audio is optional; a missing sound never fails.
- Wasm: built with `cargo xtask wasm`, served from `web/` with any static server. All assets are embedded (`include_bytes!`), so no fetching at runtime.

Implemented (M0):
- Native CLI (`cli.rs`, not compiled for wasm; `jnr-app --help`): `--level <id>` (embedded levels only, listed in `options::LEVELS`: `enemies`, `items`, `testbed`, `salzburg` (default); unknown ids exit 2 with the list), `--seed <n>` (default 1), `--scale <n>` (initial window 320n×180n, 1..=16, default 4), `--exit-after <frames>` (quit with exit code 0 after that many simulation frames), `--screenshot-at <frame>:<png>` (repeatable; writes the plain 320×180 framebuffer after that simulation frame, frame 0 = start; a write error exits 1). `--replay`/`--record` follow with the replay format.
- The "what happens after frame N" decision is the pure `Options::actions_after(frame)`; it runs after every tick (not every render frame), so screenshots and exits are exact even when several ticks run per render frame.
- F12 (native debug builds) saves the framebuffer as shown (incl. F1 overlay) to `$CARGO_TARGET_DIR/preview/f12_<frame>.png` (else `./f12_<frame>.png`) and prints the path. F2/F3 are not implemented yet.
- PNG encoding uses the `png` crate as a native-only dependency of `jnr-app`.
- Wasm uses `Options::default()` (no command line). `web/index.html` loads `web/gl.js` (unmodified copy of miniquad's loader, version in `web/README.md`) and `jnr-app.wasm`; it sets `<body data-wasm="loaded">` once the module is instantiated. `cargo xtask wasm [--debug] [--serve] [--port N]` builds (release by default) and assembles `$CARGO_TARGET_DIR/web/`; `cargo xtask wasm-smoke` (a `verify` step) loads it in headless Chromium (SwiftShader WebGL) and checks the marker and the screenshot `$CARGO_TARGET_DIR/preview/wasm_smoke.png` (not blank, ≥ 25 % sky colour `s`).

## jnr-artkit + jnr-assets: asset pipeline

- `jnr-artkit` is a library: parses `.palette` and `.sprite`, runs procedural generators, packs atlases, runs art lints. Details in `art-pipeline.md`.
- `jnr-assets/build.rs` calls `jnr-artkit` to compile all art into one packed RGBA atlas: raw pixels (`include_bytes!`, borrowed without copying) plus generated Rust code for the metadata. At runtime `jnr_assets::atlas()` builds the `jnr_artkit::atlas::Atlas` once via `OnceLock` (D-010), so `jnr-assets` depends on `jnr-artkit` at runtime for these data types. A broken sprite = a **build error** with file:line:col.
- Levels and text are embedded as strings and parsed by `jnr-core` (`Level::parse(&str)`), so tests and tools can load them directly.
- Generated files are **not committed**; sources are the single truth. Previews go to `target/preview/`.

## Content formats

### Level (`content/levels/*.level`)

TOML header + one ASCII grid. One char = one 16×16 tile or an entity spawn.

Format v2 (M1, D-022). Tile names: grey-box `solid`/`oneway`/`hazard`/`empty` and the catalog tiles (`ground`, `meadow`, `rock`, `rampart`, `hedge`, `stone`, `bridge`, `railing`, `arch`, `brick`, `oblaten_box`, `oblaten_box_empty` → solid; `railing_bg` → decorative air; `oneway_sign`, `oneway_burger`, `oneway_wood` → one-way; `salzach_water` → hazard; M3: `kalkfels` → solid, `kalkfels_bg`, `netz_post` → decorative air, `steig`, `steig_wall` (the plank drawn over the back wall), `netz` → one-way). `contains = "oblate|mozartkugel|zaunerstollen|oblaten_10"` is allowed on `brick` and `oblaten_box` only (a box defaults to `oblate`). Entities: `player_start` (exactly one), `goal`/`goal_maibaum`, `checkpoint`, `oblate`, `sign` (`text` key required, `style = "street|bus|ortsende"`, default street), `selfie_tourist` (optional `still = true`: never walks, for grey-box `00_*` test levels only, D-026), `taube`, `taube_sitting`, `mozartkugel_verkaeufer`, `fiaker`, `festungsbahn` (`to = [dx, dy]` required: its other end point in tiles relative to its spawn cell, each within ±64, not both 0; the spawn cell is where it starts, e.g. the bottom end), `prop` (`sprite` required, one of `jnr_core::PROP_SPRITES` (checked by `levels lint`); optional `layer = "front|back"` (default front: after the tiles, behind the player; back: after the parallax, before the tiles) and `dx` (px, ±64); decorative, no collision, not a core entity — the renderer draws it from the spawn with its origin on the cell's bottom centre + `dx`, grey-box: the cell's outline). M2/M3 enemies: `nordic_walker`, `blasmusiker`, `e_biker`, `bienenschwarm` (`dir`, `len`, `phase`, `dy`), `gondel` (`to`, `phase`) (D-048); `pilger` (optional `lead = true`, the pennant look), `gams`, `steinschlag` (`period` 90..=300, default 150; `phase` 0 or 41..period, default 0; `range` 4..=10 tiles, default 6) (D-061). Tiles `facade`, `arch_under`, `invisible` (never drawn) → solid; `arch`/`arch_under` are one autotile family (`autotile_family` in `jnr-render/src/tiles.rs`); `one_way_join` there lets a tile count a different neighbour as the same on some sides only (`kalkfels_bg` around `steig_wall`; `kalkfels` under `forest_path`/`meadow`/`stone`/`ground`, top only). Unknown names or fields are parse errors.

```toml
[meta]
id = "salzburg"
title_key = "stage.salzburg.title"   # "Etappe 1: Salzburg"
time_limit = 300                     # required, 1..=999 seconds
min_jump_window = 6                  # optional, frames (checked by the solver)
camera = "side"                      # optional: "side" (default, dead zone) or "climb" (D-054)
background = [                       # far to near; factor in 1/16 of the camera speed (0..=32)
  { sprite = "bg_sky_salzburg", factor = 0 },
  { sprite = "bg_untersberg", factor = 2, y = 24 },   # y: px above the screen bottom (default 0)
  { sprite = "bg_festung", factor = 4 },
  { sprite = "bg_rooftops", factor = 8 },
  # optional factor_y (0..=32, sixteenths, default 0): sinks the layer as the camera climbs (D-054)
]
music = "salzburg"

[legend]
"#" = "ground"
"B" = "brick"
"?" = { tile = "oblaten_box", contains = "oblate" }
"M" = { tile = "oblaten_box", contains = "mozartkugel" }
"-" = "oneway_sign"            # wrought-iron guild sign (Zunftschild) platform
"~" = "salzach_water"          # death
"o" = { entity = "oblate" }
"t" = { entity = "selfie_tourist" }
"f" = { entity = "fiaker" }
"P" = { entity = "player_start" }
"G" = { entity = "goal_maibaum" }

[map]
grid = '''
................................................
....................???.........................
...P....................t.......................
################################~~~~############
'''
```

`.` is always empty air and needs no legend entry; any other grid character missing from `[legend]` is a parse error with row and column. `jnr-core` maps tile names to a collision kind (empty, solid, one-way, hazard, water: the `*_water*` tiles, D-050) in `TileKind::from_tile_name`; the grey-box names `solid`, `oneway`, `hazard` work in any level (see `content/levels/00_testbed.level`).

An entity's cell is empty air by default. Any entity entry may add `tile = "<tile name>"` to draw that tile in its cell under the spawn, e.g. `"w" = { entity = "nordic_walker", tile = "wall_village_bg" }` for a walker in front of a house wall or `{ entity = "oblate", tile = "wall_village_bg" }`. The tile must be non-colliding (`TileKind::Empty`: `wall_village_bg`, `railing_bg`) and takes no `contains`; anything else is a parse error. It is stored in the grid (`Level::tiles`), not in the `EntitySpawn`, so the entity behaves exactly as on air.

Height: typically 14 rows (224 px, camera scrolls vertically a little); climb levels are taller (Falkenstein: 42 rows). Width: ~200–320 columns for a full stage.

### Text (`content/text/de.toml`)

```toml
[hud]
oblaten = "OBLATEN"
time = "ZEIT"
lives = "LEBEN"
[stage.salzburg]
title = "Etappe 1: Salzburg"
```

The renderer's font covers German special characters. Typos are checked by review (`german-satire-writing` skill), not tooling.

### Tuning (`content/tuning.toml`)

All physics/gameplay numbers (accelerations, jump impulse, gravity, coyote frames, …) in one file, in fixed-point units, so designers can tune without touching code and tests can assert the resulting game-feel metrics.

## Determinism contract

- No floats in `jnr-core` state or logic. (Floats allowed in render/app for scaling only.)
- No `HashMap`/`HashSet` iteration in core (use `Vec`/`BTreeMap`).
- No time, threads, filesystem or global state in core.
- Every replay test also runs the game twice and compares `state_hash()`; later the headless runner is built for `wasm32-wasip1` and compared against native (see `validation.md`).

## Out of scope for now

Slopes, save games, gamepad support (macroquad lacks native gamepad; add `gilrs`/web Gamepad API later), mobile touch controls, localisation beyond German (format is ready for it).
