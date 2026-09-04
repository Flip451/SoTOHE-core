//! Catalogue and trait-impl identity indexing for type-signal reports.

use std::collections::{BTreeMap, HashMap};

use domain::tddd::catalogue_v2::identifiers::CatalogueItemNamespace;
use domain::tddd::catalogue_v2::{
    CatalogueDocument, CrateName, DeletionRecord, FullyQualifiedItemPath, ModulePath, ParamName,
    TypeRef,
};
use rustdoc_types::{Id, ItemSummary};

use crate::tddd::canonical_type_identity::canonicalize_catalogue_type_ref;

/// Catalogue aliases used to map report labels to persisted entry keys.
/// Canonical identities come from T004; renderer path spellings are not identities.
#[derive(Debug, Default)]
pub(crate) struct TypeSignalIdentityIndex {
    canonical_to_keys: BTreeMap<String, Vec<String>>,
    declaration_aliases_to_keys: BTreeMap<String, Vec<String>>,
    impl_aliases_to_keys: BTreeMap<String, Vec<String>>,
    namespaces_by_key: BTreeMap<String, Vec<CatalogueItemNamespace>>,
}

impl TypeSignalIdentityIndex {
    pub(crate) fn add_canonical(&mut self, canonical: &str, key: &str) {
        add_unique(&mut self.canonical_to_keys, canonical, key);
    }

    pub(crate) fn add_alias(&mut self, alias: &str, key: &str) {
        add_unique(&mut self.declaration_aliases_to_keys, alias, key);
    }

    pub(crate) fn add_impl_alias(&mut self, alias: &str, key: &str) {
        add_unique(&mut self.impl_aliases_to_keys, alias, key);
    }

    pub(crate) fn add_namespace(&mut self, key: &str, namespace: CatalogueItemNamespace) {
        let namespaces = self.namespaces_by_key.entry(key.to_owned()).or_default();
        if !namespaces.contains(&namespace) {
            namespaces.push(namespace);
        }
    }

    pub(crate) fn key_has_namespace(&self, key: &str, namespace: CatalogueItemNamespace) -> bool {
        self.namespaces_by_key.get(key).is_some_and(|namespaces| namespaces.contains(&namespace))
    }

    pub(crate) fn has_known_namespace(&self, key: &str) -> bool {
        self.namespaces_by_key.contains_key(key)
    }

    pub(crate) fn declaration_candidates(&self, raw: &str) -> Option<Vec<String>> {
        let exact =
            self.declaration_aliases_to_keys.get(raw).or_else(|| self.canonical_to_keys.get(raw));
        if exact.is_some() || raw.contains("::") {
            return exact.cloned();
        }
        self.declaration_aliases_to_keys.get(short_name(raw)).cloned()
    }

    pub(crate) fn declaration_candidates_in_namespace(
        &self,
        raw: &str,
        namespace: CatalogueItemNamespace,
    ) -> Option<Vec<String>> {
        let candidates = self.declaration_candidates(raw)?;
        let filtered = candidates
            .into_iter()
            .filter(|key| self.key_has_namespace(key, namespace))
            .collect::<Vec<_>>();
        (!filtered.is_empty()).then_some(filtered)
    }

    /// Returns candidates for an impl owner, excluding declaration entries in
    /// the trait namespace. External owners are retained because they have no
    /// catalogue namespace entry and must remain report labels.
    pub(crate) fn owner_candidates(&self, raw: &str) -> Option<Vec<String>> {
        if let Some(keys) = self.impl_aliases_to_keys.get(raw) {
            let filtered = self.type_owner_candidates(keys.clone());
            if !filtered.is_empty() {
                return Some(filtered);
            }
        }

        let mut candidates = Vec::new();
        add_candidates(&mut candidates, self.declaration_aliases_to_keys.get(raw));
        if let Some(keys) = self.canonical_to_keys.get(raw) {
            add_candidates(&mut candidates, Some(keys));
        }
        if !raw.contains("::") {
            let short = short_name(raw);
            if short != raw {
                if let Some(keys) = self.impl_aliases_to_keys.get(short) {
                    add_candidates(&mut candidates, Some(keys));
                }
                add_candidates(&mut candidates, self.declaration_aliases_to_keys.get(short));
            }
        }

        let candidates = self.type_owner_candidates(candidates);
        (!candidates.is_empty()).then_some(candidates)
    }

    fn type_owner_candidates(&self, candidates: Vec<String>) -> Vec<String> {
        candidates
            .into_iter()
            .filter(|key| {
                !self.has_known_namespace(key)
                    || self.key_has_namespace(key, CatalogueItemNamespace::Type)
            })
            .collect()
    }

    pub(crate) fn aliases_for_entry_key<'a>(&'a self, key: &str) -> Vec<&'a str> {
        let mut aliases = Vec::new();
        for (alias, keys) in
            self.canonical_to_keys.iter().chain(self.declaration_aliases_to_keys.iter())
        {
            if keys.len() == 1
                && keys.first().is_some_and(|candidate| candidate.as_str() == key)
                && !aliases.contains(&alias.as_str())
            {
                aliases.push(alias.as_str());
            }
        }
        aliases
    }
}

/// Builds the catalogue/impl identity join used by the type-signal producer.
/// Returns a diagnostic when a declaration cannot be reconciled with rustdoc paths.
pub(crate) fn build_type_signal_identity_index(
    catalogue: &CatalogueDocument,
    rustdoc_paths: &HashMap<Id, ItemSummary>,
) -> Result<TypeSignalIdentityIndex, String> {
    let mut index = TypeSignalIdentityIndex::default();
    let catalogue_crate = catalogue.crate_name();
    let type_paths = paths_for_namespace(rustdoc_paths, CatalogueItemNamespace::Type);

    for (key, entry) in catalogue.types() {
        add_entry_identity(
            &mut index,
            catalogue_crate,
            key.as_str(),
            entry.module_path(),
            CatalogueItemNamespace::Type,
            rustdoc_paths,
        )?;
    }
    for (key, entry) in catalogue.traits() {
        add_entry_identity(
            &mut index,
            catalogue_crate,
            key.as_str(),
            entry.module_path(),
            CatalogueItemNamespace::Trait,
            rustdoc_paths,
        )?;
    }

    for deletion in catalogue.deletions() {
        let (key, namespace) = match deletion {
            DeletionRecord::Type { name, .. } => (name, CatalogueItemNamespace::Type),
            DeletionRecord::Trait { name, .. } => (name, CatalogueItemNamespace::Trait),
            DeletionRecord::Function { .. } => continue,
        };
        add_deletion_identity(&mut index, catalogue_crate, key.as_str(), namespace, rustdoc_paths)?;
    }

    for trait_impl in catalogue.trait_impls() {
        let generic_params = trait_impl
            .impl_generics()
            .iter()
            .map(|param| param.name.clone())
            .collect::<Vec<ParamName>>();
        // `for_type` is always a type-position reference. Filtering the
        // resolution set here prevents a same-named trait from making an
        // otherwise unambiguous owner ambiguous.
        let canonical = canonicalize_catalogue_type_ref(
            trait_impl.for_type(),
            catalogue_crate,
            &type_paths,
            &generic_params,
        )
        .map_err(|error| {
            format!(
                "cannot canonicalize type-signal impl owner '{}': {error}",
                trait_impl.for_type()
            )
        })?;

        // Local type entries own impl rows; external types retain their canonical key.
        let owner_key =
            declaration_key_for_canonical(&index, canonical.as_str(), CatalogueItemNamespace::Type)
                .unwrap_or_else(|| canonical.as_str().to_owned());
        index.add_impl_alias(canonical.as_str(), &owner_key);
        index.add_impl_alias(trait_impl.for_type().as_str(), &owner_key);
        index.add_impl_alias(strip_generic_suffix(trait_impl.for_type().as_str()), &owner_key);
        index.add_impl_alias(short_name(trait_impl.for_type().as_str()), &owner_key);
    }

    Ok(index)
}

pub(crate) fn add_entry_identity(
    index: &mut TypeSignalIdentityIndex,
    catalogue_crate: &CrateName,
    key: &str,
    declared_module_path: Option<&ModulePath>,
    namespace: CatalogueItemNamespace,
    rustdoc_paths: &HashMap<Id, ItemSummary>,
) -> Result<(), String> {
    let entry_key = domain::tddd::catalogue_v2::CatalogueEntryKey::try_new(key.to_owned())
        .map_err(|error| format!("invalid catalogue entry key '{key}': {error}"))?;
    index.add_namespace(key, namespace);
    let identity = match namespace {
        CatalogueItemNamespace::Type => FullyQualifiedItemPath::from_type_catalogue_entry_key(
            catalogue_crate,
            &entry_key,
            declared_module_path,
        ),
        CatalogueItemNamespace::Trait => FullyQualifiedItemPath::from_trait_catalogue_entry_key(
            catalogue_crate,
            &entry_key,
            declared_module_path,
        ),
    }
    .map_err(|error| format!("cannot construct catalogue identity for '{key}': {error}"))?;
    let identity_text = identity.to_string();
    index.add_alias(key, key);
    index.add_alias(&identity_text, key);
    index.add_alias(short_name(key), key);
    let identity_ref =
        TypeRef::new(if identity.is_placed() { identity_text } else { key.to_owned() })
            .map_err(|error| format!("cannot construct TypeRef for '{key}': {error}"))?;
    let namespace_paths = paths_for_namespace(rustdoc_paths, namespace);
    let canonical =
        canonicalize_catalogue_type_ref(&identity_ref, catalogue_crate, &namespace_paths, &[])
            .map_err(|error| format!("cannot canonicalize catalogue entry '{key}': {error}"))?;
    index.add_canonical(canonical.as_str(), key);
    Ok(())
}

pub(crate) fn add_deletion_identity(
    index: &mut TypeSignalIdentityIndex,
    catalogue_crate: &CrateName,
    key: &str,
    namespace: CatalogueItemNamespace,
    rustdoc_paths: &HashMap<Id, ItemSummary>,
) -> Result<(), String> {
    let _entry_key = domain::tddd::catalogue_v2::CatalogueEntryKey::try_new(key.to_owned())
        .map_err(|error| format!("invalid catalogue deletion key '{key}': {error}"))?;
    index.add_namespace(key, namespace);
    let identity_ref = TypeRef::new(key.to_owned())
        .map_err(|error| format!("cannot construct TypeRef for '{key}': {error}"))?;
    index.add_alias(key, key);
    index.add_alias(short_name(key), key);
    let namespace_paths = paths_for_namespace(rustdoc_paths, namespace);
    let canonical =
        canonicalize_catalogue_type_ref(&identity_ref, catalogue_crate, &namespace_paths, &[])
            .map_err(|error| format!("cannot canonicalize catalogue deletion '{key}': {error}"))?;
    if canonical.as_str().is_empty() {
        return Err(format!("cannot canonicalize catalogue deletion '{key}'"));
    }
    index.add_canonical(canonical.as_str(), key);
    Ok(())
}

fn paths_for_namespace(
    rustdoc_paths: &HashMap<Id, ItemSummary>,
    namespace: CatalogueItemNamespace,
) -> HashMap<Id, ItemSummary> {
    rustdoc_paths
        .iter()
        .filter(|(_, summary)| match namespace {
            CatalogueItemNamespace::Type => matches!(
                summary.kind,
                rustdoc_types::ItemKind::Struct
                    | rustdoc_types::ItemKind::Union
                    | rustdoc_types::ItemKind::Enum
                    | rustdoc_types::ItemKind::TypeAlias
                    | rustdoc_types::ItemKind::ExternType
                    | rustdoc_types::ItemKind::Primitive
            ),
            CatalogueItemNamespace::Trait => matches!(
                summary.kind,
                rustdoc_types::ItemKind::Trait | rustdoc_types::ItemKind::TraitAlias
            ),
        })
        .map(|(&id, summary)| (id, summary.clone()))
        .collect()
}

fn add_unique(map: &mut BTreeMap<String, Vec<String>>, alias: &str, key: &str) {
    let keys = map.entry(alias.to_owned()).or_default();
    if !keys.iter().any(|existing| existing == key) {
        keys.push(key.to_owned());
    }
}

fn declaration_key_for_canonical(
    index: &TypeSignalIdentityIndex,
    canonical: &str,
    namespace: CatalogueItemNamespace,
) -> Option<String> {
    let owner_identity = strip_generic_suffix(canonical);
    unique_key_in_namespace(index, index.canonical_to_keys.get(canonical), namespace)
        .or_else(|| {
            unique_key_in_namespace(index, index.canonical_to_keys.get(owner_identity), namespace)
        })
        .map(ToOwned::to_owned)
}

fn unique_key_in_namespace<'a>(
    index: &'a TypeSignalIdentityIndex,
    keys: Option<&'a Vec<String>>,
    namespace: CatalogueItemNamespace,
) -> Option<&'a str> {
    let mut matching = keys?.iter().filter(|key| index.key_has_namespace(key, namespace));
    let first = matching.next()?;
    matching.next().is_none().then_some(first.as_str())
}

fn add_candidates(candidates: &mut Vec<String>, keys: Option<&Vec<String>>) {
    for key in keys.into_iter().flatten() {
        if !candidates.iter().any(|candidate| candidate == key) {
            candidates.push(key.clone());
        }
    }
}

fn strip_generic_suffix(raw: &str) -> &str {
    raw.split_once('<').map_or(raw, |(head, _)| head)
}

fn short_name(raw: &str) -> &str {
    strip_generic_suffix(raw).rsplit("::").next().unwrap_or(raw)
}
