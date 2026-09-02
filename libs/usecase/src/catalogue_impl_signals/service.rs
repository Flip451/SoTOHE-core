//! `CatalogueImplSignalsService` — driving port and error type.
//!
//! Defines the application service trait and the unified error enum for the
//! `bin/sotp track catalogue-impl-signals` use case.
//!
//! [source: ADR 2026-05-11-2330 §D2]

use std::collections::BTreeMap;
use std::path::PathBuf;

use super::ports::EvaluationStartCaptureError;
use crate::tddd_feature_declaration::TdddActualFeatureDeclarationPortError;
use domain::tddd::test_obligation::ids::DiagnosticMessage;
use domain::tddd::{
    ImplementationFingerprint, LayerId,
    catalogue_v2::{AttestedCatalogueDocument, TdddLayerBinding},
};
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

/// Validation failure while constructing a catalogue-backed rustdoc export plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum RustdocExportPlanValidationError {
    /// No selected or catalogue-bearing layer can be exported.
    #[error("no TDDD-enabled layers found in architecture-rules.json")]
    NoLayers,
    /// The export plan would require more than the supported 64 layer exports.
    #[error("rustdoc export plan exceeds the maximum of 64 TDDD layers")]
    LayerLimitExceeded,
}

/// Error type for [`CatalogueImplSignalsService::run`].
///
/// Covers: invalid track id, layer-bindings load failure, catalogue load
/// failure, baseline load failure, ExtendedCrate conversion failure, schema
/// export failure (rustdoc C capture), evaluation-start fingerprint capture
/// failure, signal evaluation failure, symlink guard rejection or I/O failure,
/// and no TDDD-enabled layers found.
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
    /// Failed to capture the run-wide evaluation-start implementation fingerprint.
    #[error("evaluation-start capture failed: {0}")]
    EvaluationStartCapture(EvaluationStartCaptureError),
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

impl From<RustdocExportPlanValidationError> for CatalogueImplSignalsError {
    fn from(error: RustdocExportPlanValidationError) -> Self {
        match error {
            RustdocExportPlanValidationError::NoLayers => Self::NoLayers,
            RustdocExportPlanValidationError::LayerLimitExceeded => Self::LayerLimitExceeded,
        }
    }
}

/// Validated rustdoc export plan for one catalogue implementation-signals run.
///
/// The plan contains the selected target layer(s) and every other declaring
/// layer whose catalogue was actually loaded. Keeping the run-wide
/// implementation fingerprint beside those bindings lets later orchestration
/// steps prove that every export belongs to the same evaluation start.
#[derive(Debug, Clone)]
pub struct RustdocExportPlan {
    export_bindings: Vec<TdddLayerBinding>,
    implementation_fingerprint: ImplementationFingerprint,
}

impl PartialEq for RustdocExportPlan {
    fn eq(&self, other: &Self) -> bool {
        self.implementation_fingerprint == other.implementation_fingerprint
            && self
                .export_bindings
                .iter()
                .map(binding_identity)
                .eq(other.export_bindings.iter().map(binding_identity))
    }
}

impl Eq for RustdocExportPlan {}

fn binding_identity(binding: &TdddLayerBinding) -> (&str, &str, &str, &[String]) {
    (&binding.layer_id, &binding.catalogue_file, &binding.baseline_file, &binding.targets)
}

impl RustdocExportPlan {
    /// Validates and constructs the catalogue-bearing rustdoc export plan.
    ///
    /// A selected target is always retained. Other exports are retained only
    /// when their layer is present in `catalogues`; an absent non-target
    /// catalogue therefore cannot cause a baseline load or rustdoc export.
    /// Bindings retain the architecture-rules order and are exported once per
    /// layer, even when a selected target is also catalogue-bearing.
    ///
    /// # Errors
    ///
    /// Returns [`RustdocExportPlanValidationError::NoLayers`] when there is no
    /// selected or catalogue-bearing binding, and
    /// [`RustdocExportPlanValidationError::LayerLimitExceeded`] when the
    /// resulting export set contains more than 64 layers.
    pub fn try_new(
        evaluation_start: ImplementationFingerprint,
        selected_bindings: &[TdddLayerBinding],
        declaration_bindings: &[TdddLayerBinding],
        catalogues: &BTreeMap<LayerId, AttestedCatalogueDocument>,
    ) -> Result<Self, RustdocExportPlanValidationError> {
        let mut export_bindings = Vec::new();

        // The declaration order is the stable order supplied by
        // architecture-rules.json.  It also preserves the existing capture
        // ordering for filtered runs, while the membership test below narrows
        // the set to selected targets plus catalogue-bearing declarers.
        for binding in declaration_bindings {
            let is_selected =
                selected_bindings.iter().any(|selected| selected.layer_id == binding.layer_id);
            let has_catalogue =
                catalogues.keys().any(|layer_id| layer_id.as_ref() == binding.layer_id.as_str());
            if (is_selected || has_catalogue)
                && !export_bindings
                    .iter()
                    .any(|exported: &TdddLayerBinding| exported.layer_id == binding.layer_id)
            {
                export_bindings.push(binding.clone());
            }
        }

        // `selected_bindings` normally comes from `declaration_bindings`. Keep
        // the plan total for direct callers as well, without allowing a
        // duplicate layer to consume another export slot.
        for binding in selected_bindings {
            if !export_bindings
                .iter()
                .any(|exported: &TdddLayerBinding| exported.layer_id == binding.layer_id)
            {
                export_bindings.push(binding.clone());
            }
        }

        if export_bindings.is_empty() {
            return Err(RustdocExportPlanValidationError::NoLayers);
        }
        if export_bindings.len() > 64 {
            return Err(RustdocExportPlanValidationError::LayerLimitExceeded);
        }
        Ok(Self { export_bindings, implementation_fingerprint: evaluation_start })
    }

    /// Returns the complete validated export plan in architecture order.
    #[must_use]
    pub fn export_bindings(&self) -> &[TdddLayerBinding] {
        &self.export_bindings
    }

    /// Returns the implementation fingerprint captured at evaluation start.
    #[must_use]
    pub fn implementation_fingerprint(&self) -> &ImplementationFingerprint {
        &self.implementation_fingerprint
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

    fn test_stub_bindings(count: usize) -> Vec<TdddLayerBinding> {
        (0..count)
            .map(|index| {
                let layer_id = format!("layer_{index}");
                TdddLayerBinding {
                    layer_id: layer_id.clone(),
                    catalogue_file: format!("{layer_id}-types.json"),
                    baseline_file: format!("{layer_id}-types-baseline.json"),
                    targets: vec![layer_id],
                }
            })
            .collect()
    }

    #[test]
    fn test_rustdoc_export_plan_try_new_with_64_bindings_accepts_and_returns_bindings() {
        let bindings = test_stub_bindings(64);
        let catalogues = BTreeMap::new();

        let plan = RustdocExportPlan::try_new(
            ImplementationFingerprint::new(
                domain::tddd::Sha256Digest::try_new("a".repeat(64)).unwrap(),
            ),
            &bindings,
            &bindings,
            &catalogues,
        )
        .unwrap();

        assert!(
            plan.export_bindings()
                .iter()
                .map(binding_identity)
                .eq(bindings.iter().map(binding_identity))
        );
    }

    #[test]
    fn test_rustdoc_export_plan_try_new_with_65_bindings_returns_layer_limit_exceeded() {
        let bindings = test_stub_bindings(65);
        let result = RustdocExportPlan::try_new(
            ImplementationFingerprint::new(
                domain::tddd::Sha256Digest::try_new("a".repeat(64)).unwrap(),
            ),
            &bindings,
            &bindings,
            &BTreeMap::new(),
        );

        assert!(matches!(result, Err(RustdocExportPlanValidationError::LayerLimitExceeded)));
    }

    #[test]
    fn test_rustdoc_export_plan_try_new_keeps_selected_and_catalogue_bearing_layers_once() {
        let selected = vec![test_stub_bindings(1).remove(0)];
        let declaration_bindings = test_stub_bindings(3);
        let catalogues = declaration_bindings
            .iter()
            .filter(|binding| binding.layer_id != "layer_1")
            .map(|binding| {
                (
                    LayerId::try_new(binding.layer_id.clone()).unwrap(),
                    AttestedCatalogueDocument::attest(b"catalogue", |_| {
                        Ok::<_, std::convert::Infallible>(
                            domain::tddd::catalogue_v2::CatalogueDocument::new(
                                3,
                                domain::tddd::catalogue_v2::CrateName::new(
                                    binding.layer_id.clone(),
                                )
                                .unwrap(),
                                LayerId::try_new(binding.layer_id.clone()).unwrap(),
                            ),
                        )
                    })
                    .unwrap(),
                )
            })
            .collect::<BTreeMap<_, _>>();

        let plan = RustdocExportPlan::try_new(
            ImplementationFingerprint::new(
                domain::tddd::Sha256Digest::try_new("b".repeat(64)).unwrap(),
            ),
            &selected,
            &declaration_bindings,
            &catalogues,
        )
        .unwrap();

        assert_eq!(
            plan.export_bindings()
                .iter()
                .map(|binding| binding.layer_id.as_str())
                .collect::<Vec<_>>(),
            vec!["layer_0", "layer_2"]
        );
        assert_eq!(plan.implementation_fingerprint().as_digest().as_str(), "b".repeat(64));
    }

    #[test]
    fn test_rustdoc_export_plan_try_new_without_selected_or_catalogue_layers_returns_no_layers() {
        let result = RustdocExportPlan::try_new(
            ImplementationFingerprint::new(
                domain::tddd::Sha256Digest::try_new("a".repeat(64)).unwrap(),
            ),
            &[],
            &[],
            &BTreeMap::new(),
        );

        assert!(matches!(result, Err(RustdocExportPlanValidationError::NoLayers)));
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
            CatalogueImplSignalsError::EvaluationStartCapture(
                EvaluationStartCaptureError::AuthoritativeInput {
                    reason: domain::FreeText::new("test reason"),
                },
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
            CatalogueImplSignalsError::LayerLimitExceeded,
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
