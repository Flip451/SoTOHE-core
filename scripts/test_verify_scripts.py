import re
import unittest
from pathlib import Path

PROJECT_ROOT = Path(__file__).resolve().parent.parent
RUSTFMT_CATALOG_SOURCE_RUST_VERSION = "1.94.0"
RUSTFMT_CATALOG_SOURCE_RUSTFMT_VERSION = "1.8.0"

RUSTFMT_PRINT_CONFIG_DEFAULT_SNAPSHOT = """\
max_width = 100
hard_tabs = false
tab_spaces = 4
newline_style = "Auto"
indent_style = "Block"
use_small_heuristics = "Default"
fn_call_width = 60
attr_fn_like_width = 70
struct_lit_width = 18
struct_variant_width = 35
array_width = 60
chain_width = 60
single_line_if_else_max_width = 50
single_line_let_else_max_width = 50
wrap_comments = false
format_code_in_doc_comments = false
doc_comment_code_block_width = 100
comment_width = 80
normalize_comments = false
normalize_doc_attributes = false
format_strings = false
format_macro_matchers = false
format_macro_bodies = true
skip_macro_invocations = []
hex_literal_case = "Preserve"
empty_item_single_line = true
struct_lit_single_line = true
fn_single_line = false
where_single_line = false
imports_indent = "Block"
imports_layout = "Mixed"
imports_granularity = "Preserve"
group_imports = "Preserve"
reorder_imports = true
reorder_modules = true
reorder_impl_items = false
type_punctuation_density = "Wide"
space_before_colon = false
space_after_colon = true
spaces_around_ranges = false
binop_separator = "Front"
remove_nested_parens = true
combine_control_expr = true
short_array_element_width_threshold = 10
overflow_delimited_expr = false
struct_field_align_threshold = 0
enum_discrim_align_threshold = 0
match_arm_blocks = true
match_arm_leading_pipes = "Never"
force_multiline_blocks = false
fn_params_layout = "Tall"
brace_style = "SameLineWhere"
control_brace_style = "AlwaysSameLine"
trailing_semicolon = true
trailing_comma = "Vertical"
match_block_trailing_comma = false
blank_lines_upper_bound = 1
blank_lines_lower_bound = 0
edition = "2015"
style_edition = "2015"
version = "One"
inline_attribute_width = 0
format_generated_files = true
generated_marker_line_search_limit = 5
merge_derives = true
use_try_shorthand = false
use_field_init_shorthand = false
force_explicit_abi = true
condense_wildcard_suffixes = false
color = "Auto"
required_version = "1.8.0"
unstable_features = false
disable_all_formatting = false
skip_children = false
show_parse_errors = true
error_on_line_overflow = false
error_on_unformatted = false
ignore = []
emit_mode = "Files"
make_backup = false
"""


class VerifyScriptsTest(unittest.TestCase):
    def test_compose_uses_host_uid_gid_and_repo_local_build_caches(self) -> None:
        for rel_path in ("compose.yml", "compose.dev.yml"):
            content = (PROJECT_ROOT / rel_path).read_text(encoding="utf-8")
            self.assertIn('user: "${HOST_UID:-1000}:${HOST_GID:-1000}"', content)
            self.assertIn("HOME: /workspace/.cache/home", content)
            self.assertIn("CARGO_HOME: /workspace/.cache/cargo", content)
            self.assertIn("CARGO_TARGET_DIR: /workspace/${CARGO_TARGET_DIR_RELATIVE:-target}", content)
            self.assertIn("${SCCACHE_HOST_DIR:-./.cache/sccache}", content)
            self.assertNotIn("target_cache:/workspace/target", content)
            self.assertNotIn("cargo_registry_cache:/usr/local/cargo/registry", content)
            self.assertNotIn("cargo_git_cache:/usr/local/cargo/git", content)

        compose_dev = (PROJECT_ROOT / "compose.dev.yml").read_text(encoding="utf-8")
        self.assertIn(
            "command: bash -c 'exec /usr/local/cargo/bin/bacon \"$${BACON_JOB:-run}\" --headless'",
            compose_dev,
        )
        self.assertNotIn(
            'command: bash -lc "bacon ${BACON_JOB} --headless"', compose_dev
        )

        gitignore = (PROJECT_ROOT / ".gitignore").read_text(encoding="utf-8")
        self.assertIn(".cache/cargo/", gitignore)
        self.assertIn(".cache/home/", gitignore)
        self.assertIn(".cache/pytest/", gitignore)
        self.assertIn(".cache/sccache/", gitignore)

    def test_compose_mounts_git_readonly(self) -> None:
        """Security: every compose service with volumes must mount .git read-only."""
        import yaml

        for rel_path in ("compose.yml", "compose.dev.yml"):
            content = (PROJECT_ROOT / rel_path).read_text(encoding="utf-8")
            data = yaml.safe_load(content)
            services = data.get("services", {})
            for svc_name, svc_cfg in services.items():
                # Services using 'extends' inherit parent volumes; check resolved config.
                volumes = svc_cfg.get("volumes") if svc_cfg else None
                if volumes is None:
                    # Service inherits from parent (e.g., extends); skip.
                    continue
                git_ro_found = any(
                    ".git:/workspace/.git:ro" in str(v) for v in volumes
                )
                self.assertTrue(
                    git_ro_found,
                    f"{rel_path}/{svc_name}: .git must be mounted read-only (:ro)",
                )

    def test_compose_masks_sensitive_dirs_with_tmpfs(self) -> None:
        """Security: every compose service with volumes must mask private/ and config/secrets/ via tmpfs."""
        import yaml

        for rel_path in ("compose.yml", "compose.dev.yml"):
            content = (PROJECT_ROOT / rel_path).read_text(encoding="utf-8")
            data = yaml.safe_load(content)
            services = data.get("services", {})
            for svc_name, svc_cfg in services.items():
                # Services using 'extends' inherit parent tmpfs; skip if no own volumes.
                volumes = svc_cfg.get("volumes") if svc_cfg else None
                if volumes is None:
                    continue
                tmpfs_list = svc_cfg.get("tmpfs", [])
                tmpfs_str = " ".join(str(t) for t in tmpfs_list)
                self.assertIn(
                    "/workspace/private",
                    tmpfs_str,
                    f"{rel_path}/{svc_name}: private/ must be masked by tmpfs",
                )
                self.assertIn(
                    "/workspace/config/secrets",
                    tmpfs_str,
                    f"{rel_path}/{svc_name}: config/secrets/ must be masked by tmpfs",
                )

    def test_ci_uses_host_uid_gid_without_forcing_track_template_dev(self) -> None:
        workflow = (PROJECT_ROOT / ".github" / "workflows" / "ci.yml").read_text(
            encoding="utf-8"
        )
        self.assertIn('echo "HOST_UID=$(id -u)" >> "$GITHUB_ENV"', workflow)
        self.assertIn('echo "HOST_GID=$(id -g)" >> "$GITHUB_ENV"', workflow)
        self.assertIn(
            "mkdir -p target .cache/cargo/registry .cache/cargo/git .cache/sccache .cache/home .cache/pytest",
            workflow,
        )
        self.assertIn("docker exec ci-runner cargo make ci-container", workflow)
        self.assertNotIn("TRACK_TEMPLATE_DEV=1", workflow)

    def test_makefile_uses_locked_for_reproducible_validation_tasks(self) -> None:
        makefile = (PROJECT_ROOT / "Makefile.toml").read_text(encoding="utf-8")
        for snippet in (
            'args = ["clippy", "--locked", "--all-targets", "--all-features", "--", "-D", "warnings"]',
            'args = ["nextest", "run", "--locked", "--all-targets", "--all-features", "--no-tests=warn"]',
            'args = ["test", "--locked", "--doc", "--all-features"]',
            'args = ["test", "--locked", "--all-features", "--", "--nocapture"]',
            'args = ["check", "--locked", "--all-targets", "--all-features"]',
            'args = ["deny", "--locked", "check"]',
            'args = ["clippy", "--locked", "--tests", "--all-features", "--", "-D", "warnings"]',
            'cargo nextest run --locked "$CARGO_MAKE_TASK_ARGS"',  # test-one-exec preserves arg quoting
        ):
            self.assertIn(snippet, makefile)

    def test_human_onboarding_doc_exists_and_is_wired(self) -> None:
        onboarding = (PROJECT_ROOT / "START_HERE_HUMAN.md").read_text(encoding="utf-8")
        self.assertIn("人間と AI の責務境界", onboarding)
        self.assertIn("必須レビュー・承認ポイント", onboarding)
        self.assertIn("人間が修正してよい対象", onboarding)
        self.assertIn("TRACK_TRACEABILITY.md", onboarding)
        self.assertIn("レビューや運用判断が必要なとき", onboarding)
        self.assertIn("2章（対応付けルール）", onboarding)
        self.assertIn("4章（Interactive Implementation Contract）", onboarding)
        self.assertIn("architecture-rules.json", onboarding)
        self.assertIn("layers[].path", onboarding)
        self.assertIn("workspace member", onboarding)
        self.assertIn("project-docs/**", onboarding)
        self.assertIn("scripts/**", onboarding)
        self.assertIn(".claude/commands/**", onboarding)
        self.assertIn(".claude/agents/**", onboarding)
        self.assertIn("CLAUDE.md", onboarding)
        self.assertIn("rustfmt.toml", onboarding)
        self.assertIn(".cache/**", onboarding)
        self.assertIn("Cargo.lock", onboarding)
        conditional_idx = onboarding.index("条件付きで編集してよい:")
        no_manual_edit_idx = onboarding.index("人間が手動編集しない:")
        scripts_idx = onboarding.index("`scripts/**`")
        claude_ops_idx = onboarding.index("`.claude/commands/**`")
        claude_agents_idx = onboarding.index("`.claude/agents/**`")
        claude_md_idx = onboarding.index("`CLAUDE.md`")
        rustfmt_idx = onboarding.index("`rustfmt.toml`")
        self.assertGreater(scripts_idx, conditional_idx)
        self.assertLess(scripts_idx, no_manual_edit_idx)
        self.assertGreater(claude_ops_idx, conditional_idx)
        self.assertLess(claude_ops_idx, no_manual_edit_idx)
        self.assertGreater(claude_agents_idx, conditional_idx)
        self.assertLess(claude_agents_idx, no_manual_edit_idx)
        self.assertGreater(claude_md_idx, conditional_idx)
        self.assertLess(claude_md_idx, no_manual_edit_idx)
        self.assertGreater(rustfmt_idx, conditional_idx)
        self.assertLess(rustfmt_idx, no_manual_edit_idx)

        for rel_path in (
            "DEVELOPER_AI_WORKFLOW.md",
            "LOCAL_DEVELOPMENT.md",
            "track/workflow.md",
        ):
            content = (PROJECT_ROOT / rel_path).read_text(encoding="utf-8")
            self.assertIn("START_HERE_HUMAN.md", content)

    def test_track_command_docs_cover_priority_guardrails(self) -> None:
        commit_doc = (
            PROJECT_ROOT / ".claude" / "commands" / "track" / "commit.md"
        ).read_text(encoding="utf-8")
        self.assertIn("git diff --cached --stat", commit_doc)
        self.assertIn("track/registry.md", commit_doc)
        self.assertIn("tmp/track-commit/commit-message.txt", commit_doc)
        self.assertIn("cargo make track-commit-message", commit_doc)
        self.assertIn("tmp/track-commit/note.md", commit_doc)
        self.assertIn("cargo make track-note", commit_doc)
        self.assertIn("cargo make track-note", commit_doc)

        # /track:plan now owns track artifact creation (Option C merge)
        plan_doc = (
            PROJECT_ROOT / ".claude" / "commands" / "track" / "plan.md"
        ).read_text(encoding="utf-8")
        self.assertIn("schema_version", plan_doc)
        self.assertIn("match the created track directory name", plan_doc)
        self.assertIn("track-sync-views", plan_doc)
        self.assertIn("read-only view", plan_doc)

        implement_doc = (
            PROJECT_ROOT / ".claude" / "commands" / "track" / "implement.md"
        ).read_text(encoding="utf-8")
        self.assertIn("cargo make tools-up", implement_doc)
        self.assertIn("Cargo.lock", implement_doc)
        self.assertIn("cargo make track-transition", implement_doc)
        self.assertIn("metadata.json", implement_doc)

        conventions_doc = (
            PROJECT_ROOT / ".claude" / "commands" / "conventions" / "add.md"
        ).read_text(encoding="utf-8")
        self.assertIn("TODO:", conventions_doc)
        self.assertIn("follow-up", conventions_doc)
        self.assertIn("Do not stop right after file creation", conventions_doc)

        # `.claude/agents/orchestrator.md` was removed in commit f0ae093
        # (pre-Phase-1.5). The `tools-up` / `Cargo.lock` guardrails are now
        # covered by `.claude/commands/track/implement.md` (asserted above),
        # so there is no need to keep an orphan file check here.

    def test_rustfmt_config_exposes_overrides_and_default_catalog(self) -> None:
        rustfmt = (PROJECT_ROOT / "rustfmt.toml").read_text(encoding="utf-8")
        self.assertIn("Generated from `rustfmt --print-config default`", rustfmt)
        self.assertIn(f"`rustfmt {RUSTFMT_CATALOG_SOURCE_RUSTFMT_VERSION}`", rustfmt)
        self.assertIn(f"Rust `{RUSTFMT_CATALOG_SOURCE_RUST_VERSION}`", rustfmt)
        self.assertIn('# group_imports = "StdExternalCrate"', rustfmt)
        self.assertIn('# imports_granularity = "Crate"', rustfmt)

        active_overrides: list[str] = []
        catalog_lines: list[str] = []
        in_catalog = False
        for line in rustfmt.splitlines():
            if line == "# Default options (commented reference)":
                in_catalog = True
                continue
            if in_catalog:
                self.assertTrue(
                    line.startswith("# "), f"catalog line must stay commented: {line!r}"
                )
                catalog_lines.append(line[2:])
                continue
            if line.startswith("#") or not line.strip():
                continue
            active_overrides.append(line)

        self.assertEqual(
            active_overrides,
            [
                'edition = "2024"',
                'style_edition = "2024"',
                "max_width = 100",
                'use_small_heuristics = "Max"',
            ],
        )

        # Keep this test independent from whatever rustfmt happens to be on the host PATH.
        self.assertEqual(
            catalog_lines, RUSTFMT_PRINT_CONFIG_DEFAULT_SNAPSHOT.splitlines()
        )

        dockerfile = (PROJECT_ROOT / "Dockerfile").read_text(encoding="utf-8")
        docker_match = re.search(r"^ARG RUST_VERSION=(.+)$", dockerfile, re.MULTILINE)
        self.assertIsNotNone(docker_match)
        self.assertEqual(docker_match.group(1), RUSTFMT_CATALOG_SOURCE_RUST_VERSION)

        cargo_toml = (PROJECT_ROOT / "Cargo.toml").read_text(encoding="utf-8")
        cargo_match = re.search(r'^rust-version = "([^"]+)"$', cargo_toml, re.MULTILINE)
        self.assertIsNotNone(cargo_match)
        rust_version = cargo_match.group(1)
        # rust-version reflects MSRV from tech-stack.md, not Dockerfile toolchain version.
        # Enforce valid semver format.
        self.assertRegex(
            rust_version,
            r"^\d+\.\d+(\.\d+)?$",
            f"rust-version must be numeric major.minor[.patch]: {rust_version}",
        )
        # Verify rust-version matches MSRV declared in tech-stack.md (SSoT).
        tech_stack = (PROJECT_ROOT / "track" / "tech-stack.md").read_text(
            encoding="utf-8"
        )
        msrv_match = re.search(
            r"^\- \*\*MSRV\*\*:\s*(\d+\.\d+(?:\.\d+)?)", tech_stack, re.MULTILINE
        )
        self.assertIsNotNone(msrv_match, "tech-stack.md must declare MSRV")
        self.assertEqual(
            rust_version,
            msrv_match.group(1),
            "Cargo.toml rust-version must match tech-stack.md MSRV",
        )
        # Docker toolchain must be >= MSRV so container CI can build the workspace.
        docker_version = tuple(int(x) for x in docker_match.group(1).split("."))
        msrv_tuple = tuple(int(x) for x in rust_version.split("."))
        self.assertGreaterEqual(
            docker_version[:2],
            msrv_tuple[:2],
            f"Dockerfile RUST_VERSION ({docker_match.group(1)}) must be >= MSRV ({rust_version})",
        )

        def extract_first_toml_block(path: Path) -> list[str]:
            content = path.read_text(encoding="utf-8")
            marker = "```toml\n"
            start = content.index(marker) + len(marker)
            end = content.index("\n```", start)
            return content[start:end].splitlines()

        dev_env_lines = extract_first_toml_block(
            PROJECT_ROOT / ".claude" / "rules" / "07-dev-environment.md"
        )
        self.assertEqual(dev_env_lines, active_overrides)

        dev_env = (
            PROJECT_ROOT / ".claude" / "rules" / "07-dev-environment.md"
        ).read_text(encoding="utf-8")
        self.assertIn("full catalog", dev_env)
        self.assertIn("warning", dev_env)

    def test_dockerfile_uses_requirements_python_txt_as_ssot(self) -> None:
        """Dockerfile must install Python deps from requirements-python.txt (SSoT)."""
        dockerfile = (PROJECT_ROOT / "Dockerfile").read_text(encoding="utf-8")
        # Must COPY the file into the image
        self.assertRegex(
            dockerfile,
            r"COPY\s+requirements-python\.txt\s",
            "Dockerfile must COPY requirements-python.txt",
        )
        # Must install from the file via uv pip install -r
        # The command may span multiple lines with backslash continuations,
        # so we check each part independently.
        self.assertIn(
            "uv pip install",
            dockerfile,
            "Dockerfile must use uv pip install",
        )
        self.assertRegex(
            dockerfile,
            r"-r\s+/tmp/requirements-python\.txt",
            "Dockerfile must install from -r /tmp/requirements-python.txt",
        )
        # Must not define its own RUFF_VERSION ARG
        self.assertNotRegex(
            dockerfile,
            r"ARG RUFF_VERSION",
            "Dockerfile must not define RUFF_VERSION ARG — "
            "requirements-python.txt is the SSoT",
        )

        requirements = (PROJECT_ROOT / "requirements-python.txt").read_text(
            encoding="utf-8"
        )
        # Require full semver pin (major.minor.patch) on its own line
        self.assertRegex(
            requirements,
            r"(?m)^ruff==\d+\.\d+\.\d+$",
            "requirements-python.txt must pin ruff with full semver (e.g. ruff==0.15.5)",
        )

    # --- async-trait drift prevention ---
    # These tests verify the template's DEFAULT examples use sync traits.
    # When a project adopts an async runtime via track/tech-stack.md,
    # these files should be updated and these tests adjusted accordingly.

    def test_rules_04_uses_sync_trait_baseline(self) -> None:
        """04-coding-principles.md should use sync trait examples by default."""
        content = (
            PROJECT_ROOT / ".claude" / "rules" / "04-coding-principles.md"
        ).read_text(encoding="utf-8")
        self.assertNotIn("#[async_trait]", content)
        # Async note should point to tech-stack decision
        self.assertIn("track/tech-stack.md", content)

    def test_rules_05_uses_sync_mock_baseline(self) -> None:
        """05-testing.md should use sync mock examples by default."""
        content = (PROJECT_ROOT / ".claude" / "rules" / "05-testing.md").read_text(
            encoding="utf-8"
        )
        # Code blocks should not contain async-trait attribute or tokio test macro.
        # Note: conditional notes in blockquotes (> ...) may mention these for guidance,
        # so we check that they don't appear as actual code (preceded by newline, not >).
        import re

        # No #[async_trait] as actual code (not inside a > blockquote line)
        async_attr_in_code = re.findall(
            r"^(?!>).*#\[async_trait\]", content, re.MULTILINE
        )
        self.assertEqual(
            async_attr_in_code, [], "async_trait attribute found in code blocks"
        )
        # No #[tokio::test] as actual code
        tokio_in_code = re.findall(r"^(?!>).*#\[tokio::test\]", content, re.MULTILINE)
        self.assertEqual(tokio_in_code, [], "tokio::test macro found in code blocks")
        self.assertIn("#[automock]", content)
        # Async note should point to tech-stack decision
        self.assertIn("track/tech-stack.md", content)

    # --- Gemini multimodal docs drift prevention ---
    # Multimodal examples should use path-in-prompt, not stdin redirect.

    _MULTIMODAL_REDIRECT_RE = re.compile(
        r"<\s*\S+\.(?:pdf|png|jpe?g|gif|webp|mp4|mov|avi|mp3|wav|m4a)(?=[\s\"'`);]|$)",
        re.IGNORECASE,
    )

    def test_gemini_skill_uses_path_in_prompt(self) -> None:
        """gemini-system SKILL.md multimodal examples should use path-in-prompt."""
        content = (
            PROJECT_ROOT / ".claude" / "skills" / "gemini-system" / "SKILL.md"
        ).read_text(encoding="utf-8")
        # No stdin redirect for multimodal files
        hits = self._MULTIMODAL_REDIRECT_RE.findall(content)
        self.assertEqual(hits, [], "stdin redirect found for multimodal files")
        # File-Based Briefing Pattern should still exist (text-based stdin is OK)
        self.assertIn("File-Based Briefing Pattern", content)
        self.assertIn("tmp/gemini-briefing.md", content)

    # --- Sandbox/Hook Coverage Warning migration (T08) ---

    def test_sandbox_warning_migrated_to_guardrails(self) -> None:
        """Sandbox/Hook Coverage Warning must exist in 10-guardrails.md after 02-codex-delegation.md deletion."""
        content = (
            PROJECT_ROOT / ".claude" / "rules" / "10-guardrails.md"
        ).read_text(encoding="utf-8")
        self.assertIn("Sandbox and Hook Coverage Warning", content)
        self.assertIn("workspace-write", content)
        self.assertIn("block-direct-git-ops", content)
        # Consequences section: guard-bypass warning must be preserved
        self.assertIn("bypassing the `sotp` guard hook", content)
        self.assertIn("Consequences when using", content)
        # --full-auto implies --sandbox workspace-write warning
        self.assertIn("--full-auto", content)
        self.assertIn("--sandbox workspace-write", content)

    def test_deleted_rule_files_do_not_exist(self) -> None:
        """02-codex-delegation.md and 03-gemini-delegation.md must not exist."""
        self.assertFalse(
            (PROJECT_ROOT / ".claude" / "rules" / "02-codex-delegation.md").exists(),
            "02-codex-delegation.md should have been deleted",
        )
        self.assertFalse(
            (PROJECT_ROOT / ".claude" / "rules" / "03-gemini-delegation.md").exists(),
            "03-gemini-delegation.md should have been deleted",
        )

    def test_security_convention_references_guardrails(self) -> None:
        """security.md must reference 10-guardrails.md, not deleted 02-codex-delegation.md."""
        content = (
            PROJECT_ROOT / "knowledge" / "conventions" / "security.md"
        ).read_text(encoding="utf-8")
        self.assertIn("10-guardrails.md", content)
        self.assertNotIn("02-codex-delegation.md", content)

    # Tests for the old `.claude/hooks/lint-on-save.py` Python hook have been
    # deleted along with the hook itself in track python-hooks-removal-2026-04-10.
    # There is no replacement target to assert against — the lint-on-save
    # watcher is no longer part of the workflow. If a new lint-on-save-style
    # hook is reintroduced in the future, add fresh tests for that hook here.


if __name__ == "__main__":
    unittest.main()
