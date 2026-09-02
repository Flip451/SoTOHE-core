//! `CatalogueImplSignalsInteractor` — implements [`CatalogueImplSignalsService`].
//!
//! Orchestrates per-layer A/B/C TypeGraph fetch, signal evaluator invocation,
//! and region-by-region result formatting.
//!
//! [source: ADR 2026-05-11-2330 §D2, §D3]

use std::collections::BTreeMap;
use std::fmt::Write as FmtWrite;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use domain::SymlinkGuardPort;
use domain::tddd::LayerId;
use domain::tddd::catalogue_v2::{
    AttestedCatalogueDocument, CatalogueDocument, CatalogueDocumentLoaderError, CrateName,
    RustdocCratePort, TdddLayerBindingsPort,
};
use domain::tddd::signal_evaluator::{SignalEvaluatorPort, ThreeWaySignal, ThreeWaySignalKind};
use domain::tddd::{AuthoritativeRustdocContext, CatalogueToExtendedCratePort};

use super::helpers::{map_symlink_guard_error, validate_binding_filename};
use super::ports::EvaluationStartCapturePort;
use super::service::{
    CatalogueImplSignalsError, CatalogueImplSignalsReport, CatalogueImplSignalsService,
    RustdocExportPlan, diagnostic,
};
use super::validate_track_id;
use crate::catalogue_document_loader::AttestedCatalogueDocumentLoaderPort;
use crate::tddd_feature_declaration::TdddActualFeatureDeclarationPort;

// ---------------------------------------------------------------------------
// Interactor
// ---------------------------------------------------------------------------

/// Interactor implementing [`CatalogueImplSignalsService`].
///
/// Orchestrates per-layer A/B/C TypeGraph fetch, signal evaluator invocation,
/// and region-by-region result formatting. All I/O is performed via injected
/// ports (no direct infrastructure calls):
/// - `AttestedCatalogueDocumentLoaderPort` (A-side catalogue file load)
/// - `CatalogueToExtendedCratePort` (A-side `CatalogueDocument` → `ExtendedCrate`)
/// - `SignalEvaluatorPort` (Phase 1 + Phase 2 evaluation)
/// - `EvaluationStartCapturePort` (run-wide implementation fingerprint)
/// - `RustdocCratePort` (B-side baseline load via `load_from_path`;
///   C-side live capture via `capture_current`)
/// - `TdddLayerBindingsPort` (reads `architecture-rules.json` to enumerate layers;
///   keeps usecase free of `std::fs` per hexagonal-purity rule)
/// - `SymlinkGuardPort` (symlink stat checks; keeps usecase free of direct
///   `std::fs` I/O per hexagonal-purity rule)
///
/// `apps/cli` constructs the concrete infrastructure adapters at the composition
/// root and injects them.
///
/// [source: ADR 2026-05-11-2330 D2]
pub struct CatalogueImplSignalsInteractor {
    catalogue_loader: Arc<dyn AttestedCatalogueDocumentLoaderPort>,
    ext_crate_codec: Arc<dyn CatalogueToExtendedCratePort>,
    evaluator: Arc<dyn SignalEvaluatorPort>,
    evaluation_start_capture_port: Arc<dyn EvaluationStartCapturePort>,
    rustdoc_crate_port: Arc<dyn RustdocCratePort>,
    layer_bindings_port: Arc<dyn TdddLayerBindingsPort>,
    feature_declaration_port: Arc<dyn TdddActualFeatureDeclarationPort>,
    symlink_guard: Arc<dyn SymlinkGuardPort>,
}

impl CatalogueImplSignalsInteractor {
    /// Creates a new interactor with the given injected ports.
    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub fn new(
        catalogue_loader: Arc<dyn AttestedCatalogueDocumentLoaderPort>,
        ext_crate_codec: Arc<dyn CatalogueToExtendedCratePort>,
        evaluator: Arc<dyn SignalEvaluatorPort>,
        evaluation_start_capture_port: Arc<dyn EvaluationStartCapturePort>,
        rustdoc_crate_port: Arc<dyn RustdocCratePort>,
        layer_bindings_port: Arc<dyn TdddLayerBindingsPort>,
        feature_declaration_port: Arc<dyn TdddActualFeatureDeclarationPort>,
        symlink_guard: Arc<dyn SymlinkGuardPort>,
    ) -> Self {
        Self {
            catalogue_loader,
            ext_crate_codec,
            evaluator,
            evaluation_start_capture_port,
            rustdoc_crate_port,
            layer_bindings_port,
            feature_declaration_port,
            symlink_guard,
        }
    }

    fn validate_catalogue_set(
        &self,
        track_dir: &Path,
        items_dir: &Path,
        declaration_bindings: &[domain::tddd::catalogue_v2::TdddLayerBinding],
        attested_catalogues: &BTreeMap<LayerId, AttestedCatalogueDocument>,
    ) -> Result<(), CatalogueImplSignalsError> {
        for catalogue_binding in declaration_bindings {
            let catalogue_layer =
                LayerId::try_new(catalogue_binding.layer_id.clone()).map_err(|error| {
                    CatalogueImplSignalsError::LayerBindingsLoad(diagnostic(format!(
                        "invalid layer binding: {error}"
                    )))
                })?;
            validate_binding_filename(&catalogue_binding.catalogue_file, "catalogue_file")?;
            let catalogue_path = track_dir.join(&catalogue_binding.catalogue_file);
            self.symlink_guard
                .reject_symlinks_below(&catalogue_path, items_dir)
                .map_err(map_symlink_guard_error)?;

            match self.catalogue_loader.load(&catalogue_path) {
                Ok(current) => {
                    let Some(expected) = attested_catalogues.get(&catalogue_layer) else {
                        return Err(CatalogueImplSignalsError::CatalogueLoad(
                            catalogue_layer,
                            diagnostic(format!(
                                "catalogue presence changed while validating the stable set at '{}'",
                                catalogue_path.display()
                            )),
                        ));
                    };
                    if current.declaration_hash() != expected.declaration_hash() {
                        return Err(CatalogueImplSignalsError::CatalogueLoad(
                            catalogue_layer,
                            diagnostic(format!(
                                "catalogue declaration hash changed while validating the stable set at '{}'",
                                catalogue_path.display()
                            )),
                        ));
                    }
                }
                Err(CatalogueDocumentLoaderError::NotFound { .. }) => {
                    if attested_catalogues.contains_key(&catalogue_layer) {
                        return Err(CatalogueImplSignalsError::CatalogueLoad(
                            catalogue_layer,
                            diagnostic(format!(
                                "catalogue presence changed while validating the stable set at '{}'",
                                catalogue_path.display()
                            )),
                        ));
                    }
                }
                Err(error) => {
                    return Err(CatalogueImplSignalsError::CatalogueLoad(
                        catalogue_layer,
                        diagnostic(error.to_string()),
                    ));
                }
            }
        }
        Ok(())
    }
}

impl CatalogueImplSignalsService for CatalogueImplSignalsInteractor {
    /// Runs the catalogue-impl-signals evaluation.
    ///
    /// For each TDDD-enabled layer (or the single layer specified by `layer`):
    ///
    /// 1. Load every TDDD-enabled `<layer>-types.json` via
    ///    `AttestedCatalogueDocumentLoaderPort`.
    /// 2. Load each configured layer's `<layer>-types-baseline.json` (B) and
    ///    current TypeGraph (C) through `RustdocCratePort`, retaining the
    ///    pairs in a LayerId-keyed context map.
    /// 3. Convert each selected catalogue to `ExtendedCrate` (A) via
    ///    `CatalogueToExtendedCratePort`, passing the complete catalogue and
    ///    rustdoc context sets.
    /// 4. Run `SignalEvaluatorPort::evaluate(A, B, C)`.
    /// 5. Format the human-readable markdown report section.
    ///
    /// The track items directory is derived from `workspace_root` as
    /// `workspace_root/track/items`.
    ///
    /// Returns the assembled report as a `String` (no file writes, no `println!`).
    ///
    /// ## Layer-bindings contract
    ///
    /// The interactor trusts the bindings returned by [`TdddLayerBindingsPort::load`].
    /// Concrete implementations of that port (e.g. `FsTdddLayerBindingsAdapter`) may
    /// return a synthetic fallback binding when `architecture-rules.json` is absent;
    /// that is a port-implementation policy and not a concern of this interactor.
    /// Callers that require strict fail-closed behaviour when the rules file is absent
    /// should choose a port implementation that returns a `LoadFailed` error
    /// in that case.
    ///
    /// # Errors
    ///
    /// Returns [`CatalogueImplSignalsError`] on any failure.
    fn run(
        &self,
        track_id: String,
        workspace_root: PathBuf,
        layer: Option<String>,
    ) -> Result<CatalogueImplSignalsReport, CatalogueImplSignalsError> {
        // Validate track_id format (simple slug check, mirroring domain logic).
        validate_track_id(&track_id)?;

        // Security: guard workspace_root against symlinks in the entire path and
        // against dot-dot path traversal.
        //
        // (1) Dot-dot rejection: a caller can pass `../repo` as workspace_root, which
        //     would make `join("track/items")` resolve outside the intended directory.
        //     We reject any `..` component before any I/O.
        //
        // (2) Symlink walk: checking only the leaf component is insufficient because a
        //     symlink in an ancestor (e.g. `/home/<user>/proj` where `user` is a symlink)
        //     would redirect all I/O.  We walk every ancestor from the filesystem root
        //     and reject any that is a symlink via the injected `SymlinkGuardPort`.
        for component in workspace_root.components() {
            use std::path::Component;
            if matches!(component, Component::ParentDir) {
                return Err(CatalogueImplSignalsError::SymlinkRejected(workspace_root.clone()));
            }
        }
        self.symlink_guard
            .reject_symlinks_from_root(&workspace_root)
            .map_err(map_symlink_guard_error)?;

        // Derive items directory from workspace root (convention: workspace_root/track/items).
        // Security: check the full path from the filesystem root down to items_dir
        // (inclusive) before any port I/O.  This catches symlinks in the intermediate
        // `workspace_root/track` directory as well as `items_dir` itself.
        // `reject_symlinks_from_root` already checked workspace_root above,
        // but the components added by `.join("track").join("items")` are new and
        // must also be free of symlinks before items_dir becomes the trusted anchor.
        let items_dir = workspace_root.join("track").join("items");
        self.symlink_guard
            .reject_symlinks_from_root(&items_dir)
            .map_err(map_symlink_guard_error)?;
        let track_dir = items_dir.join(&track_id);

        // Resolve the complete layer-binding snapshot once via the injected port
        // (no std::fs in usecase). A filtered load followed by an unfiltered load
        // could combine two generations of architecture-rules.json.
        let declaration_bindings =
            self.layer_bindings_port.load(&workspace_root, None).map_err(|e| match e {
                domain::tddd::catalogue_v2::TdddLayerBindingsError::LoadFailed { reason } => {
                    CatalogueImplSignalsError::LayerBindingsLoad(diagnostic(reason))
                }
                domain::tddd::catalogue_v2::TdddLayerBindingsError::LayerNotFound { layer_id } => {
                    CatalogueImplSignalsError::LayerBindingsLoad(diagnostic(format!(
                        "layer '{layer_id}' not found or not tddd.enabled in \
                             architecture-rules.json"
                    )))
                }
                domain::tddd::catalogue_v2::TdddLayerBindingsError::NoLayers => {
                    CatalogueImplSignalsError::NoLayers
                }
            })?;

        if declaration_bindings.is_empty() {
            return Err(CatalogueImplSignalsError::NoLayers);
        }

        let bindings = if let Some(layer_id) = layer.as_deref() {
            let selected = declaration_bindings
                .iter()
                .filter(|binding| binding.layer_id == layer_id)
                .cloned()
                .collect::<Vec<_>>();
            if selected.is_empty() {
                return Err(CatalogueImplSignalsError::LayerBindingsLoad(diagnostic(format!(
                    "layer '{layer_id}' not found or not tddd.enabled in \
                         architecture-rules.json"
                ))));
            }
            selected
        } else {
            declaration_bindings.clone()
        };
        let declaration = self
            .feature_declaration_port
            .load_for_actual(&track_dir, &workspace_root, &declaration_bindings)
            .map_err(CatalogueImplSignalsError::FeatureDeclaration)?;

        // Load every TDDD-enabled catalogue against one architecture-rules
        // binding snapshot before converting any document. The codec needs
        // declarations from other layers to resolve cross-crate add references.
        let mut attested_catalogues = BTreeMap::<LayerId, AttestedCatalogueDocument>::new();
        for catalogue_binding in &declaration_bindings {
            let catalogue_layer =
                LayerId::try_new(catalogue_binding.layer_id.clone()).map_err(|error| {
                    CatalogueImplSignalsError::LayerBindingsLoad(diagnostic(format!(
                        "invalid layer binding: {error}"
                    )))
                })?;
            validate_binding_filename(&catalogue_binding.catalogue_file, "catalogue_file")?;
            let catalogue_path = track_dir.join(&catalogue_binding.catalogue_file);
            self.symlink_guard
                .reject_symlinks_below(&catalogue_path, &items_dir)
                .map_err(map_symlink_guard_error)?;
            match self.catalogue_loader.load(&catalogue_path) {
                Ok(attested_catalogue) => {
                    attested_catalogues.insert(catalogue_layer, attested_catalogue);
                }
                Err(CatalogueDocumentLoaderError::NotFound { .. }) => continue,
                Err(error) => {
                    return Err(CatalogueImplSignalsError::CatalogueLoad(
                        catalogue_layer.clone(),
                        diagnostic(error.to_string()),
                    ));
                }
            }
        }

        self.validate_catalogue_set(
            &track_dir,
            &items_dir,
            &declaration_bindings,
            &attested_catalogues,
        )?;

        let mut track_catalogues = BTreeMap::<LayerId, CatalogueDocument>::new();
        for (catalogue_layer, attested_catalogue) in &attested_catalogues {
            let doc = attested_catalogue.document().clone();
            if doc.layer() != catalogue_layer {
                return Err(CatalogueImplSignalsError::CatalogueLoad(
                    catalogue_layer.clone(),
                    diagnostic(format!(
                        "catalogue declares layer '{}' but is bound to layer '{}' in \
                         architecture-rules.json",
                        doc.layer().as_ref(),
                        catalogue_layer.as_ref()
                    )),
                ));
            }
            track_catalogues.insert(catalogue_layer.clone(), doc);
        }

        // Preserve the selected-layer contract: a requested layer still needs
        // its catalogue, while an unfiltered run treats an absent configured
        // layer as contributing no declarations.
        if layer.is_some() {
            for binding in &bindings {
                let typed_layer_id =
                    LayerId::try_new(binding.layer_id.clone()).map_err(|error| {
                        CatalogueImplSignalsError::LayerBindingsLoad(diagnostic(format!(
                            "invalid layer binding: {error}"
                        )))
                    })?;
                if !track_catalogues.contains_key(&typed_layer_id) {
                    return Err(CatalogueImplSignalsError::CatalogueLoad(
                        typed_layer_id,
                        diagnostic("layer catalogue was not loaded into the track catalogue set"),
                    ));
                }
            }
        }

        let evaluation_start = self
            .evaluation_start_capture_port
            .capture_evaluation_start()
            .map_err(CatalogueImplSignalsError::EvaluationStartCapture)?;

        let export_plan = RustdocExportPlan::try_new(
            evaluation_start,
            &bindings,
            &declaration_bindings,
            &attested_catalogues,
        )?;

        // Load every configured layer's authoritative rustdoc pair once before
        // any catalogue is encoded. The codec needs the declaring layer's
        // current paths when it places cross-layer add declarations.
        let mut rustdoc_contexts = BTreeMap::new();
        for binding in export_plan.export_bindings() {
            let typed_layer_id = LayerId::try_new(binding.layer_id.clone()).map_err(|error| {
                CatalogueImplSignalsError::LayerBindingsLoad(diagnostic(format!(
                    "invalid layer binding: {error}"
                )))
            })?;

            validate_binding_filename(&binding.baseline_file, "baseline_file")?;
            let baseline_path = track_dir.join(&binding.baseline_file);
            self.symlink_guard
                .reject_symlinks_below(&baseline_path, &items_dir)
                .map_err(map_symlink_guard_error)?;
            let baseline = self.rustdoc_crate_port.load_from_path(&baseline_path).map_err(|e| {
                CatalogueImplSignalsError::BaselineLoad(
                    typed_layer_id.clone(),
                    diagnostic(e.to_string()),
                )
            })?;

            let target_crate = match binding.targets.as_slice() {
                [single] => CrateName::new(single.clone()).map_err(|error| {
                    CatalogueImplSignalsError::SchemaExport(
                        typed_layer_id.clone(),
                        diagnostic(format!("invalid schema_export target: {error}")),
                    )
                })?,
                [] => {
                    return Err(CatalogueImplSignalsError::SchemaExport(
                        typed_layer_id.clone(),
                        diagnostic("schema_export.targets is empty"),
                    ));
                }
                _ => {
                    return Err(CatalogueImplSignalsError::SchemaExport(
                        typed_layer_id.clone(),
                        diagnostic(format!(
                            "layer has {} schema_export.targets; only single-target layers \
                             are supported (multi-crate aggregation requires port extension)",
                            binding.targets.len()
                        )),
                    ));
                }
            };

            let features = declaration.features_for(&typed_layer_id).map_err(|error| {
                CatalogueImplSignalsError::SchemaExport(
                    typed_layer_id.clone(),
                    diagnostic(format!("feature declaration omitted layer: {error}")),
                )
            })?;
            let current = self
                .rustdoc_crate_port
                .capture_current(&target_crate, features, export_plan.implementation_fingerprint())
                .map_err(|e| {
                    CatalogueImplSignalsError::SchemaExport(
                        typed_layer_id.clone(),
                        diagnostic(e.to_string()),
                    )
                })?;
            if !export_plan.snapshot_matches_evaluation_start(&current) {
                return Err(CatalogueImplSignalsError::SchemaExport(
                    typed_layer_id,
                    diagnostic(
                        "current rustdoc snapshot fingerprint does not match the evaluation-start fingerprint",
                    ),
                ));
            }

            rustdoc_contexts.insert(
                typed_layer_id.clone(),
                AuthoritativeRustdocContext::new(
                    typed_layer_id,
                    baseline.crate_data().clone(),
                    current,
                ),
            );
        }

        let mut report = String::new();
        let mut total_red: usize = 0;

        for binding in &bindings {
            let layer_id = &binding.layer_id;
            let typed_layer_id = LayerId::try_new(layer_id.clone()).map_err(|error| {
                CatalogueImplSignalsError::LayerBindingsLoad(diagnostic(format!(
                    "invalid layer binding: {error}"
                )))
            })?;

            let rustdoc_context = rustdoc_contexts.get(&typed_layer_id).ok_or_else(|| {
                CatalogueImplSignalsError::BaselineLoad(
                    typed_layer_id.clone(),
                    diagnostic("authoritative rustdoc context was not assembled"),
                )
            })?;
            let baseline_b = rustdoc_context.baseline().clone();
            let current_c = rustdoc_context.current().clone();

            // --- Step 3: Convert CatalogueDocument → ExtendedCrate (A) ---
            let extended_a = self
                .ext_crate_codec
                .encode(&typed_layer_id, &track_catalogues, &rustdoc_contexts)
                .map_err(|e| {
                    CatalogueImplSignalsError::ExtendedCrateConversion(
                        typed_layer_id.clone(),
                        diagnostic(e.to_string()),
                    )
                })?;

            // --- Step 4: Evaluate ---
            let eval_report =
                self.evaluator.evaluate(extended_a, baseline_b, current_c).map_err(|e| {
                    CatalogueImplSignalsError::Evaluation(
                        typed_layer_id.clone(),
                        diagnostic(e.to_string()),
                    )
                })?;

            // --- Step 5: Format the report section ---
            let _ = writeln!(report);
            let _ = writeln!(report, "## Layer: `{layer_id}`");
            let _ = writeln!(report);

            if eval_report.is_empty() {
                let _ = writeln!(report, "All items maintained (no non-skip signals).");
            } else {
                let _ = writeln!(report, "| Item | Region | Signal |");
                let _ = writeln!(report, "|------|--------|--------|");
                for signal in eval_report.iter() {
                    let kind_str = match signal.signal() {
                        ThreeWaySignalKind::Blue => "🔵 Blue",
                        ThreeWaySignalKind::Yellow => "🟡 Yellow",
                        ThreeWaySignalKind::Red => "🔴 Red",
                        ThreeWaySignalKind::Skip => "Skip",
                    };
                    let region_str = format!("{:?}", signal.region());
                    let _ = writeln!(
                        report,
                        "| {} | {} | {} |",
                        signal.item_name(),
                        region_str,
                        kind_str
                    );
                }
                let _ = writeln!(report);
                let blue =
                    eval_report.iter().filter(|s: &&ThreeWaySignal| s.signal().is_blue()).count();
                let yellow =
                    eval_report.iter().filter(|s: &&ThreeWaySignal| s.signal().is_yellow()).count();
                let red =
                    eval_report.iter().filter(|s: &&ThreeWaySignal| s.signal().is_red()).count();
                total_red = total_red.saturating_add(red);
                let _ =
                    writeln!(report, "Summary: 🔵 {blue} Blue | 🟡 {yellow} Yellow | 🔴 {red} Red");
            }
        }

        self.validate_catalogue_set(
            &track_dir,
            &items_dir,
            &declaration_bindings,
            &attested_catalogues,
        )?;
        Ok(CatalogueImplSignalsReport { text: report, any_red: total_red > 0 })
    }
}

// ---------------------------------------------------------------------------
// Tests (in a sibling file to keep interactor.rs under the module-size limit)
// ---------------------------------------------------------------------------

#[cfg(test)]
#[path = "interactor_tests.rs"]
mod tests;
