//! Capability-runtime directory and final-message file handling.

use std::ffi::{OsStr, OsString};
use std::fs::File;
use std::io::ErrorKind;
use std::path::{Component, Path, PathBuf};

use usecase::capability_exec::{CapabilityExecError, ProviderName};

use super::output_collector::{MAX_PROVIDER_FINAL_MESSAGE_BYTES, read_bounded_output_last_message};
use super::{dispatch_error, path_guard};

pub(super) struct RuntimeDirectory {
    pub(super) path: PathBuf,
    pub(super) directory: File,
}

pub(super) struct RuntimeOutputLastMessage {
    pub(super) name: OsString,
    pub(super) path: PathBuf,
}

pub(super) fn prepare_runtime_dir(
    repo_root: &Path,
    runtime_dir: &Path,
    provider: &ProviderName,
) -> Result<RuntimeDirectory, CapabilityExecError> {
    let normalized_root = path_guard::lexically_normalize(repo_root);
    // The original path is passed to the provider, so fail closed on every
    // raw parent traversal before normalization. Otherwise a component could
    // change into a symlink after validation and redirect `component/..`.
    if runtime_dir.components().any(|component| component == Component::ParentDir) {
        return Err(dispatch_error(
            provider,
            format!("runtime directory contains parent traversal: {}", runtime_dir.display()),
        ));
    }
    let normalized_runtime = path_guard::lexically_normalize(runtime_dir);
    if !normalized_runtime.starts_with(&normalized_root) {
        return Err(dispatch_error(
            provider,
            format!(
                "runtime directory {} escapes repository root {}",
                runtime_dir.display(),
                normalized_root.display()
            ),
        ));
    }
    let canonical_root = normalized_root.canonicalize().map_err(|error| {
        dispatch_error(
            provider,
            format!("cannot canonicalize repository root {}: {error}", repo_root.display()),
        )
    })?;
    let relative_runtime = normalized_runtime.strip_prefix(&normalized_root).map_err(|error| {
        dispatch_error(
            provider,
            format!(
                "cannot resolve runtime directory {} below repository root {}: {error}",
                normalized_runtime.display(),
                normalized_root.display()
            ),
        )
    })?;
    let mut current = open_directory_nofollow(&canonical_root).map_err(|error| {
        dispatch_error(
            provider,
            format!("cannot open repository root {}: {error}", canonical_root.display()),
        )
    })?;
    for component in relative_runtime.components() {
        let std::path::Component::Normal(name) = component else {
            return Err(dispatch_error(
                provider,
                format!(
                    "runtime directory {} contains an invalid normalized component",
                    normalized_runtime.display()
                ),
            ));
        };
        current = open_or_create_runtime_directory(&current, name).map_err(|error| {
            let component_path = normalized_runtime.join(name);
            let detail = if error.to_string().contains("refusing to follow symlink") {
                format!("refusing to follow symlink: {}", component_path.display())
            } else {
                format!("cannot open runtime directory {}: {error}", component_path.display())
            };
            dispatch_error(provider, detail)
        })?;
    }
    Ok(RuntimeDirectory { path: normalized_runtime, directory: current })
}

pub(super) fn prepare_output_last_message(
    path: &Path,
    runtime_dir: &RuntimeDirectory,
    provider: &ProviderName,
) -> Result<RuntimeOutputLastMessage, CapabilityExecError> {
    let parent = path.parent().ok_or_else(|| {
        dispatch_error(
            provider,
            format!("output-last-message path has no parent: {}", path.display()),
        )
    })?;
    if path_guard::lexically_normalize(parent) != runtime_dir.path {
        return Err(dispatch_error(
            provider,
            format!(
                "output-last-message path {} is outside runtime directory {}",
                path.display(),
                runtime_dir.path.display()
            ),
        ));
    }
    let name = path.file_name().ok_or_else(|| {
        dispatch_error(
            provider,
            format!("output-last-message path has no file name: {}", path.display()),
        )
    })?;
    match rustix::fs::statat(&runtime_dir.directory, name, rustix::fs::AtFlags::SYMLINK_NOFOLLOW) {
        Ok(metadata) if rustix::fs::FileType::from_raw_mode(metadata.st_mode).is_symlink() => {
            return Err(dispatch_error(
                provider,
                format!("refusing to follow symlink: {}", path.display()),
            ));
        }
        Ok(metadata) if !rustix::fs::FileType::from_raw_mode(metadata.st_mode).is_file() => {
            return Err(dispatch_error(
                provider,
                format!("output-last-message path is not a regular file: {}", path.display()),
            ));
        }
        Ok(_) => {
            rustix::fs::unlinkat(&runtime_dir.directory, name, rustix::fs::AtFlags::empty())
                .map_err(|error| {
                    dispatch_error(
                        provider,
                        format!(
                            "cannot initialize output-last-message {}: {error}",
                            path.display()
                        ),
                    )
                })?;
        }
        Err(error) if error.kind() == ErrorKind::NotFound => {}
        Err(error) => {
            return Err(dispatch_error(
                provider,
                format!("cannot inspect output-last-message {}: {error}", path.display()),
            ));
        }
    }
    let file = open_runtime_file(
        &runtime_dir.directory,
        name,
        rustix::fs::OFlags::WRONLY
            | rustix::fs::OFlags::CREATE
            | rustix::fs::OFlags::EXCL
            | rustix::fs::OFlags::NOFOLLOW
            | rustix::fs::OFlags::CLOEXEC,
        provider,
        path,
        "initialize output-last-message",
    )?;
    let metadata = file.metadata().map_err(|error| {
        dispatch_error(
            provider,
            format!("cannot inspect output-last-message {}: {error}", path.display()),
        )
    })?;
    if !metadata.is_file() {
        return Err(dispatch_error(
            provider,
            format!("output-last-message path is not a regular file: {}", path.display()),
        ));
    }
    Ok(RuntimeOutputLastMessage { name: name.to_owned(), path: path.to_path_buf() })
}

pub(super) fn read_output_last_message_at(
    runtime_dir: &RuntimeDirectory,
    output: &RuntimeOutputLastMessage,
    provider: &ProviderName,
) -> Result<Option<Vec<u8>>, CapabilityExecError> {
    let file = match rustix::fs::openat(
        &runtime_dir.directory,
        &output.name,
        rustix::fs::OFlags::RDONLY
            | rustix::fs::OFlags::NONBLOCK
            | rustix::fs::OFlags::NOFOLLOW
            | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::empty(),
    ) {
        Ok(file) => File::from(file),
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(dispatch_error(
                provider,
                format!("cannot open output-last-message {}: {error}", output.path.display()),
            ));
        }
    };
    let metadata = file.metadata().map_err(|error| {
        dispatch_error(
            provider,
            format!("cannot inspect output-last-message {}: {error}", output.path.display()),
        )
    })?;
    if !metadata.is_file() {
        return Err(dispatch_error(
            provider,
            format!("output-last-message path is not a regular file: {}", output.path.display()),
        ));
    }
    if metadata.len() > MAX_PROVIDER_FINAL_MESSAGE_BYTES {
        return Err(dispatch_error(
            provider,
            format!(
                "output-last-message {} exceeds {} bytes",
                output.path.display(),
                MAX_PROVIDER_FINAL_MESSAGE_BYTES
            ),
        ));
    }
    read_bounded_output_last_message(file, &output.path, provider)
}

pub(super) fn open_runtime_file(
    directory: &File,
    name: &OsStr,
    flags: rustix::fs::OFlags,
    provider: &ProviderName,
    path: &Path,
    action: &str,
) -> Result<File, CapabilityExecError> {
    rustix::fs::openat(directory, name, flags, rustix::fs::Mode::from_raw_mode(0o600))
        .map(File::from)
        .map_err(|error| {
            dispatch_error(provider, format!("cannot {action} {}: {error}", path.display()))
        })
}

fn open_or_create_runtime_directory(parent: &File, name: &OsStr) -> Result<File, std::io::Error> {
    match open_runtime_directory_at_nofollow(parent, name) {
        Ok(directory) => Ok(directory),
        Err(error) if error.kind() == ErrorKind::NotFound => {
            match rustix::fs::mkdirat(parent, name, rustix::fs::Mode::from_raw_mode(0o700)) {
                Ok(()) => {}
                Err(error) if error.kind() == ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error.into()),
            }
            open_runtime_directory_at_nofollow(parent, name)
                .map_err(|error| runtime_directory_open_error(parent, name, error))
        }
        Err(error) => Err(runtime_directory_open_error(parent, name, error)),
    }
}

fn runtime_directory_open_error(
    parent: &File,
    name: &OsStr,
    error: std::io::Error,
) -> std::io::Error {
    match rustix::fs::statat(parent, name, rustix::fs::AtFlags::SYMLINK_NOFOLLOW) {
        Ok(metadata) if rustix::fs::FileType::from_raw_mode(metadata.st_mode).is_symlink() => {
            std::io::Error::other("refusing to follow symlink")
        }
        _ => error,
    }
}

fn open_directory_nofollow(path: &Path) -> Result<File, std::io::Error> {
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

fn open_runtime_directory_at_nofollow(parent: &File, name: &OsStr) -> Result<File, std::io::Error> {
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
