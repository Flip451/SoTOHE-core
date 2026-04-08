//! Hook dispatch subcommand for security-critical hooks.
//!
//! Reads Claude Code hook JSON from stdin, dispatches to the appropriate
//! `HookHandler`, and exits with the correct code:
//! - Exit 0 = allow
//! - Exit 2 = block (Claude Code hook protocol)
//!
//! PreToolUse hooks: any internal error → exit 2 (fail-closed).

use std::io::Read as _;
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;

use domain::hook::{HookContext, HookName};
use infrastructure::shell::ConchShellParser;

/// CLI-layer serde type for Claude Code hook JSON envelope.
/// Security-critical fields (`tool_name`) must NOT use `#[serde(default)]` —
/// parse failure is caught at the CLI boundary.
/// For PreToolUse hooks this results in exit 2 (block, fail-closed).
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
    /// Content written by the Write tool (used by block-test-file-deletion guard).
    /// Deserialized with a custom helper that silently returns None if the JSON
    /// value is not a string (e.g. structured content blocks array).
    #[serde(default, deserialize_with = "deserialize_string_or_none")]
    content: Option<String>,
}

/// Deserializes `content` from hook JSON, handling both plain strings and
/// structured content blocks (`[{"type":"text","text":"..."},...]`).
/// Returns `None` only if the field is absent or contains no extractable text.
fn deserialize_string_or_none<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::Deserialize;
    let value: Option<serde_json::Value> = Option::<serde_json::Value>::deserialize(deserializer)?;
    Ok(value.and_then(|v| flatten_content_text(&v)))
}

/// Extracts text from a hook content value, recursing into nested structures.
///
/// Mirrors the behavior of Python `_shared.py`'s `flatten_text()`:
/// - Plain string: returned as-is.
/// - Array: recurse into each element, concatenate extracted text.
/// - Object with `"text"` string field: extract it.
/// - Object with `"content"` or `"message"` sub-values: recurse into them.
/// - Other types: returns `None`.
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
            // Prefer "text" field from {"type":"text","text":"..."} blocks
            if let Some(text) = obj.get("text").and_then(|v| v.as_str()) {
                if !text.is_empty() {
                    parts.push(text.to_owned());
                    return;
                }
            }
            // Recurse into all nested values to match Python _shared.py behavior.
            // String values under "message"/"content" keys are extracted directly;
            // objects and arrays are recursed into.
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

impl From<HookEnvelope> for domain::hook::HookInput {
    fn from(env: HookEnvelope) -> Self {
        Self {
            tool_name: env.tool_name,
            command: env.tool_input.command,
            file_path: env.tool_input.file_path,
            content: env.tool_input.content,
        }
    }
}

/// Hook names as CLI value enum (clap layer only — DIP).
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum CliHookName {
    /// Guard: block direct git operations.
    BlockDirectGitOps,
    /// Guard: block `rm` commands targeting test files (PreToolUse).
    BlockTestFileDeletion,
    /// Advisory: skill compliance check for UserPromptSubmit.
    SkillCompliance,
}

impl CliHookName {
    /// Converts to domain `HookName`.
    #[allow(dead_code)]
    fn to_domain(self) -> HookName {
        match self {
            Self::BlockDirectGitOps => HookName::BlockDirectGitOps,
            Self::BlockTestFileDeletion => HookName::BlockTestFileDeletion,
            Self::SkillCompliance => HookName::BlockDirectGitOps, // unused for advisory
        }
    }

    /// Returns `true` if this is a UserPromptSubmit hook (advisory, never blocks).
    fn is_user_prompt_submit(self) -> bool {
        matches!(self, Self::SkillCompliance)
    }

    /// Returns `true` if this is a PostToolUse hook (cannot block).
    fn is_post_tool_use(self) -> bool {
        false
    }
}

/// Hook subcommands.
#[derive(Debug, clap::Subcommand)]
pub enum HookCommand {
    /// Dispatch a security-critical hook via Rust logic.
    /// Reads Claude Code hook JSON from stdin.
    /// Exit 0 = allow, exit 2 = block (Claude Code hook protocol).
    /// PreToolUse hooks: any internal error → exit 2 (fail-closed).
    Dispatch {
        /// The hook to dispatch.
        #[arg(value_enum)]
        hook: CliHookName,
    },
}

/// Executes a hook subcommand.
pub fn execute(cmd: HookCommand) -> ExitCode {
    match cmd {
        HookCommand::Dispatch { hook } => execute_dispatch(hook),
    }
}

fn execute_dispatch(hook: CliHookName) -> ExitCode {
    // UserPromptSubmit hooks use a separate flow (advisory, not guard).
    if hook.is_user_prompt_submit() {
        return execute_user_prompt_submit(hook);
    }

    let is_post = hook.is_post_tool_use();

    // Read stdin JSON
    let mut stdin_buf = String::new();
    if let Err(e) = std::io::stdin().read_to_string(&mut stdin_buf) {
        return handle_error(is_post, &format!("failed to read stdin: {e}"));
    }

    // Empty stdin = hook infrastructure did not provide an envelope.
    // Fail-closed: block the command and report the error clearly.
    if stdin_buf.trim().is_empty() {
        return handle_error(is_post, "hook received empty stdin — no envelope to check");
    }

    // Parse HookEnvelope (serde) — security fields have no default
    let envelope: HookEnvelope = match serde_json::from_str(&stdin_buf) {
        Ok(env) => env,
        Err(e) => {
            return handle_error(is_post, &format!("failed to parse hook JSON: {e}"));
        }
    };

    // Build domain types
    let input: domain::hook::HookInput = envelope.into();

    let ctx =
        HookContext { project_dir: std::env::var("CLAUDE_PROJECT_DIR").ok().map(PathBuf::from) };

    // Composition root: build the shell parser adapter and inject into handlers
    let parser: Arc<dyn domain::guard::ShellParser> = Arc::new(ConchShellParser);

    // Dispatch to the appropriate handler
    let result = match hook {
        CliHookName::BlockDirectGitOps => {
            let handler = usecase::hook::GuardHookHandler { parser: Arc::clone(&parser) };
            handler_handle(&handler, &ctx, &input)
        }
        CliHookName::BlockTestFileDeletion => {
            let handler =
                usecase::hook::TestFileDeletionGuardHandler { parser: Arc::clone(&parser) };
            handler_handle(&handler, &ctx, &input)
        }
        CliHookName::SkillCompliance => return ExitCode::SUCCESS,
    };

    match result {
        Ok(verdict) => emit_verdict(&verdict),
        Err(e) => handle_error(is_post, &format!("hook error: {e}")),
    }
}

fn handler_handle(
    handler: &dyn usecase::hook::HookHandler,
    ctx: &HookContext,
    input: &domain::hook::HookInput,
) -> Result<domain::hook::HookVerdict, domain::hook::HookError> {
    handler.handle(ctx, input)
}

/// Emits the hook verdict to stdout and returns the appropriate exit code.
fn emit_verdict(verdict: &domain::hook::HookVerdict) -> ExitCode {
    // Guard: plain text reason + exit 2, or empty + exit 0
    if verdict.is_blocked() {
        if let Some(reason) = &verdict.reason {
            println!("{reason}");
        }
        exit_code(2)
    } else {
        ExitCode::SUCCESS
    }
}

/// Handles errors based on PreToolUse vs PostToolUse semantics.
fn handle_error(is_post_tool_use: bool, message: &str) -> ExitCode {
    if is_post_tool_use {
        // PostToolUse: warn + exit 0 (cannot block)
        eprintln!("warning: {message}");
        ExitCode::SUCCESS
    } else {
        // PreToolUse: exit 2 (fail-closed)
        eprintln!("error: {message}");
        exit_code(2)
    }
}

/// Returns an `ExitCode` for the given value.
fn exit_code(code: u8) -> ExitCode {
    ExitCode::from(code)
}

// ---------------------------------------------------------------------------
// UserPromptSubmit: skill compliance hook
// ---------------------------------------------------------------------------

/// Serde type for UserPromptSubmit hook JSON envelope.
#[derive(Debug, serde::Deserialize)]
struct PromptEnvelope {
    #[serde(default)]
    prompt: String,
}

/// Executes a UserPromptSubmit advisory hook.
/// Reads prompt from stdin JSON, checks skill compliance, and emits
/// `additionalContext` via stdout JSON. Always exits 0.
fn execute_user_prompt_submit(_hook: CliHookName) -> ExitCode {
    // Read stdin
    let mut stdin_buf = String::new();
    if std::io::stdin().read_to_string(&mut stdin_buf).is_err() {
        return ExitCode::SUCCESS; // advisory — never block
    }

    let prompt = match serde_json::from_str::<PromptEnvelope>(stdin_buf.trim()) {
        Ok(env) => env.prompt,
        Err(_) => return ExitCode::SUCCESS,
    };

    if prompt.is_empty() {
        return ExitCode::SUCCESS;
    }

    // Load guides and track context from project dir
    let guides = load_guides_from_project();
    let track_context = load_latest_track_context();

    // Run compliance check
    let ctx =
        domain::skill_compliance::check_compliance(&prompt, track_context.as_deref(), &guides, 3);

    if let Some(additional_context) = ctx.render() {
        let output = serde_json::json!({
            "hookSpecificOutput": {
                "hookEventName": "UserPromptSubmit",
                "additionalContext": additional_context,
            }
        });
        println!("{}", output);
    }

    ExitCode::SUCCESS
}

/// Loads guide entries from `knowledge/external/guides.json` relative to
/// `$CLAUDE_PROJECT_DIR`. Returns empty vec on any failure (advisory hook).
fn load_guides_from_project() -> Vec<domain::skill_compliance::GuideEntry> {
    let project_dir = match std::env::var("CLAUDE_PROJECT_DIR") {
        Ok(dir) => PathBuf::from(dir),
        Err(_) => return Vec::new(),
    };
    let guides_path = project_dir.join("knowledge/external/guides.json");
    infrastructure::guides_codec::load_guides(&guides_path).unwrap_or_default()
}

/// Loads context from the latest active track's spec.md and plan.md.
/// Selects the track with the most recent `updated_at` from metadata.json,
/// filtering to tracks that are not "done" or "archived".
/// Returns `None` on any failure (advisory hook — never block).
fn load_latest_track_context() -> Option<String> {
    let project_dir = PathBuf::from(std::env::var("CLAUDE_PROJECT_DIR").ok()?);
    let items_dir = project_dir.join("track/items");
    let entries = std::fs::read_dir(&items_dir).ok()?;

    // Find the latest active track by updated_at in metadata.json
    let latest = entries
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().ok().is_some_and(|ft| ft.is_dir()))
        .filter_map(|e| {
            let metadata_path = e.path().join("metadata.json");
            let content = std::fs::read_to_string(&metadata_path).ok()?;
            let json: serde_json::Value = serde_json::from_str(&content).ok()?;
            let status = json.get("status")?.as_str()?;
            // Skip done/archived tracks
            if status == "done" || status == "archived" {
                return None;
            }
            let updated_at = json.get("updated_at")?.as_str()?.to_owned();
            Some((e.path(), updated_at))
        })
        .max_by(|(_, a), (_, b)| a.cmp(b))?;

    let track_dir = latest.0;
    let mut parts = Vec::new();
    for filename in ["spec.md", "plan.md"] {
        if let Ok(content) = std::fs::read_to_string(track_dir.join(filename)) {
            parts.push(content);
        }
    }

    if parts.is_empty() { None } else { Some(parts.join("\n")) }
}
