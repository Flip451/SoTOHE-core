//! Phase 2 — S / D / C 3-way evaluation.
//!
//! Builds identity sets for S, D, and C, then evaluates each item against
//! the signal table (ADR 3 D3) to produce `ThreeWaySignal`s.

use std::collections::HashSet;

use domain::tddd::catalogue_v2::ItemAction;
use domain::tddd::{SignalRegion, ThreeWayEvaluationReport, ThreeWaySignal};
use rustdoc_types::Crate;

use super::structural_eq::items_structurally_equal;
use super::{build_function_identity_map, build_impl_identity_map, build_type_trait_identity_map};
use domain::tddd::ExtendedCrate;

/// Runs Phase 2: evaluates S / D / C and produces a `ThreeWayEvaluationReport`.
pub(super) fn phase2_evaluate(s: &ExtendedCrate, d: &Crate, c: &Crate) -> ThreeWayEvaluationReport {
    let s_krate = s.krate();

    // Derive the crate name from C's root item so that rustdoc local-trait paths
    // (`my_crate::MyTrait`) can be normalized to match codec paths (`crate::MyTrait`).
    let crate_name = c.index.get(&c.root).and_then(|item| item.name.as_deref()).unwrap_or("");

    // Build identity sets.
    // Phase 2 uses short-name keys for types/traits, matching the `ThreeWaySignal`
    // domain contract (item_name = short name for types/traits; FunctionPath for functions).
    let s_types = build_type_trait_identity_map(s_krate);
    let s_fns = build_function_identity_map(s_krate);
    // S inherits B-seeded impl blocks whose trait paths are in rustdoc format
    // (`my_crate::MyTrait`). Pass `crate_name` so those are normalized to short
    // names that match C-side keys.
    let s_impls = build_impl_identity_map(s_krate, crate_name);
    let d_types = build_type_trait_identity_map(d);
    let d_fns = build_function_identity_map(d);
    let d_impls = build_impl_identity_map(d, crate_name);
    let c_types = build_type_trait_identity_map(c);
    let c_fns = build_function_identity_map(c);
    let c_impls = build_impl_identity_map(c, crate_name);

    let mut signals: Vec<ThreeWaySignal> = Vec::new();

    // --- Evaluate S types/traits vs C ---
    for (name, s_id) in &s_types {
        let action = s.action_for(s_id).unwrap_or(ItemAction::Reference);
        let s_item = s_krate.index.get(s_id);

        if let Some(c_id) = c_types.get(name.as_str()) {
            // Item in S ∩ C.
            let c_item = c.index.get(c_id);
            let structurally_equal = match (s_item, c_item) {
                (Some(si), Some(ci)) => {
                    items_structurally_equal(si, ci, &s_krate.index, &c.index, crate_name)
                }
                _ => false,
            };
            let region = s_intersect_c_region(action, structurally_equal);
            signals.push(ThreeWaySignal::new(name.clone(), region));
        } else {
            // Item in S \ C.
            let region = s_minus_c_region(action);
            signals.push(ThreeWaySignal::new(name.clone(), region));
        }
    }

    // --- Evaluate S functions vs C ---
    for (fn_path, s_id) in &s_fns {
        let action = s.action_for(s_id).unwrap_or(ItemAction::Reference);
        let s_item = s_krate.index.get(s_id);

        if let Some(c_id) = c_fns.get(fn_path.as_str()) {
            let c_item = c.index.get(c_id);
            let structurally_equal = match (s_item, c_item) {
                (Some(si), Some(ci)) => {
                    items_structurally_equal(si, ci, &s_krate.index, &c.index, crate_name)
                }
                _ => false,
            };
            let region = s_intersect_c_region(action, structurally_equal);
            signals.push(ThreeWaySignal::new(fn_path.clone(), region));
        } else {
            let region = s_minus_c_region(action);
            signals.push(ThreeWaySignal::new(fn_path.clone(), region));
        }
    }

    // --- Evaluate S impls vs C ---
    for (key, s_id) in &s_impls {
        let action = s.action_for(s_id).unwrap_or(ItemAction::Reference);
        let s_item = s_krate.index.get(s_id);

        if let Some(c_id) = c_impls.get(key.as_str()) {
            let c_item = c.index.get(c_id);
            let structurally_equal = match (s_item, c_item) {
                (Some(si), Some(ci)) => {
                    items_structurally_equal(si, ci, &s_krate.index, &c.index, crate_name)
                }
                _ => false,
            };
            let region = s_intersect_c_region(action, structurally_equal);
            signals.push(ThreeWaySignal::new(key.clone(), region));
        } else {
            let region = s_minus_c_region(action);
            signals.push(ThreeWaySignal::new(key.clone(), region));
        }
    }

    // --- Evaluate D types/traits vs C ---
    for name in d_types.keys() {
        if c_types.contains_key(name.as_str()) {
            // D ∩ C: delete in progress.
            signals.push(ThreeWaySignal::new(name.clone(), SignalRegion::DIntersectC));
        } else {
            // D \ C: delete achieved.
            signals.push(ThreeWaySignal::new(name.clone(), SignalRegion::DMinusC));
        }
    }

    // --- Evaluate D functions vs C ---
    for fn_path in d_fns.keys() {
        if c_fns.contains_key(fn_path.as_str()) {
            signals.push(ThreeWaySignal::new(fn_path.clone(), SignalRegion::DIntersectC));
        } else {
            signals.push(ThreeWaySignal::new(fn_path.clone(), SignalRegion::DMinusC));
        }
    }

    // --- Evaluate D impls vs C ---
    for key in d_impls.keys() {
        if c_impls.contains_key(key.as_str()) {
            signals.push(ThreeWaySignal::new(key.clone(), SignalRegion::DIntersectC));
        } else {
            signals.push(ThreeWaySignal::new(key.clone(), SignalRegion::DMinusC));
        }
    }

    // --- Evaluate C \ (S ∪ D): undeclared implementations ---
    // For each C type/trait not in S or D.
    let s_union_d_types: HashSet<&str> =
        s_types.keys().chain(d_types.keys()).map(String::as_str).collect();
    for name in c_types.keys() {
        if !s_union_d_types.contains(name.as_str()) {
            signals.push(ThreeWaySignal::new(name.clone(), SignalRegion::CMinusSUnionD));
        }
    }

    // For each C function not in S or D.
    let s_union_d_fns: HashSet<&str> =
        s_fns.keys().chain(d_fns.keys()).map(String::as_str).collect();
    for fn_path in c_fns.keys() {
        if !s_union_d_fns.contains(fn_path.as_str()) {
            signals.push(ThreeWaySignal::new(fn_path.clone(), SignalRegion::CMinusSUnionD));
        }
    }

    // For each C impl not in S or D.
    let s_union_d_impls: HashSet<&str> =
        s_impls.keys().chain(d_impls.keys()).map(String::as_str).collect();
    for key in c_impls.keys() {
        if !s_union_d_impls.contains(key.as_str()) {
            signals.push(ThreeWaySignal::new(key.clone(), SignalRegion::CMinusSUnionD));
        }
    }

    ThreeWayEvaluationReport::new(signals)
}

/// Determines the `SignalRegion` for an item in S ∩ C.
pub(super) fn s_intersect_c_region(action: ItemAction, structurally_equal: bool) -> SignalRegion {
    match (action, structurally_equal) {
        (ItemAction::Add, true) => SignalRegion::SIntersectC_Match_Add,
        (ItemAction::Modify, true) => SignalRegion::SIntersectC_Match_Modify,
        (ItemAction::Reference, true) => SignalRegion::SIntersectC_Match_Reference,
        (ItemAction::Reference, false) => SignalRegion::SIntersectC_Mismatch_Reference,
        (ItemAction::Add, false) => SignalRegion::SIntersectC_Mismatch_Add,
        (ItemAction::Modify, false) => SignalRegion::SIntersectC_Mismatch_Modify,
        // Delete should never appear in S (already moved to D in Phase 1).
        // Treat as Reference mismatch as a safe fallback.
        (ItemAction::Delete, true) => SignalRegion::SIntersectC_Match_Reference,
        (ItemAction::Delete, false) => SignalRegion::SIntersectC_Mismatch_Reference,
    }
}

/// Determines the `SignalRegion` for an item in S \ C.
pub(super) fn s_minus_c_region(action: ItemAction) -> SignalRegion {
    match action {
        ItemAction::Add => SignalRegion::SMinusC_Add,
        ItemAction::Modify => SignalRegion::SMinusC_Modify,
        ItemAction::Reference => SignalRegion::SMinusC_Reference,
        // Delete should never appear in S, treat as Reference.
        ItemAction::Delete => SignalRegion::SMinusC_Reference,
    }
}
