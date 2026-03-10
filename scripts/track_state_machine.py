"""track_state_machine.py — State transition APIs for track metadata.

All mutations go through metadata.json first (SSoT), then recompute
top-level status, then optionally render plan.md.
"""

from __future__ import annotations

import json
import sys
from datetime import UTC, datetime
from pathlib import Path

from track_markdown import render_plan
from track_registry import collect_track_metadata, render_registry
from track_schema import (
    COMMIT_HASH_RE,
    VALID_OVERRIDE_STATUSES,
    TrackTask,
    _as_list,
    effective_track_status,
    parse_metadata_v2,
)


class TransitionError(Exception):
    """Raised when a state transition is invalid."""


# Valid task transitions: (from_status, to_status)
VALID_TRANSITIONS: set[tuple[str, str]] = {
    ("todo", "in_progress"),
    ("todo", "skipped"),
    ("in_progress", "done"),
    ("in_progress", "todo"),
    ("in_progress", "skipped"),
    ("done", "in_progress"),
    ("skipped", "todo"),
}


def _load_metadata(track_dir: Path) -> dict:
    metadata_file = track_dir / "metadata.json"
    return json.loads(metadata_file.read_text(encoding="utf-8"))


def _save_metadata(track_dir: Path, data: dict, *, now: datetime | None = None) -> None:
    if now is None:
        now = datetime.now(UTC)
    data["updated_at"] = now.isoformat()

    # Auto-clear override if all tasks are resolved (override incompatible with done)
    resolved = {"done", "skipped"}
    tasks = _as_list(data.get("tasks"))
    if (
        data.get("status_override")
        and tasks
        and all(isinstance(t, dict) and t.get("status") in resolved for t in tasks)
    ):
        data["status_override"] = None

    # Recompute top-level status from tasks and override
    meta = parse_metadata_v2(data)
    data["status"] = effective_track_status(meta)

    metadata_file = track_dir / "metadata.json"
    metadata_file.write_text(
        json.dumps(data, indent=2, ensure_ascii=False) + "\n", encoding="utf-8"
    )


def _next_task_id(data: dict) -> str:
    """Generate the next sequential task ID."""
    existing = [
        t.get("id", "") for t in _as_list(data.get("tasks")) if isinstance(t, dict)
    ]
    max_num = 0
    for tid in existing:
        if isinstance(tid, str) and tid.startswith("T") and tid[1:].isdigit():
            max_num = max(max_num, int(tid[1:]))
    return f"T{max_num + 1:03d}"


def add_task(
    track_dir: Path,
    description: str,
    *,
    section_id: str | None = None,
    after_task_id: str | None = None,
    now: datetime | None = None,
) -> str:
    """Add a new task to a v2 track. Returns the generated task ID."""
    if not description or not description.strip():
        raise TransitionError("Task description must not be empty")
    data = _load_metadata(track_dir)

    # Ensure tasks is a list and validate entries
    if not isinstance(data.get("tasks"), list):
        data["tasks"] = []
    for t in data["tasks"]:
        if not isinstance(t, dict):
            raise TransitionError(f"Corrupt metadata: non-dict task entry {t!r}")

    task_id = _next_task_id(data)

    # Resolve target section before mutating
    plan_data = data.get("plan", {})
    if not isinstance(plan_data, dict):
        raise TransitionError("Corrupt metadata: 'plan' is not a dict")
    sections = _as_list(plan_data.get("sections"))
    target_section = None
    if section_id is not None:
        for s in sections:
            if not isinstance(s, dict):
                continue
            if s.get("id") == section_id:
                target_section = s
                break
        if target_section is None:
            raise TransitionError(f"Section '{section_id}' not found")
    elif sections:
        # Find first dict section
        for s in sections:
            if isinstance(s, dict):
                target_section = s
                break
        if target_section is None:
            raise TransitionError("No valid sections available to add task to")
    else:
        raise TransitionError("No sections available to add task to")

    new_task = {
        "id": task_id,
        "description": description,
        "status": "todo",
        "commit_hash": None,
    }
    data["tasks"].append(new_task)

    # Ensure task_ids is a list
    if not isinstance(target_section.get("task_ids"), list):
        target_section["task_ids"] = []

    if after_task_id and after_task_id in target_section["task_ids"]:
        idx = target_section["task_ids"].index(after_task_id) + 1
        target_section["task_ids"].insert(idx, task_id)
    else:
        target_section["task_ids"].append(task_id)

    _save_metadata(track_dir, data, now=now)
    return task_id


def transition_task(
    track_dir: Path,
    task_id: str,
    new_status: str,
    *,
    commit_hash: str | None = None,
    now: datetime | None = None,
) -> None:
    """Transition a task to a new status. Raises TransitionError on invalid transition."""
    data = _load_metadata(track_dir)

    # Find task (guard against non-dict/non-list entries)
    task = None
    for t in _as_list(data.get("tasks")):
        if not isinstance(t, dict):
            raise TransitionError(f"Corrupt metadata: non-dict task entry {t!r}")
        if t.get("id") == task_id:
            task = t
            break

    if task is None:
        raise TransitionError(f"Task '{task_id}' not found")

    old_status = task.get("status")
    if old_status is None:
        raise TransitionError(f"Task '{task_id}' has no 'status' field")
    if (old_status, new_status) not in VALID_TRANSITIONS:
        raise TransitionError(
            f"Invalid transition: '{old_status}' -> '{new_status}' for task '{task_id}'"
        )

    # Validate commit_hash type and format before mutation
    if commit_hash is not None and not isinstance(commit_hash, str):
        raise TransitionError(
            f"commit_hash must be a string, got {type(commit_hash).__name__}"
        )
    if commit_hash is not None and not COMMIT_HASH_RE.match(commit_hash):
        raise TransitionError(f"Invalid commit_hash format: '{commit_hash}'")

    task["status"] = new_status

    # Handle commit_hash
    if new_status == "done" and commit_hash is not None:
        task["commit_hash"] = commit_hash
    elif new_status != "done":
        task["commit_hash"] = None

    _save_metadata(track_dir, data, now=now)


def set_track_override(
    track_dir: Path,
    override_status: str | None,
    *,
    reason: str | None = None,
    now: datetime | None = None,
) -> None:
    """Set or clear the track-level status override."""
    data = _load_metadata(track_dir)

    if override_status is None:
        data["status_override"] = None
    else:
        # Validate override status
        if override_status not in VALID_OVERRIDE_STATUSES:
            raise TransitionError(f"Invalid override status: '{override_status}'")

        # Check incompatibility: cannot block/cancel when all resolved (done/skipped)
        resolved = {"done", "skipped"}
        tasks = [t for t in _as_list(data.get("tasks")) if isinstance(t, dict)]
        if tasks and all(t.get("status") in resolved for t in tasks):
            raise TransitionError(
                f"Cannot set override '{override_status}' when all tasks are resolved"
            )
        data["status_override"] = {
            "status": override_status,
            "reason": reason or "",
        }

    _save_metadata(track_dir, data, now=now)


def next_open_task(track_dir: Path) -> TrackTask | None:
    """Return the next task to work on: first in_progress, then first todo. None if all resolved.

    Iterates in canonical plan order (section.task_ids), not raw tasks array order.
    """
    data = _load_metadata(track_dir)
    meta = parse_metadata_v2(data)
    task_map = {t.id: t for t in meta.tasks}

    # Build ordered task list from plan sections
    ordered_ids: list[str] = []
    for section in meta.plan.sections:
        for tid in section.task_ids:
            if tid not in ordered_ids:
                ordered_ids.append(tid)

    # Prioritize in_progress tasks first
    for tid in ordered_ids:
        task = task_map.get(tid)
        if task and task.status == "in_progress":
            return task
    for tid in ordered_ids:
        task = task_map.get(tid)
        if task and task.status == "todo":
            return task
    return None


def task_counts(track_dir: Path) -> dict[str, int]:
    """Return task status counts: {total, todo, in_progress, done}."""
    data = _load_metadata(track_dir)
    meta = parse_metadata_v2(data)
    counts = {"total": 0, "todo": 0, "in_progress": 0, "done": 0, "skipped": 0}
    for task in meta.tasks:
        counts["total"] += 1
        if task.status in counts:
            counts[task.status] += 1
    return counts


def sync_rendered_views(
    root: Path,
    *,
    track_id: str | None = None,
) -> list[Path]:
    """Render plan.md and registry.md from metadata.json for specified or all tracks."""
    changed: list[Path] = []
    track_root = root / "track" / "items"

    if track_id:
        dirs = [track_root / track_id]
    else:
        dirs = (
            sorted(p for p in track_root.iterdir() if p.is_dir())
            if track_root.exists()
            else []
        )

    for track_dir in dirs:
        metadata_file = track_dir / "metadata.json"
        if not metadata_file.exists():
            continue

        data = _load_metadata(track_dir)
        if data.get("schema_version") != 2:
            continue

        meta = parse_metadata_v2(data)
        plan_content = render_plan(meta)
        plan_file = track_dir / "plan.md"
        old_plan = plan_file.read_text(encoding="utf-8") if plan_file.exists() else None
        if old_plan != plan_content:
            plan_file.write_text(plan_content, encoding="utf-8")
            changed.append(plan_file)

    # Render registry.md (always uses all tracks, regardless of track_id filter)
    tracks = collect_track_metadata(root)
    registry_content = render_registry(tracks)
    registry_path = root / "track" / "registry.md"
    registry_path.parent.mkdir(parents=True, exist_ok=True)
    old_content = (
        registry_path.read_text(encoding="utf-8") if registry_path.exists() else None
    )
    if old_content != registry_content:
        registry_path.write_text(registry_content, encoding="utf-8")
        changed.append(registry_path)

    return changed


def main(argv: list[str] | None = None) -> int:
    """CLI entry point for track state machine operations.

    Usage:
        python track_state_machine.py transition <track_dir> <task_id> <status> [--commit-hash <hash>]
        python track_state_machine.py sync-views [--track-id <id>]
    """
    import argparse

    parser = argparse.ArgumentParser(description="Track state machine CLI")
    sub = parser.add_subparsers(dest="command")

    # transition subcommand
    tr = sub.add_parser("transition", help="Transition a task to a new status")
    tr.add_argument(
        "track_dir", help="Path to track directory (e.g. track/items/my-feature)"
    )
    tr.add_argument("task_id", help="Task ID (e.g. T001)")
    tr.add_argument("status", help="New status (todo, in_progress, done, skipped)")
    tr.add_argument("--commit-hash", default=None, help="Commit hash (for done status)")

    # sync-views subcommand
    sv = sub.add_parser(
        "sync-views", help="Render plan.md and registry.md from metadata.json"
    )
    sv.add_argument(
        "--track-id", default=None, help="Sync only this track (default: all)"
    )

    args = parser.parse_args(argv)

    if args.command == "transition":
        track_dir = Path(args.track_dir)
        if not track_dir.is_dir():
            print(f"[ERROR] Track directory not found: {track_dir}", file=sys.stderr)
            return 1
        try:
            transition_task(
                track_dir, args.task_id, args.status, commit_hash=args.commit_hash
            )
        except TransitionError as e:
            print(f"[ERROR] {e}", file=sys.stderr)
            return 1
        print(f"[OK] {args.task_id}: transitioned to {args.status}")
        # Auto-sync rendered views
        root = track_dir.parent.parent.parent  # track/items/<id> -> project root
        changed = sync_rendered_views(root, track_id=track_dir.name)
        for p in changed:
            print(f"[OK] Rendered: {p.relative_to(root)}")
        return 0

    if args.command == "sync-views":
        root = Path(".")
        changed = sync_rendered_views(root, track_id=args.track_id)
        if not changed:
            print("[OK] All views already up to date")
        else:
            for p in changed:
                print(f"[OK] Rendered: {p.relative_to(root)}")
        return 0

    parser.print_help()
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
