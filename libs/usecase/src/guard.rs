//! Guard shell-command check use case.
//!
//! Provides [`GuardDecision`], [`GuardCheckOutput`], [`GuardCheckService`],
//! [`GuardCheckInteractor`], and [`ShellParserPort`] so the CLI layer never
//! imports `domain::Decision`, `domain::guard::ShellParser`, or
//! `domain::GuardVerdict` directly.

use std::sync::Arc;

use domain::guard::{GuardVerdict, SimpleCommand, policy};

// ---------------------------------------------------------------------------
// Raw-command guarded-token scan (D3/IN-03 stage a)
// ---------------------------------------------------------------------------

/// Word-boundary exact-match token for the guarded-git bypass scan (D3 stage a).
pub(crate) const SOTP_GUARDED_TOKEN: &str = "SOTP_GUARDED_GIT";

/// Block reason returned when the raw-command scan finds the guarded-git token.
pub(crate) const SOTP_GUARDED_TOKEN_REASON: &str = "[Git Policy] The guarded-git token is present in the Bash command string. \
     The token must not be passed inline — it is injected only by the sotp binary \
     via its git_cli layer.";

/// Returns `true` if `command` contains the guarded-git token as a whole word
/// (word-boundary exact match). Partial identifiers like `SOTP_GUARDED_GITX`
/// do **not** match.
///
/// The argv-token scan in `domain::guard::policy::check_commands` cannot see
/// values inside variable assignments (`FOO=$SOTP_GUARDED_GIT cargo test`
/// drops the value before argv is constructed). This raw-string scan covers
/// that gap by inspecting the original Bash command string before any parsing.
pub(crate) fn raw_command_contains_guarded_token(command: &str) -> bool {
    let token = SOTP_GUARDED_TOKEN;
    let tbytes = token.as_bytes();
    let bytes = command.as_bytes();
    let tlen = tbytes.len();
    if tlen == 0 || bytes.len() < tlen {
        return false;
    }
    bytes.windows(tlen).enumerate().any(|(i, window)| {
        if window != tbytes {
            return false;
        }
        let before_ok = i == 0
            || bytes
                .get(i.wrapping_sub(1))
                .is_some_and(|b| !b.is_ascii_alphanumeric() && *b != b'_');
        let after_ok = bytes.get(i + tlen).is_none_or(|b| !b.is_ascii_alphanumeric() && *b != b'_');
        before_ok && after_ok
    })
}

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
    /// # Errors
    ///
    /// Returns [`ShellParserError`] describing the parse failure.
    fn split_shell(&self, input: &str) -> Result<Vec<String>, ShellParserError>;
}

/// Error returned by [`ShellParserPort::split_shell`].
#[derive(Debug, thiserror::Error)]
pub enum ShellParserError {
    /// Shell command parsing failed.
    #[error("{0}")]
    ParseFailed(String),
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
/// (whitespace-split). Redirect metadata (`redirect_texts`,
/// `output_redirect_texts`, and `has_output_redirect`) is **not** preserved
/// through the [`ShellParserPort`] interface. As a result, the domain guard
/// policy cannot detect git operations hidden in output-redirect targets or
/// heredoc bodies when commands are evaluated through this adapter.
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
    /// [`SimpleCommand`] values, or returns [`ShellParserError`] on failure.
    ///
    /// The returned `SimpleCommand` values only carry `argv` information; see the
    /// type-level doc for the accepted limitation on redirect/heredoc detection.
    fn parse(&self, input: &str) -> Result<Vec<SimpleCommand>, ShellParserError> {
        let command_strings = self.port.split_shell(input)?;
        Ok(command_strings
            .into_iter()
            .map(|s| SimpleCommand {
                argv: s.split_whitespace().map(str::to_owned).collect(),
                redirect_texts: Vec::new(),
                output_redirect_texts: Vec::new(),
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
        // D3/IN-03 stage (a): raw-string scan for the guarded-git token before parsing.
        // The domain argv-token scan (stage b) cannot see values inside variable
        // assignments such as `FOO=$SOTP_GUARDED_GIT cargo test`, because the
        // assignment is stripped from argv before `check_commands` runs. The raw
        // scan covers that gap.
        if raw_command_contains_guarded_token(&command) {
            return GuardCheckOutput {
                decision: GuardDecision::Block,
                reason: SOTP_GUARDED_TOKEN_REASON.to_owned(),
            };
        }

        let adapter = ShellParserPortAdapter { port: self.parser_port.as_ref() };

        let verdict = match adapter.parse(&command) {
            Ok(commands) => policy::check_commands(&commands),
            Err(err) => {
                // Fail-closed: any parse failure becomes a Block verdict.
                // The error from ShellParserPort is propagated as the block
                // reason so callers receive the actual parse failure description
                // rather than a misattributed category.
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
        fn split_shell(&self, input: &str) -> Result<Vec<String>, ShellParserError> {
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

    // D3/IN-03 stage (a): raw-command scan for SOTP_GUARDED_GIT.
    // The argv-token scan (stage b in domain) cannot see values inside variable
    // assignments such as `FOO=$SOTP_GUARDED_GIT cargo test` (the assignment value is
    // stripped from argv before policy::check_commands runs). GuardCheckInteractor must
    // catch these via the raw-string scan, matching the GuardHookHandler behavior.
    #[rstest::rstest]
    #[case::token_as_env_prefix("SOTP_GUARDED_GIT=1 git commit -m msg")]
    #[case::token_in_middle("env SOTP_GUARDED_GIT=1 cargo test")]
    #[case::token_in_assignment_value("FOO=$SOTP_GUARDED_GIT cargo test")]
    fn test_guard_check_blocks_raw_command_with_guarded_token(#[case] raw_command: &str) {
        let interactor = make_interactor();
        let output = interactor.check(raw_command.to_owned());
        assert_eq!(
            output.decision,
            GuardDecision::Block,
            "raw command containing SOTP_GUARDED_GIT must be blocked (AC-03 stage a): {raw_command}"
        );
        assert!(
            output.reason.contains("guarded-git token"),
            "block reason must mention the guarded-git token: {}",
            output.reason
        );
    }

    #[test]
    fn test_guard_check_allows_extended_identifier_containing_token() {
        let interactor = make_interactor();
        let output = interactor.check("echo SOTP_GUARDED_GITX".to_owned());
        assert_eq!(
            output.decision,
            GuardDecision::Allow,
            "extended identifier SOTP_GUARDED_GITX must not be blocked"
        );
    }

    #[test]
    fn test_guard_check_with_parse_error_blocks_fail_closed() {
        struct FailingShellParserPort;
        impl ShellParserPort for FailingShellParserPort {
            fn split_shell(&self, _input: &str) -> Result<Vec<String>, ShellParserError> {
                Err(ShellParserError::ParseFailed("unmatched quote".to_owned()))
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
