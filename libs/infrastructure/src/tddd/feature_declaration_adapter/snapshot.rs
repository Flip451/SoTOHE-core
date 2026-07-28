//! Immutable feature-declaration baseline snapshot publication.

use std::fs::{File, OpenOptions};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use domain::tddd::test_obligation::ids::DiagnosticMessage;

use super::{SNAPSHOT_FILE, diagnostic, read_bytes};
use crate::track::symlink_guard::reject_symlinks_below;

static SNAPSHOT_TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Debug)]
pub(super) enum SnapshotPublicationError {
    Mismatch,
    Read(DiagnosticMessage),
    Write(DiagnosticMessage),
}

pub(super) fn write_first_snapshot(
    path: &Path,
    trusted_root: &Path,
    bytes: &[u8],
) -> Result<(), SnapshotPublicationError> {
    write_first_snapshot_after_temporary_write(path, trusted_root, bytes, |_| {})
}

/// Creates the immutable baseline snapshot without ever publishing partial bytes.
///
/// The temporary file is created below the trusted root, fully synced, then published with a
/// hard link. `hard_link` has no replacement behavior, so a concurrent writer cannot replace
/// the snapshot selected by the first successful publisher.
pub(super) fn write_first_snapshot_after_temporary_write(
    path: &Path,
    trusted_root: &Path,
    bytes: &[u8],
    after_temporary_write: impl FnOnce(&Path),
) -> Result<(), SnapshotPublicationError> {
    match reject_symlinks_below(path, trusted_root) {
        Ok(true) => return compare_snapshot(path, trusted_root, bytes),
        Ok(false) => {}
        Err(error) => return Err(SnapshotPublicationError::Write(diagnostic(error.to_string()))),
    }
    let (mut file, temporary) = create_snapshot_temporary_file(path, trusted_root)?;
    if let Err(error) = file.write_all(bytes) {
        drop(file);
        return Err(clean_up_temporary(
            &temporary,
            SnapshotPublicationError::Write(diagnostic(error.to_string())),
        ));
    }
    if let Err(error) = file.sync_all() {
        drop(file);
        return Err(clean_up_temporary(
            &temporary,
            SnapshotPublicationError::Write(diagnostic(error.to_string())),
        ));
    }
    drop(file);

    after_temporary_write(&temporary);

    let publication = match reject_symlinks_below(path, trusted_root) {
        Ok(false) => match std::fs::hard_link(&temporary, path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                compare_snapshot(path, trusted_root, bytes)
            }
            Err(error) => Err(SnapshotPublicationError::Write(diagnostic(error.to_string()))),
        },
        Ok(true) => compare_snapshot(path, trusted_root, bytes),
        Err(error) => Err(SnapshotPublicationError::Write(diagnostic(error.to_string()))),
    };
    if let Err(error) = publication {
        return Err(clean_up_temporary(&temporary, error));
    }
    if let Err(error) = std::fs::remove_file(&temporary) {
        return Err(clean_up_temporary(
            &temporary,
            SnapshotPublicationError::Write(diagnostic(error.to_string())),
        ));
    }
    let parent = path.parent().ok_or_else(|| {
        SnapshotPublicationError::Write(diagnostic("baseline snapshot has no parent".to_owned()))
    })?;
    File::open(parent)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| SnapshotPublicationError::Write(diagnostic(error.to_string())))
}

fn create_snapshot_temporary_file(
    path: &Path,
    trusted_root: &Path,
) -> Result<(File, PathBuf), SnapshotPublicationError> {
    let parent = path.parent().ok_or_else(|| {
        SnapshotPublicationError::Write(diagnostic("baseline snapshot has no parent".to_owned()))
    })?;
    for _ in 0..1024 {
        let sequence = SNAPSHOT_TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let temporary =
            parent.join(format!(".{SNAPSHOT_FILE}.{}.{sequence}.tmp", std::process::id()));
        match reject_symlinks_below(&temporary, trusted_root) {
            Ok(false) => {}
            Ok(true) => continue,
            Err(error) => {
                return Err(SnapshotPublicationError::Write(diagnostic(error.to_string())));
            }
        }
        match OpenOptions::new().write(true).create_new(true).open(&temporary) {
            Ok(file) => return Ok((file, temporary)),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(SnapshotPublicationError::Write(diagnostic(error.to_string())));
            }
        }
    }
    Err(SnapshotPublicationError::Write(diagnostic(
        "unable to allocate baseline snapshot temporary file".to_owned(),
    )))
}

fn clean_up_temporary(
    temporary: &Path,
    error: SnapshotPublicationError,
) -> SnapshotPublicationError {
    if let Err(cleanup_error) = std::fs::remove_file(temporary)
        && cleanup_error.kind() != std::io::ErrorKind::NotFound
    {
        return SnapshotPublicationError::Write(diagnostic(format!(
            "{}; additionally unable to remove temporary baseline snapshot: {cleanup_error}",
            snapshot_publication_error_description(&error)
        )));
    }
    error
}

fn snapshot_publication_error_description(error: &SnapshotPublicationError) -> String {
    match error {
        SnapshotPublicationError::Mismatch => {
            "baseline snapshot contains different declaration bytes".to_owned()
        }
        SnapshotPublicationError::Read(reason) | SnapshotPublicationError::Write(reason) => {
            reason.as_str().to_owned()
        }
    }
}

fn compare_snapshot(
    path: &Path,
    trusted_root: &Path,
    bytes: &[u8],
) -> Result<(), SnapshotPublicationError> {
    let Some(snapshot) = read_bytes(path, trusted_root).map_err(SnapshotPublicationError::Read)?
    else {
        return Err(SnapshotPublicationError::Read(diagnostic(
            "baseline snapshot disappeared during write".to_owned(),
        )));
    };
    if snapshot == bytes { Ok(()) } else { Err(SnapshotPublicationError::Mismatch) }
}
