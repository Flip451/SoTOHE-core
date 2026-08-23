//! Global canonical-identity indexes for contract-map resolution.
//!
//! All items are `pub(super)` — implementation details of the render module.

use std::collections::{BTreeMap, BTreeSet};

#[cfg(test)]
use domain::tddd::catalogue_v2::Identifier;
use domain::tddd::catalogue_v2::identity_resolution::resolve_catalogue_identity;
use domain::tddd::catalogue_v2::{
    CatalogueDocument, CatalogueEntryKey, CrateName, FullyQualifiedItemPath, ModulePath, TypeRef,
};

use super::{trait_rep_node_id, type_rep_node_id};

// ---------------------------------------------------------------------------
// Canonical identity index
// ---------------------------------------------------------------------------

/// Index of declared catalogue identities and their rendered node ids.
///
/// The catalogue key is not used as an identity lookup. Each entry is first
/// represented as a [`FullyQualifiedItemPath`], and every reference is resolved
/// through the domain identity resolver against the complete index universe.
/// An identity with more than one rendered node is marked ambiguous and cannot
/// be selected by a caller.
pub(crate) struct NodeIndex {
    nodes: BTreeMap<FullyQualifiedItemPath, Option<String>>,
    universe: BTreeSet<FullyQualifiedItemPath>,
}

impl NodeIndex {
    pub(crate) fn new() -> Self {
        Self { nodes: BTreeMap::new(), universe: BTreeSet::new() }
    }

    /// Inserts a synthetic root identity. This helper is retained for focused
    /// renderer tests; production indexes use [`Self::insert_catalogue_entry`].
    #[cfg(test)]
    pub(crate) fn insert(&mut self, crate_name: &str, bare_name: &str, node_id: String) {
        let Ok(crate_name) = CrateName::new(crate_name.to_owned()) else {
            return;
        };
        let Ok(name) = Identifier::new(bare_name.to_owned()) else {
            return;
        };
        self.insert_identity(
            FullyQualifiedItemPath::new(crate_name, ModulePath::root(), name),
            node_id,
        );
    }

    /// Inserts a catalogue entry under its canonical identity.
    pub(crate) fn insert_catalogue_entry(
        &mut self,
        crate_name: &CrateName,
        key: &CatalogueEntryKey,
        module_path: &ModulePath,
        node_id: String,
    ) {
        let Ok(identity) =
            FullyQualifiedItemPath::from_catalogue_entry_key(crate_name, key, module_path)
        else {
            return;
        };
        self.insert_identity(identity, node_id);
    }

    fn insert_identity(&mut self, identity: FullyQualifiedItemPath, node_id: String) {
        self.universe.insert(identity.clone());
        match self.nodes.entry(identity) {
            std::collections::btree_map::Entry::Vacant(entry) => {
                entry.insert(Some(node_id));
            }
            std::collections::btree_map::Entry::Occupied(mut entry) => {
                let current = entry.get_mut();
                if current.as_deref() != Some(node_id.as_str()) {
                    *current = None;
                }
            }
        }
    }

    /// Looks up a type or trait reference through the shared domain resolver.
    ///
    /// The input may be a complete type expression such as `Wrapper<T>`; the
    /// root path is extracted by `syn`, while identity selection itself remains
    /// owned by `resolve_catalogue_identity`. Ambiguous and unresolved paths
    /// return `None`, so the renderer never guesses a node.
    pub(crate) fn resolve(&self, type_ref_str: &str, current_crate: &str) -> Option<&str> {
        let syn_type = syn::parse_str::<syn::Type>(type_ref_str).ok()?;
        let syn::Type::Path(type_path) = syn_type else {
            return None;
        };
        if type_path.qself.is_some() {
            return None;
        }

        let mut path = type_path
            .path
            .segments
            .iter()
            .map(|segment| segment.ident.to_string())
            .collect::<Vec<_>>()
            .join("::");
        if type_path.path.leading_colon.is_some() {
            path.insert_str(0, "::");
        }
        // `self` and `super` are syntax-relative prefixes, not catalogue
        // identities. The resolver accepts the crate-relative spelling; the
        // identity comparison itself remains delegated to the domain choke
        // point below. Catalogue TypeRefs do not carry a module context, so
        // both prefixes use the same crate-relative candidate here.
        let path = path
            .strip_prefix("self::")
            .or_else(|| path.strip_prefix("super::"))
            .map_or(path.clone(), |relative| format!("crate::{relative}"));
        let reference = TypeRef::new(path).ok()?;
        let catalogue_crate = CrateName::new(current_crate.to_owned()).ok()?;
        let absolute_crate =
            reference.as_str().strip_prefix("::").and_then(|path| path.split("::").next());
        let universe = self
            .universe
            .iter()
            .filter(|identity| {
                if let Some(absolute_crate) = absolute_crate {
                    identity.crate_name().as_str() == absolute_crate
                } else {
                    reference.as_str().contains("::") || identity.crate_name() == &catalogue_crate
                }
            })
            .cloned()
            .collect::<BTreeSet<_>>();
        let identity = resolve_catalogue_identity(&reference, &catalogue_crate, &universe).ok()?;
        self.nodes.get(&identity).and_then(Option::as_deref)
    }
}

// ---------------------------------------------------------------------------
// Index builders
// ---------------------------------------------------------------------------

/// Build a global trait index from all catalogues.
///
/// The returned index is separate from the type index so a same-named type and
/// trait can never be conflated. Deleted entries are excluded from both the
/// node map and the resolution universe.
pub(crate) fn build_trait_index(catalogues: &[CatalogueDocument]) -> NodeIndex {
    use domain::tddd::catalogue_v2::roles::ItemAction;

    let mut index = NodeIndex::new();
    for doc in catalogues {
        let layer = doc.layer().as_ref();
        let crate_name = doc.crate_name();
        for (trait_name, trait_entry) in doc.traits() {
            if trait_entry.action() == ItemAction::Delete {
                continue;
            }
            let rep_node_id = trait_rep_node_id(layer, crate_name.as_str(), trait_name.as_str());
            index.insert_catalogue_entry(
                crate_name,
                trait_name,
                trait_entry.module_path(),
                rep_node_id,
            );
        }
    }
    index
}

/// Build a global type index from all catalogues for TypeRef resolution.
///
/// Only `TypeEntry` values are included. Trait references use the separate
/// index from [`build_trait_index`], preventing a TypeRef from linking to a
/// same-named trait node.
pub(crate) fn build_node_index(catalogues: &[CatalogueDocument]) -> NodeIndex {
    use domain::tddd::catalogue_v2::roles::ItemAction;

    let mut index = NodeIndex::new();
    for doc in catalogues {
        let layer = doc.layer().as_ref();
        let crate_name = doc.crate_name();
        for (type_name, type_entry) in doc.types() {
            if type_entry.action() == ItemAction::Delete {
                continue;
            }
            let rep_node_id = type_rep_node_id(layer, crate_name.as_str(), type_name.as_str());
            index.insert_catalogue_entry(
                crate_name,
                type_name,
                type_entry.module_path(),
                rep_node_id,
            );
        }
    }
    index
}
