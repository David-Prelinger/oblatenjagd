# Agentic setup

Status: **approved v1**. The agent and skill files already exist under `.claude/` as drafts; commands they mention become available during M0.

## Principles

1. **The main session is the orchestrator.** It picks tasks, delegates to specialist subagents, enforces the definition of done, and keeps `docs/milestones.md` up to date. It follows the `work-loop` skill.
2. **Specialists have narrow jobs and tools.** Reviewers can't edit, so their verdicts stay independent.
3. **Docs are the shared memory.** Agents don't rely on chat history: design lives in `docs/`, decisions in `docs/decisions.md`, progress in `docs/milestones.md`.
4. **Every claim is backed by a command.** "Works" means `cargo xtask verify` is green; "looks good" means an `art-director` APPROVED verdict.
5. **No human in the loop.** Agents decide, record their decisions, and keep going (see "When agents are stuck" in `validation.md`).

## Subagents (`.claude/agents/`)

| Agent | Job | Can edit | Key tools |
|---|---|---|---|
| `game-designer` | Stage concepts, enemies, power-ups, satire, German text; keeps `game-design.md` and `de.toml` | docs, `content/text/` | Read/Write/Edit, WebSearch (local history facts) |
| `level-designer` | Builds `.level` files, runs level lint + solver, tunes difficulty from the solver report | `content/levels/`, `tests/replays/` | Bash (xtask), Read/Write/Edit |
| `pixel-artist` | Writes palettes, `.sprite` files, generator params; previews and lints its own work | `art/`, `crates/jnr-artkit/src/gen/` | Bash, Read/Write/Edit (views PNGs) |
| `art-director` | Read-only visual reviewer; returns APPROVED or numbered fixes | nothing | Read, Bash (render previews only), Glob |
| `engine-engineer` | Implements Rust code in all crates and xtask, with tests | `crates/`, `xtask/` | all |
| `qa-tester` | Runs verify, solver, replays, filmstrips; hunts bugs; writes bug reports and new replay tests | `tests/replays/` | Bash, Read, Write |
| `code-reviewer` | Read-only review of diffs against architecture and determinism rules | nothing | Read, Grep, Glob, Bash (git diff, cargo) |
| `sound-designer` | (created in milestone MA) synth patches and music patterns | `audio/` | — |

## Skills (`.claude/skills/`)

| Skill | Used by | Content |
|---|---|---|
| `work-loop` | orchestrator | the task loop below, gates, how to update milestones, when to escalate |
| `sprite-authoring` | pixel-artist | palette/sprite format, generators, preview + lint commands, common mistakes |
| `level-authoring` | level-designer | level format, legend, section design, solver usage, difficulty guidelines |
| `visual-review` | art-director, pixel-artist | how to render review images and the review rubric |
| `replay-testing` | qa-tester, level-designer, engine-engineer | replay format, solver, goldens, determinism checks |
| `play-in-vnc` | anyone | launching the game on `:1`, recording human replays, X screenshots |
| `german-satire-writing` | game-designer | tone, Austrian German, glossary, satire limits, text keys |

## The work loop

```
┌─► 1. Pick the next unchecked task in docs/milestones.md (respect order and dependencies)
│   2. If design is unclear → game-designer decides it and writes it into the docs
│   3. Implement:
│        code    → engine-engineer
│        art     → pixel-artist      ⟲ art-director until APPROVED (max 3 rounds)
│        levels  → level-designer    ⟲ solver + difficulty report
│   4. cargo xtask verify  (fix until green; the Stop hook already forces `check`)
│   5. qa-tester: try to break it, add replay tests for anything found
│   6. code-reviewer on the diff (blocking findings must be fixed)
│   7. Tick the box in milestones.md, update docs/decisions.md if needed
│   8. Commit on the feature branch (see below)
└── 9. Next task. At the end of a milestone → milestone acceptance (milestones.md), merge to main, continue.
```

Independent tasks (e.g. sprites for enemy A while the engineer builds enemy B's logic) can run in parallel subagents.

## Git workflow

- Agents use git autonomously.
- One branch per milestone (`m1-salzburg`), commits per finished task, only with green `verify`.
- After milestone acceptance the orchestrator merges into `main` and tags `m<N>`.
- Never force-push, never rewrite history on `main`.
- Commit messages: `area: what` (e.g. `core: add coyote time`), plus notes on golden or replay updates.

## Running unattended

- Start a session with: "Follow the work-loop skill for milestone M1". The orchestrator works through the tasks and only stops for escalations or milestone acceptance.
- For long runs, `/loop` can re-invoke the work loop automatically.
- Progress can always be read in `docs/milestones.md` and `git log`.

## Permissions (human, 2026-10-18)

- Agents may commit, branch, merge to `main` and tag on their own.
- Web research is allowed (facts, references, CC0 assets).
- Agents may install tools in the container (`sudo apt-get install`, `cargo install`, …). Every installed tool must also be added to `.devcontainer/Dockerfile` or `post-create.sh`, so a rebuilt container has it.
