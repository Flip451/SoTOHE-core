//! Infrastructure adapters for gate process execution and log persistence.

mod fs_paths;
mod fs_persistence;

use std::path::PathBuf;
use std::process::Command;

use usecase::gate_output::{
    GateAdapterFailureReason, GateExitCode, GateLogPath, GateLogPersistencePort,
    GateLogReservation, GateLogReservationError, GateLogWriteError, GateProcessError,
    GateProcessOutput, GateProcessPort, GateRunCommand,
};

const UNKNOWN_ABNORMAL_EXIT_CODE: i32 = -1;

#[cfg(test)]
const LOG_DIRECTORY: &str = fs_paths::LOG_DIRECTORY;

/// Process adapter that executes validated OS-native argv and captures both
/// child output streams.
#[derive(Debug, Default)]
pub struct ProcessGateRunner;

impl ProcessGateRunner {
    /// Creates a process adapter.
    #[must_use]
    pub fn new() -> ProcessGateRunner {
        ProcessGateRunner
    }
}

impl GateProcessPort for ProcessGateRunner {
    fn run(&self, command: &GateRunCommand) -> Result<GateProcessOutput, GateProcessError> {
        let Some(program) = command.command().first() else {
            return Err(GateProcessError::Spawn(GateAdapterFailureReason::new(
                "validated gate command is empty".to_owned(),
            )));
        };
        let output =
            Command::new(program).args(command.command().iter().skip(1)).output().map_err(
                |error| GateProcessError::Spawn(GateAdapterFailureReason::new(error.to_string())),
            )?;
        Ok(GateProcessOutput {
            exit_code: exit_code_from_status(&output.status),
            output: combine_output(&output.stdout, &output.stderr),
        })
    }
}

/// Maps an OS child status to the numeric status carried by the usecase port.
///
/// A normal exit code is preserved exactly. When the operating system reports
/// no numeric code because the child was terminated by signal `N` on Unix, this
/// adapter maps the status to the reserved negative value `-N`, preserving the
/// signal identity across the existing numeric usecase port. A platform that
/// reports neither a numeric code nor a signal uses `-1`. The downstream CLI
/// boundary converts these out-of-range values to its generic failure code,
/// while the summary retains the mapped status.
fn exit_code_from_status(status: &std::process::ExitStatus) -> GateExitCode {
    GateExitCode::new(status.code().unwrap_or_else(|| abnormal_exit_code(status)))
}

#[cfg(unix)]
fn abnormal_exit_code(status: &std::process::ExitStatus) -> i32 {
    use std::os::unix::process::ExitStatusExt;

    status.signal().map_or(UNKNOWN_ABNORMAL_EXIT_CODE, |signal| signal.saturating_neg())
}

#[cfg(not(unix))]
fn abnormal_exit_code(_status: &std::process::ExitStatus) -> i32 {
    UNKNOWN_ABNORMAL_EXIT_CODE
}

/// Filesystem adapter that writes complete gate output beneath a trusted root.
#[derive(Debug)]
pub struct FsGateLogPersistence {
    inner: fs_persistence::FsGateLogPersistence,
}

impl FsGateLogPersistence {
    /// Creates a persistence adapter rooted at `trusted_root`.
    #[must_use]
    pub fn new(trusted_root: PathBuf) -> FsGateLogPersistence {
        FsGateLogPersistence { inner: fs_persistence::FsGateLogPersistence::new(trusted_root) }
    }
}

impl GateLogPersistencePort for FsGateLogPersistence {
    fn reserve(
        &self,
        command: &GateRunCommand,
    ) -> Result<GateLogReservation, GateLogReservationError> {
        GateLogPersistencePort::reserve(&self.inner, command)
    }

    fn persist(
        &self,
        reservation: GateLogReservation,
        contents: &[u8],
    ) -> Result<GateLogPath, GateLogWriteError> {
        GateLogPersistencePort::persist(&self.inner, reservation, contents)
    }
}

fn combine_output(stdout: &[u8], stderr: &[u8]) -> Vec<u8> {
    let mut output = Vec::with_capacity(stdout.len().saturating_add(stderr.len()));
    output.extend_from_slice(stdout);
    if !stderr.is_empty() {
        if !output.is_empty() && output.last().copied() != Some(b'\n') {
            output.push(b'\n');
        }
        output.extend_from_slice(b"--- stderr ---\n");
        output.extend_from_slice(stderr);
    }
    output
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
mod tests {
    use std::collections::HashSet;
    use std::ffi::OsString;
    use std::sync::Arc;

    use super::*;
    use usecase::gate_output::{
        GateExitCode, GateLogReservation, GateLogReservationError, GateLogWriteError,
        GateLogWriteOutcome, GateProcessOutput, GateProcessPort, GateRunInteractor, GateRunResult,
        GateRunService,
    };

    fn command(name: &str, shell: &str) -> GateRunCommand {
        GateRunCommand::try_new(
            name.to_owned(),
            vec![OsString::from("/bin/sh"), OsString::from("-c"), OsString::from(shell)],
        )
        .expect("test command should be valid")
    }

    #[cfg(target_os = "linux")]
    struct ParentMovingRunner {
        trusted_root: PathBuf,
        moved_log_directory: PathBuf,
    }

    #[cfg(target_os = "linux")]
    impl GateProcessPort for ParentMovingRunner {
        fn run(&self, _command: &GateRunCommand) -> Result<GateProcessOutput, GateProcessError> {
            let log_directory = self.trusted_root.join(LOG_DIRECTORY);
            std::fs::rename(&log_directory, &self.moved_log_directory)
                .expect("reserved parent directory should be movable");
            std::fs::create_dir(&log_directory).expect("replacement parent should be created");
            Ok(GateProcessOutput {
                exit_code: GateExitCode::new(0),
                output: b"child output must not be written outside root".to_vec(),
            })
        }
    }

    #[test]
    fn test_process_gate_runner_captures_both_streams_and_exit_code() {
        let output = ProcessGateRunner::new()
            .run(&command(
                "process-contract",
                "printf 'stdout detail\\n'; printf 'stderr detail\\n' >&2; exit 23",
            ))
            .expect("child process should run");

        assert_eq!(output.exit_code, GateExitCode::new(23));
        assert_eq!(output.output, b"stdout detail\n--- stderr ---\nstderr detail\n");
    }

    #[test]
    fn test_process_gate_runner_captures_gate_summary_and_full_diagnostic_from_child() {
        let output = ProcessGateRunner::new()
            .run(&command(
                "aggregate-check",
                "printf '[FAIL] aggregate-item: short reason\\n'; printf 'full diagnostic detail\\n' >&2; exit 23",
            ))
            .expect("aggregate child process should run");

        assert_eq!(output.exit_code, GateExitCode::new(23));
        assert_eq!(
            output.output,
            b"[FAIL] aggregate-item: short reason\n--- stderr ---\nfull diagnostic detail\n"
        );
    }

    #[test]
    fn test_process_gate_runner_preserves_success_exit_code() {
        let output = ProcessGateRunner::new()
            .run(&command("successful-check", "printf 'success detail\\n'; exit 0"))
            .expect("successful child process should run");

        assert_eq!(output.exit_code, GateExitCode::new(0));
        assert_eq!(output.output, b"success detail\n");
    }

    #[cfg(unix)]
    #[test]
    fn test_process_gate_runner_preserves_distinct_signal_status_values() {
        let runner = ProcessGateRunner::new();
        let terminated_by_term = runner
            .run(&command("signal-terminated", "kill -TERM $$"))
            .expect("SIGTERM child should run");
        let terminated_by_kill = runner
            .run(&command("kill-terminated", "kill -KILL $$"))
            .expect("SIGKILL child should run");

        assert_eq!(terminated_by_term.exit_code, GateExitCode::new(-15));
        assert_eq!(terminated_by_kill.exit_code, GateExitCode::new(-9));
        assert_ne!(terminated_by_term.exit_code, terminated_by_kill.exit_code);
    }

    #[test]
    fn test_process_gate_runner_returns_spawn_error_for_missing_program() {
        let command = GateRunCommand::try_new(
            "missing-program".to_owned(),
            vec![OsString::from("/definitely/missing/sotp-gate")],
        )
        .expect("test command should be valid");

        let result = ProcessGateRunner::new().run(&command);

        assert!(matches!(result, Err(GateProcessError::Spawn(_))));
    }

    #[test]
    fn test_fs_gate_log_persistence_writes_the_complete_output_under_tmp_gate() {
        let root = tempfile::tempdir().expect("temporary root should be created");
        let persistence = FsGateLogPersistence::new(root.path().to_path_buf());
        let command = command("persist-contract", "true");
        let complete_output = b"stdout\n--- stderr ---\nstderr\nopaque byte: \xFF\n";

        let reservation =
            persistence.reserve(&command).expect("log destination should be reserved");
        let path =
            persistence.persist(reservation, complete_output).expect("log should be persisted");

        assert!(path.as_path().starts_with(root.path().join("tmp/gate")));
        assert_eq!(
            std::fs::read(path.as_path()).expect("persisted log should be readable"),
            complete_output
        );
    }

    #[test]
    fn test_fs_gate_log_persistence_reserves_unique_destinations_before_persisting() {
        let root = tempfile::tempdir().expect("temporary root should be created");
        let persistence = FsGateLogPersistence::new(root.path().to_path_buf());
        let command = command("unique-reservations", "true");

        let first_reservation =
            persistence.reserve(&command).expect("first log destination should be reserved");
        let second_reservation =
            persistence.reserve(&command).expect("second log destination should be reserved");
        let first_path = first_reservation.as_path().to_path_buf();
        let second_path = second_reservation.as_path().to_path_buf();

        assert_ne!(first_path, second_path);
        assert!(first_path.starts_with(root.path().join("tmp/gate")));
        assert!(second_path.starts_with(root.path().join("tmp/gate")));

        let first_log = persistence
            .persist(first_reservation, b"first reserved output")
            .expect("first reserved log should be persisted");
        let second_log = persistence
            .persist(second_reservation, b"second reserved output")
            .expect("second reserved log should be persisted");

        assert_eq!(first_log.as_path(), first_path);
        assert_eq!(second_log.as_path(), second_path);
        assert_eq!(
            std::fs::read(&first_path).expect("first persisted log should be readable"),
            b"first reserved output"
        );
        assert_eq!(
            std::fs::read(&second_path).expect("second persisted log should be readable"),
            b"second reserved output"
        );
    }

    #[test]
    fn test_fs_gate_log_persistence_allows_sixteen_live_reservations_without_pending_cap() {
        let root = tempfile::tempdir().expect("temporary root should be created");
        let persistence = FsGateLogPersistence::new(root.path().to_path_buf());
        let command = command("many-live-reservations", "true");
        let mut reservations = Vec::with_capacity(16);
        let mut reserved_paths = HashSet::with_capacity(16);

        for _ in 0..16 {
            let reservation = persistence
                .reserve(&command)
                .expect("the adapter must not impose a small pending-reservation cap");
            let path = reservation.as_path().to_path_buf();
            assert!(reserved_paths.insert(path.clone()), "every reservation needs a unique path");
            let metadata = std::fs::metadata(&path).expect("reserved file should exist");
            assert!(metadata.is_file(), "reserved destination should be a regular file");
            assert_eq!(metadata.len(), 0, "reservation should remain an empty file");
            reservations.push(reservation);
        }

        assert_eq!(reserved_paths.len(), 16);
        for (index, reservation) in reservations.into_iter().enumerate() {
            let contents = format!("reserved output {index}");
            let persisted_path = persistence
                .persist(reservation, contents.as_bytes())
                .expect("every live reservation should be persistable");
            assert!(reserved_paths.contains(&persisted_path.as_path().to_path_buf()));
            assert_eq!(
                std::fs::read(persisted_path.as_path())
                    .expect("persisted output should be readable"),
                contents.as_bytes()
            );
        }
    }

    #[test]
    fn test_fs_gate_log_persistence_rejects_reusing_consumed_reservation() {
        let root = tempfile::tempdir().expect("temporary root should be created");
        let persistence = FsGateLogPersistence::new(root.path().to_path_buf());
        let reservation = persistence
            .reserve(&command("single-use-reservation", "true"))
            .expect("log destination should be reserved");
        let reserved_path = reservation.as_path().to_path_buf();
        let duplicate_reservation = GateLogReservation::from_reserved_path(reserved_path.clone());

        persistence
            .persist(reservation, b"first persisted output")
            .expect("first persistence should consume the reservation");
        let second_result = persistence.persist(duplicate_reservation, b"second output");

        assert!(matches!(second_result, Err(GateLogWriteError::Write(_))));
        assert_eq!(
            std::fs::read(&reserved_path).expect("first persisted log should remain readable"),
            b"first persisted output"
        );
    }

    #[test]
    fn test_fs_gate_log_persistence_reports_write_failure_without_persisted_path() {
        let root = tempfile::tempdir().expect("temporary root should be created");
        let persistence = FsGateLogPersistence::new(root.path().to_path_buf());
        let reservation = persistence
            .reserve(&command("missing-reserved-file", "true"))
            .expect("log destination should be reserved");
        let reserved_path = reservation.as_path().to_path_buf();
        std::fs::remove_file(&reserved_path).expect("reserved file should be removed");

        let result = persistence.persist(reservation, b"output cannot be written");

        assert!(matches!(result, Err(GateLogWriteError::Write(_))));
        assert!(!reserved_path.exists());
    }

    #[cfg(unix)]
    #[test]
    fn test_fs_gate_log_persistence_rejects_replaced_reserved_file() {
        let root = tempfile::tempdir().expect("temporary root should be created");
        let outside = tempfile::tempdir().expect("outside root should be created");
        let outside_file = outside.path().join("outside.log");
        std::fs::write(&outside_file, b"outside content").expect("outside file should be written");
        let persistence = FsGateLogPersistence::new(root.path().to_path_buf());
        let reservation = persistence
            .reserve(&command("replaced-reservation", "true"))
            .expect("log destination should be reserved");
        let reserved_path = reservation.as_path().to_path_buf();
        std::fs::remove_file(&reserved_path).expect("reserved file should be removed");
        std::fs::hard_link(&outside_file, &reserved_path)
            .expect("replacement hard link should be created");

        let result = persistence.persist(reservation, b"must not overwrite replacement");

        assert!(matches!(result, Err(GateLogWriteError::Write(_))));
        assert_eq!(
            std::fs::read(&outside_file).expect("outside file should be readable"),
            b"outside content"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_fs_gate_log_persistence_does_not_recreate_deleted_log_directory() {
        let root = tempfile::tempdir().expect("temporary root should be created");
        let persistence = FsGateLogPersistence::new(root.path().to_path_buf());
        let reservation = persistence
            .reserve(&command("deleted-directory", "true"))
            .expect("log destination should be reserved");
        let log_directory = root.path().join(LOG_DIRECTORY);
        std::fs::remove_dir_all(&log_directory).expect("log directory should be removed");

        let result = persistence.persist(reservation, b"must not recreate directory");

        assert!(matches!(result, Err(GateLogWriteError::Write(_))));
        assert!(!log_directory.exists());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn test_fs_gate_log_persistence_rejects_moved_parent_without_outside_root_path() {
        let root = tempfile::tempdir().expect("temporary root should be created");
        let moved_root = tempfile::tempdir().expect("moved directory root should be created");
        let persistence = FsGateLogPersistence::new(root.path().to_path_buf());
        let reservation = persistence
            .reserve(&command("moved-parent", "true"))
            .expect("log destination should be reserved");
        let reserved_path = reservation.as_path().to_path_buf();
        let file_name = reserved_path.file_name().expect("reservation should have a file name");
        let log_directory = root.path().join(LOG_DIRECTORY);
        let moved_log_directory = moved_root.path().join("moved-gate");

        std::fs::rename(&log_directory, &moved_log_directory)
            .expect("reserved parent directory should be movable");
        std::fs::create_dir(&log_directory).expect("replacement parent directory should be made");

        let result = persistence.persist(reservation, b"must not publish outside trusted root");

        match result {
            Err(GateLogWriteError::Write(_)) => {}
            Err(error) => panic!("unexpected persistence error: {error:?}"),
            Ok(path) => panic!(
                "persist returned a path after the parent moved: {}",
                path.as_path().display()
            ),
        }
        assert_eq!(
            std::fs::read(moved_log_directory.join(file_name))
                .expect("moved reservation should remain readable"),
            b""
        );
        assert_eq!(
            std::fs::read_dir(&log_directory)
                .expect("replacement parent directory should remain readable")
                .count(),
            0
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn test_gate_run_interactor_reports_moved_parent_as_unavailable_without_log_path() {
        let root = tempfile::tempdir().expect("temporary root should be created");
        let moved_root = tempfile::tempdir().expect("moved directory root should be created");
        let trusted_root = root.path().to_path_buf();
        let moved_log_directory = moved_root.path().join("moved-gate");
        let persistence = Arc::new(FsGateLogPersistence::new(trusted_root.clone()));
        let runner = Arc::new(ParentMovingRunner {
            trusted_root,
            moved_log_directory: moved_log_directory.clone(),
        });
        let interactor = GateRunInteractor::new(runner, persistence);

        let result = GateRunService::execute(&interactor, command("moved-parent-outcome", "true"))
            .expect("child execution should return a closed result");

        match result {
            GateRunResult::ChildExited { exit_code, log, .. } => {
                assert_eq!(exit_code, GateExitCode::new(0));
                assert!(matches!(
                    log,
                    GateLogWriteOutcome::Unavailable(GateLogWriteError::Write(_))
                ));
            }
            GateRunResult::SpawnFailed { error, .. } => {
                panic!("unexpected spawn failure: {error:?}");
            }
        }

        let moved_files: Vec<_> = std::fs::read_dir(&moved_log_directory)
            .expect("moved parent should remain readable")
            .map(|entry| entry.expect("moved log entry should be readable").path())
            .collect();
        assert!(!moved_files.is_empty(), "the moved parent should retain reserved files");
        for path in moved_files {
            assert_eq!(
                std::fs::read(path).expect("moved reserved file should remain readable"),
                b"",
                "the child output must not be written into the moved parent"
            );
        }
        assert_eq!(
            std::fs::read_dir(root.path().join(LOG_DIRECTORY))
                .expect("replacement parent should remain readable")
                .count(),
            0
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_fs_gate_log_persistence_keeps_unpersisted_reservation_on_drop() {
        let root = tempfile::tempdir().expect("temporary root should be created");
        let reserved_path;
        {
            let persistence = FsGateLogPersistence::new(root.path().to_path_buf());
            let reservation = persistence
                .reserve(&command("dropped-reservation", "true"))
                .expect("log destination should be reserved");
            reserved_path = reservation.as_path().to_path_buf();
            assert!(reserved_path.exists());
            assert_eq!(
                std::fs::read(&reserved_path).expect("reserved file should be readable"),
                b""
            );
        }

        assert!(reserved_path.exists());
        assert_eq!(
            std::fs::read(&reserved_path).expect("unconsumed reservation should remain readable"),
            b""
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_fs_gate_log_persistence_does_not_remove_replaced_reservation_on_drop() {
        let root = tempfile::tempdir().expect("temporary root should be created");
        let persistence = FsGateLogPersistence::new(root.path().to_path_buf());
        let reservation = persistence
            .reserve(&command("replaced-drop", "true"))
            .expect("log destination should be reserved");
        let reserved_path = reservation.as_path().to_path_buf();
        std::fs::remove_file(&reserved_path).expect("reserved file should be removed");
        std::fs::write(&reserved_path, b"replacement").expect("replacement should be written");

        drop(persistence);

        assert_eq!(
            std::fs::read(&reserved_path).expect("replacement should remain readable"),
            b"replacement"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_fs_gate_log_persistence_rejects_symlinked_log_directory_component() {
        let root = tempfile::tempdir().expect("temporary root should be created");
        let outside = tempfile::tempdir().expect("outside root should be created");
        std::os::unix::fs::symlink(outside.path(), root.path().join("tmp"))
            .expect("symlink should be created");
        let persistence = FsGateLogPersistence::new(root.path().to_path_buf());

        let result = persistence.reserve(&command("symlink-contract", "true"));

        assert!(matches!(result, Err(GateLogReservationError::SymlinkComponent(_))));
    }

    #[test]
    fn test_fs_gate_log_persistence_encodes_opaque_name_without_escaping_root() {
        let root = tempfile::tempdir().expect("temporary root should be created");
        let persistence = FsGateLogPersistence::new(root.path().to_path_buf());
        let command = command("../opaque name", "true");

        let reservation =
            persistence.reserve(&command).expect("log destination should be reserved");
        let path = persistence.persist(reservation, b"log").expect("log should be persisted");

        assert!(path.as_path().starts_with(root.path().join("tmp/gate")));
        assert!(
            !path
                .as_path()
                .file_name()
                .expect("log should have a filename")
                .to_string_lossy()
                .contains('/')
        );
    }

    #[test]
    fn test_fs_gate_log_persistence_rejects_overlong_encoded_name_during_reservation() {
        let root = tempfile::tempdir().expect("temporary root should be created");
        let persistence = FsGateLogPersistence::new(root.path().to_path_buf());
        let command = command(&"%".repeat(100), "true");
        let log_directory = root.path().join(LOG_DIRECTORY);

        let result = persistence.reserve(&command);

        assert!(matches!(result, Err(GateLogReservationError::EncodedNameTooLong(_))));
        assert_eq!(
            std::fs::read_dir(log_directory)
                .expect("reservation should create the log directory")
                .count(),
            0
        );
    }

    #[test]
    fn test_fs_gate_log_persistence_reserve_failure_leaves_no_complete_log_under_trusted_root() {
        let root = tempfile::tempdir().expect("temporary root should be created");
        std::fs::write(root.path().join("tmp"), b"not a directory")
            .expect("blocking path should be written");
        let persistence = FsGateLogPersistence::new(root.path().to_path_buf());

        let result = persistence.reserve(&command("blocked-preparation", "true"));

        assert!(matches!(result, Err(GateLogReservationError::CreateDirectory(_))));
        assert!(!root.path().join(LOG_DIRECTORY).exists());
        assert_eq!(
            std::fs::read_dir(root.path()).expect("trusted root should remain readable").count(),
            1,
            "reserve failure must not leave a complete log under the trusted root"
        );
    }
}
