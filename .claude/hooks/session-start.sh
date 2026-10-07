#!/bin/bash
# SessionStart hook: prepares the build environment.
#
# - Claude Code cloud sessions (Linux): session-start-cloud.sh
# - Local sessions on macOS with Nix: session-start-macos.sh
# - Anywhere else: does nothing.
#
# The scripts get the session start reason (startup, resume, clear, ...) from
# the hook input in $HOOK_SOURCE.

set -uo pipefail

HOOK_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

input=""
[ -t 0 ] || input="$(cat)"
HOOK_SOURCE="$(printf '%s' "$input" | sed -n 's/.*"source"[[:space:]]*:[[:space:]]*"\([a-z]*\)".*/\1/p')"
export HOOK_SOURCE

if [ "${CLAUDE_CODE_REMOTE:-}" = "true" ]; then
	exec "$HOOK_DIR/session-start-cloud.sh"
elif [ "$(uname -s)" = "Darwin" ]; then
	exec "$HOOK_DIR/session-start-macos.sh"
fi
exit 0
