#!/usr/bin/env python3
"""
PostToolUse hook: Suggest design review after Task tool (planning).
"""

import json
import sys

from _agent_profiles import provider_label, render_provider_example
from _shared import load_stdin_json, print_hook_error, tool_input, tool_response_text

PLAN_INPUT_KEYWORDS = [
    "plan",
    "architecture",
    "spec",
    "track:plan",
    "track",
    "計画",
    "設計",
    "仕様",
]

PLAN_OUTPUT_KEYWORDS = [
    "plan.md",
    "spec.md",
    "implementation plan",
    "計画",
    "仕様",
]

EXCLUDED_SUBAGENT_TYPES = {"explore"}
REVIEWER_CAPABILITY = "reviewer"
DESIGN_REVIEW_PREFIX = "[Design Review Suggestion]"
DESIGN_REVIEW_TEMPLATE = (
    "{prefix} A planning task just completed. "
    "Before starting implementation, consider using the `{capability}` capability via "
    "{provider_label} to validate the Rust design for: trait correctness, ownership model, "
    "async compatibility (Send+Sync bounds), and error type hierarchy. "
    "`{provider_example}`"
)


def looks_like_plan_task(task_input: dict, tool_response: dict) -> bool:
    subagent_type = (task_input.get("subagent_type") or "").lower()
    if subagent_type in EXCLUDED_SUBAGENT_TYPES:
        return False

    combined_input = (
        (task_input.get("prompt") or "") + " " + (task_input.get("description") or "")
    ).lower()
    response_text = tool_response_text(tool_response).lower()

    input_match = any(keyword in combined_input for keyword in PLAN_INPUT_KEYWORDS)
    output_match = any(keyword in response_text for keyword in PLAN_OUTPUT_KEYWORDS)
    # Require both signals to avoid false positives from research/explore tasks.
    return input_match and output_match


def build_design_review_message() -> str:
    return DESIGN_REVIEW_TEMPLATE.format(
        prefix=DESIGN_REVIEW_PREFIX,
        capability=REVIEWER_CAPABILITY,
        provider_label=provider_label(REVIEWER_CAPABILITY),
        provider_example=render_provider_example(
            REVIEWER_CAPABILITY,
            task="Review this Rust design plan: {plan summary}",
        ),
    )


def main() -> None:
    try:
        data = load_stdin_json()
        if data.get("tool_name", "") != "Task":
            sys.exit(0)

        tool_input_data = tool_input(data)
        tool_response = data.get("tool_response", {})
        if not looks_like_plan_task(tool_input_data, tool_response):
            sys.exit(0)

        output = {
            "hookSpecificOutput": {
                "hookEventName": "PostToolUse",
                "additionalContext": build_design_review_message(),
            }
        }
        print(json.dumps(output))
        sys.exit(0)

    except Exception as err:
        print_hook_error(err)
        sys.exit(0)


if __name__ == "__main__":
    main()
