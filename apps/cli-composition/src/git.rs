//! `git` command family — per-context composition root.
//!
//! Post-cutover (track `remote-sync-dedicated-command-2026-07-04`): all inline
//! git-family methods have been deleted; the composition root's only job is to
//! wire the [`cli_driver::git::GitDriver`] over the usecase-layer
//! [`usecase::git_workflow::GitWorkflowInteractor`] composed with the atomic
//! [`usecase::git_workflow::GitPrimitivePort`] adapter. AC-03 / AC-06.

use std::sync::Arc;

// ---------------------------------------------------------------------------
// Per-context composition root
// ---------------------------------------------------------------------------

/// Composition root for the `git` command family.
///
/// Unit struct: no adapter dependencies are injected at construction time.
pub struct GitCompositionRoot;

impl GitCompositionRoot {
    /// Create a new `GitCompositionRoot`.
    pub fn new() -> Self {
        Self
    }
}

impl Default for GitCompositionRoot {
    fn default() -> Self {
        Self::new()
    }
}

impl GitCompositionRoot {
    /// Build a wired [`cli_driver::git::GitDriver`] for the git family.
    pub fn git_driver(&self) -> cli_driver::git::GitDriver {
        use infrastructure::FsGitWorkflowAdapter;
        use usecase::git_workflow::{GitPrimitivePort, GitWorkflowInteractor};

        let port: Arc<dyn GitPrimitivePort> = Arc::new(FsGitWorkflowAdapter::new());
        let service = Arc::new(GitWorkflowInteractor::new(port));
        cli_driver::git::GitDriver::new(service)
    }
}
