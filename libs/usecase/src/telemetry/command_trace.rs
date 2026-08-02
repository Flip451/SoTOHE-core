//! Typed values for a completed `sotp` command trace.

use std::num::NonZeroU8;
use std::sync::Arc;

use thiserror::Error;

/// Validation errors for command-trace values.
#[derive(Debug, Error)]
pub enum CommandTraceValueError {
    /// A command identity was empty.
    #[error("command identity must not be empty")]
    EmptyCommandIdentity,
    /// A failed command was given the success exit code.
    #[error("failed command exit code must not be zero")]
    ZeroExitCode,
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

/// A validated, non-empty `sotp` command identity.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SotpCommandIdentity(String);

impl SotpCommandIdentity {
    /// Creates a command identity from a non-empty value.
    ///
    /// # Errors
    /// Returns [`CommandTraceValueError::EmptyCommandIdentity`] when `value` is empty.
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
    ///
    /// # Errors
    ///
    /// Returns [`CommandTraceValueError::FailureRateOutOfRange`] when `value`
    /// exceeds 10_000 basis points.
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

/// A validated non-zero exit code for a failed command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommandExitCode(NonZeroU8);

impl CommandExitCode {
    /// Creates a CLI-representable failed-command exit code from a non-zero value.
    ///
    /// # Errors
    /// Returns [`CommandTraceValueError::ZeroExitCode`] when `value` is zero.
    pub fn try_new(value: u8) -> Result<Self, CommandTraceValueError> {
        NonZeroU8::new(value).map(Self).ok_or(CommandTraceValueError::ZeroExitCode)
    }

    /// Returns the exit code.
    #[must_use]
    pub fn value(&self) -> u8 {
        self.0.get()
    }
}

/// The finite result of a completed command execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandExecutionResult {
    /// The command completed successfully.
    Success,
    /// The command failed with a validated non-zero exit code.
    Failure(CommandExitCode),
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
    ///
    /// # Errors
    ///
    /// Returns [`CommandTraceValueError::ZeroExecutions`] when `executions` is
    /// zero, or [`CommandTraceValueError::FailureCountExceedsExecutions`] when
    /// `failures` exceeds `executions`.
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

    /// Returns the command identity.
    #[must_use]
    pub fn command(&self) -> &SotpCommandIdentity {
        &self.command
    }

    /// Returns the number of executions.
    #[must_use]
    pub fn executions(&self) -> CommandExecutionCount {
        self.executions
    }

    /// Returns the number of failed executions.
    #[must_use]
    pub fn failures(&self) -> CommandExecutionCount {
        self.failures
    }

    /// Returns the total execution duration.
    #[must_use]
    pub fn total_duration(&self) -> CommandDurationMillis {
        self.total_duration
    }

    /// Returns the failure rate in basis points.
    #[must_use]
    pub fn failure_rate(&self) -> CommandFailureRateBasisPoints {
        self.failure_rate
    }
}

/// Typed information about one completed command execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandTraceRecord {
    /// The completed command's identity.
    pub command: SotpCommandIdentity,
    /// The command's elapsed duration.
    pub duration: CommandDurationMillis,
    /// The completed command's result.
    pub result: CommandExecutionResult,
}

/// Error returned when a completed command trace cannot be recorded.
#[derive(Debug, Error)]
pub enum CommandTraceWriteError {
    /// The trace writer could not complete the recording operation.
    #[error("command trace writer unavailable")]
    Unavailable,
}

/// Primary application boundary for recording one completed command execution.
pub trait CommandTraceService: Send + Sync {
    /// Records a completed command execution.
    ///
    /// # Errors
    ///
    /// Returns [`CommandTraceWriteError`] when the injected trace writer cannot
    /// complete the recording operation.
    fn record(&self, record: CommandTraceRecord) -> Result<(), CommandTraceWriteError>;
}

/// Secondary port for recording one completed command trace.
pub trait CommandTraceWriterPort: Send + Sync {
    /// Records a completed command trace through the configured persistence boundary.
    ///
    /// # Errors
    ///
    /// Returns [`CommandTraceWriteError`] when recording cannot complete.
    fn record(&self, record: CommandTraceRecord) -> Result<(), CommandTraceWriteError>;
}

/// Interactor implementing [`CommandTraceService`] through an injected writer port.
pub struct CommandTraceInteractor {
    writer: Arc<dyn CommandTraceWriterPort>,
}

impl CommandTraceInteractor {
    /// Creates an interactor that records traces through `writer`.
    #[must_use]
    pub fn new(writer: Arc<dyn CommandTraceWriterPort>) -> Self {
        Self { writer }
    }
}

impl CommandTraceService for CommandTraceInteractor {
    fn record(&self, record: CommandTraceRecord) -> Result<(), CommandTraceWriteError> {
        self.writer.record(record)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::{
        CommandDurationMillis, CommandExecutionCount, CommandExecutionMetric,
        CommandExecutionResult, CommandExitCode, CommandFailureRateBasisPoints,
        CommandTraceInteractor, CommandTraceRecord, CommandTraceService, CommandTraceValueError,
        CommandTraceWriteError, CommandTraceWriterPort, SotpCommandIdentity,
        TelemetrySkippedLineCount,
    };

    #[derive(Default)]
    struct RecordingWriter {
        records: Mutex<Vec<CommandTraceRecord>>,
    }

    impl CommandTraceWriterPort for RecordingWriter {
        fn record(&self, record: CommandTraceRecord) -> Result<(), CommandTraceWriteError> {
            self.records
                .lock()
                .map(|mut records| records.push(record))
                .map_err(|_| CommandTraceWriteError::Unavailable)
        }
    }

    struct UnavailableWriter;

    impl CommandTraceWriterPort for UnavailableWriter {
        fn record(&self, _: CommandTraceRecord) -> Result<(), CommandTraceWriteError> {
            Err(CommandTraceWriteError::Unavailable)
        }
    }

    fn identity(value: &str) -> Result<SotpCommandIdentity, CommandTraceValueError> {
        SotpCommandIdentity::try_new(value.to_owned())
    }

    fn exit_code(value: u8) -> Result<CommandExitCode, CommandTraceValueError> {
        CommandExitCode::try_new(value)
    }

    #[test]
    fn test_sotp_command_identity_non_empty_retains_value() -> Result<(), CommandTraceValueError> {
        let identity = identity("track status")?;

        assert_eq!(identity.as_str(), "track status");
        Ok(())
    }

    #[test]
    fn test_sotp_command_identity_empty_returns_empty_command_identity_error() {
        let result = SotpCommandIdentity::try_new(String::new());

        assert!(matches!(result, Err(CommandTraceValueError::EmptyCommandIdentity)));
    }

    #[test]
    fn test_command_duration_millis_from_u64_exposes_value() {
        let duration = CommandDurationMillis::from(275_u64);

        assert_eq!(*duration.as_ref(), 275);
    }

    #[test]
    fn test_command_execution_count_from_u64_exposes_value() {
        let count = CommandExecutionCount::from(275_u64);

        assert_eq!(*count.as_ref(), 275);
    }

    #[test]
    fn test_telemetry_skipped_line_count_from_u64_exposes_value() {
        let count = TelemetrySkippedLineCount::from(3_u64);

        assert_eq!(*count.as_ref(), 3);
    }

    #[test]
    fn test_command_failure_rate_basis_points_maximum_retains_value()
    -> Result<(), CommandTraceValueError> {
        let rate = CommandFailureRateBasisPoints::try_new(10_000)?;

        assert_eq!(rate.value(), 10_000);
        Ok(())
    }

    #[test]
    fn test_command_failure_rate_basis_points_above_maximum_returns_range_error() {
        let result = CommandFailureRateBasisPoints::try_new(10_001);

        assert!(matches!(result, Err(CommandTraceValueError::FailureRateOutOfRange)));
    }

    #[test]
    fn test_command_execution_metric_valid_aggregation_exposes_typed_values()
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
    fn test_command_execution_metric_zero_executions_returns_error()
    -> Result<(), CommandTraceValueError> {
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
    fn test_command_execution_metric_failures_above_executions_returns_error()
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
    fn test_command_execution_metric_maximum_counts_calculates_bounded_failure_rate()
    -> Result<(), CommandTraceValueError> {
        let metric = CommandExecutionMetric::new(
            identity("telemetry")?,
            CommandExecutionCount::from(u64::MAX),
            CommandExecutionCount::from(u64::MAX),
            CommandDurationMillis::from(u64::MAX),
        )?;

        assert_eq!(metric.failure_rate().value(), 10_000);
        Ok(())
    }

    #[test]
    fn test_command_exit_code_nonzero_retains_value() -> Result<(), CommandTraceValueError> {
        let exit_code = exit_code(u8::MAX)?;

        assert_eq!(exit_code.value(), u8::MAX);
        Ok(())
    }

    #[test]
    fn test_command_exit_code_zero_returns_zero_exit_code_error() {
        let result = CommandExitCode::try_new(0);

        assert!(matches!(result, Err(CommandTraceValueError::ZeroExitCode)));
    }

    #[test]
    fn test_command_execution_result_failure_retains_exit_code()
    -> Result<(), CommandTraceValueError> {
        let result = CommandExecutionResult::Failure(exit_code(1)?);

        assert_eq!(result, CommandExecutionResult::Failure(exit_code(1)?));
        Ok(())
    }

    #[test]
    fn test_command_trace_record_validated_values_preserves_fields()
    -> Result<(), CommandTraceValueError> {
        let record = CommandTraceRecord {
            command: identity("telemetry")?,
            duration: CommandDurationMillis::from(36_u64),
            result: CommandExecutionResult::Success,
        };

        assert_eq!(record.command.as_str(), "telemetry");
        assert_eq!(*record.duration.as_ref(), 36);
        assert_eq!(record.result, CommandExecutionResult::Success);
        Ok(())
    }

    #[test]
    fn test_command_trace_service_record_success_delegates_validated_record()
    -> Result<(), Box<dyn std::error::Error>> {
        let writer = Arc::new(RecordingWriter::default());
        let interactor = CommandTraceInteractor::new(writer.clone());
        let service: &dyn CommandTraceService = &interactor;
        let record = CommandTraceRecord {
            command: identity("track status")?,
            duration: CommandDurationMillis::from(125_u64),
            result: CommandExecutionResult::Success,
        };
        let expected = record.clone();

        assert!(service.record(record).is_ok());

        let records = writer
            .records
            .lock()
            .map_err(|_| std::io::Error::other("recording writer lock poisoned"))?;
        assert_eq!(records.as_slice(), &[expected]);
        Ok(())
    }

    #[test]
    fn test_command_trace_writer_port_record_success_accepts_completed_record()
    -> Result<(), Box<dyn std::error::Error>> {
        let writer = Arc::new(RecordingWriter::default());
        let port: &dyn CommandTraceWriterPort = writer.as_ref();
        let record = CommandTraceRecord {
            command: identity("telemetry")?,
            duration: CommandDurationMillis::from(42_u64),
            result: CommandExecutionResult::Failure(exit_code(1)?),
        };
        let expected = record.clone();

        assert!(port.record(record).is_ok());

        let records = writer
            .records
            .lock()
            .map_err(|_| std::io::Error::other("recording writer lock poisoned"))?;
        assert_eq!(records.as_slice(), &[expected]);
        Ok(())
    }

    #[test]
    fn test_command_trace_interactor_record_writer_unavailable_propagates_error()
    -> Result<(), Box<dyn std::error::Error>> {
        let interactor = CommandTraceInteractor::new(Arc::new(UnavailableWriter));
        let service: &dyn CommandTraceService = &interactor;
        let record = CommandTraceRecord {
            command: identity("telemetry")?,
            duration: CommandDurationMillis::from(42_u64),
            result: CommandExecutionResult::Failure(exit_code(1)?),
        };

        let result = service.record(record);

        assert!(matches!(result, Err(CommandTraceWriteError::Unavailable)));
        Ok(())
    }
}
