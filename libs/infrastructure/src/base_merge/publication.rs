use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use super::cleanup_tree::{remove_tree_bounded, sync_tree, verify_non_baseline_content_matches};
use fs4::fs_std::FileExt as _;
use usecase::base_merge::{BaseMergeCleanupRequest, BaselineReplacementError};
use usecase::git_workflow::DiagnosticText;

use crate::tddd::tddd_layer_bindings_adapter::FsTdddLayerBindingsAdapter;
use crate::track::atomic_write::atomic_write_file;
use crate::track::symlink_guard::{reject_symlinks_below, reject_symlinks_up_to_root};
use domain::tddd::catalogue_v2::TdddLayerBindingsPort;

use super::{BASELINE_REPLACEMENT_PHASE_MARKER, TRACK_WRITER_LOCK_FILE};

const TDDD_FEATURES_BASELINE_FILE: &str = "tddd-features-baseline.json";
pub(super) struct PendingWriterLock {
    request: BaseMergeCleanupRequest,
    file: fs::File,
}

static ACTIVE_WRITER_KEYS: OnceLock<Mutex<BTreeSet<PathBuf>>> = OnceLock::new();

fn active_writer_keys() -> &'static Mutex<BTreeSet<PathBuf>> {
    ACTIVE_WRITER_KEYS.get_or_init(|| Mutex::new(BTreeSet::new()))
}

fn reserve_writer_key(key: &Path) -> Result<(), String> {
    let mut active = active_writer_keys()
        .lock()
        .map_err(|_| "active track writer key state is poisoned".to_owned())?;
    if active.insert(key.to_owned()) {
        Ok(())
    } else {
        Err("another active track cleanup transaction holds the writer lock".to_owned())
    }
}

fn release_writer_key(key: &Path) {
    if let Ok(mut active) = active_writer_keys().lock() {
        active.remove(key);
    }
}

impl Drop for PendingWriterLock {
    fn drop(&mut self) {
        release_writer_key(&track_writer_paths(&self.request).0);
    }
}
fn track_writer_paths(request: &BaseMergeCleanupRequest) -> (PathBuf, PathBuf) {
    let items_dir = request.workspace_root.join("track/items");
    (items_dir.join(request.track_id.as_ref()), items_dir)
}

pub(super) fn with_writer_lock<T, E>(
    writer_state: &Mutex<Option<PendingWriterLock>>,
    request: &BaseMergeCleanupRequest,
    retain: bool,
    operation: impl FnOnce() -> Result<T, E>,
    map_error: impl Fn(String) -> E,
) -> Result<T, E> {
    let mut pending = writer_state
        .lock()
        .map_err(|_| map_error("active track writer transaction state is poisoned".to_owned()))?;
    if let Some(active) = pending.as_ref() {
        if active.request != *request {
            return Err(map_error(
                "request does not match the pending active track transaction".to_owned(),
            ));
        }
        if retain {
            return Err(map_error(
                "another active track cleanup transaction is still pending".to_owned(),
            ));
        }
        let active = pending.take().ok_or_else(|| {
            map_error("pending active track writer transaction disappeared".to_owned())
        })?;
        let result = operation();
        drop(active);
        return result;
    }

    let (track_dir, items_dir) = track_writer_paths(request);
    reserve_writer_key(&track_dir).map_err(&map_error)?;
    let file = match acquire_track_writer_lock(&track_dir, &items_dir) {
        Ok(file) => file,
        Err(error) => {
            release_writer_key(&track_dir);
            return Err(map_error(format!("cannot acquire active track writer lock: {error}")));
        }
    };
    if retain {
        *pending = Some(PendingWriterLock { request: request.clone(), file });
        let _writer_lock = pending.as_ref().map(|active| &active.file);
        let result = operation();
        if result.is_err() {
            pending.take();
        }
        result
    } else {
        let result = operation();
        drop(file);
        release_writer_key(&track_dir);
        result
    }
}

pub(super) fn generated_baseline_file_names(
    workspace_root: &Path,
) -> Result<BTreeSet<String>, String> {
    let bindings = FsTdddLayerBindingsAdapter::new()
        .load(workspace_root, None)
        .map_err(|error| format!("cannot load TDDD layer bindings: {error}"))?;
    let mut generated = BTreeSet::from([TDDD_FEATURES_BASELINE_FILE.to_owned()]);
    for baseline_file in bindings.into_iter().map(|binding| binding.baseline_file) {
        let path = Path::new(&baseline_file);
        if path.components().count() != 1
            || path.file_name().and_then(|name| name.to_str()) != Some(baseline_file.as_str())
        {
            return Err(format!(
                "resolved TDDD baseline filename is not a safe root file: {baseline_file}"
            ));
        }
        generated.insert(baseline_file);
    }
    Ok(generated)
}

pub(super) fn acquire_track_writer_lock(
    track_dir: &Path,
    items_dir: &Path,
) -> Result<fs::File, String> {
    reject_symlinks_up_to_root(items_dir)
        .map_err(|error| format!("cannot inspect track writer lock root: {error}"))?;
    reject_symlinks_below(track_dir, items_dir)
        .map_err(|error| format!("cannot inspect active track for writer lock: {error}"))?;
    if !track_dir.is_dir() {
        return Err("active track directory is unavailable".to_owned());
    }
    let lock_path = track_dir.join(TRACK_WRITER_LOCK_FILE);
    reject_symlinks_below(&lock_path, items_dir)
        .map_err(|error| format!("cannot inspect track writer lock: {error}"))?;
    let lock_file = super::open_base_merge_lock_file(&lock_path)
        .map_err(|error| format!("cannot open track writer lock: {error}"))?;
    lock_file
        .try_lock_exclusive()
        .map_err(|error| format!("another track writer holds the lock: {error}"))?;
    Ok(lock_file)
}

pub(super) fn publish_baseline_replacements(
    track_dir: &Path,
    replacement: &Path,
    generated_baseline_files: &BTreeSet<String>,
    exchanged: &mut bool,
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
    link_writer_lock_into_replacement(track_dir, replacement, track_parent, replacement_parent)
        .map_err(|error| {
            BaselineReplacementError::Publish(DiagnosticText::new(format!(
                "cannot carry active track writer lock into replacement: {error}"
            )))
        })?;
    sync_tree(replacement, replacement_parent).map_err(|error| {
        BaselineReplacementError::Publish(DiagnosticText::new(format!(
            "cannot make replacement writer lock durable: {error}"
        )))
    })?;
    verify_non_baseline_content_matches(
        track_dir,
        &active_parent,
        replacement,
        &recovery_parent_file,
        generated_baseline_files,
    )
    .map_err(|error| {
        BaselineReplacementError::Publish(DiagnosticText::new(format!(
            "cannot publish baseline replacement after active track drift: {error}"
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
    *exchanged = true;
    if let Err(error) = verify_non_baseline_content_matches(
        track_dir,
        &active_parent,
        replacement,
        &recovery_parent_file,
        generated_baseline_files,
    ) {
        return retain_after_baseline_exchange(
            track_dir,
            &active_parent,
            &recovery_parent_file,
            replacement,
            track_parent,
            replacement_parent,
            DiagnosticText::new(format!(
                "active track changed during baseline publication: {error}"
            )),
        );
    }

    // Persist the destination first, then the source, so a crash cannot lose
    // the recovery slot after the source entry has been removed.
    if let Err(error) = recovery_parent_file.sync_all() {
        return retain_after_baseline_exchange(
            track_dir,
            &active_parent,
            &recovery_parent_file,
            replacement,
            track_parent,
            replacement_parent,
            DiagnosticText::new(format!(
                "published baseline replacement but cannot persist recovery directory: {error}"
            )),
        );
    }
    if let Err(error) = active_parent.sync_all() {
        return retain_after_baseline_exchange(
            track_dir,
            &active_parent,
            &recovery_parent_file,
            replacement,
            track_parent,
            replacement_parent,
            DiagnosticText::new(format!(
                "published baseline replacement but cannot persist active directory: {error}"
            )),
        );
    }

    // The exchange is durable before the phase marker is cleared. If the
    // marker cleanup itself fails, retain both trees; if a crash occurs in
    // this window, restart sees the marker plus the deterministic recovery
    // copy and completes the transaction.
    let active_phase_marker = track_dir.join(BASELINE_REPLACEMENT_PHASE_MARKER);
    if let Err(error) = fs::remove_file(&active_phase_marker) {
        return retain_after_baseline_exchange(
            track_dir,
            &active_parent,
            &recovery_parent_file,
            replacement,
            track_parent,
            replacement_parent,
            DiagnosticText::new(format!(
                "published baseline replacement but cannot clear phase marker: {error}"
            )),
        );
    }
    if let Err(error) = fs::File::open(track_dir).and_then(|directory| directory.sync_all()) {
        return retain_after_baseline_exchange(
            track_dir,
            &active_parent,
            &recovery_parent_file,
            replacement,
            track_parent,
            replacement_parent,
            DiagnosticText::new(format!(
                "published baseline replacement but cannot persist phase marker removal: {error}"
            )),
        );
    }
    if let Err(error) = active_parent.sync_all() {
        return retain_after_baseline_exchange(
            track_dir,
            &active_parent,
            &recovery_parent_file,
            replacement,
            track_parent,
            replacement_parent,
            DiagnosticText::new(format!(
                "published baseline replacement but cannot persist phase marker removal in parent: {error}"
            )),
        );
    }
    Ok(())
}

fn retain_after_baseline_exchange(
    active_track: &Path,
    active_parent: &fs::File,
    recovery_parent: &fs::File,
    replacement: &Path,
    active_trusted_root: &Path,
    replacement_trusted_root: &Path,
    publish: DiagnosticText,
) -> Result<(), BaselineReplacementError> {
    // The exchange has already made both complete trees reachable. Never
    // exchange them back after this point: a writer may have changed either
    // tree through a path or descriptor that was valid before the exchange.
    // Retain both paths so the next guarded run can reconcile the pending
    // recovery copy without deleting either writer's update.
    let retention = sync_tree(active_track, active_trusted_root)
        .and_then(|()| sync_tree(replacement, replacement_trusted_root))
        .and_then(|()| {
            active_parent
                .sync_all()
                .map_err(|error| format!("cannot persist active directory: {error}"))
        })
        .and_then(|()| {
            recovery_parent
                .sync_all()
                .map_err(|error| format!("cannot persist recovery directory: {error}"))
        });
    match retention {
        Ok(()) => Err(BaselineReplacementError::Publish(DiagnosticText::new(format!(
            "{publish}; active and prior track trees retained for recovery"
        )))),
        Err(retention) => Err(BaselineReplacementError::Restoration {
            publish,
            restoration: DiagnosticText::new(format!(
                "cannot durably retain active and prior track trees after publication failure: {retention}"
            )),
        }),
    }
}

fn link_writer_lock_into_replacement(
    track_dir: &Path,
    replacement: &Path,
    track_parent: &Path,
    replacement_parent: &Path,
) -> Result<(), String> {
    let active_lock = track_dir.join(TRACK_WRITER_LOCK_FILE);
    let replacement_lock = replacement.join(TRACK_WRITER_LOCK_FILE);
    reject_symlinks_below(&active_lock, track_parent)
        .map_err(|error| format!("cannot inspect active track writer lock: {error}"))?;
    reject_symlinks_below(&replacement_lock, replacement_parent)
        .map_err(|error| format!("cannot inspect replacement writer lock: {error}"))?;
    match fs::symlink_metadata(&active_lock) {
        Ok(metadata) if metadata.is_file() && !metadata.file_type().is_symlink() => {}
        Ok(_) => return Err("active track writer lock is not a regular file".to_owned()),
        Err(error) => return Err(format!("cannot inspect active track writer lock: {error}")),
    }
    match fs::symlink_metadata(&replacement_lock) {
        Ok(_) => return Err("replacement already contains the reserved writer lock".to_owned()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(format!("cannot inspect replacement writer lock: {error}")),
    }
    fs::hard_link(&active_lock, &replacement_lock)
        .map_err(|error| format!("cannot link active track writer lock: {error}"))
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

struct ExternalRecoveryDirectory {
    path: PathBuf,
}

/// Completes an interrupted publication and promotes a durable recovery copy.
pub(super) fn reconcile_interrupted_replacement(
    replacement: &Path,
    recovery_slot: &Path,
    recovery_root: &Path,
    active_track: &Path,
    generated_baseline_files: &BTreeSet<String>,
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
    let active_parent = active_track.parent().ok_or_else(|| {
        format!("active track has no parent directory: {}", active_track.display())
    })?;
    reject_symlinks_up_to_root(active_parent)
        .map_err(|error| format!("cannot inspect active track parent directory: {error}"))?;
    reject_symlinks_below(active_track, active_parent)
        .map_err(|error| format!("cannot inspect active track: {error}"))?;
    let active_parent_file = open_directory_nofollow(active_parent)
        .map_err(|error| format!("cannot open active track parent directory: {error}"))?;
    let recovery_parent_file = open_directory_nofollow(recovery_root)
        .map_err(|error| format!("cannot open baseline recovery root: {error}"))?;
    verify_non_baseline_content_matches(
        active_track,
        &active_parent_file,
        replacement,
        &recovery_parent_file,
        generated_baseline_files,
    )
    .map_err(|error| format!("active track and retained replacement differ: {error}"))?;
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
        sync_tree(recovery_slot, recovery_root)?;
        verify_non_baseline_content_matches(
            replacement,
            &recovery_parent_file,
            recovery_slot,
            &recovery_parent_file,
            generated_baseline_files,
        )?;
        remove_tree_bounded(recovery_slot, recovery_root)
            .map_err(|error| format!("cannot clear prior baseline recovery slot: {error}"))?;
    }
    fs::rename(replacement, recovery_slot)
        .map_err(|error| format!("cannot promote interrupted baseline recovery slot: {error}"))?;
    sync_directory(recovery_root)
}

pub(super) fn promote_baseline_recovery_slot(
    replacement: &Path,
    recovery_slot: &Path,
    recovery_root: &Path,
) -> Result<(), BaselineReplacementError> {
    reject_symlinks_below(recovery_slot, recovery_root).map_err(|error| {
        BaselineReplacementError::Publish(DiagnosticText::new(format!(
            "cannot inspect prior baseline recovery slot before promotion: {error}"
        )))
    })?;
    // A canonical recovery slot may be retained after a later cleanup stage
    // fails. Stage that older copy before promoting the current replacement;
    // it predates this run and therefore need not match its telemetry or
    // other non-baseline content. The active-track drift checks in
    // `publish_baseline_replacements` still guard this publication window.
    let staged_recovery = stage_recovery_copy_for_sync(recovery_slot, recovery_root, recovery_root)
        .map_err(|error| {
            BaselineReplacementError::Publish(DiagnosticText::new(format!(
                "cannot stage retained baseline recovery slot: {error}"
            )))
        })?;
    fs::rename(replacement, recovery_slot).map_err(|error| {
        BaselineReplacementError::Publish(DiagnosticText::new(format!(
            "cannot promote baseline recovery slot: {error}"
        )))
    })?;
    fs::File::open(recovery_root).and_then(|directory| directory.sync_all()).map_err(|error| {
        BaselineReplacementError::Publish(DiagnosticText::new(format!(
            "cannot persist promoted baseline recovery slot: {error}"
        )))
    })?;
    if let Some(staged_recovery) = staged_recovery {
        staged_recovery.cleanup().map_err(|error| {
            BaselineReplacementError::Publish(DiagnosticText::new(format!(
                "cannot clear superseded baseline recovery slot: {error}"
            )))
        })?;
    }
    Ok(())
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
        sync_tree(&self._temporary_directory.path, parent)?;
        remove_tree_bounded(&self._temporary_directory.path, parent)?;
        sync_directory(parent)
    }
}

/// Moves the recovery slot to reversible staging before SyncBase publication.
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

/// Validates the bounded recovery cleanup anchor.
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

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

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

    #[test]
    fn test_reconcile_interrupted_replacement_preserves_divergent_recovery_copy() {
        let fixture = tempfile::tempdir().unwrap();
        let recovery_root = fixture.path().join("recovery");
        let replacement = recovery_root.join(".replacement");
        let recovery_slot = recovery_root.join("cleanup-test");
        let active_track = fixture.path().join("active");
        std::fs::create_dir_all(&replacement).unwrap();
        std::fs::create_dir_all(&recovery_slot).unwrap();
        std::fs::create_dir_all(&active_track).unwrap();
        std::fs::write(replacement.join("metadata.json"), "same metadata").unwrap();
        std::fs::write(recovery_slot.join("metadata.json"), "same metadata").unwrap();
        std::fs::write(active_track.join("metadata.json"), "same metadata").unwrap();
        std::fs::write(replacement.join("preserved-input.txt"), "pending").unwrap();
        std::fs::write(recovery_slot.join("preserved-input.txt"), "concurrent").unwrap();
        std::fs::write(active_track.join("preserved-input.txt"), "pending").unwrap();

        let result = reconcile_interrupted_replacement(
            &replacement,
            &recovery_slot,
            &recovery_root,
            &active_track,
            &BTreeSet::new(),
        );

        assert!(result.is_err());
        assert_eq!(
            std::fs::read_to_string(replacement.join("preserved-input.txt")).unwrap(),
            "pending"
        );
        assert_eq!(
            std::fs::read_to_string(recovery_slot.join("preserved-input.txt")).unwrap(),
            "concurrent"
        );
    }

    #[test]
    fn test_reconcile_interrupted_replacement_preserves_post_exchange_replacement() {
        let fixture = tempfile::tempdir().unwrap();
        let recovery_root = fixture.path().join("recovery");
        let replacement = recovery_root.join(".replacement");
        let recovery_slot = recovery_root.join("cleanup-test");
        let active_track = fixture.path().join("active");
        std::fs::create_dir_all(&replacement).unwrap();
        std::fs::create_dir_all(&active_track).unwrap();
        std::fs::write(replacement.join("metadata.json"), "retained metadata").unwrap();
        std::fs::write(replacement.join("preserved-input.txt"), "concurrent").unwrap();
        std::fs::write(active_track.join("metadata.json"), "active metadata").unwrap();
        std::fs::write(active_track.join("preserved-input.txt"), "active").unwrap();

        let result = reconcile_interrupted_replacement(
            &replacement,
            &recovery_slot,
            &recovery_root,
            &active_track,
            &BTreeSet::new(),
        );

        assert!(result.is_err());
        assert!(replacement.is_dir());
        assert!(!recovery_slot.exists());
        assert_eq!(
            std::fs::read_to_string(replacement.join("preserved-input.txt")).unwrap(),
            "concurrent"
        );
    }

    #[test]
    fn test_reconcile_interrupted_replacement_promotes_equal_active_content() {
        let fixture = tempfile::tempdir().unwrap();
        let recovery_root = fixture.path().join("recovery");
        let replacement = recovery_root.join(".replacement");
        let recovery_slot = recovery_root.join("cleanup-test");
        let active_track = fixture.path().join("active");
        std::fs::create_dir_all(&replacement).unwrap();
        std::fs::create_dir_all(&active_track).unwrap();
        for (root, content) in [(&replacement, "same"), (&active_track, "same")] {
            std::fs::write(root.join("metadata.json"), "same metadata").unwrap();
            std::fs::write(root.join("preserved-input.txt"), content).unwrap();
        }
        std::fs::write(active_track.join(BASELINE_REPLACEMENT_PHASE_MARKER), "prepared\n").unwrap();

        reconcile_interrupted_replacement(
            &replacement,
            &recovery_slot,
            &recovery_root,
            &active_track,
            &BTreeSet::new(),
        )
        .unwrap();

        assert!(!replacement.exists());
        assert_eq!(
            std::fs::read_to_string(recovery_slot.join("preserved-input.txt")).unwrap(),
            "same"
        );
    }

    #[test]
    fn test_promote_baseline_recovery_slot_accepts_matching_non_baseline_content() {
        let fixture = tempfile::tempdir().unwrap();
        let recovery_root = fixture.path().join("recovery");
        let replacement = recovery_root.join(".replacement");
        let recovery_slot = recovery_root.join("cleanup-test");
        std::fs::create_dir_all(&replacement).unwrap();
        std::fs::create_dir_all(&recovery_slot).unwrap();
        std::fs::write(replacement.join("metadata.json"), "same metadata").unwrap();
        std::fs::write(recovery_slot.join("metadata.json"), "same metadata").unwrap();
        std::fs::write(replacement.join("preserved-input.txt"), "same input").unwrap();
        std::fs::write(recovery_slot.join("preserved-input.txt"), "same input").unwrap();
        std::fs::write(replacement.join("domain-types-baseline.json"), "new baseline").unwrap();
        std::fs::write(recovery_slot.join("domain-types-baseline.json"), "old baseline").unwrap();
        let result = promote_baseline_recovery_slot(&replacement, &recovery_slot, &recovery_root);

        assert!(result.is_ok());
        assert!(!replacement.exists());
        assert_eq!(
            std::fs::read_to_string(recovery_slot.join("preserved-input.txt")).unwrap(),
            "same input"
        );
        assert_eq!(
            std::fs::read_to_string(recovery_slot.join("domain-types-baseline.json")).unwrap(),
            "new baseline"
        );
    }
}
