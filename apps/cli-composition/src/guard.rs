//! `guard` command family — per-context composition root and CliApp shim.

use std::sync::Arc;

use crate::{CommandOutcome, error::CompositionError};

// ---------------------------------------------------------------------------
// Per-context composition root
// ---------------------------------------------------------------------------

/// Composition root for the `guard` command family.
///
/// Unit struct: no adapter dependencies are injected at construction time.
pub struct GuardCompositionRoot;

impl GuardCompositionRoot {
    /// Create a new `GuardCompositionRoot`.
    pub fn new() -> Self {
        Self
    }

    /// Check a shell command against the guard policy.
    ///
    /// Returns a JSON verdict (`{"decision":"allow"|"block","reason":"..."}`) in stdout.
    /// Exit code 0 = allow, non-zero = block.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails.
    pub fn guard_check(&self, command: String) -> Result<CommandOutcome, CompositionError> {
        use infrastructure::shell::ConchShellParser;
        use usecase::hook_dispatch::{
            HookDispatchCommand, HookDispatchInteractor, HookDispatchService, HookVerdictDecision,
        };

        let parser_port = Arc::new(ConchShellParser);
        let service = HookDispatchInteractor::new(parser_port, None, false, true);

        let dispatch_cmd = HookDispatchCommand {
            tool_name: "Bash".to_owned(),
            command: Some(command),
            file_path: None,
            content: None,
            git_hook_args: vec![],
        };

        let (decision_str, reason, is_blocked) =
            match service.dispatch("block-direct-git-ops".to_owned(), dispatch_cmd) {
                Ok(output) => {
                    let blocked = output.decision == HookVerdictDecision::Block;
                    let reason = output.reason.unwrap_or_default();
                    let decision_str = if blocked { "block" } else { "allow" };
                    (decision_str, reason, blocked)
                }
                Err(err) => ("block", format!("dispatch error: {err}"), true),
            };

        let json = serde_json::json!({
            "decision": decision_str,
            "reason": reason,
        });

        let stdout = json.to_string();
        let exit_code: u8 = if is_blocked { 1 } else { 0 };
        Ok(CommandOutcome { stdout: Some(stdout), stderr: None, exit_code })
    }
}

impl Default for GuardCompositionRoot {
    fn default() -> Self {
        Self::new()
    }
}

impl GuardCompositionRoot {
    /// Build a wired [`cli_driver::guard::GuardDriver`] for the guard family.
    pub fn guard_driver(&self) -> cli_driver::guard::GuardDriver {
        use infrastructure::shell::ConchShellParser;
        use usecase::guard::GuardCheckInteractor;

        let parser_port = Arc::new(ConchShellParser);
        let service = Arc::new(GuardCheckInteractor::new(parser_port));
        cli_driver::guard::GuardDriver::new(service)
    }
}
