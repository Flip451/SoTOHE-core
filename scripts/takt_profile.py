#!/usr/bin/env python3
"""
Render takt personas from the active agent profile and run takt with host overrides.
"""

from __future__ import annotations

import argparse
import fcntl
import importlib.util
import json
import os
import re
import shlex
import subprocess
import sys
import uuid
from collections.abc import Generator
from contextlib import contextmanager
from copy import deepcopy
from datetime import UTC, datetime
from pathlib import Path
from typing import Any

try:
    import yaml
except ModuleNotFoundError:  # pragma: no cover - exercised via runtime error path
    yaml = None

PLACEHOLDER_PATTERN = re.compile(r"{{([A-Z0-9_]+)}}")
QUEUE_SNAPSHOT_PROFILE_NAME = "__takt_queue_snapshot__"
DEFAULT_CIRCUIT_BREAKER_FAILURE_LIMIT = 3
CIRCUIT_BREAKER_FAILURE_LIMIT_ENV = "TAKT_CIRCUIT_BREAKER_LIMIT"
TASK_STATUS_PENDING = "pending"
TASK_STATUS_BLOCKED = "blocked"
TASK_FAILURE_STREAK_KEY = "agent_failure_streak"
TASK_LAST_FAILURE_AT_KEY = "agent_last_failure_at"
TASK_LAST_FAILURE_EXIT_CODE_KEY = "agent_last_failure_exit_code"
TASK_BLOCKED_AT_KEY = "agent_circuit_breaker_blocked_at"
TASK_BLOCKED_REASON_KEY = "agent_circuit_breaker_reason"
TASK_LOOP_ANALYSIS_PROVIDER_KEY = "agent_loop_analysis_provider"
TASK_LOOP_ANALYSIS_DECISION_KEY = "agent_loop_analysis_decision"
TASK_LOOP_ANALYSIS_CONFIDENCE_KEY = "agent_loop_analysis_confidence"
TASK_LOOP_ANALYSIS_RATIONALE_KEY = "agent_loop_analysis_rationale"
TASK_LOOP_ANALYSIS_AT_KEY = "agent_loop_analysis_at"
LOOP_ANALYSIS_DECISION_LOOP = "loop"
LOOP_ANALYSIS_DECISION_TRANSIENT = "transient"
LOOP_ANALYSIS_DECISION_INVALID = "invalid_response"
LOOP_ANALYSIS_DECISION_UNKNOWN = "unknown"
HANDOFFS_DIR = "handoffs"
ROLE_NAMES = {
    "planner": "planning",
    "implementer": "implementation",
    "reviewer": "review",
}
ROLE_SECOND_OPINION_TASKS = {
    "planner": "Review this Rust design and tighten the implementation plan.",
    "implementer": "Suggest a minimal Rust implementation fix for this blocker.",
    "reviewer": "Review this Rust implementation for correctness and risks.",
    "debugger": "Debug this Rust issue: <full error>",
    "researcher": "Rust pattern / crate issue: <topic>",
}


def project_root() -> Path:
    return Path(__file__).resolve().parent.parent


def profiles_path(root: Path | None = None) -> Path:
    resolved_root = root or project_root()
    return resolved_root / ".claude" / "agent-profiles.json"


def tasks_path(root: Path | None = None) -> Path:
    resolved_root = root or project_root()
    return resolved_root / ".takt" / "tasks.yaml"


@contextmanager
def _tasks_file_lock(root: Path | None = None) -> Generator[None, None, None]:
    """Acquire an exclusive file lock on tasks.yaml for read-modify-write safety."""
    lock_path = tasks_path(root).parent / "tasks.yaml.lock"
    lock_path.parent.mkdir(parents=True, exist_ok=True)
    fd = open(lock_path, "w")
    try:
        fcntl.flock(fd, fcntl.LOCK_EX)
        yield
    finally:
        fcntl.flock(fd, fcntl.LOCK_UN)
        fd.close()


def load_agent_profiles_module(root: Path | None = None):
    resolved_root = root or project_root()
    hooks_dir = resolved_root / ".claude" / "hooks"
    module_path = hooks_dir / "_agent_profiles.py"
    if str(hooks_dir) not in sys.path:
        sys.path.insert(0, str(hooks_dir))
    spec = importlib.util.spec_from_file_location("takt_agent_profiles", module_path)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"Could not load agent profile helper: {module_path}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def load_profiles(root: Path | None = None) -> dict[str, Any]:
    resolved_root = root or project_root()
    module = load_agent_profiles_module(resolved_root)
    return module.load_profiles(path=profiles_path(resolved_root))


def host_provider(
    root: Path | None = None, profiles: dict[str, Any] | None = None
) -> str:
    resolved_root = root or project_root()
    module = load_agent_profiles_module(resolved_root)
    resolved_profiles = (
        profiles if profiles is not None else load_profiles(resolved_root)
    )
    return module.takt_host_provider(profiles=resolved_profiles)


def host_model(root: Path | None = None, profiles: dict[str, Any] | None = None) -> str:
    resolved_root = root or project_root()
    module = load_agent_profiles_module(resolved_root)
    resolved_profiles = (
        profiles if profiles is not None else load_profiles(resolved_root)
    )
    return module.takt_host_model(profiles=resolved_profiles)


def host_label(root: Path | None = None, profiles: dict[str, Any] | None = None) -> str:
    resolved_root = root or project_root()
    module = load_agent_profiles_module(resolved_root)
    resolved_profiles = (
        profiles if profiles is not None else load_profiles(resolved_root)
    )
    return module.takt_host_label(profiles=resolved_profiles)


def renderable_persona_files(root: Path | None = None) -> list[Path]:
    resolved_root = root or project_root()
    return sorted((resolved_root / ".takt" / "personas").glob("*.md"))


def runtime_personas_dir(root: Path | None = None) -> Path:
    resolved_root = root or project_root()
    return resolved_root / ".takt" / "runtime" / "personas"


def snapshot_key_for_capability(capability: str) -> str:
    return f"agent_{capability}"


def task_snapshot_keys(root: Path | None = None) -> tuple[str, ...]:
    resolved_root = root or project_root()
    module = load_agent_profiles_module(resolved_root)
    capability_keys = tuple(
        snapshot_key_for_capability(capability)
        for capability in module.REQUIRED_CAPABILITIES
    )
    return (
        "agent_profile_name",
        "agent_profile_version",
        *capability_keys,
        f"agent_{module.TAKT_HOST_PROVIDER_KEY}",
        f"agent_{module.TAKT_HOST_MODEL_KEY}",
    )


def _looks_like_shell_command(text: str) -> bool:
    stripped = text.strip()
    if not stripped:
        return False
    if stripped.startswith("/") or stripped.lower().startswith("continue "):
        return False
    return True


def _format_inline_example(text: str) -> str:
    stripped = text.strip()
    if _looks_like_shell_command(stripped):
        return f"`{stripped}`"
    return stripped


def _rendered_role_example(
    capability: str, profiles: dict[str, Any], root: Path | None = None
) -> str | None:
    resolved_root = root or project_root()
    module = load_agent_profiles_module(resolved_root)
    example = module.render_provider_example(
        capability,
        task=ROLE_SECOND_OPINION_TASKS[capability],
        profiles=profiles,
    )
    if not _looks_like_shell_command(example):
        return None
    return example


def build_role_profile_note(
    capability: str, profiles: dict[str, Any], root: Path | None = None
) -> str:
    resolved_root = root or project_root()
    module = load_agent_profiles_module(resolved_root)
    host = host_label(resolved_root, profiles)
    model = host_model(resolved_root, profiles)
    specialist = module.provider_label(capability, profiles=profiles)
    role_name = ROLE_NAMES[capability]
    if specialist == host:
        return (
            f"This takt run is hosted by {host} (model: `{model}`). "
            f"The active profile routes {role_name} work to the host provider, so continue in "
            "this movement unless the workflow explicitly sends work back for replanning or fixes."
        )

    note = (
        f"This takt run is hosted by {host} (model: `{model}`). "
        f"The active profile uses {specialist} as the {role_name} specialist for second-opinion "
        "work outside this run."
    )
    example = _rendered_role_example(capability, profiles, resolved_root)
    if example is not None:
        note += f" Example handoff: `{example}`."
    return note


def build_debugger_profile_note(
    profiles: dict[str, Any], root: Path | None = None
) -> str:
    resolved_root = root or project_root()
    module = load_agent_profiles_module(resolved_root)
    host = host_label(resolved_root, profiles)
    model = host_model(resolved_root, profiles)
    debugger = module.provider_label("debugger", profiles=profiles)
    researcher = module.provider_label("researcher", profiles=profiles)
    return (
        f"This takt run is hosted by {host} (model: `{model}`). "
        f"The active profile routes debugger work to {debugger} and external research to "
        f"{researcher}."
    )


def build_support_note(
    capability: str, purpose: str, profiles: dict[str, Any], root: Path | None = None
) -> str:
    resolved_root = root or project_root()
    module = load_agent_profiles_module(resolved_root)
    label = module.provider_label(capability, profiles=profiles)
    example = module.render_provider_example(
        capability,
        task=ROLE_SECOND_OPINION_TASKS[capability],
        profiles=profiles,
    )
    return f"- {label} for {purpose}: {_format_inline_example(example)}"


def build_template_context(
    profiles: dict[str, Any], root: Path | None = None
) -> dict[str, str]:
    resolved_root = root or project_root()
    module = load_agent_profiles_module(resolved_root)
    return {
        "PLANNER_PROFILE_NOTE": build_role_profile_note(
            "planner", profiles, resolved_root
        ),
        "IMPLEMENTER_PROFILE_NOTE": build_role_profile_note(
            "implementer", profiles, resolved_root
        ),
        "REVIEWER_PROFILE_NOTE": build_role_profile_note(
            "reviewer", profiles, resolved_root
        ),
        "DEBUGGER_PROFILE_NOTE": build_debugger_profile_note(profiles, resolved_root),
        "DEBUGGER_PROVIDER_LABEL": module.provider_label("debugger", profiles=profiles),
        "RESEARCHER_PROVIDER_LABEL": module.provider_label(
            "researcher", profiles=profiles
        ),
        "DEBUGGER_SUPPORT_NOTE": build_support_note(
            "debugger",
            "ownership, trait design, and compiler diagnostics",
            profiles,
            resolved_root,
        ),
        "RESEARCH_SUPPORT_NOTE": build_support_note(
            "researcher",
            "crate research, pattern research, or external references",
            profiles,
            resolved_root,
        ),
    }


def render_template(template_text: str, context: dict[str, str]) -> str:
    def replace(match: re.Match[str]) -> str:
        key = match.group(1)
        if key not in context:
            raise ValueError(f"Unknown template placeholder: {key}")
        return context[key]

    rendered = PLACEHOLDER_PATTERN.sub(replace, template_text)
    unreplaced = PLACEHOLDER_PATTERN.findall(rendered)
    if unreplaced:
        missing = ", ".join(sorted(set(unreplaced)))
        raise ValueError(f"Unreplaced template placeholders remain: {missing}")
    return rendered


def render_personas(
    root: Path | None = None, profiles: dict[str, Any] | None = None
) -> list[Path]:
    resolved_root = root or project_root()
    resolved_profiles = (
        profiles if profiles is not None else load_profiles(resolved_root)
    )
    context = build_template_context(resolved_profiles, resolved_root)
    destination_dir = runtime_personas_dir(resolved_root)
    destination_dir.mkdir(parents=True, exist_ok=True)

    written: list[Path] = []
    for template_path in renderable_persona_files(resolved_root):
        rendered = render_template(template_path.read_text(encoding="utf-8"), context)
        destination_path = destination_dir / template_path.name
        destination_path.write_text(rendered, encoding="utf-8")
        written.append(destination_path)
    return written


def require_yaml_support() -> Any:
    if yaml is None:
        raise RuntimeError(
            "PyYAML is required for .takt/tasks.yaml parsing; install it with "
            "`uv venv .venv && uv pip install --python .venv/bin/python -r requirements-python.txt` "
            "or use a Python with PyYAML installed."
        )
    return yaml


class TasksLoader(yaml.SafeLoader if yaml is not None else object):
    pass


class TasksDumper(yaml.SafeDumper if yaml is not None else object):
    pass


if yaml is not None:
    for first_letter, resolvers in list(TasksLoader.yaml_implicit_resolvers.items()):
        TasksLoader.yaml_implicit_resolvers[first_letter] = [
            (tag, pattern)
            for tag, pattern in resolvers
            if tag != "tag:yaml.org,2002:timestamp"
        ]

    def _represent_multiline_string(dumper: yaml.SafeDumper, data: str) -> Any:
        style = "|" if "\n" in data else None
        return dumper.represent_scalar("tag:yaml.org,2002:str", data, style=style)

    TasksDumper.add_representer(str, _represent_multiline_string)


def parse_tasks_document(text: str) -> dict[str, Any] | None:
    yaml_module = require_yaml_support()
    return yaml_module.load(text, Loader=TasksLoader)


def dump_tasks_document(document: dict[str, Any]) -> str:
    yaml_module = require_yaml_support()
    return yaml_module.dump(
        document,
        Dumper=TasksDumper,
        sort_keys=False,
        allow_unicode=True,
        default_flow_style=False,
    )


def parse_tasks_file(root: Path | None = None) -> list[dict[str, Any]]:
    task_file = tasks_path(root)
    if not task_file.exists():
        return []
    try:
        document = parse_tasks_document(task_file.read_text(encoding="utf-8"))
    except RuntimeError as err:
        raise ValueError(f"Failed to parse {task_file}: {err}") from err
    except Exception as err:
        if yaml is not None and isinstance(err, yaml.YAMLError):
            raise ValueError(f"Failed to parse {task_file}: {err}") from err
        raise
    if document is None:
        return []
    if not isinstance(document, dict):
        raise ValueError(f"Expected top-level mapping in {task_file}")
    tasks = document.get("tasks", [])
    if tasks is None:
        return []
    if not isinstance(tasks, list):
        raise ValueError(f"Expected 'tasks' list in {task_file}")

    parsed_tasks: list[dict[str, Any]] = []
    for index, task in enumerate(tasks):
        if not isinstance(task, dict):
            raise ValueError(f"Expected task mapping at index {index} in {task_file}")
        parsed_tasks.append(dict(task))
    return parsed_tasks


def write_tasks_file(tasks: list[dict[str, Any]], root: Path | None = None) -> None:
    task_file = tasks_path(root)
    task_file.parent.mkdir(parents=True, exist_ok=True)
    try:
        rendered = dump_tasks_document({"tasks": tasks})
    except (RuntimeError, TypeError, ValueError) as err:
        raise ValueError(f"Failed to write {task_file}: {err}") from err
    task_file.write_text(rendered, encoding="utf-8")


def active_profile_snapshot(
    root: Path | None = None, profiles: dict[str, Any] | None = None
) -> dict[str, str]:
    resolved_root = root or project_root()
    module = load_agent_profiles_module(resolved_root)
    resolved_profiles = (
        profiles if profiles is not None else load_profiles(resolved_root)
    )
    profile_name = module.active_profile_name(resolved_profiles)
    active_mapping = module.active_profile(resolved_profiles)
    snapshot: dict[str, str] = {
        "agent_profile_name": profile_name,
        "agent_profile_version": str(module.PROFILE_VERSION),
    }
    for capability in module.REQUIRED_CAPABILITIES:
        snapshot[snapshot_key_for_capability(capability)] = active_mapping[capability]
    snapshot[f"agent_{module.TAKT_HOST_PROVIDER_KEY}"] = module.takt_host_provider(
        profiles=resolved_profiles
    )
    snapshot[f"agent_{module.TAKT_HOST_MODEL_KEY}"] = module.takt_host_model(
        profiles=resolved_profiles
    )
    snapshot[TASK_FAILURE_STREAK_KEY] = "0"
    return snapshot


def append_snapshot_to_last_task(
    snapshot: dict[str, str], root: Path | None = None
) -> None:
    with _tasks_file_lock(root):
        task_file = tasks_path(root)
        tasks = parse_tasks_file(root)
        if not tasks:
            raise ValueError(f"No task item found in {task_file}")
        tasks[-1].update(snapshot)
        write_tasks_file(tasks, root)


def task_display_name(task: dict[str, Any]) -> str:
    for key in ("name", "slug", "summary"):
        value = task.get(key)
        if isinstance(value, str) and value:
            return value
    return "<unknown-task>"


def circuit_breaker_failure_limit() -> int:
    raw_limit = os.environ.get(
        CIRCUIT_BREAKER_FAILURE_LIMIT_ENV,
        str(DEFAULT_CIRCUIT_BREAKER_FAILURE_LIMIT),
    ).strip()
    try:
        limit = int(raw_limit)
    except ValueError as err:
        raise ValueError(
            f"{CIRCUIT_BREAKER_FAILURE_LIMIT_ENV} must be an integer, got: {raw_limit!r}"
        ) from err
    if limit < 1:
        raise ValueError(
            f"{CIRCUIT_BREAKER_FAILURE_LIMIT_ENV} must be >= 1, got: {limit}"
        )
    return limit


def now_utc_iso() -> str:
    return datetime.now(UTC).isoformat().replace("+00:00", "Z")


def first_pending_task(tasks: list[dict[str, Any]]) -> dict[str, Any] | None:
    for task in tasks:
        if task.get("status") == TASK_STATUS_PENDING:
            return task
    return None


def find_task_index(
    tasks: list[dict[str, Any]], target_task: dict[str, Any]
) -> int | None:
    identity_keys = ("task_dir", "slug", "name", "summary", "created_at")
    for identity_key in identity_keys:
        identity_value = target_task.get(identity_key)
        if not isinstance(identity_value, str) or not identity_value:
            continue
        for index, task in enumerate(tasks):
            if task.get(identity_key) == identity_value:
                return index
    return None


def read_failure_streak(task: dict[str, Any]) -> int:
    raw_value = task.get(TASK_FAILURE_STREAK_KEY)
    if raw_value is None:
        return 0
    try:
        streak = int(str(raw_value))
    except ValueError as err:
        raise ValueError(
            f"Task '{task_display_name(task)}' has invalid {TASK_FAILURE_STREAK_KEY}: {raw_value!r}"
        ) from err
    if streak < 0:
        raise ValueError(
            f"Task '{task_display_name(task)}' has invalid {TASK_FAILURE_STREAK_KEY}: {raw_value!r}"
        )
    return streak


def _slugify(text: str, max_len: int = 40) -> str:
    slug = re.sub(r"[^a-z0-9]+", "-", text.lower()).strip("-")
    return slug[:max_len].rstrip("-") or "unknown"


def handoff_path(task: dict[str, Any] | None = None, root: Path | None = None) -> Path:
    resolved_root = root or project_root()
    handoffs_dir = resolved_root / ".takt" / HANDOFFS_DIR
    if task is None:
        slug = "unknown"
    else:
        raw_name = task_display_name(task)
        slug = _slugify(raw_name)
        if slug == "unknown":
            # Fallback: try slug key, then task_dir basename
            for fallback_key in ("slug", "task_dir"):
                val = task.get(fallback_key)
                if isinstance(val, str) and val:
                    candidate = _slugify(
                        Path(val).name if fallback_key == "task_dir" else val
                    )
                    if candidate != "unknown":
                        slug = candidate
                        break
    stamp = datetime.now(UTC).strftime("%Y%m%dT%H%M%SZ")
    short_id = uuid.uuid4().hex[:8]
    return handoffs_dir / f"handoff-{slug}-{stamp}-{short_id}.md"


def read_text_excerpt(path: Path, max_chars: int = 2400) -> str:
    if not path.exists():
        return "(not available)"
    text = path.read_text(encoding="utf-8", errors="replace").strip()
    if not text:
        return "(empty)"
    if len(text) <= max_chars:
        return text
    return text[:max_chars].rstrip() + "\n... (truncated)"


def sanitize_loop_prompt(text: str) -> str:
    return " ".join(text.replace('"', "'").split())


def sanitize_task_field_value(value: Any) -> str:
    text = str(value)
    return text.replace("\r", " ").replace("\n", " ").strip()


def _git_diff_stat(root: Path) -> str:
    """Return bounded git diff --stat + untracked files, or empty string on failure."""
    parts: list[str] = []
    try:
        diff_result = subprocess.run(
            ["git", "diff", "--stat", "HEAD"],
            capture_output=True,
            text=True,
            check=False,
            cwd=root,
            timeout=10,
        )
        if diff_result.returncode == 0 and diff_result.stdout.strip():
            parts.append(diff_result.stdout.strip())

        untracked_result = subprocess.run(
            ["git", "ls-files", "--others", "--exclude-standard"],
            capture_output=True,
            text=True,
            check=False,
            cwd=root,
            timeout=10,
        )
        if untracked_result.returncode == 0 and untracked_result.stdout.strip():
            untracked = untracked_result.stdout.strip().splitlines()
            parts.append("Untracked files:\n" + "\n".join(f"  {f}" for f in untracked))
    except (OSError, subprocess.TimeoutExpired):
        pass

    if not parts:
        return ""
    combined = "\n".join(parts)
    lines = combined.splitlines()
    if len(lines) > 30:
        lines = lines[:29] + [f"... ({len(lines) - 29} more lines)"]
    return "\n".join(lines)


def build_loop_analysis_prompt(
    task: dict[str, Any],
    failure_streak: int,
    failure_limit: int,
    root: Path | None = None,
) -> str:
    resolved_root = root or project_root()
    failure_excerpt = read_text_excerpt(resolved_root / ".takt" / "last-failure.log")
    debug_excerpt = read_text_excerpt(resolved_root / ".takt" / "debug-report.md")
    change_stat = _git_diff_stat(resolved_root)
    change_section = (
        f"Recent code changes (git diff --stat):\n{change_stat}\n\n"
        if change_stat
        else "Recent code changes: none detected.\n\n"
    )
    return (
        "You are analyzing a repeated takt queue failure for loop detection.\n"
        f"Task: {task_display_name(task)}\n"
        f"Piece: {task.get('piece', '<unknown-piece>')}\n"
        f"Failure streak: {failure_streak}\n"
        f"Analysis trigger threshold: {failure_limit}\n\n"
        "Failure output excerpt:\n"
        f"{failure_excerpt}\n\n"
        "Debug report excerpt:\n"
        f"{debug_excerpt}\n\n"
        + change_section
        + "Decide whether this is a self-healing loop (same root cause repeating without progress).\n"
        "If code changes exist but the error is the same, this may still be a loop.\n"
        "Return ONLY JSON with this schema:\n"
        '{"loop_detected": true|false, "confidence": 0.0-1.0, "rationale": "short reason"}'
    )


def extract_json_objects(text: str) -> list[dict[str, Any]]:
    stripped = text.strip()
    if not stripped:
        return []

    decoder = json.JSONDecoder()
    objects: list[dict[str, Any]] = []
    index = 0
    while index < len(stripped):
        start = stripped.find("{", index)
        if start == -1:
            break
        try:
            payload, end = decoder.raw_decode(stripped[start:])
        except json.JSONDecodeError:
            index = start + 1
            continue
        index = start + max(end, 1)
        if isinstance(payload, dict):
            objects.append(payload)
    return objects


def _try_strict_json_object(text: str) -> dict[str, Any] | None:
    """Parse the entire stripped text as a single JSON object."""
    stripped = text.strip()
    if not stripped:
        return None
    try:
        payload = json.loads(stripped)
    except json.JSONDecodeError:
        return None
    return payload if isinstance(payload, dict) else None


def extract_first_json_object(text: str) -> dict[str, Any] | None:
    """Extract the first JSON object from text."""
    strict = _try_strict_json_object(text)
    if strict is not None:
        return strict
    objects = extract_json_objects(text)
    return objects[0] if objects else None


def _loop_analysis_invalid_result(rationale: str) -> dict[str, Any]:
    return {
        "decision": LOOP_ANALYSIS_DECISION_INVALID,
        "confidence": 0.0,
        "rationale": rationale,
    }


def _validate_loop_analysis_payload(payload: dict[str, Any]) -> dict[str, Any]:
    """Validate and normalise a loop-analysis JSON payload."""
    loop_detected = payload.get("loop_detected")
    if not isinstance(loop_detected, bool):
        return _loop_analysis_invalid_result(
            "Loop analysis JSON is missing boolean field 'loop_detected'."
        )

    raw_confidence = payload.get("confidence", 0.0)
    try:
        confidence = float(raw_confidence)
    except (TypeError, ValueError):
        confidence = 0.0
    confidence = max(0.0, min(1.0, confidence))

    raw_rationale = payload.get("rationale", "")
    rationale = str(raw_rationale).strip() if raw_rationale is not None else ""
    if not rationale:
        rationale = "No rationale was provided by loop analysis."

    return {
        "decision": LOOP_ANALYSIS_DECISION_LOOP
        if loop_detected
        else LOOP_ANALYSIS_DECISION_TRANSIENT,
        "confidence": confidence,
        "rationale": rationale,
    }


def parse_loop_analysis_result(output: str) -> dict[str, Any]:
    """Parse the loop-analysis response into a normalised result dict.

    Decision tree (explicit branches):

    1. Strict branch — entire output is a clean JSON object.
       Uses json.loads so there is no ambiguity from surrounding noise.

    2. Invalid branch — output attempted JSON but violated the JSON-only contract
       or returned a schema-incompatible payload.

    3. Failure branch — no JSON-like payload found at all.
    """
    stripped = output.strip()

    # Branch 1: strict — whole output is JSON (LLM followed the contract).
    strict_obj = _try_strict_json_object(stripped)
    if strict_obj is not None:
        return _validate_loop_analysis_payload(strict_obj)

    # Branch 2: invalid — embedded JSON, multiple objects, trailing text, or malformed JSON-looking output.
    candidates = extract_json_objects(stripped)
    if candidates:
        return _loop_analysis_invalid_result(
            "Loop analysis response violated the JSON-only contract; return exactly one JSON object and no surrounding text."
        )

    if any(
        marker in stripped
        for marker in (
            "{",
            "}",
            "[",
            "]",
            '"loop_detected"',
            '"confidence"',
            '"rationale"',
        )
    ):
        return _loop_analysis_invalid_result(
            "Loop analysis response looked like JSON but was not a single valid JSON object."
        )

    # Branch 3: no JSON at all.
    return {
        "decision": LOOP_ANALYSIS_DECISION_UNKNOWN,
        "confidence": 0.0,
        "rationale": "Loop analysis response did not contain valid JSON.",
    }


def researcher_analysis_command(
    prompt: str,
    profiles: dict[str, Any],
    root: Path | None = None,
) -> tuple[str, list[str]]:
    resolved_root = root or project_root()
    module = load_agent_profiles_module(resolved_root)
    provider_name = module.resolve_provider("researcher", profiles=profiles)
    rendered = module.render_provider_example(
        "researcher",
        task=sanitize_loop_prompt(prompt),
        profiles=profiles,
    )
    try:
        tokens = shlex.split(rendered)
    except ValueError:
        return provider_name, []
    for token in tokens:
        if token in {
            "|",
            "||",
            "&&",
            ";",
            "<",
            ">",
            "<<",
            ">>",
            "1>",
            "2>",
            "&>",
            "2>&1",
        }:
            return provider_name, []
        if re.fullmatch(r"(?:\d+)?(?:>>?|<<?|>&|<&).+", token):
            return provider_name, []
    return provider_name, tokens


def analyze_loop_with_researcher(
    task: dict[str, Any],
    failure_streak: int,
    failure_limit: int,
    profiles: dict[str, Any],
    root: Path | None = None,
) -> dict[str, Any]:
    resolved_root = root or project_root()
    prompt = build_loop_analysis_prompt(
        task=task,
        failure_streak=failure_streak,
        failure_limit=failure_limit,
        root=resolved_root,
    )
    provider_name, command = researcher_analysis_command(
        prompt, profiles, resolved_root
    )
    if not command:
        return {
            "provider": provider_name,
            "decision": LOOP_ANALYSIS_DECISION_UNKNOWN,
            "confidence": 0.0,
            "rationale": "No executable researcher command could be derived from active profile.",
        }

    try:
        result = subprocess.run(
            command,
            cwd=resolved_root,
            text=True,
            capture_output=True,
            check=False,
            timeout=120,
        )
    except FileNotFoundError:
        return {
            "provider": provider_name,
            "decision": LOOP_ANALYSIS_DECISION_UNKNOWN,
            "confidence": 0.0,
            "rationale": f"Researcher command is not available on PATH: {command[0]}",
        }
    except subprocess.TimeoutExpired:
        return {
            "provider": provider_name,
            "decision": LOOP_ANALYSIS_DECISION_UNKNOWN,
            "confidence": 0.0,
            "rationale": "Researcher analysis timed out.",
        }
    except OSError as err:
        return {
            "provider": provider_name,
            "decision": LOOP_ANALYSIS_DECISION_UNKNOWN,
            "confidence": 0.0,
            "rationale": f"Researcher analysis failed to start: {err}",
        }

    combined_output = "\n".join(part for part in (result.stdout, result.stderr) if part)
    parsed = parse_loop_analysis_result(combined_output)
    parsed["provider"] = provider_name
    if result.returncode != 0 and parsed["decision"] in {
        LOOP_ANALYSIS_DECISION_UNKNOWN,
        LOOP_ANALYSIS_DECISION_INVALID,
    }:
        parsed["rationale"] = (
            f"Researcher analysis exited with status {result.returncode}. {parsed['rationale']}"
        )
    return parsed


def write_handoff_file(
    task: dict[str, Any],
    failure_streak: int,
    failure_limit: int,
    exit_code: int,
    loop_analysis: dict[str, Any] | None,
    root: Path | None = None,
) -> Path:
    resolved_root = root or project_root()
    path = handoff_path(task=task, root=resolved_root)
    path.parent.mkdir(parents=True, exist_ok=True)
    lines = [
        "# Takt Circuit Breaker Handoff",
        "",
        f"- generated_at: {now_utc_iso()}",
        f"- status: {TASK_STATUS_BLOCKED}",
        f"- reason: exceeded {failure_limit} consecutive queue failures",
        f"- failure_streak: {failure_streak}",
        f"- last_exit_code: {exit_code}",
        f"- task: {task_display_name(task)}",
        f"- piece: {task.get('piece', '<unknown-piece>')}",
        f"- task_dir: {task.get('task_dir', '<unknown-task-dir>')}",
    ]
    if loop_analysis is not None:
        lines.extend(
            [
                f"- loop_analysis_provider: {loop_analysis.get('provider', 'unknown')}",
                f"- loop_analysis_decision: {loop_analysis.get('decision', LOOP_ANALYSIS_DECISION_UNKNOWN)}",
                f"- loop_analysis_confidence: {loop_analysis.get('confidence', 0.0)}",
                f"- loop_analysis_rationale: {loop_analysis.get('rationale', '')}",
            ]
        )
    lines.extend(
        [
            "",
            "## Next Actions (Human)",
            "",
            "1. Inspect `.takt/last-failure.log` and `.takt/debug-report.md`.",
            "2. Decide whether to re-plan, fix manually, or cancel the task.",
            "3. If resuming automation, update task state from `blocked` to `pending` and reset failure streak.",
        ]
    )
    path.write_text("\n".join(lines) + "\n", encoding="utf-8")
    return path


def update_failure_state_after_run(
    run_exit_code: int,
    target_task: dict[str, Any],
    failure_limit: int,
    profiles: dict[str, Any],
    root: Path | None = None,
) -> tuple[int, bool, Path | None, dict[str, Any] | None]:
    with _tasks_file_lock(root):
        return _update_failure_state_inner(
            run_exit_code, target_task, failure_limit, profiles, root
        )


def _update_failure_state_inner(
    run_exit_code: int,
    target_task: dict[str, Any],
    failure_limit: int,
    profiles: dict[str, Any],
    root: Path | None = None,
) -> tuple[int, bool, Path | None, dict[str, Any] | None]:
    tasks = parse_tasks_file(root)
    task_index = find_task_index(tasks, target_task)
    if task_index is None:
        if run_exit_code == 0:
            return 0, False, None, None
        fallback_task = first_pending_task(tasks)
        if fallback_task is None:
            return 0, False, None, None
        print(
            "warning: could not find the original pending task while recording failure state; "
            f"falling back to first pending task '{task_display_name(fallback_task)}'.",
            file=sys.stderr,
        )
        task_index = tasks.index(fallback_task)

    task = tasks[task_index]
    handoff_file: Path | None = None
    loop_analysis: dict[str, Any] | None = None
    if run_exit_code == 0:
        task[TASK_FAILURE_STREAK_KEY] = "0"
        tasks[task_index] = task
        write_tasks_file(tasks, root)
        return 0, False, None, None

    failure_streak = read_failure_streak(task) + 1
    task[TASK_FAILURE_STREAK_KEY] = str(failure_streak)
    task[TASK_LAST_FAILURE_AT_KEY] = now_utc_iso()
    task[TASK_LAST_FAILURE_EXIT_CODE_KEY] = str(run_exit_code)

    blocked = False
    if failure_streak >= failure_limit:
        loop_analysis = analyze_loop_with_researcher(
            task=task,
            failure_streak=failure_streak,
            failure_limit=failure_limit,
            profiles=profiles,
            root=root,
        )
        task[TASK_LOOP_ANALYSIS_PROVIDER_KEY] = str(
            loop_analysis.get("provider", "unknown")
        )
        task[TASK_LOOP_ANALYSIS_DECISION_KEY] = str(
            loop_analysis.get("decision", LOOP_ANALYSIS_DECISION_UNKNOWN)
        )
        task[TASK_LOOP_ANALYSIS_CONFIDENCE_KEY] = str(
            loop_analysis.get("confidence", 0.0)
        )
        task[TASK_LOOP_ANALYSIS_RATIONALE_KEY] = sanitize_task_field_value(
            loop_analysis.get("rationale", "")
        )
        task[TASK_LOOP_ANALYSIS_AT_KEY] = now_utc_iso()

        blocked = loop_analysis.get("decision") != LOOP_ANALYSIS_DECISION_TRANSIENT
        if blocked:
            task["status"] = TASK_STATUS_BLOCKED
            task[TASK_BLOCKED_AT_KEY] = now_utc_iso()
            if loop_analysis.get("decision") == LOOP_ANALYSIS_DECISION_LOOP:
                task[TASK_BLOCKED_REASON_KEY] = (
                    "Circuit breaker opened after loop analysis confirmed a repeated no-progress failure."
                )
            elif loop_analysis.get("decision") == LOOP_ANALYSIS_DECISION_INVALID:
                task[TASK_BLOCKED_REASON_KEY] = (
                    "Circuit breaker opened because loop analysis returned an invalid JSON-only response at the failure threshold."
                )
            else:
                task[TASK_BLOCKED_REASON_KEY] = (
                    "Circuit breaker opened because loop analysis was inconclusive at the failure threshold."
                )

    tasks[task_index] = task
    write_tasks_file(tasks, root)
    if blocked:
        handoff_file = write_handoff_file(
            task=task,
            failure_streak=failure_streak,
            failure_limit=failure_limit,
            exit_code=run_exit_code,
            loop_analysis=loop_analysis,
            root=root,
        )
    return failure_streak, blocked, handoff_file, loop_analysis


def task_snapshot(
    task: dict[str, Any], root: Path | None = None
) -> dict[str, str] | None:
    snapshot_keys = task_snapshot_keys(root)
    present_keys = [key for key in snapshot_keys if key in task]
    if not present_keys:
        return None

    missing = [key for key in snapshot_keys if key not in task]
    if missing:
        missing_list = ", ".join(missing)
        raise ValueError(
            f"Task '{task_display_name(task)}' has an incomplete agent profile snapshot: "
            f"{missing_list}"
        )

    snapshot: dict[str, str] = {}
    for key in snapshot_keys:
        value = task.get(key)
        if value is None:
            raise ValueError(
                f"Task '{task_display_name(task)}' has null agent snapshot field '{key}'"
            )
        snapshot[key] = str(value)
    return snapshot


def profiles_from_snapshot(
    snapshot: dict[str, str], root: Path | None = None
) -> dict[str, Any]:
    resolved_root = root or project_root()
    module = load_agent_profiles_module(resolved_root)
    current_profiles = load_profiles(resolved_root)
    snapshot_profiles = deepcopy(current_profiles)

    snapshot_mapping: dict[str, str] = {}
    for capability in module.REQUIRED_CAPABILITIES:
        snapshot_mapping[capability] = snapshot[snapshot_key_for_capability(capability)]
    snapshot_mapping[module.TAKT_HOST_PROVIDER_KEY] = snapshot[
        f"agent_{module.TAKT_HOST_PROVIDER_KEY}"
    ]
    snapshot_mapping[module.TAKT_HOST_MODEL_KEY] = snapshot[
        f"agent_{module.TAKT_HOST_MODEL_KEY}"
    ]

    snapshot_profiles["profiles"][QUEUE_SNAPSHOT_PROFILE_NAME] = snapshot_mapping
    snapshot_profiles["active_profile"] = QUEUE_SNAPSHOT_PROFILE_NAME
    module.validate_profiles(snapshot_profiles)
    return snapshot_profiles


def queue_profiles(root: Path | None = None) -> dict[str, Any]:
    resolved_root = root or project_root()
    tasks = parse_tasks_file(resolved_root)
    pending = [task for task in tasks if task.get("status") == "pending"]
    if not pending:
        return load_profiles(resolved_root)

    snapped: list[dict[str, str]] = []
    unsnapped: list[str] = []
    for task in pending:
        snapshot = task_snapshot(task, resolved_root)
        if snapshot is None:
            unsnapped.append(task_display_name(task))
        else:
            snapped.append(snapshot)

    if snapped and unsnapped:
        names = ", ".join(unsnapped)
        raise ValueError(
            "Pending takt queue mixes snapped and unsnapped tasks; "
            f"re-queue or finish these tasks first: {names}"
        )
    if not snapped:
        return load_profiles(resolved_root)

    distinct_snapshots = {tuple(sorted(snapshot.items())) for snapshot in snapped}
    if len(distinct_snapshots) > 1:
        raise ValueError(
            "Pending takt queue contains multiple agent profile snapshots; "
            "run tasks separately or normalize the queue first."
        )
    return profiles_from_snapshot(snapped[0], resolved_root)


def takt_command_for_piece(
    piece: str,
    task: str,
    root: Path | None = None,
    profiles: dict[str, Any] | None = None,
) -> list[str]:
    resolved_root = root or project_root()
    resolved_profiles = (
        profiles if profiles is not None else load_profiles(resolved_root)
    )
    return [
        "takt",
        "--provider",
        host_provider(resolved_root, resolved_profiles),
        "--model",
        host_model(resolved_root, resolved_profiles),
        "--piece",
        piece,
        "--task",
        task,
        "--skip-git",
        "--pipeline",
    ]


def takt_command_for_queue(
    root: Path | None = None, profiles: dict[str, Any] | None = None
) -> list[str]:
    resolved_root = root or project_root()
    resolved_profiles = (
        profiles if profiles is not None else load_profiles(resolved_root)
    )
    return [
        "takt",
        "--provider",
        host_provider(resolved_root, resolved_profiles),
        "--model",
        host_model(resolved_root, resolved_profiles),
        "run",
    ]


def run_piece(piece: str, task: str, root: Path | None = None) -> int:
    resolved_root = root or project_root()
    profiles = load_profiles(resolved_root)
    render_personas(resolved_root, profiles=profiles)
    return subprocess.call(
        takt_command_for_piece(piece, task, resolved_root, profiles),
        cwd=resolved_root,
    )


def run_queue(root: Path | None = None) -> int:
    resolved_root = root or project_root()
    with _tasks_file_lock(resolved_root):
        tasks = parse_tasks_file(resolved_root)
        target_task = first_pending_task(tasks)
    if target_task is None:
        return 0

    failure_limit = circuit_breaker_failure_limit()
    profiles = queue_profiles(resolved_root)
    render_personas(resolved_root, profiles=profiles)
    exit_code = subprocess.call(
        takt_command_for_queue(resolved_root, profiles),
        cwd=resolved_root,
    )
    failure_streak, blocked, handoff_file, loop_analysis = (
        update_failure_state_after_run(
            run_exit_code=exit_code,
            target_task=target_task,
            failure_limit=failure_limit,
            profiles=profiles,
            root=resolved_root,
        )
    )
    if exit_code != 0:
        task_name = task_display_name(target_task)
        if loop_analysis is not None:
            analysis_provider = str(loop_analysis.get("provider", "unknown"))
            analysis_decision = str(
                loop_analysis.get("decision", LOOP_ANALYSIS_DECISION_UNKNOWN)
            )
            try:
                analysis_confidence = float(loop_analysis.get("confidence", 0.0))
            except (TypeError, ValueError):
                analysis_confidence = 0.0
            print(
                f"Loop analysis via {analysis_provider}: "
                f"decision={analysis_decision}, confidence={analysis_confidence:.2f}",
                file=sys.stderr,
            )
        if blocked:
            if (
                loop_analysis is not None
                and loop_analysis.get("decision") == LOOP_ANALYSIS_DECISION_LOOP
            ):
                reason = "after loop detection"
            elif (
                loop_analysis is not None
                and loop_analysis.get("decision") == LOOP_ANALYSIS_DECISION_INVALID
            ):
                reason = "because loop analysis returned an invalid JSON-only response"
            else:
                reason = "because loop analysis was inconclusive"
            print(
                f"Circuit breaker opened for '{task_name}' {reason} "
                f"({failure_streak} consecutive failures).",
                file=sys.stderr,
            )
            if handoff_file is not None:
                relative = handoff_file.relative_to(resolved_root)
                print(
                    f"Generated human handoff: {relative}",
                    file=sys.stderr,
                )
        else:
            print(
                f"takt queue task '{task_name}' failed "
                f"({failure_streak} consecutive failures).",
                file=sys.stderr,
            )
    return exit_code


def add_task(task: str | None = None, root: Path | None = None) -> int:
    resolved_root = root or project_root()
    profiles = load_profiles(resolved_root)
    command = ["takt", "add"]
    if task:
        command.append(task)

    exit_code = subprocess.call(command, cwd=resolved_root)
    if exit_code != 0:
        return exit_code

    append_snapshot_to_last_task(
        active_profile_snapshot(resolved_root, profiles), resolved_root
    )
    return 0


def clean_queue(root: Path | None = None) -> int:
    """Remove non-pending tasks from the queue (blocked and completed)."""
    resolved_root = root or project_root()
    with _tasks_file_lock(resolved_root):
        tasks = parse_tasks_file(resolved_root)
        if not tasks:
            print("Queue is empty. Nothing to clean.")
            return 0

        kept = [task for task in tasks if task.get("status") == TASK_STATUS_PENDING]
        removed_count = len(tasks) - len(kept)
        if removed_count == 0:
            print(f"All {len(tasks)} task(s) are pending. Nothing to clean.")
            return 0

        write_tasks_file(kept, resolved_root)
        print(
            f"Removed {removed_count} non-pending task(s). {len(kept)} pending task(s) remain."
        )
        return 0


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Render takt personas from the active profile and run takt with host overrides."
    )
    subparsers = parser.add_subparsers(dest="command", required=True)

    subparsers.add_parser(
        "render-personas", help="Render runtime personas from the active profile."
    )
    subparsers.add_parser("host-provider", help="Print the active takt host provider.")
    subparsers.add_parser("host-model", help="Print the active takt host model.")
    add_parser = subparsers.add_parser(
        "add-task",
        help="Add a task to the takt queue and snapshot the active profile onto it.",
    )
    add_parser.add_argument("task", nargs="*", help="Task summary")

    piece_parser = subparsers.add_parser(
        "run-piece",
        help="Render personas and run a direct piece with active host overrides.",
    )
    piece_parser.add_argument("piece", help="Piece name")
    piece_parser.add_argument("task", nargs="+", help="Task summary")

    subparsers.add_parser(
        "run-queue",
        help="Render personas and run queued takt tasks with active host overrides.",
    )
    subparsers.add_parser(
        "clean-queue",
        help="Remove non-pending (blocked/completed) tasks from the queue.",
    )
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv if argv is not None else sys.argv[1:])
    try:
        if args.command == "render-personas":
            render_personas()
            return 0
        if args.command == "host-provider":
            print(host_provider())
            return 0
        if args.command == "host-model":
            print(host_model())
            return 0
        if args.command == "add-task":
            joined_task = " ".join(args.task).strip() if args.task else ""
            return add_task(joined_task or None)
        if args.command == "run-piece":
            return run_piece(args.piece, " ".join(args.task))
        if args.command == "run-queue":
            return run_queue()
        if args.command == "clean-queue":
            return clean_queue()
    except ValueError as err:
        print(str(err), file=sys.stderr)
        return 1
    raise ValueError(f"Unknown command: {args.command}")


if __name__ == "__main__":
    raise SystemExit(main())
