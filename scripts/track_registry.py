"""track_registry.py — Render registry.md from track metadata.json files.

registry.md is a read-only view generated from metadata.json (SSoT).
The output is deterministic: same metadata → same registry.md content.
"""

from __future__ import annotations

import json
from pathlib import Path

try:
    from scripts.track_schema import (
        TrackMetadataV2,
        parse_metadata_v2,
        v3_branch_field_missing,
        v3_branchless_track_invalid,
        v3_non_null_branch_invalid,
    )
except ImportError:  # pragma: no cover - script execution path
    from track_schema import (
        TrackMetadataV2,
        parse_metadata_v2,
        v3_branch_field_missing,
        v3_branchless_track_invalid,
        v3_non_null_branch_invalid,
    )

# Status values that belong in the Active Tracks table
_ACTIVE_STATUSES = {"planned", "in_progress", "blocked", "cancelled"}
# Status values that belong in the Completed Tracks table
_DONE_STATUSES = {"done"}
# Status values that belong in the Archived Tracks table
_ARCHIVED_STATUSES = {"archived"}


def _next_command_for_track(track: TrackMetadataV2) -> str:
    """Suggest the next /track:* command based on track status."""
    if (
        track.schema_version == 3
        and track.status == "planned"
        and track.branch is None
    ):
        return f"`/track:activate {track.id}`"
    if track.status == "planned":
        return "`/track:implement`"
    if track.status == "in_progress":
        return "`/track:implement`"
    if track.status == "blocked":
        return "`/track:status`"
    if track.status == "cancelled":
        return "`/track:plan <feature>`"
    if track.status == "archived":
        return "`/track:plan <feature>`"
    return "`/track:status`"


def _format_date(iso_timestamp: str) -> str:
    """Extract YYYY-MM-DD from an ISO timestamp."""
    return iso_timestamp[:10] if len(iso_timestamp) >= 10 else iso_timestamp


def _is_plan_only_track(track: TrackMetadataV2) -> bool:
    return track.schema_version == 3 and track.status == "planned" and track.branch is None


def collect_track_metadata(root: Path) -> list[TrackMetadataV2]:
    """Collect and parse all v2 track metadata, sorted by updated_at descending."""
    try:
        from scripts.track_schema import all_track_directories
    except ImportError:  # pragma: no cover - script execution path
        from track_schema import all_track_directories

    results: list[TrackMetadataV2] = []
    for track_dir in all_track_directories(root):
        metadata_file = track_dir / "metadata.json"
        if not metadata_file.exists():
            continue
        try:
            data = json.loads(metadata_file.read_text(encoding="utf-8"))
        except (json.JSONDecodeError, OSError):
            continue
        if not isinstance(data, dict) or data.get("schema_version") not in (2, 3):
            continue
        if v3_branch_field_missing(data):
            raise ValueError(
                f"Missing required field 'branch' in {metadata_file.relative_to(root).as_posix()}"
            )
        if v3_branchless_track_invalid(data):
            raise ValueError(
                "Illegal branchless v3 track in "
                f"{metadata_file.relative_to(root).as_posix()}: "
                "branch=null is only allowed for planning-only tracks"
            )
        if v3_non_null_branch_invalid(data):
            raise ValueError(
                f"Invalid v3 branch value in {metadata_file.relative_to(root).as_posix()}"
            )
        results.append(parse_metadata_v2(data))

    # Sort by updated_at descending (most recently updated first)
    results.sort(key=lambda m: m.updated_at, reverse=True)
    return results


def render_registry(tracks: list[TrackMetadataV2]) -> str:
    """Render registry.md content from a list of track metadata.

    Output is deterministic: same input → same output.
    """
    active = [t for t in tracks if t.status in _ACTIVE_STATUSES]
    active.sort(key=lambda t: t.updated_at, reverse=True)
    active.sort(key=_is_plan_only_track)
    completed = [t for t in tracks if t.status in _DONE_STATUSES]
    archived = [t for t in tracks if t.status in _ARCHIVED_STATUSES]

    lines: list[str] = []

    # Header
    lines.append("# Track Registry")
    lines.append("")
    lines.append("> This file lists all tracks and their current status.")
    lines.append("> Auto-updated by `/track:plan`, `/track:plan-only`, `/track:activate`, and `/track:commit`.")
    lines.append(
        "> `/track:status` uses this file as an entry point to summarize progress."
    )
    lines.append(
        "> Each track is expected to have `spec.md` / `plan.md` / `metadata.json` / `verification.md`."
    )
    lines.append("")

    # Current Focus
    lines.append("## Current Focus")
    lines.append("")
    if active:
        latest = active[0]
        lines.append(f"- Latest active track: `{latest.id}`")
        lines.append(
            f"- Next recommended command: {_next_command_for_track(latest)}"
        )
        lines.append(f"- Last updated: `{_format_date(latest.updated_at)}`")
    else:
        lines.append("- Latest active track: `None yet`")
        lines.append("- Next recommended command: `/track:plan <feature>`")
        # Use the most recently updated track date even when no active tracks remain.
        all_sorted = sorted(tracks, key=lambda m: m.updated_at, reverse=True)
        if all_sorted:
            lines.append(
                f"- Last updated: `{_format_date(all_sorted[0].updated_at)}`"
            )
        else:
            lines.append("- Last updated: `YYYY-MM-DD`")
    lines.append("")

    # Active Tracks
    lines.append("## Active Tracks")
    lines.append("")
    lines.append("| Track | Status | Next | Updated |")
    lines.append("|------|--------|------|---------|")
    if active:
        for t in active:
            lines.append(
                f"| {t.id} | {t.status} | {_next_command_for_track(t)} | {_format_date(t.updated_at)} |"
            )
    else:
        lines.append("| _No active tracks yet_ | - | `/track:plan <feature>` | - |")
    lines.append("")

    # Completed Tracks
    lines.append("## Completed Tracks")
    lines.append("")
    lines.append("| Track | Result | Updated |")
    lines.append("|------|--------|---------|")
    if completed:
        for t in completed:
            lines.append(f"| {t.id} | Done | {_format_date(t.updated_at)} |")
    else:
        lines.append("| _No completed tracks yet_ | - | - |")
    lines.append("")

    # Archived Tracks
    lines.append("## Archived Tracks")
    lines.append("")
    lines.append("| Track | Result | Archived |")
    lines.append("|------|--------|----------|")
    if archived:
        for t in archived:
            lines.append(f"| {t.id} | Archived | {_format_date(t.updated_at)} |")
    else:
        lines.append("| _No archived tracks yet_ | - | - |")
    lines.append("")

    # Footer
    lines.append("---")
    lines.append("")
    lines.append(
        "Use `/track:plan <feature>` for the standard lane or `/track:plan-only <feature>` when planning should land before activation."
    )
    lines.append("")

    return "\n".join(lines)


def write_registry(root: Path) -> Path:
    """Collect metadata, render registry.md, and write it. Returns the path."""
    from atomic_write import atomic_write_file

    tracks = collect_track_metadata(root)
    content = render_registry(tracks)
    registry_path = root / "track" / "registry.md"
    registry_path.parent.mkdir(parents=True, exist_ok=True)
    atomic_write_file(registry_path, content)
    return registry_path
