<!-- Generated from infrastructure-types.json — DO NOT EDIT DIRECTLY -->

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| DemoRunError | error_type | — | Unavailable | 🔵 | 🔵 |
| PersistentIndexLockError | error_type | — | LockFailed | 🔵 | 🔵 |
| TrackBranchError | error_type | — | LoadFailed | 🔵 | 🔵 |

## Secondary Adapters

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CodeFragmentExtractorAdapter | secondary_adapter | — | impl CodeFragmentExtractorPort, impl Default, impl Debug | 🔵 | 🔵 |
| FsArchAdapter | secondary_adapter | — | impl ArchPort, impl Default | 🟡 | 🔵 |
| FsArchivedTrackTelemetryAdapter | secondary_adapter | — | impl ArchivedTrackTelemetryPort | 🔵 | 🔵 |
| FsConventionsAdapter | secondary_adapter | — | impl ConventionsPort, impl Default | 🟡 | 🔵 |
| FsDemoAdapter | secondary_adapter | — | impl DemoPort, impl Default | 🟡 | 🔵 |
| FsDryCorpusMetaAdapter | secondary_adapter | — | impl DryCorpusMetaPort | 🔵 | 🔵 |
| FsFileWriteAdapter | secondary_adapter | — | impl FileWritePort, impl Default | 🟡 | 🔵 |
| FsGitWorkflowAdapter | secondary_adapter | — | impl GitWorkflowService, impl Default | 🟡 | 🔵 |
| FsRefVerifyAggregateAdapter | secondary_adapter | — | impl RefVerifyAggregateService, impl Default | 🟡 | 🔵 |
| FsRefVerifyCheckApprovedAdapter | secondary_adapter | — | impl RefVerifyCheckApprovedDriverService, impl Default | 🟡 | 🔵 |
| FsRefVerifyGateStateAdapter | secondary_adapter | — | impl RefVerifyGateStatePort | 🔵 | 🔵 |
| FsRefVerifyRunAdapter | secondary_adapter | — | impl RefVerifyRunService, impl Default | 🟡 | 🔵 |
| FsReviewGateStateAdapter | secondary_adapter | — | impl ReviewGateStatePort | 🔵 | 🔵 |
| FsSpecFileWriterAdapter | secondary_adapter | — | impl SpecFileWriterPort, impl Default | 🔵 | 🔵 |
| FsTelemetryEmitDynamicAdapter | secondary_adapter | — | impl TelemetryEmitDynamicPort, impl Default | 🟡 | 🔵 |
| FsTelemetryReportAdapter | secondary_adapter | — | impl TelemetryReportPort, impl Default | 🟡 | 🔵 |
| FsVerifyAdapter | secondary_adapter | — | impl VerifyPort, impl Default, impl Debug | 🟡 | 🔵 |
| GitDryCheckDiffGetter | secondary_adapter | modify | impl DryCheckDiffSource, impl Debug | 🔵 | 🔵 |
| NoOpDryApprovalService | secondary_adapter | — | impl DryCheckApprovalService | 🔵 | 🔵 |
| NoopSemanticIndexPort | secondary_adapter | — | impl SemanticIndexPort | 🔵 | 🔵 |
| NullInsertIndexProxy | secondary_adapter | — | impl SemanticIndexPort | 🔵 | 🔵 |
| RecordingDryAgent | secondary_adapter | — | impl DryCheckAgentPort | 🔵 | 🔵 |
| SystemSleepAdapter | secondary_adapter | — | impl SleepPort, impl Debug, impl Clone, impl Copy, impl Default | 🔵 | 🔵 |

## Free Functions

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| infrastructure::demo::run_example_demo | free_function | modify | fn() -> Result<String, DemoRunError> | 🔵 | 🔵 |
| infrastructure::git_cli::collect_track_branch_claims | free_function | modify | fn(root: &std::path::Path) -> Result<Vec<TrackBranchRecord>, TrackBranchError> | 🔵 | 🔵 |
| infrastructure::git_cli::load_explicit_track_branch | free_function | modify | fn(root: &std::path::Path, track_dir: &std::path::Path) -> Result<TrackBranchRecord, TrackBranchError> | 🔵 | 🔵 |
| infrastructure::git_cli::load_explicit_track_branch_from_items_dir | free_function | modify | fn(root: &std::path::Path, items_dir: &std::path::Path, track_dir: &std::path::Path) -> Result<TrackBranchRecord, TrackBranchError> | 🔵 | 🔵 |

