<!-- Generated from usecase-types.json — DO NOT EDIT DIRECTLY -->

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| PreReviewGateError | error_type | reference | TaskContractNotFound, TaskContractReadFailed, SignalReadFailed, ImplPlanReadFailed | 🔵 | 🔵 |

## Secondary Ports

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| ImplCatalogSignalReaderPort | secondary_port | reference | fn read_signals(&self, track_id: &domain::TrackId, layer: &domain::tddd::LayerId) -> Result<domain::TypeSignalsDocument, PreReviewGateError>, fn read_optional_signals(&self, track_id: &domain::TrackId, layer: &domain::tddd::LayerId) -> Result<Option<domain::TypeSignalsDocument>, PreReviewGateError> | 🔵 | 🔵 |
| ImplPlanReaderPort | secondary_port | reference | fn read_task_statuses(&self, track_id: &domain::TrackId) -> Result<std::collections::HashMap<domain::TaskId, domain::TaskStatusKind>, PreReviewGateError> | 🔵 | 🔵 |
| TaskContractReaderPort | secondary_port | reference | fn read(&self, track_id: &domain::TrackId) -> Result<domain::task_contract::TaskContractDocument, PreReviewGateError> | 🔵 | 🔵 |

## Application Services

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CoverageVerifyService | application_service | reference | fn verify_coverage(&self, cmd: CoverageVerifyCommand) -> Result<domain::task_contract::CoverageVerifyOutcome, PreReviewGateError> | 🔵 | 🔵 |
| PreReviewGateService | application_service | reference | fn check(&self, cmd: PreReviewGateCommand) -> Result<domain::task_contract::PreReviewGateOutcome, PreReviewGateError> | 🔵 | 🔵 |

## Interactors

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CoverageVerifyInteractor | interactor | modify | — | 🔵 | 🔵 |
| PreReviewGateInteractor | interactor | modify | — | 🔵 | 🔵 |

## Commands

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CoverageVerifyCommand | command | reference | — | 🔵 | 🔵 |
| PreReviewGateCommand | command | reference | — | 🔵 | 🔵 |

