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
- Grey-box test level for items/power states: `content/levels/00_items.level`.

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
- Jump window = how many frames the JUMP press can be shifted (within ±12) while the player still survives (up to the next jump's checkpoint) and gets as far 2 s after landing (16 px tolerance). The `bounds` column says why each side ends: `fail` = the level's real limit, `route` = only the solver's route prevents a bigger shift (window shown as `≥ N`, ignored for the narrowest), `max` = ±12. Read it from the report table; look at the `_narrowest.png` shot.
- `cargo xtask scene <id> --x <px>` – screenshot at a position
- `cargo xtask filmstrip <replay>` – view the solver's run

## Difficulty guidelines (overall: medium, see `game-design.md`)
- Narrowest jump window on the main route (solver report): ≥ 6 frames in stages 1–3, ≥ 4 frames later. Bonus routes may be harder.
- No blind jumps: the landing spot must be visible on screen before the jump.
- Never place an enemy directly at a landing spot in the first two stages.
