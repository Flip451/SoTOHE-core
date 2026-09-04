//! Descriptor-relative filesystem helpers for gate-log persistence.

use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};

use super::fs_persistence::StagedLogFile;
use usecase::gate_output::{GateAdapterFailureReason, GateLogReservationError, GateLogWriteError};

pub(super) const LOG_DIRECTORY: &str = "tmp/gate";

#[cfg(target_os = "linux")]
pub(super) fn probe_rename_exchange(
    directory: &File,
    reserved_name: &str,
    reserved_file: &File,
    staged: StagedLogFile,
) -> io::Result<StagedLogFile> {
    match probe_rename_exchange_inner(directory, reserved_name, reserved_file, staged) {
        Ok(staged) => Ok(staged),
        Err((error, _staged)) => Err(error),
    }
}

#[cfg(target_os = "linux")]
fn probe_rename_exchange_inner(
    directory: &File,
    reserved_name: &str,
    reserved_file: &File,
    staged: StagedLogFile,
) -> Result<StagedLogFile, (io::Error, StagedLogFile)> {
    if let Err(error) = rustix::fs::renameat_with(
        directory,
        reserved_name,
        directory,
        &staged.name,
        rustix::fs::RenameFlags::EXCHANGE,
    ) {
        return Err((io::Error::from(error), staged));
    }
    if let Err(error) = verify_file_path(&staged.file, directory, Path::new(reserved_name)) {
        return Err((io::Error::other(error.to_string()), staged));
    }
    if let Err(error) = verify_file_path(reserved_file, directory, Path::new(&staged.name)) {
        return Err((io::Error::other(error.to_string()), staged));
    }
    if let Err(error) = rustix::fs::renameat_with(
        directory,
        reserved_name,
        directory,
        &staged.name,
        rustix::fs::RenameFlags::EXCHANGE,
    ) {
        return Err((io::Error::from(error), staged));
    }
    if let Err(error) = verify_file_path(reserved_file, directory, Path::new(reserved_name)) {
        return Err((io::Error::other(error.to_string()), staged));
    }
    if let Err(error) = verify_file_path(&staged.file, directory, Path::new(&staged.name)) {
        return Err((io::Error::other(error.to_string()), staged));
    }
    Ok(staged)
}

#[cfg(not(target_os = "linux"))]
pub(super) fn probe_rename_exchange(
    _directory: &File,
    _reserved_name: &str,
    _reserved_file: &File,
    staged: StagedLogFile,
) -> io::Result<StagedLogFile> {
    let _ = staged;
    let error = io::Error::new(
        io::ErrorKind::Unsupported,
        "descriptor-relative gate-log persistence is supported only on Linux",
    );
    Err(error)
}

#[cfg(unix)]
pub(super) fn open_staged_log_file(directory: &File, name: &str) -> io::Result<File> {
    rustix::fs::openat(
        directory,
        name,
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
pub(super) fn open_staged_log_file(_directory: &File, _name: &str) -> io::Result<File> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "descriptor-relative gate-log persistence is unsupported on this platform",
    ))
}

pub(super) fn publish_staged_log(
    directory: &File,
    staged_name: &str,
    log_path: &Path,
) -> Result<(), GateLogWriteError> {
    let log_name = log_path.file_name().ok_or_else(|| {
        GateLogWriteError::Write(GateAdapterFailureReason::new(
            "reserved gate-log path has no file name".to_owned(),
        ))
    })?;
    #[cfg(target_os = "linux")]
    {
        rustix::fs::renameat_with(
            directory,
            staged_name,
            directory,
            log_name,
            rustix::fs::RenameFlags::EXCHANGE,
        )
        .map_err(|error| {
            GateLogWriteError::Write(GateAdapterFailureReason::new(error.to_string()))
        })?;
        Ok(())
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (directory, staged_name, log_name);
        Err(GateLogWriteError::Write(GateAdapterFailureReason::new(
            "descriptor-relative gate-log persistence is unsupported on this platform".to_owned(),
        )))
    }
}

/// Opens the trusted log directory by walking each path component without
/// following symlinks. The returned descriptor remains pinned to the checked
/// directory while each log leaf is created with `openat`.
#[cfg(unix)]
pub(super) fn open_log_directory(trusted_root: &Path) -> io::Result<File> {
    let anchor = if trusted_root.is_absolute() { Path::new("/") } else { Path::new(".") };
    let root = open_directory_nofollow(anchor)?;
    let trusted_root = open_directory_components_nofollow(root, trusted_root.components())?;
    open_directory_components_nofollow(trusted_root, Path::new(LOG_DIRECTORY).components())
}

/// Opens the existing trusted log directory without creating any component.
#[cfg(unix)]
fn open_existing_log_directory(trusted_root: &Path) -> io::Result<File> {
    let trusted_root = open_existing_trusted_root(trusted_root)?;
    open_existing_directory_components_nofollow(trusted_root, Path::new(LOG_DIRECTORY).components())
}

#[cfg(unix)]
fn open_existing_trusted_root(trusted_root: &Path) -> io::Result<File> {
    let anchor = if trusted_root.is_absolute() { Path::new("/") } else { Path::new(".") };
    let root = open_directory_nofollow(anchor)?;
    open_existing_directory_components_nofollow(root, trusted_root.components())
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
fn open_existing_directory_components_nofollow(
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
        directory = open_directory_at_nofollow(&directory, name)?;
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
pub(super) fn open_log_directory(_trusted_root: &Path) -> io::Result<File> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "descriptor-relative gate-log persistence is unsupported on this platform",
    ))
}

#[cfg(not(unix))]
fn open_existing_log_directory(_trusted_root: &Path) -> io::Result<File> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "descriptor-relative gate-log persistence is unsupported on this platform",
    ))
}

#[cfg(not(unix))]
fn open_existing_trusted_root(_trusted_root: &Path) -> io::Result<File> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "descriptor-relative gate-log persistence is unsupported on this platform",
    ))
}

#[cfg(unix)]
pub(super) fn is_name_too_long(error: &io::Error) -> bool {
    error.raw_os_error() == Some(libc::ENAMETOOLONG)
}

#[cfg(not(unix))]
pub(super) fn is_name_too_long(_error: &io::Error) -> bool {
    false
}

#[cfg(unix)]
pub(super) fn is_nofollow_symlink_error(error: &io::Error) -> bool {
    error.raw_os_error() == Some(libc::ELOOP)
}

#[cfg(not(unix))]
pub(super) fn is_nofollow_symlink_error(_error: &io::Error) -> bool {
    false
}

pub(super) fn encoded_name_too_long(path: &Path) -> GateLogReservationError {
    GateLogReservationError::EncodedNameTooLong(GateAdapterFailureReason::new(format!(
        "encoded gate-log filename is {} bytes and the destination filesystem rejected it as too long",
        path.file_name().map_or(0, |name| name.len())
    )))
}

/// Creates a new log leaf without following a raced parent or leaf symlink.
///
/// Unix targets use descriptor-relative `openat` after walking the trusted root
/// and `tmp/gate` components. Other targets fail closed because this adapter has
/// no equivalent descriptor-relative, no-follow directory API on those targets.
#[cfg(unix)]
pub(super) fn open_new_log_file(path: &Path, parent: &File) -> io::Result<File> {
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
pub(super) fn open_new_log_file(_path: &Path, _parent: &File) -> io::Result<File> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "descriptor-relative gate-log persistence is unsupported on this platform",
    ))
}

#[cfg(unix)]
pub(super) fn verify_directory_identity(
    expected: &File,
    current: &File,
) -> Result<(), GateLogWriteError> {
    let expected = rustix::fs::fstat(expected).map_err(|error| {
        GateLogWriteError::Write(GateAdapterFailureReason::new(error.to_string()))
    })?;
    let current = rustix::fs::fstat(current).map_err(|error| {
        GateLogWriteError::Write(GateAdapterFailureReason::new(error.to_string()))
    })?;
    let expected_type = rustix::fs::FileType::from_raw_mode(expected.st_mode);
    let current_type = rustix::fs::FileType::from_raw_mode(current.st_mode);
    if !expected_type.is_dir()
        || !current_type.is_dir()
        || expected.st_dev != current.st_dev
        || expected.st_ino != current.st_ino
    {
        return Err(GateLogWriteError::Write(GateAdapterFailureReason::new(
            "gate-log parent directory no longer names the reserved directory".to_owned(),
        )));
    }
    Ok(())
}

#[cfg(not(unix))]
pub(super) fn verify_directory_identity(
    _expected: &File,
    _current: &File,
) -> Result<(), GateLogWriteError> {
    Err(GateLogWriteError::Write(GateAdapterFailureReason::new(
        "descriptor-relative gate-log persistence is unsupported on this platform".to_owned(),
    )))
}

#[cfg(unix)]
pub(super) fn verify_file_path(
    file: &File,
    directory: &File,
    path: &Path,
) -> Result<(), GateLogWriteError> {
    let file_name = path.file_name().ok_or_else(|| {
        GateLogWriteError::Write(GateAdapterFailureReason::new(
            "reserved gate-log path has no file name".to_owned(),
        ))
    })?;
    let reserved = rustix::fs::fstat(file).map_err(|error| {
        GateLogWriteError::Write(GateAdapterFailureReason::new(error.to_string()))
    })?;
    let current = rustix::fs::statat(directory, file_name, rustix::fs::AtFlags::SYMLINK_NOFOLLOW)
        .map_err(|error| {
        GateLogWriteError::Write(GateAdapterFailureReason::new(error.to_string()))
    })?;
    let reserved_type = rustix::fs::FileType::from_raw_mode(reserved.st_mode);
    let current_type = rustix::fs::FileType::from_raw_mode(current.st_mode);
    if !reserved_type.is_file()
        || !current_type.is_file()
        || reserved.st_dev != current.st_dev
        || reserved.st_ino != current.st_ino
    {
        return Err(GateLogWriteError::Write(GateAdapterFailureReason::new(
            "gate-log path no longer names the opened regular file".to_owned(),
        )));
    }
    Ok(())
}

#[cfg(not(unix))]
pub(super) fn verify_file_path(
    _file: &File,
    _directory: &File,
    _path: &Path,
) -> Result<(), GateLogWriteError> {
    Err(GateLogWriteError::Write(GateAdapterFailureReason::new(
        "descriptor-relative gate-log persistence is unsupported on this platform".to_owned(),
    )))
}

pub(super) fn open_log_directory_for_write(
    trusted_root: &Path,
    log_directory: &Path,
) -> Result<File, GateLogWriteError> {
    open_existing_log_directory(trusted_root).map_err(|error| {
        if is_nofollow_symlink_error(&error) {
            let component = find_symlink_component(log_directory)
                .unwrap_or_else(|| log_directory.to_path_buf());
            GateLogWriteError::SymlinkComponent(component)
        } else {
            GateLogWriteError::Write(GateAdapterFailureReason::new(error.to_string()))
        }
    })
}

pub(super) fn find_symlink_component(path: &Path) -> Option<PathBuf> {
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
