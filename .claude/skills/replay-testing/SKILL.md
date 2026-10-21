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
Other expectations: `dies_at frames=a..b`, `deaths<=N`, `player_x>=<px>` (hitbox left edge), `player_y>=<px>` / `player_y<=<px>` (hitbox top edge, whole level pixels, y grows downwards), `state=<playing|dying|stage_clear|game_over>`, `event=<Event>` (at least once), `no_event=<Event>` (`Jumped`, `Landed`, `Died`, `Respawned`, `LevelComplete`, `OblateCollected`, `PowerUp`, `Hurt`, `ExtraLife`, `BlockBumped`, `BrickBroken`, `Stomped`, `CheckpointReached`, `GameOver`), `oblaten>=N` / `oblaten=N` (the HUD Oblaten counter at the end, 0..99), `score>=N` / `score=N`, `lives=N` / `lives>=N` (at the end), `power=<normal|big|hackl>`, `swimming=<true|false>`, `boss_hits=N` (the Oberkellner's hits at the end, D-106).
Anything else is a parse error "not supported yet".

Semantics (D-013): frame N = the state after N steps (`Game::frame()`; frame 0 = before the first step), so `complete frames=731..731` means `LevelComplete` happened in step 731. Ranges are inclusive. The replay always runs to its last input; `state=`, `deaths`, `player_x`, `player_y`, `oblaten`, `score`, `lives` are checked at the end. `seed` defaults to 1; `level` is a `meta.id`. `expect` and `shots` may repeat. Shot names: `a-z 0-9 _`; frames ≤ the replay length.

## Commands
- `cargo xtask replay <file|name>` – run it, check expectations, check determinism (runs twice, compares `state_hash` every frame); `--all` runs every `tests/replays/*.replay`
- `cargo xtask replay <file> --shots` – also render the `shots` frames → `$CARGO_TARGET_DIR/preview/replays/<replay>_<shot>.png` (+ `_x4`)
- `cargo xtask solve <level|--all> [--write] [--shots]` – solver bot: report in `$CARGO_TARGET_DIR/reports/solver/<level>.{md,json}`; `--write` writes `tests/replays/<level>_solver.replay` (`source = solver`, `expect = complete frames=N..N deaths=0`, no shots). `--shots` adds shots `route_25 route_50 route_75`; they fail `goldens` until accepted with `cargo xtask golden --accept <level>_solver` after art-director APPROVED, so leave them out unless the route needs golden coverage. Same level + tuning → same route (deterministic).
- `cargo xtask filmstrip <file> [--every N]` – every 30th (Nth) frame plus the last in one image
- `cargo xtask golden` – compare all shots with `tests/golden/<replay>_<shot>.png` (the `goldens` verify step)
- `cargo xtask golden --accept <replay>_<shot>|<replay>|--all` – accept new goldens (**only** after art-director APPROVED)
- Human recording: see `play-in-vnc` (`--record <file>`)

## Bug reproduction
1. Write the shortest possible replay that triggers the bug (often handwritten).
2. Set `expect` to the **correct** behaviour, so the replay fails now and passes after the fix.
3. Name it `bug_<short_description>.replay`.
4. A failing replay breaks `cargo xtask check` (the replay unit test runs every file in `tests/replays/`), so **never commit a failing `bug_*.replay` on its own to the milestone branch**. The QA tester commits it on its own worktree branch; the engineer merges that branch, fixes the bug in the same branch, and renames the replay to `qa_<description>.replay` once it passes. Only then is it merged.

## After a physics or tuning change
1. Run `cargo xtask verify`; expect solver replays to fail.
2. `cargo xtask solve --all --write` re-creates the solver replays; check the difficulty reports still meet targets (narrowest jump window).
3. Golden screenshots of affected scenes change → art-director review → `golden --accept`.
4. Explain all of this in the commit message.
