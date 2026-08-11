<!-- Generated from domain-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CoverageVerifyOutcome | enum | modify | Passed, Blocked | 🔵 | 🔵 |
| CoverageViolation | enum | modify | MissingTaskContract, OrphanEntry, InvalidEntryRef, MissingSignalDocument, InvalidTaskRef | 🔵 | 🔵 |
| PreReviewGateOutcome | enum | modify | Passed, Blocked | 🔵 | 🔵 |
| PreReviewGateViolation | enum | modify | MissingTaskContract, NonBlueSignal | 🔵 | 🔵 |
| ScopeName | enum | modify | Main, Other | 🔵 | 🔵 |

## Value Objects

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| ContractedEntryRef | value_object | reference | — | 🔵 | 🔵 |
| MainScopeName | value_object | reference | — | 🔵 | 🔵 |
| TaskContractDocument | value_object | reference | — | 🔵 | 🔵 |

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| ScopeNameError | error_type | reference | Empty, NotAscii, Reserved | 🔵 | 🔵 |

## Free Functions

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| domain::review_v2::types::derive_review_approval_verdict | free_function | add | fn(states: impl IntoIterator<Item = (ScopeName, ReviewState)>, review_json_exists: bool) -> ReviewApprovalVerdict | 🔵 | 🔵 |

