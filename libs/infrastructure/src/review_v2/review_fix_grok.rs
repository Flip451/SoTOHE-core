//! Isolated Grok launch path for review-fix.

use std::path::Path;
use std::process::Command;
use std::time::Duration;

use domain::TrackId;
use usecase::capability_exec::{GROK_PROVIDER_NAME, ModelName, ProviderName, ReasoningEffort};
use usecase::dry_write_driver::CapabilityName;
use usecase::git_workflow::DiagnosticText;
use usecase::provider_session::{
    ProviderSessionCacheEntry, ProviderSessionCacheKey, ProviderSessionCachePort, ProviderSessionId,
};
use usecase::review_v2::run_review_fix::{
    ReviewFixRunnerError, RunReviewFixCommand, RunReviewFixOutput,
};

use crate::capability_exec::grok::{build_grok_args, resolve_grok_capability_definition};
use crate::capability_exec::process::run_grok_direct_command;
use crate::grok_common::{GrokOutputEnvelope, GrokSandbox, GrokStreamedStdout};
use crate::provider_session::FsProviderSessionCacheAdapter;

fn parse_sentinel(output: &str) -> Option<&'static str> {
    let last_line = output.lines().rev().find(|line| !line.trim().is_empty())?;
    match last_line {
        "REVIEW_FIX_STATUS: completed" => Some("completed"),
        "REVIEW_FIX_STATUS: blocked_cross_scope" => Some("blocked_cross_scope"),
        "REVIEW_FIX_STATUS: failed" => Some("failed"),
        _ => None,
    }
}

fn sentinel_to_exit_code(status: &str) -> i32 {
    match status {
        "completed" => 0,
        "blocked_cross_scope" => 2,
        _ => 1,
    }
}

const GROK_REVIEW_FIX_TIMEOUT: Duration = Duration::from_secs(60 * 60);
const GROK_REVIEW_FIX_OUTPUT_LIMIT: usize = 256 * 1024;

pub(super) fn run_review_fix_grok(
    model: &ModelName,
    effort: ReasoningEffort,
    command: &RunReviewFixCommand,
    briefing_content: &str,
    repository_root: &Path,
) -> Result<RunReviewFixOutput, ReviewFixRunnerError> {
    let definition = resolve_grok_capability_definition(repository_root, "review-fix-lead", model)
        .map_err(unexpected)?;
    let sandbox = definition.sandbox().cloned().ok_or_else(|| {
        unexpected("Grok review-fix-lead definition did not resolve a grok-sandbox permission")
    })?;
    let prompt = crate::review_v2::review_fix_runner::build_prompt_with_context(
        command.scope(),
        command,
        briefing_content,
    )?;
    let cache = FsProviderSessionCacheAdapter::new(
        repository_root.to_path_buf(),
        repository_root.join("tmp/capability-runtime"),
    );
    let key = assignment_key(command).map_err(unexpected)?;
    let resume = resumable_session(&cache, &key, model, effort);
    let output = match resume.as_deref() {
        Some(session) => {
            match launch(model, effort, &sandbox, Some(session), &prompt, repository_root) {
                Ok(output) if usable_result(&output) => output,
                _ => {
                    let _ = cache.remove(&key);
                    launch(model, effort, &sandbox, None, &prompt, repository_root)?
                }
            }
        }
        None => launch(model, effort, &sandbox, None, &prompt, repository_root)?,
    };
    if !output.exit_ok {
        return Err(unexpected(
            output
                .failure_reason
                .unwrap_or_else(|| "Grok review-fix subprocess exited unsuccessfully".to_owned()),
        ));
    }
    let text =
        output.result.ok_or_else(|| {
            unexpected(output.failure_reason.unwrap_or_else(|| {
                "Grok review-fix returned no structured-output result".to_owned()
            }))
        })?;
    let status = parse_sentinel(&text).ok_or_else(|| {
        ReviewFixRunnerError::SentinelNotFound(diagnostic(format!(
            "no REVIEW_FIX_STATUS sentinel in Grok structured output: {text}"
        )))
    })?;
    if let Some(session_id) = output.session_id {
        save_session(&cache, &key, session_id, model, effort);
    }
    Ok(RunReviewFixOutput {
        status: status.to_owned(),
        exit_code: sentinel_to_exit_code(status),
        stderr: None,
    })
}

fn usable_result(output: &LaunchOutput) -> bool {
    output.exit_ok && output.result.as_deref().and_then(parse_sentinel).is_some()
}

struct LaunchOutput {
    exit_ok: bool,
    result: Option<String>,
    failure_reason: Option<String>,
    session_id: Option<String>,
}

fn launch(
    model: &ModelName,
    effort: ReasoningEffort,
    sandbox: &GrokSandbox,
    resume_id: Option<&str>,
    prompt: &str,
    cwd: &Path,
) -> Result<LaunchOutput, ReviewFixRunnerError> {
    let args = build_grok_args(model.as_str(), effort, sandbox, resume_id, prompt);
    let mut command = Command::new("grok");
    command.args(&args).current_dir(cwd);
    let (status, collected) = run_grok_direct_command(
        &mut command,
        GROK_REVIEW_FIX_OUTPUT_LIMIT,
        GROK_REVIEW_FIX_TIMEOUT,
        "grok review-fix",
    )
    .map_err(|error| ReviewFixRunnerError::SpawnFailed(diagnostic(error.to_string())))?;
    parse_output(&collected, status.success())
}

fn parse_output(
    collected: &GrokStreamedStdout,
    exit_ok: bool,
) -> Result<LaunchOutput, ReviewFixRunnerError> {
    let Some(envelope_bytes) = collected.envelope_bytes.as_deref() else {
        return Ok(LaunchOutput {
            exit_ok,
            result: None,
            failure_reason: Some(
                "Grok review-fix returned no structured-output envelope".to_owned(),
            ),
            session_id: collected.session_id.clone(),
        });
    };
    let value: serde_json::Value = serde_json::from_slice(envelope_bytes).map_err(|error| {
        ReviewFixRunnerError::Unexpected(diagnostic(format!(
            "cannot decode Grok review-fix envelope: {error}"
        )))
    })?;
    let session_id = collected.session_id.clone();
    let envelope: GrokOutputEnvelope = serde_json::from_value(value).map_err(|error| {
        ReviewFixRunnerError::Unexpected(diagnostic(format!(
            "cannot decode Grok review-fix envelope: {error}"
        )))
    })?;
    match envelope {
        GrokOutputEnvelope::Succeeded { structured_output } => Ok(LaunchOutput {
            exit_ok,
            session_id,
            result: structured_output
                .get("result")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned),
            failure_reason: None,
        }),
        GrokOutputEnvelope::Failed { failure_reason } => Ok(LaunchOutput {
            exit_ok,
            result: None,
            failure_reason: Some(failure_reason.to_string()),
            session_id,
        }),
    }
}

fn assignment_key(command: &RunReviewFixCommand) -> Result<ProviderSessionCacheKey, String> {
    let round = match command.round_type() {
        usecase::review_v2::ReviewRoundType::Fast => "fast",
        usecase::review_v2::ReviewRoundType::Final => "final",
    };
    Ok(ProviderSessionCacheKey::TrackCapability {
        track_id: TrackId::try_new(command.track_id().to_owned())
            .map_err(|error| error.to_string())?,
        capability: CapabilityName::try_new(format!("review-fix-lead:{}:{round}", command.scope()))
            .map_err(|error| error.to_string())?,
    })
}

fn resumable_session(
    cache: &FsProviderSessionCacheAdapter,
    key: &ProviderSessionCacheKey,
    model: &ModelName,
    effort: ReasoningEffort,
) -> Option<String> {
    let entry = cache.load(key).ok().flatten()?;
    (entry.provider().as_str() == GROK_PROVIDER_NAME.as_str()
        && entry.model() == model
        && entry.effort() == effort)
        .then(|| entry.session_id().as_str().to_owned())
}

fn save_session(
    cache: &FsProviderSessionCacheAdapter,
    key: &ProviderSessionCacheKey,
    session_id: String,
    model: &ModelName,
    effort: ReasoningEffort,
) {
    let Ok(session_id) = ProviderSessionId::try_new(session_id) else {
        return;
    };
    let Ok(provider) = ProviderName::try_new(GROK_PROVIDER_NAME.as_str().to_owned()) else {
        return;
    };
    let entry = ProviderSessionCacheEntry::new(session_id, provider, model.clone(), effort);
    let _ = cache.save(key, &entry);
}

fn diagnostic(value: impl Into<String>) -> DiagnosticText {
    DiagnosticText::new(value.into())
}

fn unexpected(value: impl Into<String>) -> ReviewFixRunnerError {
    ReviewFixRunnerError::Unexpected(diagnostic(value))
}
