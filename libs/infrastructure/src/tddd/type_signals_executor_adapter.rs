//! `TypeSignalsExecutorAdapter` — infrastructure adapter for `TypeSignalsExecutorPort`.
//!
//! Wraps [`crate::tddd::type_signals_evaluator::execute_type_signals_for_layer`]
//! and bridges the domain [`domain::tddd::catalogue_v2::TdddLayerBinding`] type
//! (public fields) to the infra [`crate::verify::tddd_layers::TdddLayerBinding`]
//! type (private getter methods).

use std::path::Path;

use domain::tddd::catalogue_v2::{
    TdddLayerBinding as DomainTdddLayerBinding, TypeSignalsExecutionError, TypeSignalsExecutorPort,
};

use crate::tddd::type_signals_evaluator::{
    TypeSignalsCataloguePresence, execute_type_signals_for_layer,
    reject_symlinked_type_signals_anchor, require_type_signals_track_dir,
    type_signals_catalogue_presence, type_signals_track_dir, validate_type_signals_track_id,
};
use crate::verify::tddd_layers::TdddLayerBinding as InfraTdddLayerBinding;

// ---------------------------------------------------------------------------
// Adapter
// ---------------------------------------------------------------------------

/// Stateless adapter implementing [`TypeSignalsExecutorPort`].
///
/// Converts the domain [`DomainTdddLayerBinding`] (public fields) to the infra
/// [`InfraTdddLayerBinding`] (private getters + `signal_file()` method) and
/// delegates to [`execute_type_signals_for_layer`].
///
/// An absent catalogue file is always skipped silently (no-op, returns `Ok(())`).
/// A present catalogue is always evaluated strictly; present multi-target
/// catalogues return an error (multi-target not yet supported).
#[derive(Debug, Default)]
pub struct TypeSignalsExecutorAdapter;

impl TypeSignalsExecutorAdapter {
    /// Creates a new adapter instance.
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Convert a domain binding to an infra binding for use with the evaluator.
    ///
    /// The infra binding stores private fields only populated via
    /// `parse_tddd_layers`; `TypeSignalsExecutorAdapter` constructs a
    /// synthetic binding from the domain fields so the evaluator can be
    /// reused without changing its signature.
    ///
    /// # Errors
    ///
    /// Returns `TypeSignalsExecutionError` if the synthetic rules JSON fails to
    /// parse (should not happen in practice — indicates a logic error).
    fn to_infra_binding(
        b: &DomainTdddLayerBinding,
    ) -> Result<InfraTdddLayerBinding, TypeSignalsExecutionError> {
        use crate::verify::tddd_layers::parse_tddd_layers;
        // Build a minimal architecture-rules.json snippet that expresses the
        // binding so we can reuse `parse_tddd_layers` for correct construction.
        // `catalogue_spec_signal` is left as the default (not enabled) because
        // `execute_type_signals_for_layer` does not inspect that field.
        //
        // Use `serde_json::json!` to construct the value so that `layer_id`,
        // `catalogue_file`, and each target string are properly JSON-escaped.
        // Raw string interpolation (format!) would produce invalid JSON when any
        // of these strings contain `"` or `\` characters.
        let targets_json_array: serde_json::Value =
            serde_json::Value::Array(b.targets.iter().map(|t| serde_json::json!(t)).collect());
        let rules_value = serde_json::json!({
            "layers": [{
                "crate": b.layer_id,
                "tddd": {
                    "enabled": true,
                    "catalogue_file": b.catalogue_file,
                    "schema_export": {
                        "method": "rustdoc",
                        "targets": targets_json_array
                    }
                }
            }]
        });
        let rules_json = rules_value.to_string();
        // parse_tddd_layers returns at most one entry matching our layer.
        let mut parsed = parse_tddd_layers(&rules_json).map_err(|e| {
            TypeSignalsExecutionError(format!(
                "synthetic rules JSON failed to parse (logic error in TypeSignalsExecutorAdapter): \
                 {e}"
            ))
        })?;
        parsed.pop().ok_or_else(|| {
            TypeSignalsExecutionError(
                "synthetic rules JSON produced no layers (logic error in \
                 TypeSignalsExecutorAdapter)"
                    .to_owned(),
            )
        })
    }
}

impl TypeSignalsExecutorPort for TypeSignalsExecutorAdapter {
    /// Evaluates type signals for one layer binding.
    ///
    /// An absent catalogue file is always skipped silently (returns `Ok(())`).
    /// A present catalogue is always evaluated strictly. A present multi-target
    /// catalogue returns an error (multi-target not yet supported, CN-02).
    ///
    /// # Errors
    ///
    /// Returns [`TypeSignalsExecutionError`] on any evaluation failure.
    /// An absent catalogue file always returns `Ok(())` (unconditional skip).
    fn evaluate_layer(
        &self,
        items_dir: &Path,
        track_id: &str,
        workspace_root: &Path,
        binding: &DomainTdddLayerBinding,
    ) -> Result<(), TypeSignalsExecutionError> {
        // Security: validate items_dir first, before any binding-dependent
        // early-exit paths.  This ensures a symlinked items_dir is rejected
        // regardless of catalogue presence or binding contents.  Mirrors the
        // identical check in `execute_type_signals_for_layer`.
        reject_symlinked_type_signals_anchor(items_dir, "items_dir")
            .map_err(TypeSignalsExecutionError)?;

        // Perform input validation and binding conversion before any early-exit
        // paths so that malformed requests fail closed regardless of catalogue
        // presence.
        //
        // Validate track_id via the domain newtype before joining onto items_dir.
        // `Path::join` resolves `..`, `/`, and multi-segment paths at the OS level.
        // Using `TrackId::try_new` enforces the slug rules (single-segment, no `..`,
        // no path separators) so that the absent-catalogue skip cannot be bypassed
        // via path-traversal track IDs (e.g. `../bad`).
        let valid_track_id =
            validate_type_signals_track_id(track_id).map_err(TypeSignalsExecutionError)?;

        // Reject empty targets: a binding with no targets is always malformed.
        if binding.targets.is_empty() {
            return Err(TypeSignalsExecutionError(format!(
                "layer '{}': schema_export.targets is empty — at least one target is required",
                binding.layer_id,
            )));
        }

        // Convert the domain binding to an infra binding before any early-exit
        // paths so that catalogue_file and layer_id are validated via
        // parse_tddd_layers (is_safe_path_component) regardless of the targets
        // count.  Without this call in the multi-target path, a malformed
        // catalogue_file (e.g. containing path traversal characters) would be
        // passed directly to track_dir.join() without validation.
        let infra_binding = Self::to_infra_binding(binding)?;

        // Verify that the infra binding derives the same baseline_file as the
        // domain binding.  The infra binding computes baseline_file from
        // catalogue_file (stem + "-baseline.json"); if the caller supplied a
        // non-standard baseline path, flag it rather than silently reading the
        // wrong file.
        let derived_baseline = infra_binding.baseline_file();
        if derived_baseline != binding.baseline_file {
            return Err(TypeSignalsExecutionError(format!(
                "layer '{}': domain baseline_file '{}' differs from the infra-derived \
                 baseline_file '{}' (derived from catalogue_file '{}'); supply a standard \
                 baseline path or adjust catalogue_file",
                binding.layer_id, binding.baseline_file, derived_baseline, binding.catalogue_file,
            )));
        }

        // Multi-target bindings are not yet supported by the strict evaluator.
        // Skip silently only when the catalogue is absent (CN-02: present catalogues
        // are always evaluated; no fail-open).
        if binding.targets.len() > 1 {
            // Apply symlink guard on track directory before catalogue-presence check.
            // ADR 2026-06-01-0406 D1: the absent-catalogue skip is scoped to "track dir
            // exists AND catalogue file is missing". A missing track dir is a structural
            // anomaly and must fail-closed (not be treated as "absent catalogue").
            let track_dir = type_signals_track_dir(items_dir, &valid_track_id);
            require_type_signals_track_dir(&track_dir).map_err(TypeSignalsExecutionError)?;

            let catalogue_path = track_dir.join(infra_binding.catalogue_file());
            if type_signals_catalogue_presence(&catalogue_path)
                .map_err(TypeSignalsExecutionError)?
                == TypeSignalsCataloguePresence::Absent
            {
                return Ok(());
            }

            return Err(TypeSignalsExecutionError(format!(
                "layer '{}' has {} schema_export.targets — multi-target not yet supported",
                binding.layer_id,
                binding.targets.len()
            )));
        }

        // Check whether the catalogue file is present before invoking the
        // (expensive) rustdoc evaluator. Absent catalogue is always skipped
        // unconditionally (no gate-vs-direct distinction per ADR D1).
        //
        // Security: reject a symlinked track directory before treating a
        // missing catalogue as skippable.  Without this check a symlinked
        // track directory whose target lacks the catalogue file would be
        // silently accepted instead of reaching the strict reject_symlinks_below
        // guard inside `execute_type_signals_for_layer`.
        //
        // ADR 2026-06-01-0406 D1: the absent-catalogue skip is scoped to "track dir
        // exists AND catalogue file is missing". A missing track dir is a structural
        // anomaly and must fail-closed (not be treated as "absent catalogue").
        let track_dir = type_signals_track_dir(items_dir, &valid_track_id);
        require_type_signals_track_dir(&track_dir).map_err(TypeSignalsExecutionError)?;

        let catalogue_path = track_dir.join(infra_binding.catalogue_file());
        if type_signals_catalogue_presence(&catalogue_path).map_err(TypeSignalsExecutionError)?
            == TypeSignalsCataloguePresence::Absent
        {
            return Ok(());
        }

        execute_type_signals_for_layer(
            items_dir,
            valid_track_id.as_ref(),
            workspace_root,
            &infra_binding,
        )
        .map(|_exit| ())
        .map_err(|e| TypeSignalsExecutionError(e.0))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn domain_binding(layer_id: &str) -> DomainTdddLayerBinding {
        DomainTdddLayerBinding {
            layer_id: layer_id.to_owned(),
            catalogue_file: format!("{layer_id}-types.json"),
            baseline_file: format!("{layer_id}-types-baseline.json"),
            targets: vec![layer_id.to_owned()],
        }
    }

    #[test]
    fn test_to_infra_binding_preserves_layer_id() {
        let domain = domain_binding("domain");
        let infra = TypeSignalsExecutorAdapter::to_infra_binding(&domain).unwrap();
        assert_eq!(infra.layer_id(), "domain");
        assert_eq!(infra.catalogue_file(), "domain-types.json");
        assert_eq!(infra.baseline_file(), "domain-types-baseline.json");
        assert_eq!(infra.targets(), &["domain"]);
    }

    #[test]
    fn test_evaluate_layer_absent_catalogue_returns_ok() {
        // Absent catalogue is always skipped unconditionally.
        let dir = tempfile::tempdir().unwrap();
        let items_dir = dir.path().join("track/items");
        let track_dir = items_dir.join("my-track");
        std::fs::create_dir_all(&track_dir).unwrap();
        // No catalogue file written

        let adapter = TypeSignalsExecutorAdapter::new();
        let result =
            adapter.evaluate_layer(&items_dir, "my-track", dir.path(), &domain_binding("domain"));
        assert!(result.is_ok(), "absent catalogue must always return Ok (unconditional skip)");
    }

    #[test]
    fn test_evaluate_layer_multi_target_absent_catalogue_returns_ok() {
        // Multi-target + absent catalogue => skip silently (CN-02 compliant).
        let dir = tempfile::tempdir().unwrap();
        let items_dir = dir.path().join("track/items");
        let track_dir = items_dir.join("my-track");
        std::fs::create_dir_all(&track_dir).unwrap();
        // Catalogue file intentionally NOT created.

        let multi_binding = DomainTdddLayerBinding {
            layer_id: "my-layer".to_owned(),
            catalogue_file: "my-layer-types.json".to_owned(),
            baseline_file: "my-layer-types-baseline.json".to_owned(),
            targets: vec!["crate-a".to_owned(), "crate-b".to_owned()],
        };

        let adapter = TypeSignalsExecutorAdapter::new();
        let result = adapter.evaluate_layer(&items_dir, "my-track", dir.path(), &multi_binding);
        assert!(
            result.is_ok(),
            "multi-target + absent catalogue must skip silently (unconditional)"
        );
    }

    #[test]
    fn test_evaluate_layer_multi_target_present_catalogue_returns_error() {
        // Multi-target + present catalogue => fail-closed (CN-02: no fail-open).
        let dir = tempfile::tempdir().unwrap();
        let items_dir = dir.path().join("track/items");
        let track_dir = items_dir.join("my-track");
        std::fs::create_dir_all(&track_dir).unwrap();
        // Write a (dummy) catalogue file so the presence check detects it.
        std::fs::write(track_dir.join("my-layer-types.json"), b"{}").unwrap();

        let multi_binding = DomainTdddLayerBinding {
            layer_id: "my-layer".to_owned(),
            catalogue_file: "my-layer-types.json".to_owned(),
            baseline_file: "my-layer-types-baseline.json".to_owned(),
            targets: vec!["crate-a".to_owned(), "crate-b".to_owned()],
        };

        let adapter = TypeSignalsExecutorAdapter::new();
        let result = adapter.evaluate_layer(&items_dir, "my-track", dir.path(), &multi_binding);
        assert!(
            result.is_err(),
            "multi-target + present catalogue must return Err (fail-closed, CN-02)"
        );
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("multi-target"), "error must mention multi-target, got: {msg}");
    }

    #[test]
    fn test_evaluate_layer_missing_track_dir_returns_error() {
        // ADR 2026-06-01-0406 D1: a missing track dir is a structural anomaly and
        // must fail-closed. Only an EXISTING track dir with an absent catalogue file
        // is a sanctioned skip scenario.
        let dir = tempfile::tempdir().unwrap();
        let items_dir = dir.path().join("track/items");
        // Create items_dir but do NOT create the track subdirectory.
        std::fs::create_dir_all(&items_dir).unwrap();
        // track_dir is intentionally absent.

        let adapter = TypeSignalsExecutorAdapter::new();
        let result =
            adapter.evaluate_layer(&items_dir, "my-track", dir.path(), &domain_binding("domain"));
        assert!(
            result.is_err(),
            "missing track dir must return Err (fail-closed per ADR 2026-06-01-0406 D1)"
        );
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("missing or unstattable"),
            "error must mention missing/unstattable track directory, got: {msg}"
        );
    }

    #[test]
    fn test_evaluate_layer_multi_target_missing_track_dir_returns_error() {
        // ADR 2026-06-01-0406 D1: same fail-closed requirement for multi-target bindings.
        // A missing track dir must not be silently treated as "absent catalogue".
        let dir = tempfile::tempdir().unwrap();
        let items_dir = dir.path().join("track/items");
        // Create items_dir but do NOT create the track subdirectory.
        std::fs::create_dir_all(&items_dir).unwrap();
        // track_dir is intentionally absent.

        let multi_binding = DomainTdddLayerBinding {
            layer_id: "my-layer".to_owned(),
            catalogue_file: "my-layer-types.json".to_owned(),
            baseline_file: "my-layer-types-baseline.json".to_owned(),
            targets: vec!["crate-a".to_owned(), "crate-b".to_owned()],
        };

        let adapter = TypeSignalsExecutorAdapter::new();
        let result = adapter.evaluate_layer(&items_dir, "my-track", dir.path(), &multi_binding);
        assert!(
            result.is_err(),
            "multi-target + missing track dir must return Err (fail-closed per ADR 2026-06-01-0406 D1)"
        );
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("missing or unstattable"),
            "error must mention missing/unstattable track directory, got: {msg}"
        );
    }
}
