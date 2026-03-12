"""track_resolution.py — Branch-aware track directory resolution.

Provides resolve_track_dir() which distinguishes 'current track' (branch-bound)
from 'latest track' (global timestamp fallback for CI/reporting).
"""
from __future__ import annotations

import json
import subprocess
from pathlib import Path

TRACK_ITEMS_DIR = "track/items"
BRANCH_PREFIX = "track/"


def current_git_branch(root: Path) -> str | None:
    """Return the current git branch name, or None if detached/not a repo."""
    try:
        result = subprocess.run(
            ["git", "rev-parse", "--abbrev-ref", "HEAD"],
            capture_output=True, text=True, cwd=str(root),
        )
        if result.returncode != 0:
            return None
        branch = result.stdout.strip()
        return None if branch == "HEAD" else branch  # "HEAD" means detached
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
        if candidate.is_dir() and (candidate / "metadata.json").is_file():
            return candidate
    return None


def _latest_by_timestamp(root: Path) -> Path | None:
    """Legacy fallback: find track with most recent updated_at (excludes archived)."""
    from datetime import UTC, datetime

    track_root = root / "track" / "items"
    if not track_root.is_dir():
        return None

    epoch = datetime.min.replace(tzinfo=UTC)
    best_dir: Path | None = None
    best_ts = epoch

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
        if data.get("status") == "archived":
            continue
        raw = data.get("updated_at", "")
        if not isinstance(raw, str) or not raw.strip():
            continue
        try:
            value = raw.strip()
            if value.endswith("Z"):
                value = value[:-1] + "+00:00"
            parsed = datetime.fromisoformat(value)
            if parsed.tzinfo is None:
                parsed = parsed.replace(tzinfo=UTC)
        except (ValueError, OverflowError):
            continue
        if (parsed, track_dir.name) > (best_ts, best_dir.name if best_dir else ""):
            best_dir = track_dir
            best_ts = parsed

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

    # 2. Branch-based resolution
    branch = git_branch or current_git_branch(root)
    if branch is not None and branch.startswith(BRANCH_PREFIX):
        found = find_track_by_branch(root, branch)
        if found is not None:
            return found, warnings
        warnings.append(f"On branch '{branch}' but no matching track found")

    # 3. Legacy timestamp fallback
    if allow_legacy_timestamp_fallback:
        legacy = _latest_by_timestamp(root)
        if legacy is not None:
            warnings.append(f"Using legacy timestamp fallback: {legacy.name}")
            return legacy, warnings

    return None, warnings


def latest_legacy_track_dir(root: Path | None = None) -> tuple[Path | None, list[str]]:
    """Legacy compatibility wrapper. Returns (track_dir, warnings)."""
    effective_root = root or Path(__file__).resolve().parent.parent
    result = _latest_by_timestamp(effective_root)
    warnings: list[str] = []
    if result is not None:
        warnings.append(f"Resolved via legacy timestamp: {result.name}")
    return result, warnings
