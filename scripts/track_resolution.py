"""track_resolution.py — Branch-aware track directory resolution.

Provides resolve_track_dir() which distinguishes 'current track' (branch-bound)
from the repo-wide fallback used on non-track branches. That fallback prefers
materialized active tracks over newer branchless planning-only tracks so
standard branch-per-track work is not displaced by pre-activation planning.
"""
from __future__ import annotations

import json
import subprocess
from datetime import UTC, datetime
from pathlib import Path

try:
    from scripts.track_schema import (
        v3_branch_field_missing,
        v3_branchless_track_invalid,
        v3_non_null_branch_invalid,
    )
except ImportError:  # pragma: no cover - script execution path
    from track_schema import (
        v3_branch_field_missing,
        v3_branchless_track_invalid,
        v3_non_null_branch_invalid,
    )

TRACK_ITEMS_DIR = "track/items"
BRANCH_PREFIX = "track/"
_STATUS_ARCHIVED = "archived"
_STATUS_DONE = "done"
_STATUS_PLANNED = "planned"


def current_git_branch(root: Path) -> str | None:
    """Return the current git branch name.

    Returns:
        Branch name string for normal branches.
        ``"HEAD"`` sentinel for detached HEAD (distinct from ``None``).
        ``None`` when not inside a git repository or git is unavailable.
    """
    try:
        result = subprocess.run(
            ["git", "rev-parse", "--abbrev-ref", "HEAD"],
            capture_output=True, text=True, cwd=str(root),
        )
        if result.returncode != 0:
            return None
        branch = result.stdout.strip()
        return branch  # "HEAD" for detached, actual name otherwise
    except (OSError, FileNotFoundError):
        return None


def find_track_by_branch(root: Path, branch: str) -> Path | None:
    """Find a track directory whose metadata.json has matching branch field."""
    track_root = root / "track" / "items"
    if not track_root.is_dir():
        return None
    for track_dir in sorted(track_root.iterdir()):
        if not track_dir.is_dir():
            continue
        metadata_file = track_dir / "metadata.json"
        if not metadata_file.is_file():
            continue
        try:
            data = json.loads(metadata_file.read_text(encoding="utf-8"))
        except (json.JSONDecodeError, OSError):
            continue
        if data.get("branch") == branch:
            return track_dir
    # Fallback: try matching branch convention track/<id> to directory name
    if branch.startswith(BRANCH_PREFIX):
        track_id = branch[len(BRANCH_PREFIX):]
        candidate = track_root / track_id
        metadata_file = candidate / "metadata.json"
        if candidate.is_dir() and metadata_file.is_file():
            data = _load_track_metadata(metadata_file)
            if (
                data is not None
                and data.get("branch") is None
                and data.get("schema_version") != 3
                and data.get("status") != _STATUS_ARCHIVED
            ):
                return candidate
    return None


def _parse_updated_at(raw: object) -> datetime:
    """Parse updated_at, returning UTC epoch on invalid input."""
    epoch = datetime.min.replace(tzinfo=UTC)
    if not isinstance(raw, str) or not raw.strip():
        return epoch
    try:
        value = raw.strip()
        if value.endswith("Z"):
            value = value[:-1] + "+00:00"
        parsed = datetime.fromisoformat(value)
        if parsed.tzinfo is None:
            parsed = parsed.replace(tzinfo=UTC)
    except (ValueError, OverflowError):
        return epoch
    try:
        return parsed.astimezone(UTC)
    except OverflowError:
        return epoch


def _load_track_metadata(metadata_file: Path) -> dict | None:
    try:
        data = json.loads(metadata_file.read_text(encoding="utf-8"))
    except (json.JSONDecodeError, OSError):
        return None
    if not isinstance(data, dict):
        return None
    if (
        v3_branch_field_missing(data)
        or v3_branchless_track_invalid(data)
        or v3_non_null_branch_invalid(data)
    ):
        return None
    return data


def _track_priority(data: dict) -> int | None:
    status = data.get("status")
    if status == _STATUS_ARCHIVED:
        return None

    schema_version = data.get("schema_version")
    raw_branch = data.get("branch")
    branch = raw_branch.strip() if isinstance(raw_branch, str) else ""
    has_branch = bool(branch)

    if has_branch and status != _STATUS_DONE:
        return 2
    if not has_branch and schema_version != 3 and status not in {_STATUS_DONE, _STATUS_ARCHIVED}:
        return 2
    if not has_branch and status == _STATUS_PLANNED:
        return 1
    return 0


def _latest_by_priority(root: Path) -> Path | None:
    """Fallback selector for non-track branches."""
    track_root = root / "track" / "items"
    if not track_root.is_dir():
        return None

    best_dir: Path | None = None
    best_rank = (-1, datetime.min.replace(tzinfo=UTC), "")

    for track_dir in sorted(track_root.iterdir()):
        if not track_dir.is_dir():
            continue
        metadata_file = track_dir / "metadata.json"
        if not metadata_file.is_file():
            continue
        data = _load_track_metadata(metadata_file)
        if data is None:
            continue
        priority = _track_priority(data)
        if priority is None:
            continue
        rank = (priority, _parse_updated_at(data.get("updated_at")), track_dir.name)
        if rank > best_rank:
            best_dir = track_dir
            best_rank = rank

    return best_dir


def resolve_track_dir(
    root: Path,
    *,
    track_id: str | None = None,
    git_branch: str | None = None,
    allow_legacy_timestamp_fallback: bool = False,
) -> tuple[Path | None, list[str]]:
    """Resolve track directory with branch-aware logic.

    Resolution order:
    1. Explicit track_id if provided
    2. Branch-based lookup (git_branch or auto-detected current branch)
    3. Legacy timestamp fallback (only if allow_legacy_timestamp_fallback=True)

    Returns (track_dir, warnings).
    """
    warnings: list[str] = []
    track_root = root / "track" / "items"

    # 1. Explicit track_id
    if track_id is not None:
        candidate = track_root / track_id
        if candidate.is_dir() and (candidate / "metadata.json").is_file():
            return candidate, warnings
        warnings.append(f"Track '{track_id}' not found")
        return None, warnings

    # 2. Branch-based resolution (skip "HEAD" sentinel — detached HEAD)
    branch = git_branch or current_git_branch(root)
    if branch is not None and branch != "HEAD" and branch.startswith(BRANCH_PREFIX):
        found = find_track_by_branch(root, branch)
        if found is not None:
            return found, warnings
        warnings.append(f"On branch '{branch}' but no matching track found")

    # 3. Latest-track fallback for non-track branches
    if allow_legacy_timestamp_fallback:
        legacy = _latest_by_priority(root)
        if legacy is not None:
            warnings.append(f"Using latest track fallback: {legacy.name}")
            return legacy, warnings

    return None, warnings


def latest_legacy_track_dir(root: Path | None = None) -> tuple[Path | None, list[str]]:
    """Legacy compatibility wrapper. Returns (track_dir, warnings)."""
    effective_root = root or Path(__file__).resolve().parent.parent
    result = _latest_by_priority(effective_root)
    warnings: list[str] = []
    if result is not None:
        warnings.append(f"Resolved latest track fallback: {result.name}")
    return result, warnings
