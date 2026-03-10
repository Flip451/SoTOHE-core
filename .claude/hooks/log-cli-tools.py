#!/usr/bin/env python3
"""
Hook: Log Codex/Gemini CLI tool invocations to .claude/logs/cli-tools.jsonl.

Disabled by default. Enable only when `CLAUDE_LOG_CLI_TOOLS=1`.
"""

from __future__ import annotations

import json
import os
import shlex
import sys
from datetime import UTC, datetime
from typing import Any

try:
    import fcntl
except ImportError:
    fcntl = None  # type: ignore[assignment]

from _shared import (
    load_stdin_json,
    print_hook_error,
    project_dir,
    tool_input,
    tool_response_output,
)

MAX_LOG_SIZE = 10 * 1024 * 1024
MAX_LOG_GENERATIONS = 3
MAX_PREVIEW_CHARS = 1200
SUPPORTED_PROVIDERS = ("codex", "gemini")
SHELL_COMMAND_TOKENS = {
    "bash",
    "sh",
    "zsh",
    "fish",
    "pwsh",
    "powershell",
    "powershell.exe",
    "cmd",
    "cmd.exe",
}


def is_enabled() -> bool:
    return os.environ.get("CLAUDE_LOG_CLI_TOOLS", "").strip() == "1"


def get_log_path() -> str:
    log_dir = os.path.join(project_dir(), ".claude", "logs")
    os.makedirs(log_dir, exist_ok=True)
    return os.path.join(log_dir, "cli-tools.jsonl")


def truncate_preview(text: str, limit: int = MAX_PREVIEW_CHARS) -> str:
    stripped = text.strip()
    if len(stripped) <= limit:
        return stripped
    return stripped[:limit].rstrip() + "... [truncated]"


def split_shell_tokens(command: str) -> list[str]:
    try:
        return shlex.split(command)
    except ValueError:
        return command.split()


def normalize_command_token(token: str) -> str:
    base = token.rsplit("/", 1)[-1].rsplit("\\", 1)[-1].lower()
    if base.endswith(".exe"):
        base = base[:-4]
    return base


def extract_command_token(command: str) -> str | None:
    stripped = command.strip()
    if not stripped:
        return None
    if stripped[0] in {"'", '"'}:
        quote = stripped[0]
        end = stripped.find(quote, 1)
        if end == -1:
            return stripped[1:]
        return stripped[1:end]
    return stripped.split(None, 1)[0]


def nested_shell_command(tokens: list[str], shell_name: str) -> str | None:
    if shell_name in {"cmd"}:
        accepted_flags = {"/c", "/k"}
    elif shell_name in {"pwsh", "powershell"}:
        accepted_flags = {"-c", "-command"}
    else:
        accepted_flags = {"-c", "-lc", "-cl", "-ic", "-ci"}

    for index in range(1, len(tokens) - 1):
        if tokens[index].lower() in accepted_flags:
            return tokens[index + 1]
    return None


def split_shell_segments(payload: str) -> list[str]:
    """Split a shell payload by ;, &&, ||, | while respecting quotes.

    Uses a character-level scanner so that separators inside single- or
    double-quoted strings are preserved and backslash escapes are honoured.
    The original text of each segment is returned verbatim (no quote stripping).
    """
    segments: list[str] = []
    buf: list[str] = []
    i = 0
    n = len(payload)
    in_single = False
    in_double = False

    while i < n:
        ch = payload[i]

        # Backslash escape (not inside single quotes)
        if ch == "\\" and not in_single and i + 1 < n:
            buf.append(ch)
            buf.append(payload[i + 1])
            i += 2
            continue

        if ch == "'" and not in_double:
            in_single = not in_single
            buf.append(ch)
            i += 1
            continue

        if ch == '"' and not in_single:
            in_double = not in_double
            buf.append(ch)
            i += 1
            continue

        if not in_single and not in_double:
            if ch == ";":
                seg = "".join(buf).strip()
                if seg:
                    segments.append(seg)
                buf = []
                i += 1
                continue
            if ch == "&" and i + 1 < n and payload[i + 1] == "&":
                seg = "".join(buf).strip()
                if seg:
                    segments.append(seg)
                buf = []
                i += 2
                continue
            if ch == "|" and i + 1 < n and payload[i + 1] == "|":
                seg = "".join(buf).strip()
                if seg:
                    segments.append(seg)
                buf = []
                i += 2
                continue
            if ch == "|":
                seg = "".join(buf).strip()
                if seg:
                    segments.append(seg)
                buf = []
                i += 1
                continue

        buf.append(ch)
        i += 1

    seg = "".join(buf).strip()
    if seg:
        segments.append(seg)
    return segments


def detect_cli_provider(command: str, depth: int = 0) -> str | None:
    if depth > 2:
        return None

    command_token = extract_command_token(command)
    if command_token is None:
        return None

    normalized = normalize_command_token(command_token)
    if normalized in SUPPORTED_PROVIDERS:
        return normalized
    if normalized not in SHELL_COMMAND_TOKENS:
        return None

    tokens = split_shell_tokens(command)
    nested_command = nested_shell_command(tokens, normalized)
    if nested_command is None:
        return None

    # Check each segment of a compound shell command (e.g. "echo hi; codex run")
    for segment in split_shell_segments(nested_command):
        result = detect_cli_provider(segment, depth + 1)
        if result is not None:
            return result
    return None


def is_cli_tool_command(command: str) -> bool:
    return detect_cli_provider(command) is not None


def read_exit_code(data: dict[str, Any]) -> int | None:
    tool_response = data.get("tool_response", {})
    if not isinstance(tool_response, dict):
        return None
    for key in ("exit_code", "return_code", "status"):
        raw_value = tool_response.get(key)
        if raw_value is None:
            continue
        try:
            return int(raw_value)
        except (TypeError, ValueError):
            return None
    return None


def build_log_record(data: dict[str, Any]) -> dict[str, Any] | None:
    command = tool_input(data).get("command", "")
    if not isinstance(command, str) or not command.strip():
        return None

    provider = detect_cli_provider(command)
    if provider is None:
        return None

    output_preview = truncate_preview(tool_response_output(data))
    record: dict[str, Any] = {
        "timestamp": datetime.now(UTC).isoformat(),
        "tool": "Bash",
        "provider": provider,
        "command": command,
        "output_preview": output_preview,
    }
    exit_code = read_exit_code(data)
    if exit_code is not None:
        record["exit_code"] = exit_code
    return record


def rotate_log_if_needed(
    log_path: str, max_generations: int = MAX_LOG_GENERATIONS
) -> None:
    if not os.path.exists(log_path) or os.path.getsize(log_path) <= MAX_LOG_SIZE:
        return
    # Shift existing generations: .3 → delete, .2 → .3, .1 → .2, current → .1
    for gen in range(max_generations, 0, -1):
        src = f"{log_path}.{gen}"
        if gen == max_generations:
            if os.path.exists(src):
                os.remove(src)
        else:
            dst = f"{log_path}.{gen + 1}"
            if os.path.exists(src):
                os.replace(src, dst)
    os.replace(log_path, f"{log_path}.1")


def main() -> None:
    try:
        if not is_enabled():
            sys.exit(0)

        data = load_stdin_json()
        if data.get("tool_name", "") != "Bash":
            sys.exit(0)

        record = build_log_record(data)
        if record is None:
            sys.exit(0)

        log_path = get_log_path()
        lock_fd = None
        if fcntl is not None:
            try:
                lock_path = log_path + ".lock"
                fd = os.open(lock_path, os.O_WRONLY | os.O_CREAT | os.O_NOFOLLOW, 0o644)
                lock_fd = os.fdopen(fd, "w")
                fcntl.flock(lock_fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
            except OSError:
                if lock_fd:
                    lock_fd.close()
                sys.exit(0)
        try:
            rotate_log_if_needed(log_path)
            with open(log_path, "a", encoding="utf-8") as handle:
                handle.write(json.dumps(record, ensure_ascii=False) + "\n")
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
