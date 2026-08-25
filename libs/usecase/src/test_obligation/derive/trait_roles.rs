//! Fully qualified trait-role indexing for obligation derivation.

use std::path::PathBuf;

use domain::tddd::catalogue_v2::roles::ContractRole;
use domain::tddd::catalogue_v2::{
    CatalogueDocument, CrateName, TraitEntry, TraitImplDeclV2, TraitRefScope, TypeRef,
};
use domain::tddd::test_obligation::ids::{DiagnosticMessage, TestObligationAnchorId};

use super::identity::CatalogueDeclarationIdentity;
use super::{anchors_from_spec_refs, declaration_identity, diag, resolve_catalogue_reference};

/// Trait metadata needed to derive obligations for matching `trait_impl`s.
pub(super) struct TraitRoleEntry {
    pub(super) identity: CatalogueDeclarationIdentity,
    catalogue_entry: TraitEntry,
    pub(super) role: ContractRole,
    pub(super) anchors: Vec<TestObligationAnchorId>,
    pub(super) declaration_text: String,
}

/// Indexes every catalogue `TraitEntry` under its finite declaration-owned
/// spelling set so same-named traits from different modules remain distinct.
pub(super) fn index_trait_roles(
    catalogues: &[(PathBuf, CatalogueDocument)],
) -> Result<Vec<TraitRoleEntry>, DiagnosticMessage> {
    let mut index = Vec::new();
    for (_, catalogue) in catalogues {
        for (name, entry) in catalogue.traits() {
            let identity = declaration_identity(catalogue.crate_name(), name, entry.module_path())?;
            let anchors = anchors_from_spec_refs(entry.spec_refs())?;
            let declaration_text = format!("{entry:?}");
            if let Some(existing) =
                index.iter().find(|existing: &&TraitRoleEntry| existing.identity == identity)
            {
                if existing.catalogue_entry.eq(entry) {
                    continue;
                }
                return Err(diag(&format!(
                    "conflicting catalogue trait declarations for '{}'",
                    identity.fully_qualified_path()
                )));
            }
            index.push(TraitRoleEntry {
                identity,
                catalogue_entry: entry.clone(),
                role: entry.role().clone(),
                anchors,
                declaration_text,
            });
        }
    }
    Ok(index)
}

/// Resolves a trait impl's `trait_ref` to the matching catalogue trait.
///
/// The trait universe is intentionally separate from type declarations. A
/// bare `trait_ref` is self-crate scoped by [`TraitImplDeclV2`]; module- and
/// crate-qualified spellings retain the global closed-grammar lookup. A unique
/// exact spelling is local, no match is external, and an ambiguous spelling is
/// reported with all fully qualified candidates.
pub(super) fn resolve_trait_role<'a>(
    trait_roles: &'a [TraitRoleEntry],
    implementing_crate: &CrateName,
    trait_ref: &TypeRef,
) -> Result<Option<&'a TraitRoleEntry>, DiagnosticMessage> {
    let is_bare_self_crate = matches!(
        TraitImplDeclV2::new(trait_ref.clone(), trait_ref.clone()).trait_ref_scope(),
        TraitRefScope::SelfCrate(_)
    );
    let declarations = trait_roles.iter().filter(|entry| {
        !is_bare_self_crate
            || entry.identity.fully_qualified_path().crate_name() == implementing_crate
    });
    let Some(identity) =
        resolve_catalogue_reference(trait_ref, declarations.map(|entry| &entry.identity))?
    else {
        return Ok(None);
    };
    trait_roles.iter().find(|entry| entry.identity == *identity).map_or_else(
        || {
            Err(diag(&format!(
                "catalogue trait declaration '{}' disappeared while resolving trait_ref '{}'",
                identity.stored_key().as_str(),
                trait_ref.as_str()
            )))
        },
        |entry| Ok(Some(entry)),
    )
}
