//! Enum variant node rendering and TypeAlias undirected edge rendering (T007).
//!
//! Implements:
//! - Decision H-3 / AC-04: `TypeKindV2::Enum.variants` → variant nodes inside
//!   the entry subgraph with payload edges.
//! - Decision N-1' / AC-09: `TypeKindV2::TypeAlias.target` → undirected
//!   `---|alias_of|` edge from the alias entry subgraph to the target type subgraph.

use domain::tddd::ContractMapRendererError;
use domain::tddd::catalogue_v2::identifiers::CrateName;
use domain::tddd::catalogue_v2::variants::{VariantDecl, VariantPayload};

use super::super::StyleConfig;
use super::builder::MermaidBuilder;
use super::type_index::TypeIndex;

// ---------------------------------------------------------------------------
// Enum variant nodes (Decision H-3, AC-04)
// ---------------------------------------------------------------------------

/// Emits variant nodes inside the current (already-open) entry subgraph and
/// collects payload edges.
///
/// Each variant is rendered as a stadium-shaped node (`([variant_name])`).
/// Node class is `[node.Variant].class` from the style config.
/// Payload edges are:
/// - `VariantPayload::Tuple(fields)`: one unlabelled `--o` edge per TypeRef.
/// - `VariantPayload::Struct(fields)`: one `--o|field_name|` edge per FieldDecl.
/// - `VariantPayload::Unit`: no edges.
///
/// Variant node id: `<entry_id>_v_<index>` (index = declaration order).
///
/// `caller_crate` scopes TypeRef resolution to the same catalogue document.
///
/// # Errors
///
/// Returns `ContractMapRendererError::RenderFailed` when a variant payload `TypeRef`
/// has mismatched angle brackets (fail-closed, CN-03).
pub(super) fn emit_enum_variant_nodes(
    builder: &mut MermaidBuilder,
    entry_id: &str,
    variants: &[VariantDecl],
    caller_crate: &CrateName,
    type_index: &TypeIndex,
    style: &StyleConfig,
) -> Result<(), ContractMapRendererError> {
    let variant_shape =
        style.node.get("Variant").map(|n| n.shape.as_str()).unwrap_or("stadium").to_owned();

    let variant_payload_arrow =
        style.edge.get("variant_payload").map(|e| e.arrow.as_str()).unwrap_or("--o").to_owned();

    let variant_class =
        style.node.get("Variant").map(|n| n.class.as_str()).unwrap_or("variantNode").to_owned();

    for (i, variant) in variants.iter().enumerate() {
        let variant_id = format!("{entry_id}_v_{i}");
        builder.push_method_node(&variant_id, variant.name.as_str(), &variant_shape);

        // Collect payload edges depending on variant kind.
        emit_variant_payload_edges(
            builder,
            &variant_id,
            &variant.payload,
            caller_crate,
            type_index,
            &variant_payload_arrow,
        )?;
    }

    // Emit class attach for each variant node (after subgraph declarations).
    for i in 0..variants.len() {
        let variant_id = format!("{entry_id}_v_{i}");
        builder.push_class(&variant_id, &variant_class);
    }
    Ok(())
}

/// Emits payload edges for one variant.
///
/// # Errors
///
/// Returns `ContractMapRendererError::RenderFailed` when a `TypeRef` has mismatched
/// angle brackets (fail-closed, CN-03).
fn emit_variant_payload_edges(
    builder: &mut MermaidBuilder,
    variant_id: &str,
    payload: &VariantPayload,
    caller_crate: &CrateName,
    type_index: &TypeIndex,
    arrow: &str,
) -> Result<(), ContractMapRendererError> {
    match payload {
        VariantPayload::Unit => {
            // No edges for unit variants (AC-04).
        }
        VariantPayload::Tuple(fields) => {
            // One unlabelled edge per positional TypeRef.
            for field_ty in fields {
                if let Some(target_id) = type_index.resolve(field_ty, caller_crate)? {
                    builder.push_edge(format!("{variant_id} {arrow} {target_id}"));
                }
            }
        }
        VariantPayload::Struct(fields) => {
            // One labelled edge per named field.
            for field in fields {
                if let Some(target_id) = type_index.resolve(&field.ty, caller_crate)? {
                    builder.push_edge(format!(
                        "{variant_id} {arrow}|{}| {target_id}",
                        field.name.as_str()
                    ));
                }
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// TypeAlias undirected edge (Decision N-1', AC-09)
// ---------------------------------------------------------------------------

/// Emits the undirected `---|alias_of|` edge from the alias entry subgraph to the
/// target type subgraph.
///
/// The alias entry itself is already opened as an empty subgraph (no methods,
/// no fields) by the caller. This function emits only the connecting edge.
///
/// Edge style is `[edge.alias].arrow` (default `"---"`) with label `"alias_of"`
/// (from `[edge.alias].label`, default `"alias_of"`).
///
/// `caller_crate` scopes TypeRef resolution to the same catalogue document.
///
/// # Errors
///
/// Returns `ContractMapRendererError::RenderFailed` when the alias `TypeRef` has
/// mismatched angle brackets (fail-closed, CN-03).
pub(super) fn emit_type_alias_edge(
    builder: &mut MermaidBuilder,
    alias_id: &str,
    target: &domain::tddd::catalogue_v2::identifiers::TypeRef,
    caller_crate: &CrateName,
    type_index: &TypeIndex,
    style: &StyleConfig,
) -> Result<(), ContractMapRendererError> {
    let arrow = style.edge.get("alias").map(|e| e.arrow.as_str()).unwrap_or("---");
    let label = style.edge.get("alias").and_then(|e| e.label.as_deref()).unwrap_or("alias_of");

    if let Some(target_id) = type_index.resolve(target, caller_crate)? {
        builder.push_edge(format!("{alias_id} {arrow}|{label}| {target_id}"));
    }
    Ok(())
}
