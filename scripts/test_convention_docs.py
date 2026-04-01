import io
import tempfile
import unittest
from contextlib import redirect_stderr, redirect_stdout
from pathlib import Path
from unittest import mock

import scripts.convention_docs as convention_docs

README_TEMPLATE = """# Project Conventions

## Current Files

<!-- convention-docs:start -->
- No convention documents yet. Add one with `/conventions:add <name>`.
<!-- convention-docs:end -->
"""


class ConventionDocsTest(unittest.TestCase):
    def test_add_document_creates_file_and_updates_index(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = Path(tmp_dir)
            conventions = root / "knowledge" / "conventions"
            conventions.mkdir(parents=True, exist_ok=True)
            (conventions / "README.md").write_text(README_TEMPLATE, encoding="utf-8")

            with mock.patch.object(convention_docs, "project_root", return_value=root):
                stdout = io.StringIO()
                with redirect_stdout(stdout):
                    code = convention_docs.main(
                        [
                            "convention_docs.py",
                            "add",
                            "api-design",
                            "--summary",
                            "API の設計規約をまとめる。",
                        ]
                    )

            self.assertEqual(code, 0)
            created = conventions / "api-design.md"
            self.assertTrue(created.exists())
            content = created.read_text(encoding="utf-8")
            self.assertIn("# API Design", content)
            self.assertIn("## Scope", content)
            self.assertIn("## Examples", content)
            self.assertIn("## Exceptions", content)
            readme = (conventions / "README.md").read_text(encoding="utf-8")
            self.assertIn("`api-design.md`: API Design", readme)
            self.assertIn("[OK] Added convention document", stdout.getvalue())

    def test_add_document_rejects_duplicate(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = Path(tmp_dir)
            conventions = root / "knowledge" / "conventions"
            conventions.mkdir(parents=True, exist_ok=True)
            (conventions / "README.md").write_text(README_TEMPLATE, encoding="utf-8")
            (conventions / "api-design.md").write_text(
                "# API Design\n", encoding="utf-8"
            )

            with mock.patch.object(convention_docs, "project_root", return_value=root):
                stderr = io.StringIO()
                with redirect_stderr(stderr):
                    code = convention_docs.main(
                        ["convention_docs.py", "add", "api-design"]
                    )

            self.assertEqual(code, 1)
            self.assertIn("already exists", stderr.getvalue())

    def test_add_document_requires_slug_for_non_ascii_name(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = Path(tmp_dir)
            conventions = root / "knowledge" / "conventions"
            conventions.mkdir(parents=True, exist_ok=True)
            (conventions / "README.md").write_text(README_TEMPLATE, encoding="utf-8")

            with mock.patch.object(convention_docs, "project_root", return_value=root):
                stderr = io.StringIO()
                with redirect_stderr(stderr):
                    code = convention_docs.main(
                        ["convention_docs.py", "add", "計装方針"]
                    )

            self.assertEqual(code, 1)
            self.assertIn("require --slug", stderr.getvalue())

    def test_add_document_rejects_missing_readme_without_partial_write(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = Path(tmp_dir)
            conventions = root / "knowledge" / "conventions"
            conventions.mkdir(parents=True, exist_ok=True)

            with mock.patch.object(convention_docs, "project_root", return_value=root):
                stderr = io.StringIO()
                with redirect_stderr(stderr):
                    code = convention_docs.main(
                        ["convention_docs.py", "add", "api-design"]
                    )

            self.assertEqual(code, 1)
            self.assertIn("README index target is missing", stderr.getvalue())
            self.assertFalse((conventions / "api-design.md").exists())

    def test_add_document_rejects_missing_markers_without_partial_write(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = Path(tmp_dir)
            conventions = root / "knowledge" / "conventions"
            conventions.mkdir(parents=True, exist_ok=True)
            (conventions / "README.md").write_text(
                "# Project Conventions\n", encoding="utf-8"
            )

            with mock.patch.object(convention_docs, "project_root", return_value=root):
                stderr = io.StringIO()
                with redirect_stderr(stderr):
                    code = convention_docs.main(
                        ["convention_docs.py", "add", "api-design"]
                    )

            self.assertEqual(code, 1)
            self.assertIn("README index markers not found", stderr.getvalue())
            self.assertFalse((conventions / "api-design.md").exists())

    def test_add_document_accepts_non_ascii_name_with_slug(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = Path(tmp_dir)
            conventions = root / "knowledge" / "conventions"
            conventions.mkdir(parents=True, exist_ok=True)
            (conventions / "README.md").write_text(README_TEMPLATE, encoding="utf-8")

            with mock.patch.object(convention_docs, "project_root", return_value=root):
                stdout = io.StringIO()
                with redirect_stdout(stdout):
                    code = convention_docs.main(
                        [
                            "convention_docs.py",
                            "add",
                            "計装方針",
                            "--slug",
                            "instrumentation-policy",
                        ]
                    )

            self.assertEqual(code, 0)
            created = conventions / "instrumentation-policy.md"
            self.assertTrue(created.exists())
            self.assertIn("# 計装方針", created.read_text(encoding="utf-8"))

    def test_add_document_accepts_cargo_make_separator(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = Path(tmp_dir)
            conventions = root / "knowledge" / "conventions"
            conventions.mkdir(parents=True, exist_ok=True)
            (conventions / "README.md").write_text(README_TEMPLATE, encoding="utf-8")

            with mock.patch.object(convention_docs, "project_root", return_value=root):
                code = convention_docs.main(
                    ["convention_docs.py", "add", "--", "api-design"]
                )

            self.assertEqual(code, 0)
            self.assertTrue((conventions / "api-design.md").exists())

    def test_add_document_rejects_non_kebab_case_slug(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = Path(tmp_dir)
            conventions = root / "knowledge" / "conventions"
            conventions.mkdir(parents=True, exist_ok=True)
            (conventions / "README.md").write_text(README_TEMPLATE, encoding="utf-8")

            with mock.patch.object(convention_docs, "project_root", return_value=root):
                stderr = io.StringIO()
                with redirect_stderr(stderr):
                    code = convention_docs.main(
                        [
                            "convention_docs.py",
                            "add",
                            "計装方針",
                            "--slug",
                            "Instrumentation Policy",
                        ]
                    )

            self.assertEqual(code, 1)
            self.assertIn("kebab-case ASCII", stderr.getvalue())

    def test_verify_index_detects_out_of_sync_readme(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = Path(tmp_dir)
            conventions = root / "knowledge" / "conventions"
            conventions.mkdir(parents=True, exist_ok=True)
            (conventions / "README.md").write_text(
                README_TEMPLATE.replace(
                    "- No convention documents yet. Add one with `/conventions:add <name>`.",
                    "- `wrong.md`: Wrong",
                ),
                encoding="utf-8",
            )
            (conventions / "api-design.md").write_text(
                "# API Design\n", encoding="utf-8"
            )

            with mock.patch.object(convention_docs, "project_root", return_value=root):
                stderr = io.StringIO()
                with redirect_stderr(stderr):
                    code = convention_docs.main(["convention_docs.py", "verify-index"])

            self.assertEqual(code, 1)
            self.assertIn("out of sync", stderr.getvalue())
            self.assertIn("update-index", stderr.getvalue())

    def test_update_index_repairs_out_of_sync_readme(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = Path(tmp_dir)
            conventions = root / "knowledge" / "conventions"
            conventions.mkdir(parents=True, exist_ok=True)
            (conventions / "README.md").write_text(
                README_TEMPLATE.replace(
                    "- No convention documents yet. Add one with `/conventions:add <name>`.",
                    "- `wrong.md`: Wrong",
                ),
                encoding="utf-8",
            )
            (conventions / "api-design.md").write_text(
                "# API Design\n", encoding="utf-8"
            )

            with mock.patch.object(convention_docs, "project_root", return_value=root):
                stdout = io.StringIO()
                with redirect_stdout(stdout):
                    code = convention_docs.main(["convention_docs.py", "update-index"])

            self.assertEqual(code, 0)
            self.assertIn("Updated convention README index", stdout.getvalue())
            readme = (conventions / "README.md").read_text(encoding="utf-8")
            self.assertIn("`api-design.md`: API Design", readme)

    def test_update_index_rejects_missing_readme(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = Path(tmp_dir)
            (root / "knowledge" / "conventions").mkdir(parents=True, exist_ok=True)

            with mock.patch.object(convention_docs, "project_root", return_value=root):
                stderr = io.StringIO()
                with redirect_stderr(stderr):
                    code = convention_docs.main(["convention_docs.py", "update-index"])

            self.assertEqual(code, 1)
            self.assertIn("README index target is missing", stderr.getvalue())

    def test_verify_index_rejects_missing_readme(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = Path(tmp_dir)
            (root / "knowledge" / "conventions").mkdir(parents=True, exist_ok=True)

            with mock.patch.object(convention_docs, "project_root", return_value=root):
                stderr = io.StringIO()
                with redirect_stderr(stderr):
                    code = convention_docs.main(["convention_docs.py", "verify-index"])

            self.assertEqual(code, 1)
            self.assertIn("README index target is missing", stderr.getvalue())

    def test_verify_index_accepts_synced_readme(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = Path(tmp_dir)
            conventions = root / "knowledge" / "conventions"
            conventions.mkdir(parents=True, exist_ok=True)
            (conventions / "README.md").write_text(README_TEMPLATE, encoding="utf-8")
            (conventions / "api-design.md").write_text(
                "# API Design\n", encoding="utf-8"
            )

            with mock.patch.object(convention_docs, "project_root", return_value=root):
                convention_docs.update_readme_index()
                code = convention_docs.main(["convention_docs.py", "verify-index"])

            self.assertEqual(code, 0)

    def test_update_readme_index_uses_recommended_order(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = Path(tmp_dir)
            conventions = root / "knowledge" / "conventions"
            conventions.mkdir(parents=True, exist_ok=True)
            (conventions / "README.md").write_text(README_TEMPLATE, encoding="utf-8")
            (conventions / "testing.md").write_text("# Testing\n", encoding="utf-8")
            (conventions / "api-design.md").write_text(
                "# API Design\n", encoding="utf-8"
            )
            (conventions / "architecture.md").write_text(
                "# Architecture\n", encoding="utf-8"
            )
            (conventions / "zzz-custom.md").write_text(
                "# Zzz Custom\n", encoding="utf-8"
            )

            with mock.patch.object(convention_docs, "project_root", return_value=root):
                convention_docs.update_readme_index()

            readme = (conventions / "README.md").read_text(encoding="utf-8")
            architecture_pos = readme.index("`architecture.md`")
            api_design_pos = readme.index("`api-design.md`")
            testing_pos = readme.index("`testing.md`")
            custom_pos = readme.index("`zzz-custom.md`")
            self.assertLess(architecture_pos, api_design_pos)
            self.assertLess(api_design_pos, testing_pos)
            self.assertLess(testing_pos, custom_pos)


if __name__ == "__main__":
    unittest.main()
