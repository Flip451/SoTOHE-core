"""Tests for track_schema.py — metadata.json SSoT data model and validation."""

from __future__ import annotations

import unittest

from track_schema import (
    COMMIT_HASH_RE,
    effective_track_status,
    parse_metadata_v2,
    validate_metadata_v2,
)


def _make_task(
    id: str = "T001",
    description: str = "task",
    status: str = "todo",
    commit_hash: str | None = None,
) -> dict:
    return {
        "id": id,
        "description": description,
        "status": status,
        "commit_hash": commit_hash,
    }


def _make_section(
    id: str = "s1",
    title: str = "Section",
    description: list[str] | None = None,
    task_ids: list[str] | None = None,
) -> dict:
    return {
        "id": id,
        "title": title,
        "description": description or [],
        "task_ids": task_ids or [],
    }


def _make_valid_v2(
    *,
    tasks: list[dict] | None = None,
    sections: list[dict] | None = None,
    status: str = "planned",
    status_override: dict | None = None,
) -> dict:
    t = tasks if tasks is not None else [_make_task()]
    s = sections if sections is not None else [_make_section(task_ids=["T001"])]
    return {
        "schema_version": 2,
        "id": "demo",
        "title": "Demo Track",
        "status": status,
        "created_at": "2026-03-08T00:00:00Z",
        "updated_at": "2026-03-08T12:00:00Z",
        "status_override": status_override,
        "tasks": t,
        "plan": {"summary": [], "sections": s},
    }


class TestEffectiveTrackStatus(unittest.TestCase):
    """Test effective_track_status derivation from tasks and override."""

    def test_empty_tasks_derives_planned(self) -> None:
        data = _make_valid_v2(tasks=[], sections=[], status="planned")
        meta = parse_metadata_v2(data)
        self.assertEqual(effective_track_status(meta), "planned")

    def test_all_todo_derives_planned(self) -> None:
        data = _make_valid_v2(
            tasks=[
                _make_task("T001", status="todo"),
                _make_task("T002", status="todo"),
            ],
            sections=[_make_section(task_ids=["T001", "T002"])],
            status="planned",
        )
        meta = parse_metadata_v2(data)
        self.assertEqual(effective_track_status(meta), "planned")

    def test_any_in_progress_derives_in_progress(self) -> None:
        data = _make_valid_v2(
            tasks=[
                _make_task("T001", status="in_progress"),
                _make_task("T002", status="todo"),
            ],
            sections=[_make_section(task_ids=["T001", "T002"])],
            status="in_progress",
        )
        meta = parse_metadata_v2(data)
        self.assertEqual(effective_track_status(meta), "in_progress")

    def test_mixed_done_todo_derives_in_progress(self) -> None:
        data = _make_valid_v2(
            tasks=[
                _make_task("T001", status="done"),
                _make_task("T002", status="todo"),
            ],
            sections=[_make_section(task_ids=["T001", "T002"])],
            status="in_progress",
        )
        meta = parse_metadata_v2(data)
        self.assertEqual(effective_track_status(meta), "in_progress")

    def test_all_done_derives_done(self) -> None:
        data = _make_valid_v2(
            tasks=[
                _make_task("T001", status="done", commit_hash="abc1234"),
                _make_task("T002", status="done", commit_hash="def5678"),
            ],
            sections=[_make_section(task_ids=["T001", "T002"])],
            status="done",
        )
        meta = parse_metadata_v2(data)
        self.assertEqual(effective_track_status(meta), "done")

    def test_all_skipped_derives_done(self) -> None:
        data = _make_valid_v2(
            tasks=[
                _make_task("T001", status="skipped"),
                _make_task("T002", status="skipped"),
            ],
            sections=[_make_section(task_ids=["T001", "T002"])],
            status="done",
        )
        meta = parse_metadata_v2(data)
        self.assertEqual(effective_track_status(meta), "done")

    def test_mixed_done_skipped_derives_done(self) -> None:
        data = _make_valid_v2(
            tasks=[
                _make_task("T001", status="done", commit_hash="abc1234"),
                _make_task("T002", status="skipped"),
            ],
            sections=[_make_section(task_ids=["T001", "T002"])],
            status="done",
        )
        meta = parse_metadata_v2(data)
        self.assertEqual(effective_track_status(meta), "done")

    def test_skipped_and_todo_derives_in_progress(self) -> None:
        data = _make_valid_v2(
            tasks=[
                _make_task("T001", status="skipped"),
                _make_task("T002", status="todo"),
            ],
            sections=[_make_section(task_ids=["T001", "T002"])],
            status="in_progress",
        )
        meta = parse_metadata_v2(data)
        self.assertEqual(effective_track_status(meta), "in_progress")

    def test_override_blocked_wins(self) -> None:
        data = _make_valid_v2(
            tasks=[_make_task("T001", status="todo")],
            sections=[_make_section(task_ids=["T001"])],
            status="blocked",
            status_override={"status": "blocked", "reason": "waiting on dependency"},
        )
        meta = parse_metadata_v2(data)
        self.assertEqual(effective_track_status(meta), "blocked")

    def test_override_cancelled_wins(self) -> None:
        data = _make_valid_v2(
            tasks=[_make_task("T001", status="in_progress")],
            sections=[_make_section(task_ids=["T001"])],
            status="cancelled",
            status_override={"status": "cancelled", "reason": "no longer needed"},
        )
        meta = parse_metadata_v2(data)
        self.assertEqual(effective_track_status(meta), "cancelled")


class TestValidateMetadataV2(unittest.TestCase):
    """Test validate_metadata_v2 for schema and consistency errors."""

    def test_accepts_valid_track(self) -> None:
        data = _make_valid_v2()
        errors = validate_metadata_v2(data, track_dir_name="demo")
        self.assertEqual(errors, [])

    def test_rejects_duplicate_task_ids(self) -> None:
        data = _make_valid_v2(
            tasks=[_make_task("T001"), _make_task("T001", description="dup")],
            sections=[_make_section(task_ids=["T001"])],
        )
        errors = validate_metadata_v2(data, track_dir_name="demo")
        self.assertTrue(any("duplicate" in e.lower() for e in errors))

    def test_rejects_unknown_section_task_reference(self) -> None:
        data = _make_valid_v2(
            tasks=[_make_task("T001")],
            sections=[_make_section(task_ids=["T001", "T999"])],
        )
        errors = validate_metadata_v2(data, track_dir_name="demo")
        self.assertTrue(any("T999" in e for e in errors))

    def test_rejects_commit_hash_on_non_done_task(self) -> None:
        data = _make_valid_v2(
            tasks=[_make_task("T001", status="todo", commit_hash="abc1234")],
            sections=[_make_section(task_ids=["T001"])],
        )
        errors = validate_metadata_v2(data, track_dir_name="demo")
        self.assertTrue(any("commit_hash" in e.lower() for e in errors))

    def test_rejects_status_mismatch_with_derived(self) -> None:
        data = _make_valid_v2(
            tasks=[_make_task("T001", status="todo")],
            sections=[_make_section(task_ids=["T001"])],
            status="done",  # should be planned
        )
        errors = validate_metadata_v2(data, track_dir_name="demo")
        self.assertTrue(
            any("status" in e.lower() and "drift" in e.lower() for e in errors)
        )

    def test_rejects_id_directory_mismatch(self) -> None:
        data = _make_valid_v2()
        errors = validate_metadata_v2(data, track_dir_name="other")
        self.assertTrue(any("does not match" in e.lower() for e in errors))

    def test_rejects_missing_required_field(self) -> None:
        data = _make_valid_v2()
        del data["title"]
        errors = validate_metadata_v2(data, track_dir_name="demo")
        self.assertTrue(any("title" in e.lower() for e in errors))

    def test_accepts_skipped_task_status(self) -> None:
        data = _make_valid_v2(
            tasks=[_make_task("T001", status="skipped")],
            sections=[_make_section(task_ids=["T001"])],
            status="done",
        )
        errors = validate_metadata_v2(data, track_dir_name="demo")
        self.assertEqual(errors, [])

    def test_rejects_commit_hash_on_skipped_task(self) -> None:
        data = _make_valid_v2(
            tasks=[
                {
                    "id": "T001",
                    "description": "t",
                    "status": "skipped",
                    "commit_hash": "abc1234",
                }
            ],
            sections=[_make_section(task_ids=["T001"])],
            status="done",
        )
        errors = validate_metadata_v2(data, track_dir_name="demo")
        self.assertTrue(any("commit_hash" in e.lower() for e in errors))

    def test_rejects_invalid_task_status(self) -> None:
        data = _make_valid_v2(
            tasks=[_make_task("T001", status="unknown")],
            sections=[_make_section(task_ids=["T001"])],
        )
        errors = validate_metadata_v2(data, track_dir_name="demo")
        self.assertTrue(any("unknown" in e for e in errors))

    def test_rejects_unreferenced_task(self) -> None:
        data = _make_valid_v2(
            tasks=[_make_task("T001"), _make_task("T002")],
            sections=[_make_section(task_ids=["T001"])],  # T002 not referenced
        )
        errors = validate_metadata_v2(data, track_dir_name="demo")
        self.assertTrue(
            any("T002" in e and "not referenced" in e.lower() for e in errors)
        )

    def test_override_blocked_fails_when_all_done(self) -> None:
        data = _make_valid_v2(
            tasks=[_make_task("T001", status="done", commit_hash="abc1234")],
            sections=[_make_section(task_ids=["T001"])],
            status="blocked",
            status_override={"status": "blocked", "reason": "test"},
        )
        errors = validate_metadata_v2(data, track_dir_name="demo")
        self.assertTrue(
            any("incompatible" in e.lower() or "blocked" in e.lower() for e in errors)
        )

    def test_rejects_invalid_commit_hash_format(self) -> None:
        data = _make_valid_v2(
            tasks=[_make_task("T001", status="done", commit_hash="not-a-hash!")],
            sections=[_make_section(task_ids=["T001"])],
            status="done",
        )
        errors = validate_metadata_v2(data, track_dir_name="demo")
        self.assertTrue(
            any("commit_hash" in e.lower() and "format" in e.lower() for e in errors)
        )

    def test_rejects_duplicate_task_reference_across_sections(self) -> None:
        """Task referenced in multiple sections should fail (exactly-once rule)."""
        data = _make_valid_v2(
            tasks=[_make_task("T001")],
            sections=[
                _make_section(id="s1", task_ids=["T001"]),
                _make_section(id="s2", task_ids=["T001"]),
            ],
        )
        errors = validate_metadata_v2(data, track_dir_name="demo")
        self.assertTrue(
            any(
                "T001" in e
                and (
                    "duplicate" in e.lower()
                    or "multiple" in e.lower()
                    or "more than once" in e.lower()
                )
                for e in errors
            )
        )

    def test_non_string_commit_hash_returns_error_not_exception(self) -> None:
        """Non-string commit_hash should produce a validation error, not TypeError."""
        data = _make_valid_v2(
            tasks=[
                {"id": "T001", "description": "t", "status": "done", "commit_hash": 123}
            ],
            sections=[_make_section(task_ids=["T001"])],
            status="done",
        )
        # Must not raise TypeError
        errors = validate_metadata_v2(data, track_dir_name="demo")
        self.assertTrue(any("commit_hash" in e.lower() for e in errors))

    def test_non_dict_task_returns_error_not_exception(self) -> None:
        """Non-dict task entry should produce a validation error, not AttributeError."""
        data = _make_valid_v2()
        data["tasks"] = ["not-a-dict"]
        errors = validate_metadata_v2(data, track_dir_name="demo")
        self.assertTrue(len(errors) > 0)

    def test_non_dict_section_returns_error_not_exception(self) -> None:
        """Non-dict section entry should produce a validation error."""
        data = _make_valid_v2()
        data["plan"]["sections"] = ["not-a-dict"]
        errors = validate_metadata_v2(data, track_dir_name="demo")
        self.assertTrue(len(errors) > 0)

    def test_non_dict_override_returns_error_not_exception(self) -> None:
        """Non-dict status_override should produce a validation error."""
        data = _make_valid_v2()
        data["status_override"] = "blocked"
        errors = validate_metadata_v2(data, track_dir_name="demo")
        self.assertTrue(len(errors) > 0)

    def test_tasks_none_returns_error_not_exception(self) -> None:
        """tasks=None should produce validation errors, not TypeError."""
        data = _make_valid_v2()
        data["tasks"] = None
        errors = validate_metadata_v2(data, track_dir_name="demo")
        self.assertTrue(len(errors) > 0)

    def test_sections_none_returns_error_not_exception(self) -> None:
        """sections=None should produce validation errors, not TypeError."""
        data = _make_valid_v2()
        data["plan"]["sections"] = None
        errors = validate_metadata_v2(data, track_dir_name="demo")
        self.assertTrue(len(errors) > 0)

    def test_tasks_none_parse_returns_empty(self) -> None:
        """tasks=None should parse to empty tasks list, not TypeError."""
        data = _make_valid_v2()
        data["tasks"] = None
        meta = parse_metadata_v2(data)
        self.assertEqual(meta.tasks, [])

    def test_sections_none_parse_returns_empty(self) -> None:
        """sections=None should parse to empty sections list, not TypeError."""
        data = _make_valid_v2()
        data["plan"]["sections"] = None
        meta = parse_metadata_v2(data)
        self.assertEqual(meta.plan.sections, [])

    def test_non_dict_plan_returns_error_not_exception(self) -> None:
        """Non-dict plan field should produce a validation error, not TypeError."""
        data = _make_valid_v2()
        data["plan"] = "not-a-dict"
        errors = validate_metadata_v2(data, track_dir_name="demo")
        self.assertTrue(len(errors) > 0)

    def test_task_missing_required_keys_returns_error_not_exception(self) -> None:
        """Task dict missing 'id'/'description' should not crash parse_metadata_v2."""
        data = _make_valid_v2()
        data["tasks"] = [{"status": "todo"}]  # missing id, description
        # parse should not raise
        meta = parse_metadata_v2(data)
        self.assertEqual(len(meta.tasks), 1)

    def test_section_missing_required_keys_returns_error_not_exception(self) -> None:
        """Section dict missing 'id'/'title' should not crash parse_metadata_v2."""
        data = _make_valid_v2()
        data["plan"]["sections"] = [{"task_ids": ["T001"]}]  # missing id, title
        meta = parse_metadata_v2(data)
        self.assertEqual(len(meta.plan.sections), 1)

    def test_task_ids_non_list_int_returns_error_not_exception(self) -> None:
        """task_ids as integer should produce validation error, not TypeError."""
        data = _make_valid_v2()
        data["plan"]["sections"] = [
            {"id": "s1", "title": "Section", "description": [], "task_ids": 123}
        ]
        errors = validate_metadata_v2(data, track_dir_name="demo")
        self.assertTrue(len(errors) > 0)

    def test_rejects_non_string_title(self) -> None:
        data = _make_valid_v2()
        data["title"] = []
        errors = validate_metadata_v2(data, track_dir_name="demo")
        self.assertTrue(any("'title' must be a string" in e for e in errors))

    def test_rejects_non_string_created_at(self) -> None:
        data = _make_valid_v2()
        data["created_at"] = 123
        errors = validate_metadata_v2(data, track_dir_name="demo")
        self.assertTrue(any("'created_at' must be a string" in e for e in errors))

    def test_rejects_non_string_updated_at(self) -> None:
        data = _make_valid_v2()
        data["updated_at"] = {}
        errors = validate_metadata_v2(data, track_dir_name="demo")
        self.assertTrue(any("'updated_at' must be a string" in e for e in errors))

    def test_rejects_non_string_status(self) -> None:
        data = _make_valid_v2()
        data["status"] = 42
        errors = validate_metadata_v2(data, track_dir_name="demo")
        self.assertTrue(any("'status' must be a string" in e for e in errors))

    def test_rejects_invalid_track_status_value(self) -> None:
        data = _make_valid_v2()
        data["status"] = "unknown_status"
        errors = validate_metadata_v2(data, track_dir_name="demo")
        self.assertTrue(any("Invalid track status" in e for e in errors))

    def test_rejects_empty_title(self) -> None:
        data = _make_valid_v2()
        data["title"] = ""
        errors = validate_metadata_v2(data, track_dir_name="demo")
        self.assertTrue(any("'title' must not be empty" in e for e in errors))

    def test_rejects_empty_updated_at(self) -> None:
        data = _make_valid_v2()
        data["updated_at"] = "  "
        errors = validate_metadata_v2(data, track_dir_name="demo")
        self.assertTrue(any("'updated_at' must not be empty" in e for e in errors))

    def test_rejects_empty_task_id(self) -> None:
        data = _make_valid_v2(
            tasks=[_make_task("", description="task")],
            sections=[_make_section(task_ids=[""])],
        )
        errors = validate_metadata_v2(data, track_dir_name="demo")
        self.assertTrue(any("empty id" in e.lower() for e in errors))

    def test_rejects_empty_task_description(self) -> None:
        data = _make_valid_v2(
            tasks=[_make_task("T001", description="")],
            sections=[_make_section(task_ids=["T001"])],
        )
        errors = validate_metadata_v2(data, track_dir_name="demo")
        self.assertTrue(any("empty description" in e.lower() for e in errors))

    def test_rejects_empty_section_id(self) -> None:
        data = _make_valid_v2(
            tasks=[_make_task("T001")],
            sections=[
                {"id": "", "title": "Section", "description": [], "task_ids": ["T001"]}
            ],
        )
        errors = validate_metadata_v2(data, track_dir_name="demo")
        self.assertTrue(any("section has empty id" in e.lower() for e in errors))

    def test_rejects_empty_section_title(self) -> None:
        data = _make_valid_v2(
            tasks=[_make_task("T001")],
            sections=[
                {"id": "s1", "title": "", "description": [], "task_ids": ["T001"]}
            ],
        )
        errors = validate_metadata_v2(data, track_dir_name="demo")
        self.assertTrue(any("empty title" in e.lower() for e in errors))


class TestReservedIdWords(unittest.TestCase):
    """Track IDs containing reserved words (e.g. 'git') must be rejected."""

    def test_rejects_id_containing_git(self) -> None:
        data = _make_valid_v2()
        data["id"] = "container-git-readonly-2026-03-11"
        errors = validate_metadata_v2(
            data, track_dir_name="container-git-readonly-2026-03-11"
        )
        self.assertTrue(any("reserved segment" in e.lower() for e in errors))

    def test_rejects_id_containing_git_case_insensitive(self) -> None:
        data = _make_valid_v2()
        data["id"] = "my-Git-track"
        errors = validate_metadata_v2(data, track_dir_name="my-Git-track")
        self.assertTrue(any("reserved segment" in e.lower() for e in errors))

    def test_accepts_id_without_reserved_words(self) -> None:
        data = _make_valid_v2()
        data["id"] = "container-vcs-readonly-2026-03-11"
        errors = validate_metadata_v2(
            data, track_dir_name="container-vcs-readonly-2026-03-11"
        )
        self.assertEqual(errors, [])

    def test_accepts_id_with_git_substring_in_word(self) -> None:
        """IDs like 'legit-cleanup' should not be rejected (git is not a segment)."""
        data = _make_valid_v2()
        data["id"] = "legit-cleanup-2026-03-11"
        errors = validate_metadata_v2(
            data, track_dir_name="legit-cleanup-2026-03-11"
        )
        self.assertEqual(errors, [])


class TestCommitHashRegex(unittest.TestCase):
    def test_valid_7_char_hash(self) -> None:
        self.assertIsNotNone(COMMIT_HASH_RE.match("abc1234"))

    def test_valid_40_char_hash(self) -> None:
        self.assertIsNotNone(COMMIT_HASH_RE.match("a" * 40))

    def test_rejects_uppercase(self) -> None:
        self.assertIsNone(COMMIT_HASH_RE.match("ABC1234"))

    def test_rejects_too_short(self) -> None:
        self.assertIsNone(COMMIT_HASH_RE.match("abc123"))

    def test_rejects_non_hex(self) -> None:
        self.assertIsNone(COMMIT_HASH_RE.match("ghijklm"))


class TestArchivedStatusValidation(unittest.TestCase):
    def test_archived_with_all_done_tasks_is_valid(self) -> None:
        data = _make_valid_v2(
            status="archived",
            tasks=[_make_task("T001", status="done", commit_hash="abc1234")],
        )
        errors = validate_metadata_v2(data, track_dir_name="demo")
        self.assertEqual(errors, [])

    def test_archived_with_incomplete_tasks_is_invalid(self) -> None:
        data = _make_valid_v2(
            status="archived",
            tasks=[_make_task("T001", status="in_progress")],
        )
        errors = validate_metadata_v2(data, track_dir_name="demo")
        self.assertTrue(any("archived" in e and "done" in e for e in errors))

    def test_archived_is_valid_track_status(self) -> None:
        from track_schema import VALID_TRACK_STATUSES

        self.assertIn("archived", VALID_TRACK_STATUSES)


class TrackItemsDirConsistencyTest(unittest.TestCase):
    """Verify all production scripts use the same track/items path as track_schema.TRACK_ITEMS_DIR."""

    def test_track_items_dir_matches_production_scripts(self) -> None:
        import re
        from pathlib import Path

        from track_schema import TRACK_ITEMS_DIR

        # Production scripts that reference track/items
        scripts_with_track_items = [
            "verify_tech_stack_ready.py",
            "verify_latest_track_files.py",
            "verify_plan_progress.py",
            "verify_track_metadata.py",
            "track_registry.py",
            "track_state_machine.py",
            "external_guides.py",
        ]
        scripts_dir = Path(__file__).parent
        pattern = re.compile(r'"track"\s*/\s*"items"|["\']track/items["\']')

        for script_name in scripts_with_track_items:
            script_path = scripts_dir / script_name
            self.assertTrue(script_path.is_file(), f"Missing: {script_name}")
            content = script_path.read_text(encoding="utf-8")
            self.assertTrue(
                pattern.search(content),
                f"{script_name} does not reference 'track/items' — "
                f"canonical value is TRACK_ITEMS_DIR = {TRACK_ITEMS_DIR!r}",
            )


if __name__ == "__main__":
    unittest.main()
