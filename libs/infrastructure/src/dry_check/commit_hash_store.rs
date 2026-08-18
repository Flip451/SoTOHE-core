//! `FsDryCheckCommitHashStore` — dry-check's own filesystem adapter for
//! reading the per-track `.commit_hash` file to resolve the diff base.
//!
//! CN-01: this is NOT `FsCommitHashStore` from `review_v2` — it is a
//! dry-check-owned adapter with its own error type [`DryCheckCommitHashError`].
//! Behavior mirrors `FsCommitHashStore::read()`.

use std::path::PathBuf;

use domain::CommitHash;
use thiserror::Error;

use crate::git_cli::SystemGitRepo;
use crate::track::symlink_guard::reject_symlinks_below;
use crate::trusted_file::{RecordLocation, locate_record, read_bounded_regular_file};

/// `merge-base --is-ancestor` answers with its exit code and prints nothing, so
/// the retention limit only has to cover a diagnostic line.
const MAX_ANCESTRY_OUTPUT_BYTES: usize = 1024;

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
        match locate_record(&self.path, &self.trusted_root)
            .map_err(|error| guard_failure(&error, &path_str))?
        {
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

        let Some(content) =
            read_bounded_regular_file(&self.path, &self.trusted_root, MAX_COMMIT_RECORD_BYTES)
                .map_err(|error| bounded_record_failure(&error, &path_str))?
        else {
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
        //
        // Isolated and bounded like every other read whose answer decides a
        // gate: the ambient environment must not be able to name the repository
        // this ancestry is asked of, a replacement object or graft must not be
        // able to invent the reachability that makes the record usable, and a
        // git call that never returns must not hold the gate open. Every
        // outcome other than a clean success stays fail-closed, exactly as
        // before.
        // The query runs from the directory discovery started at, not from the
        // root it reported: a nested repository configured with `core.worktree`
        // makes `--show-toplevel` name an enclosing checkout, and asking that
        // checkout would answer about a different HEAD.
        match SystemGitRepo::discover_from_isolated(&self.trusted_root) {
            Ok(_) => {
                let output = crate::git_cli::isolated_bounded_git_output(
                    &self.trusted_root,
                    &["merge-base", "--is-ancestor", trimmed, "HEAD"],
                    MAX_ANCESTRY_OUTPUT_BYTES,
                );
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

/// The most a well-formed record can occupy.
///
/// A commit hash is 40 hexadecimal characters and the file holds one, with at most
/// a trailing newline. The cap is set well above that so an ordinary editor's
/// stray whitespace still reads, while a file that could exhaust memory does not.
const MAX_COMMIT_RECORD_BYTES: u64 = 256;

/// Preserves dry-check's operator-facing distinctions while the actual guarded
/// read is shared with the review-state adapter.
fn bounded_record_failure(error: &std::io::Error, path: &str) -> DryCheckCommitHashError {
    let detail = match error.to_string().as_str() {
        "not a regular file" => "not a regular file".to_owned(),
        "larger than the configured bound" => "larger than a commit record can be".to_owned(),
        _ => crate::sanitized_failure::io_classification(error).to_owned(),
    };
    DryCheckCommitHashError::Io { path: path.to_owned(), detail }
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
    use std::path::Path;

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

    fn git(dir: &Path, args: &[&str]) {
        let status = std::process::Command::new("git")
            .args(args)
            .current_dir(dir)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .unwrap();
        assert!(status.success(), "git {args:?} must succeed in the fixture");
    }

    fn rev_parse(dir: &Path, revision: &str) -> String {
        let output = std::process::Command::new("git")
            .args(["rev-parse", revision])
            .current_dir(dir)
            .output()
            .unwrap();
        assert!(output.status.success(), "the fixture must resolve {revision}");
        String::from_utf8(output.stdout).unwrap().trim().to_owned()
    }

    #[test]
    fn test_a_graft_cannot_make_an_unreachable_record_usable() {
        // A record naming a commit that is not reachable from HEAD must fall back
        // to the configured branch. A graft adding that commit as a parent of
        // HEAD makes an ordinary `merge-base --is-ancestor` answer yes, which
        // would hand the gate a base outside the branch's own history and shrink
        // the measured diff to whatever that base makes small.
        //
        // Grafts are the mechanism that reaches this lane: a replacement object
        // standing in for the queried commit does not move the answer, because
        // reachability is computed from the commit the argument names.
        let repo = tempfile::Builder::new()
            .prefix("dry-check-commit-hash-store-")
            .tempdir_in(std::env::current_dir().unwrap())
            .unwrap();
        let root = repo.path();
        git(root, &["init", "-b", "main"]);
        git(root, &["config", "user.email", "fixture@example.com"]);
        git(root, &["config", "user.name", "fixture"]);
        git(root, &["commit", "--allow-empty", "-m", "base"]);
        let head = rev_parse(root, "HEAD");

        // An unrelated commit on a detached history: no ancestry to HEAD at all.
        git(root, &["checkout", "-q", "--orphan", "unrelated"]);
        git(root, &["commit", "--allow-empty", "-m", "unrelated"]);
        let unreachable = rev_parse(root, "HEAD");
        git(root, &["checkout", "-q", "main"]);

        std::fs::write(root.join(".commit_hash"), format!("{unreachable}\n")).unwrap();
        let store = FsDryCheckCommitHashStore::new(root.join(".commit_hash"), root.to_path_buf());
        assert!(
            store.read().unwrap().is_none(),
            "an unreachable record must fall back before any replacement is in play"
        );

        std::fs::create_dir_all(root.join(".git/info")).unwrap();
        std::fs::write(root.join(".git/info/grafts"), format!("{head} {unreachable}\n")).unwrap();

        assert!(
            store.read().unwrap().is_none(),
            "a graft must not turn an unreachable record into a usable base"
        );
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
                located
                    .as_ref()
                    .err()
                    .is_some_and(crate::track::symlink_guard::is_symlink_rejection),
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
