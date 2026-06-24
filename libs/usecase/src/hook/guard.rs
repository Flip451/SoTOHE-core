//! Guard hook handler — blocks raw Bash commands that contain the guarded-git token.
//!
//! Scope: CN-04 / AC-03 (D3 stage a).
//! The raw Bash command string is scanned for `SOTP_GUARDED_GIT` at a word boundary.
//! If found, the command is blocked. Otherwise, the parsed-command policy check
//! (`domain::guard::policy::check_commands`) is delegated to.

use std::sync::Arc;

use domain::guard::{ShellParser, policy};
use domain::hook::{HookContext, HookError, HookInput, HookVerdict};

use super::HookHandler;
use crate::guard::{SOTP_GUARDED_TOKEN_REASON, raw_command_contains_guarded_token};

/// Hook handler for `block-direct-git-ops`.
///
/// Stage (a): scans the raw Bash command string for `SOTP_GUARDED_GIT` at a word
/// boundary and blocks if found (CN-04 / D3).
///
/// Stage (b): delegates to `domain::guard::policy::check_commands` for direct-git-subcommand
/// and launcher-stripped checks.
pub struct GuardHookHandler {
    pub parser: Arc<dyn ShellParser>,
}

impl GuardHookHandler {
    pub(crate) fn new(parser: Arc<dyn ShellParser>) -> Self {
        Self { parser }
    }
}

impl HookHandler for GuardHookHandler {
    fn handle(&self, _ctx: &HookContext, input: &HookInput) -> Result<HookVerdict, HookError> {
        let command =
            input.command.as_deref().ok_or_else(|| HookError::Input("missing command".into()))?;

        if raw_command_contains_guarded_token(command) {
            return Ok(HookVerdict::block(SOTP_GUARDED_TOKEN_REASON.to_owned()));
        }

        let commands = match self.parser.split_shell(command) {
            Ok(cmds) => cmds,
            Err(err) => {
                let verdict = policy::block_on_parse_error(&err);
                return Ok(HookVerdict::block(verdict.reason));
            }
        };

        let guard_verdict = policy::check_commands(&commands);

        if guard_verdict.is_blocked() {
            Ok(HookVerdict::block(guard_verdict.reason))
        } else {
            Ok(HookVerdict::allow())
        }
    }
}

#[cfg(test)]
#[allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::hook::test_support::simple_command;
    use domain::guard::{ParseError, SimpleCommand};
    use rstest::rstest;

    struct TestShellParser {
        commands: Vec<SimpleCommand>,
    }

    impl ShellParser for TestShellParser {
        fn split_shell(&self, _input: &str) -> Result<Vec<SimpleCommand>, ParseError> {
            Ok(self.commands.clone())
        }
    }

    fn test_parser(commands: Vec<SimpleCommand>) -> Arc<dyn ShellParser> {
        Arc::new(TestShellParser { commands })
    }

    fn test_handler(commands: Vec<SimpleCommand>) -> GuardHookHandler {
        GuardHookHandler::new(test_parser(commands))
    }

    use domain::hook::HookContext;

    fn make_input(command: &str) -> HookInput {
        HookInput {
            tool_name: "Bash".into(),
            command: Some(command.into()),
            file_path: None,
            content: None,
        }
    }

    #[test]
    fn test_guard_handler_allows_git_status() {
        let handler = test_handler(vec![simple_command(&["git", "status"])]);
        let ctx = HookContext { project_dir: None };
        let verdict = handler.handle(&ctx, &make_input("git status")).unwrap();
        assert!(!verdict.is_blocked());
    }

    #[test]
    fn test_guard_handler_blocks_git_add() {
        let handler = test_handler(vec![simple_command(&["git", "add", "."])]);
        let ctx = HookContext { project_dir: None };
        let verdict = handler.handle(&ctx, &make_input("git add .")).unwrap();
        assert!(verdict.is_blocked());
    }

    #[test]
    fn test_guard_handler_returns_error_on_missing_command() {
        let handler = test_handler(Vec::new());
        let ctx = HookContext { project_dir: None };
        let input =
            HookInput { tool_name: "Bash".into(), command: None, file_path: None, content: None };
        let result = handler.handle(&ctx, &input);
        assert!(matches!(result, Err(HookError::Input(msg)) if msg.contains("missing command")));
    }

    // AC-03 stage (a): raw commands containing the guarded-git token at a word boundary
    #[rstest]
    #[case::token_as_env_prefix("SOTP_GUARDED_GIT=1 git commit -m msg")]
    #[case::token_in_middle("env SOTP_GUARDED_GIT=1 cargo test")]
    fn test_guard_handler_blocks_raw_command_with_guarded_token(#[case] raw_command: &str) {
        let handler = test_handler(Vec::new());
        let ctx = HookContext { project_dir: None };
        let verdict = handler.handle(&ctx, &make_input(raw_command)).unwrap();
        assert!(
            verdict.is_blocked(),
            "raw command containing SOTP_GUARDED_GIT must be blocked (AC-03 stage a): {raw_command}"
        );
    }

    #[test]
    fn test_guard_handler_allows_extended_identifier_containing_token() {
        let handler = test_handler(vec![simple_command(&["echo", "SOTP_GUARDED_GITX"])]);
        let ctx = HookContext { project_dir: None };
        let verdict = handler.handle(&ctx, &make_input("echo SOTP_GUARDED_GITX")).unwrap();
        assert!(!verdict.is_blocked(), "extended identifier SOTP_GUARDED_GITX must not be blocked");
    }
}
