"""Tests for track_registry.py — registry.md rendering from metadata.json."""

from __future__ import annotations

import json
import tempfile
import unittest
from pathlib import Path

from track_registry import collect_track_metadata, render_registry, write_registry
from track_state_machine import sync_rendered_views


def _make_track(
    root: Path,
    track_id: str,
    *,
    schema_version: int = 2,
    title: str = "Test Track",
    status: str = "planned",
    updated_at: str = "2026-03-08T00:00:00Z",
    created_at: str = "2026-03-08T00:00:00Z",
    tasks: list[dict] | None = None,
    sections: list[dict] | None = None,
    status_override: dict | None = None,
    branch: str | None = None,
) -> Path:
    track_dir = root / "track" / "items" / track_id
    track_dir.mkdir(parents=True, exist_ok=True)

    t = tasks or []
    s = sections or [
        {
            "id": "s1",
            "title": "Section 1",
            "description": [],
            "task_ids": [tk["id"] for tk in t],
        }
    ]

    data = {
        "schema_version": schema_version,
        "id": track_id,
        "branch": branch,
        "title": title,
        "status": status,
        "created_at": created_at,
        "updated_at": updated_at,
        "status_override": status_override,
        "tasks": t,
        "plan": {"summary": [], "sections": s},
    }
    (track_dir / "metadata.json").write_text(
        json.dumps(data, indent=2) + "\n", encoding="utf-8"
    )
    return track_dir


def _task(id: str, status: str = "todo", commit_hash: str | None = None) -> dict:
    return {
        "id": id,
        "description": f"Task {id}",
        "status": status,
        "commit_hash": commit_hash,
    }


class TestCollectTrackMetadata(unittest.TestCase):
    def test_empty_track_items(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / "track" / "items").mkdir(parents=True)
            result = collect_track_metadata(root)
            self.assertEqual(result, [])

    def test_collects_single_track(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            _make_track(root, "feat-001", title="First Feature", status="in_progress")
            result = collect_track_metadata(root)
            self.assertEqual(len(result), 1)
            self.assertEqual(result[0].id, "feat-001")
            self.assertEqual(result[0].title, "First Feature")

    def test_skips_non_v2_tracks(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            track_dir = root / "track" / "items" / "old-track"
            track_dir.mkdir(parents=True)
            data = {"schema_version": 1, "id": "old-track", "title": "Old"}
            (track_dir / "metadata.json").write_text(json.dumps(data))
            result = collect_track_metadata(root)
            self.assertEqual(result, [])

    def test_skips_dirs_without_metadata(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / "track" / "items" / "no-meta").mkdir(parents=True)
            result = collect_track_metadata(root)
            self.assertEqual(result, [])

    def test_returns_sorted_by_updated_at_descending(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            _make_track(root, "old", updated_at="2026-03-01T00:00:00Z")
            _make_track(root, "new", updated_at="2026-03-08T00:00:00Z")
            _make_track(root, "mid", updated_at="2026-03-05T00:00:00Z")
            result = collect_track_metadata(root)
            ids = [m.id for m in result]
            self.assertEqual(ids, ["new", "mid", "old"])


class TestRenderRegistry(unittest.TestCase):
    def test_render_registry_empty_tracks(self) -> None:
        output = render_registry([])
        self.assertIn("# Track Registry", output)
        self.assertIn("_No active tracks yet_", output)
        self.assertIn("_No completed tracks yet_", output)
        self.assertIn("None yet", output)

    def test_render_registry_single_active_track(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            _make_track(
                root,
                "feat-001",
                title="Feature One",
                status="in_progress",
                updated_at="2026-03-08T00:00:00Z",
                tasks=[_task("T001", "in_progress")],
            )
            tracks = collect_track_metadata(root)
            output = render_registry(tracks)

            self.assertIn("feat-001", output)
            self.assertIn("in_progress", output)
            self.assertIn("`feat-001`", output)
            self.assertNotIn("_No active tracks yet_", output)
            self.assertIn("_No completed tracks yet_", output)

    def test_render_registry_active_and_completed_split(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            _make_track(
                root,
                "feat-done",
                title="Done Feature",
                status="done",
                updated_at="2026-03-07T00:00:00Z",
                tasks=[_task("T001", "done", "abc1234")],
            )
            _make_track(
                root,
                "feat-active",
                title="Active Feature",
                status="in_progress",
                updated_at="2026-03-08T00:00:00Z",
                tasks=[_task("T001", "in_progress")],
            )
            tracks = collect_track_metadata(root)
            output = render_registry(tracks)

            self.assertNotIn("_No active tracks yet_", output)
            self.assertNotIn("_No completed tracks yet_", output)
            self.assertIn("feat-active", output)
            self.assertIn("feat-done", output)

    def test_render_registry_deterministic_ordering(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            _make_track(
                root,
                "z-old",
                status="in_progress",
                updated_at="2026-03-01T00:00:00Z",
                tasks=[_task("T001", "in_progress")],
            )
            _make_track(
                root,
                "a-new",
                status="in_progress",
                updated_at="2026-03-08T00:00:00Z",
                tasks=[_task("T001", "in_progress")],
            )
            tracks = collect_track_metadata(root)
            output1 = render_registry(tracks)
            output2 = render_registry(tracks)
            self.assertEqual(output1, output2)
            # Newer track should appear first
            a_pos = output1.index("a-new")
            z_pos = output1.index("z-old")
            self.assertLess(a_pos, z_pos)

    def test_render_registry_current_focus_shows_latest_active(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            _make_track(
                root,
                "feat-old",
                status="in_progress",
                updated_at="2026-03-01T00:00:00Z",
                tasks=[_task("T001", "in_progress")],
            )
            _make_track(
                root,
                "feat-new",
                status="in_progress",
                updated_at="2026-03-08T00:00:00Z",
                tasks=[_task("T001", "in_progress")],
            )
            tracks = collect_track_metadata(root)
            output = render_registry(tracks)
            self.assertIn("`feat-new`", output)

    def test_render_registry_blocked_track_in_active(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            _make_track(
                root,
                "feat-blocked",
                status="blocked",
                updated_at="2026-03-08T00:00:00Z",
                tasks=[_task("T001", "todo")],
                status_override={"status": "blocked", "reason": "waiting"},
            )
            tracks = collect_track_metadata(root)
            output = render_registry(tracks)
            self.assertIn("blocked", output)
            self.assertNotIn("_No active tracks yet_", output)

    def test_render_registry_next_command_for_planned_track(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            _make_track(
                root,
                "feat-planned",
                status="planned",
                updated_at="2026-03-08T00:00:00Z",
                branch="track/feat-planned",
            )
            tracks = collect_track_metadata(root)
            output = render_registry(tracks)
            self.assertIn("/track:implement", output)

    def test_render_registry_next_command_for_branchless_planning_only_track(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            _make_track(
                root,
                "feat-plan-only",
                schema_version=3,
                status="planned",
                updated_at="2026-03-08T00:00:00Z",
                branch=None,
            )
            tracks = collect_track_metadata(root)
            output = render_registry(tracks)
            self.assertIn("/track:activate feat-plan-only", output)
            self.assertIn("/track:plan-only <feature>", output)

    def test_render_registry_keeps_legacy_v2_branchless_planned_track_on_implement(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            _make_track(
                root,
                "feat-legacy-planned",
                schema_version=2,
                status="planned",
                updated_at="2026-03-08T00:00:00Z",
                branch=None,
            )
            tracks = collect_track_metadata(root)
            output = render_registry(tracks)
            self.assertIn("/track:implement", output)
            self.assertNotIn("/track:activate feat-legacy-planned", output)

    def test_collect_track_metadata_rejects_v3_track_missing_branch_field(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            track_dir = root / "track" / "items" / "broken-v3"
            track_dir.mkdir(parents=True, exist_ok=True)
            (track_dir / "metadata.json").write_text(
                json.dumps(
                    {
                        "schema_version": 3,
                        "id": "broken-v3",
                        "title": "Broken",
                        "status": "planned",
                        "created_at": "2026-03-08T00:00:00Z",
                        "updated_at": "2026-03-08T00:00:00Z",
                        "tasks": [],
                        "plan": {"summary": [], "sections": []},
                    }
                ),
                encoding="utf-8",
            )

            with self.assertRaisesRegex(ValueError, "Missing required field 'branch'"):
                collect_track_metadata(root)

    def test_collect_track_metadata_rejects_illegal_branchless_v3_track(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            track_dir = root / "track" / "items" / "broken-v3"
            track_dir.mkdir(parents=True, exist_ok=True)
            (track_dir / "metadata.json").write_text(
                json.dumps(
                    {
                        "schema_version": 3,
                        "id": "broken-v3",
                        "title": "Broken",
                        "branch": None,
                        "status": "in_progress",
                        "created_at": "2026-03-08T00:00:00Z",
                        "updated_at": "2026-03-08T00:00:00Z",
                        "tasks": [{"id": "T001", "description": "x", "status": "todo"}],
                        "plan": {"summary": [], "sections": []},
                    }
                ),
                encoding="utf-8",
            )

            with self.assertRaisesRegex(ValueError, "Illegal branchless v3 track"):
                collect_track_metadata(root)

    def test_render_registry_prefers_materialized_active_track_in_current_focus(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            _make_track(
                root,
                "plan-only-newer",
                schema_version=3,
                status="planned",
                updated_at="2026-03-09T00:00:00Z",
                branch=None,
            )
            _make_track(
                root,
                "materialized-older",
                status="planned",
                updated_at="2026-03-08T00:00:00Z",
                branch="track/materialized-older",
            )
            tracks = collect_track_metadata(root)
            output = render_registry(tracks)
            self.assertIn("- Latest active track: `materialized-older`", output)
            self.assertIn("- Next recommended command: `/track:implement`", output)

    def test_render_registry_prefers_legacy_v2_planned_over_newer_plan_only(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            _make_track(
                root,
                "plan-only-newer",
                schema_version=3,
                status="planned",
                updated_at="2026-03-09T00:00:00Z",
                branch=None,
            )
            _make_track(
                root,
                "legacy-planned",
                schema_version=2,
                status="planned",
                updated_at="2026-03-08T00:00:00Z",
                branch=None,
            )
            tracks = collect_track_metadata(root)
            output = render_registry(tracks)
            self.assertIn("- Latest active track: `legacy-planned`", output)
            self.assertIn("- Next recommended command: `/track:implement`", output)


class TestWriteRegistry(unittest.TestCase):
    def test_write_registry_creates_file(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / "track" / "items").mkdir(parents=True)
            path = write_registry(root)
            self.assertTrue(path.exists())
            self.assertEqual(path, root / "track" / "registry.md")
            content = path.read_text(encoding="utf-8")
            self.assertIn("# Track Registry", content)

    def test_write_registry_with_tracks(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            _make_track(
                root,
                "feat-001",
                title="Feature One",
                status="in_progress",
                tasks=[_task("T001", "in_progress")],
            )
            path = write_registry(root)
            content = path.read_text(encoding="utf-8")
            self.assertIn("feat-001", content)


class TestSyncRenderedViewsRegistry(unittest.TestCase):
    def test_sync_rendered_views_updates_registry(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            _make_track(
                root,
                "feat-001",
                status="in_progress",
                tasks=[_task("T001", "in_progress")],
                sections=[
                    {
                        "id": "s1",
                        "title": "Section",
                        "description": [],
                        "task_ids": ["T001"],
                    }
                ],
            )
            changed = sync_rendered_views(root)
            paths = [p.name for p in changed]
            self.assertIn("plan.md", paths)
            self.assertIn("registry.md", paths)
            # Verify registry content
            registry = (root / "track" / "registry.md").read_text(encoding="utf-8")
            self.assertIn("feat-001", registry)

    def test_sync_rendered_views_skips_unchanged_registry(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            _make_track(
                root,
                "feat-001",
                status="in_progress",
                tasks=[_task("T001", "in_progress")],
                sections=[
                    {
                        "id": "s1",
                        "title": "Section",
                        "description": [],
                        "task_ids": ["T001"],
                    }
                ],
            )
            # First sync writes registry
            sync_rendered_views(root)
            # Second sync should not include registry.md in changed (content unchanged)
            changed = sync_rendered_views(root)
            registry_paths = [p for p in changed if p.name == "registry.md"]
            self.assertEqual(len(registry_paths), 0)


class TestArchivedTracks(unittest.TestCase):
    def test_archived_track_appears_in_archived_section(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            _make_track(
                root,
                "feat-old",
                title="Old Feature",
                status="archived",
                updated_at="2026-03-01T00:00:00Z",
                tasks=[_task("T001", "done", "abc1234")],
            )
            tracks = collect_track_metadata(root)
            output = render_registry(tracks)
            self.assertIn("## Archived Tracks", output)
            self.assertIn("feat-old", output)
            self.assertNotIn("_No archived tracks yet_", output)

    def test_archived_track_not_in_active_or_completed(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            _make_track(
                root,
                "feat-archived",
                title="Archived Feature",
                status="archived",
                updated_at="2026-03-01T00:00:00Z",
                tasks=[_task("T001", "done", "abc1234")],
            )
            tracks = collect_track_metadata(root)
            output = render_registry(tracks)
            # Should not appear between Active Tracks and Completed Tracks headers
            active_section = output.split("## Active Tracks")[1].split(
                "## Completed Tracks"
            )[0]
            completed_section = output.split("## Completed Tracks")[1].split(
                "## Archived Tracks"
            )[0]
            self.assertNotIn("feat-archived", active_section)
            self.assertNotIn("feat-archived", completed_section)

    def test_archived_track_excluded_from_current_focus(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            _make_track(
                root,
                "feat-archived",
                title="Archived Feature",
                status="archived",
                updated_at="2026-03-08T00:00:00Z",
                tasks=[_task("T001", "done", "abc1234")],
            )
            tracks = collect_track_metadata(root)
            output = render_registry(tracks)
            # No active tracks, so current focus should show None yet
            self.assertIn("None yet", output)

    def test_empty_archived_section_placeholder(self) -> None:
        output = render_registry([])
        self.assertIn("_No archived tracks yet_", output)

    def test_mixed_active_completed_archived(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            _make_track(
                root,
                "feat-active",
                status="in_progress",
                updated_at="2026-03-08T00:00:00Z",
                tasks=[_task("T001", "in_progress")],
            )
            _make_track(
                root,
                "feat-done",
                status="done",
                updated_at="2026-03-07T00:00:00Z",
                tasks=[_task("T001", "done", "abc1234")],
            )
            _make_track(
                root,
                "feat-archived",
                status="archived",
                updated_at="2026-03-01T00:00:00Z",
                tasks=[_task("T001", "done", "def5678")],
            )
            tracks = collect_track_metadata(root)
            output = render_registry(tracks)
            # All three sections should have content
            self.assertNotIn("_No active tracks yet_", output)
            self.assertNotIn("_No completed tracks yet_", output)
            self.assertNotIn("_No archived tracks yet_", output)


class TestCollectEdgeCases(unittest.TestCase):
    def test_track_items_is_file_not_dir(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / "track").mkdir()
            (root / "track" / "items").write_text("not a dir")
            result = collect_track_metadata(root)
            self.assertEqual(result, [])

    def test_equal_updated_at_stable_order(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            _make_track(root, "aaa", updated_at="2026-03-08T00:00:00Z")
            _make_track(root, "zzz", updated_at="2026-03-08T00:00:00Z")
            result1 = collect_track_metadata(root)
            result2 = collect_track_metadata(root)
            ids1 = [m.id for m in result1]
            ids2 = [m.id for m in result2]
            self.assertEqual(ids1, ids2)

    def test_malformed_metadata_skipped(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            track_dir = root / "track" / "items" / "bad"
            track_dir.mkdir(parents=True)
            (track_dir / "metadata.json").write_text("{invalid json")
            result = collect_track_metadata(root)
            self.assertEqual(result, [])


class TestCollectFromArchiveDirectory(unittest.TestCase):
    """Verify collect_track_metadata() scans track/archive/ in addition to track/items/."""

    def test_archived_track_in_archive_dir_is_collected(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            archive_dir = root / "track" / "archive" / "old-feat"
            archive_dir.mkdir(parents=True)
            (archive_dir / "metadata.json").write_text(
                json.dumps({
                    "schema_version": 2,
                    "id": "old-feat",
                    "title": "Old Feature",
                    "status": "archived",
                    "created_at": "2026-01-01T00:00:00Z",
                    "updated_at": "2026-01-01T00:00:00Z",
                    "tasks": [],
                    "plan": {"summary": [], "sections": []},
                    "status_override": None,
                })
            )
            result = collect_track_metadata(root)

        self.assertEqual(len(result), 1)
        self.assertEqual(result[0].id, "old-feat")
        self.assertEqual(result[0].status, "archived")

    def test_both_items_and_archive_are_collected(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            _make_track(root, "active-feat", status="planned")
            archive_dir = root / "track" / "archive" / "done-feat"
            archive_dir.mkdir(parents=True)
            (archive_dir / "metadata.json").write_text(
                json.dumps({
                    "schema_version": 2,
                    "id": "done-feat",
                    "title": "Done Feature",
                    "status": "archived",
                    "created_at": "2026-01-01T00:00:00Z",
                    "updated_at": "2026-01-02T00:00:00Z",
                    "tasks": [],
                    "plan": {"summary": [], "sections": []},
                    "status_override": None,
                })
            )
            result = collect_track_metadata(root)

        ids = {m.id for m in result}
        self.assertIn("active-feat", ids)
        self.assertIn("done-feat", ids)

    def test_registry_renders_archived_from_archive_dir(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            archive_dir = root / "track" / "archive" / "old-feat"
            archive_dir.mkdir(parents=True)
            (archive_dir / "metadata.json").write_text(
                json.dumps({
                    "schema_version": 2,
                    "id": "old-feat",
                    "title": "Old Feature",
                    "status": "archived",
                    "created_at": "2026-01-01T00:00:00Z",
                    "updated_at": "2026-01-01T00:00:00Z",
                    "tasks": [],
                    "plan": {"summary": [], "sections": []},
                    "status_override": None,
                })
            )
            tracks = collect_track_metadata(root)
            rendered = render_registry(tracks)

        self.assertIn("old-feat", rendered)
        self.assertIn("## Archived Tracks", rendered)
        self.assertNotIn("_No archived tracks yet_", rendered)


if __name__ == "__main__":
    unittest.main()
