#!/usr/bin/env python3
"""
PreToolUse hook: Suggest using the active researcher capability before WebSearch/WebFetch.
"""

import json
import sys

from _agent_profiles import provider_label, render_provider_example
from _shared import load_stdin_json, print_hook_error, tool_input

RESEARCH_CAPABILITY = "researcher"
RESEARCH_SUGGESTION_PREFIX = "[Research Suggestion]"
RESEARCH_SUGGESTION_TEMPLATE = (
    "{prefix} For '{query}', "
    "consider using the `{capability}` capability via {provider_label} instead of WebSearch/WebFetch. "
    "Example: `{provider_example}` "
    "Save research results to knowledge/research/."
)


def build_research_suggestion(query: str) -> str:
    return RESEARCH_SUGGESTION_TEMPLATE.format(
        prefix=RESEARCH_SUGGESTION_PREFIX,
        query=query,
        capability=RESEARCH_CAPABILITY,
        provider_label=provider_label(RESEARCH_CAPABILITY),
        provider_example=render_provider_example(RESEARCH_CAPABILITY, task=query),
    )


def main() -> None:
    try:
        data = load_stdin_json()
        tool_name = data.get("tool_name", "")
        if tool_name not in ["WebSearch", "WebFetch"]:
            sys.exit(0)

        tool_input_data = tool_input(data)
        query = tool_input_data.get("query", "") or tool_input_data.get("url", "")

        print(
            json.dumps(
                {
                    "hookSpecificOutput": {
                        "hookEventName": "PreToolUse",
                        "additionalContext": build_research_suggestion(query),
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
