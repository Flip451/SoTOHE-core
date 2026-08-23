//! Shared resolution of one catalogue path against a caller-owned identity universe.
//!
//! The domain core deliberately receives an already extracted path.  Syntax parsing
//! and construction of a rustdoc-backed universe remain responsibilities of the
//! caller's adapter; this module only defines the path-resolution semantics shared by
//! catalogue linting and infrastructure codecs.

use std::collections::BTreeSet;

use crate::tddd::catalogue_v2::identifiers::{
    CrateName, FullyQualifiedItemPath, Identifier, TypeRef,
};
use crate::tddd::catalogue_v2::roles::NonEmptyVec;

/// Failure while resolving an already extracted catalogue path.
///
/// Syntax and `TypeRef` parsing failures belong to the adapter-owned codec
/// error. This error contains only failures reachable after path extraction:
/// an identity can be ambiguous or it can be absent from the supplied
/// universe.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CatalogueIdentityResolutionError {
    /// More than one fully qualified identity matches the reference.
    #[error(
        "ambiguous identifier `{identifier}`; candidates: {candidates:?}",
        identifier = .0.as_str(),
        candidates = .1.as_slice()
    )]
    AmbiguousIdentifier(Identifier, NonEmptyVec<FullyQualifiedItemPath>),

    /// No identity in the supplied universe matches the reference.
    #[error("unresolved identifier `{identifier}`", identifier = .0.as_str())]
    UnresolvedIdentifier(TypeRef),
}

/// Resolves one caller-extracted catalogue path against a supplied identity universe.
///
/// A crate-qualified path is matched exactly.  A path without a crate prefix may
/// use the catalogue crate's module path, while a bare name first searches that
/// crate and then the remaining universe (which supports prelude identities).
/// Ambiguous and missing references are rejected instead of falling back to a
/// short-name identity.
///
/// # Errors
///
/// Returns [`CatalogueIdentityResolutionError::AmbiguousIdentifier`] with every
/// matching fully qualified candidate when the reference has more than one
/// identity. Returns [`CatalogueIdentityResolutionError::UnresolvedIdentifier`]
/// when no identity matches the reference.
pub fn resolve_catalogue_identity(
    reference: &TypeRef,
    catalogue_crate: &CrateName,
    universe: &BTreeSet<FullyQualifiedItemPath>,
) -> Result<FullyQualifiedItemPath, CatalogueIdentityResolutionError> {
    let lookup = normalize_lookup(reference.as_str(), catalogue_crate);
    let candidates = matching_candidates(&lookup, catalogue_crate, universe);

    match candidates.as_slice() {
        [identity] => Ok(identity.clone()),
        [] => Err(CatalogueIdentityResolutionError::UnresolvedIdentifier(reference.clone())),
        [first, rest @ ..] => {
            let identifier_name = lookup.rsplit("::").next().unwrap_or(lookup.as_str());
            let identifier = Identifier::new(identifier_name.to_owned()).map_err(|_| {
                CatalogueIdentityResolutionError::UnresolvedIdentifier(reference.clone())
            })?;
            Err(CatalogueIdentityResolutionError::AmbiguousIdentifier(
                identifier,
                NonEmptyVec::new(first.clone(), rest.to_vec()),
            ))
        }
    }
}

fn normalize_lookup(reference: &str, catalogue_crate: &CrateName) -> String {
    let lookup = reference.strip_prefix("::").unwrap_or(reference);
    lookup
        .strip_prefix("crate::")
        .map(|rest| format!("{}::{rest}", catalogue_crate.as_str()))
        .unwrap_or_else(|| lookup.to_owned())
}

fn matching_candidates(
    lookup: &str,
    catalogue_crate: &CrateName,
    universe: &BTreeSet<FullyQualifiedItemPath>,
) -> Vec<FullyQualifiedItemPath> {
    if lookup.contains("::") {
        let exact = universe
            .iter()
            .filter(|identity| identity.to_string() == lookup)
            .cloned()
            .collect::<Vec<_>>();
        if !exact.is_empty() {
            return exact;
        }

        // The infrastructure parser renders prelude names through the `std`
        // spelling, while rustdoc may expose the defining item from `core` or
        // `alloc`. Treat those standard-library paths as aliases only after an
        // exact match has been ruled out.
        if let Some(std_suffix) = lookup.strip_prefix("std::") {
            let (suffix_module, suffix_name) = std_suffix
                .rsplit_once("::")
                .map_or((None, std_suffix), |(module, name)| (Some(module), name));
            let aliases = universe
                .iter()
                .filter(|identity| {
                    matches!(identity.crate_name().as_str(), "alloc" | "core" | "std")
                        && suffix_name == identity.name().as_str()
                        && suffix_module.map_or_else(
                            || identity.module_path().is_root(),
                            |module| standard_public_namespace_matches(module, identity),
                        )
                })
                .cloned()
                .collect::<Vec<_>>();
            if !aliases.is_empty() {
                return aliases;
            }
        }

        // A path beginning with a known crate name is explicitly crate-qualified.
        // Once its exact identity is absent, it must fail closed rather than being
        // reinterpreted as a local module path in the catalogue crate.
        let first_segment = lookup.split_once("::").map(|(first, _)| first);
        let is_known_crate = first_segment.is_some_and(|first| {
            first == catalogue_crate.as_str()
                || matches!(first, "alloc" | "core" | "std")
                || universe.iter().any(|identity| identity.crate_name().as_str() == first)
        });
        if is_known_crate {
            return Vec::new();
        }

        // A crate-less module path such as `alpha::Entity` is local notation.
        // Never use the terminal name alone here: `beta::Entity` must not resolve
        // to an unrelated `alpha::Entity`.
        return universe
            .iter()
            .filter(|identity| {
                identity.crate_name() == catalogue_crate && local_path(identity) == lookup
            })
            .cloned()
            .collect();
    }

    let local = universe
        .iter()
        .filter(|identity| {
            identity.crate_name() == catalogue_crate && identity.name().as_str() == lookup
        })
        .cloned()
        .collect::<Vec<_>>();
    if !local.is_empty() {
        return local;
    }

    universe.iter().filter(|identity| identity.name().as_str() == lookup).cloned().collect()
}

fn local_path(identity: &FullyQualifiedItemPath) -> String {
    if identity.module_path().is_root() {
        identity.name().to_string()
    } else {
        format!("{}::{}", identity.module_path(), identity.name())
    }
}

fn standard_public_namespace_matches(
    public_module: &str,
    identity: &FullyQualifiedItemPath,
) -> bool {
    let public_segments =
        public_module.split("::").filter(|segment| !segment.is_empty()).collect::<Vec<_>>();
    let candidate_segments = identity.module_path().segments();
    let mut candidate_index = 0;
    for public_segment in &public_segments {
        let Some(candidate) = candidate_segments.get(candidate_index) else {
            return false;
        };
        if candidate.as_str() == *public_segment {
            candidate_index += 1;
            continue;
        }

        let (definition_segment, collection_kind) = match *public_segment {
            "hash_map" => ("hash", "map"),
            "btree_map" => ("btree", "map"),
            "hash_set" => ("hash", "set"),
            "btree_set" => ("btree", "set"),
            _ => return false,
        };
        let Some(map_segment) = candidate_segments.get(candidate_index + 1) else {
            return false;
        };
        if candidate.as_str() != definition_segment || map_segment.as_str() != collection_kind {
            return false;
        }
        candidate_index += 2;
    }

    let Some(remaining) = candidate_segments.get(candidate_index..) else {
        return false;
    };
    remaining.is_empty() || standard_reexport_suffix_matches(public_module, identity, remaining)
}

fn standard_reexport_suffix_matches(
    public_module: &str,
    identity: &FullyQualifiedItemPath,
    remaining: &[Identifier],
) -> bool {
    match (public_module, identity.name().as_str()) {
        ("collections", "BTreeMap") => segments_match(remaining, &["btree", "map"]),
        ("collections", "BTreeSet") => segments_match(remaining, &["btree", "set"]),
        ("collections", "HashMap") => segments_match(remaining, &["hash", "map"]),
        ("collections", "HashSet") => segments_match(remaining, &["hash", "set"]),
        ("iter", "Iterator") => segments_match(remaining, &["traits", "iterator"]),
        ("iter", "IntoIterator") => segments_match(remaining, &["traits", "collect"]),
        ("iter", "DoubleEndedIterator") => segments_match(remaining, &["traits", "double_ended"]),
        ("iter", "ExactSizeIterator") => segments_match(remaining, &["traits", "exact_size"]),
        ("ops", "Deref") => segments_match(remaining, &["deref"]),
        ("ops", "FnOnce") => segments_match(remaining, &["function"]),
        ("str", "FromStr") => segments_match(remaining, &["traits"]),
        _ => false,
    }
}

fn segments_match(segments: &[Identifier], expected: &[&str]) -> bool {
    segments.len() == expected.len()
        && segments
            .iter()
            .zip(expected.iter())
            .all(|(segment, expected)| segment.as_str() == *expected)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::tddd::catalogue_v2::identifiers::{ModulePath, TypeRef};

    fn identity(crate_name: &str, module: &[&str], name: &str) -> FullyQualifiedItemPath {
        FullyQualifiedItemPath::new(
            CrateName::new(crate_name).expect("valid crate name"),
            ModulePath::from_segments(module.to_vec()).expect("valid module path"),
            Identifier::new(name).expect("valid item name"),
        )
    }

    fn reference(value: &str) -> TypeRef {
        TypeRef::new(value).expect("non-empty TypeRef")
    }

    #[test]
    fn test_resolve_catalogue_identity_unique_short_name_returns_fully_qualified_path() {
        let expected = identity("domain", &["alpha"], "Entity");
        let universe = BTreeSet::from([expected.clone()]);

        let resolved = resolve_catalogue_identity(
            &reference("Entity"),
            &CrateName::new("domain").expect("valid crate"),
            &universe,
        )
        .expect("unique short name resolves");

        assert_eq!(resolved, expected);
        assert_eq!(resolved.to_string(), "domain::alpha::Entity");
        assert_ne!(resolved.to_string(), "Entity");

        let ambiguous_universe =
            BTreeSet::from([expected.clone(), identity("domain", &["beta"], "Entity")]);
        let ambiguous = resolve_catalogue_identity(
            &reference("Entity"),
            &CrateName::new("domain").expect("valid crate"),
            &ambiguous_universe,
        )
        .expect_err("ambiguous short name must not fall back to a short identity");
        assert!(matches!(ambiguous, CatalogueIdentityResolutionError::AmbiguousIdentifier(_, _)));

        let unresolved = resolve_catalogue_identity(
            &reference("domain::missing::Entity"),
            &CrateName::new("domain").expect("valid crate"),
            &universe,
        )
        .expect_err("unresolved qualified path must not fall back to a short identity");
        assert!(matches!(unresolved, CatalogueIdentityResolutionError::UnresolvedIdentifier(_)));
    }

    #[test]
    fn test_resolve_catalogue_identity_crate_prefixed_path_returns_exact_identity() {
        let other = identity("domain", &["alpha"], "Entity");
        let expected = identity("domain", &["beta"], "Entity");
        let universe = BTreeSet::from([other.clone(), expected.clone()]);

        let resolved = resolve_catalogue_identity(
            &reference("domain::beta::Entity"),
            &CrateName::new("domain").expect("valid crate"),
            &universe,
        )
        .expect("crate-prefixed path resolves");

        assert_eq!(resolved, expected);
        assert_ne!(resolved, other);
    }

    #[test]
    fn test_resolve_catalogue_identity_crate_keyword_path_uses_catalogue_crate() {
        let expected = identity("domain", &["alpha"], "Entity");
        let universe = BTreeSet::from([expected.clone()]);

        let resolved = resolve_catalogue_identity(
            &reference("crate::alpha::Entity"),
            &CrateName::new("domain").expect("valid crate"),
            &universe,
        )
        .expect("crate keyword path resolves");

        assert_eq!(resolved, expected);
    }

    #[test]
    fn test_resolve_catalogue_identity_relative_prefixes_use_catalogue_crate() {
        let expected = identity("domain", &["alpha"], "Entity");
        let universe = BTreeSet::from([expected.clone()]);
        let catalogue_crate = CrateName::new("domain").expect("valid crate");

        for prefix in ["self::", "super::"] {
            let error = resolve_catalogue_identity(
                &reference(&format!("{prefix}alpha::Entity")),
                &catalogue_crate,
                &universe,
            )
            .expect_err("relative path needs referring-module context");

            assert!(matches!(
                error,
                CatalogueIdentityResolutionError::UnresolvedIdentifier(unresolved)
                    if unresolved.as_str() == format!("{prefix}alpha::Entity")
            ));
        }
    }

    #[test]
    fn test_resolve_catalogue_identity_ambiguous_short_name_reports_all_candidates() {
        let first = identity("domain", &["alpha"], "Entity");
        let second = identity("domain", &["beta"], "Entity");
        let universe = BTreeSet::from([first.clone(), second.clone()]);

        let error = resolve_catalogue_identity(
            &reference("Entity"),
            &CrateName::new("domain").expect("valid crate"),
            &universe,
        )
        .expect_err("ambiguous short name must fail");

        assert!(matches!(
            error,
            CatalogueIdentityResolutionError::AmbiguousIdentifier(identifier, candidates)
                if identifier.as_str() == "Entity"
                    && candidates.as_slice() == [first, second]
        ));
    }

    #[test]
    fn test_resolve_catalogue_identity_unresolved_path_fails_closed() {
        let universe = BTreeSet::from([identity("domain", &["alpha"], "Entity")]);

        let error = resolve_catalogue_identity(
            &reference("domain::beta::Entity"),
            &CrateName::new("domain").expect("valid crate"),
            &universe,
        )
        .expect_err("unresolved path must fail");

        assert!(matches!(
            error,
            CatalogueIdentityResolutionError::UnresolvedIdentifier(unresolved)
                if unresolved.as_str() == "domain::beta::Entity"
        ));
    }

    #[test]
    fn test_resolve_catalogue_identity_std_alias_rejects_different_intermediate_modules() {
        let universe =
            BTreeSet::from([identity("alloc", &["collections", "btree", "map"], "Entry")]);

        let error = resolve_catalogue_identity(
            &reference("std::collections::hash_map::Entry"),
            &CrateName::new("usecase").expect("valid crate"),
            &universe,
        )
        .expect_err("different intermediate modules must not alias");

        assert!(matches!(
            error,
            CatalogueIdentityResolutionError::UnresolvedIdentifier(unresolved)
                if unresolved.as_str() == "std::collections::hash_map::Entry"
        ));

        let truncated_namespace_error = resolve_catalogue_identity(
            &reference("std::collections::Entry"),
            &CrateName::new("usecase").expect("valid crate"),
            &universe,
        )
        .expect_err("a truncated standard namespace must not alias a definition module");

        assert!(matches!(
            truncated_namespace_error,
            CatalogueIdentityResolutionError::UnresolvedIdentifier(unresolved)
                if unresolved.as_str() == "std::collections::Entry"
        ));
    }

    #[test]
    fn test_resolve_catalogue_identity_std_collection_map_reexports_use_definition_modules() {
        let hash_entry = identity("alloc", &["collections", "hash", "map"], "Entry");
        let btree_entry = identity("alloc", &["collections", "btree", "map"], "Entry");
        let universe = BTreeSet::from([hash_entry.clone(), btree_entry.clone()]);
        let catalogue_crate = CrateName::new("usecase").expect("valid crate");

        let resolved_hash = resolve_catalogue_identity(
            &reference("std::collections::hash_map::Entry"),
            &catalogue_crate,
            &universe,
        )
        .expect("hash_map re-export resolves to its definition");
        let resolved_btree = resolve_catalogue_identity(
            &reference("std::collections::btree_map::Entry"),
            &catalogue_crate,
            &universe,
        )
        .expect("btree_map re-export resolves to its definition");

        assert_eq!(resolved_hash, hash_entry);
        assert_eq!(resolved_btree, btree_entry);
    }

    #[test]
    fn test_resolve_catalogue_identity_std_collection_set_reexports_use_definition_modules() {
        let hash_iter = identity("alloc", &["collections", "hash", "set"], "Iter");
        let btree_iter = identity("alloc", &["collections", "btree", "set"], "Iter");
        let universe = BTreeSet::from([hash_iter.clone(), btree_iter.clone()]);
        let catalogue_crate = CrateName::new("usecase").expect("valid crate");

        let resolved_hash = resolve_catalogue_identity(
            &reference("std::collections::hash_set::Iter"),
            &catalogue_crate,
            &universe,
        )
        .expect("hash_set re-export resolves to its definition");
        let resolved_btree = resolve_catalogue_identity(
            &reference("std::collections::btree_set::Iter"),
            &catalogue_crate,
            &universe,
        )
        .expect("btree_set re-export resolves to its definition");

        assert_eq!(resolved_hash, hash_iter);
        assert_eq!(resolved_btree, btree_iter);
    }

    #[test]
    fn test_resolve_catalogue_identity_std_collection_alias_uses_definition_module() {
        let expected = identity("alloc", &["collections", "btree", "map"], "BTreeMap");
        let universe = BTreeSet::from([expected.clone()]);

        let resolved = resolve_catalogue_identity(
            &reference("std::collections::BTreeMap"),
            &CrateName::new("usecase").expect("valid crate"),
            &universe,
        )
        .expect("standard collection alias resolves to its definition");

        assert_eq!(resolved, expected);
    }

    #[test]
    fn test_resolve_catalogue_identity_std_trait_alias_uses_definition_module() {
        let expected = identity("core", &["str", "traits"], "FromStr");
        let universe = BTreeSet::from([expected.clone()]);

        let resolved = resolve_catalogue_identity(
            &reference("std::str::FromStr"),
            &CrateName::new("usecase").expect("valid crate"),
            &universe,
        )
        .expect("standard trait alias resolves to its definition");

        assert_eq!(resolved, expected);
    }

    #[test]
    fn test_resolve_catalogue_identity_std_reexports_use_authoritative_definition_paths() {
        let iterator = identity("core", &["iter", "traits", "iterator"], "Iterator");
        let deref = identity("core", &["ops", "deref"], "Deref");
        let fn_once = identity("core", &["ops", "function"], "FnOnce");
        let universe = BTreeSet::from([iterator.clone(), deref.clone(), fn_once.clone()]);

        let resolved_iterator = resolve_catalogue_identity(
            &reference("std::iter::Iterator"),
            &CrateName::new("usecase").expect("valid crate"),
            &universe,
        )
        .expect("Iterator re-export resolves");
        let resolved_deref = resolve_catalogue_identity(
            &reference("std::ops::Deref"),
            &CrateName::new("usecase").expect("valid crate"),
            &universe,
        )
        .expect("Deref re-export resolves");
        let resolved_fn_once = resolve_catalogue_identity(
            &reference("std::ops::FnOnce"),
            &CrateName::new("usecase").expect("valid crate"),
            &universe,
        )
        .expect("FnOnce re-export resolves");

        assert_eq!(resolved_iterator, iterator);
        assert_eq!(resolved_deref, deref);
        assert_eq!(resolved_fn_once, fn_once);
    }

    #[test]
    fn test_resolve_catalogue_identity_std_iterator_reexports_cover_nested_definition_modules() {
        let expected = [
            ("IntoIterator", identity("core", &["iter", "traits", "collect"], "IntoIterator")),
            (
                "DoubleEndedIterator",
                identity("core", &["iter", "traits", "double_ended"], "DoubleEndedIterator"),
            ),
            (
                "ExactSizeIterator",
                identity("core", &["iter", "traits", "exact_size"], "ExactSizeIterator"),
            ),
        ];
        let universe = expected.iter().map(|(_, path)| path.clone()).collect::<BTreeSet<_>>();
        let catalogue_crate = CrateName::new("usecase").expect("valid crate");

        for (short_name, expected_identity) in expected {
            let bare =
                resolve_catalogue_identity(&reference(short_name), &catalogue_crate, &universe)
                    .expect("bare prelude iterator trait resolves");
            let explicit = resolve_catalogue_identity(
                &reference(&format!("std::iter::{short_name}")),
                &catalogue_crate,
                &universe,
            )
            .expect("explicit std iterator trait resolves");

            assert_eq!(bare, expected_identity);
            assert_eq!(explicit, expected_identity);
        }
    }

    #[test]
    fn test_resolve_catalogue_identity_qualified_crate_miss_does_not_fall_back_to_local_module() {
        let universe = BTreeSet::from([
            identity("domain", &["alpha"], "Known"),
            identity("usecase", &["domain", "beta"], "Event"),
        ]);

        let error = resolve_catalogue_identity(
            &reference("domain::beta::Event"),
            &CrateName::new("usecase").expect("valid crate"),
            &universe,
        )
        .expect_err("missing qualified identity must not use a local module collision");

        assert!(matches!(
            error,
            CatalogueIdentityResolutionError::UnresolvedIdentifier(unresolved)
                if unresolved.as_str() == "domain::beta::Event"
        ));
    }
}
