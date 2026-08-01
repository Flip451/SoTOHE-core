<!-- Generated from usecase-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| PreReviewGateDispatchOutcome | enum | add | NotApplicable, TaskContract | 🔵 | 🔵 |
| PreReviewGateKind | enum | add | TaskContractLiveness | 🔵 | 🔵 |

## Value Objects

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| PreReviewGateMatrix | value_object | add | — | 🔵 | 🔵 |

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| PreReviewGateConfigLoadError | error_type | add | ReadFailed, InvalidMatrix | 🔵 | 🔵 |
| PreReviewGateDispatchError | error_type | add | Config, TaskContract, Lookup | 🔵 | 🔵 |
| PreReviewGateLookupError | error_type | add | UnknownScope | 🔵 | 🔵 |
| PreReviewGateMatrixError | error_type | add | MissingScope, UnknownScope, DuplicateScope, DuplicateGate | 🔵 | 🔵 |

## Secondary Ports

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| PreReviewGateConfigLoaderPort | secondary_port | add | fn load(&self, items_dir: &std::path::Path, track_id: &domain::TrackId) -> Result<PreReviewGateMatrix, PreReviewGateConfigLoadError> | 🔵 | 🔵 |

## Application Services

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| PreReviewGateDispatchService | application_service | add | fn dispatch(&self, cmd: PreReviewGateDispatchCommand) -> Result<PreReviewGateDispatchOutcome, PreReviewGateDispatchError> | 🔵 | 🔵 |

## Interactors

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| PreReviewGateDispatchInteractor | interactor | add | — | 🔵 | 🔵 |

## Commands

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| PreReviewGateDispatchCommand | command | add | — | 🔵 | 🔵 |

