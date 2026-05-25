//! `sotp review local` — unified local reviewer entry point with auto-resolved provider.
//!
//! Loads `agent-profiles.json`, resolves `reviewer` capability for the given
//! round type, and dispatches to `CodexReviewer` (provider=codex) or
//! `ClaudeReviewer` (provider=claude) (CN-03 / CN-04 / AC-01 / AC-04).

use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;

use clap::{ArgGroup, Args};
use infrastructure::agent_profiles::{AGENT_PROFILES_PATH, AgentProfiles, RoundType};
use infrastructure::git_cli::{GitRepository, SystemGitRepo};
use infrastructure::review_v2::{ClaudeReviewer, CodexReviewOutcome, CodexReviewer};

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
        Err(msg) => {
            eprintln!("{msg}");
            ExitCode::from(1)
        }
    }
}

fn run_execute_local(args: &LocalArgs) -> Result<u8, String> {
    // Step 1: Load agent-profiles.json and resolve reviewer capability.
    // Discover the repo root so the profiles path is stable regardless of cwd —
    // identical to the approach used by `pr.rs` (trigger_review / review_cycle).
    let repo = SystemGitRepo::discover()
        .map_err(|e| format!("[ERROR] failed to discover git repository root: {e}"))?;
    let profiles_path = repo.root().join(AGENT_PROFILES_PATH);
    let profiles = AgentProfiles::load(&profiles_path)
        .map_err(|e| format!("[ERROR] failed to load agent-profiles.json: {e}"))?;

    // Step 2: Resolve provider/model for the given round type (fail-closed if undefined, CN-03).
    let infra_round_type = match args.round_type {
        CodexRoundTypeArg::Fast => RoundType::Fast,
        CodexRoundTypeArg::Final => RoundType::Final,
    };
    let mut resolved =
        profiles.resolve_execution("reviewer", infra_round_type).ok_or_else(|| {
            "[ERROR] reviewer capability not defined in agent-profiles.json".to_owned()
        })?;

    // Step 3: Apply optional model override (ad-hoc use only).
    if let Some(model_override) = &args.model {
        resolved.model = Some(model_override.clone());
    }

    // Log resolved provider/model for debuggability (AC-01 / AC-04).
    eprintln!(
        "[sotp review local] provider={} model={}",
        resolved.provider,
        resolved.model.as_deref().unwrap_or("<none>")
    );

    // Step 4: Dispatch to the appropriate reviewer implementation.
    match resolved.provider.as_str() {
        "codex" => {
            let model = resolved.model.ok_or_else(|| {
                "[ERROR] codex reviewer requires a model (set model in agent-profiles.json)"
                    .to_owned()
            })?;
            dispatch_codex(args, &model)
        }
        "claude" => {
            let model = resolved.model.ok_or_else(|| {
                "[ERROR] claude reviewer requires a model (set model in agent-profiles.json)"
                    .to_owned()
            })?;
            dispatch_claude(args, &model)
        }
        other => Err(format!(
            "[ERROR] unsupported reviewer provider '{other}' \
             (supported: 'codex', 'claude')"
        )),
    }
}

fn dispatch_codex(args: &LocalArgs, model: &str) -> Result<u8, String> {
    let track_id = &args.track_id;
    let group = args.group.trim();
    let round_type_str = match args.round_type {
        CodexRoundTypeArg::Fast => "fast",
        CodexRoundTypeArg::Final => "final",
    };

    infrastructure::review_v2::validate_track_id_str(track_id)
        .map_err(|e| format!("invalid --track-id: {e}"))?;
    infrastructure::review_v2::validate_review_group_name_str(group)
        .map_err(|e| format!("invalid --group: {e}"))?;

    let maybe_briefing =
        infrastructure::review_v2::get_briefing_for_scope_str(group, track_id, &args.items_dir)?;
    if let Some(path) = &maybe_briefing {
        if !is_safe_briefing_path(path) {
            eprintln!(
                "[WARN] briefing_file for scope '{group}' contains unsafe characters — \
                 scope-specific severity policy injection skipped"
            );
        }
    }

    let mut base_prompt = build_base_prompt_from_args(&args.briefing_file, &args.prompt)?;
    infrastructure::review_v2::append_scope_briefing_reference_str(
        &mut base_prompt,
        group,
        track_id,
        &args.items_dir,
        is_safe_briefing_path,
    )?;

    let timeout = Duration::from_secs(args.timeout_seconds);
    let reviewer = CodexReviewer::new(model, timeout, base_prompt).with_scope_label(group);

    let outcome = infrastructure::review_v2::run_codex_review_str(
        track_id,
        &args.items_dir,
        group,
        round_type_str,
        reviewer,
    )?;

    emit_outcome(outcome)
}

fn dispatch_claude(args: &LocalArgs, model: &str) -> Result<u8, String> {
    let track_id = &args.track_id;
    let group = args.group.trim();
    let round_type_str = match args.round_type {
        CodexRoundTypeArg::Fast => "fast",
        CodexRoundTypeArg::Final => "final",
    };

    infrastructure::review_v2::validate_track_id_str(track_id)
        .map_err(|e| format!("invalid --track-id: {e}"))?;
    infrastructure::review_v2::validate_review_group_name_str(group)
        .map_err(|e| format!("invalid --group: {e}"))?;

    let maybe_briefing =
        infrastructure::review_v2::get_briefing_for_scope_str(group, track_id, &args.items_dir)?;
    if let Some(path) = &maybe_briefing {
        if !is_safe_briefing_path(path) {
            eprintln!(
                "[WARN] briefing_file for scope '{group}' contains unsafe characters — \
                 scope-specific severity policy injection skipped"
            );
        }
    }

    let mut base_prompt = build_base_prompt_from_args(&args.briefing_file, &args.prompt)?;
    infrastructure::review_v2::append_scope_briefing_reference_str(
        &mut base_prompt,
        group,
        track_id,
        &args.items_dir,
        is_safe_briefing_path,
    )?;

    let timeout = Duration::from_secs(args.timeout_seconds);
    let reviewer = ClaudeReviewer::new(model, timeout, base_prompt).with_scope_label(group);

    let outcome = infrastructure::review_v2::run_claude_review_str(
        track_id,
        &args.items_dir,
        group,
        round_type_str,
        reviewer,
    )?;

    emit_outcome(outcome)
}

fn emit_outcome(outcome: CodexReviewOutcome) -> Result<u8, String> {
    use std::io::Write as _;
    match outcome {
        CodexReviewOutcome::Skipped { scope_label } => {
            eprintln!("[auto-record] Scope '{scope_label}' is empty, skipping");
            writeln!(std::io::stdout(), r#"{{"verdict":"zero_findings","findings":[]}}"#)
                .map_err(|e| format!("failed to write stdout: {e}"))?;
            Ok(0)
        }
        CodexReviewOutcome::FinalCompleted { verdict_json, exit_code } => {
            writeln!(std::io::stdout(), "{verdict_json}")
                .map_err(|e| format!("failed to write stdout: {e}"))?;
            Ok(exit_code)
        }
        CodexReviewOutcome::FastCompleted { verdict_json, exit_code } => {
            writeln!(std::io::stdout(), "{verdict_json}")
                .map_err(|e| format!("failed to write stdout: {e}"))?;
            Ok(exit_code)
        }
    }
}

/// Builds the base prompt from the briefing file or inline prompt args.
///
/// # Errors
/// Returns an error if the briefing file does not exist or neither arg is provided.
fn build_base_prompt_from_args(
    briefing_file: &Option<PathBuf>,
    prompt: &Option<String>,
) -> Result<String, String> {
    if let Some(path) = briefing_file {
        if !path.is_file() {
            return Err(format!("briefing file not found: {}", path.display()));
        }
        Ok(format!("Read {} and perform the task described there.", path.display()))
    } else {
        prompt.clone().ok_or_else(|| "either --briefing-file or --prompt is required".to_owned())
    }
}

/// Resolves the reviewer provider/model from agent-profiles.json at a given path,
/// returning a fail-closed error when the capability is missing or the provider
/// is unsupported.
///
/// Extracted as a pure function so the fail-closed resolution logic can be tested
/// without spawning a subprocess or hitting the filesystem for the review cycle.
///
/// # Errors
/// Returns a human-readable error string if the profiles file cannot be loaded,
/// the `reviewer` capability is not defined, or the provider is unsupported.
#[cfg(test)]
pub(super) fn resolve_reviewer_for_test(
    profiles_path: &std::path::Path,
    round_type: CodexRoundTypeArg,
) -> Result<infrastructure::agent_profiles::ResolvedExecution, String> {
    let profiles = infrastructure::agent_profiles::AgentProfiles::load(profiles_path)
        .map_err(|e| format!("[ERROR] failed to load agent-profiles.json: {e}"))?;
    let infra_round_type = match round_type {
        CodexRoundTypeArg::Fast => infrastructure::agent_profiles::RoundType::Fast,
        CodexRoundTypeArg::Final => infrastructure::agent_profiles::RoundType::Final,
    };
    let resolved = profiles.resolve_execution("reviewer", infra_round_type).ok_or_else(|| {
        "[ERROR] reviewer capability not defined in agent-profiles.json".to_owned()
    })?;
    match resolved.provider.as_str() {
        "codex" | "claude" => Ok(resolved),
        other => Err(format!(
            "[ERROR] unsupported reviewer provider '{other}' \
             (supported: 'codex', 'claude')"
        )),
    }
}

/// Returns `true` if `path` is safe to reference as a repo-relative briefing
/// file and to inject into the markdown prompt as a backtick-quoted path bullet.
///
/// Same validation logic as in `codex_local::is_safe_briefing_path` and
/// `claude_local::is_safe_briefing_path`.
fn is_safe_briefing_path(path: &str) -> bool {
    if path.is_empty() {
        return false;
    }
    if path.chars().any(|c| c == '`' || c.is_control() || matches!(c, '\u{2028}' | '\u{2029}')) {
        return false;
    }
    if path.starts_with('/') || path.starts_with('\\') {
        return false;
    }
    if let (Some(first), Some(second)) = (path.as_bytes().first(), path.as_bytes().get(1)) {
        if *second == b':' && first.is_ascii_alphabetic() {
            return false;
        }
    }
    if path.split(['/', '\\']).any(|component| component == "..") {
        return false;
    }
    true
}
