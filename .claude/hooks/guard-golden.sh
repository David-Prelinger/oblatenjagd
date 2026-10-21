#!/usr/bin/env bash
# PreToolUse (Edit|Write): goldens (screenshots and tests/golden/audio.toml)
# are never edited directly.
# Exit code 2 blocks the tool call and shows stderr to Claude.
#
# Writes through Bash (e.g. `cargo xtask golden --accept <name>`,
# `cargo xtask audio golden --accept <id>`) are deliberately NOT blocked:
# that is the sanctioned way to update goldens.
#
# Fails safe: if jq is missing or the hook input cannot be parsed, the edit
# is blocked, because we cannot tell whether it targets a golden.
set -u

if ! command -v jq >/dev/null 2>&1; then
  echo "Blocked: guard-golden.sh needs jq to inspect the edited path, and jq is not installed." >&2
  exit 2
fi
if ! file=$(jq -r '.tool_input.file_path // empty' 2>/dev/null); then
  echo "Blocked: guard-golden.sh could not parse the hook input JSON." >&2
  exit 2
fi

case "$file" in
  tests/golden/audio.toml | */tests/golden/audio.toml)
    echo "Blocked: '$file' holds the audio goldens. They change only via 'cargo xtask audio golden --accept <id>' after a game-designer APPROVED verdict (D-117, docs/validation.md)." >&2
    exit 2
    ;;
  tests/golden/* | */tests/golden/*)
    echo "Blocked: '$file' is a golden screenshot. Goldens change only via 'cargo xtask golden --accept <name>' after an art-director APPROVED verdict (docs/validation.md)." >&2
    exit 2
    ;;
esac
exit 0
