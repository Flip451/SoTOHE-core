#!/usr/bin/env python3
"""
PostToolUse hook: Targeted detector for Rust test/build command failures.
"""

import json
import re
import sys

from _agent_profiles import provider_label, render_provider_example
from _shared import load_stdin_json, print_hook_error, tool_input, tool_response_output

TEST_BUILD_COMMANDS = [
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

FAILURE_PATTERNS = [
    r"error\[E\d{4}\]",
    r"^error:",
    r"FAILED",
    r"test .* FAILED",
    r"FAIL\s+\[",  # cargo nextest single-test failure: "FAIL  [ 0.123s] crate::mod test_name"
    r"thread '.*' panicked",
    r"assertion `left == right` failed",
    r"aborting due to",
    r"Clippy.*error",
]

SIMPLE_ERRORS = [
    "command not found",
    "No such file or directory",
]

DEBUGGER_CAPABILITY = "debugger"
DEBUG_PREFIX = "[Debug Suggestion]"
DEBUG_TEMPLATE = (
    "{prefix} {reason}. For Rust errors, pass the full error including error codes (E0XXX), "
    "file paths, and relevant code context to the `{capability}` capability via {provider_label}. "
    "`{provider_example}`"
)


def is_test_or_build_command(command: str) -> bool:
    command_lower = command.lower()
    return any(cmd in command_lower for cmd in TEST_BUILD_COMMANDS)


def has_complex_failure(output: str) -> tuple[bool, str]:
    for simple in SIMPLE_ERRORS:
        if simple in output and output.count("\n") < 5:
            return False, ""

    failure_count = 0
    for pattern in FAILURE_PATTERNS:
        matches = re.findall(pattern, output, re.IGNORECASE | re.MULTILINE)
        if matches:
            failure_count += len(matches)

    if re.search(r"error\[E\d{4}\]", output):
        return True, "Rust compiler error (with error code)"
    if re.search(r"thread '.*' panicked", output):
        return True, "Test panic detected"
    # Claude Code's Bash tool returns plain text (ANSI stripped), so ANSI
    # false-negatives are not a concern in practice.
    if re.search(r"FAIL\s+\[", output):
        return True, "cargo nextest test failure detected"
    if failure_count >= 2:
        return True, f"Multiple Rust failures detected ({failure_count} issues)"
    return False, ""


def build_debug_message(reason: str) -> str:
    return DEBUG_TEMPLATE.format(
        prefix=DEBUG_PREFIX,
        reason=reason,
        capability=DEBUGGER_CAPABILITY,
        provider_label=provider_label(DEBUGGER_CAPABILITY),
        provider_example=render_provider_example(
            DEBUGGER_CAPABILITY,
            task="Debug this Rust error: <full error>",
        ),
    )


def main() -> None:
    try:
        data = load_stdin_json()
        if data.get("tool_name", "") != "Bash":
            sys.exit(0)

        command = tool_input(data).get("command", "")
        tool_output = tool_response_output(data)

        if not is_test_or_build_command(command):
            sys.exit(0)

        has_failure, reason = has_complex_failure(tool_output)
        if has_failure:
            print(
                json.dumps(
                    {
                        "hookSpecificOutput": {
                            "hookEventName": "PostToolUse",
                            "additionalContext": build_debug_message(reason),
                        }
                    }
                )
            )

        sys.exit(0)

    except Exception as err:
        print_hook_error(err)
        sys.exit(0)


if __name__ == "__main__":
    main()
