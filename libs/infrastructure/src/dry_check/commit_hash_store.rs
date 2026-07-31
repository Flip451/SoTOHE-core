//! `FsDryCheckCommitHashStore` — dry-check's own filesystem adapter for
//! reading the per-track `.commit_hash` file to resolve the diff base.
//!
//! CN-01: this is NOT `FsCommitHashStore` from `review_v2` — it is a
//! dry-check-owned adapter with its own error type [`DryCheckCommitHashError`].
//! Behavior mirrors `FsCommitHashStore::read()`.

use std::path::{Path, PathBuf};

use domain::CommitHash;
use thiserror::Error;

use crate::git_cli::SystemGitRepo;
use crate::track::symlink_guard::reject_symlinks_below;

// ── DryCheckCommitHashError ───────────────────────────────────────────────────

/// Error from [`FsDryCheckCommitHashStore::read`].
///
/// CN-01: independent of `domain::review_v2::CommitHashError`.
/// Three failure modes: I/O error, symlink detected, or invalid hash format.
/// An absent file returns `Ok(None)` — there is no `NotFound` variant.
#[derive(Debug, Error)]
pub enum DryCheckCommitHashError {
    /// File system I/O failure.
    #[error("dry-check commit hash I/O error: {path}: {detail}")]
    Io {
        /// The file path involved.
        path: String,
        /// Human-readable description of the failure.
        detail: String,
    },
    /// The target path is a symlink (rejected for security).
    #[error("dry-check commit hash: symlink detected at {path}")]
    SymlinkDetected {
        /// The symlink path.
        path: String,
    },
    /// The stored content is not a valid commit hash.
    #[error("dry-check commit hash: invalid hash format: {0}")]
    Format(String),
}

// ── FsDryCheckCommitHashStore ─────────────────────────────────────────────────

/// Filesystem adapter for reading the per-track `.commit_hash` file used by the
/// dry-check gate to resolve the diff base.
///
/// Three outcomes from [`read`](Self::read):
/// 1. File absent → `Ok(None)` (main-tip fallback).
/// 2. Stored content is not a valid `CommitHash` → `Err(DryCheckCommitHashError::Format)`.
/// 3. Hash is not an ancestor of HEAD → `Ok(None)` (fail-closed, main-tip fallback).
///
/// The fail-closed policy (absorbing `Err(Format)` and falling through to
/// `git rev-parse <configured base branch>`) is applied by the CLI composition layer,
/// not here.
#[derive(Debug)]
pub struct FsDryCheckCommitHashStore {
    path: PathBuf,
    trusted_root: PathBuf,
}

impl FsDryCheckCommitHashStore {
    /// Construct a new [`FsDryCheckCommitHashStore`].
    #[must_use]
    pub fn new(path: PathBuf, trusted_root: PathBuf) -> FsDryCheckCommitHashStore {
        Self { path, trusted_root }
    }

    /// Read the stored commit hash, validate its format, and check ancestry.
    ///
    /// # Errors
    ///
    /// - `Err(DryCheckCommitHashError::SymlinkDetected)` if the path is a symlink.
    /// - `Err(DryCheckCommitHashError::Io)` on I/O errors other than `NotFound`.
    /// - `Err(DryCheckCommitHashError::Format)` if the file content is not a
    ///   valid `CommitHash`.
    ///
    /// Returns `Ok(None)` when the file is absent or the hash is not an ancestor
    /// of HEAD (fail-closed).
    pub fn read(&self) -> Result<Option<CommitHash>, DryCheckCommitHashError> {
        let path_str = self.path.display().to_string();

        // `new` accepts any two paths, so the containment the rest of this method
        // depends on is established here rather than assumed: the symlink guard
        // walks the components between the record and the trusted root, which says
        // nothing about whether the record is under it at all.
        match locate_record(&self.path, &self.trusted_root)? {
            RecordLocation::Inside => {}
            // No directory to hold the record: the same absence a read would find.
            RecordLocation::NoParent => return Ok(None),
            RecordLocation::Outside => {
                return Err(DryCheckCommitHashError::Io {
                    path: path_str,
                    detail: "resolves outside the trusted root".to_owned(),
                });
            }
        }

        // Symlink check before reading.
        reject_symlinks_below(&self.path, &self.trusted_root)
            .map_err(|e| guard_failure(&e, &path_str))?;

        let Some(content) = read_bounded_record(&self.path, &path_str)? else {
            // File absent → Ok(None) (main-tip fallback).
            return Ok(None);
        };

        let trimmed = content.trim();

        // Validate hash format.
        // `.commit_hash` is the well-known per-track name, and the validation
        // failure describes the content rather than the location, so neither half
        // of this needs the absolute path the record was read from.
        let hash = CommitHash::try_new(trimmed).map_err(|e| {
            DryCheckCommitHashError::Format(format!("invalid commit hash in .commit_hash: {e}"))
        })?;

        // Ancestry check: is the stored hash an ancestor of HEAD? Anchored on the
        // trusted root the record was read from, not on the process working
        // directory, so the answer describes the repository holding the record
        // rather than whichever repository the process happens to stand in.
        match SystemGitRepo::discover_from(&self.trusted_root) {
            Ok(git) => {
                let output = git.output(&["merge-base", "--is-ancestor", trimmed, "HEAD"]);
                match output {
                    Ok(o) if o.status.success() => Ok(Some(hash)),
                    // Not an ancestor or any error → fail-closed (main fallback).
                    _ => Ok(None),
                }
            }
            // git unavailable → fail-closed.
            Err(_) => Ok(None),
        }
    }
}

/// Where the record sits relative to the trusted root.
///
/// Three outcomes rather than a bool: an absent parent directory and a record
/// outside the root are different answers, and collapsing them would either turn
/// an ordinary absence into a refusal or a refusal into a silent fallback.
#[derive(Debug, PartialEq, Eq)]
enum RecordLocation {
    /// Inside the trusted root, in a directory that exists.
    Inside,
    /// No directory exists to hold the record.
    NoParent,
    /// Resolves outside the trusted root.
    Outside,
}

/// Resolves where `path` sits relative to `trusted_root`, following the crate's
/// canonical-containment pattern.
///
/// Containment is judged on the record's parent directory: the record itself is
/// often absent — that is the ordinary main-tip fallback — and canonicalising a
/// path that does not exist fails regardless of where it would have been.
///
/// An existing parent is judged canonically, so a symlink that escapes the root is
/// caught. A parent that does not exist is judged lexically instead: it resolves
/// to nothing, but it still names a location, and only a location inside the root
/// may be reported as an ordinary absence.
///
/// # Errors
///
/// Returns [`DryCheckCommitHashError::Io`] when either path exists but cannot be
/// resolved, classified without naming what was being resolved.
fn locate_record(
    path: &Path,
    trusted_root: &Path,
) -> Result<RecordLocation, DryCheckCommitHashError> {
    let canonical_root =
        trusted_root.canonicalize().map_err(|error| DryCheckCommitHashError::Io {
            path: trusted_root.display().to_string(),
            detail: crate::sanitized_failure::io_classification(&error).to_owned(),
        })?;
    let Some(parent) = path.parent() else {
        return Ok(RecordLocation::Outside);
    };
    let canonical_parent = match parent.canonicalize() {
        Ok(canonical_parent) => canonical_parent,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let absolute_parent = if parent.is_absolute() {
                parent.to_path_buf()
            } else {
                canonical_root.join(parent)
            };

            // A `NotFound` proves only that the leaf is missing, not that the way
            // to it is honest: an intermediate symlink out of the root, followed
            // by a component that does not exist, would otherwise be judged on the
            // spelling alone and pass as an ordinary absence. So the part that does
            // exist is resolved and checked first — that is where a hop would be.
            let Some(existing) = nearest_existing_ancestor(&absolute_parent)? else {
                return Ok(RecordLocation::Outside);
            };
            if !existing.starts_with(&canonical_root) {
                return Ok(RecordLocation::Outside);
            }

            // Canonicalising says where the chain ends up, not what it passed
            // through: a symlink whose target stays inside the root resolves
            // happily, and a dangling one reports `NotFound` as though nothing were
            // there at all. Neither may be walked past on the way to reporting an
            // ordinary absence, so the guard inspects the chain itself — it reads
            // link metadata rather than following it.
            reject_symlinks_below(&absolute_parent, &canonical_root)
                .map_err(|e| guard_failure(&e, &absolute_parent.display().to_string()))?;

            // With the real part proven inside and unlinked, the missing remainder
            // is judged on where it says it points.
            let stated = crate::lexical_path::lexical_normalize(&absolute_parent);
            return Ok(if stated.starts_with(&canonical_root) {
                RecordLocation::NoParent
            } else {
                RecordLocation::Outside
            });
        }
        Err(error) => {
            return Err(DryCheckCommitHashError::Io {
                path: path.display().to_string(),
                detail: crate::sanitized_failure::io_classification(&error).to_owned(),
            });
        }
    };

    if canonical_parent.starts_with(&canonical_root) {
        Ok(RecordLocation::Inside)
    } else {
        Ok(RecordLocation::Outside)
    }
}

/// The most a well-formed record can occupy.
///
/// A commit hash is 40 hexadecimal characters and the file holds one, with at most
/// a trailing newline. The cap is set well above that so an ordinary editor's
/// stray whitespace still reads, while a file that could exhaust memory does not.
const MAX_COMMIT_RECORD_BYTES: u64 = 256;

/// Reads the record, refusing anything that is not a small regular file.
///
/// Returns `Ok(None)` when the record is absent, which is the ordinary main-tip
/// fallback. The type check is what keeps a FIFO from blocking the gate for ever:
/// opening one blocks until a writer arrives, so the file type is settled before
/// anything is opened, and the read is bounded regardless.
///
/// # Errors
///
/// Returns [`DryCheckCommitHashError::Io`] for a non-regular file, a file above
/// the cap, or a read that fails, classified without naming the path.
fn read_bounded_record(
    path: &Path,
    path_str: &str,
) -> Result<Option<String>, DryCheckCommitHashError> {
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(DryCheckCommitHashError::Io {
                path: path_str.to_owned(),
                detail: crate::sanitized_failure::io_classification(&error).to_owned(),
            });
        }
    };
    if !metadata.file_type().is_file() {
        return Err(DryCheckCommitHashError::Io {
            path: path_str.to_owned(),
            detail: "not a regular file".to_owned(),
        });
    }
    if metadata.len() > MAX_COMMIT_RECORD_BYTES {
        return Err(DryCheckCommitHashError::Io {
            path: path_str.to_owned(),
            detail: "larger than a commit record can be".to_owned(),
        });
    }

    // Bounded regardless of what the metadata said: the file may grow between the
    // check and the read.
    let file = std::fs::File::open(path).map_err(|error| DryCheckCommitHashError::Io {
        path: path_str.to_owned(),
        detail: crate::sanitized_failure::io_classification(&error).to_owned(),
    })?;
    use std::io::Read as _;
    let mut content = String::new();
    file.take(MAX_COMMIT_RECORD_BYTES.saturating_add(1)).read_to_string(&mut content).map_err(
        |error| DryCheckCommitHashError::Io {
            path: path_str.to_owned(),
            detail: crate::sanitized_failure::io_classification(&error).to_owned(),
        },
    )?;
    if content.len() as u64 > MAX_COMMIT_RECORD_BYTES {
        return Err(DryCheckCommitHashError::Io {
            path: path_str.to_owned(),
            detail: "larger than a commit record can be".to_owned(),
        });
    }

    Ok(Some(content))
}

/// Canonicalises the closest ancestor of `path` that exists.
///
/// Walks up until a component resolves, so the answer describes real filesystem
/// structure rather than the spelling of a path that is not there. Returns `None`
/// when nothing along the way exists at all, which no trusted root can contain.
///
/// # Errors
///
/// Returns [`DryCheckCommitHashError::Io`] when an ancestor exists but cannot be
/// resolved, classified without naming it.
fn nearest_existing_ancestor(path: &Path) -> Result<Option<PathBuf>, DryCheckCommitHashError> {
    for ancestor in path.ancestors() {
        match ancestor.canonicalize() {
            Ok(canonical) => return Ok(Some(canonical)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(DryCheckCommitHashError::Io {
                    path: path.display().to_string(),
                    detail: crate::sanitized_failure::io_classification(&error).to_owned(),
                });
            }
        }
    }
    Ok(None)
}

/// Classifies a symlink-guard failure for the record at `path`.
///
/// The refusal is recognised by the guard's carried payload rather than by
/// [`std::io::ErrorKind::InvalidInput`]: the filesystem raises that kind for any
/// malformed path — an interior NUL byte, for instance — and reporting such a
/// failure as a symlink would point an operator at a fault that is not there.
///
/// The carried detail is the sanitized classification, because the guard's own
/// message renders the absolute component it inspected.
fn guard_failure(error: &std::io::Error, path: &str) -> DryCheckCommitHashError {
    if crate::track::symlink_guard::is_symlink_rejection(error) {
        DryCheckCommitHashError::SymlinkDetected { path: path.to_owned() }
    } else {
        DryCheckCommitHashError::Io {
            path: path.to_owned(),
            detail: crate::sanitized_failure::io_classification(error).to_owned(),
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    fn store_in_dir(dir: &tempfile::TempDir, filename: &str) -> FsDryCheckCommitHashStore {
        let path = dir.path().join(filename);
        FsDryCheckCommitHashStore::new(path, dir.path().to_owned())
    }

    #[test]
    fn test_read_returns_ok_none_when_file_absent() {
        let dir = tempfile::tempdir().unwrap();
        let store = store_in_dir(&dir, ".commit_hash");
        let result = store.read().unwrap();
        assert!(result.is_none(), "absent file should return Ok(None)");
    }

    #[test]
    fn test_read_returns_err_format_when_content_is_invalid_hash() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".commit_hash");
        // Write content that is not a valid commit hash.
        std::fs::write(&path, "not-a-valid-hash\n").unwrap();
        let store = FsDryCheckCommitHashStore::new(path, dir.path().to_owned());
        let result = store.read();
        assert!(
            matches!(result, Err(DryCheckCommitHashError::Format(_))),
            "invalid hash should return Err(Format), got: {result:?}"
        );
    }

    /// For a valid ancestor hash, `SystemGitRepo::discover()` is used. In a
    /// test environment, the hash may not be a real ancestor, so the function
    /// returns `Ok(None)` (non-ancestor → fail-closed). This tests the format
    /// validation passes and the ancestry step is reached.
    #[test]
    fn test_read_with_valid_hash_format_reaches_ancestry_check() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".commit_hash");
        // Write a syntactically valid hash (7 lowercase hex chars).
        std::fs::write(&path, "abc1234\n").unwrap();
        let store = FsDryCheckCommitHashStore::new(path, dir.path().to_owned());
        // Returns Ok(None) because `abc1234` is not an ancestor in the real repo,
        // OR returns Ok(Some) if it happens to be. Either is valid here.
        let result = store.read();
        assert!(result.is_ok(), "valid hash format should not return Err(Format), got: {result:?}");
    }

    #[cfg(unix)]
    #[test]
    fn test_read_returns_symlink_detected_for_symlink_path() {
        let dir = tempfile::tempdir().unwrap();
        let real = dir.path().join("real_hash");
        std::fs::write(&real, "abc1234\n").unwrap();
        let link = dir.path().join(".commit_hash");
        std::os::unix::fs::symlink(&real, &link).unwrap();
        let store = FsDryCheckCommitHashStore::new(link, dir.path().to_owned());
        let result = store.read();
        assert!(
            matches!(result, Err(DryCheckCommitHashError::SymlinkDetected { .. })),
            "symlink should return Err(SymlinkDetected), got: {result:?}"
        );
    }

    #[test]
    fn test_a_record_outside_the_trusted_root_is_refused_rather_than_read() {
        // The constructor takes any two paths, so a record in an unrelated tree is
        // constructible; reading it must be refused rather than served.
        let root = tempfile::tempdir().unwrap();
        let elsewhere = tempfile::tempdir().unwrap();
        let outside_record = elsewhere.path().join(".commit_hash");
        std::fs::write(&outside_record, "a1b2c3d4e5f60718293a4b5c6d7e8f9012345678\n").unwrap();

        let result = FsDryCheckCommitHashStore::new(outside_record, root.path().to_owned()).read();

        let Err(DryCheckCommitHashError::Io { detail, .. }) = result else {
            panic!("a record outside the trusted root must be refused: {result:?}");
        };
        assert_eq!(detail, "resolves outside the trusted root");
    }

    #[test]
    fn test_the_record_location_tells_absence_apart_from_escape() {
        let root = tempfile::tempdir().unwrap();
        let elsewhere = tempfile::tempdir().unwrap();

        assert_eq!(
            locate_record(&root.path().join(".commit_hash"), root.path()).unwrap(),
            RecordLocation::Inside
        );
        assert_eq!(
            locate_record(&elsewhere.path().join(".commit_hash"), root.path()).unwrap(),
            RecordLocation::Outside
        );
        // An absent directory inside the root is not an escape: nothing is there
        // to read, and the caller falls back exactly as it would for an absent
        // record.
        assert_eq!(
            locate_record(&root.path().join("no-such-track/.commit_hash"), root.path()).unwrap(),
            RecordLocation::NoParent
        );
        // But absence outside the root is still an escape. Canonicalising cannot
        // answer this — there is nothing on disk to resolve — so the location is
        // read off the path itself rather than being waved through as an ordinary
        // missing record.
        assert_eq!(
            locate_record(&elsewhere.path().join("no-such-track/.commit_hash"), root.path())
                .unwrap(),
            RecordLocation::Outside
        );
        // Including when it takes `..` to get there.
        assert_eq!(
            locate_record(&root.path().join("../no-such-sibling/.commit_hash"), root.path())
                .unwrap(),
            RecordLocation::Outside
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_a_symlink_hop_is_caught_even_when_the_rest_of_the_path_is_missing() {
        // root/link points outside, and nothing exists beyond it. Judging the
        // spelling alone would call this an ordinary absence and fall back
        // silently; the hop is real and must be refused.
        let root = tempfile::tempdir().unwrap();
        let elsewhere = tempfile::tempdir().unwrap();
        std::os::unix::fs::symlink(elsewhere.path(), root.path().join("link")).unwrap();
        let record = root.path().join("link/no-such-track/.commit_hash");

        assert_eq!(locate_record(&record, root.path()).unwrap(), RecordLocation::Outside);

        // And the adapter refuses rather than reporting the absence that would
        // send the caller to its fallback.
        let result = FsDryCheckCommitHashStore::new(record, root.path().to_owned()).read();
        let Err(DryCheckCommitHashError::Io { detail, .. }) = result else {
            panic!("a record reached through a symlink hop must be refused: {result:?}");
        };
        assert_eq!(detail, "resolves outside the trusted root");
    }

    #[cfg(unix)]
    #[test]
    fn test_a_symlink_ancestor_is_refused_even_when_it_stays_inside_the_root() {
        // Canonicalising reports where the chain ends up, so an in-root symlink
        // and a dangling one both slip past a containment check alone: the first
        // resolves to somewhere legitimate, the second reports `NotFound` as
        // though nothing were there. Neither may be walked past to report an
        // ordinary absence.
        for (label, target) in [("in-root", Some("real-track")), ("dangling", None)] {
            let root = tempfile::tempdir().unwrap();
            std::fs::create_dir_all(root.path().join("real-track")).unwrap();
            let link = root.path().join("link");
            match target {
                Some(existing) => std::os::unix::fs::symlink(root.path().join(existing), &link),
                None => std::os::unix::fs::symlink(root.path().join("no-such-target"), &link),
            }
            .unwrap();
            let record = root.path().join("link/no-such-track/.commit_hash");

            let located = locate_record(&record, root.path());

            assert!(
                matches!(located, Err(DryCheckCommitHashError::SymlinkDetected { .. })),
                "{label} symlink ancestor must be refused, got: {located:?}"
            );
            // And the adapter refuses rather than falling back.
            let result = FsDryCheckCommitHashStore::new(record, root.path().to_owned()).read();
            assert!(
                matches!(result, Err(DryCheckCommitHashError::SymlinkDetected { .. })),
                "{label}: read must refuse rather than report absence, got: {result:?}"
            );
        }
    }

    #[test]
    fn test_a_record_that_is_not_a_small_regular_file_is_refused_before_it_is_parsed() {
        let dir = tempfile::tempdir().unwrap();
        let record = dir.path().join(".commit_hash");
        // Far more than a commit hash can be, and enough that reading it unbounded
        // would be a choice rather than an accident.
        std::fs::write(&record, "0".repeat(64 * 1024)).unwrap();

        let result = FsDryCheckCommitHashStore::new(record.clone(), dir.path().to_owned()).read();

        let Err(DryCheckCommitHashError::Io { detail, .. }) = result else {
            panic!("an oversized record must be refused: {result:?}");
        };
        assert_eq!(detail, "larger than a commit record can be");
    }

    #[cfg(unix)]
    #[test]
    fn test_a_fifo_record_is_refused_rather_than_opened() {
        // Opening a FIFO blocks until a writer arrives, so the file type decides
        // before anything is opened. Without that, the gate waits for ever.
        let dir = tempfile::tempdir().unwrap();
        let record = dir.path().join(".commit_hash");
        rustix::fs::mkfifoat(rustix::fs::CWD, &record, rustix::fs::Mode::from_raw_mode(0o600))
            .unwrap();

        let result = FsDryCheckCommitHashStore::new(record, dir.path().to_owned()).read();

        let Err(DryCheckCommitHashError::Io { detail, .. }) = result else {
            panic!("a FIFO must be refused: {result:?}");
        };
        assert_eq!(detail, "not a regular file");
    }

    #[cfg(unix)]
    #[test]
    fn test_only_the_guards_own_refusal_is_reported_as_a_symlink() {
        // A real refusal, carrying the guard's payload.
        let dir = tempfile::tempdir().unwrap();
        let link = dir.path().join(".commit_hash");
        std::os::unix::fs::symlink(dir.path().join("real_hash"), &link).unwrap();
        let refusal = reject_symlinks_below(&link, dir.path())
            .expect_err("a symlinked record must be refused");

        assert!(matches!(
            guard_failure(&refusal, ".commit_hash"),
            DryCheckCommitHashError::SymlinkDetected { .. }
        ));

        // The filesystem raises the same kind for a path it simply cannot accept —
        // an interior NUL byte, say. That is an I/O fault, not a symlink, and
        // saying otherwise would send an operator looking for a link that is not
        // there.
        let malformed = std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "file name contained an unexpected NUL byte",
        );

        let mapped = guard_failure(&malformed, ".commit_hash");

        let DryCheckCommitHashError::Io { path, detail } = mapped else {
            panic!("a non-symlink InvalidInput must be an I/O failure: {mapped:?}");
        };
        assert_eq!(path, ".commit_hash");
        assert_eq!(detail, "not usable as a repository path", "the detail is the classification");
    }
}
