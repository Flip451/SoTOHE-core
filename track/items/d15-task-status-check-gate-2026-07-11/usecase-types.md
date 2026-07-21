<!-- Generated from usecase-types.json — DO NOT EDIT DIRECTLY -->

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| PreReviewGateError | error_type | reference | TaskContractNotFound, TaskContractReadFailed, SignalReadFailed, ImplPlanReadFailed | 🔵 | 🔵 |

## Secondary Ports

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| ImplPlanReaderPort | secondary_port | reference | fn read_task_statuses(&self, track_id: &domain::TrackId) -> Result<std::collections::HashMap<domain::TaskId, domain::TaskStatusKind>, PreReviewGateError> | 🔵 | 🔵 |
| TaskContractReaderPort | secondary_port | reference | fn read(&self, track_id: &domain::TrackId) -> Result<domain::task_contract::TaskContractDocument, PreReviewGateError> | 🔵 | 🔵 |

## Interactors

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CheckTestObligationsInteractor | interactor | modify | — | 🔵 | 🔵 |
| TestObligationResultsInteractor | interactor | modify | — | 🔵 | 🔵 |

## DTOs

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CheckTestObligationsOutcome | dto | modify | — | 🔵 | 🔵 |
| TestObligationResultsOutput | dto | modify | — | 🔵 | 🔵 |
| TestObligationStatusLaneSummary | dto | add | — | 🔵 | 🔵 |

## Commands

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| TestObligationResultsCommand | command | modify | — | 🔵 | 🔵 |

