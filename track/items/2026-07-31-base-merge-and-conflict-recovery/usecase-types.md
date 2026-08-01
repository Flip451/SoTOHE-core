<!-- Generated from usecase-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| BaseMergeAttemptOutcome | enum | add | Clean, Conflicted | 🟡 | 🔵 |
| BaseMergeOutcome | enum | add | Completed, Conflicted | 🟡 | 🔵 |
| GitStashCommand | enum | add | Push, Pop | 🟡 | 🔵 |
| PostMergeCleanupStage | enum | add | Views, Baseline, SyncBaseStamp | 🟡 | 🔵 |

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| BaseMergeContextError | error_type | add | Unavailable, ActiveTrackMismatch | 🟡 | 🔵 |
| BaseMergeError | error_type | add | Context, ActiveTrackMismatch, Git, PostMergeCleanup | 🟡 | 🔵 |
| BaseMergeGitError | error_type | add | Execution | 🟡 | 🔵 |
| GitStashError | error_type | add | ForbiddenBranchRefUpdate, Unavailable | 🟡 | 🔵 |
| TypeSignalsError | error_type | modify | BranchTrackMismatch, LayerBindingsLoad, NoLayers, FeatureDeclaration, AuthoritativeInputFailed, EvaluationFailed, CacheWriteFailed, InconsistentRequest | 🔵 | 🔵 |
| TypeSignalsExecutionError | error_type | modify | AuthoritativeInput, Evaluation, CacheWrite | 🔵 | 🔵 |

## Secondary Ports

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| BaseMergeCleanupPort | secondary_port | add | fn regenerate_views(&self, workspace_root: &std::path::Path, track_id: &domain::TrackId) -> Result<(), DiagnosticText>, fn recapture_baselines(&self, workspace_root: &std::path::Path, track_id: &domain::TrackId) -> Result<(), DiagnosticText>, fn record_sync_base_stamp(&self, workspace_root: &std::path::Path, track_id: &domain::TrackId) -> Result<(), DiagnosticText> | 🟡 | 🔵 |
| BaseMergeContextPort | secondary_port | add | fn load_direction(&self, workspace_root: &std::path::Path) -> Result<domain::branch_strategy::BaseMergeDirection, BaseMergeContextError> | 🟡 | 🔵 |
| BaseMergeGitPort | secondary_port | add | fn merge_base(&self, workspace_root: &std::path::Path, direction: &domain::branch_strategy::BaseMergeDirection) -> Result<BaseMergeAttemptOutcome, BaseMergeGitError> | 🟡 | 🔵 |
| GitStashPort | secondary_port | add | fn execute(&self, command: GitStashCommand) -> Result<(), GitStashError> | 🟡 | 🔵 |
| TypeSignalsExecutorPort | secondary_port | reference | fn evaluate_layer(&self, items_dir: &std::path::Path, track_id: &domain::TrackId, workspace_root: &std::path::Path, binding: &domain::tddd::catalogue_v2::TdddLayerBinding, features: &[domain::tddd::CargoFeatureName]) -> Result<(), TypeSignalsExecutionError> | 🔵 | 🔵 |

## Application Services

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| BaseMergeService | application_service | add | fn execute(&self, command: BaseMergeCommand) -> Result<BaseMergeOutcome, BaseMergeError> | 🟡 | 🔵 |
| GitStashService | application_service | add | fn execute(&self, command: GitStashCommand) -> Result<(), GitStashError> | 🟡 | 🔵 |

## Interactors

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| BaseMergeInteractor | interactor | add | — | 🟡 | 🔵 |
| GitStashInteractor | interactor | add | — | 🟡 | 🔵 |
| TypeSignalsInteractor | interactor | reference | — | 🔵 | 🔵 |

## Commands

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| BaseMergeCommand | command | add | — | 🟡 | 🔵 |

