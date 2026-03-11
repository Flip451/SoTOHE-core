#!/usr/bin/env python3
"""
PreToolUse hook: Acquire a file lock before Edit/Write/Read tool executions.

Edit/Write → exclusive lock (≈ &mut T)
Read       → shared lock (≈ &T)

Controlled by SOTP_LOCK_ENABLED env var (default: disabled).
Agent ID derived from SOTP_AGENT_ID env var or fallback to PID.
"""

import json
import os
import subprocess
import sys
from pathlib import Path

from _shared import load_stdin_json, print_hook_error, project_dir, tool_input

# Lock is opt-in: set SOTP_LOCK_ENABLED=1 to activate.
LOCK_ENABLED_VAR = "SOTP_LOCK_ENABLED"
AGENT_ID_VAR = "SOTP_AGENT_ID"
LOCKS_DIR_VAR = "SOTP_LOCKS_DIR"
CLI_BINARY_VAR = "SOTP_CLI_BINARY"
DEFAULT_LOCKS_DIR = ".locks"
DEFAULT_TIMEOUT_MS = "5000"

# Tools that require exclusive (write) access.
EXCLUSIVE_TOOLS = {"Edit", "Write"}
# Tools that require shared (read) access.
SHARED_TOOLS = {"Read"}


def _is_enabled() -> bool:
    return os.environ.get(LOCK_ENABLED_VAR, "") == "1"


def _agent_id() -> str:
    # Use parent PID (Claude Code process) so Pre and Post hooks share the
    # same fallback agent ID despite being separate subprocesses.
    return os.environ.get(AGENT_ID_VAR, f"pid-{os.getppid()}")


def _locks_dir() -> str:
    return os.environ.get(
        LOCKS_DIR_VAR, os.path.join(project_dir(), DEFAULT_LOCKS_DIR)
    )


def _cli_binary() -> str:
    return os.environ.get(CLI_BINARY_VAR, "sotp")


def _extract_file_path(data: dict) -> str | None:
    ti = tool_input(data)
    fp = ti.get("file_path")
    if isinstance(fp, str) and fp:
        return fp
    cmd = ti.get("command", "")
    if isinstance(cmd, str) and cmd:
        return None
    return None


def main() -> None:
    if not _is_enabled():
        return

    try:
        data = load_stdin_json()
    except (json.JSONDecodeError, Exception) as exc:
        # Malformed hook input — block to avoid fail-open.
        print_hook_error(exc)
        print(
            json.dumps(
                {
                    "hookSpecificOutput": {
                        "decision": "block",
                        "reason": f"File lock hook input error: {exc}",
                    }
                }
            )
        )
        sys.exit(2)

    tool_name = data.get("tool_name", "")
    if tool_name in EXCLUSIVE_TOOLS:
        mode = "exclusive"
    elif tool_name in SHARED_TOOLS:
        mode = "shared"
    else:
        return

    file_path = _extract_file_path(data)
    if not file_path:
        return

    # Pass the path as-is; FilePath::new handles parent-directory
    # canonicalization for files that don't exist yet (create-new-file flow).
    resolved = str(Path(file_path))

    agent = _agent_id()
    # Pass parent PID (Claude Code) so the lock entry references a long-lived
    # process and is not immediately reaped by stale detection.
    pid = str(os.getppid())
    cli = _cli_binary()
    locks_dir = _locks_dir()

    try:
        result = subprocess.run(
            [
                cli,
                "lock",
                "--locks-dir",
                locks_dir,
                "acquire",
                "--mode",
                mode,
                "--path",
                resolved,
                "--agent",
                agent,
                "--pid",
                pid,
                "--timeout-ms",
                DEFAULT_TIMEOUT_MS,
            ],
            capture_output=True,
            text=True,
            timeout=10,
        )
        if result.returncode != 0:
            error_msg = result.stderr.strip() or result.stdout.strip()
            print(
                json.dumps(
                    {
                        "hookSpecificOutput": {
                            "decision": "block",
                            "reason": f"File lock conflict: {error_msg}",
                        }
                    }
                )
            )
            sys.exit(2)
    except FileNotFoundError:
        # CLI binary not found — block to avoid fail-open.
        print(
            json.dumps(
                {
                    "hookSpecificOutput": {
                        "decision": "block",
                        "reason": "File lock CLI binary not found; cannot acquire lock",
                    }
                }
            )
        )
        sys.exit(2)
    except subprocess.TimeoutExpired:
        # Lock backend hung — block to avoid fail-open.
        print(
            json.dumps(
                {
                    "hookSpecificOutput": {
                        "decision": "block",
                        "reason": "File lock acquire timed out (subprocess)",
                    }
                }
            )
        )
        sys.exit(2)
    except Exception as exc:
        # Any other subprocess error — block to avoid fail-open.
        print_hook_error(exc)
        print(
            json.dumps(
                {
                    "hookSpecificOutput": {
                        "decision": "block",
                        "reason": f"File lock acquire failed: {exc}",
                    }
                }
            )
        )
        sys.exit(2)


if __name__ == "__main__":
    main()
