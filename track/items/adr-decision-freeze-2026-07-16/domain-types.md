<!-- Generated from domain-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| AdrBaselineCheckOutcome | enum | add | Passed, Blocked | 🔵 | 🔵 |
| AdrBaselineCheckViolation | enum | add | PrimaryInitUnavailable, MissingPrimaryInit, MissingRequiredStamp, SourceMissing, BaselineCopyMissing, BaselineCopyMismatch, ByteMismatch | 🔵 | 🔵 |
| AdrBaselineKind | enum | add | Init, Cite, NewAdr, NonSemanticFix, Escalation | 🔵 | 🔵 |
| AdrBaselineLedgerEntry | enum | add | Init, Cite, NewAdr, NonSemanticFix, Escalation | 🔵 | 🔵 |
| AdrBaselineRecordedCopyStatus | enum | add | Matches, Missing, HashMismatch | 🔵 | 🔵 |
| AdrBaselineSourceState | enum | add | ExistingAtForkPoint, TrackBornDraft, TrackBornPromoted | 🔵 | 🔵 |

## Value Objects

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| AdrBaselineCheckViolations | value_object | add | — | 🔵 | 🔵 |
| AdrSourceFileName | value_object | add | — | 🔵 | 🔵 |

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| AdrBaselineCheckOutcomeError | error_type | add | EmptyViolations | 🔵 | 🔵 |
| AdrSourceFileNameError | error_type | add | InvalidFileName | 🔵 | 🔵 |

## Free Functions

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| domain::adr_baseline::is_required_stamp_satisfied | free_function | add | fn(source_state: &AdrBaselineSourceState, recorded_kinds: &[AdrBaselineKind]) -> bool | 🔵 | 🔵 |

