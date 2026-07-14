<!-- Generated from domain-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| LiveRustdocSnapshotStatus | enum | add | Verified, Missing, ReadFailed, ParseFailed, HashMismatch | 🔵 | 🔵 |
| RoundType | enum | reference | Fast, Final | 🔵 | 🔵 |
| ScopeName | enum | reference | Main, Other | 🔵 | 🔵 |
| TypeSignalsLoadResult | enum | modify | Current, Stale, Missing | 🔵 | 🔵 |
| TypeSignalsReuseDecision | enum | add | SkipEvaluation, ReevaluateWithSnapshot, ReextractAndEvaluate | 🔵 | 🔵 |

## Value Objects

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| BaselineHash | value_object | add | — | 🔵 | 🔵 |
| CatalogueDeclarationHash | value_object | add | — | 🔵 | 🔵 |
| EvaluatorContractHash | value_object | add | — | 🔵 | 🔵 |
| ImplementationInputHash | value_object | add | — | 🔵 | 🔵 |
| LayerId | value_object | reference | — | 🔵 | 🔵 |
| LiveRustdocSnapshotHash | value_object | add | — | 🔵 | 🔵 |
| RustdocExtractionContractHash | value_object | add | — | 🔵 | 🔵 |
| Sha256Digest | value_object | add | — | 🔵 | 🔵 |
| TrackBranch | value_object | reference | — | 🔵 | 🔵 |
| TrackId | value_object | reference | — | 🔵 | 🔵 |
| TypeSignalsCurrentInputs | value_object | add | — | 🔵 | 🔵 |
| TypeSignalsDocument | value_object | modify | — | 🔵 | 🔵 |
| TypeSignalsFreshness | value_object | add | — | 🔵 | 🔵 |
| TypeSignalsSchemaVersion | value_object | add | — | 🔵 | 🔵 |

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| Sha256DigestError | error_type | add | InvalidLength, InvalidHex | 🔵 | 🔵 |
| TypeSignalsSchemaVersionError | error_type | add | Zero | 🔵 | 🔵 |

## Free Functions

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| domain::tddd::type_signals_doc::decide_type_signals_reuse | free_function | add | fn(recorded: &TypeSignalsFreshness, current: &TypeSignalsCurrentInputs, snapshot_status: LiveRustdocSnapshotStatus) -> TypeSignalsReuseDecision | 🔵 | 🔵 |

