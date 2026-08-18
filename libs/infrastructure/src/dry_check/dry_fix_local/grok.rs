//! Isolated Grok launch path for `dry fix-local`.

use std::path::Path;
use std::process::Command;
use std::time::Duration;

use domain::TrackId;
use usecase::capability_exec::{GROK_PROVIDER_NAME, ModelName, ProviderName, ReasoningEffort};
use usecase::dry_driver::DryDriverOutcome;
use usecase::dry_write_driver::CapabilityName;
use usecase::provider_session::{
    ProviderSessionCacheEntry, ProviderSessionCacheKey, ProviderSessionCachePort, ProviderSessionId,
};

use crate::capability_exec::grok::{build_grok_args, resolve_grok_capability_definition};
use crate::capability_exec::process::run_grok_direct_command;
use crate::grok_common::{GrokOutputEnvelope, GrokSandbox, GrokStreamedStdout};
use crate::provider_session::FsProviderSessionCacheAdapter;

use super::{dry_fix_sentinel_to_exit_code, parse_dry_fix_sentinel};

const GROK_DRY_FIX_TIMEOUT: Duration = Duration::from_secs(600);
const GROK_DRY_FIX_OUTPUT_LIMIT: usize = 256 * 1024;

pub(super) fn run_dry_fix_grok(
    model: &str,
    effort: ReasoningEffort,
    track_id: &str,
    briefing_file: &Path,
    trusted_root: &Path,
) -> Result<DryDriverOutcome, String> {
    let model_name = ModelName::try_new(model.to_owned()).map_err(|error| error.to_string())?;
    let definition = resolve_grok_capability_definition(trusted_root, "dry-fix-lead", &model_name)?;
    let sandbox = definition.sandbox().cloned().ok_or_else(|| {
        "Grok dry-fix-lead definition did not resolve a grok-sandbox permission".to_owned()
    })?;
    let prompt = super::prompt::build_dry_fix_prompt(track_id, briefing_file, trusted_root)?;
    let cache = FsProviderSessionCacheAdapter::new(
        trusted_root.to_path_buf(),
        trusted_root.join("tmp/capability-runtime"),
    );
    let key = track_capability_key(track_id)?;
    let resume = resumable_session(&cache, &key, &model_name, effort);
    let result = match resume.as_deref() {
        Some(session) => {
            match launch_grok(model, effort, &sandbox, Some(session), &prompt, trusted_root) {
                Ok(output) if usable_result(&output) => output,
                _ => {
                    let _ = cache.remove(&key);
                    launch_grok(model, effort, &sandbox, None, &prompt, trusted_root)?
                }
            }
        }
        None => launch_grok(model, effort, &sandbox, None, &prompt, trusted_root)?,
    };
    if !result.exit_ok {
        return Err(result
            .failure_reason
            .unwrap_or_else(|| "Grok dry-fix subprocess exited unsuccessfully".to_owned()));
    }
    let text = result.result.ok_or_else(|| {
        result
            .failure_reason
            .unwrap_or_else(|| "Grok dry-fix returned no structured-output result".to_owned())
    })?;
    let status = parse_dry_fix_sentinel(&text).ok_or_else(|| {
        format!("no DRY_FIX_STATUS sentinel found in Grok structured output: {text}")
    })?;
    if let Some(session_id) = result.session_id {
        save_session(&cache, &key, session_id, &model_name, effort);
    }
    Ok(DryDriverOutcome {
        stdout: Some(format!("DRY_FIX_STATUS: {status}")),
        stderr: None,
        exit_code: u8::try_from(dry_fix_sentinel_to_exit_code(status)).unwrap_or(1),
    })
}

fn usable_result(output: &GrokLaunchOutput) -> bool {
    output.exit_ok && output.result.as_deref().and_then(parse_dry_fix_sentinel).is_some()
}

struct GrokLaunchOutput {
    exit_ok: bool,
    result: Option<String>,
    failure_reason: Option<String>,
    session_id: Option<String>,
}

fn launch_grok(
    model: &str,
    effort: ReasoningEffort,
    sandbox: &GrokSandbox,
    resume_id: Option<&str>,
    prompt: &str,
    cwd: &Path,
) -> Result<GrokLaunchOutput, String> {
    let args = build_grok_args(model, effort, sandbox, resume_id, prompt);
    let mut command = Command::new("grok");
    command.args(&args).current_dir(cwd);
    let (status, collected) = run_grok_direct_command(
        &mut command,
        GROK_DRY_FIX_OUTPUT_LIMIT,
        GROK_DRY_FIX_TIMEOUT,
        "grok dry-fix",
    )
    .map_err(|error| format!("failed to spawn grok dry-fix: {error}"))?;
    parse_launch_output(&collected, status.success())
}

fn parse_launch_output(
    collected: &GrokStreamedStdout,
    exit_ok: bool,
) -> Result<GrokLaunchOutput, String> {
    let Some(envelope_bytes) = collected.envelope_bytes.as_deref() else {
        return Ok(GrokLaunchOutput {
            exit_ok,
            result: None,
            failure_reason: Some("Grok dry-fix returned no structured-output envelope".to_owned()),
            session_id: collected.session_id.clone(),
        });
    };
    let value: serde_json::Value = serde_json::from_slice(envelope_bytes)
        .map_err(|error| format!("cannot decode Grok dry-fix envelope: {error}"))?;
    let session_id = collected.session_id.clone();
    let envelope: GrokOutputEnvelope = serde_json::from_value(value)
        .map_err(|error| format!("cannot decode Grok dry-fix envelope: {error}"))?;
    match envelope {
        GrokOutputEnvelope::Succeeded { structured_output } => Ok(GrokLaunchOutput {
            exit_ok,
            session_id,
            result: structured_output
                .get("result")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned),
            failure_reason: None,
        }),
        GrokOutputEnvelope::Failed { failure_reason } => Ok(GrokLaunchOutput {
            exit_ok,
            result: None,
            failure_reason: Some(failure_reason.to_string()),
            session_id,
        }),
    }
}

fn track_capability_key(track_id: &str) -> Result<ProviderSessionCacheKey, String> {
    Ok(ProviderSessionCacheKey::TrackCapability {
        track_id: TrackId::try_new(track_id.to_owned()).map_err(|error| error.to_string())?,
        capability: CapabilityName::try_new("dry-fix-lead").map_err(|error| error.to_string())?,
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
