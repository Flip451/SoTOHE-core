<!-- Generated from infrastructure-types.json — DO NOT EDIT DIRECTLY -->

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| DryCheckCommitHashError | error_type | — | Io, SymlinkDetected, Format | 🟡 | 🔵 |

## Secondary Adapters

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CodexDryChecker | secondary_adapter | — | impl DryCheckAgentPort, impl Debug | 🟡 | 🔵 |
| FsDryCheckCommitHashStore | secondary_adapter | — | impl Debug | 🟡 | 🔵 |
| FsDryCheckStore | secondary_adapter | — | impl DryCheckReader, impl DryCheckWriter, impl Debug | 🟡 | 🔵 |
| GitDryCheckDiffGetter | secondary_adapter | — | impl DryCheckDiffSource, impl Debug | 🟡 | 🔵 |

