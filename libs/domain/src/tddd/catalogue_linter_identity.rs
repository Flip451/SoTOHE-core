//! Catalogue-entry identity helpers for the identity-sensitive lint rules.

use std::collections::{BTreeMap, BTreeSet};

use super::{CatalogueLinterError, FreeText, RoleKind, RolePayloadField, TypeRefPathExtractorPort};
use crate::tddd::catalogue_linter::ExtractedTypeRefPath;
use crate::tddd::catalogue_v2::CatalogueDocument;
use crate::tddd::catalogue_v2::identifiers::{
    CrateName, FullyQualifiedItemPath, ModulePath, TypeRef,
};
use crate::tddd::catalogue_v2::identity_resolution::{
    CatalogueIdentityResolutionError, resolve_catalogue_identity,
};
use crate::tddd::catalogue_v2::roles::ItemAction;
use crate::tddd::layer_id::LayerId;
use crate::tddd::semantic_verify::CatalogueEntryKey;

#[derive(Debug, Clone)]
pub(super) struct DeclaredIdentity {
    identity: FullyQualifiedItemPath,
    role: RoleKind,
}

pub(super) fn build_declared_identities(
    all_catalogues: &BTreeMap<LayerId, CatalogueDocument>,
) -> Result<Vec<DeclaredIdentity>, CatalogueLinterError> {
    let mut entries = Vec::new();
    for catalogue in all_catalogues.values() {
        for (key, entry) in catalogue.types() {
            if entry.action() == ItemAction::Delete {
                continue;
            }
            let identity = FullyQualifiedItemPath::from_catalogue_entry_key(
                catalogue.crate_name(),
                key,
                entry.module_path(),
            )
            .map_err(|error| {
                CatalogueLinterError::InvalidRuleConfig(FreeText::new(format!(
                    "invalid catalogue type identity '{}': {error}",
                    key.as_str()
                )))
            })?;
            entries.push(DeclaredIdentity { identity, role: super::entry_role_kind(entry) });
        }
        for (key, entry) in catalogue.traits() {
            if entry.action() == ItemAction::Delete {
                continue;
            }
            let identity = FullyQualifiedItemPath::from_catalogue_entry_key(
                catalogue.crate_name(),
                key,
                entry.module_path(),
            )
            .map_err(|error| {
                CatalogueLinterError::InvalidRuleConfig(FreeText::new(format!(
                    "invalid catalogue trait identity '{}': {error}",
                    key.as_str()
                )))
            })?;
            entries.push(DeclaredIdentity {
                identity,
                role: RoleKind::from_contract_role(entry.role()),
            });
        }
    }
    Ok(entries)
}

pub(super) fn declared_identity_universe(
    entries: &[DeclaredIdentity],
) -> BTreeSet<FullyQualifiedItemPath> {
    entries.iter().map(|entry| entry.identity.clone()).collect()
}

fn role_for_identity(
    entries: &[DeclaredIdentity],
    identity: &FullyQualifiedItemPath,
) -> Option<RoleKind> {
    let mut role = None;
    for entry in entries.iter().filter(|entry| &entry.identity == identity) {
        match role {
            None => role = Some(entry.role),
            Some(previous) if previous == entry.role => {}
            Some(_) => return None,
        }
    }
    role
}

pub(super) fn entry_identity(
    catalogue: &CatalogueDocument,
    key: &CatalogueEntryKey,
    module_path: &ModulePath,
) -> Result<FullyQualifiedItemPath, CatalogueLinterError> {
    FullyQualifiedItemPath::from_catalogue_entry_key(catalogue.crate_name(), key, module_path)
        .map_err(|error| {
            CatalogueLinterError::InvalidRuleConfig(FreeText::new(format!(
                "invalid catalogue entry identity '{}': {error}",
                key.as_str()
            )))
        })
}

pub(super) fn resolve_reference_identities<E: TypeRefPathExtractorPort>(
    type_ref: &TypeRef,
    catalogue_crate: &CrateName,
    universe: &BTreeSet<FullyQualifiedItemPath>,
    extractor: &E,
) -> Result<Vec<FullyQualifiedItemPath>, CatalogueLinterError> {
    let mut resolved = Vec::new();
    for extracted in extractor.extract(type_ref)? {
        let (path, is_generic_constructor) = match extracted {
            ExtractedTypeRefPath::Reference(path) => (path, false),
            ExtractedTypeRefPath::GenericConstructor(path) => (path, true),
        };
        match resolve_catalogue_identity(&path, catalogue_crate, universe) {
            Ok(identity) => {
                if !resolved.contains(&identity) {
                    resolved.push(identity);
                }
            }
            Err(CatalogueIdentityResolutionError::UnresolvedIdentifier(_))
                if is_generic_constructor
                    && is_external_generic_root(path.as_str(), catalogue_crate, universe) => {}
            Err(error) => return Err(CatalogueLinterError::IdentityResolutionFailed(error)),
        }
    }
    if !resolved.is_empty() {
        return Ok(resolved);
    }
    Err(CatalogueLinterError::IdentityResolutionFailed(
        CatalogueIdentityResolutionError::UnresolvedIdentifier(type_ref.clone()),
    ))
}

pub(super) fn resolution_message(type_ref: &TypeRef, error: &CatalogueLinterError) -> String {
    format!("could not resolve type '{}' against catalogue identities: {error}", type_ref)
}

pub(super) fn role_constraint_failure<E: TypeRefPathExtractorPort>(
    type_ref: &TypeRef,
    expected_role: RoleKind,
    catalogue_crate: &CrateName,
    universe: &BTreeSet<FullyQualifiedItemPath>,
    entries: &[DeclaredIdentity],
    target_field: RolePayloadField,
    extractor: &E,
) -> Option<String> {
    let identities =
        match resolve_reference_identities(type_ref, catalogue_crate, universe, extractor) {
            Ok(identities) => identities,
            Err(error) => return Some(resolution_message(type_ref, &error)),
        };
    for identity in identities {
        match role_for_identity(entries, &identity) {
            Some(role) if role == expected_role => {}
            Some(role) => {
                return Some(format!(
                    "type '{}' referenced in field '{}' declares role '{}' instead of '{}'",
                    type_ref,
                    target_field,
                    role.variant_name(),
                    expected_role.variant_name()
                ));
            }
            None => {
                return Some(format!(
                    "could not determine a unique role for resolved identity '{}'",
                    identity
                ));
            }
        }
    }
    None
}

pub(super) fn signature_contains_identity<E: TypeRefPathExtractorPort>(
    type_ref: &TypeRef,
    target: &FullyQualifiedItemPath,
    catalogue_crate: &CrateName,
    universe: &BTreeSet<FullyQualifiedItemPath>,
    extractor: &E,
) -> Result<bool, CatalogueLinterError> {
    for extracted in extractor.extract(type_ref)? {
        let (path, is_generic_constructor) = match extracted {
            ExtractedTypeRefPath::Reference(path) => (path, false),
            ExtractedTypeRefPath::GenericConstructor(path) => (path, true),
        };
        match resolve_catalogue_identity(&path, catalogue_crate, universe) {
            Ok(identity) if identity == *target => return Ok(true),
            Ok(_) => {}
            Err(CatalogueIdentityResolutionError::UnresolvedIdentifier(_))
                if is_generic_constructor
                    && is_external_generic_root(path.as_str(), catalogue_crate, universe) => {}
            Err(error @ CatalogueIdentityResolutionError::AmbiguousIdentifier(_, _)) => {
                return Err(CatalogueLinterError::IdentityResolutionFailed(error));
            }
            Err(error @ CatalogueIdentityResolutionError::UnresolvedIdentifier(_))
                if !is_external_generic_root(path.as_str(), catalogue_crate, universe) =>
            {
                return Err(CatalogueLinterError::IdentityResolutionFailed(error));
            }
            Err(error @ CatalogueIdentityResolutionError::UnresolvedIdentifier(_))
                if path_terminal_name(&path) == Some(target.name().as_str()) =>
            {
                return Err(CatalogueLinterError::IdentityResolutionFailed(error));
            }
            Err(_) => {}
        }
    }
    Ok(false)
}

fn path_terminal_name(path: &TypeRef) -> Option<&str> {
    path.as_str().strip_prefix("::").unwrap_or(path.as_str()).rsplit("::").next()
}

fn is_external_generic_root(
    path: &str,
    catalogue_crate: &CrateName,
    universe: &BTreeSet<FullyQualifiedItemPath>,
) -> bool {
    let normalized = path.strip_prefix("::").unwrap_or(path);
    let Some((root, _)) = normalized.split_once("::") else {
        // A bare generic constructor has no crate identity in catalogue
        // notation. If it did not resolve as a local declaration, treat its
        // outer constructor as the external wrapper and still resolve every
        // nested catalogue identity. Bare non-generic names never reach this
        // branch because `is_generic_wrapper_reference` requires `<...>`.
        return true;
    };

    if matches!(root, "crate" | "self" | "super") || root == catalogue_crate.as_str() {
        return false;
    }
    if matches!(root, "std" | "core" | "alloc") {
        return true;
    }
    if universe.iter().any(|identity| identity.crate_name().as_str() == root) {
        return false;
    }
    !universe.iter().any(|identity| {
        identity.crate_name() == catalogue_crate
            && identity
                .module_path()
                .segments()
                .first()
                .is_some_and(|segment| segment.as_str() == root)
    })
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::tddd::catalogue_linter::{ExtractedTypeRefPath, TypeRefPathExtractionError};
    use crate::tddd::catalogue_v2::identifiers::{Identifier, ModulePath};

    struct GenericReferenceExtractor;

    impl TypeRefPathExtractorPort for GenericReferenceExtractor {
        fn extract(
            &self,
            type_ref: &TypeRef,
        ) -> Result<Vec<ExtractedTypeRefPath>, TypeRefPathExtractionError> {
            if type_ref.as_str() == "Wrapper<domain::alpha::Event>" {
                return Ok(vec![
                    ExtractedTypeRefPath::GenericConstructor(
                        TypeRef::new("Entity").expect("valid constructor path"),
                    ),
                    ExtractedTypeRefPath::Reference(
                        TypeRef::new("domain::alpha::Event").expect("valid nested reference path"),
                    ),
                ]);
            }
            Err(TypeRefPathExtractionError::InvalidTypeRef(type_ref.clone()))
        }
    }

    struct IdentityReferenceExtractor;

    impl TypeRefPathExtractorPort for IdentityReferenceExtractor {
        fn extract(
            &self,
            type_ref: &TypeRef,
        ) -> Result<Vec<ExtractedTypeRefPath>, TypeRefPathExtractionError> {
            match type_ref.as_str() {
                "missing-catalogue-identity" => Ok(vec![ExtractedTypeRefPath::Reference(
                    TypeRef::new("domain::missing::Customer")
                        .expect("valid unresolved catalogue path"),
                )]),
                "missing-catalogue-wrapper" => Ok(vec![
                    ExtractedTypeRefPath::GenericConstructor(
                        TypeRef::new("::domain::missing::Wrapper")
                            .expect("valid unresolved generic constructor"),
                    ),
                    ExtractedTypeRefPath::Reference(
                        TypeRef::new("domain::alpha::Event").expect("valid nested catalogue path"),
                    ),
                ]),
                _ => Err(TypeRefPathExtractionError::InvalidTypeRef(type_ref.clone())),
            }
        }
    }

    fn identity(module: &str, name: &str) -> FullyQualifiedItemPath {
        FullyQualifiedItemPath::new(
            CrateName::new("domain").expect("valid crate name"),
            ModulePath::from_segments(vec![module]).expect("valid module path"),
            Identifier::new(name).expect("valid item name"),
        )
    }

    #[test]
    fn test_resolve_reference_identities_rejects_ambiguous_generic_constructor() {
        let alpha_entity = identity("alpha", "Entity");
        let beta_entity = identity("beta", "Entity");
        let alpha_event = identity("alpha", "Event");
        let universe = BTreeSet::from([alpha_entity.clone(), beta_entity.clone(), alpha_event]);
        let type_ref =
            TypeRef::new("Wrapper<domain::alpha::Event>").expect("valid wrapped type reference");

        let error = resolve_reference_identities(
            &type_ref,
            &CrateName::new("domain").expect("valid crate name"),
            &universe,
            &GenericReferenceExtractor,
        )
        .expect_err("ambiguous generic constructor must fail closed");

        assert!(matches!(
            error,
            CatalogueLinterError::IdentityResolutionFailed(
                CatalogueIdentityResolutionError::AmbiguousIdentifier(_, candidates)
            ) if candidates.as_slice() == [alpha_entity, beta_entity]
        ));
    }

    #[test]
    fn test_resolve_reference_identities_rejects_leading_colon_known_crate_constructor() {
        let event = identity("alpha", "Event");
        let universe = BTreeSet::from([event]);
        let type_ref =
            TypeRef::new("missing-catalogue-wrapper").expect("valid extractor fixture reference");

        let error = resolve_reference_identities(
            &type_ref,
            &CrateName::new("domain").expect("valid crate name"),
            &universe,
            &IdentityReferenceExtractor,
        )
        .expect_err("unresolved known-crate constructor must fail closed");

        assert!(matches!(
            error,
            CatalogueLinterError::IdentityResolutionFailed(
                CatalogueIdentityResolutionError::UnresolvedIdentifier(unresolved)
            ) if unresolved.as_str() == "::domain::missing::Wrapper"
        ));
    }

    #[test]
    fn test_signature_contains_identity_rejects_unresolved_known_crate_reference() {
        let target = identity("orders", "OrderLine");
        let universe = BTreeSet::from([target.clone()]);
        let type_ref =
            TypeRef::new("missing-catalogue-identity").expect("valid extractor fixture reference");

        let error = signature_contains_identity(
            &type_ref,
            &target,
            &CrateName::new("domain").expect("valid crate name"),
            &universe,
            &IdentityReferenceExtractor,
        )
        .expect_err("unresolved known-crate reference must fail closed");

        assert!(matches!(
            error,
            CatalogueLinterError::IdentityResolutionFailed(
                CatalogueIdentityResolutionError::UnresolvedIdentifier(unresolved)
            ) if unresolved.as_str() == "domain::missing::Customer"
        ));
    }
}
