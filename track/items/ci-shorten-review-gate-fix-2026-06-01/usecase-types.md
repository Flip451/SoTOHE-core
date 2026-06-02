<!-- Generated from usecase-types.json — DO NOT EDIT DIRECTLY -->

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| RunReviewFixError | error_type | modify | InvalidScope, InvalidTrackId, InvalidRoundType, SmokeTestFailed, FixRunnerFailed | 🟡 | 🔵 |

## Secondary Ports

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| ReviewFixRunner | secondary_port | reference | fn run_fix(&self, command: RunReviewFixCommand) -> Result<RunReviewFixOutput, ReviewFixRunnerError> | 🔵 | 🔵 |

## Commands

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| BaselineCaptureRequest | command | modify | — | 🔵 | 🔵 |
| RunReviewFixCommand | command | modify | — | 🟡 | 🔵 |
| TypeSignalsRequest | command | modify | — | 🔵 | 🔵 |

