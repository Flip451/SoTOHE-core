// STAGED FOR T021 — not yet compiled; Cargo.toml + workspace member added atomically in T021 per CN-06.
//
//! `hook` command family — primary adapter driver.
//!
//! `HookDriver` holds injected use-case interactors and exposes
//! `handle(input) -> CommandOutcome`.
//!
//! Stdin reading and provider hook JSON envelope parsing are performed at this
//! driver boundary (CN-02): the driver owns I/O, normalizes Claude and Grok
//! envelopes, and converts the result into
//! [`usecase::hook_dispatch::HookDispatchCommand`] before calling the usecase
//! layer.
//!
//! JSON parsing uses [`serde_json::Value`] directly (no derive macros) because
//! `cli_driver` does not carry `serde` as a direct dependency. Only `serde_json`
//! (already declared) is needed for this manual extraction approach.

use std::io::Read as _;
use std::path::PathBuf;
use std::sync::Arc;

use usecase::hook_dispatch::{HookDispatchCommand, HookDispatchService, HookVerdictDecision};

use crate::render::CommandOutcome;

// ---------------------------------------------------------------------------
// Input types
// ---------------------------------------------------------------------------

/// Known hook names for the `hook dispatch` subcommand.
#[derive(Debug, Clone)]
pub enum HookName {
    /// Preflight: require local git hooks setup before Bash execution.
    HooksPathSetup,
    /// Guard: block direct git operations.
    BlockDirectGitOps,
    /// Guard: block `rm` commands targeting test files (PreToolUse).
    BlockTestFileDeletion,
    /// Process-level git hook: reference transaction.
    GitRefUpdate,
    /// Process-level git hook: pre-push.
    GitPrePush,
    /// Advisory: skill compliance check for UserPromptSubmit.
    SkillCompliance,
}

impl HookName {
    /// Returns the hook name string used by the dispatch service.
    pub fn hook_name(&self) -> &'static str {
        match self {
            Self::HooksPathSetup => "hooks-path-setup",
            Self::BlockDirectGitOps => "block-direct-git-ops",
            Self::BlockTestFileDeletion => "block-test-file-deletion",
            Self::GitRefUpdate => "git-ref-update",
            Self::GitPrePush => "git-pre-push",
            Self::SkillCompliance => "skill-compliance",
        }
    }

    /// Returns whether this hook accepts positional git hook arguments.
    pub fn accepts_git_hook_args(&self) -> bool {
        matches!(self, Self::GitRefUpdate | Self::GitPrePush)
    }
}

/// Typed input for the `hook` command family.
pub enum HookInput {
    /// Dispatch a security-critical hook via Rust logic.
    Dispatch {
        /// The hook to dispatch.
        hook: HookName,
        /// Positional arguments supplied by git process hooks.
        git_hook_args: Vec<String>,
    },
}

// ---------------------------------------------------------------------------
// Hook name classification helpers
// ---------------------------------------------------------------------------

/// Returns `true` if the hook is a PostToolUse hook (cannot block).
fn is_post_tool_use(_hook_name: &str) -> bool {
    false
}

/// Returns `true` if the hook name is dispatched from git's process-level hooks.
/// These hooks do not send a JSON envelope on stdin; positional args are used instead.
fn is_git_process_hook(hook_name: &str) -> bool {
    matches!(hook_name, "git-ref-update" | "git-pre-push")
}

// ---------------------------------------------------------------------------
// JSON envelope parsing helpers (manual — no serde derive)
// ---------------------------------------------------------------------------

/// Typed error for JSON envelope parsing failures.
#[derive(Debug)]
enum HookParseError {
    InvalidJson(String),
    MissingField(String),
    Unmappable(String),
}

impl std::fmt::Display for HookParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidJson(msg) => write!(f, "{msg}"),
            Self::MissingField(msg) => write!(f, "{msg}"),
            Self::Unmappable(msg) => write!(f, "{msg}"),
        }
    }
}

/// Parsed and provider-normalized data from a PreToolUse hook JSON envelope.
struct ParsedHookEnvelope {
    tool_name: String,
    command: Option<String>,
    file_path: Option<PathBuf>,
    content: Option<String>,
}

/// Parse a Claude Code or Grok PreToolUse hook JSON envelope from `raw`.
///
/// Claude uses snake-case `tool_name` / `tool_input`; Grok uses camel-case
/// `toolName` / `toolInput` and provider-specific tool identifiers. Grok input
/// is normalized here to the tool names and fields consumed by the existing
/// Claude-oriented hook handlers.
///
/// Returns `Err(HookParseError)` when the JSON is invalid, required fields are
/// missing, or a Grok envelope cannot be mapped to the existing contract.
fn parse_hook_envelope(raw: &str) -> Result<ParsedHookEnvelope, HookParseError> {
    let value: serde_json::Value = serde_json::from_str(raw)
        .map_err(|e| HookParseError::InvalidJson(format!("failed to parse hook JSON: {e}")))?;

    match (value.get("tool_name"), value.get("toolName")) {
        (Some(_), Some(_)) => Err(HookParseError::Unmappable(
            "hook JSON contains both Claude 'tool_name' and Grok 'toolName' fields".to_owned(),
        )),
        (Some(_), None) => parse_claude_hook_envelope(&value),
        (None, Some(_)) => parse_grok_hook_envelope(&value),
        (None, None) => Err(HookParseError::MissingField(
            "hook JSON missing required field 'tool_name'".to_owned(),
        )),
    }
}

fn parse_claude_hook_envelope(
    value: &serde_json::Value,
) -> Result<ParsedHookEnvelope, HookParseError> {
    let tool_name = value
        .get("tool_name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            HookParseError::MissingField("hook JSON missing required field 'tool_name'".to_owned())
        })?
        .to_owned();

    let tool_input = value.get("tool_input");

    let command =
        tool_input.and_then(|ti| ti.get("command")).and_then(|v| v.as_str()).map(str::to_owned);

    let file_path =
        tool_input.and_then(|ti| ti.get("file_path")).and_then(|v| v.as_str()).map(PathBuf::from);

    let content = tool_input.and_then(|ti| ti.get("content")).and_then(flatten_content_text);

    Ok(ParsedHookEnvelope { tool_name, command, file_path, content })
}

/// Normalize a Grok hook envelope to the existing Claude hook-handler fields.
///
/// Grok's terminal tool is equivalent to Claude's `Bash` input. Its
/// `search_replace` tool is represented by the existing `Write` contract so
/// the test-file guard can inspect the target path and replacement content;
/// an empty replacement therefore remains fail-closed for test files.
fn parse_grok_hook_envelope(
    value: &serde_json::Value,
) -> Result<ParsedHookEnvelope, HookParseError> {
    let tool_name =
        value.get("toolName").and_then(|v| v.as_str()).filter(|name| !name.is_empty()).ok_or_else(
            || {
                HookParseError::MissingField(
                    "Grok hook JSON missing required field 'toolName'".to_owned(),
                )
            },
        )?;

    let tool_input =
        value.get("toolInput").and_then(|input| input.as_object()).ok_or_else(|| {
            HookParseError::Unmappable(
                "Grok hook JSON field 'toolInput' must be an object".to_owned(),
            )
        })?;

    match tool_name {
        "run_terminal_command" => {
            let command = required_grok_string(tool_input, tool_name, "command", false)?;
            Ok(ParsedHookEnvelope {
                tool_name: "Bash".to_owned(),
                command: Some(command),
                file_path: None,
                content: None,
            })
        }
        "search_replace" => {
            let file_path = required_grok_string(tool_input, tool_name, "file_path", false)?;
            // Validate the complete search-replace shape even though the
            // existing Write contract only needs the resulting content.
            let _old_string = required_grok_string(tool_input, tool_name, "old_string", true)?;
            let new_string = required_grok_string(tool_input, tool_name, "new_string", true)?;
            Ok(ParsedHookEnvelope {
                tool_name: "Write".to_owned(),
                command: None,
                file_path: Some(PathBuf::from(file_path)),
                content: Some(new_string),
            })
        }
        other => Err(HookParseError::Unmappable(format!(
            "Grok hook tool '{other}' cannot be mapped to the Claude hook contract"
        ))),
    }
}

fn required_grok_string(
    tool_input: &serde_json::Map<String, serde_json::Value>,
    tool_name: &str,
    field: &str,
    allow_empty: bool,
) -> Result<String, HookParseError> {
    let value = tool_input.get(field).and_then(|value| value.as_str()).ok_or_else(|| {
        HookParseError::Unmappable(format!(
            "Grok hook tool '{tool_name}' requires string field 'toolInput.{field}'"
        ))
    })?;

    if !allow_empty && value.is_empty() {
        return Err(HookParseError::Unmappable(format!(
            "Grok hook tool '{tool_name}' requires non-empty field 'toolInput.{field}'"
        )));
    }

    Ok(value.to_owned())
}

/// Flatten a JSON content value (string, array of blocks, or object) into a plain string.
///
/// Mirrors the `flatten_content_text` helper in `cli_composition/hook.rs`.
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

/// Extract the `prompt` field from a UserPromptSubmit hook JSON envelope.
///
/// Returns an empty string when the field is absent or the JSON is invalid
/// (advisory hook — never blocks on parse failure).
fn parse_prompt_envelope(raw: &str) -> String {
    serde_json::from_str::<serde_json::Value>(raw)
        .ok()
        .and_then(|v| v.get("prompt").and_then(|p| p.as_str()).map(str::to_owned))
        .unwrap_or_default()
}

// ---------------------------------------------------------------------------
// Driver
// ---------------------------------------------------------------------------

/// Primary adapter driver for the `hook` command family.
///
/// Holds injected use-case interactors; exposes `handle(input) -> CommandOutcome`.
pub struct HookDriver {
    hook_dispatch_service: Arc<dyn HookDispatchService>,
}

impl HookDriver {
    /// Create a new `HookDriver` with the given dispatch service.
    pub fn new(hook_dispatch_service: Arc<dyn HookDispatchService>) -> Self {
        Self { hook_dispatch_service }
    }

    /// Handle a hook command.
    ///
    /// Exit code 0 = allow, exit code 2 = block (Claude Code hook protocol).
    /// PreToolUse hooks: any internal error → exit code 2 (fail-closed).
    pub fn handle(&self, input: HookInput) -> CommandOutcome {
        match input {
            HookInput::Dispatch { hook, git_hook_args } => self.hook_dispatch(hook, git_hook_args),
        }
    }

    // -----------------------------------------------------------------------
    // Internal dispatch helpers
    // -----------------------------------------------------------------------

    fn hook_dispatch(&self, hook: HookName, git_hook_args: Vec<String>) -> CommandOutcome {
        if !git_hook_args.is_empty() && !hook.accepts_git_hook_args() {
            return CommandOutcome {
                stdout: None,
                stderr: Some(
                    "extra hook arguments are only supported for git process hooks".to_owned(),
                ),
                exit_code: 2,
            };
        }

        let hook_name = hook.hook_name().to_owned();
        let is_post = is_post_tool_use(&hook_name);

        // Build the dispatch command.
        // Stdin-reading strategy is determined by the hook type (I/O boundary responsibility):
        //   - git process hooks: no stdin envelope; use a placeholder Git command.
        //   - skill-compliance: read stdin, parse prompt field (UserPromptSubmit envelope).
        //   - all others: read stdin, parse PreToolUse JSON envelope.
        let dispatch_cmd = if is_git_process_hook(&hook_name) {
            HookDispatchCommand {
                tool_name: "Git".to_owned(),
                command: None,
                file_path: None,
                content: None,
                git_hook_args: git_hook_args.clone(),
            }
        } else if hook_name == "skill-compliance" {
            // Advisory hook — never block on stdin errors.
            let mut stdin_buf = String::new();
            if std::io::stdin().read_to_string(&mut stdin_buf).is_err() {
                // Fall through to dispatch with empty content; usecase returns Allow + None.
                stdin_buf = String::new();
            }
            let prompt = parse_prompt_envelope(stdin_buf.trim());
            HookDispatchCommand {
                tool_name: "UserPromptSubmit".to_owned(),
                command: None,
                file_path: None,
                content: if prompt.is_empty() { None } else { Some(prompt) },
                git_hook_args: vec![],
            }
        } else {
            // PreToolUse / PostToolUse: read stdin JSON envelope.
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

            match parse_hook_envelope(&stdin_buf) {
                Ok(parsed) => HookDispatchCommand {
                    tool_name: parsed.tool_name,
                    command: parsed.command,
                    file_path: parsed.file_path,
                    content: parsed.content,
                    git_hook_args: vec![],
                },
                Err(e) => return make_hook_error(is_post, &e.to_string()),
            }
        };

        // Single unconditional dispatch — ALL routing is inside the usecase.
        let result = self.hook_dispatch_service.dispatch(hook_name, dispatch_cmd);

        match result {
            Ok(verdict) => {
                // skill-compliance returns pre-formatted JSON output via this field.
                if let Some(output) = verdict.skill_compliance_output {
                    return CommandOutcome::success(Some(output));
                }
                if verdict.decision == HookVerdictDecision::Block {
                    let reason = verdict.reason.unwrap_or_default();
                    CommandOutcome {
                        stdout: None,
                        stderr: if reason.is_empty() { None } else { Some(reason) },
                        exit_code: 2,
                    }
                } else {
                    CommandOutcome::success(None)
                }
            }
            Err(e) => make_hook_error(is_post, &format!("hook error: {e}")),
        }
    }
}

// ---------------------------------------------------------------------------
// Error helpers
// ---------------------------------------------------------------------------

/// Build a `CommandOutcome` for a hook error, respecting pre/post semantics.
fn make_hook_error(is_post_tool_use: bool, message: &str) -> CommandOutcome {
    if is_post_tool_use {
        // PostToolUse: warn + exit 0 (cannot block)
        CommandOutcome { stdout: None, stderr: Some(format!("warning: {message}")), exit_code: 0 }
    } else {
        // PreToolUse: exit 2 (fail-closed)
        CommandOutcome { stdout: None, stderr: Some(format!("error: {message}")), exit_code: 2 }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use std::sync::Arc;

    use usecase::hook_dispatch::{
        HookDispatchCommand, HookDispatchError, HookDispatchService, HookVerdictOutput,
    };

    use super::{
        HookDriver, HookInput, HookName, HookParseError, make_hook_error, parse_hook_envelope,
    };

    #[test]
    fn test_parse_hook_envelope_grok_camel_case_terminal_input_maps_to_bash() {
        let result = parse_hook_envelope(
            r#"{"toolName":"run_terminal_command","toolInput":{"command":"git status"}}"#,
        );

        match result {
            Ok(parsed) => {
                assert_eq!(parsed.tool_name, "Bash");
                assert_eq!(parsed.command.as_deref(), Some("git status"));
                assert!(parsed.file_path.is_none());
                assert!(parsed.content.is_none());
            }
            Err(error) => panic!("unexpected parse error: {error}"),
        }
    }

    #[test]
    fn test_parse_hook_envelope_grok_search_replace_maps_to_write_contract() {
        let result = parse_hook_envelope(
            r#"{"toolName":"search_replace","toolInput":{"file_path":"tests/example.rs","old_string":"old","new_string":"new"}}"#,
        );

        match result {
            Ok(parsed) => {
                assert_eq!(parsed.tool_name, "Write");
                assert!(parsed.command.is_none());
                assert_eq!(
                    parsed.file_path.as_deref().and_then(|path| path.to_str()),
                    Some("tests/example.rs")
                );
                assert_eq!(parsed.content.as_deref(), Some("new"));
            }
            Err(error) => panic!("unexpected parse error: {error}"),
        }
    }

    #[test]
    fn test_parse_hook_envelope_grok_unknown_tool_fails_closed() {
        let result = parse_hook_envelope(
            r#"{"toolName":"unknown_tool","toolInput":{"command":"git status"}}"#,
        );

        assert!(matches!(
            result,
            Err(HookParseError::Unmappable(message))
                if message.contains("cannot be mapped")
        ));
    }

    #[test]
    fn test_parse_hook_envelope_grok_missing_input_fails_closed() {
        let result = parse_hook_envelope(r#"{"toolName":"run_terminal_command"}"#);

        assert!(matches!(
            result,
            Err(HookParseError::Unmappable(message))
                if message.contains("toolInput")
        ));
    }

    #[test]
    fn test_hook_driver_non_git_arguments_returns_fail_closed_command_outcome() {
        let driver = HookDriver::new(Arc::new(RecordingHookService));
        let outcome = driver.handle(HookInput::Dispatch {
            hook: HookName::BlockDirectGitOps,
            git_hook_args: vec!["unexpected".to_owned()],
        });

        assert_eq!(outcome.exit_code, 2);
        assert_eq!(
            outcome.stderr.as_deref(),
            Some("extra hook arguments are only supported for git process hooks")
        );
        assert!(outcome.stdout.is_none());
    }

    #[test]
    fn test_make_hook_error_unmappable_input_returns_fail_closed_command_outcome() {
        let outcome = make_hook_error(false, "unmappable Grok hook input");

        assert_eq!(outcome.exit_code, 2);
        assert_eq!(outcome.stderr.as_deref(), Some("error: unmappable Grok hook input"));
        assert!(outcome.stdout.is_none());
    }

    struct RecordingHookService;

    impl HookDispatchService for RecordingHookService {
        fn dispatch(
            &self,
            _hook_name: String,
            _command: HookDispatchCommand,
        ) -> Result<HookVerdictOutput, HookDispatchError> {
            Err(HookDispatchError::UnknownHookName("test".to_owned()))
        }

        fn check_skill_compliance(&self, _prompt: &str) -> Option<String> {
            None
        }
    }
}
