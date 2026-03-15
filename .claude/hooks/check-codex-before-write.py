#!/usr/bin/env python3
"""
PreToolUse hook: Check if planner consultation is recommended before Write/Edit.
"""

import json
import sys

from _agent_profiles import provider_label, render_provider_example
from _shared import load_stdin_json, print_hook_error, tool_input, tool_input_text

MAX_PATH_LENGTH = 4096
MAX_CONTENT_LENGTH = 1_000_000

DESIGN_INDICATORS_PATH = [
    "DESIGN.md",
    "ARCHITECTURE.md",
    "domain/",
    "/domain/",
    "port/",
    "/port/",
    "adapter/",
    "/adapter/",
    "usecase/",
    "/usecase/",
    "infrastructure/",
    "/infrastructure/",
    "lib.rs",
    "mod.rs",
]

DESIGN_INDICATORS_CONTENT = [
    "pub trait ",
    "impl Trait",
    "async fn",
    "#[async_trait]",
    "Arc<dyn ",
    "Box<dyn ",
    "PhantomData",
    "unsafe ",
]

SIMPLE_EDIT_PATHS = [
    ".gitignore",
    "README.md",
    "CHANGELOG.md",
    "Cargo.toml",
    ".env.example",
    "track/product.md",
    "track/registry.md",
]

CONTENT_REASON_TEMPLATE = (
    "Content contains '{indicator}' -- Rust ownership/trait/async design pattern"
)
LONG_SOURCE_REASON = (
    "New Rust source file -- consider architecture review before writing"
)
PATH_REASON_TEMPLATE = (
    "File path contains '{indicator}' -- likely a Rust design/domain file"
)
PLANNER_CAPABILITY = "planner"
CONSULTATION_PREFIX = "[Design Consultation Reminder]"
CONSULTATION_TEMPLATE = (
    "{prefix} {reason}. "
    "For Rust trait design, ownership patterns, and async architecture, "
    "use the `{capability}` capability via {provider_label}. "
    "**Recommended**: `{provider_example}`"
)


def validate_input(file_path: str, content: str) -> bool:
    if not file_path or len(file_path) > MAX_PATH_LENGTH:
        return False
    if len(content) > MAX_CONTENT_LENGTH:
        return False
    if ".." in file_path:
        return False
    return True


def should_suggest_design_review(
    file_path: str, content: str | None = None
) -> tuple[bool, str]:
    filepath_lower = file_path.lower()

    for pattern in SIMPLE_EDIT_PATHS:
        if pattern.lower() in filepath_lower:
            return False, ""

    if content:
        for indicator in DESIGN_INDICATORS_CONTENT:
            if indicator in content:
                return True, CONTENT_REASON_TEMPLATE.format(indicator=indicator)

        if (
            file_path.endswith(".rs")
            and ("/src/" in file_path or file_path.startswith("src/"))
            and len(content) > 200
        ):
            return True, LONG_SOURCE_REASON

    for indicator in DESIGN_INDICATORS_PATH:
        if indicator.lower() in filepath_lower:
            return True, PATH_REASON_TEMPLATE.format(indicator=indicator)

    return False, ""


def should_suggest_codex(
    file_path: str, content: str | None = None
) -> tuple[bool, str]:
    return should_suggest_design_review(file_path, content)


def build_consultation_message(reason: str) -> str:
    return CONSULTATION_TEMPLATE.format(
        prefix=CONSULTATION_PREFIX,
        reason=reason,
        capability=PLANNER_CAPABILITY,
        provider_label=provider_label(PLANNER_CAPABILITY),
        provider_example=render_provider_example(
            PLANNER_CAPABILITY,
            task="Review this Rust design: {description}",
        ),
    )


def main() -> None:
    try:
        data = load_stdin_json()
        tool_input_data = tool_input(data)
        file_path = tool_input_data.get("file_path", "")
        content = tool_input_text(tool_input_data, "content", "new_string")

        if not validate_input(file_path, content):
            sys.exit(0)

        should_suggest, reason = should_suggest_design_review(file_path, content)
        if should_suggest:
            output = {
                "hookSpecificOutput": {
                    "hookEventName": "PreToolUse",
                    "additionalContext": build_consultation_message(reason),
                }
            }
            print(json.dumps(output))

        sys.exit(0)

    except Exception as err:
        print_hook_error(err)
        sys.exit(0)


if __name__ == "__main__":
    main()
