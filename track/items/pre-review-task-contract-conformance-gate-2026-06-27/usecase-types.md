<!-- Generated from usecase-types.json — DO NOT EDIT DIRECTLY -->

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| PreReviewGateError | error_type | add | TaskContractNotFound, TaskContractReadFailed, SignalReadFailed | 🟡 | 🔵 |

## Secondary Ports

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| ImplCatalogSignalReaderPort | secondary_port | add | fn read_signals(&self, track_id: &domain::TrackId, layer: &domain::tddd::LayerId) -> Result<domain::TypeSignalsDocument, PreReviewGateError> | 🟡 | 🔵 |
| TaskContractReaderPort | secondary_port | add | fn read(&self, track_id: &domain::TrackId) -> Result<domain::task_contract::TaskContractDocument, PreReviewGateError> | 🟡 | 🔵 |

## Application Services

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| PreReviewGateService | application_service | add | fn check(&self, cmd: PreReviewGateCommand) -> Result<domain::task_contract::PreReviewGateOutcome, PreReviewGateError> | 🟡 | 🔵 |

## Interactors

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| PreReviewGateInteractor | interactor | add | — | 🟡 | 🔵 |

## Commands

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| PreReviewGateCommand | command | add | — | 🟡 | 🔵 |

