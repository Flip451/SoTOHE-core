//! Catalogue-spec-signals verification application service (usecase layer).
//!
//! Wraps `domain::check_catalogue_spec_signals` so the CLI never imports
//! `domain::CatalogueSpecSignal`, `domain::evaluate_catalogue_entry_signal`,
//! or `domain::ContentHash` directly (CN-01 / D1).

use std::path::PathBuf;
use std::sync::Arc;

use thiserror::Error;

// ── VerifySpecSignalsOutput ───────────────────────────────────────────────────

/// DTO returned by [`VerifyCatalogueSpecSignalsService`].
///
/// Contains the pass/fail verdict and a list of finding strings for each layer,
/// so the CLI can print results without importing domain signal types directly.
pub struct VerifySpecSignalsOutput {
    pub passed: bool,
    pub findings: Vec<String>,
}

// ── VerifySpecSignalsError ────────────────────────────────────────────────────

/// Error type for [`VerifyCatalogueSpecSignalsService`].
///
/// Wraps track ID validation, catalogue decode, and spec signal evaluation
/// failures without leaking `domain::ConfidenceSignal` or `domain::ContentHash`
/// across the usecase boundary.
#[derive(Debug, Error)]
pub enum VerifySpecSignalsError {
    #[error("invalid track ID: {0}")]
    InvalidTrackId(String),
    #[error("catalogue load failed: {0}")]
    CatalogueLoadFailed(String),
    #[error("signal evaluation failed: {0}")]
    SignalEvaluationFailed(String),
}

// ── VerifyCatalogueSpecSignalsService ─────────────────────────────────────────

/// Application service trait for the verify catalogue-spec-signals use case
/// (`sotp verify catalogue-spec-signals`).
///
/// Driven by the CLI layer. Wraps `domain::check_catalogue_spec_signals` so
/// the CLI never imports `domain::CatalogueSpecSignal`,
/// `domain::evaluate_catalogue_entry_signal`, or `domain::ContentHash` directly.
pub trait VerifyCatalogueSpecSignalsService: Send + Sync {
    /// Verifies catalogue-spec signals for the given track.
    ///
    /// # Errors
    ///
    /// Returns [`VerifySpecSignalsError`] on ID validation, catalogue load, or
    /// evaluation failures.
    fn verify(
        &self,
        track_id: String,
        items_dir: PathBuf,
        skip_stale: bool,
    ) -> Result<VerifySpecSignalsOutput, VerifySpecSignalsError>;
}

// ── VerifyCatalogueSpecSignalsInteractor ──────────────────────────────────────

/// Concrete struct implementing [`VerifyCatalogueSpecSignalsService`].
///
/// Constructs domain types internally and converts results to
/// [`VerifySpecSignalsOutput`] before returning to the CLI.
pub struct VerifyCatalogueSpecSignalsInteractor {
    run_fn: Arc<
        dyn Fn(String, PathBuf, bool) -> Result<VerifySpecSignalsOutput, VerifySpecSignalsError>
            + Send
            + Sync,
    >,
}

impl VerifyCatalogueSpecSignalsInteractor {
    /// Creates a new interactor with the given runner function.
    #[must_use]
    pub fn new(
        run_fn: Arc<
            dyn Fn(String, PathBuf, bool) -> Result<VerifySpecSignalsOutput, VerifySpecSignalsError>
                + Send
                + Sync,
        >,
    ) -> Self {
        Self { run_fn }
    }
}

impl VerifyCatalogueSpecSignalsService for VerifyCatalogueSpecSignalsInteractor {
    fn verify(
        &self,
        track_id: String,
        items_dir: PathBuf,
        skip_stale: bool,
    ) -> Result<VerifySpecSignalsOutput, VerifySpecSignalsError> {
        (self.run_fn)(track_id, items_dir, skip_stale)
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_verify_spec_signals_error_variants_exist() {
        let e1 = VerifySpecSignalsError::InvalidTrackId("bad".to_owned());
        assert!(matches!(e1, VerifySpecSignalsError::InvalidTrackId(_)));
        let e2 = VerifySpecSignalsError::CatalogueLoadFailed("err".to_owned());
        assert!(matches!(e2, VerifySpecSignalsError::CatalogueLoadFailed(_)));
        let e3 = VerifySpecSignalsError::SignalEvaluationFailed("err".to_owned());
        assert!(matches!(e3, VerifySpecSignalsError::SignalEvaluationFailed(_)));
    }

    #[test]
    fn test_verify_catalogue_spec_signals_interactor_delegates() {
        let run_fn = Arc::new(|_: String, _: PathBuf, _: bool| {
            Ok(VerifySpecSignalsOutput { passed: true, findings: vec!["finding".to_owned()] })
        });
        let interactor = VerifyCatalogueSpecSignalsInteractor::new(run_fn);
        let out = interactor.verify("track-2026".to_owned(), PathBuf::new(), false).unwrap();
        assert!(out.passed);
        assert_eq!(out.findings.len(), 1);
    }
}
