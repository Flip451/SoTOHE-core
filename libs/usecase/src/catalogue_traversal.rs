//! Shared catalogue-entry traversal helpers.
//!
//! Both the catalogue-spec-refs verifier (`catalogue_spec_refs.rs`) and the
//! catalogue-spec-signals refresher (`catalogue_spec_signals.rs`) iterate a
//! [`CatalogueDocument`] in the canonical order (types → traits → functions,
//! BTreeMap sorted) and normalize entry keys identically:
//!
//! - Type and trait names use `name.as_str()`.
//! - Function paths use `path.to_string()`.
//!
//! ## Key vs. section_key
//!
//! `key` is the bare entry name/path (e.g. `"Foo"`, `"my_fn"`).  It is used
//! for display purposes and as the `type_name` stored in
//! `<layer>-catalogue-spec-signals.json`.
//!
//! `section_key` is a section-qualified discriminator
//! (`"types:Foo"`, `"traits:Foo"`, `"functions:my_fn"`) that is **globally
//! unique** across all three sections of a single catalogue.  Infrastructure
//! adapters key their per-entry hash maps by `section_key` so that a type
//! and a trait that share the same short name cannot overwrite each other.
//!
//! This module centralises both key-derivation rules so both usecases cannot
//! drift independently.

use domain::tddd::catalogue_v2::CatalogueDocument;
use domain::{InformalGroundRef, SpecRef};

/// A single entry extracted from a [`CatalogueDocument`] in canonical traversal order.
pub struct CatalogueEntryRef<'a> {
    /// Bare entry name (type/trait: `name.as_str()`; function: `path.to_string()`).
    ///
    /// Used for display (error messages, `type_name` in signals documents).
    /// Not globally unique across sections — do not use as a hash-map key when
    /// both types and traits may be iterated together.
    pub key: String,
    /// Section-qualified entry key (`"types:<name>"`, `"traits:<name>"`,
    /// `"functions:<path>"`).
    ///
    /// Globally unique within a single [`CatalogueDocument`] because
    /// `CatalogueDocument` guarantees uniqueness **within** each section and
    /// the section prefix prevents cross-section collisions.  Use this field
    /// when keying `HashMap<String, ContentHash>` per-entry hash maps supplied
    /// by infrastructure adapters.
    pub section_key: String,
    /// The declared action for this entry.
    pub action: domain::tddd::catalogue_v2::roles::ItemAction,
    /// Spec-refs slice for this entry.
    pub spec_refs: &'a [SpecRef],
    /// Informal-grounds slice for this entry.
    pub informal_grounds: &'a [InformalGroundRef],
}

/// Returns an iterator over every entry in `catalogue` in canonical order:
/// types (BTreeMap sorted) → traits (BTreeMap sorted) → functions (BTreeMap sorted).
///
/// Key derivation matches the signal-refresher entry ordering contract:
/// - Types: `TypeName::as_str()` (bare) / `"types:<name>"` (section-qualified)
/// - Traits: `TraitName::as_str()` (bare) / `"traits:<name>"` (section-qualified)
/// - Functions: `FunctionPath::to_string()` (bare) / `"functions:<path>"` (section-qualified)
///
/// # Examples
///
/// ```ignore
/// for entry in iter_catalogue_entries(&catalogue) {
///     println!("{} {:?}", entry.key, entry.action);
/// }
/// ```
pub fn iter_catalogue_entries(
    catalogue: &CatalogueDocument,
) -> impl Iterator<Item = CatalogueEntryRef<'_>> {
    let types = catalogue.types.iter().map(|(name, entry)| CatalogueEntryRef {
        key: name.as_str().to_owned(),
        section_key: format!("types:{}", name.as_str()),
        action: entry.action,
        spec_refs: &entry.spec_refs,
        informal_grounds: &entry.informal_grounds,
    });
    let traits = catalogue.traits.iter().map(|(name, entry)| CatalogueEntryRef {
        key: name.as_str().to_owned(),
        section_key: format!("traits:{}", name.as_str()),
        action: entry.action,
        spec_refs: &entry.spec_refs,
        informal_grounds: &entry.informal_grounds,
    });
    let functions = catalogue.functions.iter().map(|(path, entry)| CatalogueEntryRef {
        key: path.to_string(),
        section_key: format!("functions:{path}"),
        action: entry.action,
        spec_refs: &entry.spec_refs,
        informal_grounds: &entry.informal_grounds,
    });
    types.chain(traits).chain(functions)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]
pub(crate) mod tests {
    use domain::tddd::LayerId;
    use domain::tddd::catalogue_v2::CatalogueDocument;
    use domain::tddd::catalogue_v2::composite::{StructKind, StructShape, TypeKindV2};
    use domain::tddd::catalogue_v2::entries::{FunctionEntry, TraitEntry, TypeEntry};
    use domain::tddd::catalogue_v2::identifiers::{
        CrateName, FunctionName, FunctionPath, ModulePath, TraitName, TypeName, TypeRef,
    };
    use domain::tddd::catalogue_v2::roles::{ContractRole, DataRole, FunctionRole, ItemAction};

    /// Shared test helper: create an empty v3 [`CatalogueDocument`] for `name`.
    ///
    /// Exposed `pub(crate)` so `contract_map_workflow` tests can reuse it without
    /// duplicating the construction knowledge.
    pub(crate) fn empty_v3_doc(name: &str) -> CatalogueDocument {
        let crate_name = CrateName::new(name).unwrap();
        let layer_id = LayerId::try_new(name).unwrap();
        CatalogueDocument::new(3, crate_name, layer_id)
    }

    fn type_entry() -> TypeEntry {
        TypeEntry {
            action: ItemAction::Add,
            role: DataRole::value_object(),
            kind: TypeKindV2::Struct(StructKind::new(
                StructShape::Plain { fields: vec![], has_stripped_fields: false },
                None,
            )),
            methods: vec![],
            module_path: ModulePath::root(),
            docs: None,
            spec_refs: vec![],
            informal_grounds: vec![],
        }
    }

    fn trait_entry() -> TraitEntry {
        TraitEntry {
            action: ItemAction::Add,
            role: ContractRole::SecondaryPort,
            methods: vec![],
            supertrait_bounds: vec![],
            generics: vec![],
            where_predicates: vec![],
            module_path: ModulePath::root(),
            docs: None,
            spec_refs: vec![],
            informal_grounds: vec![],
        }
    }

    fn function_entry() -> FunctionEntry {
        FunctionEntry {
            action: ItemAction::Add,
            role: FunctionRole::FreeFunction,
            params: vec![],
            returns: TypeRef::new("()").unwrap(),
            is_async: false,
            generics: vec![],
            where_predicates: vec![],
            docs: None,
            spec_refs: vec![],
            informal_grounds: vec![],
        }
    }

    fn function_path(crate_name: &str, fn_name: &str) -> FunctionPath {
        FunctionPath::at_root(
            CrateName::new(crate_name).unwrap(),
            FunctionName::new(fn_name).unwrap(),
        )
    }

    // --- Empty catalogue ---

    #[test]
    fn empty_catalogue_yields_no_entries() {
        let doc = empty_v3_doc("domain");
        let entries: Vec<_> = super::iter_catalogue_entries(&doc).collect();
        assert!(entries.is_empty());
    }

    // --- Types-only traversal ---

    #[test]
    fn type_entry_yields_correct_key_and_section_key() {
        let mut doc = empty_v3_doc("domain");
        doc.types.insert(TypeName::new("FooType").unwrap(), type_entry());

        let entries: Vec<_> = super::iter_catalogue_entries(&doc).collect();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].key, "FooType");
        assert_eq!(entries[0].section_key, "types:FooType");
    }

    // --- Traits-only traversal ---

    #[test]
    fn trait_entry_yields_correct_key_and_section_key() {
        let mut doc = empty_v3_doc("domain");
        doc.traits.insert(TraitName::new("FooTrait").unwrap(), trait_entry());

        let entries: Vec<_> = super::iter_catalogue_entries(&doc).collect();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].key, "FooTrait");
        assert_eq!(entries[0].section_key, "traits:FooTrait");
    }

    // --- Functions-only traversal ---

    #[test]
    fn function_entry_yields_correct_key_and_section_key() {
        let mut doc = empty_v3_doc("domain");
        let path = function_path("domain", "my_fn");
        let expected_key = path.to_string(); // "domain::my_fn"
        doc.functions.insert(path, function_entry());

        let entries: Vec<_> = super::iter_catalogue_entries(&doc).collect();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].key, expected_key);
        assert_eq!(entries[0].section_key, format!("functions:{expected_key}"));
    }

    // --- Canonical order: types then traits then functions ---

    #[test]
    fn traversal_order_is_types_then_traits_then_functions() {
        let mut doc = empty_v3_doc("domain");
        doc.types.insert(TypeName::new("AType").unwrap(), type_entry());
        doc.traits.insert(TraitName::new("BTrait").unwrap(), trait_entry());
        doc.functions.insert(function_path("domain", "c_fn"), function_entry());

        let keys: Vec<String> =
            super::iter_catalogue_entries(&doc).map(|e| e.section_key.clone()).collect();
        assert_eq!(keys, vec!["types:AType", "traits:BTrait", "functions:domain::c_fn"]);
    }

    // --- BTreeMap alphabetical order within each section ---

    #[test]
    fn types_are_yielded_in_btreemap_alphabetical_order() {
        let mut doc = empty_v3_doc("domain");
        // Inserted in reverse order; BTreeMap guarantees alphabetical iteration.
        doc.types.insert(TypeName::new("Zebra").unwrap(), type_entry());
        doc.types.insert(TypeName::new("Apple").unwrap(), type_entry());
        doc.types.insert(TypeName::new("Mango").unwrap(), type_entry());

        let keys: Vec<String> = super::iter_catalogue_entries(&doc)
            .filter(|e| e.section_key.starts_with("types:"))
            .map(|e| e.key.clone())
            .collect();
        assert_eq!(keys, vec!["Apple", "Mango", "Zebra"]);
    }

    // --- Cross-section collision: same short name in types and traits ---

    #[test]
    fn same_short_name_in_types_and_traits_gets_distinct_section_keys() {
        let mut doc = empty_v3_doc("domain");
        // "Shared" appears in both types and traits.
        doc.types.insert(TypeName::new("Shared").unwrap(), type_entry());
        doc.traits.insert(TraitName::new("Shared").unwrap(), trait_entry());

        let section_keys: Vec<String> =
            super::iter_catalogue_entries(&doc).map(|e| e.section_key.clone()).collect();

        // Both section_keys must be present and distinct.
        assert!(section_keys.contains(&"types:Shared".to_owned()));
        assert!(section_keys.contains(&"traits:Shared".to_owned()));
        assert_eq!(section_keys.len(), 2);
    }

    // --- action field is threaded through ---

    #[test]
    fn action_field_is_preserved_for_all_entry_kinds() {
        let mut doc = empty_v3_doc("domain");
        let mut te = type_entry();
        te.action = ItemAction::Modify;
        doc.types.insert(TypeName::new("ModType").unwrap(), te);

        let mut tre = trait_entry();
        tre.action = ItemAction::Reference;
        doc.traits.insert(TraitName::new("RefTrait").unwrap(), tre);

        let mut fe = function_entry();
        fe.action = ItemAction::Delete;
        doc.functions.insert(function_path("domain", "del_fn"), fe);

        let entries: Vec<_> = super::iter_catalogue_entries(&doc).collect();
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].action, ItemAction::Modify);
        assert_eq!(entries[1].action, ItemAction::Reference);
        assert_eq!(entries[2].action, ItemAction::Delete);
    }
}
