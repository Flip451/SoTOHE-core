#!/usr/bin/env python3
"""
PostToolUse hook: Release file lock after Edit/Write/Read tool executions.

Thin launcher — delegates to `sotp hook dispatch file-lock-release`.
PostToolUse: cannot block (exit 0 on any error, warning to stderr).
Deprecated launcher retained for rollback while settings.json calls `sotp` directly.

Controlled by SOTP_LOCK_ENABLED env var (default: disabled).
"""

import os
import sys

LOCK_ENABLED_VAR = "SOTP_LOCK_ENABLED"
CLI_BINARY_VAR = "SOTP_CLI_BINARY"
AGENT_ID_VAR = "SOTP_AGENT_ID"
LOCKS_DIR_VAR = "SOTP_LOCKS_DIR"
DEFAULT_LOCKS_DIR = ".locks"

LOCK_TOOLS = {"Edit", "Write", "Read"}


def _is_enabled() -> bool:
    return os.environ.get(LOCK_ENABLED_VAR, "") == "1"


def _cli_binary() -> str:
    return os.environ.get(CLI_BINARY_VAR, "sotp")


def _agent_id() -> str:
    return os.environ.get(AGENT_ID_VAR, f"pid-{os.getppid()}")


def _locks_dir() -> str | None:
    explicit = os.environ.get(LOCKS_DIR_VAR)
    if explicit:
        return explicit
    project = os.environ.get("CLAUDE_PROJECT_DIR")
    if project:
        return os.path.join(project, DEFAULT_LOCKS_DIR)
    # No cwd fallback — fail-closed contract.
    return None


def main() -> None:
    """Thin launcher: delegates to `sotp hook dispatch file-lock-release`.

    PostToolUse hook — cannot block:
    - CLI missing, crash, or timeout → os._exit(0) + stderr warning
    - except BaseException → os._exit(0) + stderr warning
    - stdout/stderr flushed before os._exit()
    """
    import json as _json
    import subprocess as _subprocess

    if not _is_enabled():
        return

    try:
        stdin_data = sys.stdin.buffer.read()

        # Quick filter: only Edit/Write/Read need lock release.
        try:
            data = _json.loads(stdin_data)
        except Exception:
            # PostToolUse: cannot block — warn and exit 0.
            sys.stderr.write("warning: failed to parse hook JSON for lock-release\n")
            sys.stderr.flush()
            sys.stdout.flush()
            os._exit(0)

        tool_name = data.get("tool_name", "")
        if tool_name not in LOCK_TOOLS:
            os._exit(0)

        agent = _agent_id()
        locks_dir = _locks_dir()

        # PostToolUse: no locks_dir → warn and exit 0 (cannot block).
        if locks_dir is None:
            sys.stderr.write("warning: locks_dir not set for lock-release (no CLAUDE_PROJECT_DIR or SOTP_LOCKS_DIR)\n")
            sys.stderr.flush()
            sys.stdout.flush()
            os._exit(0)

        cli = _cli_binary()
        result = _subprocess.run(
            [
                cli,
                "hook",
                "dispatch",
                "file-lock-release",
                "--locks-dir",
                locks_dir,
                "--agent",
                agent,
            ],
            input=stdin_data,
            capture_output=True,
            timeout=10,
        )

        if result.stdout:
            sys.stdout.buffer.write(result.stdout)
        if result.stderr:
            sys.stderr.buffer.write(result.stderr)

        sys.stdout.flush()
        sys.stderr.flush()
        # PostToolUse always exits 0 regardless of sotp exit code.
        os._exit(0)

    except BaseException as err:
        # PostToolUse: any error → warn + exit 0 (cannot block).
        sys.stderr.write(f"warning: hook launcher failed: {err}\n")
        sys.stderr.flush()
        sys.stdout.flush()
        os._exit(0)


if __name__ == "__main__":
    main()
