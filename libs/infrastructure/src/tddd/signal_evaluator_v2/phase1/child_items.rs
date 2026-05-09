//! Child-item helpers for Phase 1 S / D construction.
//!
//! Utilities for collecting, copying, remapping, and removing child items
//! (struct fields, enum variants, trait methods, impl blocks) when building
//! S and D from A (catalogue TypeGraph) and B (baseline rustdoc).

use std::collections::{BTreeMap, HashMap};

use rustdoc_types::{Id, Item, ItemEnum, ItemSummary};

use super::state::Phase1State;

// ---------------------------------------------------------------------------
// Child-item collection helpers
// ---------------------------------------------------------------------------

/// Collects all child item Ids referenced by an item.
///
/// Used to copy child items (StructField, Variant, variant-payload fields, etc.)
/// from A's or B's index into S when inserting a parent item.  Returns all Ids
/// that are direct children (callers recurse via `collect_all_subtree_ids`).
///
/// Covers:
/// - `Struct` plain fields, tuple fields, and impl block Ids
/// - `Enum` variant Ids and impl block Ids
/// - `Variant` payload field Ids (`Tuple` and `Struct` kinds)
/// - `Trait` and `Impl` item Ids
pub(super) fn collect_child_ids(item: &Item) -> Vec<Id> {
    match &item.inner {
        ItemEnum::Struct(s) => {
            let mut ids: Vec<Id> = match &s.kind {
                rustdoc_types::StructKind::Plain { fields, .. } => fields.clone(),
                rustdoc_types::StructKind::Tuple(opt_ids) => {
                    opt_ids.iter().filter_map(|opt| *opt).collect()
                }
                rustdoc_types::StructKind::Unit => vec![],
            };
            // Also include impl block Ids so that trait-impl items land in S.
            ids.extend_from_slice(&s.impls);
            ids
        }
        ItemEnum::Enum(e) => {
            let mut ids = e.variants.clone();
            // Also include impl block Ids.
            ids.extend_from_slice(&e.impls);
            ids
        }
        // Enum variant payloads — tuple fields and struct fields.
        ItemEnum::Variant(v) => match &v.kind {
            rustdoc_types::VariantKind::Tuple(opt_ids) => {
                opt_ids.iter().filter_map(|opt| *opt).collect()
            }
            rustdoc_types::VariantKind::Struct { fields, .. } => fields.clone(),
            rustdoc_types::VariantKind::Plain => vec![],
        },
        ItemEnum::Trait(t) => {
            // Include both trait-member items (methods, assoc types) and the
            // implementation ids so that impl blocks for a trait are also moved to D
            // when the trait is deleted.  Without `implementations`, impl blocks
            // would remain in S with `trait_.id` pointing at the deleted trait,
            // causing Phase 1.6 to report a spurious DanglingId error.
            let mut ids = t.items.clone();
            ids.extend_from_slice(&t.implementations);
            ids
        }
        ItemEnum::Impl(i) => i.items.clone(),
        _ => vec![],
    }
}

/// Recursively collects all descendant Ids in an item subtree (direct children
/// and their children, etc.) from `source_index`.
pub(super) fn collect_all_subtree_ids(item: &Item, source_index: &HashMap<Id, Item>) -> Vec<Id> {
    let mut result = Vec::new();
    for child_id in collect_child_ids(item) {
        result.push(child_id);
        if let Some(child) = source_index.get(&child_id) {
            result.extend(collect_all_subtree_ids(child, source_index));
        }
    }
    result
}

// ---------------------------------------------------------------------------
// Child-item remapping helpers
// ---------------------------------------------------------------------------

/// Remaps child-ID references in an item's `inner` according to `id_remap`.
///
/// Only the intra-item structural links (field lists, variant lists, trait item
/// lists, and impl-block id lists) are remapped.
///
/// ## Why `Type::ResolvedPath` ids are NOT remapped here
///
/// All A-sourced (catalogue-derived) `ResolvedPath` ids use the sentinel
/// `Id(UNRESOLVED_CRATE_ID)` for local type references (see `type_ref_parser.rs`).
/// Those markers are resolved to real S-side ids during Phase 1.5 by
/// `resolve_unresolved_in_item`, not here.  External crate refs also use
/// `UNRESOLVED_CRATE_ID` with an external path; they are not remapped.
///
/// B-sourced items keep their original B-side ids for type-field references, which
/// is correct because `copy_b_children_to_s` inserts all B children under their
/// original ids, so the references remain valid within S.
///
/// The only case where a structural parent id needs rewriting is `impl.for_`
/// (for struct/enum parents) and `impl.trait_` (for trait parents), handled
/// separately by `patch_impl_for_ids` and `patch_impl_trait_ids` after insertion.
pub(super) fn remap_child_ids_in_item(mut item: Item, id_remap: &HashMap<Id, Id>) -> Item {
    item.inner = match item.inner {
        ItemEnum::Struct(mut s) => {
            s.kind = match s.kind {
                rustdoc_types::StructKind::Plain { fields, has_stripped_fields } => {
                    let new_fields =
                        fields.into_iter().map(|id| *id_remap.get(&id).unwrap_or(&id)).collect();
                    rustdoc_types::StructKind::Plain { fields: new_fields, has_stripped_fields }
                }
                rustdoc_types::StructKind::Tuple(opt_ids) => {
                    let new_ids = opt_ids
                        .into_iter()
                        .map(|opt| opt.map(|id| *id_remap.get(&id).unwrap_or(&id)))
                        .collect();
                    rustdoc_types::StructKind::Tuple(new_ids)
                }
                other => other,
            };
            // Remap impl block Ids.
            s.impls = s.impls.into_iter().map(|id| *id_remap.get(&id).unwrap_or(&id)).collect();
            ItemEnum::Struct(s)
        }
        ItemEnum::Enum(mut e) => {
            e.variants =
                e.variants.into_iter().map(|id| *id_remap.get(&id).unwrap_or(&id)).collect();
            // Remap impl block Ids.
            e.impls = e.impls.into_iter().map(|id| *id_remap.get(&id).unwrap_or(&id)).collect();
            ItemEnum::Enum(e)
        }
        ItemEnum::Variant(mut v) => {
            // Remap payload field ids inside the variant so that VariantKind::Tuple
            // and VariantKind::Struct payload references are valid within S.
            v.kind = match v.kind {
                rustdoc_types::VariantKind::Tuple(opt_ids) => {
                    let new_ids = opt_ids
                        .into_iter()
                        .map(|opt| opt.map(|id| *id_remap.get(&id).unwrap_or(&id)))
                        .collect();
                    rustdoc_types::VariantKind::Tuple(new_ids)
                }
                rustdoc_types::VariantKind::Struct { fields, has_stripped_fields } => {
                    let new_fields =
                        fields.into_iter().map(|id| *id_remap.get(&id).unwrap_or(&id)).collect();
                    rustdoc_types::VariantKind::Struct { fields: new_fields, has_stripped_fields }
                }
                other => other,
            };
            ItemEnum::Variant(v)
        }
        ItemEnum::Trait(mut t) => {
            t.items = t.items.into_iter().map(|id| *id_remap.get(&id).unwrap_or(&id)).collect();
            // Remap impl-block Ids (external implementors listed in `implementations`).
            t.implementations =
                t.implementations.into_iter().map(|id| *id_remap.get(&id).unwrap_or(&id)).collect();
            ItemEnum::Trait(t)
        }
        ItemEnum::Impl(mut i) => {
            i.items = i.items.into_iter().map(|id| *id_remap.get(&id).unwrap_or(&id)).collect();
            ItemEnum::Impl(i)
        }
        other => other,
    };
    item
}

// ---------------------------------------------------------------------------
// Insert / copy helpers for A-sourced and B-sourced items
// ---------------------------------------------------------------------------

/// Inserts an A-sourced item tree into S, allocating fresh Ids for ALL
/// descendant children and remapping intra-item child-Id references.
pub(super) fn insert_a_item_tree_into_s(
    state: &mut Phase1State,
    root_item: Item,
    action: domain::tddd::catalogue_v2::ItemAction,
    path: Option<Vec<String>>,
    source_index: &HashMap<Id, Item>,
) -> Id {
    // Identify which direct children are impl blocks BEFORE remapping, so we
    // can patch their `for_` / `trait_` ids after the root gets a fresh S id.
    let is_trait = matches!(root_item.inner, ItemEnum::Trait(_));
    let old_impl_ids: Vec<Id> = collect_child_ids(&root_item)
        .into_iter()
        .filter(|id| {
            source_index.get(id).is_some_and(|item| matches!(item.inner, ItemEnum::Impl(_)))
        })
        .collect();

    // Build a complete old→new Id remap for the entire subtree.
    let subtree_ids = collect_all_subtree_ids(&root_item, source_index);
    let id_remap: HashMap<Id, Id> =
        subtree_ids.iter().map(|&old_id| (old_id, state.alloc_id())).collect();

    // Insert all descendant items with their new Ids.
    // Impl blocks inherit the parent's action so that Phase 2 evaluates them with
    // the correct region (e.g. `SMinusCAdd` instead of `SMinusCReference` for impls
    // under an added type).
    insert_remapped_children(state, &root_item, source_index, &id_remap, action);

    // Remap the root item's child references, then insert it with a fresh top-level Id.
    let remapped_root = remap_child_ids_in_item(root_item, &id_remap);
    let new_s_id = state.insert_s_type(remapped_root, action, path);

    // Patch the impl blocks' self-type / trait reference to the fresh S root id.
    // Use the remapped impl ids (each old impl id was reassigned a fresh id).
    let new_impl_ids: Vec<Id> =
        old_impl_ids.iter().filter_map(|id| id_remap.get(id)).copied().collect();
    if is_trait {
        patch_impl_trait_ids(&mut state.s_index, &new_impl_ids, new_s_id);
    } else {
        patch_impl_for_ids(&mut state.s_index, &new_impl_ids, new_s_id);
    }

    new_s_id
}

/// Recursively inserts all children of `item` into `state.s_index` using the
/// provided `id_remap` table.
///
/// `parent_action` is propagated to `state.s_actions` for `Impl` children so that
/// Phase 2 evaluates them with the correct action (e.g. `Add`) rather than
/// defaulting to `Reference`.  Non-impl children (fields, variants, methods) do
/// not appear in Phase 2 identity maps and need no action entry.
pub(super) fn insert_remapped_children(
    state: &mut Phase1State,
    item: &Item,
    source_index: &HashMap<Id, Item>,
    id_remap: &HashMap<Id, Id>,
    parent_action: domain::tddd::catalogue_v2::ItemAction,
) {
    for old_child_id in collect_child_ids(item) {
        if let Some(child) = source_index.get(&old_child_id) {
            let new_child_id = *id_remap.get(&old_child_id).unwrap_or(&old_child_id);
            let remapped_child = remap_child_ids_in_item(child.clone(), id_remap);
            let mut stored = remapped_child;
            stored.id = new_child_id;
            state.s_index.entry(new_child_id).or_insert(stored);
            // Propagate action to impl blocks so Phase 2 uses the correct region.
            if matches!(child.inner, ItemEnum::Impl(_)) {
                state.s_actions.insert(new_child_id, parent_action);
            }
            // Recurse for nested children.
            insert_remapped_children(state, child, source_index, id_remap, parent_action);
        }
    }
}

/// Inserts a B-sourced item tree into S, keeping ALL Ids (including the root) as-is.
///
/// B's items already form a consistent Id space, so no remapping is needed.
/// The top-level item is inserted under its original B-side Id.  This preserves
/// all intra-B cross-references: when a struct field's `Type::ResolvedPath.id`
/// points at another top-level B type, that Id is still valid in S because the
/// target was also inserted under its original B-side Id.  Allocating a fresh
/// S Id for the root (as was done previously) broke cross-references between
/// top-level B types, causing Phase 1.6 to report spurious `DanglingId` errors.
///
/// The `fresh_id_base` invariant established in `phase1_build_s_and_d` ensures
/// that all B-side Ids are strictly below the fresh-Id counter, so they cannot
/// clash with A-sourced items allocated later.
///
/// Impl blocks' `for_` / `trait_` references already contain the correct B-side
/// parent Id, so no patching is needed here (unlike the A-insertion path).
pub(super) fn insert_b_item_tree_into_s(
    state: &mut Phase1State,
    root_item: Item,
    action: domain::tddd::catalogue_v2::ItemAction,
    path: Option<Vec<String>>,
    source_index: &HashMap<Id, Item>,
) -> Id {
    // Copy child items recursively (keeping original child Ids).
    copy_b_children_to_s(state, &root_item, source_index);

    // Insert the top-level item under its original B-side Id.
    let b_id = root_item.id;
    let name = root_item.name.clone().unwrap_or_default();
    let kind = super::super::item_kind_from_inner(&root_item.inner);
    state.s_index.insert(b_id, root_item);
    if let Some(p) = path {
        state.s_paths.insert(b_id, ItemSummary { crate_id: 0, path: p, kind });
    }
    state.s_actions.insert(b_id, action);
    if !name.is_empty() {
        state.s_type_name_to_id.insert(name, b_id);
    }

    b_id
}

/// Copies B-sourced child items into `state.s_index`, keeping their original Ids.
pub(super) fn copy_b_children_to_s(
    state: &mut Phase1State,
    item: &Item,
    source_index: &HashMap<Id, Item>,
) {
    for child_id in collect_child_ids(item) {
        if let std::collections::hash_map::Entry::Vacant(e) = state.s_index.entry(child_id) {
            if let Some(child) = source_index.get(&child_id) {
                e.insert(child.clone());
                // Recurse for nested items (e.g. variants with payloads).
                copy_b_children_to_s(state, child, source_index);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Remove / transfer helpers
// ---------------------------------------------------------------------------

/// Recursively removes all child and descendant items of `item` from `s_index`.
///
/// Copies all non-impl direct and transitive children of `parent` from `s_index`
/// to `d_index` (without removing them from S).
///
/// Called before `remove_child_items_from_s` when moving a type/trait to D so
/// that D's parent item retains internally consistent child-id references.
/// Impl-block children are deliberately excluded here: they are handled by
/// `move_impl_children_to_d` which moves them (not copies) in full.
pub(super) fn copy_non_impl_children_to_d(
    s_index: &HashMap<Id, Item>,
    d_index: &mut HashMap<Id, Item>,
    item: &Item,
) {
    for child_id in collect_child_ids(item) {
        if let Some(child) = s_index.get(&child_id) {
            // Skip impl blocks — they are moved separately.
            if matches!(child.inner, ItemEnum::Impl(_)) {
                continue;
            }
            // Copy this child to D (clone so it remains in S for the removal pass).
            d_index.insert(child_id, child.clone());
            // Recurse for grandchildren.
            copy_non_impl_children_to_d(s_index, d_index, child);
        }
    }
}

/// Called when a type/trait is moved from S to D (Delete) to prevent stale child
/// items (fields, variants, trait methods) from lingering in S and generating
/// spurious Phase 2 signals.
pub(super) fn remove_child_items_from_s(s_index: &mut HashMap<Id, Item>, item: &Item) {
    for child_id in collect_child_ids(item) {
        if let Some(child) = s_index.remove(&child_id) {
            // Recurse to remove grandchildren.
            remove_child_items_from_s(s_index, &child);
        }
    }
}

/// Moves all direct `ItemEnum::Impl` children of `parent` from `s_index` into
/// `d_index`, including each impl's own children (methods, assoc items).
///
/// This ensures that D contains a fully self-consistent impl subtree: the impl
/// node's `items` list references method/assoc-item Ids that are all present in
/// `d_index`, so Phase 2 can traverse D without encountering dangling Id
/// references.
pub(super) fn move_impl_children_to_d(
    s_index: &mut HashMap<Id, Item>,
    d_index: &mut HashMap<Id, Item>,
    parent: &Item,
    _d_type_name_to_id: &mut BTreeMap<String, Id>,
) {
    for child_id in collect_child_ids(parent) {
        if let Some(child) = s_index.get(&child_id) {
            if matches!(child.inner, ItemEnum::Impl(_)) {
                // Clone the impl node so we can inspect its children.
                let child_clone = child.clone();
                // Move the impl's own children (methods, assoc items) from S to D
                // before removing the impl itself.  This keeps the returned D
                // graph internally consistent: impl.items entries all resolve to
                // items present in d_index.
                move_subtree_to_d(s_index, d_index, &child_clone);
                // Now transfer the impl node itself.
                if let Some(impl_item) = s_index.remove(&child_id) {
                    d_index.insert(child_id, impl_item);
                }
            }
        }
    }
}

/// Recursively moves all descendant items of `item` from `s_index` to
/// `d_index`, preserving their existing Ids.
///
/// Called by `move_impl_children_to_d` to transfer method / assoc-item subtrees
/// so that D's impl nodes have fully populated `items` lists.
fn move_subtree_to_d(
    s_index: &mut HashMap<Id, Item>,
    d_index: &mut HashMap<Id, Item>,
    item: &Item,
) {
    for child_id in collect_child_ids(item) {
        if let Some(child) = s_index.remove(&child_id) {
            // Recurse before inserting so inner children are moved first.
            move_subtree_to_d(s_index, d_index, &child);
            d_index.insert(child_id, child);
        }
    }
}

/// Removes all B-side child items of a type being replaced by Modify from `s_index`.
pub(super) fn remove_b_children_from_s(s_index: &mut HashMap<Id, Item>, b_item: &Item) {
    remove_child_items_from_s(s_index, b_item);
}

/// Collects the Ids of direct `ItemEnum::Impl` children of `parent` that are
/// currently present in `s_index`.
///
/// Used by `move_type_to_d` to snapshot which impl ids will be moved before
/// the move operation begins, so their `for_` references can be patched
/// afterwards to point to the parent's fresh D-side Id.
pub(super) fn collect_impl_child_ids(parent: &Item, s_index: &HashMap<Id, Item>) -> Vec<Id> {
    collect_child_ids(parent)
        .into_iter()
        .filter(|id| s_index.get(id).is_some_and(|item| matches!(item.inner, ItemEnum::Impl(_))))
        .collect()
}

/// Patches the `for_.id` field of impl blocks in `index` to `new_parent_id`.
///
/// Used when the parent is a **type** (struct / enum): the impl block's `for_`
/// field records the implementing type.  Called after a parent type is inserted
/// or moved with a fresh Id to fix stale `for_` references.
///
/// Only `Type::ResolvedPath` `for_` values are patched; other shapes
/// (primitives, tuples, etc.) do not carry an Id and are left unchanged.
///
/// Works on both `s_index` (after `insert_b_item_tree_into_s`) and `d_index`
/// (after `move_type_to_d`).
pub(super) fn patch_impl_for_ids(
    index: &mut HashMap<Id, Item>,
    impl_ids: &[Id],
    new_parent_id: Id,
) {
    for impl_id in impl_ids {
        if let Some(item) = index.get_mut(impl_id) {
            if let ItemEnum::Impl(ref mut impl_inner) = item.inner {
                if let rustdoc_types::Type::ResolvedPath(ref mut path) = impl_inner.for_ {
                    path.id = new_parent_id;
                }
            }
        }
    }
}

/// Patches the `trait_.id` field of impl blocks in `index` to `new_trait_id`.
///
/// Used when the parent is a **trait**: the impl block's `trait_` field records
/// the trait being implemented, while `for_` is the implementing type.  Called
/// after a trait is inserted or moved with a fresh Id so that `trait_.id` points
/// to the new scope-local Id rather than the stale B-side or A-side Id.
pub(super) fn patch_impl_trait_ids(
    index: &mut HashMap<Id, Item>,
    impl_ids: &[Id],
    new_trait_id: Id,
) {
    for impl_id in impl_ids {
        if let Some(item) = index.get_mut(impl_id) {
            if let ItemEnum::Impl(ref mut impl_inner) = item.inner {
                if let Some(ref mut trait_path) = impl_inner.trait_ {
                    trait_path.id = new_trait_id;
                }
            }
        }
    }
}

/// Copies A-sourced child items (for Modify) into S at a specific root Id,
/// allocating fresh Ids for all children and remapping child-Id references.
///
/// `parent_action` is forwarded to `insert_remapped_children` so that impl-block
/// children inherit the parent's action rather than defaulting to `Reference`.
pub(super) fn remap_and_copy_a_children_to_s(
    state: &mut Phase1State,
    root_item: &Item,
    source_index: &HashMap<Id, Item>,
    parent_action: domain::tddd::catalogue_v2::ItemAction,
) -> Item {
    let subtree_ids = collect_all_subtree_ids(root_item, source_index);
    let id_remap: HashMap<Id, Id> =
        subtree_ids.iter().map(|&old_id| (old_id, state.alloc_id())).collect();
    insert_remapped_children(state, root_item, source_index, &id_remap, parent_action);
    remap_child_ids_in_item(root_item.clone(), &id_remap)
}
