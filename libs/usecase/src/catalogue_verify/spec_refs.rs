//! Catalogue-spec-refs verification application service (usecase layer).
//!
//! This is the usecase-layer application service wrapper (`action: modify`)
//! that wraps the inner `VerifyCatalogueSpecRefs` domain-adjacent port
//! (in `crate::catalogue_spec_refs`) and returns `VerifyCatalogueSpecRefsOutput`
//! so the CLI never imports `domain::ContentHash`, `domain::SpecElementId`,
//! or `domain::tddd::LayerId` directly (CN-01 / D1).
//!
//! The inner `catalogue_spec_refs::VerifyCatalogueSpecRefs` trait is a
//! domain-adjacent port that returns `Vec<SpecRefFinding>`. This module
//! adds the application-service facade for the CLI boundary.

use std::path::PathBuf;
use std::sync::Arc;

use thiserror::Error;

// ── VerifyCatalogueSpecRefsOutput ─────────────────────────────────────────────

/// DTO returned by [`VerifyCatalogueSpecRefsService`].
///
/// Contains `passed` (true = no findings) and a list of pre-formatted finding
/// strings ready for the CLI to emit to stderr, one per line.
/// Pre-formatting in the interactor avoids the CLI needing to pattern-match on
/// `SpecRefFindingKind` variants.
#[derive(Debug)]
pub struct VerifyCatalogueSpecRefsOutput {
    pub passed: bool,
    pub findings: Vec<String>,
}

// ── VerifyCatalogueSpecRefsError ──────────────────────────────────────────────

/// Error type for [`VerifyCatalogueSpecRefsService`].
///
/// Wraps symlink guard rejections, invalid track ID, missing
/// architecture-rules.json, missing track directory, missing spec.json,
/// catalogue decode failures, and signal decode failures without leaking
/// `domain::ContentHash`, `domain::SpecElementId`, or `domain::tddd::LayerId`
/// across the usecase boundary.
#[derive(Debug, Error)]
pub enum VerifyCatalogueSpecRefsError {
    #[error("invalid track ID: {0}")]
    InvalidTrackId(String),
    #[error("symlink rejected: {0}")]
    SymlinkRejected(String),
    #[error("rules file missing: {0}")]
    RulesFileMissing(String),
    #[error("rules parse error: {0}")]
    RulesParseError(String),
    #[error("track directory missing: {0}")]
    TrackDirectoryMissing(String),
    #[error("spec.json missing")]
    SpecJsonMissing,
    #[error("catalogue decode failed: {0}")]
    CatalogueDecodeFailed(String),
    #[error("signal decode failed: {0}")]
    SignalDecodeFailed(String),
}

// ── VerifyCatalogueSpecRefsService ────────────────────────────────────────────

/// Application service trait for the verify catalogue-spec-refs use case
/// (`sotp verify catalogue-spec-refs`).
///
/// Driven by the CLI layer. Encapsulates `domain::check_catalogue_spec_ref_integrity`,
/// `domain::ContentHash`, `domain::SpecRefFinding`, and `domain::SpecElementId`
/// so that CLI commands never import these domain types directly. Returns a
/// [`VerifyCatalogueSpecRefsOutput`] DTO that the CLI formats for stderr.
pub trait VerifyCatalogueSpecRefsService: Send + Sync {
    /// Verifies catalogue-spec refs for the given track.
    ///
    /// # Errors
    ///
    /// Returns [`VerifyCatalogueSpecRefsError`] on guard, load, or decode
    /// failures.
    fn verify(
        &self,
        track_id: String,
        items_dir: PathBuf,
        workspace_root: PathBuf,
        skip_stale: bool,
    ) -> Result<VerifyCatalogueSpecRefsOutput, VerifyCatalogueSpecRefsError>;
}

// ── VerifyCatalogueSpecRefsInteractor ─────────────────────────────────────────

/// Function type for running the catalogue-spec-refs verification.
///
/// Receives `(track_id, items_dir, workspace_root, skip_stale)` and returns
/// `Result<VerifyCatalogueSpecRefsOutput, VerifyCatalogueSpecRefsError>`.
/// The CLI composition root injects the domain+infra wiring.
pub(crate) type VerifySpecRefsRunFn = Arc<
    dyn Fn(
            String,
            PathBuf,
            PathBuf,
            bool,
        ) -> Result<VerifyCatalogueSpecRefsOutput, VerifyCatalogueSpecRefsError>
        + Send
        + Sync,
>;

/// Concrete struct implementing [`VerifyCatalogueSpecRefsService`].
///
/// Constructs domain types internally and converts results to
/// [`VerifyCatalogueSpecRefsOutput`] before returning to the CLI.
pub struct VerifyCatalogueSpecRefsInteractor {
    run_fn: VerifySpecRefsRunFn,
}

impl VerifyCatalogueSpecRefsInteractor {
    /// Creates a new interactor with the given runner function.
    #[must_use]
    pub fn new<F>(run_fn: F) -> Self
    where
        F: Fn(
                String,
                PathBuf,
                PathBuf,
                bool,
            ) -> Result<VerifyCatalogueSpecRefsOutput, VerifyCatalogueSpecRefsError>
            + Send
            + Sync
            + 'static,
    {
        Self { run_fn: Arc::new(run_fn) }
    }
}

impl VerifyCatalogueSpecRefsService for VerifyCatalogueSpecRefsInteractor {
    fn verify(
        &self,
        track_id: String,
        items_dir: PathBuf,
        workspace_root: PathBuf,
        skip_stale: bool,
    ) -> Result<VerifyCatalogueSpecRefsOutput, VerifyCatalogueSpecRefsError> {
        (self.run_fn)(track_id, items_dir, workspace_root, skip_stale)
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_verify_catalogue_spec_refs_error_variants_exist() {
        let e1 = VerifyCatalogueSpecRefsError::InvalidTrackId("bad".to_owned());
        assert!(matches!(e1, VerifyCatalogueSpecRefsError::InvalidTrackId(_)));
        let e2 = VerifyCatalogueSpecRefsError::SymlinkRejected("s".to_owned());
        assert!(matches!(e2, VerifyCatalogueSpecRefsError::SymlinkRejected(_)));
        let e3 = VerifyCatalogueSpecRefsError::RulesFileMissing("r".to_owned());
        assert!(matches!(e3, VerifyCatalogueSpecRefsError::RulesFileMissing(_)));
        let e4 = VerifyCatalogueSpecRefsError::RulesParseError("p".to_owned());
        assert!(matches!(e4, VerifyCatalogueSpecRefsError::RulesParseError(_)));
        let e5 = VerifyCatalogueSpecRefsError::TrackDirectoryMissing("d".to_owned());
        assert!(matches!(e5, VerifyCatalogueSpecRefsError::TrackDirectoryMissing(_)));
        let e6 = VerifyCatalogueSpecRefsError::SpecJsonMissing;
        assert!(matches!(e6, VerifyCatalogueSpecRefsError::SpecJsonMissing));
        let e7 = VerifyCatalogueSpecRefsError::CatalogueDecodeFailed("c".to_owned());
        assert!(matches!(e7, VerifyCatalogueSpecRefsError::CatalogueDecodeFailed(_)));
        let e8 = VerifyCatalogueSpecRefsError::SignalDecodeFailed("s".to_owned());
        assert!(matches!(e8, VerifyCatalogueSpecRefsError::SignalDecodeFailed(_)));
    }

    #[test]
    fn test_verify_catalogue_spec_refs_interactor_delegates() {
        let run_fn = |_: String, _: PathBuf, _: PathBuf, _: bool| {
            Ok(VerifyCatalogueSpecRefsOutput {
                passed: false,
                findings: vec!["[error] stale signal".to_owned()],
            })
        };
        let interactor = VerifyCatalogueSpecRefsInteractor::new(run_fn);
        let out = interactor
            .verify("track-2026".to_owned(), PathBuf::new(), PathBuf::new(), false)
            .unwrap();
        assert!(!out.passed);
        assert_eq!(out.findings.len(), 1);
    }

    #[test]
    fn test_verify_catalogue_spec_refs_interactor_propagates_closure_error() {
        // Verify that an Err returned by the injected closure is propagated unchanged.
        let run_fn = |_: String, _: PathBuf, _: PathBuf, _: bool| {
            Err(VerifyCatalogueSpecRefsError::RulesFileMissing("rules.json".to_owned()))
        };
        let interactor = VerifyCatalogueSpecRefsInteractor::new(run_fn);
        let err = interactor
            .verify("track-2026".to_owned(), PathBuf::new(), PathBuf::new(), false)
            .unwrap_err();
        assert!(
            matches!(err, VerifyCatalogueSpecRefsError::RulesFileMissing(_)),
            "expected RulesFileMissing, got: {err}"
        );
    }
}
