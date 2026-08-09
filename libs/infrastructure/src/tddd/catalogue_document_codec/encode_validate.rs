//! Shared validation for domain-to-DTO generic declarations.
//!
//! Kept separate from the entry converters so the production encode module
//! remains within the repository's module-size budget.

use std::collections::HashSet;

use domain::tddd::catalogue_v2::MethodGenericParam;

use super::CatalogueDocumentCodecError;
use super::validate::{is_valid_generic_param_name, validate_bound_str_with_generics};
use crate::tddd::type_ref_parser::validate_lexical_generic_bound;

/// Validates the sole generic declaration that a type alias will encode.
///
/// Aliases support a legacy entry-level declaration as well as the current
/// kind-level declaration. The decoder applies these checks to either shape,
/// so encode must do the same to preserve the document codec round-trip
/// contract.
pub(super) fn validate_type_alias_generics(
    entry_name: &str,
    generics: &[MethodGenericParam],
) -> Result<(), CatalogueDocumentCodecError> {
    let generic_names = generics.iter().map(|generic| generic.name.as_str()).collect::<Vec<_>>();
    let mut seen = HashSet::new();
    for generic in generics {
        if !is_valid_generic_param_name(generic.name.as_str()) {
            return Err(CatalogueDocumentCodecError::InvalidEntry {
                entry_name: entry_name.to_owned(),
                reason: format!(
                    "generic param name '{}' is not a valid Rust identifier \
                     (must match [a-zA-Z_][a-zA-Z0-9_]* and must not be '_' or a path-context keyword)",
                    generic.name.as_str()
                ),
            });
        }
        if !seen.insert(generic.name.as_str()) {
            return Err(CatalogueDocumentCodecError::InvalidEntry {
                entry_name: entry_name.to_owned(),
                reason: format!(
                    "duplicate generic param name '{}' in type alias declaration",
                    generic.name.as_str()
                ),
            });
        }
        for (idx, bound) in generic.bounds.iter().enumerate() {
            validate_lexical_generic_bound(bound.as_str(), &generic_names).map_err(|error| {
                CatalogueDocumentCodecError::InvalidEntry {
                    entry_name: entry_name.to_owned(),
                    reason: format!("invalid generic param bound[{idx}]: {error}"),
                }
            })?;
        }
    }
    Ok(())
}

/// Validates generic declarations before their infallible DTO conversion.
///
/// `outer_generic_names` is the enclosing declaration context for methods;
/// those names may not be shadowed by a method-level parameter.
pub(super) fn validate_generic_params_for_encode(
    entry_name: &str,
    generics: &[MethodGenericParam],
    outer_generic_names: &[&str],
) -> Result<(), CatalogueDocumentCodecError> {
    let names = outer_generic_names
        .iter()
        .copied()
        .chain(generics.iter().map(|generic| generic.name.as_str()))
        .collect::<Vec<_>>();
    let mut seen = outer_generic_names.iter().copied().collect::<HashSet<_>>();
    for generic in generics {
        // Shape validity is guaranteed by the domain `ParamName` type; the
        // non-keyword restriction applies only in alias validation
        // (`validate_type_alias_generics`), so keyword names decoded from
        // pre-existing non-alias entries re-encode symmetrically.
        if !seen.insert(generic.name.as_str()) {
            return Err(CatalogueDocumentCodecError::InvalidEntry {
                entry_name: entry_name.to_owned(),
                reason: format!("duplicate generic param name '{}'", generic.name.as_str()),
            });
        }
        for (index, bound) in generic.bounds.iter().enumerate() {
            validate_bound_str_with_generics(bound.as_str(), &names).map_err(|error| {
                CatalogueDocumentCodecError::InvalidEntry {
                    entry_name: entry_name.to_owned(),
                    reason: format!("invalid generic param bound[{index}]: {error}"),
                }
            })?;
        }
    }
    Ok(())
}
