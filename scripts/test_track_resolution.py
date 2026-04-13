from __future__ import annotations

import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

import scripts.track_resolution as track_resolution


def _write_metadata(
    track_dir: Path,
    *,
    track_id: str,
    status: str,
    updated_at: str,
    branch: str | None,
) -> None:
    track_dir.mkdir(parents=True, exist_ok=True)
    (track_dir / "metadata.json").write_text(
        json.dumps(
            {
                "schema_version": 3,
                "id": track_id,
                "title": f"Title {track_id}",
                "status": status,
                "created_at": "2025-01-01T00:00:00Z",
                "updated_at": updated_at,
                "branch": branch,
                "tasks": [],
                "plan": {"summary": [], "sections": []},
            }
        ),
        encoding="utf-8",
    )


class TrackResolutionTest(unittest.TestCase):
    def test_package_style_imports_work_from_repo_root(self) -> None:
        repo_root = Path(__file__).resolve().parent.parent
        result = subprocess.run(
            [
                sys.executable,
                "-c",
                "import sys; sys.path = ['.', *sys.path]; import scripts.track_resolution, scripts.external_guides",
            ],
            cwd=repo_root,
            capture_output=True,
            text=True,
            check=False,
        )

        self.assertEqual(result.returncode, 0, msg=result.stderr)

    def test_resolve_track_dir_does_not_treat_branchless_planning_track_as_current_track(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = Path(tmp_dir)
            items = root / "track" / "items"
            _write_metadata(
                items / "plan-only",
                track_id="plan-only",
                status="planned",
                updated_at="2025-06-15T00:00:00Z",
                branch=None,
            )

            track_dir, warnings = track_resolution.resolve_track_dir(
                root,
                git_branch="track/plan-only",
                allow_legacy_timestamp_fallback=False,
            )

        self.assertIsNone(track_dir)
        self.assertEqual(warnings, ["On branch 'track/plan-only' but no matching track found"])

    def test_resolve_track_dir_allows_legacy_v2_planned_track_on_track_branch(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = Path(tmp_dir)
            items = root / "track" / "items"
            legacy = items / "legacy"
            legacy.mkdir(parents=True, exist_ok=True)
            (legacy / "metadata.json").write_text(
                json.dumps(
                    {
                        "schema_version": 2,
                        "id": "legacy",
                        "status": "planned",
                        "updated_at": "2025-06-15T00:00:00Z",
                        "branch": None,
                    }
                ),
                encoding="utf-8",
            )

            track_dir, warnings = track_resolution.resolve_track_dir(
                root,
                git_branch="track/legacy",
                allow_legacy_timestamp_fallback=False,
            )

        self.assertEqual(track_dir, legacy)
        self.assertEqual(warnings, [])

    def test_resolve_track_dir_prefers_materialized_active_track_in_fallback(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = Path(tmp_dir)
            items = root / "track" / "items"
            _write_metadata(
                items / "materialized",
                track_id="materialized",
                status="in_progress",
                updated_at="2025-06-01T00:00:00Z",
                branch="track/materialized",
            )
            _write_metadata(
                items / "plan-only",
                track_id="plan-only",
                status="planned",
                updated_at="2025-06-15T00:00:00Z",
                branch=None,
            )

            track_dir, warnings = track_resolution.resolve_track_dir(
                root,
                git_branch="main",
                allow_legacy_timestamp_fallback=True,
            )

        self.assertEqual(track_dir, items / "materialized")
        self.assertEqual(warnings, ["Using latest track fallback: materialized"])

    def test_resolve_track_dir_fallback_uses_plan_only_when_no_materialized_active(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = Path(tmp_dir)
            items = root / "track" / "items"
            _write_metadata(
                items / "done-track",
                track_id="done-track",
                status="done",
                updated_at="2025-06-20T00:00:00Z",
                branch="track/done-track",
            )
            _write_metadata(
                items / "plan-only",
                track_id="plan-only",
                status="planned",
                updated_at="2025-06-15T00:00:00Z",
                branch=None,
            )

            track_dir, warnings = track_resolution.resolve_track_dir(
                root,
                git_branch="main",
                allow_legacy_timestamp_fallback=True,
            )

        self.assertEqual(track_dir, items / "plan-only")
        self.assertEqual(warnings, ["Using latest track fallback: plan-only"])

    def test_resolve_track_dir_prefers_legacy_v2_planned_over_newer_plan_only(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = Path(tmp_dir)
            items = root / "track" / "items"
            _write_metadata(
                items / "plan-only",
                track_id="plan-only",
                status="planned",
                updated_at="2025-06-15T00:00:00Z",
                branch=None,
            )
            legacy = items / "legacy-planned"
            legacy.mkdir(parents=True, exist_ok=True)
            (legacy / "metadata.json").write_text(
                json.dumps(
                    {
                        "schema_version": 2,
                        "id": "legacy-planned",
                        "status": "planned",
                        "updated_at": "2025-06-01T00:00:00Z",
                        "branch": None,
                    }
                ),
                encoding="utf-8",
            )

            track_dir, warnings = track_resolution.resolve_track_dir(
                root,
                git_branch="main",
                allow_legacy_timestamp_fallback=True,
            )

        self.assertEqual(track_dir, legacy)
        self.assertEqual(warnings, ["Using latest track fallback: legacy-planned"])

    def test_resolve_track_dir_prefers_legacy_v2_active_over_newer_plan_only(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = Path(tmp_dir)
            items = root / "track" / "items"
            _write_metadata(
                items / "plan-only",
                track_id="plan-only",
                status="planned",
                updated_at="2025-06-15T00:00:00Z",
                branch=None,
            )
            legacy = items / "legacy-active"
            legacy.mkdir(parents=True, exist_ok=True)
            (legacy / "metadata.json").write_text(
                json.dumps(
                    {
                        "schema_version": 2,
                        "id": "legacy-active",
                        "status": "in_progress",
                        "updated_at": "2025-06-01T00:00:00Z",
                        "branch": None,
                    }
                ),
                encoding="utf-8",
            )

            track_dir, warnings = track_resolution.resolve_track_dir(
                root,
                git_branch="main",
                allow_legacy_timestamp_fallback=True,
            )

        self.assertEqual(track_dir, legacy)
        self.assertEqual(warnings, ["Using latest track fallback: legacy-active"])

    def test_resolve_track_dir_skips_v3_track_missing_branch_field(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = Path(tmp_dir)
            items = root / "track" / "items"
            broken = items / "broken-v3"
            broken.mkdir(parents=True, exist_ok=True)
            (broken / "metadata.json").write_text(
                json.dumps(
                    {
                        "schema_version": 3,
                        "id": "broken-v3",
                        "status": "planned",
                        "updated_at": "2025-06-15T00:00:00Z",
                    }
                ),
                encoding="utf-8",
            )

            track_dir, warnings = track_resolution.resolve_track_dir(
                root,
                git_branch="main",
                allow_legacy_timestamp_fallback=True,
            )

        self.assertIsNone(track_dir)
        self.assertEqual(warnings, [])

    def test_resolve_track_dir_skips_illegal_branchless_v3_track(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = Path(tmp_dir)
            items = root / "track" / "items"
            broken = items / "broken-v3"
            broken.mkdir(parents=True, exist_ok=True)
            (broken / "metadata.json").write_text(
                json.dumps(
                    {
                        "schema_version": 3,
                        "id": "broken-v3",
                        "status": "in_progress",
                        "updated_at": "2025-06-15T00:00:00Z",
                        "branch": None,
                    }
                ),
                encoding="utf-8",
            )

            track_dir, warnings = track_resolution.resolve_track_dir(
                root,
                git_branch="main",
                allow_legacy_timestamp_fallback=True,
            )

        self.assertIsNone(track_dir)
        self.assertEqual(warnings, [])

    def test_resolve_track_dir_skips_v3_track_with_invalid_non_track_branch(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = Path(tmp_dir)
            items = root / "track" / "items"
            _write_metadata(
                items / "broken-v3",
                track_id="broken-v3",
                status="in_progress",
                updated_at="2025-06-15T00:00:00Z",
                branch="main",
            )

            track_dir, warnings = track_resolution.resolve_track_dir(
                root,
                git_branch="main",
                allow_legacy_timestamp_fallback=True,
            )

        self.assertIsNone(track_dir)
        self.assertEqual(warnings, [])
