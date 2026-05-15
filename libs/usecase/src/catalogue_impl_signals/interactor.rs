//! `CatalogueImplSignalsInteractor` — implements [`CatalogueImplSignalsService`].
//!
//! Orchestrates per-layer A/B/C TypeGraph fetch, signal evaluator invocation,
//! and region-by-region result formatting.
//!
//! [source: ADR 2026-05-11-2330 §D2, §D3]

use std::fmt::Write as FmtWrite;
use std::path::PathBuf;
use std::sync::Arc;

use domain::SymlinkGuardPort;
use domain::tddd::CatalogueToExtendedCratePort;
use domain::tddd::catalogue_v2::{
    CatalogueDocumentLoaderPort, RustdocCratePort, TdddLayerBindingsPort,
};
use domain::tddd::signal_evaluator::{SignalEvaluatorPort, ThreeWaySignal, ThreeWaySignalKind};

use super::helpers::validate_binding_filename;
use super::service::{
    CatalogueImplSignalsError, CatalogueImplSignalsReport, CatalogueImplSignalsService,
};
use super::validate_track_id;

// ---------------------------------------------------------------------------
// Interactor
// ---------------------------------------------------------------------------

/// Interactor implementing [`CatalogueImplSignalsService`].
///
/// Orchestrates per-layer A/B/C TypeGraph fetch, signal evaluator invocation,
/// and region-by-region result formatting. All I/O is performed via injected
/// ports (no direct infrastructure calls):
/// - `CatalogueDocumentLoaderPort` (A-side catalogue file load)
/// - `CatalogueToExtendedCratePort` (A-side `CatalogueDocument` → `ExtendedCrate`)
/// - `SignalEvaluatorPort` (Phase 1 + Phase 2 evaluation)
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
    catalogue_loader: Arc<dyn CatalogueDocumentLoaderPort>,
    ext_crate_codec: Arc<dyn CatalogueToExtendedCratePort>,
    evaluator: Arc<dyn SignalEvaluatorPort>,
    rustdoc_crate_port: Arc<dyn RustdocCratePort>,
    layer_bindings_port: Arc<dyn TdddLayerBindingsPort>,
    symlink_guard: Arc<dyn SymlinkGuardPort>,
}

impl CatalogueImplSignalsInteractor {
    /// Creates a new interactor with the given injected ports.
    #[must_use]
    pub fn new(
        catalogue_loader: Arc<dyn CatalogueDocumentLoaderPort>,
        ext_crate_codec: Arc<dyn CatalogueToExtendedCratePort>,
        evaluator: Arc<dyn SignalEvaluatorPort>,
        rustdoc_crate_port: Arc<dyn RustdocCratePort>,
        layer_bindings_port: Arc<dyn TdddLayerBindingsPort>,
        symlink_guard: Arc<dyn SymlinkGuardPort>,
    ) -> Self {
        Self {
            catalogue_loader,
            ext_crate_codec,
            evaluator,
            rustdoc_crate_port,
            layer_bindings_port,
            symlink_guard,
        }
    }
}

impl CatalogueImplSignalsService for CatalogueImplSignalsInteractor {
    /// Runs the catalogue-impl-signals evaluation.
    ///
    /// For each TDDD-enabled layer (or the single layer specified by `layer`):
    ///
    /// 1. Load `<layer>-types.json` via `CatalogueDocumentLoaderPort`.
    /// 2. Convert to `ExtendedCrate` (A) via `CatalogueToExtendedCratePort`.
    /// 3. Load `<layer>-types-baseline.json` (B) via `RustdocCratePort::load_from_path`.
    /// 4. Capture current TypeGraph (C) via `RustdocCratePort::capture_current`.
    /// 5. Run `SignalEvaluatorPort::evaluate(A, B, C)`.
    /// 6. Format the human-readable markdown report section.
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
        //     symlink in an ancestor (e.g. `/home/user/proj` where `user` is a symlink)
        //     would redirect all I/O.  We walk every ancestor from the filesystem root
        //     and reject any that is a symlink via the injected `SymlinkGuardPort`.
        for component in workspace_root.components() {
            use std::path::Component;
            if matches!(component, Component::ParentDir) {
                return Err(CatalogueImplSignalsError::SymlinkRejected {
                    path: format!(
                        "workspace_root '{}' contains '..' (path traversal rejected)",
                        workspace_root.display()
                    ),
                });
            }
        }
        self.symlink_guard
            .reject_symlinks_from_root(&workspace_root)
            .map_err(|e| CatalogueImplSignalsError::SymlinkRejected { path: e.to_string() })?;

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
            .map_err(|e| CatalogueImplSignalsError::SymlinkRejected { path: e.to_string() })?;
        let track_dir = items_dir.join(&track_id);

        // Resolve layer bindings via injected port (no std::fs in usecase).
        let bindings = self.layer_bindings_port.load(&workspace_root, layer.as_deref()).map_err(
            |e| match e {
                domain::tddd::catalogue_v2::TdddLayerBindingsError::LoadFailed { reason } => {
                    CatalogueImplSignalsError::LayerBindingsLoad { reason }
                }
                domain::tddd::catalogue_v2::TdddLayerBindingsError::LayerNotFound { layer_id } => {
                    CatalogueImplSignalsError::LayerBindingsLoad {
                        reason: format!(
                            "layer '{layer_id}' not found or not tddd.enabled in \
                             architecture-rules.json"
                        ),
                    }
                }
                domain::tddd::catalogue_v2::TdddLayerBindingsError::NoLayers => {
                    CatalogueImplSignalsError::NoLayers
                }
            },
        )?;

        if bindings.is_empty() {
            return Err(CatalogueImplSignalsError::NoLayers);
        }

        let mut report = String::new();
        let mut total_red: usize = 0;

        for binding in &bindings {
            let layer_id = &binding.layer_id;

            // --- Step 1: Load CatalogueDocument (TypeGraph A source) ---
            // Guard: validate that catalogue_file is a plain filename (no `..` or
            // directory separators) to prevent path traversal from a hostile
            // architecture-rules.json binding, before joining with track_dir.
            validate_binding_filename(&binding.catalogue_file, "catalogue_file")?;
            let catalogue_path = track_dir.join(&binding.catalogue_file);
            // Guard: reject symlinks in any component below items_dir to prevent
            // path traversal via a malicious symlinked track directory.
            self.symlink_guard
                .reject_symlinks_below(&catalogue_path, &items_dir)
                .map_err(|e| CatalogueImplSignalsError::SymlinkRejected { path: e.to_string() })?;
            let doc = self.catalogue_loader.load(&catalogue_path).map_err(|e| {
                CatalogueImplSignalsError::CatalogueLoad {
                    layer_id: layer_id.clone(),
                    reason: e.to_string(),
                }
            })?;

            // --- Step 2: Convert CatalogueDocument → ExtendedCrate (A) ---
            let extended_a = self.ext_crate_codec.encode(doc).map_err(|e| {
                CatalogueImplSignalsError::ExtendedCrateConversion {
                    layer_id: layer_id.clone(),
                    reason: e.to_string(),
                }
            })?;

            // --- Step 3: Load baseline (TypeGraph B) ---
            // Guard: validate that baseline_file is a plain filename (no `..` or
            // directory separators) for the same reason as catalogue_file above.
            validate_binding_filename(&binding.baseline_file, "baseline_file")?;
            let baseline_path = track_dir.join(&binding.baseline_file);
            // Guard: same symlink rejection for the baseline path.
            self.symlink_guard
                .reject_symlinks_below(&baseline_path, &items_dir)
                .map_err(|e| CatalogueImplSignalsError::SymlinkRejected { path: e.to_string() })?;
            let baseline_b =
                self.rustdoc_crate_port.load_from_path(&baseline_path).map_err(|e| {
                    CatalogueImplSignalsError::BaselineLoad {
                        layer_id: layer_id.clone(),
                        reason: e.to_string(),
                    }
                })?;

            // --- Step 4: Capture current TypeGraph (C) ---
            // `schema_export.targets` is a Vec<String>; `capture_current` accepts one
            // crate name.  The signal evaluator operates on a single (A, B, C) tuple,
            // so multi-crate aggregation is not yet part of the port contract.
            // We therefore require exactly one target: empty is an error, and
            // multi-target is an error (fail-closed) until the port supports aggregation.
            let target_crate = match binding.targets.as_slice() {
                [single] => single.as_str(),
                [] => {
                    return Err(CatalogueImplSignalsError::SchemaExport {
                        layer_id: layer_id.clone(),
                        reason: "schema_export.targets is empty".to_owned(),
                    });
                }
                _ => {
                    return Err(CatalogueImplSignalsError::SchemaExport {
                        layer_id: layer_id.clone(),
                        reason: format!(
                            "layer has {} schema_export.targets; only single-target layers \
                             are supported (multi-crate aggregation requires port extension)",
                            binding.targets.len()
                        ),
                    });
                }
            };

            let current_c = self.rustdoc_crate_port.capture_current(target_crate).map_err(|e| {
                CatalogueImplSignalsError::SchemaExport {
                    layer_id: layer_id.clone(),
                    reason: e.to_string(),
                }
            })?;

            // --- Step 5: Evaluate ---
            let eval_report =
                self.evaluator.evaluate(extended_a, baseline_b, current_c).map_err(|e| {
                    CatalogueImplSignalsError::Evaluation {
                        layer_id: layer_id.clone(),
                        reason: e.to_string(),
                    }
                })?;

            // --- Step 6: Format the report section ---
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

        Ok(CatalogueImplSignalsReport { text: report, any_red: total_red > 0 })
    }
}

// ---------------------------------------------------------------------------
// Tests (in a sibling file to keep interactor.rs under the module-size limit)
// ---------------------------------------------------------------------------

#[cfg(test)]
#[path = "interactor_tests.rs"]
mod tests;
