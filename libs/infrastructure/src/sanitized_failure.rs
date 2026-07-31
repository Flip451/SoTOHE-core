//! Failure classifications that name what went wrong without naming where.
//!
//! The batch-plan adapters report their failures to an operator through the
//! use-case error types, and those messages identify a file by its
//! repository-relative path — the well-known configuration constant, or the
//! candidate path the measurement was handed. The underlying errors do not
//! respect that: the symlink guard renders the absolute component it rejected,
//! [`ScopeConfigLoadError`] carries the absolute path it was given in every
//! variant (and the canonical root as well, for a path escape), and `io::Error`
//! Display text is platform-defined.
//!
//! So none of those errors is embedded. Each is mapped to a fixed classification
//! here, once, for every adapter that reports one. This changes no shared error
//! type's contract — the loader keeps its own messages for the callers that want
//! them.

use crate::dry_check::DryCheckCommitHashError;
use crate::git_cli::GitError;
use crate::review_v2::scope_config_loader::ScopeConfigLoadError;

/// A cause already reduced to its classification, carried inside an
/// [`std::io::Error`].
///
/// An adapter that knows which cause it holds — because it matched on a typed
/// error — states the classification here rather than formatting a message a
/// later reader would have to sniff. The classification is the only thing carried,
/// so nothing path-bearing can travel with it.
#[derive(Debug)]
pub(crate) struct SanitizedCause(&'static str);

impl std::fmt::Display for SanitizedCause {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0)
    }
}

impl std::error::Error for SanitizedCause {}

/// Wraps `classification` as an `io::Error`, for a caller whose signature reports
/// one but whose cause is already known.
pub(crate) fn sanitized_io_error(classification: &'static str) -> std::io::Error {
    std::io::Error::other(SanitizedCause(classification))
}

/// Names how an I/O attempt failed, with nothing path-bearing in it.
///
/// A carried classification is preferred to the kind, and the symlink guard's
/// payload to both: `InvalidInput` is raised for a refusal, for a malformed path,
/// and by git discovery alike, so the kind alone separates none of them. Each
/// downcast answers exactly where a message would have to be sniffed.
pub(crate) fn io_classification(error: &std::io::Error) -> &'static str {
    if crate::track::symlink_guard::is_symlink_rejection(error) {
        return "rejected as a symlink";
    }
    if let Some(stated) = error.get_ref().and_then(|inner| inner.downcast_ref::<SanitizedCause>()) {
        return stated.0;
    }
    match error.kind() {
        std::io::ErrorKind::InvalidInput => "not usable as a repository path",
        std::io::ErrorKind::NotFound => "not found",
        std::io::ErrorKind::PermissionDenied => "permission denied",
        _ => "unreadable",
    }
}

/// Names how the per-track `.commit_hash` record could not be used, with nothing
/// path-bearing in it.
///
/// Every variant of [`DryCheckCommitHashError`] renders the absolute path it was
/// reading, so none is embedded where the outcome is reported: the record is
/// identified by its well-known name and this word says what was wrong with it.
pub(crate) fn commit_hash_classification(error: &DryCheckCommitHashError) -> &'static str {
    match error {
        DryCheckCommitHashError::Io { .. } => "unreadable",
        DryCheckCommitHashError::SymlinkDetected { .. } => "rejected as a symlink",
        DryCheckCommitHashError::Format(_) => "malformed",
    }
}

/// Names how a git invocation failed, with nothing path-bearing in it.
///
/// Generic to any git call: `CommandFailed` says git ran and refused, which means
/// something different for each command, so a caller whose command gives that
/// outcome a specific meaning states its own classification instead of taking this
/// one. `stderr` is never carried — git writes paths into it.
pub(crate) fn git_classification(error: &GitError) -> &'static str {
    match error {
        GitError::Spawn { .. } => "git could not be executed",
        GitError::CommandFailed { .. } => "git command failed",
        GitError::EmptyRepoRoot => "git reported an empty repository root",
        GitError::CurrentDir(_) => "the working directory could not be determined",
    }
}

/// Names how the scope configuration failed to load, with nothing path-bearing
/// in it.
pub(crate) fn scope_config_classification(error: &ScopeConfigLoadError) -> &'static str {
    match error {
        ScopeConfigLoadError::Io { source, .. } => io_classification(source),
        ScopeConfigLoadError::Parse { .. } => "not valid JSON",
        ScopeConfigLoadError::InvalidField { .. } => "rejected by validation",
        ScopeConfigLoadError::Config(_) => "not a usable scope configuration",
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    /// A symlink refusal as the guard raises it, carrying its own payload.
    fn symlink_refusal() -> std::io::Error {
        crate::track::symlink_guard::reject_symlinks_up_to_root(&link_into_a_directory())
            .expect_err("a symlinked component must be refused")
    }

    /// A path whose leaf is a symlink, so the guard refuses it.
    fn link_into_a_directory() -> std::path::PathBuf {
        let directory = std::env::temp_dir().join(format!(
            "sanitized-failure-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        let link = directory.join("items-link");
        let _ = std::fs::remove_file(&link);
        #[cfg(unix)]
        std::os::unix::fs::symlink(&directory, &link).unwrap();
        link
    }

    #[cfg(unix)]
    #[test]
    fn test_a_classification_carries_no_path_from_the_error_it_names() {
        // The guard's own message embeds the absolute component it refused; the
        // classification keeps only the fact of the refusal.
        let refusal = symlink_refusal();
        assert!(
            refusal.to_string().contains('/'),
            "the fixture must be an error that would leak: {refusal}"
        );
        assert_eq!(io_classification(&refusal), "rejected as a symlink");

        let load_failure = ScopeConfigLoadError::InvalidField {
            path: "/absolute/secret/review-scope.json".to_owned(),
            detail: "path escapes trusted root (/absolute/secret)".to_owned(),
        };
        let classification = scope_config_classification(&load_failure);
        assert_eq!(classification, "rejected by validation");
        assert!(!classification.contains('/'), "a classification names no path");
    }

    #[cfg(unix)]
    #[test]
    fn test_the_two_invalid_input_causes_are_not_given_the_same_word() {
        // Both a symlink refusal and an absent enclosing repository are raised as
        // `InvalidInput`, so the kind alone cannot separate them. A path the
        // filesystem itself calls invalid is a third, unrelated case.
        let refusal = io_classification(&symlink_refusal());
        let absent_repository = io_classification(
            &crate::discover_repo_for_items_dir(&std::env::temp_dir())
                .expect_err("the system temp directory is not inside a git repository"),
        );
        let malformed = io_classification(&std::io::Error::from(std::io::ErrorKind::InvalidInput));

        assert_eq!(refusal, "rejected as a symlink");
        assert_eq!(absent_repository, "no enclosing git repository");
        assert_eq!(malformed, "not usable as a repository path");
        assert_ne!(refusal, absent_repository, "the two guarded causes stay distinct");
    }

    #[test]
    fn test_a_git_fault_is_not_reported_as_an_absent_repository() {
        // Only a refusing `rev-parse --show-toplevel` means nothing encloses the
        // directory. Git failing to run, answering with an empty root, or refusing
        // some other command are separate faults, and collapsing them into the
        // discovery word would tell an operator to look in the wrong place.
        let spawn_failure = git_classification(&GitError::Spawn {
            command: "rev-parse --show-toplevel".to_owned(),
            source: std::io::Error::new(std::io::ErrorKind::NotFound, "git: command not found"),
        });
        let refused_command = git_classification(&GitError::CommandFailed {
            command: "rev-parse --show-toplevel".to_owned(),
            code: 128,
            stderr: "fatal: not a git repository: /absolute/secret/path".to_owned(),
        });
        let empty_root = git_classification(&GitError::EmptyRepoRoot);
        let no_working_directory = git_classification(&GitError::CurrentDir(std::io::Error::from(
            std::io::ErrorKind::PermissionDenied,
        )));

        assert_eq!(spawn_failure, "git could not be executed");
        assert_eq!(empty_root, "git reported an empty repository root");
        assert_eq!(no_working_directory, "the working directory could not be determined");

        // The generic word for a refusing command, and none of these carries the
        // stderr git wrote its paths into.
        assert_eq!(refused_command, "git command failed");
        for word in [spawn_failure, refused_command, empty_root, no_working_directory] {
            assert!(!word.contains('/'), "a classification names no path: {word}");
            assert_ne!(word, "no enclosing git repository", "only the discovery site says that");
        }
    }

    #[test]
    fn test_a_stated_classification_survives_the_io_error_it_travels_in() {
        let carried = sanitized_io_error("git could not be executed");

        assert_eq!(io_classification(&carried), "git could not be executed");
    }

    #[test]
    fn test_each_io_kind_the_adapters_meet_has_its_own_word() {
        let words = [
            std::io::ErrorKind::NotFound,
            std::io::ErrorKind::PermissionDenied,
            std::io::ErrorKind::InvalidInput,
            std::io::ErrorKind::Other,
        ]
        .map(|kind| io_classification(&std::io::Error::new(kind, "detail")));

        let mut unique = words.to_vec();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(unique.len(), words.len(), "each kind is distinguishable: {words:?}");
    }
}
