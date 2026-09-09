<!-- Generated from usecase-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| ReviewerExecutionProvenance | enum | add | PreSpawn, PostSpawn | 🔵 | 🔵 |
| ReviewerProcessDiagnostic | enum | add | Available, Unavailable | 🔵 | 🔵 |

## Value Objects

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| ReviewerDiagnostic | value_object | add | — | 🔵 | 🔵 |

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| ReviewerDiagnosticValidationError | error_type | add | Empty, TooLong | 🔵 | 🔵 |
| usecase::review_v2::error::ReviewCycleError | error_type | reference | UnknownScope, FileChangedDuringReview, Diff, PostReviewDiff, Hash, PostReviewHash, Reviewer, Reader | 🔵 | 🔵 |
| usecase::review_v2::error::ReviewerError | error_type | modify | UserAbort, ProcessFailed, Timeout, IllegalVerdict, Unexpected | 🔵 | 🔵 |

## Secondary Ports

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| Reviewer | secondary_port | reference | fn review(&self, target: &domain::review_v2::types::ReviewTarget) -> Result<(domain::review_v2::types::Verdict, domain::review_v2::types::LogInfo), ReviewerError>, fn fast_review(&self, target: &domain::review_v2::types::ReviewTarget) -> Result<(domain::review_v2::types::FastVerdict, domain::review_v2::types::LogInfo), ReviewerError> | 🔵 | 🔵 |

