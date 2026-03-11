<!-- Generated from metadata.json — DO NOT EDIT DIRECTLY -->
# Track state machine: DMMF domain model

Implement a DMMF-style Rust domain model for the track state machine.
Make illegal states unrepresentable using Rust enums and newtype patterns.
Track status is derived from task states, not stored as a mutable field.

## Domain layer: types, validation, state machine

Newtype IDs (TrackId, TaskId, CommitHash) with validation.
Error hierarchy (DomainError > ValidationError / TransitionError / RepositoryError).
TaskStatus enum with Done{commit_hash} data, TaskTransition commands.
TrackTask aggregate with transition() enforcing state machine edges.
TrackMetadata aggregate with derived status and StatusOverride.
PlanView/PlanSection with plan-task referential integrity.
TrackRepository trait (port).

- [x] Implement domain layer: IDs, errors, track/task types, state machine, repository trait d17fd41

## Usecase layer: track operations

SaveTrackUseCase, LoadTrackUseCase, TransitionTaskUseCase.

- [x] Implement usecase layer: Save, Load, TransitionTask use cases d17fd41

## Infrastructure layer: in-memory repository

InMemoryTrackRepository implementing TrackRepository with Mutex<HashMap>.

- [x] Implement infrastructure layer: InMemoryTrackRepository d17fd41

## CLI integration and tests

Update CLI main.rs to use new domain model.
Unit tests for domain, usecase, infrastructure, and CLI.

- [x] Update CLI and add comprehensive tests across all layers d17fd41
