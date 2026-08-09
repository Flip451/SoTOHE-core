//! Review check-zero-findings application service.
//!
//! The command asks whether one review scope has a current final
//! `zero_findings` verdict.  The concrete review-store evaluation is injected
//! by the composition root so this usecase remains independent of
//! infrastructure.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use domain::review_v2::{NotRequiredReason, RequiredReason, ReviewState, ScopeName};
use domain::{FreeText, TrackId};
use thiserror::Error;

use crate::git_workflow::DiagnosticText;

/// Typed query for one scope's final review verdict.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewCheckZeroFindingsQuery {
    track: TrackId,
    items_dir: PathBuf,
    scope: ScopeName,
}

impl ReviewCheckZeroFindingsQuery {
    /// Validates raw delivery-boundary values and constructs the domain query state.
    pub fn try_new(
        items_dir: PathBuf,
        track_id: String,
        scope: String,
    ) -> Result<Self, ReviewCheckZeroFindingsValidationError> {
        let track = TrackId::try_new(track_id).map_err(|error| {
            ReviewCheckZeroFindingsValidationError::InvalidTrackId(DiagnosticText::new(
                error.to_string(),
            ))
        })?;
        let scope = ScopeName::parse(&scope).map_err(|error| {
            ReviewCheckZeroFindingsValidationError::InvalidScope(DiagnosticText::new(
                error.to_string(),
            ))
        })?;

        Ok(Self { track, items_dir, scope })
    }
}

/// Result of evaluating one scope's final review verdict.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReviewCheckZeroFindingsOutcome {
    CurrentFinalZeroFindings,
    MissingFinalVerdict,
    StaleFinalVerdict,
    FindingsRemain,
}

/// Evaluation failure returned while checking a validated query.
#[derive(Debug, Error)]
pub enum ReviewCheckZeroFindingsEvaluationError {
    #[error("review verdict evaluation failed: {0}")]
    EvaluationFailed(DiagnosticText),
}

/// Validation error returned while constructing a check-zero-findings query.
#[derive(Debug, Error)]
pub enum ReviewCheckZeroFindingsValidationError {
    #[error("invalid check-zero-findings track id: {0}")]
    InvalidTrackId(DiagnosticText),
    #[error("invalid check-zero-findings scope: {0}")]
    InvalidScope(DiagnosticText),
}

/// Application service for the check-zero-findings operation.
pub trait ReviewCheckZeroFindingsService: Send + Sync {
    /// Evaluates whether the query's scope has a current final zero-findings verdict.
    ///
    /// # Errors
    ///
    /// Returns review-state evaluation failures.
    fn check_zero_findings(
        &self,
        query: &ReviewCheckZeroFindingsQuery,
    ) -> Result<ReviewCheckZeroFindingsOutcome, ReviewCheckZeroFindingsEvaluationError>;
}

/// Secondary port for reading the current review state of one scope.
///
/// Infrastructure adapters calculate the state from the persisted review
/// artifact and current diff. The interactor owns the resulting application
/// decision so adapters cannot convert review state into command outcomes.
pub trait ReviewCheckZeroFindingsStatePort: Send + Sync {
    /// Returns the current state for a configured scope, if that scope exists.
    ///
    /// # Errors
    ///
    /// Returns a bounded diagnostic when the underlying review runtime cannot
    /// load or evaluate the state.
    fn state_for(
        &self,
        track_id: &TrackId,
        items_dir: &Path,
        scope: &ScopeName,
    ) -> Result<Option<ReviewState>, FreeText>;
}

/// Dependency-injected implementation of [`ReviewCheckZeroFindingsService`].
pub struct ReviewCheckZeroFindingsInteractor {
    state_port: Arc<dyn ReviewCheckZeroFindingsStatePort>,
}

impl ReviewCheckZeroFindingsInteractor {
    /// Creates an interactor with an adapter for current review state access.
    #[must_use]
    pub fn new(state_port: Arc<dyn ReviewCheckZeroFindingsStatePort>) -> Self {
        Self { state_port }
    }
}

impl ReviewCheckZeroFindingsService for ReviewCheckZeroFindingsInteractor {
    fn check_zero_findings(
        &self,
        query: &ReviewCheckZeroFindingsQuery,
    ) -> Result<ReviewCheckZeroFindingsOutcome, ReviewCheckZeroFindingsEvaluationError> {
        let state = self
            .state_port
            .state_for(&query.track, &query.items_dir, &query.scope)
            .map_err(|error| {
                ReviewCheckZeroFindingsEvaluationError::EvaluationFailed(DiagnosticText::new(
                    error.to_string(),
                ))
            })?;

        match state {
            Some(ReviewState::NotRequired(NotRequiredReason::ZeroFindings)) => {
                Ok(ReviewCheckZeroFindingsOutcome::CurrentFinalZeroFindings)
            }
            Some(ReviewState::Required(RequiredReason::StaleHash)) => {
                Ok(ReviewCheckZeroFindingsOutcome::StaleFinalVerdict)
            }
            Some(ReviewState::Required(RequiredReason::FindingsRemain)) => {
                Ok(ReviewCheckZeroFindingsOutcome::FindingsRemain)
            }
            Some(ReviewState::Required(RequiredReason::NotStarted))
            | Some(ReviewState::NotRequired(NotRequiredReason::Empty)) => {
                Ok(ReviewCheckZeroFindingsOutcome::MissingFinalVerdict)
            }
            None => Ok(ReviewCheckZeroFindingsOutcome::MissingFinalVerdict),
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use domain::review_v2::{NotRequiredReason, RequiredReason, ScopeName};

    use super::*;

    enum StubStatePort {
        State(Option<ReviewState>),
        Failure,
    }

    impl ReviewCheckZeroFindingsStatePort for StubStatePort {
        fn state_for(
            &self,
            _track_id: &TrackId,
            _items_dir: &Path,
            _scope: &ScopeName,
        ) -> Result<Option<ReviewState>, FreeText> {
            match self {
                Self::State(state) => Ok(state.clone()),
                Self::Failure => Err(FreeText::new("review artifact could not be read")),
            }
        }
    }

    fn query() -> ReviewCheckZeroFindingsQuery {
        ReviewCheckZeroFindingsQuery::try_new(
            PathBuf::from("track/items"),
            "usecase-track".to_owned(),
            "usecase".to_owned(),
        )
        .unwrap()
    }

    #[test]
    fn test_check_zero_findings_current_zero_findings_state_returns_success() {
        let interactor = ReviewCheckZeroFindingsInteractor::new(Arc::new(StubStatePort::State(
            Some(ReviewState::NotRequired(NotRequiredReason::ZeroFindings)),
        )));

        let outcome = interactor.check_zero_findings(&query()).unwrap();

        assert_eq!(outcome, ReviewCheckZeroFindingsOutcome::CurrentFinalZeroFindings);
    }

    #[test]
    fn test_check_zero_findings_non_passing_review_states_are_preserved() {
        for (state, expected) in [
            (
                Some(ReviewState::Required(RequiredReason::NotStarted)),
                ReviewCheckZeroFindingsOutcome::MissingFinalVerdict,
            ),
            (
                Some(ReviewState::Required(RequiredReason::StaleHash)),
                ReviewCheckZeroFindingsOutcome::StaleFinalVerdict,
            ),
            (
                Some(ReviewState::Required(RequiredReason::FindingsRemain)),
                ReviewCheckZeroFindingsOutcome::FindingsRemain,
            ),
        ] {
            let interactor =
                ReviewCheckZeroFindingsInteractor::new(Arc::new(StubStatePort::State(state)));

            assert_eq!(interactor.check_zero_findings(&query()).unwrap(), expected);
        }
    }

    #[test]
    fn test_check_zero_findings_evaluator_error_is_propagated() {
        let interactor = ReviewCheckZeroFindingsInteractor::new(Arc::new(StubStatePort::Failure));

        let error = interactor.check_zero_findings(&query()).unwrap_err();

        assert!(matches!(
            error,
            ReviewCheckZeroFindingsEvaluationError::EvaluationFailed(diagnostic)
                if diagnostic.as_str() == "review artifact could not be read"
        ));
    }

    #[test]
    fn test_review_check_zero_findings_query_valid_raw_tokens_construct_domain_values() {
        let named_raw_scope = "usecase";
        let named = ReviewCheckZeroFindingsQuery::try_new(
            PathBuf::from("track/items"),
            "usecase-track".to_owned(),
            named_raw_scope.to_owned(),
        )
        .unwrap();
        let other_raw_scope = "other";
        let other = ReviewCheckZeroFindingsQuery::try_new(
            PathBuf::from("track/items"),
            "usecase-track".to_owned(),
            other_raw_scope.to_owned(),
        )
        .unwrap();

        assert_eq!(named.track.as_ref(), "usecase-track");
        assert_eq!(named.scope, ScopeName::parse(named_raw_scope).unwrap());
        assert_eq!(named.items_dir, PathBuf::from("track/items"));
        assert_eq!(other.scope, ScopeName::parse(other_raw_scope).unwrap());
    }

    #[test]
    fn test_review_check_zero_findings_query_invalid_raw_tokens_return_validation_error() {
        let invalid_track = ReviewCheckZeroFindingsQuery::try_new(
            PathBuf::from("track/items"),
            "INVALID_TRACK".to_owned(),
            "usecase".to_owned(),
        );
        let invalid_scope = ReviewCheckZeroFindingsQuery::try_new(
            PathBuf::from("track/items"),
            "usecase-track".to_owned(),
            "".to_owned(),
        );
        let non_ascii_scope = ReviewCheckZeroFindingsQuery::try_new(
            PathBuf::from("track/items"),
            "usecase-track".to_owned(),
            "非ASCII".to_owned(),
        );

        assert!(matches!(
            invalid_track,
            Err(ReviewCheckZeroFindingsValidationError::InvalidTrackId(_))
        ));
        assert!(matches!(
            invalid_scope,
            Err(ReviewCheckZeroFindingsValidationError::InvalidScope(_))
        ));
        assert!(matches!(
            non_ascii_scope,
            Err(ReviewCheckZeroFindingsValidationError::InvalidScope(_))
        ));
    }
}
