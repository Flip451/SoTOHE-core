//! Phase 1 main entry-point: builds S and D from A (catalogue TypeGraph) and B (baseline).

use std::collections::{HashMap, HashSet};

use domain::tddd::ExtendedCrate;
use domain::tddd::Phase1Error;
use domain::tddd::catalogue_v2::ItemAction;
use rustdoc_types::{Crate, FORMAT_VERSION, Id, Target};

use super::super::super::collect_refs::{collect_referenced_ids, item_has_unresolved_marker};
use super::super::super::external_crates::{
    build_external_crates_for_scope, patch_paths_crate_ids, patch_paths_crate_ids_extra,
};
use super::super::super::resolution::resolve_unresolved_in_item;
use super::super::super::{build_function_identity_map, build_type_trait_identity_map};
use super::super::child_items::{
    insert_a_item_tree_into_s, insert_b_item_tree_into_s, remap_and_copy_a_children_to_s,
    remap_child_ids_in_item, remove_b_children_from_s,
};
use super::super::state::Phase1State;
use super::phase16_check::check_dangling_ids;
use super::rewrite::{make_root_module_item, rewrite_type_ref_ids_in_item};
use super::step55_impls::process_standalone_impls;

// ---------------------------------------------------------------------------
// Main Phase 1 entry-point
// ---------------------------------------------------------------------------

/// Main Phase 1 entry-point: builds S and D from A and B.
pub(crate) fn phase1_build_s_and_d(
    a: ExtendedCrate,
    b: &Crate,
) -> Result<(ExtendedCrate, Crate), Phase1Error> {
    // Determine crate name from B's root item.
    let crate_name = b.index.get(&b.root).and_then(|item| item.name.clone()).unwrap_or_default();

    // Seed the fresh-Id counter above the highest Id already used by B so that
    // initial allocations do not clash with B-side Ids.
    let first_fresh_id = b.index.keys().map(|id| id.0).max().map_or(1, |m| m + 1);
    let mut state = Phase1State::new(first_fresh_id);

    // --- Pre-step: Build B-wide Id remap (T037) ---
    //
    // Allocate a fresh S Id for every entry in b.index BEFORE any insertion.
    // `Id(0)` is excluded: it is the B-side root module (never inserted into S) and
    // the `Self`-type sentinel used by rustdoc inside impl blocks.
    {
        let mut b_keys: Vec<Id> = b.index.keys().filter(|id| id.0 != 0).copied().collect();
        b_keys.sort_by_key(|id| id.0);
        let b_remap: HashMap<Id, Id> =
            b_keys.into_iter().map(|old_id| (old_id, state.alloc_id())).collect();
        state.b_id_remap = b_remap;
    }

    // --- Step 1: Build B identity maps ---
    let b_types = build_type_trait_identity_map(b);
    let b_fns = build_function_identity_map(b);

    // --- Step 2: Seed S with all B items as implicit Reference ---
    for b_id in b_types.values() {
        if let Some(b_item) = b.index.get(b_id) {
            let path = b.paths.get(b_id).map(|ps| ps.path.clone());
            insert_b_item_tree_into_s(
                &mut state,
                b_item.clone(),
                ItemAction::Reference,
                path,
                &b.index,
            );
        }
    }
    for (fn_path_str, b_id) in &b_fns {
        if let Some(b_item) = b.index.get(b_id) {
            let path = b.paths.get(b_id).map(|ps| ps.path.clone());
            let s_id = state.b_id_remap.get(b_id).copied().unwrap_or_else(|| state.alloc_id());
            let rewritten = rewrite_type_ref_ids_in_item(b_item.clone(), &state.b_id_remap);
            state.insert_s_fn_at(s_id, rewritten, fn_path_str.clone(), ItemAction::Reference, path);
        }
    }

    // Orphan impl insertion: some types (notably TypeAlias) have no `impls` field,
    // so their trait impls are standalone Impl items in B's index that `collect_child_ids`
    // cannot reach.  After T037, check the REMAPPED id for presence in s_index.
    {
        let orphan_impl_ids: Vec<Id> = b
            .index
            .keys()
            .filter(|id| {
                b.index.get(*id).is_some_and(|item| {
                    item.crate_id == 0 && matches!(item.inner, rustdoc_types::ItemEnum::Impl(_))
                }) && {
                    let remapped = state.b_id_remap.get(*id).copied().unwrap_or(**id);
                    !state.s_index.contains_key(&remapped)
                }
            })
            .copied()
            .collect();
        for impl_id in orphan_impl_ids {
            if let Some(impl_item) = b.index.get(&impl_id) {
                let new_impl_s_id =
                    state.b_id_remap.get(&impl_id).copied().unwrap_or_else(|| state.alloc_id());
                let rewritten = rewrite_type_ref_ids_in_item(impl_item.clone(), &state.b_id_remap);
                let remapped = remap_child_ids_in_item(rewritten, &state.b_id_remap);
                let mut stored_impl = remapped;
                stored_impl.id = new_impl_s_id;
                state.s_index.insert(new_impl_s_id, stored_impl);
                state.s_actions.insert(new_impl_s_id, ItemAction::Reference);
                if let rustdoc_types::ItemEnum::Impl(impl_inner) = &impl_item.inner {
                    for &child_id in &impl_inner.items {
                        if let Some(child) = b.index.get(&child_id) {
                            let new_child_s_id = state
                                .b_id_remap
                                .get(&child_id)
                                .copied()
                                .unwrap_or_else(|| state.alloc_id());
                            let rewritten_child =
                                rewrite_type_ref_ids_in_item(child.clone(), &state.b_id_remap);
                            let remapped_child =
                                remap_child_ids_in_item(rewritten_child, &state.b_id_remap);
                            let mut stored_child = remapped_child;
                            stored_child.id = new_child_s_id;
                            state.s_index.entry(new_child_s_id).or_insert(stored_child);
                            state.s_actions.insert(new_child_s_id, ItemAction::Reference);
                        }
                    }
                }
            }
        }
    }

    // --- Step 3: Build A identity maps ---
    let (a_krate, a_item_actions) = a.into_parts();
    let a_types = build_type_trait_identity_map(&a_krate);
    let a_fns = build_function_identity_map(&a_krate);

    // --- Pre-step (A-side): Build A-wide Id remap (T008, IN-10) ---
    //
    // Symmetric counterpart of the B-side b_id_remap pre-step above.
    // `Id(0)` is excluded for the same reasons as in b_id_remap.
    {
        let mut a_keys: Vec<Id> = a_krate.index.keys().filter(|id| id.0 != 0).copied().collect();
        a_keys.sort_by_key(|id| id.0);
        let a_remap: HashMap<Id, Id> =
            a_keys.into_iter().map(|old_id| (old_id, state.alloc_id())).collect();
        state.a_id_remap = a_remap;
    }

    // --- Step 4 & 5: Process A items by action ---

    // Process A types/traits.
    for (a_name, a_id) in &a_types {
        let action = a_item_actions.get(a_id).copied().unwrap_or(ItemAction::Reference);
        let in_b = b_types.contains_key(a_name.as_str());

        let a_item = match a_krate.index.get(a_id) {
            Some(item) => item.clone(),
            None => continue,
        };

        match action {
            ItemAction::Add => {
                if in_b {
                    return Err(Phase1Error::ActionContradiction(format!(
                        "action=Add declared for '{a_name}' but it already exists in baseline"
                    )));
                }
                let path = a_krate.paths.get(a_id).map(|ps| ps.path.clone());
                insert_a_item_tree_into_s(
                    &mut state,
                    a_item,
                    ItemAction::Add,
                    path,
                    &a_krate.index,
                );
            }
            ItemAction::Modify => {
                if !in_b {
                    return Err(Phase1Error::ActionContradiction(format!(
                        "action=Modify declared for '{a_name}' but it does not exist in baseline"
                    )));
                }
                let s_id = state.s_type_id(a_name).ok_or_else(|| {
                    Phase1Error::ActionContradiction(format!(
                        "action=Modify: '{a_name}' expected in S but not found (internal error)"
                    ))
                })?;
                if let Some(b_item_in_s) = state.s_index.get(&s_id).cloned() {
                    remove_b_children_from_s(
                        &mut state.s_index,
                        &mut state.s_actions,
                        &b_item_in_s,
                    );
                }
                let remapped_a_item = remap_and_copy_a_children_to_s(
                    &mut state,
                    &a_item,
                    &a_krate.index,
                    ItemAction::Modify,
                );
                state.insert_s_type_at(s_id, remapped_a_item, ItemAction::Modify);
            }
            ItemAction::Reference => {
                if !in_b {
                    return Err(Phase1Error::ActionContradiction(format!(
                        "action=Reference declared for '{a_name}' but it does not exist in baseline"
                    )));
                }
                // S already has B's item as Reference — no change needed.
            }
            ItemAction::Delete => {
                if !in_b {
                    return Err(Phase1Error::ActionContradiction(format!(
                        "action=Delete declared for '{a_name}' but it does not exist in baseline"
                    )));
                }
                let s_id = state.s_type_id(a_name).ok_or_else(|| {
                    Phase1Error::ActionContradiction(format!(
                        "action=Delete: '{a_name}' expected in S but not found (internal error)"
                    ))
                })?;
                state.move_type_to_d(s_id);
            }
        }
    }

    // Process A functions.
    for (fn_path_str, a_id) in &a_fns {
        let action = a_item_actions.get(a_id).copied().unwrap_or(ItemAction::Reference);
        let in_b = b_fns.contains_key(fn_path_str.as_str());

        let a_item = match a_krate.index.get(a_id) {
            Some(item) => item.clone(),
            None => continue,
        };

        match action {
            ItemAction::Add => {
                if in_b {
                    return Err(Phase1Error::ActionContradiction(format!(
                        "action=Add declared for function '{fn_path_str}' but it already exists in baseline"
                    )));
                }
                let path = a_krate.paths.get(a_id).map(|ps| ps.path.clone());
                let fn_s_id =
                    state.a_id_remap.get(a_id).copied().unwrap_or_else(|| state.alloc_id());
                state.insert_s_fn_at(fn_s_id, a_item, fn_path_str.clone(), ItemAction::Add, path);
            }
            ItemAction::Modify => {
                if !in_b {
                    return Err(Phase1Error::ActionContradiction(format!(
                        "action=Modify declared for function '{fn_path_str}' but it does not exist in baseline"
                    )));
                }
                let s_id = state.s_fn_id(fn_path_str).ok_or_else(|| {
                    Phase1Error::ActionContradiction(format!(
                        "action=Modify: function '{fn_path_str}' expected in S but not found (internal error)"
                    ))
                })?;
                let mut new_item = a_item;
                new_item.id = s_id;
                state.s_index.insert(s_id, new_item);
                state.s_actions.insert(s_id, ItemAction::Modify);
            }
            ItemAction::Reference => {
                if !in_b {
                    return Err(Phase1Error::ActionContradiction(format!(
                        "action=Reference declared for function '{fn_path_str}' but it does not exist in baseline"
                    )));
                }
                // S already has B's function as Reference — no change needed.
            }
            ItemAction::Delete => {
                if !in_b {
                    return Err(Phase1Error::ActionContradiction(format!(
                        "action=Delete declared for function '{fn_path_str}' but it does not exist in baseline"
                    )));
                }
                let s_id = state.s_fn_id(fn_path_str).ok_or_else(|| {
                    Phase1Error::ActionContradiction(format!(
                        "action=Delete: function '{fn_path_str}' expected in S but not found (internal error)"
                    ))
                })?;
                state.move_fn_to_d(s_id, fn_path_str.clone());
            }
        }
    }

    // --- Step 5.5: A-side unified trait-impl insertion loop (ADR `2026-05-20-0048` D4) ---
    process_standalone_impls(&mut state, &a_krate, &a_item_actions, b, &crate_name)?;

    // --- Phase 1.45: A-side type-ref id remapping (local + external) ---
    //
    // Build a comprehensive A-id → fresh-Phase1-id remapping covering BOTH local and
    // external A-side Ids, then apply `rewrite_type_ref_ids_in_item` to all A-sourced
    // items in S.
    {
        // Build A-id → S-id map for LOCAL types.
        let a_local_to_s_id: HashMap<Id, Id> = a_krate
            .paths
            .iter()
            .filter_map(|(&a_id, a_ps)| {
                if a_ps.crate_id != 0 {
                    return None;
                }
                let name = a_ps.path.last()?.as_str();
                let s_id = state.s_type_name_to_id.get(name)?;
                Some((a_id, *s_id))
            })
            .collect();

        // Build A-id → fresh-Phase1-id map for EXTERNAL type-refs.
        let mut a_external_to_fresh_id: HashMap<Id, Id> = HashMap::new();
        let a_external_paths: Vec<(Id, rustdoc_types::ItemSummary)> = a_krate
            .paths
            .iter()
            .filter(|&(_, a_ps)| a_ps.crate_id != 0)
            .map(|(&a_id, a_ps)| (a_id, a_ps.clone()))
            .collect();
        for (old_id, path_summary) in a_external_paths {
            let fresh_id = state.alloc_id();
            state.s_paths.insert(fresh_id, path_summary);
            a_external_to_fresh_id.insert(old_id, fresh_id);
        }

        // Combine both maps for the rewrite pass.
        let mut full_remap: HashMap<Id, Id> = a_local_to_s_id;
        full_remap.extend(a_external_to_fresh_id);

        if !full_remap.is_empty() {
            // Only rewrite A-sourced items (Add / Modify actions).
            let s_item_ids: Vec<Id> = state.s_index.keys().copied().collect();
            for item_id in s_item_ids {
                let item_is_a_sourced = matches!(
                    state.s_actions.get(&item_id),
                    Some(&ItemAction::Add) | Some(&ItemAction::Modify),
                );
                if item_is_a_sourced {
                    if let Some(item) = state.s_index.remove(&item_id) {
                        let rewritten = rewrite_type_ref_ids_in_item(item, &full_remap);
                        state.s_index.insert(item_id, rewritten);
                    }
                }
            }
        }
    }

    // --- Phase 1.5: Closed-world unresolved-marker resolution ---
    let s_known_names: HashSet<String> = state.s_type_name_to_id.keys().cloned().collect();

    let items_with_markers: Vec<(Id, String)> = state
        .s_index
        .iter()
        .filter_map(|(&id, item)| {
            if item_has_unresolved_marker(item) {
                Some((id, item.name.clone().unwrap_or_else(|| format!("<id:{}>", id.0))))
            } else {
                None
            }
        })
        .collect();

    for (item_id, item_name) in items_with_markers {
        let item = match state.s_index.get(&item_id).cloned() {
            Some(i) => i,
            None => continue,
        };
        let resolved = resolve_unresolved_in_item(item, &s_known_names, &state.s_type_name_to_id)?;
        state.s_index.insert(item_id, resolved);
        let _ = item_name; // used in error reporting inside resolve_unresolved_in_item
    }

    // --- Phase 1.6: Dangling Id check ---
    check_dangling_ids(&state, &a_krate, b)?;

    // --- Step 6: Build external_crates for S and D (per-scope renumbering) ---

    let a_side_path_ids: HashSet<Id> = {
        let mut a_referenced_ids: HashSet<Id> = HashSet::new();
        let mut b_referenced_ids: HashSet<Id> = HashSet::new();
        for item in state.s_index.values() {
            let is_a_sourced = matches!(
                state.s_actions.get(&item.id),
                Some(&ItemAction::Add) | Some(&ItemAction::Modify),
            );
            let refs = collect_referenced_ids(item);
            if is_a_sourced {
                for id in refs {
                    a_referenced_ids.insert(id);
                }
            } else {
                for id in refs {
                    b_referenced_ids.insert(id);
                }
            }
        }

        let mut a_side_path_ids: HashSet<Id> = HashSet::new();

        // Track A-side path ids already in s_paths from Phase 1.45.
        for (&id, ps) in &state.s_paths {
            if ps.crate_id != 0 {
                a_side_path_ids.insert(id);
            }
        }

        // Insert A-side paths for A-referenced Ids.
        for &ref_id in &a_referenced_ids {
            if state.s_paths.contains_key(&ref_id) {
                continue;
            }
            if let Some(ps) = a_krate.paths.get(&ref_id) {
                if ps.crate_id != 0 {
                    state.s_paths.insert(ref_id, ps.clone());
                    a_side_path_ids.insert(ref_id);
                    continue;
                }
            }
            if let std::collections::hash_map::Entry::Vacant(e) = state.s_paths.entry(ref_id) {
                if let Some(ps) = b.paths.get(&ref_id) {
                    if ps.crate_id != 0 {
                        e.insert(ps.clone());
                    }
                }
            }
        }

        // Insert B-side paths for B-referenced Ids.
        for &ref_id in &b_referenced_ids {
            if let std::collections::hash_map::Entry::Vacant(e) = state.s_paths.entry(ref_id) {
                if let Some(ps) = b.paths.get(&ref_id) {
                    if ps.crate_id != 0 {
                        e.insert(ps.clone());
                    }
                }
            }
        }

        a_side_path_ids
    };
    {
        let mut d_referenced_ids: HashSet<Id> = HashSet::new();
        for item in state.d_index.values() {
            for id in collect_referenced_ids(item) {
                d_referenced_ids.insert(id);
            }
        }
        for &ref_id in &d_referenced_ids {
            if let std::collections::hash_map::Entry::Vacant(e) = state.d_paths.entry(ref_id) {
                if let Some(ps) = b.paths.get(&ref_id) {
                    if ps.crate_id != 0 {
                        e.insert(ps.clone());
                    }
                }
            }
        }
    }

    let (s_external_crates, s_name_to_new_id) = build_external_crates_for_scope(
        &state.s_index,
        &state.s_paths,
        b,
        Some(&a_krate),
        Some(&a_side_path_ids),
    );
    patch_paths_crate_ids_extra(
        &mut state.s_paths,
        &a_krate,
        &s_name_to_new_id,
        Some(&a_side_path_ids),
    );
    patch_paths_crate_ids(&mut state.s_paths, b, &s_name_to_new_id, Some(&a_side_path_ids));

    let (d_external_crates, d_name_to_new_id) =
        build_external_crates_for_scope(&state.d_index, &state.d_paths, b, None, None);
    patch_paths_crate_ids(&mut state.d_paths, b, &d_name_to_new_id, None);

    // Allocate fresh Phase1-managed Ids for the S and D root modules.
    // Both ids must be allocated here — before `state.s_actions` is partially moved.
    let s_root_id = state.alloc_id();
    let d_root_id = state.alloc_id();

    // Build root module item for S.
    let mut s_top_ids: Vec<Id> =
        state.s_type_name_to_id.values().chain(state.s_fn_path_to_id.values()).copied().collect();
    s_top_ids.sort_by_key(|id| id.0);
    let s_root_item = make_root_module_item(s_root_id, crate_name.clone(), s_top_ids);
    state.s_index.insert(s_root_id, s_root_item);

    let s_krate = Crate {
        root: s_root_id,
        crate_version: None,
        includes_private: false,
        index: state.s_index,
        paths: state.s_paths,
        external_crates: s_external_crates,
        format_version: FORMAT_VERSION,
        target: Target { triple: String::new(), target_features: vec![] },
    };
    let s = ExtendedCrate::new(s_krate, state.s_actions);

    // Build root module item for D.
    let mut d_top_ids: Vec<Id> =
        state.d_type_name_to_id.values().chain(state.d_fn_path_to_id.values()).copied().collect();
    d_top_ids.sort_by_key(|id| id.0);
    let d_root_item = make_root_module_item(d_root_id, crate_name.clone(), d_top_ids);
    state.d_index.insert(d_root_id, d_root_item);

    let d = Crate {
        root: d_root_id,
        crate_version: None,
        includes_private: false,
        index: state.d_index,
        paths: state.d_paths,
        external_crates: d_external_crates,
        format_version: FORMAT_VERSION,
        target: Target { triple: String::new(), target_features: vec![] },
    };

    Ok((s, d))
}
