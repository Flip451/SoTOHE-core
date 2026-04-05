//! CLI subcommands for local reviewer workflow wrappers.

#[cfg(test)]
use std::ffi::OsString;
use std::path::PathBuf;
use std::process::ExitCode;
#[cfg(test)]
use std::time::Duration;

use clap::{ArgGroup, Args, Subcommand};
use infrastructure::git_cli::GitRepository;
#[cfg(test)]
use usecase::review_workflow::ReviewVerdict;

mod codex_local;
mod compose_v2;
#[cfg(test)]
mod tests;

use codex_local::execute_codex_local;

const DEFAULT_TIMEOUT_SECONDS: u64 = 1800;

fn make_timestamp() -> Result<domain::Timestamp, String> {
    let s = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    domain::Timestamp::new(s).map_err(|e| format!("invalid timestamp: {e}"))
}
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
    /// Resolve an active review escalation block.
    ResolveEscalation(ResolveEscalationArgs),
    /// Show per-scope review state for a track.
    Status(StatusArgs),
    /// Set approved_head in review.json (recovery command for post-commit persistence failure).
    SetApprovedHead(SetApprovedHeadArgs),
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
#[allow(dead_code)] // expected_groups used by v1 test stubs (T007 cleanup)
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

#[derive(Debug, Args)]
pub struct StatusArgs {
    /// Path to the track items directory.
    #[arg(long, default_value = "track/items")]
    items_dir: PathBuf,

    /// Track ID.
    #[arg(long)]
    track_id: String,
}

#[derive(Debug, Args)]
pub struct SetApprovedHeadArgs {
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
        ReviewCommand::ResolveEscalation(args) => execute_resolve_escalation(&args),
        ReviewCommand::Status(args) => execute_status(&args),
        ReviewCommand::SetApprovedHead(args) => execute_set_approved_head(&args),
    }
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
    use domain::review_v2::ReviewState;

    // v2: no planning-only bypass. All files are classified into scopes.
    // Empty scopes are NotRequired(Empty), reviewed scopes are NotRequired(ZeroFindings).
    // The commit gate simply checks all scopes are NotRequired.
    let track_id = domain::TrackId::try_new(&args.track_id).map_err(|e| format!("{e}"))?;

    // Fail-closed: check metadata.json escalation gate.
    // Escalation state lives in metadata.json until it is migrated to review.json.
    {
        use domain::TrackReader;
        let store = infrastructure::track::fs_store::FsTrackStore::new(&args.items_dir);
        let track = store
            .find(&track_id)
            .map_err(|e| format!("failed to read track: {e}"))?
            .ok_or_else(|| format!("track '{}' not found", track_id.as_ref()))?;
        if let Some(review_state) = track.review() {
            if let domain::EscalationPhase::Blocked(block) = review_state.escalation().phase() {
                let concerns: Vec<_> =
                    block.concerns().iter().map(|c| c.as_ref().to_owned()).collect();
                return Err(format!(
                    "[BLOCKED] Review escalation active for concerns: {concerns:?}. \
                     Run `sotp review resolve-escalation` first."
                ));
            }
        }
    }

    let comp = compose_v2::build_review_v2(&track_id, &args.items_dir)?;

    let states = comp
        .cycle
        .get_review_states(&comp.review_store)
        .map_err(|e| format!("failed to get review states: {e}"))?;

    // Collect scopes that still require review.
    let required: Vec<_> =
        states.iter().filter(|(_, state)| matches!(state, ReviewState::Required(_))).collect();

    if required.is_empty() {
        return Ok(());
    }

    // If review.json does not exist AND all required scopes are NotStarted,
    // allow commit without review. This enables PR-based review workflows
    // where local review is skipped intentionally.
    // When review.json exists but is corrupt/unreadable, the store returns
    // empty state (all NotStarted) as fail-closed — we must NOT bypass in
    // that case, so we require the file to be absent.
    // Resolve review.json relative to the git root (same as build_review_v2)
    // to avoid CWD-dependent path mismatch.
    let git = infrastructure::git_cli::SystemGitRepo::discover()
        .map_err(|e| format!("git discover: {e}"))?;
    let review_json = if args.items_dir.is_absolute() {
        args.items_dir.join(&args.track_id).join("review.json")
    } else {
        git.root().join(&args.items_dir).join(&args.track_id).join("review.json")
    };
    let all_not_started = required.iter().all(|(_, state)| {
        matches!(state, ReviewState::Required(domain::review_v2::RequiredReason::NotStarted))
    });
    if all_not_started && !review_json.exists() {
        eprintln!(
            "[WARN] No review.json found. Allowing commit for PR-based review ({} scope(s)).",
            required.len()
        );
        return Ok(());
    }

    let mut display: Vec<_> =
        required.iter().map(|(scope, state)| format!("  {scope}: {state}")).collect();
    display.sort();
    Err(format!("[BLOCKED] Review not approved. Required scopes:\n{}", display.join("\n")))
}

// ---------------------------------------------------------------------------
// status: Show per-group Fast/Final review state
// ---------------------------------------------------------------------------

fn execute_set_approved_head(args: &SetApprovedHeadArgs) -> ExitCode {
    match run_set_approved_head(args) {
        Ok(()) => {
            eprintln!("[OK] approved_head updated");
            ExitCode::SUCCESS
        }
        Err(msg) => {
            eprintln!("[ERROR] {msg}");
            ExitCode::FAILURE
        }
    }
}

fn run_set_approved_head(args: &SetApprovedHeadArgs) -> Result<(), String> {
    use domain::{ApprovedHead, ReviewJsonReader, ReviewJsonWriter, TrackId};

    let track_id =
        TrackId::try_new(&args.track_id).map_err(|e| format!("invalid track id: {e}"))?;

    // Verify current branch matches the requested track to prevent cross-track corruption.
    let current_branch = std::process::Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .output()
        .map_err(|e| format!("failed to detect branch: {e}"))?;
    if !current_branch.status.success() {
        return Err("failed to detect current branch (git rev-parse failed)".to_owned());
    }
    let branch_name = String::from_utf8_lossy(&current_branch.stdout).trim().to_owned();
    let expected_branch = format!("track/{}", args.track_id);
    if branch_name != expected_branch {
        return Err(format!(
            "current branch '{branch_name}' does not match track branch '{expected_branch}'. \
             Run this command from the correct track branch to prevent cross-track corruption."
        ));
    }

    let store = infrastructure::review_json_store::FsReviewJsonStore::new(&args.items_dir);

    let mut review = store
        .find_review(&track_id)
        .map_err(|e| format!("failed to read review.json: {e}"))?
        .ok_or_else(|| "no review.json found".to_owned())?;

    let cycle =
        review.current_cycle_mut().ok_or_else(|| "no current cycle in review.json".to_owned())?;

    // Resolve HEAD SHA
    let head_output = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .map_err(|e| format!("failed to run git rev-parse: {e}"))?;
    if !head_output.status.success() {
        return Err("git rev-parse HEAD failed".to_owned());
    }
    let head_sha = String::from_utf8_lossy(&head_output.stdout).trim().to_owned();
    let approved_head = ApprovedHead::try_new(&head_sha).map_err(|e| format!("{e}"))?;

    cycle.set_approved_head(approved_head);
    store.save_review(&track_id, &review).map_err(|e| format!("{e}"))?;
    eprintln!("[set-approved-head] Recorded: {head_sha}");
    Ok(())
}

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
