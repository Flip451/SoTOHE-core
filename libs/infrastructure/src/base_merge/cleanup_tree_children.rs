use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use super::super::TRACK_WRITER_LOCK_FILE;
use super::{CleanupTraversalBudget, copy_tree_at_depth};
use crate::track::symlink_guard::reject_symlinks_below;

/// Copies a track collection with one shared budget and per-child baseline scope.
pub(in crate::base_merge) fn copy_tree_children_with_budget(
    source: &Path,
    target: &Path,
    active_child: Option<&str>,
    trusted_target_root: &Path,
    budget: &mut CleanupTraversalBudget,
    generated_baseline_files: &BTreeSet<String>,
) -> Result<(), String> {
    budget.inspect_entry(source, 0)?;
    let metadata = fs::symlink_metadata(source)
        .map_err(|error| format!("cannot inspect {}: {error}", source.display()))?;
    if metadata.file_type().is_symlink() {
        return Err(format!("refusing symlinked cleanup input: {}", source.display()));
    }
    reject_symlinks_below(target, trusted_target_root)
        .map_err(|error| format!("cannot write through symlinked cleanup destination: {error}"))?;
    if !metadata.is_dir() {
        return copy_tree_at_depth(
            source,
            target,
            false,
            trusted_target_root,
            budget,
            generated_baseline_files,
            0,
            true,
            true,
        );
    }

    fs::create_dir_all(target)
        .map_err(|error| format!("cannot create {}: {error}", target.display()))?;
    for entry in fs::read_dir(source)
        .map_err(|error| format!("cannot enumerate {}: {error}", source.display()))?
    {
        let entry = entry.map_err(|error| format!("cannot enumerate cleanup input: {error}"))?;
        let path = entry.path();
        budget.inspect_entry(&path, 1)?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name == TRACK_WRITER_LOCK_FILE {
            continue;
        }
        let include_baselines = active_child.is_some_and(|active| active == name.as_ref());
        if !include_baselines && generated_baseline_files.contains(name.as_ref()) {
            continue;
        }
        copy_tree_at_depth(
            &path,
            &target.join(name.as_ref()),
            include_baselines,
            trusted_target_root,
            budget,
            generated_baseline_files,
            1,
            true,
            true,
        )?;
    }
    fs::File::open(target)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| format!("cannot sync staged directory {}: {error}", target.display()))
}
