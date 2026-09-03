//! Phase 1 main entry-point: builds S and D from A (catalogue TypeGraph) and B (baseline).

use std::collections::{BTreeMap, HashMap, HashSet};

use domain::tddd::ExtendedCrate;
use domain::tddd::Phase1Error;
use domain::tddd::catalogue_v2::{CrateName, ItemAction};
use rustdoc_types::{Crate, FORMAT_VERSION, Id, Target};

use super::super::super::collect_refs::{collect_referenced_ids, item_has_unresolved_marker};
use super::super::super::external_crates::{
    build_external_crates_for_scope, patch_paths_crate_ids, patch_paths_crate_ids_extra,
};
use super::super::super::impl_identity::build_impl_identity_map;
use super::super::super::resolution::resolve_unresolved_in_item;
use super::super::super::resolve_type::resolve_type;
use super::super::super::{
    RustdocTargetResolution, TypeTraitIdentityKey, TypeTraitIdentityMap,
    build_function_identity_map, build_type_trait_identity_map,
};
use super::super::child_items::{
    insert_a_item_tree_into_s, insert_b_item_tree_into_s, remap_and_copy_a_children_to_s,
    remap_child_ids_in_item, remove_b_children_from_s,
};
use super::super::rustdoc_authority::{canonicalize_rustdoc_paths, merge_definition_path_maps};
use super::super::state::Phase1State;
use super::d_paths::populate_d_paths;
use super::phase16_check::check_dangling_ids;
use super::rewrite::{make_root_module_item, rewrite_type_ref_ids_in_item};
use super::step55_impls::process_standalone_impls;
use crate::tddd::canonical_type_identity::{DefinitionPathAuthority, SYNTHETIC_UNPLACED_CRATE_ID};

// ---------------------------------------------------------------------------
// Main Phase 1 entry-point
// ---------------------------------------------------------------------------

/// Main Phase 1 entry-point with a resolved package-to-rustdoc-root translation.
pub(crate) fn phase1_build_s_and_d_with_rustdoc_root(
    a: ExtendedCrate,
    b: &Crate,
    rustdoc_root: Option<&RustdocTargetResolution>,
) -> Result<(ExtendedCrate, Crate), Phase1Error> {
    // Prefer the resolved package root and then the catalogue root.
    let crate_name = rustdoc_root
        .map(|resolution| resolution.package_name().as_str().to_owned())
        .or_else(|| a.krate().index.get(&a.krate().root).and_then(|item| item.name.clone()))
        .or_else(|| b.index.get(&b.root).and_then(|item| item.name.clone()))
        .unwrap_or_default();

    // Canonicalize baseline paths before building Phase 1 identity maps.
    let package_name = CrateName::new(crate_name.clone()).ok();
    let canonical_b = canonicalize_rustdoc_paths(
        b,
        package_name.as_ref(),
        rustdoc_root.map(|resolution| resolution.rustdoc_root_name()),
    );
    let b = &canonical_b;

    // Seed the fresh-Id counter above every Id in B's index *and* paths maps.
    // `Crate::paths` contains external items that are not present in `index`;
    // allocating into that range would make a remapped local B reference look
    // like an unrelated external path when D is assembled (for example, a
    // deleted impl for a local type could be labelled as `alloc::...::ValMut`).
    let first_fresh_id =
        b.index.keys().chain(b.paths.keys()).map(|id| id.0).max().map_or(Ok(1), |max_id| {
            max_id.checked_add(1).ok_or_else(|| {
                Phase1Error::rustdoc_root_resolution(
                    "Phase 1 item-id space is exhausted by the baseline rustdoc artifact",
                )
            })
        })?;
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
    let b_types = build_type_trait_identity_map(b)?;
    let b_fns = build_function_identity_map(b, rustdoc_root);

    // --- Step 2: Seed S with all B items as implicit Reference ---
    //
    // `b_types` is keyed by the complete `Crate::paths` identity, so same-name
    // items in different modules are independent entries rather than a single
    // short-name winner.
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
    let a_types = build_type_trait_identity_map(&a_krate)?;
    let a_fns = build_function_identity_map(&a_krate, rustdoc_root);

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
        // Delete tombstones may arrive as a short catalogue key even though the
        // baseline identity map is fully qualified. Resolve that spelling before
        // the action contradiction checks; never let a short-name lookup select
        // one member of an ambiguous baseline pair.
        let action_identity = match action {
            ItemAction::Delete => resolve_delete_identity(a_name, &b_types)?,
            _ => a_name.clone(),
        };
        let in_b = b_types.contains_key(&action_identity);

        let a_item = match a_krate.index.get(a_id) {
            Some(item) => item.clone(),
            None => continue,
        };

        match action {
            ItemAction::Add => {
                if in_b {
                    return Err(Phase1Error::action_contradiction(format!(
                        "action=Add declared for '{action_identity}' but it already exists in baseline"
                    )));
                }
                let source_path = a_krate.paths.get(a_id).cloned();
                let path = source_path.as_ref().map(|summary| summary.path.clone());
                let s_id = insert_a_item_tree_into_s(
                    &mut state,
                    a_item,
                    ItemAction::Add,
                    path,
                    &a_krate.index,
                );
                // `insert_a_item_tree_into_s` writes local-looking summaries for
                // every A item. Restore the adapter-owned marker for an omitted
                // catalogue placement so an unplaced add remains distinct from a
                // crate-root definition in the S graph.
                if source_path
                    .as_ref()
                    .is_some_and(|summary| summary.crate_id == SYNTHETIC_UNPLACED_CRATE_ID)
                {
                    if let Some(summary) = state.s_paths.get_mut(&s_id) {
                        summary.crate_id = SYNTHETIC_UNPLACED_CRATE_ID;
                    }
                }
            }
            ItemAction::Modify => {
                if !in_b {
                    return Err(Phase1Error::action_contradiction(format!(
                        "action=Modify declared for '{action_identity}' but it does not exist in baseline"
                    )));
                }
                let s_id = state.s_type_id(action_identity.path.as_str()).ok_or_else(|| {
                    Phase1Error::action_contradiction(format!(
                        "action=Modify: '{action_identity}' expected in S but not found (internal error)"
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
                    return Err(Phase1Error::action_contradiction(format!(
                        "action=Reference declared for '{action_identity}' but it does not exist in baseline"
                    )));
                }
                // S already has B's item as Reference — no change needed.
            }
            ItemAction::Delete => {
                if !in_b {
                    return Err(Phase1Error::action_contradiction(format!(
                        "action=Delete declared for '{action_identity}' but it does not exist in baseline"
                    )));
                }
                let s_id = state.s_type_id(action_identity.path.as_str()).ok_or_else(|| {
                    Phase1Error::action_contradiction(format!(
                        "action=Delete: '{action_identity}' expected in S but not found (internal error)"
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
                    return Err(Phase1Error::action_contradiction(format!(
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
                    return Err(Phase1Error::action_contradiction(format!(
                        "action=Modify declared for function '{fn_path_str}' but it does not exist in baseline"
                    )));
                }
                let s_id = state.s_fn_id(fn_path_str).ok_or_else(|| {
                    Phase1Error::action_contradiction(format!(
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
                    return Err(Phase1Error::action_contradiction(format!(
                        "action=Reference declared for function '{fn_path_str}' but it does not exist in baseline"
                    )));
                }
                // S already has B's function as Reference — no change needed.
            }
            ItemAction::Delete => {
                if !in_b {
                    return Err(Phase1Error::action_contradiction(format!(
                        "action=Delete declared for function '{fn_path_str}' but it does not exist in baseline"
                    )));
                }
                let s_id = state.s_fn_id(fn_path_str).ok_or_else(|| {
                    Phase1Error::action_contradiction(format!(
                        "action=Delete: function '{fn_path_str}' expected in S but not found (internal error)"
                    ))
                })?;
                state.move_fn_to_d(s_id, fn_path_str.clone());
            }
        }
    }

    // --- Step 5.5: unified trait-impl insertion with one canonical authority ---
    let catalogue_definition_paths = a_krate
        .paths
        .iter()
        .filter(|(_, summary)| {
            summary.crate_id == 0 || summary.crate_id == SYNTHETIC_UNPLACED_CRATE_ID
        })
        .map(|(&id, summary)| (id, summary.clone()))
        .collect::<HashMap<_, _>>();
    let definition_path_map = merge_definition_path_maps(&b.paths, &catalogue_definition_paths)?;
    let definition_paths = DefinitionPathAuthority::from_path_maps(&definition_path_map, &[]);
    let a_impl_map = build_impl_identity_map(&a_krate, &crate_name, &definition_paths)?;
    let b_impl_map = build_impl_identity_map(b, &crate_name, &definition_paths)?;
    process_standalone_impls(&mut state, &a_krate, &a_item_actions, b, &a_impl_map, &b_impl_map)?;

    // --- Phase 1.45: A-side type-ref id remapping (local + external) ---
    //
    // Build a comprehensive A-id → fresh-Phase1-id remapping covering BOTH local and
    // external A-side Ids, then apply `rewrite_type_ref_ids_in_item` to all A-sourced
    // items in S.
    {
        // Build A-id → S-id map for LOCAL types. Use the namespace-aware A/B
        // identity maps rather than `s_type_identity_to_id`: the latter is a
        // compatibility map keyed only by rendered path, so it cannot retain
        // both an unplaced type and an unplaced trait whose paths are equal.
        // Added declarations live at their preallocated A-side S ids; modified
        // and referenced declarations replace or reuse the remapped baseline id.
        let a_local_to_s_id: HashMap<Id, Id> = (&a_types)
            .into_iter()
            .filter_map(|(identity, a_id)| {
                let action = a_item_actions.get(a_id).copied().unwrap_or(ItemAction::Reference);
                let s_id = match action {
                    ItemAction::Add => state.a_id_remap.get(a_id).copied(),
                    ItemAction::Modify | ItemAction::Reference => {
                        b_types.get(identity).and_then(|b_id| state.b_id_remap.get(b_id).copied())
                    }
                    ItemAction::Delete => None,
                }?;
                state.s_index.contains_key(&s_id).then_some((*a_id, s_id))
            })
            .collect();

        // Build A-id → fresh-Phase1-id map for EXTERNAL type-refs.
        let mut a_external_to_fresh_id: HashMap<Id, Id> = HashMap::new();
        let a_external_paths: Vec<(Id, rustdoc_types::ItemSummary)> = a_krate
            .paths
            .iter()
            .filter(|&(_, a_ps)| a_ps.crate_id != 0 && a_ps.crate_id != SYNTHETIC_UNPLACED_CRATE_ID)
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
    // Phase 1.5 resolves only through authoritative fully-qualified identities.
    // Keep the short-name map for legacy child-transfer bookkeeping, but never
    // let it choose an Id for a TypeRef: same-named declarations may coexist in
    // different modules.
    let s_known_names: HashSet<String> = state.s_type_identity_to_id.keys().cloned().collect();

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
        let item =
            resolve_unresolved_impl_trait_path(item, &s_known_names, &state.s_type_identity_to_id)?;
        let resolved =
            resolve_unresolved_in_item(item, &s_known_names, &state.s_type_identity_to_id)?;
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
            if ps.crate_id != 0 && ps.crate_id != SYNTHETIC_UNPLACED_CRATE_ID {
                a_side_path_ids.insert(id);
            }
        }

        // Insert A-side paths for A-referenced Ids.
        for &ref_id in &a_referenced_ids {
            if state.s_paths.contains_key(&ref_id) {
                continue;
            }
            if let Some(ps) = a_krate.paths.get(&ref_id) {
                if ps.crate_id != 0 && ps.crate_id != SYNTHETIC_UNPLACED_CRATE_ID {
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
    populate_d_paths(&mut state, b);

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

    // All non-root allocations are complete.  Sibling helpers use the checked
    // allocator but cannot propagate its error through their existing APIs, so
    // convert a latched exhaustion into a typed Phase 1 error before allocating
    // or publishing either root.
    state.check_id_allocation()?;

    // Allocate fresh Phase1-managed Ids for the S and D root modules.
    // Both ids must be allocated here — before `state.s_actions` is partially moved.
    let s_root_id = state.alloc_id();
    let d_root_id = state.alloc_id();
    state.check_id_allocation()?;

    // Build root module item for S from namespace-aware baseline/catalogue
    // identities. The legacy path-only S map cannot be used here because it
    // would drop one of two unplaced Add declarations sharing one rendered
    // path. Deleted baseline entries have already left `s_index` and are
    // therefore excluded by the membership check.
    let mut s_top_ids: Vec<Id> = Vec::new();
    for b_id in b_types.values() {
        if let Some(s_id) = state.b_id_remap.get(b_id).copied() {
            if state.s_index.contains_key(&s_id) {
                s_top_ids.push(s_id);
            }
        }
    }
    for (_, a_id) in &a_types {
        let action = a_item_actions.get(a_id).copied().unwrap_or(ItemAction::Reference);
        if action == ItemAction::Add {
            if let Some(s_id) = state.a_id_remap.get(a_id).copied() {
                if state.s_index.contains_key(&s_id) {
                    s_top_ids.push(s_id);
                }
            }
        }
    }
    s_top_ids.extend(state.s_fn_path_to_id.values().copied());
    s_top_ids.sort_by_key(|id| id.0);
    s_top_ids.dedup();
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
    let mut d_top_ids: Vec<Id> = state
        .d_type_identity_to_id
        .values()
        .chain(state.d_fn_path_to_id.values())
        .copied()
        .collect();
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

/// Resolves a type/trait delete tombstone against the frozen baseline identity map.
///
/// Live catalogue entries have already crossed the codec's canonical identity
/// boundary. A tombstone is identity-only, however, and older catalogue files may
/// still spell it as a bare name. The baseline is the only authoritative universe
/// for that lookup: an exact qualified key wins, one short-name candidate is
/// accepted, and both ambiguous and unresolved spellings fail closed before the
/// action contradiction branch is reached.
fn resolve_delete_identity(
    tombstone: &TypeTraitIdentityKey,
    baseline_types: &TypeTraitIdentityMap,
) -> Result<TypeTraitIdentityKey, Phase1Error> {
    if baseline_types.contains_key(tombstone) {
        return Ok(tombstone.clone());
    }
    let raw_name = tombstone.path.as_str();
    if raw_name.contains("::") {
        return Err(Phase1Error::action_contradiction(format!(
            "action=Delete tombstone '{raw_name}' is unresolved in baseline rustdoc paths"
        )));
    }

    // A tombstone only matches baseline identities of its own namespace.
    let candidates = baseline_types
        .keys()
        .filter(|identity| {
            identity.namespace == tombstone.namespace && identity.short_name() == raw_name
        })
        .cloned()
        .collect::<Vec<_>>();
    match candidates.as_slice() {
        [identity] => Ok(identity.clone()),
        [] => Err(Phase1Error::action_contradiction(format!(
            "action=Delete tombstone '{raw_name}' is unresolved in baseline rustdoc paths"
        ))),
        _ => Err(Phase1Error::action_contradiction(format!(
            "action=Delete tombstone '{raw_name}' is ambiguous in baseline rustdoc paths; candidates: {}",
            candidates.iter().map(ToString::to_string).collect::<Vec<_>>().join(", ")
        ))),
    }
}

/// Resolves an impl's trait path through the suffix-aware TypeRef resolver before
/// the item-level legacy driver sees it. The driver still owns all other nested
/// resolution, but its Impl branch historically looked up only the final segment
/// in the identity map.
fn resolve_unresolved_impl_trait_path(
    mut item: rustdoc_types::Item,
    known_names: &HashSet<String>,
    identity_to_id: &BTreeMap<String, Id>,
) -> Result<rustdoc_types::Item, Phase1Error> {
    let rustdoc_types::ItemEnum::Impl(mut implementation) = item.inner else {
        return Ok(item);
    };
    if let Some(trait_path) = implementation.trait_.take() {
        let resolved = resolve_type(
            rustdoc_types::Type::ResolvedPath(trait_path),
            known_names,
            identity_to_id,
        )?;
        let rustdoc_types::Type::ResolvedPath(trait_path) = resolved else {
            return Err(Phase1Error::unresolved_type_ref(
                "trait impl path did not resolve to a path",
            ));
        };
        implementation.trait_ = Some(trait_path);
    }
    item.inner = rustdoc_types::ItemEnum::Impl(implementation);
    Ok(item)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::indexing_slicing, clippy::panic, clippy::unwrap_used)]
#[path = "main_fn_tests.rs"]
pub(crate) mod tests;
