use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{SystemTime, UNIX_EPOCH};

use domain::CommitHash;
use usecase::base_merge::BaseMergeCleanupRequest;

use super::cleanup_tree::{
    CleanupTraversalBudget, copy_tree_children_with_budget, copy_tree_with_baselines,
    remove_tree_bounded,
};
use super::publication::{
    generated_baseline_file_names, path_exists, publish_baseline_replacements, sync_directory,
    write_replacement_phase_marker,
};
use super::view_transaction::{
    publish_registry_if_unchanged, read_optional_rendered_file,
    reconcile_baseline_publication_before_views, restore_rendered_registry_if_unchanged,
    restore_view_exchange, rollback_if_published, snapshot_rendered_views,
    validate_optional_file_unchanged, validate_rendered_view_snapshot,
};
use super::{MAX_BASE_MERGE_GIT_OUTPUT_BYTES, MAX_CLEANUP_FILE_BYTES, MAX_CLEANUP_TREE_ENTRIES};
use crate::git_cli::{
    collect_bounded_git_output, guarded_git_command, spawn_bounded_git_child,
    without_history_rewrites, without_repository_selection,
};
use crate::track::atomic_write::atomic_write_file;
use crate::track::render::sync_rendered_views;
use crate::track::symlink_guard::reject_symlinks_below;
use crate::verify::tddd_layers::load_tddd_layers_from_workspace;

const VIEW_TRANSACTION_PHASE: &str = ".sotp-view-transaction-phase";
const VIEW_TRANSACTION_STAGING: &str = "staging";
const VIEW_TRANSACTION_REPLACEMENT: &str = "replacement";
const VIEW_TRANSACTION_REGISTRY_PREVIOUS: &str = "registry.previous";
const VIEW_TRANSACTION_REGISTRY_ABSENT: &str = "registry.previous.absent";
const VIEW_TRANSACTION_REGISTRY_NEXT: &str = "registry.next";
pub(super) const VIEW_TRANSACTION_PHASE_PREPARED: &str = "prepared";
pub(super) const VIEW_TRANSACTION_PHASE_TRACK_PUBLISHED: &str = "track-published";
pub(super) const VIEW_TRANSACTION_PHASE_ROLLBACK: &str = "rollback";
pub(super) const VIEW_TRANSACTION_PHASE_ROLLBACK_EXCHANGED: &str = "rollback-exchanged";
pub(super) const VIEW_TRANSACTION_PHASE_COMPLETE: &str = "complete";
pub(super) const VIEW_TRANSACTION_PHASE_ROLLED_BACK: &str = "rolled-back";

/// Render views away from the live track, then publish the complete rendered
/// track with the same atomic exchange used by baseline replacement.
pub(super) fn regenerate_views_transactionally(
    request: &BaseMergeCleanupRequest,
) -> Result<(), String> {
    super::resolve_workspace_repository_root(&request.workspace_root).map_err(str::to_owned)?;
    let track_dir = request.workspace_root.join("track/items").join(request.track_id.as_ref());
    let items_dir = request.workspace_root.join("track/items");
    reject_symlinks_below(&track_dir, &items_dir)
        .map_err(|error| format!("cannot inspect active track for view publication: {error}"))?;
    let rendered_names = rendered_view_names(&request.workspace_root)?;
    let registry_path = request.workspace_root.join("track/registry.md");
    let recovery_root = request.workspace_root.join("track/.sotp-baseline-recovery");
    reject_symlinks_below(&recovery_root, &request.workspace_root)
        .map_err(|error| format!("cannot inspect rendered-view recovery root: {error}"))?;
    fs::create_dir_all(&recovery_root)
        .map_err(|error| format!("cannot create rendered-view recovery root: {error}"))?;
    let transaction = recovery_root.join(format!(".sotp-view-transaction-{}", request.track_id));
    reject_symlinks_below(&transaction, &recovery_root)
        .map_err(|error| format!("cannot inspect rendered-view transaction: {error}"))?;
    if path_exists(&transaction)? {
        recover_view_transaction(
            &transaction,
            &recovery_root,
            &request.workspace_root,
            &track_dir,
            &registry_path,
        )?;
    }
    reconcile_baseline_publication_before_views(
        &request.workspace_root,
        &track_dir,
        &recovery_root,
        request.track_id.as_ref(),
    )?;
    let prior_views =
        snapshot_rendered_views(&request.workspace_root, &track_dir, &rendered_names)?;
    let prior_registry = read_optional_rendered_file(
        &registry_path,
        &request.workspace_root,
        MAX_CLEANUP_FILE_BYTES,
    )?;
    fs::create_dir(&transaction)
        .map_err(|error| format!("cannot create rendered-view transaction: {error}"))?;
    sync_directory(&recovery_root)?;
    let staging = transaction.join(VIEW_TRANSACTION_STAGING);
    let replacement = transaction.join(VIEW_TRANSACTION_REPLACEMENT);
    fs::create_dir(&replacement)
        .map_err(|error| format!("cannot create rendered-view replacement: {error}"))?;
    let mut exchanged = false;
    let mut transaction_prepared = false;
    let mut staged_registry_for_rollback = None;
    let result: Result<(), String> = (|| {
        fs::create_dir(&staging)
            .map_err(|error| format!("cannot create rendered-view staging workspace: {error}"))?;
        prepare_render_workspace(&request.workspace_root, &staging, request.track_id.as_ref())?;
        sync_rendered_views(&staging, Some(request.track_id.as_ref()))
            .map_err(|error| format!("cannot render views in staging workspace: {error}"))?;
        let staged_registry = read_optional_rendered_file(
            &staging.join("track/registry.md"),
            &staging,
            MAX_CLEANUP_FILE_BYTES,
        )?
        .ok_or_else(|| "rendered staging did not produce track/registry.md".to_owned())?;
        staged_registry_for_rollback = Some(staged_registry.clone());
        validate_rendered_view_snapshot(
            &request.workspace_root,
            &track_dir,
            &rendered_names,
            &prior_views,
        )?;
        validate_optional_file_unchanged(
            &registry_path,
            &request.workspace_root,
            prior_registry.as_deref(),
        )?;
        copy_tree_with_baselines(
            &staging.join("track/items").join(request.track_id.as_ref()),
            &replacement,
            true,
            &recovery_root,
            &BTreeSet::new(),
        )?;
        write_replacement_phase_marker(&replacement)
            .map_err(|error| format!("cannot prepare rendered-view replacement: {error}"))?;
        remove_tree_bounded(&staging, &transaction)?;
        write_view_transaction_state(&transaction, prior_registry.as_deref(), &staged_registry)?;
        transaction_prepared = true;
        publish_baseline_replacements(&track_dir, &replacement, &rendered_names, &mut exchanged)
            .map_err(|error| error.to_string())?;
        // Rendered views are intentionally excluded from the generic baseline
        // publisher's drift comparison because this transaction replaces them.
        // After the exchange, the recovery path is the complete prior active
        // tree. Any rendered-view write that raced with the exchange remains
        // there, so detect it before advancing the transaction or publishing
        // the registry. A mismatch fails closed and rollback restores that
        // retained tree, preserving the concurrent writer's content.
        validate_rendered_view_snapshot(
            &recovery_root,
            &replacement,
            &rendered_names,
            &prior_views,
        )
        .map_err(|error| format!("rendered view changed during publication: {error}"))?;
        write_view_transaction_phase(&transaction, "track-published")?;
        publish_registry_if_unchanged(
            &registry_path,
            &request.workspace_root,
            prior_registry.as_deref(),
            &staged_registry,
        )?;
        clear_active_view_phase_marker(&track_dir)?;
        write_view_transaction_phase(&transaction, VIEW_TRANSACTION_PHASE_COMPLETE)?;
        let recovery_slot = recovery_root.join(request.track_id.as_ref());
        remove_tree_bounded(&recovery_slot, &recovery_root)
            .and_then(|()| sync_directory(&recovery_root))
            .map_err(|error| format!("cannot clear baseline recovery slot after views: {error}"))?;
        // The complete phase is durable before cleanup begins. Cleanup only
        // reclaims a recoverable transaction; it must never turn a published
        // view transaction back into a failed stage after its rollback tree
        // has been removed. A later guarded run can remove any retained
        // complete transaction before starting a new one.
        let _cleanup = remove_tree_bounded(&transaction, &recovery_root)
            .and_then(|()| sync_directory(&recovery_root));
        Ok(())
    })();
    let result = rollback_if_published(
        result,
        transaction_prepared,
        exchanged,
        &transaction,
        &request.workspace_root,
        &track_dir,
        &replacement,
        &registry_path,
        prior_registry.as_deref(),
        staged_registry_for_rollback.as_deref(),
    );
    if result.is_err() && !transaction_prepared {
        let cleanup = remove_tree_bounded(&transaction, &recovery_root);
        return match cleanup {
            Ok(()) => result,
            Err(cleanup) => result.map_err(|error| format!("{error}; {cleanup}")),
        };
    }
    result
}
fn write_view_transaction_state(
    transaction: &Path,
    prior_registry: Option<&[u8]>,
    next_registry: &[u8],
) -> Result<(), String> {
    let previous = transaction.join(VIEW_TRANSACTION_REGISTRY_PREVIOUS);
    let absent = transaction.join(VIEW_TRANSACTION_REGISTRY_ABSENT);
    match prior_registry {
        Some(bytes) => atomic_write_file(&previous, bytes),
        None => atomic_write_file(&absent, b"absent\n"),
    }
    .map_err(|error| format!("cannot persist prior rendered registry: {error}"))?;
    atomic_write_file(&transaction.join(VIEW_TRANSACTION_REGISTRY_NEXT), next_registry)
        .map_err(|error| format!("cannot persist next rendered registry: {error}"))?;
    write_view_transaction_phase(transaction, "prepared")
}
pub(super) fn write_view_transaction_phase(transaction: &Path, phase: &str) -> Result<(), String> {
    atomic_write_file(&transaction.join(VIEW_TRANSACTION_PHASE), format!("{phase}\n").as_bytes())
        .map_err(|error| format!("cannot persist rendered-view transaction phase: {error}"))?;
    sync_directory(transaction)
}
fn recover_view_transaction(
    transaction: &Path,
    recovery_root: &Path,
    workspace_root: &Path,
    track_dir: &Path,
    registry_path: &Path,
) -> Result<(), String> {
    reject_symlinks_below(transaction, recovery_root).map_err(|error| {
        format!("cannot inspect interrupted rendered-view transaction: {error}")
    })?;
    let phase = read_optional_rendered_file(
        &transaction.join(VIEW_TRANSACTION_PHASE),
        recovery_root,
        MAX_CLEANUP_FILE_BYTES,
    )?;
    let Some(phase) = phase else {
        remove_tree_bounded(transaction, recovery_root)?;
        return sync_directory(recovery_root);
    };
    let phase = std::str::from_utf8(&phase)
        .map_err(|error| format!("interrupted rendered-view phase is not UTF-8: {error}"))?
        .trim();
    if !matches!(
        phase,
        VIEW_TRANSACTION_PHASE_PREPARED
            | VIEW_TRANSACTION_PHASE_TRACK_PUBLISHED
            | VIEW_TRANSACTION_PHASE_ROLLBACK
            | VIEW_TRANSACTION_PHASE_ROLLBACK_EXCHANGED
            | VIEW_TRANSACTION_PHASE_COMPLETE
            | VIEW_TRANSACTION_PHASE_ROLLED_BACK
    ) {
        return Err(format!("unknown interrupted rendered-view phase: {phase}"));
    }
    if matches!(phase, VIEW_TRANSACTION_PHASE_COMPLETE | VIEW_TRANSACTION_PHASE_ROLLED_BACK) {
        remove_tree_bounded(transaction, recovery_root)?;
        return sync_directory(recovery_root);
    }
    let prior = read_transaction_previous_registry(transaction, recovery_root)?;
    let next = read_optional_rendered_file(
        &transaction.join(VIEW_TRANSACTION_REGISTRY_NEXT),
        recovery_root,
        MAX_CLEANUP_FILE_BYTES,
    )?
    .ok_or_else(|| "interrupted rendered-view transaction has no next registry".to_owned())?;
    let replacement_marker = transaction
        .join(VIEW_TRANSACTION_REPLACEMENT)
        .join(super::BASELINE_REPLACEMENT_PHASE_MARKER);
    reject_symlinks_below(&replacement_marker, recovery_root)
        .map_err(|error| format!("cannot inspect interrupted rendered-view marker: {error}"))?;
    let replacement_marker_present = path_exists(&replacement_marker)?;
    let current =
        read_optional_rendered_file(registry_path, workspace_root, MAX_CLEANUP_FILE_BYTES)?;
    // `prepared` is durable before the exchange and before the post-exchange
    // drift validation. If recovery sees the exchange already happened while
    // this phase is still recorded, the transaction cannot be treated as
    // committed: persist rollback intent and use the rollback path below.
    // This preserves the prior tree across the crash window between exchange
    // and the durable `track-published` phase.
    let phase = if phase == VIEW_TRANSACTION_PHASE_PREPARED && !replacement_marker_present {
        write_view_transaction_phase(transaction, VIEW_TRANSACTION_PHASE_ROLLBACK)?;
        VIEW_TRANSACTION_PHASE_ROLLBACK
    } else {
        phase
    };
    if matches!(phase, VIEW_TRANSACTION_PHASE_ROLLBACK | VIEW_TRANSACTION_PHASE_ROLLBACK_EXCHANGED)
    {
        if phase == VIEW_TRANSACTION_PHASE_ROLLBACK && !replacement_marker_present {
            if current.as_deref() != prior.as_deref() && current.as_deref() != Some(next.as_slice())
            {
                return Err("interrupted rendered-view rollback found concurrent registry content"
                    .to_owned());
            }
            restore_view_exchange(
                workspace_root,
                track_dir,
                &transaction.join(VIEW_TRANSACTION_REPLACEMENT),
            )?;
            write_view_transaction_phase(transaction, VIEW_TRANSACTION_PHASE_ROLLBACK_EXCHANGED)?;
        } else if !replacement_marker_present {
            return Err("interrupted rendered-view rollback lost its exchanged replacement marker"
                .to_owned());
        }
        restore_rendered_registry_if_unchanged(
            registry_path,
            workspace_root,
            prior.as_deref(),
            Some(next.as_slice()),
        )?;
        write_view_transaction_phase(transaction, VIEW_TRANSACTION_PHASE_ROLLED_BACK)?;
        remove_tree_bounded(transaction, recovery_root)?;
        return sync_directory(recovery_root);
    }
    let exchanged = !replacement_marker_present;
    if !exchanged {
        if current.as_deref() == prior.as_deref() {
            remove_tree_bounded(transaction, recovery_root)?;
            return sync_directory(recovery_root);
        }
        if current.as_deref() == Some(next.as_slice()) {
            return Err(
                "interrupted rendered-view transaction advanced registry before track publication"
                    .to_owned(),
            );
        }
        return Err(
            "interrupted rendered-view transaction found concurrent registry content".to_owned()
        );
    }
    if current.as_deref() != prior.as_deref() && current.as_deref() != Some(next.as_slice()) {
        return Err(
            "interrupted rendered-view transaction found concurrent registry content".to_owned()
        );
    }
    if current.as_deref() == prior.as_deref() {
        publish_registry_if_unchanged(registry_path, workspace_root, prior.as_deref(), &next)?;
    }
    clear_active_view_phase_marker(track_dir)?;
    write_view_transaction_phase(transaction, VIEW_TRANSACTION_PHASE_COMPLETE)?;
    remove_tree_bounded(transaction, recovery_root)?;
    sync_directory(recovery_root)
}
fn read_transaction_previous_registry(
    transaction: &Path,
    trusted_root: &Path,
) -> Result<Option<Vec<u8>>, String> {
    let previous = transaction.join(VIEW_TRANSACTION_REGISTRY_PREVIOUS);
    let absent = transaction.join(VIEW_TRANSACTION_REGISTRY_ABSENT);
    let has_previous = path_exists(&previous)?;
    let has_absent = path_exists(&absent)?;
    match (has_previous, has_absent) {
        (true, false) => {
            read_optional_rendered_file(&previous, trusted_root, MAX_CLEANUP_FILE_BYTES)
        }
        (false, true) => Ok(None),
        _ => {
            Err("interrupted rendered-view transaction has invalid prior registry state".to_owned())
        }
    }
}
fn clear_active_view_phase_marker(track_dir: &Path) -> Result<(), String> {
    let marker = track_dir.join(super::BASELINE_REPLACEMENT_PHASE_MARKER);
    reject_symlinks_below(&marker, track_dir)
        .map_err(|error| format!("cannot inspect active rendered-view phase marker: {error}"))?;
    if !path_exists(&marker)? {
        return Ok(());
    }
    let metadata = fs::symlink_metadata(&marker)
        .map_err(|error| format!("cannot inspect active rendered-view phase marker: {error}"))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err("active rendered-view phase marker is not a regular file".to_owned());
    }
    fs::remove_file(&marker)
        .map_err(|error| format!("cannot clear active rendered-view phase marker: {error}"))?;
    fs::File::open(track_dir)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| format!("cannot persist active rendered-view phase marker: {error}"))
}
fn rendered_view_names(workspace_root: &Path) -> Result<BTreeSet<String>, String> {
    let mut names =
        BTreeSet::from(["contract-map.md".to_owned(), "plan.md".to_owned(), "spec.md".to_owned()]);
    let bindings = load_tddd_layers_from_workspace(workspace_root)
        .map_err(|error| format!("cannot load rendered-view layer bindings: {error}"))?;
    for name in bindings.into_iter().map(|binding| binding.rendered_file()) {
        let path = Path::new(&name);
        if path.components().count() != 1
            || path.file_name().and_then(|file| file.to_str()) != Some(name.as_str())
        {
            return Err(format!("rendered view is not a safe track-root filename: {name}"));
        }
        names.insert(name);
    }
    if names.len() > MAX_CLEANUP_TREE_ENTRIES {
        return Err(format!("rendered-view set exceeds {MAX_CLEANUP_TREE_ENTRIES} entries"));
    }
    Ok(names)
}

fn prepare_render_workspace(
    workspace_root: &Path,
    staging: &Path,
    track_id: &str,
) -> Result<(), String> {
    reject_symlinks_below(staging, workspace_root)
        .map_err(|error| format!("cannot inspect rendered-view staging workspace: {error}"))?;
    let stage_track_root = staging.join("track");
    fs::create_dir_all(stage_track_root.join("items"))
        .map_err(|error| format!("cannot create rendered-view staging tree: {error}"))?;

    let rules = workspace_root.join("architecture-rules.json");
    let rules_content =
        super::read_regular_file_bounded(&rules, workspace_root, MAX_CLEANUP_FILE_BYTES)?;
    atomic_write_file(&staging.join("architecture-rules.json"), &rules_content)
        .map_err(|error| format!("cannot stage architecture-rules.json: {error}"))?;

    let style_path = workspace_root.join(".harness/config/contract-map-style.toml");
    if let Some(style) =
        read_optional_rendered_file(&style_path, workspace_root, MAX_CLEANUP_FILE_BYTES)?
    {
        let staged_style = staging.join(".harness/config/contract-map-style.toml");
        if let Some(parent) = staged_style.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("cannot create staged style-config directory: {error}"))?;
        }
        atomic_write_file(&staged_style, &style)
            .map_err(|error| format!("cannot stage contract-map style config: {error}"))?;
    }

    let generated_baseline_files = generated_baseline_file_names(workspace_root)?;
    let mut budget = CleanupTraversalBudget::new();
    let source_items = workspace_root.join("track/items");
    copy_tree_children_with_budget(
        &source_items,
        &stage_track_root.join("items"),
        Some(track_id),
        staging,
        &mut budget,
        &generated_baseline_files,
    )?;
    let source_archive = workspace_root.join("track/archive");
    match fs::symlink_metadata(&source_archive) {
        Ok(_) => copy_tree_children_with_budget(
            &source_archive,
            &stage_track_root.join("archive"),
            None,
            staging,
            &mut budget,
            &generated_baseline_files,
        )?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(format!("cannot inspect track archive for view staging: {error}"));
        }
    }
    let staged_track = stage_track_root.join("items").join(track_id);
    if !staged_track.is_dir() {
        return Err("active track is unavailable in rendered-view staging workspace".to_owned());
    }
    Ok(())
}

pub(super) fn create_unique_directory(parent: &Path, prefix: &str) -> std::io::Result<PathBuf> {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| std::io::Error::other(error.to_string()))?
        .as_nanos();
    for suffix in 0..100_u32 {
        let path = parent.join(format!("{prefix}{}-{suffix}", std::process::id() ^ stamp as u32));
        match fs::create_dir(&path) {
            Ok(()) => return Ok(path),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::AlreadyExists,
        "could not allocate a unique cleanup directory",
    ))
}

fn pin_current_repository_hooks(
    command: &mut std::process::Command,
    repository_root: &Path,
) -> Result<(), String> {
    let hooks_path = repository_root.join(".githooks");
    if !hooks_path.is_absolute() {
        return Err("repository root is not absolute".to_owned());
    }
    // Keep guard coverage on current shims; never execute historical worktree hooks.
    command.arg("-c").arg(format!("core.hooksPath={}", hooks_path.display()));
    Ok(())
}

pub(super) fn add_commit_pinned_worktree(
    repository_root: &Path,
    worktree: &Path,
    base_commit: &CommitHash,
) -> Result<(), String> {
    let mut command = guarded_git_command();
    pin_current_repository_hooks(&mut command, repository_root)?;
    command
        .args(["worktree", "add", "--detach", "--"])
        .arg(worktree)
        .arg(base_commit.as_ref())
        .current_dir(repository_root)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    without_repository_selection(&mut command);
    without_history_rewrites(&mut command);
    let output = collect_bounded_git_output(
        spawn_bounded_git_child(&mut command).map_err(|error| error.to_string())?,
        MAX_BASE_MERGE_GIT_OUTPUT_BYTES,
    )
    .map_err(|error| error.to_string())?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!("git worktree add failed: {}", String::from_utf8_lossy(&output.stderr).trim()))
    }
}

pub(super) fn remove_commit_pinned_worktree(
    repository_root: &Path,
    worktree: &Path,
) -> Result<(), String> {
    let mut command = guarded_git_command();
    pin_current_repository_hooks(&mut command, repository_root)?;
    command
        .args(["worktree", "remove", "--force", "--"])
        .arg(worktree)
        .current_dir(repository_root)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    without_repository_selection(&mut command);
    without_history_rewrites(&mut command);
    let output = collect_bounded_git_output(
        spawn_bounded_git_child(&mut command).map_err(|error| error.to_string())?,
        MAX_BASE_MERGE_GIT_OUTPUT_BYTES,
    )
    .map_err(|error| error.to_string())?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "git worktree remove failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_drift_is_preserved_before_publication() {
        let fixture = tempfile::tempdir().unwrap();
        let registry = fixture.path().join("registry.md");
        fs::write(&registry, b"concurrent\n").unwrap();

        let result = validate_optional_file_unchanged(&registry, fixture.path(), Some(b"prior\n"));

        assert!(result.unwrap_err().contains("changed while it was being staged"));
        assert_eq!(fs::read(&registry).unwrap(), b"concurrent\n");
    }

    #[test]
    fn test_post_exchange_rendered_view_drift_fails_closed() {
        let fixture = tempfile::tempdir().unwrap();
        let active = fixture.path().join("active");
        let replacement = fixture.path().join("replacement");
        fs::create_dir_all(&active).unwrap();
        fs::create_dir_all(&replacement).unwrap();
        let names = BTreeSet::from(["domain-types.md".to_owned()]);
        fs::write(active.join("domain-types.md"), b"prior\n").unwrap();
        fs::write(replacement.join("domain-types.md"), b"concurrent\n").unwrap();

        let prior = snapshot_rendered_views(fixture.path(), &active, &names).unwrap();
        let result = validate_rendered_view_snapshot(fixture.path(), &replacement, &names, &prior);

        assert!(result.unwrap_err().contains("changed while it was being staged"));
        assert_eq!(fs::read(replacement.join("domain-types.md")).unwrap(), b"concurrent\n");
    }

    #[test]
    fn test_recover_prepared_exchanged_view_transaction_rolls_back_before_publication() {
        let fixture = tempfile::tempdir().unwrap();
        let workspace_root = fixture.path();
        let track_dir = workspace_root.join("track/items/track-a");
        let recovery_root = workspace_root.join("track/.sotp-baseline-recovery");
        let transaction = recovery_root.join(".sotp-view-transaction-track-a");
        let replacement = transaction.join(VIEW_TRANSACTION_REPLACEMENT);
        let registry = workspace_root.join("track/registry.md");

        fs::create_dir_all(&track_dir).unwrap();
        fs::create_dir_all(&replacement).unwrap();
        fs::write(track_dir.join("domain-types.md"), b"staged\n").unwrap();
        fs::write(replacement.join("domain-types.md"), b"prior\n").unwrap();
        fs::write(transaction.join(VIEW_TRANSACTION_REGISTRY_PREVIOUS), b"prior registry\n")
            .unwrap();
        fs::write(transaction.join(VIEW_TRANSACTION_REGISTRY_NEXT), b"staged registry\n").unwrap();
        fs::write(transaction.join(VIEW_TRANSACTION_PHASE), b"prepared\n").unwrap();
        fs::write(&registry, b"staged registry\n").unwrap();

        recover_view_transaction(
            &transaction,
            &recovery_root,
            workspace_root,
            &track_dir,
            &registry,
        )
        .unwrap();

        assert_eq!(fs::read(track_dir.join("domain-types.md")).unwrap(), b"prior\n");
        assert_eq!(fs::read(&registry).unwrap(), b"prior registry\n");
        assert!(!transaction.exists());
    }
}
