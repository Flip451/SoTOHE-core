"""Tests for track_branch_guard.py — branch guard verification."""

from __future__ import annotations

import json
from pathlib import Path

import pytest
from track_branch_guard import BranchGuardError, verify_track_branch


def _write_metadata(track_dir: Path, branch: str | None) -> None:
    track_dir.mkdir(parents=True, exist_ok=True)
    data = {
        "schema_version": 3,
        "id": track_dir.name,
        "branch": branch,
        "title": "Test",
        "status": "planned",
        "created_at": "2026-03-12T00:00:00Z",
        "updated_at": "2026-03-12T00:00:00Z",
        "tasks": [],
        "plan": {"summary": [], "sections": []},
    }
    (track_dir / "metadata.json").write_text(
        json.dumps(data, indent=2) + "\n", encoding="utf-8"
    )


def test_matching_branch_passes(tmp_path: Path) -> None:
    track_dir = tmp_path / "my-track"
    _write_metadata(track_dir, "track/my-track")
    # Should not raise
    verify_track_branch(track_dir, current_branch="track/my-track")


def test_mismatched_branch_raises(tmp_path: Path) -> None:
    track_dir = tmp_path / "my-track"
    _write_metadata(track_dir, "track/my-track")
    with pytest.raises(BranchGuardError, match="does not match"):
        verify_track_branch(track_dir, current_branch="main")


def test_null_branch_skips_guard(tmp_path: Path) -> None:
    track_dir = tmp_path / "my-track"
    _write_metadata(track_dir, None)
    # Should not raise even with wrong branch
    verify_track_branch(track_dir, current_branch="main")


def test_detached_head_raises(tmp_path: Path) -> None:
    track_dir = tmp_path / "my-track"
    _write_metadata(track_dir, "track/my-track")
    with pytest.raises(BranchGuardError, match="Detached HEAD"):
        verify_track_branch(track_dir, current_branch="HEAD")


def test_none_current_branch_raises(tmp_path: Path) -> None:
    track_dir = tmp_path / "my-track"
    _write_metadata(track_dir, "track/my-track")
    with pytest.raises(BranchGuardError, match="Cannot determine"):
        verify_track_branch(track_dir, current_branch=None)


def test_skip_branch_check_bypasses(tmp_path: Path) -> None:
    track_dir = tmp_path / "my-track"
    _write_metadata(track_dir, "track/my-track")
    # Should not raise even with wrong branch when skip is set
    verify_track_branch(
        track_dir, current_branch="wrong-branch", skip_branch_check=True
    )


def test_missing_metadata_skips(tmp_path: Path) -> None:
    track_dir = tmp_path / "no-metadata"
    track_dir.mkdir(parents=True)
    # No metadata.json → skip guard silently
    verify_track_branch(track_dir, current_branch="main")
