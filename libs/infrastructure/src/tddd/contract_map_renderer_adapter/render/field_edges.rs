//! Field edge, method edge, and entry label helpers extracted from `render.rs`.
//!
//! Implements:
//! - Decision K-2+(d) / K-2: PlainStruct / TupleStruct field edges.
//! - Decision N-1' / AC-09: TypeAlias undirected `---|alias_of|` edge (delegated to
//!   `enum_variants::emit_type_alias_edge`).
//! - Method param (`--o`) and returns (`-->`) edge helpers used by
//!   `render.rs` for both TraitEntry methods and FunctionEntry edges.
//! - `entry_label`: entry subgraph label from sub-module path + short name.

use domain::tddd::ContractMapRendererError;
use domain::tddd::catalogue_v2::composite::TypeKindV2;
use domain::tddd::catalogue_v2::identifiers::{CrateName, ModulePath, TypeRef};

use super::super::{EdgeStyle, StyleConfig};
use super::builder::MermaidBuilder;
use super::enum_variants::emit_type_alias_edge;
use super::type_index::TypeIndex;

// ---------------------------------------------------------------------------
// Field edges (Decision K-2+(d) + Decision K-2 for TupleStruct)
// ---------------------------------------------------------------------------

/// Emits field edges from a TypeEntry subgraph to field type subgraphs, and
/// for `TypeAlias` emits the undirected `---|alias_of|` edge (T007, Decision N-1').
///
/// - `PlainStruct { has_stripped_fields: false, fields }`: one `--o|field_name|` edge per field.
/// - `PlainStruct { has_stripped_fields: true }`: no edges emitted (AC-08).
/// - `TupleStruct { has_stripped_fields: false, fields }`: `--o|.0|`, `--o|.1|` etc.
/// - `TupleStruct { has_stripped_fields: true }`: no edges emitted (AC-08).
/// - `UnitStruct`: no edges.
/// - `Enum`: variant payload edges are emitted by `emit_enum_variant_nodes` (T007); no
///   additional edges here.
/// - `TypeAlias { target }`: emits `---|alias_of|` undirected edge to target type (T007,
///   Decision N-1', AC-09).
///
/// `caller_crate` scopes TypeRef resolution to the same catalogue document (same-catalogue).
///
/// # Errors
///
/// Returns `ContractMapRendererError::RenderFailed` when a `TypeRef` has mismatched angle
/// brackets (fail-closed, CN-03).
pub(super) fn emit_field_edges(
    builder: &mut MermaidBuilder,
    entry_id: &str,
    kind: &TypeKindV2,
    caller_crate: &CrateName,
    type_index: &TypeIndex,
    style: &StyleConfig,
) -> Result<(), ContractMapRendererError> {
    let arrow = style.edge.get("field").map(|e| e.arrow.as_str()).unwrap_or("--o");

    match kind {
        TypeKindV2::PlainStruct { fields, has_stripped_fields: false, .. } => {
            for field in fields {
                if let Some(target_id) = type_index.resolve(&field.ty, caller_crate)? {
                    builder.push_edge(format!(
                        "{entry_id} {arrow}|{}| {target_id}",
                        field.name.as_str()
                    ));
                }
            }
        }
        TypeKindV2::TupleStruct { fields, has_stripped_fields: false } => {
            for (i, field_ty) in fields.iter().enumerate() {
                if let Some(target_id) = type_index.resolve(field_ty, caller_crate)? {
                    builder.push_edge(format!("{entry_id} {arrow}|.{i}| {target_id}"));
                }
            }
        }
        TypeKindV2::TypeAlias { target } => {
            // Undirected alias edge: alias entry subgraph --- alias_of --- target subgraph
            // (Decision N-1', AC-09). Delegated to enum_variants module.
            emit_type_alias_edge(builder, entry_id, target, caller_crate, type_index, style)?;
        }
        // has_stripped_fields: true → skip all field edges (AC-08).
        TypeKindV2::PlainStruct { has_stripped_fields: true, .. }
        | TypeKindV2::TupleStruct { has_stripped_fields: true, .. }
        // UnitStruct: no field edges.
        | TypeKindV2::UnitStruct
        // Enum: variant payload edges handled by emit_enum_variant_nodes (T007).
        | TypeKindV2::Enum { .. } => {}
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Method edge emission helpers
// ---------------------------------------------------------------------------

/// Emits a `--o` edge from `src_id` to the resolved param type subgraph.
///
/// The edge style is read from `style.edge["method_param"].arrow`. If the
/// TypeRef is unresolvable (not in the same catalogue), the edge is silently skipped.
/// `caller_crate` scopes TypeRef resolution to the same catalogue document.
///
/// # Errors
///
/// Returns `ContractMapRendererError::RenderFailed` when the `TypeRef` has mismatched
/// angle brackets (fail-closed, CN-03).
pub(super) fn collect_param_edge(
    builder: &mut MermaidBuilder,
    src_id: &str,
    param_ty: &TypeRef,
    caller_crate: &CrateName,
    type_index: &TypeIndex,
    style: &StyleConfig,
) -> Result<(), ContractMapRendererError> {
    if let Some(target_id) = type_index.resolve(param_ty, caller_crate)? {
        let arrow =
            style.edge.get("method_param").map(|e: &EdgeStyle| e.arrow.as_str()).unwrap_or("--o");
        builder.push_edge(format!("{src_id} {arrow} {target_id}"));
    }
    Ok(())
}

/// Emits a `-->` edge from `src_id` to the resolved return type subgraph.
///
/// The edge style is read from `style.edge["method_returns"].arrow`. If the
/// TypeRef is unresolvable (not in the same catalogue), the edge is silently skipped.
/// `caller_crate` scopes TypeRef resolution to the same catalogue document.
///
/// # Errors
///
/// Returns `ContractMapRendererError::RenderFailed` when the `TypeRef` has mismatched
/// angle brackets (fail-closed, CN-03).
pub(super) fn collect_returns_edge(
    builder: &mut MermaidBuilder,
    src_id: &str,
    returns_ty: &TypeRef,
    caller_crate: &CrateName,
    type_index: &TypeIndex,
    style: &StyleConfig,
) -> Result<(), ContractMapRendererError> {
    if let Some(target_id) = type_index.resolve(returns_ty, caller_crate)? {
        let arrow =
            style.edge.get("method_returns").map(|e: &EdgeStyle| e.arrow.as_str()).unwrap_or("-->");
        builder.push_edge(format!("{src_id} {arrow} {target_id}"));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Label helpers
// ---------------------------------------------------------------------------

/// Builds the entry subgraph label from sub-module path and short name.
///
/// When `module_path` is root (empty), returns just `name`.
/// When `module_path` has segments, returns `seg1::seg2::...::name`
/// (sub-module path + name, per Decision U-6d-iii label spec).
///
/// Example: `module_path = ["team", "manager"]`, `name = "TeamManager"` →
/// `"team::manager::TeamManager"`.
pub(super) fn entry_label(module_path: &ModulePath, name: &str) -> String {
    if module_path.is_root() { name.to_owned() } else { format!("{}::{name}", module_path) }
}
