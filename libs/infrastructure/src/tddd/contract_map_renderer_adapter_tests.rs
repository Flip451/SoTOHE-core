use std::io::Write;

use domain::tddd::catalogue_v2::composite::TypeKindV2;
use domain::tddd::catalogue_v2::entries::{FunctionEntry, TraitEntry, TypeEntry};
use domain::tddd::catalogue_v2::identifiers::TypeRef;
use domain::tddd::catalogue_v2::roles::{ContractRole, DataRole, FunctionRole, ItemAction};
use domain::tddd::catalogue_v2::{
    CatalogueDocument, CrateName, FunctionName, FunctionPath, ModulePath, TraitName, TypeName,
};
use domain::tddd::{
    ContractMapRenderOptions, ContractMapRenderer, ContractMapRendererError, LayerId,
};
use tempfile::TempDir;

use super::{
    ContractMapRendererAdapter, catalogues_to_nodes, function_node_id, trait_node_id, type_node_id,
};

// ---------------------------------------------------------------------------
// Helper factories
// ---------------------------------------------------------------------------

fn layer(s: &str) -> LayerId {
    LayerId::try_new(s).unwrap()
}

fn type_name(s: &str) -> TypeName {
    TypeName::new(s).unwrap()
}

fn trait_name(s: &str) -> TraitName {
    TraitName::new(s).unwrap()
}

fn make_minimal_catalogue(layer_id: &str, crate_name: &str) -> CatalogueDocument {
    CatalogueDocument::new(3, CrateName::new(crate_name).unwrap(), layer(layer_id))
}

fn make_type_entry() -> TypeEntry {
    TypeEntry {
        action: ItemAction::Add,
        role: DataRole::ValueObject,
        kind: TypeKindV2::PlainStruct {
            fields: vec![],
            has_stripped_fields: false,
            typestate: None,
        },
        methods: vec![],
        trait_impls: vec![],
        module_path: ModulePath::root(),
        docs: None,
        spec_refs: vec![],
        informal_grounds: vec![],
    }
}

fn make_trait_entry() -> TraitEntry {
    TraitEntry {
        action: ItemAction::Add,
        role: ContractRole::SecondaryPort,
        methods: vec![],
        supertrait_bounds: vec![],
        module_path: ModulePath::root(),
        docs: None,
        spec_refs: vec![],
        informal_grounds: vec![],
    }
}

fn make_function_entry() -> FunctionEntry {
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

fn write_style_config(dir: &TempDir, content: &str) -> std::path::PathBuf {
    let path = dir.path().join("contract-map-style.toml");
    let mut f = std::fs::File::create(&path).unwrap();
    f.write_all(content.as_bytes()).unwrap();
    path
}

fn valid_toml_content() -> &'static str {
    // Minimal valid TOML that satisfies the StyleConfig schema.
    r#"
[filter]
include_function_roles = []
"#
}

// ---------------------------------------------------------------------------
// Test 1: absent config → StyleConfigNotFound
// ---------------------------------------------------------------------------

#[test]
fn test_render_with_absent_config_returns_style_config_not_found() {
    let dir = TempDir::new().unwrap();
    let nonexistent_path = dir.path().join("does-not-exist.toml");
    let adapter = ContractMapRendererAdapter::new(nonexistent_path.clone());
    let catalogues: Vec<CatalogueDocument> = vec![];
    let layer_order: Vec<LayerId> = vec![];
    let opts = ContractMapRenderOptions::empty();

    let result = adapter.render(&catalogues, &layer_order, &opts);
    assert!(
        matches!(
            result,
            Err(ContractMapRendererError::StyleConfigNotFound { ref path })
            if path == &nonexistent_path
        ),
        "expected StyleConfigNotFound, got: {result:?}"
    );
}

// ---------------------------------------------------------------------------
// Test 2: malformed TOML → StyleConfigParse
// ---------------------------------------------------------------------------

#[test]
fn test_render_with_malformed_toml_returns_style_config_parse() {
    let dir = TempDir::new().unwrap();
    let path = write_style_config(&dir, "this is [not valid toml {{{{");
    let adapter = ContractMapRendererAdapter::new(path.clone());
    let catalogues: Vec<CatalogueDocument> = vec![];
    let layer_order: Vec<LayerId> = vec![];
    let opts = ContractMapRenderOptions::empty();

    let result = adapter.render(&catalogues, &layer_order, &opts);
    assert!(
        matches!(
            result,
            Err(ContractMapRendererError::StyleConfigParse { path: ref p, reason: ref r })
            if *p == path && !r.is_empty()
        ),
        "expected StyleConfigParse, got: {result:?}"
    );
}

// ---------------------------------------------------------------------------
// Test 3: valid config → render returns Ok (scaffold placeholder)
// ---------------------------------------------------------------------------

#[test]
fn test_render_with_valid_config_returns_ok() {
    let dir = TempDir::new().unwrap();
    let path = write_style_config(&dir, valid_toml_content());
    let adapter = ContractMapRendererAdapter::new(path);
    let catalogues: Vec<CatalogueDocument> = vec![];
    let layer_order: Vec<LayerId> = vec![];
    let opts = ContractMapRenderOptions::empty();

    let result = adapter.render(&catalogues, &layer_order, &opts);
    assert!(result.is_ok(), "expected Ok, got: {result:?}");
}

// ---------------------------------------------------------------------------
// Test 4: node_id scheme — same name Type vs Trait produces different ids
// ---------------------------------------------------------------------------

#[test]
fn test_node_id_scheme_same_name_type_and_trait_produce_different_ids() {
    let layer_id = layer("domain");
    let name = "UserRepository";
    let t_id = type_node_id(&layer_id, &type_name(name));
    let r_id = trait_node_id(&layer_id, &trait_name(name));

    assert_ne!(
        t_id, r_id,
        "type_node_id and trait_node_id must not collide for the same name and layer"
    );
    assert!(t_id.starts_with('T'), "type id must start with 'T': {t_id}");
    assert!(r_id.starts_with('R'), "trait id must start with 'R': {r_id}");
}

// ---------------------------------------------------------------------------
// Test 5: node_id format verification — T, R, F prefixes + structure
// ---------------------------------------------------------------------------

#[test]
fn test_type_node_id_format_matches_spec() {
    // T<len>_<sanitized_layer>_<sanitized_name>
    // layer="domain", name="UserEmail"
    // len = unsanitized name char count = len("UserEmail") = 9
    // body = "domain_UserEmail"
    // expected: "T9_domain_UserEmail"
    let id = type_node_id(&layer("domain"), &type_name("UserEmail"));
    assert_eq!(id, "T9_domain_UserEmail");
}

#[test]
fn test_trait_node_id_format_matches_spec() {
    // R<len>_<sanitized_layer>_<sanitized_name>
    // layer="domain", name="UserEmail"
    // len = unsanitized name char count = len("UserEmail") = 9
    // body = "domain_UserEmail"
    // expected: "R9_domain_UserEmail"
    let id = trait_node_id(&layer("domain"), &trait_name("UserEmail"));
    assert_eq!(id, "R9_domain_UserEmail");
}

#[test]
fn test_function_node_id_format_matches_spec() {
    // F<len>_<sanitized_layer>_<sanitized_full_path>
    // layer="domain", path = crate_name="domain", module=["tddd"], name="register"
    // full_path_raw = path.to_string() = "domain::tddd::register" (22 chars)
    // sanitize("domain::tddd::register"): alpha stays, ':' → '_'
    //   → "domain__tddd__register"
    // sl = sanitize("domain") = "domain"
    // body = "domain_domain__tddd__register"
    // expected: "F22_domain_domain__tddd__register"
    let crate_name = CrateName::new("domain").unwrap();
    let module_path = ModulePath::from_segments(vec!["tddd".to_string()]).unwrap();
    let fn_name = FunctionName::new("register").unwrap();
    let path = FunctionPath::new(crate_name, module_path, fn_name);
    let id = function_node_id(&layer("domain"), &path);
    assert_eq!(id, "F22_domain_domain__tddd__register");
}

// ---------------------------------------------------------------------------
// Test 6: sanitize replaces non-alphanumeric chars with '_'
// ---------------------------------------------------------------------------

#[test]
fn test_type_node_id_sanitizes_hyphens_in_layer_name() {
    // layer names can contain hyphens (e.g. "my-layer")
    // sanitize("my-layer"): 'my' kept; '-' → '_'; 'layer' kept → "my_layer"
    // len = unsanitized name char count = len("FooBar") = 6
    // body = "my_layer_FooBar"
    let id = type_node_id(&layer("my-layer"), &type_name("FooBar"));
    assert_eq!(id, "T6_my_layer_FooBar");
}

// ---------------------------------------------------------------------------
// Test 7 (optional): catalogues_to_nodes preserves layer/crate/module context
// ---------------------------------------------------------------------------

#[test]
fn test_catalogues_to_nodes_preserves_layer_and_doc_context() {
    let mut doc = make_minimal_catalogue("domain", "mylib");
    doc.types.insert(type_name("Foo"), make_type_entry());
    doc.traits.insert(trait_name("Bar"), make_trait_entry());
    let fn_path =
        FunctionPath::at_root(CrateName::new("mylib").unwrap(), FunctionName::new("baz").unwrap());
    doc.functions.insert(fn_path.clone(), make_function_entry());

    let catalogues = vec![doc];
    let nodes = catalogues_to_nodes(&catalogues);

    assert_eq!(nodes.len(), 3, "expected 3 nodes (1 type + 1 trait + 1 function)");

    // All nodes must reference the "domain" layer.
    for node in &nodes {
        let node_layer = match node {
            super::CatalogueNode::Type { layer, .. } => layer,
            super::CatalogueNode::Trait { layer, .. } => layer,
            super::CatalogueNode::Function { layer, .. } => layer,
        };
        assert_eq!(node_layer.as_ref(), "domain");
    }
}

// ---------------------------------------------------------------------------
// Test 8: no collision across multiple entries in same layer
// ---------------------------------------------------------------------------

#[test]
fn test_node_id_no_collision_type_vs_trait_across_layers() {
    // Ensure that different layers also produce different IDs.
    let id_d = type_node_id(&layer("domain"), &type_name("MyType"));
    let id_u = type_node_id(&layer("usecase"), &type_name("MyType"));
    assert_ne!(id_d, id_u);
}

// ---------------------------------------------------------------------------
// Test 9: function_node_id — cross-crate collision prevention
// ---------------------------------------------------------------------------

#[test]
fn test_function_node_id_no_collision_same_module_and_name_different_crates() {
    // Two functions with the same module path and function name in different crates
    // within the same architecture layer must produce different node ids because
    // crate_name is included in the Display form used for full_path_raw.
    let module_path = ModulePath::from_segments(vec!["utils".to_string()]).unwrap();
    let fn_name = FunctionName::new("helper").unwrap();

    let path_a =
        FunctionPath::new(CrateName::new("crate_a").unwrap(), module_path.clone(), fn_name.clone());
    let path_b = FunctionPath::new(CrateName::new("crate_b").unwrap(), module_path, fn_name);

    let id_a = function_node_id(&layer("domain"), &path_a);
    let id_b = function_node_id(&layer("domain"), &path_b);

    assert_ne!(
        id_a, id_b,
        "function_node_id must differ for same module/name in different crates: \
         id_a={id_a}, id_b={id_b}"
    );
}

// ---------------------------------------------------------------------------
// Test 10: function_node_id — component-boundary ambiguity prevention
// ---------------------------------------------------------------------------

#[test]
fn test_function_node_id_no_collision_on_underscore_boundary_ambiguity() {
    // Paths that share the same underscore-joined string but differ in component
    // boundaries must produce different node ids. The Display form uses "::" as
    // separators, which sanitize to "__" (two underscores), while a literal "_"
    // inside a component name sanitizes to a single "_".
    // - `crate::b_c::d` → Display: "crate::b_c::d" → sanitized: "crate__b_c__d"
    // - `crate::b::c_d` → Display: "crate::b::c_d" → sanitized: "crate__b__c_d"
    let crate_name_a = CrateName::new("crate").unwrap();
    let path_a = FunctionPath::new(
        crate_name_a,
        ModulePath::from_segments(vec!["b_c".to_string()]).unwrap(),
        FunctionName::new("d").unwrap(),
    );

    let crate_name_b = CrateName::new("crate").unwrap();
    let path_b = FunctionPath::new(
        crate_name_b,
        ModulePath::from_segments(vec!["b".to_string()]).unwrap(),
        FunctionName::new("c_d").unwrap(),
    );

    let id_a = function_node_id(&layer("domain"), &path_a);
    let id_b = function_node_id(&layer("domain"), &path_b);

    assert_ne!(
        id_a, id_b,
        "function_node_id must differ for paths whose components differ only in \
         underscore boundary placement: id_a={id_a}, id_b={id_b}"
    );
}
