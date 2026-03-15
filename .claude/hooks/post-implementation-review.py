#!/usr/bin/env python3
"""
PostToolUse hook: Suggest review after significant Rust implementations.
"""

import difflib
import json
import os
import sys

try:
    import fcntl
except ImportError:
    fcntl = None  # type: ignore[assignment]

from _agent_profiles import provider_label, render_provider_example
from _shared import load_stdin_json, print_hook_error, tool_input, tool_input_text

MAX_PATH_LENGTH = 4096
MAX_CONTENT_LENGTH = 1_000_000
MIN_FILES_FOR_REVIEW = 3
MIN_LINES_FOR_REVIEW = 80
SESSION_ENV_KEYS = [
    "CLAUDE_SESSION_ID",
    "CLAUDE_CONVERSATION_ID",
    "CLAUDE_CHAT_ID",
]
REVIEWER_CAPABILITY = "reviewer"
REVIEW_SUGGESTION_PREFIX = "[Rust Code Review Suggestion]"
REVIEW_SUGGESTION_TEMPLATE = (
    "{prefix} {reason} in this session. "
    "Consider using the `{capability}` capability via {provider_label} for: "
    "ownership/lifetime correctness, idiomatic patterns, error handling, "
    "and performance characteristics. "
    "**Recommended**: `{provider_example}`"
)


def get_state_file() -> str:
    project_dir = os.environ.get("CLAUDE_PROJECT_DIR")
    if project_dir:
        logs_dir = os.path.join(project_dir, ".claude", "logs")
    else:
        logs_dir = os.path.join(
            os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "logs"
        )
    return os.path.join(logs_dir, "post-implementation-review-state.json")


def current_session_marker() -> str:
    for key in SESSION_ENV_KEYS:
        value = os.environ.get(key)
        if value:
            return f"{key}:{value}"
    return f"ppid:{os.getppid()}"


def validate_input(file_path: str, content: str) -> bool:
    if not file_path or len(file_path) > MAX_PATH_LENGTH:
        return False
    if len(content) > MAX_CONTENT_LENGTH:
        return False
    if ".." in file_path:
        return False
    return True


def load_state() -> dict:
    session_marker = current_session_marker()
    try:
        state_file = get_state_file()
        if os.path.exists(state_file):
            with open(state_file, encoding="utf-8") as handle:
                state = json.load(handle)
            if state.get("session_marker") == session_marker:
                state.setdefault("files_changed", [])
                if "total_changed_lines" not in state:
                    state["total_changed_lines"] = state.get("total_lines", 0)
                state.setdefault("review_suggested", False)
                return state
    except Exception:
        pass
    return {
        "session_marker": session_marker,
        "files_changed": [],
        "total_changed_lines": 0,
        "review_suggested": False,
    }


def save_state(state: dict) -> None:
    try:
        state_file = get_state_file()
        state_dir = os.path.dirname(state_file)
        os.makedirs(state_dir, exist_ok=True)
        # Atomic write: write to temp file then rename
        tmp_path = state_file + ".tmp"
        with open(tmp_path, "w", encoding="utf-8") as handle:
            json.dump(state, handle)
        os.replace(tmp_path, state_file)
    except Exception:
        pass


def meaningful_lines(content: str) -> list[str]:
    return [
        line
        for line in content.split("\n")
        if line.strip()
        and not line.strip().startswith("//")
        and not line.strip().startswith("/*")
        and not line.strip().startswith("*")
    ]


def count_lines(content: str) -> int:
    return len(meaningful_lines(content))


def count_changed_lines(old_content: str, new_content: str) -> int:
    before = meaningful_lines(old_content)
    after = meaningful_lines(new_content)
    changes = 0
    for tag, i1, i2, j1, j2 in difflib.SequenceMatcher(a=before, b=after).get_opcodes():
        if tag == "equal":
            continue
        changes += (i2 - i1) + (j2 - j1)
    return changes


def measure_change_lines(
    tool_name: str, tool_input_data: dict, state: dict | None = None
) -> int:
    if tool_name == "Edit":
        old_content = tool_input_text(tool_input_data, "old_string")
        new_content = tool_input_text(tool_input_data, "new_string")
        if old_content or new_content:
            return count_changed_lines(old_content, new_content)
    content = tool_input_text(tool_input_data, "content", "new_string")
    if tool_name == "Write" and state is not None:
        file_path = tool_input_data.get("file_path", "")
        snapshots = state.setdefault("file_snapshots", {})
        old_snapshot = snapshots.get(file_path, "")
        new_meaningful = "\n".join(meaningful_lines(content))
        snapshots[file_path] = new_meaningful
        return count_changed_lines(old_snapshot, new_meaningful)
    return count_lines(content)


def should_suggest_review(state: dict) -> tuple[bool, str]:
    if state.get("review_suggested"):
        return False, ""
    files_count = len(state.get("files_changed", []))
    total_changed_lines = state.get("total_changed_lines", state.get("total_lines", 0))
    if files_count >= MIN_FILES_FOR_REVIEW:
        return True, f"{files_count} Rust files modified"
    if total_changed_lines >= MIN_LINES_FOR_REVIEW:
        return True, f"{total_changed_lines}+ lines of Rust/TOML changes recorded"
    return False, ""


def build_review_message(reason: str) -> str:
    return REVIEW_SUGGESTION_TEMPLATE.format(
        prefix=REVIEW_SUGGESTION_PREFIX,
        reason=reason,
        capability=REVIEWER_CAPABILITY,
        provider_label=provider_label(REVIEWER_CAPABILITY),
        provider_example=render_provider_example(
            REVIEWER_CAPABILITY,
            task="Review this Rust implementation: $(git diff)",
        ),
    )


def main() -> None:
    try:
        data = load_stdin_json()
        if data.get("tool_name", "") not in ["Write", "Edit"]:
            sys.exit(0)

        tool_input_data = tool_input(data)
        tool_name = data.get("tool_name", "")
        file_path = tool_input_data.get("file_path", "")
        content = tool_input_text(tool_input_data, "content", "new_string")
        if not validate_input(file_path, content):
            sys.exit(0)
        if not any(file_path.endswith(ext) for ext in [".rs", ".toml"]):
            sys.exit(0)

        # Acquire lock to prevent concurrent state corruption.
        lock_fd = None
        if fcntl is not None:
            try:
                lock_path = get_state_file() + ".lock"
                os.makedirs(os.path.dirname(lock_path), exist_ok=True)
                fd = os.open(lock_path, os.O_WRONLY | os.O_CREAT | os.O_NOFOLLOW, 0o644)
                lock_fd = os.fdopen(fd, "w")
                fcntl.flock(lock_fd, fcntl.LOCK_EX)
            except OSError:
                if lock_fd:
                    lock_fd.close()
                sys.exit(0)

        try:
            state = load_state()
            if file_path not in state["files_changed"]:
                state["files_changed"].append(file_path)
            state["total_changed_lines"] += measure_change_lines(
                tool_name, tool_input_data, state
            )
            save_state(state)

            should_review, reason = should_suggest_review(state)
            if should_review:
                state["review_suggested"] = True
                save_state(state)
                print(
                    json.dumps(
                        {
                            "hookSpecificOutput": {
                                "hookEventName": "PostToolUse",
                                "additionalContext": build_review_message(reason),
                            }
                        }
                    )
                )
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
