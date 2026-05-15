//! Cross-catalogue trait impl edge resolution for the T008 renderer.
//!
//! Implements Decision O-2 + O-3 + O-a:
//! - Build a global `BTreeMap<(crate_name_str, trait_name_str), trait_subgraph_id>` index
//!   at render start (one per `render_mermaid()` call — not long-lived, Decision O-a).
//! - Emit `-.->|impl|` edges from TypeEntry concrete types to their workspace-internal
//!   trait targets (Decision O-2 + O-3).
//! - Silently skip workspace-external traits (std, core, serde, etc.) — lookup miss
//!   produces no edge (Decision J-2 + CN-08).
//!
//! `trait_impls` exists only on `TypeEntry`. `TraitEntry` has no `trait_impls` field.

use std::collections::BTreeMap;

use domain::tddd::LayerId;
use domain::tddd::catalogue_v2::entries::TypeEntry;
use domain::tddd::catalogue_v2::identifiers::CrateName;
use domain::tddd::catalogue_v2::roles::ItemAction;
use domain::tddd::catalogue_v2::{CatalogueDocument, TraitName};

use super::super::{StyleConfig, trait_node_id};
use super::builder::MermaidBuilder;

// ---------------------------------------------------------------------------
// TraitIndex — cross-catalogue trait_name → node_id resolution (T008)
// ---------------------------------------------------------------------------

/// Maps `(crate_name_str, trait_name_str)` to the mermaid subgraph id of the
/// corresponding `TraitEntry` across all catalogues.
///
/// Built once per `render_mermaid()` call from the full catalogue slice.
/// Not long-lived — avoids stale state between render calls (Decision O-a).
///
/// Lookup miss (workspace-external trait) silently produces no edge (Decision J-2).
pub(super) struct TraitIndex {
    /// Maps `(crate_name_str, trait_name_str)` → trait subgraph id.
    traits: BTreeMap<(String, String), String>,
}

impl TraitIndex {
    /// Builds a `TraitIndex` from a slice of `CatalogueDocument` references.
    ///
    /// Accepts `&[&CatalogueDocument]` so that callers can pass a pre-filtered
    /// reference slice (e.g. catalogues from rendered layers only) without an extra
    /// allocation. For each document, iterates over `doc.traits` and inserts a
    /// `(doc.crate_name, trait_name)` → `trait_node_id(...)` mapping.
    ///
    /// Entries with `action: Delete` are excluded from the index so that a live type's
    /// `trait_impls` declaration referencing a deleted trait resolves to `None` (no
    /// edge) rather than a dangling `-.->|impl|` edge pointing to an absent subgraph.
    ///
    /// `render_mermaid` restricts the input to the rendered-layer subset so that
    /// trait_impl edges never point to trait nodes absent from the rendered output
    /// (dangling edge prevention, Decision O-a, CN-08).
    pub(super) fn build(catalogues: &[&CatalogueDocument]) -> Self {
        let mut traits = BTreeMap::new();
        for doc in catalogues {
            let layer: &LayerId = &doc.layer;
            let crate_name: &CrateName = &doc.crate_name;
            let crate_key = crate_name.as_str().to_owned();
            for (trait_name, entry) in &doc.traits {
                // Skip deletion records — deleted traits must not resolve to a node id
                // because they are not rendered in the contract map.
                if entry.action == ItemAction::Delete {
                    continue;
                }
                let id = trait_node_id(layer, crate_name, trait_name);
                traits.insert((crate_key.clone(), trait_name.as_str().to_owned()), id);
            }
        }
        Self { traits }
    }

    /// Resolves a `(origin_crate, trait_name)` pair to the mermaid subgraph id.
    ///
    /// Returns `None` when the pair is not found (workspace-external trait).
    /// Callers silently skip the edge on `None` (Decision J-2 + CN-08).
    pub(super) fn resolve(&self, origin_crate: &CrateName, trait_name: &TraitName) -> Option<&str> {
        let key = (origin_crate.as_str().to_owned(), trait_name.as_str().to_owned());
        self.traits.get(&key).map(|s| s.as_str())
    }
}

// ---------------------------------------------------------------------------
// Trait impl edge emission
// ---------------------------------------------------------------------------

/// Emits `-.->|impl|` edges from a `TypeEntry`'s concrete type subgraph to
/// the workspace-internal trait subgraphs it declares implementation for.
///
/// For each `TraitImplDeclV2` in `type_entry.trait_impls`:
/// - Looks up `(impl.origin_crate, impl.trait_name)` in `trait_index`.
/// - If found: emits `<type_node_id> -.->|impl| <trait_node_id>` to the edge buffer.
/// - If not found: silently skips (workspace-external trait — Decision J-2 + CN-08).
///
/// Arrow and label come from `[edge.trait_impl]` in the style config.
/// Defaults: `arrow = "-.->"`, `label = "impl"`.
pub(super) fn emit_trait_impl_edges(
    builder: &mut MermaidBuilder,
    type_id: &str,
    type_entry: &TypeEntry,
    trait_index: &TraitIndex,
    style: &StyleConfig,
) {
    let arrow = style.edge.get("trait_impl").map(|e| e.arrow.as_str()).unwrap_or("-.->").to_owned();
    let label =
        style.edge.get("trait_impl").and_then(|e| e.label.as_deref()).unwrap_or("impl").to_owned();

    for impl_decl in &type_entry.trait_impls {
        if let Some(trait_id) = trait_index.resolve(&impl_decl.origin_crate, &impl_decl.trait_name)
        {
            builder.push_edge(format!("{type_id} {arrow}|{label}| {trait_id}"));
        }
        // Workspace-external trait → lookup miss → silently skip (Decision J-2 + CN-08).
    }
}
