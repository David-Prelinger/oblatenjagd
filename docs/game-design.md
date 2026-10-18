# Game design

Status: **approved v1**. Title: **„Oblatenjagd"**.

## Premise

The hero, **Resi** – a Salzburg local in a Dirndl – wants one thing: a fresh slice of Zaunerstollen at **Café Zauner in Bad Ischl**. But Postbus 150 (Salzburg → Bad Ischl, a real line) is hopelessly full of tourists, so the hero sets off on foot along the old route of the *Ischlerbahn* (the Salzkammergut-Lokalbahn, closed 1957). On the way: tour groups, Fiaker, pilgrims, swans, regatta sailors and the entire Salzkammergut cliché machine.

Tone: affectionate satire. We make fun of tourism, kitsch, Austrian bureaucracy and local pride, never of individual people or groups in a mean way. Lederhosen and Dirndl are worn by everybody, including the enemies.

Language: all in-game text is German with Austrian flavour ("Servus", "Grüß Gott", "Jause", "Hackl", "Bim"). See `.claude/skills/german-satire-writing/`.

## Core gameplay (Super Mario Bros. style)

- Side-scrolling from left to right; the camera never scrolls back left (like SMB).
- Run, jump with variable height (hold to jump higher), stomp enemies from above.
- Collect **Zauner Oblaten** (the coins). 100 Oblaten = 1 extra life.
- Hit **Oblatenkisten** (the "?" blocks) from below to release Oblaten or power-ups.
- Lives, score, countdown timer, checkpoint halfway through each stage.
- Each stage ends at a **Maibaum** (maypole) instead of a flagpole: the higher you grab it, the more points.

### Difficulty: medium

Precise SMB-style platforming, but without SMB's harsh punishment:
- 3 lives at the start; extra lives from 100 Oblaten and Zaunerstollen.
- One checkpoint per stage; dying respawns there.
- Game over → „Weiter?" restarts the **current stage** with 3 lives (unlimited continues; the score is reset).
- The player's hurtbox is a bit smaller than the sprite, and stomps are generous (landing on an enemy's upper half counts).
- No deaths the player can't see coming: hazards are visible before the jump that leads to them.
- Jump windows (measured by the solver): ≥ 6 frames in stages 1–3, ≥ 4 frames later. Optional bonus routes may be harder.

### Level-design reach rules (stages 1–3)

Derived from the measured movement (below) so required jumps keep their ≥ 6-frame windows. The solver report is the final judge; these rules keep designers from fighting it.

| Jump | Reach (flat, ground to ground) | Required in stages 1–3 | Optional / bonus only |
|---|---|---|---|
| Walking jump, held | ~5 tiles across, ~4 tiles up | gaps ≤ 4 tiles, step-ups ≤ 3 tiles | – |
| Running jump, held | ~9 tiles across, ~5 tiles up | gaps 5–6 tiles only with ≥ 8 tiles of flat run-up | gaps 7–8, step-ups of 4–5 tiles |
| Tap jump | ~1.5 tiles up | 1-tile steps | – |

- No ducking (see "Power states"): every required passage is ≥ 2 tiles high, so Zuckerschock Resi (hitbox 30 px) fits everywhere.
- The camera shows ~11 tiles ahead of the player (`follow_x = 144`), so every required jump's landing spot is on screen at take-off; don't place enemies so that they drop or fly into a landing spot that wasn't visible at take-off.
- No required jump onto a 1-tile-wide platform in stage 1.

### Power states

| State | Trigger | Effect |
|---|---|---|
| Normal | start | dies on hit |
| **Zuckerschock** (big) | eat a **Mozartkugel** | taller, breaks bricks, survives one hit |
| **Hackl** (thrower) | grab **St. Wolfgang's Hackl** (introduced in the Falkenstein stage, after the legend of the axe throw) | throws axes in an arc |
| 1-Up | **Zaunerstollen** (rare) | extra life |

- Physics are the same in both sizes; only the hitbox differs: normal 12×22 px, Zuckerschock **12×30 px** (sprite 16×32). Growing keeps the feet in place.
- **No ducking** in M1: DOWN only matters for one-way drop-through (DOWN + JUMP, D-011). Levels respect this (≥ 2-tile passages).
- **Hurt**: Zuckerschock → Normal plus **120 invulnerability frames** (2 s; sprite blinks every 4 frames). Enemies and projectiles can't hurt during them, but pits, water and time-up still kill.
- **Transformation**: growing or shrinking freezes the game for 30 frames while the sprite alternates between sizes every 4 frames. The timer doesn't run during the freeze.
- **Death**: a hit as Normal, water, falling out of the level, or time-up. Lives −1; then the stage intro screen (with lives) and a respawn at the start or the checkpoint, always as Normal.

### Movement feel (measured in M0, accepted for M1, tunable in `content/tuning.toml`)

| Metric | Target | Measured (M0) |
|---|---|---|
| Walk speed | ~1.5 px/frame | 1.5 |
| Run speed | ~2.5 px/frame | 2.5 |
| Jump height, tap | ~1.5 tiles | ~25 px |
| Jump height, hold, walking | ~4 tiles | ~65 px |
| Jump height, hold, running | ~5 tiles | ~81 px |
| Jump reach, walking, held (flat, ground to ground) | ~5 tiles (±0.5) | – (add test in M1) |
| Jump reach, running, held (flat, ground to ground) | ~9 tiles (±0.5) | ~9 tiles |
| Coyote time | 6 frames | 6 |
| Jump buffer | 6 frames | 6 |
| Max fall speed | ~4 px/frame | 4.0 |
| Stomp bounce, JUMP not held (new in M1) | rise ~1 tile (16 ± 4 px) | – |
| Stomp bounce, JUMP held (new in M1) | rise ~3 tiles (48–56 px) | – |
| Hurt invulnerability (new in M1) | 120 frames | – |

These targets are automated game-feel tests (`validation.md`, `crates/jnr-core/src/tests/feel.rs`). The M0 values are close to Super Mario Bros. (walk ~1.5, run ~2.5 px/frame, max fall 4 px/frame, running jump ~5 tiles high and ~9 tiles far), which is what we want; the long running jump is a ceiling for optional routes, not a gap size for required jumps (see "Level-design reach rules"). The stomp bounce gets its own tuning keys in M1; the engine picks values that hit the targets above.

## The journey: stages

One stage per station for now; more stages per station can be added later. An illustrated route map with the Wolfgangsee is shown between stages.

| # | Stage | Setting & mechanics | Enemies / tropes |
|---|---|---|---|
| 1 | **Salzburg** | Mirabellgarten, Makartsteg love locks, Getreidegasse with wrought-iron guild signs as platforms, Domplatz with the Jedermann stage, Festungsbahn funicular | Selfie tourists, Sound-of-Music tour groups, Fiaker, Mozartkugel sellers, pigeons |
| 2 | **St. Gilgen** | Lakeside promenade, Mozart's mother's birthplace, Zwölferhorn cable car (moving platforms) | Wanderers with Nordic-walking poles, e-bikers, Blasmusik (shockwave notes) |
| 3 | **Brunnwinkl** | Small hamlet; nod to bee researcher Karl von Frisch: beehives, bee dance | Bee swarms that follow the "waggle dance" pattern |
| 4 | **Falkenstein** | Pilgrim path up the Falkensteinwand, the hermitage church, the *Durchkriechstein* crawl-through (shed your sins = lose your power-up or gain a bonus). Get the **Hackl** | Pilgrim groups, falling rocks, chamois |
| 5 | **St. Wolfgang** | Pilgrimage church, "Im Weißen Rößl" hotel, Schafbergbahn cog railway | Operetta singers, souvenir stands, tourist buses |
| 6 | **Wolfgangsee** | Auto-scrolling lake stage: jump between **Traundl** boats, sailing boats, the paddle steamer; swim sections | Aggressive swans, regatta sailors, water skiers, Gelsen (mosquitoes) |
| 7 | **Strobl** | "The town everybody only drives through": traffic jams as platforms, roundabouts | Traffic, holiday-flat landlords, cyclists |
| 8 | **Bad Ischl** | Kaiservilla, Esplanade, salt (Salzkammergut = salt), spa town | Kurgäste in bathrobes, k.u.k. officers |
| 9 | **Café Zauner** (finale) | Short final stage inside the café: cake displays, waiters with trays; ends with the hero getting the Zaunerstollen | Final boss: **the queue** (or the head waiter) |

## Stage 1: Salzburg (Milestone 1 scope)

Length **~260 columns** (~4160 px, roughly 90–150 s for a good run), 14 rows high (ground top at row 12, rows 12–13 ground or water). `time_limit = 300` (generous on purpose for the first stage). The route follows the real walk from the main station through the old town: Südtiroler Platz (Postbus stops) → Mirabellgarten → Makartsteg → Hanuschplatz (big bus hub) → Getreidegasse → Residenzplatz → Domplatz → Kapitelplatz → Festungsbahn.

### Sections

| # | Section | Columns | New mechanic (introduced safely) | Enemies | Landmarks / jokes |
|---|---|---|---|---|---|
| 1 | **Busbahnhof** | 0–29 | walk, run, jump; Oblaten; first Oblatenkiste and bricks over flat ground | none | Postbus stop with a crowd of tourists (background), sign „Bus 150 / leider voll" |
| 2 | **Mirabellgarten** | 30–79 | stomping; Mozartkugel power-up; breaking bricks as Zuckerschock | Selfie-Tourist ×4 | hedges (Heckentheater) as solid platforms, Pegasus fountain, Zwergerlgarten dwarf statues as 1-tile platforms, sign „Do-Re-Mi / Fotopunkt" |
| 3 | **Makartsteg** | 80–124 | pits / water death; pigeons | Taube ×4 | bridge railings covered in love locks; gaps are where railing panels "fell off from too much love"; sign „Liebe max. / 2 Tonnen" |
| 4 | **Getreidegasse** | 125–179 | one-way Zunftschilder (upper route), checkpoint, thrown projectiles | Selfie-Tourist ×4, Mozartkugel-Verkäufer ×2 | checkpoint bus stop at Hanuschplatz (col 128, sign „Bus 150 / auch voll"), wrought-iron guild signs (one shaped like a burger), Mozart's birthplace (yellow façade, „Mozarts / Geburtshaus"), stand „Ur-Original / Mozartkugeln" |
| 5 | **Domplatz → Festung** | 180–259 | charging non-stompable enemy; moving platform (funicular); Maibaum height bonus | Fiaker ×1, Selfie-Tourist ×3, Taube ×2 | Residenzplatz with the Fiaker stand and Residenzbrunnen; Dombögen arches; Jedermann stage and grandstand („Jedermann / ausverkauft"); Kapitelplatz with the giant gold ball *Sphaera* („Keine / Mozartkugel"); Festungsbahn („Festungsbahn / seit 1892"); Maibaum with Wurstkranz; Ortsende sign „Salzburg" (red diagonal bar) |

Section details (for the level-designer; exact tiles are theirs to choose within these rules):

1. **Busbahnhof (0–29)**: no way to die except the timer. A row of 5 Oblaten at walking height, then a `?`-Oblatenkiste (Oblate) 4 tiles above the ground with bricks either side, then a 2-tile and a 3-tile step to teach the held jump.
2. **Mirabellgarten (30–79)**: the first Selfie-Tourist walks towards the player on flat ground with ≥ 6 free tiles on both sides. Right after, an `M`-Oblatenkiste (Mozartkugel) with a hedge wall 6 tiles to its right so the rolling Mozartkugel turns back and can't be lost. Then bricks with Oblaten behind them (only reachable as Zuckerschock, or by an optional jump over the hedge). Pegasus fountain: basin rim 2 tiles high, statue plinth 4 tiles high with an Oblaten arc above. A pair of Selfie-Tourists on the hedges, one more on the ground. Zwergerlgarten: 4 dwarf statues (1×1 tile, optional bonus Oblaten). **No pits.**
3. **Makartsteg (80–124)**: 3 tiles of ground before the bridge, deck at ground level, Salzach water below (rows 12–13 = water under gaps). Gap sequence: 2, 3, 3, 4 tiles (all walking-jump gaps). The first Taube flies at head height over solid deck (no gap below) so the player meets it safely. Later pigeons fly across gaps above the jump arc; stomping them is an optional shortcut, never required. One optional 6-tile gap with an Oblaten arc on a raised side railing (running jump, bonus route).
4. **Getreidegasse (125–179)**: checkpoint at col 128 (Hanuschplatz), followed by an `M`-Oblatenkiste within 10 columns (after a respawn the player is Normal again). Street level: 4 Selfie-Tourists in pairs. Upper route: Zunftschilder (one-way) at rows 8 and 5 forming a staircase above the crowd; one sign has Oblaten below it, reachable only by drop-through (DOWN + JUMP), which is optional. Mozartkugel-Verkäufer #1 at street level in front of Mozart's birthplace, #2 on a stand roof 3 tiles up; their arcs must leave a ≥ 6-frame gap the solver can measure.
5. **Domplatz → Festung (180–259)**:
   - Residenzplatz (180–204): flat, open square. The Fiaker waits at col ~200. The Residenzbrunnen (3 tiles high) at col ~192 is a safe place to let it pass.
   - Dombögen + Domplatz (205–229): arches (ceiling ≥ 4 tiles high), Jedermann stage (2-tile wooden platform), grandstand tiers as a staircase with Selfie-Tourists.
   - Kapitelplatz (230–237): Sphaera as a 2×2 solid block with the statue on top as a 1×1 platform; giant chess pieces as optional platforms.
   - Festungsbahn (238–247): the funicular car rides diagonally from ground level up **5 tiles** over ~8 columns onto the ramparts. Solid rock (stair-stepped) below the whole track, so falling off means waiting for the car, never death. ≥ 3 tiles of free space above the track. Two Tauben on the ramparts.
   - Maibaum (248–259): the rampart edge (5 tiles above the meadow) at col 248, the Maibaum at col **252** on the meadow, the Ortsende sign at col 255, the level ends at 259. Jumping off the rampart edge towards the pole lets good jumps reach the Wurstkranz; just walking off still touches the pole lower down.

### Entities and behaviour

All speeds in px/frame at 60 Hz. "Activates" = within one screen of the camera (architecture.md); enemies start moving towards the player (left). All patterns are deterministic (frame counters from activation, no RNG), so the solver and replays see the same thing every time.

| Entity | Sprite / hitbox | Behaviour | Stomp? | Points |
|---|---|---|---|---|
| **Selfie-Tourist** (`selfie_tourist`) | 16×24 / 12×20 | Walks 0.5 left; turns at walls and other enemies; walks off ledges. Every 240 frames (from activation) stops for 48 frames to take a selfie: selfie stick up, 2-frame white flash at frame 24 (harmless). Dangerous from the side all the time. | yes | 100 |
| **Taube** (`taube`) | 16×16 / 12×10 | Flies left at 0.75, sine wave amplitude 16 px, period 120 frames; ignores tiles; despawns off the left edge. Variant `taube_sitting`: sits until the player is within 6 tiles horizontally, then rises 16 px over 16 frames and flies like `taube`. On stomp it tumbles down off screen (no collision). | yes | 100 |
| **Mozartkugel-Verkäufer** (`mozartkugel_verkaeufer`) | 16×24 / 12×22 | Stands still, faces the player. While the player is within 10 tiles horizontally: every 120 frames a 20-frame wind-up (arm raised, clearly visible), then throws. | yes | 200 |
| ↳ thrown Mozartkugel (`mozartkugel_thrown`) | 8×8 / 6×6 | Fixed arc (not aimed): vx ±1.25 towards the player's side, vy −3.0, gravity 0.125/frame² (apex ~36 px, ~4 tiles range at throw height); keeps falling until it hits a solid tile or leaves the level, then vanishes. **Unwrapped** brown chocolate ball so it can't be mistaken for the wrapped power-up. | no (hurts) | – |
| **Fiaker** (`fiaker`) | 48×32 / 44×28 | Parked and visible. When the player is within 8 tiles horizontally: 40-frame telegraph (horse rears, carriage shakes), then charges left at 2.0 until it leaves the screen (never turns). Touching it from above (falling, feet above its hitbox middle) gives a normal stomp bounce but **no damage and no points**; any other contact hurts. | no | – |
| **Mozartkugel** power-up (`mozartkugel`) | 12×12 / 10×10 | Rises out of its box over 32 frames (0.5/frame), then rolls right at 1.0, uses the player's gravity and max fall speed, turns at walls, falls off ledges (lost in water). Touch: Normal → Zuckerschock. Already Zuckerschock: points only. | – | 1000 |
| **Zaunerstollen** 1-Up (`zaunerstollen`, optional in M1) | 16×12 / 14×10 | Like the Mozartkugel but rolls at 1.25. Touch: +1 life, popup „+1 Leben". | – | – |
| **Oblate** (`oblate`) | 12×12 / 10×10 | Static, collected on overlap. From a box: pops up 2 tiles and is collected automatically. 100 Oblaten → +1 life, counter back to 0. | – | 200 |
| **Oblatenkiste** (tile `oblaten_box`, `contains` = `oblate` / `mozartkugel` / `zaunerstollen`) | 16×16 tile | Hit from below (either size): bumps up 4 px for 8 frames, releases its content once, then becomes the solid empty box `oblaten_box_empty`. | – | – |
| **Brick** (tile `brick`, optional `contains = "oblaten_10"`) | 16×16 tile | Hit from below as Normal: bumps (4 px, 8 frames). As Zuckerschock: breaks into 4 debris particles. `oblaten_10`: one Oblate per hit, up to 10 or until 240 frames after the first hit, then empty box (doesn't break). | – | 50 (break) |
| **Checkpoint** (`checkpoint`, Haltestelle sign) | 16×32 / column | Round bus-stop sign (yellow disc, green rim, green „H"), with a timetable board below. Activates when the player's centre passes its x (any height). Then the timetable board shows „Bus 150 / auch voll" and the popup „Zwischenstopp" appears. | – | – |
| **Maibaum** goal (`goal_maibaum`) | pole 16×144 + top | Peeled spruce trunk, green wreath with red-white ribbons, **Wurstkranz** (sausage wreath) hanging at the top. Trigger: its whole column (D-011). | – | by height |
| **Sign** (`sign`, `text = <key>`) | board sprite by style | Background decoration, no collision. Text from `de.toml`, ≤ 2 lines of ≤ 12 characters, drawn centred on the board. Styles: `street` (default), `bus`, `ortsende` (white board, black text, red diagonal bar drawn over it). | – | – |
| **Festungsbahn car** (`festungsbahn`) | 48×24 / top surface 48 px wide | Moves along a straight diagonal between two end points (set in the level), travel 150 frames each way, waits 60 frames at each end, starts at the bottom. Solid from above only (one-way top). Carries a standing player by its own per-frame movement. | – | – |

**Stomping** (all stompable enemies): a stomp happens when the player is falling (vy > 0) and the player's feet were above the enemy hitbox's vertical middle on the previous frame (generous, "upper half counts"). Several enemies in the same frame are all stomped. Any other contact with an enemy or projectile hurts. Bumping a brick or box from below defeats an enemy standing on it (100 points).

**Stomp chain**: stomps without touching the ground score 100, 200, 400, 800, 1000, 2000, 4000, 8000, then +1 life for each further stomp (Mozartkugel-Verkäufer as the first stomp of a chain starts at 200). The chain resets on landing.

### Scoring, lives, timer, checkpoint

| Thing | Value |
|---|---|
| Oblate | 200 points, +1 Oblate (100 → +1 life) |
| Stomp | chain above |
| Brick broken | 50 |
| Power-up eaten | 1000 |
| Time bonus at the goal | 50 per remaining second (tallied 1 s per frame) |
| Maibaum (player's feet height h above the pole base at first touch) | h < 2 tiles: 100 · 2–4: 400 · 4–6: 800 · 6–8: 2000 · ≥ 8 tiles (Wurstkranz): **5000** + popup „Wurstkranz!" |

- **Lives**: start with 3, max 99. Score popups (small numbers) float up 16 px over 30 frames above whatever scored.
- **Timer**: `time_limit` seconds, counts down every 60 frames during Playing only. At ≤ 60 the ZEIT value turns red. At 0 the player dies; „Zeit is' um!" is shown during the death.
- **Checkpoint respawn**: after a death the whole level is reset to its initial state (enemies, boxes, bricks, Oblaten, like SMB); only the checkpoint flag, score, Oblaten counter and lives survive. The timer is reset to `time_limit`. The player respawns on the ground at the checkpoint (or the start) as Normal.
- **Game over** → „Weiter?": START restarts the current stage from its start (not the checkpoint) with 3 lives, score 0, Oblaten 0.

### Goal sequence

Touch the Maibaum → enemies freeze, timer stops, height score popup → Resi slides down the pole at 2 px/frame → at the base she hops off and walks right at walk speed past the Ortsende sign and off screen → time bonus tally → stage-clear text („Etappe geschafft!" + „Weiter nach St. Gilgen.") for 180 frames. In M1 (only one stage) this is followed by „Fortsetzung folgt …" for 180 frames, then the title screen. From M2 on, the route map follows instead.

### Screens and HUD (320×180, 8 px font, line height 9)

- **HUD** (Playing, Pause, Dying, StageClear): two rows at the top, labels at y = 4 in white `w`, values at y = 13 in `Y`; columns left-aligned at x = 16 (PUNKTE, 6 digits zero-padded), x = 96 (OBLATEN, 2 digits; with the `hud_oblate` icon: icon at x = 96, digits at x = 106), x = 184 (LEBEN), x = 256 (ZEIT, 3 digits; red `r` at ≤ 60). No background panel (the font's shadow is enough); levels keep the top 2 tiles free of important action.
- **Title**: Salzburg parallax background (static) with Resi idle on the ground at the left. Logo sprite `title_logo` (pixel-artist, ≤ 240×48, centred at y ≈ 24; fallback: `title.name` as text), subtitle `title.subtitle` at y ≈ 84, prompt `title.press_start` at y = 128 blinking (30 frames on, 30 off). START starts the stage intro.
- **Stage intro** (150 frames, also after every death): navy `B` background, `stage.salzburg.title` centred at y = 64, `stage.salzburg.tagline` at y = 80 in `i`, Resi's idle sprite and „× N" lives at y ≈ 112.
- **Pause** (nice-to-have in M1): START during Playing; level frozen behind a `B` panel (x 112–207, y 78–101, `k` border, 1 px `S` highlight top and left), `ui.pause` in `w` centred at y = 86.
- **Game over**: navy background, `ui.game_over` at y = 72; after 60 frames `ui.continue` at y = 96, blinking.
- **Stage clear**: texts centred over the level at y = 72 and 84 (`stage.clear`, `stage.salzburg.next`), time bonus line `stage.time_bonus` + number at y = 100 during the tally.

### Required for M1

Player (Normal + Zuckerschock, hurt, death), Oblaten, Oblatenkiste, bricks, one-way Zunftschilder, Salzach water death, 4 enemy types (Selfie-Tourist, Taube incl. sitting variant, Mozartkugel-Verkäufer with projectile, Fiaker), Festungsbahn moving platform, signs, checkpoint, Maibaum goal with height score, HUD, title, stage intro, game over, stage clear. Optional in M1: Zaunerstollen, `oblaten_10` brick, pause. Stretch: the **Sound-of-Music tour group** (a guide with an umbrella walks at 0.5; 3 tourists follow her exact path 20 frames apart; stomping the guide turns the followers into ordinary Selfie-Tourists).

### New mechanics: purpose, safe introduction, test

| Mechanic | Purpose | Introduced safely | Test |
|---|---|---|---|
| Stomp + bounce | core SMB verb | lone Selfie-Tourist on flat ground, sec. 2 | feel tests (bounce heights); replay `salzburg_stomp` (stomp, chain, side hit) |
| Zuckerschock / hurt | survive one hit, break bricks | Mozartkugel box with a wall that stops it, sec. 2 | replay: eat, break brick, get hit (i-frames), shrink; feel test: i-frames = 120 |
| Water / pits | the main danger | first gap 2 tiles, visible, sec. 3 | replay: fall into the Salzach → death + respawn |
| Taube | moving air obstacle | first one over solid deck | solver jump windows ≥ 6 on the Makartsteg |
| One-way drop-through | optional route | never required in stage 1 | replay: drop through a Zunftschild to the Oblaten |
| Thrown Mozartkugel | timing obstacle | wind-up telegraph, fixed arc | solver windows ≥ 6; replay: hit by a projectile as Zuckerschock |
| Fiaker | dodge instead of stomp | visible and parked before the telegraph, fountain to hide on | replay: jump over it; replay: bounce off its top without damage |
| Festungsbahn | moving platform | rock below the track, no death possible | replay: ride up; unit test: the car carries the player exactly |
| Checkpoint | medium difficulty | automatic | replay: die after it → respawn there, level reset, timer full |
| Maibaum score | skill bonus at the end | can't be missed (whole column) | unit test of the height zones; replay reaching the Wurstkranz |
| Timer | pressure | 300 s, red at ≤ 60 | replay waiting out the timer → death |

## UI text (German)

All keys live in `content/text/de.toml` (D-006). UI lines ≤ 24 characters, signs ≤ 2 lines of ≤ 12 characters, only characters of the font's charset (D-012; no `°`, `€`, `&`, `*`).

| Key | Text |
|---|---|
| `title.name` | Oblatenjagd |
| `title.subtitle` | Salzburg – Bad Ischl |
| `title.press_start` | Drück START, oida |
| `hud.score` / `hud.oblaten` / `hud.lives` / `hud.time` | PUNKTE / OBLATEN / LEBEN / ZEIT |
| `ui.pause` | Jausenpause |
| `ui.game_over` | Aus is'! Spiel vorbei. |
| `ui.continue` | Weiter? Drück START |
| `ui.time_up` | Zeit is' um! |
| `ui.to_be_continued` | Fortsetzung folgt … |
| `popup.extra_life` | +1 Leben |
| `popup.checkpoint` | Zwischenstopp |
| `popup.wurstkranz` | Wurstkranz! |
| `stage.clear` | Etappe geschafft! |
| `stage.time_bonus` | Zeitbonus |
| `stage.salzburg.title` | Etappe 1: Salzburg |
| `stage.salzburg.tagline` | Bus 150? Voll. Zu Fuß! |
| `stage.salzburg.next` | Weiter nach St. Gilgen. |
| `stage.salzburg.sign_*` | the sign texts in the section table (`sign_bus`, `sign_mirabell`, `sign_makartsteg`, `sign_checkpoint`, `sign_geburtshaus`, `sign_stand`, `sign_jedermann`, `sign_sphaera`, `sign_festungsbahn`, `sign_ortsende`) |

## Sources (facts used in Stage 1)

- Festungsbahn: opened 1 August 1892, Austria's oldest funicular still running, two counter-running cars – [salzburg.info](https://salzburg.info/de/reiseinfos/anreise-verkehr/festungsbahn), [Salzburg24](https://www.salzburg24.at/news/salzburg/stadt/festungsbahn-besteht-seit-120-jahren-art-189911)
- Love locks on the Makartsteg (officially Marko-Feingold-Steg): the city regularly cuts off locks, about 1.5 t a year, one round ~2 t, because they strain the railing netting – [Salzburg24](https://www.salzburg24.at/news/salzburg/stadt/15000-liebesschloesser-in-der-stadt-salzburg-muessen-weichen-art-274322), [Salzburg24](https://www.salzburg24.at/news/salzburg/stadt/viele-liebesschloesser-am-feingold-steg-werden-abgezwickt-art-318455)
- Sphaera (Stephan Balkenhol, 2007), gold ball with a man on top on Kapitelplatz, popularly called the "Balkenhol-Mozartkugel"; giant chess on the square – [salzburg.info](https://salzburg.info/de/salzburg/creative-salzburg/kunst-im-oeffentlichen-raum/walk-of-modern-art/sphaera), [salzburg.info Kapitelplatz](https://salzburg.info/de/sehenswertes/plaetze-strassen/kapitelplatz-kapitelschwemme)
- Maibaumkraxeln: climbing the (often soaped) Maibaum to the Wurstkranz at the top – [salzburg.info](https://www.salzburg.info/de/magazin/kunst-kultur/Von-Maibaumkraxlern-und-Maibaumdieben_a_282454)
- Austrian bus-stop sign: yellow disc, green rim, green „H" (§ 2 KflG-DV) – [jusline](https://www.jusline.at/paragraphs/history/456477369)
- Mozartkugel naming dispute (Fürst's „Original" vs. other makers' „Echte"), behind the „Ur-Original" stand sign – [Wikipedia](https://en.wikipedia.org/wiki/Mozartkugel)
- Common knowledge (no source needed): Pegasus fountain and Mirabellgarten as Sound of Music "Do-Re-Mi" location, Zwergerlgarten, Mozart's birthplace at Getreidegasse 9, Jedermann on Domplatz, Fiaker at Residenzplatz, Hanuschplatz bus hub at the end of the Makartsteg.

## Resolved decisions (human, 2026-10-18)

1. **Title**: „Oblatenjagd"
2. **Hero**: Resi in a Dirndl (one playable character)
3. **Real names**: real places, businesses and historical people (Café Zauner, Postbus, Fürst Mozartkugeln, Kaiser Franz Joseph, …) may be used by name – it's a private project.
4. **Difficulty**: medium (see "Difficulty: medium" above)
