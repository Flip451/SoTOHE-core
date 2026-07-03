<!-- Generated from infrastructure-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| MergeMethodDocument | enum | add | Squash, Merge, Rebase | 🔵 | 🔵 |

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| BranchStrategyConfigError | error_type | add | Io, Parse | 🔵 | 🔵 |

## DTOs

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| BranchStrategySnapshotDocument | dto | add | — | 🔵 | 🔵 |
| TrackDocumentV2 | dto | modify | — | 🔵 | 🔵 |

## Secondary Adapters

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CodexDryFixLocalRunner | secondary_adapter | add | impl Default | 🔵 | 🔵 |
| DryCheckServiceFactoryAdapter | secondary_adapter | add | impl DryCheckServiceFactoryPort | 🔵 | 🔵 |
| DryDriverAdapter | secondary_adapter | add | impl Default, impl DryDriverPort | 🔵 | 🔵 |
| FsDiffBaseResolverAdapter | secondary_adapter | add | impl DiffBaseResolverPort | 🔵 | 🔵 |
| FsDryApprovalFactoryAdapter | secondary_adapter | add | impl DryApprovalFactoryPort | 🔵 | 🔵 |
| FsDryBaseBranchAdapter | secondary_adapter | add | impl DryBaseBranchPort | 🔵 | 🔵 |
| FsDryCheckConfigLoaderAdapter | secondary_adapter | add | impl DryCheckConfigLoaderPort | 🔵 | 🔵 |
| FsDryCheckStorageFactoryAdapter | secondary_adapter | add | impl DryCheckStorageFactoryPort | 🔵 | 🔵 |
| FsDryCorpusFragmentsAdapter | secondary_adapter | add | impl DryCorpusFragmentsPort | 🔵 | 🔵 |
| FsDryCorpusRootManifestAdapter | secondary_adapter | add | impl DryCorpusRootManifestWriterPort | 🔵 | 🔵 |
| FsDryDiffBaseFactoryAdapter | secondary_adapter | add | impl DryDiffBaseFactoryPort | 🔵 | 🔵 |
| FsDryRepoRootAdapter | secondary_adapter | add | impl DryRepoRootPort | 🔵 | 🔵 |
| FsDryWriteConfigLoaderAdapter | secondary_adapter | add | impl DryWriteConfigLoaderPort | 🔵 | 🔵 |
| FsFixpointDryGateFactoryAdapter | secondary_adapter | add | impl FixpointDryGateFactoryPort | 🔵 | 🔵 |
| FsFixpointGateStateFactoryAdapter | secondary_adapter | add | impl FixpointGateStateFactoryPort | 🔵 | 🔵 |
| FsFixpointWorkspaceContextAdapter | secondary_adapter | add | impl FixpointWorkspaceContextPort | 🔵 | 🔵 |
| FsReviewGateStateAdapter | secondary_adapter | modify | impl ReviewGateStatePort | 🔵 | 🔵 |
| JsonConfigBranchStrategyAdapter | secondary_adapter | add | impl BranchStrategyPort | 🔵 | 🔵 |
| RecordingDryTierTelemetryAdapter | secondary_adapter | add | impl DryTierTelemetryPort | 🔵 | 🔵 |
| SnapshotBranchStrategyAdapter | secondary_adapter | add | impl BranchStrategyPort | 🔵 | 🔵 |

