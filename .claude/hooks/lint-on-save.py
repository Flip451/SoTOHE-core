#!/usr/bin/env python3
"""
PostToolUse hook: Run rustfmt and cargo check on Rust files after Edit/Write.
"""

import json
import os
import re
import subprocess
import sys
from pathlib import Path, PurePosixPath, PureWindowsPath

from _shared import load_stdin_json, print_hook_error, project_dir, tool_input

try:
    import fcntl
except ImportError:
    fcntl = None  # type: ignore[assignment]

import hashlib

MAX_PATH_LENGTH = 4096
TOOLS_DAEMON_SERVICE = "tools-daemon"
CONTAINER_WORKSPACE = "/workspace"
LINT_ON_SAVE_PREFIX = "[lint-on-save]"
LINT_ON_SAVE_SUFFIX = "Run `cargo make clippy` for lint details."
WINDOWS_ABSOLUTE_PATH = re.compile(r"^[A-Za-z]:[\\/]|^\\\\")
# Total budget must fit within the hook timeout (30s) with margin.
_DAEMON_CHECK_TIMEOUT = 5
_RUSTFMT_TIMEOUT = 8
_CLIPPY_TIMEOUT = 12
_LOCK_DIR = "/tmp"


def _lock_file_for_project(project_dir_path: str) -> str:
    normalized = os.path.normpath(project_dir_path)
    digest = hashlib.sha256(normalized.encode()).hexdigest()[:12]
    return f"{_LOCK_DIR}/claude-lint-on-save-{digest}.lock"


def validate_path(file_path: str) -> bool:
    return (
        bool(file_path) and len(file_path) <= MAX_PATH_LENGTH and ".." not in file_path
    )


def get_file_path(data: dict) -> str | None:
    file_path = tool_input(data).get("file_path")
    return file_path if isinstance(file_path, str) else None


def resolve_project_path(file_path: str, current_project_dir: str) -> Path:
    path = Path(file_path)
    if not path.is_absolute():
        path = Path(current_project_dir) / path
    return path.resolve()


def looks_like_windows_path(path: str) -> bool:
    return bool(WINDOWS_ABSOLUTE_PATH.match(path))


def relative_project_path(file_path: str, current_project_dir: str) -> str:
    if looks_like_windows_path(file_path) or looks_like_windows_path(
        current_project_dir
    ):
        path = PureWindowsPath(file_path)
        project = PureWindowsPath(current_project_dir)
        if not path.is_absolute():
            path = project / path
        relative = path.relative_to(project)
        return PurePosixPath(*relative.parts).as_posix()

    resolved = resolve_project_path(file_path, current_project_dir)
    return resolved.relative_to(Path(current_project_dir).resolve()).as_posix()


def is_path_in_project(file_path: str, current_project_dir: str) -> bool:
    try:
        relative_project_path(file_path, current_project_dir)
        return True
    except ValueError:
        return False


def is_rust_file(path: str) -> bool:
    return path.endswith(".rs")


def find_cargo_manifest(file_path: str) -> str | None:
    current = Path(file_path).parent
    for _ in range(10):
        candidate = current / "Cargo.toml"
        if candidate.exists():
            return str(current)
        parent = current.parent
        if parent == current:
            break
        current = parent
    return None


def host_to_container_path(host_path: str, project_dir: str) -> str:
    rel = relative_project_path(host_path, project_dir)
    return f"{CONTAINER_WORKSPACE}/{rel}"


def is_daemon_running(project_dir: str) -> bool:
    try:
        result = subprocess.run(
            [
                "docker",
                "compose",
                "ps",
                "--status",
                "running",
                "-q",
                TOOLS_DAEMON_SERVICE,
            ],
            cwd=project_dir,
            capture_output=True,
            text=True,
            timeout=_DAEMON_CHECK_TIMEOUT,
        )
        return result.returncode == 0 and bool(result.stdout.strip())
    except (FileNotFoundError, subprocess.TimeoutExpired, OSError):
        return False


def run_in_daemon(
    cmd: list[str],
    workdir: str,
    project_dir: str,
    timeout: int = _CLIPPY_TIMEOUT,
) -> tuple[int, str, str]:
    container_workdir = host_to_container_path(workdir, project_dir)
    full_cmd = [
        "docker",
        "compose",
        "exec",
        "-T",
        "--workdir",
        container_workdir,
        TOOLS_DAEMON_SERVICE,
        *cmd,
    ]
    try:
        result = subprocess.run(
            full_cmd,
            cwd=project_dir,
            capture_output=True,
            text=True,
            timeout=timeout,
        )
        return result.returncode, result.stdout, result.stderr
    except subprocess.TimeoutExpired:
        return 1, "", "Command timed out"
    except (FileNotFoundError, OSError) as err:
        return 1, "", str(err)


def build_lint_message(rel_path: str, issues: list[str]) -> str:
    return (
        f"{LINT_ON_SAVE_PREFIX} Issues in {rel_path}: "
        + " | ".join(issues[:2])
        + f" {LINT_ON_SAVE_SUFFIX}"
    )


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
        if not file_path or not validate_path(file_path) or not is_rust_file(file_path):
            sys.exit(0)

        current_project_dir = project_dir()
        if not is_path_in_project(file_path, current_project_dir):
            sys.exit(0)
        if not is_daemon_running(current_project_dir):
            sys.exit(0)

        # Acquire exclusive lock to prevent multiple lint-on-save racing on Cargo lock.
        # Non-blocking: if another instance holds the lock, skip this run.
        # On platforms without fcntl (Windows), skip locking entirely.
        lock_fd = None
        if fcntl is not None:
            try:
                lock_fd = open(_lock_file_for_project(current_project_dir), "w")
                fcntl.flock(lock_fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
            except OSError:
                if lock_fd:
                    lock_fd.close()
                sys.exit(0)

        try:
            resolved_file_path = str(
                resolve_project_path(file_path, current_project_dir)
            )
            crate_dir = find_cargo_manifest(resolved_file_path) or current_project_dir
            rel_path = relative_project_path(file_path, current_project_dir)
            container_file_path = host_to_container_path(file_path, current_project_dir)
            issues: list[str] = []

            ret, stdout, stderr = run_in_daemon(
                ["rustfmt", container_file_path],
                workdir=crate_dir,
                project_dir=current_project_dir,
                timeout=_RUSTFMT_TIMEOUT,
            )
            if ret != 0:
                issues.append(
                    f"rustfmt failed: {(stderr or stdout or 'unknown error')[:1500]}"
                )

            ret, stdout, stderr = run_in_daemon(
                ["cargo", "check", "--all-targets"],
                workdir=crate_dir,
                project_dir=current_project_dir,
                timeout=_CLIPPY_TIMEOUT,
            )
            if ret != 0:
                issues.append(
                    f"cargo check: {(stderr or stdout or 'unknown error')[:1500]}"
                )

            if issues:
                print(build_hook_output(build_lint_message(rel_path, issues)))
        finally:
            if lock_fd is not None and fcntl is not None:
                fcntl.flock(lock_fd, fcntl.LOCK_UN)
                lock_fd.close()

        sys.exit(0)

    except Exception as err:
        print_hook_error(err)
        sys.exit(0)


if __name__ == "__main__":
    main()
