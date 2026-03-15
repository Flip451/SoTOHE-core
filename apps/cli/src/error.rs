//! CLI error type that unifies errors from all layers.

use std::process::ExitCode;

use thiserror::Error;

/// Unified error type for CLI commands.
///
/// Each variant wraps an error from a specific layer or represents
/// a CLI-specific failure. The `Display` impl (via thiserror) produces
/// user-facing messages suitable for `eprintln!`.
#[derive(Debug, Error)]
pub enum CliError {
    #[error("{0}")]
    Domain(#[from] domain::DomainError),

    #[error("{0}")]
    TrackRead(#[from] domain::TrackReadError),

    #[error("{0}")]
    TrackWrite(#[from] domain::TrackWriteError),

    #[error("{0}")]
    Repository(#[from] domain::RepositoryError),

    #[error("{0}")]
    Git(#[from] infrastructure::git_cli::GitError),

    #[error("{0}")]
    Gh(#[from] infrastructure::gh_cli::GhError),

    #[error("{0}")]
    Worktree(#[from] domain::WorktreeError),

    #[error("{0}")]
    TrackResolution(#[from] usecase::track_resolution::TrackResolutionError),

    #[error("{0}")]
    GitWorkflow(#[from] usecase::git_workflow::GitWorkflowError),

    #[error("{0}")]
    PrWorkflow(#[from] usecase::pr_workflow::PrWorkflowError),

    #[error("{0}")]
    WorktreeGuard(#[from] usecase::worktree_guard::WorktreeGuardError),

    #[error("{0}")]
    ReviewWorkflow(#[from] usecase::review_workflow::ReviewWorkflowError),

    /// Generic message for errors that don't fit a specific variant.
    #[error("{0}")]
    Message(String),

    /// I/O errors from the CLI layer itself.
    #[error("{0}")]
    Io(#[from] std::io::Error),
}

impl CliError {
    /// Converts this error into an `ExitCode`.
    ///
    /// All errors map to `ExitCode::FAILURE` (exit code 1).
    #[must_use]
    pub fn exit_code(&self) -> ExitCode {
        ExitCode::FAILURE
    }
}
