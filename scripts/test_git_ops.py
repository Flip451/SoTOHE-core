import json
import os
import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from scripts import git_ops


class GitOpsTest(unittest.TestCase):
    def setUp(self) -> None:
        self.temp_dir = tempfile.TemporaryDirectory()
        self.root = Path(self.temp_dir.name)
        self.previous_cwd = Path.cwd()
        os.chdir(self.root)
        self.run_git("init")
        self.run_git("config", "user.email", "codex@example.com")
        self.run_git("config", "user.name", "Codex")

    def tearDown(self) -> None:
        os.chdir(self.previous_cwd)
        self.temp_dir.cleanup()

    def run_git(self, *args: str) -> str:
        result = subprocess.run(
            ["git", *args],
            check=True,
            text=True,
            capture_output=True,
        )
        return result.stdout

    def test_add_all_excludes_transient_track_commit_files(self) -> None:
        (self.root / "tracked.txt").write_text("base\n", encoding="utf-8")
        self.run_git("add", "tracked.txt")
        self.run_git("commit", "-m", "initial")

        (self.root / "tracked.txt").write_text("changed\n", encoding="utf-8")
        (self.root / "new.txt").write_text("new\n", encoding="utf-8")
        track_commit_dir = self.root / "tmp" / "track-commit"
        track_commit_dir.mkdir(parents=True, exist_ok=True)
        (track_commit_dir / "add-paths.txt").write_text(
            "tracked.txt\n", encoding="utf-8"
        )
        (track_commit_dir / "commit-message.txt").write_text(
            "track commit message\n", encoding="utf-8"
        )
        (track_commit_dir / "note.md").write_text("track note\n", encoding="utf-8")

        code = git_ops.main(["add-all"])

        self.assertEqual(code, 0)
        staged = set(self.run_git("diff", "--cached", "--name-only").splitlines())
        self.assertEqual(staged, {"new.txt", "tracked.txt"})

    def test_add_from_file_stages_selected_paths_and_cleans_up(self) -> None:
        (self.root / "tracked.txt").write_text("base\n", encoding="utf-8")
        (self.root / "other.txt").write_text("base\n", encoding="utf-8")
        self.run_git("add", "tracked.txt", "other.txt")
        self.run_git("commit", "-m", "initial")

        (self.root / "tracked.txt").write_text("changed\n", encoding="utf-8")
        (self.root / "other.txt").write_text("changed\n", encoding="utf-8")
        stage_list = self.root / "tmp" / "track-commit" / "add-paths.txt"
        stage_list.parent.mkdir(parents=True, exist_ok=True)
        stage_list.write_text("tracked.txt\n# comment\ntracked.txt\n", encoding="utf-8")

        code = git_ops.main(["add-from-file", str(stage_list), "--cleanup"])

        self.assertEqual(code, 0)
        self.assertFalse(stage_list.exists())
        staged = set(self.run_git("diff", "--cached", "--name-only").splitlines())
        self.assertEqual(staged, {"tracked.txt"})

    def test_add_from_file_rejects_transient_automation_paths(self) -> None:
        stage_list = self.root / "tmp" / "track-commit" / "add-paths.txt"
        stage_list.parent.mkdir(parents=True, exist_ok=True)
        stage_list.write_text("tmp/track-commit/note.md\n", encoding="utf-8")

        code = git_ops.main(["add-from-file", str(stage_list)])

        self.assertEqual(code, 1)

    def test_add_from_file_rejects_transient_parent_directories(self) -> None:
        (self.root / "tracked.txt").write_text("base\n", encoding="utf-8")
        self.run_git("add", "tracked.txt")
        self.run_git("commit", "-m", "initial")

        track_commit_dir = self.root / "tmp" / "track-commit"
        track_commit_dir.mkdir(parents=True, exist_ok=True)
        (track_commit_dir / "commit-message.txt").write_text(
            "message\n", encoding="utf-8"
        )
        (track_commit_dir / "note.md").write_text("note\n", encoding="utf-8")
        stage_list = track_commit_dir / "add-paths.txt"
        stage_list.write_text("tmp/track-commit\n", encoding="utf-8")

        code = git_ops.main(["add-from-file", str(stage_list), "--cleanup"])

        self.assertEqual(code, 1)
        self.assertTrue(stage_list.exists())
        staged = set(self.run_git("diff", "--cached", "--name-only").splitlines())
        self.assertEqual(staged, set())

    def test_add_from_file_rejects_git_pathspec_shorthand(self) -> None:
        (self.root / "tracked.txt").write_text("base\n", encoding="utf-8")
        self.run_git("add", "tracked.txt")
        self.run_git("commit", "-m", "initial")

        (self.root / "new.txt").write_text("new\n", encoding="utf-8")
        track_commit_dir = self.root / "tmp" / "track-commit"
        track_commit_dir.mkdir(parents=True, exist_ok=True)
        (track_commit_dir / "commit-message.txt").write_text(
            "message\n", encoding="utf-8"
        )
        (track_commit_dir / "note.md").write_text("note\n", encoding="utf-8")
        stage_list = track_commit_dir / "add-paths.txt"

        for entry in (":/", ":!tmp/track-commit", ":^tmp/track-commit"):
            with self.subTest(entry=entry):
                stage_list.write_text(f"{entry}\n", encoding="utf-8")

                code = git_ops.main(["add-from-file", str(stage_list), "--cleanup"])

                self.assertEqual(code, 1)
                self.assertTrue(stage_list.exists())
                staged = set(
                    self.run_git("diff", "--cached", "--name-only").splitlines()
                )
                self.assertEqual(staged, set())

    def test_add_from_file_rejects_glob_patterns(self) -> None:
        (self.root / "tracked.txt").write_text("base\n", encoding="utf-8")
        self.run_git("add", "tracked.txt")
        self.run_git("commit", "-m", "initial")

        (self.root / "src").mkdir(parents=True, exist_ok=True)
        (self.root / "src" / "lib.rs").write_text("// test\n", encoding="utf-8")
        stage_list = self.root / "tmp" / "track-commit" / "add-paths.txt"
        stage_list.parent.mkdir(parents=True, exist_ok=True)
        stage_list.write_text("src/*.rs\n", encoding="utf-8")

        code = git_ops.main(["add-from-file", str(stage_list), "--cleanup"])

        self.assertEqual(code, 1)
        self.assertTrue(stage_list.exists())
        staged = set(self.run_git("diff", "--cached", "--name-only").splitlines())
        self.assertEqual(staged, set())

    def test_add_from_file_rejects_missing_file(self) -> None:
        code = git_ops.main(["add-from-file", "tmp/track-commit/missing-paths.txt"])

        self.assertEqual(code, 1)

    def test_commit_from_file_uses_track_commit_scratch_and_cleans_up(self) -> None:
        (self.root / "tracked.txt").write_text("base\n", encoding="utf-8")
        self.run_git("add", "tracked.txt")
        self.run_git("commit", "-m", "initial")
        self.run_git("checkout", "-b", "track/example")

        track_dir = self.root / "track" / "items" / "example"
        track_dir.mkdir(parents=True, exist_ok=True)
        (track_dir / "metadata.json").write_text(
            '{"schema_version":3,"branch":"track/example","status":"in_progress"}',
            encoding="utf-8",
        )

        (self.root / "tracked.txt").write_text("changed\n", encoding="utf-8")
        self.run_git("add", "tracked.txt")
        message_path = self.root / "tmp" / "track-commit" / "commit-message.txt"
        message_path.parent.mkdir(parents=True, exist_ok=True)
        message_path.write_text("Commit from file\n\nbody\n", encoding="utf-8")

        # Mock _repo_root so the branch guard resolves inside the tmpdir
        # (avoids detached HEAD in CI where the outer repo is checked out headless).
        with patch.object(git_ops, "_repo_root", return_value=self.root):
            code = git_ops.main(["commit-from-file", str(message_path), "--cleanup"])

        self.assertEqual(code, 0)
        self.assertFalse(message_path.exists())
        self.assertEqual(
            self.run_git("log", "-1", "--pretty=%s").strip(), "Commit from file"
        )

    def test_commit_from_file_requires_explicit_selector_on_non_track_branch(self) -> None:
        (self.root / "tracked.txt").write_text("base\n", encoding="utf-8")
        self.run_git("add", "tracked.txt")
        self.run_git("commit", "-m", "initial")

        (self.root / "tracked.txt").write_text("changed\n", encoding="utf-8")
        self.run_git("add", "tracked.txt")
        message_path = self.root / "tmp" / "track-commit" / "commit-message.txt"
        message_path.parent.mkdir(parents=True, exist_ok=True)
        message_path.write_text("Commit from main\n", encoding="utf-8")

        with patch.object(git_ops, "_repo_root", return_value=self.root):
            code = git_ops.main(["commit-from-file", str(message_path), "--cleanup"])

        self.assertEqual(code, 1)
        self.assertTrue(message_path.exists())
        self.assertEqual(self.run_git("log", "-1", "--pretty=%s").strip(), "initial")

    def test_commit_from_file_rejects_non_artifact_changes_for_planning_only_track(
        self,
    ) -> None:
        track_dir = self.root / "track" / "items" / "example"
        track_dir.mkdir(parents=True, exist_ok=True)
        (track_dir / "metadata.json").write_text(
            '{"schema_version":3,"branch":null,"status":"planned"}',
            encoding="utf-8",
        )
        (self.root / "src.rs").write_text("fn main() {}\n", encoding="utf-8")
        self.run_git("add", "src.rs")
        message_path = self.root / "tmp" / "track-commit" / "commit-message.txt"
        track_dir_file = self.root / "tmp" / "track-commit" / "track-dir.txt"
        message_path.parent.mkdir(parents=True, exist_ok=True)
        message_path.write_text("Planning-only commit\n", encoding="utf-8")
        track_dir_file.write_text("track/items/example\n", encoding="utf-8")

        with patch.object(git_ops, "_repo_root", return_value=self.root):
            code = git_ops.main(["commit-from-file", str(message_path), "--cleanup"])

        self.assertEqual(code, 1)
        self.assertTrue(message_path.exists())
        self.assertFalse(track_dir_file.exists())
        self.assertIn("src.rs", self.run_git("diff", "--cached", "--name-only"))

    def test_commit_from_file_rejects_deletions_outside_planning_only_allowlist(self) -> None:
        (self.root / "src.rs").write_text("fn main() {}\n", encoding="utf-8")
        self.run_git("add", "src.rs")
        self.run_git("commit", "-m", "initial")

        track_dir = self.root / "track" / "items" / "example"
        track_dir.mkdir(parents=True, exist_ok=True)
        (track_dir / "metadata.json").write_text(
            '{"schema_version":3,"branch":null,"status":"planned"}',
            encoding="utf-8",
        )

        (self.root / "src.rs").unlink()
        self.run_git("add", "-u", "src.rs")
        message_path = self.root / "tmp" / "track-commit" / "commit-message.txt"
        track_dir_file = self.root / "tmp" / "track-commit" / "track-dir.txt"
        message_path.parent.mkdir(parents=True, exist_ok=True)
        message_path.write_text("Planning-only commit\n", encoding="utf-8")
        track_dir_file.write_text("track/items/example\n", encoding="utf-8")

        with patch.object(git_ops, "_repo_root", return_value=self.root):
            code = git_ops.main(["commit-from-file", str(message_path), "--cleanup"])

        self.assertEqual(code, 1)
        self.assertTrue(message_path.exists())
        self.assertFalse(track_dir_file.exists())
        self.assertIn("D\tsrc.rs", self.run_git("diff", "--cached", "--name-status"))

    def test_commit_from_file_resolves_relative_track_dir_file_from_nested_directory(
        self,
    ) -> None:
        (self.root / "tracked.txt").write_text("base\n", encoding="utf-8")
        self.run_git("add", "tracked.txt")
        self.run_git("commit", "-m", "initial")
        self.run_git("checkout", "-b", "track/example")

        track_dir = self.root / "track" / "items" / "example"
        track_dir.mkdir(parents=True, exist_ok=True)
        (track_dir / "metadata.json").write_text(
            '{"schema_version":3,"branch":"track/example","status":"in_progress"}',
            encoding="utf-8",
        )

        (self.root / "tracked.txt").write_text("changed\n", encoding="utf-8")
        self.run_git("add", "tracked.txt")
        scratch = self.root / "tmp" / "track-commit"
        scratch.mkdir(parents=True, exist_ok=True)
        message_path = scratch / "commit-message.txt"
        track_dir_file = scratch / "track-dir.txt"
        message_path.write_text("Nested commit\n", encoding="utf-8")
        track_dir_file.write_text("track/items/example\n", encoding="utf-8")

        nested = self.root / "nested"
        nested.mkdir(parents=True, exist_ok=True)
        current = Path.cwd()
        try:
            os.chdir(nested)
            with patch.object(git_ops, "_repo_root", return_value=self.root):
                code = git_ops.main(["commit-from-file", str(message_path), "--cleanup"])
        finally:
            os.chdir(current)

        self.assertEqual(code, 0)
        self.assertFalse(message_path.exists())
        self.assertFalse(track_dir_file.exists())
        self.assertEqual(self.run_git("log", "-1", "--pretty=%s").strip(), "Nested commit")

    def test_commit_from_file_rejects_illegal_branchless_v3_track_selector(self) -> None:
        track_dir = self.root / "track" / "items" / "example"
        track_dir.mkdir(parents=True, exist_ok=True)
        (track_dir / "metadata.json").write_text(
            '{"schema_version":3,"branch":null,"status":"in_progress","tasks":[]}',
            encoding="utf-8",
        )
        message_path = self.root / "tmp" / "track-commit" / "commit-message.txt"
        track_dir_file = self.root / "tmp" / "track-commit" / "track-dir.txt"
        message_path.parent.mkdir(parents=True, exist_ok=True)
        message_path.write_text("Should fail\n", encoding="utf-8")
        track_dir_file.write_text("track/items/example\n", encoding="utf-8")

        with patch.object(git_ops, "_repo_root", return_value=self.root):
            code = git_ops.main(["commit-from-file", str(message_path), "--cleanup"])

        self.assertEqual(code, 1)
        self.assertTrue(message_path.exists())
        self.assertFalse(track_dir_file.exists())

    def test_branch_guard_rejects_unactivated_planning_only_track_branch(self) -> None:
        items_dir = self.root / "track" / "items"
        items_dir.mkdir(parents=True, exist_ok=True)
        track_dir = items_dir / "example"
        track_dir.mkdir(parents=True, exist_ok=True)
        (track_dir / "metadata.json").write_text(
            json.dumps(
                {
                    "schema_version": 3,
                    "id": "example",
                    "branch": None,
                    "status": "planned",
                }
            ),
            encoding="utf-8",
        )

        (self.root / "tracked.txt").write_text("base\n", encoding="utf-8")
        self.run_git("add", "tracked.txt")
        self.run_git("commit", "-m", "initial")
        self.run_git("checkout", "-b", "track/example")

        message_path = self.root / "tmp" / "track-commit" / "commit-message.txt"
        message_path.parent.mkdir(parents=True, exist_ok=True)
        message_path.write_text("Should fail\n", encoding="utf-8")

        with patch.object(git_ops, "_repo_root", return_value=self.root):
            code = git_ops.main(["commit-from-file", str(message_path), "--cleanup"])

        self.assertEqual(code, 1)
        self.assertEqual(self.run_git("log", "-1", "--pretty=%s").strip(), "initial")

    def test_branch_guard_rejects_illegal_branchless_v3_track_branch(self) -> None:
        items_dir = self.root / "track" / "items"
        items_dir.mkdir(parents=True, exist_ok=True)
        track_dir = items_dir / "example"
        track_dir.mkdir(parents=True, exist_ok=True)
        (track_dir / "metadata.json").write_text(
            json.dumps(
                {
                    "schema_version": 3,
                    "id": "example",
                    "branch": None,
                    "status": "in_progress",
                }
            ),
            encoding="utf-8",
        )

        (self.root / "tracked.txt").write_text("base\n", encoding="utf-8")
        self.run_git("add", "tracked.txt")
        self.run_git("commit", "-m", "initial")
        self.run_git("checkout", "-b", "track/example")

        message_path = self.root / "tmp" / "track-commit" / "commit-message.txt"
        message_path.parent.mkdir(parents=True, exist_ok=True)
        message_path.write_text("Should fail\n", encoding="utf-8")

        with patch.object(git_ops, "_repo_root", return_value=self.root):
            code = git_ops.main(["commit-from-file", str(message_path), "--cleanup"])

        self.assertEqual(code, 1)
        self.assertEqual(self.run_git("log", "-1", "--pretty=%s").strip(), "initial")

    def test_note_from_file_uses_track_commit_scratch_and_cleans_up(self) -> None:
        (self.root / "tracked.txt").write_text("base\n", encoding="utf-8")
        self.run_git("add", "tracked.txt")
        self.run_git("commit", "-m", "initial")

        note_path = self.root / "tmp" / "track-commit" / "note.md"
        note_path.parent.mkdir(parents=True, exist_ok=True)
        note_path.write_text("note line 1\nnote line 2\n", encoding="utf-8")

        code = git_ops.main(["note-from-file", str(note_path), "--cleanup"])

        self.assertEqual(code, 0)
        self.assertFalse(note_path.exists())
        self.assertEqual(
            self.run_git("notes", "show", "HEAD"), "note line 1\nnote line 2\n"
        )

    def test_commit_from_file_rejects_missing_file(self) -> None:
        code = git_ops.main(["commit-from-file", ".takt/missing-message.txt"])

        self.assertEqual(code, 1)

    def test_note_from_file_rejects_missing_file(self) -> None:
        code = git_ops.main(["note-from-file", ".takt/missing-note.txt"])

        self.assertEqual(code, 1)

    def test_branch_guard_allows_archived_track_in_archive_dir(self) -> None:
        """Branch guard should pass when the track is in track/archive/ with status archived."""
        import json

        # Need an initial commit so git rev-parse works
        (self.root / "dummy.txt").write_text("init\n", encoding="utf-8")
        self.run_git("add", "dummy.txt")
        self.run_git("commit", "-m", "initial")

        archive_dir = self.root / "track" / "archive" / "my-feat"
        archive_dir.mkdir(parents=True, exist_ok=True)
        (archive_dir / "metadata.json").write_text(
            json.dumps({
                "schema_version": 3,
                "id": "my-feat",
                "branch": "track/my-feat",
                "status": "archived",
            })
            + "\n",
            encoding="utf-8",
        )
        # track/items/ exists but is empty — no track claims the branch there
        items_dir = self.root / "track" / "items"
        items_dir.mkdir(parents=True, exist_ok=True)

        self.run_git("checkout", "-b", "track/my-feat")

        with patch.object(git_ops, "_repo_root", return_value=self.root), \
             patch.object(git_ops, "_safe_repo_items_dir", return_value=(self.root, items_dir.resolve())):
            code = git_ops._verify_branch_by_auto_detection()

        self.assertEqual(code, 0)

    def test_branch_guard_rejects_non_archived_track_in_archive_dir(self) -> None:
        """Branch guard should reject track in track/archive/ that is not status archived."""
        import json

        # Need an initial commit so git rev-parse works
        (self.root / "dummy.txt").write_text("init\n", encoding="utf-8")
        self.run_git("add", "dummy.txt")
        self.run_git("commit", "-m", "initial")

        archive_dir = self.root / "track" / "archive" / "bad-feat"
        archive_dir.mkdir(parents=True, exist_ok=True)
        (archive_dir / "metadata.json").write_text(
            json.dumps({
                "schema_version": 3,
                "id": "bad-feat",
                "branch": "track/bad-feat",
                "status": "done",
            })
            + "\n",
            encoding="utf-8",
        )
        items_dir = self.root / "track" / "items"
        items_dir.mkdir(parents=True, exist_ok=True)

        self.run_git("checkout", "-b", "track/bad-feat")

        with patch.object(git_ops, "_repo_root", return_value=self.root), \
             patch.object(git_ops, "_safe_repo_items_dir", return_value=(self.root, items_dir.resolve())):
            code = git_ops._verify_branch_by_auto_detection()

        self.assertEqual(code, 1)


if __name__ == "__main__":
    unittest.main()
