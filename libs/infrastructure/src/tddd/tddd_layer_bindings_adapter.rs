//! `FsTdddLayerBindingsAdapter` — infrastructure adapter for `TdddLayerBindingsPort`.
//!
//! Reads `architecture-rules.json` via `reject_symlinks_below` + `parse_tddd_layers`
//! (bypasses `load_tddd_layers_from_path`'s legacy synthetic-domain fallback) and
//! maps the results to domain `TdddLayerBinding` entries so that `libs/usecase`
//! never calls `std::fs` directly (hexagonal-purity rule).
//!
//! [source: ADR 2026-05-11-2330 §D2]

use std::path::Path;

use domain::tddd::catalogue_v2::{
    TdddLayerBinding as DomainTdddLayerBinding, TdddLayerBindingsError, TdddLayerBindingsPort,
};

use crate::track::symlink_guard::reject_symlinks_below;
use crate::verify::tddd_layers::parse_tddd_layers;

// ---------------------------------------------------------------------------
// FsTdddLayerBindingsAdapter
// ---------------------------------------------------------------------------

/// Stateless filesystem adapter implementing [`TdddLayerBindingsPort`].
///
/// Reads `architecture-rules.json` directly (via symlink guard + `parse_tddd_layers`)
/// and maps the result to domain `TdddLayerBinding` entries. Injected into
/// `CatalogueImplSignalsInteractor` at the `apps/cli` composition root.
///
/// [source: ADR 2026-05-11-2330 D2]
#[derive(Debug, Clone, Default)]
pub struct FsTdddLayerBindingsAdapter;

impl FsTdddLayerBindingsAdapter {
    /// Creates a new adapter instance.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl TdddLayerBindingsPort for FsTdddLayerBindingsAdapter {
    /// Loads TDDD-enabled layer bindings from `workspace_root/architecture-rules.json`.
    ///
    /// If `layer_filter` is `Some`, returns only the binding for the given layer id.
    ///
    /// # Errors
    ///
    /// Returns [`TdddLayerBindingsError::LoadFailed`] if the file cannot be read or parsed.
    ///
    /// Returns [`TdddLayerBindingsError::LayerNotFound`] if a layer filter was
    /// supplied and no matching enabled layer exists.
    ///
    /// Returns [`TdddLayerBindingsError::NoLayers`] if no `tddd.enabled` layers
    /// were found (and no filter was supplied).
    fn load(
        &self,
        workspace_root: &Path,
        layer_filter: Option<&str>,
    ) -> Result<Vec<DomainTdddLayerBinding>, TdddLayerBindingsError> {
        let rules_path = workspace_root.join("architecture-rules.json");
        // Fail-closed for catalogue-impl-signals: `load_tddd_layers_from_path`
        // returns a legacy synthetic `domain` binding when `architecture-rules.json`
        // is absent (pre-multilayer compatibility). For this port we must NOT fall
        // back to a synthetic binding — running the signal report against an
        // undeclared layer set would let stale legacy artifacts pass while the
        // rules file is genuinely missing.
        //
        // To avoid a TOCTOU race (exists() → deleted → load sees absent → fallback),
        // we bypass `load_tddd_layers_from_path` entirely: run the symlink guard
        // directly, then read the file ourselves. An absent file (Ok(false)) is an
        // explicit configuration error for this port, not a legacy-compat fallback.
        let content = match reject_symlinks_below(&rules_path, workspace_root) {
            Ok(true) => {
                // File present and not a symlink — read it.
                std::fs::read_to_string(&rules_path)
                    .map_err(|e| TdddLayerBindingsError::LoadFailed { reason: e.to_string() })?
            }
            Ok(false) => {
                // File genuinely absent — fail closed (no synthetic fallback).
                return Err(TdddLayerBindingsError::LoadFailed {
                    reason: format!(
                        "architecture-rules.json not found at {}: TDDD layer bindings cannot be \
                         determined without an explicit rules file (legacy synthetic-domain \
                         fallback is intentionally disabled for this port)",
                        rules_path.display()
                    ),
                });
            }
            Err(e) => {
                return Err(TdddLayerBindingsError::LoadFailed { reason: e.to_string() });
            }
        };
        let infra_bindings = parse_tddd_layers(&content)
            .map_err(|e| TdddLayerBindingsError::LoadFailed { reason: e.to_string() })?;

        // Convert infra TdddLayerBinding → domain TdddLayerBinding.
        let mut domain_bindings: Vec<DomainTdddLayerBinding> = infra_bindings
            .into_iter()
            .map(|b| {
                let layer_id = b.layer_id().to_owned();
                let catalogue_file = b.catalogue_file().to_owned();
                let baseline_file = b.baseline_file();
                let targets = b.targets().to_vec();
                DomainTdddLayerBinding { layer_id, catalogue_file, baseline_file, targets }
            })
            .collect();

        // Apply optional layer filter.
        if let Some(filter) = layer_filter {
            domain_bindings.retain(|b| b.layer_id == filter);
            if domain_bindings.is_empty() {
                return Err(TdddLayerBindingsError::LayerNotFound { layer_id: filter.to_owned() });
            }
        }

        if domain_bindings.is_empty() {
            return Err(TdddLayerBindingsError::NoLayers);
        }

        Ok(domain_bindings)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_load_absent_arch_rules_fails_closed() {
        // For catalogue-impl-signals: when architecture-rules.json is absent, the adapter
        // must NOT silently fall back to a synthetic `domain` binding (that would let stale
        // legacy artifacts pass while the rules file is genuinely missing).  Map missing-rules
        // to LoadFailed so the caller surfaces the configuration error.
        let adapter = FsTdddLayerBindingsAdapter::new();
        let tmp = tempfile::tempdir().unwrap();
        let err = adapter.load(tmp.path(), None).unwrap_err();
        assert!(
            matches!(err, TdddLayerBindingsError::LoadFailed { .. }),
            "expected LoadFailed for absent architecture-rules.json, got: {err}"
        );
    }

    #[test]
    fn test_load_malformed_arch_rules_returns_load_failed() {
        // When architecture-rules.json exists but contains invalid JSON, LoadFailed is returned.
        let adapter = FsTdddLayerBindingsAdapter::new();
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("architecture-rules.json"), b"not valid json").unwrap();
        let err = adapter.load(tmp.path(), None).unwrap_err();
        assert!(
            matches!(err, TdddLayerBindingsError::LoadFailed { .. }),
            "expected LoadFailed, got: {err}"
        );
    }

    #[test]
    fn test_load_with_unknown_filter_returns_layer_not_found() {
        let adapter = FsTdddLayerBindingsAdapter::new();
        // Use the real workspace architecture-rules.json (parent of this crate root)
        // so we have at least one valid layer to enumerate.
        let ws =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().parent().unwrap();
        // If the arch rules file exists, try a definitely-absent layer name.
        let rules_path = ws.join("architecture-rules.json");
        if !rules_path.exists() {
            return; // Skip in isolated build environments
        }
        let err = adapter.load(ws, Some("does-not-exist-layer-99")).unwrap_err();
        assert!(
            matches!(err, TdddLayerBindingsError::LayerNotFound { .. }),
            "expected LayerNotFound, got: {err}"
        );
    }

    #[test]
    fn test_fs_tddd_layer_bindings_adapter_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<FsTdddLayerBindingsAdapter>();
    }
}
