<!-- Generated from usecase-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| GuardDecision | enum | — | Allow, Block | 🔵 | 🔵 |
| ReviewApprovalDecision | enum | — | Approved, ApprovedWithBypass, Blocked | 🔵 | 🔵 |
| HookVerdictDecision | enum | — | Allow, Block | 🔵 | 🔵 |
| ReviewRoundType | enum | — | Fast, Final | 🔵 | 🔵 |
| LintRuleKind | enum | — | FieldEmpty, FieldNonEmpty, KindLayerConstraint | 🔵 | 🔵 |

## Value Objects

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| PathClassification | value_object | reference | — | 🔵 | 🔵 |

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| ExportSchemaError | error_type | — | ExportFailed, SerializationFailed | 🔵 | 🔵 |
| ReviewCheckApprovedError | error_type | — | InvalidTrackId, ReviewStoreError, EvaluationFailed | 🔵 | 🔵 |
| HookDispatchError | error_type | — | UnknownHookName, HandlerFailed | 🔵 | 🔵 |
| TrackPhaseError | error_type | — | InvalidTrackId, TrackNotFound, ImplPlanLoadFailed | 🔵 | 🔵 |
| TaskOperationError | error_type | — | InvalidTrackId, InvalidTaskId, InvalidCommitHash, TrackNotFound, TaskNotFound, TransitionFailed, StoreFailed, BranchGuardFailed, BranchlessGuardFailed | 🔵 | 🔵 |
| PreCommitTypeSignalsError | error_type | — | GitDiscoverFailed, RulesFileMissing, RulesParseError, SymlinkRejected, MetadataLoadFailed, ImplPlanLoadFailed, TypeSignalsRecomputeFailed | 🔵 | 🔵 |
| RunReviewError | error_type | — | InvalidTrackId, InvalidGroupName, CompositionFailed, ReviewerFailed | 🔵 | 🔵 |
| CommitHashPersistenceError | error_type | — | InvalidTrackId, GitDiscoverFailed, BranchMismatch, RevParseFailed, InvalidSha, StoreWriteFailed, TrackDirMissing | 🔵 | 🔵 |
| ScopeQueryError | error_type | modify | DiffGet, UnknownScope, InvalidPath, InvalidScopeName | 🔵 | 🔵 |
| RenderContractMapError | error_type | modify | CatalogueLoaderFailed, ContractMapWriterFailed, EmptyCatalogue, LayerNotFound, UnknownKindFilter, InvalidTrackId | 🔵 | 🔵 |
| RunCatalogueLintError | error_type | modify | CatalogueLoad, LintExecution, InvalidLayer, InvalidRuleSpec | 🔵 | 🔵 |

## Secondary Ports

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| ShellParserPort | secondary_port | — | fn split_shell(&self, input: &str) -> Result<Vec<String>, String> | 🔵 | 🔵 |
| SchemaExporterPort | secondary_port | — | fn export_as_json(&self, crate_name: &str) -> Result<String, String> | 🔵 | 🔵 |
| HookShellParserPort | secondary_port | — | fn split_shell(&self, input: &str) -> Result<Vec<SimpleCommand>, ParseError> | 🔵 | 🔵 |

## Application Services

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| GuardCheckService | application_service | — | fn check(&self, command: String) -> GuardCheckOutput | 🔵 | 🔵 |
| ExportSchemaService | application_service | — | fn export(&self, command: ExportSchemaCommand) -> Result<String, ExportSchemaError> | 🔵 | 🔵 |
| ReviewCheckApprovedService | application_service | — | fn check_approved(&self, track_id: String, items_dir: PathBuf) -> Result<ReviewApprovalOutput, ReviewCheckApprovedError> | 🔵 | 🔵 |
| HookDispatchService | application_service | — | fn dispatch(&self, hook_name: String, command: HookDispatchCommand) -> Result<HookVerdictOutput, HookDispatchError> | 🔵 | 🔵 |
| TrackPhaseService | application_service | — | fn resolve(&self, track_id: String, items_dir: PathBuf) -> Result<TrackPhaseOutput, TrackPhaseError> | 🔵 | 🔵 |
| VerifyAdrSignals | application_service | modify | fn verify(&self, command: VerifyAdrSignalsCommand) -> Result<AdrVerifyReport, VerifyAdrSignalsError> | 🔵 | 🔵 |
| ScopeQueryService | application_service | modify | fn classify(&self, paths: Vec<FilePath>) -> Result<Vec<PathClassification>, ScopeQueryError>, fn files(&self, scope: ScopeName) -> Result<Vec<FilePath>, ScopeQueryError>, fn classify_by_strings(&self, paths: Vec<String>) -> Result<Vec<ScopeClassificationOutput>, ScopeQueryError>, fn files_by_string(&self, scope: String) -> Result<Vec<String>, ScopeQueryError> | 🔵 | 🔵 |
| TaskOperationService | application_service | — | fn transition_task(&self, cmd: TaskTransitionCommand) -> Result<TaskOperationOutput, TaskOperationError>, fn add_task(&self, cmd: AddTaskCommand) -> Result<TaskOperationOutput, TaskOperationError>, fn set_override(&self, cmd: SetOverrideCommand) -> Result<TaskOperationOutput, TaskOperationError>, fn clear_override(&self, cmd: ClearOverrideCommand) -> Result<TaskOperationOutput, TaskOperationError> | 🔵 | 🔵 |
| TaskQueryService | application_service | — | fn next_task(&self, track_id: String, items_dir: PathBuf) -> Result<Option<NextTaskOutput>, TaskOperationError>, fn task_counts(&self, track_id: String, items_dir: PathBuf) -> Result<TaskCountsOutput, TaskOperationError> | 🔵 | 🔵 |
| PreCommitTypeSignalsService | application_service | — | fn run(&self, track_id: String, workspace_root: PathBuf) -> Result<PreCommitTypeSignalsOutput, PreCommitTypeSignalsError> | 🔵 | 🔵 |
| RunReviewService | application_service | — | fn run(&self, command: RunReviewCommand) -> Result<RunReviewOutput, RunReviewError> | 🔵 | 🔵 |
| CommitHashPersistenceService | application_service | — | fn persist(&self, track_id: String, workspace_root: PathBuf) -> Result<String, CommitHashPersistenceError> | 🔵 | 🔵 |
| RenderContractMap | application_service | modify | fn execute(&self, cmd: &RenderContractMapCommand) -> Result<RenderContractMapOutput, RenderContractMapError> | 🔵 | 🔵 |
| RunCatalogueLint | application_service | modify | fn execute(&self, cmd: RunCatalogueLintCommand) -> Result<Vec<CatalogueLintViolation>, RunCatalogueLintError> | 🔵 | 🔵 |

## Use Cases

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| ActivateTrackUseCase | use_case | modify | — | 🔵 | 🔵 |

## Interactors

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| GuardCheckInteractor | interactor | — | — | 🔵 | 🔵 |
| ExportSchemaInteractor | interactor | — | — | 🔵 | 🔵 |
| ReviewCheckApprovedInteractor | interactor | — | — | 🔵 | 🔵 |
| HookDispatchInteractor | interactor | — | — | 🔵 | 🔵 |
| TrackPhaseInteractor | interactor | — | — | 🔵 | 🔵 |
| VerifyAdrSignalsInteractor | interactor | modify | — | 🔵 | 🔵 |
| ScopeQueryInteractor | interactor | modify | — | 🔵 | 🔵 |
| TaskOperationInteractor | interactor | — | — | 🔵 | 🔵 |
| TaskQueryInteractor | interactor | — | — | 🔵 | 🔵 |
| PreCommitTypeSignalsInteractor | interactor | — | — | 🔵 | 🔵 |
| RunReviewInteractor | interactor | — | — | 🔵 | 🔵 |
| CommitHashPersistenceInteractor | interactor | — | — | 🔵 | 🔵 |
| RenderContractMapInteractor | interactor | modify | — | 🔵 | 🔵 |
| RunCatalogueLintInteractor | interactor | modify | — | 🔵 | 🔵 |

## DTOs

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| TrackStatusOutput | dto | — | — | 🔵 | 🔵 |
| GuardCheckOutput | dto | — | — | 🔵 | 🔵 |
| TaskOperationOutput | dto | — | — | 🔵 | 🔵 |
| ReviewApprovalOutput | dto | — | — | 🔵 | 🔵 |
| HookVerdictOutput | dto | — | — | 🔵 | 🔵 |
| TrackPhaseOutput | dto | — | — | 🔵 | 🔵 |
| ScopeClassificationOutput | dto | — | — | 🔵 | 🔵 |
| NextTaskOutput | dto | — | — | 🔵 | 🔵 |
| TaskCountsOutput | dto | — | — | 🔵 | 🔵 |
| PreCommitTypeSignalsOutput | dto | — | — | 🔵 | 🔵 |
| RunReviewOutput | dto | — | — | 🔵 | 🔵 |
| RenderContractMapOutput | dto | reference | — | 🔵 | 🔵 |
| LintRuleSpec | dto | — | — | 🔵 | 🔵 |

## Commands

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| ExportSchemaCommand | command | — | — | 🔵 | 🔵 |
| HookDispatchCommand | command | — | — | 🔵 | 🔵 |
| TaskTransitionCommand | command | — | — | 🔵 | 🔵 |
| AddTaskCommand | command | — | — | 🔵 | 🔵 |
| SetOverrideCommand | command | — | — | 🔵 | 🔵 |
| ClearOverrideCommand | command | — | — | 🔵 | 🔵 |
| RunReviewCommand | command | — | — | 🔵 | 🔵 |
| RenderContractMapCommand | command | modify | — | 🔵 | 🔵 |
| RunCatalogueLintCommand | command | modify | — | 🔵 | 🔵 |

## Free Functions

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| check_compliance_render | free_function | — | — | 🔵 | 🔵 |
| has_skill_command | free_function | — | — | 🔵 | 🔵 |
| reject_branchless_guard_by_str | free_function | — | — | 🔵 | 🔵 |
| reject_branchless_implementation_transition_by_str | free_function | — | — | 🔵 | 🔵 |

