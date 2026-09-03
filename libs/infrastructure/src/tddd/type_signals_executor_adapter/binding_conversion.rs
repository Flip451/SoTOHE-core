//! Conversion of domain TDDD bindings to the infrastructure representation.

use domain::tddd::catalogue_v2::TdddLayerBinding as DomainTdddLayerBinding;
use usecase::git_workflow::DiagnosticText;
use usecase::type_signals::TypeSignalsExecutionError;

use crate::verify::tddd_layers::TdddLayerBinding as InfraTdddLayerBinding;

/// Convert a domain binding to an infra binding for use with the evaluator.
///
/// The infra binding stores private fields only populated via
/// `parse_tddd_layers`; this helper constructs a synthetic binding from the
/// domain fields so the evaluator can be reused without changing its signature.
///
/// # Errors
///
/// Returns `TypeSignalsExecutionError` if the synthetic rules JSON fails to
/// parse (which indicates a logic error in the adapter).
pub(super) fn to_infra_binding(
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
        "version": 2,
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
        TypeSignalsExecutionError::AuthoritativeInput(DiagnosticText::new(format!(
            "synthetic rules JSON failed to parse (logic error in TypeSignalsExecutorAdapter): \
             {e}"
        )))
    })?;
    parsed.pop().ok_or_else(|| {
        TypeSignalsExecutionError::AuthoritativeInput(DiagnosticText::new(
            "synthetic rules JSON produced no layers (logic error in \
             TypeSignalsExecutorAdapter)"
                .to_owned(),
        ))
    })
}
