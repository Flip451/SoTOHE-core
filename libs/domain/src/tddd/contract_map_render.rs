//! Contract Map renderer — IN-24 minimal placeholder.
//!
//! The detailed v3 contract-map rendering pipeline is deferred (OS-07 / IN-24).
//! V3 `CatalogueDocument` entries use `DataRole`, `ContractRole`, and
//! `FunctionRole`, which require ADR-level decisions on visualization strategy
//! (node shapes, edge semantics, trait_impls, DataRole vs ContractRole
//! clustering). Until those decisions are made, the renderer emits a minimal
//! placeholder listing entry names per layer, referencing IN-24 / OS-07 as
//! the deferral spec items.
//!
//! This renderer is **v3-native**: it reads `CatalogueDocument` directly and
//! does not go through any v2 conversion. Non-v3 catalogues are rejected at
//! the `CatalogueDocumentCodec::decode` level (CN-11 — fail-closed).
//!
//! Placement rationale: the function is I/O-free and is called directly
//! from the usecase interactor. Per ADR 2026-04-17-1528 §D1 this belongs
//! in the domain layer — rendering the catalogue is a pure transformation,
//! not an infrastructure concern.
//!
//! Layer-agnostic invariant (ADR §4.5): the renderer never hard-codes
//! layer names. Every subgraph label comes from the keys of `catalogues`,
//! ordered by `layer_order`.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use crate::tddd::LayerId;
use crate::tddd::catalogue_v2::CatalogueDocument;
use crate::tddd::contract_map_content::ContractMapContent;
use crate::tddd::contract_map_options::ContractMapRenderOptions;

/// Render the contract map for the given per-layer catalogues.
///
/// **IN-24 minimal placeholder**: the detailed v3 rendering pipeline is
/// deferred (OS-07 / IN-24). This function emits:
/// 1. A generated-file marker (CN-08) with an IN-24 / OS-07 deferral note.
/// 2. A `flowchart LR` block containing one `subgraph` per layer (in the
///    order given by `layer_order`, filtered by `opts.layers`), each listing
///    the layer's entry names as comment nodes for observability. No node
///    shapes, no edges, no v3→v2 conversion.
///
/// The full v3 rendering pipeline (node shapes from `DataRole` /
/// `ContractRole` / `FunctionRole`, edges, `trait_impls` clustering) requires
/// ADR-level design decisions and is tracked under OS-07 as a follow-up.
///
/// `opts` is accepted for API stability; `opts.signal_overlay` and
/// `opts.action_overlay` are intentionally ignored by the placeholder.
///
/// # Errors
///
/// This function is infallible; it always returns a `ContractMapContent`.
#[must_use]
pub fn render_contract_map(
    catalogues: &BTreeMap<LayerId, CatalogueDocument>,
    layer_order: &[LayerId],
    opts: &ContractMapRenderOptions,
) -> ContractMapContent {
    // Layer filter — preserve topological order from `layer_order`.
    let active_layers: Vec<&LayerId> = if opts.layers.is_empty() {
        layer_order.iter().collect()
    } else {
        layer_order.iter().filter(|l| opts.layers.contains(l)).collect()
    };

    let mut out = String::new();
    out.push_str("<!-- Generated contract-map-renderer — DO NOT EDIT DIRECTLY -->\n");
    out.push_str(
        "<!-- IN-24 / OS-07 DEFERRED: detailed v3 contract-map rendering requires \
         ADR-level design decisions (node shapes, edges, role clustering). \
         This placeholder lists entry names per layer only. -->\n",
    );
    out.push_str("```mermaid\n");
    out.push_str("flowchart LR\n");
    out.push_str(
        "    %% contract-map renderer: IN-24 minimal placeholder \
         (detailed v3 rendering deferred to follow-up ADR/track per OS-07).\n",
    );
    out.push_str(
        "    %% Each layer block lists entry names for observability. \
         No node shapes or edges are emitted.\n",
    );

    for layer in &active_layers {
        let label = sanitize_id(layer.as_ref());
        let raw = layer.as_ref();
        let _ = writeln!(out, "    subgraph {label} [{raw}]");

        if let Some(doc) = catalogues.get(layer) {
            // Emit type entry names.
            for type_name in doc.types.keys() {
                let _ = writeln!(out, "        %% type: {}", type_name.as_str());
            }
            // Emit trait entry names.
            for trait_name in doc.traits.keys() {
                let _ = writeln!(out, "        %% trait: {}", trait_name.as_str());
            }
            // Emit function entry paths.
            for fn_path in doc.functions.keys() {
                let _ = writeln!(out, "        %% fn: {fn_path}");
            }
        }

        out.push_str("    end\n");
    }

    out.push_str("```\n");
    ContractMapContent::new(out)
}

/// Rewrite an arbitrary string into a mermaid-safe identifier.
///
/// ASCII alphanumerics pass through verbatim. `_` is escaped as `__`.
/// Any other code point is escaped as `_<hex>_`. Empty input maps to `_`.
fn sanitize_id(raw: &str) -> String {
    if raw.is_empty() {
        return "_".to_owned();
    }
    let mut out = String::with_capacity(raw.len());
    for ch in raw.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch);
        } else if ch == '_' {
            out.push_str("__");
        } else {
            let _ = write!(out, "_{:x}_", ch as u32);
        }
    }
    out
}

#[cfg(test)]
#[path = "contract_map_render_tests.rs"]
mod tests;
