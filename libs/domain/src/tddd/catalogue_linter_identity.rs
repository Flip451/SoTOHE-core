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
    pub(super) locality_modules: &'a BTreeSet<ModulePath>,
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

/// Returns the live module paths that prove a crate-local root is in scope.
///
/// Type and trait module paths come from their canonical declared identities;
/// function paths are already fully qualified keys and contribute their own
/// definition module paths. Deleted entries are excluded consistently with the
/// identity universe, so tombstones cannot make an unresolved path look local.
pub(super) fn declared_locality_modules(
    all_catalogues: &BTreeMap<LayerId, CatalogueDocument>,
    entries: &[DeclaredIdentity],
    catalogue_crate: &CrateName,
) -> BTreeSet<ModulePath> {
    let mut modules = entries
        .iter()
        .filter(|entry| entry.identity.crate_name() == catalogue_crate)
        .map(|entry| entry.identity.module_path().clone())
        .collect::<BTreeSet<_>>();

    for catalogue in
        all_catalogues.values().filter(|catalogue| catalogue.crate_name() == catalogue_crate)
    {
        for (path, entry) in catalogue.functions() {
            if entry.action() != ItemAction::Delete {
                modules.insert(path.module_path.clone());
            }
        }
    }

    modules
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
        if let Some(identity) = classify_catalogue_path(
            &path,
            context.catalogue_crate,
            context.universe,
            context.locality_modules,
        )? {
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
        if let Some(identity) = classify_catalogue_path(
            &path,
            context.catalogue_crate,
            context.universe,
            context.locality_modules,
        )? {
            if identity == *target {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

/// Classifies a syntactic path using only catalogue declarations and crate
/// context. `None` means an explicitly external Rust path; `Some` is a
/// resolved catalogue identity. Unknown bare names fail closed because the
/// linter cannot prove whether they are an external import or a missing
/// catalogue declaration.
fn classify_catalogue_path(
    path: &TypeRef,
    catalogue_crate: &CrateName,
    universe: &BTreeSet<FullyQualifiedItemPath>,
    locality_modules: &BTreeSet<ModulePath>,
) -> Result<Option<FullyQualifiedItemPath>, CatalogueLinterError> {
    let normalized = path.as_str().strip_prefix("::").unwrap_or(path.as_str());
    if normalized == "Self" {
        return Ok(None);
    }

    match resolve_catalogue_identity(path, catalogue_crate, universe) {
        Ok(identity) => Ok(Some(identity)),
        Err(CatalogueIdentityResolutionError::UnresolvedIdentifier(location))
            if is_known_external_bare_path(location.as_str())
                || is_explicit_external_qualified_path(
                    normalized,
                    catalogue_crate,
                    universe,
                    locality_modules,
                ) =>
        {
            // Resolution has already had the first chance to find a declared
            // identity. Only an unresolved path can be classified as external;
            // this preserves declarations such as `domain::std::Entity`.
            Ok(None)
        }
        Err(CatalogueIdentityResolutionError::UnresolvedIdentifier(location))
            if normalized.contains("::") =>
        {
            Err(CatalogueLinterError::IdentityResolutionFailed(
                CatalogueIdentityResolutionError::UnresolvedIdentifier(location),
            ))
        }
        Err(CatalogueIdentityResolutionError::UnresolvedIdentifier(location)) => {
            Err(CatalogueLinterError::IdentityResolutionFailed(
                CatalogueIdentityResolutionError::ClassificationFailed { location },
            ))
        }
        Err(error) => Err(CatalogueLinterError::IdentityResolutionFailed(error)),
    }
}

fn is_explicit_external_qualified_path(
    normalized: &str,
    catalogue_crate: &CrateName,
    universe: &BTreeSet<FullyQualifiedItemPath>,
    locality_modules: &BTreeSet<ModulePath>,
) -> bool {
    let Some((root, _)) = normalized.split_once("::") else {
        return false;
    };

    if matches!(root, "std" | "core" | "alloc") {
        return true;
    }

    root != catalogue_crate.as_str()
        && !matches!(root, "crate" | "self" | "super")
        && !universe.iter().any(|identity| identity.crate_name().as_str() == root)
        && !is_known_catalogue_crate(root)
        && !locality_modules.iter().any(|module_path| {
            module_path.segments().first().is_some_and(|segment| segment.as_str() == root)
        })
}

fn is_known_external_bare_path(path: &str) -> bool {
    matches!(
        path,
        "bool"
            | "char"
            | "str"
            | "f32"
            | "f64"
            | "i8"
            | "i16"
            | "i32"
            | "i64"
            | "i128"
            | "isize"
            | "u8"
            | "u16"
            | "u32"
            | "u64"
            | "u128"
            | "usize"
            | "never"
            | "Option"
            | "Result"
            | "Vec"
            | "String"
            | "Box"
            | "Rc"
            | "Arc"
            | "Cell"
            | "RefCell"
            | "Pin"
            | "PhantomData"
            | "PhantomPinned"
            | "Path"
            | "PathBuf"
            | "Cow"
            | "HashMap"
            | "HashSet"
            | "BTreeMap"
            | "BTreeSet"
            | "VecDeque"
            | "LinkedList"
            | "Iterator"
            | "IntoIterator"
            | "DoubleEndedIterator"
            | "ExactSizeIterator"
            | "FromIterator"
            | "Extend"
            | "Sum"
            | "Product"
            | "Send"
            | "Sync"
            | "Sized"
            | "Unpin"
            | "Clone"
            | "Copy"
            | "Debug"
            | "Display"
            | "Default"
            | "Eq"
            | "Ord"
            | "Hash"
            | "PartialEq"
            | "PartialOrd"
            | "From"
            | "Into"
            | "TryFrom"
            | "TryInto"
            | "AsRef"
            | "AsMut"
            | "Deref"
            | "DerefMut"
            | "Drop"
            | "Fn"
            | "FnMut"
            | "FnOnce"
            | "ToString"
            | "ToOwned"
            | "Borrow"
            | "BorrowMut"
            | "Mutex"
            | "RwLock"
            | "Error"
            | "Read"
            | "Write"
            | "Seek"
            | "BufRead"
            | "Formatter"
            | "Add"
            | "Sub"
            | "Mul"
            | "Div"
            | "Rem"
            | "Neg"
            | "Not"
            | "BitAnd"
            | "BitOr"
            | "BitXor"
            | "Shl"
            | "Shr"
            | "Index"
            | "IndexMut"
            | "AddAssign"
            | "SubAssign"
            | "MulAssign"
            | "DivAssign"
            | "RemAssign"
            | "BitAndAssign"
            | "BitOrAssign"
            | "BitXorAssign"
            | "ShlAssign"
            | "ShrAssign"
            | "FromStr"
            | "Hasher"
            | "BuildHasher"
    )
}

fn is_known_catalogue_crate(crate_name: &str) -> bool {
    matches!(
        crate_name,
        "domain" | "usecase" | "infrastructure" | "cli" | "cli_driver" | "cli_composition"
    )
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
        static EMPTY_LOCALITY_MODULES: std::sync::OnceLock<BTreeSet<ModulePath>> =
            std::sync::OnceLock::new();
        CatalogueIdentityContext {
            catalogue_crate,
            universe,
            locality_modules: EMPTY_LOCALITY_MODULES.get_or_init(BTreeSet::new),
            entries: &[],
        }
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
    fn test_resolve_reference_identities_rejects_leading_colon_known_crate_constructor() {
        let event = identity("alpha", "Event");
        let universe = BTreeSet::from([event]);
        let type_ref =
            TypeRef::new("missing-catalogue-wrapper").expect("valid extractor fixture reference");
        let catalogue_crate = CrateName::new("domain").expect("valid crate name");

        let error = resolve_reference_identities(
            &type_ref,
            identity_context(&catalogue_crate, &universe),
            &IdentityReferenceExtractor,
            empty_inspection(),
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
        let catalogue_crate = CrateName::new("domain").expect("valid crate name");

        let error = signature_contains_identity(
            &type_ref,
            &target,
            identity_context(&catalogue_crate, &universe),
            &IdentityReferenceExtractor,
            empty_inspection(),
        )
        .expect_err("unresolved known-crate reference must fail closed");

        assert!(matches!(
            error,
            CatalogueLinterError::IdentityResolutionFailed(
                CatalogueIdentityResolutionError::UnresolvedIdentifier(unresolved)
            ) if unresolved.as_str() == "domain::missing::Customer"
        ));
    }

    struct ClassificationFailureExtractor;

    impl TypeRefPathExtractorPort for ClassificationFailureExtractor {
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
    fn test_resolve_reference_identities_reports_classification_failure_for_unknown_bare_path() {
        let type_ref = TypeRef::new("ImportedButUnclassified").expect("valid TypeRef");
        let universe = BTreeSet::new();
        let catalogue_crate = CrateName::new("domain").expect("valid crate name");

        let error = resolve_reference_identities(
            &type_ref,
            identity_context(&catalogue_crate, &universe),
            &ClassificationFailureExtractor,
            empty_inspection(),
        )
        .expect_err("unknown bare paths must not be guessed as external");

        assert!(matches!(
            error,
            CatalogueLinterError::IdentityResolutionFailed(
                CatalogueIdentityResolutionError::ClassificationFailed { location }
            ) if location == type_ref
        ));
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
                &ClassificationFailureExtractor,
                empty_inspection(),
            )
            .expect("catalogue identity must win over external-root classification");

            assert_eq!(resolved, vec![expected]);
        }
    }

    #[test]
    fn test_resolve_reference_identities_classifies_unresolved_std_path_as_external() {
        let universe = BTreeSet::new();
        let catalogue_crate = CrateName::new("domain").expect("valid crate name");
        let type_ref = TypeRef::new("std::vec::Vec").expect("valid external path");

        let resolved = resolve_reference_identities(
            &type_ref,
            identity_context(&catalogue_crate, &universe),
            &ClassificationFailureExtractor,
            empty_inspection(),
        )
        .expect("unresolved standard-library path is external");

        assert!(resolved.is_empty());
    }

    #[test]
    fn test_resolve_reference_identities_classifies_supported_bare_external_paths() {
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
                &ClassificationFailureExtractor,
                empty_inspection(),
            )
            .expect("supported bare standard path is external");

            assert!(resolved.is_empty(), "unexpected catalogue identity for {path}");
        }
    }
}
