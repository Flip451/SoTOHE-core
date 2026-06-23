//! `hook` command family — per-context composition root and CliApp shim.
//!
//! The composition root owns stdin reading (CN-02): the CLI layer passes the
//! hook name plus any git hook positional arguments. Claude Code hook JSON
//! envelopes are read from stdin here before dispatching via
//! `HookDispatchInteractor`.

use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::{CommandOutcome, error::CompositionError};

// ---------------------------------------------------------------------------
// Per-context composition root
// ---------------------------------------------------------------------------

/// Composition root for the `hook` command family.
///
/// Unit struct: no adapter dependencies are injected at construction time.
pub struct HookCompositionRoot;

impl HookCompositionRoot {
    /// Create a new `HookCompositionRoot`.
    pub fn new() -> Self {
        Self
    }
}

impl Default for HookCompositionRoot {
    fn default() -> Self {
        Self::new()
    }
}

impl HookCompositionRoot {
    /// Build a wired [`cli_driver::hook::HookDriver`] for the hook family.
    ///
    /// Reads process environment values here (composition root responsibility per CN-02)
    /// and passes them to the use-case interactor.
    pub fn hook_driver(&self) -> cli_driver::hook::HookDriver {
        use infrastructure::shell::ConchShellParser;
        use usecase::hook_dispatch::HookDispatchInteractor;

        let guarded_git_token_present = std::env::var("SOTP_GUARDED_GIT").is_ok();
        let hooks_path_configured = hooks_path_configured();
        let project_dir = std::env::var("CLAUDE_PROJECT_DIR").ok().map(PathBuf::from);

        let parser_port = Arc::new(ConchShellParser);
        let service = Arc::new(HookDispatchInteractor::new(
            parser_port,
            project_dir,
            guarded_git_token_present,
            hooks_path_configured,
        ));

        cli_driver::hook::HookDriver::new(service)
    }
}

/// CLI-layer serde type for Claude Code hook JSON envelope.
/// Security-critical fields (`tool_name`) must NOT use `#[serde(default)]` —
/// parse failure is caught at the CLI boundary.
/// For PreToolUse hooks this results in exit code 2 (block, fail-closed).
#[derive(Debug, Clone, serde::Deserialize)]
struct HookEnvelope {
    /// Required — no `#[serde(default)]`.
    tool_name: String,
    #[serde(default)]
    tool_input: HookToolInput,
}

#[derive(Debug, Clone, Default, serde::Deserialize)]
struct HookToolInput {
    command: Option<String>,
    file_path: Option<PathBuf>,
    /// Content written by the Write tool.
    #[serde(default, deserialize_with = "deserialize_string_or_none")]
    content: Option<String>,
}

fn deserialize_string_or_none<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::Deserialize;
    let value: Option<serde_json::Value> = Option::<serde_json::Value>::deserialize(deserializer)?;
    Ok(value.and_then(|v| flatten_content_text(&v)))
}

fn flatten_content_text(value: &serde_json::Value) -> Option<String> {
    let mut parts = Vec::new();
    collect_text_parts(value, &mut parts);
    if parts.is_empty() { None } else { Some(parts.join("\n")) }
}

fn collect_text_parts(value: &serde_json::Value, parts: &mut Vec<String>) {
    match value {
        serde_json::Value::String(s) => {
            if !s.is_empty() {
                parts.push(s.clone());
            }
        }
        serde_json::Value::Array(arr) => {
            for item in arr {
                collect_text_parts(item, parts);
            }
        }
        serde_json::Value::Object(obj) => {
            if let Some(text) = obj.get("text").and_then(|v| v.as_str()) {
                if !text.is_empty() {
                    parts.push(text.to_owned());
                    return;
                }
            }
            for (key, sub) in obj {
                match sub {
                    serde_json::Value::String(s) if !s.is_empty() => {
                        if key == "message" || key == "content" {
                            parts.push(s.clone());
                        }
                    }
                    serde_json::Value::Object(_) | serde_json::Value::Array(_) => {
                        collect_text_parts(sub, parts);
                    }
                    _ => {}
                }
            }
        }
        _ => {}
    }
}

/// Serde type for UserPromptSubmit hook JSON envelope.
#[derive(Debug, serde::Deserialize)]
struct PromptEnvelope {
    #[serde(default)]
    prompt: String,
}

/// Returns `true` if the hook name is a UserPromptSubmit hook (advisory, never blocks).
fn is_user_prompt_submit(hook_name: &str) -> bool {
    hook_name == "skill-compliance"
}

/// Returns `true` if the hook name is a PostToolUse hook (cannot block).
fn is_post_tool_use(_hook_name: &str) -> bool {
    false
}

/// Returns `true` if the hook name is dispatched from git's process-level hooks.
fn is_git_process_hook(hook_name: &str) -> bool {
    matches!(hook_name, "git-ref-update" | "git-pre-push")
}

/// Returns `true` when git is sending a reference-transaction notification that
/// cannot affect the transaction outcome and must not run the guarded-git verdict.
fn is_git_ref_update_final_notification(hook_name: &str, hook_args: &[String]) -> bool {
    hook_name == "git-ref-update"
        && hook_args.first().is_some_and(|state| matches!(state.as_str(), "committed" | "aborted"))
}

fn hooks_path_configured() -> bool {
    infrastructure::verify::hooks_path::verify(Path::new(".")).is_ok()
}

impl HookCompositionRoot {
    /// Dispatch a security-critical hook via Rust logic.
    ///
    /// Reads Claude Code hook JSON from stdin for Claude Code hooks.
    /// Uses positional hook arguments for process-level git hooks.
    /// Exit code 0 = allow, exit code 2 = block (Claude Code hook protocol).
    /// PreToolUse hooks: any internal error → exit code 2 (fail-closed).
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails.
    pub fn hook_dispatch(
        &self,
        hook_name: String,
        hook_args: Vec<String>,
    ) -> Result<CommandOutcome, CompositionError> {
        use infrastructure::shell::ConchShellParser;
        use usecase::hook_dispatch::{
            HookDispatchCommand, HookDispatchInteractor, HookDispatchService, HookVerdictDecision,
        };

        // UserPromptSubmit hooks use a separate flow (advisory, not guard).
        if is_user_prompt_submit(&hook_name) {
            return self.hook_dispatch_user_prompt_submit();
        }

        if is_git_ref_update_final_notification(&hook_name, &hook_args) {
            return Ok(CommandOutcome::success(None));
        }

        let is_post = is_post_tool_use(&hook_name);
        let guarded_git_token_present = std::env::var("SOTP_GUARDED_GIT").is_ok();
        let hooks_path_configured = hooks_path_configured();

        let dispatch_cmd = if is_git_process_hook(&hook_name) {
            HookDispatchCommand {
                tool_name: "Git".to_owned(),
                command: None,
                file_path: None,
                content: None,
                git_hook_args: hook_args.clone(),
            }
        } else {
            // Read stdin JSON
            let mut stdin_buf = String::new();
            if let Err(e) = std::io::stdin().read_to_string(&mut stdin_buf) {
                return make_hook_error(is_post, &format!("failed to read stdin: {e}"));
            }

            if stdin_buf.trim().is_empty() {
                return make_hook_error(
                    is_post,
                    "hook received empty stdin — no envelope to check",
                );
            }

            let envelope: HookEnvelope = match serde_json::from_str(&stdin_buf) {
                Ok(env) => env,
                Err(e) => {
                    return make_hook_error(is_post, &format!("failed to parse hook JSON: {e}"));
                }
            };

            HookDispatchCommand {
                tool_name: envelope.tool_name,
                command: envelope.tool_input.command,
                file_path: envelope.tool_input.file_path,
                content: envelope.tool_input.content,
                git_hook_args: vec![],
            }
        };

        let parser_port = Arc::new(ConchShellParser);
        let project_dir = std::env::var("CLAUDE_PROJECT_DIR").ok().map(PathBuf::from);
        let service = HookDispatchInteractor::new(
            parser_port,
            project_dir,
            guarded_git_token_present,
            hooks_path_configured,
        );

        let result = service.dispatch(hook_name, dispatch_cmd);

        match result {
            Ok(verdict) => {
                let is_block = verdict.decision == HookVerdictDecision::Block;
                if is_block {
                    let reason = verdict.reason.unwrap_or_default();
                    Ok(CommandOutcome {
                        stdout: None,
                        stderr: if reason.is_empty() { None } else { Some(reason) },
                        exit_code: 2,
                    })
                } else {
                    Ok(CommandOutcome::success(None))
                }
            }
            Err(e) => make_hook_error(is_post, &format!("hook error: {e}")),
        }
    }

    fn hook_dispatch_user_prompt_submit(&self) -> Result<CommandOutcome, CompositionError> {
        let mut stdin_buf = String::new();
        if std::io::stdin().read_to_string(&mut stdin_buf).is_err() {
            return Ok(CommandOutcome::success(None)); // advisory — never block
        }

        let prompt = match serde_json::from_str::<PromptEnvelope>(stdin_buf.trim()) {
            Ok(env) => env.prompt,
            Err(_) => {
                return Ok(CommandOutcome::success(None));
            }
        };

        if prompt.is_empty() {
            return Ok(CommandOutcome::success(None));
        }

        let has_track_command = prompt.to_lowercase().contains("/track:");
        if !has_track_command && !usecase::skill_compliance::has_skill_command(&prompt) {
            return Ok(CommandOutcome::success(None));
        }

        let stdout = usecase::skill_compliance::check_compliance_render(&prompt).map(|ctx| {
            serde_json::json!({
                "hookSpecificOutput": {
                    "hookEventName": "UserPromptSubmit",
                    "additionalContext": ctx,
                }
            })
            .to_string()
        });

        Ok(CommandOutcome::success(stdout))
    }
}

/// Build a `CommandOutcome` for a hook error, respecting pre/post semantics.
fn make_hook_error(
    is_post_tool_use: bool,
    message: &str,
) -> Result<CommandOutcome, CompositionError> {
    if is_post_tool_use {
        // PostToolUse: warn + exit 0 (cannot block)
        Ok(CommandOutcome {
            stdout: None,
            stderr: Some(format!("warning: {message}")),
            exit_code: 0,
        })
    } else {
        // PreToolUse: exit 2 (fail-closed)
        Ok(CommandOutcome { stdout: None, stderr: Some(format!("error: {message}")), exit_code: 2 })
    }
}
