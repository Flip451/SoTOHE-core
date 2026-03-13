//! CLI subcommands for guarded local git workflow wrappers.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use clap::{Args, Subcommand};
use serde::Deserialize;

const TRANSIENT_AUTOMATION_FILES: &[&str] = &[
    ".takt/pending-add-paths.txt",
    ".takt/pending-note.md",
    ".takt/pending-commit-message.txt",
    "tmp/track-commit/add-paths.txt",
    "tmp/track-commit/commit-message.txt",
    "tmp/track-commit/note.md",
    "tmp/track-commit/track-dir.txt",
];
const TRANSIENT_AUTOMATION_DIRS: &[&str] = &[".takt/handoffs", "tmp"];
const GLOB_MAGIC_CHARS: &[char] = &['*', '?', '[', ']'];

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

fn repo_root() -> Result<PathBuf, String> {
    let cwd = std::env::current_dir()
        .map_err(|err| format!("failed to determine current directory: {err}"))?;
    let output = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .current_dir(&cwd)
        .output()
        .map_err(|err| format!("failed to run git rev-parse --show-toplevel: {err}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        return Err(if stderr.is_empty() {
            "failed to resolve repository root".to_owned()
        } else {
            format!("failed to resolve repository root: {stderr}")
        });
    }

    let root = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    if root.is_empty() {
        return Err("git rev-parse --show-toplevel returned an empty path".to_owned());
    }
    Ok(PathBuf::from(root))
}

fn resolve_repo_path(root: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() { path.to_path_buf() } else { root.join(path) }
}

fn run_git(args: &[&str], root: &Path) -> Result<i32, String> {
    let status = Command::new("git")
        .args(args)
        .current_dir(root)
        .status()
        .map_err(|err| format!("failed to run git {}: {err}", args.join(" ")))?;
    Ok(status.code().unwrap_or(1))
}

fn run_git_output(args: &[&str], root: &Path) -> Result<std::process::Output, String> {
    Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .map_err(|err| format!("failed to run git {}: {err}", args.join(" ")))
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

    let transient_paths: HashSet<PathBuf> =
        TRANSIENT_AUTOMATION_FILES.iter().map(PathBuf::from).collect();
    let transient_dirs: HashSet<PathBuf> =
        TRANSIENT_AUTOMATION_DIRS.iter().map(PathBuf::from).collect();
    let mut stage_paths = Vec::new();
    let mut seen = HashSet::new();

    let content = fs::read_to_string(path)
        .map_err(|err| format!("failed to read stage path list {}: {err}", path.display()))?;
    for raw_line in content.lines() {
        let entry = raw_line.trim();
        if entry.is_empty() || entry.starts_with('#') || !seen.insert(entry.to_owned()) {
            continue;
        }

        let entry_path = PathBuf::from(entry);
        if entry_path.is_absolute() {
            return Err(format!("Stage path list must use repo-relative paths: {entry}"));
        }
        if entry_path
            .components()
            .any(|component| matches!(component, std::path::Component::ParentDir))
        {
            return Err(format!("Stage path list cannot escape the repo root: {entry}"));
        }
        if matches!(entry, "." | "./") {
            return Err(format!("Stage path list cannot use whole-worktree pathspecs: {entry}"));
        }
        if entry.starts_with(':') {
            return Err(format!(
                "Stage path list cannot use git pathspec magic or shorthand: {entry}"
            ));
        }
        if entry.chars().any(|ch| GLOB_MAGIC_CHARS.contains(&ch)) {
            return Err(format!("Stage path list cannot use glob patterns: {entry}"));
        }
        if transient_paths
            .iter()
            .any(|transient| entry_path == *transient || transient.starts_with(&entry_path))
        {
            return Err(format!(
                "Stage path list cannot include transient automation files or their parent directories: {entry}"
            ));
        }
        if transient_dirs.iter().any(|transient_dir| {
            entry_path == *transient_dir
                || entry_path.starts_with(transient_dir)
                || transient_dir.starts_with(&entry_path)
        }) {
            return Err(format!(
                "Stage path list cannot include transient automation directories or their contents: {entry}"
            ));
        }

        stage_paths.push(entry.to_owned());
    }

    if stage_paths.is_empty() {
        return Err(format!("Stage path list file has no usable entries: {}", path.display()));
    }

    Ok(stage_paths)
}

fn add_all() -> ExitCode {
    let root = match repo_root() {
        Ok(root) => root,
        Err(err) => {
            eprintln!("[ERROR] {err}");
            return ExitCode::FAILURE;
        }
    };

    let mut owned_args = vec!["add".to_owned(), "-A".to_owned(), "--".to_owned(), ".".to_owned()];
    owned_args.extend(TRANSIENT_AUTOMATION_FILES.iter().map(|path| format!(":(exclude){path}")));
    owned_args.extend(TRANSIENT_AUTOMATION_DIRS.iter().map(|path| format!(":(exclude){path}")));
    let args: Vec<&str> = owned_args.iter().map(String::as_str).collect();

    match run_git(&args, &root) {
        Ok(0) => ExitCode::SUCCESS,
        Ok(_) => ExitCode::FAILURE,
        Err(err) => {
            eprintln!("[ERROR] {err}");
            ExitCode::FAILURE
        }
    }
}

fn add_from_file(path: &Path, cleanup: bool) -> ExitCode {
    let root = match repo_root() {
        Ok(root) => root,
        Err(err) => {
            eprintln!("[ERROR] {err}");
            return ExitCode::FAILURE;
        }
    };
    let path = resolve_repo_path(&root, path);
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
    match run_git(&args, &root) {
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
    let root = match repo_root() {
        Ok(root) => root,
        Err(err) => {
            eprintln!("[ERROR] {err}");
            return ExitCode::FAILURE;
        }
    };
    let path = resolve_repo_path(&root, path);

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
        .map(|track_dir| resolve_repo_path(&root, track_dir))
        .or_else(|| load_optional_track_dir(&root, track_dir_file.as_deref()));

    let guard_result = if let Some(track_dir) = effective_track_dir.as_deref() {
        verify_commit_branch(&root, track_dir)
    } else {
        verify_branch_by_auto_detection(&root)
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
    match run_git(&["commit", "-F", path_str.as_str()], &root) {
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
    let root = match repo_root() {
        Ok(root) => root,
        Err(err) => {
            eprintln!("[ERROR] {err}");
            return ExitCode::FAILURE;
        }
    };
    let path = resolve_repo_path(&root, path);
    if let Err(err) = ensure_existing_nonempty_file(&path, "git note file") {
        eprintln!("[ERROR] {err}");
        return ExitCode::FAILURE;
    }
    let path_str = path.to_string_lossy().into_owned();
    match run_git(&["notes", "add", "-f", "-F", path_str.as_str(), "HEAD"], &root) {
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
    let root = match repo_root() {
        Ok(root) => root,
        Err(err) => {
            eprintln!("[ERROR] {err}");
            return ExitCode::FAILURE;
        }
    };

    println!("Switching to {branch}...");
    match run_git(&["checkout", branch], &root) {
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
    match run_git(&["pull", "--ff-only"], &root) {
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

fn current_git_branch(root: &Path) -> Result<Option<String>, String> {
    let output = run_git_output(&["rev-parse", "--abbrev-ref", "HEAD"], root)?;
    if !output.status.success() {
        return Ok(None);
    }
    Ok(Some(String::from_utf8_lossy(&output.stdout).trim().to_owned()))
}

fn verify_commit_branch(root: &Path, track_dir: &Path) -> Result<(), String> {
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
    let Some(expected_branch) = metadata.branch else {
        return Ok(());
    };

    match current_git_branch(root)? {
        None => Err(format!("Cannot determine current git branch — expected '{expected_branch}'")),
        Some(branch) if branch == "HEAD" => {
            Err(format!("Detached HEAD — expected branch '{expected_branch}', cannot verify"))
        }
        Some(branch) if branch != expected_branch => {
            Err(format!("Current branch '{branch}' does not match expected '{expected_branch}'"))
        }
        Some(_) => Ok(()),
    }
}

fn verify_branch_by_auto_detection(root: &Path) -> Result<(), String> {
    let branch = match current_git_branch(root)? {
        Some(branch) => branch,
        None => return Err("cannot determine current git branch".to_owned()),
    };
    if branch == "HEAD" {
        return Err("detached HEAD — cannot verify track branch".to_owned());
    }
    if !branch.starts_with("track/") {
        return Ok(());
    }

    let items_root = root.join("track/items");
    let archive_root = root.join("track/archive");

    let mut matches = Vec::new();
    if items_root.is_dir() {
        for entry in read_directories(&items_root)? {
            let metadata_path = entry.join("metadata.json");
            if !metadata_path.is_file() {
                continue;
            }
            if let Ok(metadata) = read_metadata(&metadata_path) {
                if metadata.branch.as_deref() == Some(branch.as_str()) {
                    matches.push(entry);
                }
            }
        }
    }

    if matches.is_empty() {
        if archive_root.is_dir() {
            for entry in read_directories(&archive_root)? {
                let metadata_path = entry.join("metadata.json");
                if !metadata_path.is_file() {
                    continue;
                }
                if let Ok(metadata) = read_metadata(&metadata_path) {
                    if metadata.branch.as_deref() == Some(branch.as_str())
                        && metadata.status.as_deref() == Some("archived")
                    {
                        return Ok(());
                    }
                }
            }
        }

        let slug = branch.trim_start_matches("track/");
        let fallback_dir = items_root.join(slug);
        let fallback_metadata = fallback_dir.join("metadata.json");
        if fallback_metadata.is_file() {
            let metadata = read_metadata(&fallback_metadata)?;
            if metadata.branch.is_none() {
                return Ok(());
            }
        }

        return Err(format!(
            "on branch '{branch}' but no track claims this branch in metadata.json"
        ));
    }

    if matches.len() > 1 {
        let names = matches
            .iter()
            .filter_map(|path| path.file_name().and_then(|name| name.to_str()))
            .collect::<Vec<_>>()
            .join(", ");
        return Err(format!("multiple tracks claim branch '{branch}': {names}"));
    }

    match matches.first() {
        Some(track_dir) => verify_commit_branch(root, track_dir),
        None => Err("internal error: expected exactly one branch match".to_owned()),
    }
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
        add_all, add_from_file, commit_from_file, current_git_branch, load_optional_track_dir,
        load_stage_paths, note_from_file, repo_root, resolve_repo_path, switch_and_pull,
    };
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

        assert_eq!(repo_root().unwrap(), dir.path());
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
        assert_eq!(current_git_branch(dir.path()).unwrap().as_deref(), Some("track/example"));
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
        assert_eq!(current_git_branch(dir.path()).unwrap().as_deref(), Some("main"));
    }
}
