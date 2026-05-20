<!-- Generated from usecase-types.json — DO NOT EDIT DIRECTLY -->

## Application Services

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| ReviewCheckApprovedService | application_service | reference | fn check_approved(&self, track_id: String, items_dir: std::path::PathBuf) -> Result<ReviewApprovalOutput, ReviewCheckApprovedError> | 🔵 | 🔵 |
| TaskOperationService | application_service | reference | fn transition_task(&self, cmd: TaskTransitionCommand) -> Result<TaskOperationOutput, TaskOperationError>, fn add_task(&self, cmd: AddTaskCommand) -> Result<TaskOperationOutput, TaskOperationError>, fn set_override(&self, cmd: SetOverrideCommand) -> Result<TaskOperationOutput, TaskOperationError>, fn clear_override(&self, cmd: ClearOverrideCommand) -> Result<TaskOperationOutput, TaskOperationError> | 🔵 | 🔵 |

## Interactors

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| ReviewCheckApprovedInteractor | interactor | modify | — | 🔵 | 🔵 |
| TaskOperationInteractor | interactor | modify | — | 🔵 | 🔵 |

