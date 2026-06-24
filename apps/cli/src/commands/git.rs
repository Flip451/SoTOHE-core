//! CLI subcommands for guarded local git workflow wrappers.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Args, Subcommand};
use cli_composition::GitCompositionRoot;

use crate::commands::outcome_to_exit;

#[derive(Debug, Subcommand)]
pub enum GitCommand {
    /// Stage the whole worktree except transient automation scratch files.
    AddAll,
    /// Stage repo-relative paths listed in a file.
    AddFromFile(FileArgs),
    /// Create a commit using the message stored in a file.
    CommitFromFile(CommitFromFileArgs),
    /// Attach a git note using the contents of a file.
    NoteFromFile(FileArgs),
    /// Switch to a branch and pull latest changes.
    SwitchAndPull(SwitchAndPullArgs),
    /// Unstage paths (remove from git index without discarding worktree changes).
    Unstage(UnstageArgs),
}

#[derive(Debug, Args)]
pub struct FileArgs {
    pub path: PathBuf,
    #[arg(long, default_value_t = false)]
    pub cleanup: bool,
}

#[derive(Debug, Args)]
pub struct CommitFromFileArgs {
    pub path: PathBuf,
    #[arg(long, default_value_t = false)]
    pub cleanup: bool,
    #[arg(long)]
    pub track_dir: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct SwitchAndPullArgs {
    pub branch: String,
}

#[derive(Debug, Args)]
pub struct UnstageArgs {
    /// Paths to unstage (repo-relative).
    #[arg(required = true)]
    pub paths: Vec<PathBuf>,
}

pub fn execute(cmd: GitCommand) -> ExitCode {
    let app = GitCompositionRoot::new();
    match cmd {
        GitCommand::AddAll => outcome_to_exit(app.git_add_all()),
        GitCommand::AddFromFile(args) => {
            outcome_to_exit(app.git_add_from_file(args.path, args.cleanup))
        }
        GitCommand::CommitFromFile(args) => {
            outcome_to_exit(app.git_commit_from_file(args.path, args.cleanup, args.track_dir))
        }
        GitCommand::NoteFromFile(args) => {
            outcome_to_exit(app.git_note_from_file(args.path, args.cleanup))
        }
        GitCommand::SwitchAndPull(args) => outcome_to_exit(app.git_switch_and_pull(args.branch)),
        GitCommand::Unstage(args) => outcome_to_exit(app.git_unstage(args.paths)),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::{Command, ExitCode};
    use std::sync::{Mutex, OnceLock};

    use infrastructure::git_cli::{GitRepository, resolve_repo_path};

    // Private helpers re-exported from cli-composition for test access.
    // These are kept here so existing integration-level tests continue to work.

    fn cwd_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    struct CurrentDirGuard {
        original: PathBuf,
    }

    impl CurrentDirGuard {
        fn change_to(path: &Path) -> Self {
            let original = std::env::current_dir().unwrap();
            std::env::set_current_dir(path).unwrap();
            Self { original }
        }
    }

    impl Drop for CurrentDirGuard {
        fn drop(&mut self) {
            std::env::set_current_dir(&self.original).unwrap();
        }
    }

    fn run_git(root: &Path, args: &[&str]) {
        let status = Command::new("git").args(args).current_dir(root).status().unwrap();
        assert!(status.success(), "git command failed: git {}", args.join(" "));
    }

    fn run_git_output(root: &Path, args: &[&str]) -> String {
        let output = Command::new("git").args(args).current_dir(root).output().unwrap();
        assert!(output.status.success(), "git command failed: git {}", args.join(" "));
        String::from_utf8(output.stdout).unwrap()
    }

    fn init_repo() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        run_git(dir.path(), &["init", "-b", "main"]);
        run_git(dir.path(), &["config", "user.email", "codex@example.com"]);
        run_git(dir.path(), &["config", "user.name", "Codex"]);
        dir
    }

    #[test]
    fn resolve_repo_path_anchors_relative_paths_at_repo_root() {
        let root = Path::new("/repo/root");

        assert_eq!(
            resolve_repo_path(root, Path::new("tmp/track-commit/note.md")),
            PathBuf::from("/repo/root/tmp/track-commit/note.md")
        );
        assert_eq!(
            resolve_repo_path(root, Path::new("/tmp/note.md")),
            PathBuf::from("/tmp/note.md")
        );
    }

    // Helper: discover the git repo and return a SystemGitRepo for test assertions.
    fn repo() -> infrastructure::git_cli::SystemGitRepo {
        infrastructure::git_cli::SystemGitRepo::discover().unwrap()
    }

    #[test]
    fn repo_root_resolves_git_toplevel_from_nested_directory() {
        let _lock = cwd_lock().lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        run_git(dir.path(), &["init"]);
        let nested = dir.path().join("nested/deeper");
        fs::create_dir_all(&nested).unwrap();

        let _guard = CurrentDirGuard::change_to(&nested);

        let r = repo();

        assert_eq!(r.root(), dir.path());
    }

    #[test]
    fn add_all_excludes_transient_files_from_nested_directory() {
        let _lock = cwd_lock().lock().unwrap();
        let dir = init_repo();
        fs::write(dir.path().join("tracked.txt"), "base\n").unwrap();
        run_git(dir.path(), &["add", "tracked.txt"]);
        run_git(dir.path(), &["commit", "-m", "initial"]);

        fs::write(dir.path().join("tracked.txt"), "changed\n").unwrap();
        fs::write(dir.path().join("new.txt"), "new\n").unwrap();
        fs::create_dir_all(dir.path().join("tmp/track-commit")).unwrap();
        fs::write(dir.path().join("tmp/track-commit/commit-message.txt"), "message\n").unwrap();

        let nested = dir.path().join("nested");
        fs::create_dir_all(&nested).unwrap();
        let _guard = CurrentDirGuard::change_to(&nested);

        assert_eq!(super::execute(super::GitCommand::AddAll), ExitCode::SUCCESS);
        assert_eq!(
            run_git_output(dir.path(), &["diff", "--cached", "--name-only"])
                .lines()
                .collect::<Vec<_>>(),
            vec!["new.txt", "tracked.txt"]
        );
    }

    #[test]
    fn add_all_succeeds_when_gitignored_transient_dir_exists() {
        let _lock = cwd_lock().lock().unwrap();
        let dir = init_repo();
        fs::write(dir.path().join("tracked.txt"), "base\n").unwrap();
        run_git(dir.path(), &["add", "tracked.txt"]);
        run_git(dir.path(), &["commit", "-m", "initial"]);

        fs::write(dir.path().join(".gitignore"), "tmp/\n").unwrap();
        fs::create_dir_all(dir.path().join("tmp/track-commit")).unwrap();
        fs::write(dir.path().join("tmp/track-commit/commit-message.txt"), "message\n").unwrap();
        fs::write(dir.path().join("tracked.txt"), "changed\n").unwrap();

        let _guard = CurrentDirGuard::change_to(dir.path());

        assert_eq!(super::execute(super::GitCommand::AddAll), ExitCode::SUCCESS);

        let staged = run_git_output(dir.path(), &["diff", "--cached", "--name-only"]);
        let staged_files: Vec<&str> = staged.lines().collect();
        assert!(
            staged_files.contains(&".gitignore"),
            "expected .gitignore to be staged, got: {staged_files:?}"
        );
        assert!(
            staged_files.contains(&"tracked.txt"),
            "expected tracked.txt to be staged, got: {staged_files:?}"
        );
    }

    #[test]
    fn add_all_succeeds_when_gitignored_transient_file_exists() {
        let _lock = cwd_lock().lock().unwrap();
        let dir = init_repo();
        fs::write(dir.path().join("tracked.txt"), "base\n").unwrap();
        run_git(dir.path(), &["add", "tracked.txt"]);
        run_git(dir.path(), &["commit", "-m", "initial"]);

        fs::write(dir.path().join(".gitignore"), "tmp/track-commit/commit-message.txt\n").unwrap();
        fs::create_dir_all(dir.path().join("tmp/track-commit")).unwrap();
        fs::write(dir.path().join("tmp/track-commit/commit-message.txt"), "message\n").unwrap();
        fs::write(dir.path().join("tracked.txt"), "changed\n").unwrap();

        let _guard = CurrentDirGuard::change_to(dir.path());

        assert_eq!(super::execute(super::GitCommand::AddAll), ExitCode::SUCCESS);

        let staged = run_git_output(dir.path(), &["diff", "--cached", "--name-only"]);
        let staged_files: Vec<&str> = staged.lines().collect();
        assert!(
            staged_files.contains(&"tracked.txt"),
            "expected tracked.txt to be staged, got: {staged_files:?}"
        );
    }

    #[test]
    fn add_from_file_stages_paths_from_nested_directory() {
        let _lock = cwd_lock().lock().unwrap();
        let dir = init_repo();
        fs::write(dir.path().join("tracked.txt"), "base\n").unwrap();
        run_git(dir.path(), &["add", "tracked.txt"]);
        run_git(dir.path(), &["commit", "-m", "initial"]);

        fs::write(dir.path().join("tracked.txt"), "changed\n").unwrap();
        fs::create_dir_all(dir.path().join("tmp/track-commit")).unwrap();
        let add_paths = dir.path().join("tmp/track-commit/add-paths.txt");
        fs::write(&add_paths, "tracked.txt\n").unwrap();

        let nested = dir.path().join("nested");
        fs::create_dir_all(&nested).unwrap();
        let _guard = CurrentDirGuard::change_to(&nested);

        assert_eq!(
            super::execute(super::GitCommand::AddFromFile(super::FileArgs {
                path: PathBuf::from("tmp/track-commit/add-paths.txt"),
                cleanup: true,
            })),
            ExitCode::SUCCESS
        );
        assert!(!add_paths.exists());
        assert_eq!(
            run_git_output(dir.path(), &["diff", "--cached", "--name-only"]).trim(),
            "tracked.txt"
        );
    }

    #[test]
    fn commit_from_file_uses_explicit_track_dir_from_nested_directory() {
        let _lock = cwd_lock().lock().unwrap();
        let dir = init_repo();
        fs::write(dir.path().join("tracked.txt"), "base\n").unwrap();
        run_git(dir.path(), &["add", "tracked.txt"]);
        run_git(dir.path(), &["commit", "-m", "initial"]);
        run_git(dir.path(), &["checkout", "-b", "track/example"]);

        let track_dir = dir.path().join("track/items/example");
        fs::create_dir_all(&track_dir).unwrap();
        fs::write(
            track_dir.join("metadata.json"),
            r#"{"branch":"track/example","status":"in_progress"}"#,
        )
        .unwrap();

        fs::write(dir.path().join("tracked.txt"), "changed\n").unwrap();
        run_git(dir.path(), &["add", "tracked.txt"]);
        let scratch = dir.path().join("tmp/track-commit");
        fs::create_dir_all(&scratch).unwrap();
        let commit_message = scratch.join("commit-message.txt");
        fs::write(&commit_message, "Track commit\n").unwrap();

        let nested = dir.path().join("nested");
        fs::create_dir_all(&nested).unwrap();
        let _guard = CurrentDirGuard::change_to(&nested);

        assert_eq!(
            super::execute(super::GitCommand::CommitFromFile(super::CommitFromFileArgs {
                path: PathBuf::from("tmp/track-commit/commit-message.txt"),
                cleanup: true,
                track_dir: Some(PathBuf::from("track/items/example")),
            })),
            ExitCode::SUCCESS
        );
        assert!(!commit_message.exists());
        assert_eq!(repo().current_branch().unwrap().as_deref(), Some("track/example"));
        assert_eq!(
            run_git_output(dir.path(), &["log", "-1", "--pretty=%s"]).trim(),
            "Track commit"
        );
    }

    #[test]
    fn commit_from_file_requires_track_branch_when_no_explicit_selector() {
        let _lock = cwd_lock().lock().unwrap();
        let dir = init_repo();
        fs::write(dir.path().join("tracked.txt"), "base\n").unwrap();
        run_git(dir.path(), &["add", "tracked.txt"]);
        run_git(dir.path(), &["commit", "-m", "initial"]);

        fs::write(dir.path().join("tracked.txt"), "changed\n").unwrap();
        run_git(dir.path(), &["add", "tracked.txt"]);
        let scratch = dir.path().join("tmp/track-commit");
        fs::create_dir_all(&scratch).unwrap();
        let commit_message = scratch.join("commit-message.txt");
        fs::write(&commit_message, "Commit from main\n").unwrap();

        let nested = dir.path().join("nested");
        fs::create_dir_all(&nested).unwrap();
        let _guard = CurrentDirGuard::change_to(&nested);

        // Should fail (not on track branch)
        let exit = super::execute(super::GitCommand::CommitFromFile(super::CommitFromFileArgs {
            path: PathBuf::from("tmp/track-commit/commit-message.txt"),
            cleanup: true,
            track_dir: None,
        }));
        assert_ne!(exit, ExitCode::SUCCESS);
        assert!(commit_message.exists());
        assert_eq!(run_git_output(dir.path(), &["log", "-1", "--pretty=%s"]).trim(), "initial");
    }

    #[test]
    fn note_from_file_reads_track_commit_scratch_from_nested_directory() {
        let _lock = cwd_lock().lock().unwrap();
        let dir = init_repo();
        fs::write(dir.path().join("tracked.txt"), "base\n").unwrap();
        run_git(dir.path(), &["add", "tracked.txt"]);
        run_git(dir.path(), &["commit", "-m", "initial"]);
        fs::create_dir_all(dir.path().join("tmp/track-commit")).unwrap();
        let note = dir.path().join("tmp/track-commit/note.md");
        fs::write(&note, "note line 1\nnote line 2\n").unwrap();

        let nested = dir.path().join("nested");
        fs::create_dir_all(&nested).unwrap();
        let _guard = CurrentDirGuard::change_to(&nested);

        assert_eq!(
            super::execute(super::GitCommand::NoteFromFile(super::FileArgs {
                path: PathBuf::from("tmp/track-commit/note.md"),
                cleanup: true,
            })),
            ExitCode::SUCCESS
        );
        assert!(!note.exists());
        assert_eq!(
            run_git_output(dir.path(), &["notes", "show", "HEAD"]),
            "note line 1\nnote line 2\n"
        );
    }

    #[test]
    fn unstage_removes_paths_from_index_without_discarding_worktree() {
        let _lock = cwd_lock().lock().unwrap();
        let dir = init_repo();
        fs::write(dir.path().join("a.txt"), "base\n").unwrap();
        fs::write(dir.path().join("b.txt"), "base\n").unwrap();
        run_git(dir.path(), &["add", "a.txt", "b.txt"]);
        run_git(dir.path(), &["commit", "-m", "initial"]);

        fs::write(dir.path().join("a.txt"), "changed\n").unwrap();
        fs::write(dir.path().join("b.txt"), "changed\n").unwrap();
        run_git(dir.path(), &["add", "a.txt", "b.txt"]);

        let _guard = CurrentDirGuard::change_to(dir.path());

        assert_eq!(
            super::execute(super::GitCommand::Unstage(super::UnstageArgs {
                paths: vec![PathBuf::from("a.txt")],
            })),
            ExitCode::SUCCESS
        );

        let staged = run_git_output(dir.path(), &["diff", "--cached", "--name-only"]);
        assert!(!staged.contains("a.txt"), "a.txt should be unstaged");
        assert!(staged.contains("b.txt"), "b.txt should remain staged");

        let worktree = run_git_output(dir.path(), &["diff", "--name-only"]);
        assert!(worktree.contains("a.txt"), "a.txt worktree change should be preserved");
    }

    #[test]
    fn switch_and_pull_uses_repo_root_from_nested_directory() {
        let _lock = cwd_lock().lock().unwrap();
        let dir = init_repo();
        fs::write(dir.path().join("tracked.txt"), "base\n").unwrap();
        run_git(dir.path(), &["add", "tracked.txt"]);
        run_git(dir.path(), &["commit", "-m", "initial"]);
        run_git(dir.path(), &["checkout", "-b", "feature"]);

        let nested = dir.path().join("nested");
        fs::create_dir_all(&nested).unwrap();
        let _guard = CurrentDirGuard::change_to(&nested);

        // No remote/upstream is configured, so `git pull --ff-only` exits non-zero.
        // switch-and-pull must still report success: the branch switch itself
        // succeeded and a failed pull is only a warning. This pins the non-fatal
        // pull behavior so a future change cannot silently make it fatal.
        assert_eq!(
            super::execute(super::GitCommand::SwitchAndPull(super::SwitchAndPullArgs {
                branch: "main".to_owned(),
            })),
            ExitCode::SUCCESS
        );
        assert_eq!(repo().current_branch().unwrap().as_deref(), Some("main"));
    }

    #[test]
    fn load_stage_paths_accepts_unique_repo_relative_paths() {
        // Test the validation through the CLI facade
        let dir = tempfile::tempdir().unwrap();
        let list = dir.path().join("add-paths.txt");
        fs::write(&list, "src/lib.rs\n# comment\nsrc/lib.rs\nREADME.md\n").unwrap();

        // Just verify the file can be read and deduplicated by running add-from-file
        // with a temp git repo
        let _lock = cwd_lock().lock().unwrap();
        let repo_dir = init_repo();
        fs::create_dir_all(repo_dir.path().join("src")).unwrap();
        fs::write(repo_dir.path().join("src/lib.rs"), "fn main() {}\n").unwrap();
        run_git(repo_dir.path(), &["add", "src/lib.rs"]);
        run_git(repo_dir.path(), &["commit", "-m", "init"]);
        fs::write(repo_dir.path().join("src/lib.rs"), "fn main2() {}\n").unwrap();
        fs::write(repo_dir.path().join("README.md"), "readme\n").unwrap();
        let add_paths_file = repo_dir.path().join("add-paths.txt");
        fs::write(&add_paths_file, "src/lib.rs\n# comment\nsrc/lib.rs\nREADME.md\n").unwrap();

        let _guard = CurrentDirGuard::change_to(repo_dir.path());

        let exit = super::execute(super::GitCommand::AddFromFile(super::FileArgs {
            path: PathBuf::from("add-paths.txt"),
            cleanup: false,
        }));
        assert_eq!(exit, ExitCode::SUCCESS);
        let staged = run_git_output(repo_dir.path(), &["diff", "--cached", "--name-only"]);
        assert!(staged.contains("src/lib.rs"));
        assert!(staged.contains("README.md"));
    }

    #[test]
    fn load_stage_paths_rejects_transient_automation_directory() {
        let _lock = cwd_lock().lock().unwrap();
        let dir = init_repo();
        fs::write(dir.path().join("tracked.txt"), "base\n").unwrap();
        run_git(dir.path(), &["add", "tracked.txt"]);
        run_git(dir.path(), &["commit", "-m", "initial"]);

        let list = dir.path().join("bad-paths.txt");
        fs::write(&list, "tmp/track-commit\n").unwrap();

        let _guard = CurrentDirGuard::change_to(dir.path());

        let exit = super::execute(super::GitCommand::AddFromFile(super::FileArgs {
            path: PathBuf::from("bad-paths.txt"),
            cleanup: false,
        }));
        assert_ne!(exit, ExitCode::SUCCESS, "should reject transient automation directory");
    }
}
