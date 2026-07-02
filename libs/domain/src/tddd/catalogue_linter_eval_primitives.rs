//! `ForbidPrimitiveInTypes` rule evaluation (T005, ADR `2026-07-01-0004`).
//!
//! This module is declared by `catalogue_linter.rs` via `#[path]` and is not
//! a public module. [`evaluate_forbid_primitive_in_types`] is invoked from the
//! `ForbidPrimitiveInTypes` match arm in `catalogue_linter_eval.rs`.
//!
//! Collects every catalogue-structural `TypeRef`-bearing slot (named struct
//! field, enum variant field, method/function param, method/function return,
//! generic bound, `type_alias` target) for entries selected by the rule's
//! `RuleTarget` when the caller's target layer is one of the rule's `layers`,
//! scans each slot via the injected [`PrimitiveOccurrenceScanner`], and emits a
//! [`CatalogueLintViolation`] per (entry, position, primitive) match against
//! the rule's requested `positions`.

use super::helpers::{
    collect_methods_for_type, function_entries_for_target, trait_entries_for_target,
    type_entries_for_target,
};
use super::{CatalogueLintViolation, CatalogueLinterError, CatalogueLinterRule};
use crate::tddd::catalogue_v2::CatalogueDocument;
use crate::tddd::catalogue_v2::composite::{StructShape, TypeKindV2};
use crate::tddd::catalogue_v2::identifiers::TypeRef;
use crate::tddd::catalogue_v2::methods::{
    MethodGenericParam, ParamDeclaration, WherePredicateDecl,
};
use crate::tddd::catalogue_v2::roles::NonEmptyVec;
use crate::tddd::catalogue_v2::variants::VariantPayload;
use crate::tddd::layer_id::LayerId;
use crate::tddd::primitive_occurrence_scanner::{
    PrimitiveName, PrimitiveOccurrencePosition, PrimitiveOccurrenceScanner,
};

/// One catalogue-structural `TypeRef` slot discovered while collecting
/// occurrence sites for a `ForbidPrimitiveInTypes` rule.
///
/// Owns `entry_name` and `type_ref` (rather than borrowing) because the three
/// entry kinds this module scans (`TypeEntry`, `TraitEntry`, `FunctionEntry`)
/// key their catalogue maps with different identifier types, and only
/// `TypeName` / `TraitName` expose a borrowed `.as_str()`; `FunctionPath` only
/// implements `Display`. Owning uniformly avoids threading a lifetime
/// parameter through every collector function for a single non-`Display`
/// outlier.
struct PrimitiveSlot {
    entry_name: String,
    type_ref: TypeRef,
    position: PrimitiveOccurrencePosition,
}

/// Evaluates a `ForbidPrimitiveInTypes` rule against `target_layer_id`'s
/// catalogue only when `target_layer_id` is included in the rule's `layers`.
///
/// The caller (`evaluate_catalogue_lint`) is invoked once per layer by the
/// composition root; this function respects that per-layer contract by scoping
/// the scan to the caller's `target_layer_id` and returning an empty violation
/// list when the rule does not apply to that layer. This avoids the
/// double-counting that would occur if the rule iterated its own `layers`
/// list independently of the caller's target layer. The caller validates all
/// configured rule layers against `all_catalogues` before invoking this helper.
///
/// For every catalogue entry selected by `rule.target()`, iterates its
/// `TypeRef`-bearing catalogue-structural slots, scans each via `scanner`,
/// and emits a [`CatalogueLintViolation`] for every (entry, position,
/// primitive) combination where `positions` requests a position at which the
/// scan found one of `primitives`.
///
/// # Errors
///
/// Returns [`CatalogueLinterError::ScanFailed`] when the injected `scanner`
/// fails to parse a catalogue `TypeRef`.
pub(super) fn evaluate_forbid_primitive_in_types<S: PrimitiveOccurrenceScanner>(
    rule: &CatalogueLinterRule,
    catalogue: &CatalogueDocument,
    target_layer_id: &LayerId,
    primitives: &NonEmptyVec<PrimitiveName>,
    layers: &NonEmptyVec<LayerId>,
    positions: &NonEmptyVec<PrimitiveOccurrencePosition>,
    scanner: &S,
) -> Result<Vec<CatalogueLintViolation>, CatalogueLinterError> {
    if !layers.as_slice().iter().any(|layer_id| layer_id == target_layer_id) {
        return Ok(Vec::new());
    }

    let discriminant_name = rule.kind().discriminant_name();
    let mut violations = Vec::new();

    let mut slots: Vec<PrimitiveSlot> = Vec::new();
    collect_type_entry_slots(catalogue, rule, &mut slots)?;
    collect_trait_entry_slots(catalogue, rule, &mut slots);
    collect_function_entry_slots(catalogue, rule, &mut slots);

    check_slots(discriminant_name, &slots, primitives, positions, scanner, &mut violations)?;

    Ok(violations)
}

/// Collects `NamedField` / `VariantField` / `TypeAliasTarget` slots from a
/// type entry's own shape, plus `Param` / `Return` / `Bound` slots from its
/// methods (`TypeEntry.methods` merged with matching `inherent_impls`, via
/// `collect_methods_for_type`).
fn collect_type_entry_slots(
    catalogue: &CatalogueDocument,
    rule: &CatalogueLinterRule,
    slots: &mut Vec<PrimitiveSlot>,
) -> Result<(), CatalogueLinterError> {
    for (name, entry) in type_entries_for_target(catalogue, rule.target()) {
        let entry_name = name.as_str().to_owned();

        match &entry.kind {
            TypeKindV2::Struct(struct_kind) => {
                // Only the `Plain` (named-field) shape has a `NamedField`-equivalent
                // slot; `Tuple` struct fields are unnamed and have no matching
                // `PrimitiveOccurrencePosition` variant, so they are excluded.
                if let StructShape::Plain { fields, .. } = &struct_kind.shape {
                    for field in fields {
                        slots.push(PrimitiveSlot {
                            entry_name: entry_name.clone(),
                            type_ref: field.ty.clone(),
                            position: PrimitiveOccurrencePosition::NamedField,
                        });
                    }
                }
            }
            TypeKindV2::Enum { variants } => {
                for variant in variants {
                    match &variant.payload {
                        VariantPayload::Unit => {}
                        VariantPayload::Tuple(type_refs) => {
                            for type_ref in type_refs {
                                slots.push(PrimitiveSlot {
                                    entry_name: entry_name.clone(),
                                    type_ref: type_ref.clone(),
                                    position: PrimitiveOccurrencePosition::VariantField,
                                });
                            }
                        }
                        VariantPayload::Struct(fields) => {
                            for field in fields {
                                slots.push(PrimitiveSlot {
                                    entry_name: entry_name.clone(),
                                    type_ref: field.ty.clone(),
                                    position: PrimitiveOccurrencePosition::VariantField,
                                });
                            }
                        }
                    }
                }
            }
            TypeKindV2::TypeAlias { target } => {
                slots.push(PrimitiveSlot {
                    entry_name: entry_name.clone(),
                    type_ref: target.clone(),
                    position: PrimitiveOccurrencePosition::TypeAliasTarget,
                });
            }
        }

        let methods = collect_methods_for_type(catalogue, entry, &entry_name)?;
        for method in &methods {
            push_param_return_generic_slots(
                &entry_name,
                &method.params,
                &method.returns,
                &method.generics,
                &method.where_predicates,
                slots,
            );
        }
    }
    Ok(())
}

/// Collects `Param` / `Return` / `Bound` slots from a trait entry's methods,
/// its own `generics` / `where_predicates`, its `supertrait_bounds`, and its
/// associated types' `bounds`. Associated consts (`AssocConstDecl`) are out of
/// scope (T005 covers the 6 named slot kinds only).
fn collect_trait_entry_slots(
    catalogue: &CatalogueDocument,
    rule: &CatalogueLinterRule,
    slots: &mut Vec<PrimitiveSlot>,
) {
    for (name, entry) in trait_entries_for_target(catalogue, rule.target()) {
        let entry_name = name.as_str();

        for method in &entry.methods {
            push_param_return_generic_slots(
                entry_name,
                &method.params,
                &method.returns,
                &method.generics,
                &method.where_predicates,
                slots,
            );
        }

        push_generic_and_where_slots(entry_name, &entry.generics, &entry.where_predicates, slots);

        for bound in &entry.supertrait_bounds {
            slots.push(PrimitiveSlot {
                entry_name: entry_name.to_owned(),
                type_ref: bound.clone(),
                position: PrimitiveOccurrencePosition::Bound,
            });
        }

        for assoc_type in &entry.assoc_types {
            for bound in &assoc_type.bounds {
                slots.push(PrimitiveSlot {
                    entry_name: entry_name.to_owned(),
                    type_ref: bound.clone(),
                    position: PrimitiveOccurrencePosition::Bound,
                });
            }
        }
    }
}

/// Collects `Param` / `Return` / `Bound` slots from a free function entry.
fn collect_function_entry_slots(
    catalogue: &CatalogueDocument,
    rule: &CatalogueLinterRule,
    slots: &mut Vec<PrimitiveSlot>,
) {
    for (path, entry) in function_entries_for_target(catalogue, rule.target()) {
        // `FunctionPath` has no `.as_str()` (only `Display`), unlike
        // `TypeName` / `TraitName`; `.to_string()` is the only way to obtain
        // a name for the slot's `entry_name`.
        let entry_name = path.to_string();
        push_param_return_generic_slots(
            &entry_name,
            &entry.params,
            &entry.returns,
            &entry.generics,
            &entry.where_predicates,
            slots,
        );
    }
}

/// Pushes a `Param` slot for each parameter, a `Return` slot for the return
/// type, and `Bound` slots for `generics` / `where_predicates` — the four
/// fields shared identically by both `MethodDeclaration` and `FunctionEntry`.
fn push_param_return_generic_slots(
    entry_name: &str,
    params: &[ParamDeclaration],
    returns: &TypeRef,
    generics: &[MethodGenericParam],
    where_predicates: &[WherePredicateDecl],
    slots: &mut Vec<PrimitiveSlot>,
) {
    for param in params {
        slots.push(PrimitiveSlot {
            entry_name: entry_name.to_owned(),
            type_ref: param.ty.clone(),
            position: PrimitiveOccurrencePosition::Param,
        });
    }
    slots.push(PrimitiveSlot {
        entry_name: entry_name.to_owned(),
        type_ref: returns.clone(),
        position: PrimitiveOccurrencePosition::Return,
    });
    push_generic_and_where_slots(entry_name, generics, where_predicates, slots);
}

/// Pushes a `Bound` slot for each generic param's bounds and each
/// where-clause predicate type expression. Both `WherePredicateDecl.lhs` (the
/// constrained type expression) and each `rhs` bound are `TypeRef`-bearing
/// bound positions.
fn push_generic_and_where_slots(
    entry_name: &str,
    generics: &[MethodGenericParam],
    where_predicates: &[WherePredicateDecl],
    slots: &mut Vec<PrimitiveSlot>,
) {
    for generic in generics {
        for bound in &generic.bounds {
            slots.push(PrimitiveSlot {
                entry_name: entry_name.to_owned(),
                type_ref: bound.clone(),
                position: PrimitiveOccurrencePosition::Bound,
            });
        }
    }
    for pred in where_predicates {
        slots.push(PrimitiveSlot {
            entry_name: entry_name.to_owned(),
            type_ref: pred.lhs.clone(),
            position: PrimitiveOccurrencePosition::Bound,
        });
        for bound in &pred.rhs {
            slots.push(PrimitiveSlot {
                entry_name: entry_name.to_owned(),
                type_ref: bound.clone(),
                position: PrimitiveOccurrencePosition::Bound,
            });
        }
    }
}

/// Scans every collected slot via `scanner`, and for each position in
/// `positions`, emits one `CatalogueLintViolation` per primitive the scan
/// found at that position.
fn check_slots<S: PrimitiveOccurrenceScanner>(
    discriminant_name: &'static str,
    slots: &[PrimitiveSlot],
    primitives: &NonEmptyVec<PrimitiveName>,
    positions: &NonEmptyVec<PrimitiveOccurrencePosition>,
    scanner: &S,
    violations: &mut Vec<CatalogueLintViolation>,
) -> Result<(), CatalogueLinterError> {
    for slot in slots {
        let report = scanner.scan(slot.type_ref.clone(), primitives.clone(), slot.position)?;
        for position in positions.as_slice() {
            let Some(found) = report.by_position().get(position) else {
                continue;
            };
            for primitive in found {
                violations.push(CatalogueLintViolation::new(
                    discriminant_name,
                    slot.entry_name.clone(),
                    format!(
                        "primitive '{}' found at {:?} in type reference '{}'",
                        primitive.as_str(),
                        position,
                        slot.type_ref.as_str(),
                    ),
                ));
            }
        }
    }
    Ok(())
}
