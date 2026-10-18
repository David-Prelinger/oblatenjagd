# Backlog

Deferred work and known issues, picked up by the work loop when relevant.

- **Stop hook vs parallel agents**: the Stop hook checks the whole workspace, so the orchestrator's turn is blocked by unfinished edits of background subagents. Options: run parallel engineers in git worktrees (`isolation: worktree`) and merge, or let the hook ignore failures while subagents are active. Decide before running parallel code tasks in M0.
