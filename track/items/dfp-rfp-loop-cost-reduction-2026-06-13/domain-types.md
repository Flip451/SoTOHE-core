<!-- Generated from domain-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| FixpointStep | enum | — | RunDfp, RunRfp, RunRefVerify, Commit | 🔵 | 🔵 |

## Value Objects

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| DryCheckConfigFingerprint | value_object | — | — | 🟡 | 🔵 |
| DryCheckCorpusFingerprint | value_object | — | — | 🟡 | 🟡 |
| DryCheckCoverageRecord | value_object | — | — | 🟡 | 🔵 |
| DryCheckEntry | value_object | modify | — | 🔵 | 🔵 |
| DryCheckRecord | value_object | modify | — | 🔵 | 🔵 |
| ReviewScopeSet | value_object | — | — | 🟡 | 🔵 |

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| DryCheckConfigFingerprintError | error_type | — | InvalidFormat | 🔵 | 🟡 |
| DryCheckCorpusFingerprintError | error_type | — | InvalidFormat | 🔵 | 🟡 |
| DryCheckEntryError | error_type | reference | ChangedPathOutsidePair | 🔵 | 🔵 |
| DryCheckRecordError | error_type | reference | ChangedPathOutsidePair | 🔵 | 🔵 |
| ReviewScopeSetError | error_type | — | Empty | 🔵 | 🔵 |

