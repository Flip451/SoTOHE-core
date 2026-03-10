import io
import json
import tempfile
import unittest
from contextlib import redirect_stderr, redirect_stdout
from pathlib import Path
from unittest import mock

import scripts.architecture_rules as architecture_rules


class ArchitectureRulesTest(unittest.TestCase):
    def test_expected_deny_rules_are_derived_from_layers(self) -> None:
        rules = {
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
                    "crate": "api",
                    "path": "apps/api",
                    "may_depend_on": ["usecase"],
                    "deny_reason": "",
                },
            ],
        }

        self.assertEqual(
            architecture_rules.expected_deny_rules(rules),
            [
                {"crate": "domain", "wrappers": ["usecase"], "reason": "domain"},
                {"crate": "usecase", "wrappers": ["api"], "reason": "usecase"},
            ],
        )

    def test_direct_check_matrix_lists_forbidden_direct_dependencies(self) -> None:
        rules = {
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
                    "crate": "api",
                    "path": "apps/api",
                    "may_depend_on": ["usecase"],
                    "deny_reason": "",
                },
            ],
        }

        self.assertEqual(
            architecture_rules.direct_check_matrix(rules),
            [
                ("domain", ["api", "usecase"]),
                ("usecase", ["api"]),
                ("api", ["domain"]),
            ],
        )

    def test_verify_sync_accepts_matching_files(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = Path(tmp_dir)
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
                                "crate": "api",
                                "path": "apps/api",
                                "may_depend_on": ["usecase"],
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
            (root / "Cargo.toml").write_text(
                "\n".join(
                    [
                        "[workspace]",
                        'members = ["libs/domain", "libs/usecase", "apps/api"]',
                    ]
                )
                + "\n",
                encoding="utf-8",
            )
            (root / "deny.toml").write_text(
                "\n".join(
                    [
                        "deny = [",
                        '  { crate = "usecase", wrappers = ["api"], reason = "usecase" },',
                        '  { crate = "domain", wrappers = ["usecase"], reason = "domain" },',
                        "]",
                    ]
                )
                + "\n",
                encoding="utf-8",
            )

            self.assertEqual(architecture_rules.verify_sync(root), [])

    def test_verify_sync_reports_workspace_drift(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = Path(tmp_dir)
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
                        ],
                    },
                    ensure_ascii=False,
                    indent=2,
                )
                + "\n",
                encoding="utf-8",
            )
            (root / "Cargo.toml").write_text(
                '[workspace]\nmembers = ["libs/other"]\n', encoding="utf-8"
            )
            (root / "deny.toml").write_text("deny = []\n", encoding="utf-8")

            errors = architecture_rules.verify_sync(root)

        self.assertEqual(len(errors), 1)
        self.assertIn("workspace members mismatch", errors[0])

    def test_verify_sync_reports_deny_reason_drift(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = Path(tmp_dir)
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
                        ],
                    },
                    ensure_ascii=False,
                    indent=2,
                )
                + "\n",
                encoding="utf-8",
            )
            (root / "Cargo.toml").write_text(
                "\n".join(
                    [
                        "[workspace]",
                        'members = ["libs/domain", "libs/usecase"]',
                    ]
                )
                + "\n",
                encoding="utf-8",
            )
            (root / "deny.toml").write_text(
                "\n".join(
                    [
                        "deny = [",
                        '  { crate = "domain", wrappers = ["usecase"], reason = "wrong" },',
                        "]",
                    ]
                )
                + "\n",
                encoding="utf-8",
            )

            errors = architecture_rules.verify_sync(root)

        self.assertEqual(len(errors), 1)
        self.assertIn("deny.toml layer policy mismatch", errors[0])

    def test_verify_sync_reports_malformed_deny_entry(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = Path(tmp_dir)
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
                        ],
                    },
                    ensure_ascii=False,
                    indent=2,
                )
                + "\n",
                encoding="utf-8",
            )
            (root / "Cargo.toml").write_text(
                "\n".join(
                    [
                        "[workspace]",
                        'members = ["libs/domain", "libs/usecase"]',
                    ]
                )
                + "\n",
                encoding="utf-8",
            )
            (root / "deny.toml").write_text(
                "\n".join(
                    [
                        "deny = [",
                        '  { crate = "domain", wrappers = ["usecase"] },',
                        "]",
                    ]
                )
                + "\n",
                encoding="utf-8",
            )

            errors = architecture_rules.verify_sync(root)

        self.assertTrue(any("Failed to parse deny.toml" in error for error in errors))

    def test_verify_sync_reports_malformed_cargo_toml(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = Path(tmp_dir)
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
                        ],
                    },
                    ensure_ascii=False,
                    indent=2,
                )
                + "\n",
                encoding="utf-8",
            )
            (root / "Cargo.toml").write_text(
                '[workspace\nmembers = ["libs/domain"]\n', encoding="utf-8"
            )
            (root / "deny.toml").write_text("deny = []\n", encoding="utf-8")

            errors = architecture_rules.verify_sync(root)

        self.assertTrue(any("Failed to parse Cargo.toml" in error for error in errors))

    def test_verify_sync_allows_workspace_member_reordering(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = Path(tmp_dir)
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
                                "crate": "api",
                                "path": "apps/api",
                                "may_depend_on": ["usecase"],
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
            (root / "Cargo.toml").write_text(
                "\n".join(
                    [
                        "[workspace]",
                        'members = ["apps/api", "libs/domain", "libs/usecase"]',
                    ]
                )
                + "\n",
                encoding="utf-8",
            )
            (root / "deny.toml").write_text(
                "\n".join(
                    [
                        "deny = [",
                        '  { crate = "usecase", wrappers = ["api"], reason = "usecase" },',
                        '  { crate = "domain", wrappers = ["usecase"], reason = "domain" },',
                        "]",
                    ]
                )
                + "\n",
                encoding="utf-8",
            )

            self.assertEqual(architecture_rules.verify_sync(root), [])

    def test_claude_workspace_map_paths_extract_workspace_members_from_tree(
        self,
    ) -> None:
        claude_text = "\n".join(
            [
                "## 7. Workspace Map",
                "",
                "```text",
                "Cargo.toml",
                "apps/",
                "├── api/",
                "│   └── src/",
                "└── server/",
                "libs/",
                "├── domain/",
                "├── usecase/",
                "└── infrastructure/",
                "track/",
                "└── items/<id>/",
                "```",
            ]
        )

        self.assertEqual(
            architecture_rules.claude_workspace_map_paths(claude_text),
            {
                "apps",
                "apps/api",
                "apps/api/src",
                "apps/server",
                "libs",
                "libs/domain",
                "libs/usecase",
                "libs/infrastructure",
                "track",
                "track/items/<id>",
            },
        )

    def test_verify_claude_workspace_map_accepts_matching_tree(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = Path(tmp_dir)
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
                                "crate": "api",
                                "path": "apps/api",
                                "may_depend_on": ["domain"],
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
            (root / "CLAUDE.md").write_text(
                "\n".join(
                    [
                        "## 7. Workspace Map",
                        "",
                        "```text",
                        "apps/",
                        "└── api/",
                        "libs/",
                        "└── domain/",
                        "```",
                    ]
                )
                + "\n",
                encoding="utf-8",
            )

            self.assertEqual(architecture_rules.verify_claude_workspace_map(root), [])

    def test_verify_claude_workspace_map_reports_missing_workspace_member(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = Path(tmp_dir)
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
                                "crate": "api",
                                "path": "apps/api",
                                "may_depend_on": ["domain"],
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
            (root / "CLAUDE.md").write_text(
                "\n".join(
                    [
                        "## 7. Workspace Map",
                        "",
                        "```text",
                        "libs/",
                        "└── domain/",
                        "```",
                    ]
                )
                + "\n",
                encoding="utf-8",
            )

            errors = architecture_rules.verify_claude_workspace_map(root)

        self.assertEqual(len(errors), 1)
        self.assertIn("Workspace Map is missing workspace members", errors[0])
        self.assertIn("apps/api", errors[0])

    def test_parse_deny_rules_accepts_reordered_fields(self) -> None:
        deny_text = "\n".join(
            [
                "deny = [",
                '  { reason = "domain", wrappers = ["usecase"], crate = "domain" },',
                "]",
            ]
        )

        self.assertEqual(
            architecture_rules.parse_deny_rules(deny_text),
            [{"crate": "domain", "wrappers": ["usecase"], "reason": "domain"}],
        )

    def test_parse_workspace_members_ignores_comments(self) -> None:
        cargo_text = "\n".join(
            [
                "[workspace]",
                "members = [",
                '  "libs/domain",',
                '  # "libs/old",',
                '  "apps/api",',
                "]",
            ]
        )

        self.assertEqual(
            architecture_rules.parse_workspace_members(cargo_text),
            ["libs/domain", "apps/api"],
        )

    def test_parse_deny_rules_accepts_braces_in_reason(self) -> None:
        deny_text = "\n".join(
            [
                "[bans]",
                "deny = [",
                '  { crate = "domain", wrappers = ["usecase"], reason = "Domain {core}" },',
                "]",
            ]
        )

        self.assertEqual(
            architecture_rules.parse_deny_rules(deny_text),
            [{"crate": "domain", "wrappers": ["usecase"], "reason": "Domain {core}"}],
        )

    def test_parse_workspace_members_reports_missing_tomllib_gracefully(self) -> None:
        with mock.patch.object(architecture_rules, "tomllib", None):
            with self.assertRaisesRegex(ValueError, "requires Python 3.11\\+"):
                architecture_rules.parse_workspace_members(
                    '[workspace]\nmembers = ["libs/domain"]\n'
                )

    def test_layer_rules_rejects_non_dict_entry(self) -> None:
        with self.assertRaisesRegex(ValueError, "each layer entry must be an object"):
            architecture_rules.layer_rules({"version": 1, "layers": ["bad"]})

    def test_layer_rules_rejects_duplicate_crate(self) -> None:
        rules = {
            "version": 1,
            "layers": [
                {
                    "crate": "domain",
                    "path": "libs/domain",
                    "may_depend_on": [],
                    "deny_reason": "domain",
                },
                {
                    "crate": "domain",
                    "path": "libs/other",
                    "may_depend_on": [],
                    "deny_reason": "duplicate",
                },
            ],
        }

        with self.assertRaisesRegex(ValueError, "duplicate crate"):
            architecture_rules.layer_rules(rules)

    def test_layer_rules_rejects_duplicate_path(self) -> None:
        rules = {
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
                    "path": "libs/domain",
                    "may_depend_on": ["domain"],
                    "deny_reason": "usecase",
                },
            ],
        }

        with self.assertRaisesRegex(ValueError, "duplicate path"):
            architecture_rules.layer_rules(rules)

    def test_layer_rules_rejects_unknown_dependency(self) -> None:
        rules = {
            "version": 1,
            "layers": [
                {
                    "crate": "usecase",
                    "path": "libs/usecase",
                    "may_depend_on": ["nonexistent"],
                    "deny_reason": "usecase",
                },
            ],
        }

        with self.assertRaisesRegex(ValueError, "unknown dependencies"):
            architecture_rules.layer_rules(rules)

    def test_layer_rules_rejects_self_dependency(self) -> None:
        rules = {
            "version": 1,
            "layers": [
                {
                    "crate": "domain",
                    "path": "libs/domain",
                    "may_depend_on": ["domain"],
                    "deny_reason": "domain",
                },
            ],
        }

        with self.assertRaisesRegex(ValueError, "cannot depend on itself"):
            architecture_rules.layer_rules(rules)

    def test_expected_deny_rules_rejects_empty_reason_for_referenced_layer(
        self,
    ) -> None:
        rules = {
            "version": 1,
            "layers": [
                {
                    "crate": "domain",
                    "path": "libs/domain",
                    "may_depend_on": [],
                    "deny_reason": "",
                },
                {
                    "crate": "usecase",
                    "path": "libs/usecase",
                    "may_depend_on": ["domain"],
                    "deny_reason": "usecase",
                },
            ],
        }

        with self.assertRaisesRegex(ValueError, "non-empty 'deny_reason'"):
            architecture_rules.expected_deny_rules(rules)

    def test_main_verify_sync_emits_error_output(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = Path(tmp_dir)
            docs_dir = root / "docs"
            docs_dir.mkdir(parents=True, exist_ok=True)
            (docs_dir / "architecture-rules.json").write_text(
                json.dumps({"version": 1, "layers": []}, ensure_ascii=False, indent=2)
                + "\n",
                encoding="utf-8",
            )
            (root / "Cargo.toml").write_text(
                "[workspace]\nmembers = []\n", encoding="utf-8"
            )
            (root / "deny.toml").write_text("deny = []\n", encoding="utf-8")

            stdout = io.StringIO()
            stderr = io.StringIO()
            with redirect_stdout(stdout), redirect_stderr(stderr):
                with mock.patch.object(
                    architecture_rules, "project_root", return_value=root
                ):
                    code = architecture_rules.main(
                        ["architecture_rules.py", "verify-sync"]
                    )

        self.assertEqual(code, 1)
        self.assertEqual(stdout.getvalue(), "")
        self.assertIn("Failed to load architecture rules", stderr.getvalue())


if __name__ == "__main__":
    unittest.main()
