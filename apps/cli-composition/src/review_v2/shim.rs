//! `ReviewCompositionRoot` definition and `ReviewServiceImpl` — the concrete
//! implementation of `usecase::review_v2::ReviewService`.
//!
//! `ReviewServiceImpl` wires all 11 individual sub-services internally so that
//! `ReviewDriver` holds only one `Arc<dyn ReviewService>` (D3/D4 cli_driver
//! policy).

use std::path::PathBuf;
use std::sync::Arc;

use usecase::commit_hash_persistence::CommitHashPersistenceError;
use usecase::review_v2::aggregate_service::{ReviewRunInput, ReviewService};
use usecase::review_v2::review_aux::{
    ReviewAuxError, ReviewResultsInteractor, ReviewResultsService,
};
use usecase::review_v2::run_review_fix::RunReviewFixService;
use usecase::review_v2::{
    ReviewApprovalOutput, ReviewCheckApprovedError, ReviewCheckZeroFindingsInteractor,
    ReviewCheckZeroFindingsService, ReviewCheckZeroFindingsStatePort, ReviewRunLocalOutput,
    RunReviewError, RunReviewOutput,
};

// ── Per-context composition root ──────────────────────────────────────────────

/// Composition root for the `review_v2` command family.
///
/// Unit struct: no adapter dependencies are injected at construction time.
pub struct ReviewCompositionRoot;

impl ReviewCompositionRoot {
    /// Create a new `ReviewCompositionRoot`.
    pub fn new() -> Self {
        Self
    }

    /// Construct a fully-wired [`cli_driver::review::ReviewDriver`].
    ///
    /// Wires the aggregate review service and focused check-zero-findings
    /// service separately. `run_local` is additionally gated by the configured
    /// pre-review command dispatcher.
    pub fn review_driver(&self) -> cli_driver::review::ReviewDriver {
        let inner = Arc::new(review_service_impl()) as Arc<dyn ReviewService>;
        let service = super::pre_review_command::gate_local_review_service(inner);
        cli_driver::review::ReviewDriver::new(
            service,
            review_results_service(),
            check_zero_findings_service(),
        )
    }

    /// Construct a fully-wired [`cli_driver::review::ReviewFixDriver`].
    pub fn review_fix_driver(&self) -> cli_driver::review::ReviewFixDriver {
        review_fix_driver_from_service(super::run_fix::review_fix_service())
    }

    #[cfg(test)]
    #[allow(dead_code)]
    fn review_fix_driver_with_service(
        &self,
        service: Arc<dyn RunReviewFixService>,
    ) -> cli_driver::review::ReviewFixDriver {
        review_fix_driver_from_service(service)
    }
}

/// Connects a review-fix service to its CLI driver without executing it.
fn review_fix_driver_from_service(
    service: Arc<dyn RunReviewFixService>,
) -> cli_driver::review::ReviewFixDriver {
    cli_driver::review::ReviewFixDriver::new(service)
}

impl Default for ReviewCompositionRoot {
    fn default() -> Self {
        Self::new()
    }
}

// ── ReviewServiceImpl ─────────────────────────────────────────────────────────

/// Concrete implementation of [`ReviewService`] that delegates to the
/// `ReviewCompositionRoot` methods.
///
/// All wiring complexity stays here; `ReviewDriver` holds only one
/// `Arc<dyn ReviewService>`.
pub(crate) struct ReviewServiceImpl;

pub(super) fn review_service_impl() -> ReviewServiceImpl {
    ReviewServiceImpl
}

pub(super) fn review_results_service() -> Arc<dyn ReviewResultsService> {
    Arc::new(ReviewResultsInteractor::new(
        Arc::new(infrastructure::review_v2::ResultsScopeAdapter),
        Arc::new(infrastructure::review_v2::ResultsStateAdapter),
        Arc::new(infrastructure::review_v2::ResultsRoundAdapter),
    ))
}

fn check_zero_findings_service() -> Arc<dyn ReviewCheckZeroFindingsService> {
    let state_port: Arc<dyn ReviewCheckZeroFindingsStatePort> =
        Arc::new(infrastructure::review_v2::ReviewCheckZeroFindingsStateAdapter);
    Arc::new(ReviewCheckZeroFindingsInteractor::new(state_port))
}

impl ReviewService for ReviewServiceImpl {
    fn run_codex(&self, input: ReviewRunInput) -> Result<RunReviewOutput, RunReviewError> {
        let root = ReviewCompositionRoot::new();
        let comp_input = super::ReviewRunCodexInput {
            model: input.model,
            timeout_seconds: input.timeout_seconds,
            briefing_file: input.briefing_file,
            prompt: input.prompt,
            track_id: input.track_id,
            round_type: input.round_type,
            group: input.group,
            items_dir: input.items_dir,
        };
        match root.review_run_codex(comp_input) {
            Ok(output) => Ok(output),
            Err(e) => Err(RunReviewError::ReviewerFailed(e.to_string())),
        }
    }

    fn run_claude(&self, input: ReviewRunInput) -> Result<RunReviewOutput, RunReviewError> {
        let root = ReviewCompositionRoot::new();
        let comp_input = super::ReviewRunClaudeInput {
            model: input.model,
            timeout_seconds: input.timeout_seconds,
            briefing_file: input.briefing_file,
            prompt: input.prompt,
            track_id: input.track_id,
            round_type: input.round_type,
            group: input.group,
            items_dir: input.items_dir,
        };
        match root.review_run_claude(comp_input) {
            Ok(output) => Ok(output),
            Err(e) => Err(RunReviewError::ReviewerFailed(e.to_string())),
        }
    }

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
        let root = ReviewCompositionRoot::new();
        let input = super::ReviewRunLocalInput {
            model,
            timeout_seconds,
            briefing_file,
            prompt,
            track_id,
            round_type,
            group,
            items_dir,
        };
        match root.review_run_local_ungated(input) {
            Ok(output) => output,
            Err(e) => ReviewRunLocalOutput {
                summary: None,
                diagnostics: vec![usecase::git_workflow::DiagnosticText::new(e.to_string())],
                exit_code: 1,
            },
        }
    }

    fn check_approved(
        &self,
        track_id: String,
        items_dir: PathBuf,
    ) -> Result<ReviewApprovalOutput, ReviewCheckApprovedError> {
        super::approved::check_approved_str(&track_id, &items_dir)
    }

    fn classify(
        &self,
        paths: Vec<String>,
        track_id: Option<String>,
        items_dir: PathBuf,
    ) -> Result<Vec<(String, String)>, ReviewAuxError> {
        let root = ReviewCompositionRoot::new();
        root.review_classify(paths, track_id, items_dir)
            .map_err(|e| ReviewAuxError::Failed(e.to_string()))
    }

    fn files(
        &self,
        scope: String,
        track_id: Option<String>,
        items_dir: PathBuf,
    ) -> Result<Vec<String>, ReviewAuxError> {
        let root = ReviewCompositionRoot::new();
        root.review_files(scope, track_id, items_dir)
            .map_err(|e| ReviewAuxError::Failed(e.to_string()))
    }

    fn validate_scope(
        &self,
        scope: String,
        track_id: Option<String>,
        items_dir: PathBuf,
    ) -> Result<(), ReviewAuxError> {
        let root = ReviewCompositionRoot::new();
        root.review_validate_scope(scope, track_id, items_dir)
            .map(|_| ())
            .map_err(|e| ReviewAuxError::Failed(e.to_string()))
    }

    fn get_briefing(
        &self,
        scope: String,
        track_id: Option<String>,
        items_dir: PathBuf,
    ) -> Result<Option<String>, ReviewAuxError> {
        let root = ReviewCompositionRoot::new();
        root.review_get_briefing(scope, track_id, items_dir)
            .map(|outcome| outcome.stdout)
            .map_err(|e| ReviewAuxError::Failed(e.to_string()))
    }

    fn persist_commit_hash(
        &self,
        track_id: String,
        _workspace_root: PathBuf,
    ) -> Result<String, CommitHashPersistenceError> {
        super::commit_hash::persist_commit_hash_for_track(&track_id)
            .map_err(CommitHashPersistenceError::StoreWriteFailed)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    use usecase::git_workflow::DiagnosticText;
    use usecase::review_v2::run_review_fix::{
        ReviewFixRunnerError, RunReviewFixError, RunReviewFixOutput, RunReviewFixRequest,
        RunReviewFixService,
    };

    use super::ReviewCompositionRoot;

    #[test]
    fn test_review_composition_root_gates_local_review_service() {
        let source = include_str!("shim.rs");
        let production_source = source.split("\n#[cfg(test)]").next().unwrap();

        assert!(
            production_source.contains("Arc::new(review_service_impl()) as Arc<dyn ReviewService>")
        );
        assert!(
            production_source
                .contains("super::pre_review_command::gate_local_review_service(inner)")
        );
        assert!(production_source.contains("review_results_service()"));
    }

    #[test]
    fn test_review_composition_root_public_surface_is_pure_di() {
        let shim_source = include_str!("shim.rs");
        let production_source = shim_source.split("\n#[cfg(test)]").next().expect("test boundary");
        let module_source = include_str!("mod.rs");
        let inputs_source = include_str!("inputs.rs");

        assert!(shim_source.contains("pub fn new() -> Self"));
        assert!(
            shim_source.contains("pub fn review_driver(&self) -> cli_driver::review::ReviewDriver")
        );
        assert!(
            shim_source
                .contains("pub fn review_fix_driver(&self) -> cli_driver::review::ReviewFixDriver")
        );
        assert!(
            !module_source.contains("pub fn review_"),
            "legacy review operations must not remain public root methods"
        );
        assert!(
            !inputs_source.contains("ReviewResultsInput"),
            "the composition layer must not reintroduce the driver-owned results DTO"
        );
        assert!(
            !inputs_source.contains("RunReviewFixLocalInput"),
            "the composition layer must not reintroduce the deleted review-fix input DTO"
        );
        assert!(
            !production_source.contains("splitn(2, '\\t')")
                && !production_source.contains("stdout.lines()"),
            "composition must not parse presentation text back into typed review values"
        );
        assert!(
            production_source
                .contains("fn review_results_service() -> Arc<dyn ReviewResultsService>"),
            "the focused results interactor must be wired separately from the aggregate"
        );
    }

    #[test]
    fn test_review_composition_root_review_fix_driver_factory_is_wire_only() {
        struct CountingService {
            invocations: Arc<AtomicUsize>,
        }

        impl RunReviewFixService for CountingService {
            fn run(
                &self,
                _request: RunReviewFixRequest,
            ) -> Result<RunReviewFixOutput, RunReviewFixError> {
                self.invocations.fetch_add(1, Ordering::SeqCst);
                Err(RunReviewFixError::FixRunnerFailed(ReviewFixRunnerError::Unexpected(
                    DiagnosticText::new("unexpected review-fix invocation"),
                )))
            }
        }

        let invocations = Arc::new(AtomicUsize::new(0));
        let root = ReviewCompositionRoot::new();
        let production_driver = root.review_fix_driver();
        assert_eq!(
            invocations.load(Ordering::SeqCst),
            0,
            "constructing the root and its production driver must not execute a use case"
        );
        drop(production_driver);

        let injected_driver = root.review_fix_driver_with_service(Arc::new(CountingService {
            invocations: Arc::clone(&invocations),
        }));
        assert_eq!(
            invocations.load(Ordering::SeqCst),
            0,
            "wiring an injected review-fix service must not execute or render it"
        );
        drop(injected_driver);
        assert_eq!(invocations.load(Ordering::SeqCst), 0);
    }
}
