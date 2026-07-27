<!-- Generated from usecase-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| PrCommand | enum | add | Push, Ensure, Status, WaitAndMerge, TriggerReview, PollReview, ReviewCycle | 🔵 | 🔵 |
| PrReviewCycleMode | enum | add | Start, Resume | 🔵 | 🔵 |
| ResolvedSignalChainCommand | enum | add | CalcAdrUser, CheckAdrUser, CalcSpecAdr, CheckSpecAdr, CalcCatalogSpec, CheckCatalogSpec, CalcImplCatalog, CheckImplCatalog | 🔵 | 🔵 |
| SignalCommand | enum | add | CalcAdrUser, CheckAdrUser, CalcSpecAdr, CheckSpecAdr, CalcCatalogSpec, CheckCatalogSpec, CalcImplCatalog, CheckImplCatalog, CheckGate | 🔵 | 🔵 |
| SignalGateName | enum | reference | Commit, Merge | 🔵 | 🔵 |
| SignalRootSelection | enum | add | Supplied, Discover | 🔵 | 🔵 |
| SignalStrictOverride | enum | add | UseGateMatrix, ForceStrict | 🔵 | 🔵 |

## Value Objects

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| PrBaseOverride | value_object | add | — | 🔵 | 🔵 |
| PrIdentifier | value_object | add | — | 🔵 | 🔵 |
| PrPollIntervalSeconds | value_object | add | — | 🔵 | 🔵 |
| PrPollTimeoutSeconds | value_object | add | — | 🔵 | 🔵 |
| PrTrackIdOverride | value_object | add | — | 🔵 | 🔵 |
| SignalFailureReason | value_object | add | — | 🔵 | 🔵 |

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| SignalCommandPortError | error_type | add | RepositoryDiscovery, BranchAbsent, BranchReadFailure, SpecPathResolution, Persistence, Execution | 🔵 | 🔵 |
| SignalGateConfigError | error_type | add | RepositoryDiscovery, ConfigurationNotFound, ConfigurationInvalid | 🔵 | 🔵 |

## Secondary Ports

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| PrCommandPort | secondary_port | add | fn execute(&self, command: PrCommand) -> PrCommandOutput | 🔵 | 🔵 |
| SignalActiveTrackResolverPort | secondary_port | add | fn resolve_active_track(&self, workspace_root: Option<&std::path::Path>) -> Result<domain::TrackId, SignalCommandPortError> | 🔵 | 🔵 |
| SignalCommandPort | secondary_port | add | fn execute(&self, command: ResolvedSignalChainCommand) -> Result<SignalChainExecutionReport, SignalCommandPortError> | 🔵 | 🔵 |
| SignalGateConfigPort | secondary_port | add | fn load(&self, workspace_root: Option<&std::path::Path>) -> Result<domain::SignalGateMatrix, SignalGateConfigError> | 🔵 | 🔵 |
| SignalSpecPathResolverPort | secondary_port | add | fn resolve_spec_path(&self, workspace_root: Option<&std::path::Path>, spec_json_path: Option<&std::path::Path>) -> Result<std::path::PathBuf, SignalCommandPortError> | 🔵 | 🔵 |

## Interactors

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| PrCommandInteractor | interactor | modify | — | 🔵 | 🔵 |
| SignalCommandInteractor | interactor | add | — | 🔵 | 🔵 |

## DTOs

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| PrCommandOutput | dto | reference | — | 🔵 | 🔵 |
| SignalChainExecutionReport | dto | add | — | 🔵 | 🔵 |
| SignalCommandOutput | dto | reference | — | 🔵 | 🔵 |

