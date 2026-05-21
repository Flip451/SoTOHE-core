//! Layer-agnostic integration tests for the Contract Map render
//! pipeline (ADR 2026-04-17-1528 §4.5, ADR 2026-05-20-2221).
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
//! `catalogue_bulk_loader::load_all_catalogues_native` and render via
//! `ContractMapRendererAdapter` using a temporary minimal style config.
//!
//! The assertions cover:
//!
//! 1. The render call succeeds (style config is valid).
//! 2. The output contains `flowchart LR` (minimal placeholder, T003).
//! 3. No layer names from *other* fixtures leak into the output (guards
//!    against any accidental hard-coded layer name inside the renderer).
//!
//! NOTE: Full subgraph-per-layer assertions will be restored in T005 when
//! the subgraph rendering pipeline is implemented. This file tests the
//! wiring chain (T001–T003) only.

#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use std::path::PathBuf;

use domain::tddd::{ContractMapRenderOptions, ContractMapRenderer};
use infrastructure::tddd::catalogue_bulk_loader::load_all_catalogues_native;
use infrastructure::tddd::contract_map_renderer_adapter::ContractMapRendererAdapter;

fn fixtures_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/architecture_rules")
}

/// Write a minimal valid style config file to a temp dir and return its path.
fn write_minimal_style_config(dir: &std::path::Path) -> PathBuf {
    let path = dir.join("contract-map-style.toml");
    std::fs::write(&path, "[filter]\ninclude_function_roles = []\n").unwrap();
    path
}

fn render_for(fixture: &str, style_config_path: PathBuf) -> String {
    let fixture_dir = fixtures_root().join(fixture);
    let rules_path = fixture_dir.join("architecture-rules.json");
    let track_dir = fixture_dir.join("track_dir");
    // Trust the fixture directory itself — tests fabricate regular
    // files underneath it, so symlink traversal should never fire.
    let (order, catalogues) = load_all_catalogues_native(&track_dir, &rules_path, &fixture_dir)
        .unwrap_or_else(|e| panic!("load_all_catalogues_native failed for {fixture}: {e}"));
    let catalogues_vec: Vec<_> = catalogues.values().cloned().collect();
    let adapter = ContractMapRendererAdapter::new(style_config_path);
    let opts = ContractMapRenderOptions::empty();
    let content = adapter
        .render(&catalogues_vec, &order, &opts)
        .unwrap_or_else(|e| panic!("render failed for {fixture}: {e}"));
    content.into_string()
}

// ---- fixture_2layers ---------------------------------------------------

#[test]
fn test_fixture_2layers_render_succeeds_with_valid_style_config() {
    let tmp = tempfile::tempdir().unwrap();
    let style_path = write_minimal_style_config(tmp.path());
    let out = render_for("fixture_2layers", style_path);
    assert!(out.contains("flowchart LR"), "render must contain flowchart LR; got:\n{out}");
}

#[test]
fn test_fixture_2layers_does_not_leak_foreign_layer_names_in_placeholder() {
    let tmp = tempfile::tempdir().unwrap();
    let style_path = write_minimal_style_config(tmp.path());
    let out = render_for("fixture_2layers", style_path);
    // T003 placeholder: no subgraphs yet. Verify no foreign layer names are hardcoded.
    for foreign in &["domain", "usecase", "infrastructure", "application", "port", "gateway"] {
        assert!(
            !out.contains(&format!("subgraph {foreign}")),
            "output must not render foreign layer '{foreign}'; got:\n{out}"
        );
    }
}

// ---- fixture_3layers_default -------------------------------------------

#[test]
fn test_fixture_3layers_default_render_succeeds_with_valid_style_config() {
    let tmp = tempfile::tempdir().unwrap();
    let style_path = write_minimal_style_config(tmp.path());
    let out = render_for("fixture_3layers_default", style_path);
    assert!(out.contains("flowchart LR"), "render must contain flowchart LR; got:\n{out}");
}

#[test]
fn test_fixture_3layers_default_does_not_leak_foreign_layer_names_in_placeholder() {
    let tmp = tempfile::tempdir().unwrap();
    let style_path = write_minimal_style_config(tmp.path());
    let out = render_for("fixture_3layers_default", style_path);
    for foreign in &["core", "adapter", "application", "port", "gateway"] {
        assert!(
            !out.contains(&format!("subgraph {foreign}")),
            "output must not render foreign layer '{foreign}'; got:\n{out}"
        );
    }
}

// ---- fixture_custom_names ----------------------------------------------

#[test]
fn test_fixture_custom_names_render_succeeds_with_valid_style_config() {
    let tmp = tempfile::tempdir().unwrap();
    let style_path = write_minimal_style_config(tmp.path());
    let out = render_for("fixture_custom_names", style_path);
    assert!(out.contains("flowchart LR"), "render must contain flowchart LR; got:\n{out}");
}

#[test]
fn test_fixture_custom_names_does_not_leak_foreign_layer_names_in_placeholder() {
    let tmp = tempfile::tempdir().unwrap();
    let style_path = write_minimal_style_config(tmp.path());
    let out = render_for("fixture_custom_names", style_path);
    for foreign in &["core", "adapter", "domain", "usecase", "infrastructure"] {
        assert!(
            !out.contains(&format!("subgraph {foreign}")),
            "output must not render foreign layer '{foreign}'; got:\n{out}"
        );
    }
}
