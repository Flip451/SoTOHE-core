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
pub struct SuccessfulProgramExecutionRecord(ProgramExecutionRecord);

impl SuccessfulProgramExecutionRecord {
    #[must_use]
    pub fn record(&self) -> &ProgramExecutionRecord {
        &self.0
    }
}

/// A program execution record whose outcome is known to prevent review.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FailedProgramExecutionRecord {
    NonZeroExit(ProgramExecutionRecord),
    TimedOut(ProgramExecutionRecord),
    OutputLimitExceeded(ProgramExecutionRecord),
}

impl FailedProgramExecutionRecord {
    #[must_use]
    pub fn record(&self) -> &ProgramExecutionRecord {
        match self {
            Self::NonZeroExit(record)
            | Self::TimedOut(record)
            | Self::OutputLimitExceeded(record) => record,
        }
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
                ClassifiedProgramExecutionRecord::Succeeded(SuccessfulProgramExecutionRecord(self))
            }
            ProgramRunOutcome::Exited { .. } => ClassifiedProgramExecutionRecord::Failed(
                FailedProgramExecutionRecord::NonZeroExit(self),
            ),
            ProgramRunOutcome::TimedOut { .. } => ClassifiedProgramExecutionRecord::Failed(
                FailedProgramExecutionRecord::TimedOut(self),
            ),
            ProgramRunOutcome::OutputLimitExceeded { .. } => {
                ClassifiedProgramExecutionRecord::Failed(
                    FailedProgramExecutionRecord::OutputLimitExceeded(self),
                )
            }
        }
    }
}
