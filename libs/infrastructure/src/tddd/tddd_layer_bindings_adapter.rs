//! `FsTdddLayerBindingsAdapter` — infrastructure adapter for `TdddLayerBindingsPort`.
//!
//! Reads `architecture-rules.json` via `reject_symlinks_below` + `parse_tddd_layers`
//! and maps the results to domain `TdddLayerBinding` entries so that `libs/usecase`
//! never calls `std::fs` directly (hexagonal-purity rule).
//!
//! Two construction modes (controlled by the `legacy_fallback` field):
//! - `new()` — fail-closed (no fallback): absent `architecture-rules.json` → `LoadFailed`.
//!   Use for `catalogue-impl-signals` where running against an undeclared layer set
//!   would let stale legacy artifacts pass undetected.
//! - `new_with_legacy_fallback()` — lenient: absent `architecture-rules.json` → synthetic
//!   `domain` binding (same as `load_tddd_layers_from_path`'s pre-multilayer fallback).
//!   Use for `type-signals` and `baseline-capture` so legacy tracks without an
//!   `architecture-rules.json` continue to work.
//!
//! [source: ADR 2026-05-11-2330 §D2]

use std::path::Path;

use domain::tddd::catalogue_v2::{
    TdddLayerBinding as DomainTdddLayerBinding, TdddLayerBindingsError, TdddLayerBindingsPort,
};

use crate::track::symlink_guard::reject_symlinks_below;
use crate::verify::tddd_layers::parse_tddd_layers;

/// Synthetic legacy `architecture-rules.json` content used when the file is absent
/// and `legacy_fallback` is enabled.  Mirrors `load_tddd_layers_from_path`'s fallback.
const LEGACY_FALLBACK_JSON: &str = r#"{"layers":[{"crate":"domain","tddd":{"enabled":true,"catalogue_file":"domain-types.json"}}]}"#;

// ---------------------------------------------------------------------------
// FsTdddLayerBindingsAdapter
// ---------------------------------------------------------------------------

/// Filesystem adapter implementing [`TdddLayerBindingsPort`].
///
/// Reads `architecture-rules.json` directly (via symlink guard + `parse_tddd_layers`)
/// and maps the result to domain `TdddLayerBinding` entries.
///
/// Constructed with [`new`](Self::new) (fail-closed, for `catalogue-impl-signals`) or
/// [`new_with_legacy_fallback`](Self::new_with_legacy_fallback) (lenient, for
/// `type-signals` / `baseline-capture`). See module-level docs for the distinction.
///
/// [source: ADR 2026-05-11-2330 D2]
#[derive(Debug, Clone)]
pub struct FsTdddLayerBindingsAdapter {
    /// When `true`, an absent `architecture-rules.json` returns a synthetic
    /// domain binding (legacy pre-multilayer compatibility).  When `false`,
    /// an absent file is a hard `LoadFailed` error (fail-closed).
    legacy_fallback: bool,
}

impl Default for FsTdddLayerBindingsAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl FsTdddLayerBindingsAdapter {
    /// Creates a fail-closed adapter instance.
    ///
    /// An absent `architecture-rules.json` returns [`TdddLayerBindingsError::LoadFailed`].
    /// Use for commands where a missing rules file is a configuration error (e.g.
    /// `catalogue-impl-signals`).
    #[must_use]
    pub fn new() -> Self {
        Self { legacy_fallback: false }
    }

    /// Creates an adapter with legacy synthetic-domain fallback.
    ///
    /// When `architecture-rules.json` is absent (file not found), returns a single
    /// synthetic `domain` binding (`domain-types.json`) identical to the
    /// `load_tddd_layers_from_path` pre-multilayer fallback.  Use for commands that
    /// must remain compatible with legacy tracks (e.g. `type-signals`, `baseline-capture`).
    #[must_use]
    pub fn new_with_legacy_fallback() -> Self {
        Self { legacy_fallback: true }
    }
}

impl TdddLayerBindingsPort for FsTdddLayerBindingsAdapter {
    /// Loads TDDD-enabled layer bindings from `workspace_root/architecture-rules.json`.
    ///
    /// If `layer_filter` is `Some`, returns only the binding for the given layer id.
    ///
    /// When `architecture-rules.json` is absent and this adapter was constructed with
    /// [`new_with_legacy_fallback`](FsTdddLayerBindingsAdapter::new_with_legacy_fallback),
    /// returns a single synthetic `domain` binding (pre-multilayer compatibility).
    /// When constructed with [`new`](FsTdddLayerBindingsAdapter::new), an absent file
    /// returns [`TdddLayerBindingsError::LoadFailed`] (fail-closed).
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
        // To avoid a TOCTOU race (exists() → deleted → load sees absent → fallback),
        // we bypass `load_tddd_layers_from_path` entirely: run the symlink guard
        // directly, then read the file ourselves.
        let content = match reject_symlinks_below(&rules_path, workspace_root) {
            Ok(true) => {
                // File present and not a symlink — read it.
                std::fs::read_to_string(&rules_path)
                    .map_err(|e| TdddLayerBindingsError::LoadFailed { reason: e.to_string() })?
            }
            Ok(false) => {
                // File genuinely absent.
                if self.legacy_fallback {
                    // Legacy fallback: return synthetic domain binding so pre-multilayer
                    // tracks (without architecture-rules.json) continue to work.
                    LEGACY_FALLBACK_JSON.to_owned()
                } else {
                    // Fail-closed: an absent rules file is a configuration error for
                    // commands like catalogue-impl-signals where running against an
                    // undeclared layer set would let stale artifacts pass undetected.
                    return Err(TdddLayerBindingsError::LoadFailed {
                        reason: format!(
                            "architecture-rules.json not found at {}: TDDD layer bindings cannot \
                             be determined without an explicit rules file (legacy synthetic-domain \
                             fallback is intentionally disabled for this adapter mode)",
                            rules_path.display()
                        ),
                    });
                }
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
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
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

    // --- legacy fallback mode (new_with_legacy_fallback) ---

    #[test]
    fn test_load_absent_arch_rules_with_legacy_fallback_returns_synthetic_domain() {
        // When architecture-rules.json is absent and the adapter is in legacy-fallback
        // mode, a single synthetic domain binding must be returned (mirrors
        // `load_tddd_layers_from_path`'s pre-multilayer fallback).
        let adapter = FsTdddLayerBindingsAdapter::new_with_legacy_fallback();
        let tmp = tempfile::tempdir().unwrap();

        let bindings = adapter.load(tmp.path(), None).unwrap();

        assert_eq!(
            bindings.len(),
            1,
            "expected exactly one synthetic binding, got {}",
            bindings.len()
        );
        assert_eq!(bindings[0].layer_id, "domain", "synthetic binding must have layer_id 'domain'");
        assert_eq!(
            bindings[0].catalogue_file, "domain-types.json",
            "synthetic binding must have catalogue_file 'domain-types.json'"
        );
    }

    #[test]
    fn test_load_absent_arch_rules_legacy_fallback_with_domain_filter_returns_domain() {
        // When architecture-rules.json is absent and a `--layer domain` filter is applied,
        // the synthetic domain binding must match the filter and be returned.
        let adapter = FsTdddLayerBindingsAdapter::new_with_legacy_fallback();
        let tmp = tempfile::tempdir().unwrap();

        let bindings = adapter.load(tmp.path(), Some("domain")).unwrap();

        assert_eq!(bindings.len(), 1, "expected one domain binding");
        assert_eq!(bindings[0].layer_id, "domain");
    }

    #[test]
    fn test_load_absent_arch_rules_legacy_fallback_with_non_domain_filter_returns_layer_not_found()
    {
        // When architecture-rules.json is absent, the synthetic fallback only exposes
        // the `domain` layer. Requesting any other layer must return LayerNotFound.
        let adapter = FsTdddLayerBindingsAdapter::new_with_legacy_fallback();
        let tmp = tempfile::tempdir().unwrap();

        let err = adapter.load(tmp.path(), Some("usecase")).unwrap_err();
        assert!(
            matches!(err, TdddLayerBindingsError::LayerNotFound { .. }),
            "expected LayerNotFound for non-domain filter on absent rules file, got: {err}"
        );
    }

    #[test]
    fn test_load_malformed_arch_rules_legacy_fallback_still_returns_load_failed() {
        // Even in legacy-fallback mode, a malformed JSON file must return LoadFailed
        // (the fallback only applies when the file is genuinely absent).
        let adapter = FsTdddLayerBindingsAdapter::new_with_legacy_fallback();
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("architecture-rules.json"), b"not valid json").unwrap();

        let err = adapter.load(tmp.path(), None).unwrap_err();
        assert!(
            matches!(err, TdddLayerBindingsError::LoadFailed { .. }),
            "expected LoadFailed for malformed JSON even in legacy-fallback mode, got: {err}"
        );
    }
}
