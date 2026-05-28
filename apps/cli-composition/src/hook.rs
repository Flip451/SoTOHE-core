//! `hook` command family — CliApp impl methods.
//!
//! The composition root owns stdin reading (CN-02): the CLI layer passes only
//! the hook name; the method reads the hook JSON envelope from stdin and
//! dispatches via `HookDispatchInteractor`.

use std::io::Read as _;
use std::path::PathBuf;
use std::sync::Arc;

use crate::{CliApp, CommandOutcome};

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

impl CliApp {
    /// Dispatch a security-critical hook via Rust logic.
    ///
    /// Reads Claude Code hook JSON from stdin.
    /// Exit code 0 = allow, exit code 2 = block (Claude Code hook protocol).
    /// PreToolUse hooks: any internal error → exit code 2 (fail-closed).
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails.
    pub fn hook_dispatch(&self, hook_name: String) -> Result<CommandOutcome, String> {
        use infrastructure::shell::ConchShellParser;
        use usecase::hook_dispatch::{
            HookDispatchCommand, HookDispatchInteractor, HookDispatchService, HookVerdictDecision,
        };

        // UserPromptSubmit hooks use a separate flow (advisory, not guard).
        if is_user_prompt_submit(&hook_name) {
            return self.hook_dispatch_user_prompt_submit();
        }

        let is_post = is_post_tool_use(&hook_name);

        // Read stdin JSON
        let mut stdin_buf = String::new();
        if let Err(e) = std::io::stdin().read_to_string(&mut stdin_buf) {
            return make_hook_error(is_post, &format!("failed to read stdin: {e}"));
        }

        if stdin_buf.trim().is_empty() {
            return make_hook_error(is_post, "hook received empty stdin — no envelope to check");
        }

        let envelope: HookEnvelope = match serde_json::from_str(&stdin_buf) {
            Ok(env) => env,
            Err(e) => {
                return make_hook_error(is_post, &format!("failed to parse hook JSON: {e}"));
            }
        };

        let dispatch_cmd = HookDispatchCommand {
            tool_name: envelope.tool_name,
            command: envelope.tool_input.command,
            file_path: envelope.tool_input.file_path,
            content: envelope.tool_input.content,
        };

        let parser_port = Arc::new(ConchShellParser);
        let project_dir = std::env::var("CLAUDE_PROJECT_DIR").ok().map(PathBuf::from);
        let service = HookDispatchInteractor::new(parser_port, project_dir);

        let result = service.dispatch(hook_name, dispatch_cmd);

        match result {
            Ok(verdict) => {
                let is_block = verdict.decision == HookVerdictDecision::Block;
                if is_block {
                    let reason = verdict.reason.unwrap_or_default();
                    Ok(CommandOutcome {
                        stdout: if reason.is_empty() { None } else { Some(reason) },
                        stderr: None,
                        exit_code: 2,
                    })
                } else {
                    Ok(CommandOutcome::success(None))
                }
            }
            Err(e) => make_hook_error(is_post, &format!("hook error: {e}")),
        }
    }

    fn hook_dispatch_user_prompt_submit(&self) -> Result<CommandOutcome, String> {
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
fn make_hook_error(is_post_tool_use: bool, message: &str) -> Result<CommandOutcome, String> {
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
