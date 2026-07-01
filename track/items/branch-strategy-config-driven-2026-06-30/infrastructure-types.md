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
| FsDiffBaseResolverAdapter | secondary_adapter | add | impl DiffBaseResolverPort | 🔵 | 🔵 |
| FsDryApprovalFactoryAdapter | secondary_adapter | add | impl DryApprovalFactoryPort | 🟡 | 🔵 |
| FsDryCheckConfigLoaderAdapter | secondary_adapter | add | impl DryCheckConfigLoaderPort | 🟡 | 🔵 |
| FsFixpointDryGateFactoryAdapter | secondary_adapter | add | impl FixpointDryGateFactoryPort | 🟡 | 🔵 |
| FsFixpointGateStateFactoryAdapter | secondary_adapter | add | impl FixpointGateStateFactoryPort | 🟡 | 🔵 |
| FsFixpointWorkspaceContextAdapter | secondary_adapter | add | impl FixpointWorkspaceContextPort | 🟡 | 🔵 |
| FsReviewGateStateAdapter | secondary_adapter | modify | impl ReviewGateStatePort | 🔵 | 🔵 |
| JsonConfigBranchStrategyAdapter | secondary_adapter | add | impl BranchStrategyPort | 🔵 | 🔵 |
| SnapshotBranchStrategyAdapter | secondary_adapter | add | impl BranchStrategyPort | 🔵 | 🔵 |

