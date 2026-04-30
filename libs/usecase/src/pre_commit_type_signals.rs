//! Pre-commit type-signal recomputation application service (usecase layer).
//!
//! Wraps the `ConfidenceSignal` classification logic so that CLI
//! `commands/make.rs` never imports `domain::ConfidenceSignal` directly
//! (CN-01 / D1). Returns [`PreCommitTypeSignalsOutput`] containing the signal
//! verdict and optional layer bindings for downstream catalogue-spec steps.

use std::path::PathBuf;
use std::sync::Arc;

use thiserror::Error;

// ── PreCommitTypeSignalsOutput ────────────────────────────────────────────────

/// DTO returned by [`PreCommitTypeSignalsService::run`].
///
/// Contains the overall verdict (pass/blocked), list of red signal names, list
/// of yellow signal names, and whether the track is frozen (Done/Archived).
///
/// The `frozen` flag lets the caller skip downstream catalogue-spec steps
/// without re-reading domain state. CLI uses this DTO to produce actionable
/// pre-commit messages without importing `domain::ConfidenceSignal`.
pub struct PreCommitTypeSignalsOutput {
    pub blocked: bool,
    pub frozen: bool,
    pub red_signals: Vec<String>,
    pub yellow_signals: Vec<String>,
}

// ── PreCommitTypeSignalsError ─────────────────────────────────────────────────

/// Error type for [`PreCommitTypeSignalsService`].
///
/// Wraps git discover failures, architecture-rules.json parse errors, symlink
/// guard rejections, and track metadata load failures without leaking domain
/// error types across the usecase boundary.
#[derive(Debug, Error)]
pub enum PreCommitTypeSignalsError {
    #[error("git discover failed: {0}")]
    GitDiscoverFailed(String),
    #[error("rules file missing: {0}")]
    RulesFileMissing(String),
    #[error("rules parse error: {0}")]
    RulesParseError(String),
    #[error("symlink rejected: {0}")]
    SymlinkRejected(String),
    #[error("metadata load failed: {0}")]
    MetadataLoadFailed(String),
    #[error("impl-plan load failed: {0}")]
    ImplPlanLoadFailed(String),
    #[error("type signals recompute failed: {0}")]
    TypeSignalsRecomputeFailed(String),
}

// ── PreCommitTypeSignalsService ───────────────────────────────────────────────

/// Application service trait for the pre-commit type-signal recomputation use
/// case (`sotp make track-commit-message` pre-commit step).
///
/// Driven by the CLI layer. Wraps the `ConfidenceSignal` classification logic
/// so that `commands/make.rs` never imports `domain::ConfidenceSignal` directly.
/// Returns [`PreCommitTypeSignalsOutput`] containing the signal verdict and
/// optional layer bindings for downstream catalogue-spec steps.
pub trait PreCommitTypeSignalsService: Send + Sync {
    /// Runs the pre-commit type signal recomputation for the given track.
    ///
    /// # Errors
    ///
    /// Returns [`PreCommitTypeSignalsError`] on git, rules, metadata, or signal
    /// recompute failures.
    fn run(
        &self,
        track_id: String,
        workspace_root: PathBuf,
    ) -> Result<PreCommitTypeSignalsOutput, PreCommitTypeSignalsError>;
}

// ── PreCommitTypeSignalsInteractor ────────────────────────────────────────────

/// Concrete struct implementing [`PreCommitTypeSignalsService`].
///
/// Constructs domain types internally and converts results to
/// [`PreCommitTypeSignalsOutput`] before returning to the CLI.
pub struct PreCommitTypeSignalsInteractor {
    run_fn: Arc<
        dyn Fn(String, PathBuf) -> Result<PreCommitTypeSignalsOutput, PreCommitTypeSignalsError>
            + Send
            + Sync,
    >,
}

impl PreCommitTypeSignalsInteractor {
    /// Creates a new interactor with the given runner function.
    #[must_use]
    pub fn new(
        run_fn: Arc<
            dyn Fn(String, PathBuf) -> Result<PreCommitTypeSignalsOutput, PreCommitTypeSignalsError>
                + Send
                + Sync,
        >,
    ) -> Self {
        Self { run_fn }
    }
}

impl PreCommitTypeSignalsService for PreCommitTypeSignalsInteractor {
    fn run(
        &self,
        track_id: String,
        workspace_root: PathBuf,
    ) -> Result<PreCommitTypeSignalsOutput, PreCommitTypeSignalsError> {
        (self.run_fn)(track_id, workspace_root)
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_pre_commit_type_signals_error_variants_exist() {
        let e1 = PreCommitTypeSignalsError::GitDiscoverFailed("git".to_owned());
        assert!(matches!(e1, PreCommitTypeSignalsError::GitDiscoverFailed(_)));
        let e2 = PreCommitTypeSignalsError::RulesFileMissing("arch".to_owned());
        assert!(matches!(e2, PreCommitTypeSignalsError::RulesFileMissing(_)));
        let e3 = PreCommitTypeSignalsError::RulesParseError("json".to_owned());
        assert!(matches!(e3, PreCommitTypeSignalsError::RulesParseError(_)));
        let e4 = PreCommitTypeSignalsError::SymlinkRejected("sym".to_owned());
        assert!(matches!(e4, PreCommitTypeSignalsError::SymlinkRejected(_)));
        let e5 = PreCommitTypeSignalsError::MetadataLoadFailed("meta".to_owned());
        assert!(matches!(e5, PreCommitTypeSignalsError::MetadataLoadFailed(_)));
        let e6 = PreCommitTypeSignalsError::ImplPlanLoadFailed("plan".to_owned());
        assert!(matches!(e6, PreCommitTypeSignalsError::ImplPlanLoadFailed(_)));
        let e7 = PreCommitTypeSignalsError::TypeSignalsRecomputeFailed("sig".to_owned());
        assert!(matches!(e7, PreCommitTypeSignalsError::TypeSignalsRecomputeFailed(_)));
    }

    #[test]
    fn test_pre_commit_type_signals_interactor_delegates() {
        let run_fn = Arc::new(|_: String, _: PathBuf| {
            Ok(PreCommitTypeSignalsOutput {
                blocked: false,
                frozen: false,
                red_signals: Vec::new(),
                yellow_signals: vec!["TypeFoo".to_owned()],
            })
        });
        let interactor = PreCommitTypeSignalsInteractor::new(run_fn);
        let out = interactor.run("my-track-2026".to_owned(), PathBuf::new()).unwrap();
        assert!(!out.blocked);
        assert!(!out.frozen);
        assert!(out.red_signals.is_empty());
        assert_eq!(out.yellow_signals.len(), 1);
    }
}
