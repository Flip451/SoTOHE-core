//! Filesystem secondary adapter for the review scope configuration
//! (IN-06, AC-08, OUT-08).
//!
//! [`FsReviewScopeConfigReader`] implements
//! [`usecase::batch_plan::ScopeConfigReaderPort`] by delegating to the existing
//! scope-config loader; it does not reparse `review-scope.json` itself. A scope
//! with no configured ceiling is an unconstrained scope, not a failure, so the
//! only error this adapter reports is a configuration it could not load.

use std::path::Path;

use domain::review_v2::ReviewScopeConfig;
use domain::{FreeText, TrackId};
use usecase::batch_plan::{ScopeConfigReadError, ScopeConfigReaderPort};

use crate::git_cli::SystemGitRepo;
use crate::review_v2::load_v2_scope_config;

/// Repository-relative location of the review scope configuration.
pub(crate) const REVIEW_SCOPE_CONFIG: &str = ".harness/config/review-scope.json";

/// Loads the review scope configuration for a track.
///
/// Constructed with no arguments so composition roots stay zero-argument
/// wiring accessors; the items directory arrives with each call.
#[derive(Debug, Default)]
pub struct FsReviewScopeConfigReader;

impl FsReviewScopeConfigReader {
    /// Creates the adapter.
    #[must_use]
    pub fn new() -> FsReviewScopeConfigReader {
        FsReviewScopeConfigReader
    }
}

impl ScopeConfigReaderPort for FsReviewScopeConfigReader {
    fn read(
        &self,
        _items_dir: &Path,
        track_id: &TrackId,
    ) -> Result<ReviewScopeConfig, ScopeConfigReadError> {
        let repo = SystemGitRepo::discover().map_err(|error| ScopeConfigReadError::ReadFailed {
            message: FreeText::new(format!("git repository could not be discovered: {error}")),
        })?;
        let root = repo.root();

        load_v2_scope_config(&root.join(REVIEW_SCOPE_CONFIG), track_id, root).map_err(|error| {
            ScopeConfigReadError::ReadFailed {
                message: FreeText::new(format!("load {REVIEW_SCOPE_CONFIG}: {error}")),
            }
        })
    }
}

#[cfg(test)]
#[allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use std::path::PathBuf;

    use domain::review_v2::{MainScopeName, ScopeName};

    use super::*;

    #[test]
    fn test_the_reader_hands_back_the_configuration_the_ceilings_are_read_from() {
        let config = FsReviewScopeConfigReader::new()
            .read(&PathBuf::from("track/items"), &TrackId::try_new("some-track").unwrap())
            .unwrap();

        let domain = ScopeName::Main(MainScopeName::new("domain").unwrap());
        assert!(config.contains_scope(&domain), "the shipped configuration declares `domain`");
        assert!(
            config.diff_ceiling_for_scope(&domain).is_some(),
            "a configured scope resolves a ceiling"
        );
    }

    #[test]
    fn test_a_scope_with_no_configured_ceiling_is_unconstrained_rather_than_a_failure() {
        let config = FsReviewScopeConfigReader::new()
            .read(&PathBuf::from("track/items"), &TrackId::try_new("some-track").unwrap())
            .unwrap();

        // `other` is the implicit scope: never configured, never inheriting the
        // global default, and reading it is not an error.
        assert_eq!(config.diff_ceiling_for_scope(&ScopeName::Other), None);
    }
}
