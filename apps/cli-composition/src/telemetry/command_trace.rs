//! Composition wiring for repository-local completed-command traces.

use std::path::PathBuf;
use std::sync::Arc;

use cli_driver::telemetry::command_trace::CommandTraceDriver;

use crate::CompositionError;

/// Repository-local composition root for completed-command tracing.
///
/// Construction validates only the private rotation defaults. It can therefore
/// return only [`CompositionError::WiringFailed`]; the adapter, interactor, and
/// driver constructors are infallible and construction performs no I/O.
pub struct CommandTraceCompositionRoot {
    command_trace_driver: CommandTraceDriver,
}

impl CommandTraceCompositionRoot {
    /// Creates the complete local command-tracing dependency graph.
    ///
    /// The trace output is deliberately repository-local and kept in the
    /// ignored `track/items/logs/` runtime-log area. The one MiB active-file
    /// bound limits routine CLI diagnostics while retaining five generations
    /// for recent local investigation; these are local defaults, not a
    /// cross-track retention policy.
    ///
    /// # Errors
    ///
    /// Returns [`CompositionError::WiringFailed`] only when the fixed rotation
    /// defaults cannot be converted into their validated infrastructure types.
    pub fn new(project_root: PathBuf) -> Result<Self, CompositionError> {
        use infrastructure::telemetry::command_trace::FsCommandTraceAdapter;
        use usecase::telemetry::command_trace::{
            CommandTraceInteractor, CommandTraceService, CommandTraceWriterPort,
        };

        let rotation_policy = default_rotation_policy()?;
        let output_path =
            project_root.join("track").join("items").join("logs").join("command-trace.jsonl");
        let writer: Arc<dyn CommandTraceWriterPort> =
            Arc::new(FsCommandTraceAdapter::new(output_path, rotation_policy));
        let service: Arc<dyn CommandTraceService> = Arc::new(CommandTraceInteractor::new(writer));

        Ok(Self { command_trace_driver: CommandTraceDriver::new(service) })
    }

    /// Returns the fully wired completed-command trace driver.
    #[must_use]
    pub fn command_trace_driver(&self) -> &CommandTraceDriver {
        &self.command_trace_driver
    }
}

const DEFAULT_MAX_FILE_SIZE_BYTES: u64 = 1_048_576;
const DEFAULT_RETAINED_FILE_COUNT: u16 = 5;

fn default_rotation_policy()
-> Result<infrastructure::telemetry::command_trace::CommandTraceRotationPolicy, CompositionError> {
    rotation_policy_from_values(DEFAULT_MAX_FILE_SIZE_BYTES, DEFAULT_RETAINED_FILE_COUNT)
}

fn rotation_policy_from_values(
    max_file_size_bytes: u64,
    retained_file_count: u16,
) -> Result<infrastructure::telemetry::command_trace::CommandTraceRotationPolicy, CompositionError>
{
    use infrastructure::telemetry::command_trace::{
        CommandTraceFileSizeLimitBytes, CommandTraceRetainedFileCount, CommandTraceRotationPolicy,
    };

    let max_file_size = CommandTraceFileSizeLimitBytes::try_new(max_file_size_bytes)
        .map_err(|error| CompositionError::WiringFailed(error.to_string()))?;
    let retained_files = CommandTraceRetainedFileCount::try_new(retained_file_count)
        .map_err(|error| CompositionError::WiringFailed(error.to_string()))?;

    Ok(CommandTraceRotationPolicy::new(max_file_size, retained_files))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    use usecase::telemetry::command_trace::{
        CommandDurationMillis, CommandExecutionResult, CommandTraceRecord, SotpCommandIdentity,
    };

    use super::*;

    fn successful_record() -> Result<CommandTraceRecord, Box<dyn std::error::Error>> {
        Ok(CommandTraceRecord {
            command: SotpCommandIdentity::try_new("track status".to_owned())?,
            duration: CommandDurationMillis::from(42_u64),
            result: CommandExecutionResult::Success,
        })
    }

    fn rotated_path(output_path: &Path, generation: u16) -> PathBuf {
        let mut path = output_path.as_os_str().to_os_string();
        path.push(format!(".{generation}"));
        PathBuf::from(path)
    }

    #[test]
    fn test_command_trace_composition_root_new_wires_driver_without_writing()
    -> Result<(), Box<dyn std::error::Error>> {
        let temporary_directory = tempfile::TempDir::new()?;
        let output_path = temporary_directory.path().join("track/items/logs/command-trace.jsonl");

        let root = CommandTraceCompositionRoot::new(temporary_directory.path().to_path_buf())?;

        assert!(
            !output_path.exists(),
            "construction must not create the trace output before a record is handled"
        );
        let outcome = root.command_trace_driver().handle(
            cli_driver::CommandOutcome::success(Some("completed".to_owned())),
            successful_record()?,
        );
        assert_eq!(outcome.exit_code, 0);

        let content = fs::read_to_string(output_path)?;
        let record: serde_json::Value = serde_json::from_str(content.trim())?;
        assert_eq!(record.get("command").and_then(serde_json::Value::as_str), Some("track status"));
        assert_eq!(record.get("duration_ms").and_then(serde_json::Value::as_u64), Some(42));
        assert_eq!(
            record
                .get("result")
                .and_then(|result| result.get("status"))
                .and_then(serde_json::Value::as_str),
            Some("success")
        );
        Ok(())
    }

    #[test]
    fn test_command_trace_composition_root_rotation_validation_maps_to_wiring_failed() {
        let result = rotation_policy_from_values(0, DEFAULT_RETAINED_FILE_COUNT);

        assert!(matches!(result, Err(CompositionError::WiringFailed(_))));
    }

    #[test]
    fn test_command_trace_composition_root_driver_rotates_and_bounds_retained_files()
    -> Result<(), Box<dyn std::error::Error>> {
        let temporary_directory = tempfile::TempDir::new()?;
        let output_path = temporary_directory.path().join("track/items/logs/command-trace.jsonl");
        let output_directory = output_path.parent().ok_or("trace output path has no parent")?;
        fs::create_dir_all(output_directory)?;
        fs::write(&output_path, vec![b'x'; usize::try_from(DEFAULT_MAX_FILE_SIZE_BYTES)?])?;
        for generation in 1..=DEFAULT_RETAINED_FILE_COUNT {
            fs::write(
                rotated_path(&output_path, generation),
                format!("old-generation-{generation}"),
            )?;
        }

        let root = CommandTraceCompositionRoot::new(temporary_directory.path().to_path_buf())?;
        let outcome = root
            .command_trace_driver()
            .handle(cli_driver::CommandOutcome::success(None), successful_record()?);

        assert_eq!(outcome.exit_code, 0);
        assert!(fs::metadata(&output_path)?.len() <= DEFAULT_MAX_FILE_SIZE_BYTES);
        let active = fs::read_to_string(&output_path)?;
        assert!(active.contains("track status"));
        assert_eq!(fs::metadata(rotated_path(&output_path, 1))?.len(), DEFAULT_MAX_FILE_SIZE_BYTES);
        assert_eq!(fs::read_to_string(rotated_path(&output_path, 2))?, "old-generation-1");
        assert_eq!(
            fs::read_to_string(rotated_path(&output_path, DEFAULT_RETAINED_FILE_COUNT))?,
            "old-generation-4"
        );
        assert!(!rotated_path(&output_path, DEFAULT_RETAINED_FILE_COUNT + 1).exists());
        Ok(())
    }
}
