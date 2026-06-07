//! Mermaid rendering internals for the contract-map renderer (T004–T009).
//!
//! All items in this module are `pub(super)` — they are implementation details
//! of `ContractMapRendererAdapter` and must not appear in the infrastructure
//! crate's public API (Decision P-3 / CN-11).

mod emit;
mod node_index;
mod style_config;
mod type_ref;

use std::collections::BTreeMap;

use domain::tddd::catalogue_v2::CatalogueDocument;
use domain::tddd::{ContractMapRendererError, LayerId};

// Re-export sub-module items so that emit.rs (which imports via `super::`) and
// render_mermaid (defined here) can continue to reach them without path changes.
pub(crate) use node_index::{NodeIndex, build_node_index, build_trait_index};
pub(crate) use style_config::{
    StyleConfig, apply_shape, class_def_line, edge_arrow_label, edge_line,
};
pub(crate) use type_ref::{resolve_trait_subgraph, resolve_type_ref_node_ids};

use emit::{EntryKind, emit_entry};

// ---------------------------------------------------------------------------
// Rendering helpers — node ID generators
// ---------------------------------------------------------------------------

/// Sanitize a string for use as a mermaid node_id segment.
/// Replaces every character that is not ASCII alphanumeric or underscore with `_`.
pub(super) fn sanitize(s: &str) -> String {
    s.chars().map(|c| if c.is_ascii_alphanumeric() || c == '_' { c } else { '_' }).collect()
}

/// Generate a subgraph id for a Type entry (Decision D-2).
///
/// Format: `T<len>_<sanitized_layer>_<sanitized_crate>_<sanitized_name>`
/// where `<len>` is the length of `<sanitized_layer>_<sanitized_crate>_<sanitized_name>`.
///
/// This id is the **container** subgraph id only.  Edge endpoints must use
/// [`type_rep_node_id`] (the representative node inside the subgraph) so that
/// no edge points at a subgraph id, which breaks Dagre/ELK cluster-boundary
/// layout.
pub(super) fn type_node_id(layer: &str, crate_name: &str, type_name: &str) -> String {
    let sl = sanitize(layer);
    let sc = sanitize(crate_name);
    let sn = sanitize(type_name);
    let body = format!("{sl}_{sc}_{sn}");
    format!("T{}_{}", body.len(), body)
}

/// Generate the representative node id for a Type entry.
///
/// The representative node is emitted **inside** the entry subgraph and acts
/// as the sole valid edge target for the type.  Its id is the subgraph id
/// (from [`type_node_id`]) with an `__self` suffix appended, ensuring the
/// two ids are always distinct and collision-free.
pub(super) fn type_rep_node_id(layer: &str, crate_name: &str, type_name: &str) -> String {
    format!("{}__self", type_node_id(layer, crate_name, type_name))
}

/// Generate a subgraph id for a Trait entry (Decision D-2).
///
/// Format: `R<len>_<sanitized_layer>_<sanitized_crate>_<sanitized_name>`
///
/// This id is the **container** subgraph id only.  Edge endpoints must use
/// [`trait_rep_node_id`] (the representative node inside the subgraph).
pub(super) fn trait_node_id(layer: &str, crate_name: &str, trait_name: &str) -> String {
    let sl = sanitize(layer);
    let sc = sanitize(crate_name);
    let sn = sanitize(trait_name);
    let body = format!("{sl}_{sc}_{sn}");
    format!("R{}_{}", body.len(), body)
}

/// Generate the representative node id for a Trait entry.
///
/// Appends `__self` to the subgraph id from [`trait_node_id`].
pub(super) fn trait_rep_node_id(layer: &str, crate_name: &str, trait_name: &str) -> String {
    format!("{}__self", trait_node_id(layer, crate_name, trait_name))
}

/// Generate a node_id for a Function entry (Decision D-2).
///
/// Format: `F<len>_<sanitized_layer>_<sanitized_crate>_<sanitized_full_path>`
pub(super) fn function_node_id(layer: &str, crate_name: &str, full_path: &str) -> String {
    let sl = sanitize(layer);
    let sc = sanitize(crate_name);
    let sp = sanitize(full_path);
    let body = format!("{sl}_{sc}_{sp}");
    format!("F{}_{}", body.len(), body)
}

/// Generate a subgraph id for a module (top-level module aggregation, U-6d-iii).
fn module_subgraph_id(layer: &str, crate_name: &str, module_first_segment: &str) -> String {
    let sl = sanitize(layer);
    let sc = sanitize(crate_name);
    let sm = sanitize(module_first_segment);
    format!("{sl}_{sc}_module_{sm}")
}

/// Generate a subgraph id for a layer.
fn layer_subgraph_id(layer: &str) -> String {
    sanitize(layer)
}

// ---------------------------------------------------------------------------
// T009: main assembly
// ---------------------------------------------------------------------------

/// Render a mermaid flowchart from a set of catalogue documents.
///
/// # Errors
///
/// Propagates any `ContractMapRendererError` that arises during rendering
/// (none in the current implementation, but the signature is kept for future extension).
pub(super) fn render_mermaid(
    catalogues: &[CatalogueDocument],
    layer_order: &[LayerId],
    style: &StyleConfig,
) -> Result<String, ContractMapRendererError> {
    // T004: build global trait index (per-render-call, CN-05).
    let trait_index = build_trait_index(catalogues);
    // T004: build global node index for TypeRef resolution (field/param/return edges).
    let node_index = build_node_index(catalogues);

    // Collect: for each layer subgraph, collect all catalogue documents belonging to it.
    // Index documents by layer id string for quick lookup.
    let mut docs_by_layer: BTreeMap<String, Vec<&CatalogueDocument>> = BTreeMap::new();
    for doc in catalogues {
        docs_by_layer.entry(doc.layer.as_ref().to_string()).or_default().push(doc);
    }

    // Output sections.
    let mut class_defs: Vec<String> = Vec::new();
    let mut subgraph_lines: Vec<String> = Vec::new();
    let mut edge_lines: Vec<String> = Vec::new();
    let mut class_attach: Vec<String> = Vec::new();

    // T009(b): classDef definitions — alphabetical from [class.*] (CN-08).
    for (class_name, class_style) in &style.class {
        class_defs.push(class_def_line(class_name, class_style));
    }

    // T009(c): layer subgraphs in layer_order (CN-01/GO-03).
    for layer_id in layer_order {
        let layer_str = layer_id.as_ref();
        let layer_sg_id = layer_subgraph_id(layer_str);

        subgraph_lines.push(format!("subgraph {layer_sg_id}[\"{layer_str}\"]"));
        subgraph_lines.push("  direction TB".to_string());

        // Sort docs within layer alphabetically by crate_name (CN-08).
        let docs_in_layer = docs_by_layer.get(layer_str).cloned().unwrap_or_default();
        let mut sorted_docs: Vec<&CatalogueDocument> = docs_in_layer;
        sorted_docs.sort_by_key(|d| d.crate_name.as_str());

        for doc in &sorted_docs {
            let crate_str = doc.crate_name.as_str();
            let layer_str_doc = doc.layer.as_ref();

            // Build inherent_impls index for this doc: type_name -> Vec<methods>
            let mut inherent_methods: BTreeMap<
                String,
                Vec<&domain::tddd::catalogue_v2::methods::MethodDeclaration>,
            > = BTreeMap::new();
            for impl_decl in &doc.inherent_impls {
                let tn = impl_decl.type_name.as_str().to_string();
                for m in &impl_decl.methods {
                    inherent_methods.entry(tn.clone()).or_default().push(m);
                }
            }

            // Separate entries into root (module_path=[]) and module-grouped.
            // Delete-action entries are skipped — the contract-map shows the resulting
            // contract, not removed items.
            let mut module_first_segs: BTreeMap<String, Vec<EntryKind<'_>>> = BTreeMap::new();
            let mut root_entries: Vec<EntryKind<'_>> = Vec::new();

            use domain::tddd::catalogue_v2::roles::ItemAction;

            // Types
            for (type_name, type_entry) in &doc.types {
                if type_entry.action == ItemAction::Delete {
                    continue; // deleted types must not appear in the rendered map
                }
                if type_entry.module_path.is_root() {
                    root_entries.push(EntryKind::Type(type_name.as_str(), type_entry));
                } else {
                    let first_seg = type_entry
                        .module_path
                        .segments()
                        .first()
                        .map(|s| s.as_str())
                        .unwrap_or("")
                        .to_string();
                    module_first_segs
                        .entry(first_seg)
                        .or_default()
                        .push(EntryKind::Type(type_name.as_str(), type_entry));
                }
            }

            // Traits
            for (trait_name, trait_entry) in &doc.traits {
                if trait_entry.action == ItemAction::Delete {
                    continue; // deleted traits must not appear in the rendered map
                }
                if trait_entry.module_path.is_root() {
                    root_entries.push(EntryKind::Trait(trait_name.as_str(), trait_entry));
                } else {
                    let first_seg = trait_entry
                        .module_path
                        .segments()
                        .first()
                        .map(|s| s.as_str())
                        .unwrap_or("")
                        .to_string();
                    module_first_segs
                        .entry(first_seg)
                        .or_default()
                        .push(EntryKind::Trait(trait_name.as_str(), trait_entry));
                }
            }

            // Functions
            for (fn_path, fn_entry) in &doc.functions {
                if fn_entry.action == ItemAction::Delete {
                    continue; // deleted functions must not appear in the rendered map
                }
                if fn_path.module_path.is_root() {
                    root_entries.push(EntryKind::Function(fn_path, fn_entry));
                } else {
                    let first_seg = fn_path
                        .module_path
                        .segments()
                        .first()
                        .map(|s| s.as_str())
                        .unwrap_or("")
                        .to_string();
                    module_first_segs
                        .entry(first_seg)
                        .or_default()
                        .push(EntryKind::Function(fn_path, fn_entry));
                }
            }

            // Emit root entries directly under the layer subgraph.
            for entry in &root_entries {
                emit_entry(
                    entry,
                    &mut subgraph_lines,
                    &mut edge_lines,
                    &mut class_attach,
                    style,
                    &inherent_methods,
                    &node_index,
                    layer_str_doc,
                    crate_str,
                )?;
            }

            // Emit module subgraphs.
            for (first_seg, entries) in &module_first_segs {
                let mod_sg_id = module_subgraph_id(layer_str_doc, crate_str, first_seg);
                let mod_label = format!("{crate_str}::{first_seg}");
                subgraph_lines.push(format!("  subgraph {mod_sg_id}[\"{mod_label}\"]"));
                subgraph_lines.push("    direction TB".to_string());

                for entry in entries {
                    emit_entry(
                        entry,
                        &mut subgraph_lines,
                        &mut edge_lines,
                        &mut class_attach,
                        style,
                        &inherent_methods,
                        &node_index,
                        layer_str_doc,
                        crate_str,
                    )?;
                }

                subgraph_lines.push("  end".to_string());
            }
        }

        subgraph_lines.push("end".to_string());

        // T008: trait impl edges for this layer's docs.
        for doc in &sorted_docs {
            let crate_str = doc.crate_name.as_str();
            for trait_impl in &doc.trait_impls {
                let for_type_str = trait_impl.for_type.as_str();
                let trait_ref_str = trait_impl.trait_ref.as_str();

                // Resolve for_type to a node_id via the global node index.
                // Workspace-internal cross-crate for_type (e.g. "domain::MyType") is
                // resolved through the index. Workspace-external types (std, external
                // crates) are not in the index and are silently skipped (O-2 / ADR line 286).
                let source_id = match node_index.resolve(for_type_str, crate_str) {
                    Some(id) => id.to_string(),
                    None => continue, // silent skip (workspace-external, OS-04)
                };

                // Resolve trait_ref to target subgraph_id (CN-10: silent skip if external).
                let target_id = match resolve_trait_subgraph(trait_ref_str, crate_str, &trait_index)
                {
                    Some(id) => id.to_string(),
                    None => continue, // silent skip (CN-10 / AC-06)
                };

                let (arrow, label) = edge_arrow_label(&style.edge, "trait_impl")?;
                edge_lines.push(edge_line(&source_id, arrow, label, &target_id));
            }
        }
    }

    // Assemble output per IN-18 / ADR Render Output structure.
    // The mermaid body (flowchart LR + content sections) is wrapped in a
    // fenced markdown block so GitHub renders it as a diagram rather than
    // plain text.  Order within the fence: classDef → layer-subgraph →
    // edge → class-attach (IN-18 unchanged).
    let mut out = String::new();
    out.push_str("<!-- Generated contract-map-renderer — DO NOT EDIT DIRECTLY -->\n");
    out.push_str("```mermaid\n");
    out.push_str("flowchart LR\n");

    for line in &class_defs {
        out.push_str(line);
        out.push('\n');
    }

    for line in &subgraph_lines {
        out.push_str(line);
        out.push('\n');
    }

    for line in &edge_lines {
        out.push_str(line);
        out.push('\n');
    }

    for line in &class_attach {
        out.push_str(line);
        out.push('\n');
    }

    out.push_str("```\n");

    Ok(out)
}
