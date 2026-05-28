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
