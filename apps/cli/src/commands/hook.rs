//! Hook dispatch subcommand for security-critical hooks.
//!
//! Reads Claude Code hook JSON from stdin, dispatches to the appropriate
//! `HookHandler`, and exits with the correct code:
//! - Exit 0 = allow
//! - Exit 2 = block (Claude Code hook protocol)
//!
//! PreToolUse hooks (guard, lock-acquire): any internal error → exit 2 (fail-closed).
//! PostToolUse hooks (lock-release): any error → stderr warning + exit 0 (cannot block).

use std::io::Read as _;
use std::path::PathBuf;
use std::process::ExitCode;

use domain::Decision;
use domain::hook::{HookContext, HookName};
use domain::lock::AgentId;

/// CLI-layer serde type for Claude Code hook JSON envelope.
/// Security-critical fields (`tool_name`) must NOT use `#[serde(default)]` —
/// parse failure is caught at the CLI boundary.
/// For PreToolUse hooks this results in exit 2 (block, fail-closed).
/// For PostToolUse hooks (lock-release) it results in stderr warning + exit 0.
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
}

impl From<HookEnvelope> for domain::hook::HookInput {
    fn from(env: HookEnvelope) -> Self {
        Self {
            tool_name: env.tool_name,
            command: env.tool_input.command,
            file_path: env.tool_input.file_path,
        }
    }
}

/// Hook names as CLI value enum (clap layer only — DIP).
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum CliHookName {
    /// Guard: block direct git operations.
    BlockDirectGitOps,
    /// Lock: acquire file lock (PreToolUse).
    FileLockAcquire,
    /// Lock: release file lock (PostToolUse).
    FileLockRelease,
}

impl CliHookName {
    /// Converts to domain `HookName`.
    #[allow(dead_code)]
    fn to_domain(self) -> HookName {
        match self {
            Self::BlockDirectGitOps => HookName::BlockDirectGitOps,
            Self::FileLockAcquire => HookName::FileLockAcquire,
            Self::FileLockRelease => HookName::FileLockRelease,
        }
    }

    /// Returns `true` if this is a PostToolUse hook (cannot block).
    fn is_post_tool_use(self) -> bool {
        matches!(self, Self::FileLockRelease)
    }
}

/// Hook subcommands.
#[derive(Debug, clap::Subcommand)]
pub enum HookCommand {
    /// Dispatch a security-critical hook via Rust logic.
    /// Reads Claude Code hook JSON from stdin.
    /// Exit 0 = allow, exit 2 = block (Claude Code hook protocol).
    /// PreToolUse hooks: any internal error → exit 2 (fail-closed).
    /// PostToolUse hooks (lock-release): any error → stderr warning + exit 0 (cannot block).
    Dispatch {
        /// The hook to dispatch.
        #[arg(value_enum)]
        hook: CliHookName,

        /// Locks directory (for file-lock hooks).
        /// Default: `$CLAUDE_PROJECT_DIR/.locks` (project-root-anchored).
        /// Also read from `$SOTP_LOCKS_DIR`.
        /// If neither `--locks-dir` nor `$SOTP_LOCKS_DIR` nor `$CLAUDE_PROJECT_DIR`
        /// is set → exit 2 (fail-closed). No cwd fallback — prevents split-registry.
        #[arg(long, env = "SOTP_LOCKS_DIR")]
        locks_dir: Option<PathBuf>,

        /// Agent ID (for file-lock hooks). Required for lock hooks.
        /// MUST be passed explicitly by the hook command, because the `sotp`
        /// process cannot infer Claude Code's parent pid reliably.
        #[arg(long, env = "SOTP_AGENT_ID")]
        agent: Option<String>,

        /// Process ID of the lock holder (required for lock-acquire only).
        /// Not needed for lock-release (release API uses path + agent only).
        /// Not needed for guard hooks (block-direct-git-ops).
        /// CLI-arg-only (no env var — must be explicitly passed by hook command).
        #[arg(long)]
        pid: Option<u32>,
    },
}

/// Executes a hook subcommand.
pub fn execute(cmd: HookCommand) -> ExitCode {
    match cmd {
        HookCommand::Dispatch { hook, locks_dir, agent, pid } => {
            execute_dispatch(hook, locks_dir, agent, pid)
        }
    }
}

fn execute_dispatch(
    hook: CliHookName,
    locks_dir: Option<PathBuf>,
    agent: Option<String>,
    pid: Option<u32>,
) -> ExitCode {
    let is_post = hook.is_post_tool_use();

    // Read stdin JSON
    let mut stdin_buf = String::new();
    if let Err(e) = std::io::stdin().read_to_string(&mut stdin_buf) {
        return handle_error(is_post, &format!("failed to read stdin: {e}"));
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

    // Resolve locks_dir with CLAUDE_PROJECT_DIR fallback
    let resolved_locks_dir = resolve_locks_dir(locks_dir);

    let ctx = HookContext {
        project_dir: std::env::var("CLAUDE_PROJECT_DIR").ok().map(PathBuf::from),
        locks_dir: resolved_locks_dir,
        agent: agent.map(AgentId::new),
        pid,
    };

    // Dispatch to the appropriate handler
    let result = match hook {
        CliHookName::BlockDirectGitOps => {
            let handler = usecase::hook::GuardHookHandler;
            handler_handle(&handler, &ctx, &input)
        }
        CliHookName::FileLockAcquire | CliHookName::FileLockRelease => {
            // Lock hooks require locks_dir to construct FsFileLockManager.
            let Some(ref dir) = ctx.locks_dir else {
                return handle_error(is_post, "locks_dir is required for lock hooks but not set");
            };
            let lock_manager = match infrastructure::lock::FsFileLockManager::new(dir) {
                Ok(lm) => std::sync::Arc::new(lm),
                Err(e) => {
                    return handle_error(is_post, &format!("failed to init lock manager: {e}"));
                }
            };
            match hook {
                CliHookName::FileLockAcquire => {
                    let handler = usecase::hook::LockAcquireHookHandler::new(lock_manager);
                    handler_handle(&handler, &ctx, &input)
                }
                CliHookName::FileLockRelease => {
                    let handler = usecase::hook::LockReleaseHookHandler::new(lock_manager);
                    handler_handle(&handler, &ctx, &input)
                }
                CliHookName::BlockDirectGitOps => {
                    // unreachable — handled by outer match arm
                    return handle_error(is_post, "internal dispatch error");
                }
            }
        }
    };

    match result {
        Ok(verdict) => emit_verdict(hook, &verdict),
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

/// Resolves locks directory: explicit > $SOTP_LOCKS_DIR (handled by clap env) > $CLAUDE_PROJECT_DIR/.locks
fn resolve_locks_dir(explicit: Option<PathBuf>) -> Option<PathBuf> {
    if explicit.is_some() {
        return explicit;
    }
    // Fallback to $CLAUDE_PROJECT_DIR/.locks
    std::env::var("CLAUDE_PROJECT_DIR").ok().map(|dir| PathBuf::from(dir).join(".locks"))
}

/// Emits the hook verdict to stdout and returns the appropriate exit code.
fn emit_verdict(hook: CliHookName, verdict: &domain::hook::HookVerdict) -> ExitCode {
    match hook {
        CliHookName::BlockDirectGitOps => {
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
        CliHookName::FileLockAcquire => {
            // Lock-acquire: block JSON + exit 2, or allow + exit 0
            match verdict.decision {
                Decision::Block => {
                    let output = serde_json::json!({
                        "hookSpecificOutput": {
                            "decision": "block",
                            "reason": verdict.reason.as_deref().unwrap_or(""),
                        }
                    });
                    println!("{output}");
                    exit_code(2)
                }
                Decision::Allow => ExitCode::SUCCESS,
            }
        }
        CliHookName::FileLockRelease => {
            // Lock-release: always exit 0 (PostToolUse cannot block)
            ExitCode::SUCCESS
        }
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
