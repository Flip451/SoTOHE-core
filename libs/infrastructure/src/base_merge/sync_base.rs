//! Transactional SyncBase stamping and recovery-copy handling.

use std::fs;
use std::io::Read;
use std::path::Path;

use usecase::base_merge::{BaseMergeCleanupRequest, SyncBaseRecordError};
use usecase::git_workflow::DiagnosticText;

use crate::track::atomic_write::atomic_write_file;
use crate::track::symlink_guard::{reject_symlinks_below, reject_symlinks_up_to_root};

use super::MAX_SYNC_BASE_RECORD_BYTES;
use super::publication::{stage_recovery_copy_for_sync, validate_recovery_cleanup_target};
use super::sync_base_record::{SyncBaseRecord, SyncBaseRecordSchemaVersion, decode, encode};

pub(super) fn write_sync_base_record_atomically(
    request: &BaseMergeCleanupRequest,
) -> Result<(), SyncBaseRecordError> {
    let track_dir = request.workspace_root.join("track/items").join(request.track_id.as_ref());
    let items_dir = request.workspace_root.join("track/items");
    reject_symlinks_up_to_root(&items_dir)
        .map_err(|error| SyncBaseRecordError::Validation(DiagnosticText::new(error.to_string())))?;
    reject_symlinks_below(&track_dir, &items_dir)
        .map_err(|error| SyncBaseRecordError::Validation(DiagnosticText::new(error.to_string())))?;
    if !track_dir.is_dir() {
        return Err(SyncBaseRecordError::Write(DiagnosticText::new(
            "active track directory is unavailable",
        )));
    }

    let record = SyncBaseRecord {
        schema_version: SyncBaseRecordSchemaVersion::V1,
        track_id: request.track_id.clone(),
        base_branch: request.base_branch.clone(),
        base_commit: request.base_commit.clone(),
    };
    let encoded = encode(&record)
        .map_err(|error| SyncBaseRecordError::Generation(DiagnosticText::new(error.to_string())))?;
    let decoded = decode(&encoded)
        .map_err(|error| SyncBaseRecordError::Validation(DiagnosticText::new(error.to_string())))?;
    if decoded != record {
        return Err(SyncBaseRecordError::Validation(DiagnosticText::new(
            "sync-base record failed round-trip validation",
        )));
    }

    let path = track_dir.join(".sync-base.json");
    let mut needs_write = true;
    let mut previous_stamp = None;
    if reject_symlinks_below(&path, &track_dir)
        .map_err(|error| SyncBaseRecordError::Validation(DiagnosticText::new(error.to_string())))?
    {
        let existing = read_regular_file_bounded(&path, &track_dir, MAX_SYNC_BASE_RECORD_BYTES)
            .map_err(|error| SyncBaseRecordError::Write(DiagnosticText::new(error)))?;
        previous_stamp = Some(existing.clone());
        let existing = std::str::from_utf8(&existing).map_err(|error| {
            SyncBaseRecordError::Validation(DiagnosticText::new(format!(
                "existing sync-base record is not UTF-8: {error}"
            )))
        })?;
        match decode(existing) {
            Ok(previous) if previous == record => needs_write = false,
            Err(error) => {
                return Err(SyncBaseRecordError::Validation(DiagnosticText::new(
                    error.to_string(),
                )));
            }
            Ok(_) => {}
        }
    }

    let recovery_root = request.workspace_root.join("track/.sotp-baseline-recovery");
    let recovery_slot = recovery_root.join(request.track_id.as_ref());
    validate_recovery_cleanup_target(&recovery_slot, &recovery_root)
        .map_err(|error| SyncBaseRecordError::Validation(DiagnosticText::new(error)))?;
    // Keep the reversible staging move on the same filesystem and under the
    // already-validated recovery boundary. It is removed explicitly once the
    // SyncBase transaction succeeds.
    let temporary_parent = &recovery_root;

    let staged_recovery =
        stage_recovery_copy_for_sync(&recovery_slot, &recovery_root, temporary_parent)
            .map_err(|error| SyncBaseRecordError::Replacement(DiagnosticText::new(error)))?;
    let write_result = if needs_write {
        match atomic_write_file(&path, encoded.as_bytes()) {
            Ok(()) => Ok(()),
            Err(error) => {
                let write_error = error.to_string();
                match restore_sync_base_stamp(&path, previous_stamp.as_deref(), &track_dir) {
                    Ok(()) => {
                        Err(SyncBaseRecordError::Replacement(DiagnosticText::new(write_error)))
                    }
                    Err(restoration) => Err(SyncBaseRecordError::Replacement(DiagnosticText::new(
                        format!("{write_error}; {restoration}"),
                    ))),
                }
            }
        }
    } else {
        Ok(())
    };
    // Keep a staged recovery copy as the sole recovery location when the
    // stamp write fails. A later call can adopt that deterministic pending
    // path and retry cleanup without ever creating canonical/pending
    // duplicates.
    write_result?;
    if let Some(staged) = staged_recovery {
        if let Err(cleanup) = staged.cleanup() {
            // Destructive cleanup has started. Do not rename a potentially
            // partial tree back into the canonical recovery slot; leave the
            // staged path in place for diagnosis. Keep the newly committed
            // SyncBase marker so it cannot be replaced by an older record.
            return Err(SyncBaseRecordError::Replacement(DiagnosticText::new(format!(
                "baseline recovery cleanup failed: {cleanup}"
            ))));
        }
    }
    Ok(())
}

fn restore_sync_base_stamp(
    path: &Path,
    previous: Option<&[u8]>,
    trusted_root: &Path,
) -> Result<(), String> {
    reject_symlinks_below(path, trusted_root)
        .map_err(|error| format!("cannot inspect SyncBase restoration target: {error}"))?;
    match previous {
        Some(previous) => atomic_write_file(path, previous)
            .map_err(|error| format!("cannot restore prior SyncBase record: {error}")),
        None => {
            match fs::symlink_metadata(path) {
                Ok(metadata) if metadata.file_type().is_symlink() => {
                    return Err(format!(
                        "refusing symlinked SyncBase restoration target: {}",
                        path.display()
                    ));
                }
                Ok(metadata) if metadata.is_file() => fs::remove_file(path).map_err(|error| {
                    format!("cannot remove partially published SyncBase record: {error}")
                })?,
                Ok(_) => {
                    return Err(format!(
                        "refusing non-regular SyncBase restoration target: {}",
                        path.display()
                    ));
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
                Err(error) => {
                    return Err(format!("cannot inspect SyncBase restoration target: {error}"));
                }
            }
            fs::File::open(trusted_root)
                .and_then(|directory| directory.sync_all())
                .map_err(|error| format!("cannot persist SyncBase restoration: {error}"))
        }
    }
}

pub(super) fn read_regular_file_bounded(
    path: &Path,
    trusted_root: &Path,
    limit: u64,
) -> Result<Vec<u8>, String> {
    reject_symlinks_below(path, trusted_root)
        .map_err(|error| format!("cannot inspect {}: {error}", path.display()))?;
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("cannot inspect {}: {error}", path.display()))?;
    if metadata.file_type().is_symlink() {
        return Err(format!("refusing symlinked file: {}", path.display()));
    }
    if !metadata.is_file() {
        return Err(format!("refusing non-regular file: {}", path.display()));
    }
    if metadata.len() > limit {
        return Err(format!("file exceeds read-size limit: {}", path.display()));
    }
    let mut file =
        fs::File::open(path).map_err(|error| format!("cannot open {}: {error}", path.display()))?;
    let opened_metadata = file
        .metadata()
        .map_err(|error| format!("cannot inspect opened file {}: {error}", path.display()))?;
    if !opened_metadata.is_file() {
        return Err(format!("refusing non-regular file: {}", path.display()));
    }
    let mut content = Vec::with_capacity(usize::try_from(metadata.len()).unwrap_or(0));
    Read::by_ref(&mut file)
        .take(limit.saturating_add(1))
        .read_to_end(&mut content)
        .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    if u64::try_from(content.len()).map_or(true, |length| length > limit) {
        return Err(format!("file exceeds read-size limit: {}", path.display()));
    }
    Ok(content)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use domain::branch_strategy::BaseBranchName;
    use domain::{CommitHash, TrackId};
    use std::fs;
    use std::path::Path;

    fn request(workspace_root: &Path, base_commit: &str) -> BaseMergeCleanupRequest {
        BaseMergeCleanupRequest {
            workspace_root: workspace_root.to_path_buf(),
            track_id: TrackId::try_new("cleanup-test").unwrap(),
            base_branch: BaseBranchName::try_new("develop".to_owned()).unwrap(),
            base_commit: CommitHash::try_new(base_commit.to_owned()).unwrap(),
        }
    }

    #[test]
    fn test_write_sync_base_record_atomically_replaces_existing_record_without_partial_or_temp() {
        let fixture = tempfile::tempdir().unwrap();
        let track_dir = fixture.path().join("track/items/cleanup-test");
        fs::create_dir_all(&track_dir).unwrap();
        let stamp = track_dir.join(".sync-base.json");
        let temporary_replacement =
            track_dir.join(format!(".tmp-.sync-base.json-{}", std::process::id()));
        let first = request(fixture.path(), "0123456789abcdef");
        let later = request(fixture.path(), "fedcba9876543210");

        write_sync_base_record_atomically(&first).unwrap();
        let prior = fs::read(&stamp).unwrap();

        fs::create_dir(&temporary_replacement).unwrap();
        let failed = write_sync_base_record_atomically(&later);
        assert!(matches!(failed, Err(SyncBaseRecordError::Replacement(_))));
        assert_eq!(fs::read(&stamp).unwrap(), prior);
        fs::remove_dir(&temporary_replacement).unwrap();

        write_sync_base_record_atomically(&later).unwrap();
        assert_eq!(
            fs::read_to_string(&stamp).unwrap(),
            r#"{"schema_version":"v1","track_id":"cleanup-test","base_branch":"develop","base_commit":"fedcba9876543210"}"#
        );
        assert!(!temporary_replacement.exists());
    }
}
