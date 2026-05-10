//! Contract Map renderer — OS-06 stub.
//!
//! The full v3 rendering pipeline is OS-06 deferred (tracked as T012).
//! V3 `CatalogueDocument` entries use `DataRole`, `ContractRole`, and
//! `FunctionRole` which require ADR-level decisions on visualization strategy
//! (node shapes, edge semantics, trait_impls, DataRole vs ContractRole
//! clustering). Until those decisions are made, the renderer emits an
//! OS-06 deferment notice with empty layer subgraphs so that the
//! pre-commit pipeline can complete without blocking on v2 codec failures.
//!
//! T008: `catalogue_bulk_loader` converts v3 catalogues to stub
//! `TypeCatalogueDocument` entries (preserving entry names + role-mapped
//! shapes) so the renderer signature is unchanged. The full v3 pipeline
//! (edges, node shapes from `DataRole`/`ContractRole`) is tracked as T012.
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
use crate::tddd::catalogue::TypeCatalogueDocument;
use crate::tddd::contract_map_content::ContractMapContent;
use crate::tddd::contract_map_options::ContractMapRenderOptions;

/// Render the contract map for the given per-layer catalogues.
///
/// **OS-06 stub**: the full v3 rendering pipeline is deferred (T012).
/// This function emits:
/// 1. A generated-file marker (CN-08) with an OS-06 deferment comment.
/// 2. A `flowchart LR` block containing one empty subgraph per layer
///    (in the order given by `layer_order`, filtered by `opts.layers`).
/// 3. A comment per subgraph showing how many entries the loader stub
///    produced (for observability).
///
/// No edges are emitted. The full v3 rendering pipeline (node shapes,
/// edges from `DataRole`/`ContractRole`/`FunctionRole`, `trait_impls`)
/// is tracked under T012 and requires ADR-level design decisions.
///
/// `opts` is accepted for API stability; `opts.signal_overlay` and
/// `opts.action_overlay` are intentionally ignored by the stub.
#[must_use]
pub fn render_contract_map(
    catalogues: &BTreeMap<LayerId, TypeCatalogueDocument>,
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
        "<!-- OS-06 DEFERRED: v3 rendering pipeline requires ADR-level design decisions. \
         Full implementation tracked as T012. -->\n",
    );
    out.push_str("```mermaid\n");
    out.push_str("flowchart LR\n");
    out.push_str("    %% contract-map renderer is OS-06 deferred (track tddd-v2-2026-05-08).\n");
    out.push_str(
        "    %% v3 schema redesign requires ADR-level design decisions \
         and is tracked separately as T012.\n",
    );
    out.push_str("    %% Until then, this renderer outputs only the layer scaffold.\n");

    for layer in &active_layers {
        let label = sanitize_id(layer.as_ref());
        let raw = layer.as_ref();
        let _ = writeln!(out, "    subgraph {label} [{raw}]");
        // Emit entry count as a comment for observability.
        if let Some(doc) = catalogues.get(layer) {
            let count = doc.entries().len();
            if count > 0 {
                let _ = writeln!(out, "        %% {count} entries (nodes deferred to T012)");
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
