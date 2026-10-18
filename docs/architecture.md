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

- `jnr-artkit` has no dependencies by default and builds for wasm; PNG encoding is behind its `png` feature, which only xtask enables.

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

Modules (M0): `fx` (fixed-point), `input`, `tuning`, `level`, `physics` (boxes + tile collision), `player`, `camera`, `game`, `text` (the `Texts` table), plus `hash` (FNV-1a) and `rng` (xorshift64*).

- **Game modes** as a state machine inside core: `Title → StageIntro → Playing → (Dying | StageClear) → ...`. Menus are part of core so they are testable too.
- **Fixed-point**: positions/velocities are `i32` in 1/256 pixel (`Fx` newtype). Identical results native vs wasm.
- **Entities**: a plain `Vec<Entity>` with an `EntityKind` enum and per-kind update functions. No ECS framework — the game is small and an enum is easier for agents to reason about.
- **Collision**: AABB vs tile grid (solid, one-way, hazard, water/death), AABB vs AABB for entities. No slopes in M1.
- **One-way platforms** block only from above; the player drops through by pressing DOWN + JUMP while standing on one (DOWN alone does nothing).
- **Camera** lives in core (it affects enemy activation like in SMB and must be deterministic).
- **Spawning**: enemies activate when they come within one screen of the camera; despawn when far behind.
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
- `RenderContext::new(&Atlas, &Texts, &Level)` is built once (by `jnr-app` at startup, again on level change) and resolves the font, the texts and the tile/player/goal sprite lookups (incl. the player anim per state), so drawing a frame does no lookups or allocations.
- `render(&Game, &RenderContext, &mut Framebuffer)`: sky (`Layer` list as the parallax hook) → tiles (`tile_<name>` sprite, else grey-box per `TileKind`) → goal (`goal` sprite, else a pole) → player (`player` sprite at the feet, anim idle/walk/run/jump/fall with fallbacks, else the hitbox box; blinks every 4 frames counted from the death) → stage-clear text (key `stage.clear`). `render_debug` is the opt-in F1 overlay (hitboxes, frame, position).

## jnr-app: macroquad shell

```
loop {
    poll keyboard (later: gamepad) → InputFrame
    accumulator += frame_time; while accumulator >= 1/60 { events = game.step(input); audio.play(events) }
    renderer.draw(&game, &mut fb); texture.update(fb); draw_texture scaled by integer factor, letterboxed
}
```

- `FilterMode::Nearest`, integer scale, black letterbox.
- CLI flags (native): `--level <id>`, `--replay <file>`, `--screenshot-at <frame>:<file>`, `--exit-after <frames>`, `--seed`, `--scale`.
- Debug keys (debug builds): F1 hitboxes, F2 frame-step, F3 slow-mo, F12 save framebuffer PNG.
- Audio: maps core `Event`s to sounds (macroquad audio). Audio is optional; a missing sound never fails.
- Wasm: built with `cargo xtask wasm`, served from `web/` with any static server. All assets are embedded (`include_bytes!`), so no fetching at runtime.

Implemented (M0):
- Native CLI (`cli.rs`, not compiled for wasm; `jnr-app --help`): `--level <id>` (embedded levels only, listed in `options::LEVELS`; currently `testbed`; unknown ids exit 2 with the list), `--seed <n>` (default 1), `--scale <n>` (initial window 320n×180n, 1..=16, default 4), `--exit-after <frames>` (quit with exit code 0 after that many simulation frames), `--screenshot-at <frame>:<png>` (repeatable; writes the plain 320×180 framebuffer after that simulation frame, frame 0 = start; a write error exits 1). `--replay`/`--record` follow with the replay format.
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

The example below is the **M1 target** (`oblaten_box` with `contains`, `oblate`, `selfie_tourist`, … are not parsed yet). `jnr-core` currently supports the tile names `ground`/`brick`/`solid`, `oneway`/`oneway_sign`, `hazard`/`salzach_water` (and `empty`) and the entities `player_start` and `goal`/`goal_maibaum`; anything else is a parse error.

```toml
[meta]
id = "salzburg"
title_key = "stage.salzburg.title"   # "Etappe 1: Salzburg"
time_limit = 300
background = ["sky_salzburg", "untersberg_far", "festung_mid", "rooftops_near"]
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

`.` is always empty air and needs no legend entry; any other grid character missing from `[legend]` is a parse error with row and column. `jnr-core` maps tile names to a collision kind (empty, solid, one-way, hazard) in `TileKind::from_tile_name`; the grey-box names `solid`, `oneway`, `hazard` work in any level (see `content/levels/00_testbed.level`).

Height: typically 14 rows (224 px, camera scrolls vertically a little). Width: ~200–320 columns for a full stage.

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
