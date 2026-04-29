//! CLI subcommands for local reviewer workflow wrappers.

#[cfg(test)]
use std::ffi::OsString;
use std::path::PathBuf;
use std::process::ExitCode;
#[cfg(test)]
use std::time::Duration;

use clap::{ArgGroup, Args, Subcommand};
use domain::review_v2::ReviewExistsPort;
#[cfg(test)]
use usecase::review_workflow::ReviewVerdict;

mod codex_local;
mod compose_v2;
#[cfg(test)]
mod tests;

use codex_local::execute_codex_local;

const DEFAULT_TIMEOUT_SECONDS: u64 = 1800;

#[cfg(test)]
pub(super) const REVIEW_RUNTIME_DIR: &str = "tmp/reviewer-runtime";
#[cfg(test)]
pub(super) const POLL_INTERVAL: Duration = Duration::from_millis(50);
#[cfg(test)]
pub(super) const CODEX_BIN_ENV: &str = "SOTP_CODEX_BIN";

#[derive(Debug, Subcommand)]
pub enum ReviewCommand {
    /// Run the local Codex-backed reviewer and auto-record verdict to review.json.
    CodexLocal(CodexLocalArgs),
    /// Check if review is approved for commit.
    CheckApproved(CheckApprovedArgs),
    /// Show per-scope review state for a track.
    Status(StatusArgs),
}

/// CLI round type for auto-record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum CodexRoundTypeArg {
    Fast,
    Final,
}

#[derive(Debug, Args)]
#[command(group(
    ArgGroup::new("review_input")
        .required(true)
        .args(["briefing_file", "prompt"])
))]
pub struct CodexLocalArgs {
    /// Model name resolved from `.harness/config/agent-profiles.json`.
    #[arg(long)]
    pub(super) model: String,

    /// Timeout for the reviewer subprocess in seconds.
    #[arg(long, default_value_t = DEFAULT_TIMEOUT_SECONDS)]
    pub(super) timeout_seconds: u64,

    /// Path to a briefing file that the reviewer should read.
    #[arg(long)]
    pub(super) briefing_file: Option<PathBuf>,

    /// Inline prompt for the reviewer.
    #[arg(long)]
    pub(super) prompt: Option<String>,

    /// Test-only explicit path where the wrapper should ask Codex to write the final message.
    #[cfg(test)]
    #[arg(long, hide = true)]
    pub(super) output_last_message: Option<PathBuf>,

    /// Track ID (used for auto-recording verdict to review.json).
    #[arg(long)]
    pub(super) track_id: String,

    /// Round type: fast or final.
    #[arg(long, value_enum)]
    pub(super) round_type: CodexRoundTypeArg,

    /// Review scope name (e.g., "domain", "infrastructure", "other").
    #[arg(long)]
    pub(super) group: String,

    /// Path to track items directory.
    #[arg(long, default_value = "track/items")]
    pub(super) items_dir: PathBuf,
}

/// Validated auto-record arguments ready for use after Codex completes.
#[derive(Debug)]
#[allow(dead_code)] // expected_groups used by v1 test stubs (pending cleanup)
pub(super) struct ValidatedAutoRecordArgs {
    pub(super) track_id: domain::TrackId,
    pub(super) round_type: domain::RoundType,
    pub(super) group_name: domain::ReviewGroupName,
    pub(super) expected_groups: Vec<domain::ReviewGroupName>,
    pub(super) items_dir: PathBuf,
    pub(super) diff_base: String,
}

/// Validates and parses auto-record arguments from `CodexLocalArgs`.
///
/// All record fields are now required (auto-record is always on).
///
/// # Errors
/// Returns a human-readable error string if args are invalid.
pub(super) fn validate_auto_record_args(
    args: &CodexLocalArgs,
) -> Result<ValidatedAutoRecordArgs, String> {
    let track_id =
        domain::TrackId::try_new(&args.track_id).map_err(|e| format!("invalid --track-id: {e}"))?;
    let group_name = domain::ReviewGroupName::try_new(&args.group)
        .map_err(|e| format!("invalid --group: {e}"))?;

    let round_type = match args.round_type {
        CodexRoundTypeArg::Fast => domain::RoundType::Fast,
        CodexRoundTypeArg::Final => domain::RoundType::Final,
    };

    Ok(ValidatedAutoRecordArgs {
        track_id,
        round_type,
        group_name,
        expected_groups: Vec::new(),
        items_dir: args.items_dir.clone(),
        diff_base: String::new(),
    })
}

#[derive(Debug, Args)]
pub struct CheckApprovedArgs {
    /// Path to the track items directory.
    #[arg(long, default_value = "track/items")]
    items_dir: PathBuf,

    /// Track ID.
    #[arg(long)]
    track_id: String,
}

#[derive(Debug, Args)]
pub struct StatusArgs {
    /// Path to the track items directory.
    #[arg(long, default_value = "track/items")]
    items_dir: PathBuf,

    /// Track ID.
    #[arg(long)]
    track_id: String,
}

// These types are only needed by the test shim in codex_local.rs.
#[cfg(test)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ReviewRunResult {
    pub(super) verdict: ReviewVerdict,
    pub(super) final_message: Option<String>,
    pub(super) output_last_message: PathBuf,
    pub(super) output_last_message_auto_managed: bool,
    pub(super) verdict_detail: Option<String>,
}

#[cfg(test)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct CodexInvocation {
    pub(super) bin: OsString,
    pub(super) args: Vec<OsString>,
}

#[cfg(test)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct OutputLastMessagePath {
    pub(super) path: PathBuf,
    pub(super) auto_managed: bool,
}

pub fn execute(cmd: ReviewCommand) -> ExitCode {
    match cmd {
        ReviewCommand::CodexLocal(args) => execute_codex_local(&args),
        ReviewCommand::CheckApproved(args) => execute_check_approved(&args),
        ReviewCommand::Status(args) => execute_status(&args),
    }
}

// ---------------------------------------------------------------------------
// check-approved: Verify review.status == approved with current code hash
// ---------------------------------------------------------------------------

/// Formats an `ReviewApprovalVerdict` into the human-readable message and exit
/// code for the `check-approved` command.
///
/// Extracted as a pure function so that tests can assert on the *exact* message
/// prefix (`[OK]` / `[WARN]` / `[BLOCKED]`) without having to redirect stderr.
///
/// Observable surface (AC-10):
/// - `Approved`            → `[OK] …`   + `ExitCode::SUCCESS`
/// - `ApprovedWithBypass`  → `[WARN] …` + `ExitCode::SUCCESS`
/// - `Blocked`             → `[BLOCKED] …` + `ExitCode::FAILURE`
#[cfg_attr(not(test), allow(dead_code))]
pub(super) fn format_approval_verdict(
    verdict: domain::review_v2::ReviewApprovalVerdict,
) -> (String, ExitCode) {
    use domain::review_v2::ReviewApprovalVerdict;
    match verdict {
        ReviewApprovalVerdict::Approved => {
            ("[OK] Review is approved and code hash is current".to_owned(), ExitCode::SUCCESS)
        }
        ReviewApprovalVerdict::ApprovedWithBypass { not_started_count } => (
            format!(
                "[WARN] No review.json found. Allowing commit for PR-based review ({not_started_count} scope(s))."
            ),
            ExitCode::SUCCESS,
        ),
        ReviewApprovalVerdict::Blocked { required_scopes } => {
            let mut display: Vec<_> =
                required_scopes.iter().map(|scope| format!("  {scope}")).collect();
            display.sort();
            (
                format!("[BLOCKED] Review not approved. Required scopes:\n{}", display.join("\n")),
                ExitCode::FAILURE,
            )
        }
    }
}

fn execute_check_approved(args: &CheckApprovedArgs) -> ExitCode {
    match run_check_approved(args) {
        Ok(verdict) => {
            let (msg, code) = format_approval_verdict(verdict);
            eprintln!("{msg}");
            code
        }
        Err(msg) => {
            eprintln!("{msg}");
            ExitCode::FAILURE
        }
    }
}

fn run_check_approved(
    args: &CheckApprovedArgs,
) -> Result<domain::review_v2::ReviewApprovalVerdict, String> {
    let track_id = domain::TrackId::try_new(&args.track_id).map_err(|e| format!("{e}"))?;

    let comp = compose_v2::build_review_v2(&track_id, &args.items_dir)?;

    let review_json_exists = comp
        .review_store
        .review_json_exists()
        .map_err(|e| format!("failed to check review.json existence: {e}"))?;

    comp.cycle
        .evaluate_approval(&comp.review_store, review_json_exists)
        .map_err(|e| format!("failed to evaluate approval: {e}"))
}

// ---------------------------------------------------------------------------
// status: Show per-group Fast/Final review state
// ---------------------------------------------------------------------------

fn execute_status(args: &StatusArgs) -> ExitCode {
    match run_status(args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(msg) => {
            eprintln!("{msg}");
            ExitCode::FAILURE
        }
    }
}

fn run_status(args: &StatusArgs) -> Result<(), String> {
    use domain::review_v2::{NotRequiredReason, ReviewState};

    let track_id =
        domain::TrackId::try_new(&args.track_id).map_err(|e| format!("invalid track id: {e}"))?;
    let comp = compose_v2::build_review_v2(&track_id, &args.items_dir)?;

    let states = comp
        .cycle
        .get_review_states(&comp.review_store)
        .map_err(|e| format!("failed to get review states: {e}"))?;

    if states.is_empty() {
        println!("Review status: no scopes (empty diff)");
        return Ok(());
    }

    println!("Review status (v2 scope-based):");
    println!("Diff base: {}", comp.base);

    let mut sorted: Vec<_> = states.iter().collect();
    sorted.sort_by_key(|(scope, _)| scope.to_string());

    let mut approved_count = 0;
    let mut empty_count = 0;
    let mut required_count = 0;
    for (scope, state) in &sorted {
        let indicator = match state {
            ReviewState::Required(_) => {
                required_count += 1;
                "[-]"
            }
            ReviewState::NotRequired(NotRequiredReason::Empty) => {
                empty_count += 1;
                "[.]"
            }
            ReviewState::NotRequired(NotRequiredReason::ZeroFindings) => {
                approved_count += 1;
                "[+]"
            }
        };
        println!("  {indicator} {scope}: {state}");
    }

    println!(
        "Summary: {approved_count} approved, {empty_count} empty, {required_count} required, {} total",
        sorted.len()
    );

    Ok(())
}
