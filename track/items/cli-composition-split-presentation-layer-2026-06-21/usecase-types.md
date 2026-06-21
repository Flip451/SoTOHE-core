<!-- Generated from usecase-types.json — DO NOT EDIT DIRECTLY -->

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| ArchivedTrackTelemetryError | error_type | — | Io, Serialize | 🔵 | 🔵 |
| D4OrchestrationError | error_type | — | DiffFragment, DryGate, PrPolling | 🟡 | 🔵 |
| SignalGateError | error_type | — | ChainExecutionFailed, InvalidTrackId, StrictnessConfigLoad | 🟡 | 🔵 |
| SpecAdrSignalError | error_type | — | Read, Decode, Encode, Write | 🟡 | 🔵 |

## Secondary Ports

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| ArchivedTrackTelemetryPort | secondary_port | — | fn emit(&self, subcommand: String) -> Result<(), ArchivedTrackTelemetryError> | 🔵 | 🔵 |
| BranchReaderPort | secondary_port | reference | fn current_branch(&self) -> Result<Option<String>, BranchReadError> | 🔵 | 🔵 |
| DryCheckAgentPort | secondary_port | reference | fn judge(&self, tier: DryCheckJudgeTier, low_path: std::path::PathBuf, high_path: std::path::PathBuf) -> Result<DryCheckAgentJudgment, DryCheckAgentError> | 🔵 | 🔵 |
| DryCheckApprovalService | secondary_port | reference | fn check_approved(&self, current_refs: std::collections::BTreeSet<domain::dry_check::FragmentRef>, approval: domain::dry_check::DryCheckApprovalVerdict) -> Result<DryCheckApprovalVerdict, DryCheckCycleError> | 🔵 | 🔵 |
| RefVerifyGateStatePort | secondary_port | reference | fn ref_verify_status(&self) -> Result<RefVerifyGateStatus, FixpointResolveError> | 🔵 | 🔵 |
| ReviewGateStatePort | secondary_port | reference | fn review_status(&self) -> Result<ReviewGateStatus, FixpointResolveError> | 🔵 | 🔵 |
| SemanticIndexPort | secondary_port | reference | fn insert(&self, path: std::path::PathBuf, content: String) -> Result<(), SemanticIndexError>, fn insert_batch(&self, items: Vec<(std::path::PathBuf, String)>) -> Result<(), SemanticIndexError>, fn delete_by_source_path(&self, path: std::path::PathBuf) -> Result<(), SemanticIndexError>, fn search(&self, query: String, limit: usize) -> Result<Vec<domain::semantic_dup::SearchResult>, SemanticIndexError> | 🔵 | 🔵 |
| SpecFileWriterPort | secondary_port | — | fn read_spec_json(&self, path: std::path::PathBuf) -> Result<String, SpecAdrSignalError>, fn write_spec_json(&self, path: std::path::PathBuf, content: String) -> Result<(), SpecAdrSignalError> | 🟡 | 🔵 |

## Application Services

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| ArchivedTrackTelemetryService | application_service | — | fn emit(&self, cmd: ArchivedTrackTelemetryCommand) -> Result<(), ArchivedTrackTelemetryError> | 🔵 | 🔵 |
| DryFragmentPipelineService | application_service | — | fn derive_current_refs(&self, cmd: DryFragmentPipelineCommand) -> Result<DryFragmentPipelineOutput, D4OrchestrationError> | 🟡 | 🔵 |
| FixpointDryGateService | application_service | — | fn resolve_dry_gate(&self, cmd: FixpointDryGateCommand) -> Result<FixpointDryGateOutput, D4OrchestrationError> | 🟡 | 🔵 |
| PrReviewPollingService | application_service | — | fn poll(&self, cmd: PrReviewPollingCommand) -> Result<PrReviewPollingOutput, D4OrchestrationError> | 🟡 | 🔵 |
| SignalGateService | application_service | — | fn run_gate(&self, cmd: SignalGateCommand) -> Result<SignalGateOutput, SignalGateError> | 🟡 | 🔵 |
| SpecAdrSignalService | application_service | — | fn calc_and_persist(&self, cmd: SpecAdrSignalCommand) -> Result<SpecAdrSignalOutput, SpecAdrSignalError> | 🟡 | 🔵 |

## Interactors

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| ArchivedTrackTelemetryInteractor | interactor | — | — | 🔵 | 🔵 |
| DryFragmentPipelineInteractor | interactor | — | — | 🟡 | 🔵 |
| FixpointDryGateInteractor | interactor | — | — | 🟡 | 🔵 |
| PrReviewPollingInteractor | interactor | — | — | 🟡 | 🔵 |
| SignalGateInteractor | interactor | — | — | 🟡 | 🔵 |
| SpecAdrSignalInteractor | interactor | — | — | 🟡 | 🔵 |

## DTOs

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| DryFragmentPipelineOutput | dto | — | — | 🟡 | 🔵 |
| FixpointDryGateOutput | dto | — | — | 🟡 | 🔵 |
| PrReviewPollingOutput | dto | — | — | 🟡 | 🔵 |
| SignalChainOutput | dto | — | — | 🟡 | 🔵 |
| SignalGateOutput | dto | — | — | 🟡 | 🔵 |
| SpecAdrSignalOutput | dto | — | — | 🟡 | 🔵 |

## Commands

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| ArchivedTrackTelemetryCommand | command | — | — | 🔵 | 🔵 |
| DryFragmentPipelineCommand | command | — | — | 🟡 | 🔵 |
| FixpointDryGateCommand | command | — | — | 🟡 | 🔵 |
| PrReviewPollingCommand | command | — | — | 🟡 | 🔵 |
| SignalGateCommand | command | — | — | 🟡 | 🔵 |
| SpecAdrSignalCommand | command | — | — | 🟡 | 🔵 |

