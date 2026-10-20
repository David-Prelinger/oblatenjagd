# Milestones

Status: **approved v1**. Agents tick boxes here as work is completed (definition of done: `validation.md`).

## M0 – Foundations (no game content yet) — ✅ accepted (docs/reports/M0.md)

Goal: the whole toolchain and validation loop work end-to-end on a grey-box test level.

- [x] Human approves the plan docs, agents and skills (2026-10-18)
- [x] Cargo workspace with all crates (rename package `hello_daniel` away)
- [x] `xtask` with `check` and `verify` skeletons and JSON/Markdown reports
- [x] Hooks in `.claude/settings.json` (rustfmt, Stop → check, golden guard)
- [x] `jnr-core`: fixed-point types, `InputFrame`, `Game::step`, `state_hash`, tile collision, a player box that runs and jumps
- [x] `jnr-render`: framebuffer, blit, bitmap font with German characters
- [x] `jnr-artkit`: `.palette` + `.sprite` parser, atlas packing, lints, preview + contact sheet
- [x] Palette and `docs/art-style.md` written and approved by art-director
- [x] `jnr-app`: macroquad window on `:1`, integer scaling, keyboard input, replay playback (`--replay`, `--record`), F12 screenshot
- [x] Headless runner: replays, screenshots, filmstrips
- [x] Solver bot finishes the grey-box test level
- [x] Golden screenshot test working
- [x] `vnc-smoke` working
- [x] wasm build compiles and runs in a browser (automated: cargo xtask wasm-smoke)

**Acceptance:** see "Milestone acceptance" below; plus the grey box moves on the noVNC desktop (`vnc-smoke`).

## M1 – Stage 1: Salzburg (first playable) — ✅ accepted 2026-10-18 (docs/reports/M1.md)

Scope as in `game-design.md` → "Stage 1: Salzburg".

- [x] Player: walk, run, variable jump, coyote time, jump buffer, stomp; game-feel tests green
- [x] Player states: normal, Zuckerschock (Mozartkugel), hurt with invulnerability frames, death
- [x] Oblaten, Oblatenkisten, bricks, one-way Zunftschilder, Salzach water death
- [x] Enemies: Selfie-Tourist, Taube, Mozartkugel-Verkäufer, Fiaker
- [x] Checkpoint, Maibaum goal with height-based score
- [x] Camera (no scrolling back), enemy activation by distance
- [x] HUD, title screen, stage intro, game over, stage clear (German text from `de.toml`)
- [x] Salzburg tileset, parallax (sky, Untersberg, Festung, rooftops), all M1 sprites — art-director APPROVED
- [x] Level `01_salzburg.level` designed with the 5 sections; solver finishes it; difficulty report reviewed
- [x] Replays + goldens for the key scenes (salzburg_tour: 10 shots, art-director APPROVED)
- [x] wasm build runs the stage in the browser (E6: the app embeds every level and starts on the Salzburg title; `wasm-smoke` checks the Salzburg parallax sky)
- [x] `verify` treats any remaining placeholder step as a failure (no "skipped" layers left; `Skipped` status removed in M0)
- [ ] Stretch: Sound-of-Music tour group enemy — deferred to backlog (not required for M1)

**Acceptance:** see "Milestone acceptance" below.

## M2 – Stage 2 St. Gilgen + Stage 3 Brunnwinkl + route map — ✅ accepted 2026-10-19 (docs/reports/M2.md)

Scope as in `game-design.md` → "Multi-stage flow", "Route map", "Stage 2: St. Gilgen", "Stage 3: Brunnwinkl"; decisions D-041 … D-046; sprites in `sprite-catalog.md` → "M2 additions". Branch `m2-stgilgen-brunnwinkl`.

Design
- [x] Stage 2 and 3 specs, multi-stage flow, route map, new entities and mechanics tables (game-designer)
- [x] German text for both stages, the map and the new popup in `content/text/de.toml`

Engine (parallel worktrees, D-009)
- [x] Multi-stage flow: route order data, stage switch after the goal sequence, carry-over (score, lives, Oblaten, power state), continue restarts the current stage, title starts Salzburg, teaser + „Fortsetzung folgt …" after the last playable stage, `--level` starts the run at a stage; intro shows `player_big` when carried over; app embeds the new levels (`options::LEVELS`); replays can follow the flow across a stage change (+ an expectation for the current stage)
- [x] Route map mode: phases Arrive/Walk/Wait/Teaser, START skip/continue, auto-continue after 300 frames, route waypoints + station dots + labels + panel; map text keys in `screens::TEXT_KEYS`
- [x] Gondel: free-running ping-pong platform (`to`, `phase`), cable drawing, `levels lint` clearance and pit recovery for `gondel`
- [x] Nordic-Walker (ledge turning), Blasmusiker + notes, E-Biker (telegraph, gravity, crash → „Hoppala!" popup, no points), Bienenschwarm (`dir`, `len`, `phase`, figure-eight); chain start 200 for 200-point enemies; tuning keys validated in `Tuning::parse`
- [x] New tile names and collision kinds (`wall_village_bg`, `balcony`, `facade_village`, `roof_shingle`, `jetty`, `jetty_post`, `wolfgangsee_water`, `wolfgangsee_water_deep`, `forest_path`, `beute`); new `PROP_SPRITES`
- [x] Unit tests for every new behaviour listed in the "New mechanics" tables

Art (pixel-artist, art-director APPROVED)
- [x] Enemies package: `nordic_walker`, `blasmusiker`, `note`, `e_biker`, `bienenschwarm`, platform `gondel`
- [x] St. Gilgen package: tiles (`wall_village_bg`, `balcony`, `roof_shingle`, `jetty`, `jetty_post`, Wolfgangsee water), props (`prellbock`, `talstation`, `seilbahn_stuetze`, `mozartbrunnen`, `mozarthaus`, `musikpavillon`), layers `bg_schafberg`, `bg_wolfgangsee`, `bg_stgilgen`
- [x] Brunnwinkl package: tiles (`facade_village`, `forest_path`, `beute`), props (`muehlrad`, `bienenhaus`, `futterschaelchen`), layer `bg_forest`
- [x] Route map package: `map_salzkammergut`, `map_station`, the `é` font glyph

Levels (level-designer)
- [x] `02_st_gilgen.level` (280 × 17) with the 6 sections; solver finishes it; `min_jump_window = 6` holds
- [x] `03_brunnwinkl.level` (240 × 17) with the 5 sections and exactly five houses; every swarm points at its bonus; solver finishes it; `min_jump_window = 6` holds
- [x] `time_limit` of both set from the measured relaxed run (D-045: St. Gilgen 210, Brunnwinkl 200); Maibaum zones checked by replay (walk off → 100, running jump → Wurstkranz)

Replays and goldens
- [x] Mechanic replays from both "New mechanics" tables (gondolas, trench recovery, walker on the jetty, note hit + bench shelter, swarm hit + wait, bonus 1 + Zaunerstollen, e-biker stomp / crash / side hit, checkpoint respawns)
- [x] Flow replays: Salzburg clear → map → St. Gilgen intro with carry-over; game over in St. Gilgen → continue restarts St. Gilgen; Brunnwinkl clear → teaser → title
- [x] Tours `st_gilgen_tour`, `brunnwinkl_tour` (`start = title`, solver inputs, shots per section incl. the checkpoint bus joke) and route-map goldens (after Salzburg; the teaser after Brunnwinkl; one shot with all labels) — art-director APPROVED
- [x] M1 replays and goldens whose title-flow ending changes (map instead of „Fortsetzung folgt …") updated with an art-director verdict

Platform
- [x] wasm build plays the route Salzburg → St. Gilgen → Brunnwinkl; `wasm-smoke` still green

**Acceptance:** see "Milestone acceptance" below; the game-designer review includes both stages' filmstrips, the route map and all new German text.

## M3 – Stage 4 Falkenstein: vertical climbing, Hackl, Durchkriechstein — ✅ accepted 2026-10-19 (docs/reports/M3.md)

Scope as in `game-design.md` → "Stage 4: Falkenstein" (incl. "Climb camera" and "Climbing rules") and "Power states"; decisions D-053 … D-058; sprites in `sprite-catalog.md` → "M3 additions". One stage only. Branch `m3-falkenstein`.

Design
- [x] Stage 4 spec: vertical layout, climbing rules, climb camera, sections, entities, new-mechanics table, sources (game-designer)
- [x] Power hierarchy with Hackl, Durchkriechstein rules, enemy rules; decisions D-053 … D-058
- [x] German text for Stage 4 (title, tagline, next, 11 signs) and the popups `popup.hackl`, `popup.ballast` in `content/text/de.toml`
- [x] Sprite catalog "M3 additions"

Engine (parallel worktrees, D-009)
- [x] Climb camera: `[meta] camera = "side|climb"` (default side, existing levels bit-identical), settle / in-air follow / fall-follow limit, death line, start and respawn framing, goal-sequence framing at 164, vertical activation proximity; tuning keys `climb_*` validated in `Tuning::parse`
- [x] Background `factor_y` (level format, `RenderContext`, renderer); existing levels unchanged
- [x] Hackl power state: `contains = "hackl"`, the `hackl` item (rises, stays), Normal/Zuckerschock → Hackl, hit → Zuckerschock + i-frames, freeze with alternating sprites for every power change, carry-over across stages, `player_hackl` in the renderer, intro and route map, `throw` pose
- [x] Thrown Hackl: RUN-press throw, max 2, fixed arc, sticks in solids, passes one-way, removed in water/off screen; hits per kind as in D-056 (stompable defeated + base points, Fiaker immune, swarm/projectiles pass, rock 50)
- [x] Durchkriechstein: entity with `len`, start conditions, scripted crawl (hitbox 12×12, 1 px/frame, invulnerable, input ignored), outcomes (+1 life / shed to Normal + „Ballast weg!"), once per attempt, `crawl` anim in the renderer (fallback: hidden crawl, D-057)
- [x] Enemies: Pilger (walks off ledges, `lead`), Gams (stand/crouch/hop, landing check, never leaves its ledge, chain start 200), Steinschlag spawner (`period`, `phase`, `range`; counter only in range and on screen; telegraph + pebbles) + falling Stein (shatters on solid/one-way, hurts from above, `stein_debris`)
- [x] New tiles and kinds: `kalkfels`, `steig` and `netz` (one-way), `kalkfels_bg` (decorative); new `PROP_SPRITES`; new text keys in `screens::TEXT_KEYS`
- [x] `levels lint` for climb levels: fall depth (7–9 tiles rejected), spawner niche height (4–6 tiles above the ground below), Durchkriechstein geometry, shaft width ≤ 9 (warning)
- [x] Solver in climb levels: progress = x + height gained; jump-window recovery also checks height (D-054), or the documented fallback
- [x] Unit tests for every behaviour in the Stage 4 "New mechanics" table

Art (pixel-artist, art-director APPROVED)
- [x] Player package: `player_hackl` (all anims incl. `throw`, `crawl`), `crawl` for `player` and `player_big`, items `hackl`, `hackl_thrown`
- [x] Enemies package: `pilger`, `pilger_lead`, `gams`, `stein`, `kiesel`, `stein_debris`
- [x] Falkenstein package: tiles `kalkfels`, `kalkfels_bg`, `steig`, `netz`; props `gasthof_fuerberg`, `bildstock`, `steinhaufen`, `falkensteinkirche`, `brunnkapelle`, `hackelwurfkapelle`, `durchkriechstein`; layer `bg_falkenstein`

Levels (level-designer)
- [x] `04_falkenstein.level` (240 × 42, `camera = "climb"`) with the 6 sections, the shaft and the Durchkriechstein as specified; app `options::LEVELS` line; solver finishes it; `min_jump_window = 5` holds; fall-depth lint clean
- [x] `time_limit` set from the measured relaxed run (D-045); Oblaten 70 ± 10; Maibaum zones checked by replay (walk off → 100, running jump → Wurstkranz); D-029 crown check with a shot at the touch

Replays and goldens
- [x] Mechanic replays from the Stage 4 "New mechanics" table (cleft death at the death line, 6-tile drop survives, shaft miss, pilgrims down the zigzag, rock shelter + rock hit + Hackl vs rock, Gams stomp + Gams hit, Hackl pickup from Normal, axes on group C, Hackl hit → Zuckerschock, crawl as Normal / Zuckerschock / Hackl, big Resi at the mouth without DOWN, checkpoint respawn in the shaft)
- [x] Flow replays: Brunnwinkl clear → map → Falkenstein intro with carry-over; Falkenstein clear with the Hackl → teaser to St. Wolfgang (`player_hackl` on the map) → title; game over in Falkenstein → continue restarts Falkenstein
- [x] Tour `falkenstein_tour` (`start = title`, solver inputs, shots per section incl. the shaft, the church with the stone, the Hackl throw and the checkpoint bus joke) — art-director APPROVED
- [x] M2 replays and goldens whose ending changes because Falkenstein is now playable re-cut with an art-director verdict: re-cut (level-designer, M3): `brunnwinkl_tour` (ends on the route map in Wait; shot `teaser` → `route_map`, golden pending), `flow_m2_brunnwinkl_teaser_title` → `flow_m2_brunnwinkl_to_falkenstein` and `qa_m2_flow_full_route_teaser` → `qa_m2_flow_full_route_to_falkenstein` (both end on the Falkenstein intro with the run carried over); `brunnwinkl_tour_route_map` APPROVED and accepted, orphaned `brunnwinkl_tour_teaser.png` deleted

Platform
- [x] wasm build plays the route through Falkenstein; `wasm-smoke` still green; verify runtime stays within budget (backlog: it grows with every stage)

**Acceptance:** see "Milestone acceptance" below; the game-designer review includes the Falkenstein filmstrip, the climb-camera framing at the foot, in the shaft and at the summit, the Hackl and crawl replays, and all new German text.

## M4 – Stage 5 St. Wolfgang: Schafbergbahn, Operettensänger, Souvenirstände, Reisebusse — ✅ accepted 2026-10-19 (docs/reports/M4.md)

Scope as in `game-design.md` → "Stage 5: St. Wolfgang" (incl. "Difficulty and reach rules (stage 5)") and "Route map"; decisions D-066 … D-072; sprites in `sprite-catalog.md` → "M4 additions". One stage only; extras are in the backlog (D-066). Branch `m4-st-wolfgang`.

Design
- [x] Stage 5 spec: shape (side-scrolling with one long ride), difficulty and reach rules, sections with columns, section details, entities and behaviour, axe classification, new-mechanics table, sources (game-designer)
- [x] Decisions D-066 … D-072 (scope, Schafbergbahn, Operettensänger, Souvenirstand/Schneekugel, Reisebus, Hackl in Stage 5, route-map labels)
- [x] German text for Stage 5 (title, tagline, next, 14 signs) and the popup `popup.bravo` in `content/text/de.toml`
- [x] Sprite catalog "M4 additions"

Engine (parallel worktrees, D-009)
- [x] (E21) Schafbergbahn: `{ entity = "schafbergbahn", to = [dx, dy] }` (4 ≤ |dx| ≤ 32) on the Festungsbahn state machine with `schafbergbahn_size` 64×32, `schafbergbahn_frames_per_tile` 24, `schafbergbahn_wait` 90; renderer: drawn track (rails, rack, sleepers, + 1 tile at each end) and `dampf` puffs; `levels lint`: swept-body clearance (body and 3 tiles above its roof free along the whole track), pit recovery skipped over lethal pits; tuning keys validated in `Tuning::parse` (fallbacks in D-067: tile track, or stepped Festungsbahn cars)
- [x] (E22) Operettensänger: range counter (inhale 0–39, column 40–87, idle to 179, pauses out of range), the column (12 px, grows 8 px/frame to 80, passes one-way, stops at solid), column checked before the stomp, stomp 200 / chain start 200, knocked off by a bumped block, „Bravo!" popup on defeat (fallback D-068: fixed column + lint)
- [x] (E22) Souvenirstand + Schneekugel: spawner fields `period`/`phase`/`range` (parse validation like the Steinschlag), counter only left of it, in range and on screen, telegraph 0–29, spawn at 30, max 2 alive; globe roll 1.0 with gravity, off ledges, shatters on walls (`schneekugel_scherben`), removed in water/off screen, passes enemies and items, stomp 100 in the chain (fallback D-069: not stompable)
- [x] Reisebus: trigger < 161 px to its centre, telegraph 60, drive 1.5 ignoring tiles, optional `stop` (120-frame photo stop + 30-frame telegraph), roof bounce without damage or points in every state, side hurt in every state, removed off screen left; `levels lint`: lane clear 24 tiles (fallback D-070: no `stop`)
- [x] Axe classification for every new kind (D-071) in the exhaustive `axe_hit` match; renderer: shatter look for a defeated Schneekugel
- [x] (E21) New tiles and kinds: `standl` (solid), `standl_dach`, `wartehaus_dach` (one-way), `strasse` (solid), `tunnel_bg` (decorative), fallback `zahnrad_gleis_0..3` (decorative); new `PROP_SPRITES` (the catalog's "Props (group `props`, M4)" table; extend the catalog test in `xtask/src/levels.rs` to read it); `popup.bravo` in `screens::TEXT_KEYS`
- [x] (E21; slot at top y 59, D-072 addendum) Route-map labels (D-072): St. Wolfgang's alternate slot (top y 69) on screens ending on Wolfgangsee or Strobl for every form; Falkenstein's alternate slot (x 170) on the screen ending on St. Wolfgang for every form; remove the two `st_wolfgang` pairs from `LATER` and the Falkenstein pair from `NORMAL_TOUCH` in the map test; all other map screens unchanged
- [x] Solver handles the new kinds (`world_hash` covers singer counters, stand counters and globes, bus state, train state); a grey-box level `00_m4_enemies` with handwritten `m4_enemies_*` replays and its solver replay
- [x] Unit tests for every behaviour in the Stage 5 "New mechanics" table

Art (pixel-artist, art-director APPROVED)
- [x] Enemies package: `operettensaenger`, `hohes_c`, `souvenirstand`, `schneekugel`, `schneekugel_scherben`, `reisebus`
- [x] Schafbergbahn package: `schafbergbahn`, `dampf`, the drawn-track colours (and the fallback `zahnrad_gleis_*` tiles only if D-067's fallback is taken)
- [x] St. Wolfgang package: tiles `standl`, `standl_dach`, `wartehaus_dach`, `strasse`, `tunnel_bg`; props `talstation_schafberg`, `haltestelle_alm`, `tunnelportal`, `wartehaus`, `pilgerbrunnen`, `wallfahrtskirche`, `weisses_roessl`, `roessl_pferd`, `schiffsstation`; layers `bg_schafberg_sued`, `bg_stwolfgang`

Levels (level-designer)
- [x] `05_st_wolfgang.level` (300 × 17, `camera = "side"`) with the 6 sections as specified; app `options::LEVELS` line; solver finishes it; `min_jump_window = 5` holds and the design target is met (narrowest 5–7, ≥ 3 required jumps ≤ 8), or the gap is recorded for the game-designer
- [x] `levels lint` clean (train clearance, bus lanes); every bus has its dodge in view; stand #1's globes shatter on the curb; nothing reaches the train track while ridden
- [x] `time_limit` set from the measured relaxed run (D-045); Oblaten 75 ± 10; Maibaum zones checked by replay (walk off → 100, running jump → Wurstkranz)

Replays and goldens
- [x] Mechanic replays from the Stage 5 "New mechanics" table: train ride up and off at the Alm, fall off over the steps and re-board, fall into the gorge, Oblaten hops; stand #1 globes on the curb, globe stomp, globe hit as Normal; Markt lane with the tourist; bus #1 watched from the Alm, bounce over bus #2 at its photo stop, wait it out on the shelter, bus side hit as Zuckerschock; singer #2 stomp in the window, column hit as Zuckerschock, axe on #2; H-box as Normal, axes on the pilgrims and a globe, axe bounces off a bus; Zaunerstollen on the Kaiserterrasse; checkpoint respawn at 150 (buses parked again, boxes fresh)
- [x] Flow replays: Falkenstein clear → map (Wait, Falkenstein label in its alternate slot) → St. Wolfgang intro with carry-over incl. the Hackl (`player_hackl` in the intro); St. Wolfgang clear → teaser to Wolfgangsee (St. Wolfgang label raised) → title; game over in St. Wolfgang → continue restarts St. Wolfgang
- [x] Tour `st_wolfgang_tour` (`start = title`, solver inputs, shots per section incl. the train over the gorge, the bus at its photo stop, the pigeon on the Pilgerbrunnen, a high C, the H-box and the checkpoint bus joke) — art-director APPROVED; add it to the xtask tour coupling test
- [x] M3 replays and goldens whose ending changes because St. Wolfgang is now playable re-cut with an art-director verdict (`falkenstein_tour` ends on the route map in Wait instead of the teaser; the M3 flow replays that end in the Falkenstein teaser now end on the St. Wolfgang intro); orphaned teaser goldens deleted
- [x] Route-map label goldens: the Falkenstein → St. Wolfgang Wait screen and the St. Wolfgang → Wolfgangsee teaser in Normal and Hackl form (D-072); plus the M3 backlog shot of St. Gilgen → Brunnwinkl Wait with a big Resi (art-director)

Platform
- [x] wasm build plays the route through St. Wolfgang; `wasm-smoke` still green; verify runtime stays within budget (record it: it grows with every stage)

**Acceptance:** see "Milestone acceptance" below; the game-designer review includes the St. Wolfgang filmstrip, the train ride, a bus photo stop, a high C over the street, the Hackl replays, the route-map label screens and all new German text.

## M5 – Stage 6 Wolfgangsee: auto-scroll, swimming, Traundl and Dampfer, Schwäne, Gelsen, Regatta — ✅ accepted 2026-10-20 (docs/reports/M5.md)

Scope as in `game-design.md` → "Stage 6: Wolfgangsee" (incl. "Difficulty and reach rules (stage 6)", "Auto-scroll camera" and "Swimming") and "Power states"; decisions D-074 … D-081; sprites in `sprite-catalog.md` → "M5 additions". One stage only; extras are in the backlog (D-074). Branch `m5-wolfgangsee`.

Design
- [x] Stage 6 spec: shape (a lake crossing with an auto-scroll zone, no lethal pits), vertical layout, difficulty and reach rules, auto-scroll and swimming rules with numbers, sections with columns, section details, entities and behaviour, axe classification, new-mechanics table, sources (game-designer)
- [x] Decisions D-074 … D-081 (scope, auto-scroll camera, swimming, boats, Schwan, Gelse, Regattaboot, Hackl in Stage 6; route-map labels need no decision, D-074)
- [x] German text for Stage 6 (title, tagline, next, 12 signs) and the popup `popup.beleidigt` in `content/text/de.toml`
- [x] Sprite catalog "M5 additions"

Engine (parallel worktrees, D-009; E25–E29 can run in parallel, E30 integrates after them; reserve distinct hash tags per task, the distinct-tag test catches clashes)
- [x] (E25) verify speed-up: two-pass parallel solver, parallel replays and goldens, no full solve inside `cargo test`, per-process solver temp dirs (D-082; verify ~8.3 → ~1.5–2.5 min)
- [x] (E26) Auto-scroll camera: `[meta] camera = "autoscroll"` + `autoscroll = [from, to]` (parse validation: required with this camera only, `0 ≤ from < to ≤ width − 20`); lead-in (side camera) → scroll at exactly `from × 16`, 192 subpx/frame with a subpixel scroll position and a whole-px camera, paused while frozen/paused/dying/in the goal → lead-out at `to × 16`; push with tile collision and the squeeze death (`DeathCause::Squeezed`, normal hit death); right clamp `camera x + 320 − 64` for movement and carry; D-051 off inside the zone only; start/respawn framing + 60-frame hold inside the zone; the "fully on screen" helper for enemy triggers; tuning `camera.autoscroll_speed`/`_hold`/`_right_margin` validated in `Tuning::parse`; `levels lint`: goal ≥ `to + 20`, squeeze faces (top ≤ 3 tiles above the surface in front), checkpoint on a deck; solver frame budget covers the scroll, `world_hash` covers the scroll state; grey box `00_m5_autoscroll` with `m5_autoscroll_*` replays (pushed into water, squeezed against a test wall, platform carry past the edge, checkpoint hold). `side`/`climb` levels bit-identical. Fallback (D-075): the stage ships as `camera = "side"`. Done (D-083): no fallback taken; the right clamp caps her speed at the scroll speed (no stutter); the "fully on screen" helpers are `Camera::fully_on_screen` and `UpdateCtx::fully_on_screen`; the grey box water stays lethal until E27
- [x] (E27) Swimming: `TileKind::Swim` and the tiles `see_wasser`, `see_wasser_tief`, `steg_pfahl`; the chest-point rule, surface band and rise cap, floating + DOWN dive, sinking, stroke (`Event::Stroke`), jump-out (`Event::Jumped`, `jump_speed_walk`), horizontal accel/decel/max, entry clamp + cleared buffer/coyote, the bed as ground; same numbers in every form; no stomps or roof bounces while swimming, no throw, axes and items removed/lost in swim water, walking enemies treat Swim like Water; entity-on-tile (D-049) extended to Swim tiles; tuning `[swim]` validated (`autoscroll_speed < swim.speed` once E26 lands); the solver's window metric skips strokes; swim lint (a solid bed under every swim column; no solid on a swim surface in a scroll zone); renderer: the `swim` anim per form (fallback: the `jump` anim), optional `blase`; feel tests (stroke rise, max sink, swim speed, float, held/tap jump-out onto a 1-tile deck in every form); grey box `00_m5_swim` with `m5_swim_*` replays. D-050's water tests pass unchanged. Fallback (D-076): surface-only swimming
- [x] (E28) Boats: `traundl` (`to = [dx, 0]`, 2 ≤ |dx| ≤ 8, `phase`; the Gondel's free-running machine with `traundl_size` 48×16, `traundl_wait`, `traundl_frames_per_tile`) and `dampfer` (`to = [dx, 0]`, 8 ≤ dx ≤ 64; the Festungsbahn's waiting machine one way: travel `dampfer_frames_per_tile` 20, then moored for good; `dampfer_size` 160×16; the paddle-wheel hurt zone 32×20 at the hull's middle while it travels, hurting the player, never stomped, axe-proof); both `Pass` for the axe; shared landing/carry/crush/never-despawn and the clearance lint; the recovery lint skipped over swim water (stub it on `Water | Swim` until E27 lands); tiles `steg` (one-way), `seegrund`, `kai`, `poller` (solid); `PROP_SPRITES` for the "Props (group `props`, M5)" table and the catalog test reading it; renderer: `traundl` (`idle`/`ride`), `dampfer` (`idle`/`ride`), `gischt` spray while the wheel turns, grey boxes until the art lands; tuning validated (the Dampfer at least as fast as the scroll); grey box `00_m5_boats` with `m5_boats_*` replays. Fallbacks (D-077): decorative wheel; a returning Dampfer; Traundl = `gondel` with the Traundl sprite
- [x] (E29) Enemies: Schwan (float height, trigger in range + fully on screen + rested, telegraph 32, charge 1.5 × 64, return 0.5, rest 60, stomp 200 / chain start 200, „Beleidigt!" stacked like „Bravo!"), Gelse (wait 60 frames fully on screen, homing accel 1/32 and speed 0.75 per axis towards her top + 4, never enters water or swim tiles, hunt 480 → leaves harmless, stomp 100), Regattaboot (the Reisebus machine in the surface row, 24×56 hitbox from the masthead to the keel, trigger < 161 px + fully on screen, telegraph 60, left at 1.5 ignoring tiles, bounce without damage or points from above, lane lint); axe classification (Schwan/Gelse Defeat, Regattaboot Bounce) in the exhaustive `axe_hit` match; `popup.beleidigt` in `screens::TEXT_KEYS`; `world_hash` covers the new states; grey box `00_m5_enemies` with `m5_enemies_*` replays (uses `wolfgangsee_water` as the float/avoid surface until E27's Swim kind lands, then switches). Fallbacks: D-078 pacing swan, D-079 Taube-sine Gelse, D-080 Fiaker machine
- [x] (E30, after E26–E29) Integration: a grey box `00_m5_lake` with every M5 kind inside an auto-scroll zone over swim water (solver finishes it, narrowest window ≥ 5, its solver replay); the solver handles swimming in the zone (route-limited fallback per D-075 recorded if taken); interaction tests across tasks (a swan or Gelse vs a diver, the wheel vs a swimmer, the edge pushing a swimmer and a Traundl rider, a respawn in the zone)
- [x] Unit tests for every behaviour in the Stage 6 "New mechanics" table

Art (pixel-artist, art-director APPROVED)
- [x] Player package: `swim` for `player`, `player_big`, `player_hackl`; `blase` (optional)
- [x] Enemies package: `schwan`, `gelse`, `regattaboot`
- [x] Boats package: `traundl`, `dampfer`, `gischt`
- [x] Wolfgangsee package: tiles `see_wasser`, `see_wasser_tief`, `steg_pfahl`, `seegrund`, `steg`, `kai`, `poller`; props `adventlaterne`, `dalben`, `schilf`, `startboje`; layer `bg_suedufer`. Swim water must read as different from the lethal `wolfgangsee_water` at 1× (contact sheet side by side)

Levels (level-designer)
- [x] `06_wolfgangsee.level` (260 × 17, `camera = "autoscroll"`, `autoscroll = [16, 208]`, D-087) with the 7 sections as specified; app `options::LEVELS` line; solver finishes it; `min_jump_window = 5` holds; narrowest window and the named hazard timings recorded for the game-designer (D-074/D-073 style)
- [x] `levels lint` clean (squeeze faces, swim beds, regatta lanes, boat clearance, goal ≥ `to + 20`); every enemy on screen ≥ 60 frames before it can touch Resi (checked with a replay hugging the right clamp); the respawn on the pontoon checked with a replay; the steamer's `to` puts its moored front within reach soon after arrival
- [x] `time_limit` set from the measured relaxed run (D-045); Oblaten 70 ± 10; Maibaum zones checked by replay (walk off → 100, running jump → Wurstkranz)

Replays and goldens
- [x] Mechanic replays from the Stage 6 "New mechanics" table: the bathing notch (dive, collect, jump out); idle on the quay → pushed into the water; the steamer ride to Gschwendt; missing the steamer → swim and board at the bow; a swimmer hurt by the paddle wheel; swan #1 under the deck; dive under swan #2, jump-out stomp, hurt as Zuckerschock; Gelse dive-away, stomp from a Traundl, hurt as Normal → death; the Traundl chain and a missed hop → swim; regatta dive, sail jump from Traundl D, masthead bounce, side hit as Zuckerschock; Zaunerstollen on the perch; checkpoint respawn on the pontoon (hold, scroll, level reset); Hackl: axe on a swan from the deck, RUN while swimming throws nothing, axe bounces off a regatta boat
- [x] Flow replays: St. Wolfgang clear → map (Wait) → Wolfgangsee intro with carry-over incl. the Hackl; Wolfgangsee clear → teaser to Strobl → title; game over in Wolfgangsee → continue restarts Wolfgangsee
- [x] Tour `wolfgangsee_tour` (`start = title`, solver inputs, shots per section incl. the bathing notch, the steamer with its wheel, a swan's hiss, a Gelse over a diver, the lantern with the checkpoint bus joke, a Traundl, a regatta start, the Strobl quay) — art-director APPROVED; add it to the xtask tour coupling test
- [x] M4 replays and goldens whose ending changes because the Wolfgangsee is now playable re-cut with an art-director verdict (`st_wolfgang_tour` ends on the route map in Wait instead of the teaser; the M4 flow replays that end in the St. Wolfgang teaser now end on the Wolfgangsee intro); orphaned teaser goldens deleted
- [x] Route-map goldens: St. Wolfgang → Wolfgangsee Wait and the Wolfgangsee → Strobl teaser, Normal and Hackl form (St. Wolfgang's raised label, D-072)

Platform
- [x] wasm build plays the route through the Wolfgangsee; `wasm-smoke` still green; verify runtime within budget after E25 (record it): 320 s at M5 acceptance, solver step 271 s

**Acceptance:** see "Milestone acceptance" below; the game-designer review includes the Wolfgangsee filmstrip, the scroll start on the steamer, a dive under a swan, a Gelse waiting over a diver, a regatta start, the respawn on the pontoon, the Hackl replays and all new German text.

## M6 – Stage 7 Strobl + Stage 8 Bad Ischl: Fensterläden, Ampeln and Autos, Kurgäste, Offiziere

Scope as in `game-design.md` → "Stage 7: Strobl" and "Stage 8: Bad Ischl" (incl. their "Difficulty and reach rules") and "Route map"; decisions D-092 … D-098; sprites in `sprite-catalog.md` → "M6 additions". Two stages (as in M2); extras are in the backlog (D-092). Branch `m6-strobl-bad-ischl`.

Design
- [x] Stage 7 and Stage 8 specs: shape (two side levels), difficulty and reach rules, sections with columns, section details, entities and behaviour, axe classification, new-mechanics tables, sources (game-designer)
- [x] Decisions D-092 … D-098 (scope, Ampel/Auto, Fensterladen, Kurgast, Offizier, Postbus/water/Hackl/ending, route-map labels at Bad Ischl and Café Zauner)
- [x] German text for both stages (titles, taglines, next, 13 + 15 signs) and the popups `popup.kurruhe`, `popup.abtreten` in `content/text/de.toml`
- [x] Sprite catalog "M6 additions"

Engine (parallel worktrees, D-009; E36–E40 can run in parallel, E41 integrates after them; each task uses only its reserved hash tags, the distinct-tag test catches clashes; current maxima: `EntityKind` 25, `EnemyKind` 16, `Popup` sub-tag 8)
- [ ] (E35) solver speed-up (D-091) — in progress elsewhere; must land before the M6 levels are solved in `verify`
- [ ] (E36) Traffic: the `ampel` spawner (`period` 300–600 with the minimum `30 + cars × 40 + 64 + 30 + 120`, `cars` 1–4, `phase` 0 or ≥ `30 + cars × 40`, `range` 6–16; parse validation; counter only left of it, in range and fully on screen; red-amber 0–29, cars at 30 + k × 40, four green blinks 64, amber 30, red; max 4 alive; a spawn overlapping the player is skipped) and the `auto` (28×14, left at 2.0, ignores tiles, passes enemies and platforms, roof bounce without damage or points in every form, side hurt, removed off screen left; axe Bounce; walkers ignore it); the Reisebus lane lint extended to Ampel lanes (24 tiles left of the spawner); `reisebus` field `look = "postbus"` (parse; renderer only: `postbus` sprite, `halt` anim at the stop); renderer: `ampel` light anims from the counter, `auto` looks by spawn index, grey boxes until the art lands; tuning `enemies.ampel_*`, `enemies.auto_*` validated in `Tuning::parse`; `world_hash` covers the counters and cars; grey box `00_m6_traffic` with `m6_traffic_*` replays (watch from above, cross in red, wait on a shelter, roof bounces, side hit, Postbus halt) and its solver replay. **Hash tags**: `EntityKind::Ampel` 26 (reserved 26–27), `EnemyKind::Auto` 17 (reserved 17–18). Fallback (D-093): single `auto` chargers + a decorative Ampel
- [ ] (E37) Fensterladen + the stage clock: `{ entity = "fensterladen", period, open, phase }` (parse validation as D-094); **the stage clock** (frames of play since the stage start or the last respawn; frozen, paused and dying frames don't count; in `state_hash`; a `Game` accessor E39 uses); a stationary platform whose one-way top is on only while open (landing, standing, jump-through via the shared platform code; a closing shutter drops a rider with coyote frames, never pushes or crushes; an opening one never lifts a player inside it); never despawns; the shared clearance and recovery lints with a zero-length track; renderer: `open`/`rattle`/`closed`/`warn` from the clock (rattle shake renderer-only), grey box until the art lands; tuning `platforms.fensterladen_*` validated; grey box `00_m6_shutters` with `m6_shutters_*` replays (staircase pair, closing under her, jump-through while opening). **Hash tags**: `EntityKind::Fensterladen` 28 (reserved 28–29). Fallback (D-094): static one-way platforms
- [x] (E38) Kurgast: walker 0.375 with the Nordic-Walker turn rules; stomp → lying (16×16 one-way top platform, harmless, walkers turn at it, chargers/cars/notes pass), rest 300 + wake 40 pausing while she stands on him, stands up with the feet kept; 100 on the first stomp only (later stomps bounce, no points, chain unchanged); bump knocks off; „Kurruhe!" popup stacked like „Bravo!"; axe Defeat walking / Pass lying; `sinks_in_swim_water` true; the enemy-as-platform hook in the shared landing code (one-way top, never moves); renderer: `kurgast` + `liegestuhl` anims, grey boxes; tuning `enemies.kurgast_*` validated; `popup.kurruhe` in `screens::TEXT_KEYS`; grey box `00_m6_kurgast` with `m6_kurgast_*` replays (chair as a step to a 4-tile ledge, waiting on the chair through a full rest, hurt by a waking Kurgast, a second stomp scores nothing). **Hash tags**: `EnemyKind::Kurgast` 19 (reserved 19–20), `Popup::Kurruhe` 9 (reserved 9). Fallback (D-095): no platform, squished and removed
- [ ] (E39) Offizier: the drill on the stage clock (needs E37's clock; until it merges, a stub `Game::stage_clock()` marked `// E37:`), `phase` and `lead` fields (parse), march 0.5 for f 0–167 with the Nordic-Walker turn rules, halt 168–239, the 8×16 sabre zone during 192–239 checked before the stomp in `Game::interact` (the high-C path), 200 with chain start 200, „Abtreten!" on stomp/axe/bump; axe Defeat, sabre Pass; `sinks_in_swim_water` true; renderer: `offizier`/`offizier_lead` anims, the sabre drawn from the zone, grey boxes; tuning `enemies.offizier_*` validated; `popup.abtreten` in `screens::TEXT_KEYS`; grey box `00_m6_offiziere` with `m6_offiziere_*` replays (file in step after staggered activation, 3-chain in the march, sabre hurt, stomp during „Habt Acht!"). **Hash tags**: `EnemyKind::Offizier` 21 (reserved 21–22), `Popup::Abtreten` 10 (reserved 10–11). Fallback (D-096): a plain marching file
- [ ] (E40) Route map, tiles and props: D-098's alternate slots (`bad_ischl`: `label_alt` top 94 / right 266, `cafe_zauner`: top 58 / right 290, both `label_alt_ends = ["bad_ischl"]`), `LATER` removed from the map test, a unit test pinning both slots; new tiles `felssteg` (one-way), `kies` (solid), `einfahrt_bg` (`Empty`); `PROP_SPRITES` for the "Props (group `props`, M6)" table and the catalog test reading it; `options::LEVELS` lines for `07_strobl` and `08_bad_ischl` (with the levels). No hash tags. Fallback (D-098): Bad Ischl's alternate below the dot, or the art-director moves the dot
- [ ] (E41, after E35–E40) Integration: a grey box `00_m6_mix` with every M6 kind (a shutter over a car lane, a Kurgast's chair in an Ampel lane, an officer file meeting a lying Kurgast, a Postbus, a singer next to a file; the solver finishes it, narrowest window ≥ 4, its solver replay); the solver handles the new kinds (`world_hash` covers the stage clock, Ampel counters and cars, shutter states, Kurgast rest/wake, officer drill); the exhaustive `axe_hit` and `sinks_in_swim_water` matches name every new kind (D-097); interaction tests across tasks (a car passes a lying Kurgast; a closing shutter under a Kurgast-chair-jumper; officers turn at a lying Kurgast; a respawn restarts the clock, the shutters and the drill) Also (art-director, Strobl art round 1): `kalkfels_bg` counts a `felssteg` neighbour as the same tile on all four sides (the deck draws the joined cliff behind itself, like the `steig_wall` pair in `one_way_join`); `felssteg` itself only joins `felssteg`.
- [ ] Unit tests for every behaviour in the Stage 7 and Stage 8 "New mechanics" tables

Art (pixel-artist, art-director APPROVED)
- [x] Traffic package: `auto` (three looks), `postbus` (with `halt`), `ampel`
- [x] Strobl package: platform `fensterladen`; tiles `felssteg`, `einfahrt_bg`; props `buste`, `pension` (or its two variants), `gasthof_strobl`, `lokalbahn_haltestelle`; layer `bg_strobl`
- [x] Bad Ischl enemies package: `kurgast`, `liegestuhl`, `offizier`, `offizier_lead`
- [x] Bad Ischl package: tile `kies`; props `kaiser_jagdstandbild`, `katrin_talstation`, `kaiservilla`, `seeauerhaus`, `lehar_villa`, `trinkhalle`, `kongresshaus`, `trafik`, `zauner_fassade`, `warteschlange`; layers `bg_katrin`, `bg_badischl`
- [ ] Route map: check the Bad Ischl spire and the Café Zauner cake against the new label slots (D-098); move a dot by ≤ 2 px if needed

Levels (level-designer)
- [ ] `07_strobl.level` (280 × 17, `camera = "side"`) with the 6 sections as specified; app `options::LEVELS` line; solver finishes it; `min_jump_window = 5` holds; narrowest window and the named timings (shutter pair, Ampel #2, walkway gaps with Gelse #1, E-Biker #2 at the Ache) recorded for the game-designer (D-073 style)
- [ ] `08_bad_ischl.level` (300 × 17, `camera = "side"`) with the 6 sections as specified; app `options::LEVELS` line; solver finishes it; `min_jump_window = 4` holds; narrowest window and the named timings (files B and C, notch 1 with Kurgast #3, singer #2, the Fiaker) recorded
- [ ] `levels lint` clean in both (car and bus lanes, platform clearance, pen walls ≥ 2 high, signs ≥ 6 columns apart and D-088, collectibles row 8 or lower and the box rule checked with a shot at every box); every Ampel has its refuge in view; both shutters of the required pair open together ≥ 60 frames
- [ ] `time_limit` of both set from the measured relaxed run (D-045, measured by the ZEIT clock, D-090); Oblaten 75 ± 10 (Strobl) and 80 ± 10 (Bad Ischl); Maibaum zones checked by replay in both (walk off → 100, running jump → Wurstkranz)

Replays and goldens
- [ ] Mechanic replays from the Stage 7 "New mechanics" table: shutter #1 → balcony → Zaunerstollen; the pair #2 → #3 → annex; a shutter closing under her; the Dorfplatz shutters as a refuge from the Postbus; Postbus halt waited out on shelter B and bounced over; Ampel #1 watched from the terrace; Ampel #2 crossed in red, waited out on shelter C, roof bounces; a car side hit as Zuckerschock; walkway fall → death; Gelse #1 stomped on deck C; E-Biker #1 crash, E-Biker #2 into the Ache; checkpoint respawn at 132
- [ ] Mechanic replays from the Stage 8 "New mechanics" table: Kurgast #1 → chair → canopy; waiting on a chair through a full rest; hurt by a waking Kurgast; the chair as a step over file B; file A watched from the terrace; a 3-chain on file B; a sabre hurt as Zuckerschock; file C + singer #2; the Fiaker beneath Resi on the villa steps; checkpoint respawn at 152 (drill and Kurgäste reset)
- [ ] Hackl replays: carry the Hackl into Strobl (intro draws `player_hackl`), an axe bounces off a car and off the Postbus; axes on a walking Kurgast (Defeat) and through a lying one (Pass); an axe on an officer („Abtreten!")
- [ ] Flow replays: Wolfgangsee clear → map (Wait) → Strobl intro with carry-over; Strobl clear → map (Wait, D-098 labels) → Bad Ischl intro; Bad Ischl clear → teaser to Café Zauner → title; game over in Strobl and in Bad Ischl → continue restarts that stage
- [ ] Tours `strobl_tour` and `bad_ischl_tour` (`start = title`, solver inputs, shots per section incl. a shutter opening, the Postbus at its halt, a car stream under the terrace, the walkway, an E-Biker in the Ache, the chair under Resi, a file presenting sabres, the queue at the goal, both checkpoint bus jokes) — art-director APPROVED; add both to the xtask tour coupling test
- [ ] M5 replays and goldens whose ending changes because Strobl is now playable re-cut with an art-director verdict (`wolfgangsee_tour` ends on the route map in Wait instead of the teaser; the M5 flow replays that end in the Wolfgangsee teaser now end on the Strobl intro); orphaned teaser goldens deleted
- [ ] Route-map goldens: Strobl → Bad Ischl Wait (both alternate slots) and the Bad Ischl → Café Zauner teaser, Normal and Hackl form

Platform
- [ ] wasm build plays the route through Bad Ischl; `wasm-smoke` still green; verify runtime within budget after E35 (record it)

**Acceptance:** see "Milestone acceptance" below; the game-designer review includes both stages' filmstrips, a shutter staircase, the Postbus halt, an Ampel's car stream, the Bürglstein walkway, a Kurgast's chair used as a step, a file presenting sabres, the queue at Café Zauner, the route-map label screens and all new German text.

## Milestone acceptance (done by agents, no human)

1. `cargo xtask verify` green on the milestone branch.
2. `qa-tester` full pass over everything in the milestone: verdict PASS.
3. `art-director` reviews the full contact sheet and the filmstrip of the solver replay of every stage: APPROVED.
4. `game-designer` reviews the filmstrips and all German text against `game-design.md`: matches the spec, jokes land.
5. The orchestrator writes `docs/reports/M<N>.md`: what was built, screenshots, verify summary, known issues, deferred work.
6. Merge the milestone branch into `main`, tag `m<N>`, and start the next milestone.

## Later milestones (order can change)

- **M3** Falkenstein: vertical climbing, Hackl power-up, Durchkriechstein (accepted, docs/reports/M3.md)
- **M4** St. Wolfgang: Schafbergbahn, Operettensänger, Souvenirstände, Reisebusse (accepted, docs/reports/M4.md)
- **M5** Wolfgangsee: auto-scroll, boats (Traundl, paddle steamer), swimming (accepted, docs/reports/M5.md)
- **M6** Strobl + Bad Ischl
- **M7** Café Zauner finale + ending
- **MA** Audio (low priority, can be pulled forward at any time): own sfxr-style synth in `jnr-audio` for sound effects; music written as text patterns (tracker style, Ländler/Blasmusik) rendered by the same synth; CC0 sounds from freesound.org/OpenGameArt only if needed, recorded in `assets/LICENSES.md`. Adds a `sound-designer` agent and a `sfx-authoring` skill. Re-enable macroquad's `audio` feature (disabled in M0).
- **MW** Web release: polished `index.html`, gamepad support, headless-browser smoke test, hosting
