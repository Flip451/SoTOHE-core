//! Frozen per-layer Cargo feature selection for type-signal evaluation.

use std::collections::BTreeMap;
use std::path::Path;

use domain::tddd::catalogue_v2::{
    AttestedCatalogueDocument, CrateName, TdddLayerBinding as DomainTdddLayerBinding,
};
use domain::tddd::type_signals_doc::RustdocExecutionIdentity;
use domain::tddd::{CargoFeatureName, LayerId};

use super::EvaluateSignalsError;
use super::freshness::RustdocProvider;
use crate::verify::tddd_layers::TdddLayerBinding;
#[cfg(test)]
use usecase::tddd_feature_declaration::TdddActualFeatureDeclarationPortError;

#[derive(Debug, Default)]
pub(super) struct FeatureDeclarationSnapshot {
    pub(super) declaration_bytes: Option<Vec<u8>>,
    pub(super) baseline_bytes: Option<Vec<u8>>,
}

pub(super) fn resolve_execution_identities(
    configured_layers: &[TdddLayerBinding],
    target_layer: &LayerId,
    loaded_catalogues: &BTreeMap<LayerId, AttestedCatalogueDocument>,
    feature_selections: &BTreeMap<LayerId, Vec<CargoFeatureName>>,
    rustdoc: &impl RustdocProvider,
) -> Result<BTreeMap<LayerId, RustdocExecutionIdentity>, EvaluateSignalsError> {
    let mut identities = BTreeMap::new();
    for binding in configured_layers {
        let layer = LayerId::try_new(binding.layer_id().to_owned()).map_err(|error| {
            EvaluateSignalsError::authoritative_input(format!("invalid layer id: {error}"))
        })?;
        if &layer != target_layer && !loaded_catalogues.contains_key(&layer) {
            continue;
        }
        let [target] = binding.targets() else {
            return Err(EvaluateSignalsError::authoritative_input(format!(
                "type-signal layer '{layer}' requires exactly one rustdoc target"
            )));
        };
        let target = CrateName::new(target.to_owned()).map_err(|error| {
            EvaluateSignalsError::authoritative_input(format!(
                "invalid rustdoc target crate '{target}' for layer '{layer}': {error}"
            ))
        })?;
        let features = feature_selections.get(&layer).ok_or_else(|| {
            EvaluateSignalsError::authoritative_input(format!(
                "feature declaration has no selection for configured layer '{layer}'"
            ))
        })?;
        let identity = rustdoc.execution_identity(&target, features).map_err(|error| {
            EvaluateSignalsError::authoritative_input(format!(
                "cannot resolve rustdoc execution identity for layer '{layer}': {error}"
            ))
        })?;
        crate::schema_export::require_exclusive_rustdoc_target(
            identity.target_directory().as_path(),
        )
        .map_err(|error| EvaluateSignalsError::authoritative_input(error.to_string()))?;
        identities.insert(layer, identity);
    }
    Ok(identities)
}

/// Decodes the captured feature declaration so every rustdoc export uses the
/// feature selection belonging to its own declaring layer. The usecase already
/// validates the target selection before entering this evaluator; the equality
/// check below keeps the evaluator's authoritative snapshot aligned with that
/// port input as well.
pub(super) fn load_layer_feature_selections(
    track_dir: &Path,
    workspace_root: &Path,
    configured_layers: &[TdddLayerBinding],
    target_layer: &LayerId,
    target_features: &[CargoFeatureName],
    captured: &FeatureDeclarationSnapshot,
) -> Result<BTreeMap<LayerId, Vec<CargoFeatureName>>, EvaluateSignalsError> {
    let domain_bindings = configured_layers
        .iter()
        .map(|binding| DomainTdddLayerBinding {
            layer_id: binding.layer_id().to_owned(),
            catalogue_file: binding.catalogue_file().to_owned(),
            baseline_file: binding.baseline_file(),
            targets: binding.targets().to_vec(),
        })
        .collect::<Vec<_>>();
    let declaration =
        match crate::tddd::feature_declaration_adapter::load_actual_from_captured_bytes(
            track_dir,
            workspace_root,
            &domain_bindings,
            captured.declaration_bytes.as_deref(),
            captured.baseline_bytes.as_deref(),
        ) {
            Ok(declaration) => declaration,
            Err(error) => {
                #[cfg(test)]
            if matches!(
                &error,
                TdddActualFeatureDeclarationPortError::Read(
                    usecase::tddd_feature_declaration::TdddFeatureDeclarationReadError::MissingDeclaration { .. }
                )
            ) {
                return fallback_layer_feature_selections(
                    configured_layers,
                    target_layer,
                    target_features,
                );
            }
                return Err(EvaluateSignalsError::authoritative_input(format!(
                    "cannot load frozen TDDD feature declaration: {error}"
                )));
            }
        };
    let selections = declaration.layers().clone();
    let declared_target = selections.get(target_layer).ok_or_else(|| {
        EvaluateSignalsError::authoritative_input(format!(
            "feature declaration has no selection for target layer '{target_layer}'"
        ))
    })?;
    if declared_target.as_slice() != target_features {
        return Err(EvaluateSignalsError::authoritative_input(format!(
            "feature selection for target layer '{target_layer}' differs between the feature declaration and executor input"
        )));
    }
    Ok(selections)
}

#[cfg(test)]
fn fallback_layer_feature_selections(
    configured_layers: &[TdddLayerBinding],
    target_layer: &LayerId,
    target_features: &[CargoFeatureName],
) -> Result<BTreeMap<LayerId, Vec<CargoFeatureName>>, EvaluateSignalsError> {
    configured_layers
        .iter()
        .map(|binding| {
            let layer = LayerId::try_new(binding.layer_id().to_owned()).map_err(|error| {
                EvaluateSignalsError::authoritative_input(format!("invalid layer id: {error}"))
            })?;
            let features =
                if &layer == target_layer { target_features.to_vec() } else { Vec::new() };
            Ok((layer, features))
        })
        .collect()
}
