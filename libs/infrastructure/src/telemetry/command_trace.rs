//! Typed configuration for local command-trace file rotation.

use thiserror::Error;

/// A positive byte limit for an active command-trace file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommandTraceFileSizeLimitBytes(u64);

impl CommandTraceFileSizeLimitBytes {
    /// Constructs a positive file-size limit.
    pub fn try_new(value: u64) -> Result<Self, CommandTracePolicyError> {
        if value == 0 {
            return Err(CommandTracePolicyError::ZeroFileSizeLimit);
        }

        Ok(Self(value))
    }

    /// Returns the configured number of bytes.
    pub fn value(&self) -> u64 {
        self.0
    }
}

/// A positive count of rotated command-trace files to retain.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommandTraceRetainedFileCount(u16);

impl CommandTraceRetainedFileCount {
    /// Constructs a positive retained-file count.
    pub fn try_new(value: u16) -> Result<Self, CommandTracePolicyError> {
        if value == 0 {
            return Err(CommandTracePolicyError::ZeroRetainedFileCount);
        }

        Ok(Self(value))
    }

    /// Returns the configured number of files to retain.
    pub fn value(&self) -> u16 {
        self.0
    }
}

/// Failures constructing typed command-trace rotation-policy values.
#[derive(Debug, Error)]
pub enum CommandTracePolicyError {
    /// The active file-size limit was zero.
    #[error("command trace file-size limit must be greater than zero")]
    ZeroFileSizeLimit,

    /// The rotated-file retention count was zero.
    #[error("command trace retained-file count must be greater than zero")]
    ZeroRetainedFileCount,
}

/// Typed configuration for local command-trace file rotation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommandTraceRotationPolicy {
    /// Maximum byte size of the active file before rotation.
    pub max_file_size: CommandTraceFileSizeLimitBytes,
    /// Number of rotated files to retain.
    pub retained_files: CommandTraceRetainedFileCount,
}

impl CommandTraceRotationPolicy {
    /// Combines validated rotation bounds into one policy.
    pub fn new(
        max_file_size: CommandTraceFileSizeLimitBytes,
        retained_files: CommandTraceRetainedFileCount,
    ) -> Self {
        Self { max_file_size, retained_files }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_file_size_limit() -> Result<CommandTraceFileSizeLimitBytes, CommandTracePolicyError> {
        CommandTraceFileSizeLimitBytes::try_new(4_096)
    }

    fn valid_retained_file_count() -> Result<CommandTraceRetainedFileCount, CommandTracePolicyError>
    {
        CommandTraceRetainedFileCount::try_new(3)
    }

    #[test]
    fn test_command_trace_file_size_limit_bytes_zero_returns_zero_file_size_limit() {
        assert!(matches!(
            CommandTraceFileSizeLimitBytes::try_new(0),
            Err(CommandTracePolicyError::ZeroFileSizeLimit)
        ));
    }

    #[test]
    fn test_command_trace_file_size_limit_bytes_positive_value_round_trips()
    -> Result<(), CommandTracePolicyError> {
        assert_eq!(valid_file_size_limit()?.value(), 4_096);
        Ok(())
    }

    #[test]
    fn test_command_trace_retained_file_count_zero_returns_zero_retained_file_count() {
        assert!(matches!(
            CommandTraceRetainedFileCount::try_new(0),
            Err(CommandTracePolicyError::ZeroRetainedFileCount)
        ));
    }

    #[test]
    fn test_command_trace_retained_file_count_positive_value_round_trips()
    -> Result<(), CommandTracePolicyError> {
        assert_eq!(valid_retained_file_count()?.value(), 3);
        Ok(())
    }

    #[test]
    fn test_command_trace_rotation_policy_valid_values_constructs_policy()
    -> Result<(), CommandTracePolicyError> {
        let max_file_size = valid_file_size_limit()?;
        let retained_files = valid_retained_file_count()?;

        let policy = CommandTraceRotationPolicy::new(max_file_size, retained_files);

        assert_eq!(policy.max_file_size, max_file_size);
        assert_eq!(policy.retained_files, retained_files);
        Ok(())
    }
}
