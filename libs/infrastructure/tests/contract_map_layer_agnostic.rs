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

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]

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

// ---- T009 / IN-10 / AC-08: Phase 2 features layer-agnostic check -----

use std::collections::BTreeMap;

use domain::ConfidenceSignal;
use domain::tddd::LayerId;
use domain::tddd::catalogue::{
    MemberDeclaration, MethodDeclaration, ParamDeclaration, TypeAction, TypeCatalogueDocument,
    TypeCatalogueEntry, TypeDefinitionKind, TypeSignal,
};

/// Build a fixture-agnostic catalogue set covering every Phase 2 renderer
/// feature: FreeFunction node (IN-01), Interactor `-.impl.->` edge (IN-02),
/// Field-derived edges (IN-05), action overlay (IN-06), signal overlay
/// (IN-07). All entries live in the first layer in `layer_names`; the
/// renderer is invoked with both overlay flags on.
///
/// The helper exists so each fixture can run the same Phase 2 assertion
/// suite against its own layer naming, proving the renderer never
/// hardcodes `domain` / `usecase` / `infrastructure`.
fn render_phase2_features_for(layer_names: &[&str]) -> String {
    let layers: Vec<LayerId> = layer_names
        .iter()
        .map(|n| LayerId::try_new((*n).to_owned()).expect("valid layer id"))
        .collect();
    let primary = layers[0].clone();

    let entries = vec![
        // IN-01: FreeFunction kind node.
        TypeCatalogueEntry::new(
            "do_thing",
            "free function",
            TypeDefinitionKind::FreeFunction {
                expected_params: vec![ParamDeclaration::new("x", "ThingId")],
                expected_returns: vec!["Outcome".to_owned()],
            },
            TypeAction::Add,
            true,
        )
        .unwrap(),
        // IN-02: Interactor with declares_application_service → impl edge.
        TypeCatalogueEntry::new(
            "MyService",
            "primary port",
            TypeDefinitionKind::ApplicationService {
                expected_methods: vec![MethodDeclaration::new(
                    "execute",
                    Some("&self".into()),
                    vec![],
                    "()",
                    false,
                )],
            },
            TypeAction::Add,
            true,
        )
        .unwrap(),
        TypeCatalogueEntry::new(
            "MyServiceImpl",
            "interactor implementing MyService",
            TypeDefinitionKind::Interactor {
                declares_application_service: Some("MyService".to_owned()),
            },
            TypeAction::Modify,
            true,
        )
        .unwrap(),
        // action=Reference fixture entry — referenced by IN-06 action
        // overlay assertion below (`reference_action` annotation).
        TypeCatalogueEntry::new(
            "ForwardRef",
            "reference-action sample for IN-06 overlay test",
            TypeDefinitionKind::ValueObject,
            TypeAction::Reference,
            true,
        )
        .unwrap(),
        // action=Modify + Yellow signal fixture — referenced by IN-06
        // (modify_action) and IN-07 (yellow_signal) overlay assertions below.
        TypeCatalogueEntry::new(
            "ValidationErr",
            "modify-action + yellow-signal sample for IN-06 / IN-07 overlay tests",
            TypeDefinitionKind::SecondaryPort { expected_methods: vec![] },
            TypeAction::Modify,
            true,
        )
        .unwrap(),
        // IN-05: Dto with Field referencing ThingId → field edge.
        TypeCatalogueEntry::new(
            "ThingDto",
            "dto with field",
            TypeDefinitionKind::Dto,
            TypeAction::Add,
            true,
        )
        .unwrap()
        .with_members(vec![MemberDeclaration::field("id", "ThingId")])
        .unwrap(),
        // ThingId — referenced by both the FreeFunction param and the
        // Dto field. This entry intentionally lacks an action override to
        // verify Add-default keeps no `:::action` suffix.
        TypeCatalogueEntry::new(
            "ThingId",
            "value object referenced by free fn + dto field",
            TypeDefinitionKind::ValueObject,
            TypeAction::Add,
            true,
        )
        .unwrap(),
    ];

    // Attach a Yellow signal to ValidationErr to exercise IN-07 signal
    // overlay alongside the IN-06 action overlay below.
    // Signal kind_tag must match the entry's kind_tag — PR #115 fix made
    // signal overlay lookup `(type_name, kind_tag)`-keyed.
    let signals = vec![TypeSignal::new(
        "ValidationErr",
        "secondary_port",
        ConfidenceSignal::Yellow,
        true,
        vec![],
        vec![],
        vec![],
    )];

    let mut doc = TypeCatalogueDocument::new(2, entries);
    doc.set_signals(signals);
    let mut catalogues: BTreeMap<LayerId, TypeCatalogueDocument> = BTreeMap::new();
    catalogues.insert(primary, doc);

    let opts = ContractMapRenderOptions {
        action_overlay: true,
        signal_overlay: true,
        ..Default::default()
    };
    let content = render_contract_map(&catalogues, &layers, &opts);
    content.into_string()
}

/// Mirror the renderer's `sanitize_id` function so tests can compute
/// layer-specific mermaid node identifiers without importing domain internals.
///
/// Rules (identical to the renderer):
/// - ASCII alphanumeric characters pass through unchanged.
/// - `_` is escaped as `__`.
/// - Any other code point is encoded as `_<hex>_`.
/// - Empty input maps to `_`.
fn sanitize_id_test(raw: &str) -> String {
    if raw.is_empty() {
        return "_".to_owned();
    }
    let mut out = String::with_capacity(raw.len());
    for ch in raw.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch);
        } else if ch == '_' {
            out.push_str("__");
        } else {
            use std::fmt::Write as _;
            let _ = write!(out, "_{:x}_", ch as u32);
        }
    }
    out
}

/// Compute the mermaid node identifier for `name` in `layer`, mirroring the
/// renderer's `node_id` function: `L{sanitized_layer_len}_{sanitized_layer}_{sanitized_name}`.
fn node_id_test(layer: &str, name: &str) -> String {
    let l = sanitize_id_test(layer);
    let n = sanitize_id_test(name);
    format!("L{}_{l}_{n}", l.len())
}

fn assert_phase2_features_present(out: &str, primary_layer: &str) {
    let nid = |name: &str| node_id_test(primary_layer, name);

    // IN-01: free_function classDef + full node-shape line for `do_thing`.
    // Shape: `{id}[do_thing]:::free_function` (FreeFunction + action=Add → no
    // extra action suffix).
    assert!(out.contains("classDef free_function"), "free_function classDef must appear");
    let do_thing_id = nid("do_thing");
    assert!(
        out.contains(&format!("{do_thing_id}[do_thing]:::free_function")),
        "FreeFunction node must use shape {do_thing_id}[do_thing]:::free_function; got:\n{out}"
    );
    // IN-02: Interactor → ApplicationService impl edge with full node ids.
    // Edge format: `    {impl_id} -.impl.-> {svc_id}` (4-space indent).
    let impl_id = nid("MyServiceImpl");
    let svc_id = nid("MyService");
    assert!(
        out.contains(&format!("{impl_id} -.impl.-> {svc_id}")),
        "Interactor impl edge `{impl_id} -.impl.-> {svc_id}` must appear; got:\n{out}"
    );
    // Compute node ids for `ForwardRef` and `ValidationErr` for use by
    // IN-06 (action overlay) and IN-07 (signal overlay) assertions below.
    let fwd_id = nid("ForwardRef");
    let verr_id = nid("ValidationErr");
    // IN-05: full field-edge line from ThingDto to ThingId.
    // Edge label is `escape_edge_label(".id")` = `"\".id\""`.
    // Full edge: `    {dto_id} -->|".id"| {thing_id}` (4-space indent).
    let dto_id = nid("ThingDto");
    let thing_id = nid("ThingId");
    assert!(
        out.contains(&format!("{dto_id} -->|\".id\"| {thing_id}")),
        "field edge `{dto_id} -->\".id\" {thing_id}` must appear; got:\n{out}"
    );
    // IN-06: action overlay classDefs + annotations on specific nodes.
    // `MyServiceImpl` has action=Modify → its shape ends with `:::modify_action`.
    // `ForwardRef` has action=Reference → its shape ends with `:::reference_action`.
    assert!(out.contains("classDef modify_action"), "modify_action classDef must appear");
    assert!(out.contains("classDef reference_action"), "reference_action classDef must appear");
    assert!(
        out.contains(&format!("{impl_id}[\\MyServiceImpl/]:::modify_action")),
        "modify_action must be applied to {impl_id}; got:\n{out}"
    );
    assert!(
        out.contains(&format!("{fwd_id}(ForwardRef):::reference_action")),
        "reference_action must be applied to {fwd_id}; got:\n{out}"
    );
    // IN-07: signal overlay classDef + yellow_signal inline annotation on
    // ValidationErr. The renderer appends `:::yellow_signal` to the shape
    // string (not a separate class line), so we check the node shape directly.
    // `ValidationErr` has action=Modify and Yellow signal → shape ends with
    // `:::modify_action:::yellow_signal`.
    assert!(out.contains("classDef yellow_signal"), "yellow_signal classDef must appear");
    // ValidationErr is a SecondaryPort — shape is `[[ValidationErr]]` (subroutine).
    assert!(
        out.contains(&format!("{verr_id}[[ValidationErr]]:::modify_action:::yellow_signal")),
        "yellow_signal must be applied inline to {verr_id}; got:\n{out}"
    );
}

#[test]
fn test_fixture_2layers_phase2_features_layer_agnostic() {
    let out = render_phase2_features_for(&["core", "adapter"]);
    assert_phase2_features_present(&out, "core");
    assert_no_foreign_layers(
        &out,
        &["domain", "usecase", "infrastructure", "application", "port", "gateway"],
    );
}

#[test]
fn test_fixture_3layers_default_phase2_features_layer_agnostic() {
    let out = render_phase2_features_for(&["domain", "usecase", "infrastructure"]);
    assert_phase2_features_present(&out, "domain");
    assert_no_foreign_layers(&out, &["core", "adapter", "application", "port", "gateway"]);
}

#[test]
fn test_fixture_custom_names_phase2_features_layer_agnostic() {
    let out = render_phase2_features_for(&["port", "application", "gateway"]);
    assert_phase2_features_present(&out, "port");
    assert_no_foreign_layers(&out, &["core", "adapter", "domain", "usecase", "infrastructure"]);
}
