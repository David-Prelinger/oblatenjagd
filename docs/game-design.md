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

(Stage 4 adds climbing rules on top of these: see "Stage 4: Falkenstein" → "Climbing rules".)

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
| **Hackl** (thrower, big) | grab **St. Wolfgang's Hackl** from an `H`-Oblatenkiste (from M3, Falkenstein: at the Hackelwurfkapelle, after the legend of the axe throw), from Normal or Zuckerschock | big like Zuckerschock (breaks bricks), and a new press of RUN throws a Hackl in an arc (D-056) |
| 1-Up | **Zaunerstollen** (rare) | extra life |

- Physics are the same in every state; only the hitbox differs: normal 12×22 px, Zuckerschock and Hackl **12×30 px** (sprite 16×32). Growing keeps the feet in place.
- **Hierarchy** (from M3, D-056): Normal → (Mozartkugel) Zuckerschock → (Hackl) Hackl; a Hackl also turns Normal straight into Hackl. A hit steps down one state: Hackl → Zuckerschock, Zuckerschock → Normal, Normal dies. An item that doesn't raise the state gives its points only (Mozartkugel as Hackl: 1000).
- **No ducking**: DOWN only matters for one-way drop-through (DOWN + JUMP, D-011) and, from M3, for entering the Durchkriechstein (DOWN at its mouth starts a scripted crawl, D-057). Levels respect this (≥ 2-tile passages everywhere else).
- **Hurt**: Hackl → Zuckerschock or Zuckerschock → Normal, plus **120 invulnerability frames** (2 s; sprite blinks every 4 frames). Enemies and projectiles can't hurt during them, but pits, water, the climb camera's death line and time-up still kill.
- **Transformation**: every power change (growing, shrinking, Zuckerschock ↔ Hackl) freezes the game for 30 frames while the sprite alternates between the old and the new state's sprite every 4 frames. The timer doesn't run during the freeze.
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
| 4 | **Falkenstein** | Pilgrim path from Fürberg up the Kreuzweg and the Falkensteinwand (vertical climbing, climb camera), the Falkensteinkirche with the *Durchkriechstein* crawl-through (shed your sins = lose your power-up, or +1 life if you have none), the Hackelwurfkapelle where Resi gets the **Hackl** (M3 spec below) | Pilgrim groups, falling rocks (Steinschlag), chamois (Gams) |
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

The stage order is the route (D-042): `salzburg` → `st_gilgen` → `brunnwinkl` → `falkenstein` → `st_wolfgang` → `wolfgangsee` → `strobl` → `bad_ischl` → `cafe_zauner`. A station is playable when a level with that id ships; in M2 these are the first three (`01_salzburg`, `02_st_gilgen`, `03_brunnwinkl`), M3 adds `04_falkenstein` (so the teaser moves from Brunnwinkl → Falkenstein to Falkenstein → St. Wolfgang).

| Thing | Into the next stage | After a death | Game over → „Weiter?" |
|---|---|---|---|
| Score | carries over | kept | 0 |
| Lives | carry over | −1 | 3 |
| Oblaten counter | carries over | kept | 0 |
| Power state | **carries over** (Zuckerschock and, from M3, Hackl stay, like SMB) | Normal | Normal |
| Timer | the new stage's `time_limit` | reset | reset |
| Checkpoint | cleared | kept | cleared |
| Stage | the next station | same stage | **same stage**, from its start (D-017) |

- **Title → START** always starts the route at Salzburg (no stage select, no saves in M2). The title screen itself is unchanged.
- **Stage clear → route map → intro of the next stage → play.** The Maibaum sequence runs as in Stage 1 up to the `Clear` phase; the route map replaces „Fortsetzung folgt …".
- **Last playable stage** (Brunnwinkl in M2, Falkenstein in M3; generally: the next station has no level yet): the route map still plays, Resi walks on to the next station (Falkenstein), and the panel shows „Fortsetzung folgt …" for 180 frames instead of the prompt; then the title, and the run is reset (score 0, 3 lives, Oblaten 0, Normal).
- **Game over in stage 2 or 3** restarts that stage (not Salzburg) with 3 lives, score 0, Oblaten 0, Normal. Unlimited continues, as in Stage 1.
- **Stage intro** for every stage: as in "Screens and HUD", with that stage's `stage.<id>.title` and `.tagline`; Resi drawn in her current power state (`player`, `player_big` or `player_hackl`; the route map too).
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
| Brunnwinkl | 124, 58 | above-left: right-aligned to x 114, rows 43–51 (art-director M2; clear of the Salzburg label and the Salzburg→St. Gilgen route); alternate slot rows 20–28 on a map screen where a big Resi stands on the St. Gilgen or Brunnwinkl dot (M3) |
| Falkenstein | 152, 68 | below, ending at x ≤ 172 (clear of the St. Wolfgang marker); alternate slot ending at x 171 on a map screen where a big Resi stands on St. Wolfgang (M3) |
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

Length **280 columns** (~4480 px), 17 rows high with Stage 1's vertical layout (3 empty rows on top for the goal camera, the street's top is row 15, rows 15–16 ground or water; the goal meadow one row lower). `time_limit = 210` (D-045: 3 × the QA relaxed run of 68 s, M2 level pass; later relaxed runs of ~66 s keep it within the rule's rounding, gondola waits vary by player). Measured at M2 acceptance: solver narrowest jump window 10 (≥ 6), 85 Oblaten in the stage (84 placed + 1 from the `?`-box). The route follows the real village from the bus station at the foot of the Zwölferhorn to the lake: Busbahnhof → Zwölferhorn-Talstation → Mozartplatz (Rathaus, Mozartbrunnen) → Ischler Straße with the Mozarthaus (the former Pfleggericht where Mozart's mother was born and Nannerl lived) → Seepromenade with the bandstand → Schiffsanlegestelle → Maibaum on the lake meadow. Background: sky (reuse `bg_sky_salzburg`), `bg_schafberg` across the lake, `bg_wolfgangsee` (lake + far shore), `bg_stgilgen` (village houses).

New in this stage: **free-running moving platforms** (Zwölferhorn gondolas), **an enemy that stays on its platform** (Nordic-Walker), **horizontal projectile trains** (Blasmusik notes). Reused: Selfie-Tourist, Taube (+ sitting), one Mozartkugel-Verkäufer, water death (Wolfgangsee), checkpoint, Maibaum.

### Sections

| # | Section | Columns | New mechanic (introduced safely) | Enemies | Landmarks / jokes |
|---|---|---|---|---|---|
| 1 | **Busbahnhof** | 0–34 | Mozartkugel early; Nordic-Walker (turns at ledges) | Nordic-Walker ×2 | `bus` sign „Bus 150 / 4x stündlich" (line 150 now runs up to four times an hour – and is still full); the old Ischlerbahn's buffer stop (`prellbock`) with „Ischlerbahn / bis 1957" |
| 2 | **Zwölferhorn-Talstation** | 35–94 | Gondel: free-running moving platforms | Selfie-Tourist ×1 | valley station, yellow cabins (the real colours), pylon; „Seilbahn / seit 1957" (the cable car opened the year the Ischlerbahn closed), „Dachfahrt / verboten" at the first cabin (Resi rides on the roof anyway) |
| 3 | **Mozartplatz** | 95–139 | walker on a raised platform + Verkäufer (combination) | Selfie-Tourist ×3, Nordic-Walker ×1, Mozartkugel-Verkäufer ×1, `taube_sitting` ×1 | Mozartbrunnen (young Mozart with his violin, 1926) with a pigeon on his head; sign „Hier wohnte / nie Mozart" (the „Mozartdorf" never had Mozart living in it) |
| 4 | **Ischler Straße** | 140–189 | checkpoint; upper route over balconies with patrolling walkers | Nordic-Walker ×2, Selfie-Tourist ×2 | checkpoint (col 142) with the `bus` sign „Bus 150 / alle 4 voll" beside it (D-039); Mozarthaus with „Mozarts Mama / geboren 1720" and „Hier wohnte / Nannerl" (the callback); wooden balconies with geraniums |
| 5 | **Seepromenade** | 190–234 | Blasmusik notes | Blasmusiker ×2 | bandstand (Musikpavillon) with the Platzkonzert, „Platzkonzert / Zugabe droht", lake view |
| 6 | **Schiffsanlegestelle → Maibaum** | 235–279 | combination: jetty gaps with a walker guarding a jetty segment | Nordic-Walker ×1, Taube ×1 | wooden pier, „Linienschiff / auch voll" (the boat is full too), ticket kiosk roof as the launch to the Maibaum, Ortsende „St. Gilgen" |

Section details (for the level-designer; exact tiles are theirs within these rules; columns are targets):

1. **Busbahnhof (0–34)**: no pits. Start at col 2, the „Bus 150 / 4x stündlich" `bus` board at col 5 (right of the start, clear of Resi standing there; E12 art review). 5 Oblaten at walking height (6–10). `M`-Oblatenkiste at col 14, row 11 (4 tiles up) with bricks either side and a 2-tile bus-shelter wall (`stone`) at 18–19 so the rolling Mozartkugel turns back (Stage 1 pattern); a Resi who carried Zuckerschock over just scores it. The `prellbock` prop (col 20) and the „Ischlerbahn / bis 1957" sign at 24 (decorative, street level; the walker passing in front of it is accepted, enemies are drawn over signs everywhere). Nordic-Walker #1 at col 25 on flat street with ≥ 6 free tiles either side. A 1-tile-high flower planter (`stone`, 6 wide, 29–34) with Nordic-Walker #2 on it: it paces to the planter's edge and **turns** instead of stepping off. The planter's left end is a raised 3-tile flower column (`stone`, col 28, M2 level pass): Resi climbs it and stands above the walker's head (no side contact, unlike a 1-tile step where a walking-pace hop onto the planter lands in front of the walker), watches it turn at the right edge and steps off onto it (stomp) – the mechanic shown where it can't hurt. The column also turns walker #1, so walker #1 paces the street 20–27.
2. **Zwölferhorn-Talstation (35–94)**:
   - Forecourt 35–42: `talstation` prop (back layer) behind 36–44, „Seilbahn / seit 1957" sign at 37, Selfie-Tourist at col 40 (a selfie in front of the cabins). Steps up (1 tile each) to the station platform (`stone`, top 2 tiles above the street, 43–46). „Dachfahrt / verboten" sign at 44, on the platform, clear of the cabin.
   - **Gondel A** (intro): spawns at col 47 with its roof ≤ 8 px above the platform (step or tap-jump on), `to = [6, -1]`. Under its whole track a **meadow trench 2 tiles deep with stairs at both ends** (47–54; the cabin is 2 tiles wide, so it ends over 53–54): falling off means climbing out, never death (D-027/D-028). A line of Oblaten 2 tiles above the roof along the track (collected while riding). It ends next to a meadow knoll (55–61, top level with A's upper end).
   - **Gondel B**: spawns at col 62, `to = [7, -2]`, over a **death ravine** (62–70, empty to the level bottom). It ends next to the pylon platform (`rock`, 71–73, `seilbahn_stuetze` prop behind it).
   - **Gondel C**: spawns at the pylon at col 74 (roof 8 px above the pylon top: tap jump on), `to = [7, 2]` (descending), over the ravine (74–82), ending at a ledge 83–85 (1 tile above the street), street from 86. B and C share the period (284 frames); C's `phase` (245) is tuned with the solver so that on the solver's and a relaxed run's timing C is at the pylon or arrives within ~70 frames of B's arrival. **Accepted limit (M2 acceptance)**: C activates a screen after B, so their relative phase depends on how long the player takes between the two activations, and no `phase` can bound the wait for every player. The rule that matters holds anyway: the pylon is a safe solid rock platform, the cable shows where C is, and the longest possible wait is under one period (284 frames ≈ 4.7 s, within the "under 5 s" target below). An engine change (activate C together with B) is optional backlog polish, not required.
   - No Taube in this section: a Taube activated a screen ahead would drift into the tracks while Resi rides or waits (D-032). Keep ≥ 3 tiles free above every track (lint).
3. **Mozartplatz (95–139)**: flat cobbled square (no pits). Mozartbrunnen 100–104: basin rim 1 tile high (5 wide, solid `stone`), the statue plinth 3 tiles high and 1 wide at 102 (drawn = solid, D-038), the `mozartbrunnen` prop over it; a `taube_sitting` on Mozart's head (rises when Resi is within 6 tiles, flies over the square). „Hier wohnte / nie Mozart" sign at 97, left of the basin and not covering it. Selfie-Tourists at 114, 128, 138 (M2 level pass: only one between the basin and the Verkäufer, so it paces the whole 105–121 strip instead of being pinned against the basin rim by a second tourist). Mozartkugel-Verkäufer at street level at col 122, right beside a souvenir stand (one-way stand roof 117–120, row 12; the Mozartdorf sells Mozartkugeln anyway): he stands in the open, so he is stompable from the stand roof or with a hop (D-046); keep the tourists out of its landing zone in front of it where possible (it's still fine: thrown balls don't hurt enemies). A Gastgarten platform (`oneway_wood`, row 12 = 3 up, 131–137) with Nordic-Walker #3 patrolling it and a `?`-box 3 tiles above its middle (optional).
4. **Ischler Straße (140–189)**:
   - Checkpoint at col 142; the „Bus 150 / alle 4 voll" `bus` sign right of it (145, spanning 143–147) without hiding the checkpoint pole or the box; `M`-Oblatenkiste at col 150, row 11 (within 10 columns, as in Stage 1).
   - Mozarthaus first (shipped order, M2 level pass): `mozarthaus` prop (back layer) behind 151–158 with a solid stone portal step at 154–155 (it turns the Mozartkugel and tourist #5); „Mozarts Mama / geboren 1720" at 151, „Hier wohnte / Nannerl" at 158 (both at street level). Selfie-Tourist #5 at 152 roams the street left of the portal step.
   - Then four walk-through village houses H1 162–166, H2 169–174, H3 177–182, H4 186–191, drawn **full height down to the street** with the decorative back tile `wall_village_bg` (non-solid), so there are no floating façade blocks (M1 art note), each under a full gable of the non-solid `roof_bg` (one row per tile narrower on each side, sprite-catalog convention); their Oblaten hang in front of the wall just above each balcony (entity `tile = "wall_village_bg"`, D-049), never inside a roof. `balcony` one-way tiles form the optional upper route, a staircase as with the Zunftschilder: a trough step at 161 → B1 row 11 at 162–167 → B2 row 8 at 169–175 → B3 row 11 at 177–183 → B4 row 9 at 185–192 → the bandstand roof (sec. 5). Nordic-Walker #4 on B1 and #5 on B2: they pace their balcony and never fall onto the street (the mechanic's payoff).
   - Selfie-Tourist #6 at 185 on the street, penned by a trough at 183 and a planter at 188–189 so it stays away from under the balcony walkers. The street is always passable without the balconies (one-way, 3 tiles of headroom).
5. **Seepromenade (190–234)**:
   - „Platzkonzert / Zugabe droht" sign at 193.
   - Musikpavillon: `oneway_wood` floor at row 13 (2 tiles up, 196–203), its roof solid `roof_shingle` at row 9 (196–203, optional platform with Oblaten, reached from the balcony route's B4), `musikpavillon` prop (back layer: posts, railing, roof fringe). Resi walks **under** the floor (passage 2 tiles high, Zuckerschock fits). **Blasmusiker #1** on the floor at col 200, facing the player: his notes fly 2 tiles + 10 px above the street, i.e. **over the head** of a standing Resi (Normal and Zuckerschock): the first notes are seen, not suffered (safe introduction). Jumping onto the floor into a note train is a visible, fair risk.
   - No Selfie-Tourist here (moved to Ischler Straße in the M2 level pass): the promenade belongs to the Blasmusik.
   - A park bench (`stone`, 1 tile high, 2 wide, 214–215): standing on it lifts Resi's feet above the note train, and **notes vanish on solid tiles**, so the bench also shields whoever stands behind it.
   - **Blasmusiker #2** at street level at col 223, facing left: notes at 10 px above the street – jump the train, hide behind the bench, or stomp him between bars (200). Flat ground 216–235, no gap within his note range (D-041: note trains never cross a required gap in stage 2; after Resi passes him his notes reach col 234 at most, short of the pier).
   - „Linienschiff / auch voll" sign at 234, at the pier entrance.
6. **Schiffsanlegestelle → Maibaum (235–279)**:
   - Wooden pier (`jetty` deck at street level, `jetty_post` below) 236–257 over Wolfgangsee water (`wolfgangsee_water` rows 15–16 in the gaps). Shipped gaps 239–240 (2), 244–246 (3), 253–254 (2).
   - Nordic-Walker #6 on the 6-tile jetty segment 247–252 between two gaps: it paces the segment and never falls in; land on it (stomp) or time the landing while it walks away. Its segment is ≥ 4 tiles so the landing is never blocked for the whole cycle.
   - Taube #3 high above the gaps (row 6, above every jump arc including Zuckerschock jumps, D-038), spawn ~col 258 so it meets Resi over 247–251 (D-032).
   - The kiosk (Schiffskassa) right after the pier: a solid block 260–265 behind a 3-high step at 258–259 (a walking-jump step-up, within the reach rules), its roof 6 tiles above the meadow. Meadow one row lower from 266. **Maibaum at col 270** on the meadow (lowest ground, D-029), Ortsende „St. Gilgen" at 273, level end 279 (camera clamps at the goal, D-031). A good running held jump off the kiosk roof reaches the Wurstkranz (feet ≥ 7 tiles, D-038), walking off scores 100. Nothing flies near this jump.

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

Length **240 columns** (~3840 px, the shortest stage so far – the hamlet really is five houses: „Fünf Häuser am See" is Karl von Frisch's own book about it), 17 rows, Stage 1's vertical layout. `time_limit = 200` (D-045: 3 × the ~60 s relaxed run = 180, clamped to the 200 s minimum). Measured at M2 acceptance: solver narrowest jump window 11 (≥ 6), 53 Oblaten in the stage. Route along the north shore: the Victor-von-Scheffel-Steig from St. Gilgen → the old Kornmühle at the entrance of Brunnwinkl (house 1; the Exner/Frisch family's summer seat since 1882) → the bee house → houses 2–5 → the bathing jetty → Maibaum. Postbus 150 runs along the **south** shore, so Brunnwinkl has no bus at all – that's this stage's bus joke. Background: sky (reuse), `bg_schafberg` (reuse), `bg_wolfgangsee` (reuse), `bg_forest` (shore forest, new).

New in this stage: **bee swarms dancing the waggle dance** (a periodic, non-stompable hazard whose straight run points to a bonus, after von Frisch's discovery that the dance's direction tells the hive where the food is) and **e-bikers** (a stompable charger that follows the ground). Reused: Nordic-Walker (now on rooftops), Selfie-Tourist, Taube, water death, checkpoint, Maibaum.

### Sections

| # | Section | Columns | New mechanic (introduced safely) | Enemies | Landmarks / jokes |
|---|---|---|---|---|---|
| 1 | **Uferweg** (Scheffel-Steig) | 0–41 | E-Biker: telegraph, charge, crash into a boulder | E-Biker ×1, Nordic-Walker ×1 | `bus` sign „Bus 150 / fährt drüben" (D-052); „Gehweg / nicht Radweg" |
| 2 | **Kornmühle** | 42–89 | Bienenschwarm, high above the path; its dance points to the first bonus | Bienenschwarm ×1, Selfie-Tourist ×1 | „Fünf Häuser / am See"; the old mill (house 1) with its water wheel; „Frischluft / seit 1882"; bonus 1 on the mill roof with the blue `futterschaelchen` |
| 3 | **Bienenhaus** | 90–119 | swarms crossing the path (wait for the opening) | Bienenschwarm ×2, Nordic-Walker ×1 | the bee house with painted hive fronts, hive stacks as steps, „Tanzschule / Brunnwinkl" |
| 4 | **Fünf Häuser** | 120–199 | checkpoint; combination: e-biker on the open path, walkers on roofs, a far dance to the 1-up | E-Biker ×2, Nordic-Walker ×2, Bienenschwarm ×1, Selfie-Tourist ×1 | checkpoint (122) + `bus` sign „Bus 150 / winkt rüber" (from across the lake); houses 2–5 as rooftops; info board „Nobelpreis / für alle"; „Futter nur / auf Blau" at the Zaunerstollen cache |
| 5 | **Badesteg → Maibaum** | 200–239 | – (finale) | Bienenschwarm ×1, Taube ×1 | bathing jetty, boathouse roof as the launch, Maibaum, Ortsende „Brunnwinkl" |

House count (the running joke must be exact): the mill (sec. 2) and houses 2–5 (sec. 4) are the **five houses**; the bee house is a shed and doesn't count.

Section details:

1. **Uferweg (0–41)**: forest path (`forest_path`) along the shore, no pits. Start col 2, „Bus 150 / fährt drüben" `bus` board at col 5 (clear of Resi at the start; E12 art review). Oblaten 6–10, `?`-box at col 12 (row 11), `M`-box at col 16 (row 11) with a 2-tile rock wall at 19 to turn the Mozartkugel. „Gehweg / nicht Radweg" sign at 22. A boulder (`rock`, 2×2) at 25–26. **E-Biker #1** parked at col 34 facing left on a flat path 26–41: it is on screen long before its 9-tile trigger, telegraphs 40 frames (bell, headlight), charges left – and **crashes into the boulder** („Hoppala!", no points) if Resi waits on or behind it; or Resi stomps it from the boulder (200). Nordic-Walker #1 at col 41 afterwards, with the „Fünf Häuser / am See" sign at 40 just left of it (the walker is drawn in front of it; shipped in the M2 level pass, the sign greets Resi at the hamlet's edge right before the mill race).
2. **Kornmühle (42–89)**:
   - Mill race: a 2-tile water gap (`wolfgangsee_water`) at 43–44 (first water, walking jump), the `muehlrad` water-wheel prop on its right bank.
   - Hive stack (`beute`, 1 tile high, 49–50) and **Bienenschwarm #1** anchored above it at col 50, row 9 (shipped; its lowest point is 19 px above a standing Zuckerschock even on the hive), `dir = 0` (run to the right), `len = 2`: its lowest point stays ≥ 1 tile above the head of a standing Zuckerschock, so **walking underneath is safe**, and nothing in its columns needs or invites a jump (flat ground 47–54, no Oblaten, no box; the mill race's landing is ≥ 2 columns left of its path). Resi walks under it and watches the dance; jumping into it is a visible choice. Its run points right at **bonus 1** on the mill's ridge (`futterschaelchen` prop + 5 Oblaten at the run's height), ~8 tiles away (a short run = a near goal).
   - Kornmühle (house 1) 55–63: solid `facade_village` walls 3 tiles high, stepped `roof_shingle` to a ridge 5 tiles above the path (steps ≤ 2 tiles, climbable from either side; there is no way around – in Brunnwinkl you climb the houses). The step-up at 55 is ≥ 2 columns right of the swarm's path. „Frischluft / seit 1882" sign at 65, after the mill.
   - 66–89: meadow path, Selfie-Tourist at 72 (selfie with the lake), a `?`-box, Oblaten. No pits.
3. **Bienenhaus (90–119)**: „Tanzschule / Brunnwinkl" sign at 92. Shipped as **two** `bienenhaus` sheds (back layer) behind 94–101 and 109–116, each under a `roof_shingle` roof at row 9 (6 tiles up, optional, Oblaten on both), with a `beute` stack in front (103–108: 1 high at 103 and 108, 3 high at 104–107) as the stairs up to both roofs; sheds are not houses, the five-house count stays exact. **Swarm #2** at col 99 on the path, `dir = 2` (straight up), `len = 4`; **swarm #3** at col 112, `dir = 2`, `len = 3`, `phase` half a period off #2. A vertical dance rises off the path, loops out to one side, comes back down through its start and loops to the other side: the path is free while the swarm is in the upper part of its figure-eight – wait, then walk or run under it (solver window ≥ 6). Their runs point straight up, at the bee-house roof (on a vertical comb „straight up" means „towards the sun"). Nordic-Walker #2 paces the top of the 3-high hive stack at 104–107, between the two swarms and ≥ 1 column clear of both swarms' loops (#2 reaches cols 97–101, #3 cols 111–114).
4. **Fünf Häuser (120–199)**:
   - Checkpoint at col 122, the „Bus 150 / winkt rüber" `bus` sign right of it (124–126); `M`-box at col 130 (row 11).
   - Paved path 132–150 (`ground`; the real lakeside path has a paved stretch): **E-Biker #2** parked at col 146 on open ground – no boulder this time: jump over it (its hitbox is 20 px high) or stomp it.
   - **House 2** 151–156, **house 3** 161–166, **house 4** 176–182 (the biggest), **house 5** 190–195: solid walls 2–3 tiles high, stepped roofs, gardens in between at path level. Nordic-Walker #3 on house 2's ridge, #4 on house 5's ridge (they patrol the ridge and turn at its ends). The section's Selfie-Tourist walks the paved path (col 140, M2 level pass; in the 4-wide garden 157–160 it was always where a walk off house 2's eave lands).
   - Garden 167–175: **swarm #4** anchored on the garden ground at col 170, `dir = 1` (45° up-right), `len = 4`: the long run points far up-right at **bonus 2** on house 4's ridge: an Oblatenkiste with the stage's **Zaunerstollen** (a `chimney` stack, 2 solid tiles on the eave step at 182, stops it from rolling off) next to the `futterschaelchen` on its blue card (shipped: box at 177, row 7, 3 tiles above the ridge, the dish on the eave step at 176); „Futter nur / auf Blau" sign standing on house 4's ridge right beside the dish (the garden at 174 is inside swarm #4's reach, 169–174).
   - „Nobelpreis / für alle" info board at 185 (in Brunnwinkl the neighbours helped count von Frisch's dotted bees; the joke is that the prize should go to all of them).
   - **E-Biker #3** parked in the garden 184–189 at col 188: it triggers while Resi is on house 4's roof and crashes into house 4's wall below her (a gag; only dangerous to a Resi standing in that garden).
5. **Badesteg → Maibaum (200–239)**:
   - Bathing jetty (`jetty`/`jetty_post`) 201–226 over water: gaps 203–204 (2) and 215–217 (3, moved one column right in the M2 level pass: the solver's jump window there was 5 at 214–216); the segment between them (205–214) is 10 wide. **Swarm #5** at col 209, `dir = 2`, `len = 3`, in the segment, so its loops (±1.5 tiles) never reach the landing tiles (205–206) or the take-off of the 3-gap (212–214); 3 jetty tiles (218–220) land before the boathouse.
   - Taube #1 high above the 3-gap (row 7, above every walking jump arc, Zuckerschock included), spawn ~col 224, meets Resi over 213–217 (D-032); not near the boathouse or the Maibaum. Game-designer confirmation (M2 acceptance, backlog item): from the jetty it can't be stomped (a held running jump's feet stay below its hitbox), but a held running jump can bump into it from below. Accepted: the 3-gap needs only a walking jump, the pigeon is on screen well before the take-off, and it is the stage's „look up" moment, not a stomp target.
   - Boathouse: the `boathouse` prop (back layer, col 223, `dx = 8`) on the jetty deck 221–226 (`jetty` row 15, `jetty_post` row 16), with `invisible` collision in its drawn 2/4/5 profile (woodpile 221, stair tower 222, roof ridge 223–226, D-038), its roof 6 tiles above the meadow; meadow one row lower from 227. **Maibaum at col 230**, Ortsende „Brunnwinkl" at 233, level end 239. Running held jump off the roof → Wurstkranz (D-038).

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

## Stage 4: Falkenstein (Milestone 3 scope)

**Premise / joke line**: Bus 150 doesn't stop on the north shore, the boat is full, so Resi does what pilgrims have done since 1291: she walks up the Falkenstein. The way is marked trail no. 28 and is steep. Very steep. At the top St. Wolfgang threw his Hackl about 5 km to pick the spot for his church. Resi gets a Hackl too and throws it at whoever's in the way, which is 300 000 pilgrims, a few chamois and the mountain itself.

Length **240 columns** (3840 px) and **42 rows** high (672 px, the first stage that is taller than one screen; D-053). `camera = "climb"` (D-054). The stage climbs **29 tiles** (464 px, ~2.6 screens). It goes from the Fürberg landing on the lake (ground top row 40) up the Kreuzweg gorge and along the foot of the Falkensteinwand, then up a vertical shaft in the wall to the church plateau (row 11), and finally along the rocky saddle to the Maibaum (meadow row 16). Real route: Fürberg (538 m) → Kreuzweg (trail 28) → Falkensteinkirche with the Schliefstein → Brunnkapelle → Hackelwurfkapelle → the saddle with the Aberseeblick (the trail goes on down to Ried; the stage ends before that descent, which is backlog). `time_limit = 240` provisional (D-045: the level-designer sets 3 × the measured relaxed run at acceptance; climbing and the rock waits make this stage slower than its length suggests). `min_jump_window = 5` (D-053). Oblaten target **70 ± 10** (Salzburg 81, St. Gilgen 85, Brunnwinkl 53). Background: `bg_sky_salzburg` (reused, `factor_y` 0), `bg_wolfgangsee` (reused, factor 8, `factor_y` 3: the lake sinks as Resi climbs), `bg_falkenstein` (new near layer: pines and limestone outcrops, factor 13, `factor_y` 8). `factor_y` is new (D-054).

New in this stage: **vertical climbing with the climb camera** (it settles on the ground, follows falls only a little way, and a fall below the screen kills), **jump-through wooden planks in a vertical shaft** (`steig`, one-way; there are no ladders, D-055), **the Hackl power-up** with thrown axes (D-056), **the Durchkriechstein** (D-057), and **Pilger**, **Gams** and **Steinschlag** with rockfall nets (D-058). Reused: Mozartkugel, Zaunerstollen, `taube_sitting` (the pigeon on the monument gag), checkpoint, Maibaum. Not reused: water pits (the lake is far below), Gondel, Blasmusik, bees, E-Biker.

### Vertical layout (rows are the top tile of the walking surface)

| Section | Columns | Ground |
|---|---|---|
| Fürberg | 0–27 | row 40 (= Stage 1's street, two rows above the level bottom) |
| Kreuzweg | 28–79 | 40 → 28 in steps of 1–3 tiles |
| Steinschlag | 80–117 | 28 → 26 |
| Falkensteinwand (shaft) | 118–131 | path 26 → planks 23 / 20 / 17 / 14 → plateau 11 |
| Falkensteinkirche | 132–169 | plateau 11 |
| Hackelwurf-Sattel → Maibaum | 170–239 | saddle 11–13, launch top row 10, meadow row 16 |

Under every walking surface the mountain is solid (`kalkfels`) down to the level bottom, except for the two lethal clefts (sec. 3 and 6), which are empty down to the bottom (≥ 10 tiles deep, D-054). The goal area's rows 0–16 copy Stages 1–3's vertical layout (meadow row 16, launch 6 tiles above, 3 empty rows on top), so the goal camera frames it exactly like the other stages.

### Climbing rules (stage 4; in addition to the stage 1–3 reach rules)

| Thing | Rule |
|---|---|
| Step-up, required | ≤ 3 tiles (walking held jump reaches ~4); 4-tile step-ups optional only |
| Jump up through a one-way plank (`steig`, `netz`) | plank top ≤ 3 tiles above the standing surface; required planks ≥ 3 tiles wide (the shaft's are 5), optional ones ≥ 2 |
| Gap over a lethal cleft | ≤ 4 tiles, landing ≥ 2 wide, landing at the same height or ≤ 1 tile higher; at most two clefts in the stage |
| Walking off a ledge (drop) | required drops ≤ 2 tiles (the lower ground is on screen at the settle framing); optional drops ≤ 6 tiles; **drops of 7–9 tiles are forbidden** (survivable but she lands off screen); ≥ 10 tiles or bottomless = a cleft (death at the death line). Checked by the fall-depth lint (D-054) |
| Missed jump | every required upward jump: a miss lands ≤ 4 tiles below the take-off on non-lethal ground (no long falls back down the mountain) |
| Visibility | at the settle framing (feet at screen y 148), ~7.5 tiles above the feet are visible under the HUD (126 px) and 2 below: every required landing (≤ 3 up, ≤ 2 down) and every enemy that can reach it is on screen at take-off, so no blind jumps upward |
| Rock spawners | the niche cell's bottom is 4–6 tiles above the ground the rock hits (then the whole niche is on screen below the HUD for a player standing on that ground); the first spawner of the stage drops onto a net |
| Shaft | inner width ≤ 9 columns (then the camera never has to scroll past the shaft's left wall, so every plank stays reachable); entered at the bottom left, left at the top right; nothing outside its columns can be reached before the exit; ≤ 1 enemy in it |
| Moving hazards | at most one moving hazard acts on any required jump (D-041; a pilgrim group counts as one); no rock column (spawner) above the landing of a required jump that already has another hazard |
| Jump window | `min_jump_window = 5` (stages 1–3: 6; the design rule allows 4 from stage 4 on, we ramp gently) |
| Required landings | ≥ 2 tiles wide; 1-tile landings only over non-lethal ground (as in stage 3) |

### Climb camera (`[meta] camera = "climb"`, D-054)

Horizontal behaviour is unchanged (`follow_x = 144`, never scrolls left except D-051). Vertically it is SMW-style: it settles on the ground she stands on instead of following every jump. Today's dead-zone camera would bob up and down with every jump in a tall level, and after a climb it leaves only ~3 tiles visible above her head. Every `camera = "side"` level (all M1/M2 levels, the default) keeps today's dead-zone camera exactly.

| Rule | Value (tuning `[camera]`) |
|---|---|
| **Grounded** (on a tile, a one-way tile or a moving platform; also during the Durchkriechstein crawl) | target `camera.y = feet − climb_feet_y` with `climb_feet_y = 148` (the framing of the street in Stages 1–3); the camera moves towards the target by at most `climb_settle_speed = 2` px per frame. The feet height of the last grounded frame is remembered as `ground_feet` |
| **In the air, upwards** | if the hitbox top is above screen y `climb_top = 40`: `camera.y = top − 40` at once (high bounces stay under the HUD) |
| **In the air, downwards** | if the feet are below screen y 148: `camera.y = feet − 148`, but never more than `climb_fall_follow = 80` px below the settle target of `ground_feet` (`camera.y ≤ ground_feet − 148 + 80`) |
| **Death line** | when the player's hitbox top is below the screen bottom (`top > camera.y + 180`) she dies like falling out of the level (same death, same cause). With the numbers above: drops up to ~8 tiles survive (6 tiles lands at screen y 164, on screen); a cleft ≥ 10 tiles deep kills ~40 frames after the take-off, without a long fall to the level bottom |
| Clamp | to the level as always; at the level bottom the old "below the level" death still applies |
| Start / respawn | the camera starts at the settle target (no settling animation) |
| Goal sequence | x fixed (D-040); y settles on her feet with `climb_goal_feet_y = 164` at 2 px/frame, so the goal meadow ends framed exactly as in Stages 1–3 (pole base at screen y 164, crown in the D-029 HUD gap) |
| Activation | in climb levels an entity activates when, as before, it is within `activate_margin` horizontally **and** its spawn cell is within 64 px (4 tiles) of the screen vertically. Without this, pilgrims and chamois high up the mountain would wake up off screen and come down unseen |
| Parallax | background layers may set `factor_y` (sixteenths, 0..=32, default 0): the layer is drawn `(max camera y − camera.y) × factor_y / 16` px lower than today's bottom-aligned position, so it sits exactly where it does today when the camera is at the level bottom and sinks as Resi climbs. Layers only ever move down, so the opaque sky behind them never shows a gap |

**Risk and fallback** (engine's call, record it in `decisions.md` if taken): if the settle camera turns out to be fiddly, ship the simpler variant instead. It uses the existing dead-zone follow with a per-level dead zone (`[meta] camera_deadzone = [112, 136]`, which keeps ~6 tiles visible above a standing Resi) plus the death line and the vertical activation rule. The camera then bobs with jumps, but everything stays fair. The design rules above hold for both variants.

### Sections

| # | Section | Columns | New mechanic (introduced safely) | Enemies | Landmarks / jokes |
|---|---|---|---|---|---|
| 1 | **Fürberg** | 0–27 | Pilger (one, on flat ground) | Pilger ×1 | Fürberg landing (jetty, the boat line's stop), Gasthof Fürberg, `bus` sign „Bus 150 / nur z'Fuß"; trail sign „Weg 28 / steil. Sehr." |
| 2 | **Kreuzweg** | 28–79 | the climb camera (stairs up); a pilgrim group coming down the steps; jump-through planks (Holzstiege) over solid ground | Pilger ×6 (one group of 3, one pair, one single) | the steep forest gorge with wayside shrines (`bildstock`), „Bitte Stein / mitnehmen" at a pile of stones (pilgrims carried stones up as penance), „Pilgerstau / seit 1291" |
| 3 | **Steinschlag** | 80–117 | falling rocks with their telegraph; rockfall net as a shelter; Gams on an unreachable shelf; the first lethal cleft | Steinschlag ×4, Gams ×1 | overhanging limestone wall with niches, „Steinschlag / inklusive", „Gamswild / hat Vorrang"; checkpoint (col 114) + `bus` sign „Bus 150 / winzig, voll" (the bus is visible far below on the other shore) |
| 4 | **Falkensteinwand** | 118–131 | the vertical shaft (jump-through planks, camera only goes up) | Pilger ×1 (comes down the zigzag) | the wall's wooden stairway bolted to the rock (`steig`), Oblaten arcs between the planks |
| 5 | **Falkensteinkirche** | 132–169 | **Durchkriechstein** (optional crawl) | none (a rest) | the church built against the rock (1626), the Brunnkapelle, a heap of carried stones, „Passt jeder / ohne Sünden", the Einsiedelei bonus cave with the Zaunerstollen and „Einsiedelei / zu vermieten" |
| 6 | **Hackelwurf-Sattel → Maibaum** | 170–239 | **Hackl** power-up and throwing; Gams on the path | Pilger ×3 (one group), Gams ×2, `taube_sitting` ×1 | Hackelwurfkapelle with the statue of Wolfgang throwing the axe (a pigeon on his head), „Hacklwurf / Rekord: 5 km"; the Aberseeblick rock as the launch, Maibaum, Ortsende „Falkenstein" |

Section details (for the level-designer; exact tiles are theirs within these rules; columns are targets):

1. **Fürberg (0–27)**: ground row 40, no pits, no way to die except the timer. Start at col 2 on the Fürberg jetty (`jetty` deck on row 40 over `jetty_post`/`wolfgangsee_water` rows 41; the left level edge is the wall). „Bus 150 / nur z'Fuß" `bus` board at col 5 (clear of Resi at the start, E12 rule). Gasthof Fürberg (`gasthof_fuerberg`, back layer) behind 8–15. 5 Oblaten at walking height (7–11). `M`-Oblatenkiste at col 13, row 36 (4 up) with bricks either side and a 2-tile garden wall (`stone`) at 17 so the Mozartkugel turns back (the Stage 1 pattern). „Weg 28 / steil. Sehr." sign at 20, where the trail leaves the shore. **Pilger #1** at col 25 on flat ground, ≥ 6 free tiles either side: the first pilgrim is met where it can't hurt.
2. **Kreuzweg (28–79)**: the forest gorge (`forest_path` steps, `kalkfels` outcrops), rising from row 40 to row 28. Terraces ≥ 4 wide, steps 1–3 tiles (the first three steps 1–2 tiles: the climb camera is introduced on easy stairs). Three or four `bildstock` wayside shrines on terraces (decoration only; faith is not the joke). „Bitte Stein / mitnehmen" sign at col 32 beside a `steinhaufen` prop at the foot of the climb (pilgrims carried stones up to the church).
   - „Pilgerstau / seit 1291" sign at ~44, on the terrace below a long terrace (≥ 10 wide, 2 tiles higher) with **pilgrim group A** (3 Pilger, the leftmost one with `lead = true`, one cell apart) walking left. They come down the step towards Resi one after the other (pilgrims walk off ledges): wait below them and stomp the chain (100 / 200 / 400), or jump up behind them. The step is ≤ 2 tiles so their drop and Resi's view are both on screen.
   - **Holzstiege** (~60–70): two `steig` planks (one-way, 5 wide, overlapping by 2 columns) 3 and 6 tiles above a terrace lead up to the next terrace. There is solid ground under all of it, so a missed jump lands on the plank or terrace below (≤ 3 tiles): the shaft's mechanic, introduced where nothing can go wrong. Oblaten above each plank.
   - Pilger pair B (#5–6) on a terrace near 72 and single #7 on the top terrace (~77); a `?`-box over a terrace. Kreuzweg ends at row 28.
3. **Steinschlag (80–117)**: the path runs along the foot of the Falkensteinwand under an **overhang**: solid `kalkfels` from the level top down to 7 tiles above the path, with niche cells cut one row into its underside for the spawners (niche bottom 6 tiles above the path, within the 4–6 rule, so it's on screen below the HUD at the settle framing). Nothing required happens under the overhang above 5 tiles, so no jump bumps it.
   - „Steinschlag / inklusive" sign at col 81.
   - **Rock #1 (intro)** at col 88 with `range = 9`, over a `netz` shelter (one-way rockfall net, 3 tiles above the path, cols 86–90): the first rock wobbles, its pebbles trickle down the column, then it falls and shatters **on the net** while Resi watches from the path. She walks under the net safely (Zuckerschock fits: 48 px of headroom). This teaches rocks, the telegraph and the shelter at once.
   - **Rock #2** at col 95 over open path (`period` 150): wait for the shatter, then walk on. „Gamswild / hat Vorrang" sign at 97.
   - **Gams #1** on a rock tongue: the overhang reaches down at col 104 to a solid `kalkfels` shelf 3 tiles above the path (99–104; no floating blocks, the shelf hangs from the cliff). The Gams hops back and forth on 99–103 (the wall at 104 and the ledge end at 99 turn it, it never leaves the shelf) and can't reach the path: walk under it safely (3 tiles of headroom), or jump up from the path's left side to stomp it (200, optional). The shelf also shelters the path below it from rocks. The overhang above the shelf stays 4 tiles up (the Gams' hop reaches 2.9 tiles above its floor).
   - **Rocks #3 and #4** at cols 106 and 109, `period` 150, `phase` 75 apart: a timing run across open path, with a two-step up (row 28 → 26) at ~105 that is not under either column.
   - **Cleft** (the „Abbruch", 3 tiles, ~111–113): empty down to the level bottom (13 tiles, a death below the screen). It's a walking jump with flat take-off and landing at row 26, and no rock column within 2 tiles of it (one hazard per jump).
   - **Checkpoint at col 114** (row 26) with the „Bus 150 / winzig, voll" `bus` sign right of it (centred ~117, not covering the pole). From here the far shore with the road is in the background: the bus is tiny and still full (the D-039 adjacent-sign rule).
4. **Falkensteinwand (118–131)**: the shaft. Left wall col 120 (solid from the level top to row 22; the entrance below it, rows 23–25, is 3 tiles high), right wall cols 130–131 (solid from row 11, the plateau surface, down to the bottom). Inner columns 121–129 (9, the shaft rule). The interior is the decorative back wall `kalkfels_bg` (drawn darker and flatter than solid rock, D-038).
   - The path (row 26) continues as the shaft floor. **`M`-Oblatenkiste at col 123, row 22** (within 10 columns of the checkpoint, as in every stage): the Mozartkugel rolls in the shaft floor between the walls and can't be lost.
   - `steig` planks (one-way, 5 wide): row 23 cols 121–125, row 20 cols 125–129, row 17 cols 121–125, row 14 cols 125–129; then a 3-tile step from the row-14 plank onto the plateau at col 130 (row 11). Each jump is 3 tiles up and through the plank above. A miss lands on the plank (or floor) she jumped from. Overlapping by one column means a pilgrim walking off a plank's end lands on the plank below.
   - **Pilger #8** starts on the row-14 plank and walks the zigzag down (off each plank end onto the next). He is visible from the plank below the whole way. Wait at the far end of a plank while he drops past, or stomp him when he lands on her level.
   - Oblaten: a short arc above each plank (collected on the way up), ~12 in the shaft.
   - No side niches and nothing reachable outside 121–129 before the exit (the shaft rule).
5. **Falkensteinkirche (132–169)**: plateau at row 11, flat, **no enemies and no rocks** (a rest after the wall, and nothing can reach the stone's exit).
   - `falkensteinkirche` prop (back layer) at 134–139 against the rock: a small white church built into the cliff, under a `kalkfels` overhang. The `steinhaufen` prop (the stones the pilgrims carried up) at 141. `brunnkapelle` prop (back) at 144.
   - **Einsiedelei** (optional, the stage's Zaunerstollen): a cave in the cliff above the plateau, floor 4 tiles up (row 7), 3 wide (~159–161), 2 tiles high (rows 5–6, `kalkfels_bg` interior), reached by a held 4-tile jump straight from the plateau (optional, so the 4-tile step-up is allowed). The middle tile of its ceiling (row 4) is a `Z`-Oblatenkiste (`zaunerstollen`), hit by a hop inside the cave in either size; the cave's walls stop the rolling Zaunerstollen. „Einsiedelei / zu vermieten" sign on the plateau below it (hermits lived here from 1659 to 1811; holiday-flat prices are a fair target).
   - **Durchkriechstein** at cols 150–153: the `durchkriechstein` prop (back layer) over `invisible` solid tiles in rows 8–9, cols 150–153 (the boulder's top is 3 tiles above the plateau: the required route climbs over it), and the tunnel row 10, cols 150–153, left empty (`durchkriechstein` entity in cell (150, 10), `len = 4`). DOWN at the mouth (col 149, facing right) starts the crawl; the exit (col 154) has ≥ 2 empty rows. „Passt jeder / ohne Sünden" sign at col 146, left of the mouth, not covering it. The legend is the joke: the stone was said to let only the sinless through, and then even a large person fits. So big Resi fits too, and comes out lighter.
   - Oblaten on the boulder top and along the plateau.
6. **Hackelwurf-Sattel → Maibaum (170–239)**:
   - „Hacklwurf / Rekord: 5 km" sign at col 170. `hackelwurfkapelle` prop (back layer) at 172–174 with Wolfgang's statue throwing the axe; a **`taube_sitting` on the statue's head** (the pigeon-on-a-monument gag). It rises when Resi is within 6 tiles and flies left over the flat plateau at head height (the D-032 meeting rule: no other hazard there).
   - **`H`-Oblatenkiste at col 176, row 7** (4 up). The Hackl rises out of it and stays put. Flat plateau 170–183 with no enemies within 8 columns of the box (safe pickup).
   - **Pilger group C** (#9–11, `lead = true` on the leftmost) at 188–190 walking left over flat ground 180–193: the first Hackl targets, approaching slowly from 12+ tiles away (the axe flies ~7 tiles). Without the Hackl they're a stomp chain as in sec. 2.
   - The saddle (194–222): rocky ups and downs between rows 11 and 13 (steps ≤ 2). **Gams #2** on a flat stretch 195–200 on the path (it hops at Resi: throw, stomp, or pass while it stands). A **cleft** (3 tiles, 207–209, empty down to the bottom) with no enemy within 6 columns of either edge (flat take-off 201–206, landing 210–215). **Gams #3** on a flat stretch 216–221.
   - **Aberseeblick** (the launch, 223–226): a rock tower whose top is row 10 (6 tiles above the meadow), climbed by ≤ 3-tile steps from the saddle. Meadow (row 16) from 227, **Maibaum at col 230** (lowest ground of its screen, D-029), Ortsende „Falkenstein" at 233, level end 239 (camera clamps at the goal, D-031). Running held jump off the Aberseeblick → Wurstkranz (D-038); walking off → 100 (a 6-tile drop: allowed as optional by the drop rule; the pole column catches her anyway). Nothing flies or walks near this jump (Gams #3 can't leave its stretch).

### Entities and behaviour (new in Stage 4)

Same conventions as Stages 1–3 (px/frame at 60 Hz, distances horizontal centre to centre, frame counters from activation or from entering range, never the RNG). Numbers go to `tuning.toml`; key names are the engine's choice.

| Entity | Sprite / hitbox | Behaviour | Stomp? | Points |
|---|---|---|---|---|
| **Pilger** (`pilger`, optional `lead = true`) | 16×24 / 12×22 (`lead`: 16×32 sprite with a pennant on his stick, same hitbox) | Walks 0.5 towards the player at activation (left); turns at walls and other enemies on foot; **walks off ledges** (gravity and max fall speed as the player's), so on the Kreuzweg and in the shaft pilgrims come down the mountain step by step. Groups are 2–3 pilgrims placed one cell apart, which stay in file at equal speed. A pilgrim carries a penance stone on his back (art only). `lead` only changes the sprite (the group's pilgrim leader with a pennant on his stick, like a tour guide's umbrella). | yes | 100 |
| **Gams** (`gams`, chamois) | 24×20 / 16×14 | Ledge-bound hopper. Cycle from activation: **stand 60** frames → **crouch 16** (the telegraph: head down, legs bent) → **hop**: vx ±1.0, vy −4.0, gravity 0.25 (apex 32 px = 2 tiles, airtime 32 frames, lands 32 px = 2 tiles further) → stand again (period 108). At the start of each crouch it faces the player, then checks the **landing cell** 2 tiles ahead at its current floor height: if there is no solid or one-way top there (ledge end) or a wall is in the way, it turns and checks the other side; if both are blocked it hops straight up (vx 0). So it never leaves its ledge. During the hop it collides with tiles (lands on solid and one-way tops, a wall stops vx). Activates like other enemies; no range trigger. | yes | 200 (starts a chain at 200, D-046) |
| **Steinschlag** (`steinschlag` spawner; fields `period` 90–300, default 150; `phase` 0 or 41..period−1, default 0; `range` 4–10 tiles, default 6) | niche: draws a resting `stein` | Its counter runs **only while the player is within `range` tiles horizontally and the spawner cell is fully on screen below the HUD** (screen y ≥ 24). Out of range it pauses and keeps its pose. Counter frame f of each `period`; the counter starts at `phase` when the player first comes in range, and `phase` is 0 or 41..period−1 (validated), so the first rock of an approach always follows a full telegraph. f 0–39 **telegraph**: the rock wobbles in its niche and pebbles (`kiesel`, renderer only) trickle down the whole column to the first solid or one-way tile below. At f 40 the rock drops. The niche shows a resting rock again from 30 frames after the drop. | – | – |
| ↳ **Stein** (falling rock) | 16×16 / 12×12 | Falls straight down from the niche: vy from 0, gravity 0.25, max fall 4.0. **Shatters on the first solid or one-way tile** (4 `stein_debris` pieces like a broken brick) or is removed below the level. Hurts on any contact, also from above (it can't be stomped; a jump onto it is a hurt). It passes through enemies and items. A thrown Hackl shatters it (both gone, 50 points). | no (hurts) | 50 (Hackl only) |
| **Rockfall net** (tile `netz`) | 16×16 tile | One-way (the player and enemies stand on it and jump up through it); rocks shatter on it like on any one-way tile. Drawn as a steel-cable net on posts. Used 3 tiles above a path as a shelter. | – | – |
| **Hackl** power-up (`hackl`) | 16×16 / 12×12 | From an `H`-box (`{ tile = "oblaten_box", contains = "hackl" }`): rises out of it over 32 frames (0.5/frame), then **stays where it is** (it doesn't roll; SMB's fire flower). Touch: Normal or Zuckerschock → **Hackl** (30-frame transformation freeze, alternating sprites; growing from Normal keeps the feet, D-024 applies), popup „Wirf, Resi, wirf!" plus the score popup. Already Hackl: points only. Removed when it falls behind the despawn margin like other items. | – | 1000 |
| ↳ thrown **Hackl** (`hackl_thrown`) | 12×12 / 8×8 | **A new press of RUN** while Hackl (Playing, not frozen, not crawling, not in the goal sequence) throws one if fewer than **2** are alive; holding RUN still runs. Throw pose 10 frames. Spawn: centre 8 px in front of the player's centre (facing side), 10 px below her hitbox top. Fixed arc, independent of her speed: **vx ±3.0, vy −3.0, gravity 0.1875** (apex 24 px = 1.5 tiles at 16 frames, back at hand height after 32 frames and 6 tiles; on flat ground it lands ~7 tiles ahead), max fall 4.0. Spins (`spin`). **Solid tile → sticks** (`stuck` pose, harmless, removed after 20 frames); passes through one-way tiles; water or a hazard tile, or leaving the screen at any edge → removed. **Hits**: every stompable enemy (Selfie-Tourist, Taube, Verkäufer, Nordic-Walker, Blasmusiker, E-Biker, Pilger, Gams) is defeated (tumbles off the screen like a knocked enemy) for its base stomp value (100/200, outside the stomp chain); the Fiaker: the axe bounces off (removed, no effect); a Bienenschwarm, thrown Mozartkugeln and notes: passes through; a falling Stein: both destroyed, 50. One target per axe; blocks and items are not affected. | – | target's |
| **Durchkriechstein** (`durchkriechstein`, field `len` 2–6 tiles, default 4) | trigger in the tunnel's first cell | A 1-tile-high tunnel through a boulder at ground level, `len` cells to the right of and including its cell. **Start**: while Playing, the player is grounded on the tunnel floor's height, facing right, her hitbox's right edge within 4 px of the tunnel mouth, **DOWN held and JUMP not**, and the stone is unused this attempt. **Crawl** (scripted): her hitbox becomes 12×12, she moves right at 1 px/frame through the tunnel until her hitbox's left edge is past its right end (len × 16 + 12 px; 76 frames for `len` 4), then stands up with her feet on the floor. Input is ignored (JUMP still tracked, D-025), she can't be hurt, enemies keep moving, the timer runs, the camera follows. Turning back is impossible (the tradition: no looking back). **Outcome** at the exit: Normal → **+1 life** (popup „+1 Leben"; at the lives cap 1000 points instead); Zuckerschock or Hackl → **shed to Normal** (30-frame transformation freeze, no invulnerability frames), popup „Ballast weg!", no points. Then the stone is used until the level resets (a death resets it, as boxes; one bonus per attempt, so dying to farm lives only breaks even). | – | – |

Tuning targets for the record: a Gams cycle is 108 frames (1.8 s); a rock dropped from 6 tiles hits the path ~32 frames after the telegraph ends (~72 after it starts); the default rock period leaves ~118 frames between a shatter and the next drop. Enemy count: Pilger 11, Gams 3, Steinschlag 4, `taube_sitting` 1.

### New mechanics: purpose, safe introduction, test

| Mechanic | Purpose | Introduced safely | Test |
|---|---|---|---|
| Climb camera (settle, fall limit, death line) | a stage that goes up; you see where you jump | the Kreuzweg's first steps are 1–2 tiles on solid stairs | unit tests: settle speed and target (feet at 148), in-air follow up (top < 40) and down (limit 80 px below the last ground's framing), death line (hitbox top below the screen bottom kills, Normal and big), clamp, respawn framing, a 6-tile drop survives and a 10-tile one kills, `camera = "side"` levels unchanged (every M1/M2 replay and golden passes unchanged); goal camera frames the meadow at 164; lint: fall depth 7–9 tiles rejected; replays: fall into the sec. 3 cleft → death without falling to the level bottom; walk off a 6-tile ledge (the Maibaum launch) → survives |
| Vertical parallax (`factor_y`) | the lake sinks as Resi climbs | – | render unit test: offset = (max camera y − camera y) × factor_y / 16, 0 at the level bottom; goldens at the foot and at the top of the wall |
| Jump-through planks / the shaft | vertical climbing without ladders | the Holzstiege over solid ground in sec. 2 | solver finishes the shaft (climb progress metric, D-054); windows ≥ 5; replay: miss a shaft jump → lands on the plank below; replay: Pilger #8 walks down the zigzag |
| Steinschlag | read a telegraph from above, find shelter | rock #1 shatters on a net in front of Resi | unit tests: counter only in range and with the niche on screen, telegraph 0–39, drop at 40, gravity/max fall, shatters on solid and one-way, hurts from above, passes enemies, `phase`/`period`; replays: shelter under the net, hit as Zuckerschock (shrinks), Hackl shatters a rock (50); solver windows ≥ 5 at rocks #3/#4 |
| Pilger walking down steps | enemies that come down the mountain at you | single pilgrim on flat ground first, then a group on a 2-tile step | unit tests: walks off ledges, turns at walls/enemies, a group stays in file; replay: stomp chain on group A (100/200/400) |
| Gams | a hopping enemy with a readable crouch | Gams #1 on a shelf it can't leave, above the path | unit tests: cycle 60/16/32, arc (2 tiles up, 2 across), turns at ledge ends and walls, hops in place when boxed in, never leaves its ledge, stomp 200 with chain start 200, knocked off by a bumped block; replays: stomp #1 from the path, hit by #2 as Zuckerschock |
| Hackl power-up + throwing | St. Wolfgang's legend as a verb; a ranged answer to the stage's enemies | box on a calm plateau, pilgrim group C approaching slowly from 14 tiles | unit tests: item rises and stays, Normal → Hackl and Zuckerschock → Hackl (freeze, feet kept), Hackl hit → Zuckerschock + 120 i-frames, throw on RUN press only (holding RUN throws once), max 2 alive, arc numbers, sticks in solids, passes one-way, hits per kind (stompable defeated + base points, Fiaker bounce-off, swarm/projectiles pass, rock 50), no throw while frozen/crawling/in the goal; replays: get the Hackl, defeat group C with axes, get hit (→ Zuckerschock), carry Hackl to the route map and the teaser (`player_hackl` drawn) |
| Durchkriechstein | the custom as an optional gamble: shed your power, or gain a life if you're Normal | optional, beside the required route over the boulder, on an enemy-free plateau | unit tests: start conditions (grounded, facing, within 4 px, DOWN without JUMP, unused), scripted move and length, no hurt during it, outcomes per state, life cap → 1000, used once per attempt, reset on death; lint: tunnel cells empty with solid above and below, mouth floor and exit headroom (2 rows); replays: crawl as Normal (+1 life), as Zuckerschock (→ Normal, „Ballast weg!"), as Hackl (→ Normal), a big Resi at the mouth without DOWN is never stuck (walks back, climbs over) |
| Checkpoint before the shaft | medium difficulty with a long climb | automatic | replay: die in the shaft after the checkpoint → respawn at 114 as Normal, level reset (`M`-box fresh) |

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
| `stage.brunnwinkl.sign_*` | `sign_bus` „Bus 150 / fährt drüben", `sign_gehweg` „Gehweg / nicht Radweg", `sign_haeuser` „Fünf Häuser / am See", `sign_muehle` „Frischluft / seit 1882", `sign_tanzschule` „Tanzschule / Brunnwinkl", `sign_checkpoint` „Bus 150 / winkt rüber", `sign_futter` „Futter nur / auf Blau", `sign_nobelpreis` „Nobelpreis / für alle", `sign_ortsende` „Brunnwinkl" |
| `popup.hoppala` | Hoppala! (an e-biker crashes into a wall) |
| `stage.falkenstein.title` / `.tagline` / `.next` | Etappe 4: Falkenstein / Steil. Steiler. Hackl. / Auf nach St. Wolfgang. |
| `stage.falkenstein.sign_*` | `sign_bus` „Bus 150 / nur z'Fuß", `sign_weg` „Weg 28 / steil. Sehr.", `sign_steine` „Bitte Stein / mitnehmen", `sign_pilgerstau` „Pilgerstau / seit 1291", `sign_steinschlag` „Steinschlag / inklusive", `sign_gams` „Gamswild / hat Vorrang", `sign_checkpoint` „Bus 150 / winzig, voll", `sign_durchkriech` „Passt jeder / ohne Sünden", `sign_einsiedelei` „Einsiedelei / zu vermieten", `sign_hackl` „Hacklwurf / Rekord: 5 km", `sign_ortsende` „Falkenstein" |
| `popup.hackl` | Wirf, Resi, wirf! (the Hackl is picked up) |
| `popup.ballast` | Ballast weg! (a big Resi comes out of the Durchkriechstein as Normal; not „Erleichtert!", which also reads as relieving oneself, D-052 rule) |
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

## Sources (facts used in Stage 4)

- Falkenstein pilgrim path St. Gilgen → Fürberg → over the Falkenstein → Ried → St. Wolfgang, waymarked as trail no. 28 from the Fürberg car park, lowest point 538 m, highest 807 m, ~200 m of steep ascent, 24 signposted points of interest (behind „Weg 28 / steil. Sehr.") – [salzkammergut.at: Der Falkenstein-Pilgerweg](https://www.salzkammergut.at/en/oesterreich-tour/detail/430010480/der-falkenstein-pilgerweg.html)
- From Fürberg the Kreuzweg climbs a steep wooded gorge to the Falkensteinkirche, which is built above a „Durchkriechstein"; stones that pilgrims carried uphill as penance, cup-marked stones read as traces of the saint and the devil, the Quellkapelle, a rocky saddle with chapels and the Aberseeblick, a waterfall and the descent to Ried; Wolfgang threw his axe from the Falkenstein to choose the church site ~4 km away; in the Middle Ages St. Wolfgang ranked with Rome and Santiago, and most pilgrims came over the Falkenstein – [oberoesterreich.at: Der Weg der Wallfahrer](https://www.oberoesterreich.at/oesterreich-tour/detail/100275/der-weg-der-wallfahrer.html)
- Falkensteinkirchlein: a Wolfgang chapel documented 1350; the present church built 1626 around a cave in the Falkensteinwand (Johann Wilhelm Lueger, Pfleger of Hüttenstein); the crawl-through stone (Schliefstein) was passed in silence and without looking back, and was said to let even a large person through if free of sin; ~300 000 pilgrims in peak years of the 16th century; hermits lived by the church 1659–1811; the Brunnkapelle (1669) over Wolfgang's spring; the Hackelwurfkapelle where he threw the axe, with a figure of Wolfgang throwing it – [Austria-Forum: Falkensteinkirchlein](https://austria-forum.org/af/AustriaWiki/Falkensteinkirchlein)
- Customs still practised in part: carrying stones (Steinetragen), the echo call, crawling through the stone cave, washing the eyes, turning an old cult stone – [salzkammergut.at: Falkensteinkirche](https://www.salzkammergut.at/oesterreich-poi/detail/430010230/falkensteinkirche.html?type=2026)
- St. Wolfgang (born 924, Bishop of Regensburg) withdrew to the Abersee (the old name of the Wolfgangsee) in 976; he threw his axe from the Falkenstein, and it landed ~5 km away on a rock above the lake where the market St. Wolfgang grew (behind „Hacklwurf / Rekord: 5 km" and the Hackl power-up); first pilgrimages documented around 1291 (behind „Pilgerstau / seit 1291") – [SalzburgerLand: 1100 Jahre heiliger Wolfgang](https://www.salzburgerland.com/de/magazin/1-100-jahre-heiliger-wolfgang/)
- The Wolfgangsee boat line stops at Fürberg and Ried-Falkenstein (the Fürberg landing at the start) – [salzkammergut.at: Wolfgangsee-Schifffahrt](https://www.salzkammergut.at/oesterreich-poi/detail/400639/wolfgangsee-schifffahrt.html)
- Common knowledge (no source needed): chamois (Gämsen) in the Northern Limestone Alps and the „Gamswild"/„Wildwechsel" traffic-sign idiom; „Achtung Steinschlag" signs and rockfall nets on Austrian mountain paths; the Postbus 150 runs along the south shore (Stages 2–3 sources), so the north-shore pilgrim path has no bus.

## Resolved decisions (human, 2026-10-18)

1. **Title**: „Oblatenjagd"
2. **Hero**: Resi in a Dirndl (one playable character)
3. **Real names**: real places, businesses and historical people (Café Zauner, Postbus, Fürst Mozartkugeln, Kaiser Franz Joseph, …) may be used by name – it's a private project.
4. **Difficulty**: medium (see "Difficulty: medium" above)
