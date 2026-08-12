//! Tests for `TypeSignalsInteractor`.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::Path;
use std::sync::{Arc, Mutex};

use crate::tddd_feature_declaration::{
    TdddActualFeatureDeclarationPort, TdddActualFeatureDeclarationPortError,
    TdddFeatureDeclarationReadError,
};
use domain::tddd::catalogue_v2::{TdddLayerBinding, TdddLayerBindingsError, TdddLayerBindingsPort};
use domain::tddd::{CargoFeatureName, LayerId, TdddFeatureDeclaration};
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
        _features: &[domain::tddd::CargoFeatureName],
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
        _features: &[domain::tddd::CargoFeatureName],
    ) -> Result<(), TypeSignalsExecutionError> {
        Err(TypeSignalsExecutionError::Evaluation(crate::git_workflow::DiagnosticText::new(
            "evaluation failed: nightly not installed",
        )))
    }
}

struct StagedFailingExecutor(TypeSignalsExecutionError);

impl TypeSignalsExecutorPort for StagedFailingExecutor {
    fn evaluate_layer(
        &self,
        _items_dir: &Path,
        _track_id: &TrackId,
        _workspace_root: &Path,
        _binding: &TdddLayerBinding,
        _features: &[domain::tddd::CargoFeatureName],
    ) -> Result<(), TypeSignalsExecutionError> {
        Err(self.0.clone())
    }
}

struct TrackIdSpyExecutor {
    seen_track_ids: Arc<Mutex<Vec<TrackId>>>,
}

impl TypeSignalsExecutorPort for TrackIdSpyExecutor {
    fn evaluate_layer(
        &self,
        _items_dir: &Path,
        track_id: &TrackId,
        _workspace_root: &Path,
        _binding: &TdddLayerBinding,
        _features: &[domain::tddd::CargoFeatureName],
    ) -> Result<(), TypeSignalsExecutionError> {
        self.seen_track_ids.lock().unwrap().push(track_id.clone());
        Ok(())
    }
}

struct EmptyFeatureDeclaration;

impl TdddActualFeatureDeclarationPort for EmptyFeatureDeclaration {
    fn load_for_actual(
        &self,
        _track_dir: &Path,
        _workspace_root: &Path,
        layers: &[TdddLayerBinding],
    ) -> Result<TdddFeatureDeclaration, TdddActualFeatureDeclarationPortError> {
        let required_layers = layers
            .iter()
            .map(|binding| LayerId::try_new(binding.layer_id.clone()))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| TdddActualFeatureDeclarationPortError::BaselineSnapshotMismatch)?;
        TdddFeatureDeclaration::try_new(
            required_layers.iter().cloned().map(|layer| (layer, Vec::new())).collect(),
            &required_layers,
        )
        .map_err(|_| TdddActualFeatureDeclarationPortError::BaselineSnapshotMismatch)
    }
}

struct MissingFeatureDeclaration;

impl TdddActualFeatureDeclarationPort for MissingFeatureDeclaration {
    fn load_for_actual(
        &self,
        track_dir: &Path,
        _workspace_root: &Path,
        _layers: &[TdddLayerBinding],
    ) -> Result<TdddFeatureDeclaration, TdddActualFeatureDeclarationPortError> {
        Err(TdddActualFeatureDeclarationPortError::Read(
            TdddFeatureDeclarationReadError::MissingDeclaration {
                path: track_dir.join("tddd-features.json"),
            },
        ))
    }
}

struct SemanticDupFeatureDeclaration;

impl TdddActualFeatureDeclarationPort for SemanticDupFeatureDeclaration {
    fn load_for_actual(
        &self,
        _track_dir: &Path,
        _workspace_root: &Path,
        layers: &[TdddLayerBinding],
    ) -> Result<TdddFeatureDeclaration, TdddActualFeatureDeclarationPortError> {
        let required_layers = layers
            .iter()
            .map(|binding| LayerId::try_new(binding.layer_id.clone()))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| TdddActualFeatureDeclarationPortError::BaselineSnapshotMismatch)?;
        let semantic_dup = CargoFeatureName::try_new("semantic-dup".to_owned())
            .map_err(|_| TdddActualFeatureDeclarationPortError::BaselineSnapshotMismatch)?;
        TdddFeatureDeclaration::try_new(
            required_layers
                .iter()
                .cloned()
                .map(|layer| {
                    let features = if layer.as_ref() == "infrastructure" {
                        vec![semantic_dup.clone()]
                    } else {
                        Vec::new()
                    };
                    (layer, features)
                })
                .collect(),
            &required_layers,
        )
        .map_err(|_| TdddActualFeatureDeclarationPortError::BaselineSnapshotMismatch)
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
    TypeSignalsInteractor::new(layer_bindings, executor, Arc::new(EmptyFeatureDeclaration))
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

#[test]
fn test_new_with_injected_ports_runs_the_requested_layer() {
    let interactor = TypeSignalsInteractor::new(
        Arc::new(StubLayerBindings { bindings: vec![stub_binding("domain")] }),
        Arc::new(SuccessExecutor),
        Arc::new(EmptyFeatureDeclaration),
    );
    let tmp = tempfile::tempdir().unwrap();

    let result = interactor.run(valid_request(tmp.path()));

    assert!(result.is_ok(), "injected ports must be used by the constructed interactor");
}

#[test]
fn test_run_with_absent_feature_declaration_stops_before_executor() {
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct CountingExecutor(Arc<AtomicUsize>);

    impl TypeSignalsExecutorPort for CountingExecutor {
        fn evaluate_layer(
            &self,
            _items_dir: &Path,
            _track_id: &TrackId,
            _workspace_root: &Path,
            _binding: &TdddLayerBinding,
            _features: &[domain::tddd::CargoFeatureName],
        ) -> Result<(), TypeSignalsExecutionError> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    let executions = Arc::new(AtomicUsize::new(0));
    let interactor = TypeSignalsInteractor::new(
        Arc::new(StubLayerBindings { bindings: vec![stub_binding("domain")] }),
        Arc::new(CountingExecutor(Arc::clone(&executions))),
        Arc::new(MissingFeatureDeclaration),
    );
    let tmp = tempfile::tempdir().unwrap();

    let error = interactor.run(valid_request(tmp.path())).unwrap_err();

    assert!(matches!(error, TypeSignalsError::FeatureDeclaration(_)));
    assert_eq!(
        executions.load(Ordering::SeqCst),
        0,
        "missing declarations must stop the canonical path before an executor can launch rustdoc"
    );
}

#[test]
fn test_run_forwards_declared_layer_features_to_executor() {
    struct FeatureSpyExecutor {
        seen_features: Arc<Mutex<Vec<Vec<CargoFeatureName>>>>,
    }

    impl TypeSignalsExecutorPort for FeatureSpyExecutor {
        fn evaluate_layer(
            &self,
            _items_dir: &Path,
            _track_id: &TrackId,
            _workspace_root: &Path,
            _binding: &TdddLayerBinding,
            features: &[CargoFeatureName],
        ) -> Result<(), TypeSignalsExecutionError> {
            self.seen_features.lock().unwrap().push(features.to_vec());
            Ok(())
        }
    }

    let seen_features = Arc::new(Mutex::new(Vec::new()));
    let interactor = TypeSignalsInteractor::new(
        Arc::new(StubLayerBindings { bindings: vec![stub_binding("infrastructure")] }),
        Arc::new(FeatureSpyExecutor { seen_features: Arc::clone(&seen_features) }),
        Arc::new(SemanticDupFeatureDeclaration),
    );
    let tmp = tempfile::tempdir().unwrap();

    interactor.run(valid_request(tmp.path())).unwrap();

    assert_eq!(
        seen_features.lock().unwrap().as_slice(),
        [vec![CargoFeatureName::try_new("semantic-dup".to_owned()).unwrap()]],
        "the canonical executor must receive the layer's frozen feature declaration"
    );
}

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

#[test]
fn test_run_propagates_track_id_to_every_layer_and_rejects_mismatched_active_track() {
    let seen_track_ids = Arc::new(Mutex::new(Vec::new()));
    let interactor = build_interactor(
        Arc::new(StubLayerBindings {
            bindings: vec![stub_binding("domain"), stub_binding("usecase")],
        }),
        Arc::new(TrackIdSpyExecutor { seen_track_ids: Arc::clone(&seen_track_ids) }),
    );
    let tmp = tempfile::tempdir().unwrap();

    let mismatched_request = TypeSignalsRequest {
        items_dir: tmp.path().join("track/items"),
        track_id: TrackId::try_new("test-track-2026-01-01").unwrap(),
        branch: TrackBranch::try_new("track/other-track").unwrap(),
        workspace_root: tmp.path().to_path_buf(),
        layer: None,
    };
    let mismatch = interactor.run(mismatched_request).unwrap_err();
    assert!(matches!(mismatch, TypeSignalsError::BranchTrackMismatch { .. }));
    assert!(
        seen_track_ids.lock().unwrap().is_empty(),
        "the active-track guard must reject a mismatch before any layer evaluation"
    );

    let request = valid_request(tmp.path());
    let expected_track_id = request.track_id.clone();
    interactor.run(request).unwrap();

    assert_eq!(
        seen_track_ids.lock().unwrap().as_slice(),
        [expected_track_id.clone(), expected_track_id].as_slice(),
        "every layer evaluation must receive the request track id"
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
fn test_run_maps_evaluation_executor_failure_to_evaluation_failed() {
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
fn test_run_maps_authoritative_and_cache_write_executor_failures() {
    let tmp = tempfile::tempdir().unwrap();
    let authoritative = build_interactor(
        Arc::new(StubLayerBindings { bindings: vec![stub_binding("domain")] }),
        Arc::new(StagedFailingExecutor(TypeSignalsExecutionError::AuthoritativeInput(
            crate::git_workflow::DiagnosticText::new("baseline unreadable"),
        ))),
    );
    let cache_write = build_interactor(
        Arc::new(StubLayerBindings { bindings: vec![stub_binding("domain")] }),
        Arc::new(StagedFailingExecutor(TypeSignalsExecutionError::CacheWrite(
            crate::git_workflow::DiagnosticText::new("atomic write rejected"),
        ))),
    );

    assert!(matches!(
        authoritative.run(valid_request(tmp.path())),
        Err(TypeSignalsError::AuthoritativeInputFailed { .. })
    ));
    assert!(matches!(
        cache_write.run(valid_request(tmp.path())),
        Err(TypeSignalsError::CacheWriteFailed { .. })
    ));
}

#[test]
fn test_run_maps_evaluation_variant_from_executor_to_layered_failure() {
    let interactor = build_interactor(
        Arc::new(StubLayerBindings { bindings: vec![stub_binding("domain")] }),
        Arc::new(StagedFailingExecutor(TypeSignalsExecutionError::Evaluation(
            crate::git_workflow::DiagnosticText::new("signal evaluator rejected the graph"),
        ))),
    );
    let tmp = tempfile::tempdir().unwrap();

    let error = interactor.run(valid_request(tmp.path())).unwrap_err();

    assert!(matches!(
        error,
        TypeSignalsError::EvaluationFailed { ref layer_id, ref reason }
            if layer_id.as_ref() == "domain" && reason.as_str() == "signal evaluator rejected the graph"
    ));
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
            _features: &[domain::tddd::CargoFeatureName],
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
