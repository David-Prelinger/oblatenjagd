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

## M3 – Stage 4 Falkenstein: vertical climbing, Hackl, Durchkriechstein

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

## M4 – Stage 5 St. Wolfgang: Schafbergbahn, Operettensänger, Souvenirstände, Reisebusse

Scope as in `game-design.md` → "Stage 5: St. Wolfgang" (incl. "Difficulty and reach rules (stage 5)") and "Route map"; decisions D-066 … D-072; sprites in `sprite-catalog.md` → "M4 additions". One stage only; extras are in the backlog (D-066). Branch `m4-st-wolfgang`.

Design
- [x] Stage 5 spec: shape (side-scrolling with one long ride), difficulty and reach rules, sections with columns, section details, entities and behaviour, axe classification, new-mechanics table, sources (game-designer)
- [x] Decisions D-066 … D-072 (scope, Schafbergbahn, Operettensänger, Souvenirstand/Schneekugel, Reisebus, Hackl in Stage 5, route-map labels)
- [x] German text for Stage 5 (title, tagline, next, 14 signs) and the popup `popup.bravo` in `content/text/de.toml`
- [x] Sprite catalog "M4 additions"

Engine (parallel worktrees, D-009)
- [ ] Schafbergbahn: `{ entity = "schafbergbahn", to = [dx, dy] }` (4 ≤ |dx| ≤ 32) on the Festungsbahn state machine with `schafbergbahn_size` 64×32, `schafbergbahn_frames_per_tile` 24, `schafbergbahn_wait` 90; renderer: drawn track (rails, rack, sleepers, + 1 tile at each end) and `dampf` puffs; `levels lint`: swept-body clearance (body and 3 tiles above its roof free along the whole track), pit recovery skipped over lethal pits; tuning keys validated in `Tuning::parse` (fallbacks in D-067: tile track, or stepped Festungsbahn cars)
- [ ] Operettensänger: range counter (inhale 0–39, column 40–87, idle to 179, pauses out of range), the column (12 px, grows 8 px/frame to 80, passes one-way, stops at solid), column checked before the stomp, stomp 200 / chain start 200, knocked off by a bumped block, „Bravo!" popup on defeat (fallback D-068: fixed column + lint)
- [ ] Souvenirstand + Schneekugel: spawner fields `period`/`phase`/`range` (parse validation like the Steinschlag), counter only left of it, in range and on screen, telegraph 0–29, spawn at 30, max 2 alive; globe roll 1.0 with gravity, off ledges, shatters on walls (`schneekugel_scherben`), removed in water/off screen, passes enemies and items, stomp 100 in the chain (fallback D-069: not stompable)
- [ ] Reisebus: trigger < 161 px to its centre, telegraph 60, drive 1.5 ignoring tiles, optional `stop` (120-frame photo stop + 30-frame telegraph), roof bounce without damage or points in every state, side hurt in every state, removed off screen left; `levels lint`: lane clear 24 tiles (fallback D-070: no `stop`)
- [ ] Axe classification for every new kind (D-071) in the exhaustive `axe_hit` match; renderer: shatter look for a defeated Schneekugel
- [ ] New tiles and kinds: `standl` (solid), `standl_dach`, `wartehaus_dach` (one-way), `strasse` (solid), `tunnel_bg` (decorative), fallback `zahnrad_gleis_0..3` (decorative); new `PROP_SPRITES` (the catalog's "Props (group `props`, M4)" table; extend the catalog test in `xtask/src/levels.rs` to read it); `popup.bravo` in `screens::TEXT_KEYS`
- [ ] Route-map labels (D-072): St. Wolfgang's alternate slot (top y 69) on screens ending on Wolfgangsee or Strobl for every form; Falkenstein's alternate slot (x 170) on the screen ending on St. Wolfgang for every form; remove the two `st_wolfgang` pairs from `LATER` and the Falkenstein pair from `NORMAL_TOUCH` in the map test; all other map screens unchanged
- [ ] Solver handles the new kinds (`world_hash` covers singer counters, stand counters and globes, bus state, train state); a grey-box level `00_m4_enemies` with handwritten `m4_enemies_*` replays and its solver replay
- [ ] Unit tests for every behaviour in the Stage 5 "New mechanics" table

Art (pixel-artist, art-director APPROVED)
- [ ] Enemies package: `operettensaenger`, `hohes_c`, `souvenirstand`, `schneekugel`, `schneekugel_scherben`, `reisebus`
- [ ] Schafbergbahn package: `schafbergbahn`, `dampf`, the drawn-track colours (and the fallback `zahnrad_gleis_*` tiles only if D-067's fallback is taken)
- [ ] St. Wolfgang package: tiles `standl`, `standl_dach`, `wartehaus_dach`, `strasse`, `tunnel_bg`; props `talstation_schafberg`, `haltestelle_alm`, `tunnelportal`, `wartehaus`, `pilgerbrunnen`, `wallfahrtskirche`, `weisses_roessl`, `roessl_pferd`, `schiffsstation`; layers `bg_schafberg_sued`, `bg_stwolfgang`

Levels (level-designer)
- [ ] `05_st_wolfgang.level` (300 × 17, `camera = "side"`) with the 6 sections as specified; app `options::LEVELS` line; solver finishes it; `min_jump_window = 5` holds and the design target is met (narrowest 5–7, ≥ 3 required jumps ≤ 8), or the gap is recorded for the game-designer
- [ ] `levels lint` clean (train clearance, bus lanes); every bus has its dodge in view; stand #1's globes shatter on the curb; nothing reaches the train track while ridden
- [ ] `time_limit` set from the measured relaxed run (D-045); Oblaten 75 ± 10; Maibaum zones checked by replay (walk off → 100, running jump → Wurstkranz)

Replays and goldens
- [ ] Mechanic replays from the Stage 5 "New mechanics" table: train ride up and off at the Alm, fall off over the steps and re-board, fall into the gorge, Oblaten hops; stand #1 globes on the curb, globe stomp, globe hit as Normal; Markt lane with the tourist; bus #1 watched from the Alm, bounce over bus #2 at its photo stop, wait it out on the shelter, bus side hit as Zuckerschock; singer #2 stomp in the window, column hit as Zuckerschock, axe on #2; H-box as Normal, axes on the pilgrims and a globe, axe bounces off a bus; Zaunerstollen on the Kaiserterrasse; checkpoint respawn at 150 (buses parked again, boxes fresh)
- [ ] Flow replays: Falkenstein clear → map (Wait, Falkenstein label in its alternate slot) → St. Wolfgang intro with carry-over incl. the Hackl (`player_hackl` in the intro); St. Wolfgang clear → teaser to Wolfgangsee (St. Wolfgang label raised) → title; game over in St. Wolfgang → continue restarts St. Wolfgang
- [ ] Tour `st_wolfgang_tour` (`start = title`, solver inputs, shots per section incl. the train over the gorge, the bus at its photo stop, the pigeon on the Pilgerbrunnen, a high C, the H-box and the checkpoint bus joke) — art-director APPROVED; add it to the xtask tour coupling test
- [ ] M3 replays and goldens whose ending changes because St. Wolfgang is now playable re-cut with an art-director verdict (`falkenstein_tour` ends on the route map in Wait instead of the teaser; the M3 flow replays that end in the Falkenstein teaser now end on the St. Wolfgang intro); orphaned teaser goldens deleted
- [ ] Route-map label goldens: the Falkenstein → St. Wolfgang Wait screen and the St. Wolfgang → Wolfgangsee teaser in Normal and Hackl form (D-072); plus the M3 backlog shot of St. Gilgen → Brunnwinkl Wait with a big Resi (art-director)

Platform
- [ ] wasm build plays the route through St. Wolfgang; `wasm-smoke` still green; verify runtime stays within budget (record it: it grows with every stage)

**Acceptance:** see "Milestone acceptance" below; the game-designer review includes the St. Wolfgang filmstrip, the train ride, a bus photo stop, a high C over the street, the Hackl replays, the route-map label screens and all new German text.

## Milestone acceptance (done by agents, no human)

1. `cargo xtask verify` green on the milestone branch.
2. `qa-tester` full pass over everything in the milestone: verdict PASS.
3. `art-director` reviews the full contact sheet and the filmstrip of the solver replay of every stage: APPROVED.
4. `game-designer` reviews the filmstrips and all German text against `game-design.md`: matches the spec, jokes land.
5. The orchestrator writes `docs/reports/M<N>.md`: what was built, screenshots, verify summary, known issues, deferred work.
6. Merge the milestone branch into `main`, tag `m<N>`, and start the next milestone.

## Later milestones (order can change)

- **M3** Falkenstein: vertical climbing, Hackl power-up, Durchkriechstein (accepted, docs/reports/M3.md)
- **M4** St. Wolfgang: Schafbergbahn, Operettensänger, Souvenirstände, Reisebusse (in progress, section above)
- **M5** Wolfgangsee: auto-scroll, boats (Traundl, paddle steamer), swimming
- **M6** Strobl + Bad Ischl
- **M7** Café Zauner finale + ending
- **MA** Audio (low priority, can be pulled forward at any time): own sfxr-style synth in `jnr-audio` for sound effects; music written as text patterns (tracker style, Ländler/Blasmusik) rendered by the same synth; CC0 sounds from freesound.org/OpenGameArt only if needed, recorded in `assets/LICENSES.md`. Adds a `sound-designer` agent and a `sfx-authoring` skill. Re-enable macroquad's `audio` feature (disabled in M0).
- **MW** Web release: polished `index.html`, gamepad support, headless-browser smoke test, hosting
