<!-- Generated from usecase-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| ClassifiedProgramExecutionRecord | enum | add | Succeeded, Failed | 🔵 | 🔵 |
| PhaseCommandEnterOutcome | enum | add | Completed, Blocked | 🟡 | 🔵 |
| PreReviewCommandDispatchOutcome | enum | add | ReadyForReview, Blocked | 🔵 | 🔵 |
| ProgramOutputStream | enum | add | Stdout, Stderr | 🔵 | 🔵 |
| ProgramRunOutcome | enum | add | Exited, TimedOut, OutputLimitExceeded | 🔵 | 🔵 |
| ReviewScopeSelector | enum | add | Named, Other | 🔵 | 🔵 |
| ReviewTrackSelector | enum | add | Explicit, CurrentBranch | 🔵 | 🔵 |

## Value Objects

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CommandArgument | value_object | add | — | 🟡 | 🔵 |
| CommandArgv | value_object | add | — | 🟡 | 🔵 |
| CommandConfigSchemaVersion | value_object | add | — | 🟡 | 🔵 |
| CommandDeclarationId | value_object | add | — | 🟡 | 🔵 |
| CommandSequenceIndex | value_object | add | — | 🟡 | 🔵 |
| CommandTimeoutSeconds | value_object | add | — | 🟡 | 🔵 |
| ConfiguredCommand | value_object | add | — | 🟡 | 🔵 |
| OutputCaptureLimitBytes | value_object | add | — | 🟡 | 🔵 |
| PhaseCommandConfig | value_object | add | — | 🟡 | 🔵 |
| PhaseCommandDeclaration | value_object | add | — | 🟡 | 🔵 |
| PreReviewCommandConfig | value_object | add | — | 🟡 | 🔵 |
| PreReviewScopeCommandDeclaration | value_object | add | — | 🔵 | 🔵 |
| ProgramExitCode | value_object | add | — | 🟡 | 🔵 |
| UnvalidatedTimeoutSeconds | value_object | add | — | 🟡 | 🔵 |

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CommandArgvValidationError | error_type | add | Empty, RecursiveInvocation | 🟡 | 🔵 |
| CommandConfigLoadError | error_type | add | ReadFailed, DecodeFailed, Invalid | 🔵 | 🔵 |
| CommandConfigValidationError | error_type | add | InvalidSchemaVersion, InvalidDeclarationId, InvalidReviewScope, DuplicateDeclaration, DuplicateScope, EmptyArgv, TimeoutOutOfRange, RecursiveInvocation | 🟡 | 🔵 |
| CommandDeclarationIdValidationError | error_type | add | Empty | 🟡 | 🔵 |
| CommandTimeoutValidationError | error_type | add | OutOfRange | 🟡 | 🔵 |
| ConfiguredCommandValidationError | error_type | add | Argv, Timeout | 🟡 | 🔵 |
| CurrentReviewTrackResolveError | error_type | add | ResolveFailed | 🔵 | 🔵 |
| PhaseCommandConfigValidationError | error_type | add | InvalidSchemaVersion, DuplicateDeclaration | 🟡 | 🔵 |
| PhaseCommandEnterError | error_type | add | Config, UnknownPhase, Runner | 🟡 | 🔵 |
| PhaseCommandExplainError | error_type | add | Config, UnknownPhase | 🟡 | 🔵 |
| PreReviewCommandConfigValidationError | error_type | add | InvalidSchemaVersion, DuplicateScope | 🟡 | 🔵 |
| PreReviewCommandDispatchError | error_type | add | Config, UnknownScope, TrackResolution, Runner | 🟡 | 🔵 |
| ProgramRunnerError | error_type | add | SpawnFailed, WaitFailed, TerminateFailed | 🔵 | 🔵 |

## Secondary Ports

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CurrentReviewTrackResolverPort | secondary_port | add | fn resolve(&self, repository_root: &std::path::Path) -> Result<domain::TrackId, CurrentReviewTrackResolveError> | 🔵 | 🔵 |
| PhaseCommandConfigLoaderPort | secondary_port | add | fn load(&self, repository_root: &std::path::Path) -> Result<PhaseCommandConfig, CommandConfigLoadError> | 🟡 | 🔵 |
| PreReviewCommandConfigLoaderPort | secondary_port | add | fn load(&self, repository_root: &std::path::Path, track_id: &domain::TrackId) -> Result<PreReviewCommandConfig, CommandConfigLoadError> | 🔵 | 🔵 |
| ProgramRunnerPort | secondary_port | add | fn run(&self, invocation: ProgramInvocation) -> Result<ProgramRunOutcome, ProgramRunnerError> | 🔵 | 🔵 |

## Application Services

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| PhaseCommandService | application_service | add | fn validate(&self, command: PhaseValidateCommand) -> Result<(), CommandConfigLoadError>, fn explain(&self, query: PhaseExplainQuery) -> Result<PhaseCommandExplanation, PhaseCommandExplainError>, fn enter(&self, command: PhaseEnterCommand) -> Result<PhaseCommandEnterOutcome, PhaseCommandEnterError> | 🟡 | 🔵 |
| PreReviewCommandDispatchService | application_service | add | fn dispatch(&self, command: PreReviewCommandDispatchCommand) -> Result<PreReviewCommandDispatchOutcome, PreReviewCommandDispatchError> | 🔵 | 🔵 |

## Interactors

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| PhaseCommandInteractor | interactor | add | — | 🟡 | 🔵 |
| PreReviewCommandDispatchInteractor | interactor | add | — | 🟡 | 🔵 |
| PreReviewCommandGatedReviewInteractor | interactor | add | — | 🟡 | 🔵 |

## DTOs

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CapturedProgramOutput | dto | add | — | 🔵 | 🔵 |
| FailedProgramExecutionRecord | dto | add | — | 🟡 | 🔵 |
| PhaseCommandExplanation | dto | add | — | 🟡 | 🔵 |
| ProgramExecutionRecord | dto | add | — | 🔵 | 🔵 |
| ProgramInvocation | dto | add | — | 🔵 | 🔵 |
| SuccessfulProgramExecutionRecord | dto | add | — | 🟡 | 🔵 |

## Commands

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| PhaseEnterCommand | command | add | — | 🟡 | 🔵 |
| PhaseValidateCommand | command | add | — | 🟡 | 🔵 |
| PreReviewCommandDispatchCommand | command | add | — | 🔵 | 🔵 |

## Queries

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| PhaseExplainQuery | query | add | — | 🟡 | 🔵 |

