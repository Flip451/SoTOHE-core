//! Closed catalogue spelling matching shared by obligation derivation consumers.

use std::collections::BTreeSet;

use domain::tddd::catalogue_v2::entries::TypeEntry;
use domain::tddd::catalogue_v2::identifiers::{CrateName, FullyQualifiedItemPath, TypeRef};
use domain::tddd::catalogue_v2::{CatalogueDocument, CatalogueEntryKey, ModulePath, TraitEntry};
use domain::tddd::test_obligation::ids::DiagnosticMessage;

use super::{catalogue_key, diag};

/// The finite declaration-owned spelling set used by derive-time identity matching.
///
/// `stored_key` is retained as the downstream catalogue identity. The other two
/// spellings are constructed from the declaration's module and crate context; no
/// spelling supplied by a reference is normalized into either form.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CatalogueDeclarationIdentity {
    stored_key: CatalogueEntryKey,
    fully_qualified_path: FullyQualifiedItemPath,
    accepted_spellings: BTreeSet<String>,
}

impl CatalogueDeclarationIdentity {
    /// Returns the catalogue key that owns the declaration.
    #[must_use]
    pub(crate) fn stored_key(&self) -> &CatalogueEntryKey {
        &self.stored_key
    }

    /// Returns the fully qualified path used in ambiguity diagnostics.
    #[must_use]
    pub(crate) fn fully_qualified_path(&self) -> &FullyQualifiedItemPath {
        &self.fully_qualified_path
    }

    fn accepts(&self, reference: &TypeRef) -> bool {
        self.accepted_spellings.contains(reference.as_str())
    }
}

/// Builds the finite spelling set for one catalogue declaration.
pub(crate) fn declaration_identity(
    crate_name: &CrateName,
    stored_key: &CatalogueEntryKey,
    module_path: &ModulePath,
) -> Result<CatalogueDeclarationIdentity, DiagnosticMessage> {
    let key_identity = declaration_key_identity(crate_name, stored_key, module_path)?;
    let name = key_identity.name().as_str();
    let local_spelling = join_module_and_name(module_path, name);
    let fully_qualified_path = FullyQualifiedItemPath::new(
        crate_name.clone(),
        module_path.clone(),
        key_identity.name().clone(),
    );
    let crate_spelling = format!("{}::{local_spelling}", crate_name.as_str());
    let accepted_spellings =
        [stored_key.as_str(), local_spelling.as_str(), crate_spelling.as_str()]
            .into_iter()
            .map(str::to_owned)
            .collect();

    Ok(CatalogueDeclarationIdentity {
        stored_key: stored_key.clone(),
        fully_qualified_path,
        accepted_spellings,
    })
}

/// Resolves a reference against the caller-supplied declaration universe.
///
/// Matching is exact against each declaration's finite spelling set. No match
/// is external; multiple matches fail closed with every candidate's fully
/// qualified path.
pub(crate) fn resolve_catalogue_reference<'a, I>(
    reference: &TypeRef,
    declarations: I,
) -> Result<Option<&'a CatalogueDeclarationIdentity>, DiagnosticMessage>
where
    I: IntoIterator<Item = &'a CatalogueDeclarationIdentity>,
{
    let matches = declarations
        .into_iter()
        .filter(|declaration| declaration.accepts(reference))
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [] => Ok(None),
        [declaration] => Ok(Some(declaration)),
        _ => Err(diag(&format_ambiguous_reference(reference, &matches))),
    }
}

/// Resolves an inherent-impl owner through the type declaration universe.
pub(crate) fn resolve_named_type_entry<'a>(
    catalogue: &'a CatalogueDocument,
    reference: &CatalogueEntryKey,
) -> Result<Option<(&'a CatalogueEntryKey, &'a TypeEntry)>, DiagnosticMessage> {
    let type_ref = TypeRef::new(reference.as_str().to_owned())
        .map_err(|error| diag(&format!("invalid inherent_impl type_name: {error}")))?;
    let identities = catalogue_type_identities(catalogue)?;
    let Some(identity) = resolve_catalogue_reference(&type_ref, &identities)? else {
        return Ok(None);
    };
    catalogue.types().iter().find(|(key, _)| *key == identity.stored_key()).map(Some).ok_or_else(
        || {
            diag(&format!(
                "catalogue type declaration '{}' disappeared while resolving inherent_impl owner",
                identity.stored_key().as_str()
            ))
        },
    )
}

/// Resolves a self type to its catalogue-stored key when it is local.
///
/// External or absent self types retain their verbatim source spelling because
/// trait implementations may legitimately target types outside this catalogue.
/// Ambiguous local identities remain errors and never fall back to a short key.
pub(crate) fn resolve_named_type_key(
    catalogue: &CatalogueDocument,
    reference: &TypeRef,
) -> Result<CatalogueEntryKey, DiagnosticMessage> {
    let identities = catalogue_type_identities(catalogue)?;
    match resolve_catalogue_reference(reference, &identities)? {
        Some(identity) => Ok(identity.stored_key().clone()),
        None => catalogue_key(reference.as_str()),
    }
}

/// Builds the type-only declaration universe for one catalogue.
pub(crate) fn catalogue_type_identities(
    catalogue: &CatalogueDocument,
) -> Result<Vec<CatalogueDeclarationIdentity>, DiagnosticMessage> {
    catalogue
        .types()
        .iter()
        .map(|(key, entry)| declaration_identity(catalogue.crate_name(), key, entry.module_path()))
        .collect()
}

/// Builds the trait-only declaration universe for all supplied catalogues.
///
/// Exact duplicate snapshots are deduplicated so re-reading the same catalogue
/// does not turn one declaration into an artificial ambiguity. Conflicting
/// snapshots for one identity fail closed rather than depending on input order.
pub(crate) fn catalogues_trait_identities(
    catalogues: &[&CatalogueDocument],
) -> Result<Vec<CatalogueDeclarationIdentity>, DiagnosticMessage> {
    let mut declarations: Vec<(CatalogueDeclarationIdentity, TraitEntry)> = Vec::new();
    for catalogue in catalogues {
        for (key, entry) in catalogue.traits() {
            let identity = declaration_identity(catalogue.crate_name(), key, entry.module_path())?;
            if let Some((_, existing_entry)) =
                declarations.iter().find(|(existing, _)| *existing == identity)
            {
                if existing_entry == entry {
                    continue;
                }
                return Err(diag(&format!(
                    "conflicting catalogue trait declarations for '{}'",
                    identity.fully_qualified_path()
                )));
            }
            declarations.push((identity, entry.clone()));
        }
    }
    Ok(declarations.into_iter().map(|(identity, _)| identity).collect())
}

/// Resolves a trait reference and returns its declaration text, if it is local.
pub(crate) fn trait_declaration_text_for_reference(
    catalogues: &[&CatalogueDocument],
    reference: &TypeRef,
) -> Result<Option<String>, DiagnosticMessage> {
    let identities = catalogues_trait_identities(catalogues)?;
    let Some(identity) = resolve_catalogue_reference(reference, &identities)? else {
        return Ok(None);
    };
    for catalogue in catalogues {
        for (key, entry) in catalogue.traits() {
            let candidate = declaration_identity(catalogue.crate_name(), key, entry.module_path())?;
            if candidate == *identity {
                return Ok(Some(format!("{entry:?}")));
            }
        }
    }
    Err(diag(&format!(
        "catalogue trait declaration '{}' disappeared while resolving trait reference",
        identity.stored_key().as_str()
    )))
}

fn format_ambiguous_reference(
    reference: &TypeRef,
    matches: &[&CatalogueDeclarationIdentity],
) -> String {
    let candidates = matches
        .iter()
        .map(|declaration| declaration.fully_qualified_path().to_string())
        .collect::<Vec<_>>();
    format!(
        "catalogue reference '{}' is ambiguous; candidates: {}",
        reference.as_str(),
        candidates.join(", ")
    )
}

fn declaration_key_identity(
    crate_name: &CrateName,
    stored_key: &CatalogueEntryKey,
    module_path: &ModulePath,
) -> Result<FullyQualifiedItemPath, DiagnosticMessage> {
    FullyQualifiedItemPath::from_catalogue_entry_key(crate_name, stored_key, module_path).map_err(
        |error| {
            diag(&format!(
                "invalid catalogue declaration identity '{}': {error}",
                stored_key.as_str()
            ))
        },
    )
}

fn join_module_and_name(module_path: &ModulePath, name: &str) -> String {
    if module_path.is_root() { name.to_owned() } else { format!("{module_path}::{name}") }
}
