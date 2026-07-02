<!-- Generated from usecase-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| DryCheckApprovedOutcome | enum | add | Approved, Blocked, Failure | 🔵 | 🔵 |
| DryWriteOutcome | enum | add | Success, Failure | 🔵 | 🔵 |
| FixpointResolveDriverOutcome | enum | add | RunDfp, RunRfp, RunRefVerify, Commit, Failure | 🔵 | 🔵 |

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| DryCheckConfigLoaderError | error_type | add | Unavailable | 🔵 | 🔵 |
| FixpointWorkspaceContextError | error_type | add | Unavailable | 🔵 | 🔵 |

## Secondary Ports

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| BranchStrategyPort | secondary_port | add | fn base_branch(&self) -> &str, fn merge_target(&self) -> &str, fn merge_method(&self) -> domain::branch_strategy::MergeMethod, fn track_prefix(&self) -> &str | 🔵 | 🔵 |
| DiffBaseResolverPort | secondary_port | reference | fn resolve_diff_base(&self, track_dir: &std::path::Path, canonical_root: &std::path::Path, repo_root: &std::path::Path) -> Result<domain::CommitHash, DiffBaseResolverError> | 🔵 | 🔵 |
| DryApprovalFactoryPort | secondary_port | reference | fn build_approval(&self, track_dir: &std::path::Path, canonical_root: &std::path::Path, dry_config: DryCheckConfig, config_fingerprint: domain::dry_check::DryCheckConfigFingerprint, corpus_fingerprint: domain::dry_check::DryCheckCorpusFingerprint) -> std::sync::Arc<dyn DryCheckApprovalService + Send + Sync> | 🔵 | 🔵 |
| DryCheckConfigLoaderPort | secondary_port | add | fn load(&self, repo_root: &std::path::Path) -> Result<(DryCheckConfig, domain::dry_check::DryCheckConfigFingerprint), DryCheckConfigLoaderError> | 🔵 | 🔵 |
| DryDriverPort | secondary_port | modify | fn dry_write(&self, input: DryWriteDriverInput) -> DryWriteOutcome, fn dry_results(&self, input: DryResultsDriverInput) -> DryDriverOutcome, fn dry_check_approved(&self, input: DryCheckApprovedDriverInput) -> DryCheckApprovedOutcome, fn dry_fix_local(&self, input: DryFixLocalDriverInput) -> DryDriverOutcome | 🔵 | 🔵 |
| FixpointDryGateFactoryPort | secondary_port | add | fn build(&self, base_branch: &str) -> std::sync::Arc<dyn FixpointDryGateService> | 🔵 | 🔵 |
| FixpointGateStateFactoryPort | secondary_port | add | fn build_review_gate(&self, items_dir: &std::path::Path, base_branch: &str) -> std::sync::Arc<dyn ReviewGateStatePort>, fn build_ref_verify_gate(&self, items_dir: &std::path::Path) -> std::sync::Arc<dyn RefVerifyGateStatePort> | 🔵 | 🔵 |
| FixpointWorkspaceContextPort | secondary_port | add | fn resolve_context(&self, items_dir: &std::path::Path, track_id: &domain::TrackId) -> Result<FixpointWorkspaceContext, FixpointWorkspaceContextError> | 🔵 | 🔵 |
| RefVerifyGateStatePort | secondary_port | reference | fn ref_verify_status(&self, track_id: &domain::TrackId) -> Result<RefVerifyGateStatus, FixpointResolveError> | 🔵 | 🔵 |
| ReviewGateStatePort | secondary_port | reference | fn review_status(&self, track_id: &domain::TrackId) -> Result<ReviewGateStatus, FixpointResolveError> | 🔵 | 🔵 |

## Application Services

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| DryDriverService | application_service | modify | fn dry_write(&self, input: DryWriteDriverInput) -> DryWriteOutcome, fn dry_results(&self, input: DryResultsDriverInput) -> DryDriverOutcome, fn dry_check_approved(&self, input: DryCheckApprovedDriverInput) -> DryCheckApprovedOutcome, fn dry_fix_local(&self, input: DryFixLocalDriverInput) -> DryDriverOutcome | 🔵 | 🔵 |
| FixpointDryGateService | application_service | reference | fn resolve_dry_gate(&self, cmd: FixpointDryGateCommand) -> Result<FixpointDryGateOutput, D4OrchestrationError> | 🔵 | 🔵 |
| FixpointResolveDriverService | application_service | add | fn fixpoint_resolve(&self, input: FixpointResolveDriverInput) -> FixpointResolveDriverOutcome | 🔵 | 🔵 |
| TrackService | application_service | modify | fn init(&self, items_dir: std::path::PathBuf, track_id: String, description: String) -> TrackCommandOutput, fn transition(&self, items_dir: std::path::PathBuf, track_id: Option<String>, task_id: String, target_status: String, commit_hash: Option<String>) -> TrackCommandOutput, fn resolve(&self, items_dir: std::path::PathBuf, track_id: Option<String>) -> TrackCommandOutput, fn branch_create(&self, items_dir: std::path::PathBuf, track_id: String) -> TrackCommandOutput, fn branch_switch(&self, items_dir: std::path::PathBuf, track_id: String) -> TrackCommandOutput, fn views_validate(&self, project_root: std::path::PathBuf) -> TrackCommandOutput, fn views_sync(&self, project_root: std::path::PathBuf, track_id: Option<String>) -> TrackCommandOutput, fn add_task(&self, items_dir: std::path::PathBuf, track_id: Option<String>, description: String, section: Option<String>, after: Option<String>) -> TrackCommandOutput, fn set_override(&self, items_dir: std::path::PathBuf, track_id: Option<String>, status: String, reason: String) -> TrackCommandOutput, fn clear_override(&self, items_dir: std::path::PathBuf, track_id: Option<String>) -> TrackCommandOutput, fn next_task(&self, items_dir: std::path::PathBuf, track_id: Option<String>) -> TrackCommandOutput, fn task_counts(&self, items_dir: std::path::PathBuf, track_id: Option<String>) -> TrackCommandOutput, fn archive(&self, items_dir: std::path::PathBuf, track_id: String) -> TrackCommandOutput, fn detect_active(&self, project_root: std::path::PathBuf) -> TrackCommandOutput, fn switch_base(&self, project_root: std::path::PathBuf) -> TrackCommandOutput | 🔵 | 🔵 |

## Interactors

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| DryDriverInteractor | interactor | reference | — | 🔵 | 🔵 |
| FixpointResolveDriverInteractor | interactor | add | — | 🔵 | 🔵 |

## DTOs

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| DryWriteFindingSummary | dto | add | — | 🔵 | 🔵 |
| FixpointResolveDriverInput | dto | add | — | 🔵 | 🔵 |
| FixpointWorkspaceContext | dto | add | — | 🔵 | 🔵 |

