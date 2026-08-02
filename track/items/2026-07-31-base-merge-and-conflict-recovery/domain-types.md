<!-- Generated from domain-types.json — DO NOT EDIT DIRECTLY -->

## Value Objects

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| BaseBranchName | value_object | add | — | 🔵 | 🔵 |
| BaseMergeDirection | value_object | add | — | 🔵 | 🔵 |
| BaselineHash | value_object | add | — | 🔵 | 🔵 |
| CommitHash | value_object | reference | — | 🔵 | 🔵 |
| TypeSignalsCacheKey | value_object | add | — | 🔵 | 🔵 |
| TypeSignalsDocument | value_object | modify | — | 🔵 | 🔵 |

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| BaseMergeDirectionError | error_type | add | InactiveTrack, InvalidBaseName | 🔵 | 🔵 |

## Free Functions

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| domain::branch_strategy::derive_base_merge_direction | free_function | add | fn(track: &TrackMetadata) -> Result<BaseMergeDirection, BaseMergeDirectionError> | 🔵 | 🔵 |
| domain::tddd::type_signals_doc::decide_type_signals_reuse | free_function | modify | fn(recorded_key: &TypeSignalsCacheKey, current_key: &TypeSignalsCacheKey) -> TypeSignalsReuseDecision | 🔵 | 🔵 |

## Aggregate Roots

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| TrackMetadata | aggregate_root | reference | — | 🔵 | 🔵 |

