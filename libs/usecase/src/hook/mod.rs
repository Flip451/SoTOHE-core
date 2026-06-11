//! Hook dispatch use cases (OCP: each hook implements `HookHandler` independently).

pub mod git;
pub mod guard;
pub mod test_file_deletion;

pub use git::{GitPrePushHandler, GitRefUpdateHandler};
pub use guard::GuardHookHandler;
pub use test_file_deletion::TestFileDeletionGuardHandler;

use domain::hook::{HookContext, HookError, HookInput, HookName, HookVerdict};

/// Trait implemented by each hook handler (Open/Closed Principle).
pub trait HookHandler: Send + Sync {
    fn handle(&self, ctx: &HookContext, input: &HookInput) -> Result<HookVerdict, HookError>;
}

/// Dispatch the named hook to the given handler.
pub fn dispatch(
    _name: HookName,
    handler: &dyn HookHandler,
    ctx: &HookContext,
    input: &HookInput,
) -> Result<HookVerdict, HookError> {
    handler.handle(ctx, input)
}

#[cfg(test)]
pub(crate) mod test_support {
    use domain::guard::SimpleCommand;

    pub(crate) fn simple_command(argv: &[&str]) -> SimpleCommand {
        SimpleCommand {
            argv: argv.iter().map(|arg| (*arg).to_string()).collect(),
            redirect_texts: Vec::new(),
            output_redirect_texts: Vec::new(),
            has_output_redirect: false,
        }
    }
}
