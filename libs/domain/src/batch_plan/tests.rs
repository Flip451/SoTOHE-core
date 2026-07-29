//! Unit tests for the declared batch-plan value objects.

use std::collections::BTreeSet;

use super::*;
use crate::review_v2::{MainScopeName, ScopeName};
use crate::{TaskId, TrackId};

fn scope(name: &str) -> ScopeName {
    ScopeName::Main(MainScopeName::new(name).unwrap())
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
