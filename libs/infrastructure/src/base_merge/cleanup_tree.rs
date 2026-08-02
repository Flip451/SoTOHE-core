//! Bounded, symlink-safe staging of track data for base-merge cleanup.

use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use super::{
    MAX_CLEANUP_FILE_BYTES, MAX_CLEANUP_TREE_BYTES, MAX_CLEANUP_TREE_DEPTH,
    MAX_CLEANUP_TREE_ENTRIES, read_regular_file_bounded,
};
use crate::FsSymlinkGuard;
use crate::tddd::baseline_rustdoc_codec::BaselineRustdocCodec;
use crate::tddd::feature_declaration_adapter::FsTdddFeatureDeclarationAdapter;
use crate::tddd::rustdoc_baseline_capture_adapter::RustdocBaselineCaptureAdapter;
use crate::tddd::tddd_layer_bindings_adapter::FsTdddLayerBindingsAdapter;
use crate::track::atomic_write::atomic_write_file;
use crate::track::symlink_guard::{reject_symlinks_below, reject_symlinks_up_to_root};
use domain::TrackId;
use domain::tddd::catalogue_v2::TdddLayerBindingsPort;
use usecase::base_merge::BaselineReplacementError;
use usecase::baseline_capture::{
    BaselineCaptureInteractor, BaselineCaptureRequest, BaselineCaptureService,
};
use usecase::git_workflow::DiagnosticText;

pub(super) fn copy_cleanup_inputs(
    source_workspace: &Path,
    target_workspace: &Path,
    track_id: &str,
) -> Result<(), String> {
    reject_symlinks_up_to_root(target_workspace)
        .map_err(|error| format!("cannot inspect detached cleanup workspace: {error}"))?;
    let rules = source_workspace.join("architecture-rules.json");
    if !reject_symlinks_below(&rules, source_workspace)
        .map_err(|error| format!("cannot inspect architecture-rules.json: {error}"))?
    {
        return Err("architecture-rules.json is unavailable".to_owned());
    }
    let target_rules = target_workspace.join("architecture-rules.json");
    reject_symlinks_below(&target_rules, target_workspace)
        .map_err(|error| format!("cannot inspect detached architecture-rules.json: {error}"))?;
    let rules_content =
        read_regular_file_bounded(&rules, source_workspace, MAX_CLEANUP_FILE_BYTES)?;
    atomic_write_file(&target_rules, &rules_content)
        .map_err(|error| format!("cannot copy architecture-rules.json: {error}"))?;

    let source_track = source_workspace.join("track/items").join(track_id);
    let target_track = target_workspace.join("track/items").join(track_id);
    replace_tree(&source_track, &target_track, false, target_workspace)?;
    remove_baseline_files(&target_track)
}

pub(super) fn replace_tree(
    source: &Path,
    target: &Path,
    include_baselines: bool,
    trusted_target_root: &Path,
) -> Result<(), String> {
    remove_tree_bounded(target, trusted_target_root)?;
    copy_tree(source, target, include_baselines, trusted_target_root)
}

/// Removes a tree without following symlinks or traversing beyond bounded
/// depth/entry limits. Missing paths are already in the desired state.
pub(super) fn remove_tree_bounded(path: &Path, trusted_root: &Path) -> Result<(), String> {
    let mut budget = CleanupTraversalBudget::new();
    remove_tree_at_depth(path, trusted_root, &mut budget, 0)
}

fn remove_tree_at_depth(
    path: &Path,
    trusted_root: &Path,
    budget: &mut CleanupTraversalBudget,
    depth: usize,
) -> Result<(), String> {
    budget.inspect_entry(path, depth)?;
    reject_symlinks_below(path, trusted_root)
        .map_err(|error| format!("cannot inspect removal target {}: {error}", path.display()))?;
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(format!("cannot inspect {}: {error}", path.display())),
    };
    if metadata.file_type().is_symlink() || metadata.is_file() {
        fs::remove_file(path)
            .map_err(|error| format!("cannot remove {}: {error}", path.display()))?;
        return Ok(());
    }
    if !metadata.is_dir() {
        return Err(format!("refusing non-regular removal target: {}", path.display()));
    }
    for entry in fs::read_dir(path)
        .map_err(|error| format!("cannot enumerate removal target {}: {error}", path.display()))?
    {
        let entry = entry.map_err(|error| format!("cannot enumerate removal target: {error}"))?;
        remove_tree_at_depth(&entry.path(), trusted_root, budget, depth + 1)?;
    }
    fs::remove_dir(path).map_err(|error| format!("cannot remove {}: {error}", path.display()))
}

#[derive(Debug)]
pub(super) struct CleanupTraversalBudget {
    entries_remaining: usize,
    bytes_remaining: u64,
}

impl CleanupTraversalBudget {
    pub(super) const fn new() -> Self {
        Self {
            entries_remaining: MAX_CLEANUP_TREE_ENTRIES,
            bytes_remaining: MAX_CLEANUP_TREE_BYTES,
        }
    }

    pub(super) fn inspect_entry(&mut self, path: &Path, depth: usize) -> Result<(), String> {
        if depth > MAX_CLEANUP_TREE_DEPTH {
            return Err(format!(
                "cleanup input exceeds maximum directory depth at {}",
                path.display()
            ));
        }
        self.entries_remaining = self.entries_remaining.checked_sub(1).ok_or_else(|| {
            format!("cleanup input exceeds {MAX_CLEANUP_TREE_ENTRIES} filesystem entries")
        })?;
        Ok(())
    }

    fn file_limit(&self, path: &Path, declared_length: u64) -> Result<u64, String> {
        if declared_length > MAX_CLEANUP_FILE_BYTES {
            return Err(format!("cleanup input file exceeds byte limit: {}", path.display()));
        }
        if declared_length > self.bytes_remaining {
            return Err(format!("cleanup input exceeds total byte limit at {}", path.display()));
        }
        Ok(self.bytes_remaining.min(MAX_CLEANUP_FILE_BYTES))
    }

    fn consume_file(&mut self, path: &Path, copied: u64) -> Result<(), String> {
        self.bytes_remaining = self.bytes_remaining.checked_sub(copied).ok_or_else(|| {
            format!("cleanup input exceeds total byte limit at {}", path.display())
        })?;
        Ok(())
    }
}

pub(super) fn copy_tree(
    source: &Path,
    target: &Path,
    include_baselines: bool,
    trusted_target_root: &Path,
) -> Result<(), String> {
    let mut budget = CleanupTraversalBudget::new();
    copy_tree_with_budget(source, target, include_baselines, trusted_target_root, &mut budget)
}

fn copy_tree_with_budget(
    source: &Path,
    target: &Path,
    include_baselines: bool,
    trusted_target_root: &Path,
    budget: &mut CleanupTraversalBudget,
) -> Result<(), String> {
    copy_tree_at_depth(source, target, include_baselines, trusted_target_root, budget, 0, false)
}

fn copy_tree_at_depth(
    source: &Path,
    target: &Path,
    include_baselines: bool,
    trusted_target_root: &Path,
    budget: &mut CleanupTraversalBudget,
    depth: usize,
    already_counted: bool,
) -> Result<(), String> {
    if !already_counted {
        budget.inspect_entry(source, depth)?;
    }
    let metadata = fs::symlink_metadata(source)
        .map_err(|error| format!("cannot inspect {}: {error}", source.display()))?;
    if metadata.file_type().is_symlink() {
        return Err(format!("refusing symlinked cleanup input: {}", source.display()));
    }
    reject_symlinks_below(target, trusted_target_root)
        .map_err(|error| format!("cannot write through symlinked cleanup destination: {error}"))?;
    if metadata.is_dir() {
        fs::create_dir_all(target)
            .map_err(|error| format!("cannot create {}: {error}", target.display()))?;
        for entry in fs::read_dir(source)
            .map_err(|error| format!("cannot enumerate {}: {error}", source.display()))?
        {
            let entry =
                entry.map_err(|error| format!("cannot enumerate cleanup input: {error}"))?;
            let path = entry.path();
            budget.inspect_entry(&path, depth + 1)?;
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if !include_baselines && name.ends_with("-types-baseline.json") {
                continue;
            }
            copy_tree_at_depth(
                &path,
                &target.join(name.as_ref()),
                include_baselines,
                trusted_target_root,
                budget,
                depth + 1,
                true,
            )?;
        }
        fs::File::open(target).and_then(|directory| directory.sync_all()).map_err(|error| {
            format!("cannot sync staged directory {}: {error}", target.display())
        })?;
        return Ok(());
    }
    if !metadata.is_file() {
        return Err(format!("refusing non-regular cleanup input: {}", source.display()));
    }
    copy_regular_file_bounded(source, target, budget.file_limit(source, metadata.len())?, budget)
}

fn copy_regular_file_bounded(
    source: &Path,
    target: &Path,
    limit: u64,
    budget: &mut CleanupTraversalBudget,
) -> Result<(), String> {
    let mut input = fs::File::open(source)
        .map_err(|error| format!("cannot open cleanup input {}: {error}", source.display()))?;
    let opened_metadata = input.metadata().map_err(|error| {
        format!("cannot inspect opened cleanup input {}: {error}", source.display())
    })?;
    if !opened_metadata.is_file() {
        return Err(format!("refusing non-regular cleanup input: {}", source.display()));
    }
    let mut output =
        fs::OpenOptions::new().write(true).create_new(true).open(target).map_err(|error| {
            format!("cannot create staged cleanup file {}: {error}", target.display())
        })?;
    let copied =
        std::io::copy(&mut Read::by_ref(&mut input).take(limit.saturating_add(1)), &mut output)
            .map_err(|error| format!("cannot copy cleanup input {}: {error}", source.display()))?;
    if copied > limit {
        return Err(format!("cleanup input exceeds total byte limit at {}", source.display()));
    }
    output.flush().and_then(|()| output.sync_all()).map_err(|error| {
        format!("cannot sync staged cleanup file {}: {error}", target.display())
    })?;
    budget.consume_file(source, copied)
}

fn remove_baseline_files(track_dir: &Path) -> Result<(), String> {
    for path in baseline_files_below(track_dir)? {
        fs::remove_file(&path)
            .map_err(|error| format!("cannot clear stale baseline {}: {error}", path.display()))?;
    }
    Ok(())
}

pub(super) fn capture_baselines_in_worktree(
    worktree: &Path,
    track_id: &str,
) -> Result<(), BaselineReplacementError> {
    let interactor = BaselineCaptureInteractor::new(
        std::sync::Arc::new(FsSymlinkGuard::new()),
        std::sync::Arc::new(FsTdddLayerBindingsAdapter::new()),
        std::sync::Arc::new(RustdocBaselineCaptureAdapter::new()),
        std::sync::Arc::new(FsTdddFeatureDeclarationAdapter::new()),
    );
    interactor
        .run(BaselineCaptureRequest {
            track_id: track_id.to_owned(),
            workspace_root: worktree.to_path_buf(),
            source_workspace: None,
            layer: None,
        })
        .map_err(|error| {
            BaselineReplacementError::Generation(DiagnosticText::new(error.to_string()))
        })
}

pub(super) fn collect_validated_baselines(worktree: &Path, track_id: &str) -> Result<(), String> {
    let track_id = TrackId::try_new(track_id.to_owned())
        .map_err(|error| format!("invalid track id in baseline cleanup: {error}"))?;
    let bindings = FsTdddLayerBindingsAdapter::new()
        .load(worktree, None)
        .map_err(|error| format!("cannot load TDDD layer bindings: {error}"))?;
    let source_track = worktree.join("track/items").join(track_id.as_ref());
    for binding in bindings {
        let source = source_track.join(&binding.baseline_file);
        let bytes = read_regular_file_bounded(&source, worktree, MAX_CLEANUP_FILE_BYTES)?;
        let text = std::str::from_utf8(&bytes).map_err(|error| {
            format!("generated baseline {} is not UTF-8: {error}", source.display())
        })?;
        BaselineRustdocCodec::from_json(text).map_err(|error| {
            format!("generated baseline {} failed validation: {error}", source.display())
        })?;
    }
    Ok(())
}

pub(super) fn sync_tree(path: &Path, trusted_root: &Path) -> Result<(), String> {
    let mut budget = CleanupTraversalBudget::new();
    sync_tree_at_depth(path, trusted_root, &mut budget, 0)
}

fn sync_tree_at_depth(
    path: &Path,
    trusted_root: &Path,
    budget: &mut CleanupTraversalBudget,
    depth: usize,
) -> Result<(), String> {
    budget.inspect_entry(path, depth)?;
    reject_symlinks_below(path, trusted_root)
        .map_err(|error| format!("cannot inspect staged baseline replacement: {error}"))?;
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        format!("cannot inspect staged baseline replacement {}: {error}", path.display())
    })?;
    if metadata.file_type().is_symlink() {
        return Err(format!("refusing symlinked staged baseline replacement: {}", path.display()));
    }
    if metadata.is_dir() {
        for entry in fs::read_dir(path).map_err(|error| {
            format!("cannot enumerate staged baseline replacement {}: {error}", path.display())
        })? {
            let entry = entry.map_err(|error| {
                format!("cannot enumerate staged baseline replacement: {error}")
            })?;
            sync_tree_at_depth(&entry.path(), trusted_root, budget, depth + 1)?;
        }
        return fs::File::open(path).and_then(|directory| directory.sync_all()).map_err(|error| {
            format!("cannot sync staged baseline directory {}: {error}", path.display())
        });
    }
    if !metadata.is_file() {
        return Err(format!("refusing non-regular staged baseline entry: {}", path.display()));
    }
    fs::File::open(path)
        .and_then(|file| file.sync_all())
        .map_err(|error| format!("cannot sync staged baseline file {}: {error}", path.display()))
}

fn baseline_files_below(track_dir: &Path) -> Result<Vec<PathBuf>, String> {
    let mut baseline_files = Vec::new();
    let mut budget = CleanupTraversalBudget::new();
    collect_baseline_files_below(track_dir, &mut baseline_files, &mut budget, 0, false)?;
    Ok(baseline_files)
}

fn collect_baseline_files_below(
    track_dir: &Path,
    baseline_files: &mut Vec<PathBuf>,
    budget: &mut CleanupTraversalBudget,
    depth: usize,
    already_counted: bool,
) -> Result<(), String> {
    if !already_counted {
        budget.inspect_entry(track_dir, depth)?;
    }
    let metadata = fs::symlink_metadata(track_dir)
        .map_err(|error| format!("cannot inspect {}: {error}", track_dir.display()))?;
    if metadata.file_type().is_symlink() {
        return Err(format!(
            "refusing symlinked baseline staging directory: {}",
            track_dir.display()
        ));
    }
    if !metadata.is_dir() {
        return Err(format!("baseline staging path is not a directory: {}", track_dir.display()));
    }
    for entry in fs::read_dir(track_dir)
        .map_err(|error| format!("cannot enumerate {}: {error}", track_dir.display()))?
    {
        let entry = entry.map_err(|error| format!("cannot enumerate baseline staging: {error}"))?;
        let path = entry.path();
        budget.inspect_entry(&path, depth + 1)?;
        let child = fs::symlink_metadata(&path)
            .map_err(|error| format!("cannot inspect {}: {error}", path.display()))?;
        if child.file_type().is_symlink() {
            return Err(format!("refusing symlinked baseline staging entry: {}", path.display()));
        }
        if child.is_dir() {
            collect_baseline_files_below(&path, baseline_files, budget, depth + 1, true)?;
        } else if !child.is_file() {
            return Err(format!("refusing non-regular baseline staging entry: {}", path.display()));
        } else if path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.ends_with("-types-baseline.json"))
        {
            baseline_files.push(path);
        }
    }
    Ok(())
}
