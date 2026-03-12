"""track_schema.py — metadata.json SSoT data model, parsing, and validation.

metadata.json is the Single Source of Truth for track status.
plan.md and registry.md are read-only views rendered from this data.
"""

from __future__ import annotations

import re
from dataclasses import dataclass
from typing import Literal

TaskStatus = Literal["todo", "in_progress", "done", "skipped"]
TrackStatus = Literal[
    "planned", "in_progress", "done", "blocked", "cancelled", "archived"
]
OverrideStatus = Literal["blocked", "cancelled"]

VALID_TASK_STATUSES: set[str] = {"todo", "in_progress", "done", "skipped"}
VALID_TRACK_STATUSES: set[str] = {
    "planned",
    "in_progress",
    "done",
    "blocked",
    "cancelled",
    "archived",
}
VALID_OVERRIDE_STATUSES: set[str] = {"blocked", "cancelled"}

STATE_TO_MARKER: dict[str, str] = {
    "todo": " ",
    "in_progress": "~",
    "done": "x",
    "skipped": "-",
}
LEGACY_STATE_MAP: dict[str, str] = {
    " ": "todo",
    "~": "in_progress",
    "x": "done",
    "X": "done",
    "-": "skipped",
}
COMMIT_HASH_RE = re.compile(r"^[0-9a-f]{7,40}$")

TRACK_ITEMS_DIR = "track/items"

# Reserved words that must not appear as hyphen-delimited segments in track IDs.
# The guard hook's broad "git" substring match blocks Bash commands whose arguments
# contain these words, causing false positives for track operations.
# Checked per-segment (split on '-') so IDs like "legit-cleanup" are allowed.
RESERVED_ID_SEGMENTS: list[str] = ["git"]

REQUIRED_V2_FIELDS = [
    "schema_version",
    "id",
    "title",
    "status",
    "created_at",
    "updated_at",
    "tasks",
    "plan",
]

REQUIRED_V3_FIELDS = REQUIRED_V2_FIELDS + ["branch"]

BRANCH_PREFIX = "track/"


def _as_list(value: object) -> list:
    """Coerce a value to list. Returns value if already a list, else empty list."""
    return value if isinstance(value, list) else []


@dataclass(frozen=True)
class TrackTask:
    id: str
    description: str
    status: str  # TaskStatus
    commit_hash: str | None


@dataclass(frozen=True)
class PlanSection:
    id: str
    title: str
    description: list[str]
    task_ids: list[str]


@dataclass(frozen=True)
class PlanView:
    summary: list[str]
    sections: list[PlanSection]


@dataclass(frozen=True)
class TrackStatusOverride:
    status: str  # OverrideStatus
    reason: str


@dataclass(frozen=True)
class TrackMetadataV2:
    schema_version: int
    id: str
    branch: str | None
    title: str
    status: str  # TrackStatus
    created_at: str
    updated_at: str
    tasks: list[TrackTask]
    plan: PlanView
    status_override: TrackStatusOverride | None


def parse_metadata_v2(data: dict) -> TrackMetadataV2:
    """Parse a dict into TrackMetadataV2. Assumes basic structure is present."""
    tasks = [
        TrackTask(
            id=t.get("id", ""),
            description=t.get("description", ""),
            status=t.get("status", ""),
            commit_hash=t.get("commit_hash"),
        )
        for t in _as_list(data.get("tasks"))
        if isinstance(t, dict)
    ]

    plan_data = data.get("plan", {})
    if not isinstance(plan_data, dict):
        plan_data = {}
    sections = [
        PlanSection(
            id=s.get("id", ""),
            title=s.get("title", ""),
            description=_as_list(s.get("description")),
            task_ids=_as_list(s.get("task_ids")),
        )
        for s in _as_list(plan_data.get("sections"))
        if isinstance(s, dict)
    ]
    plan = PlanView(
        summary=_as_list(plan_data.get("summary")),
        sections=sections,
    )

    override = None
    o = data.get("status_override")
    if isinstance(o, dict):
        override = TrackStatusOverride(
            status=o.get("status", ""), reason=o.get("reason", "")
        )

    return TrackMetadataV2(
        schema_version=data.get("schema_version", 2),
        id=data.get("id", ""),
        branch=data.get("branch"),
        title=data.get("title", ""),
        status=data.get("status", ""),
        created_at=data.get("created_at", ""),
        updated_at=data.get("updated_at", ""),
        tasks=tasks,
        plan=plan,
        status_override=override,
    )


def effective_track_status(meta: TrackMetadataV2) -> str:
    """Derive track status from tasks and override. metadata.json is SSoT."""
    if meta.status_override is not None:
        return meta.status_override.status

    if not meta.tasks:
        return "planned"

    statuses = [t.status for t in meta.tasks]
    resolved = {"done", "skipped"}
    if all(s == "todo" for s in statuses):
        return "planned"
    if all(s in resolved for s in statuses):
        return "done"
    return "in_progress"


def validate_metadata_v2(data: dict, *, track_dir_name: str) -> list[str]:
    """Validate a v2 metadata.json dict. Returns list of error strings (empty = valid)."""
    errors: list[str] = []

    # Required fields
    for field_name in REQUIRED_V2_FIELDS:
        if field_name not in data:
            errors.append(f"Missing required field '{field_name}'")
    if errors:
        return errors  # short-circuit on missing structure

    # Top-level field type and non-empty checks
    for str_field in ("id", "title", "status", "created_at", "updated_at"):
        val = data[str_field]
        if not isinstance(val, str):
            errors.append(f"'{str_field}' must be a string, got {type(val).__name__}")
        elif not val.strip():
            errors.append(f"'{str_field}' must not be empty")
    if errors:
        return errors  # short-circuit on type/empty mismatches

    # id-directory match
    if data["id"] != track_dir_name:
        errors.append(
            f"metadata id '{data['id']}' does not match directory '{track_dir_name}'"
        )

    # Reserved segment check — guard hook blocks Bash commands containing these words
    track_id_segments = set(data["id"].lower().split("-"))
    for word in RESERVED_ID_SEGMENTS:
        if word in track_id_segments:
            errors.append(
                f"Track id '{data['id']}' contains reserved segment '{word}'. "
                f"This causes false positives in the guard hook. "
                f"Use an alternative (e.g. 'vcs' instead of 'git')."
            )

    # schema_version
    sv = data.get("schema_version")
    if sv not in (2, 3):
        errors.append(f"Expected schema_version=2 or 3, got {sv}")

    # branch validation (v3 requires branch for non-archived tracks)
    branch = data.get("branch")
    if sv == 3:
        if branch is not None:
            if not isinstance(branch, str) or not branch.strip():
                errors.append("'branch' must be a non-empty string for v3")
            elif not branch.startswith(BRANCH_PREFIX):
                errors.append(
                    f"'branch' must start with '{BRANCH_PREFIX}', got '{branch}'"
                )
        elif data["status"] != "archived":
            errors.append(
                "'branch' is required for v3 tracks with non-archived status"
            )

    # status value check
    if data["status"] not in VALID_TRACK_STATUSES:
        errors.append(f"Invalid track status '{data['status']}'")

    # Task validation
    raw_tasks = data.get("tasks")
    if raw_tasks is not None and not isinstance(raw_tasks, list):
        errors.append(f"'tasks' field is not a list: {type(raw_tasks).__name__}")
    task_ids: list[str] = []
    for task in _as_list(raw_tasks):
        if not isinstance(task, dict):
            errors.append(f"Task entry is not a dict: {task!r}")
            continue

        tid = task.get("id", "")
        if not isinstance(tid, str):
            errors.append(f"Task has non-string id: {tid!r}")
            tid = repr(tid)
        elif not tid.strip():
            errors.append("Task has empty id")
        task_ids.append(tid)

        # Non-empty description
        tdesc = task.get("description", "")
        if isinstance(tdesc, str) and not tdesc.strip():
            errors.append(f"Task '{tid}' has empty description")

        # Valid task status
        ts = task.get("status")
        if not isinstance(ts, str) or ts not in VALID_TASK_STATUSES:
            errors.append(f"Task '{tid}' has invalid status '{task.get('status')}'")

        # commit_hash only on done
        ch = task.get("commit_hash")
        if ch is not None and task.get("status") != "done":
            errors.append(
                f"Task '{tid}' has commit_hash but status is '{task.get('status')}' (must be 'done')"
            )

        # commit_hash type and format
        if ch is not None:
            if not isinstance(ch, str):
                errors.append(f"Task '{tid}' has non-string commit_hash: {ch!r}")
            elif not COMMIT_HASH_RE.match(ch):
                errors.append(f"Task '{tid}' has invalid commit_hash format: '{ch}'")

    # Duplicate task IDs
    seen_ids: set[str] = set()
    for tid in task_ids:
        if tid in seen_ids:
            errors.append(f"Duplicate task id '{tid}'")
        seen_ids.add(tid)

    # Section task reference validation — count references per task
    ref_count: dict[str, int] = {}
    plan_data = data.get("plan", {})
    if not isinstance(plan_data, dict):
        errors.append(f"'plan' field is not a dict: {plan_data!r}")
        plan_data = {}
    raw_sections = plan_data.get("sections")
    if raw_sections is not None and not isinstance(raw_sections, list):
        errors.append(
            f"'plan.sections' field is not a list: {type(raw_sections).__name__}"
        )
    for section in _as_list(raw_sections):
        if not isinstance(section, dict):
            errors.append(f"Section entry is not a dict: {section!r}")
            continue
        sid = section.get("id", "")
        if isinstance(sid, str) and not sid.strip():
            errors.append("Section has empty id")
        stitle = section.get("title", "")
        if isinstance(stitle, str) and not stitle.strip():
            errors.append(f"Section '{sid}' has empty title")
        for ref_id in _as_list(section.get("task_ids")):
            if not isinstance(ref_id, str):
                errors.append(
                    f"Section '{section.get('id', '?')}' has non-string task_id: {ref_id!r}"
                )
                continue
            if ref_id not in seen_ids:
                errors.append(
                    f"Section '{section.get('id', '?')}' references unknown task '{ref_id}'"
                )
            ref_count[ref_id] = ref_count.get(ref_id, 0) + 1

    # Every task must be referenced exactly once
    for tid in task_ids:
        count = ref_count.get(tid, 0)
        if count == 0:
            errors.append(f"Task '{tid}' is not referenced by any section")
        elif count > 1:
            errors.append(f"Task '{tid}' is referenced more than once ({count} times)")

    # Status override validation
    override = data.get("status_override")
    if override is not None:
        if not isinstance(override, dict):
            errors.append(f"status_override is not a dict: {override!r}")
            override = None  # prevent further .get() calls
        else:
            if override.get("status") not in VALID_OVERRIDE_STATUSES:
                errors.append(f"Invalid override status '{override.get('status')}'")

    # Derive effective status and compare
    meta = parse_metadata_v2(data)
    derived = effective_track_status(meta)

    # Override incompatibility: blocked/cancelled on all-resolved (done/skipped)
    if override is not None and isinstance(override, dict):
        task_statuses = [
            t.get("status") for t in _as_list(data.get("tasks")) if isinstance(t, dict)
        ]
        resolved = {"done", "skipped"}
        if task_statuses and all(s in resolved for s in task_statuses):
            errors.append(
                f"status_override '{override.get('status')}' is incompatible with all tasks resolved"
            )

    # Status drift check
    # archived is a manual post-done status; derived will be "done" but status is "archived"
    if data["status"] == "archived":
        if derived != "done":
            errors.append(
                f"Status drift: archived track must have all tasks resolved (done/skipped), but derived='{derived}'"
            )
    elif data["status"] != derived:
        errors.append(
            f"Status drift: metadata.status='{data['status']}' but derived='{derived}'"
        )

    return errors
