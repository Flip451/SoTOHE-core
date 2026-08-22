//! Private validation for type-alias declarations used by catalogue lint.
//!
//! The public diagnostic types stay in `catalogue_linter.rs` so their
//! fully-qualified rustdoc identities remain stable. This module contains
//! only the validation details needed before the rule evaluator runs.

use crate::tddd::catalogue_v2::identifiers::TypeName;
use crate::tddd::catalogue_v2::roles::NonEmptyVec;
use crate::tddd::catalogue_v2::{CatalogueDocument, TypeKindV2};
use crate::tddd::primitive_occurrence_scanner::{
    PrimitiveName, PrimitiveOccurrencePosition, PrimitiveOccurrenceScanner,
};

use super::{CatalogueLinterError, FreeText};

pub(super) fn validate_type_alias_generic_parameters<S: PrimitiveOccurrenceScanner>(
    catalogue: &CatalogueDocument,
    scanner: &S,
) -> Result<(), CatalogueLinterError> {
    for (alias_name, entry) in catalogue.types() {
        let TypeKindV2::TypeAlias { generics, .. } = entry.kind() else {
            continue;
        };
        let alias_name = type_alias_display_name(alias_name)?;
        if !generics.is_empty() && !entry.generics().is_empty() {
            return Err(CatalogueLinterError::ConflictingTypeAliasGenericParameters {
                alias_name: alias_name.clone(),
            });
        }
        let generics = if generics.is_empty() { entry.generics() } else { generics };

        let mut seen = std::collections::BTreeSet::new();
        for generic in generics {
            if !super::is_valid_type_alias_generic_parameter_name(generic.name.as_str()) {
                return Err(CatalogueLinterError::InvalidTypeAliasGenericParameterName {
                    alias_name: alias_name.clone(),
                    parameter_name: generic.name.clone(),
                });
            }
            if !seen.insert(generic.name.clone()) {
                return Err(CatalogueLinterError::DuplicateTypeAliasGenericParameter {
                    alias_name: alias_name.clone(),
                    parameter_name: generic.name.clone(),
                });
            }

            for bound in &generic.bounds {
                let probe =
                    PrimitiveName::new("__sotp_catalogue_lint_syntax_probe").map_err(|_| {
                        CatalogueLinterError::InvalidRuleConfig(FreeText::new(
                            "catalogue lint syntax probe is invalid",
                        ))
                    })?;
                scanner.scan(
                    bound.clone(),
                    NonEmptyVec::new(probe, vec![]),
                    PrimitiveOccurrencePosition::Bound,
                )?;
            }
        }
    }
    Ok(())
}

fn type_alias_display_name(
    alias_name: &crate::tddd::semantic_verify::CatalogueEntryKey,
) -> Result<TypeName, CatalogueLinterError> {
    let terminal_segment = alias_name.as_str().rsplit("::").next().unwrap_or(alias_name.as_str());
    TypeName::new(terminal_segment).map_err(|_| {
        CatalogueLinterError::InvalidRuleConfig(FreeText::new(
            "type alias key has an invalid terminal identifier",
        ))
    })
}
