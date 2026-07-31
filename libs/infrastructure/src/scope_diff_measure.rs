//! Git secondary adapter measuring a track's per-scope diff
//! (IN-10, IN-14, IN-15, AC-17, CN-05, OUT-08).
//!
//! [`GitScopeDiffMeasurer`] owns the line-count definition that used to live in
//! workflow prose:
//!
//! - a scope's figure is `additions + deletions`,
//! - measured from the batch base over the committed, staged and unstaged
//!   changes in one `git diff <base>` pass,
//! - untracked files count their whole length as additions,
//! - and files are attributed to scopes by the existing review-scope
//!   classification, which this adapter reuses rather than reimplements.
//!
//! The base itself is resolved by the existing per-track diff-base resolver:
//! the track's recorded commit hash, degrading to the configured base branch
//! when that record is missing, malformed or no longer an ancestor (CN-05).

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
use crate::git_cli::{SystemGitRepo, guarded_git_command};
use crate::review_scope_config_reader::REVIEW_SCOPE_CONFIG;
use crate::review_v2::load_v2_scope_config;
use crate::track::symlink_guard::reject_symlinks_below;
use crate::track_artifact::{TrackArtifactReadError, read_track_artifact};

const METADATA_FILE: &str = "metadata.json";
const MAX_TRACK_METADATA_BYTES: u64 = 4 * 1024 * 1024;
const MAX_GIT_OUTPUT_BYTES: u64 = 16 * 1024 * 1024;
const MAX_UNTRACKED_FILE_BYTES: u64 = 16 * 1024 * 1024;

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
        let repo = crate::discover_repo_for_items_dir(items_dir)
            .map_err(|error| measure_failed(format!("git repository not discovered: {error}")))?;
        let root = repo.root().to_path_buf();
        let items_dir = resolve_items_dir_under(items_dir, &root)?;
        let track_dir = items_dir.join(track_id.as_ref());

        let base = resolve_base(&root, &items_dir, track_id, &track_dir)?;
        let scope_config =
            load_v2_scope_config(&root.join(REVIEW_SCOPE_CONFIG), track_id, &root)
                .map_err(|error| measure_failed(format!("load {REVIEW_SCOPE_CONFIG}: {error}")))?;

        // Rename detection may be enabled by repository configuration. Turn it
        // off explicitly so every changed path has one unambiguous numstat row
        // and can be classified by its real repository-relative location.
        let numstat =
            git_bytes(&repo, &["diff", "--no-renames", "--numstat", "-z", base.as_ref(), "--"])?;
        let untracked = git_bytes(&repo, &["ls-files", "-z", "--others", "--exclude-standard"])?;

        let mut changed = parse_numstat(&numstat)?;
        let untracked = retained_paths(&scope_config, untracked_paths(&untracked)?);
        changed.extend(untracked_additions(&root, &untracked)?);

        Ok(accumulate_by_scope(&scope_config, &coalesce_by_path(changed)))
    }
}

/// Resolves the items directory against the repository it was discovered from,
/// refusing one that resolves outside it: a measurement only means something
/// for a track inside the repository being measured.
fn resolve_items_dir_under(
    items_dir: &Path,
    root: &Path,
) -> Result<std::path::PathBuf, ScopeDiffMeasureError> {
    let canonical = items_dir
        .canonicalize()
        .map_err(|error| measure_failed(format!("items_dir rejected: {error}")))?;
    let canonical_root = root
        .canonicalize()
        .map_err(|error| measure_failed(format!("canonicalize repository root: {error}")))?;
    if !canonical.starts_with(&canonical_root) {
        return Err(measure_failed(format!(
            "items_dir {} resolves outside the repository {}",
            canonical.display(),
            canonical_root.display()
        )));
    }
    Ok(canonical)
}

/// Resolves the batch base: the track's recorded commit hash, or the base branch
/// captured in the track's immutable metadata snapshot when that record cannot
/// be used (CN-05).
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
                TrackArtifactReadError::Failed(message) => measure_failed(format!(
                    "read {METADATA_FILE} for track '{}': {message}",
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
    let canonical_root = root
        .canonicalize()
        .map_err(|error| measure_failed(format!("canonicalize repository root: {error}")))?;

    FsDiffBaseResolverAdapter::new(metadata.branch_strategy_snapshot().base_branch().to_owned())
        .resolve_diff_base(track_dir, &canonical_root, root)
        .map_err(|error| measure_failed(format!("resolve diff base: {error}")))
}

fn git_bytes(repo: &SystemGitRepo, args: &[&str]) -> Result<Vec<u8>, ScopeDiffMeasureError> {
    let mut child = guarded_git_command()
        .args(args)
        .current_dir(repo.root())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| measure_failed(format!("git {}: {error}", args.join(" "))))?;
    let mut output = Vec::new();
    let read_result = child
        .stdout
        .take()
        .ok_or_else(|| measure_failed(format!("git {} did not provide stdout", args.join(" "))))?
        .take(MAX_GIT_OUTPUT_BYTES.saturating_add(1))
        .read_to_end(&mut output);
    if output.len() as u64 > MAX_GIT_OUTPUT_BYTES {
        let _ = child.kill();
        let _ = child.wait();
        return Err(measure_failed(format!(
            "git {} exceeded maximum output size of {MAX_GIT_OUTPUT_BYTES} bytes",
            args.join(" ")
        )));
    }
    read_result
        .map_err(|error| measure_failed(format!("read git {} output: {error}", args.join(" "))))?;
    let status = child
        .wait()
        .map_err(|error| measure_failed(format!("wait for git {}: {error}", args.join(" "))))?;
    if !status.success() {
        return Err(measure_failed(format!(
            "git {} failed (exit {})",
            args.join(" "),
            status.code().unwrap_or(-1)
        )));
    }
    Ok(output)
}

/// Reads NUL-delimited `git diff --numstat -z` output as one `additions +
/// deletions` figure per file. A binary file reports `-` for both counts and
/// contributes nothing.
fn parse_numstat(output: &[u8]) -> Result<Vec<(FilePath, u32)>, ScopeDiffMeasureError> {
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
            continue;
        }
        let additions = parse_numstat_count(additions, record)?;
        let deletions = parse_numstat_count(deletions, record)?;
        changed.push((file_path_bytes(path)?, additions.saturating_add(deletions)));
    }
    Ok(changed)
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
                    "refusing untracked path {}: {error}",
                    absolute_path.display()
                )));
            }
        }
        let lines = count_file_lines(&absolute_path, &mut remaining_bytes)?;
        changed.push((path.clone(), lines));
    }
    Ok(changed)
}

/// Counts newline-delimited lines without requiring UTF-8 or buffering a whole file.
///
/// The shared byte budget bounds all untracked-file reads. Metadata makes an
/// oversized regular file fail before I/O, while the bounded reader closes the
/// race if that file grows after its size is checked.
fn count_file_lines(path: &Path, remaining_bytes: &mut u64) -> Result<u32, ScopeDiffMeasureError> {
    let path_metadata = std::fs::symlink_metadata(path).map_err(|error| {
        measure_failed(format!("read metadata for untracked file {}: {error}", path.display()))
    })?;
    if !path_metadata.file_type().is_file() {
        return Err(measure_failed(format!(
            "untracked file {} is not a regular file",
            path.display()
        )));
    }

    let file = std::fs::File::open(path).map_err(|error| {
        measure_failed(format!("read untracked file {}: {error}", path.display()))
    })?;
    let metadata = file.metadata().map_err(|error| {
        measure_failed(format!("read metadata for untracked file {}: {error}", path.display()))
    })?;
    if !metadata.is_file() || metadata.len() > *remaining_bytes {
        return Err(measure_failed(format!(
            "untracked file {} exceeds the remaining {remaining_bytes}-byte read budget",
            path.display()
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
            measure_failed(format!("read untracked file {}: {error}", path.display()))
        })?;
        if bytes_read == 0 {
            break;
        }
        bytes_seen = bytes_seen.saturating_add(bytes_read as u64);
        if bytes_seen > budget {
            return Err(measure_failed(format!(
                "untracked file {} exceeds the remaining {budget}-byte read budget",
                path.display()
            )));
        }
        let Some(chunk) = buffer.get(..bytes_read) else {
            return Err(measure_failed(format!(
                "short read from untracked file {}",
                path.display()
            )));
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

/// Merges a path's tracked and untracked contributions before it is classified.
/// A path can legitimately occur in both inventories when a tracked file is
/// deleted or renamed and a new untracked file is created at the old path.
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

/// Attributes each changed file to the scopes the review configuration puts it
/// in and sums their figures. A file in two scopes counts in both, matching the
/// classification's own independent-review rule.
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
        MAX_UNTRACKED_FILE_BYTES, accumulate_by_scope, coalesce_by_path, count_file_lines,
        parse_numstat, retained_paths, untracked_additions, untracked_paths,
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

        assert!(count_file_lines(file.path(), &mut remaining_bytes).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn test_count_file_lines_fifo_returns_error_without_blocking() {
        let directory = tempfile::tempdir().unwrap();
        let fifo = directory.path().join("untracked.rs");
        rustix::fs::mkfifoat(rustix::fs::CWD, &fifo, rustix::fs::Mode::from_raw_mode(0o600))
            .unwrap();
        let mut remaining_bytes = MAX_UNTRACKED_FILE_BYTES;

        let error = count_file_lines(&fifo, &mut remaining_bytes).unwrap_err();

        assert!(matches!(error, usecase::batch_plan::ScopeDiffMeasureError::MeasureFailed { .. }));
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

        assert!(result.is_err(), "untracked symlinks must not be followed");
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
        let changed = parse_numstat(
            b"12\t5\tlibs/domain/src/a.rs\0\
            3\t0\tlibs/domain/src/b.rs\0",
        )
        .unwrap();

        assert_eq!(changed.len(), 2);
        assert_eq!(changed[0].1, 17, "12 additions + 5 deletions");
        assert_eq!(changed[1].1, 3);

        let measured = accumulate_by_scope(&config(), &changed);
        let domain = measured.iter().find(|diff| diff.scope().to_string() == "domain").unwrap();
        assert_eq!(domain.lines(), LineCount::new(20));
    }

    #[test]
    fn test_a_binary_file_contributes_no_countable_lines() {
        let changed = parse_numstat(
            b"-\t-\tlibs/domain/src/logo.png\0\
            7\t1\tlibs/domain/src/a.rs\0",
        )
        .unwrap();

        assert_eq!(changed.len(), 1);
        assert_eq!(changed[0].1, 8);
    }

    #[test]
    fn test_an_unreadable_numstat_record_is_refused() {
        assert!(parse_numstat(b"nonsense\0").is_err());
        assert!(parse_numstat(b"x\ty\tlibs/domain/src/a.rs\0").is_err());
    }

    #[test]
    fn test_nul_delimited_numstat_preserves_a_path_containing_a_newline() {
        let changed = parse_numstat(b"1\t0\tlibs/domain/src/line\nbreak.rs\0").unwrap();

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
