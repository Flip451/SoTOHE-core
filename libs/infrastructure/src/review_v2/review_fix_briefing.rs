//! Trusted briefing loading at the review-fix provider boundary.

use std::path::Component;

use usecase::review_v2::run_review_fix::{ReviewFixRunnerError, RunReviewFixCommand};

pub(crate) const MAX_BRIEFING_BYTES: u64 = 64 * 1024;

/// Reads a briefing only after validating it as a bounded regular file below the resolved root.
pub(crate) fn read_trusted_briefing(
    command: &RunReviewFixCommand,
) -> Result<String, ReviewFixRunnerError> {
    let briefing_file = command.briefing_file();
    if briefing_file.is_absolute()
        || briefing_file.components().any(|component| matches!(component, Component::ParentDir))
    {
        return Err(briefing_read_error(
            "review-fix briefing file must be a relative path beneath the repository root",
        ));
    }

    let repository_root = command.repository_root();
    let briefing_path = repository_root.join(briefing_file);
    crate::track::symlink_guard::reject_symlinks_below(&briefing_path, repository_root).map_err(
        |error| briefing_read_error(&format!("review-fix briefing file is not trusted: {error}")),
    )?;

    crate::trusted_file::read_bounded_regular_file(
        &briefing_path,
        repository_root,
        MAX_BRIEFING_BYTES,
    )
    .map_err(|error| briefing_read_error(&format!("failed to read review-fix briefing: {error}")))?
    .ok_or_else(|| briefing_read_error("failed to read review-fix briefing: file does not exist"))
}

fn briefing_read_error(message: &str) -> ReviewFixRunnerError {
    ReviewFixRunnerError::Unexpected(usecase::git_workflow::DiagnosticText::new(message))
}
