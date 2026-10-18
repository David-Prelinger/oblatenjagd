---
name: work-loop
description: The orchestrator's procedure for working through docs/milestones.md autonomously - pick a task, delegate to subagents, enforce the definition of done, update tracking, commit, escalate. Use when asked to "work on milestone X", "continue", or run unattended.
---

# Work loop (orchestrator)

Status: approved v1. Needs `cargo xtask` (available from M0).

## Each iteration
1. **Orient**: read `docs/milestones.md`, `git status`, `git log --oneline -10`. Make sure you're on the milestone branch (`m<N>-<name>`); create it from `main` if missing.
2. **Pick** the first unchecked task whose dependencies are done. Split it if it's bigger than roughly one focused session of work.
3. **Clarify**: if the design is missing or ambiguous, ask `game-designer` to decide it and write it into the docs.
4. **Delegate** (run independent parts in parallel):
   - code → `engine-engineer`
   - art → `pixel-artist`, then `art-director`; loop until APPROVED (max 3 rounds)
   - level → `level-designer`
   Give each subagent: the goal, the relevant doc sections, the acceptance criteria, and what to report.
5. **Verify**: `cargo xtask verify` must be green. Send failures back to the responsible agent.
6. **QA**: `qa-tester` on the feature. Bugs → back to step 4 with the failing replay.
7. **Review**: `code-reviewer` on the diff. Blocking findings → back to step 4.
8. **Record**: tick the box in `docs/milestones.md`; update docs; add to `docs/decisions.md` if a cross-cutting decision was made.
9. **Commit** on the milestone branch: `area: summary`, mention any golden/replay updates and approvals.
10. **Continue** with the next task.

## End of a milestone
Run the "Milestone acceptance" steps in `docs/milestones.md`: verify, QA pass, art-director and game-designer reviews, write `docs/reports/M<N>.md`, merge into `main`, tag `m<N>`, then start the next milestone.

## When stuck (no human in the loop)
- Same task failed review/verification 3 times → simplify the scope, record it in `docs/decisions.md`, put the rest in `docs/backlog.md`, move on.
- Missing tool → install it and add it to `.devcontainer/Dockerfile` or `post-create.sh`.
- Only stop and wait for the human if something needs credentials or payment, or would rewrite history on `main`.
