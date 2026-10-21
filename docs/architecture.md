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
│  ├─ jnr-audio/              # integer synth: .sfx/.inst parser, renderer to 44.1 kHz mono i16, WAV (MA)
│  └─ jnr-app/                # macroquad binary: native + wasm. Thin shell.
├─ xtask/                     # dev tool: check, verify, sprites, levels, solve, replay, golden, wasm, vnc
├─ art/
│  ├─ palettes/main.palette
│  ├─ sprites/**/*.sprite
│  ├─ generators/*.toml       # parameters for procedural art (parallax, water, tiles)
│  └─ sprites/ui/font.sprite  # bitmap font incl. ÄÖÜäöüß„“– (see jnr-render)
├─ audio/                     # sound sources (MA): sfx/*.sfx, jingles/*.sfx, instruments/*.inst, music/*.song, targets.toml (lint targets)
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
jnr-audio ──► jnr-core   (the cues read events and game state; E58a)
xtask ──► everything (never the other way round)
```

- `jnr-audio` is pure and wasm-safe like `jnr-core` (`serde` + `toml` only; never macroquad, quad-snd, rand) and under the same determinism source scan (D-117); it also may not mention `as_float`, `Value::Float` or `toml::Value` (every audio number is an integer key). `jnr-render` never depends on it.

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
pub enum Event { Jumped, Landed, Died, Respawned, LevelComplete, OblateCollected, PowerUp, Hurt, ExtraLife, BlockBumped,
                 BrickBroken, Stomped, CheckpointReached, GameOver, HacklThrown, AxeHit, CrawlStarted, Stroke,
                 Bounced /* MA, D-113: output only, never hashed */ }
```

Modules (M0, + `entity` in M1, + `swim` in M5): `fx` (fixed-point), `input`, `tuning`, `level`, `physics` (boxes + tile collision), `player`, `camera`, `game`, `text` (the `Texts` table), `replay` (replay file parse/format, `Replay::run` / `new_game` + `run_game`, expectation checks; see `validation.md` L5), `route` (the journey and the run's levels, M2), plus `hash` (FNV-1a) and `rng` (xorshift64*).

`Game::teleport_player(x_px)` is a tooling-only entry point (`cargo xtask scene`): it places the player on the floor at x without physics; the game, replays and the solver never call it (enforced by the architecture lint: only xtask and test code may call it).

- **Game modes** as a state machine inside core: `Title → StageIntro → Playing → (Dying | StageClear) → ...`. Menus are part of core so they are testable too.
  - Implemented (M1, E4, `game.rs` + `game/flow.rs`, D-025): `Mode::{Title, StageIntro, Playing, Paused, Dying, StageClear, GameOver}`. `Game::new_at(.., Start::Title)` is the real game (title, a 150-frame intro at the start and after every death and continue, START pauses); `Game::new` (= `Start::Playing`) starts in play without those screens (tests, solver, replays by default). `StageClear` runs the Maibaum sequence (`Game::stage_clear()`: phase `Slide → Hop → Walk → Tally → Clear → ToBeContinued`; a `goal_stollen` has `Serve` instead of `Slide → Hop → Walk`, M7), then `Title`. The renderer draws the full screens in `jnr-render::screens`, the HUD in `hud`, signs in `signs`; `screens::TEXT_KEYS` lists every text key the engine uses (unit-tested against de.toml).
  - Multi-stage runs (M2, E10, `route.rs` + `game/flow.rs`, D-047): `route::STATIONS` is the journey (ids, map dots, label sides and slots, waypoints; a station's `label_alt` slot is used on the map screens ending on one of its `label_alt_ends` for every form, or with a big Resi covering the primary slot at the end dot: D-072, D-098); a `Route` is the run's parsed levels (stage i = station i), built outside core (`route::select`: a replay's `route` header, else the standard route of the levels named after stations, else a lone level) and handed to `Game::new_run(route, stage, ..)`. In the real flow, after the `Clear` phase `Mode::RouteMap` (`Game::route_map()`: `Arrive → Walk → Wait | Teaser`) leads to the next stage's intro (the game swaps its own level; score, lives, Oblaten, power carried over) or, after the last playable stage, to the title with the run reset to its first stage. `Start::Playing` games and lone levels keep the M1 ending (`ToBeContinued` → title). The renderer draws the map in `jnr-render::map`; callers rebuild their `RenderContext` when `!ctx.is_for(game.level())`.
- **Fixed-point**: positions/velocities are `i32` in 1/256 pixel (`Fx` newtype). Identical results native vs wasm.
- **Entities**: a plain `Vec<Entity>` with an `EntityKind` enum and per-kind update functions. No ECS framework — the game is small and an enum is easier for agents to reason about.
  - Implemented (M1, E1, `entity.rs` + `game.rs`): level spawns (`SpawnKind`) become inactive entities on every level reset; activation within `game.activate_margin` of the right screen edge, removal `game.despawn_margin` px behind the camera, below the level, or when finished (moving platforms are never removed, D-027). Kinds: `Oblate`, `PoppedOblate`, `Mozartkugel`, `Zaunerstollen`, `Debris`, `Popup(Score|ExtraLife|Checkpoint)`, `Bump` (a raised tile), `Checkpoint`, `Sign`, `Enemy(..)`, `Defeated(..)` (a stomped or knocked enemy: squished in place for `enemies.squish_frames`, or tumbling down without collision), `Thrown` (the Verkäufer's Mozartkugel: hurts, can't be stomped), `Festungsbahn` (moving one-way platform). The renderer reads `Game::entities()`, `Game::tile(col, row)` (the current grid), `Game::tile_bump_px`, `Game::checkpoint()`, `score()`, `oblaten()`, `lives()`, `time_left()`, `death_cause()`.
  - Enemy behaviour (M1, E3, `enemies.rs`, D-026): per-kind update functions driven by frame counters from activation (`Entity::timer`, `Entity::count`; never the RNG), numbers in `tuning.toml` `[enemies]`. Selfie-Tourist walks (turns at walls and at other enemies on foot, walks off ledges) with a selfie stop every `tourist_selfie_period` frames; Taube flies left on an integer sine (`enemies::sine_offset`, Bhaskara approximation, no floats) ignoring tiles; the sitting Taube waits, rises, then flies; the Verkäufer faces the player and throws while the player is in range; the Fiaker telegraphs, then charges left along its row ignoring tiles until off screen. Distances to the player are horizontal, centre to centre. Bumping a block knocks off stompable enemies standing on it (`score.bump_enemy`) and makes rolling items on it hop. Per-kind `state` values are `enemies::state::*`; the renderer reads `Entity::pose(&Tuning) -> Pose` (animation incl. the selfie flash frame) and `Entity::facing_left()` (`dir < 0`).
  - Festungsbahn (M1, E3, `platform.rs`): legend `{ entity = "festungsbahn", to = [dx, dy] }` (required; the other end point in tiles relative to the spawn cell, the car starts left-aligned in its spawn cell with its bottom on the cell's bottom edge). It waits for its rider (D-030), a state machine in `Entity::state` (`platform::car::{WAIT_LOWER, UP, WAIT_UPPER, DOWN}`, frame counter `count`): at its lower end (the larger y; `to`'s dy sign, a level track's lower end is its spawn cell) it waits until the player stands on it (`UpdateCtx::player_standing`: grounded, feet on its top), then travels up `festungsbahn_travel` frames in a straight line (position recomputed from the start point every frame, so no drift), waits `platforms.festungsbahn_wait` at the upper end, travels down and waits again. A car spawned at its upper end first travels down. Only its top edge collides, from above: `platform::land` after the player's own movement (feet crossed the top while not moving up), `platform::carry` when the car moves (a standing player moves by exactly the car's movement, with tile collision; a car rising into a player who isn't moving up lifts them). Squeezed against a ceiling, the player is held by the ceiling and drops through the car (never crushed); `cargo xtask levels lint` requires `platform::TRACK_CLEARANCE_PX` (3 tiles) of free space above the car along the whole track, and a way back up out of the pit under it (`platform::recovery_problems`: steps up of at most 3 tiles from the floor under the track to its upper end, D-028). DOWN + JUMP on the car is a normal jump (drop-through is for one-way tiles only).
  - Gondel and M2 enemies (M2, E11, D-047): `gondel` is a free-running ping-pong platform sharing the Festungsbahn's landing/carry/crush code and lints (`EntityKind::is_platform`); new kinds `Enemy(NordicWalker | Blasmusiker | EBiker | Bienenschwarm)`, `Note` (projectile) and `Gondel`; the renderer reads poses via `Entity::pose`, the swarm's shake via `Entity::shake_px` and the cable via `platform::track_ends`.
  - Schafbergbahn (M4, E21, D-067): `EntityKind::Schafbergbahn` / `SpawnKind::Schafbergbahn` (`{ entity = "schafbergbahn", to = [dx, dy] }`, 4 ≤ |dx| ≤ 32) runs on the Festungsbahn's waiting state machine with its own timing (`platform::car_timing` → `CarTiming { travel, wait }`: travel = max(|dx|, |dy|) × `platforms.schafbergbahn_frames_per_tile`, `schafbergbahn_wait` at the top; body `schafbergbahn_size`, whole tiles, validated). Its position comes from `platform::track_offset`: x on the straight line, the height snapped to whole px on the rail (`platform::rail_offset`, −floor(u·(−dy)/dx)), so the roof never sits between two rail rows. Landing, carrying, crush, never-despawn and the pit-recovery lint are the shared platform code; `platform::track_problems` checks its **swept body** plus 3 tiles above the roof free of solid and one-way tiles. Axe: `Pass`. The renderer draws its track behind the tiles (`entities::draw_track`: `d` rails at Y and Y+3, `m` rack, `O` sleepers, a `d` girder at Y+5 under the whole track, Y = the spawn feet y + 1 + `rail_offset`, where the sprite's wheels stand), the train (`schafbergbahn`, `ride` while moving, `idle` standing; grey box `k` with a funnel stub) and `dampf` puffs computed from its counter (`entities::steam_puffs`: every 20 travel frames, 24 frames each, rising 1 px/frame where they left the funnel). Test level `00_schafbergbahn`, replays `schafbergbahn_*`.
  - M3 enemies (M3, E16, D-061): `Enemy(Pilger | Gams)` (Pilger walks off ledges; Gams stand/crouch/hop with the ledge check `enemies::hop_is_clear`), `Steinschlag` (rock spawner: counter only while the player is in `range` and the niche is on screen below the HUD; `UpdateCtx::screen_top`), `Stein` (falling rock, hurts on any contact, shatters on solid/one-way tiles into `SteinDebris`). The thrown Hackl calls `Game::defeat_by_projectile(i)` (Pilger 100, Gams 200, Stein 50; `false` for other kinds). The renderer draws a `lead` Pilger as `pilger_lead` (from `EntitySpawn::lead`), the niche with `Entity::pose` (`None` = empty) and the `kiesel` trickle down `enemies::rock_column_bottom`.
  - M4 singer and stand (M4, E22, D-068/D-069 + addenda): `Enemy(Operettensaenger)` (range counter in `count`, states `STAND`/`INHALE`/`SING`; the high-C column is no entity: `enemies::high_c_column(&Entity, &TileMap, &Tuning) -> Option<Aabb>`, recomputed from his counter every frame, clipped at the first solid tile, checked in `Game::interact` before every other contact rule and drawn by the renderer as stacked `hohes_c` rings), `Souvenirstand` (spawner, hitbox = its cell, states `WAITING`/`COUNTING`/`SKIPPING`; it counts its own globes alive through `UpdateCtx::globes`, the spawn points of every alive globe), `Enemy(Schneekugel)` (rolls left, shatters on walls) and `SchneekugelScherben` (4 cosmetic shards: every shatter, by a wall, a stomp or an axe, removes the globe at once and spawns them, so a `Defeated(Schneekugel)` is never drawn). `Popup::Bravo` („Bravo!", `enemies::bravo`) stacks above the score popup of every defeated singer (stomp, axe, bumped block).
  - Player power states (`Power::Normal|Big|Hackl`, `is_big`, `after_hit`), hurt/i-frames, 30-frame transformation freeze (`Player::prev_power` for the alternating looks), score/Oblaten/lives/timer, checkpoint, level reset on death and `Mode::GameOver` live in `game.rs` (D-022).
  - Hackl and Durchkriechstein (M3, E15, D-060): `game/power.rs` (pickup, throw on a new RUN press, `Game::axe_hits`, the crawl), `hackl.rs` (the axe's flight and the **axe-hit hook** `EnemyKind::axe_hit` / `Entity::axe_hit` → `AxeHit::{Pass, Defeat, Bounce, Shatter}`, exhaustive per kind; hits go through E16's `Game::defeat_by_projectile` first), `crawl.rs` (`starts`, `geometry_problems` for the lint). Kinds `Hackl` (item), `ThrownHackl` (`hackl::axe::{FLY, STUCK}`, poses `Spin`/`Stuck`), `Durchkriechstein` (tunnel hitbox, `crawl::stone::{UNUSED, USED}`, not drawn). Player fields `run_held`, `throw_pose`, `crawl_to` (`crawling()`); events `HacklThrown`, `AxeHit`, `CrawlStarted`; tuning `[hackl]`, `[crawl]`. The renderer draws `player_hackl` (fallback: `player_big` with `r` → `G`), its `throw` (frame by the frames since the throw) and `crawl` anims (no `crawl`: hidden), the intro and the route map in the carried power state.
  - Reisebus (M4, E23, D-070): `Enemy(Reisebus)` (`enemies::update_reisebus`, states `PARKED → TELEGRAPH → DRIVE`, with a `stop` also `PHOTO_STOP → RESTART → DRIVE`; poses `Idle`, `Telegraph`, `Drive`, `Fotostopp { flash }`). Spawned left-aligned over 4 cells from its spawn cell (hitbox centred under the 64 px sprite); `Entity::base_x` keeps the parked x, the photo stop is `base_x − stop` tiles (clamped exactly, so any whole-tile `stop` works at 1.5 px/frame). Not stompable (`EnemyKind::stompable`), so the generic contact rule gives the Fiaker's roof bounce (no damage, no points, D-027 feet-on-top) and side hurt in every state; axe `Bounce`. `on_foot` (walkers turn at it) unless driving. Tuning `enemies.reisebus_*`. Lint `enemies::reisebus_lane_problems` (`levels lint`): its hitbox rows from its own cells to `REISEBUS_LANE_TILES` (24) tiles left, clamped at the level's left edge, hold no solid or one-way tile, and its photo stop lies on the level. Grey box until the art arrives: white box, `d` window band, blinking `Y` hazard lights (`BUS_HAZARD_TICKS`), white flashes on the photo stop's flash frames (renderer `draw_bus_marks`). Test level `00_m4_bus` with `m4_bus_*` replays.
  - Boats (M5, E28, D-077 + addendum): `EntityKind::Traundl` / `SpawnKind::Traundl` (`{ entity = "traundl", to = [dx, 0], phase = N }`, 2 ≤ |dx| ≤ 8) is the Gondel's free-running machine with its own numbers (`platform::free_timing` → `CarTiming { travel, wait }`; `place_free`/`update_free` serve both kinds; hull `platforms.traundl_size` 48×16, `traundl_wait`, `traundl_frames_per_tile`). `EntityKind::Dampfer` / `SpawnKind::Dampfer` (`{ entity = "dampfer", to = [dx, 0] }`, 8 ≤ dx ≤ 64) runs on the Festungsbahn's waiting state machine **one way** (`CarTiming::moor`: after its travel of dx × `dampfer_frames_per_tile` frames it goes to `platform::car::MOORED` and stays there until the level resets; hull `dampfer_size` 160×16, whole tiles, up to 320 px). Both are placed left-aligned in their spawn cell with the bottom on the cell's bottom (the water line) and share landing, carrying, crush, never-despawn, `world_hash` and the lints via `EntityKind::is_platform`; axe `Pass`. The **paddle wheel** is no entity: `platform::paddle_wheel(&Entity, &Tuning) -> Option<Aabb>` (only while the Dampfer is `UP`: `dampfer_wheel_zone` 32×20 centred on the hull's centre x, its top `dampfer_wheel_rise` 4 px above the hull's bottom), checked in `Game::interact` right after the high-C column: any overlap hurts (never stomped). `platform::recovery_problems` skips columns whose first tile below the track is swim water and passes when the exit column is swim water; lethal water is looked through (D-084). Renderer (`entities::draw_traundl`, `draw_dampfer`): `traundl` (`ride` while it moves, else `idle`; grey box `l` with a `G` rower stub at the stern), `dampfer` (`ride` while it travels, else `idle`; grey box `w` with an `o` funnel stub and the wheel zone outlined `d` while it turns), both never mirrored; two `gischt` `spray` sprites on the water line ±8 px around the wheel's centre while it turns (grey box: white spray under the water line). Test level `00_m5_boats` with `m5_boats_*` replays.
  - Fensterladen and the stage clock (M6, E37, D-094): `Game::stage_clock() -> u32` (hashed in `state_hash`) counts the frames the ZEIT timer counts since the stage start or the last respawn: it ticks in `Game::tick_timer` (so frozen, paused, dying, intro and goal-sequence frames don't count) and `reset_level` sets it to 0 (stage start, respawn, continue, the next stage of a run). Every timed entity reads it (the Offiziere's drill, D-096, too). `EntityKind::Fensterladen` / `SpawnKind::Fensterladen` (`{ entity = "fensterladen", period, open, phase }`: `period` 120–480 (240), `open` 60..=period − 30 (150), `phase` 0..period (0); hash tag 28, 28–29 reserved) is a stationary `platforms.fensterladen_size` 32×8 body, left-aligned in its cell with its top on the cell's top edge. Its frame f = (clock + `phase`) mod `period` (`platform::shutter_frame`) is its `count` and `state` = `platform::shutter::{OPEN, CLOSED}` (open for f < `open`); `Game::place_shutters` sets them for every shutter, active or not, right after each clock tick (and `Entity::from_spawn` at clock 0), so they never drift and keep an exact relative phase. Its own update does nothing. It is a platform (`is_platform`: never despawns, `world_hash` with state and count, the clearance and recovery lints as a zero-length track via `platform::track`); landing and standing go through the shared `platform::land`, which only takes platforms that `platform::carries` (a Fensterladen only while `OPEN`). So a closing one stops holding its rider (she falls with the coyote frames `stand` gave her; nothing is pushed or crushed) and an opening one never lifts a player whose feet are below its top. Axe `Pass`. Renderer (`entities::draw_fensterladen`): `platform::shutter_look` → `ShutterLook::{Open, Rattle, Closed, Warn}` (the last `fensterladen_rattle` 30 open and the last `fensterladen_warn` 20 closed frames) picks the `fensterladen` anim of that name, drawn at the body's top + 24 px; grey box until the art lands: an `r` body (rattling: shaken 1 px down every other 4 ticks, drawn only), closed two folded-up `r` leaves, warning plus an `e` face. Test level `00_m6_shutters` with `m6_shutters_*` replays.
  - M5 lake enemies (M5, E29, D-078 … D-081 + addenda): `Enemy(Schwan | Gelse | Regattaboot)` (hash ids 14–16). **Schwan** (`enemies::update_schwan`, states `HOME → TELEGRAPH → CHARGE → PADDLE_BACK → REST → HOME`, `base_x` = its home; poses `Idle`/`Telegraph`/`Charge`/`Squished`), floating with its hitbox bottom `schwan_float_px` under its cell's top; stompable for 200 (chain start 200), every defeat (stomp, axe) shows „Beleidigt!" (`Popup::Beleidigt`) stacked above the score like „Bravo!" (`enemies::defeat_popup`). **Gelse** (`enemies::update_gelse`, states `HOVER → HUNT → LEAVE`, poses `Wait`/`Fly`/`Leave`; the renderer bobs a hovering one ±2 px, `entities::gelse_bob_px`): the hover counts only frames it is fully on screen; the hunt reads `UpdateCtx::player_top` (new); its moves are cut at the edge of any water tile (`enemies::move_clear_of_water`); leaving it is harmless (`Entity::harmful`) and lets axes pass. **Regattaboot**: the Reisebus's machine (`update_reisebus` with the `regattaboot_*` keys, no `stop`), 2 cells in the surface row, hitbox bottom `regattaboot_keel_px` under the cell top; not stompable, so the generic contact rule bounces off its masthead, but only off the 8×20 masthead part (`enemies::regatta_parts`, `regattaboot_mast`); the body below it (full width) hurts on any contact; axe `Bounce` (whole hitbox); drawn from the sprite's bottom (cell bottom), 4 px below the hitbox. The **contact rule** is `enemies::contact(kind, from_above, swimming) -> Contact::{Stomp, Bounce, Hurt}` (every contact hurts while swimming; `Game::player_swimming` is `Player::swimming`, E30). Triggers that need the enemy fully on screen use a private `enemies::fully_on_screen` (to be replaced by E26's camera helper). The lane lint `enemies::reisebus_lane_problems` covers the Regattaboot too (D-080 message). The M5 kinds use swim water (`see_wasser`) as their surface; the Gelse's moves are cut at lethal and swim water alike (E30). Grey boxes until the art arrives: Schwan white (two white wing posts while it hisses), Gelse dark `k`, Regattaboot cream with a `d` mast, a blinking `r` start flag while it telegraphs and a `Y` bow wave while it races (`draw_boat_marks`). Test level `00_m5_enemies` with `m5_enemies_*` replays.
  - Traffic (M6, E36, D-093/D-097 addenda): `EntityKind::Ampel` (hash tag 26) is a spawner in the road-level air cell of a side street's mouth (`enemies::update_ampel`, states `WAITING`/`COUNTING`, `count` = cycle frame): its counter runs only while the player's centre is left of it, within `range` and the cell is fully on screen (`UpdateCtx::fully_on_screen`), and at f = `ampel_telegraph` + k × `ampel_gap` it sends car k unless `ampel_max_alive` of its cars are alive (`UpdateCtx::cars`, by spawn point) or the car would spawn inside the player (`UpdateCtx::player_box`). Its light is `enemies::ampel_light` → `AmpelLight`. `Enemy(Auto)` (id 17) drives left at `auto_speed` through everything, is removed off screen left, and takes the Reisebus's contact rule (roof bounce without damage or points, side hurt) and axe `Bounce`; its `count` (index in its green) picks the renderer's look. The lane lint `enemies::reisebus_lane_problems` covers Ampel lanes. A `reisebus` with `look = "postbus"` behaves exactly like any bus; only the renderer differs (`postbus`, `halt` at its stop). Renderer: `entities::draw_ampel` (`ampel` anims by `entities::ampel_anim`), cars by `AUTO_DRIVE`, the approved `ampel`, `auto` and `postbus` art, grey boxes only as fallbacks. Test level `00_m6_traffic` with `m6_traffic_*` replays.
  - Kurgast (M6, E38, D-095 + addendum): `Enemy(Kurgast)` (hash id 19), states `WALK → LIE → WAKE → RESTED → LIE …` (`enemies::update_kurgast`): walking he is the Nordic-Walker at `kurgast_speed` (`walk_turning_at_ledges`); a stomp lays him down (`enemies::lie_down`: the hitbox becomes the `kurgast_chair`, centre and feet kept), he rests `kurgast_rest` and shows the wake telegraph `kurgast_wake`, both counting only frames the player doesn't stand on him (`platform::stands_on` against `UpdateCtx::player_standing`), then stands up and walks on. **Enemy-as-platform hook**: `enemies::is_platform_now(&Entity)` (a Kurgast in his chair) next to `EntityKind::is_platform` in `platform::land`: a one-way top that never moves, so no `carry`, normal despawn. In the chair he is harmless (`Entity::harmful`), `on_foot` and lets axes pass. Points: `enemies::scores_on_defeat` (only before his first stomp): later stomps bounce without points or chain change, a bump or axe after it gives nothing. „Kurruhe!" (`Popup::Kurruhe`) on every lie-down, stacked above the score popup. Poses `Walk`/`Liegen`/`Wake`; the renderer draws the chair as `liegestuhl` (else the `kurgast` sprite's `liegen`/`wake`), grey boxes until the art lands (`m`, an `R` stripe on the chair, white arms while waking). Test level `00_m6_kurgast` with `m6_kurgast_*` replays.
  - Offizier (M6, E39, D-096 + addendum): `Enemy(Offizier)` (hash id 21, `Popup::Abtreten` sub-tag 10). A drill on the **stage clock** (E37's `Game::stage_clock()`, passed as `UpdateCtx::stage_clock`): `enemies::update_offizier` sets `count` = (clock + spawn `phase`) mod `offizier_cycle` and the state `WALK` (march at `offizier_speed` with the Nordic-Walker's `enemies::walk_turning_at_ledges`) / `ATTENTION` / `PRESENT` (halted), poses `March`/`Habtacht`/`Praesentiert`. The sabre zone is no entity: `enemies::sabre_zone` (8×16 above his hitbox top, only in `PRESENT`) is checked in `Game::interact` before his hitbox and the stomp, like the high C. 200 (chain start 200), „Abtreten!" via `enemies::defeat_popup` on every defeat; axe `Defeat`; on foot; sinks in swim water. Renderer: `offizier` / `offizier_lead` (spawn `lead`), grey box with a hilt, a flag for the flag bearer and the sabre drawn from `sabre_zone` (`draw_officer_marks`). Test level `00_m6_offiziere` with `m6_offiziere_*` replays.
  - Kellner and Wartende (M7, E45, D-105 + addendum): `Enemy(Kellner)` (hash id 23, 23–24 reserved; `Popup::KommGleich` sub-tag 12), states `WALK` → `TRAY_BOUNCE` (`count` = frames since the bounce) → `WALK` (`enemies::update_kellner`): the Nordic-Walker's walk (`walk_turning_at_ledges`) at `kellner_speed` (1.0), turning at walls, ledges and enemies on foot (`turn_at_enemies`; he is `on_foot` himself). Not stompable, so the generic contact rule gives the roof bounce off his 12×26 hitbox top (the tray: no damage, no points, the chain neither advances nor resets; JUMP held gives the full stomp bounce) and side hurt; in `Game::interact`'s `Contact::Bounce` branch `enemies::tray_bounce` starts his `bounce` pose for `kellner_bounce_pose` frames (looks only) and returns „Komm' gleich!" centred on him, `POPUP_STACK_PX` above the tray (on every bounce). `knock_off_block` knocks him off too (stompable kinds **or** the Kellner: 100); axe `Bounce`; sinks in swim water. **Wartender**: `SpawnKind::Wartender` (`{ entity = "wartender" }`, no fields) spawns an `Enemy(SelfieTourist)` with the Selfie-Tourist hitbox and `dir = 0` (the D-026 `still` machine): no new `EnemyKind`, no new behaviour, legal in real levels (the `still = true` lint is unchanged). Renderer: `kellner` (`walk`, `bounce` via `Pose::Bounce`, `squished`), grey box `d` with a white apron over the lower half and a 2 px `i` tray line at the top, 2 px lower in `bounce` (`draw_kellner_marks`); `entities::wartender_look` (the spawn's kind is `Wartender`: `wartender_look_at` = (column div 3) mod 3, D-112, so a queue 3 apart cycles the looks) picks `wartender` `idle_N`/`selfie_N`/`squished_N` (`wartender_anim`), never mirrored; without the `wartender` sprite the Selfie-Tourist's grey box. Test level `00_m7_kellner` with `m7_kellner_*` replays.
  - Oberkellner (M7, E46, D-106 + addendum): `Enemy(Oberkellner)` (hash id 25; `EntityKind::Rechnung` tag 30; `Popup::Anstellen`/`Unerhoert`/`BitteSehr` sub-tags 13/14/15), the boss in the level's last 20 columns (`enemies::ARENA_COLS`). `enemies::update_oberkellner`: states `state::oberkellner::{WAIT, INTRO, TELEGRAPH, DASH, BREATHE, VOLLEY, STUN, DONE}`, `count` = frames into the state (1 on its first frame), `Entity::hits` (new field, 0 for every other entity, hashed in `state_hash` and in `world_hash`'s enemy word) picks the round's `enemies.oberkellner_*` numbers. **Lock**: the first update that starts with the camera at its right clamp (`UpdateCtx::screen_left` = level width − 320; he activates a screen ahead) turns `WAIT` into `INTRO` and returns the „Hinten anstellen!" popup; from then on only frames of play advance him (entities don't update while frozen, paused or dying). Rounds: `TELEGRAPH` faces the far wall (more free columns ahead, `free_columns`), `DASH` moves with `TileMap::move_x` and enters `BREATHE` on the frame his side touches the wall (flush with its face), `BREATHE` swaps to `oberkellner_breathe_hitbox` (16×30, feet and centre kept; `prev_y` moves with the top so the stomp rule judges the box she touches), after a missed breather with hits ≥ 1 `VOLLEY`: `Game::update_entities` spawns `enemies::volley` (one `Rechnung` per `BILL_COLUMNS` column of the round, hanging in `BILL_ROW` 8) on its first frame; it ends on the first frame with `UpdateCtx::bills` = 0. **Contact** (`Game::interact`, the officer's state-dependent rule): `Entity::harmful` is false in `WAIT`/`INTRO`/`STUN`/`DONE`; otherwise `enemies::oberkellner_contact`: from above a hit in `BREATHE` (`Contact::Stomp`), a bounce off the tray (`Contact::Bounce`, the Fiaker's feet-on-top) in the tray-up states, any other contact hurts. **Hits** (`Game::hit_oberkellner` ← stomp, or `AxeHit::Hit`, a new axe class, from `Entity::axe_hit` in `BREATHE`; the tray is `Bounce`, the harmless states `Pass`): 1000 + „Unerhört!" → `STUN` (60), the third 5000 + „Bitte sehr!" → `DONE`, every `Rechnung` removed; points outside the stomp chain. `enemies::boss_undefeated(&[Entity])` is the finale's query (E47), `enemies::boss_hits` the replay expectation `boss_hits=N`. **Rechnung** (`enemies::update_rechnung`, `state::rechnung::{HANG, FALL}`): hangs `oberkellner_bill_hang` frames (harmless), then falls at `oberkellner_bill_fall` with x = column x + `sine_offset(count, sway_period, sway)`; gone on a solid or one-way tile (`move_y` Floor), in water or a hazard, on a hurt (`Game::interact`: a falling bill hurts on any contact and is removed unless she blinks), at the defeat; axe `Pass`. Lint `enemies::arena_problems` (`levels lint`): exactly one, in the arena, standing on its floor; 3-high solid refuges at both ends and one flat floor between with only air above it from `ARENA_CLEAR_FROM_ROW` 7 (no solid, one-way, water or hazard tile); no other enemy, spawner, sign, platform (`allowed_in_arena`, exhaustive), box or brick; every bill column with its sway over the floor. Renderer: `oberkellner` anims by pose (`Idle`, `Anstellen`, `Telegraph`, `Dash`, `Verschnaufen`, `Zahlen`, `Stun`, `Verbeugen`), `rechnung` (`Hang`, `Flutter`) in the enemy layer; grey boxes until the art lands: a `d` box with an `i` tray line on top while the tray is up and a 2×10 `i` bar at his front side in the breather (`draw_tray_marks`), bills as `w` boxes. Test level `00_m7_boss` (the arena alone, `camera_top = 92`, the `goal_stollen` on the counter) with `m7_boss_*` replays. Solver: `solver::search_progress` adds the boss's hits × (level width + 1 subpx) in a level with an Oberkellner, so the search fights him (the grey box: complete at 1650 from the counter).
  - M6 integration (E41, D-099): `Game::world_hash` adds the stage clock while an Offizier is in the entity list (an officer not yet active starts his drill at the clock; nodes of one frame differ in it after a power-up freeze); other levels hash as before. Grey box `00_m6_mix` (every M6 kind next to another) with its solver replay; cross-task tests in `jnr-core` `tests/m6_mix.rs`. Renderer: `kalkfels_bg` joins a `felssteg` deck on all four sides (`tiles::one_way_join`), `felssteg` joins only itself.
- **Collision**: AABB vs tile grid (solid, one-way, hazard = kills on touch, water = kills once the feet sank `player.water_sink_px` in, D-050), AABB vs AABB for entities. No slopes in M1.
- **Swimming** (M5, E27, D-076, `swim.rs` + `Player::update`): `TileKind::Swim` (`see_wasser`, `see_wasser_tief`, `steg_pfahl`) never kills and is no floor. Before she moves, `swim::swim_surface` checks her chest point (hitbox centre x, `swim.chest_px` below the top, every form); in swim water `Player::update_swim` replaces the walk/jump/gravity rules: entry clamps vy to [−`stroke_speed`, `max_sink`] and clears buffer and coyote; LEFT/RIGHT `accel`/`decel` towards ±`speed` (RUN no faster); a new JUMP press within `surface_band` px of the surface is a jump-out (`jump_speed_walk`, `Event::Jumped`, `Player::leaving_water` = normal airborne rules until she stops rising), below it a stroke (`Event::Stroke`); at the surface without DOWN and not rising she floats (vy 0), otherwise `swim.gravity` up to `max_sink`; after the move the chest is capped at the water line (no rise above it except by a jump-out). The lake bed is ground. While swimming every enemy contact hurts (`enemies::contact`, fed by `Game::player_swimming()` = `Player::swimming`, E30), RUN throws nothing; items, axes, E-Bikers and snow globes are lost in it (`TileMap::swallows`, Hazard|Water|Swim), walking enemies are removed at its touch (`EnemyKind::sinks_in_swim_water`, exhaustive); an entity may stand on a swim tile (`tile = "see_wasser"`, D-049 extended). Hashed: `swimming`, `leaving_water`, `swim_frames` (the renderer's `swim` anim frame, restarted by a stroke; fallback the `jump` anim; optional `blase` bubbles from the frame counter). Lint `swim::problems` (bed under every swim column; no solid on a swim surface in the columns an auto-scroll shows).
- **One-way platforms** block only from above; the player drops through by pressing DOWN + JUMP while standing on one (DOWN alone does nothing).
- **Camera** lives in core (it affects enemy activation like in SMB and must be deterministic).
  - Climb camera (M3, E14, D-054, `camera.rs`): a level's `[meta] camera = "side|climb"` (default `side`: the dead-zone `Camera::follow`, unchanged). `climb` uses `Camera::follow_climb` with a `ClimbFocus` (hitbox centre x, top, feet, grounded, and `Game`'s remembered `ground_feet`, the feet of the last grounded frame): horizontally the same; grounded it settles towards settle feet − `camera.climb_feet_y` by ≤ `climb_settle_speed` px per frame, where the settle feet (`ClimbFocus::settle_feet`, D-065) follow the ground down at once but up only by what a step exceeds `climb_up_slack` (3 tiles), and catch up with the feet once she walks more than `climb_up_reach` px from where she stepped up (`slack_x`), so a box or plank up to 3 tiles high keeps the ground below on screen; while that slack holds, the in-air up-follow line is `climb_top` minus the lag (floored at `climb_top_min`, no bob), and a landing with the top above `climb_top` catches up `climb_catch_up_speed` px per frame faster; in the air it follows up at once when the top is above screen y `climb_top`, and down while the feet are below `climb_feet_y`, at most `climb_fall_follow` px below the last ground's own framing (`ground_feet`, not the settle feet; the limit never pulls it up). Start, respawn and stage changes frame the feet at `climb_feet_y` at once (`Camera::new_climb`, settle feet = feet); the goal sequence settles the feet at `climb_goal_feet_y` (`Camera::settle_y`). **Death line**: after the camera's follow, a hitbox top below the screen bottom (`player.y > camera.y + 180`, subpixels) dies with `DeathCause::Pit`.
  - Auto-scroll camera (M5, E26, D-075, D-083, `camera.rs`): `[meta] camera = "autoscroll"` with `autoscroll = [from, to]` (columns; `LevelMeta::autoscroll_zone`). `Game` keeps an `Autoscroll` (`ScrollPhase::{LeadIn, Scroll, LeadOut}`, the subpixel position `pos`, the respawn `hold`; read with `Game::autoscroll()`) and calls `Camera::follow_autoscroll`: lead-in = the side camera until following would put x at or past `from × 16`, then x = exactly `from × 16` and the scroll starts (no hold); in the scroll `pos` grows by `camera.autoscroll_speed` (192 subpx) every frame of play (the camera's x is its floor; nothing moves while frozen, paused, dying or in the goal sequence, since those frames never reach the camera), after `autoscroll_hold` frames if the start/respawn framing (`Autoscroll::placed`) lay in the zone; at `to × 16` the lead-out (side camera) takes over for good. Vertically always the side dead zone. While scrolling (`Game::scrolling`, the hold included): the player's own movement and any platform carry stop where her hitbox's right edge reaches camera x + 320 − `autoscroll_right_margin` (`player::Bounds`, `platform::carry`'s `max_x`; stopped there her speed drops to the scroll speed so she keeps pace without a stutter); D-051 (`follow_carried_left`) is off; after the frame's camera follow a hitbox left of the camera's x is moved right with tile collision (`push_from_left_edge`), and if a solid tile stops it she dies with `DeathCause::Squeezed` (drawn as the normal hit death). `state_hash`/`world_hash` hash the scroll state only in auto-scroll levels. Enemy triggers that must not start off screen use `Camera::fully_on_screen` / `UpdateCtx::fully_on_screen` (the hitbox inside the camera rectangle, edges included).
  - The finale and the ending (M7, E47, D-107, `game/flow.rs`, `game/ending.rs`): `SpawnKind::GoalStollen` (`goal_stollen`; `SpawnKind::is_goal` covers both goal kinds) is a goal whose trigger is its own 16×16 cell (`level_parts`), touchable only while armed (`Game::goal_armed`: `!enemies::boss_undefeated(&entities)`, E46's query; the `// E46:` stub returns false until it lands, so a level without a boss is armed from the start). The touch (`touch_stollen`) adds `score.stollen` (10 000) with a score popup and `Popup::Stollen` („Zaunerstollen!", hash sub-tag 17), raises `LevelComplete` and starts `ClearPhase::Serve` (hash 6) instead of Slide/Hop/Walk: no horizontal speed, in the air she keeps her vertical speed and falls with the player's gravity onto tiles (`serve_fall`), and `goal.serve_frames` (90) count from the frame she stands; then the unchanged Tally and Clear. After Clear, `enter_ending` (real flow, a route, the journey's last station `STATIONS[8]`) switches to `Mode::Ending`; otherwise the route map or `ToBeContinued` as before. `Mode::Ending` steps `Game::ending()` (`Ending { phase, timer, page }`, `EndingPhase::{Map, Scene, Bill, Credits, Ende}`) with START as its only input (a new press, ignored for `ending.start_ignore_frames` of a phase or credits page, `bill_start_frames`/`end_start_frames` in Bill and Ende; JUMP tracked); fixed frame counts from `tuning.toml` `[ending]` (validated); after Ende `Game::cleared` (the session's „durchgespielt" mark, hashed, never reset, survives `end_run`) is set and `end_run(true)` shows the title as `Mode::Title { from_ending: true }` (hashed with the mode): this marked title takes START only once its `frames` reach `ending.end_start_frames` (60, the game-over prompt's convention; a START held from Ende stays no press, D-025), while every other title (boot, `end_run(false)` after the teaser or „Fortsetzung folgt …") takes it at once (D-111, E50). `Game::oblaten_total` (hashed) counts every Oblate of the run, never wraps, and resets with the score in `reset_run` (continue, title, end of run). Replays: `state=ending`, `cleared=true|false`. Renderer: `jnr-render::ending` (Scene with `ending_cafe` at 16, 12 or text only; Bill on `ending_zettel` at 72, 18 or a D-037 panel of the same 176×144 box, one line per `ending.bill_line_frames`, numbers right-aligned in `Y`; credits pages from `credits.pageN.{title,line1..4}`, missing keys skipped; Ende), `map::draw_ending_map` (every station done, the whole route walked, Resi on the last dot, the three panel lines), the marked title in `screens::draw_title` (`stollen_teller` at Resi's feet, `title.cleared`, `title.press_start_again`), the bell (`stollen_glocke` `zu`/`offen`/`leer`, `scene::glocke_anim`) and the plate above her head (`scene::serving`: the Serve, and on through its Tally and Clear); grey boxes for every missing sprite. Grey box `00_m7_finale` and the `flow_m7_finale_*` replays put it on station 9 of a route of the real stages.
  - Side camera top clamp (M6, E43, D-103, `camera.rs`, `game.rs`): an optional `[meta] camera_top = <level px>` (side levels only: climb and auto-scroll levels reject it; `0 ≤ camera_top ≤ level_h − 180`; `LevelMeta::camera_top`). `Game::clamp_camera_top` applies `Camera::clamp_top` (y ≥ camera_top, or the bottom clamp if that is higher; also the body of D-090's `clamp_autoscroll_top`) after the dead-zone `Camera::follow` every frame of play, after `Camera::new` in `place_camera` (start, respawn, stage change, the title framing) and after the goal sequence's `follow_y`, so it holds in every phase. Where it wins over the dead zone her top may leave the screen (D-090). It changes only the camera's y: player, entities and events stay identical, except where the camera's y feeds back (axes leaving the screen; nothing camera-triggered in Bad Ischl). The key is hashed into the level's static hash only when present (`level_parts`), so levels without it keep their hashes. Bad Ischl sets 76 (the street's top at screen y ≤ 164).
  - M7 integration (E49, D-109): no engine change in `jnr-core`; the Stage 9 kinds were checked together. Grey box `00_m7_cafe` (88 × 17, `camera_top = 92`, every M7 kind, tile and prop: a Kellner pen whose right end is a still queue, a Kurgast beside a 3-high vitrine, the checkpoint, an `M`-box before a 1-high step, the arena with the Oberkellner and the `goal_stollen`) with its solver replay (the D-106 plan: the solver beats the boss); cross-task tests in `jnr-core` `tests/m7_cafe.rs` (a Kellner turning at a Wartender and at a lying Kurgast, the Mozartkugel turning before the arena, the bell arming on exactly the third hit and never on a respawn, a respawn in a volley rebuilding the same fight, an axe through the stun, the fixed camera through play, pause, deaths, the game over, the goal sequence and every ending phase, the new kinds in swim water); xtask test `fixed_room_cameras_never_move_vertically_in_any_replay` (every replay of a level whose `camera_top` is its bottom clamp keeps that camera y on every frame). Renderer: `marmor_punkt` is in `marmor`'s autotile family (`tiles::autotile_family`), so the floor dots draw no lip line; `Popup::Hoppala`'s key joined `screens::TEXT_KEYS` (every text popup is listed now).
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
- Implemented (M1): parallax backgrounds (`background.rs`): `RenderContext::with_background(&[(sprite, factor16, y, factor_y16)])`, layers back to front, tiled horizontally at `-((camera.x * factor / 16) mod width)`, bottom-aligned to the screen (raised by `y`); vertical parallax (M3, D-054): a layer is drawn `background::sink_px` = `(max camera y − camera.y) × factor_y / 16` px lower (0 at the level bottom, never up; `RenderContext` keeps the level's max camera y); the flat sky `s` stays behind them unless the first layer is opaque, full-height and fixed to the screen bottom (`y` 0, `factor_y` 0; any scroll factor, e.g. M7's `bg_zauner_salon` at 16, E48); missing sprites are skipped. Autotiling (`tiles.rs`, D-020): `TileLook` per tile name resolved once; `auto` (16 frames) picks the frame by `autotile_mask` (computed from the live level while drawing), other tiles animate by the game frame.
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
- Audio: plays the cues of `jnr_audio::cues::Director` (fed after every `Game::step`, see "jnr-audio" → "Cues") through macroquad audio. Audio is optional; a missing sound never fails. Platform layer as built in E54 (D-114 … D-116, D-118), below; the cue wiring and real sounds come with E59.
- Wasm: built with `cargo xtask wasm`, served from `web/` with any static server. All assets are embedded (`include_bytes!`), so no fetching at runtime.

Implemented (M0):
- Native CLI (`cli.rs`, not compiled for wasm; `jnr-app --help`): `--level <id>` (embedded levels only, listed in `options::LEVELS`: `enemies`, `items`, `testbed`, `salzburg` (default); unknown ids exit 2 with the list), `--seed <n>` (default 1), `--scale <n>` (initial window 320n×180n, 1..=16, default 4), `--exit-after <frames>` (quit with exit code 0 after that many simulation frames), `--screenshot-at <frame>:<png>` (repeatable; writes the plain 320×180 framebuffer after that simulation frame, frame 0 = start; a write error exits 1). `--replay`/`--record` follow with the replay format.
- The "what happens after frame N" decision is the pure `Options::actions_after(frame)`; it runs after every tick (not every render frame), so screenshots and exits are exact even when several ticks run per render frame.
- F12 (native debug builds) saves the framebuffer as shown (incl. F1 overlay) to `$CARGO_TARGET_DIR/preview/f12_<frame>.png` (else `./f12_<frame>.png`) and prints the path. F2/F3 are not implemented yet.
- PNG encoding uses the `png` crate as a native-only dependency of `jnr-app`.
- Wasm uses `Options::default()` (no command line). `web/index.html` loads `web/gl.js` (unmodified copy of miniquad's loader, version in `web/README.md`) and `jnr-app.wasm`; it sets `<body data-wasm="loaded">` once the module is instantiated. `cargo xtask wasm [--debug] [--serve] [--port N]` builds (release by default) and assembles `$CARGO_TARGET_DIR/web/`; `cargo xtask wasm-smoke` (a `verify` step) loads it in headless Chromium (SwiftShader WebGL) and checks the marker and the screenshot `$CARGO_TARGET_DIR/preview/wasm_smoke.png` (not blank, ≥ 25 % sky colour `s`).

Implemented (MA, E54): audio platform (`src/audio.rs`, the only code that touches `macroquad::audio`):
- `jnr-app` enables macroquad's `audio` feature in its own `Cargo.toml` (quad-snd 0.2.8; the workspace entry stays `default-features = false`).
- **Panic guard** (native, D-115): `audio::install_panic_guard()` runs in `main()` before `Window::from_config` (macroquad creates the audio context, and quad-snd its thread, while it sets up the window). The pure `classify_panic(file, thread)` treats a panic located in `quad-snd` on a thread other than `main` as the audio thread dying (no device: `Can't open PCM device.`): it sets the static `AUDIO_DEAD`, prints one line `jnr-app: audio unavailable (<msg>), playing silently` and swallows the panic. Everything else, including a quad-snd panic on the main thread (an undecodable WAV), goes to the previous hook. libasound's own `ALSA lib …` stderr lines remain (cosmetic).
- **Backend gate**: every macroquad audio call goes through `Audio::may_call()`, which refuses while the thread is dead (an injectable probe, `audio_dead` in the app, a stub in tests), after a load timeout or while muted, and counts the calls it allows (`calls()`: load starts, frames with load polls, plays, stops, volume changes and unloads of a `Sound`). Unit tests: a dead probe gives 0 calls and `off() == Some(NoDevice)`; `--mute` through a whole replay's cues gives 0 calls. `Audio`'s `Drop` leaks loaded `Sound`s when the thread is dead (their drop would send a delete). Result: no `Audio thread died` lines (quad-snd prints one per call otherwise); `vnc-smoke` asserts it.
- **Frame-budgeted loader**: `Load<T>` pins a load future and polls it once per render frame with `Waker::noop()`; after `LOAD_BUDGET_FRAMES` (120) without a result it reports `TimedOut`. While nothing has loaded yet that is the wasm-without-`audio.js` case (`load_sound_from_bytes` never returns): `Audio` logs `audio load timed out after 120 frames; audio off` once and stays off, the game runs on. Once any sound has loaded, a timeout only gives up that one sound (E59 review). Natively a load finishes on its first poll. Loads start on the first frame: a send to quad-snd before its thread dies succeeds silently, and the guard sets the flag before the dying thread drops its receiver, so no later call reaches a dead channel. While muted, nothing is started or polled. `Audio::update()` runs once per render frame after the frame's simulation steps (so a cue's sound loads and starts on its own frame natively), never blocks.
- **Mute**: `--mute` (native CLI) starts muted: nothing is loaded or played. The **M** key (`MUTE_KEY`, both platforms, not a game key; a test checks it is not in `KEY_MAP`) toggles mute, stops everything playing when muting, and logs `sound off`/`sound on`. No player-visible text.
- **Cues (E59)**: `main` feeds `jnr_audio::cues::Director` (`begin` once, `step` after every `Game::step`, also with `--replay`) and hands the cues to `Audio::cues`. The pure `audio::Mixer` turns each cue into `Op::{Play, Stop, Volume}` on a `SoundId::{Sfx(Play), Music(Music)}`: an SFX stops what its voice started last, then plays (one live copy per id); `Song` stops the old song and twin, starts the new one looped (the `unterwasser` twin muted on the same frame, when the bank has it); `Jingle` replaces the jingle; `HoldSong`/`HoldJingle` set volume 0 and back, `Duck` 0.5 (−6 dB), `Underwater` swaps the song's and twin's volumes. Every sound is rendered at its design level, so plays use volume 1.0. `Audio` renders all SFX and `cues::PITCHED` variants on its first active frame (`jnr-app: audio: 27 sfx rendered in 2.5 ms`, debug build) and loads them through the budgeted loader; a song is rendered and loaded when its cue first comes up; the song and its twin start together, in the first update in which both are loaded (`Mixer::start_ops`, D-114: they must stay in step; natively the cue's own frame); a new song unloads the old ones (jingles stay), a song replaced before or while loading is dropped, and songs unloaded while muted are parked and dropped at the next active frame (only a dead or off backend leaks them). Each music cue also has its own gain (`Music::gain_pct`: `boss_intro` 25 %, −12 dB), applied by the app and `jnr_audio::mix` alike. An SFX cue whose sound hasn't loaded is dropped. M: muting stops everything, unmuting restarts the current song from its start. The E54 test tone is gone (`vnc-smoke` checks real SFX onsets).
- **wasm**: `crates/jnr-app/build.rs` emits, only for `wasm32`, `cargo:rustc-link-arg-bins=--allow-undefined-file=<manifest>/wasm-audio-imports.txt` with exactly the 9 `audio_*` imports of quad-snd's `web_snd.rs` (D-116; quad-snd lacks `wasm_import_module`, and rustc no longer allows undefined symbols on wasm32). `web/audio.js` is an unmodified copy of quad-snd 0.2.8's `js/audio.js` (sha256 and licence in `web/README.md`, `audio.js.LICENSE-MIT` next to it); `index.html` loads it after `gl.js`, and `xtask wasm` copies both into the web dir. xtask tests pin it: the quad-snd version in `Cargo.lock` equals `QUAD_SND_VERSION`, the imports list equals the `extern "C"` block of the registry's `web_snd.rs`, `audio.js` matches the README's sha256, and `index.html` loads it between `gl.js` and the module. The browser `AudioContext` stays suspended until the first key press (autoplay policy; `audio.js` resumes it).

## jnr-artkit + jnr-assets: asset pipeline

- `jnr-artkit` is a library: parses `.palette` and `.sprite`, runs procedural generators, packs atlases, runs art lints. Details in `art-pipeline.md`.
- `jnr-assets/build.rs` calls `jnr-artkit` to compile all art into one packed RGBA atlas: raw pixels (`include_bytes!`, borrowed without copying) plus generated Rust code for the metadata. At runtime `jnr_assets::atlas()` builds the `jnr_artkit::atlas::Atlas` once via `OnceLock` (D-010), so `jnr-assets` depends on `jnr-artkit` at runtime for these data types. A broken sprite = a **build error** with file:line:col.
- Levels and text are embedded as strings and parsed by `jnr-core` (`Level::parse(&str)`), so tests and tools can load them directly.
- Audio (E58b): `build.rs` also reads the flat source directories `audio/{sfx,jingles}/*.sfx`, `audio/instruments/*.inst`, `audio/music/*.song`, `audio/quotes/*.quote` (a source file anywhere else is a build error; `README.md`, `targets.toml` are not sources), checks them (incl. each music cue's kind: loops as `kind = "loop"` songs, one-shots as jingle songs or `jingles/*.sfx`) with `jnr_audio::bank::Bank::load` (parse errors, duplicate ids, music-lint errors: a **build error** with `file:line:col`) and embeds them as `AUDIO_SOURCES` (`include_str!`, sorted by path). `jnr_assets::audio_sources()` returns them, `jnr_assets::audio_bank()` the parsed `Bank` (once, `OnceLock`, ~0.2 ms release). `jnr-assets` therefore depends on `jnr-audio` (normal and build); `jnr-render` reaches it only as a dev-dependency.
- Generated files are **not committed**; sources are the single truth. Previews go to `target/preview/`.

## jnr-audio: integer synth (MA, D-114, D-117)

Pure crate (E55): text patches → 44 100 Hz mono `i16` PCM, bit-identical on native and wasm (integer maths only, checked by the arch-lint's determinism scan; a test pins the FNV-1a hash of a patch that uses every feature, and the same hash was measured from a wasm32 build in Node). Debug builds compile it with `opt-level = 2`.

- `patch`: `Sfx::parse(file, text)` and `Instrument::parse(file, text)`; the id is the file stem (`[a-z][a-z0-9_]*`). Errors are `file:line:col: error[<kind>]: message` with kinds `syntax`, `schema` (unknown/missing key, wrong type, any float), `key` (a key not allowed in this file kind or on this wave), `range`, `wave`, `layers`, `note`, `kit`, `id`.
- `synth`: one layer = one voice. A `u32` phase accumulator whose increment is recomputed every 32 samples from the pitch in cents (slide + vibrato + arpeggio + detune) via `tables::scale_by_cents` (Q30 semitone and cent tables). Oscillators in Q15: `pulse` (zero-mean and peak-normalised for any duty), `triangle`, `saw`, `sine` (256-entry table, linear interpolation), `noise` (15-bit LFSR, taps 0/1, seed 1, stepping twice per period of `freq_hz`). Envelopes are linear Q16 segments whose boundaries are `ms_to_samples` of the cumulative times (exact to the sample). Filters: one-pole low-pass and high-pass (`a = w/(1+w)`, `w = 2π·fc/fs`), and a band-pass (Chamberlin state-variable filter, 2× oversampled, unity gain at the centre, centre glides in cents over the sound). Tremolo scales the gain.
- `render`: voices are summed into an `i32` buffer (`add_sfx`, `add_note`), then `to_pcm` clamps to ±32767 and **counts** clamped samples (`Render { pcm, clamped }`). `Pcm::fnv64()` is the golden fingerprint. `FRAMES_PER_TICK = 735`.
- `wav`: `write(&Pcm)` (44-byte RIFF header, PCM 16-bit mono) and `read` (skips unknown chunks; for tests and tools).
- `song` (E57): `Song::parse(file, text)`, the `.song` format below (error kinds also `score`, `pattern`, `channel`, `music`); `song.events` is the order laid out in time (per channel, ties merged). `song_render`: `render_song(&Song, &instruments, &RenderOptions) -> SongRender`. `music_lint`: `lint_song(&Song, &LintContext) -> Vec<Lint>` (`file:line:col: error[music-<code>]: …`). `quote`: `.quote` melody fixtures and `check_quote`.

### `.sfx` (`audio/sfx/*.sfx`, `audio/jingles/*.sfx`)

TOML, integers only, units in the key names. 1–8 `[[layer]]`s, summed:

| Key | Values | Default | Notes |
|---|---|---|---|
| `wave` | `pulse` `triangle` `saw` `sine` `noise` | required | |
| `duty_pct` | 1..=99 | 50 | pulse only |
| `freq_hz` | 20..=20000 | required | noise: the LFSR steps at 2 × `freq_hz` (20000 ≈ white) |
| `slide_to_hz` | 20..=20000 | none | glide in cents (exponential in Hz) |
| `slide_ms` | 0..=10000 | the rest of the sound | only with `slide_to_hz` |
| `slide_delay_ms` | 0..=10000 | 0 | the glide starts this long after the layer's start (e.g. a 1-up's last step); only with a slide |
| `attack_ms`, `hold_ms`, `decay_ms` | 0..=10000 each | 0 | linear 0 → peak → peak → 0; at least one > 0 |
| `volume_pct` | 0..=100 | required | of full scale; layers add up, clamps are counted |
| `vibrato` | `{ depth_cents = 0..=1200, rate_hz = 0..=50, delay_ms = 0..=10000, to_depth_cents = 0..=1200, ramp_ms = 0..=10000 }` | off; `delay_ms` 0, `to_depth_cents` = `depth_cents`, `ramp_ms` 0 | off before `delay_ms` (the LFO starts at phase 0 there), then the depth goes linearly from `depth_cents` to `to_depth_cents` over `ramp_ms` (with `ramp_ms = 0`: `to_depth_cents` at once). A delayed vibrato: `{ depth_cents = 20, rate_hz = 6, delay_ms = 200 }`; growing: `depth_cents = 0, to_depth_cents = 50, ramp_ms = 500`; fading: `depth_cents = 100, to_depth_cents = 0, ramp_ms = 200`. `rate_hz` ≥ 1 when either depth > 0 |
| `arp` | `{ every_ms = 1..=1000, semitones = [-24..=24; ≤ 8] }` | off | cycles through the list |
| `tremolo` | `{ depth_pct = 0..=100, rate_hz = 1..=50 }` | off | gain × (1 − depth·(1+sin)/2) |
| `lowpass_hz`, `highpass_hz` | 20..=20000 | off | one-pole; high-pass < low-pass |
| `bandpass` | `{ hz = 20..=10000, to_hz = 20..=10000, q_pct = 100..=2000 }` | off | `to_hz` defaults to `hz`; Q = `q_pct`/100. Unity gain holds for a tone at the centre; it is no peak bound for broadband noise, whose peaks can exceed the input's, so check the clamp count. A **low-pass sweep** is approximated with a Q 1 band-pass sweep (`q_pct = 100`); `lowpass_hz` itself is fixed |
| `delay_ms` | 0..=10000 | 0 | the layer starts later |
| `repeat` | `{ count = 1..=64, every_ms = 1..=10000 }` | off | the whole layer again, `every_ms` apart |

The sound's length is the longest layer's `delay + attack + hold + decay` (+ repeats).

Cutting early (E57's notes, SFX retriggers): `render::add_sfx_until` and `add_note_until` take an absolute sample `until`; the voice is unchanged before it, fades out linearly over `CUT_FADE` = 88 samples (2 ms) from it, and adds nothing after; a voice starting at or after `until` adds nothing.

The crate's test fixtures (example `.sfx`/`.inst` files) are in `crates/jnr-audio/tests/fixtures/`; `audio/` holds only game sounds.

```toml
# audio/sfx/jump.sfx
[[layer]]
wave = "pulse"
duty_pct = 25
freq_hz = 294
slide_to_hz = 880
slide_ms = 120
attack_ms = 2
hold_ms = 8
decay_ms = 140
volume_pct = 35
```

### `.inst` (`audio/instruments/*.inst`)

**Melodic**: `range = ["e1", "f4"]` (note names `c4`, `f#3`, `bb2`, octaves 0..=8; `a4` = 440 Hz, equal temperament) and 1–8 `[[layer]]`s with the `.sfx` keys except `freq_hz`, `slide_to_hz`, `hold_ms` and `repeat`, plus `detune_cents` (−2400..=2400), `slide_cents` (−4800..=4800, needs `slide_ms`), and an ADSR: `attack_ms`, `decay_ms` (to sustain), `sustain_pct` (default 100), `release_ms`. A note's `gate` (samples) is counted from its onset, a layer's `delay_ms` included, so every layer of a note sounds `gate + release` samples from the onset (a layer whose delay is ≥ the gate stays silent). A release before the end of attack or decay starts from the level reached; a release from level 0 (e.g. a gate of 0) has no samples.

**Kit**: a `[kit]` table mapping single letters (A–Z, a–z) to patch names, and one `[[patch.<name>.layer]]` list per patch with `.sfx` keys. No `range` or top-level `[[layer]]`; every patch must be used.

```toml
# audio/instruments/kit.inst
[kit]
K = "kick"

[[patch.kick.layer]]
wave = "sine"
freq_hz = 120
slide_to_hz = 50
slide_ms = 60
attack_ms = 1
hold_ms = 20
decay_ms = 120
volume_pct = 70
```

### `.song` (`audio/music/*.song`, E57)

A song is a **loop** (it loops from sample 0, D-114) or a **jingle** (once: a stage intro, a fanfare, `game_over`). TOML header plus tracker-style scores in multi-line literal strings, bar-structured so that every channel's bars line up under each other, like a system in a score:

```toml
# audio/music/example.song
[meta]
id = "example"            # = the file stem
kind = "loop"             # loop | jingle
tempo_bpm = 120           # beats per minute; a beat is the meter's lower number
meter = "2/4"             # 2/4 | 3/4 | 6/8 (6/8 counts eighths: dotted quarter = 66 is tempo_bpm = 198)
steps_per_beat = 4        # the grid: a 2/4 bar has 2 x 4 = 8 steps
key = "bb major"          # tonic (c, c#, db, ... bb, b) + major | minor
bars = 24                 # the bars of `order` (checked)
order = ["A", "B", "A"]   # patterns in playing order (repeat one by naming it again)
source = "original"       # or the quoted work; then `edition` (the PD edition) is required
# edition = "AMA, Breitkopf 1886"
# length_of = "wolfgangsee"   # render to that song's exact length (the underwater track)

[channels]                # 1..=6, top to bottom in the piano roll
lead   = { instrument = "fluegelhorn", volume_pct = 40 }
bass   = { instrument = "helikon", volume_pct = 30 }
chords = { instrument = "begleitung", volume_pct = 20, arp_ms = 17 }  # default 17; 0 = together
drums  = { instrument = "kit", volume_pct = 25 }

[pattern.A]
score = '''
lead   | r/2 bb4 d5 f5 | eb5/4> d5 | c5/2 d5 eb5 c5 | bb4/4 _       |  # the Oblaten-Motiv
bass   | bb2/4 f2      | bb2/4 f2  | f2/4 c3        | bb2/4 f2      |
chords | r/2 [d4,f4,bb4]' r [d4,f4,bb4]' | r/2 [d4,f4,bb4]' r [d4,f4,bb4]' | r/2 [c4,f4,a4]' r [c4,f4,a4]' | r/2 [d4,f4,bb4]' r/4 |
drums  | K/2 h K h     | K/2 h K h | K/2 h K h      | [K,S]/2 S S S |

lead   | c5/4 a4       | ...                                       # the next bars: a new "system"
'''

[pattern.B]
key = "f major"           # optional: this section's key (a Trio, a B section)
score = '''
...
'''

[[quote]]                 # optional, one per quoted tune (see `.quote`)
fixture = "oblaten_motiv"
channel = "lead"
bar = 1                   # 1-based song bar
step = 2                  # optional: steps into that bar (a pickup)
# transpose = 0           # semitones from the fixture to the song
# augment = 1             # the song's lengths are the fixture's times this
# compare = "exact"       # exact (pitch and length) | pitches (a tune squeezed into another meter)
```

**Score lines**: `<channel> | bar | bar | …`, tokens separated by spaces. Several lines of the same channel in one score continue each other (a block of a few bars for every channel is a "system"); a channel without a line is silent in that pattern; all its present lines have the same number of bars. `#` at a line start or after a space starts a comment (`f#4` is a note).

**Token**: `<sound>[/<steps>][>]['][@<velocity>]`.

| Sound | Meaning |
|---|---|
| `c5`, `f#4`, `bb3` | a note (octaves 0..=8); `c#5!` marks a note outside the key as intended |
| `[d4,f4,bb4]` | a chord of ≤ 3 notes, no spaces: one voice arpeggiated every `arp_ms` (the chip chord), the first note is the root; `arp_ms = 0` sounds the notes together |
| `K`, `[K,S]` | kit letters (any letter of the kit except `r`), several struck together |
| `r` | a rest (releases ring into it) |
| `_` | a tie: the previous note or hit lasts longer, also across bar lines and patterns |

`/<steps>`: the length (default: the previous token's of that channel in the pattern, across its lines; its first token in a pattern: one beat). Every bar must sum to meter × `steps_per_beat` (else a `score` error at the bar). `>` accent (velocity 125 %), `'` staccato (sounds half the length), `@<pct>` velocity 1..=200; the velocity scales the channel's `volume_pct`.

**Rendering a song** (`song_render`):
- Time: a song of `S` steps is `L` samples; step `k` starts at `k·L/S` (rounded down, spread evenly). A jingle's `L` is its natural length; a loop's is rounded to the nearest multiple of **4096 frames** (D-114): at most 2048 frames (46 ms) per loop. `RenderOptions { tempo_pct, length_frames }`: `tempo_pct` (50..=200; 112 is the "hurry" render, D-114); `length_frames` forces an exact length (a loop: a multiple of 4096). `song_render::render_in(&songs, &instruments, id, &opts)` resolves `length_of` (`resolve_length`: the target loop's length at the same tempo) and is what the game and xtask call.
- **Channels are monophonic**: a new note, chord or hit cuts what still sounds on its channel (the `CUT_FADE` of `add_note_until`, applied to the summed voice: identical for one layer, a few LSBs of rounding apart for several); a rest doesn't cut.
- **Loops wrap**: tails past the loop end are added at the start, and the last notes are cut by the channel's first onset one loop later, so the loop equals the steady state of playing it forever (tested sample-exact against three passes in a row).
- **Cache**: identical voices (channel, notes or hits, gate, velocity) are synthesised once and added wherever they recur (the 90 s, 6-channel test song renders in ~15 ms in release).
- A jingle's PCM runs on to the end of its last release.

**Music lints** (`music_lint`; part of `cargo xtask audio lint`; a bar that doesn't fill the meter is already a `score` parse error, so patterns and the loop end sit on bar lines): errors `instrument` (exists; kit vs. melodic tokens; kit letters), `bars`, `range` (the instrument's `range`), `key` (outside the pattern's key without `!`; minor keys include the raised 6th and 7th), `length` (loops 20–90 s, jingles ≤ 10 s), `length-of` (the target exists, is a loop, ≤ 3 % apart), `quote`, `provenance` (quoting a PD fixture needs a non-"original" `source`); warnings `accidental` (`!` on an in-key note), `clash` (two melodic channels a semitone, 7th or 9th apart where one starts a note on a beat), `consonance` (beat 1, highest vs. lowest melodic channel: unison, 3rd, 5th, 6th or octave in ≥ 70 % of bars), `melody-range` (> 2 octaves), `repeat` (a pattern > 4 times in a row), `unused-pattern`.

**Piano roll** (`cargo xtask audio render <id>|<file.song>` → `$CARGO_TARGET_DIR/preview/audio/<id>_roll.png`, next to the WAV, stats and spectrogram): 8 bars per row, one lane per channel, pitch upwards with a line and a label at every C, bar numbers and pattern names on top, bright bar lines and dim beat lines. Notes in the channel's colour (brighter = louder), a white onset column, drawn as long as they sound; `!` notes have a magenta top edge, notes the lints reject are red; kit lanes have one row per patch; a loop ends in a green line.

### `.quote` (`audio/quotes/*.quote`, E57)

A melody fixture: the first notes of a quoted tune, transcribed from its public-domain edition (D-113, audio-design §9.3). A song's `[[quote]]` must play it (the `quote` lint), so a pattern edit that breaks a quote fails. Single notes, `r` and `_` on the fixture's own grid; `|` is only for reading:

```toml
# audio/quotes/haydn_94_andante.quote
source = "Joseph Haydn, Symphony No. 94, II. Andante, bars 1-2 of the theme"
edition = "Breitkopf & Haertel, ed. J. Rietz, c. 1855 (IMSLP)"
steps_per_beat = 2          # eighths in 2/4
notes = '''
c4/1 c4 e4 e4 | g4 g4 e4/2
'''
```

The check takes the channel's notes (the top note of a chord) from the quote's bar and step on, merges consecutive rests, and compares pitches (plus `transpose`) and lengths in beats (times `augment`; `compare = "pitches"` skips lengths). The last fixture note may be held longer in the song. The first difference is reported with the song's `file:line:col`. The leitmotif is a fixture too (`source = "original"`).

Test fixtures for both formats are in `crates/jnr-audio/tests/fixtures/music/` (placeholder instruments, `placeholder_polka.song`, `placeholder_fanfare.song`, two `.quote`s).
### Tooling (`cargo xtask audio`, E56)

`xtask/src/audio/` renders, measures and lints sounds; it is the only place with f64 sound analysis (`stats.rs`: levels, DC, head/tail, an FFT for the centroid and the spectrogram). Sources are enumerated per `audio::Kind` (`Sfx` = `audio/sfx/*.sfx`, `Jingle` = `audio/jingles/*.sfx`, `Song` = `audio/music/*.song`, rendered and drawn by `xtask/src/audio_song.rs`); a new kind adds a variant with its directory, its render arm in `audio::render`, its target in `audio::target_for` and any extra review image in `audio::write_review` (the piano roll), and `render`, `lint` and `golden` handle it. Lint targets for `.sfx` sounds live in `audio/targets.toml` (class, optional peak, duration, centroid band, `placeholder`), outside the `.sfx` format: they are review data, not synth input. Commands, lints and the golden flow: `docs/validation.md`, "Audio".


### Bank (`jnr_audio::bank`, E58b, D-114)

```rust
let bank = jnr_assets::audio_bank();                       // every source, checked at build time
let sfx: Vec<(Play, Pcm)> = bank.startup_sfx();            // Sfx::ALL + cues::PITCHED, rendered (~1.5 ms release)
let song: Option<MusicRender> = bank.render_music(Music::Salzburg, Variant::Normal)?;  // on demand; None = not authored yet
// MusicRender { pcm, looped, clamped }; Variant::Hurry = a loop at 112 % (the ZEIT-60 render)
let wav = jnr_audio::wav::write(&song.pcm);                // bytes for load_sound_from_bytes
```

- `Bank { sfx, jingles, instruments, songs, quotes }` by id; `has_music(Music)` is a `.song` or a `jingles/*.sfx`. A song renders through `song_render::render_in`, so an underwater twin (`length_of`) gets its stage piece's length. `bank::transpose(&Sfx, semitones)` moves every fixed oscillator pitch (filters and timing stay) for the pitched variants.
- `jnr_audio::cues::PENDING_MUSIC` lists the music cues without a source yet (all 25 at E58b, empty since MA; a test keeps it empty); tests in `jnr-assets` fail for a missing cue that isn't pending and for a pending one that exists, and `levels lint` accepts a level `music` only if it is embedded or pending.

### Cues (`jnr_audio::cues`, E58a, D-113/D-114)

Pure mapping from the simulation to sound commands; the app only executes them. As built in detail: `docs/proposals/ma-audio-architecture.md` → "As built (E58a)".

```rust
let mut d = Director::new();
let cues: Vec<Cue> = d.begin(&game);              // the first frame (title music, or the stage piece)
game.step(input); let cues = d.step(&game, input); // after every step; reads game.events()
enum Cue { Sfx(Play), StopSfx(Sfx), CutSfx(Cut), Song(Music), StopSong, Jingle(Music), StopJingle,
           HoldSong(bool), HoldJingle(bool), Duck(bool), Underwater(bool) }
struct Play { sfx: Sfx, semitones: i8 }            // pitched variants: cues::PITCHED (pre-rendered)
```

- `Sfx` (the 25 must SFX: id, voice P/W/I/A, priority 1–9, design length), `Music` (25 songs and jingles of audio-design §5; `level_song`, `underwater_twin`), `sfx_for(Event, Power) -> EventSound` (exhaustive `match`: a new `Event` doesn't compile until its sound is decided).
- `Voices` holds the §6.1 rules (per step one sound per voice, highest priority first; a busy voice is replaced only by an equal or higher priority; cuts; priority ≥ 7 ducks). The director derives the rest from a small snapshot of the previous step (mode/phase, `time_left`, the tally bonus, the ending timer, the Oberkellner's state, the singing Operettensänger by spawn index), the camera (off-screen entities are silent) and the swim API (`swim::swim_surface`, the water line for the underwater swap).
- A level names its piece with `[meta] music = "<id>"` (`levels lint`: required in journey levels, a `Music::level_song` id); `music` is not hashed.
- Cue logs: `cargo xtask replay <file> --cue-log`, goldens `tests/golden/cues/*.cues` (`cargo xtask cues`, validation.md L6b).

### Offline mix (`jnr_audio::mix`, E60, D-122)

`mix::mix(&Sounds, &[(frame, Cue)], end_frame) -> Mix` plays a cue timeline into one `i32` buffer by D-114's playback rules (one live copy per SFX id, the voice's last sound stopped, hard stops, songs looping from sample 0, jingles once, hold/duck/underwater volumes; `cues::{SFX_VOLUME_PCT, MUSIC_VOLUME_PCT, DUCKED_MUSIC_VOLUME_PCT}`, shared with the app) and clamps it (`Render`, clamp count), with the `Span` of every sound heard and the sounds missing from the table (silent). `Sounds` is filled from the app's own `bank::Bank` (`render_sfx(Play)`, pitched variants included through `bank::transpose`; `render_music(m, Variant::Normal)`), so there is one parsing and one variant path. Used by `cargo xtask replay <file> --audio` (validation.md L6c).

## Content formats

### Level (`content/levels/*.level`)

TOML header + one ASCII grid. One char = one 16×16 tile or an entity spawn.

Format v2 (M1, D-022). Tile names: grey-box `solid`/`oneway`/`hazard`/`empty` and the catalog tiles (`ground`, `meadow`, `rock`, `rampart`, `hedge`, `stone`, `bridge`, `railing`, `arch`, `brick`, `oblaten_box`, `oblaten_box_empty` → solid; `railing_bg` → decorative air; `oneway_sign`, `oneway_burger`, `oneway_wood` → one-way; `salzach_water` → hazard; M3: `kalkfels` → solid, `kalkfels_bg`, `netz_post` → decorative air, `steig`, `steig_wall` (the plank drawn over the back wall), `netz` → one-way; M4: `standl`, `strasse` → solid, `standl_dach`, `wartehaus_dach` → one-way, `tunnel_bg`, `tunnel_bg_lamp`, `zahnrad_gleis_0..3` → decorative air). `contains = "oblate|mozartkugel|zaunerstollen|oblaten_10|hackl"` is allowed on `brick` and `oblaten_box` only (a box defaults to `oblate`). Entities: `player_start` (exactly one), `goal`/`goal_maibaum`, `checkpoint`, `oblate`, `sign` (`text` key required, `style = "street|bus|ortsende"`, default street), `selfie_tourist` (optional `still = true`: never walks, for grey-box `00_*` test levels only, D-026), `taube`, `taube_sitting` (optional `dy`, −15..=15 px, default 0, negative = up: its feet sit that far from the cell bottom, e.g. on a perch 40 px above the ground; E17), `mozartkugel_verkaeufer`, `fiaker`, `festungsbahn` (`to = [dx, dy]` required: its other end point in tiles relative to its spawn cell, each within ±64, not both 0; the spawn cell is where it starts, e.g. the bottom end), `prop` (`sprite` required, one of `jnr_core::PROP_SPRITES` (checked by `levels lint`); optional `layer = "front|back"` (default front: after the tiles, behind the player; back: after the parallax, before the tiles) and `dx` (px, ±96: one 96 px prop width, e.g. a prop centred over a solid block with no free cell under its centre, M6); decorative, no collision, not a core entity — the renderer draws it from the spawn with its origin on the cell's bottom centre + `dx`, grey-box: the cell's outline). M2/M3 enemies: `nordic_walker`, `blasmusiker`, `e_biker`, `bienenschwarm` (`dir`, `len`, `phase`, `dy`), `gondel` (`to`, `phase`) (D-048); `pilger` (optional `lead = true`, the pennant look), `gams`, `steinschlag` (`period` 90..=300, default 150; `phase` 0 or 41..period, default 0; `range` 4..=10 tiles, default 6) (D-061). `durchkriechstein` (`len` 2..=6 tiles, default 4; placed in the tunnel's first cell; geometry: `crawl::geometry_problems`) (D-060). M4 (D-068, D-069): `operettensaenger` (no fields), `souvenirstand` (`period` 90..=300, default 150; `phase` 0 or 31..period, default 0; `range` 4..=10 tiles, default 8; placed in the air cell left of its counter, the stand's body is tiles). M4: `schafbergbahn` (`to` required, 4 ≤ |dx| ≤ 32, dy within ±64) (D-067). Tiles `facade`, `arch_under`, `invisible` (never drawn) → solid; `arch`/`arch_under` are one autotile family (`autotile_family` in `jnr-render/src/tiles.rs`; M7: `marmor_punkt` joins `marmor`); `one_way_join` there lets a tile count a different neighbour as the same on some sides only (`kalkfels_bg` around `steig_wall`; `kalkfels` under `forest_path`/`meadow`/`stone`/`ground`, top only). Unknown names or fields are parse errors. M4: `reisebus` (optional `stop` 1..=16 tiles: the photo stop; placed in the leftmost of its 4 cells on the street row; lane lint, D-070). M5 (D-077): `traundl` (`to = [dx, 0]` required, 2 ≤ |dx| ≤ 8, dy 0; optional `phase`), `dampfer` (`to = [dx, 0]` required, 8 ≤ dx ≤ 64, dy 0), both placed in the deck-row cell where the hull starts; tiles `steg` → one-way, `kai`, `seegrund`, `poller` → solid; props `adventlaterne`, `dalben`, `schilf`, `startboje`. M5 (D-078 … D-080): `schwan` (no fields; placed in a surface-row swim-water cell, `tile = "see_wasser"`: any entity may sit on a swim tile, none on lethal water, D-076/E30), `gelse` (no fields; hitbox centred in its cell), `regattaboot` (no fields; placed in the left of its 2 cells in the surface row; lane lint as for the Reisebus). M6 (E40): tiles `felssteg` → one-way, `kies` → solid, `einfahrt_bg` → decorative air; props `buste`, `pension`, `gasthof_strobl`, `lokalbahn_haltestelle`, `kaiser_jagdstandbild`, `katrin_talstation`, `kaiservilla`, `seeauerhaus`, `lehar_villa`, `trinkhalle`, `kongresshaus`, `trafik`, `zauner_fassade`, `warteschlange` (the catalog's "Props (group `props`, M6)" table). M6 (D-093, D-097): `ampel` (`period` 300..=600, default 400, at least 30 + cars × 40 + 64 + 30 + 120; `cars` 1..=4, default 3; `phase` 0 or 30 + cars × 40..period, default 0; `range` 6..=16 tiles, default 12; placed in the road-level air cell of the side street's mouth; its cars' lane is linted like a bus's), `reisebus` takes `look = "reisebus|postbus"` (renderer only). M6 (D-096): `offizier` (optional `lead = true`, the flag bearer's look; optional `phase` 0..=239, default 0: frames into the drill on the stage clock). M7 (E48): tiles `marmor`, `marmor_punkt`, `stuck`, `vitrine`, `theke` → solid, `luster` → one-way (grey boxes by kind until the art lands); props `zauner_tuer`, `garderobe`, `kuchentheke`, `luster_deko` (the catalog's "Props (group `props`, M7)" table; the goal's `stollen_glocke` is drawn by the renderer from the `goal_stollen`, not a prop). M7 (D-105): `kellner` (no fields), `wartender` (no fields: a Selfie-Tourist that never walks, legal in stages; drawn facing right in its look by (column div 3) mod 3, D-112).

```toml
[meta]
id = "salzburg"
title_key = "stage.salzburg.title"   # "Etappe 1: Salzburg"
time_limit = 300                     # required, 1..=999 seconds
min_jump_window = 6                  # optional, frames (checked by the solver)
camera = "side"                      # optional: "side" (default, dead zone), "climb" (D-054) or "autoscroll" (D-075)
# autoscroll = [16, 208]             # columns [from, to]: required with camera = "autoscroll", an error otherwise; 0 <= from < to <= width - 20
# camera_top = 76                    # optional, level px (D-103): the side camera never above it; only with camera = "side"; 0 <= camera_top <= level_h - 180
background = [                       # far to near; factor in 1/16 of the camera speed (0..=32)
  { sprite = "bg_sky_salzburg", factor = 0 },
  { sprite = "bg_untersberg", factor = 2, y = 24 },   # y: px above the screen bottom (default 0)
  { sprite = "bg_festung", factor = 4 },
  { sprite = "bg_rooftops", factor = 8 },
  # optional factor_y (0..=32, sixteenths, default 0): sinks the layer as the camera climbs (D-054)
]
music = "salzburg"                   # [meta]: the stage piece (a jingle-free song id of audio-design §5; required in journey levels, MA)

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

`.` is always empty air and needs no legend entry; any other grid character missing from `[legend]` is a parse error with row and column. `jnr-core` maps tile names to a collision kind (empty, solid, one-way, hazard, water: the `*_water*` tiles, D-050; swim: `see_wasser`, `see_wasser_tief`, `steg_pfahl`, D-076) in `TileKind::from_tile_name`; the grey-box names `solid`, `oneway`, `hazard` work in any level (see `content/levels/00_testbed.level`).

An entity's cell is empty air by default. Any entity entry may add `tile = "<tile name>"` to draw that tile in its cell under the spawn, e.g. `"w" = { entity = "nordic_walker", tile = "wall_village_bg" }` for a walker in front of a house wall or `{ entity = "oblate", tile = "wall_village_bg" }`. The tile must be non-colliding (`TileKind::Empty`: `wall_village_bg`, `railing_bg`) or swim water (`TileKind::Swim`, M5: Oblaten and lake enemies in the water, D-076) and takes no `contains`; anything else is a parse error. It is stored in the grid (`Level::tiles`), not in the `EntitySpawn`, so the entity behaves exactly as on air.

Height: typically 14 rows (224 px, camera scrolls vertically a little); climb levels are taller (Falkenstein: 42 rows). Auto-scroll levels (D-075) need `width ≥ to + 20` and their goal at column ≥ `to + 20` (`levels lint`); the grey box is `content/levels/00_m5_autoscroll.level`. Width: ~200–320 columns for a full stage.

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

All physics/gameplay numbers (accelerations, jump impulse, gravity, coyote frames, …) in one file, in fixed-point units, so designers can tune without touching code and tests can assert the resulting game-feel metrics. `Tuning::parse` validates every section (e.g. `[camera]`: the dead zone, the climb keys, and since M5 `autoscroll_speed` (subpx/frame, `0 < speed < player.walk_speed`; E27 adds `< swim.speed`), `autoscroll_hold` (0..=600 frames) and `autoscroll_right_margin` (0..=160 px)).

## Determinism contract

- No floats in `jnr-core` state or logic. (Floats allowed in render/app for scaling only.)
- No `HashMap`/`HashSet` iteration in core (use `Vec`/`BTreeMap`).
- No time, threads, filesystem or global state in core.
- Every replay test also runs the game twice and compares `state_hash()`; later the headless runner is built for `wasm32-wasip1` and compared against native (see `validation.md`).

## Out of scope for now

Slopes, save games, gamepad support (macroquad lacks native gamepad; add `gilrs`/web Gamepad API later), mobile touch controls, localisation beyond German (format is ready for it).
