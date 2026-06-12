//! Hook dispatch use case boundary types and service.
//!
//! Provides [`HookDispatchCommand`], [`HookVerdictDecision`], [`HookVerdictOutput`],
//! [`HookDispatchError`], [`HookDispatchService`], and [`HookDispatchInteractor`]
//! so the CLI layer never imports `domain::hook::HookContext`,
//! `domain::hook::HookInput`, or `domain::hook::HookVerdict` directly.

use std::sync::Arc;

use domain::Decision;
use domain::guard::{ParseError, SimpleCommand};
use domain::hook::{HookContext, HookInput, HookVerdict};

use crate::hook::{
    GitPrePushHandler, GitRefUpdateHandler, GuardHookHandler, HookHandler,
    TestFileDeletionGuardHandler,
};

// ---------------------------------------------------------------------------
// Public boundary types
// ---------------------------------------------------------------------------

/// CQRS command object for the hook dispatch use case.
///
/// Carries the raw `tool_name`, optional `command` string, optional `file_path`,
/// and optional `content` string from the Claude Code hook JSON envelope.
/// The CLI parses the envelope into this command so that the hook dispatcher
/// (usecase) never requires `domain::hook::HookInput` to be constructed at the
/// CLI boundary.
#[derive(Debug, Clone)]
pub struct HookDispatchCommand {
    /// The name of the tool being invoked (always present in hook envelope).
    pub tool_name: String,
    /// The shell command (for guard hooks).
    pub command: Option<String>,
    /// The file path (used by the Write tool for test-file deletion guard).
    pub file_path: Option<std::path::PathBuf>,
    /// The file content (for Write tool: used by test-file deletion guard to detect
    /// empty-content writes, which are equivalent to file truncation/deletion).
    pub content: Option<String>,
}

/// Enum representing the hook verdict outcome at the usecase boundary.
///
/// Mirrors `domain::hook::HookVerdict` (Allow/Block) without exposing the
/// domain type. Used by [`HookVerdictOutput`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookVerdictDecision {
    /// The operation is allowed to proceed.
    Allow,
    /// The operation is blocked.
    Block,
}

/// DTO returned by the hook dispatch service to the CLI.
///
/// Wraps the verdict outcome ([`HookVerdictDecision`]) and a reason string
/// without exposing `domain::hook::HookVerdict` across the usecase boundary.
/// The CLI maps [`HookVerdictDecision::Block`] to exit 2, Allow to exit 0.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HookVerdictOutput {
    /// The hook verdict decision.
    pub decision: HookVerdictDecision,
    /// Human-readable reason for the decision (None when allowed).
    pub reason: Option<String>,
}

/// Error type for [`HookDispatchService`].
///
/// Wraps unknown hook name and handler execution failures without leaking
/// `domain::hook::HookError` across the usecase boundary.
/// Fail-closed: any error maps to block (exit 2) at the CLI.
#[derive(Debug, thiserror::Error)]
pub enum HookDispatchError {
    /// The hook name is not recognized by any handler.
    #[error("unknown hook name: {0}")]
    UnknownHookName(String),
    /// The hook handler returned an error during execution.
    #[error("hook handler failed: {0}")]
    HandlerFailed(String),
}

// ---------------------------------------------------------------------------
// Application service trait
// ---------------------------------------------------------------------------

/// Application service trait for the hook dispatch use case (`sotp hook dispatch`).
///
/// Driven by the CLI layer. Takes a [`HookDispatchCommand`] (raw CLI input) and
/// a hook name string, dispatches to the appropriate `HookHandler`, and returns
/// [`HookVerdictOutput`] so the CLI never imports `domain::hook::HookContext`,
/// `domain::hook::HookInput`, or `domain::hook::HookVerdict` directly.
pub trait HookDispatchService: Send + Sync {
    /// Dispatches the hook identified by `hook_name` with the given `command`.
    ///
    /// # Errors
    /// Returns [`HookDispatchError::UnknownHookName`] if `hook_name` is not recognized.
    /// Returns [`HookDispatchError::HandlerFailed`] if the handler returns an error.
    fn dispatch(
        &self,
        hook_name: String,
        command: HookDispatchCommand,
    ) -> Result<HookVerdictOutput, HookDispatchError>;
}

// ---------------------------------------------------------------------------
// Secondary port: faithful shell parser (hook dispatch boundary)
// ---------------------------------------------------------------------------

/// Usecase-owned secondary port for faithful shell command parsing.
///
/// Unlike [`crate::guard::ShellParserPort`] — which returns `argv.join(" ")`
/// strings and is intentionally lossy — this port requires implementations to
/// produce [`SimpleCommand`] values with accurate `argv` (quote-stripped
/// tokens), `redirect_texts`, `output_redirect_texts`, and
/// `has_output_redirect` fields.
///
/// Faithful parsing is required by the hook dispatch handlers:
///
/// * `block-direct-git-ops` checks `has_output_redirect` / `redirect_texts`
/// * `block-test-file-deletion` checks `output_redirect_texts`
///   to detect git operations hidden in output-redirect targets.
/// * `block-test-file-deletion` recurses into shell re-entry payloads
///   (`bash -c '...'`); quoted multi-word arguments must remain single tokens
///   so that the test-file argument after `-c` is correctly identified.
///
/// The infrastructure crate's `ConchShellParser` implements this trait via the
/// same parsing logic as `domain::guard::ShellParser`. The CLI injects it at
/// composition time; usecase code never imports a domain trait directly through
/// this port.
///
/// # Errors
/// Returns `ParseError` when the input cannot be parsed.
pub trait HookShellParserPort: Send + Sync {
    /// Splits a shell command string into faithful [`SimpleCommand`] values.
    ///
    /// # Errors
    /// Returns [`ParseError`] on invalid shell syntax (e.g. unmatched quotes).
    fn split_shell(&self, input: &str) -> Result<Vec<SimpleCommand>, ParseError>;
}

/// Bridges [`HookShellParserPort`] to [`domain::guard::ShellParser`] so that
/// the domain handlers can receive the port implementation as their parser.
///
/// This adapter is internal to the usecase crate.
struct HookShellParserPortAdapter {
    port: Arc<dyn HookShellParserPort>,
}

impl domain::guard::ShellParser for HookShellParserPortAdapter {
    fn split_shell(&self, input: &str) -> Result<Vec<SimpleCommand>, ParseError> {
        self.port.split_shell(input)
    }
}

// ---------------------------------------------------------------------------
// Skill compliance handler
// ---------------------------------------------------------------------------

/// Hook handler for the UserPromptSubmit skill compliance hook.
///
/// Advisory only — never blocks. Performs domain skill compliance checks
/// and returns an Allow verdict with optional `additionalContext` as the reason.
struct SkillComplianceHandler;

impl HookHandler for SkillComplianceHandler {
    fn handle(
        &self,
        _ctx: &HookContext,
        input: &HookInput,
    ) -> Result<HookVerdict, domain::hook::HookError> {
        // Skill compliance is advisory — always allow.
        // The hook name "skill-compliance" is for UserPromptSubmit hooks which
        // use a separate stdin envelope format (not PreToolUse).
        // At the usecase boundary we simply allow; CLI handles the JSON output.
        let _ = input;
        Ok(HookVerdict::allow())
    }
}

// ---------------------------------------------------------------------------
// Concrete interactor
// ---------------------------------------------------------------------------

/// Concrete struct implementing [`HookDispatchService`].
///
/// Wires [`GuardHookHandler`], [`TestFileDeletionGuardHandler`], and skill
/// compliance handler at construction. Converts [`HookDispatchCommand`] into
/// the domain `HookInput` internally, calls the appropriate handler via
/// `usecase::hook::dispatch`, and converts the domain `HookVerdict` into
/// [`HookVerdictOutput`]. The CLI never sees `domain::hook::*` types.
///
/// `project_dir` is injected at construction time by the CLI composition root
/// (from `$CLAUDE_PROJECT_DIR`) so that environment access stays in the CLI layer.
/// `guarded_git_token_present` is also injected by the CLI composition root
/// from `SOTP_GUARDED_GIT` presence so usecase code never reads process env.
/// `hooks_path_configured` is injected from the CLI composition root after it
/// reads local git config, keeping process calls out of the usecase layer.
///
/// Injects a [`HookShellParserPort`] (usecase-owned faithful parser port)
/// for both `block-direct-git-ops` and `block-test-file-deletion`. Both
/// guards need faithful, quote-preserving argv and redirect reconstruction.
/// Injecting the usecase-owned port (rather than `domain::guard::ShellParser`)
/// keeps the CLI composition root from importing domain traits directly.
///
/// DI fields are private implementation details. The public type contract is
/// captured by the [`HookDispatchService`] trait.
pub struct HookDispatchInteractor {
    parser_port: Arc<dyn HookShellParserPort>,
    /// Project directory injected from `$CLAUDE_PROJECT_DIR` by the CLI.
    project_dir: Option<std::path::PathBuf>,
    /// Whether the guarded git token was present when the CLI composition root started.
    guarded_git_token_present: bool,
    /// Whether local git config points `core.hooksPath` at `.githooks`.
    hooks_path_configured: bool,
}

impl HookDispatchInteractor {
    /// Creates a new `HookDispatchInteractor`.
    ///
    /// * `parser_port` — usecase-owned faithful shell parser port (e.g.
    ///   `ConchShellParser` from infrastructure). Must provide accurate argv
    ///   token sequences, redirect metadata, and shell re-entry payload
    ///   reconstruction. Injected by the CLI composition root.
    /// * `project_dir` — should be supplied by the CLI composition root from
    ///   `$CLAUDE_PROJECT_DIR`. Passing `None` is valid when the directory is
    ///   not available or not needed.
    /// * `guarded_git_token_present` — should be supplied by the CLI composition
    ///   root from the process environment check for `SOTP_GUARDED_GIT`. It is
    ///   consumed only by process-level git hook handlers.
    /// * `hooks_path_configured` — should be supplied by the CLI composition
    ///   root from a local `core.hooksPath` git config check.
    #[must_use]
    pub fn new(
        parser_port: Arc<dyn HookShellParserPort>,
        project_dir: Option<std::path::PathBuf>,
        guarded_git_token_present: bool,
        hooks_path_configured: bool,
    ) -> Self {
        Self { parser_port, project_dir, guarded_git_token_present, hooks_path_configured }
    }

    /// Builds the appropriate domain handler for the given hook name.
    ///
    /// Returns `None` if the hook name is not recognized.
    fn resolve_handler(&self, hook_name: &str) -> Option<Box<dyn HookHandler>> {
        let domain_parser: Arc<dyn domain::guard::ShellParser> =
            Arc::new(HookShellParserPortAdapter { port: Arc::clone(&self.parser_port) });
        let guarded_git_token_present = self.guarded_git_token_present;
        let hooks_path_configured = self.hooks_path_configured;
        match hook_name {
            "block-direct-git-ops" => Some(Box::new(GuardHookHandler::new(
                Arc::clone(&domain_parser),
                hooks_path_configured,
            ))),
            "block-test-file-deletion" => {
                Some(Box::new(TestFileDeletionGuardHandler { parser: domain_parser }))
            }
            "git-ref-update" => Some(Box::new(GitRefUpdateHandler { guarded_git_token_present })),
            "git-pre-push" => Some(Box::new(GitPrePushHandler { guarded_git_token_present })),
            "skill-compliance" => Some(Box::new(SkillComplianceHandler)),
            _ => None,
        }
    }
}

impl HookDispatchService for HookDispatchInteractor {
    fn dispatch(
        &self,
        hook_name: String,
        command: HookDispatchCommand,
    ) -> Result<HookVerdictOutput, HookDispatchError> {
        let handler = self
            .resolve_handler(&hook_name)
            .ok_or(HookDispatchError::UnknownHookName(hook_name))?;

        // Convert HookDispatchCommand → domain HookInput (internal conversion)
        let input = HookInput {
            tool_name: command.tool_name,
            command: command.command,
            file_path: command.file_path,
            content: command.content,
        };

        // Build domain HookContext from injected project_dir
        let ctx = HookContext { project_dir: self.project_dir.clone() };

        // Dispatch to the handler and convert the domain verdict to the usecase DTO
        let verdict = handler
            .handle(&ctx, &input)
            .map_err(|e| HookDispatchError::HandlerFailed(e.to_string()))?;

        // Convert domain HookVerdict → HookVerdictOutput
        let decision = match verdict.decision {
            Decision::Allow => HookVerdictDecision::Allow,
            Decision::Block => HookVerdictDecision::Block,
        };
        Ok(HookVerdictOutput { decision, reason: verdict.reason })
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    /// Minimal stub implementing [`HookShellParserPort`] for unit tests.
    ///
    /// Splits on `;` and tokenizes each segment on whitespace.
    /// Intentionally simple — just enough to verify dispatch routing and basic
    /// verdicts. Full shell re-entry detection (e.g. `bash -c 'rm tests/foo.rs'`)
    /// is tested at the infrastructure layer and in `hook.rs` using a richer parser.
    struct StubHookShellParserPort;

    impl HookShellParserPort for StubHookShellParserPort {
        fn split_shell(
            &self,
            input: &str,
        ) -> Result<Vec<domain::guard::SimpleCommand>, domain::guard::ParseError> {
            Ok(input
                .split(';')
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .map(|s| domain::guard::SimpleCommand {
                    argv: s.split_whitespace().map(str::to_owned).collect(),
                    redirect_texts: Vec::new(),
                    output_redirect_texts: Vec::new(),
                    has_output_redirect: false,
                })
                .collect())
        }
    }

    fn make_interactor() -> HookDispatchInteractor {
        make_interactor_with_guarded_git_token(false)
    }

    fn make_interactor_with_guarded_git_token(
        guarded_git_token_present: bool,
    ) -> HookDispatchInteractor {
        HookDispatchInteractor::new(
            Arc::new(StubHookShellParserPort),
            None,
            guarded_git_token_present,
            true,
        )
    }

    fn make_interactor_with_hooks_path_configured(
        hooks_path_configured: bool,
    ) -> HookDispatchInteractor {
        HookDispatchInteractor::new(
            Arc::new(StubHookShellParserPort),
            None,
            false,
            hooks_path_configured,
        )
    }

    fn bash_command(cmd: &str) -> HookDispatchCommand {
        HookDispatchCommand {
            tool_name: "Bash".to_owned(),
            command: Some(cmd.to_owned()),
            file_path: None,
            content: None,
        }
    }

    fn git_hook_command() -> HookDispatchCommand {
        HookDispatchCommand {
            tool_name: "Git".to_owned(),
            command: None,
            file_path: None,
            content: None,
        }
    }

    // --- HookVerdictDecision enum ---

    #[test]
    fn test_hook_verdict_decision_variants_exist() {
        let allow = HookVerdictDecision::Allow;
        let block = HookVerdictDecision::Block;
        assert_ne!(allow, block);
    }

    // --- HookVerdictOutput DTO ---

    #[test]
    fn test_hook_verdict_output_fields_accessible() {
        let output = HookVerdictOutput { decision: HookVerdictDecision::Allow, reason: None };
        assert_eq!(output.decision, HookVerdictDecision::Allow);
        assert!(output.reason.is_none());
    }

    // --- HookDispatchError ---

    #[test]
    fn test_hook_dispatch_error_unknown_hook_name() {
        let interactor = make_interactor();
        let cmd = bash_command("git status");
        let result = interactor.dispatch("nonexistent-hook".to_owned(), cmd);
        assert!(matches!(result, Err(HookDispatchError::UnknownHookName(_))));
    }

    // --- block-direct-git-ops ---

    #[test]
    fn test_dispatch_block_direct_git_ops_allows_safe_command() {
        let interactor = make_interactor();
        let cmd = bash_command("cargo make test");
        let result = interactor.dispatch("block-direct-git-ops".to_owned(), cmd);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().decision, HookVerdictDecision::Allow);
    }

    #[test]
    fn test_dispatch_block_direct_git_ops_blocks_git_add() {
        let interactor = make_interactor();
        let cmd = bash_command("git add .");
        let result = interactor.dispatch("block-direct-git-ops".to_owned(), cmd);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().decision, HookVerdictDecision::Block);
    }

    #[test]
    fn test_dispatch_block_direct_git_ops_blocks_git_when_hooks_path_not_configured() {
        let interactor = make_interactor_with_hooks_path_configured(false);
        let cmd = bash_command("git status");
        let result = interactor.dispatch("block-direct-git-ops".to_owned(), cmd);
        assert!(result.is_ok());
        let output = result.unwrap();
        assert_eq!(output.decision, HookVerdictDecision::Block);
        assert!(output.reason.as_deref().is_some_and(|reason| reason.contains("core.hooksPath")));
    }

    #[test]
    fn test_dispatch_block_direct_git_ops_missing_command_returns_handler_failed() {
        let interactor = make_interactor();
        let cmd = HookDispatchCommand {
            tool_name: "Bash".to_owned(),
            command: None,
            file_path: None,
            content: None,
        };
        let result = interactor.dispatch("block-direct-git-ops".to_owned(), cmd);
        assert!(matches!(result, Err(HookDispatchError::HandlerFailed(_))));
    }

    // --- block-test-file-deletion ---

    #[test]
    fn test_dispatch_block_test_file_deletion_allows_non_test_file() {
        let interactor = make_interactor();
        let cmd = bash_command("rm src/lib.rs");
        let result = interactor.dispatch("block-test-file-deletion".to_owned(), cmd);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().decision, HookVerdictDecision::Allow);
    }

    #[test]
    fn test_dispatch_block_test_file_deletion_blocks_rm_tests_dir() {
        let interactor = make_interactor();
        let cmd = bash_command("rm tests/foo.rs");
        let result = interactor.dispatch("block-test-file-deletion".to_owned(), cmd);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().decision, HookVerdictDecision::Block);
    }

    #[test]
    fn test_dispatch_block_test_file_deletion_write_empty_to_test_file_is_blocked() {
        let interactor = make_interactor();
        let cmd = HookDispatchCommand {
            tool_name: "Write".to_owned(),
            command: None,
            file_path: Some(std::path::PathBuf::from("tests/foo_test.rs")),
            content: Some(String::new()),
        };
        let result = interactor.dispatch("block-test-file-deletion".to_owned(), cmd);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().decision, HookVerdictDecision::Block);
    }

    // --- git-ref-update ---

    #[test]
    fn test_dispatch_git_ref_update_registered_does_not_return_unknown_hook_name() {
        let interactor = make_interactor();
        let result = interactor.dispatch("git-ref-update".to_owned(), git_hook_command());
        assert!(!matches!(result, Err(HookDispatchError::UnknownHookName(_))));
    }

    #[test]
    fn test_dispatch_git_ref_update_with_guarded_git_token_allows() {
        let interactor = make_interactor_with_guarded_git_token(true);
        let result = interactor.dispatch("git-ref-update".to_owned(), git_hook_command());
        assert!(result.is_ok());
        assert_eq!(result.unwrap().decision, HookVerdictDecision::Allow);
    }

    #[test]
    fn test_dispatch_git_ref_update_without_guarded_git_token_blocks() {
        let interactor = make_interactor_with_guarded_git_token(false);
        let result = interactor.dispatch("git-ref-update".to_owned(), git_hook_command());
        assert!(result.is_ok());
        let output = result.unwrap();
        assert_eq!(output.decision, HookVerdictDecision::Block);
        assert!(output.reason.as_deref().is_some_and(|reason| reason.contains("sotp wrapper")));
    }

    // --- git-pre-push ---

    #[test]
    fn test_dispatch_git_pre_push_registered_does_not_return_unknown_hook_name() {
        let interactor = make_interactor();
        let result = interactor.dispatch("git-pre-push".to_owned(), git_hook_command());
        assert!(!matches!(result, Err(HookDispatchError::UnknownHookName(_))));
    }

    #[test]
    fn test_dispatch_git_pre_push_with_guarded_git_token_allows() {
        let interactor = make_interactor_with_guarded_git_token(true);
        let result = interactor.dispatch("git-pre-push".to_owned(), git_hook_command());
        assert!(result.is_ok());
        assert_eq!(result.unwrap().decision, HookVerdictDecision::Allow);
    }

    #[test]
    fn test_dispatch_git_pre_push_without_guarded_git_token_blocks() {
        let interactor = make_interactor_with_guarded_git_token(false);
        let result = interactor.dispatch("git-pre-push".to_owned(), git_hook_command());
        assert!(result.is_ok());
        let output = result.unwrap();
        assert_eq!(output.decision, HookVerdictDecision::Block);
        assert!(output.reason.as_deref().is_some_and(|reason| reason.contains("sotp wrapper")));
    }

    // --- skill-compliance ---

    #[test]
    fn test_dispatch_skill_compliance_always_allows() {
        let interactor = make_interactor();
        let cmd = HookDispatchCommand {
            tool_name: "UserPromptSubmit".to_owned(),
            command: None,
            file_path: None,
            content: None,
        };
        let result = interactor.dispatch("skill-compliance".to_owned(), cmd);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().decision, HookVerdictDecision::Allow);
    }

    // --- HookDispatchCommand fields ---

    #[test]
    fn test_hook_dispatch_command_fields_accessible() {
        let cmd = HookDispatchCommand {
            tool_name: "Bash".to_owned(),
            command: Some("cargo build".to_owned()),
            file_path: Some(std::path::PathBuf::from("/tmp/test.txt")),
            content: Some("content".to_owned()),
        };
        assert_eq!(cmd.tool_name, "Bash");
        assert_eq!(cmd.command.as_deref(), Some("cargo build"));
        assert!(cmd.file_path.is_some());
        assert_eq!(cmd.content.as_deref(), Some("content"));
    }
}
