# Milestones

Status: **approved v1**. Agents tick boxes here as work is completed (definition of done: `validation.md`).

## M0 – Foundations (no game content yet)

Goal: the whole toolchain and validation loop work end-to-end on a grey-box test level.

- [x] Human approves the plan docs, agents and skills (2026-10-18)
- [x] Cargo workspace with all crates (rename package `hello_daniel` away)
- [x] `xtask` with `check` and `verify` skeletons and JSON/Markdown reports
- [x] Hooks in `.claude/settings.json` (rustfmt, Stop → check, golden guard)
- [x] `jnr-core`: fixed-point types, `InputFrame`, `Game::step`, `state_hash`, tile collision, a player box that runs and jumps
- [ ] `jnr-render`: framebuffer, blit, bitmap font with German characters
- [x] `jnr-artkit`: `.palette` + `.sprite` parser, atlas packing, lints, preview + contact sheet
- [x] Palette and `docs/art-style.md` written and approved by art-director
- [ ] `jnr-app`: macroquad window on `:1`, integer scaling, keyboard input, replay playback, F12 screenshot
- [ ] Headless runner: replays, screenshots, filmstrips
- [ ] Solver bot finishes the grey-box test level
- [ ] Golden screenshot test working
- [ ] `vnc-smoke` working
- [ ] wasm build compiles and runs in a browser (manual check once, via `web/index.html`)

**Acceptance:** see "Milestone acceptance" below; plus the grey box moves on the noVNC desktop (`vnc-smoke`).

## M1 – Stage 1: Salzburg (first playable)

Scope as in `game-design.md` → "Stage 1: Salzburg".

- [ ] Player: walk, run, variable jump, coyote time, jump buffer, stomp; game-feel tests green
- [ ] Player states: normal, Zuckerschock (Mozartkugel), hurt with invulnerability frames, death
- [ ] Oblaten, Oblatenkisten, bricks, one-way Zunftschilder, Salzach water death
- [ ] Enemies: Selfie-Tourist, Taube, Mozartkugel-Verkäufer, Fiaker
- [ ] Checkpoint, Maibaum goal with height-based score
- [ ] Camera (no scrolling back), enemy activation by distance
- [ ] HUD, title screen, stage intro, game over, stage clear (German text from `de.toml`)
- [ ] Salzburg tileset, parallax (sky, Untersberg, Festung, rooftops), all M1 sprites — art-director APPROVED
- [ ] Level `01_salzburg.level` designed with the 5 sections; solver finishes it; difficulty report reviewed
- [ ] Replays + goldens for the key scenes
- [ ] wasm build runs the stage in the browser
- [ ] `verify` treats any remaining placeholder step as a failure (no "skipped" layers left)
- [ ] Stretch: Sound-of-Music tour group enemy

**Acceptance:** see "Milestone acceptance" below.

## Milestone acceptance (done by agents, no human)

1. `cargo xtask verify` green on the milestone branch.
2. `qa-tester` full pass over everything in the milestone: verdict PASS.
3. `art-director` reviews the full contact sheet and the filmstrip of the solver replay of every stage: APPROVED.
4. `game-designer` reviews the filmstrips and all German text against `game-design.md`: matches the spec, jokes land.
5. The orchestrator writes `docs/reports/M<N>.md`: what was built, screenshots, verify summary, known issues, deferred work.
6. Merge the milestone branch into `main`, tag `m<N>`, and start the next milestone.

## Later milestones (order can change)

- **M2** St. Gilgen + Brunnwinkl; route map screen between stages
- **M3** Falkenstein: vertical climbing, Hackl power-up, Durchkriechstein
- **M4** St. Wolfgang
- **M5** Wolfgangsee: auto-scroll, boats (Traundl, paddle steamer), swimming
- **M6** Strobl + Bad Ischl
- **M7** Café Zauner finale + ending
- **MA** Audio (low priority, can be pulled forward at any time): own sfxr-style synth in `jnr-audio` for sound effects; music written as text patterns (tracker style, Ländler/Blasmusik) rendered by the same synth; CC0 sounds from freesound.org/OpenGameArt only if needed, recorded in `assets/LICENSES.md`. Adds a `sound-designer` agent and a `sfx-authoring` skill. Re-enable macroquad's `audio` feature (disabled in M0).
- **MW** Web release: polished `index.html`, gamepad support, headless-browser smoke test, hosting
