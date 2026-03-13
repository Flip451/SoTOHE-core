use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde::Deserialize;

pub trait GitRepository {
    fn root(&self) -> &Path;
    fn status(&self, args: &[&str]) -> Result<i32, String>;
    fn output(&self, args: &[&str]) -> Result<Output, String>;

    fn resolve_path(&self, path: &Path) -> PathBuf {
        resolve_repo_path(self.root(), path)
    }

    fn current_branch(&self) -> Result<Option<String>, String> {
        let output = self.output(&["rev-parse", "--abbrev-ref", "HEAD"])?;
        if !output.status.success() {
            return Ok(None);
        }
        Ok(Some(String::from_utf8_lossy(&output.stdout).trim().to_owned()))
    }
}

#[derive(Debug, Clone)]
pub struct SystemGitRepo {
    root: PathBuf,
}

impl SystemGitRepo {
    pub fn discover() -> Result<Self, String> {
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

        Ok(Self { root: PathBuf::from(root) })
    }
}

impl GitRepository for SystemGitRepo {
    fn root(&self) -> &Path {
        &self.root
    }

    fn status(&self, args: &[&str]) -> Result<i32, String> {
        let status = Command::new("git")
            .args(args)
            .current_dir(&self.root)
            .status()
            .map_err(|err| format!("failed to run git {}: {err}", args.join(" ")))?;
        Ok(status.code().unwrap_or(1))
    }

    fn output(&self, args: &[&str]) -> Result<Output, String> {
        Command::new("git")
            .args(args)
            .current_dir(&self.root)
            .output()
            .map_err(|err| format!("failed to run git {}: {err}", args.join(" ")))
    }
}

pub fn resolve_repo_path(root: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() { path.to_path_buf() } else { root.join(path) }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackBranchRecord {
    pub display_path: String,
    pub track_name: String,
    pub branch: Option<String>,
    pub status: Option<String>,
}

#[derive(Debug, Deserialize)]
struct BranchMetadata {
    branch: Option<String>,
    status: Option<String>,
}

pub fn load_explicit_track_branch(
    root: &Path,
    track_dir: &Path,
) -> Result<TrackBranchRecord, String> {
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
    Ok(TrackBranchRecord {
        display_path: track_dir.display().to_string(),
        track_name: track_dir
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default()
            .to_owned(),
        branch: metadata.branch,
        status: metadata.status,
    })
}

pub fn collect_track_branch_claims(root: &Path) -> Result<Vec<TrackBranchRecord>, String> {
    let mut claims = Vec::new();
    for relative_root in ["track/items", "track/archive"] {
        let claims_root = root.join(relative_root);
        if !claims_root.is_dir() {
            continue;
        }
        for entry in read_directories(&claims_root)? {
            let metadata_path = entry.join("metadata.json");
            if !metadata_path.is_file() {
                continue;
            }
            let metadata = read_metadata(&metadata_path)?;
            claims.push(TrackBranchRecord {
                display_path: entry.strip_prefix(root).unwrap_or(&entry).display().to_string(),
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
    Ok(claims)
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
    use std::fs;
    use std::path::PathBuf;
    use std::process::Command;
    use std::sync::{Mutex, OnceLock};

    use super::{
        GitRepository, SystemGitRepo, collect_track_branch_claims, load_explicit_track_branch,
        resolve_repo_path,
    };

    fn cwd_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    struct CurrentDirGuard {
        original: PathBuf,
    }

    impl CurrentDirGuard {
        fn change_to(path: &std::path::Path) -> Self {
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

    fn run_git(root: &std::path::Path, args: &[&str]) {
        let status = Command::new("git").args(args).current_dir(root).status().unwrap();
        assert!(status.success(), "git command failed: git {}", args.join(" "));
    }

    #[test]
    fn system_git_repo_discovers_root_from_nested_directory() {
        let _lock = cwd_lock().lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        run_git(dir.path(), &["init"]);
        let nested = dir.path().join("nested/deeper");
        fs::create_dir_all(&nested).unwrap();

        let _guard = CurrentDirGuard::change_to(&nested);
        let repo = SystemGitRepo::discover().unwrap();

        assert_eq!(repo.root(), dir.path());
    }

    #[test]
    fn resolve_repo_path_anchors_relative_paths_at_repo_root() {
        let root = std::path::Path::new("/repo/root");

        assert_eq!(
            resolve_repo_path(root, std::path::Path::new(".takt/pending-note.md")),
            PathBuf::from("/repo/root/.takt/pending-note.md")
        );
        assert_eq!(
            resolve_repo_path(root, std::path::Path::new("/tmp/note.md")),
            PathBuf::from("/tmp/note.md")
        );
    }

    #[test]
    fn load_explicit_track_branch_rejects_non_track_items_directory() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("track/items")).unwrap();
        let track_dir = dir.path().join("elsewhere/example");
        fs::create_dir_all(&track_dir).unwrap();
        fs::write(track_dir.join("metadata.json"), "{\"branch\":\"track/example\"}").unwrap();

        let err = load_explicit_track_branch(dir.path(), &track_dir).unwrap_err();

        assert!(err.contains("Track directory must be exactly track/items/<id>"));
    }

    #[test]
    fn collect_track_branch_claims_includes_items_and_archive() {
        let dir = tempfile::tempdir().unwrap();
        let active = dir.path().join("track/items/active");
        let archived = dir.path().join("track/archive/archived");
        fs::create_dir_all(&active).unwrap();
        fs::create_dir_all(&archived).unwrap();
        fs::write(
            active.join("metadata.json"),
            "{\"branch\":\"track/active\",\"status\":\"in_progress\"}",
        )
        .unwrap();
        fs::write(archived.join("metadata.json"), "{\"branch\":null,\"status\":\"archived\"}")
            .unwrap();

        let claims = collect_track_branch_claims(dir.path()).unwrap();
        let display_paths =
            claims.iter().map(|claim| claim.display_path.as_str()).collect::<Vec<_>>();

        assert_eq!(claims.len(), 2);
        assert!(display_paths.contains(&"track/items/active"));
        assert!(display_paths.contains(&"track/archive/archived"));
    }

    #[test]
    fn collect_track_branch_claims_fails_closed_on_invalid_metadata() {
        let dir = tempfile::tempdir().unwrap();
        let active = dir.path().join("track/items/active");
        fs::create_dir_all(&active).unwrap();
        fs::write(active.join("metadata.json"), "{not-json").unwrap();

        let err = collect_track_branch_claims(dir.path()).unwrap_err();

        assert!(err.contains("Cannot read or parse metadata.json"));
        assert!(err.contains("track/items/active/metadata.json"));
    }
}
