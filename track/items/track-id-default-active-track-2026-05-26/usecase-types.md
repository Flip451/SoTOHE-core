<!-- Generated from usecase-types.json — DO NOT EDIT DIRECTLY -->

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| ActiveTrackResolveError | error_type | — | BranchRead, Resolution, BranchMismatch | 🔵 | 🔵 |
| BranchReadError | error_type | — | ReadFailed | 🔵 | 🔵 |
| TrackResolutionError | error_type | reference | DetachedHead, NotTrackBranch, NoBranch, InvalidTrackId, UnsupportedTargetStatus, TrackNotFound, ReadError | 🔵 | 🔵 |

## Secondary Ports

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| BranchReaderPort | secondary_port | — | fn current_branch(&self) -> Result<Option<String>, BranchReadError> | 🔵 | 🔵 |

## Application Services

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| ActiveTrackResolveService | application_service | — | fn resolve_active_track(&self) -> Result<String, ActiveTrackResolveError>, fn resolve_for_read(&self, explicit_id: Option<String>) -> Result<String, ActiveTrackResolveError>, fn resolve_for_write(&self, explicit_id: Option<String>) -> Result<String, ActiveTrackResolveError> | 🔵 | 🔵 |
| TaskOperationService | application_service | reference | fn transition_task(&self, cmd: TaskTransitionCommand) -> Result<TaskOperationOutput, TaskOperationError>, fn add_task(&self, cmd: AddTaskCommand) -> Result<TaskOperationOutput, TaskOperationError>, fn set_override(&self, cmd: SetOverrideCommand) -> Result<TaskOperationOutput, TaskOperationError>, fn clear_override(&self, cmd: ClearOverrideCommand) -> Result<TaskOperationOutput, TaskOperationError> | 🔵 | 🔵 |

## Interactors

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| ActiveTrackResolveInteractor | interactor | — | — | 🔵 | 🔵 |
| TaskOperationInteractor | interactor | modify | — | 🔵 | 🔵 |

## DTOs

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| ExplicitTrackBranch | dto | modify | — | 🔵 | 🔵 |
| TrackBranchClaim | dto | modify | — | 🔵 | 🔵 |

## Commands

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| AddTaskCommand | command | modify | — | 🔵 | 🔵 |
| ClearOverrideCommand | command | modify | — | 🔵 | 🔵 |
| SetOverrideCommand | command | modify | — | 🔵 | 🔵 |
| TaskTransitionCommand | command | modify | — | 🔵 | 🔵 |

## Free Functions

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| usecase::git_workflow::validate_planning_only_commit_paths | free_function | delete | fn() -> Result<(), String> | 🔵 | 🔵 |
| usecase::track_resolution::resolve_track_id_from_branch | free_function | reference | fn(branch: Option<&str>) -> Result<String, TrackResolutionError> | 🔵 | 🔵 |

