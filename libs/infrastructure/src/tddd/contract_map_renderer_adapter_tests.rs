use std::io::Write;

use domain::tddd::catalogue_v2::composite::{TypeKindV2, TypestateMarker, TypestateTransitions};
use domain::tddd::catalogue_v2::entries::{FunctionEntry, TraitEntry, TypeEntry};
use domain::tddd::catalogue_v2::identifiers::{
    FieldName, MethodName, ParamName, TypeRef, VariantName,
};
use domain::tddd::catalogue_v2::methods::{MethodDeclaration, ParamDeclaration};
use domain::tddd::catalogue_v2::roles::{ContractRole, DataRole, FunctionRole, ItemAction};
use domain::tddd::catalogue_v2::variants::{FieldDecl, VariantDecl};
use domain::tddd::catalogue_v2::{
    CatalogueDocument, CrateName, FunctionName, FunctionPath, ModulePath, TraitImplDeclV2,
    TraitName, TypeName,
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

/// Full style config with the edge sections needed for T006/T007 edge rendering tests.
fn full_toml_content() -> &'static str {
    r#"
[edge.method_param]
arrow = "--o"

[edge.method_returns]
arrow = "-->"

[edge.field]
arrow = "--o"

[edge.variant_payload]
arrow = "--o"

[edge.alias]
arrow = "---"
label = "alias_of"

[edge.transition]
arrow = "==>"
label = "transitions_to"

[edge.trait_impl]
arrow = '-.->'
label = "impl"

[role.ValueObject]
class = "valueObject"

[role.SecondaryPort]
class = "secondaryPort"

[role.FreeFunction]
class = "freeFunction"

[role.UseCaseFunction]
class = "useCaseFunction"

[node.Method]
shape = "round"
class = "methodNode"

[node.Variant]
shape = "stadium"
class = "variantNode"

[node.Function]
shape = "subroutine"
class = "functionNode"

[pattern.Typestate]
overlay_class = "typestate"

[filter]
include_function_roles = []
"#
}

/// Full style config with function role filter applied (T008 Decision I-1 tests).
fn toml_with_function_role_filter(roles: &[&str]) -> String {
    let roles_list = roles.iter().map(|r| format!("\"{r}\"")).collect::<Vec<_>>().join(", ");
    format!(
        r#"
[edge.method_param]
arrow = "--o"

[edge.method_returns]
arrow = "-->"

[edge.field]
arrow = "--o"

[edge.variant_payload]
arrow = "--o"

[edge.alias]
arrow = "---"
label = "alias_of"

[edge.transition]
arrow = "==>"
label = "transitions_to"

[edge.trait_impl]
arrow = '-.->'
label = "impl"

[role.ValueObject]
class = "valueObject"

[role.SecondaryPort]
class = "secondaryPort"

[role.FreeFunction]
class = "freeFunction"

[role.UseCaseFunction]
class = "useCaseFunction"

[node.Method]
shape = "round"
class = "methodNode"

[node.Variant]
shape = "stadium"
class = "variantNode"

[node.Function]
shape = "subroutine"
class = "functionNode"

[pattern.Typestate]
overlay_class = "typestate"

[filter]
include_function_roles = [{roles_list}]
"#
    )
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
// Test 6: layer component uses injective escape_id_component (not lossy sanitize)
// ---------------------------------------------------------------------------

#[test]
fn test_type_node_id_escapes_hyphens_in_layer_name_injectivly() {
    // layer names can contain hyphens (e.g. "my-layer").
    // escape_id_component("my-layer"): 'my' kept; '-' → '_d_'; 'layer' kept → "my_d_layer"
    // sanitize("mylib") = "mylib", sanitize("FooBar") = "FooBar"
    // len = unsanitized name char count = len("FooBar") = 6
    // body = "my_d_layer_mylib_FooBar"
    // expected: "T6_my_d_layer_mylib_FooBar"
    let id = type_node_id(&layer("my-layer"), &crate_name("mylib"), &type_name("FooBar"));
    assert_eq!(id, "T6_my_d_layer_mylib_FooBar");
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

// ---------------------------------------------------------------------------
// T007 tests: Enum variant nodes, TypeAlias edge, typestate transition edges
// ---------------------------------------------------------------------------

// Test 28: Enum with VariantPayload::Unit → variant node emitted, no payload edges
#[test]
fn test_render_enum_unit_variant_emits_variant_node_without_payload_edges() {
    let mut doc = make_minimal_catalogue("domain", "mylib");

    let variant_name = VariantName::new("None").unwrap();
    let entry = TypeEntry {
        action: ItemAction::Add,
        role: DataRole::ValueObject,
        kind: TypeKindV2::Enum { variants: vec![VariantDecl::unit(variant_name.clone())] },
        methods: vec![],
        trait_impls: vec![],
        module_path: ModulePath::root(),
        docs: None,
        spec_refs: vec![],
        informal_grounds: vec![],
    };
    doc.types.insert(type_name("MyOption"), entry);
    let layer_id = layer("domain");
    let cn = crate_name("mylib");

    let mermaid = render_with_full_style(&[doc], std::slice::from_ref(&layer_id));

    let entry_id = type_node_id(&layer_id, &cn, &type_name("MyOption"));
    let variant_id = format!("{entry_id}_v_0");

    // Variant node should be present with stadium shape: ([None])
    assert!(
        mermaid.contains(&format!("{variant_id}([None])")),
        "unit variant must be rendered as stadium node, got:\n{mermaid}"
    );
    // Unit variant must not produce any edge from variant_id
    assert!(
        !mermaid.contains(&format!("{variant_id} --o")),
        "unit variant must not produce payload edges, got:\n{mermaid}"
    );
}

// Test 29: Enum with VariantPayload::Tuple → unlabelled --o edges per TypeRef (AC-04)
#[test]
fn test_render_enum_tuple_variant_emits_unlabelled_payload_edges() {
    let mut doc = make_minimal_catalogue("domain", "mylib");

    // Target type
    doc.types.insert(type_name("UserId"), make_type_entry());

    let variant_name = VariantName::new("Some").unwrap();
    let enum_entry = TypeEntry {
        action: ItemAction::Add,
        role: DataRole::ValueObject,
        kind: TypeKindV2::Enum {
            variants: vec![VariantDecl::tuple(variant_name, vec![TypeRef::new("UserId").unwrap()])],
        },
        methods: vec![],
        trait_impls: vec![],
        module_path: ModulePath::root(),
        docs: None,
        spec_refs: vec![],
        informal_grounds: vec![],
    };
    doc.types.insert(type_name("MyOption"), enum_entry);
    let layer_id = layer("domain");
    let cn = crate_name("mylib");

    let mermaid = render_with_full_style(&[doc], std::slice::from_ref(&layer_id));

    let entry_id = type_node_id(&layer_id, &cn, &type_name("MyOption"));
    let variant_id = format!("{entry_id}_v_0");
    let user_id_id = type_node_id(&layer_id, &cn, &type_name("UserId"));

    // Tuple variant: unlabelled --o edge to each TypeRef
    assert!(
        mermaid.contains(&format!("{variant_id} --o {user_id_id}")),
        "tuple variant must produce unlabelled --o edge to each TypeRef, got:\n{mermaid}"
    );
    // Must NOT produce a labelled edge
    assert!(
        !mermaid.contains(&format!("{variant_id} --o|")),
        "tuple variant must not produce labelled edges, got:\n{mermaid}"
    );
}

// Test 30: Enum with VariantPayload::Struct → labelled --o|field_name| edges per FieldDecl (AC-04)
#[test]
fn test_render_enum_struct_variant_emits_labelled_payload_edges() {
    let mut doc = make_minimal_catalogue("domain", "mylib");

    // Target type
    doc.types.insert(type_name("ErrorCode"), make_type_entry());

    let variant_name = VariantName::new("Fail").unwrap();
    let field_decl =
        FieldDecl::new(FieldName::new("code").unwrap(), TypeRef::new("ErrorCode").unwrap());
    let enum_entry = TypeEntry {
        action: ItemAction::Add,
        role: DataRole::ErrorType,
        kind: TypeKindV2::Enum {
            variants: vec![VariantDecl::struct_variant(variant_name, vec![field_decl])],
        },
        methods: vec![],
        trait_impls: vec![],
        module_path: ModulePath::root(),
        docs: None,
        spec_refs: vec![],
        informal_grounds: vec![],
    };
    doc.types.insert(type_name("AppError"), enum_entry);
    let layer_id = layer("domain");
    let cn = crate_name("mylib");

    let mermaid = render_with_full_style(&[doc], std::slice::from_ref(&layer_id));

    let entry_id = type_node_id(&layer_id, &cn, &type_name("AppError"));
    let variant_id = format!("{entry_id}_v_0");
    let error_code_id = type_node_id(&layer_id, &cn, &type_name("ErrorCode"));

    // Struct variant: labelled --o|field_name| edge per FieldDecl
    assert!(
        mermaid.contains(&format!("{variant_id} --o|code| {error_code_id}")),
        "struct variant must produce labelled --o|code| edge, got:\n{mermaid}"
    );
}

// Test 31: Enum with multiple variants → multiple variant nodes in index order (AC-04)
#[test]
fn test_render_enum_multiple_variants_emits_all_nodes_in_declaration_order() {
    let mut doc = make_minimal_catalogue("domain", "mylib");

    let v0 = VariantDecl::unit(VariantName::new("A").unwrap());
    let v1 = VariantDecl::unit(VariantName::new("B").unwrap());
    let v2 = VariantDecl::unit(VariantName::new("C").unwrap());
    let entry = TypeEntry {
        action: ItemAction::Add,
        role: DataRole::ValueObject,
        kind: TypeKindV2::Enum { variants: vec![v0, v1, v2] },
        methods: vec![],
        trait_impls: vec![],
        module_path: ModulePath::root(),
        docs: None,
        spec_refs: vec![],
        informal_grounds: vec![],
    };
    doc.types.insert(type_name("Abc"), entry);
    let layer_id = layer("domain");
    let cn = crate_name("mylib");

    let mermaid = render_with_full_style(&[doc], std::slice::from_ref(&layer_id));

    let entry_id = type_node_id(&layer_id, &cn, &type_name("Abc"));
    // All three variant nodes must be present
    assert!(
        mermaid.contains(&format!("{entry_id}_v_0([A])")),
        "variant A must be present as _v_0, got:\n{mermaid}"
    );
    assert!(
        mermaid.contains(&format!("{entry_id}_v_1([B])")),
        "variant B must be present as _v_1, got:\n{mermaid}"
    );
    assert!(
        mermaid.contains(&format!("{entry_id}_v_2([C])")),
        "variant C must be present as _v_2, got:\n{mermaid}"
    );
    // Declaration order: A before B before C
    let pos_a = mermaid.find(&format!("{entry_id}_v_0([A])")).unwrap();
    let pos_b = mermaid.find(&format!("{entry_id}_v_1([B])")).unwrap();
    let pos_c = mermaid.find(&format!("{entry_id}_v_2([C])")).unwrap();
    assert!(pos_a < pos_b && pos_b < pos_c, "variants must appear in declaration order");
}

// Test 32: TypeAlias → empty subgraph + ---|alias_of| undirected edge to target (AC-09)
#[test]
fn test_render_type_alias_emits_empty_subgraph_and_undirected_edge() {
    let mut doc = make_minimal_catalogue("domain", "mylib");

    // Target type
    doc.types.insert(type_name("String"), make_type_entry());

    let alias_entry = TypeEntry {
        action: ItemAction::Add,
        role: DataRole::ValueObject,
        kind: TypeKindV2::TypeAlias { target: TypeRef::new("String").unwrap() },
        methods: vec![],
        trait_impls: vec![],
        module_path: ModulePath::root(),
        docs: None,
        spec_refs: vec![],
        informal_grounds: vec![],
    };
    doc.types.insert(type_name("Name"), alias_entry);
    let layer_id = layer("domain");
    let cn = crate_name("mylib");

    let mermaid = render_with_full_style(&[doc], std::slice::from_ref(&layer_id));

    let alias_id = type_node_id(&layer_id, &cn, &type_name("Name"));
    let target_id = type_node_id(&layer_id, &cn, &type_name("String"));

    // Alias entry must be rendered as a subgraph (empty — no methods/variants)
    assert!(
        mermaid.contains(&format!("subgraph {alias_id}[\"Name\"]")),
        "alias must be rendered as a subgraph, got:\n{mermaid}"
    );
    // Undirected edge: alias_id ---|alias_of| target_id
    assert!(
        mermaid.contains(&format!("{alias_id} ---|alias_of| {target_id}")),
        "alias must produce ---|alias_of| undirected edge to target, got:\n{mermaid}"
    );
}

// Test 33: TypeAlias with unresolvable target → no edge (AC-09 silent skip)
#[test]
fn test_render_type_alias_with_unresolvable_target_produces_no_edge() {
    let mut doc = make_minimal_catalogue("domain", "mylib");

    // "ExternalTarget" is NOT in the catalogue
    let alias_entry = TypeEntry {
        action: ItemAction::Add,
        role: DataRole::ValueObject,
        kind: TypeKindV2::TypeAlias { target: TypeRef::new("ExternalTarget").unwrap() },
        methods: vec![],
        trait_impls: vec![],
        module_path: ModulePath::root(),
        docs: None,
        spec_refs: vec![],
        informal_grounds: vec![],
    };
    doc.types.insert(type_name("MyAlias"), alias_entry);
    let layer_id = layer("domain");
    let cn = crate_name("mylib");

    let mermaid = render_with_full_style(&[doc], std::slice::from_ref(&layer_id));

    let alias_id = type_node_id(&layer_id, &cn, &type_name("MyAlias"));

    // Subgraph still rendered
    assert!(
        mermaid.contains(&format!("subgraph {alias_id}[\"MyAlias\"]")),
        "alias subgraph must still be rendered even with unresolvable target, got:\n{mermaid}"
    );
    // But no alias edge
    assert!(
        !mermaid.contains("---|alias_of|"),
        "unresolvable alias target must produce no alias edge, got:\n{mermaid}"
    );
}

// Test 34: Typestate transition method → ==>|transitions_to| edge (AC-03)
#[test]
fn test_render_typestate_transition_method_uses_transition_edge_style() {
    let mut doc = make_minimal_catalogue("domain", "mylib");

    // Return type for the transition method
    doc.types.insert(type_name("NextState"), make_type_entry());
    // Return type for the non-transition method
    doc.types.insert(type_name("UserId"), make_type_entry());

    let transition_method_name = MethodName::new("approve").unwrap();
    let normal_method_name = MethodName::new("get_id").unwrap();

    let transition_method = MethodDeclaration::new(
        transition_method_name.clone(),
        None,
        vec![],
        TypeRef::new("NextState").unwrap(),
        false,
        None,
    );
    let normal_method = MethodDeclaration::new(
        normal_method_name.clone(),
        None,
        vec![],
        TypeRef::new("UserId").unwrap(),
        false,
        None,
    );

    // PlainStruct with typestate: "approve" is a transition method
    let ts_marker = TypestateMarker::new(
        TypeName::new("ReviewMachine").unwrap(),
        TypestateTransitions::new(vec![transition_method_name.clone()]),
    );
    let entry = TypeEntry {
        action: ItemAction::Add,
        role: DataRole::ValueObject,
        kind: TypeKindV2::PlainStruct {
            fields: vec![],
            has_stripped_fields: false,
            typestate: Some(ts_marker),
        },
        methods: vec![transition_method, normal_method],
        trait_impls: vec![],
        module_path: ModulePath::root(),
        docs: None,
        spec_refs: vec![],
        informal_grounds: vec![],
    };
    doc.types.insert(type_name("PendingReview"), entry);
    let layer_id = layer("domain");
    let cn = crate_name("mylib");

    let mermaid = render_with_full_style(&[doc], std::slice::from_ref(&layer_id));

    let entry_id = type_node_id(&layer_id, &cn, &type_name("PendingReview"));
    // m_0 = transition method (approve), m_1 = normal method (get_id)
    let transition_method_id = format!("{entry_id}_m_0");
    let normal_method_id = format!("{entry_id}_m_1");
    let next_state_id = type_node_id(&layer_id, &cn, &type_name("NextState"));
    let user_id_id = type_node_id(&layer_id, &cn, &type_name("UserId"));

    // Transition method returns edge must use ==>|transitions_to|
    assert!(
        mermaid.contains(&format!("{transition_method_id} ==>|transitions_to| {next_state_id}")),
        "transition method returns must use ==>|transitions_to|, got:\n{mermaid}"
    );
    // Normal method returns edge must use --> (unchanged)
    assert!(
        mermaid.contains(&format!("{normal_method_id} --> {user_id_id}")),
        "non-transition method returns must still use -->, got:\n{mermaid}"
    );
    // Transition method must NOT have a normal --> returns edge
    assert!(
        !mermaid.contains(&format!("{transition_method_id} --> {next_state_id}")),
        "transition method must not produce normal --> returns edge, got:\n{mermaid}"
    );
}

// Test 35: Typestate PlainStruct → overlay class attached (pattern.Typestate, AC-03)
#[test]
fn test_render_typestate_entry_gets_overlay_class_attached() {
    let mut doc = make_minimal_catalogue("domain", "mylib");

    let ts_marker = TypestateMarker::new(
        TypeName::new("MyMachine").unwrap(),
        TypestateTransitions::new(vec![]),
    );
    let entry = TypeEntry {
        action: ItemAction::Add,
        role: DataRole::ValueObject,
        kind: TypeKindV2::PlainStruct {
            fields: vec![],
            has_stripped_fields: false,
            typestate: Some(ts_marker),
        },
        methods: vec![],
        trait_impls: vec![],
        module_path: ModulePath::root(),
        docs: None,
        spec_refs: vec![],
        informal_grounds: vec![],
    };
    doc.types.insert(type_name("StateFoo"), entry);
    let layer_id = layer("domain");
    let cn = crate_name("mylib");

    let mermaid = render_with_full_style(&[doc], std::slice::from_ref(&layer_id));

    let entry_id = type_node_id(&layer_id, &cn, &type_name("StateFoo"));
    // Overlay class attach line must be present
    assert!(
        mermaid.contains(&format!("class {entry_id} typestate")),
        "typestate entry must have overlay class 'typestate' attached, got:\n{mermaid}"
    );
}

// Test 36: PlainStruct without typestate → no overlay class (AC-03 control)
#[test]
fn test_render_non_typestate_plain_struct_has_no_overlay_class() {
    let mut doc = make_minimal_catalogue("domain", "mylib");

    let entry = TypeEntry {
        action: ItemAction::Add,
        role: DataRole::ValueObject,
        kind: TypeKindV2::PlainStruct {
            fields: vec![],
            has_stripped_fields: false,
            typestate: None, // no typestate
        },
        methods: vec![],
        trait_impls: vec![],
        module_path: ModulePath::root(),
        docs: None,
        spec_refs: vec![],
        informal_grounds: vec![],
    };
    doc.types.insert(type_name("NormalStruct"), entry);
    let layer_id = layer("domain");

    let mermaid = render_with_full_style(&[doc], std::slice::from_ref(&layer_id));

    // No typestate overlay class should appear
    assert!(
        !mermaid.contains("typestate"),
        "non-typestate struct must not have typestate overlay class, got:\n{mermaid}"
    );
}

// Test 37: Typestate param edges remain --o (unchanged by typestate, AC-03)
#[test]
fn test_render_typestate_transition_method_param_edge_uses_standard_arrow() {
    let mut doc = make_minimal_catalogue("domain", "mylib");

    doc.types.insert(type_name("ParamType"), make_type_entry());
    doc.types.insert(type_name("NextState"), make_type_entry());

    let method_name = MethodName::new("go").unwrap();
    let method = MethodDeclaration::new(
        method_name.clone(),
        None,
        vec![ParamDeclaration::new(
            ParamName::new("x").unwrap(),
            TypeRef::new("ParamType").unwrap(),
        )],
        TypeRef::new("NextState").unwrap(),
        false,
        None,
    );

    let ts_marker = TypestateMarker::new(
        TypeName::new("Machine").unwrap(),
        TypestateTransitions::new(vec![method_name.clone()]),
    );
    let entry = TypeEntry {
        action: ItemAction::Add,
        role: DataRole::ValueObject,
        kind: TypeKindV2::PlainStruct {
            fields: vec![],
            has_stripped_fields: false,
            typestate: Some(ts_marker),
        },
        methods: vec![method],
        trait_impls: vec![],
        module_path: ModulePath::root(),
        docs: None,
        spec_refs: vec![],
        informal_grounds: vec![],
    };
    doc.types.insert(type_name("StateA"), entry);
    let layer_id = layer("domain");
    let cn = crate_name("mylib");

    let mermaid = render_with_full_style(&[doc], std::slice::from_ref(&layer_id));

    let entry_id = type_node_id(&layer_id, &cn, &type_name("StateA"));
    let method_id = format!("{entry_id}_m_0");
    let param_id = type_node_id(&layer_id, &cn, &type_name("ParamType"));
    let next_id = type_node_id(&layer_id, &cn, &type_name("NextState"));

    // Param edge: still --o (unchanged)
    assert!(
        mermaid.contains(&format!("{method_id} --o {param_id}")),
        "transition method param edge must still use --o, got:\n{mermaid}"
    );
    // Returns edge: transition ==>|transitions_to|
    assert!(
        mermaid.contains(&format!("{method_id} ==>|transitions_to| {next_id}")),
        "transition method returns must use ==>|transitions_to|, got:\n{mermaid}"
    );
}

// Test 38: Enum variant nodes are placed inside the entry subgraph (Decision H-3)
#[test]
fn test_render_enum_variant_nodes_placed_inside_entry_subgraph() {
    let mut doc = make_minimal_catalogue("domain", "mylib");

    let entry = TypeEntry {
        action: ItemAction::Add,
        role: DataRole::ValueObject,
        kind: TypeKindV2::Enum {
            variants: vec![VariantDecl::unit(VariantName::new("VarA").unwrap())],
        },
        methods: vec![],
        trait_impls: vec![],
        module_path: ModulePath::root(),
        docs: None,
        spec_refs: vec![],
        informal_grounds: vec![],
    };
    doc.types.insert(type_name("MyEnum"), entry);
    let layer_id = layer("domain");
    let cn = crate_name("mylib");

    let mermaid = render_with_full_style(&[doc], std::slice::from_ref(&layer_id));

    let entry_id = type_node_id(&layer_id, &cn, &type_name("MyEnum"));
    let variant_id = format!("{entry_id}_v_0");

    // Variant must appear after the entry subgraph open line (inside the subgraph)
    let entry_open_pos = mermaid.find(&format!("subgraph {entry_id}")).unwrap();
    let variant_pos = mermaid.find(&format!("{variant_id}([VarA])")).unwrap();
    let entry_end_pos = {
        // Find the "end" that closes this subgraph — appears after entry_open_pos
        let after_open = &mermaid[entry_open_pos..];
        entry_open_pos + after_open.find("end").unwrap()
    };
    assert!(
        variant_pos > entry_open_pos && variant_pos < entry_end_pos,
        "variant node must appear inside the entry subgraph (between open and end), \
         open={entry_open_pos}, variant={variant_pos}, end={entry_end_pos}, mermaid:\n{mermaid}"
    );
}

// Test 39: Enum with Tuple variant and multiple TypeRefs → one edge per TypeRef (AC-04)
#[test]
fn test_render_enum_tuple_variant_with_multiple_type_refs_emits_one_edge_per_ref() {
    let mut doc = make_minimal_catalogue("domain", "mylib");

    doc.types.insert(type_name("ErrorCode"), make_type_entry());
    doc.types.insert(type_name("ErrorMsg"), make_type_entry());

    let variant = VariantDecl::tuple(
        VariantName::new("Failure").unwrap(),
        vec![TypeRef::new("ErrorCode").unwrap(), TypeRef::new("ErrorMsg").unwrap()],
    );
    let entry = TypeEntry {
        action: ItemAction::Add,
        role: DataRole::ErrorType,
        kind: TypeKindV2::Enum { variants: vec![variant] },
        methods: vec![],
        trait_impls: vec![],
        module_path: ModulePath::root(),
        docs: None,
        spec_refs: vec![],
        informal_grounds: vec![],
    };
    doc.types.insert(type_name("MyError"), entry);
    let layer_id = layer("domain");
    let cn = crate_name("mylib");

    let mermaid = render_with_full_style(&[doc], std::slice::from_ref(&layer_id));

    let entry_id = type_node_id(&layer_id, &cn, &type_name("MyError"));
    let variant_id = format!("{entry_id}_v_0");
    let code_id = type_node_id(&layer_id, &cn, &type_name("ErrorCode"));
    let msg_id = type_node_id(&layer_id, &cn, &type_name("ErrorMsg"));

    assert!(
        mermaid.contains(&format!("{variant_id} --o {code_id}")),
        "tuple variant must have edge to ErrorCode, got:\n{mermaid}"
    );
    assert!(
        mermaid.contains(&format!("{variant_id} --o {msg_id}")),
        "tuple variant must have edge to ErrorMsg, got:\n{mermaid}"
    );
}

// ---------------------------------------------------------------------------
// T008 tests: cross-catalogue trait_impl edges, function role filter,
//             mermaid output ordering, AC-14 layer-agnostic invariant
// ---------------------------------------------------------------------------

// Test 40 (T008-a): cross-catalogue trait_impl edge — type in crate A implements
// trait declared in crate B; edge emitted as -.->|impl| (Decision O-2 + O-3 + O-a)
#[test]
fn test_render_cross_catalogue_trait_impl_edge_is_emitted() {
    // Two catalogues in the same layer:
    // - domain_crate: declares TypeA with trait_impls = [MyPort (from port_crate)]
    // - port_crate: declares TraitMyPort
    let layer_id = layer("domain");

    let mut doc_domain = make_minimal_catalogue("domain", "domain_crate");
    let mut doc_port = make_minimal_catalogue("domain", "port_crate");

    // Declare the trait in port_crate
    doc_port.traits.insert(trait_name("MyPort"), make_trait_entry());

    // TypeA in domain_crate implements MyPort from port_crate
    let type_a = TypeEntry {
        action: ItemAction::Add,
        role: DataRole::Entity,
        kind: TypeKindV2::PlainStruct {
            fields: vec![],
            has_stripped_fields: false,
            typestate: None,
        },
        methods: vec![],
        trait_impls: vec![TraitImplDeclV2::new(
            TraitName::new("MyPort").unwrap(),
            CrateName::new("port_crate").unwrap(),
        )],
        module_path: ModulePath::root(),
        docs: None,
        spec_refs: vec![],
        informal_grounds: vec![],
    };
    doc_domain.types.insert(type_name("TypeA"), type_a);

    let type_a_id = type_node_id(&layer_id, &crate_name("domain_crate"), &type_name("TypeA"));
    let my_port_id = trait_node_id(&layer_id, &crate_name("port_crate"), &trait_name("MyPort"));

    let mermaid = render_with_full_style(&[doc_domain, doc_port], &[layer_id]);

    // Edge: TypeA -.->|impl| MyPort (Decision O-2 + O-3)
    assert!(
        mermaid.contains(&format!("{type_a_id} -.->|impl| {my_port_id}")),
        "cross-catalogue trait_impl edge must be emitted as -.->|impl|, got:\n{mermaid}"
    );
}

// Test 41 (T008-a): workspace-external trait → silent skip, no edge emitted
// (Decision J-2 + CN-08)
#[test]
fn test_render_workspace_external_trait_impl_is_silently_skipped() {
    let layer_id = layer("domain");
    let mut doc = make_minimal_catalogue("domain", "my_crate");

    // TypeA implements std::fmt::Display — "std" crate has no catalogue
    let type_a = TypeEntry {
        action: ItemAction::Add,
        role: DataRole::ValueObject,
        kind: TypeKindV2::PlainStruct {
            fields: vec![],
            has_stripped_fields: false,
            typestate: None,
        },
        methods: vec![],
        trait_impls: vec![TraitImplDeclV2::new(
            TraitName::new("Display").unwrap(),
            CrateName::new("std").unwrap(),
        )],
        module_path: ModulePath::root(),
        docs: None,
        spec_refs: vec![],
        informal_grounds: vec![],
    };
    doc.types.insert(type_name("MyType"), type_a);

    let mermaid = render_with_full_style(&[doc], &[layer_id]);

    // "Display" trait is from "std" which has no catalogue entry → silent skip
    assert!(
        !mermaid.contains("-.->"),
        "workspace-external trait impl must be silently skipped (no edge), got:\n{mermaid}"
    );
    assert!(
        !mermaid.contains("Display"),
        "workspace-external trait name must not appear in the output, got:\n{mermaid}"
    );
}

// Test 42 (T008): mermaid output ordering — classDef before subgraphs before
// edges before class attach lines (Decision U, CN-05)
#[test]
fn test_render_output_ordering_classdef_before_subgraphs_before_edges_before_class_lines() {
    let layer_id = layer("domain");
    let mut doc_domain = make_minimal_catalogue("domain", "domain_crate");
    let mut doc_port = make_minimal_catalogue("domain", "port_crate");

    // A trait in port_crate
    doc_port.traits.insert(trait_name("MyTrait"), make_trait_entry());

    // A type that has a class and a trait impl edge
    let type_entry = TypeEntry {
        action: ItemAction::Add,
        role: DataRole::ValueObject,
        kind: TypeKindV2::PlainStruct {
            fields: vec![],
            has_stripped_fields: false,
            typestate: None,
        },
        methods: vec![],
        trait_impls: vec![TraitImplDeclV2::new(
            TraitName::new("MyTrait").unwrap(),
            CrateName::new("port_crate").unwrap(),
        )],
        module_path: ModulePath::root(),
        docs: None,
        spec_refs: vec![],
        informal_grounds: vec![],
    };
    doc_domain.types.insert(type_name("MyType"), type_entry);

    // Use style config with a classDef
    let dir = TempDir::new().unwrap();
    let style_content = r##"
[edge.trait_impl]
arrow = '-.->'
label = "impl"

[role.ValueObject]
class = "valueObject"

[role.SecondaryPort]
class = "secondaryPort"

[node.Method]
shape = "round"
class = "methodNode"

[class.valueObject]
fill = "#fff"
stroke = "#000"
stroke_width = "1px"
stroke_dasharray = "0"

[filter]
include_function_roles = []
"##;
    let path = write_style_config(&dir, style_content);
    let adapter = ContractMapRendererAdapter::new(path);
    let opts = ContractMapRenderOptions::empty();
    let result = adapter.render(&[doc_domain, doc_port], &[layer_id], &opts).unwrap();
    let mermaid = result.into_string();

    // Find positions of each section
    let classdef_pos = mermaid.find("classDef ").unwrap();
    let subgraph_pos = mermaid.find("subgraph ").unwrap();
    let edge_pos = mermaid.find("-.->").unwrap();
    // class attach lines come after edges
    let class_attach_pos =
        mermaid.find("class T").unwrap_or_else(|| mermaid.find("class R").unwrap_or(mermaid.len()));

    assert!(
        classdef_pos < subgraph_pos,
        "classDef lines must precede subgraph lines, got:\n{mermaid}"
    );
    assert!(subgraph_pos < edge_pos, "subgraph lines must precede edge lines, got:\n{mermaid}");
    assert!(
        edge_pos < class_attach_pos,
        "edge lines must precede class attach lines, got:\n{mermaid}"
    );
}

// Test 43 (T008, AC-14): layer-agnostic invariant — arbitrary LayerId values used
// as subgraph labels without hardcoding (CN-02, AC-14)
#[test]
fn test_render_arbitrary_layer_ids_used_as_subgraph_labels() {
    let doc_a = make_minimal_catalogue("my-layer", "crate_a");
    let doc_b = make_minimal_catalogue("other-layer", "crate_b");
    let layer_order = vec![layer("my-layer"), layer("other-layer")];

    let mermaid = render_with_full_style(&[doc_a, doc_b], &layer_order);

    // Each LayerId must appear verbatim as the subgraph label (not mangled)
    assert!(
        mermaid.contains("subgraph L_my_d_layer[\"my-layer\"]"),
        "arbitrary layer 'my-layer' must appear as subgraph label, got:\n{mermaid}"
    );
    assert!(
        mermaid.contains("subgraph L_other_d_layer[\"other-layer\"]"),
        "arbitrary layer 'other-layer' must appear as subgraph label, got:\n{mermaid}"
    );
    // Must NOT contain any hardcoded layer name like "domain" or "infrastructure"
    assert!(
        !mermaid.contains("\"domain\""),
        "output must not contain hardcoded 'domain' label, got:\n{mermaid}"
    );
    assert!(
        !mermaid.contains("\"infrastructure\""),
        "output must not contain hardcoded 'infrastructure' label, got:\n{mermaid}"
    );
}

// Test 44 (T008): classDef lines are alphabetically ordered by class name (Decision U, CN-05)
#[test]
fn test_render_classdef_lines_are_alphabetically_ordered() {
    // Style with multiple class entries to verify alphabetical ordering
    let dir = TempDir::new().unwrap();
    let style_content = r##"
[class.zClass]
fill = "#fff"
stroke = "#000"
stroke_width = "1px"
stroke_dasharray = "0"

[class.aClass]
fill = "#f00"
stroke = "#000"
stroke_width = "1px"
stroke_dasharray = "0"

[class.mClass]
fill = "#0f0"
stroke = "#000"
stroke_width = "1px"
stroke_dasharray = "0"

[filter]
include_function_roles = []
"##;
    let path = write_style_config(&dir, style_content);
    let adapter = ContractMapRendererAdapter::new(path);
    let opts = ContractMapRenderOptions::empty();
    let result = adapter.render(&[], &[], &opts).unwrap();
    let mermaid = result.into_string();

    let pos_a = mermaid.find("classDef aClass").unwrap();
    let pos_m = mermaid.find("classDef mClass").unwrap();
    let pos_z = mermaid.find("classDef zClass").unwrap();

    assert!(
        pos_a < pos_m && pos_m < pos_z,
        "classDef lines must be alphabetically ordered (a < m < z), got:\n{mermaid}"
    );
}

// Test 45 (T008): class attach lines use separate `class <id> <className>` format
// (Decision U, CN-05 — no subgraph-inline ::: syntax)
#[test]
fn test_render_class_attach_uses_separate_class_lines_not_inline_syntax() {
    let mut doc = make_minimal_catalogue("domain", "mylib");
    doc.types.insert(type_name("MyType"), make_type_entry());
    let layer_id = layer("domain");
    let cn = crate_name("mylib");

    let mermaid = render_with_full_style(&[doc], std::slice::from_ref(&layer_id));
    let entry_id = type_node_id(&layer_id, &cn, &type_name("MyType"));

    // Separate class attach line: "class <id> <className>"
    assert!(
        mermaid.contains(&format!("class {entry_id} ")),
        "class attach must use separate 'class <id> <className>' line, got:\n{mermaid}"
    );
    // Must NOT use inline ::: syntax on the subgraph line.
    // Mermaid's inline class form is `subgraph id:::className` (no `{` separator
    // between `:::` and the class name). Check for `{entry_id}:::` to catch any
    // reintroduction of this banned form (CN-05).
    assert!(
        !mermaid.contains(&format!("{entry_id}:::")),
        "class attach must NOT use inline subgraph ::: syntax, got:\n{mermaid}"
    );
}

// Test 46 (T008, Decision I-1): empty include_function_roles → all functions rendered
#[test]
fn test_render_empty_function_role_filter_renders_all_functions() {
    let mut doc = make_minimal_catalogue("domain", "mylib");
    let fn_path_a =
        FunctionPath::at_root(CrateName::new("mylib").unwrap(), FunctionName::new("fn_a").unwrap());
    let fn_path_b =
        FunctionPath::at_root(CrateName::new("mylib").unwrap(), FunctionName::new("fn_b").unwrap());

    // fn_a: FreeFunction
    doc.functions.insert(fn_path_a.clone(), make_function_entry());
    // fn_b: UseCaseFunction
    doc.functions.insert(
        fn_path_b.clone(),
        FunctionEntry {
            action: ItemAction::Add,
            role: FunctionRole::UseCaseFunction,
            params: vec![],
            returns: TypeRef::new("()").unwrap(),
            is_async: false,
            generics: vec![],
            where_predicates: vec![],
            docs: None,
            spec_refs: vec![],
            informal_grounds: vec![],
        },
    );

    let layer_id = layer("domain");
    let fn_a_id = function_node_id(&layer_id, &fn_path_a);
    let fn_b_id = function_node_id(&layer_id, &fn_path_b);

    // With empty filter → both functions rendered
    let mermaid = render_with_full_style(&[doc], &[layer_id]);

    assert!(
        mermaid.contains(&format!("{fn_a_id}[[fn_a]]")),
        "empty filter: FreeFunction fn_a must be rendered, got:\n{mermaid}"
    );
    assert!(
        mermaid.contains(&format!("{fn_b_id}[[fn_b]]")),
        "empty filter: UseCaseFunction fn_b must be rendered, got:\n{mermaid}"
    );
}

// Test 47 (T008, Decision I-1): non-empty include_function_roles → only matching
// functions rendered, others silently skipped (IN-10)
#[test]
fn test_render_non_empty_function_role_filter_skips_non_matching_functions() {
    let mut doc = make_minimal_catalogue("domain", "mylib");
    let fn_path_free = FunctionPath::at_root(
        CrateName::new("mylib").unwrap(),
        FunctionName::new("free_fn").unwrap(),
    );
    let fn_path_uc = FunctionPath::at_root(
        CrateName::new("mylib").unwrap(),
        FunctionName::new("uc_fn").unwrap(),
    );

    // free_fn: FreeFunction
    doc.functions.insert(fn_path_free.clone(), make_function_entry());
    // uc_fn: UseCaseFunction
    doc.functions.insert(
        fn_path_uc.clone(),
        FunctionEntry {
            action: ItemAction::Add,
            role: FunctionRole::UseCaseFunction,
            params: vec![],
            returns: TypeRef::new("()").unwrap(),
            is_async: false,
            generics: vec![],
            where_predicates: vec![],
            docs: None,
            spec_refs: vec![],
            informal_grounds: vec![],
        },
    );

    let layer_id = layer("domain");
    let fn_free_id = function_node_id(&layer_id, &fn_path_free);
    let fn_uc_id = function_node_id(&layer_id, &fn_path_uc);

    // With filter ["UseCaseFunction"] → only uc_fn rendered; free_fn skipped
    let dir = TempDir::new().unwrap();
    let toml_content = toml_with_function_role_filter(&["UseCaseFunction"]);
    let path = write_style_config(&dir, &toml_content);
    let adapter = ContractMapRendererAdapter::new(path);
    let opts = ContractMapRenderOptions::empty();
    let result = adapter.render(&[doc], &[layer_id], &opts).unwrap();
    let mermaid = result.into_string();

    assert!(
        mermaid.contains(&format!("{fn_uc_id}[[uc_fn]]")),
        "UseCaseFunction uc_fn must be rendered when in filter, got:\n{mermaid}"
    );
    assert!(
        !mermaid.contains(&format!("{fn_free_id}[[free_fn]]")),
        "FreeFunction free_fn must be skipped when not in filter, got:\n{mermaid}"
    );
}

// Test 48 (T008-b): trait in an excluded layer (via opts.layers allowlist) must
// NOT produce a trait_impl edge — prevents dangling Mermaid edges (Decision O-a,
// CN-08). This is the cross-layer exclusion case: TypeA is in the rendered layer
// "domain" and implements TraitB from the excluded layer "infra".
#[test]
fn test_render_trait_impl_edge_not_emitted_when_trait_layer_is_excluded_by_opts_layers() {
    // domain layer: one type that implements a trait from the infra layer.
    let layer_domain = layer("domain");
    let layer_infra = layer("infra");

    let mut doc_domain = make_minimal_catalogue("domain", "domain_crate");
    let mut doc_infra = make_minimal_catalogue("infra", "infra_crate");

    // Declare the trait in the infra catalogue (a different layer).
    doc_infra.traits.insert(trait_name("InfraTrait"), make_trait_entry());

    // TypeA in domain_crate declares an impl of InfraTrait from infra_crate.
    let type_a = TypeEntry {
        action: ItemAction::Add,
        role: DataRole::Entity,
        kind: TypeKindV2::PlainStruct {
            fields: vec![],
            has_stripped_fields: false,
            typestate: None,
        },
        methods: vec![],
        trait_impls: vec![TraitImplDeclV2::new(
            TraitName::new("InfraTrait").unwrap(),
            CrateName::new("infra_crate").unwrap(),
        )],
        module_path: ModulePath::root(),
        docs: None,
        spec_refs: vec![],
        informal_grounds: vec![],
    };
    doc_domain.types.insert(type_name("TypeA"), type_a);

    // Render only the "domain" layer (opts.layers = ["domain"]). The "infra" layer
    // is excluded: its trait node is never rendered, so no edge should be emitted.
    let dir = TempDir::new().unwrap();
    let path = write_style_config(&dir, full_toml_content());
    let adapter = ContractMapRendererAdapter::new(path);
    let opts = ContractMapRenderOptions { layers: vec![layer_domain.clone()] };
    let result =
        adapter.render(&[doc_domain, doc_infra], &[layer_domain, layer_infra], &opts).unwrap();
    let mermaid = result.into_string();

    // The infra layer (and its trait node) is excluded: no edge must be emitted.
    assert!(
        !mermaid.contains("-.->"),
        "trait_impl edge must not be emitted when the trait's layer is excluded by opts.layers, \
         got:\n{mermaid}"
    );
    assert!(
        !mermaid.contains("InfraTrait"),
        "excluded trait name must not appear in the output, got:\n{mermaid}"
    );
    // The type in the rendered domain layer must still appear.
    assert!(
        mermaid.contains("TypeA"),
        "TypeA in the rendered domain layer must appear in the output, got:\n{mermaid}"
    );
}

// Test 48 (T008-i): unknown FunctionRole name in include_function_roles → StyleConfigParse
// (fail-closed validation; a typo must not silently filter everything out)
#[test]
fn test_render_with_unknown_function_role_in_filter_returns_style_config_parse() {
    let dir = TempDir::new().unwrap();
    let bad_toml = r#"
[filter]
include_function_roles = ["FreeFunctionn"]
"#;
    let path = write_style_config(&dir, bad_toml);
    let adapter = ContractMapRendererAdapter::new(path.clone());
    let catalogues: Vec<CatalogueDocument> = vec![];
    let layer_order: Vec<LayerId> = vec![];
    let opts = ContractMapRenderOptions::empty();

    let result = adapter.render(&catalogues, &layer_order, &opts);
    assert!(
        matches!(
            result,
            Err(ContractMapRendererError::StyleConfigParse { path: ref p, reason: ref r })
            if *p == path && r.contains("FreeFunctionn")
        ),
        "expected StyleConfigParse for unknown role 'FreeFunctionn', got: {result:?}"
    );
}

// ===========================================================================
// T010 ACCEPTANCE TESTS (AC-01 through AC-12)
//
// These tests use a rich, layer-agnostic multi-crate fixture to exercise all
// acceptance criteria in an integrated manner. Each test targets a specific AC
// or a group of closely-related ACs, using fixtures that cross crate and layer
// boundaries to validate the full rendering pipeline.
// ===========================================================================

// ---------------------------------------------------------------------------
// Fixture builder helpers for T010 acceptance tests
// ---------------------------------------------------------------------------

/// Helper: build a TraitEntry with the given ContractRole.
fn make_trait_entry_with_role(role: ContractRole) -> TraitEntry {
    TraitEntry {
        action: ItemAction::Add,
        role,
        methods: vec![],
        supertrait_bounds: vec![],
        module_path: ModulePath::root(),
        docs: None,
        spec_refs: vec![],
        informal_grounds: vec![],
    }
}

/// Helper: build a TraitEntry with one method, at crate root, with the given ContractRole.
fn make_trait_entry_with_role_and_module(role: ContractRole, module: ModulePath) -> TraitEntry {
    TraitEntry {
        action: ItemAction::Add,
        role,
        methods: vec![],
        supertrait_bounds: vec![],
        module_path: module,
        docs: None,
        spec_refs: vec![],
        informal_grounds: vec![],
    }
}

/// Helper: build a TypeEntry (PlainStruct, no fields, no methods, no typestate) in the given module.
fn make_plain_struct_in_module(module: ModulePath) -> TypeEntry {
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
        module_path: module,
        docs: None,
        spec_refs: vec![],
        informal_grounds: vec![],
    }
}

/// Helper: render using the full style config and return the mermaid string.
/// Same as `render_with_full_style` but accepts the layer_order as a slice reference.
fn render_acceptance(catalogues: &[CatalogueDocument], layer_order: &[LayerId]) -> String {
    render_with_full_style(catalogues, layer_order)
}

// ---------------------------------------------------------------------------
// Test 49 (AC-01): Two CatalogueDocuments in the same layer are aggregated into
// one layer subgraph (1 layer N crate scenario).
// ---------------------------------------------------------------------------

#[test]
fn test_ac01_two_catalogues_same_layer_aggregated_into_one_layer_subgraph() {
    // Two crates in the same "my-domain" layer:
    // - crate_a declares TypeA
    // - crate_b declares TypeB
    let layer_id = layer("my-domain");
    let cn_a = crate_name("crate_a");
    let cn_b = crate_name("crate_b");

    let mut doc_a = make_minimal_catalogue("my-domain", "crate_a");
    let mut doc_b = make_minimal_catalogue("my-domain", "crate_b");

    doc_a.types.insert(type_name("TypeA"), make_type_entry());
    doc_b.types.insert(type_name("TypeB"), make_type_entry());

    let mermaid = render_acceptance(&[doc_a, doc_b], std::slice::from_ref(&layer_id));

    // There must be exactly one layer subgraph for "my-domain" (injective escaped id).
    // "my-domain" → escape_id_component → "my_d_domain" → subgraph id: "L_my_d_domain"
    let count = mermaid.matches("subgraph L_my_d_domain[\"my-domain\"]").count();
    assert_eq!(
        count, 1,
        "exactly one layer subgraph must be emitted for 'my-domain' (1 layer N crate), \
         got {count} occurrences in:\n{mermaid}"
    );

    // Both TypeA and TypeB must appear inside that single layer subgraph.
    let type_a_id = type_node_id(&layer_id, &cn_a, &type_name("TypeA"));
    let type_b_id = type_node_id(&layer_id, &cn_b, &type_name("TypeB"));

    assert!(
        mermaid.contains(&format!("subgraph {type_a_id}[\"TypeA\"]")),
        "TypeA from crate_a must appear in mermaid, got:\n{mermaid}"
    );
    assert!(
        mermaid.contains(&format!("subgraph {type_b_id}[\"TypeB\"]")),
        "TypeB from crate_b must appear in mermaid, got:\n{mermaid}"
    );

    // The two type node ids must be distinct (no collision between crates).
    assert_ne!(
        type_a_id, type_b_id,
        "node ids for same-name types in different crates of the same layer must be distinct"
    );
}

// ---------------------------------------------------------------------------
// Test 50 (AC-02): TypeEntry and TraitEntry with zero methods produce empty
// subgraphs. FunctionEntry produces a standalone node.
// ---------------------------------------------------------------------------

#[test]
fn test_ac02_zero_method_entries_produce_empty_subgraphs_function_produces_standalone_node() {
    let layer_id = layer("app-layer");
    let cn = crate_name("my_crate");

    let mut doc = make_minimal_catalogue("app-layer", "my_crate");
    doc.types.insert(type_name("EmptyStruct"), make_type_entry());
    doc.traits.insert(trait_name("EmptyTrait"), make_trait_entry());
    let fn_path = FunctionPath::at_root(
        CrateName::new("my_crate").unwrap(),
        FunctionName::new("lone_fn").unwrap(),
    );
    doc.functions.insert(fn_path.clone(), make_function_entry());

    let mermaid = render_acceptance(&[doc], std::slice::from_ref(&layer_id));

    // EmptyStruct → subgraph (even with zero methods)
    let struct_id = type_node_id(&layer_id, &cn, &type_name("EmptyStruct"));
    assert!(
        mermaid.contains(&format!("subgraph {struct_id}[\"EmptyStruct\"]")),
        "AC-02: EmptyStruct must be rendered as subgraph even with zero methods, got:\n{mermaid}"
    );

    // EmptyTrait → subgraph (even with zero methods)
    let trait_id = trait_node_id(&layer_id, &cn, &trait_name("EmptyTrait"));
    assert!(
        mermaid.contains(&format!("subgraph {trait_id}[\"EmptyTrait\"]")),
        "AC-02: EmptyTrait must be rendered as subgraph even with zero methods, got:\n{mermaid}"
    );

    // FunctionEntry → standalone subroutine node (NOT a subgraph)
    let fn_id = function_node_id(&layer_id, &fn_path);
    assert!(
        mermaid.contains(&format!("{fn_id}[[lone_fn]]")),
        "AC-02: FunctionEntry must produce a standalone subroutine node, got:\n{mermaid}"
    );
    assert!(
        !mermaid.contains(&format!("subgraph {fn_id}")),
        "AC-02: FunctionEntry must NOT be a subgraph, got:\n{mermaid}"
    );
}

// ---------------------------------------------------------------------------
// Test 51 (AC-03 + AC-02): Typestate transition edge uses ==>|transitions_to|
// while normal returns edge uses -->. Both appear in same entry.
// ---------------------------------------------------------------------------

#[test]
fn test_ac03_typestate_transition_edge_distinguished_from_normal_returns_edge() {
    let layer_id = layer("state-layer");
    let cn = crate_name("state_crate");

    let mut doc = make_minimal_catalogue("state-layer", "state_crate");

    // Return types for the two methods.
    doc.types.insert(type_name("NextState"), make_type_entry());
    doc.types.insert(type_name("InfoValue"), make_type_entry());

    let transition_method_name = MethodName::new("advance").unwrap();
    let normal_method_name = MethodName::new("get_info").unwrap();

    let transition_method = MethodDeclaration::new(
        transition_method_name.clone(),
        None,
        vec![],
        TypeRef::new("NextState").unwrap(),
        false,
        None,
    );
    let normal_method = MethodDeclaration::new(
        normal_method_name.clone(),
        None,
        vec![],
        TypeRef::new("InfoValue").unwrap(),
        false,
        None,
    );

    let ts_marker = TypestateMarker::new(
        TypeName::new("StateMachine").unwrap(),
        TypestateTransitions::new(vec![transition_method_name]),
    );

    let entry = TypeEntry {
        action: ItemAction::Add,
        role: DataRole::Entity,
        kind: TypeKindV2::PlainStruct {
            fields: vec![],
            has_stripped_fields: false,
            typestate: Some(ts_marker),
        },
        methods: vec![transition_method, normal_method],
        trait_impls: vec![],
        module_path: ModulePath::root(),
        docs: None,
        spec_refs: vec![],
        informal_grounds: vec![],
    };
    doc.types.insert(type_name("StateNode"), entry);

    let mermaid = render_acceptance(&[doc], std::slice::from_ref(&layer_id));

    let entry_id = type_node_id(&layer_id, &cn, &type_name("StateNode"));
    let m0_id = format!("{entry_id}_m_0"); // advance (transition)
    let m1_id = format!("{entry_id}_m_1"); // get_info (normal)
    let next_state_id = type_node_id(&layer_id, &cn, &type_name("NextState"));
    let info_id = type_node_id(&layer_id, &cn, &type_name("InfoValue"));

    // AC-03: transition method returns uses ==>|transitions_to|
    assert!(
        mermaid.contains(&format!("{m0_id} ==>|transitions_to| {next_state_id}")),
        "AC-03: transition method must use ==>|transitions_to|, got:\n{mermaid}"
    );
    // AC-03: normal method returns uses --> (not transition style)
    assert!(
        mermaid.contains(&format!("{m1_id} --> {info_id}")),
        "AC-03: normal method returns must use -->, got:\n{mermaid}"
    );
    // AC-03: transition method must NOT produce normal --> returns edge
    assert!(
        !mermaid.contains(&format!("{m0_id} --> {next_state_id}")),
        "AC-03: transition method must not produce normal --> edge, got:\n{mermaid}"
    );
}

// ---------------------------------------------------------------------------
// Test 52 (AC-04): Enum VariantPayload::Tuple, Struct, and Unit all render
// correctly with distinct edge behaviour.
// ---------------------------------------------------------------------------

#[test]
fn test_ac04_enum_all_three_variant_payload_kinds_render_correctly() {
    let layer_id = layer("enum-layer");
    let cn = crate_name("enum_crate");

    let mut doc = make_minimal_catalogue("enum-layer", "enum_crate");

    // Target types for Tuple and Struct variants.
    doc.types.insert(type_name("PayloadA"), make_type_entry());
    doc.types.insert(type_name("PayloadB"), make_type_entry());

    // Enum with three variants:
    // - UnitV: Unit → no edge
    // - TupleV: Tuple(PayloadA) → unlabelled --o
    // - StructV: Struct { field_x: PayloadB } → labelled --o|field_x|
    let v_unit = VariantDecl::unit(VariantName::new("UnitV").unwrap());
    let v_tuple = VariantDecl::tuple(
        VariantName::new("TupleV").unwrap(),
        vec![TypeRef::new("PayloadA").unwrap()],
    );
    let v_struct = VariantDecl::struct_variant(
        VariantName::new("StructV").unwrap(),
        vec![FieldDecl::new(FieldName::new("field_x").unwrap(), TypeRef::new("PayloadB").unwrap())],
    );

    let enum_entry = TypeEntry {
        action: ItemAction::Add,
        role: DataRole::ErrorType,
        kind: TypeKindV2::Enum { variants: vec![v_unit, v_tuple, v_struct] },
        methods: vec![],
        trait_impls: vec![],
        module_path: ModulePath::root(),
        docs: None,
        spec_refs: vec![],
        informal_grounds: vec![],
    };
    doc.types.insert(type_name("MyEnum"), enum_entry);

    let mermaid = render_acceptance(&[doc], std::slice::from_ref(&layer_id));

    let entry_id = type_node_id(&layer_id, &cn, &type_name("MyEnum"));
    let v0_id = format!("{entry_id}_v_0"); // UnitV
    let v1_id = format!("{entry_id}_v_1"); // TupleV
    let v2_id = format!("{entry_id}_v_2"); // StructV
    let payload_a_id = type_node_id(&layer_id, &cn, &type_name("PayloadA"));
    let payload_b_id = type_node_id(&layer_id, &cn, &type_name("PayloadB"));

    // UnitV: node present, no payload edge
    assert!(
        mermaid.contains(&format!("{v0_id}([UnitV])")),
        "AC-04: UnitV must be rendered as stadium node, got:\n{mermaid}"
    );
    assert!(
        !mermaid.contains(&format!("{v0_id} --o")),
        "AC-04: UnitV must not produce any payload edge, got:\n{mermaid}"
    );

    // TupleV: node present, unlabelled --o edge to PayloadA
    assert!(
        mermaid.contains(&format!("{v1_id}([TupleV])")),
        "AC-04: TupleV must be rendered as stadium node, got:\n{mermaid}"
    );
    assert!(
        mermaid.contains(&format!("{v1_id} --o {payload_a_id}")),
        "AC-04: TupleV must produce unlabelled --o edge, got:\n{mermaid}"
    );
    // Must NOT produce a labelled edge for TupleV
    assert!(
        !mermaid.contains(&format!("{v1_id} --o|")),
        "AC-04: TupleV must not produce labelled edge, got:\n{mermaid}"
    );

    // StructV: node present, labelled --o|field_x| edge to PayloadB
    assert!(
        mermaid.contains(&format!("{v2_id}([StructV])")),
        "AC-04: StructV must be rendered as stadium node, got:\n{mermaid}"
    );
    assert!(
        mermaid.contains(&format!("{v2_id} --o|field_x| {payload_b_id}")),
        "AC-04: StructV must produce labelled --o|field_x| edge, got:\n{mermaid}"
    );
}

// ---------------------------------------------------------------------------
// Test 53 (AC-05): node_id collision-free across Types, Traits, and Functions
// in a multi-crate, multi-layer fixture.
// ---------------------------------------------------------------------------

#[test]
fn test_ac05_node_id_collision_free_across_types_traits_functions_and_layers() {
    // Two layers, two crates each. Each crate has a type, a trait, and a function
    // with the same name "Entity" / "Port" / "helper". Node ids must all be distinct.
    let layer_a = layer("layer-a");
    let layer_b = layer("layer-b");
    let cn1 = crate_name("crate_one");
    let cn2 = crate_name("crate_two");

    let mut doc_a1 = make_minimal_catalogue("layer-a", "crate_one");
    let mut doc_a2 = make_minimal_catalogue("layer-a", "crate_two");
    let mut doc_b1 = make_minimal_catalogue("layer-b", "crate_one");

    doc_a1.types.insert(type_name("Entity"), make_type_entry());
    doc_a1.traits.insert(trait_name("Entity"), make_trait_entry()); // same name as type!
    doc_a2.types.insert(type_name("Entity"), make_type_entry());
    doc_b1.types.insert(type_name("Entity"), make_type_entry());

    let fn_path_a1 = FunctionPath::at_root(
        CrateName::new("crate_one").unwrap(),
        FunctionName::new("helper").unwrap(),
    );
    doc_a1.functions.insert(fn_path_a1.clone(), make_function_entry());

    // A second "helper" function in crate_two (same name, different crate) — this exercises
    // the crate-dimension of collision avoidance that the node_id scheme must handle.
    let fn_path_a2 = FunctionPath::at_root(
        CrateName::new("crate_two").unwrap(),
        FunctionName::new("helper").unwrap(),
    );
    doc_a2.functions.insert(fn_path_a2.clone(), make_function_entry());

    let mermaid = render_acceptance(&[doc_a1, doc_a2, doc_b1], &[layer_a.clone(), layer_b.clone()]);

    // Collect the node ids we expect.
    let type_a1_id = type_node_id(&layer_a, &cn1, &type_name("Entity"));
    let trait_a1_id = trait_node_id(&layer_a, &cn1, &trait_name("Entity"));
    let type_a2_id = type_node_id(&layer_a, &cn2, &type_name("Entity"));
    let type_b1_id = type_node_id(&layer_b, &cn1, &type_name("Entity"));
    let fn_a1_id = function_node_id(&layer_a, &fn_path_a1);
    let fn_a2_id = function_node_id(&layer_a, &fn_path_a2);

    // fn_a1_id and fn_a2_id share the same function name but different crates — they must
    // be distinct, which would fail if the node_id scheme omitted the crate component.
    assert_ne!(
        fn_a1_id, fn_a2_id,
        "AC-05: same function name in different crates must produce distinct node ids"
    );

    // All ids must be distinct.
    let ids = vec![
        type_a1_id.clone(),
        trait_a1_id.clone(),
        type_a2_id.clone(),
        type_b1_id.clone(),
        fn_a1_id.clone(),
        fn_a2_id.clone(),
    ];
    let mut unique_ids = ids.clone();
    unique_ids.sort();
    unique_ids.dedup();
    assert_eq!(
        ids.len(),
        unique_ids.len(),
        "AC-05: all node ids must be distinct; found duplicates among: {ids:?}"
    );

    // Type prefix T vs Trait prefix R ensures no collision for same name + same layer + same crate.
    assert!(type_a1_id.starts_with('T'), "type id must start with T: {type_a1_id}");
    assert!(trait_a1_id.starts_with('R'), "trait id must start with R: {trait_a1_id}");

    // All ids must appear in the output.
    assert!(mermaid.contains(&type_a1_id), "AC-05: {type_a1_id} must appear in output");
    assert!(mermaid.contains(&trait_a1_id), "AC-05: {trait_a1_id} must appear in output");
    assert!(mermaid.contains(&type_a2_id), "AC-05: {type_a2_id} must appear in output");
    assert!(mermaid.contains(&type_b1_id), "AC-05: {type_b1_id} must appear in output");
    assert!(mermaid.contains(&fn_a1_id), "AC-05: {fn_a1_id} must appear in output");
    assert!(mermaid.contains(&fn_a2_id), "AC-05: {fn_a2_id} must appear in output");
}

// ---------------------------------------------------------------------------
// Test 54 (AC-06): config absent → fail-closed StyleConfigNotFound.
// This integrated test verifies the fail-closed behaviour in the context of a
// rich fixture (not just an empty catalogue).
// ---------------------------------------------------------------------------

#[test]
fn test_ac06_config_absent_fail_closed_with_rich_fixture() {
    let dir = TempDir::new().unwrap();
    let nonexistent = dir.path().join("missing-style.toml");
    let adapter = ContractMapRendererAdapter::new(nonexistent.clone());

    // Rich fixture with multiple crates and types.
    let mut doc = make_minimal_catalogue("rich-layer", "rich_crate");
    doc.types.insert(type_name("RichType"), make_type_entry());
    doc.traits.insert(trait_name("RichTrait"), make_trait_entry());

    let opts = ContractMapRenderOptions::empty();
    let result = adapter.render(&[doc], &[layer("rich-layer")], &opts);

    assert!(
        matches!(
            result,
            Err(ContractMapRendererError::StyleConfigNotFound { ref path })
            if path == &nonexistent
        ),
        "AC-06: must return StyleConfigNotFound when config is absent, got: {result:?}"
    );
}

// ---------------------------------------------------------------------------
// Test 55 (AC-07): All three ContractRole values on TraitEntry produce the same
// -.->|impl| edge style (no role-dependent variation).
// ---------------------------------------------------------------------------

#[test]
fn test_ac07_all_contract_roles_produce_unified_trait_impl_edge_style() {
    let layer_id = layer("arch-layer");
    let cn = crate_name("impl_crate");

    let mut doc = make_minimal_catalogue("arch-layer", "impl_crate");

    // Three trait entries, one per ContractRole.
    doc.traits.insert(
        trait_name("SpecPort"),
        make_trait_entry_with_role(ContractRole::SpecificationPort),
    );
    doc.traits.insert(
        trait_name("AppService"),
        make_trait_entry_with_role(ContractRole::ApplicationService),
    );
    doc.traits
        .insert(trait_name("SecPort"), make_trait_entry_with_role(ContractRole::SecondaryPort));

    // One type that implements all three traits.
    let impl_type = TypeEntry {
        action: ItemAction::Add,
        role: DataRole::SecondaryAdapter,
        kind: TypeKindV2::PlainStruct {
            fields: vec![],
            has_stripped_fields: false,
            typestate: None,
        },
        methods: vec![],
        trait_impls: vec![
            TraitImplDeclV2::new(TraitName::new("SpecPort").unwrap(), cn.clone()),
            TraitImplDeclV2::new(TraitName::new("AppService").unwrap(), cn.clone()),
            TraitImplDeclV2::new(TraitName::new("SecPort").unwrap(), cn.clone()),
        ],
        module_path: ModulePath::root(),
        docs: None,
        spec_refs: vec![],
        informal_grounds: vec![],
    };
    doc.types.insert(type_name("Adapter"), impl_type);

    let mermaid = render_acceptance(&[doc], std::slice::from_ref(&layer_id));

    let type_id = type_node_id(&layer_id, &cn, &type_name("Adapter"));
    let spec_port_id = trait_node_id(&layer_id, &cn, &trait_name("SpecPort"));
    let app_service_id = trait_node_id(&layer_id, &cn, &trait_name("AppService"));
    let sec_port_id = trait_node_id(&layer_id, &cn, &trait_name("SecPort"));

    // AC-07: all three impl edges must use -.->|impl| (unified style regardless of ContractRole)
    assert!(
        mermaid.contains(&format!("{type_id} -.->|impl| {spec_port_id}")),
        "AC-07: SpecificationPort trait impl must use -.->|impl|, got:\n{mermaid}"
    );
    assert!(
        mermaid.contains(&format!("{type_id} -.->|impl| {app_service_id}")),
        "AC-07: ApplicationService trait impl must use -.->|impl|, got:\n{mermaid}"
    );
    assert!(
        mermaid.contains(&format!("{type_id} -.->|impl| {sec_port_id}")),
        "AC-07: SecondaryPort trait impl must use -.->|impl|, got:\n{mermaid}"
    );

    // AC-07: must NOT have role-specific edge variations (e.g. different arrow for different roles).
    // All three edges use the same `-.->` arrow — count occurrences.
    let impl_edge_count = mermaid.matches("-.->|impl|").count();
    assert_eq!(
        impl_edge_count, 3,
        "AC-07: exactly 3 -.->|impl| edges must be emitted (one per impl), got {impl_edge_count}"
    );
}

// ---------------------------------------------------------------------------
// Test 56 (AC-08): has_stripped_fields suppresses all field edges for both
// PlainStruct and TupleStruct. Stripped type still gets a subgraph.
// ---------------------------------------------------------------------------

#[test]
fn test_ac08_has_stripped_fields_true_suppresses_all_field_edges_for_both_struct_kinds() {
    let layer_id = layer("strip-layer");
    let cn = crate_name("stripped_crate");

    let mut doc = make_minimal_catalogue("strip-layer", "stripped_crate");
    doc.types.insert(type_name("FieldTarget"), make_type_entry());
    doc.types.insert(type_name("TupleTarget"), make_type_entry());

    // PlainStruct with has_stripped_fields: true
    let plain_stripped = TypeEntry {
        action: ItemAction::Add,
        role: DataRole::ValueObject,
        kind: TypeKindV2::PlainStruct {
            fields: vec![FieldDecl::new(
                FieldName::new("secret").unwrap(),
                TypeRef::new("FieldTarget").unwrap(),
            )],
            has_stripped_fields: true,
            typestate: None,
        },
        methods: vec![],
        trait_impls: vec![],
        module_path: ModulePath::root(),
        docs: None,
        spec_refs: vec![],
        informal_grounds: vec![],
    };
    doc.types.insert(type_name("StrippedPlain"), plain_stripped);

    // TupleStruct with has_stripped_fields: true
    let tuple_stripped = TypeEntry {
        action: ItemAction::Add,
        role: DataRole::ValueObject,
        kind: TypeKindV2::TupleStruct {
            fields: vec![TypeRef::new("TupleTarget").unwrap()],
            has_stripped_fields: true,
        },
        methods: vec![],
        trait_impls: vec![],
        module_path: ModulePath::root(),
        docs: None,
        spec_refs: vec![],
        informal_grounds: vec![],
    };
    doc.types.insert(type_name("StrippedTuple"), tuple_stripped);

    let mermaid = render_acceptance(&[doc], std::slice::from_ref(&layer_id));

    let plain_id = type_node_id(&layer_id, &cn, &type_name("StrippedPlain"));
    let tuple_id = type_node_id(&layer_id, &cn, &type_name("StrippedTuple"));
    let field_target_id = type_node_id(&layer_id, &cn, &type_name("FieldTarget"));
    let tuple_target_id = type_node_id(&layer_id, &cn, &type_name("TupleTarget"));

    // AC-08: both stripped types must still appear as subgraphs (just no field edges).
    assert!(
        mermaid.contains(&format!("subgraph {plain_id}[\"StrippedPlain\"]")),
        "AC-08: StrippedPlain must appear as subgraph, got:\n{mermaid}"
    );
    assert!(
        mermaid.contains(&format!("subgraph {tuple_id}[\"StrippedTuple\"]")),
        "AC-08: StrippedTuple must appear as subgraph, got:\n{mermaid}"
    );

    // AC-08: no field edges must be emitted for stripped types.
    assert!(
        !mermaid.contains(&format!("{plain_id} --o|secret| {field_target_id}")),
        "AC-08: StrippedPlain must not produce field edges (has_stripped_fields=true), got:\n{mermaid}"
    );
    assert!(
        !mermaid.contains(&format!("{tuple_id} --o|.0| {tuple_target_id}")),
        "AC-08: StrippedTuple must not produce field edges (has_stripped_fields=true), got:\n{mermaid}"
    );
}

// ---------------------------------------------------------------------------
// Test 57 (AC-09): TypeAlias renders as empty subgraph + ---|alias_of| undirected
// edge to the target type subgraph.
// ---------------------------------------------------------------------------

#[test]
fn test_ac09_type_alias_renders_as_empty_subgraph_with_undirected_alias_of_edge() {
    let layer_id = layer("alias-layer");
    let cn = crate_name("alias_crate");

    let mut doc = make_minimal_catalogue("alias-layer", "alias_crate");
    doc.types.insert(type_name("OriginalType"), make_type_entry());

    let alias_entry = TypeEntry {
        action: ItemAction::Add,
        role: DataRole::Dto,
        kind: TypeKindV2::TypeAlias { target: TypeRef::new("OriginalType").unwrap() },
        methods: vec![],
        trait_impls: vec![],
        module_path: ModulePath::root(),
        docs: None,
        spec_refs: vec![],
        informal_grounds: vec![],
    };
    doc.types.insert(type_name("AliasType"), alias_entry);

    let mermaid = render_acceptance(&[doc], std::slice::from_ref(&layer_id));

    let alias_id = type_node_id(&layer_id, &cn, &type_name("AliasType"));
    let original_id = type_node_id(&layer_id, &cn, &type_name("OriginalType"));

    // AC-09: alias renders as a subgraph (empty — no methods, no variants)
    assert!(
        mermaid.contains(&format!("subgraph {alias_id}[\"AliasType\"]")),
        "AC-09: TypeAlias must be rendered as a subgraph, got:\n{mermaid}"
    );

    // AC-09: undirected ---|alias_of| edge to target
    assert!(
        mermaid.contains(&format!("{alias_id} ---|alias_of| {original_id}")),
        "AC-09: TypeAlias must produce ---|alias_of| undirected edge to target, got:\n{mermaid}"
    );

    // AC-09: must NOT produce a directed edge to the target
    assert!(
        !mermaid.contains(&format!("{alias_id} --> {original_id}")),
        "AC-09: TypeAlias must not produce a directed --> edge, got:\n{mermaid}"
    );
    assert!(
        !mermaid.contains(&format!("{alias_id} --o {original_id}")),
        "AC-09: TypeAlias must not produce a directed --o edge, got:\n{mermaid}"
    );
}

// ---------------------------------------------------------------------------
// Test 58 (AC-10): workspace-external traits are silently skipped; workspace-
// internal traits produce -.->|impl| edges via cross-catalogue index.
// ---------------------------------------------------------------------------

#[test]
fn test_ac10_workspace_external_traits_silently_skipped_workspace_internal_traits_produce_edges() {
    let layer_id = layer("ws-layer");
    let cn_impl = crate_name("impl_crate");
    let cn_port = crate_name("port_crate");

    // port_crate: declares WorkspaceTrait
    let mut doc_port = make_minimal_catalogue("ws-layer", "port_crate");
    doc_port.traits.insert(trait_name("WorkspaceTrait"), make_trait_entry());

    // impl_crate: type implements WorkspaceTrait (workspace-internal) AND
    // std::fmt::Debug (workspace-external, origin_crate="std") AND
    // some_ext::Serialize (workspace-external, origin_crate="serde")
    let mut doc_impl = make_minimal_catalogue("ws-layer", "impl_crate");
    let impl_type = TypeEntry {
        action: ItemAction::Add,
        role: DataRole::ValueObject,
        kind: TypeKindV2::PlainStruct {
            fields: vec![],
            has_stripped_fields: false,
            typestate: None,
        },
        methods: vec![],
        trait_impls: vec![
            // workspace-internal: must produce edge
            TraitImplDeclV2::new(TraitName::new("WorkspaceTrait").unwrap(), cn_port.clone()),
            // workspace-external (std): must be silently skipped
            TraitImplDeclV2::new(TraitName::new("Debug").unwrap(), CrateName::new("std").unwrap()),
            // workspace-external (core): must be silently skipped
            TraitImplDeclV2::new(TraitName::new("Clone").unwrap(), CrateName::new("core").unwrap()),
            // workspace-external (third-party): must be silently skipped
            TraitImplDeclV2::new(
                TraitName::new("Serialize").unwrap(),
                CrateName::new("serde").unwrap(),
            ),
        ],
        module_path: ModulePath::root(),
        docs: None,
        spec_refs: vec![],
        informal_grounds: vec![],
    };
    doc_impl.types.insert(type_name("ImplType"), impl_type);

    let mermaid = render_acceptance(&[doc_impl, doc_port], std::slice::from_ref(&layer_id));

    let impl_type_id = type_node_id(&layer_id, &cn_impl, &type_name("ImplType"));
    let workspace_trait_id = trait_node_id(&layer_id, &cn_port, &trait_name("WorkspaceTrait"));

    // AC-10: workspace-internal WorkspaceTrait → edge emitted
    assert!(
        mermaid.contains(&format!("{impl_type_id} -.->|impl| {workspace_trait_id}")),
        "AC-10: workspace-internal WorkspaceTrait must produce -.->|impl| edge, got:\n{mermaid}"
    );

    // AC-10: workspace-external traits must NOT produce edges or appear in output.
    assert!(
        !mermaid.contains("Debug"),
        "AC-10: workspace-external trait 'Debug' must not appear in output, got:\n{mermaid}"
    );
    assert!(
        !mermaid.contains("Clone"),
        "AC-10: workspace-external trait 'Clone' must not appear in output, got:\n{mermaid}"
    );
    assert!(
        !mermaid.contains("Serialize"),
        "AC-10: workspace-external trait 'Serialize' must not appear in output, got:\n{mermaid}"
    );

    // AC-10: exactly one -.-> edge (only the workspace-internal one).
    let edge_count = mermaid.matches("-.->").count();
    assert_eq!(
        edge_count, 1,
        "AC-10: exactly 1 -.-> edge (only the workspace-internal trait), got {edge_count}"
    );
}

// ---------------------------------------------------------------------------
// Test 59 (AC-11): crate-root entries (module_path=[]) appear directly in the
// layer subgraph; entries with module_path=["some_module"] are placed inside a
// top-module subgraph.
// ---------------------------------------------------------------------------

#[test]
fn test_ac11_crate_root_entries_in_layer_subgraph_and_module_entries_in_top_module_subgraph() {
    let layer_id = layer("mod-layer");
    let cn = crate_name("module_crate");

    let mut doc = make_minimal_catalogue("mod-layer", "module_crate");

    // crate-root entry (module_path = [])
    let root_entry = make_plain_struct_in_module(ModulePath::root());
    doc.types.insert(type_name("RootEntry"), root_entry);

    // module entry (module_path = ["some_module"])
    let module_entry = make_plain_struct_in_module(
        ModulePath::from_segments(vec!["some_module".to_string()]).unwrap(),
    );
    doc.types.insert(type_name("ModuleEntry"), module_entry);

    let mermaid = render_acceptance(&[doc], std::slice::from_ref(&layer_id));

    // Layer subgraph id: "mod-layer" → escape_id_component → '-' becomes "_d_" → "mod_d_layer"
    // Subgraph id: "L_mod_d_layer"
    let layer_sg = "L_mod_d_layer";
    // Top-module subgraph id: layer_sg + "_M_" + escape_id_component("some_module").
    // escape_id_component: '_' → "__", so "some_module" → "some__module"
    // Full id: "L_mod_d_layer_M_some__module"
    let module_sg = "L_mod_d_layer_M_some__module";

    // AC-11: top-module subgraph label must be "<layer>::<top_module>" (raw label, not escaped)
    assert!(
        mermaid.contains(&format!("subgraph {module_sg}[\"mod-layer::some_module\"]")),
        "AC-11: top-module subgraph must have label 'mod-layer::some_module', got:\n{mermaid}"
    );

    let root_id = type_node_id(&layer_id, &cn, &type_name("RootEntry"));
    let module_id = type_node_id(&layer_id, &cn, &type_name("ModuleEntry"));

    // RootEntry must appear directly in the layer subgraph (no module prefix in label).
    assert!(
        mermaid.contains(&format!("subgraph {root_id}[\"RootEntry\"]")),
        "AC-11: RootEntry must have label 'RootEntry' (no module prefix), got:\n{mermaid}"
    );

    // ModuleEntry label must include the module path.
    assert!(
        mermaid.contains(&format!("subgraph {module_id}[\"some_module::ModuleEntry\"]")),
        "AC-11: ModuleEntry must have label 'some_module::ModuleEntry', got:\n{mermaid}"
    );

    // Structural ordering: layer subgraph open → module subgraph → module entry subgraph.
    let layer_open = mermaid.find(&format!("subgraph {layer_sg}[")).unwrap();
    let module_open = mermaid.find(&format!("subgraph {module_sg}")).unwrap();
    let module_entry_open = mermaid.find(&format!("subgraph {module_id}")).unwrap();
    let root_open = mermaid.find(&format!("subgraph {root_id}")).unwrap();

    // Layer subgraph must precede all entries.
    assert!(
        layer_open < module_open,
        "AC-11: layer subgraph must precede module subgraph, got:\n{mermaid}"
    );
    assert!(
        layer_open < root_open,
        "AC-11: layer subgraph must precede root entry subgraph, got:\n{mermaid}"
    );

    // Find the "end" that closes the module subgraph — it is the first "end" token
    // appearing after module_open in the output.
    let module_end = {
        let after_module_open = &mermaid[module_open..];
        module_open + after_module_open.find("end").unwrap()
    };

    // Module entry must be inside the module subgraph: after open AND before close.
    assert!(
        module_entry_open > module_open && module_entry_open < module_end,
        "AC-11: ModuleEntry must be inside the top-module subgraph (between open and end), \
         module_open={module_open}, module_entry_open={module_entry_open}, module_end={module_end}, \
         got:\n{mermaid}"
    );
}

// ---------------------------------------------------------------------------
// Test 60 (AC-12): mermaid output ordering: classDef → layer subgraphs →
// edge definitions → class attach lines. No inline ::: syntax (CN-05).
// ---------------------------------------------------------------------------

#[test]
fn test_ac12_mermaid_output_ordering_classdef_subgraphs_edges_class_attach() {
    // Build a fixture that generates at least one entry of each section:
    // - classDef (from [class.*] in style config)
    // - layer subgraph
    // - trait_impl edge
    // - class attach line
    let layer_id = layer("ordered-layer");
    let cn = crate_name("order_crate");

    let mut doc_types = make_minimal_catalogue("ordered-layer", "order_crate");
    let mut doc_traits = make_minimal_catalogue("ordered-layer", "order_crate");

    // A trait that will be implemented.
    doc_traits.traits.insert(trait_name("OrderTrait"), make_trait_entry());

    // A type that implements OrderTrait → produces a -.->|impl| edge.
    let impl_type = TypeEntry {
        action: ItemAction::Add,
        role: DataRole::ValueObject,
        kind: TypeKindV2::PlainStruct {
            fields: vec![],
            has_stripped_fields: false,
            typestate: None,
        },
        methods: vec![],
        trait_impls: vec![TraitImplDeclV2::new(TraitName::new("OrderTrait").unwrap(), cn.clone())],
        module_path: ModulePath::root(),
        docs: None,
        spec_refs: vec![],
        informal_grounds: vec![],
    };
    doc_types.types.insert(type_name("ImplType"), impl_type);

    // Use a style config that includes a classDef so we can detect the ordering.
    let dir = TempDir::new().unwrap();
    let style_content = r##"
[edge.trait_impl]
arrow = '-.->'
label = "impl"

[edge.method_param]
arrow = "--o"

[edge.method_returns]
arrow = "-->"

[edge.field]
arrow = "--o"

[edge.variant_payload]
arrow = "--o"

[edge.alias]
arrow = "---"
label = "alias_of"

[edge.transition]
arrow = "==>"
label = "transitions_to"

[role.ValueObject]
class = "valueObject"

[role.SecondaryPort]
class = "secondaryPort"

[node.Method]
shape = "round"
class = "methodNode"

[node.Variant]
shape = "stadium"
class = "variantNode"

[node.Function]
shape = "subroutine"
class = "functionNode"

[class.valueObject]
fill = "#fff"
stroke = "#000"
stroke_width = "1px"
stroke_dasharray = "0"

[class.secondaryPort]
fill = "#eef"
stroke = "#009"
stroke_width = "2px"
stroke_dasharray = "5"

[pattern.Typestate]
overlay_class = "typestate"

[filter]
include_function_roles = []
"##;
    let path = write_style_config(&dir, style_content);
    let adapter = ContractMapRendererAdapter::new(path);
    let opts = ContractMapRenderOptions::empty();
    let result =
        adapter.render(&[doc_types, doc_traits], std::slice::from_ref(&layer_id), &opts).unwrap();
    let mermaid = result.into_string();

    // AC-12: classDef lines must come first.
    let classdef_pos = mermaid.find("classDef ").unwrap();
    // AC-12: layer subgraph lines must follow classDef.
    let subgraph_pos = mermaid.find("subgraph ").unwrap();
    // AC-12: edge lines must follow subgraphs.
    let edge_pos = mermaid.find("-.->").unwrap();
    // AC-12: class attach lines must come after edges.
    // class attach pattern: "class <id> " (followed by class name)
    let class_attach_pos = {
        // Find the first "class T" or "class R" after the edges section.
        let after_subgraphs = &mermaid[subgraph_pos..];
        let rel = after_subgraphs
            .find("class T")
            .or_else(|| after_subgraphs.find("class R"))
            .expect("at least one class attach line must exist");
        subgraph_pos + rel
    };

    assert!(
        classdef_pos < subgraph_pos,
        "AC-12: classDef lines must precede subgraph lines, got:\n{mermaid}"
    );
    assert!(
        subgraph_pos < edge_pos,
        "AC-12: subgraph lines must precede edge lines, got:\n{mermaid}"
    );
    assert!(
        edge_pos < class_attach_pos,
        "AC-12: edge lines must precede class attach lines, got:\n{mermaid}"
    );

    // AC-12 / CN-05: no inline ::: syntax must appear in the subgraph declarations.
    assert!(
        !mermaid.contains(":::"),
        "AC-12 / CN-05: inline ::: class syntax must not appear in output, got:\n{mermaid}"
    );
}

// ---------------------------------------------------------------------------
// Test 61 (AC-01 + AC-05 + AC-10 + AC-11 integrated): rich multi-crate,
// multi-layer fixture combining all of AC-01, AC-05, AC-10, AC-11 together.
// Two arbitrary layers (layer-a + layer-b), two crates in layer-a, one crate in layer-b.
// ---------------------------------------------------------------------------

#[test]
fn test_integrated_ac01_ac05_ac10_ac11_rich_multi_crate_fixture() {
    // Layers: arbitrary names to verify layer-agnostic rendering (AC-14).
    let layer_a = layer("layer-a");
    let layer_b = layer("layer-b");

    // layer-a: two crates — "core_crate" and "ports_crate"
    let cn_core = crate_name("core_crate");
    let cn_ports = crate_name("ports_crate");
    // layer-b: one crate — "uc_crate"
    let cn_uc = crate_name("uc_crate");

    // ports_crate: declares a SpecificationPort trait (crate root)
    let mut doc_ports = make_minimal_catalogue("layer-a", "ports_crate");
    doc_ports.traits.insert(
        trait_name("LayerPort"),
        make_trait_entry_with_role_and_module(ContractRole::SpecificationPort, ModulePath::root()),
    );

    // core_crate: has two entries
    // (a) crate-root TypeA implementing LayerPort (workspace-internal) and Debug (external)
    // (b) module-path entry TypeB in "events" module
    let mut doc_core = make_minimal_catalogue("layer-a", "core_crate");

    // AC-11: crate-root entry (module_path=[])
    let type_a = TypeEntry {
        action: ItemAction::Add,
        role: DataRole::AggregateRoot,
        kind: TypeKindV2::PlainStruct {
            fields: vec![],
            has_stripped_fields: false,
            typestate: None,
        },
        methods: vec![],
        trait_impls: vec![
            // AC-10: workspace-internal → edge
            TraitImplDeclV2::new(TraitName::new("LayerPort").unwrap(), cn_ports.clone()),
            // AC-10: workspace-external → silent skip
            TraitImplDeclV2::new(TraitName::new("Debug").unwrap(), CrateName::new("std").unwrap()),
        ],
        module_path: ModulePath::root(),
        docs: None,
        spec_refs: vec![],
        informal_grounds: vec![],
    };
    doc_core.types.insert(type_name("TypeA"), type_a);

    // AC-11: module-path entry (module_path=["events"])
    let type_b =
        make_plain_struct_in_module(ModulePath::from_segments(vec!["events".to_string()]).unwrap());
    doc_core.types.insert(type_name("TypeB"), type_b);

    // uc_crate: a type in layer-b also implementing LayerPort cross-layer
    // (trait in layer-a, type in layer-b — cross-layer impl resolution)
    let mut doc_uc = make_minimal_catalogue("layer-b", "uc_crate");
    let uc_type = TypeEntry {
        action: ItemAction::Add,
        role: DataRole::UseCase,
        kind: TypeKindV2::PlainStruct {
            fields: vec![],
            has_stripped_fields: false,
            typestate: None,
        },
        methods: vec![],
        // AC-10: LayerPort is in layer-a's ports_crate — workspace-internal, cross-layer
        trait_impls: vec![TraitImplDeclV2::new(
            TraitName::new("LayerPort").unwrap(),
            cn_ports.clone(),
        )],
        module_path: ModulePath::root(),
        docs: None,
        spec_refs: vec![],
        informal_grounds: vec![],
    };
    doc_uc.types.insert(type_name("UseCaseType"), uc_type);

    let catalogues = vec![doc_core, doc_ports, doc_uc];
    let layer_order = vec![layer_a.clone(), layer_b.clone()];
    let mermaid = render_acceptance(&catalogues, &layer_order);

    // AC-01: exactly one layer-a layer subgraph (two crates → same subgraph).
    // "layer-a" → escape_id_component → '-' becomes "_d_" → "layer_d_a"
    // Subgraph id: "L_layer_d_a"
    let layer_a_sg_count = mermaid.matches("subgraph L_layer_d_a[\"layer-a\"]").count();
    assert_eq!(
        layer_a_sg_count, 1,
        "AC-01: exactly one 'layer-a' layer subgraph must be emitted, got {layer_a_sg_count}:\n{mermaid}"
    );

    // AC-11: top-module subgraph for "events" must appear under layer-a.
    // Module sg id: "L_layer_d_a_M_events"; label: "layer-a::events"
    assert!(
        mermaid.contains("subgraph L_layer_d_a_M_events[\"layer-a::events\"]"),
        "AC-11: events top-module subgraph must appear, got:\n{mermaid}"
    );

    // AC-11: TypeA (crate-root) must appear directly in layer-a (no module subgraph).
    let type_a_id = type_node_id(&layer_a, &cn_core, &type_name("TypeA"));
    assert!(
        mermaid.contains(&format!("subgraph {type_a_id}[\"TypeA\"]")),
        "AC-11: TypeA (crate-root) must appear, got:\n{mermaid}"
    );

    // AC-11: TypeB (events module) must appear inside the events top-module subgraph.
    let type_b_id = type_node_id(&layer_a, &cn_core, &type_name("TypeB"));
    assert!(
        mermaid.contains(&format!("subgraph {type_b_id}[\"events::TypeB\"]")),
        "AC-11: TypeB must have label 'events::TypeB', got:\n{mermaid}"
    );

    // AC-10: workspace-internal LayerPort impl from TypeA → -.->|impl| edge
    let layer_port_id = trait_node_id(&layer_a, &cn_ports, &trait_name("LayerPort"));
    assert!(
        mermaid.contains(&format!("{type_a_id} -.->|impl| {layer_port_id}")),
        "AC-10: TypeA must produce -.->|impl| edge to LayerPort, got:\n{mermaid}"
    );

    // AC-10: workspace-external Debug must NOT appear
    assert!(
        !mermaid.contains("Debug"),
        "AC-10: external trait 'Debug' must not appear in output, got:\n{mermaid}"
    );

    // AC-10: UseCaseType also implements LayerPort cross-layer → edge emitted
    let uc_type_id = type_node_id(&layer_b, &cn_uc, &type_name("UseCaseType"));
    assert!(
        mermaid.contains(&format!("{uc_type_id} -.->|impl| {layer_port_id}")),
        "AC-10: UseCaseType must produce -.->|impl| edge to LayerPort (cross-layer), got:\n{mermaid}"
    );

    // AC-05: all node ids must be distinct
    let ids_in_output =
        vec![type_a_id.clone(), type_b_id.clone(), layer_port_id.clone(), uc_type_id.clone()];
    let mut unique = ids_in_output.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(
        ids_in_output.len(),
        unique.len(),
        "AC-05: all node ids must be distinct: {ids_in_output:?}"
    );
}
