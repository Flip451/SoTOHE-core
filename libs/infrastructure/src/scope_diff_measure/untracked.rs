use std::collections::HashSet;
use std::io::{BufReader, Read};
use std::path::Path;

use domain::review_v2::{FilePath, ReviewScopeConfig};
use globset::{GlobBuilder, GlobSet, GlobSetBuilder};
use usecase::batch_plan::ScopeDiffMeasureError;

use super::measure_failed;
use crate::sanitized_failure::io_classification;
use crate::track::symlink_guard::reject_symlinks_below;

pub(super) const MAX_UNTRACKED_FILE_BYTES: u64 = 16 * 1024 * 1024;
const GIT_EXCLUDE_PATHSPEC_PREFIX: &str = ":(top,exclude)";
/// Generated build, cache, and credential outputs excluded before collection.
pub(super) const IGNORED_NON_REVIEW_PATHS: [&str; 33] = [
    ":(top,exclude)target/**",
    ":(top,exclude)target-*/**",
    ":(top,exclude)**/*.rs.bk",
    ":(top,exclude).claude/logs/**",
    ":(top,exclude).claude/worktrees/**",
    ":(top,exclude).fastembed_cache/**",
    ":(top,exclude)**/.fastembed_cache/**",
    ":(top,exclude).semantic_index/**",
    ":(top,exclude)**/.semantic_index/**",
    ":(top,exclude).semantic_index.*",
    // Git reports an ignored nested repository as the directory itself; exclude
    // that exact path as well as its descendants before regular-file inspection.
    ":(top,exclude).cache",
    ":(top,exclude)sotp-dry-index-*/**",
    ":(top,exclude).env",
    ":(top,exclude).env.*",
    ":(top,exclude)**/*.pem",
    ":(top,exclude)**/*.key",
    ":(top,exclude)private/**",
    ":(top,exclude)config/secrets/**",
    ":(top,exclude)tmp/**",
    ":(top,exclude).cache/**",
    ":(top,exclude).harness/tools/**",
    ":(top,exclude)bin/sotp",
    ":(top,exclude).cargo-install/**",
    ":(top,exclude)repomix-output.txt",
    ":(top,exclude)repomix-output.xml",
    ":(top,exclude)repomix-output.*/**",
    ":(top,exclude).idea/**",
    ":(top,exclude).vscode/**",
    ":(top,exclude).locks/**",
    ":(top,exclude)track/items/**/.commit_hash",
    ":(top,exclude)track/items/**/.commit_hash.tmp",
    ":(top,exclude)track/items/*/*-graph*/**",
    ":(top,exclude)track/items/*/logs/**",
];

/// Counts an untracked file's whole length as additions.
pub(super) fn untracked_paths(output: &[u8]) -> Result<Vec<FilePath>, ScopeDiffMeasureError> {
    output
        .split(|byte| *byte == b'\0')
        .filter(|raw_path| !raw_path.is_empty())
        .map(file_path_bytes)
        .collect()
}

/// Builds matchers for the directory prefixes represented by the exclusion
/// pathspecs. Keeping the prefixes derived from [`IGNORED_NON_REVIEW_PATHS`]
/// prevents the Git pathspec list and the Rust-side safety filter from drifting.
fn ignored_non_review_path_set() -> Result<GlobSet, ScopeDiffMeasureError> {
    let mut builder = GlobSetBuilder::new();
    for prefix in IGNORED_NON_REVIEW_PATHS.iter().filter_map(|pathspec| {
        pathspec
            .strip_prefix(GIT_EXCLUDE_PATHSPEC_PREFIX)
            .and_then(|pathspec| pathspec.strip_suffix("/**"))
    }) {
        for pattern in [prefix.to_owned(), format!("{prefix}/**")] {
            let glob =
                GlobBuilder::new(&pattern).literal_separator(true).build().map_err(|error| {
                    measure_failed(format!("invalid ignored path pattern: {error}"))
                })?;
            builder.add(glob);
        }
    }
    builder.build().map_err(|error| measure_failed(format!("build ignored path matcher: {error}")))
}

/// Removes entries that Git emitted despite a non-review exclusion pathspec.
/// Trailing separators are ignored for matching because Git uses them to mark
/// an embedded repository as a directory; they remain intact for reviewable
/// paths so [`count_file_lines`] can fail closed on the non-regular entry.
pub(super) fn drop_ignored_non_review_paths(
    paths: Vec<FilePath>,
) -> Result<Vec<FilePath>, ScopeDiffMeasureError> {
    let ignored_paths = ignored_non_review_path_set()?;
    Ok(paths
        .into_iter()
        .filter(|path| !ignored_paths.is_match(path.as_str().trim_end_matches('/')))
        .collect())
}

/// Keeps paths that [`ReviewScopeConfig::classify`] will include in a scope.
/// This applies operational and other-track exclusions before any file I/O.
pub(super) fn retained_paths(
    scope_config: &ReviewScopeConfig,
    paths: Vec<FilePath>,
) -> Vec<FilePath> {
    let included: HashSet<FilePath> =
        scope_config.classify(&paths).into_values().flatten().collect();
    paths.into_iter().filter(|path| included.contains(path)).collect()
}

pub(super) fn untracked_additions(
    root: &Path,
    paths: &[FilePath],
) -> Result<Vec<(FilePath, u32)>, ScopeDiffMeasureError> {
    let mut changed = Vec::new();
    let mut remaining_bytes = MAX_UNTRACKED_FILE_BYTES;
    for path in paths {
        let absolute_path = root.join(path.as_str());
        match reject_symlinks_below(&absolute_path, root) {
            Ok(true) => {}
            Ok(false) => continue,
            Err(error) => {
                return Err(measure_failed(format!(
                    "refusing untracked path {}: {}",
                    path.as_str(),
                    io_classification(&error)
                )));
            }
        }
        let lines = count_file_lines(&absolute_path, path.as_str(), &mut remaining_bytes)?;
        changed.push((path.clone(), lines));
    }
    Ok(changed)
}

/// Counts newline-delimited lines without requiring UTF-8 or whole-file buffering.
///
/// `relative` is the candidate's repository-relative path and is what every
/// failure names: `path` is absolute and stays out of the reported message.
pub(super) fn count_file_lines(
    path: &Path,
    relative: &str,
    remaining_bytes: &mut u64,
) -> Result<u32, ScopeDiffMeasureError> {
    let path_metadata = std::fs::symlink_metadata(path).map_err(|error| {
        measure_failed(format!(
            "read metadata for untracked file {relative}: {}",
            io_classification(&error)
        ))
    })?;
    if !path_metadata.file_type().is_file() {
        return Err(measure_failed(format!("untracked file {relative} is not a regular file")));
    }

    let file = std::fs::File::open(path).map_err(|error| {
        measure_failed(format!("read untracked file {relative}: {}", io_classification(&error)))
    })?;
    let metadata = file.metadata().map_err(|error| {
        measure_failed(format!(
            "read metadata for untracked file {relative}: {}",
            io_classification(&error)
        ))
    })?;
    if !metadata.is_file() || metadata.len() > *remaining_bytes {
        return Err(measure_failed(format!(
            "untracked file {relative} exceeds the remaining {remaining_bytes}-byte read budget"
        )));
    }

    let budget = *remaining_bytes;
    let mut reader = BufReader::new(file.take(budget.saturating_add(1)));
    let mut buffer = [0_u8; 8 * 1024];
    let mut bytes_seen = 0_u64;
    let mut lines = 0_u32;
    let mut saw_bytes = false;
    let mut last_byte = 0_u8;

    loop {
        let bytes_read = reader.read(&mut buffer).map_err(|error| {
            measure_failed(format!("read untracked file {relative}: {}", io_classification(&error)))
        })?;
        if bytes_read == 0 {
            break;
        }
        bytes_seen = bytes_seen.saturating_add(bytes_read as u64);
        if bytes_seen > budget {
            return Err(measure_failed(format!(
                "untracked file {relative} exceeds the remaining {budget}-byte read budget"
            )));
        }
        let Some(chunk) = buffer.get(..bytes_read) else {
            return Err(measure_failed(format!("short read from untracked file {relative}")));
        };
        saw_bytes = true;
        if let Some(byte) = chunk.last() {
            last_byte = *byte;
        }
        for byte in chunk {
            if *byte == b'\n' {
                lines = lines.saturating_add(1);
            }
        }
    }

    if saw_bytes && last_byte != b'\n' {
        lines = lines.saturating_add(1);
    }
    *remaining_bytes = remaining_bytes.saturating_sub(bytes_seen);
    Ok(lines)
}

pub(super) fn file_path(raw: &str) -> Result<FilePath, ScopeDiffMeasureError> {
    let normalized = raw.strip_prefix("./").unwrap_or(raw);
    FilePath::new(normalized)
        .map_err(|error| measure_failed(format!("invalid path '{normalized}': {error}")))
}

pub(super) fn file_path_bytes(raw: &[u8]) -> Result<FilePath, ScopeDiffMeasureError> {
    let raw = std::str::from_utf8(raw)
        .map_err(|_| measure_failed("Git returned a non-UTF-8 repository path"))?;
    file_path(raw)
}
