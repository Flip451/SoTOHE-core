from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path

import scripts.verify_latest_track_files as verify_latest_track_files


def _write_metadata(
    track_dir: Path,
    *,
    status: str,
    updated_at: str,
    branch: str | None,
) -> None:
    track_dir.mkdir(parents=True, exist_ok=True)
    (track_dir / "metadata.json").write_text(
        json.dumps(
            {
                "schema_version": 3,
                "id": track_dir.name,
                "title": f"Title {track_dir.name}",
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


class VerifyLatestTrackFilesTest(unittest.TestCase):
    def test_latest_track_dir_prefers_materialized_active_over_newer_plan_only(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = Path(tmp_dir)
            items = root / "track" / "items"
            _write_metadata(
                items / "materialized",
                status="in_progress",
                updated_at="2025-06-01T00:00:00Z",
                branch="track/materialized",
            )
            _write_metadata(
                items / "plan-only",
                status="planned",
                updated_at="2025-06-15T00:00:00Z",
                branch=None,
            )

            latest_dir, errors = verify_latest_track_files.latest_track_dir(root)

        self.assertEqual(errors, [])
        self.assertEqual(latest_dir, items / "materialized")

    def test_latest_track_dir_prefers_plan_only_over_completed_track(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = Path(tmp_dir)
            items = root / "track" / "items"
            _write_metadata(
                items / "done-track",
                status="done",
                updated_at="2025-06-20T00:00:00Z",
                branch="track/done-track",
            )
            _write_metadata(
                items / "plan-only",
                status="planned",
                updated_at="2025-06-15T00:00:00Z",
                branch=None,
            )

            latest_dir, errors = verify_latest_track_files.latest_track_dir(root)

        self.assertEqual(errors, [])
        self.assertEqual(latest_dir, items / "plan-only")

    def test_latest_track_dir_reports_missing_branch_field_for_v3_track(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = Path(tmp_dir)
            broken = root / "track" / "items" / "broken-v3"
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

            latest_dir, errors = verify_latest_track_files.latest_track_dir(root)

        self.assertIsNone(latest_dir)
        self.assertTrue(any("branch is missing" in error for error in errors))

    def test_latest_track_dir_prefers_legacy_v2_planned_over_newer_plan_only(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = Path(tmp_dir)
            items = root / "track" / "items"
            _write_metadata(
                items / "plan-only",
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
                        "status": "planned",
                        "updated_at": "2025-06-01T00:00:00Z",
                        "branch": None,
                    }
                ),
                encoding="utf-8",
            )

            latest_dir, errors = verify_latest_track_files.latest_track_dir(root)

        self.assertEqual(errors, [])
        self.assertEqual(latest_dir, legacy)

    def test_latest_track_dir_prefers_legacy_v2_active_over_newer_plan_only(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = Path(tmp_dir)
            items = root / "track" / "items"
            _write_metadata(
                items / "plan-only",
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
                        "status": "in_progress",
                        "updated_at": "2025-06-01T00:00:00Z",
                        "branch": None,
                    }
                ),
                encoding="utf-8",
            )

            latest_dir, errors = verify_latest_track_files.latest_track_dir(root)

        self.assertEqual(errors, [])
        self.assertEqual(latest_dir, legacy)

    def test_latest_track_dir_reports_illegal_branchless_v3_track(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = Path(tmp_dir)
            broken = root / "track" / "items" / "broken-v3"
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

            latest_dir, errors = verify_latest_track_files.latest_track_dir(root)

        self.assertIsNone(latest_dir)
        self.assertTrue(any("branchless v3 metadata" in error for error in errors))

    def test_latest_track_dir_reports_invalid_non_track_branch_for_v3_track(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = Path(tmp_dir)
            broken = root / "track" / "items" / "broken-v3"
            broken.mkdir(parents=True, exist_ok=True)
            (broken / "metadata.json").write_text(
                json.dumps(
                    {
                        "schema_version": 3,
                        "id": "broken-v3",
                        "title": "Broken",
                        "status": "in_progress",
                        "created_at": "2025-01-01T00:00:00Z",
                        "updated_at": "2025-06-15T00:00:00Z",
                        "branch": "main",
                        "tasks": [],
                        "plan": {"summary": [], "sections": []},
                    }
                ),
                encoding="utf-8",
            )

            latest_dir, errors = verify_latest_track_files.latest_track_dir(root)

        self.assertIsNone(latest_dir)
        self.assertTrue(any("branch" in error for error in errors))

    def test_latest_track_dir_reports_non_object_metadata_as_invalid(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = Path(tmp_dir)
            broken = root / "track" / "items" / "broken-v3"
            broken.mkdir(parents=True, exist_ok=True)
            (broken / "metadata.json").write_text("[]", encoding="utf-8")

            latest_dir, errors = verify_latest_track_files.latest_track_dir(root)

        self.assertIsNone(latest_dir)
        self.assertTrue(any("metadata.json is invalid" in error for error in errors))
