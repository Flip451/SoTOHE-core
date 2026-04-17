//! Layer-agnostic integration tests for the Contract Map render
//! pipeline (ADR 2026-04-17-1528 §4.5).
//!
//! Three architecture fixtures exercise every layer-name degree of
//! freedom the renderer is supposed to support:
//!
//! * `fixture_2layers` — a 2-layer template (`core` / `adapter`) that
//!   the default SoTOHE-core layering does not include.
//! * `fixture_3layers_default` — the real SoTOHE-core layering
//!   (`domain` / `usecase` / `infrastructure`) as a baseline.
//! * `fixture_custom_names` — a 3-layer template that renames every
//!   layer (`port` / `application` / `gateway`).
//!
//! For each fixture, we load the catalogues via
//! `catalogue_bulk_loader::load_all_catalogues` and render via
//! `domain::tddd::render_contract_map`. The assertions cover:
//!
//! 1. One subgraph per `tddd.enabled` layer, labelled with the fixture's
//!    `layers[].crate` value verbatim.
//! 2. Subgraph order matches the `may_depend_on` topological sort (no
//!    dependencies first).
//! 3. No layer names from *other* fixtures leak into the output (guards
//!    against any accidental hard-coded layer name inside the renderer).

#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use std::path::PathBuf;

use domain::tddd::{ContractMapRenderOptions, render_contract_map};
use infrastructure::tddd::catalogue_bulk_loader::load_all_catalogues;

fn fixtures_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/architecture_rules")
}

fn render_for(fixture: &str) -> String {
    let fixture_dir = fixtures_root().join(fixture);
    let rules_path = fixture_dir.join("architecture-rules.json");
    let track_dir = fixture_dir.join("track_dir");
    // Trust the fixture directory itself — tests fabricate regular
    // files underneath it, so symlink traversal should never fire.
    let (order, catalogues) = load_all_catalogues(&track_dir, &rules_path, &fixture_dir)
        .unwrap_or_else(|e| panic!("load_all_catalogues failed for {fixture}: {e}"));
    let content = render_contract_map(&catalogues, &order, &ContractMapRenderOptions::empty());
    content.into_string()
}

fn subgraph_position(haystack: &str, layer: &str) -> usize {
    haystack
        .find(&format!("subgraph {layer} [{layer}]"))
        .unwrap_or_else(|| panic!("expected subgraph '{layer}' in output; got:\n{haystack}"))
}

fn assert_no_foreign_layers(output: &str, foreign: &[&str]) {
    for layer in foreign {
        let needle = format!("subgraph {layer} [{layer}]");
        assert!(
            !output.contains(&needle),
            "output must not render foreign layer '{layer}'; got:\n{output}"
        );
    }
}

// ---- fixture_2layers ---------------------------------------------------

#[test]
fn test_fixture_2layers_emits_subgraph_per_enabled_layer() {
    let out = render_for("fixture_2layers");
    let core_pos = subgraph_position(&out, "core");
    let adapter_pos = subgraph_position(&out, "adapter");
    assert!(core_pos > 0);
    assert!(adapter_pos > 0);
    assert_eq!(
        out.matches("subgraph ").count(),
        2,
        "fixture_2layers must emit exactly 2 subgraphs; got:\n{out}"
    );
}

#[test]
fn test_fixture_2layers_respects_may_depend_on_topological_order() {
    let out = render_for("fixture_2layers");
    let core_pos = subgraph_position(&out, "core");
    let adapter_pos = subgraph_position(&out, "adapter");
    assert!(
        core_pos < adapter_pos,
        "core (no deps) must appear before adapter (depends on core); got:\n{out}"
    );
}

#[test]
fn test_fixture_2layers_does_not_leak_other_fixture_layer_names() {
    let out = render_for("fixture_2layers");
    assert_no_foreign_layers(
        &out,
        &["domain", "usecase", "infrastructure", "application", "port", "gateway"],
    );
}

// ---- fixture_3layers_default -------------------------------------------

#[test]
fn test_fixture_3layers_default_emits_subgraph_per_enabled_layer() {
    let out = render_for("fixture_3layers_default");
    let domain_pos = subgraph_position(&out, "domain");
    let usecase_pos = subgraph_position(&out, "usecase");
    let infra_pos = subgraph_position(&out, "infrastructure");
    assert!(domain_pos > 0 && usecase_pos > 0 && infra_pos > 0);
    assert_eq!(
        out.matches("subgraph ").count(),
        3,
        "fixture_3layers_default must emit exactly 3 subgraphs; got:\n{out}"
    );
}

#[test]
fn test_fixture_3layers_default_respects_may_depend_on_topological_order() {
    let out = render_for("fixture_3layers_default");
    let domain_pos = subgraph_position(&out, "domain");
    let usecase_pos = subgraph_position(&out, "usecase");
    let infra_pos = subgraph_position(&out, "infrastructure");
    assert!(domain_pos < usecase_pos, "domain must appear before usecase");
    assert!(usecase_pos < infra_pos, "usecase must appear before infrastructure");
}

#[test]
fn test_fixture_3layers_default_does_not_leak_other_fixture_layer_names() {
    let out = render_for("fixture_3layers_default");
    assert_no_foreign_layers(&out, &["core", "adapter", "application", "port", "gateway"]);
}

// ---- fixture_custom_names ----------------------------------------------

#[test]
fn test_fixture_custom_names_emits_subgraph_per_enabled_layer() {
    let out = render_for("fixture_custom_names");
    let port_pos = subgraph_position(&out, "port");
    let app_pos = subgraph_position(&out, "application");
    let gateway_pos = subgraph_position(&out, "gateway");
    assert!(port_pos > 0 && app_pos > 0 && gateway_pos > 0);
    assert_eq!(
        out.matches("subgraph ").count(),
        3,
        "fixture_custom_names must emit exactly 3 subgraphs; got:\n{out}"
    );
}

#[test]
fn test_fixture_custom_names_respects_may_depend_on_topological_order() {
    let out = render_for("fixture_custom_names");
    let port_pos = subgraph_position(&out, "port");
    let app_pos = subgraph_position(&out, "application");
    let gateway_pos = subgraph_position(&out, "gateway");
    assert!(port_pos < app_pos, "port (no deps) must appear before application");
    assert!(app_pos < gateway_pos, "application must appear before gateway");
}

#[test]
fn test_fixture_custom_names_does_not_leak_other_fixture_layer_names() {
    let out = render_for("fixture_custom_names");
    assert_no_foreign_layers(&out, &["core", "adapter", "domain", "usecase", "infrastructure"]);
}
