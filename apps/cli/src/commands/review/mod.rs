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

const DEFAULT_TIMEOUT_SECONDS: u64 = 1800;

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
    /// Show per-group Fast/Final review state for a track.
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

    /// Enable auto-record: call record-round internally after verdict extraction.
    #[arg(long, default_value_t = false)]
    pub(super) auto_record: bool,

    /// Track ID for auto-record (required when --auto-record is set).
    #[arg(long, requires = "auto_record")]
    pub(super) track_id: Option<String>,

    /// Round type for auto-record (required when --auto-record is set).
    #[arg(long, requires = "auto_record", value_enum)]
    pub(super) round_type: Option<CodexRoundTypeArg>,

    /// Review group name for auto-record (required when --auto-record is set).
    #[arg(long, requires = "auto_record")]
    pub(super) group: Option<String>,

    /// Comma-separated expected group names for auto-record.
    #[arg(long, requires = "auto_record", value_delimiter = ',')]
    pub(super) expected_groups: Vec<String>,

    /// Path to track items directory.
    #[arg(long, default_value = "track/items")]
    pub(super) items_dir: PathBuf,

    /// Base ref for diff scope computation.
    #[arg(long, default_value = "main")]
    pub(super) diff_base: String,
}

/// Pre-validated auto-record arguments ready for use after Codex completes.
#[derive(Debug)]
pub(super) struct ValidatedAutoRecordArgs {
    pub(super) track_id: domain::TrackId,
    pub(super) round_type: domain::RoundType,
    pub(super) group_name: domain::ReviewGroupName,
    pub(super) expected_groups: Vec<domain::ReviewGroupName>,
    pub(super) items_dir: PathBuf,
    pub(super) diff_base: String,
}

/// Validates auto-record arguments before spawning the Codex subprocess.
///
/// When `--auto-record` is set, all required fields must be present.
/// Returns parsed domain types ready for use, or an error message.
///
/// # Errors
///
/// Returns a human-readable error string if required args are missing or invalid.
pub(super) fn validate_auto_record_args(
    args: &CodexLocalArgs,
) -> Result<Option<ValidatedAutoRecordArgs>, String> {
    if !args.auto_record {
        return Ok(None);
    }

    let track_id_str = args.track_id.as_deref().ok_or("--auto-record requires --track-id")?;
    let round_type = args.round_type.ok_or("--auto-record requires --round-type")?;
    let group = args.group.as_deref().ok_or("--auto-record requires --group")?;

    if args.expected_groups.is_empty() {
        return Err("--auto-record requires --expected-groups".to_owned());
    }

    let track_id =
        domain::TrackId::try_new(track_id_str).map_err(|e| format!("invalid --track-id: {e}"))?;
    let group_name =
        domain::ReviewGroupName::try_new(group).map_err(|e| format!("invalid --group: {e}"))?;
    let expected_groups: Vec<domain::ReviewGroupName> = args
        .expected_groups
        .iter()
        .map(|s| {
            domain::ReviewGroupName::try_new(s.trim())
                .map_err(|e| format!("invalid --expected-groups: {e}"))
        })
        .collect::<Result<_, _>>()?;

    // Ensure --group is included in --expected-groups (otherwise the recorded
    // round won't affect approval/escalation logic for this group).
    if !expected_groups.contains(&group_name) {
        return Err(format!(
            "--group '{}' must be included in --expected-groups",
            group_name.as_ref()
        ));
    }

    let round_type = match round_type {
        CodexRoundTypeArg::Fast => domain::RoundType::Fast,
        CodexRoundTypeArg::Final => domain::RoundType::Final,
    };

    Ok(Some(ValidatedAutoRecordArgs {
        track_id,
        round_type,
        group_name,
        expected_groups,
        items_dir: args.items_dir.clone(),
        diff_base: args.diff_base.clone(),
    }))
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

    /// Base ref for diff scope computation.
    #[arg(long, default_value = "main")]
    diff_base: String,
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
        ReviewCommand::Status(args) => execute_status(&args),
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
        base_ref: args.diff_base.clone(),
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
    use domain::ReviewJsonReader;
    use infrastructure::git_cli::{GitRepository, SystemGitRepo};
    use infrastructure::review_group_policy::{
        ResolvedReviewGroupPolicy, load_review_groups_override,
    };
    use usecase::review_workflow::scope::DiffScopeProvider;

    let store = infrastructure::track::fs_store::FsTrackStore::new(&args.items_dir);
    let hasher = infrastructure::review_adapters::SystemGitHasher;
    let review_store = infrastructure::review_json_store::FsReviewJsonStore::new(&args.items_dir);

    // Detect whether this is a planning-only commit from staged files.
    // Fail-closed: if detection fails, assume code files are present.
    let planning_only = detect_planning_only().unwrap_or(false);

    // Compute current partition snapshot only when a review cycle exists.
    // Deferred: planning-only commits with no cycle should not be blocked by
    // snapshot computation errors (e.g., missing base_ref).
    let track_id_parsed = domain::TrackId::try_new(&args.track_id).map_err(|e| format!("{e}"))?;
    let current_snapshot = review_store
        .find_review(&track_id_parsed)
        .map_err(|e| format!("failed to read review.json: {e}"))?
        .and_then(|r| {
            r.current_cycle().map(|c| {
                let base_ref = c.base_ref().to_owned();
                let cycle_group_names: std::collections::BTreeSet<_> =
                    c.group_names().cloned().collect();
                (base_ref, cycle_group_names)
            })
        })
        .map(|(base_ref, cycle_group_names)| -> Result<_, String> {
            let git = SystemGitRepo::discover().map_err(|e| format!("{e}"))?;
            let scope_json = git.root().join("track/review-scope.json");
            let base_groups =
                infrastructure::review_adapters::load_base_review_groups(&scope_json)?;
            let override_config = load_review_groups_override(&args.items_dir, &track_id_parsed)
                .map_err(|e| format!("{e}"))?;

            let base_policy = ResolvedReviewGroupPolicy::resolve(&base_groups, None)
                .map_err(|e| format!("{e}"))?;
            let policy = ResolvedReviewGroupPolicy::resolve(&base_groups, override_config.as_ref())
                .map_err(|e| format!("{e}"))?;

            let diff_scope = infrastructure::review_adapters::GitDiffScopeProvider
                .changed_files(&base_ref)
                .map_err(|e| format!("{e}"))?;
            let diff_files: Vec<_> = diff_scope.files().into_iter().cloned().collect();
            let full_partition = policy.partition(&diff_files).map_err(|e| format!("{e}"))?;

            // Filter partition to match cycle's group set to avoid PartitionChanged
            // false positive when the cycle was created with a subset of groups.
            let other_key =
                domain::ReviewGroupName::try_new("other").map_err(|e| format!("{e}"))?;
            // Filter to cycle's group set, re-mapping non-cycle groups to "other"
            // so their files still contribute to the scope hash (fail-closed).
            let mut filtered = std::collections::BTreeMap::new();
            for (name, paths) in full_partition.groups() {
                if cycle_group_names.contains(name) {
                    filtered.insert(name.clone(), paths.clone());
                } else {
                    filtered.entry(other_key.clone()).or_default().extend(paths.iter().cloned());
                }
            }
            filtered.entry(other_key).or_default();
            let partition = usecase::review_workflow::groups::GroupPartition::try_new(filtered)
                .map_err(|e| format!("{e}"))?;

            Ok(usecase::review_workflow::groups::ReviewPartitionSnapshot::new(
                base_policy.policy_hash(),
                policy.policy_hash(),
                partition,
            ))
        })
        .transpose()?;

    let input = usecase::review_workflow::usecases::CheckApprovedInput {
        items_dir: args.items_dir.clone(),
        track_id: args.track_id.clone(),
        planning_only,
        current_snapshot,
    };
    usecase::review_workflow::usecases::check_approved(
        input,
        &store,
        &store,
        &hasher,
        &review_store,
    )
}

/// Returns `true` if all staged files match the planning-only allowlist.
///
/// Planning-only files are track docs, configuration, and documentation files
/// that do not require a reviewer-approved review cycle before committing.
///
/// Uses `--name-status` instead of `--name-only` to capture both source and
/// destination paths for renames/copies, preventing a code file renamed into
/// a planning-only directory from bypassing the review guard.
fn detect_planning_only() -> Result<bool, String> {
    use infrastructure::git_cli::{GitRepository, SystemGitRepo};

    let git = SystemGitRepo::discover().map_err(|e| format!("git error: {e}"))?;
    let output = git
        .output(&["diff", "--cached", "--name-status", "--diff-filter=ACMRDT"])
        .map_err(|e| format!("git diff error: {e}"))?;
    if !output.status.success() {
        return Err("git diff --cached failed".to_string());
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let paths = extract_paths_from_name_status(&stdout);

    if paths.is_empty() {
        return Ok(true);
    }

    Ok(paths.iter().all(|f| is_planning_only_path(f)))
}

/// Extracts all file paths from `git diff --name-status` output.
///
/// For renames/copies (R/C lines), both source and destination paths are included
/// so that a code file moved into a planning-only directory is still detected.
fn extract_paths_from_name_status(output: &str) -> Vec<&str> {
    let mut paths = Vec::new();
    for line in output.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        // Format: "STATUS\tpath" or "R100\told_path\tnew_path"
        let mut parts = line.split('\t');
        let _status = parts.next(); // skip status column
        if let Some(first_path) = parts.next() {
            paths.push(first_path);
            // Renames/copies have a second path (destination)
            if let Some(second_path) = parts.next() {
                paths.push(second_path);
            }
        }
    }
    paths
}

/// Checks whether a file path belongs to the planning-only allowlist.
///
/// A file is planning-only when it:
/// 1. Resides in an allowed directory prefix (track docs, config, documentation), AND
/// 2. Has a known documentation/config extension (`.md`, `.json`, `.txt`)
///
/// This two-layer check (directory + extension) prevents any code file placed in
/// a documentation directory from bypassing the review guard. Unknown extensions
/// are treated as code (fail-closed).
fn is_planning_only_path(path: &str) -> bool {
    const PREFIXES: &[&str] =
        &["track/", ".claude/commands/", ".claude/docs/", ".claude/rules/", "docs/", "knowledge/"];
    // Exact config files that are always planning-only.
    const EXACT_FILES: &[&str] = &[
        "CLAUDE.md",
        "DEVELOPER_AI_WORKFLOW.md",
        "TRACK_TRACEABILITY.md",
        "README.md",
        ".claude/agent-profiles.json",
        ".claude/settings.json",
        ".claude/settings.local.json",
        "architecture-rules.json",
    ];

    if EXACT_FILES.contains(&path) {
        return true;
    }

    // Root-level .md files (no directory separator) are always planning-only.
    if !path.contains('/') && path.ends_with(".md") {
        return true;
    }

    // Must be in an allowed directory prefix.
    if !PREFIXES.iter().any(|p| path.starts_with(p)) {
        return false;
    }

    // Within allowed directories, only known doc/config extensions are planning-only.
    // Unknown extensions are treated as code (fail-closed).
    const DOC_EXTENSIONS: &[&str] = &[".md", ".json", ".txt", ".csv"];

    DOC_EXTENSIONS.iter().any(|ext| path.ends_with(ext))
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
    use domain::{ReviewJsonReader, TrackId, TrackReader};

    let track_id =
        TrackId::try_new(&args.track_id).map_err(|e| format!("invalid track id: {e}"))?;

    // Verify track exists in metadata.json (fail-closed on orphaned review.json)
    let store = infrastructure::track::fs_store::FsTrackStore::new(&args.items_dir);
    let _track = store
        .find(&track_id)
        .map_err(|e| format!("failed to read track: {e}"))?
        .ok_or_else(|| format!("track '{}' not found", track_id.as_ref()))?;

    // Read review.json (cycle-based model only — no legacy metadata.json fallback)
    let json_store = infrastructure::review_json_store::FsReviewJsonStore::new(&args.items_dir);
    let review_json = json_store
        .find_review(&track_id)
        .map_err(|e| format!("failed to read review.json: {e}"))?;

    match review_json {
        Some(rj) => run_status_review_json(&rj),
        None => {
            println!("Review status: NotStarted (no review.json)");
            Ok(())
        }
    }
}

/// Displays review status from the new cycle-based review.json model.
fn run_status_review_json(review: &domain::ReviewJson) -> Result<(), String> {
    use domain::RoundType;

    let cycle = match review.current_cycle() {
        Some(c) => c,
        None => {
            println!("Review status: NoCycle (review.json exists but no cycles)");
            return Ok(());
        }
    };

    println!("Review source: review.json (cycle-based)");
    println!("Cycle ID:      {}", cycle.cycle_id());
    println!("Started at:    {}", cycle.started_at());
    println!("Base ref:      {}", cycle.base_ref());
    println!("Policy hash:   {}", cycle.policy_hash());

    let group_count = cycle.groups().len();
    if group_count == 0 {
        println!("Groups:        (none)");
    } else {
        println!("Groups ({group_count}):");
        let mut sorted: Vec<_> = cycle.groups().iter().collect();
        sorted.sort_by_key(|(name, _)| name.to_string());

        for (name, state) in sorted {
            let round_count = state.rounds().len();
            let fast_latest = state.latest_round(RoundType::Fast);
            let final_latest = state.latest_round(RoundType::Final);

            let fast_str = format_round_status(fast_latest);
            let final_str = format_round_status(final_latest);

            // Check if final comes after latest fast (required for approval)
            let ordering_ok = state.final_after_latest_fast();
            let ordering_note = if fast_latest.is_some() && final_latest.is_some() && !ordering_ok {
                " ⚠ Final must be rerun after latest Fast"
            } else {
                ""
            };

            println!(
                "  {name} ({round_count} rounds, scope: {} files):{ordering_note}",
                state.scope().len()
            );
            println!("    Fast:  {fast_str}");
            println!("    Final: {final_str}");
        }
    }

    Ok(())
}

/// Formats the latest round status for display, distinguishing zero_findings,
/// findings_remain, and failure outcomes.
fn format_round_status(round: Option<&domain::GroupRound>) -> String {
    use domain::GroupRoundOutcome;
    let Some(r) = round else {
        return "(none)".to_string();
    };
    match r.outcome() {
        GroupRoundOutcome::Success(verdict) => {
            if r.is_successful_zero_findings() {
                format!("zero_findings (hash: {})", truncate_hash(r.hash()))
            } else {
                let count = verdict.findings().len();
                format!("findings_remain ({count} findings, hash: {})", truncate_hash(r.hash()))
            }
        }
        GroupRoundOutcome::Failure { error_message } => {
            let msg = error_message.as_deref().unwrap_or("unknown error");
            format!("FAILURE: {msg} (hash: {})", truncate_hash(r.hash()))
        }
    }
}

/// Truncates a hash string for display (first 16 chars + "...").
fn truncate_hash(hash: &str) -> String {
    // Use char_indices to avoid panic on multi-byte UTF-8
    match hash.char_indices().nth(16) {
        Some((byte_pos, _)) if hash.len() > 20 => format!("{}...", &hash[..byte_pos]),
        _ => hash.to_owned(),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod review_json_status_tests {
    use super::*;
    use domain::{GroupRound, GroupRoundVerdict, RoundType, StoredFinding, Timestamp};

    fn ts(s: &str) -> Timestamp {
        Timestamp::new(s).unwrap()
    }

    #[test]
    fn test_format_round_status_none() {
        assert_eq!(format_round_status(None), "(none)");
    }

    #[test]
    fn test_format_round_status_zero_findings() {
        let round = GroupRound::success(
            RoundType::Fast,
            ts("2026-03-30T10:00:00Z"),
            "short-hash",
            GroupRoundVerdict::ZeroFindings,
        )
        .unwrap();
        let result = format_round_status(Some(&round));
        assert!(result.starts_with("zero_findings"));
        assert!(result.contains("short-hash"));
    }

    #[test]
    fn test_format_round_status_findings_remain() {
        let findings = vec![StoredFinding::new("bug found", None, None, None)];
        let verdict = GroupRoundVerdict::findings_remain(findings).unwrap();
        let round = GroupRound::success(RoundType::Final, ts("2026-03-30T10:00:00Z"), "h", verdict)
            .unwrap();
        let result = format_round_status(Some(&round));
        assert!(result.starts_with("findings_remain"));
        assert!(result.contains("1 findings"));
    }

    #[test]
    fn test_format_round_status_failure() {
        let round = GroupRound::failure(
            RoundType::Fast,
            ts("2026-03-30T10:00:00Z"),
            "h",
            Some("timeout after 300s".into()),
        )
        .unwrap();
        let result = format_round_status(Some(&round));
        assert!(result.starts_with("FAILURE"));
        assert!(result.contains("timeout after 300s"));
    }

    #[test]
    fn test_truncate_hash_short() {
        assert_eq!(truncate_hash("short"), "short");
    }

    #[test]
    fn test_truncate_hash_long() {
        let long = "a".repeat(64);
        let result = truncate_hash(&long);
        assert!(result.ends_with("..."));
        assert_eq!(result.len(), 19); // 16 + "..."
    }
}
