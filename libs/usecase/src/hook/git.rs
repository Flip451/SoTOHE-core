//! Process-level git hook handlers.
//!
//! These handlers do not parse shell commands. They only consume the
//! `guarded_git_token_present` flag injected by the CLI composition root.

use domain::hook::{HookContext, HookError, HookInput, HookVerdict};

use super::HookHandler;

/// Remediation shown when a git hook runs outside the guarded sotp path.
pub(crate) const GUARDED_GIT_HOOK_REMEDIATION_MESSAGE: &str = "[Git Policy] Direct git ref updates \
     and pushes are blocked unless they run through the guarded sotp git wrappers. Use the \
     appropriate sotp wrapper (`cargo make track-add-paths`, `cargo make track-commit-message`, \
     `cargo make track-branch-create`, `cargo make track-branch-switch`, or \
     `cargo make track-pr-push`) or ask the user to perform the git operation manually.";

/// Hook handler for `git-ref-update`.
pub struct GitRefUpdateHandler {
    pub(crate) guarded_git_token_present: bool,
}

impl HookHandler for GitRefUpdateHandler {
    fn handle(&self, _ctx: &HookContext, _input: &HookInput) -> Result<HookVerdict, HookError> {
        Ok(guarded_git_verdict(self.guarded_git_token_present))
    }
}

/// Hook handler for `git-pre-push`.
pub struct GitPrePushHandler {
    pub(crate) guarded_git_token_present: bool,
}

impl HookHandler for GitPrePushHandler {
    fn handle(&self, _ctx: &HookContext, _input: &HookInput) -> Result<HookVerdict, HookError> {
        Ok(guarded_git_verdict(self.guarded_git_token_present))
    }
}

fn guarded_git_verdict(guarded_git_token_present: bool) -> HookVerdict {
    if guarded_git_token_present {
        HookVerdict::allow()
    } else {
        HookVerdict::block(GUARDED_GIT_HOOK_REMEDIATION_MESSAGE)
    }
}
