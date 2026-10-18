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
- `cargo xtask solve <id>` – proves the stage can be completed; writes a replay and a difficulty report
- `cargo xtask scene <id> --x <px>` – screenshot at a position
- `cargo xtask filmstrip <replay>` – view the solver's run

## Difficulty guidelines (overall: medium, see `game-design.md`)
- Narrowest jump window on the main route (solver report): ≥ 6 frames in stages 1–3, ≥ 4 frames later. Bonus routes may be harder.
- No blind jumps: the landing spot must be visible on screen before the jump.
- Never place an enemy directly at a landing spot in the first two stages.
