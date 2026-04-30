//! Guard shell-command check use case.
//!
//! Provides [`GuardDecision`], [`GuardCheckOutput`], [`GuardCheckService`],
//! [`GuardCheckInteractor`], and [`ShellParserPort`] so the CLI layer never
//! imports `domain::Decision`, `domain::guard::ShellParser`, or
//! `domain::GuardVerdict` directly.

use std::sync::Arc;

use domain::guard::{GuardVerdict, SimpleCommand, policy};

// ---------------------------------------------------------------------------
// Public boundary types
// ---------------------------------------------------------------------------

/// Usecase-owned binary policy decision (mirrors `domain::Decision`).
///
/// The CLI consumes this enum instead of importing `domain::Decision` directly,
/// satisfying the CN-01 constraint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuardDecision {
    /// The operation is allowed to proceed.
    Allow,
    /// The operation is blocked.
    Block,
}

/// Output DTO returned by the guard check service.
///
/// Wraps [`GuardDecision`] and a human-readable reason string so the CLI
/// never imports `domain::Decision` or `domain::GuardVerdict` directly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuardCheckOutput {
    /// The policy decision.
    pub decision: GuardDecision,
    /// Human-readable reason for the decision (empty when allowed).
    pub reason: String,
}

// ---------------------------------------------------------------------------
// Secondary port (driven port)
// ---------------------------------------------------------------------------

/// Usecase-owned secondary port for shell command parsing.
///
/// Mirrors `domain::guard::ShellParser` but uses primitive `String` types
/// for the parsed commands so the CLI composition root can inject
/// `ConchShellParser` (infrastructure) without importing the domain trait.
///
/// Inside the usecase crate, `ShellParserPortAdapter` converts the returned
/// strings into `domain::guard::SimpleCommand` values for the policy engine.
///
/// # Boundary contract
///
/// Each `String` returned by `split_shell` is a **whitespace-delimited token
/// sequence** representing one simple command's argv. Shell quoting, escaping,
/// and multi-word arguments that contain spaces are **not preserved** — the
/// returned strings are expected to be re-split on whitespace to recover argv
/// tokens. This is an accepted interface simplification; see
/// `ShellParserPortAdapter` for the accepted trade-off details.
pub trait ShellParserPort: Send + Sync {
    /// Splits a shell command string into individual command token sequences.
    ///
    /// Each returned `String` represents one simple command's argv tokens
    /// joined by whitespace. The argv tokens are recovered by splitting the
    /// returned strings on whitespace. Shell quoting and multi-word arguments
    /// are **not preserved** through this interface.
    ///
    /// Returns an `Err(String)` on parse failure.
    ///
    /// # Errors
    ///
    /// Returns a `String` describing the parse failure.
    fn split_shell(&self, input: &str) -> Result<Vec<String>, String>;
}

// ---------------------------------------------------------------------------
// Application service trait
// ---------------------------------------------------------------------------

/// Application service trait for the guard shell-command check use case.
///
/// Driven by the CLI layer. The implementation ([`GuardCheckInteractor`])
/// delegates to the domain guard policy via [`ShellParserPort`].
///
/// The public API exposes only usecase-owned types so the CLI never imports
/// `domain::Decision` or `domain::GuardVerdict`.
///
/// # Scope of enforcement
///
/// Policy enforcement is limited to the information provided by the injected
/// [`ShellParserPort`] implementation. The default boundary only propagates
/// `argv` tokens; output-redirect targets and heredoc bodies are not visible
/// to the policy unless the port implementation explicitly preserves them.
/// See `ShellParserPortAdapter` for the accepted trade-off details.
pub trait GuardCheckService: Send + Sync {
    /// Checks a shell command string against the guard policy.
    ///
    /// Returns a [`GuardCheckOutput`] describing the policy decision and
    /// optional reason. Never returns an error: parse failures are
    /// converted to a `Block` verdict (fail-closed).
    fn check(&self, command: String) -> GuardCheckOutput;
}

// ---------------------------------------------------------------------------
// Internal adapter: converts ShellParserPort output to domain SimpleCommand values
// ---------------------------------------------------------------------------

/// Converts [`ShellParserPort`] output into [`SimpleCommand`] values for the domain policy.
///
/// This struct is internal to the usecase crate and invisible to the CLI.
/// It reconstructs [`SimpleCommand`] values from the `Vec<String>` produced
/// by [`ShellParserPort::split_shell`].
///
/// # Known limitation (accepted design trade-off)
///
/// The reconstruction only restores `argv` from each command string
/// (whitespace-split). `redirect_texts` and `has_output_redirect` are **not**
/// preserved through the [`ShellParserPort`] interface. As a result, the domain
/// guard policy cannot detect git operations hidden in output-redirect targets
/// or heredoc bodies when commands are evaluated through this adapter.
///
/// This is an intentional boundary simplification: the `ShellParserPort`
/// abstraction preserves only the argv information needed for basic command
/// classification. The infrastructure implementation (`ConchShellParser`)
/// provides full redirect information when wired directly to the domain.
/// If redirect-based policy enforcement is required, inject an infrastructure
/// adapter that implements `ShellParserPort` with full redirect preservation.
struct ShellParserPortAdapter<'a> {
    port: &'a dyn ShellParserPort,
}

impl ShellParserPortAdapter<'_> {
    /// Parses the command string via the port and converts it to a list of
    /// [`SimpleCommand`] values, or returns the raw error string on failure.
    ///
    /// The returned `SimpleCommand` values only carry `argv` information; see the
    /// type-level doc for the accepted limitation on redirect/heredoc detection.
    fn parse(&self, input: &str) -> Result<Vec<SimpleCommand>, String> {
        let command_strings = self.port.split_shell(input)?;
        Ok(command_strings
            .into_iter()
            .map(|s| SimpleCommand {
                argv: s.split_whitespace().map(str::to_owned).collect(),
                redirect_texts: Vec::new(),
                has_output_redirect: false,
            })
            .collect())
    }
}

// ---------------------------------------------------------------------------
// Concrete interactor
// ---------------------------------------------------------------------------

/// Concrete implementation of [`GuardCheckService`].
///
/// Holds a [`ShellParserPort`] secondary port (`Arc<dyn ShellParserPort>`) and
/// uses `ShellParserPortAdapter` to bridge it to the domain guard policy.
/// The `domain::GuardVerdict` + `domain::Decision` result is converted into
/// [`GuardCheckOutput`] before returning to the CLI.
///
/// DI fields are private implementation details. The public type contract is
/// captured by the [`GuardCheckService`] trait.
pub struct GuardCheckInteractor {
    parser_port: Arc<dyn ShellParserPort>,
}

impl GuardCheckInteractor {
    /// Creates a new `GuardCheckInteractor` with the given shell parser port.
    #[must_use]
    pub fn new(parser_port: Arc<dyn ShellParserPort>) -> Self {
        Self { parser_port }
    }
}

impl GuardCheckService for GuardCheckInteractor {
    fn check(&self, command: String) -> GuardCheckOutput {
        let adapter = ShellParserPortAdapter { port: self.parser_port.as_ref() };

        let verdict = match adapter.parse(&command) {
            Ok(commands) => policy::check_commands(&commands),
            Err(err) => {
                // Fail-closed: any parse failure becomes a Block verdict.
                // The error string from ShellParserPort is propagated as the
                // block reason so callers receive the actual parse failure
                // description rather than a misattributed category.
                GuardVerdict::block(format!("unparseable command: {err}"))
            }
        };

        let decision = match verdict.decision {
            domain::Decision::Allow => GuardDecision::Allow,
            domain::Decision::Block => GuardDecision::Block,
        };

        GuardCheckOutput { decision, reason: verdict.reason }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    /// Minimal stub implementing [`ShellParserPort`] for unit tests.
    ///
    /// Splits on `';'` to simulate multi-command strings.
    struct StubShellParserPort;

    impl ShellParserPort for StubShellParserPort {
        fn split_shell(&self, input: &str) -> Result<Vec<String>, String> {
            Ok(input.split(';').map(|s| s.trim().to_owned()).filter(|s| !s.is_empty()).collect())
        }
    }

    /// Returns a [`GuardCheckInteractor`] backed by [`StubShellParserPort`].
    fn make_interactor() -> GuardCheckInteractor {
        GuardCheckInteractor::new(Arc::new(StubShellParserPort))
    }

    #[test]
    fn test_guard_check_allows_safe_command() {
        let interactor = make_interactor();
        let output = interactor.check("cargo make test".to_owned());
        assert_eq!(output.decision, GuardDecision::Allow);
    }

    #[test]
    fn test_guard_check_blocks_git_add() {
        let interactor = make_interactor();
        let output = interactor.check("git add .".to_owned());
        assert_eq!(output.decision, GuardDecision::Block);
        assert!(!output.reason.is_empty(), "block verdict should have a non-empty reason");
    }

    #[test]
    fn test_guard_check_blocks_git_commit() {
        let interactor = make_interactor();
        let output = interactor.check("git commit -m 'msg'".to_owned());
        assert_eq!(output.decision, GuardDecision::Block);
    }

    #[test]
    fn test_guard_check_blocks_git_push() {
        let interactor = make_interactor();
        let output = interactor.check("git push".to_owned());
        assert_eq!(output.decision, GuardDecision::Block);
    }

    #[test]
    fn test_guard_check_allows_git_status() {
        let interactor = make_interactor();
        let output = interactor.check("git status".to_owned());
        assert_eq!(output.decision, GuardDecision::Allow);
    }

    #[test]
    fn test_guard_check_allows_git_log() {
        let interactor = make_interactor();
        let output = interactor.check("git log --oneline".to_owned());
        assert_eq!(output.decision, GuardDecision::Allow);
    }

    #[test]
    fn test_guard_check_with_parse_error_blocks_fail_closed() {
        struct FailingShellParserPort;
        impl ShellParserPort for FailingShellParserPort {
            fn split_shell(&self, _input: &str) -> Result<Vec<String>, String> {
                Err("unmatched quote".to_owned())
            }
        }

        let interactor = GuardCheckInteractor::new(Arc::new(FailingShellParserPort));
        let output = interactor.check("git 'broken".to_owned());
        assert_eq!(output.decision, GuardDecision::Block, "parse errors must be fail-closed");
    }

    #[test]
    fn test_guard_decision_enum_variants_exist() {
        let allow = GuardDecision::Allow;
        let block = GuardDecision::Block;
        assert_ne!(allow, block);
    }

    #[test]
    fn test_guard_check_output_fields_accessible() {
        let output = GuardCheckOutput { decision: GuardDecision::Allow, reason: String::new() };
        assert_eq!(output.decision, GuardDecision::Allow);
        assert!(output.reason.is_empty());
    }
}
