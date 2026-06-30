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
| JsonConfigBranchStrategyAdapter | secondary_adapter | add | impl BranchStrategyPort | 🔵 | 🔵 |
| SnapshotBranchStrategyAdapter | secondary_adapter | add | impl BranchStrategyPort | 🔵 | 🔵 |

