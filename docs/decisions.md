# Decision log

Short records of decisions and why. Newest at the bottom. Agents append here when they make a decision that affects more than one file or crate; to change an existing decision, add a new entry that supersedes it and explains why.

## D-001 Rust + macroquad (not SDL2, not Bevy)
- macroquad targets `wasm32-unknown-unknown` directly; SDL2 from Rust needs Emscripten for the web, which adds a C toolchain, bigger bundles and breakage.
- Bevy is powerful but heavy (compile times, ECS ceremony) for a 2D pixel game where we render in software anyway.
- Risk is contained: only `jnr-app` uses macroquad.

## D-002 Software renderer into a 320×180 framebuffer
- Pixel-perfect by construction, no GPU dependency (the container has none).
- The same code produces headless screenshots, so golden-image tests show exactly what the player sees.
- 320×180 is 16:9 and scales by whole numbers to 720p (×4), 1080p (×6) and 1440p (×8).

## D-003 Fixed-point simulation at fixed 60 Hz
- Bit-identical results native vs wasm → replays and solver bots stay valid everywhere.
- Classic platformers (SMB) use sub-pixel integer math too; it suits tight pixel-art movement.

## D-004 All assets as text sources
- `.sprite` text grids, procedural generators, ASCII levels, TOML text/tuning.
- Agents can author and diff them; no binary blobs in git except approved golden screenshots.
- Pixel art is produced inside the dev container with no external editor. **No LibreSprite / Aseprite** (human decision).

## D-005 Headless-first validation
- Every feature must be verifiable without a window (`cargo xtask verify`). The VNC window is for humans and a final smoke test only.

## D-006 Documentation in English, game text in German
- Docs, code, comments, commit messages: English.
- All player-visible text: German (Austrian flavour), only in `content/text/de.toml`.

## D-007 Fully autonomous agents (human, 2026-10-18)
- No human involvement: agents decide, commit, merge to `main` and accept milestones themselves (see `docs/milestones.md`).
- Web research and installing tools in the container are allowed; installed tools go into the dev container config.

## D-008 Game decisions (human, 2026-10-18)
- Title „Oblatenjagd"; hero **Resi** in a Dirndl; real names of places, businesses and historical people may be used; difficulty **medium** (see `game-design.md`).

## D-009 Parallel code tasks run in git worktrees
- Two engineers editing the same tree break each other's `cargo xtask check` (and block the orchestrator's Stop hook).
- Parallel code tasks therefore run in isolated git worktrees, each with its own `CARGO_TARGET_DIR` (`/workspaces/.cargo-target/wt-<task>`), and commit on their own branch; the orchestrator merges them into the milestone branch.
