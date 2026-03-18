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
    /// Guard hook: blocks `rm` commands targeting test files.
    BlockTestFileDeletion,
}

/// Context for hook execution. Built by the CLI layer from:
/// - `project_dir`: `$CLAUDE_PROJECT_DIR` env var (set by Claude Code)
/// - `locks_dir`: `$SOTP_LOCKS_DIR` env var or `--locks-dir` CLI arg
///   (default: `$CLAUDE_PROJECT_DIR/.locks` — must be project-root-anchored)
///   If neither is set → exit 2 (fail-closed, prevents split-registry)
/// - `agent`: `$SOTP_AGENT_ID` env var or `--agent` CLI arg — passed by the
///   shell hook command in `.claude/settings.json`.
/// - `pid`: `--pid` CLI arg — passed by the shell hook command (lock-acquire only).
///
/// ## PID / Agent Propagation for Lock Hooks
///
/// Lock hooks are invoked directly via shell commands in `.claude/settings.json`.
/// The shell hook command passes `--pid "$PPID"` (Claude Code's PID) and
/// `--agent "$SOTP_AGENT_ID"` explicitly.
///
/// - lock-acquire: `--agent` + `--pid` required
/// - lock-release: `--agent` required, `--pid` optional
/// - guard (block-direct-git-ops): pid/agent irrelevant, can be omitted
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
    /// The file path (for lock hooks: `file-lock-acquire`, `file-lock-release`;
    /// also used by the Write tool for test-file deletion guard).
    pub file_path: Option<PathBuf>,
    /// The file content (for Write tool: used by test-file deletion guard to detect
    /// empty-content writes, which are equivalent to file truncation/deletion).
    pub content: Option<String>,
}
