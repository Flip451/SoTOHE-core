//! `CatalogueImplSignalsService` and `CatalogueImplSignalsInteractor`.
//!
//! Application service (driving port) and interactor for the
//! `bin/sotp track catalogue-impl-signals` use case.
//!
//! Orchestrates per-layer A/B/C TypeGraph fetch, signal evaluator invocation,
//! and region-by-region result formatting. All I/O is performed via injected
//! secondary ports — no direct infrastructure calls from this module.
//!
//! The CLI layer (`apps/cli`) constructs the concrete infrastructure adapters
//! (`FsCatalogueDocumentLoader`, `CatalogueToExtendedCrateCodec`,
//! `SignalEvaluatorV2`, `RustdocCrateAdapter`) and injects them at the
//! composition root.
//!
//! [source: ADR 2026-05-11-2330 §D2, §D3]

mod helpers;
mod interactor;
mod service;

pub use interactor::CatalogueImplSignalsInteractor;
pub use service::{
    CatalogueImplSignalsError, CatalogueImplSignalsReport, CatalogueImplSignalsService,
};

// ---------------------------------------------------------------------------
// Private helpers (shared across submodules)
// ---------------------------------------------------------------------------

/// Validates a track ID string (lowercase slug: `[a-z0-9]([a-z0-9-]*[a-z0-9])?`).
///
/// Delegates to the canonical domain `TrackId::try_new` validation, mapping the
/// domain `ValidationError` into this module's `InvalidTrackId` variant so the
/// slug rule has a single source of truth (ADR D1).
///
/// # Errors
///
/// Returns `CatalogueImplSignalsError::InvalidTrackId` if the ID is invalid.
pub(crate) fn validate_track_id(id: &str) -> Result<(), CatalogueImplSignalsError> {
    domain::TrackId::try_new(id)
        .map(|_| ())
        .map_err(|e| CatalogueImplSignalsError::InvalidTrackId { reason: e.to_string() })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_track_id_empty_returns_error() {
        let err = validate_track_id("").unwrap_err();
        assert!(matches!(err, CatalogueImplSignalsError::InvalidTrackId { .. }));
    }

    #[test]
    fn test_validate_track_id_valid_slug_passes() {
        validate_track_id("my-track-2026-01-01").unwrap();
    }

    #[test]
    fn test_validate_track_id_single_segment_passes() {
        validate_track_id("tddd").unwrap();
    }

    #[test]
    fn test_validate_track_id_leading_hyphen_returns_error() {
        let err = validate_track_id("-my-track").unwrap_err();
        assert!(matches!(err, CatalogueImplSignalsError::InvalidTrackId { .. }));
    }

    #[test]
    fn test_validate_track_id_double_hyphen_returns_error() {
        let err = validate_track_id("my--track").unwrap_err();
        assert!(matches!(err, CatalogueImplSignalsError::InvalidTrackId { .. }));
    }

    #[test]
    fn test_validate_track_id_trailing_hyphen_returns_error() {
        let err = validate_track_id("my-track-").unwrap_err();
        assert!(matches!(err, CatalogueImplSignalsError::InvalidTrackId { .. }));
    }

    #[test]
    fn test_validate_track_id_invalid_character_returns_error() {
        let err = validate_track_id("bad track id!!").unwrap_err();
        assert!(matches!(err, CatalogueImplSignalsError::InvalidTrackId { .. }));
    }

    #[test]
    fn test_validate_track_id_uppercase_returns_error() {
        let err = validate_track_id("My-Track").unwrap_err();
        assert!(matches!(err, CatalogueImplSignalsError::InvalidTrackId { .. }));
    }
}
