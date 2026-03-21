//! CLI subcommands for local reviewer workflow wrappers.

use std::ffi::OsString;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;

use clap::{ArgGroup, Args, Subcommand};
use usecase::review_workflow::ReviewVerdict;

mod codex_local;
#[cfg(test)]
mod tests;

use codex_local::execute_codex_local;

const DEFAULT_TIMEOUT_SECONDS: u64 = 600;

fn make_timestamp() -> Result<domain::Timestamp, String> {
    let s = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    domain::Timestamp::new(s).map_err(|e| format!("invalid timestamp: {e}"))
}
pub(super) const REVIEW_RUNTIME_DIR: &str = "tmp/reviewer-runtime";
pub(super) const POLL_INTERVAL: Duration = Duration::from_millis(50);
#[cfg(test)]
pub(super) const CODEX_BIN_ENV: &str = "SOTP_CODEX_BIN";

#[derive(Debug, Subcommand)]
pub enum ReviewCommand {
    /// Run the local Codex-backed reviewer through a repo-owned wrapper.
    CodexLocal(CodexLocalArgs),
    /// Record a review round result into metadata.json.
    RecordRound(RecordRoundArgs),
    /// Check if review is approved for commit.
    CheckApproved(CheckApprovedArgs),
    /// Resolve an active review escalation block.
    ResolveEscalation(ResolveEscalationArgs),
}

#[derive(Debug, Args)]
#[command(group(
    ArgGroup::new("review_input")
        .required(true)
        .args(["briefing_file", "prompt"])
))]
pub struct CodexLocalArgs {
    /// Model name resolved from `.claude/agent-profiles.json`.
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
}

#[derive(Debug, Args)]
pub struct RecordRoundArgs {
    /// Round type: fast or final.
    #[arg(long)]
    round_type: String,

    /// Review group name (e.g., "infra-domain").
    #[arg(long)]
    group: String,

    /// Verdict JSON string (e.g., '{"verdict":"zero_findings","findings":[]}').
    #[arg(long)]
    verdict: String,

    /// Comma-separated list of expected group names.
    #[arg(long)]
    expected_groups: String,

    /// Comma-separated list of concern slugs for escalation tracking.
    /// Extracted from reviewer findings. Empty for zero_findings rounds.
    #[arg(long, default_value = "")]
    concerns: String,

    /// Path to the track items directory.
    #[arg(long, default_value = "track/items")]
    items_dir: PathBuf,

    /// Track ID.
    #[arg(long)]
    track_id: String,
}

#[derive(Debug, Args)]
pub struct ResolveEscalationArgs {
    /// Track ID.
    #[arg(long)]
    track_id: String,

    /// Comma-separated list of blocked concerns to resolve.
    /// Must match the concerns currently blocking escalation.
    #[arg(long)]
    blocked_concerns: String,

    /// Path to workspace search artifact.
    #[arg(long)]
    workspace_search_ref: String,

    /// Path to reinvention check artifact.
    #[arg(long)]
    reinvention_check_ref: String,

    /// Decision: adopt_workspace, adopt_crate, or continue_self.
    #[arg(long)]
    decision: String,

    /// Summary of the decision rationale.
    #[arg(long)]
    summary: String,

    /// Path to the track items directory.
    #[arg(long, default_value = "track/items")]
    items_dir: PathBuf,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ReviewRunResult {
    pub(super) verdict: ReviewVerdict,
    pub(super) final_message: Option<String>,
    pub(super) output_last_message: PathBuf,
    pub(super) output_last_message_auto_managed: bool,
    pub(super) verdict_detail: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct CodexInvocation {
    pub(super) bin: OsString,
    pub(super) args: Vec<OsString>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RenderedCommandResult {
    pub(super) exit_code: u8,
    pub(super) stdout_lines: Vec<String>,
    pub(super) stderr_lines: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct OutputLastMessagePath {
    pub(super) path: PathBuf,
    pub(super) auto_managed: bool,
}

#[derive(Debug)]
pub(super) struct AutoManagedArtifacts {
    paths: Vec<PathBuf>,
}

impl AutoManagedArtifacts {
    pub(super) fn new<'a>(artifacts: impl IntoIterator<Item = &'a OutputLastMessagePath>) -> Self {
        Self {
            paths: artifacts
                .into_iter()
                .filter(|artifact| artifact.auto_managed)
                .map(|artifact| artifact.path.clone())
                .collect(),
        }
    }
}

impl Drop for AutoManagedArtifacts {
    fn drop(&mut self) {
        for path in &self.paths {
            let _ = std::fs::remove_file(path);
        }
    }
}

pub fn execute(cmd: ReviewCommand) -> ExitCode {
    match cmd {
        ReviewCommand::CodexLocal(args) => execute_codex_local(&args),
        ReviewCommand::RecordRound(args) => execute_record_round(&args),
        ReviewCommand::CheckApproved(args) => execute_check_approved(&args),
        ReviewCommand::ResolveEscalation(args) => execute_resolve_escalation(&args),
    }
}

// ---------------------------------------------------------------------------
// record-round: Write review round results to metadata.json
// ---------------------------------------------------------------------------

/// Exit code used when a review escalation block prevents recording a round.
const EXIT_CODE_ESCALATION_BLOCKED: u8 = 3;

fn execute_record_round(args: &RecordRoundArgs) -> ExitCode {
    match run_record_round(args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(usecase::review_workflow::usecases::RecordRoundError::EscalationBlocked(concerns)) => {
            eprintln!(
                "[BLOCKED] Review escalation active for concerns: {concerns:?}\n\
                 Required actions:\n\
                 \x20 1. Workspace Search: use Grep to check if existing code solves this problem\n\
                 \x20 2. Reinvention Check: invoke researcher capability to survey crates.io\n\
                 \x20 3. Decision: run `sotp review resolve-escalation` with evidence"
            );
            ExitCode::from(EXIT_CODE_ESCALATION_BLOCKED)
        }
        Err(usecase::review_workflow::usecases::RecordRoundError::Other(msg)) => {
            eprintln!("{msg}");
            ExitCode::FAILURE
        }
    }
}

fn run_record_round(
    args: &RecordRoundArgs,
) -> Result<(), usecase::review_workflow::usecases::RecordRoundError> {
    let protocol = infrastructure::review_adapters::RecordRoundProtocolImpl {
        items_dir: args.items_dir.clone(),
        group_display: args.group.clone(),
    };
    let timestamp =
        make_timestamp().map_err(usecase::review_workflow::usecases::RecordRoundError::Other)?;
    let input = usecase::review_workflow::usecases::RecordRoundInput {
        round_type: args.round_type.clone(),
        group: args.group.clone(),
        verdict: args.verdict.clone(),
        expected_groups: args.expected_groups.clone(),
        concerns: args.concerns.clone(),
        items_dir: args.items_dir.clone(),
        track_id: args.track_id.clone(),
        timestamp,
    };
    usecase::review_workflow::usecases::record_round(input, &protocol)
}

// ---------------------------------------------------------------------------
// resolve-escalation: Resolve an active review escalation block
// ---------------------------------------------------------------------------

fn execute_resolve_escalation(args: &ResolveEscalationArgs) -> ExitCode {
    match run_resolve_escalation(args) {
        Ok(decision) => {
            println!("[OK] Escalation resolved: {decision}");
            ExitCode::SUCCESS
        }
        Err(msg) => {
            eprintln!("{msg}");
            ExitCode::FAILURE
        }
    }
}

fn run_resolve_escalation(args: &ResolveEscalationArgs) -> Result<String, String> {
    // Validate artifact paths exist before calling usecase.
    if !std::path::Path::new(&args.workspace_search_ref).exists() {
        return Err(format!("workspace search artifact not found: {}", args.workspace_search_ref));
    }
    if !std::path::Path::new(&args.reinvention_check_ref).exists() {
        return Err(format!(
            "reinvention check artifact not found: {}",
            args.reinvention_check_ref
        ));
    }
    let store = infrastructure::track::fs_store::FsTrackStore::new(&args.items_dir);
    let timestamp = make_timestamp()?;
    let input = usecase::review_workflow::usecases::ResolveEscalationInput {
        track_id: args.track_id.clone(),
        blocked_concerns: args.blocked_concerns.clone(),
        workspace_search_ref: args.workspace_search_ref.clone(),
        reinvention_check_ref: args.reinvention_check_ref.clone(),
        decision: args.decision.clone(),
        summary: args.summary.clone(),
        items_dir: args.items_dir.clone(),
        timestamp,
    };
    usecase::review_workflow::usecases::resolve_escalation(input, &store)
}

// ---------------------------------------------------------------------------
// check-approved: Verify review.status == approved with current code hash
// ---------------------------------------------------------------------------

fn execute_check_approved(args: &CheckApprovedArgs) -> ExitCode {
    match run_check_approved(args) {
        Ok(()) => {
            eprintln!("[OK] Review is approved and code hash is current");
            ExitCode::SUCCESS
        }
        Err(msg) => {
            eprintln!("{msg}");
            ExitCode::FAILURE
        }
    }
}

fn run_check_approved(args: &CheckApprovedArgs) -> Result<(), String> {
    let store = infrastructure::track::fs_store::FsTrackStore::new(&args.items_dir);
    let hasher = infrastructure::review_adapters::SystemGitHasher;
    let input = usecase::review_workflow::usecases::CheckApprovedInput {
        items_dir: args.items_dir.clone(),
        track_id: args.track_id.clone(),
    };
    usecase::review_workflow::usecases::check_approved(input, &store, &store, &hasher)
}
