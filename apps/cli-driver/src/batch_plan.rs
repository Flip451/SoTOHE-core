//! `batch-plan` command family — primary adapter driver (IN-06, IN-07, AC-06,
//! AC-07).
//!
//! [`BatchPlanDriver`] holds the injected gate service, hands it the transport
//! input, and renders the output record it returns: exit 0 when the plan
//! conforms, exit 1 with every finding named when it does not. Each finding is
//! rendered with its own figures rather than as a count, so the output says
//! which batch, which scope and by how much. Only the use case's own records
//! are rendered — no domain value object reaches this layer.

use std::sync::Arc;

use usecase::batch_plan::{
    BatchPlanCheckCommand, BatchPlanCheckError, BatchPlanCheckOutput, BatchPlanCheckService,
    BatchPlanViolationOutput,
};

use crate::render::CommandOutcome;

// ---------------------------------------------------------------------------
// BatchPlanInput
// ---------------------------------------------------------------------------

/// Typed input for the batch-plan command family, carrying the track context
/// unresolved.
///
/// `track_id` is optional because the bin passes the raw flag through without
/// deciding anything: resolving an omitted id from the current branch and
/// validating the result into a domain identifier both happen inside the
/// driver, so the bin needs no working-directory read. It stays a raw transport
/// string at this surface; `items_dir` travels with the input rather than with
/// construction, so the composition root stays a zero-argument wiring accessor.
#[derive(Debug, Clone)]
pub enum BatchPlanInput {
    /// Run the Phase 3 gate over the track's declared batch plan.
    Check {
        /// Active track identifier as given on the command line, or `None` to
        /// resolve it from the current branch.
        track_id: Option<String>,
        /// Directory the track's artifacts live under.
        items_dir: std::path::PathBuf,
    },
}

// ---------------------------------------------------------------------------
// BatchPlanDriver
// ---------------------------------------------------------------------------

/// Primary adapter for the batch-plan command family.
///
/// Hands the unresolved track context to the gate service and renders whatever
/// comes back: resolving it is part of that use case, so this adapter drives a
/// single service and decides nothing itself.
pub struct BatchPlanDriver {
    check_service: Arc<dyn BatchPlanCheckService>,
}

impl std::fmt::Debug for BatchPlanDriver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BatchPlanDriver").finish_non_exhaustive()
    }
}

impl BatchPlanDriver {
    /// Injects the gate service this driver runs through.
    #[must_use]
    pub fn new(check_service: Arc<dyn BatchPlanCheckService>) -> BatchPlanDriver {
        BatchPlanDriver { check_service }
    }

    /// Dispatches an input and renders the result.
    pub fn handle(&self, input: BatchPlanInput) -> CommandOutcome {
        match input {
            BatchPlanInput::Check { track_id, items_dir } => self.handle_check(track_id, items_dir),
        }
    }

    fn handle_check(
        &self,
        track_id: Option<String>,
        items_dir: std::path::PathBuf,
    ) -> CommandOutcome {
        match self.check_service.check(BatchPlanCheckCommand { track_id, items_dir }) {
            Ok(BatchPlanCheckOutput::Passed) => CommandOutcome::success(None),
            Ok(BatchPlanCheckOutput::Blocked { violations }) => {
                CommandOutcome::failure(Some(render_violations(violations.as_slice())))
            }
            Err(error) => CommandOutcome::failure(Some(render_error(&error))),
        }
    }
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

fn render_error(error: &BatchPlanCheckError) -> String {
    format!("[BLOCKED] batch-plan check could not run: {error}")
}

fn render_violations(violations: &[BatchPlanViolationOutput]) -> String {
    let mut lines =
        vec![format!("[BLOCKED] batch-plan check found {} violation(s):", violations.len())];
    lines.extend(violations.iter().map(render_violation));
    lines.join("\n")
}

fn render_violation(violation: &BatchPlanViolationOutput) -> String {
    match violation {
        BatchPlanViolationOutput::CeilingExceeded { batch_id, scope, total, ceiling } => format!(
            "- batch '{batch_id}' reaches {total} lines in scope '{scope}', above its ceiling of \
             {ceiling}"
        ),
        BatchPlanViolationOutput::OversizeScopeHasMultipleContributors {
            batch_id,
            scope,
            indivisible_task,
            other_contributors,
        } => format!(
            "- batch '{batch_id}' is over the ceiling in scope '{scope}', where the indivisible \
             task '{indivisible_task}' is not the only contributor (also: {})",
            other_contributors.join(", ")
        ),
        BatchPlanViolationOutput::UnknownTaskRef { task_id } => {
            format!(
                "- the batch plan names task '{task_id}', which the implementation plan does not have"
            )
        }
        BatchPlanViolationOutput::UnplannedTask { task_id } => {
            format!("- planned task '{task_id}' belongs to no batch")
        }
        BatchPlanViolationOutput::DependencyInLaterBatch {
            task_id,
            task_batch,
            dependency,
            dependency_batch,
        } => format!(
            "- task '{task_id}' in batch '{task_batch}' declares a dependency on '{dependency}', \
             which batch '{dependency_batch}' only delivers later"
        ),
    }
}
#[cfg(test)]
#[allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use domain::FreeText;
    use usecase::batch_plan::NonEmptyViolationOutputs;

    use super::*;

    struct StubService {
        outcome: Option<BatchPlanCheckOutput>,
    }

    /// Records the command it was handed and always passes.
    struct RecordingService {
        seen: std::sync::Mutex<Vec<(Option<String>, std::path::PathBuf)>>,
    }

    impl RecordingService {
        fn new() -> Self {
            Self { seen: std::sync::Mutex::new(Vec::new()) }
        }
    }

    impl BatchPlanCheckService for RecordingService {
        fn check(
            &self,
            cmd: BatchPlanCheckCommand,
        ) -> Result<BatchPlanCheckOutput, BatchPlanCheckError> {
            self.seen.lock().unwrap().push((cmd.track_id, cmd.items_dir));
            Ok(BatchPlanCheckOutput::Passed)
        }
    }

    impl BatchPlanCheckService for StubService {
        fn check(
            &self,
            _cmd: BatchPlanCheckCommand,
        ) -> Result<BatchPlanCheckOutput, BatchPlanCheckError> {
            match &self.outcome {
                Some(outcome) => Ok(outcome.clone()),
                None => Err(BatchPlanCheckError::BatchPlanNotFound),
            }
        }
    }

    fn driver(outcome: Option<BatchPlanCheckOutput>) -> BatchPlanDriver {
        BatchPlanDriver::new(Arc::new(StubService { outcome }))
    }

    fn blocked(violations: Vec<BatchPlanViolationOutput>) -> BatchPlanCheckOutput {
        BatchPlanCheckOutput::Blocked {
            violations: NonEmptyViolationOutputs::try_new(violations).unwrap(),
        }
    }

    fn check(track_id: &str) -> BatchPlanInput {
        BatchPlanInput::Check {
            track_id: Some(track_id.to_owned()),
            items_dir: std::path::PathBuf::from("track/items"),
        }
    }

    fn check_without_track() -> BatchPlanInput {
        BatchPlanInput::Check { track_id: None, items_dir: std::path::PathBuf::from("track/items") }
    }

    #[test]
    fn test_the_unresolved_track_context_is_handed_to_the_service_as_it_arrived() {
        // Both shapes reach the service unchanged: the driver decides nothing
        // about the track, and resolution happens inside the use case.
        let recording = Arc::new(RecordingService::new());
        let driver = BatchPlanDriver::new(Arc::clone(&recording) as Arc<dyn BatchPlanCheckService>);

        assert_eq!(driver.handle(check("some-track")).exit_code, 0);
        assert_eq!(driver.handle(check_without_track()).exit_code, 0);

        let seen = recording.seen.lock().unwrap().clone();
        assert_eq!(
            seen,
            vec![
                (Some("some-track".to_owned()), std::path::PathBuf::from("track/items")),
                (None, std::path::PathBuf::from("track/items")),
            ]
        );
    }

    #[test]
    fn test_a_track_that_could_not_be_resolved_is_rendered_as_a_failure() {
        struct UnresolvableService;
        impl BatchPlanCheckService for UnresolvableService {
            fn check(
                &self,
                _cmd: BatchPlanCheckCommand,
            ) -> Result<BatchPlanCheckOutput, BatchPlanCheckError> {
                Err(BatchPlanCheckError::TrackResolutionFailed {
                    message: FreeText::new("branch read failed: not a track branch"),
                })
            }
        }

        let outcome =
            BatchPlanDriver::new(Arc::new(UnresolvableService)).handle(check_without_track());

        assert_eq!(outcome.exit_code, 1);
        let rendered = outcome.stderr.unwrap();
        assert!(rendered.contains("no active track could be resolved"), "rendered as: {rendered}");
    }

    #[test]
    fn test_a_conforming_plan_exits_zero_with_nothing_to_report() {
        let outcome = driver(Some(BatchPlanCheckOutput::Passed)).handle(check("some-track"));

        assert_eq!(outcome.exit_code, 0);
        assert_eq!(outcome.stderr, None);
    }

    #[test]
    fn test_a_blocked_plan_exits_one_and_names_each_findings_own_figures() {
        let outcome = driver(Some(blocked(vec![
            BatchPlanViolationOutput::CeilingExceeded {
                batch_id: "B1".to_owned(),
                scope: "domain".to_owned(),
                total: 650,
                ceiling: 500,
            },
            BatchPlanViolationOutput::DependencyInLaterBatch {
                task_id: "T001".to_owned(),
                task_batch: "B1".to_owned(),
                dependency: "T002".to_owned(),
                dependency_batch: "B2".to_owned(),
            },
        ])))
        .handle(check("some-track"));

        assert_eq!(outcome.exit_code, 1);
        let rendered = outcome.stderr.unwrap();
        assert!(rendered.contains("2 violation(s)"), "rendered as: {rendered}");
        assert!(rendered.contains("650") && rendered.contains("500"), "rendered as: {rendered}");
        assert!(rendered.contains("domain") && rendered.contains("B1"), "rendered as: {rendered}");
        assert!(
            rendered.contains("T001") && rendered.contains("T002") && rendered.contains("B2"),
            "rendered as: {rendered}"
        );
    }

    #[test]
    fn test_a_failure_to_run_the_check_is_reported_apart_from_a_verdict() {
        let outcome = driver(None).handle(check("some-track"));

        assert_eq!(outcome.exit_code, 1);
        let rendered = outcome.stderr.unwrap();
        assert!(rendered.contains("could not run"), "rendered as: {rendered}");
        assert!(rendered.contains("no batch plan"), "rendered as: {rendered}");
    }

    #[test]
    fn test_every_finding_kind_renders_the_parties_it_names() {
        let rendered = driver(Some(blocked(vec![
            BatchPlanViolationOutput::OversizeScopeHasMultipleContributors {
                batch_id: "B1".to_owned(),
                scope: "domain".to_owned(),
                indivisible_task: "T001".to_owned(),
                other_contributors: vec!["T002".to_owned()],
            },
            BatchPlanViolationOutput::UnknownTaskRef { task_id: "T404".to_owned() },
            BatchPlanViolationOutput::UnplannedTask { task_id: "T009".to_owned() },
        ])))
        .handle(check("some-track"))
        .stderr
        .unwrap();

        assert!(rendered.contains("T001") && rendered.contains("T002"), "{rendered}");
        assert!(rendered.contains("T404"), "{rendered}");
        assert!(rendered.contains("T009"), "{rendered}");
        assert_eq!(rendered.lines().count(), 4, "one header plus one line per finding");
    }

    #[test]
    fn test_each_cross_file_finding_on_its_own_makes_the_command_fail() {
        let cases = [
            (
                "unplanned",
                BatchPlanViolationOutput::UnplannedTask { task_id: "T009".to_owned() },
                "T009",
            ),
            (
                "unknown ref",
                BatchPlanViolationOutput::UnknownTaskRef { task_id: "T404".to_owned() },
                "T404",
            ),
            (
                "dependency in a later batch",
                BatchPlanViolationOutput::DependencyInLaterBatch {
                    task_id: "T001".to_owned(),
                    task_batch: "B1".to_owned(),
                    dependency: "T002".to_owned(),
                    dependency_batch: "B2".to_owned(),
                },
                "T002",
            ),
            (
                "oversize scope with several contributors",
                BatchPlanViolationOutput::OversizeScopeHasMultipleContributors {
                    batch_id: "B1".to_owned(),
                    scope: "domain".to_owned(),
                    indivisible_task: "T001".to_owned(),
                    other_contributors: vec!["T002".to_owned()],
                },
                "T001",
            ),
        ];

        for (label, violation, named) in cases {
            let outcome = driver(Some(blocked(vec![violation]))).handle(check("some-track"));

            assert_eq!(outcome.exit_code, 1, "{label} alone must fail the command");
            let rendered = outcome.stderr.unwrap();
            assert!(rendered.contains("1 violation(s)"), "{label}: {rendered}");
            assert!(rendered.contains(named), "{label} must name {named}: {rendered}");
        }
    }

    #[test]
    fn test_a_read_failure_message_reaches_the_operator() {
        struct FailingService;
        impl BatchPlanCheckService for FailingService {
            fn check(
                &self,
                _cmd: BatchPlanCheckCommand,
            ) -> Result<BatchPlanCheckOutput, BatchPlanCheckError> {
                Err(BatchPlanCheckError::ScopeConfigReadFailed {
                    message: FreeText::new("review-scope.json is unreadable"),
                })
            }
        }

        let outcome = BatchPlanDriver::new(Arc::new(FailingService)).handle(check("some-track"));

        assert_eq!(outcome.exit_code, 1);
        assert!(outcome.stderr.unwrap().contains("review-scope.json is unreadable"));
    }
}
