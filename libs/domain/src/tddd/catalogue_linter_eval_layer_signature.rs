//! Layer-based method-signature boundary evaluation.

use std::collections::BTreeMap;

use super::eval::eval_helpers::sig_type_contains_entry;
use super::helpers::{collect_methods_for_type, trait_entries_for_target, type_entries_for_target};
use super::{CatalogueLintViolation, CatalogueLinterError, RuleTarget};
use crate::tddd::catalogue_v2::CatalogueDocument;
use crate::tddd::catalogue_v2::methods::MethodDeclaration;
use crate::tddd::catalogue_v2::roles::{ItemAction, NonEmptyVec};
use crate::tddd::layer_id::LayerId;

pub(super) fn evaluate(
    rule_kind: &'static str,
    target: &RuleTarget,
    forbidden_layers: &NonEmptyVec<LayerId>,
    catalogue: &CatalogueDocument,
    all_catalogues: &BTreeMap<LayerId, CatalogueDocument>,
    target_layer_id: &LayerId,
) -> Result<Vec<CatalogueLintViolation>, CatalogueLinterError> {
    let mut violations = Vec::new();
    for (name, entry) in type_entries_for_target(catalogue, target) {
        evaluate_methods(
            rule_kind,
            name.as_str(),
            collect_methods_for_type(catalogue, entry, name.as_str())?,
            forbidden_layers,
            all_catalogues,
            target_layer_id,
            &mut violations,
        );
    }
    for (name, entry) in trait_entries_for_target(catalogue, target) {
        evaluate_methods(
            rule_kind,
            name.as_str(),
            entry.methods().iter(),
            forbidden_layers,
            all_catalogues,
            target_layer_id,
            &mut violations,
        );
    }
    Ok(violations)
}

fn evaluate_methods<'a>(
    rule_kind: &'static str,
    entry_name: &str,
    methods: impl IntoIterator<Item = &'a MethodDeclaration>,
    forbidden_layers: &NonEmptyVec<LayerId>,
    all_catalogues: &BTreeMap<LayerId, CatalogueDocument>,
    target_layer_id: &LayerId,
    violations: &mut Vec<CatalogueLintViolation>,
) {
    for method in methods {
        let sig_types = method
            .params
            .iter()
            .map(|param| param.ty.as_str())
            .chain(std::iter::once(method.returns.as_str()));
        'sig_slot: for type_ref_str in sig_types {
            for (layer_id, candidate_catalogue) in all_catalogues {
                if !forbidden_layers.as_slice().contains(layer_id) {
                    continue;
                }
                let referenced_type = candidate_catalogue
                    .types()
                    .iter()
                    .filter(|(_, entry)| entry.action() != ItemAction::Delete)
                    .map(|(name, _)| name.as_str())
                    .chain(
                        candidate_catalogue
                            .traits()
                            .iter()
                            .filter(|(_, entry)| entry.action() != ItemAction::Delete)
                            .map(|(name, _)| name.as_str()),
                    )
                    .find(|type_name| {
                        sig_type_contains_entry(
                            type_ref_str,
                            type_name,
                            layer_id,
                            target_layer_id,
                            all_catalogues,
                        )
                    });
                if let Some(referenced_type) = referenced_type {
                    violations.push(CatalogueLintViolation::new(
                        rule_kind,
                        entry_name,
                        format!(
                            "method '{}' signature contains type '{}' declared in forbidden layer '{}' (resolved entry '{}')",
                            method.name.as_str(), type_ref_str, layer_id, referenced_type
                        ),
                    ));
                    continue 'sig_slot;
                }
            }
        }
    }
}
