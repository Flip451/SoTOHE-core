//! Input DTOs for the `review_v2` command family.

use std::path::PathBuf;

/// Input DTO for `review_run_codex`.
#[derive(Debug, Clone)]
pub struct ReviewRunCodexInput {
    pub model: String,
    pub timeout_seconds: u64,
    pub briefing_file: Option<PathBuf>,
    pub prompt: Option<String>,
    pub track_id: Option<String>,
    pub round_type: String,
    pub group: String,
    pub items_dir: PathBuf,
}

/// Input DTO for `review_run_claude`.
#[derive(Debug, Clone)]
pub struct ReviewRunClaudeInput {
    pub model: String,
    pub timeout_seconds: u64,
    pub briefing_file: Option<PathBuf>,
    pub prompt: Option<String>,
    pub track_id: Option<String>,
    pub round_type: String,
    pub group: String,
    pub items_dir: PathBuf,
}

/// Input DTO for `review_run_local`.
#[derive(Debug, Clone)]
pub struct ReviewRunLocalInput {
    pub model: Option<String>,
    pub timeout_seconds: u64,
    pub briefing_file: Option<PathBuf>,
    pub prompt: Option<String>,
    pub track_id: Option<String>,
    pub round_type: String,
    pub group: String,
    pub items_dir: PathBuf,
}

/// Input DTO for `review_run_fix_local` (`sotp review fix-local`).
///
/// Maps to the 7 CLI flags: `--scope` / `--briefing-file` / `--track-id` /
/// `--round-type` / `--reviewer-model` / `--model` / `--scope-files`.
/// Carries stdlib-typed fields only — no domain or infrastructure types (CN-02).
#[derive(Debug, Clone)]
pub struct RunReviewFixLocalInput {
    /// Scope name (e.g., `"cli"`, `"infrastructure"`).
    pub scope: String,
    /// Path to the briefing file passed to the fixer.
    pub briefing_file: Option<std::path::PathBuf>,
    /// Track ID. Required (no auto-resolve from branch for write operations).
    pub track_id: String,
    /// Round type: `"fast"` or `"final"`.
    pub round_type: String,
    /// Model for the nested reviewer subprocess.
    pub reviewer_model: String,
    /// Model for the fixer (Codex) subprocess.
    pub model: String,
    /// Files the fixer may modify (modification boundary).
    pub scope_files: Vec<std::path::PathBuf>,
}

/// Input DTO for `review_results`.
#[derive(Debug, Clone)]
pub struct ReviewResultsInput {
    pub track_id: Option<String>,
    pub items_dir: PathBuf,
    pub scope: Option<String>,
    pub all: bool,
    pub limit: u32,
    pub round_type: String,
    pub no_hint: bool,
}
