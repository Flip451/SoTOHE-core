//! CLI subcommands for local reviewer workflow wrappers.

#[cfg(test)]
use std::ffi::OsString;
use std::path::PathBuf;
use std::process::ExitCode;
#[cfg(test)]
use std::time::Duration;

use clap::{ArgGroup, Args, Subcommand};
use usecase::review_v2::{ReviewApprovalDecision, ReviewApprovalOutput};
#[cfg(test)]
use usecase::review_workflow::ReviewVerdict;

mod classify;
mod claude_local;
mod codex_local;
mod compose_v2;
mod files;
mod local;
mod results;
#[cfg(test)]
mod tests;

use classify::{ClassifyArgs, execute_classify};
use claude_local::execute_claude_local;
use codex_local::execute_codex_local;
use files::{FilesArgs, execute_files};
use local::{LocalArgs, execute_local};
use results::execute_results;

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
    /// Run the local Claude-backed reviewer and auto-record verdict to review.json.
    ClaudeLocal(ClaudeLocalArgs),
    /// Run the local reviewer with provider auto-resolved from agent-profiles.json.
    Local(LocalArgs),
    /// Check if review is approved for commit.
    CheckApproved(CheckApprovedArgs),
    /// Show review results: per-scope state summary, optional round history, and a commit hint.
    ///
    /// Read-only canonical API replacing direct `review.json` access. With `--limit 0`
    /// (the default) the output is the state summary only — the equivalent of the
    /// removed `sotp review status` subcommand.
    Results(ResultsArgs),
    /// Classify each given path into review scopes (`<path>TAB<scope-csv>` lines).
    ///
    /// Pure-logic command: validates paths via `FilePath::new` and consults the
    /// scope config without invoking the diff getter.
    Classify(ClassifyArgs),
    /// List the diff files belonging to the given scope (one path per line).
    ///
    /// Validates the scope name before any diff I/O (AC-08); unknown names
    /// produce a stderr message and `ExitCode::FAILURE` without touching git.
    Files(FilesArgs),
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
}

/// CLI args for `sotp review claude-local`.
#[derive(Debug, Args)]
#[command(group(
    ArgGroup::new("claude_review_input")
        .required(true)
        .args(["briefing_file", "prompt"])
))]
pub struct ClaudeLocalArgs {
    /// Model name for the Claude reviewer.
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
}

/// Validated auto-record arguments ready for use after the reviewer completes.
///
/// Stores all fields as plain strings / standard types so the CLI module
/// never needs to import domain types (CN-01 / AC-03). The conversion to
/// `domain::TrackId`, `domain::RoundType`, and `domain::ReviewGroupName`
/// happens inside `infrastructure::review_v2::run_*_review_str` when
/// the review cycle is actually executed.
#[derive(Debug)]
#[allow(dead_code)] // expected_groups preserved for API compatibility
pub(super) struct ValidatedAutoRecordArgs {
    pub(super) track_id: String,
    pub(super) round_type_str: String, // "fast" | "final"
    pub(super) group_name: String,
    pub(super) expected_groups: Vec<String>,
    pub(super) items_dir: PathBuf,
    pub(super) diff_base: String,
}

/// Shared validation logic for auto-record arguments (raw strings).
///
/// Used by both `validate_auto_record_args` (CodexLocalArgs) and
/// `validate_claude_auto_record_args` (ClaudeLocalArgs).
///
/// # Errors
/// Returns a human-readable error string if args are invalid.
pub(super) fn validate_auto_record_args_raw(
    track_id: &str,
    group: &str,
    round_type: CodexRoundTypeArg,
    items_dir: PathBuf,
) -> Result<ValidatedAutoRecordArgs, String> {
    // Validate track ID format via infrastructure helper (no domain import needed).
    infrastructure::review_v2::validate_track_id_str(track_id)
        .map_err(|e| format!("invalid --track-id: {e}"))?;
    // Validate group name format via infrastructure helper.
    infrastructure::review_v2::validate_review_group_name_str(group)
        .map_err(|e| format!("invalid --group: {e}"))?;

    let round_type_str = match round_type {
        CodexRoundTypeArg::Fast => "fast",
        CodexRoundTypeArg::Final => "final",
    };

    // `validate_review_group_name_str` accepts inputs with leading/trailing
    // whitespace because `domain::ReviewGroupName::try_new` trims before
    // validation. Propagate the trimmed value so downstream scope lookup uses
    // the canonical form (otherwise " domain " would pass validation but then
    // fail unknown-scope on lookup).
    let group_name = group.trim().to_owned();

    Ok(ValidatedAutoRecordArgs {
        track_id: track_id.to_owned(),
        round_type_str: round_type_str.to_owned(),
        group_name,
        expected_groups: Vec::new(),
        items_dir,
        diff_base: String::new(),
    })
}

/// Validates and parses auto-record arguments from `CodexLocalArgs`.
///
/// All record fields are now required (auto-record is always on). Validation
/// is performed using the infrastructure crate's string-based parsing helpers
/// so that no domain types appear in the CLI module (CN-01 / AC-03).
/// When `track_id` is `None`, the active track is resolved from the current
/// git branch (CN-01, AC-01).
///
/// # Errors
/// Returns a human-readable error string if args are invalid.
pub(super) fn validate_auto_record_args(
    args: &CodexLocalArgs,
) -> Result<ValidatedAutoRecordArgs, String> {
    let track_id =
        crate::commands::track::resolve_track_id(args.track_id.clone(), &args.items_dir)?;
    validate_auto_record_args_raw(&track_id, &args.group, args.round_type, args.items_dir.clone())
}

/// Validates and parses auto-record arguments from `ClaudeLocalArgs`.
///
/// When `track_id` is `None`, the active track is resolved from the current
/// git branch (CN-01, AC-01).
///
/// # Errors
/// Returns a human-readable error string if args are invalid.
pub(super) fn validate_claude_auto_record_args(
    args: &ClaudeLocalArgs,
) -> Result<ValidatedAutoRecordArgs, String> {
    let track_id =
        crate::commands::track::resolve_track_id(args.track_id.clone(), &args.items_dir)?;
    validate_auto_record_args_raw(&track_id, &args.group, args.round_type, args.items_dir.clone())
}

#[derive(Debug, Args)]
pub struct CheckApprovedArgs {
    /// Path to the track items directory.
    #[arg(long, default_value = "track/items")]
    items_dir: PathBuf,

    /// Track ID.
    /// When omitted, resolved from the current git branch (`track/<id>`).
    #[arg(long)]
    track_id: Option<String>,
}

/// Round-type filter for `sotp review results --round-type ...`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum RoundTypeFilter {
    /// Include only fast rounds.
    Fast,
    /// Include only final rounds.
    Final,
    /// Include all rounds (default).
    Any,
}

/// `--limit` value: `0` (state summary only, default) | `N` (a positive integer) | `all`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResultsLimit {
    /// `--limit 0` — state summary only.
    Zero,
    /// `--limit N` (where `N >= 1`) — show up to `N` recent rounds.
    Count(u32),
    /// `--limit all` — show every round.
    All,
}

impl std::str::FromStr for ResultsLimit {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.eq_ignore_ascii_case("all") {
            return Ok(Self::All);
        }
        match s.parse::<u32>() {
            Ok(0) => Ok(Self::Zero),
            Ok(n) => Ok(Self::Count(n)),
            Err(_) => Err(format!(
                "invalid --limit value: '{s}' (expected non-negative integer or 'all')"
            )),
        }
    }
}

#[derive(Debug, Args)]
#[command(group(
    ArgGroup::new("scope_selector")
        .args(["scope", "all"])
        .multiple(false)
))]
pub struct ResultsArgs {
    /// Path to the track items directory.
    #[arg(long, default_value = "track/items")]
    pub(super) items_dir: PathBuf,

    /// Track ID.
    /// When omitted, resolved from the current git branch (`track/<id>`).
    #[arg(long)]
    pub(super) track_id: Option<String>,

    /// Show only the named scope (mutually exclusive with `--all`).
    #[arg(long)]
    pub(super) scope: Option<String>,

    /// Show every scope (equivalent to omitting `--scope`; mutually exclusive with `--scope`).
    #[arg(long, default_value_t = false)]
    pub(super) all: bool,

    /// `0` (state summary only, default), a positive integer `N`, or `all`.
    #[arg(long, default_value = "0")]
    pub(super) limit: ResultsLimit,

    /// Round-type filter applied to history rounds.
    #[arg(long, value_enum, default_value_t = RoundTypeFilter::Any)]
    pub(super) round_type: RoundTypeFilter,

    /// Suppress the commit hint line.
    #[arg(long)]
    pub(super) no_hint: bool,
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
        ReviewCommand::ClaudeLocal(args) => execute_claude_local(&args),
        ReviewCommand::Local(args) => execute_local(&args),
        ReviewCommand::CheckApproved(args) => execute_check_approved(&args),
        ReviewCommand::Results(args) => execute_results(&args),
        ReviewCommand::Classify(args) => execute_classify(&args),
        ReviewCommand::Files(args) => execute_files(&args),
    }
}

// ---------------------------------------------------------------------------
// check-approved: Verify review.status == approved with current code hash
// ---------------------------------------------------------------------------

/// Formats a `ReviewApprovalOutput` into the human-readable message and exit
/// code for the `check-approved` command.
///
/// Extracted as a pure function so that tests can assert on the *exact* message
/// prefix (`[OK]` / `[WARN]` / `[BLOCKED]`) without having to redirect stderr.
///
/// Observable surface (AC-10):
/// - `Approved`            → `[OK] …`   + `ExitCode::SUCCESS`
/// - `ApprovedWithBypass`  → `[WARN] …` + `ExitCode::SUCCESS`
/// - `Blocked`             → `[BLOCKED] …` + `ExitCode::FAILURE`
pub(super) fn format_approval_verdict(output: ReviewApprovalOutput) -> (String, ExitCode) {
    match output.decision {
        ReviewApprovalDecision::Approved => {
            ("[OK] Review is approved and code hash is current".to_owned(), ExitCode::SUCCESS)
        }
        ReviewApprovalDecision::ApprovedWithBypass => {
            let count = output.bypass_scope_count.unwrap_or(0);
            (
                format!(
                    "[WARN] No review.json found. Allowing commit for PR-based review ({count} scope(s))."
                ),
                ExitCode::SUCCESS,
            )
        }
        ReviewApprovalDecision::Blocked => {
            let mut display: Vec<_> =
                output.blocked_scopes.iter().map(|scope| format!("  {scope}")).collect();
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
        Ok(output) => {
            let (msg, code) = format_approval_verdict(output);
            eprintln!("{msg}");
            code
        }
        Err(msg) => {
            eprintln!("{msg}");
            ExitCode::FAILURE
        }
    }
}

fn run_check_approved(args: &CheckApprovedArgs) -> Result<ReviewApprovalOutput, String> {
    let track_id =
        crate::commands::track::resolve_track_id(args.track_id.clone(), &args.items_dir)?;
    infrastructure::review_v2::check_approved_str(&track_id, &args.items_dir)
        .map_err(|e| format!("{e}"))
}
