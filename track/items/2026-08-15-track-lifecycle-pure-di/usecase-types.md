<!-- Generated from usecase-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| TrackBaselineCaptureLayerResult | enum | add | Captured, AlreadyExists | 🔵 | 🔵 |
| TrackCatalogueLintActiveResult | enum | add | Checked, Skipped | 🔵 | 🔵 |
| TrackLayerFilter | enum | add | All, Selected | 🔵 | 🔵 |
| TrackLayerSelection | enum | add | All, One | 🔵 | 🔵 |
| TrackLayerSignalResult | enum | add | Evaluated, Skipped | 🔵 | 🔵 |
| TrackNextTaskResult | enum | add | Found, NoOpenTask | 🔵 | 🔵 |
| TrackResolutionCommand | enum | add | ReadFromItems, ReadFromRoot, WriteFromItems, WriteFromRoot, DetectActive | 🔵 | 🔵 |
| TrackResolutionResult | enum | add | Resolved, Inactive | 🔵 | 🔵 |
| TrackResolveResult | enum | add | Ready, Blocked | 🟡 | 🔵 |
| TrackSelection | enum | add | Active, Explicit | 🔵 | 🔵 |
| TrackSpecAnchorSelection | enum | add | All, One | 🔵 | 🔵 |
| TrackSpecElementHashResult | enum | add | Single, All | 🔵 | 🔵 |
| TrackSwitchBaseResult | enum | add | Synced, SyncWarning, CheckoutFailed | 🟡 | 🔵 |
| TrackTaskTransition | enum | add | Todo, InProgress, Done, Skipped | 🔵 | 🔵 |
| TrackTransitionResult | enum | add | Transitioned, Rejected | 🔵 | 🔵 |
| TrackTypeGraphEdgeSelection | enum | add | Methods, Fields, Impls, All | 🟡 | 🔵 |
| TrackTypeGraphResult | enum | add | — | 🟡 | 🔵 |
| TrackViewSyncOutcome | enum | add | Synchronized, Warning | 🔵 | 🔵 |
| TrackViewsScope | enum | add | RegistryOnly, Track | 🔵 | 🔵 |
| TrackViewsSyncResult | enum | add | AlreadyCurrent, Rendered | 🟡 | 🔵 |

## Value Objects

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| ProcessExitCode | value_object | add | — | 🔵 | 🔵 |
| RenderedViewPath | value_object | add | — | 🔵 | 🔵 |
| TaskCount | value_object | add | — | 🔵 | 🔵 |
| TrackCatalogueEntryCount | value_object | add | — | 🔵 | 🔵 |
| TrackCataloguePath | value_object | add | — | 🔵 | 🔵 |
| TrackDirectoryPath | value_object | add | — | 🔵 | 🔵 |
| TrackItemsDirectory | value_object | add | — | 🔵 | 🔵 |
| TrackLifecycleIdInput | value_object | add | — | 🔵 | 🔵 |
| TrackLintRulesFile | value_object | add | — | 🔵 | 🔵 |
| TrackRenderedLayerCount | value_object | add | — | 🔵 | 🔵 |
| TrackSourceWorkspace | value_object | add | — | 🔵 | 🔵 |
| TrackTypeGraphClusterDepth | value_object | add | — | 🟡 | 🔵 |
| TrackWorkspaceRoot | value_object | add | — | 🔵 | 🔵 |
| TrackWrittenFileCount | value_object | add | — | 🔵 | 🔵 |

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| TrackAddTaskError | error_type | add | ExecutionFailed | 🔵 | 🔵 |
| TrackArchiveError | error_type | add | ExecutionFailed | 🔵 | 🔵 |
| TrackBaselineCaptureError | error_type | add | ExecutionFailed | 🔵 | 🔵 |
| TrackBaselineGraphError | error_type | add | ExecutionFailed | 🔵 | 🔵 |
| TrackBranchCreateError | error_type | add | ExecutionFailed | 🔵 | 🔵 |
| TrackBranchSwitchError | error_type | add | ExecutionFailed | 🔵 | 🔵 |
| TrackCatalogueImplSignalsError | error_type | add | ExecutionFailed | 🔵 | 🔵 |
| TrackCatalogueLintActiveError | error_type | add | ExecutionFailed | 🔵 | 🔵 |
| TrackCatalogueSpecSignalsError | error_type | add | ExecutionFailed | 🔵 | 🔵 |
| TrackClearOverrideError | error_type | add | ExecutionFailed | 🔵 | 🔵 |
| TrackContractMapError | error_type | add | ExecutionFailed | 🔵 | 🔵 |
| TrackInitError | error_type | add | ExecutionFailed | 🔵 | 🔵 |
| TrackLintError | error_type | add | ExecutionFailed | 🔵 | 🔵 |
| TrackNextTaskError | error_type | add | ExecutionFailed | 🔵 | 🔵 |
| TrackResolutionCompatError | error_type | add | Unavailable | 🔵 | 🔵 |
| TrackResolveError | error_type | add | ExecutionFailed | 🟡 | 🔵 |
| TrackSetCommitHashError | error_type | add | ExecutionFailed | 🔵 | 🔵 |
| TrackSetOverrideError | error_type | add | ExecutionFailed | 🔵 | 🔵 |
| TrackSpecElementHashError | error_type | add | ExecutionFailed | 🔵 | 🔵 |
| TrackSwitchBaseError | error_type | add | ExecutionFailed | 🟡 | 🔵 |
| TrackTaskCountsError | error_type | add | ExecutionFailed | 🔵 | 🔵 |
| TrackTransitionError | error_type | add | ExecutionFailed | 🔵 | 🔵 |
| TrackTypeGraphError | error_type | add | RemovedCommand | 🟡 | 🔵 |
| TrackTypeSignalsError | error_type | add | ExecutionFailed | 🔵 | 🔵 |
| TrackViewsSyncError | error_type | add | ExecutionFailed | 🟡 | 🔵 |
| TrackViewsValidateError | error_type | add | ExecutionFailed | 🟡 | 🔵 |

## Secondary Ports

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| TrackBaselineCapturePort | secondary_port | add | fn execute(&self, track_id: domain::TrackId, command: TrackBaselineCaptureCommand) -> Result<TrackBaselineCaptureResult, TrackBaselineCaptureError> | 🔵 | 🔵 |
| TrackBaselineGraphPort | secondary_port | add | fn execute(&self, track_id: domain::TrackId, command: TrackBaselineGraphCommand) -> Result<TrackBaselineGraphResult, TrackBaselineGraphError> | 🔵 | 🔵 |
| TrackBranchStrategyPort | secondary_port | add | fn global_for_items(&self, items_dir: &TrackItemsDirectory) -> Result<domain::BranchStrategySnapshot, DiagnosticText>, fn snapshot_for_track(&self, workspace_root: &TrackWorkspaceRoot, track_id: &domain::TrackId) -> Result<domain::BranchStrategySnapshot, DiagnosticText> | 🔵 | 🔵 |
| TrackCatalogueImplSignalsPort | secondary_port | add | fn execute(&self, track_id: domain::TrackId, command: TrackCatalogueImplSignalsCommand) -> Result<TrackCatalogueImplSignalsResult, TrackCatalogueImplSignalsError> | 🔵 | 🔵 |
| TrackCatalogueLintActivePort | secondary_port | add | fn execute(&self, track_id: domain::TrackId, command: TrackCatalogueLintActiveCommand) -> Result<TrackCatalogueLintActiveResult, TrackCatalogueLintActiveError> | 🔵 | 🔵 |
| TrackCatalogueSpecSignalsPort | secondary_port | add | fn execute(&self, track_id: domain::TrackId, command: TrackCatalogueSpecSignalsCommand) -> Result<TrackCatalogueSpecSignalsResult, TrackCatalogueSpecSignalsError> | 🔵 | 🔵 |
| TrackCommitHashPort | secondary_port | add | fn persist_current_for_track(&self, track_id: &domain::TrackId) -> Result<domain::CommitHash, DiagnosticText> | 🔵 | 🔵 |
| TrackContractMapPort | secondary_port | add | fn execute(&self, track_id: domain::TrackId, command: TrackContractMapCommand) -> Result<TrackContractMapResult, TrackContractMapError> | 🔵 | 🔵 |
| TrackLintPort | secondary_port | add | fn execute(&self, track_id: domain::TrackId, command: TrackLintCommand) -> Result<TrackLintResult, TrackLintError> | 🔵 | 🔵 |
| TrackMetadataPort | secondary_port | add | fn save(&self, items_dir: &TrackItemsDirectory, metadata: domain::TrackMetadata) -> Result<(), DiagnosticText>, fn find(&self, items_dir: &TrackItemsDirectory, track_id: &domain::TrackId) -> Result<Option<domain::TrackMetadata>, DiagnosticText> | 🔵 | 🔵 |
| TrackNextTaskQueryPort | secondary_port | add | fn next_task(&self, track_id: domain::TrackId, items_dir: TrackItemsDirectory) -> Result<Option<NextTaskOutput>, TrackNextTaskError> | 🔵 | 🔵 |
| TrackOverrideClearPort | secondary_port | add | fn clear_override(&self, track_id: domain::TrackId, items_dir: TrackItemsDirectory) -> Result<TaskOperationOutput, TaskOperationError> | 🔵 | 🔵 |
| TrackOverrideSetPort | secondary_port | add | fn set_override(&self, track_id: domain::TrackId, items_dir: TrackItemsDirectory, status: domain::StatusOverrideKind, reason: DiagnosticText) -> Result<TaskOperationOutput, TaskOperationError> | 🔵 | 🔵 |
| TrackResolutionPort | secondary_port | add | fn execute(&self, command: TrackResolutionCommand) -> Result<TrackResolutionResult, TrackResolutionCompatError> | 🔵 | 🔵 |
| TrackSelectionPort | secondary_port | add | fn resolve_required(&self, items_dir: &TrackItemsDirectory, selection: &TrackSelection) -> Result<domain::TrackId, DiagnosticText>, fn resolve_active(&self, workspace_root: &TrackWorkspaceRoot) -> Result<domain::TrackId, DiagnosticText>, fn resolve_views_scope(&self, workspace_root: &TrackWorkspaceRoot, selection: &TrackSelection) -> Result<TrackViewsScope, DiagnosticText> | 🔵 | 🔵 |
| TrackSpecElementHashPort | secondary_port | add | fn execute(&self, track_id: domain::TrackId, command: TrackSpecElementHashCommand) -> Result<TrackSpecElementHashResult, TrackSpecElementHashError> | 🔵 | 🔵 |
| TrackTaskAddPort | secondary_port | add | fn add_task(&self, track_id: domain::TrackId, items_dir: TrackItemsDirectory, description: domain::NonEmptyString, section: Option<domain::NonEmptyString>, after: Option<domain::TaskId>) -> Result<TaskOperationOutput, TaskOperationError> | 🔵 | 🔵 |
| TrackTaskCountsQueryPort | secondary_port | add | fn task_counts(&self, track_id: domain::TrackId, items_dir: TrackItemsDirectory) -> Result<TaskCountsOutput, TrackTaskCountsError> | 🔵 | 🔵 |
| TrackTaskTransitionPort | secondary_port | add | fn transition_task(&self, track_id: domain::TrackId, items_dir: TrackItemsDirectory, task_id: domain::TaskId, transition: TrackTaskTransition) -> Result<TaskTransitionOutcome, TaskOperationError> | 🔵 | 🔵 |
| TrackTypeGraphPort | secondary_port | add | fn execute(&self, track_id: domain::TrackId, command: TrackTypeGraphCommand) -> Result<TrackTypeGraphResult, TrackTypeGraphError> | 🟡 | 🔵 |
| TrackTypeSignalsPort | secondary_port | add | fn execute(&self, track_id: domain::TrackId, command: TrackTypeSignalsCommand) -> Result<TrackTypeSignalsResult, TrackTypeSignalsError> | 🔵 | 🔵 |
| TrackViewsPort | secondary_port | add | fn validate(&self, workspace_root: &TrackWorkspaceRoot) -> Result<(), DiagnosticText>, fn sync(&self, workspace_root: &TrackWorkspaceRoot, scope: &TrackViewsScope) -> Result<Vec<RenderedViewPath>, DiagnosticText> | 🔵 | 🔵 |

## Application Services

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| TrackAddTaskService | application_service | add | fn execute(&self, command: TrackAddTaskCommand) -> Result<TrackAddTaskResult, TrackAddTaskError> | 🔵 | 🔵 |
| TrackArchiveService | application_service | add | fn execute(&self, command: TrackArchiveCommand) -> Result<TrackArchiveResult, TrackArchiveError> | 🔵 | 🔵 |
| TrackBaselineCaptureService | application_service | add | fn execute(&self, command: TrackBaselineCaptureCommand) -> Result<TrackBaselineCaptureResult, TrackBaselineCaptureError> | 🔵 | 🔵 |
| TrackBaselineGraphService | application_service | add | fn execute(&self, command: TrackBaselineGraphCommand) -> Result<TrackBaselineGraphResult, TrackBaselineGraphError> | 🔵 | 🔵 |
| TrackBranchCreateService | application_service | add | fn execute(&self, command: TrackBranchCreateCommand) -> Result<TrackBranchCreateResult, TrackBranchCreateError> | 🔵 | 🔵 |
| TrackBranchSwitchService | application_service | add | fn execute(&self, command: TrackBranchSwitchCommand) -> Result<TrackBranchSwitchResult, TrackBranchSwitchError> | 🔵 | 🔵 |
| TrackCatalogueImplSignalsService | application_service | add | fn execute(&self, command: TrackCatalogueImplSignalsCommand) -> Result<TrackCatalogueImplSignalsResult, TrackCatalogueImplSignalsError> | 🔵 | 🔵 |
| TrackCatalogueLintActiveService | application_service | add | fn execute(&self, command: TrackCatalogueLintActiveCommand) -> Result<TrackCatalogueLintActiveResult, TrackCatalogueLintActiveError> | 🔵 | 🔵 |
| TrackCatalogueSpecSignalsService | application_service | add | fn execute(&self, command: TrackCatalogueSpecSignalsCommand) -> Result<TrackCatalogueSpecSignalsResult, TrackCatalogueSpecSignalsError> | 🔵 | 🔵 |
| TrackClearOverrideService | application_service | add | fn execute(&self, command: TrackClearOverrideCommand) -> Result<TrackClearOverrideResult, TrackClearOverrideError> | 🔵 | 🔵 |
| TrackContractMapService | application_service | add | fn execute(&self, command: TrackContractMapCommand) -> Result<TrackContractMapResult, TrackContractMapError> | 🔵 | 🔵 |
| TrackInitService | application_service | add | fn execute(&self, command: TrackInitCommand) -> Result<TrackInitResult, TrackInitError> | 🔵 | 🔵 |
| TrackLintService | application_service | add | fn execute(&self, command: TrackLintCommand) -> Result<TrackLintResult, TrackLintError> | 🔵 | 🔵 |
| TrackNextTaskService | application_service | add | fn execute(&self, command: TrackNextTaskCommand) -> Result<TrackNextTaskResult, TrackNextTaskError> | 🔵 | 🔵 |
| TrackResolutionService | application_service | add | fn execute(&self, command: TrackResolutionCommand) -> Result<TrackResolutionResult, TrackResolutionCompatError> | 🔵 | 🔵 |
| TrackResolveService | application_service | add | fn execute(&self, command: TrackResolveCommand) -> Result<TrackResolveResult, TrackResolveError> | 🟡 | 🔵 |
| TrackSetCommitHashService | application_service | add | fn execute(&self, command: TrackSetCommitHashCommand) -> Result<TrackSetCommitHashResult, TrackSetCommitHashError> | 🔵 | 🔵 |
| TrackSetOverrideService | application_service | add | fn execute(&self, command: TrackSetOverrideCommand) -> Result<TrackSetOverrideResult, TrackSetOverrideError> | 🔵 | 🔵 |
| TrackSpecElementHashService | application_service | add | fn execute(&self, command: TrackSpecElementHashCommand) -> Result<TrackSpecElementHashResult, TrackSpecElementHashError> | 🔵 | 🔵 |
| TrackSwitchBaseService | application_service | add | fn execute(&self, command: TrackSwitchBaseCommand) -> Result<TrackSwitchBaseResult, TrackSwitchBaseError> | 🟡 | 🔵 |
| TrackTaskCountsService | application_service | add | fn execute(&self, command: TrackTaskCountsCommand) -> Result<TrackTaskCountsResult, TrackTaskCountsError> | 🟡 | 🔵 |
| TrackTransitionService | application_service | add | fn execute(&self, command: TrackTransitionCommand) -> Result<TrackTransitionResult, TrackTransitionError> | 🔵 | 🔵 |
| TrackTypeGraphService | application_service | add | fn execute(&self, command: TrackTypeGraphCommand) -> Result<TrackTypeGraphResult, TrackTypeGraphError> | 🟡 | 🔵 |
| TrackTypeSignalsService | application_service | add | fn execute(&self, command: TrackTypeSignalsCommand) -> Result<TrackTypeSignalsResult, TrackTypeSignalsError> | 🔵 | 🔵 |
| TrackViewsSyncService | application_service | add | fn execute(&self, command: TrackViewsSyncCommand) -> Result<TrackViewsSyncResult, TrackViewsSyncError> | 🟡 | 🔵 |
| TrackViewsValidateService | application_service | add | fn execute(&self, command: TrackViewsValidateCommand) -> Result<TrackViewsValidateResult, TrackViewsValidateError> | 🟡 | 🔵 |

## Interactors

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| TaskQueryInteractor | interactor | modify | — | 🔵 | 🔵 |
| TrackAddTaskInteractor | interactor | add | — | 🔵 | 🔵 |
| TrackArchiveInteractor | interactor | add | — | 🔵 | 🔵 |
| TrackBaselineCaptureInteractor | interactor | add | — | 🔵 | 🔵 |
| TrackBaselineGraphInteractor | interactor | add | — | 🔵 | 🔵 |
| TrackBranchCreateInteractor | interactor | add | — | 🔵 | 🔵 |
| TrackBranchSwitchInteractor | interactor | add | — | 🔵 | 🔵 |
| TrackCatalogueImplSignalsInteractor | interactor | add | — | 🔵 | 🔵 |
| TrackCatalogueLintActiveInteractor | interactor | add | — | 🔵 | 🔵 |
| TrackCatalogueSpecSignalsInteractor | interactor | add | — | 🔵 | 🔵 |
| TrackClearOverrideInteractor | interactor | add | — | 🔵 | 🔵 |
| TrackContractMapInteractor | interactor | add | — | 🔵 | 🔵 |
| TrackInitInteractor | interactor | add | — | 🔵 | 🔵 |
| TrackLintInteractor | interactor | add | — | 🔵 | 🔵 |
| TrackNextTaskInteractor | interactor | add | — | 🔵 | 🔵 |
| TrackResolutionInteractor | interactor | add | — | 🔵 | 🔵 |
| TrackResolveInteractor | interactor | add | — | 🟡 | 🔵 |
| TrackSetCommitHashInteractor | interactor | add | — | 🔵 | 🔵 |
| TrackSetOverrideInteractor | interactor | add | — | 🔵 | 🔵 |
| TrackSpecElementHashInteractor | interactor | add | — | 🔵 | 🔵 |
| TrackSwitchBaseInteractor | interactor | add | — | 🟡 | 🔵 |
| TrackTaskCountsInteractor | interactor | add | — | 🟡 | 🔵 |
| TrackTransitionInteractor | interactor | add | — | 🔵 | 🔵 |
| TrackTypeGraphInteractor | interactor | add | — | 🟡 | 🔵 |
| TrackTypeSignalsInteractor | interactor | add | — | 🔵 | 🔵 |
| TrackViewsSyncInteractor | interactor | add | — | 🟡 | 🔵 |
| TrackViewsValidateInteractor | interactor | add | — | 🟡 | 🔵 |

## DTOs

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| NextTaskOutput | dto | modify | — | 🔵 | 🔵 |
| TrackAddTaskResult | dto | add | — | 🔵 | 🔵 |
| TrackArchiveResult | dto | add | — | 🔵 | 🔵 |
| TrackBaselineCaptureResult | dto | add | — | 🔵 | 🔵 |
| TrackBaselineGraphResult | dto | add | — | 🔵 | 🔵 |
| TrackBranchCreateResult | dto | add | — | 🔵 | 🔵 |
| TrackBranchSwitchResult | dto | add | — | 🔵 | 🔵 |
| TrackCatalogueImplLayerResult | dto | add | — | 🔵 | 🔵 |
| TrackCatalogueImplSignalsResult | dto | add | — | 🔵 | 🔵 |
| TrackCatalogueLintLayerResult | dto | add | — | 🔵 | 🔵 |
| TrackCatalogueSpecSignalsResult | dto | add | — | 🔵 | 🔵 |
| TrackClearOverrideResult | dto | add | — | 🔵 | 🔵 |
| TrackContractMapResult | dto | add | — | 🔵 | 🔵 |
| TrackInitResult | dto | add | — | 🔵 | 🔵 |
| TrackLintResult | dto | add | — | 🔵 | 🔵 |
| TrackSetCommitHashResult | dto | add | — | 🔵 | 🔵 |
| TrackSetOverrideResult | dto | add | — | 🔵 | 🔵 |
| TrackTaskCountsResult | dto | add | — | 🟡 | 🔵 |
| TrackTypeSignalsResult | dto | add | — | 🔵 | 🔵 |
| TrackViewsValidateResult | dto | add | — | 🟡 | 🔵 |

## Commands

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| TrackAddTaskCommand | command | add | — | 🔵 | 🔵 |
| TrackArchiveCommand | command | add | — | 🔵 | 🔵 |
| TrackBaselineCaptureCommand | command | add | — | 🔵 | 🔵 |
| TrackBaselineGraphCommand | command | add | — | 🔵 | 🔵 |
| TrackBranchCreateCommand | command | add | — | 🔵 | 🔵 |
| TrackBranchSwitchCommand | command | add | — | 🔵 | 🔵 |
| TrackCatalogueImplSignalsCommand | command | add | — | 🔵 | 🔵 |
| TrackCatalogueLintActiveCommand | command | add | — | 🔵 | 🔵 |
| TrackCatalogueSpecSignalsCommand | command | add | — | 🔵 | 🔵 |
| TrackClearOverrideCommand | command | add | — | 🔵 | 🔵 |
| TrackContractMapCommand | command | add | — | 🔵 | 🔵 |
| TrackInitCommand | command | add | — | 🔵 | 🔵 |
| TrackLintCommand | command | add | — | 🔵 | 🔵 |
| TrackNextTaskCommand | command | add | — | 🔵 | 🔵 |
| TrackResolveCommand | command | add | — | 🟡 | 🔵 |
| TrackSetCommitHashCommand | command | add | — | 🔵 | 🔵 |
| TrackSetOverrideCommand | command | add | — | 🔵 | 🔵 |
| TrackSpecElementHashCommand | command | add | — | 🔵 | 🔵 |
| TrackSwitchBaseCommand | command | add | — | 🟡 | 🔵 |
| TrackTaskCountsCommand | command | add | — | 🔵 | 🔵 |
| TrackTransitionCommand | command | add | — | 🔵 | 🔵 |
| TrackTypeGraphCommand | command | add | — | 🟡 | 🔵 |
| TrackTypeSignalsCommand | command | add | — | 🔵 | 🔵 |
| TrackViewsSyncCommand | command | add | — | 🟡 | 🔵 |
| TrackViewsValidateCommand | command | add | — | 🟡 | 🔵 |

