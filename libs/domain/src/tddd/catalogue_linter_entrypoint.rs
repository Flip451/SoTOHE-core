//! Private evaluation entrypoint orchestration for the catalogue linter.

use super::eval;
use super::{
    CatalogueLintViolation, CatalogueLinterError, CatalogueLinterRule, FreeText,
    TypeRefPathExtractorPort,
};
use crate::tddd::catalogue_v2::CatalogueDocument;
use crate::tddd::layer_id::LayerId;
use crate::tddd::primitive_occurrence_scanner::PrimitiveOccurrenceScanner;
use std::collections::BTreeMap;

#[path = "catalogue_linter_generic.rs"]
mod generic;

#[path = "catalogue_linter_validation.rs"]
mod validation;

#[path = "catalogue_linter_preflight.rs"]
mod preflight;

pub(super) fn evaluate_catalogue_lint<
    S: PrimitiveOccurrenceScanner,
    E: TypeRefPathExtractorPort,
>(
    rules: &[CatalogueLinterRule],
    all_catalogues: &BTreeMap<LayerId, CatalogueDocument>,
    target_layer_id: &LayerId,
    scanner: &S,
    extractor: &E,
) -> Result<Vec<CatalogueLintViolation>, CatalogueLinterError> {
    for catalogue in all_catalogues.values() {
        validation::validate_type_alias_generic_parameters(catalogue, scanner)?;
    }
    preflight::inspect_all_catalogue_type_refs(all_catalogues, extractor)?;
    eval::evaluate_catalogue_lint(rules, all_catalogues, target_layer_id, scanner, extractor)
}

#[cfg(test)]
/// Runs the pre-existing rule-focused unit fixtures without the entrypoint's
/// whole-catalogue TypeRef gate. Those fixtures deliberately use partial
/// identity universes to exercise individual rule diagnostics; dedicated
/// preflight tests cover the public entrypoint contract.
pub(super) fn evaluate_catalogue_lint_rules_only<
    S: PrimitiveOccurrenceScanner,
    E: TypeRefPathExtractorPort,
>(
    rules: &[CatalogueLinterRule],
    all_catalogues: &BTreeMap<LayerId, CatalogueDocument>,
    target_layer_id: &LayerId,
    scanner: &S,
    extractor: &E,
) -> Result<Vec<CatalogueLintViolation>, CatalogueLinterError> {
    for catalogue in all_catalogues.values() {
        validation::validate_type_alias_generic_parameters(catalogue, scanner)?;
    }
    eval::evaluate_catalogue_lint(rules, all_catalogues, target_layer_id, scanner, extractor)
}
