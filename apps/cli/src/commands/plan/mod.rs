//! CLI subcommands for local planner workflow wrappers.

use std::ffi::OsString;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;

use clap::{ArgGroup, Args, Subcommand};

mod codex_local;
#[cfg(test)]
mod tests;

use codex_local::execute_codex_local;

const DEFAULT_TIMEOUT_SECONDS: u64 = 600;

pub(crate) const PLAN_RUNTIME_DIR: &str = "tmp/planner-runtime";
pub(crate) const POLL_INTERVAL: Duration = Duration::from_millis(50);
#[cfg(test)]
pub(crate) const CODEX_BIN_ENV: &str = "SOTP_CODEX_BIN";

#[derive(Debug, Subcommand)]
pub enum PlanCommand {
    /// Run the local Codex-backed planner through a repo-owned wrapper.
    CodexLocal(PlanCodexLocalArgs),
}

#[derive(Debug, Args)]
#[command(group(
    ArgGroup::new("plan_input")
        .required(true)
        .args(["briefing_file", "prompt"])
))]
pub struct PlanCodexLocalArgs {
    /// Model name resolved from `.harness/config/agent-profiles.json`.
    #[arg(long)]
    pub(super) model: String,

    /// Timeout for the planner subprocess in seconds.
    #[arg(long, default_value_t = DEFAULT_TIMEOUT_SECONDS)]
    pub(super) timeout_seconds: u64,

    /// Path to a briefing file that the planner should read.
    #[arg(long)]
    pub(super) briefing_file: Option<PathBuf>,

    /// Inline prompt for the planner.
    #[arg(long)]
    pub(super) prompt: Option<String>,
}

/// Result of a Codex planner subprocess invocation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PlanRunResult {
    /// Raw exit code from the Codex subprocess.
    pub(super) exit_code: u8,
}

/// Codex invocation configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct CodexInvocation {
    pub(super) bin: OsString,
    pub(super) args: Vec<OsString>,
}

pub fn execute(cmd: PlanCommand) -> ExitCode {
    match cmd {
        PlanCommand::CodexLocal(args) => execute_codex_local(&args),
    }
}
