#!/usr/bin/env bash
# terapi-env.sh — configure terapi environment variables and launch terapi
#
# Usage:
#   ./terapi-env.sh [terapi args...]
#   source terapi-env.sh          # export vars into current shell only (no launch)
#
# Override any variable before sourcing/running, e.g.:
#   TERAPI_DIR=~/myproject/.terapi ./terapi-env.sh run campaign.toml

# ── Data directory ─────────────────────────────────────────────────────────────
# Priority: env var > ./.terapi/ (auto-detected by terapi) > ~/.config/terapi/
# Uncomment and adjust to pin a specific directory:
# export TERAPI_DIR="${TERAPI_DIR:-$HOME/.config/terapi}"

# ── External JSON editor ───────────────────────────────────────────────────────
# Used when pressing E on the Body tab or on a campaign step body.
# Defaults to jsoned if found in PATH, otherwise falls back to $EDITOR.
if command -v jsoned &>/dev/null; then
    export TERAPI_JSON_EDITOR="${TERAPI_JSON_EDITOR:-jsoned}"
elif [ -n "$EDITOR" ]; then
    export TERAPI_JSON_EDITOR="${TERAPI_JSON_EDITOR:-$EDITOR}"
fi

# ── Response diff tool ─────────────────────────────────────────────────────────
# Used when pressing d in JSON or Raw view to compare two responses.
# The tool receives two file paths as arguments: $TERAPI_DIFF file1 file2
# Examples: difft, delta, colordiff -u, nvim -d
# Default (when unset): diff -u file1 file2 | less -R
if command -v difft &>/dev/null; then
    export TERAPI_DIFF="${TERAPI_DIFF:-difft}"
elif command -v delta &>/dev/null; then
    export TERAPI_DIFF="${TERAPI_DIFF:-delta}"
fi

# ── Fallback text editor ───────────────────────────────────────────────────────
# Used by terapi to open collection/campaign TOML files directly.
# Respects $EDITOR / $VISUAL already set in the environment.
export EDITOR="${EDITOR:-vi}"
export VISUAL="${VISUAL:-$EDITOR}"

# ── Launch ─────────────────────────────────────────────────────────────────────
# When sourced (not executed), stop here so vars are exported into the shell.
if [ "${BASH_SOURCE[0]}" = "$0" ]; then
    exec terapi "$@"
fi
