//! Unit tests for `contract_map_render` (OS-06 stub).
//!
//! T008: The full v3 rendering pipeline is OS-06 deferred (T012).
//! These tests verify the stub behaviour: the renderer accepts
//! `TypeCatalogueDocument` (the v3→v2 stub produced by `catalogue_bulk_loader`)
//! and emits:
//! 1. A generated-file marker (CN-08).
//! 2. An OS-06 deferment comment.
//! 3. A `flowchart LR` block with one empty subgraph per active layer.
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
use crate::tddd::catalogue::{
    TypeAction, TypeCatalogueDocument, TypeCatalogueEntry, TypeDefinitionKind,
};
use crate::tddd::contract_map_options::ContractMapRenderOptions;

fn layer(name: &str) -> LayerId {
    LayerId::try_new(name.to_owned()).unwrap()
}

fn empty_doc() -> TypeCatalogueDocument {
    TypeCatalogueDocument::new(2, vec![])
}

fn doc_with_entries(entries: Vec<TypeCatalogueEntry>) -> TypeCatalogueDocument {
    TypeCatalogueDocument::new(2, entries)
}

fn entry(name: &str, kind: TypeDefinitionKind) -> TypeCatalogueEntry {
    TypeCatalogueEntry::new(name, format!("{name} description"), kind, TypeAction::Add, true)
        .unwrap()
}

fn simple_3layer_catalogues() -> (BTreeMap<LayerId, TypeCatalogueDocument>, Vec<LayerId>) {
    let domain = layer("domain");
    let usecase = layer("usecase");
    let infra = layer("infrastructure");

    let domain_doc = doc_with_entries(vec![entry(
        "User",
        TypeDefinitionKind::ValueObject {
            expected_members: Vec::new(),
            expected_methods: Vec::new(),
        },
    )]);
    let usecase_doc = doc_with_entries(vec![entry(
        "RegisterUser",
        TypeDefinitionKind::UseCase { expected_members: Vec::new(), expected_methods: Vec::new() },
    )]);
    let infra_doc = empty_doc();

    let mut catalogues: BTreeMap<LayerId, TypeCatalogueDocument> = BTreeMap::new();
    catalogues.insert(domain.clone(), domain_doc);
    catalogues.insert(usecase.clone(), usecase_doc);
    catalogues.insert(infra.clone(), infra_doc);

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
fn test_render_contract_map_emits_os06_deferment_comment() {
    let (catalogues, order) = simple_3layer_catalogues();
    let content = render_contract_map(&catalogues, &order, &ContractMapRenderOptions::empty());
    let text = content.as_ref();
    assert!(
        text.contains("OS-06 DEFERRED"),
        "output must contain OS-06 deferment comment; got:\n{text}"
    );
    assert!(text.contains("T012"), "output must reference T012 follow-up; got:\n{text}");
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
fn test_render_contract_map_entry_count_comment() {
    // OS-06 stub: entries are still counted via TypeCatalogueDocument stub
    // produced by catalogue_bulk_loader for observability.
    let (catalogues, order) = simple_3layer_catalogues();
    let content = render_contract_map(&catalogues, &order, &ContractMapRenderOptions::empty());
    let text = content.as_ref();
    // domain has 1 entry, usecase has 1 entry; infrastructure has 0 (no comment).
    assert!(
        text.contains("1 entries (nodes deferred to T012)"),
        "stub must emit entry count comment; got:\n{text}"
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
    let mut catalogues: BTreeMap<LayerId, TypeCatalogueDocument> = BTreeMap::new();
    let only = layer("domain");
    catalogues.insert(only.clone(), empty_doc());
    let order = vec![only];

    let content = render_contract_map(&catalogues, &order, &ContractMapRenderOptions::empty());
    let text = content.as_ref();
    assert!(text.contains("flowchart LR"));
    assert!(text.contains("subgraph domain [domain]"));
    assert!(!text.contains("subgraph usecase"), "no usecase subgraph expected");
}

#[test]
fn test_render_contract_map_no_edges_emitted_by_stub() {
    // T012: edges are deferred. The stub must not emit any mermaid edge syntax.
    // Note: `-->` appears in HTML comment close markers (`-->`); we check for
    // mermaid-specific edge patterns (with `|` label delimiters) instead.
    let (catalogues, order) = simple_3layer_catalogues();
    let content = render_contract_map(&catalogues, &order, &ContractMapRenderOptions::empty());
    let text = content.as_ref();
    assert!(!text.contains("-->|"), "stub must not emit method-call edges (OS-06)");
    assert!(!text.contains("-.impl.->"), "stub must not emit trait-impl edges (OS-06)");
    assert!(!text.contains("==>|"), "stub must not emit typestate transition edges (OS-06)");
}

#[test]
fn test_render_contract_map_empty_layer_order_produces_minimal_scaffold() {
    let catalogues: BTreeMap<LayerId, TypeCatalogueDocument> = BTreeMap::new();
    let order: Vec<LayerId> = Vec::new();
    let content = render_contract_map(&catalogues, &order, &ContractMapRenderOptions::empty());
    let text = content.as_ref();
    // Minimal output: header + deferment comment + empty flowchart.
    assert!(text.contains("flowchart LR"));
    assert!(text.contains("OS-06 DEFERRED"));
    assert!(!text.contains("subgraph"));
}
