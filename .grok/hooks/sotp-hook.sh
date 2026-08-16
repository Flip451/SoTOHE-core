#!/usr/bin/env sh
set -eu

hook_id="${1:-}"
if [ -z "$hook_id" ]; then
  echo "[SoTOHE Grok Hook] missing hook id" >&2
  exit 2
fi

dispatch() {
  status=0
  "$@" || status=$?
  if [ "$status" -eq 0 ]; then
    exit 0
  fi
  exit 2
}

if [ -n "${SOTP_CLI_BINARY:-}" ]; then
  dispatch "$SOTP_CLI_BINARY" hook dispatch "$hook_id"
fi

repo_root="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"

if [ -x "$repo_root/bin/sotp" ]; then
  dispatch "$repo_root/bin/sotp" hook dispatch "$hook_id"
fi

if command -v sotp >/dev/null 2>&1; then
  dispatch sotp hook dispatch "$hook_id"
fi

echo "[SoTOHE Grok Hook] sotp CLI is not available. Build bin/sotp or set SOTP_CLI_BINARY." >&2
exit 2
