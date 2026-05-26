<!-- Generated from usecase-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| ActivateTrackOutcome | enum | delete | Activated, AlreadyActive | 🔵 | 🔵 |

## Value Objects

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| PrBranchContext | value_object | modify | — | 🔵 | 🔵 |

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| PreCommitTypeSignalsError | error_type | modify | GitDiscoverFailed, RulesFileMissing, RulesParseError, SymlinkRejected, BranchNotFound, BranchMismatch, TypeSignalsRecomputeFailed | 🔵 | 🔵 |
| TrackResolutionError | error_type | modify | DetachedHead, NotTrackBranch, NoBranch, InvalidTrackId, UnsupportedTargetStatus, TrackNotFound, ReadError | 🔵 | 🔵 |
| TypeSignalsError | error_type | reference | InvalidTrackId, NonActiveTrack, BranchTrackMismatch, LayerBindingsLoad, NoLayers, EvaluationFailed, InconsistentRequest | 🔵 | 🔵 |
| WorktreeGuardError | error_type | delete | DirtyWorktree, WorktreeReadFailed | 🔵 | 🔵 |

## Application Services

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| PreCommitTypeSignalsService | application_service | reference | fn run(&self, track_id: String, workspace_root: std::path::PathBuf) -> Result<PreCommitTypeSignalsOutput, PreCommitTypeSignalsError> | 🔵 | 🔵 |
| TypeSignalsService | application_service | reference | fn run(&self, request: TypeSignalsRequest) -> Result<(), TypeSignalsError> | 🔵 | 🔵 |

## Use Cases

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| ActivateTrackUseCase | use_case | delete | — | 🔵 | 🔵 |

## Interactors

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| PreCommitTypeSignalsInteractor | interactor | modify | — | 🔵 | 🔵 |
| TaskOperationInteractor | interactor | modify | — | 🔵 | 🔵 |
| TrackPhaseInteractor | interactor | modify | — | 🔵 | 🔵 |
| TypeSignalsInteractor | interactor | reference | — | 🔵 | 🔵 |

## DTOs

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| PreCommitTypeSignalsOutput | dto | modify | — | 🔵 | 🔵 |

## Commands

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| TypeSignalsRequest | command | reference | — | 🔵 | 🔵 |

## Free Functions

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| usecase::track_resolution::reject_branchless_guard | free_function | delete | fn() -> Result<(), TrackResolutionError> | 🔵 | 🔵 |
| usecase::track_resolution::reject_branchless_guard_by_str | free_function | delete | fn() -> Result<(), TrackResolutionError> | 🔵 | 🔵 |
| usecase::track_resolution::reject_branchless_implementation_transition | free_function | delete | fn() -> Result<(), TrackResolutionError> | 🔵 | 🔵 |
| usecase::track_resolution::reject_branchless_implementation_transition_by_str | free_function | delete | fn() -> Result<(), TrackResolutionError> | 🔵 | 🔵 |
| usecase::track_resolution::resolve_track_or_plan_id_from_branch | free_function | delete | fn() -> Result<String, TrackResolutionError> | 🔵 | 🔵 |
| usecase::worktree_guard::ensure_clean_worktree | free_function | delete | fn() -> Result<(), WorktreeGuardError> | 🔵 | 🔵 |
| usecase::worktree_guard::parse_dirty_worktree_paths | free_function | delete | fn() -> Vec<String> | 🔵 | 🔵 |
| usecase::worktree_guard::validate_clean_worktree | free_function | delete | fn() -> Result<(), WorktreeGuardError> | 🔵 | 🔵 |

