use std::collections::HashSet;
use std::fs;
use std::io::{BufReader, Read};
use std::path::Path;

use domain::review_v2::{FilePath, ReviewScopeConfig};
use serde::Deserialize;
use usecase::batch_plan::ScopeDiffMeasureError;

use super::measure_failed;
use crate::sanitized_failure::io_classification;
use crate::track::symlink_guard::reject_symlinks_below;

pub(super) const MAX_UNTRACKED_FILE_BYTES: u64 = 16 * 1024 * 1024;
const GIT_EXCLUDE_PATHSPEC_PREFIX: &str = ":(top,exclude)";
const SCOPE_DIFF_EXCLUSIONS_CONFIG: &str = ".harness/config/scope-diff-exclusions.json";
const MAX_SCOPE_DIFF_EXCLUSIONS_BYTES: u64 = 64 * 1024;
const SUPPORTED_SCOPE_DIFF_EXCLUSIONS_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ScopeDiffExclusionsConfig {
    schema_version: u32,
    exclusions: Vec<String>,
}

/// Operator-selected paths excluded from both untracked enumerations.
#[derive(Debug)]
pub(super) struct ScopeDiffExclusions {
    git_pathspecs: Vec<String>,
}

impl ScopeDiffExclusions {
    fn from_patterns(patterns: Vec<String>) -> Result<Self, ScopeDiffMeasureError> {
        if patterns.is_empty() {
            return Err(measure_failed(format!(
                "load {SCOPE_DIFF_EXCLUSIONS_CONFIG}: no exclusion patterns configured"
            )));
        }

        let mut git_pathspecs = Vec::with_capacity(patterns.len());
        for (index, pattern) in patterns.iter().enumerate() {
            validate_exclusion_pattern(pattern, index)?;
            git_pathspecs.push(format!("{GIT_EXCLUDE_PATHSPEC_PREFIX}{pattern}"));
        }

        Ok(Self { git_pathspecs })
    }

    pub(super) fn git_pathspecs(&self) -> &[String] {
        &self.git_pathspecs
    }
}

/// Loads the mandatory operator-owned measurement exclusions from the repository.
pub(super) fn load_scope_diff_exclusions(
    repository_root: &Path,
) -> Result<ScopeDiffExclusions, ScopeDiffMeasureError> {
    let source = read_scope_diff_exclusions(repository_root)?;
    let config: ScopeDiffExclusionsConfig = serde_json::from_str(&source).map_err(|_| {
        measure_failed(format!("load {SCOPE_DIFF_EXCLUSIONS_CONFIG}: not valid JSON"))
    })?;
    if config.schema_version != SUPPORTED_SCOPE_DIFF_EXCLUSIONS_SCHEMA_VERSION {
        return Err(measure_failed(format!(
            "load {SCOPE_DIFF_EXCLUSIONS_CONFIG}: unsupported schema version"
        )));
    }
    ScopeDiffExclusions::from_patterns(config.exclusions)
}

fn read_scope_diff_exclusions(repository_root: &Path) -> Result<String, ScopeDiffMeasureError> {
    let root = repository_root.canonicalize().map_err(|error| {
        measure_failed(format!(
            "load {SCOPE_DIFF_EXCLUSIONS_CONFIG}: repository root is unreadable ({})",
            io_classification(&error)
        ))
    })?;
    let path = root.join(SCOPE_DIFF_EXCLUSIONS_CONFIG);
    match reject_symlinks_below(&path, &root).map_err(|error| {
        measure_failed(format!(
            "load {SCOPE_DIFF_EXCLUSIONS_CONFIG}: {}",
            io_classification(&error)
        ))
    })? {
        true => {}
        false => {
            return Err(measure_failed(format!("load {SCOPE_DIFF_EXCLUSIONS_CONFIG}: not found")));
        }
    }

    let metadata = fs::symlink_metadata(&path).map_err(|error| {
        measure_failed(format!(
            "load {SCOPE_DIFF_EXCLUSIONS_CONFIG}: {}",
            io_classification(&error)
        ))
    })?;
    if !metadata.file_type().is_file() || metadata.len() > MAX_SCOPE_DIFF_EXCLUSIONS_BYTES {
        return Err(measure_failed(format!(
            "load {SCOPE_DIFF_EXCLUSIONS_CONFIG}: not a bounded regular file"
        )));
    }

    let mut source = String::new();
    fs::File::open(&path)
        .map_err(|error| {
            measure_failed(format!(
                "load {SCOPE_DIFF_EXCLUSIONS_CONFIG}: {}",
                io_classification(&error)
            ))
        })?
        .take(MAX_SCOPE_DIFF_EXCLUSIONS_BYTES.saturating_add(1))
        .read_to_string(&mut source)
        .map_err(|error| {
            measure_failed(format!(
                "load {SCOPE_DIFF_EXCLUSIONS_CONFIG}: {}",
                io_classification(&error)
            ))
        })?;
    if source.len() as u64 > MAX_SCOPE_DIFF_EXCLUSIONS_BYTES {
        return Err(measure_failed(format!(
            "load {SCOPE_DIFF_EXCLUSIONS_CONFIG}: file exceeds its size limit"
        )));
    }
    Ok(source)
}

fn validate_exclusion_pattern(pattern: &str, index: usize) -> Result<(), ScopeDiffMeasureError> {
    if pattern.trim().is_empty()
        || pattern.contains('\0')
        || pattern.starts_with('/')
        || pattern.starts_with(':')
        || pattern.split('/').any(|component| component == "..")
    {
        return Err(measure_failed(format!(
            "load {SCOPE_DIFF_EXCLUSIONS_CONFIG}: invalid exclusion pattern at index {index}"
        )));
    }
    Ok(())
}

/// Counts an untracked file's whole length as additions.
pub(super) fn untracked_paths(output: &[u8]) -> Result<Vec<FilePath>, ScopeDiffMeasureError> {
    output
        .split(|byte| *byte == b'\0')
        .filter(|raw_path| !raw_path.is_empty())
        .map(file_path_bytes)
        .collect()
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
