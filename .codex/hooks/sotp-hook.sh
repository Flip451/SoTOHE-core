#!/usr/bin/env sh
set -eu

hook_id="${1:-}"
if [ -z "$hook_id" ]; then
  echo "[SoTOHE Codex Hook] missing hook id" >&2
  exit 2
fi

if [ -n "${SOTP_CLI_BINARY:-}" ]; then
  exec "$SOTP_CLI_BINARY" hook dispatch "$hook_id"
fi

repo_root="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"

if [ -x "$repo_root/bin/sotp" ]; then
  exec "$repo_root/bin/sotp" hook dispatch "$hook_id"
fi

if command -v sotp >/dev/null 2>&1; then
  exec sotp hook dispatch "$hook_id"
fi

echo "[SoTOHE Codex Hook] sotp CLI is not available. Build bin/sotp or set SOTP_CLI_BINARY." >&2
exit 2
