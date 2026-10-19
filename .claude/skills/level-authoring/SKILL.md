---
name: level-authoring
description: How to build a stage - the .level format (TOML header + ASCII grid), the legend, pacing of sections, lint and solver commands, and difficulty guidelines. Use when creating or editing content/levels/*.level.
---

# Level authoring

Status: approved v1. Format spec in `docs/architecture.md` → "Level".

## Format reminders
- One char = one 16×16 tile or an entity spawn. Legend in `[legend]`, grid in `[map].grid`.
- All rows must be the same width. Row 0 is the top. Usually 14 rows tall.
- Exactly one `P` (player start), at least one goal (`G`), one checkpoint per stage (`C`).
- Format v2 (D-022, full list in `docs/architecture.md` → "Level"): `"?" = { tile = "oblaten_box", contains = "mozartkugel" }` (contains: `oblate`, `mozartkugel`, `zaunerstollen`, `oblaten_10`; only on `brick`/`oblaten_box`), `"o" = { entity = "oblate" }`, `"C" = { entity = "checkpoint" }`, `"s" = { entity = "sign", text = "stage.salzburg.sign_bus", style = "bus" }` (style `street`/`bus`/`ortsende`), enemies by catalog name. `[meta]` needs `time_limit`; `background = [{ sprite = "bg_rooftops", factor = 8, y = 0 }]` (factor in 1/16 camera speed; optional `y` = px above the screen bottom); optional `min_jump_window`.
- Festungsbahn: `"F" = { entity = "festungsbahn", to = [8, -5] }` — `to` is its other end point in tiles relative to its cell (required); the car (3 tiles wide, 24 px high) starts left-aligned in that cell with its bottom on the cell's bottom edge. It **waits for its rider** (D-030): at its lower end (the end with the larger y) it waits until the player stands on it, travels up 150 frames, waits 60 frames at the top, travels back down and waits for the next rider. A car placed at its upper end (`to` goes down) first travels down. A fallen player can only call the car back while its lower end is on screen (the camera never scrolls back), so the stairs under the track are the real way out. Only its top is solid (from above), so the player boards it from above (keep its top a few px below the boarding ledge) and walks through its sides. `levels lint` requires 3 tiles of free space above the car along the whole track, and stair-stepped rock under it: from the floor under the track to the floor beyond its upper end, no step up higher than 3 tiles (D-027/D-028: falling off is never a softlock). Keep the stairs below the car's path.
- Props (D-030/D-031): `"S" = { entity = "prop", sprite = "sphaera", dx = 8 }` draws a catalog prop sprite (`sphaera`, `pegasus`, `fountain_water`, `zwerg`, `residenzbrunnen`, `geburtshaus`; `levels lint` checks the name, list `jnr_core::PROP_SPRITES`) with its bottom centre on the cell's bottom centre, moved `dx` px (optional, ±64; 8 = centred between two columns). No collision: put solid tiles under/behind it. Default layer front (after the tiles, behind the player); `layer = "back"` draws it before the tiles (façades behind the street). Tile `invisible` is solid but never drawn (under a prop like the Sphaera); `facade` and `arch_under` are solid autotiles, and `arch`/`arch_under` autotile as one family (an arch on its underside shows no edge between them).
- Entity on a tile: an entity's cell is air unless its entry adds `tile = "..."`, e.g. `"w" = { entity = "nordic_walker", tile = "wall_village_bg" }` or `"o" = { entity = "oblate", tile = "wall_village_bg" }` (walkers/Oblaten in front of a house wall without a hole in it). Only non-colliding tiles (`wall_village_bg`, `railing_bg`, `roof_bg`); solid/one-way/hazard tiles under an entity are a parse error. Needs one legend char per entity+tile combination.
- Bienenschwarm: `{ entity = "bienenschwarm", dir = 0..=7, len = 2..=4, phase = N, dy = -8 }` — `dy` (±8 px, default 0, negative = up) moves its anchor from the cell centre; `dy = -8` lifts a swarm anchored just above the ground so its return arcs dip less into it (Brunnwinkl #2–#5); a diagonal run's return can still touch the ground by ~1 px at its low point, see the „E12 art-review notes" in `docs/backlog.md`.
- Enemies patrol on frame counters from activation (one screen + 320 px ahead of the camera), so where they are when the player arrives depends on the player's speed; Selfie-Tourists walk off ledges into pits and turn at walls and other enemies; the Fiaker ignores tiles on its row. `{ entity = "selfie_tourist", still = true }` (never walks) is a fixture for `00_*` test levels only; `levels lint` rejects it in stages.
- Grey-box test levels: `content/levels/00_items.level` (items/power states), `content/levels/00_enemies.level` (enemies, Festungsbahn).

## Jump reach (check `content/tuning.toml`; update this table when tuning changes)
| Situation | Max height | Max gap |
|---|---|---|
| walking jump | 4 tiles (65 px) | ~5 tiles (82 px airborne distance) |
| running jump | 5 tiles (81 px) | ~9 tiles (157 px airborne distance) |

Measured in M0 by `crates/jnr-core/tests/feel.rs`. Gaps "max" means flat ground to flat ground; leave a safety margin of 1 tile in Stage 1.

## Pacing per section
1. **Introduce** a mechanic where failing is harmless (no pit under it).
2. **Develop** it with mild danger.
3. **Twist**: combine it with an earlier mechanic.
4. **Breather**: a few safe screens with Oblaten as a reward.
Place Oblaten to show the intended path (arcs show jumps). Hidden blocks reward exploration, never block progress.

## Commands
- `cargo xtask levels lint <id>` – structure, legend, references, text keys
- `cargo xtask solve <id>` – proves the stage can be completed (beam search over the real simulation, ~0.6 s for the testbed); prints a summary and writes the difficulty report `$CARGO_TARGET_DIR/reports/solver/<id>.{md,json}` (frames, search deaths, every jump with its frame, x, hold and **jump window**, the narrowest one) plus `preview/solver/<id>_narrowest.png`. If it fails: furthest grounded x and frame, and `preview/solver/<id>_stuck.png` there.
- `cargo xtask solve <id> --write` – also writes `tests/replays/<id>_solver.replay`; commit it (the `solver` verify step fails without it). `cargo xtask solve --all` for every level (that is the `solver` verify step)
- Jump window = how many frames the JUMP press can be shifted (within ±12) while the player still survives the jump and can still get as far as the original 2 s after landing (16 px tolerance; D-033): either with the route's later inputs, or by re-planning from the landing with a small beam search (so a later press that would have to move by a frame doesn't count against this jump). The last jump's checkpoint is the finish, so slack there is measured against the route's finish frame + 30. The `bounds` column says why each side ends: `fail` = the level's real limit, `route` = only the solver's route prevents a bigger shift (window shown as `≥ N`, ignored for the narrowest), `max` = ±12. Read it from the report table; look at the `_narrowest.png` shot.
- `cargo xtask scene <id> --x <px>` – screenshot at a position
- `cargo xtask filmstrip <replay>` – view the solver's run

## Difficulty guidelines (overall: medium, see `game-design.md`)
- Narrowest jump window on the main route (solver report): ≥ 6 frames in stages 1–3, ≥ 4 frames later. Bonus routes may be harder.
- No blind jumps: the landing spot must be visible on screen before the jump.
- Never place an enemy directly at a landing spot in the first two stages.
