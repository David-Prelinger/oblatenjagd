---
name: play-in-vnc
description: Launch the real game window on the noVNC desktop (DISPLAY=:1), play replays visibly, record human replays, and take X screenshots. Use for smoke tests or when the human wants to see or play the game.
---

# Play in VNC

Status: approved v1. Needs `jnr-app` (from M0).

The dev container runs a virtual desktop on `DISPLAY=:1` (1440x768, 16-bit colour). The human watches it at http://localhost:6080 (noVNC, no password). Commands below assume `DISPLAY=:1` is set (`export DISPLAY=:1`).

## Commands
- Play: `cargo run -p jnr-app -- --level salzburg` (run in the background so you aren't blocked). Today only `--level testbed` (the default) is embedded; `cargo run -p jnr-app -- --help` lists all flags.
- Scripted run that quits by itself: `cargo run -p jnr-app -- --exit-after 120 --screenshot-at 60:target/preview/f60.png` (exit code 0; the PNG is the 320×180 framebuffer after simulation frame 60; `--screenshot-at` is repeatable, `--seed`/`--scale` also exist)
- Show a replay: `cargo run -p jnr-app -- --replay tests/replays/testbed_route.replay --exit-after 760`. Level and seed come from the replay; the keyboard is ignored (Escape quits); after the last input nothing is held. Combine with `--screenshot-at` to grab frames.
- Record a run: `cargo run -p jnr-app -- --record tests/replays/human_<name>.replay` (see below).
- Screenshot of the whole desktop: `import -window root -display :1 $CARGO_TARGET_DIR/preview/vnc.png`, then look at it with Read.
- In-game framebuffer screenshot: F12 (debug builds) → `$CARGO_TARGET_DIR/preview/f12_<frame>.png` (or `./f12_<frame>.png` if unset); the path is printed
- Smoke test: `cargo xtask vnc-smoke` (also the last `verify` step): plays `testbed_route.replay` in the real window, compares its `--screenshot-at` frames pixel-exactly with the goldens and checks the root screenshot `$CARGO_TARGET_DIR/preview/vnc_smoke_root.png` shows the game. Mismatches: `$CARGO_TARGET_DIR/reports/vnc-smoke/`; app output: `$CARGO_TARGET_DIR/preview/vnc_smoke/app.log`. ~13 s.
- Browser build: `cargo xtask wasm --serve` (http://localhost:8080); headless check without a human: `cargo xtask wasm-smoke` → `$CARGO_TARGET_DIR/preview/wasm_smoke.png`

## Recording a replay
1. `cargo run -p jnr-app -- --record <file> [--exit-after N]` (not combinable with `--replay`). Missing directories are created.
2. Play. The session ends on Escape, window close, or `--exit-after`; then the app writes the replay and prints `recorded: <path>`.
3. The file has `source = human`, the level/seed, the RLE inputs and a derived `expect`: `complete frames=N..N deaths=<n>` if the stage was completed, else `state=<mode> player_x>=<x> deaths=<n>`. It passes `cargo xtask replay <file>` right away; tighten or loosen the `expect` by hand and add `shots` if needed.
4. Scripted (agent) recording with xdotool:
   ```
   cargo build -p jnr-app
   $CARGO_TARGET_DIR/debug/jnr-app --record /tmp/run.replay --exit-after 300 &
   WID=$(xdotool search --sync --onlyvisible --name '^Oblatenjagd$' | head -1)
   xdotool windowactivate --sync $WID
   xdotool keydown Right; sleep 0.8; xdotool keydown x; sleep 0.4; xdotool keyup x; sleep 1; xdotool keyup Right
   wait; cargo xtask replay /tmp/run.replay
   ```
   Don't send Escape before the game window has focus (it quits the game).

## Notes
- Rendering is software-only (no GPU). If the window doesn't appear, check `echo $DISPLAY` and that the desktop service is running (`xdotool getdisplaygeometry`).
- The desktop is 16-bit: colours in `import` screenshots are rounded (RGB565); compare exact pixels with `--screenshot-at` PNGs instead.
- Stop any game process you started when done.
- Controls: arrows = move, X/Space = jump, Y/Z/Shift = run, Enter = start/pause, Escape = quit (native), F1 debug overlay, F12 framebuffer screenshot.
