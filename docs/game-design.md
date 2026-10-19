# Game design

Status: **approved v1**. Title: **„Oblatenjagd"**.

## Premise

The hero, **Resi** – a Salzburg local in a Dirndl – wants one thing: a fresh slice of Zaunerstollen at **Café Zauner in Bad Ischl**. But Postbus 150 (Salzburg → Bad Ischl, a real line) is hopelessly full of tourists, so the hero sets off on foot along the old route of the *Ischlerbahn* (the Salzkammergut-Lokalbahn, closed 1957). On the way: tour groups, Fiaker, pilgrims, swans, regatta sailors and the entire Salzkammergut cliché machine.

Tone: affectionate satire. We make fun of tourism, kitsch, Austrian bureaucracy and local pride, never of individual people or groups in a mean way. Lederhosen and Dirndl are worn by everybody, including the enemies.

Language: all in-game text is German with Austrian flavour ("Servus", "Grüß Gott", "Jause", "Hackl", "Bim"). See `.claude/skills/german-satire-writing/`.

## Core gameplay (Super Mario Bros. style)

- Side-scrolling from left to right; the camera never scrolls back left (like SMB). Only exception: a moving platform carrying Resi left past the screen edge takes the camera along (D-051).
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
- Stages 2–3 ramp gently (D-041): the same reach rules and `min_jump_window = 6`; required landings ≥ 2 tiles wide in stage 2 (a gondola roof is 2 tiles), 1-tile landings in stage 3 only over non-lethal ground; **at most one moving hazard** (Taube, note train, bee swarm, charger) acts on any required jump; no enemy reaches a moving platform's track while it can be ridden (D-032 extended to gondolas) or the jump to the Maibaum; Tauben placed by the meeting rule (D-032), what is drawn as solid is solid (D-038).

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
| 2 | **St. Gilgen** | Busbahnhof, Zwölferhorn cable car (free-running gondolas), Mozartplatz, Mozarthaus (Mozart's mother's birthplace), lakeside promenade with bandstand, boat pier (M2 spec below) | Nordic-Walker (turn at ledges), Blasmusiker (note trains), Selfie-Tourists, Tauben, one Mozartkugel-Verkäufer |
| 3 | **Brunnwinkl** | Hamlet of five houses; Karl von Frisch's family estate: old mill, bee house, bathing jetty (M2 spec below) | Bee swarms dancing the waggle-dance figure-eight (the dance points to a bonus), e-bikers on the shore path, Nordic-Walkers |
| 4 | **Falkenstein** | Pilgrim path up the Falkensteinwand, the hermitage church, the *Durchkriechstein* crawl-through (shed your sins = lose your power-up or gain a bonus). Get the **Hackl** | Pilgrim groups, falling rocks, chamois |
| 5 | **St. Wolfgang** | Pilgrimage church, "Im Weißen Rößl" hotel, Schafbergbahn cog railway | Operetta singers, souvenir stands, tourist buses |
| 6 | **Wolfgangsee** | Auto-scrolling lake stage: jump between **Traundl** boats, sailing boats, the paddle steamer; swim sections | Aggressive swans, regatta sailors, water skiers, Gelsen (mosquitoes) |
| 7 | **Strobl** | "The town everybody only drives through": traffic jams as platforms, roundabouts | Traffic, holiday-flat landlords, cyclists |
| 8 | **Bad Ischl** | Kaiservilla, Esplanade, salt (Salzkammergut = salt), spa town | Kurgäste in bathrobes, k.u.k. officers |
| 9 | **Café Zauner** (finale) | Short final stage inside the café: cake displays, waiters with trays; ends with the hero getting the Zaunerstollen | Final boss: **the queue** (or the head waiter) |

## Stage 1: Salzburg (Milestone 1 scope)

Length **262 columns** (~4190 px, D-031; measured at M1 acceptance: the solver finishes in 1601 frames ≈ 27 s, a relaxed full run takes ~65 s, explorers longer; 81 Oblaten in the stage), 17 rows high (3 empty rows on top for the goal camera, D-030; the street's top is row 15, rows 15–16 are ground or water). `time_limit = 300` (generous on purpose for the first stage). The route follows the real walk from the main station through the old town: Südtiroler Platz (Postbus stops) → Mirabellgarten → Makartsteg → Hanuschplatz (big bus hub) → Getreidegasse → Residenzplatz → Domplatz → Kapitelplatz → Festungsbahn.

### Sections

| # | Section | Columns | New mechanic (introduced safely) | Enemies | Landmarks / jokes |
|---|---|---|---|---|---|
| 1 | **Busbahnhof** | 0–29 | walk, run, jump; Oblaten; first Oblatenkiste and bricks over flat ground | none | Postbus stop sign „Bus 150 / leider voll" (style `bus`; a background crowd of waiting tourists is in the backlog) |
| 2 | **Mirabellgarten** | 30–79 | stomping; Mozartkugel power-up; breaking bricks as Zuckerschock | Selfie-Tourist ×4 | hedges (Heckentheater) as solid platforms, Pegasus fountain, Zwergerlgarten dwarf statues on 1-tile stones, sign „Do-Re-Mi / Fotopunkt" |
| 3 | **Makartsteg** | 80–124 | pits / water death; pigeons | Taube ×4 | bridge railings covered in love locks; gaps are where railing panels "fell off from too much love"; sign „Liebe max. / 2 Tonnen" |
| 4 | **Getreidegasse** | 125–179 | one-way Zunftschilder (upper route), checkpoint, thrown projectiles | Selfie-Tourist ×4, Mozartkugel-Verkäufer ×2 | checkpoint bus stop at Hanuschplatz (col 128) with a `bus` sign „Bus 150 / auch voll" beside it (D-039), wrought-iron guild signs (one shaped like a burger), Mozart's birthplace (yellow façade, „Mozarts / Geburtshaus"), stand „Ur-Original / Mozartkugeln" |
| 5 | **Domplatz → Festung** | 180–259 | charging non-stompable enemy; moving platform (funicular); Maibaum height bonus | Fiaker ×1, Selfie-Tourist ×3, Taube ×2 | Residenzplatz with the Fiaker stand and Residenzbrunnen; Dombögen arches; Jedermann stage and grandstand („Jedermann / ausverkauft"); Kapitelplatz with the giant gold ball *Sphaera* („Keine / Mozartkugel"); Festungsbahn („Festungsbahn / seit 1892"); Maibaum with Wurstkranz; Ortsende sign „Salzburg" (red diagonal bar) |

Section details (for the level-designer; exact tiles are theirs to choose within these rules):

1. **Busbahnhof (0–29)**: no way to die except the timer. A row of 5 Oblaten at walking height, then a `?`-Oblatenkiste (Oblate) 4 tiles above the ground with bricks either side, then a 2-tile and a 3-tile step to teach the held jump.
2. **Mirabellgarten (30–79)**: the first Selfie-Tourist (col 46) walks towards the player on flat ground with ≥ 6 free tiles on both sides. Above it, an `M`-Oblatenkiste (Mozartkugel, col 47) with a hedge wall 4 tiles to its right (hedge 51–55) so the rolling Mozartkugel turns back and can't be lost. Tourist #2 stands on the hedge top, walled in by its raised ends so it is still there when Resi arrives (D-038). Then a compact hedge planter (57–61) with a brick floor holding Oblaten and tourist #3: break in from below as Zuckerschock or jump in from the hedge (optional). Pegasus fountain (64–71): basin rim 2 tiles high, statue plinth 4 tiles high with an Oblaten arc above; the „Do-Re-Mi / Fotopunkt" sign stands right next to it, clear of the basin (the fountain is the real Do-Re-Mi filming spot, that's the joke). Zwergerlgarten: 4 dwarf statues on 1-tile stones (73/74/78/79, optional bonus Oblaten) with tourist #4 between them. **No pits.**
3. **Makartsteg (80–124)**: 3 tiles of ground before the bridge, deck at ground level, Salzach water below (rows 15–16 = water under gaps). Gap sequence: 2, 3, 3, 4 tiles (all walking-jump gaps). The first Taube flies at head height over solid deck (no gap below) so the player meets it safely. Tauben activate a screen ahead and drift left at 0.75 px/frame, so a walking or running player meets one roughly 7–11 columns left of its spawn column: place it accordingly (spawn ~col 97 for a meeting over the deck at ~86–90). Later pigeons fly across gaps above the jump arc (shipped: spawns 105/114/123 at row 6, above every jump arc including Zuckerschock jumps from the railing, D-038); stomping them is an optional shortcut, never required. One optional 6-tile gap with an Oblaten arc on a raised side railing (running jump, bonus route).
4. **Getreidegasse (125–179)**: checkpoint at col 128 (Hanuschplatz) with the „Bus 150 / auch voll" `bus` sign beside it (D-039), followed by an `M`-Oblatenkiste within 10 columns (shipped: col 132; after a respawn the player is Normal again). Street level: 4 Selfie-Tourists in pairs (138/141, kept out from under the Erker by a bench at 142; 150/155). Upper route: Zunftschilder (one-way) at rows 11 and 8 (3 and 6 tiles above the street) forming a staircase above the crowd; the burger sign (row 8, ≥ 1 tile of headroom, D-038) sits on top of an Erker (bay window) with Oblaten inside, reachable only by drop-through (DOWN + JUMP), which is optional. The houses are upper-floor façade blocks hanging into the screen from above (cols 125–128, 144–148, 166–169, 175–178). Mozartkugel-Verkäufer #1 at street level in front of Mozart's birthplace, #2 on a stand roof 3 tiles up; their arcs must leave a ≥ 6-frame gap the solver can measure. Keep ≥ 10 columns between them: a street-level throw lands ~4 tiles to the thrower's side, a roof throw ~5 tiles, so with less distance the two landing zones overlap into a crossfire right where the projectile is still new.
5. **Domplatz → Festung (180–259)**:
   - Residenzplatz (180–204): flat, open square. The Fiaker waits at col 200 (D-035). The Residenzbrunnen (190–194, all five columns solid at the drawn 3-tile height, D-038) is a safe place to let it pass: standing on it triggers the Fiaker, which charges through underneath.
   - Dombögen + Domplatz (205–229): arches (ceiling ≥ 4 tiles high), Jedermann stage (2-tile wooden platform) with the „Jedermann / ausverkauft" sign in front of it, grandstand tiers as a staircase with Selfie-Tourists. The Dombögen are a building bridging the street (the real arcades link Residenz and Dom and carry the DomQuartier museum route with its Dombogen terrace), so the block reaches up to the top of the screen (`facade`/`arch` above, `arch_under` on its lowest row, D-030) instead of floating as a slab in the sky.
   - Kapitelplatz (230–237): Sphaera as a 2×2 solid block (invisible tiles under the `sphaera` prop) with the statue on top as a 1×1 platform, „Keine / Mozartkugel" sign in front. (The giant chess pieces of the real square are not in M1, see backlog.) A `taube_sitting` sits on the statue's head (pigeons on monuments, the local classic); it rises when Resi reaches the grandstand and flies over solid ground.
   - Festungsbahn (238–247): the funicular car rides diagonally from ground level up **5 tiles** over ~8 columns onto the ramparts. It waits at its **lower** station until Resi stands on it (D-030), so the level spawns it there: in a shallow station trench (238–242) with the car's top 8 px above the street, so jumping on is the natural move; walking in drops Resi into the trench, whose stairs lead back up (D-038). Solid rock (stair-stepped) below the whole track, so falling off means waiting for the car or climbing back, never death. ≥ 3 tiles of free space above the track. **No enemy may reach the track or the rider during the ride** (the moving platform is introduced safely): the section's second Taube flies in over Kapitelplatz at grandstand head height (spawn col 229, row 9, D-038), so it has crossed the street before the ride starts (D-032).
   - Maibaum (248–261): the rampart edge (5 tiles above the street, 6 above the meadow) at col 248, the Maibaum at col **252** on the meadow (lowest ground, D-029), the Ortsende sign at col 255, the level ends at 261 (262 columns, D-031: the camera clamps at the goal). A good running held jump off the rampart edge or the Festungsbahn car reaches the Wurstkranz (feet ≥ 7 tiles above the pole base, D-038); a walking or standing jump off the car scores less, and just walking off the rampart touches the pole near its base (100). The jump to the pole stays free of enemies.

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
| **Checkpoint** (`checkpoint`, Haltestelle sign) | 16×32 / column | Round bus-stop sign (yellow disc, green rim, green „H"), with a small timetable board below (no text: 16 px is too narrow). Activates when the player's centre passes its x (any height): the disc turns active and the popup „Zwischenstopp" appears. The „Bus 150 / auch voll" punchline is a separate `bus` sign next to it, placed by the level (D-039). | – | – |
| **Maibaum** goal (`goal_maibaum`) | pole drawn up to the Wurstkranz zone (7 tiles, D-038) + top | Peeled spruce trunk, green wreath with red-white ribbons, **Wurstkranz** (sausage wreath) hanging at the top. Trigger: its whole column (D-011). | – | by height |
| **Sign** (`sign`, `text = <key>`) | board sprite by style | Background decoration, no collision. Text from `de.toml`, ≤ 2 lines of ≤ 12 characters, drawn centred on the board. Styles: `street` (default), `bus`, `ortsende` (white board, black text, red diagonal bar drawn over it). | – | – |
| **Festungsbahn car** (`festungsbahn`) | 48×24 / top surface 48 px wide | Moves along a straight diagonal between two end points (set in the level; levels spawn it at the lower station), travel 150 frames each way. Waits at the lower station until the player stands on it, rides up, waits 60 frames at the top, rides back down and waits for the next rider (D-030). Solid from above only (one-way top). Carries a standing player by its own per-frame movement. | – | – |

**Stomping** (all stompable enemies): a stomp happens when the player is falling (vy > 0) and the player's feet were above the enemy hitbox's vertical middle on the previous frame (generous, "upper half counts"). Several enemies in the same frame are all stomped. Any other contact with an enemy or projectile hurts. Bumping a brick or box from below defeats an enemy standing on it (100 points).

**Stomp chain**: stomps without touching the ground score 100, 200, 400, 800, 1000, 2000, 4000, 8000, then +1 life for each further stomp (Mozartkugel-Verkäufer as the first stomp of a chain starts at 200; from M2 every 200-point enemy does: Blasmusiker, E-Biker, D-046). The chain resets on landing.

### Scoring, lives, timer, checkpoint

| Thing | Value |
|---|---|
| Oblate | 200 points, +1 Oblate (100 → +1 life) |
| Stomp | chain above |
| Brick broken | 50 |
| Power-up eaten | 1000 |
| Time bonus at the goal | 50 per remaining second (tallied 1 s per frame) |
| Maibaum (player's feet height h above the pole base at first touch) | h < 2 tiles: 100 · 2–4: 400 · 4–6: 800 · 6–7: 2000 · ≥ 7 tiles (Wurstkranz): **5000** + popup „Wurstkranz!" |

- **Lives**: start with 3, max 99. Score popups (small numbers) float up 16 px over 30 frames above whatever scored.
- **Timer**: `time_limit` seconds, counts down every 60 frames during Playing only. At ≤ 60 the ZEIT value turns red. At 0 the player dies; „Zeit is' um!" is shown during the death.
- **Checkpoint respawn**: after a death the whole level is reset to its initial state (enemies, boxes, bricks, Oblaten, like SMB); only the checkpoint flag, score, Oblaten counter and lives survive. The timer is reset to `time_limit`. The player respawns on the ground at the checkpoint (or the start) as Normal.
- **Game over** → „Weiter?": START restarts the current stage from its start (not the checkpoint) with 3 lives, score 0, Oblaten 0.

### Goal sequence

Touch the Maibaum → enemies freeze, timer stops, height score popup → Resi slides down the pole at 2 px/frame → at the base she hops off and walks right at walk speed past the Ortsende sign and off screen → time bonus tally → stage-clear text („Etappe geschafft!" + „Weiter nach St. Gilgen.") for 180 frames. In M1 (only one stage) this is followed by „Fortsetzung folgt …" for 180 frames, then the title screen. From M2 on, the route map follows instead (see "Multi-stage flow" and "Route map"); the second stage-clear line is `stage.<id>.next` of the cleared stage.

### Screens and HUD (320×180, 8 px font, line height 9)

- **HUD** (Playing, Pause, Dying, StageClear): two rows at the top, labels at y = 4 in white `w`, values at y = 13 in `Y`; columns left-aligned at x = 16 (PUNKTE, 6 digits zero-padded), x = 96 (OBLATEN, 2 digits; with the `hud_oblate` icon: icon at x = 96, digits at x = 106), x = 184 (LEBEN), x = 256 (ZEIT, 3 digits; red `r` at ≤ 60). No background panel: labels and numbers have a full 1 px `k` outline (D-037); levels keep the top 2 tiles free of important action.
- **Title**: Salzburg parallax background (static) with Resi idle on the ground at the left. Logo sprite `title_logo` (pixel-artist, ≤ 240×48, centred at y ≈ 24; fallback: `title.name` as text), subtitle `title.subtitle` at y ≈ 84, prompt `title.press_start` at y = 128 blinking (30 frames on, 30 off). START starts the stage intro.
- **Stage intro** (150 frames, also after every death): navy `B` background, `stage.<id>.title` centred at y = 64, `stage.<id>.tagline` at y = 80 in `i`, Resi's idle sprite and „× N" lives at y ≈ 112 (from M2: `player_big` when she carries Zuckerschock into the stage, D-042).
- **Pause** (nice-to-have in M1): START during Playing; level frozen behind a `B` panel (x 112–207, y 78–101, `k` border, 1 px `S` highlight top and left), `ui.pause` in `w` centred at y = 86.
- **Game over**: navy background, `ui.game_over` at y = 72; after 60 frames `ui.continue` at y = 96, blinking.
- **Stage clear**: from the tally on, a navy panel (D-037: `B` fill, 1 px `k` border, `S` highlight top and left, 4 px padding; y 69–109, width from the widest line, centred) carries the texts centred at y = 74 and 84 (`stage.clear`, `stage.salzburg.next`) and the time bonus line `stage.time_bonus` + number (`Y`) at y = 96. „Zeit is' um!" (time up) stays at y = 72 without a panel.

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

## Multi-stage flow (Milestone 2)

The stage order is the route (D-042): `salzburg` → `st_gilgen` → `brunnwinkl` → `falkenstein` → `st_wolfgang` → `wolfgangsee` → `strobl` → `bad_ischl` → `cafe_zauner`. A station is playable when a level with that id ships; in M2 these are the first three (`01_salzburg`, `02_st_gilgen`, `03_brunnwinkl`).

| Thing | Into the next stage | After a death | Game over → „Weiter?" |
|---|---|---|---|
| Score | carries over | kept | 0 |
| Lives | carry over | −1 | 3 |
| Oblaten counter | carries over | kept | 0 |
| Power state | **carries over** (Zuckerschock stays, like SMB) | Normal | Normal |
| Timer | the new stage's `time_limit` | reset | reset |
| Checkpoint | cleared | kept | cleared |
| Stage | the next station | same stage | **same stage**, from its start (D-017) |

- **Title → START** always starts the route at Salzburg (no stage select, no saves in M2). The title screen itself is unchanged.
- **Stage clear → route map → intro of the next stage → play.** The Maibaum sequence runs as in Stage 1 up to the `Clear` phase; the route map replaces „Fortsetzung folgt …".
- **Last playable stage** (Brunnwinkl in M2; generally: the next station has no level yet): the route map still plays, Resi walks on to the next station (Falkenstein), and the panel shows „Fortsetzung folgt …" for 180 frames instead of the prompt; then the title, and the run is reset (score 0, 3 lives, Oblaten 0, Normal).
- **Game over in stage 2 or 3** restarts that stage (not Salzburg) with 3 lives, score 0, Oblaten 0, Normal. Unlimited continues, as in Stage 1.
- **Stage intro** for every stage: as in "Screens and HUD", with that stage's `stage.<id>.title` and `.tagline`; Resi drawn in her current power state.
- **Dev entry**: `--level <id>` starts the run at that stage (title → its intro) with the default carry-over values and continues along the route from there.
- **Replays and tests** (engine's choice, recommended): `start = title` replays follow the real flow across stages; `start = playing` runs keep ending in the title after the goal sequence as in M1, so the solver replays and the M1 replays keep their meaning.

## Route map (between stages)

Shown after every stage clear in the real flow; never after a death or a continue. Full screen 320×180, no HUD, no RNG.

**What it shows**
- Backdrop `map_salzkammergut` (hand-drawn, no text in the art): a stylised painted Wanderkarte of the Salzkammergut. Salzburg top left (a tiny Festung), the Fuschlsee, the Wolfgangsee large in the middle running from St. Gilgen (west end) to Strobl (east end), the Zwölferhorn south of St. Gilgen, the Schafberg above the north shore, Bad Ischl further east with a small cake for Café Zauner. The bottom band (y ≥ 136) stays calm because the panel sits there.
- **Route**: drawn by the engine along a waypoint polyline (route data), the whole route as small dots (1 px, every 4 px), the part already walked as bolder 2×2 dots. Colours: art-director's choice within the map's palette.
- **Stations**: `map_station` markers on the route, `todo` (not reached), `done` (cleared), `next` (blinking). Every station has its name label (`map.station.<id>`, 8 px font, full `k` outline): reached stations `w`, the next one `Y`, the future ones `i`. Label positions are per station (route data) so that no two labels overlap and each stays ≥ 4 px inside the screen.
- **Resi**: her normal `player` (or `player_big`) sprite standing on the station dot (`idle`) or walking along the route (`walk`, facing the direction of travel). No new sprite.
- **Panel** (D-037 style: `B` fill, `k` border, `S` highlight top and left, 4 px padding, centred, width from the widest line), y 138–175: `map.next_stop` „Nächster Halt:" in `w` at y 143, the next station's name in `Y` at y 153, and at y 165 either the blinking prompt `map.continue` „Weiter mit START" (30 frames on, 30 off) or `ui.to_be_continued` (teaser).

Starting layout (pixel-artist and engine may move dots and labels by a few px to fit the art; the final numbers live in the route data, checked by a golden that shows all labels):

| Station | Dot (x, y) | Label |
|---|---|---|
| Salzburg | 28, 36 | right of the dot |
| St. Gilgen | 100, 70 | left |
| Brunnwinkl | 124, 58 | above-left: right-aligned to x 114, rows 43–51 (art-director M2; clear of the Salzburg label and the Salzburg→St. Gilgen route) |
| Falkenstein | 152, 68 | below, ending at x ≤ 172 (clear of the St. Wolfgang marker) |
| St. Wolfgang | 180, 84 | right |
| Wolfgangsee (on the lake) | 196, 108 | below |
| Strobl | 232, 118 | right |
| Bad Ischl | 276, 98 | above (engine, E10: below touches the Strobl label) |
| Café Zauner | 300, 76 | left: right-aligned to x 290, rows 69–77 (art-director M2; clear of the St. Wolfgang and Bad Ischl labels) |

**Timing** (60 Hz; numbers go to `tuning.toml`, names are the engine's choice)

| Phase | Frames | What happens |
|---|---|---|
| Arrive | 40 | hard cut from the stage-clear panel; Resi idle on the cleared station; its marker turns `done` at frame 20 |
| Walk | path length in px (1 px/frame; ≈ 30–110) | Resi walks the waypoints to the next station; walked dots appear behind her; the panel is shown from the start of this phase |
| Wait | until START, at most 300 | Resi idle on the next station (`next` blinking), prompt blinking; START or the 300th frame → intro of the next stage |
| Teaser (next station has no level) | 180 | replaces Wait: line 3 shows „Fortsetzung folgt …", START is ignored; then the title |

**Inputs**: START (a new press) during Arrive or Walk skips to Wait with Resi on the next station; START during Wait continues. Everything else is ignored; JUMP is tracked as in every non-play mode (D-025), so a held JUMP is no new press in the next stage.

## Stage 2: St. Gilgen (Milestone 2 scope)

Length **280 columns** (~4480 px), 17 rows high with Stage 1's vertical layout (3 empty rows on top for the goal camera, the street's top is row 15, rows 15–16 ground or water; the goal meadow one row lower). `time_limit = 240` (provisional, D-045: set from the measured relaxed run at level acceptance). The route follows the real village from the bus station at the foot of the Zwölferhorn to the lake: Busbahnhof → Zwölferhorn-Talstation → Mozartplatz (Rathaus, Mozartbrunnen) → Ischler Straße with the Mozarthaus (the former Pfleggericht where Mozart's mother was born and Nannerl lived) → Seepromenade with the bandstand → Schiffsanlegestelle → Maibaum on the lake meadow. Background: sky (reuse `bg_sky_salzburg`), `bg_schafberg` across the lake, `bg_wolfgangsee` (lake + far shore), `bg_stgilgen` (village houses).

New in this stage: **free-running moving platforms** (Zwölferhorn gondolas), **an enemy that stays on its platform** (Nordic-Walker), **horizontal projectile trains** (Blasmusik notes). Reused: Selfie-Tourist, Taube (+ sitting), one Mozartkugel-Verkäufer, water death (Wolfgangsee), checkpoint, Maibaum.

### Sections

| # | Section | Columns | New mechanic (introduced safely) | Enemies | Landmarks / jokes |
|---|---|---|---|---|---|
| 1 | **Busbahnhof** | 0–34 | Mozartkugel early; Nordic-Walker (turns at ledges) | Nordic-Walker ×2 | `bus` sign „Bus 150 / 4x stündlich" (line 150 now runs up to four times an hour – and is still full); the old Ischlerbahn's buffer stop (`prellbock`) with „Ischlerbahn / bis 1957" |
| 2 | **Zwölferhorn-Talstation** | 35–94 | Gondel: free-running moving platforms | Selfie-Tourist ×1 | valley station, yellow cabins (the real colours), pylon; „Seilbahn / seit 1957" (the cable car opened the year the Ischlerbahn closed), „Dachfahrt / verboten" at the first cabin (Resi rides on the roof anyway) |
| 3 | **Mozartplatz** | 95–139 | walker on a raised platform + Verkäufer (combination) | Selfie-Tourist ×3, Nordic-Walker ×1, Mozartkugel-Verkäufer ×1, `taube_sitting` ×1 | Mozartbrunnen (young Mozart with his violin, 1926) with a pigeon on his head; sign „Hier wohnte / nie Mozart" (the „Mozartdorf" never had Mozart living in it) |
| 4 | **Ischler Straße** | 140–189 | checkpoint; upper route over balconies with patrolling walkers | Nordic-Walker ×2, Selfie-Tourist ×1 | checkpoint (col 142) with the `bus` sign „Bus 150 / alle 4 voll" beside it (D-039); Mozarthaus with „Mozarts Mama / geboren 1720" and „Hier wohnte / Nannerl" (the callback); wooden balconies with geraniums |
| 5 | **Seepromenade** | 190–234 | Blasmusik notes | Blasmusiker ×2, Selfie-Tourist ×1 | bandstand (Musikpavillon) with the Platzkonzert, „Platzkonzert / Zugabe droht", lake view |
| 6 | **Schiffsanlegestelle → Maibaum** | 235–279 | combination: jetty gaps with a walker guarding a jetty segment | Nordic-Walker ×1, Taube ×1 | wooden pier, „Linienschiff / auch voll" (the boat is full too), ticket kiosk roof as the launch to the Maibaum, Ortsende „St. Gilgen" |

Section details (for the level-designer; exact tiles are theirs within these rules; columns are targets):

1. **Busbahnhof (0–34)**: no pits. Start at col 2, the „Bus 150 / 4x stündlich" `bus` board at col 5 (right of the start, clear of Resi standing there; E12 art review). 5 Oblaten at walking height (6–10). `M`-Oblatenkiste at col 14, row 11 (4 tiles up) with bricks either side and a 2-tile bus-shelter wall (`stone`) at 18–19 so the rolling Mozartkugel turns back (Stage 1 pattern); a Resi who carried Zuckerschock over just scores it. The `prellbock` prop and the „Ischlerbahn / bis 1957" sign at 21–23 (decorative, street level). Nordic-Walker #1 at col 26 on flat street with ≥ 6 free tiles either side. A 1-tile-high flower planter (`stone`, 6 wide, 29–34) with Nordic-Walker #2 on it: it paces to the planter's edge and **turns** instead of stepping off (the mechanic, shown where it can't hurt: the planter is a 1-tile step, stomping from the street is easy).
2. **Zwölferhorn-Talstation (35–94)**:
   - Forecourt 35–42: `talstation` prop (back layer) behind 36–44, „Seilbahn / seit 1957" sign at 37, Selfie-Tourist at col 40 (a selfie in front of the cabins). Steps up (1 tile each) to the station platform (`stone`, top 2 tiles above the street, 43–46). „Dachfahrt / verboten" sign at 44, on the platform, clear of the cabin.
   - **Gondel A** (intro): spawns at col 47 with its roof ≤ 8 px above the platform (step or tap-jump on), `to = [6, -1]`. Under its whole track a **meadow trench 2 tiles deep with stairs at both ends** (47–54; the cabin is 2 tiles wide, so it ends over 53–54): falling off means climbing out, never death (D-027/D-028). A line of Oblaten 2 tiles above the roof along the track (collected while riding). It ends next to a meadow knoll (55–61, top level with A's upper end).
   - **Gondel B**: spawns at col 62, `to = [7, -2]`, over a **death ravine** (62–70, empty to the level bottom). It ends next to the pylon platform (`rock`, 71–73, `seilbahn_stuetze` prop behind it).
   - **Gondel C**: spawns at the pylon at col 74, `to = [6, 2]` (descending), over the ravine (74–81), ending at a ledge 82–85 (top 3 tiles above the street), then stairs down to the street (86–89). C's `phase` is set so that C waits at the pylon or arrives there within ≤ 2 s of B's arrival (the designer tunes it with the solver).
   - No Taube in this section: a Taube activated a screen ahead would drift into the tracks while Resi rides or waits (D-032). Keep ≥ 3 tiles free above every track (lint).
3. **Mozartplatz (95–139)**: flat cobbled square (no pits). Mozartbrunnen 100–104: basin rim 1 tile high (5 wide, solid `stone`), the statue plinth 3 tiles high and 1 wide at 102 (drawn = solid, D-038), the `mozartbrunnen` prop over it; a `taube_sitting` on Mozart's head (rises when Resi is within 6 tiles, flies over the square). „Hier wohnte / nie Mozart" sign at 97, left of the basin and not covering it. Selfie-Tourists at 108, 116, 124. Mozartkugel-Verkäufer at street level at col 120, in front of a souvenir stand (the Mozartdorf sells Mozartkugeln anyway); keep the tourists out of its landing zone in front of it where possible (it's still fine: thrown balls don't hurt enemies). A Gastgarten platform (`oneway_wood`, row 12 = 3 up, 131–137) with Nordic-Walker #3 patrolling it and a `?`-box 3 tiles above its middle (optional).
4. **Ischler Straße (140–189)**:
   - Checkpoint at col 142; the „Bus 150 / alle 4 voll" `bus` sign right of it (144–146) without hiding the checkpoint pole or the box; `M`-Oblatenkiste at col 150, row 11 (within 10 columns, as in Stage 1).
   - Village houses drawn **full height down to the street** with the new decorative back tile `wall_village_bg` (non-solid), so there are no floating façade blocks (M1 art note); `balcony` one-way tiles in front of them form the optional upper route: B1 row 11 at 154–158, B2 row 8 at 168–173, B3 row 11 at 178–182 (3 and 6 tiles up; a staircase as with the Zunftschilder). Nordic-Walker #4 on B1 and #5 on B2: they pace their balcony and never fall onto the street (the mechanic's payoff). Oblaten on B2.
   - Mozarthaus: `mozarthaus` prop (back layer) behind 160–167; „Mozarts Mama / geboren 1720" at 159, „Hier wohnte / Nannerl" at 168 (both at street level, clear of the balcony tiles).
   - Selfie-Tourist #5 on the street at 176. The street is always passable without the balconies.
5. **Seepromenade (190–234)**:
   - „Platzkonzert / Zugabe droht" sign at 193.
   - Musikpavillon: `oneway_wood` floor at row 13 (2 tiles up, 196–203), its roof solid `roof_shingle` at row 9 (196–203, optional platform with Oblaten, reached from the floor), `musikpavillon` prop (back layer: posts, railing, roof fringe). Resi walks **under** the floor (passage 2 tiles high, Zuckerschock fits). **Blasmusiker #1** on the floor at col 200, facing the player: his notes fly 2 tiles + 10 px above the street, i.e. **over the head** of a standing Resi (Normal and Zuckerschock): the first notes are seen, not suffered (safe introduction). Jumping onto the floor into a note train is a visible, fair risk.
   - Selfie-Tourist #6 at 207 (notes pass through enemies).
   - A park bench (`stone`, 1 tile high, 2 wide, 213–214): standing on it lifts Resi's feet above the note train, and **notes vanish on solid tiles**, so the bench also shields whoever stands behind it.
   - **Blasmusiker #2** at street level at col 222, facing left: notes at 10 px above the street – jump the train, hide behind the bench, or stomp him between bars (200). Flat ground 215–234, no gap within his note range (D-041: note trains never cross a required gap in stage 2).
6. **Schiffsanlegestelle → Maibaum (235–279)**:
   - Wooden pier (`jetty` deck at street level, `jetty_post` below) 236–257 over Wolfgangsee water (`wolfgangsee_water` rows 15–16 in the gaps). Gaps 239–240 (2), 245–247 (3), 252–254 (3).
   - Nordic-Walker #6 on the 4-tile jetty segment 248–251 between two gaps: it paces the segment and never falls in; land on it (stomp) or time the landing while it walks away. Its segment is ≥ 4 tiles so the landing is never blocked for the whole cycle.
   - Taube #3 high above the gaps (row 6, above every jump arc including Zuckerschock jumps, D-038), spawn ~col 258 so it meets Resi over 247–251 (D-032).
   - „Linienschiff / auch voll" sign at the pier's end (256), the kiosk (Schiffskassa) right after: a solid block 260–265 with stairs (steps ≤ 2 tiles, 258–259), its roof 6 tiles above the meadow. Meadow one row lower from 266. **Maibaum at col 270** on the meadow (lowest ground, D-029), Ortsende „St. Gilgen" at 273, level end 279 (camera clamps at the goal, D-031). A good running held jump off the kiosk roof reaches the Wurstkranz (feet ≥ 7 tiles, D-038), walking off scores 100. Nothing flies near this jump.

### Entities and behaviour (new in Stage 2)

Same conventions as Stage 1 (px/frame at 60 Hz, distances horizontal centre to centre, frame counters from activation, no RNG). Numbers go to `tuning.toml` (`[enemies]`, `[platforms]`).

| Entity | Sprite / hitbox | Behaviour | Stomp? | Points |
|---|---|---|---|---|
| **Nordic-Walker** (`nordic_walker`) | 16×24 / 12×22 | Walks 0.75 (brisk: overtakes nobody, turns at everybody) towards the player at activation; turns at walls, other enemies on foot **and ledges**: before stepping on, it checks the cell below its leading foot one px ahead; no solid or one-way tile there → it turns. Falls only if the floor under it disappears (broken brick). Poles plant in a 4-frame walk cycle (cosmetic „klick-klack"). | yes | 100 |
| **Blasmusiker** (`blasmusiker`, tuba player in Tracht) | 24×24 / 12×22 | Stands still, faces the player. While the player is within 10 tiles (< 161 px), a bar counter runs (pauses outside, keeping its pose, like the Verkäufer): bar of **180 frames**: frames 0–29 **inhale** (telegraph: cheeks puffed, tuba bell raised, clearly visible), notes released at frames **30, 54, 78** („Hum-ta-ta", ¾ time; `blow` pose 8 frames each), then idle until 179. The counter starts at 0 when the player first comes in range, so every note train is preceded by an inhale. | yes | 200 |
| ↳ note (`note`) | 8×8 / 6×6 | Spawns at the tuba bell, centre 10 px above the musician's feet, 10 px in front of his centre; flies **straight** towards his facing side at 1.0 for at most 160 px (then vanishes); passes through one-way tiles and enemies; vanishes on solid tiles or outside the level. Trains are 6 + 24 + 24 px long with ~100 px of air between trains. | no (hurts) | – |
| **Gondel** (`gondel`, Zwölferhorn cabin) | 32×52 sprite (cabin 32×24 + hanger 28 px above) / top surface 32 px | Level syntax as the Festungsbahn: `to = [dx, dy]` (other end in tiles, \|dx\| ≤ 8, so a rider carried back never reaches the screen's left edge) and optional `phase` (frames into its cycle at activation). **Free-running ping-pong** (it never waits for a rider): wait 30 frames at the spawn end → travel to the other end at 1 px/frame along the major axis (frames = max(\|dx\|, \|dy\|) × 16) → wait 30 → travel back → repeat. Positions recomputed from the spawn point every frame. Collision, carrying, crush and landing exactly as the Festungsbahn (one-way top, D-026/D-034). The renderer draws the **cable**: a 1 px line between the hanger tops at both end positions, extended 1 tile beyond each end (the cable is the telegraph: it shows exactly where the cabin will go). Never despawns. | – | – |

Tuning targets for the record: track `[6, -1]` = 96 frames of travel, period 252 frames (4.2 s); the longest wait for a cabin is under 5 s.

### New mechanics: purpose, safe introduction, test

| Mechanic | Purpose | Introduced safely | Test |
|---|---|---|---|
| Gondel (free-running platform) | timing a moving platform, riding across a pit | first cabin over a 2-deep trench with stairs (no death); the cable shows the path | unit tests: positions over a full period for a few `to`/`phase` values (ends, waits, no drift), carry exactness, rider at the screen's left edge; lint: 3-tile clearance + pit recovery for `gondel`; replay `st_gilgen_gondeln` (A → B → C); replay: fall off A into the trench and climb out; solver windows ≥ 6 boarding B and C |
| Nordic-Walker (turns at ledges) | enemies that hold platforms; landings that need a look | #1 on flat street, #2 on a 1-high planter | unit tests: turns at a ledge (solid and one-way edge), at walls and enemies, falls when its brick breaks; replay: stomp the walker on the jetty segment |
| Blasmusik note trains | jump-over / shelter timing, a projectile that isn't an arc | first musician on the bandstand floor, notes over a standing Resi's head | unit tests: bar timing (inhale 0–29, notes at 30/54/78, period 180, counter pauses out of range), note speed, range 160, vanishes on solids, passes one-way; replays: hit by a note as Zuckerschock (shrinks), shelter behind the bench, stomp the musician (200, chain start 200); solver windows ≥ 6 at #2 |
| Combination (jetty + walker + Taube) | a stage-2 finale | one moving hazard per jump (D-041) | solver windows ≥ 6; replay: die in the water after the checkpoint → respawn at 142, level reset |
| Carry-over into the stage | reward for a clean Stage 1 | – | flow replay: Zuckerschock + score + lives + Oblaten carried from Salzburg into the St. Gilgen intro |

## Stage 3: Brunnwinkl (Milestone 2 scope)

Length **240 columns** (~3840 px, the shortest stage so far – the hamlet really is five houses: „Fünf Häuser am See" is Karl von Frisch's own book about it), 17 rows, Stage 1's vertical layout. `time_limit = 200` (provisional, D-045). Route along the north shore: the Victor-von-Scheffel-Steig from St. Gilgen → the old Kornmühle at the entrance of Brunnwinkl (house 1; the Exner/Frisch family's summer seat since 1882) → the bee house → houses 2–5 → the bathing jetty → Maibaum. Postbus 150 runs along the **south** shore, so Brunnwinkl has no bus at all – that's this stage's bus joke. Background: sky (reuse), `bg_schafberg` (reuse), `bg_wolfgangsee` (reuse), `bg_forest` (shore forest, new).

New in this stage: **bee swarms dancing the waggle dance** (a periodic, non-stompable hazard whose straight run points to a bonus, after von Frisch's discovery that the dance's direction tells the hive where the food is) and **e-bikers** (a stompable charger that follows the ground). Reused: Nordic-Walker (now on rooftops), Selfie-Tourist, Taube, water death, checkpoint, Maibaum.

### Sections

| # | Section | Columns | New mechanic (introduced safely) | Enemies | Landmarks / jokes |
|---|---|---|---|---|---|
| 1 | **Uferweg** (Scheffel-Steig) | 0–41 | E-Biker: telegraph, charge, crash into a boulder | E-Biker ×1, Nordic-Walker ×1 | `bus` sign „Bus 150 / anderes Ufer"; „Gehweg / nicht Radweg" |
| 2 | **Kornmühle** | 42–89 | Bienenschwarm, high above the path; its dance points to the first bonus | Bienenschwarm ×1, Selfie-Tourist ×1 | „Fünf Häuser / am See"; the old mill (house 1) with its water wheel; „Frischluft / seit 1882"; bonus 1 on the mill roof with the blue `futterschaelchen` |
| 3 | **Bienenhaus** | 90–119 | swarms crossing the path (wait for the opening) | Bienenschwarm ×2, Nordic-Walker ×1 | the bee house with painted hive fronts, hive stacks as steps, „Tanzschule / Brunnwinkl" |
| 4 | **Fünf Häuser** | 120–199 | checkpoint; combination: e-biker on the open path, walkers on roofs, a far dance to the 1-up | E-Biker ×2, Nordic-Walker ×2, Bienenschwarm ×1, Selfie-Tourist ×1 | checkpoint (122) + `bus` sign „Bus 150 / winkt rüber" (from across the lake); houses 2–5 as rooftops; info board „Nobelpreis / für alle"; „Futter nur / auf Blau" at the Zaunerstollen cache |
| 5 | **Badesteg → Maibaum** | 200–239 | – (finale) | Bienenschwarm ×1, Taube ×1 | bathing jetty, boathouse roof as the launch, Maibaum, Ortsende „Brunnwinkl" |

House count (the running joke must be exact): the mill (sec. 2) and houses 2–5 (sec. 4) are the **five houses**; the bee house is a shed and doesn't count.

Section details:

1. **Uferweg (0–41)**: forest path (`forest_path`) along the shore, no pits. Start col 2, „Bus 150 / anderes Ufer" `bus` board at col 5 (clear of Resi at the start; E12 art review). Oblaten 6–10, `?`-box at col 12 (row 11), `M`-box at col 16 (row 11) with a 2-tile rock wall at 20 to turn the Mozartkugel. „Gehweg / nicht Radweg" sign at 22. A boulder (`rock`, 2×2) at 24–25. **E-Biker #1** parked at col 34 facing left on a flat path 26–41: it is on screen long before its 9-tile trigger, telegraphs 40 frames (bell, headlight), charges left – and **crashes into the boulder** („Hoppala!", no points) if Resi waits on or behind it; or Resi stomps it from the boulder (200). Nordic-Walker #1 at col 40 afterwards.
2. **Kornmühle (42–89)**:
   - „Fünf Häuser / am See" sign at 42.
   - Mill race: a 2-tile water gap (`wolfgangsee_water`) at 43–44 (first water, walking jump), the `muehlrad` water-wheel prop on its right bank.
   - Hive stack (`beute`, 1 tile high, 49–50) and **Bienenschwarm #1** anchored above it at col 50, row 10, `dir = 0` (run to the right), `len = 2`: its lowest point stays ≥ 1 tile above the head of a standing Zuckerschock, so **walking underneath is safe**, and nothing in its columns needs or invites a jump (flat ground 47–54, no Oblaten, no box; the mill race's landing is ≥ 2 columns left of its path). Resi walks under it and watches the dance; jumping into it is a visible choice. Its run points right at **bonus 1** on the mill's ridge (`futterschaelchen` prop + 5 Oblaten at the run's height), ~8 tiles away (a short run = a near goal).
   - Kornmühle (house 1) 55–63: solid `facade_village` walls 3 tiles high, stepped `roof_shingle` to a ridge 5 tiles above the path (steps ≤ 2 tiles, climbable from either side; there is no way around – in Brunnwinkl you climb the houses). The step-up at 55 is ≥ 2 columns right of the swarm's path. „Frischluft / seit 1882" sign at 65, after the mill.
   - 66–89: meadow path, Selfie-Tourist at 72 (selfie with the lake), a `?`-box, Oblaten. No pits.
3. **Bienenhaus (90–119)**: „Tanzschule / Brunnwinkl" sign at 92. The `bienenhaus` prop (back layer) behind 95–112 with `beute` stacks in front (1–3 tiles high) as stairs up to its `roof_shingle` roof (5 tiles up, optional, Oblaten). **Swarm #2** at col 99 on the path, `dir = 2` (straight up), `len = 4`; **swarm #3** at col 110, `dir = 2`, `len = 3`, `phase` half a period off #2. A vertical dance rises off the path, loops out to one side, comes back down through its start and loops to the other side: the path is free while the swarm is in the upper part of its figure-eight – wait, then walk or run under it (solver window ≥ 6). Their runs point straight up, at the bee-house roof (on a vertical comb „straight up" means „towards the sun"). Nordic-Walker #2 paces the top of a 3-high hive stack at 103–106, between the two swarms and ≥ 1 column clear of both swarms' loops (#2 reaches ±2 tiles around col 99, #3 ±1.5 tiles around col 110).
4. **Fünf Häuser (120–199)**:
   - Checkpoint at col 122, the „Bus 150 / winkt rüber" `bus` sign right of it (124–126); `M`-box at col 130 (row 11).
   - Paved path 132–150 (`ground`; the real lakeside path has a paved stretch): **E-Biker #2** parked at col 146 on open ground – no boulder this time: jump over it (its hitbox is 20 px high) or stomp it.
   - **House 2** 151–156, **house 3** 161–166, **house 4** 176–182 (the biggest), **house 5** 190–195: solid walls 2–3 tiles high, stepped roofs, gardens in between at path level. Nordic-Walker #3 on house 2's ridge, #4 on house 5's ridge (they patrol the ridge and turn at its ends). Selfie-Tourist at 159 in the garden.
   - Garden 167–175: **swarm #4** anchored on the garden ground at col 170, `dir = 1` (45° up-right), `len = 4`: the long run points far up-right at **bonus 2** on house 4's ridge: an Oblatenkiste with the stage's **Zaunerstollen** (a chimney, 1 solid tile, stops it from rolling off) next to the `futterschaelchen` on its blue card; „Futter nur / auf Blau" sign at 174.
   - „Nobelpreis / für alle" info board at 185 (in Brunnwinkl the neighbours helped count von Frisch's dotted bees; the joke is that the prize should go to all of them).
   - **E-Biker #3** parked in the garden 184–189 at col 188: it triggers while Resi is on house 4's roof and crashes into house 4's wall below her (a gag; only dangerous to a Resi standing in that garden).
5. **Badesteg → Maibaum (200–239)**:
   - Bathing jetty (`jetty`/`jetty_post`) 201–220 over water: gaps 203–204 (2) and 214–216 (3); the segment between them (205–213) is 9 wide. **Swarm #5** at col 209, `dir = 2`, `len = 3`, in the middle of that segment, so its loops (±1.5 tiles) never reach the landing tiles (205–206) or the take-off of the 3-gap (212–213).
   - Taube #1 high above the 3-gap (row 6, above every jump arc), spawn ~col 224, meets Resi over 213–217 (D-032); not near the boathouse or the Maibaum.
   - Boathouse: a solid block 221–226 with stairs (steps ≤ 2 tiles), its roof 6 tiles above the meadow; meadow one row lower from 227. **Maibaum at col 230**, Ortsende „Brunnwinkl" at 233, level end 239. Running held jump off the roof → Wurstkranz (D-038).

### Entities and behaviour (new in Stage 3)

| Entity | Sprite / hitbox | Behaviour | Stomp? | Points |
|---|---|---|---|---|
| **Bienenschwarm** (`bienenschwarm`) | 24×24 / 14×14 | A dancing swarm anchored at its spawn point A (cell centre). Level fields: `dir` (0–7, × 45°, 0 = right, 2 = up, 4 = left, 6 = down; default 0), `len` (waggle-run length in tiles, 2–4; default 3), optional `phase` (frames), optional `dy` (anchor offset from the cell centre in px, ±8, negative = up; default 0). Cycle: **waggle run** from A straight along `dir` for L = len × 16 px at 1 px/frame (diagonals: the same L along the diagonal); **return** to A on a half circle with diameter AB over 64 frames, first on the left side of the run (counter-clockwise), the next time on the right side – the figure-eight of the real waggle dance. Period = 2 × (L + 64) frames (len 2: 192, len 3: 224, len 4: 256). Integer sine as the Taube (D-026). During the run the sprite shakes ±2 px across the run every 4 frames (renderer only; the hitbox moves on the straight line) and uses `waggle`, on the return `buzz`. Ignores tiles; levels keep its whole path out of solid tiles. Never despawns while on screen; activates like other enemies. Any contact hurts, including from above (a swarm can't be stomped); it can't be defeated in M2. **Level rule**: every swarm's run points at a bonus (Oblaten, a box) within one screen, and a longer run means a farther bonus (von Frisch: the waggle run's duration encodes the distance). | no (hurts) | – |
| **E-Biker** (`e_biker`) | 32×24 / 26×20 | Parked and visible, facing left (rider in Lycra over Lederhosen, one foot down). When the player is within 9 tiles (< 145 px): **40-frame telegraph** (bell rings, headlight blinks, front wheel lifts), then rides left at 2.0 along the ground: player gravity and max fall speed, rides off ledges and into water (gone); passes through other enemies; never turns. **Hits a wall → crashes**: `crash` pose for 30 frames with the popup „Hoppala!", then removed, **no points**. Removed once its right edge is left of the screen. Stomp while parked, telegraphing or riding: 200 (the rider sits dazed, `squished`); any other contact hurts. Bumping a block under a parked one knocks it off like other stompable enemies. | yes | 200 |
| **Futterschälchen** (`futterschaelchen`, prop) | 16×8 | Decoration marking a bee bonus: a small glass dish on a blue card (von Frisch trained bees to sugar water on blue paper to prove they see colour). No collision; the bonus itself is normal Oblaten / boxes. | – | – |

### New mechanics: purpose, safe introduction, test

| Mechanic | Purpose | Introduced safely | Test |
|---|---|---|---|
| Bienenschwarm (figure-eight) | read a periodic pattern and wait for the opening; the first enemy that can't be stomped at all | swarm #1 dances above flat ground, out of reach of every jump there | unit tests: positions over a full period for `dir` 0/1/2 and `len` 2/3/4 (run, left loop, right loop, back at A), `phase`, hitbox on the straight line during the run; replays: hurt by a swarm as Zuckerschock, pass swarm #2 by waiting; solver windows ≥ 6 at swarms #2, #3, #5 |
| The dance points to a bonus | exploration reward grounded in von Frisch's discovery | swarm #1's bonus is near and on the way (mill roof) | level review checklist (every swarm points at its bonus; QA verifies); replay collecting bonus 1 and the Zaunerstollen (`lives` +1) |
| E-Biker | a fast charger you may stomp (unlike the Fiaker), following the ground | parked in view, telegraph, a boulder to wait on that it crashes into | unit tests: trigger < 145 px, telegraph 40, speed 2.0, gravity/ledges, crash on a wall (removed, no points, popup), stomp 200 + chain start 200; replays: stomp it, let it crash at the boulder, side hit as Normal → death |
| Walkers on rooftops | reuse of the stage-2 enemy in a new place | – | solver windows ≥ 6 over houses 2 and 5 |
| Last stage of M2 → teaser | the route continues | – | flow replay: clear Brunnwinkl → map walks to Falkenstein → „Fortsetzung folgt …" → title, run reset |

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
| `stage.st_gilgen.title` / `.tagline` / `.next` | Etappe 2: St. Gilgen / Mozartdorf ohne Mozart. / Weiter nach Brunnwinkl. |
| `stage.st_gilgen.sign_*` | `sign_bus` „Bus 150 / 4x stündlich", `sign_ischlerbahn` „Ischlerbahn / bis 1957", `sign_seilbahn` „Seilbahn / seit 1957", `sign_dachfahrt` „Dachfahrt / verboten", `sign_mozartplatz` „Hier wohnte / nie Mozart", `sign_checkpoint` „Bus 150 / alle 4 voll", `sign_mozarthaus` „Mozarts Mama / geboren 1720", `sign_nannerl` „Hier wohnte / Nannerl", `sign_platzkonzert` „Platzkonzert / Zugabe droht", `sign_schiff` „Linienschiff / auch voll", `sign_ortsende` „St. Gilgen" |
| `stage.brunnwinkl.title` / `.tagline` / `.next` | Etappe 3: Brunnwinkl / Fünf Häuser, null Ruhe. / Weiter zum Falkenstein. |
| `stage.brunnwinkl.sign_*` | `sign_bus` „Bus 150 / anderes Ufer", `sign_gehweg` „Gehweg / nicht Radweg", `sign_haeuser` „Fünf Häuser / am See", `sign_muehle` „Frischluft / seit 1882", `sign_tanzschule` „Tanzschule / Brunnwinkl", `sign_checkpoint` „Bus 150 / winkt rüber", `sign_futter` „Futter nur / auf Blau", `sign_nobelpreis` „Nobelpreis / für alle", `sign_ortsende` „Brunnwinkl" |
| `popup.hoppala` | Hoppala! (an e-biker crashes into a wall) |
| `map.next_stop` | Nächster Halt: |
| `map.continue` | Weiter mit START |
| `map.station.<id>` | Salzburg, St. Gilgen, Brunnwinkl, Falkenstein, St. Wolfgang, Wolfgangsee, Strobl, Bad Ischl, Café Zauner (needs the `é` glyph appended to the font, D-043) |

## Sources (facts used in Stage 1)

- Festungsbahn: opened 1 August 1892, Austria's oldest funicular still running, two counter-running cars – [salzburg.info](https://salzburg.info/de/reiseinfos/anreise-verkehr/festungsbahn), [Salzburg24](https://www.salzburg24.at/news/salzburg/stadt/festungsbahn-besteht-seit-120-jahren-art-189911)
- Love locks on the Makartsteg (officially Marko-Feingold-Steg): the city regularly cuts off locks, about 1.5 t a year, one round ~2 t, because they strain the railing netting – [Salzburg24](https://www.salzburg24.at/news/salzburg/stadt/15000-liebesschloesser-in-der-stadt-salzburg-muessen-weichen-art-274322), [Salzburg24](https://www.salzburg24.at/news/salzburg/stadt/viele-liebesschloesser-am-feingold-steg-werden-abgezwickt-art-318455)
- Sphaera (Stephan Balkenhol, 2007), gold ball with a man on top on Kapitelplatz, popularly called the "Balkenhol-Mozartkugel"; giant chess on the square – [salzburg.info](https://salzburg.info/de/salzburg/creative-salzburg/kunst-im-oeffentlichen-raum/walk-of-modern-art/sphaera), [salzburg.info Kapitelplatz](https://salzburg.info/de/sehenswertes/plaetze-strassen/kapitelplatz-kapitelschwemme)
- Dombögen: arcades linking Residenz and Dom; the DomQuartier museum route crosses them (Dombogen terrace) – [domquartier.at](https://www.domquartier.at/en/?p=1720), [salzkammergut.at](https://www.salzkammergut.at/en/oesterreich-poi/detail/430014870/domquartier-salzburg.html?type=2026)
- Maibaumkraxeln: climbing the (often soaped) Maibaum to the Wurstkranz at the top – [salzburg.info](https://www.salzburg.info/de/magazin/kunst-kultur/Von-Maibaumkraxlern-und-Maibaumdieben_a_282454)
- Austrian bus-stop sign: yellow disc, green rim, green „H" (§ 2 KflG-DV) – [jusline](https://www.jusline.at/paragraphs/history/456477369)
- Getreidegasse guild signs: wrought-iron Zunftschilder, mostly from the Wieber locksmith family; the old Mödlhammer brewery sign's „M" now stands for McDonald's (behind the burger-shaped guild sign) – [salzburgerland.com](https://www.salzburgerland.com/de/magazin/die-zunftschilder-in-der-salzburger-getreidegasse/)
- Mozartkugel naming dispute (Fürst's „Original" vs. other makers' „Echte"), behind the „Ur-Original" stand sign – [Wikipedia](https://en.wikipedia.org/wiki/Mozartkugel)
- Common knowledge (no source needed): Pegasus fountain and Mirabellgarten as Sound of Music "Do-Re-Mi" location, Zwergerlgarten, Mozart's birthplace at Getreidegasse 9, Jedermann on Domplatz, Fiaker at Residenzplatz, Hanuschplatz bus hub at the end of the Makartsteg.

## Sources (facts used in Stages 2 and 3)

- Mozarthaus St. Gilgen (Ischler Straße 15): built 1718–1720 as the Pfleggericht by Mozart's grandfather Wolfgang Nikolaus Pertl; Anna Maria Pertl born there on 25 December 1720; Nannerl lived there 1784–1801 as the wife of the local Pfleger (17 years); a court until 2002, a cultural centre since 2009 – [salzkammergut.at](https://www.salzkammergut.at/oesterreich-poi/detail/402182/mozarthaus-am-wolfgangsee-nannerl-museum.html), [salzkammergut.at](https://www.salzkammergut.at/oesterreich-poi/detail/402182/denkmal-mozarthaus-in-sankt-gilgen.html)
- W. A. Mozart never lived in St. Gilgen despite the nickname „Mozartdorf" (behind „Hier wohnte / nie Mozart") – [Lehmanns (book excerpt)](https://www.lehmanns.ch/media/87305364)
- Mozartbrunnen on Mozartplatz: bronze of the young Mozart playing the violin, by Karl Wollek, 1926 – [salzkammergut.at](https://www.salzkammergut.at/en/oesterreich-poi/detail/401931/mozart-fountain.html)
- Zwölferhorn cable car: completed 1957, old cabins for four people in yellow or red; no permit extension after 60+ years; new line opened 23 October 2020, cabins kept the yellow/red colours – [Salzburg24](https://www.salzburg24.at/news/salzburg/flachgau/neue-seilbahn-auf-das-zwoelferhorn-in-betrieb-art-234245), [salzburgerland.com](https://www.salzburgerland.com/de/magazin/zwoelferhorn-seilbahn-erstrahlt-in-neuem-glanz/)
- Postbus 150 runs Salzburg – St. Gilgen up to four times an hour at peak times, up to 10 000 passengers on a weekend (behind „4x stündlich / alle 4 voll") – [Salzburg24](https://www.salzburg24.at/news/salzburg/flachgau/bis-zu-10000-fahrgaeste-am-wochenende-linie-150-faehrt-ab-september-bis-zu-vier-mal-pro-stunde-art-336855)
- Brunnwinkl: hamlet on the Wolfgangsee near St. Gilgen, where Karl von Frisch decoded the waggle dance – [Wikipedia](https://en.wikipedia.org/wiki/Brunnwinkl); the Exner family (von Frisch's mother's family) rented the old Kornmühle in 1882 and bought houses there, the „fünf Häuser am See"; bee experiments there from 1912/13, local residents helped count the marked bees; information board in Brunnwinkl – [Salzburgwiki](https://wiki.sn.at/wiki/index.php/Karl_von_Frisch); von Frisch's book „Fünf Häuser am See. Der Brunnwinkl" – [Amazon listing](https://www.amazon.de/F%C3%BCnf-H%C3%A4user-See-Brunnwinkl-Sommersitzes/dp/354010044X); Nobel Prize 1973 – [Wikipedia](https://en.wikipedia.org/wiki/Karl_von_Frisch)
- The shore path St. Gilgen – Brunnwinkl – Fürberg (Victor-von-Scheffel-Steig, partly paved), Brunnwinkl as the start of the Falkenstein hike; bus 150 serves St. Gilgen, not the north shore – [komoot](https://www.komoot.com/smarttour/38298959), [Jana Meerman](https://janameerman.com/falkenstein-hike/)
- Waggle dance: a straight waggle run then a half-circle return, alternating sides (figure-eight); the run's angle to vertical gives the direction relative to the sun, its duration the distance – [NC State Extension](https://content.ces.ncsu.edu/honey-bee-dance-language), [Wikipedia](https://en.wikipedia.org/wiki/Waggle_dance)
- Colour vision: von Frisch trained bees to sugar-water dishes on blue paper among grey cards (published 1914) – [Lernhelfer](https://www.lernhelfer.de/schuelerlexikon/biologie/artikel/karl-von-frisch)
- Common knowledge (no source needed): Wolfgangsee boat line („Linienschiff") from St. Gilgen, Platzkonzerte of the local brass bands („Humtata"), the B158 and the bus running along the south shore, Nordic walking and e-bikes around the lake.

## Resolved decisions (human, 2026-10-18)

1. **Title**: „Oblatenjagd"
2. **Hero**: Resi in a Dirndl (one playable character)
3. **Real names**: real places, businesses and historical people (Café Zauner, Postbus, Fürst Mozartkugeln, Kaiser Franz Joseph, …) may be used by name – it's a private project.
4. **Difficulty**: medium (see "Difficulty: medium" above)
