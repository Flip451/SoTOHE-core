//! `ExtendedCrate` — TypeGraph A / S representation.
//!
//! `ExtendedCrate` wraps a `rustdoc_types::Crate` (the internal TypeGraph
//! representation for B / C) and extends it with a per-Item action map
//! (`item_actions: BTreeMap<Id, ItemAction>`).
//!
//! ## Design (ADR 2 D2)
//!
//! | Graph | Type | Note |
//! |---|---|---|
//! | A (Catalogue-derived) | `ExtendedCrate` | Built by the Catalogue → A codec |
//! | S (merge intermediate) | `ExtendedCrate` | Built by Signal evaluator Phase 1 |
//! | B (Baseline) | `rustdoc_types::Crate` | Pure rustdoc output |
//! | C (Current) | `rustdoc_types::Crate` | Pure rustdoc output |
//! | D (Delete-set) | `rustdoc_types::Crate` | Implicit action = Delete; no `item_actions` needed |
//!
//! `item_actions` maps `rustdoc_types::Id` → `ItemAction`. After Phase 1
//! B-side Id renumbering, ALL items in S — including children (fields, variants,
//! impl methods) — have an explicit entry in `item_actions` (with the appropriate
//! action: `Reference` for B-sourced items, `Add`/`Modify` for A-sourced items).
//! Previously, B-derived Reference entries were absent and callers defaulted to
//! `Reference`; that default remains correct for any absent key, but in practice
//! every S-item now has an explicit entry in the map.
//!
//! No serde derives — per ADR `knowledge/adr/2026-04-14-1531-domain-serde-ripout.md`,
//! the domain layer is serialization-free.

use std::collections::{BTreeMap, HashMap};

use rustdoc_types::{Crate, Id};

use crate::tddd::catalogue_v2::ItemAction;

/// TypeGraph A / S: a `rustdoc_types::Crate` extended with per-Item actions.
///
/// `krate` holds the full rustdoc-compatible item index (types, traits,
/// functions, impls, …). `item_actions` maps each item's `Id` to its declared
/// `ItemAction` from the originating `CatalogueDocument`.
///
/// Items **absent** from `item_actions` should be interpreted by callers as
/// implicitly `Reference`.  After Phase 1 B-side Id renumbering, all items in
/// TypeGraph S — including B-derived Reference entries and all child items —
/// have an explicit `item_actions` entry.  The implicit-Reference default is
/// kept for backward compatibility but is no longer exercised by Phase 1 output.
///
/// ## Invariants
///
/// * All `Id` keys in `item_actions` SHOULD exist in `krate.index`.  The
///   codec that constructs `ExtendedCrate` is responsible for maintaining this
///   invariant.
/// * In TypeGraph S (Phase 1 output), `ItemAction::Delete` entries are removed
///   from `krate.index` and moved to the companion `DeleteSet`.  Therefore a
///   valid S will never have `ItemAction::Delete` in `item_actions`.
#[derive(Debug, Clone)]
pub struct ExtendedCrate {
    /// The inner rustdoc-compatible crate graph.
    krate: Crate,
    /// Per-Item action map from the originating catalogue document.
    item_actions: BTreeMap<Id, ItemAction>,
    /// Orphan-impl ownership map: trait-impl `Id` → owning type `Id`.
    ///
    /// Populated by the catalogue codec for trait-impl items whose `for_` type
    /// is external (i.e. `impl MyTrait for external_crate::ExternalType`).
    /// In these cases the orphan-impl pass in Phase 1 cannot infer the parent
    /// type action from `impl.for_.id` (which points to a synthetic external id),
    /// so it uses this map as a fallback to find the owning local type and inherit
    /// its `ItemAction`.
    ///
    /// Empty for S-graph (Phase 1 output) and for B-/C-sourced crates.
    impl_owner_map: HashMap<Id, Id>,
}

impl ExtendedCrate {
    /// Constructs a new `ExtendedCrate`.
    ///
    /// # Errors
    ///
    /// This constructor is infallible; validation (e.g., that every key in
    /// `item_actions` exists in `krate.index`) is the codec's responsibility.
    #[must_use]
    pub fn new(krate: Crate, item_actions: BTreeMap<Id, ItemAction>) -> Self {
        Self { krate, item_actions, impl_owner_map: HashMap::new() }
    }

    /// Attaches an orphan-impl ownership map and returns `self`.
    ///
    /// Used by the catalogue codec to record `impl_id → owning_type_id` for
    /// trait-impl items whose `for_` type is external.  The Phase 1 orphan-impl
    /// pass uses this map as a fallback when `impl.for_.id` does not appear in
    /// `a_item_actions` (because it points to a synthetic external id rather
    /// than the owning local type).
    #[must_use]
    pub fn with_impl_owner_map(mut self, map: HashMap<Id, Id>) -> Self {
        self.impl_owner_map = map;
        self
    }

    /// Returns a reference to the inner `rustdoc_types::Crate`.
    #[must_use]
    pub fn krate(&self) -> &Crate {
        &self.krate
    }

    /// Returns a reference to the per-Item action map.
    #[must_use]
    pub fn item_actions(&self) -> &BTreeMap<Id, ItemAction> {
        &self.item_actions
    }

    /// Returns a reference to the orphan-impl ownership map (`impl_id → owning_type_id`).
    ///
    /// Empty unless the catalogue codec explicitly set it via `with_impl_owner_map`.
    #[must_use]
    pub fn impl_owner_map(&self) -> &HashMap<Id, Id> {
        &self.impl_owner_map
    }

    /// Looks up the action for a given `Id`.
    ///
    /// Returns `None` when the id is absent from `item_actions`.  Callers
    /// should interpret an absent entry as an implicit `Reference` action.
    /// Note: for TypeGraph S produced by Phase 1, every item has an explicit
    /// entry, so `None` is not expected in practice for S-graph lookups.
    ///
    /// `Id` implements `Copy` so this method accepts `&Id` for `BTreeMap::get`
    /// compatibility.
    #[must_use]
    pub fn action_for(&self, id: &Id) -> Option<ItemAction> {
        self.item_actions.get(id).copied()
    }

    /// Consumes `self` and returns `(krate, item_actions)`.
    ///
    /// Note: callers that need the orphan-impl ownership map must call
    /// `impl_owner_map()` before invoking this method, or use
    /// `into_all_parts()` to decompose all three fields at once.
    #[must_use]
    pub fn into_parts(self) -> (Crate, BTreeMap<Id, ItemAction>) {
        (self.krate, self.item_actions)
    }

    /// Consumes `self` and returns `(krate, item_actions, impl_owner_map)`.
    ///
    /// Unlike `into_parts()`, this method preserves the orphan-impl ownership
    /// map, making round-tripping lossless for callers that need all three fields.
    #[must_use]
    pub fn into_all_parts(self) -> (Crate, BTreeMap<Id, ItemAction>, HashMap<Id, Id>) {
        (self.krate, self.item_actions, self.impl_owner_map)
    }
}

// ---------------------------------------------------------------------------
// Tests — AC-04
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use std::collections::{BTreeMap, HashMap};

    use rustdoc_types::{Crate, FORMAT_VERSION, Id, Item, ItemEnum, Module};

    use super::*;

    /// Build a minimal `rustdoc_types::Crate` for testing.
    fn empty_krate() -> Crate {
        Crate {
            root: Id(0),
            crate_version: None,
            includes_private: false,
            index: HashMap::new(),
            paths: HashMap::new(),
            external_crates: HashMap::new(),
            format_version: FORMAT_VERSION,
            // target is present in rustdoc-types 0.57.3 — empty triple for tests
            target: rustdoc_types::Target { triple: String::new(), target_features: vec![] },
        }
    }

    fn root_item(raw_id: u32) -> (Id, Item) {
        let id = Id(raw_id);
        let item = Item {
            id,
            crate_id: 0,
            name: Some("root".to_string()),
            span: None,
            visibility: rustdoc_types::Visibility::Public,
            docs: None,
            links: HashMap::new(),
            attrs: vec![],
            deprecation: None,
            inner: ItemEnum::Module(Module { is_crate: true, items: vec![], is_stripped: false }),
        };
        (id, item)
    }

    #[test]
    fn test_extended_crate_new_stores_krate_and_actions() {
        let krate = empty_krate();
        let mut actions = BTreeMap::new();
        let id = Id(1);
        actions.insert(id, ItemAction::Add);

        let ec = ExtendedCrate::new(krate, actions);
        assert_eq!(ec.item_actions().len(), 1);
        assert_eq!(ec.action_for(&id), Some(ItemAction::Add));
    }

    #[test]
    fn test_extended_crate_action_for_absent_id_returns_none() {
        let krate = empty_krate();
        let ec = ExtendedCrate::new(krate, BTreeMap::new());
        let missing = Id(999);
        assert_eq!(ec.action_for(&missing), None);
    }

    #[test]
    fn test_extended_crate_krate_ref_accessible() {
        let krate = empty_krate();
        let ec = ExtendedCrate::new(krate, BTreeMap::new());
        assert_eq!(ec.krate().format_version, FORMAT_VERSION);
    }

    #[test]
    fn test_extended_crate_into_parts_roundtrip() {
        let krate = empty_krate();
        let mut actions = BTreeMap::new();
        let id = Id(2);
        actions.insert(id, ItemAction::Modify);

        let ec = ExtendedCrate::new(krate, actions);
        let (recovered_krate, recovered_actions) = ec.into_parts();
        assert_eq!(recovered_krate.format_version, FORMAT_VERSION);
        assert_eq!(recovered_actions.get(&id), Some(&ItemAction::Modify));
    }

    #[test]
    fn test_extended_crate_all_item_actions_roundtrip() {
        // Verify all 4 ItemAction variants can be stored and retrieved.
        let krate = empty_krate();
        let mut actions = BTreeMap::new();
        actions.insert(Id(1), ItemAction::Add);
        actions.insert(Id(2), ItemAction::Modify);
        actions.insert(Id(3), ItemAction::Reference);
        actions.insert(Id(4), ItemAction::Delete);

        let ec = ExtendedCrate::new(krate, actions);
        assert_eq!(ec.action_for(&Id(1)), Some(ItemAction::Add));
        assert_eq!(ec.action_for(&Id(2)), Some(ItemAction::Modify));
        assert_eq!(ec.action_for(&Id(3)), Some(ItemAction::Reference));
        assert_eq!(ec.action_for(&Id(4)), Some(ItemAction::Delete));
    }

    #[test]
    fn test_extended_crate_item_in_krate_index_accessible_via_krate() {
        let mut krate = empty_krate();
        let (id, item) = root_item(0);
        krate.index.insert(id, item);

        let ec = ExtendedCrate::new(krate, BTreeMap::new());
        assert!(ec.krate().index.contains_key(&id));
    }

    #[test]
    fn test_extended_crate_clone_produces_independent_instance() {
        let krate = empty_krate();
        let mut actions = BTreeMap::new();
        let id = Id(1);
        actions.insert(id, ItemAction::Add);

        let ec1 = ExtendedCrate::new(krate, actions);
        let ec2 = ec1.clone();
        // Both instances should have the same action for the id.
        assert_eq!(ec1.action_for(&id), Some(ItemAction::Add));
        assert_eq!(ec2.action_for(&id), Some(ItemAction::Add));
    }

    #[test]
    fn test_extended_crate_impl_owner_map_default_is_empty() {
        let krate = empty_krate();
        let ec = ExtendedCrate::new(krate, BTreeMap::new());
        assert!(ec.impl_owner_map().is_empty());
    }

    #[test]
    fn test_extended_crate_with_impl_owner_map_stores_map() {
        let krate = empty_krate();
        let impl_id = Id(10);
        let owner_id = Id(20);
        let mut owner_map = HashMap::new();
        owner_map.insert(impl_id, owner_id);

        let ec = ExtendedCrate::new(krate, BTreeMap::new()).with_impl_owner_map(owner_map);

        assert_eq!(ec.impl_owner_map().get(&impl_id), Some(&owner_id));
        assert_eq!(ec.impl_owner_map().len(), 1);
    }

    #[test]
    fn test_extended_crate_into_all_parts_roundtrip() {
        let krate = empty_krate();
        let mut actions = BTreeMap::new();
        let action_id = Id(3);
        actions.insert(action_id, ItemAction::Add);

        let impl_id = Id(10);
        let owner_id = Id(20);
        let mut owner_map = HashMap::new();
        owner_map.insert(impl_id, owner_id);

        let ec = ExtendedCrate::new(krate, actions).with_impl_owner_map(owner_map);
        let (recovered_krate, recovered_actions, recovered_owner_map) = ec.into_all_parts();

        assert_eq!(recovered_krate.format_version, FORMAT_VERSION);
        assert_eq!(recovered_actions.get(&action_id), Some(&ItemAction::Add));
        assert_eq!(recovered_owner_map.get(&impl_id), Some(&owner_id));
    }
}
