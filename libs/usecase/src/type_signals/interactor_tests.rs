//! Tests for `TypeSignalsInteractor`.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::Path;
use std::sync::Arc;

use domain::TrackStatus;
use domain::tddd::catalogue_v2::{
    MissingCataloguePolicy, TdddLayerBinding, TdddLayerBindingsError, TdddLayerBindingsPort,
    TrackStatusReadError, TrackStatusReaderPort, TypeSignalsExecutionError,
    TypeSignalsExecutorPort,
};

use super::super::service::{TypeSignalsError, TypeSignalsRequest, TypeSignalsService};
use super::TypeSignalsInteractor;

// ---------------------------------------------------------------------------
// Test stubs
// ---------------------------------------------------------------------------

struct ActiveStatusReader;

impl TrackStatusReaderPort for ActiveStatusReader {
    fn read_status(
        &self,
        _items_dir: &Path,
        _track_id: &str,
    ) -> Result<TrackStatus, TrackStatusReadError> {
        Ok(TrackStatus::InProgress)
    }
}

struct FrozenStatusReader;

impl TrackStatusReaderPort for FrozenStatusReader {
    fn read_status(
        &self,
        _items_dir: &Path,
        _track_id: &str,
    ) -> Result<TrackStatus, TrackStatusReadError> {
        Ok(TrackStatus::Done)
    }
}

struct FailingStatusReader;

impl TrackStatusReaderPort for FailingStatusReader {
    fn read_status(
        &self,
        _items_dir: &Path,
        _track_id: &str,
    ) -> Result<TrackStatus, TrackStatusReadError> {
        Err(TrackStatusReadError("metadata read failed".to_owned()))
    }
}

struct StubLayerBindings {
    bindings: Vec<TdddLayerBinding>,
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

struct NoLayersBindings;

impl TdddLayerBindingsPort for NoLayersBindings {
    fn load(
        &self,
        _workspace_root: &Path,
        _layer_filter: Option<&str>,
    ) -> Result<Vec<TdddLayerBinding>, TdddLayerBindingsError> {
        Err(TdddLayerBindingsError::NoLayers)
    }
}

struct SuccessExecutor;

impl TypeSignalsExecutorPort for SuccessExecutor {
    fn evaluate_layer(
        &self,
        _items_dir: &Path,
        _track_id: &str,
        _workspace_root: &Path,
        _binding: &TdddLayerBinding,
        _policy: MissingCataloguePolicy,
    ) -> Result<(), TypeSignalsExecutionError> {
        Ok(())
    }
}

struct FailingExecutor;

impl TypeSignalsExecutorPort for FailingExecutor {
    fn evaluate_layer(
        &self,
        _items_dir: &Path,
        _track_id: &str,
        _workspace_root: &Path,
        _binding: &TdddLayerBinding,
        _policy: MissingCataloguePolicy,
    ) -> Result<(), TypeSignalsExecutionError> {
        Err(TypeSignalsExecutionError("evaluation failed: nightly not installed".to_owned()))
    }
}

fn stub_binding(layer_id: &str) -> TdddLayerBinding {
    TdddLayerBinding {
        layer_id: layer_id.to_owned(),
        catalogue_file: format!("{layer_id}-types.json"),
        baseline_file: format!("{layer_id}-types-baseline.json"),
        targets: vec![layer_id.to_owned()],
    }
}

fn build_interactor(
    status_reader: Arc<dyn TrackStatusReaderPort>,
    layer_bindings: Arc<dyn TdddLayerBindingsPort>,
    executor: Arc<dyn TypeSignalsExecutorPort>,
) -> TypeSignalsInteractor {
    TypeSignalsInteractor::new(status_reader, layer_bindings, executor)
}

fn valid_request(tmp: &std::path::Path) -> TypeSignalsRequest {
    TypeSignalsRequest {
        items_dir: tmp.join("track/items"),
        track_id: "test-track-2026-01-01".to_owned(),
        workspace_root: tmp.to_path_buf(),
        layer: None,
        lenient: false,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// The interactor derives `items_dir` from `workspace_root` internally, so
/// the `items_dir` field of the request is ignored.  This test exercises the
/// common CLI default where `workspace_root = "."` and `items_dir =
/// "track/items"` are passed as separate, non-equal raw `PathBuf`s — they
/// must be accepted without an `InconsistentRequest` error.
#[test]
fn test_run_with_dot_workspace_root_and_relative_items_dir_succeeds() {
    let interactor = build_interactor(
        Arc::new(ActiveStatusReader),
        Arc::new(StubLayerBindings { bindings: vec![stub_binding("domain")] }),
        Arc::new(SuccessExecutor),
    );
    // workspace_root = "."     → interactor derives items_dir = "./track/items"
    // request items_dir ignored; no InconsistentRequest can fire.
    let req = TypeSignalsRequest {
        items_dir: std::path::PathBuf::from("track/items"),
        track_id: "test-track-2026-01-01".to_owned(),
        workspace_root: std::path::PathBuf::from("."),
        layer: None,
        lenient: false,
    };

    let result = interactor.run(req);
    assert!(
        result.is_ok(),
        "items_dir is ignored; workspace_root = '.' must succeed, got: {result:?}"
    );
}

#[test]
fn test_run_with_invalid_track_id_returns_error() {
    let interactor = build_interactor(
        Arc::new(ActiveStatusReader),
        Arc::new(StubLayerBindings { bindings: vec![stub_binding("domain")] }),
        Arc::new(SuccessExecutor),
    );
    let tmp = tempfile::tempdir().unwrap();
    let mut req = valid_request(tmp.path());
    req.track_id = "bad track id!!".to_owned();

    let err = interactor.run(req).unwrap_err();
    assert!(
        matches!(err, TypeSignalsError::InvalidTrackId { .. }),
        "invalid track id must return InvalidTrackId error, got: {err:?}"
    );
}

#[test]
fn test_run_with_failing_status_reader_returns_error() {
    let interactor = build_interactor(
        Arc::new(FailingStatusReader),
        Arc::new(StubLayerBindings { bindings: vec![stub_binding("domain")] }),
        Arc::new(SuccessExecutor),
    );
    let tmp = tempfile::tempdir().unwrap();
    let req = valid_request(tmp.path());

    let err = interactor.run(req).unwrap_err();
    assert!(
        matches!(err, TypeSignalsError::StatusReadFailed { .. }),
        "status read failure must return StatusReadFailed, got: {err:?}"
    );
}

#[test]
fn test_run_with_frozen_track_returns_error() {
    let interactor = build_interactor(
        Arc::new(FrozenStatusReader),
        Arc::new(StubLayerBindings { bindings: vec![stub_binding("domain")] }),
        Arc::new(SuccessExecutor),
    );
    let tmp = tempfile::tempdir().unwrap();
    let req = valid_request(tmp.path());

    let err = interactor.run(req).unwrap_err();
    assert!(
        matches!(err, TypeSignalsError::TrackFrozen { .. }),
        "frozen track must return TrackFrozen, got: {err:?}"
    );
    let msg = err.to_string();
    assert!(msg.contains("status=done"), "message must mention the status, got: {msg}");
}

#[test]
fn test_run_with_no_layers_returns_error() {
    let interactor = build_interactor(
        Arc::new(ActiveStatusReader),
        Arc::new(NoLayersBindings),
        Arc::new(SuccessExecutor),
    );
    let tmp = tempfile::tempdir().unwrap();
    let req = valid_request(tmp.path());

    let err = interactor.run(req).unwrap_err();
    assert!(
        matches!(err, TypeSignalsError::NoLayers),
        "no layers must return NoLayers error, got: {err:?}"
    );
}

#[test]
fn test_run_with_failing_executor_returns_error() {
    let interactor = build_interactor(
        Arc::new(ActiveStatusReader),
        Arc::new(StubLayerBindings { bindings: vec![stub_binding("domain")] }),
        Arc::new(FailingExecutor),
    );
    let tmp = tempfile::tempdir().unwrap();
    let req = valid_request(tmp.path());

    let err = interactor.run(req).unwrap_err();
    assert!(
        matches!(err, TypeSignalsError::EvaluationFailed { .. }),
        "evaluation failure must return EvaluationFailed, got: {err:?}"
    );
}

#[test]
fn test_run_with_success_returns_ok() {
    let interactor = build_interactor(
        Arc::new(ActiveStatusReader),
        Arc::new(StubLayerBindings { bindings: vec![stub_binding("domain")] }),
        Arc::new(SuccessExecutor),
    );
    let tmp = tempfile::tempdir().unwrap();
    let req = valid_request(tmp.path());

    let result = interactor.run(req);
    assert!(result.is_ok(), "successful evaluation must return Ok, got: {result:?}");
}

#[test]
fn test_run_with_multiple_layers_processes_all() {
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct CountingExecutor(Arc<AtomicUsize>);

    impl TypeSignalsExecutorPort for CountingExecutor {
        fn evaluate_layer(
            &self,
            _items_dir: &Path,
            _track_id: &str,
            _workspace_root: &Path,
            _binding: &TdddLayerBinding,
            _policy: MissingCataloguePolicy,
        ) -> Result<(), TypeSignalsExecutionError> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    let count = Arc::new(AtomicUsize::new(0));
    let interactor = build_interactor(
        Arc::new(ActiveStatusReader),
        Arc::new(StubLayerBindings {
            bindings: vec![stub_binding("domain"), stub_binding("usecase")],
        }),
        Arc::new(CountingExecutor(Arc::clone(&count))),
    );
    let tmp = tempfile::tempdir().unwrap();
    let req = valid_request(tmp.path());

    interactor.run(req).unwrap();
    assert_eq!(count.load(Ordering::SeqCst), 2, "both layers must be processed");
}

#[test]
fn test_run_lenient_mode_passes_skip_silently_policy() {
    use std::sync::Mutex;

    struct PolicyCapture(Arc<Mutex<Vec<MissingCataloguePolicy>>>);

    impl TypeSignalsExecutorPort for PolicyCapture {
        fn evaluate_layer(
            &self,
            _items_dir: &Path,
            _track_id: &str,
            _workspace_root: &Path,
            _binding: &TdddLayerBinding,
            policy: MissingCataloguePolicy,
        ) -> Result<(), TypeSignalsExecutionError> {
            self.0.lock().unwrap().push(policy);
            Ok(())
        }
    }

    let policies: Arc<Mutex<Vec<MissingCataloguePolicy>>> = Arc::new(Mutex::new(Vec::new()));
    let interactor = build_interactor(
        Arc::new(ActiveStatusReader),
        Arc::new(StubLayerBindings { bindings: vec![stub_binding("domain")] }),
        Arc::new(PolicyCapture(Arc::clone(&policies))),
    );
    let tmp = tempfile::tempdir().unwrap();
    let mut req = valid_request(tmp.path());
    req.lenient = true;

    interactor.run(req).unwrap();

    let captured = policies.lock().unwrap();
    assert_eq!(captured.len(), 1);
    assert_eq!(
        captured.first().expect("one policy must be recorded"),
        &MissingCataloguePolicy::SkipSilently,
        "lenient mode must pass SkipSilently policy"
    );
}

#[test]
fn test_run_strict_mode_passes_fail_closed_policy() {
    use std::sync::Mutex;

    struct PolicyCapture(Arc<Mutex<Vec<MissingCataloguePolicy>>>);

    impl TypeSignalsExecutorPort for PolicyCapture {
        fn evaluate_layer(
            &self,
            _items_dir: &Path,
            _track_id: &str,
            _workspace_root: &Path,
            _binding: &TdddLayerBinding,
            policy: MissingCataloguePolicy,
        ) -> Result<(), TypeSignalsExecutionError> {
            self.0.lock().unwrap().push(policy);
            Ok(())
        }
    }

    let policies: Arc<Mutex<Vec<MissingCataloguePolicy>>> = Arc::new(Mutex::new(Vec::new()));
    let interactor = build_interactor(
        Arc::new(ActiveStatusReader),
        Arc::new(StubLayerBindings { bindings: vec![stub_binding("domain")] }),
        Arc::new(PolicyCapture(Arc::clone(&policies))),
    );
    let tmp = tempfile::tempdir().unwrap();
    let mut req = valid_request(tmp.path());
    req.lenient = false;

    interactor.run(req).unwrap();

    let captured = policies.lock().unwrap();
    assert_eq!(captured.len(), 1);
    assert_eq!(
        captured.first().expect("one policy must be recorded"),
        &MissingCataloguePolicy::FailClosed,
        "strict mode must pass FailClosed policy"
    );
}
