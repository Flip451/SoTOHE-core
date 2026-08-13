use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use super::BASELINE_REPLACEMENT_PHASE_MARKER;
use super::baseline_support::{
    VIEW_TRANSACTION_PHASE_ROLLBACK, VIEW_TRANSACTION_PHASE_ROLLBACK_EXCHANGED,
    VIEW_TRANSACTION_PHASE_ROLLED_BACK, write_view_transaction_phase,
};
use super::publication::{
    generated_baseline_file_names, path_exists, reconcile_interrupted_replacement, sync_directory,
    write_replacement_phase_marker,
};
use crate::track::atomic_write::atomic_write_file;
use crate::track::registry_lock::acquire_registry_lock;
use crate::track::symlink_guard::{reject_symlinks_below, reject_symlinks_up_to_root};

pub(super) type RenderedViewSnapshot = Vec<Option<Vec<u8>>>;

pub(super) fn snapshot_rendered_views(
    workspace_root: &Path,
    track_dir: &Path,
    names: &BTreeSet<String>,
) -> Result<RenderedViewSnapshot, String> {
    let mut total_bytes = 0_u64;
    names
        .iter()
        .map(|name| {
            let remaining =
                super::MAX_CLEANUP_TREE_BYTES.checked_sub(total_bytes).ok_or_else(|| {
                    format!(
                        "rendered-view snapshot exceeds {} bytes",
                        super::MAX_CLEANUP_TREE_BYTES
                    )
                })?;
            let content = read_optional_rendered_file(
                &track_dir.join(name),
                workspace_root,
                remaining.min(super::MAX_CLEANUP_FILE_BYTES),
            )?;
            if let Some(content) = &content {
                total_bytes = total_bytes
                    .checked_add(u64::try_from(content.len()).map_err(|error| error.to_string())?)
                    .ok_or_else(|| {
                        format!(
                            "rendered-view snapshot exceeds {} bytes",
                            super::MAX_CLEANUP_TREE_BYTES
                        )
                    })?;
            }
            Ok(content)
        })
        .collect()
}

pub(super) fn validate_rendered_view_snapshot(
    workspace_root: &Path,
    track_dir: &Path,
    names: &BTreeSet<String>,
    prior: &RenderedViewSnapshot,
) -> Result<(), String> {
    if snapshot_rendered_views(workspace_root, track_dir, names)?.as_slice() == prior.as_slice() {
        Ok(())
    } else {
        Err("rendered view changed while it was being staged".to_owned())
    }
}

pub(super) fn read_optional_rendered_file(
    path: &Path,
    trusted_root: &Path,
    limit: u64,
) -> Result<Option<Vec<u8>>, String> {
    reject_symlinks_below(path, trusted_root)
        .map_err(|error| format!("cannot inspect rendered file {}: {error}", path.display()))?;
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_file() => {
            super::read_regular_file_bounded(path, trusted_root, limit).map(Some)
        }
        Ok(_) => Err(format!("rendered file is not a regular file: {}", path.display())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!("cannot inspect rendered file {}: {error}", path.display())),
    }
}

pub(super) fn validate_optional_file_unchanged(
    path: &Path,
    trusted_root: &Path,
    prior: Option<&[u8]>,
) -> Result<(), String> {
    let current = read_optional_rendered_file(path, trusted_root, super::MAX_CLEANUP_FILE_BYTES)?;
    if current.as_deref() == prior {
        Ok(())
    } else {
        Err(format!("rendered file changed while it was being staged: {}", path.display()))
    }
}

pub(super) fn publish_registry_if_unchanged(
    path: &Path,
    trusted_root: &Path,
    prior: Option<&[u8]>,
    rendered: &[u8],
) -> Result<bool, String> {
    let _registry_lock = acquire_registry_lock(path, trusted_root)?;
    validate_optional_file_unchanged(path, trusted_root, prior)?;
    if prior == Some(rendered) {
        return Ok(false);
    }
    atomic_write_file(path, rendered)
        .map_err(|error| format!("cannot publish rendered registry: {error}"))?;
    if read_optional_rendered_file(path, trusted_root, super::MAX_CLEANUP_FILE_BYTES)?.as_deref()
        != Some(rendered)
    {
        return Err(format!("rendered file changed during publication: {}", path.display()));
    }
    Ok(true)
}

pub(super) fn reconcile_baseline_publication_before_views(
    workspace_root: &Path,
    track_dir: &Path,
    recovery_root: &Path,
    track_id: &str,
) -> Result<(), String> {
    let generated = generated_baseline_file_names(workspace_root)?;
    let replacement = recovery_root.join(format!(".sotp-baseline-replacement-{track_id}"));
    let recovery_slot = recovery_root.join(track_id);
    let active_marker = track_dir.join(BASELINE_REPLACEMENT_PHASE_MARKER);
    reject_symlinks_below(&replacement, recovery_root).map_err(|error| {
        format!("cannot inspect baseline replacement before view staging: {error}")
    })?;
    reject_symlinks_below(&recovery_slot, recovery_root).map_err(|error| {
        format!("cannot inspect baseline recovery slot before view staging: {error}")
    })?;
    reject_symlinks_below(&active_marker, track_dir).map_err(|error| {
        format!("cannot inspect active baseline marker before view staging: {error}")
    })?;
    let had_replacement = path_exists(&replacement)?;
    if had_replacement {
        reconcile_interrupted_replacement(
            &replacement,
            &recovery_slot,
            recovery_root,
            track_dir,
            &generated,
        )?;
    }
    if path_exists(&active_marker)? {
        if !had_replacement && !path_exists(&recovery_slot)? {
            return Err("active track contains an unreconciled baseline replacement phase marker"
                .to_owned());
        }
        fs::remove_file(&active_marker).map_err(|error| {
            format!("cannot clear recovered baseline replacement phase marker: {error}")
        })?;
        sync_directory(track_dir)?;
    }
    Ok(())
}

pub(super) fn restore_view_exchange(
    workspace_root: &Path,
    track_dir: &Path,
    replacement: &Path,
) -> Result<(), String> {
    let track_parent = track_dir
        .parent()
        .ok_or_else(|| "active track directory has no parent directory".to_owned())?;
    let replacement_parent = replacement
        .parent()
        .ok_or_else(|| "view replacement has no parent directory".to_owned())?;
    reject_symlinks_up_to_root(track_parent)
        .map_err(|error| format!("cannot inspect active view parent: {error}"))?;
    reject_symlinks_up_to_root(replacement_parent)
        .map_err(|error| format!("cannot inspect view replacement parent: {error}"))?;
    reject_symlinks_below(track_dir, workspace_root)
        .map_err(|error| format!("cannot inspect active view tree: {error}"))?;
    reject_symlinks_below(replacement, workspace_root)
        .map_err(|error| format!("cannot inspect view replacement tree: {error}"))?;
    let active_parent = open_directory(track_parent, "active view parent")?;
    let replacement_parent_file = open_directory(replacement_parent, "view replacement parent")?;
    let track_name =
        track_dir.file_name().ok_or_else(|| "active track directory has no name".to_owned())?;
    let replacement_name =
        replacement.file_name().ok_or_else(|| "view replacement has no name".to_owned())?;
    write_replacement_phase_marker(track_dir)
        .map_err(|error| format!("cannot persist rollback view transaction marker: {error:?}"))?;
    rustix::fs::renameat_with(
        &active_parent,
        track_name,
        &replacement_parent_file,
        replacement_name,
        rustix::fs::RenameFlags::EXCHANGE,
    )
    .map_err(|error| format!("cannot restore prior rendered-view tree: {error}"))?;
    active_parent
        .sync_all()
        .and_then(|()| replacement_parent_file.sync_all())
        .map_err(|error| format!("cannot persist restored rendered-view tree: {error}"))
}

pub(super) fn rollback_view_publication(
    transaction: &Path,
    workspace_root: &Path,
    track_dir: &Path,
    replacement: &Path,
    registry_path: &Path,
    prior_registry: Option<&[u8]>,
    staged_registry: &[u8],
) -> Result<(), String> {
    restore_view_exchange(workspace_root, track_dir, replacement)?;
    write_view_transaction_phase(transaction, VIEW_TRANSACTION_PHASE_ROLLBACK_EXCHANGED)?;
    restore_rendered_registry_if_unchanged(
        registry_path,
        workspace_root,
        prior_registry,
        Some(staged_registry),
    )?;
    write_view_transaction_phase(transaction, VIEW_TRANSACTION_PHASE_ROLLED_BACK)
}

pub(super) fn restore_rendered_registry_if_unchanged(
    registry_path: &Path,
    workspace_root: &Path,
    prior_registry: Option<&[u8]>,
    staged_registry: Option<&[u8]>,
) -> Result<(), String> {
    let _registry_lock = acquire_registry_lock(registry_path, workspace_root)?;
    let current = read_optional_file(registry_path, workspace_root)?;
    if current.as_deref() == prior_registry {
        return Ok(());
    }
    if current.as_deref() != staged_registry {
        return Err("cannot roll back rendered registry after concurrent change".to_owned());
    }
    match prior_registry {
        Some(bytes) => atomic_write_file(registry_path, bytes)
            .map_err(|error| format!("cannot restore prior rendered registry: {error}"))?,
        None => {
            fs::remove_file(registry_path).map_err(|error| {
                format!("cannot remove rendered registry during rollback: {error}")
            })?;
            let parent = registry_path
                .parent()
                .ok_or_else(|| "rendered registry has no parent directory".to_owned())?;
            sync_directory(parent)?;
        }
    }
    if read_optional_file(registry_path, workspace_root)?.as_deref() != prior_registry {
        return Err("rendered registry changed during rollback".to_owned());
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(super) fn rollback_if_published(
    result: Result<(), String>,
    transaction_prepared: bool,
    exchanged: bool,
    transaction: &Path,
    workspace_root: &Path,
    track_dir: &Path,
    replacement: &Path,
    registry_path: &Path,
    prior_registry: Option<&[u8]>,
    staged_registry: Option<&[u8]>,
) -> Result<(), String> {
    if result.is_ok() || !transaction_prepared || !exchanged {
        return result;
    }
    let Some(staged_registry) = staged_registry else {
        return result.map_err(|error| {
            format!("{error}; rendered-view transaction has no staged registry for rollback")
        });
    };
    if let Err(phase) = write_view_transaction_phase(transaction, VIEW_TRANSACTION_PHASE_ROLLBACK) {
        return result.map_err(|error| {
            format!("{error}; cannot persist rendered-view rollback phase: {phase}")
        });
    }
    match rollback_view_publication(
        transaction,
        workspace_root,
        track_dir,
        replacement,
        registry_path,
        prior_registry,
        staged_registry,
    ) {
        Ok(()) => result,
        Err(rollback) => result.map_err(|error| format!("{error}; {rollback}")),
    }
}

fn read_optional_file(path: &Path, workspace_root: &Path) -> Result<Option<Vec<u8>>, String> {
    reject_symlinks_below(path, workspace_root)
        .map_err(|error| format!("cannot inspect rendered registry: {error}"))?;
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_file() && !metadata.file_type().is_symlink() => {
            super::read_regular_file_bounded(path, workspace_root, super::MAX_CLEANUP_FILE_BYTES)
                .map(Some)
        }
        Ok(_) => Err("rendered registry is not a regular file".to_owned()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!("cannot inspect rendered registry: {error}")),
    }
}

fn open_directory(path: &Path, label: &str) -> Result<fs::File, String> {
    rustix::fs::open(
        path,
        rustix::fs::OFlags::RDONLY
            | rustix::fs::OFlags::DIRECTORY
            | rustix::fs::OFlags::NOFOLLOW
            | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::empty(),
    )
    .map(fs::File::from)
    .map_err(|error| format!("cannot open {label}: {error}"))
}
