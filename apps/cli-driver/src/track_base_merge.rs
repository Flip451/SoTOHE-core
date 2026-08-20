//! Base-merge rendering for [`crate::track::TrackDriver`].

use usecase::base_merge::BaseMergeOutcome;

use crate::render::CommandOutcome;

/// Render the application-service result of a guarded base merge.
pub(crate) fn render_base_merge_result(
    result: Result<BaseMergeOutcome, usecase::base_merge::BaseMergeError>,
) -> CommandOutcome {
    const CONFLICT_RECOVERY_HANDOFF: &str = "continue with the recover workflow (/track:recover on Claude Code, $track-recover on Codex)";

    match result {
        Ok(BaseMergeOutcome::Completed) => {
            CommandOutcome::success(Some("base merge completed".to_owned()))
        }
        Ok(BaseMergeOutcome::Conflicted) => CommandOutcome::failure(Some(format!(
            "base merge conflicted; {CONFLICT_RECOVERY_HANDOFF}"
        ))),
        Err(usecase::base_merge::BaseMergeError::ConflictedCleanupFailed(error)) => {
            CommandOutcome::failure(Some(format!(
                "base merge conflicted; cleanup failed: {error}; {CONFLICT_RECOVERY_HANDOFF}"
            )))
        }
        Err(error) => CommandOutcome::failure(Some(format!("base merge failed: {error}"))),
    }
}
