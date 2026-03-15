#!/usr/bin/env python3
"""
PostToolUse hook: Broad Bash error detector that suggests the active debugger capability.
"""

import json
import re
import shlex
import sys

from _agent_profiles import (
    provider_command_prefixes,
    provider_label,
    render_provider_example,
)
from _shared import load_stdin_json, print_hook_error, tool_input, tool_response_output

ERROR_PATTERNS = [
    r"error\[E\d{4}\]",
    r"^error: ",
    r"cannot borrow",
    r"does not live long enough",
    r"mismatched types",
    r"trait bound.*not satisfied",
    r"use of moved value",
    r"cannot move out of",
    r"test .* FAILED",
    r"thread '.*' panicked",
    r"assertion `left == right` failed",
    r"aborting due to",
    r"Traceback \(most recent call last\)",
    r"(?:Error|Exception):\s+\S",
    r"FAIL[ED:\s]",
    r"fatal:",
    r"segmentation fault",
]

IGNORE_COMMANDS = [
    "git status",
    "git log",
    "git diff",
    "git branch",
    "git show",
    "git notes show",
    "git notes list",
    "ls",
    "pwd",
    "cat",
    "head",
    "tail",
    "echo",
    "which",
    "tree",
]

TARGETED_TEST_BUILD_COMMANDS = [
    "cargo test",
    "cargo nextest",
    "cargo build",
    "cargo check",
    "cargo clippy",
    "cargo make test",
    "cargo make ci",
    "cargo make clippy",
    "cargo make fmt",
]
MIN_OUTPUT_LENGTH = 20
ERROR_PREFIX = "[Error Detected]"
DEBUGGER_CAPABILITY = "debugger"
RUST_CONTEXT = (
    " This is a Rust compile/borrow error -- pass the FULL error output "
    "including the error code (e.g., E0382) to the active debugger capability for accurate diagnosis."
)


def should_ignore_command(command: str) -> bool:
    stripped = command.strip()
    if any(stripped.startswith(ignore) for ignore in IGNORE_COMMANDS):
        return True
    try:
        lowered = " ".join(shlex.split(stripped.lower()))
    except ValueError:
        lowered = " ".join(stripped.lower().split())
    if any(target in lowered for target in TARGETED_TEST_BUILD_COMMANDS):
        return True
    return any(
        lowered.startswith(prefix.lower()) for prefix in provider_command_prefixes()
    )


def detect_errors(output: str) -> list[str]:
    return [
        pattern
        for pattern in ERROR_PATTERNS
        if re.search(pattern, output, re.IGNORECASE | re.MULTILINE)
    ]


def is_rust_error(output: str) -> bool:
    rust_patterns = [r"error\[E\d{4}\]", r"cannot borrow", r"thread '.*' panicked"]
    return any(re.search(pattern, output) for pattern in rust_patterns)


def build_error_guidance() -> str:
    return (
        f"**Action**: Use the `{DEBUGGER_CAPABILITY}` capability via "
        f"{provider_label(DEBUGGER_CAPABILITY)}. "
        f"`{render_provider_example(DEBUGGER_CAPABILITY, task='Analyze this Rust error: <full error output>')}`"
    )


def build_error_message(error_count: int, tool_output: str) -> str:
    rust_context = RUST_CONTEXT if is_rust_error(tool_output) else ""
    return (
        f"{ERROR_PREFIX} {error_count} error pattern(s) found in command output."
        f"{rust_context} {build_error_guidance()}"
    )


def main() -> None:
    try:
        data = load_stdin_json()
        if data.get("tool_name", "") != "Bash":
            sys.exit(0)

        tool_input_data = tool_input(data)
        command = tool_input_data.get("command", "")
        tool_output = tool_response_output(data)

        if (
            not command
            or not tool_output
            or len(tool_output) < MIN_OUTPUT_LENGTH
            or should_ignore_command(command)
        ):
            sys.exit(0)

        errors = detect_errors(tool_output)
        if errors:
            output = {
                "hookSpecificOutput": {
                    "hookEventName": "PostToolUse",
                    "additionalContext": build_error_message(len(errors), tool_output),
                }
            }
            print(json.dumps(output))

        sys.exit(0)

    except Exception as err:
        print_hook_error(err)
        sys.exit(0)


if __name__ == "__main__":
    main()
