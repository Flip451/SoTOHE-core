import json
import os
import re
import subprocess
import sys
import tempfile
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
    def run_script(self, script_name: str, setup) -> subprocess.CompletedProcess[str]:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = Path(tmp_dir)
            scripts_dir = root / "scripts"
            scripts_dir.mkdir(parents=True, exist_ok=True)
            for source_path in (PROJECT_ROOT / "scripts").glob("*.py"):
                (scripts_dir / source_path.name).write_text(
                    source_path.read_text(encoding="utf-8"), encoding="utf-8"
                )
            target_script = scripts_dir / script_name
            target_script.write_text(
                (PROJECT_ROOT / "scripts" / script_name).read_text(encoding="utf-8"),
                encoding="utf-8",
            )
            setup(root)
            return subprocess.run(
                ["bash", str(target_script)],
                cwd=root,
                env={**os.environ, "PYTHON_BIN": sys.executable},
                text=True,
                capture_output=True,
                check=False,
            )

    def run_python_script(
        self, script_name: str, setup
    ) -> subprocess.CompletedProcess[str]:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = Path(tmp_dir)
            scripts_dir = root / "scripts"
            scripts_dir.mkdir(parents=True, exist_ok=True)
            for source_path in (PROJECT_ROOT / "scripts").glob("*.py"):
                (scripts_dir / source_path.name).write_text(
                    source_path.read_text(encoding="utf-8"), encoding="utf-8"
                )
            target_script = scripts_dir / script_name
            target_script.write_text(
                (PROJECT_ROOT / "scripts" / script_name).read_text(encoding="utf-8"),
                encoding="utf-8",
            )
            setup(root)
            return subprocess.run(
                [sys.executable, str(target_script)],
                cwd=root,
                env={**os.environ},
                text=True,
                capture_output=True,
                check=False,
            )

    def claude_fixture(self) -> str:
        return "project-docs/conventions/\n"

    def setup_verify_orchestra_fixture(
        self, root: Path, *, minified: bool = False
    ) -> None:
        settings = json.loads(
            (PROJECT_ROOT / ".claude" / "settings.json").read_text(encoding="utf-8")
        )
        settings_path = root / ".claude" / "settings.json"
        settings_path.parent.mkdir(parents=True, exist_ok=True)
        serialized = (
            json.dumps(settings, separators=(",", ":"))
            if minified
            else json.dumps(settings, indent=2)
        )
        settings_path.write_text(serialized + "\n", encoding="utf-8")

        agent_profiles_path = root / ".claude" / "agent-profiles.json"
        agent_profiles_path.parent.mkdir(parents=True, exist_ok=True)
        agent_profiles_path.write_text(
            (PROJECT_ROOT / ".claude" / "agent-profiles.json").read_text(
                encoding="utf-8"
            ),
            encoding="utf-8",
        )
        research_doc_path = (
            root / ".claude" / "docs" / "research" / "planner-pr-review-cycle-2026-03-12.md"
        )
        research_doc_path.parent.mkdir(parents=True, exist_ok=True)
        research_doc_path.write_text(
            (
                PROJECT_ROOT
                / ".claude"
                / "docs"
                / "research"
                / "planner-pr-review-cycle-2026-03-12.md"
            ).read_text(encoding="utf-8"),
            encoding="utf-8",
        )

        hooks_dir = root / ".claude" / "hooks"
        hooks_dir.mkdir(parents=True, exist_ok=True)
        for source_path in (PROJECT_ROOT / ".claude" / "hooks").glob("*.py"):
            (hooks_dir / source_path.name).write_text(
                source_path.read_text(encoding="utf-8"), encoding="utf-8"
            )

        agents_dir = root / ".claude" / "agents"
        agents_dir.mkdir(parents=True, exist_ok=True)
        for source_path in (PROJECT_ROOT / ".claude" / "agents").glob("*.md"):
            (agents_dir / source_path.name).write_text(
                source_path.read_text(encoding="utf-8"), encoding="utf-8"
            )

        for skill_path in (PROJECT_ROOT / ".claude" / "skills").rglob("SKILL.md"):
            target = root / skill_path.relative_to(PROJECT_ROOT)
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_text(skill_path.read_text(encoding="utf-8"), encoding="utf-8")

        for command_path in (PROJECT_ROOT / ".claude" / "commands").rglob("*.md"):
            target = root / command_path.relative_to(PROJECT_ROOT)
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_text(
                command_path.read_text(encoding="utf-8"), encoding="utf-8"
            )

        for rule_path in (PROJECT_ROOT / ".claude" / "rules").glob("*.md"):
            target = root / rule_path.relative_to(PROJECT_ROOT)
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_text(rule_path.read_text(encoding="utf-8"), encoding="utf-8")

    def setup_verify_architecture_docs_fixture(self, root: Path) -> None:
        scripts_dir = root / "scripts"
        (scripts_dir / "convention_docs.py").write_text(
            (PROJECT_ROOT / "scripts" / "convention_docs.py").read_text(
                encoding="utf-8"
            ),
            encoding="utf-8",
        )
        (scripts_dir / "architecture_rules.py").write_text(
            (PROJECT_ROOT / "scripts" / "architecture_rules.py").read_text(
                encoding="utf-8"
            ),
            encoding="utf-8",
        )
        (root / "Cargo.toml").write_text(
            "\n".join(
                [
                    "[workspace]",
                    'members = ["libs/domain", "libs/usecase", "libs/infrastructure", "apps/api", "apps/server"]',
                ]
            )
            + "\n",
            encoding="utf-8",
        )
        docs_dir = root / "docs"
        docs_dir.mkdir(parents=True, exist_ok=True)
        (docs_dir / "architecture-rules.json").write_text(
            json.dumps(
                {
                    "version": 1,
                    "layers": [
                        {
                            "crate": "domain",
                            "path": "libs/domain",
                            "may_depend_on": [],
                            "deny_reason": "domain",
                        },
                        {
                            "crate": "usecase",
                            "path": "libs/usecase",
                            "may_depend_on": ["domain"],
                            "deny_reason": "usecase",
                        },
                        {
                            "crate": "infrastructure",
                            "path": "libs/infrastructure",
                            "may_depend_on": ["domain"],
                            "deny_reason": "infra",
                        },
                        {
                            "crate": "api",
                            "path": "apps/api",
                            "may_depend_on": ["usecase"],
                            "deny_reason": "api",
                        },
                        {
                            "crate": "server",
                            "path": "apps/server",
                            "may_depend_on": [
                                "api",
                                "domain",
                                "infrastructure",
                                "usecase",
                            ],
                            "deny_reason": "",
                        },
                    ],
                },
                ensure_ascii=False,
                indent=2,
            )
            + "\n",
            encoding="utf-8",
        )
        (root / "deny.toml").write_text(
            "\n".join(
                [
                    "deny = [",
                    '  { crate = "infrastructure", wrappers = ["server"], reason = "infra" },',
                    '  { crate = "api", wrappers = ["server"], reason = "api" },',
                    '  { crate = "usecase", wrappers = ["api", "server"], reason = "usecase" },',
                    '  { crate = "domain", wrappers = ["infrastructure", "server", "usecase"], reason = "domain" },',
                    "]",
                ]
            )
            + "\n",
            encoding="utf-8",
        )
        (root / "track").mkdir(parents=True, exist_ok=True)
        (root / "track" / "tech-stack.md").write_text(
            "\n".join(
                [
                    "libs/domain",
                    "libs/usecase",
                    "libs/infrastructure",
                    "apps/api",
                    "apps/server",
                ]
            )
            + "\n",
            encoding="utf-8",
        )
        (root / "track" / "workflow.md").write_text(
            "\n".join(
                [
                    "`cargo make check-layers` passes",
                    "`cargo make verify-plan-progress` passes",
                    "`cargo make verify-track-metadata` passes",
                    "`cargo make verify-tech-stack` passes",
                    "`cargo make scripts-selftest` passes",
                    "`cargo make hooks-selftest` passes",
                    "`cargo make verify-orchestra` passes",
                    "`cargo make verify-latest-track` passes",
                    "/track:revert",
                    "D[Infra Layer] --> C",
                ]
            )
            + "\n",
            encoding="utf-8",
        )
        (root / "CLAUDE.md").write_text(
            self.claude_fixture(), encoding="utf-8"
        )
        (root / ".codex").mkdir(parents=True, exist_ok=True)
        (root / ".codex" / "instructions.md").write_text(
            "base instructions\n", encoding="utf-8"
        )
        (docs_dir / "README.md").write_text("base docs\n", encoding="utf-8")
        (root / "DEVELOPER_AI_WORKFLOW.md").write_text(
            "\n".join(
                [
                    "cargo make verify-orchestra",
                    "cargo make verify-track-metadata",
                    "cargo make verify-tech-stack",
                    "cargo make verify-latest-track",
                    "/track:revert",
                    "cargo make scripts-selftest",
                    "cargo make hooks-selftest",
                ]
            )
            + "\n",
            encoding="utf-8",
        )
        (root / "TAKT_TRACK_TRACEABILITY.md").write_text(
            "\n".join(
                [
                    "Responsibility Split (Fixed)",
                    "scripts-selftest-local",
                    "hooks-selftest-local",
                    "verify-latest-track-local",
                    "cargo make ci",
                ]
            )
            + "\n",
            encoding="utf-8",
        )

    def test_verify_plan_progress_rejects_missing_schema_version(self) -> None:
        """Track without schema_version should be rejected (v2-only)."""

        def setup(root: Path) -> None:
            track_dir = root / "track" / "items" / "demo"
            track_dir.mkdir(parents=True, exist_ok=True)
            (track_dir / "metadata.json").write_text(
                json.dumps(
                    {
                        "id": "demo",
                        "title": "Demo",
                        "status": "planned",
                        "created_at": "2026-03-02",
                        "updated_at": "2026-03-02",
                    }
                )
                + "\n",
                encoding="utf-8",
            )
            (track_dir / "spec.md").write_text("# spec\n", encoding="utf-8")
            (track_dir / "plan.md").write_text("- [x] done\n", encoding="utf-8")

        result = self.run_python_script("verify_plan_progress.py", setup)

        self.assertEqual(result.returncode, 1)
        self.assertIn("schema_version must be 2", result.stdout)

    def test_verify_track_metadata_rejects_invalid_json(self) -> None:
        def setup(root: Path) -> None:
            track_dir = root / "track" / "items" / "demo"
            track_dir.mkdir(parents=True, exist_ok=True)
            (track_dir / "metadata.json").write_text(
                '{"id":"demo", broken, "title":"x", "status":"todo"}\n',
                encoding="utf-8",
            )

        result = self.run_python_script("verify_track_metadata.py", setup)

        self.assertEqual(result.returncode, 1)
        self.assertIn("Cannot read metadata.json", result.stdout)

    def test_verify_track_metadata_rejects_non_utf8_json(self) -> None:
        """Non-UTF8 metadata.json should produce a read error."""

        def setup(root: Path) -> None:
            track_dir = root / "track" / "items" / "demo"
            track_dir.mkdir(parents=True, exist_ok=True)
            (track_dir / "metadata.json").write_bytes(b"\x80\x81\x82")

        result = self.run_python_script("verify_track_metadata.py", setup)

        self.assertEqual(result.returncode, 1)
        self.assertIn("Cannot read metadata.json", result.stdout)

    def test_verify_track_metadata_rejects_missing_schema_version(self) -> None:
        """v1 metadata (no schema_version) should be rejected — v2 only."""

        def setup(root: Path) -> None:
            track_dir = root / "track" / "items" / "demo"
            track_dir.mkdir(parents=True, exist_ok=True)
            (track_dir / "metadata.json").write_text(
                json.dumps(
                    {
                        "id": "demo",
                        "title": "Demo",
                        "status": "planned",
                        "created_at": "2026-03-02",
                        "updated_at": "2026-03-02",
                    }
                )
                + "\n",
                encoding="utf-8",
            )

        result = self.run_python_script("verify_track_metadata.py", setup)

        self.assertEqual(result.returncode, 1)
        self.assertIn("schema_version must be 2", result.stdout)

    # -- Phase 5: v2 metadata.json SSoT integration tests --

    def test_verify_track_metadata_v2_full_validation(self) -> None:
        def setup(root: Path) -> None:
            track_dir = root / "track" / "items" / "demo"
            track_dir.mkdir(parents=True, exist_ok=True)
            (track_dir / "metadata.json").write_text(
                json.dumps(
                    {
                        "schema_version": 2,
                        "id": "demo",
                        "title": "Demo Track",
                        "status": "planned",
                        "created_at": "2026-03-08T00:00:00Z",
                        "updated_at": "2026-03-08T12:00:00Z",
                        "status_override": None,
                        "tasks": [
                            {
                                "id": "T001",
                                "description": "task",
                                "status": "todo",
                                "commit_hash": None,
                            }
                        ],
                        "plan": {
                            "summary": [],
                            "sections": [
                                {
                                    "id": "s1",
                                    "title": "Section",
                                    "description": [],
                                    "task_ids": ["T001"],
                                }
                            ],
                        },
                    }
                )
                + "\n",
                encoding="utf-8",
            )

        result = self.run_python_script("verify_track_metadata.py", setup)

        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("v2 schema validation passed", result.stdout)

    def test_verify_track_metadata_v2_rejects_status_drift(self) -> None:
        def setup(root: Path) -> None:
            track_dir = root / "track" / "items" / "demo"
            track_dir.mkdir(parents=True, exist_ok=True)
            (track_dir / "metadata.json").write_text(
                json.dumps(
                    {
                        "schema_version": 2,
                        "id": "demo",
                        "title": "Demo Track",
                        "status": "done",
                        "created_at": "2026-03-08T00:00:00Z",
                        "updated_at": "2026-03-08T12:00:00Z",
                        "status_override": None,
                        "tasks": [
                            {
                                "id": "T001",
                                "description": "task",
                                "status": "todo",
                                "commit_hash": None,
                            }
                        ],
                        "plan": {
                            "summary": [],
                            "sections": [
                                {
                                    "id": "s1",
                                    "title": "Section",
                                    "description": [],
                                    "task_ids": ["T001"],
                                }
                            ],
                        },
                    }
                )
                + "\n",
                encoding="utf-8",
            )

        result = self.run_python_script("verify_track_metadata.py", setup)

        self.assertEqual(result.returncode, 1)
        self.assertIn("Status drift", result.stdout)

    def test_verify_track_metadata_scans_archive_directory(self) -> None:
        """verify_track_metadata should validate tracks in track/archive/ as well."""

        def setup(root: Path) -> None:
            track_dir = root / "track" / "archive" / "old-feat"
            track_dir.mkdir(parents=True, exist_ok=True)
            (track_dir / "metadata.json").write_text(
                json.dumps(
                    {
                        "schema_version": 2,
                        "id": "old-feat",
                        "title": "Old Feature",
                        "status": "archived",
                        "created_at": "2026-01-01T00:00:00Z",
                        "updated_at": "2026-03-01T00:00:00Z",
                        "status_override": None,
                        "tasks": [
                            {
                                "id": "T001",
                                "description": "task",
                                "status": "done",
                                "commit_hash": "abc1234",
                            }
                        ],
                        "plan": {
                            "summary": [],
                            "sections": [
                                {
                                    "id": "s1",
                                    "title": "Section",
                                    "description": [],
                                    "task_ids": ["T001"],
                                }
                            ],
                        },
                    }
                )
                + "\n",
                encoding="utf-8",
            )

        result = self.run_python_script("verify_track_metadata.py", setup)

        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("v2 schema validation passed", result.stdout)

    def test_verify_plan_progress_v2_plan_in_sync(self) -> None:
        """v2 track with plan.md matching metadata.json should pass."""

        def setup(root: Path) -> None:
            import sys

            sys.path.insert(0, str(PROJECT_ROOT / "scripts"))
            from track_markdown import render_plan
            from track_schema import parse_metadata_v2

            track_dir = root / "track" / "items" / "demo"
            track_dir.mkdir(parents=True, exist_ok=True)
            data = {
                "schema_version": 2,
                "id": "demo",
                "title": "Demo Track",
                "status": "planned",
                "created_at": "2026-03-08T00:00:00Z",
                "updated_at": "2026-03-08T12:00:00Z",
                "status_override": None,
                "tasks": [
                    {
                        "id": "T001",
                        "description": "task",
                        "status": "todo",
                        "commit_hash": None,
                    }
                ],
                "plan": {
                    "summary": [],
                    "sections": [
                        {
                            "id": "s1",
                            "title": "Section",
                            "description": [],
                            "task_ids": ["T001"],
                        }
                    ],
                },
            }
            (track_dir / "metadata.json").write_text(
                json.dumps(data) + "\n", encoding="utf-8"
            )
            (track_dir / "spec.md").write_text("# spec\n", encoding="utf-8")
            meta = parse_metadata_v2(data)
            (track_dir / "plan.md").write_text(render_plan(meta), encoding="utf-8")

        result = self.run_python_script("verify_plan_progress.py", setup)

        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("plan.md is in sync with metadata.json", result.stdout)

    def test_verify_plan_progress_v2_plan_out_of_sync(self) -> None:
        """v2 track with stale plan.md should fail."""

        def setup(root: Path) -> None:
            track_dir = root / "track" / "items" / "demo"
            track_dir.mkdir(parents=True, exist_ok=True)
            data = {
                "schema_version": 2,
                "id": "demo",
                "title": "Demo Track",
                "status": "planned",
                "created_at": "2026-03-08T00:00:00Z",
                "updated_at": "2026-03-08T12:00:00Z",
                "status_override": None,
                "tasks": [
                    {
                        "id": "T001",
                        "description": "task",
                        "status": "todo",
                        "commit_hash": None,
                    }
                ],
                "plan": {
                    "summary": [],
                    "sections": [
                        {
                            "id": "s1",
                            "title": "Section",
                            "description": [],
                            "task_ids": ["T001"],
                        }
                    ],
                },
            }
            (track_dir / "metadata.json").write_text(
                json.dumps(data) + "\n", encoding="utf-8"
            )
            (track_dir / "spec.md").write_text("# spec\n", encoding="utf-8")
            (track_dir / "plan.md").write_text(
                "# stale plan content\n", encoding="utf-8"
            )

        result = self.run_python_script("verify_plan_progress.py", setup)

        self.assertEqual(result.returncode, 1)
        self.assertIn("plan.md is out of sync with metadata.json", result.stdout)
        self.assertIn("Read-only violation", result.stdout)
        self.assertIn("SSoT guidance", result.stdout)
        self.assertIn("transition_task()", result.stdout)

    def test_verify_plan_progress_scans_archive_directory(self) -> None:
        """verify_plan_progress should validate tracks in track/archive/ as well."""

        def setup(root: Path) -> None:
            import sys

            sys.path.insert(0, str(PROJECT_ROOT / "scripts"))
            from track_markdown import render_plan
            from track_schema import parse_metadata_v2

            meta = {
                "schema_version": 2,
                "id": "old-feat",
                "title": "Old Feature",
                "status": "archived",
                "created_at": "2026-01-01T00:00:00Z",
                "updated_at": "2026-03-01T00:00:00Z",
                "status_override": None,
                "tasks": [
                    {
                        "id": "T001",
                        "description": "archived task",
                        "status": "done",
                        "commit_hash": "abc1234",
                    }
                ],
                "plan": {
                    "summary": ["Archived feature summary"],
                    "sections": [
                        {
                            "id": "s1",
                            "title": "Archive Section",
                            "description": [],
                            "task_ids": ["T001"],
                        }
                    ],
                },
            }
            track_dir = root / "track" / "archive" / "old-feat"
            track_dir.mkdir(parents=True, exist_ok=True)
            (track_dir / "metadata.json").write_text(
                json.dumps(meta) + "\n", encoding="utf-8"
            )
            parsed = parse_metadata_v2(meta)
            plan_text = render_plan(parsed)
            (track_dir / "plan.md").write_text(plan_text, encoding="utf-8")
            (track_dir / "spec.md").write_text("# spec\n", encoding="utf-8")

        result = self.run_python_script("verify_plan_progress.py", setup)

        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)

    def test_verify_track_metadata_rejects_unsupported_schema_version(self) -> None:
        """schema_version other than 2 or 3 should be rejected."""

        def setup(root: Path) -> None:
            track_dir = root / "track" / "items" / "demo"
            track_dir.mkdir(parents=True, exist_ok=True)
            (track_dir / "metadata.json").write_text(
                json.dumps({"schema_version": 99, "id": "demo"}) + "\n",
                encoding="utf-8",
            )

        result = self.run_python_script("verify_track_metadata.py", setup)

        self.assertEqual(result.returncode, 1)
        self.assertIn("schema_version must be 2 or 3", result.stdout)

    def test_verify_track_metadata_rejects_non_dict_json(self) -> None:
        """metadata.json with array root should be rejected."""

        def setup(root: Path) -> None:
            track_dir = root / "track" / "items" / "demo"
            track_dir.mkdir(parents=True, exist_ok=True)
            (track_dir / "metadata.json").write_text("[1,2,3]\n", encoding="utf-8")

        result = self.run_python_script("verify_track_metadata.py", setup)

        self.assertEqual(result.returncode, 1)
        self.assertIn("root must be an object", result.stdout)

    def test_verify_plan_progress_rejects_unsupported_schema_version(self) -> None:
        """schema_version other than 2 or 3 should produce an error."""

        def setup(root: Path) -> None:
            track_dir = root / "track" / "items" / "demo"
            track_dir.mkdir(parents=True, exist_ok=True)
            (track_dir / "metadata.json").write_text(
                json.dumps({"schema_version": 99, "id": "demo"}) + "\n",
                encoding="utf-8",
            )
            (track_dir / "spec.md").write_text("# spec\n", encoding="utf-8")
            (track_dir / "plan.md").write_text("- [x] done\n", encoding="utf-8")

        result = self.run_python_script("verify_plan_progress.py", setup)

        self.assertEqual(result.returncode, 1)
        self.assertIn("schema_version must be 2 or 3", result.stdout)

    def test_verify_plan_progress_rejects_non_dict_json(self) -> None:
        """metadata.json with array root should produce an error."""

        def setup(root: Path) -> None:
            track_dir = root / "track" / "items" / "demo"
            track_dir.mkdir(parents=True, exist_ok=True)
            (track_dir / "metadata.json").write_text("[1]\n", encoding="utf-8")
            (track_dir / "spec.md").write_text("# spec\n", encoding="utf-8")
            (track_dir / "plan.md").write_text("- [x] done\n", encoding="utf-8")

        result = self.run_python_script("verify_plan_progress.py", setup)

        self.assertEqual(result.returncode, 1)
        self.assertIn("root must be an object", result.stdout)

    def test_verify_plan_progress_rejects_corrupt_json(self) -> None:
        """Corrupt metadata.json should produce an error, not fall through to v1."""

        def setup(root: Path) -> None:
            track_dir = root / "track" / "items" / "demo"
            track_dir.mkdir(parents=True, exist_ok=True)
            (track_dir / "metadata.json").write_text(
                "{invalid json\n", encoding="utf-8"
            )
            (track_dir / "spec.md").write_text("# spec\n", encoding="utf-8")
            (track_dir / "plan.md").write_text("- [x] done\n", encoding="utf-8")

        result = self.run_python_script("verify_plan_progress.py", setup)

        self.assertEqual(result.returncode, 1)
        self.assertIn("Cannot read metadata.json", result.stdout)

    def test_verify_plan_progress_rejects_non_utf8_metadata(self) -> None:
        """Non-UTF8 metadata.json should produce a read error in plan progress."""

        def setup(root: Path) -> None:
            track_dir = root / "track" / "items" / "demo"
            track_dir.mkdir(parents=True, exist_ok=True)
            (track_dir / "metadata.json").write_bytes(b"\x80\x81\x82")
            (track_dir / "plan.md").write_text("- [x] done\n", encoding="utf-8")

        result = self.run_python_script("verify_plan_progress.py", setup)

        self.assertEqual(result.returncode, 1)
        self.assertIn("Cannot read metadata.json", result.stdout)

    def test_verify_plan_progress_convention_path_extraction(self) -> None:
        from verify_plan_progress import extract_convention_paths

        plan_text = """# Plan

## Related Conventions (Required Reading)

- `project-docs/conventions/api-design.md`
- `project-docs/conventions/error-handling.md`

## Task List
"""
        paths = extract_convention_paths(plan_text)
        self.assertEqual(
            paths,
            [
                "project-docs/conventions/api-design.md",
                "project-docs/conventions/error-handling.md",
            ],
        )

    def test_verify_plan_progress_convention_path_none_listed(self) -> None:
        from verify_plan_progress import extract_convention_paths

        plan_text = """# Plan

## Related Conventions (Required Reading)

- None

## Tasks
"""
        paths = extract_convention_paths(plan_text)
        self.assertEqual(paths, [])

    def test_verify_plan_progress_convention_path_missing_file(self) -> None:
        from verify_plan_progress import validate_convention_paths

        plan_text = """## Related Conventions (Required Reading)

- `project-docs/conventions/nonexistent.md`
"""
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            errors = validate_convention_paths(plan_text, root)
            self.assertEqual(len(errors), 1)
            self.assertIn("nonexistent.md", errors[0])

    def test_verify_plan_progress_convention_path_traversal_rejected(self) -> None:
        from verify_plan_progress import validate_convention_paths

        plan_text = """## Related Conventions (Required Reading)

- `project-docs/conventions/../../CLAUDE.md`
"""
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            errors = validate_convention_paths(plan_text, root)
            self.assertEqual(len(errors), 1)
            self.assertIn("..", errors[0])

    def test_verify_plan_progress_convention_path_existing_file(self) -> None:
        from verify_plan_progress import validate_convention_paths

        plan_text = """## Related Conventions (Required Reading)

- `project-docs/conventions/api-design.md`
"""
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            conv_file = root / "project-docs" / "conventions" / "api-design.md"
            conv_file.parent.mkdir(parents=True)
            conv_file.write_text("# API Design\n", encoding="utf-8")
            errors = validate_convention_paths(plan_text, root)
            self.assertEqual(errors, [])

    def test_verify_latest_track_checks_newest_directory(self) -> None:
        def setup(root: Path) -> None:
            older = root / "track" / "items" / "old"
            newer = root / "track" / "items" / "new"
            older.mkdir(parents=True, exist_ok=True)
            newer.mkdir(parents=True, exist_ok=True)

            for path, content in (
                ("spec.md", "ok\n"),
                ("plan.md", "- [ ] old task\n"),
                ("verification.md", "verified\n"),
            ):
                target = older / path
                target.write_text(content, encoding="utf-8")
                os.utime(target, (10, 10))

            (older / "metadata.json").write_text(
                json.dumps({"updated_at": "2026-03-01T00:00:00Z"}) + "\n",
                encoding="utf-8",
            )

            for path, content in (("spec.md", "ok\n"), ("plan.md", "- [ ] new task\n")):
                target = newer / path
                target.write_text(content, encoding="utf-8")
                os.utime(target, (20, 20))

            (newer / "metadata.json").write_text(
                json.dumps({"updated_at": "2026-03-02T00:00:00Z"}) + "\n",
                encoding="utf-8",
            )

        result = self.run_python_script("verify_latest_track_files.py", setup)

        self.assertEqual(result.returncode, 1)
        self.assertIn(
            "Latest track is missing verification.md: track/items/new/verification.md",
            result.stdout,
        )

    def test_verify_latest_track_uses_metadata_updated_at_instead_of_file_mtime(
        self,
    ) -> None:
        def setup(root: Path) -> None:
            older = root / "track" / "items" / "old"
            newer = root / "track" / "items" / "new"
            older.mkdir(parents=True, exist_ok=True)
            newer.mkdir(parents=True, exist_ok=True)

            for path, content in (
                ("spec.md", "ok\n"),
                ("plan.md", "- [ ] old task\n"),
                ("verification.md", "verified\n"),
            ):
                old_target = older / path
                old_target.write_text(content, encoding="utf-8")
                os.utime(old_target, (25, 25))

            for path, content in (
                ("spec.md", "ok\n"),
                ("plan.md", "- [ ] new task\n"),
                ("verification.md", "verified\n"),
            ):
                new_target = newer / path
                new_target.write_text(content, encoding="utf-8")
                os.utime(new_target, (5, 5))

            (older / "metadata.json").write_text(
                json.dumps({"updated_at": "2026-03-01T00:00:00Z"}) + "\n",
                encoding="utf-8",
            )
            (newer / "metadata.json").write_text(
                json.dumps({"updated_at": "2026-03-02T00:00:00Z"}) + "\n",
                encoding="utf-8",
            )

        result = self.run_python_script("verify_latest_track_files.py", setup)

        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn(
            "Latest track has complete spec.md, plan.md, and verification.md: track/items/new",
            result.stdout,
        )

    def test_verify_latest_track_ignores_auxiliary_file_mtime_and_uses_metadata(
        self,
    ) -> None:
        def setup(root: Path) -> None:
            older = root / "track" / "items" / "old"
            newer = root / "track" / "items" / "new"
            older.mkdir(parents=True, exist_ok=True)
            newer.mkdir(parents=True, exist_ok=True)

            for path, content in (("spec.md", "ok\n"), ("plan.md", "- [ ] old task\n")):
                target = older / path
                target.write_text(content, encoding="utf-8")
                os.utime(target, (10, 10))

            verification = older / "verification.md"
            verification.write_text("verified\n", encoding="utf-8")
            os.utime(verification, (10, 10))
            (older / "metadata.json").write_text(
                json.dumps({"updated_at": "2026-03-01T00:00:00Z"}) + "\n",
                encoding="utf-8",
            )

            for path, content in (
                ("spec.md", "ok\n"),
                ("plan.md", "- [ ] new task\n"),
                ("verification.md", "verified\n"),
            ):
                target = newer / path
                target.write_text(content, encoding="utf-8")
                os.utime(target, (20, 20))
            (newer / "metadata.json").write_text(
                json.dumps({"updated_at": "2026-03-02T00:00:00Z"}) + "\n",
                encoding="utf-8",
            )

            scratch = older / "notes.md"
            scratch.write_text("latest scratch only\n", encoding="utf-8")
            os.utime(scratch, (30, 30))

        result = self.run_python_script("verify_latest_track_files.py", setup)

        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn(
            "Latest track has complete spec.md, plan.md, and verification.md: track/items/new",
            result.stdout,
        )

    def test_verify_latest_track_rejects_placeholder_content(self) -> None:
        def setup(root: Path) -> None:
            track_dir = root / "track" / "items" / "demo"
            track_dir.mkdir(parents=True, exist_ok=True)
            (track_dir / "metadata.json").write_text(
                json.dumps({"updated_at": "2026-03-02"}) + "\n",
                encoding="utf-8",
            )
            (track_dir / "spec.md").write_text("TODO: fill spec\n", encoding="utf-8")
            (track_dir / "plan.md").write_text(
                "- [ ] implement feature\n", encoding="utf-8"
            )
            (track_dir / "verification.md").write_text(
                "Verified manually.\n", encoding="utf-8"
            )

        result = self.run_python_script("verify_latest_track_files.py", setup)

        self.assertEqual(result.returncode, 1)
        self.assertIn("spec.md still contains placeholders", result.stdout)

    def test_verify_latest_track_rejects_heading_only_verification(self) -> None:
        def setup(root: Path) -> None:
            track_dir = root / "track" / "items" / "demo"
            track_dir.mkdir(parents=True, exist_ok=True)
            (track_dir / "metadata.json").write_text(
                json.dumps({"updated_at": "2026-03-02"}) + "\n",
                encoding="utf-8",
            )
            (track_dir / "spec.md").write_text("実装対象の概要\n", encoding="utf-8")
            (track_dir / "plan.md").write_text(
                "- [ ] implement feature\n", encoding="utf-8"
            )
            (track_dir / "verification.md").write_text(
                "# scope verified\n# manual verification steps\n# result / open issues\n# verified_at\n",
                encoding="utf-8",
            )

        result = self.run_python_script("verify_latest_track_files.py", setup)

        self.assertEqual(result.returncode, 1)
        self.assertIn(
            "verification.md lacks substantive content beyond headings", result.stdout
        )

    def test_verify_latest_track_accepts_multiline_plan_with_task_items(self) -> None:
        def setup(root: Path) -> None:
            track_dir = root / "track" / "items" / "demo"
            track_dir.mkdir(parents=True, exist_ok=True)
            (track_dir / "metadata.json").write_text(
                json.dumps({"updated_at": "2026-03-02"}) + "\n",
                encoding="utf-8",
            )
            (track_dir / "spec.md").write_text("実装対象の概要\n", encoding="utf-8")
            (track_dir / "plan.md").write_text(
                "作業計画\n\n- [ ] first task\n- [x] second task\n",
                encoding="utf-8",
            )
            (track_dir / "verification.md").write_text(
                "Verified manually.\n", encoding="utf-8"
            )

        result = self.run_python_script("verify_latest_track_files.py", setup)

        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn(
            "Latest track has complete spec.md, plan.md, and verification.md",
            result.stdout,
        )

    def test_verify_latest_track_rejects_scaffold_only_verification_bullets(
        self,
    ) -> None:
        def setup(root: Path) -> None:
            track_dir = root / "track" / "items" / "demo"
            track_dir.mkdir(parents=True, exist_ok=True)
            (track_dir / "metadata.json").write_text(
                json.dumps({"updated_at": "2026-03-02"}) + "\n",
                encoding="utf-8",
            )
            (track_dir / "spec.md").write_text("実装対象の概要\n", encoding="utf-8")
            (track_dir / "plan.md").write_text(
                "- [ ] implement feature\n", encoding="utf-8"
            )
            (track_dir / "verification.md").write_text(
                "- scope verified\n- manual verification steps\n- result / open issues\n- verified_at\n",
                encoding="utf-8",
            )

        result = self.run_python_script("verify_latest_track_files.py", setup)

        self.assertEqual(result.returncode, 1)
        self.assertIn(
            "verification.md still contains scaffold placeholders", result.stdout
        )

    def test_verify_latest_track_rejects_verification_with_mixed_scaffold_and_real_content(
        self,
    ) -> None:
        def setup(root: Path) -> None:
            track_dir = root / "track" / "items" / "demo"
            track_dir.mkdir(parents=True, exist_ok=True)
            (track_dir / "metadata.json").write_text(
                json.dumps({"updated_at": "2026-03-02"}) + "\n",
                encoding="utf-8",
            )
            (track_dir / "spec.md").write_text("実装対象の概要\n", encoding="utf-8")
            (track_dir / "plan.md").write_text(
                "- [ ] implement feature\n", encoding="utf-8"
            )
            (track_dir / "verification.md").write_text(
                "\n".join(
                    [
                        "- scope verified",
                        "- manual verification steps",
                        "Verified manually on Linux.",
                        "- result / open issues",
                        "- verified_at",
                    ]
                )
                + "\n",
                encoding="utf-8",
            )

        result = self.run_python_script("verify_latest_track_files.py", setup)

        self.assertEqual(result.returncode, 1)
        self.assertIn(
            "verification.md still contains scaffold placeholders", result.stdout
        )
        self.assertIn("1:- scope verified", result.stdout)

    def test_verify_latest_track_skips_archived_tracks(self) -> None:
        def setup(root: Path) -> None:
            archived_dir = root / "track" / "items" / "archived-feat"
            archived_dir.mkdir(parents=True, exist_ok=True)
            (archived_dir / "metadata.json").write_text(
                json.dumps({"updated_at": "2026-03-08T00:00:00Z", "status": "archived"})
                + "\n",
                encoding="utf-8",
            )
            (archived_dir / "spec.md").write_text("done spec\n", encoding="utf-8")
            (archived_dir / "plan.md").write_text("- [x] done task\n", encoding="utf-8")
            (archived_dir / "verification.md").write_text(
                "verified\n", encoding="utf-8"
            )

            active_dir = root / "track" / "items" / "active-feat"
            active_dir.mkdir(parents=True, exist_ok=True)
            (active_dir / "metadata.json").write_text(
                json.dumps(
                    {"updated_at": "2026-03-07T00:00:00Z", "status": "in_progress"}
                )
                + "\n",
                encoding="utf-8",
            )
            (active_dir / "spec.md").write_text("active spec\n", encoding="utf-8")
            (active_dir / "plan.md").write_text("- [ ] active task\n", encoding="utf-8")
            (active_dir / "verification.md").write_text(
                "pending verification\n", encoding="utf-8"
            )

        result = self.run_python_script("verify_latest_track_files.py", setup)

        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("track/items/active-feat", result.stdout)
        self.assertNotIn("archived-feat", result.stdout)

    def test_verify_latest_track_all_archived_skips_check(self) -> None:
        def setup(root: Path) -> None:
            archived_dir = root / "track" / "items" / "old-feat"
            archived_dir.mkdir(parents=True, exist_ok=True)
            (archived_dir / "metadata.json").write_text(
                json.dumps({"updated_at": "2026-03-08T00:00:00Z", "status": "archived"})
                + "\n",
                encoding="utf-8",
            )

        result = self.run_python_script("verify_latest_track_files.py", setup)

        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("No tracks yet", result.stdout)

    def test_verify_latest_track_skips_tracks_in_archive_directory(self) -> None:
        """Tracks physically in track/archive/ should be skipped as archived."""

        def setup(root: Path) -> None:
            # Track in track/archive/ — should be skipped (no status check needed)
            archived_dir = root / "track" / "archive" / "old-feat"
            archived_dir.mkdir(parents=True, exist_ok=True)
            (archived_dir / "metadata.json").write_text(
                json.dumps({"updated_at": "2026-03-08T00:00:00Z", "status": "archived"})
                + "\n",
                encoding="utf-8",
            )

            # Active track in track/items/ — should be selected and pass
            active_dir = root / "track" / "items" / "active-feat"
            active_dir.mkdir(parents=True, exist_ok=True)
            (active_dir / "metadata.json").write_text(
                json.dumps(
                    {"updated_at": "2026-03-07T00:00:00Z", "status": "in_progress"}
                )
                + "\n",
                encoding="utf-8",
            )
            (active_dir / "spec.md").write_text("active spec\n", encoding="utf-8")
            (active_dir / "plan.md").write_text(
                "- [ ] active task\n", encoding="utf-8"
            )
            (active_dir / "verification.md").write_text(
                "pending verification\n", encoding="utf-8"
            )

        result = self.run_python_script("verify_latest_track_files.py", setup)

        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("track/items/active-feat", result.stdout)
        self.assertNotIn("old-feat", result.stdout)

    def test_verify_latest_track_archive_dir_with_malformed_metadata_does_not_fail(
        self,
    ) -> None:
        """Malformed metadata.json in track/archive/ must not cause verify to fail."""

        def setup(root: Path) -> None:
            # Malformed metadata in track/archive/ — must be skipped by path
            bad_arch = root / "track" / "archive" / "bad-arch"
            bad_arch.mkdir(parents=True, exist_ok=True)
            (bad_arch / "metadata.json").write_text("{not json}\n", encoding="utf-8")

            # Valid active track in track/items/
            active_dir = root / "track" / "items" / "active-feat"
            active_dir.mkdir(parents=True, exist_ok=True)
            (active_dir / "metadata.json").write_text(
                json.dumps(
                    {"updated_at": "2026-03-07T00:00:00Z", "status": "in_progress"}
                )
                + "\n",
                encoding="utf-8",
            )
            (active_dir / "spec.md").write_text("active spec\n", encoding="utf-8")
            (active_dir / "plan.md").write_text(
                "- [ ] active task\n", encoding="utf-8"
            )
            (active_dir / "verification.md").write_text(
                "pending verification\n", encoding="utf-8"
            )

        result = self.run_python_script("verify_latest_track_files.py", setup)

        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("track/items/active-feat", result.stdout)

    def test_verify_latest_track_handles_non_string_status_gracefully(self) -> None:
        def setup(root: Path) -> None:
            track_dir = root / "track" / "items" / "bad-status"
            track_dir.mkdir(parents=True, exist_ok=True)
            (track_dir / "metadata.json").write_text(
                json.dumps(
                    {
                        "updated_at": "2026-03-08T00:00:00Z",
                        "status": ["not", "a", "string"],
                    }
                )
                + "\n",
                encoding="utf-8",
            )
            (track_dir / "spec.md").write_text("spec content\n", encoding="utf-8")
            (track_dir / "plan.md").write_text("- [ ] some task\n", encoding="utf-8")
            (track_dir / "verification.md").write_text("verified\n", encoding="utf-8")

        result = self.run_python_script("verify_latest_track_files.py", setup)

        # Should not crash with TypeError; non-string status treated as non-archived
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)

    def test_verify_latest_track_todo_inside_code_block_passes(self) -> None:
        def setup(root: Path) -> None:
            track_dir = root / "track" / "items" / "demo"
            track_dir.mkdir(parents=True, exist_ok=True)
            (track_dir / "metadata.json").write_text(
                json.dumps({"updated_at": "2026-03-02"}) + "\n",
                encoding="utf-8",
            )
            (track_dir / "spec.md").write_text(
                "\n".join(
                    [
                        "Feature description here.",
                        "",
                        "```rust",
                        "// TODO: implement this later",
                        "fn placeholder() {}",
                        "```",
                        "",
                    ]
                )
                + "\n",
                encoding="utf-8",
            )
            (track_dir / "plan.md").write_text(
                "- [ ] implement feature\n", encoding="utf-8"
            )
            (track_dir / "verification.md").write_text(
                "Verified manually.\n", encoding="utf-8"
            )

        result = self.run_python_script("verify_latest_track_files.py", setup)

        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertNotIn("placeholders", result.stdout)

    def test_verify_latest_track_todo_outside_code_block_fails(self) -> None:
        def setup(root: Path) -> None:
            track_dir = root / "track" / "items" / "demo"
            track_dir.mkdir(parents=True, exist_ok=True)
            (track_dir / "metadata.json").write_text(
                json.dumps({"updated_at": "2026-03-02"}) + "\n",
                encoding="utf-8",
            )
            (track_dir / "spec.md").write_text(
                "\n".join(
                    [
                        "Feature description here.",
                        "",
                        "```rust",
                        "// TODO: this is inside a fence and should be ignored",
                        "```",
                        "",
                        "TODO: this is outside and should be flagged",
                        "",
                    ]
                )
                + "\n",
                encoding="utf-8",
            )
            (track_dir / "plan.md").write_text(
                "- [ ] implement feature\n", encoding="utf-8"
            )
            (track_dir / "verification.md").write_text(
                "Verified manually.\n", encoding="utf-8"
            )

        result = self.run_python_script("verify_latest_track_files.py", setup)

        self.assertEqual(result.returncode, 1)
        self.assertIn("spec.md still contains placeholders", result.stdout)

    def test_verify_latest_track_japanese_scaffold_headings_detected(self) -> None:
        def setup(root: Path) -> None:
            track_dir = root / "track" / "items" / "demo"
            track_dir.mkdir(parents=True, exist_ok=True)
            (track_dir / "metadata.json").write_text(
                json.dumps({"updated_at": "2026-03-02"}) + "\n",
                encoding="utf-8",
            )
            (track_dir / "spec.md").write_text("実装対象の概要\n", encoding="utf-8")
            (track_dir / "plan.md").write_text(
                "- [ ] implement feature\n", encoding="utf-8"
            )
            (track_dir / "verification.md").write_text(
                "\n".join(
                    [
                        "## Scope",
                        "- 検証範囲",
                        "## Steps",
                        "- 手動検証手順",
                        "## Results",
                        "- 結果 / 未解決事項",
                        "## Date",
                        "- 検証日",
                    ]
                )
                + "\n",
                encoding="utf-8",
            )

        result = self.run_python_script("verify_latest_track_files.py", setup)

        self.assertEqual(result.returncode, 1)
        self.assertIn(
            "verification.md still contains scaffold placeholders", result.stdout
        )

    def test_verify_tech_stack_fails_by_default_without_tracks(self) -> None:
        def setup(root: Path) -> None:
            track_dir = root / "track"
            track_dir.mkdir(parents=True, exist_ok=True)
            (track_dir / "tech-stack.md").write_text(
                "- **DB**: `TODO: PostgreSQL / SQLite / MySQL / なし`\n",
                encoding="utf-8",
            )
            # Add an in_progress track so the planning-phase bypass does not fire
            items_dir = track_dir / "items" / "task-a"
            items_dir.mkdir(parents=True, exist_ok=True)
            (items_dir / "metadata.json").write_text(
                '{"status": "in_progress"}', encoding="utf-8"
            )

        result = self.run_python_script("verify_tech_stack_ready.py", setup)

        self.assertEqual(result.returncode, 1)
        self.assertIn("Unresolved tech stack TODOs found", result.stdout)

    def test_verify_tech_stack_enforces_todos_even_with_marker_when_tracks_exist(
        self,
    ) -> None:
        def setup(root: Path) -> None:
            track_dir = root / "track"
            track_dir.mkdir(parents=True, exist_ok=True)
            (track_dir / "tech-stack.md").write_text(
                "- **DB**: `TODO: PostgreSQL / SQLite / MySQL / なし`\n",
                encoding="utf-8",
            )
            (root / ".track-template-dev").write_text("", encoding="utf-8")
            items_a = root / "track" / "items" / "a"
            items_a.mkdir(parents=True, exist_ok=True)
            (items_a / "metadata.json").write_text(
                '{"status": "in_progress"}', encoding="utf-8"
            )
            items_b = root / "track" / "items" / "b"
            items_b.mkdir(parents=True, exist_ok=True)
            (items_b / "metadata.json").write_text(
                '{"status": "planned"}', encoding="utf-8"
            )

        result = self.run_python_script("verify_tech_stack_ready.py", setup)

        self.assertEqual(result.returncode, 1)
        self.assertIn("Unresolved tech stack TODOs found", result.stdout)

    def test_verify_tech_stack_skips_in_template_dev_mode_without_tracks(self) -> None:
        def setup(root: Path) -> None:
            track_dir = root / "track"
            track_dir.mkdir(parents=True, exist_ok=True)
            (track_dir / "tech-stack.md").write_text(
                "- **DB**: `TODO: PostgreSQL / SQLite / MySQL / なし`\n",
                encoding="utf-8",
            )
            (root / ".track-template-dev").write_text("", encoding="utf-8")

        result = self.run_python_script("verify_tech_stack_ready.py", setup)

        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("Template development mode is enabled", result.stdout)

    def test_verify_tech_stack_detects_bare_todo_without_backticks(self) -> None:
        def setup(root: Path) -> None:
            track_dir = root / "track"
            track_dir.mkdir(parents=True, exist_ok=True)
            (track_dir / "tech-stack.md").write_text(
                "- **DB**: TODO: PostgreSQL / SQLite\n", encoding="utf-8"
            )
            items_dir = track_dir / "items" / "task-a"
            items_dir.mkdir(parents=True, exist_ok=True)
            (items_dir / "metadata.json").write_text(
                '{"status": "in_progress"}', encoding="utf-8"
            )

        result = self.run_python_script("verify_tech_stack_ready.py", setup)

        self.assertEqual(result.returncode, 1)
        self.assertIn("Unresolved tech stack TODOs found", result.stdout)

    def test_verify_tech_stack_detects_bare_todo_in_table_row(self) -> None:
        def setup(root: Path) -> None:
            track_dir = root / "track"
            track_dir.mkdir(parents=True, exist_ok=True)
            (track_dir / "tech-stack.md").write_text(
                "| DB | TODO: decide |\n", encoding="utf-8"
            )
            items_dir = track_dir / "items" / "task-a"
            items_dir.mkdir(parents=True, exist_ok=True)
            (items_dir / "metadata.json").write_text(
                '{"status": "in_progress"}', encoding="utf-8"
            )

        result = self.run_python_script("verify_tech_stack_ready.py", setup)

        self.assertEqual(result.returncode, 1)
        self.assertIn("Unresolved tech stack TODOs found", result.stdout)

    def test_verify_tech_stack_detects_bare_todo_in_reason_line(self) -> None:
        def setup(root: Path) -> None:
            track_dir = root / "track"
            track_dir.mkdir(parents=True, exist_ok=True)
            (track_dir / "tech-stack.md").write_text(
                "理由: TODO: 未決定\n", encoding="utf-8"
            )
            items_dir = track_dir / "items" / "task-a"
            items_dir.mkdir(parents=True, exist_ok=True)
            (items_dir / "metadata.json").write_text(
                '{"status": "in_progress"}', encoding="utf-8"
            )

        result = self.run_python_script("verify_tech_stack_ready.py", setup)

        self.assertEqual(result.returncode, 1)
        self.assertIn("Unresolved tech stack TODOs found", result.stdout)

    def test_verify_tech_stack_passes_with_resolved_entries(self) -> None:
        def setup(root: Path) -> None:
            track_dir = root / "track"
            track_dir.mkdir(parents=True, exist_ok=True)
            (track_dir / "tech-stack.md").write_text(
                "- **DB**: PostgreSQL\n- **Framework**: axum 0.8\n理由: 実績と安定性\n",
                encoding="utf-8",
            )

        result = self.run_python_script("verify_tech_stack_ready.py", setup)

        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("no blocking TODO", result.stdout)

    def test_verify_tech_stack_skips_in_template_dev_mode_from_env_without_tracks(
        self,
    ) -> None:
        def setup(root: Path) -> None:
            track_dir = root / "track"
            track_dir.mkdir(parents=True, exist_ok=True)
            (track_dir / "tech-stack.md").write_text(
                "- **DB**: `TODO: PostgreSQL / SQLite / MySQL / なし`\n",
                encoding="utf-8",
            )

        with tempfile.TemporaryDirectory() as tmp_dir:
            root = Path(tmp_dir)
            scripts_dir = root / "scripts"
            scripts_dir.mkdir(parents=True, exist_ok=True)
            for source_path in (PROJECT_ROOT / "scripts").glob("*.py"):
                (scripts_dir / source_path.name).write_text(
                    source_path.read_text(encoding="utf-8"), encoding="utf-8"
                )
            setup(root)
            result = subprocess.run(
                [sys.executable, str(scripts_dir / "verify_tech_stack_ready.py")],
                cwd=root,
                env={**os.environ, "TRACK_TEMPLATE_DEV": "1"},
                text=True,
                capture_output=True,
                check=False,
            )

        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("Template development mode is enabled", result.stdout)

    def test_verify_tech_stack_todo_allowed_when_all_planned(self) -> None:
        def setup(root: Path) -> None:
            track_dir = root / "track"
            track_dir.mkdir(parents=True, exist_ok=True)
            (track_dir / "tech-stack.md").write_text(
                "- **DB**: `TODO: PostgreSQL / SQLite / MySQL / なし`\n",
                encoding="utf-8",
            )
            items_dir = track_dir / "items" / "task-a"
            items_dir.mkdir(parents=True, exist_ok=True)
            (items_dir / "metadata.json").write_text(
                '{"status": "planned"}', encoding="utf-8"
            )

        result = self.run_python_script("verify_tech_stack_ready.py", setup)

        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("All tracks are in 'planned' status", result.stdout)

    def test_verify_tech_stack_todo_allowed_when_planned_with_archived_history(
        self,
    ) -> None:
        """Archived tracks in track/archive/ must not block the planning-phase bypass."""

        def setup(root: Path) -> None:
            track_dir = root / "track"
            track_dir.mkdir(parents=True, exist_ok=True)
            (track_dir / "tech-stack.md").write_text(
                "- **DB**: `TODO: PostgreSQL / SQLite / MySQL / なし`\n",
                encoding="utf-8",
            )
            # New active track in planned status
            items_dir = track_dir / "items" / "new-track"
            items_dir.mkdir(parents=True, exist_ok=True)
            (items_dir / "metadata.json").write_text(
                '{"status": "planned"}', encoding="utf-8"
            )
            # Old track already archived in track/archive/ — must be ignored
            arch_dir = track_dir / "archive" / "old-track"
            arch_dir.mkdir(parents=True, exist_ok=True)
            (arch_dir / "metadata.json").write_text(
                '{"status": "archived"}', encoding="utf-8"
            )

        result = self.run_python_script("verify_tech_stack_ready.py", setup)

        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("All tracks are in 'planned' status", result.stdout)

    def test_verify_tech_stack_todo_blocked_when_in_progress(self) -> None:
        def setup(root: Path) -> None:
            track_dir = root / "track"
            track_dir.mkdir(parents=True, exist_ok=True)
            (track_dir / "tech-stack.md").write_text(
                "- **DB**: `TODO: PostgreSQL / SQLite / MySQL / なし`\n",
                encoding="utf-8",
            )
            items_dir = track_dir / "items" / "task-a"
            items_dir.mkdir(parents=True, exist_ok=True)
            (items_dir / "metadata.json").write_text(
                '{"status": "in_progress"}', encoding="utf-8"
            )

        result = self.run_python_script("verify_tech_stack_ready.py", setup)

        self.assertEqual(result.returncode, 1)
        self.assertIn("Unresolved tech stack TODOs found", result.stdout)

    def test_verify_tech_stack_todo_blocked_when_done(self) -> None:
        def setup(root: Path) -> None:
            track_dir = root / "track"
            track_dir.mkdir(parents=True, exist_ok=True)
            (track_dir / "tech-stack.md").write_text(
                "- **DB**: `TODO: PostgreSQL / SQLite / MySQL / なし`\n",
                encoding="utf-8",
            )
            items_dir = track_dir / "items" / "task-a"
            items_dir.mkdir(parents=True, exist_ok=True)
            (items_dir / "metadata.json").write_text(
                '{"status": "done"}', encoding="utf-8"
            )

        result = self.run_python_script("verify_tech_stack_ready.py", setup)

        self.assertEqual(result.returncode, 1)
        self.assertIn("Unresolved tech stack TODOs found", result.stdout)

    def test_verify_tech_stack_fail_closed_missing_metadata(self) -> None:
        def setup(root: Path) -> None:
            track_dir = root / "track"
            track_dir.mkdir(parents=True, exist_ok=True)
            (track_dir / "tech-stack.md").write_text(
                "- **DB**: `TODO: PostgreSQL / SQLite / MySQL / なし`\n",
                encoding="utf-8",
            )
            # Create track dir without metadata.json
            (track_dir / "items" / "task-a").mkdir(parents=True, exist_ok=True)

        result = self.run_python_script("verify_tech_stack_ready.py", setup)

        self.assertEqual(result.returncode, 1)
        self.assertIn("Cannot read track metadata", result.stdout)

    def test_verify_tech_stack_fail_closed_corrupt_metadata(self) -> None:
        def setup(root: Path) -> None:
            track_dir = root / "track"
            track_dir.mkdir(parents=True, exist_ok=True)
            (track_dir / "tech-stack.md").write_text(
                "- **DB**: `TODO: PostgreSQL / SQLite / MySQL / なし`\n",
                encoding="utf-8",
            )
            items_dir = track_dir / "items" / "task-a"
            items_dir.mkdir(parents=True, exist_ok=True)
            (items_dir / "metadata.json").write_text(
                "not valid json{{{", encoding="utf-8"
            )

        result = self.run_python_script("verify_tech_stack_ready.py", setup)

        self.assertEqual(result.returncode, 1)
        self.assertIn("Cannot read track metadata", result.stdout)

    def test_verify_tech_stack_fail_closed_non_object_metadata(self) -> None:
        def setup(root: Path) -> None:
            track_dir = root / "track"
            track_dir.mkdir(parents=True, exist_ok=True)
            (track_dir / "tech-stack.md").write_text(
                "- **DB**: `TODO: PostgreSQL / SQLite / MySQL / なし`\n",
                encoding="utf-8",
            )
            items_dir = track_dir / "items" / "task-a"
            items_dir.mkdir(parents=True, exist_ok=True)
            (items_dir / "metadata.json").write_text("[]", encoding="utf-8")

        result = self.run_python_script("verify_tech_stack_ready.py", setup)

        self.assertEqual(result.returncode, 1)
        self.assertIn("Cannot read track metadata", result.stdout)

    def test_verify_tech_stack_archived_only_blocks_todo(self) -> None:
        def setup(root: Path) -> None:
            track_dir = root / "track"
            track_dir.mkdir(parents=True, exist_ok=True)
            (track_dir / "tech-stack.md").write_text(
                "- **DB**: `TODO: PostgreSQL / SQLite / MySQL / なし`\n",
                encoding="utf-8",
            )
            items_dir = track_dir / "items" / "task-a"
            items_dir.mkdir(parents=True, exist_ok=True)
            (items_dir / "metadata.json").write_text(
                '{"status": "archived"}', encoding="utf-8"
            )

        result = self.run_python_script("verify_tech_stack_ready.py", setup)

        self.assertEqual(result.returncode, 1)
        self.assertIn("Unresolved tech stack TODOs found", result.stdout)

    def test_verify_tech_stack_resolved_passes_with_bad_metadata(self) -> None:
        def setup(root: Path) -> None:
            track_dir = root / "track"
            track_dir.mkdir(parents=True, exist_ok=True)
            (track_dir / "tech-stack.md").write_text(
                "- **DB**: PostgreSQL\n", encoding="utf-8"
            )
            items_dir = track_dir / "items" / "task-a"
            items_dir.mkdir(parents=True, exist_ok=True)
            (items_dir / "metadata.json").write_text("[]", encoding="utf-8")

        result = self.run_python_script("verify_tech_stack_ready.py", setup)

        self.assertEqual(result.returncode, 0)
        self.assertIn("no blocking TODO placeholders", result.stdout)

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

    def test_verify_architecture_docs_skips_conventions_checks_when_not_bootstrapped(
        self,
    ) -> None:
        def setup(root: Path) -> None:
            self.setup_verify_architecture_docs_fixture(root)

        result = self.run_python_script("verify_architecture_docs.py", setup)

        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("project conventions not bootstrapped", result.stdout)
        self.assertIn("verify_architecture_docs PASSED", result.stdout)

    def test_verify_architecture_docs_rejects_partially_bootstrapped_conventions(
        self,
    ) -> None:
        def setup(root: Path) -> None:
            self.setup_verify_architecture_docs_fixture(root)
            conventions_dir = root / "project-docs" / "conventions"
            conventions_dir.mkdir(parents=True, exist_ok=True)
            (conventions_dir / "api-design.md").write_text(
                "# API Design\n", encoding="utf-8"
            )

        result = self.run_python_script("verify_architecture_docs.py", setup)

        self.assertEqual(result.returncode, 1)
        self.assertIn("contains convention documents but is missing", result.stdout)
        self.assertIn("verify_architecture_docs FAILED", result.stdout)

    def test_verify_architecture_docs_rejects_architecture_rule_drift(self) -> None:
        def setup(root: Path) -> None:
            self.setup_verify_architecture_docs_fixture(root)
            (root / "deny.toml").write_text(
                "\n".join(
                    [
                        "deny = [",
                        '  { crate = "domain", wrappers = ["usecase"], reason = "drift" },',
                        "]",
                    ]
                )
                + "\n",
                encoding="utf-8",
            )

        result = self.run_python_script("verify_architecture_docs.py", setup)

        self.assertEqual(result.returncode, 1)
        self.assertIn("deny.toml layer policy mismatch", result.stdout + result.stderr)
        self.assertIn("verify_architecture_docs FAILED", result.stdout)

    def test_verify_architecture_docs_accepts_fully_bootstrapped_conventions(
        self,
    ) -> None:
        def setup(root: Path) -> None:
            self.setup_verify_architecture_docs_fixture(root)
            conventions_dir = root / "project-docs" / "conventions"
            conventions_dir.mkdir(parents=True, exist_ok=True)
            (conventions_dir / "README.md").write_text(
                "\n".join(
                    [
                        "# Project Conventions",
                        "",
                        "<!-- convention-docs:start -->",
                        "- `api-design.md`: API Design",
                        "<!-- convention-docs:end -->",
                        "",
                    ]
                ),
                encoding="utf-8",
            )
            (conventions_dir / "api-design.md").write_text(
                "# API Design\n", encoding="utf-8"
            )
            (root / ".codex" / "instructions.md").write_text(
                "project-docs/conventions/\n", encoding="utf-8"
            )
            (root / "docs" / "README.md").write_text(
                "project-docs/conventions/\n", encoding="utf-8"
            )
            (root / "DEVELOPER_AI_WORKFLOW.md").write_text(
                "\n".join(
                    [
                        "project-docs/conventions/",
                        "cargo make verify-orchestra",
                        "cargo make verify-track-metadata",
                        "cargo make verify-tech-stack",
                        "cargo make verify-latest-track",
                        "/track:revert",
                        "cargo make scripts-selftest",
                        "cargo make hooks-selftest",
                    ]
                )
                + "\n",
                encoding="utf-8",
            )
            commands_dir = root / ".claude" / "commands" / "conventions"
            commands_dir.mkdir(parents=True, exist_ok=True)
            (commands_dir / "add.md").write_text("add convention\n", encoding="utf-8")

        result = self.run_python_script("verify_architecture_docs.py", setup)

        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("project conventions detected", result.stdout)
        self.assertIn("index is in sync", result.stdout)
        self.assertIn("verify_architecture_docs PASSED", result.stdout)

    def test_verify_orchestra_guardrails_accepts_minified_settings_json(self) -> None:
        def setup(root: Path) -> None:
            self.setup_verify_orchestra_fixture(root, minified=True)

        result = self.run_python_script("verify_orchestra_guardrails.py", setup)

        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("agent teams enabled", result.stdout)
        self.assertIn("CLAUDE_CODE_SUBAGENT_MODEL allowlisted", result.stdout)
        self.assertIn("no hardcoded Codex model literals", result.stdout)
        self.assertIn("verify_orchestra_guardrails PASSED", result.stdout)

    def test_verify_orchestra_guardrails_rejects_non_allowlisted_subagent_model(
        self,
    ) -> None:
        def setup(root: Path) -> None:
            self.setup_verify_orchestra_fixture(root, minified=True)
            settings_path = root / ".claude" / "settings.json"
            settings = json.loads(settings_path.read_text(encoding="utf-8"))
            settings["env"]["CLAUDE_CODE_SUBAGENT_MODEL"] = "claude-unknown"
            settings_path.write_text(
                json.dumps(settings, separators=(",", ":")) + "\n", encoding="utf-8"
            )

        result = self.run_python_script("verify_orchestra_guardrails.py", setup)

        self.assertEqual(result.returncode, 1)
        self.assertIn("CLAUDE_CODE_SUBAGENT_MODEL must be one of", result.stdout)
        self.assertIn("verify_orchestra_guardrails FAILED", result.stdout)

    def test_verify_orchestra_guardrails_rejects_hardcoded_codex_model_literal(
        self,
    ) -> None:
        def setup(root: Path) -> None:
            self.setup_verify_orchestra_fixture(root, minified=True)
            skill_path = root / ".claude" / "skills" / "codex-system" / "SKILL.md"
            skill_path.write_text(
                skill_path.read_text(encoding="utf-8") + "\nmodel: gpt-9\n",
                encoding="utf-8",
            )

        result = self.run_python_script("verify_orchestra_guardrails.py", setup)

        self.assertEqual(result.returncode, 1)
        self.assertIn("contains hardcoded Codex model literal", result.stdout)
        self.assertIn("verify_orchestra_guardrails FAILED", result.stdout)

    def test_verify_orchestra_guardrails_rejects_default_model_only_guidance(
        self,
    ) -> None:
        replacements = {
            Path(".claude/commands/track/review.md"): (
                "Resolve `{model}` from `profiles.<active_profile>.provider_model_overrides.<provider>` first, then fall back to `providers.<provider>.default_model`.",
                "Read the provider's `default_model` to get `{model}`.",
            ),
            Path(".claude/skills/codex-system/SKILL.md"): (
                "profiles.<active_profile>.provider_model_overrides.codex  →  {model}\nfallback: providers.codex.default_model  →  {model}",
                "providers.codex.default_model  →  {model}",
            ),
            Path(".claude/skills/track-plan/SKILL.md"): (
                "Resolve `{model}` from `profiles.<active_profile>.provider_model_overrides.codex` first, then `providers.codex.default_model`",
                "Resolve `{model}` from `providers.codex.default_model`",
            ),
        }

        for relative_path, (old, new) in replacements.items():
            with self.subTest(path=str(relative_path)):
                def setup(
                    root: Path,
                    relative_path: Path = relative_path,
                    old: str = old,
                    new: str = new,
                ) -> None:
                    self.setup_verify_orchestra_fixture(root, minified=True)
                    target_path = root / relative_path
                    target_path.write_text(
                        target_path.read_text(encoding="utf-8").replace(old, new),
                        encoding="utf-8",
                    )

                result = self.run_python_script("verify_orchestra_guardrails.py", setup)

                self.assertEqual(result.returncode, 1)
                self.assertIn(
                    "missing canonical override-first guidance", result.stdout
                )
                self.assertIn("verify_orchestra_guardrails FAILED", result.stdout)

    def test_verify_orchestra_guardrails_rejects_missing_default_model_fallback(
        self,
    ) -> None:
        replacements = {
            Path(".claude/commands/track/review.md"): (
                " then fall back to `providers.<provider>.default_model`.",
                "",
            ),
            Path(".claude/skills/codex-system/SKILL.md"): (
                "\nfallback: providers.codex.default_model  →  {model}",
                "",
            ),
            Path(".claude/skills/track-plan/SKILL.md"): (
                " then `providers.codex.default_model`",
                "",
            ),
        }

        for relative_path, (old, new) in replacements.items():
            with self.subTest(path=str(relative_path)):
                def setup(
                    root: Path,
                    relative_path: Path = relative_path,
                    old: str = old,
                    new: str = new,
                ) -> None:
                    self.setup_verify_orchestra_fixture(root, minified=True)
                    target_path = root / relative_path
                    target_path.write_text(
                        target_path.read_text(encoding="utf-8").replace(old, new),
                        encoding="utf-8",
                    )

                result = self.run_python_script("verify_orchestra_guardrails.py", setup)

                self.assertEqual(result.returncode, 1)
                self.assertIn(
                    "missing canonical override-first guidance", result.stdout
                )
                self.assertIn("verify_orchestra_guardrails FAILED", result.stdout)

    def test_verify_orchestra_guardrails_rejects_stale_default_model_only_guidance(
        self,
    ) -> None:
        stale_lines = {
            Path(".claude/commands/track/review.md"): (
                "\nRead the provider's `default_model` to get `{model}`.\n"
            ),
            Path(".claude/skills/codex-system/SKILL.md"): (
                "\nread `providers.codex.default_model` from `.claude/agent-profiles.json` and pass as `--model {model}`\n"
            ),
            Path(".claude/skills/track-plan/SKILL.md"): (
                '\ncodex exec --model gpt-5.3-codex --sandbox read-only --full-auto "\n'
            ),
        }

        for relative_path, stale_line in stale_lines.items():
            with self.subTest(path=str(relative_path)):
                def setup(
                    root: Path,
                    relative_path: Path = relative_path,
                    stale_line: str = stale_line,
                ) -> None:
                    self.setup_verify_orchestra_fixture(root, minified=True)
                    target_path = root / relative_path
                    target_path.write_text(
                        target_path.read_text(encoding="utf-8") + stale_line,
                        encoding="utf-8",
                    )

                result = self.run_python_script("verify_orchestra_guardrails.py", setup)

                self.assertEqual(result.returncode, 1)
                self.assertIn(
                    "still contains stale default_model-only guidance", result.stdout
                )
                self.assertIn("verify_orchestra_guardrails FAILED", result.stdout)

    def test_verify_orchestra_guardrails_rejects_missing_allow_entry(self) -> None:
        def setup(root: Path) -> None:
            self.setup_verify_orchestra_fixture(root, minified=True)
            settings_path = root / ".claude" / "settings.json"
            settings = json.loads(settings_path.read_text(encoding="utf-8"))
            settings["permissions"]["allow"].remove("Bash(cargo make track-local-review:*)")
            settings_path.write_text(
                json.dumps(settings, separators=(",", ":")) + "\n", encoding="utf-8"
            )

        result = self.run_python_script("verify_orchestra_guardrails.py", setup)

        self.assertEqual(result.returncode, 1)
        self.assertIn(
            "Missing in .claude/settings.json: cargo make track-local-review permission",
            result.stdout,
        )
        self.assertIn("verify_orchestra_guardrails FAILED", result.stdout)

    def test_verify_orchestra_guardrails_rejects_stale_raw_reviewer_command_guidance(
        self,
    ) -> None:
        stale_lines = {
            Path(".claude/agent-profiles.json"): (
                '\n        "reviewer": "codex exec review --uncommitted --json --model {model} --full-auto",\n'
            ),
            Path(".claude/commands/track/review.md"): (
                '\ntimeout 180 codex exec --model {model} --sandbox read-only --full-auto "\n'
            ),
            Path(".claude/skills/codex-system/SKILL.md"): (
                '\ntimeout 180 codex exec --model {model} --sandbox read-only --full-auto \\\n  "Review this Rust implementation: {description}"\n'
            ),
            Path(".claude/rules/02-codex-delegation.md"): (
                '\ntimeout 180 codex exec --model {model} --sandbox read-only --full-auto \\\n  "Review this Rust implementation: {description}"\n'
            ),
            Path(".claude/docs/research/planner-pr-review-cycle-2026-03-12.md"): (
                "\ncodex exec review --uncommitted --json --model {model} --full-auto\n"
            ),
        }

        for relative_path, stale_line in stale_lines.items():
            with self.subTest(path=str(relative_path)):
                def setup(
                    root: Path,
                    relative_path: Path = relative_path,
                    stale_line: str = stale_line,
                ) -> None:
                    self.setup_verify_orchestra_fixture(root, minified=True)
                    target_path = root / relative_path
                    target_path.write_text(
                        target_path.read_text(encoding="utf-8") + stale_line,
                        encoding="utf-8",
                    )

                result = self.run_python_script("verify_orchestra_guardrails.py", setup)

                self.assertEqual(result.returncode, 1)
                self.assertIn(
                    "still contains stale reviewer command guidance", result.stdout
                )
                self.assertIn("verify_orchestra_guardrails FAILED", result.stdout)

    def test_verify_orchestra_guardrails_rejects_timeout_600_raw_reviewer_guidance(
        self,
    ) -> None:
        stale_lines = {
            Path(".claude/commands/track/review.md"): (
                '\ntimeout 600 codex exec --model {model} --sandbox read-only --full-auto "\n'
            ),
            Path(".claude/skills/codex-system/SKILL.md"): (
                '\ntimeout 600 codex exec --model {model} --sandbox read-only --full-auto \\\n  "Review this Rust implementation: {description}"\n'
            ),
            Path(".claude/rules/02-codex-delegation.md"): (
                '\ntimeout 600 codex exec --model {model} --sandbox read-only --full-auto \\\n  "Review this Rust implementation: {description}"\n'
            ),
        }

        for relative_path, stale_line in stale_lines.items():
            with self.subTest(path=str(relative_path)):
                def setup(
                    root: Path,
                    relative_path: Path = relative_path,
                    stale_line: str = stale_line,
                ) -> None:
                    self.setup_verify_orchestra_fixture(root, minified=True)
                    target_path = root / relative_path
                    target_path.write_text(
                        target_path.read_text(encoding="utf-8") + stale_line,
                        encoding="utf-8",
                    )

                result = self.run_python_script("verify_orchestra_guardrails.py", setup)

                self.assertEqual(result.returncode, 1)
                self.assertIn(
                    "still contains stale reviewer command guidance", result.stdout
                )
                self.assertIn("verify_orchestra_guardrails FAILED", result.stdout)

    def test_verify_orchestra_guardrails_rejects_raw_review_subcommand_guidance(
        self,
    ) -> None:
        stale_lines = {
            Path(".claude/agent-profiles.json"): (
                '\n        "reviewer": "codex exec review --uncommitted --json --model {model} --full-auto",\n'
            ),
            Path(".claude/rules/02-codex-delegation.md"): (
                '\ncodex exec review --uncommitted --json --model {model} --full-auto\n'
            ),
        }

        for relative_path, stale_line in stale_lines.items():
            with self.subTest(path=str(relative_path)):
                def setup(
                    root: Path,
                    relative_path: Path = relative_path,
                    stale_line: str = stale_line,
                ) -> None:
                    self.setup_verify_orchestra_fixture(root, minified=True)
                    target_path = root / relative_path
                    target_path.write_text(
                        target_path.read_text(encoding="utf-8") + stale_line,
                        encoding="utf-8",
                    )

                result = self.run_python_script("verify_orchestra_guardrails.py", setup)

                self.assertEqual(result.returncode, 1)
                self.assertIn(
                    "still contains stale reviewer command guidance", result.stdout
                )
                self.assertIn("verify_orchestra_guardrails FAILED", result.stdout)

    def test_verify_orchestra_guardrails_rejects_missing_reviewer_json_contract(
        self,
    ) -> None:
        targets = {
            Path(".claude/commands/track/review.md"): [
                '{"verdict":"zero_findings","findings":[]}',
                '{"verdict":"findings_remain","findings":[{"message":"describe the bug","severity":"P1","file":"path/to/file.rs","line":123}]}',
                "Every object field is required by the output schema.",
                "use `null` for that field instead of omitting it.",
            ],
            Path(".claude/skills/codex-system/SKILL.md"): [
                '{"verdict":"zero_findings","findings":[]}',
                '{"verdict":"findings_remain","findings":[{"message":"describe the bug","severity":"P1","file":"path/to/file.rs","line":123}]}',
                "Every object field is required by the output schema.",
                "use `null` for that field instead of omitting it.",
            ],
            Path(".claude/rules/02-codex-delegation.md"): [
                '{"verdict":"zero_findings","findings":[]}',
                '{"verdict":"findings_remain","findings":[{"message":"describe the bug","severity":"P1","file":"path/to/file.rs","line":123}]}',
                "field 自体は省略せず `null` を使う。",
            ],
        }

        for relative_path, snippets in targets.items():
            for snippet in snippets:
                with self.subTest(path=str(relative_path), snippet=snippet):
                    def setup(
                        root: Path,
                        relative_path: Path = relative_path,
                        snippet: str = snippet,
                    ) -> None:
                        self.setup_verify_orchestra_fixture(root, minified=True)
                        target_path = root / relative_path
                        target_path.write_text(
                            target_path.read_text(encoding="utf-8").replace(snippet, ""),
                            encoding="utf-8",
                        )

                    result = self.run_python_script("verify_orchestra_guardrails.py", setup)

                    self.assertEqual(result.returncode, 1)
                    self.assertIn("missing reviewer wrapper guidance", result.stdout)
                    self.assertIn("verify_orchestra_guardrails FAILED", result.stdout)

    def test_verify_orchestra_guardrails_rejects_missing_make_separator_in_reviewer_examples(
        self,
    ) -> None:
        replacements = {
            Path(".claude/agent-profiles.json"): (
                'cargo make track-local-review -- --model {model} --prompt \\"{task}\\"',
                'cargo make track-local-review --model {model} --prompt \\"{task}\\"',
            ),
            Path(".claude/commands/track/review.md"): (
                "cargo make track-local-review -- --model {model} --briefing-file tmp/codex-briefing.md",
                "cargo make track-local-review --model {model} --briefing-file tmp/codex-briefing.md",
            ),
            Path(".claude/skills/codex-system/SKILL.md"): (
                'cargo make track-local-review -- --model {model} --prompt "',
                'cargo make track-local-review --model {model} --prompt "',
            ),
            Path(".claude/rules/02-codex-delegation.md"): (
                "cargo make track-local-review -- --model {model} --prompt \\",
                "cargo make track-local-review --model {model} --prompt \\",
            ),
        }

        for relative_path, (expected, stale) in replacements.items():
            with self.subTest(path=str(relative_path)):
                def setup(
                    root: Path,
                    relative_path: Path = relative_path,
                    expected: str = expected,
                    stale: str = stale,
                ) -> None:
                    self.setup_verify_orchestra_fixture(root, minified=True)
                    target_path = root / relative_path
                    target_path.write_text(
                        target_path.read_text(encoding="utf-8").replace(expected, stale),
                        encoding="utf-8",
                    )

                result = self.run_python_script("verify_orchestra_guardrails.py", setup)

                self.assertEqual(result.returncode, 1)
                self.assertIn("missing reviewer wrapper guidance", result.stdout)
                self.assertIn("verify_orchestra_guardrails FAILED", result.stdout)

    def test_verify_orchestra_guardrails_rejects_missing_direct_hook_dispatch_commands(
        self,
    ) -> None:
        replacements = {
            "block-direct-git-ops": "python3 \"$CLAUDE_PROJECT_DIR/.claude/hooks/block-direct-git-ops.py\"",
            "file-lock-acquire": "python3 \"$CLAUDE_PROJECT_DIR/.claude/hooks/file-lock-acquire.py\"",
            "file-lock-release": "python3 \"$CLAUDE_PROJECT_DIR/.claude/hooks/file-lock-release.py\"",
        }

        for hook_name, replacement in replacements.items():
            with self.subTest(hook=hook_name):
                def setup(
                    root: Path,
                    hook_name: str = hook_name,
                    replacement: str = replacement,
                ) -> None:
                    self.setup_verify_orchestra_fixture(root, minified=True)
                    settings_path = root / ".claude" / "settings.json"
                    settings = json.loads(settings_path.read_text(encoding="utf-8"))
                    for bindings in settings["hooks"].values():
                        for binding in bindings:
                            for hook in binding.get("hooks", []):
                                command = hook.get("command")
                                if isinstance(command, str) and hook_name in command:
                                    hook["command"] = replacement
                    settings_path.write_text(
                        json.dumps(settings, separators=(",", ":")) + "\n",
                        encoding="utf-8",
                    )

                result = self.run_python_script("verify_orchestra_guardrails.py", setup)

                self.assertEqual(result.returncode, 1)
                self.assertIn("Missing in .claude/settings.json:", result.stdout)
                self.assertIn("verify_orchestra_guardrails FAILED", result.stdout)

    def test_verify_orchestra_guardrails_rejects_missing_cache_deny_entry(self) -> None:
        def setup(root: Path) -> None:
            self.setup_verify_orchestra_fixture(root, minified=True)
            settings_path = root / ".claude" / "settings.json"
            settings = json.loads(settings_path.read_text(encoding="utf-8"))
            settings["permissions"]["deny"].remove("Read(./.cache/cargo/**)")
            settings_path.write_text(
                json.dumps(settings, separators=(",", ":")) + "\n", encoding="utf-8"
            )

        result = self.run_python_script("verify_orchestra_guardrails.py", setup)

        self.assertEqual(result.returncode, 1)
        self.assertIn(
            "Missing in .claude/settings.json: cargo cache read deny rule",
            result.stdout,
        )
        self.assertIn("verify_orchestra_guardrails FAILED", result.stdout)

    def test_verify_orchestra_guardrails_rejects_direct_script_permission_with_custom_interpreter(
        self,
    ) -> None:
        def setup(root: Path) -> None:
            self.setup_verify_orchestra_fixture(root, minified=True)
            settings_path = root / ".claude" / "settings.json"
            settings = json.loads(settings_path.read_text(encoding="utf-8"))
            settings["permissions"]["allow"].append(
                "Bash(/opt/python/bin/python3 scripts/architecture_rules.py:*)"
            )
            settings_path.write_text(
                json.dumps(settings, separators=(",", ":")) + "\n", encoding="utf-8"
            )

        result = self.run_python_script("verify_orchestra_guardrails.py", setup)

        self.assertEqual(result.returncode, 1)
        self.assertIn(
            "direct repo scripts must be routed through cargo make wrappers",
            result.stdout,
        )
        self.assertIn("verify_orchestra_guardrails FAILED", result.stdout)

    def test_verify_orchestra_guardrails_rejects_extra_project_allow_entry_without_extension_registry(
        self,
    ) -> None:
        def setup(root: Path) -> None:
            self.setup_verify_orchestra_fixture(root, minified=True)
            settings_path = root / ".claude" / "settings.json"
            settings = json.loads(settings_path.read_text(encoding="utf-8"))
            settings["permissions"]["allow"].append("Bash(cargo make project-custom:*)")
            settings_path.write_text(
                json.dumps(settings, separators=(",", ":")) + "\n", encoding="utf-8"
            )

        result = self.run_python_script("verify_orchestra_guardrails.py", setup)

        self.assertEqual(result.returncode, 1)
        self.assertIn(
            "contains unexpected allow entry: Bash(cargo make project-custom:*)",
            result.stdout,
        )
        self.assertIn("permission-extensions.json", result.stdout)
        self.assertIn("verify_orchestra_guardrails FAILED", result.stdout)

    def test_verify_orchestra_guardrails_rejects_approval_gated_clean_permission(
        self,
    ) -> None:
        def setup(root: Path) -> None:
            self.setup_verify_orchestra_fixture(root, minified=True)
            settings_path = root / ".claude" / "settings.json"
            settings = json.loads(settings_path.read_text(encoding="utf-8"))
            settings["permissions"]["allow"].append("Bash(cargo make clean)")
            settings_path.write_text(
                json.dumps(settings, separators=(",", ":")) + "\n", encoding="utf-8"
            )

        result = self.run_python_script("verify_orchestra_guardrails.py", setup)

        self.assertEqual(result.returncode, 1)
        self.assertIn(
            "contains Bash(cargo make clean) - direct access would be silently allowed",
            result.stdout,
        )
        self.assertIn("verify_orchestra_guardrails FAILED", result.stdout)

    def test_verify_orchestra_guardrails_rejects_missing_env_deny_entry(self) -> None:
        def setup(root: Path) -> None:
            self.setup_verify_orchestra_fixture(root, minified=True)
            settings_path = root / ".claude" / "settings.json"
            settings = json.loads(settings_path.read_text(encoding="utf-8"))
            settings["permissions"]["deny"].remove("Read(./.env)")
            settings_path.write_text(
                json.dumps(settings, separators=(",", ":")) + "\n", encoding="utf-8"
            )

        result = self.run_python_script("verify_orchestra_guardrails.py", setup)

        self.assertEqual(result.returncode, 1)
        self.assertIn(
            "Missing in .claude/settings.json: env file read deny rule", result.stdout
        )
        self.assertIn("verify_orchestra_guardrails FAILED", result.stdout)

    def test_verify_orchestra_guardrails_accepts_extra_deny_entry(self) -> None:
        def setup(root: Path) -> None:
            self.setup_verify_orchestra_fixture(root, minified=True)
            settings_path = root / ".claude" / "settings.json"
            settings = json.loads(settings_path.read_text(encoding="utf-8"))
            settings["permissions"]["deny"].append("Grep(./.env)")
            settings_path.write_text(
                json.dumps(settings, separators=(",", ":")) + "\n", encoding="utf-8"
            )

        result = self.run_python_script("verify_orchestra_guardrails.py", setup)

        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("verify_orchestra_guardrails PASSED", result.stdout)

    def test_verify_orchestra_guardrails_rejects_extra_readonly_git_allow_entry_without_extension_registry(
        self,
    ) -> None:
        def setup(root: Path) -> None:
            self.setup_verify_orchestra_fixture(root, minified=True)
            settings_path = root / ".claude" / "settings.json"
            settings = json.loads(settings_path.read_text(encoding="utf-8"))
            settings["permissions"]["allow"].append("Bash(git merge-base:*)")
            settings_path.write_text(
                json.dumps(settings, separators=(",", ":")) + "\n", encoding="utf-8"
            )

        result = self.run_python_script("verify_orchestra_guardrails.py", setup)

        self.assertEqual(result.returncode, 1)
        self.assertIn(
            "contains unexpected allow entry: Bash(git merge-base:*)", result.stdout
        )
        self.assertIn("permission-extensions.json", result.stdout)
        self.assertIn("verify_orchestra_guardrails FAILED", result.stdout)

    def test_verify_orchestra_guardrails_accepts_extra_allow_entry_with_extension_registry(
        self,
    ) -> None:
        def setup(root: Path) -> None:
            self.setup_verify_orchestra_fixture(root, minified=True)
            settings_path = root / ".claude" / "settings.json"
            settings = json.loads(settings_path.read_text(encoding="utf-8"))
            settings["permissions"]["allow"].append("Bash(cargo make project-custom:*)")
            settings["permissions"]["allow"].append("Bash(git merge-base:*)")
            settings_path.write_text(
                json.dumps(settings, separators=(",", ":")) + "\n", encoding="utf-8"
            )
            extension_path = root / ".claude" / "permission-extensions.json"
            extension_path.write_text(
                json.dumps(
                    {
                        "extra_allow": [
                            "Bash(cargo make project-custom:*)",
                            "Bash(git merge-base:*)",
                        ]
                    },
                    separators=(",", ":"),
                )
                + "\n",
                encoding="utf-8",
            )

        result = self.run_python_script("verify_orchestra_guardrails.py", setup)

        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("verify_orchestra_guardrails PASSED", result.stdout)

    def test_verify_orchestra_guardrails_rejects_unsupported_extension_registry_entries(
        self,
    ) -> None:
        unsupported_entries = [
            "Read(/etc/**)",
            "WebSearch(*)",
            "Bash(curl:*)",
            "Bash(git checkout:*)",
        ]

        def setup(root: Path) -> None:
            self.setup_verify_orchestra_fixture(root, minified=True)
            settings_path = root / ".claude" / "settings.json"
            settings = json.loads(settings_path.read_text(encoding="utf-8"))
            settings["permissions"]["allow"].extend(unsupported_entries)
            settings_path.write_text(
                json.dumps(settings, separators=(",", ":")) + "\n", encoding="utf-8"
            )
            extension_path = root / ".claude" / "permission-extensions.json"
            extension_path.write_text(
                json.dumps({"extra_allow": unsupported_entries}, separators=(",", ":"))
                + "\n",
                encoding="utf-8",
            )

        result = self.run_python_script("verify_orchestra_guardrails.py", setup)

        self.assertEqual(result.returncode, 1)
        self.assertIn(
            "contains unsupported extra_allow entry: Read(/etc/**)",
            result.stdout,
        )
        self.assertIn(
            "contains unsupported extra_allow entry: WebSearch(*)",
            result.stdout,
        )
        self.assertIn(
            "contains unsupported extra_allow entry: Bash(curl:*)",
            result.stdout,
        )
        self.assertIn(
            "contains unsupported extra_allow entry: Bash(git checkout:*)",
            result.stdout,
        )
        self.assertIn("verify_orchestra_guardrails FAILED", result.stdout)

    def test_verify_orchestra_guardrails_rejects_wildcard_bypass_of_guarded_cargo_make_tasks(
        self,
    ) -> None:
        bypass_entries = [
            "Bash(cargo make clean:*)",
            "Bash(cargo make add-all:*)",
        ]

        def setup(root: Path) -> None:
            self.setup_verify_orchestra_fixture(root, minified=True)
            settings_path = root / ".claude" / "settings.json"
            settings = json.loads(settings_path.read_text(encoding="utf-8"))
            settings["permissions"]["allow"].extend(bypass_entries)
            settings_path.write_text(
                json.dumps(settings, separators=(",", ":")) + "\n", encoding="utf-8"
            )
            extension_path = root / ".claude" / "permission-extensions.json"
            extension_path.write_text(
                json.dumps({"extra_allow": bypass_entries}, separators=(",", ":"))
                + "\n",
                encoding="utf-8",
            )

        result = self.run_python_script("verify_orchestra_guardrails.py", setup)

        self.assertEqual(result.returncode, 1)
        self.assertIn(
            "contains extension for guarded cargo make task: Bash(cargo make clean:*)",
            result.stdout,
        )
        self.assertIn(
            "contains extension for guarded cargo make task: Bash(cargo make add-all:*)",
            result.stdout,
        )
        self.assertIn("verify_orchestra_guardrails FAILED", result.stdout)

    def test_verify_orchestra_guardrails_rejects_latent_extension_registry_entry(
        self,
    ) -> None:
        def setup(root: Path) -> None:
            self.setup_verify_orchestra_fixture(root, minified=True)
            extension_path = root / ".claude" / "permission-extensions.json"
            extension_path.write_text(
                json.dumps(
                    {"extra_allow": ["Bash(cargo make latent-custom:*)"]},
                    separators=(",", ":"),
                )
                + "\n",
                encoding="utf-8",
            )

        result = self.run_python_script("verify_orchestra_guardrails.py", setup)

        self.assertEqual(result.returncode, 1)
        self.assertIn(
            "contains latent extra_allow entry not present in .claude/settings.json permissions.allow: "
            "Bash(cargo make latent-custom:*)",
            result.stdout,
        )
        self.assertIn("verify_orchestra_guardrails FAILED", result.stdout)

    def test_verify_orchestra_guardrails_rejects_direct_script_permission_even_with_extension_registry(
        self,
    ) -> None:
        def setup(root: Path) -> None:
            self.setup_verify_orchestra_fixture(root, minified=True)
            settings_path = root / ".claude" / "settings.json"
            settings = json.loads(settings_path.read_text(encoding="utf-8"))
            settings["permissions"]["allow"].append(
                "Bash(/opt/python/bin/python3 scripts/architecture_rules.py:*)"
            )
            settings_path.write_text(
                json.dumps(settings, separators=(",", ":")) + "\n", encoding="utf-8"
            )
            extension_path = root / ".claude" / "permission-extensions.json"
            extension_path.write_text(
                json.dumps(
                    {
                        "extra_allow": [
                            "Bash(/opt/python/bin/python3 scripts/architecture_rules.py:*)",
                        ]
                    },
                    separators=(",", ":"),
                )
                + "\n",
                encoding="utf-8",
            )

        result = self.run_python_script("verify_orchestra_guardrails.py", setup)

        self.assertEqual(result.returncode, 1)
        self.assertIn(
            "direct repo scripts must be routed through cargo make wrappers",
            result.stdout,
        )
        self.assertIn("verify_orchestra_guardrails FAILED", result.stdout)

    def test_verify_orchestra_guardrails_rejects_extra_git_allow_entry(self) -> None:
        def setup(root: Path) -> None:
            self.setup_verify_orchestra_fixture(root, minified=True)
            settings_path = root / ".claude" / "settings.json"
            settings = json.loads(settings_path.read_text(encoding="utf-8"))
            settings["permissions"]["allow"].append("Bash(git fetch:*)")
            settings_path.write_text(
                json.dumps(settings, separators=(",", ":")) + "\n", encoding="utf-8"
            )

        result = self.run_python_script("verify_orchestra_guardrails.py", setup)

        self.assertEqual(result.returncode, 1)
        self.assertIn(
            "contains Bash(git fetch:*) - direct access would be silently allowed",
            result.stdout,
        )
        self.assertIn("verify_orchestra_guardrails FAILED", result.stdout)

    def test_verify_orchestra_guardrails_rejects_missing_private_dir_deny_entry(
        self,
    ) -> None:
        def setup(root: Path) -> None:
            self.setup_verify_orchestra_fixture(root, minified=True)
            settings_path = root / ".claude" / "settings.json"
            settings = json.loads(settings_path.read_text(encoding="utf-8"))
            settings["permissions"]["deny"].remove("Read(./private/**)")
            settings_path.write_text(
                json.dumps(settings, separators=(",", ":")) + "\n", encoding="utf-8"
            )

        result = self.run_python_script("verify_orchestra_guardrails.py", setup)

        self.assertEqual(result.returncode, 1)
        self.assertIn(
            "Missing in .claude/settings.json: private dir read deny rule",
            result.stdout,
        )
        self.assertIn("verify_orchestra_guardrails FAILED", result.stdout)

    def test_verify_orchestra_guardrails_rejects_missing_config_secrets_dir_deny_entry(
        self,
    ) -> None:
        def setup(root: Path) -> None:
            self.setup_verify_orchestra_fixture(root, minified=True)
            settings_path = root / ".claude" / "settings.json"
            settings = json.loads(settings_path.read_text(encoding="utf-8"))
            settings["permissions"]["deny"].remove("Read(./config/secrets/**)")
            settings_path.write_text(
                json.dumps(settings, separators=(",", ":")) + "\n", encoding="utf-8"
            )

        result = self.run_python_script("verify_orchestra_guardrails.py", setup)

        self.assertEqual(result.returncode, 1)
        self.assertIn(
            "Missing in .claude/settings.json: config secrets read deny rule",
            result.stdout,
        )
        self.assertIn("verify_orchestra_guardrails FAILED", result.stdout)

    def test_verify_orchestra_guardrails_rejects_missing_parent_dir_instruction_in_teammate_idle(
        self,
    ) -> None:
        def setup(root: Path) -> None:
            self.setup_verify_orchestra_fixture(root, minified=True)
            settings_path = root / ".claude" / "settings.json"
            settings = json.loads(settings_path.read_text(encoding="utf-8"))
            # Remove the "parent directory" phrase from TeammateIdle feedback
            for binding in settings["hooks"]["TeammateIdle"]:
                for hook in binding["hooks"]:
                    hook["command"] = hook["command"].replace("parent directory", "")
            settings_path.write_text(
                json.dumps(settings, separators=(",", ":")) + "\n", encoding="utf-8"
            )

        result = self.run_python_script("verify_orchestra_guardrails.py", setup)

        self.assertEqual(result.returncode, 1)
        self.assertIn(
            "TeammateIdle feedback instructs creating parent directory", result.stdout
        )
        self.assertIn("verify_orchestra_guardrails FAILED", result.stdout)

    def test_verify_orchestra_guardrails_rejects_missing_agent_teams_dir_in_teammate_idle(
        self,
    ) -> None:
        def setup(root: Path) -> None:
            self.setup_verify_orchestra_fixture(root, minified=True)
            settings_path = root / ".claude" / "settings.json"
            settings = json.loads(settings_path.read_text(encoding="utf-8"))
            # Remove the "agent-teams" phrase from TeammateIdle feedback
            for binding in settings["hooks"]["TeammateIdle"]:
                for hook in binding["hooks"]:
                    hook["command"] = hook["command"].replace("agent-teams", "")
            settings_path.write_text(
                json.dumps(settings, separators=(",", ":")) + "\n", encoding="utf-8"
            )

        result = self.run_python_script("verify_orchestra_guardrails.py", setup)

        self.assertEqual(result.returncode, 1)
        self.assertIn(
            "TeammateIdle feedback references agent-teams log directory", result.stdout
        )
        self.assertIn("verify_orchestra_guardrails FAILED", result.stdout)

    def test_verify_orchestra_guardrails_rejects_readme_only_without_real_agent_files(
        self,
    ) -> None:
        def setup(root: Path) -> None:
            self.setup_verify_orchestra_fixture(root, minified=True)
            agents_dir = root / ".claude" / "agents"
            # Remove all real agent files, leaving only README.md
            for f in agents_dir.glob("*.md"):
                if f.name != "README.md":
                    f.unlink()

        result = self.run_python_script("verify_orchestra_guardrails.py", setup)

        self.assertEqual(result.returncode, 1)
        self.assertIn("Missing required agent definition", result.stdout)
        self.assertIn("orchestrator.md", result.stdout)
        self.assertIn("rust-implementation-lead.md", result.stdout)
        self.assertIn("verify_orchestra_guardrails FAILED", result.stdout)

    def test_verify_orchestra_guardrails_rejects_readme_plus_one_real_agent_file(
        self,
    ) -> None:
        def setup(root: Path) -> None:
            self.setup_verify_orchestra_fixture(root, minified=True)
            agents_dir = root / ".claude" / "agents"
            # Remove one required agent file; README.md + one real file = not enough
            target = agents_dir / "rust-implementation-lead.md"
            self.assertTrue(
                target.exists(), "Fixture must contain rust-implementation-lead.md"
            )
            target.unlink()

        result = self.run_python_script("verify_orchestra_guardrails.py", setup)

        self.assertEqual(result.returncode, 1)
        self.assertIn("Missing required agent definition", result.stdout)
        self.assertIn("rust-implementation-lead.md", result.stdout)
        self.assertIn("verify_orchestra_guardrails FAILED", result.stdout)

    def test_verify_orchestra_guardrails_accepts_readme_plus_two_required_agent_files(
        self,
    ) -> None:
        def setup(root: Path) -> None:
            self.setup_verify_orchestra_fixture(root, minified=True)
            # The fixture already copies all .md files including README.md and both agent files

        result = self.run_python_script("verify_orchestra_guardrails.py", setup)

        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("orchestrator.md: agent definition exists", result.stdout)
        self.assertIn(
            "rust-implementation-lead.md: agent definition exists", result.stdout
        )
        self.assertIn("verify_orchestra_guardrails PASSED", result.stdout)

    def test_verify_orchestra_guardrails_detects_tracked_settings_local_json(
        self,
    ) -> None:
        """If settings.local.json is tracked by git, verify_orchestra should fail."""

        def setup(root: Path) -> None:
            self.setup_verify_orchestra_fixture(root, minified=True)
            # Initialize a git repo so git ls-files works
            subprocess.run(["git", "init"], cwd=root, check=True, capture_output=True)
            subprocess.run(
                ["git", "config", "user.email", "test@example.com"],
                cwd=root,
                check=True,
                capture_output=True,
            )
            subprocess.run(
                ["git", "config", "user.name", "Test"],
                cwd=root,
                check=True,
                capture_output=True,
            )
            subprocess.run(
                ["git", "add", "-A"], cwd=root, check=True, capture_output=True
            )
            subprocess.run(
                ["git", "commit", "-m", "init"],
                cwd=root,
                check=True,
                capture_output=True,
            )
            # Create and force-track settings.local.json (bypassing .gitignore)
            local_settings = root / ".claude" / "settings.local.json"
            local_settings.write_text(
                '{"permissions":{"allow":["Bash(python3:*)"]}}\n', encoding="utf-8"
            )
            subprocess.run(
                ["git", "add", "-f", str(local_settings)],
                cwd=root,
                check=True,
                capture_output=True,
            )
            subprocess.run(
                ["git", "commit", "-m", "add local settings"],
                cwd=root,
                check=True,
                capture_output=True,
            )

        result = self.run_python_script("verify_orchestra_guardrails.py", setup)

        self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
        self.assertIn("settings.local.json is tracked by git", result.stdout)

    def test_verify_orchestra_guardrails_accepts_absent_settings_local_json(
        self,
    ) -> None:
        """When settings.local.json doesn't exist, verify_orchestra should pass."""

        def setup(root: Path) -> None:
            self.setup_verify_orchestra_fixture(root, minified=True)
            # Initialize a git repo so git ls-files works
            subprocess.run(["git", "init"], cwd=root, check=True, capture_output=True)
            subprocess.run(
                ["git", "config", "user.email", "test@example.com"],
                cwd=root,
                check=True,
                capture_output=True,
            )
            subprocess.run(
                ["git", "config", "user.name", "Test"],
                cwd=root,
                check=True,
                capture_output=True,
            )
            subprocess.run(
                ["git", "add", "-A"], cwd=root, check=True, capture_output=True
            )
            subprocess.run(
                ["git", "commit", "-m", "init"],
                cwd=root,
                check=True,
                capture_output=True,
            )

        result = self.run_python_script("verify_orchestra_guardrails.py", setup)

        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("absent or gitignored (expected)", result.stdout)

    def test_verify_orchestra_guardrails_fails_on_git_fatal_inside_repo(self) -> None:
        """A git fatal error (not 'no repo') inside a real repo should fail the check."""

        def setup(root: Path) -> None:
            self.setup_verify_orchestra_fixture(root, minified=True)
            subprocess.run(["git", "init"], cwd=root, check=True, capture_output=True)
            subprocess.run(
                ["git", "config", "user.email", "test@example.com"],
                cwd=root,
                check=True,
                capture_output=True,
            )
            subprocess.run(
                ["git", "config", "user.name", "Test"],
                cwd=root,
                check=True,
                capture_output=True,
            )
            subprocess.run(
                ["git", "add", "-A"], cwd=root, check=True, capture_output=True
            )
            subprocess.run(
                ["git", "commit", "-m", "init"],
                cwd=root,
                check=True,
                capture_output=True,
            )
            # Corrupt the git index to force a fatal exit 128 that is NOT "not a git repo"
            (root / ".git" / "index").write_bytes(b"\x00\x00\x00")

        result = self.run_python_script("verify_orchestra_guardrails.py", setup)

        self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
        self.assertIn("git ls-files failed", result.stdout)

    def test_takt_ci_descriptions_match_current_gate_list(self) -> None:
        expected_condition = (
            "All checks pass: cargo make ci (fmt-check, clippy, test, test-doc, deny, "
            "scripts-selftest, hooks-selftest, check-layers, verify-arch-docs, "
            "verify-plan-progress, verify-track-metadata, verify-track-registry, "
            "verify-tech-stack, verify-orchestra, verify-latest-track)"
        )
        stale_condition = (
            "All checks pass: cargo make ci (fmt-check, clippy, test, test-doc, deny, "
            "check-layers, verify-arch-docs, verify-plan-progress, verify-track-metadata, "
            "verify-track-registry, verify-tech-stack, verify-orchestra, verify-latest-track)"
        )

        for rel_path in (
            ".takt/pieces/spec-to-impl.yaml",
            ".takt/pieces/full-cycle.yaml",
        ):
            content = (PROJECT_ROOT / rel_path).read_text(encoding="utf-8")
            self.assertIn(expected_condition, content)
            self.assertNotIn(stale_condition, content)

        quality_checker = (
            PROJECT_ROOT / ".takt" / "personas" / "quality-checker.md"
        ).read_text(encoding="utf-8")
        self.assertNotIn("- `check`", quality_checker)
        self.assertIn("- `scripts-selftest`", quality_checker)
        self.assertIn("- `hooks-selftest`", quality_checker)
        self.assertIn("- `verify-track-metadata`", quality_checker)
        self.assertIn("- `verify-tech-stack`", quality_checker)
        self.assertIn("spec.md, plan.md, and verification.md", quality_checker)

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

    def test_takt_wrappers_are_legacy_compatibility_surfaces(self) -> None:
        makefile = (PROJECT_ROOT / "Makefile.toml").read_text(encoding="utf-8")
        for task_name in (
            "takt-add",
            "takt-run",
            "takt-render-personas",
            "takt-full-cycle",
            "takt-spec-to-impl",
            "takt-impl-review",
            "takt-tdd-cycle",
        ):
            self.assertIn(f"[tasks.{task_name}]", makefile)
        self.assertIn("TAKT_PYTHON", makefile)
        self.assertIn(".venv/bin/python", makefile)
        self.assertIn('exec "$TAKT_PYTHON" scripts/takt_profile.py run-queue', makefile)
        self.assertIn(
            'exec "$TAKT_PYTHON" scripts/takt_profile.py render-personas', makefile
        )
        self.assertIn(
            'exec "$TAKT_PYTHON" scripts/takt_profile.py add-task "$CARGO_MAKE_TASK_ARGS"',
            makefile,
        )
        self.assertIn(
            'exec "$TAKT_PYTHON" scripts/takt_profile.py run-piece full-cycle "$CARGO_MAKE_TASK_ARGS"',
            makefile,
        )
        self.assertIn("scripts/takt_profile.py", makefile)
        self.assertNotIn("scripts/run-python.sh", makefile)
        self.assertNotIn('command = "/usr/bin/python3"', makefile)

        for rel_path in (
            "LOCAL_DEVELOPMENT.md",
            "DEVELOPER_AI_WORKFLOW.md",
            "track/workflow.md",
        ):
            content = (PROJECT_ROOT / rel_path).read_text(encoding="utf-8")
            self.assertIn("migration compatibility", content)
            self.assertNotIn("`takt prompt` を直接使う場合だけ", content)
        self.assertIn(
            "Python 3.11+",
            (PROJECT_ROOT / "LOCAL_DEVELOPMENT.md").read_text(encoding="utf-8"),
        )
        self.assertIn(
            "Python 3.11+",
            (PROJECT_ROOT / "DEVELOPER_AI_WORKFLOW.md").read_text(encoding="utf-8"),
        )
        self.assertIn(
            ".tool-versions",
            (PROJECT_ROOT / "LOCAL_DEVELOPMENT.md").read_text(encoding="utf-8"),
        )
        self.assertIn(
            ".tool-versions",
            (PROJECT_ROOT / "DEVELOPER_AI_WORKFLOW.md").read_text(encoding="utf-8"),
        )
        self.assertIn(
            "PYTHON_BIN",
            (PROJECT_ROOT / "LOCAL_DEVELOPMENT.md").read_text(encoding="utf-8"),
        )
        self.assertIn(
            "PYTHON_BIN",
            (PROJECT_ROOT / "DEVELOPER_AI_WORKFLOW.md").read_text(encoding="utf-8"),
        )

    def test_human_onboarding_doc_exists_and_is_wired(self) -> None:
        onboarding = (PROJECT_ROOT / "START_HERE_HUMAN.md").read_text(encoding="utf-8")
        self.assertIn("人間と AI の責務境界", onboarding)
        self.assertIn("必須レビュー・承認ポイント", onboarding)
        self.assertIn("人間が修正してよい対象", onboarding)
        self.assertIn("TAKT_TRACK_TRACEABILITY.md", onboarding)
        self.assertIn("レビューや運用判断が必要なとき", onboarding)
        self.assertIn("2章（対応付けルール）", onboarding)
        self.assertIn("4章（Interactive Implementation Contract）", onboarding)
        self.assertIn("docs/architecture-rules.json", onboarding)
        self.assertIn("layers[].path", onboarding)
        self.assertIn("workspace member", onboarding)
        self.assertIn("project-docs/**", onboarding)
        self.assertIn("scripts/**", onboarding)
        self.assertIn(".claude/commands/**", onboarding)
        self.assertIn(".claude/agents/**", onboarding)
        self.assertIn("CLAUDE.md", onboarding)
        self.assertIn("rustfmt.toml", onboarding)
        self.assertIn(".takt/**", onboarding)
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
        self.assertIn("cargo make note-pending", commit_doc)

        # /track:plan now owns track artifact creation (Option C merge)
        plan_doc = (
            PROJECT_ROOT / ".claude" / "commands" / "track" / "plan.md"
        ).read_text(encoding="utf-8")
        self.assertIn("schema_version", plan_doc)
        self.assertIn("match the created track directory name", plan_doc)
        self.assertIn("render_plan()", plan_doc)
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

        workflow_doc = (PROJECT_ROOT / ".claude" / "docs" / "WORKFLOW.md").read_text(
            encoding="utf-8"
        )
        self.assertIn("cargo make tools-up", workflow_doc)
        self.assertIn("Cargo.lock", workflow_doc)

        orchestrator_doc = (
            PROJECT_ROOT / ".claude" / "agents" / "orchestrator.md"
        ).read_text(encoding="utf-8")
        self.assertIn("cargo make tools-up", orchestrator_doc)
        self.assertIn("Cargo.lock", orchestrator_doc)

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

    def test_plan_language_and_diagram_policy_are_aligned(self) -> None:
        plan_doc = (
            PROJECT_ROOT / ".claude" / "commands" / "track" / "plan.md"
        ).read_text(encoding="utf-8")
        self.assertIn("Japanese", plan_doc)
        self.assertIn("flowchart TD", plan_doc)
        self.assertIn("DESIGN.md", plan_doc)

        design_doc = (PROJECT_ROOT / ".claude" / "docs" / "DESIGN.md").read_text(
            encoding="utf-8"
        )
        self.assertIn("plan.md", design_doc)
        self.assertIn("flowchart TD", design_doc)
        self.assertIn("English", design_doc)

        takt_config = (PROJECT_ROOT / ".takt" / "config.yaml").read_text(
            encoding="utf-8"
        )
        self.assertIn("language: ja", takt_config)
        self.assertIn("plan.md", takt_config)
        self.assertIn("flowchart TD", takt_config)

        planner_template = (
            PROJECT_ROOT / ".takt" / "personas" / "rust-planner.md"
        ).read_text(encoding="utf-8")
        self.assertIn("# Implementation Plan:", planner_template)
        self.assertIn("flowchart TD", planner_template)

    # --- async-trait drift prevention ---

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

    def test_gemini_delegation_uses_path_in_prompt(self) -> None:
        """03-gemini-delegation.md multimodal example should use path-in-prompt, not stdin redirect."""
        content = (
            PROJECT_ROOT / ".claude" / "rules" / "03-gemini-delegation.md"
        ).read_text(encoding="utf-8")
        # No stdin redirect for multimodal files
        hits = self._MULTIMODAL_REDIRECT_RE.findall(content)
        self.assertEqual(hits, [], "stdin redirect found for multimodal files")
        # Should reference agent-profiles.json as source of truth
        self.assertIn("agent-profiles.json", content)
        # Should have takt parser note
        self.assertIn("takt_profile.py", content)

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

    def test_lint_on_save_delegates_edition_to_rustfmt_toml(self) -> None:
        """lint-on-save.py should not hardcode --edition; rustfmt.toml is the source of truth."""
        content = (PROJECT_ROOT / ".claude" / "hooks" / "lint-on-save.py").read_text(
            encoding="utf-8"
        )
        self.assertNotIn("--edition", content)

    def test_lint_on_save_clippy_uses_all_targets(self) -> None:
        """lint-on-save.py clippy should use --all-targets to match CI scope."""
        content = (PROJECT_ROOT / ".claude" / "hooks" / "lint-on-save.py").read_text(
            encoding="utf-8"
        )
        self.assertIn("--all-targets", content)

    def test_handoff_uses_per_task_files_not_single_overwrite(self) -> None:
        """takt_profile should use per-task handoff files in .takt/handoffs/ directory."""
        from scripts import git_ops, takt_profile

        # handoff_path should return a path under .takt/handoffs/
        self.assertEqual(takt_profile.HANDOFFS_DIR, "handoffs")
        self.assertFalse(hasattr(takt_profile, "PENDING_HANDOFF_FILE"))

        # git_ops should exclude the handoffs directory, not a single file
        self.assertNotIn(".takt/pending-handoff.md", git_ops.TRANSIENT_AUTOMATION_FILES)
        self.assertIn(".takt/handoffs", git_ops.TRANSIENT_AUTOMATION_DIRS)


if __name__ == "__main__":
    unittest.main()
