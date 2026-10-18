#!/usr/bin/env bash
# PostToolUse (Edit|Write): format an edited .rs file with rustfmt.
# Reads the hook JSON from stdin (tool_input.file_path). Never blocks:
# if rustfmt fails (e.g. a syntax error mid-edit) it stays silent and
# `cargo xtask check` reports the problem later. Without jq it does nothing.
set -u

file=$(jq -r '.tool_input.file_path // empty' 2>/dev/null)
case "$file" in
  *.rs) ;;
  *) exit 0 ;;
esac
[ -f "$file" ] || exit 0

rustfmt --edition 2024 "$file" >/dev/null 2>&1 || true
exit 0
