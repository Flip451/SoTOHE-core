"""track_registry.py — Render registry.md from track metadata.json files.

registry.md is a read-only view generated from metadata.json (SSoT).
The output is deterministic: same metadata → same registry.md content.
"""

from __future__ import annotations

import json
from pathlib import Path

from track_schema import TrackMetadataV2, parse_metadata_v2

# Status values that belong in the Active Tracks table
_ACTIVE_STATUSES = {"planned", "in_progress", "blocked", "cancelled"}
# Status values that belong in the Completed Tracks table
_DONE_STATUSES = {"done"}
# Status values that belong in the Archived Tracks table
_ARCHIVED_STATUSES = {"archived"}


def _next_command_for_status(status: str) -> str:
    """Suggest the next /track:* command based on track status."""
    if status == "planned":
        return "`/track:implement`"
    if status == "in_progress":
        return "`/track:full-cycle <task>`"
    if status == "blocked":
        return "`/track:status`"
    if status == "cancelled":
        return "`/track:plan <feature>`"
    if status == "archived":
        return "`/track:plan <feature>`"
    return "`/track:status`"


def _format_date(iso_timestamp: str) -> str:
    """Extract YYYY-MM-DD from an ISO timestamp."""
    return iso_timestamp[:10] if len(iso_timestamp) >= 10 else iso_timestamp


def collect_track_metadata(root: Path) -> list[TrackMetadataV2]:
    """Collect and parse all v2 track metadata, sorted by updated_at descending."""
    track_root = root / "track" / "items"
    if not track_root.is_dir():
        return []

    results: list[TrackMetadataV2] = []
    for track_dir in sorted(track_root.iterdir()):
        if not track_dir.is_dir():
            continue
        metadata_file = track_dir / "metadata.json"
        if not metadata_file.exists():
            continue
        try:
            data = json.loads(metadata_file.read_text(encoding="utf-8"))
        except (json.JSONDecodeError, OSError):
            continue
        if not isinstance(data, dict) or data.get("schema_version") != 2:
            continue
        results.append(parse_metadata_v2(data))

    # Sort by updated_at descending (most recently updated first)
    results.sort(key=lambda m: m.updated_at, reverse=True)
    return results


def render_registry(tracks: list[TrackMetadataV2]) -> str:
    """Render registry.md content from a list of track metadata.

    Output is deterministic: same input → same output.
    """
    active = [t for t in tracks if t.status in _ACTIVE_STATUSES]
    completed = [t for t in tracks if t.status in _DONE_STATUSES]
    archived = [t for t in tracks if t.status in _ARCHIVED_STATUSES]

    lines: list[str] = []

    # Header
    lines.append("# Track Registry")
    lines.append("")
    lines.append("> This file lists all tracks and their current status.")
    lines.append("> Auto-updated by `/track:plan` (on approval) and `/track:commit`.")
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
            f"- Next recommended command: {_next_command_for_status(latest.status)}"
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
                f"| {t.id} | {t.status} | {_next_command_for_status(t.status)} | {_format_date(t.updated_at)} |"
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
    lines.append("Use `/track:plan <feature>` to start a new feature or bugfix track.")
    lines.append("")

    return "\n".join(lines)


def write_registry(root: Path) -> Path:
    """Collect metadata, render registry.md, and write it. Returns the path."""
    tracks = collect_track_metadata(root)
    content = render_registry(tracks)
    registry_path = root / "track" / "registry.md"
    registry_path.parent.mkdir(parents=True, exist_ok=True)
    registry_path.write_text(content, encoding="utf-8")
    return registry_path
