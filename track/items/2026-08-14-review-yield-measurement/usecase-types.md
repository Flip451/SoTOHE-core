<!-- Generated from usecase-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| ReviewYieldValue | enum | add | Scope, RoundType, Provider, Model, ReasoningEffort | 🔵 | 🔵 |

## Value Objects

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| ResolvedReviewerAssignment | value_object | add | — | 🔵 | 🔵 |
| ReviewDetectionRateBasisPoints | value_object | add | — | 🔵 | 🔵 |
| ReviewExecutionCount | value_object | add | — | 🔵 | 🔵 |
| ReviewFindingCount | value_object | add | — | 🔵 | 🔵 |

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| ReviewCycleError | error_type | modify | UnknownScope, FileChangedDuringReview, Diff, PostReviewDiff, Hash, PostReviewHash, Reviewer, Reader | 🔵 | 🔵 |
| ReviewYieldValueError | error_type | add | DetectionRateOutOfRange | 🔵 | 🔵 |

## Secondary Ports

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| ResolvedReviewer | secondary_port | add | fn resolved_assignment(&self) -> &ResolvedReviewerAssignment | 🔵 | 🔵 |
| Reviewer | secondary_port | reference | fn review(&self, target: &domain::review_v2::ReviewTarget) -> Result<(domain::review_v2::Verdict, domain::review_v2::LogInfo), ReviewerError>, fn fast_review(&self, target: &domain::review_v2::ReviewTarget) -> Result<(domain::review_v2::FastVerdict, domain::review_v2::LogInfo), ReviewerError> | 🔵 | 🔵 |

## DTOs

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| ReviewYieldMetric | dto | add | — | 🔵 | 🔵 |
| TelemetryReportOutput | dto | modify | — | 🔵 | 🔵 |

