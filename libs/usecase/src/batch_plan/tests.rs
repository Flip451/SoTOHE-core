//! Contract tests for the batch-plan ports and the Phase 3 gate interactor.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use domain::batch_plan::{
    BatchDeclaration, BatchId, BatchPlanDocument, BatchPlanGateOutcome, BatchPlanGateViolation,
    BatchPlanValidationError, IndivisibilityJustification, LineCount, MeasuredScopeDiff,
    NonEmptyGateViolations, ScopeLineEstimate, TaskDecomposition, TaskEstimate,
};
use domain::review_v2::{MainScopeName, ReviewScopeConfig, ScopeName};
use domain::{FreeText, NonEmptyString, TaskId, TaskStatus, TrackId, TrackTask};

use super::{
    BatchPlanCheckCommand, BatchPlanCheckError, BatchPlanCheckInteractor, BatchPlanCheckService,
    BatchPlanReadError, BatchPlanReaderPort, PlannedTaskReadError, PlannedTaskReaderPort,
    ScopeConfigReadError, ScopeConfigReaderPort, ScopeDiffMeasureError, ScopeDiffMeasurePort,
};

// ── fixtures ──────────────────────────────────────────────────────────────────

fn track() -> TrackId {
    TrackId::try_new("scope-diff-ceiling-admission-enforcement-2026-07-29").unwrap()
}

fn items_dir() -> PathBuf {
    PathBuf::from("track/items")
}

fn task(id: &str) -> TaskId {
    TaskId::try_new(id).unwrap()
}

fn scope(name: &str) -> ScopeName {
    ScopeName::Main(MainScopeName::new(name).unwrap())
}

fn planned_task(id: &str, depends_on: &[&str]) -> TrackTask {
    TrackTask::with_dependencies(
        task(id),
        NonEmptyString::try_new(format!("task {id}")).unwrap(),
        TaskStatus::Todo,
        depends_on.iter().map(|dependency| task(dependency)).collect(),
    )
}

/// A one-batch plan holding T001 with a 150-line domain estimate.
fn plan_document() -> BatchPlanDocument {
    BatchPlanDocument::new(
        track(),
        vec![
            TaskEstimate::new(
                task("T001"),
                vec![ScopeLineEstimate::new(
                    scope("domain"),
                    LineCount::new(100),
                    LineCount::new(50),
                )],
                TaskDecomposition::Decomposable,
            )
            .unwrap(),
        ],
        vec![BatchDeclaration::new(BatchId::try_new("B1").unwrap(), vec![task("T001")]).unwrap()],
    )
    .unwrap()
}

fn command() -> BatchPlanCheckCommand {
    BatchPlanCheckCommand { track_id: track(), items_dir: items_dir() }
}

// ── stubs ─────────────────────────────────────────────────────────────────────

type Calls = Mutex<Vec<(PathBuf, TrackId)>>;

enum BatchPlanStub {
    Document,
    NotFound,
    ReadFailed,
    /// The plan the test built itself.
    Plan(BatchPlanDocument),
}

struct StubBatchPlanReader {
    stub: BatchPlanStub,
    calls: Calls,
}

impl StubBatchPlanReader {
    fn new(stub: BatchPlanStub) -> Self {
        Self { stub, calls: Mutex::new(Vec::new()) }
    }
}

impl BatchPlanReaderPort for StubBatchPlanReader {
    fn read(
        &self,
        items_dir: &Path,
        track_id: &TrackId,
    ) -> Result<BatchPlanDocument, BatchPlanReadError> {
        self.calls.lock().unwrap().push((items_dir.to_path_buf(), track_id.clone()));
        match &self.stub {
            BatchPlanStub::Document => Ok(plan_document()),
            BatchPlanStub::NotFound => Err(BatchPlanReadError::NotFound),
            BatchPlanStub::ReadFailed => {
                Err(BatchPlanReadError::ReadFailed { message: FreeText::new("unreadable") })
            }
            BatchPlanStub::Plan(plan) => Ok(plan.clone()),
        }
    }
}

enum PlannedTaskStub {
    Tasks(Vec<TrackTask>),
    NotFound,
    ReadFailed,
}

struct StubPlannedTaskReader {
    stub: PlannedTaskStub,
    calls: Calls,
}

impl StubPlannedTaskReader {
    fn new(stub: PlannedTaskStub) -> Self {
        Self { stub, calls: Mutex::new(Vec::new()) }
    }
}

impl PlannedTaskReaderPort for StubPlannedTaskReader {
    fn read_planned_tasks(
        &self,
        items_dir: &Path,
        track_id: &TrackId,
    ) -> Result<Vec<TrackTask>, PlannedTaskReadError> {
        self.calls.lock().unwrap().push((items_dir.to_path_buf(), track_id.clone()));
        match &self.stub {
            PlannedTaskStub::Tasks(tasks) => Ok(tasks.clone()),
            PlannedTaskStub::NotFound => Err(PlannedTaskReadError::NotFound),
            PlannedTaskStub::ReadFailed => {
                Err(PlannedTaskReadError::ReadFailed { message: FreeText::new("unreadable") })
            }
        }
    }
}

struct StubScopeConfigReader {
    ceiling: Option<u32>,
    fails: bool,
    calls: Calls,
}

impl StubScopeConfigReader {
    fn new(ceiling: Option<u32>) -> Self {
        Self { ceiling, fails: false, calls: Mutex::new(Vec::new()) }
    }

    fn failing() -> Self {
        Self { ceiling: None, fails: true, calls: Mutex::new(Vec::new()) }
    }
}

impl ScopeConfigReaderPort for StubScopeConfigReader {
    fn read(
        &self,
        items_dir: &Path,
        track_id: &TrackId,
    ) -> Result<ReviewScopeConfig, ScopeConfigReadError> {
        self.calls.lock().unwrap().push((items_dir.to_path_buf(), track_id.clone()));
        if self.fails {
            return Err(ScopeConfigReadError::ReadFailed { message: FreeText::new("unreadable") });
        }
        ReviewScopeConfig::new(
            track_id,
            vec![("domain".to_owned(), vec!["libs/domain/**".to_owned()], None, self.ceiling)],
            Vec::new(),
            Vec::new(),
            None,
        )
        .map_err(|error| ScopeConfigReadError::ReadFailed {
            message: FreeText::new(error.to_string()),
        })
    }
}

struct StubScopeDiffMeasurer {
    measured: Vec<MeasuredScopeDiff>,
    fails: bool,
}

impl ScopeDiffMeasurePort for StubScopeDiffMeasurer {
    fn measure_scope_diff(
        &self,
        _items_dir: &Path,
        _track_id: &TrackId,
    ) -> Result<Vec<MeasuredScopeDiff>, ScopeDiffMeasureError> {
        if self.fails {
            return Err(ScopeDiffMeasureError::MeasureFailed {
                message: FreeText::new("git unavailable"),
            });
        }
        Ok(self.measured.clone())
    }
}

fn interactor(
    batch_plan: BatchPlanStub,
    planned: PlannedTaskStub,
    scope_config: StubScopeConfigReader,
) -> BatchPlanCheckInteractor {
    BatchPlanCheckInteractor::new(
        Arc::new(StubBatchPlanReader::new(batch_plan)),
        Arc::new(StubPlannedTaskReader::new(planned)),
        Arc::new(scope_config),
    )
}

/// Runs the gate over a plan and task list the test built itself, under
/// `ceiling` for the `domain` scope.
fn gate_over(
    plan: BatchPlanDocument,
    planned: Vec<TrackTask>,
    ceiling: Option<u32>,
) -> BatchPlanGateOutcome {
    interactor(
        BatchPlanStub::Plan(plan),
        PlannedTaskStub::Tasks(planned),
        StubScopeConfigReader::new(ceiling),
    )
    .check(command())
    .unwrap()
}

fn estimate(id: &str, scopes: &[(&str, u32, u32)], indivisible: Option<&str>) -> TaskEstimate {
    TaskEstimate::new(
        task(id),
        scopes
            .iter()
            .map(|(name, production, test)| {
                ScopeLineEstimate::new(
                    scope(name),
                    LineCount::new(*production),
                    LineCount::new(*test),
                )
            })
            .collect(),
        match indivisible {
            Some(reason) => TaskDecomposition::Indivisible(
                IndivisibilityJustification::try_new(reason).unwrap(),
            ),
            None => TaskDecomposition::Decomposable,
        },
    )
    .unwrap()
}

fn batch(id: &str, members: &[&str]) -> BatchDeclaration {
    BatchDeclaration::new(
        BatchId::try_new(id).unwrap(),
        members.iter().map(|member| task(member)).collect(),
    )
    .unwrap()
}

fn plan_of(estimates: Vec<TaskEstimate>, batches: Vec<BatchDeclaration>) -> BatchPlanDocument {
    BatchPlanDocument::new(track(), estimates, batches).unwrap()
}

// ── BatchPlanCheckCommand ─────────────────────────────────────────────────────

#[test]
fn test_the_check_command_carries_the_track_and_the_items_directory() {
    let cmd = command();

    assert_eq!(cmd.track_id, track());
    assert_eq!(cmd.items_dir, items_dir());
    assert_eq!(cmd.clone(), cmd);
}

// ── driven ports ──────────────────────────────────────────────────────────────

#[test]
fn test_the_batch_plan_reader_port_hands_the_document_it_read_to_its_caller() {
    let reader = StubBatchPlanReader::new(BatchPlanStub::Document);

    let document = reader.read(&items_dir(), &track()).unwrap();

    assert_eq!(document.track_id(), &track());
    assert_eq!(document.batches().len(), 1);
    assert_eq!(reader.calls.lock().unwrap().as_slice(), &[(items_dir(), track())]);
}

#[test]
fn test_the_batch_plan_reader_port_reports_an_absent_plan_apart_from_a_read_failure() {
    let absent = StubBatchPlanReader::new(BatchPlanStub::NotFound);
    let unreadable = StubBatchPlanReader::new(BatchPlanStub::ReadFailed);

    assert!(matches!(absent.read(&items_dir(), &track()), Err(BatchPlanReadError::NotFound)));
    assert!(matches!(
        unreadable.read(&items_dir(), &track()),
        Err(BatchPlanReadError::ReadFailed { .. })
    ));
}

#[test]
fn test_the_planned_task_reader_port_returns_the_tasks_with_their_declared_dependencies() {
    let reader = StubPlannedTaskReader::new(PlannedTaskStub::Tasks(vec![
        planned_task("T001", &[]),
        planned_task("T002", &["T001"]),
    ]));

    let tasks = reader.read_planned_tasks(&items_dir(), &track()).unwrap();

    assert_eq!(tasks.len(), 2);
    assert!(tasks[0].depends_on().is_empty());
    assert_eq!(tasks[1].depends_on(), &[task("T001")]);
    assert_eq!(reader.calls.lock().unwrap().as_slice(), &[(items_dir(), track())]);
}

#[test]
fn test_the_planned_task_reader_port_reports_an_absent_plan_apart_from_a_read_failure() {
    let absent = StubPlannedTaskReader::new(PlannedTaskStub::NotFound);
    let unreadable = StubPlannedTaskReader::new(PlannedTaskStub::ReadFailed);

    assert!(matches!(
        absent.read_planned_tasks(&items_dir(), &track()),
        Err(PlannedTaskReadError::NotFound)
    ));
    assert!(matches!(
        unreadable.read_planned_tasks(&items_dir(), &track()),
        Err(PlannedTaskReadError::ReadFailed { .. })
    ));
}

#[test]
fn test_the_scope_config_reader_port_returns_the_configuration_the_ceilings_come_from() {
    let reader = StubScopeConfigReader::new(Some(500));

    let config = reader.read(&items_dir(), &track()).unwrap();

    assert_eq!(config.diff_ceiling_for_scope(&scope("domain")), Some(500));
    assert_eq!(config.diff_ceiling_for_scope(&scope("usecase")), None);
    assert_eq!(reader.calls.lock().unwrap().as_slice(), &[(items_dir(), track())]);
    assert!(matches!(
        StubScopeConfigReader::failing().read(&items_dir(), &track()),
        Err(ScopeConfigReadError::ReadFailed { .. })
    ));
}

#[test]
fn test_the_scope_diff_measure_port_returns_one_measured_figure_per_scope() {
    let measurer = StubScopeDiffMeasurer {
        measured: vec![MeasuredScopeDiff::new(scope("domain"), LineCount::new(412))],
        fails: false,
    };

    let measured = measurer.measure_scope_diff(&items_dir(), &track()).unwrap();

    assert_eq!(measured.len(), 1);
    assert_eq!(measured[0].scope(), &scope("domain"));
    assert_eq!(measured[0].lines(), LineCount::new(412));

    let failing = StubScopeDiffMeasurer { measured: Vec::new(), fails: true };
    assert!(matches!(
        failing.measure_scope_diff(&items_dir(), &track()),
        Err(ScopeDiffMeasureError::MeasureFailed { .. })
    ));
}

// ── BatchPlanCheckInteractor ──────────────────────────────────────────────────

#[test]
fn test_the_interactor_returns_the_domain_verdict_for_a_conforming_plan() {
    let service = interactor(
        BatchPlanStub::Document,
        PlannedTaskStub::Tasks(vec![planned_task("T001", &[])]),
        StubScopeConfigReader::new(Some(500)),
    );

    let outcome = service.check(command()).unwrap();

    assert_eq!(outcome, BatchPlanGateOutcome::Passed);
}

#[test]
fn test_the_interactor_returns_the_blocked_verdict_with_the_findings_it_carries() {
    // T002 is planned but the batch plan places only T001.
    let service = interactor(
        BatchPlanStub::Document,
        PlannedTaskStub::Tasks(vec![planned_task("T001", &[]), planned_task("T002", &[])]),
        StubScopeConfigReader::new(Some(500)),
    );

    let outcome = service.check(command()).unwrap();

    assert_eq!(
        outcome.violations().map(NonEmptyGateViolations::as_slice),
        Some(&[BatchPlanGateViolation::UnplannedTask { task_id: task("T002") }][..])
    );
}

#[test]
fn test_the_interactor_reports_an_absent_batch_plan_as_an_error_rather_than_a_pass() {
    let service = interactor(
        BatchPlanStub::NotFound,
        PlannedTaskStub::Tasks(vec![planned_task("T001", &[])]),
        StubScopeConfigReader::new(Some(500)),
    );

    assert!(matches!(service.check(command()), Err(BatchPlanCheckError::BatchPlanNotFound)));
}

#[test]
fn test_the_interactor_maps_each_port_failure_to_its_own_error() {
    let unreadable_plan = interactor(
        BatchPlanStub::ReadFailed,
        PlannedTaskStub::Tasks(Vec::new()),
        StubScopeConfigReader::new(Some(500)),
    );
    assert!(matches!(
        unreadable_plan.check(command()),
        Err(BatchPlanCheckError::BatchPlanReadFailed { .. })
    ));

    let absent_impl_plan = interactor(
        BatchPlanStub::Document,
        PlannedTaskStub::NotFound,
        StubScopeConfigReader::new(Some(500)),
    );
    assert!(matches!(
        absent_impl_plan.check(command()),
        Err(BatchPlanCheckError::ImplPlanNotFound)
    ));

    let unreadable_impl_plan = interactor(
        BatchPlanStub::Document,
        PlannedTaskStub::ReadFailed,
        StubScopeConfigReader::new(Some(500)),
    );
    assert!(matches!(
        unreadable_impl_plan.check(command()),
        Err(BatchPlanCheckError::ImplPlanReadFailed { .. })
    ));

    let unreadable_config = interactor(
        BatchPlanStub::Document,
        PlannedTaskStub::Tasks(vec![planned_task("T001", &[])]),
        StubScopeConfigReader::failing(),
    );
    assert!(matches!(
        unreadable_config.check(command()),
        Err(BatchPlanCheckError::ScopeConfigReadFailed { .. })
    ));
}

#[test]
fn test_the_check_returns_a_fail_closed_error_when_a_batch_scope_sum_passes_the_ceiling() {
    // B1 = T001 (300 + 100) + T002 (200 + 50) = 650 domain lines against 500.
    let outcome = gate_over(
        plan_of(
            vec![
                estimate("T001", &[("domain", 300, 100)], None),
                estimate("T002", &[("domain", 200, 50)], None),
            ],
            vec![batch("B1", &["T001", "T002"])],
        ),
        vec![planned_task("T001", &[]), planned_task("T002", &[])],
        Some(500),
    );

    assert_eq!(
        outcome.violations().map(NonEmptyGateViolations::as_slice),
        Some(
            &[BatchPlanGateViolation::CeilingExceeded {
                batch_id: BatchId::try_new("B1").unwrap(),
                scope: scope("domain"),
                total: LineCount::new(650),
                ceiling: LineCount::new(500),
            }][..]
        )
    );
}

#[test]
fn test_the_check_lets_an_over_ceiling_scope_through_for_a_single_justified_contributor() {
    // 1000 domain lines against 500, declared by the one task that says why it
    // cannot be split.
    let exempt = gate_over(
        plan_of(
            vec![estimate("T001", &[("domain", 700, 300)], Some("one transition table"))],
            vec![batch("B1", &["T001"])],
        ),
        vec![planned_task("T001", &[])],
        Some(500),
    );
    assert_eq!(exempt, BatchPlanGateOutcome::Passed);

    // The same excess with a second contributor is not exempt.
    let shared = gate_over(
        plan_of(
            vec![
                estimate("T001", &[("domain", 700, 300)], Some("one transition table")),
                estimate("T002", &[("domain", 10, 5)], None),
            ],
            vec![batch("B1", &["T001", "T002"])],
        ),
        vec![planned_task("T001", &[]), planned_task("T002", &[])],
        Some(500),
    );
    assert_eq!(
        shared.violations().map(NonEmptyGateViolations::as_slice),
        Some(
            &[BatchPlanGateViolation::OversizeScopeHasMultipleContributors {
                batch_id: BatchId::try_new("B1").unwrap(),
                scope: scope("domain"),
                indivisible_task: task("T001"),
                other_contributors: vec![task("T002")],
            }][..]
        )
    );
}

#[test]
fn test_the_check_leaves_a_scope_with_no_resolved_ceiling_out_of_the_comparison() {
    // 5000 domain lines, and no ceiling configured for any scope.
    let outcome = gate_over(
        plan_of(
            vec![estimate("T001", &[("domain", 3_000, 2_000)], None)],
            vec![batch("B1", &["T001"])],
        ),
        vec![planned_task("T001", &[])],
        None,
    );

    assert_eq!(
        outcome,
        BatchPlanGateOutcome::Passed,
        "an unresolved ceiling cannot be exceeded, whatever the total"
    );
}

#[test]
fn test_the_check_reports_a_batch_plan_task_the_implementation_plan_does_not_have() {
    let outcome = gate_over(
        plan_of(
            vec![
                estimate("T001", &[("domain", 10, 5)], None),
                estimate("T404", &[("domain", 10, 5)], None),
            ],
            vec![batch("B1", &["T001", "T404"])],
        ),
        vec![planned_task("T001", &[])],
        Some(500),
    );

    assert_eq!(
        outcome.violations().map(NonEmptyGateViolations::as_slice),
        Some(&[BatchPlanGateViolation::UnknownTaskRef { task_id: task("T404") }][..])
    );
}

#[test]
fn test_the_check_reports_a_declared_dependency_that_sits_in_a_later_batch() {
    // T001 depends on T002, but T002 is batched after it.
    let outcome = gate_over(
        plan_of(
            vec![
                estimate("T001", &[("domain", 10, 5)], None),
                estimate("T002", &[("domain", 10, 5)], None),
            ],
            vec![batch("B1", &["T001"]), batch("B2", &["T002"])],
        ),
        vec![planned_task("T001", &["T002"]), planned_task("T002", &[])],
        Some(500),
    );

    assert_eq!(
        outcome.violations().map(NonEmptyGateViolations::as_slice),
        Some(
            &[BatchPlanGateViolation::DependencyInLaterBatch {
                task_id: task("T001"),
                task_batch: BatchId::try_new("B1").unwrap(),
                dependency: task("T002"),
                dependency_batch: BatchId::try_new("B2").unwrap(),
            }][..]
        )
    );
}

#[test]
fn test_the_check_leaves_the_batch_placement_of_undeclared_task_pairs_alone() {
    // The same placement as the previous case, with no dependency declared.
    let outcome = gate_over(
        plan_of(
            vec![
                estimate("T001", &[("domain", 10, 5)], None),
                estimate("T002", &[("domain", 10, 5)], None),
            ],
            vec![batch("B1", &["T001"]), batch("B2", &["T002"])],
        ),
        vec![planned_task("T001", &[]), planned_task("T002", &[])],
        Some(500),
    );

    assert_eq!(outcome, BatchPlanGateOutcome::Passed);
}

#[test]
fn test_the_check_judges_each_scope_of_a_batch_on_its_own() {
    // domain reaches 650 against 500; usecase stays at 60 and passes.
    let outcome = gate_over(
        plan_of(
            vec![estimate("T001", &[("domain", 500, 150), ("usecase", 40, 20)], None)],
            vec![batch("B1", &["T001"])],
        ),
        vec![planned_task("T001", &[])],
        Some(500),
    );

    let violations = outcome.violations().map(NonEmptyGateViolations::as_slice).unwrap_or(&[]);
    assert_eq!(violations.len(), 1, "only the over-budget scope is reported: {violations:?}");
    assert!(matches!(
        violations.first(),
        Some(BatchPlanGateViolation::CeilingExceeded { total, .. }) if *total == LineCount::new(650)
    ));
}

#[test]
fn test_the_check_names_an_unknown_task_once_because_a_plan_cannot_separate_the_two_sides() {
    // An estimate no batch claims cannot be assembled …
    let estimate_only = BatchPlanDocument::new(
        track(),
        vec![
            estimate("T001", &[("domain", 10, 5)], None),
            estimate("T404", &[("domain", 10, 5)], None),
        ],
        vec![batch("B1", &["T001"])],
    );
    assert!(matches!(estimate_only, Err(BatchPlanValidationError::UnassignedTask { .. })));

    // … and neither can a batch member without an estimate.
    let member_only = BatchPlanDocument::new(
        track(),
        vec![estimate("T001", &[("domain", 10, 5)], None)],
        vec![batch("B1", &["T001", "T404"])],
    );
    assert!(matches!(member_only, Err(BatchPlanValidationError::MissingTaskEstimate { .. })));

    // So a task id unknown to the implementation plan always stands on both
    // sides of a plan the check receives, and the check names it exactly once.
    let outcome = gate_over(
        plan_of(
            vec![
                estimate("T001", &[("domain", 10, 5)], None),
                estimate("T404", &[("domain", 10, 5)], None),
            ],
            vec![batch("B1", &["T001", "T404"])],
        ),
        vec![planned_task("T001", &[])],
        Some(500),
    );
    assert_eq!(
        outcome.violations().map(NonEmptyGateViolations::as_slice),
        Some(&[BatchPlanGateViolation::UnknownTaskRef { task_id: task("T404") }][..])
    );
}

#[test]
fn test_the_check_accepts_a_declared_dependency_in_the_same_batch_or_an_earlier_one() {
    // T002 depends on T001, and both sit in B1.
    let same_batch = gate_over(
        plan_of(
            vec![
                estimate("T001", &[("domain", 10, 5)], None),
                estimate("T002", &[("domain", 10, 5)], None),
            ],
            vec![batch("B1", &["T001", "T002"])],
        ),
        vec![planned_task("T001", &[]), planned_task("T002", &["T001"])],
        Some(500),
    );
    assert_eq!(same_batch, BatchPlanGateOutcome::Passed);

    // The same edge with the dependency one batch earlier.
    let earlier_batch = gate_over(
        plan_of(
            vec![
                estimate("T001", &[("domain", 10, 5)], None),
                estimate("T002", &[("domain", 10, 5)], None),
            ],
            vec![batch("B1", &["T001"]), batch("B2", &["T002"])],
        ),
        vec![planned_task("T001", &[]), planned_task("T002", &["T001"])],
        Some(500),
    );
    assert_eq!(earlier_batch, BatchPlanGateOutcome::Passed);
}

#[test]
fn test_a_plan_placing_one_task_in_two_batches_can_never_reach_the_check() {
    // The other half of "exactly one batch": such a plan cannot be assembled,
    // so no reader can hand one to the service.
    let doubly_claimed = BatchPlanDocument::new(
        track(),
        vec![
            estimate("T001", &[("domain", 10, 5)], None),
            estimate("T002", &[("domain", 10, 5)], None),
        ],
        vec![batch("B1", &["T001", "T002"]), batch("B2", &["T001"])],
    );
    let Err(BatchPlanValidationError::DuplicateBatchMembership { task_id, batch_ids }) =
        doubly_claimed
    else {
        panic!("a task claimed by two batches must be refused before any check runs");
    };
    assert_eq!(task_id, task("T001"));
    assert_eq!(batch_ids, vec![BatchId::try_new("B1").unwrap(), BatchId::try_new("B2").unwrap()]);

    // Every plan the check does receive therefore places each planned task in
    // exactly one batch, and the membership half of the gate passes.
    let well_formed = plan_of(
        vec![
            estimate("T001", &[("domain", 10, 5)], None),
            estimate("T002", &[("domain", 10, 5)], None),
        ],
        vec![batch("B1", &["T001"]), batch("B2", &["T002"])],
    );
    for member in ["T001", "T002"] {
        let claiming =
            well_formed.batches().iter().filter(|batch| batch.contains(&task(member))).count();
        assert_eq!(claiming, 1, "{member} must be claimed by exactly one batch");
    }
    assert_eq!(
        gate_over(
            well_formed,
            vec![planned_task("T001", &[]), planned_task("T002", &[])],
            Some(500)
        ),
        BatchPlanGateOutcome::Passed
    );
}

#[test]
fn test_an_absent_implementation_plan_stops_the_gate_instead_of_checking_an_empty_task_list() {
    // With no tasks to compare against, "every planned task is batched" would
    // hold vacuously; the read failure has to surface instead.
    let service = interactor(
        BatchPlanStub::Document,
        PlannedTaskStub::NotFound,
        StubScopeConfigReader::new(Some(500)),
    );

    assert!(matches!(service.check(command()), Err(BatchPlanCheckError::ImplPlanNotFound)));
}

#[test]
fn test_the_interactor_passes_the_commands_track_and_directory_to_every_port() {
    let batch_plan_reader = Arc::new(StubBatchPlanReader::new(BatchPlanStub::Document));
    let planned_task_reader =
        Arc::new(StubPlannedTaskReader::new(PlannedTaskStub::Tasks(vec![planned_task(
            "T001",
            &[],
        )])));
    let scope_config_reader = Arc::new(StubScopeConfigReader::new(Some(500)));
    let service = BatchPlanCheckInteractor::new(
        Arc::clone(&batch_plan_reader) as Arc<dyn BatchPlanReaderPort>,
        Arc::clone(&planned_task_reader) as Arc<dyn PlannedTaskReaderPort>,
        Arc::clone(&scope_config_reader) as Arc<dyn ScopeConfigReaderPort>,
    );

    service.check(command()).unwrap();

    let expected = [(items_dir(), track())];
    assert_eq!(batch_plan_reader.calls.lock().unwrap().as_slice(), &expected);
    assert_eq!(planned_task_reader.calls.lock().unwrap().as_slice(), &expected);
    assert_eq!(scope_config_reader.calls.lock().unwrap().as_slice(), &expected);
}
