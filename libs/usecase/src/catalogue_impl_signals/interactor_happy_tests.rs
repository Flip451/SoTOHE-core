//! Happy-path and report-format tests for `CatalogueImplSignalsInteractor`.
//!
//! These tests use `EmptyEvaluator` + `EmptyRustdocPort` to drive the interactor
//! through the full port-wiring path without invoking real cargo/rustdoc.
//!
//! Loaded as `mod happy_tests;` from `interactor_tests.rs` so the test helpers
//! and mock ports defined there are shared via `use super::*`.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::Arc;

use domain::tddd::catalogue_v2::{CatalogueDocument, TdddLayerBinding};
use domain::tddd::extended_crate::ExtendedCrate;

use super::super::super::service::{CatalogueImplSignalsError, CatalogueImplSignalsService};
use super::{
    EmptyEvaluator, EmptyRustdocPort, FailingEvaluator, FailingRustdocPort, SingleBlueEvaluator,
    SingleRedEvaluator, StubLayerBindings, StubLoader, build_interactor, empty_rustdoc_crate,
    minimal_catalogue_doc, stub_binding,
};

// -------------------------------------------------------------------------
// EmptyExtendedCrateCodec — only needed for happy-path tests
// -------------------------------------------------------------------------

/// `CatalogueToExtendedCratePort` that returns an empty `ExtendedCrate`.
struct EmptyExtendedCrateCodec;

impl domain::tddd::CatalogueToExtendedCratePort for EmptyExtendedCrateCodec {
    fn encode(
        &self,
        _doc: CatalogueDocument,
    ) -> Result<ExtendedCrate, domain::tddd::NewTypeGraphCodecError> {
        use std::collections::BTreeMap;
        Ok(ExtendedCrate::new(empty_rustdoc_crate(), BTreeMap::new()))
    }
}

// -------------------------------------------------------------------------
// Happy-path and report-format tests
// -------------------------------------------------------------------------

#[test]
fn test_run_with_empty_evaluation_returns_all_items_maintained_report() {
    let binding = stub_binding("domain");
    let doc = minimal_catalogue_doc("domain");
    let interactor = build_interactor(
        Arc::new(StubLoader { doc }),
        Arc::new(EmptyExtendedCrateCodec),
        Arc::new(EmptyEvaluator),
        Arc::new(EmptyRustdocPort),
        Arc::new(StubLayerBindings { bindings: vec![binding] }),
    );
    // Use a real temp dir so the workspace_root symlink guard passes.
    let tmp = tempfile::tempdir().unwrap();
    let report = interactor.run("my-track".to_owned(), tmp.path().to_path_buf(), None).unwrap();
    assert!(
        report.text.contains("## Layer: `domain`"),
        "report must contain layer header: {}",
        report.text
    );
    assert!(
        report.text.contains("All items maintained"),
        "empty evaluation must produce 'All items maintained': {}",
        report.text
    );
    assert!(!report.any_red, "empty evaluation must have any_red = false");
}

#[test]
fn test_run_with_single_blue_signal_report_contains_signal_table() {
    let binding = stub_binding("domain");
    let doc = minimal_catalogue_doc("domain");
    let interactor = build_interactor(
        Arc::new(StubLoader { doc }),
        Arc::new(EmptyExtendedCrateCodec),
        Arc::new(SingleBlueEvaluator),
        Arc::new(EmptyRustdocPort),
        Arc::new(StubLayerBindings { bindings: vec![binding] }),
    );
    let tmp = tempfile::tempdir().unwrap();
    let report = interactor.run("my-track".to_owned(), tmp.path().to_path_buf(), None).unwrap();
    assert!(
        report.text.contains("| Item | Region | Signal |"),
        "report must contain table header: {}",
        report.text
    );
    assert!(report.text.contains("🔵 Blue"), "report must contain Blue signal: {}", report.text);
    assert!(report.text.contains("Summary:"), "report must contain Summary line: {}", report.text);
    assert!(!report.any_red, "single Blue signal must have any_red = false");
}

#[test]
fn test_run_with_single_red_signal_sets_any_red_true() {
    // Regression coverage for the `any_red` flag: a Red signal in the evaluation
    // must surface as `any_red = true`. This replaces the deleted markdown-scan
    // tests `test_report_has_red_signals_with_red_row_returns_true` /
    // `test_report_has_red_signals_summary_zero_red_is_not_false_positive` that
    // used to live on the CLI side before `report_has_red_signals` was retired.
    let binding = stub_binding("domain");
    let doc = minimal_catalogue_doc("domain");
    let interactor = build_interactor(
        Arc::new(StubLoader { doc }),
        Arc::new(EmptyExtendedCrateCodec),
        Arc::new(SingleRedEvaluator),
        Arc::new(EmptyRustdocPort),
        Arc::new(StubLayerBindings { bindings: vec![binding] }),
    );
    let tmp = tempfile::tempdir().unwrap();
    let report = interactor.run("my-track".to_owned(), tmp.path().to_path_buf(), None).unwrap();
    assert!(report.text.contains("🔴 Red"), "report must contain Red signal: {}", report.text);
    assert!(report.any_red, "single Red signal must have any_red = true");
}

#[test]
fn test_run_multi_target_binding_returns_schema_export_error() {
    // A binding with multiple targets must fail-closed: the signal evaluator
    // requires a single (A, B, C) tuple; multi-crate aggregation is not supported.
    let binding = TdddLayerBinding {
        layer_id: "domain".to_owned(),
        catalogue_file: "domain-types.json".to_owned(),
        baseline_file: "domain-types-baseline.json".to_owned(),
        targets: vec!["domain".to_owned(), "domain_extra".to_owned()],
    };
    let doc = minimal_catalogue_doc("domain");
    let interactor = build_interactor(
        Arc::new(StubLoader { doc }),
        Arc::new(EmptyExtendedCrateCodec),
        Arc::new(EmptyEvaluator),
        Arc::new(EmptyRustdocPort),
        Arc::new(StubLayerBindings { bindings: vec![binding] }),
    );
    let tmp = tempfile::tempdir().unwrap();
    let err = interactor.run("my-track".to_owned(), tmp.path().to_path_buf(), None).unwrap_err();
    assert!(
        matches!(err, CatalogueImplSignalsError::SchemaExport { .. }),
        "multi-target binding must return SchemaExport error, got: {err:?}"
    );
}

#[test]
fn test_run_evaluation_failure_returns_evaluation_error() {
    let binding = stub_binding("domain");
    let doc = minimal_catalogue_doc("domain");
    let interactor = build_interactor(
        Arc::new(StubLoader { doc }),
        Arc::new(EmptyExtendedCrateCodec),
        Arc::new(FailingEvaluator),
        Arc::new(EmptyRustdocPort),
        Arc::new(StubLayerBindings { bindings: vec![binding] }),
    );
    let tmp = tempfile::tempdir().unwrap();
    let err = interactor.run("my-track".to_owned(), tmp.path().to_path_buf(), None).unwrap_err();
    assert!(
        matches!(err, CatalogueImplSignalsError::Evaluation { .. }),
        "expected Evaluation error, got: {err:?}"
    );
}

#[test]
fn test_run_baseline_load_failure_returns_baseline_load_error() {
    let binding = stub_binding("domain");
    let doc = minimal_catalogue_doc("domain");
    let interactor = build_interactor(
        Arc::new(StubLoader { doc }),
        Arc::new(EmptyExtendedCrateCodec),
        Arc::new(EmptyEvaluator),
        Arc::new(FailingRustdocPort),
        Arc::new(StubLayerBindings { bindings: vec![binding] }),
    );
    let tmp = tempfile::tempdir().unwrap();
    let err = interactor.run("my-track".to_owned(), tmp.path().to_path_buf(), None).unwrap_err();
    assert!(
        matches!(err, CatalogueImplSignalsError::BaselineLoad { .. }),
        "expected BaselineLoad error, got: {err:?}"
    );
}
