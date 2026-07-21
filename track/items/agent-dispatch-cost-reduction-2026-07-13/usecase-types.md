<!-- Generated from usecase-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CapabilityDispatchOutcome | enum | reference | Executed, DelegateInHost | 🔵 | 🔵 |
| CapabilityResumeRequest | enum | add | Fresh, ResumeWithoutTarget, Resume | 🔵 | 🔵 |
| ProviderSessionCacheKey | enum | add | Review, TrackCapability, WorkspaceCapability | 🔵 | 🔵 |
| ReasoningEffort | enum | add | Low, Medium, High, XHigh, Max | 🔵 | 🔵 |

## Value Objects

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CapabilityFailureDetail | value_object | reference | — | 🔵 | 🔵 |
| CapabilityFilePath | value_object | reference | — | 🔵 | 🔵 |
| CapabilityName | value_object | reference | — | 🔵 | 🔵 |
| DiagnosticText | value_object | reference | — | 🔵 | 🔵 |
| ModelName | value_object | reference | — | 🔵 | 🔵 |
| ProviderName | value_object | reference | — | 🔵 | 🔵 |
| ProviderSessionCacheEntry | value_object | add | — | 🔵 | 🔵 |
| ProviderSessionId | value_object | add | — | 🔵 | 🔵 |
| ReviewerPrompt | value_object | add | — | 🔵 | 🔵 |
| SubagentName | value_object | add | — | 🔵 | 🔵 |
| TargetArtifactPath | value_object | add | — | 🔵 | 🔵 |
| TargetArtifactSet | value_object | add | — | 🔵 | 🔵 |
| TimeoutSeconds | value_object | reference | — | 🔵 | 🔵 |

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CapabilityExecError | error_type | modify | ProfileResolution, ExecutionModeRejected, ModelMissing, EffortMissing, UnsupportedProvider, SourceValidation, AdapterPreflight, DispatchFailed | 🔵 | 🔵 |
| CapabilityInputValidationError | error_type | modify | EmptyProviderName, EmptyModelName, EmptyFilePath, InvalidFilePath, EmptyContent, ZeroTimeoutSeconds, EmptyTargetArtifactSet | 🔵 | 🔵 |
| ProviderSessionCacheError | error_type | add | StorageUnavailable, EntryInvalid, IdentityBoundaryViolation | 🔵 | 🔵 |
| RunReviewFixError | error_type | modify | InvalidScope, InvalidTrackId, InvalidRoundType, SmokeTestFailed, FixRunnerFailed, SubagentDispatchRequired | 🔵 | 🔵 |
| TypeSignalsError | error_type | modify | BranchTrackMismatch, LayerBindingsLoad, NoLayers, EvaluationFailed, InconsistentRequest | 🔵 | 🔵 |
| TypeSignalsExecutionError | error_type | add | — | 🔵 | 🔵 |

## Secondary Ports

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CapabilityProfilePort | secondary_port | reference | fn resolve(&self, capability: &CapabilityName) -> Result<CapabilityProfile, CapabilityExecError> | 🔵 | 🔵 |
| CapabilityProviderPort | secondary_port | reference | fn provider(&self) -> &ProviderName, fn dispatch(&self, request: &CapabilityDispatchRequest) -> Result<CapabilityDispatchOutcome, CapabilityExecError> | 🔵 | 🔵 |
| ProviderSessionCachePort | secondary_port | add | fn load(&self, key: &ProviderSessionCacheKey) -> Result<Option<ProviderSessionCacheEntry>, ProviderSessionCacheError>, fn save(&self, key: &ProviderSessionCacheKey, entry: &ProviderSessionCacheEntry) -> Result<(), ProviderSessionCacheError>, fn remove(&self, key: &ProviderSessionCacheKey) -> Result<(), ProviderSessionCacheError> | 🔵 | 🔵 |
| ReviewFixRunner | secondary_port | reference | fn run_fix(&self, command: RunReviewFixCommand) -> Result<RunReviewFixOutput, ReviewFixRunnerError> | 🔵 | 🔵 |
| Reviewer | secondary_port | reference | fn review(&self, target: &domain::review_v2::ReviewTarget) -> Result<(domain::review_v2::Verdict, domain::review_v2::LogInfo), domain::review_v2::ReviewerError>, fn fast_review(&self, target: &domain::review_v2::ReviewTarget) -> Result<(domain::review_v2::FastVerdict, domain::review_v2::LogInfo), domain::review_v2::ReviewerError> | 🔵 | 🔵 |
| SchemaExporterPort | secondary_port | reference | fn export_as_json(&self, crate_name: &str) -> Result<String, SchemaExporterError> | 🔵 | 🔵 |
| TypeSignalsExecutorPort | secondary_port | add | fn evaluate_layer(&self, items_dir: &std::path::Path, track_id: &domain::ids::TrackId, workspace_root: &std::path::Path, binding: &domain::tddd::catalogue_v2::TdddLayerBinding) -> Result<(), TypeSignalsExecutionError> | 🔵 | 🔵 |

## Application Services

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| ReviewService | application_service | modify | fn run_codex(&self, input: ReviewRunInput) -> Result<RunReviewOutput, RunReviewError>, fn run_claude(&self, input: ReviewRunInput) -> Result<RunReviewOutput, RunReviewError>, fn run_local(&self, model: Option<String>, timeout_seconds: u64, briefing_file: Option<std::path::PathBuf>, prompt: Option<String>, track_id: Option<String>, round_type: String, group: String, items_dir: std::path::PathBuf) -> ReviewRunLocalOutput, fn check_approved(&self, track_id: String, items_dir: std::path::PathBuf) -> Result<ReviewApprovalOutput, ReviewCheckApprovedError>, fn results(&self, track_id: Option<String>, items_dir: std::path::PathBuf, scope: Option<String>, all: bool, limit: u32, round_type: String, no_hint: bool) -> Result<String, ReviewAuxError>, fn classify(&self, paths: Vec<String>, track_id: Option<String>, items_dir: std::path::PathBuf) -> Result<Vec<(String, String)>, ReviewAuxError>, fn files(&self, scope: String, track_id: Option<String>, items_dir: std::path::PathBuf) -> Result<Vec<String>, ReviewAuxError>, fn validate_scope(&self, scope: String, track_id: Option<String>, items_dir: std::path::PathBuf) -> Result<(), ReviewAuxError>, fn get_briefing(&self, scope: String, track_id: Option<String>, items_dir: std::path::PathBuf) -> Result<Option<String>, ReviewAuxError>, fn persist_commit_hash(&self, track_id: String, workspace_root: std::path::PathBuf) -> Result<String, CommitHashPersistenceError> | 🔵 | 🔵 |
| TypeSignalsService | application_service | reference | fn run(&self, request: TypeSignalsRequest) -> Result<(), TypeSignalsError> | 🔵 | 🔵 |

## Interactors

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| TypeSignalsInteractor | interactor | modify | — | 🔵 | 🔵 |

## DTOs

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CapabilityDispatchRequest | dto | reference | — | 🔵 | 🔵 |
| CapabilityProfile | dto | modify | — | 🔵 | 🔵 |
| SubagentDispatchInstruction | dto | add | — | 🔵 | 🔵 |

## Commands

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CapabilityExecRequest | command | modify | — | 🔵 | 🔵 |
| TypeSignalsRequest | command | modify | — | 🔵 | 🔵 |

