---
name: play-in-vnc
description: Launch the real game window on the noVNC desktop (DISPLAY=:1), play replays visibly, record human replays, and take X screenshots. Use for smoke tests or when the human wants to see or play the game.
---

# Play in VNC

Status: approved v1. Needs `jnr-app` (from M0).

The dev container runs a virtual desktop on `DISPLAY=:1`. The human watches it at http://localhost:6080 (noVNC, no password).

## Commands
- Play: `cargo run -p jnr-app -- --level salzburg` (run in the background so you aren't blocked). Today only `--level testbed` (the default) is embedded; `cargo run -p jnr-app -- --help` lists all flags.
- Scripted run that quits by itself: `cargo run -p jnr-app -- --exit-after 120 --screenshot-at 60:target/preview/f60.png` (exit code 0; the PNG is the 320×180 framebuffer after simulation frame 60; `--screenshot-at` is repeatable, `--seed`/`--scale` also exist)
- Show a replay: `cargo run -p jnr-app -- --replay tests/replays/<file> --exit-after <frames>`
- Record a human run: `cargo run -p jnr-app -- --level salzburg --record tests/replays/human_<name>.replay`
- Screenshot of the whole desktop: `import -window root -display :1 target/preview/vnc.png`, then look at it with Read.
- In-game framebuffer screenshot: F12 (debug builds) → `$CARGO_TARGET_DIR/preview/f12_<frame>.png` (or `./f12_<frame>.png` if unset); the path is printed
- Smoke test: `cargo xtask vnc-smoke`
- Browser build: `cargo xtask wasm --serve` (http://localhost:8080); headless check without a human: `cargo xtask wasm-smoke` → `$CARGO_TARGET_DIR/preview/wasm_smoke.png`

## Notes
- Rendering is software-only (no GPU). If the window doesn't appear, check `echo $DISPLAY` and that the desktop service is running.
- Stop any game process you started when done.
- Controls: arrows = move, X/Space = jump, Y/Shift = run, Enter = start/pause, F1–F3 debug overlays.
