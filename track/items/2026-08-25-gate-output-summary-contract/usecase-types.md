<!-- Generated from usecase-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| GateLogWriteOutcome | enum | add | Persisted, Unavailable | 🔵 | 🔵 |
| GateRunResult | enum | add | ChildExited, SpawnFailed | 🔵 | 🔵 |

## Value Objects

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| GateExitCode | value_object | add | — | 🔵 | 🔵 |

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| GateLogReservationError | error_type | add | OutsideRoot, SymlinkComponent, Clock, CreateDirectory, CreateFile, EncodedNameTooLong | 🔵 | 🔵 |
| GateLogWriteError | error_type | add | OutsideRoot, SymlinkComponent, Write | 🔵 | 🔵 |
| GateProcessError | error_type | add | Spawn | 🔵 | 🔵 |
| GateRunCommandError | error_type | add | EmptyCommand | 🔵 | 🔵 |
| GateRunError | error_type | add | Prepare | 🔵 | 🔵 |

## Secondary Ports

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| GateLogPersistencePort | secondary_port | add | fn reserve(&self, command: &GateRunCommand) -> Result<GateLogReservation, GateLogReservationError>, fn persist(&self, reservation: GateLogReservation, contents: &[u8]) -> Result<GateLogPath, GateLogWriteError> | 🔵 | 🔵 |
| GateProcessPort | secondary_port | add | fn run(&self, command: &GateRunCommand) -> Result<GateProcessOutput, GateProcessError> | 🔵 | 🔵 |

## Application Services

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| GateRunService | application_service | add | fn execute(&self, command: GateRunCommand) -> Result<GateRunResult, GateRunError> | 🔵 | 🔵 |

## Interactors

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| GateRunInteractor | interactor | add | — | 🔵 | 🔵 |

## DTOs

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| GateAdapterFailureReason | dto | add | — | 🔵 | 🔵 |
| GateLogPath | dto | add | — | 🔵 | 🔵 |
| GateLogReservation | dto | add | — | 🔵 | 🔵 |
| GateProcessOutput | dto | add | — | 🔵 | 🔵 |

## Commands

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| GateRunCommand | command | add | — | 🔵 | 🔵 |

