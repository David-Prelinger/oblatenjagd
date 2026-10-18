# Oblatenjagd – a Salzkammergut jump 'n' run

A Super-Mario-style pixel-art platformer in Rust/macroquad. The journey goes from Salzburg via St. Gilgen, Brunnwinkl, Falkenstein, St. Wolfgang, the Wolfgangsee and Strobl to Bad Ischl and Café Zauner. The hero is Resi in a Dirndl, the coins are Zauner Oblaten, and the tone is affectionate satire.

**Project phase: plan approved, M0 next.** Work runs autonomously with no human in the loop: follow the `work-loop` skill.

## Read first
- `docs/architecture.md` – crates, data formats, determinism contract
- `docs/game-design.md` – gameplay, stages, Stage 1 scope
- `docs/art-pipeline.md` – how pixel art is made as text and code
- `docs/validation.md` – check/verify, replays, solver, goldens, definition of done
- `docs/agentic-setup.md` – subagents, skills, work loop
- `docs/milestones.md` – what's next
- `docs/decisions.md` – why things are the way they are

## Hard rules
- `jnr-core` is pure: no macroquad, no floats in the simulation, no `HashMap` iteration, no time/fs/threads/global RNG.
- Only `jnr-app` may use macroquad.
- All player-visible text is German and lives only in `content/text/de.toml`. Docs, code and comments are in English.
- All assets are text sources. No external pixel editors, no binary art in git (golden screenshots are the only exception).
- Golden screenshots change only through `cargo xtask golden --accept`, after an art-director APPROVED verdict.
- A task is done only when `cargo xtask verify` is green (see `docs/validation.md`).
- Record cross-cutting decisions in `docs/decisions.md`; to change one, add a superseding entry.
- Git: commit on the milestone branch only with green `verify`; merge to `main` after milestone acceptance; never force-push.
- You may install tools (`sudo apt-get install …`, `cargo install …`); add every one to `.devcontainer/Dockerfile` or `post-create.sh`.

## Commands (available from M0)
- `cargo xtask check` – fast: fmt, clippy, builds (native + wasm), unit tests, lints
- `cargo xtask verify` – full: check + solver + replays + goldens + VNC smoke test
- `cargo xtask sprites {preview,sheet,anim,lint}` · `cargo xtask levels lint` · `cargo xtask solve <level>`
- `cargo xtask replay <file>` · `cargo xtask filmstrip <replay>` · `cargo xtask scene <level> --x <px>`
- `cargo run -p jnr-app -- --level salzburg` – play on the noVNC desktop (`DISPLAY=:1`, http://localhost:6080)

## Environment
- Dev container (Debian bookworm, arm64), no GPU. `CARGO_TARGET_DIR=/workspaces/.cargo-target`.
- Available tools: `python3`, ImageMagick (`convert`, `montage`, `import`), the `wasm32-unknown-unknown` target.
