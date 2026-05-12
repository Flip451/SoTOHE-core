//! `CatalogueImplSignalsService` — driving port and error type.
//!
//! Defines the application service trait and the unified error enum for the
//! `bin/sotp track catalogue-impl-signals` use case.
//!
//! [source: ADR 2026-05-11-2330 §D2]

use std::path::PathBuf;

use thiserror::Error;

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Error type for [`CatalogueImplSignalsService::run`].
///
/// Covers: invalid track id, layer-bindings load failure, catalogue load
/// failure, baseline load failure, ExtendedCrate conversion failure, schema
/// export failure (rustdoc C capture), signal evaluation failure, symlink guard
/// rejection, and no TDDD-enabled layers found.
///
/// [source: ADR 2026-05-11-2330 D2]
#[derive(Debug, Error)]
pub enum CatalogueImplSignalsError {
    /// The track ID format is invalid.
    #[error("invalid track id: {reason}")]
    InvalidTrackId {
        /// Human-readable reason from the domain validator.
        reason: String,
    },
    /// Failed to load the TDDD layer bindings from `architecture-rules.json`.
    ///
    /// Covers both "file could not be read or parsed" (`LoadFailed`) and
    /// "requested layer not found or not `tddd.enabled`" (`LayerNotFound`)
    /// from [`domain::tddd::catalogue_v2::TdddLayerBindingsError`].
    #[error("layer bindings load failed: {reason}")]
    LayerBindingsLoad {
        /// Human-readable reason.
        reason: String,
    },
    /// Failed to load the catalogue document for a layer.
    #[error("catalogue load failed for layer '{layer_id}': {reason}")]
    CatalogueLoad {
        /// Layer id for which loading failed.
        layer_id: String,
        /// Human-readable reason.
        reason: String,
    },
    /// Failed to load the baseline rustdoc JSON for a layer.
    #[error("baseline load failed for layer '{layer_id}': {reason}")]
    BaselineLoad {
        /// Layer id for which loading failed.
        layer_id: String,
        /// Human-readable reason.
        reason: String,
    },
    /// Failed to convert `CatalogueDocument` → `ExtendedCrate`.
    #[error("ExtendedCrate conversion failed for layer '{layer_id}': {reason}")]
    ExtendedCrateConversion {
        /// Layer id for which conversion failed.
        layer_id: String,
        /// Human-readable reason.
        reason: String,
    },
    /// Failed to capture the current rustdoc JSON (C-side).
    #[error("schema export failed for layer '{layer_id}': {reason}")]
    SchemaExport {
        /// Layer id for which capture failed.
        layer_id: String,
        /// Human-readable reason.
        reason: String,
    },
    /// Signal evaluation failed for a layer.
    #[error("signal evaluation failed for layer '{layer_id}': {reason}")]
    Evaluation {
        /// Layer id for which evaluation failed.
        layer_id: String,
        /// Human-readable reason.
        reason: String,
    },
    /// A symlink guard rejected a path.
    #[error("symlink guard rejected path: {path}")]
    SymlinkRejected {
        /// The rejected path (as a string for Display).
        path: String,
    },
    /// No TDDD-enabled layers found.
    #[error("no TDDD-enabled layers found in architecture-rules.json")]
    NoLayers,
}

// ---------------------------------------------------------------------------
// Service trait
// ---------------------------------------------------------------------------

/// Application service (driving port) for the `bin/sotp track
/// catalogue-impl-signals` use case.
///
/// Returns a formatted markdown string with the per-layer 11-region signal
/// table for stdout output. The `layer` parameter optionally filters to a
/// single layer; `None` means all TDDD-enabled layers.
///
/// [source: ADR 2026-05-11-2330 D2]
pub trait CatalogueImplSignalsService: Send + Sync {
    /// Runs the catalogue-impl-signals evaluation for the given track.
    ///
    /// Returns a formatted markdown report string (one section per layer).
    ///
    /// # Errors
    ///
    /// Returns [`CatalogueImplSignalsError`] on any failure (see variant docs).
    fn run(
        &self,
        track_id: String,
        workspace_root: PathBuf,
        layer: Option<String>,
    ) -> Result<String, CatalogueImplSignalsError>;
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_catalogue_impl_signals_error_display_covers_all_variants() {
        let variants = [
            CatalogueImplSignalsError::InvalidTrackId { reason: "test reason".to_owned() },
            CatalogueImplSignalsError::LayerBindingsLoad { reason: "test reason".to_owned() },
            CatalogueImplSignalsError::CatalogueLoad {
                layer_id: "domain".to_owned(),
                reason: "test reason".to_owned(),
            },
            CatalogueImplSignalsError::BaselineLoad {
                layer_id: "domain".to_owned(),
                reason: "test reason".to_owned(),
            },
            CatalogueImplSignalsError::ExtendedCrateConversion {
                layer_id: "domain".to_owned(),
                reason: "test reason".to_owned(),
            },
            CatalogueImplSignalsError::SchemaExport {
                layer_id: "domain".to_owned(),
                reason: "test reason".to_owned(),
            },
            CatalogueImplSignalsError::Evaluation {
                layer_id: "domain".to_owned(),
                reason: "test reason".to_owned(),
            },
            CatalogueImplSignalsError::SymlinkRejected { path: "/tmp/symlink".to_owned() },
            CatalogueImplSignalsError::NoLayers,
        ];
        for v in &variants {
            let msg = v.to_string();
            assert!(!msg.is_empty(), "Display must produce non-empty output for {v:?}");
        }
    }

    #[test]
    fn test_catalogue_impl_signals_error_display_contains_context() {
        let err = CatalogueImplSignalsError::CatalogueLoad {
            layer_id: "infra".to_owned(),
            reason: "file missing".to_owned(),
        };
        let msg = err.to_string();
        assert!(msg.contains("infra"), "Display must include layer_id");
        assert!(msg.contains("file missing"), "Display must include reason");
    }
}
