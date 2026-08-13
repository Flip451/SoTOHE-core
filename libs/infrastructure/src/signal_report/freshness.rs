//! Git-backed freshness validation for persisted implementation-catalogue signals.

use std::path::Path;

use domain::CommitHash;
use usecase::signal_report::{SignalReportChain, SignalReportError};

use crate::capability_exec::MAX_CAPABILITY_EXEC_TEXT_BYTES;
use crate::git_cli::isolated_bounded_git_output;

const MAX_SIGNAL_TREE_OUTPUT_BYTES: usize = 8 * 1024;

/// Validates a worktree implementation-catalogue signal against the current
/// commit and the immediately preceding evaluation commit.
pub(super) fn validate_impl_catalog_freshness(
    root: &Path,
    signal_path: &Path,
    recorded_head: &CommitHash,
    worktree_bytes: &[u8],
) -> Result<(), SignalReportError> {
    let unavailable = || SignalReportError::SourceUnavailable(SignalReportChain::ImplCatalog);
    let current_head = resolve_freshness_commit(root, "HEAD").ok_or_else(unavailable)?;
    if recorded_head == &current_head {
        return Ok(());
    }

    let evaluation_head = resolve_freshness_commit(root, "HEAD^1").ok_or_else(unavailable)?;
    if recorded_head != &evaluation_head {
        return Err(unavailable());
    }

    let relative_signal_path =
        signal_path.strip_prefix(root).ok().and_then(Path::to_str).ok_or_else(unavailable)?;
    let Some((head_object_id, head_bytes)) =
        read_signal_blob_at_revision(root, "HEAD", relative_signal_path, true)
            .map_err(|_| unavailable())?
    else {
        return Err(unavailable());
    };
    let parent_object_id =
        read_signal_blob_at_revision(root, "HEAD^1", relative_signal_path, false)
            .map_err(|_| unavailable())?
            .map(|(object_id, _)| object_id);
    if parent_object_id.as_deref() == Some(head_object_id.as_str()) {
        return Err(unavailable());
    }
    if head_bytes.as_deref() != Some(worktree_bytes) {
        return Err(unavailable());
    }
    Ok(())
}

fn resolve_freshness_commit(root: &Path, revision: &str) -> Option<CommitHash> {
    let output = isolated_bounded_git_output(
        root,
        &["rev-parse", "--verify", revision],
        MAX_SIGNAL_TREE_OUTPUT_BYTES,
    )
    .ok()?;
    if !output.status.success() {
        return None;
    }
    let commit = std::str::from_utf8(&output.stdout).ok()?.trim();
    CommitHash::try_new(commit.to_owned()).ok()
}

/// A blob's object id plus, when requested, its bytes.
type SignalBlobEntry = (String, Option<Vec<u8>>);

fn read_signal_blob_at_revision(
    root: &Path,
    revision: &str,
    relative_signal_path: &str,
    include_bytes: bool,
) -> Result<Option<SignalBlobEntry>, String> {
    let output = isolated_bounded_git_output(
        root,
        &["ls-tree", "-z", revision, "--", relative_signal_path],
        MAX_SIGNAL_TREE_OUTPUT_BYTES,
    )
    .map_err(|error| format!("failed to inspect {relative_signal_path} at {revision}: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "git ls-tree failed for {relative_signal_path} at {revision} (exit {}): {}",
            output.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    let mut entries = output.stdout.split(|byte| *byte == 0).filter(|entry| !entry.is_empty());
    let Some(entry) = entries.next() else {
        return Ok(None);
    };
    if entries.next().is_some() {
        return Err(format!(
            "git ls-tree returned multiple entries for {relative_signal_path} at {revision}"
        ));
    }
    let tab = entry.iter().position(|byte| *byte == b'\t').ok_or_else(|| {
        format!("git ls-tree returned malformed metadata for {relative_signal_path}")
    })?;
    let (metadata, entry_path_with_tab) = entry.split_at(tab);
    let entry_path = entry_path_with_tab
        .get(1..)
        .ok_or_else(|| format!("git ls-tree returned malformed path for {relative_signal_path}"))?;
    if entry_path != relative_signal_path.as_bytes() {
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
    let bytes = if include_bytes {
        let git_ref = format!("{revision}:{relative_signal_path}");
        let output = isolated_bounded_git_output(
            root,
            &["show", &git_ref],
            MAX_CAPABILITY_EXEC_TEXT_BYTES as usize,
        )
        .map_err(|error| format!("failed to read {relative_signal_path} at {revision}: {error}"))?;
        if !output.status.success() {
            return Err(format!(
                "git show failed for {relative_signal_path} at {revision} (exit {}): {}",
                output.status.code().unwrap_or(-1),
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
        Some(output.stdout)
    } else {
        None
    };
    Ok(Some((object_id.to_owned(), bytes)))
}
