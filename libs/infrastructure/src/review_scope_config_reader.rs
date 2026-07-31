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

use crate::review_v2::load_v2_scope_config;
use crate::sanitized_failure::{io_classification, scope_config_classification};

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
        items_dir: &Path,
        track_id: &TrackId,
    ) -> Result<ReviewScopeConfig, ScopeConfigReadError> {
        // Anchored on the items directory, so the ceilings come from the same
        // repository the track artifacts do — and isolated from the ambient Git
        // environment, which could otherwise name a different one. The measured
        // diff this configuration is compared against is taken from the items
        // directory's repository; reading the ceilings from anywhere else would
        // judge one repository's work by another's limits.
        let (repo, anchor) =
            crate::discover_isolated_repo_for_items_dir(items_dir).map_err(|error| {
                ScopeConfigReadError::ReadFailed {
                    message: FreeText::new(format!(
                        "git repository could not be discovered: {}",
                        io_classification(&error)
                    )),
                }
            })?;
        ensure_encloses(repo.root(), &anchor)?;
        let root = repo.root();

        // The loader's error names the absolute path it was given; the file is
        // already identified by its repository-relative constant, so only the
        // classification travels out to the operator.
        load_v2_scope_config(&root.join(REVIEW_SCOPE_CONFIG), track_id, root).map_err(|error| {
            ScopeConfigReadError::ReadFailed {
                message: FreeText::new(format!(
                    "load {REVIEW_SCOPE_CONFIG}: {}",
                    scope_config_classification(&error)
                )),
            }
        })
    }
}

/// Refuses a repository that does not enclose the items directory the ceilings
/// were asked for.
///
/// Discovery walks up from the anchor, so its root is an ancestor of it. This
/// asserts that outcome rather than assuming it: a root elsewhere would mean the
/// configuration read belongs to a different tree than the plan and diff it will
/// be compared against.
fn ensure_encloses(root: &Path, anchor: &Path) -> Result<(), ScopeConfigReadError> {
    let canonical_root = root.canonicalize().map_err(|error| ScopeConfigReadError::ReadFailed {
        message: FreeText::new(format!(
            "repository root could not be resolved: {}",
            io_classification(&error)
        )),
    })?;
    if anchor.starts_with(&canonical_root) {
        return Ok(());
    }
    // Neither path is named: both are absolute, and what an operator can act on
    // is that the two do not belong together.
    Err(ScopeConfigReadError::ReadFailed {
        message: FreeText::new(
            "the discovered repository does not enclose the items directory".to_owned(),
        ),
    })
}

#[cfg(test)]
#[allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use std::path::PathBuf;

    use domain::review_v2::{MainScopeName, ScopeName};

    use super::*;

    /// This workspace's own items directory: the reader is anchored on it, so
    /// the configuration it loads is this repository's shipped one.
    fn workspace_items_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../track/items")
    }

    #[test]
    fn test_the_reader_hands_back_the_configuration_the_ceilings_are_read_from() {
        let config = FsReviewScopeConfigReader::new()
            .read(&workspace_items_dir(), &TrackId::try_new("some-track").unwrap())
            .unwrap();

        let domain = ScopeName::Main(MainScopeName::new("domain").unwrap());
        assert!(config.contains_scope(&domain), "the shipped configuration declares `domain`");
        assert!(
            config.diff_ceiling_for_scope(&domain).is_some(),
            "a configured scope resolves a ceiling"
        );
    }

    #[test]
    fn test_the_reader_follows_the_items_directory_into_its_own_repository() {
        // A repository of its own, holding a ceiling no other configuration
        // declares: reading it proves the items directory decides which tree is
        // read, not the directory this process stands in.
        let repo = tempfile::Builder::new()
            .prefix("review-scope-anchor-")
            .tempdir_in(std::env::current_dir().unwrap())
            .unwrap();
        let root = repo.path();
        std::fs::create_dir_all(root.join(".harness/config")).unwrap();
        std::fs::write(
            root.join(".harness/config/review-scope.json"),
            r#"{"version": 2, "groups": {"domain": {"patterns": ["libs/domain/**"],
                "diff_ceiling_lines": 42}},
                "review_operational": [], "other_track": [], "default_diff_ceiling_lines": 500}"#,
        )
        .unwrap();
        std::fs::write(
            root.join("architecture-rules.json"),
            r#"{"layers":[{"crate":"domain","path":"libs/domain"}]}"#,
        )
        .unwrap();
        std::fs::create_dir_all(root.join("track/items/some-track")).unwrap();
        let status = std::process::Command::new("git")
            .args(["init", "-q"])
            .current_dir(root)
            .status()
            .unwrap();
        assert!(status.success(), "the fixture needs a repository of its own");

        let config = FsReviewScopeConfigReader::new()
            .read(&root.join("track/items"), &TrackId::try_new("some-track").unwrap())
            .unwrap();

        let domain = ScopeName::Main(MainScopeName::new("domain").unwrap());
        assert_eq!(config.diff_ceiling_for_scope(&domain), Some(42));
    }

    #[cfg(unix)]
    #[test]
    fn test_a_symlinked_items_directory_is_refused_rather_than_followed() {
        // The anchor decides which repository is read, so a symlinked items
        // directory is refused on the path as supplied instead of being resolved
        // into whichever tree it points at.
        let real = tempfile::Builder::new()
            .prefix("review-scope-symlink-")
            .tempdir_in(std::env::current_dir().unwrap())
            .unwrap();
        std::fs::create_dir_all(real.path().join("track/items/some-track")).unwrap();
        let linked = real.path().join("items-link");
        std::os::unix::fs::symlink(real.path().join("track/items"), &linked).unwrap();

        let error = FsReviewScopeConfigReader::new()
            .read(&linked, &TrackId::try_new("some-track").unwrap())
            .unwrap_err();

        let ScopeConfigReadError::ReadFailed { message } = error;
        // The refusal names the classification, not the guard's own message: that
        // one renders the absolute component it rejected.
        assert!(message.as_str().contains("rejected as a symlink"), "unexpected: {message}");
        assert!(
            !message.as_str().contains(&real.path().display().to_string()),
            "no absolute path may reach the operator: {message}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_a_symlinked_ancestor_of_the_items_directory_is_refused_as_well() {
        // The link sits above the items directory: canonicalising the supplied
        // path would follow it, so every component is checked first.
        let real = tempfile::Builder::new()
            .prefix("review-scope-ancestor-")
            .tempdir_in(std::env::current_dir().unwrap())
            .unwrap();
        std::fs::create_dir_all(real.path().join("repo/track/items/some-track")).unwrap();
        let linked_root = real.path().join("repo-link");
        std::os::unix::fs::symlink(real.path().join("repo"), &linked_root).unwrap();

        let error = FsReviewScopeConfigReader::new()
            .read(&linked_root.join("track/items"), &TrackId::try_new("some-track").unwrap())
            .unwrap_err();

        let ScopeConfigReadError::ReadFailed { message } = error;
        assert!(message.as_str().contains("rejected as a symlink"), "unexpected: {message}");
        assert!(
            !message.as_str().contains(&real.path().display().to_string()),
            "no absolute path may reach the operator: {message}"
        );
    }

    #[test]
    fn test_the_two_unresolved_ceiling_lanes_are_the_only_ones_the_configuration_offers() {
        // A configuration with one scope that declares no ceiling and no global
        // default, so both legitimate unresolved-ceiling lanes are observable on
        // the configuration this reader returns.
        let repo = tempfile::Builder::new()
            .prefix("review-scope-lanes-")
            .tempdir_in(std::env::current_dir().unwrap())
            .unwrap();
        let root = repo.path();
        std::fs::create_dir_all(root.join(".harness/config")).unwrap();
        std::fs::write(
            root.join(".harness/config/review-scope.json"),
            r#"{"version": 2, "groups": {"domain": {"patterns": ["libs/domain/**"]}},
                "review_operational": [], "other_track": []}"#,
        )
        .unwrap();
        std::fs::write(
            root.join("architecture-rules.json"),
            r#"{"layers":[{"crate":"domain","path":"libs/domain"}]}"#,
        )
        .unwrap();
        std::fs::create_dir_all(root.join("track/items/some-track")).unwrap();
        let status = std::process::Command::new("git")
            .args(["init", "-q"])
            .current_dir(root)
            .status()
            .unwrap();
        assert!(status.success(), "the fixture needs a repository of its own");

        let config = FsReviewScopeConfigReader::new()
            .read(&root.join("track/items"), &TrackId::try_new("some-track").unwrap())
            .unwrap();

        // Lane one: a configured scope with neither a per-scope ceiling nor a
        // global default to inherit.
        let domain = ScopeName::Main(MainScopeName::new("domain").unwrap());
        assert!(config.contains_scope(&domain));
        assert_eq!(config.diff_ceiling_for_scope(&domain), None);
        // Lane two: the reserved implicit scope, valid but never configured.
        assert!(config.contains_scope(&ScopeName::Other));
        assert_eq!(config.diff_ceiling_for_scope(&ScopeName::Other), None);
        // No third lane: a name the configuration does not hold is not a scope
        // whose total goes uncompared, it is a name the gate reports.
        let unknown = ScopeName::Main(MainScopeName::new("domian").unwrap());
        assert!(!config.contains_scope(&unknown));
        assert_eq!(config.diff_ceiling_for_scope(&unknown), None);
    }

    #[test]
    fn test_a_repository_that_does_not_enclose_the_items_directory_is_refused() {
        // Discovery walking up from the anchor cannot produce this, which is why
        // it is asserted rather than assumed: were a redirected discovery ever to
        // return a root elsewhere, the ceilings read would belong to a different
        // tree than the plan and diff they are compared against.
        let anchor = tempfile::Builder::new()
            .prefix("review-scope-anchor-encloses-")
            .tempdir_in(std::env::current_dir().unwrap())
            .unwrap();
        let elsewhere = tempfile::Builder::new()
            .prefix("review-scope-elsewhere-")
            .tempdir_in(std::env::current_dir().unwrap())
            .unwrap();
        let canonical_anchor = anchor.path().canonicalize().unwrap();

        let error = ensure_encloses(elsewhere.path(), &canonical_anchor)
            .expect_err("a repository that does not enclose the anchor cannot supply its ceilings");

        let ScopeConfigReadError::ReadFailed { message } = error;
        assert!(message.as_str().contains("does not enclose"), "unexpected: {message}");
        assert!(
            !message.as_str().contains(&canonical_anchor.display().to_string())
                && !message.as_str().contains(&elsewhere.path().display().to_string()),
            "no absolute path may reach the operator: {message}"
        );

        // The ordinary case still passes: the repository the items directory sits
        // in encloses it.
        ensure_encloses(anchor.path(), &canonical_anchor)
            .expect("the enclosing repository must be accepted");
    }

    #[test]
    fn test_a_scope_with_no_configured_ceiling_is_unconstrained_rather_than_a_failure() {
        let config = FsReviewScopeConfigReader::new()
            .read(&workspace_items_dir(), &TrackId::try_new("some-track").unwrap())
            .unwrap();

        // `other` is the implicit scope: never configured, never inheriting the
        // global default, and reading it is not an error.
        assert_eq!(config.diff_ceiling_for_scope(&ScopeName::Other), None);
    }
}
