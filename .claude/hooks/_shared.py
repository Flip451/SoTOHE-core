#!/usr/bin/env python3
"""
Shared helpers for local Claude Python hooks.
"""

from __future__ import annotations

import json
import os
import sys
from typing import Any

INPUT_TEXT_KEYS = ("text", "content", "message", "new_string")
RESPONSE_TEXT_KEYS = ("stdout", "stderr", "content", "result", "text", "message")


def load_stdin_json() -> dict[str, Any]:
    return json.load(sys.stdin)


def tool_input(data: dict[str, Any]) -> dict[str, Any]:
    value = data.get("tool_input", {})
    return value if isinstance(value, dict) else {}


def _string_parts(value: Any, preferred_keys: tuple[str, ...]) -> list[str]:
    if isinstance(value, str):
        return [value] if value else []
    if isinstance(value, list):
        parts: list[str] = []
        for item in value:
            parts.extend(_string_parts(item, preferred_keys))
        return parts
    if isinstance(value, dict):
        parts = []
        for key in preferred_keys:
            if key in value:
                parts.extend(_string_parts(value[key], preferred_keys))
        if parts:
            return parts
        for item in value.values():
            if isinstance(item, (dict, list)):
                parts.extend(_string_parts(item, preferred_keys))
        return parts
    return []


def flatten_text(
    value: Any, preferred_keys: tuple[str, ...] = RESPONSE_TEXT_KEYS
) -> str:
    return "\n".join(_string_parts(value, preferred_keys))


def tool_response_text(response: Any) -> str:
    if isinstance(response, dict):
        parts = []
        for key in ("stdout", "stderr", "content", "result"):
            value = flatten_text(response.get(key, ""), RESPONSE_TEXT_KEYS)
            if value:
                parts.append(value)
        return "\n".join(parts)
    return flatten_text(response, RESPONSE_TEXT_KEYS) or str(response)


def tool_response_output(data: dict[str, Any]) -> str:
    return tool_response_text(data.get("tool_response", {}))


def tool_input_text(tool_input_data: dict[str, Any], *keys: str) -> str:
    parts = [
        flatten_text(tool_input_data.get(key, ""), INPUT_TEXT_KEYS) for key in keys
    ]
    return "\n".join(part for part in parts if part)


def project_dir() -> str:
    return os.environ.get("CLAUDE_PROJECT_DIR", os.getcwd())


def print_hook_error(err: Exception) -> None:
    print(f"Hook error: {err}", file=sys.stderr)
