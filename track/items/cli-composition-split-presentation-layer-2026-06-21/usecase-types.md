<!-- Generated from usecase-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| PrReviewPollingOutput | enum | — | ReviewFound, ZeroFindings, Timeout | 🔵 | 🔵 |

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| ArchivedTrackTelemetryError | error_type | — | Io, Serialize | 🔵 | 🔵 |
| D4OrchestrationError | error_type | — | DiffFragment, DryGate, PrPolling | 🔵 | 🔵 |
| SignalGateError | error_type | — | ChainExecutionFailed, InvalidTrackId, StrictnessConfigLoad | 🔵 | 🔵 |
| SpecAdrSignalError | error_type | — | Read, Decode, Encode, Write | 🔵 | 🔵 |

## Secondary Ports

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| AdrChainRunnerPort | secondary_port | — | fn run_adr_chain(&self, project_root: std::path::PathBuf, strict: bool) -> Result<SignalChainOutput, String> | 🔵 | 🔵 |
| ArchivedTrackTelemetryPort | secondary_port | — | fn emit(&self, track_id: String, subcommand: String, exit_code: i32, duration_ms: u64) -> Result<(), ArchivedTrackTelemetryError> | 🔵 | 🔵 |
| BranchReaderPort | secondary_port | reference | fn current_branch(&self) -> Result<Option<String>, BranchReadError> | 🔵 | 🔵 |
| CodeFragmentExtractorPort | secondary_port | — | fn extract(&self, workspace_root: &std::path::Path) -> Result<Vec<domain::semantic_dup::CodeFragment>, String> | 🔵 | 🔵 |
| DiffBaseResolverPort | secondary_port | — | fn resolve_diff_base(&self, track_dir: &std::path::Path, canonical_root: &std::path::Path, repo_root: &std::path::Path) -> Result<domain::CommitHash, String> | 🔵 | 🔵 |
| DryApprovalFactoryPort | secondary_port | — | fn build_approval(&self, track_dir: &std::path::Path, canonical_root: &std::path::Path, dry_config: usecase::dry_check::DryCheckConfig, config_fingerprint: domain::dry_check::DryCheckConfigFingerprint, corpus_fingerprint: domain::dry_check::DryCheckCorpusFingerprint) -> std::sync::Arc<dyn usecase::dry_check::DryCheckApprovalService + Send + Sync> | 🔵 | 🔵 |
| DryCheckAgentPort | secondary_port | reference | fn judge(&self, tier: DryCheckJudgeTier, low_path: std::path::PathBuf, high_path: std::path::PathBuf) -> Result<DryCheckAgentJudgment, DryCheckAgentError> | 🔵 | 🔵 |
| DryCheckApprovalService | secondary_port | reference | fn check_approved(&self, current_refs: std::collections::BTreeSet<domain::dry_check::FragmentRef>, approval: domain::dry_check::DryCheckApprovalVerdict) -> Result<DryCheckApprovalVerdict, DryCheckCycleError> | 🔵 | 🔵 |
| DryCheckDiffSource | secondary_port | modify | fn list_changed_hunks(&self, base: &domain::CommitHash, repo_root: &std::path::Path) -> Result<Vec<domain::dry_check::DiffFileHunks>, crate::dry_check::errors::DryCheckDiffError> | 🔵 | 🔵 |
| DryCorpusMetaPort | secondary_port | — | fn resolve_corpus_meta(&self, track_dir: &std::path::Path, canonical_root: &std::path::Path, repo_root: &std::path::Path) -> Result<(std::path::PathBuf, domain::dry_check::DryCheckCorpusFingerprint), String> | 🔵 | 🔵 |
| LayerChainRunnerPort | secondary_port | — | fn run_catalog_spec_chain(&self, strict: bool, signal_reader: &dyn usecase::signal::SignalLayerReader) -> Result<SignalChainOutput, String>, fn run_impl_catalog_chain(&self, strict: bool, signal_reader: &dyn usecase::signal::SignalLayerReader) -> Result<SignalChainOutput, String> | 🔵 | 🔵 |
| PrListIssueCommentsPort | secondary_port | — | fn list_issue_comments(&self, repo_nwo: &str, pr: &str) -> Result<String, String> | 🔵 | 🔵 |
| PrListReactionsPort | secondary_port | — | fn list_reactions(&self, repo_nwo: &str, pr: &str) -> Result<String, String> | 🔵 | 🔵 |
| PrListReviewsPort | secondary_port | — | fn list_reviews(&self, repo_nwo: &str, pr: &str) -> Result<String, String> | 🔵 | 🔵 |
| PrRepoNwoPort | secondary_port | — | fn repo_nwo(&self) -> Result<String, String> | 🔵 | 🔵 |
| RefVerifyGateStatePort | secondary_port | reference | fn ref_verify_status(&self) -> Result<RefVerifyGateStatus, FixpointResolveError> | 🔵 | 🔵 |
| ReviewGateStatePort | secondary_port | reference | fn review_status(&self) -> Result<ReviewGateStatus, FixpointResolveError> | 🔵 | 🔵 |
| SemanticIndexPort | secondary_port | reference | fn insert(&self, path: std::path::PathBuf, content: String) -> Result<(), SemanticIndexError>, fn insert_batch(&self, items: Vec<(std::path::PathBuf, String)>) -> Result<(), SemanticIndexError>, fn delete_by_source_path(&self, path: std::path::PathBuf) -> Result<(), SemanticIndexError>, fn search(&self, query: String, limit: usize) -> Result<Vec<domain::semantic_dup::SearchResult>, SemanticIndexError> | 🔵 | 🔵 |
| SleepPort | secondary_port | — | fn sleep(&self, duration: std::time::Duration) -> () | 🔵 | 🔵 |
| SpecAdrChainRunnerPort | secondary_port | — | fn run_spec_adr_chain(&self, spec_json_path: std::path::PathBuf, strict: bool) -> Result<SignalChainOutput, String> | 🔵 | 🔵 |
| SpecFileWriterPort | secondary_port | — | fn read_spec_json(&self, path: std::path::PathBuf) -> Result<domain::SpecDocument, SpecAdrSignalError>, fn write_spec_json(&self, path: std::path::PathBuf, doc: &domain::SpecDocument) -> Result<(), SpecAdrSignalError> | 🔵 | 🔵 |

## Application Services

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| ArchivedTrackTelemetryService | application_service | — | fn emit(&self, cmd: ArchivedTrackTelemetryCommand) -> Result<(), ArchivedTrackTelemetryError> | 🔵 | 🔵 |
| DryFragmentPipelineService | application_service | — | fn derive_current_refs(&self, cmd: DryFragmentPipelineCommand) -> Result<DryFragmentPipelineOutput, D4OrchestrationError> | 🔵 | 🔵 |
| FixpointDryGateService | application_service | — | fn resolve_dry_gate(&self, cmd: FixpointDryGateCommand) -> Result<FixpointDryGateOutput, D4OrchestrationError> | 🔵 | 🔵 |
| PrReviewPollingService | application_service | — | fn poll(&self, cmd: PrReviewPollingCommand) -> Result<PrReviewPollingOutput, D4OrchestrationError> | 🔵 | 🔵 |
| SignalGateService | application_service | — | fn run_gate(&self, cmd: SignalGateCommand) -> Result<SignalGateOutput, SignalGateError> | 🔵 | 🔵 |
| SpecAdrSignalService | application_service | — | fn calc_and_persist(&self, cmd: SpecAdrSignalCommand) -> Result<SpecAdrSignalOutput, SpecAdrSignalError> | 🔵 | 🔵 |

## Use Cases

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| FixpointDryGateOutput | use_case | — | — | 🔵 | 🔵 |

## Interactors

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| ArchivedTrackTelemetryInteractor | interactor | — | — | 🔵 | 🔵 |
| DryFragmentPipelineInteractor | interactor | — | — | 🔵 | 🔵 |
| FixpointDryGateInteractor | interactor | — | — | 🔵 | 🔵 |
| PrReviewPollingInteractor | interactor | — | — | 🔵 | 🔵 |
| SignalGateInteractor | interactor | — | — | 🔵 | 🔵 |
| SpecAdrSignalInteractor | interactor | — | — | 🔵 | 🔵 |

## DTOs

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| DryFragmentPipelineOutput | dto | — | — | 🔵 | 🔵 |
| SignalChainOutput | dto | — | — | 🔵 | 🔵 |
| SignalGateOutput | dto | — | — | 🔵 | 🔵 |
| SpecAdrSignalOutput | dto | — | — | 🔵 | 🔵 |

## Commands

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| ArchivedTrackTelemetryCommand | command | — | — | 🔵 | 🔵 |
| DryFragmentPipelineCommand | command | — | — | 🔵 | 🔵 |
| FixpointDryGateCommand | command | — | — | 🔵 | 🔵 |
| PrReviewPollingCommand | command | — | — | 🔵 | 🔵 |
| SignalGateCommand | command | — | — | 🔵 | 🔵 |
| SpecAdrSignalCommand | command | — | — | 🔵 | 🔵 |

