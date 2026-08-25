//! Catalogue-entry identity helpers for the identity-sensitive lint rules.

use std::collections::{BTreeMap, BTreeSet};

use super::{CatalogueLinterError, FreeText, RoleKind, RolePayloadField, TypeRefPathExtractorPort};
use crate::tddd::catalogue_linter::ExtractedTypeRefPath;
use crate::tddd::catalogue_v2::CatalogueDocument;
use crate::tddd::catalogue_v2::identifiers::{
    CrateName, FullyQualifiedItemPath, ModulePath, ParamName, TypeRef,
};
use crate::tddd::catalogue_v2::identity_resolution::{
    CatalogueIdentityResolutionError, resolve_catalogue_identity,
};
use crate::tddd::catalogue_v2::methods::MethodGenericParam;
use crate::tddd::catalogue_v2::roles::ItemAction;
use crate::tddd::layer_id::LayerId;
use crate::tddd::semantic_verify::CatalogueEntryKey;

#[derive(Debug, Clone)]
pub(super) struct DeclaredIdentity {
    identity: FullyQualifiedItemPath,
    role: RoleKind,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct TypeRefInspectionContext<'a> {
    pub(super) type_parameters: &'a [ParamName],
    pub(super) lifetime_parameters: &'a [ParamName],
    pub(super) const_parameters: &'a [ParamName],
}

#[derive(Debug, Clone, Copy)]
pub(super) struct CatalogueIdentityContext<'a> {
    pub(super) catalogue_crate: &'a CrateName,
    pub(super) universe: &'a BTreeSet<FullyQualifiedItemPath>,
    pub(super) entries: &'a [DeclaredIdentity],
}

pub(super) fn generic_parameter_names(generics: &[MethodGenericParam]) -> Vec<ParamName> {
    generics.iter().map(|generic| generic.name.clone()).collect()
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
    context: CatalogueIdentityContext<'_>,
    extractor: &E,
    inspection: TypeRefInspectionContext<'_>,
) -> Result<Vec<FullyQualifiedItemPath>, CatalogueLinterError> {
    let mut resolved = Vec::new();
    for extracted in extractor.extract(
        type_ref,
        inspection.type_parameters,
        inspection.lifetime_parameters,
        inspection.const_parameters,
    )? {
        let ExtractedTypeRefPath::Path(path) = extracted else {
            continue;
        };
        if let Some(identity) =
            classify_catalogue_path(&path, context.catalogue_crate, context.universe)?
        {
            if !resolved.contains(&identity) {
                resolved.push(identity);
            }
        }
    }
    Ok(resolved)
}

pub(super) fn resolution_message(type_ref: &TypeRef, error: &CatalogueLinterError) -> String {
    format!("could not resolve type '{}' against catalogue identities: {error}", type_ref)
}

pub(super) fn role_constraint_failure<E: TypeRefPathExtractorPort>(
    type_ref: &TypeRef,
    expected_role: RoleKind,
    context: CatalogueIdentityContext<'_>,
    target_field: RolePayloadField,
    extractor: &E,
    inspection: TypeRefInspectionContext<'_>,
) -> Option<String> {
    let identities = match resolve_reference_identities(type_ref, context, extractor, inspection) {
        Ok(identities) => identities,
        Err(error) => return Some(resolution_message(type_ref, &error)),
    };
    for identity in identities {
        match role_for_identity(context.entries, &identity) {
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
    context: CatalogueIdentityContext<'_>,
    extractor: &E,
    inspection: TypeRefInspectionContext<'_>,
) -> Result<bool, CatalogueLinterError> {
    for extracted in extractor.extract(
        type_ref,
        inspection.type_parameters,
        inspection.lifetime_parameters,
        inspection.const_parameters,
    )? {
        let ExtractedTypeRefPath::Path(path) = extracted else {
            continue;
        };
        if let Some(identity) =
            classify_catalogue_path(&path, context.catalogue_crate, context.universe)?
        {
            if identity == *target {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

/// Classifies a syntactic path against the finite set of declared catalogue
/// identities. `None` means that the path did not match a catalogue identity;
/// Chain 3 owns validating whether such a path exists in the implementation.
/// Ambiguous catalogue matches remain an error because catalogue-local
/// identity must be unique.
fn classify_catalogue_path(
    path: &TypeRef,
    catalogue_crate: &CrateName,
    universe: &BTreeSet<FullyQualifiedItemPath>,
) -> Result<Option<FullyQualifiedItemPath>, CatalogueLinterError> {
    let normalized = path.as_str().strip_prefix("::").unwrap_or(path.as_str());
    if normalized == "Self" {
        return Ok(None);
    }

    match resolve_catalogue_identity(path, catalogue_crate, universe) {
        Ok(identity) => Ok(Some(identity)),
        Err(CatalogueIdentityResolutionError::UnresolvedIdentifier(_)) => Ok(None),
        Err(error) => Err(CatalogueLinterError::IdentityResolutionFailed(error)),
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::tddd::catalogue_linter::{ExtractedTypeRefPath, TypeRefPathExtractionError};
    use crate::tddd::catalogue_v2::identifiers::{Identifier, ModulePath};
    use crate::tddd::catalogue_v2::identity_resolution::CatalogueIdentityResolutionError;

    struct GenericReferenceExtractor;

    impl TypeRefPathExtractorPort for GenericReferenceExtractor {
        fn extract(
            &self,
            type_ref: &TypeRef,
            _type_parameters: &[ParamName],
            _lifetime_parameters: &[ParamName],
            _const_parameters: &[ParamName],
        ) -> Result<Vec<ExtractedTypeRefPath>, TypeRefPathExtractionError> {
            if type_ref.as_str() == "Wrapper<domain::alpha::Event>" {
                return Ok(vec![
                    ExtractedTypeRefPath::Path(
                        TypeRef::new("Entity").expect("valid constructor path"),
                    ),
                    ExtractedTypeRefPath::Path(
                        TypeRef::new("domain::alpha::Event").expect("valid nested reference path"),
                    ),
                ]);
            }
            Err(TypeRefPathExtractionError::UnsupportedSyntax { location: type_ref.clone() })
        }
    }

    struct IdentityReferenceExtractor;

    impl TypeRefPathExtractorPort for IdentityReferenceExtractor {
        fn extract(
            &self,
            type_ref: &TypeRef,
            _type_parameters: &[ParamName],
            _lifetime_parameters: &[ParamName],
            _const_parameters: &[ParamName],
        ) -> Result<Vec<ExtractedTypeRefPath>, TypeRefPathExtractionError> {
            match type_ref.as_str() {
                "missing-catalogue-identity" => Ok(vec![ExtractedTypeRefPath::Path(
                    TypeRef::new("domain::missing::Customer")
                        .expect("valid unresolved catalogue path"),
                )]),
                "missing-catalogue-wrapper" => Ok(vec![
                    ExtractedTypeRefPath::Path(
                        TypeRef::new("::domain::missing::Wrapper")
                            .expect("valid unresolved generic constructor"),
                    ),
                    ExtractedTypeRefPath::Path(
                        TypeRef::new("domain::alpha::Event").expect("valid nested catalogue path"),
                    ),
                ]),
                _ => Err(TypeRefPathExtractionError::UnsupportedSyntax {
                    location: type_ref.clone(),
                }),
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

    fn identity_context<'a>(
        catalogue_crate: &'a CrateName,
        universe: &'a BTreeSet<FullyQualifiedItemPath>,
    ) -> CatalogueIdentityContext<'a> {
        CatalogueIdentityContext { catalogue_crate, universe, entries: &[] }
    }

    fn empty_inspection() -> TypeRefInspectionContext<'static> {
        TypeRefInspectionContext {
            type_parameters: &[],
            lifetime_parameters: &[],
            const_parameters: &[],
        }
    }

    #[test]
    fn test_resolve_reference_identities_rejects_ambiguous_generic_constructor() {
        let alpha_entity = identity("alpha", "Entity");
        let beta_entity = identity("beta", "Entity");
        let alpha_event = identity("alpha", "Event");
        let universe = BTreeSet::from([alpha_entity.clone(), beta_entity.clone(), alpha_event]);
        let type_ref =
            TypeRef::new("Wrapper<domain::alpha::Event>").expect("valid wrapped type reference");
        let catalogue_crate = CrateName::new("domain").expect("valid crate name");

        let error = resolve_reference_identities(
            &type_ref,
            identity_context(&catalogue_crate, &universe),
            &GenericReferenceExtractor,
            empty_inspection(),
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
    fn test_resolve_reference_identities_skips_unmatched_path_and_keeps_catalogue_match() {
        let event = identity("alpha", "Event");
        let universe = BTreeSet::from([event.clone()]);
        let type_ref =
            TypeRef::new("missing-catalogue-wrapper").expect("valid extractor fixture reference");
        let catalogue_crate = CrateName::new("domain").expect("valid crate name");

        let resolved = resolve_reference_identities(
            &type_ref,
            identity_context(&catalogue_crate, &universe),
            &IdentityReferenceExtractor,
            empty_inspection(),
        )
        .expect("unmatched paths are delegated to Chain 3");

        assert_eq!(resolved, vec![event]);
    }

    #[test]
    fn test_signature_contains_identity_skips_unmatched_known_crate_reference() {
        let target = identity("orders", "OrderLine");
        let universe = BTreeSet::from([target.clone()]);
        let type_ref =
            TypeRef::new("missing-catalogue-identity").expect("valid extractor fixture reference");
        let catalogue_crate = CrateName::new("domain").expect("valid crate name");

        let contains = signature_contains_identity(
            &type_ref,
            &target,
            identity_context(&catalogue_crate, &universe),
            &IdentityReferenceExtractor,
            empty_inspection(),
        )
        .expect("unmatched paths are delegated to Chain 3");

        assert!(!contains);
    }

    struct EchoPathExtractor;

    impl TypeRefPathExtractorPort for EchoPathExtractor {
        fn extract(
            &self,
            type_ref: &TypeRef,
            _type_parameters: &[ParamName],
            _lifetime_parameters: &[ParamName],
            _const_parameters: &[ParamName],
        ) -> Result<Vec<ExtractedTypeRefPath>, TypeRefPathExtractionError> {
            Ok(vec![ExtractedTypeRefPath::Path(type_ref.clone())])
        }
    }

    #[test]
    fn test_resolve_reference_identities_skips_unknown_bare_path() {
        let type_ref = TypeRef::new("ImportedButUnclassified").expect("valid TypeRef");
        let universe = BTreeSet::new();
        let catalogue_crate = CrateName::new("domain").expect("valid crate name");

        let resolved = resolve_reference_identities(
            &type_ref,
            identity_context(&catalogue_crate, &universe),
            &EchoPathExtractor,
            empty_inspection(),
        )
        .expect("unmatched bare paths are delegated to Chain 3");

        assert!(resolved.is_empty());
    }

    #[test]
    fn test_resolve_reference_identities_skips_unmatched_paths_regardless_of_root() {
        let catalogue_crate = CrateName::new("domain").expect("valid crate name");
        let universe = BTreeSet::new();

        for reference in ["payments::Missing", "serde::Serialize", "heplers::Thing"] {
            let reference = TypeRef::new(reference).expect("valid TypeRef");
            let resolved = resolve_reference_identities(
                &reference,
                identity_context(&catalogue_crate, &universe),
                &EchoPathExtractor,
                empty_inspection(),
            )
            .expect("unmatched qualified paths are delegated to Chain 3");
            assert!(resolved.is_empty(), "unexpected catalogue identity for {reference}");
        }
    }

    #[test]
    fn test_resolve_reference_identities_prefers_catalogue_identity_under_std_core_alloc_modules() {
        let catalogue_crate = CrateName::new("domain").expect("valid crate name");

        for root in ["std", "core", "alloc"] {
            let expected = identity(root, "Entity");
            let universe = BTreeSet::from([expected.clone()]);
            let type_ref = TypeRef::new(format!("domain::{root}::Entity"))
                .expect("valid crate-qualified catalogue path");

            let resolved = resolve_reference_identities(
                &type_ref,
                identity_context(&catalogue_crate, &universe),
                &EchoPathExtractor,
                empty_inspection(),
            )
            .expect("declared catalogue identities resolve before unmatched paths are skipped");

            assert_eq!(resolved, vec![expected]);
        }
    }

    #[test]
    fn test_resolve_reference_identities_skips_unmatched_std_path() {
        let universe = BTreeSet::new();
        let catalogue_crate = CrateName::new("domain").expect("valid crate name");
        let type_ref = TypeRef::new("std::vec::Vec").expect("valid external path");

        let resolved = resolve_reference_identities(
            &type_ref,
            identity_context(&catalogue_crate, &universe),
            &EchoPathExtractor,
            empty_inspection(),
        )
        .expect("unmatched standard-library paths are delegated to Chain 3");

        assert!(resolved.is_empty());
    }

    #[test]
    fn test_resolve_reference_identities_skips_unmatched_bare_paths() {
        let universe = BTreeSet::new();
        let catalogue_crate = CrateName::new("domain").expect("valid crate name");

        for path in [
            "f32",
            "f64",
            "Display",
            "Hash",
            "TryFrom",
            "VecDeque",
            "LinkedList",
            "Mutex",
            "RwLock",
        ] {
            let type_ref = TypeRef::new(path).expect("valid external path");
            let resolved = resolve_reference_identities(
                &type_ref,
                identity_context(&catalogue_crate, &universe),
                &EchoPathExtractor,
                empty_inspection(),
            )
            .expect("unmatched bare paths are delegated to Chain 3");

            assert!(resolved.is_empty(), "unexpected catalogue identity for {path}");
        }
    }
}
