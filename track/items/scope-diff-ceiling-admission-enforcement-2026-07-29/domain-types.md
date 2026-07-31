<!-- Generated from domain-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| AdmissionDecision | enum | add | Admitted, Rejected | 🔵 | 🔵 |
| AdmissionRejection | enum | add | NotCurrentBatchMember, NoCurrentBatch, ScopeCeilingWouldBeExceeded | 🔵 | 🔵 |
| BatchPlanGateOutcome | enum | add | Passed, Blocked | 🔵 | 🔵 |
| BatchPlanGateViolation | enum | add | CeilingExceeded, OversizeScopeHasMultipleContributors, UnknownTaskRef, UnplannedTask, DependencyInLaterBatch, UnknownMainScopeName | 🔵 | 🔵 |
| ScopeCeiling | enum | add | Unconstrained, Limited | 🔵 | 🔵 |
| TaskDecomposition | enum | add | Decomposable, Indivisible | 🔵 | 🔵 |
| TaskStatusKind | enum | reference | Todo, InProgress, Done, Skipped | 🔵 | 🔵 |
| TaskTransition | enum | reference | Start, Complete, BackfillHash, ResetToTodo, Skip, Reopen | 🔵 | 🔵 |

## Value Objects

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| BatchDeclaration | value_object | add | — | 🔵 | 🔵 |
| BatchId | value_object | add | — | 🔵 | 🔵 |
| BatchPlanDocument | value_object | add | — | 🔵 | 🔵 |
| IndivisibilityJustification | value_object | add | — | 🔵 | 🔵 |
| LineCount | value_object | add | — | 🔵 | 🔵 |
| MeasuredScopeDiff | value_object | add | — | 🔵 | 🔵 |
| NonEmptyGateViolations | value_object | add | — | 🔵 | 🔵 |
| NonZeroLineCount | value_object | add | — | 🔵 | 🔵 |
| ScopeLineEstimate | value_object | add | — | 🔵 | 🔵 |
| TaskEstimate | value_object | add | — | 🔵 | 🔵 |

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| AdmissionEvaluationError | error_type | add | MissingTaskEstimate, UnknownMainScopeName | 🔵 | 🔵 |
| BatchPlanValidationError | error_type | add | EmptyJustification, EmptyBatchId, EmptyBatch, DuplicateTaskEstimate, DuplicateScopeEstimate, EmptyScopeEstimates, DuplicateBatchId, MissingTaskEstimate, UnassignedTask, DuplicateBatchMembership | 🔵 | 🔵 |
| ValidationError | error_type | modify | EmptyString, InvalidTrackId, InvalidTaskId, InvalidCommitHash, InvalidTimestamp, InvalidTrackBranch, BranchIdMismatch, StatusOverrideMismatch, EmptyTrackTitle, EmptyTaskDescription, EmptyPlanSectionId, EmptyPlanSectionTitle, DuplicateTaskId, DuplicatePlanSectionId, UnknownTaskReference, DuplicateTaskReference, UnreferencedTask, OverrideIncompatibleWithResolvedTasks, TrackActivationRequiresPlanningOnly, TrackActivationRequiresSchemaV3, TrackAlreadyMaterialized, UnsupportedTargetStatus, SectionNotFound, NoSectionsAvailable, TaskDescriptionMutated, TaskRemoved, DuplicateElementId, InvalidLayerId, InvalidSpecElementId, EmptyAdrAnchor, EmptyConventionAnchor, InvalidContentHash, EmptyInformalGroundSummary, MultiLineInformalGroundSummary, EmptyDecisionGroundRef, InvalidObligationMinimum, InvalidDetectionRate, UnknownDependencyReference, DependencyCycle, PlanOrderViolatesDependency | 🔵 | 🔵 |

## Domain Services

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| ImplPlanDocument | domain_service | modify | — | 🔵 | 🔵 |
| ReviewScopeConfig | domain_service | reference | — | 🔵 | 🔵 |

## Free Functions

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| domain::batch_plan::check_batch_plan | free_function | add | fn(plan: &BatchPlanDocument, scope_config: &ReviewScopeConfig, planned_tasks: &[TrackTask]) -> BatchPlanGateOutcome | 🔵 | 🔵 |
| domain::batch_plan::evaluate_admission | free_function | add | fn(plan: &BatchPlanDocument, scope_config: &ReviewScopeConfig, candidate: &TaskId, committed_task_ids: &std::collections::BTreeSet<TaskId>, in_progress_task_ids: &std::collections::BTreeSet<TaskId>, measured: &[MeasuredScopeDiff]) -> Result<AdmissionDecision, AdmissionEvaluationError> | 🔵 | 🔵 |

## Entities

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| TrackTask | entity | modify | — | 🔵 | 🔵 |

