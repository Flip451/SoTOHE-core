use domain::CommitHash;
use domain::review_v2::{FastVerdict, FilePath, LogInfo, ReviewHash, ReviewTarget, Verdict};

use super::error::{DiffGetError, ReviewHasherError, ReviewerError};
use crate::capability_exec::{ModelName, ProviderName, ReasoningEffort};

/// The resolved identity of one reviewer session.
///
/// This value is owned by the concrete reviewer adapter that will execute the
/// session. Keeping the assignment next to that adapter makes it impossible
/// for a caller to record telemetry for a different provider configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedReviewerAssignment {
    track_id: domain::TrackId,
    scope: domain::review_v2::ScopeName,
    provider: ProviderName,
    model: ModelName,
    reasoning_effort: ReasoningEffort,
}

impl ResolvedReviewerAssignment {
    /// Creates a resolved reviewer assignment from validated values.
    #[must_use]
    pub fn new(
        track_id: domain::TrackId,
        scope: domain::review_v2::ScopeName,
        provider: ProviderName,
        model: ModelName,
        reasoning_effort: ReasoningEffort,
    ) -> Self {
        Self { track_id, scope, provider, model, reasoning_effort }
    }

    /// Returns the track identity used by the reviewer.
    #[must_use]
    pub fn track_id(&self) -> &domain::TrackId {
        &self.track_id
    }

    /// Returns the review scope used by the reviewer.
    #[must_use]
    pub fn scope(&self) -> &domain::review_v2::ScopeName {
        &self.scope
    }

    /// Returns the resolved provider.
    #[must_use]
    pub fn provider(&self) -> &ProviderName {
        &self.provider
    }

    /// Returns the resolved model.
    #[must_use]
    pub fn model(&self) -> &ModelName {
        &self.model
    }

    /// Returns the resolved reasoning effort.
    #[must_use]
    pub fn reasoning_effort(&self) -> ReasoningEffort {
        self.reasoning_effort
    }
}

/// Secondary port exposing the assignment authority of a concrete reviewer.
pub trait ResolvedReviewer: Reviewer {
    /// Returns the validated assignment used by this reviewer.
    fn resolved_assignment(&self) -> &ResolvedReviewerAssignment;
}

/// Usecase port for the external reviewer (e.g., Codex).
pub trait Reviewer {
    /// Performs a final review on the given target files.
    ///
    /// # Errors
    /// Returns `ReviewerError` on abort, timeout, or illegal verdict format.
    fn review(&self, target: &ReviewTarget) -> Result<(Verdict, LogInfo), ReviewerError>;

    /// Performs a fast (advisory) review on the given target files.
    ///
    /// Fast verdicts are not used for approval decisions.
    ///
    /// # Errors
    /// Returns `ReviewerError` on abort, timeout, or illegal verdict format.
    fn fast_review(&self, target: &ReviewTarget) -> Result<(FastVerdict, LogInfo), ReviewerError>;
}

/// Usecase port for obtaining the list of changed files (diff).
///
/// Infrastructure implementation uses git merge-base + 4-source union.
pub trait DiffGetter {
    /// Lists all changed files relative to the given base commit.
    ///
    /// Returns the union of: committed diff from merge-base, staged, unstaged,
    /// and untracked files.
    ///
    /// # Errors
    /// Returns `DiffGetError` on git operation failure.
    fn list_diff_files(&self, base: &CommitHash) -> Result<Vec<FilePath>, DiffGetError>;
}

/// Usecase port for computing review hashes from file contents.
///
/// Infrastructure implementation uses sorted manifest + SHA256 + O_NOFOLLOW.
pub trait ReviewHasher {
    /// Computes the hash of a review target's file contents.
    ///
    /// Empty targets return `ReviewHash::Empty`.
    ///
    /// # Errors
    /// Returns `ReviewHasherError` on I/O or computation failure.
    fn calc(&self, target: &ReviewTarget) -> Result<ReviewHash, ReviewHasherError>;
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
mod tests {
    use super::*;
    use domain::review_v2::{MainScopeName, ScopeName};

    fn assignment() -> ResolvedReviewerAssignment {
        ResolvedReviewerAssignment::new(
            domain::TrackId::try_new("ports-test-track").expect("valid track id"),
            ScopeName::Main(MainScopeName::new("infrastructure").expect("valid scope")),
            ProviderName::try_new("codex").expect("valid provider"),
            ModelName::try_new("gpt-5.4").expect("valid model"),
            ReasoningEffort::High,
        )
    }

    struct ResolvedReviewerStub {
        assignment: ResolvedReviewerAssignment,
    }

    impl Reviewer for ResolvedReviewerStub {
        fn review(&self, _: &ReviewTarget) -> Result<(Verdict, LogInfo), ReviewerError> {
            Err(ReviewerError::Timeout)
        }

        fn fast_review(&self, _: &ReviewTarget) -> Result<(FastVerdict, LogInfo), ReviewerError> {
            Err(ReviewerError::Timeout)
        }
    }

    impl ResolvedReviewer for ResolvedReviewerStub {
        fn resolved_assignment(&self) -> &ResolvedReviewerAssignment {
            &self.assignment
        }
    }

    #[test]
    fn test_resolved_reviewer_assignment_accessors_return_constructor_values() {
        let value = assignment();

        assert_eq!(value.track_id().as_ref(), "ports-test-track");
        assert_eq!(value.scope().to_string(), "infrastructure");
        assert_eq!(value.provider().as_str(), "codex");
        assert_eq!(value.model().as_str(), "gpt-5.4");
        assert_eq!(value.reasoning_effort(), ReasoningEffort::High);
    }

    #[test]
    fn test_resolved_reviewer_port_exposes_the_reviewer_owned_assignment() {
        let reviewer = ResolvedReviewerStub { assignment: assignment() };

        assert_eq!(reviewer.resolved_assignment(), &reviewer.assignment);
    }
}
