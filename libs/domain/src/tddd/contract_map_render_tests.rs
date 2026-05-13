//! Unit tests for `contract_map_render` (IN-24 minimal placeholder).
//!
//! T025: The renderer is now v3-native — it takes `CatalogueDocument` directly
//! (no v3→v2 conversion). These tests verify the placeholder behaviour:
//! 1. A generated-file marker (CN-08).
//! 2. An IN-24 / OS-07 deferral comment.
//! 3. A `flowchart LR` block with one `subgraph` per active layer, each
//!    listing entry names as `%% type/trait/fn:` comments for observability.
//!
//! This file is compiled only under `#[cfg(test)]` via the `#[path]` attribute
//! in the parent module:
//! ```text
//! #[cfg(test)]
//! #[path = "contract_map_render_tests.rs"]
//! mod tests;
//! ```

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]

use std::collections::BTreeMap;

use super::*;
use crate::tddd::LayerId;
use crate::tddd::catalogue_v2::composite::TypeKindV2;
use crate::tddd::catalogue_v2::document::CatalogueDocument;
use crate::tddd::catalogue_v2::entries::{FunctionEntry, TraitEntry, TypeEntry};
use crate::tddd::catalogue_v2::identifiers::{
    CrateName, FunctionName, FunctionPath, MethodName, ModulePath, TraitName, TypeName, TypeRef,
};
use crate::tddd::catalogue_v2::methods::MethodDeclaration;
use crate::tddd::catalogue_v2::roles::{
    ContractRole, DataRole, FunctionRole, ItemAction, SelfReceiver,
};
use crate::tddd::contract_map_options::ContractMapRenderOptions;

fn layer(name: &str) -> LayerId {
    LayerId::try_new(name.to_owned()).unwrap()
}

fn empty_doc(crate_name: &str) -> CatalogueDocument {
    CatalogueDocument::new(3, CrateName::new(crate_name).unwrap(), layer(crate_name))
}

fn doc_with_type(crate_name: &str, type_name: &str) -> CatalogueDocument {
    let mut doc = empty_doc(crate_name);
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
        module_path: ModulePath::root(),
        docs: None,
        spec_refs: vec![],
        informal_grounds: vec![],
    };
    doc.types.insert(TypeName::new(type_name).unwrap(), entry);
    doc
}

fn doc_with_trait(crate_name: &str, trait_name: &str) -> CatalogueDocument {
    let mut doc = empty_doc(crate_name);
    let method = MethodDeclaration::new(
        MethodName::new("load").unwrap(),
        Some(SelfReceiver::SharedRef),
        vec![],
        TypeRef::new("()").unwrap(),
        false,
        None,
    );
    let entry = TraitEntry {
        action: ItemAction::Add,
        role: ContractRole::SecondaryPort,
        methods: vec![method],
        supertrait_bounds: vec![],
        module_path: ModulePath::root(),
        docs: None,
        spec_refs: vec![],
        informal_grounds: vec![],
    };
    doc.traits.insert(TraitName::new(trait_name).unwrap(), entry);
    doc
}

fn doc_with_function(crate_name: &str, fn_name: &str) -> CatalogueDocument {
    let mut doc = empty_doc(crate_name);
    let crate_n = CrateName::new(crate_name).unwrap();
    let fn_path = FunctionPath::at_root(crate_n, FunctionName::new(fn_name).unwrap());
    let entry = FunctionEntry {
        action: ItemAction::Add,
        role: FunctionRole::FreeFunction,
        params: vec![],
        returns: TypeRef::new("()").unwrap(),
        is_async: false,
        generics: vec![],
        docs: None,
        spec_refs: vec![],
        informal_grounds: vec![],
    };
    doc.functions.insert(fn_path, entry);
    doc
}

fn simple_3layer_catalogues() -> (BTreeMap<LayerId, CatalogueDocument>, Vec<LayerId>) {
    let domain = layer("domain");
    let usecase = layer("usecase");
    let infra = layer("infrastructure");

    let mut catalogues: BTreeMap<LayerId, CatalogueDocument> = BTreeMap::new();
    catalogues.insert(domain.clone(), doc_with_type("domain", "User"));
    catalogues.insert(usecase.clone(), doc_with_trait("usecase", "UserRepository"));
    catalogues.insert(infra.clone(), empty_doc("infrastructure"));

    let layer_order = vec![domain, usecase, infra];
    (catalogues, layer_order)
}

#[test]
fn test_render_contract_map_emits_fenced_mermaid_block() {
    let (catalogues, order) = simple_3layer_catalogues();
    let content = render_contract_map(&catalogues, &order, &ContractMapRenderOptions::empty());
    let text = content.as_ref();
    // CN-08: output starts with the generated marker comment.
    assert!(
        text.starts_with("<!-- Generated contract-map-renderer — DO NOT EDIT DIRECTLY -->"),
        "output must start with generated marker; got:\n{text}"
    );
    assert!(text.contains("```mermaid\n"), "output must contain mermaid fence");
    assert!(text.trim_end().ends_with("```"), "output must end with closing fence");
    assert!(text.contains("flowchart LR"));
}

#[test]
fn test_render_contract_map_emits_in24_os07_deferral_comment() {
    let (catalogues, order) = simple_3layer_catalogues();
    let content = render_contract_map(&catalogues, &order, &ContractMapRenderOptions::empty());
    let text = content.as_ref();
    assert!(text.contains("IN-24"), "output must contain IN-24 deferral reference; got:\n{text}");
    assert!(text.contains("OS-07"), "output must contain OS-07 deferral reference; got:\n{text}");
}

#[test]
fn test_render_contract_map_produces_subgraph_per_layer_in_order() {
    let (catalogues, order) = simple_3layer_catalogues();
    let content = render_contract_map(&catalogues, &order, &ContractMapRenderOptions::empty());
    let text = content.as_ref();
    let domain_pos = text.find("subgraph domain [domain]").unwrap();
    let usecase_pos = text.find("subgraph usecase [usecase]").unwrap();
    let infra_pos = text.find("subgraph infrastructure [infrastructure]").unwrap();
    assert!(domain_pos < usecase_pos, "domain must appear before usecase");
    assert!(usecase_pos < infra_pos, "usecase must appear before infrastructure");
}

#[test]
fn test_render_contract_map_lists_type_entry_names_as_comments() {
    let crate_name = "domain";
    let mut catalogues: BTreeMap<LayerId, CatalogueDocument> = BTreeMap::new();
    let l = layer(crate_name);
    catalogues.insert(l.clone(), doc_with_type(crate_name, "User"));
    let order = vec![l];

    let content = render_contract_map(&catalogues, &order, &ContractMapRenderOptions::empty());
    let text = content.as_ref();
    assert!(
        text.contains("%% type: User"),
        "output must list type entry 'User' as comment; got:\n{text}"
    );
}

#[test]
fn test_render_contract_map_lists_trait_entry_names_as_comments() {
    let crate_name = "domain";
    let mut catalogues: BTreeMap<LayerId, CatalogueDocument> = BTreeMap::new();
    let l = layer(crate_name);
    catalogues.insert(l.clone(), doc_with_trait(crate_name, "UserRepository"));
    let order = vec![l];

    let content = render_contract_map(&catalogues, &order, &ContractMapRenderOptions::empty());
    let text = content.as_ref();
    assert!(
        text.contains("%% trait: UserRepository"),
        "output must list trait entry 'UserRepository' as comment; got:\n{text}"
    );
}

#[test]
fn test_render_contract_map_lists_function_entry_paths_as_comments() {
    let crate_name = "domain";
    let mut catalogues: BTreeMap<LayerId, CatalogueDocument> = BTreeMap::new();
    let l = layer(crate_name);
    catalogues.insert(l.clone(), doc_with_function(crate_name, "register_user"));
    let order = vec![l];

    let content = render_contract_map(&catalogues, &order, &ContractMapRenderOptions::empty());
    let text = content.as_ref();
    assert!(text.contains("%% fn:"), "output must list function entry as comment; got:\n{text}");
    assert!(
        text.contains("register_user"),
        "output must contain function name 'register_user'; got:\n{text}"
    );
}

#[test]
fn test_render_contract_map_layer_filter_restricts_subgraphs() {
    let (catalogues, order) = simple_3layer_catalogues();
    let opts = ContractMapRenderOptions { layers: vec![layer("domain")], ..Default::default() };
    let content = render_contract_map(&catalogues, &order, &opts);
    let text = content.as_ref();
    assert!(text.contains("subgraph domain [domain]"), "domain subgraph must be present");
    assert!(!text.contains("subgraph usecase"), "usecase must be filtered out");
    assert!(!text.contains("subgraph infrastructure"), "infrastructure must be filtered out");
}

#[test]
fn test_render_contract_map_single_layer_architecture() {
    let mut catalogues: BTreeMap<LayerId, CatalogueDocument> = BTreeMap::new();
    let only = layer("domain");
    catalogues.insert(only.clone(), empty_doc("domain"));
    let order = vec![only];

    let content = render_contract_map(&catalogues, &order, &ContractMapRenderOptions::empty());
    let text = content.as_ref();
    assert!(text.contains("flowchart LR"));
    assert!(text.contains("subgraph domain [domain]"));
    assert!(!text.contains("subgraph usecase"), "no usecase subgraph expected");
}

#[test]
fn test_render_contract_map_no_edges_emitted_by_placeholder() {
    // IN-24: edges are deferred. The placeholder must not emit any mermaid edge syntax.
    let (catalogues, order) = simple_3layer_catalogues();
    let content = render_contract_map(&catalogues, &order, &ContractMapRenderOptions::empty());
    let text = content.as_ref();
    assert!(!text.contains("-->|"), "placeholder must not emit method-call edges (IN-24)");
    assert!(!text.contains("-.impl.->"), "placeholder must not emit trait-impl edges (IN-24)");
    assert!(!text.contains("==>|"), "placeholder must not emit typestate transition edges (IN-24)");
}

#[test]
fn test_render_contract_map_empty_layer_order_produces_minimal_scaffold() {
    let catalogues: BTreeMap<LayerId, CatalogueDocument> = BTreeMap::new();
    let order: Vec<LayerId> = Vec::new();
    let content = render_contract_map(&catalogues, &order, &ContractMapRenderOptions::empty());
    let text = content.as_ref();
    // Minimal output: header + deferral comment + empty flowchart.
    assert!(text.contains("flowchart LR"));
    assert!(text.contains("IN-24"));
    assert!(!text.contains("subgraph"));
}

#[test]
fn test_render_contract_map_does_not_reference_os06_or_t012() {
    // Verify that old stale deferral references are gone.
    let (catalogues, order) = simple_3layer_catalogues();
    let content = render_contract_map(&catalogues, &order, &ContractMapRenderOptions::empty());
    let text = content.as_ref();
    assert!(!text.contains("OS-06"), "output must NOT reference stale OS-06; got:\n{text}");
    assert!(!text.contains("T012"), "output must NOT reference stale T012; got:\n{text}");
}
