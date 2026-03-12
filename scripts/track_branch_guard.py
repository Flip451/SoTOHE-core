"""track_branch_guard.py — Branch guard for track mutation operations.

Compares the current git branch with the track's metadata.json ``branch``
field and rejects mismatches.  All mutation paths (transition, add_task,
set_override, commit) should call ``verify_track_branch()`` before writing.

Skip policy:
  - branch is None in metadata.json → skip (legacy / planning phase)
  - detached HEAD (``"HEAD"`` sentinel) → reject
  - ``--skip-branch-check`` / ``skip_branch_check=True`` → skip
  - ``now`` parameter set (test determinism) → skip in Python path
"""

from __future__ import annotations

import json
from pathlib import Path


class BranchGuardError(Exception):
    """Raised when the current git branch does not match the track's expected branch."""


def verify_track_branch(
    track_dir: Path,
    *,
    current_branch: str | None,
    skip_branch_check: bool = False,
) -> None:
    """Verify that the current git branch matches the track's metadata.json branch.

    Args:
        track_dir: Path to the track directory containing metadata.json.
        current_branch: Current git branch name (``"HEAD"`` for detached,
            ``None`` for non-repo / unavailable).
        skip_branch_check: If True, skip all validation.

    Raises:
        BranchGuardError: When the branch does not match or is ambiguous.
    """
    if skip_branch_check:
        return

    metadata_file = track_dir / "metadata.json"
    if not metadata_file.is_file():
        return  # no metadata → nothing to guard

    try:
        data = json.loads(metadata_file.read_text(encoding="utf-8"))
    except (json.JSONDecodeError, OSError):
        return  # corrupt / unreadable → let downstream handle

    expected_branch = data.get("branch")
    if expected_branch is None:
        return  # branch=null → skip guard (legacy / planning phase)

    if current_branch is None:
        raise BranchGuardError(
            f"Cannot determine current git branch — expected '{expected_branch}'"
        )

    if current_branch == "HEAD":
        raise BranchGuardError(
            f"Detached HEAD — expected branch '{expected_branch}', cannot verify"
        )

    if current_branch != expected_branch:
        raise BranchGuardError(
            f"Current branch '{current_branch}' does not match "
            f"expected '{expected_branch}'"
        )
