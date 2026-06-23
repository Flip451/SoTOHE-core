//! `semantic_dup` command family — primary adapter driver.
//!
//! `SemanticDupDriver` holds an injected
//! [`usecase::semantic_dup_driver::SemanticDupDriverService`] and exposes
//! `handle(input) -> CommandOutcome`.

use std::path::PathBuf;
use std::sync::Arc;

use usecase::semantic_dup_driver::{
    DupCheckDriverInput, FindSimilarDriverInput, IndexBuildDriverInput,
    IndexMeasureQualityDriverInput, SemanticDupDriverOutcome, SemanticDupDriverService,
};

use crate::render::CommandOutcome;

// ---------------------------------------------------------------------------
// Input type
// ---------------------------------------------------------------------------

/// Typed input for the `semantic_dup` command family.
pub enum SemanticDupInput {
    /// Run the semantic similarity search for a given fragment.
    FindSimilar {
        /// Inline fragment text to search for.
        fragment_text: String,
        /// Number of top-k similar fragments to return.
        top_k: usize,
        /// Path to the local LanceDB semantic index database.
        db_path: PathBuf,
    },
    /// Build (or rebuild) the semantic index from workspace Rust sources.
    IndexBuild {
        /// Workspace root to scan for `*.rs` source files.
        workspace_root: PathBuf,
        /// Path to the local LanceDB semantic index database.
        db_path: PathBuf,
    },
    /// Measure embedding quality metrics over workspace fragments (JSON output).
    IndexMeasureQuality {
        /// Root of the workspace to scan for Rust sources.
        workspace_root: PathBuf,
    },
    /// Soft gate: check fragments for near-duplicates in the semantic index.
    DupCheck {
        /// List of fragment file paths to check.
        fragment_files: Vec<PathBuf>,
        /// Cosine similarity threshold (0.0–1.0) above which a match is flagged.
        threshold: f32,
        /// Path to the local LanceDB semantic index database.
        db_path: PathBuf,
        /// Optional path to the acknowledgement file (newline-separated hash list).
        ack_file: Option<PathBuf>,
        /// Whether to acknowledge all warnings from this run.
        ack: bool,
    },
}

// ---------------------------------------------------------------------------
// Driver
// ---------------------------------------------------------------------------

/// Primary adapter driver for the `semantic_dup` command family.
///
/// Holds an injected [`SemanticDupDriverService`]; exposes `handle(input) -> CommandOutcome`.
pub struct SemanticDupDriver {
    service: Arc<dyn SemanticDupDriverService>,
}

impl SemanticDupDriver {
    /// Create a new `SemanticDupDriver` with the given service.
    pub fn new(service: Arc<dyn SemanticDupDriverService>) -> Self {
        Self { service }
    }

    /// Handle a semantic_dup command.
    pub fn handle(&self, input: SemanticDupInput) -> CommandOutcome {
        match input {
            SemanticDupInput::FindSimilar { fragment_text, top_k, db_path } => {
                self.find_similar(fragment_text, top_k, db_path)
            }
            SemanticDupInput::IndexBuild { workspace_root, db_path } => {
                self.index_build(workspace_root, db_path)
            }
            SemanticDupInput::IndexMeasureQuality { workspace_root } => {
                self.index_measure_quality(workspace_root)
            }
            SemanticDupInput::DupCheck { fragment_files, threshold, db_path, ack_file, ack } => {
                self.dup_check(fragment_files, threshold, db_path, ack_file, ack)
            }
        }
    }

    // -----------------------------------------------------------------------
    // Private helpers — translate input fields → service calls
    // -----------------------------------------------------------------------

    fn find_similar(
        &self,
        fragment_text: String,
        top_k: usize,
        db_path: PathBuf,
    ) -> CommandOutcome {
        let outcome =
            self.service.find_similar(FindSimilarDriverInput { fragment_text, top_k, db_path });
        into_command_outcome(outcome)
    }

    fn index_build(&self, workspace_root: PathBuf, db_path: PathBuf) -> CommandOutcome {
        let outcome = self.service.index_build(IndexBuildDriverInput { workspace_root, db_path });
        into_command_outcome(outcome)
    }

    fn index_measure_quality(&self, workspace_root: PathBuf) -> CommandOutcome {
        let outcome =
            self.service.index_measure_quality(IndexMeasureQualityDriverInput { workspace_root });
        into_command_outcome(outcome)
    }

    fn dup_check(
        &self,
        fragment_files: Vec<PathBuf>,
        threshold: f32,
        db_path: PathBuf,
        ack_file: Option<PathBuf>,
        ack: bool,
    ) -> CommandOutcome {
        let outcome = self.service.dup_check(DupCheckDriverInput {
            fragment_files,
            threshold,
            db_path,
            ack_file,
            ack,
        });
        into_command_outcome(outcome)
    }
}

// ---------------------------------------------------------------------------
// Conversion helper
// ---------------------------------------------------------------------------

/// Convert a `SemanticDupDriverOutcome` (usecase boundary type) into a `CommandOutcome`.
fn into_command_outcome(outcome: SemanticDupDriverOutcome) -> CommandOutcome {
    CommandOutcome { stdout: outcome.stdout, stderr: outcome.stderr, exit_code: outcome.exit_code }
}
