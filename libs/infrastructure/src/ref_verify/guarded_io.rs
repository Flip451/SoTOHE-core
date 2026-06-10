//! Descriptor-pinned guarded filesystem I/O shared by the ref-verify adapters.
//!
//! Every read/write the ref-verify pipeline performs against the repository
//! goes through these helpers: paths are validated lexically (no `..`, no
//! absolute escape), opened component-by-component with `O_NOFOLLOW`, and the
//! final handle is re-verified against the trusted root via `/proc/self/fd`.
//! Cache writes additionally take an exclusive lock and use an
//! atomic-rename temp file.

use std::io::{Read as _, Write as _};
use std::path::{Component, Path, PathBuf};

use fs4::fs_std::FileExt;
use usecase::ref_verify::RefVerifyError;

// ---------------------------------------------------------------------------
// Lexical path validation
// ---------------------------------------------------------------------------

/// Resolve and validate a repo-relative file path against the project root.
///
/// Rejects absolute paths, `..` path-traversal components, and symlinks anywhere
/// on the resolved path.  Returns the absolute resolved path on success.
pub(super) fn resolve_and_guard_path(
    project_root: &Path,
    file: &Path,
    context: &str,
) -> Result<PathBuf, RefVerifyError> {
    let project_root = project_root.canonicalize().map_err(|e| RefVerifyError::VerifierPort {
        message: format!(
            "{context}: cannot canonicalize project root '{}': {e}",
            project_root.display()
        ),
    })?;
    if file.is_absolute() {
        return Err(RefVerifyError::VerifierPort {
            message: format!("{context}: invalid path (absolute): {}", file.display()),
        });
    }
    if file.components().any(|c| c == Component::ParentDir) {
        return Err(RefVerifyError::VerifierPort {
            message: format!("{context}: invalid path (path-traversal): {}", file.display()),
        });
    }
    if !file.components().any(|c| matches!(c, Component::Normal(_))) {
        return Err(RefVerifyError::VerifierPort {
            message: format!("{context}: invalid path (empty or '.'): {}", file.display()),
        });
    }

    let resolved = lexically_normalize(&project_root.join(file));
    if !resolved.starts_with(&project_root) {
        return Err(RefVerifyError::VerifierPort {
            message: format!("{context}: path escapes project root: {}", file.display()),
        });
    }
    Ok(resolved)
}

pub(super) fn lexically_normalize(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            other => normalized.push(other.as_os_str()),
        }
    }
    normalized
}

pub(super) fn relative_path_below_root(
    path: &Path,
    trusted_root: &Path,
    canon_root: &Path,
) -> Result<PathBuf, String> {
    let normalized = lexically_normalize(path);
    let root_norm = lexically_normalize(trusted_root);
    let rel = if normalized.is_absolute() {
        normalized.strip_prefix(canon_root).map_err(|_| {
            format!("path '{}' is outside trusted root '{}'", path.display(), canon_root.display())
        })?
    } else if !root_norm.as_os_str().is_empty() {
        normalized.strip_prefix(&root_norm).unwrap_or(&normalized)
    } else {
        normalized.as_path()
    };
    if !rel.components().any(|c| matches!(c, Component::Normal(_)))
        || rel.components().any(|c| !matches!(c, Component::Normal(_)))
    {
        return Err(format!("invalid path below trusted root: '{}'", path.display()));
    }
    Ok(rel.to_path_buf())
}

// ---------------------------------------------------------------------------
// Descriptor-pinned open helpers
// ---------------------------------------------------------------------------

#[cfg(unix)]
pub(super) fn proc_fd_child_path(parent_dir: &std::fs::File, child: &std::ffi::OsStr) -> PathBuf {
    use std::os::unix::io::AsRawFd;
    PathBuf::from(format!("/proc/self/fd/{}", parent_dir.as_raw_fd())).join(Path::new(child))
}

#[cfg(unix)]
pub(super) fn open_child_in_dir(
    parent_dir: &std::fs::File,
    child: &std::ffi::OsStr,
    flags: i32,
) -> std::io::Result<std::fs::File> {
    use std::os::unix::fs::OpenOptionsExt;
    std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(flags)
        .open(proc_fd_child_path(parent_dir, child))
}

pub(super) fn read_guarded_text(path: &Path, trusted_root: &Path) -> Result<String, String> {
    let mut file = open_guarded_file_for_read(path, trusted_root)?;
    let mut text = String::new();
    file.read_to_string(&mut text).map_err(|e| format!("cannot read '{}': {e}", path.display()))?;
    Ok(text)
}

#[cfg(unix)]
pub(super) fn open_guarded_file_for_read(
    path: &Path,
    trusted_root: &Path,
) -> Result<std::fs::File, String> {
    open_guarded_path(path, trusted_root, GuardedPathKind::File)
}

#[cfg(not(unix))]
pub(super) fn open_guarded_file_for_read(
    path: &Path,
    _trusted_root: &Path,
) -> Result<std::fs::File, String> {
    Err(format!(
        "ref-verify guarded reads require descriptor-pinned opening; unsupported on this platform for '{}'",
        path.display()
    ))
}

pub(super) fn verify_opened_handle_within_root(
    file: &std::fs::File,
    trusted_root: &Path,
    path: &Path,
) -> Result<(), String> {
    let canon_root = canonicalize_trusted_root(trusted_root)?;
    verify_opened_handle_within_canon_root(file, &canon_root, path)
}

pub(super) fn canonicalize_trusted_root(trusted_root: &Path) -> Result<PathBuf, String> {
    trusted_root
        .canonicalize()
        .map_err(|e| format!("cannot canonicalize trusted root '{}': {e}", trusted_root.display()))
}

pub(super) fn verify_opened_handle_within_canon_root(
    file: &std::fs::File,
    canon_root: &Path,
    path: &Path,
) -> Result<(), String> {
    let opened_path = opened_handle_path(file, path)?;
    if !opened_path.starts_with(canon_root) {
        return Err(format!(
            "opened path escapes trusted root: '{}' resolved to '{}'",
            path.display(),
            opened_path.display()
        ));
    }
    Ok(())
}

#[cfg(unix)]
pub(super) fn opened_handle_path(file: &std::fs::File, path: &Path) -> Result<PathBuf, String> {
    use std::os::unix::io::AsRawFd;
    let proc_path = format!("/proc/self/fd/{}", file.as_raw_fd());
    std::fs::read_link(&proc_path).map_err(|e| {
        format!("cannot verify opened file '{}' via '{}': {e}", path.display(), proc_path)
    })
}

#[cfg(not(unix))]
pub(super) fn opened_handle_path(file: &std::fs::File, path: &Path) -> Result<PathBuf, String> {
    let canon_path = path
        .canonicalize()
        .map_err(|e| format!("cannot canonicalize opened path '{}': {e}", path.display()))?;
    let path_meta = std::fs::metadata(path)
        .map_err(|e| format!("cannot stat opened path '{}': {e}", path.display()))?;
    let file_meta = file
        .metadata()
        .map_err(|e| format!("cannot stat opened file '{}': {e}", path.display()))?;
    if !path_meta.is_file() || !file_meta.is_file() {
        return Err(format!("opened path is not a regular file: '{}'", path.display()));
    }
    Ok(canon_path)
}

#[cfg(unix)]
#[derive(Clone, Copy)]
pub(super) enum GuardedPathKind {
    File,
    Directory,
}

#[cfg(unix)]
impl GuardedPathKind {
    fn terminal_flags(self) -> i32 {
        match self {
            Self::File => libc::O_NOFOLLOW | libc::O_CLOEXEC,
            Self::Directory => libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
        }
    }

    fn validate_terminal(self, opened: &std::fs::File, path: &Path) -> Result<(), String> {
        let meta =
            opened.metadata().map_err(|e| format!("cannot stat '{}': {e}", path.display()))?;
        match self {
            Self::File if !meta.is_file() => {
                Err(format!("not a regular file: '{}'", path.display()))
            }
            Self::Directory if !meta.is_dir() => {
                Err(format!("not a directory: '{}'", path.display()))
            }
            Self::File | Self::Directory => Ok(()),
        }
    }
}

#[cfg(unix)]
pub(super) fn open_guarded_path(
    path: &Path,
    trusted_root: &Path,
    kind: GuardedPathKind,
) -> Result<std::fs::File, String> {
    let canon_root = canonicalize_trusted_root(trusted_root)?;
    let normalized = lexically_normalize(path);
    let root_norm = lexically_normalize(trusted_root);
    let mut dir = open_root_directory_nofollow(&canon_root)
        .map_err(|e| format!("cannot open trusted root '{}': {e}", canon_root.display()))?;
    if matches!(kind, GuardedPathKind::Directory)
        && (normalized == canon_root || normalized == root_norm)
    {
        return Ok(dir);
    }

    let rel = relative_path_below_root(path, trusted_root, &canon_root)?;
    let mut components = rel.components().peekable();
    while let Some(component) = components.next() {
        let std::path::Component::Normal(name) = component else {
            return Err(format!("invalid path component in '{}'", path.display()));
        };
        let is_terminal = components.peek().is_none();
        let flags = if is_terminal {
            kind.terminal_flags()
        } else {
            libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC
        };
        let opened = open_child_in_dir(&dir, name, flags).map_err(|e| {
            if is_terminal {
                format!("cannot open '{}': {e}", path.display())
            } else {
                format!("cannot open directory component '{:?}': {e}", name)
            }
        })?;
        if is_terminal {
            kind.validate_terminal(&opened, path)?;
            verify_opened_handle_within_canon_root(&opened, &canon_root, path)?;
            return Ok(opened);
        }
        dir = opened;
    }
    Err(format!("invalid path (empty): '{}'", path.display()))
}

pub(super) fn verify_opened_file_within_root(
    file: &std::fs::File,
    trusted_root: &Path,
    path: &Path,
) -> Result<(), String> {
    verify_opened_handle_within_root(file, trusted_root, path)
}

// ---------------------------------------------------------------------------
// Guarded directory enumeration
// ---------------------------------------------------------------------------

#[cfg(unix)]
pub(super) fn open_guarded_directory(
    path: &Path,
    trusted_root: &Path,
) -> std::io::Result<std::fs::File> {
    open_guarded_path(path, trusted_root, GuardedPathKind::Directory).map_err(std::io::Error::other)
}

#[cfg(unix)]
pub(super) fn open_root_directory_nofollow(canon_root: &Path) -> std::io::Result<std::fs::File> {
    use std::os::unix::fs::OpenOptionsExt;
    let dir = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(canon_root)?;
    verify_opened_file_within_root(&dir, canon_root, canon_root).map_err(std::io::Error::other)?;
    Ok(dir)
}

#[cfg(unix)]
pub(super) fn guarded_track_dir_entry_names(
    track_dir: &Path,
    project_root: &Path,
) -> Result<Vec<std::ffi::OsString>, RefVerifyError> {
    use std::os::unix::io::AsRawFd;

    let dir = open_guarded_directory(track_dir, project_root).map_err(|e| {
        RefVerifyError::VerifierPort {
            message: format!("cannot open track directory '{}': {e}", track_dir.display()),
        }
    })?;
    let fd_path = PathBuf::from(format!("/proc/self/fd/{}", dir.as_raw_fd()));
    let read_dir = std::fs::read_dir(&fd_path).map_err(|e| RefVerifyError::VerifierPort {
        message: format!("cannot read guarded track directory '{}': {e}", track_dir.display()),
    })?;
    let mut names = Vec::new();
    for entry in read_dir {
        let entry = entry.map_err(|e| RefVerifyError::VerifierPort {
            message: format!("error reading guarded track directory entry: {e}"),
        })?;
        names.push(entry.file_name());
    }
    Ok(names)
}

#[cfg(not(unix))]
pub(super) fn guarded_track_dir_entry_names(
    track_dir: &Path,
    _project_root: &Path,
) -> Result<Vec<std::ffi::OsString>, RefVerifyError> {
    Err(RefVerifyError::VerifierPort {
        message: format!(
            "ref-verify layer discovery requires descriptor-pinned directory enumeration; unsupported on this platform for '{}'",
            track_dir.display()
        ),
    })
}

// ---------------------------------------------------------------------------
// Locked atomic cache writes
// ---------------------------------------------------------------------------

pub(super) struct CacheWriteGuard {
    _lock: std::fs::File,
    #[cfg(unix)]
    pub(super) parent_dir: std::fs::File,
}

impl CacheWriteGuard {
    pub(super) fn acquire(path: &Path, trusted_root: &Path) -> Result<Self, RefVerifyError> {
        #[cfg(unix)]
        {
            let parent = path.parent().ok_or_else(|| RefVerifyError::CachePersistence {
                message: format!("cache path has no parent directory: '{}'", path.display()),
            })?;
            let parent_dir = open_guarded_directory(parent, trusted_root).map_err(|e| {
                RefVerifyError::CachePersistence {
                    message: format!("cannot open cache directory '{}': {e}", parent.display()),
                }
            })?;
            let lock_path = path.with_extension("json.lock");
            let lock_name =
                lock_path.file_name().ok_or_else(|| RefVerifyError::CachePersistence {
                    message: format!("cache lock path has no file name: '{}'", lock_path.display()),
                })?;
            let lock_file = open_nofollow_lock_in_dir(&parent_dir, lock_name).map_err(|e| {
                RefVerifyError::CachePersistence {
                    message: format!(
                        "cannot open verify-cache lock '{}': {e}",
                        lock_path.display()
                    ),
                }
            })?;
            let lock_meta = lock_file.metadata().map_err(|e| RefVerifyError::CachePersistence {
                message: format!("cannot stat verify-cache lock '{}': {e}", lock_path.display()),
            })?;
            if !lock_meta.is_file() {
                return Err(RefVerifyError::CachePersistence {
                    message: format!(
                        "verify-cache lock is not a regular file: '{}'",
                        lock_path.display()
                    ),
                });
            }
            verify_opened_file_within_root(&lock_file, trusted_root, &lock_path).map_err(|e| {
                RefVerifyError::CachePersistence {
                    message: format!(
                        "verify-cache lock guard failed for '{}': {e}",
                        lock_path.display()
                    ),
                }
            })?;
            lock_file.lock_exclusive().map_err(|e| RefVerifyError::CachePersistence {
                message: format!("cannot lock verify-cache '{}': {e}", lock_path.display()),
            })?;
            Ok(Self { _lock: lock_file, parent_dir })
        }
        #[cfg(not(unix))]
        {
            let _ = (path, trusted_root);
            Err(RefVerifyError::CachePersistence {
                message:
                    "ref-verify cache writes require descriptor-pinned opening; unsupported on this platform"
                        .to_owned(),
            })
        }
    }
}

#[cfg(unix)]
static CACHE_TMP_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

#[cfg(unix)]
fn open_nofollow_lock_in_dir(
    parent_dir: &std::fs::File,
    lock_name: &std::ffi::OsStr,
) -> std::io::Result<std::fs::File> {
    use std::os::unix::fs::OpenOptionsExt;
    std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(proc_fd_child_path(parent_dir, lock_name))
}

#[cfg(unix)]
pub(super) fn atomic_write_guarded_file(
    path: &Path,
    parent_dir: &std::fs::File,
    content: &[u8],
) -> std::io::Result<()> {
    use std::os::unix::fs::OpenOptionsExt;
    let target_name = path.file_name().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::InvalidInput, "path has no file name")
    })?;
    let counter = CACHE_TMP_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    let tmp_name = format!(".tmp-ref-verify-cache-{}-{nanos}-{counter}", std::process::id());
    let tmp_path = proc_fd_child_path(parent_dir, std::ffi::OsStr::new(&tmp_name));
    let target_path = proc_fd_child_path(parent_dir, target_name);
    let mut tmp_file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(&tmp_path)?;
    if let Err(e) = tmp_file.write_all(content).and_then(|()| tmp_file.sync_all()) {
        drop(tmp_file);
        let _ = std::fs::remove_file(&tmp_path);
        return Err(e);
    }
    drop(tmp_file);
    if let Err(e) = std::fs::rename(&tmp_path, &target_path) {
        let _ = std::fs::remove_file(&tmp_path);
        return Err(e);
    }
    parent_dir.sync_all()
}

#[cfg(not(unix))]
pub(super) fn atomic_write_guarded_file(
    path: &Path,
    _trusted_root: &Path,
    _content: &[u8],
) -> std::io::Result<()> {
    Err(std::io::Error::other(format!(
        "ref-verify cache writes require descriptor-pinned opening; unsupported on this platform for '{}'",
        path.display()
    )))
}
