//! `FsCommitHashStore` — filesystem adapter for .commit_hash.

use std::path::PathBuf;

use domain::CommitHash;
use domain::review_v2::{CommitHashError, CommitHashReader, CommitHashWriter};

use super::{atomic_write_file, reject_symlinks_below};
use crate::git_cli::{GitRepository, SystemGitRepo};

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
        let path_str = self.path.display().to_string();
        reject_symlinks_below(&self.path, &self.trusted_root)
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::InvalidInput {
                    CommitHashError::SymlinkDetected { path: path_str.clone() }
                } else {
                    CommitHashError::Io { path: path_str.clone(), detail: e.to_string() }
                }
            })
            .map(|_| ())
    }
}

impl CommitHashReader for FsCommitHashStore {
    fn read(&self) -> Result<Option<CommitHash>, CommitHashError> {
        self.reject_symlinks()?;

        let content = match std::fs::read_to_string(&self.path) {
            Ok(c) => c,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => {
                return Err(CommitHashError::Io {
                    path: self.path.display().to_string(),
                    detail: format!("read: {e}"),
                });
            }
        };

        let trimmed = content.trim();
        let hash = CommitHash::try_new(trimmed).map_err(|e| {
            CommitHashError::Format(format!("invalid commit hash in {}: {e}", self.path.display()))
        })?;

        // Ancestry validation (infra implementation detail, not trait contract)
        match SystemGitRepo::discover() {
            Ok(git) => {
                let output = git.output(&["merge-base", "--is-ancestor", trimmed, "HEAD"]);
                match output {
                    Ok(o) if o.status.success() => Ok(Some(hash)),
                    _ => Ok(None), // fail-closed: scope expands
                }
            }
            Err(_) => Ok(None), // git unavailable → fail-closed
        }
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
