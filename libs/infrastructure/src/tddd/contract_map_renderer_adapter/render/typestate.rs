//! Typestate transition edge rendering (T007).
//!
//! Implements Decision G-2'b / AC-03:
//! When `TypeKindV2::PlainStruct.typestate` is `Some(TypestateMarker)`, the method
//! names returned by `TypestateMarker.transitions().transition_methods()` indicate
//! transition methods. For those methods the returns edge is rendered with the
//! transition style (`==>|transitions_to|`) rather than the normal returns style
//! (`-->`).
//!
//! `param` edges remain unchanged (`--o`) regardless of typestate membership.

use domain::tddd::ContractMapRendererError;
use domain::tddd::catalogue_v2::composite::TypestateMarker;
use domain::tddd::catalogue_v2::identifiers::{CrateName, MethodName, TypeRef};
use domain::tddd::catalogue_v2::methods::MethodDeclaration;

use super::super::StyleConfig;
use super::builder::MermaidBuilder;
use super::type_index::TypeIndex;

// ---------------------------------------------------------------------------
// Typestate transition returns edge emission (Decision G-2'b, AC-03)
// ---------------------------------------------------------------------------

/// Emits a returns edge for a method, using transition style if the method is
/// listed in the typestate transition set.
///
/// If `typestate_marker` is `Some` and the method name appears in
/// `transitions.transition_methods()`, the returns edge uses the transition
/// arrow (`==>`) with label `"transitions_to"`. Otherwise the standard returns
/// arrow (`-->`) is used.
///
/// If the return TypeRef is unresolvable (not in the same catalogue), the edge
/// is silently skipped.
///
/// `caller_crate` scopes TypeRef resolution to the same catalogue document.
///
/// # Errors
///
/// Returns `ContractMapRendererError::RenderFailed` when the `TypeRef` has mismatched
/// angle brackets (fail-closed, CN-03).
#[allow(clippy::too_many_arguments)]
pub(super) fn emit_returns_edge_maybe_transition(
    builder: &mut MermaidBuilder,
    method_id: &str,
    method_name: &MethodName,
    returns_ty: &TypeRef,
    typestate_marker: Option<&TypestateMarker>,
    caller_crate: &CrateName,
    type_index: &TypeIndex,
    style: &StyleConfig,
) -> Result<(), ContractMapRendererError> {
    let Some(target_id) = type_index.resolve(returns_ty, caller_crate)? else {
        return Ok(());
    };

    if is_transition_method(method_name, typestate_marker) {
        // Use transition edge style (==>|transitions_to|).
        let arrow = style.edge.get("transition").map(|e| e.arrow.as_str()).unwrap_or("==>");
        let label = style
            .edge
            .get("transition")
            .and_then(|e| e.label.as_deref())
            .unwrap_or("transitions_to");
        builder.push_edge(format!("{method_id} {arrow}|{label}| {target_id}"));
    } else {
        // Normal returns edge (-->).
        let arrow = style.edge.get("method_returns").map(|e| e.arrow.as_str()).unwrap_or("-->");
        builder.push_edge(format!("{method_id} {arrow} {target_id}"));
    }
    Ok(())
}

/// Returns `true` if `method_name` is listed in the typestate transition set.
///
/// When `typestate_marker` is `None`, always returns `false` (no typestate → no
/// transition edges).
fn is_transition_method(
    method_name: &MethodName,
    typestate_marker: Option<&TypestateMarker>,
) -> bool {
    typestate_marker
        .map(|ts| ts.transitions().transition_methods().contains(method_name))
        .unwrap_or(false)
}

// ---------------------------------------------------------------------------
// Typestate overlay class emission
// ---------------------------------------------------------------------------

/// Emits a typestate overlay class attach for the entry subgraph when the entry
/// has a typestate marker.
///
/// The overlay class is read from `[pattern.Typestate].overlay_class` in the
/// style config. If the key is absent the overlay is silently skipped.
///
/// This is an additive attach: the entry already has its role class attached by
/// the caller. The overlay is a second `class <id> <overlay>` line (T007).
pub(super) fn maybe_emit_typestate_overlay(
    builder: &mut MermaidBuilder,
    entry_id: &str,
    typestate_marker: Option<&TypestateMarker>,
    style: &StyleConfig,
) {
    if typestate_marker.is_none() {
        return;
    }
    if let Some(overlay_class) = style.pattern.get("Typestate").map(|p| p.overlay_class.as_str()) {
        builder.push_class(entry_id, overlay_class);
    }
}

// ---------------------------------------------------------------------------
// Typestate-aware method loop (convenience wrapper used by render.rs)
// ---------------------------------------------------------------------------

/// Emits all method nodes (with param + returns edges) for a TypeEntry, using
/// typestate-aware returns edge style when applicable.
///
/// Param edges are always `--o` (unchanged by typestate membership).
/// Returns edges use `==>|transitions_to|` for transition methods and `-->` for all others.
///
/// # Errors
///
/// Returns `ContractMapRendererError::RenderFailed` when any param or returns `TypeRef`
/// has mismatched angle brackets (fail-closed, CN-03).
#[allow(clippy::too_many_arguments)]
pub(super) fn emit_methods_with_typestate(
    builder: &mut MermaidBuilder,
    entry_id: &str,
    methods: &[MethodDeclaration],
    typestate_marker: Option<&TypestateMarker>,
    caller_crate: &CrateName,
    type_index: &TypeIndex,
    style: &StyleConfig,
    method_shape: &str,
) -> Result<(), ContractMapRendererError> {
    let param_arrow = style
        .edge
        .get("method_param")
        .map(|e: &super::super::EdgeStyle| e.arrow.as_str())
        .unwrap_or("--o")
        .to_owned();

    for (i, method) in methods.iter().enumerate() {
        let method_id = format!("{entry_id}_m_{i}");
        builder.push_method_node(&method_id, method.name.as_str(), method_shape);

        // Param edges (unchanged by typestate).
        for param in &method.params {
            if let Some(target_id) = type_index.resolve(&param.ty, caller_crate)? {
                builder.push_edge(format!("{method_id} {param_arrow} {target_id}"));
            }
        }

        // Returns edge (typestate-aware).
        emit_returns_edge_maybe_transition(
            builder,
            &method_id,
            &method.name,
            &method.returns,
            typestate_marker,
            caller_crate,
            type_index,
            style,
        )?;
    }
    Ok(())
}
