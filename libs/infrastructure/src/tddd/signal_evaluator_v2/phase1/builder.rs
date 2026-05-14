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
    patch_impl_trait_ids, remap_and_copy_a_children_to_s, remap_child_ids_in_item,
    remove_b_children_from_s,
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
    // initial allocations do not clash with B-side Ids.  After T037 we renumber
    // ALL B items into fresh S Ids, but we still need the counter to start above
    // the B range so that the pre-built b_id_remap Ids are all fresh.
    let first_fresh_id = b.index.keys().map(|id| id.0).max().map_or(1, |m| m + 1);
    let mut state = Phase1State::new(first_fresh_id);

    // --- Pre-step: Build B-wide Id remap (T037) ---
    //
    // Allocate a fresh S Id for every entry in b.index BEFORE any insertion so
    // that both A-sourced and B-sourced items occupy the same "fresh" Id space
    // in s_index, with no overlap.  `insert_b_item_tree_into_s` and the B-function
    // / orphan-impl insertion passes use this map to place every B item at its
    // pre-allocated fresh S Id.
    //
    // `Id(0)` is excluded from the remap for two reasons:
    //   1. It is the B-side root module, which is never inserted into S (S gets
    //      its own fresh root module item at the end of Phase 1).
    //   2. Rustdoc uses `Id(0)` as a `Self`-type sentinel inside impl blocks
    //      (`impl_.for_` for primitive / self-referential types).  Phase 1.6's
    //      dangling-Id check explicitly skips `Id(0)` as a sentinel (Rule 0:
    //      `if referenced_id.0 == 0 { continue; }`).  Remapping `Id(0)` would
    //      turn that sentinel into a fresh S id that is never inserted into
    //      `s_index`, causing Phase 1.6 to report spurious `DanglingId` errors.
    {
        // Sort B-side Ids before allocating fresh S Ids so that `b_id_remap` is
        // deterministic across runs.  `HashMap::keys()` has no guaranteed iteration
        // order; iterating it directly would assign different fresh Ids to the same
        // B-side key on each invocation, making Phase 1 output non-reproducible.
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
    // For each B function, insert into S at the pre-allocated b_id_remap Id.
    // The function's signature type-refs are rewritten via b_id_remap so that
    // B-side parameter / return type Ids are consistent with the renumbered
    // B-side types in s_index.
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
    // cannot reach.  Phase 2's `build_impl_identity_map` does find them in C (walking the
    // whole index), so omitting them from S would produce spurious `CMinusSUnionD` signals.
    //
    // After T037, all B items are renumbered via b_id_remap, so original B Ids are NEVER in
    // s_index.  Check whether the REMAPPED id is already present (i.e. was inserted as a
    // child of a type's `impls` list) rather than the raw B-side id.
    {
        let orphan_impl_ids: Vec<Id> = b
            .index
            .keys()
            .filter(|id| {
                b.index.get(*id).is_some_and(|item| {
                    item.crate_id == 0 && matches!(item.inner, ItemEnum::Impl(_))
                }) && {
                    // The remapped S id is absent from s_index → this impl was not inserted
                    // as part of any type's child subtree → it is an orphan impl.
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
                // Rewrite the impl's type-level refs via b_id_remap.
                let rewritten = rewrite_type_ref_ids_in_item(impl_item.clone(), &state.b_id_remap);
                // Remap structural child-list references (impl.items).
                let remapped = remap_child_ids_in_item(rewritten, &state.b_id_remap);
                let mut stored_impl = remapped;
                stored_impl.id = new_impl_s_id;
                state.s_index.insert(new_impl_s_id, stored_impl);
                state.s_actions.insert(new_impl_s_id, ItemAction::Reference);
                // Insert the impl's direct children (methods, assoc items) with renumbered Ids.
                if let ItemEnum::Impl(impl_inner) = &impl_item.inner {
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
                    remove_b_children_from_s(
                        &mut state.s_index,
                        &mut state.s_actions,
                        &b_item_in_s,
                    );
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

    // --- Step 5.5: A-side orphan-impl pass (Symptom B fix, T038 / IN-31) ---
    //
    // Some A-side types (notably `TypeAlias`) have no `impls` field in `rustdoc_types`,
    // so trait-impl items generated by `encode_type_alias` / `encode_trait_impl_blocks`
    // are recorded as standalone entries in `a_krate.index` and are NOT reached by
    // the A type-processing loop above (which traverses parents via `collect_child_ids`
    // → `Struct.impls` / `Enum.impls` / `Trait.implementations`).  Phase 2's
    // `build_impl_identity_map` does find them in C (walking the whole index), so
    // omitting them from S for the `Add` case would produce spurious `CMinusSUnionD`
    // (Red) signals.
    //
    // Symmetric to the B-side orphan-impl pass earlier in this function.  Detection
    // rule: an `Impl` item in `a_krate.index` is "orphan" iff it is NOT in the
    // `impls` / `implementations` list of any `Struct` / `Enum` / `Trait` in
    // `a_krate.index`.  Apply the same Id-renumber rule as T037: insert via
    // `insert_a_item_tree_into_s` so the orphan and its method/assoc-item children
    // get fresh S Ids, action propagates through the subtree, and Phase 1.45
    // rewrites the impl's `for_.id` / `trait_.id` via the standard A-side remap.
    //
    // Action coverage: this pass inserts only when the implementing type's
    // catalogue action is `Add`.  Other actions are intentionally skipped:
    //
    // - `Reference`: the implementing type exists in B unchanged; B's rustdoc emits
    //   the same trait-impls, which the earlier B-side orphan-impl pass already
    //   inserted into S with Reference action.  Inserting again from A would
    //   duplicate the impl in S.
    // - `Delete`: the B-side orphan-impl pass inserts B's impl into S, and
    //   `state.move_type_to_d` (called for the parent type in Step 4/5) scans
    //   `s_index` for impls whose `for_.id` points at the deleted parent and
    //   moves them to D.  Inserting A's mirror would corrupt this hand-off.
    // - `Modify`: TypeAlias with modified trait-impls is a rare edge case where
    //   B's old impls (already in S via the B-side pass) provide a baseline that
    //   Phase 2 can match against.  Replacing them with A's new impls is a
    //   follow-up task; the current pass leaves them as B-sourced Reference.
    {
        // Collect impl Ids that are reachable via A-side type/trait subtree traversal
        // in Step 4.  An impl is "reachable" (not orphan) iff it is listed in the
        // `impls` field of a Struct or Enum (which `collect_child_ids` always follows),
        // OR it is listed in `Trait.implementations` for a Trait whose own action is
        // `Add` or `Modify` (only those trait subtrees are traversed in Step 4;
        // `Reference` and `Delete` traits do not cause A-side child insertion).
        //
        // Impls listed in `Trait.implementations` for a `Reference` trait are NOT
        // excluded: the Reference-trait subtree is never traversed from A, so a new
        // TypeAlias impl that appears in that trait's `implementations` list would
        // otherwise be silently skipped by this pass.
        let a_referenced_impl_ids: HashSet<Id> = a_krate
            .index
            .iter()
            .flat_map(|(id, item)| match &item.inner {
                ItemEnum::Struct(s) => s.impls.clone(),
                ItemEnum::Enum(e) => e.impls.clone(),
                ItemEnum::Trait(t) => {
                    // Only exclude trait-listed impls when the trait itself is
                    // being inserted by the A-side loop (Add or Modify).  For
                    // Reference / Delete traits, Step 4 does not traverse their
                    // subtree, so impls in `implementations` may be genuinely
                    // orphan from A's processing perspective.
                    let action = a_item_actions.get(id).copied();
                    if matches!(action, Some(ItemAction::Add) | Some(ItemAction::Modify)) {
                        t.implementations.clone()
                    } else {
                        vec![]
                    }
                }
                _ => vec![],
            })
            .collect();

        let a_orphan_impls: Vec<Id> = a_krate
            .index
            .iter()
            .filter(|(id, item)| {
                item.crate_id == 0
                    && matches!(item.inner, ItemEnum::Impl(_))
                    && !a_referenced_impl_ids.contains(id)
            })
            .map(|(id, _)| *id)
            .collect();

        for orphan_id in a_orphan_impls {
            let impl_item = match a_krate.index.get(&orphan_id) {
                Some(it) => it.clone(),
                None => continue,
            };

            // Inherit action from the implementing type via `impl.for_.id` → `a_item_actions`.
            let parent_action = if let ItemEnum::Impl(impl_) = &impl_item.inner {
                if let Type::ResolvedPath(p) = &impl_.for_ {
                    a_item_actions.get(&p.id).copied()
                } else {
                    None
                }
            } else {
                None
            };

            // Only `Add` is inserted by this pass; see the comment block above for why
            // Reference / Delete / Modify are handled elsewhere (or deferred).
            if parent_action != Some(ItemAction::Add) {
                continue;
            }

            let path = a_krate.paths.get(&orphan_id).map(|ps| ps.path.clone());
            insert_a_item_tree_into_s(&mut state, impl_item, ItemAction::Add, path, &a_krate.index);
        }
    }

    // --- Phase 1.45: A-side type-ref id remapping (local + external) ---
    //
    // ## Problem
    //
    // `type_ref_parser::parse_type_ref` encodes local catalogue types to A-side item Ids
    // via `resolve_local`, and external types to A-side synthetic Ids via
    // `ensure_external_type_id`.  Both kinds of A-side Ids are embedded directly in
    // `Type::ResolvedPath.id` and are NOT remapped to S-side (Phase1-fresh) Ids during
    // item insertion (only structural child-list Ids are remapped in
    // `remap_child_ids_in_item`).
    //
    // These small A-side Ids (allocated sequentially from 0 inside the codec) can collide
    // with B-side Ids when both are placed in the same `s_paths` / `s_index` map:
    //
    // - **Local collision (pre-existing fix)**: A-side local Id N references a local type
    //   whose S-side Id is M (different from N).  Phase 1.6 would incorrectly validate
    //   N as "in S" by finding an unrelated B-side item at N.
    //
    // - **External collision (new fix)**: A-side synthetic external Id N (e.g. Id(13) for
    //   `core::cmp::Eq`) coincides numerically with a B-side external Id N (e.g. B's Id(65)
    //   for `core::cmp::PartialEq`).  When both A-sourced and B-sourced items reference Id N
    //   as their impl trait, Step 6 inserts A's path entry at N (overwriting B's), so B-side
    //   trait impls resolve through A's wrong `external_crates` entry and generate incorrect
    //   identity keys in `build_impl_identity_map`.
    //
    // ## Fix
    //
    // Build a comprehensive A-id → fresh-Phase1-id remapping that covers BOTH local and
    // external A-side Ids, then apply `rewrite_type_ref_ids_in_item` to all A-sourced items
    // in S.  After this phase:
    //
    // - All type-ref Ids in A-sourced items are >= `first_fresh_id` (Phase1-allocated).
    // - External trait path entries from A-sourced impl blocks are pre-inserted into
    //   `s_paths` at their fresh Phase1 Ids with the correct A-side crate_id (to be
    //   re-numbered by `patch_paths_crate_ids_extra` in Step 6).
    // - Step 6 only needs Vacant-entry insertion for B-side paths — no force-overwrite,
    //   no namespace collision.
    //
    // The rewrite is safe because Phase 2 uses short-name identity and `format_type`
    // (L1 resolution, not `ResolvedPath.id`) for structural equality; the only consumers
    // of these Ids are Phase 1.6's dangling-Id check and Step 6's path-copy loop.
    {
        // Build A-id → S-id map for LOCAL types:
        // For each A-side local item (crate_id == 0), look up its short name in
        // `s_type_name_to_id` to find the S-side Id (which may differ from the A-side Id).
        let a_local_to_s_id: HashMap<Id, Id> = a_krate
            .paths
            .iter()
            .filter_map(|(&a_id, a_ps)| {
                if a_ps.crate_id != 0 {
                    return None; // handled separately below
                }
                let name = a_ps.path.last()?.as_str();
                let s_id = state.s_type_name_to_id.get(name)?;
                Some((a_id, *s_id))
            })
            .collect();

        // Build A-id → fresh-Phase1-id map for EXTERNAL type-refs:
        // For ALL A-side external path entries (crate_id != 0), allocate a fresh Phase1 Id
        // and pre-insert the path entry at the fresh Id so Step 6's Vacant-entry insertion
        // finds it already populated.
        //
        // A IDs and Phase-1 fresh S IDs are independent — the catalogue codec allocates its
        // own sequential counter for A while Phase-1 uses a separate counter.  There is no
        // numeric relationship between the two spaces and we must not filter by `< first_fresh_id`:
        // any A external ID (regardless of value) may collide with a freshly allocated S item Id.
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
            // Only rewrite A-sourced items (Add / Modify actions) and their child items.
            // B-sourced Reference items have already had their intra-B type refs rewritten
            // via b_id_remap during insertion (T037); re-applying the A-side full_remap
            // would incorrectly remap B-sourced type refs to A-side Ids.
            //
            // After T037, ALL items in S have an entry in s_actions (top-level + children).
            // The `s_actions` lookup is the sole authoritative discriminator.
            // After T037, ALL items in S (top-level + children) have an entry in
            // s_actions because:
            //   - `insert_remapped_children` now propagates action to every child.
            //   - `insert_remapped_children_with_type_rewrite` does the same for B children.
            //   - `insert_s_fn` / `insert_s_fn_at` populate s_actions for functions.
            //
            // The old `None => item_id.0 >= first_fresh_id` heuristic was needed when
            // B-side children had no s_actions entry and were kept at their original B Ids
            // (< first_fresh_id).  After T037, B-side children are renumbered to fresh Ids
            // just like A-side children, so the numeric heuristic would wrongly identify
            // renumbered B children as A-sourced.  s_actions is now the sole discriminator.
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
    // After T037, both A- and B-sourced items are renumbered into the same fresh S Id space.
    // A-side type refs (after Phase 1.45) and B-side internal type refs (after b_id_remap
    // rewriting in `insert_b_item_tree_into_s`) both resolve to fresh S Ids in `s_index`.
    // B-side external type refs (not in b.index, so not in b_id_remap) keep their original
    // B-side Id values and are validated against b.paths.
    //
    // The item-source discriminator uses `s_actions` exclusively (no id-based heuristic).
    //
    // A-SOURCED items (Add/Modify in s_actions):
    //   Type ref Ids are Phase-1.45-remapped S Ids.
    //   A-side paths are consulted first, B-side paths as fallback.
    //   0. Id(0) — sentinel — skip.
    //   1. In S → valid.
    //   2a. In `a_krate.paths` with crate_id != 0 → A-side external → valid.
    //   2b. In `a_krate.paths` with crate_id == 0 → A-side local not remapped → DanglingId.
    //   3. In `b.paths` with crate_id != 0 → B-side external (possible for Modify) → valid.
    //   4. In `b.paths` with crate_id == 0 → deleted local → DanglingId.
    //   5. Not found → stale → DanglingId.
    //
    // B-SOURCED items (Reference in s_actions):
    //   Internal B-to-B type refs have been rewritten to fresh S Ids (Rule 1 covers them).
    //   External B type refs (not in b.index) keep their original B-side Ids and are
    //   validated against b.paths.
    //   A-side paths are NOT consulted to avoid false positives.
    //   0. Id(0) — sentinel — skip.
    //   1. In S → valid (covers renumbered internal B refs).
    //   2. In `b.paths` with crate_id != 0 → B-side external → valid.
    //   3. In `b.paths` with crate_id == 0 → deleted local → DanglingId.
    //   4. Not found → stale → DanglingId.
    //
    // `collect_referenced_ids` pre-filters the Id list so that `Id(UNRESOLVED_CRATE_ID)` is
    // never yielded (A-side external type refs that were not resolved to a concrete external Id
    // keep `UNRESOLVED_CRATE_ID` after Phase 1.5 and are explicitly skipped by the collector).

    let s_ids: HashSet<Id> = state.s_index.keys().copied().collect();

    for item in state.s_index.values() {
        let item_name = item.name.clone().unwrap_or_else(|| format!("<id:{}>", item.id.0));

        // Determine whether this item is A-sourced.
        //
        // After T037, ALL items in S (top-level + children) have an entry in
        // `s_actions` — see the Phase 1.45 discriminator comment for details.
        // The `s_actions` lookup is therefore the sole authoritative discriminator.
        //
        // A-sourced: Add / Modify action.
        // B-sourced: Reference action.
        let item_is_a_sourced = matches!(
            state.s_actions.get(&item.id),
            Some(&ItemAction::Add) | Some(&ItemAction::Modify),
        );

        for referenced_id in collect_referenced_ids(item) {
            // Rule 0: Id(0) is the crate root module / `Self` sentinel — skip unconditionally.
            if referenced_id.0 == 0 {
                continue;
            }
            // Rule 1: id is in S — valid regardless of source.
            if s_ids.contains(&referenced_id) || state.s_paths.contains_key(&referenced_id) {
                continue;
            }

            if item_is_a_sourced {
                // A-sourced item: type ref Ids are A-side Ids (or S-side after remapping).
                // Check A-side paths before B-side to correctly handle A-side synthetic
                // external Ids (e.g. `std::result::Result`) that may coincidentally collide
                // with B-side local Ids (e.g. a constant `AGENT_PROFILES_PATH`).
                if let Some(a_ps) = a_krate.paths.get(&referenced_id) {
                    if a_ps.crate_id != 0 {
                        // Rule 2a: A-side external-crate item — valid cross-crate ref.
                        continue;
                    }
                    // Rule 2b: A-side local id not remapped by Phase 1.45 (type deleted or
                    // a function Id — functions are not tracked in s_type_name_to_id).
                    let a_name = a_ps.path.last().map(|s| s.as_str()).unwrap_or("<unknown>");
                    return Err(Phase1Error::DanglingId(format!(
                        "{item_name} -> Id({}) is an A-side local ref to '{}' which is not present in S \
                         (the type may have been deleted or was never declared in the catalogue)",
                        referenced_id.0, a_name
                    )));
                }
                // Fallthrough: not in A-side paths; check B-side (possible for Modify items
                // that have not had every type ref remapped, or synthetic impl items).
                if let Some(ps) = b.paths.get(&referenced_id) {
                    if ps.crate_id != 0 {
                        // Rule 3: B-side external-crate item — valid.
                        continue;
                    }
                    // Rule 4: B-side local deleted.
                    return Err(Phase1Error::DanglingId(format!(
                        "{item_name} -> Id({}) not found in S (may have been deleted in Phase 1)",
                        referenced_id.0
                    )));
                }
                // Rule 5: stale or unknown.
                return Err(Phase1Error::DanglingId(format!(
                    "{item_name} -> Id({}) is not in S and not in A or B paths (stale reference or unknown id)",
                    referenced_id.0
                )));
            } else {
                // B-sourced item: type ref Ids are B-side Ids.  Do NOT consult A-side paths —
                // A-side local function Ids may collide with B-side external type Ids, causing
                // false-positive DanglingId errors.
                if let Some(ps) = b.paths.get(&referenced_id) {
                    if ps.crate_id != 0 {
                        // Rule 2: B-side external-crate item — valid.
                        continue;
                    }
                    // Rule 3: B-side local deleted.
                    return Err(Phase1Error::DanglingId(format!(
                        "{item_name} -> Id({}) not found in S (may have been deleted in Phase 1)",
                        referenced_id.0
                    )));
                }
                // Rule 4: stale or unknown.
                return Err(Phase1Error::DanglingId(format!(
                    "{item_name} -> Id({}) is not in S and not in B paths (stale reference or unknown id)",
                    referenced_id.0
                )));
            }
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
    //
    // After Phase 1.45, all A-sourced type-ref IDs (both local and external) have been
    // remapped to fresh Phase1 IDs (>= first_fresh_id) and A-side external path entries are
    // already in s_paths.  We only need to copy B-side external paths using Vacant-entry
    // insertion — no force-overwrite needed.
    let a_side_path_ids: HashSet<Id> = {
        // Collect all Ids referenced by items in S (types + impls + functions).
        //
        // Separate A-sourced from B-sourced references using s_actions as the sole
        // discriminator (after T037, all items including children have s_actions entries).
        //
        // This distinction is critical because A and B use independent Id spaces.
        // After Phase 1.45, A-sourced type-ref IDs no longer collide with B-side IDs,
        // so Vacant-entry insertion is safe for all path entries.
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

        // Copy external path summaries for referenced Ids into s_paths.
        //
        // A-side external paths: remapped to fresh IDs in Phase 1.45 and already in s_paths.
        // Any remaining A-referenced IDs that have A-side external path entries are type-refs
        // in field/generic/return types; insert them using Vacant-entry insertion.
        //
        // B-side external paths: inserted for B-referenced Ids using Vacant-entry insertion.
        //
        // Track which Ids were inserted from A so `patch_paths_crate_ids_extra` (not
        // `patch_paths_crate_ids`) handles their crate_id renumbering.  A and B use
        // independent `external_crates` numbering, so their crate_id values for the same
        // crate name (e.g. "core" = 1 in A, "core" = 2 in B) must not be mixed.
        let mut a_side_path_ids: HashSet<Id> = HashSet::new();

        // Ids already in s_paths from Phase 1.45 (A-side remapped external IDs) are tracked
        // by collecting them now so patch_paths_crate_ids_extra handles them.
        for (&id, ps) in &state.s_paths {
            if ps.crate_id != 0 {
                // Only A-side paths were inserted into s_paths by Phase 1.45 so far.
                a_side_path_ids.insert(id);
            }
        }

        // Insert A-side paths for A-referenced Ids (type-refs in field/method signatures).
        // These are IDs from `collect_referenced_ids` on A-sourced items.
        // After Phase 1.45, impl trait IDs and all external type-ref IDs are already handled;
        // remaining refs come from ResolvedPath type refs not covered by Phase 1.45 (e.g.
        // A-side local paths that survived without remapping).
        // Use Vacant-entry insertion: these IDs are fresh (from Phase 1.45 remapping) or
        // only appear in A's path table (no collision with s_paths after Phase 1.45).
        for &ref_id in &a_referenced_ids {
            // Skip IDs already in s_paths (from Phase 1.45 or previous loop iterations).
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
            // A-side does not have an external path for this Id: fall back to B's path table.
            if let std::collections::hash_map::Entry::Vacant(e) = state.s_paths.entry(ref_id) {
                if let Some(ps) = b.paths.get(&ref_id) {
                    if ps.crate_id != 0 {
                        e.insert(ps.clone());
                    }
                }
            }
        }

        // Insert B-side paths for B-referenced Ids (using B's path table only).
        for &ref_id in &b_referenced_ids {
            if let std::collections::hash_map::Entry::Vacant(e) = state.s_paths.entry(ref_id) {
                if let Some(ps) = b.paths.get(&ref_id) {
                    if ps.crate_id != 0 {
                        e.insert(ps.clone());
                    }
                }
            }
        }

        // Store the A-side id set for use after the scope block ends.
        a_side_path_ids
    };
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
    // not present in b.paths; pass Some(&a_krate) and Some(&a_side_path_ids) so those
    // paths entries are resolved via a_krate.external_crates rather than b.external_crates
    // (the two tables use independent crate_id namespaces and must not be mixed).
    let (s_external_crates, s_name_to_new_id) = build_external_crates_for_scope(
        &state.s_index,
        &state.s_paths,
        b,
        Some(&a_krate),
        Some(&a_side_path_ids),
    );
    // Patch A-side path entries first (using A's external_crates numbering), then patch
    // B-side path entries (using B's external_crates numbering), explicitly excluding
    // A-side ids from the B-side pass.  This prevents mis-mapping A-sourced crate_ids
    // through B's table: A and B use independent external_crates numbering, so the same
    // numeric crate_id may refer to different crates in each table.
    patch_paths_crate_ids_extra(
        &mut state.s_paths,
        &a_krate,
        &s_name_to_new_id,
        Some(&a_side_path_ids),
    );
    patch_paths_crate_ids(&mut state.s_paths, b, &s_name_to_new_id, Some(&a_side_path_ids));

    // D contains only B-sourced items; no A-side ids remain, so extra_a = None.
    let (d_external_crates, d_name_to_new_id) =
        build_external_crates_for_scope(&state.d_index, &state.d_paths, b, None, None);
    patch_paths_crate_ids(&mut state.d_paths, b, &d_name_to_new_id, None);

    // Allocate fresh Phase1-managed Ids for the S and D root modules before
    // consuming any fields of `state`.
    //
    // Both ids must be allocated here — before `state.s_actions` is partially
    // moved into `ExtendedCrate::new` — because `alloc_id` takes `&mut self`
    // (borrowing `state`) and cannot be called after a partial move.
    //
    // Use fresh ids rather than hardcoding Id(0) for the root module.  After
    // T037, B items are renumbered via `b_id_remap` (excluding `Id(0)`, which
    // is the sentinel / root).  A fresh root id keeps the S root distinct from
    // all renumbered B-side items and from all A-side items.
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
    // (d_root_id was pre-allocated above before the partial move of state.s_actions.)
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

// ---------------------------------------------------------------------------
// A-side type-ref id remapping (Phase 1.45)
// ---------------------------------------------------------------------------

/// Rewrites all `Type::ResolvedPath.id` (and nested path Ids in generic args, bounds,
/// etc.) within `item` using `id_map`.  Ids not present in the map are left unchanged.
///
/// This is used in Phase 1.45 to replace A-side local type Ids with their S-side
/// equivalents after all A items have been inserted into S, and in B-side insertion
/// (T037) to replace B-side type refs with the corresponding fresh S Ids from
/// `b_id_remap`.
pub(super) fn rewrite_type_ref_ids_in_item(mut item: Item, id_map: &HashMap<Id, Id>) -> Item {
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
