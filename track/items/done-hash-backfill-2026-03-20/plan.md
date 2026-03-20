<!-- Generated from metadata.json — DO NOT EDIT DIRECTLY -->
# Domain semantics hardening: type-safe states, eliminate stringly-typed fields

Domain layer semantics hardening: eliminate stringly-typed fields and Option-encoded distinct states.
Phase A (WF-40): Split TaskStatus::Done into DonePending/DoneTraced for commit_hash backfill.
Phase B: Type-safe track phase resolution (TrackStatus enum, NextCommand enum) + infrastructure propagation.
Phase C: Review system ADTs (ReviewGroupProgress, CodeHash::NotRecorded, NonEmptyString fields) + codec propagation.
Phase D: Minor cleanups (AutoPhaseError, StatusOverride, ReviewGroupName newtype) + codec/CLI propagation.
All changes maintain JSON serialization backward-compatibility (field omission preserved, no new required fields).

## Phase A: TaskStatus Split and BackfillHash (WF-40)

Split Done { commit_hash: Option<CommitHash> } into DonePending and DoneTraced { commit_hash: CommitHash }
Add TaskTransition::BackfillHash { commit_hash: CommitHash }
Transition rules: InProgress + Complete(None) -> DonePending, InProgress + Complete(Some) -> DoneTraced
DonePending + BackfillHash(hash) -> DoneTraced { hash }
DoneTraced + BackfillHash -> InvalidTaskTransition (no overwrite)
DonePending/DoneTraced + Reopen -> InProgress
TaskStatusKind::Done is preserved for external compatibility
resolve_transition() signature change: accept &TaskStatus instead of TaskStatusKind to discriminate DonePending vs DoneTraced
usecase execute_by_status(): pass task.status() instead of task.status().kind() to resolve_transition
CLI transition.rs: pass full TaskStatus from TrackTask to resolve_transition
Update usecase resolve_transition, codec, render, CLI match arms

- [ ] domain: Split TaskStatus::Done into DonePending/DoneTraced, add BackfillHash transition
- [ ] usecase: Update resolve_transition() to accept &TaskStatus (not TaskStatusKind), update execute_by_status() caller to pass task.status() instead of task.status().kind()
- [ ] infrastructure/codec: Update parse_task_status and task_to_document for DonePending/DoneTraced
- [ ] infrastructure/render: Update plan.md rendering match arms for DonePending/DoneTraced
- [ ] CLI: Update pr.rs task resolution check and state_ops.rs task counts for new variants
- [ ] docs: Update task-completion-flow.md WF-40 constraint and TODO.md

## Phase B: Type-safe Track Phase Resolution

T007: Harden resolve_phase() to use TrackStatus enum matching (it already takes TrackMetadata but derives status via string internally)
resolve_phase_from_record: accept TrackStatus instead of &str, remove silent fallback wildcard arm
Note: resolve_phase_from_record has no non-test production callers; resolve_phase (used by resolve.rs and render.rs) is the primary target
Production callers: apps/cli/src/commands/track/resolve.rs and libs/infrastructure/src/track/render.rs
T008: Replace next_command: String with NextCommand enum
Variants: Implement, Done, ActivateTrack(TrackId), PlanNewFeature, Status
Add Display impl for user-facing output (/track:implement, etc.)
T009: Propagate to infrastructure/CLI — resolve.rs and render.rs (next_command usage, TrackStatus matching)

- [ ] domain: resolve_phase and resolve_phase_from_record — status param &str -> TrackStatus enum, remove silent fallback
- [ ] domain: TrackPhaseInfo.next_command String -> NextCommand enum
- [ ] infrastructure/CLI: Propagate Phase B changes — update resolve.rs and render.rs for NextCommand enum and TrackStatus matching

## Phase C: Review System ADTs

T010: Add CodeHash::NotRecorded, change ReviewState.code_hash from Option<CodeHash> to CodeHash
Serialization: NotRecorded preserves current behavior — field omitted from JSON (not emitted as null)
T011: Replace ReviewGroupState { fast: Option<_>, final_round: Option<_> } with ReviewGroupProgress ADT
Variants: NoRounds, FastOnly(ReviewRoundResult), FinalOnly(ReviewRoundResult), BothRounds { fast, final_round }
FinalOnly preserves backward-compat for legacy metadata with final_round-only groups
with_final_only constructor renamed to from_legacy_final_only for clarity (codec path only)
T012: Use NonEmptyString for ReviewEscalationResolution.workspace_search_ref/reinvention_check_ref/summary
Move validation from resolve_escalation() into the constructor
T013: Propagate to infrastructure/codec + CLI — review_from_document/review_to_document for CodeHash/ReviewGroupProgress, and apps/cli/src/commands/review.rs for ReviewEscalationResolution::new() constructor change

- [ ] domain: Fold Option<CodeHash> into CodeHash::NotRecorded, remove Option wrapper from ReviewState
- [ ] domain: ReviewGroupState Option pair -> ReviewGroupProgress ADT (NoRounds/FastOnly/FinalOnly/BothRounds)
- [ ] domain: ReviewEscalationResolution String fields -> NonEmptyString for workspace_search_ref/reinvention_check_ref/summary
- [ ] infrastructure/codec + CLI: Propagate Phase C changes — review_from_document/review_to_document for CodeHash/ReviewGroupProgress, review.rs for ReviewEscalationResolution constructor

## Phase D: Minor Domain Cleanups

T014: AutoPhaseError fields from/phase/to: String -> AutoPhase enum
T015: StatusOverride -> struct { kind: StatusOverrideKind, reason: NonEmptyString }
T016: Review group name String -> ReviewGroupName newtype throughout (Vec<String>, HashMap<String, ...>, record_round group: &str, expected_groups: &[String], record_round_with_pending same params)
T017: Propagate to infrastructure/codec + CLI — StatusOverride codec, AutoPhaseError callers, ReviewGroupName codec and CLI review.rs (group/expected_groups params)

- [ ] domain: AutoPhaseError String fields -> AutoPhase enum for from/phase/to
- [ ] domain: StatusOverride refactor - extract StatusOverrideKind enum, use NonEmptyString for reason
- [ ] domain: Review group name String -> ReviewGroupName newtype throughout (Vec<String>, HashMap<String, ...>, record_round group: &str, record_round expected_groups: &[String], record_round_with_pending same params)
- [ ] infrastructure/codec + CLI: Propagate Phase D changes — StatusOverride codec, AutoPhaseError callers, ReviewGroupName codec and CLI review.rs (group/expected_groups params)
