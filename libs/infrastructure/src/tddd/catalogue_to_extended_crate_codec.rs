//! Catalogue → ExtendedCrate (TypeGraph A) codec.
//!
//! `CatalogueToExtendedCrateCodec` converts a domain `CatalogueDocument` into an
//! `ExtendedCrate` (TypeGraph A). It implements the secondary-adapter role for the
//! `CatalogueToExtendedCratePort` port declared in the domain layer.
//!
//! ## Conversion pipeline (ADR 2 D8 / D9 / D10 / D11)
//!
//! 1. Pre-pass Id assignment: assign incremental `rustdoc_types::Id`s to all entries.
//!    Id(0) is reserved for the root module.
//! 2. External crate collection: the TypeRef parser gathers crate prefixes from
//!    trait implementations and their nested type expressions to build
//!    `Crate::external_crates`.
//! 3. TypeRef parse: convert each `TypeRef` string via `syn::parse_str` into
//!    `rustdoc_types::Type`. Unresolvable identifiers become open-world "unresolved
//!    markers" (ADR 2 D10).
//! 4. Inline → id-ref: `FieldDecl` / `VariantDecl` are promoted to individual
//!    `rustdoc_types::Item` entries and the parent references them via `Vec<Id>`.
//! 5. Inherent impl grouping: all `MethodDeclaration`s on a type are grouped into a
//!    single `Impl` item per type.
//! 6. Trait impl blocks: `TraitImplDeclV2` entries produce `Impl` items with trait
//!    identity only (no method items — ADR 2 D12).
//! 7. Crate.paths: each in-crate item gets an `ItemSummary` with
//!    `path = [crate_name, ...module_path, item_name]`.
//! 8. item_actions: each catalogue entry's `ItemAction` is recorded in
//!    `ExtendedCrate::item_actions`.
//!
//! (infrastructure-types.json: CatalogueToExtendedCrateCodec)

use std::collections::{BTreeSet, HashMap, HashSet};

use domain::tddd::CatalogueToExtendedCratePort;
use domain::tddd::NewTypeGraphCodecError;
use domain::tddd::catalogue_v2::identifiers::{
    CatalogueItemNamespace, FullyQualifiedItemPath, Identifier, ModulePath, TypeRef,
};
use domain::tddd::catalogue_v2::identity_resolution::{
    CatalogueIdentityResolutionError, resolve_catalogue_identity_for_action_in_namespace,
};
use domain::tddd::catalogue_v2::{CatalogueDocument, CatalogueEntryKey, ItemAction};
use domain::tddd::extended_crate::ExtendedCrate;
use domain::tddd::test_obligation::ids::{DiagnosticMessage, unavailable_diagnostic_message};
use rustdoc_types::{Crate, Id, ItemSummary};

use crate::tddd::canonical_type_identity::{
    SYNTHETIC_UNPLACED_CRATE_ID, canonicalize_rustdoc_root_path,
};

#[path = "catalogue_to_extended_crate_codec/encoder.rs"]
mod encoder;
#[path = "catalogue_to_extended_crate_codec/encoder_deletions.rs"]
mod encoder_deletions;
#[path = "catalogue_to_extended_crate_codec/encoder_state_core.rs"]
mod encoder_state_core;
#[path = "catalogue_to_extended_crate_codec/encoder_state_fn_trait_codec.rs"]
mod encoder_state_fn_trait_codec;
#[path = "catalogue_to_extended_crate_codec/encoder_state_type_codec.rs"]
mod encoder_state_type_codec;
#[path = "catalogue_to_extended_crate_codec/encoder_state_type_ref_parsing.rs"]
mod encoder_state_type_ref_parsing;
#[path = "catalogue_to_extended_crate_codec/encoder_state_type_ref_resolution.rs"]
mod encoder_state_type_ref_resolution;
#[path = "catalogue_to_extended_crate_codec/helpers.rs"]
mod helpers;

use encoder::Encoder;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum PathNamespace {
    Type,
    Trait,
    Function,
    Other(rustdoc_types::ItemKind),
}

fn path_namespace(kind: rustdoc_types::ItemKind) -> PathNamespace {
    match kind {
        rustdoc_types::ItemKind::Struct
        | rustdoc_types::ItemKind::Union
        | rustdoc_types::ItemKind::Enum
        | rustdoc_types::ItemKind::TypeAlias
        | rustdoc_types::ItemKind::ExternType
        | rustdoc_types::ItemKind::Primitive => PathNamespace::Type,
        rustdoc_types::ItemKind::Trait | rustdoc_types::ItemKind::TraitAlias => {
            PathNamespace::Trait
        }
        rustdoc_types::ItemKind::Function => PathNamespace::Function,
        other => PathNamespace::Other(other),
    }
}

// ---------------------------------------------------------------------------
// CatalogueToExtendedCrateCodec
// ---------------------------------------------------------------------------

/// Stateless codec that converts `CatalogueDocument` → `ExtendedCrate` (TypeGraph A).
///
/// Implements `CatalogueToExtendedCratePort`. Instantiate with `new()` and call
/// `encode()`.
#[derive(Debug, Clone, Default)]
pub struct CatalogueToExtendedCrateCodec;

/// Builds the domain error used by every codec parsing boundary.
pub(super) fn invalid_type_ref(
    type_ref: impl Into<String>,
    reason: impl Into<String>,
) -> NewTypeGraphCodecError {
    let mut raw_type_ref = type_ref.into();
    if raw_type_ref.is_empty() {
        raw_type_ref = "<empty TypeRef>".to_owned();
    }
    let type_ref = loop {
        match TypeRef::new(raw_type_ref.clone()) {
            Ok(type_ref) => break type_ref,
            Err(_) => raw_type_ref = "<invalid TypeRef>".to_owned(),
        }
    };
    let diagnostic = reason.into();
    let diagnostic =
        DiagnosticMessage::try_new(diagnostic).unwrap_or_else(|_| unavailable_diagnostic_message());
    NewTypeGraphCodecError::InvalidTypeRef(type_ref, diagnostic)
}

pub(super) fn map_identity_resolution_error(
    error: CatalogueIdentityResolutionError,
) -> NewTypeGraphCodecError {
    match error {
        CatalogueIdentityResolutionError::AmbiguousIdentifier(identifier, candidates) => {
            NewTypeGraphCodecError::AmbiguousIdentifier(identifier, candidates)
        }
        CatalogueIdentityResolutionError::UnresolvedIdentifier(type_ref) => {
            NewTypeGraphCodecError::UnresolvedIdentifier(type_ref)
        }
        CatalogueIdentityResolutionError::ClassificationFailed { location } => {
            NewTypeGraphCodecError::UnresolvedIdentifier(location)
        }
    }
}

/// Returns the declared item segment from a short or qualified catalogue key.
pub(super) fn entry_item_name(key: &CatalogueEntryKey) -> &str {
    key.as_str().rsplit("::").next().unwrap_or(key.as_str())
}

impl CatalogueToExtendedCrateCodec {
    /// Creates a new codec instance.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl CatalogueToExtendedCratePort for CatalogueToExtendedCrateCodec {
    fn encode(
        &self,
        doc: domain::tddd::catalogue_v2::CatalogueDocument,
        baseline: &Crate,
        current: &Crate,
    ) -> Result<ExtendedCrate, NewTypeGraphCodecError> {
        let resolution_paths = resolution_paths_for_catalogue(&doc, baseline, current)?;
        // Non-add declarations and deletions must resolve against the baseline
        // only (D3): keep the baseline identity set separate from the merged
        // resolution set the add route reads.
        let baseline_paths = normalized_paths_for_doc(baseline, doc.crate_name());
        let baseline_identities =
            paths_from_map_for_catalogue_crate(&baseline_paths, doc.crate_name());
        Encoder::new(doc, resolution_paths, baseline_identities).run()
    }
}

/// Builds the one rustdoc/catalogue resolution set consumed by the codec.
///
/// Rustdoc remains authoritative for existing declarations. Catalogue `add`
/// declarations are appended as synthetic summaries so declaration-first work
/// can reference a type before rustdoc contains its implementation. Omitted
/// placement is resolved against the current set when there is exactly one
/// candidate; otherwise it remains unplaced or fails closed according to D3.
pub(super) fn resolution_paths_for_catalogue(
    doc: &CatalogueDocument,
    baseline: &Crate,
    current: &Crate,
) -> Result<HashMap<Id, ItemSummary>, NewTypeGraphCodecError> {
    let baseline_paths = normalized_paths_for_doc(baseline, doc.crate_name());
    let current_paths = normalized_paths_for_doc(current, doc.crate_name());
    let mut paths = merge_authoritative_paths(&baseline_paths, &current_paths)?;
    let baseline_identities = paths_from_map(&baseline_paths);
    let current_identities = paths_from_map(&current_paths);
    let mut known_paths = paths
        .values()
        .map(|summary| (summary.path.clone(), path_namespace(summary.kind)))
        .collect::<HashSet<_>>();
    let mut used_ids = paths.keys().copied().collect::<HashSet<_>>();
    let mut next_id = used_ids.iter().map(|id| id.0).max().unwrap_or(0).saturating_add(1);

    for (key, entry) in doc.types() {
        if entry.action() != ItemAction::Add {
            continue;
        }
        let identity = resolve_add_identity(
            doc.crate_name(),
            key,
            entry.module_path(),
            CatalogueItemNamespace::Type,
            &baseline_identities,
            &current_identities,
        )?;
        insert_synthetic_summary(
            &mut paths,
            &mut known_paths,
            &mut next_id,
            &mut used_ids,
            &identity,
            rustdoc_types::ItemKind::Struct,
        )?;
    }
    for (key, entry) in doc.traits() {
        if entry.action() != ItemAction::Add {
            continue;
        }
        let identity = resolve_add_identity(
            doc.crate_name(),
            key,
            entry.module_path(),
            CatalogueItemNamespace::Trait,
            &baseline_identities,
            &current_identities,
        )?;
        insert_synthetic_summary(
            &mut paths,
            &mut known_paths,
            &mut next_id,
            &mut used_ids,
            &identity,
            rustdoc_types::ItemKind::Trait,
        )?;
    }
    Ok(paths)
}

fn resolve_add_identity(
    crate_name: &domain::tddd::catalogue_v2::CrateName,
    key: &CatalogueEntryKey,
    declared_module_path: Option<&ModulePath>,
    namespace: CatalogueItemNamespace,
    baseline: &BTreeSet<FullyQualifiedItemPath>,
    current: &BTreeSet<FullyQualifiedItemPath>,
) -> Result<FullyQualifiedItemPath, NewTypeGraphCodecError> {
    let identity = match namespace {
        CatalogueItemNamespace::Type => FullyQualifiedItemPath::from_type_catalogue_entry_key(
            crate_name,
            key,
            declared_module_path,
        ),
        CatalogueItemNamespace::Trait => FullyQualifiedItemPath::from_trait_catalogue_entry_key(
            crate_name,
            key,
            declared_module_path,
        ),
    }
    .map_err(|error| {
        invalid_type_ref(key.as_str(), format!("invalid catalogue identity: {error}"))
    })?;
    let reference = TypeRef::new(if identity.is_placed() {
        identity.to_string()
    } else {
        key.as_str().to_owned()
    })
    .map_err(|error| {
        invalid_type_ref(key.as_str(), format!("invalid catalogue identity: {error}"))
    })?;
    resolve_catalogue_identity_for_action_in_namespace(
        &reference,
        crate_name,
        ItemAction::Add,
        baseline,
        current,
        namespace,
    )
    .map_err(map_identity_resolution_error)
}

fn insert_synthetic_summary(
    paths: &mut HashMap<Id, ItemSummary>,
    known_paths: &mut HashSet<(Vec<String>, PathNamespace)>,
    next_id: &mut u32,
    used_ids: &mut HashSet<Id>,
    identity: &FullyQualifiedItemPath,
    kind: rustdoc_types::ItemKind,
) -> Result<(), NewTypeGraphCodecError> {
    let path = identity_path(identity);
    let namespace = path_namespace(kind);
    if !known_paths.insert((path.clone(), namespace)) {
        return Ok(());
    }
    let id = next_unused_id(next_id, used_ids)
        .ok_or_else(|| invalid_type_ref("catalogue paths", "no unused item id remains"))?;
    let crate_id = if identity.is_placed() { 0 } else { SYNTHETIC_UNPLACED_CRATE_ID };
    paths.insert(id, ItemSummary { crate_id, path, kind });
    Ok(())
}

fn identity_path(identity: &FullyQualifiedItemPath) -> Vec<String> {
    let mut path = vec![identity.crate_name().as_str().to_owned()];
    if let Some(module_path) = identity.module_path() {
        path.extend(module_path.segments().iter().map(|segment| segment.as_str().to_owned()));
    }
    path.push(identity.name().as_str().to_owned());
    path
}

fn paths_from_map(paths: &HashMap<Id, ItemSummary>) -> BTreeSet<FullyQualifiedItemPath> {
    paths.values().filter_map(summary_identity).collect()
}

fn paths_from_map_for_catalogue_crate(
    paths: &HashMap<Id, ItemSummary>,
    catalogue_crate: &domain::tddd::catalogue_v2::CrateName,
) -> BTreeSet<FullyQualifiedItemPath> {
    paths
        .values()
        .filter(|summary| summary.crate_id == 0)
        .filter_map(summary_identity)
        .filter(|identity| identity.crate_name() == catalogue_crate)
        .collect()
}

fn summary_identity(summary: &ItemSummary) -> Option<FullyQualifiedItemPath> {
    if !matches!(path_namespace(summary.kind), PathNamespace::Type | PathNamespace::Trait) {
        return None;
    }
    let (crate_name, rest) = summary.path.split_first()?;
    let (name, module_segments) = rest.split_last()?;
    let crate_name = domain::tddd::catalogue_v2::CrateName::new(crate_name.clone()).ok()?;
    let name = Identifier::new(name.clone()).ok()?;
    if summary.crate_id == SYNTHETIC_UNPLACED_CRATE_ID && !module_segments.is_empty() {
        return None;
    }
    let module_path = ModulePath::from_segments(module_segments.to_vec()).ok()?;
    let namespace = if matches!(
        summary.kind,
        rustdoc_types::ItemKind::Trait | rustdoc_types::ItemKind::TraitAlias
    ) {
        CatalogueItemNamespace::Trait
    } else {
        CatalogueItemNamespace::Type
    };
    Some(match (namespace, summary.crate_id == SYNTHETIC_UNPLACED_CRATE_ID) {
        (CatalogueItemNamespace::Type, true) => {
            FullyQualifiedItemPath::new_unplaced_type(crate_name, name)
        }
        (CatalogueItemNamespace::Trait, true) => {
            FullyQualifiedItemPath::new_unplaced_trait(crate_name, name)
        }
        (CatalogueItemNamespace::Type, false) => {
            FullyQualifiedItemPath::new_type(crate_name, module_path, name)
        }
        (CatalogueItemNamespace::Trait, false) => {
            FullyQualifiedItemPath::new_trait(crate_name, module_path, name)
        }
    })
}

#[cfg(test)]
#[allow(clippy::expect_used)]
fn authoritative_paths(baseline: &Crate, current: &Crate) -> HashMap<Id, ItemSummary> {
    merge_authoritative_paths(&baseline.paths, &current.paths).expect("path ids must be available")
}

pub(super) fn normalized_paths_for_doc(
    krate: &Crate,
    package_name: &domain::tddd::catalogue_v2::CrateName,
) -> HashMap<Id, ItemSummary> {
    let rustdoc_root = krate
        .index
        .get(&krate.root)
        .and_then(|item| item.name.as_deref())
        .and_then(|name| domain::tddd::catalogue_v2::CrateName::new(name.to_owned()).ok());
    krate
        .paths
        .iter()
        .map(|(id, summary)| {
            let mut normalized = summary.clone();
            normalized.path = if summary.crate_id == 0 {
                canonicalize_rustdoc_root_path(&summary.path, package_name, rustdoc_root.as_ref())
            } else {
                summary.path.clone()
            };
            (*id, normalized)
        })
        .collect()
}

fn merge_authoritative_paths(
    baseline: &HashMap<Id, ItemSummary>,
    current: &HashMap<Id, ItemSummary>,
) -> Result<HashMap<Id, ItemSummary>, NewTypeGraphCodecError> {
    let mut paths = baseline.clone();
    // Both rustdoc runs allocate Ids independently. Reserve the complete input
    // union before allocating a replacement so a current-only Id that has not
    // been visited yet cannot be overwritten by a remapped baseline collision.
    let mut used_ids = baseline.keys().chain(current.keys()).copied().collect::<HashSet<_>>();
    let mut known_paths = paths
        .values()
        .map(|summary| (summary.path.clone(), path_namespace(summary.kind)))
        .collect::<HashSet<_>>();
    let mut next_id = used_ids.iter().map(|id| id.0).max().unwrap_or(0).saturating_add(1);
    let mut current_entries = current.iter().collect::<Vec<_>>();
    current_entries.sort_unstable_by_key(|(id, _)| id.0);
    for (&id, summary) in current_entries {
        // Baseline and current rustdoc crates assign ids independently. The same
        // identity therefore commonly appears under two ids; retaining both would
        // make a bare catalogue path look ambiguous even though it names one item.
        if known_paths.contains(&(summary.path.clone(), path_namespace(summary.kind))) {
            continue;
        }
        match paths.get(&id) {
            Some(existing)
                if existing.path == summary.path
                    && path_namespace(existing.kind) == path_namespace(summary.kind) => {}
            Some(_) => {
                let fresh_id = next_unused_id(&mut next_id, &mut used_ids).ok_or_else(|| {
                    invalid_type_ref("rustdoc paths", "no unused item id remains")
                })?;
                paths.insert(fresh_id, summary.clone());
                known_paths.insert((summary.path.clone(), path_namespace(summary.kind)));
            }
            None => {
                paths.insert(id, summary.clone());
                known_paths.insert((summary.path.clone(), path_namespace(summary.kind)));
            }
        }
    }
    Ok(paths)
}

fn next_unused_id(next_id: &mut u32, used_ids: &mut HashSet<Id>) -> Option<Id> {
    loop {
        let candidate = Id(*next_id);
        if used_ids.insert(candidate) {
            *next_id = (*next_id).checked_add(1).map_or(u32::MAX, |next| next);
            return Some(candidate);
        }
        *next_id = (*next_id).checked_add(1)?;
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic, clippy::expect_used)]
#[path = "catalogue_to_extended_crate_codec_tests.rs"]
mod tests;
