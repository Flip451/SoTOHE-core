<!-- Generated from usecase-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| BaseMergeAttemptOutcome | enum | add | Clean, Conflicted | 🔵 | 🔵 |
| BaseMergeOutcome | enum | add | Completed, Conflicted | 🔵 | 🔵 |
| GitStashPushOutcome | enum | add | Created, NothingToStash | 🔵 | 🔵 |

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| BaseMergeContextError | error_type | add | Unavailable, ActiveTrackMismatch | 🔵 | 🔵 |
| BaseMergeError | error_type | add | Context, ActiveTrackMismatch, Git, DirtyWorktree, PostMergeCleanup, ConflictedCleanupFailed | 🔵 | 🔵 |
| BaseMergeGitError | error_type | add | Execution, DirtyWorktree | 🔵 | 🔵 |
| BaselineReplacementError | error_type | add | Isolation, Generation, Validation, Publish, Restoration | 🔵 | 🔵 |
| GitStashPopError | error_type | add | ForbiddenBranchRefUpdate, NoPendingGuardedStash, Unavailable, StashIdentityMissing | 🔵 | 🔵 |
| GitStashPushError | error_type | add | ForbiddenBranchRefUpdate, PendingGuardedStashExists, Unavailable | 🔵 | 🔵 |
| PostMergeCleanupError | error_type | add | Views, Baseline, SyncBaseStamp | 🔵 | 🔵 |
| SyncBaseRecordError | error_type | add | Generation, Validation, Write, Replacement | 🔵 | 🔵 |
| TypeSignalsError | error_type | modify | BranchTrackMismatch, LayerBindingsLoad, NoLayers, FeatureDeclaration, AuthoritativeInputFailed, EvaluationFailed, CacheWriteFailed, InconsistentRequest | 🔵 | 🔵 |
| TypeSignalsExecutionError | error_type | modify | AuthoritativeInput, Evaluation, CacheWrite | 🔵 | 🔵 |
| ViewsRegenerationError | error_type | add | Regeneration | 🔵 | 🔵 |

## Secondary Ports

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| BaseMergeCleanupPort | secondary_port | add | fn regenerate_views(&self, request: &BaseMergeCleanupRequest) -> Result<(), ViewsRegenerationError>, fn replace_baselines(&self, request: &BaseMergeCleanupRequest) -> Result<(), BaselineReplacementError>, fn write_sync_base_record(&self, request: &BaseMergeCleanupRequest) -> Result<(), SyncBaseRecordError> | 🔵 | 🔵 |
| BaseMergeContextPort | secondary_port | add | fn load_direction(&self, workspace_root: &std::path::Path) -> Result<domain::branch_strategy::BaseMergeDirection, BaseMergeContextError> | 🔵 | 🔵 |
| BaseMergeGitPort | secondary_port | add | fn ensure_worktree_clean(&self, workspace_root: &std::path::Path) -> Result<(), BaseMergeGitError>, fn merge_base(&self, workspace_root: &std::path::Path, direction: &domain::branch_strategy::BaseMergeDirection) -> Result<BaseMergeAttemptOutcome, BaseMergeGitError> | 🔵 | 🔵 |
| GitStashPort | secondary_port | add | fn push(&self) -> Result<GitStashPushOutcome, GitStashPushError>, fn pop(&self) -> Result<(), GitStashPopError> | 🔵 | 🔵 |
| TrackBlobReader | secondary_port | modify | fn read_spec_document(&self, branch: &str, track_id: &str) -> BlobFetchResult<domain::spec::SpecDocument>, fn read_type_catalogue(&self, branch: &str, track_id: &str, layer_id: &str) -> BlobFetchResult<(Vec<u8>, String)>, fn read_impl_plan(&self, branch: &str, track_id: &str) -> BlobFetchResult<domain::ImplPlanDocument>, fn read_enabled_layers(&self, _branch: &str) -> BlobFetchResult<Vec<String>>, fn read_catalogue_for_spec_ref_check(&self, _branch: &str, _track_id: &str, _layer_id: &str) -> BlobFetchResult<(domain::tddd::catalogue_v2::CatalogueDocument, String, std::collections::HashMap<String, domain::ContentHash>)>, fn read_catalogue_spec_signals_document(&self, _branch: &str, _track_id: &str, _layer_id: &str) -> BlobFetchResult<domain::CatalogueSpecSignalsDocument>, fn read_catalogue_spec_signal_opted_in_layers(&self, _branch: &str) -> BlobFetchResult<Vec<String>>, fn read_type_signals(&self, _branch: &str, _track_id: &str, _layer_id: &str) -> BlobFetchResult<domain::TypeSignalsDocument>, fn read_adr_verify_report(&self, _branch: &str) -> BlobFetchResult<domain::AdrVerifyReport> | 🔵 | 🔵 |
| TypeSignalsExecutorPort | secondary_port | reference | fn evaluate_layer(&self, items_dir: &std::path::Path, track_id: &domain::TrackId, workspace_root: &std::path::Path, binding: &domain::tddd::catalogue_v2::TdddLayerBinding, features: &[domain::tddd::CargoFeatureName]) -> Result<(), TypeSignalsExecutionError> | 🔵 | 🔵 |

## Application Services

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| BaseMergeService | application_service | add | fn execute(&self, command: BaseMergeCommand) -> Result<BaseMergeOutcome, BaseMergeError> | 🔵 | 🔵 |
| GitStashService | application_service | add | fn push(&self) -> Result<GitStashPushOutcome, GitStashPushError>, fn pop(&self) -> Result<(), GitStashPopError> | 🔵 | 🔵 |

## Interactors

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| BaseMergeInteractor | interactor | add | — | 🔵 | 🔵 |
| GitStashInteractor | interactor | add | — | 🔵 | 🔵 |
| TypeSignalsInteractor | interactor | reference | — | 🔵 | 🔵 |

## DTOs

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| BaseMergeCleanupRequest | dto | add | — | 🔵 | 🔵 |

## Commands

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| BaseMergeCommand | command | add | — | 🔵 | 🔵 |

