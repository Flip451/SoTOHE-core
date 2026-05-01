//! Guard subcommand for shell command policy checking.

use std::process::ExitCode;
use std::sync::Arc;

use infrastructure::shell::ConchShellParser;
use usecase::hook_dispatch::{
    HookDispatchCommand, HookDispatchInteractor, HookDispatchService, HookVerdictDecision,
};

/// Guard subcommands for shell command checking.
#[derive(Debug, clap::Subcommand)]
pub enum GuardCommand {
    /// Check a shell command against the guard policy.
    Check {
        /// The shell command string to check.
        #[arg(long)]
        command: String,
    },
}

/// Executes a guard subcommand.
pub fn execute(cmd: GuardCommand) -> ExitCode {
    match cmd {
        GuardCommand::Check { command } => execute_check(&command),
    }
}

fn execute_check(command: &str) -> ExitCode {
    // Composition root: wire ConchShellParser (infrastructure) as Arc<dyn HookShellParserPort>
    // into HookDispatchInteractor, then dispatch through block-direct-git-ops.
    //
    // This path uses the faithful parser port (HookShellParserPort) rather than the
    // lossy ShellParserPort, so redirect targets and has_output_redirect are preserved
    // for policy evaluation — matching the pre-migration security posture.
    //
    // CLI never imports domain::Decision, domain::guard::*, or domain::guard::ShellParser.
    let parser_port = Arc::new(ConchShellParser);
    let service = HookDispatchInteractor::new(parser_port, None);

    // Construct a synthetic Bash tool hook dispatch command.
    // GuardHookHandler reads command from HookDispatchCommand::command.
    let dispatch_cmd = HookDispatchCommand {
        tool_name: "Bash".to_owned(),
        command: Some(command.to_owned()),
        file_path: None,
        content: None,
    };

    let (decision_str, reason, is_blocked) =
        match service.dispatch("block-direct-git-ops".to_owned(), dispatch_cmd) {
            Ok(output) => {
                let blocked = output.decision == HookVerdictDecision::Block;
                let reason = output.reason.unwrap_or_default();
                let decision_str = if blocked { "block" } else { "allow" };
                (decision_str, reason, blocked)
            }
            Err(err) => {
                // Fail-closed: any dispatch error becomes a block verdict.
                ("block", format!("dispatch error: {err}"), true)
            }
        };

    // Output JSON verdict to stdout
    let json = serde_json::json!({
        "decision": decision_str,
        "reason": reason,
    });
    println!("{json}");

    if is_blocked { ExitCode::FAILURE } else { ExitCode::SUCCESS }
}
