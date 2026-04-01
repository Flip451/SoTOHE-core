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
            (root / "architecture-rules.json").write_text(
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
            (root / "architecture-rules.json").write_text(
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
            (root / "architecture-rules.json").write_text(
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
            (root / "architecture-rules.json").write_text(
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
            (root / "architecture-rules.json").write_text(
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
            (root / "architecture-rules.json").write_text(
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

    def test_extra_dirs_accepts_optional_entries(self) -> None:
        rules = {
            "version": 1,
            "layers": [
                {
                    "crate": "domain",
                    "path": "libs/domain",
                    "may_depend_on": [],
                    "deny_reason": "domain",
                }
            ],
            "extra_dirs": [
                {"path": "track", "label": "workflow state"},
                {"path": "track/items/<id>"},
            ],
        }

        self.assertEqual(
            architecture_rules.extra_dirs(rules),
            [
                {"path": "track", "label": "workflow state"},
                {"path": "track/items/<id>", "label": ""},
            ],
        )

    def test_extra_dirs_rejects_duplicate_layer_path(self) -> None:
        rules = {
            "version": 1,
            "layers": [
                {
                    "crate": "cli",
                    "path": "apps/cli",
                    "may_depend_on": [],
                    "deny_reason": "",
                }
            ],
            "extra_dirs": [{"path": "apps/cli", "label": "duplicate"}],
        }

        with self.assertRaisesRegex(ValueError, "duplicates layer path"):
            architecture_rules.extra_dirs(rules)

    def test_render_workspace_tree_renders_crates_only_by_default(self) -> None:
        rules = {
            "version": 1,
            "layers": [
                {
                    "crate": "domain",
                    "path": "libs/domain",
                    "may_depend_on": [],
                    "deny_reason": "domain layer",
                },
                {
                    "crate": "api",
                    "path": "apps/api",
                    "may_depend_on": ["domain"],
                    "deny_reason": "",
                },
            ],
        }

        rendered = architecture_rules.render_workspace_tree(
            rules, include_extra_dirs=False
        )

        self.assertIn("Cargo.toml", rendered)
        self.assertIn("apps/", rendered)
        self.assertIn("└── api/", rendered)
        self.assertIn("libs/", rendered)
        self.assertIn("# domain crate", rendered)
        self.assertIn("# api crate", rendered)
        self.assertNotIn("track/", rendered)

    def test_render_workspace_tree_full_includes_extra_dirs(self) -> None:
        rules = {
            "version": 1,
            "layers": [
                {
                    "crate": "domain",
                    "path": "libs/domain",
                    "may_depend_on": [],
                    "deny_reason": "domain layer",
                }
            ],
            "extra_dirs": [
                {"path": "knowledge/conventions", "label": "project rules"},
                {"path": "track/items/<id>", "label": "active track"},
            ],
        }

        rendered = architecture_rules.render_workspace_tree(
            rules, include_extra_dirs=True
        )

        self.assertIn("knowledge/", rendered)
        self.assertIn("└── conventions/", rendered)
        self.assertIn("# domain crate", rendered)
        self.assertIn("# project rules", rendered)
        self.assertIn("track/", rendered)
        self.assertIn("└── items/", rendered)
        self.assertIn("└── <id>/", rendered)

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
            (root / "architecture-rules.json").write_text(
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
