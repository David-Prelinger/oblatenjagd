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

### Difficulty: medium

Precise SMB-style platforming, but without SMB's harsh punishment:
- 3 lives at the start; extra lives from 100 Oblaten and Zaunerstollen.
- One checkpoint per stage; dying respawns there.
- Game over → „Weiter?" restarts the **current stage** with 3 lives (unlimited continues; the score is reset).
- The player's hurtbox is a bit smaller than the sprite, and stomps are generous (landing on an enemy's upper half counts).
- No deaths the player can't see coming: hazards are visible before the jump that leads to them.
- Jump windows (measured by the solver): ≥ 6 frames in stages 1–3, ≥ 4 frames later. Optional bonus routes may be harder.

### Power states

| State | Trigger | Effect |
|---|---|---|
| Normal | start | dies on hit |
| **Zuckerschock** (big) | eat a **Mozartkugel** | taller, breaks bricks, survives one hit |
| **Hackl** (thrower) | grab **St. Wolfgang's Hackl** (introduced in the Falkenstein stage, after the legend of the axe throw) | throws axes in an arc |
| 1-Up | **Zaunerstollen** (rare) | extra life |

### Movement feel (initial targets, tunable in `content/tuning.toml`)

| Metric | Target |
|---|---|
| Walk speed | ~1.5 px/frame |
| Run speed | ~2.5 px/frame |
| Jump height, tap | ~1.5 tiles |
| Jump height, hold, walking | ~4 tiles |
| Jump height, hold, running | ~5 tiles |
| Coyote time | 6 frames |
| Jump buffer | 6 frames |
| Max fall speed | ~4 px/frame |

These targets become automated game-feel tests (`validation.md`).

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

Length ~250 columns (~4000 px, roughly 90–150 s for a good run). Five sections:

1. **Busbahnhof** – start. Sign: „Bus 150 – leider voll". Tutorial: run, jump, first Oblaten, first Oblatenkiste.
2. **Mirabellgarten** – hedges as platforms, Pegasus fountain. First **Selfie-Tourist** enemies (walk back and forth, stomp them). First Mozartkugel power-up.
3. **Makartsteg** – bridge over the Salzach with gaps; falling into the Salzach = death. **Tauben** (pigeons) flying in a sine wave.
4. **Getreidegasse** – narrow street, **Zunftschilder** (guild signs) as one-way platforms, Mozart's birthplace (yellow façade). **Mozartkugel-Verkäufer** throws Mozartkugeln in arcs. Checkpoint here.
5. **Domplatz → Festung** – a **Fiaker** charges along the square (cannot be stomped, jump over it); the Festungsbahn funicular as a moving platform; **Maibaum** goal on the way out of town, with the place-name sign "Salzburg" crossed out in red.

Required for M1: player (normal + Zuckerschock), Oblaten, Oblatenkiste, bricks, one-way platforms, water death, 4 enemy types (Selfie-Tourist, Taube, Mozartkugel-Verkäufer, Fiaker), checkpoint, Maibaum goal, HUD, title screen, stage intro, game over. The Sound-of-Music tour group (a conga line of tourists following a guide's umbrella) is a stretch goal.

## UI text (German, examples)

- Title: „Oblatenjagd" – „Drück START, oida"
- HUD: `OBLATEN`, `PUNKTE`, `ZEIT`, `LEBEN`
- Stage intro: „Etappe 1: Salzburg"
- Game over: „Aus is'! Spiel vorbei."
- Stage clear: „Etappe geschafft! Weiter nach St. Gilgen."

## Resolved decisions (human, 2026-10-18)

1. **Title**: „Oblatenjagd"
2. **Hero**: Resi in a Dirndl (one playable character)
3. **Real names**: real places, businesses and historical people (Café Zauner, Postbus, Fürst Mozartkugeln, Kaiser Franz Joseph, …) may be used by name – it's a private project.
4. **Difficulty**: medium (see "Difficulty: medium" above)
