import json
import tempfile
import unittest
from pathlib import Path

import scripts.check_layers as check_layers


class CheckLayersTest(unittest.TestCase):
    def rules(self) -> dict:
        return {
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
                    "deny_reason": "api",
                },
                {
                    "crate": "server",
                    "path": "apps/server",
                    "may_depend_on": ["api"],
                    "deny_reason": "",
                },
                {
                    "crate": "infrastructure",
                    "path": "libs/infrastructure",
                    "may_depend_on": ["domain"],
                    "deny_reason": "infra",
                },
            ],
        }

    def metadata(
        self,
        graph: dict[str, set[str]],
        dev_graph: dict[str, set[str]] | None = None,
    ) -> dict:
        names = sorted(graph)
        workspace_members = [f"path+file:///repo/{name}#0.1.0" for name in names]
        packages = [
            {"id": member_id, "name": name}
            for name, member_id in zip(names, workspace_members, strict=True)
        ]
        id_by_name = {package["name"]: package["id"] for package in packages}
        nodes = []
        for name in names:
            normal_deps = sorted(graph[name])
            dev_deps = sorted(dev_graph.get(name, set())) if dev_graph else []
            deps = []
            for dep in normal_deps:
                deps.append(
                    {
                        "pkg": id_by_name[dep],
                        "dep_kinds": [{"kind": None, "target": None}],
                    }
                )
            for dep in dev_deps:
                deps.append(
                    {
                        "pkg": id_by_name[dep],
                        "dep_kinds": [{"kind": "dev", "target": None}],
                    }
                )
            nodes.append(
                {
                    "id": id_by_name[name],
                    "deps": deps,
                    "dependencies": [
                        id_by_name[d] for d in sorted(set(normal_deps) | set(dev_deps))
                    ],
                }
            )
        return {
            "packages": packages,
            "workspace_members": workspace_members,
            "resolve": {"nodes": nodes},
        }

    def test_validate_dependencies_allows_expected_transitive_paths(self) -> None:
        graph = {
            "domain": set(),
            "usecase": {"domain"},
            "api": {"usecase"},
            "server": {"api"},
            "infrastructure": {"domain"},
        }

        errors = check_layers.validate_dependencies(
            self.rules(), graph, mode="transitive"
        )

        self.assertEqual(errors, [])

    def test_validate_dependencies_detects_prohibited_transitive_path(self) -> None:
        graph = {
            "domain": set(),
            "usecase": {"domain"},
            "api": {"usecase", "infrastructure"},
            "server": {"api"},
            "infrastructure": {"domain"},
        }

        errors = check_layers.validate_dependencies(
            self.rules(), graph, mode="transitive"
        )

        self.assertIn(
            "api: prohibited direct dependency path api -> infrastructure", errors
        )
        self.assertIn(
            "server: prohibited transitive dependency path server -> api -> infrastructure",
            errors,
        )

    def test_validate_dependencies_transitive_mode_still_rejects_direct_skip_dependency(
        self,
    ) -> None:
        graph = {
            "domain": set(),
            "usecase": {"domain"},
            "api": {"usecase"},
            "server": {"api", "domain"},
            "infrastructure": {"domain"},
        }

        errors = check_layers.validate_dependencies(
            self.rules(), graph, mode="transitive"
        )

        self.assertIn(
            "server: prohibited direct dependency path server -> domain", errors
        )

    def test_workspace_graph_ignores_dev_dependencies(self) -> None:
        """Dev-dependencies should not appear in the layer graph."""
        graph = {
            "domain": set(),
            "usecase": {"domain"},
            "api": {"usecase"},
            "server": {"api"},
            "infrastructure": {"domain"},
        }
        dev_graph = {
            # api dev-depends on infrastructure (e.g. for integration tests)
            "api": {"infrastructure"},
        }
        metadata = self.metadata(graph, dev_graph=dev_graph)
        actual = check_layers.workspace_graph(metadata)

        # api should NOT have infrastructure as a dependency
        self.assertNotIn("infrastructure", actual.get("api", set()))
        # normal deps should still be present
        self.assertIn("usecase", actual.get("api", set()))

    def test_workspace_graph_dev_dep_does_not_cause_violation(self) -> None:
        """A dev-dependency that would violate layer rules must be ignored."""
        graph = {
            "domain": set(),
            "usecase": {"domain"},
            "api": {"usecase"},
            "server": {"api"},
            "infrastructure": {"domain"},
        }
        dev_graph = {"api": {"infrastructure"}}
        metadata = self.metadata(graph, dev_graph=dev_graph)
        actual = check_layers.workspace_graph(metadata)

        errors = check_layers.validate_dependencies(
            self.rules(), actual, mode="transitive"
        )
        self.assertEqual(errors, [])

    def test_workspace_graph_build_dep_excluded(self) -> None:
        """Build-dependencies should not appear in the layer graph."""
        names = ["domain", "usecase"]
        workspace_members = [f"path+file:///repo/{n}#0.1.0" for n in names]
        packages = [
            {"id": wid, "name": n}
            for n, wid in zip(names, workspace_members, strict=True)
        ]
        id_by_name = {p["name"]: p["id"] for p in packages}
        nodes = [
            {
                "id": id_by_name["usecase"],
                "deps": [
                    {
                        "pkg": id_by_name["domain"],
                        "dep_kinds": [{"kind": "build", "target": None}],
                    }
                ],
                "dependencies": [id_by_name["domain"]],
            },
            {"id": id_by_name["domain"], "deps": [], "dependencies": []},
        ]
        metadata = {
            "packages": packages,
            "workspace_members": workspace_members,
            "resolve": {"nodes": nodes},
        }
        actual = check_layers.workspace_graph(metadata)
        self.assertNotIn("domain", actual.get("usecase", set()))

    def test_workspace_graph_missing_dep_kinds_includes_dep(self) -> None:
        """When dep_kinds is missing, dependency should be included (safe default)."""
        names = ["domain", "usecase"]
        workspace_members = [f"path+file:///repo/{n}#0.1.0" for n in names]
        packages = [
            {"id": wid, "name": n}
            for n, wid in zip(names, workspace_members, strict=True)
        ]
        id_by_name = {p["name"]: p["id"] for p in packages}
        nodes = [
            {
                "id": id_by_name["usecase"],
                "deps": [{"pkg": id_by_name["domain"]}],  # no dep_kinds
                "dependencies": [id_by_name["domain"]],
            },
            {"id": id_by_name["domain"], "deps": [], "dependencies": []},
        ]
        metadata = {
            "packages": packages,
            "workspace_members": workspace_members,
            "resolve": {"nodes": nodes},
        }
        actual = check_layers.workspace_graph(metadata)
        self.assertIn("domain", actual.get("usecase", set()))

    def test_workspace_graph_fallback_to_flat_dependencies(self) -> None:
        """When 'deps' field is absent, fall back to flat 'dependencies' list."""
        names = ["domain", "usecase"]
        workspace_members = [f"path+file:///repo/{n}#0.1.0" for n in names]
        packages = [
            {"id": wid, "name": n}
            for n, wid in zip(names, workspace_members, strict=True)
        ]
        id_by_name = {p["name"]: p["id"] for p in packages}
        nodes = [
            {
                "id": id_by_name["usecase"],
                # No "deps" field at all
                "dependencies": [id_by_name["domain"]],
            },
            {"id": id_by_name["domain"], "dependencies": []},
        ]
        metadata = {
            "packages": packages,
            "workspace_members": workspace_members,
            "resolve": {"nodes": nodes},
        }
        actual = check_layers.workspace_graph(metadata)
        self.assertIn("domain", actual.get("usecase", set()))

    def test_main_accepts_metadata_file_for_testing(self) -> None:
        graph = {
            "domain": set(),
            "usecase": {"domain"},
            "api": {"usecase"},
            "server": {"api"},
            "infrastructure": {"domain"},
        }
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = Path(tmp_dir)
            docs_dir = root / "docs"
            docs_dir.mkdir(parents=True, exist_ok=True)
            (root / "architecture-rules.json").write_text(
                json.dumps(self.rules(), ensure_ascii=False, indent=2) + "\n",
                encoding="utf-8",
            )
            metadata_path = root / "metadata.json"
            metadata_path.write_text(
                json.dumps(self.metadata(graph)) + "\n", encoding="utf-8"
            )

            code = check_layers.main(
                [
                    "check_layers.py",
                    "--root",
                    str(root),
                    "--metadata-file",
                    str(metadata_path),
                    "--mode",
                    "transitive",
                ]
            )

        self.assertEqual(code, 0)


if __name__ == "__main__":
    unittest.main()
