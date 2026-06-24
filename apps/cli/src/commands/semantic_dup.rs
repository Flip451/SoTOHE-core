//! `sotp find-similar`, `sotp dup-check`, and `sotp dup-index` subcommands.
//!
//! Each subcommand delegates argument parsing to clap, constructs the
//! corresponding `cli_composition` input DTO, and calls the matching `CliApp`
//! method.  All composition (adapter construction, interactor wiring) is
//! performed inside `cli_composition::CliApp`, following the existing pattern.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Args, Subcommand};
use cli_composition::SemanticDupCompositionRoot;
use cli_driver::semantic_dup::SemanticDupInput;

use crate::commands::driver_outcome_to_exit;

// ── sotp find-similar ─────────────────────────────────────────────────────────

/// Arguments for `sotp find-similar`.
#[derive(Debug, Args)]
pub struct FindSimilarArgs {
    /// Inline fragment text to search for.  Mutually exclusive with `--file`.
    #[arg(conflicts_with = "file", required_unless_present = "file")]
    pub fragment: Option<String>,

    /// Path to a file whose content is used as the query fragment.
    #[arg(long, conflicts_with = "fragment", required_unless_present = "fragment")]
    pub file: Option<PathBuf>,

    /// Number of top-k similar fragments to return (default: 5).
    #[arg(long, default_value_t = 5)]
    pub top_k: usize,

    /// Path to the local LanceDB semantic index database.
    #[arg(long, default_value = ".semantic_index")]
    pub db_path: PathBuf,
}

/// Execute `sotp find-similar`.
///
/// CN-05: information-only, never blocks (always exits 0).
pub fn execute_find_similar(args: FindSimilarArgs) -> ExitCode {
    driver_outcome_to_exit(SemanticDupCompositionRoot::new().semantic_dup_driver().handle(
        SemanticDupInput::FindSimilar {
            fragment_text: args.fragment,
            file_path: args.file,
            top_k: args.top_k,
            db_path: args.db_path,
        },
    ))
}

// ── sotp dup-index ────────────────────────────────────────────────────────────

/// Subcommands for `sotp dup-index`.
#[derive(Debug, Subcommand)]
pub enum DupIndexCommand {
    /// Build (or rebuild) the semantic index from workspace Rust sources.
    Build(DupIndexBuildArgs),
    /// Measure embedding quality metrics over workspace fragments (JSON output).
    MeasureQuality(DupIndexMeasureQualityArgs),
}

/// Arguments for `sotp dup-index build`.
#[derive(Debug, Args)]
pub struct DupIndexBuildArgs {
    /// Workspace root to scan for `*.rs` source files.
    #[arg(long, default_value = ".")]
    pub workspace_root: PathBuf,

    /// Path to the local LanceDB semantic index database.
    #[arg(long, default_value = ".semantic_index")]
    pub db_path: PathBuf,
}

/// Arguments for `sotp dup-index measure-quality`.
#[derive(Debug, Args)]
pub struct DupIndexMeasureQualityArgs {
    /// Workspace root to scan for `*.rs` source files.
    #[arg(long, default_value = ".")]
    pub workspace_root: PathBuf,
}

/// Execute `sotp dup-index <subcommand>`.
pub fn execute_dup_index(cmd: DupIndexCommand) -> ExitCode {
    let driver = SemanticDupCompositionRoot::new().semantic_dup_driver();
    match cmd {
        DupIndexCommand::Build(args) => {
            driver_outcome_to_exit(driver.handle(SemanticDupInput::IndexBuild {
                workspace_root: args.workspace_root,
                db_path: args.db_path,
            }))
        }
        DupIndexCommand::MeasureQuality(args) => {
            driver_outcome_to_exit(driver.handle(SemanticDupInput::IndexMeasureQuality {
                workspace_root: args.workspace_root,
            }))
        }
    }
}

// ── sotp dup-check ────────────────────────────────────────────────────────────

/// Arguments for `sotp dup-check`.
#[derive(Debug, Args)]
pub struct DupCheckArgs {
    /// Path to a newline-separated file listing fragment file paths to check.
    /// Each line must be a path to a file whose content is a single code fragment.
    #[arg(long)]
    pub files_from: PathBuf,

    /// Cosine similarity threshold (0.0–1.0) above which a match is flagged
    /// (default: 0.8).
    #[arg(long, default_value_t = 0.8_f32)]
    pub threshold: f32,

    /// Path to the local LanceDB semantic index database.
    #[arg(long, default_value = ".semantic_index")]
    pub db_path: PathBuf,

    /// Path to the acknowledgement file (newline-separated hash list).
    /// When provided, already-acked fragments are suppressed (AC-05).
    #[arg(long)]
    pub ack_file: Option<PathBuf>,

    /// Acknowledge all warnings from this run, writing their hashes to
    /// `--ack-file`.  Requires `--ack-file` to be set (AC-05).
    #[arg(long, requires = "ack_file")]
    pub ack: bool,
}

/// Execute `sotp dup-check`.
///
/// CN-02/AC-04: soft gate — warnings go to stderr, always exits 0.
/// AC-05: fragments whose hash appears in `--ack-file` are suppressed.
pub fn execute_dup_check(args: DupCheckArgs) -> ExitCode {
    driver_outcome_to_exit(SemanticDupCompositionRoot::new().semantic_dup_driver().handle(
        SemanticDupInput::DupCheck {
            files_from: args.files_from,
            threshold: args.threshold,
            db_path: args.db_path,
            ack_file: args.ack_file,
            ack: args.ack,
        },
    ))
}
