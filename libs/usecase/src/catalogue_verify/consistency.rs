//! Catalogue consistency verification application service (usecase layer).
//!
//! Wraps `domain::check_consistency` so the CLI never imports
//! `domain::TypeCatalogueDocument`, `domain::TypeGraph`, `domain::TypeBaseline`,
//! or `domain::ConsistencyReport` directly (CN-01 / D1).

use std::path::PathBuf;
use std::sync::Arc;

use thiserror::Error;

// ── VerifyCatalogueConsistencyOutput ──────────────────────────────────────────

/// DTO returned by [`VerifyCatalogueConsistencyService`].
///
/// Contains the pass/fail verdict and a list of finding strings so the CLI can
/// print results without importing `domain::ConsistencyReport` or
/// `domain::verify::VerifyFinding` directly.
pub struct VerifyCatalogueConsistencyOutput {
    pub passed: bool,
    pub findings: Vec<String>,
}

// ── VerifyCatalogueConsistencyError ───────────────────────────────────────────

/// Error type for [`VerifyCatalogueConsistencyService`].
///
/// Wraps invalid track ID, catalogue decode failures, schema export failures,
/// and baseline load failures without leaking domain error types or
/// `domain::schema::SchemaExportError` across the usecase boundary.
#[derive(Debug, Error)]
pub enum VerifyCatalogueConsistencyError {
    #[error("invalid track ID: {0}")]
    InvalidTrackId(String),
    #[error("catalogue load failed: {0}")]
    CatalogueLoadFailed(String),
    #[error("schema export failed: {0}")]
    SchemaExportFailed(String),
    #[error("baseline load failed: {0}")]
    BaselineLoadFailed(String),
}

// ── VerifyCatalogueConsistencyService ─────────────────────────────────────────

/// Application service trait for the verify catalogue-consistency use case
/// (`sotp verify catalogue-consistency`).
///
/// Driven by the CLI layer. Wraps `domain::check_consistency` so the CLI never
/// imports `domain::TypeCatalogueDocument`, `domain::TypeGraph`,
/// `domain::TypeBaseline`, or `domain::ConsistencyReport` directly.
pub trait VerifyCatalogueConsistencyService: Send + Sync {
    /// Verifies catalogue consistency for the given track.
    ///
    /// # Errors
    ///
    /// Returns [`VerifyCatalogueConsistencyError`] on ID validation, load, or
    /// export failures.
    fn verify(
        &self,
        track_id: String,
        items_dir: PathBuf,
        workspace_root: PathBuf,
    ) -> Result<VerifyCatalogueConsistencyOutput, VerifyCatalogueConsistencyError>;
}

// ── VerifyCatalogueConsistencyInteractor ──────────────────────────────────────

/// Concrete struct implementing [`VerifyCatalogueConsistencyService`].
///
/// Constructs domain types internally and converts results to
/// [`VerifyCatalogueConsistencyOutput`] before returning to the CLI.
///
/// Uses the `run_fn` closure pattern to avoid importing infrastructure
/// types from the usecase crate (same as `ReviewCheckApprovedInteractor`).
pub struct VerifyCatalogueConsistencyInteractor {
    run_fn: Arc<
        dyn Fn(
                String,
                PathBuf,
                PathBuf,
            )
                -> Result<VerifyCatalogueConsistencyOutput, VerifyCatalogueConsistencyError>
            + Send
            + Sync,
    >,
}

impl VerifyCatalogueConsistencyInteractor {
    /// Creates a new interactor with the given runner function.
    #[must_use]
    pub fn new(
        run_fn: Arc<
            dyn Fn(
                    String,
                    PathBuf,
                    PathBuf,
                )
                    -> Result<VerifyCatalogueConsistencyOutput, VerifyCatalogueConsistencyError>
                + Send
                + Sync,
        >,
    ) -> Self {
        Self { run_fn }
    }
}

impl VerifyCatalogueConsistencyService for VerifyCatalogueConsistencyInteractor {
    fn verify(
        &self,
        track_id: String,
        items_dir: PathBuf,
        workspace_root: PathBuf,
    ) -> Result<VerifyCatalogueConsistencyOutput, VerifyCatalogueConsistencyError> {
        (self.run_fn)(track_id, items_dir, workspace_root)
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_verify_catalogue_consistency_error_variants_exist() {
        let e1 = VerifyCatalogueConsistencyError::InvalidTrackId("bad".to_owned());
        assert!(matches!(e1, VerifyCatalogueConsistencyError::InvalidTrackId(_)));
        let e2 = VerifyCatalogueConsistencyError::CatalogueLoadFailed("err".to_owned());
        assert!(matches!(e2, VerifyCatalogueConsistencyError::CatalogueLoadFailed(_)));
        let e3 = VerifyCatalogueConsistencyError::SchemaExportFailed("err".to_owned());
        assert!(matches!(e3, VerifyCatalogueConsistencyError::SchemaExportFailed(_)));
        let e4 = VerifyCatalogueConsistencyError::BaselineLoadFailed("err".to_owned());
        assert!(matches!(e4, VerifyCatalogueConsistencyError::BaselineLoadFailed(_)));
    }

    #[test]
    fn test_verify_catalogue_consistency_interactor_delegates() {
        let run_fn = Arc::new(|_: String, _: PathBuf, _: PathBuf| {
            Ok(VerifyCatalogueConsistencyOutput { passed: true, findings: Vec::new() })
        });
        let interactor = VerifyCatalogueConsistencyInteractor::new(run_fn);
        let out =
            interactor.verify("track-2026".to_owned(), PathBuf::new(), PathBuf::new()).unwrap();
        assert!(out.passed);
        assert!(out.findings.is_empty());
    }
}
