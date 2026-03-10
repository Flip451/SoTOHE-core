import io
import tempfile
import unittest
from contextlib import redirect_stderr, redirect_stdout
from pathlib import Path
from unittest import mock

import scripts.takt_failure_report as takt_failure_report


class TaktFailureReportTest(unittest.TestCase):
    def test_output_excerpt_prefers_log_tail_for_long_output(self) -> None:
        output = "\n".join(f"line {index}" for index in range(1, 26))

        excerpt = takt_failure_report.output_excerpt(output, max_lines=5)

        self.assertIn("showing last 5 of 25 lines", excerpt)
        self.assertNotIn("line 1", excerpt)
        self.assertIn("line 21", excerpt)
        self.assertIn("line 25", excerpt)

    def test_primary_guidance_uses_post_test_analysis_for_takt_test_exec_commands(
        self,
    ) -> None:
        analyzer, summary, guidance = takt_failure_report.primary_guidance(
            "cargo make test-one-exec failing_test",
            "error[E0382]: use of moved value: `user`\n",
        )

        self.assertEqual(analyzer, "post-test-analysis")
        self.assertEqual(summary, "Rust compiler error (with error code)")
        self.assertIn(takt_failure_report.post_test_analysis.DEBUG_PREFIX, guidance)

    def test_primary_guidance_falls_back_to_error_to_codex_for_non_targeted_commands(
        self,
    ) -> None:
        analyzer, summary, guidance = takt_failure_report.primary_guidance(
            "./scripts/custom-rust-check.sh",
            "error[E0502]: cannot borrow `x` as mutable\n",
        )

        self.assertEqual(analyzer, "error-to-codex")
        self.assertTrue(summary.endswith("error pattern(s) detected"))
        self.assertIn(takt_failure_report.error_to_codex.ERROR_PREFIX, guidance)

    def test_main_writes_debug_report(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = Path(tmp_dir)
            output_file = root / ".takt" / "last-failure.log"
            output_file.parent.mkdir(parents=True, exist_ok=True)
            output_file.write_text(
                "error[E0382]: use of moved value\n", encoding="utf-8"
            )

            stdout = io.StringIO()
            with mock.patch.object(
                takt_failure_report, "project_root", return_value=root
            ):
                with redirect_stdout(stdout):
                    code = takt_failure_report.main(
                        [
                            "takt_failure_report.py",
                            "--command",
                            "cargo make test-one-exec failing_test",
                        ]
                    )

            self.assertEqual(code, 0)
            self.assertIn("Wrote debug report", stdout.getvalue())
            report = (root / ".takt" / "debug-report.md").read_text(encoding="utf-8")
            self.assertIn(
                "failing command: `cargo make test-one-exec failing_test`", report
            )
            self.assertIn("analyzer: `post-test-analysis`", report)
            self.assertIn("Hook-Derived Guidance", report)

    def test_main_rejects_missing_output_file(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = Path(tmp_dir)
            stderr = io.StringIO()

            with mock.patch.object(
                takt_failure_report, "project_root", return_value=root
            ):
                with redirect_stderr(stderr):
                    code = takt_failure_report.main(
                        [
                            "takt_failure_report.py",
                            "--command",
                            "cargo make test-one-exec failing_test",
                        ]
                    )

            self.assertEqual(code, 1)
            self.assertIn("Output file not found", stderr.getvalue())

    def test_main_accepts_cargo_make_separator(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = Path(tmp_dir)
            output_file = root / ".takt" / "last-failure.log"
            output_file.parent.mkdir(parents=True, exist_ok=True)
            output_file.write_text(
                "error[E0382]: use of moved value\n", encoding="utf-8"
            )

            with mock.patch.object(
                takt_failure_report, "project_root", return_value=root
            ):
                code = takt_failure_report.main(
                    [
                        "takt_failure_report.py",
                        "--",
                        "--command",
                        "cargo make test-one-exec failing_test",
                    ]
                )

            self.assertEqual(code, 0)
            self.assertTrue((root / ".takt" / "debug-report.md").exists())

    def test_main_rejects_paths_outside_project_root(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = Path(tmp_dir)
            output_file = root / ".takt" / "last-failure.log"
            output_file.parent.mkdir(parents=True, exist_ok=True)
            output_file.write_text(
                "error[E0382]: use of moved value\n", encoding="utf-8"
            )

            stderr = io.StringIO()
            with mock.patch.object(
                takt_failure_report, "project_root", return_value=root
            ):
                with redirect_stderr(stderr):
                    code = takt_failure_report.main(
                        [
                            "takt_failure_report.py",
                            "--command",
                            "cargo make test-one-exec failing_test",
                            "--report-file",
                            "../outside.md",
                        ]
                    )

            self.assertEqual(code, 1)
            self.assertIn("Path must stay within project root", stderr.getvalue())


if __name__ == "__main__":
    unittest.main()
