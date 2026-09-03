//! Infrastructure-owned reconciliation of catalogue type notation.
//!
//! Catalogue values remain source notation. This module is the
//! implementation boundary that resolves that notation against the rustdoc
//! path table before an identity is compared or stored.

use std::collections::{BTreeSet, HashMap};

use domain::tddd::NewTypeGraphCodecError;
use domain::tddd::catalogue_v2::identifiers::{
    CrateName, FullyQualifiedItemPath, ParamName, TypeRef,
};
use domain::tddd::catalogue_v2::identity_resolution::{
    CatalogueIdentityResolutionError, resolve_catalogue_identity,
};
use rustdoc_types::{GenericArgs, Id, ItemSummary, Path, Type};

use super::type_ref_parser::parse_type_ref_with_generics;

/// Reserved `ItemSummary::crate_id` used for synthetic summaries whose
/// catalogue declaration intentionally has no known module placement.
///
/// `rustdoc_types::ItemSummary` has no placement-unknown variant. The codec
/// therefore keeps the path segments human-readable (`[crate, name]`) while
/// carrying this adapter-owned marker so identity extraction can retain the
/// domain `Unplaced*` state instead of manufacturing a crate-root identity.
pub(crate) const SYNTHETIC_UNPLACED_CRATE_ID: u32 = u32::MAX;

mod canonicalization;
mod rustdoc_paths;

use canonicalization::{
    canonicalize_generic_args, canonicalize_type, render_identity_type, unique_resolved_id,
};
pub(crate) use rustdoc_paths::{
    canonicalize_function_identity_path, canonicalize_rustdoc_root_path,
};
use rustdoc_paths::{canonicalize_path, invalid_type_ref, summary_identity};

/// Canonical identity of a catalogue type expression after implementation matching.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CanonicalTypeIdentity(String);

impl CanonicalTypeIdentity {
    /// Returns the canonical identity text for diagnostics and rustdoc matching.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for CanonicalTypeIdentity {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// Shared definition-path authority for one reconciliation pass.
#[derive(Debug, Clone, Default)]
pub(crate) struct DefinitionPathAuthority {
    primary: BTreeSet<FullyQualifiedItemPath>,
    fallback: BTreeSet<FullyQualifiedItemPath>,
}

impl DefinitionPathAuthority {
    pub(crate) fn from_path_maps(
        primary: &HashMap<Id, ItemSummary>,
        fallbacks: &[&HashMap<Id, ItemSummary>],
    ) -> Self {
        let primary = primary.values().filter_map(summary_identity).collect::<BTreeSet<_>>();
        let fallback = fallbacks
            .iter()
            .flat_map(|paths| paths.values().filter_map(summary_identity))
            .collect::<BTreeSet<_>>();
        Self { primary, fallback }
    }

    fn resolve(
        &self,
        reference: &TypeRef,
        catalogue_crate: &CrateName,
    ) -> Result<FullyQualifiedItemPath, CatalogueIdentityResolutionError> {
        match resolve_catalogue_identity(reference, catalogue_crate, &self.primary) {
            Ok(identity) => Ok(identity),
            Err(CatalogueIdentityResolutionError::UnresolvedIdentifier(_))
                if !self.fallback.is_empty() =>
            {
                resolve_catalogue_identity(reference, catalogue_crate, &self.fallback)
            }
            Err(error) => Err(error),
        }
    }
}

/// Resolves loose catalogue TypeRef notation into the infrastructure identity form.
///
/// Parsing is delegated to the existing type_ref_parser authority. Generic
/// parameters are classified during that parse, and every path node is then
/// resolved against rustdoc_paths.
///
/// # Errors
///
/// Returns InvalidTypeRef for syntax or rendering failures,
/// AmbiguousIdentifier for multiple candidates, and UnresolvedIdentifier when
/// no rustdoc path identifies a referenced item.
pub fn canonicalize_catalogue_type_ref(
    type_ref: &TypeRef,
    catalogue_crate: &CrateName,
    rustdoc_paths: &HashMap<Id, ItemSummary>,
    generic_params: &[ParamName],
) -> Result<CanonicalTypeIdentity, NewTypeGraphCodecError> {
    let universe = rustdoc_paths.values().filter_map(summary_identity).collect::<BTreeSet<_>>();
    let generic_names = generic_params.iter().map(|param| param.as_str()).collect::<Vec<_>>();
    let external_crates = HashMap::new();
    let parsed = parse_type_ref_with_generics(
        type_ref.as_str(),
        &|name| unique_resolved_id(name, catalogue_crate, &universe, rustdoc_paths),
        1,
        &external_crates,
        &mut |_| 1,
        &generic_names,
    )
    .map_err(|reason| invalid_type_ref(type_ref, reason))?;
    let authority = DefinitionPathAuthority::from_path_maps(rustdoc_paths, &[]);
    let canonical = canonicalize_type(parsed, type_ref, catalogue_crate, &authority, None)?;
    let rendered = render_identity_type(&canonical).ok_or_else(|| {
        invalid_type_ref(type_ref, "the parsed type has no canonical Rust rendering")
    })?;
    Ok(CanonicalTypeIdentity(rendered))
}

/// Resolves one rustdoc path through the same catalogue-identity boundary used by
/// catalogue codecs and signal producers.
pub(crate) fn canonicalize_rustdoc_path(
    path: &Path,
    catalogue_crate: &CrateName,
    rustdoc_paths: &HashMap<Id, ItemSummary>,
    authority: &DefinitionPathAuthority,
) -> Result<String, NewTypeGraphCodecError> {
    let source = TypeRef::new(path.path.clone())
        .map_err(|_| invalid_type_ref("rustdoc_path", "rustdoc path is not a valid TypeRef"))?;
    canonicalize_path(
        &path.path,
        &source,
        catalogue_crate,
        authority,
        Some(path.id),
        Some(rustdoc_paths),
    )
}

pub(crate) fn canonicalize_rustdoc_type_with_authority(
    ty: &Type,
    catalogue_crate: &CrateName,
    rustdoc_paths: &HashMap<Id, ItemSummary>,
    authority: &DefinitionPathAuthority,
) -> Result<Type, NewTypeGraphCodecError> {
    let source = TypeRef::new("rustdoc_type".to_owned())
        .map_err(|_| invalid_type_ref("rustdoc_type", "failed to construct an internal TypeRef"))?;
    canonicalize_type(ty.clone(), &source, catalogue_crate, authority, Some(rustdoc_paths))
}

/// Canonicalizes generic arguments and all path-bearing constraint nodes through
/// the shared definition-path resolver.
pub(crate) fn canonicalize_rustdoc_generic_args_with_authority(
    args: &GenericArgs,
    catalogue_crate: &CrateName,
    rustdoc_paths: &HashMap<Id, ItemSummary>,
    authority: &DefinitionPathAuthority,
) -> Result<GenericArgs, NewTypeGraphCodecError> {
    let source = TypeRef::new("rustdoc_type".to_owned())
        .map_err(|_| invalid_type_ref("rustdoc_type", "failed to construct an internal TypeRef"))?;
    canonicalize_generic_args(
        args.clone(),
        &source,
        catalogue_crate,
        authority,
        Some(rustdoc_paths),
    )
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
mod tests {
    use super::*;
    use rustdoc_types::{GenericArg, ItemKind};

    fn paths(entries: &[(u32, &[&str])]) -> HashMap<Id, ItemSummary> {
        entries
            .iter()
            .map(|(id, path)| {
                (
                    Id(*id),
                    ItemSummary {
                        crate_id: 0,
                        path: path.iter().map(|segment| (*segment).to_owned()).collect(),
                        kind: ItemKind::Struct,
                    },
                )
            })
            .collect()
    }

    fn canonical(
        source: &str,
        rustdoc_paths: &HashMap<Id, ItemSummary>,
        generic_params: &[&str],
    ) -> Result<CanonicalTypeIdentity, NewTypeGraphCodecError> {
        let type_ref = TypeRef::new(source).expect("test TypeRef must be non-empty");
        let crate_name = CrateName::new("domain").expect("test crate name must be valid");
        let params = generic_params
            .iter()
            .map(|name| ParamName::new(*name).expect("test generic name must be valid"))
            .collect::<Vec<_>>();
        canonicalize_catalogue_type_ref(&type_ref, &crate_name, rustdoc_paths, &params)
    }

    fn generic_args_with_type_path(path_id: u32, path: &str) -> GenericArgs {
        GenericArgs::AngleBracketed {
            args: vec![GenericArg::Type(Type::ResolvedPath(Path {
                path: path.to_owned(),
                id: Id(path_id),
                args: None,
            }))],
            constraints: vec![],
        }
    }

    #[test]
    fn test_canonicalize_unique_short_name_uses_rustdoc_path() {
        let rustdoc_paths = paths(&[(1, &["domain", "a", "Thing"])]);
        let identity = canonical("Thing", &rustdoc_paths, &[]).expect("short name resolves");
        assert_eq!(identity.as_str(), "domain::a::Thing");
    }

    #[test]
    fn test_canonicalize_crate_prefixed_name_preserves_full_path() {
        let rustdoc_paths = paths(&[(1, &["domain", "a", "Thing"])]);
        let identity = canonical("domain::a::Thing", &rustdoc_paths, &[])
            .expect("crate-qualified name resolves");
        assert_eq!(identity.to_string(), "domain::a::Thing");
    }

    #[test]
    fn test_canonicalize_exact_cross_crate_path_precedes_local_suffix() {
        let rustdoc_paths =
            paths(&[(1, &["domain", "serde", "Serialize"]), (2, &["serde", "Serialize"])]);
        let identity = canonical("serde::Serialize", &rustdoc_paths, &[])
            .expect("the exact cross-crate path resolves");
        assert_eq!(identity.as_str(), "serde::Serialize");
    }

    #[test]
    fn test_canonicalize_std_prelude_name_uses_std_path() {
        let rustdoc_paths = paths(&[(1, &["std", "option", "Option"])]);
        let identity = canonical("Option", &rustdoc_paths, &[]).expect("prelude name resolves");
        assert_eq!(identity.as_str(), "std::option::Option");
    }

    #[test]
    fn test_canonicalize_prelude_traits_uses_rustdoc_definition_paths() {
        let rustdoc_paths = paths(&[
            (1, &["core", "iter", "traits", "iterator", "Iterator"]),
            (5, &["core", "iter", "traits", "collect", "IntoIterator"]),
            (6, &["core", "iter", "traits", "double_ended", "DoubleEndedIterator"]),
            (7, &["core", "iter", "traits", "exact_size", "ExactSizeIterator"]),
            (2, &["core", "ops", "deref", "Deref"]),
            (3, &["core", "ops", "function", "FnOnce"]),
            (4, &["core", "ops", "Drop"]),
        ]);

        for (source, expected) in [
            ("Iterator", "core::iter::traits::iterator::Iterator"),
            ("IntoIterator", "core::iter::traits::collect::IntoIterator"),
            ("DoubleEndedIterator", "core::iter::traits::double_ended::DoubleEndedIterator"),
            ("ExactSizeIterator", "core::iter::traits::exact_size::ExactSizeIterator"),
            ("Deref", "core::ops::deref::Deref"),
            ("FnOnce", "core::ops::function::FnOnce"),
            ("Drop", "core::ops::Drop"),
        ] {
            let identity = canonical(source, &rustdoc_paths, &[]).expect("prelude trait resolves");
            assert_eq!(identity.as_str(), expected, "unexpected identity for {source}");
        }
    }

    #[test]
    fn test_canonicalize_explicit_std_reexports_uses_rustdoc_definition_paths() {
        let rustdoc_paths = paths(&[
            (1, &["core", "iter", "traits", "iterator", "Iterator"]),
            (4, &["core", "iter", "traits", "collect", "IntoIterator"]),
            (5, &["core", "iter", "traits", "double_ended", "DoubleEndedIterator"]),
            (6, &["core", "iter", "traits", "exact_size", "ExactSizeIterator"]),
            (2, &["core", "ops", "deref", "Deref"]),
            (3, &["core", "ops", "function", "FnOnce"]),
        ]);

        for (source, expected) in [
            ("std::iter::Iterator", "core::iter::traits::iterator::Iterator"),
            ("std::iter::IntoIterator", "core::iter::traits::collect::IntoIterator"),
            (
                "std::iter::DoubleEndedIterator",
                "core::iter::traits::double_ended::DoubleEndedIterator",
            ),
            ("std::iter::ExactSizeIterator", "core::iter::traits::exact_size::ExactSizeIterator"),
            ("std::ops::Deref", "core::ops::deref::Deref"),
            ("std::ops::FnOnce", "core::ops::function::FnOnce"),
        ] {
            let identity = canonical(source, &rustdoc_paths, &[])
                .expect("explicit standard-library path resolves");
            assert_eq!(identity.as_str(), expected, "unexpected identity for {source}");
        }
    }

    #[test]
    fn test_canonicalize_generic_parameter_and_nested_generic_are_rewritten_together() {
        let rustdoc_paths = paths(&[(1, &["std", "option", "Option"])]);
        let identity =
            canonical("Option<T>", &rustdoc_paths, &["T"]).expect("generic type resolves");
        assert_eq!(identity.as_str(), "std::option::Option<T>");
    }

    #[test]
    fn test_canonicalize_incrate_duplicate_names_remain_distinct() {
        let rustdoc_paths =
            paths(&[(1, &["domain", "alpha", "Shared"]), (2, &["domain", "beta", "Shared"])]);
        let alpha: CanonicalTypeIdentity = canonical("domain::alpha::Shared", &rustdoc_paths, &[])
            .expect("the alpha in-crate path resolves");
        let beta: CanonicalTypeIdentity = canonical("domain::beta::Shared", &rustdoc_paths, &[])
            .expect("the beta in-crate path resolves");

        assert_eq!(alpha.as_str(), "domain::alpha::Shared");
        assert_eq!(beta.as_str(), "domain::beta::Shared");
        assert_ne!(alpha, beta, "fully qualified in-crate paths must remain distinct identities");
    }

    #[test]
    fn test_canonicalize_borrowed_type_uses_authoritative_alloc_path() {
        let rustdoc_paths = paths(&[
            (1, &["alloc", "collections", "btree", "map", "BTreeMap"]),
            (2, &["domain", "CatalogueEntryKey"]),
            (3, &["domain", "TypeEntry"]),
        ]);
        let identity = canonical("&BTreeMap<CatalogueEntryKey, TypeEntry>", &rustdoc_paths, &[])
            .expect("borrowed generic type resolves through rustdoc paths");
        assert_eq!(
            identity.as_str(),
            "&alloc::collections::btree::map::BTreeMap<domain::CatalogueEntryKey, domain::TypeEntry>"
        );
    }

    #[test]
    fn test_canonicalize_prelude_result_ignores_unrelated_result_aliases() {
        let rustdoc_paths = paths(&[
            (1, &["core", "result", "Result"]),
            (2, &["core", "fmt", "Result"]),
            (3, &["std", "io", "error", "Result"]),
        ]);
        let identity = canonical("Result", &rustdoc_paths, &[])
            .expect("the result prelude path resolves uniquely");
        assert_eq!(identity.as_str(), "core::result::Result");
    }

    #[test]
    fn test_canonicalize_bare_external_name_does_not_use_unqualified_identity() {
        let rustdoc_paths = paths(&[(1, &["anyhow", "Result"])]);

        let error = canonical("Result", &rustdoc_paths, &[])
            .expect_err("an external identity must not satisfy a bare catalogue name");

        assert!(matches!(error, NewTypeGraphCodecError::UnresolvedIdentifier(_)));
        assert!(error.to_string().contains("Result"));
    }

    #[test]
    fn test_canonicalize_bare_non_prelude_external_name_fails_closed() {
        let rustdoc_paths = paths(&[(1, &["anyhow", "Error"])]);

        let error = canonical("Error", &rustdoc_paths, &[])
            .expect_err("a non-prelude external identity must require qualification");

        assert!(matches!(error, NewTypeGraphCodecError::UnresolvedIdentifier(_)));
        assert!(error.to_string().contains("Error"));
    }

    #[test]
    fn test_canonicalize_ambiguous_short_name_reports_all_candidates() {
        let rustdoc_paths =
            paths(&[(1, &["domain", "a", "Thing"]), (2, &["domain", "b", "Thing"])]);
        let error = canonical("Thing", &rustdoc_paths, &[]).expect_err("name is ambiguous");
        let NewTypeGraphCodecError::AmbiguousIdentifier(identifier, candidates) = error else {
            panic!("expected ambiguous identifier");
        };
        assert_eq!(identifier.as_str(), "Thing");
        let candidates = candidates.as_slice().iter().map(ToString::to_string).collect::<Vec<_>>();
        assert_eq!(candidates, vec!["domain::a::Thing", "domain::b::Thing"]);
    }

    #[test]
    fn test_canonicalize_unresolved_name_fails_closed() {
        let error = canonical("Missing", &HashMap::new(), &[]).expect_err("missing path fails");
        assert!(matches!(error, NewTypeGraphCodecError::UnresolvedIdentifier(_)));
        assert!(error.to_string().contains("Missing"));
    }

    #[test]
    fn test_canonicalize_unresolved_qualified_name_does_not_use_unrelated_path_id() {
        let rustdoc_paths = paths(&[(1, &["domain", "a", "Thing"])]);
        let error = canonical("external::Missing", &rustdoc_paths, &[])
            .expect_err("unresolved qualified path must fail closed");
        assert!(matches!(error, NewTypeGraphCodecError::UnresolvedIdentifier(_)));
        assert!(error.to_string().contains("external::Missing"));
    }

    #[test]
    fn test_canonicalize_rustdoc_path_uses_authoritative_path_id() {
        let rustdoc_paths =
            paths(&[(1, &["domain", "alpha", "Shared"]), (2, &["domain", "beta", "Shared"])]);
        let authority = DefinitionPathAuthority::from_path_maps(&rustdoc_paths, &[]);
        let ty = Type::ResolvedPath(Path { path: "Shared".to_owned(), id: Id(1), args: None });

        let canonical = canonicalize_rustdoc_type_with_authority(
            &ty,
            &CrateName::new("domain").expect("valid crate"),
            &rustdoc_paths,
            &authority,
        )
        .expect("the authoritative id selects alpha");

        assert!(matches!(
            canonical,
            Type::ResolvedPath(Path { path, .. }) if path == "domain::alpha::Shared"
        ));
    }

    #[test]
    fn test_canonicalize_rustdoc_path_missing_authoritative_id_falls_back_to_spelling() {
        let rustdoc_paths = paths(&[(1, &["domain", "alpha", "Shared"])]);
        let authority = DefinitionPathAuthority::from_path_maps(&rustdoc_paths, &[]);
        let ty = Type::ResolvedPath(Path { path: "Shared".to_owned(), id: Id(99), args: None });

        let canonical = canonicalize_rustdoc_type_with_authority(
            &ty,
            &CrateName::new("domain").expect("valid crate"),
            &rustdoc_paths,
            &authority,
        )
        .expect("a missing path summary falls back to spelling resolution");

        assert!(matches!(
            canonical,
            Type::ResolvedPath(Path { path, .. }) if path == "domain::alpha::Shared"
        ));
    }

    #[test]
    fn test_canonicalize_trait_impl_generic_args_synthetic_id_falls_back_to_spelling() {
        let rustdoc_paths = paths(&[(1, &["domain", "errors", "PrimitiveOccurrenceScanError"])]);
        let authority = DefinitionPathAuthority::from_path_maps(&rustdoc_paths, &[]);
        let crate_name = CrateName::new("domain").expect("valid crate");

        for synthetic_id in [u32::MAX, u32::MAX - 1] {
            let canonical = canonicalize_rustdoc_generic_args_with_authority(
                &generic_args_with_type_path(synthetic_id, "PrimitiveOccurrenceScanError"),
                &crate_name,
                &rustdoc_paths,
                &authority,
            )
            .expect("a synthetic path id must use spelling-based authority resolution");

            assert!(matches!(
                canonical,
                GenericArgs::AngleBracketed { args, .. }
                    if matches!(
                        args.as_slice(),
                        [GenericArg::Type(Type::ResolvedPath(Path { path, .. }))]
                            if path == "domain::errors::PrimitiveOccurrenceScanError"
                    )
            ));
        }
    }

    #[test]
    fn test_canonicalize_trait_impl_generic_args_unresolved_synthetic_id_fails_closed() {
        let rustdoc_paths = paths(&[(1, &["domain", "errors", "KnownError"])]);
        let authority = DefinitionPathAuthority::from_path_maps(&rustdoc_paths, &[]);
        let error = canonicalize_rustdoc_generic_args_with_authority(
            &generic_args_with_type_path(u32::MAX, "MissingError"),
            &CrateName::new("domain").expect("valid crate"),
            &rustdoc_paths,
            &authority,
        )
        .expect_err("an unresolved spelling must remain fail-closed");

        assert!(matches!(
            error,
            NewTypeGraphCodecError::UnresolvedIdentifier(identifier)
                if identifier.as_str() == "MissingError"
        ));
    }

    #[test]
    fn test_canonicalize_trait_impl_generic_args_existing_id_uses_authoritative_summary() {
        let rustdoc_paths =
            paths(&[(1, &["domain", "alpha", "Shared"]), (2, &["domain", "beta", "Shared"])]);
        let authority = DefinitionPathAuthority::from_path_maps(&rustdoc_paths, &[]);
        let canonical = canonicalize_rustdoc_generic_args_with_authority(
            &generic_args_with_type_path(1, "Shared"),
            &CrateName::new("domain").expect("valid crate"),
            &rustdoc_paths,
            &authority,
        )
        .expect("an existing path id must use its authoritative summary");

        assert!(matches!(
            canonical,
            GenericArgs::AngleBracketed { args, .. }
                if matches!(
                    args.as_slice(),
                    [GenericArg::Type(Type::ResolvedPath(Path { path, .. }))]
                        if path == "domain::alpha::Shared"
                )
        ));
    }

    #[test]
    fn test_canonicalize_infer_type_fails_closed() {
        let error = canonical("_", &HashMap::new(), &[]).expect_err("infer is not an identity");
        assert!(matches!(error, NewTypeGraphCodecError::InvalidTypeRef(..)));
    }

    #[test]
    fn test_canonicalize_short_name_ignores_non_type_path_candidates() {
        let mut rustdoc_paths = paths(&[(1, &["domain", "a", "Thing"])]);
        rustdoc_paths.insert(
            Id(2),
            ItemSummary {
                crate_id: 0,
                path: vec!["domain".to_owned(), "b".to_owned(), "Thing".to_owned()],
                kind: ItemKind::Function,
            },
        );
        let identity = canonical("Thing", &rustdoc_paths, &[])
            .expect("a function with the same short name is not a type candidate");
        assert_eq!(identity.as_str(), "domain::a::Thing");
    }

    #[test]
    fn test_canonicalize_impl_generic_and_incrate_paths_share_authoritative_universe() {
        let rustdoc_paths = paths(&[
            (1, &["core", "iter", "traits", "iterator", "Iterator"]),
            (2, &["domain", "alpha", "Shared"]),
            (3, &["domain", "beta", "Shared"]),
        ]);

        let identity = canonical(
            "impl std::iter::Iterator<Item = domain::alpha::Shared>",
            &rustdoc_paths,
            &[],
        )
        .expect("impl trait and its associated generic type resolve");

        assert_eq!(
            identity.as_str(),
            "impl core::iter::traits::iterator::Iterator<Item = domain::alpha::Shared>"
        );
        assert!(!identity.as_str().contains("domain::beta::Shared"));
    }

    #[test]
    fn test_canonicalize_loose_module_path_preserves_declaration_boundary() {
        let rustdoc_paths =
            paths(&[(1, &["domain", "alpha", "Shared"]), (2, &["domain", "beta", "Shared"])]);

        for (source, expected) in
            [("alpha::Shared", "domain::alpha::Shared"), ("beta::Shared", "domain::beta::Shared")]
        {
            let identity = canonical(source, &rustdoc_paths, &[])
                .expect("module-qualified loose notation resolves");
            assert_eq!(identity.as_str(), expected, "unexpected identity for {source}");
        }
    }

    #[test]
    fn test_canonicalize_catalogue_type_ref_observes_value_identity_and_rejects_absent_reference() {
        let rustdoc_paths = paths(&[(1, &["domain", "generated", "AddedType"])]);
        let type_ref = TypeRef::new("AddedType".to_owned()).expect("valid TypeRef");
        let crate_name = CrateName::new("domain").expect("valid crate name");

        let identity = canonicalize_catalogue_type_ref(&type_ref, &crate_name, &rustdoc_paths, &[])
            .expect("the public canonicalization entry resolves the declared identity");
        assert_eq!(identity.as_str(), "domain::generated::AddedType");
        assert_eq!(identity.to_string(), identity.as_str());

        let missing = TypeRef::new("MissingFromBoth".to_owned()).expect("valid TypeRef");
        let error = canonicalize_catalogue_type_ref(&missing, &crate_name, &rustdoc_paths, &[])
            .expect_err("a TypeRef absent from rustdoc and the catalogue universe fails closed");
        assert!(matches!(error, NewTypeGraphCodecError::UnresolvedIdentifier(_)));
    }

    #[test]
    fn test_canonicalize_function_identity_applies_bin_root_alias_once() {
        let package = CrateName::new("cli").expect("valid package name");
        let rustdoc_root = CrateName::new("sotp").expect("valid rustdoc root");
        let path = vec!["sotp".to_owned(), "commands".to_owned(), "run".to_owned()];

        assert_eq!(
            canonicalize_function_identity_path(&path, Some(&package), Some(&rustdoc_root)),
            "cli::commands::run"
        );
        assert_eq!(
            canonicalize_function_identity_path(
                &["serde".to_owned(), "Serialize".to_owned()],
                Some(&package),
                Some(&rustdoc_root),
            ),
            "serde::Serialize"
        );
    }
}
