//! Shared resolution of one catalogue path against a caller-owned identity universe.
//!
//! The domain core deliberately receives an already extracted path.  Syntax parsing
//! and construction of a rustdoc-backed universe remain responsibilities of the
//! caller's adapter; this module only defines the path-resolution semantics shared by
//! catalogue linting and infrastructure codecs.

use std::collections::BTreeSet;

use crate::tddd::catalogue_v2::identifiers::{
    CatalogueItemNamespace, CrateName, FullyQualifiedItemPath, Identifier, ModulePath, TypeRef,
};
use crate::tddd::catalogue_v2::roles::NonEmptyVec;
use crate::tddd::catalogue_v2::{CatalogueDocument, ItemAction};
use crate::tddd::semantic_verify::CatalogueEntryKey;

/// Canonical external trait identities used as the linter's standard-library
/// seed universe. Keeping these paths beside the shared resolver ensures that
/// aliases such as `std::cmp::PartialEq` are resolved against one canonical set.
pub(crate) const STANDARD_EXTERNAL_TRAIT_PATHS: &str = concat!(
    "core::convert::From core::convert::Into core::convert::TryFrom ",
    "core::convert::TryInto core::convert::AsRef core::convert::AsMut ",
    "core::clone::Clone core::marker::Copy core::marker::Send core::marker::Sync ",
    "core::marker::Sized core::marker::Unpin core::fmt::Debug core::fmt::Display ",
    "core::cmp::PartialEq core::cmp::Eq core::cmp::PartialOrd core::cmp::Ord ",
    "core::hash::Hash core::hash::Hasher core::hash::BuildHasher ",
    "core::default::Default core::iter::Iterator core::iter::IntoIterator ",
    "core::iter::DoubleEndedIterator core::iter::ExactSizeIterator ",
    "core::iter::FromIterator core::iter::Extend core::iter::Sum core::iter::Product ",
    "core::ops::Drop core::ops::Deref core::ops::DerefMut core::ops::FnOnce ",
    "core::ops::FnMut core::ops::Fn core::ops::Add core::ops::Sub core::ops::Mul ",
    "core::ops::Div core::ops::Rem core::ops::Neg core::ops::Not core::ops::BitAnd ",
    "core::ops::BitOr core::ops::BitXor core::ops::Shl core::ops::Shr ",
    "core::ops::Index core::ops::IndexMut core::ops::RangeBounds ",
    "core::ops::AddAssign core::ops::SubAssign ",
    "core::ops::MulAssign core::ops::DivAssign core::ops::RemAssign ",
    "core::ops::BitAndAssign core::ops::BitOrAssign core::ops::BitXorAssign ",
    "core::ops::ShlAssign core::ops::ShrAssign core::str::traits::FromStr ",
    "core::borrow::Borrow core::borrow::BorrowMut core::error::Error ",
    "std::io::Read std::io::Write std::io::Seek std::io::BufRead",
);

/// Failure while resolving an already extracted catalogue path.
///
/// Syntax and `TypeRef` parsing failures belong to the adapter-owned extractor
/// error. This error contains failures reachable after path extraction: an
/// identity can be ambiguous, absent from the supplied universe, or impossible
/// to classify as either an in-catalogue path or an explicitly external path.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CatalogueIdentityResolutionError {
    /// More than one fully qualified identity matches the reference.
    #[error(
        "ambiguous identifier `{identifier}`; candidates: FullyQualifiedItemPath {candidates:?}",
        identifier = .0.as_str(),
        candidates = .1.as_slice()
    )]
    AmbiguousIdentifier(Identifier, NonEmptyVec<FullyQualifiedItemPath>),

    /// No identity in the supplied universe matches the reference.
    #[error("unresolved identifier `{identifier}`", identifier = .0.as_str())]
    UnresolvedIdentifier(TypeRef),

    /// The extracted path does not carry enough crate/context information to
    /// classify it as a catalogue reference or an explicitly external path.
    #[error("could not classify TypeRef path at `{location}`")]
    ClassificationFailed {
        /// The extracted path whose ownership is unknown.
        location: TypeRef,
    },
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
    resolve_catalogue_identity_in_namespace(reference, catalogue_crate, universe, None)
}

/// Resolves the namespace of a task-contract entry from the catalogue section
/// that declares it.
///
/// Type and trait entries are distinct identities even when their keys are the
/// same. Function entries and deletion records for functions retain the
/// report-label identity and therefore return `None`. A key declared in more
/// than one live or deletion section, or in no section, is rejected instead of
/// being assigned a namespace by convention.
///
/// # Errors
///
/// Returns [`CatalogueIdentityResolutionError::ClassificationFailed`] when the
/// key has more than one matching live or deletion declaration. Returns
/// [`CatalogueIdentityResolutionError::UnresolvedIdentifier`] when it is not
/// declared in any catalogue section.
pub fn resolve_contract_entry_namespace(
    document: &CatalogueDocument,
    entry_key: &CatalogueEntryKey,
) -> Result<Option<CatalogueItemNamespace>, CatalogueIdentityResolutionError> {
    let reference = TypeRef::from_non_empty(entry_key.as_str().to_owned());
    let mut matches = Vec::new();
    if document.types().contains_key(entry_key) {
        matches.push(Some(CatalogueItemNamespace::Type));
    }
    if document.traits().contains_key(entry_key) {
        matches.push(Some(CatalogueItemNamespace::Trait));
    }
    if document
        .functions()
        .keys()
        .any(|function_path| function_path.to_string() == entry_key.as_str())
    {
        matches.push(None);
    }
    for deletion in document.deletions() {
        match deletion {
            crate::tddd::catalogue_v2::DeletionRecord::Type { name, .. } if name == entry_key => {
                matches.push(Some(CatalogueItemNamespace::Type))
            }
            crate::tddd::catalogue_v2::DeletionRecord::Trait { name, .. } if name == entry_key => {
                matches.push(Some(CatalogueItemNamespace::Trait))
            }
            crate::tddd::catalogue_v2::DeletionRecord::Function { path, .. }
                if path.to_string() == entry_key.as_str() =>
            {
                matches.push(None)
            }
            _ => {}
        }
    }

    match matches.as_slice() {
        [namespace] => Ok(*namespace),
        [] => Err(CatalogueIdentityResolutionError::UnresolvedIdentifier(reference)),
        [..] => Err(CatalogueIdentityResolutionError::ClassificationFailed { location: reference }),
    }
}

/// Resolves a catalogue identity within one namespace.
///
/// The caller supplies a type-only or trait-only universe when the spelling is
/// ambiguous across Rust namespaces. Keeping the namespace in the domain
/// identity makes same-named types and traits independently representable.
///
/// # Errors
///
/// Returns the same fail-closed errors as [`resolve_catalogue_identity`].
pub fn resolve_catalogue_identity_in_namespace(
    reference: &TypeRef,
    catalogue_crate: &CrateName,
    universe: &BTreeSet<FullyQualifiedItemPath>,
    namespace: Option<CatalogueItemNamespace>,
) -> Result<FullyQualifiedItemPath, CatalogueIdentityResolutionError> {
    let lookup = normalize_lookup(reference.as_str(), catalogue_crate);
    let candidates = matching_candidates(&lookup, catalogue_crate, universe, namespace);

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

/// Namespace-scoped form of [`resolve_catalogue_identity_for_action_in_namespace`].
///
/// # Errors
///
/// Returns an ambiguity or unresolved error when D3 cannot establish one
/// identity without guessing.
pub fn resolve_catalogue_identity_for_action_in_namespace(
    reference: &TypeRef,
    catalogue_crate: &CrateName,
    action: ItemAction,
    baseline: &BTreeSet<FullyQualifiedItemPath>,
    current: &BTreeSet<FullyQualifiedItemPath>,
    namespace: CatalogueItemNamespace,
) -> Result<FullyQualifiedItemPath, CatalogueIdentityResolutionError> {
    let lookup = normalize_lookup(reference.as_str(), catalogue_crate);
    let is_unplaced_reference = !lookup.contains("::");
    if action == ItemAction::Add {
        let baseline_matches = if is_unplaced_reference {
            matching_name_candidates(&lookup, catalogue_crate, baseline, Some(namespace))
        } else {
            matching_candidates(&lookup, catalogue_crate, baseline, Some(namespace))
        };
        if !baseline_matches.is_empty() {
            return ambiguous_or_unresolved(reference, lookup.as_str(), baseline_matches);
        }

        let current_matches = if is_unplaced_reference {
            matching_name_candidates(&lookup, catalogue_crate, current, Some(namespace))
        } else {
            matching_candidates(&lookup, catalogue_crate, current, Some(namespace))
        };
        return match current_matches.as_slice() {
            [identity] => Ok(identity.clone()),
            [] if is_unplaced_reference => {
                unplaced_identity(reference, &lookup, catalogue_crate, namespace)
            }
            [] => {
                let name = lookup.rsplit("::").next().unwrap_or(lookup.as_str());
                let name = Identifier::new(name.to_owned()).map_err(|_| {
                    CatalogueIdentityResolutionError::UnresolvedIdentifier(reference.clone())
                })?;
                let mut path_segments = lookup.split("::").collect::<Vec<_>>();
                let _ = path_segments.pop();
                if path_segments.first().copied() == Some(catalogue_crate.as_str()) {
                    let _ = path_segments.remove(0);
                }
                let module = path_segments.into_iter().map(str::to_owned).collect::<Vec<_>>();
                let module_path = ModulePath::from_segments(module).map_err(|_| {
                    CatalogueIdentityResolutionError::UnresolvedIdentifier(reference.clone())
                })?;
                Ok(match namespace {
                    CatalogueItemNamespace::Type => {
                        FullyQualifiedItemPath::new_type(catalogue_crate.clone(), module_path, name)
                    }
                    CatalogueItemNamespace::Trait => FullyQualifiedItemPath::new_trait(
                        catalogue_crate.clone(),
                        module_path,
                        name,
                    ),
                })
            }
            [_, ..] => ambiguous_or_unresolved(
                reference,
                lookup.rsplit("::").next().unwrap_or(lookup.as_str()),
                current_matches,
            ),
        };
    }

    let baseline_matches = matching_candidates(&lookup, catalogue_crate, baseline, Some(namespace));
    match baseline_matches.as_slice() {
        [identity] => Ok(identity.clone()),
        [] => Err(CatalogueIdentityResolutionError::UnresolvedIdentifier(reference.clone())),
        [_, ..] => ambiguous_or_unresolved(
            reference,
            lookup.rsplit("::").next().unwrap_or(lookup.as_str()),
            baseline_matches,
        ),
    }
}

pub(crate) fn normalize_lookup(reference: &str, catalogue_crate: &CrateName) -> String {
    let lookup = reference.strip_prefix("::").unwrap_or(reference);
    let lookup = lookup.split('<').next().unwrap_or(lookup).trim();
    lookup
        .strip_prefix("crate::")
        .map(|rest| format!("{}::{rest}", catalogue_crate.as_str()))
        .unwrap_or_else(|| lookup.to_owned())
}

/// Returns whether an unresolved qualified path is explicitly external to the
/// catalogue crate represented by `universe`.
///
/// Resolution must happen before this classification. A path rooted in the
/// catalogue crate, `crate`, `self`, `super`, or a declared local module is not
/// external merely because its terminal item is missing; callers must report
/// that unresolved identity instead of silently skipping it.
pub(crate) fn is_explicit_external_path(
    reference: &TypeRef,
    catalogue_crate: &CrateName,
    universe: &BTreeSet<FullyQualifiedItemPath>,
) -> bool {
    let normalized = normalize_lookup(reference.as_str(), catalogue_crate);
    let Some((root, _)) = normalized.split_once("::") else {
        return false;
    };

    if matches!(root, "std" | "core" | "alloc") {
        return true;
    }
    if root == catalogue_crate.as_str() || matches!(root, "crate" | "self" | "super") {
        return false;
    }
    if universe.iter().any(|identity| identity.crate_name().as_str() == root) {
        return true;
    }

    !universe.iter().any(|identity| {
        identity.crate_name() == catalogue_crate
            && identity.module_path().is_some_and(|module_path| {
                module_path.segments().first().is_some_and(|segment| segment.as_str() == root)
            })
    })
}

fn matching_candidates(
    lookup: &str,
    catalogue_crate: &CrateName,
    universe: &BTreeSet<FullyQualifiedItemPath>,
    namespace: Option<CatalogueItemNamespace>,
) -> Vec<FullyQualifiedItemPath> {
    if lookup.contains("::") {
        let exact = universe
            .iter()
            .filter(|identity| {
                // An unplaced identity from another crate still has an exact
                // crate-qualified spelling (`crate::Name`); only its module
                // placement is unknown. A local unplaced identity must not
                // match after `crate::` normalization, because that would
                // treat omitted placement as an implicit crate-root path.
                (identity.is_placed() || identity.crate_name() != catalogue_crate)
                    && identity.to_string() == lookup
                    && namespace.is_none_or(|expected| identity.namespace() == expected)
            })
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
                    if namespace.is_some_and(|expected| identity.namespace() != expected) {
                        return false;
                    }
                    matches!(identity.crate_name().as_str(), "alloc" | "core" | "std")
                        && suffix_name == identity.name().as_str()
                        && suffix_module.map_or_else(
                            || {
                                identity
                                    .module_path()
                                    .is_some_and(|module_path| module_path.is_root())
                            },
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
                identity.crate_name() == catalogue_crate
                    && local_path(identity).as_deref() == Some(lookup)
                    && namespace.is_none_or(|expected| identity.namespace() == expected)
            })
            .cloned()
            .collect();
    }

    let local = universe
        .iter()
        .filter(|identity| {
            identity.crate_name() == catalogue_crate
                && identity.name().as_str() == lookup
                && namespace.is_none_or(|expected| identity.namespace() == expected)
        })
        .cloned()
        .collect::<Vec<_>>();
    if !local.is_empty() {
        return local;
    }

    universe
        .iter()
        .filter(|identity| {
            identity.name().as_str() == lookup
                && namespace.is_none_or(|expected| identity.namespace() == expected)
        })
        .cloned()
        .collect()
}

fn matching_name_candidates(
    lookup: &str,
    catalogue_crate: &CrateName,
    universe: &BTreeSet<FullyQualifiedItemPath>,
    namespace: Option<CatalogueItemNamespace>,
) -> Vec<FullyQualifiedItemPath> {
    universe
        .iter()
        .filter(|identity| {
            identity.crate_name() == catalogue_crate
                && identity.name().as_str() == lookup
                && namespace.is_none_or(|expected| identity.namespace() == expected)
        })
        .cloned()
        .collect()
}

fn local_path(identity: &FullyQualifiedItemPath) -> Option<String> {
    identity.module_path().map(|module_path| {
        if module_path.is_root() {
            identity.name().to_string()
        } else {
            format!("{module_path}::{}", identity.name())
        }
    })
}

fn unplaced_identity(
    reference: &TypeRef,
    lookup: &str,
    catalogue_crate: &CrateName,
    namespace: CatalogueItemNamespace,
) -> Result<FullyQualifiedItemPath, CatalogueIdentityResolutionError> {
    let name = Identifier::new(lookup.to_owned())
        .map_err(|_| CatalogueIdentityResolutionError::UnresolvedIdentifier(reference.clone()))?;
    Ok(match namespace {
        CatalogueItemNamespace::Type => {
            FullyQualifiedItemPath::new_unplaced_type(catalogue_crate.clone(), name)
        }
        CatalogueItemNamespace::Trait => {
            FullyQualifiedItemPath::new_unplaced_trait(catalogue_crate.clone(), name)
        }
    })
}

fn ambiguous_or_unresolved(
    reference: &TypeRef,
    identifier_name: &str,
    candidates: Vec<FullyQualifiedItemPath>,
) -> Result<FullyQualifiedItemPath, CatalogueIdentityResolutionError> {
    let identifier = Identifier::new(identifier_name.to_owned())
        .map_err(|_| CatalogueIdentityResolutionError::UnresolvedIdentifier(reference.clone()))?;
    let Some((first, rest)) = candidates.split_first() else {
        return Err(CatalogueIdentityResolutionError::UnresolvedIdentifier(reference.clone()));
    };
    Err(CatalogueIdentityResolutionError::AmbiguousIdentifier(
        identifier,
        NonEmptyVec::new(first.clone(), rest.to_vec()),
    ))
}

fn standard_public_namespace_matches(
    public_module: &str,
    identity: &FullyQualifiedItemPath,
) -> bool {
    let public_segments =
        public_module.split("::").filter(|segment| !segment.is_empty()).collect::<Vec<_>>();
    let Some(candidate_module_path) = identity.module_path() else {
        return false;
    };
    let candidate_segments = candidate_module_path.segments();
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
        ("collections", "BTreeMap") => {
            segments_match(remaining, &["btree", "map"])
                || segments_match(remaining, &["btree_map"])
        }
        ("collections", "BTreeSet") => {
            segments_match(remaining, &["btree", "set"])
                || segments_match(remaining, &["btree_set"])
        }
        ("collections", "HashMap") => {
            segments_match(remaining, &["hash", "map"]) || segments_match(remaining, &["hash_map"])
        }
        ("collections", "HashSet") => {
            segments_match(remaining, &["hash", "set"]) || segments_match(remaining, &["hash_set"])
        }
        ("collections", "BinaryHeap") => segments_match(remaining, &["binary_heap"]),
        ("collections", "LinkedList") => segments_match(remaining, &["linked_list"]),
        ("collections", "VecDeque") => segments_match(remaining, &["vec_deque"]),
        ("sync", "Mutex") => segments_match(remaining, &["poison", "mutex"]),
        ("sync", "RwLock") => segments_match(remaining, &["poison", "rwlock"]),
        ("iter", "Iterator") => segments_match(remaining, &["traits", "iterator"]),
        ("iter", "IntoIterator") => segments_match(remaining, &["traits", "collect"]),
        ("iter", "DoubleEndedIterator") => segments_match(remaining, &["traits", "double_ended"]),
        ("iter", "ExactSizeIterator") => segments_match(remaining, &["traits", "exact_size"]),
        ("iter", "FromIterator" | "Extend") => segments_match(remaining, &["traits", "collect"]),
        ("iter", "Sum" | "Product") => segments_match(remaining, &["traits", "accum"]),
        ("ops", "Deref") | ("ops", "DerefMut") => segments_match(remaining, &["deref"]),
        ("ops", "Fn") | ("ops", "FnMut") | ("ops", "FnOnce") => {
            segments_match(remaining, &["function"])
        }
        ("ops", "Drop") => segments_match(remaining, &["drop"]),
        (
            "ops",
            "Add" | "AddAssign" | "Div" | "DivAssign" | "Mul" | "MulAssign" | "Neg" | "Rem"
            | "RemAssign" | "Sub" | "SubAssign",
        ) => segments_match(remaining, &["arith"]),
        (
            "ops",
            "BitAnd" | "BitAndAssign" | "BitOr" | "BitOrAssign" | "BitXor" | "BitXorAssign" | "Not"
            | "Shl" | "ShlAssign" | "Shr" | "ShrAssign",
        ) => segments_match(remaining, &["bit"]),
        ("ops", "Index" | "IndexMut") => segments_match(remaining, &["index"]),
        (
            "ops",
            "Bound" | "IntoBounds" | "OneSidedRange" | "OneSidedRangeBound" | "Range"
            | "RangeBounds" | "RangeFrom" | "RangeFull" | "RangeInclusive" | "RangeTo"
            | "RangeToInclusive",
        ) => segments_match(remaining, &["range"]),
        ("ops", "ControlFlow") => segments_match(remaining, &["control_flow"]),
        ("ops", "Coroutine" | "CoroutineState") => segments_match(remaining, &["coroutine"]),
        ("ops", "AsyncFn" | "AsyncFnMut" | "AsyncFnOnce") => {
            segments_match(remaining, &["async_function"])
        }
        ("ops", "FromResidual" | "Residual" | "Try" | "Yeet") => {
            segments_match(remaining, &["try_trait"])
        }
        ("ops", "CoerceShared" | "Reborrow") => segments_match(remaining, &["reborrow"]),
        ("ops", "CoerceUnsized" | "DispatchFromDyn") => segments_match(remaining, &["unsize"]),
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
    use crate::tddd::catalogue_v2::DeletionRecord;
    use crate::tddd::catalogue_v2::composite::{StructKind, StructShape, TypeKindV2};
    use crate::tddd::catalogue_v2::entries::{FunctionEntry, TraitEntry, TypeEntry};
    use crate::tddd::catalogue_v2::identifiers::{FunctionName, FunctionPath, ModulePath, TypeRef};
    use crate::tddd::catalogue_v2::roles::{ContractRole, DataRole, FunctionRole};
    use crate::tddd::layer_id::LayerId;

    fn identity(crate_name: &str, module: &[&str], name: &str) -> FullyQualifiedItemPath {
        FullyQualifiedItemPath::new(
            CrateName::new(crate_name).expect("valid crate name"),
            ModulePath::from_segments(module.to_vec()).expect("valid module path"),
            Identifier::new(name).expect("valid item name"),
        )
    }

    fn trait_identity(crate_name: &str, module: &[&str], name: &str) -> FullyQualifiedItemPath {
        FullyQualifiedItemPath::new_trait(
            CrateName::new(crate_name).expect("valid crate name"),
            ModulePath::from_segments(module.to_vec()).expect("valid module path"),
            Identifier::new(name).expect("valid item name"),
        )
    }

    fn reference(value: &str) -> TypeRef {
        TypeRef::new(value).expect("non-empty TypeRef")
    }

    fn empty_catalogue() -> CatalogueDocument {
        CatalogueDocument::new(
            5,
            CrateName::new("domain").expect("valid crate name"),
            LayerId::try_new("domain").expect("valid layer id"),
        )
    }

    fn simple_type_entry() -> TypeEntry {
        TypeEntry::new(
            ItemAction::Add,
            DataRole::value_object(),
            TypeKindV2::Struct(StructKind::new(
                StructShape::Plain { fields: vec![], has_stripped_fields: false },
                None,
            )),
            vec![],
            vec![],
            vec![],
            Some(ModulePath::root()),
            None,
            vec![],
            vec![],
        )
    }

    fn simple_trait_entry() -> TraitEntry {
        TraitEntry::new(
            ItemAction::Add,
            ContractRole::SecondaryPort,
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
            Some(ModulePath::root()),
            None,
            vec![],
            vec![],
        )
    }

    fn simple_function_entry() -> FunctionEntry {
        FunctionEntry::new(
            ItemAction::Add,
            FunctionRole::FreeFunction,
            vec![],
            TypeRef::new("()").expect("valid return type"),
            false,
            vec![],
            vec![],
            None,
            vec![],
            vec![],
        )
    }

    fn assert_contract_entry_classification_failed(
        document: &CatalogueDocument,
        entry_key: &CatalogueEntryKey,
    ) {
        assert!(matches!(
            resolve_contract_entry_namespace(document, entry_key),
            Err(CatalogueIdentityResolutionError::ClassificationFailed { location })
                if location.as_str() == entry_key.as_str()
        ));
    }

    #[test]
    fn test_resolve_contract_entry_namespace_uses_sections_and_fails_closed() {
        let type_key = CatalogueEntryKey::try_new("SharedType".to_owned()).unwrap();
        let mut type_document = empty_catalogue();
        type_document.insert_type(type_key.clone(), simple_type_entry());
        assert_eq!(
            resolve_contract_entry_namespace(&type_document, &type_key),
            Ok(Some(CatalogueItemNamespace::Type))
        );

        let trait_key = CatalogueEntryKey::try_new("SharedTrait".to_owned()).unwrap();
        let mut trait_document = empty_catalogue();
        trait_document.insert_trait(trait_key.clone(), simple_trait_entry());
        assert_eq!(
            resolve_contract_entry_namespace(&trait_document, &trait_key),
            Ok(Some(CatalogueItemNamespace::Trait))
        );

        let function_key = CatalogueEntryKey::try_new("domain::compute".to_owned()).unwrap();
        let function_path = FunctionPath::at_root(
            CrateName::new("domain").expect("valid crate name"),
            FunctionName::new("compute").expect("valid function name"),
        );
        let mut function_document = empty_catalogue();
        function_document.insert_function(function_path, simple_function_entry());
        assert_eq!(resolve_contract_entry_namespace(&function_document, &function_key), Ok(None));

        let duplicate_key = CatalogueEntryKey::try_new("Shared".to_owned()).unwrap();
        let mut duplicate_document = empty_catalogue();
        duplicate_document.insert_type(duplicate_key.clone(), simple_type_entry());
        duplicate_document.insert_trait(duplicate_key.clone(), simple_trait_entry());
        assert!(matches!(
            resolve_contract_entry_namespace(&duplicate_document, &duplicate_key),
            Err(CatalogueIdentityResolutionError::ClassificationFailed { location })
                if location.as_str() == "Shared"
        ));

        let missing_key = CatalogueEntryKey::try_new("Missing".to_owned()).unwrap();
        let missing_document = empty_catalogue();
        assert!(matches!(
            resolve_contract_entry_namespace(&missing_document, &missing_key),
            Err(CatalogueIdentityResolutionError::UnresolvedIdentifier(reference))
                if reference.as_str() == "Missing"
        ));
    }

    #[test]
    fn test_resolve_contract_entry_namespace_includes_deletions_and_rejects_all_ambiguity() {
        let deleted_type_key = CatalogueEntryKey::try_new("DeletedType".to_owned()).unwrap();
        let mut deleted_type_document = empty_catalogue();
        deleted_type_document.push_deletion(DeletionRecord::Type {
            name: deleted_type_key.clone(),
            spec_refs: vec![],
            informal_grounds: vec![],
        });
        assert_eq!(
            resolve_contract_entry_namespace(&deleted_type_document, &deleted_type_key),
            Ok(Some(CatalogueItemNamespace::Type))
        );

        let deleted_trait_key = CatalogueEntryKey::try_new("DeletedTrait".to_owned()).unwrap();
        let mut deleted_trait_document = empty_catalogue();
        deleted_trait_document.push_deletion(DeletionRecord::Trait {
            name: deleted_trait_key.clone(),
            spec_refs: vec![],
            informal_grounds: vec![],
        });
        assert_eq!(
            resolve_contract_entry_namespace(&deleted_trait_document, &deleted_trait_key),
            Ok(Some(CatalogueItemNamespace::Trait))
        );

        let deleted_function_path = FunctionPath::at_root(
            CrateName::new("domain").expect("valid crate name"),
            FunctionName::new("deleted_function").expect("valid function name"),
        );
        let deleted_function_key =
            CatalogueEntryKey::try_new(deleted_function_path.to_string()).unwrap();
        let mut deleted_function_document = empty_catalogue();
        deleted_function_document.push_deletion(DeletionRecord::Function {
            path: deleted_function_path,
            spec_refs: vec![],
            informal_grounds: vec![],
        });
        assert_eq!(
            resolve_contract_entry_namespace(&deleted_function_document, &deleted_function_key),
            Ok(None)
        );

        let shared_key = CatalogueEntryKey::try_new("domain::Shared".to_owned()).unwrap();
        let shared_function_path = FunctionPath::at_root(
            CrateName::new("domain").expect("valid crate name"),
            FunctionName::new("Shared").expect("valid function name"),
        );

        let mut live_type_and_function = empty_catalogue();
        live_type_and_function.insert_type(shared_key.clone(), simple_type_entry());
        live_type_and_function
            .insert_function(shared_function_path.clone(), simple_function_entry());
        assert_contract_entry_classification_failed(&live_type_and_function, &shared_key);

        let mut live_trait_and_function = empty_catalogue();
        live_trait_and_function.insert_trait(shared_key.clone(), simple_trait_entry());
        live_trait_and_function
            .insert_function(shared_function_path.clone(), simple_function_entry());
        assert_contract_entry_classification_failed(&live_trait_and_function, &shared_key);

        let mut live_type_and_trait = empty_catalogue();
        live_type_and_trait.insert_type(shared_key.clone(), simple_type_entry());
        live_type_and_trait.insert_trait(shared_key.clone(), simple_trait_entry());
        assert_contract_entry_classification_failed(&live_type_and_trait, &shared_key);

        let mut deleted_type_and_trait = empty_catalogue();
        deleted_type_and_trait.push_deletion(DeletionRecord::Type {
            name: shared_key.clone(),
            spec_refs: vec![],
            informal_grounds: vec![],
        });
        deleted_type_and_trait.push_deletion(DeletionRecord::Trait {
            name: shared_key.clone(),
            spec_refs: vec![],
            informal_grounds: vec![],
        });
        assert_contract_entry_classification_failed(&deleted_type_and_trait, &shared_key);

        let mut deleted_type_and_function = empty_catalogue();
        deleted_type_and_function.push_deletion(DeletionRecord::Type {
            name: shared_key.clone(),
            spec_refs: vec![],
            informal_grounds: vec![],
        });
        deleted_type_and_function.push_deletion(DeletionRecord::Function {
            path: shared_function_path.clone(),
            spec_refs: vec![],
            informal_grounds: vec![],
        });
        assert_contract_entry_classification_failed(&deleted_type_and_function, &shared_key);

        let mut deleted_trait_and_function = empty_catalogue();
        deleted_trait_and_function.push_deletion(DeletionRecord::Trait {
            name: shared_key.clone(),
            spec_refs: vec![],
            informal_grounds: vec![],
        });
        deleted_trait_and_function.push_deletion(DeletionRecord::Function {
            path: shared_function_path.clone(),
            spec_refs: vec![],
            informal_grounds: vec![],
        });
        assert_contract_entry_classification_failed(&deleted_trait_and_function, &shared_key);

        let mut duplicate_type_declarations = empty_catalogue();
        duplicate_type_declarations.insert_type(shared_key.clone(), simple_type_entry());
        duplicate_type_declarations.push_deletion(DeletionRecord::Type {
            name: shared_key.clone(),
            spec_refs: vec![],
            informal_grounds: vec![],
        });
        assert_contract_entry_classification_failed(&duplicate_type_declarations, &shared_key);

        let mut live_type_and_deleted_trait = empty_catalogue();
        live_type_and_deleted_trait.insert_type(shared_key.clone(), simple_type_entry());
        live_type_and_deleted_trait.push_deletion(DeletionRecord::Trait {
            name: shared_key.clone(),
            spec_refs: vec![],
            informal_grounds: vec![],
        });
        assert_contract_entry_classification_failed(&live_type_and_deleted_trait, &shared_key);

        let mut live_type_and_deleted_function = empty_catalogue();
        live_type_and_deleted_function.insert_type(shared_key.clone(), simple_type_entry());
        live_type_and_deleted_function.push_deletion(DeletionRecord::Function {
            path: shared_function_path.clone(),
            spec_refs: vec![],
            informal_grounds: vec![],
        });
        assert_contract_entry_classification_failed(&live_type_and_deleted_function, &shared_key);

        let mut live_trait_and_deleted_type = empty_catalogue();
        live_trait_and_deleted_type.insert_trait(shared_key.clone(), simple_trait_entry());
        live_trait_and_deleted_type.push_deletion(DeletionRecord::Type {
            name: shared_key.clone(),
            spec_refs: vec![],
            informal_grounds: vec![],
        });
        assert_contract_entry_classification_failed(&live_trait_and_deleted_type, &shared_key);

        let mut live_trait_and_deleted_trait = empty_catalogue();
        live_trait_and_deleted_trait.insert_trait(shared_key.clone(), simple_trait_entry());
        live_trait_and_deleted_trait.push_deletion(DeletionRecord::Trait {
            name: shared_key.clone(),
            spec_refs: vec![],
            informal_grounds: vec![],
        });
        assert_contract_entry_classification_failed(&live_trait_and_deleted_trait, &shared_key);

        let mut live_trait_and_deleted_function = empty_catalogue();
        live_trait_and_deleted_function.insert_trait(shared_key.clone(), simple_trait_entry());
        live_trait_and_deleted_function.push_deletion(DeletionRecord::Function {
            path: shared_function_path.clone(),
            spec_refs: vec![],
            informal_grounds: vec![],
        });
        assert_contract_entry_classification_failed(&live_trait_and_deleted_function, &shared_key);

        let mut live_function_and_deleted_type = empty_catalogue();
        live_function_and_deleted_type
            .insert_function(shared_function_path.clone(), simple_function_entry());
        live_function_and_deleted_type.push_deletion(DeletionRecord::Type {
            name: shared_key.clone(),
            spec_refs: vec![],
            informal_grounds: vec![],
        });
        assert_contract_entry_classification_failed(&live_function_and_deleted_type, &shared_key);

        let mut live_function_and_deleted_trait = empty_catalogue();
        live_function_and_deleted_trait
            .insert_function(shared_function_path.clone(), simple_function_entry());
        live_function_and_deleted_trait.push_deletion(DeletionRecord::Trait {
            name: shared_key.clone(),
            spec_refs: vec![],
            informal_grounds: vec![],
        });
        assert_contract_entry_classification_failed(&live_function_and_deleted_trait, &shared_key);

        let mut live_function_and_deleted_function = empty_catalogue();
        live_function_and_deleted_function
            .insert_function(shared_function_path.clone(), simple_function_entry());
        live_function_and_deleted_function.push_deletion(DeletionRecord::Function {
            path: shared_function_path.clone(),
            spec_refs: vec![],
            informal_grounds: vec![],
        });
        assert_contract_entry_classification_failed(
            &live_function_and_deleted_function,
            &shared_key,
        );

        let mut duplicate_deleted_types = empty_catalogue();
        for _ in 0..2 {
            duplicate_deleted_types.push_deletion(DeletionRecord::Type {
                name: shared_key.clone(),
                spec_refs: vec![],
                informal_grounds: vec![],
            });
        }
        assert_contract_entry_classification_failed(&duplicate_deleted_types, &shared_key);
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
        let expected = [
            ("BTreeMap", identity("alloc", &["collections", "btree", "map"], "BTreeMap")),
            ("BTreeSet", identity("alloc", &["collections", "btree", "set"], "BTreeSet")),
            ("HashMap", identity("std", &["collections", "hash", "map"], "HashMap")),
            ("HashSet", identity("std", &["collections", "hash", "set"], "HashSet")),
            ("LinkedList", identity("alloc", &["collections", "linked_list"], "LinkedList")),
            ("VecDeque", identity("alloc", &["collections", "vec_deque"], "VecDeque")),
        ];
        let universe = expected.iter().map(|(_, path)| path.clone()).collect::<BTreeSet<_>>();
        let catalogue_crate = CrateName::new("usecase").expect("valid crate");

        for (name, expected_identity) in expected {
            let resolved = resolve_catalogue_identity(
                &reference(&format!("std::collections::{name}")),
                &catalogue_crate,
                &universe,
            )
            .expect("standard collection alias resolves to its definition");

            assert_eq!(resolved, expected_identity);
        }
    }

    #[test]
    fn test_resolve_catalogue_identity_std_sync_aliases_use_poison_definition_modules() {
        let mutex = identity("std", &["sync", "poison", "mutex"], "Mutex");
        let rw_lock = identity("std", &["sync", "poison", "rwlock"], "RwLock");
        let universe = BTreeSet::from([mutex.clone(), rw_lock.clone()]);
        let catalogue_crate = CrateName::new("usecase").expect("valid crate");

        let resolved_mutex =
            resolve_catalogue_identity(&reference("std::sync::Mutex"), &catalogue_crate, &universe)
                .expect("Mutex re-export resolves to the poison definition");
        let resolved_rw_lock = resolve_catalogue_identity(
            &reference("std::sync::RwLock"),
            &catalogue_crate,
            &universe,
        )
        .expect("RwLock re-export resolves to the poison definition");

        assert_eq!(resolved_mutex, mutex);
        assert_eq!(resolved_rw_lock, rw_lock);
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
        let deref_mut = identity("core", &["ops", "deref"], "DerefMut");
        let fn_trait = identity("core", &["ops", "function"], "Fn");
        let fn_mut = identity("core", &["ops", "function"], "FnMut");
        let fn_once = identity("core", &["ops", "function"], "FnOnce");
        let add = identity("core", &["ops", "arith"], "Add");
        let universe = BTreeSet::from([
            iterator.clone(),
            deref.clone(),
            deref_mut.clone(),
            fn_trait.clone(),
            fn_mut.clone(),
            fn_once.clone(),
            add.clone(),
        ]);

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
        let resolved_deref_mut = resolve_catalogue_identity(
            &reference("std::ops::DerefMut"),
            &CrateName::new("usecase").expect("valid crate"),
            &universe,
        )
        .expect("DerefMut re-export resolves");
        let resolved_fn = resolve_catalogue_identity(
            &reference("std::ops::Fn"),
            &CrateName::new("usecase").expect("valid crate"),
            &universe,
        )
        .expect("Fn re-export resolves");
        let resolved_fn_mut = resolve_catalogue_identity(
            &reference("std::ops::FnMut"),
            &CrateName::new("usecase").expect("valid crate"),
            &universe,
        )
        .expect("FnMut re-export resolves");
        let resolved_fn_once = resolve_catalogue_identity(
            &reference("std::ops::FnOnce"),
            &CrateName::new("usecase").expect("valid crate"),
            &universe,
        )
        .expect("FnOnce re-export resolves");
        let resolved_add = resolve_catalogue_identity(
            &reference("std::ops::Add"),
            &CrateName::new("usecase").expect("valid crate"),
            &universe,
        )
        .expect("Add re-export resolves");

        assert_eq!(resolved_iterator, iterator);
        assert_eq!(resolved_deref, deref);
        assert_eq!(resolved_deref_mut, deref_mut);
        assert_eq!(resolved_fn, fn_trait);
        assert_eq!(resolved_fn_mut, fn_mut);
        assert_eq!(resolved_fn_once, fn_once);
        assert_eq!(resolved_add, add);
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

    #[test]
    fn test_fully_qualified_item_path_preserves_omitted_placement_and_namespace() {
        let crate_name = CrateName::new("domain").expect("valid crate");
        let key = crate::tddd::semantic_verify::CatalogueEntryKey::try_new("Shared".to_owned())
            .expect("valid key");
        let unplaced =
            FullyQualifiedItemPath::from_type_catalogue_entry_key(&crate_name, &key, None)
                .expect("bare key is valid");
        assert!(matches!(unplaced, FullyQualifiedItemPath::UnplacedType { .. }));
        assert_eq!(unplaced.module_path(), None);

        let root = ModulePath::root();
        let placed =
            FullyQualifiedItemPath::from_type_catalogue_entry_key(&crate_name, &key, Some(&root))
                .expect("explicit root placement is valid");
        assert!(matches!(placed, FullyQualifiedItemPath::PlacedType { .. }));
        assert_eq!(placed.module_path(), Some(&root));

        let trait_identity =
            FullyQualifiedItemPath::from_trait_catalogue_entry_key(&crate_name, &key, None)
                .expect("bare trait key is valid");
        assert!(matches!(trait_identity, FullyQualifiedItemPath::UnplacedTrait { .. }));
        assert_ne!(unplaced, trait_identity);
    }

    #[test]
    fn test_resolve_catalogue_identity_for_action_applies_d3_placement_rules() {
        let crate_name = CrateName::new("domain").expect("valid crate");
        let thing_reference = reference("Thing");
        let baseline = BTreeSet::new();
        let current = BTreeSet::from([identity("domain", &["generated"], "Thing")]);

        let resolved = resolve_catalogue_identity_for_action_in_namespace(
            &thing_reference,
            &crate_name,
            ItemAction::Add,
            &baseline,
            &current,
            CatalogueItemNamespace::Type,
        )
        .expect("one current candidate resolves omitted placement");
        assert_eq!(resolved, identity("domain", &["generated"], "Thing"));

        let unimplemented = resolve_catalogue_identity_for_action_in_namespace(
            &reference("Future"),
            &crate_name,
            ItemAction::Add,
            &baseline,
            &BTreeSet::new(),
            CatalogueItemNamespace::Type,
        )
        .expect("an absent add remains an unplaced identity");
        assert!(matches!(unimplemented, FullyQualifiedItemPath::UnplacedType { .. }));

        let baseline_collision = BTreeSet::from([identity("domain", &["old"], "Thing")]);
        assert!(
            resolve_catalogue_identity_for_action_in_namespace(
                &thing_reference,
                &crate_name,
                ItemAction::Add,
                &baseline_collision,
                &current,
                CatalogueItemNamespace::Type,
            )
            .is_err()
        );

        let ambiguous_current = BTreeSet::from([
            identity("domain", &["alpha"], "Thing"),
            identity("domain", &["beta"], "Thing"),
        ]);
        assert!(
            resolve_catalogue_identity_for_action_in_namespace(
                &thing_reference,
                &crate_name,
                ItemAction::Add,
                &baseline,
                &ambiguous_current,
                CatalogueItemNamespace::Type,
            )
            .is_err()
        );

        let modified = resolve_catalogue_identity_for_action_in_namespace(
            &thing_reference,
            &crate_name,
            ItemAction::Modify,
            &current,
            &BTreeSet::new(),
            CatalogueItemNamespace::Type,
        )
        .expect("existing-item actions resolve against one baseline identity");
        assert_eq!(modified, identity("domain", &["generated"], "Thing"));
    }

    #[test]
    fn test_resolve_catalogue_identity_for_action_omitted_delete_and_reference_resolve_unique_baseline()
     {
        let crate_name = CrateName::new("domain").expect("valid crate");
        let expected = identity("domain", &["existing"], "Thing");
        let baseline = BTreeSet::from([expected.clone()]);

        for action in [ItemAction::Delete, ItemAction::Reference] {
            let resolved = resolve_catalogue_identity_for_action_in_namespace(
                &reference("Thing"),
                &crate_name,
                action,
                &baseline,
                &BTreeSet::new(),
                CatalogueItemNamespace::Type,
            )
            .expect("an omitted delete/reference resolves its unique baseline identity");
            assert_eq!(resolved, expected);
        }
    }

    #[test]
    fn test_resolve_catalogue_identity_for_action_omitted_delete_requires_baseline_candidate() {
        let crate_name = CrateName::new("domain").expect("valid crate");
        let current = BTreeSet::from([identity("domain", &["current"], "Thing")]);

        let error = resolve_catalogue_identity_for_action_in_namespace(
            &reference("Thing"),
            &crate_name,
            ItemAction::Delete,
            &BTreeSet::new(),
            &current,
            CatalogueItemNamespace::Type,
        )
        .expect_err("delete must fail closed without a baseline candidate");

        assert!(matches!(
            error,
            CatalogueIdentityResolutionError::UnresolvedIdentifier(unresolved)
                if unresolved.as_str() == "Thing"
        ));
    }

    #[test]
    fn test_resolve_catalogue_identity_for_action_omitted_reference_requires_baseline_candidate() {
        let crate_name = CrateName::new("domain").expect("valid crate");
        let current = BTreeSet::from([identity("domain", &["current"], "Thing")]);

        let error = resolve_catalogue_identity_for_action_in_namespace(
            &reference("Thing"),
            &crate_name,
            ItemAction::Reference,
            &BTreeSet::new(),
            &current,
            CatalogueItemNamespace::Type,
        )
        .expect_err("reference must fail closed without a baseline candidate");

        assert!(matches!(
            error,
            CatalogueIdentityResolutionError::UnresolvedIdentifier(unresolved)
                if unresolved.as_str() == "Thing"
        ));
    }

    #[test]
    fn test_resolve_catalogue_identity_for_action_qualified_near_match_fails_closed() {
        let crate_name = CrateName::new("domain").expect("valid crate");
        let baseline = BTreeSet::from([identity("domain", &["alpha"], "Thing")]);

        let error = resolve_catalogue_identity_for_action_in_namespace(
            &reference("domain::beta::Thing"),
            &crate_name,
            ItemAction::Reference,
            &baseline,
            &BTreeSet::new(),
            CatalogueItemNamespace::Type,
        )
        .expect_err("a qualified near-match must not fall back to a suffix match");

        assert!(matches!(
            error,
            CatalogueIdentityResolutionError::UnresolvedIdentifier(unresolved)
                if unresolved.as_str() == "domain::beta::Thing"
        ));
    }

    #[test]
    fn test_resolve_catalogue_identity_qualified_path_matches_unplaced_identity() {
        let catalogue_crate = CrateName::new("usecase").expect("valid crate");
        let unplaced = FullyQualifiedItemPath::new_unplaced_type(
            CrateName::new("domain").expect("valid crate"),
            Identifier::new("UserId").expect("valid item name"),
        );
        let universe = BTreeSet::from([unplaced.clone()]);

        let resolved =
            resolve_catalogue_identity(&reference("domain::UserId"), &catalogue_crate, &universe)
                .expect("a crate-qualified reference identifies an unplaced declaration");
        assert_eq!(resolved, unplaced);

        let error = resolve_catalogue_identity(
            &reference("domain::model::UserId"),
            &catalogue_crate,
            &universe,
        )
        .expect_err("an unmatched module path remains fail-closed");
        assert!(matches!(
            error,
            CatalogueIdentityResolutionError::UnresolvedIdentifier(unresolved)
                if unresolved.as_str() == "domain::model::UserId"
        ));
    }

    #[test]
    fn test_resolve_catalogue_identity_same_crate_qualified_unplaced_identity_remains_unresolved() {
        let catalogue_crate = CrateName::new("domain").expect("valid crate");
        let unplaced = FullyQualifiedItemPath::new_unplaced_type(
            catalogue_crate.clone(),
            Identifier::new("UserId").expect("valid item name"),
        );
        let universe = BTreeSet::from([unplaced.clone()]);

        for spelling in ["crate::UserId", "domain::UserId"] {
            let error =
                resolve_catalogue_identity(&reference(spelling), &catalogue_crate, &universe)
                    .expect_err("a same-crate qualified path must not place an unplaced identity");
            assert!(matches!(
                error,
                CatalogueIdentityResolutionError::UnresolvedIdentifier(unresolved)
                    if unresolved.as_str() == spelling
            ));
        }
        assert!(!unplaced.is_placed());
        assert_eq!(unplaced.module_path(), None);
    }

    #[test]
    fn test_resolve_catalogue_identity_namespace_keeps_same_named_type_and_trait_separate() {
        let type_path = identity("domain", &["types"], "Shared");
        let trait_path = trait_identity("domain", &["traits"], "Shared");
        let universe = BTreeSet::from([type_path.clone(), trait_path.clone()]);
        let crate_name = CrateName::new("domain").expect("valid crate");

        let resolved_type = resolve_catalogue_identity_in_namespace(
            &reference("Shared"),
            &crate_name,
            &universe,
            Some(CatalogueItemNamespace::Type),
        )
        .expect("type namespace resolves independently");
        let resolved_trait = resolve_catalogue_identity_in_namespace(
            &reference("Shared"),
            &crate_name,
            &universe,
            Some(CatalogueItemNamespace::Trait),
        )
        .expect("trait namespace resolves independently");
        assert_eq!(resolved_type, type_path);
        assert_eq!(resolved_trait, trait_path);
    }

    #[test]
    fn test_resolve_catalogue_identity_in_namespace_resolves_combined_rustdoc_and_catalogue_add_universe()
     {
        let catalogue_crate = CrateName::new("domain").expect("valid crate");
        let rustdoc_identity = identity("domain", &["rustdoc"], "Existing");
        let catalogue_add_identity = identity("domain", &["generated"], "Added");
        let combined_universe =
            BTreeSet::from([rustdoc_identity.clone(), catalogue_add_identity.clone()]);

        let resolved = resolve_catalogue_identity_in_namespace(
            &reference("Added"),
            &catalogue_crate,
            &combined_universe,
            Some(CatalogueItemNamespace::Type),
        )
        .expect("a catalogue Add identity resolves from the combined universe");

        assert_eq!(resolved, catalogue_add_identity);
        assert_eq!(resolved.namespace(), CatalogueItemNamespace::Type);
        assert_ne!(resolved, rustdoc_identity);
    }

    #[test]
    fn test_resolve_catalogue_identity_in_namespace_absent_combined_universe_fails_closed() {
        let catalogue_crate = CrateName::new("domain").expect("valid crate");
        let combined_universe = BTreeSet::from([
            identity("domain", &["rustdoc"], "Existing"),
            identity("domain", &["generated"], "Added"),
        ]);

        let error = resolve_catalogue_identity_in_namespace(
            &reference("Missing"),
            &catalogue_crate,
            &combined_universe,
            Some(CatalogueItemNamespace::Type),
        )
        .expect_err("a reference absent from rustdoc and catalogue must fail closed");

        assert!(matches!(
            error,
            CatalogueIdentityResolutionError::UnresolvedIdentifier(unresolved)
                if unresolved.as_str() == "Missing"
        ));
    }

    #[test]
    fn test_resolve_catalogue_identity_declared_helper_covers_catalogue_universe_and_fail_closed_branches()
     {
        let catalogue_crate = CrateName::new("domain").expect("valid crate");
        let first = identity("domain", &["generated"], "First");
        let second = identity("domain", &["generated"], "Second");
        let universe = BTreeSet::from([second.clone(), first.clone()]);

        // The declared three-parameter helper resolves catalogue-derived
        // identities from one caller-owned universe, independently of the
        // order in which the declarations were collected.
        assert_eq!(
            resolve_catalogue_identity(&reference("First"), &catalogue_crate, &universe)
                .expect("first catalogue declaration resolves"),
            first
        );
        assert_eq!(
            resolve_catalogue_identity(
                &reference("domain::generated::Second"),
                &catalogue_crate,
                &universe
            )
            .expect("qualified second catalogue declaration resolves"),
            second
        );
        let near_match = resolve_catalogue_identity(
            &reference("domain::other::Second"),
            &catalogue_crate,
            &universe,
        )
        .expect_err("a qualified near-match must not fall back by suffix");
        assert!(matches!(near_match, CatalogueIdentityResolutionError::UnresolvedIdentifier(_)));

        // The route adapters hand the same combined universe to this
        // declared helper. Baseline, current, and catalogue-derived entries
        // remain independently addressable once combined by the caller.
        let baseline_identity = identity("domain", &["baseline"], "Existing");
        let current_identity = identity("domain", &["current"], "Current");
        let catalogue_identity = identity("domain", &["generated"], "Added");
        let combined_universe = BTreeSet::from([
            baseline_identity.clone(),
            current_identity.clone(),
            catalogue_identity.clone(),
        ]);
        for (spelling, expected) in [
            ("Existing", baseline_identity),
            ("Current", current_identity),
            ("Added", catalogue_identity),
        ] {
            assert_eq!(
                resolve_catalogue_identity(
                    &reference(spelling),
                    &catalogue_crate,
                    &combined_universe
                )
                .expect("combined route universe resolves through one helper"),
                expected
            );
        }

        let absent = resolve_catalogue_identity(&reference("Missing"), &catalogue_crate, &universe)
            .expect_err("a reference absent from rustdoc and catalogue must fail closed");
        assert!(matches!(absent, CatalogueIdentityResolutionError::UnresolvedIdentifier(_)));

        // The unscoped helper does not collapse same-named Rust namespaces;
        // the action-aware namespace form can then select the declared type
        // identity without being confused by the same-named trait.
        let shared_type = identity("domain", &["types"], "Shared");
        let shared_trait = trait_identity("domain", &["traits"], "Shared");
        let shared_universe = BTreeSet::from([shared_type.clone(), shared_trait.clone()]);
        let ambiguous =
            resolve_catalogue_identity(&reference("Shared"), &catalogue_crate, &shared_universe)
                .expect_err("same-named type and trait must not silently collapse");
        assert!(matches!(
            ambiguous,
            CatalogueIdentityResolutionError::AmbiguousIdentifier(_, candidates)
                if candidates.as_slice().contains(&shared_type)
                    && candidates.as_slice().contains(&shared_trait)
        ));
        let namespace_selected = resolve_catalogue_identity_for_action_in_namespace(
            &reference("Shared"),
            &catalogue_crate,
            ItemAction::Modify,
            &shared_universe,
            &BTreeSet::new(),
            CatalogueItemNamespace::Type,
        )
        .expect("modify resolves the sole baseline candidate in its namespace");
        assert_eq!(namespace_selected, shared_type);

        // D3's Add and Modify branches are explicit: an add may resolve to a
        // sole current candidate, while modify must reject zero and multiple
        // baseline candidates instead of guessing.
        let add_resolved = resolve_catalogue_identity_for_action_in_namespace(
            &reference("Second"),
            &catalogue_crate,
            ItemAction::Add,
            &BTreeSet::new(),
            &BTreeSet::from([second.clone()]),
            CatalogueItemNamespace::Type,
        )
        .expect("add-to-add reference resolves through the current universe");
        assert_eq!(add_resolved, second);

        let modify_without_baseline = resolve_catalogue_identity_for_action_in_namespace(
            &reference("First"),
            &catalogue_crate,
            ItemAction::Modify,
            &BTreeSet::new(),
            &universe,
            CatalogueItemNamespace::Type,
        )
        .expect_err("modify without a baseline candidate must fail closed");
        assert!(matches!(
            modify_without_baseline,
            CatalogueIdentityResolutionError::UnresolvedIdentifier(_)
        ));

        let multiple_baseline = BTreeSet::from([
            identity("domain", &["alpha"], "First"),
            identity("domain", &["beta"], "First"),
        ]);
        let modify_with_multiple_baseline = resolve_catalogue_identity_for_action_in_namespace(
            &reference("First"),
            &catalogue_crate,
            ItemAction::Modify,
            &multiple_baseline,
            &BTreeSet::new(),
            CatalogueItemNamespace::Type,
        )
        .expect_err("modify with multiple baseline candidates must fail closed");
        assert!(matches!(
            modify_with_multiple_baseline,
            CatalogueIdentityResolutionError::AmbiguousIdentifier(_, _)
        ));
    }
}
