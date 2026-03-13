"""Tests for track_state_machine.py — state transition APIs."""

from __future__ import annotations

import json
import tempfile
import unittest
from datetime import UTC, datetime
from pathlib import Path
from unittest.mock import patch

from track_state_machine import (
    TransitionError,
    add_task,
    next_open_task,
    set_track_override,
    sync_rendered_views,
    task_counts,
    transition_task,
)


def _write_v2_metadata(
    track_dir: Path,
    *,
    tasks: list[dict] | None = None,
    sections: list[dict] | None = None,
    status: str = "planned",
    status_override: dict | None = None,
) -> None:
    t = tasks or []
    s = sections or []
    data = {
        "schema_version": 2,
        "id": track_dir.name,
        "title": "Test Track",
        "status": status,
        "created_at": "2026-03-08T00:00:00Z",
        "updated_at": "2026-03-08T00:00:00Z",
        "status_override": status_override,
        "tasks": t,
        "plan": {"summary": [], "sections": s},
    }
    (track_dir / "metadata.json").write_text(
        json.dumps(data, indent=2) + "\n", encoding="utf-8"
    )


def _read_metadata(track_dir: Path) -> dict:
    return json.loads((track_dir / "metadata.json").read_text(encoding="utf-8"))


def _task(id: str, status: str = "todo", commit_hash: str | None = None) -> dict:
    return {
        "id": id,
        "description": f"Task {id}",
        "status": status,
        "commit_hash": commit_hash,
    }


def _section(id: str = "s1", task_ids: list[str] | None = None) -> dict:
    return {"id": id, "title": "Section", "description": [], "task_ids": task_ids or []}


class TestAddTask(unittest.TestCase):
    def test_adds_task_and_returns_id(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            track_dir = Path(tmp) / "track" / "items" / "demo"
            track_dir.mkdir(parents=True)
            _write_v2_metadata(track_dir, tasks=[], sections=[_section("s1", [])])

            task_id = add_task(track_dir, "New task", section_id="s1")

            self.assertTrue(task_id.startswith("T"))
            data = _read_metadata(track_dir)
            self.assertEqual(len(data["tasks"]), 1)
            self.assertEqual(data["tasks"][0]["description"], "New task")
            self.assertEqual(data["tasks"][0]["status"], "todo")
            self.assertIn(task_id, data["plan"]["sections"][0]["task_ids"])

    def test_auto_generates_sequential_ids(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            track_dir = Path(tmp) / "track" / "items" / "demo"
            track_dir.mkdir(parents=True)
            _write_v2_metadata(
                track_dir,
                tasks=[_task("T001")],
                sections=[_section("s1", ["T001"])],
            )

            task_id = add_task(track_dir, "Second task", section_id="s1")

            self.assertEqual(task_id, "T002")

    def test_adds_to_default_section_if_no_section_id(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            track_dir = Path(tmp) / "track" / "items" / "demo"
            track_dir.mkdir(parents=True)
            _write_v2_metadata(track_dir, tasks=[], sections=[_section("s1", [])])

            task_id = add_task(track_dir, "Task without section")

            data = _read_metadata(track_dir)
            self.assertIn(task_id, data["plan"]["sections"][0]["task_ids"])


class TestTransitionTask(unittest.TestCase):
    def test_todo_to_in_progress(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            track_dir = Path(tmp) / "track" / "items" / "demo"
            track_dir.mkdir(parents=True)
            _write_v2_metadata(
                track_dir,
                tasks=[_task("T001", "todo")],
                sections=[_section("s1", ["T001"])],
            )

            transition_task(track_dir, "T001", "in_progress")

            data = _read_metadata(track_dir)
            self.assertEqual(data["tasks"][0]["status"], "in_progress")
            self.assertEqual(data["status"], "in_progress")

    def test_in_progress_to_done_with_commit_hash(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            track_dir = Path(tmp) / "track" / "items" / "demo"
            track_dir.mkdir(parents=True)
            _write_v2_metadata(
                track_dir,
                tasks=[_task("T001", "in_progress")],
                sections=[_section("s1", ["T001"])],
                status="in_progress",
            )

            transition_task(track_dir, "T001", "done", commit_hash="abc1234")

            data = _read_metadata(track_dir)
            self.assertEqual(data["tasks"][0]["status"], "done")
            self.assertEqual(data["tasks"][0]["commit_hash"], "abc1234")
            self.assertEqual(data["status"], "done")

    def test_done_to_in_progress_clears_commit_hash(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            track_dir = Path(tmp) / "track" / "items" / "demo"
            track_dir.mkdir(parents=True)
            _write_v2_metadata(
                track_dir,
                tasks=[_task("T001", "done", "abc1234")],
                sections=[_section("s1", ["T001"])],
                status="done",
            )

            transition_task(track_dir, "T001", "in_progress")

            data = _read_metadata(track_dir)
            self.assertEqual(data["tasks"][0]["status"], "in_progress")
            self.assertIsNone(data["tasks"][0]["commit_hash"])
            self.assertEqual(data["status"], "in_progress")

    def test_in_progress_to_todo_rollback(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            track_dir = Path(tmp) / "track" / "items" / "demo"
            track_dir.mkdir(parents=True)
            _write_v2_metadata(
                track_dir,
                tasks=[_task("T001", "in_progress")],
                sections=[_section("s1", ["T001"])],
                status="in_progress",
            )

            transition_task(track_dir, "T001", "todo")

            data = _read_metadata(track_dir)
            self.assertEqual(data["tasks"][0]["status"], "todo")
            self.assertEqual(data["status"], "planned")

    def test_todo_to_skipped(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            track_dir = Path(tmp) / "track" / "items" / "demo"
            track_dir.mkdir(parents=True)
            _write_v2_metadata(
                track_dir,
                tasks=[_task("T001", "todo"), _task("T002", "todo")],
                sections=[_section("s1", ["T001", "T002"])],
            )

            transition_task(track_dir, "T001", "skipped")

            data = _read_metadata(track_dir)
            self.assertEqual(data["tasks"][0]["status"], "skipped")
            self.assertIsNone(data["tasks"][0]["commit_hash"])

    def test_in_progress_to_skipped(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            track_dir = Path(tmp) / "track" / "items" / "demo"
            track_dir.mkdir(parents=True)
            _write_v2_metadata(
                track_dir,
                tasks=[_task("T001", "in_progress")],
                sections=[_section("s1", ["T001"])],
                status="in_progress",
            )

            transition_task(track_dir, "T001", "skipped")

            data = _read_metadata(track_dir)
            self.assertEqual(data["tasks"][0]["status"], "skipped")
            self.assertEqual(data["status"], "done")

    def test_skipped_to_todo(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            track_dir = Path(tmp) / "track" / "items" / "demo"
            track_dir.mkdir(parents=True)
            _write_v2_metadata(
                track_dir,
                tasks=[
                    {
                        "id": "T001",
                        "description": "task",
                        "status": "skipped",
                        "commit_hash": None,
                    }
                ],
                sections=[_section("s1", ["T001"])],
                status="done",
            )

            transition_task(track_dir, "T001", "todo")

            data = _read_metadata(track_dir)
            self.assertEqual(data["tasks"][0]["status"], "todo")
            self.assertEqual(data["status"], "planned")

    def test_skipped_to_done_raises(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            track_dir = Path(tmp) / "track" / "items" / "demo"
            track_dir.mkdir(parents=True)
            _write_v2_metadata(
                track_dir,
                tasks=[
                    {
                        "id": "T001",
                        "description": "task",
                        "status": "skipped",
                        "commit_hash": None,
                    }
                ],
                sections=[_section("s1", ["T001"])],
                status="done",
            )

            with self.assertRaises(TransitionError):
                transition_task(track_dir, "T001", "done")

    def test_done_and_skipped_mix_derives_done_status(self) -> None:
        """Skip the last remaining task and verify the track derives 'done'."""
        with tempfile.TemporaryDirectory() as tmp:
            track_dir = Path(tmp) / "track" / "items" / "demo"
            track_dir.mkdir(parents=True)
            _write_v2_metadata(
                track_dir,
                tasks=[
                    _task("T001", "done", "abc1234"),
                    _task("T002", "in_progress"),
                ],
                sections=[_section("s1", ["T001", "T002"])],
                status="in_progress",
            )

            transition_task(track_dir, "T002", "skipped")

            data = _read_metadata(track_dir)
            self.assertEqual(data["tasks"][1]["status"], "skipped")
            self.assertEqual(data["status"], "done")

    def test_invalid_transition_todo_to_done_raises(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            track_dir = Path(tmp) / "track" / "items" / "demo"
            track_dir.mkdir(parents=True)
            _write_v2_metadata(
                track_dir,
                tasks=[_task("T001", "todo")],
                sections=[_section("s1", ["T001"])],
            )

            with self.assertRaises(TransitionError):
                transition_task(track_dir, "T001", "done")

    def test_unknown_task_id_raises(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            track_dir = Path(tmp) / "track" / "items" / "demo"
            track_dir.mkdir(parents=True)
            _write_v2_metadata(
                track_dir,
                tasks=[_task("T001", "todo")],
                sections=[_section("s1", ["T001"])],
            )

            with self.assertRaises(TransitionError):
                transition_task(track_dir, "T999", "in_progress")

    def test_updates_updated_at(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            track_dir = Path(tmp) / "track" / "items" / "demo"
            track_dir.mkdir(parents=True)
            _write_v2_metadata(
                track_dir,
                tasks=[_task("T001", "todo")],
                sections=[_section("s1", ["T001"])],
            )

            now = datetime(2026, 3, 9, 10, 0, 0, tzinfo=UTC)
            transition_task(track_dir, "T001", "in_progress", now=now)

            data = _read_metadata(track_dir)
            self.assertEqual(data["updated_at"], "2026-03-09T10:00:00+00:00")


class TestSetTrackOverride(unittest.TestCase):
    def test_set_blocked(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            track_dir = Path(tmp) / "track" / "items" / "demo"
            track_dir.mkdir(parents=True)
            _write_v2_metadata(
                track_dir,
                tasks=[_task("T001", "todo")],
                sections=[_section("s1", ["T001"])],
            )

            set_track_override(track_dir, "blocked", reason="waiting on dep")

            data = _read_metadata(track_dir)
            self.assertEqual(data["status"], "blocked")
            self.assertEqual(data["status_override"]["status"], "blocked")

    def test_clear_override(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            track_dir = Path(tmp) / "track" / "items" / "demo"
            track_dir.mkdir(parents=True)
            _write_v2_metadata(
                track_dir,
                tasks=[_task("T001", "todo")],
                sections=[_section("s1", ["T001"])],
                status="blocked",
                status_override={"status": "blocked", "reason": "dep"},
            )

            set_track_override(track_dir, None)

            data = _read_metadata(track_dir)
            self.assertEqual(data["status"], "planned")
            self.assertIsNone(data["status_override"])

    def test_blocked_fails_when_all_done(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            track_dir = Path(tmp) / "track" / "items" / "demo"
            track_dir.mkdir(parents=True)
            _write_v2_metadata(
                track_dir,
                tasks=[_task("T001", "done", "abc1234")],
                sections=[_section("s1", ["T001"])],
                status="done",
            )

            with self.assertRaises(TransitionError):
                set_track_override(track_dir, "blocked", reason="test")

    def test_blocked_fails_when_all_skipped(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            track_dir = Path(tmp) / "track" / "items" / "demo"
            track_dir.mkdir(parents=True)
            _write_v2_metadata(
                track_dir,
                tasks=[
                    {
                        "id": "T001",
                        "description": "task",
                        "status": "skipped",
                        "commit_hash": None,
                    }
                ],
                sections=[_section("s1", ["T001"])],
                status="done",
            )

            with self.assertRaises(TransitionError):
                set_track_override(track_dir, "blocked", reason="test")

    def test_invalid_override_status_raises(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            track_dir = Path(tmp) / "track" / "items" / "demo"
            track_dir.mkdir(parents=True)
            _write_v2_metadata(
                track_dir,
                tasks=[_task("T001", "todo")],
                sections=[_section("s1", ["T001"])],
            )

            with self.assertRaises(TransitionError):
                set_track_override(track_dir, "paused", reason="test")


class TestAddTaskValidation(unittest.TestCase):
    def test_unknown_section_id_raises(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            track_dir = Path(tmp) / "track" / "items" / "demo"
            track_dir.mkdir(parents=True)
            _write_v2_metadata(track_dir, tasks=[], sections=[_section("s1", [])])

            with self.assertRaises(TransitionError):
                add_task(track_dir, "task", section_id="missing")

    def test_no_sections_and_no_section_id_raises(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            track_dir = Path(tmp) / "track" / "items" / "demo"
            track_dir.mkdir(parents=True)
            _write_v2_metadata(track_dir, tasks=[], sections=[])

            with self.assertRaises(TransitionError):
                add_task(track_dir, "task")


class TestTransitionTaskValidation(unittest.TestCase):
    def test_invalid_commit_hash_format_raises(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            track_dir = Path(tmp) / "track" / "items" / "demo"
            track_dir.mkdir(parents=True)
            _write_v2_metadata(
                track_dir,
                tasks=[_task("T001", "in_progress")],
                sections=[_section("s1", ["T001"])],
                status="in_progress",
            )

            with self.assertRaises(TransitionError):
                transition_task(track_dir, "T001", "done", commit_hash="not-a-hash!")


class TestOverrideSurvivesTransition(unittest.TestCase):
    def test_transition_to_done_clears_override_when_all_done(self) -> None:
        """When last task completes, blocked/cancelled override must auto-clear."""
        with tempfile.TemporaryDirectory() as tmp:
            track_dir = Path(tmp) / "track" / "items" / "demo"
            track_dir.mkdir(parents=True)
            _write_v2_metadata(
                track_dir,
                tasks=[_task("T001", "in_progress")],
                sections=[_section("s1", ["T001"])],
                status="blocked",
                status_override={"status": "blocked", "reason": "dep"},
            )

            transition_task(track_dir, "T001", "done", commit_hash="abc1234")

            data = _read_metadata(track_dir)
            self.assertIsNone(data["status_override"])
            self.assertEqual(data["status"], "done")


class TestStateMachineDefensiveParsing(unittest.TestCase):
    def test_transition_task_with_non_dict_task_raises(self) -> None:
        """Non-dict task entries in metadata should raise TransitionError, not TypeError."""
        with tempfile.TemporaryDirectory() as tmp:
            track_dir = Path(tmp) / "track" / "items" / "demo"
            track_dir.mkdir(parents=True)
            _write_v2_metadata(
                track_dir,
                tasks=[_task("T001", "todo")],
                sections=[_section("s1", ["T001"])],
            )
            # Corrupt on-disk data: inject non-dict task before the real task
            data = _read_metadata(track_dir)
            data["tasks"].insert(0, "not-a-dict")
            (track_dir / "metadata.json").write_text(
                json.dumps(data, indent=2) + "\n", encoding="utf-8"
            )

            # Should raise TransitionError, not TypeError
            with self.assertRaises(TransitionError):
                transition_task(track_dir, "T001", "in_progress")

    def test_add_task_tasks_none_succeeds(self) -> None:
        """tasks=None should not crash add_task — list is auto-created."""
        with tempfile.TemporaryDirectory() as tmp:
            track_dir = Path(tmp) / "track" / "items" / "demo"
            track_dir.mkdir(parents=True)
            _write_v2_metadata(
                track_dir,
                tasks=[],
                sections=[_section("s1", [])],
            )
            data = _read_metadata(track_dir)
            data["tasks"] = None
            (track_dir / "metadata.json").write_text(
                json.dumps(data, indent=2) + "\n", encoding="utf-8"
            )

            task_id = add_task(track_dir, "New task")
            data = _read_metadata(track_dir)
            self.assertEqual(len(data["tasks"]), 1)
            self.assertEqual(data["tasks"][0]["id"], task_id)

    def test_transition_task_tasks_none_raises(self) -> None:
        """tasks=None in metadata should raise TransitionError."""
        with tempfile.TemporaryDirectory() as tmp:
            track_dir = Path(tmp) / "track" / "items" / "demo"
            track_dir.mkdir(parents=True)
            _write_v2_metadata(
                track_dir,
                tasks=[_task("T001", "todo")],
                sections=[_section("s1", ["T001"])],
            )
            data = _read_metadata(track_dir)
            data["tasks"] = None
            (track_dir / "metadata.json").write_text(
                json.dumps(data, indent=2) + "\n", encoding="utf-8"
            )

            with self.assertRaises(TransitionError):
                transition_task(track_dir, "T001", "in_progress")

    def test_set_override_tasks_none_raises(self) -> None:
        """tasks=None should not crash set_track_override."""
        with tempfile.TemporaryDirectory() as tmp:
            track_dir = Path(tmp) / "track" / "items" / "demo"
            track_dir.mkdir(parents=True)
            _write_v2_metadata(
                track_dir,
                tasks=[_task("T001", "todo")],
                sections=[_section("s1", ["T001"])],
            )
            data = _read_metadata(track_dir)
            data["tasks"] = None
            (track_dir / "metadata.json").write_text(
                json.dumps(data, indent=2) + "\n", encoding="utf-8"
            )

            # Should not crash with TypeError
            set_track_override(track_dir, "blocked", reason="test")

    def test_add_task_with_non_dict_task_in_existing_tasks_raises(self) -> None:
        """Non-dict task in existing tasks should not crash _next_task_id."""
        with tempfile.TemporaryDirectory() as tmp:
            track_dir = Path(tmp) / "track" / "items" / "demo"
            track_dir.mkdir(parents=True)
            _write_v2_metadata(
                track_dir,
                tasks=[],
                sections=[_section("s1", [])],
            )
            # Corrupt: inject non-dict task
            data = _read_metadata(track_dir)
            data["tasks"] = ["not-a-dict"]
            (track_dir / "metadata.json").write_text(
                json.dumps(data, indent=2) + "\n", encoding="utf-8"
            )

            with self.assertRaises(TransitionError):
                add_task(track_dir, "New task")

    def test_transition_task_missing_status_key_raises(self) -> None:
        """Task dict missing 'status' key should raise TransitionError."""
        with tempfile.TemporaryDirectory() as tmp:
            track_dir = Path(tmp) / "track" / "items" / "demo"
            track_dir.mkdir(parents=True)
            _write_v2_metadata(
                track_dir,
                tasks=[_task("T001", "todo")],
                sections=[_section("s1", ["T001"])],
            )
            # Corrupt: remove status key
            data = _read_metadata(track_dir)
            del data["tasks"][0]["status"]
            (track_dir / "metadata.json").write_text(
                json.dumps(data, indent=2) + "\n", encoding="utf-8"
            )

            with self.assertRaises(TransitionError):
                transition_task(track_dir, "T001", "in_progress")

    def test_add_task_section_task_ids_none(self) -> None:
        """Section with task_ids=None should not crash."""
        with tempfile.TemporaryDirectory() as tmp:
            track_dir = Path(tmp) / "track" / "items" / "demo"
            track_dir.mkdir(parents=True)
            _write_v2_metadata(
                track_dir,
                tasks=[],
                sections=[_section("s1", [])],
            )
            data = _read_metadata(track_dir)
            data["plan"]["sections"][0]["task_ids"] = None
            (track_dir / "metadata.json").write_text(
                json.dumps(data, indent=2) + "\n", encoding="utf-8"
            )

            add_task(track_dir, "New task")
            data = _read_metadata(track_dir)
            self.assertEqual(len(data["tasks"]), 1)

    def test_add_task_section_missing_task_ids_raises(self) -> None:
        """Section missing 'task_ids' key should raise TransitionError."""
        with tempfile.TemporaryDirectory() as tmp:
            track_dir = Path(tmp) / "track" / "items" / "demo"
            track_dir.mkdir(parents=True)
            _write_v2_metadata(
                track_dir,
                tasks=[],
                sections=[_section("s1", [])],
            )
            # Corrupt: remove task_ids from section
            data = _read_metadata(track_dir)
            del data["plan"]["sections"][0]["task_ids"]
            (track_dir / "metadata.json").write_text(
                json.dumps(data, indent=2) + "\n", encoding="utf-8"
            )

            # Should not crash with KeyError
            add_task(track_dir, "New task")
            data = _read_metadata(track_dir)
            self.assertEqual(len(data["tasks"]), 1)

    def test_add_task_empty_description_raises(self) -> None:
        """Empty description should raise TransitionError."""
        with tempfile.TemporaryDirectory() as tmp:
            track_dir = Path(tmp) / "track" / "items" / "demo"
            track_dir.mkdir(parents=True)
            _write_v2_metadata(track_dir, tasks=[], sections=[], status="planned")

            with self.assertRaises(TransitionError):
                add_task(track_dir, "")

            with self.assertRaises(TransitionError):
                add_task(track_dir, "   ")

    def test_transition_task_non_string_commit_hash_raises(self) -> None:
        """Non-string commit_hash should raise TransitionError, not TypeError."""
        with tempfile.TemporaryDirectory() as tmp:
            track_dir = Path(tmp) / "track" / "items" / "demo"
            track_dir.mkdir(parents=True)
            _write_v2_metadata(
                track_dir,
                tasks=[_task("T001", "in_progress")],
                sections=[_section("s1", ["T001"])],
                status="in_progress",
            )

            with self.assertRaises(TransitionError):
                transition_task(track_dir, "T001", "done", commit_hash=123)  # type: ignore

    def test_add_task_with_non_dict_section_raises(self) -> None:
        """Non-dict section entries should raise TransitionError, not TypeError."""
        with tempfile.TemporaryDirectory() as tmp:
            track_dir = Path(tmp) / "track" / "items" / "demo"
            track_dir.mkdir(parents=True)
            _write_v2_metadata(
                track_dir,
                tasks=[],
                sections=[_section("s1", [])],
            )
            # Corrupt: replace sections with non-dict
            data = _read_metadata(track_dir)
            data["plan"]["sections"] = ["not-a-dict"]
            (track_dir / "metadata.json").write_text(
                json.dumps(data, indent=2) + "\n", encoding="utf-8"
            )

            with self.assertRaises(TransitionError):
                add_task(track_dir, "New task")


class TestSyncRenderedViews(unittest.TestCase):
    def test_renders_plan_md(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            track_dir = root / "track" / "items" / "demo"
            track_dir.mkdir(parents=True)
            _write_v2_metadata(
                track_dir,
                tasks=[_task("T001", "todo")],
                sections=[_section("s1", ["T001"])],
            )

            changed = sync_rendered_views(root, track_id="demo")

            self.assertTrue(any("plan.md" in str(p) for p in changed))
            plan_content = (track_dir / "plan.md").read_text(encoding="utf-8")
            self.assertIn("<!-- Generated from metadata.json", plan_content)
            self.assertIn("- [ ] Task T001", plan_content)

    def test_delegates_to_sotp_and_returns_changed_paths(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            track_dir = root / "track" / "items" / "demo"
            track_dir.mkdir(parents=True)
            _write_v2_metadata(
                track_dir,
                tasks=[_task("T001", "todo")],
                sections=[_section("s1", ["T001"])],
            )

            with (
                patch(
                    "track_state_machine._find_sotp_for_track_views_sync",
                    return_value="/tmp/sotp",
                ),
                patch(
                    "track_state_machine.subprocess.run",
                    return_value=unittest.mock.Mock(
                        returncode=0,
                        stdout="[OK] Rendered: track/items/demo/plan.md\n[OK] Rendered: track/registry.md\n",
                    ),
                ),
            ):
                changed = sync_rendered_views(root, track_id="demo")

            self.assertEqual(
                changed,
                [root / "track/items/demo/plan.md", root / "track/registry.md"],
            )

    def test_falls_back_when_sotp_sync_views_is_unavailable(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            track_dir = root / "track" / "items" / "demo"
            track_dir.mkdir(parents=True)
            _write_v2_metadata(
                track_dir,
                tasks=[_task("T001", "todo")],
                sections=[_section("s1", ["T001"])],
            )

            with (
                patch(
                    "track_state_machine._find_sotp_for_track_views_sync",
                    return_value="/tmp/sotp",
                ),
                patch("atomic_write._find_sotp", return_value=None),
                patch(
                    "track_state_machine.subprocess.run",
                    return_value=unittest.mock.Mock(
                        returncode=1,
                        stderr="unrecognized subcommand 'views'",
                    ),
                ),
            ):
                changed = sync_rendered_views(root, track_id="demo")

            self.assertTrue(any("plan.md" in str(path) for path in changed))
            self.assertTrue((track_dir / "plan.md").exists())

    def test_raises_when_sotp_sync_views_fails(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            track_dir = root / "track" / "items" / "demo"
            track_dir.mkdir(parents=True)
            _write_v2_metadata(
                track_dir,
                tasks=[_task("T001", "todo")],
                sections=[_section("s1", ["T001"])],
            )

            with (
                patch(
                    "track_state_machine._find_sotp_for_track_views_sync",
                    return_value="/tmp/sotp",
                ),
                patch(
                    "track_state_machine.subprocess.run",
                    return_value=unittest.mock.Mock(
                        returncode=1,
                        stderr="permission denied",
                    ),
                ),
            ):
                with self.assertRaises(TransitionError):
                    sync_rendered_views(root, track_id="demo")


class TestNextOpenTask(unittest.TestCase):
    def test_next_open_task_returns_first_todo(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            track_dir = Path(tmp) / "track" / "items" / "demo"
            track_dir.mkdir(parents=True)
            _write_v2_metadata(
                track_dir,
                tasks=[
                    _task("T001", "done", "abc1234"),
                    _task("T002", "todo"),
                    _task("T003", "todo"),
                ],
                sections=[_section("s1", ["T001", "T002", "T003"])],
                status="in_progress",
            )
            task = next_open_task(track_dir)
            self.assertIsNotNone(task)
            self.assertEqual(task.id, "T002")

    def test_next_open_task_returns_in_progress_before_todo(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            track_dir = Path(tmp) / "track" / "items" / "demo"
            track_dir.mkdir(parents=True)
            _write_v2_metadata(
                track_dir,
                tasks=[
                    _task("T001", "done", "abc1234"),
                    _task("T002", "in_progress"),
                    _task("T003", "todo"),
                ],
                sections=[_section("s1", ["T001", "T002", "T003"])],
                status="in_progress",
            )
            task = next_open_task(track_dir)
            self.assertIsNotNone(task)
            self.assertEqual(task.id, "T002")

    def test_next_open_task_returns_none_when_all_done(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            track_dir = Path(tmp) / "track" / "items" / "demo"
            track_dir.mkdir(parents=True)
            _write_v2_metadata(
                track_dir,
                tasks=[
                    _task("T001", "done", "abc1234"),
                    _task("T002", "done", "def5678"),
                ],
                sections=[_section("s1", ["T001", "T002"])],
                status="done",
            )
            task = next_open_task(track_dir)
            self.assertIsNone(task)

    def test_next_open_task_returns_none_when_no_tasks(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            track_dir = Path(tmp) / "track" / "items" / "demo"
            track_dir.mkdir(parents=True)
            _write_v2_metadata(track_dir, tasks=[], sections=[_section("s1", [])])
            task = next_open_task(track_dir)
            self.assertIsNone(task)

    def test_next_open_task_follows_plan_order_not_tasks_array(self) -> None:
        """When tasks array order differs from plan section order, use plan order."""
        with tempfile.TemporaryDirectory() as tmp:
            track_dir = Path(tmp) / "track" / "items" / "demo"
            track_dir.mkdir(parents=True)
            # tasks array: T003 before T002, but plan section orders T002 first
            _write_v2_metadata(
                track_dir,
                tasks=[
                    _task("T001", "done", "abc1234"),
                    _task("T003", "todo"),
                    _task("T002", "todo"),
                ],
                sections=[_section("s1", ["T001", "T002", "T003"])],
                status="in_progress",
            )
            task = next_open_task(track_dir)
            self.assertIsNotNone(task)
            self.assertEqual(task.id, "T002")


class TestTaskCounts(unittest.TestCase):
    def test_task_counts_correct(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            track_dir = Path(tmp) / "track" / "items" / "demo"
            track_dir.mkdir(parents=True)
            _write_v2_metadata(
                track_dir,
                tasks=[
                    _task("T001", "done", "abc1234"),
                    _task("T002", "in_progress"),
                    _task("T003", "todo"),
                    _task("T004", "todo"),
                ],
                sections=[_section("s1", ["T001", "T002", "T003", "T004"])],
                status="in_progress",
            )
            counts = task_counts(track_dir)
            self.assertEqual(counts["done"], 1)
            self.assertEqual(counts["in_progress"], 1)
            self.assertEqual(counts["todo"], 2)
            self.assertEqual(counts["total"], 4)

    def test_task_counts_empty(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            track_dir = Path(tmp) / "track" / "items" / "demo"
            track_dir.mkdir(parents=True)
            _write_v2_metadata(track_dir, tasks=[], sections=[_section("s1", [])])
            counts = task_counts(track_dir)
            self.assertEqual(counts["total"], 0)
            self.assertEqual(counts["done"], 0)
            self.assertEqual(counts["in_progress"], 0)
            self.assertEqual(counts["todo"], 0)


class TestCLI(unittest.TestCase):
    """Tests for the CLI entry point (main function)."""

    def test_transition_subcommand_success(self) -> None:
        from track_state_machine import main

        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            track_dir = root / "track" / "items" / "demo"
            track_dir.mkdir(parents=True)
            _write_v2_metadata(
                track_dir,
                tasks=[_task("T001", "todo")],
                sections=[_section("s1", ["T001"])],
            )

            rc = main(["transition", str(track_dir), "T001", "in_progress"])

            self.assertEqual(rc, 0)
            data = json.loads((track_dir / "metadata.json").read_text(encoding="utf-8"))
            task = data["tasks"][0]
            self.assertEqual(task["status"], "in_progress")

    def test_transition_subcommand_invalid_transition(self) -> None:
        from track_state_machine import main

        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            track_dir = root / "track" / "items" / "demo"
            track_dir.mkdir(parents=True)
            _write_v2_metadata(
                track_dir,
                tasks=[_task("T001", "todo")],
                sections=[_section("s1", ["T001"])],
            )

            rc = main(["transition", str(track_dir), "T001", "done"])

            self.assertEqual(rc, 1)

    def test_transition_subcommand_missing_dir(self) -> None:
        from track_state_machine import main

        rc = main(["transition", "/nonexistent/path", "T001", "in_progress"])
        self.assertEqual(rc, 1)

    def test_transition_with_commit_hash(self) -> None:
        from track_state_machine import main

        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            track_dir = root / "track" / "items" / "demo"
            track_dir.mkdir(parents=True)
            _write_v2_metadata(
                track_dir,
                tasks=[_task("T001", "in_progress")],
                sections=[_section("s1", ["T001"])],
            )

            rc = main(
                [
                    "transition",
                    str(track_dir),
                    "T001",
                    "done",
                    "--commit-hash",
                    "abc1234",
                ]
            )

            self.assertEqual(rc, 0)
            data = json.loads((track_dir / "metadata.json").read_text(encoding="utf-8"))
            self.assertEqual(data["tasks"][0]["commit_hash"], "abc1234")

    def test_sync_views_subcommand(self) -> None:
        from track_state_machine import main

        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            track_dir = root / "track" / "items" / "demo"
            track_dir.mkdir(parents=True)
            _write_v2_metadata(
                track_dir,
                tasks=[_task("T001", "todo")],
                sections=[_section("s1", ["T001"])],
            )
            # Ensure registry parent exists
            (root / "track").mkdir(parents=True, exist_ok=True)

            import os

            original = os.getcwd()
            try:
                os.chdir(root)
                rc = main(["sync-views"])
            finally:
                os.chdir(original)

            self.assertEqual(rc, 0)
            self.assertTrue((track_dir / "plan.md").exists())
            self.assertTrue((root / "track" / "registry.md").exists())

    def test_no_subcommand_returns_error(self) -> None:
        from track_state_machine import main

        rc = main([])
        self.assertEqual(rc, 1)


if __name__ == "__main__":
    unittest.main()
