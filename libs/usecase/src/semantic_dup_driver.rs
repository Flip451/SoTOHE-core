//! Driver-level service port for the `semantic_dup` command family.
//!
//! Defines a single `SemanticDupDriverService` trait that the
//! `cli_driver::semantic_dup::SemanticDupDriver` invokes, plus a pass-through
//! `SemanticDupDriverInteractor` that delegates to an injected
//! `SemanticDupDriverPort`.
//!
//! The adapter implementing `SemanticDupDriverPort` lives in `cli_composition`
//! and delegates to `SemanticDupCompositionRoot` methods.

use std::path::PathBuf;
use std::sync::Arc;

// ── Input types ───────────────────────────────────────────────────────────────

/// Input for `sotp find-similar` (driver boundary).
#[derive(Debug, Clone)]
pub struct FindSimilarDriverInput {
    /// Inline fragment text to search for.  Mutually exclusive with `file_path`.
    pub fragment_text: Option<String>,
    /// Path to a file whose content is used as the query fragment.  Mutually exclusive
    /// with `fragment_text`.  The file is read by the composition layer.
    pub file_path: Option<PathBuf>,
    pub top_k: usize,
    pub db_path: PathBuf,
}

/// Input for `sotp dup-index build` (driver boundary).
#[derive(Debug, Clone)]
pub struct IndexBuildDriverInput {
    pub workspace_root: PathBuf,
    pub db_path: PathBuf,
}

/// Input for `sotp dup-index measure-quality` (driver boundary).
#[derive(Debug, Clone)]
pub struct IndexMeasureQualityDriverInput {
    pub workspace_root: PathBuf,
}

/// Input for `sotp dup-check` (driver boundary).
#[derive(Debug, Clone)]
pub struct DupCheckDriverInput {
    /// Path to a newline-separated file listing fragment file paths to check.
    /// The file is read by the composition layer.
    pub files_from: PathBuf,
    pub threshold: f32,
    pub db_path: PathBuf,
    pub ack_file: Option<PathBuf>,
    pub ack: bool,
}

// ── Output type ───────────────────────────────────────────────────────────────

/// Unified command outcome returned to the driver.
///
/// Mirrors `cli_driver::render::CommandOutcome`; defined here as a plain struct
/// so the usecase layer carries no dependency on `cli_driver`.
#[derive(Debug, Clone)]
pub struct SemanticDupDriverOutcome {
    pub stdout: Option<String>,
    pub stderr: Option<String>,
    pub exit_code: u8,
}

impl SemanticDupDriverOutcome {
    /// Convenience constructor: success with optional stdout text.
    pub fn success(stdout: Option<String>) -> Self {
        Self { stdout, stderr: None, exit_code: 0 }
    }

    /// Convenience constructor: failure with optional stderr text.
    pub fn failure(msg: Option<String>) -> Self {
        Self { stdout: None, stderr: msg, exit_code: 1 }
    }
}

// ── Port ──────────────────────────────────────────────────────────────────────

/// Secondary port for the `semantic_dup` command family.
///
/// Implemented by an adapter in `cli_composition` that delegates to
/// `SemanticDupCompositionRoot` methods.
pub trait SemanticDupDriverPort: Send + Sync {
    /// Run `sotp find-similar`.
    fn find_similar(&self, input: FindSimilarDriverInput) -> SemanticDupDriverOutcome;

    /// Run `sotp dup-index build`.
    fn index_build(&self, input: IndexBuildDriverInput) -> SemanticDupDriverOutcome;

    /// Run `sotp dup-index measure-quality`.
    fn index_measure_quality(
        &self,
        input: IndexMeasureQualityDriverInput,
    ) -> SemanticDupDriverOutcome;

    /// Run `sotp dup-check`.
    fn dup_check(&self, input: DupCheckDriverInput) -> SemanticDupDriverOutcome;
}

// ── Service ───────────────────────────────────────────────────────────────────

/// Application service trait for the `semantic_dup` command family.
pub trait SemanticDupDriverService: Send + Sync {
    /// Run `sotp find-similar`.
    fn find_similar(&self, input: FindSimilarDriverInput) -> SemanticDupDriverOutcome;

    /// Run `sotp dup-index build`.
    fn index_build(&self, input: IndexBuildDriverInput) -> SemanticDupDriverOutcome;

    /// Run `sotp dup-index measure-quality`.
    fn index_measure_quality(
        &self,
        input: IndexMeasureQualityDriverInput,
    ) -> SemanticDupDriverOutcome;

    /// Run `sotp dup-check`.
    fn dup_check(&self, input: DupCheckDriverInput) -> SemanticDupDriverOutcome;
}

// ── Interactor ────────────────────────────────────────────────────────────────

/// Interactor implementing [`SemanticDupDriverService`] by delegating to the port.
pub struct SemanticDupDriverInteractor {
    port: Arc<dyn SemanticDupDriverPort>,
}

impl SemanticDupDriverInteractor {
    /// Create a new interactor bound to the given port.
    #[must_use]
    pub fn new(port: Arc<dyn SemanticDupDriverPort>) -> Self {
        Self { port }
    }
}

impl SemanticDupDriverService for SemanticDupDriverInteractor {
    fn find_similar(&self, input: FindSimilarDriverInput) -> SemanticDupDriverOutcome {
        self.port.find_similar(input)
    }

    fn index_build(&self, input: IndexBuildDriverInput) -> SemanticDupDriverOutcome {
        self.port.index_build(input)
    }

    fn index_measure_quality(
        &self,
        input: IndexMeasureQualityDriverInput,
    ) -> SemanticDupDriverOutcome {
        self.port.index_measure_quality(input)
    }

    fn dup_check(&self, input: DupCheckDriverInput) -> SemanticDupDriverOutcome {
        self.port.dup_check(input)
    }
}
