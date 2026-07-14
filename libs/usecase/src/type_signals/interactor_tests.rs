//! Tests for `TypeSignalsInteractor`.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::Path;
use std::sync::Arc;

use domain::tddd::catalogue_v2::{TdddLayerBinding, TdddLayerBindingsError, TdddLayerBindingsPort};
use domain::{TrackBranch, TrackId};

use super::super::{
    TypeSignalsError, TypeSignalsExecutionError, TypeSignalsExecutorPort, TypeSignalsRequest,
    TypeSignalsService,
};
use super::TypeSignalsInteractor;

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

struct FailingLayerBindings;

impl TdddLayerBindingsPort for FailingLayerBindings {
    fn load(
        &self,
        _workspace_root: &Path,
        _layer_filter: Option<&str>,
    ) -> Result<Vec<TdddLayerBinding>, TdddLayerBindingsError> {
        Err(TdddLayerBindingsError::LoadFailed { reason: "cannot read rules".to_owned() })
    }
}

struct SuccessExecutor;

impl TypeSignalsExecutorPort for SuccessExecutor {
    fn evaluate_layer(
        &self,
        _items_dir: &Path,
        _track_id: &TrackId,
        _workspace_root: &Path,
        _binding: &TdddLayerBinding,
    ) -> Result<(), TypeSignalsExecutionError> {
        Ok(())
    }
}

struct FailingExecutor;

impl TypeSignalsExecutorPort for FailingExecutor {
    fn evaluate_layer(
        &self,
        _items_dir: &Path,
        _track_id: &TrackId,
        _workspace_root: &Path,
        _binding: &TdddLayerBinding,
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
    layer_bindings: Arc<dyn TdddLayerBindingsPort>,
    executor: Arc<dyn TypeSignalsExecutorPort>,
) -> TypeSignalsInteractor {
    TypeSignalsInteractor::new(layer_bindings, executor)
}

fn valid_request(tmp: &std::path::Path) -> TypeSignalsRequest {
    TypeSignalsRequest {
        items_dir: tmp.join("track/items"),
        track_id: TrackId::try_new("test-track-2026-01-01").unwrap(),
        branch: TrackBranch::try_new("track/test-track-2026-01-01").unwrap(),
        workspace_root: tmp.to_path_buf(),
        layer: None,
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
        Arc::new(StubLayerBindings { bindings: vec![stub_binding("domain")] }),
        Arc::new(SuccessExecutor),
    );
    // workspace_root = "."     → interactor derives items_dir = "./track/items"
    // request items_dir ignored; no InconsistentRequest can fire.
    let req = TypeSignalsRequest {
        items_dir: std::path::PathBuf::from("track/items"),
        track_id: TrackId::try_new("test-track-2026-01-01").unwrap(),
        branch: TrackBranch::try_new("track/test-track-2026-01-01").unwrap(),
        workspace_root: std::path::PathBuf::from("."),
        layer: None,
    };

    let result = interactor.run(req);
    assert!(
        result.is_ok(),
        "items_dir is ignored; workspace_root = '.' must succeed, got: {result:?}"
    );
}

/// CN-07 guard: a valid branch for another track is rejected.
#[test]
fn test_run_rejects_branch_track_id_mismatch() {
    let interactor = build_interactor(
        Arc::new(StubLayerBindings { bindings: vec![stub_binding("domain")] }),
        Arc::new(SuccessExecutor),
    );
    let tmp = tempfile::tempdir().unwrap();
    let req = TypeSignalsRequest {
        items_dir: tmp.path().join("track/items"),
        track_id: TrackId::try_new("test-track-2026-01-01").unwrap(),
        branch: TrackBranch::try_new("track/other-track").unwrap(),
        workspace_root: tmp.path().to_path_buf(),
        layer: None,
    };

    let err = interactor.run(req).unwrap_err();
    assert!(
        matches!(
            err,
            TypeSignalsError::BranchTrackMismatch { ref branch, ref track_id }
                if branch.as_ref() == "track/other-track" && track_id.as_ref() == "test-track-2026-01-01"
        ),
        "branch/track-id mismatch must return BranchTrackMismatch, got: {err:?}"
    );
}

/// CN-07 guard: a Done track is allowed when the current branch matches `track/<id>`.
#[test]
fn test_run_allows_done_track_on_matching_branch() {
    // The CN-07 guard checks branch, not track status. A Done track on its own
    // branch must be allowed through the guard so type-signals can render.
    let interactor = build_interactor(
        Arc::new(StubLayerBindings { bindings: vec![stub_binding("domain")] }),
        Arc::new(SuccessExecutor),
    );
    let tmp = tempfile::tempdir().unwrap();
    let req = valid_request(tmp.path()); // branch = "track/test-track-2026-01-01"

    let result = interactor.run(req);
    assert!(
        result.is_ok(),
        "a matching branch must pass CN-07 regardless of track status, got: {result:?}"
    );
}

#[test]
fn test_run_with_no_layers_returns_error() {
    let interactor = build_interactor(Arc::new(NoLayersBindings), Arc::new(SuccessExecutor));
    let tmp = tempfile::tempdir().unwrap();
    let req = valid_request(tmp.path());

    let err = interactor.run(req).unwrap_err();
    assert!(
        matches!(err, TypeSignalsError::NoLayers),
        "no layers must return NoLayers error, got: {err:?}"
    );
}

#[test]
fn test_run_with_layer_bindings_load_failure_preserves_diagnostic_text() {
    let interactor = build_interactor(Arc::new(FailingLayerBindings), Arc::new(SuccessExecutor));
    let tmp = tempfile::tempdir().unwrap();

    let err = interactor.run(valid_request(tmp.path())).unwrap_err();
    assert!(
        matches!(err, TypeSignalsError::LayerBindingsLoad { ref reason } if reason.as_str() == "cannot read rules"),
        "layer bindings load failure must preserve its diagnostic text, got: {err:?}"
    );
}

#[test]
fn test_run_with_failing_executor_returns_error() {
    let interactor = build_interactor(
        Arc::new(StubLayerBindings { bindings: vec![stub_binding("domain")] }),
        Arc::new(FailingExecutor),
    );
    let tmp = tempfile::tempdir().unwrap();
    let req = valid_request(tmp.path());

    let err = interactor.run(req).unwrap_err();
    assert!(
        matches!(err, TypeSignalsError::EvaluationFailed { ref layer_id, ref reason }
            if layer_id.as_ref() == "domain" && reason.as_str() == "evaluation failed: nightly not installed"),
        "evaluation failure must return EvaluationFailed, got: {err:?}"
    );
}

#[test]
fn test_run_with_invalid_layer_binding_preserves_diagnostic_text() {
    let interactor = build_interactor(
        Arc::new(StubLayerBindings { bindings: vec![stub_binding("invalid layer")] }),
        Arc::new(SuccessExecutor),
    );
    let tmp = tempfile::tempdir().unwrap();

    let err = interactor.run(valid_request(tmp.path())).unwrap_err();
    assert!(
        matches!(err, TypeSignalsError::InconsistentRequest { ref reason }
            if reason.as_str().starts_with("layer binding contains an invalid layer id:")),
        "invalid layer binding must return an explanatory diagnostic, got: {err:?}"
    );
}

#[test]
fn test_run_with_success_returns_ok() {
    let interactor = build_interactor(
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
            _track_id: &TrackId,
            _workspace_root: &Path,
            _binding: &TdddLayerBinding,
        ) -> Result<(), TypeSignalsExecutionError> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    let count = Arc::new(AtomicUsize::new(0));
    let interactor = build_interactor(
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
