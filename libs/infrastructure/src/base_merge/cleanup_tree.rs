use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::{
    MAX_CLEANUP_FILE_BYTES, MAX_CLEANUP_TREE_BYTES, MAX_CLEANUP_TREE_DEPTH,
    MAX_CLEANUP_TREE_ENTRIES, TRACK_WRITER_LOCK_FILE, read_regular_file_bounded,
};
use crate::FsSymlinkGuard;
use crate::tddd::baseline_rustdoc_codec::BaselineRustdocCodec;
use crate::tddd::feature_declaration_adapter::FsTdddFeatureDeclarationAdapter;
use crate::tddd::tddd_layer_bindings_adapter::FsTdddLayerBindingsAdapter;
use crate::track::atomic_write::atomic_write_file;
use crate::track::symlink_guard::{reject_symlinks_below, reject_symlinks_up_to_root};
use domain::TrackId;
use domain::tddd::catalogue_v2::{RustdocBaselineCapturePort, TdddLayerBindingsPort};
use usecase::base_merge::BaselineReplacementError;
use usecase::baseline_capture::{
    BaselineCaptureInteractor, BaselineCaptureRequest, BaselineCaptureService,
};
use usecase::git_workflow::DiagnosticText;

use super::BASELINE_REPLACEMENT_PHASE_MARKER;
use crate::conventions_resolve::directory_walk::{
    ListingError, bounded_entries, open_directory_at,
};

#[cfg(test)]
pub(super) fn copy_cleanup_inputs(
    source_workspace: &Path,
    target_workspace: &Path,
    track_id: &str,
) -> Result<(), String> {
    let generated_baseline_files =
        super::publication::generated_baseline_file_names(source_workspace)?;
    copy_cleanup_inputs_with_baselines(
        source_workspace,
        target_workspace,
        track_id,
        &generated_baseline_files,
    )
}

pub(super) fn copy_cleanup_inputs_with_baselines(
    source_workspace: &Path,
    target_workspace: &Path,
    track_id: &str,
    generated_baseline_files: &BTreeSet<String>,
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
    remove_tree_bounded(&target_track, target_workspace)?;
    copy_tree_with_baselines(
        &source_track,
        &target_track,
        false,
        target_workspace,
        generated_baseline_files,
    )?;
    remove_baseline_files(&target_track, generated_baseline_files)
}

#[cfg(test)]
pub(super) fn replace_tree(
    source: &Path,
    target: &Path,
    include_baselines: bool,
    trusted_target_root: &Path,
) -> Result<(), String> {
    remove_tree_bounded(target, trusted_target_root)?;
    copy_tree_with_baselines(
        source,
        target,
        include_baselines,
        trusted_target_root,
        &BTreeSet::new(),
    )
}

pub(super) fn remove_tree_bounded(path: &Path, trusted_root: &Path) -> Result<(), String> {
    let mut budget = CleanupTraversalBudget::new();
    let mut snapshot = Vec::new();
    collect_removal_snapshot(path, trusted_root, &mut budget, 0, &mut snapshot)?;
    for entry in snapshot.into_iter().rev() {
        remove_snapshot_entry(&entry, trusted_root)?;
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RemovalEntryKind {
    File,
    Directory,
    Symlink,
}

#[derive(Debug)]
struct RemovalEntry {
    path: PathBuf,
    kind: RemovalEntryKind,
}

fn collect_removal_snapshot(
    path: &Path,
    trusted_root: &Path,
    budget: &mut CleanupTraversalBudget,
    depth: usize,
    snapshot: &mut Vec<RemovalEntry>,
) -> Result<(), String> {
    budget.inspect_entry(path, depth)?;
    reject_symlinks_below(path, trusted_root)
        .map_err(|error| format!("cannot inspect removal target {}: {error}", path.display()))?;
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(format!("cannot inspect {}: {error}", path.display())),
    };
    let kind = if metadata.file_type().is_symlink() {
        RemovalEntryKind::Symlink
    } else if metadata.is_file() {
        if metadata.is_file() {
            budget.file_limit(path, metadata.len())?;
            budget.consume_file(path, metadata.len())?;
        }
        RemovalEntryKind::File
    } else if metadata.is_dir() {
        RemovalEntryKind::Directory
    } else {
        return Err(format!("refusing non-regular removal target: {}", path.display()));
    };
    snapshot.push(RemovalEntry { path: path.to_path_buf(), kind });
    if kind == RemovalEntryKind::Directory {
        for entry in fs::read_dir(path).map_err(|error| {
            format!("cannot enumerate removal target {}: {error}", path.display())
        })? {
            let entry =
                entry.map_err(|error| format!("cannot enumerate removal target: {error}"))?;
            collect_removal_snapshot(&entry.path(), trusted_root, budget, depth + 1, snapshot)?;
        }
    }
    Ok(())
}

fn remove_snapshot_entry(entry: &RemovalEntry, trusted_root: &Path) -> Result<(), String> {
    let path = &entry.path;
    reject_symlinks_below(path, trusted_root)
        .map_err(|error| format!("cannot inspect removal target {}: {error}", path.display()))?;
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(format!("cannot inspect {}: {error}", path.display())),
    };
    let actual_kind = if metadata.file_type().is_symlink() {
        RemovalEntryKind::Symlink
    } else if metadata.is_file() {
        RemovalEntryKind::File
    } else if metadata.is_dir() {
        RemovalEntryKind::Directory
    } else {
        return Err(format!("refusing non-regular removal target: {}", path.display()));
    };
    if actual_kind != entry.kind {
        return Err(format!(
            "removal target changed type during bounded cleanup: {}",
            path.display()
        ));
    }
    match actual_kind {
        RemovalEntryKind::File | RemovalEntryKind::Symlink => fs::remove_file(path),
        RemovalEntryKind::Directory => fs::remove_dir(path),
    }
    .map_err(|error| format!("cannot remove {}: {error}", path.display()))
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

pub(super) fn copy_tree_with_baselines(
    source: &Path,
    target: &Path,
    include_baselines: bool,
    trusted_target_root: &Path,
    generated_baseline_files: &BTreeSet<String>,
) -> Result<(), String> {
    let mut budget = CleanupTraversalBudget::new();
    copy_tree_with_budget(
        source,
        target,
        include_baselines,
        trusted_target_root,
        &mut budget,
        generated_baseline_files,
    )
}

pub(super) fn verify_non_baseline_content_matches(
    active_track: &Path,
    active_parent: &fs::File,
    prepared_replacement: &Path,
    replacement_parent: &fs::File,
    generated_baseline_files: &BTreeSet<String>,
) -> Result<(), String> {
    let active =
        collect_non_baseline_snapshot(active_track, active_parent, generated_baseline_files)?;
    let prepared = collect_non_baseline_snapshot(
        prepared_replacement,
        replacement_parent,
        generated_baseline_files,
    )?;
    (active == prepared)
        .then_some(())
        .ok_or_else(|| "active track content changed during baseline capture".to_owned())
}

#[derive(Debug, Eq, PartialEq)]
enum SnapshotEntry {
    Directory,
    File(Vec<u8>),
}

fn collect_non_baseline_snapshot(
    root: &Path,
    parent: &fs::File,
    generated_baseline_files: &BTreeSet<String>,
) -> Result<BTreeMap<PathBuf, SnapshotEntry>, String> {
    let root_name = root.file_name().ok_or_else(|| {
        format!("cannot inspect track snapshot without a name: {}", root.display())
    })?;
    let directory = open_directory_at(parent, Path::new(root_name))
        .map_err(|error| format!("cannot open track snapshot {}: {error}", root.display()))?;
    let mut budget = CleanupTraversalBudget::new();
    let mut snapshot = BTreeMap::new();
    collect_non_baseline_snapshot_at(
        root,
        directory,
        PathBuf::new(),
        &mut budget,
        0,
        &mut snapshot,
        generated_baseline_files,
    )?;
    Ok(snapshot)
}

fn collect_non_baseline_snapshot_at(
    snapshot_root: &Path,
    directory: fs::File,
    relative: PathBuf,
    budget: &mut CleanupTraversalBudget,
    depth: usize,
    snapshot: &mut BTreeMap<PathBuf, SnapshotEntry>,
    generated_baseline_files: &BTreeSet<String>,
) -> Result<(), String> {
    let path = snapshot_root.join(&relative);
    budget.inspect_entry(&path, depth)?;
    snapshot.insert(relative.clone(), SnapshotEntry::Directory);
    let mut listing_budget = MAX_CLEANUP_TREE_ENTRIES;
    let entries = match bounded_entries(&directory, &mut listing_budget) {
        Ok(entries) => entries,
        Err(ListingError::BudgetExhausted) => {
            return Err(format!(
                "cleanup input exceeds filesystem entry limit at {}",
                path.display()
            ));
        }
        Err(ListingError::Io(error)) => {
            return Err(format!("cannot enumerate track snapshot {}: {error}", path.display()));
        }
    };
    for entry in entries {
        let child_relative = relative.join(&entry.name);
        let child_path = snapshot_root.join(&child_relative);
        if entry.is_symlink {
            budget.inspect_entry(&child_path, depth + 1)?;
            return Err(format!(
                "refusing symlinked track snapshot entry: {}",
                child_path.display()
            ));
        }
        if entry.is_dir {
            let nested_directory = open_directory_at(&directory, &entry.name).map_err(|error| {
                format!("cannot open track snapshot directory {}: {error}", child_path.display())
            })?;
            collect_non_baseline_snapshot_at(
                snapshot_root,
                nested_directory,
                child_relative,
                budget,
                depth + 1,
                snapshot,
                generated_baseline_files,
            )?;
            continue;
        }
        budget.inspect_entry(&child_path, depth + 1)?;
        if !entry.is_file {
            return Err(format!(
                "refusing non-regular track snapshot entry: {}",
                child_path.display()
            ));
        }
        if is_ignored_publication_file(&child_relative, generated_baseline_files) {
            continue;
        }
        let file = open_snapshot_leaf_at_nofollow(&directory, &entry.name).map_err(|error| {
            format!("cannot open track snapshot file {}: {error}", child_path.display())
        })?;
        let opened_metadata = file.metadata().map_err(|error| {
            format!("cannot inspect opened track snapshot file {}: {error}", child_path.display())
        })?;
        if !opened_metadata.is_file() {
            return Err(format!(
                "track snapshot file changed while it was inspected: {}",
                child_path.display()
            ));
        }
        let size = opened_metadata.len();
        budget.file_limit(&child_path, size)?;
        budget.consume_file(&child_path, size)?;
        let mut content = Vec::new();
        file.take(size.saturating_add(1)).read_to_end(&mut content).map_err(|error| {
            format!("cannot read track snapshot file {}: {error}", child_path.display())
        })?;
        if content.len() as u64 != size {
            return Err(format!(
                "track snapshot file changed during publication: {}",
                child_path.display()
            ));
        }
        snapshot.insert(child_relative, SnapshotEntry::File(content));
    }
    Ok(())
}

fn open_snapshot_leaf_at_nofollow(parent: &fs::File, name: &Path) -> std::io::Result<fs::File> {
    rustix::fs::openat(
        parent,
        name,
        rustix::fs::OFlags::RDONLY | rustix::fs::OFlags::NOFOLLOW | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::empty(),
    )
    .map(fs::File::from)
    .map_err(std::io::Error::from)
}

fn is_ignored_publication_file(
    relative: &Path,
    generated_baseline_files: &BTreeSet<String>,
) -> bool {
    relative.components().count() == 1
        && relative.file_name().and_then(|name| name.to_str()).is_some_and(|name| {
            name == BASELINE_REPLACEMENT_PHASE_MARKER
                || name == TRACK_WRITER_LOCK_FILE
                || generated_baseline_files.contains(name)
        })
}

fn copy_tree_with_budget(
    source: &Path,
    target: &Path,
    include_baselines: bool,
    trusted_target_root: &Path,
    budget: &mut CleanupTraversalBudget,
    generated_baseline_files: &BTreeSet<String>,
) -> Result<(), String> {
    copy_tree_at_depth(
        source,
        target,
        include_baselines,
        trusted_target_root,
        budget,
        generated_baseline_files,
        0,
        false,
    )
}

#[allow(clippy::too_many_arguments)]
fn copy_tree_at_depth(
    source: &Path,
    target: &Path,
    include_baselines: bool,
    trusted_target_root: &Path,
    budget: &mut CleanupTraversalBudget,
    generated_baseline_files: &BTreeSet<String>,
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
            if depth == 0 && name == TRACK_WRITER_LOCK_FILE {
                continue;
            }
            if depth == 0 && !include_baselines && generated_baseline_files.contains(name.as_ref())
            {
                continue;
            }
            copy_tree_at_depth(
                &path,
                &target.join(name.as_ref()),
                include_baselines,
                trusted_target_root,
                budget,
                generated_baseline_files,
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

fn remove_baseline_files(
    track_dir: &Path,
    generated_baseline_files: &BTreeSet<String>,
) -> Result<(), String> {
    for path in baseline_files_below(track_dir, generated_baseline_files)? {
        fs::remove_file(&path)
            .map_err(|error| format!("cannot clear stale baseline {}: {error}", path.display()))?;
    }
    Ok(())
}

pub(super) fn capture_baselines_in_worktree(
    worktree: &Path,
    track_id: &str,
    capture: Arc<dyn RustdocBaselineCapturePort>,
) -> Result<(), BaselineReplacementError> {
    let interactor = BaselineCaptureInteractor::new(
        std::sync::Arc::new(FsSymlinkGuard::new()),
        std::sync::Arc::new(FsTdddLayerBindingsAdapter::new()),
        capture,
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
    budget.file_limit(path, metadata.len())?;
    budget.consume_file(path, metadata.len())?;
    fs::File::open(path)
        .and_then(|file| file.sync_all())
        .map_err(|error| format!("cannot sync staged baseline file {}: {error}", path.display()))
}

fn baseline_files_below(
    track_dir: &Path,
    generated_baseline_files: &BTreeSet<String>,
) -> Result<Vec<PathBuf>, String> {
    let mut baseline_files = Vec::new();
    let mut budget = CleanupTraversalBudget::new();
    collect_baseline_files_below(
        track_dir,
        &mut baseline_files,
        &mut budget,
        generated_baseline_files,
        0,
        false,
    )?;
    Ok(baseline_files)
}

fn collect_baseline_files_below(
    track_dir: &Path,
    baseline_files: &mut Vec<PathBuf>,
    budget: &mut CleanupTraversalBudget,
    generated_baseline_files: &BTreeSet<String>,
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
            collect_baseline_files_below(
                &path,
                baseline_files,
                budget,
                generated_baseline_files,
                depth + 1,
                true,
            )?;
        } else if !child.is_file() {
            return Err(format!("refusing non-regular baseline staging entry: {}", path.display()));
        } else if depth == 0
            && path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| generated_baseline_files.contains(name))
        {
            baseline_files.push(path);
        }
    }
    Ok(())
}
