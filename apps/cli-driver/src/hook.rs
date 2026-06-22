// STAGED FOR T021 — not yet compiled; Cargo.toml + workspace member added atomically in T021 per CN-06.
//
//! `hook` command family — primary adapter driver.
//!
//! `HookDriver` holds injected use-case interactors and exposes
//! `handle(input) -> CommandOutcome`.  The JSON formatting helpers here
//! mirror those in `apps/cli-composition/src/hook.rs` (lines ~263-270);
//! T021 removes the `cli_composition` duplicate when the live path is flipped.

// TODO(T021): add use-case + infrastructure imports once Cargo.toml is materialized.
// use std::sync::Arc;
// use usecase::hook_dispatch::{
//     HookDispatchCommand, HookDispatchInteractor, HookDispatchService, HookVerdictDecision,
// };
// use infrastructure::shell::ConchShellParser;

use crate::render::CommandOutcome;

// ---------------------------------------------------------------------------
// Input types
// ---------------------------------------------------------------------------

/// Known hook names for the `hook dispatch` subcommand.
#[derive(Debug, Clone)]
pub enum HookName {
    /// Preflight: require local git hooks setup before Bash execution.
    HooksPathSetup,
    /// Guard: block direct git operations.
    BlockDirectGitOps,
    /// Guard: block `rm` commands targeting test files (PreToolUse).
    BlockTestFileDeletion,
    /// Process-level git hook: reference transaction.
    GitRefUpdate,
    /// Process-level git hook: pre-push.
    GitPrePush,
    /// Advisory: skill compliance check for UserPromptSubmit.
    SkillCompliance,
}

impl HookName {
    /// Returns the hook name string used by the dispatch service.
    pub fn hook_name(&self) -> &'static str {
        match self {
            Self::HooksPathSetup => "hooks-path-setup",
            Self::BlockDirectGitOps => "block-direct-git-ops",
            Self::BlockTestFileDeletion => "block-test-file-deletion",
            Self::GitRefUpdate => "git-ref-update",
            Self::GitPrePush => "git-pre-push",
            Self::SkillCompliance => "skill-compliance",
        }
    }

    /// Returns whether this hook accepts positional git hook arguments.
    pub fn accepts_git_hook_args(&self) -> bool {
        matches!(self, Self::GitRefUpdate | Self::GitPrePush)
    }
}

/// Typed input for the `hook` command family.
pub enum HookInput {
    /// Dispatch a security-critical hook via Rust logic.
    Dispatch {
        /// The hook to dispatch.
        hook: HookName,
        /// Positional arguments supplied by git process hooks.
        git_hook_args: Vec<String>,
    },
}

// ---------------------------------------------------------------------------
// Driver
// ---------------------------------------------------------------------------

/// Primary adapter driver for the `hook` command family.
///
/// Holds injected use-case interactors; exposes `handle(input) -> CommandOutcome`.
pub struct HookDriver {
    // TODO(T021): inject use-case interactors here.
    // hook_dispatch_service: Arc<dyn usecase::hook_dispatch::HookDispatchService>,
    // skill_compliance_service: Arc<dyn usecase::skill_compliance::SkillComplianceService>,
}

impl HookDriver {
    /// Create a new `HookDriver`.
    ///
    /// TODO(T021): accept injected interactors as parameters once the crate
    /// dependency graph is materialized.
    pub fn new() -> Self {
        Self {}
    }

    /// Handle a hook command.
    ///
    /// Exit code 0 = allow, exit code 2 = block (Claude Code hook protocol).
    /// PreToolUse hooks: any internal error → exit code 2 (fail-closed).
    ///
    /// TODO(T021): wire real use-case invocation once Cargo.toml is materialized.
    pub fn handle(&self, input: HookInput) -> CommandOutcome {
        match input {
            HookInput::Dispatch { hook, git_hook_args } => self.hook_dispatch(hook, git_hook_args),
        }
    }

    // -----------------------------------------------------------------------
    // Internal dispatch helpers
    // -----------------------------------------------------------------------

    fn hook_dispatch(&self, hook: HookName, git_hook_args: Vec<String>) -> CommandOutcome {
        if !git_hook_args.is_empty() && !hook.accepts_git_hook_args() {
            return CommandOutcome {
                stdout: None,
                stderr: Some(
                    "extra hook arguments are only supported for git process hooks".to_owned(),
                ),
                exit_code: 2,
            };
        }

        // TODO(T021): invoke HookDispatchInteractor here.
        // Placeholder: mirrors the allow path produced by cli_composition::HookCompositionRoot.
        CommandOutcome::success(None)
    }

    // -----------------------------------------------------------------------
    // JSON formatting helpers (duplicated from cli_composition/src/hook.rs
    // lines ~263-270; T021 removes the cli_composition copy).
    // -----------------------------------------------------------------------

    /// Build the UserPromptSubmit JSON output for the skill-compliance hook.
    ///
    /// Mirrors cli_composition/src/hook.rs lines ~263-270.
    pub fn render_skill_compliance_output(&self, additional_context: &str) -> String {
        // JSON formatting — mirrors cli_composition/src/hook.rs lines ~263-270.
        serde_json::json!({
            "hookSpecificOutput": {
                "hookEventName": "UserPromptSubmit",
                "additionalContext": additional_context,
            }
        })
        .to_string()
    }
}

impl Default for HookDriver {
    fn default() -> Self {
        Self::new()
    }
}
