"""track_markdown.py — Legacy plan.md parser and plan.md renderer.

The legacy parser normalizes AI output variations in checkbox markers.
The renderer generates plan.md as a read-only view from metadata.json (SSoT).
"""

from __future__ import annotations

import re
from dataclasses import dataclass

from track_schema import STATE_TO_MARKER, TrackMetadataV2, TrackTask

# -- Legacy checkbox parser --------------------------------------------------

TASK_PREFIX_RE = re.compile(r"^\s*(?:[-*]|\d+\.)\s+\[")
TASK_CANDIDATE_RE = re.compile(
    r"^\s*(?:[-*]|\d+\.)\s+\[(?P<raw_state>[^\]]*)\](?P<body_suffix>.*)$"
)

# Normalize whitespace and case variations in checkbox state.
# Keys are lowercased raw_state content (between brackets).
STATE_MAP: dict[str, str] = {
    " ": "todo",
    "  ": "todo",
    "x": "done",
    "x ": "done",
    " x": "done",
    " x ": "done",
    "~": "in_progress",
    "~ ": "in_progress",
    " ~": "in_progress",
    " ~ ": "in_progress",
    "-": "skipped",
    "- ": "skipped",
    " -": "skipped",
    " - ": "skipped",
}


@dataclass(frozen=True)
class PlanSummary:
    total_tasks: int
    todo_count: int
    in_progress_count: int
    done_count: int
    skipped_count: int
    invalid_lines: list[tuple[int, str, str]]
    aggregate_status: str | None  # planned | in_progress | done | None (on invalid)


def summarize_plan(plan_text: str) -> PlanSummary:
    """Parse a legacy plan.md and return task counts and aggregate status."""
    todo = in_progress = done = skipped = 0
    invalid: list[tuple[int, str, str]] = []

    for line_no, line in enumerate(plan_text.splitlines(), start=1):
        # Detect lines that look like task checkboxes but don't fully match
        if TASK_PREFIX_RE.search(line) and not TASK_CANDIDATE_RE.match(line):
            invalid.append((line_no, line, "malformed task checkbox"))
            continue

        match = TASK_CANDIDATE_RE.match(line)
        if match is None:
            continue

        raw_state = match.group("raw_state").lower()
        body = match.group("body_suffix").strip()
        if not body:
            invalid.append((line_no, line, "missing task body"))
            continue

        normalized = STATE_MAP.get(raw_state)
        if normalized is None:
            invalid.append(
                (
                    line_no,
                    line,
                    f"unsupported checkbox state '{match.group('raw_state')}'",
                )
            )
            continue

        if normalized == "todo":
            todo += 1
        elif normalized == "in_progress":
            in_progress += 1
        elif normalized == "skipped":
            skipped += 1
        else:
            done += 1

    total = todo + in_progress + done + skipped
    resolved = done + skipped
    if invalid:
        aggregate: str | None = None
    elif total == 0:
        aggregate = "planned"
    elif in_progress > 0:
        aggregate = "in_progress"
    elif resolved == total:
        aggregate = "done"
    elif resolved > 0 and todo > 0:
        aggregate = "in_progress"
    else:
        aggregate = "planned"

    return PlanSummary(total, todo, in_progress, done, skipped, invalid, aggregate)


# -- Plan renderer -----------------------------------------------------------


def render_plan(meta: TrackMetadataV2) -> str:
    """Render plan.md as a read-only view from metadata.json (SSoT).

    Output is deterministic: same metadata → same plan.md content.
    """
    lines: list[str] = []

    # Generated header
    lines.append("<!-- Generated from metadata.json — DO NOT EDIT DIRECTLY -->")
    lines.append(f"# {meta.title}")
    lines.append("")

    # Summary
    if meta.plan.summary:
        for s in meta.plan.summary:
            lines.append(str(s))
        lines.append("")

    # Build task lookup (skip non-hashable ids)
    task_map: dict[str, TrackTask] = {}
    for t in meta.tasks:
        if isinstance(t.id, str):
            task_map[t.id] = t

    # Sections
    for section in meta.plan.sections:
        lines.append(f"## {section.title}")
        lines.append("")

        if section.description:
            for desc in section.description:
                lines.append(str(desc))
            lines.append("")

        for tid in section.task_ids:
            if not isinstance(tid, str):
                continue
            task = task_map.get(tid)
            if task is None:
                continue
            marker = STATE_TO_MARKER.get(task.status, "?")
            suffix = (
                f" {task.commit_hash}"
                if task.status == "done" and task.commit_hash
                else ""
            )
            lines.append(f"- [{marker}] {task.description}{suffix}")

        lines.append("")

    return "\n".join(lines)
