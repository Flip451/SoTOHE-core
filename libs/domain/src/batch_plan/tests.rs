//! Unit tests for the declared batch-plan value objects.

use std::collections::BTreeSet;

use super::*;
use crate::review_v2::{MainScopeName, ReviewScopeConfig, ScopeName};
use crate::{CommitHash, NonEmptyString, TaskId, TaskStatus, TrackId, TrackTask};

fn scope(name: &str) -> ScopeName {
    ScopeName::Main(MainScopeName::new(name).unwrap())
}

fn main_scope(name: &str) -> MainScopeName {
    MainScopeName::new(name).unwrap()
}

/// An estimate for the reserved implicit scope, which no configuration declares
/// and no ceiling ever constrains.
fn other_scope_estimate(production: u32, test: u32) -> ScopeLineEstimate {
    ScopeLineEstimate::new(ScopeName::Other, LineCount::new(production), LineCount::new(test))
}

fn task(id: &str) -> TaskId {
    TaskId::try_new(id).unwrap()
}

fn batch_id(value: &str) -> BatchId {
    BatchId::try_new(value).unwrap()
}

fn justification(text: &str) -> IndivisibilityJustification {
    IndivisibilityJustification::try_new(text).unwrap()
}

fn track() -> TrackId {
    TrackId::try_new("scope-diff-ceiling-admission-enforcement-2026-07-29").unwrap()
}

fn scope_estimate(name: &str, production: u32, test: u32) -> ScopeLineEstimate {
    ScopeLineEstimate::new(scope(name), LineCount::new(production), LineCount::new(test))
}

fn estimate(
    id: &str,
    scope_estimates: Vec<ScopeLineEstimate>,
    decomposition: TaskDecomposition,
) -> TaskEstimate {
    TaskEstimate::new(task(id), scope_estimates, decomposition).unwrap()
}

fn declaration(id: &str, members: &[&str]) -> BatchDeclaration {
    BatchDeclaration::new(batch_id(id), members.iter().map(|member| task(member)).collect())
        .unwrap()
}

fn committed(task_ids: &[&str]) -> BTreeSet<TaskId> {
    task_ids.iter().map(|id| task(id)).collect()
}

/// A scope configuration declaring `entries` as `(scope name, per-scope
/// ceiling)`, with no global default ceiling.
fn scope_config(entries: &[(&str, Option<u32>)]) -> ReviewScopeConfig {
    ReviewScopeConfig::new(
        &track(),
        entries
            .iter()
            .map(|(name, ceiling)| {
                ((*name).to_owned(), vec![format!("libs/{name}/**")], None, *ceiling)
            })
            .collect(),
        Vec::new(),
        Vec::new(),
        None,
    )
    .unwrap()
}

fn planned_plan(estimates: Vec<TaskEstimate>, batches: Vec<BatchDeclaration>) -> BatchPlanDocument {
    BatchPlanDocument::new(track(), estimates, batches).unwrap()
}

/// An implementation-plan task declaring `depends_on`.
fn planned(id: &str, depends_on: &[&str]) -> TrackTask {
    TrackTask::with_dependencies(
        task(id),
        NonEmptyString::try_new(format!("task {id}")).unwrap(),
        TaskStatus::Todo,
        depends_on.iter().map(|dependency| task(dependency)).collect(),
    )
}

/// An implementation-plan task in `status`, declaring no dependency.
fn planned_in(id: &str, status: TaskStatus) -> TrackTask {
    TrackTask::with_dependencies(
        task(id),
        NonEmptyString::try_new(format!("task {id}")).unwrap(),
        status,
        Vec::new(),
    )
}

/// A task settled the committed way: done with a commit recorded.
fn settled_by_commit(id: &str) -> TrackTask {
    planned_in(
        id,
        TaskStatus::DoneTraced {
            commit_hash: CommitHash::try_new("a1b2c3d4e5f60718293a4b5c6d7e8f9012345678").unwrap(),
        },
    )
}

fn violations_of(outcome: &BatchPlanGateOutcome) -> &[BatchPlanGateViolation] {
    outcome.violations().map_or(&[], NonEmptyGateViolations::as_slice)
}

/// The same set shape as [`committed`], named for the in-progress argument.
fn in_progress(task_ids: &[&str]) -> BTreeSet<TaskId> {
    committed(task_ids)
}

fn measured(entries: &[(&str, u32)]) -> Vec<MeasuredScopeDiff> {
    entries
        .iter()
        .map(|(name, lines)| MeasuredScopeDiff::new(scope(name), LineCount::new(*lines)))
        .collect()
}

/// B1 = T001 (domain 150) + T002 (domain 300); B2 = T003 (usecase 75).
fn admission_plan() -> BatchPlanDocument {
    planned_plan(
        vec![
            estimate(
                "T001",
                vec![scope_estimate("domain", 100, 50)],
                TaskDecomposition::Decomposable,
            ),
            estimate(
                "T002",
                vec![scope_estimate("domain", 200, 100)],
                TaskDecomposition::Decomposable,
            ),
            estimate(
                "T003",
                vec![scope_estimate("usecase", 50, 25)],
                TaskDecomposition::Decomposable,
            ),
        ],
        vec![declaration("B1", &["T001", "T002"]), declaration("B2", &["T003"])],
    )
}

fn admission_config() -> ReviewScopeConfig {
    scope_config(&[("domain", Some(500)), ("usecase", Some(500))])
}

// ── LineCount ─────────────────────────────────────────────────────────────────

#[test]
fn test_line_count_carries_the_declared_figure() {
    assert_eq!(LineCount::new(240).value(), 240);
    assert_eq!(LineCount::new(0).value(), 0);
}

#[test]
fn test_line_count_sums_a_production_and_a_test_figure() {
    let production = LineCount::new(180);
    let test = LineCount::new(120);

    assert_eq!(production.saturating_add(&test), LineCount::new(300));
}

#[test]
fn test_line_count_saturates_instead_of_wrapping_at_the_maximum() {
    let huge = LineCount::new(u32::MAX);

    assert_eq!(huge.saturating_add(&LineCount::new(1)).value(), u32::MAX);
}

#[test]
fn test_largest_ceiling_total_above_u32_max_refused() {
    let above_maximum = LineCount::new(u32::MAX).saturating_add(&LineCount::new(1));

    assert!(!ScopeCeiling::resolve(Some(u32::MAX)).admits(&above_maximum));
}

#[test]
fn test_line_counts_order_by_magnitude() {
    assert!(LineCount::new(299) < LineCount::new(300));
    assert!(LineCount::new(300) <= LineCount::new(300));
    assert!(LineCount::new(301) > LineCount::new(300));
}

// ── ScopeCeiling ──────────────────────────────────────────────────────────────

#[test]
fn test_scope_ceiling_resolves_a_configured_limit_into_a_line_count() {
    assert_eq!(ScopeCeiling::resolve(Some(500)), ScopeCeiling::Limited(LineCount::new(500)));
    assert_eq!(ScopeCeiling::resolve(Some(500)).limit(), Some(LineCount::new(500)));
}

#[test]
fn test_scope_ceiling_resolves_an_absent_configuration_as_unconstrained() {
    assert_eq!(ScopeCeiling::resolve(None), ScopeCeiling::Unconstrained);
    assert_eq!(ScopeCeiling::resolve(None).limit(), None);
}

#[test]
fn test_limited_scope_ceiling_admits_a_total_at_the_limit_and_refuses_one_above_it() {
    let ceiling = ScopeCeiling::resolve(Some(500));

    assert!(ceiling.admits(&LineCount::new(499)));
    assert!(ceiling.admits(&LineCount::new(500)));
    assert!(!ceiling.admits(&LineCount::new(501)));
}

#[test]
fn test_unconstrained_scope_ceiling_admits_every_total() {
    let ceiling = ScopeCeiling::resolve(None);

    assert!(ceiling.admits(&LineCount::new(0)));
    assert!(ceiling.admits(&LineCount::new(50_000)));
    assert!(ceiling.admits(&LineCount::new(u32::MAX)));
}

// ── MeasuredScopeDiff ─────────────────────────────────────────────────────────

#[test]
fn test_measured_scope_diff_carries_the_measured_figure_for_its_scope() {
    let measured = MeasuredScopeDiff::new(scope("domain"), LineCount::new(412));

    assert_eq!(measured.scope(), &scope("domain"));
    assert_eq!(measured.lines(), LineCount::new(412));
}

// ── IndivisibilityJustification ───────────────────────────────────────────────

#[test]
fn test_indivisibility_justification_carries_the_stated_reason() {
    let reason = IndivisibilityJustification::try_new(
        "the enum and its exhaustive match arms cannot compile apart",
    )
    .unwrap();

    assert_eq!(reason.as_str(), "the enum and its exhaustive match arms cannot compile apart");
}

#[test]
fn test_indivisibility_justification_rejects_a_blank_reason_instead_of_dropping_it() {
    assert!(matches!(
        IndivisibilityJustification::try_new(""),
        Err(BatchPlanValidationError::EmptyJustification)
    ));
    assert!(matches!(
        IndivisibilityJustification::try_new("   \n  "),
        Err(BatchPlanValidationError::EmptyJustification)
    ));
}

// ── TaskDecomposition ─────────────────────────────────────────────────────────

#[test]
fn test_indivisible_decomposition_reports_the_justification_it_carries() {
    let decomposition = TaskDecomposition::Indivisible(justification("one atomic rename"));

    assert!(decomposition.is_indivisible());
    assert_eq!(decomposition.justification().map(IndivisibilityJustification::as_str), {
        Some("one atomic rename")
    });
}

#[test]
fn test_decomposable_decomposition_reports_no_justification() {
    let decomposition = TaskDecomposition::Decomposable;

    assert!(!decomposition.is_indivisible());
    assert!(decomposition.justification().is_none());
}

// ── ScopeLineEstimate ─────────────────────────────────────────────────────────

#[test]
fn test_scope_line_estimate_keeps_production_and_test_figures_separate() {
    let estimate =
        ScopeLineEstimate::new(scope("domain"), LineCount::new(180), LineCount::new(120));

    assert_eq!(estimate.scope(), &scope("domain"));
    assert_eq!(estimate.production_lines(), LineCount::new(180));
    assert_eq!(estimate.test_lines(), LineCount::new(120));
}

#[test]
fn test_scope_line_estimate_totals_production_and_test_lines() {
    let estimate =
        ScopeLineEstimate::new(scope("domain"), LineCount::new(180), LineCount::new(120));

    assert_eq!(estimate.total(), LineCount::new(300));
}

#[test]
fn test_scope_line_estimate_declares_test_lines_as_one_total_including_obligation_derived_tests() {
    let ordinary_test_lines = LineCount::new(90);
    let obligation_derived_test_lines = LineCount::new(30);
    let declared_test_lines = ordinary_test_lines.saturating_add(&obligation_derived_test_lines);

    let estimate =
        ScopeLineEstimate::new(scope("domain"), LineCount::new(180), declared_test_lines);

    // The test figure is one summed quantity covering both kinds of test code,
    // and it stays separate from the production figure.
    assert_eq!(estimate.test_lines(), LineCount::new(120));
    assert_eq!(estimate.production_lines(), LineCount::new(180));
    assert_eq!(estimate.total(), LineCount::new(300));
}

// ── TaskEstimate ──────────────────────────────────────────────────────────────

#[test]
fn test_task_estimate_declares_the_production_and_test_figures_of_each_touched_scope() {
    let estimate = estimate(
        "T001",
        vec![scope_estimate("domain", 180, 120), scope_estimate("usecase", 60, 40)],
        TaskDecomposition::Decomposable,
    );

    let domain = estimate.estimate_for(&scope("domain")).unwrap();
    assert_eq!(domain.production_lines(), LineCount::new(180));
    assert_eq!(domain.test_lines(), LineCount::new(120));

    let usecase = estimate.estimate_for(&scope("usecase")).unwrap();
    assert_eq!(usecase.production_lines(), LineCount::new(60));
    assert_eq!(usecase.test_lines(), LineCount::new(40));
}

#[test]
fn test_an_estimate_above_the_resolved_ceiling_states_why_its_task_is_indivisible() {
    let ceiling = ScopeCeiling::resolve(Some(500));
    let oversize = estimate(
        "T002",
        vec![scope_estimate("domain", 700, 300)],
        TaskDecomposition::Indivisible(justification("the transition table cannot be split")),
    );
    let ordinary =
        estimate("T003", vec![scope_estimate("domain", 80, 40)], TaskDecomposition::Decomposable);

    let oversize_total = oversize.estimate_for(&scope("domain")).unwrap().total();
    assert_eq!(oversize_total, LineCount::new(1000));
    assert!(!ceiling.admits(&oversize_total));
    assert!(oversize.decomposition().is_indivisible());
    assert_eq!(
        oversize.decomposition().justification().map(IndivisibilityJustification::as_str),
        Some("the transition table cannot be split")
    );

    let ordinary_total = ordinary.estimate_for(&scope("domain")).unwrap().total();
    assert_eq!(ordinary_total, LineCount::new(120));
    assert!(ceiling.admits(&ordinary_total));
    assert!(!ordinary.decomposition().is_indivisible());
    assert!(ordinary.decomposition().justification().is_none());
}

#[test]
fn test_task_estimate_holds_one_estimate_per_touched_scope() {
    let estimate = TaskEstimate::new(
        task("T001"),
        vec![
            ScopeLineEstimate::new(scope("domain"), LineCount::new(180), LineCount::new(120)),
            ScopeLineEstimate::new(scope("usecase"), LineCount::new(60), LineCount::new(40)),
        ],
        TaskDecomposition::Decomposable,
    )
    .unwrap();

    assert_eq!(estimate.task_id(), &task("T001"));
    assert_eq!(estimate.scope_estimates().len(), 2);
    assert_eq!(estimate.scope_estimates()[0].scope(), &scope("domain"));
    assert_eq!(estimate.scope_estimates()[1].scope(), &scope("usecase"));
}

#[test]
fn test_task_estimate_finds_the_estimate_declared_for_a_named_scope() {
    let estimate = TaskEstimate::new(
        task("T001"),
        vec![ScopeLineEstimate::new(scope("domain"), LineCount::new(180), LineCount::new(120))],
        TaskDecomposition::Decomposable,
    )
    .unwrap();

    assert_eq!(estimate.estimate_for(&scope("domain")).map(ScopeLineEstimate::total), {
        Some(LineCount::new(300))
    });
    assert!(estimate.estimate_for(&scope("infrastructure")).is_none());
}

#[test]
fn test_task_estimate_rejects_two_figures_for_the_same_scope() {
    let result = TaskEstimate::new(
        task("T001"),
        vec![
            ScopeLineEstimate::new(scope("domain"), LineCount::new(180), LineCount::new(120)),
            ScopeLineEstimate::new(scope("domain"), LineCount::new(10), LineCount::new(5)),
        ],
        TaskDecomposition::Decomposable,
    );

    let Err(BatchPlanValidationError::DuplicateScopeEstimate { task_id, scope: duplicated }) =
        result
    else {
        panic!("a repeated scope must be rejected");
    };
    assert_eq!(task_id, task("T001"));
    assert_eq!(duplicated, scope("domain"));
}

#[test]
fn test_task_estimate_carries_the_decomposition_state_of_its_task() {
    let indivisible = TaskEstimate::new(
        task("T002"),
        vec![ScopeLineEstimate::new(scope("domain"), LineCount::new(700), LineCount::new(300))],
        TaskDecomposition::Indivisible(justification("the state machine has one transition table")),
    )
    .unwrap();
    let decomposable = TaskEstimate::new(
        task("T003"),
        vec![ScopeLineEstimate::new(scope("domain"), LineCount::new(80), LineCount::new(40))],
        TaskDecomposition::Decomposable,
    )
    .unwrap();

    assert!(indivisible.decomposition().is_indivisible());
    assert_eq!(
        indivisible.decomposition().justification().map(IndivisibilityJustification::as_str),
        Some("the state machine has one transition table")
    );
    assert!(!decomposable.decomposition().is_indivisible());
    assert!(decomposable.decomposition().justification().is_none());
}

// ── BatchId ───────────────────────────────────────────────────────────────────

#[test]
fn test_batch_id_carries_the_declared_identifier() {
    assert_eq!(BatchId::try_new("B1").unwrap().as_str(), "B1");
}

#[test]
fn test_batch_id_rejects_a_blank_identifier() {
    assert!(matches!(BatchId::try_new(""), Err(BatchPlanValidationError::EmptyBatchId)));
    assert!(matches!(BatchId::try_new("  \t "), Err(BatchPlanValidationError::EmptyBatchId)));
}

#[test]
fn test_batches_with_different_identifiers_are_distinguishable() {
    assert_ne!(batch_id("B1"), batch_id("B2"));
    assert_eq!(batch_id("B1"), batch_id("B1"));
}

// ── BatchDeclaration ──────────────────────────────────────────────────────────

#[test]
fn test_batch_declaration_preserves_the_declared_member_order() {
    let declaration =
        BatchDeclaration::new(batch_id("B1"), vec![task("T003"), task("T001"), task("T002")])
            .unwrap();

    assert_eq!(declaration.id(), &batch_id("B1"));
    assert_eq!(declaration.task_ids(), &[task("T003"), task("T001"), task("T002")]);
}

#[test]
fn test_batch_declaration_reports_whether_a_task_is_one_of_its_members() {
    let declaration =
        BatchDeclaration::new(batch_id("B1"), vec![task("T001"), task("T002")]).unwrap();

    assert!(declaration.contains(&task("T001")));
    assert!(declaration.contains(&task("T002")));
    assert!(!declaration.contains(&task("T003")));
}

#[test]
fn test_batch_declaration_with_no_members_is_rejected_rather_than_skipped() {
    let result = BatchDeclaration::new(batch_id("B1"), Vec::new());

    let Err(BatchPlanValidationError::EmptyBatch { batch_id: offending }) = result else {
        panic!("a batch with no member must be rejected");
    };
    assert_eq!(offending, batch_id("B1"));
}

// ── BatchPlanValidationError ──────────────────────────────────────────────────

#[test]
fn test_missing_estimate_rejection_names_the_task_without_an_estimate() {
    let rejection = BatchPlanValidationError::MissingTaskEstimate { task_id: task("T007") };

    assert!(rejection.to_string().contains("T007"), "rendered as: {rejection}");
}

#[test]
fn test_unassigned_task_rejection_names_the_task_that_belongs_to_no_batch() {
    let rejection = BatchPlanValidationError::UnassignedTask { task_id: task("T009") };

    assert!(rejection.to_string().contains("T009"), "rendered as: {rejection}");
}

#[test]
fn test_duplicate_membership_rejection_names_every_batch_claiming_the_task() {
    let rejection = BatchPlanValidationError::DuplicateBatchMembership {
        task_id: task("T004"),
        batch_ids: vec![batch_id("B1"), batch_id("B3")],
    };

    let rendered = rejection.to_string();
    assert!(rendered.contains("T004"), "rendered as: {rendered}");
    assert!(rendered.contains("B1"), "rendered as: {rendered}");
    assert!(rendered.contains("B3"), "rendered as: {rendered}");
}

// ── BatchPlanDocument ─────────────────────────────────────────────────────────

/// A well-formed two-batch plan: `B1` holds `T001` (domain + usecase) and
/// `T002` (domain), `B2` holds `T003` (usecase).
fn valid_plan() -> BatchPlanDocument {
    BatchPlanDocument::new(
        track(),
        vec![
            estimate(
                "T001",
                vec![scope_estimate("domain", 180, 120), scope_estimate("usecase", 10, 5)],
                TaskDecomposition::Decomposable,
            ),
            estimate(
                "T002",
                vec![scope_estimate("domain", 60, 40)],
                TaskDecomposition::Decomposable,
            ),
            estimate(
                "T003",
                vec![scope_estimate("usecase", 30, 20)],
                TaskDecomposition::Decomposable,
            ),
        ],
        vec![declaration("B1", &["T001", "T002"]), declaration("B2", &["T003"])],
    )
    .unwrap()
}

#[test]
fn test_the_plan_carries_the_track_id_estimates_and_batches_it_was_declared_with() {
    let plan = valid_plan();

    assert_eq!(plan.track_id(), &track());
    assert_eq!(
        plan.task_estimates().iter().map(|e| e.task_id().as_ref()).collect::<Vec<_>>(),
        vec!["T001", "T002", "T003"]
    );
    assert_eq!(
        plan.batches().iter().map(|batch| batch.id().as_str()).collect::<Vec<_>>(),
        vec!["B1", "B2"]
    );
    assert_eq!(
        plan.estimate_for(&task("T002"))
            .and_then(|e| e.estimate_for(&scope("domain")))
            .map(ScopeLineEstimate::total),
        Some(LineCount::new(100))
    );
    assert!(plan.estimate_for(&task("T009")).is_none());
}

#[test]
fn test_the_plan_keeps_its_batches_in_declaration_order_under_their_identifiers() {
    let plan = valid_plan();

    let first = plan.batches().first().unwrap();
    let second = plan.batches().get(1).unwrap();
    assert_eq!(first.id().as_str(), "B1");
    assert_eq!(first.task_ids(), &[task("T001"), task("T002")]);
    assert_eq!(second.id().as_str(), "B2");
    assert_eq!(second.task_ids(), &[task("T003")]);
}

#[test]
fn test_every_planned_task_belongs_to_exactly_one_declared_batch() {
    let plan = valid_plan();

    for (member, expected) in [("T001", "B1"), ("T002", "B1"), ("T003", "B2")] {
        let claiming: Vec<&str> = plan
            .batches()
            .iter()
            .filter(|batch| batch.contains(&task(member)))
            .map(|batch| batch.id().as_str())
            .collect();
        assert_eq!(claiming, vec![expected], "{member} must be claimed by exactly one batch");
        assert_eq!(plan.batch_of(&task(member)).map(|batch| batch.id().as_str()), Some(expected));
    }
    assert!(plan.batch_of(&task("T009")).is_none());
}

#[test]
fn test_the_plan_rejects_a_batch_member_without_a_declared_estimate() {
    let result = BatchPlanDocument::new(
        track(),
        vec![estimate(
            "T001",
            vec![scope_estimate("domain", 180, 120)],
            TaskDecomposition::Decomposable,
        )],
        vec![declaration("B1", &["T001", "T002"])],
    );

    let Err(BatchPlanValidationError::MissingTaskEstimate { task_id }) = result else {
        panic!("a batch member without an estimate must be rejected");
    };
    assert_eq!(task_id, task("T002"));
}

#[test]
fn test_the_plan_rejects_an_estimated_task_that_no_batch_claims() {
    let result = BatchPlanDocument::new(
        track(),
        vec![
            estimate(
                "T001",
                vec![scope_estimate("domain", 180, 120)],
                TaskDecomposition::Decomposable,
            ),
            estimate(
                "T002",
                vec![scope_estimate("domain", 60, 40)],
                TaskDecomposition::Decomposable,
            ),
        ],
        vec![declaration("B1", &["T001"])],
    );

    let Err(BatchPlanValidationError::UnassignedTask { task_id }) = result else {
        panic!("an estimated task in no batch must be rejected");
    };
    assert_eq!(task_id, task("T002"));
}

#[test]
fn test_the_plan_rejects_a_task_claimed_by_more_than_one_batch() {
    let result = BatchPlanDocument::new(
        track(),
        vec![estimate(
            "T001",
            vec![scope_estimate("domain", 180, 120)],
            TaskDecomposition::Decomposable,
        )],
        vec![declaration("B1", &["T001"]), declaration("B2", &["T001"])],
    );

    let Err(BatchPlanValidationError::DuplicateBatchMembership { task_id, batch_ids }) = result
    else {
        panic!("a task claimed by two batches must be rejected");
    };
    assert_eq!(task_id, task("T001"));
    assert_eq!(batch_ids, vec![batch_id("B1"), batch_id("B2")]);
}

#[test]
fn test_a_declared_settled_task_is_not_exempt_from_the_file_internal_invariants() {
    // Declaring a settled task is optional, not exempting: these invariants judge
    // the declaration against itself, so they apply to every id the plan names
    // whatever state the task carries in the implementation plan.
    let settled = settled_by_commit("T001");
    assert!(matches!(settled.status(), TaskStatus::DoneTraced { .. }), "the fixture is settled");

    let member_without_estimate = BatchPlanDocument::new(
        track(),
        vec![estimate(
            "T002",
            vec![scope_estimate("domain", 10, 5)],
            TaskDecomposition::Decomposable,
        )],
        vec![declaration("B1", &[settled.id().as_ref(), "T002"])],
    );
    let Err(BatchPlanValidationError::MissingTaskEstimate { task_id }) = member_without_estimate
    else {
        panic!("a declared member still needs an estimate, settled or not");
    };
    assert_eq!(task_id, task("T001"));

    let claimed_twice = BatchPlanDocument::new(
        track(),
        vec![estimate(
            "T001",
            vec![scope_estimate("domain", 10, 5)],
            TaskDecomposition::Decomposable,
        )],
        vec![declaration("B1", &["T001"]), declaration("B2", &["T001"])],
    );
    assert!(matches!(
        claimed_twice,
        Err(BatchPlanValidationError::DuplicateBatchMembership { .. })
    ));
}

#[test]
fn test_the_plan_rejects_a_task_estimated_more_than_once() {
    let result = BatchPlanDocument::new(
        track(),
        vec![
            estimate(
                "T001",
                vec![scope_estimate("domain", 180, 120)],
                TaskDecomposition::Decomposable,
            ),
            estimate(
                "T001",
                vec![scope_estimate("domain", 10, 5)],
                TaskDecomposition::Decomposable,
            ),
        ],
        vec![declaration("B1", &["T001"])],
    );

    let Err(BatchPlanValidationError::DuplicateTaskEstimate { task_id }) = result else {
        panic!("a task estimated twice must be rejected");
    };
    assert_eq!(task_id, task("T001"));
}

#[test]
fn test_the_plan_rejects_a_batch_identifier_declared_more_than_once() {
    let result = BatchPlanDocument::new(
        track(),
        vec![
            estimate(
                "T001",
                vec![scope_estimate("domain", 180, 120)],
                TaskDecomposition::Decomposable,
            ),
            estimate(
                "T002",
                vec![scope_estimate("domain", 60, 40)],
                TaskDecomposition::Decomposable,
            ),
        ],
        vec![declaration("B1", &["T001"]), declaration("B1", &["T002"])],
    );

    let Err(BatchPlanValidationError::DuplicateBatchId { batch_id: offending }) = result else {
        panic!("a repeated batch identifier must be rejected");
    };
    assert_eq!(offending, batch_id("B1"));
}

#[test]
fn test_the_plan_derives_a_batch_scope_total_from_its_member_estimates() {
    let plan = valid_plan();
    let first_batch = plan.batches().first().unwrap();

    // B1 = T001 (domain 180 + 120) + T002 (domain 60 + 40).
    assert_eq!(plan.scope_total(first_batch, &scope("domain")), LineCount::new(400));
    // Only T001 touches usecase in B1.
    assert_eq!(plan.scope_total(first_batch, &scope("usecase")), LineCount::new(15));
}

#[test]
fn test_the_plan_reports_no_lines_for_a_scope_no_member_touches() {
    let plan = valid_plan();
    let first_batch = plan.batches().first().unwrap();

    assert_eq!(plan.scope_total(first_batch, &scope("infrastructure")), LineCount::new(0));
}

#[test]
fn test_the_current_batch_is_the_earliest_batch_with_an_uncommitted_member() {
    let plan = valid_plan();

    assert_eq!(
        plan.current_batch(&committed(&[])).map(|batch| batch.id().as_str()),
        Some("B1"),
        "nothing committed yet"
    );
    assert_eq!(
        plan.current_batch(&committed(&["T001"])).map(|batch| batch.id().as_str()),
        Some("B1"),
        "B1 still has an uncommitted member"
    );
    assert_eq!(
        plan.current_batch(&committed(&["T001", "T002"])).map(|batch| batch.id().as_str()),
        Some("B2"),
        "B1 is fully committed, so the next batch opens"
    );
}

#[test]
fn test_the_plan_has_no_current_batch_once_every_member_is_committed() {
    let plan = valid_plan();

    assert!(plan.current_batch(&committed(&["T001", "T002", "T003"])).is_none());
}

#[test]
fn test_the_plan_tells_an_oversize_indivisible_task_apart_from_an_ordinary_one() {
    let ceiling = ScopeCeiling::resolve(Some(500));
    let plan = BatchPlanDocument::new(
        track(),
        vec![
            estimate(
                "T002",
                vec![scope_estimate("domain", 700, 300)],
                TaskDecomposition::Indivisible(justification(
                    "the transition table cannot be split",
                )),
            ),
            estimate(
                "T003",
                vec![scope_estimate("domain", 80, 40)],
                TaskDecomposition::Decomposable,
            ),
        ],
        vec![declaration("B1", &["T002"]), declaration("B2", &["T003"])],
    )
    .unwrap();

    // Read straight off the plan: which tasks exceed the resolved ceiling, and
    // which of them state why they cannot be split.
    let oversize: Vec<&str> = plan
        .task_estimates()
        .iter()
        .filter(|estimate| {
            estimate
                .estimate_for(&scope("domain"))
                .is_some_and(|scope_estimate| !ceiling.admits(&scope_estimate.total()))
        })
        .map(|estimate| estimate.task_id().as_ref())
        .collect();
    assert_eq!(oversize, vec!["T002"]);

    let justified: Vec<&str> = plan
        .task_estimates()
        .iter()
        .filter(|estimate| estimate.decomposition().is_indivisible())
        .map(|estimate| estimate.task_id().as_ref())
        .collect();
    assert_eq!(justified, oversize);
    assert!(plan.estimate_for(&task("T003")).unwrap().decomposition().justification().is_none());
}

// ── BatchPlanGateOutcome / NonEmptyGateViolations ─────────────────────────────

#[test]
fn test_a_gate_run_with_no_findings_is_a_passed_verdict() {
    let outcome = BatchPlanGateOutcome::from_violations(Vec::new());

    assert_eq!(outcome, BatchPlanGateOutcome::Passed);
    assert!(outcome.violations().is_none());
}

#[test]
fn test_a_gate_run_with_findings_is_a_blocked_verdict_carrying_them() {
    let finding = BatchPlanGateViolation::UnplannedTask { task_id: task("T009") };

    let outcome = BatchPlanGateOutcome::from_violations(vec![finding.clone()]);

    assert_eq!(violations_of(&outcome), &[finding]);
}

#[test]
fn test_an_empty_violation_list_cannot_become_a_blocked_verdict() {
    assert!(NonEmptyGateViolations::try_new(Vec::new()).is_none());

    let findings = vec![
        BatchPlanGateViolation::UnplannedTask { task_id: task("T009") },
        BatchPlanGateViolation::UnknownTaskRef { task_id: task("T404") },
    ];
    let wrapped = NonEmptyGateViolations::try_new(findings.clone()).unwrap();
    assert_eq!(wrapped.as_slice(), findings.as_slice());
    assert_eq!(wrapped.into_vec(), findings);
}

// ── check_batch_plan ──────────────────────────────────────────────────────────

#[test]
fn test_the_gate_passes_a_plan_whose_batches_stay_within_every_resolved_ceiling() {
    let plan = planned_plan(
        vec![
            estimate(
                "T001",
                vec![scope_estimate("domain", 200, 100), scope_estimate("usecase", 40, 20)],
                TaskDecomposition::Decomposable,
            ),
            estimate(
                "T002",
                vec![scope_estimate("domain", 60, 40)],
                TaskDecomposition::Decomposable,
            ),
        ],
        vec![declaration("B1", &["T001", "T002"])],
    );

    let outcome = check_batch_plan(
        &plan,
        &scope_config(&[("domain", Some(500)), ("usecase", Some(500))]),
        &[planned("T001", &[]), planned("T002", &["T001"])],
    );

    assert_eq!(outcome, BatchPlanGateOutcome::Passed);
}

#[test]
fn test_the_gate_blocks_a_batch_whose_scope_total_exceeds_the_ceiling() {
    let plan = planned_plan(
        vec![
            estimate(
                "T001",
                vec![scope_estimate("domain", 300, 100)],
                TaskDecomposition::Decomposable,
            ),
            estimate(
                "T002",
                vec![scope_estimate("domain", 200, 50)],
                TaskDecomposition::Decomposable,
            ),
        ],
        vec![declaration("B1", &["T001", "T002"])],
    );

    let outcome = check_batch_plan(
        &plan,
        &scope_config(&[("domain", Some(500))]),
        &[planned("T001", &[]), planned("T002", &[])],
    );

    assert_eq!(
        violations_of(&outcome),
        &[BatchPlanGateViolation::CeilingExceeded {
            batch_id: batch_id("B1"),
            scope: scope("domain"),
            total: LineCount::new(650),
            ceiling: LineCount::new(500),
        }]
    );
}

#[test]
fn test_the_gate_judges_each_scope_of_a_batch_separately() {
    let plan = planned_plan(
        vec![estimate(
            "T001",
            vec![scope_estimate("domain", 400, 250), scope_estimate("usecase", 40, 20)],
            TaskDecomposition::Decomposable,
        )],
        vec![declaration("B1", &["T001"])],
    );

    let outcome = check_batch_plan(
        &plan,
        &scope_config(&[("domain", Some(500)), ("usecase", Some(500))]),
        &[planned("T001", &[])],
    );

    // domain is over at 650; usecase stays at 60 and is checked as usual.
    let over_budget = scope("domain");
    assert_eq!(violations_of(&outcome).len(), 1);
    assert!(matches!(
        violations_of(&outcome).first(),
        Some(BatchPlanGateViolation::CeilingExceeded { scope, .. }) if *scope == over_budget
    ));
}

#[test]
fn test_the_gate_exempts_a_scope_whose_only_contributor_states_why_it_cannot_be_split() {
    let plan = planned_plan(
        vec![estimate(
            "T001",
            vec![scope_estimate("domain", 700, 300)],
            TaskDecomposition::Indivisible(justification("the transition table cannot be split")),
        )],
        vec![declaration("B1", &["T001"])],
    );

    let outcome =
        check_batch_plan(&plan, &scope_config(&[("domain", Some(500))]), &[planned("T001", &[])]);

    assert_eq!(outcome, BatchPlanGateOutcome::Passed);
}

#[test]
fn test_the_gate_blocks_an_oversize_scope_that_has_more_than_one_contributor() {
    let plan = planned_plan(
        vec![
            estimate(
                "T001",
                vec![scope_estimate("domain", 700, 300)],
                TaskDecomposition::Indivisible(justification(
                    "the transition table cannot be split",
                )),
            ),
            estimate(
                "T002",
                vec![scope_estimate("domain", 10, 5)],
                TaskDecomposition::Decomposable,
            ),
        ],
        vec![declaration("B1", &["T001", "T002"])],
    );

    let outcome = check_batch_plan(
        &plan,
        &scope_config(&[("domain", Some(500))]),
        &[planned("T001", &[]), planned("T002", &[])],
    );

    assert_eq!(
        violations_of(&outcome),
        &[BatchPlanGateViolation::OversizeScopeHasMultipleContributors {
            batch_id: batch_id("B1"),
            scope: scope("domain"),
            indivisible_task: task("T001"),
            other_contributors: vec![task("T002")],
        }]
    );
}

#[test]
fn test_the_gate_does_not_compare_a_scope_with_no_resolved_ceiling() {
    let plan = planned_plan(
        vec![estimate(
            "T001",
            vec![scope_estimate("domain", 5_000, 5_000)],
            TaskDecomposition::Decomposable,
        )],
        vec![declaration("B1", &["T001"])],
    );

    let outcome =
        check_batch_plan(&plan, &scope_config(&[("domain", None)]), &[planned("T001", &[])]);

    assert_eq!(outcome, BatchPlanGateOutcome::Passed, "an unconstrained scope changes no verdict");
}

#[test]
fn test_the_gate_blocks_a_plan_that_names_a_task_the_implementation_plan_does_not_have() {
    let plan = planned_plan(
        vec![
            estimate(
                "T001",
                vec![scope_estimate("domain", 10, 5)],
                TaskDecomposition::Decomposable,
            ),
            estimate(
                "T404",
                vec![scope_estimate("domain", 10, 5)],
                TaskDecomposition::Decomposable,
            ),
        ],
        vec![declaration("B1", &["T001", "T404"])],
    );

    let outcome =
        check_batch_plan(&plan, &scope_config(&[("domain", Some(500))]), &[planned("T001", &[])]);

    assert_eq!(
        violations_of(&outcome),
        &[BatchPlanGateViolation::UnknownTaskRef { task_id: task("T404") }]
    );
}

#[test]
fn test_the_gate_blocks_a_planned_task_that_belongs_to_no_batch() {
    let plan = planned_plan(
        vec![estimate(
            "T001",
            vec![scope_estimate("domain", 10, 5)],
            TaskDecomposition::Decomposable,
        )],
        vec![declaration("B1", &["T001"])],
    );

    let outcome = check_batch_plan(
        &plan,
        &scope_config(&[("domain", Some(500))]),
        &[planned("T001", &[]), planned("T002", &[])],
    );

    assert_eq!(
        violations_of(&outcome),
        &[BatchPlanGateViolation::UnplannedTask { task_id: task("T002") }]
    );
}

#[test]
fn test_the_gate_passes_a_plan_that_declares_only_the_remaining_work() {
    // T001 is done with a commit recorded and T002 is skipped, so a plan holding
    // only T003 declares everything it is required to.
    let plan = planned_plan(
        vec![estimate(
            "T003",
            vec![scope_estimate("domain", 10, 5)],
            TaskDecomposition::Decomposable,
        )],
        vec![declaration("B1", &["T003"])],
    );
    let config = scope_config(&[("domain", Some(500))]);
    let planned_tasks =
        [settled_by_commit("T001"), planned_in("T002", TaskStatus::Skipped), planned("T003", &[])];

    assert_eq!(check_batch_plan(&plan, &config, &planned_tasks), BatchPlanGateOutcome::Passed);

    // With the settled tasks undeclared, the one declared batch is the current
    // one, so its todo member is not refused for membership either.
    let decision = evaluate_admission(
        &plan,
        &config,
        &task("T003"),
        &committed(&["T001", "T002"]),
        &in_progress(&[]),
        &measured(&[]),
    )
    .unwrap();
    assert_eq!(decision, AdmissionDecision::Admitted);
}

#[test]
fn test_the_gate_reports_an_unsettled_planned_task_in_no_batch_whichever_unsettled_state_it_is_in()
{
    let plan = planned_plan(
        vec![estimate(
            "T001",
            vec![scope_estimate("domain", 10, 5)],
            TaskDecomposition::Decomposable,
        )],
        vec![declaration("B1", &["T001"])],
    );
    let config = scope_config(&[("domain", Some(500))]);

    // Done without a commit recorded is unsettled too: the work is not on the
    // branch, so the plan is still required to declare it.
    for status in [TaskStatus::Todo, TaskStatus::InProgress, TaskStatus::DonePending] {
        let outcome = check_batch_plan(
            &plan,
            &config,
            &[planned("T001", &[]), planned_in("T002", status.clone())],
        );

        assert_eq!(
            violations_of(&outcome),
            &[BatchPlanGateViolation::UnplannedTask { task_id: task("T002") }],
            "{status:?} is unsettled and must still be reported"
        );
    }
}

#[test]
fn test_a_declared_settled_task_is_judged_exactly_as_an_unsettled_one_would_be() {
    let config = scope_config(&[("domain", Some(500))]);

    // Its estimate counts towards the batch total: 400 declared by the settled
    // task plus 250 by the todo one is 650 against 500.
    let summed = planned_plan(
        vec![
            estimate(
                "T001",
                vec![scope_estimate("domain", 300, 100)],
                TaskDecomposition::Decomposable,
            ),
            estimate(
                "T002",
                vec![scope_estimate("domain", 200, 50)],
                TaskDecomposition::Decomposable,
            ),
        ],
        vec![declaration("B1", &["T001", "T002"])],
    );
    assert_eq!(
        violations_of(&check_batch_plan(
            &summed,
            &config,
            &[settled_by_commit("T001"), planned("T002", &[])]
        )),
        &[BatchPlanGateViolation::CeilingExceeded {
            batch_id: batch_id("B1"),
            scope: scope("domain"),
            total: LineCount::new(650),
            ceiling: LineCount::new(500),
        }]
    );

    // The single-contributor exemption is applied to it on the same terms.
    let indivisible = planned_plan(
        vec![estimate(
            "T001",
            vec![scope_estimate("domain", 700, 300)],
            TaskDecomposition::Indivisible(justification("the transition table cannot be split")),
        )],
        vec![declaration("B1", &["T001"])],
    );
    assert_eq!(
        check_batch_plan(&indivisible, &config, &[settled_by_commit("T001")]),
        BatchPlanGateOutcome::Passed
    );

    // An unknown scope name it declares still fires, and so does the existence
    // check on an id no task provides.
    let misdeclared = planned_plan(
        vec![estimate(
            "T404",
            vec![scope_estimate("domian", 10, 5)],
            TaskDecomposition::Decomposable,
        )],
        vec![declaration("B1", &["T404"])],
    );
    assert_eq!(
        violations_of(&check_batch_plan(&misdeclared, &config, &[settled_by_commit("T001")])),
        &[
            BatchPlanGateViolation::UnknownTaskRef { task_id: task("T404") },
            BatchPlanGateViolation::UnknownMainScopeName {
                task_id: task("T404"),
                scope: main_scope("domian"),
            },
        ]
    );
}

#[test]
fn test_the_gate_blocks_a_declared_dependency_whose_target_sits_in_a_later_batch() {
    let plan = planned_plan(
        vec![
            estimate(
                "T001",
                vec![scope_estimate("domain", 10, 5)],
                TaskDecomposition::Decomposable,
            ),
            estimate(
                "T002",
                vec![scope_estimate("domain", 10, 5)],
                TaskDecomposition::Decomposable,
            ),
        ],
        vec![declaration("B1", &["T001"]), declaration("B2", &["T002"])],
    );

    let outcome = check_batch_plan(
        &plan,
        &scope_config(&[("domain", Some(500))]),
        &[planned("T001", &["T002"]), planned("T002", &[])],
    );

    assert_eq!(
        violations_of(&outcome),
        &[BatchPlanGateViolation::DependencyInLaterBatch {
            task_id: task("T001"),
            task_batch: batch_id("B1"),
            dependency: task("T002"),
            dependency_batch: batch_id("B2"),
        }]
    );
}

#[test]
fn test_the_gate_ignores_a_dependency_edge_whose_settled_endpoint_the_plan_does_not_declare() {
    let config = scope_config(&[("domain", Some(500))]);

    // T002 declares a dependency on the settled T001, which the plan leaves out.
    // The edge has no batch to compare against, so the ordering check skips it —
    // and a delivered endpoint can never be the later batch anyway.
    let remaining_work_only = planned_plan(
        vec![estimate(
            "T002",
            vec![scope_estimate("domain", 10, 5)],
            TaskDecomposition::Decomposable,
        )],
        vec![declaration("B1", &["T002"])],
    );
    assert_eq!(
        check_batch_plan(
            &remaining_work_only,
            &config,
            &[settled_by_commit("T001"), planned("T002", &["T001"])]
        ),
        BatchPlanGateOutcome::Passed,
        "neither the skipped edge nor the undeclared settled task is a finding"
    );

    // The contrast: with both endpoints unsettled and declared, the same edge
    // shape pointing into a later batch is still reported.
    let both_declared = planned_plan(
        vec![
            estimate(
                "T001",
                vec![scope_estimate("domain", 10, 5)],
                TaskDecomposition::Decomposable,
            ),
            estimate(
                "T002",
                vec![scope_estimate("domain", 10, 5)],
                TaskDecomposition::Decomposable,
            ),
        ],
        vec![declaration("B1", &["T002"]), declaration("B2", &["T001"])],
    );
    assert_eq!(
        violations_of(&check_batch_plan(
            &both_declared,
            &config,
            &[planned("T001", &[]), planned("T002", &["T001"])]
        )),
        &[BatchPlanGateViolation::DependencyInLaterBatch {
            task_id: task("T002"),
            task_batch: batch_id("B1"),
            dependency: task("T001"),
            dependency_batch: batch_id("B2"),
        }]
    );
}

#[test]
fn test_the_gate_accepts_a_declared_dependency_in_the_same_batch_or_an_earlier_one() {
    let estimates = || {
        vec![
            estimate(
                "T001",
                vec![scope_estimate("domain", 10, 5)],
                TaskDecomposition::Decomposable,
            ),
            estimate(
                "T002",
                vec![scope_estimate("domain", 10, 5)],
                TaskDecomposition::Decomposable,
            ),
        ]
    };
    let planned_tasks = [planned("T001", &[]), planned("T002", &["T001"])];
    let config = scope_config(&[("domain", Some(500))]);

    let same_batch = planned_plan(estimates(), vec![declaration("B1", &["T001", "T002"])]);
    let earlier_batch =
        planned_plan(estimates(), vec![declaration("B1", &["T001"]), declaration("B2", &["T002"])]);

    assert_eq!(
        check_batch_plan(&same_batch, &config, &planned_tasks),
        BatchPlanGateOutcome::Passed
    );
    assert_eq!(
        check_batch_plan(&earlier_batch, &config, &planned_tasks),
        BatchPlanGateOutcome::Passed
    );
}

#[test]
fn test_the_gate_ignores_the_batch_placement_of_tasks_that_declare_no_dependency() {
    let plan = planned_plan(
        vec![
            estimate(
                "T001",
                vec![scope_estimate("domain", 10, 5)],
                TaskDecomposition::Decomposable,
            ),
            estimate(
                "T002",
                vec![scope_estimate("domain", 10, 5)],
                TaskDecomposition::Decomposable,
            ),
        ],
        // T002 is batched before T001, which would violate an edge if one existed.
        vec![declaration("B1", &["T002"]), declaration("B2", &["T001"])],
    );

    let outcome = check_batch_plan(
        &plan,
        &scope_config(&[("domain", Some(500))]),
        &[planned("T001", &[]), planned("T002", &[])],
    );

    assert_eq!(outcome, BatchPlanGateOutcome::Passed);
}

#[test]
fn test_the_gate_blocks_an_estimate_naming_a_scope_the_configuration_does_not_define() {
    // The canonical case: a misspelled configured name. It is reported rather
    // than resolving no ceiling and going uncompared, so the uncompared lanes
    // stay the two legitimate ones (AC-08).
    let plan = planned_plan(
        vec![estimate(
            "T001",
            vec![scope_estimate("domian", 10, 5)],
            TaskDecomposition::Decomposable,
        )],
        vec![declaration("B1", &["T001"])],
    );

    let outcome =
        check_batch_plan(&plan, &scope_config(&[("domain", Some(500))]), &[planned("T001", &[])]);

    assert_eq!(
        violations_of(&outcome),
        &[BatchPlanGateViolation::UnknownMainScopeName {
            task_id: task("T001"),
            scope: main_scope("domian"),
        }]
    );
}

#[test]
fn test_the_gate_never_reports_the_reserved_other_scope_as_an_unknown_name() {
    let plan = planned_plan(
        vec![estimate(
            "T001",
            vec![other_scope_estimate(5_000, 5_000), scope_estimate("domain", 10, 5)],
            TaskDecomposition::Decomposable,
        )],
        vec![declaration("B1", &["T001"])],
    );

    let outcome =
        check_batch_plan(&plan, &scope_config(&[("domain", Some(500))]), &[planned("T001", &[])]);

    assert_eq!(
        outcome,
        BatchPlanGateOutcome::Passed,
        "`other` is always valid and carries no ceiling, so its size changes no verdict"
    );
}

#[test]
fn test_the_gate_reports_every_violation_it_finds_rather_than_stopping_at_the_first() {
    let plan = planned_plan(
        vec![
            estimate(
                "T001",
                vec![scope_estimate("domain", 300, 100)],
                TaskDecomposition::Decomposable,
            ),
            estimate(
                "T404",
                vec![scope_estimate("domain", 200, 50)],
                TaskDecomposition::Decomposable,
            ),
        ],
        vec![declaration("B1", &["T001", "T404"])],
    );

    let outcome = check_batch_plan(
        &plan,
        &scope_config(&[("domain", Some(500))]),
        &[planned("T001", &[]), planned("T002", &[])],
    );

    assert_eq!(
        violations_of(&outcome),
        &[
            BatchPlanGateViolation::UnknownTaskRef { task_id: task("T404") },
            BatchPlanGateViolation::UnplannedTask { task_id: task("T002") },
            BatchPlanGateViolation::CeilingExceeded {
                batch_id: batch_id("B1"),
                scope: scope("domain"),
                total: LineCount::new(650),
                ceiling: LineCount::new(500),
            },
        ]
    );
}

#[test]
fn test_a_task_claimed_by_two_batches_never_reaches_the_gate() {
    let estimates = || {
        vec![
            estimate(
                "T001",
                vec![scope_estimate("domain", 10, 5)],
                TaskDecomposition::Decomposable,
            ),
            estimate(
                "T002",
                vec![scope_estimate("domain", 10, 5)],
                TaskDecomposition::Decomposable,
            ),
        ]
    };

    // The half of "exactly one batch" that is not UnplannedTask: a plan that
    // lists T001 in two batches cannot be assembled at all, so no such plan can
    // be handed to the gate.
    let doubly_claimed = BatchPlanDocument::new(
        track(),
        estimates(),
        vec![declaration("B1", &["T001", "T002"]), declaration("B2", &["T001"])],
    );
    let Err(BatchPlanValidationError::DuplicateBatchMembership { task_id, batch_ids }) =
        doubly_claimed
    else {
        panic!("a task claimed by two batches must be refused before the gate sees it");
    };
    assert_eq!(task_id, task("T001"));
    assert_eq!(batch_ids, vec![batch_id("B1"), batch_id("B2")]);

    // Every plan the gate does receive therefore places each planned task in
    // exactly one batch, and passes the membership half of the check.
    let plan =
        planned_plan(estimates(), vec![declaration("B1", &["T001"]), declaration("B2", &["T002"])]);
    for member in ["T001", "T002"] {
        let claiming = plan.batches().iter().filter(|batch| batch.contains(&task(member))).count();
        assert_eq!(claiming, 1, "{member} must be claimed by exactly one batch");
    }
    assert_eq!(
        check_batch_plan(
            &plan,
            &scope_config(&[("domain", Some(500))]),
            &[planned("T001", &[]), planned("T002", &[])]
        ),
        BatchPlanGateOutcome::Passed
    );
}

#[test]
fn test_an_unknown_task_id_is_reported_once_because_estimates_and_batch_members_are_one_set() {
    let known =
        || estimate("T001", vec![scope_estimate("domain", 10, 5)], TaskDecomposition::Decomposable);
    let unknown =
        || estimate("T404", vec![scope_estimate("domain", 10, 5)], TaskDecomposition::Decomposable);

    // A batch member with no estimate is not assemblable …
    assert!(matches!(
        BatchPlanDocument::new(track(), vec![known()], vec![declaration("B1", &["T001", "T404"])]),
        Err(BatchPlanValidationError::MissingTaskEstimate { .. })
    ));
    // … and neither is an estimate no batch claims …
    assert!(matches!(
        BatchPlanDocument::new(
            track(),
            vec![known(), unknown()],
            vec![declaration("B1", &["T001"])]
        ),
        Err(BatchPlanValidationError::UnassignedTask { .. })
    ));

    // … so an id unknown to the implementation plan is always both an estimate
    // id and a batch-member id, and the gate names it exactly once.
    let plan = planned_plan(vec![known(), unknown()], vec![declaration("B1", &["T001", "T404"])]);
    let outcome =
        check_batch_plan(&plan, &scope_config(&[("domain", Some(500))]), &[planned("T001", &[])]);

    assert_eq!(
        violations_of(&outcome),
        &[BatchPlanGateViolation::UnknownTaskRef { task_id: task("T404") }]
    );
}

// ── NonZeroLineCount / AdmissionDecision ──────────────────────────────────────

#[test]
fn test_a_zero_quantity_cannot_become_a_non_zero_line_count() {
    assert!(NonZeroLineCount::try_new(LineCount::new(0)).is_none());
    assert_eq!(
        NonZeroLineCount::try_new(LineCount::new(250)).map(|count| count.get()),
        Some(LineCount::new(250))
    );
}

#[test]
fn test_an_admission_decision_carries_its_rejection_only_when_it_refuses() {
    let rejection = AdmissionRejection::NotCurrentBatchMember {
        task_id: task("T003"),
        task_batch: batch_id("B2"),
        current_batch: batch_id("B1"),
    };

    assert!(AdmissionDecision::Admitted.is_admitted());
    assert!(AdmissionDecision::Admitted.rejection().is_none());

    let refused = AdmissionDecision::Rejected(rejection.clone());
    assert!(!refused.is_admitted());
    assert_eq!(refused.rejection(), Some(&rejection));
}

#[test]
fn test_a_rejection_renders_the_arithmetic_it_carries() {
    let not_a_member = AdmissionRejection::NotCurrentBatchMember {
        task_id: task("T003"),
        task_batch: batch_id("B2"),
        current_batch: batch_id("B1"),
    };
    let rendered = not_a_member.to_string();
    assert!(rendered.contains("T003"), "rendered as: {rendered}");
    assert!(rendered.contains("B2") && rendered.contains("B1"), "rendered as: {rendered}");

    let over_ceiling = AdmissionRejection::ScopeCeilingWouldBeExceeded {
        scope: scope("domain"),
        prior_contribution: NonZeroLineCount::try_new(LineCount::new(250)).unwrap(),
        candidate_estimate: LineCount::new(300),
        ceiling: LineCount::new(500),
    };
    let rendered = over_ceiling.to_string();
    assert!(rendered.contains("domain"), "rendered as: {rendered}");
    assert!(rendered.contains("250") && rendered.contains("300"), "rendered as: {rendered}");
    assert!(rendered.contains("500"), "rendered as: {rendered}");
}

// ── evaluate_admission ────────────────────────────────────────────────────────

#[test]
fn test_admission_admits_a_current_batch_member_within_every_ceiling() {
    let decision = evaluate_admission(
        &admission_plan(),
        &admission_config(),
        &task("T001"),
        &committed(&[]),
        &in_progress(&[]),
        &measured(&[]),
    )
    .unwrap();

    assert_eq!(decision, AdmissionDecision::Admitted);
}

#[test]
fn test_admission_refuses_a_later_batch_task_until_the_current_batch_is_committed() {
    let plan = admission_plan();
    let config = admission_config();

    let refused = evaluate_admission(
        &plan,
        &config,
        &task("T003"),
        &committed(&[]),
        &in_progress(&[]),
        &measured(&[]),
    )
    .unwrap();
    assert_eq!(
        refused,
        AdmissionDecision::Rejected(AdmissionRejection::NotCurrentBatchMember {
            task_id: task("T003"),
            task_batch: batch_id("B2"),
            current_batch: batch_id("B1"),
        })
    );

    let admitted = evaluate_admission(
        &plan,
        &config,
        &task("T003"),
        &committed(&["T001", "T002"]),
        &in_progress(&[]),
        &measured(&[]),
    )
    .unwrap();
    assert_eq!(admitted, AdmissionDecision::Admitted, "B2 opens once B1 is committed");
}

#[test]
fn test_admission_admits_any_estimate_when_the_scope_carries_nothing_yet() {
    let plan = planned_plan(
        vec![estimate(
            "T001",
            vec![scope_estimate("domain", 900, 200)],
            TaskDecomposition::Indivisible(justification("the transition table cannot be split")),
        )],
        vec![declaration("B1", &["T001"])],
    );

    let decision = evaluate_admission(
        &plan,
        &admission_config(),
        &task("T001"),
        &committed(&[]),
        &in_progress(&[]),
        &measured(&[]),
    )
    .unwrap();

    assert_eq!(
        decision,
        AdmissionDecision::Admitted,
        "1100 declared lines against a 500 ceiling still get in as the first contribution"
    );
}

#[test]
fn test_admission_refuses_when_the_prior_contribution_plus_the_estimate_passes_the_ceiling() {
    let decision = evaluate_admission(
        &admission_plan(),
        &admission_config(),
        &task("T002"),
        &committed(&[]),
        &in_progress(&[]),
        &measured(&[("domain", 250)]),
    )
    .unwrap();

    assert_eq!(
        decision,
        AdmissionDecision::Rejected(AdmissionRejection::ScopeCeilingWouldBeExceeded {
            scope: scope("domain"),
            prior_contribution: NonZeroLineCount::try_new(LineCount::new(250)).unwrap(),
            candidate_estimate: LineCount::new(300),
            ceiling: LineCount::new(500),
        })
    );
}

#[test]
fn test_admission_counts_the_measured_diff_and_the_in_progress_estimates_together() {
    // 100 measured + T001's declared 150 = 250 prior, plus T002's 300 = 550.
    let decision = evaluate_admission(
        &admission_plan(),
        &admission_config(),
        &task("T002"),
        &committed(&[]),
        &in_progress(&["T001"]),
        &measured(&[("domain", 100)]),
    )
    .unwrap();

    let Some(AdmissionRejection::ScopeCeilingWouldBeExceeded { prior_contribution, .. }) =
        decision.rejection()
    else {
        panic!("the accumulated contribution must refuse the transition: {decision:?}");
    };
    assert_eq!(prior_contribution.get(), LineCount::new(250));
}

#[test]
fn test_admission_leaves_scopes_the_candidate_does_not_declare_out_of_the_comparison() {
    // T003 declares usecase only; domain is far over its ceiling already.
    let decision = evaluate_admission(
        &admission_plan(),
        &admission_config(),
        &task("T003"),
        &committed(&["T001", "T002"]),
        &in_progress(&[]),
        &measured(&[("domain", 9_000)]),
    )
    .unwrap();

    assert_eq!(decision, AdmissionDecision::Admitted);
}

#[test]
fn test_admission_errors_when_the_batch_plan_holds_no_estimate_for_the_candidate() {
    let result = evaluate_admission(
        &admission_plan(),
        &admission_config(),
        &task("T404"),
        &committed(&[]),
        &in_progress(&[]),
        &measured(&[]),
    );

    let Err(AdmissionEvaluationError::MissingTaskEstimate { task_id }) = result else {
        panic!("an unestimated candidate must be an error, not a silent pass");
    };
    assert_eq!(task_id, task("T404"));
}

#[test]
fn test_admission_errors_when_the_candidate_declares_a_scope_the_configuration_does_not_define() {
    let plan = planned_plan(
        vec![estimate(
            "T001",
            vec![scope_estimate("domian", 900, 200)],
            TaskDecomposition::Decomposable,
        )],
        vec![declaration("B1", &["T001"])],
    );

    let result = evaluate_admission(
        &plan,
        &admission_config(),
        &task("T001"),
        &committed(&[]),
        &in_progress(&[]),
        &measured(&[("domian", 400)]),
    );

    // Same rank as a missing estimate: it leaves through the error channel rather
    // than becoming a decision, and names both parties.
    let error = result.expect_err("an unrecognised name must not admit on an unresolved ceiling");
    assert!(
        matches!(
            &error,
            AdmissionEvaluationError::UnknownMainScopeName { task_id, scope }
                if task_id == &task("T001") && scope == &main_scope("domian")
        ),
        "unexpected: {error:?}"
    );
    assert!(error.to_string().contains("domian"), "rendered as: {error}");
}

#[test]
fn test_admission_never_errors_on_an_estimate_for_the_reserved_other_scope() {
    let plan = planned_plan(
        vec![estimate(
            "T001",
            vec![other_scope_estimate(900, 200)],
            TaskDecomposition::Decomposable,
        )],
        vec![declaration("B1", &["T001"])],
    );

    let decision = evaluate_admission(
        &plan,
        &admission_config(),
        &task("T001"),
        &committed(&[]),
        &in_progress(&[]),
        // `other` already carries lines, so the guard is reached rather than
        // short-circuited by a zero prior contribution — and still admits, since
        // `other` resolves no ceiling.
        &[MeasuredScopeDiff::new(ScopeName::Other, LineCount::new(9_000))],
    )
    .unwrap();

    assert_eq!(decision, AdmissionDecision::Admitted);
}
