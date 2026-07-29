<!-- Generated from usecase-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| TaskTransitionOutcome | enum | add | Transitioned, Rejected | 🟡 | 🔵 |

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| BatchPlanCheckError | error_type | add | BatchPlanNotFound, BatchPlanReadFailed, ImplPlanNotFound, ImplPlanReadFailed, ScopeConfigReadFailed | 🟡 | 🔵 |
| BatchPlanReadError | error_type | add | NotFound, ReadFailed | 🟡 | 🔵 |
| PlannedTaskReadError | error_type | add | NotFound, ReadFailed | 🟡 | 🔵 |
| ScopeConfigReadError | error_type | add | ReadFailed | 🟡 | 🔵 |
| ScopeDiffMeasureError | error_type | add | MeasureFailed | 🟡 | 🔵 |

## Secondary Ports

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| BatchPlanReaderPort | secondary_port | add | fn read(&self, items_dir: &std::path::Path, track_id: &domain::TrackId) -> Result<domain::batch_plan::BatchPlanDocument, BatchPlanReadError> | 🟡 | 🔵 |
| PlannedTaskReaderPort | secondary_port | add | fn read_planned_tasks(&self, items_dir: &std::path::Path, track_id: &domain::TrackId) -> Result<Vec<domain::TrackTask>, PlannedTaskReadError> | 🟡 | 🔵 |
| ScopeConfigReaderPort | secondary_port | add | fn read(&self, items_dir: &std::path::Path, track_id: &domain::TrackId) -> Result<domain::review_v2::ReviewScopeConfig, ScopeConfigReadError> | 🟡 | 🔵 |
| ScopeDiffMeasurePort | secondary_port | add | fn measure_scope_diff(&self, items_dir: &std::path::Path, track_id: &domain::TrackId) -> Result<Vec<domain::batch_plan::MeasuredScopeDiff>, ScopeDiffMeasureError> | 🟡 | 🔵 |

## Application Services

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| BatchPlanCheckService | application_service | add | fn check(&self, cmd: BatchPlanCheckCommand) -> Result<domain::batch_plan::BatchPlanGateOutcome, BatchPlanCheckError> | 🟡 | 🔵 |
| TaskOperationService | application_service | modify | fn transition_task(&self, cmd: TaskTransitionCommand) -> Result<TaskTransitionOutcome, TaskOperationError>, fn add_task(&self, cmd: AddTaskCommand) -> Result<TaskOperationOutput, TaskOperationError>, fn set_override(&self, cmd: SetOverrideCommand) -> Result<TaskOperationOutput, TaskOperationError>, fn clear_override(&self, cmd: ClearOverrideCommand) -> Result<TaskOperationOutput, TaskOperationError> | 🟡 | 🔵 |

## Interactors

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| BatchPlanCheckInteractor | interactor | add | — | 🟡 | 🔵 |
| TaskOperationInteractor | interactor | modify | — | 🟡 | 🔵 |

## Commands

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| BatchPlanCheckCommand | command | add | — | 🟡 | 🔵 |

