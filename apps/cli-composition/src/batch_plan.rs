//! Composition root for the `batch-plan` command family (IN-06, AC-06).
//!
//! Wires the three filesystem adapters into the gate interactor, wires the
//! pre-existing active-track resolve interactor over the git branch reader, and
//! hands both to the driver. Pure dependency injection: the accessor takes no
//! argument and returns no error, because the items directory travels with the
//! driver input and each adapter resolves its paths per call.

use std::sync::Arc;

use cli_driver::batch_plan::BatchPlanDriver;
use infrastructure::batch_plan_reader::FsBatchPlanReader;
use infrastructure::branch_reader::LazyBranchReader;
use infrastructure::planned_task_reader::FsPlannedTaskReader;
use infrastructure::review_scope_config_reader::FsReviewScopeConfigReader;
use usecase::batch_plan::{
    BatchPlanCheckInteractor, BatchPlanReaderPort, PlannedTaskReaderPort, ScopeConfigReaderPort,
};
use usecase::track_resolution::{ActiveTrackResolveInteractor, ActiveTrackResolveService};

/// Composition root for the batch-plan command family.
#[derive(Debug, Default)]
pub struct BatchPlanCompositionRoot;

impl BatchPlanCompositionRoot {
    /// Creates the composition root.
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Builds a fully wired [`BatchPlanDriver`].
    #[must_use]
    pub fn batch_plan_driver(&self) -> BatchPlanDriver {
        let batch_plan_reader: Arc<dyn BatchPlanReaderPort> = Arc::new(FsBatchPlanReader::new());
        let planned_task_reader: Arc<dyn PlannedTaskReaderPort> =
            Arc::new(FsPlannedTaskReader::new());
        let scope_config_reader: Arc<dyn ScopeConfigReaderPort> =
            Arc::new(FsReviewScopeConfigReader::new());

        let track_resolver: Arc<dyn ActiveTrackResolveService> =
            Arc::new(ActiveTrackResolveInteractor::new(Arc::new(LazyBranchReader::new())));

        BatchPlanDriver::new(Arc::new(BatchPlanCheckInteractor::new(
            batch_plan_reader,
            planned_task_reader,
            scope_config_reader,
            track_resolver,
        )))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use cli_driver::batch_plan::BatchPlanInput;

    use super::*;

    /// Writes a track's two planning artifacts into a fresh items directory and
    /// returns it. `batch_plan` is the artifact text under test.
    fn items_dir_with(batch_plan: &str, impl_plan: &str) -> tempfile::TempDir {
        let dir = tempfile::Builder::new()
            .prefix("batch-plan-composition-")
            .tempdir_in(std::env::current_dir().unwrap())
            .unwrap();
        let track_dir = dir.path().join("some-track");
        std::fs::create_dir_all(&track_dir).unwrap();
        std::fs::write(track_dir.join("batch-plan.json"), batch_plan).unwrap();
        std::fs::write(track_dir.join("impl-plan.json"), impl_plan).unwrap();
        dir
    }

    const TWO_TODO_TASKS: &str = r#"{
      "schema_version": 2,
      "tasks": [
        {"id": "T001", "description": "First", "status": "todo"},
        {"id": "T002", "description": "Second", "status": "todo"}
      ],
      "plan": {"summary": [], "sections": [
        {"id": "S1", "title": "Impl", "description": [], "task_ids": ["T001", "T002"]}
      ]}
    }"#;

    const ONE_TODO_TASK: &str = r#"{
      "schema_version": 2,
      "tasks": [{"id": "T001", "description": "First", "status": "todo"}],
      "plan": {"summary": [], "sections": [
        {"id": "S1", "title": "Impl", "description": [], "task_ids": ["T001"]}
      ]}
    }"#;

    fn check(items_dir: &std::path::Path) -> cli_driver::render::CommandOutcome {
        BatchPlanCompositionRoot::new().batch_plan_driver().handle(BatchPlanInput::Check {
            track_id: Some("some-track".to_owned()),
            items_dir: items_dir.to_path_buf(),
        })
    }

    #[test]
    fn test_the_wired_driver_blocks_a_batch_whose_scope_sum_passes_the_resolved_ceiling() {
        // The shipped configuration gives `domain` a 500-line ceiling; B1
        // declares 300 + 250 = 550 across two decomposable tasks.
        let dir = items_dir_with(
            r#"{
              "schema_version": 1,
              "track_id": "some-track",
              "task_estimates": [
                {"task_id": "T001", "scope_estimates": [
                  {"scope": "domain", "production_lines": 300, "test_lines": 0}]},
                {"task_id": "T002", "scope_estimates": [
                  {"scope": "domain", "production_lines": 250, "test_lines": 0}]}
              ],
              "batches": [{"id": "B1", "task_ids": ["T001", "T002"]}]
            }"#,
            TWO_TODO_TASKS,
        );

        let outcome = check(dir.path());

        assert_eq!(outcome.exit_code, 1, "an excess is fail-closed");
        let rendered = outcome.stderr.unwrap();
        assert!(rendered.contains("550") && rendered.contains("500"), "rendered as: {rendered}");
        assert!(rendered.contains("domain") && rendered.contains("B1"), "rendered as: {rendered}");
    }

    #[test]
    fn test_the_wired_driver_lets_a_single_justified_contributor_through_the_same_ceiling() {
        // 700 + 300 = 1000 lines against the same 500-line ceiling, declared by
        // the one task that states why it cannot be split.
        let dir = items_dir_with(
            r#"{
              "schema_version": 1,
              "track_id": "some-track",
              "task_estimates": [
                {"task_id": "T001",
                 "scope_estimates": [
                   {"scope": "domain", "production_lines": 700, "test_lines": 300}],
                 "oversize_justification": "the transition table cannot be split"}
              ],
              "batches": [{"id": "B1", "task_ids": ["T001"]}]
            }"#,
            ONE_TODO_TASK,
        );

        let outcome = check(dir.path());

        assert_eq!(outcome.exit_code, 0, "the sole-contributor exemption applies");
        assert_eq!(outcome.stderr, None);
    }

    #[test]
    fn test_the_wired_driver_keeps_checking_the_other_scopes_of_an_over_budget_batch() {
        // domain reaches 550 against 500; usecase stays at 60 and passes.
        let dir = items_dir_with(
            r#"{
              "schema_version": 1,
              "track_id": "some-track",
              "task_estimates": [
                {"task_id": "T001", "scope_estimates": [
                  {"scope": "domain", "production_lines": 300, "test_lines": 250},
                  {"scope": "usecase", "production_lines": 40, "test_lines": 20}]}
              ],
              "batches": [{"id": "B1", "task_ids": ["T001"]}]
            }"#,
            ONE_TODO_TASK,
        );

        let outcome = check(dir.path());

        assert_eq!(outcome.exit_code, 1);
        let rendered = outcome.stderr.unwrap();
        assert!(rendered.contains("1 violation(s)"), "only domain is reported: {rendered}");
        assert!(rendered.contains("domain") && rendered.contains("550"), "{rendered}");
        assert!(!rendered.contains("usecase"), "usecase stays within its ceiling: {rendered}");
    }

    #[test]
    fn test_the_root_wires_a_driver_with_no_argument_and_no_error_channel() {
        let driver = BatchPlanCompositionRoot::new().batch_plan_driver();

        // The items directory travels with the input, not with construction.
        let outcome = driver.handle(BatchPlanInput::Check {
            track_id: Some("Not A Track".to_owned()),
            items_dir: std::path::PathBuf::from("track/items"),
        });

        assert_eq!(outcome.exit_code, 1, "the wired driver validates its transport identifier");
    }

    #[test]
    fn test_the_default_root_wires_the_same_driver_as_the_constructor() {
        let from_default: BatchPlanCompositionRoot = Default::default();
        let from_default = from_default.batch_plan_driver();
        let from_new = BatchPlanCompositionRoot::new().batch_plan_driver();

        for driver in [from_default, from_new] {
            let outcome = driver.handle(BatchPlanInput::Check {
                track_id: Some("no-such-track".to_owned()),
                items_dir: std::path::PathBuf::from("track/items"),
            });
            // The wired reader reports the absent plan rather than passing.
            assert_eq!(outcome.exit_code, 1);
            assert!(outcome.stderr.unwrap().contains("no batch plan"));
        }
    }
}
