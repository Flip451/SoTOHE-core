<!-- Generated from usecase-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CapturedStreamOutput | enum | add | Complete, TruncatedTail | 🟡 | 🔵 |
| ProgramRunOutcome | enum | modify | Exited, TimedOut | 🟡 | 🔵 |

## Secondary Ports

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| ProgramRunnerPort | secondary_port | reference | fn run(&self, invocation: ProgramInvocation) -> Result<ProgramRunOutcome, ProgramRunnerError> | 🔵 | 🔵 |
| ReviewFixRunner | secondary_port | reference | fn run_fix(&self, command: RunReviewFixCommand) -> Result<RunReviewFixOutput, ReviewFixRunnerError> | 🔵 | 🔵 |

## DTOs

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CapturedProgramOutput | dto | modify | — | 🟡 | 🔵 |
| FailedProgramExecutionRecord | dto | reference | — | 🔵 | 🔵 |
| ProgramExecutionRecord | dto | reference | — | 🔵 | 🔵 |
| SuccessfulProgramExecutionRecord | dto | reference | — | 🔵 | 🔵 |

