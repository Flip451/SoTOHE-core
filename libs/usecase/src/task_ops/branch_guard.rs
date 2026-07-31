//! The branch guard every track mutation passes before it writes.
//!
//! Part of the parent module's use case; a separate file only so that neither
//! module outgrows the workspace module-size limit. The logic is unchanged from
//! when it sat inline.

use std::sync::Arc;

use domain::{TrackId, TrackReader};

use super::TaskOperationError;
use crate::track_resolution::{BranchReadError, BranchReaderPort};

/// Enforces the branch guard for track mutation operations.
///
/// When a `branch_reader` port is provided:
/// - Branchless tracks (`branch = None` in metadata) pass the guard unconditionally.
/// - Tracks with a branch require the current git branch (returned by
///   [`BranchReaderPort::current_branch`]) to match the expected branch;
///   detached HEAD state is rejected.
/// - When the current branch does not match, returns
///   [`TaskOperationError::BranchGuardFailed`].
/// - When the HEAD is detached (reader returns `Some("HEAD")`), returns
///   [`TaskOperationError::BranchlessGuardFailed`].
///
/// When no `branch_reader` is provided, the guard is a no-op.
///
/// # Errors
///
/// Returns [`TaskOperationError::BranchGuardFailed`] when the branch does not match.
/// Returns [`TaskOperationError::BranchlessGuardFailed`] for detached HEAD state.
pub(super) fn enforce_branch_guard<R: TrackReader>(
    store: &R,
    track_id: &TrackId,
    _items_dir: &std::path::Path,
    branch_reader: Option<&Arc<dyn BranchReaderPort>>,
) -> Result<(), TaskOperationError> {
    let Some(reader) = branch_reader else {
        return Ok(()); // no reader injected — skip guard
    };

    // Read track metadata to determine expected branch.
    let track = store
        .find(track_id)
        .map_err(TaskOperationError::from)?
        .ok_or_else(|| TaskOperationError::TrackNotFound(track_id.to_string()))?;

    let expected_branch = match track.branch() {
        None => return Ok(()), // branchless track — skip guard
        Some(b) => b.as_ref().to_owned(),
    };

    // Delegate branch reading to the injected port.
    let actual_branch_opt =
        reader.current_branch().map_err(|BranchReadError::ReadFailed(msg)| {
            TaskOperationError::BranchGuardFailed(format!("branch read failed: {msg}"))
        })?;

    let actual_branch = match actual_branch_opt {
        Some(b) => b,
        None => {
            return Err(TaskOperationError::BranchGuardFailed(
                "branch read returned no branch name".to_owned(),
            ));
        }
    };

    // Detached HEAD → ambiguous branch state.
    if actual_branch == "HEAD" {
        return Err(TaskOperationError::BranchlessGuardFailed(format!(
            "detached HEAD — expected branch '{expected_branch}', cannot verify"
        )));
    }

    // Branch mismatch → guard fails.
    if actual_branch != expected_branch {
        return Err(TaskOperationError::BranchGuardFailed(format!(
            "current branch '{actual_branch}' does not match expected '{expected_branch}'"
        )));
    }

    Ok(())
}
