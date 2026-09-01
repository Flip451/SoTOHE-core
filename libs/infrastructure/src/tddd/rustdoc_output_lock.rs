//! Descriptor-pinned ownership of Cargo's rustdoc output directory.

use std::fs::File;
use std::io::{Error, ErrorKind, Read as _};
use std::path::{Component, Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};

use fs4::fs_std::FileExt as _;

use domain::schema::SchemaExportError;

pub(crate) const RUSTDOC_OUTPUT_LOCK_TIMEOUT: Duration = Duration::from_secs(120);
const RUSTDOC_OUTPUT_LOCK_FILE: &str = ".sotp-rustdoc-json.lock";
const RUSTDOC_OUTPUT_LOCK_POLL_INTERVAL: Duration = Duration::from_millis(10);

/// A lock proving exclusive participation in one resolved Cargo target directory.
///
/// The lock is acquired before the expected output path is selected and is held
/// until the output bytes have been copied into memory. Every repository rustdoc
/// writer enters through this boundary, so an exporter cannot replace another
/// export between path validation and snapshot capture.
#[derive(Debug)]
pub(crate) struct RustdocOutputLock {
    file: File,
    target_directory: File,
    target_dir: PathBuf,
    #[cfg(unix)]
    target_generation: DirectoryGeneration,
}

impl RustdocOutputLock {
    /// Acquires the target-directory lock, failing closed after 120 seconds.
    pub(crate) fn acquire(target_dir: &Path) -> Result<Self, SchemaExportError> {
        #[cfg(test)]
        let timeout = std::env::var("SOTOHE_TEST_RUSTDOC_OUTPUT_LOCK_TIMEOUT_MS")
            .ok()
            .and_then(|value| value.parse::<u64>().ok())
            .map(Duration::from_millis)
            .unwrap_or(RUSTDOC_OUTPUT_LOCK_TIMEOUT);
        #[cfg(not(test))]
        let timeout = RUSTDOC_OUTPUT_LOCK_TIMEOUT;
        Self::acquire_with_timeout(target_dir, timeout)
    }

    fn acquire_with_timeout(
        target_dir: &Path,
        timeout: Duration,
    ) -> Result<Self, SchemaExportError> {
        let target_dir =
            absolute_path(target_dir).map_err(|error| lock_error(error.to_string()))?;
        let target_directory = open_or_create_directory_nofollow(&target_dir).map_err(|error| {
            lock_error(format!(
                "cannot open rustdoc output lock in '{}': {error}",
                target_dir.display()
            ))
        })?;
        let file = open_lock_file(&target_directory).map_err(|error| {
            lock_error(format!(
                "cannot open rustdoc output lock in '{}': {error}",
                target_dir.display()
            ))
        })?;
        #[cfg(unix)]
        let target_generation = directory_generation(&target_directory).map_err(|error| {
            lock_error(format!(
                "cannot inspect rustdoc target directory '{}': {error}",
                target_dir.display()
            ))
        })?;
        let started = Instant::now();
        loop {
            match file.try_lock_exclusive() {
                Ok(true) => {
                    return Ok(Self {
                        file,
                        target_directory,
                        target_dir,
                        #[cfg(unix)]
                        target_generation,
                    });
                }
                Ok(false) => {
                    let Some(remaining) = timeout.checked_sub(started.elapsed()) else {
                        return Err(lock_error(format!(
                            "timed out acquiring rustdoc output lock after {:?}",
                            timeout
                        )));
                    };
                    thread::sleep(remaining.min(RUSTDOC_OUTPUT_LOCK_POLL_INTERVAL));
                }
                Err(error) if error.kind() == ErrorKind::WouldBlock => {
                    let Some(remaining) = timeout.checked_sub(started.elapsed()) else {
                        return Err(lock_error(format!(
                            "timed out acquiring rustdoc output lock after {:?}",
                            timeout
                        )));
                    };
                    thread::sleep(remaining.min(RUSTDOC_OUTPUT_LOCK_POLL_INTERVAL));
                }
                Err(error) => {
                    return Err(lock_error(format!(
                        "rustdoc output lock operation failed: {error}"
                    )));
                }
            }
        }
    }

    /// Reads one expected output through a descriptor-relative no-follow open.
    pub(crate) fn read_bytes(
        &self,
        expected_path: &Path,
        max_bytes: u64,
    ) -> Result<Vec<u8>, SchemaExportError> {
        let _held_lock = &self.file;
        verify_target_directory_generation(self).map_err(|error| {
            lock_error(format!(
                "rustdoc target directory generation changed for '{}': {error}",
                self.target_dir.display()
            ))
        })?;
        let file = open_relative_file(&self.target_directory, &self.target_dir, expected_path)
            .map_err(|error| {
                lock_error(format!(
                    "cannot read rustdoc output '{}': {error}",
                    expected_path.display()
                ))
            })?;
        let metadata = file.metadata().map_err(|error| lock_error(error.to_string()))?;
        if !metadata.is_file() {
            return Err(lock_error(format!(
                "rustdoc output '{}' is not a regular file",
                expected_path.display()
            )));
        }
        if metadata.len() > max_bytes {
            return Err(lock_error(format!(
                "rustdoc output '{}' exceeds {max_bytes} bytes",
                expected_path.display()
            )));
        }
        let initial_generation = file_generation(&metadata);
        let mut bytes = Vec::new();
        (&file)
            .take(max_bytes.saturating_add(1))
            .read_to_end(&mut bytes)
            .map_err(|error| lock_error(error.to_string()))?;
        if bytes.len() as u64 > max_bytes {
            return Err(lock_error(format!(
                "rustdoc output '{}' exceeds {max_bytes} bytes",
                expected_path.display()
            )));
        }
        let final_generation = file
            .metadata()
            .map(|metadata| file_generation(&metadata))
            .map_err(|error| lock_error(error.to_string()))?;
        if initial_generation != final_generation {
            return Err(lock_error(format!(
                "rustdoc output '{}' changed while it was being read",
                expected_path.display()
            )));
        }
        Ok(bytes)
    }

    #[cfg(test)]
    pub(crate) fn acquire_for_test(
        target_dir: &Path,
        timeout: Duration,
    ) -> Result<Self, SchemaExportError> {
        Self::acquire_with_timeout(target_dir, timeout)
    }
}

fn lock_error(message: impl Into<String>) -> SchemaExportError {
    SchemaExportError::RustdocFailed(message.into())
}

#[cfg(unix)]
fn open_lock_file(directory: &File) -> std::io::Result<File> {
    let file = rustix::fs::openat(
        directory,
        RUSTDOC_OUTPUT_LOCK_FILE,
        rustix::fs::OFlags::RDWR
            | rustix::fs::OFlags::CREATE
            | rustix::fs::OFlags::NOFOLLOW
            | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::from_raw_mode(0o600),
    )
    .map(File::from)
    .map_err(std::io::Error::from)?;
    if !file.metadata()?.is_file() {
        return Err(Error::new(ErrorKind::InvalidInput, "rustdoc lock is not a regular file"));
    }
    Ok(file)
}

#[cfg(not(unix))]
fn open_lock_file(_directory: &File) -> std::io::Result<File> {
    Err(Error::new(
        ErrorKind::Unsupported,
        "descriptor-relative no-follow rustdoc locks are supported only on Unix",
    ))
}

#[cfg(unix)]
fn open_relative_file(
    target_directory: &File,
    target_dir: &Path,
    expected_path: &Path,
) -> std::io::Result<File> {
    let expected_path = absolute_path(expected_path)?;
    let relative = expected_path.strip_prefix(target_dir).map_err(|_| {
        Error::new(ErrorKind::PermissionDenied, "rustdoc output is outside its target directory")
    })?;
    let mut directory = target_directory.try_clone()?;
    let mut components = relative.components().peekable();
    while let Some(component) = components.next() {
        let Component::Normal(name) = component else {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                "rustdoc output path contains a non-normal component",
            ));
        };
        if components.peek().is_some() {
            directory = rustix::fs::openat(
                &directory,
                name,
                rustix::fs::OFlags::RDONLY
                    | rustix::fs::OFlags::DIRECTORY
                    | rustix::fs::OFlags::NOFOLLOW
                    | rustix::fs::OFlags::CLOEXEC,
                rustix::fs::Mode::empty(),
            )
            .map(File::from)
            .map_err(std::io::Error::from)?;
        } else {
            return rustix::fs::openat(
                &directory,
                name,
                rustix::fs::OFlags::RDONLY
                    | rustix::fs::OFlags::NONBLOCK
                    | rustix::fs::OFlags::NOFOLLOW
                    | rustix::fs::OFlags::CLOEXEC,
                rustix::fs::Mode::empty(),
            )
            .map(File::from)
            .map_err(std::io::Error::from);
        }
    }
    Err(Error::new(ErrorKind::InvalidInput, "rustdoc output path has no file name"))
}

#[cfg(not(unix))]
fn open_relative_file(
    _target_directory: &File,
    _target_dir: &Path,
    _expected_path: &Path,
) -> std::io::Result<File> {
    Err(Error::new(
        ErrorKind::Unsupported,
        "descriptor-relative no-follow rustdoc reads are supported only on Unix",
    ))
}

#[cfg(unix)]
fn open_or_create_directory_nofollow(path: &Path) -> std::io::Result<File> {
    let mut directory = rustix::fs::open(
        Path::new("/"),
        rustix::fs::OFlags::RDONLY
            | rustix::fs::OFlags::DIRECTORY
            | rustix::fs::OFlags::NOFOLLOW
            | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::empty(),
    )
    .map(File::from)
    .map_err(std::io::Error::from)?;
    for component in path.components() {
        let Component::Normal(name) = component else {
            if matches!(component, Component::RootDir | Component::CurDir) {
                continue;
            }
            return Err(Error::new(ErrorKind::InvalidInput, "target directory contains '..'"));
        };
        let flags = rustix::fs::OFlags::RDONLY
            | rustix::fs::OFlags::DIRECTORY
            | rustix::fs::OFlags::NOFOLLOW
            | rustix::fs::OFlags::CLOEXEC;
        let opened = match rustix::fs::openat(&directory, name, flags, rustix::fs::Mode::empty()) {
            Ok(opened) => opened,
            Err(error) if error.kind() == ErrorKind::NotFound => {
                match rustix::fs::mkdirat(&directory, name, rustix::fs::Mode::from_raw_mode(0o700))
                {
                    Ok(()) | Err(rustix::io::Errno::EXIST) => {}
                    Err(error) => return Err(error.into()),
                }
                rustix::fs::openat(&directory, name, flags, rustix::fs::Mode::empty())?
            }
            Err(error) => return Err(error.into()),
        };
        directory = File::from(opened);
    }
    Ok(directory)
}

#[cfg(not(unix))]
fn open_or_create_directory_nofollow(_path: &Path) -> std::io::Result<File> {
    Err(Error::new(
        ErrorKind::Unsupported,
        "descriptor-relative no-follow rustdoc locks are supported only on Unix",
    ))
}

fn absolute_path(path: &Path) -> std::io::Result<PathBuf> {
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        Ok(std::env::current_dir()?.join(path))
    }
}

#[cfg(unix)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DirectoryGeneration {
    device: u64,
    inode: u64,
}

#[cfg(unix)]
fn directory_generation(file: &File) -> std::io::Result<DirectoryGeneration> {
    use std::os::unix::fs::MetadataExt as _;

    let metadata = file.metadata()?;
    if !metadata.is_dir() {
        return Err(Error::new(ErrorKind::InvalidInput, "rustdoc target is not a directory"));
    }
    Ok(DirectoryGeneration { device: metadata.dev(), inode: metadata.ino() })
}

#[cfg(unix)]
fn verify_target_directory_generation(lock: &RustdocOutputLock) -> std::io::Result<()> {
    use std::os::unix::fs::MetadataExt as _;

    crate::track::symlink_guard::reject_symlinks_up_to_root(&lock.target_dir)?;
    let metadata = std::fs::symlink_metadata(&lock.target_dir)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(Error::new(
            ErrorKind::InvalidInput,
            "rustdoc target path is not the originally opened directory",
        ));
    }
    let current = DirectoryGeneration { device: metadata.dev(), inode: metadata.ino() };
    if current != lock.target_generation {
        return Err(Error::new(
            ErrorKind::InvalidData,
            "rustdoc target directory was replaced after lock acquisition",
        ));
    }
    Ok(())
}

#[cfg(not(unix))]
fn verify_target_directory_generation(_lock: &RustdocOutputLock) -> std::io::Result<()> {
    Err(Error::new(
        ErrorKind::Unsupported,
        "descriptor-relative rustdoc output reads are supported only on Unix",
    ))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FileGeneration {
    length: u64,
    modified_nanos: u128,
    #[cfg(unix)]
    device: u64,
    #[cfg(unix)]
    inode: u64,
    #[cfg(unix)]
    change_seconds: i64,
    #[cfg(unix)]
    change_nanos: i64,
}

fn file_generation(metadata: &std::fs::Metadata) -> FileGeneration {
    let modified_nanos = metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0_u128, |duration| duration.as_nanos());
    FileGeneration {
        length: metadata.len(),
        modified_nanos,
        #[cfg(unix)]
        device: {
            use std::os::unix::fs::MetadataExt as _;
            metadata.dev()
        },
        #[cfg(unix)]
        inode: {
            use std::os::unix::fs::MetadataExt as _;
            metadata.ino()
        },
        #[cfg(unix)]
        change_seconds: {
            use std::os::unix::fs::MetadataExt as _;
            metadata.ctime()
        },
        #[cfg(unix)]
        change_nanos: {
            use std::os::unix::fs::MetadataExt as _;
            metadata.ctime_nsec()
        },
    }
}
