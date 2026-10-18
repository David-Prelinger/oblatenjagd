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

## M2 – Stage 2 St. Gilgen + Stage 3 Brunnwinkl + route map

Scope as in `game-design.md` → "Multi-stage flow", "Route map", "Stage 2: St. Gilgen", "Stage 3: Brunnwinkl"; decisions D-041 … D-046; sprites in `sprite-catalog.md` → "M2 additions". Branch `m2-stgilgen-brunnwinkl`.

Design
- [x] Stage 2 and 3 specs, multi-stage flow, route map, new entities and mechanics tables (game-designer)
- [x] German text for both stages, the map and the new popup in `content/text/de.toml`

Engine (parallel worktrees, D-009)
- [ ] Multi-stage flow: route order data, stage switch after the goal sequence, carry-over (score, lives, Oblaten, power state), continue restarts the current stage, title starts Salzburg, teaser + „Fortsetzung folgt …" after the last playable stage, `--level` starts the run at a stage; intro shows `player_big` when carried over; app embeds the new levels (`options::LEVELS`); replays can follow the flow across a stage change (+ an expectation for the current stage)
- [ ] Route map mode: phases Arrive/Walk/Wait/Teaser, START skip/continue, auto-continue after 300 frames, route waypoints + station dots + labels + panel; map text keys in `screens::TEXT_KEYS`
- [ ] Gondel: free-running ping-pong platform (`to`, `phase`), cable drawing, `levels lint` clearance and pit recovery for `gondel`
- [ ] Nordic-Walker (ledge turning), Blasmusiker + notes, E-Biker (telegraph, gravity, crash → „Hoppala!" popup, no points), Bienenschwarm (`dir`, `len`, `phase`, figure-eight); chain start 200 for 200-point enemies; tuning keys validated in `Tuning::parse`
- [ ] New tile names and collision kinds (`wall_village_bg`, `balcony`, `facade_village`, `roof_shingle`, `jetty`, `jetty_post`, `wolfgangsee_water`, `wolfgangsee_water_deep`, `forest_path`, `beute`); new `PROP_SPRITES`
- [ ] Unit tests for every new behaviour listed in the "New mechanics" tables

Art (pixel-artist, art-director APPROVED)
- [ ] Enemies package: `nordic_walker`, `blasmusiker`, `note`, `e_biker`, `bienenschwarm`, platform `gondel`
- [ ] St. Gilgen package: tiles (`wall_village_bg`, `balcony`, `roof_shingle`, `jetty`, `jetty_post`, Wolfgangsee water), props (`prellbock`, `talstation`, `seilbahn_stuetze`, `mozartbrunnen`, `mozarthaus`, `musikpavillon`), layers `bg_schafberg`, `bg_wolfgangsee`, `bg_stgilgen`
- [ ] Brunnwinkl package: tiles (`facade_village`, `forest_path`, `beute`), props (`muehlrad`, `bienenhaus`, `futterschaelchen`), layer `bg_forest`
- [ ] Route map package: `map_salzkammergut`, `map_station`, the `é` font glyph

Levels (level-designer)
- [ ] `02_st_gilgen.level` (280 × 17) with the 6 sections; solver finishes it; `min_jump_window = 6` holds
- [ ] `03_brunnwinkl.level` (240 × 17) with the 5 sections and exactly five houses; every swarm points at its bonus; solver finishes it; `min_jump_window = 6` holds
- [ ] `time_limit` of both set from the measured relaxed run (D-045); Maibaum zones checked by replay (walk off → 100, running jump → Wurstkranz)

Replays and goldens
- [ ] Mechanic replays from both "New mechanics" tables (gondolas, trench recovery, walker on the jetty, note hit + bench shelter, swarm hit + wait, bonus 1 + Zaunerstollen, e-biker stomp / crash / side hit, checkpoint respawns)
- [ ] Flow replays: Salzburg clear → map → St. Gilgen intro with carry-over; game over in St. Gilgen → continue restarts St. Gilgen; Brunnwinkl clear → teaser → title
- [ ] Tours `st_gilgen_tour`, `brunnwinkl_tour` (`start = title`, solver inputs, shots per section incl. the checkpoint bus joke) and route-map goldens (after Salzburg; the teaser after Brunnwinkl; one shot with all labels) — art-director APPROVED
- [ ] M1 replays and goldens whose title-flow ending changes (map instead of „Fortsetzung folgt …") updated with an art-director verdict

Platform
- [ ] wasm build plays the route Salzburg → St. Gilgen → Brunnwinkl; `wasm-smoke` still green

**Acceptance:** see "Milestone acceptance" below; the game-designer review includes both stages' filmstrips, the route map and all new German text.

## Milestone acceptance (done by agents, no human)

1. `cargo xtask verify` green on the milestone branch.
2. `qa-tester` full pass over everything in the milestone: verdict PASS.
3. `art-director` reviews the full contact sheet and the filmstrip of the solver replay of every stage: APPROVED.
4. `game-designer` reviews the filmstrips and all German text against `game-design.md`: matches the spec, jokes land.
5. The orchestrator writes `docs/reports/M<N>.md`: what was built, screenshots, verify summary, known issues, deferred work.
6. Merge the milestone branch into `main`, tag `m<N>`, and start the next milestone.

## Later milestones (order can change)

- **M3** Falkenstein: vertical climbing, Hackl power-up, Durchkriechstein
- **M4** St. Wolfgang
- **M5** Wolfgangsee: auto-scroll, boats (Traundl, paddle steamer), swimming
- **M6** Strobl + Bad Ischl
- **M7** Café Zauner finale + ending
- **MA** Audio (low priority, can be pulled forward at any time): own sfxr-style synth in `jnr-audio` for sound effects; music written as text patterns (tracker style, Ländler/Blasmusik) rendered by the same synth; CC0 sounds from freesound.org/OpenGameArt only if needed, recorded in `assets/LICENSES.md`. Adds a `sound-designer` agent and a `sfx-authoring` skill. Re-enable macroquad's `audio` feature (disabled in M0).
- **MW** Web release: polished `index.html`, gamepad support, headless-browser smoke test, hosting
