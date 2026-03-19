//! Hook domain types — framework-free, NO serde/serde_json dependency.

use std::path::PathBuf;

/// Names of security-critical hooks dispatched via Rust.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookName {
    /// Guard hook: blocks direct git operations.
    BlockDirectGitOps,
    /// Guard hook: blocks `rm` commands targeting test files.
    BlockTestFileDeletion,
}

/// Context for hook execution. Built by the CLI layer from:
/// - `project_dir`: `$CLAUDE_PROJECT_DIR` env var (set by Claude Code)
#[derive(Debug, Clone)]
pub struct HookContext {
    /// Project directory from `$CLAUDE_PROJECT_DIR`.
    pub project_dir: Option<PathBuf>,
}

/// Framework-free hook input extracted from Claude Code hook JSON.
/// Parsing from HookEnvelope (serde) happens in the CLI/infrastructure layer (DIP).
#[derive(Debug, Clone)]
pub struct HookInput {
    /// The name of the tool being invoked (always present — required in HookEnvelope serde).
    pub tool_name: String,
    /// The shell command (for guard hook: `block-direct-git-ops`).
    pub command: Option<String>,
    /// The file path (used by the Write tool for test-file deletion guard).
    pub file_path: Option<PathBuf>,
    /// The file content (for Write tool: used by test-file deletion guard to detect
    /// empty-content writes, which are equivalent to file truncation/deletion).
    pub content: Option<String>,
}
