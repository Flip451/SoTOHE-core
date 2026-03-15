#!/usr/bin/env python3
"""
PostToolUse hook: Run ruff on Python files after Edit/Write.

Runs directly on the host (no Docker needed). Ruff is sub-100ms,
well within the hook timeout budget.
"""

import json
import os
import subprocess
import sys

from _shared import load_stdin_json, print_hook_error, project_dir, tool_input

_RUFF_TIMEOUT = 5
LINT_PREFIX = "[python-lint]"


def get_file_path(data: dict) -> str | None:
    file_path = tool_input(data).get("file_path")
    return file_path if isinstance(file_path, str) else None


def is_python_file(path: str) -> bool:
    return path.endswith(".py")


def resolve_path(file_path: str, current_project_dir: str) -> str:
    """Resolve file_path, anchoring relative paths to project_dir."""
    if not os.path.isabs(file_path):
        file_path = os.path.join(current_project_dir, file_path)
    return os.path.realpath(file_path)


def is_path_in_project(file_path: str, current_project_dir: str) -> bool:
    try:
        resolved = resolve_path(file_path, current_project_dir)
        project = os.path.realpath(current_project_dir)
        prefix = project if project.endswith(os.sep) else project + os.sep
        return resolved.startswith(prefix) or resolved == project
    except (ValueError, OSError):
        return False


def find_ruff() -> str | None:
    """Find ruff binary: prefer .venv, then PATH."""
    current_dir = project_dir()
    venv_ruff = os.path.join(current_dir, ".venv", "bin", "ruff")
    if os.path.isfile(venv_ruff) and os.access(venv_ruff, os.X_OK):
        return venv_ruff
    # Fallback to PATH
    try:
        result = subprocess.run(
            ["which", "ruff"], capture_output=True, text=True, timeout=2
        )
        if result.returncode == 0 and result.stdout.strip():
            return result.stdout.strip()
    except (FileNotFoundError, subprocess.TimeoutExpired, OSError):
        pass
    return None


def build_hook_output(message: str) -> str:
    return json.dumps(
        {
            "hookSpecificOutput": {
                "hookEventName": "PostToolUse",
                "additionalContext": message,
            }
        }
    )


def main() -> None:
    try:
        data = load_stdin_json()
        if data.get("tool_name", "") not in ["Edit", "Write"]:
            sys.exit(0)

        file_path = get_file_path(data)
        if not file_path or not is_python_file(file_path):
            sys.exit(0)

        current_project_dir = project_dir()
        if not is_path_in_project(file_path, current_project_dir):
            sys.exit(0)

        ruff_bin = find_ruff()
        if not ruff_bin:
            sys.exit(0)

        resolved_file = resolve_path(file_path, current_project_dir)

        try:
            result = subprocess.run(
                [ruff_bin, "check", resolved_file],
                cwd=current_project_dir,
                capture_output=True,
                text=True,
                timeout=_RUFF_TIMEOUT,
            )
        except (FileNotFoundError, subprocess.TimeoutExpired, OSError):
            sys.exit(0)

        if result.returncode != 0:
            # Prefer stdout (lint diagnostics), fall back to stderr
            output = (result.stdout.strip() or result.stderr.strip())[:1500]
            if output:
                message = f"{LINT_PREFIX} {output}"
                print(build_hook_output(message))

        sys.exit(0)

    except Exception as err:
        print_hook_error(err)
        sys.exit(0)


if __name__ == "__main__":
    main()
