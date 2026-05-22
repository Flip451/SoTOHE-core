//! Node extraction logic for the baseline-graph renderer (T005).
//!
//! Extracts the 5 fixed node kinds (Decision B-r1) from a `rustdoc_types::Crate` index:
//! - `ItemEnum::Struct` (all 3 forms: UnitStruct / TupleStruct / PlainStruct)
//! - `ItemEnum::Enum`
//! - `ItemEnum::TypeAlias`
//! - `ItemEnum::Trait`
//! - `ItemEnum::Function` (standalone / top-level only — inherent methods and trait
//!   methods are handled by BB / H' decisions respectively)
//!
//! Visibility filter (Decision CC-1):
//! - Top-level entries: `Visibility::Public` only.
//! - Exception: `Visibility::Default` is accepted for trait-associated items (trait methods,
//!   enum variants) when their parent entry is Public. These items are *not* extracted as
//!   top-level nodes here — they are handled by T007 (H decision) and T007/H' decision.
//!   The exception is therefore implicit in the per-item filter: top-level node extraction
//!   only ever looks at `Visibility::Public` items.
//!
//! Function listing range (Decision I):
//! - All `Visibility::Public` Functions from `krate.index` are included by default.
//! - Standalone-only: items whose crate_id == 0 and that are present in `krate.paths`.
//!   Items in `krate.index` but absent from `krate.paths` are likely re-exported or
//!   internal synthetic items; we skip them to stay within the crate's public surface.

use rustdoc_types::{Id, Item, ItemEnum, Visibility};

use domain::tddd::baseline_document::BaselineDocument;

// ---------------------------------------------------------------------------
// Public surface of this module
// ---------------------------------------------------------------------------

/// A node extracted from a `rustdoc_types::Crate` (Decision B-r1).
///
/// Each variant wraps a reference to the originating `BaselineDocument` and the
/// rustdoc `Id` + `Item` for the extracted entry. Lifetime `'a` is bound to the
/// `BaselineDocument` slice passed to [`extract_nodes`].
///
/// Currently consumed by T006-T010 rendering tasks. The `#[allow(dead_code)]` below
/// suppresses premature warnings while those tasks are not yet implemented.
#[derive(Debug)]
#[allow(dead_code)]
pub(super) enum ExtractedNode<'a> {
    /// `ItemEnum::Struct` — all 3 forms (Unit / Tuple / Plain).
    Struct { doc: &'a BaselineDocument, id: Id, item: &'a Item },
    /// `ItemEnum::Enum`.
    Enum { doc: &'a BaselineDocument, id: Id, item: &'a Item },
    /// `ItemEnum::TypeAlias`.
    TypeAlias { doc: &'a BaselineDocument, id: Id, item: &'a Item },
    /// `ItemEnum::Trait`.
    Trait { doc: &'a BaselineDocument, id: Id, item: &'a Item },
    /// `ItemEnum::Function` — standalone / top-level only (Decision B-r1 / I).
    Function { doc: &'a BaselineDocument, id: Id, item: &'a Item },
}

#[allow(dead_code)]
impl<'a> ExtractedNode<'a> {
    /// Return the associated `BaselineDocument`.
    pub(super) fn doc(&self) -> &'a BaselineDocument {
        match self {
            ExtractedNode::Struct { doc, .. }
            | ExtractedNode::Enum { doc, .. }
            | ExtractedNode::TypeAlias { doc, .. }
            | ExtractedNode::Trait { doc, .. }
            | ExtractedNode::Function { doc, .. } => doc,
        }
    }

    /// Return the rustdoc `Id` of the extracted item.
    pub(super) fn id(&self) -> Id {
        match self {
            ExtractedNode::Struct { id, .. }
            | ExtractedNode::Enum { id, .. }
            | ExtractedNode::TypeAlias { id, .. }
            | ExtractedNode::Trait { id, .. }
            | ExtractedNode::Function { id, .. } => *id,
        }
    }

    /// Return a reference to the rustdoc `Item`.
    pub(super) fn item(&self) -> &'a Item {
        match self {
            ExtractedNode::Struct { item, .. }
            | ExtractedNode::Enum { item, .. }
            | ExtractedNode::TypeAlias { item, .. }
            | ExtractedNode::Trait { item, .. }
            | ExtractedNode::Function { item, .. } => item,
        }
    }
}

// ---------------------------------------------------------------------------
// Top-level extraction
// ---------------------------------------------------------------------------

/// Extract all B-r1 nodes (5 kinds) from all baselines that belong to `layer`.
///
/// Items are filtered by:
/// 1. Layer membership: `baseline.layer == *layer`.
/// 2. Crate-local: item's `crate_id == 0` (own-crate items only).
/// 3. Presence in `krate.paths`: ensures the item is a declared top-level path
///    (not a re-export shadow or internal synthetic item).
/// 4. Visibility: `Visibility::Public` only (Decision CC-1).
///    Trait methods and enum variants with `Visibility::Default` are NOT extracted
///    here; they are handled by H / H' decisions (T007).
/// 5. Kind: one of the 5 fixed kinds (Decision B-r1).
/// 6. For `Function` items only: must not be an associated method.
///    Associated methods (belonging to a `Trait` or `Impl` `items` list) appear in
///    `krate.paths` with `kind: ItemKind::Function` and `Visibility::Public`, but they
///    are NOT standalone free functions (Decision B-r1 / I). The method-id set is built
///    from all `Trait.items` and `Impl.items` lists in the crate index before extraction.
///
/// Returns nodes in crate iteration order (HashMap, non-deterministic between runs
/// for the same crate, but deterministic within a single call for a given krate).
/// Callers that need stable ordering should sort the output themselves.
#[allow(dead_code)]
pub(super) fn extract_nodes<'a>(
    baselines: &'a [BaselineDocument],
    layer: &domain::tddd::layer_id::LayerId,
) -> Vec<ExtractedNode<'a>> {
    let mut nodes: Vec<ExtractedNode<'a>> = Vec::new();

    for doc in baselines {
        if doc.layer != *layer {
            continue;
        }
        let krate = &doc.krate;

        // Build the set of all method Ids that belong to a Trait or Impl items list.
        // These are associated methods (inherent or trait), not standalone free functions.
        let method_ids: std::collections::HashSet<Id> = krate
            .index
            .values()
            .flat_map(|item| match &item.inner {
                ItemEnum::Trait(t) => t.items.as_slice(),
                ItemEnum::Impl(i) => i.items.as_slice(),
                _ => &[],
            })
            .copied()
            .collect();

        for (id, item) in &krate.index {
            // Crate-local items only (crate_id == 0 means own crate).
            if item.crate_id != 0 {
                continue;
            }
            // Must be present in krate.paths to qualify as a top-level declared item.
            if !krate.paths.contains_key(id) {
                continue;
            }
            // Visibility filter: Public only (Decision CC-1).
            if !matches!(item.visibility, Visibility::Public) {
                continue;
            }
            // For Function items: skip associated methods (inherent or trait).
            // They appear in krate.paths with ItemKind::Function but are not standalone.
            if matches!(item.inner, ItemEnum::Function(_)) && method_ids.contains(id) {
                continue;
            }
            // Kind filter: 5 fixed kinds (Decision B-r1).
            if let Some(node) = try_make_node(doc, *id, item) {
                nodes.push(node);
            }
        }
    }

    nodes
}

/// Attempt to construct an `ExtractedNode` for the given item.
///
/// Returns `Some` for the 5 supported kinds, `None` for everything else.
fn try_make_node<'a>(
    doc: &'a BaselineDocument,
    id: Id,
    item: &'a Item,
) -> Option<ExtractedNode<'a>> {
    match &item.inner {
        ItemEnum::Struct(_) => Some(ExtractedNode::Struct { doc, id, item }),
        ItemEnum::Enum(_) => Some(ExtractedNode::Enum { doc, id, item }),
        ItemEnum::TypeAlias(_) => Some(ExtractedNode::TypeAlias { doc, id, item }),
        ItemEnum::Trait(_) => Some(ExtractedNode::Trait { doc, id, item }),
        ItemEnum::Function(_) => Some(ExtractedNode::Function { doc, id, item }),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use std::collections::HashMap;

    use domain::tddd::baseline_document::BaselineDocument;
    use domain::tddd::catalogue_v2::identifiers::CrateName;
    use domain::tddd::layer_id::LayerId;
    use rustdoc_types::{
        Crate, Enum, FORMAT_VERSION, FunctionHeader, FunctionSignature, Generics, Id, Impl, Item,
        ItemEnum, ItemKind, ItemSummary, Module, Struct, StructKind, Target, Trait, Type,
        TypeAlias, Visibility,
    };

    use super::*;

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

    fn empty_generics() -> Generics {
        Generics { params: vec![], where_predicates: vec![] }
    }

    fn make_item(id: Id, name: Option<&str>, inner: ItemEnum, vis: Visibility) -> Item {
        Item {
            id,
            crate_id: 0,
            name: name.map(|s| s.to_string()),
            span: None,
            visibility: vis,
            docs: None,
            links: HashMap::new(),
            attrs: vec![],
            deprecation: None,
            inner,
        }
    }

    fn pub_item(id: Id, name: &str, inner: ItemEnum) -> Item {
        make_item(id, Some(name), inner, Visibility::Public)
    }

    fn private_item(id: Id, name: &str, inner: ItemEnum) -> Item {
        make_item(id, Some(name), inner, Visibility::Crate)
    }

    fn default_vis_item(id: Id, name: &str, inner: ItemEnum) -> Item {
        make_item(id, Some(name), inner, Visibility::Default)
    }

    fn struct_inner() -> ItemEnum {
        ItemEnum::Struct(Struct {
            kind: StructKind::Plain { fields: vec![], has_stripped_fields: false },
            generics: empty_generics(),
            impls: vec![],
        })
    }

    fn enum_inner() -> ItemEnum {
        ItemEnum::Enum(Enum {
            generics: empty_generics(),
            variants: vec![],
            impls: vec![],
            has_stripped_variants: false,
        })
    }

    fn type_alias_inner() -> ItemEnum {
        ItemEnum::TypeAlias(TypeAlias {
            type_: rustdoc_types::Type::Primitive("u32".to_string()),
            generics: empty_generics(),
        })
    }

    fn trait_inner() -> ItemEnum {
        ItemEnum::Trait(Trait {
            is_auto: false,
            is_unsafe: false,
            is_dyn_compatible: true,
            items: vec![],
            generics: empty_generics(),
            bounds: vec![],
            implementations: vec![],
        })
    }

    fn function_inner() -> ItemEnum {
        ItemEnum::Function(rustdoc_types::Function {
            sig: FunctionSignature { inputs: vec![], output: None, is_c_variadic: false },
            generics: empty_generics(),
            header: FunctionHeader {
                is_unsafe: false,
                is_const: false,
                is_async: false,
                abi: rustdoc_types::Abi::Rust,
            },
            has_body: true,
        })
    }

    fn module_inner(items: Vec<Id>) -> ItemEnum {
        ItemEnum::Module(Module { is_crate: true, items, is_stripped: false })
    }

    fn make_crate(
        root_id: Id,
        _crate_name: &str,
        index: HashMap<Id, Item>,
        paths: HashMap<Id, ItemSummary>,
    ) -> Crate {
        Crate {
            root: root_id,
            crate_version: None,
            includes_private: false,
            index,
            paths,
            external_crates: HashMap::new(),
            format_version: FORMAT_VERSION,
            target: Target { triple: String::new(), target_features: vec![] },
        }
    }

    fn make_baseline(layer_str: &str, crate_name_str: &str, krate: Crate) -> BaselineDocument {
        BaselineDocument::new(
            LayerId::try_new(layer_str).unwrap(),
            CrateName::new(crate_name_str).unwrap(),
            krate,
        )
    }

    fn item_summary(crate_id: u32, path: Vec<&str>, kind: ItemKind) -> ItemSummary {
        ItemSummary { crate_id, path: path.into_iter().map(|s| s.to_string()).collect(), kind }
    }

    // -----------------------------------------------------------------------
    // T005: 5-kind extraction
    // -----------------------------------------------------------------------

    #[test]
    fn test_extract_nodes_extracts_all_5_kinds_when_public() {
        // Arrange: one crate with one public item of each B-r1 kind.
        let root_id = Id(0);
        let struct_id = Id(1);
        let enum_id = Id(2);
        let alias_id = Id(3);
        let trait_id = Id(4);
        let fn_id = Id(5);

        let all_ids = vec![struct_id, enum_id, alias_id, trait_id, fn_id];

        let mut index = HashMap::new();
        index.insert(root_id, pub_item(root_id, "my_crate", module_inner(all_ids.clone())));
        index.insert(struct_id, pub_item(struct_id, "MyStruct", struct_inner()));
        index.insert(enum_id, pub_item(enum_id, "MyEnum", enum_inner()));
        index.insert(alias_id, pub_item(alias_id, "MyAlias", type_alias_inner()));
        index.insert(trait_id, pub_item(trait_id, "MyTrait", trait_inner()));
        index.insert(fn_id, pub_item(fn_id, "my_fn", function_inner()));

        let mut paths = HashMap::new();
        paths.insert(struct_id, item_summary(0, vec!["my_crate", "MyStruct"], ItemKind::Struct));
        paths.insert(enum_id, item_summary(0, vec!["my_crate", "MyEnum"], ItemKind::Enum));
        paths.insert(alias_id, item_summary(0, vec!["my_crate", "MyAlias"], ItemKind::TypeAlias));
        paths.insert(trait_id, item_summary(0, vec!["my_crate", "MyTrait"], ItemKind::Trait));
        paths.insert(fn_id, item_summary(0, vec!["my_crate", "my_fn"], ItemKind::Function));

        let krate = make_crate(root_id, "my_crate", index, paths);
        let baseline = make_baseline("domain", "my_crate", krate);
        let layer = LayerId::try_new("domain").unwrap();

        // Act
        let baselines = [baseline];
        let nodes = extract_nodes(&baselines, &layer);

        // Assert: exactly 5 nodes extracted (one per kind).
        assert_eq!(nodes.len(), 5, "expected 5 nodes, got {}: {:?}", nodes.len(), nodes);

        // Check each kind is present.
        let has_struct = nodes.iter().any(|n| matches!(n, ExtractedNode::Struct { .. }));
        let has_enum = nodes.iter().any(|n| matches!(n, ExtractedNode::Enum { .. }));
        let has_alias = nodes.iter().any(|n| matches!(n, ExtractedNode::TypeAlias { .. }));
        let has_trait = nodes.iter().any(|n| matches!(n, ExtractedNode::Trait { .. }));
        let has_fn = nodes.iter().any(|n| matches!(n, ExtractedNode::Function { .. }));

        assert!(has_struct, "Struct not extracted");
        assert!(has_enum, "Enum not extracted");
        assert!(has_alias, "TypeAlias not extracted");
        assert!(has_trait, "Trait not extracted");
        assert!(has_fn, "Function not extracted");
    }

    #[test]
    fn test_extract_nodes_excludes_module_items() {
        // Module items must NOT be extracted (they are used for subgraph structure,
        // not as B-r1 nodes — Decision B-r1 excludes Module).
        let root_id = Id(0);
        let mut index = HashMap::new();
        index.insert(root_id, pub_item(root_id, "my_crate", module_inner(vec![])));
        // Module itself has no path entry (crate root), so even if paths were
        // populated it would be filtered by the paths check.
        let krate = make_crate(root_id, "my_crate", index, HashMap::new());
        let baseline = make_baseline("domain", "my_crate", krate);
        let layer = LayerId::try_new("domain").unwrap();

        let baselines = [baseline];
        let nodes = extract_nodes(&baselines, &layer);
        assert!(nodes.is_empty(), "Module items must not be extracted, got {nodes:?}");
    }

    // -----------------------------------------------------------------------
    // T005: visibility filter — Public only
    // -----------------------------------------------------------------------

    #[test]
    fn test_extract_nodes_excludes_crate_visibility() {
        // Visibility::Crate (private to crate) must be excluded (Decision CC-1).
        let root_id = Id(0);
        let struct_id = Id(1);

        let mut index = HashMap::new();
        index.insert(root_id, pub_item(root_id, "my_crate", module_inner(vec![struct_id])));
        index.insert(struct_id, private_item(struct_id, "PrivateStruct", struct_inner()));

        let mut paths = HashMap::new();
        paths.insert(
            struct_id,
            item_summary(0, vec!["my_crate", "PrivateStruct"], ItemKind::Struct),
        );

        let krate = make_crate(root_id, "my_crate", index, paths);
        let baseline = make_baseline("domain", "my_crate", krate);
        let layer = LayerId::try_new("domain").unwrap();

        let baselines = [baseline];
        let nodes = extract_nodes(&baselines, &layer);
        assert!(nodes.is_empty(), "Crate-visibility struct must not be extracted, got {nodes:?}");
    }

    #[test]
    fn test_extract_nodes_excludes_restricted_visibility() {
        // Visibility::Restricted(...) must also be excluded (Decision CC-1).
        let root_id = Id(0);
        let struct_id = Id(1);

        let mut index = HashMap::new();
        index.insert(root_id, pub_item(root_id, "my_crate", module_inner(vec![struct_id])));
        index.insert(
            struct_id,
            make_item(
                struct_id,
                Some("RestrictedStruct"),
                struct_inner(),
                Visibility::Restricted { parent: Id(0), path: "super".to_string() },
            ),
        );

        let mut paths = HashMap::new();
        paths.insert(
            struct_id,
            item_summary(0, vec!["my_crate", "RestrictedStruct"], ItemKind::Struct),
        );

        let krate = make_crate(root_id, "my_crate", index, paths);
        let baseline = make_baseline("domain", "my_crate", krate);
        let layer = LayerId::try_new("domain").unwrap();

        let baselines = [baseline];
        let nodes = extract_nodes(&baselines, &layer);
        assert!(
            nodes.is_empty(),
            "Restricted-visibility struct must not be extracted, got {nodes:?}"
        );
    }

    #[test]
    fn test_extract_nodes_excludes_default_visibility_top_level() {
        // Top-level items with Visibility::Default must NOT be extracted (Decision CC-1).
        // Default is only the exception for trait-associated items and enum variants,
        // which are processed by H / H' decisions — not as top-level B-r1 nodes.
        let root_id = Id(0);
        let struct_id = Id(1);

        let mut index = HashMap::new();
        index.insert(root_id, pub_item(root_id, "my_crate", module_inner(vec![struct_id])));
        index.insert(struct_id, default_vis_item(struct_id, "DefaultStruct", struct_inner()));

        let mut paths = HashMap::new();
        paths.insert(
            struct_id,
            item_summary(0, vec!["my_crate", "DefaultStruct"], ItemKind::Struct),
        );

        let krate = make_crate(root_id, "my_crate", index, paths);
        let baseline = make_baseline("domain", "my_crate", krate);
        let layer = LayerId::try_new("domain").unwrap();

        let baselines = [baseline];
        let nodes = extract_nodes(&baselines, &layer);
        assert!(
            nodes.is_empty(),
            "Default-visibility top-level items must not be extracted (H/H' handle them separately), got {nodes:?}"
        );
    }

    // -----------------------------------------------------------------------
    // T005: Default visibility exception — only for child items (trait methods / variants)
    // -----------------------------------------------------------------------

    #[test]
    fn test_extract_nodes_does_not_extract_trait_method_item_as_top_level() {
        // Trait methods appear as Function items with Visibility::Default in krate.index.
        // They must NOT be extracted as top-level nodes here (handled by H' decision in T007).
        // Verified by: such items do not appear in krate.paths (they are referenced only
        // via the parent Trait.items list), so the paths-presence filter rejects them.
        let root_id = Id(0);
        let trait_id = Id(1);
        let method_id = Id(2);

        let trait_with_method = pub_item(
            trait_id,
            "MyTrait",
            ItemEnum::Trait(Trait {
                is_auto: false,
                is_unsafe: false,
                is_dyn_compatible: true,
                items: vec![method_id],
                generics: empty_generics(),
                bounds: vec![],
                implementations: vec![],
            }),
        );
        let method_item = default_vis_item(method_id, "my_method", function_inner());

        let mut index = HashMap::new();
        index.insert(root_id, pub_item(root_id, "my_crate", module_inner(vec![trait_id])));
        index.insert(trait_id, trait_with_method);
        index.insert(method_id, method_item);

        let mut paths = HashMap::new();
        // Only the Trait itself appears in paths; its method does NOT.
        paths.insert(trait_id, item_summary(0, vec!["my_crate", "MyTrait"], ItemKind::Trait));

        let krate = make_crate(root_id, "my_crate", index, paths);
        let baseline = make_baseline("domain", "my_crate", krate);
        let layer = LayerId::try_new("domain").unwrap();

        let baselines = [baseline];
        let nodes = extract_nodes(&baselines, &layer);

        // Should extract the Trait but NOT the method as a top-level node.
        assert_eq!(nodes.len(), 1, "expected 1 node (the Trait), got {nodes:?}");
        assert!(
            matches!(nodes[0], ExtractedNode::Trait { .. }),
            "expected Trait node, got {:?}",
            nodes[0]
        );
    }

    #[test]
    fn test_extract_nodes_does_not_extract_inherent_method_as_top_level() {
        // Inherent methods appear as Function items with Visibility::Public in krate.index
        // AND in krate.paths (unlike trait methods, which are absent from paths).
        // They must NOT be extracted as top-level B-r1 Function nodes (Decision I /
        // standalone-function requirement). The method-id guard (built from Impl.items)
        // must reject them even when they pass the visibility and paths filters.
        let root_id = Id(0);
        let struct_id = Id(1);
        let impl_id = Id(2);
        let method_id = Id(3);

        // Inherent impl: `impl MyStruct { pub fn my_method(&self) }`
        let impl_item = pub_item(
            impl_id,
            "",
            ItemEnum::Impl(Impl {
                is_unsafe: false,
                generics: empty_generics(),
                provided_trait_methods: vec![],
                trait_: None,
                for_: Type::Primitive("MyStruct".to_string()),
                items: vec![method_id],
                is_negative: false,
                is_synthetic: false,
                blanket_impl: None,
            }),
        );
        let method_item = pub_item(method_id, "my_method", function_inner());

        let mut index = HashMap::new();
        index.insert(root_id, pub_item(root_id, "my_crate", module_inner(vec![struct_id])));
        index.insert(struct_id, pub_item(struct_id, "MyStruct", struct_inner()));
        index.insert(impl_id, impl_item);
        index.insert(method_id, method_item);

        let mut paths = HashMap::new();
        paths.insert(struct_id, item_summary(0, vec!["my_crate", "MyStruct"], ItemKind::Struct));
        // Inherent method is registered in paths with ItemKind::Function and Visibility::Public,
        // as done by catalogue_to_extended_crate_codec. It must still be excluded.
        paths.insert(
            method_id,
            item_summary(0, vec!["my_crate", "MyStruct", "my_method"], ItemKind::Function),
        );

        let krate = make_crate(root_id, "my_crate", index, paths);
        let baseline = make_baseline("domain", "my_crate", krate);
        let layer = LayerId::try_new("domain").unwrap();

        let baselines = [baseline];
        let nodes = extract_nodes(&baselines, &layer);

        // Should extract the Struct but NOT the method as a top-level node.
        assert_eq!(nodes.len(), 1, "expected 1 node (the Struct), got {nodes:?}");
        assert!(
            matches!(nodes[0], ExtractedNode::Struct { .. }),
            "expected Struct node, got {:?}",
            nodes[0]
        );
    }

    // -----------------------------------------------------------------------
    // T005: Function listing range (Decision I) — Public only, default filter-less
    // -----------------------------------------------------------------------

    #[test]
    fn test_extract_nodes_includes_public_functions() {
        // All Visibility::Public functions in krate.paths must be included (Decision I).
        let root_id = Id(0);
        let fn_id = Id(1);

        let mut index = HashMap::new();
        index.insert(root_id, pub_item(root_id, "my_crate", module_inner(vec![fn_id])));
        index.insert(fn_id, pub_item(fn_id, "public_fn", function_inner()));

        let mut paths = HashMap::new();
        paths.insert(fn_id, item_summary(0, vec!["my_crate", "public_fn"], ItemKind::Function));

        let krate = make_crate(root_id, "my_crate", index, paths);
        let baseline = make_baseline("domain", "my_crate", krate);
        let layer = LayerId::try_new("domain").unwrap();

        let baselines = [baseline];
        let nodes = extract_nodes(&baselines, &layer);
        assert_eq!(nodes.len(), 1, "expected 1 Function node");
        assert!(
            matches!(nodes[0], ExtractedNode::Function { .. }),
            "expected Function, got {:?}",
            nodes[0]
        );
    }

    #[test]
    fn test_extract_nodes_excludes_private_function() {
        // Private (Visibility::Crate) functions must be excluded (Decision CC-1).
        let root_id = Id(0);
        let fn_id = Id(1);

        let mut index = HashMap::new();
        index.insert(root_id, pub_item(root_id, "my_crate", module_inner(vec![fn_id])));
        index.insert(fn_id, private_item(fn_id, "private_fn", function_inner()));

        let mut paths = HashMap::new();
        paths.insert(fn_id, item_summary(0, vec!["my_crate", "private_fn"], ItemKind::Function));

        let krate = make_crate(root_id, "my_crate", index, paths);
        let baseline = make_baseline("domain", "my_crate", krate);
        let layer = LayerId::try_new("domain").unwrap();

        let baselines = [baseline];
        let nodes = extract_nodes(&baselines, &layer);
        assert!(nodes.is_empty(), "Private function must not be extracted, got {nodes:?}");
    }

    #[test]
    fn test_extract_nodes_excludes_items_not_in_paths() {
        // Items absent from krate.paths are not standalone top-level items
        // (could be re-exports, internal synthetic items, etc.) and must be skipped.
        let root_id = Id(0);
        let struct_id = Id(1);

        let mut index = HashMap::new();
        index.insert(root_id, pub_item(root_id, "my_crate", module_inner(vec![struct_id])));
        index.insert(struct_id, pub_item(struct_id, "MyStruct", struct_inner()));
        // Intentionally NOT adding struct_id to paths.

        let krate = make_crate(root_id, "my_crate", index, HashMap::new());
        let baseline = make_baseline("domain", "my_crate", krate);
        let layer = LayerId::try_new("domain").unwrap();

        let baselines = [baseline];
        let nodes = extract_nodes(&baselines, &layer);
        assert!(nodes.is_empty(), "Items not in krate.paths must not be extracted, got {nodes:?}");
    }

    #[test]
    fn test_extract_nodes_excludes_external_crate_items() {
        // Items with crate_id != 0 belong to external crates and must be excluded.
        let root_id = Id(0);
        let external_id = Id(1);

        let mut index = HashMap::new();
        index.insert(root_id, pub_item(root_id, "my_crate", module_inner(vec![])));
        // crate_id = 99 means an external crate item.
        index.insert(
            external_id,
            Item {
                id: external_id,
                crate_id: 99,
                name: Some("ExternalStruct".to_string()),
                span: None,
                visibility: Visibility::Public,
                docs: None,
                links: HashMap::new(),
                attrs: vec![],
                deprecation: None,
                inner: struct_inner(),
            },
        );

        let mut paths = HashMap::new();
        paths.insert(
            external_id,
            item_summary(99, vec!["external_crate", "ExternalStruct"], ItemKind::Struct),
        );

        let krate = make_crate(root_id, "my_crate", index, paths);
        let baseline = make_baseline("domain", "my_crate", krate);
        let layer = LayerId::try_new("domain").unwrap();

        let baselines = [baseline];
        let nodes = extract_nodes(&baselines, &layer);
        assert!(nodes.is_empty(), "External crate items must not be extracted, got {nodes:?}");
    }

    // -----------------------------------------------------------------------
    // T005: layer filter
    // -----------------------------------------------------------------------

    #[test]
    fn test_extract_nodes_filters_by_layer() {
        // Baselines from a different layer must not be included.
        let root_id = Id(0);
        let struct_id = Id(1);

        let mut index = HashMap::new();
        index.insert(root_id, pub_item(root_id, "my_crate", module_inner(vec![struct_id])));
        index.insert(struct_id, pub_item(struct_id, "MyStruct", struct_inner()));

        let mut paths = HashMap::new();
        paths.insert(struct_id, item_summary(0, vec!["my_crate", "MyStruct"], ItemKind::Struct));

        let krate = make_crate(root_id, "my_crate", index, paths);
        let baseline = make_baseline("usecase", "my_crate", krate);

        // Query for "domain" layer but baseline is "usecase".
        let layer = LayerId::try_new("domain").unwrap();
        let baselines = [baseline];
        let nodes = extract_nodes(&baselines, &layer);
        assert!(
            nodes.is_empty(),
            "Baselines from a different layer must not be extracted, got {nodes:?}"
        );
    }

    // -----------------------------------------------------------------------
    // T005: ExtractedNode accessor methods
    // -----------------------------------------------------------------------

    #[test]
    fn test_extracted_node_accessors_return_correct_fields() {
        let root_id = Id(0);
        let struct_id = Id(42);

        let mut index = HashMap::new();
        index.insert(root_id, pub_item(root_id, "my_crate", module_inner(vec![struct_id])));
        index.insert(struct_id, pub_item(struct_id, "MyStruct", struct_inner()));

        let mut paths = HashMap::new();
        paths.insert(struct_id, item_summary(0, vec!["my_crate", "MyStruct"], ItemKind::Struct));

        let krate = make_crate(root_id, "my_crate", index, paths);
        let baseline = make_baseline("domain", "my_crate", krate);
        let layer = LayerId::try_new("domain").unwrap();

        let baselines = [baseline];
        let nodes = extract_nodes(&baselines, &layer);
        assert_eq!(nodes.len(), 1);

        let node = &nodes[0];
        assert_eq!(node.id(), struct_id, "id() must return Id(42)");
        assert_eq!(node.item().name.as_deref(), Some("MyStruct"), "item() must return the item");
        // doc() is the baseline itself (we can check its crate_name).
        assert_eq!(node.doc().crate_name.as_str(), "my_crate");
    }

    // -----------------------------------------------------------------------
    // T005: multiple baselines
    // -----------------------------------------------------------------------

    #[test]
    fn test_extract_nodes_aggregates_across_multiple_baselines() {
        // Two baselines for the same layer must both be enumerated.
        let layer_str = "domain";

        // Baseline A: one Struct.
        let root_a = Id(0);
        let struct_a = Id(1);
        let mut index_a = HashMap::new();
        index_a.insert(root_a, pub_item(root_a, "crate_a", module_inner(vec![struct_a])));
        index_a.insert(struct_a, pub_item(struct_a, "StructA", struct_inner()));
        let mut paths_a = HashMap::new();
        paths_a.insert(struct_a, item_summary(0, vec!["crate_a", "StructA"], ItemKind::Struct));
        let krate_a = make_crate(root_a, "crate_a", index_a, paths_a);
        let baseline_a = make_baseline(layer_str, "crate_a", krate_a);

        // Baseline B: one Trait.
        let root_b = Id(0);
        let trait_b = Id(1);
        let mut index_b = HashMap::new();
        index_b.insert(root_b, pub_item(root_b, "crate_b", module_inner(vec![trait_b])));
        index_b.insert(trait_b, pub_item(trait_b, "TraitB", trait_inner()));
        let mut paths_b = HashMap::new();
        paths_b.insert(trait_b, item_summary(0, vec!["crate_b", "TraitB"], ItemKind::Trait));
        let krate_b = make_crate(root_b, "crate_b", index_b, paths_b);
        let baseline_b = make_baseline(layer_str, "crate_b", krate_b);

        let layer = LayerId::try_new(layer_str).unwrap();
        let baselines = [baseline_a, baseline_b];
        let nodes = extract_nodes(&baselines, &layer);

        assert_eq!(nodes.len(), 2, "both baselines must be aggregated, got {nodes:?}");
        let has_struct = nodes.iter().any(|n| matches!(n, ExtractedNode::Struct { .. }));
        let has_trait = nodes.iter().any(|n| matches!(n, ExtractedNode::Trait { .. }));
        assert!(has_struct, "Struct from baseline_a must be present");
        assert!(has_trait, "Trait from baseline_b must be present");
    }
}
