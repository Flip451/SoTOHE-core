<!-- Generated from usecase-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| ReviewerProcessDiagnostic | enum | add | Available, Unavailable | 🟡 | 🔵 |

## Value Objects

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| ReviewerDiagnostic | value_object | add | — | 🟡 | 🔵 |

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| ReviewerDiagnosticValidationError | error_type | add | Empty, TooLong | 🟡 | 🔵 |
| usecase::review_v2::error::ReviewCycleError | error_type | reference | UnknownScope, FileChangedDuringReview, Diff, PostReviewDiff, Hash, PostReviewHash, Reviewer, Reader | 🔵 | 🔵 |
| usecase::review_v2::error::ReviewerError | error_type | modify | UserAbort, ProcessFailed, Timeout, IllegalVerdict, Unexpected | 🟡 | 🔵 |

