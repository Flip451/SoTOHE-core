//! Infrastructure-owned reconciliation of catalogue type notation.
//!
//! Catalogue values remain source notation. This module is the
//! implementation boundary that resolves that notation against the rustdoc
//! path table before an identity is compared or stored.

use std::collections::{BTreeSet, HashMap};

use domain::tddd::NewTypeGraphCodecError;
use domain::tddd::catalogue_v2::identifiers::{
    CrateName, FullyQualifiedItemPath, Identifier, ModulePath, ParamName, TypeRef,
};
use domain::tddd::catalogue_v2::identity_resolution::{
    CatalogueIdentityResolutionError, resolve_catalogue_identity,
};
use domain::tddd::test_obligation::ids::{DiagnosticMessage, unavailable_diagnostic_message};
use rustdoc_types::{
    AssocItemConstraint, AssocItemConstraintKind, DynTrait, GenericArg, GenericArgs, GenericBound,
    Id, ItemKind, ItemSummary, Path, Term, Type,
};

use super::type_ref_parser::{
    STD_PRELUDE_TYPES, parse_type_ref_with_generics, render_bound, render_type,
};

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
    let canonical = canonicalize_type(parsed, type_ref, catalogue_crate, &universe)?;
    let rendered = render_identity_type(&canonical).ok_or_else(|| {
        invalid_type_ref(type_ref, "the parsed type has no canonical Rust rendering")
    })?;
    Ok(CanonicalTypeIdentity(rendered))
}

fn unique_resolved_id(
    short_name: &str,
    catalogue_crate: &CrateName,
    universe: &BTreeSet<FullyQualifiedItemPath>,
    rustdoc_paths: &HashMap<Id, ItemSummary>,
) -> Option<Id> {
    let reference = TypeRef::new(short_name.to_owned()).ok()?;
    let identity = match resolve_catalogue_identity(&reference, catalogue_crate, universe) {
        Ok(identity) => identity,
        Err(CatalogueIdentityResolutionError::AmbiguousIdentifier(_, candidates))
            if candidates
                .as_slice()
                .iter()
                .all(|candidate| candidate.crate_name() == catalogue_crate) =>
        {
            // Keep the parser on the source-spelling path so the final
            // canonicalization pass reports the complete local candidate set.
            candidates.as_slice().first()?.clone()
        }
        Err(_) => return None,
    };
    if identity.crate_name() != catalogue_crate {
        return None;
    }
    rustdoc_paths.iter().find_map(|(id, summary)| {
        summary_identity(summary).filter(|candidate| candidate == &identity).map(|_| *id)
    })
}

fn render_identity_type(ty: &Type) -> Option<String> {
    match ty {
        Type::ImplTrait(bounds) => {
            let rendered = bounds.iter().map(render_bound).collect::<Option<Vec<_>>>()?;
            Some(format!("impl {}", rendered.join(" + ")))
        }
        Type::Infer => None,
        Type::Pat { type_, .. } => render_identity_type(type_),
        _ => render_type(ty),
    }
}

fn canonicalize_type(
    ty: Type,
    source: &TypeRef,
    catalogue_crate: &CrateName,
    universe: &BTreeSet<FullyQualifiedItemPath>,
) -> Result<Type, NewTypeGraphCodecError> {
    match ty {
        Type::ResolvedPath(path) => {
            let args = canonicalize_args(path.args, source, catalogue_crate, universe)?;
            let path_name = canonicalize_path(&path.path, source, catalogue_crate, universe)?;
            Ok(Type::ResolvedPath(Path { path: path_name, id: path.id, args }))
        }
        Type::Tuple(elements) => Ok(Type::Tuple(
            elements
                .into_iter()
                .map(|element| canonicalize_type(element, source, catalogue_crate, universe))
                .collect::<Result<Vec<_>, _>>()?,
        )),
        Type::Slice(inner) => {
            Ok(Type::Slice(Box::new(canonicalize_type(*inner, source, catalogue_crate, universe)?)))
        }
        Type::Array { type_, len } => Ok(Type::Array {
            type_: Box::new(canonicalize_type(*type_, source, catalogue_crate, universe)?),
            len,
        }),
        Type::Pat { type_, __pat_unstable_do_not_use } => Ok(Type::Pat {
            type_: Box::new(canonicalize_type(*type_, source, catalogue_crate, universe)?),
            __pat_unstable_do_not_use,
        }),
        Type::BorrowedRef { lifetime, is_mutable, type_ } => Ok(Type::BorrowedRef {
            lifetime,
            is_mutable,
            type_: Box::new(canonicalize_type(*type_, source, catalogue_crate, universe)?),
        }),
        Type::RawPointer { is_mutable, type_ } => Ok(Type::RawPointer {
            is_mutable,
            type_: Box::new(canonicalize_type(*type_, source, catalogue_crate, universe)?),
        }),
        Type::ImplTrait(bounds) => Ok(Type::ImplTrait(
            bounds
                .into_iter()
                .map(|bound| canonicalize_bound(bound, source, catalogue_crate, universe))
                .collect::<Result<Vec<_>, _>>()?,
        )),
        Type::DynTrait(dyn_trait) => Ok(Type::DynTrait(DynTrait {
            traits: dyn_trait
                .traits
                .into_iter()
                .map(|poly| {
                    let path =
                        canonicalize_path(&poly.trait_.path, source, catalogue_crate, universe)?;
                    let args =
                        canonicalize_args(poly.trait_.args, source, catalogue_crate, universe)?;
                    Ok(rustdoc_types::PolyTrait {
                        trait_: Path { path, id: poly.trait_.id, args },
                        generic_params: poly.generic_params,
                    })
                })
                .collect::<Result<Vec<_>, NewTypeGraphCodecError>>()?,
            lifetime: dyn_trait.lifetime,
        })),
        Type::FunctionPointer(function_pointer) => {
            let inputs = function_pointer
                .sig
                .inputs
                .into_iter()
                .map(|(name, input)| {
                    Ok((name, canonicalize_type(input, source, catalogue_crate, universe)?))
                })
                .collect::<Result<Vec<_>, NewTypeGraphCodecError>>()?;
            let output = function_pointer
                .sig
                .output
                .map(|output| canonicalize_type(output, source, catalogue_crate, universe))
                .transpose()?;
            Ok(Type::FunctionPointer(Box::new(rustdoc_types::FunctionPointer {
                sig: rustdoc_types::FunctionSignature {
                    inputs,
                    output,
                    is_c_variadic: function_pointer.sig.is_c_variadic,
                },
                generic_params: function_pointer.generic_params,
                header: function_pointer.header,
            })))
        }
        Type::QualifiedPath { name, args, self_type, trait_ } => Ok(Type::QualifiedPath {
            name,
            args: args
                .map(|args| canonicalize_args(Some(args), source, catalogue_crate, universe))
                .transpose()?
                .flatten(),
            self_type: Box::new(canonicalize_type(*self_type, source, catalogue_crate, universe)?),
            trait_: trait_
                .map(|path| {
                    Ok(Path {
                        path: canonicalize_path(&path.path, source, catalogue_crate, universe)?,
                        id: path.id,
                        args: canonicalize_args(path.args, source, catalogue_crate, universe)?,
                    })
                })
                .transpose()?,
        }),
        other => Ok(other),
    }
}

fn canonicalize_args(
    args: Option<Box<GenericArgs>>,
    source: &TypeRef,
    catalogue_crate: &CrateName,
    universe: &BTreeSet<FullyQualifiedItemPath>,
) -> Result<Option<Box<GenericArgs>>, NewTypeGraphCodecError> {
    args.map(|args| {
        canonicalize_generic_args(*args, source, catalogue_crate, universe).map(Box::new)
    })
    .transpose()
}

fn canonicalize_generic_args(
    args: GenericArgs,
    source: &TypeRef,
    catalogue_crate: &CrateName,
    universe: &BTreeSet<FullyQualifiedItemPath>,
) -> Result<GenericArgs, NewTypeGraphCodecError> {
    match args {
        GenericArgs::AngleBracketed { args, constraints } => Ok(GenericArgs::AngleBracketed {
            args: args
                .into_iter()
                .map(|arg| canonicalize_arg(arg, source, catalogue_crate, universe))
                .collect::<Result<Vec<_>, _>>()?,
            constraints: constraints
                .into_iter()
                .map(|constraint| {
                    let args =
                        canonicalize_args(constraint.args, source, catalogue_crate, universe)?;
                    let binding = match constraint.binding {
                        AssocItemConstraintKind::Equality(term) => {
                            AssocItemConstraintKind::Equality(canonicalize_term(
                                term,
                                source,
                                catalogue_crate,
                                universe,
                            )?)
                        }
                        AssocItemConstraintKind::Constraint(bounds) => {
                            AssocItemConstraintKind::Constraint(
                                bounds
                                    .into_iter()
                                    .map(|bound| {
                                        canonicalize_bound(bound, source, catalogue_crate, universe)
                                    })
                                    .collect::<Result<Vec<_>, _>>()?,
                            )
                        }
                    };
                    Ok(AssocItemConstraint { args, binding, ..constraint })
                })
                .collect::<Result<Vec<_>, NewTypeGraphCodecError>>()?,
        }),
        GenericArgs::Parenthesized { inputs, output } => Ok(GenericArgs::Parenthesized {
            inputs: inputs
                .into_iter()
                .map(|input| canonicalize_type(input, source, catalogue_crate, universe))
                .collect::<Result<Vec<_>, _>>()?,
            output: output
                .map(|output| canonicalize_type(output, source, catalogue_crate, universe))
                .transpose()?,
        }),
        GenericArgs::ReturnTypeNotation => Ok(GenericArgs::ReturnTypeNotation),
    }
}

fn canonicalize_arg(
    arg: GenericArg,
    source: &TypeRef,
    catalogue_crate: &CrateName,
    universe: &BTreeSet<FullyQualifiedItemPath>,
) -> Result<GenericArg, NewTypeGraphCodecError> {
    match arg {
        GenericArg::Type(ty) => {
            Ok(GenericArg::Type(canonicalize_type(ty, source, catalogue_crate, universe)?))
        }
        other => Ok(other),
    }
}

fn canonicalize_bound(
    bound: GenericBound,
    source: &TypeRef,
    catalogue_crate: &CrateName,
    universe: &BTreeSet<FullyQualifiedItemPath>,
) -> Result<GenericBound, NewTypeGraphCodecError> {
    match bound {
        GenericBound::TraitBound { trait_, generic_params, modifier } => {
            Ok(GenericBound::TraitBound {
                trait_: Path {
                    path: canonicalize_path(&trait_.path, source, catalogue_crate, universe)?,
                    id: trait_.id,
                    args: canonicalize_args(trait_.args, source, catalogue_crate, universe)?,
                },
                generic_params,
                modifier,
            })
        }
        other => Ok(other),
    }
}

fn canonicalize_term(
    term: Term,
    source: &TypeRef,
    catalogue_crate: &CrateName,
    universe: &BTreeSet<FullyQualifiedItemPath>,
) -> Result<Term, NewTypeGraphCodecError> {
    match term {
        Term::Type(ty) => Ok(Term::Type(canonicalize_type(ty, source, catalogue_crate, universe)?)),
        other => Ok(other),
    }
}

fn canonicalize_path(
    raw_path: &str,
    source: &TypeRef,
    catalogue_crate: &CrateName,
    universe: &BTreeSet<FullyQualifiedItemPath>,
) -> Result<String, NewTypeGraphCodecError> {
    if raw_path.strip_prefix("::").unwrap_or(raw_path) == "Self" {
        return Ok(raw_path.to_owned());
    }
    let path = TypeRef::new(raw_path.to_owned())
        .map_err(|_| invalid_type_ref(source, "path must not be empty"))?;
    let identity = resolve_catalogue_identity(&path, catalogue_crate, universe)
        .map_err(map_identity_resolution_error)?;
    let bare_name = raw_path.strip_prefix("::").unwrap_or(raw_path);
    if !bare_name.contains("::")
        && identity.crate_name() != catalogue_crate
        && !STD_PRELUDE_TYPES.contains(&bare_name)
    {
        return Err(NewTypeGraphCodecError::UnresolvedIdentifier(path));
    }
    Ok(identity.to_string())
}

fn map_identity_resolution_error(
    error: CatalogueIdentityResolutionError,
) -> NewTypeGraphCodecError {
    match error {
        CatalogueIdentityResolutionError::AmbiguousIdentifier(identifier, candidates) => {
            NewTypeGraphCodecError::AmbiguousIdentifier(identifier, candidates)
        }
        CatalogueIdentityResolutionError::UnresolvedIdentifier(type_ref) => {
            NewTypeGraphCodecError::UnresolvedIdentifier(type_ref)
        }
    }
}

fn summary_identity(summary: &ItemSummary) -> Option<FullyQualifiedItemPath> {
    if !is_type_identity_kind(summary.kind) {
        return None;
    }
    let (crate_name, rest) = summary.path.split_first()?;
    let (name, module_segments) = rest.split_last()?;
    let crate_name = CrateName::new(crate_name.clone()).ok()?;
    let name = Identifier::new(name.clone()).ok()?;
    let module_path = ModulePath::from_segments(module_segments.to_vec()).ok()?;
    Some(FullyQualifiedItemPath::new(crate_name, module_path, name))
}

fn is_type_identity_kind(kind: ItemKind) -> bool {
    matches!(
        kind,
        ItemKind::Struct
            | ItemKind::Union
            | ItemKind::Enum
            | ItemKind::TypeAlias
            | ItemKind::Trait
            | ItemKind::TraitAlias
            | ItemKind::ExternType
            | ItemKind::Primitive
    )
}

fn invalid_type_ref(type_ref: &TypeRef, reason: impl Into<String>) -> NewTypeGraphCodecError {
    let diagnostic = DiagnosticMessage::try_new(reason.into())
        .unwrap_or_else(|_| unavailable_diagnostic_message());
    NewTypeGraphCodecError::InvalidTypeRef(type_ref.clone(), diagnostic)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
mod tests {
    use super::*;
    use rustdoc_types::ItemKind;

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
}
