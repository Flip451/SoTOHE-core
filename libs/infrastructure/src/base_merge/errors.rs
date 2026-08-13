use usecase::base_merge::{BaseMergeContextError, BaseMergeGitError};
use usecase::git_workflow::DiagnosticText;

pub(super) fn context_unavailable(detail: &'static str) -> BaseMergeContextError {
    BaseMergeContextError::Unavailable(DiagnosticText::new(detail))
}

pub(super) fn git_execution_error(detail: impl Into<String>) -> BaseMergeGitError {
    BaseMergeGitError::Execution(DiagnosticText::new(detail))
}
