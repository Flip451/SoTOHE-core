"""Tests for track_markdown.py — legacy plan.md parser and plan renderer."""

from __future__ import annotations

import unittest

from track_markdown import render_plan, summarize_plan
from track_schema import (
    TrackMetadataV2,
    parse_metadata_v2,
)

# ================================================================
# Checkbox normalization tests (legacy parser)
# ================================================================


class TestSummarizePlanNormalization(unittest.TestCase):
    """Verify that AI output variations in checkbox markers are normalized."""

    def test_standard_done_x(self) -> None:
        summary = summarize_plan("- [x] task done")
        self.assertEqual(summary.done_count, 1)
        self.assertEqual(summary.invalid_lines, [])

    def test_uppercase_done_X(self) -> None:
        summary = summarize_plan("- [X] task done")
        self.assertEqual(summary.done_count, 1)
        self.assertEqual(summary.invalid_lines, [])

    def test_space_before_x(self) -> None:
        summary = summarize_plan("- [ x] task done")
        self.assertEqual(summary.done_count, 1)
        self.assertEqual(summary.invalid_lines, [])

    def test_space_after_x(self) -> None:
        summary = summarize_plan("- [x ] task done")
        self.assertEqual(summary.done_count, 1)
        self.assertEqual(summary.invalid_lines, [])

    def test_space_before_X(self) -> None:
        summary = summarize_plan("- [ X] task done")
        self.assertEqual(summary.done_count, 1)
        self.assertEqual(summary.invalid_lines, [])

    def test_space_after_X(self) -> None:
        summary = summarize_plan("- [X ] task done")
        self.assertEqual(summary.done_count, 1)
        self.assertEqual(summary.invalid_lines, [])

    def test_space_around_x(self) -> None:
        summary = summarize_plan("- [ x ] task done")
        self.assertEqual(summary.done_count, 1)
        self.assertEqual(summary.invalid_lines, [])

    def test_standard_todo(self) -> None:
        summary = summarize_plan("- [ ] task todo")
        self.assertEqual(summary.todo_count, 1)
        self.assertEqual(summary.invalid_lines, [])

    def test_double_space_todo(self) -> None:
        summary = summarize_plan("- [  ] task todo")
        self.assertEqual(summary.todo_count, 1)
        self.assertEqual(summary.invalid_lines, [])

    def test_standard_in_progress(self) -> None:
        summary = summarize_plan("- [~] task wip")
        self.assertEqual(summary.in_progress_count, 1)
        self.assertEqual(summary.invalid_lines, [])

    def test_space_before_tilde(self) -> None:
        summary = summarize_plan("- [ ~] task wip")
        self.assertEqual(summary.in_progress_count, 1)
        self.assertEqual(summary.invalid_lines, [])

    def test_space_after_tilde(self) -> None:
        summary = summarize_plan("- [~ ] task wip")
        self.assertEqual(summary.in_progress_count, 1)
        self.assertEqual(summary.invalid_lines, [])

    def test_space_around_tilde(self) -> None:
        summary = summarize_plan("- [ ~ ] task wip")
        self.assertEqual(summary.in_progress_count, 1)
        self.assertEqual(summary.invalid_lines, [])

    def test_standard_skipped(self) -> None:
        summary = summarize_plan("- [-] task skipped")
        self.assertEqual(summary.skipped_count, 1)
        self.assertEqual(summary.invalid_lines, [])

    def test_space_before_dash(self) -> None:
        summary = summarize_plan("- [ -] task skipped")
        self.assertEqual(summary.skipped_count, 1)
        self.assertEqual(summary.invalid_lines, [])

    def test_space_after_dash(self) -> None:
        summary = summarize_plan("- [- ] task skipped")
        self.assertEqual(summary.skipped_count, 1)
        self.assertEqual(summary.invalid_lines, [])

    def test_space_around_dash(self) -> None:
        summary = summarize_plan("- [ - ] task skipped")
        self.assertEqual(summary.skipped_count, 1)
        self.assertEqual(summary.invalid_lines, [])

    def test_done_with_trailing_hash(self) -> None:
        summary = summarize_plan("- [x] task done abc1234")
        self.assertEqual(summary.done_count, 1)
        self.assertEqual(summary.invalid_lines, [])

    def test_numbered_list_task(self) -> None:
        summary = summarize_plan("1. [x] task done")
        self.assertEqual(summary.done_count, 1)

    def test_asterisk_list_task(self) -> None:
        summary = summarize_plan("* [ ] task todo")
        self.assertEqual(summary.todo_count, 1)


class TestSummarizePlanRejection(unittest.TestCase):
    """Verify that invalid checkbox states are properly rejected."""

    def test_rejects_slash(self) -> None:
        summary = summarize_plan("- [/] invalid")
        self.assertTrue(len(summary.invalid_lines) > 0)

    def test_rejects_question_mark(self) -> None:
        summary = summarize_plan("- [?] invalid")
        self.assertTrue(len(summary.invalid_lines) > 0)

    def test_rejects_multi_char_abc(self) -> None:
        summary = summarize_plan("- [abc] invalid")
        self.assertTrue(len(summary.invalid_lines) > 0)

    def test_rejects_mixed_x_tilde(self) -> None:
        summary = summarize_plan("- [x~] invalid")
        self.assertTrue(len(summary.invalid_lines) > 0)

    def test_rejects_empty_brackets(self) -> None:
        summary = summarize_plan("- [] invalid")
        self.assertTrue(len(summary.invalid_lines) > 0)

    def test_rejects_triple_space(self) -> None:
        summary = summarize_plan("- [   ] invalid")
        self.assertTrue(len(summary.invalid_lines) > 0)

    def test_rejects_missing_body(self) -> None:
        summary = summarize_plan("- [x]")
        self.assertTrue(len(summary.invalid_lines) > 0)


class TestSummarizePlanEdgeCases(unittest.TestCase):
    """Edge cases: non-task lines, empty plan, aggregate derivation."""

    def test_ignores_non_task_lines(self) -> None:
        text = "# Heading\nSome text with [brackets] inside.\n- [x] real task"
        summary = summarize_plan(text)
        self.assertEqual(summary.done_count, 1)
        self.assertEqual(summary.total_tasks, 1)

    def test_empty_plan(self) -> None:
        summary = summarize_plan("")
        self.assertEqual(summary.total_tasks, 0)
        self.assertEqual(summary.aggregate_status, "planned")

    def test_no_task_lines(self) -> None:
        summary = summarize_plan("# Plan\nJust some text.")
        self.assertEqual(summary.total_tasks, 0)
        self.assertEqual(summary.aggregate_status, "planned")

    def test_aggregate_planned(self) -> None:
        summary = summarize_plan("- [ ] a\n- [ ] b")
        self.assertEqual(summary.aggregate_status, "planned")

    def test_aggregate_in_progress(self) -> None:
        summary = summarize_plan("- [~] a\n- [ ] b")
        self.assertEqual(summary.aggregate_status, "in_progress")

    def test_aggregate_done(self) -> None:
        summary = summarize_plan("- [x] a\n- [X] b")
        self.assertEqual(summary.aggregate_status, "done")

    def test_aggregate_mixed_done_todo(self) -> None:
        summary = summarize_plan("- [x] a\n- [ ] b")
        self.assertEqual(summary.aggregate_status, "in_progress")

    def test_aggregate_done_with_skipped(self) -> None:
        summary = summarize_plan("- [x] a\n- [-] b")
        self.assertEqual(summary.aggregate_status, "done")

    def test_aggregate_all_skipped(self) -> None:
        summary = summarize_plan("- [-] a\n- [-] b")
        self.assertEqual(summary.aggregate_status, "done")

    def test_aggregate_skipped_and_todo(self) -> None:
        summary = summarize_plan("- [-] a\n- [ ] b")
        self.assertEqual(summary.aggregate_status, "in_progress")

    def test_aggregate_none_on_invalid(self) -> None:
        summary = summarize_plan("- [/] bad")
        self.assertIsNone(summary.aggregate_status)

    def test_in_progress_counting_with_normalized_variants(self) -> None:
        text = "- [~] a\n- [ ~] b\n- [~ ] c"
        summary = summarize_plan(text)
        self.assertEqual(summary.in_progress_count, 3)


# ================================================================
# Plan renderer tests
# ================================================================


def _make_metadata_v2(
    *,
    tasks: list[dict] | None = None,
    sections: list[dict] | None = None,
    summary: list[str] | None = None,
) -> TrackMetadataV2:
    t = tasks or [
        {"id": "T001", "description": "task one", "status": "todo", "commit_hash": None}
    ]
    s = sections or [
        {"id": "s1", "title": "Tasks", "description": [], "task_ids": ["T001"]}
    ]
    data = {
        "schema_version": 2,
        "id": "demo",
        "title": "Demo Track",
        "status": "planned",
        "created_at": "2026-03-08T00:00:00Z",
        "updated_at": "2026-03-08T12:00:00Z",
        "status_override": None,
        "tasks": t,
        "plan": {"summary": summary or [], "sections": s},
    }
    return parse_metadata_v2(data)


class TestRenderPlan(unittest.TestCase):
    """Test plan.md rendering from metadata.json."""

    def test_deterministic_output(self) -> None:
        meta = _make_metadata_v2()
        result1 = render_plan(meta)
        result2 = render_plan(meta)
        self.assertEqual(result1, result2)

    def test_contains_generated_header(self) -> None:
        meta = _make_metadata_v2()
        result = render_plan(meta)
        self.assertIn("<!-- Generated from metadata.json", result)

    def test_renders_todo_marker(self) -> None:
        meta = _make_metadata_v2(
            tasks=[
                {
                    "id": "T001",
                    "description": "task",
                    "status": "todo",
                    "commit_hash": None,
                }
            ],
            sections=[
                {"id": "s1", "title": "Tasks", "description": [], "task_ids": ["T001"]}
            ],
        )
        result = render_plan(meta)
        self.assertIn("- [ ] task", result)

    def test_renders_in_progress_marker(self) -> None:
        meta = _make_metadata_v2(
            tasks=[
                {
                    "id": "T001",
                    "description": "wip",
                    "status": "in_progress",
                    "commit_hash": None,
                }
            ],
            sections=[
                {"id": "s1", "title": "Tasks", "description": [], "task_ids": ["T001"]}
            ],
        )
        result = render_plan(meta)
        self.assertIn("- [~] wip", result)

    def test_renders_done_marker_with_commit_hash(self) -> None:
        meta = _make_metadata_v2(
            tasks=[
                {
                    "id": "T001",
                    "description": "done task",
                    "status": "done",
                    "commit_hash": "abc1234",
                }
            ],
            sections=[
                {"id": "s1", "title": "Tasks", "description": [], "task_ids": ["T001"]}
            ],
        )
        result = render_plan(meta)
        self.assertIn("- [x] done task abc1234", result)

    def test_renders_done_marker_without_commit_hash(self) -> None:
        meta = _make_metadata_v2(
            tasks=[
                {
                    "id": "T001",
                    "description": "done task",
                    "status": "done",
                    "commit_hash": None,
                }
            ],
            sections=[
                {"id": "s1", "title": "Tasks", "description": [], "task_ids": ["T001"]}
            ],
        )
        result = render_plan(meta)
        self.assertIn("- [x] done task", result)
        self.assertNotIn("None", result)

    def test_renders_skipped_marker(self) -> None:
        meta = _make_metadata_v2(
            tasks=[
                {
                    "id": "T001",
                    "description": "skipped task",
                    "status": "skipped",
                    "commit_hash": None,
                }
            ],
            sections=[
                {"id": "s1", "title": "Tasks", "description": [], "task_ids": ["T001"]}
            ],
        )
        result = render_plan(meta)
        self.assertIn("- [-] skipped task", result)

    def test_renders_multiple_sections(self) -> None:
        meta = _make_metadata_v2(
            tasks=[
                {
                    "id": "T001",
                    "description": "design task",
                    "status": "done",
                    "commit_hash": None,
                },
                {
                    "id": "T002",
                    "description": "impl task",
                    "status": "todo",
                    "commit_hash": None,
                },
            ],
            sections=[
                {
                    "id": "design",
                    "title": "Design",
                    "description": ["Design phase."],
                    "task_ids": ["T001"],
                },
                {
                    "id": "impl",
                    "title": "Implementation",
                    "description": [],
                    "task_ids": ["T002"],
                },
            ],
        )
        result = render_plan(meta)
        self.assertIn("## Design", result)
        self.assertIn("## Implementation", result)
        design_pos = result.index("## Design")
        impl_pos = result.index("## Implementation")
        self.assertLess(design_pos, impl_pos)

    def test_renders_summary(self) -> None:
        meta = _make_metadata_v2(summary=["This is the plan summary."])
        result = render_plan(meta)
        self.assertIn("This is the plan summary.", result)

    def test_empty_tasks_renders_minimal(self) -> None:
        meta = _make_metadata_v2(tasks=[], sections=[])
        result = render_plan(meta)
        self.assertIn("<!-- Generated from metadata.json", result)

    def test_section_description_rendered(self) -> None:
        meta = _make_metadata_v2(
            tasks=[
                {
                    "id": "T001",
                    "description": "task",
                    "status": "todo",
                    "commit_hash": None,
                }
            ],
            sections=[
                {
                    "id": "s1",
                    "title": "Phase 1",
                    "description": ["Core work here."],
                    "task_ids": ["T001"],
                }
            ],
        )
        result = render_plan(meta)
        self.assertIn("Core work here.", result)


if __name__ == "__main__":
    unittest.main()
