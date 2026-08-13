//! `FsCommitHashStore` — filesystem adapter for .commit_hash.

use std::path::PathBuf;

use domain::CommitHash;
use domain::review_v2::{CommitHashError, CommitHashReader, CommitHashWriter};

use super::{atomic_write_file, reject_symlinks_below};
use crate::git_cli::{SystemGitRepo, isolated_bounded_git_output};
use crate::trusted_file::{RecordLocation, locate_record, read_bounded_regular_file};

/// Ancestry probes have only a one-byte success protocol, but retain a small
/// bounded diagnostic allowance for a refused Git invocation.
const MAX_ANCESTRY_OUTPUT_BYTES: usize = 1024;

/// Filesystem-based .commit_hash reader/writer with ancestry validation.
pub struct FsCommitHashStore {
    path: PathBuf,
    trusted_root: PathBuf,
}

impl FsCommitHashStore {
    #[must_use]
    pub fn new(commit_hash_path: PathBuf, trusted_root: PathBuf) -> Self {
        Self { path: commit_hash_path, trusted_root }
    }

    /// Rejects symlinks on the path below trusted_root.
    fn reject_symlinks(&self) -> Result<(), CommitHashError> {
        if self.locate_record()? == RecordLocation::Outside {
            return Err(CommitHashError::Io {
                path: self.path.display().to_string(),
                detail: "resolves outside the trusted root".to_owned(),
            });
        }
        let path_str = self.path.display().to_string();
        reject_symlinks_below(&self.path, &self.trusted_root)
            .map_err(|e| {
                if crate::track::symlink_guard::is_symlink_rejection(&e) {
                    CommitHashError::SymlinkDetected { path: path_str.clone() }
                } else {
                    CommitHashError::Io { path: path_str.clone(), detail: e.to_string() }
                }
            })
            .map(|_| ())
    }

    /// Establishes canonical containment before the component-wise symlink
    /// guard, so an arbitrary path passed to `new` cannot be treated as an
    /// ordinary missing record outside this store's trusted root.
    fn locate_record(&self) -> Result<RecordLocation, CommitHashError> {
        locate_record(&self.path, &self.trusted_root).map_err(|error| {
            if crate::track::symlink_guard::is_symlink_rejection(&error) {
                CommitHashError::SymlinkDetected { path: self.path.display().to_string() }
            } else {
                CommitHashError::Io {
                    path: self.path.display().to_string(),
                    detail: error.to_string(),
                }
            }
        })
    }

    /// Reads the record through the trusted bounded-file path before any Git
    /// operation. Its absence is the domain port's documented main-tip
    /// fallback, independent of whether this directory is a Git repository.
    fn read_record(&self) -> Result<Option<String>, CommitHashError> {
        match self.locate_record()? {
            RecordLocation::Inside => {}
            RecordLocation::NoParent => return Ok(None),
            RecordLocation::Outside => {
                return Err(CommitHashError::Io {
                    path: self.path.display().to_string(),
                    detail: "resolves outside the trusted root".to_owned(),
                });
            }
        }
        self.reject_symlinks()?;

        read_bounded_regular_file(&self.path, &self.trusted_root, MAX_COMMIT_RECORD_BYTES).map_err(
            |error| CommitHashError::Io {
                path: self.path.display().to_string(),
                detail: format!("read: {error}"),
            },
        )
    }

    /// Validates record content only when it is an ancestor in `git`'s exact,
    /// already-isolated repository and history view.
    fn validate_record_for_git(
        &self,
        git: &SystemGitRepo,
        content: &str,
    ) -> Result<Option<CommitHash>, CommitHashError> {
        let trimmed = content.trim();
        let hash = CommitHash::try_new(trimmed).map_err(|error| {
            CommitHashError::Format(format!(
                "invalid commit hash in {}: {error}",
                self.path.display()
            ))
        })?;

        let output = isolated_bounded_git_output(
            git.root(),
            &["merge-base", "--is-ancestor", trimmed, "HEAD"],
            MAX_ANCESTRY_OUTPUT_BYTES,
        )
        .map_err(|error| CommitHashError::Io {
            path: self.path.display().to_string(),
            detail: format!("git merge-base --is-ancestor: {error}"),
        })?;
        match output.status.code() {
            Some(0) => Ok(Some(hash)),
            // `merge-base --is-ancestor` reserves exit 1 for the ordinary
            // negative answer. Any other status means Git could not decide,
            // so the caller must not silently select a different diff base.
            Some(1) => Ok(None),
            status => Err(CommitHashError::Io {
                path: self.path.display().to_string(),
                detail: format!(
                    "git merge-base --is-ancestor could not decide (exit {})",
                    status.unwrap_or(-1)
                ),
            }),
        }
    }

    /// Reads a persisted hash only when it is an ancestor in `git`'s exact,
    /// already-isolated repository and history view.
    pub(crate) fn read_for_git(
        &self,
        git: &SystemGitRepo,
    ) -> Result<Option<CommitHash>, CommitHashError> {
        let Some(content) = self.read_record()? else {
            return Ok(None);
        };
        self.validate_record_for_git(git, &content)
    }
}

/// The most a commit record can occupy. The bounded reader is defense in depth
/// against an oversized file and a FIFO or device supplied in its place.
const MAX_COMMIT_RECORD_BYTES: u64 = 256;

impl CommitHashReader for FsCommitHashStore {
    fn read(&self) -> Result<Option<CommitHash>, CommitHashError> {
        let Some(content) = self.read_record()? else {
            return Ok(None);
        };
        // Legacy callers do not hold a repository handle. Anchor discovery to
        // the trusted root and keep the same isolated ancestry lane used by
        // check-zero-findings rather than inheriting process Git overrides.
        let git = SystemGitRepo::discover_from_isolated(&self.trusted_root).map_err(|error| {
            CommitHashError::Io {
                path: self.path.display().to_string(),
                detail: format!("git discover: {error}"),
            }
        })?;
        self.validate_record_for_git(&git, &content)
    }
}

impl CommitHashWriter for FsCommitHashStore {
    fn write(&self, hash: &CommitHash) -> Result<(), CommitHashError> {
        self.reject_symlinks()?;

        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| CommitHashError::Io {
                path: parent.display().to_string(),
                detail: format!("create dir: {e}"),
            })?;
        }
        atomic_write_file(&self.path, hash.as_ref().as_bytes()).map_err(|e| CommitHashError::Io {
            path: self.path.display().to_string(),
            detail: format!("atomic write: {e}"),
        })
    }

    fn clear(&self) -> Result<(), CommitHashError> {
        self.reject_symlinks()?;

        match std::fs::remove_file(&self.path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(CommitHashError::Io {
                path: self.path.display().to_string(),
                detail: format!("remove: {e}"),
            }),
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use std::path::Path;
    use std::process::Command;

    use domain::CommitHash;
    use domain::review_v2::{CommitHashReader as _, CommitHashWriter as _};

    use super::{CommitHashError, FsCommitHashStore, SystemGitRepo};

    fn git(root: &Path, args: &[&str]) {
        let status = Command::new("git").args(args).current_dir(root).status().unwrap();
        assert!(status.success(), "git {:?} must succeed", args);
    }

    fn git_stdout(root: &Path, args: &[&str]) -> String {
        let output = Command::new("git").args(args).current_dir(root).output().unwrap();
        assert!(output.status.success(), "git {:?} must succeed", args);
        String::from_utf8(output.stdout).unwrap().trim().to_owned()
    }

    fn initialized_repo() -> tempfile::TempDir {
        let repo = tempfile::tempdir().unwrap();
        git(repo.path(), &["init", "-b", "main"]);
        git(repo.path(), &["config", "user.email", "fixture@example.com"]);
        git(repo.path(), &["config", "user.name", "Fixture"]);
        repo
    }

    #[test]
    fn test_read_for_git_returns_none_for_a_clean_non_ancestor() {
        let repo = initialized_repo();
        git(repo.path(), &["commit", "--allow-empty", "-m", "base"]);
        git(repo.path(), &["checkout", "-b", "side"]);
        git(repo.path(), &["commit", "--allow-empty", "-m", "side"]);
        let side = git_stdout(repo.path(), &["rev-parse", "HEAD"]);
        git(repo.path(), &["checkout", "main"]);
        git(repo.path(), &["commit", "--allow-empty", "-m", "main"]);

        let record = repo.path().join(".commit_hash");
        std::fs::write(&record, side).unwrap();
        let store = FsCommitHashStore::new(record, repo.path().to_path_buf());
        let git = SystemGitRepo::discover_from_isolated(repo.path()).unwrap();

        assert!(store.read_for_git(&git).unwrap().is_none());
    }

    #[test]
    fn test_read_for_git_propagates_a_failed_ancestry_probe() {
        let repo = initialized_repo();
        let record = repo.path().join(".commit_hash");
        std::fs::write(&record, "a".repeat(40)).unwrap();
        let store = FsCommitHashStore::new(record, repo.path().to_path_buf());
        let git = SystemGitRepo::discover_from_isolated(repo.path()).unwrap();

        let error = store.read_for_git(&git).unwrap_err();

        assert!(matches!(error, CommitHashError::Io { .. }));
        assert!(error.to_string().contains("could not decide"));
    }

    #[test]
    fn test_legacy_reader_propagates_isolated_discovery_failure() {
        let root = tempfile::tempdir().unwrap();
        let record = root.path().join(".commit_hash");
        std::fs::write(&record, "a".repeat(40)).unwrap();
        let store = FsCommitHashStore::new(record, root.path().to_path_buf());

        let error = store.read().unwrap_err();

        assert!(matches!(error, CommitHashError::Io { .. }));
        assert!(error.to_string().contains("git discover"));
    }

    #[test]
    fn test_legacy_reader_returns_none_for_an_absent_record_without_git_discovery() {
        let root = tempfile::tempdir().unwrap();
        let store =
            FsCommitHashStore::new(root.path().join(".commit_hash"), root.path().to_path_buf());

        assert!(store.read().unwrap().is_none());
    }

    #[test]
    fn test_write_rejects_a_relative_record_path() {
        let root = tempfile::tempdir().unwrap();
        let store = FsCommitHashStore::new(
            std::path::PathBuf::from("untrusted/.commit_hash"),
            root.path().to_path_buf(),
        );
        let hash = CommitHash::try_new("a".repeat(40)).unwrap();

        let error = store.write(&hash).unwrap_err();

        assert!(matches!(error, CommitHashError::Io { .. }));
        assert!(error.to_string().contains("must be absolute"));
    }

    #[cfg(unix)]
    #[test]
    fn test_malformed_record_path_is_not_classified_as_a_symlink() {
        use std::ffi::OsString;
        use std::os::unix::ffi::{OsStrExt as _, OsStringExt as _};

        let root = tempfile::tempdir().unwrap();
        let mut malformed = root.path().as_os_str().as_bytes().to_vec();
        malformed.extend_from_slice(b"/bad\0.commit_hash");
        let store = FsCommitHashStore::new(
            std::path::PathBuf::from(OsString::from_vec(malformed)),
            root.path().to_path_buf(),
        );
        let hash = CommitHash::try_new("a".repeat(40)).unwrap();

        let error = store.write(&hash).unwrap_err();

        assert!(matches!(error, CommitHashError::Io { .. }));
    }

    #[cfg(unix)]
    #[test]
    fn test_symlink_record_path_is_classified_as_a_symlink() {
        let root = tempfile::tempdir().unwrap();
        let target = root.path().join("target");
        let record = root.path().join(".commit_hash");
        std::fs::write(&target, "a".repeat(40)).unwrap();
        std::os::unix::fs::symlink(&target, &record).unwrap();
        let store = FsCommitHashStore::new(record, root.path().to_path_buf());
        let hash = CommitHash::try_new("a".repeat(40)).unwrap();

        let error = store.write(&hash).unwrap_err();

        assert!(matches!(error, CommitHashError::SymlinkDetected { .. }));
    }
}
