//! Shared helpers for building Codex CLI argument vectors.
//!
//! Both the `DryCheckAgentPort` adapter (`codex_dry_checker`) and the
//! `Reviewer` adapter (`codex_reviewer`) build the same `codex exec`
//! argument pattern: model, read-only sandbox, reasoning-effort config,
//! output schema/last-message, and prompt.  This module centralises that
//! construction so future changes to Codex CLI flags only need to happen
//! in one place.

use std::ffi::OsString;
use std::path::Path;

/// Build the argument vector for a `codex exec --sandbox read-only` invocation.
///
/// Produces: `exec --model <model> --sandbox read-only --config
/// model_reasoning_effort="<reasoning_effort>" --output-schema <schema>
/// --output-last-message <last_msg> <prompt>`.
///
/// # Arguments
/// - `model`: Codex model name (e.g. `"gpt-5.5"`).
/// - `reasoning_effort`: `model_reasoning_effort` value (e.g. `"high"`).
/// - `prompt`: Full prompt string passed as the final positional argument.
/// - `output_last_message`: Path where Codex writes the last message JSON.
/// - `output_schema`: Path to the JSON schema file for structured output.
pub fn build_codex_read_only_invocation(
    model: &str,
    reasoning_effort: &str,
    prompt: &str,
    output_last_message: &Path,
    output_schema: &Path,
) -> Vec<OsString> {
    let mut args = vec![OsString::from("exec"), OsString::from("--model"), OsString::from(model)];
    // MUST use read-only sandbox. Do NOT use --full-auto here because it
    // implies --sandbox workspace-write and Codex CLI applies it after our
    // explicit --sandbox read-only, overriding the safety constraint.
    args.extend([OsString::from("--sandbox"), OsString::from("read-only")]);
    args.extend([
        OsString::from("--config"),
        OsString::from(format!("model_reasoning_effort=\"{reasoning_effort}\"")),
    ]);
    args.extend([
        OsString::from("--output-schema"),
        output_schema.as_os_str().to_os_string(),
        OsString::from("--output-last-message"),
        output_last_message.as_os_str().to_os_string(),
        OsString::from(prompt),
    ]);
    args
}
