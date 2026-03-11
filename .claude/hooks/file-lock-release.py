#!/usr/bin/env python3
"""
PostToolUse hook: Release file lock after Edit/Write/Read tool executions.

Controlled by SOTP_LOCK_ENABLED env var (default: disabled).
"""

import json
import os
import subprocess
from pathlib import Path

from _shared import load_stdin_json, print_hook_error, project_dir, tool_input

LOCK_ENABLED_VAR = "SOTP_LOCK_ENABLED"
AGENT_ID_VAR = "SOTP_AGENT_ID"
LOCKS_DIR_VAR = "SOTP_LOCKS_DIR"
CLI_BINARY_VAR = "SOTP_CLI_BINARY"
DEFAULT_LOCKS_DIR = ".locks"

LOCK_TOOLS = {"Edit", "Write", "Read"}


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
    return None


def main() -> None:
    if not _is_enabled():
        return

    try:
        data = load_stdin_json()
    except (json.JSONDecodeError, Exception) as exc:
        print_hook_error(exc)
        return

    tool_name = data.get("tool_name", "")
    if tool_name not in LOCK_TOOLS:
        return

    file_path = _extract_file_path(data)
    if not file_path:
        return

    # Pass the path as-is; the CLI resolves canonical paths internally.
    # Skipping on non-existence would leak registry entries after delete/rename.
    resolved = str(Path(file_path))

    agent = _agent_id()
    cli = _cli_binary()
    locks_dir = _locks_dir()

    try:
        subprocess.run(
            [
                cli,
                "lock",
                "--locks-dir",
                locks_dir,
                "release",
                "--path",
                resolved,
                "--agent",
                agent,
            ],
            capture_output=True,
            text=True,
            timeout=5,
        )
    except (FileNotFoundError, subprocess.TimeoutExpired):
        pass
    except Exception as exc:
        print_hook_error(exc)


if __name__ == "__main__":
    main()
