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
//! 2. External crate collection: gather `TraitImplDeclV2::origin_crate` names and
//!    `TypeRef` crate prefixes to build `Crate::external_crates`.
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

use std::collections::{HashMap, HashSet};

use domain::tddd::CatalogueToExtendedCratePort;
use domain::tddd::NewTypeGraphCodecError;
use domain::tddd::catalogue_v2::CatalogueEntryKey;
use domain::tddd::catalogue_v2::identifiers::TypeRef;
use domain::tddd::extended_crate::ExtendedCrate;
use domain::tddd::test_obligation::ids::{DiagnosticMessage, unavailable_diagnostic_message};
use rustdoc_types::{Crate, Id, ItemSummary};

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
    Function,
    Other(rustdoc_types::ItemKind),
}

fn path_namespace(kind: rustdoc_types::ItemKind) -> PathNamespace {
    match kind {
        rustdoc_types::ItemKind::Struct
        | rustdoc_types::ItemKind::Union
        | rustdoc_types::ItemKind::Enum
        | rustdoc_types::ItemKind::TypeAlias
        | rustdoc_types::ItemKind::Trait
        | rustdoc_types::ItemKind::TraitAlias
        | rustdoc_types::ItemKind::ExternType
        | rustdoc_types::ItemKind::Primitive => PathNamespace::Type,
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
        Encoder::new(doc, authoritative_paths(baseline, current)).run()
    }
}

fn authoritative_paths(baseline: &Crate, current: &Crate) -> HashMap<Id, ItemSummary> {
    let mut paths = baseline.paths.clone();
    let mut known_paths = paths
        .values()
        .map(|summary| (summary.path.clone(), path_namespace(summary.kind)))
        .collect::<HashSet<_>>();
    let mut next_id = paths.keys().map(|id| id.0).max().unwrap_or(0).saturating_add(1);
    for (id, summary) in &current.paths {
        // Baseline and current rustdoc crates assign ids independently. The same
        // identity therefore commonly appears under two ids; retaining both would
        // make a bare catalogue path look ambiguous even though it names one item.
        if known_paths.contains(&(summary.path.clone(), path_namespace(summary.kind))) {
            continue;
        }
        match paths.get(id) {
            Some(existing)
                if existing.path == summary.path
                    && path_namespace(existing.kind) == path_namespace(summary.kind) => {}
            Some(_) => {
                paths.insert(Id(next_id), summary.clone());
                known_paths.insert((summary.path.clone(), path_namespace(summary.kind)));
                next_id = next_id.saturating_add(1);
            }
            None => {
                paths.insert(*id, summary.clone());
                known_paths.insert((summary.path.clone(), path_namespace(summary.kind)));
            }
        }
    }
    paths
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic, clippy::expect_used)]
#[path = "catalogue_to_extended_crate_codec_tests.rs"]
mod tests;
