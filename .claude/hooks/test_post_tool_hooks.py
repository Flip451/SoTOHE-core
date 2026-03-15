import builtins
import io
import json
import os
import tempfile
import unittest
from contextlib import redirect_stdout
from pathlib import Path
from unittest import mock

from test_helpers import load_hook_module, write_agent_profiles

shared = load_hook_module("_shared")
post_test_analysis = load_hook_module("post-test-analysis")
error_to_codex = load_hook_module("error-to-codex")
post_implementation_review = load_hook_module("post-implementation-review")
check_codex_after_plan = load_hook_module("check-codex-after-plan")
lint_on_save = load_hook_module("lint-on-save")
python_lint_on_save = load_hook_module("python-lint-on-save")
log_cli_tools = load_hook_module("log-cli-tools")


class PostToolHooksTest(unittest.TestCase):
    def with_profile(self, active_profile: str, mutator=None):
        temp_dir = tempfile.TemporaryDirectory()
        self.addCleanup(temp_dir.cleanup)
        config_path = write_agent_profiles(
            Path(temp_dir.name) / "agent-profiles.json", active_profile, mutator
        )
        return mock.patch.dict(
            os.environ, {"CLAUDE_AGENT_PROFILES_PATH": str(config_path)}
        )

    def test_post_test_analysis_targets_ci_related_commands(self) -> None:
        self.assertTrue(post_test_analysis.is_test_or_build_command("cargo make ci"))
        self.assertTrue(
            post_test_analysis.is_test_or_build_command("cargo build --workspace")
        )

    def test_error_to_codex_skips_commands_handled_by_post_test_analysis(self) -> None:
        self.assertTrue(error_to_codex.should_ignore_command("cargo test --workspace"))
        self.assertTrue(
            error_to_codex.should_ignore_command("cargo   test   --workspace")
        )
        self.assertTrue(error_to_codex.should_ignore_command("cargo make ci"))
        self.assertFalse(
            error_to_codex.should_ignore_command("python3 scripts/report.py")
        )

    def test_error_to_codex_skips_git_show_and_notes(self) -> None:
        self.assertTrue(error_to_codex.should_ignore_command("git show e564aef"))
        self.assertTrue(error_to_codex.should_ignore_command("git notes show HEAD"))
        self.assertTrue(error_to_codex.should_ignore_command("git notes list"))

    def test_error_to_codex_ignores_only_active_provider_commands(self) -> None:
        def mutator(profiles: dict) -> None:
            profiles["providers"]["inactive"] = {
                "label": "Inactive CLI",
                "supported_capabilities": ["debugger"],
                "invoke_examples": {
                    "debugger": 'inactivecli debug "{task}"',
                },
            }

        with self.with_profile("default", mutator):
            self.assertFalse(
                error_to_codex.should_ignore_command("inactivecli debug issue")
            )
            self.assertTrue(
                error_to_codex.should_ignore_command("codex exec --model gpt-5.4")
            )

    def test_error_to_codex_does_not_detect_errors_in_clean_output(self) -> None:
        self.assertEqual(
            error_to_codex.detect_errors("Build succeeded. 3 tests passed."), []
        )

    def test_error_to_codex_includes_rust_context_for_borrow_errors(self) -> None:
        message = error_to_codex.build_error_message(
            1, "error[E0502]: cannot borrow `x` as mutable"
        )
        self.assertIn("compile/borrow", message)

    def test_error_to_codex_omits_rust_context_for_generic_errors(self) -> None:
        message = error_to_codex.build_error_message(
            1, "FAILED: some process exited with code 1"
        )
        self.assertNotIn("compile/borrow", message)

    def test_post_test_analysis_reads_stderr(self) -> None:
        data = {
            "tool_name": "Bash",
            "tool_input": {"command": "cargo test --workspace"},
            "tool_response": {
                "stderr": "error[E0382]: use of moved value\nthread 'x' panicked"
            },
        }

        stdout = io.StringIO()
        with mock.patch.object(
            post_test_analysis, "load_stdin_json", return_value=data
        ):
            with redirect_stdout(stdout):
                with self.assertRaises(SystemExit) as exc:
                    post_test_analysis.main()

        output = stdout.getvalue()
        self.assertEqual(exc.exception.code, 0)
        payload = json.loads(output)
        message = payload["hookSpecificOutput"]["additionalContext"]
        self.assertIn(post_test_analysis.DEBUG_PREFIX, message)
        self.assertIn(
            post_test_analysis.build_debug_message(
                "Rust compiler error (with error code)"
            ),
            message,
        )

    def test_error_to_codex_reads_stderr(self) -> None:
        data = {
            "tool_name": "Bash",
            "tool_input": {"command": "./scripts/custom-rust-check.sh"},
            "tool_response": {"stderr": "error[E0502]: cannot borrow `x` as mutable"},
        }

        stdout = io.StringIO()
        with mock.patch.object(error_to_codex, "load_stdin_json", return_value=data):
            with redirect_stdout(stdout):
                with self.assertRaises(SystemExit) as exc:
                    error_to_codex.main()

        output = stdout.getvalue()
        self.assertEqual(exc.exception.code, 0)
        self.assertIn(error_to_codex.ERROR_PREFIX, output)
        self.assertIn(error_to_codex.RUST_CONTEXT.strip(), output)

    def test_check_codex_after_plan_reads_content_blocks(self) -> None:
        tool_input = {
            "prompt": "Create implementation plan for this feature",
            "description": "track:plan for onboarding",
        }
        tool_response = {
            "content": [
                {"type": "text", "text": "Updated plan.md with implementation plan"},
                {"type": "text", "text": "Also touched spec.md"},
            ]
        }

        self.assertTrue(
            check_codex_after_plan.looks_like_plan_task(tool_input, tool_response)
        )

    def test_check_codex_after_plan_reads_result_blocks(self) -> None:
        tool_input = {
            "prompt": "Write a spec and plan",
            "description": "architecture planning task",
        }
        tool_response = {
            "result": [
                {"type": "text", "text": "Created spec.md"},
                {"type": "text", "text": "Added implementation plan"},
            ]
        }

        self.assertTrue(
            check_codex_after_plan.looks_like_plan_task(tool_input, tool_response)
        )

    def test_check_codex_after_plan_uses_constant_message(self) -> None:
        data = {
            "tool_name": "Task",
            "tool_input": {
                "prompt": "Create implementation plan",
                "description": "track:plan for service refactor",
            },
            "tool_response": {"content": "Updated plan.md with implementation plan"},
        }

        stdout = io.StringIO()
        with mock.patch.object(
            check_codex_after_plan, "load_stdin_json", return_value=data
        ):
            with redirect_stdout(stdout):
                with self.assertRaises(SystemExit) as exc:
                    check_codex_after_plan.main()

        output = stdout.getvalue()
        self.assertEqual(exc.exception.code, 0)
        payload = json.loads(output)
        message = payload["hookSpecificOutput"]["additionalContext"]
        self.assertIn(check_codex_after_plan.DESIGN_REVIEW_PREFIX, message)
        self.assertEqual(check_codex_after_plan.build_design_review_message(), message)

    def test_post_implementation_review_counts_structured_new_string(self) -> None:
        content = shared.tool_input_text(
            {
                "new_string": [
                    {"type": "text", "text": "fn a() {}\n\n// comment\nfn b() {}"},
                ]
            },
            "content",
            "new_string",
        )

        self.assertEqual(post_implementation_review.count_lines(content), 2)

    def test_post_implementation_review_counts_edit_diff_instead_of_full_payload(
        self,
    ) -> None:
        old_content = "\n".join(f"let value_{index} = {index};" for index in range(90))
        new_content = f"{old_content}\nlet added = 90;"

        changed = post_implementation_review.measure_change_lines(
            "Edit",
            {
                "old_string": old_content,
                "new_string": new_content,
            },
        )

        self.assertEqual(changed, 1)

    def test_post_implementation_review_write_uses_diff_not_full_content(self) -> None:
        state: dict = {"file_snapshots": {}}
        content_v1 = "\n".join(f"let x_{i} = {i};" for i in range(50))
        content_v2 = content_v1 + "\nlet added = 50;"

        # First Write: all meaningful lines count as new
        lines_v1 = post_implementation_review.measure_change_lines(
            "Write",
            {"file_path": "libs/domain/src/lib.rs", "content": content_v1},
            state,
        )
        self.assertEqual(lines_v1, 50)

        # Second Write to same file: only the diff counts
        lines_v2 = post_implementation_review.measure_change_lines(
            "Write",
            {"file_path": "libs/domain/src/lib.rs", "content": content_v2},
            state,
        )
        self.assertEqual(lines_v2, 1)

    def test_post_implementation_review_write_without_state_falls_back_to_count(
        self,
    ) -> None:
        content = "fn a() {}\nfn b() {}"
        lines = post_implementation_review.measure_change_lines(
            "Write",
            {"file_path": "libs/domain/src/lib.rs", "content": content},
        )
        self.assertEqual(lines, 2)

    def test_post_implementation_review_write_edit_write_minor_overcount(self) -> None:
        state: dict = {"file_snapshots": {}}
        fp = "libs/domain/src/lib.rs"
        content_v1 = "fn a() {}\nfn b() {}"

        # Write v1: 2 new lines
        lines = post_implementation_review.measure_change_lines(
            "Write",
            {"file_path": fp, "content": content_v1},
            state,
        )
        self.assertEqual(lines, 2)

        # Edit: change b -> c (counted as 2: old removed + new added)
        lines = post_implementation_review.measure_change_lines(
            "Edit",
            {"file_path": fp, "old_string": "fn b() {}", "new_string": "fn c() {}"},
            state,
        )
        self.assertEqual(lines, 2)

        # Write v2 after edit: Edit diff re-counted from last Write snapshot,
        # but this minor overcount is acceptable (vs original full-content bug)
        content_v2 = "fn a() {}\nfn c() {}"
        lines = post_implementation_review.measure_change_lines(
            "Write",
            {"file_path": fp, "content": content_v2},
            state,
        )
        self.assertEqual(lines, 2)  # re-counts the b->c edit diff

    def test_post_implementation_review_resets_state_for_new_session(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            state_file = Path(tmp_dir) / "post-implementation-review-state.json"
            state_file.write_text(
                json.dumps(
                    {
                        "session_marker": "ppid:old",
                        "files_changed": ["a.rs"],
                        "total_lines": 120,
                        "review_suggested": True,
                    }
                ),
                encoding="utf-8",
            )

            with mock.patch.object(
                post_implementation_review,
                "get_state_file",
                return_value=str(state_file),
            ):
                with mock.patch.object(
                    post_implementation_review,
                    "current_session_marker",
                    return_value="ppid:new",
                ):
                    state = post_implementation_review.load_state()

        self.assertEqual(state["session_marker"], "ppid:new")
        self.assertEqual(state["files_changed"], [])
        self.assertEqual(state["total_changed_lines"], 0)
        self.assertFalse(state["review_suggested"])

    def test_post_implementation_review_load_state_migrates_legacy_total_lines(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            state_file = Path(tmp_dir) / "post-implementation-review-state.json"
            state_file.write_text(
                json.dumps(
                    {
                        "session_marker": "ppid:current",
                        "files_changed": ["a.rs"],
                        "total_lines": 12,
                        "review_suggested": False,
                    }
                ),
                encoding="utf-8",
            )

            with mock.patch.object(
                post_implementation_review,
                "get_state_file",
                return_value=str(state_file),
            ):
                with mock.patch.object(
                    post_implementation_review,
                    "current_session_marker",
                    return_value="ppid:current",
                ):
                    state = post_implementation_review.load_state()

        self.assertEqual(state["total_changed_lines"], 12)

    def test_post_implementation_review_main_tracks_edit_diff_in_state(self) -> None:
        old_content = "\n".join(f"let value_{index} = {index};" for index in range(90))
        new_content = f"{old_content}\nlet added = 90;"
        data = {
            "tool_name": "Edit",
            "tool_input": {
                "file_path": "libs/domain/src/lib.rs",
                "old_string": old_content,
                "new_string": new_content,
            },
        }

        with tempfile.TemporaryDirectory() as tmp_dir:
            state_file = Path(tmp_dir) / "post-implementation-review-state.json"
            stdout = io.StringIO()

            with mock.patch.object(
                post_implementation_review,
                "get_state_file",
                return_value=str(state_file),
            ):
                with mock.patch.object(
                    post_implementation_review, "load_stdin_json", return_value=data
                ):
                    with redirect_stdout(stdout):
                        with self.assertRaises(SystemExit) as exc:
                            post_implementation_review.main()

            self.assertEqual(exc.exception.code, 0)
            self.assertEqual(stdout.getvalue(), "")
            state = json.loads(state_file.read_text(encoding="utf-8"))
            self.assertEqual(state["files_changed"], ["libs/domain/src/lib.rs"])
            self.assertEqual(state["total_changed_lines"], 1)

    def test_post_implementation_review_main_write_twice_counts_diff_not_double(
        self,
    ) -> None:
        content_v1 = "\n".join(f"let x_{i} = {i};" for i in range(50))
        content_v2 = content_v1 + "\nlet added = 50;"

        with tempfile.TemporaryDirectory() as tmp_dir:
            state_file = Path(tmp_dir) / "post-implementation-review-state.json"
            stdout = io.StringIO()

            with mock.patch.object(
                post_implementation_review,
                "get_state_file",
                return_value=str(state_file),
            ):
                # First Write
                data1 = {
                    "tool_name": "Write",
                    "tool_input": {
                        "file_path": "libs/domain/src/lib.rs",
                        "content": content_v1,
                    },
                }
                with mock.patch.object(
                    post_implementation_review, "load_stdin_json", return_value=data1
                ):
                    with redirect_stdout(stdout):
                        with self.assertRaises(SystemExit):
                            post_implementation_review.main()

                state = json.loads(state_file.read_text(encoding="utf-8"))
                self.assertEqual(state["total_changed_lines"], 50)

                # Second Write to same file — only 1 line added
                data2 = {
                    "tool_name": "Write",
                    "tool_input": {
                        "file_path": "libs/domain/src/lib.rs",
                        "content": content_v2,
                    },
                }
                with mock.patch.object(
                    post_implementation_review, "load_stdin_json", return_value=data2
                ):
                    with redirect_stdout(stdout):
                        with self.assertRaises(SystemExit):
                            post_implementation_review.main()

                state = json.loads(state_file.read_text(encoding="utf-8"))
                self.assertEqual(
                    state["total_changed_lines"], 51
                )  # 50 + 1, not 50 + 51

    def test_post_test_analysis_message_can_switch_debugger_provider(self) -> None:
        def mutator(profiles: dict) -> None:
            profiles["profiles"]["default"]["debugger"] = "claude"
            profiles["providers"]["claude"]["invoke_examples"]["debugger"] = (
                "/track:review"
            )

        with self.with_profile("default", mutator):
            message = post_test_analysis.build_debug_message(
                "Rust compiler error (with error code)"
            )

        self.assertIn("Claude Code", message)
        self.assertIn("/track:review", message)
        self.assertNotIn("codex exec", message)

    def test_post_test_analysis_detects_nextest_single_fail_line(self) -> None:
        # cargo nextest emits "FAIL  [ 0.003s] crate::mod test_name" for single failures.
        nextest_output = "FAIL  [ 0.003s] domain::tests::test_new_user_with_blank_id_returns_invalid_user_id_error"
        has_failure, reason = post_test_analysis.has_complex_failure(nextest_output)
        self.assertTrue(has_failure)
        self.assertIn("nextest", reason)

    def test_post_test_analysis_main_emits_debug_for_nextest_fail(self) -> None:
        data = {
            "tool_name": "Bash",
            "tool_input": {"command": "cargo make test"},
            "tool_response": {
                "stdout": "FAIL  [ 0.003s] domain::tests::test_new_user_with_blank_id_returns_invalid_user_id_error"
            },
        }

        stdout = io.StringIO()
        with mock.patch.object(
            post_test_analysis, "load_stdin_json", return_value=data
        ):
            with redirect_stdout(stdout):
                with self.assertRaises(SystemExit) as exc:
                    post_test_analysis.main()

        self.assertEqual(exc.exception.code, 0)
        payload = json.loads(stdout.getvalue())
        message = payload["hookSpecificOutput"]["additionalContext"]
        self.assertIn(post_test_analysis.DEBUG_PREFIX, message)
        self.assertIn("nextest", message)

    def test_check_codex_after_plan_message_can_switch_reviewer_provider(self) -> None:
        with self.with_profile("claude-heavy"):
            message = check_codex_after_plan.build_design_review_message()

        self.assertIn("Claude Code", message)
        self.assertIn("/track:review", message)
        self.assertNotIn("codex exec", message)

    def test_post_implementation_review_message_can_switch_reviewer_provider(
        self,
    ) -> None:
        with self.with_profile("claude-heavy"):
            message = post_implementation_review.build_review_message(
                "3 Rust files modified"
            )

        self.assertIn("Claude Code", message)
        self.assertIn("/track:review", message)
        self.assertNotIn("codex exec", message)

    def test_lint_on_save_uses_constant_message(self) -> None:
        message = lint_on_save.build_lint_message(
            "libs/domain/src/lib.rs",
            ["rustfmt failed: broken", "cargo check: warning"],
        )
        self.assertEqual(
            message,
            "[lint-on-save] Issues in libs/domain/src/lib.rs: rustfmt failed: broken | cargo check: warning "
            "Run `cargo make clippy` for lint details.",
        )

    def test_lint_on_save_builds_standard_hook_output(self) -> None:
        output = lint_on_save.build_hook_output("message")
        self.assertIn('"hookEventName": "PostToolUse"', output)
        self.assertIn('"additionalContext": "message"', output)

    def test_lint_on_save_get_file_path_reads_tool_input_once(self) -> None:
        file_path = lint_on_save.get_file_path(
            {"tool_input": {"file_path": "/tmp/main.rs"}}
        )
        self.assertEqual(file_path, "/tmp/main.rs")

    def test_lint_on_save_detects_paths_inside_project_root(self) -> None:
        self.assertTrue(
            lint_on_save.is_path_in_project("/repo/libs/domain/src/lib.rs", "/repo")
        )
        self.assertFalse(lint_on_save.is_path_in_project("/tmp/outside.rs", "/repo"))

    def test_lint_on_save_handles_windows_style_paths(self) -> None:
        self.assertTrue(
            lint_on_save.is_path_in_project(
                r"C:\repo\apps\api\src\lib.rs",
                r"C:\repo",
            )
        )
        self.assertEqual(
            lint_on_save.host_to_container_path(
                r"C:\repo\apps\api\src\lib.rs",
                r"C:\repo",
            ),
            "/workspace/apps/api/src/lib.rs",
        )
        self.assertFalse(
            lint_on_save.is_path_in_project(
                r"C:\outside\src\lib.rs",
                r"C:\repo",
            )
        )

    def test_lint_on_save_ignores_non_edit_write_tools(self) -> None:
        data = {
            "tool_name": "Bash",
            "tool_input": {"file_path": "/repo/libs/domain/src/lib.rs"},
        }

        with mock.patch.dict(lint_on_save.os.environ, {}, clear=True):
            with mock.patch.object(lint_on_save, "load_stdin_json", return_value=data):
                with self.assertRaises(SystemExit) as exc:
                    lint_on_save.main()

        self.assertEqual(exc.exception.code, 0)

    def test_log_cli_tools_writes_provider_command_and_output_preview(self) -> None:
        data = {
            "tool_name": "Bash",
            "tool_input": {
                "command": 'codex exec --model gpt-5.4 --sandbox read-only "review"'
            },
            "tool_response": {
                "stdout": "first line\nsecond line",
                "stderr": "",
                "exit_code": 0,
            },
        }

        with tempfile.TemporaryDirectory() as tmp_dir:
            with mock.patch.dict(
                os.environ,
                {
                    "CLAUDE_LOG_CLI_TOOLS": "1",
                    "CLAUDE_PROJECT_DIR": tmp_dir,
                },
                clear=False,
            ):
                with mock.patch.object(
                    log_cli_tools, "load_stdin_json", return_value=data
                ):
                    with self.assertRaises(SystemExit) as exc:
                        log_cli_tools.main()

            self.assertEqual(exc.exception.code, 0)
            log_path = Path(tmp_dir) / ".claude" / "logs" / "cli-tools.jsonl"
            self.assertTrue(log_path.exists())
            lines = log_path.read_text(encoding="utf-8").splitlines()
            self.assertEqual(len(lines), 1)
            payload = json.loads(lines[0])
            self.assertEqual(payload["provider"], "codex")
            self.assertEqual(
                payload["command"],
                'codex exec --model gpt-5.4 --sandbox read-only "review"',
            )
            self.assertEqual(payload["output_preview"], "first line\nsecond line")
            self.assertEqual(payload["exit_code"], 0)

    def test_log_cli_tools_detects_provider_from_absolute_cli_path(self) -> None:
        data = {
            "tool_name": "Bash",
            "tool_input": {
                "command": '/usr/local/bin/codex exec --model gpt-5.4 "review"'
            },
            "tool_response": {"stdout": "ok"},
        }

        record = log_cli_tools.build_log_record(data)

        self.assertIsNotNone(record)
        self.assertEqual(record["provider"], "codex")

    def test_log_cli_tools_detects_provider_inside_shell_wrapper(self) -> None:
        data = {
            "tool_name": "Bash",
            "tool_input": {"command": "bash -lc 'C:/Tools/gemini.exe -p \"Research\"'"},
            "tool_response": {"stdout": "ok"},
        }

        record = log_cli_tools.build_log_record(data)

        self.assertIsNotNone(record)
        self.assertEqual(record["provider"], "gemini")

    def test_log_cli_tools_detects_provider_inside_cmd_wrapper(self) -> None:
        data = {
            "tool_name": "Bash",
            "tool_input": {
                "command": 'cmd.exe /c "C:\\Tools\\gemini.exe -p \\"Research\\""'
            },
            "tool_response": {"stdout": "ok"},
        }

        record = log_cli_tools.build_log_record(data)

        self.assertIsNotNone(record)
        self.assertEqual(record["provider"], "gemini")

    def test_log_cli_tools_does_not_treat_provider_name_in_arguments_as_cli_invocation(
        self,
    ) -> None:
        for command in ("echo codex", "printf gemini"):
            data = {
                "tool_name": "Bash",
                "tool_input": {"command": command},
                "tool_response": {"stdout": "ok"},
            }
            self.assertIsNone(log_cli_tools.build_log_record(data), command)

    def test_log_cli_tools_ignores_non_cli_commands(self) -> None:
        data = {
            "tool_name": "Bash",
            "tool_input": {"command": "cargo make ci"},
            "tool_response": {"stdout": "ok"},
        }

        with tempfile.TemporaryDirectory() as tmp_dir:
            with mock.patch.dict(
                os.environ,
                {
                    "CLAUDE_LOG_CLI_TOOLS": "1",
                    "CLAUDE_PROJECT_DIR": tmp_dir,
                },
                clear=False,
            ):
                with mock.patch.object(
                    log_cli_tools, "load_stdin_json", return_value=data
                ):
                    with self.assertRaises(SystemExit) as exc:
                        log_cli_tools.main()

            self.assertEqual(exc.exception.code, 0)
            log_path = Path(tmp_dir) / ".claude" / "logs" / "cli-tools.jsonl"
            self.assertFalse(log_path.exists())

    def test_log_cli_tools_rotation_creates_numbered_generations(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            log_path = os.path.join(tmp_dir, "cli-tools.jsonl")
            # Write content exceeding MAX_LOG_SIZE
            with open(log_path, "w") as fh:
                fh.write("x" * (log_cli_tools.MAX_LOG_SIZE + 1))

            log_cli_tools.rotate_log_if_needed(log_path)

            self.assertFalse(os.path.exists(log_path))
            self.assertTrue(os.path.exists(log_path + ".1"))

    def test_log_cli_tools_rotation_shifts_existing_generations(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            log_path = os.path.join(tmp_dir, "cli-tools.jsonl")
            # Pre-create generation .1
            with open(log_path + ".1", "w") as fh:
                fh.write("gen1")
            # Write oversized current log
            with open(log_path, "w") as fh:
                fh.write("y" * (log_cli_tools.MAX_LOG_SIZE + 1))

            log_cli_tools.rotate_log_if_needed(log_path)

            self.assertFalse(os.path.exists(log_path))
            # .1 is the new rotation, .2 is the old .1
            self.assertTrue(os.path.exists(log_path + ".1"))
            self.assertTrue(os.path.exists(log_path + ".2"))
            with open(log_path + ".2") as fh:
                self.assertEqual(fh.read(), "gen1")

    def test_log_cli_tools_rotation_drops_oldest_beyond_max(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            log_path = os.path.join(tmp_dir, "cli-tools.jsonl")
            # Pre-create generations up to max
            for gen in range(1, log_cli_tools.MAX_LOG_GENERATIONS + 1):
                with open(f"{log_path}.{gen}", "w") as fh:
                    fh.write(f"gen{gen}")
            # Write oversized current log
            with open(log_path, "w") as fh:
                fh.write("z" * (log_cli_tools.MAX_LOG_SIZE + 1))

            log_cli_tools.rotate_log_if_needed(log_path)

            self.assertFalse(os.path.exists(log_path))
            self.assertTrue(os.path.exists(log_path + ".1"))
            # Oldest generation should have been dropped
            max_gen = log_cli_tools.MAX_LOG_GENERATIONS
            self.assertFalse(
                os.path.exists(f"{log_path}.{max_gen + 1}"),
                f"Generation {max_gen + 1} should not exist after rotation",
            )

    def test_log_cli_tools_detects_provider_in_compound_shell_command(self) -> None:
        """Detect CLI provider after ; or && in a shell -c payload."""
        compound_cases = [
            ("bash -c 'echo hi; codex exec task'", "codex"),
            ('bash -c "ls && gemini -p query"', "gemini"),
            ("sh -c 'true || codex run task'", "codex"),
            ("bash -c 'echo a | gemini -p b'", "gemini"),
            ("bash -lc 'cd /tmp && codex exec --full-auto task'", "codex"),
        ]
        for command, expected_provider in compound_cases:
            data = {
                "tool_name": "Bash",
                "tool_input": {"command": command},
                "tool_response": {"stdout": "ok"},
            }
            record = log_cli_tools.build_log_record(data)
            self.assertIsNotNone(record, f"Expected provider for: {command}")
            self.assertEqual(
                record["provider"],
                expected_provider,
                f"Wrong provider for: {command}",
            )

    def test_log_cli_tools_compound_command_without_cli_returns_none(self) -> None:
        """Compound commands without CLI tools should not produce a record."""
        data = {
            "tool_name": "Bash",
            "tool_input": {"command": "bash -c 'echo hi; ls -la && cat file'"},
            "tool_response": {"stdout": "ok"},
        }
        self.assertIsNone(log_cli_tools.build_log_record(data))

    def test_log_cli_tools_ignores_separator_inside_quotes(self) -> None:
        """Separators inside quoted strings should not split the payload."""
        # The inner python -c contains a semicolon inside quotes — should not
        # cause a false split.  The real codex invocation is after the outer ;.
        data = {
            "tool_name": "Bash",
            "tool_input": {
                "command": """bash -c 'python3 -c "print(1);"; codex exec task'"""
            },
            "tool_response": {"stdout": "ok"},
        }
        record = log_cli_tools.build_log_record(data)
        self.assertIsNotNone(record)
        self.assertEqual(record["provider"], "codex")

    def test_lint_on_save_emits_post_tool_use_output(self) -> None:
        data = {
            "tool_name": "Edit",
            "tool_input": {"file_path": "/repo/libs/domain/src/lib.rs"},
        }

        stdout = io.StringIO()
        with mock.patch.dict(lint_on_save.os.environ, {}, clear=True):
            with mock.patch.object(lint_on_save, "load_stdin_json", return_value=data):
                with mock.patch.object(
                    lint_on_save, "project_dir", return_value="/repo"
                ):
                    with mock.patch.object(
                        lint_on_save, "is_daemon_running", return_value=True
                    ):
                        with mock.patch.object(
                            lint_on_save,
                            "find_cargo_manifest",
                            return_value="/repo/libs/domain",
                        ):
                            with mock.patch.object(
                                lint_on_save,
                                "run_in_daemon",
                                side_effect=[
                                    (1, "", "broken format"),
                                    (0, "", ""),
                                ],
                            ):
                                with redirect_stdout(stdout):
                                    with self.assertRaises(SystemExit) as exc:
                                        lint_on_save.main()

        output = stdout.getvalue()
        self.assertEqual(exc.exception.code, 0)
        self.assertIn('"hookEventName": "PostToolUse"', output)
        self.assertIn("[lint-on-save] Issues in libs/domain/src/lib.rs", output)

    def test_lint_on_save_main_handles_windows_style_paths(self) -> None:
        data = {
            "tool_name": "Edit",
            "tool_input": {"file_path": r"C:\repo\libs\domain\src\lib.rs"},
        }
        stdout = io.StringIO()

        with mock.patch.dict(lint_on_save.os.environ, {}, clear=True):
            with mock.patch.object(lint_on_save, "load_stdin_json", return_value=data):
                with mock.patch.object(
                    lint_on_save, "project_dir", return_value=r"C:\repo"
                ):
                    with mock.patch.object(
                        lint_on_save, "is_daemon_running", return_value=True
                    ):
                        with mock.patch.object(
                            lint_on_save,
                            "resolve_project_path",
                            return_value=Path("/repo/libs/domain/src/lib.rs"),
                        ):
                            with mock.patch.object(
                                lint_on_save,
                                "find_cargo_manifest",
                                return_value=r"C:\repo\libs\domain",
                            ):
                                with mock.patch.object(
                                    lint_on_save,
                                    "run_in_daemon",
                                    side_effect=[
                                        (1, "", "broken format"),
                                        (0, "", ""),
                                    ],
                                ) as run_in_daemon:
                                    with redirect_stdout(stdout):
                                        with self.assertRaises(SystemExit) as exc:
                                            lint_on_save.main()

        self.assertEqual(exc.exception.code, 0)
        output = stdout.getvalue()
        self.assertIn("[lint-on-save] Issues in libs/domain/src/lib.rs", output)
        run_in_daemon.assert_any_call(
            ["rustfmt", "/workspace/libs/domain/src/lib.rs"],
            workdir=r"C:\repo\libs\domain",
            project_dir=r"C:\repo",
            timeout=lint_on_save._RUSTFMT_TIMEOUT,
        )
        run_in_daemon.assert_any_call(
            ["cargo", "check", "--all-targets"],
            workdir=r"C:\repo\libs\domain",
            project_dir=r"C:\repo",
            timeout=lint_on_save._CLIPPY_TIMEOUT,
        )

    def test_lint_on_save_skips_when_lock_held(self) -> None:
        """When the file lock is already held, main() should exit(0) without running lint."""
        data = {"tool_name": "Edit", "tool_input": {"file_path": "/repo/src/lib.rs"}}

        with mock.patch.dict(lint_on_save.os.environ, {}, clear=True):
            with mock.patch.object(lint_on_save, "load_stdin_json", return_value=data):
                with mock.patch.object(
                    lint_on_save, "project_dir", return_value="/repo"
                ):
                    with mock.patch.object(
                        lint_on_save, "is_daemon_running", return_value=True
                    ):
                        with mock.patch.object(
                            lint_on_save, "run_in_daemon"
                        ) as run_in_daemon:
                            # Simulate lock already held by raising BlockingIOError
                            real_open = builtins.open

                            def fake_open(path, *a, **kw):
                                if "claude-lint-on-save" in str(path):
                                    raise OSError("lock held")
                                return real_open(path, *a, **kw)

                            with mock.patch("builtins.open", side_effect=fake_open):
                                with self.assertRaises(SystemExit) as exc:
                                    lint_on_save.main()

        self.assertEqual(exc.exception.code, 0)
        run_in_daemon.assert_not_called()

    def test_lint_on_save_lock_file_is_project_scoped(self) -> None:
        path_a = lint_on_save._lock_file_for_project("/project-a")
        path_b = lint_on_save._lock_file_for_project("/project-b")
        self.assertNotEqual(path_a, path_b)
        self.assertIn("claude-lint-on-save", path_a)

    def test_lint_on_save_lock_file_normalizes_paths(self) -> None:
        base = lint_on_save._lock_file_for_project("/repo")
        self.assertEqual(base, lint_on_save._lock_file_for_project("/repo/"))
        self.assertEqual(base, lint_on_save._lock_file_for_project("/repo/."))
        self.assertEqual(base, lint_on_save._lock_file_for_project("/repo/../repo"))

    def test_lint_on_save_skips_files_outside_project_root(self) -> None:
        data = {"tool_name": "Edit", "tool_input": {"file_path": "/tmp/outside.rs"}}

        with mock.patch.dict(lint_on_save.os.environ, {}, clear=True):
            with mock.patch.object(lint_on_save, "load_stdin_json", return_value=data):
                with mock.patch.object(
                    lint_on_save, "project_dir", return_value="/repo"
                ):
                    with mock.patch.object(
                        lint_on_save, "is_daemon_running", return_value=True
                    ):
                        with mock.patch.object(
                            lint_on_save, "run_in_daemon"
                        ) as run_in_daemon:
                            with self.assertRaises(SystemExit) as exc:
                                lint_on_save.main()

        self.assertEqual(exc.exception.code, 0)
        run_in_daemon.assert_not_called()

    def test_python_lint_on_save_ignores_non_python_files(self) -> None:
        data = {
            "tool_name": "Edit",
            "tool_input": {"file_path": "/repo/libs/domain/src/lib.rs"},
        }

        with mock.patch.dict(python_lint_on_save.os.environ, {}, clear=True):
            with mock.patch.object(
                python_lint_on_save, "load_stdin_json", return_value=data
            ):
                with self.assertRaises(SystemExit) as exc:
                    python_lint_on_save.main()

        self.assertEqual(exc.exception.code, 0)

    def test_python_lint_on_save_ignores_non_edit_write_tools(self) -> None:
        data = {
            "tool_name": "Bash",
            "tool_input": {"file_path": "/repo/scripts/check.py"},
        }

        with mock.patch.dict(python_lint_on_save.os.environ, {}, clear=True):
            with mock.patch.object(
                python_lint_on_save, "load_stdin_json", return_value=data
            ):
                with self.assertRaises(SystemExit) as exc:
                    python_lint_on_save.main()

        self.assertEqual(exc.exception.code, 0)

    def test_python_lint_on_save_skips_files_outside_project(self) -> None:
        data = {"tool_name": "Edit", "tool_input": {"file_path": "/tmp/outside.py"}}

        with mock.patch.dict(python_lint_on_save.os.environ, {}, clear=True):
            with mock.patch.object(
                python_lint_on_save, "load_stdin_json", return_value=data
            ):
                with mock.patch.object(
                    python_lint_on_save, "project_dir", return_value="/repo"
                ):
                    with mock.patch.object(
                        python_lint_on_save, "find_ruff", return_value="/usr/bin/ruff"
                    ):
                        with self.assertRaises(SystemExit) as exc:
                            python_lint_on_save.main()

        self.assertEqual(exc.exception.code, 0)

    def test_python_lint_on_save_emits_output_on_ruff_failure(self) -> None:
        data = {
            "tool_name": "Edit",
            "tool_input": {"file_path": "/repo/scripts/check.py"},
        }

        stdout = io.StringIO()
        with mock.patch.dict(python_lint_on_save.os.environ, {}, clear=True):
            with mock.patch.object(
                python_lint_on_save, "load_stdin_json", return_value=data
            ):
                with mock.patch.object(
                    python_lint_on_save, "project_dir", return_value="/repo"
                ):
                    with mock.patch.object(
                        python_lint_on_save, "find_ruff", return_value="/usr/bin/ruff"
                    ):
                        ruff_result = mock.Mock(
                            returncode=1,
                            stdout="scripts/check.py:5:1: F841 unused variable\n",
                            stderr="",
                        )
                        with mock.patch("subprocess.run", return_value=ruff_result):
                            with redirect_stdout(stdout):
                                with self.assertRaises(SystemExit) as exc:
                                    python_lint_on_save.main()

        output = stdout.getvalue()
        self.assertEqual(exc.exception.code, 0)
        payload = json.loads(output)
        self.assertIn(
            "[python-lint]", payload["hookSpecificOutput"]["additionalContext"]
        )

    def test_python_lint_on_save_emits_stderr_when_stdout_empty(self) -> None:
        data = {
            "tool_name": "Edit",
            "tool_input": {"file_path": "/repo/scripts/check.py"},
        }

        stdout = io.StringIO()
        with mock.patch.dict(python_lint_on_save.os.environ, {}, clear=True):
            with mock.patch.object(
                python_lint_on_save, "load_stdin_json", return_value=data
            ):
                with mock.patch.object(
                    python_lint_on_save, "project_dir", return_value="/repo"
                ):
                    with mock.patch.object(
                        python_lint_on_save, "find_ruff", return_value="/usr/bin/ruff"
                    ):
                        ruff_result = mock.Mock(
                            returncode=1, stdout="", stderr="ruff: internal error\n"
                        )
                        with mock.patch("subprocess.run", return_value=ruff_result):
                            with redirect_stdout(stdout):
                                with self.assertRaises(SystemExit) as exc:
                                    python_lint_on_save.main()

        output = stdout.getvalue()
        self.assertEqual(exc.exception.code, 0)
        payload = json.loads(output)
        self.assertIn(
            "internal error", payload["hookSpecificOutput"]["additionalContext"]
        )

    def test_python_lint_on_save_resolves_relative_paths(self) -> None:
        self.assertTrue(
            python_lint_on_save.is_path_in_project("scripts/check.py", "/repo")
        )
        self.assertFalse(
            python_lint_on_save.is_path_in_project("/tmp/outside.py", "/repo")
        )

    def test_python_lint_on_save_silent_when_ruff_passes(self) -> None:
        data = {
            "tool_name": "Edit",
            "tool_input": {"file_path": "/repo/scripts/check.py"},
        }

        stdout = io.StringIO()
        with mock.patch.dict(python_lint_on_save.os.environ, {}, clear=True):
            with mock.patch.object(
                python_lint_on_save, "load_stdin_json", return_value=data
            ):
                with mock.patch.object(
                    python_lint_on_save, "project_dir", return_value="/repo"
                ):
                    with mock.patch.object(
                        python_lint_on_save, "find_ruff", return_value="/usr/bin/ruff"
                    ):
                        ruff_result = mock.Mock(returncode=0, stdout="", stderr="")
                        with mock.patch("subprocess.run", return_value=ruff_result):
                            with redirect_stdout(stdout):
                                with self.assertRaises(SystemExit) as exc:
                                    python_lint_on_save.main()

        self.assertEqual(exc.exception.code, 0)
        self.assertEqual(stdout.getvalue(), "")

    def test_python_lint_on_save_skips_when_ruff_not_found(self) -> None:
        data = {
            "tool_name": "Edit",
            "tool_input": {"file_path": "/repo/scripts/check.py"},
        }

        stdout = io.StringIO()
        with mock.patch.dict(python_lint_on_save.os.environ, {}, clear=True):
            with mock.patch.object(
                python_lint_on_save, "load_stdin_json", return_value=data
            ):
                with mock.patch.object(
                    python_lint_on_save, "project_dir", return_value="/repo"
                ):
                    with mock.patch.object(
                        python_lint_on_save, "find_ruff", return_value=None
                    ):
                        with redirect_stdout(stdout):
                            with self.assertRaises(SystemExit) as exc:
                                python_lint_on_save.main()

        self.assertEqual(exc.exception.code, 0)
        self.assertEqual(stdout.getvalue(), "")


if __name__ == "__main__":
    unittest.main()
