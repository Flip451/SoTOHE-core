//! Global node and trait indexes for TypeRef/trait_ref resolution.
//!
//! All items are `pub(super)` — implementation details of the render module.

use std::collections::BTreeMap;

use domain::tddd::catalogue_v2::CatalogueDocument;

use super::{trait_rep_node_id, type_rep_node_id};

// ---------------------------------------------------------------------------
// Global node index for TypeRef resolution
// ---------------------------------------------------------------------------

/// Global node index for resolving `TypeRef` strings to rendered mermaid node IDs.
///
/// Built once per render call from all catalogue documents (Decision O-2/O-3
/// pattern, CN-05). Used to resolve field/param/return/variant TypeRef targets so
/// edges connect to the actual rendered subgraph nodes rather than auto-created
/// ghost nodes.
///
/// The index stores a single qualified map: `"crate_name::TypeName"` → `node_id`.
/// This supports two resolution modes:
/// - **Qualified lookup** (`"crate::Name"` in the TypeRef): exact map lookup.
/// - **Bare-name lookup** (no `::` in the TypeRef): self-crate scoped — resolves
///   `current_crate::name`. Bare names in the catalogue schema represent self-crate
///   types; no cross-crate fallback is performed (avoids silently wiring generic
///   params like `T` or `Self` to a coincidentally-named type in another crate).
pub(crate) struct NodeIndex {
    /// `"crate_name::TypeName"` → `node_id`.
    qualified: BTreeMap<String, String>,
}

impl NodeIndex {
    pub(crate) fn new() -> Self {
        Self { qualified: BTreeMap::new() }
    }

    /// Insert a type entry into the index.
    pub(crate) fn insert(&mut self, crate_name: &str, bare_name: &str, node_id: String) {
        let qualified_key = format!("{crate_name}::{bare_name}");
        self.qualified.insert(qualified_key, node_id);
    }

    /// Look up a `TypeRef` string and return the matching node_id, if resolvable.
    ///
    /// `current_crate` is the crate name of the catalogue document that owns the
    /// entry being emitted. It is used to scope bare-name lookups: bare `TypeRef`
    /// strings denote self-crate types, so resolution is restricted to the
    /// current-crate's index entries.
    ///
    /// Resolution:
    /// 1. Strip generic suffix (`"Foo<T>"` → `"Foo"`). If stripping yields an empty
    ///    string (e.g. `"<T as Trait>::Assoc"`), skip index lookup — these complex
    ///    forms are never catalogue entries and would produce malformed ids.
    /// 2. Normalize Rust-keyword path prefixes (`crate::`, `self::`, `super::`) by
    ///    taking the last `::` segment. This handles catalogue TypeRefs written as
    ///    `"crate::Foo"` or `"crate::module::Foo"`, treating them as self-crate bare
    ///    names (`"Foo"`).
    /// 3. If the normalised ref has `::`, try qualified lookup (`"crate_name::Foo"`)
    ///    in `qualified`. Returns `None` if not found (workspace-external path).
    /// 4. For bare names, look up `current_crate::stripped` in `qualified`. Returns
    ///    `None` if not found — bare names in the catalogue schema represent self-crate
    ///    types; no cross-crate fallback is performed (avoids silently wiring generic
    ///    params like `T` or `Self` to a coincidentally-named type in another crate).
    pub(crate) fn resolve(&self, type_ref_str: &str, current_crate: &str) -> Option<&str> {
        let stripped = strip_generics(type_ref_str);
        // Guard: complex refs that strip to empty are not catalogue entries.
        if stripped.is_empty() {
            return None;
        }
        // Normalize Rust-keyword path prefixes (crate::, self::, super::) to bare name.
        // e.g. "crate::module::Foo" → "Foo", "self::Bar" → "Bar".
        let normalised = if stripped.starts_with("crate::")
            || stripped.starts_with("self::")
            || stripped.starts_with("super::")
        {
            stripped.rsplit("::").next().unwrap_or(stripped)
        } else {
            stripped
        };
        if normalised.is_empty() {
            return None;
        }
        if normalised.contains("::") {
            // Qualified path: try exact lookup first (e.g. "domain_core::UserId" — 2 segments).
            if let Some(node_id) = self.qualified.get(normalised) {
                return Some(node_id.as_str());
            }
            // Fallback for module-qualified paths (3+ segments, e.g. "domain::module::TypeName"):
            // extract crate (first segment) + type name (last segment) and try "crate::TypeName".
            // This covers TypeRefs written as fully module-qualified paths where the index key
            // stores only "crate::TypeName" (the catalogue key is bare name, not module-path).
            let mut segments = normalised.splitn(2, "::");
            if let (Some(crate_seg), Some(rest)) = (segments.next(), segments.next()) {
                let type_name = rest.rsplit("::").next().unwrap_or(rest);
                let fallback_key = format!("{crate_seg}::{type_name}");
                return self.qualified.get(fallback_key.as_str()).map(|s| s.as_str());
            }
            return None;
        }
        // Bare name: self-crate only (no cross-crate fallback).
        let current_crate_key = format!("{current_crate}::{normalised}");
        self.qualified.get(&current_crate_key).map(|s| s.as_str())
    }
}

// ---------------------------------------------------------------------------
// Strip generic arguments
// ---------------------------------------------------------------------------

/// Strip generic arguments from a type/trait name string.
///
/// `"SomeTrait<Foo, Bar>"` → `"SomeTrait"`.
/// `"MyType"` → `"MyType"` (unchanged).
pub(crate) fn strip_generics(name: &str) -> &str {
    name.split_once('<').map_or(name, |(head, _)| head)
}

// ---------------------------------------------------------------------------
// Index builders
// ---------------------------------------------------------------------------

/// Build a global trait index from all catalogues (Decision O-2/O-3).
///
/// Returns `BTreeMap<(crate_name_str, trait_name_str), rep_node_id_str>` where
/// `rep_node_id_str` is the **representative node** id inside the trait subgraph
/// (i.e. the `__self` node, not the subgraph container id).  Edges must target
/// representative nodes, never subgraph ids, to avoid Dagre/ELK cluster-boundary
/// layout breakage.
///
/// Entries with `action: Delete` are excluded — deleted items must not appear
/// as edge targets or in the rendered contract-map output.
pub(crate) fn build_trait_index(
    catalogues: &[CatalogueDocument],
) -> BTreeMap<(String, String), String> {
    use domain::tddd::catalogue_v2::roles::ItemAction;

    let mut index: BTreeMap<(String, String), String> = BTreeMap::new();
    for doc in catalogues {
        let layer = doc.layer.as_ref();
        let crate_name = doc.crate_name.as_str();
        for (trait_name, trait_entry) in &doc.traits {
            // Skip Delete-action entries — they must not appear in the rendered map.
            if trait_entry.action == ItemAction::Delete {
                continue;
            }
            // Store the representative node id (not the subgraph container id) so that
            // trait_impl edges target a real node rather than a subgraph.
            let rep_node_id = trait_rep_node_id(layer, crate_name, trait_name.as_str());
            index.insert((crate_name.to_string(), trait_name.as_str().to_string()), rep_node_id);
        }
    }
    index
}

/// Build a global node index from all catalogues for TypeRef resolution.
///
/// Populates `NodeIndex` covering **`TypeEntry` only** (not `TraitEntry`), keyed
/// both by qualified `"crate_name::Name"` and by bare `"Name"`. This index is
/// used to resolve field/param/return/variant TypeRef targets to their actual
/// rendered mermaid node IDs (Decision D-2).
///
/// The stored node id is the **representative node** id (the `__self` node inside
/// the entry subgraph), not the subgraph container id.  Edges must target
/// representative nodes, never subgraph ids, to avoid Dagre/ELK cluster-boundary
/// layout breakage.
///
/// `TraitEntry` names are deliberately excluded: trait_impl target resolution uses
/// a separate `build_trait_index` + `resolve_trait_subgraph` path. Mixing type and
/// trait names in the same index would cause a TypeRef that matches only a trait to
/// incorrectly link to a trait subgraph, and a name shared by a type and a trait to
/// become ambiguous and fall back to a ghost node.
///
/// Entries with `action: Delete` are excluded — deleted types must not appear as
/// edge target nodes in the rendered contract-map output.
pub(crate) fn build_node_index(catalogues: &[CatalogueDocument]) -> NodeIndex {
    use domain::tddd::catalogue_v2::roles::ItemAction;

    let mut index = NodeIndex::new();
    for doc in catalogues {
        let layer = doc.layer.as_ref();
        let crate_name = doc.crate_name.as_str();
        for (type_name, type_entry) in &doc.types {
            // Skip Delete-action entries — they must not appear in the rendered map.
            if type_entry.action == ItemAction::Delete {
                continue;
            }
            // Store the representative node id (not the subgraph container id) so that
            // all resolved edges target a real node rather than a subgraph.
            let rep_node_id = type_rep_node_id(layer, crate_name, type_name.as_str());
            index.insert(crate_name, type_name.as_str(), rep_node_id);
        }
    }
    index
}
