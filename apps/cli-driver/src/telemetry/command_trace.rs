//! Primary adapter for completed-command trace recording.
//!
//! The driver keeps tracing observational: a failed trace write never changes the
//! completed command's outcome. The usecase service still receives and returns its
//! typed write error at its boundary.

use std::sync::Arc;

use usecase::telemetry::command_trace::{
    CommandTraceRecord, CommandTraceService, CommandTraceWriteError,
};

use crate::render::CommandOutcome;

/// Primary adapter that records one completed command through the usecase boundary.
pub struct CommandTraceDriver {
    service: Arc<dyn CommandTraceService>,
}

impl CommandTraceDriver {
    /// Creates a trace driver with the supplied application service.
    #[must_use]
    pub fn new(service: Arc<dyn CommandTraceService>) -> Self {
        Self { service }
    }

    /// Records a completed command and returns its outcome.
    ///
    /// A trace-write failure preserves the completed command's stdout and exit code,
    /// then appends its rendered diagnostic to stderr.
    #[must_use]
    pub fn handle(&self, outcome: CommandOutcome, record: CommandTraceRecord) -> CommandOutcome {
        match self.service.record(record) {
            Ok(()) => outcome,
            Err(error) => Self::with_trace_write_diagnostic(outcome, error),
        }
    }

    fn with_trace_write_diagnostic(
        outcome: CommandOutcome,
        error: CommandTraceWriteError,
    ) -> CommandOutcome {
        let CommandOutcome { stdout, stderr, exit_code } = outcome;
        let diagnostic = error.to_string();
        let stderr = match stderr {
            Some(existing) => format!("{existing}\n{diagnostic}"),
            None => diagnostic,
        };

        CommandOutcome { stdout, stderr: Some(stderr), exit_code }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use usecase::telemetry::command_trace::{
        CommandDurationMillis, CommandExecutionResult, CommandExitCode, CommandTraceRecord,
        CommandTraceService, CommandTraceValueError, CommandTraceWriteError, SotpCommandIdentity,
    };

    use super::CommandTraceDriver;
    use crate::render::CommandOutcome;

    #[derive(Default)]
    struct RecordingService {
        records: Mutex<Vec<CommandTraceRecord>>,
    }

    impl CommandTraceService for RecordingService {
        fn record(&self, record: CommandTraceRecord) -> Result<(), CommandTraceWriteError> {
            self.records
                .lock()
                .map(|mut records| records.push(record))
                .map_err(|_| CommandTraceWriteError::Unavailable)
        }
    }

    struct UnavailableService;

    impl CommandTraceService for UnavailableService {
        fn record(&self, _: CommandTraceRecord) -> Result<(), CommandTraceWriteError> {
            Err(CommandTraceWriteError::Unavailable)
        }
    }

    fn success_record() -> Result<CommandTraceRecord, CommandTraceValueError> {
        Ok(CommandTraceRecord {
            command: SotpCommandIdentity::try_new("track status".to_owned())?,
            duration: CommandDurationMillis::from(125_u64),
            result: CommandExecutionResult::Success,
        })
    }

    fn failure_record(exit_code: u8) -> Result<CommandTraceRecord, CommandTraceValueError> {
        Ok(CommandTraceRecord {
            command: SotpCommandIdentity::try_new("track status".to_owned())?,
            duration: CommandDurationMillis::from(125_u64),
            result: CommandExecutionResult::Failure(CommandExitCode::try_new(exit_code)?),
        })
    }

    #[test]
    fn test_command_trace_driver_success_submits_one_record_and_preserves_outcome()
    -> Result<(), Box<dyn std::error::Error>> {
        let service = Arc::new(RecordingService::default());
        let driver = CommandTraceDriver::new(service.clone());
        let record = success_record()?;
        let expected = record.clone();
        let expected_outcome = CommandOutcome {
            stdout: Some("completed output".to_owned()),
            stderr: Some("completed diagnostic".to_owned()),
            exit_code: 0,
        };

        let outcome = driver.handle(expected_outcome.clone(), record);

        assert_eq!(outcome.exit_code, expected_outcome.exit_code);
        assert_eq!(outcome.stdout, expected_outcome.stdout);
        assert_eq!(outcome.stderr, expected_outcome.stderr);
        let records = service
            .records
            .lock()
            .map_err(|_| std::io::Error::other("recording service lock poisoned"))?;
        assert_eq!(records.as_slice(), &[expected]);
        Ok(())
    }

    #[test]
    fn test_command_trace_driver_write_failure_preserves_command_failure_outcome()
    -> Result<(), Box<dyn std::error::Error>> {
        let driver = CommandTraceDriver::new(Arc::new(UnavailableService));
        let expected_outcome = CommandOutcome {
            stdout: Some("partial output".to_owned()),
            stderr: Some("command failure".to_owned()),
            exit_code: u8::MAX,
        };

        let outcome = driver.handle(expected_outcome.clone(), failure_record(u8::MAX)?);

        assert_eq!(outcome.exit_code, expected_outcome.exit_code);
        assert_eq!(outcome.stdout, expected_outcome.stdout);
        let stderr =
            outcome.stderr.as_deref().ok_or("trace write failure did not render stderr")?;
        assert!(stderr.contains("command failure"));
        assert!(stderr.contains("command trace writer unavailable"));
        Ok(())
    }
}
