//! `TypeSignalsExecutorAdapter` — infrastructure adapter for `TypeSignalsExecutorPort`.
//!
//! Wraps [`crate::tddd::type_signals_evaluator::execute_type_signals_for_layer`]
//! and bridges the domain [`domain::tddd::catalogue_v2::TdddLayerBinding`] type
//! (public fields) to the infra [`crate::verify::tddd_layers::TdddLayerBinding`]
//! type (private getter methods).

use std::path::Path;

use domain::TrackId;
use domain::tddd::catalogue_v2::TdddLayerBinding as DomainTdddLayerBinding;
use usecase::type_signals::{TypeSignalsExecutionError, TypeSignalsExecutorPort};

#[cfg(feature = "test-helpers")]
use crate::tddd::type_signals_evaluator::{
    RustdocLaunchObserver, execute_type_signals_for_layer_with_launch_observer,
};
use crate::tddd::type_signals_evaluator::{
    execute_type_signals_for_layer, reject_symlinked_type_signals_anchor,
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
/// Every catalogue input is evaluated conservatively. Missing or unverifiable
/// inputs fail closed; multi-target catalogues return an error because they are
/// not yet supported.
#[derive(Debug, Default)]
pub struct TypeSignalsExecutorAdapter {
    #[cfg(feature = "test-helpers")]
    launch_observer: Option<RustdocLaunchObserver>,
}

impl TypeSignalsExecutorAdapter {
    /// Creates a new adapter instance.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Create an adapter wired to a test-only rustdoc launch observer.
    #[cfg(feature = "test-helpers")]
    #[must_use]
    pub fn with_rustdoc_launch_observer(launch_observer: RustdocLaunchObserver) -> Self {
        Self { launch_observer: Some(launch_observer) }
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
    /// Missing catalogue inputs are errors rather than reuse skips. A
    /// multi-target catalogue returns an error (not yet supported, CN-02).
    ///
    /// # Errors
    ///
    /// Returns [`TypeSignalsExecutionError`] on any evaluation failure.
    fn evaluate_layer(
        &self,
        items_dir: &Path,
        track_id: &TrackId,
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
        let valid_track_id = track_id;

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
        if binding.targets.len() > 1 {
            return Err(TypeSignalsExecutionError(format!(
                "layer '{}' has {} schema_export.targets — multi-target not yet supported",
                binding.layer_id,
                binding.targets.len()
            )));
        }

        #[cfg(feature = "test-helpers")]
        let execution = match &self.launch_observer {
            Some(observer) => execute_type_signals_for_layer_with_launch_observer(
                items_dir,
                valid_track_id,
                workspace_root,
                &infra_binding,
                observer,
            ),
            None => execute_type_signals_for_layer(
                items_dir,
                valid_track_id,
                workspace_root,
                &infra_binding,
            ),
        };
        #[cfg(not(feature = "test-helpers"))]
        let execution = execute_type_signals_for_layer(
            items_dir,
            valid_track_id,
            workspace_root,
            &infra_binding,
        );

        execution.map(|_exit| ()).map_err(|e| TypeSignalsExecutionError(e.0))
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

    fn track_id() -> TrackId {
        TrackId::try_new("my-track").unwrap()
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
    fn test_evaluate_layer_absent_catalogue_returns_error() {
        // Missing catalogue leaves a freshness input unknown and must fail closed.
        let dir = tempfile::tempdir().unwrap();
        let items_dir = dir.path().join("track/items");
        let track_dir = items_dir.join("my-track");
        std::fs::create_dir_all(&track_dir).unwrap();
        // No catalogue file written

        let adapter = TypeSignalsExecutorAdapter::new();
        let result =
            adapter.evaluate_layer(&items_dir, &track_id(), dir.path(), &domain_binding("domain"));
        assert!(result.is_err(), "absent catalogue must fail closed");
    }

    #[test]
    fn test_evaluate_layer_rejects_items_dir_outside_workspace() {
        let workspace = tempfile::tempdir().unwrap();
        let external = tempfile::tempdir().unwrap();
        let items_dir = external.path().join("track/items");
        std::fs::create_dir_all(items_dir.join("my-track")).unwrap();

        let result = TypeSignalsExecutorAdapter::new().evaluate_layer(
            &items_dir,
            &track_id(),
            workspace.path(),
            &domain_binding("domain"),
        );

        assert!(
            matches!(&result, Err(error) if error.0.contains("resolves outside workspace_root")),
            "items_dir outside the workspace must be rejected before reads: {result:?}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_evaluate_layer_rejects_symlinked_catalogue_before_read() {
        let workspace = tempfile::tempdir().unwrap();
        let items_dir = workspace.path().join("track/items");
        let track_dir = items_dir.join("my-track");
        std::fs::create_dir_all(&track_dir).unwrap();
        let outside_catalogue = workspace.path().join("outside-types.json");
        std::fs::write(&outside_catalogue, "{}").unwrap();
        std::os::unix::fs::symlink(&outside_catalogue, track_dir.join("domain-types.json"))
            .unwrap();

        let result = TypeSignalsExecutorAdapter::new().evaluate_layer(
            &items_dir,
            &track_id(),
            workspace.path(),
            &domain_binding("domain"),
        );

        assert!(
            matches!(&result, Err(error) if error.0.contains("symlink guard rejected catalogue")),
            "a symlinked catalogue must be rejected before read: {result:?}"
        );
    }

    #[test]
    fn test_evaluate_layer_multi_target_absent_catalogue_returns_error() {
        // Multi-target evaluation is unsupported even if the catalogue is absent.
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
        let result = adapter.evaluate_layer(&items_dir, &track_id(), dir.path(), &multi_binding);
        assert!(result.is_err(), "multi-target + absent catalogue must fail closed");
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
        let result = adapter.evaluate_layer(&items_dir, &track_id(), dir.path(), &multi_binding);
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
            adapter.evaluate_layer(&items_dir, &track_id(), dir.path(), &domain_binding("domain"));
        assert!(
            result.is_err(),
            "missing track dir must return Err (fail-closed per ADR 2026-06-01-0406 D1)"
        );
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("cannot read catalogue"),
            "error must identify the missing input, got: {msg}"
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
        let result = adapter.evaluate_layer(&items_dir, &track_id(), dir.path(), &multi_binding);
        assert!(
            result.is_err(),
            "multi-target + missing track dir must return Err (fail-closed per ADR 2026-06-01-0406 D1)"
        );
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("multi-target"),
            "error must preserve the unsupported-target cause, got: {msg}"
        );
    }
}
