<!-- Generated from usecase-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CapabilityDispatchOutcome | enum | reference | Executed, DelegateInHost | 🔵 | 🔵 |
| CapabilityResumeRequest | enum | add | Fresh, ResumeWithoutTarget, Resume | 🟡 | 🔵 |
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
| TargetArtifactPath | value_object | add | — | 🔵 | 🔵 |
| TargetArtifactSet | value_object | add | — | 🔵 | 🔵 |
| TimeoutSeconds | value_object | reference | — | 🔵 | 🔵 |

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CapabilityExecError | error_type | modify | ProfileResolution, ExecutionModeRejected, ModelMissing, EffortMissing, UnsupportedProvider, SourceValidation, AdapterPreflight, DispatchFailed | 🔵 | 🔵 |
| CapabilityInputValidationError | error_type | modify | EmptyProviderName, EmptyModelName, EmptyFilePath, InvalidFilePath, EmptyContent, ZeroTimeoutSeconds, EmptyTargetArtifactSet | 🔵 | 🔵 |
| ProviderSessionCacheError | error_type | add | StorageUnavailable, EntryInvalid, IdentityBoundaryViolation | 🔵 | 🔵 |
| TypeSignalsError | error_type | modify | BranchTrackMismatch, LayerBindingsLoad, NoLayers, EvaluationFailed, InconsistentRequest | 🔵 | 🔵 |
| TypeSignalsExecutionError | error_type | add | — | 🔵 | 🔵 |

## Secondary Ports

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CapabilityProfilePort | secondary_port | reference | fn resolve(&self, capability: &CapabilityName) -> Result<CapabilityProfile, CapabilityExecError> | 🔵 | 🔵 |
| CapabilityProviderPort | secondary_port | reference | fn provider(&self) -> &ProviderName, fn dispatch(&self, request: &CapabilityDispatchRequest) -> Result<CapabilityDispatchOutcome, CapabilityExecError> | 🔵 | 🔵 |
| ProviderSessionCachePort | secondary_port | add | fn load(&self, key: &ProviderSessionCacheKey) -> Result<Option<ProviderSessionCacheEntry>, ProviderSessionCacheError>, fn save(&self, key: &ProviderSessionCacheKey, entry: &ProviderSessionCacheEntry) -> Result<(), ProviderSessionCacheError>, fn remove(&self, key: &ProviderSessionCacheKey) -> Result<(), ProviderSessionCacheError> | 🔵 | 🔵 |
| Reviewer | secondary_port | reference | fn review(&self, target: &domain::review_v2::ReviewTarget) -> Result<(domain::review_v2::Verdict, domain::review_v2::LogInfo), domain::review_v2::ReviewerError>, fn fast_review(&self, target: &domain::review_v2::ReviewTarget) -> Result<(domain::review_v2::FastVerdict, domain::review_v2::LogInfo), domain::review_v2::ReviewerError> | 🔵 | 🔵 |
| SchemaExporterPort | secondary_port | reference | fn export_as_json(&self, crate_name: &str) -> Result<String, SchemaExporterError> | 🔵 | 🔵 |
| TypeSignalsExecutorPort | secondary_port | add | fn evaluate_layer(&self, items_dir: &std::path::Path, track_id: &domain::ids::TrackId, workspace_root: &std::path::Path, binding: &domain::tddd::catalogue_v2::TdddLayerBinding) -> Result<(), TypeSignalsExecutionError> | 🔵 | 🔵 |

## Application Services

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
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

## Commands

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CapabilityExecRequest | command | modify | — | 🟡 | 🔵 |
| TypeSignalsRequest | command | modify | — | 🔵 | 🔵 |

