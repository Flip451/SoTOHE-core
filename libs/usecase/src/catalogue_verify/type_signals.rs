//! Type signals application service (usecase layer).
//!
//! Wraps `domain::schema::SchemaExporter` and `domain::TypeSignalsDocument`
//! so the CLI never imports these domain types directly (CN-01 / D1).

use std::path::PathBuf;
use std::sync::Arc;

use thiserror::Error;

// ── LayerSignalSummary ────────────────────────────────────────────────────────

/// DTO returned per layer by [`TypeSignalsService`].
///
/// Contains the `layer_id`, blue count, yellow count, and red count so the CLI
/// can print signal summaries without importing `domain::ConfidenceSignal` or
/// `domain::TypeSignalsDocument` directly.
#[derive(Debug)]
pub struct LayerSignalSummary {
    pub layer_id: String,
    pub blue_count: usize,
    pub yellow_count: usize,
    pub red_count: usize,
}

// ── TypeSignalsError ──────────────────────────────────────────────────────────

/// Error type for [`TypeSignalsService`].
///
/// Wraps invalid track ID, inactive track (done/archived), unknown layer,
/// catalogue decode failures, and schema export failures without leaking
/// `domain::schema::SchemaExportError` or `domain::TrackStatus` across the
/// usecase boundary.
#[derive(Debug, Error)]
pub enum TypeSignalsError {
    #[error("invalid track ID: {0}")]
    InvalidTrackId(String),
    #[error("inactive track: {0}")]
    InactiveTrack(String),
    #[error("unknown layer: {0}")]
    UnknownLayer(String),
    #[error("catalogue load failed: {0}")]
    CatalogueLoadFailed(String),
    #[error("schema export failed: {0}")]
    SchemaExportFailed(String),
}

// ── TypeSignalsService ────────────────────────────────────────────────────────

/// Application service trait for the track type-signals use case
/// (`sotp track type-signals`).
///
/// Driven by the CLI layer. Wraps `domain::schema::SchemaExporter` and
/// `domain::TypeSignalsDocument` so the CLI never imports these domain types
/// directly. Evaluates type signals for each TDDD-enabled layer catalogue and
/// writes the updated signal document.
pub trait TypeSignalsService: Send + Sync {
    /// Evaluates type signals for each layer.
    ///
    /// # Errors
    ///
    /// Returns [`TypeSignalsError`] on ID validation, inactive track, unknown
    /// layer, catalogue load, or schema export failures.
    fn evaluate(
        &self,
        track_id: String,
        items_dir: PathBuf,
        workspace_root: PathBuf,
        layer_filter: Option<String>,
    ) -> Result<Vec<LayerSignalSummary>, TypeSignalsError>;
}

// ── TypeSignalsInteractor ─────────────────────────────────────────────────────

/// Function type for running the type signals evaluation.
///
/// Receives `(track_id, items_dir, workspace_root, layer_filter)` and returns
/// `Result<Vec<LayerSignalSummary>, TypeSignalsError>`.
/// The CLI composition root injects the domain+infra wiring.
pub(crate) type TypeSignalsRunFn = Arc<
    dyn Fn(
            String,
            PathBuf,
            PathBuf,
            Option<String>,
        ) -> Result<Vec<LayerSignalSummary>, TypeSignalsError>
        + Send
        + Sync,
>;

/// Concrete struct implementing [`TypeSignalsService`].
///
/// Constructs domain types internally and converts results to
/// `Vec<LayerSignalSummary>` before returning to the CLI.
pub struct TypeSignalsInteractor {
    run_fn: TypeSignalsRunFn,
}

impl TypeSignalsInteractor {
    /// Creates a new interactor with the given runner function.
    #[must_use]
    pub fn new<F>(run_fn: F) -> Self
    where
        F: Fn(
                String,
                PathBuf,
                PathBuf,
                Option<String>,
            ) -> Result<Vec<LayerSignalSummary>, TypeSignalsError>
            + Send
            + Sync
            + 'static,
    {
        Self { run_fn: Arc::new(run_fn) }
    }
}

impl TypeSignalsService for TypeSignalsInteractor {
    fn evaluate(
        &self,
        track_id: String,
        items_dir: PathBuf,
        workspace_root: PathBuf,
        layer_filter: Option<String>,
    ) -> Result<Vec<LayerSignalSummary>, TypeSignalsError> {
        (self.run_fn)(track_id, items_dir, workspace_root, layer_filter)
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    #[test]
    fn test_type_signals_error_variants_exist() {
        let e1 = TypeSignalsError::InvalidTrackId("bad".to_owned());
        assert!(matches!(e1, TypeSignalsError::InvalidTrackId(_)));
        let e2 = TypeSignalsError::InactiveTrack("done".to_owned());
        assert!(matches!(e2, TypeSignalsError::InactiveTrack(_)));
        let e3 = TypeSignalsError::UnknownLayer("missing".to_owned());
        assert!(matches!(e3, TypeSignalsError::UnknownLayer(_)));
        let e4 = TypeSignalsError::CatalogueLoadFailed("err".to_owned());
        assert!(matches!(e4, TypeSignalsError::CatalogueLoadFailed(_)));
        let e5 = TypeSignalsError::SchemaExportFailed("err".to_owned());
        assert!(matches!(e5, TypeSignalsError::SchemaExportFailed(_)));
    }

    #[test]
    fn test_type_signals_interactor_delegates_and_returns_summaries() {
        let run_fn = |_: String, _: PathBuf, _: PathBuf, _: Option<String>| {
            Ok(vec![LayerSignalSummary {
                layer_id: "domain".to_owned(),
                blue_count: 3,
                yellow_count: 1,
                red_count: 0,
            }])
        };
        let interactor = TypeSignalsInteractor::new(run_fn);
        let summaries = interactor
            .evaluate("track-2026".to_owned(), PathBuf::new(), PathBuf::new(), None)
            .unwrap();
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].layer_id, "domain");
        assert_eq!(summaries[0].blue_count, 3);
    }

    #[test]
    fn test_type_signals_interactor_propagates_closure_error() {
        // Verify that an Err returned by the injected closure is propagated unchanged.
        let run_fn = |_: String, _: PathBuf, _: PathBuf, _: Option<String>| {
            Err(TypeSignalsError::CatalogueLoadFailed("missing catalogue".to_owned()))
        };
        let interactor = TypeSignalsInteractor::new(run_fn);
        let err = interactor
            .evaluate("track-2026".to_owned(), PathBuf::new(), PathBuf::new(), None)
            .unwrap_err();
        assert!(
            matches!(err, TypeSignalsError::CatalogueLoadFailed(_)),
            "expected CatalogueLoadFailed, got: {err}"
        );
    }
}
