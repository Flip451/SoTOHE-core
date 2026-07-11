//! Test-obligation gate use cases (`bin/sotp test-obligation {derive,check,evaluate,results}`)
//! plus authoring helpers.
//!
//! The four interactors form the SoT-chain third link (impl → catalogue) as an
//! obligation-fulfillment gate (GO-01 / GO-02):
//!
//! * [`mod@derive`] — deterministic projection `(catalogue, spec, rules) → obligations`.
//! * [`mod@check`] — pure-read totality + drift + existence-based scope gate.
//! * [`mod@evaluate`] — LLM-backed fulfillment / waiver semantic verification with a
//!   three-hash frozen verdict cache.
//! * [`mod@results`] — informational aggregation of the frozen verdict caches.
//! * [`mod@bindings_skeleton`] — read-only skeleton generation for implementer-authored bindings.
//!
//! Shared, verifier-agnostic helpers (active-track guard, content hashing, and
//! catalogue declaration canonicalisation) live here so `derive` and `check`
//! freeze and compare declaration hashes identically.

mod check_contract;
mod check_sources;
mod check_support;
mod results_status;
mod status_lanes;

pub mod bindings_skeleton;
pub mod check;
pub mod derive;
pub mod evaluate;
pub mod hasher;
pub mod results;

pub use domain::tddd::test_obligation::errors;

use std::path::{Component, Path, PathBuf};

use domain::ContentHash;
use domain::TrackId;
use domain::tddd::catalogue_v2::CatalogueDocument;
use domain::tddd::semantic_verify::{CatalogueEntryRef, CatalogueSectionKey};
use domain::tddd::test_obligation::ids::DiagnosticMessage;
use domain::tddd::test_obligation::obligations::TestObligation;
use domain::tddd::test_obligation::vocab::TargetEntryRoleKind;

/// A loaded catalogue paired with the command path that produced it.
#[derive(Debug, Clone)]
pub(crate) struct LoadedCatalogueDocument {
    read_path: PathBuf,
    read_file_path: String,
    artifact_file_path: String,
    document: CatalogueDocument,
}

impl LoadedCatalogueDocument {
    /// Builds a loaded catalogue record from a command path and parsed document.
    #[must_use]
    pub(crate) fn new(path: &Path, document: CatalogueDocument) -> Self {
        let read_path = path.to_path_buf();
        let read_file_path = path.display().to_string();
        let artifact_file_path = catalogue_artifact_path(path);
        Self { read_path, read_file_path, artifact_file_path, document }
    }

    /// Returns the parsed catalogue document.
    #[must_use]
    pub(crate) fn document(&self) -> &CatalogueDocument {
        &self.document
    }

    /// Returns the command path used to load this catalogue.
    #[must_use]
    pub(crate) fn read_path(&self) -> &Path {
        &self.read_path
    }

    pub(super) fn matches_file_path(&self, file_path: &str) -> bool {
        self.read_file_path == file_path
            || self.artifact_file_path == catalogue_artifact_path(Path::new(file_path))
    }
}

/// Shared input for commands that read catalogue snapshots for a track.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestObligationCatalogueCommandInput {
    track_id: TrackId,
    current_branch: String,
    catalogue_paths: Vec<PathBuf>,
}

impl TestObligationCatalogueCommandInput {
    /// Builds shared catalogue-command input.
    #[must_use]
    pub fn new(track_id: TrackId, current_branch: String, catalogue_paths: Vec<PathBuf>) -> Self {
        Self { track_id, current_branch, catalogue_paths }
    }

    fn track_id(&self) -> &TrackId {
        &self.track_id
    }

    fn current_branch(&self) -> &str {
        &self.current_branch
    }

    fn catalogue_paths(&self) -> &[PathBuf] {
        &self.catalogue_paths
    }
}

/// Builds a non-empty [`DiagnosticMessage`], substituting a constant for empty
/// input so a diagnostic can never be silent. Mirrors the panic-free codec
/// helper (`infrastructure::test_obligation::diagnostic`).
#[must_use]
pub(crate) fn diag(message: &str) -> DiagnosticMessage {
    let mut text = if message.trim().is_empty() {
        "unspecified test-obligation error".to_owned()
    } else {
        message.to_owned()
    };
    loop {
        match DiagnosticMessage::try_new(text) {
            Ok(message) => return message,
            // Unreachable: `text` is non-empty on entry; reset defensively.
            Err(_) => text = "unspecified test-obligation error".to_owned(),
        }
    }
}

/// Returns `true` when `current_branch` is the active track branch for
/// `track_id` (`track/<id>`), mirroring the ref-verify active-track guard.
#[must_use]
pub(crate) fn is_active_branch(track_id: &TrackId, current_branch: &str) -> bool {
    current_branch == format!("track/{}", track_id.as_ref())
}

/// Computes a SHA-256 [`ContentHash`] over `bytes`.
///
/// Used by `derive` (freeze) and `check` (recompute + compare) so the two agree
/// on the canonical hash of a catalogue declaration without routing through the
/// `ContentHasherPort` (which is reserved for the `evaluate` cache-key freeze).
#[must_use]
pub(crate) fn sha256_content_hash(bytes: &[u8]) -> ContentHash {
    use sha2::{Digest as _, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let mut out = [0u8; 32];
    out.copy_from_slice(&hasher.finalize());
    ContentHash::from_bytes(out)
}

/// Canonical declaration text for the catalogue entry named `key` in the
/// `section_key` section, if present.
///
/// The `Debug` rendering captures every declaration field deterministically, so
/// hashing it detects any declaration content change (`decl_changed` drift) and
/// serves as the evidence-side declaration text for the fulfillment verifier.
#[must_use]
pub(crate) fn entry_declaration_text(
    catalogue: &CatalogueDocument,
    section_key: &CatalogueSectionKey,
    key: &str,
) -> Option<String> {
    match section_key {
        CatalogueSectionKey::Types => {
            catalogue.types().iter().find(|(n, _)| n.as_str() == key).map(|(_, e)| format!("{e:?}"))
        }
        CatalogueSectionKey::Traits => catalogue
            .traits()
            .iter()
            .find(|(n, _)| n.as_str() == key)
            .map(|(_, e)| format!("{e:?}")),
        CatalogueSectionKey::Functions => catalogue
            .functions()
            .iter()
            .find(|(p, _)| p.to_string() == key)
            .map(|(_, e)| format!("{e:?}")),
    }
}

/// Searches every catalogue for an unambiguous declaration named `key`.
///
/// Voluntary / unresolved edges carry only a bare entry key. If that key appears
/// in more than one catalogue section, return `None` so callers escalate rather
/// than verifying against the wrong declaration.
#[cfg(test)]
#[must_use]
pub(crate) fn find_declaration_text(catalogues: &[CatalogueDocument], key: &str) -> Option<String> {
    let mut found = None;
    for catalogue in catalogues {
        for section in [
            CatalogueSectionKey::Types,
            CatalogueSectionKey::Traits,
            CatalogueSectionKey::Functions,
        ] {
            if let Some(text) = entry_declaration_text(catalogue, &section, key) {
                if found.is_some() {
                    return None;
                }
                found = Some(text);
            }
        }
    }
    found
}

/// Searches every loaded catalogue for an unambiguous declaration named `key`.
#[must_use]
pub(crate) fn find_declaration_text_from_loaded(
    catalogues: &[LoadedCatalogueDocument],
    key: &str,
) -> Option<String> {
    let mut found = None;
    for catalogue in catalogues {
        for section in [
            CatalogueSectionKey::Types,
            CatalogueSectionKey::Traits,
            CatalogueSectionKey::Functions,
        ] {
            if let Some(text) = entry_declaration_text(catalogue.document(), &section, key) {
                if found.is_some() {
                    return None;
                }
                found = Some(text);
            }
        }
    }
    found
}

/// Resolves the declaration text for a derived obligation.
///
/// Trait-impl obligations use the implementer type as their entry key but freeze
/// the actual `TraitImplDeclV2`; use the item identifier (`trait_impl:<trait>`)
/// to re-resolve that same impl declaration instead of falling back to a type
/// declaration with the same key.
#[cfg(test)]
#[must_use]
pub(crate) fn obligation_declaration_text(
    catalogues: &[CatalogueDocument],
    obligation: &TestObligation,
) -> Option<String> {
    if matches!(obligation.target_role(), TargetEntryRoleKind::TraitImpl(_)) {
        return trait_impl_declaration_text(catalogues, obligation);
    }
    target_entry_declaration_text(catalogues, obligation.target_entry())
}

/// Resolves the declaration text for a derived obligation from its recorded
/// catalogue file.
#[must_use]
pub(crate) fn obligation_declaration_text_from_loaded(
    catalogues: &[LoadedCatalogueDocument],
    obligation: &TestObligation,
) -> Option<String> {
    let matching = catalogues
        .iter()
        .filter(|catalogue| catalogue.matches_file_path(&obligation.target_entry().file_path))
        .map(LoadedCatalogueDocument::document);
    if matches!(obligation.target_role(), TargetEntryRoleKind::TraitImpl(_)) {
        return trait_impl_declaration_text_in(matching, obligation);
    }
    target_entry_declaration_text_in(matching, obligation.target_entry())
}

#[cfg(test)]
fn target_entry_declaration_text(
    catalogues: &[CatalogueDocument],
    target: &CatalogueEntryRef,
) -> Option<String> {
    target_entry_declaration_text_in(catalogues.iter(), target)
}

fn target_entry_declaration_text_in<'a>(
    catalogues: impl IntoIterator<Item = &'a CatalogueDocument>,
    target: &CatalogueEntryRef,
) -> Option<String> {
    for catalogue in catalogues {
        if let Some(text) =
            entry_declaration_text(catalogue, &target.section_key, target.entry_key.as_str())
        {
            return Some(text);
        }
    }
    None
}

#[cfg(test)]
fn trait_impl_declaration_text(
    catalogues: &[CatalogueDocument],
    obligation: &TestObligation,
) -> Option<String> {
    trait_impl_declaration_text_in(catalogues.iter(), obligation)
}

fn trait_impl_declaration_text_in<'a>(
    catalogues: impl IntoIterator<Item = &'a CatalogueDocument>,
    obligation: &TestObligation,
) -> Option<String> {
    let trait_ref = obligation.id().item_identifier().as_str().strip_prefix("trait_impl:")?;
    let for_type = obligation.id().entry_key().as_str();
    for catalogue in catalogues {
        for impl_decl in catalogue.trait_impls() {
            if impl_decl.for_type().as_str() == for_type
                && impl_decl.trait_ref().as_str() == trait_ref
            {
                return Some(format!("{impl_decl:?}"));
            }
        }
    }
    None
}

/// Returns the artifact identity for a catalogue path.
///
/// Catalogue reads use paths anchored at the discovered workspace root. Track
/// artifacts must instead remain portable across checkouts, so a catalogue path
/// below `track/items/` is stored relative to that project root. Paths outside
/// that shape retain their supplied display form.
#[must_use]
pub(crate) fn catalogue_artifact_path(path: &Path) -> String {
    let components: Vec<Component<'_>> = path.components().collect();
    let Some(start) = components.windows(2).position(|pair| {
        matches!(pair, [Component::Normal(track), Component::Normal(items)]
            if *track == "track" && *items == "items")
    }) else {
        return path.display().to_string();
    };

    components
        .iter()
        .skip(start)
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

/// Collects the spec-anchor element ids cited by every catalogue entry across
/// all layers (the "cited set" for the uncited `AC` / `CN` finding — IN-16).
#[must_use]
pub(crate) fn cited_anchor_ids(catalogues: &[CatalogueDocument]) -> Vec<String> {
    use crate::catalogue_traversal::iter_catalogue_entries;
    use domain::tddd::catalogue_v2::roles::ItemAction;

    let mut ids = Vec::new();
    for catalogue in catalogues {
        for entry in iter_catalogue_entries(catalogue) {
            if matches!(entry.action, ItemAction::Delete) {
                continue;
            }
            for spec_ref in entry.spec_refs {
                let id = spec_ref.anchor.as_ref().to_owned();
                if !ids.contains(&id) {
                    ids.push(id);
                }
            }
        }
    }
    ids
}
