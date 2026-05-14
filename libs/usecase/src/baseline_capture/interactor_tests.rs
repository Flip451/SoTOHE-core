//! Tests for `BaselineCaptureInteractor`.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::Path;
use std::sync::Arc;

use domain::SymlinkGuardError;
use domain::SymlinkGuardPort;
use domain::tddd::catalogue_v2::{
    BaselineCaptureIoError, RustdocBaselineCapturePort, TdddLayerBinding, TdddLayerBindingsError,
    TdddLayerBindingsPort,
};

use super::super::service::{BaselineCaptureError, BaselineCaptureRequest, BaselineCaptureService};
use super::BaselineCaptureInteractor;

// ---------------------------------------------------------------------------
// Test stubs
// ---------------------------------------------------------------------------

/// Symlink guard that accepts all paths (no symlinks found).
struct PermissiveSymlinkGuard;

impl SymlinkGuardPort for PermissiveSymlinkGuard {
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

/// Symlink guard that always rejects (simulates symlink found at root).
struct RejectingSymlinkGuard;

impl SymlinkGuardPort for RejectingSymlinkGuard {
    fn reject_symlinks_from_root(&self, path: &Path) -> Result<(), SymlinkGuardError> {
        Err(SymlinkGuardError::SymlinkFound { path: path.to_string_lossy().to_string() })
    }

    fn reject_symlinks_below(
        &self,
        path: &Path,
        _trusted_root: &Path,
    ) -> Result<(), SymlinkGuardError> {
        Err(SymlinkGuardError::SymlinkFound { path: path.to_string_lossy().to_string() })
    }
}

/// Layer bindings stub that returns a fixed set of bindings.
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

/// Layer bindings stub that always returns NoLayers.
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

/// Baseline capture stub that always succeeds.
struct SuccessCapture;

impl RustdocBaselineCapturePort for SuccessCapture {
    fn capture(
        &self,
        _items_dir: &Path,
        _track_id: &str,
        _rustdoc_workspace: &Path,
        _binding: &TdddLayerBinding,
        _force: bool,
    ) -> Result<(), BaselineCaptureIoError> {
        Ok(())
    }
}

/// Baseline capture stub that always fails.
struct FailingCapture;

impl RustdocBaselineCapturePort for FailingCapture {
    fn capture(
        &self,
        _items_dir: &Path,
        _track_id: &str,
        _rustdoc_workspace: &Path,
        _binding: &TdddLayerBinding,
        _force: bool,
    ) -> Result<(), BaselineCaptureIoError> {
        Err(BaselineCaptureIoError("capture failed: nightly not installed".to_owned()))
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
    layer_bindings: Arc<dyn TdddLayerBindingsPort>,
    capture: Arc<dyn RustdocBaselineCapturePort>,
) -> BaselineCaptureInteractor {
    BaselineCaptureInteractor::new(Arc::new(PermissiveSymlinkGuard), layer_bindings, capture)
}

fn valid_request(tmp: &std::path::Path) -> BaselineCaptureRequest {
    BaselineCaptureRequest {
        track_id: "test-track-2026-01-01".to_owned(),
        workspace_root: tmp.to_path_buf(),
        source_workspace: None,
        layer: None,
        force: false,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn test_run_with_invalid_track_id_returns_error() {
    let interactor = build_interactor(
        Arc::new(StubLayerBindings { bindings: vec![stub_binding("domain")] }),
        Arc::new(SuccessCapture),
    );
    let tmp = tempfile::tempdir().unwrap();

    let mut req = valid_request(tmp.path());
    req.track_id = "bad track id!!".to_owned();

    let err = interactor.run(req).unwrap_err();
    assert!(
        matches!(err, BaselineCaptureError::InvalidTrackId { .. }),
        "invalid track id must return InvalidTrackId error, got: {err:?}"
    );
}

#[test]
fn test_run_with_dotdot_workspace_root_returns_symlink_rejected() {
    let interactor = build_interactor(
        Arc::new(StubLayerBindings { bindings: vec![stub_binding("domain")] }),
        Arc::new(SuccessCapture),
    );

    let req = BaselineCaptureRequest {
        track_id: "test-track-2026-01-01".to_owned(),
        workspace_root: std::path::PathBuf::from("../outside"),
        source_workspace: None,
        layer: None,
        force: false,
    };

    let err = interactor.run(req).unwrap_err();
    assert!(
        matches!(err, BaselineCaptureError::SymlinkRejected { .. }),
        "dotdot workspace_root must return SymlinkRejected, got: {err:?}"
    );
}

#[test]
fn test_run_with_symlinked_workspace_root_returns_symlink_rejected() {
    let interactor = BaselineCaptureInteractor::new(
        Arc::new(RejectingSymlinkGuard),
        Arc::new(StubLayerBindings { bindings: vec![stub_binding("domain")] }),
        Arc::new(SuccessCapture),
    );
    let tmp = tempfile::tempdir().unwrap();
    let req = valid_request(tmp.path());

    let err = interactor.run(req).unwrap_err();
    assert!(
        matches!(err, BaselineCaptureError::SymlinkRejected { .. }),
        "rejecting symlink guard must return SymlinkRejected, got: {err:?}"
    );
}

#[test]
fn test_run_with_no_layers_returns_no_layers_error() {
    let interactor = build_interactor(Arc::new(NoLayersBindings), Arc::new(SuccessCapture));
    let tmp = tempfile::tempdir().unwrap();
    let req = valid_request(tmp.path());

    let err = interactor.run(req).unwrap_err();
    assert!(
        matches!(err, BaselineCaptureError::NoLayers),
        "no layers must return NoLayers error, got: {err:?}"
    );
}

#[test]
fn test_run_with_failing_capture_returns_capture_failed_error() {
    let interactor = build_interactor(
        Arc::new(StubLayerBindings { bindings: vec![stub_binding("domain")] }),
        Arc::new(FailingCapture),
    );
    let tmp = tempfile::tempdir().unwrap();
    let req = valid_request(tmp.path());

    let err = interactor.run(req).unwrap_err();
    assert!(
        matches!(err, BaselineCaptureError::CaptureFailed { .. }),
        "capture failure must return CaptureFailed error, got: {err:?}"
    );
}

#[test]
fn test_run_with_success_capture_returns_ok() {
    let interactor = build_interactor(
        Arc::new(StubLayerBindings { bindings: vec![stub_binding("domain")] }),
        Arc::new(SuccessCapture),
    );
    let tmp = tempfile::tempdir().unwrap();
    let req = valid_request(tmp.path());

    let result = interactor.run(req);
    assert!(result.is_ok(), "successful capture must return Ok(()), got: {result:?}");
}

#[test]
fn test_run_with_multiple_layers_processes_all() {
    // Uses a capture counter to verify all layers are processed.
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct CountingCapture(Arc<AtomicUsize>);

    impl RustdocBaselineCapturePort for CountingCapture {
        fn capture(
            &self,
            _items_dir: &Path,
            _track_id: &str,
            _rustdoc_workspace: &Path,
            _binding: &TdddLayerBinding,
            _force: bool,
        ) -> Result<(), BaselineCaptureIoError> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    let count = Arc::new(AtomicUsize::new(0));
    let interactor = build_interactor(
        Arc::new(StubLayerBindings {
            bindings: vec![stub_binding("domain"), stub_binding("usecase")],
        }),
        Arc::new(CountingCapture(Arc::clone(&count))),
    );
    let tmp = tempfile::tempdir().unwrap();
    let req = valid_request(tmp.path());

    interactor.run(req).unwrap();
    assert_eq!(count.load(Ordering::SeqCst), 2, "both layers must be processed");
}

#[test]
fn test_run_with_dotdot_source_workspace_returns_symlink_rejected() {
    let interactor = build_interactor(
        Arc::new(StubLayerBindings { bindings: vec![stub_binding("domain")] }),
        Arc::new(SuccessCapture),
    );
    let tmp = tempfile::tempdir().unwrap();

    let req = BaselineCaptureRequest {
        track_id: "test-track-2026-01-01".to_owned(),
        workspace_root: tmp.path().to_path_buf(),
        source_workspace: Some(std::path::PathBuf::from("../outside")),
        layer: None,
        force: false,
    };

    let err = interactor.run(req).unwrap_err();
    assert!(
        matches!(err, BaselineCaptureError::SymlinkRejected { .. }),
        "dotdot source_workspace must return SymlinkRejected, got: {err:?}"
    );
}

#[test]
fn test_run_with_symlinked_source_workspace_returns_symlink_rejected() {
    // Use the rejecting guard so that source_workspace symlink check fires.
    let interactor = BaselineCaptureInteractor::new(
        Arc::new(RejectingSymlinkGuard),
        Arc::new(StubLayerBindings { bindings: vec![stub_binding("domain")] }),
        Arc::new(SuccessCapture),
    );
    let tmp = tempfile::tempdir().unwrap();
    let source = tempfile::tempdir().unwrap();

    let req = BaselineCaptureRequest {
        track_id: "test-track-2026-01-01".to_owned(),
        workspace_root: tmp.path().to_path_buf(),
        source_workspace: Some(source.path().to_path_buf()),
        layer: None,
        force: false,
    };

    let err = interactor.run(req).unwrap_err();
    assert!(
        matches!(err, BaselineCaptureError::SymlinkRejected { .. }),
        "rejecting symlink guard on source_workspace must return SymlinkRejected, got: {err:?}"
    );
}

#[test]
fn test_run_source_workspace_is_passed_to_capture() {
    use std::sync::Mutex;

    struct WorkspaceCapture(Arc<Mutex<Vec<std::path::PathBuf>>>);

    impl RustdocBaselineCapturePort for WorkspaceCapture {
        fn capture(
            &self,
            _items_dir: &Path,
            _track_id: &str,
            rustdoc_workspace: &Path,
            _binding: &TdddLayerBinding,
            _force: bool,
        ) -> Result<(), BaselineCaptureIoError> {
            self.0.lock().unwrap().push(rustdoc_workspace.to_path_buf());
            Ok(())
        }
    }

    let captured_workspaces: Arc<Mutex<Vec<std::path::PathBuf>>> = Arc::new(Mutex::new(Vec::new()));
    let interactor = build_interactor(
        Arc::new(StubLayerBindings { bindings: vec![stub_binding("domain")] }),
        Arc::new(WorkspaceCapture(Arc::clone(&captured_workspaces))),
    );
    let tmp = tempfile::tempdir().unwrap();
    let source = tempfile::tempdir().unwrap();

    let req = BaselineCaptureRequest {
        track_id: "test-track-2026-01-01".to_owned(),
        workspace_root: tmp.path().to_path_buf(),
        source_workspace: Some(source.path().to_path_buf()),
        layer: None,
        force: false,
    };

    interactor.run(req).unwrap();

    let workspaces = captured_workspaces.lock().unwrap();
    assert_eq!(workspaces.len(), 1);
    assert_eq!(
        workspaces.first().expect("at least one workspace must be recorded"),
        source.path(),
        "source_workspace must be passed to capture"
    );
}
