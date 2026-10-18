#!/usr/bin/env bash
# Stop: run `cargo xtask check`; if it fails, block stopping so the agent
# fixes the problem first. Prints only the decision JSON on stdout.
# Guarded by stop_hook_active to avoid endless loops: when the agent is
# already continuing because of this hook, stopping is allowed and the
# bypass is logged (check is not re-run).
# Fails open (allows stopping, logs why) if jq is missing.
set -u

root="${CLAUDE_PROJECT_DIR:-$(cd "$(dirname "$0")/../.." && pwd)}"
cd "$root" || exit 0

target="${CARGO_TARGET_DIR:-target}"
case "$target" in /*) ;; *) target="$root/$target" ;; esac
reports="$target/reports"
mkdir -p "$reports"
log="$reports/check-hook.log"

if ! command -v jq >/dev/null 2>&1; then
  echo "$(date -u +%FT%TZ) stop-check: jq not installed, check skipped, stop allowed" >>"$log"
  exit 0
fi

active=$(jq -r '.stop_hook_active // false' 2>/dev/null)
if [ "$active" = "true" ]; then
  echo "$(date -u +%FT%TZ) stop-check: stop_hook_active=true, stop allowed without re-running check (an earlier block may be unresolved, see $reports/check.md)" >>"$log"
  exit 0
fi

# Remove the old report so a stale one is never mistaken for this run's.
rm -f "$reports/check.json"

if cargo xtask check >"$log" 2>&1; then
  exit 0
fi

failed=""
if [ -f "$reports/check.json" ]; then
  failed=$(jq -r '[.steps[] | select(.status == "fail") | .name] | join(", ")' "$reports/check.json" 2>/dev/null)
fi
if [ -n "$failed" ]; then
  reason="cargo xtask check failed (steps: $failed). Fix before finishing; details: $reports/check.md"
else
  reason="cargo xtask check failed to run. Fix before finishing; output: $log"
fi

jq -n --arg reason "$reason" '{decision: "block", reason: $reason}'
exit 0
