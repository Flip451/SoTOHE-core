import io
import os
import tempfile
import unittest
from contextlib import redirect_stdout
from unittest import mock

from test_helpers import load_hook_module, write_agent_profiles

shared = load_hook_module("_shared")
check_codex_before_write = load_hook_module("check-codex-before-write")
suggest_gemini_research = load_hook_module("suggest-gemini-research")


class PreToolHooksTest(unittest.TestCase):
    def with_profile(self, active_profile: str, mutator=None):
        temp_dir = tempfile.TemporaryDirectory()
        self.addCleanup(temp_dir.cleanup)
        config_path = write_agent_profiles(
            temp_dir.name + "/agent-profiles.json", active_profile, mutator
        )
        return mock.patch.dict(
            os.environ, {"CLAUDE_AGENT_PROFILES_PATH": str(config_path)}
        )

    def test_check_codex_before_write_reads_structured_content(self) -> None:
        tool_input_data = {
            "file_path": "libs/domain/src/lib.rs",
            "content": [{"type": "text", "text": "pub trait UserRepo {}"}],
        }

        content = shared.tool_input_text(tool_input_data, "content", "new_string")
        should_suggest, reason = check_codex_before_write.should_suggest_design_review(
            tool_input_data["file_path"], content
        )

        self.assertTrue(should_suggest)
        self.assertEqual(
            reason,
            check_codex_before_write.CONTENT_REASON_TEMPLATE.format(
                indicator="pub trait "
            ),
        )

    def test_check_codex_before_write_uses_path_reason_when_content_is_generic(
        self,
    ) -> None:
        should_suggest, reason = check_codex_before_write.should_suggest_design_review(
            "libs/domain/src/lib.rs",
            "let x = 1;",
        )

        self.assertTrue(should_suggest)
        self.assertEqual(
            reason,
            check_codex_before_write.PATH_REASON_TEMPLATE.format(indicator="domain/"),
        )

    def test_suggest_gemini_research_uses_constant_message(self) -> None:
        query = "latest rust crates"
        self.assertEqual(
            suggest_gemini_research.build_research_suggestion(query),
            suggest_gemini_research.RESEARCH_SUGGESTION_TEMPLATE.format(
                prefix=suggest_gemini_research.RESEARCH_SUGGESTION_PREFIX,
                query=query,
                capability=suggest_gemini_research.RESEARCH_CAPABILITY,
                provider_label="Gemini CLI",
                provider_example='gemini -p "Research: latest rust crates"',
            ),
        )

    def test_suggest_gemini_research_escapes_shell_sensitive_query_in_example(
        self,
    ) -> None:
        query = 'latest "; rm -rf / #'
        message = suggest_gemini_research.build_research_suggestion(query)

        self.assertIn('gemini -p "Research: latest \\"; rm -rf / #"', message)
        self.assertNotIn("2>/dev/null", message)

    def test_check_codex_before_write_message_uses_active_planner_provider(
        self,
    ) -> None:
        with self.with_profile("claude-heavy"):
            message = check_codex_before_write.build_consultation_message("reason")

        self.assertIn("Claude Code", message)
        self.assertIn("/track:plan <feature>", message)
        self.assertNotIn("codex exec", message)

    def test_check_codex_before_write_is_silent_during_takt_session(self) -> None:
        data = {
            "tool_input": {
                "file_path": "libs/domain/src/lib.rs",
                "content": "pub trait UserRepo {}",
            }
        }
        stdout = io.StringIO()

        with mock.patch.dict(os.environ, {"TAKT_SESSION": "1"}, clear=False):
            with mock.patch.object(
                check_codex_before_write, "load_stdin_json", return_value=data
            ):
                with redirect_stdout(stdout):
                    with self.assertRaises(SystemExit) as exc:
                        check_codex_before_write.main()

        self.assertEqual(exc.exception.code, 0)
        self.assertEqual(stdout.getvalue(), "")

    def test_suggest_gemini_research_switches_to_active_research_provider(self) -> None:
        with self.with_profile("codex-heavy"):
            message = suggest_gemini_research.build_research_suggestion(
                "latest rust crates"
            )

        self.assertIn("Codex CLI", message)
        self.assertIn("Research this Rust topic: latest rust crates", message)
        self.assertNotIn('gemini -p "Research: latest rust crates"', message)


if __name__ == "__main__":
    unittest.main()
