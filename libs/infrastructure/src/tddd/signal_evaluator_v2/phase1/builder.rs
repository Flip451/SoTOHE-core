//! Phase 1 main entry-point: builds S and D from A (catalogue TypeGraph) and B (baseline).

use std::collections::{HashMap, HashSet};

use domain::tddd::ExtendedCrate;
use domain::tddd::Phase1Error;
use domain::tddd::catalogue_v2::ItemAction;
use rustdoc_types::{
    AssocItemConstraint, AssocItemConstraintKind, Crate, DynTrait, FORMAT_VERSION, GenericArg,
    GenericArgs, GenericBound, GenericParamDef, GenericParamDefKind, Id, Item, ItemEnum, Module,
    Path, PolyTrait, Target, Term, Type, WherePredicate,
};

use super::super::collect_refs::{collect_referenced_ids, item_has_unresolved_marker};
use super::super::external_crates::{
    build_external_crates_for_scope, patch_paths_crate_ids, patch_paths_crate_ids_extra,
};
use super::super::resolution::resolve_unresolved_in_item;
use super::super::{build_function_identity_map, build_type_trait_identity_map};
use super::child_items::{
    collect_child_ids, insert_a_item_tree_into_s, insert_b_item_tree_into_s, patch_impl_for_ids,
    patch_impl_trait_ids, remap_and_copy_a_children_to_s, remove_b_children_from_s,
};
use super::state::Phase1State;

// ---------------------------------------------------------------------------
// Main Phase 1 entry-point
// ---------------------------------------------------------------------------

/// Main Phase 1 entry-point: builds S and D from A and B.
pub(in crate::tddd::signal_evaluator_v2) fn phase1_build_s_and_d(
    a: ExtendedCrate,
    b: &Crate,
) -> Result<(ExtendedCrate, Crate), Phase1Error> {
    // Determine crate name from B's root item.
    let crate_name = b.index.get(&b.root).and_then(|item| item.name.clone()).unwrap_or_default();

    // Seed the fresh-Id counter above the highest Id already used by B so that
    // new allocations never clash with B-seeded items in s_index.
    let first_fresh_id = b.index.keys().map(|id| id.0).max().map_or(1, |m| m + 1);
    let mut state = Phase1State::new(first_fresh_id);

    // --- Step 1: Build B identity maps ---
    let b_types = build_type_trait_identity_map(b);
    let b_fns = build_function_identity_map(b);

    // --- Step 2: Seed S with all B items as implicit Reference ---
    // For each B type/trait, insert into S (including child items).
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
    // For each B function, insert into S.
    for (fn_path_str, b_id) in &b_fns {
        if let Some(b_item) = b.index.get(b_id) {
            let path = b.paths.get(b_id).map(|ps| ps.path.clone());
            state.insert_s_fn(b_item.clone(), fn_path_str.clone(), ItemAction::Reference, path);
        }
    }

    // Orphan impl insertion: some types (notably TypeAlias) have no `impls` field,
    // so their trait impls are standalone Impl items in B's index that `collect_child_ids`
    // cannot reach.  Phase 2's `build_impl_identity_map` does find them in C (walking the
    // whole index), so omitting them from S would produce spurious `CMinusSUnionD` signals.
    // Insert any Impl item not already in S.  Their method/assoc-item children are also
    // inserted (they are found via the impl's own `items` list, which is the only subtree
    // possible for an orphan impl).
    {
        let orphan_impl_ids: Vec<Id> = b
            .index
            .keys()
            .filter(|id| {
                b.index.get(*id).is_some_and(|item| {
                    item.crate_id == 0 && matches!(item.inner, ItemEnum::Impl(_))
                }) && !state.s_index.contains_key(*id)
            })
            .copied()
            .collect();
        for impl_id in orphan_impl_ids {
            if let Some(impl_item) = b.index.get(&impl_id) {
                // Insert the impl itself.
                state.s_index.insert(impl_id, impl_item.clone());
                state.s_actions.insert(impl_id, ItemAction::Reference);
                // Insert the impl's direct children (methods, assoc items).
                if let ItemEnum::Impl(impl_inner) = &impl_item.inner {
                    for &child_id in &impl_inner.items {
                        if let Some(child) = b.index.get(&child_id) {
                            state.s_index.entry(child_id).or_insert_with(|| child.clone());
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
                // Add: identity must NOT exist in B.
                if in_b {
                    return Err(Phase1Error::ActionContradiction(format!(
                        "action=Add declared for '{a_name}' but it already exists in baseline"
                    )));
                }
                let path = a_krate.paths.get(a_id).map(|ps| ps.path.clone());
                // Insert top-level item and all child items from A's index,
                // remapping A-side child Ids to fresh S Ids to avoid clashing
                // with B-seeded child Ids already in s_index.
                insert_a_item_tree_into_s(
                    &mut state,
                    a_item,
                    ItemAction::Add,
                    path,
                    &a_krate.index,
                );
            }
            ItemAction::Modify => {
                // Modify: identity must exist in B.
                if !in_b {
                    return Err(Phase1Error::ActionContradiction(format!(
                        "action=Modify declared for '{a_name}' but it does not exist in baseline"
                    )));
                }
                // Replace B's item in S with A's item (intended shape post-modification).
                // First purge all B-side child items so they do not linger next to A's children.
                let s_id = state.s_type_id(a_name).ok_or_else(|| {
                    Phase1Error::ActionContradiction(format!(
                        "action=Modify: '{a_name}' expected in S but not found (internal error)"
                    ))
                })?;
                let is_trait = matches!(a_item.inner, ItemEnum::Trait(_));
                if let Some(b_item_in_s) = state.s_index.get(&s_id).cloned() {
                    remove_b_children_from_s(&mut state.s_index, &b_item_in_s);
                }
                // Remap A child Ids to fresh S Ids before inserting.
                // Pass ItemAction::Modify so impl-block children inherit the parent's action.
                let remapped_a_item = remap_and_copy_a_children_to_s(
                    &mut state,
                    &a_item,
                    &a_krate.index,
                    ItemAction::Modify,
                );
                // Collect new impl child Ids (from the remapped item) before inserting,
                // so we can patch their impl.for_ / impl.trait_ references afterwards.
                let new_impl_ids: Vec<rustdoc_types::Id> = collect_child_ids(&remapped_a_item)
                    .into_iter()
                    .filter(|id| {
                        state
                            .s_index
                            .get(id)
                            .is_some_and(|item| matches!(item.inner, ItemEnum::Impl(_)))
                    })
                    .collect();
                state.insert_s_type_at(s_id, remapped_a_item, ItemAction::Modify);
                // Patch the impl blocks' self-type / trait reference to point at the preserved
                // S-side parent id.  This is the same step performed in the Add path.
                if is_trait {
                    patch_impl_trait_ids(&mut state.s_index, &new_impl_ids, s_id);
                } else {
                    patch_impl_for_ids(&mut state.s_index, &new_impl_ids, s_id);
                }
            }
            ItemAction::Reference => {
                // Reference: identity must exist in B.
                if !in_b {
                    return Err(Phase1Error::ActionContradiction(format!(
                        "action=Reference declared for '{a_name}' but it does not exist in baseline"
                    )));
                }
                // S already has B's item as Reference — no change needed.
                // The action is already ItemAction::Reference from step 2.
            }
            ItemAction::Delete => {
                // Delete: identity must exist in B.
                if !in_b {
                    return Err(Phase1Error::ActionContradiction(format!(
                        "action=Delete declared for '{a_name}' but it does not exist in baseline"
                    )));
                }
                // Move B's item from S to D.
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
                state.insert_s_fn(a_item, fn_path_str.clone(), ItemAction::Add, path);
            }
            ItemAction::Modify => {
                if !in_b {
                    return Err(Phase1Error::ActionContradiction(format!(
                        "action=Modify declared for function '{fn_path_str}' but it does not exist in baseline"
                    )));
                }
                // Replace the existing S function item with A's version.
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

    // --- Phase 1.45: A-side local type-ref id remapping ---
    // `type_ref_parser::parse_type_ref` resolves local catalogue types to A-side item Ids
    // via `resolve_local`. These A-side Ids are embedded directly in `Type::ResolvedPath.id`
    // and are NOT remapped to S-side Ids during item insertion (only structural child-list Ids
    // are remapped in `remap_child_ids_in_item`).
    //
    // Without remapping:
    // - Phase 1.6 Rule 1 can produce false positives: an A-side Id for a deleted type may
    //   coincidentally match a retained B/S Id for a different item, incorrectly passing the
    //   dangling-Id check.
    //
    // Fix: build an A-side-id → S-side-id mapping from `a_krate.paths` + `s_type_name_to_id`,
    // then rewrite all `Type::ResolvedPath.id` values in every S item using this map.  The
    // rewrite is safe because Phase 2 uses short-name identity and `format_type` L1 resolution
    // (not `ResolvedPath.id`) for structural equality; the only consumer of these Ids is
    // Phase 1.6 itself.
    {
        // Build A-id → S-id map: for each A-side local item, find its name via a_krate.paths,
        // then look up the S-side id via s_type_name_to_id.
        let a_to_s_id: HashMap<Id, Id> = a_krate
            .paths
            .iter()
            .filter_map(|(&a_id, a_ps)| {
                if a_ps.crate_id != 0 {
                    return None; // external crate — not a local type
                }
                // Use the last path segment as the type short name.
                let name = a_ps.path.last()?.as_str();
                let s_id = state.s_type_name_to_id.get(name)?;
                Some((a_id, *s_id))
            })
            .collect();

        if !a_to_s_id.is_empty() {
            // Only rewrite A-sourced items (Add / Modify actions) and their child items.
            // B-sourced Reference items use B-side Ids which are correct and must not be
            // rewritten: A and B use independent Id spaces and their Ids may collide.
            //
            // A-sourced top-level items have `Add` or `Modify` in `s_actions`.
            // A-sourced child items (fields, variant payloads, impl methods) got
            // FRESH ids >= `first_fresh_id` via `insert_a_item_tree_into_s`.
            // B-sourced items and their children kept their original B-side ids
            // which are all < `first_fresh_id` (by construction).
            //
            // So the correct discriminator is: item_id.0 >= first_fresh_id → A-sourced child.
            // Also include top-level A-sourced items (Add/Modify in s_actions) even if their
            // ids were assigned by `insert_s_type_at` (Modify keeps the B-side id).
            let a_sourced_top_ids: std::collections::HashSet<Id> = state
                .s_actions
                .iter()
                .filter_map(|(&id, &action)| {
                    if action == ItemAction::Add || action == ItemAction::Modify {
                        Some(id)
                    } else {
                        None
                    }
                })
                .collect();
            // Collect all item Ids in s_index to avoid borrowing state twice.
            let s_item_ids: Vec<Id> = state.s_index.keys().copied().collect();
            for item_id in s_item_ids {
                // Rewrite only A-sourced items:
                // 1. Top-level items with Add/Modify action.
                // 2. Child items allocated with fresh ids >= first_fresh_id.
                // Skip B-sourced Reference items (id < first_fresh_id AND in s_actions as Reference).
                let should_rewrite =
                    a_sourced_top_ids.contains(&item_id) || item_id.0 >= first_fresh_id;
                if should_rewrite {
                    if let Some(item) = state.s_index.remove(&item_id) {
                        let rewritten = rewrite_type_ref_ids_in_item(item, &a_to_s_id);
                        state.s_index.insert(item_id, rewritten);
                    }
                }
            }
        }
    }

    // --- Phase 1.5: Closed-world unresolved-marker resolution ---
    // Collect all type names currently in S (the closed-world universe after Delete).
    let s_known_names: HashSet<String> = state.s_type_name_to_id.keys().cloned().collect();

    // Scan all items in S for unresolved markers (Id == UNRESOLVED_CRATE_ID).
    // Collect (item_id, item_name) pairs that contain unresolved markers.
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
        // Attempt to resolve all unresolved markers in this item.
        let resolved = resolve_unresolved_in_item(item, &s_known_names, &state.s_type_name_to_id)?;
        state.s_index.insert(item_id, resolved);
        let _ = item_name; // used in error reporting inside resolve_unresolved_in_item
    }

    // --- Phase 1.6: Dangling Id check ---
    // After resolution and A-side id remapping (Phase 1.45), check that no local-crate Id
    // referenced in S's items is missing from S.
    //
    // `collect_referenced_ids` pre-filters the Id list so that:
    // - `Id(UNRESOLVED_CRATE_ID)` is **never** yielded.  A-sourced external type references
    //   (e.g. `serde::Serialize`) retain `Id(UNRESOLVED_CRATE_ID)` as the path id after
    //   Phase 1.5 (because `is_local_unresolved_path` guards external names).
    //   `collect_ids_from_type` explicitly skips any id equal to `UNRESOLVED_CRATE_ID`, so
    //   external references never reach this check and are never rejected as dangling.
    //
    // After Phase 1.45, all A-sourced local type refs have been remapped to S-side Ids.
    // A-side Ids that could not be remapped (because the target type was deleted) remain stale;
    // Phase 1.6 correctly rejects them via Rule 4 below.
    //
    // Exemption rules for non-UNRESOLVED ids (in evaluation order):
    // 1. Present in `s_ids` or `s_paths` → S-local ref (possibly remapped from A-side), valid.
    // 2. In `b.paths` with `crate_id != 0` → B-side external-crate item (std/serde/…), valid.
    // 3. In `b.paths` with `crate_id == 0` but not in S → local item deleted in Phase 1, REJECT.
    // 4. In `a_krate.paths` with `crate_id != 0` → A-side external-crate item referenced by an
    //    Add/Modify item (e.g. std::vec::Vec, an external trait impl).  Phase 1.45 does not
    //    remap external-crate Ids (they use `UNRESOLVED_CRATE_ID` anyway), but some paths in
    //    `a_krate.paths` may carry external-crate synthetic Ids that survived remapping.
    // 5. Not in `b.paths`, not in `a_krate.paths` (external), not in S → stale or unknown id,
    //    REJECT (fail-closed).
    //
    // Note: the old Rule 5 exempted B-side child items (StructField, Variant, …) found in
    // `b.index` but not `b.paths` as unconditionally valid.  That was incorrect: if the child's
    // parent was deleted in Phase 1, `remove_child_items_from_s` purges the child from S too,
    // so the reference to it is genuinely dangling.  Since `insert_b_item_tree_into_s` now
    // inserts ALL B child items into S under their original Ids (via `copy_b_children_to_s`),
    // every valid B child ref is already in `s_ids` and is covered by Rule 1.  Any id that
    // reaches this point is therefore stale/dangling regardless of `b.index` membership.
    let s_ids: HashSet<Id> = state.s_index.keys().copied().collect();

    for item in state.s_index.values() {
        let item_name = item.name.clone().unwrap_or_else(|| format!("<id:{}>", item.id.0));
        for referenced_id in collect_referenced_ids(item) {
            // Rule 1: id is in S (B-sourced or remapped A-sourced) — valid.
            if s_ids.contains(&referenced_id) || state.s_paths.contains_key(&referenced_id) {
                continue;
            }
            // Rules 2–3: id appears in B's paths.
            if let Some(ps) = b.paths.get(&referenced_id) {
                if ps.crate_id != 0 {
                    // Rule 2: External-crate item (std/serde/…) — valid cross-crate ref.
                    continue;
                }
                // Rule 3: Local top-level item (crate_id == 0) not in S → deleted → dangling.
                return Err(Phase1Error::DanglingId(format!(
                    "{item_name} -> Id({}) not found in S (may have been deleted in Phase 1)",
                    referenced_id.0
                )));
            }
            // Rule 4: id appears in A's paths with crate_id != 0 — A-side external-crate item.
            // (Local A-side Ids were remapped to S-side Ids in Phase 1.45 and are covered by
            // Rule 1.  An A-side local Id that survives here means Phase 1.45 could not find
            // a corresponding S item — which is a dangling reference, caught by Rule 5 below.)
            if let Some(a_ps) = a_krate.paths.get(&referenced_id) {
                if a_ps.crate_id != 0 {
                    // Rule 4: External-crate item (std/serde/…) — valid cross-crate ref.
                    continue;
                }
                // A-side local id that was NOT remapped in Phase 1.45 (target type deleted).
                let a_name = a_ps.path.last().map(|s| s.as_str()).unwrap_or("<unknown>");
                return Err(Phase1Error::DanglingId(format!(
                    "{item_name} -> Id({}) is an A-side local ref to '{}' which is not present in S \
                     (the type may have been deleted or was never declared in the catalogue)",
                    referenced_id.0, a_name
                )));
            }
            // Rule 5: Not in S, not in b.paths, not in a_krate.paths → stale or unknown id,
            // REJECT (fail-closed).
            return Err(Phase1Error::DanglingId(format!(
                "{item_name} -> Id({}) is not in S and not in B or A paths (stale reference or unknown id)",
                referenced_id.0
            )));
        }
    }

    // --- Step 6: Build external_crates for S and D (per-scope renumbering) ---
    // Also patch ItemSummary.crate_id values so S/D paths are internally consistent.

    // Copy referenced external ItemSummary entries from B (and A for S) into scope paths
    // before building the external_crates map.  This ensures that any consumer that resolves
    // an external item's Id via `krate.paths → crate_id → external_crates` can find the
    // entry and that `patch_paths_crate_ids` correctly updates their crate_id to the new
    // per-scope numbering.
    // Only copy entries for Ids actually present in the scope's items (type-ref scanner
    // finds these), and only external ones (crate_id != 0).
    {
        // Collect all Ids referenced by items in S (types + impls + functions).
        let mut s_referenced_ids: HashSet<Id> = HashSet::new();
        for item in state.s_index.values() {
            for id in collect_referenced_ids(item) {
                s_referenced_ids.insert(id);
            }
        }
        // Copy external B-side path summaries for those ids into s_paths.
        for &ref_id in &s_referenced_ids {
            if let std::collections::hash_map::Entry::Vacant(e) = state.s_paths.entry(ref_id) {
                if let Some(ps) = b.paths.get(&ref_id) {
                    if ps.crate_id != 0 {
                        e.insert(ps.clone());
                    }
                }
            }
        }
        // Copy external A-side path summaries (for A-added items that reference external
        // types with A-side synthetic Ids not in b.paths).
        for &ref_id in &s_referenced_ids {
            if let std::collections::hash_map::Entry::Vacant(e) = state.s_paths.entry(ref_id) {
                if let Some(ps) = a_krate.paths.get(&ref_id) {
                    if ps.crate_id != 0 {
                        e.insert(ps.clone());
                    }
                }
            }
        }
    }
    {
        let mut d_referenced_ids: HashSet<Id> = HashSet::new();
        for item in state.d_index.values() {
            for id in collect_referenced_ids(item) {
                d_referenced_ids.insert(id);
            }
        }
        // Copy external B-side path summaries into d_paths (D is B-only).
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

    // S may contain A-sourced items that reference external types via A-side synthetic Ids
    // not present in b.paths; pass Some(&a_krate) so those Ids can be resolved via
    // a_krate.paths + a_krate.external_crates.
    let (s_external_crates, s_name_to_new_id) =
        build_external_crates_for_scope(&state.s_index, &state.s_paths, b, Some(&a_krate));
    patch_paths_crate_ids(&mut state.s_paths, b, &s_name_to_new_id);
    // Also patch A-side external path summaries now in s_paths.
    patch_paths_crate_ids_extra(&mut state.s_paths, &a_krate, &s_name_to_new_id);

    // D contains only B-sourced items; no A-side ids remain, so extra_a = None.
    let (d_external_crates, d_name_to_new_id) =
        build_external_crates_for_scope(&state.d_index, &state.d_paths, b, None);
    patch_paths_crate_ids(&mut state.d_paths, b, &d_name_to_new_id);

    // Build root module item for S.
    let root_id = Id(0);
    let mut s_top_ids: Vec<Id> =
        state.s_type_name_to_id.values().chain(state.s_fn_path_to_id.values()).copied().collect();
    s_top_ids.sort_by_key(|id| id.0);
    let s_root_item = make_root_module_item(root_id, crate_name.clone(), s_top_ids);
    state.s_index.insert(root_id, s_root_item);

    let s_krate = Crate {
        root: root_id,
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
    let d_root_item = make_root_module_item(root_id, crate_name.clone(), d_top_ids);
    state.d_index.insert(root_id, d_root_item);

    let d = Crate {
        root: root_id,
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

// ---------------------------------------------------------------------------
// A-side type-ref id remapping (Phase 1.45)
// ---------------------------------------------------------------------------

/// Rewrites all `Type::ResolvedPath.id` (and nested path Ids in generic args, bounds,
/// etc.) within `item` using `id_map`.  Ids not present in the map are left unchanged.
///
/// This is used in Phase 1.45 to replace A-side local type Ids with their S-side
/// equivalents after all A items have been inserted into S.
fn rewrite_type_ref_ids_in_item(mut item: Item, id_map: &HashMap<Id, Id>) -> Item {
    item.inner = match item.inner {
        ItemEnum::StructField(ty) => ItemEnum::StructField(rewrite_type_ids(&ty, id_map)),
        ItemEnum::TypeAlias(mut ta) => {
            ta.type_ = rewrite_type_ids(&ta.type_, id_map);
            ta.generics = rewrite_generics_ids(ta.generics, id_map);
            ItemEnum::TypeAlias(ta)
        }
        ItemEnum::Struct(mut s) => {
            s.generics = rewrite_generics_ids(s.generics, id_map);
            ItemEnum::Struct(s)
        }
        ItemEnum::Enum(mut e) => {
            e.generics = rewrite_generics_ids(e.generics, id_map);
            ItemEnum::Enum(e)
        }
        ItemEnum::Trait(mut t) => {
            t.generics = rewrite_generics_ids(t.generics, id_map);
            t.bounds = t.bounds.into_iter().map(|b| rewrite_bound_ids(b, id_map)).collect();
            ItemEnum::Trait(t)
        }
        ItemEnum::Function(mut f) => {
            f.sig.inputs = f
                .sig
                .inputs
                .into_iter()
                .map(|(name, ty)| (name, rewrite_type_ids(&ty, id_map)))
                .collect();
            f.sig.output = f.sig.output.map(|ty| rewrite_type_ids(&ty, id_map));
            f.generics = rewrite_generics_ids(f.generics, id_map);
            ItemEnum::Function(f)
        }
        ItemEnum::Impl(mut i) => {
            i.for_ = rewrite_type_ids(&i.for_, id_map);
            if let Some(ref mut trait_path) = i.trait_ {
                rewrite_path_id(trait_path, id_map);
            }
            i.generics = rewrite_generics_ids(i.generics, id_map);
            ItemEnum::Impl(i)
        }
        ItemEnum::AssocType { generics, bounds, type_ } => {
            let generics = rewrite_generics_ids(generics, id_map);
            let bounds = bounds.into_iter().map(|b| rewrite_bound_ids(b, id_map)).collect();
            let type_ = type_.map(|ty| rewrite_type_ids(&ty, id_map));
            ItemEnum::AssocType { generics, bounds, type_ }
        }
        ItemEnum::AssocConst { type_, value } => {
            ItemEnum::AssocConst { type_: rewrite_type_ids(&type_, id_map), value }
        }
        other => other,
    };
    item
}

/// Rewrites all `ResolvedPath.id` and nested path Ids within a `Type`.
fn rewrite_type_ids(ty: &Type, id_map: &HashMap<Id, Id>) -> Type {
    match ty {
        Type::ResolvedPath(p) => {
            let new_id = id_map.get(&p.id).copied().unwrap_or(p.id);
            let new_args = p.args.as_deref().map(|a| Box::new(rewrite_generic_args_ids(a, id_map)));
            Type::ResolvedPath(Path { path: p.path.clone(), id: new_id, args: new_args })
        }
        Type::BorrowedRef { lifetime, is_mutable, type_: inner } => Type::BorrowedRef {
            lifetime: lifetime.clone(),
            is_mutable: *is_mutable,
            type_: Box::new(rewrite_type_ids(inner, id_map)),
        },
        Type::Slice(inner) => Type::Slice(Box::new(rewrite_type_ids(inner, id_map))),
        Type::Array { type_: inner, len } => {
            Type::Array { type_: Box::new(rewrite_type_ids(inner, id_map)), len: len.clone() }
        }
        Type::Tuple(tys) => Type::Tuple(tys.iter().map(|t| rewrite_type_ids(t, id_map)).collect()),
        Type::RawPointer { is_mutable, type_: inner } => Type::RawPointer {
            is_mutable: *is_mutable,
            type_: Box::new(rewrite_type_ids(inner, id_map)),
        },
        Type::ImplTrait(bounds) => {
            Type::ImplTrait(bounds.iter().map(|b| rewrite_bound_ids(b.clone(), id_map)).collect())
        }
        Type::DynTrait(dt) => {
            let new_traits: Vec<PolyTrait> = dt
                .traits
                .iter()
                .map(|pt| {
                    let mut new_path = pt.trait_.clone();
                    rewrite_path_id(&mut new_path, id_map);
                    PolyTrait { trait_: new_path, generic_params: pt.generic_params.clone() }
                })
                .collect();
            Type::DynTrait(DynTrait { traits: new_traits, lifetime: dt.lifetime.clone() })
        }
        Type::FunctionPointer(fp) => {
            let new_inputs = fp
                .sig
                .inputs
                .iter()
                .map(|(name, t)| (name.clone(), rewrite_type_ids(t, id_map)))
                .collect();
            let new_output = fp.sig.output.as_ref().map(|t| rewrite_type_ids(t, id_map));
            let new_fp = rustdoc_types::FunctionPointer {
                sig: rustdoc_types::FunctionSignature {
                    inputs: new_inputs,
                    output: new_output,
                    is_c_variadic: fp.sig.is_c_variadic,
                },
                generic_params: fp.generic_params.clone(),
                header: fp.header.clone(),
            };
            Type::FunctionPointer(Box::new(new_fp))
        }
        Type::QualifiedPath { name, self_type, trait_, args } => {
            let new_self = rewrite_type_ids(self_type, id_map);
            let new_trait = trait_.as_ref().map(|p| {
                let mut np = p.clone();
                rewrite_path_id(&mut np, id_map);
                np
            });
            let new_args = args.as_deref().map(|a| Box::new(rewrite_generic_args_ids(a, id_map)));
            Type::QualifiedPath {
                name: name.clone(),
                self_type: Box::new(new_self),
                trait_: new_trait,
                args: new_args,
            }
        }
        Type::Pat { type_: inner, __pat_unstable_do_not_use } => Type::Pat {
            type_: Box::new(rewrite_type_ids(inner, id_map)),
            __pat_unstable_do_not_use: __pat_unstable_do_not_use.clone(),
        },
        // Generic, Primitive, Infer — no Ids to remap.
        other => other.clone(),
    }
}

/// Rewrites path Ids inside a `GenericArgs` value.
fn rewrite_generic_args_ids(args: &GenericArgs, id_map: &HashMap<Id, Id>) -> GenericArgs {
    match args {
        GenericArgs::AngleBracketed { args: ga, constraints } => {
            let new_args = ga
                .iter()
                .map(|a| match a {
                    GenericArg::Type(t) => GenericArg::Type(rewrite_type_ids(t, id_map)),
                    other => other.clone(),
                })
                .collect();
            let new_constraints: Vec<AssocItemConstraint> = constraints
                .iter()
                .map(|c| {
                    let new_binding = match &c.binding {
                        AssocItemConstraintKind::Equality(Term::Type(ty)) => {
                            AssocItemConstraintKind::Equality(Term::Type(rewrite_type_ids(
                                ty, id_map,
                            )))
                        }
                        AssocItemConstraintKind::Constraint(bounds) => {
                            AssocItemConstraintKind::Constraint(
                                bounds
                                    .iter()
                                    .map(|b| rewrite_bound_ids(b.clone(), id_map))
                                    .collect(),
                            )
                        }
                        other => other.clone(),
                    };
                    AssocItemConstraint {
                        name: c.name.clone(),
                        args: c
                            .args
                            .as_deref()
                            .map(|a| Box::new(rewrite_generic_args_ids(a, id_map))),
                        binding: new_binding,
                    }
                })
                .collect();
            GenericArgs::AngleBracketed { args: new_args, constraints: new_constraints }
        }
        GenericArgs::Parenthesized { inputs, output } => GenericArgs::Parenthesized {
            inputs: inputs.iter().map(|t| rewrite_type_ids(t, id_map)).collect(),
            output: output.as_ref().map(|t| rewrite_type_ids(t, id_map)),
        },
        other => other.clone(),
    }
}

/// Rewrites a single `Path.id` in place using `id_map`.
fn rewrite_path_id(path: &mut Path, id_map: &HashMap<Id, Id>) {
    if let Some(&new_id) = id_map.get(&path.id) {
        path.id = new_id;
    }
    if let Some(ref mut args) = path.args {
        **args = rewrite_generic_args_ids(args, id_map);
    }
}

/// Rewrites path Ids inside a `GenericBound`.
fn rewrite_bound_ids(bound: GenericBound, id_map: &HashMap<Id, Id>) -> GenericBound {
    match bound {
        GenericBound::TraitBound { trait_, modifier, generic_params } => {
            let mut new_trait = trait_;
            rewrite_path_id(&mut new_trait, id_map);
            // Rewrite ids inside HRTB binder params (e.g. `for<T: LocalTrait>`).
            // Without this, local type ids nested in HRTB binders remain in A's id space
            // and are rejected by Phase 1.6 as dangling references.
            let new_generic_params =
                generic_params.into_iter().map(|p| rewrite_generic_param_ids(p, id_map)).collect();
            GenericBound::TraitBound {
                trait_: new_trait,
                modifier,
                generic_params: new_generic_params,
            }
        }
        other => other,
    }
}

/// Rewrites path Ids inside a `Generics` (param bounds + where predicates).
fn rewrite_generics_ids(
    mut generics: rustdoc_types::Generics,
    id_map: &HashMap<Id, Id>,
) -> rustdoc_types::Generics {
    generics.params =
        generics.params.into_iter().map(|p| rewrite_generic_param_ids(p, id_map)).collect();
    generics.where_predicates = generics
        .where_predicates
        .into_iter()
        .map(|pred| rewrite_where_predicate_ids(pred, id_map))
        .collect();
    generics
}

/// Rewrites path Ids inside a `GenericParamDef`.
fn rewrite_generic_param_ids(
    mut param: GenericParamDef,
    id_map: &HashMap<Id, Id>,
) -> GenericParamDef {
    param.kind = match param.kind {
        GenericParamDefKind::Type { bounds, default, is_synthetic } => {
            let new_bounds = bounds.into_iter().map(|b| rewrite_bound_ids(b, id_map)).collect();
            let new_default = default.map(|ty| rewrite_type_ids(&ty, id_map));
            GenericParamDefKind::Type { bounds: new_bounds, default: new_default, is_synthetic }
        }
        GenericParamDefKind::Const { type_, default } => {
            GenericParamDefKind::Const { type_: rewrite_type_ids(&type_, id_map), default }
        }
        other => other,
    };
    param
}

/// Rewrites path Ids inside a `WherePredicate`.
fn rewrite_where_predicate_ids(pred: WherePredicate, id_map: &HashMap<Id, Id>) -> WherePredicate {
    match pred {
        WherePredicate::BoundPredicate { type_: ty, bounds, generic_params } => {
            let new_ty = rewrite_type_ids(&ty, id_map);
            let new_bounds = bounds.into_iter().map(|b| rewrite_bound_ids(b, id_map)).collect();
            WherePredicate::BoundPredicate { type_: new_ty, bounds: new_bounds, generic_params }
        }
        WherePredicate::EqPredicate { lhs, rhs } => {
            let new_lhs = rewrite_type_ids(&lhs, id_map);
            let new_rhs = match rhs {
                Term::Type(ty) => Term::Type(rewrite_type_ids(&ty, id_map)),
                other => other,
            };
            WherePredicate::EqPredicate { lhs: new_lhs, rhs: new_rhs }
        }
        other => other,
    }
}

/// Creates a root `Module` item for a crate.
pub(super) fn make_root_module_item(root_id: Id, crate_name: String, items: Vec<Id>) -> Item {
    Item {
        id: root_id,
        crate_id: 0,
        name: Some(crate_name),
        span: None,
        visibility: rustdoc_types::Visibility::Public,
        docs: None,
        links: HashMap::new(),
        attrs: vec![],
        deprecation: None,
        inner: ItemEnum::Module(Module { is_crate: true, items, is_stripped: false }),
    }
}
