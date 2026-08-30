//! Filesystem gate-log reservation and persistence transaction state.

use std::collections::HashMap;
use std::fs::File;
use std::io::{self, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use super::fs_paths;
use usecase::gate_output::{
    GateAdapterFailureReason, GateLogPath, GateLogPersistencePort, GateLogReservation,
    GateLogReservationError, GateLogWriteError, GateRunCommand,
};

const MAX_LOG_CREATE_ATTEMPTS: usize = 64;
const STAGED_LOG_PREFIX: &str = ".gate-log-stage";

static LOG_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Debug)]
pub(super) struct ReservedLogFile {
    pub(super) file: File,
    pub(super) directory: File,
}

#[derive(Debug)]
pub(super) struct StagedLogFile {
    pub(super) file: File,
    pub(super) directory: File,
    pub(super) name: String,
}

#[derive(Debug, Default)]
struct StagedLogState {
    reusable: Option<StagedLogFile>,
}

#[derive(Debug)]
struct ReservedFileRegistry {
    files: Mutex<HashMap<PathBuf, ReservedLogFile>>,
}

impl ReservedFileRegistry {
    fn new() -> ReservedFileRegistry {
        ReservedFileRegistry { files: Mutex::new(HashMap::new()) }
    }
}

#[derive(Debug)]
pub(super) struct FsGateLogPersistence {
    trusted_root: PathBuf,
    reserved_files: ReservedFileRegistry,
    staged_logs: Mutex<StagedLogState>,
}

impl FsGateLogPersistence {
    /// Creates a persistence adapter rooted at `trusted_root`.
    #[must_use]
    pub(super) fn new(trusted_root: PathBuf) -> FsGateLogPersistence {
        FsGateLogPersistence {
            trusted_root,
            reserved_files: ReservedFileRegistry::new(),
            staged_logs: Mutex::new(StagedLogState::default()),
        }
    }
}

impl GateLogPersistencePort for FsGateLogPersistence {
    fn reserve(
        &self,
        command: &GateRunCommand,
    ) -> Result<GateLogReservation, GateLogReservationError> {
        ensure_platform_support()?;
        self.check_trusted_root()?;
        let log_directory = self.trusted_root.join(fs_paths::LOG_DIRECTORY);
        self.check_reservation_path(&log_directory, true)?;

        let log_directory_handle =
            fs_paths::open_log_directory(&self.trusted_root).map_err(|error| {
                GateLogReservationError::CreateDirectory(GateAdapterFailureReason::new(
                    error.to_string(),
                ))
            })?;
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(GateLogReservationError::Clock)?
            .as_nanos();
        let encoded_name = encode_name(command.name());
        let mut reserved_files = self.reserved_files.files.lock().map_err(|_| {
            GateLogReservationError::CreateFile(GateAdapterFailureReason::new(
                "gate-log reservation state is unavailable".to_owned(),
            ))
        })?;
        let mut staged_logs = self.staged_logs.lock().map_err(|_| {
            GateLogReservationError::CreateFile(GateAdapterFailureReason::new(
                "gate-log staging state is unavailable".to_owned(),
            ))
        })?;
        for _ in 0..MAX_LOG_CREATE_ATTEMPTS {
            let sequence = LOG_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let filename =
                format!("{}-{}-{timestamp}-{sequence}.log", encoded_name, std::process::id(),);
            let log_path = log_directory.join(&filename);

            let file_result = fs_paths::open_new_log_file(&log_path, &log_directory_handle);
            let file = match file_result {
                Ok(file) => file,
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) if fs_paths::is_name_too_long(&error) => {
                    return Err(fs_paths::encoded_name_too_long(&log_path));
                }
                Err(error) => {
                    return Err(GateLogReservationError::CreateFile(
                        GateAdapterFailureReason::new(error.to_string()),
                    ));
                }
            };
            let staged = match stage_probe_file(&mut staged_logs, &log_directory_handle) {
                Ok(staged) => staged,
                Err(error) => {
                    return Err(GateLogReservationError::CreateFile(
                        GateAdapterFailureReason::new(error.to_string()),
                    ));
                }
            };
            let staged = fs_paths::probe_rename_exchange(
                &log_directory_handle,
                &filename,
                &file,
                staged,
            )
            .map_err(|error| {
                GateLogReservationError::CreateFile(GateAdapterFailureReason::new(format!(
                    "gate-log publish capability is unavailable for the destination filesystem: {error}"
                )))
            })?;
            if reserved_files.contains_key(&log_path) {
                return Err(GateLogReservationError::CreateFile(GateAdapterFailureReason::new(
                    "gate-log reservation path is already tracked".to_owned(),
                )));
            }
            reserved_files.insert(
                log_path.clone(),
                ReservedLogFile { file, directory: log_directory_handle },
            );
            staged_logs.reusable = Some(staged);
            return Ok(GateLogReservation::from_reserved_path(log_path));
        }

        Err(GateLogReservationError::CreateFile(GateAdapterFailureReason::new(
            "could not allocate a unique gate-log filename".to_owned(),
        )))
    }

    fn persist(
        &self,
        reservation: GateLogReservation,
        contents: &[u8],
    ) -> Result<GateLogPath, GateLogWriteError> {
        let log_path = reservation.as_path().to_path_buf();
        let reserved = self.take_reserved_file(&log_path)?;
        self.check_trusted_root_for_write()?;
        let log_directory = self.trusted_root.join(fs_paths::LOG_DIRECTORY);
        self.check_reserved_path(&log_path, &log_directory)?;

        let initial_log_directory_handle =
            fs_paths::open_log_directory_for_write(&self.trusted_root, &log_directory)?;
        fs_paths::verify_directory_identity(&reserved.directory, &initial_log_directory_handle)?;

        let mut staged_logs = self.staged_logs.lock().map_err(|_| {
            GateLogWriteError::Write(GateAdapterFailureReason::new(
                "gate-log staging state is unavailable".to_owned(),
            ))
        })?;
        let staged = stage_log_contents(&mut staged_logs, &initial_log_directory_handle, contents)?;
        let final_log_directory_handle =
            fs_paths::open_log_directory_for_write(&self.trusted_root, &log_directory)?;
        fs_paths::verify_directory_identity(&reserved.directory, &final_log_directory_handle)?;
        fs_paths::verify_file_path(&reserved.file, &final_log_directory_handle, &log_path)?;
        fs_paths::verify_file_path(
            &staged.file,
            &final_log_directory_handle,
            Path::new(&staged.name),
        )?;

        let mut exchanged = false;
        let finalize_result = (|| {
            fs_paths::publish_staged_log(&final_log_directory_handle, &staged.name, &log_path)?;
            exchanged = true;
            fs_paths::verify_file_path(&staged.file, &final_log_directory_handle, &log_path)?;
            fs_paths::verify_file_path(
                &reserved.file,
                &final_log_directory_handle,
                Path::new(&staged.name),
            )?;

            let published_log_directory_handle =
                fs_paths::open_log_directory_for_write(&self.trusted_root, &log_directory)?;
            fs_paths::verify_directory_identity(
                &reserved.directory,
                &published_log_directory_handle,
            )?;
            fs_paths::verify_file_path(&staged.file, &published_log_directory_handle, &log_path)?;
            published_log_directory_handle.sync_all().map_err(|error| {
                GateLogWriteError::Write(GateAdapterFailureReason::new(error.to_string()))
            })?;
            Ok::<(), GateLogWriteError>(())
        })();
        if let Err(error) = finalize_result {
            let rollback_succeeded = if !exchanged {
                true
            } else {
                rollback_exchanged_log(&final_log_directory_handle, &staged, &reserved, &log_path)
            };
            if rollback_succeeded {
                staged_logs.reusable = Some(staged);
            }
            return Err(error);
        }
        staged_logs.reusable = Some(StagedLogFile {
            file: reserved.file,
            directory: reserved.directory,
            name: staged.name,
        });
        Ok(GateLogPath::from_persisted_path(log_path))
    }
}

#[cfg(target_os = "linux")]
fn ensure_platform_support() -> Result<(), GateLogReservationError> {
    Ok(())
}

#[cfg(not(target_os = "linux"))]
fn ensure_platform_support() -> Result<(), GateLogReservationError> {
    Err(GateLogReservationError::CreateFile(GateAdapterFailureReason::new(
        "descriptor-relative gate-log persistence is supported only on Linux".to_owned(),
    )))
}

fn stage_probe_file(state: &mut StagedLogState, directory: &File) -> io::Result<StagedLogFile> {
    if let Some(mut staged) = state.reusable.take() {
        let reusable_is_current = fs_paths::verify_directory_identity(&staged.directory, directory)
            .and_then(|()| {
                fs_paths::verify_file_path(&staged.file, directory, Path::new(&staged.name))
            })
            .is_ok();
        if reusable_is_current {
            if let Err(error) = clear_staged_contents(&mut staged.file) {
                state.reusable = Some(staged);
                return Err(error);
            }
            return Ok(staged);
        }
        // The descriptor or leaf is stale. `take` has removed it from the
        // reusable state; do not unlink by name, and create a new staged file.
    }

    for _ in 0..MAX_LOG_CREATE_ATTEMPTS {
        let sequence = LOG_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let name = format!("{STAGED_LOG_PREFIX}-probe-{}-{sequence}", std::process::id());
        let staged_directory = directory.try_clone()?;
        let file = match fs_paths::open_staged_log_file(directory, &name) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        };
        return Ok(StagedLogFile { file, directory: staged_directory, name });
    }

    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "could not allocate a unique gate-log probe filename",
    ))
}

fn clear_staged_contents(file: &mut File) -> io::Result<()> {
    file.set_len(0)?;
    file.seek(SeekFrom::Start(0))?;
    file.sync_all()
}

impl FsGateLogPersistence {
    fn take_reserved_file(&self, path: &Path) -> Result<ReservedLogFile, GateLogWriteError> {
        let mut reserved_files = self.reserved_files.files.lock().map_err(|_| {
            GateLogWriteError::Write(GateAdapterFailureReason::new(
                "gate-log reservation state is unavailable".to_owned(),
            ))
        })?;
        reserved_files.remove(path).ok_or_else(|| {
            GateLogWriteError::Write(GateAdapterFailureReason::new(
                "gate-log reservation is not owned by this adapter".to_owned(),
            ))
        })
    }
}

fn stage_log_contents(
    state: &mut StagedLogState,
    directory: &File,
    contents: &[u8],
) -> Result<StagedLogFile, GateLogWriteError> {
    if let Some(mut staged) = state.reusable.take() {
        let reusable_is_current = fs_paths::verify_directory_identity(&staged.directory, directory)
            .and_then(|()| {
                fs_paths::verify_file_path(&staged.file, directory, Path::new(&staged.name))
            })
            .is_ok();
        if reusable_is_current {
            if let Err(error) = write_staged_contents(&mut staged.file, contents) {
                state.reusable = Some(staged);
                return Err(error);
            }
            return Ok(staged);
        }
        // The descriptor or leaf is stale. `take` has removed it from the
        // reusable state; do not unlink by name, and let the creation loop
        // below use the current verified directory instead.
    }

    for _ in 0..MAX_LOG_CREATE_ATTEMPTS {
        let sequence = LOG_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let staged_name = format!("{STAGED_LOG_PREFIX}-{}-{sequence}", std::process::id());
        let staged_directory = directory.try_clone().map_err(|error| {
            GateLogWriteError::Write(GateAdapterFailureReason::new(error.to_string()))
        })?;
        let staged_file = match fs_paths::open_staged_log_file(directory, &staged_name) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(GateLogWriteError::Write(GateAdapterFailureReason::new(
                    error.to_string(),
                )));
            }
        };
        let mut staged =
            StagedLogFile { file: staged_file, directory: staged_directory, name: staged_name };
        let is_regular_file = match staged.file.metadata() {
            Ok(metadata) => metadata.is_file(),
            Err(error) => {
                state.reusable = Some(staged);
                return Err(GateLogWriteError::Write(GateAdapterFailureReason::new(
                    error.to_string(),
                )));
            }
        };
        if !is_regular_file {
            state.reusable = Some(staged);
            return Err(GateLogWriteError::Write(GateAdapterFailureReason::new(
                "staged gate-log path is not a regular file".to_owned(),
            )));
        }
        if let Err(error) = write_staged_contents(&mut staged.file, contents) {
            state.reusable = Some(staged);
            return Err(error);
        }
        return Ok(staged);
    }

    Err(GateLogWriteError::Write(GateAdapterFailureReason::new(
        "could not allocate a unique staged gate-log filename".to_owned(),
    )))
}

/// Rolls back the exchange only while both names still identify this
/// transaction's inodes. A failed identity check is fail-closed: no second
/// exchange is attempted.
fn rollback_exchanged_log(
    directory: &File,
    staged: &StagedLogFile,
    reserved: &ReservedLogFile,
    log_path: &Path,
) -> bool {
    if fs_paths::verify_directory_identity(&reserved.directory, directory).is_err() {
        return false;
    }
    if fs_paths::verify_file_path(&staged.file, directory, log_path).is_err() {
        return false;
    }
    if fs_paths::verify_file_path(&reserved.file, directory, Path::new(&staged.name)).is_err() {
        return false;
    }
    fs_paths::publish_staged_log(directory, &staged.name, log_path).is_ok()
}

fn write_staged_contents(file: &mut File, contents: &[u8]) -> Result<(), GateLogWriteError> {
    file.set_len(0).map_err(|error| {
        GateLogWriteError::Write(GateAdapterFailureReason::new(error.to_string()))
    })?;
    file.seek(SeekFrom::Start(0)).map_err(|error| {
        GateLogWriteError::Write(GateAdapterFailureReason::new(error.to_string()))
    })?;
    file.write_all(contents)
        .and_then(|()| file.sync_all())
        .map_err(|error| GateLogWriteError::Write(GateAdapterFailureReason::new(error.to_string())))
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

impl FsGateLogPersistence {
    fn check_trusted_root(&self) -> Result<(), GateLogReservationError> {
        if self.trusted_root.as_os_str().is_empty() {
            return Err(GateLogReservationError::OutsideRoot(PathBuf::from(
                fs_paths::LOG_DIRECTORY,
            )));
        }
        match crate::track::symlink_guard::reject_symlinks_up_to_root(&self.trusted_root) {
            Ok(()) => Ok(()),
            Err(error) if crate::track::symlink_guard::is_symlink_rejection(&error) => {
                let component = fs_paths::find_symlink_component(&self.trusted_root)
                    .unwrap_or_else(|| self.trusted_root.clone());
                Err(GateLogReservationError::SymlinkComponent(component))
            }
            Err(error) => Err(GateLogReservationError::CreateDirectory(
                GateAdapterFailureReason::new(error.to_string()),
            )),
        }
    }

    fn check_trusted_root_for_write(&self) -> Result<(), GateLogWriteError> {
        if self.trusted_root.as_os_str().is_empty() {
            return Err(GateLogWriteError::OutsideRoot(PathBuf::from(fs_paths::LOG_DIRECTORY)));
        }
        match crate::track::symlink_guard::reject_symlinks_up_to_root(&self.trusted_root) {
            Ok(()) => Ok(()),
            Err(error) if crate::track::symlink_guard::is_symlink_rejection(&error) => {
                let component = fs_paths::find_symlink_component(&self.trusted_root)
                    .unwrap_or_else(|| self.trusted_root.clone());
                Err(GateLogWriteError::SymlinkComponent(component))
            }
            Err(error) => {
                Err(GateLogWriteError::Write(GateAdapterFailureReason::new(error.to_string())))
            }
        }
    }

    fn check_reservation_path(
        &self,
        path: &Path,
        is_directory: bool,
    ) -> Result<(), GateLogReservationError> {
        if !path.starts_with(&self.trusted_root) {
            return Err(GateLogReservationError::OutsideRoot(path.to_path_buf()));
        }
        match crate::track::symlink_guard::reject_symlinks_below(path, &self.trusted_root) {
            Ok(_) => Ok(()),
            Err(error) if crate::track::symlink_guard::is_symlink_rejection(&error) => {
                let component =
                    fs_paths::find_symlink_component(path).unwrap_or_else(|| path.to_path_buf());
                Err(GateLogReservationError::SymlinkComponent(component))
            }
            Err(error) if !is_directory && fs_paths::is_name_too_long(&error) => {
                Err(fs_paths::encoded_name_too_long(path))
            }
            Err(error) if is_directory => Err(GateLogReservationError::CreateDirectory(
                GateAdapterFailureReason::new(error.to_string()),
            )),
            Err(error) => Err(GateLogReservationError::CreateFile(GateAdapterFailureReason::new(
                error.to_string(),
            ))),
        }
    }

    fn check_write_path(&self, path: &Path) -> Result<(), GateLogWriteError> {
        if !path.starts_with(&self.trusted_root) {
            return Err(GateLogWriteError::OutsideRoot(path.to_path_buf()));
        }
        match crate::track::symlink_guard::reject_symlinks_below(path, &self.trusted_root) {
            Ok(_) => Ok(()),
            Err(error) if crate::track::symlink_guard::is_symlink_rejection(&error) => {
                let component =
                    fs_paths::find_symlink_component(path).unwrap_or_else(|| path.to_path_buf());
                Err(GateLogWriteError::SymlinkComponent(component))
            }
            Err(error) => {
                Err(GateLogWriteError::Write(GateAdapterFailureReason::new(error.to_string())))
            }
        }
    }

    fn check_reserved_path(
        &self,
        path: &Path,
        log_directory: &Path,
    ) -> Result<(), GateLogWriteError> {
        if path.parent() != Some(log_directory) {
            return Err(GateLogWriteError::OutsideRoot(path.to_path_buf()));
        }
        self.check_write_path(path)
    }
}
