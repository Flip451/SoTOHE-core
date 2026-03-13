use std::path::{Path, PathBuf};
use std::process::{Command, Output};

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

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::process::Command;
    use std::sync::{Mutex, OnceLock};

    use super::{GitRepository, SystemGitRepo, resolve_repo_path};

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
}
