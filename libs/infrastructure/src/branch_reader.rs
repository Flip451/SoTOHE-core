//! Git secondary adapter for reading the current branch (IN-06, AC-06).
//!
//! [`LazyBranchReader`] implements [`usecase::track_resolution::BranchReaderPort`]
//! by discovering the repository on each call and translating the git failure
//! into the port's error. Both of those are adapter work, so they live here
//! rather than inside a composition root, which may only construct and inject.
//!
//! Discovery is deferred to the call so that wiring succeeds in contexts with
//! no repository: an undiscoverable repository surfaces as a branch-read
//! failure through the caller's own error channel.

use usecase::track_resolution::{BranchReadError, BranchReaderPort};

use crate::git_cli::SystemGitRepo;

/// Reads the current branch, discovering the repository at read time.
///
/// Constructed with no arguments so composition roots stay zero-argument
/// wiring accessors.
#[derive(Debug, Default)]
pub struct LazyBranchReader;

impl LazyBranchReader {
    /// Creates the adapter.
    #[must_use]
    pub fn new() -> LazyBranchReader {
        LazyBranchReader
    }
}

impl BranchReaderPort for LazyBranchReader {
    fn current_branch(&self) -> Result<Option<String>, BranchReadError> {
        let repo = SystemGitRepo::discover()
            .map_err(|error| BranchReadError::ReadFailed(error.to_string()))?;
        repo.current_branch().map_err(|error| BranchReadError::ReadFailed(error.to_string()))
    }
}

#[cfg(test)]
#[allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn test_the_reader_reports_the_branch_of_the_repository_it_discovers() {
        // The test process runs inside this repository, so a name is available
        // whenever HEAD is not detached.
        let branch = LazyBranchReader::new().current_branch().unwrap();

        assert!(branch.is_some(), "a discovered repository reports a branch or the HEAD sentinel");
    }

    #[test]
    fn test_a_repository_that_cannot_be_discovered_becomes_a_branch_read_failure() {
        // Reading from a directory outside any repository: discovery fails at
        // call time and is translated, rather than panicking or being decided
        // at wiring time.
        let outside = tempfile::Builder::new()
            .prefix("branch-reader-")
            .tempdir_in(std::env::temp_dir())
            .unwrap();
        let original = std::env::current_dir().unwrap();
        std::env::set_current_dir(outside.path()).unwrap();

        let result = LazyBranchReader::new().current_branch();

        std::env::set_current_dir(original).unwrap();
        match result {
            Err(BranchReadError::ReadFailed(message)) => {
                assert!(!message.is_empty(), "the failure names why discovery did not succeed");
            }
            // A temp directory may still sit inside a repository on some hosts;
            // then the read simply succeeds and there is nothing to translate.
            Ok(_) => {}
        }
    }
}
