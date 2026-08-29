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

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

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
use domain::tddd::layer_id::LayerId;
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

/// Marker used for placed synthetic declarations from another catalogue.
///
/// The resolution-path map has no accompanying `external_crates` table, so it
/// only needs a non-local crate id to preserve the external-vs-local distinction
/// while the final encoder allocates the real external crate id on demand.
const SYNTHETIC_EXTERNAL_CRATE_ID: u32 = u32::MAX - 1;

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
        target_layer: &LayerId,
        track_catalogues: &BTreeMap<LayerId, CatalogueDocument>,
        baseline: &Crate,
        current: &Crate,
    ) -> Result<ExtendedCrate, NewTypeGraphCodecError> {
        let doc = track_catalogues.get(target_layer).ok_or_else(|| {
            invalid_type_ref(
                target_layer.to_string(),
                "target layer has no catalogue in the track catalogue map",
            )
        })?;
        let resolution_paths =
            resolution_paths_for_catalogue(target_layer, track_catalogues, baseline, current)?;
        // Non-add declarations and deletions must resolve against the baseline
        // only (D3): keep the baseline identity set separate from the merged
        // resolution set the add route reads.
        let baseline_paths = normalized_paths_for_doc(baseline, doc.crate_name());
        let baseline_identities =
            paths_from_map_for_catalogue_crate(&baseline_paths, doc.crate_name());
        Encoder::new(doc.clone(), resolution_paths, baseline_identities).run()
    }
}

/// Encodes a single catalogue as the target layer's track-catalogue map.
#[cfg(test)]
pub(crate) fn encode_document(
    doc: CatalogueDocument,
    baseline: &Crate,
    current: &Crate,
) -> Result<ExtendedCrate, NewTypeGraphCodecError> {
    let layer = doc.layer().clone();
    let catalogues = BTreeMap::from([(layer.clone(), doc)]);
    CatalogueToExtendedCrateCodec::new().encode(&layer, &catalogues, baseline, current)
}

/// Builds the shared resolution set for a single-catalogue test or caller.
#[cfg(all(test, feature = "test-helpers"))]
pub(crate) fn resolution_paths_for_document(
    doc: &CatalogueDocument,
    baseline: &Crate,
    current: &Crate,
) -> Result<HashMap<Id, ItemSummary>, NewTypeGraphCodecError> {
    let layer = doc.layer().clone();
    let catalogues = BTreeMap::from([(layer.clone(), doc.clone())]);
    resolution_paths_for_catalogue(&layer, &catalogues, baseline, current)
}

/// Builds the one rustdoc/catalogue resolution set consumed by the codec.
///
/// Rustdoc remains authoritative for existing declarations. Catalogue `add`
/// declarations are appended as synthetic summaries so declaration-first work
/// can reference a type before rustdoc contains its implementation. Add
/// declarations from other track catalogues use their declaring crate as the
/// external identity root; the referencing catalogue does not need a duplicate
/// declaration. Omitted placement is resolved against the current set when
/// there is exactly one candidate; otherwise it remains unplaced or fails closed
/// according to D3.
pub(super) fn resolution_paths_for_catalogue(
    target_layer: &LayerId,
    track_catalogues: &BTreeMap<LayerId, CatalogueDocument>,
    baseline: &Crate,
    current: &Crate,
) -> Result<HashMap<Id, ItemSummary>, NewTypeGraphCodecError> {
    let target_doc = track_catalogues.get(target_layer).ok_or_else(|| {
        invalid_type_ref(
            target_layer.to_string(),
            "target layer has no catalogue in the track catalogue map",
        )
    })?;
    let baseline_paths = normalized_paths_for_doc(baseline, target_doc.crate_name());
    let current_paths = normalized_paths_for_doc(current, target_doc.crate_name());
    let mut paths = merge_authoritative_paths(&baseline_paths, &current_paths)?;
    let baseline_identities = paths_from_map(&baseline_paths);
    let current_identities = paths_from_map(&current_paths);
    let mut known_paths = paths
        .values()
        .map(|summary| (summary.path.clone(), path_namespace(summary.kind)))
        .collect::<HashSet<_>>();
    let mut used_ids = paths.keys().copied().collect::<HashSet<_>>();
    let mut next_id = used_ids.iter().map(|id| id.0).max().unwrap_or(0).saturating_add(1);

    // Keep the target layer's existing add route first. Other layers extend the
    // same set; they do not create a second resolution path or require a
    // duplicate entry in the target catalogue.
    insert_catalogue_additions(
        target_doc,
        false,
        &mut paths,
        &mut known_paths,
        &mut next_id,
        &mut used_ids,
        &baseline_identities,
        &current_identities,
    )?;
    for (layer, catalogue) in track_catalogues {
        if layer == target_layer {
            continue;
        }
        insert_catalogue_additions(
            catalogue,
            true,
            &mut paths,
            &mut known_paths,
            &mut next_id,
            &mut used_ids,
            &baseline_identities,
            &current_identities,
        )?;
    }
    Ok(paths)
}

#[allow(clippy::too_many_arguments)]
fn insert_catalogue_additions(
    catalogue: &CatalogueDocument,
    external: bool,
    paths: &mut HashMap<Id, ItemSummary>,
    known_paths: &mut HashSet<(Vec<String>, PathNamespace)>,
    next_id: &mut u32,
    used_ids: &mut HashSet<Id>,
    baseline_identities: &BTreeSet<FullyQualifiedItemPath>,
    current_identities: &BTreeSet<FullyQualifiedItemPath>,
) -> Result<(), NewTypeGraphCodecError> {
    for (key, entry) in catalogue.types() {
        if entry.action() != ItemAction::Add {
            continue;
        }
        let declared_identity = catalogue_entry_identity(
            catalogue.crate_name(),
            key,
            entry.module_path(),
            CatalogueItemNamespace::Type,
        )?;
        if external {
            let path = identity_path(&declared_identity);
            if known_paths.contains(&(path, PathNamespace::Type)) {
                continue;
            }
        }
        let identity = resolve_add_identity(
            catalogue.crate_name(),
            key,
            entry.module_path(),
            CatalogueItemNamespace::Type,
            baseline_identities,
            current_identities,
        )?;
        insert_synthetic_summary(
            paths,
            known_paths,
            next_id,
            used_ids,
            &identity,
            rustdoc_types::ItemKind::Struct,
            external,
        )?;
    }
    for (key, entry) in catalogue.traits() {
        if entry.action() != ItemAction::Add {
            continue;
        }
        let declared_identity = catalogue_entry_identity(
            catalogue.crate_name(),
            key,
            entry.module_path(),
            CatalogueItemNamespace::Trait,
        )?;
        if external {
            let path = identity_path(&declared_identity);
            if known_paths.contains(&(path, PathNamespace::Trait)) {
                continue;
            }
        }
        let identity = resolve_add_identity(
            catalogue.crate_name(),
            key,
            entry.module_path(),
            CatalogueItemNamespace::Trait,
            baseline_identities,
            current_identities,
        )?;
        insert_synthetic_summary(
            paths,
            known_paths,
            next_id,
            used_ids,
            &identity,
            rustdoc_types::ItemKind::Trait,
            external,
        )?;
    }
    Ok(())
}

fn resolve_add_identity(
    crate_name: &domain::tddd::catalogue_v2::CrateName,
    key: &CatalogueEntryKey,
    declared_module_path: Option<&ModulePath>,
    namespace: CatalogueItemNamespace,
    baseline: &BTreeSet<FullyQualifiedItemPath>,
    current: &BTreeSet<FullyQualifiedItemPath>,
) -> Result<FullyQualifiedItemPath, NewTypeGraphCodecError> {
    let identity = catalogue_entry_identity(crate_name, key, declared_module_path, namespace)?;
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

fn catalogue_entry_identity(
    crate_name: &domain::tddd::catalogue_v2::CrateName,
    key: &CatalogueEntryKey,
    declared_module_path: Option<&ModulePath>,
    namespace: CatalogueItemNamespace,
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
    if key.as_str().contains("::") {
        if let (Some(declared), Some(actual)) = (declared_module_path, identity.module_path()) {
            if declared != actual {
                return Err(invalid_type_ref(
                    key.as_str(),
                    format!(
                        "catalogue key '{}' implies module_path '{}', but entry module_path is '{}',",
                        key.as_str(),
                        actual,
                        declared
                    ),
                ));
            }
        }
    }
    Ok(identity)
}

fn insert_synthetic_summary(
    paths: &mut HashMap<Id, ItemSummary>,
    known_paths: &mut HashSet<(Vec<String>, PathNamespace)>,
    next_id: &mut u32,
    used_ids: &mut HashSet<Id>,
    identity: &FullyQualifiedItemPath,
    kind: rustdoc_types::ItemKind,
    external: bool,
) -> Result<(), NewTypeGraphCodecError> {
    let path = identity_path(identity);
    let namespace = path_namespace(kind);
    if !known_paths.insert((path.clone(), namespace)) {
        return Ok(());
    }
    let id = next_unused_id(next_id, used_ids)
        .ok_or_else(|| invalid_type_ref("catalogue paths", "no unused item id remains"))?;
    let crate_id = if identity.is_placed() {
        if external { SYNTHETIC_EXTERNAL_CRATE_ID } else { 0 }
    } else {
        SYNTHETIC_UNPLACED_CRATE_ID
    };
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

pub(super) fn paths_from_map(paths: &HashMap<Id, ItemSummary>) -> BTreeSet<FullyQualifiedItemPath> {
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
