//! Generic, bounded process-runner port.

use std::path::PathBuf;

use domain::FreeText;
use thiserror::Error;

use crate::operator_command::{
    CommandArgv, CommandSequenceIndex, CommandTimeoutSeconds, ConfiguredCommand,
    OutputCaptureLimitBytes,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramExitCode(i32);
impl ProgramExitCode {
    #[must_use]
    pub fn new(value: i32) -> Self {
        Self(value)
    }
    #[must_use]
    pub fn as_i32(&self) -> i32 {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapturedProgramOutput {
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramInvocation {
    pub argv: CommandArgv,
    pub repository_root: PathBuf,
    pub timeout: CommandTimeoutSeconds,
    pub stdout_limit: OutputCaptureLimitBytes,
    pub stderr_limit: OutputCaptureLimitBytes,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProgramOutputStream {
    Stdout,
    Stderr,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProgramRunOutcome {
    Exited { exit_code: ProgramExitCode, output: CapturedProgramOutput },
    TimedOut { output: CapturedProgramOutput },
    OutputLimitExceeded { stream: ProgramOutputStream, output: CapturedProgramOutput },
}

#[derive(Debug, Error)]
pub enum ProgramRunnerError {
    #[error("program could not be spawned: {message}")]
    SpawnFailed { message: FreeText },
    #[error("program could not be awaited: {message}")]
    WaitFailed { message: FreeText },
    #[error("program could not be terminated: {message}")]
    TerminateFailed { message: FreeText },
}

pub trait ProgramRunnerPort: Send + Sync {
    fn run(&self, invocation: ProgramInvocation) -> Result<ProgramRunOutcome, ProgramRunnerError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramExecutionRecord {
    pub sequence_index: CommandSequenceIndex,
    pub command: ConfiguredCommand,
    pub outcome: ProgramRunOutcome,
}

/// A program execution record whose outcome is known to be a zero exit.
///
/// The inner record is private so callers cannot construct a successful record
/// from a timeout, output-limit, or non-zero-exit outcome.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SuccessfulProgramExecutionRecord {
    record: ProgramExecutionRecord,
}

impl AsRef<ProgramExecutionRecord> for SuccessfulProgramExecutionRecord {
    fn as_ref(&self) -> &ProgramExecutionRecord {
        &self.record
    }
}

/// A program execution record whose outcome is known to prevent review.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FailedProgramExecutionRecord {
    record: ProgramExecutionRecord,
}

impl AsRef<ProgramExecutionRecord> for FailedProgramExecutionRecord {
    fn as_ref(&self) -> &ProgramExecutionRecord {
        &self.record
    }
}

/// Classifies a completed program record into the only two review-authorizing states.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClassifiedProgramExecutionRecord {
    Succeeded(SuccessfulProgramExecutionRecord),
    Failed(FailedProgramExecutionRecord),
}

impl ProgramExecutionRecord {
    #[must_use]
    pub fn classify(self) -> ClassifiedProgramExecutionRecord {
        match &self.outcome {
            ProgramRunOutcome::Exited { exit_code, .. } if exit_code.as_i32() == 0 => {
                ClassifiedProgramExecutionRecord::Succeeded(SuccessfulProgramExecutionRecord {
                    record: self,
                })
            }
            ProgramRunOutcome::Exited { .. }
            | ProgramRunOutcome::TimedOut { .. }
            | ProgramRunOutcome::OutputLimitExceeded { .. } => {
                ClassifiedProgramExecutionRecord::Failed(FailedProgramExecutionRecord {
                    record: self,
                })
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::panic, clippy::unwrap_used)]
mod tests {
    use crate::operator_command::{CommandArgument, ConfiguredCommand};

    use super::{
        CapturedProgramOutput, ClassifiedProgramExecutionRecord, ProgramExecutionRecord,
        ProgramExitCode, ProgramOutputStream, ProgramRunOutcome,
    };

    fn record(outcome: ProgramRunOutcome) -> ProgramExecutionRecord {
        ProgramExecutionRecord {
            sequence_index: crate::operator_command::CommandSequenceIndex::new(0),
            command: ConfiguredCommand::try_new(
                vec![CommandArgument::try_new("command".to_owned())],
                None,
            )
            .unwrap(),
            outcome,
        }
    }

    fn output() -> CapturedProgramOutput {
        CapturedProgramOutput { stdout: Vec::new(), stderr: Vec::new() }
    }

    #[test]
    fn test_program_execution_record_classify_zero_exit_returns_succeeded() {
        let classified = record(ProgramRunOutcome::Exited {
            exit_code: ProgramExitCode::new(0),
            output: output(),
        })
        .classify();

        match classified {
            ClassifiedProgramExecutionRecord::Succeeded(success) => {
                assert_eq!(success.as_ref().sequence_index.as_usize(), 0);
            }
            ClassifiedProgramExecutionRecord::Failed(failure) => {
                panic!("expected successful execution record, got {failure:?}");
            }
        }
    }

    #[test]
    fn test_program_execution_record_classify_non_zero_exit_returns_non_zero_failure() {
        let classified = record(ProgramRunOutcome::Exited {
            exit_code: ProgramExitCode::new(1),
            output: output(),
        })
        .classify();

        match classified {
            ClassifiedProgramExecutionRecord::Failed(failure) => {
                assert!(matches!(
                    failure.as_ref().outcome,
                    ProgramRunOutcome::Exited { ref exit_code, .. } if exit_code.as_i32() != 0
                ));
            }
            ClassifiedProgramExecutionRecord::Succeeded(success) => {
                panic!("expected non-zero failure record, got {success:?}");
            }
        }
    }

    #[test]
    fn test_program_execution_record_classify_output_limit_returns_output_limit_failure() {
        let classified = record(ProgramRunOutcome::OutputLimitExceeded {
            stream: ProgramOutputStream::Stdout,
            output: output(),
        })
        .classify();

        match classified {
            ClassifiedProgramExecutionRecord::Failed(failure) => {
                assert!(matches!(
                    failure.as_ref().outcome,
                    ProgramRunOutcome::OutputLimitExceeded { .. }
                ));
            }
            ClassifiedProgramExecutionRecord::Succeeded(success) => {
                panic!("expected output-limit failure record, got {success:?}");
            }
        }
    }

    #[test]
    fn test_program_execution_record_classify_timeout_returns_timed_out_failure() {
        let classified = record(ProgramRunOutcome::TimedOut { output: output() }).classify();

        match classified {
            ClassifiedProgramExecutionRecord::Failed(failure) => {
                assert!(matches!(failure.as_ref().outcome, ProgramRunOutcome::TimedOut { .. }));
            }
            ClassifiedProgramExecutionRecord::Succeeded(success) => {
                panic!("expected timed-out failure record, got {success:?}");
            }
        }
    }
}
