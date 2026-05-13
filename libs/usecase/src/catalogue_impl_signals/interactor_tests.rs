//! Error-path tests for `CatalogueImplSignalsInteractor`.
//!
//! Split from `interactor.rs` to keep the production-code file under 400 lines.
//! Loaded via `#[cfg(test)] #[path = "interactor_tests.rs"] mod tests;` in
//! `interactor.rs`.
//!
//! Happy-path / report-format tests live in `interactor_happy_tests.rs`, which
//! is included as a submodule below so it can share these helpers.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use domain::tddd::LayerId;
use domain::tddd::catalogue_v2::{
    CatalogueDocument, CatalogueDocumentLoaderError, CatalogueDocumentLoaderPort, RustdocCratePort,
    RustdocCratePortError, TdddLayerBinding, TdddLayerBindingsError, TdddLayerBindingsPort,
};
use domain::tddd::extended_crate::ExtendedCrate;
use domain::tddd::signal_evaluator::phase1_error::Phase1Error;
use domain::tddd::signal_evaluator::port::SignalEvaluatorPort;
// ThreeWaySignal is not pub-re-exported from the parent module, so it cannot be
// reached via `use super::*` and must be imported explicitly here.
use domain::tddd::signal_evaluator::region::{ThreeWayEvaluationReport, ThreeWaySignal};
use domain::{SymlinkGuardError, SymlinkGuardPort};
use rustdoc_types::{Crate, FORMAT_VERSION};

use super::super::service::{CatalogueImplSignalsError, CatalogueImplSignalsService};
use super::CatalogueImplSignalsInteractor;

// -------------------------------------------------------------------------
// Test helpers — also re-used by `happy_tests`
// -------------------------------------------------------------------------

/// Build a minimal `rustdoc_types::Crate` with no items.
pub(super) fn empty_rustdoc_crate() -> Crate {
    Crate {
        root: rustdoc_types::Id(0),
        crate_version: None,
        includes_private: false,
        index: HashMap::new(),
        paths: HashMap::new(),
        external_crates: HashMap::new(),
        format_version: FORMAT_VERSION,
        target: rustdoc_types::Target { triple: String::new(), target_features: vec![] },
    }
}

pub(super) fn minimal_catalogue_doc(crate_name: &str) -> CatalogueDocument {
    use domain::tddd::catalogue_v2::CrateName;
    let layer = LayerId::try_new(crate_name).unwrap();
    let name = CrateName::new(crate_name).unwrap();
    CatalogueDocument::new(3, name, layer)
}

pub(super) fn stub_binding(layer_id: &str) -> TdddLayerBinding {
    TdddLayerBinding {
        layer_id: layer_id.to_owned(),
        catalogue_file: format!("{layer_id}-types.json"),
        baseline_file: format!("{layer_id}-types-baseline.json"),
        targets: vec![layer_id.to_owned()],
    }
}

// -------------------------------------------------------------------------
// Mock ports — also re-used by `happy_tests`
// -------------------------------------------------------------------------

pub(super) struct StubLoader {
    pub(super) doc: CatalogueDocument,
}

impl CatalogueDocumentLoaderPort for StubLoader {
    fn load(&self, _path: &Path) -> Result<CatalogueDocument, CatalogueDocumentLoaderError> {
        Ok(self.doc.clone())
    }
}

pub(super) struct FailingLoader;

impl CatalogueDocumentLoaderPort for FailingLoader {
    fn load(&self, path: &Path) -> Result<CatalogueDocument, CatalogueDocumentLoaderError> {
        Err(CatalogueDocumentLoaderError::NotFound { path: path.to_path_buf() })
    }
}

/// `CatalogueToExtendedCratePort` that always fails.
pub(super) struct FailingCodec;

impl domain::tddd::CatalogueToExtendedCratePort for FailingCodec {
    fn encode(
        &self,
        _doc: CatalogueDocument,
    ) -> Result<ExtendedCrate, domain::tddd::NewTypeGraphCodecError> {
        Err(domain::tddd::NewTypeGraphCodecError::InvalidTypeRef("stub".to_owned()))
    }
}

/// `SignalEvaluatorPort` that always returns an empty report.
pub(super) struct EmptyEvaluator;

impl SignalEvaluatorPort for EmptyEvaluator {
    fn evaluate(
        &self,
        _a: ExtendedCrate,
        _b: Crate,
        _c: Crate,
    ) -> Result<ThreeWayEvaluationReport, Phase1Error> {
        Ok(ThreeWayEvaluationReport::new(vec![]))
    }
}

/// `SignalEvaluatorPort` that always returns a single Blue signal.
pub(super) struct SingleBlueEvaluator;

impl SignalEvaluatorPort for SingleBlueEvaluator {
    fn evaluate(
        &self,
        _a: ExtendedCrate,
        _b: Crate,
        _c: Crate,
    ) -> Result<ThreeWayEvaluationReport, Phase1Error> {
        use domain::tddd::signal_evaluator::region::SignalRegion;
        let signal = ThreeWaySignal::new("MyType".to_owned(), SignalRegion::SIntersectC_Match_Add);
        Ok(ThreeWayEvaluationReport::new(vec![signal]))
    }
}

/// `SignalEvaluatorPort` that always returns an Evaluation failure.
pub(super) struct FailingEvaluator;

impl SignalEvaluatorPort for FailingEvaluator {
    fn evaluate(
        &self,
        _a: ExtendedCrate,
        _b: Crate,
        _c: Crate,
    ) -> Result<ThreeWayEvaluationReport, Phase1Error> {
        Err(Phase1Error::ActionContradiction("stub contradiction".to_owned()))
    }
}

/// `RustdocCratePort` that always panics — used in tests that stop before
/// the rustdoc ports are reached.
pub(super) struct NeverCalledRustdocPort;

impl RustdocCratePort for NeverCalledRustdocPort {
    fn load_from_path(&self, _path: &Path) -> Result<Crate, RustdocCratePortError> {
        panic!("NeverCalledRustdocPort::load_from_path must not be called in these tests")
    }

    fn capture_current(&self, _crate_name: &str) -> Result<Crate, RustdocCratePortError> {
        panic!("NeverCalledRustdocPort::capture_current must not be called in these tests")
    }
}

/// `RustdocCratePort` that returns empty rustdoc crates for load and capture.
pub(super) struct EmptyRustdocPort;

impl RustdocCratePort for EmptyRustdocPort {
    fn load_from_path(&self, _path: &Path) -> Result<Crate, RustdocCratePortError> {
        Ok(empty_rustdoc_crate())
    }

    fn capture_current(&self, _crate_name: &str) -> Result<Crate, RustdocCratePortError> {
        Ok(empty_rustdoc_crate())
    }
}

/// `RustdocCratePort` that always returns `NotFound` for load and `CaptureFailed` for capture.
pub(super) struct FailingRustdocPort;

impl RustdocCratePort for FailingRustdocPort {
    fn load_from_path(&self, path: &Path) -> Result<Crate, RustdocCratePortError> {
        Err(RustdocCratePortError::NotFound { path: path.to_path_buf() })
    }

    fn capture_current(&self, crate_name: &str) -> Result<Crate, RustdocCratePortError> {
        Err(RustdocCratePortError::CaptureFailed {
            crate_name: crate_name.to_owned(),
            reason: "stub capture failure".to_owned(),
        })
    }
}

pub(super) struct StubLayerBindings {
    pub(super) bindings: Vec<TdddLayerBinding>,
}

impl TdddLayerBindingsPort for StubLayerBindings {
    fn load(
        &self,
        _workspace_root: &Path,
        _layer_filter: Option<&str>,
    ) -> Result<Vec<TdddLayerBinding>, TdddLayerBindingsError> {
        Ok(self.bindings.clone())
    }
}

pub(super) struct EmptyLayerBindings;

impl TdddLayerBindingsPort for EmptyLayerBindings {
    fn load(
        &self,
        _workspace_root: &Path,
        _layer_filter: Option<&str>,
    ) -> Result<Vec<TdddLayerBinding>, TdddLayerBindingsError> {
        Ok(vec![])
    }
}

pub(super) struct FailingLayerBindings;

impl TdddLayerBindingsPort for FailingLayerBindings {
    fn load(
        &self,
        _workspace_root: &Path,
        _layer_filter: Option<&str>,
    ) -> Result<Vec<TdddLayerBinding>, TdddLayerBindingsError> {
        Err(TdddLayerBindingsError::LoadFailed {
            reason: "architecture-rules.json not found".to_owned(),
        })
    }
}

pub(super) struct LayerNotFoundLayerBindings {
    pub(super) missing_layer_id: String,
}

impl TdddLayerBindingsPort for LayerNotFoundLayerBindings {
    fn load(
        &self,
        _workspace_root: &Path,
        _layer_filter: Option<&str>,
    ) -> Result<Vec<TdddLayerBinding>, TdddLayerBindingsError> {
        Err(TdddLayerBindingsError::LayerNotFound { layer_id: self.missing_layer_id.clone() })
    }
}

/// No-op `SymlinkGuardPort` that always reports "no symlink found".
///
/// Used as the default in tests that don't exercise the symlink guard path.
pub(super) struct NoopSymlinkGuard;

impl SymlinkGuardPort for NoopSymlinkGuard {
    fn reject_symlinks_from_root(&self, _path: &Path) -> Result<(), SymlinkGuardError> {
        Ok(())
    }

    fn reject_symlinks_below(
        &self,
        _path: &Path,
        _trusted_root: &Path,
    ) -> Result<(), SymlinkGuardError> {
        Ok(())
    }
}

/// `SymlinkGuardPort` that always rejects with `SymlinkFound`.
///
/// Used to test that the interactor correctly propagates symlink rejection.
pub(super) struct AlwaysRejectSymlinkGuard;

impl SymlinkGuardPort for AlwaysRejectSymlinkGuard {
    fn reject_symlinks_from_root(&self, path: &Path) -> Result<(), SymlinkGuardError> {
        Err(SymlinkGuardError::SymlinkFound { path: path.display().to_string() })
    }

    fn reject_symlinks_below(
        &self,
        path: &Path,
        _trusted_root: &Path,
    ) -> Result<(), SymlinkGuardError> {
        Err(SymlinkGuardError::SymlinkFound { path: path.display().to_string() })
    }
}

// -------------------------------------------------------------------------
// Interactor builder helper — also re-used by `happy_tests`
// -------------------------------------------------------------------------

pub(super) fn build_interactor(
    loader: Arc<dyn CatalogueDocumentLoaderPort>,
    codec: Arc<dyn domain::tddd::CatalogueToExtendedCratePort>,
    evaluator: Arc<dyn SignalEvaluatorPort>,
    rustdoc: Arc<dyn RustdocCratePort>,
    bindings: Arc<dyn TdddLayerBindingsPort>,
) -> CatalogueImplSignalsInteractor {
    build_interactor_with_guard(
        loader,
        codec,
        evaluator,
        rustdoc,
        bindings,
        Arc::new(NoopSymlinkGuard),
    )
}

pub(super) fn build_interactor_with_guard(
    loader: Arc<dyn CatalogueDocumentLoaderPort>,
    codec: Arc<dyn domain::tddd::CatalogueToExtendedCratePort>,
    evaluator: Arc<dyn SignalEvaluatorPort>,
    rustdoc: Arc<dyn RustdocCratePort>,
    bindings: Arc<dyn TdddLayerBindingsPort>,
    symlink_guard: Arc<dyn SymlinkGuardPort>,
) -> CatalogueImplSignalsInteractor {
    CatalogueImplSignalsInteractor::new(loader, codec, evaluator, rustdoc, bindings, symlink_guard)
}

// -------------------------------------------------------------------------
// Happy-path / report-format tests (in sibling file to keep this file short)
// -------------------------------------------------------------------------

#[cfg(test)]
#[path = "interactor_happy_tests.rs"]
mod happy_tests;

// -------------------------------------------------------------------------
// CatalogueImplSignalsInteractor::run — error-path tests
// -------------------------------------------------------------------------

#[test]
fn test_run_workspace_root_with_dotdot_returns_symlink_rejected_error() {
    // A workspace_root containing `..` must be rejected before any I/O.
    let interactor = build_interactor(
        Arc::new(FailingLoader),
        Arc::new(FailingCodec),
        Arc::new(EmptyEvaluator),
        Arc::new(NeverCalledRustdocPort),
        Arc::new(StubLayerBindings { bindings: vec![] }),
    );
    let bad_root = std::path::PathBuf::from("/tmp/../etc");
    let err = interactor.run("my-track".to_owned(), bad_root, None).unwrap_err();
    assert!(
        matches!(err, CatalogueImplSignalsError::SymlinkRejected { .. }),
        "expected SymlinkRejected for dot-dot workspace_root, got: {err:?}"
    );
}

#[test]
fn test_run_invalid_track_id_returns_invalid_track_id_error() {
    let interactor = build_interactor(
        Arc::new(FailingLoader),
        Arc::new(FailingCodec),
        Arc::new(EmptyEvaluator),
        Arc::new(NeverCalledRustdocPort),
        Arc::new(StubLayerBindings { bindings: vec![] }),
    );
    let err = interactor
        .run("BAD TRACK ID!!".to_owned(), std::path::PathBuf::from("/tmp"), None)
        .unwrap_err();
    assert!(
        matches!(err, CatalogueImplSignalsError::InvalidTrackId { .. }),
        "expected InvalidTrackId, got: {err:?}"
    );
}

#[test]
fn test_run_no_layers_returns_no_layers_error() {
    let interactor = build_interactor(
        Arc::new(FailingLoader),
        Arc::new(FailingCodec),
        Arc::new(EmptyEvaluator),
        Arc::new(NeverCalledRustdocPort),
        Arc::new(EmptyLayerBindings),
    );
    let err =
        interactor.run("my-track".to_owned(), std::path::PathBuf::from("/tmp"), None).unwrap_err();
    assert!(matches!(err, CatalogueImplSignalsError::NoLayers), "expected NoLayers, got: {err:?}");
}

#[test]
fn test_run_layer_bindings_load_failure_returns_layer_bindings_load_error() {
    let interactor = build_interactor(
        Arc::new(FailingLoader),
        Arc::new(FailingCodec),
        Arc::new(EmptyEvaluator),
        Arc::new(NeverCalledRustdocPort),
        Arc::new(FailingLayerBindings),
    );
    let err =
        interactor.run("my-track".to_owned(), std::path::PathBuf::from("/tmp"), None).unwrap_err();
    assert!(
        matches!(err, CatalogueImplSignalsError::LayerBindingsLoad { .. }),
        "expected LayerBindingsLoad, got: {err:?}"
    );
}

#[test]
fn test_run_catalogue_load_failure_returns_catalogue_load_error() {
    let binding = stub_binding("domain");
    let interactor = build_interactor(
        Arc::new(FailingLoader),
        Arc::new(FailingCodec),
        Arc::new(EmptyEvaluator),
        Arc::new(NeverCalledRustdocPort),
        Arc::new(StubLayerBindings { bindings: vec![binding] }),
    );
    let err =
        interactor.run("my-track".to_owned(), std::path::PathBuf::from("/tmp"), None).unwrap_err();
    assert!(
        matches!(err, CatalogueImplSignalsError::CatalogueLoad { .. }),
        "expected CatalogueLoad, got: {err:?}"
    );
}

#[test]
fn test_run_ext_crate_conversion_failure_returns_ext_crate_conversion_error() {
    let binding = stub_binding("domain");
    let doc = minimal_catalogue_doc("domain");
    let interactor = build_interactor(
        Arc::new(StubLoader { doc }),
        Arc::new(FailingCodec),
        Arc::new(EmptyEvaluator),
        Arc::new(NeverCalledRustdocPort),
        Arc::new(StubLayerBindings { bindings: vec![binding] }),
    );
    let err =
        interactor.run("my-track".to_owned(), std::path::PathBuf::from("/tmp"), None).unwrap_err();
    assert!(
        matches!(err, CatalogueImplSignalsError::ExtendedCrateConversion { .. }),
        "expected ExtendedCrateConversion, got: {err:?}"
    );
}

#[test]
fn test_run_layer_not_found_with_layer_filter_returns_layer_bindings_load_error() {
    // When a layer filter is supplied and the port returns `LayerNotFound`,
    // `run` must map this to `CatalogueImplSignalsError::LayerBindingsLoad`.
    // This covers the `TdddLayerBindingsError::LayerNotFound` branch in the
    // error mapping (the `LoadFailed` branch is covered by the test above).
    let interactor = build_interactor(
        Arc::new(FailingLoader),
        Arc::new(FailingCodec),
        Arc::new(EmptyEvaluator),
        Arc::new(NeverCalledRustdocPort),
        Arc::new(LayerNotFoundLayerBindings { missing_layer_id: "nonexistent".to_owned() }),
    );
    let err = interactor
        .run(
            "my-track".to_owned(),
            std::path::PathBuf::from("/tmp"),
            Some("nonexistent".to_owned()),
        )
        .unwrap_err();
    assert!(
        matches!(err, CatalogueImplSignalsError::LayerBindingsLoad { .. }),
        "LayerNotFound must map to LayerBindingsLoad, got: {err:?}"
    );
}

#[test]
fn test_run_symlink_guard_rejection_returns_symlink_rejected_error() {
    // When the injected SymlinkGuardPort always rejects, run() must return SymlinkRejected.
    let interactor = build_interactor_with_guard(
        Arc::new(FailingLoader),
        Arc::new(FailingCodec),
        Arc::new(EmptyEvaluator),
        Arc::new(NeverCalledRustdocPort),
        Arc::new(StubLayerBindings { bindings: vec![] }),
        Arc::new(AlwaysRejectSymlinkGuard),
    );
    let err =
        interactor.run("my-track".to_owned(), std::path::PathBuf::from("/tmp"), None).unwrap_err();
    assert!(
        matches!(err, CatalogueImplSignalsError::SymlinkRejected { .. }),
        "expected SymlinkRejected from guard port, got: {err:?}"
    );
}

#[test]
fn test_run_path_traversal_in_catalogue_file_rejected() {
    let mut binding = stub_binding("domain");
    binding.catalogue_file = "../../../etc/passwd".to_owned();
    let interactor = build_interactor(
        Arc::new(FailingLoader),
        Arc::new(FailingCodec),
        Arc::new(EmptyEvaluator),
        Arc::new(NeverCalledRustdocPort),
        Arc::new(StubLayerBindings { bindings: vec![binding] }),
    );
    let err =
        interactor.run("my-track".to_owned(), std::path::PathBuf::from("/tmp"), None).unwrap_err();
    assert!(
        matches!(err, CatalogueImplSignalsError::SymlinkRejected { .. }),
        "expected SymlinkRejected for path traversal in catalogue_file, got: {err:?}"
    );
}
