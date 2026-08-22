//! Fully qualified trait-role indexing for obligation derivation.

use std::path::PathBuf;

use domain::tddd::catalogue_v2::roles::ContractRole;
use domain::tddd::catalogue_v2::{CatalogueDocument, FullyQualifiedItemPath, TraitRefScope};
use domain::tddd::test_obligation::ids::{DiagnosticMessage, TestObligationAnchorId};

use super::{anchors_from_spec_refs, diag};

/// Trait metadata needed to derive obligations for matching `trait_impl`s.
pub(super) struct TraitRoleEntry {
    pub(super) identity: FullyQualifiedItemPath,
    pub(super) role: ContractRole,
    pub(super) anchors: Vec<TestObligationAnchorId>,
    pub(super) declaration_text: String,
}

/// Indexes every catalogue `TraitEntry` under its fully qualified identity so a
/// `trait_impl`'s `trait_ref` can be resolved across crates without collapsing
/// same-named traits from different modules (IN-03 / CO-01).
pub(super) fn index_trait_roles(
    catalogues: &[(PathBuf, CatalogueDocument)],
) -> Result<Vec<TraitRoleEntry>, DiagnosticMessage> {
    let mut index = Vec::new();
    for (_, catalogue) in catalogues {
        for (name, entry) in catalogue.traits() {
            index.push(TraitRoleEntry {
                identity: FullyQualifiedItemPath::from_catalogue_entry_key(
                    catalogue.crate_name(),
                    name,
                    entry.module_path(),
                )
                .map_err(|error| diag(&error.to_string()))?,
                role: entry.role().clone(),
                anchors: anchors_from_spec_refs(entry.spec_refs())?,
                declaration_text: format!("{entry:?}"),
            });
        }
    }
    Ok(index)
}

/// Resolves a trait impl's classified `trait_ref` to the matching catalogue trait.
///
/// Bare trait refs are self-crate refs and may resolve only when exactly one
/// full-path candidate exists in the impl catalogue's crate. Workspace-qualified
/// refs are matched against their exact full path. External refs yield no
/// obligations because there is no catalogue `TraitEntry` role to project.
pub(super) fn resolve_trait_role<'a>(
    trait_roles: &'a [TraitRoleEntry],
    scope: &TraitRefScope,
    impl_crate_name: &str,
) -> Result<Option<&'a TraitRoleEntry>, DiagnosticMessage> {
    match scope {
        TraitRefScope::SelfCrate(key) => {
            let mut candidates = trait_roles.iter().filter(|entry| {
                entry.identity.crate_name().as_str() == impl_crate_name
                    && entry.identity.name().as_str() == key.as_str()
            });
            let Some(candidate) = candidates.next() else {
                return Ok(None);
            };
            let candidate_identity = candidate.identity.clone();
            if candidates.any(|entry| entry.identity != candidate_identity) {
                Ok(None)
            } else {
                // Preserve the existing first-match behavior when the same
                // catalogue identity is repeated across input catalogues.
                Ok(Some(candidate))
            }
        }
        TraitRefScope::Workspace(key) => {
            let identity = FullyQualifiedItemPath::from_fully_qualified_key(key)
                .map_err(|error| diag(&error.to_string()))?;
            Ok(trait_roles.iter().find(|entry| entry.identity == identity))
        }
        TraitRefScope::External => Ok(None),
    }
}
