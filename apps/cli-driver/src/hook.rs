// STAGED FOR T021 — not yet compiled; Cargo.toml + workspace member added atomically in T021 per CN-06.
//
//! `hook` command family — primary adapter driver.
//!
//! `HookDriver` holds injected use-case interactors and exposes
//! `handle(input) -> HookExecution`.
//!
//! Stdin reading and provider hook JSON envelope parsing are performed at this
//! driver boundary (CN-02): the driver owns I/O, selects the envelope contract
//! from the explicit host, normalizes the selected provider envelope, and
//! converts the result into
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

/// Driver-boundary connection-host selector.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookHost {
    /// Claude Code's snake-case hook envelope.
    Claude,
    /// Codex's snake-case hook envelope.
    Codex,
    /// Grok's camel-case hook envelope.
    Grok,
}

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

/// Raw hook-driver boundary record; `git_hook_args` is opaque ordered
/// process-argument payload without a narrower domain concept.
pub struct HookInput {
    /// The parsed hook selector.
    pub hook: HookName,
    /// The selected agent host, or `None` for a Git process hook.
    pub host: Option<HookHost>,
    /// Positional arguments supplied by a Git process hook.
    pub git_hook_args: Vec<String>,
}

/// Hook-specific rendered execution classification.
pub enum HookExecution {
    /// The input failed validation, parsing, or I/O handling.
    InputError(CommandOutcome),
    /// The post-dispatch service or handler failed after valid input reached the use case.
    InternalError(CommandOutcome),
    /// The use-case handler deliberately blocked the hook operation.
    HookBlock(CommandOutcome),
    /// An advisory hook produced context for the host.
    AdvisoryFired(CommandOutcome),
    /// The hook completed without a block or advisory output.
    Allow(CommandOutcome),
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
    InvalidField(String),
    MissingField(String),
    Unmappable(String),
}

impl std::fmt::Display for HookParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidJson(msg) => write!(f, "{msg}"),
            Self::InvalidField(msg) => write!(f, "{msg}"),
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

/// Parse a host-selected PreToolUse hook JSON envelope from `raw`.
///
/// Claude and Codex use snake-case `tool_name` / `tool_input`; Grok uses
/// camel-case `toolName` / `toolInput` and provider-specific tool identifiers.
/// The selected host is authoritative: the parser never infers a host from
/// the JSON or falls back to another provider's envelope contract.
///
/// Returns `Err(HookParseError)` when the JSON is invalid, required fields are
/// missing, or a Grok envelope cannot be mapped to the existing contract.
fn parse_hook_envelope(raw: &str, host: HookHost) -> Result<ParsedHookEnvelope, HookParseError> {
    let value: serde_json::Value = serde_json::from_str(raw)
        .map_err(|e| HookParseError::InvalidJson(format!("failed to parse hook JSON: {e}")))?;

    match host {
        HookHost::Claude | HookHost::Codex => parse_claude_hook_envelope(&value),
        HookHost::Grok => parse_grok_hook_envelope(&value),
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
fn parse_prompt_envelope(raw: &str) -> Result<String, HookParseError> {
    let value: serde_json::Value = serde_json::from_str(raw)
        .map_err(|e| HookParseError::InvalidJson(format!("failed to parse prompt JSON: {e}")))?;

    let prompt = value.get("prompt").ok_or_else(|| {
        HookParseError::MissingField(
            "UserPromptSubmit hook JSON missing required field 'prompt'".to_owned(),
        )
    })?;

    prompt.as_str().map(str::to_owned).ok_or_else(|| {
        HookParseError::InvalidField(
            "UserPromptSubmit hook JSON field 'prompt' must be a string".to_owned(),
        )
    })
}

// ---------------------------------------------------------------------------
// Driver
// ---------------------------------------------------------------------------

/// Primary adapter driver for the `hook` command family.
///
/// Holds injected use-case interactors; exposes `handle(input) -> HookExecution`.
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
    /// Validate the hook-kind/host/argv combination and dispatch it.
    ///
    /// Exit code 0 = allow, exit code 2 = block or input error (Claude Code
    /// hook protocol). The typed return keeps input failures, internal service
    /// failures, and actual use-case blocks distinct for callers such as telemetry.
    pub fn handle(&self, input: HookInput) -> HookExecution {
        let HookInput { hook, host, git_hook_args } = input;

        if hook.accepts_git_hook_args() {
            self.git_dispatch(hook, host, git_hook_args)
        } else {
            self.agent_dispatch(hook, host, git_hook_args)
        }
    }

    // -----------------------------------------------------------------------
    // Internal dispatch helpers
    // -----------------------------------------------------------------------

    fn agent_dispatch(
        &self,
        hook: HookName,
        host: Option<HookHost>,
        git_hook_args: Vec<String>,
    ) -> HookExecution {
        let Some(host) = host else {
            return HookExecution::InputError(make_hook_error(
                false,
                "agent hooks require --host claude|codex|grok",
            ));
        };

        if !git_hook_args.is_empty() {
            return HookExecution::InputError(make_hook_error(
                false,
                "extra hook arguments are only supported for git process hooks",
            ));
        }

        let hook_name = hook.hook_name().to_owned();
        let is_post = is_post_tool_use(&hook_name);

        let mut stdin_buf = String::new();
        if let Err(e) = std::io::stdin().read_to_string(&mut stdin_buf) {
            return HookExecution::InputError(make_hook_error(
                is_post,
                &format!("failed to read stdin: {e}"),
            ));
        }

        self.dispatch_agent_input(hook, host, &stdin_buf)
    }

    fn dispatch_agent_input(
        &self,
        hook: HookName,
        host: HookHost,
        stdin_buf: &str,
    ) -> HookExecution {
        let hook_name = hook.hook_name().to_owned();
        let is_post = is_post_tool_use(&hook_name);

        // Build the dispatch command.
        // Stdin-reading strategy is determined by the hook type (I/O boundary responsibility):
        //   - skill-compliance: read stdin, parse the common prompt field.
        //   - all other agent hooks: read stdin, parse the selected host's
        //     PreToolUse JSON envelope.
        let dispatch_cmd = if hook_name == "skill-compliance" {
            let prompt = match parse_prompt_envelope(stdin_buf.trim()) {
                Ok(prompt) => prompt,
                Err(e) => {
                    return HookExecution::InputError(make_hook_error(is_post, &e.to_string()));
                }
            };
            HookDispatchCommand {
                tool_name: "UserPromptSubmit".to_owned(),
                command: None,
                file_path: None,
                content: if prompt.is_empty() { None } else { Some(prompt) },
                git_hook_args: vec![],
            }
        } else {
            if stdin_buf.trim().is_empty() {
                return HookExecution::InputError(make_hook_error(
                    is_post,
                    "hook received empty stdin — no envelope to check",
                ));
            }

            match parse_hook_envelope(stdin_buf, host) {
                Ok(parsed) => HookDispatchCommand {
                    tool_name: parsed.tool_name,
                    command: parsed.command,
                    file_path: parsed.file_path,
                    content: parsed.content,
                    git_hook_args: vec![],
                },
                Err(e) => {
                    return HookExecution::InputError(make_hook_error(is_post, &e.to_string()));
                }
            }
        };

        self.dispatch_command(hook_name, is_post, dispatch_cmd)
    }

    fn git_dispatch(
        &self,
        hook: HookName,
        host: Option<HookHost>,
        git_hook_args: Vec<String>,
    ) -> HookExecution {
        let hook_name = hook.hook_name().to_owned();
        if host.is_some() {
            return HookExecution::InputError(make_hook_error(
                false,
                "git process hooks must not specify --host",
            ));
        }

        if !is_git_process_hook(&hook_name) {
            return HookExecution::InputError(make_hook_error(
                false,
                "extra hook arguments are only supported for git process hooks",
            ));
        }

        // Git process hooks do not receive an agent envelope on stdin. Keep
        // this path separate so their positional arguments reach the existing
        // process-level handlers without any provider parsing.
        let dispatch_cmd = HookDispatchCommand {
            tool_name: "Git".to_owned(),
            command: None,
            file_path: None,
            content: None,
            git_hook_args,
        };

        self.dispatch_command(hook_name, false, dispatch_cmd)
    }

    fn dispatch_command(
        &self,
        hook_name: String,
        is_post: bool,
        dispatch_cmd: HookDispatchCommand,
    ) -> HookExecution {
        // Single unconditional dispatch — ALL routing is inside the usecase.
        let result = self.hook_dispatch_service.dispatch(hook_name, dispatch_cmd);

        match result {
            Ok(verdict) => {
                if verdict.decision == HookVerdictDecision::Block {
                    let reason = verdict.reason.unwrap_or_default();
                    return HookExecution::HookBlock(CommandOutcome {
                        stdout: None,
                        stderr: if reason.is_empty() { None } else { Some(reason) },
                        exit_code: 2,
                    });
                }
                // skill-compliance returns pre-formatted JSON output via this field.
                if let Some(output) = verdict.skill_compliance_output {
                    HookExecution::AdvisoryFired(CommandOutcome::success(Some(output)))
                } else {
                    HookExecution::Allow(CommandOutcome::success(None))
                }
            }
            Err(e) => {
                HookExecution::InternalError(make_hook_error(is_post, &format!("hook error: {e}")))
            }
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
    use std::sync::{Arc, Mutex};

    use crate::render::CommandOutcome;
    use usecase::hook_dispatch::{
        HookDispatchCommand, HookDispatchError, HookDispatchService, HookVerdictDecision,
        HookVerdictOutput,
    };

    use super::{
        HookDriver, HookExecution, HookHost, HookInput, HookName, HookParseError, make_hook_error,
        parse_hook_envelope, parse_prompt_envelope,
    };

    #[test]
    fn test_parse_hook_envelope_grok_terminal_ignores_conflicting_aliases() {
        let parsed = parse_hook_envelope(
            r#"{"toolName":"run_terminal_command","toolInput":{"command":"git status"},"tool_name":"Write","tool_input":{"file_path":"tests/example.rs","content":""}}"#,
            HookHost::Grok,
        )
        .expect("Grok terminal envelope must parse");

        assert_eq!(parsed.tool_name, "Bash");
        assert_eq!(parsed.command.as_deref(), Some("git status"));
        assert!(parsed.file_path.is_none());
        assert!(parsed.content.is_none());
    }

    #[test]
    fn test_parse_hook_envelope_grok_search_replace_ignores_conflicting_aliases() {
        let parsed = parse_hook_envelope(
            r#"{"toolName":"search_replace","toolInput":{"file_path":"tests/example.rs","old_string":"old","new_string":"new"},"tool_name":"run_terminal_command","tool_input":{"command":"git status"}}"#,
            HookHost::Grok,
        )
        .expect("Grok search-replace envelope must parse");

        assert_eq!(parsed.tool_name, "Write");
        assert!(parsed.command.is_none());
        assert_eq!(
            parsed.file_path.as_deref().and_then(|path| path.to_str()),
            Some("tests/example.rs")
        );
        assert_eq!(parsed.content.as_deref(), Some("new"));
    }

    #[test]
    fn test_parse_hook_envelope_grok_rejects_unknown_and_invalid_values() {
        let cases = [
            (
                r#"{"toolName":"unknown_tool","toolInput":{"command":"git status"}}"#,
                "cannot be mapped",
            ),
            (r#"{"toolName":"run_terminal_command"}"#, "toolInput"),
            (
                r#"{"toolName":"run_terminal_command","toolInput":{"command":42}}"#,
                "requires string field",
            ),
        ];

        for (raw, message) in cases {
            let result = parse_hook_envelope(raw, HookHost::Grok);
            assert!(matches!(
                result,
                Err(HookParseError::Unmappable(actual)) if actual.contains(message)
            ));
        }
    }

    #[test]
    fn test_parse_hook_envelope_grok_rejects_missing_required_values() {
        let cases = [
            (
                r#"{"toolName":"search_replace","toolInput":{"old_string":"old","new_string":"new"}}"#,
                "file_path",
            ),
            (
                r#"{"toolName":"search_replace","toolInput":{"file_path":"tests/example.rs","new_string":"new"}}"#,
                "old_string",
            ),
            (
                r#"{"toolName":"search_replace","toolInput":{"file_path":"tests/example.rs","old_string":"old"}}"#,
                "new_string",
            ),
        ];

        for (raw, field) in cases {
            let result = parse_hook_envelope(raw, HookHost::Grok);
            assert!(matches!(
                result,
                Err(HookParseError::Unmappable(message)) if message.contains(field)
            ));
        }
    }

    #[test]
    fn test_parse_hook_envelope_uses_only_the_selected_host_contract() {
        let grok_for_claude = parse_hook_envelope(
            r#"{"toolName":"run_terminal_command","toolInput":{"command":"git status"}}"#,
            HookHost::Claude,
        );
        assert!(matches!(
            grok_for_claude,
            Err(HookParseError::MissingField(message)) if message.contains("tool_name")
        ));

        let claude_for_grok = parse_hook_envelope(
            r#"{"tool_name":"Bash","tool_input":{"command":"git status"}}"#,
            HookHost::Grok,
        );
        assert!(matches!(
            claude_for_grok,
            Err(HookParseError::MissingField(message)) if message.contains("toolName")
        ));
    }

    #[test]
    fn test_parse_hook_envelope_claude_and_codex_share_snake_case_contract() {
        let raw = r#"{"tool_name":"Bash","tool_input":{"command":"git status"}}"#;

        for host in [HookHost::Claude, HookHost::Codex] {
            let parsed = parse_hook_envelope(raw, host).expect("snake-case envelope must parse");
            assert_eq!(parsed.tool_name, "Bash");
            assert_eq!(parsed.command.as_deref(), Some("git status"));
        }
    }

    #[test]
    fn test_parse_hook_envelope_codex_apply_patch_remains_snake_case() {
        let parsed = parse_hook_envelope(
            r#"{"tool_name":"apply_patch","tool_input":{"patch":"*** Begin Patch"}}"#,
            HookHost::Codex,
        )
        .expect("Codex apply_patch envelope must parse");

        assert_eq!(parsed.tool_name, "apply_patch");
        assert!(parsed.command.is_none());
        assert!(parsed.file_path.is_none());
        assert!(parsed.content.is_none());
    }

    #[test]
    fn test_parse_prompt_envelope_preserves_prompt_and_rejects_invalid_input() {
        assert_eq!(
            parse_prompt_envelope(r#"{"prompt":"/track:review"}"#).unwrap(),
            "/track:review"
        );
        assert!(matches!(
            parse_prompt_envelope("not json"),
            Err(HookParseError::InvalidJson(message)) if message.contains("prompt JSON")
        ));
        assert!(matches!(
            parse_prompt_envelope(r#"{"prompt":42}"#),
            Err(HookParseError::InvalidField(message)) if message.contains("prompt")
        ));
    }

    #[test]
    fn test_hook_input_is_raw_record_with_optional_host_and_ordered_git_argv() {
        let input = HookInput {
            hook: HookName::GitPrePush,
            host: None,
            git_hook_args: vec!["origin".to_owned(), "https://example.com".to_owned()],
        };

        assert_eq!(input.hook.hook_name(), "git-pre-push");
        assert!(input.host.is_none());
        assert_eq!(input.git_hook_args, ["origin", "https://example.com"]);
    }

    #[test]
    fn test_hook_driver_missing_agent_host_is_input_error() {
        for hook in [HookName::BlockDirectGitOps, HookName::SkillCompliance] {
            let (driver, service) = driver_with(Response::Allow);
            let execution = driver.handle(HookInput { hook, host: None, git_hook_args: vec![] });

            assert!(matches!(execution, HookExecution::InputError(_)));
            assert_eq!(outcome(&execution).exit_code, 2);
            assert_eq!(
                outcome(&execution).stderr.as_deref(),
                Some("error: agent hooks require --host claude|codex|grok")
            );
            assert!(service.calls.lock().unwrap().is_empty());
        }
    }

    #[test]
    fn test_hook_driver_forbids_host_on_git_process_hook() {
        let (driver, service) = driver_with(Response::Allow);
        let execution = driver.handle(HookInput {
            hook: HookName::GitRefUpdate,
            host: Some(HookHost::Claude),
            git_hook_args: vec!["committed".to_owned()],
        });

        assert!(matches!(execution, HookExecution::InputError(_)));
        assert_eq!(outcome(&execution).exit_code, 2);
        assert!(outcome(&execution).stderr.as_deref().unwrap().contains("must not specify --host"));
        assert!(service.calls.lock().unwrap().is_empty());
    }

    #[test]
    fn test_hook_driver_forbids_git_argv_on_agent_hook() {
        let (driver, service) = driver_with(Response::Allow);
        let execution = driver.handle(HookInput {
            hook: HookName::BlockDirectGitOps,
            host: Some(HookHost::Claude),
            git_hook_args: vec!["unexpected".to_owned()],
        });

        assert!(matches!(execution, HookExecution::InputError(_)));
        assert_eq!(outcome(&execution).exit_code, 2);
        assert!(
            outcome(&execution)
                .stderr
                .as_deref()
                .unwrap()
                .contains("only supported for git process hooks")
        );
        assert!(service.calls.lock().unwrap().is_empty());
    }

    #[test]
    fn test_hook_driver_git_dispatch_preserves_argv_and_allows() {
        let (driver, service) = driver_with(Response::Allow);
        let execution = driver.handle(HookInput {
            hook: HookName::GitPrePush,
            host: None,
            git_hook_args: vec!["origin".to_owned(), "https://example.com".to_owned()],
        });

        assert!(matches!(execution, HookExecution::Allow(_)));
        assert_eq!(outcome(&execution).exit_code, 0);
        let (_, command) = service.calls.lock().unwrap().pop().expect("dispatch call");
        assert_eq!(command.tool_name, "Git");
        assert_eq!(command.git_hook_args, ["origin", "https://example.com"]);
    }

    #[test]
    fn test_hook_driver_distinguishes_actual_block_from_input_error() {
        let (blocking_driver, _) = driver_with(Response::Block);
        let block = blocking_driver.handle(HookInput {
            hook: HookName::GitPrePush,
            host: None,
            git_hook_args: vec!["origin".to_owned()],
        });

        let (error_driver, _) = driver_with(Response::Allow);
        let input_error = error_driver.handle(HookInput {
            hook: HookName::BlockDirectGitOps,
            host: None,
            git_hook_args: vec![],
        });

        assert!(matches!(block, HookExecution::HookBlock(_)));
        assert!(matches!(input_error, HookExecution::InputError(_)));
        assert_eq!(outcome(&block).exit_code, 2);
        assert_eq!(outcome(&input_error).exit_code, 2);
    }

    #[test]
    fn test_hook_driver_agent_parse_failure_is_input_error() {
        let (driver, service) = driver_with(Response::Allow);
        let execution =
            driver.dispatch_agent_input(HookName::BlockDirectGitOps, HookHost::Claude, "not json");

        assert!(matches!(execution, HookExecution::InputError(_)));
        assert_eq!(outcome(&execution).exit_code, 2);
        assert!(service.calls.lock().unwrap().is_empty());
    }

    #[test]
    fn test_hook_driver_claude_and_codex_share_pretool_use_normalization_and_guard_behavior() {
        let raw = r#"{"tool_name":"Bash","tool_input":{"command":"git add safe.txt"}}"#;

        for host in [HookHost::Claude, HookHost::Codex] {
            let (driver, service) = driver_with(Response::Block);
            let execution = driver.dispatch_agent_input(HookName::BlockDirectGitOps, host, raw);

            assert!(matches!(execution, HookExecution::HookBlock(_)));
            assert_eq!(outcome(&execution).exit_code, 2);
            assert_eq!(outcome(&execution).stderr.as_deref(), Some("blocked by test service"));

            let (hook_name, command) = service.calls.lock().unwrap().pop().expect("dispatch call");
            assert_eq!(hook_name, "block-direct-git-ops");
            assert_eq!(command.tool_name, "Bash");
            assert_eq!(command.command.as_deref(), Some("git add safe.txt"));
        }
    }

    #[test]
    fn test_hook_driver_skill_context_is_advisory_fired() {
        let (driver, service) = driver_with(Response::Advisory);
        let execution = driver.dispatch_agent_input(
            HookName::SkillCompliance,
            HookHost::Grok,
            r#"{"prompt":"/track:review"}"#,
        );

        assert!(matches!(execution, HookExecution::AdvisoryFired(_)));
        assert_eq!(outcome(&execution).exit_code, 0);
        assert!(outcome(&execution).stdout.as_deref().unwrap().contains("additionalContext"));
        let (hook_name, command) = service.calls.lock().unwrap().pop().expect("dispatch call");
        assert_eq!(hook_name, "skill-compliance");
        assert_eq!(command.tool_name, "UserPromptSubmit");
        assert_eq!(command.content.as_deref(), Some("/track:review"));
    }

    #[test]
    fn test_hook_driver_skill_parse_failure_is_input_error_for_outer_advisory_absorption() {
        let (driver, service) = driver_with(Response::Advisory);
        let execution =
            driver.dispatch_agent_input(HookName::SkillCompliance, HookHost::Claude, "not json");

        assert!(matches!(execution, HookExecution::InputError(_)));
        assert_eq!(outcome(&execution).exit_code, 2);
        assert!(service.calls.lock().unwrap().is_empty());
    }

    #[test]
    fn test_hook_driver_valid_skill_without_context_is_allow() {
        let (driver, _) = driver_with(Response::Allow);
        let execution = driver.dispatch_agent_input(
            HookName::SkillCompliance,
            HookHost::Claude,
            r#"{"prompt":"hello"}"#,
        );

        assert!(matches!(execution, HookExecution::Allow(_)));
        assert_eq!(outcome(&execution).exit_code, 0);
        assert!(outcome(&execution).stdout.is_none());
    }

    #[test]
    fn test_hook_driver_service_failure_is_internal_error() {
        let (driver, _) = driver_with(Response::Error);
        let execution = driver.dispatch_agent_input(
            HookName::BlockDirectGitOps,
            HookHost::Claude,
            r#"{"tool_name":"Bash","tool_input":{"command":"printf ok"}}"#,
        );

        assert!(matches!(execution, HookExecution::InternalError(_)));
        assert_eq!(outcome(&execution).exit_code, 2);
        assert!(outcome(&execution).stderr.as_deref().unwrap().contains("hook error"));
    }

    #[test]
    fn test_make_hook_error_is_fail_closed_for_pre_tool_use() {
        let outcome = make_hook_error(false, "unmappable Grok hook input");

        assert_eq!(outcome.exit_code, 2);
        assert_eq!(outcome.stderr.as_deref(), Some("error: unmappable Grok hook input"));
        assert!(outcome.stdout.is_none());
    }

    fn outcome(execution: &HookExecution) -> &CommandOutcome {
        match execution {
            HookExecution::InputError(outcome)
            | HookExecution::InternalError(outcome)
            | HookExecution::HookBlock(outcome)
            | HookExecution::AdvisoryFired(outcome)
            | HookExecution::Allow(outcome) => outcome,
        }
    }

    fn driver_with(response: Response) -> (HookDriver, Arc<StubHookService>) {
        let service = Arc::new(StubHookService { response, calls: Mutex::new(Vec::new()) });
        let driver = HookDriver::new(service.clone());
        (driver, service)
    }

    #[derive(Clone, Copy)]
    enum Response {
        Allow,
        Block,
        Advisory,
        Error,
    }

    struct StubHookService {
        response: Response,
        calls: Mutex<Vec<(String, HookDispatchCommand)>>,
    }

    impl HookDispatchService for StubHookService {
        fn dispatch(
            &self,
            hook_name: String,
            command: HookDispatchCommand,
        ) -> Result<HookVerdictOutput, HookDispatchError> {
            self.calls.lock().unwrap().push((hook_name, command));
            match self.response {
                Response::Allow => Ok(HookVerdictOutput {
                    decision: HookVerdictDecision::Allow,
                    reason: None,
                    skill_compliance_output: None,
                }),
                Response::Block => Ok(HookVerdictOutput {
                    decision: HookVerdictDecision::Block,
                    reason: Some("blocked by test service".to_owned()),
                    skill_compliance_output: None,
                }),
                Response::Advisory => Ok(HookVerdictOutput {
                    decision: HookVerdictDecision::Allow,
                    reason: None,
                    skill_compliance_output: Some(
                        r#"{"hookSpecificOutput":{"additionalContext":"advice"}}"#.to_owned(),
                    ),
                }),
                Response::Error => {
                    Err(HookDispatchError::HandlerFailed("stub service failure".to_owned()))
                }
            }
        }

        fn check_skill_compliance(&self, _prompt: &str) -> Option<String> {
            None
        }
    }
}
