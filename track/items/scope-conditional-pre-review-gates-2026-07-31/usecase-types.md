<!-- Generated from usecase-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| ClassifiedProgramExecutionRecord | enum | add | Succeeded, Failed | 🔵 | 🔵 |
| PhaseCommandEnterOutcome | enum | add | Completed, Blocked | 🔵 | 🔵 |
| PreReviewCommandDispatchOutcome | enum | add | ReadyForReview, Blocked | 🔵 | 🔵 |
| ProgramOutputStream | enum | add | Stdout, Stderr | 🔵 | 🔵 |
| ProgramRunOutcome | enum | add | Exited, TimedOut, OutputLimitExceeded | 🔵 | 🔵 |
| RefVerifyChainFilter | enum | reference | Chain1, Chain2, All | 🔵 | 🔵 |
| ReviewCheckZeroFindingsOutcome | enum | add | CurrentFinalZeroFindings, MissingFinalVerdict, StaleFinalVerdict, FindingsRemain | 🔵 | 🔵 |
| ReviewScopeSelection | enum | add | Named, All | 🟡 | 🔵 |
| ReviewScopeSelector | enum | add | Named, Other | 🔵 | 🔵 |
| ReviewTrackSelector | enum | add | Explicit, CurrentBranch | 🔵 | 🔵 |

## Value Objects

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CommandArgument | value_object | add | — | 🔵 | 🔵 |
| CommandArgv | value_object | add | — | 🔵 | 🔵 |
| CommandConfigSchemaVersion | value_object | add | — | 🔵 | 🔵 |
| CommandDeclarationId | value_object | add | — | 🔵 | 🔵 |
| CommandSequenceIndex | value_object | add | — | 🔵 | 🔵 |
| CommandTimeoutSeconds | value_object | add | — | 🔵 | 🔵 |
| ConfiguredCommand | value_object | add | — | 🔵 | 🔵 |
| DiagnosticText | value_object | reference | — | 🔵 | 🔵 |
| OutputCaptureLimitBytes | value_object | add | — | 🔵 | 🔵 |
| PhaseCommandConfig | value_object | add | — | 🔵 | 🔵 |
| PhaseCommandDeclaration | value_object | add | — | 🔵 | 🔵 |
| PreReviewCommandConfig | value_object | add | — | 🔵 | 🔵 |
| PreReviewScopeCommandDeclaration | value_object | add | — | 🔵 | 🔵 |
| ProgramExitCode | value_object | add | — | 🔵 | 🔵 |
| UnvalidatedTimeoutSeconds | value_object | add | — | 🔵 | 🔵 |

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CommandArgvValidationError | error_type | add | Empty, RecursiveInvocation | 🔵 | 🔵 |
| CommandConfigLoadError | error_type | add | ReadFailed, DecodeFailed, Invalid | 🔵 | 🔵 |
| CommandConfigValidationError | error_type | add | InvalidSchemaVersion, InvalidDeclarationId, InvalidReviewScope, DuplicateDeclaration, DuplicateScope, EmptyArgv, TimeoutOutOfRange, RecursiveInvocation, PersistedHostArgument | 🔵 | 🔵 |
| CommandDeclarationIdValidationError | error_type | add | Empty | 🔵 | 🔵 |
| CommandTimeoutValidationError | error_type | add | OutOfRange | 🔵 | 🔵 |
| ConfiguredCommandValidationError | error_type | add | Argv, Timeout, PersistedHostArgument | 🔵 | 🔵 |
| CurrentReviewTrackResolveError | error_type | add | ResolveFailed | 🔵 | 🔵 |
| PhaseCommandConfigValidationError | error_type | add | InvalidSchemaVersion, DuplicateDeclaration | 🔵 | 🔵 |
| PhaseCommandEnterError | error_type | add | Config, UnknownPhase, Runner | 🔵 | 🔵 |
| PhaseCommandExplainError | error_type | add | Config, UnknownPhase | 🔵 | 🔵 |
| PreReviewCommandConfigValidationError | error_type | add | InvalidSchemaVersion, DuplicateScope | 🔵 | 🔵 |
| PreReviewCommandDispatchError | error_type | add | Config, UnknownScope, TrackResolution, TrackMismatch, Runner | 🔵 | 🔵 |
| ProgramRunnerError | error_type | add | SpawnFailed, WaitFailed, TerminateFailed | 🔵 | 🔵 |
| ReviewCheckZeroFindingsEvaluationError | error_type | add | EvaluationFailed | 🔵 | 🔵 |
| ReviewCheckZeroFindingsValidationError | error_type | add | InvalidTrackId, InvalidScope | 🔵 | 🔵 |
| ReviewScopeSelectionValidationError | error_type | add | ScopeAndAll, InvalidScope | 🟡 | 🔵 |

## Secondary Ports

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CurrentReviewTrackResolverPort | secondary_port | add | fn resolve(&self, repository_root: &std::path::Path) -> Result<domain::TrackId, CurrentReviewTrackResolveError> | 🔵 | 🔵 |
| PhaseCommandConfigLoaderPort | secondary_port | add | fn load(&self, repository_root: &std::path::Path) -> Result<PhaseCommandConfig, CommandConfigLoadError> | 🔵 | 🔵 |
| PreReviewCommandConfigLoaderPort | secondary_port | add | fn load(&self, repository_root: &std::path::Path, track_id: &domain::TrackId) -> Result<PreReviewCommandConfig, CommandConfigLoadError> | 🔵 | 🔵 |
| ProgramRunnerPort | secondary_port | add | fn run(&self, invocation: ProgramInvocation) -> Result<ProgramRunOutcome, ProgramRunnerError> | 🔵 | 🔵 |
| ReviewCheckZeroFindingsStatePort | secondary_port | add | fn state_for(&self, track_id: &domain::TrackId, items_dir: &std::path::Path, scope: &domain::review_v2::ScopeName) -> Result<Option<domain::review_v2::ReviewState>, domain::FreeText> | 🔵 | 🔵 |

## Application Services

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CapabilityExecService | application_service | reference | fn execute(&self, request: CapabilityExecRequest) -> Result<CapabilityDispatchOutcome, CapabilityExecError> | 🔵 | 🔵 |
| PhaseCommandService | application_service | add | fn validate(&self, command: PhaseValidateCommand) -> Result<(), CommandConfigLoadError>, fn explain(&self, query: PhaseExplainQuery) -> Result<PhaseCommandExplanation, PhaseCommandExplainError>, fn enter(&self, command: PhaseEnterCommand) -> Result<PhaseCommandEnterOutcome, PhaseCommandEnterError> | 🔵 | 🔵 |
| PreReviewCommandDispatchService | application_service | add | fn dispatch(&self, command: PreReviewCommandDispatchCommand) -> Result<PreReviewCommandDispatchOutcome, PreReviewCommandDispatchError> | 🔵 | 🔵 |
| RefVerifyAggregateService | application_service | modify | fn run(&self, track_id: &str, items_dir: &std::path::Path) -> Result<RefVerifyRunOutcome, RefVerifyDriverError>, fn results(&self, track_id: &str, items_dir: &std::path::Path, chain: RefVerifyChainFilter, layer: RefVerifyLayerFilter, verdict: RefVerifyVerdictFilter) -> Result<RefVerifyResultsOutput, RefVerifyDriverError> | 🔵 | 🔵 |
| RefVerifyCheckApprovedDriverService | application_service | modify | fn check_approved(&self, track_id: &str, items_dir: &std::path::Path, chain: RefVerifyChainFilter) -> Result<RefVerifyCheckApprovedOutcome, RefVerifyDriverError> | 🔵 | 🔵 |
| ReviewCheckZeroFindingsService | application_service | add | fn check_zero_findings(&self, query: &ReviewCheckZeroFindingsQuery) -> Result<ReviewCheckZeroFindingsOutcome, ReviewCheckZeroFindingsEvaluationError> | 🔵 | 🔵 |
| ReviewService | application_service | modify | fn run_codex(&self, input: ReviewRunInput) -> Result<RunReviewOutput, RunReviewError>, fn run_claude(&self, input: ReviewRunInput) -> Result<RunReviewOutput, RunReviewError>, fn run_local(&self, model: Option<String>, timeout_seconds: u64, briefing_file: Option<std::path::PathBuf>, prompt: Option<String>, track_id: Option<String>, round_type: String, group: String, items_dir: std::path::PathBuf) -> ReviewRunLocalOutput, fn check_approved(&self, track_id: String, items_dir: std::path::PathBuf) -> Result<ReviewApprovalOutput, ReviewCheckApprovedError>, fn results(&self, track_id: Option<String>, items_dir: std::path::PathBuf, selection: ReviewScopeSelection, limit: u32, round_type: String, no_hint: bool) -> Result<String, ReviewAuxError>, fn classify(&self, paths: Vec<String>, track_id: Option<String>, items_dir: std::path::PathBuf) -> Result<Vec<(String, String)>, ReviewAuxError>, fn files(&self, scope: String, track_id: Option<String>, items_dir: std::path::PathBuf) -> Result<Vec<String>, ReviewAuxError>, fn validate_scope(&self, scope: String, track_id: Option<String>, items_dir: std::path::PathBuf) -> Result<(), ReviewAuxError>, fn get_briefing(&self, scope: String, track_id: Option<String>, items_dir: std::path::PathBuf) -> Result<Option<String>, ReviewAuxError>, fn persist_commit_hash(&self, track_id: String, workspace_root: std::path::PathBuf) -> Result<String, CommitHashPersistenceError> | 🟡 | 🔵 |

## Interactors

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CapabilityExecInteractor | interactor | reference | — | 🔵 | 🔵 |
| PhaseCommandInteractor | interactor | add | — | 🔵 | 🔵 |
| PreReviewCommandDispatchInteractor | interactor | add | — | 🔵 | 🔵 |
| PreReviewCommandGatedReviewInteractor | interactor | add | — | 🔵 | 🔵 |
| ReviewCheckZeroFindingsInteractor | interactor | add | — | 🔵 | 🔵 |

## DTOs

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CapturedProgramOutput | dto | add | — | 🔵 | 🔵 |
| FailedProgramExecutionRecord | dto | add | — | 🔵 | 🔵 |
| PhaseCommandExplanation | dto | add | — | 🔵 | 🔵 |
| ProgramExecutionRecord | dto | add | — | 🔵 | 🔵 |
| ProgramInvocation | dto | add | — | 🔵 | 🔵 |
| SuccessfulProgramExecutionRecord | dto | add | — | 🔵 | 🔵 |

## Commands

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CapabilityExecRequest | command | modify | — | 🔵 | 🔵 |
| PhaseEnterCommand | command | add | — | 🔵 | 🔵 |
| PhaseValidateCommand | command | add | — | 🔵 | 🔵 |
| PreReviewCommandDispatchCommand | command | add | — | 🔵 | 🔵 |

## Queries

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| PhaseExplainQuery | query | add | — | 🔵 | 🔵 |
| ReviewCheckZeroFindingsQuery | query | add | — | 🔵 | 🔵 |

