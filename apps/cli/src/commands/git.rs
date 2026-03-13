//! CLI subcommands for guarded local git workflow wrappers.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Args, Subcommand};
use infrastructure::git_cli::{GitRepository, SystemGitRepo, resolve_repo_path};
use serde::Deserialize;
use usecase::git_workflow::{
    ExplicitTrackBranch, TRANSIENT_AUTOMATION_DIRS, TRANSIENT_AUTOMATION_FILES, TrackBranchClaim,
    validate_stage_path_entries, verify_auto_detected_branch, verify_explicit_track_branch,
};

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

#[derive(Debug, Deserialize)]
struct BranchMetadata {
    branch: Option<String>,
    status: Option<String>,
}

pub fn execute(cmd: GitCommand) -> ExitCode {
    match cmd {
        GitCommand::AddAll => add_all(),
        GitCommand::AddFromFile(args) => add_from_file(&args.path, args.cleanup),
        GitCommand::CommitFromFile(args) => {
            commit_from_file(&args.path, args.cleanup, args.track_dir.as_deref())
        }
        GitCommand::NoteFromFile(args) => note_from_file(&args.path, args.cleanup),
        GitCommand::SwitchAndPull(args) => switch_and_pull(&args.branch),
    }
}

fn repo() -> Result<SystemGitRepo, String> {
    SystemGitRepo::discover()
}

fn ensure_existing_nonempty_file(path: &Path, label: &str) -> Result<(), String> {
    if !path.is_file() {
        return Err(format!("Missing {label}: {}", path.display()));
    }

    let content = fs::read_to_string(path)
        .map_err(|err| format!("failed to read {label} {}: {err}", path.display()))?;
    if content.trim().is_empty() {
        return Err(format!("{label} is empty: {}", path.display()));
    }
    Ok(())
}

fn load_stage_paths(path: &Path) -> Result<Vec<String>, String> {
    ensure_existing_nonempty_file(path, "stage path list file")?;

    let content = fs::read_to_string(path)
        .map_err(|err| format!("failed to read stage path list {}: {err}", path.display()))?;
    validate_stage_path_entries(content.lines()).map_err(|err| {
        if err == "Stage path list file has no usable entries" {
            format!("{err}: {}", path.display())
        } else {
            err
        }
    })
}

fn add_all() -> ExitCode {
    let repo = match repo() {
        Ok(repo) => repo,
        Err(err) => {
            eprintln!("[ERROR] {err}");
            return ExitCode::FAILURE;
        }
    };

    let mut owned_args = vec!["add".to_owned(), "-A".to_owned(), "--".to_owned(), ".".to_owned()];
    owned_args.extend(TRANSIENT_AUTOMATION_FILES.iter().map(|path| format!(":(exclude){path}")));
    owned_args.extend(TRANSIENT_AUTOMATION_DIRS.iter().map(|path| format!(":(exclude){path}")));
    let args: Vec<&str> = owned_args.iter().map(String::as_str).collect();

    match repo.status(&args) {
        Ok(0) => ExitCode::SUCCESS,
        Ok(_) => ExitCode::FAILURE,
        Err(err) => {
            eprintln!("[ERROR] {err}");
            ExitCode::FAILURE
        }
    }
}

fn add_from_file(path: &Path, cleanup: bool) -> ExitCode {
    let repo = match repo() {
        Ok(repo) => repo,
        Err(err) => {
            eprintln!("[ERROR] {err}");
            return ExitCode::FAILURE;
        }
    };
    let path = repo.resolve_path(path);
    let stage_paths = match load_stage_paths(&path) {
        Ok(paths) => paths,
        Err(err) => {
            eprintln!("[ERROR] {err}");
            return ExitCode::FAILURE;
        }
    };

    let mut owned_args = vec!["add".to_owned(), "--".to_owned()];
    owned_args.extend(stage_paths);
    let args: Vec<&str> = owned_args.iter().map(String::as_str).collect();
    match repo.status(&args) {
        Ok(0) => {
            if cleanup {
                let _ = fs::remove_file(path);
            }
            ExitCode::SUCCESS
        }
        Ok(_) => ExitCode::FAILURE,
        Err(err) => {
            eprintln!("[ERROR] {err}");
            ExitCode::FAILURE
        }
    }
}

fn commit_from_file(path: &Path, cleanup: bool, track_dir: Option<&Path>) -> ExitCode {
    let repo = match repo() {
        Ok(repo) => repo,
        Err(err) => {
            eprintln!("[ERROR] {err}");
            return ExitCode::FAILURE;
        }
    };
    let path = repo.resolve_path(path);

    if let Err(err) = ensure_existing_nonempty_file(&path, "commit message file") {
        eprintln!("[ERROR] {err}");
        return ExitCode::FAILURE;
    }

    let track_dir_file = if track_dir.is_none() {
        path.parent().map(|parent| parent.join("track-dir.txt"))
    } else {
        None
    };
    let effective_track_dir = track_dir
        .map(|track_dir| repo.resolve_path(track_dir))
        .or_else(|| load_optional_track_dir(repo.root(), track_dir_file.as_deref()));

    let guard_result = if let Some(track_dir) = effective_track_dir.as_deref() {
        verify_commit_branch(repo.root(), track_dir, &repo)
    } else {
        verify_branch_by_auto_detection(&repo)
    };
    if let Err(err) = guard_result {
        eprintln!("[ERROR] Branch guard: {err}");
        if cleanup {
            if let Some(track_dir_file) = track_dir_file.as_deref() {
                let _ = fs::remove_file(track_dir_file);
            }
        }
        return ExitCode::FAILURE;
    }

    let path_str = path.to_string_lossy().into_owned();
    match repo.status(&["commit", "-F", path_str.as_str()]) {
        Ok(0) => {
            if cleanup {
                let _ = fs::remove_file(path);
                if let Some(track_dir_file) = track_dir_file.as_deref() {
                    let _ = fs::remove_file(track_dir_file);
                }
            }
            ExitCode::SUCCESS
        }
        Ok(_) => {
            if cleanup {
                if let Some(track_dir_file) = track_dir_file.as_deref() {
                    let _ = fs::remove_file(track_dir_file);
                }
            }
            ExitCode::FAILURE
        }
        Err(err) => {
            eprintln!("[ERROR] {err}");
            ExitCode::FAILURE
        }
    }
}

fn note_from_file(path: &Path, cleanup: bool) -> ExitCode {
    let repo = match repo() {
        Ok(repo) => repo,
        Err(err) => {
            eprintln!("[ERROR] {err}");
            return ExitCode::FAILURE;
        }
    };
    let path = repo.resolve_path(path);
    if let Err(err) = ensure_existing_nonempty_file(&path, "git note file") {
        eprintln!("[ERROR] {err}");
        return ExitCode::FAILURE;
    }
    let path_str = path.to_string_lossy().into_owned();
    match repo.status(&["notes", "add", "-f", "-F", path_str.as_str(), "HEAD"]) {
        Ok(0) => {
            if cleanup {
                let _ = fs::remove_file(path);
            }
            ExitCode::SUCCESS
        }
        Ok(_) => ExitCode::FAILURE,
        Err(err) => {
            eprintln!("[ERROR] {err}");
            ExitCode::FAILURE
        }
    }
}

fn switch_and_pull(branch: &str) -> ExitCode {
    let repo = match repo() {
        Ok(repo) => repo,
        Err(err) => {
            eprintln!("[ERROR] {err}");
            return ExitCode::FAILURE;
        }
    };

    println!("Switching to {branch}...");
    match repo.status(&["checkout", branch]) {
        Ok(0) => {}
        Ok(code) => {
            eprintln!("[ERROR] Failed to checkout {branch}");
            return ExitCode::from(code as u8);
        }
        Err(err) => {
            eprintln!("[ERROR] {err}");
            return ExitCode::FAILURE;
        }
    }

    println!("Pulling latest from origin/{branch}...");
    match repo.status(&["pull", "--ff-only"]) {
        Ok(0) => {
            println!("[OK] On {branch}, up to date.");
            ExitCode::SUCCESS
        }
        Ok(_) => {
            println!("[WARN] Pull failed (may not have remote tracking branch)");
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("[ERROR] {err}");
            ExitCode::FAILURE
        }
    }
}

fn load_optional_track_dir(root: &Path, path: Option<&Path>) -> Option<PathBuf> {
    let path = path?;
    let raw = fs::read_to_string(path).ok()?;
    let trimmed = raw.trim();
    if trimmed.is_empty() { None } else { Some(resolve_repo_path(root, Path::new(trimmed))) }
}

fn verify_commit_branch(
    root: &Path,
    track_dir: &Path,
    repo: &impl GitRepository,
) -> Result<(), String> {
    if !track_dir.is_dir() {
        return Err(format!("Track directory not found: {}", track_dir.display()));
    }

    let repo_items_dir = root.join("track/items").canonicalize().map_err(|err| {
        format!("track/items/ resolves outside the repository root or is unavailable: {err}")
    })?;
    let resolved = track_dir.canonicalize().map_err(|err| {
        format!("failed to resolve track directory {}: {err}", track_dir.display())
    })?;
    if resolved.parent() != Some(repo_items_dir.as_path()) {
        return Err(format!(
            "Track directory must be exactly track/items/<id>: {}",
            track_dir.display()
        ));
    }

    let metadata = read_metadata(&track_dir.join("metadata.json"))?;
    verify_explicit_track_branch(
        repo.current_branch()?.as_deref(),
        &ExplicitTrackBranch {
            display_path: track_dir.display().to_string(),
            expected_branch: metadata.branch,
        },
    )
}

fn verify_branch_by_auto_detection(repo: &impl GitRepository) -> Result<(), String> {
    let items_root = repo.root().join("track/items");
    let archive_root = repo.root().join("track/archive");
    let mut claims = Vec::new();
    if items_root.is_dir() {
        for entry in read_directories(&items_root)? {
            let metadata_path = entry.join("metadata.json");
            if !metadata_path.is_file() {
                continue;
            }
            if let Ok(metadata) = read_metadata(&metadata_path) {
                claims.push(TrackBranchClaim {
                    track_name: entry
                        .file_name()
                        .and_then(|name| name.to_str())
                        .unwrap_or_default()
                        .to_owned(),
                    branch: metadata.branch,
                    status: metadata.status,
                });
            }
        }
    }
    if archive_root.is_dir() {
        for entry in read_directories(&archive_root)? {
            let metadata_path = entry.join("metadata.json");
            if !metadata_path.is_file() {
                continue;
            }
            if let Ok(metadata) = read_metadata(&metadata_path) {
                claims.push(TrackBranchClaim {
                    track_name: entry
                        .file_name()
                        .and_then(|name| name.to_str())
                        .unwrap_or_default()
                        .to_owned(),
                    branch: metadata.branch,
                    status: metadata.status,
                });
            }
        }
    }

    verify_auto_detected_branch(repo.current_branch()?.as_deref(), &claims)
}

fn read_directories(root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut dirs = Vec::new();
    for entry in fs::read_dir(root)
        .map_err(|err| format!("failed to read directory {}: {err}", root.display()))?
    {
        let entry = entry.map_err(|err| format!("failed to read directory entry: {err}"))?;
        if entry.path().is_dir() {
            dirs.push(entry.path());
        }
    }
    dirs.sort();
    Ok(dirs)
}

fn read_metadata(path: &Path) -> Result<BranchMetadata, String> {
    let content = fs::read_to_string(path).map_err(|err| {
        format!("Cannot read or parse metadata.json in {}: {err}", path.display())
    })?;
    serde_json::from_str(&content)
        .map_err(|err| format!("Cannot read or parse metadata.json in {}: {err}", path.display()))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::{
        add_all, add_from_file, commit_from_file, load_optional_track_dir, load_stage_paths,
        note_from_file, repo, switch_and_pull,
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

        assert!(err.contains("transient automation"));
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
            resolve_repo_path(root, Path::new(".takt/pending-note.md")),
            PathBuf::from("/repo/root/.takt/pending-note.md")
        );
        assert_eq!(
            resolve_repo_path(root, Path::new("/tmp/note.md")),
            PathBuf::from("/tmp/note.md")
        );
    }

    #[test]
    fn load_optional_track_dir_anchors_relative_paths_at_repo_root() {
        let dir = tempfile::tempdir().unwrap();
        let track_dir_file = dir.path().join("track-dir.txt");
        fs::write(&track_dir_file, "track/items/example\n").unwrap();

        let resolved = load_optional_track_dir(dir.path(), Some(&track_dir_file)).unwrap();

        assert_eq!(resolved, dir.path().join("track/items/example"));
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
        fs::create_dir_all(dir.path().join(".takt")).unwrap();
        fs::write(dir.path().join(".takt/pending-note.md"), "note\n").unwrap();
        fs::create_dir_all(dir.path().join("tmp/track-commit")).unwrap();
        fs::write(dir.path().join("tmp/track-commit/commit-message.txt"), "message\n").unwrap();

        let nested = dir.path().join("nested");
        fs::create_dir_all(&nested).unwrap();
        let _guard = CurrentDirGuard::change_to(&nested);

        assert_eq!(add_all(), ExitCode::SUCCESS);
        assert_eq!(
            run_git_output(dir.path(), &["diff", "--cached", "--name-only"])
                .lines()
                .collect::<Vec<_>>(),
            vec!["new.txt", "tracked.txt"]
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
            add_from_file(Path::new("tmp/track-commit/add-paths.txt"), true),
            ExitCode::SUCCESS
        );
        assert!(!add_paths.exists());
        assert_eq!(
            run_git_output(dir.path(), &["diff", "--cached", "--name-only"]).trim(),
            "tracked.txt"
        );
    }

    #[test]
    fn commit_from_file_resolves_relative_track_dir_file_from_nested_directory() {
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
        let track_dir_file = scratch.join("track-dir.txt");
        fs::write(&commit_message, "Track commit\n").unwrap();
        fs::write(&track_dir_file, "track/items/example\n").unwrap();

        let nested = dir.path().join("nested");
        fs::create_dir_all(&nested).unwrap();
        let _guard = CurrentDirGuard::change_to(&nested);

        assert_eq!(
            commit_from_file(Path::new("tmp/track-commit/commit-message.txt"), true, None),
            ExitCode::SUCCESS
        );
        assert!(!commit_message.exists());
        assert!(!track_dir_file.exists());
        assert_eq!(repo().unwrap().current_branch().unwrap().as_deref(), Some("track/example"));
        assert_eq!(
            run_git_output(dir.path(), &["log", "-1", "--pretty=%s"]).trim(),
            "Track commit"
        );
    }

    #[test]
    fn note_from_file_reads_repo_relative_path_from_nested_directory() {
        let _lock = cwd_lock().lock().unwrap();
        let dir = init_repo();
        fs::write(dir.path().join("tracked.txt"), "base\n").unwrap();
        run_git(dir.path(), &["add", "tracked.txt"]);
        run_git(dir.path(), &["commit", "-m", "initial"]);
        fs::create_dir_all(dir.path().join(".takt")).unwrap();
        let note = dir.path().join(".takt/pending-note.md");
        fs::write(&note, "note line 1\nnote line 2\n").unwrap();

        let nested = dir.path().join("nested");
        fs::create_dir_all(&nested).unwrap();
        let _guard = CurrentDirGuard::change_to(&nested);

        assert_eq!(note_from_file(Path::new(".takt/pending-note.md"), true), ExitCode::SUCCESS);
        assert!(!note.exists());
        assert_eq!(
            run_git_output(dir.path(), &["notes", "show", "HEAD"]),
            "note line 1\nnote line 2\n"
        );
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

        assert_eq!(switch_and_pull("main"), ExitCode::SUCCESS);
        assert_eq!(repo().unwrap().current_branch().unwrap().as_deref(), Some("main"));
    }
}
