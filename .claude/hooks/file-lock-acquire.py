#!/usr/bin/env python3
"""
PreToolUse hook: Acquire a file lock before Edit/Write/Read tool executions.

Thin launcher — delegates to `sotp hook dispatch file-lock-acquire`.
PreToolUse: fail-closed (exit 2 on any error).
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

# Tools that require exclusive (write) access.
EXCLUSIVE_TOOLS = {"Edit", "Write"}
# Tools that require shared (read) access.
SHARED_TOOLS = {"Read"}


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
    # No cwd fallback — fail-closed: caller must treat None as missing.
    return None


def main() -> None:
    """Thin launcher: delegates to `sotp hook dispatch file-lock-acquire`.

    PreToolUse hook — fail-closed:
    - CLI missing, crash, or timeout → os._exit(2) (block)
    - except BaseException → os._exit(2) (block)
    - stdout/stderr flushed before os._exit()
    """
    import json as _json
    import subprocess as _subprocess

    if not _is_enabled():
        return

    try:
        stdin_data = sys.stdin.buffer.read()

        # Quick filter: only Edit/Write/Read need locking.
        try:
            data = _json.loads(stdin_data)
        except Exception:
            sys.stderr.write("error: failed to parse hook JSON\n")
            sys.stderr.flush()
            sys.stdout.flush()
            os._exit(2)

        tool_name = data.get("tool_name", "")
        if tool_name not in EXCLUSIVE_TOOLS and tool_name not in SHARED_TOOLS:
            os._exit(0)

        # Compute pid and agent in Python (sotp's getppid() would be this process).
        pid = str(os.getppid())
        agent = _agent_id()
        locks_dir = _locks_dir()

        # Fail-closed: no locks_dir means no CLAUDE_PROJECT_DIR and no SOTP_LOCKS_DIR.
        if locks_dir is None:
            sys.stderr.write("error: locks_dir not set (no CLAUDE_PROJECT_DIR or SOTP_LOCKS_DIR)\n")
            sys.stderr.flush()
            sys.stdout.flush()
            os._exit(2)

        cli = _cli_binary()
        result = _subprocess.run(
            [
                cli,
                "hook",
                "dispatch",
                "file-lock-acquire",
                "--locks-dir",
                locks_dir,
                "--agent",
                agent,
                "--pid",
                pid,
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
        os._exit(result.returncode)

    except BaseException as err:
        # PreToolUse: any error → block (fail-closed).
        sys.stderr.write(f"error: hook launcher failed: {err}\n")
        sys.stderr.flush()
        sys.stdout.flush()
        os._exit(2)


if __name__ == "__main__":
    main()
