//! Contract tests for the batch-plan ports and the Phase 3 gate interactor.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use domain::batch_plan::{
    BatchDeclaration, BatchId, BatchPlanDocument, BatchPlanValidationError,
    IndivisibilityJustification, LineCount, MeasuredScopeDiff, ScopeLineEstimate,
    TaskDecomposition, TaskEstimate,
};
use domain::review_v2::{MainScopeName, ReviewScopeConfig, ScopeName};
use domain::{FreeText, NonEmptyString, TaskId, TaskStatus, TrackId, TrackTask};

use super::{
    BatchPlanCheckCommand, BatchPlanCheckError, BatchPlanCheckInteractor, BatchPlanCheckOutput,
    BatchPlanCheckService, BatchPlanReaderPort, BatchPlanViolationOutput, NonEmptyViolationOutputs,
    PlanArtifactReadError, PlannedTaskReaderPort, ScopeConfigReadError, ScopeConfigReaderPort,
    ScopeDiffMeasureError, ScopeDiffMeasurePort,
};
use crate::track_resolution::{
    ActiveTrackResolveError, ActiveTrackResolveService, BranchReadError,
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
    BatchPlanCheckCommand { track_id: Some(track().as_ref().to_owned()), items_dir: items_dir() }
}

/// The same command with the track left for the interactor to resolve.
fn command_without_track() -> BatchPlanCheckCommand {
    BatchPlanCheckCommand { track_id: None, items_dir: items_dir() }
}

/// Resolves an omitted id to `self_resolved`, or fails when it is `None`.
struct StubTrackResolver {
    self_resolved: Option<String>,
}

impl StubTrackResolver {
    fn resolving_to_the_track() -> Self {
        Self { self_resolved: Some(track().as_ref().to_owned()) }
    }
}

impl ActiveTrackResolveService for StubTrackResolver {
    fn resolve_active_track(&self) -> Result<String, ActiveTrackResolveError> {
        self.self_resolved.clone().ok_or_else(|| {
            ActiveTrackResolveError::BranchRead(BranchReadError::ReadFailed(
                "not a track branch".to_owned(),
            ))
        })
    }

    fn resolve_for_read(
        &self,
        explicit_id: Option<String>,
    ) -> Result<String, ActiveTrackResolveError> {
        match explicit_id {
            Some(id) => Ok(id),
            None => self.resolve_active_track(),
        }
    }

    fn resolve_for_write(
        &self,
        explicit_id: Option<String>,
    ) -> Result<String, ActiveTrackResolveError> {
        self.resolve_for_read(explicit_id)
    }
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
    ) -> Result<BatchPlanDocument, PlanArtifactReadError> {
        self.calls.lock().unwrap().push((items_dir.to_path_buf(), track_id.clone()));
        match &self.stub {
            BatchPlanStub::Document => Ok(plan_document()),
            BatchPlanStub::NotFound => Err(PlanArtifactReadError::NotFound),
            BatchPlanStub::ReadFailed => {
                Err(PlanArtifactReadError::ReadFailed { message: FreeText::new("unreadable") })
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
    ) -> Result<Vec<TrackTask>, PlanArtifactReadError> {
        self.calls.lock().unwrap().push((items_dir.to_path_buf(), track_id.clone()));
        match &self.stub {
            PlannedTaskStub::Tasks(tasks) => Ok(tasks.clone()),
            PlannedTaskStub::NotFound => Err(PlanArtifactReadError::NotFound),
            PlannedTaskStub::ReadFailed => {
                Err(PlanArtifactReadError::ReadFailed { message: FreeText::new("unreadable") })
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
        Arc::new(StubTrackResolver::resolving_to_the_track()),
    )
}

/// Runs the gate over a plan and task list the test built itself, under
/// `ceiling` for the `domain` scope.
fn gate_over(
    plan: BatchPlanDocument,
    planned: Vec<TrackTask>,
    ceiling: Option<u32>,
) -> BatchPlanCheckOutput {
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
fn test_the_check_command_carries_the_track_context_unresolved() {
    let explicit = command();
    assert_eq!(explicit.track_id.as_deref(), Some(track().as_ref()));
    assert_eq!(explicit.items_dir, items_dir());
    assert_eq!(explicit.clone(), explicit);

    // An omitted track is carried as such, for the interactor to resolve.
    assert_eq!(command_without_track().track_id, None);
}

#[test]
fn test_the_interactor_resolves_an_omitted_track_before_reading_anything() {
    let service = interactor(
        BatchPlanStub::Document,
        PlannedTaskStub::Tasks(vec![planned_task("T001", &[])]),
        StubScopeConfigReader::new(Some(500)),
    );

    let outcome = service.check(command_without_track()).unwrap();

    assert_eq!(outcome, BatchPlanCheckOutput::Passed, "the resolved track drives the reads");
}

#[test]
fn test_a_track_that_cannot_be_resolved_is_an_error_of_its_own() {
    let service = BatchPlanCheckInteractor::new(
        Arc::new(StubBatchPlanReader::new(BatchPlanStub::Document)),
        Arc::new(StubPlannedTaskReader::new(PlannedTaskStub::Tasks(Vec::new()))),
        Arc::new(StubScopeConfigReader::new(Some(500))),
        Arc::new(StubTrackResolver { self_resolved: None }),
    );

    assert!(matches!(
        service.check(command_without_track()),
        Err(BatchPlanCheckError::TrackResolutionFailed { .. })
    ));
}

#[test]
fn test_a_resolved_value_that_is_not_a_track_id_is_refused_before_any_read() {
    let service = BatchPlanCheckInteractor::new(
        Arc::new(StubBatchPlanReader::new(BatchPlanStub::Document)),
        Arc::new(StubPlannedTaskReader::new(PlannedTaskStub::Tasks(Vec::new()))),
        Arc::new(StubScopeConfigReader::new(Some(500))),
        Arc::new(StubTrackResolver { self_resolved: Some("Not A Track".to_owned()) }),
    );

    assert!(matches!(
        service.check(command_without_track()),
        Err(BatchPlanCheckError::TrackResolutionFailed { .. })
    ));
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

    assert!(matches!(absent.read(&items_dir(), &track()), Err(PlanArtifactReadError::NotFound)));
    assert!(matches!(
        unreadable.read(&items_dir(), &track()),
        Err(PlanArtifactReadError::ReadFailed { .. })
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
        Err(PlanArtifactReadError::NotFound)
    ));
    assert!(matches!(
        unreadable.read_planned_tasks(&items_dir(), &track()),
        Err(PlanArtifactReadError::ReadFailed { .. })
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

    // The domain verdict arrives as this crate's own record, so the adapter
    // never holds a domain value.
    assert_eq!(outcome, BatchPlanCheckOutput::Passed);
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
        outcome,
        BatchPlanCheckOutput::Blocked {
            violations: NonEmptyViolationOutputs::try_new(vec![
                BatchPlanViolationOutput::UnplannedTask { task_id: "T002".to_owned() }
            ])
            .unwrap()
        }
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
fn test_a_gate_that_could_not_run_is_reported_apart_from_the_verdict_it_would_have_produced() {
    // Inputs the gate can read produce a verdict …
    let readable = interactor(
        BatchPlanStub::Document,
        PlannedTaskStub::Tasks(vec![planned_task("T001", &[])]),
        StubScopeConfigReader::new(Some(500)),
    );
    assert_eq!(readable.check(command()).unwrap(), BatchPlanCheckOutput::Passed);

    // … while a track that declares no batch plan yields no verdict at all: the
    // absence travels through the error channel, so it can never be read as a
    // plan that conforms.
    let absent = interactor(
        BatchPlanStub::NotFound,
        PlannedTaskStub::Tasks(vec![planned_task("T001", &[])]),
        StubScopeConfigReader::new(Some(500)),
    );
    let error = absent.check(command()).unwrap_err();
    assert!(matches!(error, BatchPlanCheckError::BatchPlanNotFound));
    assert_eq!(error.to_string(), "the track declares no batch plan");

    // A track that cannot be resolved stops the gate one step earlier, and is a
    // failure of its own rather than the same one: nothing was read, so there is
    // no plan to judge.
    let unresolvable = BatchPlanCheckInteractor::new(
        Arc::new(StubBatchPlanReader::new(BatchPlanStub::Document)),
        Arc::new(StubPlannedTaskReader::new(PlannedTaskStub::Tasks(vec![planned_task(
            "T001",
            &[],
        )]))),
        Arc::new(StubScopeConfigReader::new(Some(500))),
        Arc::new(StubTrackResolver { self_resolved: None }),
    );
    let error = unresolvable.check(command_without_track()).unwrap_err();
    assert!(matches!(error, BatchPlanCheckError::TrackResolutionFailed { .. }));
    assert!(
        error.to_string().starts_with("no active track could be resolved"),
        "rendered as: {error}"
    );

    // A plan that does not conform is the other side of the distinction: the
    // findings are a value the gate returned, not a failure to run it.
    let blocked = gate_over(
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
    assert!(matches!(blocked, BatchPlanCheckOutput::Blocked { .. }));
}

#[test]
fn test_a_blocked_verdict_cannot_be_assembled_with_nothing_to_report() {
    // An empty list is not a findings list: the wrapper refuses it, so the
    // domain's non-emptiness survives the crossing instead of being dropped
    // where the adapter consumes the verdict.
    assert_eq!(NonEmptyViolationOutputs::try_new(Vec::new()), None);

    // One finding is enough, and the findings come back in the order they were
    // given, through either accessor.
    let violations = NonEmptyViolationOutputs::try_new(vec![
        BatchPlanViolationOutput::UnplannedTask { task_id: "T002".to_owned() },
        BatchPlanViolationOutput::UnknownTaskRef { task_id: "T404".to_owned() },
    ])
    .unwrap();
    assert_eq!(
        violations.as_slice(),
        &[
            BatchPlanViolationOutput::UnplannedTask { task_id: "T002".to_owned() },
            BatchPlanViolationOutput::UnknownTaskRef { task_id: "T404".to_owned() },
        ][..]
    );
    assert_eq!(violations.into_vec().len(), 2);

    // A blocked verdict the gate produced carries that wrapper rather than a
    // bare list: T002 is planned but the batch plan places only T001.
    let outcome = interactor(
        BatchPlanStub::Document,
        PlannedTaskStub::Tasks(vec![planned_task("T001", &[]), planned_task("T002", &[])]),
        StubScopeConfigReader::new(Some(500)),
    )
    .check(command())
    .unwrap();
    assert_eq!(
        outcome,
        BatchPlanCheckOutput::Blocked {
            violations: NonEmptyViolationOutputs::try_new(vec![
                BatchPlanViolationOutput::UnplannedTask { task_id: "T002".to_owned() }
            ])
            .unwrap()
        }
    );
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
        outcome,
        BatchPlanCheckOutput::Blocked {
            violations: NonEmptyViolationOutputs::try_new(vec![
                BatchPlanViolationOutput::CeilingExceeded {
                    batch_id: "B1".to_owned(),
                    scope: "domain".to_owned(),
                    total: 650,
                    ceiling: 500,
                }
            ])
            .unwrap()
        }
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
    assert_eq!(exempt, BatchPlanCheckOutput::Passed);

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
        shared,
        BatchPlanCheckOutput::Blocked {
            violations: NonEmptyViolationOutputs::try_new(vec![
                BatchPlanViolationOutput::OversizeScopeHasMultipleContributors {
                    batch_id: "B1".to_owned(),
                    scope: "domain".to_owned(),
                    indivisible_task: "T001".to_owned(),
                    other_contributors: vec!["T002".to_owned()],
                }
            ])
            .unwrap()
        }
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
        BatchPlanCheckOutput::Passed,
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
        outcome,
        BatchPlanCheckOutput::Blocked {
            violations: NonEmptyViolationOutputs::try_new(vec![
                BatchPlanViolationOutput::UnknownTaskRef { task_id: "T404".to_owned() }
            ])
            .unwrap()
        }
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
        outcome,
        BatchPlanCheckOutput::Blocked {
            violations: NonEmptyViolationOutputs::try_new(vec![
                BatchPlanViolationOutput::DependencyInLaterBatch {
                    task_id: "T001".to_owned(),
                    task_batch: "B1".to_owned(),
                    dependency: "T002".to_owned(),
                    dependency_batch: "B2".to_owned(),
                }
            ])
            .unwrap()
        }
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

    assert_eq!(outcome, BatchPlanCheckOutput::Passed);
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

    let BatchPlanCheckOutput::Blocked { violations } = outcome else {
        panic!("the over-budget scope must be reported");
    };
    let violations = violations.as_slice();
    assert_eq!(violations.len(), 1, "only the over-budget scope is reported: {violations:?}");
    assert!(matches!(
        violations.first(),
        Some(BatchPlanViolationOutput::CeilingExceeded { total, .. }) if *total == 650
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
        outcome,
        BatchPlanCheckOutput::Blocked {
            violations: NonEmptyViolationOutputs::try_new(vec![
                BatchPlanViolationOutput::UnknownTaskRef { task_id: "T404".to_owned() }
            ])
            .unwrap()
        }
    );
}

#[test]
fn test_a_batch_member_no_task_provides_is_reported_whichever_batch_claims_it() {
    // A batch member without an estimate is refused before any check runs, so a
    // membership id always carries an estimate: the two surfaces hold the same
    // ids and the check reaches a member through either of them.
    let member_without_estimate = BatchPlanDocument::new(
        track(),
        vec![estimate("T001", &[("domain", 10, 5)], None)],
        vec![batch("B1", &["T001"]), batch("B2", &["T404"])],
    );
    assert!(matches!(
        member_without_estimate,
        Err(BatchPlanValidationError::MissingTaskEstimate { .. })
    ));

    // With every estimate well formed, an id only the second batch claims is
    // still named: membership is read across all batches, not just the first.
    let outcome = gate_over(
        plan_of(
            vec![
                estimate("T001", &[("domain", 10, 5)], None),
                estimate("T404", &[("domain", 10, 5)], None),
            ],
            vec![batch("B1", &["T001"]), batch("B2", &["T404"])],
        ),
        vec![planned_task("T001", &[])],
        Some(500),
    );
    assert_eq!(
        outcome,
        BatchPlanCheckOutput::Blocked {
            violations: NonEmptyViolationOutputs::try_new(vec![
                BatchPlanViolationOutput::UnknownTaskRef { task_id: "T404".to_owned() }
            ])
            .unwrap()
        }
    );

    // The same two batches with both members planned pass, so the finding
    // follows the ids a task list does not provide rather than the batch a
    // member sits in.
    let both_planned = gate_over(
        plan_of(
            vec![
                estimate("T001", &[("domain", 10, 5)], None),
                estimate("T404", &[("domain", 10, 5)], None),
            ],
            vec![batch("B1", &["T001"]), batch("B2", &["T404"])],
        ),
        vec![planned_task("T001", &[]), planned_task("T404", &[])],
        Some(500),
    );
    assert_eq!(both_planned, BatchPlanCheckOutput::Passed);
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
    assert_eq!(same_batch, BatchPlanCheckOutput::Passed);

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
    assert_eq!(earlier_batch, BatchPlanCheckOutput::Passed);
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
        BatchPlanCheckOutput::Passed
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
        Arc::new(StubTrackResolver::resolving_to_the_track()),
    );

    service.check(command()).unwrap();

    let expected = [(items_dir(), track())];
    assert_eq!(batch_plan_reader.calls.lock().unwrap().as_slice(), &expected);
    assert_eq!(planned_task_reader.calls.lock().unwrap().as_slice(), &expected);
    assert_eq!(scope_config_reader.calls.lock().unwrap().as_slice(), &expected);
}
