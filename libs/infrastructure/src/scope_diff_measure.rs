//! Git secondary adapter measuring a track's per-scope diff
//! (IN-10, IN-14, IN-15, AC-17, CN-05, OUT-08).
//!
//! It sums additions and deletions from the batch base, counts untracked files
//! as additions, and reuses existing scope classification. Base resolution
//! degrades from the recorded commit to the configured base branch (CN-05).

use std::collections::{BTreeMap, HashSet};
use std::io::{BufReader, Read};
use std::path::Path;
use std::process::Stdio;

use domain::batch_plan::{LineCount, MeasuredScopeDiff};
use domain::review_v2::{FilePath, ReviewScopeConfig, ScopeName};
use domain::{FreeText, TrackId};
use usecase::batch_plan::{ScopeDiffMeasureError, ScopeDiffMeasurePort};
use usecase::fixpoint_resolve::DiffBaseResolverPort;

use crate::dry_check::FsDiffBaseResolverAdapter;
use crate::git_cli::{
    collect_bounded_git_output, guarded_git_command, spawn_bounded_git_child,
    without_history_rewrites, without_repository_selection,
};
use crate::review_scope_config_reader::REVIEW_SCOPE_CONFIG;
use crate::review_v2::load_v2_scope_config;
use crate::sanitized_failure::{io_classification, scope_config_classification};
use crate::track::symlink_guard::reject_symlinks_below;
use crate::track_artifact::{TrackArtifactReadError, read_track_artifact};

const METADATA_FILE: &str = "metadata.json";
const MAX_TRACK_METADATA_BYTES: u64 = 4 * 1024 * 1024;
const MAX_GIT_OUTPUT_BYTES: u64 = 16 * 1024 * 1024;
const MAX_UNTRACKED_FILE_BYTES: u64 = 16 * 1024 * 1024;
const MAX_BINARY_TREE_PATHSPEC_BYTES: usize = 64 * 1024;
/// Generated build, cache, and credential outputs excluded before collection.
const IGNORED_NON_REVIEW_PATHS: [&str; 32] = [
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

fn measure_failed(message: impl Into<String>) -> ScopeDiffMeasureError {
    ScopeDiffMeasureError::MeasureFailed { message: FreeText::new(message.into()) }
}

/// Measures a track's actual per-scope diff through git.
///
/// Constructed with no arguments so composition roots stay zero-argument
/// wiring accessors; the items directory arrives with each call.
#[derive(Debug, Default)]
pub struct GitScopeDiffMeasurer;

impl GitScopeDiffMeasurer {
    /// Creates the adapter.
    #[must_use]
    pub fn new() -> GitScopeDiffMeasurer {
        GitScopeDiffMeasurer
    }
}

impl ScopeDiffMeasurePort for GitScopeDiffMeasurer {
    fn measure_scope_diff(
        &self,
        items_dir: &Path,
        track_id: &TrackId,
    ) -> Result<Vec<MeasuredScopeDiff>, ScopeDiffMeasureError> {
        // Anchored on the items directory, so the diff measured is the one of
        // the repository the track artifacts live in.
        let (repo, _) =
            crate::discover_isolated_repo_for_items_dir(items_dir).map_err(|error| {
                measure_failed(format!(
                    "git repository not discovered: {}",
                    io_classification(&error)
                ))
            })?;
        let root = repo.root().to_path_buf();
        let items_dir = resolve_items_dir_under(items_dir, &root)?;
        let track_dir = items_dir.join(track_id.as_ref());

        let base = resolve_base(&root, &items_dir, track_id, &track_dir)?;
        let scope_config = load_v2_scope_config(&root.join(REVIEW_SCOPE_CONFIG), track_id, &root)
            .map_err(|error| {
            measure_failed(format!(
                "load {REVIEW_SCOPE_CONFIG}: {}",
                scope_config_classification(&error)
            ))
        })?;

        // Run from the discovery anchor, with paths pinned to the full tree.
        // Disable rename, textconv, and external-diff configuration so numstat
        // is a raw, unambiguous gate measurement.
        let numstat = git_bytes(
            &items_dir,
            &[
                "diff",
                "--no-renames",
                "--no-relative",
                "--no-textconv",
                "--no-ext-diff",
                "--ignore-submodules=none",
                "--numstat",
                "-z",
                base.as_ref(),
                "--",
            ],
        )?;
        // Enumerate ordinary and ignored files separately. The ignored pass is
        // needed for in-scope source files. Only the ignored pass excludes
        // trusted non-review outputs: a visible file may be explicitly
        // re-included by an ignore negation and must reach classification.
        let visible_untracked = git_bytes(
            &items_dir,
            &["ls-files", "-z", "--others", "--exclude-standard", "--full-name", "--", ":/"],
        )?;
        let ignored_args = [
            "ls-files",
            "-z",
            "--others",
            "--ignored",
            "--exclude-standard",
            "--full-name",
            "--",
            ":/",
        ]
        .into_iter()
        .chain(IGNORED_NON_REVIEW_PATHS)
        .collect::<Vec<_>>();
        let ignored_untracked = git_bytes(&items_dir, &ignored_args)?;

        let mut changed = binary_safe_numstat_additions(
            &root,
            &items_dir,
            base.as_ref(),
            retained_numstat_entries(&scope_config, parse_numstat(&numstat)?),
        )?;
        let mut untracked = untracked_paths(&visible_untracked)?;
        untracked.extend(untracked_paths(&ignored_untracked)?);
        let untracked = retained_paths(&scope_config, untracked);
        changed.extend(untracked_additions(&root, &untracked)?);

        Ok(accumulate_by_scope(&scope_config, &coalesce_by_path(changed)))
    }
}

/// Resolves the items directory inside its discovered repository.
fn resolve_items_dir_under(
    items_dir: &Path,
    root: &Path,
) -> Result<std::path::PathBuf, ScopeDiffMeasureError> {
    let canonical = items_dir.canonicalize().map_err(|error| {
        measure_failed(format!("items_dir rejected: {}", io_classification(&error)))
    })?;
    let canonical_root = root.canonicalize().map_err(|error| {
        measure_failed(format!("canonicalize repository root: {}", io_classification(&error)))
    })?;
    if !canonical.starts_with(&canonical_root) {
        // Neither path is named: both are absolute, and the operator supplied the
        // one that matters.
        return Err(measure_failed(
            "items_dir resolves outside the repository it was discovered from".to_owned(),
        ));
    }
    ensure_anchor_has_no_nested_git_directory(&canonical, &canonical_root)?;
    Ok(canonical)
}

/// Refuses a nested Git directory between the discovery anchor and reported root.
fn ensure_anchor_has_no_nested_git_directory(
    items_dir: &Path,
    reported_root: &Path,
) -> Result<(), ScopeDiffMeasureError> {
    let mut ancestor = items_dir;
    while ancestor != reported_root {
        match std::fs::symlink_metadata(ancestor.join(".git")) {
            Ok(_) => {
                return Err(measure_failed(
                    "items_dir is nested beneath a different git repository".to_owned(),
                ));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(measure_failed(format!(
                    "inspect items_dir ancestry: {}",
                    io_classification(&error)
                )));
            }
        }
        ancestor = ancestor.parent().ok_or_else(|| {
            measure_failed("items_dir ancestry could not reach the repository root".to_owned())
        })?;
    }
    Ok(())
}

/// Resolves the recorded base, degrading to the metadata base branch (CN-05).
fn resolve_base(
    root: &Path,
    items_dir: &Path,
    track_id: &TrackId,
    track_dir: &Path,
) -> Result<domain::CommitHash, ScopeDiffMeasureError> {
    let metadata =
        read_track_artifact(items_dir, track_id, METADATA_FILE, MAX_TRACK_METADATA_BYTES).map_err(
            |error| match error {
                TrackArtifactReadError::NotFound => measure_failed(format!(
                    "{METADATA_FILE} not found for track '{}'",
                    track_id.as_ref()
                )),
                // The failure names what was wrong and nothing about where the
                // reader looked; the file is already identified by its
                // well-known name and its track.
                TrackArtifactReadError::Failed(classification) => measure_failed(format!(
                    "read {METADATA_FILE} for track '{}': {classification}",
                    track_id.as_ref()
                )),
            },
        )?;
    let (metadata, _) = crate::track::codec::decode(&metadata).map_err(|error| {
        measure_failed(format!("decode {METADATA_FILE} for track '{}': {error}", track_id.as_ref()))
    })?;
    if metadata.id() != track_id {
        return Err(measure_failed(format!(
            "{METADATA_FILE} track_id '{}' does not match requested track '{}'",
            metadata.id().as_ref(),
            track_id.as_ref()
        )));
    }
    let canonical_root = root.canonicalize().map_err(|error| {
        measure_failed(format!("canonicalize repository root: {}", io_classification(&error)))
    })?;

    FsDiffBaseResolverAdapter::new(metadata.branch_strategy_snapshot().base_branch().to_owned())
        .resolve_diff_base(track_dir, &canonical_root, root)
        // The resolver's message carries the paths it consulted; the base branch it
        // failed to resolve is the part an operator can act on.
        .map_err(|_| {
            measure_failed(format!(
                "resolve diff base: neither the track's commit record nor the '{}' base branch \
                 could be resolved",
                metadata.branch_strategy_snapshot().base_branch()
            ))
        })
}

fn git_bytes(command_dir: &Path, args: &[&str]) -> Result<Vec<u8>, ScopeDiffMeasureError> {
    git_bytes_with_limit(command_dir, args, MAX_GIT_OUTPUT_BYTES as usize)
}

fn git_bytes_with_limit(
    command_dir: &Path,
    args: &[&str],
    max_output_bytes: usize,
) -> Result<Vec<u8>, ScopeDiffMeasureError> {
    let mut command = guarded_git_command();
    without_repository_selection(&mut command);
    // Ignore replacement objects so they cannot understate a measured scope.
    without_history_rewrites(&mut command);
    command.env("GIT_NO_LAZY_FETCH", "1");
    command
        .args(args)
        .current_dir(command_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let child = spawn_bounded_git_child(&mut command).map_err(|error| {
        measure_failed(format!("start git {}: {}", args.join(" "), io_classification(&error)))
    })?;
    let output = collect_bounded_git_output(child, max_output_bytes).map_err(|error| {
        measure_failed(format!(
            "collect git {} output: {}",
            args.join(" "),
            io_classification(&error)
        ))
    })?;
    let status = output.status;
    if !status.success() {
        return Err(measure_failed(format!(
            "git {} failed (exit {})",
            args.join(" "),
            status.code().map_or_else(|| "terminated".to_owned(), |code| code.to_string())
        )));
    }
    Ok(output.stdout)
}

/// Reads NUL-delimited `git diff --numstat -z` output as one `additions +
/// deletions` figure per file. A binary marker is retained so the caller can
/// derive a conservative attribute-independent figure rather than drop it.
fn parse_numstat(output: &[u8]) -> Result<Vec<NumstatEntry>, ScopeDiffMeasureError> {
    let mut changed = Vec::new();
    for record in output.split(|byte| *byte == b'\0') {
        if record.is_empty() {
            continue;
        }
        let mut fields = record.splitn(3, |byte| *byte == b'\t');
        let (Some(additions), Some(deletions), Some(path)) =
            (fields.next(), fields.next(), fields.next())
        else {
            return Err(measure_failed(format!(
                "unreadable numstat record: '{}'",
                String::from_utf8_lossy(record)
            )));
        };
        if additions == b"-" || deletions == b"-" {
            changed.push(NumstatEntry::Binary(file_path_bytes(path)?));
            continue;
        }
        let additions = parse_numstat_count(additions, record)?;
        let deletions = parse_numstat_count(deletions, record)?;
        changed.push(NumstatEntry::Counted(
            file_path_bytes(path)?,
            additions.saturating_add(deletions),
        ));
    }
    Ok(changed)
}

enum NumstatEntry {
    Counted(FilePath, u32),
    Binary(FilePath),
}

/// Gives every `-/-` row an attribute-independent conservative figure.
fn binary_safe_numstat_additions(
    root: &Path,
    command_dir: &Path,
    base: &str,
    entries: Vec<NumstatEntry>,
) -> Result<Vec<(FilePath, u32)>, ScopeDiffMeasureError> {
    let mut changed = Vec::new();
    let binary_paths: Vec<FilePath> = entries
        .iter()
        .filter_map(|entry| match entry {
            NumstatEntry::Counted(_, _) => None,
            NumstatEntry::Binary(path) => Some(path.clone()),
        })
        .collect();
    let base_sizes = binary_base_sizes(command_dir, base, &binary_paths)?;
    for entry in entries {
        match entry {
            NumstatEntry::Counted(path, lines) => changed.push((path, lines)),
            NumstatEntry::Binary(path) => {
                let base_size = base_sizes.get(path.as_str()).copied().unwrap_or(0);
                changed.push((
                    path.clone(),
                    base_size.saturating_add(binary_worktree_size(root, command_dir, &path)?),
                ));
            }
        }
    }
    Ok(changed)
}

/// Batches base-tree lookups into bounded argv-sized requests.
fn binary_base_sizes(
    command_dir: &Path,
    base: &str,
    paths: &[FilePath],
) -> Result<BTreeMap<String, u32>, ScopeDiffMeasureError> {
    let mut sizes = BTreeMap::new();
    let mut start = 0;
    while start < paths.len() {
        let mut pathspecs = Vec::new();
        let mut bytes = 0;
        while let Some(path) = paths.get(start + pathspecs.len()) {
            let pathspec = format!("./{}", path.as_str());
            if !pathspecs.is_empty() && bytes + pathspec.len() > MAX_BINARY_TREE_PATHSPEC_BYTES {
                break;
            }
            bytes += pathspec.len();
            pathspecs.push(pathspec);
        }
        start += pathspecs.len();
        let mut args = vec![
            "ls-tree".to_owned(),
            "--full-tree".to_owned(),
            "-l".to_owned(),
            "-z".to_owned(),
            base.to_owned(),
            "--".to_owned(),
        ];
        args.extend(pathspecs);
        let refs: Vec<&str> = args.iter().map(String::as_str).collect();
        let output = git_bytes_with_limit(command_dir, &refs, MAX_GIT_OUTPUT_BYTES as usize)?;
        for record in output.split(|byte| *byte == b'\0').filter(|record| !record.is_empty()) {
            let Some(separator) = record.iter().position(|byte| *byte == b'\t') else {
                return Err(measure_failed("unreadable base-tree entry for binary numstat path"));
            };
            let metadata = record.get(..separator).ok_or_else(|| {
                measure_failed("unreadable base-tree entry for binary numstat path")
            })?;
            let path_bytes = record.get(separator.saturating_add(1)..).ok_or_else(|| {
                measure_failed("unreadable base-tree entry for binary numstat path")
            })?;
            let path = file_path_bytes(path_bytes)?;
            let size = match metadata.split(|byte| *byte == b' ').nth(1) {
                Some(b"tree") => 0,
                Some(b"commit") => u32::MAX,
                Some(b"blob") => metadata
                    .split(|byte| *byte == b' ')
                    .rfind(|field| !field.is_empty())
                    .and_then(|size| std::str::from_utf8(size).ok())
                    .and_then(|size| size.parse::<u64>().ok())
                    .map(conservative_line_upper_bound)
                    .ok_or_else(|| {
                        measure_failed("unreadable base-blob size for binary numstat path")
                    })?,
                _ => {
                    return Err(measure_failed(
                        "unreadable base-tree entry for binary numstat path",
                    ));
                }
            };
            sizes.insert(path.as_str().to_owned(), size);
        }
    }
    Ok(sizes)
}

/// Derives an attribute-independent count for the current working-tree entry.
fn binary_worktree_size(
    root: &Path,
    command_dir: &Path,
    path: &FilePath,
) -> Result<u32, ScopeDiffMeasureError> {
    let absolute_path = root.join(path.as_str());
    match reject_symlinks_below(&absolute_path, root) {
        Ok(true) => {}
        Ok(false) => return Ok(0),
        Err(error) => {
            return Err(measure_failed(format!(
                "refusing binary numstat path {}: {}",
                path.as_str(),
                io_classification(&error)
            )));
        }
    }
    match std::fs::symlink_metadata(&absolute_path) {
        Ok(metadata) if metadata.file_type().is_dir() => {
            if is_worktree_gitlink(command_dir, path)? { Ok(u32::MAX) } else { Ok(0) }
        }
        Ok(metadata) if metadata.file_type().is_file() => {
            Ok(conservative_line_upper_bound(metadata.len()))
        }
        Ok(_) => Err(measure_failed(format!(
            "binary numstat path {} is not a regular file",
            path.as_str()
        ))),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(0),
        Err(error) => Err(measure_failed(format!(
            "read metadata for binary numstat path {}: {}",
            path.as_str(),
            io_classification(&error)
        ))),
    }
}

fn is_worktree_gitlink(command_dir: &Path, path: &FilePath) -> Result<bool, ScopeDiffMeasureError> {
    let pathspec = format!(":(top,literal){}", path.as_str());
    let output = git_bytes_with_limit(
        command_dir,
        &["ls-files", "--stage", "--full-name", "-z", "--", &pathspec],
        MAX_GIT_OUTPUT_BYTES as usize,
    )?;
    Ok(output.split(|byte| *byte == b'\0').filter(|record| !record.is_empty()).any(|record| {
        record.starts_with(b"160000 ")
            && record.splitn(2, |byte| *byte == b'\t').nth(1) == Some(path.as_str().as_bytes())
    }))
}

fn conservative_line_upper_bound(bytes: u64) -> u32 {
    u32::try_from(bytes).unwrap_or(u32::MAX)
}

fn parse_numstat_count(field: &[u8], record: &[u8]) -> Result<u32, ScopeDiffMeasureError> {
    std::str::from_utf8(field).ok().and_then(|value| value.parse().ok()).ok_or_else(|| {
        measure_failed(format!(
            "unreadable numstat count in record: '{}'",
            String::from_utf8_lossy(record)
        ))
    })
}

/// Counts an untracked file's whole length as additions.
fn untracked_paths(output: &[u8]) -> Result<Vec<FilePath>, ScopeDiffMeasureError> {
    output
        .split(|byte| *byte == b'\0')
        .filter(|raw_path| !raw_path.is_empty())
        .map(file_path_bytes)
        .collect()
}

/// Keeps paths that [`ReviewScopeConfig::classify`] will include in a scope.
/// This applies operational and other-track exclusions before any file I/O.
fn retained_paths(scope_config: &ReviewScopeConfig, paths: Vec<FilePath>) -> Vec<FilePath> {
    let included: HashSet<FilePath> =
        scope_config.classify(&paths).into_values().flatten().collect();
    paths.into_iter().filter(|path| included.contains(path)).collect()
}

/// Applies the same operational and other-track exclusions to tracked rows
/// before a binary fallback can inspect their working-tree paths.
fn retained_numstat_entries(
    scope_config: &ReviewScopeConfig,
    entries: Vec<NumstatEntry>,
) -> Vec<NumstatEntry> {
    let paths = entries
        .iter()
        .map(|entry| match entry {
            NumstatEntry::Counted(path, _) | NumstatEntry::Binary(path) => path.clone(),
        })
        .collect();
    let retained: HashSet<FilePath> = retained_paths(scope_config, paths).into_iter().collect();
    entries
        .into_iter()
        .filter(|entry| match entry {
            NumstatEntry::Counted(path, _) | NumstatEntry::Binary(path) => retained.contains(path),
        })
        .collect()
}

fn untracked_additions(
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
fn count_file_lines(
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

fn file_path(raw: &str) -> Result<FilePath, ScopeDiffMeasureError> {
    let normalized = raw.strip_prefix("./").unwrap_or(raw);
    FilePath::new(normalized)
        .map_err(|error| measure_failed(format!("invalid path '{normalized}': {error}")))
}

fn file_path_bytes(raw: &[u8]) -> Result<FilePath, ScopeDiffMeasureError> {
    let raw = std::str::from_utf8(raw)
        .map_err(|_| measure_failed("Git returned a non-UTF-8 repository path"))?;
    file_path(raw)
}

/// Merges tracked and untracked contributions for a path before classification.
fn coalesce_by_path(changed: Vec<(FilePath, u32)>) -> Vec<(FilePath, u32)> {
    let mut coalesced: BTreeMap<String, (FilePath, u32)> = BTreeMap::new();
    for (path, lines) in changed {
        coalesced
            .entry(path.as_str().to_owned())
            .and_modify(|(_, total)| *total = total.saturating_add(lines))
            .or_insert((path, lines));
    }
    coalesced.into_values().collect()
}

/// Attributes changed files to their configured scopes and sums their figures.
fn accumulate_by_scope(
    scope_config: &ReviewScopeConfig,
    changed: &[(FilePath, u32)],
) -> Vec<MeasuredScopeDiff> {
    let files: Vec<FilePath> = changed.iter().map(|(path, _)| path.clone()).collect();
    let classified = scope_config.classify(&files);

    let mut totals: BTreeMap<String, (ScopeName, u32)> = BTreeMap::new();
    for (scope, scope_files) in classified {
        let total = scope_files
            .iter()
            .filter_map(|file| {
                changed.iter().find(|(path, _)| path == file).map(|(_, lines)| *lines)
            })
            .fold(0_u32, u32::saturating_add);
        totals.insert(scope.to_string(), (scope, total));
    }

    totals
        .into_values()
        .map(|(scope, total)| MeasuredScopeDiff::new(scope, LineCount::new(total)))
        .collect()
}

#[cfg(test)]
#[allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::{
        MAX_UNTRACKED_FILE_BYTES, METADATA_FILE, NumstatEntry, accumulate_by_scope,
        binary_base_sizes, coalesce_by_path, count_file_lines, parse_numstat,
        retained_numstat_entries, retained_paths, untracked_additions, untracked_paths,
    };
    use domain::TrackId;
    use domain::batch_plan::LineCount;
    use domain::review_v2::{FilePath, ReviewScopeConfig};

    fn config() -> ReviewScopeConfig {
        ReviewScopeConfig::new(
            &TrackId::try_new("some-track").unwrap(),
            vec![
                ("domain".to_owned(), vec!["libs/domain/**".to_owned()], None, Some(500)),
                ("usecase".to_owned(), vec!["libs/usecase/**".to_owned()], None, Some(500)),
            ],
            Vec::new(),
            Vec::new(),
            None,
        )
        .unwrap()
    }

    // ── git-backed fixture ────────────────────────────────────────────────────

    fn git(dir: &std::path::Path, args: &[&str]) {
        let status = std::process::Command::new("git")
            .args(args)
            .current_dir(dir)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .unwrap();
        assert!(status.success(), "git {args:?} failed in {}", dir.display());
    }

    fn git_with_file_protocol(dir: &std::path::Path, args: &[&str]) {
        let status = std::process::Command::new("git")
            .args(["-c", "protocol.file.allow=always"])
            .args(args)
            .current_dir(dir)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .unwrap();
        assert!(status.success(), "git {args:?} failed in {}", dir.display());
    }

    fn write(dir: &std::path::Path, rel: &str, contents: &str) {
        let path = dir.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, contents).unwrap();
    }

    /// A repository whose `main` holds the base commit and whose track branch
    /// carries one committed change to `libs/domain/src/committed.rs` (+3
    /// lines). Nothing is staged, unstaged or untracked yet.
    fn fixture_repo() -> tempfile::TempDir {
        let repo = tempfile::Builder::new()
            .prefix("scope-diff-measure-repo-")
            .tempdir_in(std::env::current_dir().unwrap())
            .unwrap();
        let root = repo.path();

        write(
            root,
            ".harness/config/review-scope.json",
            r#"{"version": 2, "groups": {"domain": {"patterns": ["libs/domain/**"]}},
                "review_operational": ["track/items/<track-id>/review.json"],
                "other_track": [], "default_diff_ceiling_lines": 500}"#,
        );
        write(root, "libs/domain/src/committed.rs", &"line\n".repeat(10));
        write(root, "libs/domain/src/staged.rs", &"line\n".repeat(5));
        write(root, "libs/domain/src/unstaged.rs", &"line\n".repeat(5));
        write(root, "libs/usecase/src/renamed.rs", &"line\n".repeat(5));
        std::fs::create_dir_all(root.join("track/items/some-track")).unwrap();
        write(
            root,
            "track/items/some-track/metadata.json",
            r#"{"schema_version":6,"id":"some-track","title":"Test Track","created_at":"2026-01-01T00:00:00Z","updated_at":"2026-01-01T00:00:00Z","branch_strategy_snapshot":{"base_branch":"main","merge_target":"main","merge_method":"merge"}}"#,
        );

        git(root, &["init", "-b", "main"]);
        git(root, &["config", "user.email", "fixture@example.com"]);
        git(root, &["config", "user.name", "fixture"]);
        git(root, &["add", "-A"]);
        git(root, &["commit", "-m", "base"]);

        // The track branch carries one committed change over the base.
        git(root, &["checkout", "-b", "track/some-track"]);
        write(root, "libs/domain/src/committed.rs", &"line\n".repeat(13));
        git(root, &["add", "libs/domain/src/committed.rs"]);
        git(root, &["commit", "-m", "committed change"]);

        repo
    }

    /// Serialises the working-directory change these fixtures need. Test
    /// binaries share one process under plain `cargo test`; the workspace gate
    /// runs each test in its own process.
    static CWD_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn measure_in(root: &std::path::Path) -> Vec<domain::batch_plan::MeasuredScopeDiff> {
        use usecase::batch_plan::ScopeDiffMeasurePort;

        let _guard = CWD_LOCK.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let original = std::env::current_dir().unwrap();
        std::env::set_current_dir(root).unwrap();

        let measured = super::GitScopeDiffMeasurer::new().measure_scope_diff(
            std::path::Path::new("track/items"),
            &TrackId::try_new("some-track").unwrap(),
        );

        std::env::set_current_dir(original).unwrap();
        measured.unwrap()
    }

    fn domain_lines(measured: &[domain::batch_plan::MeasuredScopeDiff]) -> u32 {
        measured
            .iter()
            .find(|diff| diff.scope().to_string() == "domain")
            .map_or(0, |diff| diff.lines().value())
    }

    fn counted_entries(entries: Vec<NumstatEntry>) -> Vec<(FilePath, u32)> {
        entries
            .into_iter()
            .map(|entry| match entry {
                NumstatEntry::Counted(path, lines) => (path, lines),
                NumstatEntry::Binary(_) => {
                    panic!("this fixture contains only counted numstat rows")
                }
            })
            .collect()
    }

    #[test]
    fn test_a_replacement_object_cannot_hide_the_committed_diff() {
        // The base is the `main` tip and the track branch carries +3 committed
        // lines over it. A `refs/replace` entry standing the base in for that tip
        // makes an ordinary `git diff <base>` compare the tip with itself and
        // report nothing at all — a scope measured as zero, and a ceiling that
        // cannot be exceeded. Repository state, not environment: clearing the
        // repository-selecting variables does not touch it.
        let repo = fixture_repo();
        let root = repo.path().to_path_buf();
        assert_eq!(domain_lines(&measure_in(&root)), 3, "the honest figure before any replacement");

        let base = rev_parse(&root, "main");
        let tip = rev_parse(&root, "HEAD");
        git(&root, &["replace", &base, &tip]);

        assert_eq!(
            domain_lines(&measure_in(&root)),
            3,
            "the measurement must read the recorded history, not the presented one"
        );
    }

    fn rev_parse(dir: &std::path::Path, revision: &str) -> String {
        let output = std::process::Command::new("git")
            .args(["rev-parse", revision])
            .current_dir(dir)
            .output()
            .unwrap();
        assert!(output.status.success(), "the fixture must resolve {revision}");
        String::from_utf8(output.stdout).unwrap().trim().to_owned()
    }

    #[test]
    fn test_the_measurer_sums_committed_staged_and_unstaged_changes_from_the_base() {
        let repo = fixture_repo();
        let root = repo.path().to_path_buf();

        // committed on the track branch: +3 lines (10 -> 13).
        // staged: +2 lines (5 -> 7).
        write(&root, "libs/domain/src/staged.rs", &"line\n".repeat(7));
        git(&root, &["add", "libs/domain/src/staged.rs"]);
        // unstaged: +4 lines (5 -> 9).
        write(&root, "libs/domain/src/unstaged.rs", &"line\n".repeat(9));

        let measured = measure_in(&root);

        assert_eq!(
            domain_lines(&measured),
            3 + 2 + 4,
            "all three change kinds are summed against the base"
        );
    }

    #[test]
    fn test_the_measurement_follows_the_items_directory_rather_than_the_working_directory() {
        use usecase::batch_plan::ScopeDiffMeasurePort;

        let repo = fixture_repo();
        let root = repo.path().to_path_buf();
        // unstaged: +4 lines (5 -> 9), on top of the committed +3.
        write(&root, "libs/domain/src/unstaged.rs", &"line\n".repeat(9));

        // The working directory is left alone: the items directory names the
        // repository, so that is the tree measured.
        let measured = super::GitScopeDiffMeasurer::new()
            .measure_scope_diff(&root.join("track/items"), &TrackId::try_new("some-track").unwrap())
            .unwrap();

        assert_eq!(domain_lines(&measured), 3 + 4, "the fixture repository's own diff is measured");
    }

    #[test]
    fn test_the_measurer_refuses_an_items_dir_outside_the_anchor_reported_worktree() {
        use usecase::batch_plan::ScopeDiffMeasurePort;

        // Git can discover this inner repository from its items directory while
        // `core.worktree` makes --show-toplevel report the unrelated outer
        // checkout. In that shape, paths emitted by the inner repository must
        // never be joined to the outer worktree for an untracked-file read.
        let outer = fixture_repo();
        let inner = outer.path().join("nested");
        let items_dir = inner.join("track/items");
        std::fs::create_dir_all(&items_dir).unwrap();
        git(&inner, &["init", "-b", "main"]);
        git(&inner, &["config", "core.worktree", outer.path().to_str().unwrap()]);

        let error = super::GitScopeDiffMeasurer::new()
            .measure_scope_diff(&items_dir, &TrackId::try_new("some-track").unwrap())
            .expect_err("the reported worktree must enclose the items-dir anchor");

        assert!(
            error.to_string().contains("items_dir is nested beneath a different git repository"),
            "the anchor mismatch is refused before reading untracked paths: {error}"
        );
    }

    #[test]
    fn test_an_untracked_file_is_counted_as_additions_for_all_of_its_lines() {
        let repo = fixture_repo();
        let root = repo.path().to_path_buf();

        // Only the committed +3 so far.
        assert_eq!(domain_lines(&measure_in(&root)), 3);

        write(&root, "libs/domain/src/untracked.rs", &"line\n".repeat(7));

        assert_eq!(
            domain_lines(&measure_in(&root)),
            3 + 7,
            "an untracked file adds its whole length, with no deletions"
        );
    }

    #[test]
    fn test_an_ignored_untracked_in_scope_file_is_counted_as_additions() {
        let repo = fixture_repo();
        let root = repo.path().to_path_buf();
        write(&root, ".gitignore", "libs/domain/src/ignored.rs\n");
        write(&root, "libs/domain/src/ignored.rs", &"line\n".repeat(7));

        assert_eq!(
            domain_lines(&measure_in(&root)),
            3 + 7,
            "ignore configuration must not suppress an in-scope untracked file"
        );
    }

    #[test]
    fn test_an_ignore_negated_visible_file_is_kept_for_scope_classification() {
        let repo = fixture_repo();
        let root = repo.path().to_path_buf();
        write(&root, ".gitignore", ".env*\n!.env.example\n");
        write(&root, ".env.example", &"line\n".repeat(5));

        let measured = measure_in(&root);
        assert_eq!(
            measured
                .iter()
                .find(|diff| diff.scope().to_string() == "other")
                .map(|diff| diff.lines().value()),
            Some(7),
            "a visible ignore negation must not be filtered as a generated output"
        );
    }

    #[test]
    fn test_an_ignored_build_artifact_is_excluded_before_the_untracked_read_budget() {
        let repo = fixture_repo();
        let root = repo.path().to_path_buf();
        write(&root, ".gitignore", "target/\n");
        let artifact = root.join("target/generated.rs");
        std::fs::create_dir_all(artifact.parent().unwrap()).unwrap();
        std::fs::File::create(&artifact)
            .unwrap()
            .set_len(MAX_UNTRACKED_FILE_BYTES.saturating_add(1))
            .unwrap();

        assert_eq!(
            domain_lines(&measure_in(&root)),
            3,
            "ignored build output must not consume the untracked-file read budget"
        );
    }

    #[test]
    fn test_current_track_generated_outputs_are_excluded_before_the_untracked_read_budget() {
        let repo = fixture_repo();
        let root = repo.path().to_path_buf();
        write(
            &root,
            ".gitignore",
            "track/items/some-track/.commit_hash\ntrack/items/some-track/.commit_hash.tmp\ntrack/items/some-track/infrastructure-graph/\ntrack/items/some-track/logs/\n",
        );
        for path in [
            "track/items/some-track/.commit_hash",
            "track/items/some-track/.commit_hash.tmp",
            "track/items/some-track/infrastructure-graph/cache.json",
            "track/items/some-track/logs/telemetry.jsonl",
        ] {
            let artifact = root.join(path);
            std::fs::create_dir_all(artifact.parent().unwrap()).unwrap();
            std::fs::File::create(artifact)
                .unwrap()
                .set_len(MAX_UNTRACKED_FILE_BYTES.saturating_add(1))
                .unwrap();
        }

        assert_eq!(
            domain_lines(&measure_in(&root)),
            3,
            "generated current-track state must not reach the untracked reader"
        );
    }

    #[test]
    fn test_a_current_track_artifact_is_kept_for_scope_classification() {
        let repo = fixture_repo();
        let root = repo.path().to_path_buf();
        write(&root, "track/items/some-track/spec.json", &"line\n".repeat(5));

        let measured = measure_in(&root);
        let other = measured
            .iter()
            .find(|diff| diff.scope().to_string() == "other")
            .map(|diff| diff.lines().value());
        assert_eq!(other, Some(5), "track artifacts must reach configuration classification");
    }

    #[test]
    fn test_base_blob_counter_treats_a_leading_colon_as_a_literal_path() {
        let repo = fixture_repo();
        let root = repo.path();
        write(root, ":binary.rs", "first\nsecond\n");
        git(root, &["add", "--", "./:binary.rs"]);
        git(root, &["commit", "-m", "colon path"]);
        assert_eq!(
            binary_base_sizes(root, "HEAD", &[FilePath::new(":binary.rs").unwrap()])
                .unwrap()
                .get(":binary.rs"),
            Some(&13)
        );
    }

    #[test]
    fn test_base_tree_entry_counts_as_zero_for_a_file_type_transition() {
        let repo = fixture_repo();
        let root = repo.path();
        write(root, "replaced/child", "base\n");
        git(root, &["add", "replaced/child"]);
        git(root, &["commit", "-m", "base tree"]);

        assert_eq!(
            binary_base_sizes(root, "HEAD", &[FilePath::new("replaced").unwrap()])
                .unwrap()
                .get("replaced"),
            Some(&0)
        );
    }

    #[test]
    fn test_base_gitlink_entry_counts_conservatively() {
        let repo = fixture_repo();
        let root = repo.path();
        let cache_info = format!("160000,{},submodule", rev_parse(root, "HEAD"));
        git(root, &["update-index", "--add", "--cacheinfo", &cache_info]);
        git(root, &["commit", "-m", "base gitlink"]);

        assert_eq!(
            binary_base_sizes(root, "HEAD", &[FilePath::new("submodule").unwrap()])
                .unwrap()
                .get("submodule"),
            Some(&u32::MAX)
        );
    }

    #[test]
    fn test_a_submodule_ignore_configuration_cannot_suppress_a_gitlink_diff() {
        let repo = fixture_repo();
        let root = repo.path().to_path_buf();
        let path = "libs/domain/src/gitlink";
        let source = tempfile::Builder::new()
            .prefix("scope-diff-measure-submodule-")
            .tempdir_in(root.parent().unwrap())
            .unwrap();
        git(source.path(), &["init", "-b", "main"]);
        git(source.path(), &["config", "user.email", "fixture@example.com"]);
        git(source.path(), &["config", "user.name", "fixture"]);
        write(source.path(), "lib.rs", "source\n");
        git(source.path(), &["add", "lib.rs"]);
        git(source.path(), &["commit", "-m", "source"]);

        git_with_file_protocol(&root, &["submodule", "add", source.path().to_str().unwrap(), path]);
        git(&root, &["config", "diff.ignoreSubmodules", "all"]);

        assert_eq!(
            domain_lines(&measure_in(&root)),
            4,
            "a configured submodule ignore must not understate a gitlink change"
        );
    }

    #[test]
    fn test_binary_worktree_size_from_items_dir_counts_a_gitlink_conservatively() {
        let repo = fixture_repo();
        let root = repo.path().to_path_buf();
        let path = FilePath::new("libs/domain/src/gitlink").unwrap();
        let cache_info = format!("160000,{},{}", rev_parse(&root, "HEAD"), path.as_str());
        git(&root, &["update-index", "--add", "--cacheinfo", &cache_info]);
        std::fs::create_dir_all(root.join(path.as_str())).unwrap();

        assert_eq!(
            super::binary_worktree_size(&root, &root.join("track/items"), &path).unwrap(),
            u32::MAX,
            "the top-level gitlink must be visible from the nested discovery anchor"
        );
    }

    #[test]
    fn test_a_binary_attribute_cannot_suppress_a_tracked_text_change() {
        let repo = fixture_repo();
        let root = repo.path().to_path_buf();
        write(&root, ".gitattributes", "libs/domain/src/committed.rs binary\n");

        assert_eq!(
            domain_lines(&measure_in(&root)),
            50 + 65,
            "a repository attribute must produce a conservative count, not zero lines"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_binary_worktree_metadata_is_not_read_through_a_symlinked_ancestor() {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let external_file = outside.path().join("domain/src/binary.bin");
        std::fs::create_dir_all(external_file.parent().unwrap()).unwrap();
        std::fs::write(&external_file, "outside").unwrap();
        std::os::unix::fs::symlink(outside.path(), root.path().join("libs")).unwrap();

        let error = super::binary_worktree_size(
            root.path(),
            root.path(),
            &FilePath::new("libs/domain/src/binary.bin").unwrap(),
        )
        .expect_err("the ancestor symlink must be refused before metadata can short-circuit");

        assert!(error.to_string().contains("rejected as a symlink"), "got: {error}");
    }

    #[test]
    fn test_an_untracked_non_utf8_file_is_counted_from_its_bytes() {
        let repo = fixture_repo();
        let root = repo.path().to_path_buf();
        let path = root.join("libs/domain/src/non-utf8.rs");
        std::fs::write(&path, b"first\n\xffsecond\nthird").unwrap();

        assert_eq!(
            domain_lines(&measure_in(&root)),
            3 + 3,
            "non-UTF-8 bytes must not suppress an untracked file's line count"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_an_operational_untracked_symlink_is_excluded_before_it_is_read() {
        let repo = fixture_repo();
        let root = repo.path();
        let target = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(target.path(), "outside\n").unwrap();
        std::os::unix::fs::symlink(target.path(), root.join("track/items/some-track/review.json"))
            .unwrap();

        assert_eq!(
            domain_lines(&measure_in(root)),
            3,
            "an excluded operational path must not be read or counted"
        );
    }

    #[test]
    fn test_count_file_lines_file_above_budget_returns_error() {
        let file = tempfile::NamedTempFile::new().unwrap();
        file.as_file().set_len(MAX_UNTRACKED_FILE_BYTES.saturating_add(1)).unwrap();
        let mut remaining_bytes = MAX_UNTRACKED_FILE_BYTES;

        assert!(
            count_file_lines(file.path(), "libs/domain/src/oversized.rs", &mut remaining_bytes)
                .is_err()
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_count_file_lines_fifo_returns_error_without_blocking() {
        let directory = tempfile::tempdir().unwrap();
        let fifo = directory.path().join("untracked.rs");
        rustix::fs::mkfifoat(rustix::fs::CWD, &fifo, rustix::fs::Mode::from_raw_mode(0o600))
            .unwrap();
        let mut remaining_bytes = MAX_UNTRACKED_FILE_BYTES;

        let error = count_file_lines(&fifo, "libs/domain/src/untracked.rs", &mut remaining_bytes)
            .unwrap_err();

        assert!(matches!(error, usecase::batch_plan::ScopeDiffMeasureError::MeasureFailed { .. }));
        // A non-regular file is named by the relative path it was reported under,
        // not by the absolute one it was read from.
        let rendered = error.to_string();
        assert!(rendered.contains("libs/domain/src/untracked.rs"), "rendered as: {rendered}");
        assert!(
            !rendered.contains(&directory.path().display().to_string()),
            "rendered as: {rendered}"
        );
    }

    #[test]
    fn test_the_measurer_uses_the_track_snapshot_not_live_branch_strategy_config() {
        let repo = fixture_repo();
        let root = repo.path().to_path_buf();
        write(
            &root,
            ".harness/config/branch-strategy.json",
            r#"{"base_branch":"not-a-real-branch","merge_target":"not-a-real-branch","merge_method":"merge"}"#,
        );

        assert_eq!(
            domain_lines(&measure_in(&root)),
            3,
            "the snapshot's main base branch must be used despite later global config changes"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_a_path_containing_a_newline_survives_the_whole_measurement() {
        let repo = fixture_repo();
        let root = repo.path().to_path_buf();

        // Only the committed +3 so far.
        assert_eq!(domain_lines(&measure_in(&root)), 3);

        // A newline is a legal character in a path here. Line-delimited git
        // output would split this record in two; the NUL-delimited request
        // keeps it whole, so the file is classified by its real location.
        write(&root, "libs/domain/src/line\nbreak.rs", &"line\n".repeat(4));
        // A tracked change to a quote-worthy name goes through the same records.
        write(&root, "libs/domain/src/quote\"name.rs", &"line\n".repeat(2));
        git(&root, &["add", "--", "libs/domain/src/quote\"name.rs"]);

        assert_eq!(
            domain_lines(&measure_in(&root)),
            3 + 4 + 2,
            "both awkward names are counted under `domain`"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_an_untracked_symlink_is_rejected_before_its_target_is_read() {
        let repo = fixture_repo();
        let root = repo.path();
        let target = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(target.path(), "outside\n").unwrap();
        let link = root.join("libs/domain/src/untracked-link.rs");
        std::os::unix::fs::symlink(target.path(), &link).unwrap();

        let paths = untracked_paths(b"libs/domain/src/untracked-link.rs\0").unwrap();
        let result = untracked_additions(root, &paths);

        let error = result.expect_err("untracked symlinks must not be followed");
        // The refusal names the candidate by its repository-relative path. The
        // absolute prefix — a temporary directory here, an operator's checkout in
        // practice — is not part of what the measurement reports, and neither is
        // the guard's own message, which embeds the component it rejected.
        let rendered = error.to_string();
        assert!(
            rendered.contains("libs/domain/src/untracked-link.rs"),
            "the refusal must name the relative path: {rendered}"
        );
        assert!(
            !rendered.contains(&root.display().to_string()),
            "the refusal must not embed the repository root: {rendered}"
        );
        assert!(
            !rendered.contains(&target.path().display().to_string()),
            "the refusal must not embed the symlink target either: {rendered}"
        );
    }

    #[test]
    fn test_a_failure_to_measure_names_no_absolute_path() {
        use usecase::batch_plan::ScopeDiffMeasurePort;

        // A track the repository does not hold: the measurement fails on the
        // metadata read, one of the lanes whose underlying error carries the
        // absolute path it was reading.
        let repo = fixture_repo();
        let root = repo.path();

        let error = super::GitScopeDiffMeasurer::new()
            .measure_scope_diff(&root.join("track/items"), &TrackId::try_new("absent").unwrap())
            .expect_err("a track with no metadata cannot be measured");

        let rendered = error.to_string();
        assert!(rendered.contains(METADATA_FILE), "the failure names the file: {rendered}");
        assert!(rendered.contains("absent"), "the failure names the track: {rendered}");
        assert!(
            !rendered.contains(&root.display().to_string()),
            "no absolute path may reach the operator: {rendered}"
        );
    }

    #[test]
    fn test_retained_paths_excludes_operational_paths_before_content_is_read() {
        let config = ReviewScopeConfig::new(
            &TrackId::try_new("some-track").unwrap(),
            vec![("domain".to_owned(), vec!["libs/domain/**".to_owned()], None, None)],
            vec!["track/items/<track-id>/review.json".to_owned()],
            Vec::new(),
            None,
        )
        .unwrap();
        let paths = vec![
            FilePath::new("libs/domain/src/lib.rs").unwrap(),
            FilePath::new("track/items/some-track/review.json").unwrap(),
        ];

        assert_eq!(
            retained_paths(&config, paths),
            vec![FilePath::new("libs/domain/src/lib.rs").unwrap()]
        );
    }

    #[test]
    fn test_retained_numstat_entries_excludes_operational_binary_paths_before_fallbacks() {
        let config = ReviewScopeConfig::new(
            &TrackId::try_new("some-track").unwrap(),
            vec![("domain".to_owned(), vec!["libs/domain/**".to_owned()], None, None)],
            vec!["track/items/<track-id>/review.json".to_owned()],
            Vec::new(),
            None,
        )
        .unwrap();
        let entries = vec![
            NumstatEntry::Binary(FilePath::new("track/items/some-track/review.json").unwrap()),
            NumstatEntry::Counted(FilePath::new("libs/domain/src/lib.rs").unwrap(), 3),
        ];

        let retained = retained_numstat_entries(&config, entries);

        assert!(matches!(
            retained.as_slice(),
            [NumstatEntry::Counted(path, 3)] if path.as_str() == "libs/domain/src/lib.rs"
        ));
    }

    #[test]
    fn test_coalesce_by_path_sums_tracked_and_untracked_contributions() {
        let path = FilePath::new("libs/domain/src/recreated.rs").unwrap();
        let coalesced = coalesce_by_path(vec![(path.clone(), 4), (path.clone(), 7)]);

        assert_eq!(coalesced, vec![(path, 11)]);
    }

    #[test]
    fn test_rename_detection_cannot_obscure_the_destination_scope() {
        let repo = fixture_repo();
        let root = repo.path().to_path_buf();
        git(&root, &["config", "diff.renames", "true"]);
        git(&root, &["mv", "libs/usecase/src/renamed.rs", "libs/domain/src/renamed.rs"]);

        assert_eq!(
            domain_lines(&measure_in(&root)),
            3 + 5,
            "the destination path is counted even when repository config enables rename detection"
        );
    }

    #[test]
    fn test_an_unusable_commit_record_degrades_the_base_to_the_configured_branch() {
        let repo = fixture_repo();
        let root = repo.path().to_path_buf();
        write(&root, "libs/domain/src/unstaged.rs", &"line\n".repeat(9));

        // No commit record at all: the base is the configured branch, so the
        // track branch's own commit is inside the measured range.
        let without_record = domain_lines(&measure_in(&root));
        assert_eq!(without_record, 3 + 4);

        // A malformed record degrades to the same base rather than failing.
        write(&root, "track/items/some-track/.commit_hash", "not-a-commit-hash\n");
        assert_eq!(domain_lines(&measure_in(&root)), without_record);

        // A well-formed hash that is not an ancestor of HEAD degrades too.
        write(
            &root,
            "track/items/some-track/.commit_hash",
            "0123456789abcdef0123456789abcdef01234567\n",
        );
        assert_eq!(domain_lines(&measure_in(&root)), without_record);
    }

    #[test]
    fn test_a_valid_commit_record_is_honoured_from_another_repositorys_working_directory() {
        use usecase::batch_plan::ScopeDiffMeasurePort;

        // The measured repository, holding a record that names its own track-branch
        // tip: an ancestor of its HEAD, so the base is that commit and only the
        // uncommitted work is in range.
        let measured_repo = fixture_repo();
        let measured_root = measured_repo.path().to_path_buf();
        write(&measured_root, "libs/domain/src/unstaged.rs", &"line\n".repeat(9));
        let head = String::from_utf8(
            std::process::Command::new("git")
                .args(["rev-parse", "HEAD"])
                .current_dir(&measured_root)
                .output()
                .unwrap()
                .stdout,
        )
        .unwrap();
        write(&measured_root, "track/items/some-track/.commit_hash", head.trim());

        // A second repository, unrelated to the first, is where the process stands.
        // The record's ancestry must be asked of the repository the record was read
        // from; asking this one would find no such commit and degrade the base.
        //
        // Its history is built from content of its own rather than from the shared
        // fixture: two fixture repositories assembled from identical trees in the
        // same second produce identical commit ids, which would make the stored hash
        // resolvable here and leave the anchoring untested.
        let elsewhere = tempfile::Builder::new()
            .prefix("scope-diff-measure-elsewhere-")
            .tempdir_in(std::env::current_dir().unwrap())
            .unwrap();
        write(elsewhere.path(), "unrelated.txt", "a history of its own\n");
        git(elsewhere.path(), &["init", "-b", "main"]);
        git(elsewhere.path(), &["config", "user.email", "elsewhere@example.com"]);
        git(elsewhere.path(), &["config", "user.name", "elsewhere"]);
        git(elsewhere.path(), &["add", "-A"]);
        git(elsewhere.path(), &["commit", "-m", "unrelated history"]);
        assert!(
            !std::process::Command::new("git")
                .args(["cat-file", "-e", &format!("{}^{{commit}}", head.trim())])
                .current_dir(elsewhere.path())
                .output()
                .unwrap()
                .status
                .success(),
            "the stored commit must not exist in the working-directory repository, or the \
             anchoring is untested"
        );

        let measured = {
            let _guard = CWD_LOCK.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            let original = std::env::current_dir().unwrap();
            std::env::set_current_dir(elsewhere.path()).unwrap();
            let measured = super::GitScopeDiffMeasurer::new().measure_scope_diff(
                &measured_root.join("track/items"),
                &TrackId::try_new("some-track").unwrap(),
            );
            std::env::set_current_dir(original).unwrap();
            measured.unwrap()
        };

        assert_eq!(
            domain_lines(&measured),
            4,
            "the record is honoured, so the committed +3 sits behind the base and only the \
             unstaged +4 is measured"
        );
    }

    #[test]
    fn test_a_scope_figure_is_additions_plus_deletions() {
        let changed = counted_entries(
            parse_numstat(
                b"12\t5\tlibs/domain/src/a.rs\0\
                3\t0\tlibs/domain/src/b.rs\0",
            )
            .unwrap(),
        );

        assert_eq!(changed.len(), 2);
        assert_eq!(changed[0].1, 17, "12 additions + 5 deletions");
        assert_eq!(changed[1].1, 3);

        let measured = accumulate_by_scope(&config(), &changed);
        let domain = measured.iter().find(|diff| diff.scope().to_string() == "domain").unwrap();
        assert_eq!(domain.lines(), LineCount::new(20));
    }

    #[test]
    fn test_a_binary_numstat_record_is_retained_for_safe_counting() {
        let changed = parse_numstat(b"-\t-\tlibs/domain/src/logo.png\0").unwrap();

        assert!(matches!(
            changed.as_slice(),
            [NumstatEntry::Binary(path)] if path.as_str() == "libs/domain/src/logo.png"
        ));
    }

    #[test]
    fn test_an_unreadable_numstat_record_is_refused() {
        assert!(parse_numstat(b"nonsense\0").is_err());
        assert!(parse_numstat(b"x\ty\tlibs/domain/src/a.rs\0").is_err());
    }

    #[test]
    fn test_nul_delimited_numstat_preserves_a_path_containing_a_newline() {
        let changed =
            counted_entries(parse_numstat(b"1\t0\tlibs/domain/src/line\nbreak.rs\0").unwrap());

        assert_eq!(changed.len(), 1);
        assert_eq!(changed[0].0.as_str(), "libs/domain/src/line\nbreak.rs");
        assert_eq!(changed[0].1, 1);
    }

    #[test]
    fn test_the_measurer_satisfies_the_port_and_names_only_figures_it_measured() {
        use std::path::Path;

        use usecase::batch_plan::{ScopeDiffMeasureError, ScopeDiffMeasurePort};

        let measurer = super::GitScopeDiffMeasurer::new();
        let port: &dyn ScopeDiffMeasurePort = &measurer;

        match port
            .measure_scope_diff(Path::new("track/items"), &TrackId::try_new("some-track").unwrap())
        {
            // A measurement names one figure per scope it found, never a
            // half-state: every entry carries both the scope and its lines.
            Ok(measured) => {
                let mut seen: Vec<String> =
                    measured.iter().map(|diff| diff.scope().to_string()).collect();
                let unique = {
                    seen.sort();
                    seen.dedup();
                    seen.len()
                };
                assert_eq!(unique, measured.len(), "one figure per scope");
            }
            // A failure says why it could not measure rather than reporting zero.
            Err(ScopeDiffMeasureError::MeasureFailed { message }) => {
                assert!(!message.as_str().is_empty(), "a failure names its reason");
            }
        }
    }

    #[test]
    fn test_each_scope_is_summed_from_the_files_the_classification_puts_in_it() {
        let changed = vec![
            (FilePath::new("libs/domain/src/a.rs").unwrap(), 100),
            (FilePath::new("libs/usecase/src/b.rs").unwrap(), 40),
            (FilePath::new("README.md").unwrap(), 9),
        ];

        let measured = accumulate_by_scope(&config(), &changed);

        let figure = |name: &str| {
            measured.iter().find(|diff| diff.scope().to_string() == name).map(|diff| diff.lines())
        };
        assert_eq!(figure("domain"), Some(LineCount::new(100)));
        assert_eq!(figure("usecase"), Some(LineCount::new(40)));
        // The unmatched file lands in the implicit `other` scope.
        assert_eq!(figure("other"), Some(LineCount::new(9)));
    }
}
