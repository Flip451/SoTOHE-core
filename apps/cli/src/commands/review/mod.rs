//! CLI subcommands for local reviewer workflow wrappers.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{ArgGroup, Args, Subcommand};
use cli_driver::review::{ReviewCheckRoundSelect, ReviewCheckZeroFindingsInput, ReviewInput};
#[cfg(test)]
use usecase::review_v2::{ReviewApprovalDecision, ReviewApprovalOutput};

mod classify;
mod files;
mod fix_local;
mod local;
mod results;
#[cfg(test)]
mod tests;

use classify::{ClassifyArgs, execute_classify};
use files::{FilesArgs, execute_files};
use fix_local::{FixLocalArgs, execute_fix_local};
use local::{LocalArgs, execute_local};
use results::execute_results;

const DEFAULT_TIMEOUT_SECONDS: u64 = 3_600;

#[derive(Debug, Subcommand)]
pub enum ReviewCommand {
    /// Run the local reviewer with provider auto-resolved from agent-profiles.json.
    Local(LocalArgs),
    /// Run the review-fix-lead fixer with provider auto-resolved from agent-profiles.json.
    ///
    /// Resolves `review-fix-lead` capability from agent-profiles.json, constructs
    /// the fixer (currently Codex only), and executes the fix cycle. Accepts
    /// `--scope` / `--briefing-file` / `--round-type` (required) plus optional
    /// `--track-id` (auto-resolved from the current git branch when omitted) and
    /// optional `--model` override. The reviewer model and scope boundary are
    /// self-resolved by the fixer skill (ADR 2026-06-01-2300 D1/D3).
    FixLocal(FixLocalArgs),
    /// Check if review is approved for commit.
    CheckApproved(ReviewCheckApprovedArgs),
    /// Check whether a scope has a current final zero-findings verdict.
    CheckZeroFindings(CheckZeroFindingsArgs),
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
pub struct ReviewCheckApprovedArgs {
    /// Path to the track items directory.
    #[arg(long, default_value = "track/items")]
    items_dir: PathBuf,

    /// Track ID.
    /// When omitted, resolved from the current git branch (`track/<id>`).
    #[arg(long)]
    track_id: Option<String>,
}

/// CLI arguments for `review check-zero-findings`.
///
/// Registration in [`ReviewCommand`] and execution wiring are deliberately
/// owned by T042, which finalizes that command enum's shape.
#[derive(Debug, Args)]
pub struct CheckZeroFindingsArgs {
    /// Path to the track items directory.
    #[arg(long, default_value = "track/items")]
    items_dir: PathBuf,

    /// Track ID. When omitted, it is resolved from the current track branch.
    #[arg(long)]
    track_id: Option<String>,

    /// Review scope whose final verdict is checked.
    #[arg(long)]
    scope: String,

    /// The only convergence-eligible review round.
    #[arg(long, value_enum)]
    round: ReviewCheckRoundArg,
}

/// Round selector accepted by `review check-zero-findings`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum ReviewCheckRoundArg {
    Final,
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

pub fn execute(cmd: ReviewCommand) -> ExitCode {
    match cmd {
        ReviewCommand::Local(args) => execute_local(&args),
        ReviewCommand::FixLocal(args) => execute_fix_local(&args),
        ReviewCommand::CheckApproved(args) => execute_check_approved(&args),
        ReviewCommand::CheckZeroFindings(args) => execute_check_zero_findings(&args),
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
#[cfg(test)]
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

fn execute_check_approved(args: &ReviewCheckApprovedArgs) -> ExitCode {
    let track_id =
        match crate::commands::track::resolve_track_id(args.track_id.clone(), &args.items_dir) {
            Ok(id) => id,
            Err(msg) => {
                eprintln!("{msg}");
                return ExitCode::FAILURE;
            }
        };
    let outcome = cli_composition::ReviewCompositionRoot::new()
        .review_driver()
        .handle(ReviewInput::CheckApproved(track_id, args.items_dir.clone()));
    if let Some(msg) = &outcome.stdout {
        println!("{msg}");
    }
    if let Some(msg) = &outcome.stderr {
        eprintln!("{msg}");
    }
    ExitCode::from(outcome.exit_code)
}

fn execute_check_zero_findings(args: &CheckZeroFindingsArgs) -> ExitCode {
    let track_id =
        match crate::commands::track::resolve_track_id(args.track_id.clone(), &args.items_dir) {
            Ok(track_id) => track_id,
            Err(message) => {
                eprintln!("{message}");
                return ExitCode::FAILURE;
            }
        };
    let round = match args.round {
        ReviewCheckRoundArg::Final => ReviewCheckRoundSelect::Final,
    };
    let input = match ReviewCheckZeroFindingsInput::try_new(
        args.items_dir.clone(),
        track_id,
        args.scope.clone(),
        round,
    ) {
        Ok(input) => input,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::FAILURE;
        }
    };
    let outcome = cli_composition::ReviewCompositionRoot::new()
        .review_driver()
        .handle(ReviewInput::CheckZeroFindings(input));
    if let Some(message) = &outcome.stdout {
        println!("{message}");
    }
    if let Some(message) = &outcome.stderr {
        eprintln!("{message}");
    }
    ExitCode::from(outcome.exit_code)
}
