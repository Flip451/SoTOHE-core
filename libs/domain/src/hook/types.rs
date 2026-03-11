//! Hook domain types — framework-free, NO serde/serde_json dependency.

use std::path::PathBuf;

/// Names of security-critical hooks dispatched via Rust.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookName {
    /// Guard hook: blocks direct git operations.
    BlockDirectGitOps,
    /// Lock hook: acquires file lock before tool use.
    FileLockAcquire,
    /// Lock hook: releases file lock after tool use.
    FileLockRelease,
}

/// Context for hook execution. Built by the CLI layer from:
/// - `project_dir`: `$CLAUDE_PROJECT_DIR` env var (set by Claude Code)
/// - `locks_dir`: `$SOTP_LOCKS_DIR` env var or `--locks-dir` CLI arg
///   (default: `$CLAUDE_PROJECT_DIR/.locks` — must be project-root-anchored)
///   If neither is set → exit 2 (fail-closed, prevents split-registry)
/// - `agent`: `$SOTP_AGENT_ID` env var or `--agent` CLI arg — NO SAFE DEFAULT in sotp
///   (same reason as pid: sotp's ppid is the Python launcher, not Claude Code).
///   Python launcher MUST pass `--agent` explicitly (e.g., `f"pid-{os.getppid()}"`).
/// - `pid`: `--pid` CLI arg — NO SAFE DEFAULT in sotp.
///
/// ## PID / Agent Propagation for Lock Hooks
///
/// The Python launcher for lock hooks runs: Python → sotp (subprocess).
/// If sotp uses `getppid()`, it gets the Python launcher PID (short-lived),
/// not Claude Code's PID. This makes the lock immediately stale-reapable.
///
/// Therefore, lock-acquire launchers MUST compute pid/agent in Python and
/// pass them explicitly via `--pid` and `--agent` CLI args, exactly as
/// the current `file-lock-acquire.py` does:
///   pid = os.getppid()      # Claude Code PID (Python's parent)
///   agent = f"pid-{pid}"    # or $SOTP_AGENT_ID
///
/// Lock-release launchers MUST pass `--agent` but `--pid` is optional
/// (`FileLockManager::release` takes only `path` + `agent`).
///
/// For `block-direct-git-ops` (guard hook), pid/agent are irrelevant
/// and can be omitted.
///
/// Python launchers pass `--locks-dir` and `--agent` via CLI args or env vars.
/// `--pid` is CLI-arg-only (no env var — must be explicitly passed by launcher).
///
/// All fields are `Option` because different hooks need different subsets.
/// The CLI layer validates per-hook requirements:
/// - guard (block-direct-git-ops): only `project_dir` needed (for future use);
///   if `$CLAUDE_PROJECT_DIR` is unset, guard still works (it only inspects the command)
/// - lock-acquire: `project_dir` (for locks_dir default) + `locks_dir` + `agent` + `pid`
///   required — missing any → exit 2
/// - lock-release: `locks_dir` + `agent` required — `pid` NOT needed
///   (`FileLockManager::release` takes only `path` + `agent`, not `pid`)
#[derive(Debug, Clone)]
pub struct HookContext {
    /// Project directory from `$CLAUDE_PROJECT_DIR`.
    pub project_dir: Option<PathBuf>,
    /// Lock registry directory.
    pub locks_dir: Option<PathBuf>,
    /// Agent identifier for lock operations.
    pub agent: Option<crate::lock::AgentId>,
    /// Process ID of the lock holder (required for lock-acquire only).
    /// Not needed for lock-release (release API uses path + agent only).
    /// Not needed for guard hooks (block-direct-git-ops).
    /// CLI-arg-only (no env var — must be explicitly passed by launcher).
    pub pid: Option<u32>,
}

/// Framework-free hook input extracted from Claude Code hook JSON.
/// Parsing from HookEnvelope (serde) happens in the CLI/infrastructure layer (DIP).
#[derive(Debug, Clone)]
pub struct HookInput {
    /// The name of the tool being invoked (always present — required in HookEnvelope serde).
    pub tool_name: String,
    /// The shell command (for guard hook: `block-direct-git-ops`).
    pub command: Option<String>,
    /// The file path (for lock hooks: `file-lock-acquire`, `file-lock-release`).
    pub file_path: Option<PathBuf>,
}
