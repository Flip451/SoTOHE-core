import tempfile
import unittest
from pathlib import Path

from test_helpers import load_hook_module, write_agent_profiles

agent_profiles = load_hook_module("_agent_profiles")


class AgentProfilesTest(unittest.TestCase):
    def write_mutated_profiles(self, mutator, active_profile: str = "default") -> Path:
        temp_dir = tempfile.TemporaryDirectory()
        self.addCleanup(temp_dir.cleanup)
        path = Path(temp_dir.name) / "agent-profiles.json"
        return write_agent_profiles(path, active_profile, mutator)

    def test_default_profile_resolves_all_required_capabilities(self) -> None:
        """Verify all capabilities resolve to a valid provider.

        Critical mappings (reviewer, debugger, workflow_host) are pinned because
        the review pipeline fail-closes on non-Codex reviewers. Flexible mappings
        (planner, implementer, researcher, etc.) only check validity.
        """
        profiles = agent_profiles.load_profiles()

        self.assertEqual(agent_profiles.active_profile_name(profiles), "default")

        # Critical mappings — pipeline invariants
        self.assertEqual(
            agent_profiles.resolve_provider("reviewer", profiles=profiles), "codex"
        )
        self.assertEqual(
            agent_profiles.resolve_provider("debugger", profiles=profiles), "codex"
        )
        self.assertEqual(
            agent_profiles.workflow_host_provider(profiles=profiles), "claude"
        )

        # Flexible mappings — any valid provider is OK
        for cap in agent_profiles.REQUIRED_CAPABILITIES:
            provider = agent_profiles.resolve_provider(cap, profiles=profiles)
            self.assertIn(
                provider,
                ("claude", "codex", "gemini"),
                f"capability '{cap}' resolved to unexpected provider '{provider}'",
            )

    def test_provider_label_and_example_are_non_empty_for_all_capabilities(self) -> None:
        """Verify label/example resolution works for all capabilities without hardcoding values."""
        profiles = agent_profiles.load_profiles()

        for cap in agent_profiles.REQUIRED_CAPABILITIES:
            label = agent_profiles.provider_label(cap, profiles=profiles)
            self.assertTrue(label, f"provider_label for '{cap}' should be non-empty")
            example = agent_profiles.provider_example(cap, profiles=profiles)
            self.assertTrue(example, f"provider_example for '{cap}' should be non-empty")

        # Workflow host fields must always be populated
        self.assertTrue(agent_profiles.workflow_host_provider(profiles=profiles))
        self.assertTrue(agent_profiles.workflow_host_model(profiles=profiles))
        self.assertTrue(agent_profiles.workflow_host_label(profiles=profiles))

    def test_load_profiles_rejects_missing_required_capability(self) -> None:
        def mutator(profiles: dict) -> None:
            del profiles["profiles"]["default"]["multimodal_reader"]

        path = self.write_mutated_profiles(mutator)

        with self.assertRaisesRegex(
            agent_profiles.AgentProfilesError, "missing required capabilities"
        ):
            agent_profiles.load_profiles(path)

    def test_load_profiles_rejects_unknown_provider_reference(self) -> None:
        def mutator(profiles: dict) -> None:
            profiles["profiles"]["default"]["multimodal_reader"] = "unknown"

        path = self.write_mutated_profiles(mutator)

        with self.assertRaisesRegex(
            agent_profiles.AgentProfilesError, "unknown provider 'unknown'"
        ):
            agent_profiles.load_profiles(path)

    def test_load_profiles_rejects_unsupported_capability_assignment(self) -> None:
        def mutator(profiles: dict) -> None:
            profiles["profiles"]["default"]["multimodal_reader"] = "claude"

        path = self.write_mutated_profiles(mutator)

        with self.assertRaisesRegex(
            agent_profiles.AgentProfilesError, "does not support capability"
        ):
            agent_profiles.load_profiles(path)

    def test_load_profiles_rejects_non_claude_orchestrator(self) -> None:
        def mutator(profiles: dict) -> None:
            profiles["profiles"]["default"]["orchestrator"] = "codex"

        path = self.write_mutated_profiles(mutator)

        with self.assertRaisesRegex(
            agent_profiles.AgentProfilesError, "must use 'claude'"
        ):
            agent_profiles.load_profiles(path)

    def test_load_profiles_rejects_unsupported_workflow_host_provider(self) -> None:
        def mutator(profiles: dict) -> None:
            profiles["profiles"]["default"]["workflow_host_provider"] = "gemini"

        path = self.write_mutated_profiles(mutator)

        with self.assertRaisesRegex(
            agent_profiles.AgentProfilesError, "must be one of"
        ):
            agent_profiles.load_profiles(path)

    def test_load_profiles_rejects_missing_workflow_host_model(self) -> None:
        def mutator(profiles: dict) -> None:
            del profiles["profiles"]["default"]["workflow_host_model"]

        path = self.write_mutated_profiles(mutator)

        with self.assertRaisesRegex(
            agent_profiles.AgentProfilesError, "workflow_host_model"
        ):
            agent_profiles.load_profiles(path)

    def test_provider_example_falls_back_to_default_template(self) -> None:
        def mutator(profiles: dict) -> None:
            gemini = profiles["providers"]["gemini"]
            del gemini["invoke_examples"]["researcher"]
            gemini["supported_capabilities"] = ["researcher", "multimodal_reader"]

        path = self.write_mutated_profiles(mutator)

        self.assertEqual(
            agent_profiles.provider_example("researcher", path=path),
            'gemini -p "Research: {task}"',
        )

    def test_render_provider_example_replaces_task_and_path_tokens(self) -> None:
        profiles = agent_profiles.load_profiles()

        rendered = agent_profiles.render_provider_example(
            "multimodal_reader",
            task="summarize this spec",
            file_path="docs/spec.pdf",
            profiles=profiles,
        )

        self.assertEqual(
            rendered, 'gemini -p "Extract from docs/spec.pdf: summarize this spec"'
        )

    def test_render_provider_example_escapes_shell_sensitive_task_text(self) -> None:
        profiles = agent_profiles.load_profiles()

        rendered = agent_profiles.render_provider_example(
            "researcher",
            task='latest "; rm -rf / #',
            profiles=profiles,
        )

        self.assertEqual(rendered, 'gemini -p "Research: latest \\"; rm -rf / #"')

    def test_render_provider_example_quotes_unquoted_path_placeholders(self) -> None:
        def mutator(profiles: dict) -> None:
            profiles["providers"]["gemini"]["invoke_examples"]["multimodal_reader"] = (
                "gemini --file {path}"
            )

        path = self.write_mutated_profiles(mutator)

        rendered = agent_profiles.render_provider_example(
            "multimodal_reader",
            file_path="docs/spec with space.pdf",
            path=path,
        )

        self.assertEqual(rendered, "gemini --file 'docs/spec with space.pdf'")

    def test_render_provider_example_preserves_literal_placeholder_text_in_task(
        self,
    ) -> None:
        profiles = agent_profiles.load_profiles()

        rendered = agent_profiles.render_provider_example(
            "researcher",
            task="show literal {path}",
            profiles=profiles,
        )

        self.assertEqual(rendered, 'gemini -p "Research: show literal {path}"')

    def test_provider_command_prefixes_include_shell_and_slash_commands_only(
        self,
    ) -> None:
        def mutator(profiles: dict) -> None:
            profiles["providers"]["inactive"] = {
                "label": "Inactive CLI",
                "supported_capabilities": ["researcher"],
                "invoke_examples": {
                    "researcher": 'inactivecli run "{task}"',
                },
            }

        path = self.write_mutated_profiles(mutator)

        prefixes = agent_profiles.provider_command_prefixes(path=path)

        self.assertIn("/track:status", prefixes)
        self.assertIn("codex", prefixes)
        self.assertIn("gemini", prefixes)
        self.assertNotIn("inactivecli", prefixes)
        self.assertNotIn("Continue", prefixes)

    # ----------------------------------------------------------------
    # Task A: Model injection tests
    # ----------------------------------------------------------------

    def test_load_profiles_accepts_provider_with_default_model(self) -> None:
        """Provider may declare default_model as a non-empty string."""

        def mutator(profiles: dict) -> None:
            profiles["providers"]["codex"]["default_model"] = "gpt-5.4"

        path = self.write_mutated_profiles(mutator)
        loaded = agent_profiles.load_profiles(path)
        codex = loaded["providers"]["codex"]
        self.assertEqual(codex["default_model"], "gpt-5.4")

    def test_load_profiles_accepts_profile_with_model_overrides(self) -> None:
        """Profile may declare provider_model_overrides as dict[str, str]."""

        def mutator(profiles: dict) -> None:
            profiles["providers"]["codex"]["default_model"] = "gpt-5.4"
            profiles["profiles"]["default"]["provider_model_overrides"] = {
                "codex": "gpt-5.4"
            }

        path = self.write_mutated_profiles(mutator)
        loaded = agent_profiles.load_profiles(path)
        overrides = loaded["profiles"]["default"].get("provider_model_overrides")
        self.assertEqual(overrides, {"codex": "gpt-5.4"})

    def test_load_profiles_rejects_non_string_default_model(self) -> None:
        def mutator(profiles: dict) -> None:
            profiles["providers"]["codex"]["default_model"] = 123

        path = self.write_mutated_profiles(mutator)
        with self.assertRaisesRegex(
            agent_profiles.AgentProfilesError, "default_model.*string"
        ):
            agent_profiles.load_profiles(path)

    def test_load_profiles_rejects_empty_default_model(self) -> None:
        def mutator(profiles: dict) -> None:
            profiles["providers"]["codex"]["default_model"] = "   "

        path = self.write_mutated_profiles(mutator)
        with self.assertRaisesRegex(
            agent_profiles.AgentProfilesError, "default_model.*empty"
        ):
            agent_profiles.load_profiles(path)

    def test_load_profiles_rejects_non_dict_model_overrides(self) -> None:
        def mutator(profiles: dict) -> None:
            profiles["profiles"]["default"]["provider_model_overrides"] = "gpt-5.4"

        path = self.write_mutated_profiles(mutator)
        with self.assertRaisesRegex(
            agent_profiles.AgentProfilesError, "provider_model_overrides.*dict"
        ):
            agent_profiles.load_profiles(path)

    def test_load_profiles_rejects_non_string_model_override_value(self) -> None:
        def mutator(profiles: dict) -> None:
            profiles["profiles"]["default"]["provider_model_overrides"] = {"codex": 42}

        path = self.write_mutated_profiles(mutator)
        with self.assertRaisesRegex(
            agent_profiles.AgentProfilesError, "provider_model_overrides.*string"
        ):
            agent_profiles.load_profiles(path)

    def test_load_profiles_rejects_model_override_for_unknown_provider(self) -> None:
        def mutator(profiles: dict) -> None:
            profiles["profiles"]["default"]["provider_model_overrides"] = {
                "unknown_provider": "gpt-5.4"
            }

        path = self.write_mutated_profiles(mutator)
        with self.assertRaisesRegex(
            agent_profiles.AgentProfilesError, "unknown provider.*unknown_provider"
        ):
            agent_profiles.load_profiles(path)

    def test_resolve_provider_model_returns_provider_default(self) -> None:
        def mutator(profiles: dict) -> None:
            profiles["providers"]["codex"]["default_model"] = "gpt-5.4"

        path = self.write_mutated_profiles(mutator)
        model = agent_profiles.resolve_provider_model("codex", path=path)
        self.assertEqual(model, "gpt-5.4")

    def test_resolve_provider_model_profile_override_wins(self) -> None:
        def mutator(profiles: dict) -> None:
            profiles["providers"]["codex"]["default_model"] = "gpt-5.4"
            profiles["profiles"]["default"]["provider_model_overrides"] = {
                "codex": "gpt-5.4"
            }

        path = self.write_mutated_profiles(mutator)
        model = agent_profiles.resolve_provider_model("codex", path=path)
        self.assertEqual(model, "gpt-5.4")

    def test_resolve_provider_model_returns_none_when_no_model(self) -> None:
        """Provider without default_model and no override returns None."""
        path = self.write_mutated_profiles(lambda p: None)
        model = agent_profiles.resolve_provider_model("gemini", path=path)
        self.assertIsNone(model)

    def test_render_provider_example_injects_model_placeholder(self) -> None:
        def mutator(profiles: dict) -> None:
            profiles["providers"]["codex"]["default_model"] = "gpt-5.4"
            for key in profiles["providers"]["codex"]["invoke_examples"]:
                profiles["providers"]["codex"]["invoke_examples"][key] = profiles[
                    "providers"
                ]["codex"]["invoke_examples"][key].replace("gpt-5.4", "{model}")

        path = self.write_mutated_profiles(mutator)
        rendered = agent_profiles.render_provider_example(
            "reviewer", task="test task", path=path
        )
        self.assertIn("gpt-5.4", rendered)
        self.assertNotIn("{model}", rendered)

    def test_load_profiles_rejects_unresolvable_model_placeholder_at_validation(
        self,
    ) -> None:
        """Validation catches {model} in template with no default_model or override."""

        def mutator(profiles: dict) -> None:
            profiles["providers"]["codex"].pop("default_model", None)
            for key in profiles["providers"]["codex"]["invoke_examples"]:
                tpl = profiles["providers"]["codex"]["invoke_examples"][key]
                if "{model}" not in tpl:
                    profiles["providers"]["codex"]["invoke_examples"][key] = (
                        tpl.replace("gpt-5.4", "{model}")
                    )

        path = self.write_mutated_profiles(mutator)
        with self.assertRaisesRegex(
            agent_profiles.AgentProfilesError,
            r"model.*no model is configured|model.*not configured",
        ):
            agent_profiles.load_profiles(path)

    # ----------------------------------------------------------------
    # Backward compatibility: legacy config without default_model
    # ----------------------------------------------------------------

    def test_legacy_config_without_default_model_renders_hardcoded_model(self) -> None:
        """Old config with hardcoded model in invoke_examples (no default_model) still works."""

        def mutator(profiles: dict) -> None:
            profiles["providers"]["codex"].pop("default_model", None)
            for key in profiles["providers"]["codex"]["invoke_examples"]:
                tpl = profiles["providers"]["codex"]["invoke_examples"][key]
                profiles["providers"]["codex"]["invoke_examples"][key] = tpl.replace(
                    "{model}", "gpt-5.4"
                )

        path = self.write_mutated_profiles(mutator)
        rendered = agent_profiles.render_provider_example(
            "reviewer", task="test task", path=path
        )
        self.assertIn("gpt-5.4", rendered)
        self.assertNotIn("{model}", rendered)

    # ----------------------------------------------------------------
    # Security: model placeholder escaping
    # ----------------------------------------------------------------

    def test_render_model_escapes_shell_sensitive_characters(self) -> None:
        """Model value with shell metacharacters is single-quoted (safe)."""

        def mutator(profiles: dict) -> None:
            profiles["providers"]["codex"]["default_model"] = 'evil"; $(rm -rf /)'

        path = self.write_mutated_profiles(mutator)
        rendered = agent_profiles.render_provider_example(
            "reviewer", task="test task", path=path
        )
        # shlex.quote wraps in single quotes — safe against expansion
        self.assertIn("'evil", rendered)
        self.assertIn("evil", rendered)
        # Verify the dangerous value is NOT unquoted in the output
        self.assertNotIn('--model evil"', rendered)

    def test_render_model_escapes_backtick_injection(self) -> None:
        """Model value with backticks is single-quoted (safe against expansion)."""

        def mutator(profiles: dict) -> None:
            profiles["providers"]["codex"]["default_model"] = "model`whoami`end"

        path = self.write_mutated_profiles(mutator)
        rendered = agent_profiles.render_provider_example(
            "reviewer", task="test task", path=path
        )
        # shlex.quote wraps in single quotes — backticks are not expanded
        self.assertIn("'model`whoami`end'", rendered)

    def test_resolve_provider_model_strips_whitespace(self) -> None:
        """Model value with leading/trailing whitespace is trimmed."""

        def mutator(profiles: dict) -> None:
            profiles["providers"]["codex"]["default_model"] = "  gpt-5.4  "

        path = self.write_mutated_profiles(mutator)
        model = agent_profiles.resolve_provider_model("codex", path=path)
        self.assertEqual(model, "gpt-5.4")

    def test_load_profiles_accepts_null_provider_model_overrides(self) -> None:
        """Explicit null provider_model_overrides is treated as 'not set' (same as missing)."""

        def mutator(profiles: dict) -> None:
            profiles["profiles"]["default"]["provider_model_overrides"] = None

        path = self.write_mutated_profiles(mutator)
        loaded = agent_profiles.load_profiles(path)
        self.assertIsNone(loaded["profiles"]["default"].get("provider_model_overrides"))

    def test_load_profiles_accepts_null_default_model(self) -> None:
        """Explicit null default_model is treated as 'not set' (same as missing)."""

        def mutator(profiles: dict) -> None:
            profiles["providers"]["codex"]["default_model"] = None
            # Revert templates to hardcoded model so {model} check doesn't fail
            for key in profiles["providers"]["codex"]["invoke_examples"]:
                tpl = profiles["providers"]["codex"]["invoke_examples"][key]
                profiles["providers"]["codex"]["invoke_examples"][key] = tpl.replace(
                    "{model}", "gpt-5.4"
                )

        path = self.write_mutated_profiles(mutator)
        loaded = agent_profiles.load_profiles(path)
        self.assertIsNone(loaded["providers"]["codex"].get("default_model"))


if __name__ == "__main__":
    unittest.main()
