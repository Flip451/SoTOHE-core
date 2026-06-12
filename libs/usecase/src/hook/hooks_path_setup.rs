//! hooksPath setup preflight handler.
//!
//! This guard has a deliberately narrow responsibility: when local git hooks are
//! not configured, stop Bash execution early and direct the operator to the setup
//! commands that install the process-level git hook enforcement.

use std::sync::Arc;

use domain::guard::{ShellParser, SimpleCommand};
use domain::hook::{HookContext, HookError, HookInput, HookVerdict};

use super::HookHandler;

const HOOKS_PATH_SETUP_MESSAGE: &str = "[Git Policy] core.hooksPath is not configured. \
     Run `cargo make bootstrap` to configure `.githooks`, \
     or run exactly `git config --local core.hooksPath .githooks`.";

const CARGO_MAKE_BOOTSTRAP: &[&str] = &["cargo", "make", "bootstrap"];
const GIT_CONFIG_HOOKS_PATH: &[&str] = &["git", "config", "--local", "core.hooksPath", ".githooks"];

/// Hook handler for `hooks-path-setup`.
///
/// This handler is independent from `block-direct-git-ops`: it performs only the
/// environment preflight needed before process-level git hooks can enforce git
/// writes.
pub struct HooksPathSetupHandler {
    parser: Arc<dyn ShellParser>,
    hooks_path_configured: bool,
}

impl HooksPathSetupHandler {
    pub(crate) fn new(parser: Arc<dyn ShellParser>, hooks_path_configured: bool) -> Self {
        Self { parser, hooks_path_configured }
    }
}

impl HookHandler for HooksPathSetupHandler {
    fn handle(&self, _ctx: &HookContext, input: &HookInput) -> Result<HookVerdict, HookError> {
        if self.hooks_path_configured {
            return Ok(HookVerdict::allow());
        }

        let command =
            input.command.as_deref().ok_or_else(|| HookError::Input("missing command".into()))?;

        if is_allowed_setup_command(self.parser.as_ref(), command) {
            Ok(HookVerdict::allow())
        } else {
            Ok(HookVerdict::block(HOOKS_PATH_SETUP_MESSAGE))
        }
    }
}

fn is_allowed_setup_command(parser: &dyn ShellParser, command: &str) -> bool {
    let Ok(commands) = parser.split_shell(command) else {
        return false;
    };
    let [simple_command] = commands.as_slice() else {
        return false;
    };
    argv_matches(simple_command, CARGO_MAKE_BOOTSTRAP)
        || argv_matches(simple_command, GIT_CONFIG_HOOKS_PATH)
}

fn argv_matches(command: &SimpleCommand, expected: &[&str]) -> bool {
    command.argv.iter().map(String::as_str).eq(expected.iter().copied())
        && !command.has_output_redirect
        && command.redirect_texts.is_empty()
        && command.output_redirect_texts.is_empty()
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::hook::test_support::simple_command;
    use domain::guard::ParseError;

    struct TestShellParser {
        commands: Result<Vec<SimpleCommand>, ParseError>,
    }

    impl ShellParser for TestShellParser {
        fn split_shell(&self, _input: &str) -> Result<Vec<SimpleCommand>, ParseError> {
            self.commands.clone()
        }
    }

    fn test_handler(
        commands: Vec<SimpleCommand>,
        hooks_path_configured: bool,
    ) -> HooksPathSetupHandler {
        HooksPathSetupHandler::new(
            Arc::new(TestShellParser { commands: Ok(commands) }),
            hooks_path_configured,
        )
    }

    fn test_handler_with_parse_error() -> HooksPathSetupHandler {
        HooksPathSetupHandler::new(
            Arc::new(TestShellParser { commands: Err(ParseError::UnmatchedQuote) }),
            false,
        )
    }

    fn make_input(command: &str) -> HookInput {
        HookInput {
            tool_name: "Bash".to_owned(),
            command: Some(command.to_owned()),
            file_path: None,
            content: None,
        }
    }

    #[test]
    fn test_hooks_path_setup_allows_any_command_when_configured() {
        let handler = test_handler(Vec::new(), true);
        let verdict = handler
            .handle(&HookContext { project_dir: None }, &make_input("python3 -c 'print(1)'"))
            .unwrap();
        assert!(!verdict.is_blocked());
    }

    #[test]
    fn test_hooks_path_setup_allows_bootstrap_when_not_configured() {
        let handler = test_handler(vec![simple_command(CARGO_MAKE_BOOTSTRAP)], false);
        let verdict = handler
            .handle(&HookContext { project_dir: None }, &make_input("cargo   make   bootstrap"))
            .unwrap();
        assert!(!verdict.is_blocked());
    }

    #[test]
    fn test_hooks_path_setup_allows_direct_config_when_not_configured() {
        let handler = test_handler(vec![simple_command(GIT_CONFIG_HOOKS_PATH)], false);
        let verdict = handler
            .handle(
                &HookContext { project_dir: None },
                &make_input("git config --local core.hooksPath .githooks"),
            )
            .unwrap();
        assert!(!verdict.is_blocked());
    }

    #[test]
    fn test_hooks_path_setup_blocks_other_bash_when_not_configured() {
        let handler = test_handler(vec![simple_command(&["python3", "-c", "print(1)"])], false);
        let verdict = handler
            .handle(
                &HookContext { project_dir: None },
                &make_input("python3 -c \"import subprocess; subprocess.run(['git', 'status'])\""),
            )
            .unwrap();
        assert!(verdict.is_blocked());
        assert!(verdict.reason.as_deref().is_some_and(|reason| {
            reason.contains("core.hooksPath") && reason.contains("cargo make bootstrap")
        }));
    }

    #[test]
    fn test_hooks_path_setup_blocks_compound_setup_command_when_not_configured() {
        let handler = test_handler(
            vec![simple_command(CARGO_MAKE_BOOTSTRAP), simple_command(&["git", "status"])],
            false,
        );
        let verdict = handler
            .handle(
                &HookContext { project_dir: None },
                &make_input("cargo make bootstrap && git status"),
            )
            .unwrap();
        assert!(verdict.is_blocked());
    }

    #[test]
    fn test_hooks_path_setup_blocks_parse_error_when_not_configured() {
        let handler = test_handler_with_parse_error();
        let verdict = handler
            .handle(&HookContext { project_dir: None }, &make_input("cargo make 'bootstrap"))
            .unwrap();
        assert!(verdict.is_blocked());
    }
}
