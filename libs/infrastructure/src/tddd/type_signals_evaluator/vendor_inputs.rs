//! Git-visible vendor and rustdoc input closure helpers.

use std::path::Path;

use super::super::EvaluateSignalsError;
use super::super::layer_graph::LayerCrateRoot;
use super::{
    MAX_SOURCE_DEPTH, MAX_SOURCE_ENTRIES, MAX_SOURCE_FILES, MAX_SOURCE_PATH_LIST_BYTES,
    TreeFileDigest, digest_local_file, load_layer_graph, local_input_error,
};

const VENDOR_ROOT: &str = "vendor";

pub(crate) fn collect_local_tree_file_digests(
    workspace_root: &Path,
    roots: &[LayerCrateRoot],
    remaining_budget: &mut u64,
) -> Result<Vec<TreeFileDigest>, EvaluateSignalsError> {
    let mut files = Vec::new();
    let mut visited_entries = 0usize;
    for root in roots {
        ensure_local_tree_directory(
            workspace_root,
            &root.path,
            format!("layer crate '{}'", root.crate_name),
        )?;
        collect_local_tree_file_digests_for_root(
            workspace_root,
            &root.path,
            &mut visited_entries,
            &mut files,
            remaining_budget,
        )?;
    }
    if files.is_empty() {
        return Err(EvaluateSignalsError::authoritative_input(
            "architecture layer closure contains no regular files".to_owned(),
        ));
    }

    // `vendor/` is a workspace-level Cargo patch closure, not an architecture
    // layer. Keep it in the implementation-input closure whenever it exists;
    // Git remains the authority for which tracked and non-ignored files are
    // visible, so ignored files cannot affect freshness.
    if optional_local_tree_directory_present(workspace_root, VENDOR_ROOT)? {
        collect_local_tree_file_digests_for_root(
            workspace_root,
            VENDOR_ROOT,
            &mut visited_entries,
            &mut files,
            remaining_budget,
        )?;
    }
    Ok(files)
}

fn ensure_local_tree_directory(
    workspace_root: &Path,
    relative_root: &str,
    label: String,
) -> Result<(), EvaluateSignalsError> {
    let path = workspace_root.join(relative_root);
    crate::track::symlink_guard::reject_symlinks_up_to_root(&path).map_err(|error| {
        EvaluateSignalsError::authoritative_input(format!(
            "cannot inspect {label} at '{relative_root}': {error}"
        ))
    })?;
    let metadata = std::fs::symlink_metadata(&path).map_err(|error| {
        EvaluateSignalsError::authoritative_input(format!(
            "cannot inspect {label} at '{relative_root}': {error}"
        ))
    })?;
    if metadata.file_type().is_symlink() {
        return Err(EvaluateSignalsError::authoritative_input(format!(
            "{label} at '{relative_root}' is a symlink"
        )));
    }
    if !metadata.is_dir() {
        return Err(EvaluateSignalsError::authoritative_input(format!(
            "{label} at '{relative_root}' is not a directory"
        )));
    }
    Ok(())
}

fn optional_local_tree_directory_present(
    workspace_root: &Path,
    relative_root: &str,
) -> Result<bool, EvaluateSignalsError> {
    let path = workspace_root.join(relative_root);
    crate::track::symlink_guard::reject_symlinks_up_to_root(&path).map_err(|error| {
        EvaluateSignalsError::authoritative_input(format!(
            "cannot inspect optional implementation tree '{relative_root}': {error}"
        ))
    })?;
    let metadata = match std::fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => {
            return Err(EvaluateSignalsError::authoritative_input(format!(
                "cannot inspect optional implementation tree '{relative_root}': {error}"
            )));
        }
    };
    if metadata.file_type().is_symlink() {
        return Err(EvaluateSignalsError::authoritative_input(format!(
            "optional implementation tree '{relative_root}' is a symlink"
        )));
    }
    if !metadata.is_dir() {
        return Err(EvaluateSignalsError::authoritative_input(format!(
            "optional implementation tree '{relative_root}' is not a directory"
        )));
    }
    Ok(true)
}

fn collect_local_tree_file_digests_for_root(
    workspace_root: &Path,
    relative_root: &str,
    visited_entries: &mut usize,
    files: &mut Vec<TreeFileDigest>,
    remaining_budget: &mut u64,
) -> Result<(), EvaluateSignalsError> {
    let paths = collect_local_tree_paths(workspace_root, relative_root, visited_entries)?;
    for relative in paths {
        if is_vcs_internal(&relative) {
            continue;
        }
        if files.len() >= MAX_SOURCE_FILES {
            return Err(EvaluateSignalsError::authoritative_input(format!(
                "implementation source traversal exceeds maximum of {MAX_SOURCE_FILES} files"
            )));
        }

        let path = workspace_root.join(&relative);
        crate::track::symlink_guard::reject_symlinks_up_to_root(&path).map_err(|error| {
            local_input_error(&path, format!("cannot inspect file path: {error}"))
        })?;
        let metadata = match std::fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => {
                return Err(local_input_error(&path, format!("cannot stat tree entry: {error}")));
            }
        };
        if metadata.file_type().is_symlink() {
            return Err(EvaluateSignalsError::authoritative_input(format!(
                "implementation tree entry '{relative}' is a symlink"
            )));
        }
        if !metadata.is_file() {
            return Err(EvaluateSignalsError::authoritative_input(format!(
                "implementation tree entry '{relative}' is not a regular file"
            )));
        }
        let (digest, _) = digest_local_file(&path, &relative, remaining_budget)?;
        files.push((relative.into_bytes(), digest));
    }
    Ok(())
}

fn collect_local_tree_paths(
    workspace_root: &Path,
    crate_root: &str,
    visited_entries: &mut usize,
) -> Result<Vec<String>, EvaluateSignalsError> {
    let output = crate::git_cli::isolation::isolated_bounded_git_output(
        workspace_root,
        &[
            "ls-files",
            "-z",
            "--cached",
            "--others",
            "--exclude-standard",
            "--full-name",
            "--",
            crate_root,
        ],
        MAX_SOURCE_PATH_LIST_BYTES,
    )
    .map_err(|error| {
        EvaluateSignalsError::authoritative_input(format!(
            "cannot enumerate implementation files below '{crate_root}': {error}"
        ))
    })?;
    if !output.status.success() {
        return Err(EvaluateSignalsError::authoritative_input(format!(
            "git ls-files failed below '{crate_root}' (exit {}): {}",
            output.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }

    let prefix = format!("{crate_root}/");
    let mut paths = Vec::new();
    for record in output.stdout.split(|byte| *byte == 0).filter(|record| !record.is_empty()) {
        *visited_entries = visited_entries.checked_add(1).ok_or_else(|| {
            EvaluateSignalsError::authoritative_input(
                "implementation source entry count overflowed".to_owned(),
            )
        })?;
        if *visited_entries > MAX_SOURCE_ENTRIES {
            return Err(EvaluateSignalsError::authoritative_input(format!(
                "implementation source traversal exceeds maximum of {MAX_SOURCE_ENTRIES} entries"
            )));
        }
        let relative = String::from_utf8(record.to_vec()).map_err(|error| {
            EvaluateSignalsError::authoritative_input(format!(
                "Git returned a non-UTF-8 implementation file path below '{crate_root}': {error}"
            ))
        })?;
        if !relative.starts_with(&prefix) {
            return Err(EvaluateSignalsError::authoritative_input(format!(
                "Git returned an implementation file outside '{crate_root}': {relative}"
            )));
        }
        if is_vcs_internal(&relative) {
            continue;
        }
        if crate_relative_depth(crate_root, &relative) > MAX_SOURCE_DEPTH {
            return Err(EvaluateSignalsError::authoritative_input(format!(
                "implementation source traversal exceeds maximum depth of {MAX_SOURCE_DEPTH} at '{relative}'"
            )));
        }
        paths.push(relative);
    }
    paths.sort();
    Ok(paths)
}

/// Returns the Git-visible files that a fresh rustdoc workspace must expose.
/// This is deliberately the same graph closure, optional vendor closure, and
/// path authority used by the local and branch implementation-input digests.
pub(crate) fn rustdoc_input_paths(
    workspace_root: &Path,
    target: &str,
) -> Result<(Vec<String>, Vec<String>), EvaluateSignalsError> {
    let graph = load_layer_graph(workspace_root)?;
    let roots = graph.crate_roots_for(target).map_err(EvaluateSignalsError::authoritative_input)?;
    let mut paths = vec!["Cargo.lock".to_owned(), "Cargo.toml".to_owned()];
    let mut visited_entries = 0_usize;
    let members = roots.iter().map(|root| root.path.clone()).collect::<Vec<_>>();
    for root in roots {
        paths.extend(collect_local_tree_paths(workspace_root, &root.path, &mut visited_entries)?);
    }
    if optional_local_tree_directory_present(workspace_root, VENDOR_ROOT)? {
        paths.extend(collect_local_tree_paths(workspace_root, VENDOR_ROOT, &mut visited_entries)?);
    }
    paths.sort();
    paths.dedup();
    if paths.len() > MAX_SOURCE_FILES.saturating_add(2) {
        return Err(EvaluateSignalsError::authoritative_input(format!(
            "rustdoc input closure exceeds maximum of {MAX_SOURCE_FILES} source files"
        )));
    }
    Ok((paths, members))
}

pub(crate) fn crate_relative_depth(crate_root: &str, relative: &str) -> usize {
    relative.strip_prefix(crate_root).unwrap_or(relative).trim_start_matches('/').split('/').count()
}

fn is_vcs_internal(path: &str) -> bool {
    path.split('/').any(|component| component == ".git")
}
