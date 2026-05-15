use std::io::Write;

use domain::tddd::catalogue_v2::composite::TypeKindV2;
use domain::tddd::catalogue_v2::entries::{FunctionEntry, TraitEntry, TypeEntry};
use domain::tddd::catalogue_v2::identifiers::{FieldName, MethodName, ParamName, TypeRef};
use domain::tddd::catalogue_v2::methods::{MethodDeclaration, ParamDeclaration};
use domain::tddd::catalogue_v2::roles::{ContractRole, DataRole, FunctionRole, ItemAction};
use domain::tddd::catalogue_v2::variants::FieldDecl;
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

fn crate_name(s: &str) -> CrateName {
    CrateName::new(s).unwrap()
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

/// Full style config with the edge sections needed for T006 edge rendering tests.
fn full_toml_content() -> &'static str {
    r#"
[edge.method_param]
arrow = "--o"

[edge.method_returns]
arrow = "-->"

[edge.field]
arrow = "--o"

[role.ValueObject]
class = "valueObject"

[role.SecondaryPort]
class = "secondaryPort"

[role.FreeFunction]
class = "freeFunction"

[node.Method]
shape = "round"
class = "methodNode"

[node.Function]
shape = "subroutine"
class = "functionNode"

[filter]
include_function_roles = []
"#
}

/// Renders using the full style config and returns the mermaid string.
fn render_with_full_style(catalogues: &[CatalogueDocument], layer_order: &[LayerId]) -> String {
    let dir = TempDir::new().unwrap();
    let path = write_style_config(&dir, full_toml_content());
    let adapter = ContractMapRendererAdapter::new(path);
    let opts = ContractMapRenderOptions::empty();
    let result = adapter.render(catalogues, layer_order, &opts).unwrap();
    result.into_string()
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
    let cn = crate_name("mylib");
    let name = "UserRepository";
    let t_id = type_node_id(&layer_id, &cn, &type_name(name));
    let r_id = trait_node_id(&layer_id, &cn, &trait_name(name));

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
    // T<len>_<sanitized_layer>_<sanitized_crate>_<sanitized_name>
    // layer="domain", crate_name="mylib", name="UserEmail"
    // len = unsanitized name char count = len("UserEmail") = 9
    // body = "domain_mylib_UserEmail"
    // expected: "T9_domain_mylib_UserEmail"
    let id = type_node_id(&layer("domain"), &crate_name("mylib"), &type_name("UserEmail"));
    assert_eq!(id, "T9_domain_mylib_UserEmail");
}

#[test]
fn test_trait_node_id_format_matches_spec() {
    // R<len>_<sanitized_layer>_<sanitized_crate>_<sanitized_name>
    // layer="domain", crate_name="mylib", name="UserEmail"
    // len = unsanitized name char count = len("UserEmail") = 9
    // body = "domain_mylib_UserEmail"
    // expected: "R9_domain_mylib_UserEmail"
    let id = trait_node_id(&layer("domain"), &crate_name("mylib"), &trait_name("UserEmail"));
    assert_eq!(id, "R9_domain_mylib_UserEmail");
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
    // body = "my_layer_mylib_FooBar"
    // expected: "T6_my_layer_mylib_FooBar"
    let id = type_node_id(&layer("my-layer"), &crate_name("mylib"), &type_name("FooBar"));
    assert_eq!(id, "T6_my_layer_mylib_FooBar");
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
    let cn = crate_name("mylib");
    let id_d = type_node_id(&layer("domain"), &cn, &type_name("MyType"));
    let id_u = type_node_id(&layer("usecase"), &cn, &type_name("MyType"));
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

// ---------------------------------------------------------------------------
// T006 tests: mermaid render output structure
// ---------------------------------------------------------------------------

// Test 11: empty catalogues → flowchart TD header
#[test]
fn test_render_empty_catalogues_produces_flowchart_td_header() {
    let mermaid = render_with_full_style(&[], &[]);
    assert!(
        mermaid.starts_with("flowchart TD\n"),
        "output must start with 'flowchart TD\\n', got: {mermaid:?}"
    );
}

// Test 12: layer subgraph is emitted for a single-layer catalogue
#[test]
fn test_render_single_layer_emits_layer_subgraph() {
    let doc = make_minimal_catalogue("domain", "mylib");
    let layer_id = layer("domain");
    let mermaid = render_with_full_style(&[doc], &[layer_id]);
    // Layer sg id uses injective escape_id_component encoding:
    // "domain" → no hyphens/underscores → "domain" → "L_domain".
    assert!(
        mermaid.contains("subgraph L_domain[\"domain\"]"),
        "output must contain layer subgraph declaration with injective id, got:\n{mermaid}"
    );
    assert!(mermaid.contains("end"), "output must contain 'end' closing the subgraph");
}

// Test 13: TypeEntry is rendered as a subgraph (Decision F-2+b2-ii)
#[test]
fn test_render_type_entry_produces_entry_subgraph() {
    let mut doc = make_minimal_catalogue("domain", "mylib");
    doc.types.insert(type_name("UserEmail"), make_type_entry());
    let layer_id = layer("domain");
    let cn = crate_name("mylib");

    let mermaid = render_with_full_style(&[doc], std::slice::from_ref(&layer_id));
    let expected_id = type_node_id(&layer_id, &cn, &type_name("UserEmail"));
    assert!(
        mermaid.contains(&format!("subgraph {expected_id}[\"UserEmail\"]")),
        "output must contain TypeEntry subgraph with id={expected_id}, got:\n{mermaid}"
    );
}

// Test 14: TraitEntry is rendered as a subgraph even with zero methods (Decision F-2+b2-ii)
#[test]
fn test_render_trait_entry_with_no_methods_produces_empty_subgraph() {
    let mut doc = make_minimal_catalogue("domain", "mylib");
    doc.traits.insert(trait_name("UserRepo"), make_trait_entry());
    let layer_id = layer("domain");
    let cn = crate_name("mylib");

    let mermaid = render_with_full_style(&[doc], std::slice::from_ref(&layer_id));
    let expected_id = trait_node_id(&layer_id, &cn, &trait_name("UserRepo"));
    assert!(
        mermaid.contains(&format!("subgraph {expected_id}[\"UserRepo\"]")),
        "output must contain TraitEntry subgraph with id={expected_id}, got:\n{mermaid}"
    );
}

// Test 15: FunctionEntry is rendered as a standalone subroutine node (Decision F-2+d1)
#[test]
fn test_render_function_entry_produces_standalone_node_not_subgraph() {
    let mut doc = make_minimal_catalogue("domain", "mylib");
    let fn_path = FunctionPath::at_root(
        CrateName::new("mylib").unwrap(),
        FunctionName::new("do_work").unwrap(),
    );
    doc.functions.insert(fn_path.clone(), make_function_entry());
    let layer_id = layer("domain");

    let mermaid = render_with_full_style(&[doc], std::slice::from_ref(&layer_id));
    let fn_id = function_node_id(&layer_id, &fn_path);
    // Subroutine shape: [[name]]
    assert!(
        mermaid.contains(&format!("{fn_id}[[do_work]]")),
        "output must contain function node with subroutine shape, got:\n{mermaid}"
    );
    // Must NOT be a subgraph
    assert!(
        !mermaid.contains(&format!("subgraph {fn_id}")),
        "function entry must NOT be a subgraph, got:\n{mermaid}"
    );
}

// Test 16: method node inside TypeEntry subgraph (Decision F-2+b2-ii)
#[test]
fn test_render_type_entry_method_is_placed_inside_entry_subgraph() {
    let method = MethodDeclaration::new(
        MethodName::new("email").unwrap(),
        None,
        vec![],
        TypeRef::new("String").unwrap(),
        false,
        None,
    );
    let mut doc = make_minimal_catalogue("domain", "mylib");
    let entry = TypeEntry {
        action: ItemAction::Add,
        role: DataRole::ValueObject,
        kind: TypeKindV2::PlainStruct {
            fields: vec![],
            has_stripped_fields: false,
            typestate: None,
        },
        methods: vec![method],
        trait_impls: vec![],
        module_path: ModulePath::root(),
        docs: None,
        spec_refs: vec![],
        informal_grounds: vec![],
    };
    doc.types.insert(type_name("UserId"), entry);
    let layer_id = layer("domain");
    let cn = crate_name("mylib");

    let mermaid = render_with_full_style(&[doc], std::slice::from_ref(&layer_id));
    let entry_id = type_node_id(&layer_id, &cn, &type_name("UserId"));
    let method_id = format!("{entry_id}_m_0");

    // Method node rendered as round shape: (method_name)
    assert!(
        mermaid.contains(&format!("{method_id}(email)")),
        "output must contain method node with round shape, got:\n{mermaid}"
    );
    // Method node appears AFTER the entry subgraph open line.
    let entry_sg_pos = mermaid.find(&format!("subgraph {entry_id}")).unwrap();
    let method_pos = mermaid.find(&format!("{method_id}(email)")).unwrap();
    assert!(
        method_pos > entry_sg_pos,
        "method node must appear inside the entry subgraph, got:\n{mermaid}"
    );
}

// Test 17: top-module subgraph grouping (Decision U-6d-iii)
#[test]
fn test_render_entry_with_module_path_creates_top_module_subgraph() {
    let mut doc = make_minimal_catalogue("domain", "mylib");
    let entry = TypeEntry {
        action: ItemAction::Add,
        role: DataRole::ValueObject,
        kind: TypeKindV2::PlainStruct {
            fields: vec![],
            has_stripped_fields: false,
            typestate: None,
        },
        methods: vec![],
        trait_impls: vec![],
        module_path: ModulePath::from_segments(vec!["user".to_string(), "profile".to_string()])
            .unwrap(),
        docs: None,
        spec_refs: vec![],
        informal_grounds: vec![],
    };
    doc.types.insert(type_name("ProfileId"), entry);
    let layer_id = layer("domain");
    let cn = crate_name("mylib");

    let mermaid = render_with_full_style(&[doc], std::slice::from_ref(&layer_id));

    // Top-module subgraph id uses injective escape_id_component encoding:
    // layer "domain" → "L_domain"; top_seg "user" → "user" → "L_domain_M_user".
    assert!(
        mermaid.contains("subgraph L_domain_M_user[\"domain::user\"]"),
        "output must contain top-module subgraph with injective id, got:\n{mermaid}"
    );
    // Entry label must include sub-module path.
    let entry_id = type_node_id(&layer_id, &cn, &type_name("ProfileId"));
    assert!(
        mermaid.contains(&format!("subgraph {entry_id}[\"user::profile::ProfileId\"]")),
        "entry label must include sub-module path, got:\n{mermaid}"
    );
}

// Test 18: crate-root entries are placed directly in layer subgraph (AC-11)
#[test]
fn test_render_root_module_entry_is_in_layer_subgraph_directly() {
    let mut doc = make_minimal_catalogue("domain", "mylib");
    doc.types.insert(type_name("RootType"), make_type_entry());
    let layer_id = layer("domain");

    let mermaid = render_with_full_style(&[doc], std::slice::from_ref(&layer_id));

    // Layer subgraph must appear with injective id using escape_id_component encoding.
    // "domain" → no hyphens/underscores → "L_domain".
    assert!(
        mermaid.contains("subgraph L_domain[\"domain\"]"),
        "layer subgraph must appear with injective id, got:\n{mermaid}"
    );
    // There must be no top-module subgraph (top-module id has _M_ infix).
    assert!(
        !mermaid.contains("subgraph L_domain_M_"),
        "root-module entry must not create a top-module subgraph, got:\n{mermaid}"
    );
}

// Test 19: PlainStruct field edges (Decision K-2+(d))
#[test]
fn test_render_plain_struct_field_edges_are_emitted() {
    let mut doc = make_minimal_catalogue("domain", "mylib");

    // Target type (field type subgraph).
    doc.types.insert(type_name("Email"), make_type_entry());

    // Source type with a field pointing to Email.
    let source_entry = TypeEntry {
        action: ItemAction::Add,
        role: DataRole::ValueObject,
        kind: TypeKindV2::PlainStruct {
            fields: vec![FieldDecl::new(
                FieldName::new("email").unwrap(),
                TypeRef::new("Email").unwrap(),
            )],
            has_stripped_fields: false,
            typestate: None,
        },
        methods: vec![],
        trait_impls: vec![],
        module_path: ModulePath::root(),
        docs: None,
        spec_refs: vec![],
        informal_grounds: vec![],
    };
    doc.types.insert(type_name("User"), source_entry);
    let layer_id = layer("domain");
    let cn = crate_name("mylib");

    let mermaid = render_with_full_style(&[doc], std::slice::from_ref(&layer_id));

    let user_id = type_node_id(&layer_id, &cn, &type_name("User"));
    let email_id = type_node_id(&layer_id, &cn, &type_name("Email"));
    // Field edge: `<user_sg> --o|email| <email_sg>`
    assert!(
        mermaid.contains(&format!("{user_id} --o|email| {email_id}")),
        "output must contain field edge --o|email| for PlainStruct, got:\n{mermaid}"
    );
}

// Test 20: has_stripped_fields: true suppresses field edges (AC-08)
#[test]
fn test_render_stripped_fields_produces_no_field_edges() {
    let mut doc = make_minimal_catalogue("domain", "mylib");
    doc.types.insert(type_name("Email"), make_type_entry());
    let source_entry = TypeEntry {
        action: ItemAction::Add,
        role: DataRole::ValueObject,
        kind: TypeKindV2::PlainStruct {
            fields: vec![FieldDecl::new(
                FieldName::new("email").unwrap(),
                TypeRef::new("Email").unwrap(),
            )],
            has_stripped_fields: true, // stripped → no field edges
            typestate: None,
        },
        methods: vec![],
        trait_impls: vec![],
        module_path: ModulePath::root(),
        docs: None,
        spec_refs: vec![],
        informal_grounds: vec![],
    };
    doc.types.insert(type_name("User"), source_entry);
    let layer_id = layer("domain");
    let cn = crate_name("mylib");

    let mermaid = render_with_full_style(&[doc], std::slice::from_ref(&layer_id));

    let user_id = type_node_id(&layer_id, &cn, &type_name("User"));
    let email_id = type_node_id(&layer_id, &cn, &type_name("Email"));
    assert!(
        !mermaid.contains(&format!("{user_id} --o|email| {email_id}")),
        "stripped fields must not produce field edges, got:\n{mermaid}"
    );
}

// Test 21: TupleStruct field edges with positional labels (Decision K-2)
#[test]
fn test_render_tuple_struct_field_edges_have_positional_labels() {
    let mut doc = make_minimal_catalogue("domain", "mylib");
    doc.types.insert(type_name("Inner"), make_type_entry());

    let source_entry = TypeEntry {
        action: ItemAction::Add,
        role: DataRole::ValueObject,
        kind: TypeKindV2::TupleStruct {
            fields: vec![TypeRef::new("Inner").unwrap()],
            has_stripped_fields: false,
        },
        methods: vec![],
        trait_impls: vec![],
        module_path: ModulePath::root(),
        docs: None,
        spec_refs: vec![],
        informal_grounds: vec![],
    };
    doc.types.insert(type_name("Wrapper"), source_entry);
    let layer_id = layer("domain");
    let cn = crate_name("mylib");

    let mermaid = render_with_full_style(&[doc], std::slice::from_ref(&layer_id));

    let wrapper_id = type_node_id(&layer_id, &cn, &type_name("Wrapper"));
    let inner_id = type_node_id(&layer_id, &cn, &type_name("Inner"));
    assert!(
        mermaid.contains(&format!("{wrapper_id} --o|.0| {inner_id}")),
        "TupleStruct field must use positional label .0, got:\n{mermaid}"
    );
}

// Test 22: method param edge (Decision F-2+b2-ii)
#[test]
fn test_render_method_param_edge_is_emitted() {
    let mut doc = make_minimal_catalogue("domain", "mylib");
    doc.types.insert(type_name("UserId"), make_type_entry());

    let method = MethodDeclaration::new(
        MethodName::new("find").unwrap(),
        None,
        vec![ParamDeclaration::new(ParamName::new("id").unwrap(), TypeRef::new("UserId").unwrap())],
        TypeRef::new("()").unwrap(),
        false,
        None,
    );
    let entry = TypeEntry {
        action: ItemAction::Add,
        role: DataRole::ValueObject,
        kind: TypeKindV2::PlainStruct {
            fields: vec![],
            has_stripped_fields: false,
            typestate: None,
        },
        methods: vec![method],
        trait_impls: vec![],
        module_path: ModulePath::root(),
        docs: None,
        spec_refs: vec![],
        informal_grounds: vec![],
    };
    doc.types.insert(type_name("UserRepo"), entry);
    let layer_id = layer("domain");
    let cn = crate_name("mylib");

    let mermaid = render_with_full_style(&[doc], std::slice::from_ref(&layer_id));

    let entry_id = type_node_id(&layer_id, &cn, &type_name("UserRepo"));
    let method_id = format!("{entry_id}_m_0");
    let param_id = type_node_id(&layer_id, &cn, &type_name("UserId"));

    assert!(
        mermaid.contains(&format!("{method_id} --o {param_id}")),
        "method param must produce '--o' edge to the param type subgraph, got:\n{mermaid}"
    );
}

// Test 23: method returns edge (Decision F-2+b2-ii)
#[test]
fn test_render_method_returns_edge_is_emitted() {
    let mut doc = make_minimal_catalogue("domain", "mylib");
    doc.types.insert(type_name("UserId"), make_type_entry());

    let method = MethodDeclaration::new(
        MethodName::new("id").unwrap(),
        None,
        vec![],
        TypeRef::new("UserId").unwrap(),
        false,
        None,
    );
    let entry = TypeEntry {
        action: ItemAction::Add,
        role: DataRole::ValueObject,
        kind: TypeKindV2::PlainStruct {
            fields: vec![],
            has_stripped_fields: false,
            typestate: None,
        },
        methods: vec![method],
        trait_impls: vec![],
        module_path: ModulePath::root(),
        docs: None,
        spec_refs: vec![],
        informal_grounds: vec![],
    };
    doc.types.insert(type_name("User"), entry);
    let layer_id = layer("domain");
    let cn = crate_name("mylib");

    let mermaid = render_with_full_style(&[doc], std::slice::from_ref(&layer_id));

    let entry_id = type_node_id(&layer_id, &cn, &type_name("User"));
    let method_id = format!("{entry_id}_m_0");
    let returns_id = type_node_id(&layer_id, &cn, &type_name("UserId"));

    assert!(
        mermaid.contains(&format!("{method_id} --> {returns_id}")),
        "method returns must produce '-->' edge to the return type subgraph, got:\n{mermaid}"
    );
}

// Test 24: unresolvable TypeRef produces no edge (same-catalogue resolution only for T006)
#[test]
fn test_render_unresolvable_typeref_produces_no_edge() {
    let mut doc = make_minimal_catalogue("domain", "mylib");
    // "ExternalType" is NOT in the catalogues — should be silently skipped.
    let method = MethodDeclaration::new(
        MethodName::new("find").unwrap(),
        None,
        vec![ParamDeclaration::new(
            ParamName::new("x").unwrap(),
            TypeRef::new("ExternalType").unwrap(),
        )],
        TypeRef::new("()").unwrap(),
        false,
        None,
    );
    let entry = TypeEntry {
        action: ItemAction::Add,
        role: DataRole::ValueObject,
        kind: TypeKindV2::PlainStruct {
            fields: vec![],
            has_stripped_fields: false,
            typestate: None,
        },
        methods: vec![method],
        trait_impls: vec![],
        module_path: ModulePath::root(),
        docs: None,
        spec_refs: vec![],
        informal_grounds: vec![],
    };
    doc.types.insert(type_name("Holder"), entry);
    let layer_id = layer("domain");

    let mermaid = render_with_full_style(&[doc], &[layer_id]);

    assert!(
        !mermaid.contains("ExternalType"),
        "unresolvable TypeRef must be silently skipped (no edge), got:\n{mermaid}"
    );
}

// Test 25: layer_order controls layer emission order
#[test]
fn test_render_layer_order_controls_layer_emission_order() {
    let doc_domain = make_minimal_catalogue("domain", "mylib");
    let doc_infra = make_minimal_catalogue("infrastructure", "myinfra");
    let layer_order = vec![layer("infrastructure"), layer("domain")];

    let mermaid = render_with_full_style(&[doc_domain, doc_infra], &layer_order);

    // Injective layer subgraph ids use escape_id_component encoding:
    // "infrastructure" → no hyphens/underscores → "L_infrastructure"
    // "domain" → no hyphens/underscores → "L_domain"
    let pos_infra = mermaid.find("subgraph L_infrastructure").unwrap();
    let pos_domain = mermaid.find("subgraph L_domain").unwrap();
    assert!(
        pos_infra < pos_domain,
        "infrastructure layer must appear before domain (as specified in layer_order), got:\n{mermaid}"
    );
}

// Test 26: opts.layers non-empty restricts rendered layers to the allowlist
#[test]
fn test_render_opts_layers_filter_restricts_output_to_allowlist() {
    let doc_domain = make_minimal_catalogue("domain", "mylib");
    let doc_infra = make_minimal_catalogue("infrastructure", "myinfra");
    let layer_order = vec![layer("domain"), layer("infrastructure")];

    // Render with opts.layers = ["domain"] — infrastructure must be excluded.
    let dir = TempDir::new().unwrap();
    let path = write_style_config(&dir, full_toml_content());
    let adapter = ContractMapRendererAdapter::new(path);
    let opts = ContractMapRenderOptions { layers: vec![layer("domain")] };
    let result = adapter.render(&[doc_domain, doc_infra], &layer_order, &opts).unwrap();
    let mermaid = result.into_string();

    // Injective layer subgraph ids use escape_id_component encoding:
    // "domain" → no hyphens/underscores → "L_domain"
    // "infrastructure" → no hyphens/underscores → "L_infrastructure"
    assert!(
        mermaid.contains("subgraph L_domain"),
        "domain must be included when listed in opts.layers, got:\n{mermaid}"
    );
    assert!(
        !mermaid.contains("subgraph L_infrastructure"),
        "infrastructure must be excluded when not in opts.layers, got:\n{mermaid}"
    );
}

// Test 27: layer_sg_id produces distinct ids for "my-layer" vs "my_layer"
//
// Both LayerId values have the same char count (8) and sanitize to the same
// string ("my_layer"). A length-prefix scheme would silently collide; the
// escape_id_component encoding must keep them distinct.
#[test]
fn test_render_hyphen_layer_and_underscore_layer_produce_distinct_subgraph_ids() {
    // Use layer_order with "my-layer" first, "my_layer" second.
    let doc_hyphen = make_minimal_catalogue("my-layer", "crate_a");
    let doc_under = make_minimal_catalogue("my_layer", "crate_b");
    let layer_order = vec![layer("my-layer"), layer("my_layer")];

    let mermaid = render_with_full_style(&[doc_hyphen, doc_under], &layer_order);

    // "my-layer" → escape_id_component → "my_d_layer" → id = "L_my_d_layer"
    // "my_layer" → escape_id_component → "my__layer"  → id = "L_my__layer"
    assert!(
        mermaid.contains("subgraph L_my_d_layer[\"my-layer\"]"),
        "escaped id for 'my-layer' must be L_my_d_layer, got:\n{mermaid}"
    );
    assert!(
        mermaid.contains("subgraph L_my__layer[\"my_layer\"]"),
        "escaped id for 'my_layer' must be L_my__layer, got:\n{mermaid}"
    );
    // The two ids must be distinct (this is the core injectivity property).
    let pos_hyphen = mermaid.find("subgraph L_my_d_layer").unwrap();
    let pos_under = mermaid.find("subgraph L_my__layer").unwrap();
    assert_ne!(
        pos_hyphen, pos_under,
        "hyphen-layer and underscore-layer must produce distinct subgraph ids, got:\n{mermaid}"
    );
}
