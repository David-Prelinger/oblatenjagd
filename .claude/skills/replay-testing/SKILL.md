---
name: replay-testing
description: Replay files, the solver bot, golden screenshots and determinism checks - how to record, write, run and update them. Use when testing gameplay, reproducing bugs, or after physics/level changes.
---

# Replay testing

Status: approved v1. Background in `docs/validation.md` (L4–L6).

## Replay format (`tests/replays/<name>.replay`)
```
level  = salzburg
seed   = 1
source = solver | human | handwritten
expect = complete frames=2400..3600 oblaten>=30 deaths=0
shots  = 120:intro 900:getreidegasse
---
R 60        # input flags held for N frames: L R U D J(ump) B(run) S(tart); "-" = nothing
RB 30
RBJ 18
- 10
```
Other expectations: `dies_at frames=a..b`, `deaths<=N`, `player_x>=<px>`, `state=<playing|dying|stage_clear>`, `no_event=<Event>` (`Jumped`, `Landed`, `Died`, `Respawned`, `LevelComplete`).
`oblaten>=N` is not supported until M1 (parse error "not supported yet").

Semantics (D-013): frame N = the state after N steps (`Game::frame()`; frame 0 = before the first step), so `complete frames=731..731` means `LevelComplete` happened in step 731. Ranges are inclusive. The replay always runs to its last input; `state=`, `deaths`, `player_x` are checked at the end. `seed` defaults to 1; `level` is a `meta.id`. `expect` and `shots` may repeat. Shot names: `a-z 0-9 _`; frames ≤ the replay length.

## Commands
- `cargo xtask replay <file|name>` – run it, check expectations, check determinism (runs twice, compares `state_hash` every frame); `--all` runs every `tests/replays/*.replay`
- `cargo xtask replay <file> --shots` – also render the `shots` frames → `$CARGO_TARGET_DIR/preview/replays/<replay>_<shot>.png` (+ `_x4`)
- `cargo xtask solve <level>` – solver bot; writes `tests/replays/<level>_solver.replay` + report
- `cargo xtask filmstrip <file> [--every N]` – every 30th (Nth) frame plus the last in one image
- `cargo xtask golden` – compare all shots with `tests/golden/<replay>_<shot>.png` (the `goldens` verify step)
- `cargo xtask golden --accept <replay>_<shot>|<replay>|--all` – accept new goldens (**only** after art-director APPROVED)
- Human recording: see `play-in-vnc` (`--record <file>`)

## Bug reproduction
1. Write the shortest possible replay that triggers the bug (often handwritten).
2. Set `expect` to the **correct** behaviour, so the replay fails now and passes after the fix.
3. Name it `bug_<short_description>.replay`.

## After a physics or tuning change
1. Run `cargo xtask verify`; expect solver replays to fail.
2. `cargo xtask solve <level>` for each level; check the difficulty report still meets targets.
3. Golden screenshots of affected scenes change → art-director review → `golden --accept`.
4. Explain all of this in the commit message.
