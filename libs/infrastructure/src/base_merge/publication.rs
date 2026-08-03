//! Atomic publication and bounded recovery storage for commit-pinned baselines.

use std::fs;
use std::path::{Path, PathBuf};

use super::cleanup_tree::{remove_tree_bounded, sync_tree};
use usecase::base_merge::BaselineReplacementError;
use usecase::git_workflow::DiagnosticText;

use crate::track::atomic_write::atomic_write_file;
use crate::track::symlink_guard::{reject_symlinks_below, reject_symlinks_up_to_root};

use super::BASELINE_REPLACEMENT_PHASE_MARKER;

/// Exchanges the prepared tree with the active track. The recovery slot lives
/// outside `track/items`, so even an interrupted exchange cannot be enumerated
/// as another active track by view regeneration.
pub(super) fn publish_baseline_replacements(
    track_dir: &Path,
    replacement: &Path,
) -> Result<(), BaselineReplacementError> {
    let track_parent = track_dir.parent().ok_or_else(|| {
        BaselineReplacementError::Publish(DiagnosticText::new(
            "active track directory has no parent directory",
        ))
    })?;
    let replacement_parent = replacement.parent().ok_or_else(|| {
        BaselineReplacementError::Publish(DiagnosticText::new(
            "baseline recovery slot has no parent directory",
        ))
    })?;
    reject_symlinks_up_to_root(replacement_parent).map_err(|error| {
        BaselineReplacementError::Publish(DiagnosticText::new(format!(
            "cannot inspect baseline recovery parent directory: {error}"
        )))
    })?;
    // Revalidate immediately before taking the parent handle. The handle is
    // opened with NOFOLLOW so a concurrent substitution of `track/items` is
    // rejected rather than redirecting the exchange to another directory.
    reject_symlinks_up_to_root(track_parent).map_err(|error| {
        BaselineReplacementError::Publish(DiagnosticText::new(format!(
            "cannot inspect active track parent directory: {error}"
        )))
    })?;
    reject_symlinks_below(track_dir, track_parent).map_err(|error| {
        BaselineReplacementError::Publish(DiagnosticText::new(format!(
            "cannot inspect active track directory: {error}"
        )))
    })?;
    let track_name = track_dir.file_name().ok_or_else(|| {
        BaselineReplacementError::Publish(DiagnosticText::new(
            "active track directory has no directory name",
        ))
    })?;
    let replacement_name = replacement.file_name().ok_or_else(|| {
        BaselineReplacementError::Publish(DiagnosticText::new(
            "baseline recovery slot has no directory name",
        ))
    })?;
    let active_parent = open_directory_nofollow(track_parent).map_err(|error| {
        BaselineReplacementError::Publish(DiagnosticText::new(format!(
            "cannot open active track parent directory {}: {error}",
            track_parent.display()
        )))
    })?;
    let recovery_parent_file = open_directory_nofollow(replacement_parent).map_err(|error| {
        BaselineReplacementError::Publish(DiagnosticText::new(format!(
            "cannot open baseline recovery parent directory {}: {error}",
            replacement_parent.display()
        )))
    })?;

    let prepared_phase_marker = replacement.join(BASELINE_REPLACEMENT_PHASE_MARKER);
    match fs::symlink_metadata(&prepared_phase_marker) {
        Ok(metadata) if metadata.is_file() => {}
        Ok(_) => {
            return Err(BaselineReplacementError::Publish(DiagnosticText::new(
                "baseline replacement phase marker is not a regular file",
            )));
        }
        Err(error) => {
            return Err(BaselineReplacementError::Publish(DiagnosticText::new(format!(
                "cannot inspect baseline replacement phase marker: {error}"
            ))));
        }
    }

    sync_tree(replacement, replacement_parent).map_err(|error| {
        BaselineReplacementError::Publish(DiagnosticText::new(format!(
            "cannot make staged baseline replacement durable: {error}"
        )))
    })?;
    rustix::fs::renameat_with(
        &active_parent,
        track_name,
        &recovery_parent_file,
        replacement_name,
        rustix::fs::RenameFlags::EXCHANGE,
    )
    .map_err(|error| {
        BaselineReplacementError::Publish(DiagnosticText::new(format!(
            "cannot atomically publish complete baseline replacement: {error}"
        )))
    })?;

    // Persist the destination first, then the source, so a crash cannot lose
    // the recovery slot after the source entry has been removed.
    if let Err(error) = recovery_parent_file.sync_all() {
        return restore_after_baseline_exchange(
            &active_parent,
            &recovery_parent_file,
            track_name,
            replacement_name,
            replacement,
            replacement_parent,
            DiagnosticText::new(format!(
                "published baseline replacement but cannot persist recovery directory: {error}"
            )),
        );
    }
    if let Err(error) = active_parent.sync_all() {
        return restore_after_baseline_exchange(
            &active_parent,
            &recovery_parent_file,
            track_name,
            replacement_name,
            replacement,
            replacement_parent,
            DiagnosticText::new(format!(
                "published baseline replacement but cannot persist active directory: {error}"
            )),
        );
    }

    // The exchange is durable before the phase marker is cleared. If the
    // marker cleanup itself fails, rollback remains safe; if a crash occurs
    // in this window, restart sees the marker plus the deterministic recovery
    // copy and completes the transaction.
    let active_phase_marker = track_dir.join(BASELINE_REPLACEMENT_PHASE_MARKER);
    if let Err(error) = fs::remove_file(&active_phase_marker) {
        return restore_after_baseline_exchange(
            &active_parent,
            &recovery_parent_file,
            track_name,
            replacement_name,
            replacement,
            replacement_parent,
            DiagnosticText::new(format!(
                "published baseline replacement but cannot clear phase marker: {error}"
            )),
        );
    }
    if let Err(error) = fs::File::open(track_dir).and_then(|directory| directory.sync_all()) {
        return restore_after_baseline_exchange(
            &active_parent,
            &recovery_parent_file,
            track_name,
            replacement_name,
            replacement,
            replacement_parent,
            DiagnosticText::new(format!(
                "published baseline replacement but cannot persist phase marker removal: {error}"
            )),
        );
    }
    if let Err(error) = active_parent.sync_all() {
        return restore_after_baseline_exchange(
            &active_parent,
            &recovery_parent_file,
            track_name,
            replacement_name,
            replacement,
            replacement_parent,
            DiagnosticText::new(format!(
                "published baseline replacement but cannot persist phase marker removal in parent: {error}"
            )),
        );
    }
    Ok(())
}

fn open_directory_nofollow(path: &Path) -> std::io::Result<fs::File> {
    rustix::fs::open(
        path,
        rustix::fs::OFlags::RDONLY
            | rustix::fs::OFlags::DIRECTORY
            | rustix::fs::OFlags::NOFOLLOW
            | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::empty(),
    )
    .map(fs::File::from)
    .map_err(Into::into)
}

/// A recovery copy moved to a pending directory while SyncBase is being
/// published. The directory is explicitly cleaned once the transaction
/// succeeds; on an error it remains available for recovery.
struct ExternalRecoveryDirectory {
    path: PathBuf,
}

/// Completes a publication that exchanged the active track but was interrupted
/// before the prepared prior track could be promoted to the canonical recovery
/// slot. A prepared track is durable and bounded before it is adopted; an
/// incomplete pre-publication directory is discarded so the next run can
/// prepare a fresh candidate.
pub(super) fn reconcile_interrupted_replacement(
    replacement: &Path,
    recovery_slot: &Path,
    recovery_root: &Path,
) -> Result<(), String> {
    reject_symlinks_below(replacement, recovery_root)
        .map_err(|error| format!("cannot inspect interrupted baseline replacement: {error}"))?;
    let metadata = fs::symlink_metadata(replacement)
        .map_err(|error| format!("cannot inspect interrupted baseline replacement: {error}"))?;
    if !metadata.is_dir() {
        return Err(format!(
            "interrupted baseline replacement is not a directory: {}",
            replacement.display()
        ));
    }
    let phase_marker = replacement.join(BASELINE_REPLACEMENT_PHASE_MARKER);
    if path_exists(&phase_marker)? {
        remove_tree_bounded(replacement, recovery_root)?;
        sync_directory(recovery_root)?;
        return Ok(());
    }
    let metadata_marker = replacement.join("metadata.json");
    let metadata_is_regular = match fs::symlink_metadata(&metadata_marker) {
        Ok(metadata) => metadata.is_file(),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
        Err(error) => {
            return Err(format!(
                "cannot inspect interrupted baseline replacement metadata: {error}"
            ));
        }
    };
    if !metadata_is_regular {
        remove_tree_bounded(replacement, recovery_root)?;
        sync_directory(recovery_root)?;
        return Ok(());
    }
    sync_tree(replacement, recovery_root)?;
    if path_exists(recovery_slot)? {
        reject_symlinks_below(recovery_slot, recovery_root)
            .map_err(|error| format!("cannot inspect prior baseline recovery slot: {error}"))?;
        remove_tree_bounded(recovery_slot, recovery_root)
            .map_err(|error| format!("cannot clear prior baseline recovery slot: {error}"))?;
    }
    fs::rename(replacement, recovery_slot)
        .map_err(|error| format!("cannot promote interrupted baseline recovery slot: {error}"))?;
    sync_directory(recovery_root)
}

pub(super) fn write_replacement_phase_marker(
    replacement: &Path,
) -> Result<(), BaselineReplacementError> {
    let marker = replacement.join(BASELINE_REPLACEMENT_PHASE_MARKER);
    atomic_write_file(&marker, b"prepared\n").map_err(|error| {
        BaselineReplacementError::Isolation(DiagnosticText::new(format!(
            "cannot persist baseline replacement phase marker: {error}"
        )))
    })?;
    sync_directory(replacement)
        .map_err(|error| BaselineReplacementError::Isolation(DiagnosticText::new(error)))
}

pub(super) struct StagedRecoveryCopy {
    _temporary_directory: ExternalRecoveryDirectory,
}

impl StagedRecoveryCopy {
    pub(super) fn cleanup(&self) -> Result<(), String> {
        let parent = self
            ._temporary_directory
            .path
            .parent()
            .ok_or_else(|| "external recovery staging directory has no parent".to_owned())?;
        // Preflight the full tree and its bounded traversal budget before any
        // destructive removal so an oversized or symlinked backup cannot be
        // partially deleted during cleanup.
        sync_tree(&self._temporary_directory.path, parent)?;
        remove_tree_bounded(&self._temporary_directory.path, parent)?;
        sync_directory(parent)
    }
}

/// Moves the recovery slot out of its authoritative name before SyncBase is
/// published. The move is reversible, so a failed stamp write can restore the
/// prior track without deleting it after the stamp becomes authoritative.
pub(super) fn stage_recovery_copy_for_sync(
    path: &Path,
    trusted_root: &Path,
    temporary_parent: &Path,
) -> Result<Option<StagedRecoveryCopy>, String> {
    validate_recovery_cleanup_target(path, trusted_root)?;
    reject_symlinks_up_to_root(temporary_parent)
        .map_err(|error| format!("cannot inspect recovery staging parent: {error}"))?;
    let name =
        path.file_name().ok_or_else(|| format!("recovery copy has no name: {}", path.display()))?;
    let temporary_path =
        temporary_parent.join(format!(".sotp-baseline-recovery-{}", name.to_string_lossy()));
    reject_symlinks_below(&temporary_path, temporary_parent)
        .map_err(|error| format!("cannot inspect recovery staging directory: {error}"))?;
    let temporary_directory = ExternalRecoveryDirectory { path: temporary_path };
    let staged = temporary_directory.path.join(name);
    if path_exists(&temporary_directory.path)? {
        if path_exists(path)? {
            // A failed stamp write may have restored the canonical slot while
            // leaving an empty or stale pending directory. Reconcile the
            // duplicate before allowing the new stamp to become authoritative.
            remove_tree_bounded(&temporary_directory.path, temporary_parent)
                .map_err(|error| format!("cannot reconcile pending recovery copy: {error}"))?;
            sync_directory(temporary_parent)?;
        } else {
            return Ok(Some(StagedRecoveryCopy { _temporary_directory: temporary_directory }));
        }
    }
    if !path_exists(path)? {
        return Ok(None);
    }
    fs::create_dir(&temporary_directory.path)
        .map_err(|error| format!("cannot create recovery staging directory: {error}"))?;
    if let Err(error) = fs::rename(path, &staged) {
        let cleanup = remove_tree_bounded(&temporary_directory.path, temporary_parent);
        return Err(match cleanup {
            Ok(()) => format!("cannot stage baseline recovery copy: {error}"),
            Err(cleanup) => format!("cannot stage baseline recovery copy: {error}; {cleanup}"),
        });
    }
    let durability = sync_tree(&temporary_directory.path, temporary_parent)
        .and_then(|()| sync_directory(temporary_parent))
        .and_then(|()| sync_directory(trusted_root));
    if let Err(error) = durability {
        let restoration = fs::rename(&staged, path)
            .and_then(|()| sync_directory_io(trusted_root))
            .map_err(|restore| format!("cannot restore staged recovery copy: {restore}"));
        return Err(match restoration {
            Ok(()) => match remove_tree_bounded(&temporary_directory.path, temporary_parent) {
                Ok(()) => format!("cannot persist staged baseline recovery copy: {error}"),
                Err(cleanup) => {
                    format!("cannot persist staged baseline recovery copy: {error}; {cleanup}")
                }
            },
            Err(restoration) => {
                format!("cannot persist staged baseline recovery copy: {error}; {restoration}")
            }
        });
    }
    Ok(Some(StagedRecoveryCopy { _temporary_directory: temporary_directory }))
}

pub(super) fn path_exists(path: &Path) -> Result<bool, String> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(format!("cannot inspect {}: {error}", path.display())),
    }
}

pub(super) fn sync_directory(path: &Path) -> Result<(), String> {
    sync_directory_io(path).map_err(|error| format!("cannot sync {}: {error}", path.display()))
}

fn sync_directory_io(path: &Path) -> std::io::Result<()> {
    fs::File::open(path).and_then(|directory| directory.sync_all())
}

/// Validates the recovery cleanup anchor before an authoritative SyncBase
/// record is published. The bounded remover intentionally stops at
/// `trusted_root`, so that anchor must itself be a real directory and every
/// component below it must be checked independently.
pub(super) fn validate_recovery_cleanup_target(
    path: &Path,
    trusted_root: &Path,
) -> Result<(), String> {
    reject_symlinks_up_to_root(trusted_root)
        .map_err(|error| format!("cannot inspect baseline recovery root: {error}"))?;
    match fs::symlink_metadata(trusted_root) {
        Ok(metadata) if !metadata.is_dir() => {
            return Err(format!(
                "baseline recovery root is not a directory: {}",
                trusted_root.display()
            ));
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => {
            return Err(format!(
                "cannot inspect baseline recovery root {}: {error}",
                trusted_root.display()
            ));
        }
    }
    reject_symlinks_below(path, trusted_root)
        .map_err(|error| format!("cannot inspect baseline recovery slot: {error}"))?;
    Ok(())
}

fn restore_after_baseline_exchange(
    active_parent: &fs::File,
    recovery_parent: &fs::File,
    track_name: &std::ffi::OsStr,
    replacement_name: &std::ffi::OsStr,
    replacement: &Path,
    replacement_parent: &Path,
    publish: DiagnosticText,
) -> Result<(), BaselineReplacementError> {
    let restoration = rustix::fs::renameat_with(
        active_parent,
        track_name,
        recovery_parent,
        replacement_name,
        rustix::fs::RenameFlags::EXCHANGE,
    )
    .map_err(|error| {
        DiagnosticText::new(format!(
            "cannot restore prior track after publication failure: {error}"
        ))
    })
    // After the rollback exchange, the active directory is the destination
    // containing the restored prior track. Persist it before syncing the
    // recovery source, otherwise a crash can lose the durable backup without
    // durably restoring the active entry.
    .and_then(|()| {
        active_parent.sync_all().map_err(|error| {
            DiagnosticText::new(format!(
                "cannot persist restored active directory after publication failure: {error}"
            ))
        })
    })
    .and_then(|()| {
        recovery_parent.sync_all().map_err(|error| {
            DiagnosticText::new(format!(
                "cannot persist restored recovery directory after publication failure: {error}"
            ))
        })
    });
    if let Err(restoration) = restoration {
        return Err(BaselineReplacementError::Restoration { publish, restoration });
    }

    if let Err(marker) = write_replacement_phase_marker(replacement) {
        return Err(BaselineReplacementError::Restoration {
            publish,
            restoration: DiagnosticText::new(format!(
                "prior track restored but cannot persist prepared replacement phase: {marker:?}"
            )),
        });
    }
    if let Err(error) = remove_tree_bounded(replacement, replacement_parent) {
        return Err(BaselineReplacementError::Restoration {
            publish,
            restoration: DiagnosticText::new(format!(
                "prior track restored but failed to remove staged replacement: {error}"
            )),
        });
    }
    if let Err(error) = recovery_parent.sync_all() {
        return Err(BaselineReplacementError::Restoration {
            publish,
            restoration: DiagnosticText::new(format!(
                "prior track restored but cannot persist recovery-slot removal: {error}"
            )),
        });
    }
    Err(BaselineReplacementError::Publish(publish))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_restore_after_exchange_restores_prior_track_and_removes_staged_tree() {
        let fixture = tempfile::tempdir().unwrap();
        let items = fixture.path().join("track/items");
        let active = items.join("cleanup-test");
        let replacement = items.join(".replacement");
        std::fs::create_dir_all(&active).unwrap();
        std::fs::create_dir_all(&replacement).unwrap();
        std::fs::write(active.join("marker"), "new").unwrap();
        std::fs::write(replacement.join("marker"), "prior").unwrap();
        let active_parent = fs::File::open(&items).unwrap();
        let recovery_parent = fs::File::open(&items).unwrap();

        let result = restore_after_baseline_exchange(
            &active_parent,
            &recovery_parent,
            active.file_name().unwrap(),
            replacement.file_name().unwrap(),
            &replacement,
            &items,
            DiagnosticText::new("injected publication failure"),
        );

        assert!(matches!(result, Err(BaselineReplacementError::Publish(_))));
        assert_eq!(std::fs::read_to_string(active.join("marker")).unwrap(), "prior");
        assert!(!replacement.exists(), "the staged replacement must be removed after rollback");
    }

    #[test]
    fn test_restore_after_exchange_reports_typed_failure_when_prior_tree_is_missing() {
        let fixture = tempfile::tempdir().unwrap();
        let items = fixture.path().join("track/items");
        let active = items.join("cleanup-test");
        let replacement = items.join(".replacement");
        std::fs::create_dir_all(&active).unwrap();
        std::fs::write(active.join("marker"), "new").unwrap();
        let active_parent = fs::File::open(&items).unwrap();
        let recovery_parent = fs::File::open(&items).unwrap();

        let result = restore_after_baseline_exchange(
            &active_parent,
            &recovery_parent,
            active.file_name().unwrap(),
            replacement.file_name().unwrap(),
            &replacement,
            &items,
            DiagnosticText::new("injected publication failure"),
        );

        assert!(matches!(result, Err(BaselineReplacementError::Restoration { .. })));
        assert_eq!(std::fs::read_to_string(active.join("marker")).unwrap(), "new");
    }
}
