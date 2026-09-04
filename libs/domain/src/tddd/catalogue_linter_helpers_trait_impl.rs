//! Trait-impl matching helpers split out of `catalogue_linter_helpers` for module size.

use super::*;

/// Returns whether a trait implementation for `type_name` resolves to the
/// required fully qualified trait identity.
pub(in crate::tddd::catalogue_linter) fn has_trait_impl<'a, E: TypeRefPathExtractorPort>(
    catalogue: &CatalogueDocument,
    type_name: &str,
    type_module_path: impl Into<Option<&'a ModulePath>>,
    trait_name_prefix: &str,
    extractor: &E,
) -> Result<bool, CatalogueLinterError> {
    let type_identity = canonical_catalogue_identity(catalogue, type_name, type_module_path)?;
    let type_identities = declared_type_identities(catalogue)?;
    let trait_identities = declared_trait_identities(catalogue)?;
    let external_trait_identities = allowed_external_trait_identities(catalogue, extractor)?;

    for ti in catalogue.trait_impls() {
        // Exclude delete-action impl entries: a deleted impl does not count as present.
        if ti.action() == ItemAction::Delete {
            continue;
        }
        let type_parameters = impl_type_parameters(ti);
        let for_type = root_path_occurrence(ti.for_type(), extractor, &type_parameters)?;
        let Some(for_type_identity) = resolve_catalogue_entry_reference(
            catalogue,
            for_type.as_str(),
            &type_identities,
            true,
        )?
        else {
            continue;
        };
        if for_type_identity == type_identity
            && trait_ref_matches(
                catalogue,
                ti.trait_ref(),
                trait_name_prefix,
                &trait_identities,
                &external_trait_identities,
                extractor,
                &type_parameters,
            )?
        {
            return Ok(true);
        }
    }
    Ok(false)
}

fn trait_ref_matches<E: TypeRefPathExtractorPort>(
    catalogue: &CatalogueDocument,
    actual_ref: &TypeRef,
    required_ref: &str,
    trait_identities: &BTreeSet<FullyQualifiedItemPath>,
    external_trait_identities: &BTreeSet<FullyQualifiedItemPath>,
    extractor: &E,
    type_parameters: &[ParamName],
) -> Result<bool, CatalogueLinterError> {
    let actual_ref = root_path_occurrence(actual_ref, extractor, type_parameters)?;
    let required_ref = TypeRef::new(required_ref.to_owned()).map_err(|error| {
        CatalogueLinterError::InvalidRuleConfig(FreeText::new(format!(
            "invalid required trait reference '{required_ref}': {error}"
        )))
    })?;
    let required_ref = root_path_occurrence(&required_ref, extractor, &[])?;
    let actual_identity = resolve_trait_identity(
        catalogue,
        &actual_ref,
        trait_identities,
        external_trait_identities,
    )?;
    let required_identity = resolve_trait_identity(
        catalogue,
        &required_ref,
        trait_identities,
        external_trait_identities,
    )?;

    match (actual_identity, required_identity) {
        (Some(actual), Some(required)) => Ok(actual == required),
        // Unresolved references are not identities and must not match by their
        // terminal spelling or path suffix.
        (None, None) => Ok(false),
        _ => Ok(false),
    }
}
