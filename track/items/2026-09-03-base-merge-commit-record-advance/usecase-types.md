<!-- Generated from usecase-types.json — DO NOT EDIT DIRECTLY -->

## Value Objects

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| usecase::git_workflow::DiagnosticText | value_object | reference | — | 🔵 | 🔵 |

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| usecase::base_merge::PostMergeCleanupError | error_type | modify | Views, Baseline, CommitRecord | 🔵 | 🔵 |

## Secondary Ports

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| TrackCommitHashPort | secondary_port | reference | fn persist_current_for_track(&self, track_id: &domain::ids::TrackId) -> Result<domain::ids::CommitHash, DiagnosticText> | 🔵 | 🔵 |

## Application Services

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| TrackSetCommitHashService | application_service | reference | fn execute(&self, command: TrackSetCommitHashCommand) -> Result<TrackSetCommitHashResult, TrackSetCommitHashError> | 🔵 | 🔵 |

## Interactors

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| usecase::base_merge::BaseMergeInteractor | interactor | modify | — | 🔵 | 🔵 |
| usecase::track_lifecycle::track_set_commit_hash::TrackSetCommitHashInteractor | interactor | reference | — | 🔵 | 🔵 |

