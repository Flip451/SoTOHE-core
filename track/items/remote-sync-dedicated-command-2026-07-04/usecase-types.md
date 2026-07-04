<!-- Generated from usecase-types.json — DO NOT EDIT DIRECTLY -->

## Value Objects

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| DiagnosticText | value_object | add | — | 🔵 | 🔵 |

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| BranchReadError | error_type | reference | ReadFailed | 🔵 | 🔵 |
| GitWorkflowError | error_type | modify | Validation, NoBranch, DetachedHead, BranchMismatch, Message, Unavailable, SyncUpstreamNotSet, SyncNonFastForward, SyncWorktreeUnresolved, Fs, SwitchFailed | 🔵 | 🔵 |
| TelemetryAggregateServiceError | error_type | reference | ReportUnavailable, EmitUnavailable | 🔵 | 🔵 |

## Secondary Ports

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| ArchivedTelemetryFactoryPort | secondary_port | add | fn build(&self, telemetry_dir: &std::path::Path) -> std::sync::Arc<dyn ArchivedTrackTelemetryPort> | 🔵 | 🔵 |
| ArchivedTrackTelemetryPort | secondary_port | reference | — | 🔵 | 🔵 |
| BranchReaderPort | secondary_port | reference | fn current_branch(&self) -> Result<Option<String>, BranchReadError> | 🔵 | 🔵 |
| GitPrimitivePort | secondary_port | add | fn current_branch(&self, project_root: Option<&std::path::Path>) -> Result<Option<String>, GitWorkflowError>, fn sync_current_branch(&self, project_root: Option<&std::path::Path>) -> Result<(), GitWorkflowError>, fn switch_branch(&self, project_root: Option<&std::path::Path>, branch: &str) -> Result<(), GitWorkflowError>, fn create_branch(&self, project_root: Option<&std::path::Path>, new_branch: &str, base_branch: &str) -> Result<(), GitWorkflowError>, fn branch_exists(&self, project_root: Option<&std::path::Path>, branch: &str) -> Result<bool, GitWorkflowError>, fn move_path(&self, project_root: Option<&std::path::Path>, src: &std::path::Path, dst: &std::path::Path) -> Result<(), GitWorkflowError>, fn fetch_branch(&self, project_root: Option<&std::path::Path>, branch: &str) -> Result<(), GitWorkflowError>, fn show_file_at_ref(&self, project_root: Option<&std::path::Path>, git_ref: &str, path: &std::path::Path) -> Result<String, GitWorkflowError>, fn resolve_commit(&self, project_root: Option<&std::path::Path>, rev: &str) -> Result<Option<domain::CommitHash>, GitWorkflowError>, fn resolve_repo_root(&self, project_root: Option<&std::path::Path>) -> Result<std::path::PathBuf, GitWorkflowError>, fn stage_all(&self, project_root: Option<&std::path::Path>) -> Result<(), GitWorkflowError>, fn stage_from_file(&self, project_root: Option<&std::path::Path>, path: &std::path::Path, cleanup: bool) -> Result<(), GitWorkflowError>, fn commit_from_message_file(&self, project_root: Option<&std::path::Path>, path: &std::path::Path, cleanup: bool) -> Result<(), GitWorkflowError>, fn note_from_file(&self, project_root: Option<&std::path::Path>, path: &std::path::Path, cleanup: bool) -> Result<(), GitWorkflowError>, fn unstage(&self, project_root: Option<&std::path::Path>, paths: &[std::path::PathBuf]) -> Result<(), GitWorkflowError>, fn read_explicit_track_branch(&self, project_root: Option<&std::path::Path>, track_dir: &std::path::Path) -> Result<ExplicitTrackBranch, GitWorkflowError>, fn collect_track_branch_claims(&self, project_root: Option<&std::path::Path>) -> Result<Vec<TrackBranchClaim>, GitWorkflowError> | 🔵 | 🔵 |
| TelemetryReportPort | secondary_port | reference | — | 🔵 | 🔵 |
| TrackArchiveFsPort | secondary_port | add | fn path_is_dir(&self, path: &std::path::Path) -> Result<bool, GitWorkflowError>, fn path_exists(&self, path: &std::path::Path) -> Result<bool, GitWorkflowError>, fn create_dir_all(&self, path: &std::path::Path) -> Result<(), GitWorkflowError>, fn rename_path(&self, src: &std::path::Path, dst: &std::path::Path) -> Result<(), GitWorkflowError>, fn list_dir_file_names(&self, path: &std::path::Path) -> Result<Vec<std::path::PathBuf>, GitWorkflowError>, fn remove_dir(&self, path: &std::path::Path) -> Result<(), GitWorkflowError> | 🔵 | 🔵 |

## Application Services

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| GitWorkflowService | application_service | modify | fn stage_all(&self) -> Result<(), GitWorkflowError>, fn stage_from_file(&self, path: &std::path::Path, cleanup: bool) -> Result<(), GitWorkflowError>, fn commit_from_file(&self, path: &std::path::Path, cleanup: bool, track_dir: Option<&std::path::Path>) -> Result<(), GitWorkflowError>, fn note_from_file(&self, path: &std::path::Path, cleanup: bool) -> Result<(), GitWorkflowError>, fn unstage(&self, paths: &[std::path::PathBuf]) -> Result<(), GitWorkflowError>, fn current_branch_track_id(&self) -> Result<Option<domain::TrackId>, GitWorkflowError>, fn sync_current_branch(&self) -> Result<(), GitWorkflowError> | 🔵 | 🔵 |
| TelemetryAggregateService | application_service | reference | — | 🔵 | 🔵 |

## Use Cases

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| PrGitInteractor | use_case | add | — | 🔵 | 🔵 |
| ReviewGitInteractor | use_case | add | — | 🔵 | 🔵 |
| TelemetryEmitInteractor | use_case | add | — | 🔵 | 🔵 |
| TelemetryReportInteractor | use_case | add | — | 🔵 | 🔵 |
| TrackGitInteractor | use_case | add | — | 🔵 | 🔵 |

## Interactors

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| GitWorkflowInteractor | interactor | modify | — | 🔵 | 🔵 |
| TelemetryAggregateInteractor | interactor | add | — | 🔵 | 🔵 |

## DTOs

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| ExplicitTrackBranch | dto | reference | — | 🔵 | 🔵 |
| TelemetryReportOutput | dto | reference | — | 🔵 | 🔵 |
| TrackBranchClaim | dto | reference | — | 🔵 | 🔵 |

## Free Functions

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| usecase::git_workflow::validate_stage_path_entries | free_function | reference | fn(entries: I) -> Result<Vec<String>, GitWorkflowError> | 🔵 | 🔵 |
| usecase::git_workflow::verify_auto_detected_branch | free_function | reference | fn(current_branch: Option<&str>, claims: &[TrackBranchClaim]) -> Result<(), GitWorkflowError> | 🔵 | 🔵 |
| usecase::git_workflow::verify_explicit_track_branch | free_function | reference | fn(current_branch: Option<&str>, explicit_track: &ExplicitTrackBranch) -> Result<(), GitWorkflowError> | 🔵 | 🔵 |

