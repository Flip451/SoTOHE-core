//! Typed values used to aggregate completed command events from telemetry.jsonl.

use thiserror::Error;

/// Validation errors for command metrics.
#[derive(Debug, Error)]
pub enum CommandTraceValueError {
    /// A command identity was empty.
    #[error("command identity must not be empty")]
    EmptyCommandIdentity,
    /// A command metric has no executions.
    #[error("command execution count must not be zero")]
    ZeroExecutions,
    /// A command metric has more failures than executions.
    #[error("command failure count must not exceed executions")]
    FailureCountExceedsExecutions,
    /// A command failure rate is outside the valid basis-points range.
    #[error("command failure rate must be between 0 and 10_000 basis points")]
    FailureRateOutOfRange,
}

/// A validated, non-empty command label read from a TrackSubcommand event.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SotpCommandIdentity(String);

impl SotpCommandIdentity {
    /// Creates a command identity from a non-empty value.
    pub fn try_new(value: String) -> Result<Self, CommandTraceValueError> {
        if value.is_empty() {
            return Err(CommandTraceValueError::EmptyCommandIdentity);
        }
        Ok(Self(value))
    }

    /// Returns the command identity as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A command duration measured in milliseconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommandDurationMillis(u64);

impl From<u64> for CommandDurationMillis {
    fn from(value: u64) -> Self {
        Self(value)
    }
}

impl AsRef<u64> for CommandDurationMillis {
    fn as_ref(&self) -> &u64 {
        &self.0
    }
}

/// A count of command executions or failed command executions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommandExecutionCount(u64);

impl From<u64> for CommandExecutionCount {
    fn from(value: u64) -> Self {
        Self(value)
    }
}

impl AsRef<u64> for CommandExecutionCount {
    fn as_ref(&self) -> &u64 {
        &self.0
    }
}

/// A count of telemetry records skipped during aggregation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TelemetrySkippedLineCount(u64);

impl From<u64> for TelemetrySkippedLineCount {
    fn from(value: u64) -> Self {
        Self(value)
    }
}

impl AsRef<u64> for TelemetrySkippedLineCount {
    fn as_ref(&self) -> &u64 {
        &self.0
    }
}

/// A validated command failure rate expressed in basis points.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommandFailureRateBasisPoints(u16);

impl CommandFailureRateBasisPoints {
    /// Creates a failure rate within the inclusive 0..=10_000 range.
    pub fn try_new(value: u16) -> Result<Self, CommandTraceValueError> {
        if value > 10_000 {
            return Err(CommandTraceValueError::FailureRateOutOfRange);
        }
        Ok(Self(value))
    }

    /// Returns the failure rate in basis points.
    #[must_use]
    pub fn value(&self) -> u16 {
        self.0
    }
}

/// Aggregated execution metrics for one command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandExecutionMetric {
    command: SotpCommandIdentity,
    executions: CommandExecutionCount,
    failures: CommandExecutionCount,
    total_duration: CommandDurationMillis,
    failure_rate: CommandFailureRateBasisPoints,
}

impl CommandExecutionMetric {
    /// Creates a metric from one command's aggregate values.
    pub fn new(
        command: SotpCommandIdentity,
        executions: CommandExecutionCount,
        failures: CommandExecutionCount,
        total_duration: CommandDurationMillis,
    ) -> Result<Self, CommandTraceValueError> {
        if executions.0 == 0 {
            return Err(CommandTraceValueError::ZeroExecutions);
        }
        if failures.0 > executions.0 {
            return Err(CommandTraceValueError::FailureCountExceedsExecutions);
        }

        let basis_points =
            u16::try_from((u128::from(failures.0) * 10_000) / u128::from(executions.0))
                .map_err(|_| CommandTraceValueError::FailureRateOutOfRange)?;
        let failure_rate = CommandFailureRateBasisPoints::try_new(basis_points)?;
        Ok(Self { command, executions, failures, total_duration, failure_rate })
    }

    #[must_use]
    pub fn command(&self) -> &SotpCommandIdentity {
        &self.command
    }

    #[must_use]
    pub fn executions(&self) -> CommandExecutionCount {
        self.executions
    }

    #[must_use]
    pub fn failures(&self) -> CommandExecutionCount {
        self.failures
    }

    #[must_use]
    pub fn total_duration(&self) -> CommandDurationMillis {
        self.total_duration
    }

    #[must_use]
    pub fn failure_rate(&self) -> CommandFailureRateBasisPoints {
        self.failure_rate
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity(value: &str) -> Result<SotpCommandIdentity, CommandTraceValueError> {
        SotpCommandIdentity::try_new(value.to_owned())
    }

    #[test]
    fn test_command_identity_non_empty_retains_value() -> Result<(), CommandTraceValueError> {
        assert_eq!(identity("telemetry")?.as_str(), "telemetry");
        Ok(())
    }

    #[test]
    fn test_command_identity_empty_returns_error() {
        assert!(matches!(
            SotpCommandIdentity::try_new(String::new()),
            Err(CommandTraceValueError::EmptyCommandIdentity)
        ));
    }

    #[test]
    fn test_command_metric_valid_aggregation_calculates_failure_rate()
    -> Result<(), CommandTraceValueError> {
        let metric = CommandExecutionMetric::new(
            identity("telemetry")?,
            CommandExecutionCount::from(3),
            CommandExecutionCount::from(1),
            CommandDurationMillis::from(540),
        )?;
        assert_eq!(metric.command().as_str(), "telemetry");
        assert_eq!(*metric.executions().as_ref(), 3);
        assert_eq!(*metric.failures().as_ref(), 1);
        assert_eq!(*metric.total_duration().as_ref(), 540);
        assert_eq!(metric.failure_rate().value(), 3_333);
        Ok(())
    }

    #[test]
    fn test_command_metric_zero_executions_returns_error() -> Result<(), CommandTraceValueError> {
        let result = CommandExecutionMetric::new(
            identity("telemetry")?,
            CommandExecutionCount::from(0),
            CommandExecutionCount::from(0),
            CommandDurationMillis::from(0),
        );
        assert!(matches!(result, Err(CommandTraceValueError::ZeroExecutions)));
        Ok(())
    }

    #[test]
    fn test_command_metric_failures_above_executions_returns_error()
    -> Result<(), CommandTraceValueError> {
        let result = CommandExecutionMetric::new(
            identity("telemetry")?,
            CommandExecutionCount::from(2),
            CommandExecutionCount::from(3),
            CommandDurationMillis::from(0),
        );
        assert!(matches!(result, Err(CommandTraceValueError::FailureCountExceedsExecutions)));
        Ok(())
    }

    #[test]
    fn test_command_failure_rate_out_of_range_returns_error() {
        assert!(matches!(
            CommandFailureRateBasisPoints::try_new(10_001),
            Err(CommandTraceValueError::FailureRateOutOfRange)
        ));
    }
}
