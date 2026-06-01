<!-- Generated from usecase-types.json — DO NOT EDIT DIRECTLY -->

## Value Objects

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| RunReviewFixOutput | value_object | — | — | 🔵 | 🔵 |

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| ReviewFixRunnerError | error_type | — | SmokeTestFailed, SpawnFailed, SentinelNotFound, Unexpected | 🔵 | 🔵 |
| RunReviewFixError | error_type | — | InvalidScope, InvalidTrackId, InvalidRoundType, EmptyScopeFiles, SmokeTestFailed, FixRunnerFailed | 🔵 | 🔵 |

## Secondary Ports

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| ReviewFixRunner | secondary_port | — | fn run_fix(&self, command: RunReviewFixCommand) -> Result<RunReviewFixOutput, ReviewFixRunnerError> | 🔵 | 🔵 |

## Application Services

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| RunReviewFixService | application_service | — | fn run(&self, command: RunReviewFixCommand) -> Result<RunReviewFixOutput, RunReviewFixError> | 🔵 | 🔵 |

## Interactors

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| RunReviewFixInteractor | interactor | — | — | 🔵 | 🔵 |

## Commands

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| RunReviewFixCommand | command | — | — | 🔵 | 🔵 |

