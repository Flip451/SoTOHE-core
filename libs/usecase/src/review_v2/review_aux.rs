//! Auxiliary application service ports for the `review` command family.
//!
//! Covers operations that do not fit the primary `RunReviewService` /
//! `RunReviewFixService` / `ReviewCheckApprovedService` pattern:
//! - `ReviewResultsService` — render review results output string
//! - `ReviewValidateScopeService` — validate a scope name
//! - `ReviewGetBriefingService` — get briefing path for a scope
//! - `ReviewRunLocalService` — run the provider-auto-resolved reviewer
//!
//! All interactors use the function-pointer pattern (mirroring `RunReviewInteractor`)
//! so that `cli_composition` injects the infrastructure wiring without violating
//! the hexagonal boundary.

use std::path::PathBuf;
use std::sync::Arc;

use domain::review_v2::{
    NotRequiredReason, RequiredReason, ReviewApprovalVerdict, ReviewState, ScopeName,
    derive_review_approval_verdict,
};
use thiserror::Error;

use crate::git_workflow::DiagnosticText;

/// Usecase-owned review-scope boundary value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewScopeName(String);

impl ReviewScopeName {
    pub fn try_new(value: String) -> Result<Self, ReviewScopeNameValidationError> {
        let scope = ScopeName::parse(&value).map_err(|error| {
            ReviewScopeNameValidationError::Invalid(DiagnosticText::new(error.to_string()))
        })?;
        Ok(match scope {
            ScopeName::Main(_) => Self(value),
            ScopeName::Other => Self::other(),
        })
    }

    /// Returns the domain-defined catch-all scope for read-model output.
    #[must_use]
    pub fn other() -> Self {
        Self("other".to_owned())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Error)]
pub enum ReviewScopeNameValidationError {
    #[error("invalid review scope: {0}")]
    Invalid(DiagnosticText),
}

/// Format-valid scope selector supplied by the delivery boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReviewScopeSelectionRequest {
    /// Render only the named scope after configured-universe resolution.
    NamedCandidate(ReviewScopeName),
    /// Render the configured scope universe.
    All,
}

impl ReviewScopeSelectionRequest {
    /// Converts raw delivery values into the exclusive, format-valid request.
    pub fn try_new(
        scope: Option<String>,
        all: bool,
    ) -> Result<Self, ReviewScopeSelectionValidationError> {
        match (scope, all) {
            (Some(_), true) => Err(ReviewScopeSelectionValidationError::ScopeAndAll),
            (Some(scope), false) => {
                ReviewScopeName::try_new(scope).map(Self::NamedCandidate).map_err(|error| {
                    ReviewScopeSelectionValidationError::InvalidScope(DiagnosticText::new(
                        error.to_string(),
                    ))
                })
            }
            (None, _) => Ok(Self::All),
        }
    }
}

/// Rejection returned when a review-results scope selector is invalid.
#[derive(Debug, Error)]
pub enum ReviewScopeSelectionValidationError {
    /// `--scope` and `--all` were both supplied.
    #[error("--scope and --all cannot be used together")]
    ScopeAndAll,
    /// The supplied scope name cannot form a domain `ScopeName`.
    #[error("invalid review results scope: {0}")]
    InvalidScope(DiagnosticText),
}

/// Review-finding output with presentation-neutral metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewerFindingOutput {
    pub message: DiagnosticText,
    pub severity: Option<String>,
    pub file: Option<String>,
    pub line: Option<u64>,
    pub category: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NonEmptyReviewerFindingsOutput(Vec<ReviewerFindingOutput>);

impl NonEmptyReviewerFindingsOutput {
    pub fn try_new(
        findings: Vec<ReviewerFindingOutput>,
    ) -> Result<Self, ReviewFindingsOutputValidationError> {
        if findings.is_empty() {
            return Err(ReviewFindingsOutputValidationError::Empty);
        }
        Ok(Self(findings))
    }
    #[must_use]
    pub fn as_slice(&self) -> &[ReviewerFindingOutput] {
        &self.0
    }
}

#[derive(Debug, Error)]
pub enum ReviewFindingsOutputValidationError {
    #[error("review findings must not be empty")]
    Empty,
}

/// Structured application output for `review results`.
#[derive(Debug)]
pub struct ReviewResultsOutput {
    pub base: String,
    pub scopes: Vec<ReviewScopeResultOutput>,
    pub hint_should_emit: bool,
}

/// Structured output for one configured review scope.
#[derive(Debug)]
pub struct ReviewScopeResultOutput {
    pub scope: ReviewScopeName,
    pub state: ReviewScopeResultState,
    pub rounds: Vec<ReviewRoundResultOutput>,
}

/// Finite review state projection used by the primary adapter.
#[derive(Debug)]
pub enum ReviewScopeResultState {
    RequiredNotStarted,
    RequiredFindingsRemain,
    RequiredStaleHash,
    Empty,
    Approved,
}

/// Structured output for one persisted review round.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewRoundResultOutput {
    pub round_type: crate::review_v2::ReviewRoundType,
    pub at: String,
    pub verdict: ReviewRoundResultVerdict,
}

/// Typed review-round verdict projection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReviewRoundResultVerdict {
    ZeroFindings,
    FindingsRemain(NonEmptyReviewerFindingsOutput),
}

/// Scope data made available by the read adapter.
#[derive(Debug, Clone)]
pub struct ReviewResultsScopeSnapshot {
    pub base: String,
    pub configured_scopes: Vec<ReviewScopeName>,
    pub review_json_exists: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReviewRequiredReason {
    NotStarted,
    FindingsRemain,
    StaleHash,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReviewNotRequiredReason {
    Empty,
    ZeroFindings,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReviewStoredScopeState {
    Required(ReviewRequiredReason),
    NotRequired(ReviewNotRequiredReason),
}
#[derive(Debug, Clone)]
pub struct ReviewStoredScopeStateEntry {
    pub scope: ReviewScopeName,
    pub state: ReviewStoredScopeState,
}
/// Stored-round projection alias reusing the shared review-round verdict DTO.
pub type ReviewStoredRoundVerdict = ReviewRoundResultVerdict;
/// Stored-round projection alias reusing the shared review-round output DTO.
pub type ReviewStoredRound = ReviewRoundResultOutput;

pub trait ReviewResultsScopePort: Send + Sync {
    fn load_scope_snapshot(
        &self,
        track_id: Option<&str>,
        items_dir: &std::path::Path,
    ) -> Result<ReviewResultsScopeSnapshot, ReviewResultsError>;
}
pub trait ReviewResultsStatePort: Send + Sync {
    fn load_scope_states(
        &self,
        track_id: Option<&str>,
        items_dir: &std::path::Path,
    ) -> Result<Vec<ReviewStoredScopeStateEntry>, ReviewResultsError>;
}
pub trait ReviewResultsRoundPort: Send + Sync {
    fn load_scope_rounds(
        &self,
        track_id: Option<&str>,
        items_dir: &std::path::Path,
        scope: &ReviewScopeName,
    ) -> Result<Vec<ReviewStoredRound>, ReviewResultsError>;
}

// ── ReviewAuxError ────────────────────────────────────────────────────────────

/// Error type for review auxiliary service operations.
///
/// Used by [`ReviewClassifyService`], [`ReviewFilesService`],
/// [`ReviewValidateScopeService`], and [`ReviewGetBriefingService`].
#[derive(Debug, thiserror::Error)]
pub enum ReviewAuxError {
    /// The operation failed (scope invalid, briefing not found, I/O failure, etc.).
    #[error("{0}")]
    Failed(String),
}

/// Error boundary for review-results projection and its backing ports.
#[derive(Debug, thiserror::Error)]
pub enum ReviewResultsError {
    /// A results operation failed while loading or projecting persisted state.
    #[error("{0}")]
    Failed(DiagnosticText),
    /// A format-valid requested scope is absent from the configured universe.
    #[error("unknown review results scope: {}", .0.as_str())]
    UnknownScope(ReviewScopeName),
    /// A state port response omitted a configured scope and cannot safely be projected.
    #[error("review results state missing for configured scope '{}'", .0.as_str())]
    MissingScopeState(ReviewScopeName),
}

// ── ReviewClassifyService ─────────────────────────────────────────────────────

/// Application service (primary port) for `sotp review classify`.
pub trait ReviewClassifyService: Send + Sync {
    /// Classify each path string into its review scope(s).
    ///
    /// Returns one `(path, scopes_csv)` pair per input path.
    fn classify(
        &self,
        paths: Vec<String>,
        track_id: Option<String>,
        items_dir: PathBuf,
    ) -> Result<Vec<(String, String)>, ReviewAuxError>;
}

/// Function-pointer interactor implementing [`ReviewClassifyService`].
pub struct ReviewClassifyInteractor {
    #[allow(clippy::type_complexity)]
    run_fn: Arc<
        dyn Fn(
                Vec<String>,
                Option<String>,
                PathBuf,
            ) -> Result<Vec<(String, String)>, ReviewAuxError>
            + Send
            + Sync,
    >,
}

impl ReviewClassifyInteractor {
    /// Create with injected function.
    #[must_use]
    #[allow(clippy::type_complexity)]
    pub fn new(
        run_fn: Arc<
            dyn Fn(
                    Vec<String>,
                    Option<String>,
                    PathBuf,
                ) -> Result<Vec<(String, String)>, ReviewAuxError>
                + Send
                + Sync,
        >,
    ) -> Self {
        Self { run_fn }
    }
}

impl ReviewClassifyService for ReviewClassifyInteractor {
    fn classify(
        &self,
        paths: Vec<String>,
        track_id: Option<String>,
        items_dir: PathBuf,
    ) -> Result<Vec<(String, String)>, ReviewAuxError> {
        (self.run_fn)(paths, track_id, items_dir)
    }
}

// ── ReviewFilesService ────────────────────────────────────────────────────────

/// Application service (primary port) for `sotp review files`.
pub trait ReviewFilesService: Send + Sync {
    /// List the diff files belonging to the given scope (one per entry).
    fn files(
        &self,
        scope: String,
        track_id: Option<String>,
        items_dir: PathBuf,
    ) -> Result<Vec<String>, ReviewAuxError>;
}

/// Function-pointer interactor implementing [`ReviewFilesService`].
pub struct ReviewFilesInteractor {
    #[allow(clippy::type_complexity)]
    run_fn: Arc<
        dyn Fn(String, Option<String>, PathBuf) -> Result<Vec<String>, ReviewAuxError>
            + Send
            + Sync,
    >,
}

impl ReviewFilesInteractor {
    /// Create with injected function.
    #[must_use]
    #[allow(clippy::type_complexity)]
    pub fn new(
        run_fn: Arc<
            dyn Fn(String, Option<String>, PathBuf) -> Result<Vec<String>, ReviewAuxError>
                + Send
                + Sync,
        >,
    ) -> Self {
        Self { run_fn }
    }
}

impl ReviewFilesService for ReviewFilesInteractor {
    fn files(
        &self,
        scope: String,
        track_id: Option<String>,
        items_dir: PathBuf,
    ) -> Result<Vec<String>, ReviewAuxError> {
        (self.run_fn)(scope, track_id, items_dir)
    }
}

// ── ReviewResultsService ──────────────────────────────────────────────────────

/// Application service (primary port) for `sotp review results`.
pub trait ReviewResultsService: Send + Sync {
    /// Resolve the selector and return structured review results.
    ///
    /// Presentation options are deliberately kept at the driver boundary.
    fn results(
        &self,
        track_id: Option<String>,
        items_dir: PathBuf,
        request: ReviewScopeSelectionRequest,
    ) -> Result<ReviewResultsOutput, ReviewResultsError>;
}

/// Port-backed interactor that resolves the scope universe and projects state.
pub struct ReviewResultsInteractor {
    scope_port: Arc<dyn ReviewResultsScopePort>,
    state_port: Arc<dyn ReviewResultsStatePort>,
    round_port: Arc<dyn ReviewResultsRoundPort>,
}

impl ReviewResultsInteractor {
    #[must_use]
    pub fn new(
        scope_port: Arc<dyn ReviewResultsScopePort>,
        state_port: Arc<dyn ReviewResultsStatePort>,
        round_port: Arc<dyn ReviewResultsRoundPort>,
    ) -> Self {
        Self { scope_port, state_port, round_port }
    }
}

impl ReviewResultsService for ReviewResultsInteractor {
    fn results(
        &self,
        track_id: Option<String>,
        items_dir: PathBuf,
        request: ReviewScopeSelectionRequest,
    ) -> Result<ReviewResultsOutput, ReviewResultsError> {
        let snapshot = self.scope_port.load_scope_snapshot(track_id.as_deref(), &items_dir)?;
        let configured_scopes = snapshot.configured_scopes;
        let mut scopes = match request {
            ReviewScopeSelectionRequest::All => configured_scopes.clone(),
            ReviewScopeSelectionRequest::NamedCandidate(scope) => {
                if !configured_scopes.contains(&scope) {
                    return Err(ReviewResultsError::UnknownScope(scope));
                }
                vec![scope]
            }
        };
        scopes.sort_by(|left, right| left.as_str().cmp(right.as_str()));
        let states = self.state_port.load_scope_states(track_id.as_deref(), &items_dir)?;
        let approval_states = configured_scopes
            .iter()
            .map(|scope| {
                let entry = states
                    .iter()
                    .find(|entry| entry.scope == *scope)
                    .ok_or_else(|| ReviewResultsError::MissingScopeState(scope.clone()))?;
                Ok((domain_scope_name(scope)?, domain_review_state(&entry.state)))
            })
            .collect::<Result<Vec<_>, ReviewResultsError>>()?;
        let hint_should_emit = snapshot.review_json_exists
            && matches!(
                derive_review_approval_verdict(approval_states, snapshot.review_json_exists),
                ReviewApprovalVerdict::Approved
            );
        let mut output_scopes = Vec::new();
        for scope in scopes {
            let entry = states
                .iter()
                .find(|entry| entry.scope == scope)
                .ok_or_else(|| ReviewResultsError::MissingScopeState(scope.clone()))?;
            let state = match entry.state {
                ReviewStoredScopeState::Required(ReviewRequiredReason::NotStarted) => {
                    ReviewScopeResultState::RequiredNotStarted
                }
                ReviewStoredScopeState::Required(ReviewRequiredReason::FindingsRemain) => {
                    ReviewScopeResultState::RequiredFindingsRemain
                }
                ReviewStoredScopeState::Required(ReviewRequiredReason::StaleHash) => {
                    ReviewScopeResultState::RequiredStaleHash
                }
                ReviewStoredScopeState::NotRequired(ReviewNotRequiredReason::Empty) => {
                    ReviewScopeResultState::Empty
                }
                ReviewStoredScopeState::NotRequired(ReviewNotRequiredReason::ZeroFindings) => {
                    ReviewScopeResultState::Approved
                }
            };
            let rounds =
                self.round_port.load_scope_rounds(track_id.as_deref(), &items_dir, &scope)?;
            output_scopes.push(ReviewScopeResultOutput { scope, state, rounds });
        }
        Ok(ReviewResultsOutput { base: snapshot.base, scopes: output_scopes, hint_should_emit })
    }
}

fn domain_scope_name(scope: &ReviewScopeName) -> Result<ScopeName, ReviewResultsError> {
    ScopeName::parse(scope.as_str())
        .map_err(|error| ReviewResultsError::Failed(DiagnosticText::new(error.to_string())))
}

fn domain_review_state(state: &ReviewStoredScopeState) -> ReviewState {
    match state {
        ReviewStoredScopeState::Required(ReviewRequiredReason::NotStarted) => {
            ReviewState::Required(RequiredReason::NotStarted)
        }
        ReviewStoredScopeState::Required(ReviewRequiredReason::FindingsRemain) => {
            ReviewState::Required(RequiredReason::FindingsRemain)
        }
        ReviewStoredScopeState::Required(ReviewRequiredReason::StaleHash) => {
            ReviewState::Required(RequiredReason::StaleHash)
        }
        ReviewStoredScopeState::NotRequired(ReviewNotRequiredReason::Empty) => {
            ReviewState::NotRequired(NotRequiredReason::Empty)
        }
        ReviewStoredScopeState::NotRequired(ReviewNotRequiredReason::ZeroFindings) => {
            ReviewState::NotRequired(NotRequiredReason::ZeroFindings)
        }
    }
}

// ── ReviewValidateScopeService ────────────────────────────────────────────────

/// Application service (primary port) for `sotp review validate-scope`.
pub trait ReviewValidateScopeService: Send + Sync {
    /// Validate a scope name for the given track.
    ///
    /// Returns `Ok(())` on success or an error on failure.
    fn validate_scope(
        &self,
        scope: String,
        track_id: Option<String>,
        items_dir: PathBuf,
    ) -> Result<(), ReviewAuxError>;
}

/// Function-pointer interactor implementing [`ReviewValidateScopeService`].
pub struct ReviewValidateScopeInteractor {
    #[allow(clippy::type_complexity)]
    run_fn:
        Arc<dyn Fn(String, Option<String>, PathBuf) -> Result<(), ReviewAuxError> + Send + Sync>,
}

impl ReviewValidateScopeInteractor {
    /// Create with injected function.
    #[must_use]
    #[allow(clippy::type_complexity)]
    pub fn new(
        run_fn: Arc<
            dyn Fn(String, Option<String>, PathBuf) -> Result<(), ReviewAuxError> + Send + Sync,
        >,
    ) -> Self {
        Self { run_fn }
    }
}

impl ReviewValidateScopeService for ReviewValidateScopeInteractor {
    fn validate_scope(
        &self,
        scope: String,
        track_id: Option<String>,
        items_dir: PathBuf,
    ) -> Result<(), ReviewAuxError> {
        (self.run_fn)(scope, track_id, items_dir)
    }
}

// ── ReviewGetBriefingService ──────────────────────────────────────────────────

/// Application service (primary port) for `sotp review get-briefing`.
pub trait ReviewGetBriefingService: Send + Sync {
    /// Get the briefing file path for the given scope.
    ///
    /// Returns the path string if one exists, or `None`.
    fn get_briefing(
        &self,
        scope: String,
        track_id: Option<String>,
        items_dir: PathBuf,
    ) -> Result<Option<String>, ReviewAuxError>;
}

/// Function-pointer interactor implementing [`ReviewGetBriefingService`].
pub struct ReviewGetBriefingInteractor {
    #[allow(clippy::type_complexity)]
    run_fn: Arc<
        dyn Fn(String, Option<String>, PathBuf) -> Result<Option<String>, ReviewAuxError>
            + Send
            + Sync,
    >,
}

impl ReviewGetBriefingInteractor {
    /// Create with injected function.
    #[must_use]
    #[allow(clippy::type_complexity)]
    pub fn new(
        run_fn: Arc<
            dyn Fn(String, Option<String>, PathBuf) -> Result<Option<String>, ReviewAuxError>
                + Send
                + Sync,
        >,
    ) -> Self {
        Self { run_fn }
    }
}

impl ReviewGetBriefingService for ReviewGetBriefingInteractor {
    fn get_briefing(
        &self,
        scope: String,
        track_id: Option<String>,
        items_dir: PathBuf,
    ) -> Result<Option<String>, ReviewAuxError> {
        (self.run_fn)(scope, track_id, items_dir)
    }
}

// ── ReviewRunLocalService ─────────────────────────────────────────────────────

/// Output DTO from `ReviewRunLocalService`.
///
/// Carries structured diagnostics so the primary adapter owns their final
/// stderr rendering.
pub struct ReviewRunLocalOutput {
    /// Human-readable review summary for stdout.
    pub summary: Option<String>,
    /// Ordered diagnostics for the delivery adapter to render to stderr.
    pub diagnostics: Vec<DiagnosticText>,
    /// Process exit code.
    pub exit_code: u8,
}

/// Application service (primary port) for `sotp review local` (provider-resolved).
pub trait ReviewRunLocalService: Send + Sync {
    /// Run the reviewer with the provider auto-resolved from agent-profiles.json.
    #[allow(clippy::too_many_arguments)]
    fn run_local(
        &self,
        model: Option<String>,
        timeout_seconds: u64,
        briefing_file: Option<PathBuf>,
        prompt: Option<String>,
        track_id: Option<String>,
        round_type: String,
        group: String,
        items_dir: PathBuf,
    ) -> ReviewRunLocalOutput;
}

/// Function-pointer interactor implementing [`ReviewRunLocalService`].
pub struct ReviewRunLocalInteractor {
    #[allow(clippy::type_complexity)]
    run_fn: Arc<
        dyn Fn(
                Option<String>,
                u64,
                Option<PathBuf>,
                Option<String>,
                Option<String>,
                String,
                String,
                PathBuf,
            ) -> ReviewRunLocalOutput
            + Send
            + Sync,
    >,
}

impl ReviewRunLocalInteractor {
    /// Create with injected function.
    #[must_use]
    #[allow(clippy::type_complexity)]
    pub fn new(
        run_fn: Arc<
            dyn Fn(
                    Option<String>,
                    u64,
                    Option<PathBuf>,
                    Option<String>,
                    Option<String>,
                    String,
                    String,
                    PathBuf,
                ) -> ReviewRunLocalOutput
                + Send
                + Sync,
        >,
    ) -> Self {
        Self { run_fn }
    }
}

impl ReviewRunLocalService for ReviewRunLocalInteractor {
    #[allow(clippy::too_many_arguments)]
    fn run_local(
        &self,
        model: Option<String>,
        timeout_seconds: u64,
        briefing_file: Option<PathBuf>,
        prompt: Option<String>,
        track_id: Option<String>,
        round_type: String,
        group: String,
        items_dir: PathBuf,
    ) -> ReviewRunLocalOutput {
        (self.run_fn)(
            model,
            timeout_seconds,
            briefing_file,
            prompt,
            track_id,
            round_type,
            group,
            items_dir,
        )
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Arc;

    use crate::git_workflow::DiagnosticText;

    use super::{
        NonEmptyReviewerFindingsOutput, ReviewFindingsOutputValidationError,
        ReviewNotRequiredReason, ReviewRequiredReason, ReviewResultsError, ReviewResultsInteractor,
        ReviewResultsRoundPort, ReviewResultsScopePort, ReviewResultsScopeSnapshot,
        ReviewResultsService, ReviewResultsStatePort, ReviewRoundResultOutput,
        ReviewRoundResultVerdict, ReviewScopeName, ReviewScopeNameValidationError,
        ReviewScopeResultState, ReviewScopeSelectionRequest, ReviewScopeSelectionValidationError,
        ReviewStoredRound, ReviewStoredScopeState, ReviewStoredScopeStateEntry,
        ReviewerFindingOutput,
    };

    #[test]
    fn test_review_scope_selection_omitted_selector_constructs_all() {
        let selection = ReviewScopeSelectionRequest::try_new(None, false)
            .expect("an omitted selector must select the complete universe");

        assert_eq!(selection, ReviewScopeSelectionRequest::All);
    }

    #[test]
    fn test_review_scope_selection_explicit_all_constructs_all() {
        let selection = ReviewScopeSelectionRequest::try_new(None, true)
            .expect("an explicit all selector must select the complete universe");

        assert_eq!(selection, ReviewScopeSelectionRequest::All);
    }

    #[test]
    fn test_review_scope_selection_named_scope_constructs_named_variant() {
        let selection = ReviewScopeSelectionRequest::try_new(Some("cli_driver".to_owned()), false)
            .expect("a valid scope name must select the named range");

        assert!(
            matches!(selection, ReviewScopeSelectionRequest::NamedCandidate(scope) if scope.as_str() == "cli_driver")
        );
    }

    #[test]
    fn test_review_scope_selection_scope_and_all_returns_conflict_error() {
        let selection = ReviewScopeSelectionRequest::try_new(Some("cli_driver".to_owned()), true);

        assert!(matches!(selection, Err(ReviewScopeSelectionValidationError::ScopeAndAll)));
    }

    #[test]
    fn test_review_scope_selection_invalid_scope_returns_validation_error() {
        let selection = ReviewScopeSelectionRequest::try_new(Some("".to_owned()), false);

        assert!(matches!(selection, Err(ReviewScopeSelectionValidationError::InvalidScope(_))));
    }

    #[test]
    fn test_review_scope_selection_request_rejects_format_invalid_scope() {
        let request = ReviewScopeSelectionRequest::try_new(Some("非ASCII".to_owned()), false);

        assert!(matches!(request, Err(ReviewScopeSelectionValidationError::InvalidScope(_))));
    }

    #[test]
    fn test_review_scope_name_domain_valid_ascii_forms_succeed() {
        for value in ["scope.name", "scope with spaces", " "] {
            let scope = ReviewScopeName::try_new(value.to_owned()).expect("valid domain scope");

            assert_eq!(scope.as_str(), value);
        }
    }

    #[test]
    fn test_review_scope_name_domain_invalid_forms_return_error() {
        for value in ["", "非ASCII"] {
            assert!(matches!(
                ReviewScopeName::try_new(value.to_owned()),
                Err(ReviewScopeNameValidationError::Invalid(_))
            ));
        }
    }

    #[test]
    fn test_review_scope_name_other_output_constructor_preserves_domain_sentinel() {
        assert_eq!(ReviewScopeName::other().as_str(), "other");
    }

    #[test]
    fn test_review_scope_selection_request_reports_declared_boundary_errors() {
        let conflicting = ReviewScopeSelectionRequest::try_new(Some("cli".to_owned()), true);
        let invalid = ReviewScopeSelectionRequest::try_new(Some("非ASCII".to_owned()), false);

        assert!(matches!(conflicting, Err(ReviewScopeSelectionValidationError::ScopeAndAll)));
        assert!(matches!(invalid, Err(ReviewScopeSelectionValidationError::InvalidScope(_))));
    }

    #[test]
    fn test_review_results_error_failed_preserves_typed_diagnostic_text() {
        let error = ReviewResultsError::Failed(DiagnosticText::new("results port failure"));

        assert_eq!(error.to_string(), "results port failure");
    }

    #[test]
    fn test_review_results_interactor_preserves_loader_failed_error_and_display() {
        struct FailingScopePort;

        impl ReviewResultsScopePort for FailingScopePort {
            fn load_scope_snapshot(
                &self,
                _track_id: Option<&str>,
                _items_dir: &std::path::Path,
            ) -> Result<ReviewResultsScopeSnapshot, ReviewResultsError> {
                Err(ReviewResultsError::Failed(DiagnosticText::new(
                    "review results scope snapshot could not be loaded",
                )))
            }
        }

        struct UnusedStatePort;

        impl ReviewResultsStatePort for UnusedStatePort {
            fn load_scope_states(
                &self,
                _track_id: Option<&str>,
                _items_dir: &std::path::Path,
            ) -> Result<Vec<ReviewStoredScopeStateEntry>, ReviewResultsError> {
                panic!("a loader failure must return before state loading")
            }
        }

        struct UnusedRoundPort;

        impl ReviewResultsRoundPort for UnusedRoundPort {
            fn load_scope_rounds(
                &self,
                _track_id: Option<&str>,
                _items_dir: &std::path::Path,
                _scope: &ReviewScopeName,
            ) -> Result<Vec<ReviewStoredRound>, ReviewResultsError> {
                panic!("a loader failure must return before round loading")
            }
        }

        let error = ReviewResultsInteractor::new(
            Arc::new(FailingScopePort),
            Arc::new(UnusedStatePort),
            Arc::new(UnusedRoundPort),
        )
        .results(None, PathBuf::from("track/items"), ReviewScopeSelectionRequest::All)
        .expect_err("the loader-owned failure must pass through unchanged");

        assert!(matches!(&error, ReviewResultsError::Failed(_)));
        assert_eq!(error.to_string(), "review results scope snapshot could not be loaded");
    }

    #[test]
    fn test_review_results_interactor_produces_unknown_scope_error_and_display() {
        struct ScopePort;

        impl ReviewResultsScopePort for ScopePort {
            fn load_scope_snapshot(
                &self,
                _track_id: Option<&str>,
                _items_dir: &std::path::Path,
            ) -> Result<ReviewResultsScopeSnapshot, ReviewResultsError> {
                Ok(ReviewResultsScopeSnapshot {
                    base: "base".to_owned(),
                    configured_scopes: vec![
                        ReviewScopeName::try_new("configured".to_owned()).expect("valid scope"),
                    ],
                    review_json_exists: true,
                })
            }
        }

        struct UnusedStatePort;

        impl ReviewResultsStatePort for UnusedStatePort {
            fn load_scope_states(
                &self,
                _track_id: Option<&str>,
                _items_dir: &std::path::Path,
            ) -> Result<Vec<ReviewStoredScopeStateEntry>, ReviewResultsError> {
                panic!("unknown scope must be rejected before state loading")
            }
        }

        struct UnusedRoundPort;

        impl ReviewResultsRoundPort for UnusedRoundPort {
            fn load_scope_rounds(
                &self,
                _track_id: Option<&str>,
                _items_dir: &std::path::Path,
                _scope: &ReviewScopeName,
            ) -> Result<Vec<ReviewStoredRound>, ReviewResultsError> {
                panic!("unknown scope must be rejected before round loading")
            }
        }

        let error = ReviewResultsInteractor::new(
            Arc::new(ScopePort),
            Arc::new(UnusedStatePort),
            Arc::new(UnusedRoundPort),
        )
        .results(
            None,
            PathBuf::from("track/items"),
            ReviewScopeSelectionRequest::try_new(Some("not-configured".to_owned()), false)
                .expect("format-valid unconfigured scope"),
        )
        .expect_err("the interactor must own configured-universe membership rejection");

        assert!(matches!(
            &error,
            ReviewResultsError::UnknownScope(scope) if scope.as_str() == "not-configured"
        ));
        assert_eq!(error.to_string(), "unknown review results scope: not-configured");
    }

    #[test]
    fn test_review_round_result_output_preserves_typed_round_fields() {
        let round = ReviewRoundResultOutput {
            round_type: crate::review_v2::ReviewRoundType::Final,
            at: "2026-08-10T12:34:56Z".to_owned(),
            verdict: ReviewRoundResultVerdict::ZeroFindings,
        };

        assert!(matches!(round.round_type, crate::review_v2::ReviewRoundType::Final));
        assert_eq!(round.at, "2026-08-10T12:34:56Z");
        assert!(matches!(round.verdict, ReviewRoundResultVerdict::ZeroFindings));
        assert_eq!(round, round.clone());
    }

    #[test]
    fn test_review_round_result_verdict_preserves_zero_and_non_empty_findings() {
        let zero = ReviewRoundResultVerdict::ZeroFindings;
        assert!(matches!(zero, ReviewRoundResultVerdict::ZeroFindings));

        let findings = NonEmptyReviewerFindingsOutput::try_new(vec![ReviewerFindingOutput {
            message: crate::git_workflow::DiagnosticText::new("typed finding"),
            severity: Some("P1".to_owned()),
            file: None,
            line: None,
            category: None,
        }])
        .expect("a finding must preserve the non-empty invariant");
        let remaining = ReviewRoundResultVerdict::FindingsRemain(findings);

        assert!(matches!(
            remaining,
            ReviewRoundResultVerdict::FindingsRemain(findings)
                if findings.as_slice().len() == 1
                    && findings
                        .as_slice()
                        .first()
                        .is_some_and(|finding| finding.message.as_str() == "typed finding")
        ));
    }

    #[test]
    fn test_non_empty_reviewer_findings_rejects_empty_input() {
        let result = NonEmptyReviewerFindingsOutput::try_new(Vec::new());

        assert!(matches!(result, Err(ReviewFindingsOutputValidationError::Empty)));
    }

    #[test]
    fn test_review_scope_result_state_exposes_declared_variants() {
        assert!(matches!(
            ReviewScopeResultState::RequiredNotStarted,
            ReviewScopeResultState::RequiredNotStarted
        ));
        assert!(matches!(
            ReviewScopeResultState::RequiredFindingsRemain,
            ReviewScopeResultState::RequiredFindingsRemain
        ));
        assert!(matches!(
            ReviewScopeResultState::RequiredStaleHash,
            ReviewScopeResultState::RequiredStaleHash
        ));
        assert!(matches!(ReviewScopeResultState::Empty, ReviewScopeResultState::Empty));
        assert!(matches!(ReviewScopeResultState::Approved, ReviewScopeResultState::Approved));
    }

    #[test]
    fn test_review_results_interactor_projects_multi_scope_state_and_round_content() {
        struct ScopePort;
        impl ReviewResultsScopePort for ScopePort {
            fn load_scope_snapshot(
                &self,
                _track_id: Option<&str>,
                _items_dir: &std::path::Path,
            ) -> Result<ReviewResultsScopeSnapshot, ReviewResultsError> {
                Ok(ReviewResultsScopeSnapshot {
                    base: "base".to_owned(),
                    configured_scopes: vec![
                        ReviewScopeName::try_new("beta".to_owned()).expect("valid"),
                        ReviewScopeName::try_new("alpha".to_owned()).expect("valid"),
                    ],
                    review_json_exists: true,
                })
            }
        }
        struct StatePort;
        impl ReviewResultsStatePort for StatePort {
            fn load_scope_states(
                &self,
                _track_id: Option<&str>,
                _items_dir: &std::path::Path,
            ) -> Result<Vec<ReviewStoredScopeStateEntry>, ReviewResultsError> {
                Ok(vec![
                    ReviewStoredScopeStateEntry {
                        scope: ReviewScopeName::try_new("alpha".to_owned()).expect("valid"),
                        state: ReviewStoredScopeState::Required(
                            ReviewRequiredReason::FindingsRemain,
                        ),
                    },
                    ReviewStoredScopeStateEntry {
                        scope: ReviewScopeName::try_new("beta".to_owned()).expect("valid"),
                        state: ReviewStoredScopeState::NotRequired(
                            ReviewNotRequiredReason::ZeroFindings,
                        ),
                    },
                ])
            }
        }
        struct RoundPort;
        impl ReviewResultsRoundPort for RoundPort {
            fn load_scope_rounds(
                &self,
                _track_id: Option<&str>,
                _items_dir: &std::path::Path,
                scope: &ReviewScopeName,
            ) -> Result<Vec<ReviewStoredRound>, ReviewResultsError> {
                match scope.as_str() {
                    "alpha" => Ok(vec![
                        ReviewRoundResultOutput {
                            round_type: crate::review_v2::ReviewRoundType::Fast,
                            at: "2026-08-10T12:34:56Z".to_owned(),
                            verdict: ReviewRoundResultVerdict::ZeroFindings,
                        },
                        ReviewRoundResultOutput {
                            round_type: crate::review_v2::ReviewRoundType::Final,
                            at: "2026-08-10T12:35:56Z".to_owned(),
                            verdict: ReviewRoundResultVerdict::FindingsRemain(
                                NonEmptyReviewerFindingsOutput::try_new(vec![
                                    ReviewerFindingOutput {
                                        message: crate::git_workflow::DiagnosticText::new(
                                            "retained finding",
                                        ),
                                        severity: Some("P1".to_owned()),
                                        file: Some("src/lib.rs".to_owned()),
                                        line: Some(42),
                                        category: Some("correctness".to_owned()),
                                    },
                                ])
                                .expect("one finding is non-empty"),
                            ),
                        },
                    ]),
                    "beta" => Ok(vec![ReviewRoundResultOutput {
                        round_type: crate::review_v2::ReviewRoundType::Final,
                        at: "2026-08-10T12:36:56Z".to_owned(),
                        verdict: ReviewRoundResultVerdict::ZeroFindings,
                    }]),
                    other => Err(ReviewResultsError::Failed(DiagnosticText::new(format!(
                        "unexpected scope: {other}"
                    )))),
                }
            }
        }
        let interactor = ReviewResultsInteractor::new(
            Arc::new(ScopePort),
            Arc::new(StatePort),
            Arc::new(RoundPort),
        );

        let output = interactor
            .results(
                Some("results-track".to_owned()),
                PathBuf::from("track/items"),
                ReviewScopeSelectionRequest::All,
            )
            .expect("injected results service must succeed");

        assert_eq!(output.base, "base");
        assert_eq!(
            output.scopes.iter().map(|scope| scope.scope.as_str()).collect::<Vec<_>>(),
            vec!["alpha", "beta"],
            "All must enumerate the whole configured universe"
        );
        let alpha = output.scopes.first().expect("alpha must be present");
        assert!(matches!(alpha.state, ReviewScopeResultState::RequiredFindingsRemain));
        assert!(matches!(
            alpha.rounds.as_slice(),
            [
                ReviewRoundResultOutput {
                    round_type: crate::review_v2::ReviewRoundType::Fast,
                    at,
                    verdict: ReviewRoundResultVerdict::ZeroFindings,
                },
                ReviewRoundResultOutput {
                    round_type: crate::review_v2::ReviewRoundType::Final,
                    at: final_at,
                    verdict: ReviewRoundResultVerdict::FindingsRemain(findings),
                },
            ] if at == "2026-08-10T12:34:56Z"
                && final_at == "2026-08-10T12:35:56Z"
                && matches!(
                    findings.as_slice(),
                    [ReviewerFindingOutput {
                        message,
                        severity: Some(severity),
                        file: Some(file),
                        line: Some(42),
                        category: Some(category),
                    }] if message.as_str() == "retained finding"
                        && severity == "P1"
                        && file == "src/lib.rs"
                        && category == "correctness"
                )
        ));
        let beta = output.scopes.get(1).expect("beta must be present");
        assert!(matches!(beta.state, ReviewScopeResultState::Approved));
        assert!(matches!(
            beta.rounds.as_slice(),
            [ReviewRoundResultOutput {
                round_type: crate::review_v2::ReviewRoundType::Final,
                at,
                verdict: ReviewRoundResultVerdict::ZeroFindings,
            }] if at == "2026-08-10T12:36:56Z"
        ));

        let named = interactor
            .results(
                Some("results-track".to_owned()),
                PathBuf::from("track/items"),
                ReviewScopeSelectionRequest::try_new(Some("beta".to_owned()), false)
                    .expect("valid named selection"),
            )
            .expect("named selection must succeed");
        assert_eq!(named.scopes.len(), 1, "Named must limit output to its scope");
        assert_eq!(
            named.scopes.first().expect("one scope was asserted above").scope.as_str(),
            "beta"
        );
        assert!(
            !named.hint_should_emit,
            "a filtered approved scope must not hide track-wide findings from the commit hint"
        );
    }

    #[test]
    fn test_review_results_interactor_named_other_casing_selects_and_displays_only_other_scope() {
        struct ScopePort;

        impl ReviewResultsScopePort for ScopePort {
            fn load_scope_snapshot(
                &self,
                _track_id: Option<&str>,
                _items_dir: &std::path::Path,
            ) -> Result<ReviewResultsScopeSnapshot, ReviewResultsError> {
                Ok(ReviewResultsScopeSnapshot {
                    base: "base".to_owned(),
                    configured_scopes: vec![ReviewScopeName::other()],
                    review_json_exists: false,
                })
            }
        }

        struct StatePort;

        impl ReviewResultsStatePort for StatePort {
            fn load_scope_states(
                &self,
                _track_id: Option<&str>,
                _items_dir: &std::path::Path,
            ) -> Result<Vec<ReviewStoredScopeStateEntry>, ReviewResultsError> {
                Ok(vec![ReviewStoredScopeStateEntry {
                    scope: ReviewScopeName::other(),
                    state: ReviewStoredScopeState::NotRequired(
                        ReviewNotRequiredReason::ZeroFindings,
                    ),
                }])
            }
        }

        struct RoundPort;

        impl ReviewResultsRoundPort for RoundPort {
            fn load_scope_rounds(
                &self,
                _track_id: Option<&str>,
                _items_dir: &std::path::Path,
                scope: &ReviewScopeName,
            ) -> Result<Vec<ReviewStoredRound>, ReviewResultsError> {
                assert_eq!(scope.as_str(), "other");
                Ok(Vec::new())
            }
        }

        let interactor = ReviewResultsInteractor::new(
            Arc::new(ScopePort),
            Arc::new(StatePort),
            Arc::new(RoundPort),
        );

        for selector in ["other", "Other", "OTHER"] {
            let output = interactor
                .results(
                    Some("results-other-2026".to_owned()),
                    PathBuf::from("track/items"),
                    ReviewScopeSelectionRequest::try_new(Some(selector.to_owned()), false)
                        .expect("every catch-all casing must be a valid named selection"),
                )
                .expect("the implicit Other scope must be displayable by name");

            assert_eq!(output.scopes.len(), 1, "{selector} must select only Other");
            let scope = output.scopes.first().expect("one selected scope was asserted above");
            assert_eq!(scope.scope.as_str(), "other");
            assert!(matches!(scope.state, ReviewScopeResultState::Approved));
        }
    }

    #[test]
    fn test_review_results_interactor_projects_required_and_not_required_stored_states() {
        struct ScopePort;

        impl ReviewResultsScopePort for ScopePort {
            fn load_scope_snapshot(
                &self,
                _track_id: Option<&str>,
                _items_dir: &std::path::Path,
            ) -> Result<ReviewResultsScopeSnapshot, ReviewResultsError> {
                Ok(ReviewResultsScopeSnapshot {
                    base: "base".to_owned(),
                    configured_scopes: vec![
                        ReviewScopeName::try_new("required".to_owned()).expect("valid scope"),
                        ReviewScopeName::try_new("empty".to_owned()).expect("valid scope"),
                    ],
                    review_json_exists: true,
                })
            }
        }

        struct StatePort;

        impl ReviewResultsStatePort for StatePort {
            fn load_scope_states(
                &self,
                _track_id: Option<&str>,
                _items_dir: &std::path::Path,
            ) -> Result<Vec<ReviewStoredScopeStateEntry>, ReviewResultsError> {
                Ok(vec![
                    ReviewStoredScopeStateEntry {
                        scope: ReviewScopeName::try_new("required".to_owned())
                            .expect("valid scope"),
                        state: ReviewStoredScopeState::Required(ReviewRequiredReason::NotStarted),
                    },
                    ReviewStoredScopeStateEntry {
                        scope: ReviewScopeName::try_new("empty".to_owned()).expect("valid scope"),
                        state: ReviewStoredScopeState::NotRequired(ReviewNotRequiredReason::Empty),
                    },
                ])
            }
        }

        struct RoundPort;

        impl ReviewResultsRoundPort for RoundPort {
            fn load_scope_rounds(
                &self,
                _track_id: Option<&str>,
                _items_dir: &std::path::Path,
                _scope: &ReviewScopeName,
            ) -> Result<Vec<ReviewStoredRound>, ReviewResultsError> {
                Ok(Vec::new())
            }
        }

        let output = ReviewResultsInteractor::new(
            Arc::new(ScopePort),
            Arc::new(StatePort),
            Arc::new(RoundPort),
        )
        .results(
            Some("results-states-2026".to_owned()),
            PathBuf::from("track/items"),
            ReviewScopeSelectionRequest::All,
        )
        .expect("stored states must project through the all-selection flow");

        assert!(matches!(
            output
                .scopes
                .iter()
                .find(|scope| scope.scope.as_str() == "required")
                .expect("required scope must be projected")
                .state,
            ReviewScopeResultState::RequiredNotStarted
        ));
        assert!(matches!(
            output
                .scopes
                .iter()
                .find(|scope| scope.scope.as_str() == "empty")
                .expect("empty scope must be projected")
                .state,
            ReviewScopeResultState::Empty
        ));
    }

    #[test]
    fn test_review_results_interactor_rejects_missing_configured_scope_state() {
        struct ScopePort;
        impl ReviewResultsScopePort for ScopePort {
            fn load_scope_snapshot(
                &self,
                _track_id: Option<&str>,
                _items_dir: &std::path::Path,
            ) -> Result<ReviewResultsScopeSnapshot, ReviewResultsError> {
                Ok(ReviewResultsScopeSnapshot {
                    base: "base".to_owned(),
                    configured_scopes: vec![
                        ReviewScopeName::try_new("alpha".to_owned()).expect("valid scope"),
                        ReviewScopeName::try_new("beta".to_owned()).expect("valid scope"),
                    ],
                    review_json_exists: true,
                })
            }
        }
        struct StatePort;
        impl ReviewResultsStatePort for StatePort {
            fn load_scope_states(
                &self,
                _track_id: Option<&str>,
                _items_dir: &std::path::Path,
            ) -> Result<Vec<ReviewStoredScopeStateEntry>, ReviewResultsError> {
                Ok(vec![ReviewStoredScopeStateEntry {
                    scope: ReviewScopeName::try_new("alpha".to_owned()).expect("valid scope"),
                    state: ReviewStoredScopeState::NotRequired(
                        ReviewNotRequiredReason::ZeroFindings,
                    ),
                }])
            }
        }
        struct RoundPort;
        impl ReviewResultsRoundPort for RoundPort {
            fn load_scope_rounds(
                &self,
                _track_id: Option<&str>,
                _items_dir: &std::path::Path,
                _scope: &ReviewScopeName,
            ) -> Result<Vec<ReviewStoredRound>, ReviewResultsError> {
                Err(ReviewResultsError::Failed(DiagnosticText::new(
                    "incomplete state snapshots must fail before loading rounds",
                )))
            }
        }

        let error = ReviewResultsInteractor::new(
            Arc::new(ScopePort),
            Arc::new(StatePort),
            Arc::new(RoundPort),
        )
        .results(None, PathBuf::from("track/items"), ReviewScopeSelectionRequest::All)
        .expect_err("a configured scope without state must fail closed");

        assert!(matches!(
            &error,
            ReviewResultsError::MissingScopeState(scope) if scope.as_str() == "beta"
        ));
        assert_eq!(error.to_string(), "review results state missing for configured scope 'beta'");
    }

    #[test]
    fn test_review_results_interactor_bypass_verdict_does_not_emit_commit_hint() {
        struct ScopePort;
        impl ReviewResultsScopePort for ScopePort {
            fn load_scope_snapshot(
                &self,
                _track_id: Option<&str>,
                _items_dir: &std::path::Path,
            ) -> Result<ReviewResultsScopeSnapshot, ReviewResultsError> {
                Ok(ReviewResultsScopeSnapshot {
                    base: "base".to_owned(),
                    configured_scopes: vec![
                        ReviewScopeName::try_new("alpha".to_owned()).expect("valid scope"),
                    ],
                    review_json_exists: false,
                })
            }
        }
        struct StatePort;
        impl ReviewResultsStatePort for StatePort {
            fn load_scope_states(
                &self,
                _track_id: Option<&str>,
                _items_dir: &std::path::Path,
            ) -> Result<Vec<ReviewStoredScopeStateEntry>, ReviewResultsError> {
                Ok(vec![ReviewStoredScopeStateEntry {
                    scope: ReviewScopeName::try_new("alpha".to_owned()).expect("valid scope"),
                    state: ReviewStoredScopeState::Required(ReviewRequiredReason::NotStarted),
                }])
            }
        }
        struct RoundPort;
        impl ReviewResultsRoundPort for RoundPort {
            fn load_scope_rounds(
                &self,
                _track_id: Option<&str>,
                _items_dir: &std::path::Path,
                _scope: &ReviewScopeName,
            ) -> Result<Vec<ReviewStoredRound>, ReviewResultsError> {
                Ok(Vec::new())
            }
        }

        let output = ReviewResultsInteractor::new(
            Arc::new(ScopePort),
            Arc::new(StatePort),
            Arc::new(RoundPort),
        )
        .results(None, PathBuf::from("track/items"), ReviewScopeSelectionRequest::All)
        .expect("complete bypass state is renderable");

        assert!(!output.hint_should_emit, "bypass is not an approved commit hint");
    }

    #[test]
    fn test_review_results_interactor_requires_persisted_review_for_approved_commit_hint() {
        struct ScopePort;
        impl ReviewResultsScopePort for ScopePort {
            fn load_scope_snapshot(
                &self,
                _track_id: Option<&str>,
                _items_dir: &std::path::Path,
            ) -> Result<ReviewResultsScopeSnapshot, ReviewResultsError> {
                Ok(ReviewResultsScopeSnapshot {
                    base: "base".to_owned(),
                    configured_scopes: vec![
                        ReviewScopeName::try_new("alpha".to_owned()).expect("valid scope"),
                    ],
                    review_json_exists: false,
                })
            }
        }
        struct StatePort;
        impl ReviewResultsStatePort for StatePort {
            fn load_scope_states(
                &self,
                _track_id: Option<&str>,
                _items_dir: &std::path::Path,
            ) -> Result<Vec<ReviewStoredScopeStateEntry>, ReviewResultsError> {
                Ok(vec![ReviewStoredScopeStateEntry {
                    scope: ReviewScopeName::try_new("alpha".to_owned()).expect("valid scope"),
                    state: ReviewStoredScopeState::NotRequired(
                        ReviewNotRequiredReason::ZeroFindings,
                    ),
                }])
            }
        }
        struct RoundPort;
        impl ReviewResultsRoundPort for RoundPort {
            fn load_scope_rounds(
                &self,
                _track_id: Option<&str>,
                _items_dir: &std::path::Path,
                _scope: &ReviewScopeName,
            ) -> Result<Vec<ReviewStoredRound>, ReviewResultsError> {
                Ok(Vec::new())
            }
        }

        let output = ReviewResultsInteractor::new(
            Arc::new(ScopePort),
            Arc::new(StatePort),
            Arc::new(RoundPort),
        )
        .results(None, PathBuf::from("track/items"), ReviewScopeSelectionRequest::All)
        .expect("an empty-diff result must be renderable");

        assert!(!output.hint_should_emit, "a missing review.json must suppress the commit hint");
    }
}
