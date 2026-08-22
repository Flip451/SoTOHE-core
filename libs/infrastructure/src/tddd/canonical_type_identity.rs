//! Infrastructure-owned reconciliation of catalogue type notation.
//!
//! Catalogue values remain source notation. This module is the
//! implementation boundary that resolves that notation against the rustdoc
//! path table before an identity is compared or stored.

use std::collections::{BTreeMap, HashMap};

use domain::tddd::NewTypeGraphCodecError;
use domain::tddd::catalogue_v2::identifiers::{
    CrateName, FullyQualifiedItemPath, Identifier, ModulePath, ParamName, TypeRef,
};
use domain::tddd::catalogue_v2::roles::NonEmptyVec;
use domain::tddd::test_obligation::ids::{DiagnosticMessage, unavailable_diagnostic_message};
use rustdoc_types::{
    AssocItemConstraint, AssocItemConstraintKind, DynTrait, GenericArg, GenericArgs, GenericBound,
    Id, ItemKind, ItemSummary, Path, Term, Type,
};

use super::type_ref_parser::{
    STD_PRELUDE_TYPES, parse_syn_type, parse_type_ref_with_generics, render_bound, render_type,
    std_canonical_path,
};
use syn::visit::Visit;

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
    let generic_names = generic_params.iter().map(|param| param.as_str()).collect::<Vec<_>>();
    let external_crates = HashMap::new();
    reject_ambiguous_bare_paths(type_ref.as_str(), generic_params, catalogue_crate, rustdoc_paths)?;
    let parsed = parse_type_ref_with_generics(
        type_ref.as_str(),
        &|name| unique_local_id(name, catalogue_crate, rustdoc_paths),
        1,
        &external_crates,
        &mut |_| 1,
        &generic_names,
    )
    .map_err(|reason| invalid_type_ref(type_ref, reason))?;
    let canonical = canonicalize_type(parsed, type_ref, catalogue_crate, rustdoc_paths)?;
    let rendered = render_identity_type(&canonical).ok_or_else(|| {
        invalid_type_ref(type_ref, "the parsed type has no canonical Rust rendering")
    })?;
    Ok(CanonicalTypeIdentity(rendered))
}

fn unique_local_id(
    short_name: &str,
    catalogue_crate: &CrateName,
    rustdoc_paths: &HashMap<Id, ItemSummary>,
) -> Option<Id> {
    let mut matches = rustdoc_paths.iter().filter(|(_, summary)| {
        summary_identity(summary).is_some_and(|identity| {
            identity.crate_name() == catalogue_crate && identity.name().as_str() == short_name
        })
    });
    let (id, _) = matches.next()?;
    matches.next().is_none().then_some(*id)
}

fn reject_ambiguous_bare_paths(
    source: &str,
    generic_params: &[ParamName],
    catalogue_crate: &CrateName,
    rustdoc_paths: &HashMap<Id, ItemSummary>,
) -> Result<(), NewTypeGraphCodecError> {
    let Ok(syntax) = parse_syn_type(source) else {
        return Ok(());
    };
    let generic_names = generic_params.iter().map(|param| param.as_str()).collect::<Vec<_>>();
    let mut finder = AmbiguousPathFinder {
        catalogue_crate,
        generic_names: &generic_names,
        rustdoc_paths,
        found: None,
    };
    finder.visit_type(&syntax);
    let Some(name) = finder.found else {
        return Ok(());
    };
    let Ok(type_ref) = TypeRef::new(source.to_owned()) else {
        return Ok(());
    };
    let candidate_name = name.clone();
    let identifier = Identifier::new(name)
        .map_err(|_| invalid_type_ref(&type_ref, "path contains an invalid identifier"))?;
    let candidates = rustdoc_paths
        .values()
        .filter_map(summary_identity)
        .filter(|identity| {
            identity.crate_name() == catalogue_crate && identity.name().as_str() == candidate_name
        })
        .collect::<Vec<_>>();
    let mut candidates = candidates;
    candidates.sort();
    let Some((first, rest)) = candidates.split_first() else {
        return Err(NewTypeGraphCodecError::UnresolvedIdentifier(type_ref));
    };
    Err(NewTypeGraphCodecError::AmbiguousIdentifier(
        identifier,
        NonEmptyVec::new(first.clone(), rest.to_vec()),
    ))
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

struct AmbiguousPathFinder<'a> {
    catalogue_crate: &'a CrateName,
    generic_names: &'a [&'a str],
    rustdoc_paths: &'a HashMap<Id, ItemSummary>,
    found: Option<String>,
}

impl<'ast> Visit<'ast> for AmbiguousPathFinder<'_> {
    fn visit_path(&mut self, path: &'ast syn::Path) {
        if self.found.is_none()
            && path.leading_colon.is_none()
            && path.segments.len() == 1
            && let Some(segment) = path.segments.first()
        {
            let name = segment.ident.to_string();
            let is_generic = self.generic_names.contains(&name.as_str());
            let local_count = self
                .rustdoc_paths
                .values()
                .filter_map(summary_identity)
                .filter(|identity| {
                    identity.crate_name() == self.catalogue_crate
                        && identity.name().as_str() == name
                })
                .count();
            if !is_generic && local_count > 1 {
                self.found = Some(name);
            }
        }
        syn::visit::visit_path(self, path);
    }
}

fn canonicalize_type(
    ty: Type,
    source: &TypeRef,
    catalogue_crate: &CrateName,
    rustdoc_paths: &HashMap<Id, ItemSummary>,
) -> Result<Type, NewTypeGraphCodecError> {
    match ty {
        Type::ResolvedPath(path) => {
            let args = canonicalize_args(path.args, source, catalogue_crate, rustdoc_paths)?;
            let path_name = canonicalize_path(&path.path, source, catalogue_crate, rustdoc_paths)?;
            Ok(Type::ResolvedPath(Path { path: path_name, id: path.id, args }))
        }
        Type::Tuple(elements) => Ok(Type::Tuple(
            elements
                .into_iter()
                .map(|element| canonicalize_type(element, source, catalogue_crate, rustdoc_paths))
                .collect::<Result<Vec<_>, _>>()?,
        )),
        Type::Slice(inner) => Ok(Type::Slice(Box::new(canonicalize_type(
            *inner,
            source,
            catalogue_crate,
            rustdoc_paths,
        )?))),
        Type::Array { type_, len } => Ok(Type::Array {
            type_: Box::new(canonicalize_type(*type_, source, catalogue_crate, rustdoc_paths)?),
            len,
        }),
        Type::Pat { type_, __pat_unstable_do_not_use } => Ok(Type::Pat {
            type_: Box::new(canonicalize_type(*type_, source, catalogue_crate, rustdoc_paths)?),
            __pat_unstable_do_not_use,
        }),
        Type::BorrowedRef { lifetime, is_mutable, type_ } => Ok(Type::BorrowedRef {
            lifetime,
            is_mutable,
            type_: Box::new(canonicalize_type(*type_, source, catalogue_crate, rustdoc_paths)?),
        }),
        Type::RawPointer { is_mutable, type_ } => Ok(Type::RawPointer {
            is_mutable,
            type_: Box::new(canonicalize_type(*type_, source, catalogue_crate, rustdoc_paths)?),
        }),
        Type::ImplTrait(bounds) => Ok(Type::ImplTrait(
            bounds
                .into_iter()
                .map(|bound| canonicalize_bound(bound, source, catalogue_crate, rustdoc_paths))
                .collect::<Result<Vec<_>, _>>()?,
        )),
        Type::DynTrait(dyn_trait) => Ok(Type::DynTrait(DynTrait {
            traits: dyn_trait
                .traits
                .into_iter()
                .map(|poly| {
                    let path = canonicalize_path(
                        &poly.trait_.path,
                        source,
                        catalogue_crate,
                        rustdoc_paths,
                    )?;
                    let args = canonicalize_args(
                        poly.trait_.args,
                        source,
                        catalogue_crate,
                        rustdoc_paths,
                    )?;
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
                    Ok((name, canonicalize_type(input, source, catalogue_crate, rustdoc_paths)?))
                })
                .collect::<Result<Vec<_>, NewTypeGraphCodecError>>()?;
            let output = function_pointer
                .sig
                .output
                .map(|output| canonicalize_type(output, source, catalogue_crate, rustdoc_paths))
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
                .map(|args| canonicalize_args(Some(args), source, catalogue_crate, rustdoc_paths))
                .transpose()?
                .flatten(),
            self_type: Box::new(canonicalize_type(
                *self_type,
                source,
                catalogue_crate,
                rustdoc_paths,
            )?),
            trait_: trait_
                .map(|path| {
                    Ok(Path {
                        path: canonicalize_path(
                            &path.path,
                            source,
                            catalogue_crate,
                            rustdoc_paths,
                        )?,
                        id: path.id,
                        args: canonicalize_args(path.args, source, catalogue_crate, rustdoc_paths)?,
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
    rustdoc_paths: &HashMap<Id, ItemSummary>,
) -> Result<Option<Box<GenericArgs>>, NewTypeGraphCodecError> {
    args.map(|args| {
        canonicalize_generic_args(*args, source, catalogue_crate, rustdoc_paths).map(Box::new)
    })
    .transpose()
}

fn canonicalize_generic_args(
    args: GenericArgs,
    source: &TypeRef,
    catalogue_crate: &CrateName,
    rustdoc_paths: &HashMap<Id, ItemSummary>,
) -> Result<GenericArgs, NewTypeGraphCodecError> {
    match args {
        GenericArgs::AngleBracketed { args, constraints } => Ok(GenericArgs::AngleBracketed {
            args: args
                .into_iter()
                .map(|arg| canonicalize_arg(arg, source, catalogue_crate, rustdoc_paths))
                .collect::<Result<Vec<_>, _>>()?,
            constraints: constraints
                .into_iter()
                .map(|constraint| {
                    let args =
                        canonicalize_args(constraint.args, source, catalogue_crate, rustdoc_paths)?;
                    let binding = match constraint.binding {
                        AssocItemConstraintKind::Equality(term) => {
                            AssocItemConstraintKind::Equality(canonicalize_term(
                                term,
                                source,
                                catalogue_crate,
                                rustdoc_paths,
                            )?)
                        }
                        AssocItemConstraintKind::Constraint(bounds) => {
                            AssocItemConstraintKind::Constraint(
                                bounds
                                    .into_iter()
                                    .map(|bound| {
                                        canonicalize_bound(
                                            bound,
                                            source,
                                            catalogue_crate,
                                            rustdoc_paths,
                                        )
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
                .map(|input| canonicalize_type(input, source, catalogue_crate, rustdoc_paths))
                .collect::<Result<Vec<_>, _>>()?,
            output: output
                .map(|output| canonicalize_type(output, source, catalogue_crate, rustdoc_paths))
                .transpose()?,
        }),
        GenericArgs::ReturnTypeNotation => Ok(GenericArgs::ReturnTypeNotation),
    }
}

fn canonicalize_arg(
    arg: GenericArg,
    source: &TypeRef,
    catalogue_crate: &CrateName,
    rustdoc_paths: &HashMap<Id, ItemSummary>,
) -> Result<GenericArg, NewTypeGraphCodecError> {
    match arg {
        GenericArg::Type(ty) => {
            Ok(GenericArg::Type(canonicalize_type(ty, source, catalogue_crate, rustdoc_paths)?))
        }
        other => Ok(other),
    }
}

fn canonicalize_bound(
    bound: GenericBound,
    source: &TypeRef,
    catalogue_crate: &CrateName,
    rustdoc_paths: &HashMap<Id, ItemSummary>,
) -> Result<GenericBound, NewTypeGraphCodecError> {
    match bound {
        GenericBound::TraitBound { trait_, generic_params, modifier } => {
            Ok(GenericBound::TraitBound {
                trait_: Path {
                    path: canonicalize_path(&trait_.path, source, catalogue_crate, rustdoc_paths)?,
                    id: trait_.id,
                    args: canonicalize_args(trait_.args, source, catalogue_crate, rustdoc_paths)?,
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
    rustdoc_paths: &HashMap<Id, ItemSummary>,
) -> Result<Term, NewTypeGraphCodecError> {
    match term {
        Term::Type(ty) => {
            Ok(Term::Type(canonicalize_type(ty, source, catalogue_crate, rustdoc_paths)?))
        }
        other => Ok(other),
    }
}

fn canonicalize_path(
    raw_path: &str,
    source: &TypeRef,
    catalogue_crate: &CrateName,
    rustdoc_paths: &HashMap<Id, ItemSummary>,
) -> Result<String, NewTypeGraphCodecError> {
    let lookup = raw_path.strip_prefix("::").unwrap_or(raw_path);
    if lookup == "Self" {
        return Ok(raw_path.to_owned());
    }

    let candidates = matching_candidates(lookup, catalogue_crate, rustdoc_paths);

    match candidates.len() {
        0 => Err(NewTypeGraphCodecError::UnresolvedIdentifier(source.clone())),
        1 => {
            let Some((_, identity)) = candidates.into_iter().next() else {
                return Err(NewTypeGraphCodecError::UnresolvedIdentifier(source.clone()));
            };
            Ok(identity.to_string())
        }
        _ => {
            let identifier = Identifier::new(lookup.rsplit("::").next().unwrap_or(lookup))
                .map_err(|_| invalid_type_ref(source, "path contains an invalid identifier"))?;
            let mut identities = candidates.into_values();
            let Some(first) = identities.next() else {
                return Err(NewTypeGraphCodecError::UnresolvedIdentifier(source.clone()));
            };
            Err(NewTypeGraphCodecError::AmbiguousIdentifier(
                identifier,
                NonEmptyVec::new(first, identities.collect()),
            ))
        }
    }
}

fn matching_candidates(
    lookup: &str,
    catalogue_crate: &CrateName,
    rustdoc_paths: &HashMap<Id, ItemSummary>,
) -> BTreeMap<String, FullyQualifiedItemPath> {
    let summaries = rustdoc_paths
        .values()
        .filter_map(|summary| summary_identity(summary).map(|identity| (summary, identity)))
        .collect::<Vec<_>>();
    let local_prefix = format!("{}::", catalogue_crate.as_str());
    let normalized_crate_path = lookup
        .strip_prefix("crate::")
        .map(|rest| format!("{local_prefix}{rest}"))
        .unwrap_or_else(|| lookup.to_owned());
    let is_bare = !lookup.contains("::");

    let mut local_suffixes = BTreeMap::new();
    let mut exact = BTreeMap::new();
    let mut prelude_aliases = BTreeMap::new();
    let mut bare = BTreeMap::new();
    let prelude_name = STD_PRELUDE_TYPES
        .iter()
        .copied()
        .find(|name| std_canonical_path(name) == normalized_crate_path);
    let prelude_module = prelude_name
        .and_then(|name| std_canonical_path(name).split("::").nth(1).map(str::to_owned));
    for (_, identity) in summaries {
        let full = identity.to_string();
        let local = full.strip_prefix(&local_prefix);
        if normalized_crate_path == full {
            exact.insert(full.clone(), identity.clone());
        }
        if let (Some(prelude_name), Some(prelude_module)) =
            (prelude_name, prelude_module.as_deref())
            && identity.name().as_str() == prelude_name
            && identity.module_path().segments().first().map(|segment| segment.as_str())
                == Some(prelude_module)
            && matches!(identity.crate_name().as_str(), "alloc" | "core" | "std")
        {
            prelude_aliases.insert(full.clone(), identity.clone());
        }
        if let Some(local) = local
            && !is_bare
            && local == lookup
        {
            local_suffixes.insert(full.clone(), identity.clone());
        }
        if is_bare && identity.name().as_str() == lookup {
            bare.insert(full, identity);
        }
    }

    if !exact.is_empty() {
        return exact;
    }
    if !prelude_aliases.is_empty() {
        return prelude_aliases;
    }
    if !local_suffixes.is_empty() {
        return local_suffixes;
    }
    if is_bare {
        let local_bare = bare
            .iter()
            .filter(|(path, _)| path.starts_with(&local_prefix))
            .map(|(path, identity)| (path.clone(), identity.clone()))
            .collect::<BTreeMap<_, _>>();
        if !local_bare.is_empty() {
            return local_bare;
        }
        return bare;
    }
    BTreeMap::new()
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
