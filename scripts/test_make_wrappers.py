import json
import os
import subprocess
import sys
import tempfile
import textwrap
import unittest
from pathlib import Path

PROJECT_ROOT = Path(__file__).resolve().parent.parent


GUIDES_STUB = textwrap.dedent(
    """\
    import json
    import sys

    print(
        json.dumps(
            {
                "script": "external_guides.py",
                "argv": sys.argv[1:],
                "python": sys.executable,
            }
        )
    )
    """
)


CONVENTIONS_STUB = textwrap.dedent(
    """\
    import json
    import sys

    print(
        json.dumps(
            {
                "script": "convention_docs.py",
                "argv": sys.argv[1:],
                "python": sys.executable,
            }
        )
    )
    """
)


ARCH_RULES_STUB = textwrap.dedent(
    """\
    import json
    import sys

    print(
        json.dumps(
            {
                "script": "architecture_rules.py",
                "argv": sys.argv[1:],
                "python": sys.executable,
            }
        )
    )
    """
)



VERIFY_ORCHESTRA_STUB = textwrap.dedent(
    """\
    import json
    import os
    import sys

    print(
        json.dumps(
            {
                "script": "verify_orchestra_guardrails.py",
                "argv": sys.argv[1:],
                "python": sys.executable,
                "wrapper_marker": os.environ.get("VERIFY_ORCHESTRA_WRAPPER_MARKER"),
            }
        )
    )
    """
)

PASSING_TEST = textwrap.dedent(
    """\
    import unittest


    class SmokeTest(unittest.TestCase):
        def test_ok(self) -> None:
            self.assertTrue(True)


    if __name__ == "__main__":
        unittest.main()
    """
)


class MakeWrappersTest(unittest.TestCase):
    def write_text(self, path: Path, content: str) -> None:
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(content, encoding="utf-8")

    def make_fixture(self, root: Path) -> None:
        self.write_text(
            root / "Makefile.toml",
            (PROJECT_ROOT / "Makefile.toml").read_text(encoding="utf-8"),
        )
        self.write_text(root / "scripts" / "external_guides.py", GUIDES_STUB)
        self.write_text(root / "scripts" / "convention_docs.py", CONVENTIONS_STUB)
        self.write_text(root / "scripts" / "architecture_rules.py", ARCH_RULES_STUB)
        self.write_text(
            root / "scripts" / "verify_orchestra_guardrails.py", VERIFY_ORCHESTRA_STUB
        )
        for test_name in (
            "test_architecture_rules.py",
            "test_verify_scripts.py",
            "test_convention_docs.py",
            "test_external_guides.py",
            "test_make_wrappers.py",
        ):
            self.write_text(root / "scripts" / test_name, PASSING_TEST)

        self.write_text(
            root / ".claude" / "hooks" / "test_wrapper_hook.py", PASSING_TEST
        )
        self.make_python3_stub(root)
        self.make_pytest_stub(root)

    def make_python3_stub(self, root: Path) -> Path:
        python3_path = root / "bin" / "python3"
        self.write_text(
            python3_path,
            textwrap.dedent(
                f"""\
                #!/usr/bin/env bash
                exec "{sys.executable}" "$@"
                """
            ),
        )
        os.chmod(python3_path, 0o755)
        return python3_path

    def make_pytest_stub(self, root: Path) -> Path:
        pytest_path = root / "bin" / "pytest"
        self.write_text(
            pytest_path,
            textwrap.dedent(
                """\
                #!/usr/bin/env bash
                echo "pytest stub: $*"
                exit 0
                """
            ),
        )
        os.chmod(pytest_path, 0o755)
        return pytest_path

    def make_docker_stub(self, root: Path) -> Path:
        docker_path = root / "bin" / "docker"
        self.write_text(
            docker_path,
            textwrap.dedent(
                f"""\
                #!{sys.executable}
                import json
                import os
                import sys

                print(
                    json.dumps(
                        {{
                            "script": "docker",
                            "argv": sys.argv[1:],
                            "docker_buildkit": os.environ.get("DOCKER_BUILDKIT"),
                            "compose_docker_cli_build": os.environ.get("COMPOSE_DOCKER_CLI_BUILD"),
                        }}
                    )
                )
                """
            ),
        )
        os.chmod(docker_path, 0o755)
        return docker_path

    def run_make(
        self,
        root: Path,
        task: str,
        *args: str,
        env_updates: dict[str, str] | None = None,
        allow_private: bool = False,
    ) -> subprocess.CompletedProcess[str]:
        env = {**os.environ, **(env_updates or {})}
        env["PATH"] = str(root / "bin") + os.pathsep + env["PATH"]
        command = ["cargo", "make"]
        if allow_private:
            command.append("--allow-private")
        command.extend([task, *args])
        return subprocess.run(
            command,
            cwd=root,
            env=env,
            text=True,
            capture_output=True,
            check=False,
        )

    def parse_json_line(self, output: str) -> dict[str, object]:
        for line in output.splitlines():
            stripped = line.strip()
            if stripped.startswith("{") and stripped.endswith("}"):
                return json.loads(stripped)
        self.fail(f"no JSON payload found in output:\n{output}")

    def test_verify_orchestra_local_uses_rust_cli(self) -> None:
        makefile = (PROJECT_ROOT / "Makefile.toml").read_text(encoding="utf-8")
        task_header = "[tasks.verify-orchestra-local]"
        task_start = makefile.index(task_header)
        next_task = makefile.find("\n[tasks.", task_start + len(task_header))
        task_body = (
            makefile[task_start:] if next_task == -1 else makefile[task_start:next_task]
        )

        self.assertIn('script_runner = "@shell"', task_body)
        self.assertIn(
            "script = ['cargo run --quiet -p cli -- verify orchestra']",
            task_body,
        )
        self.assertNotIn("verify_orchestra_guardrails.py", task_body)

    def test_track_transition_wrapper_delegates_to_sotp_make(self) -> None:
        makefile = (PROJECT_ROOT / "Makefile.toml").read_text(encoding="utf-8")
        task_header = "[tasks.track-transition]"
        task_start = makefile.index(task_header)
        next_task = makefile.find("\n[tasks.", task_start + len(task_header))
        task_body = (
            makefile[task_start:] if next_task == -1 else makefile[task_start:next_task]
        )

        self.assertIn('command = "bin/sotp"', task_body)
        self.assertIn('"make"', task_body)
        self.assertIn('"track-transition"', task_body)
        self.assertIn('"${@}"', task_body)
        self.assertNotIn('script_runner', task_body)

    def test_track_state_ops_wrappers_delegate_to_sotp_make(self) -> None:
        makefile = (PROJECT_ROOT / "Makefile.toml").read_text(encoding="utf-8")

        # command + args format (single-word positional args only)
        for task_header, task_name in (
            ("[tasks.track-next-task]", "track-next-task"),
            ("[tasks.track-task-counts]", "track-task-counts"),
        ):
            with self.subTest(task=task_header):
                task_start = makefile.index(task_header)
                next_task = makefile.find("\n[tasks.", task_start + len(task_header))
                task_body = (
                    makefile[task_start:] if next_task == -1 else makefile[task_start:next_task]
                )
                self.assertIn('command = "bin/sotp"', task_body, f"{task_header} missing command")
                self.assertIn(f'"{task_name}"', task_body, f"{task_header} missing task name in args")
                self.assertIn('"make"', task_body, f"{task_header} missing 'make' in args")
                self.assertIn('"${@}"', task_body, f"{task_header} missing arg forwarding")
                self.assertNotIn('script_runner', task_body, f"{task_header} should not use script_runner")

        # Shell-based wrappers (multi-word positional args need "$@" quoting)
        for task_header, task_name in (
            ("[tasks.track-add-task]", "track-add-task"),
            ("[tasks.track-set-override]", "track-set-override"),
            ("[tasks.spec-approve]", "spec-approve"),
        ):
            with self.subTest(task=task_header):
                task_start = makefile.index(task_header)
                next_task = makefile.find("\n[tasks.", task_start + len(task_header))
                task_body = (
                    makefile[task_start:] if next_task == -1 else makefile[task_start:next_task]
                )
                self.assertIn('script_runner = "@shell"', task_body, f"{task_header} missing script_runner")
                self.assertIn(f'bin/sotp make {task_name}', task_body, f"{task_header} missing sotp make call")
                self.assertIn('"$@"', task_body, f"{task_header} missing shell $@ forwarding")

        # Direct bin/sotp subcommand wrappers (not via sotp make)
        for task_header, expected_call in (
            ("[tasks.track-signals]", "bin/sotp track signals"),
        ):
            with self.subTest(task=task_header):
                task_start = makefile.index(task_header)
                next_task = makefile.find("\n[tasks.", task_start + len(task_header))
                task_body = (
                    makefile[task_start:] if next_task == -1 else makefile[task_start:next_task]
                )
                self.assertIn('script_runner = "@shell"', task_body, f"{task_header} missing script_runner")
                self.assertIn(expected_call, task_body, f"{task_header} missing {expected_call}")
                self.assertIn('"$@"', task_body, f"{task_header} missing shell $@ forwarding")

    def test_track_branch_ops_delegate_to_sotp_make(self) -> None:
        makefile = (PROJECT_ROOT / "Makefile.toml").read_text(encoding="utf-8")

        for task_header, task_name in (
            ("[tasks.track-branch-create]", "track-branch-create"),
            ("[tasks.track-branch-switch]", "track-branch-switch"),
            ("[tasks.track-activate]", "track-activate"),
            ("[tasks.track-plan-branch]", "track-plan-branch"),
            ("[tasks.track-resolve]", "track-resolve"),
        ):
            with self.subTest(task=task_header):
                task_start = makefile.index(task_header)
                next_task = makefile.find("\n[tasks.", task_start + len(task_header))
                task_body = (
                    makefile[task_start:] if next_task == -1 else makefile[task_start:next_task]
                )
                self.assertIn('command = "bin/sotp"', task_body, f"{task_header} missing command")
                self.assertIn(f'"{task_name}"', task_body, f"{task_header} missing task name")
                self.assertIn('"make"', task_body, f"{task_header} missing 'make' in args")
                self.assertIn('"${@}"', task_body, f"{task_header} missing arg forwarding")
                self.assertNotIn('script_runner', task_body, f"{task_header} should not use script_runner")

    def test_track_sync_views_delegates_to_sotp_make(self) -> None:
        makefile = (PROJECT_ROOT / "Makefile.toml").read_text(encoding="utf-8")
        task_header = "[tasks.track-sync-views]"
        task_start = makefile.index(task_header)
        next_task = makefile.find("\n[tasks.", task_start + len(task_header))
        task_body = (
            makefile[task_start:] if next_task == -1 else makefile[task_start:next_task]
        )
        self.assertIn('command = "bin/sotp"', task_body)
        self.assertIn('"make"', task_body)
        self.assertIn('"track-sync-views"', task_body)
        self.assertIn('"${@}"', task_body)
        self.assertNotIn('script_runner', task_body)

    def test_track_git_wrappers_delegate_to_sotp_make(self) -> None:
        makefile = (PROJECT_ROOT / "Makefile.toml").read_text(encoding="utf-8")

        for task_header, task_name in (
            ("[tasks.track-switch-main]", "track-switch-main"),
            ("[tasks.track-add-paths]", "track-add-paths"),
            ("[tasks.track-commit-message]", "track-commit-message"),
            ("[tasks.track-note]", "track-note"),
        ):
            with self.subTest(task=task_header):
                task_start = makefile.index(task_header)
                next_task = makefile.find("\n[tasks.", task_start + len(task_header))
                task_body = (
                    makefile[task_start:]
                    if next_task == -1
                    else makefile[task_start:next_task]
                )
                self.assertIn('command = "bin/sotp"', task_body, f"{task_header} missing command")
                self.assertIn(f'"{task_name}"', task_body, f"{task_header} missing task name")
                self.assertIn('"make"', task_body, f"{task_header} missing 'make' in args")
                self.assertNotIn('script_runner', task_body, f"{task_header} should not use script_runner")

    def test_track_pr_wrappers_delegate_to_sotp_make(self) -> None:
        makefile = (PROJECT_ROOT / "Makefile.toml").read_text(encoding="utf-8")

        for task_header, task_name in (
            ("[tasks.track-pr-push]", "track-pr-push"),
            ("[tasks.track-pr-ensure]", "track-pr-ensure"),
            ("[tasks.track-pr]", "track-pr"),
            ("[tasks.track-pr-review]", "track-pr-review"),
            ("[tasks.track-pr-merge]", "track-pr-merge"),
            ("[tasks.track-pr-status]", "track-pr-status"),
        ):
            with self.subTest(task=task_header):
                task_start = makefile.index(task_header)
                next_task = makefile.find("\n[tasks.", task_start + len(task_header))
                task_body = (
                    makefile[task_start:]
                    if next_task == -1
                    else makefile[task_start:next_task]
                )
                self.assertIn('command = "bin/sotp"', task_body, f"{task_header} missing command")
                self.assertIn(f'"{task_name}"', task_body, f"{task_header} missing task name")
                self.assertIn('"make"', task_body, f"{task_header} missing 'make' in args")
                self.assertNotIn('script_runner', task_body, f"{task_header} should not use script_runner")

    def test_track_local_plan_wrapper_delegates_to_sotp_make(self) -> None:
        makefile = (PROJECT_ROOT / "Makefile.toml").read_text(encoding="utf-8")
        task_header = "[tasks.track-local-plan]"
        task_start = makefile.index(task_header)
        next_task = makefile.find("\n[tasks.", task_start + len(task_header))
        task_body = (
            makefile[task_start:] if next_task == -1 else makefile[task_start:next_task]
        )

        self.assertIn('script_runner = "@shell"', task_body)
        self.assertIn('bin/sotp make track-local-plan', task_body)
        self.assertIn('"$@"', task_body)

    def test_track_local_review_wrapper_delegates_to_sotp_make(self) -> None:
        makefile = (PROJECT_ROOT / "Makefile.toml").read_text(encoding="utf-8")
        task_header = "[tasks.track-local-review]"
        task_start = makefile.index(task_header)
        next_task = makefile.find("\n[tasks.", task_start + len(task_header))
        task_body = (
            makefile[task_start:] if next_task == -1 else makefile[task_start:next_task]
        )

        self.assertIn('script_runner = "@shell"', task_body)
        self.assertIn('bin/sotp make track-local-review', task_body)
        self.assertIn('"$@"', task_body)

    def test_track_review_status_wrapper_delegates_to_sotp_make(self) -> None:
        makefile = (PROJECT_ROOT / "Makefile.toml").read_text(encoding="utf-8")
        task_header = "[tasks.track-review-status]"
        task_start = makefile.index(task_header)
        next_task = makefile.find("\n[tasks.", task_start + len(task_header))
        task_body = (
            makefile[task_start:] if next_task == -1 else makefile[task_start:next_task]
        )

        self.assertIn('script_runner = "@shell"', task_body)
        self.assertIn('bin/sotp make track-review-status', task_body)
        self.assertIn('"$@"', task_body)

    def test_track_set_commit_hash_wrapper_delegates_to_sotp_make(self) -> None:
        makefile = (PROJECT_ROOT / "Makefile.toml").read_text(encoding="utf-8")
        task_header = "[tasks.track-set-commit-hash]"
        task_start = makefile.index(task_header)
        next_task = makefile.find("\n[tasks.", task_start + len(task_header))
        task_body = (
            makefile[task_start:] if next_task == -1 else makefile[task_start:next_task]
        )

        self.assertIn('script_runner = "@shell"', task_body)
        self.assertIn('bin/sotp make track-set-commit-hash', task_body)
        self.assertIn('"$@"', task_body)

    def test_guides_wrappers_smoke(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = Path(tmp_dir)
            self.make_fixture(root)

            cases = (
                ("guides-list", (), ["list"]),
                ("guides-fetch", ("demo-guide",), ["fetch", "demo-guide"]),
                ("guides-usage", (), ["usage"]),
                ("guides-setup", (), ["setup"]),
                (
                    "guides-add",
                    (
                        "--",
                        "--id",
                        "demo-guide",
                        "--title",
                        "Demo Guide",
                        "--source-url",
                        "https://example.com/demo.md",
                        "--license",
                        "MIT",
                    ),
                    [
                        "add",
                        "--",
                        "--id",
                        "demo-guide",
                        "--title",
                        "Demo Guide",
                        "--source-url",
                        "https://example.com/demo.md",
                        "--license",
                        "MIT",
                    ],
                ),
            )
            for task, args, expected in cases:
                result = self.run_make(root, task, *args)
                self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
                payload = self.parse_json_line(result.stdout)
                self.assertEqual(payload["script"], "external_guides.py")
                self.assertEqual(payload["argv"], expected)
                self.assertIn("python", str(payload["python"]))

    def test_conventions_wrappers_smoke(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = Path(tmp_dir)
            self.make_fixture(root)

            cases = (
                (
                    "conventions-add",
                    ("api-design",),
                    ["add", "api-design"],
                ),
                ("conventions-update-index", (), ["update-index"]),
                ("conventions-verify-index", (), ["verify-index"]),
            )
            for task, args, expected in cases:
                result = self.run_make(root, task, *args)
                self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
                payload = self.parse_json_line(result.stdout)
                self.assertEqual(payload["script"], "convention_docs.py")
                self.assertEqual(payload["argv"], expected)
                self.assertIn("python", str(payload["python"]))

    def test_architecture_rules_wrappers_smoke(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = Path(tmp_dir)
            self.make_fixture(root)

            cases = (
                ("architecture-rules-workspace-members", ["workspace-members"]),
                ("workspace-tree", ["workspace-tree"]),
                ("workspace-tree-full", ["workspace-tree-full"]),
                ("architecture-rules-direct-checks", ["direct-checks"]),
                ("architecture-rules-verify-sync", ["verify-sync"]),
            )
            for task, expected in cases:
                result = self.run_make(root, task)
                self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
                payload = self.parse_json_line(result.stdout)
                self.assertEqual(payload["script"], "architecture_rules.py")
                self.assertEqual(payload["argv"], expected)
                self.assertIn("python", str(payload["python"]))

    def test_selftest_wrappers_smoke(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = Path(tmp_dir)
            self.make_fixture(root)

            # make_fixture replaces every referenced selftest target with PASSING_TEST,
            # so this smoke test validates wrapper wiring without executing production tests.
            for task in (
                "guides-selftest-local",
                "scripts-selftest-local",
            ):
                result = self.run_make(root, task, allow_private=True)
                self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
                combined_output = result.stdout + result.stderr
                self.assertIn("pytest stub:", combined_output)

    def test_git_ops_wrapper_tasks_are_exact_and_file_backed(self) -> None:
        makefile = (PROJECT_ROOT / "Makefile.toml").read_text(encoding="utf-8")

        # add-all: migrated to sotp make
        task_header = "[tasks.add-all]"
        task_start = makefile.index(task_header)
        next_task = makefile.find("\n[tasks.", task_start + len(task_header))
        task_body = (
            makefile[task_start:] if next_task == -1 else makefile[task_start:next_task]
        )
        self.assertIn('command = "bin/sotp"', task_body)
        self.assertIn('"make"', task_body)
        self.assertIn('"add-all"', task_body)
        self.assertNotIn('script_runner', task_body)

        # track-add-paths: migrated to sotp make
        task_header = "[tasks.track-add-paths]"
        task_start = makefile.index(task_header)
        next_task = makefile.find("\n[tasks.", task_start + len(task_header))
        task_body = (
            makefile[task_start:] if next_task == -1 else makefile[task_start:next_task]
        )
        self.assertIn('command = "bin/sotp"', task_body)
        self.assertIn('"make"', task_body)
        self.assertIn('"track-add-paths"', task_body)
        self.assertNotIn('script_runner', task_body)

        # track-commit-message: migrated to sotp make (CI + commit logic handled in Rust)
        task_header = "[tasks.track-commit-message]"
        task_start = makefile.index(task_header)
        next_task = makefile.find("\n[tasks.", task_start + len(task_header))
        task_body = (
            makefile[task_start:] if next_task == -1 else makefile[task_start:next_task]
        )
        self.assertIn('command = "bin/sotp"', task_body)
        self.assertIn('"make"', task_body)
        self.assertIn('"track-commit-message"', task_body)
        self.assertNotIn('script_runner', task_body)

        # track-note: migrated to sotp make
        task_header = "[tasks.track-note]"
        task_start = makefile.index(task_header)
        next_task = makefile.find("\n[tasks.", task_start + len(task_header))
        task_body = (
            makefile[task_start:] if next_task == -1 else makefile[task_start:next_task]
        )
        self.assertIn('command = "bin/sotp"', task_body)
        self.assertIn('"make"', task_body)
        self.assertIn('"track-note"', task_body)
        self.assertNotIn('script_runner', task_body)

    def test_ci_container_tasks_exist_and_are_public(self) -> None:
        makefile = (PROJECT_ROOT / "Makefile.toml").read_text(encoding="utf-8")

        def extract_task_body(task_name: str) -> str:
            header = f"[tasks.{task_name}]"
            start = makefile.index(header)
            next_task = makefile.find("\n[tasks.", start + len(header))
            return makefile[start:] if next_task == -1 else makefile[start:next_task]

        # ci-container should exist and NOT be private
        ci_container_body = extract_task_body("ci-container")
        self.assertNotIn("private = true", ci_container_body)
        self.assertIn(
            '[gate] Run CI checks inside a pre-existing container (no docker compose)',
            ci_container_body,
        )

        # ci-rust-container should exist and NOT be private
        ci_rust_container_body = extract_task_body("ci-rust-container")
        self.assertNotIn("private = true", ci_rust_container_body)
        self.assertIn(
            '[gate] Run Rust-only CI checks inside a pre-existing container (no docker compose)',
            ci_rust_container_body,
        )

        # ci-container should have the same dependencies as ci-local
        ci_local_body = extract_task_body("ci-local")
        self.assertIn("private = true", ci_local_body)
        # Extract dependencies line from ci-local
        for line in ci_local_body.splitlines():
            if line.strip().startswith("dependencies"):
                ci_local_deps = line.strip()
                break
        for line in ci_container_body.splitlines():
            if line.strip().startswith("dependencies"):
                ci_container_deps = line.strip()
                break
        self.assertEqual(ci_local_deps, ci_container_deps)

        # ci-rust-container should have the same dependencies as ci-rust-local
        ci_rust_local_body = extract_task_body("ci-rust-local")
        self.assertIn("private = true", ci_rust_local_body)
        for line in ci_rust_local_body.splitlines():
            if line.strip().startswith("dependencies"):
                ci_rust_local_deps = line.strip()
                break
        for line in ci_rust_container_body.splitlines():
            if line.strip().startswith("dependencies"):
                ci_rust_container_deps = line.strip()
                break
        self.assertEqual(ci_rust_local_deps, ci_rust_container_deps)

    def test_docker_wrappers_smoke(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = Path(tmp_dir)
            self.make_fixture(root)
            self.make_docker_stub(root)

            cases = (
                (
                    "guides-selftest",
                    (),
                    [
                        "compose",
                        "run",
                        "--rm",
                        "tools",
                        "cargo",
                        "make",
                        "--allow-private",
                        "guides-selftest-local",
                    ],
                ),
                (
                    "scripts-selftest",
                    (),
                    [
                        "compose",
                        "run",
                        "--rm",
                        "tools",
                        "cargo",
                        "make",
                        "--allow-private",
                        "scripts-selftest-local",
                    ],
                ),
                ("build-tools", (), ["compose", "build", "tools"]),
                (
                    "build-dev",
                    (),
                    [
                        "compose",
                        "-f",
                        "compose.yml",
                        "-f",
                        "compose.dev.yml",
                        "build",
                        "app",
                    ],
                ),
                (
                    "up",
                    (),
                    [
                        "compose",
                        "-f",
                        "compose.yml",
                        "-f",
                        "compose.dev.yml",
                        "up",
                        "-d",
                        "app",
                    ],
                ),
                (
                    "down",
                    (),
                    ["compose", "-f", "compose.yml", "-f", "compose.dev.yml", "down"],
                ),
                (
                    "logs",
                    (),
                    [
                        "compose",
                        "-f",
                        "compose.yml",
                        "-f",
                        "compose.dev.yml",
                        "logs",
                        "-f",
                        "app",
                    ],
                ),
                (
                    "ps",
                    (),
                    ["compose", "-f", "compose.yml", "-f", "compose.dev.yml", "ps"],
                ),
                ("shell", (), ["compose", "run", "--rm", "tools", "bash"]),
                ("tools-up", (), ["compose", "up", "-d", "tools-daemon"]),
                ("tools-down", (), ["compose", "stop", "tools-daemon"]),
                (
                    "fmt-exec",
                    (),
                    [
                        "compose",
                        "exec",
                        "-T",
                        "tools-daemon",
                        "cargo",
                        "make",
                        "--allow-private",
                        "fmt-local",
                    ],
                ),
                (
                    "clippy-exec",
                    (),
                    [
                        "compose",
                        "exec",
                        "-T",
                        "tools-daemon",
                        "cargo",
                        "make",
                        "--allow-private",
                        "clippy-local",
                    ],
                ),
                (
                    "test-exec",
                    (),
                    [
                        "compose",
                        "exec",
                        "-T",
                        "tools-daemon",
                        "cargo",
                        "make",
                        "--allow-private",
                        "test-local",
                    ],
                ),
                (
                    "test-one-exec",
                    ("server::tests",),
                    [
                        "compose",
                        "exec",
                        "-T",
                        "tools-daemon",
                        "cargo",
                        "nextest",
                        "run",
                        "--locked",
                        "server::tests",
                    ],
                ),
                (
                    "check-exec",
                    (),
                    [
                        "compose",
                        "exec",
                        "-T",
                        "tools-daemon",
                        "cargo",
                        "make",
                        "--allow-private",
                        "check-local",
                    ],
                ),
                (
                    "machete-exec",
                    (),
                    [
                        "compose",
                        "exec",
                        "-T",
                        "tools-daemon",
                        "cargo",
                        "make",
                        "--allow-private",
                        "machete-local",
                    ],
                ),
                (
                    "deny-exec",
                    (),
                    [
                        "compose",
                        "exec",
                        "-T",
                        "tools-daemon",
                        "cargo",
                        "make",
                        "--allow-private",
                        "deny-local",
                    ],
                ),
                (
                    "llvm-cov-exec",
                    (),
                    [
                        "compose",
                        "exec",
                        "-T",
                        "tools-daemon",
                        "cargo",
                        "make",
                        "--allow-private",
                        "llvm-cov-local",
                    ],
                ),
                (
                    "fmt",
                    (),
                    [
                        "compose",
                        "run",
                        "--rm",
                        "tools",
                        "cargo",
                        "make",
                        "--allow-private",
                        "fmt-local",
                    ],
                ),
                (
                    "fmt-check",
                    (),
                    [
                        "compose",
                        "run",
                        "--rm",
                        "tools",
                        "cargo",
                        "make",
                        "--allow-private",
                        "fmt-check-local",
                    ],
                ),
                (
                    "clippy",
                    (),
                    [
                        "compose",
                        "run",
                        "--rm",
                        "tools",
                        "cargo",
                        "make",
                        "--allow-private",
                        "clippy-local",
                    ],
                ),
                (
                    "test",
                    (),
                    [
                        "compose",
                        "run",
                        "--rm",
                        "tools",
                        "cargo",
                        "make",
                        "--allow-private",
                        "test-local",
                    ],
                ),
                (
                    "test-doc",
                    (),
                    [
                        "compose",
                        "run",
                        "--rm",
                        "tools",
                        "cargo",
                        "make",
                        "--allow-private",
                        "test-doc-local",
                    ],
                ),
                (
                    "test-nocapture",
                    (),
                    [
                        "compose",
                        "run",
                        "--rm",
                        "tools",
                        "cargo",
                        "make",
                        "--allow-private",
                        "test-nocapture-local",
                    ],
                ),
                (
                    "bacon",
                    (),
                    [
                        "compose",
                        "-f",
                        "compose.yml",
                        "-f",
                        "compose.dev.yml",
                        "run",
                        "--rm",
                        "app",
                        "bacon",
                    ],
                ),
                (
                    "bacon-test",
                    (),
                    [
                        "compose",
                        "-f",
                        "compose.yml",
                        "-f",
                        "compose.dev.yml",
                        "run",
                        "--rm",
                        "app",
                        "bacon",
                        "test",
                        "--headless",
                    ],
                ),
                (
                    "check",
                    (),
                    [
                        "compose",
                        "run",
                        "--rm",
                        "tools",
                        "cargo",
                        "make",
                        "--allow-private",
                        "check-local",
                    ],
                ),
                (
                    "deny",
                    (),
                    [
                        "compose",
                        "run",
                        "--rm",
                        "tools",
                        "cargo",
                        "make",
                        "--allow-private",
                        "deny-local",
                    ],
                ),
                (
                    "machete",
                    (),
                    [
                        "compose",
                        "run",
                        "--rm",
                        "tools",
                        "cargo",
                        "make",
                        "--allow-private",
                        "machete-local",
                    ],
                ),
                (
                    "clippy-tests",
                    (),
                    [
                        "compose",
                        "run",
                        "--rm",
                        "tools",
                        "cargo",
                        "make",
                        "--allow-private",
                        "clippy-tests-local",
                    ],
                ),
                (
                    "llvm-cov",
                    (),
                    [
                        "compose",
                        "run",
                        "--rm",
                        "tools",
                        "cargo",
                        "make",
                        "--allow-private",
                        "llvm-cov-local",
                    ],
                ),
                (
                    "check-layers",
                    (),
                    [
                        "compose",
                        "run",
                        "--rm",
                        "tools",
                        "cargo",
                        "make",
                        "--allow-private",
                        "check-layers-local",
                    ],
                ),
                (
                    "verify-arch-docs",
                    (),
                    [
                        "compose",
                        "run",
                        "--rm",
                        "tools",
                        "cargo",
                        "make",
                        "--allow-private",
                        "verify-arch-docs-local",
                    ],
                ),
                (
                    "verify-plan-progress",
                    (),
                    [
                        "compose",
                        "run",
                        "--rm",
                        "tools",
                        "cargo",
                        "make",
                        "--allow-private",
                        "verify-plan-progress-local",
                    ],
                ),
                (
                    "verify-track-metadata",
                    (),
                    [
                        "compose",
                        "run",
                        "--rm",
                        "tools",
                        "cargo",
                        "make",
                        "--allow-private",
                        "verify-track-metadata-local",
                    ],
                ),
                (
                    "verify-tech-stack",
                    (),
                    [
                        "compose",
                        "run",
                        "--rm",
                        "tools",
                        "cargo",
                        "make",
                        "--allow-private",
                        "verify-tech-stack-local",
                    ],
                ),
                (
                    "verify-orchestra",
                    (),
                    [
                        "compose",
                        "run",
                        "--rm",
                        "tools",
                        "cargo",
                        "make",
                        "--allow-private",
                        "verify-orchestra-local",
                    ],
                ),
                (
                    "ci-rust",
                    (),
                    [
                        "compose",
                        "run",
                        "--rm",
                        "tools",
                        "cargo",
                        "make",
                        "--allow-private",
                        "ci-rust-local",
                    ],
                ),
                (
                    "ci",
                    (),
                    [
                        "compose",
                        "run",
                        "--rm",
                        "tools",
                        "cargo",
                        "make",
                        "--allow-private",
                        "ci-local",
                    ],
                ),
                ("clean", (), ["compose", "down", "--volumes", "--remove-orphans"]),
            )

            path_with_stub = str(root / "bin") + os.pathsep + os.environ["PATH"]
            for task, args, expected in cases:
                with self.subTest(task=task):
                    result = self.run_make(
                        root, task, *args, env_updates={"PATH": path_with_stub}
                    )
                    self.assertEqual(
                        result.returncode, 0, result.stdout + result.stderr
                    )
                    payload = self.parse_json_line(result.stdout)
                    self.assertEqual(payload["script"], "docker")
                    self.assertEqual(payload["argv"], expected)
                    self.assertEqual(payload["docker_buildkit"], "1")
                    self.assertEqual(payload["compose_docker_cli_build"], "1")


if __name__ == "__main__":
    unittest.main()
