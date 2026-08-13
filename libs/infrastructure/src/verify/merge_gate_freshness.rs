//! Git operations used to validate the freshness of generated merge-gate artifacts.

use std::path::Path;

use domain::{CommitHash, validate_branch_ref};

use crate::git_cli::isolation::isolated_bounded_git_output;

const MAX_BRANCH_COMMIT_BYTES: usize = 8 * 1024;

/// Derives the signal filename for a declaration filename by the same rule
/// as `TdddLayerBinding::signal_file()` (infrastructure/verify/tddd_layers,
/// T003): strip `.json`, drop a trailing `s` if present, append
/// `-signals.json`. This keeps the signal-path binding next to the branch
/// evaluation-commit freshness check.
pub(super) fn signal_file_name_for(catalogue_filename: &str) -> String {
    let stem = catalogue_filename.strip_suffix(".json").unwrap_or(catalogue_filename);
    let signal_stem = if let Some(trimmed) = stem.strip_suffix('s') {
        format!("{trimmed}-signals")
    } else {
        format!("{stem}-signals")
    };
    format!("{signal_stem}.json")
}

/// Resolves the commit against which the branch's committed signal artifact
/// was evaluated. Signal generation records the checked-out HEAD before the
/// generated artifact is committed, so the branch tip's first parent is the
/// evaluation commit for a committed signal file.
pub(super) fn read_branch_evaluation_commit(
    repo_root: &Path,
    branch: &str,
) -> Result<CommitHash, String> {
    validate_branch_ref(branch).map_err(|error| format!("invalid branch ref: {error}"))?;
    let evaluation_revision = format!("origin/{branch}^1^{{commit}}");
    resolve_revision(repo_root, &evaluation_revision, "branch evaluation commit")
}

/// Resolves the tip signal artifact's object ID and requires a regular file.
pub(super) fn read_branch_signal_blob_id(
    repo_root: &Path,
    branch: &str,
    signal_path: &str,
) -> Result<String, String> {
    validate_branch_ref(branch).map_err(|error| format!("invalid branch ref: {error}"))?;
    let revision = format!("origin/{branch}");
    read_signal_blob_id_at_revision(repo_root, &revision, signal_path)?
        .ok_or_else(|| format!("signal artifact {signal_path} is absent at branch tip {revision}"))
}

/// Checks whether the signal artifact's blob changed in the branch tip commit.
///
/// The tip object ID comes from the blob already fetched for decoding. Only
/// the parent tree is inspected here, so the predicate never scans unrelated
/// paths and never fetches the tip blob a second time.
pub(super) fn signal_artifact_changed_in_tip(
    repo_root: &Path,
    branch: &str,
    signal_path: &str,
    tip_object_id: &str,
) -> Result<bool, String> {
    validate_branch_ref(branch).map_err(|error| format!("invalid branch ref: {error}"))?;
    let parent = format!("origin/{branch}^1");
    let parent_object_id = read_signal_blob_id_at_revision(repo_root, &parent, signal_path)?;
    Ok(parent_object_id.as_deref() != Some(tip_object_id))
}

fn read_signal_blob_id_at_revision(
    repo_root: &Path,
    revision: &str,
    signal_path: &str,
) -> Result<Option<String>, String> {
    let output = isolated_bounded_git_output(
        repo_root,
        &["ls-tree", "-z", revision, "--", signal_path],
        MAX_BRANCH_COMMIT_BYTES,
    )
    .map_err(|error| format!("failed to inspect {signal_path} at {revision}: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "git ls-tree failed for {signal_path} at {revision} (exit {}): {}",
            output.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let mut entries = output.stdout.split(|byte| *byte == 0).filter(|entry| !entry.is_empty());
    let Some(entry) = entries.next() else {
        return Ok(None);
    };
    if entries.next().is_some() {
        return Err(format!("git ls-tree returned multiple entries for {signal_path}"));
    }
    let tab = entry
        .iter()
        .position(|byte| *byte == b'\t')
        .ok_or_else(|| format!("git ls-tree returned malformed metadata for {signal_path}"))?;
    let (metadata, entry_path_with_tab) = entry.split_at(tab);
    let entry_path = entry_path_with_tab
        .get(1..)
        .ok_or_else(|| format!("git ls-tree returned malformed path for {signal_path}"))?;
    if entry_path != signal_path.as_bytes() {
        return Err(format!("git ls-tree returned an unexpected path at {revision}"));
    }
    let mut fields = metadata.splitn(3, |byte| *byte == b' ');
    let mode = std::str::from_utf8(fields.next().unwrap_or_default())
        .map_err(|error| format!("git ls-tree returned invalid mode: {error}"))?;
    let entry_kind = std::str::from_utf8(fields.next().unwrap_or_default())
        .map_err(|error| format!("git ls-tree returned invalid entry kind: {error}"))?;
    let object_id = std::str::from_utf8(fields.next().unwrap_or_default())
        .map_err(|error| format!("git ls-tree returned invalid object id: {error}"))?;
    if !matches!(mode, "100644" | "100755") || entry_kind != "blob" {
        return Err(format!(
            "signal artifact at {revision} is not a regular file (mode {mode}, kind {entry_kind})"
        ));
    }
    if object_id.is_empty()
        || object_id.len() > 128
        || !object_id.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(format!("git ls-tree returned an invalid object id at {revision}"));
    }
    Ok(Some(object_id.to_owned()))
}

fn resolve_revision(
    repo_root: &Path,
    revision: &str,
    description: &str,
) -> Result<CommitHash, String> {
    let args = ["rev-parse", "--verify", revision];
    let output = isolated_bounded_git_output(repo_root, &args, MAX_BRANCH_COMMIT_BYTES)
        .map_err(|error| format!("failed to resolve {description}: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "git rev-parse failed for {description} (exit {}): {}",
            output.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let commit = std::str::from_utf8(&output.stdout)
        .map_err(|error| format!("{description} is not UTF-8: {error}"))?
        .trim();
    CommitHash::try_new(commit.to_owned())
        .map_err(|error| format!("{description} is invalid: {error}"))
}
