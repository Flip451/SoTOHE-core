//! Tests for `BaselineCaptureInteractor`.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::Path;
use std::sync::Arc;

use crate::tddd_feature_declaration::{
    TdddBaselineFeatureDeclarationPort, TdddBaselineFeatureDeclarationPortError,
};
use domain::tddd::catalogue_v2::{
    BaselineCaptureIoError, RustdocBaselineCapturePort, TdddLayerBinding, TdddLayerBindingsError,
    TdddLayerBindingsPort,
};
use domain::tddd::{CargoFeatureName, LayerId, TdddFeatureDeclaration};
use domain::{SymlinkGuardError, SymlinkGuardPort, TrackId};

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

/// Symlink guard that reports an I/O failure for every inspected path.
struct IoFailingSymlinkGuard;

impl SymlinkGuardPort for IoFailingSymlinkGuard {
    fn reject_symlinks_from_root(&self, path: &Path) -> Result<(), SymlinkGuardError> {
        Err(SymlinkGuardError::Io {
            path: path.to_string_lossy().to_string(),
            reason: "permission denied".to_owned(),
        })
    }

    fn reject_symlinks_below(
        &self,
        path: &Path,
        _trusted_root: &Path,
    ) -> Result<(), SymlinkGuardError> {
        Err(SymlinkGuardError::Io {
            path: path.to_string_lossy().to_string(),
            reason: "permission denied".to_owned(),
        })
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
        _track_id: &TrackId,
        _rustdoc_workspace: &Path,
        _binding: &TdddLayerBinding,
        _features: &[CargoFeatureName],
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
        _track_id: &TrackId,
        _rustdoc_workspace: &Path,
        _binding: &TdddLayerBinding,
        _features: &[CargoFeatureName],
    ) -> Result<(), BaselineCaptureIoError> {
        Err(BaselineCaptureIoError("capture failed: nightly not installed".to_owned()))
    }
}

/// Feature declaration stub that returns an explicit empty feature list for every binding.
struct EmptyFeatureDeclaration;

impl TdddBaselineFeatureDeclarationPort for EmptyFeatureDeclaration {
    fn load_for_baseline(
        &self,
        _track_dir: &Path,
        _workspace_root: &Path,
        layers: &[TdddLayerBinding],
    ) -> Result<TdddFeatureDeclaration, TdddBaselineFeatureDeclarationPortError> {
        let required_layers = layers
            .iter()
            .map(|binding| LayerId::try_new(binding.layer_id.clone()).unwrap())
            .collect::<Vec<_>>();
        let layer_features =
            required_layers.iter().cloned().map(|layer| (layer, Vec::new())).collect();
        Ok(TdddFeatureDeclaration::try_new(layer_features, &required_layers).unwrap())
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
    BaselineCaptureInteractor::new(
        Arc::new(PermissiveSymlinkGuard),
        layer_bindings,
        capture,
        Arc::new(EmptyFeatureDeclaration),
    )
}

fn valid_request(tmp: &std::path::Path) -> BaselineCaptureRequest {
    BaselineCaptureRequest {
        track_id: "test-track-2026-01-01".to_owned(),
        workspace_root: tmp.to_path_buf(),
        source_workspace: None,
        layer: None,
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
        matches!(err, BaselineCaptureError::InvalidTrackId(_)),
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
    };

    let err = interactor.run(req).unwrap_err();
    assert!(
        matches!(err, BaselineCaptureError::SymlinkRejected(_)),
        "dotdot workspace_root must return SymlinkRejected, got: {err:?}"
    );
}

#[test]
fn test_run_with_symlinked_workspace_root_returns_symlink_rejected() {
    let interactor = BaselineCaptureInteractor::new(
        Arc::new(RejectingSymlinkGuard),
        Arc::new(StubLayerBindings { bindings: vec![stub_binding("domain")] }),
        Arc::new(SuccessCapture),
        Arc::new(EmptyFeatureDeclaration),
    );
    let tmp = tempfile::tempdir().unwrap();
    let req = valid_request(tmp.path());

    let err = interactor.run(req).unwrap_err();
    assert!(
        matches!(err, BaselineCaptureError::SymlinkRejected(_)),
        "rejecting symlink guard must return SymlinkRejected, got: {err:?}"
    );
}

#[test]
fn test_run_with_symlink_guard_io_preserves_path_and_reason() {
    let interactor = BaselineCaptureInteractor::new(
        Arc::new(IoFailingSymlinkGuard),
        Arc::new(StubLayerBindings { bindings: vec![stub_binding("domain")] }),
        Arc::new(SuccessCapture),
        Arc::new(EmptyFeatureDeclaration),
    );
    let tmp = tempfile::tempdir().unwrap();

    let err = interactor.run(valid_request(tmp.path())).unwrap_err();

    assert!(matches!(
        err,
        BaselineCaptureError::SymlinkGuardIo(path, reason)
            if path.as_path() == tmp.path() && reason.as_str() == "permission denied"
    ));
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
        matches!(err, BaselineCaptureError::CaptureFailed(_, _)),
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
            _track_id: &TrackId,
            _rustdoc_workspace: &Path,
            _binding: &TdddLayerBinding,
            _features: &[CargoFeatureName],
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
fn test_run_with_feature_declaration_forwards_layer_features_to_capture() {
    use std::collections::BTreeMap;
    use std::sync::Mutex;

    type ObservedFeatures = Vec<(String, Vec<String>)>;

    struct DeclaredFeatures;

    impl TdddBaselineFeatureDeclarationPort for DeclaredFeatures {
        fn load_for_baseline(
            &self,
            _track_dir: &Path,
            _workspace_root: &Path,
            _layers: &[TdddLayerBinding],
        ) -> Result<TdddFeatureDeclaration, TdddBaselineFeatureDeclarationPortError> {
            let domain = LayerId::try_new("domain".to_owned()).unwrap();
            let infrastructure = LayerId::try_new("infrastructure".to_owned()).unwrap();
            let feature = CargoFeatureName::try_new("semantic-dup".to_owned()).unwrap();
            TdddFeatureDeclaration::try_new(
                BTreeMap::from([(domain.clone(), vec![]), (infrastructure.clone(), vec![feature])]),
                &[domain, infrastructure],
            )
            .map_err(|_| TdddBaselineFeatureDeclarationPortError::BaselineSnapshotMismatch)
        }
    }

    struct RecordingCapture(Arc<Mutex<ObservedFeatures>>);

    impl RustdocBaselineCapturePort for RecordingCapture {
        fn capture(
            &self,
            _items_dir: &Path,
            _track_id: &TrackId,
            _rustdoc_workspace: &Path,
            binding: &TdddLayerBinding,
            features: &[CargoFeatureName],
        ) -> Result<(), BaselineCaptureIoError> {
            self.0.lock().unwrap().push((
                binding.layer_id.clone(),
                features.iter().map(|feature| feature.as_str().to_owned()).collect(),
            ));
            Ok(())
        }
    }

    let observed = Arc::new(Mutex::new(Vec::new()));
    let interactor = BaselineCaptureInteractor::new(
        Arc::new(PermissiveSymlinkGuard),
        Arc::new(StubLayerBindings {
            bindings: vec![stub_binding("domain"), stub_binding("infrastructure")],
        }),
        Arc::new(RecordingCapture(Arc::clone(&observed))),
        Arc::new(DeclaredFeatures),
    );
    let workspace = tempfile::tempdir().unwrap();

    interactor.run(valid_request(workspace.path())).unwrap();

    assert_eq!(
        observed.lock().unwrap().as_slice(),
        [
            ("domain".to_owned(), vec![]),
            ("infrastructure".to_owned(), vec!["semantic-dup".to_owned()]),
        ]
    );
}

#[test]
fn test_run_with_missing_feature_declaration_returns_error_before_capture() {
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct MissingFeatureDeclaration;

    impl TdddBaselineFeatureDeclarationPort for MissingFeatureDeclaration {
        fn load_for_baseline(
            &self,
            track_dir: &Path,
            _workspace_root: &Path,
            _layers: &[TdddLayerBinding],
        ) -> Result<TdddFeatureDeclaration, TdddBaselineFeatureDeclarationPortError> {
            Err(TdddBaselineFeatureDeclarationPortError::Read(
                crate::tddd_feature_declaration::TdddFeatureDeclarationReadError::MissingDeclaration {
                    path: track_dir.join("tddd-features.json"),
                },
            ))
        }
    }

    struct CountingCapture(Arc<AtomicUsize>);

    impl RustdocBaselineCapturePort for CountingCapture {
        fn capture(
            &self,
            _items_dir: &Path,
            _track_id: &TrackId,
            _rustdoc_workspace: &Path,
            _binding: &TdddLayerBinding,
            _features: &[CargoFeatureName],
        ) -> Result<(), BaselineCaptureIoError> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    let captures = Arc::new(AtomicUsize::new(0));
    let interactor = BaselineCaptureInteractor::new(
        Arc::new(PermissiveSymlinkGuard),
        Arc::new(StubLayerBindings { bindings: vec![stub_binding("domain")] }),
        Arc::new(CountingCapture(Arc::clone(&captures))),
        Arc::new(MissingFeatureDeclaration),
    );
    let workspace = tempfile::tempdir().unwrap();

    let error = interactor.run(valid_request(workspace.path())).unwrap_err();

    assert!(matches!(error, BaselineCaptureError::FeatureDeclaration(_)));
    assert_eq!(captures.load(Ordering::SeqCst), 0, "rustdoc capture must not start");
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
    };

    let err = interactor.run(req).unwrap_err();
    assert!(
        matches!(err, BaselineCaptureError::SymlinkRejected(_)),
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
        Arc::new(EmptyFeatureDeclaration),
    );
    let tmp = tempfile::tempdir().unwrap();
    let source = tempfile::tempdir().unwrap();

    let req = BaselineCaptureRequest {
        track_id: "test-track-2026-01-01".to_owned(),
        workspace_root: tmp.path().to_path_buf(),
        source_workspace: Some(source.path().to_path_buf()),
        layer: None,
    };

    let err = interactor.run(req).unwrap_err();
    assert!(
        matches!(err, BaselineCaptureError::SymlinkRejected(_)),
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
            _track_id: &TrackId,
            rustdoc_workspace: &Path,
            _binding: &TdddLayerBinding,
            _features: &[CargoFeatureName],
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

#[test]
fn test_run_source_workspace_replaces_baseline_without_sync_or_view_side_effects() {
    struct SourceWorkspaceBaselineCapture;

    impl RustdocBaselineCapturePort for SourceWorkspaceBaselineCapture {
        fn capture(
            &self,
            items_dir: &Path,
            track_id: &TrackId,
            rustdoc_workspace: &Path,
            binding: &TdddLayerBinding,
            _features: &[CargoFeatureName],
        ) -> Result<(), BaselineCaptureIoError> {
            let source_baseline = std::fs::read(rustdoc_workspace.join("baseline.json"))
                .map_err(|error| BaselineCaptureIoError(error.to_string()))?;
            let baseline_path = items_dir.join(track_id.as_ref()).join(&binding.baseline_file);
            std::fs::write(baseline_path, source_baseline)
                .map_err(|error| BaselineCaptureIoError(error.to_string()))
        }
    }

    let workspace = tempfile::tempdir().unwrap();
    let source_workspace = tempfile::tempdir().unwrap();
    let track_dir = workspace.path().join("track/items/test-track-2026-01-01");
    std::fs::create_dir_all(&track_dir).unwrap();

    let prior_baseline = b"baseline-before-recovery";
    let recovered_baseline = b"baseline-from-exact-merged-base";
    let prior_sync_base = b"{\"schema_version\":1,\"base_commit\":\"prior\"}";
    let prior_rendered_view = b"# rendered view before recovery\n";
    let baseline_path = track_dir.join("domain-types-baseline.json");
    let sync_base_path = track_dir.join(".sync-base.json");
    let rendered_view_path = track_dir.join("plan.md");
    std::fs::write(&baseline_path, prior_baseline).unwrap();
    std::fs::write(&sync_base_path, prior_sync_base).unwrap();
    std::fs::write(&rendered_view_path, prior_rendered_view).unwrap();
    std::fs::write(source_workspace.path().join("baseline.json"), recovered_baseline).unwrap();

    let interactor = build_interactor(
        Arc::new(StubLayerBindings { bindings: vec![stub_binding("domain")] }),
        Arc::new(SourceWorkspaceBaselineCapture),
    );
    interactor
        .run(BaselineCaptureRequest {
            track_id: "test-track-2026-01-01".to_owned(),
            workspace_root: workspace.path().to_path_buf(),
            source_workspace: Some(source_workspace.path().to_path_buf()),
            layer: None,
        })
        .unwrap();

    assert_eq!(std::fs::read(&baseline_path).unwrap(), recovered_baseline);
    assert_eq!(std::fs::read(&sync_base_path).unwrap(), prior_sync_base);
    assert_eq!(std::fs::read(&rendered_view_path).unwrap(), prior_rendered_view);
}
