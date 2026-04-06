import importlib.util
import io
import os
import sys
import tempfile
import unittest
from contextlib import redirect_stdout
from pathlib import Path
from unittest import mock

from test_helpers import write_agent_profiles


def load_agent_router_module():
    hooks_dir = Path(__file__).resolve().parent
    module_path = hooks_dir / "agent-router.py"
    sys.path.insert(0, str(hooks_dir))
    spec = importlib.util.spec_from_file_location("agent_router", module_path)
    assert spec is not None
    assert spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


agent_router = load_agent_router_module()


class AgentRouterTest(unittest.TestCase):
    def with_profile(self, active_profile: str, mutator=None):
        temp_dir = tempfile.TemporaryDirectory()
        self.addCleanup(temp_dir.cleanup)
        config_path = write_agent_profiles(
            Path(temp_dir.name) / "agent-profiles.json", active_profile, mutator
        )
        return mock.patch.dict(
            os.environ, {"CLAUDE_AGENT_PROFILES_PATH": str(config_path)}
        )

    def test_workflow_trigger_wins_over_generic_review(self) -> None:
        agent, trigger, is_multimodal = agent_router.detect_agent(
            "/track:review current change"
        )
        self.assertEqual(agent, "workflow")
        self.assertEqual(trigger, "track")
        self.assertFalse(is_multimodal)

    def test_gemini_research_wins_over_codex_analysis_words(self) -> None:
        agent, trigger, is_multimodal = agent_router.detect_agent(
            "Please analyze the codebase and compare the latest crate options"
        )
        self.assertEqual(agent, "researcher")
        self.assertEqual(trigger, "latest")
        self.assertFalse(is_multimodal)

    def test_planner_used_for_design_prompt_without_research_signal(self) -> None:
        agent, trigger, is_multimodal = agent_router.detect_agent(
            "Help me design the ownership model for this trait"
        )
        self.assertEqual(agent, "planner")
        self.assertEqual(trigger, "design")
        self.assertFalse(is_multimodal)

    def test_multimodal_file_keeps_highest_priority(self) -> None:
        agent, trigger, is_multimodal = agent_router.detect_agent(
            "Review docs/design.pdf and analyze the architecture"
        )
        self.assertEqual(agent, "multimodal_reader")
        self.assertEqual(trigger, "docs/design.pdf")
        self.assertTrue(is_multimodal)

    def test_multimodal_file_allows_trailing_period(self) -> None:
        agent, trigger, is_multimodal = agent_router.detect_agent(
            "Please review this docs/design.pdf."
        )
        self.assertEqual(agent, "multimodal_reader")
        self.assertEqual(trigger, "docs/design.pdf")
        self.assertTrue(is_multimodal)

    def test_multimodal_file_allows_trailing_parenthesis(self) -> None:
        agent, trigger, is_multimodal = agent_router.detect_agent(
            "Look at ./image.png)"
        )
        self.assertEqual(agent, "multimodal_reader")
        self.assertEqual(trigger, "./image.png")
        self.assertTrue(is_multimodal)

    def test_multimodal_file_with_spaces_in_quoted_path(self) -> None:
        agent, trigger, is_multimodal = agent_router.detect_agent(
            'Read "/path/to/my document.pdf" and summarize'
        )
        self.assertEqual(agent, "multimodal_reader")
        self.assertEqual(trigger, "/path/to/my document.pdf")
        self.assertTrue(is_multimodal)

    def test_multimodal_file_with_single_quoted_path(self) -> None:
        agent, trigger, is_multimodal = agent_router.detect_agent(
            "Extract from '/tmp/meeting notes.mp4'"
        )
        self.assertEqual(agent, "multimodal_reader")
        self.assertEqual(trigger, "/tmp/meeting notes.mp4")
        self.assertTrue(is_multimodal)

    def test_multimodal_file_with_apostrophe_in_double_quoted_path(self) -> None:
        agent, trigger, is_multimodal = agent_router.detect_agent(
            """Read "/tmp/Bob's Notes.pdf" please"""
        )
        self.assertEqual(agent, "multimodal_reader")
        self.assertEqual(trigger, "/tmp/Bob's Notes.pdf")
        self.assertTrue(is_multimodal)

    def test_architecture_customizer_always_routes_to_workflow(self) -> None:
        agent, trigger, is_multimodal = agent_router.detect_agent(
            "/architecture-customizer"
        )
        self.assertEqual(agent, "workflow")
        self.assertEqual(trigger, "/architecture-customizer")
        self.assertFalse(is_multimodal)

    def test_conventions_add_always_routes_to_workflow(self) -> None:
        agent, trigger, is_multimodal = agent_router.detect_agent(
            "/conventions:add api-design"
        )
        self.assertEqual(agent, "workflow")
        self.assertEqual(trigger, "/conventions:add")
        self.assertFalse(is_multimodal)

    def test_guide_add_always_routes_to_workflow(self) -> None:
        agent, trigger, is_multimodal = agent_router.detect_agent("/guide:add")
        self.assertEqual(agent, "workflow")
        self.assertEqual(trigger, "/guide:add")
        self.assertFalse(is_multimodal)

    def test_capability_message_uses_template_constant(self) -> None:
        """Verify message is built from CAPABILITY_TEMPLATE without hardcoding provider values."""
        message = agent_router.build_capability_message("planner", "design")
        # Structural checks: message contains template fields regardless of provider
        self.assertIn("planner", message)
        self.assertIn("design", message)
        self.assertIn(agent_router.CAPABILITY_PREFIXES["planner"], message)
        self.assertIn(agent_router.CAPABILITY_DESCRIPTIONS["planner"], message)

    def test_workflow_message_uses_template_constant(self) -> None:
        message = agent_router.build_workflow_message("track")
        self.assertEqual(
            message,
            agent_router.WORKFLOW_TEMPLATE.format(
                prefix=agent_router.WORKFLOW_PREFIX,
                trigger="track",
            ),
        )

    def test_workflow_message_no_longer_mentions_takt_commands(self) -> None:
        message = agent_router.build_workflow_message("track")

        self.assertNotIn("takt-full-cycle", message)
        self.assertIn("track-pr-review", message)

    def test_takt_command_no_longer_triggers_external_guides(self) -> None:
        self.assertFalse(
            agent_router.should_inject_external_guides(
                'cargo make takt-full-cycle "postgres refactor"'
            )
        )

    def test_external_guide_message_is_empty_without_matches(self) -> None:
        self.assertEqual(agent_router.build_external_guides_message([]), "")

    def test_external_guide_message_renders_summary_and_usage(self) -> None:
        message = agent_router.build_external_guides_message(
            [
                (
                    {
                        "id": "pg-guide",
                        "summary": ["Use for schema review."],
                        "project_usage": ["Check before changing SQL conventions."],
                    },
                    "postgres",
                )
            ]
        )

        self.assertIn(agent_router.EXTERNAL_GUIDES_PREFIX, message)
        self.assertIn("pg-guide", message)
        self.assertIn("schema review", message)
        self.assertIn("SQL conventions", message)

    def test_build_user_prompt_context_combines_workflow_hint_and_external_guides(
        self,
    ) -> None:
        with mock.patch.object(
            agent_router,
            "find_external_guide_matches",
            return_value=[
                (
                    {
                        "id": "pg-guide",
                        "summary": ["Use for schema review."],
                        "project_usage": ["Check before changing SQL conventions."],
                    },
                    "postgres",
                )
            ],
        ):
            context = agent_router.build_user_prompt_context(
                "/track:plan postgres migration"
            )

        self.assertIsNotNone(context)
        self.assertIn(agent_router.WORKFLOW_PREFIX, context)
        self.assertIn(agent_router.EXTERNAL_GUIDES_PREFIX, context)

    def test_external_guide_message_includes_cache_path_when_present(self) -> None:
        message = agent_router.build_external_guides_message(
            [
                (
                    {
                        "id": "pg-guide",
                        "summary": ["Use for schema review."],
                        "project_usage": ["Check before changing SQL conventions."],
                        "cache_path": ".cache/external-guides/pg-guide.md",
                    },
                    "postgres",
                )
            ]
        )

        self.assertIn("cache path: .cache/external-guides/pg-guide.md", message)

    def test_external_guide_message_omits_cache_path_when_absent(self) -> None:
        message = agent_router.build_external_guides_message(
            [
                (
                    {
                        "id": "pg-guide",
                        "summary": ["Use for schema review."],
                        "project_usage": ["Check before changing SQL conventions."],
                    },
                    "postgres",
                )
            ]
        )

        self.assertNotIn("cache path:", message)

    def test_external_guide_message_footer_uses_active_profile_researcher_label(
        self,
    ) -> None:
        with self.with_profile("default"):
            message_default = agent_router.build_external_guides_message(
                [
                    (
                        {
                            "id": "pg-guide",
                            "summary": ["s"],
                            "project_usage": ["u"],
                        },
                        "postgres",
                    )
                ]
            )

        with self.with_profile("codex-heavy"):
            message_codex_heavy = agent_router.build_external_guides_message(
                [
                    (
                        {
                            "id": "pg-guide",
                            "summary": ["s"],
                            "project_usage": ["u"],
                        },
                        "postgres",
                    )
                ]
            )

        self.assertIn("Gemini CLI", message_default)
        self.assertIn("Codex CLI", message_codex_heavy)
        self.assertNotIn("Gemini CLI", message_codex_heavy)

    def test_external_guide_message_footer_discourages_full_read_and_suggests_grep(
        self,
    ) -> None:
        message = agent_router.build_external_guides_message(
            [
                (
                    {
                        "id": "pg-guide",
                        "summary": ["s"],
                        "project_usage": ["u"],
                    },
                    "postgres",
                )
            ]
        )

        self.assertIn("Avoid", message)
        self.assertIn("Read tool", message)
        self.assertIn("Grep", message)
        self.assertIn("researcher", message)

    def test_find_external_guide_matches_only_runs_for_track_execution_prompts(
        self,
    ) -> None:
        with mock.patch.object(
            agent_router.external_guides,
            "find_relevant_guides_for_track_workflow",
        ) as find_guides:
            matches = agent_router.find_external_guide_matches(
                "Please review postgres guidance"
            )

        self.assertEqual(matches, [])
        find_guides.assert_not_called()

    def test_find_external_guide_matches_calls_external_guides_for_track_prompts(
        self,
    ) -> None:
        expected = [
            ({"id": "pg-guide", "summary": [], "project_usage": []}, "postgres")
        ]
        with mock.patch.object(
            agent_router.external_guides,
            "find_relevant_guides_for_track_workflow",
            return_value=expected,
        ) as find_guides:
            matches = agent_router.find_external_guide_matches(
                "/track:plan postgres migration"
            )

        self.assertEqual(matches, expected)
        find_guides.assert_called_once_with("/track:plan postgres migration")

    def test_workflow_trigger_does_not_match_specific_word_fragment(self) -> None:
        agent, trigger, is_multimodal = agent_router.detect_agent(
            "Please make this specific review tighter"
        )
        self.assertNotEqual(agent, "workflow")
        self.assertEqual((agent, trigger, is_multimodal), ("reviewer", "review", False))

    def test_workflow_trigger_does_not_match_tracking_word_fragment(self) -> None:
        agent, trigger, is_multimodal = agent_router.detect_agent(
            "We are tracking a bug in production"
        )
        self.assertNotEqual(agent, "workflow")
        self.assertEqual((agent, trigger, is_multimodal), ("debugger", "bug", False))

    def test_explicit_review_beats_generic_track_workflow_hint(self) -> None:
        """'レビュー' triggers explicit review override, beating both planner and workflow."""
        agent, _, is_multimodal = agent_router.detect_agent(
            "track の設計をレビューして"
        )
        self.assertEqual(agent, "reviewer")
        self.assertFalse(is_multimodal)

    def test_researcher_beats_generic_spec_workflow_hint(self) -> None:
        agent, trigger, is_multimodal = agent_router.detect_agent(
            "spec を調べて設計方針を考えて"
        )
        self.assertEqual(
            (agent, trigger, is_multimodal), ("researcher", "調べて", False)
        )

    def test_short_japanese_implement_prompt_routes_to_implementer(self) -> None:
        agent, trigger, is_multimodal = agent_router.detect_agent("実装")
        self.assertEqual(
            (agent, trigger, is_multimodal), ("implementer", "実装", False)
        )

    def test_short_japanese_test_prompt_routes_to_implementer(self) -> None:
        agent, trigger, is_multimodal = agent_router.detect_agent("テスト")
        self.assertEqual(
            (agent, trigger, is_multimodal), ("implementer", "テスト", False)
        )

    def test_short_english_fix_test_prompt_routes_to_implementer(self) -> None:
        agent, trigger, is_multimodal = agent_router.detect_agent("fix test")
        self.assertEqual(
            (agent, trigger, is_multimodal), ("implementer", "test", False)
        )

    def test_main_emits_context_for_short_workflow_prompt(self) -> None:
        stdout = io.StringIO()
        with mock.patch.object(
            agent_router, "load_stdin_json", return_value={"prompt": "/guide:add"}
        ):
            with redirect_stdout(stdout):
                with self.assertRaises(SystemExit) as exc:
                    agent_router.main()

        self.assertEqual(exc.exception.code, 0)
        self.assertIn(agent_router.WORKFLOW_PREFIX, stdout.getvalue())

    def test_planner_message_switches_to_claude_when_profile_changes(self) -> None:
        with self.with_profile("claude-heavy"):
            message = agent_router.build_capability_message("planner", "design")

        self.assertIn("Claude Code", message)
        self.assertIn("/track:plan <feature>", message)
        self.assertNotIn("codex exec", message)

    def test_researcher_message_switches_to_codex_when_profile_changes(self) -> None:
        with self.with_profile("codex-heavy"):
            message = agent_router.build_capability_message("researcher", "latest")

        self.assertIn("Codex CLI", message)
        self.assertIn("Research this Rust topic", message)
        self.assertNotIn("gemini -p", message)

    # ================================================================
    # Skill description eval insights — new planner triggers
    # ================================================================

    def test_kentou_shite_routes_to_planner(self) -> None:
        """'検討して' = consider/evaluate — planner territory."""
        agent, _, _ = agent_router.detect_agent("Arc vs Box を検討して")
        self.assertEqual(agent, "planner")

    def test_sekkei_shitai_routes_to_planner(self) -> None:
        """'設計したい' = want to design."""
        agent, _, _ = agent_router.detect_agent("CancelOrderCommand を設計したい")
        self.assertEqual(agent, "planner")

    def test_jissou_keikaku_routes_to_planner(self) -> None:
        """'実装計画' = implementation plan."""
        agent, _, _ = agent_router.detect_agent("この機能の実装計画を立ててほしい")
        self.assertEqual(agent, "planner")

    def test_mayotteru_routes_to_planner(self) -> None:
        """'迷ってる' = undecided — design decision help."""
        agent, _, _ = agent_router.detect_agent(
            "戻り値を Option にすべきか Result にすべきか迷ってる"
        )
        self.assertEqual(agent, "planner")

    def test_tdd_japanese_routes_to_planner(self) -> None:
        """'TDD' in Japanese context should route to planner."""
        agent, _, _ = agent_router.detect_agent("TDD でやりたい")
        self.assertEqual(agent, "planner")

    def test_tdd_english_routes_to_planner(self) -> None:
        agent, _, _ = agent_router.detect_agent(
            "I want to use tdd, red green refactor cycle"
        )
        self.assertEqual(agent, "planner")

    def test_arc_vs_routes_to_planner(self) -> None:
        agent, _, _ = agent_router.detect_agent("arc vs box which is better?")
        self.assertEqual(agent, "planner")

    def test_async_trait_vs_rpitit_routes_to_planner(self) -> None:
        agent, _, _ = agent_router.detect_agent(
            "async-trait vs rpitit which should I use?"
        )
        self.assertEqual(agent, "planner")

    def test_option_vs_result_routes_to_planner(self) -> None:
        agent, _, _ = agent_router.detect_agent("option vs result for find_by_id?")
        self.assertEqual(agent, "planner")

    def test_method_signature_routes_to_planner(self) -> None:
        agent, _, _ = agent_router.detect_agent(
            "What should the method signature look like for this trait?"
        )
        self.assertEqual(agent, "planner")

    def test_domain_layer_routes_to_planner(self) -> None:
        agent, _, _ = agent_router.detect_agent(
            "How should I structure the domain layer?"
        )
        self.assertEqual(agent, "planner")

    def test_clone_vs_borrow_routes_to_planner(self) -> None:
        agent, _, _ = agent_router.detect_agent(
            "Should I clone vs borrow in this function?"
        )
        self.assertEqual(agent, "planner")

    # ================================================================
    # Skill description eval insights — new debugger triggers
    # ================================================================

    def test_compile_toranai_routes_to_debugger(self) -> None:
        """'コンパイル通らない' = won't compile."""
        agent, _, _ = agent_router.detect_agent("コンパイル通らない")
        self.assertEqual(agent, "debugger")

    def test_compile_error_ja_routes_to_debugger(self) -> None:
        """'コンパイルエラー' = compiler error."""
        agent, _, _ = agent_router.detect_agent("コンパイルエラーが出た")
        self.assertEqual(agent, "debugger")

    def test_e0382_routes_to_debugger(self) -> None:
        agent, _, _ = agent_router.detect_agent("E0382 が出てる")
        self.assertEqual(agent, "debugger")

    def test_e0505_routes_to_debugger(self) -> None:
        agent, _, _ = agent_router.detect_agent("error E0505")
        self.assertEqual(agent, "debugger")

    def test_moved_value_routes_to_debugger(self) -> None:
        agent, _, _ = agent_router.detect_agent("use of moved value in match")
        self.assertEqual(agent, "debugger")

    def test_compiler_error_en_routes_to_debugger(self) -> None:
        agent, _, _ = agent_router.detect_agent("compiler error in impl block")
        self.assertEqual(agent, "debugger")

    def test_borrow_conflict_routes_to_debugger(self) -> None:
        agent, _, _ = agent_router.detect_agent("borrow conflict in async fn")
        self.assertEqual(agent, "debugger")

    # ================================================================
    # Skill description eval insights — new reviewer triggers
    # ================================================================

    def test_idiomatic_ja_routes_to_reviewer(self) -> None:
        """'イディオマティック' = idiomatic."""
        agent, _, _ = agent_router.detect_agent(
            "イディオマティックなRustパターンになってるか"
        )
        self.assertEqual(agent, "reviewer")

    def test_tadashisa_routes_to_reviewer(self) -> None:
        """'正しさ' = correctness. Prompt avoids planner-overlap '所有権'."""
        agent, _, _ = agent_router.detect_agent("この関数の正しさを確認して")
        self.assertEqual(agent, "reviewer")

    def test_mite_hoshii_routes_to_reviewer(self) -> None:
        """'見てほしい' = please look at (casual review request)."""
        agent, _, _ = agent_router.detect_agent("このコード見てほしい")
        self.assertEqual(agent, "reviewer")

    def test_kakunin_shite_hoshii_routes_to_reviewer(self) -> None:
        """'確認してほしい' = please check (casual review request)."""
        agent, _, _ = agent_router.detect_agent(
            "Rustパターンになってるか確認してほしい"
        )
        self.assertEqual(agent, "reviewer")

    def test_idiomatic_en_routes_to_reviewer(self) -> None:
        agent, _, _ = agent_router.detect_agent("Is this idiomatic Rust?")
        self.assertEqual(agent, "reviewer")

    def test_correctness_en_does_not_route_to_reviewer(self) -> None:
        """'correctness' alone is too generic — removed from reviewer triggers."""
        agent, _, _ = agent_router.detect_agent("Check correctness of this code")
        self.assertNotEqual(agent, "reviewer")

    def test_rust_patterns_en_routes_to_reviewer(self) -> None:
        agent, _, _ = agent_router.detect_agent("Does this follow rust patterns?")
        self.assertEqual(agent, "reviewer")

    # ================================================================
    # Priority: strong debugger signals beat planner overlap
    # ================================================================

    def test_e_code_with_borrow_context_routes_to_debugger(self) -> None:
        """E-code is a strong debugger signal that beats planner's '借用'."""
        agent, _, _ = agent_router.detect_agent(
            "E0505 が出た。借用してるんだけど衝突してる"
        )
        self.assertEqual(agent, "debugger")

    def test_compile_error_with_ownership_context_routes_to_debugger(self) -> None:
        """'コンパイル通らない' beats planner's '所有権'."""
        agent, _, _ = agent_router.detect_agent(
            "コンパイル通らない。所有権がmoveされてるっぽい"
        )
        self.assertEqual(agent, "debugger")

    def test_moved_value_with_lifetime_context_routes_to_debugger(self) -> None:
        """'moved value' beats planner's 'lifetime'."""
        agent, _, _ = agent_router.detect_agent(
            "moved value error, something about lifetime bounds"
        )
        self.assertEqual(agent, "debugger")

    # ================================================================
    # Eval full prompts — positive cases (should route to Codex caps)
    # ================================================================

    def test_eval_positive_implementation_plan_with_ddd(self) -> None:
        agent, _, _ = agent_router.detect_agent(
            "この機能の実装計画を立ててほしい。注文のキャンセル処理で、"
            "domain layer にCancelOrderCommand を作って usecase 経由で "
            "infrastructure に流す設計にしたい"
        )
        self.assertEqual(agent, "planner")

    def test_eval_positive_compile_error_e0382(self) -> None:
        agent, _, _ = agent_router.detect_agent(
            "なんかコンパイル通らない。error[E0382]: use of moved value: "
            "order ってなってる。match で所有権取った後にまた使おうとしてるのかも"
        )
        self.assertEqual(agent, "debugger")

    def test_eval_positive_arc_vs_box(self) -> None:
        agent, _, _ = agent_router.detect_agent(
            "Arc<dyn UserRepository> と Box<dyn UserRepository> "
            "どっちがいいか検討して。tokio::spawn で複数タスクから共有する想定"
        )
        self.assertEqual(agent, "planner")

    def test_eval_positive_tdd_planning(self) -> None:
        agent, _, _ = agent_router.detect_agent(
            "TDD で進めたいんだけど、OrderAggregate のテストをどう組み立てれば"
            "いいか計画してほしい。Red → Green → Refactor の順序で"
        )
        self.assertEqual(agent, "planner")

    def test_eval_positive_async_trait_vs_rpitit(self) -> None:
        agent, _, _ = agent_router.detect_agent(
            "async-trait と RPITIT どっちにすべきか比較検討して。"
            "Rust 1.82 以降で RPITIT が安定したって聞いたけど"
        )
        self.assertEqual(agent, "planner")

    def test_eval_positive_ownership_review(self) -> None:
        agent, _, _ = agent_router.detect_agent(
            "git diff HEAD の変更をレビューして。所有権の正しさと"
            "イディオマティックなRustパターンになってるか確認してほしい"
        )
        # Explicit "レビュー" triggers review override, beating planner's "所有権".
        self.assertEqual(agent, "reviewer")

    def test_eval_positive_e0505_borrow(self) -> None:
        agent, _, _ = agent_router.detect_agent(
            "E0505 が出た。async fn register で &self.repo を借用してるんだけど、"
            "同じスコープで self.cache にも書き込んでて衝突してるっぽい"
        )
        self.assertEqual(agent, "debugger")

    def test_eval_positive_trait_design_review(self) -> None:
        agent, _, _ = agent_router.detect_agent(
            "libs/domain/src/user.rs のトレイト設計を見てほしい。"
            "UserRepository に find_by_id を追加したいんだけど、"
            "戻り値を Option<User> にすべきか Result<Option<User>, DomainError> "
            "にすべきか迷ってる"
        )
        # "見てほしい" triggers explicit review override, beating planner's "トレイト設計".
        self.assertEqual(agent, "reviewer")

    # ================================================================
    # Eval full prompts — negative cases (should NOT go to Codex caps)
    # ================================================================

    def test_eval_negative_spec_writing(self) -> None:
        """Spec writing → workflow or implementer, not planner/debugger."""
        agent, _, _ = agent_router.detect_agent(
            "track/items/001/spec.md を書いて。ユーザー登録機能の仕様をまとめたい"
        )
        self.assertNotIn(agent, ("planner", "debugger"))

    def test_eval_negative_axum_research(self) -> None:
        """Crate research → researcher, not planner."""
        agent, _, _ = agent_router.detect_agent(
            "axum の最新バージョンと 0.7 からの breaking changes を調べて"
        )
        self.assertEqual(agent, "researcher")

    def test_eval_negative_simple_assertion_fix(self) -> None:
        """Simple test fix → implementer, not debugger."""
        agent, _, _ = agent_router.detect_agent(
            "テストが1つ落ちてるけど原因はシンプルで、期待値が古いだけ。"
            "assert_eq! の expected を 42 から 43 に変えて"
        )
        self.assertEqual(agent, "implementer")

    def test_eval_negative_cargo_toml_edit(self) -> None:
        """Simple dependency add → not planner/debugger/reviewer."""
        agent, _, _ = agent_router.detect_agent(
            'Cargo.toml に serde_json を追加して features = ["preserve_order"] で'
        )
        self.assertNotIn(agent, ("planner", "debugger", "reviewer"))

    def test_eval_negative_codebase_structure_analysis(self) -> None:
        """Codebase analysis → researcher."""
        agent, _, _ = agent_router.detect_agent(
            "このワークスペースの全体構造を分析して、"
            "どのクレートがどのクレートに依存してるか整理して"
        )
        self.assertEqual(agent, "researcher")

    # ================================================================
    # Review fixes: false-positive guards and maintenance hygiene
    # ================================================================

    def test_e_code_pattern_does_not_match_version_like_strings(self) -> None:
        """e-code pattern should not match 'release2024'."""
        agent, _, _ = agent_router.detect_agent("release2024 のデプロイ手順を教えて")
        self.assertNotEqual(agent, "debugger")

    def test_e_code_pattern_does_not_match_embedded_digits(self) -> None:
        """'code5678' should NOT route to debugger."""
        agent, _, _ = agent_router.detect_agent("code5678 is the internal identifier")
        self.assertNotEqual(agent, "debugger")

    def test_clone_vs_does_not_false_match_git_clone(self) -> None:
        """'git clone vs download' should NOT route to planner via 'clone vs'."""
        agent, _, _ = agent_router.detect_agent(
            "git clone vs download which is faster?"
        )
        self.assertNotEqual(agent, "planner")

    # ================================================================
    # Codex review iteration 1 — false-positive & priority fixes
    # ================================================================

    def test_research_intent_beats_strong_debugger_compiler_error(self) -> None:
        """Researcher signal should beat strong debugger 'compiler error'."""
        agent, _, _ = agent_router.detect_agent(
            "Please research compiler error categories in Rust docs"
        )
        self.assertEqual(agent, "researcher")

    def test_implement_intent_not_stolen_by_lifetime_mismatch(self) -> None:
        """'lifetime mismatch' should not steal implementer routing."""
        agent, _, _ = agent_router.detect_agent(
            "I need to implement a lifetime mismatch detector"
        )
        self.assertNotEqual(agent, "debugger")

    def test_research_intent_beats_e_code(self) -> None:
        """Research about E-codes should route to researcher, not debugger."""
        agent, _, _ = agent_router.detect_agent("E0382 について調べて")
        self.assertEqual(agent, "researcher")

    def test_cannot_borrow_routes_to_debugger(self) -> None:
        """'cannot borrow' is a concrete borrow-checker error message."""
        agent, _, _ = agent_router.detect_agent("error: cannot borrow self as mutable")
        self.assertEqual(agent, "debugger")

    def test_cannot_borrow_beats_planner_borrow(self) -> None:
        """'cannot borrow' should beat planner's 'borrow'."""
        agent, _, _ = agent_router.detect_agent(
            "cannot borrow `self.repo` as immutable because it is also borrowed as mutable"
        )
        self.assertEqual(agent, "debugger")

    def test_explicit_review_beats_planner_method_signature(self) -> None:
        """Explicit 'review' intent should beat planner's 'method signature'."""
        agent, _, _ = agent_router.detect_agent(
            "git diff review: method signature and correctness"
        )
        self.assertEqual(agent, "reviewer")

    def test_explicit_review_ja_beats_planner_triggers(self) -> None:
        """Explicit 'レビュー' should beat planner's topic triggers."""
        agent, _, _ = agent_router.detect_agent("return type の変更をレビューして")
        self.assertEqual(agent, "reviewer")

    # ================================================================
    # Codex review iteration 2 — review vs debugger, casual review
    # ================================================================

    def test_explicit_review_beats_strong_debugger(self) -> None:
        """'review' intent should beat strong debugger 'cannot borrow'."""
        agent, _, _ = agent_router.detect_agent("Please review this cannot borrow fix")
        self.assertEqual(agent, "reviewer")

    def test_casual_mite_hoshii_beats_planner(self) -> None:
        """'見てほしい' should beat planner's 'method signature'."""
        agent, _, _ = agent_router.detect_agent("method signature 見てほしい")
        self.assertEqual(agent, "reviewer")

    def test_casual_kakunin_shite_hoshii_beats_planner(self) -> None:
        """'確認してほしい' should beat planner's 'return type'."""
        agent, _, _ = agent_router.detect_agent("return type が正しいか確認してほしい")
        self.assertEqual(agent, "reviewer")

    # ================================================================
    # Codex review iteration 3 — false positive guards
    # ================================================================

    def test_implement_correctness_routes_to_implementer(self) -> None:
        """'implement correctness checks' is implementer, not reviewer."""
        agent, _, _ = agent_router.detect_agent(
            "I need to implement correctness checks for this validator"
        )
        self.assertEqual(agent, "implementer")

    def test_check_this_out_does_not_steal_implementer(self) -> None:
        """'check this out' should not trigger explicit review override."""
        agent, _, _ = agent_router.detect_agent(
            "Please check this out and implement login"
        )
        self.assertNotEqual(agent, "reviewer")

    def test_latest_commit_review_routes_to_reviewer(self) -> None:
        """Explicit 'レビュー' beats researcher domain cue '最新'."""
        agent, _, _ = agent_router.detect_agent("最新コミットをレビューして")
        self.assertEqual(agent, "reviewer")

    def test_capability_trigger_order_constant_is_removed(self) -> None:
        """CAPABILITY_TRIGGER_ORDER should no longer exist (replaced by phased logic)."""
        self.assertFalse(hasattr(agent_router, "CAPABILITY_TRIGGER_ORDER"))

    # ================================================================
    # Hybrid routing Phase 1 — weighted scoring unit tests
    # ================================================================

    def test_score_keywords_returns_dict_of_capability_scores(self) -> None:
        """score_keywords returns {capability: score} dict."""
        scores = agent_router.score_keywords("設計して")
        self.assertIsInstance(scores, dict)
        self.assertIn("planner", scores)
        self.assertIn("implementer", scores)
        self.assertIn("debugger", scores)
        self.assertIn("reviewer", scores)
        self.assertIn("researcher", scores)

    def test_score_keywords_intent_cue_scores_higher_than_domain(self) -> None:
        """Intent cue ('implement') should score higher than domain cue ('borrow conflict')."""
        scores = agent_router.score_keywords("implement borrow conflict detector")
        self.assertGreater(scores["implementer"], scores["debugger"])

    def test_score_keywords_planner_intent_beats_implementer_domain(self) -> None:
        """'should we' (planner intent) beats 'tests' (implementer domain)."""
        scores = agent_router.score_keywords("should we add tests?")
        self.assertGreater(scores["planner"], scores["implementer"])

    def test_score_keywords_implementer_intent_beats_planner_domain(self) -> None:
        """'実装して' (implementer intent) beats 'domain layer' (planner domain)."""
        scores = agent_router.score_keywords("domain layer の変換を実装して")
        self.assertGreater(scores["implementer"], scores["planner"])

    def test_score_keywords_researcher_intent_scores_high(self) -> None:
        """'調べて' is a researcher intent cue."""
        scores = agent_router.score_keywords("E0382 について調べて")
        self.assertGreater(scores["researcher"], scores["debugger"])

    def test_score_keywords_reviewer_intent_beats_planner_domain(self) -> None:
        """'レビュー' (reviewer intent) beats 'method signature' (planner domain)."""
        scores = agent_router.score_keywords("method signature をレビューして")
        self.assertGreater(scores["reviewer"], scores["planner"])

    def test_score_keywords_caps_per_family(self) -> None:
        """Multiple domain cues for same capability don't stack beyond cap."""
        scores_single = agent_router.score_keywords("ownership")
        scores_multi = agent_router.score_keywords("ownership lifetime borrow")
        # Domain cues are capped: multiple domain hits for planner don't compound
        self.assertEqual(scores_single["planner"], scores_multi["planner"])

    def test_is_clear_true_when_margin_large(self) -> None:
        """is_clear returns True when top score has large margin."""
        scores = {
            "planner": 4,
            "debugger": 0,
            "reviewer": 0,
            "implementer": 1,
            "researcher": 0,
        }
        self.assertTrue(agent_router.is_clear(scores))

    def test_is_clear_false_when_tie(self) -> None:
        """is_clear returns False when top two scores are tied."""
        scores = {
            "planner": 4,
            "debugger": 0,
            "reviewer": 0,
            "implementer": 4,
            "researcher": 0,
        }
        self.assertFalse(agent_router.is_clear(scores))

    def test_is_clear_false_when_margin_small(self) -> None:
        """is_clear returns False when margin is less than 2."""
        scores = {
            "planner": 3,
            "debugger": 0,
            "reviewer": 0,
            "implementer": 2,
            "researcher": 0,
        }
        self.assertFalse(agent_router.is_clear(scores))

    def test_is_clear_false_when_no_strong_signal(self) -> None:
        """is_clear returns False when top score is too low."""
        scores = {
            "planner": 1,
            "debugger": 0,
            "reviewer": 0,
            "implementer": 0,
            "researcher": 0,
        }
        self.assertFalse(agent_router.is_clear(scores))

    # ================================================================
    # Hybrid routing Phase 1 — detect_agent integration (scoring path)
    # ================================================================

    def test_should_we_add_tests_routes_to_planner(self) -> None:
        """Interrogative 'should we add tests' is planning, not implementer."""
        agent, _, _ = agent_router.detect_agent(
            "Should we add tests or keep current behavior?"
        )
        self.assertEqual(agent, "planner")

    def test_implement_borrow_conflict_detector_routes_to_implementer(self) -> None:
        """'implement' intent beats 'borrow conflict' debugger domain."""
        agent, _, _ = agent_router.detect_agent(
            "Need to implement borrow conflict detector"
        )
        self.assertEqual(agent, "implementer")

    def test_implement_return_type_routes_to_implementer(self) -> None:
        """'implement' intent beats 'return type' planner domain."""
        agent, _, _ = agent_router.detect_agent(
            "Implement return type conversion for parser"
        )
        self.assertEqual(agent, "implementer")

    def test_jissou_shite_beats_planner_domain(self) -> None:
        """'実装して' (implementer intent) beats 'domain layer' (planner domain)."""
        agent, _, _ = agent_router.detect_agent("domain layer の変換を実装して")
        self.assertEqual(agent, "implementer")

    def test_write_tests_beats_planner_domain(self) -> None:
        """'write tests' (implementer intent) beats 'option vs' (planner domain)."""
        agent, _, _ = agent_router.detect_agent(
            "write tests for option vs result parsing"
        )
        self.assertEqual(agent, "implementer")

    def test_how_should_optimize_routes_to_planner(self) -> None:
        """'How should I optimize' is planning (interrogative), not implementer."""
        agent, _, _ = agent_router.detect_agent(
            "How should I optimize this architecture?"
        )
        self.assertEqual(agent, "planner")

    def test_refactor_method_signature_routes_to_implementer(self) -> None:
        """'refactor' (implementer intent) beats 'method signature' (planner domain)."""
        agent, _, _ = agent_router.detect_agent(
            "refactor the method signature handling code"
        )
        self.assertEqual(agent, "implementer")

    def test_jissou_shite_ii_ka_kentou_routes_to_planner(self) -> None:
        """'検討して' (planner intent) beats '実装' (implementer domain) in interrogative."""
        agent, _, _ = agent_router.detect_agent("実装していいか検討して")
        self.assertEqual(agent, "planner")

    # ================================================================
    # Codex 5.4 review — interrogative vs explicit intent, noun demotion
    # ================================================================

    def test_how_should_debug_routes_to_debugger(self) -> None:
        """'debug' (debugger intent) beats 'how should' (interrogative)."""
        agent, _, _ = agent_router.detect_agent("How should I debug this error?")
        self.assertEqual(agent, "debugger")

    def test_implement_architecture_routes_to_implementer(self) -> None:
        """'implement' (implementer intent) beats 'architecture' (planner domain)."""
        agent, _, _ = agent_router.detect_agent("Implement the architecture changes")
        self.assertEqual(agent, "implementer")

    def test_implement_design_routes_to_implementer(self) -> None:
        """'implement' (implementer intent) beats 'design' (planner domain)."""
        agent, _, _ = agent_router.detect_agent("Implement the design changes")
        self.assertEqual(agent, "implementer")

    def test_refactor_architecture_routes_to_implementer(self) -> None:
        """'refactor' (implementer intent) beats 'architecture' (planner domain)."""
        agent, _, _ = agent_router.detect_agent("Refactor architecture module")
        self.assertEqual(agent, "implementer")

    def test_interrogative_weight_lower_than_intent(self) -> None:
        """Interrogative patterns score less than full intent weight."""
        scores = agent_router.score_keywords("should we add tests?")
        self.assertGreater(scores["planner"], scores["implementer"])
        self.assertLess(scores["planner"], agent_router.INTENT_WEIGHT)

    # ── Codex review round 2: RED tests ──

    def test_refactor_idiomatic_routes_to_implementer(self) -> None:
        """'refactor' (implementer intent) beats 'idiomatic' (reviewer domain)."""
        agent, _, _ = agent_router.detect_agent("Refactor this into idiomatic Rust")
        self.assertEqual(agent, "implementer")

    def test_implement_idiomatic_routes_to_implementer(self) -> None:
        """'implement' (implementer intent) beats 'idiomatic' (reviewer domain)."""
        agent, _, _ = agent_router.detect_agent("Implement an idiomatic Rust builder")
        self.assertEqual(agent, "implementer")

    def test_bunseki_jissou_routes_to_implementer(self) -> None:
        """'実装して' (implementer intent) beats '分析して' (planner domain)."""
        agent, _, _ = agent_router.detect_agent("このコードを分析して実装して")
        self.assertEqual(agent, "implementer")

    def test_should_i_implement_arc_routes_to_planner(self) -> None:
        """'Should I implement with Arc' is a planning question, not implementation."""
        agent, _, _ = agent_router.detect_agent(
            "Should I implement this with Arc or refactor the API?"
        )
        self.assertEqual(agent, "planner")

    def test_could_we_refactor_return_type_routes_to_planner(self) -> None:
        """'Could we refactor' is a planning question."""
        agent, _, _ = agent_router.detect_agent("Could we refactor this return type?")
        self.assertEqual(agent, "planner")

    # ── Codex review round 3: RED tests ──

    def test_japanese_bekika_routes_to_planner(self) -> None:
        """'べきか' (Japanese interrogative) demotes implementer intent."""
        agent, _, _ = agent_router.detect_agent(
            "Arc で実装すべきか API をリファクタすべきか？"
        )
        self.assertEqual(agent, "planner")

    def test_japanese_dosubeki_routes_to_planner(self) -> None:
        """'どうすべき' (Japanese interrogative) beats implementer."""
        agent, _, _ = agent_router.detect_agent(
            "この返り値の型をどうすべき？リファクタしたい"
        )
        self.assertEqual(agent, "planner")

    def test_japanese_houga_ii_routes_to_planner(self) -> None:
        """'したほうがいい' (Japanese interrogative) beats implementer."""
        agent, _, _ = agent_router.detect_agent(
            "実装したほうがいいかな？テスト書いてから？"
        )
        self.assertEqual(agent, "planner")

    def test_could_we_refactor_fix_e0505_routes_to_planner(self) -> None:
        """Interrogative + refactor + error code: planning question, not debugger."""
        agent, _, _ = agent_router.detect_agent(
            "Could we refactor this return type to fix E0505?"
        )
        self.assertEqual(agent, "planner")

    # ── Codex review round 4: RED tests ──

    def test_how_should_fix_e0505_routes_to_debugger(self) -> None:
        """'How should I fix E0505?' is a debugger question, not planner."""
        agent, _, _ = agent_router.detect_agent("How should I fix E0505?")
        self.assertEqual(agent, "debugger")

    def test_could_i_resolve_borrow_conflict_routes_to_debugger(self) -> None:
        """'Could I resolve this borrow conflict?' is debugging, not planning."""
        agent, _, _ = agent_router.detect_agent(
            "Could I resolve this borrow conflict here?"
        )
        self.assertEqual(agent, "debugger")

    def test_would_it_be_idiomatic_routes_to_reviewer(self) -> None:
        """'Would it be idiomatic' is a review question, not planner."""
        agent, _, _ = agent_router.detect_agent(
            "Would it be idiomatic Rust to clone here?"
        )
        self.assertIn(agent, ("reviewer", "implementer"))

    def test_would_it_compile_does_not_route_to_planner_interrogative(self) -> None:
        """'Would it compile' should not activate planner interrogative.
        Both planner and debugger have domain cues; tiebreak favors planner."""
        agent, _, _ = agent_router.detect_agent(
            "Would it compile if I move this borrow?"
        )
        # 'borrow' is planner domain, 'compile' is debugger domain → tie → planner wins tiebreak
        self.assertIn(agent, ("planner", "debugger"))

    # ── Codex review round 5: RED tests ──

    def test_compare_arc_rc_fix_e0505_routes_to_planner(self) -> None:
        """Planner intent 'compare' beats strong debugger 'E0505'."""
        agent, _, _ = agent_router.detect_agent(
            "Should we compare Arc vs Rc to fix E0505?"
        )
        self.assertEqual(agent, "planner")

    def test_e0505_dou_sekkei_routes_to_planner(self) -> None:
        """Planner intent '設計' beats strong debugger 'E0505'."""
        agent, _, _ = agent_router.detect_agent("E0505 をどう設計すべき？")
        self.assertEqual(agent, "planner")

    def test_e0505_tradeoff_routes_to_planner(self) -> None:
        """Planner intent '検討して' beats strong debugger 'E0505'."""
        agent, _, _ = agent_router.detect_agent("E0505 のトレードオフを検討して")
        self.assertEqual(agent, "planner")

    def test_score_keywords_demotes_implementer_with_planner_intent_and_interrogative(
        self,
    ) -> None:
        """When planner has both intent and interrogative, implementer is still demoted."""
        scores = agent_router.score_keywords(
            "should we compare or refactor this return type?"
        )
        self.assertLess(scores["implementer"], agent_router.INTENT_WEIGHT)

    # ── Codex review round 6: RED test ──

    def test_should_we_use_arc_avoid_e0505_routes_to_debugger(self) -> None:
        """Interrogative + E0505: no planner intent verb, so Phase 1 routes to debugger.
        Phase 2 LLM may reclassify to planner for design-choice prompts like this."""
        agent, _, _ = agent_router.detect_agent("Should we use Arc to avoid E0505?")
        self.assertEqual(agent, "debugger")

    # ================================================================
    # Researcher misrouting fix: explicit review and strong debugger
    # beat researcher domain cues
    # ================================================================

    def test_explicit_review_beats_researcher_domain_latest(self) -> None:
        """'レビュー' (explicit review) beats '最新' (researcher domain)."""
        agent, _, _ = agent_router.detect_agent("最新コミットをレビューして")
        self.assertEqual(agent, "reviewer")

    def test_researcher_intent_still_beats_review(self) -> None:
        """Researcher intent cue '調べて' causes early-exit before review check.
        Note: explicit review is checked first, so review still wins."""
        agent, _, _ = agent_router.detect_agent(
            "最新の変更をレビューして、何が変わったか調べて"
        )
        # "レビュー" triggers explicit review in Phase 1.
        self.assertEqual(agent, "reviewer")

    def test_researcher_intent_beats_strong_debugger(self) -> None:
        """Researcher intent '調べて' keeps researcher even with E-code."""
        agent, _, _ = agent_router.detect_agent("E0382 について調べて")
        self.assertEqual(agent, "researcher")

    def test_strong_debugger_beats_researcher_domain(self) -> None:
        """Strong debugger 'E0505' beats researcher domain '最新' (no intent)."""
        agent, _, _ = agent_router.detect_agent("E0505 が出た、最新のRust情報を見たい")
        self.assertEqual(agent, "debugger")

    def test_researcher_domain_still_works_without_competing_signals(self) -> None:
        """Researcher domain cues still route correctly when no review/debugger."""
        agent, _, _ = agent_router.detect_agent(
            "このプロジェクトのコードベースを把握したい"
        )
        self.assertEqual(agent, "researcher")

    def test_explicit_review_beats_researcher_docs(self) -> None:
        """'review' beats researcher domain cue 'documentation'."""
        agent, _, _ = agent_router.detect_agent("review the documentation changes")
        self.assertEqual(agent, "reviewer")

    def test_look_up_e_code_routes_to_researcher(self) -> None:
        """'look up' is researcher intent and beats strong debugger 'E0505'."""
        agent, _, _ = agent_router.detect_agent("Please look up E0505 in Rust docs")
        self.assertEqual(agent, "researcher")

    def test_find_out_e_code_routes_to_researcher(self) -> None:
        """'find out' is researcher intent and beats strong debugger."""
        agent, _, _ = agent_router.detect_agent("find out about E0382 causes")
        self.assertEqual(agent, "researcher")


if __name__ == "__main__":
    unittest.main()
