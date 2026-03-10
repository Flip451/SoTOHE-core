import os
import subprocess
import tempfile
import unittest
from pathlib import Path

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

    def test_add_all_excludes_transient_pending_and_track_commit_files(self) -> None:
        (self.root / "tracked.txt").write_text("base\n", encoding="utf-8")
        self.run_git("add", "tracked.txt")
        self.run_git("commit", "-m", "initial")

        (self.root / "tracked.txt").write_text("changed\n", encoding="utf-8")
        (self.root / "new.txt").write_text("new\n", encoding="utf-8")
        pending_dir = self.root / ".takt"
        pending_dir.mkdir(parents=True, exist_ok=True)
        (pending_dir / "pending-add-paths.txt").write_text(
            "tracked.txt\n", encoding="utf-8"
        )
        (pending_dir / "pending-note.md").write_text("note\n", encoding="utf-8")
        (pending_dir / "pending-commit-message.txt").write_text(
            "message\n", encoding="utf-8"
        )
        handoffs_dir = pending_dir / "handoffs"
        handoffs_dir.mkdir(parents=True, exist_ok=True)
        (handoffs_dir / "handoff-my-task-20260308T120000Z.md").write_text(
            "handoff\n", encoding="utf-8"
        )
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

    def test_add_from_file_rejects_transient_automation_dir_contents(self) -> None:
        stage_list = self.root / "tmp" / "track-commit" / "add-paths.txt"
        stage_list.parent.mkdir(parents=True, exist_ok=True)
        stage_list.write_text(
            ".takt/handoffs/handoff-task-20260308T120000Z.md\n", encoding="utf-8"
        )

        code = git_ops.main(["add-from-file", str(stage_list)])

        self.assertEqual(code, 1)

    def test_add_from_file_rejects_transient_automation_dir_itself(self) -> None:
        stage_list = self.root / "tmp" / "track-commit" / "add-paths.txt"
        stage_list.parent.mkdir(parents=True, exist_ok=True)
        stage_list.write_text(".takt/handoffs\n", encoding="utf-8")

        code = git_ops.main(["add-from-file", str(stage_list)])

        self.assertEqual(code, 1)

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

    def test_commit_from_file_uses_message_and_cleans_up(self) -> None:
        (self.root / "tracked.txt").write_text("base\n", encoding="utf-8")
        self.run_git("add", "tracked.txt")
        self.run_git("commit", "-m", "initial")

        (self.root / "tracked.txt").write_text("changed\n", encoding="utf-8")
        self.run_git("add", "tracked.txt")
        message_path = self.root / ".takt" / "pending-commit-message.txt"
        message_path.parent.mkdir(parents=True, exist_ok=True)
        message_path.write_text("Commit from file\n\nbody\n", encoding="utf-8")

        code = git_ops.main(["commit-from-file", str(message_path), "--cleanup"])

        self.assertEqual(code, 0)
        self.assertFalse(message_path.exists())
        self.assertEqual(
            self.run_git("log", "-1", "--pretty=%s").strip(), "Commit from file"
        )

    def test_note_from_file_uses_contents_and_cleans_up(self) -> None:
        (self.root / "tracked.txt").write_text("base\n", encoding="utf-8")
        self.run_git("add", "tracked.txt")
        self.run_git("commit", "-m", "initial")

        note_path = self.root / ".takt" / "pending-note.md"
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


if __name__ == "__main__":
    unittest.main()
