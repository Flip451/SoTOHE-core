//! Phase1State — mutable accumulator built during Phase 1 S / D construction.

use std::collections::{BTreeMap, HashMap};

use domain::tddd::catalogue_v2::ItemAction;
use rustdoc_types::{Id, Item, ItemKind, ItemSummary};

use super::super::item_kind_from_inner;
use super::child_items::{
    collect_impl_child_ids, copy_non_impl_children_to_d, move_impl_children_to_d,
    patch_impl_for_ids, patch_impl_trait_ids, remove_child_items_from_s,
};

// ---------------------------------------------------------------------------
// Phase 1 state
// ---------------------------------------------------------------------------

/// Mutable state built during Phase 1.
pub(super) struct Phase1State {
    /// Fresh Id counter (Id(0) = root module reserved).
    pub(super) next_id: u32,
    /// item index for S.
    pub(super) s_index: HashMap<Id, Item>,
    /// paths map for S.
    pub(super) s_paths: HashMap<Id, ItemSummary>,
    /// item_actions for S.
    pub(super) s_actions: BTreeMap<Id, ItemAction>,
    /// item index for D.
    pub(super) d_index: HashMap<Id, Item>,
    /// paths map for D.
    pub(super) d_paths: HashMap<Id, ItemSummary>,
    /// short_name → Id for types/traits currently in S.
    pub(super) s_type_name_to_id: BTreeMap<String, Id>,
    /// function_path_string → Id for functions currently in S.
    pub(super) s_fn_path_to_id: BTreeMap<String, Id>,
    /// short_name → Id for types/traits currently in D.
    pub(super) d_type_name_to_id: BTreeMap<String, Id>,
    /// function_path_string → Id for functions in D.
    pub(super) d_fn_path_to_id: BTreeMap<String, Id>,
}

impl Phase1State {
    /// Creates a new `Phase1State`.
    ///
    /// `first_fresh_id` is the first Id value that is safe to allocate without
    /// colliding with any Id already present in B's index.  Pass `b.index.keys().map(|id|
    /// id.0).max().map_or(1, |m| m + 1)` to ensure all fresh Ids are above the
    /// B-side namespace.
    pub(super) fn new(first_fresh_id: u32) -> Self {
        Self {
            next_id: first_fresh_id,
            s_index: HashMap::new(),
            s_paths: HashMap::new(),
            s_actions: BTreeMap::new(),
            d_index: HashMap::new(),
            d_paths: HashMap::new(),
            s_type_name_to_id: BTreeMap::new(),
            s_fn_path_to_id: BTreeMap::new(),
            d_type_name_to_id: BTreeMap::new(),
            d_fn_path_to_id: BTreeMap::new(),
        }
    }

    pub(super) fn alloc_id(&mut self) -> Id {
        let id = Id(self.next_id);
        self.next_id += 1;
        id
    }

    /// Inserts a type/trait item into S with a fresh Id.
    /// Returns the newly allocated Id.
    pub(super) fn insert_s_type(
        &mut self,
        item: Item,
        action: ItemAction,
        path: Option<Vec<String>>,
    ) -> Id {
        let new_id = self.alloc_id();
        let name = item.name.clone().unwrap_or_default();
        let kind = item_kind_from_inner(&item.inner);
        let mut new_item = item;
        new_item.id = new_id;
        self.s_index.insert(new_id, new_item);
        if let Some(p) = path {
            self.s_paths.insert(new_id, ItemSummary { crate_id: 0, path: p, kind });
        }
        self.s_actions.insert(new_id, action);
        if !name.is_empty() {
            self.s_type_name_to_id.insert(name, new_id);
        }
        new_id
    }

    /// Inserts a type/trait item into S at a *specific* Id (for Modify: keep same Id position).
    pub(super) fn insert_s_type_at(&mut self, id: Id, item: Item, action: ItemAction) {
        let name = item.name.clone().unwrap_or_default();
        let mut new_item = item;
        new_item.id = id;
        self.s_index.insert(id, new_item);
        self.s_actions.insert(id, action);
        if !name.is_empty() {
            self.s_type_name_to_id.insert(name, id);
        }
    }

    /// Inserts a function item into S with a fresh Id.
    pub(super) fn insert_s_fn(
        &mut self,
        item: Item,
        fn_path: String,
        action: ItemAction,
        path: Option<Vec<String>>,
    ) -> Id {
        let new_id = self.alloc_id();
        let mut new_item = item;
        new_item.id = new_id;
        self.s_index.insert(new_id, new_item);
        if let Some(p) = path {
            self.s_paths
                .insert(new_id, ItemSummary { crate_id: 0, path: p, kind: ItemKind::Function });
        }
        self.s_actions.insert(new_id, action);
        self.s_fn_path_to_id.insert(fn_path, new_id);
        new_id
    }

    /// Moves a type/trait from S to D.
    ///
    /// Impl blocks (`ItemEnum::Impl`) that belonged to the type — together with
    /// their full subtrees (methods, assoc items) — are transferred to D so that
    /// Phase 2 can evaluate them as `DIntersectC` / `DMinusC` signals for the
    /// deleted type's trait implementations.  Non-impl direct children (fields,
    /// variants) are purged from `s_index` since they have no counterpart in D.
    ///
    /// After all impl blocks are moved to D, their `for_.id` is patched to the
    /// new D-side Id allocated for the parent.  This ensures D is internally
    /// consistent: impl.for_ references an Id that exists in `d_index`.
    ///
    /// Orphan impls — impl blocks that were inserted via the Phase 1 orphan-impl
    /// pass (for types like `TypeAlias` that have no `impls` field) and therefore
    /// are not reachable through `collect_child_ids` — are also detected and moved
    /// to D so that Phase 1.6 does not report spurious `DanglingId` errors.
    pub(super) fn move_type_to_d(&mut self, s_id: Id) {
        if let Some(item) = self.s_index.remove(&s_id) {
            let name = item.name.clone().unwrap_or_default();
            // Determine whether the item is a trait so we patch the correct impl field.
            let is_trait = matches!(item.inner, rustdoc_types::ItemEnum::Trait(_));
            // Collect impl ids that will be moved to D before any removal.
            let impl_ids = collect_impl_child_ids(&item, &self.s_index);
            // Move child impl blocks from S to D before purging other children.
            move_impl_children_to_d(
                &mut self.s_index,
                &mut self.d_index,
                &item,
                &mut self.d_type_name_to_id,
            );
            // Copy non-impl children (fields, variants, methods) to D so that D's parent
            // item keeps internally consistent child-id references.  Then purge from S.
            copy_non_impl_children_to_d(&self.s_index, &mut self.d_index, &item);
            remove_child_items_from_s(&mut self.s_index, &item);
            let new_id = self.alloc_id();
            let mut new_item = item;
            new_item.id = new_id;
            // Capture path summary before removing
            let path_summary = self.s_paths.remove(&s_id);
            self.d_index.insert(new_id, new_item);
            if let Some(ps) = path_summary {
                self.d_paths.insert(new_id, ps);
            }
            if !name.is_empty() {
                self.d_type_name_to_id.insert(name, new_id);
            }
            // Patch impl blocks now in D: update the parent reference from the old
            // S-side or B-side Id to the fresh D-side new_id.
            // - For types (struct/enum): patch `impl.for_.id`.
            // - For traits: patch `impl.trait_.id` (for_ is the implementing type, not the trait).
            if is_trait {
                patch_impl_trait_ids(&mut self.d_index, &impl_ids, new_id);
            } else {
                patch_impl_for_ids(&mut self.d_index, &impl_ids, new_id);
            }

            // Orphan impl handling: move any remaining S-index Impl items whose
            // `for_.id` still references the old s_id (orphan impls not reachable
            // through `collect_child_ids`, e.g. impls for `TypeAlias`).
            if !is_trait {
                let orphan_ids: Vec<Id> = self
                    .s_index
                    .iter()
                    .filter_map(|(&id, item)| {
                        if let rustdoc_types::ItemEnum::Impl(impl_) = &item.inner {
                            if let rustdoc_types::Type::ResolvedPath(p) = &impl_.for_ {
                                if p.id == s_id {
                                    return Some(id);
                                }
                            }
                        }
                        None
                    })
                    .collect();
                for orphan_id in orphan_ids {
                    if let Some(mut orphan) = self.s_index.remove(&orphan_id) {
                        // Patch for_.id to point to the new D-side id.
                        if let rustdoc_types::ItemEnum::Impl(ref mut impl_) = orphan.inner {
                            if let rustdoc_types::Type::ResolvedPath(ref mut p) = impl_.for_ {
                                if p.id == s_id {
                                    p.id = new_id;
                                }
                            }
                        }
                        // Move orphan's method children to d_index as well.
                        let child_ids: Vec<Id> =
                            if let rustdoc_types::ItemEnum::Impl(ref impl_) = orphan.inner {
                                impl_.items.clone()
                            } else {
                                vec![]
                            };
                        for child_id in child_ids {
                            if let Some(child) = self.s_index.remove(&child_id) {
                                self.d_index.entry(child_id).or_insert(child);
                            }
                        }
                        let orphan_new_id = self.alloc_id();
                        let mut patched = orphan;
                        patched.id = orphan_new_id;
                        self.d_index.insert(orphan_new_id, patched);
                        self.s_actions.remove(&orphan_id);
                    }
                }
            }
        }
        self.s_actions.remove(&s_id);
        // Remove from s name map
        self.s_type_name_to_id.retain(|_, v| *v != s_id);
    }

    /// Moves a function from S to D.
    pub(super) fn move_fn_to_d(&mut self, s_id: Id, fn_path: String) {
        if let Some(item) = self.s_index.remove(&s_id) {
            let new_id = self.alloc_id();
            let mut new_item = item;
            new_item.id = new_id;
            let path_summary = self.s_paths.remove(&s_id);
            self.d_index.insert(new_id, new_item);
            if let Some(ps) = path_summary {
                self.d_paths.insert(new_id, ps);
            }
            self.d_fn_path_to_id.insert(fn_path.clone(), new_id);
        }
        self.s_actions.remove(&s_id);
        self.s_fn_path_to_id.remove(&fn_path);
    }

    /// Returns the Id of a type/trait currently in S by short name.
    pub(super) fn s_type_id(&self, name: &str) -> Option<Id> {
        self.s_type_name_to_id.get(name).copied()
    }

    /// Returns the Id of a function currently in S by FunctionPath string.
    pub(super) fn s_fn_id(&self, fn_path: &str) -> Option<Id> {
        self.s_fn_path_to_id.get(fn_path).copied()
    }
}
