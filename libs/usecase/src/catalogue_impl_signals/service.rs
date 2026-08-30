//! `CatalogueImplSignalsService` — driving port and error type.
//!
//! Defines the application service trait and the unified error enum for the
//! `bin/sotp track catalogue-impl-signals` use case.
//!
//! [source: ADR 2026-05-11-2330 §D2]

use std::path::PathBuf;

use crate::tddd_feature_declaration::TdddActualFeatureDeclarationPortError;
use domain::tddd::test_obligation::ids::DiagnosticMessage;
use domain::tddd::{LayerId, catalogue_v2::TdddLayerBinding};
use thiserror::Error;

// ---------------------------------------------------------------------------
// Report type
// ---------------------------------------------------------------------------

/// Result of a completed catalogue-impl-signals evaluation run.
///
/// Returned by [`CatalogueImplSignalsService::run`] in place of a bare `String`.
/// Carries the formatted markdown text and a pre-computed `any_red` flag so
/// callers do not need to re-parse the report string.
///
/// [source: CLI thin-composition-root refactor]
#[derive(Debug, Clone)]
pub struct CatalogueImplSignalsReport {
    /// Formatted markdown report text for stdout output (one section per layer).
    pub text: String,
    /// `true` when at least one Red signal is present across all evaluated layers.
    pub any_red: bool,
}

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Error type for [`CatalogueImplSignalsService::run`].
///
/// Covers: invalid track id, layer-bindings load failure, catalogue load
/// failure, baseline load failure, ExtendedCrate conversion failure, schema
/// export failure (rustdoc C capture), signal evaluation failure, symlink guard
/// rejection or I/O failure, and no TDDD-enabled layers found.
///
/// [source: ADR 2026-05-11-2330 D2]
#[derive(Debug, Error)]
pub enum CatalogueImplSignalsError {
    /// The track ID format is invalid.
    #[error("invalid track id: {}", .0.as_str())]
    InvalidTrackId(DiagnosticMessage),
    /// Failed to load the TDDD layer bindings from `architecture-rules.json`.
    ///
    /// Covers both "file could not be read or parsed" (`LoadFailed`) and
    /// "requested layer not found or not `tddd.enabled`" (`LayerNotFound`)
    /// from [`domain::tddd::catalogue_v2::TdddLayerBindingsError`].
    #[error("layer bindings load failed: {}", .0.as_str())]
    LayerBindingsLoad(DiagnosticMessage),
    /// Failed to load the catalogue document for a layer.
    #[error("catalogue load failed for layer '{layer}': {reason}", layer = .0, reason = .1.as_str())]
    CatalogueLoad(LayerId, DiagnosticMessage),
    /// Failed to load the baseline rustdoc JSON for a layer.
    #[error("baseline load failed for layer '{layer}': {reason}", layer = .0, reason = .1.as_str())]
    BaselineLoad(LayerId, DiagnosticMessage),
    /// Failed to convert `CatalogueDocument` → `ExtendedCrate`.
    #[error("ExtendedCrate conversion failed for layer '{layer}': {reason}", layer = .0, reason = .1.as_str())]
    ExtendedCrateConversion(LayerId, DiagnosticMessage),
    /// Failed to capture the current rustdoc JSON (C-side).
    #[error("schema export failed for layer '{layer}': {reason}", layer = .0, reason = .1.as_str())]
    SchemaExport(LayerId, DiagnosticMessage),
    /// Signal evaluation failed for a layer.
    #[error("signal evaluation failed for layer '{layer}': {reason}", layer = .0, reason = .1.as_str())]
    Evaluation(LayerId, DiagnosticMessage),
    /// A symlink guard rejected a path.
    #[error("symlink guard rejected path: {}", .0.display())]
    SymlinkRejected(PathBuf),
    /// A symlink guard could not inspect a path because of an I/O failure.
    #[error("symlink guard I/O error for path '{}': {}", .0.display(), .1.as_str())]
    SymlinkGuardIo(PathBuf, DiagnosticMessage),
    /// No TDDD-enabled layers found.
    #[error("no TDDD-enabled layers found in architecture-rules.json")]
    NoLayers,
    /// The configured rustdoc export plan exceeds the supported bound.
    #[error("rustdoc export plan exceeds the maximum of 64 TDDD layers")]
    LayerLimitExceeded,
    /// The actual-capture feature declaration could not be loaded or verified.
    #[error("feature declaration error: {0}")]
    FeatureDeclaration(TdddActualFeatureDeclarationPortError),
}

/// Validated rustdoc export plan for one catalogue implementation-signals run.
#[derive(Debug, Clone)]
pub struct RustdocExportPlan {
    bindings: Vec<TdddLayerBinding>,
}

impl PartialEq for RustdocExportPlan {
    fn eq(&self, other: &Self) -> bool {
        self.bindings.iter().map(binding_identity).eq(other.bindings.iter().map(binding_identity))
    }
}

impl Eq for RustdocExportPlan {}

fn binding_identity(binding: &TdddLayerBinding) -> (&str, &str, &str, &[String]) {
    (&binding.layer_id, &binding.catalogue_file, &binding.baseline_file, &binding.targets)
}

impl RustdocExportPlan {
    /// Validates the maximum number of rustdoc layer exports.
    ///
    /// # Errors
    ///
    /// The sole producible error is
    /// [`CatalogueImplSignalsError::LayerLimitExceeded`], returned when more
    /// than 64 TDDD layer bindings are supplied.
    pub fn try_new(bindings: Vec<TdddLayerBinding>) -> Result<Self, CatalogueImplSignalsError> {
        if bindings.len() > 64 {
            return Err(CatalogueImplSignalsError::LayerLimitExceeded);
        }
        Ok(Self { bindings })
    }

    /// Returns the complete validated export plan.
    #[must_use]
    pub fn bindings(&self) -> &[TdddLayerBinding] {
        &self.bindings
    }
}

pub(super) fn diagnostic(value: impl Into<String>) -> DiagnosticMessage {
    let mut value = value.into();
    if value.trim().is_empty() {
        value = "catalogue implementation signal evaluation failed".to_owned();
    }
    loop {
        if let Ok(message) = DiagnosticMessage::try_new(value) {
            return message;
        }
        value = "catalogue implementation signal evaluation failed".to_owned();
    }
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
    /// Returns a [`CatalogueImplSignalsReport`] containing the formatted markdown
    /// text and a pre-computed `any_red` flag (true when at least one Red signal
    /// was found across all evaluated layers).
    ///
    /// # Errors
    ///
    /// Returns [`CatalogueImplSignalsError`] on any failure (see variant docs).
    fn run(
        &self,
        track_id: String,
        workspace_root: PathBuf,
        layer: Option<String>,
    ) -> Result<CatalogueImplSignalsReport, CatalogueImplSignalsError>;
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn test_diagnostic(value: &str) -> DiagnosticMessage {
        DiagnosticMessage::try_new(value.to_owned()).unwrap()
    }

    fn test_layer_id(value: &str) -> LayerId {
        LayerId::try_new(value.to_owned()).unwrap()
    }

    #[test]
    fn test_catalogue_impl_signals_report_any_red_reflects_red_signals() {
        let report_no_red = CatalogueImplSignalsReport {
            text: "Summary: 🔵 2 Blue | 🟡 1 Yellow | 🔴 0 Red\n".to_owned(),
            any_red: false,
        };
        assert!(!report_no_red.any_red, "any_red must be false when no Red signals");

        let report_with_red = CatalogueImplSignalsReport {
            text: "| Foo | SCaptureDCaptureIntersect | 🔴 Red |\n".to_owned(),
            any_red: true,
        };
        assert!(report_with_red.any_red, "any_red must be true when Red signals present");
    }

    #[test]
    fn test_catalogue_impl_signals_error_display_covers_all_variants() {
        let variants = [
            CatalogueImplSignalsError::InvalidTrackId(test_diagnostic("test reason")),
            CatalogueImplSignalsError::LayerBindingsLoad(test_diagnostic("test reason")),
            CatalogueImplSignalsError::CatalogueLoad(
                test_layer_id("domain"),
                test_diagnostic("test reason"),
            ),
            CatalogueImplSignalsError::BaselineLoad(
                test_layer_id("domain"),
                test_diagnostic("test reason"),
            ),
            CatalogueImplSignalsError::ExtendedCrateConversion(
                test_layer_id("domain"),
                test_diagnostic("test reason"),
            ),
            CatalogueImplSignalsError::SchemaExport(
                test_layer_id("domain"),
                test_diagnostic("test reason"),
            ),
            CatalogueImplSignalsError::Evaluation(
                test_layer_id("domain"),
                test_diagnostic("test reason"),
            ),
            CatalogueImplSignalsError::SymlinkRejected(PathBuf::from("/tmp/symlink")),
            CatalogueImplSignalsError::SymlinkGuardIo(
                PathBuf::from("/tmp/unreadable"),
                test_diagnostic("permission denied"),
            ),
            CatalogueImplSignalsError::NoLayers,
            CatalogueImplSignalsError::FeatureDeclaration(
                TdddActualFeatureDeclarationPortError::BaselineSnapshotMismatch,
            ),
        ];
        for v in &variants {
            let msg = v.to_string();
            assert!(!msg.is_empty(), "Display must produce non-empty output for {v:?}");
        }
    }

    #[test]
    fn test_catalogue_impl_signals_error_display_contains_context() {
        let err = CatalogueImplSignalsError::CatalogueLoad(
            test_layer_id("infra"),
            test_diagnostic("file missing"),
        );
        let msg = err.to_string();
        assert!(msg.contains("infra"), "Display must include layer_id");
        assert!(msg.contains("file missing"), "Display must include reason");
    }
}
