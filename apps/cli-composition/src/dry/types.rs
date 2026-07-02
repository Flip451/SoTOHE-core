//! Input DTOs for the `dry` command family, extracted to keep `dry.rs` under
//! the module-size hard limit.

use std::path::PathBuf;

/// Input DTO for `sotp dry write`.
#[derive(Debug, Clone)]
pub struct DryWriteInput {
    /// Track ID used to locate the per-track dry-check.json and .commit_hash.
    pub track_id: String,
    /// Optional explicit base commit (overrides FsDryCheckCommitHashStore lookup).
    pub base_commit: Option<String>,
    /// Path to the LanceDB semantic index database.
    pub db_path: PathBuf,
    /// Cosine similarity threshold (0.0–1.0) for the dry-check gate.
    pub threshold: Option<f32>,
    /// Root of the workspace to scan for Rust sources (corpus extraction).
    pub workspace_root: PathBuf,
    /// Path to the track items directory.
    pub items_dir: PathBuf,
    /// Codex model name for the DryCheckAgentPort.
    /// `None` means "use the model from `agent-profiles.json`".
    /// An explicit value overrides the profile model.
    pub model: Option<String>,
    /// Capability name forwarded to CodexDryChecker.
    pub capability_name: String,
}

/// Input DTO for `sotp dry results`.
#[derive(Debug, Clone)]
pub struct DryResultsInput {
    /// Track ID used to locate the per-track dry-check.json.
    pub track_id: String,
    /// Verdict filter: "all" / "not-a-violation" / "accepted" / "violation"
    /// (default "all"). Parsed to `VerdictFilter` inside cli-composition (CN-02).
    pub filter: String,
    /// Path to the track items directory.
    pub items_dir: PathBuf,
}

/// Input DTO for `sotp dry fix-local` (`dry_run_fix_local`).
///
/// Maps to the 2 required CLI flags plus the optional model override:
/// `--track-id` / `--briefing-file` / `--model`.
/// Carries stdlib-typed fields only — no domain or infrastructure types (CN-02).
#[derive(Debug, Clone)]
pub struct RunDryFixLocalInput {
    /// Track ID. Required (no auto-resolve from branch for write operations).
    pub track_id: String,
    /// Path to the briefing file passed to the dry-fix-lead fixer. Required.
    pub briefing_file: PathBuf,
    /// Model for the fixer (Codex) subprocess.
    /// `None` means "use the model from `agent-profiles.json`".
    /// An explicit value overrides the profile model.
    pub model: Option<String>,
}

/// Input DTO for `sotp dry check-approved`.
///
/// D5 / T005: `dry check-approved` is a pure-read staleness + all-resolved gate
/// (no embedding, no similarity search, no corpus / index / threshold), so the
/// old `db_path` / `threshold` / `workspace_root` fields are removed.
#[derive(Debug, Clone)]
pub struct DryCheckApprovedInput {
    /// Track ID used to locate the per-track dry-check.json and .commit_hash.
    pub track_id: String,
    /// Optional explicit base commit (overrides FsDryCheckCommitHashStore lookup).
    pub base_commit: Option<String>,
    /// Path to the track items directory.
    pub items_dir: PathBuf,
}
