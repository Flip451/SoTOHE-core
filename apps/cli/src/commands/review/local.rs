//! `sotp review local` — unified local reviewer entry point with auto-resolved provider.
//!
//! Loads `agent-profiles.json`, resolves `reviewer` capability for the given
//! round type, and dispatches to `CodexReviewer` (provider=codex) or
//! `ClaudeReviewer` (provider=claude) via `CliApp.review_run_local`
//! (CN-03 / CN-04 / AC-01 / AC-04).

use std::io::{self, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{ArgGroup, Args};
use cli_driver::review::ReviewInput;

use super::CodexRoundTypeArg;

/// Arguments for `sotp review local`.
#[derive(Debug, Args)]
#[command(group(
    ArgGroup::new("local_review_input")
        .required(true)
        .args(["briefing_file", "prompt"])
))]
pub struct LocalArgs {
    /// Path to a briefing file that the reviewer should read.
    #[arg(long)]
    pub(super) briefing_file: Option<PathBuf>,

    /// Inline prompt for the reviewer.
    #[arg(long)]
    pub(super) prompt: Option<String>,

    /// Track ID (used for auto-recording verdict to review.json).
    /// When omitted, resolved from the current git branch (`track/<id>`).
    #[arg(long)]
    pub(super) track_id: Option<String>,

    /// Round type: fast or final.
    #[arg(long, value_enum)]
    pub(super) round_type: CodexRoundTypeArg,

    /// Review scope name (e.g., "domain", "infrastructure", "other").
    #[arg(long)]
    pub(super) group: String,

    /// Path to track items directory.
    #[arg(long, default_value = "track/items")]
    pub(super) items_dir: PathBuf,

    /// Timeout for the reviewer subprocess in seconds.
    #[arg(long, default_value_t = super::DEFAULT_TIMEOUT_SECONDS)]
    pub(super) timeout_seconds: u64,

    /// Optional model override (ad-hoc use only; normally resolved from agent-profiles.json).
    #[arg(long)]
    pub(super) model: Option<String>,
}

pub(super) fn execute_local(args: &LocalArgs) -> ExitCode {
    match run_execute_local(args) {
        Ok(code) => ExitCode::from(code),
        Err(e) => {
            eprintln!("{e}");
            ExitCode::from(1)
        }
    }
}

fn run_execute_local(args: &LocalArgs) -> Result<u8, crate::CliError> {
    let round_type = match args.round_type {
        CodexRoundTypeArg::Fast => "fast".to_owned(),
        CodexRoundTypeArg::Final => "final".to_owned(),
    };

    let input = ReviewInput::RunLocal {
        model: args.model.clone(),
        timeout_seconds: args.timeout_seconds,
        briefing_file: args.briefing_file.clone(),
        prompt: args.prompt.clone(),
        track_id: args.track_id.clone(),
        round_type,
        group: args.group.clone(),
        items_dir: args.items_dir.clone(),
    };

    let outcome = cli_composition::ReviewCompositionRoot::new().review_driver().handle(input);

    if let Some(line) = &outcome.stdout {
        writeln!(io::stdout(), "{line}").map_err(crate::CliError::Io)?;
    }
    if let Some(line) = &outcome.stderr {
        eprintln!("{line}");
    }
    Ok(outcome.exit_code)
}

/// Test-only error type for `resolve_reviewer_for_test`.
#[cfg(test)]
#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub(super) struct LocalTestError(String);

/// Resolves the reviewer provider/model from agent-profiles.json at a given path,
/// returning a fail-closed error when the capability is missing or the provider
/// is unsupported.
///
/// Extracted as a pure function so the fail-closed resolution logic can be tested
/// without spawning a subprocess or hitting the filesystem for the review cycle.
///
/// # Errors
/// Returns an error if the profiles file cannot be loaded, the `reviewer` capability
/// is not defined, or the provider is unsupported.
#[cfg(test)]
pub(super) fn resolve_reviewer_for_test(
    profiles_path: &std::path::Path,
    round_type: CodexRoundTypeArg,
) -> Result<infrastructure::agent_profiles::ResolvedExecution, LocalTestError> {
    let profiles = infrastructure::agent_profiles::AgentProfiles::load(profiles_path)
        .map_err(|e| LocalTestError(format!("[ERROR] failed to load agent-profiles.json: {e}")))?;
    let infra_round_type = match round_type {
        CodexRoundTypeArg::Fast => infrastructure::agent_profiles::RoundType::Fast,
        CodexRoundTypeArg::Final => infrastructure::agent_profiles::RoundType::Final,
    };
    let resolved = profiles.resolve_execution("reviewer", infra_round_type).ok_or_else(|| {
        LocalTestError("[ERROR] reviewer capability not defined in agent-profiles.json".to_owned())
    })?;
    match resolved.provider.as_str() {
        "codex" | "claude" => Ok(resolved),
        other => Err(LocalTestError(format!(
            "[ERROR] unsupported reviewer provider '{other}' \
             (supported: 'codex', 'claude')"
        ))),
    }
}
