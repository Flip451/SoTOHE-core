//! CLI subcommands for guarded local git workflow wrappers.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Args, Subcommand};
use infrastructure::git_cli::{
    GitRepository, SystemGitRepo, collect_track_branch_claims, load_explicit_track_branch,
};
use usecase::git_workflow::{
    ExplicitTrackBranch, TRANSIENT_AUTOMATION_DIRS, TRANSIENT_AUTOMATION_FILES, TrackBranchClaim,
    validate_stage_path_entries, verify_auto_detected_branch, verify_explicit_track_branch,
};

use crate::CliError;

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
    match cmd {
        GitCommand::AddAll => match add_all() {
            Ok(code) => code,
            Err(err) => {
                eprintln!("{err}");
                err.exit_code()
            }
        },
        GitCommand::AddFromFile(args) => match add_from_file(&args.path, args.cleanup) {
            Ok(code) => code,
            Err(err) => {
                eprintln!("{err}");
                err.exit_code()
            }
        },
        GitCommand::CommitFromFile(args) => {
            match commit_from_file(&args.path, args.cleanup, args.track_dir.as_deref()) {
                Ok(code) => code,
                Err(err) => {
                    eprintln!("{err}");
                    err.exit_code()
                }
            }
        }
        GitCommand::NoteFromFile(args) => match note_from_file(&args.path, args.cleanup) {
            Ok(code) => code,
            Err(err) => {
                eprintln!("{err}");
                err.exit_code()
            }
        },
        GitCommand::SwitchAndPull(args) => match switch_and_pull(&args.branch) {
            Ok(code) => code,
            Err(err) => {
                eprintln!("{err}");
                err.exit_code()
            }
        },
        GitCommand::Unstage(args) => match unstage(&args.paths) {
            Ok(code) => code,
            Err(err) => {
                eprintln!("{err}");
                err.exit_code()
            }
        },
    }
}

fn repo() -> Result<SystemGitRepo, CliError> {
    SystemGitRepo::discover().map_err(CliError::from)
}

fn ensure_existing_nonempty_file(path: &Path, label: &str) -> Result<(), CliError> {
    if !path.is_file() {
        return Err(CliError::Message(format!("Missing {label}: {}", path.display())));
    }

    let content = fs::read_to_string(path).map_err(|err| {
        CliError::Message(format!("failed to read {label} {}: {err}", path.display()))
    })?;
    if content.trim().is_empty() {
        return Err(CliError::Message(format!("{label} is empty: {}", path.display())));
    }
    Ok(())
}

fn load_stage_paths(path: &Path) -> Result<Vec<String>, CliError> {
    ensure_existing_nonempty_file(path, "stage path list file")?;

    let content = fs::read_to_string(path).map_err(|err| {
        CliError::Message(format!("failed to read stage path list {}: {err}", path.display()))
    })?;
    validate_stage_path_entries(content.lines()).map_err(|err| {
        let msg = err.to_string();
        if msg == "Stage path list file has no usable entries" {
            CliError::Message(format!("{msg}: {}", path.display()))
        } else {
            CliError::Message(msg)
        }
    })
}

fn unstage(paths: &[PathBuf]) -> Result<ExitCode, CliError> {
    let repo = repo()?;
    let mut args = vec!["restore", "--staged", "--"];
    let path_strs: Vec<String> = paths.iter().map(|p| p.display().to_string()).collect();
    args.extend(path_strs.iter().map(String::as_str));
    let code = repo.status(&args)?;
    Ok(ExitCode::from(u8::try_from(code).unwrap_or(1)))
}

fn add_all() -> Result<ExitCode, CliError> {
    let repo = repo()?;
    repo.stage_all_excluding(TRANSIENT_AUTOMATION_FILES, TRANSIENT_AUTOMATION_DIRS)?;
    Ok(ExitCode::SUCCESS)
}

fn add_from_file(path: &Path, cleanup: bool) -> Result<ExitCode, CliError> {
    let repo = repo()?;
    let path = repo.resolve_path(path);
    let stage_paths = load_stage_paths(&path)?;

    let mut owned_args = vec!["add".to_owned(), "--".to_owned()];
    owned_args.extend(stage_paths);
    let args: Vec<&str> = owned_args.iter().map(String::as_str).collect();
    match repo.status(&args)? {
        0 => {
            if cleanup {
                let _ = fs::remove_file(path);
            }
            Ok(ExitCode::SUCCESS)
        }
        _ => Ok(ExitCode::FAILURE),
    }
}

fn commit_from_file(
    path: &Path,
    cleanup: bool,
    track_dir: Option<&Path>,
) -> Result<ExitCode, CliError> {
    let repo = repo()?;
    let path = repo.resolve_path(path);

    ensure_existing_nonempty_file(&path, "commit message file")?;

    let explicit_track =
        track_dir.map(|td| load_explicit_track(repo.root(), &repo.resolve_path(td))).transpose()?;

    // Fail-closed: non-track-branch commits are always rejected, regardless of whether
    // an explicit track selector is provided.  The explicit --track-dir flag identifies
    // which track to verify against, but does not bypass the branch-type gate.
    match repo.current_branch()?.as_deref() {
        Some(branch) if branch.starts_with("track/") => {}
        Some("HEAD") => {
            return Err(CliError::Message(
                "detached HEAD: switch to a track branch before committing".to_owned(),
            ));
        }
        Some(_) => {
            return Err(CliError::Message(
                "non-track branch: switch to a track branch before committing".to_owned(),
            ));
        }
        None => {
            return Err(CliError::Message(
                "cannot determine current git branch; switch to a track branch before committing"
                    .to_owned(),
            ));
        }
    }

    let guard_result = if let Some(explicit_track) = explicit_track.as_ref() {
        verify_commit_branch(&repo, explicit_track)
    } else {
        verify_branch_by_auto_detection(&repo)
    };
    if let Err(err) = guard_result {
        // Do NOT remove the commit message file on guard failure: the commit did not
        // proceed and the file must remain available for retry after the user fixes
        // the branch or track selector.
        return Err(CliError::Message(format!("Branch guard: {err}")));
    }

    let path_str = path.to_string_lossy().into_owned();
    match repo.status(&["commit", "-F", path_str.as_str()])? {
        0 => {
            if cleanup {
                let _ = fs::remove_file(path);
            }
            Ok(ExitCode::SUCCESS)
        }
        _ => Ok(ExitCode::FAILURE),
    }
}

fn note_from_file(path: &Path, cleanup: bool) -> Result<ExitCode, CliError> {
    let repo = repo()?;
    let path = repo.resolve_path(path);
    ensure_existing_nonempty_file(&path, "git note file")?;
    let path_str = path.to_string_lossy().into_owned();
    match repo.status(&["notes", "add", "-f", "-F", path_str.as_str(), "HEAD"])? {
        0 => {
            if cleanup {
                let _ = fs::remove_file(path);
            }
            Ok(ExitCode::SUCCESS)
        }
        _ => Ok(ExitCode::FAILURE),
    }
}

fn switch_and_pull(branch: &str) -> Result<ExitCode, CliError> {
    let repo = repo()?;

    println!("Switching to {branch}...");
    match repo.status(&["checkout", branch])? {
        0 => {}
        code => {
            eprintln!("Failed to checkout {branch}");
            return Ok(ExitCode::from(code as u8));
        }
    }

    println!("Pulling latest from origin/{branch}...");
    match repo.status(&["pull", "--ff-only"])? {
        0 => {
            println!("[OK] On {branch}, up to date.");
            Ok(ExitCode::SUCCESS)
        }
        _ => {
            println!("[WARN] Pull failed (may not have remote tracking branch)");
            Ok(ExitCode::SUCCESS)
        }
    }
}

fn load_explicit_track(root: &Path, track_dir: &Path) -> Result<ExplicitTrackBranch, CliError> {
    let metadata = load_explicit_track_branch(root, track_dir).map_err(CliError::Message)?;
    Ok(ExplicitTrackBranch {
        display_path: metadata.display_path,
        expected_branch: metadata.branch,
        status: metadata.status,
    })
}

fn verify_commit_branch(
    repo: &impl GitRepository,
    explicit_track: &ExplicitTrackBranch,
) -> Result<(), CliError> {
    verify_explicit_track_branch(repo.current_branch()?.as_deref(), explicit_track)
        .map_err(CliError::from)
}

fn verify_branch_by_auto_detection(repo: &impl GitRepository) -> Result<(), CliError> {
    let claims = collect_track_branch_claims(repo.root())
        .map_err(CliError::Message)?
        .into_iter()
        .map(|claim| TrackBranchClaim {
            track_name: claim.track_name,
            branch: claim.branch,
            status: claim.status,
        })
        .collect::<Vec<_>>();

    verify_auto_detected_branch(repo.current_branch()?.as_deref(), &claims).map_err(CliError::from)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::{
        add_all, add_from_file, commit_from_file, load_stage_paths, note_from_file, repo,
        switch_and_pull, unstage,
    };
    use infrastructure::git_cli::{GitRepository, resolve_repo_path};
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::{Command, ExitCode};
    use std::sync::{Mutex, OnceLock};

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
    fn load_stage_paths_accepts_unique_repo_relative_paths() {
        let dir = tempfile::tempdir().unwrap();
        let list = dir.path().join("add-paths.txt");
        fs::write(&list, "src/lib.rs\n# comment\nsrc/lib.rs\nREADME.md\n").unwrap();

        let paths = load_stage_paths(&list).unwrap();

        assert_eq!(paths, vec!["src/lib.rs".to_owned(), "README.md".to_owned()]);
    }

    #[test]
    fn load_stage_paths_rejects_transient_automation_directory() {
        let dir = tempfile::tempdir().unwrap();
        let list = dir.path().join("add-paths.txt");
        fs::write(&list, "tmp/track-commit\n").unwrap();

        let err = load_stage_paths(&list).unwrap_err();

        assert!(err.to_string().contains("transient automation"));
    }

    #[test]
    fn repo_root_resolves_git_toplevel_from_nested_directory() {
        let _lock = cwd_lock().lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        run_git(dir.path(), &["init"]);
        let nested = dir.path().join("nested/deeper");
        fs::create_dir_all(&nested).unwrap();

        let _guard = CurrentDirGuard::change_to(&nested);

        let repo = repo().unwrap();

        assert_eq!(repo.root(), dir.path());
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

        assert_eq!(add_all().unwrap(), ExitCode::SUCCESS);
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

        // Create .gitignore that ignores tmp/
        fs::write(dir.path().join(".gitignore"), "tmp/\n").unwrap();
        // Create gitignored files inside tmp/
        fs::create_dir_all(dir.path().join("tmp/track-commit")).unwrap();
        fs::write(dir.path().join("tmp/track-commit/commit-message.txt"), "message\n").unwrap();
        // Also create a trackable change
        fs::write(dir.path().join("tracked.txt"), "changed\n").unwrap();

        let _guard = CurrentDirGuard::change_to(dir.path());

        // add_all must succeed despite gitignored tmp/ overlapping with exclude patterns
        assert_eq!(add_all().unwrap(), ExitCode::SUCCESS);

        // Verify that tracked changes were actually staged
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

        // Gitignore a specific transient file (not the whole dir)
        fs::write(dir.path().join(".gitignore"), "tmp/track-commit/commit-message.txt\n").unwrap();
        fs::create_dir_all(dir.path().join("tmp/track-commit")).unwrap();
        fs::write(dir.path().join("tmp/track-commit/commit-message.txt"), "message\n").unwrap();
        fs::write(dir.path().join("tracked.txt"), "changed\n").unwrap();

        let _guard = CurrentDirGuard::change_to(dir.path());

        assert_eq!(add_all().unwrap(), ExitCode::SUCCESS);

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
            add_from_file(Path::new("tmp/track-commit/add-paths.txt"), true).unwrap(),
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
        // Regression: commit_from_file must accept --track-dir (explicit path) from a nested
        // working directory and commit successfully on the matching track branch.
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

        // Pass --track-dir explicitly (the path is resolved relative to repo root).
        assert_eq!(
            commit_from_file(
                Path::new("tmp/track-commit/commit-message.txt"),
                true,
                Some(Path::new("track/items/example")),
            )
            .unwrap(),
            ExitCode::SUCCESS
        );
        assert!(!commit_message.exists());
        assert_eq!(repo().unwrap().current_branch().unwrap().as_deref(), Some("track/example"));
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

        assert!(
            commit_from_file(Path::new("tmp/track-commit/commit-message.txt"), true, None).is_err()
        );
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
            note_from_file(Path::new("tmp/track-commit/note.md"), true).unwrap(),
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

        assert_eq!(unstage(&[PathBuf::from("a.txt")]).unwrap(), ExitCode::SUCCESS);

        // a.txt should be unstaged (not in index diff) but still modified in worktree
        let staged = run_git_output(dir.path(), &["diff", "--cached", "--name-only"]);
        assert!(!staged.contains("a.txt"), "a.txt should be unstaged");
        assert!(staged.contains("b.txt"), "b.txt should remain staged");

        // worktree change preserved
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

        assert_eq!(switch_and_pull("main").unwrap(), ExitCode::SUCCESS);
        assert_eq!(repo().unwrap().current_branch().unwrap().as_deref(), Some("main"));
    }
}
