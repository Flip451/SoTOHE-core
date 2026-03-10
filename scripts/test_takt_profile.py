import io
import json
import subprocess
import tempfile
import unittest
from contextlib import redirect_stderr, redirect_stdout
from pathlib import Path
from unittest import mock

import scripts.takt_profile as takt_profile

PROJECT_ROOT = Path(__file__).resolve().parent.parent


class TaktProfileTest(unittest.TestCase):
    def copy_fixture_tree(self, root: Path, active_profile: str = "default") -> None:
        hooks_dir = root / ".claude" / "hooks"
        hooks_dir.mkdir(parents=True, exist_ok=True)
        (hooks_dir / "_agent_profiles.py").write_text(
            (PROJECT_ROOT / ".claude" / "hooks" / "_agent_profiles.py").read_text(
                encoding="utf-8"
            ),
            encoding="utf-8",
        )

        profiles = json.loads(
            (PROJECT_ROOT / ".claude" / "agent-profiles.json").read_text(
                encoding="utf-8"
            )
        )
        profiles["active_profile"] = active_profile
        (root / ".claude" / "agent-profiles.json").parent.mkdir(
            parents=True, exist_ok=True
        )
        (root / ".claude" / "agent-profiles.json").write_text(
            json.dumps(profiles),
            encoding="utf-8",
        )

        source_personas = PROJECT_ROOT / ".takt" / "personas"
        target_personas = root / ".takt" / "personas"
        target_personas.mkdir(parents=True, exist_ok=True)
        for source_path in source_personas.glob("*.md"):
            (target_personas / source_path.name).write_text(
                source_path.read_text(encoding="utf-8"),
                encoding="utf-8",
            )
        (root / ".takt" / "tasks.yaml").write_text("tasks: []\n", encoding="utf-8")

    def write_single_queue_task(
        self,
        root: Path,
        *,
        status: str = "pending",
        failure_streak: int | None = None,
    ) -> None:
        lines = [
            "tasks:",
            "  - worktree: true",
            "    piece: full-cycle",
            "    auto_pr: true",
            "    draft_pr: true",
            "    name: sample-queue-task",
            f"    status: {status}",
            "    slug: sample-queue-task",
            "    summary: sample queue task",
            "    task_dir: .takt/tasks/20260304-164008-sample-queue-task",
            "    created_at: 2026-03-04T16:40:08.902Z",
            "    started_at: null",
            "    completed_at: null",
            "    owner_pid: null",
            "    agent_profile_name: default",
            "    agent_profile_version: 1",
            "    agent_orchestrator: claude",
            "    agent_planner: codex",
            "    agent_researcher: gemini",
            "    agent_implementer: claude",
            "    agent_reviewer: codex",
            "    agent_debugger: codex",
            "    agent_multimodal_reader: gemini",
            "    agent_takt_host_provider: claude",
            "    agent_takt_host_model: claude-opus-4-6",
        ]
        if failure_streak is not None:
            lines.append(f"    agent_failure_streak: {failure_streak}")
        (root / ".takt" / "tasks.yaml").write_text(
            "\n".join(lines) + "\n",
            encoding="utf-8",
        )

    def test_default_host_values_follow_active_profile(self) -> None:
        self.assertEqual(takt_profile.host_provider(), "claude")
        self.assertEqual(takt_profile.host_model(), "claude-opus-4-6")
        self.assertEqual(takt_profile.host_label(), "Claude Code")

    # ------------------------------------------------------------------
    # extract_first_json_object: helper extraction behavior
    # ------------------------------------------------------------------

    def test_extract_first_json_object_strict_clean_json(self) -> None:
        obj = takt_profile.extract_first_json_object('{"key": "value"}')
        self.assertEqual(obj, {"key": "value"})

    def test_extract_first_json_object_strict_rejects_non_object_json(self) -> None:
        # Strict parse returns None for a non-object; scan also finds nothing dict-shaped.
        obj = takt_profile.extract_first_json_object("[1, 2, 3]")
        self.assertIsNone(obj)

    def test_extract_first_json_object_noisy_prefix_still_finds_object(self) -> None:
        obj = takt_profile.extract_first_json_object(
            'some noise {"key": "value"} trailing'
        )
        self.assertEqual(obj, {"key": "value"})

    def test_extract_first_json_object_braces_in_string_value(self) -> None:
        obj = takt_profile.extract_first_json_object('{"key": "val { with braces }"}')
        self.assertEqual(obj, {"key": "val { with braces }"})

    def test_extract_first_json_object_empty_input_returns_none(self) -> None:
        self.assertIsNone(takt_profile.extract_first_json_object(""))
        self.assertIsNone(takt_profile.extract_first_json_object("   "))

    # ------------------------------------------------------------------
    # parse_loop_analysis_result: strict branch (clean JSON output)
    # ------------------------------------------------------------------

    def test_parse_loop_analysis_result_strict_loop_detected_true(self) -> None:
        parsed = takt_profile.parse_loop_analysis_result(
            '{"loop_detected": true, "confidence": 0.9, "rationale": "repeated failure"}'
        )

        self.assertEqual(parsed["decision"], takt_profile.LOOP_ANALYSIS_DECISION_LOOP)
        self.assertEqual(parsed["confidence"], 0.9)
        self.assertEqual(parsed["rationale"], "repeated failure")

    def test_parse_loop_analysis_result_strict_loop_detected_false(self) -> None:
        parsed = takt_profile.parse_loop_analysis_result(
            '{"loop_detected": false, "confidence": 0.4, "rationale": "transient error"}'
        )

        self.assertEqual(
            parsed["decision"], takt_profile.LOOP_ANALYSIS_DECISION_TRANSIENT
        )
        self.assertEqual(parsed["confidence"], 0.4)

    def test_parse_loop_analysis_result_strict_missing_loop_detected_field(
        self,
    ) -> None:
        parsed = takt_profile.parse_loop_analysis_result(
            '{"decision": "loop", "confidence": 0.9}'
        )

        self.assertEqual(
            parsed["decision"], takt_profile.LOOP_ANALYSIS_DECISION_INVALID
        )
        self.assertIn("loop_detected", parsed["rationale"])

    # ------------------------------------------------------------------
    # parse_loop_analysis_result: invalid branch (JSON-only contract)
    # ------------------------------------------------------------------

    def test_parse_loop_analysis_result_rejects_noisy_prefix_and_valid_payload_as_invalid(
        self,
    ) -> None:
        parsed = takt_profile.parse_loop_analysis_result(
            'preface {"note":"hello"} then {"loop_detected": true, "confidence": 0.9, "rationale": "loop"}'
        )

        self.assertEqual(
            parsed["decision"], takt_profile.LOOP_ANALYSIS_DECISION_INVALID
        )
        self.assertIn("JSON-only contract", parsed["rationale"])

    def test_parse_loop_analysis_result_handles_braces_inside_json_strings(
        self,
    ) -> None:
        # Acceptance condition: braces inside string values in strict JSON remain valid.
        parsed = takt_profile.parse_loop_analysis_result(
            '{"loop_detected": false, "confidence": 0.7, "rationale": "x { y"}'
        )

        self.assertEqual(
            parsed["decision"], takt_profile.LOOP_ANALYSIS_DECISION_TRANSIENT
        )
        self.assertEqual(parsed["confidence"], 0.7)
        self.assertEqual(parsed["rationale"], "x { y")

    def test_parse_loop_analysis_result_noisy_json_without_loop_detected_returns_invalid(
        self,
    ) -> None:
        parsed = takt_profile.parse_loop_analysis_result(
            'Here is the result: {"note": "done"} end'
        )

        self.assertEqual(
            parsed["decision"], takt_profile.LOOP_ANALYSIS_DECISION_INVALID
        )
        self.assertIn("JSON-only contract", parsed["rationale"])

    def test_parse_loop_analysis_result_string_loop_detected_returns_invalid(
        self,
    ) -> None:
        parsed = takt_profile.parse_loop_analysis_result(
            '{"loop_detected": "true", "confidence": 0.9, "rationale": "loop"}'
        )

        self.assertEqual(
            parsed["decision"], takt_profile.LOOP_ANALYSIS_DECISION_INVALID
        )
        self.assertIn("loop_detected", parsed["rationale"])

    def test_parse_loop_analysis_result_malformed_json_returns_invalid(self) -> None:
        parsed = takt_profile.parse_loop_analysis_result(
            '{"loop_detected": true, "confidence": 0.9, "rationale": "loop"'
        )

        self.assertEqual(
            parsed["decision"], takt_profile.LOOP_ANALYSIS_DECISION_INVALID
        )
        self.assertIn("looked like JSON", parsed["rationale"])

    # ------------------------------------------------------------------
    # parse_loop_analysis_result: failure branch (no JSON at all)
    # ------------------------------------------------------------------

    def test_parse_loop_analysis_result_no_json_returns_unknown(self) -> None:
        parsed = takt_profile.parse_loop_analysis_result("no json here at all")

        self.assertEqual(
            parsed["decision"], takt_profile.LOOP_ANALYSIS_DECISION_UNKNOWN
        )
        self.assertIn("did not contain valid JSON", parsed["rationale"])

    def test_parse_loop_analysis_result_empty_returns_unknown(self) -> None:
        parsed = takt_profile.parse_loop_analysis_result("")

        self.assertEqual(
            parsed["decision"], takt_profile.LOOP_ANALYSIS_DECISION_UNKNOWN
        )

    def test_run_queue_blocks_task_when_analysis_returns_invalid_response(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = Path(tmp_dir)
            self.copy_fixture_tree(root)
            self.write_single_queue_task(root, failure_streak=2)

            stderr = io.StringIO()
            with mock.patch.object(takt_profile, "project_root", return_value=root):
                with mock.patch.object(takt_profile, "render_personas"):
                    with mock.patch(
                        "scripts.takt_profile.analyze_loop_with_researcher",
                        return_value={
                            "provider": "gemini",
                            "decision": takt_profile.LOOP_ANALYSIS_DECISION_INVALID,
                            "confidence": 0.0,
                            "rationale": "Loop analysis response violated the JSON-only contract.",
                        },
                    ):
                        with mock.patch.object(
                            takt_profile,
                            "circuit_breaker_failure_limit",
                            return_value=3,
                        ):
                            with mock.patch(
                                "scripts.takt_profile.subprocess.call", return_value=1
                            ):
                                with redirect_stderr(stderr):
                                    code = takt_profile.run_queue()

            self.assertEqual(code, 1)
            tasks_text = (root / ".takt" / "tasks.yaml").read_text(encoding="utf-8")
            self.assertIn("status: blocked", tasks_text)
            self.assertIn("agent_loop_analysis_decision: invalid_response", tasks_text)
            parsed_tasks = takt_profile.parse_tasks_file(root)
            self.assertEqual(parsed_tasks[0]["agent_failure_streak"], "3")
            self.assertEqual(
                parsed_tasks[0]["agent_circuit_breaker_reason"],
                "Circuit breaker opened because loop analysis returned an invalid JSON-only response at the failure threshold.",
            )
            combined = stderr.getvalue()
            self.assertIn(
                "Loop analysis via gemini: decision=invalid_response, confidence=0.00",
                combined,
            )
            self.assertIn(
                "Circuit breaker opened for 'sample-queue-task' because loop analysis returned an invalid JSON-only response (3 consecutive failures).",
                combined,
            )

    def test_render_personas_replaces_profile_placeholders(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = Path(tmp_dir)
            self.copy_fixture_tree(root)

            written = []
            with mock.patch.object(takt_profile, "project_root", return_value=root):
                written = takt_profile.render_personas()

            self.assertTrue(written)
            debugger = (
                root / ".takt" / "runtime" / "personas" / "rust-debugger.md"
            ).read_text(encoding="utf-8")
            self.assertIn("Codex CLI", debugger)
            self.assertIn("Gemini CLI", debugger)
            self.assertIn("claude-opus-4-6", debugger)
            self.assertNotIn("{{", debugger)

            note_writer = (
                root / ".takt" / "runtime" / "personas" / "note-writer.md"
            ).read_text(encoding="utf-8")
            self.assertIn("structured git note", note_writer)
            self.assertNotIn("{{", note_writer)

            planner = (
                root / ".takt" / "runtime" / "personas" / "rust-planner.md"
            ).read_text(encoding="utf-8")
            self.assertIn("# Implementation Plan:", planner)
            self.assertIn("flowchart TD", planner)
            self.assertIn("Do not use ASCII box art", planner)

            implementer = (
                root / ".takt" / "runtime" / "personas" / "rust-implementer.md"
            ).read_text(encoding="utf-8")
            self.assertIn("cargo make tools-up", implementer)
            self.assertIn("Cargo.lock", implementer)

    def test_codex_heavy_profile_changes_host_and_debugger_render(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = Path(tmp_dir)
            self.copy_fixture_tree(root, active_profile="codex-heavy")

            with mock.patch.object(takt_profile, "project_root", return_value=root):
                takt_profile.render_personas()
                provider = takt_profile.host_provider()
                model = takt_profile.host_model()

            self.assertEqual(provider, "codex")
            self.assertEqual(model, "gpt-5.4")

            debugger = (
                root / ".takt" / "runtime" / "personas" / "rust-debugger.md"
            ).read_text(encoding="utf-8")
            self.assertIn("This takt run is hosted by Codex CLI", debugger)
            self.assertIn(
                "The active profile routes debugger work to Codex CLI", debugger
            )
            self.assertIn("external research to Codex CLI", debugger)

    def test_run_piece_invokes_takt_with_host_override(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = Path(tmp_dir)
            self.copy_fixture_tree(root)

            with mock.patch.object(takt_profile, "project_root", return_value=root):
                with mock.patch.object(
                    takt_profile, "render_personas"
                ) as render_personas:
                    with mock.patch(
                        "scripts.takt_profile.subprocess.call", return_value=0
                    ) as call:
                        code = takt_profile.run_piece("full-cycle", "task summary")

            self.assertEqual(code, 0)
            render_personas.assert_called_once()
            call.assert_called_once_with(
                [
                    "takt",
                    "--provider",
                    "claude",
                    "--model",
                    "claude-opus-4-6",
                    "--piece",
                    "full-cycle",
                    "--task",
                    "task summary",
                    "--skip-git",
                    "--pipeline",
                ],
                cwd=root,
            )

    def test_add_task_snapshots_active_profile_onto_last_queue_entry(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = Path(tmp_dir)
            self.copy_fixture_tree(root)

            def fake_call(command, cwd):
                self.assertEqual(command, ["takt", "add", "sample queue task"])
                self.assertEqual(cwd, root)
                (root / ".takt" / "tasks.yaml").write_text(
                    "\n".join(
                        [
                            "tasks:",
                            "  - worktree: true",
                            "    piece: full-cycle",
                            "    auto_pr: true",
                            "    draft_pr: true",
                            "    name: sample-queue-task",
                            "    status: pending",
                            "    slug: sample-queue-task",
                            "    summary: sample queue task",
                            "    task_dir: .takt/tasks/20260304-164008-sample-queue-task",
                            "    created_at: 2026-03-04T16:40:08.902Z",
                            "    started_at: null",
                            "    completed_at: null",
                            "    owner_pid: null",
                        ]
                    )
                    + "\n",
                    encoding="utf-8",
                )
                return 0

            with mock.patch.object(takt_profile, "project_root", return_value=root):
                with mock.patch(
                    "scripts.takt_profile.subprocess.call", side_effect=fake_call
                ):
                    code = takt_profile.add_task("sample queue task")

            self.assertEqual(code, 0)
            tasks_text = (root / ".takt" / "tasks.yaml").read_text(encoding="utf-8")
            self.assertIn("agent_profile_name: default", tasks_text)
            self.assertIn("agent_planner: codex", tasks_text)
            self.assertIn("agent_researcher: gemini", tasks_text)
            self.assertIn("agent_takt_host_provider: claude", tasks_text)
            self.assertIn("agent_takt_host_model: claude-opus-4-6", tasks_text)

    def test_run_queue_uses_snapshotted_profile_when_current_profile_differs(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = Path(tmp_dir)
            self.copy_fixture_tree(root, active_profile="codex-heavy")
            (root / ".takt" / "tasks.yaml").write_text(
                "\n".join(
                    [
                        "tasks:",
                        "  - worktree: true",
                        "    piece: full-cycle",
                        "    auto_pr: true",
                        "    draft_pr: true",
                        "    name: sample-queue-task",
                        "    status: pending",
                        "    slug: sample-queue-task",
                        "    summary: sample queue task",
                        "    task_dir: .takt/tasks/20260304-164008-sample-queue-task",
                        "    created_at: 2026-03-04T16:40:08.902Z",
                        "    started_at: null",
                        "    completed_at: null",
                        "    owner_pid: null",
                        "    agent_profile_name: default",
                        "    agent_profile_version: 1",
                        "    agent_orchestrator: claude",
                        "    agent_planner: codex",
                        "    agent_researcher: gemini",
                        "    agent_implementer: claude",
                        "    agent_reviewer: codex",
                        "    agent_debugger: codex",
                        "    agent_multimodal_reader: gemini",
                        "    agent_takt_host_provider: claude",
                        "    agent_takt_host_model: claude-opus-4-6",
                    ]
                )
                + "\n",
                encoding="utf-8",
            )

            with mock.patch.object(takt_profile, "project_root", return_value=root):
                with mock.patch.object(
                    takt_profile, "render_personas"
                ) as render_personas:
                    with mock.patch(
                        "scripts.takt_profile.subprocess.call", return_value=0
                    ) as call:
                        code = takt_profile.run_queue()

            self.assertEqual(code, 0)
            render_personas.assert_called_once()
            rendered_profiles = render_personas.call_args.kwargs["profiles"]
            self.assertEqual(
                rendered_profiles["active_profile"], "__takt_queue_snapshot__"
            )
            self.assertEqual(
                rendered_profiles["profiles"]["__takt_queue_snapshot__"][
                    "takt_host_provider"
                ],
                "claude",
            )
            call.assert_called_once_with(
                [
                    "takt",
                    "--provider",
                    "claude",
                    "--model",
                    "claude-opus-4-6",
                    "run",
                ],
                cwd=root,
            )

    def test_run_queue_rejects_mixed_snapshots(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = Path(tmp_dir)
            self.copy_fixture_tree(root)
            (root / ".takt" / "tasks.yaml").write_text(
                "\n".join(
                    [
                        "tasks:",
                        "  - worktree: true",
                        "    piece: full-cycle",
                        "    auto_pr: true",
                        "    draft_pr: true",
                        "    name: task-a",
                        "    status: pending",
                        "    slug: task-a",
                        "    summary: task a",
                        "    task_dir: .takt/tasks/a",
                        "    created_at: 2026-03-04T16:40:08.902Z",
                        "    started_at: null",
                        "    completed_at: null",
                        "    owner_pid: null",
                        "    agent_profile_name: default",
                        "    agent_profile_version: 1",
                        "    agent_orchestrator: claude",
                        "    agent_planner: codex",
                        "    agent_researcher: gemini",
                        "    agent_implementer: claude",
                        "    agent_reviewer: codex",
                        "    agent_debugger: codex",
                        "    agent_multimodal_reader: gemini",
                        "    agent_takt_host_provider: claude",
                        "    agent_takt_host_model: claude-opus-4-6",
                        "  - worktree: true",
                        "    piece: full-cycle",
                        "    auto_pr: true",
                        "    draft_pr: true",
                        "    name: task-b",
                        "    status: pending",
                        "    slug: task-b",
                        "    summary: task b",
                        "    task_dir: .takt/tasks/b",
                        "    created_at: 2026-03-04T16:40:09.902Z",
                        "    started_at: null",
                        "    completed_at: null",
                        "    owner_pid: null",
                        "    agent_profile_name: codex-heavy",
                        "    agent_profile_version: 1",
                        "    agent_orchestrator: claude",
                        "    agent_planner: codex",
                        "    agent_researcher: codex",
                        "    agent_implementer: codex",
                        "    agent_reviewer: codex",
                        "    agent_debugger: codex",
                        "    agent_multimodal_reader: gemini",
                        "    agent_takt_host_provider: codex",
                        "    agent_takt_host_model: gpt-5.4",
                    ]
                )
                + "\n",
                encoding="utf-8",
            )

            with mock.patch.object(takt_profile, "project_root", return_value=root):
                with self.assertRaisesRegex(
                    ValueError, "multiple agent profile snapshots"
                ):
                    takt_profile.queue_profiles()

    def test_run_queue_returns_zero_when_no_pending_tasks(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = Path(tmp_dir)
            self.copy_fixture_tree(root)

            with mock.patch.object(takt_profile, "project_root", return_value=root):
                with mock.patch("scripts.takt_profile.subprocess.call") as call:
                    code = takt_profile.run_queue()

            self.assertEqual(code, 0)
            call.assert_not_called()

    def test_run_queue_failure_increments_task_failure_streak(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = Path(tmp_dir)
            self.copy_fixture_tree(root)
            self.write_single_queue_task(root, failure_streak=1)

            stderr = io.StringIO()
            with mock.patch.object(takt_profile, "project_root", return_value=root):
                with mock.patch.object(takt_profile, "render_personas"):
                    with mock.patch.object(
                        takt_profile, "circuit_breaker_failure_limit", return_value=3
                    ):
                        with mock.patch(
                            "scripts.takt_profile.subprocess.call", return_value=1
                        ):
                            with redirect_stderr(stderr):
                                code = takt_profile.run_queue()

            self.assertEqual(code, 1)
            tasks_text = (root / ".takt" / "tasks.yaml").read_text(encoding="utf-8")
            self.assertIn("status: pending", tasks_text)
            self.assertIn("agent_failure_streak:", tasks_text)
            parsed_tasks = takt_profile.parse_tasks_file(root)
            self.assertEqual(parsed_tasks[0]["agent_failure_streak"], "2")
            self.assertIn(
                "takt queue task 'sample-queue-task' failed (2 consecutive failures).",
                stderr.getvalue(),
            )
            handoffs_dir = root / ".takt" / takt_profile.HANDOFFS_DIR
            self.assertFalse(handoffs_dir.exists() and any(handoffs_dir.iterdir()))

    def test_run_queue_limit_failure_stays_pending_when_analysis_is_not_loop(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = Path(tmp_dir)
            self.copy_fixture_tree(root)
            self.write_single_queue_task(root, failure_streak=2)

            stderr = io.StringIO()
            with mock.patch.object(takt_profile, "project_root", return_value=root):
                with mock.patch.object(takt_profile, "render_personas"):
                    with mock.patch(
                        "scripts.takt_profile.analyze_loop_with_researcher",
                        return_value={
                            "provider": "gemini",
                            "decision": takt_profile.LOOP_ANALYSIS_DECISION_TRANSIENT,
                            "confidence": 0.92,
                            "rationale": "Transient failure,\nnot an infinite loop.",
                        },
                    ):
                        with mock.patch.object(
                            takt_profile,
                            "circuit_breaker_failure_limit",
                            return_value=3,
                        ):
                            with mock.patch(
                                "scripts.takt_profile.subprocess.call", return_value=1
                            ):
                                with redirect_stderr(stderr):
                                    code = takt_profile.run_queue()

            self.assertEqual(code, 1)
            tasks_text = (root / ".takt" / "tasks.yaml").read_text(encoding="utf-8")
            self.assertIn("status: pending", tasks_text)
            self.assertIn("agent_failure_streak:", tasks_text)
            self.assertIn("agent_loop_analysis_provider: gemini", tasks_text)
            self.assertIn("agent_loop_analysis_decision: transient", tasks_text)
            parsed_tasks = takt_profile.parse_tasks_file(root)
            self.assertEqual(len(parsed_tasks), 1)
            self.assertEqual(parsed_tasks[0]["agent_failure_streak"], "3")
            self.assertEqual(parsed_tasks[0]["agent_loop_analysis_confidence"], "0.92")
            self.assertEqual(
                parsed_tasks[0]["agent_loop_analysis_rationale"],
                "Transient failure, not an infinite loop.",
            )
            handoffs_dir = root / ".takt" / takt_profile.HANDOFFS_DIR
            self.assertFalse(handoffs_dir.exists() and any(handoffs_dir.iterdir()))
            combined = stderr.getvalue()
            self.assertIn(
                "Loop analysis via gemini: decision=transient, confidence=0.92",
                combined,
            )
            self.assertIn(
                "takt queue task 'sample-queue-task' failed (3 consecutive failures).",
                combined,
            )

    def test_run_queue_blocks_task_when_analysis_detects_loop_and_writes_handoff(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = Path(tmp_dir)
            self.copy_fixture_tree(root)
            self.write_single_queue_task(root, failure_streak=2)

            stderr = io.StringIO()
            with mock.patch.object(takt_profile, "project_root", return_value=root):
                with mock.patch.object(takt_profile, "render_personas"):
                    with mock.patch(
                        "scripts.takt_profile.analyze_loop_with_researcher",
                        return_value={
                            "provider": "gemini",
                            "decision": takt_profile.LOOP_ANALYSIS_DECISION_LOOP,
                            "confidence": 0.95,
                            "rationale": "Same failure signature repeated with no progress.",
                        },
                    ):
                        with mock.patch.object(
                            takt_profile,
                            "circuit_breaker_failure_limit",
                            return_value=3,
                        ):
                            with mock.patch(
                                "scripts.takt_profile.subprocess.call", return_value=1
                            ):
                                with redirect_stderr(stderr):
                                    code = takt_profile.run_queue()

            self.assertEqual(code, 1)
            tasks_text = (root / ".takt" / "tasks.yaml").read_text(encoding="utf-8")
            self.assertIn("status: blocked", tasks_text)
            self.assertIn("agent_failure_streak:", tasks_text)
            self.assertIn("agent_circuit_breaker_blocked_at:", tasks_text)
            self.assertIn("agent_circuit_breaker_reason:", tasks_text)
            self.assertIn("agent_loop_analysis_decision: loop", tasks_text)
            parsed_tasks = takt_profile.parse_tasks_file(root)
            self.assertEqual(parsed_tasks[0]["agent_failure_streak"], "3")
            self.assertEqual(parsed_tasks[0]["agent_loop_analysis_confidence"], "0.95")
            handoffs_dir = root / ".takt" / takt_profile.HANDOFFS_DIR
            self.assertTrue(handoffs_dir.is_dir())
            handoff_files = list(handoffs_dir.glob("handoff-sample-queue-task-*.md"))
            self.assertEqual(len(handoff_files), 1)
            handoff = handoff_files[0].read_text(encoding="utf-8")
            self.assertIn("# Takt Circuit Breaker Handoff", handoff)
            self.assertIn("status: blocked", handoff)
            self.assertIn("sample-queue-task", handoff)
            self.assertIn("loop_analysis_decision: loop", handoff)
            combined = stderr.getvalue()
            self.assertIn(
                "Loop analysis via gemini: decision=loop, confidence=0.95", combined
            )
            self.assertIn(
                "Circuit breaker opened for 'sample-queue-task' after loop detection (3 consecutive failures).",
                combined,
            )
            self.assertIn(
                "Generated human handoff: .takt/handoffs/handoff-sample-queue-task-",
                combined,
            )

    def test_run_queue_blocks_task_when_analysis_is_unknown(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = Path(tmp_dir)
            self.copy_fixture_tree(root)
            self.write_single_queue_task(root, failure_streak=2)

            stderr = io.StringIO()
            with mock.patch.object(takt_profile, "project_root", return_value=root):
                with mock.patch.object(takt_profile, "render_personas"):
                    with mock.patch(
                        "scripts.takt_profile.analyze_loop_with_researcher",
                        return_value={
                            "provider": "gemini",
                            "decision": takt_profile.LOOP_ANALYSIS_DECISION_UNKNOWN,
                            "confidence": 0.0,
                            "rationale": "Researcher analysis timed out.",
                        },
                    ):
                        with mock.patch.object(
                            takt_profile,
                            "circuit_breaker_failure_limit",
                            return_value=3,
                        ):
                            with mock.patch(
                                "scripts.takt_profile.subprocess.call", return_value=1
                            ):
                                with redirect_stderr(stderr):
                                    code = takt_profile.run_queue()

            self.assertEqual(code, 1)
            tasks_text = (root / ".takt" / "tasks.yaml").read_text(encoding="utf-8")
            self.assertIn("status: blocked", tasks_text)
            self.assertIn("agent_loop_analysis_decision: unknown", tasks_text)
            parsed_tasks = takt_profile.parse_tasks_file(root)
            self.assertEqual(parsed_tasks[0]["agent_failure_streak"], "3")
            self.assertEqual(
                parsed_tasks[0]["agent_circuit_breaker_reason"],
                "Circuit breaker opened because loop analysis was inconclusive at the failure threshold.",
            )
            handoffs_dir = root / ".takt" / takt_profile.HANDOFFS_DIR
            self.assertTrue(handoffs_dir.is_dir())
            handoff_files = list(handoffs_dir.glob("handoff-sample-queue-task-*.md"))
            self.assertEqual(len(handoff_files), 1)
            combined = stderr.getvalue()
            self.assertIn(
                "Loop analysis via gemini: decision=unknown, confidence=0.00", combined
            )
            self.assertIn(
                "Circuit breaker opened for 'sample-queue-task' because loop analysis was inconclusive (3 consecutive failures).",
                combined,
            )
            self.assertIn(
                "Generated human handoff: .takt/handoffs/handoff-sample-queue-task-",
                combined,
            )

    def test_run_queue_success_resets_task_failure_streak(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = Path(tmp_dir)
            self.copy_fixture_tree(root)
            self.write_single_queue_task(root, failure_streak=2)

            with mock.patch.object(takt_profile, "project_root", return_value=root):
                with mock.patch.object(takt_profile, "render_personas"):
                    with mock.patch(
                        "scripts.takt_profile.subprocess.call", return_value=0
                    ):
                        code = takt_profile.run_queue()

            self.assertEqual(code, 0)
            tasks_text = (root / ".takt" / "tasks.yaml").read_text(encoding="utf-8")
            self.assertIn("status: pending", tasks_text)
            self.assertIn("agent_failure_streak:", tasks_text)
            parsed_tasks = takt_profile.parse_tasks_file(root)
            self.assertEqual(parsed_tasks[0]["agent_failure_streak"], "0")

    def test_researcher_analysis_command_rejects_shell_redirect_examples(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = Path(tmp_dir)
            self.copy_fixture_tree(root)
            profiles = json.loads(
                (root / ".claude" / "agent-profiles.json").read_text(encoding="utf-8")
            )
            profiles["providers"]["gemini"]["invoke_examples"]["researcher"] = (
                'gemini -p "Research: {task}" 2>/dev/null'
            )

            provider_name, command = takt_profile.researcher_analysis_command(
                "Research this failure",
                profiles,
                root=root,
            )

            self.assertEqual(provider_name, "gemini")
            self.assertEqual(command, [])

    def test_write_tasks_file_round_trips_yaml_sensitive_strings(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = Path(tmp_dir)
            self.copy_fixture_tree(root)
            tasks = [
                {
                    "name": "sample-queue-task",
                    "status": "pending",
                    "slug": "sample-queue-task",
                    "summary": "sample queue task",
                    "task_dir": ".takt/tasks/20260304-164008-sample-queue-task",
                    "created_at": "2026-03-04T16:40:08.902Z",
                    "agent_loop_analysis_rationale": "Don't loop: same failure #123",
                    "agent_loop_analysis_provider": "'leading-quote",
                    "agent_loop_analysis_decision": '"leading-dquote',
                    "agent_loop_analysis_confidence": "'both'",
                    "agent_circuit_breaker_reason": "line one\nline two",
                }
            ]

            takt_profile.write_tasks_file(tasks, root)

            tasks_text = (root / ".takt" / "tasks.yaml").read_text(encoding="utf-8")
            self.assertIn("created_at: 2026-03-04T16:40:08.902Z", tasks_text)
            self.assertIn("agent_loop_analysis_rationale:", tasks_text)
            self.assertIn("agent_loop_analysis_provider:", tasks_text)
            self.assertIn("agent_loop_analysis_decision:", tasks_text)
            self.assertIn("agent_loop_analysis_confidence:", tasks_text)
            self.assertIn("agent_circuit_breaker_reason: |-", tasks_text)
            self.assertIn("  line one", tasks_text)
            self.assertIn("  line two", tasks_text)

            parsed_tasks = takt_profile.parse_tasks_file(root)
            self.assertEqual(parsed_tasks[0]["created_at"], "2026-03-04T16:40:08.902Z")
            self.assertEqual(
                parsed_tasks[0]["agent_loop_analysis_rationale"],
                "Don't loop: same failure #123",
            )
            self.assertEqual(
                parsed_tasks[0]["agent_loop_analysis_provider"], "'leading-quote"
            )
            self.assertEqual(
                parsed_tasks[0]["agent_loop_analysis_decision"], '"leading-dquote'
            )
            self.assertEqual(
                parsed_tasks[0]["agent_loop_analysis_confidence"], "'both'"
            )
            self.assertEqual(
                parsed_tasks[0]["agent_circuit_breaker_reason"], "line one\nline two"
            )

    def test_parse_tasks_file_accepts_block_scalar_yaml(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = Path(tmp_dir)
            self.copy_fixture_tree(root)
            (root / ".takt" / "tasks.yaml").write_text(
                "\n".join(
                    [
                        "tasks:",
                        "  - name: sample-queue-task",
                        "    status: pending",
                        "    summary: |-",
                        "      line one",
                        "      line two",
                    ]
                )
                + "\n",
                encoding="utf-8",
            )

            parsed_tasks = takt_profile.parse_tasks_file(root)

            self.assertEqual(len(parsed_tasks), 1)
            self.assertEqual(parsed_tasks[0]["summary"], "line one\nline two")

    def test_parse_tasks_file_wraps_yaml_parse_errors(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = Path(tmp_dir)
            self.copy_fixture_tree(root)
            (root / ".takt" / "tasks.yaml").write_text(
                "\n".join(
                    [
                        "tasks:",
                        "  - name: sample-queue-task",
                        "    summary: bad: [",
                    ]
                )
                + "\n",
                encoding="utf-8",
            )

            with self.assertRaisesRegex(ValueError, r"Failed to parse .*tasks\.yaml"):
                takt_profile.parse_tasks_file(root)

    def test_main_prints_host_provider(self) -> None:
        stdout = io.StringIO()
        with redirect_stdout(stdout):
            code = takt_profile.main(["host-provider"])

        self.assertEqual(code, 0)
        self.assertEqual(stdout.getvalue().strip(), "claude")

    def test_main_returns_error_for_invalid_queue_snapshots(self) -> None:
        stderr = io.StringIO()
        with mock.patch.object(
            takt_profile, "run_queue", side_effect=ValueError("broken queue")
        ):
            with redirect_stderr(stderr):
                code = takt_profile.main(["run-queue"])

        self.assertEqual(code, 1)
        self.assertEqual(stderr.getvalue().strip(), "broken queue")

    def test_slugify_basic(self) -> None:
        self.assertEqual(takt_profile._slugify("Hello World!"), "hello-world")
        self.assertEqual(takt_profile._slugify(""), "unknown")
        self.assertEqual(takt_profile._slugify("  "), "unknown")
        self.assertEqual(takt_profile._slugify("タスク"), "unknown")
        self.assertEqual(takt_profile._slugify("a" * 60), "a" * 40)
        self.assertEqual(takt_profile._slugify("My--Task"), "my-task")

    def test_handoff_path_uses_handoffs_dir_with_task_slug(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            task = {"name": "sample-task", "piece": "full-cycle"}
            path = takt_profile.handoff_path(task=task, root=root)
            self.assertEqual(path.parent.name, takt_profile.HANDOFFS_DIR)
            self.assertTrue(path.name.startswith("handoff-sample-task-"))
            self.assertTrue(path.name.endswith(".md"))

    def test_handoff_path_falls_back_to_slug_key_for_non_ascii_name(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            task = {"name": "タスク名", "slug": "my-slug", "piece": "full-cycle"}
            path = takt_profile.handoff_path(task=task, root=root)
            self.assertIn("handoff-my-slug-", path.name)

    def test_handoff_path_falls_back_to_task_dir_for_non_ascii(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            task = {
                "name": "タスク名",
                "task_dir": "/path/to/my-dir",
                "piece": "full-cycle",
            }
            path = takt_profile.handoff_path(task=task, root=root)
            self.assertIn("handoff-my-dir-", path.name)

    def test_handoff_path_without_task_uses_unknown(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            path = takt_profile.handoff_path(root=root)
            self.assertIn("handoff-unknown-", path.name)

    def test_write_handoff_file_creates_in_handoffs_dir(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            task = {"name": "my-task", "piece": "full-cycle"}
            path = takt_profile.write_handoff_file(
                task=task,
                failure_streak=3,
                failure_limit=3,
                exit_code=1,
                loop_analysis=None,
                root=root,
            )
            self.assertTrue(path.exists())
            content = path.read_text(encoding="utf-8")
            self.assertIn("# Takt Circuit Breaker Handoff", content)
            self.assertIn("my-task", content)
            self.assertEqual(path.parent.name, takt_profile.HANDOFFS_DIR)

    def test_write_handoff_file_does_not_overwrite_previous(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            t1 = {"name": "task-alpha", "piece": "full-cycle"}
            t2 = {"name": "task-beta", "piece": "full-cycle"}
            p1 = takt_profile.write_handoff_file(
                task=t1,
                failure_streak=3,
                failure_limit=3,
                exit_code=1,
                loop_analysis=None,
                root=root,
            )
            p2 = takt_profile.write_handoff_file(
                task=t2,
                failure_streak=3,
                failure_limit=3,
                exit_code=1,
                loop_analysis=None,
                root=root,
            )
            self.assertNotEqual(p1, p2)
            self.assertTrue(p1.exists())
            self.assertTrue(p2.exists())
            handoffs = list(p1.parent.iterdir())
            self.assertEqual(len(handoffs), 2)


class BuildLoopAnalysisPromptTest(unittest.TestCase):
    def test_prompt_includes_git_diff_stat_section(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            takt_dir = root / ".takt"
            takt_dir.mkdir()
            (takt_dir / "last-failure.log").write_text(
                "error: something\n", encoding="utf-8"
            )
            (takt_dir / "debug-report.md").write_text("# Debug\n", encoding="utf-8")
            # Initialize a git repo with a tracked file, then modify it
            subprocess.run(["git", "init"], cwd=root, check=True, capture_output=True)
            subprocess.run(
                ["git", "config", "user.email", "t@t.com"],
                cwd=root,
                check=True,
                capture_output=True,
            )
            subprocess.run(
                ["git", "config", "user.name", "T"],
                cwd=root,
                check=True,
                capture_output=True,
            )
            (root / "a.rs").write_text("fn main() {}\n", encoding="utf-8")
            subprocess.run(
                ["git", "add", "-A"], cwd=root, check=True, capture_output=True
            )
            subprocess.run(
                ["git", "commit", "-m", "init"],
                cwd=root,
                check=True,
                capture_output=True,
            )
            (root / "a.rs").write_text(
                'fn main() { println!("hi"); }\n', encoding="utf-8"
            )

            task = {"description": "test-task", "piece": "full-cycle"}
            prompt = takt_profile.build_loop_analysis_prompt(
                task=task,
                failure_streak=3,
                failure_limit=3,
                root=root,
            )
            self.assertIn("git diff --stat", prompt)
            self.assertIn("a.rs", prompt)

    def test_prompt_includes_untracked_files(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            takt_dir = root / ".takt"
            takt_dir.mkdir()
            (takt_dir / "last-failure.log").write_text("error\n", encoding="utf-8")
            subprocess.run(["git", "init"], cwd=root, check=True, capture_output=True)
            subprocess.run(
                ["git", "config", "user.email", "t@t.com"],
                cwd=root,
                check=True,
                capture_output=True,
            )
            subprocess.run(
                ["git", "config", "user.name", "T"],
                cwd=root,
                check=True,
                capture_output=True,
            )
            (root / "a.rs").write_text("fn main() {}\n", encoding="utf-8")
            subprocess.run(
                ["git", "add", "-A"], cwd=root, check=True, capture_output=True
            )
            subprocess.run(
                ["git", "commit", "-m", "init"],
                cwd=root,
                check=True,
                capture_output=True,
            )
            # Create an untracked file (no tracked changes)
            (root / "new_file.rs").write_text("fn new() {}\n", encoding="utf-8")

            task = {"description": "test-task", "piece": "full-cycle"}
            prompt = takt_profile.build_loop_analysis_prompt(
                task=task,
                failure_streak=3,
                failure_limit=3,
                root=root,
            )
            self.assertIn("Untracked files", prompt)
            self.assertIn("new_file.rs", prompt)

    def test_prompt_shows_none_when_no_changes(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            takt_dir = root / ".takt"
            takt_dir.mkdir()
            (takt_dir / "last-failure.log").write_text("error\n", encoding="utf-8")
            subprocess.run(["git", "init"], cwd=root, check=True, capture_output=True)
            subprocess.run(
                ["git", "config", "user.email", "t@t.com"],
                cwd=root,
                check=True,
                capture_output=True,
            )
            subprocess.run(
                ["git", "config", "user.name", "T"],
                cwd=root,
                check=True,
                capture_output=True,
            )
            (root / "a.rs").write_text("fn main() {}\n", encoding="utf-8")
            subprocess.run(
                ["git", "add", "-A"], cwd=root, check=True, capture_output=True
            )
            subprocess.run(
                ["git", "commit", "-m", "init"],
                cwd=root,
                check=True,
                capture_output=True,
            )

            task = {"description": "test-task", "piece": "full-cycle"}
            prompt = takt_profile.build_loop_analysis_prompt(
                task=task,
                failure_streak=3,
                failure_limit=3,
                root=root,
            )
            self.assertIn("none detected", prompt)


class CleanQueueTest(unittest.TestCase):
    def _write_tasks_yaml(self, root: Path, tasks: list[dict]) -> None:
        takt_dir = root / ".takt"
        takt_dir.mkdir(parents=True, exist_ok=True)
        import yaml as _yaml

        with (takt_dir / "tasks.yaml").open("w", encoding="utf-8") as f:
            _yaml.dump({"tasks": tasks}, f, sort_keys=False, allow_unicode=True)

    def test_clean_removes_blocked_tasks(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            self._write_tasks_yaml(
                root,
                [
                    {"description": "task-a", "status": "pending"},
                    {"description": "task-b", "status": "blocked"},
                ],
            )

            stdout = io.StringIO()
            with redirect_stdout(stdout):
                code = takt_profile.clean_queue(root)

            self.assertEqual(code, 0)
            remaining = takt_profile.parse_tasks_file(root)
            self.assertEqual(len(remaining), 1)
            self.assertEqual(remaining[0]["description"], "task-a")
            self.assertIn("Removed 1", stdout.getvalue())

    def test_clean_noop_when_all_pending(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            self._write_tasks_yaml(
                root,
                [
                    {"description": "task-a", "status": "pending"},
                ],
            )

            stdout = io.StringIO()
            with redirect_stdout(stdout):
                code = takt_profile.clean_queue(root)

            self.assertEqual(code, 0)
            self.assertIn("Nothing to clean", stdout.getvalue())

    def test_clean_empty_queue(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)

            stdout = io.StringIO()
            with redirect_stdout(stdout):
                code = takt_profile.clean_queue(root)

            self.assertEqual(code, 0)
            self.assertIn("empty", stdout.getvalue())


class TasksFileLockTest(unittest.TestCase):
    def _write_tasks_yaml(self, root: Path, tasks: list[dict]) -> None:
        takt_dir = root / ".takt"
        takt_dir.mkdir(parents=True, exist_ok=True)
        import yaml as _yaml

        with (takt_dir / "tasks.yaml").open("w", encoding="utf-8") as f:
            _yaml.dump({"tasks": tasks}, f, sort_keys=False, allow_unicode=True)

    def test_lock_creates_lock_file(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / ".takt").mkdir(parents=True, exist_ok=True)
            with takt_profile._tasks_file_lock(root):
                lock_file = root / ".takt" / "tasks.yaml.lock"
                self.assertTrue(lock_file.exists())

    def test_lock_released_after_context(self) -> None:
        """After exiting the context, the lock should be released and re-acquirable."""
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / ".takt").mkdir(parents=True, exist_ok=True)
            with takt_profile._tasks_file_lock(root):
                pass
            # Should be able to re-acquire
            with takt_profile._tasks_file_lock(root):
                pass

    def test_clean_queue_with_lock(self) -> None:
        """clean_queue should work correctly with the file lock."""
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            self._write_tasks_yaml(
                root,
                [
                    {"description": "task-a", "status": "pending"},
                    {"description": "task-b", "status": "blocked"},
                ],
            )

            stdout = io.StringIO()
            with redirect_stdout(stdout):
                code = takt_profile.clean_queue(root)

            self.assertEqual(code, 0)
            remaining = takt_profile.parse_tasks_file(root)
            self.assertEqual(len(remaining), 1)


if __name__ == "__main__":
    unittest.main()
