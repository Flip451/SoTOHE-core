//! `TypeSignalsExecutorAdapter` — infrastructure adapter for `TypeSignalsExecutorPort`.
//!
//! Wraps [`crate::tddd::type_signals_evaluator::execute_type_signals_for_layer`]
//! and bridges the domain [`domain::tddd::catalogue_v2::TdddLayerBinding`] type
//! (public fields) to the infra [`crate::verify::tddd_layers::TdddLayerBinding`]
//! type (private getter methods).

use std::path::Path;

use domain::tddd::catalogue_v2::{
    MissingCataloguePolicy, TdddLayerBinding as DomainTdddLayerBinding, TypeSignalsExecutionError,
    TypeSignalsExecutorPort,
};

use crate::tddd::type_signals_evaluator::execute_type_signals_for_layer;
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
/// For `MissingCataloguePolicy::SkipSilently`, inspects the catalogue path
/// before invoking the evaluator and returns `Ok(())` when it is absent.
/// For `MissingCataloguePolicy::FailClosed`, propagates errors from the
/// evaluator directly.
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
    /// For `SkipSilently`: if the catalogue file is not a regular file at
    /// the expected path, returns `Ok(())` (mirrors `execute_type_signals_lenient`
    /// NotFound branch in CLI `signals.rs`).
    ///
    /// For `FailClosed`: delegates directly to `execute_type_signals_for_layer`
    /// and propagates all errors.
    ///
    /// Multi-target bindings (`targets.len() > 1`) are silently skipped in
    /// `SkipSilently` mode (mirrors the pre-commit skip in `signals.rs:228-230`)
    /// and return an error in `FailClosed` mode.
    ///
    /// # Errors
    ///
    /// Returns [`TypeSignalsExecutionError`] on any evaluation failure,
    /// except when `policy` is `SkipSilently` and the catalogue is absent or
    /// the binding has multiple targets.
    fn evaluate_layer(
        &self,
        items_dir: &Path,
        track_id: &str,
        workspace_root: &Path,
        binding: &DomainTdddLayerBinding,
        policy: MissingCataloguePolicy,
    ) -> Result<(), TypeSignalsExecutionError> {
        // Security: validate items_dir first, before any policy-dependent or
        // binding-dependent early-exit paths.  This ensures a symlinked items_dir
        // is rejected regardless of policy (SkipSilently multi-target, missing
        // catalogue, etc.) and regardless of the binding contents.  Mirrors the
        // identical check in `execute_type_signals_for_layer`.
        match items_dir.symlink_metadata() {
            Ok(meta) if meta.file_type().is_symlink() => {
                return Err(TypeSignalsExecutionError(format!(
                    "symlink guard: refusing to use symlinked items_dir: {}",
                    items_dir.display()
                )));
            }
            Ok(_) => {}
            Err(e) => {
                return Err(TypeSignalsExecutionError(format!(
                    "symlink guard: cannot stat items_dir '{}': {e}",
                    items_dir.display()
                )));
            }
        }

        // Perform input validation and binding conversion before any early-exit
        // paths so that malformed requests fail closed regardless of policy or
        // catalogue presence.
        //
        // Validate track_id via the domain newtype before joining onto items_dir.
        // `Path::join` resolves `..`, `/`, and multi-segment paths at the OS level.
        // Using `TrackId::try_new` enforces the slug rules (single-segment, no `..`,
        // no path separators) so that SkipSilently's catalogue preflight cannot be
        // bypassed via path-traversal track IDs (e.g. `../bad`).
        let valid_track_id = domain::TrackId::try_new(track_id).map_err(|e| {
            TypeSignalsExecutionError(format!("invalid track_id '{track_id}': {e}"))
        })?;

        // Reject empty targets: a binding with no targets is always malformed.
        if binding.targets.is_empty() {
            return Err(TypeSignalsExecutionError(format!(
                "layer '{}': schema_export.targets is empty — at least one target is required",
                binding.layer_id,
            )));
        }

        // Multi-target bindings are not yet supported by the strict evaluator.
        if binding.targets.len() > 1 {
            if policy == MissingCataloguePolicy::SkipSilently {
                // Pre-commit: skip unsupported multi-target configs.
                return Ok(());
            }
            return Err(TypeSignalsExecutionError(format!(
                "layer '{}' has {} schema_export.targets — multi-target not yet supported",
                binding.layer_id,
                binding.targets.len()
            )));
        }

        // Convert the domain binding to an infra binding upfront so that any
        // structural errors (e.g. invalid layer_id, empty targets) are detected
        // before the catalogue-presence check.  This ensures that a missing
        // catalogue cannot mask a malformed binding in SkipSilently mode.
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

        // In SkipSilently mode, check whether the catalogue file is present
        // before invoking the (expensive) rustdoc evaluator.
        if policy == MissingCataloguePolicy::SkipSilently {
            // Security: reject a symlinked track directory before treating a
            // missing catalogue as "silently skippable".  Without this check a
            // symlinked track directory whose target lacks the catalogue file
            // would be silently accepted instead of reaching the strict
            // reject_symlinks_below guard inside `execute_type_signals_for_layer`.
            let track_dir = items_dir.join(valid_track_id.as_ref());
            match track_dir.symlink_metadata() {
                Ok(meta) if meta.file_type().is_symlink() => {
                    return Err(TypeSignalsExecutionError(format!(
                        "symlink guard: refusing to follow symlinked track directory: {}",
                        track_dir.display()
                    )));
                }
                Ok(_) | Err(_) => {
                    // Directory is real (or absent) — continue to catalogue check.
                }
            }

            let catalogue_path = track_dir.join(&binding.catalogue_file);
            match std::fs::symlink_metadata(&catalogue_path) {
                Ok(meta) if meta.file_type().is_file() => {
                    // File exists and is a regular file — proceed to evaluation.
                }
                Ok(_) => {
                    // Non-file (symlink, directory, etc.) — delegate to the
                    // strict evaluator so the caller sees the same error as CI.
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    // Genuinely absent — skip silently.
                    return Ok(());
                }
                Err(e) => {
                    return Err(TypeSignalsExecutionError(format!(
                        "pre-commit: cannot stat {}: {e}",
                        catalogue_path.display()
                    )));
                }
            }
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
    fn test_evaluate_layer_skip_silently_absent_catalogue_returns_ok() {
        let dir = tempfile::tempdir().unwrap();
        let items_dir = dir.path().join("track/items");
        let track_dir = items_dir.join("my-track");
        std::fs::create_dir_all(&track_dir).unwrap();
        // No catalogue file written

        let adapter = TypeSignalsExecutorAdapter::new();
        let result = adapter.evaluate_layer(
            &items_dir,
            "my-track",
            dir.path(),
            &domain_binding("domain"),
            MissingCataloguePolicy::SkipSilently,
        );
        assert!(result.is_ok(), "absent catalogue in SkipSilently mode must return Ok");
    }

    #[test]
    fn test_evaluate_layer_multi_target_skip_silently_returns_ok() {
        let dir = tempfile::tempdir().unwrap();
        let items_dir = dir.path().join("track/items");
        std::fs::create_dir_all(&items_dir).unwrap();

        let multi_binding = DomainTdddLayerBinding {
            layer_id: "my-layer".to_owned(),
            catalogue_file: "my-layer-types.json".to_owned(),
            baseline_file: "my-layer-types-baseline.json".to_owned(),
            targets: vec!["crate-a".to_owned(), "crate-b".to_owned()],
        };

        let adapter = TypeSignalsExecutorAdapter::new();
        let result = adapter.evaluate_layer(
            &items_dir,
            "my-track",
            dir.path(),
            &multi_binding,
            MissingCataloguePolicy::SkipSilently,
        );
        assert!(result.is_ok(), "multi-target in SkipSilently mode must skip silently");
    }

    #[test]
    fn test_evaluate_layer_multi_target_fail_closed_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let items_dir = dir.path().join("track/items");
        std::fs::create_dir_all(&items_dir).unwrap();

        let multi_binding = DomainTdddLayerBinding {
            layer_id: "my-layer".to_owned(),
            catalogue_file: "my-layer-types.json".to_owned(),
            baseline_file: "my-layer-types-baseline.json".to_owned(),
            targets: vec!["crate-a".to_owned(), "crate-b".to_owned()],
        };

        let adapter = TypeSignalsExecutorAdapter::new();
        let result = adapter.evaluate_layer(
            &items_dir,
            "my-track",
            dir.path(),
            &multi_binding,
            MissingCataloguePolicy::FailClosed,
        );
        assert!(result.is_err(), "multi-target in FailClosed mode must return error");
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("multi-target"), "error must mention multi-target, got: {msg}");
    }
}
