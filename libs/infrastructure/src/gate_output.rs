//! Infrastructure adapters for gate process execution and log persistence.

use std::fs::File;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use usecase::gate_output::{
    GateAdapterFailureReason, GateExitCode, GateLogPath, GateLogPersistenceError,
    GateLogPersistencePort, GateProcessError, GateProcessOutput, GateProcessPort, GateRunCommand,
};

const LOG_DIRECTORY: &str = "tmp/gate";
const MAX_LOG_CREATE_ATTEMPTS: usize = 64;
const UNKNOWN_ABNORMAL_EXIT_CODE: i32 = -1;

static LOG_SEQUENCE: AtomicU64 = AtomicU64::new(0);

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
    trusted_root: PathBuf,
}

impl FsGateLogPersistence {
    /// Creates a persistence adapter rooted at `trusted_root`.
    #[must_use]
    pub fn new(trusted_root: PathBuf) -> FsGateLogPersistence {
        FsGateLogPersistence { trusted_root }
    }
}

impl GateLogPersistencePort for FsGateLogPersistence {
    fn persist(
        &self,
        command: &GateRunCommand,
        contents: &[u8],
    ) -> Result<GateLogPath, GateLogPersistenceError> {
        self.check_trusted_root()?;
        let log_directory = self.trusted_root.join(LOG_DIRECTORY);
        self.check_path(&log_directory, true)?;

        let log_directory_handle = open_log_directory(&self.trusted_root).map_err(|error| {
            GateLogPersistenceError::CreateDirectory(GateAdapterFailureReason::new(
                error.to_string(),
            ))
        })?;
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(GateLogPersistenceError::Clock)?
            .as_nanos();

        for _ in 0..MAX_LOG_CREATE_ATTEMPTS {
            let sequence = LOG_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let filename = format!(
                "{}-{}-{timestamp}-{sequence}.log",
                encode_name(command.name()),
                std::process::id(),
            );
            let log_path = log_directory.join(filename);
            self.check_path(&log_path, false)?;

            let file_result = open_new_log_file(&log_path, &log_directory_handle);
            let mut file = match file_result {
                Ok(file) => file,
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => {
                    return Err(GateLogPersistenceError::Write(GateAdapterFailureReason::new(
                        error.to_string(),
                    )));
                }
            };
            let metadata = file.metadata().map_err(|error| {
                GateLogPersistenceError::Write(GateAdapterFailureReason::new(error.to_string()))
            })?;
            if !metadata.is_file() {
                return Err(GateLogPersistenceError::Write(GateAdapterFailureReason::new(
                    "new gate-log path is not a regular file".to_owned(),
                )));
            }
            file.write_all(contents).and_then(|()| file.sync_all()).map_err(|error| {
                GateLogPersistenceError::Write(GateAdapterFailureReason::new(error.to_string()))
            })?;
            return Ok(GateLogPath::from_persisted_path(log_path));
        }

        Err(GateLogPersistenceError::Write(GateAdapterFailureReason::new(
            "could not allocate a unique gate-log filename".to_owned(),
        )))
    }
}

/// Opens the trusted log directory by walking each path component without
/// following symlinks. The returned descriptor remains pinned to the checked
/// directory while each log leaf is created with `openat`.
#[cfg(unix)]
fn open_log_directory(trusted_root: &Path) -> io::Result<File> {
    let anchor = if trusted_root.is_absolute() { Path::new("/") } else { Path::new(".") };
    let root = open_directory_nofollow(anchor)?;
    let trusted_root = open_directory_components_nofollow(root, trusted_root.components())?;
    open_directory_components_nofollow(trusted_root, Path::new(LOG_DIRECTORY).components())
}

#[cfg(unix)]
fn open_directory_components_nofollow(
    mut directory: File,
    components: std::path::Components<'_>,
) -> io::Result<File> {
    for component in components {
        let name = match component {
            std::path::Component::RootDir | std::path::Component::CurDir => continue,
            std::path::Component::Normal(name) => name,
            std::path::Component::ParentDir => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "gate-log root cannot contain a parent component",
                ));
            }
            std::path::Component::Prefix(_) => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "gate-log root cannot contain a path prefix",
                ));
            }
        };
        directory = open_or_create_directory_at(&directory, name)?;
    }
    Ok(directory)
}

#[cfg(unix)]
fn open_directory_nofollow(path: &Path) -> io::Result<File> {
    rustix::fs::open(
        path,
        rustix::fs::OFlags::RDONLY
            | rustix::fs::OFlags::DIRECTORY
            | rustix::fs::OFlags::NOFOLLOW
            | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::empty(),
    )
    .map(File::from)
    .map_err(Into::into)
}

#[cfg(unix)]
fn open_directory_at_nofollow(parent: &File, name: &std::ffi::OsStr) -> io::Result<File> {
    rustix::fs::openat(
        parent,
        name,
        rustix::fs::OFlags::RDONLY
            | rustix::fs::OFlags::DIRECTORY
            | rustix::fs::OFlags::NOFOLLOW
            | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::empty(),
    )
    .map(File::from)
    .map_err(Into::into)
}

#[cfg(unix)]
fn open_or_create_directory_at(parent: &File, name: &std::ffi::OsStr) -> io::Result<File> {
    match open_directory_at_nofollow(parent, name) {
        Ok(directory) => Ok(directory),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            match rustix::fs::mkdirat(parent, name, rustix::fs::Mode::from_raw_mode(0o777)) {
                Ok(()) | Err(rustix::io::Errno::EXIST) => {}
                Err(error) => return Err(error.into()),
            }
            open_directory_at_nofollow(parent, name)
        }
        Err(error) => Err(error),
    }
}

#[cfg(not(unix))]
fn open_log_directory(_trusted_root: &Path) -> io::Result<File> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "descriptor-relative gate-log persistence is unsupported on this platform",
    ))
}

/// Creates a new log leaf without following a raced parent or leaf symlink.
///
/// Unix targets use descriptor-relative `openat` after walking the trusted root
/// and `tmp/gate` components. Other targets fail closed because this adapter has
/// no equivalent descriptor-relative, no-follow directory API on those targets.
#[cfg(unix)]
fn open_new_log_file(path: &Path, parent: &File) -> io::Result<File> {
    let file_name = path.file_name().ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "gate-log path has no file name")
    })?;
    rustix::fs::openat(
        parent,
        file_name,
        rustix::fs::OFlags::WRONLY
            | rustix::fs::OFlags::CREATE
            | rustix::fs::OFlags::EXCL
            | rustix::fs::OFlags::NOFOLLOW
            | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::from_raw_mode(0o600),
    )
    .map(File::from)
    .map_err(Into::into)
}

#[cfg(not(unix))]
fn open_new_log_file(_path: &Path, _parent: &File) -> io::Result<File> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "descriptor-relative gate-log persistence is unsupported on this platform",
    ))
}

impl FsGateLogPersistence {
    fn check_trusted_root(&self) -> Result<(), GateLogPersistenceError> {
        if self.trusted_root.as_os_str().is_empty() {
            return Err(GateLogPersistenceError::OutsideRoot(PathBuf::from(LOG_DIRECTORY)));
        }
        match crate::track::symlink_guard::reject_symlinks_up_to_root(&self.trusted_root) {
            Ok(()) => Ok(()),
            Err(error) if crate::track::symlink_guard::is_symlink_rejection(&error) => {
                let component = find_symlink_component(&self.trusted_root)
                    .unwrap_or_else(|| self.trusted_root.clone());
                Err(GateLogPersistenceError::SymlinkComponent(component))
            }
            Err(error) => Err(GateLogPersistenceError::CreateDirectory(
                GateAdapterFailureReason::new(error.to_string()),
            )),
        }
    }

    fn check_path(&self, path: &Path, is_directory: bool) -> Result<(), GateLogPersistenceError> {
        if !path.starts_with(&self.trusted_root) {
            return Err(GateLogPersistenceError::OutsideRoot(path.to_path_buf()));
        }
        match crate::track::symlink_guard::reject_symlinks_below(path, &self.trusted_root) {
            Ok(_) => Ok(()),
            Err(error) if crate::track::symlink_guard::is_symlink_rejection(&error) => {
                let component = find_symlink_component(path).unwrap_or_else(|| path.to_path_buf());
                Err(GateLogPersistenceError::SymlinkComponent(component))
            }
            Err(error) if is_directory => Err(GateLogPersistenceError::CreateDirectory(
                GateAdapterFailureReason::new(error.to_string()),
            )),
            Err(error) => Err(GateLogPersistenceError::Write(GateAdapterFailureReason::new(
                error.to_string(),
            ))),
        }
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

fn encode_name(name: &str) -> String {
    let mut encoded = String::new();
    for byte in name.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-') {
            encoded.push(char::from(byte));
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }
    if encoded.is_empty() { "gate".to_owned() } else { encoded }
}

fn find_symlink_component(path: &Path) -> Option<PathBuf> {
    for ancestor in path.ancestors() {
        if ancestor.as_os_str().is_empty() {
            break;
        }
        if let Ok(metadata) = ancestor.symlink_metadata() {
            if metadata.file_type().is_symlink() {
                return Some(ancestor.to_path_buf());
            }
        }
    }
    None
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
mod tests {
    use std::ffi::OsString;

    use super::*;
    use usecase::gate_output::{GateExitCode, GateProcessPort};

    fn command(name: &str, shell: &str) -> GateRunCommand {
        GateRunCommand::try_new(
            name.to_owned(),
            vec![OsString::from("/bin/sh"), OsString::from("-c"), OsString::from(shell)],
        )
        .expect("test command should be valid")
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

        let path = persistence.persist(&command, complete_output).expect("log should be persisted");

        assert!(path.as_path().starts_with(root.path().join(LOG_DIRECTORY)));
        assert_eq!(
            std::fs::read(path.as_path()).expect("persisted log should be readable"),
            complete_output
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

        let result = persistence.persist(&command("symlink-contract", "true"), b"log");

        assert!(matches!(result, Err(GateLogPersistenceError::SymlinkComponent(_))));
    }

    #[test]
    fn test_fs_gate_log_persistence_encodes_opaque_name_without_escaping_root() {
        let root = tempfile::tempdir().expect("temporary root should be created");
        let persistence = FsGateLogPersistence::new(root.path().to_path_buf());
        let command = command("../opaque name", "true");

        let path = persistence.persist(&command, b"log").expect("log should be persisted");

        assert!(path.as_path().starts_with(root.path().join(LOG_DIRECTORY)));
        assert!(
            !path
                .as_path()
                .file_name()
                .expect("log should have a filename")
                .to_string_lossy()
                .contains('/')
        );
    }
}
