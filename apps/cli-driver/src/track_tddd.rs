//! Primary adapter for Track TDDD command contexts.

use std::path::PathBuf;
use std::sync::Arc;

use usecase::track_lifecycle::tddd::baseline_capture::{
    TrackBaselineCaptureCommand, TrackBaselineCaptureError, TrackBaselineCaptureService,
};
use usecase::track_lifecycle::tddd::baseline_graph::{
    TrackBaselineGraphCommand, TrackBaselineGraphError, TrackBaselineGraphService,
};
use usecase::track_lifecycle::{
    TrackItemsDirectory, TrackLayerFilter, TrackLayerSelection, TrackLifecycleIdInput,
    TrackSelection, TrackSourceWorkspace, TrackWorkspaceRoot,
};

use crate::adr_baseline::TrackIdInput;
use crate::render::CommandOutcome;
use crate::track_resolution::TrackResolutionDiagnostic;

/// Validated `track/items` input for a TDDD command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackItemsDirectoryInput {
    value: PathBuf,
}

impl TrackItemsDirectoryInput {
    /// Validates and wraps a `track/items` path.
    pub fn try_new(value: PathBuf) -> Result<Self, TrackResolutionDiagnostic> {
        TrackItemsDirectory::try_new(value.clone()).map_err(|error| {
            if error.to_string().contains("must end in 'track/items'") {
                TrackResolutionDiagnostic::new(format!(
                    "--items-dir must point to '<project-root>/track/items'; got {}",
                    value.display()
                ))
            } else {
                TrackResolutionDiagnostic::new(error.to_string())
            }
        })?;
        Ok(Self { value })
    }

    /// Derives the workspace-root input from this items directory.
    #[must_use]
    pub fn workspace_root(&self) -> TrackWorkspaceRootInput {
        let root = self
            .value
            .parent()
            .and_then(std::path::Path::parent)
            .map(std::path::Path::to_path_buf)
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or_else(|| PathBuf::from("."));
        TrackWorkspaceRootInput { value: root }
    }

    fn into_usecase(self) -> Result<TrackItemsDirectory, TrackResolutionDiagnostic> {
        TrackItemsDirectory::try_new(self.value)
            .map_err(|error| TrackResolutionDiagnostic::new(error.to_string()))
    }
}

/// Validated comma-separated TDDD layer input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackLayersInput {
    value: Vec<usecase::LayerId>,
}

impl TrackLayersInput {
    /// Parses and validates a comma-separated layer list.
    pub fn try_new(value: String) -> Result<Self, TrackResolutionDiagnostic> {
        let mut layers = Vec::new();
        for token in value.split(',') {
            let trimmed = token.trim();
            if trimmed.is_empty() {
                continue;
            }
            let layer = usecase::LayerId::try_new(trimmed.to_owned()).map_err(|error| {
                TrackResolutionDiagnostic::new(format!("invalid layer id '{trimmed}': {error}"))
            })?;
            layers.push(layer);
        }
        Ok(Self { value: layers })
    }

    fn into_usecase(self) -> TrackLayerFilter {
        TrackLayerFilter::Selected(self.value)
    }
}

/// Validated workspace-root input for a TDDD command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackWorkspaceRootInput {
    value: PathBuf,
}

impl TryFrom<PathBuf> for TrackWorkspaceRootInput {
    type Error = String;

    fn try_from(value: PathBuf) -> Result<Self, Self::Error> {
        TrackWorkspaceRoot::try_new(value.clone())
            .map(|_| Self { value })
            .map_err(|error| error.to_string())
    }
}

impl TrackWorkspaceRootInput {
    fn into_usecase(self) -> Result<TrackWorkspaceRoot, String> {
        TrackWorkspaceRoot::try_new(self.value).map_err(|error| error.to_string())
    }
}

/// Validated rustdoc source-workspace input for a TDDD command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackSourceWorkspaceInput {
    value: PathBuf,
}

impl TryFrom<PathBuf> for TrackSourceWorkspaceInput {
    type Error = String;

    fn try_from(value: PathBuf) -> Result<Self, Self::Error> {
        TrackSourceWorkspace::try_new(value.clone())
            .map(|_| Self { value })
            .map_err(|error| error.to_string())
    }
}

impl TrackSourceWorkspaceInput {
    fn into_usecase(self) -> Result<TrackSourceWorkspace, String> {
        TrackSourceWorkspace::try_new(self.value).map_err(|error| error.to_string())
    }
}

/// Validated TDDD layer input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackLayerInput {
    value: usecase::LayerId,
}

impl TryFrom<String> for TrackLayerInput {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        usecase::LayerId::try_new(value)
            .map(|value| Self { value })
            .map_err(|error| error.to_string())
    }
}

impl TrackLayerInput {
    fn into_usecase(self) -> usecase::LayerId {
        self.value
    }
}

/// Typed primary-adapter input for TDDD baseline capture.
#[derive(Debug, PartialEq, Eq)]
pub struct TrackTdddBaselineCaptureInput {
    /// Optional explicit track id; omitted values select the active track.
    pub track_id: Option<TrackIdInput>,
    /// Workspace containing the track artifacts.
    pub workspace_root: TrackWorkspaceRootInput,
    /// Optional workspace from which rustdoc is run.
    pub source_workspace: Option<TrackSourceWorkspaceInput>,
    /// Optional layer filter.
    pub layer: Option<TrackLayerInput>,
}

/// Typed primary-adapter input for TDDD baseline-graph rendering.
#[derive(Debug, PartialEq, Eq)]
pub struct TrackTdddBaselineGraphInput {
    /// Optional explicit track id; omitted values select the active track.
    pub track_id: Option<TrackIdInput>,
    /// Directory containing the track items.
    pub items_dir: TrackItemsDirectoryInput,
    /// Workspace containing the track artifacts and graph configuration.
    pub workspace_root: TrackWorkspaceRootInput,
    /// Optional comma-separated layer filter.
    pub layers: Option<TrackLayersInput>,
}

/// Primary adapter for the Track TDDD command family.
pub struct TrackTdddDriver {
    baseline_capture: Arc<dyn TrackBaselineCaptureService>,
    baseline_graph: Arc<dyn TrackBaselineGraphService>,
}

impl TrackTdddDriver {
    /// Creates a baseline-capture driver from its application service.
    #[must_use]
    pub fn new(
        baseline_capture: Arc<dyn TrackBaselineCaptureService>,
        baseline_graph: Arc<dyn TrackBaselineGraphService>,
    ) -> Self {
        Self { baseline_capture, baseline_graph }
    }

    /// Executes the baseline-capture input boundary.
    pub fn handle(&self, input: TrackTdddBaselineCaptureInput) -> CommandOutcome {
        let command = match input_to_command(input) {
            Ok(command) => command,
            Err(error) => return CommandOutcome::failure(Some(error)),
        };
        self.baseline_capture
            .execute(command)
            .map(|_| CommandOutcome::success(None))
            .unwrap_or_else(baseline_capture_error_to_outcome)
    }

    /// Executes the baseline-graph input boundary.
    pub fn handle_baseline_graph(&self, input: TrackTdddBaselineGraphInput) -> CommandOutcome {
        let command = match baseline_graph_input_to_command(input) {
            Ok(command) => command,
            Err(error) => return CommandOutcome::failure(Some(error)),
        };
        self.baseline_graph
            .execute(command)
            .map(render_baseline_graph_result)
            .unwrap_or_else(baseline_graph_error_to_outcome)
    }
}

fn input_to_command(
    input: TrackTdddBaselineCaptureInput,
) -> Result<TrackBaselineCaptureCommand, String> {
    let track = input
        .track_id
        .map(|track_id| TrackLifecycleIdInput::try_new(track_id.to_string()))
        .transpose()
        .map_err(|error| error.to_string())
        .map(TrackSelection::from_input)?;
    let workspace_root = input.workspace_root.into_usecase()?;
    let source_workspace =
        input.source_workspace.map(TrackSourceWorkspaceInput::into_usecase).transpose()?;
    let layer = input
        .layer
        .map(|layer| TrackLayerSelection::One(layer.into_usecase()))
        .unwrap_or(TrackLayerSelection::All);
    Ok(TrackBaselineCaptureCommand { track, workspace_root, source_workspace, layer })
}

fn baseline_capture_error_to_outcome(error: TrackBaselineCaptureError) -> CommandOutcome {
    CommandOutcome::failure(Some(error.to_string()))
}

fn baseline_graph_input_to_command(
    input: TrackTdddBaselineGraphInput,
) -> Result<TrackBaselineGraphCommand, String> {
    let track = input
        .track_id
        .map(|track_id| TrackLifecycleIdInput::try_new(track_id.to_string()))
        .transpose()
        .map_err(|error| error.to_string())
        .map(TrackSelection::from_input)?;
    let items_dir = input.items_dir.into_usecase().map_err(|error| error.to_string())?;
    let workspace_root = input.workspace_root.into_usecase()?;
    let layers = input.layers.map(TrackLayersInput::into_usecase).unwrap_or(TrackLayerFilter::All);
    Ok(TrackBaselineGraphCommand { track, items_dir, workspace_root, layers })
}

fn render_baseline_graph_result(
    result: usecase::track_lifecycle::tddd::baseline_graph::TrackBaselineGraphResult,
) -> CommandOutcome {
    CommandOutcome::success(Some(format!(
        "[OK] baseline-graph: wrote depth-1 overview + depth-2 cluster files for track '{}' \
         (layers={}, files={})",
        result.track_id,
        result.rendered_layers.value(),
        result.written_files.value(),
    )))
}

fn baseline_graph_error_to_outcome(error: TrackBaselineGraphError) -> CommandOutcome {
    CommandOutcome::failure(Some(error.to_string()))
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
mod tests {
    use std::sync::Mutex;

    use super::*;
    use usecase::track_lifecycle::tddd::baseline_capture::TrackBaselineCaptureResult;
    use usecase::track_lifecycle::tddd::baseline_graph::TrackBaselineGraphResult;
    use usecase::track_lifecycle::{TrackRenderedLayerCount, TrackWrittenFileCount};

    #[test]
    fn test_track_items_directory_input_try_new_rejects_noncanonical_path() {
        let error = TrackItemsDirectoryInput::try_new(PathBuf::from("items"))
            .expect_err("noncanonical items dir must fail");
        assert!(error.to_string().contains("--items-dir must point to"));
    }

    #[test]
    fn test_track_path_inputs_reject_parent_relative_paths_without_filesystem_access() {
        let items =
            TrackItemsDirectoryInput::try_new(PathBuf::from("/workspace/anchor/../track/items"));
        assert!(items.is_err());

        let workspace = TrackWorkspaceRootInput::try_from(PathBuf::from("/workspace/anchor/.."));
        assert!(workspace.is_err());
    }

    struct UnusedBaselineGraphService;

    impl TrackBaselineGraphService for UnusedBaselineGraphService {
        fn execute(
            &self,
            _: TrackBaselineGraphCommand,
        ) -> Result<TrackBaselineGraphResult, TrackBaselineGraphError> {
            Err(TrackBaselineGraphError::ExecutionFailed(
                usecase::git_workflow::DiagnosticText::new("baseline graph service unused"),
            ))
        }
    }

    struct RecordingService {
        commands: Mutex<Vec<TrackBaselineCaptureCommand>>,
        result: Result<TrackBaselineCaptureResult, TrackBaselineCaptureError>,
    }

    impl TrackBaselineCaptureService for RecordingService {
        fn execute(
            &self,
            command: TrackBaselineCaptureCommand,
        ) -> Result<TrackBaselineCaptureResult, TrackBaselineCaptureError> {
            self.commands.lock().expect("command lock is available").push(command);
            match &self.result {
                Ok(result) => Ok(result.clone()),
                Err(error) => Err(TrackBaselineCaptureError::ExecutionFailed(
                    usecase::git_workflow::DiagnosticText::new(error.to_string()),
                )),
            }
        }
    }

    struct RecordingBaselineGraphService {
        commands: Mutex<Vec<TrackBaselineGraphCommand>>,
        result: Result<(usize, usize), String>,
    }

    impl TrackBaselineGraphService for RecordingBaselineGraphService {
        fn execute(
            &self,
            command: TrackBaselineGraphCommand,
        ) -> Result<TrackBaselineGraphResult, TrackBaselineGraphError> {
            let track_id = match &command.track {
                TrackSelection::Explicit(track_id) => track_id.clone(),
                TrackSelection::Active => {
                    domain::TrackId::try_new("graph-track").expect("track id is valid")
                }
            };
            self.commands.lock().expect("command lock is available").push(command);
            match &self.result {
                Ok((rendered_layers, written_files)) => Ok(TrackBaselineGraphResult {
                    track_id,
                    rendered_layers: TrackRenderedLayerCount::new(*rendered_layers),
                    written_files: TrackWrittenFileCount::new(*written_files),
                }),
                Err(error) => Err(TrackBaselineGraphError::ExecutionFailed(
                    usecase::git_workflow::DiagnosticText::new(error),
                )),
            }
        }
    }

    fn workspace_root() -> TrackWorkspaceRootInput {
        TrackWorkspaceRootInput::try_from(PathBuf::from("workspace"))
            .expect("workspace root is valid")
    }

    fn track_id() -> TrackIdInput {
        "capture-track".parse().expect("track id is valid")
    }

    #[test]
    fn test_track_tddd_driver_valid_baseline_capture_input_returns_success() {
        let service = Arc::new(RecordingService {
            commands: Mutex::new(Vec::new()),
            result: Ok(TrackBaselineCaptureResult { layers: vec![] }),
        });
        let driver = TrackTdddDriver::new(service.clone(), Arc::new(UnusedBaselineGraphService));

        let outcome = driver.handle(TrackTdddBaselineCaptureInput {
            track_id: Some(track_id()),
            workspace_root: workspace_root(),
            source_workspace: None,
            layer: Some(TrackLayerInput::try_from("usecase".to_owned()).expect("layer is valid")),
        });

        assert_eq!(outcome.exit_code, 0);
        assert_eq!(outcome.stdout, None);
        let commands = service.commands.lock().expect("command lock is available");
        assert_eq!(commands.len(), 1);
        let command = commands.first().expect("driver forwards one command");
        assert!(matches!(
            &command.track,
            TrackSelection::Explicit(track_id) if track_id.as_ref() == "capture-track"
        ));
        assert_eq!(command.workspace_root.as_path(), std::path::Path::new("workspace"));
        assert_eq!(command.source_workspace.as_ref().map(|workspace| workspace.as_path()), None);
        assert!(matches!(
            &command.layer,
            TrackLayerSelection::One(layer) if layer.as_ref() == "usecase"
        ));
    }

    #[test]
    fn test_track_tddd_driver_invalid_workspace_input_returns_failure_without_service_call() {
        let service = Arc::new(RecordingService {
            commands: Mutex::new(Vec::new()),
            result: Ok(TrackBaselineCaptureResult { layers: vec![] }),
        });
        let driver = TrackTdddDriver::new(service.clone(), Arc::new(UnusedBaselineGraphService));

        let input = TrackTdddBaselineCaptureInput {
            track_id: None,
            workspace_root: TrackWorkspaceRootInput { value: PathBuf::from("../escape") },
            source_workspace: None,
            layer: None,
        };
        let outcome = driver.handle(input);

        assert_eq!(outcome.exit_code, 1);
        assert!(service.commands.lock().expect("command lock is available").is_empty());
    }

    #[test]
    fn test_track_tddd_driver_service_error_maps_to_failure_outcome() {
        let service = Arc::new(RecordingService {
            commands: Mutex::new(Vec::new()),
            result: Err(TrackBaselineCaptureError::ExecutionFailed(
                usecase::git_workflow::DiagnosticText::new("capture failed"),
            )),
        });
        let driver = TrackTdddDriver::new(service, Arc::new(UnusedBaselineGraphService));

        let outcome = driver.handle(TrackTdddBaselineCaptureInput {
            track_id: Some(track_id()),
            workspace_root: workspace_root(),
            source_workspace: None,
            layer: None,
        });

        assert_eq!(outcome.stderr.as_deref(), Some("capture failed"));
        assert_eq!(outcome.exit_code, 1);
    }

    fn graph_input() -> TrackTdddBaselineGraphInput {
        TrackTdddBaselineGraphInput {
            track_id: Some("graph-track".parse().expect("track id is valid")),
            items_dir: TrackItemsDirectoryInput::try_new(PathBuf::from("workspace/track/items"))
                .expect("items directory is valid"),
            workspace_root: workspace_root(),
            layers: Some(
                TrackLayersInput::try_new("domain, usecase".to_owned())
                    .expect("layer list is valid"),
            ),
        }
    }

    #[test]
    fn test_track_tddd_items_directory_input_rejects_noncanonical_path() {
        let result = TrackItemsDirectoryInput::try_new(PathBuf::from("fixture/items"));

        assert_eq!(
            result.unwrap_err().message(),
            "--items-dir must point to '<project-root>/track/items'; got fixture/items"
        );
    }

    #[test]
    fn test_track_layers_input_invalid_layer_preserves_legacy_diagnostic_prefix() {
        let inner_error = usecase::LayerId::try_new("not a layer".to_owned())
            .expect_err("invalid layer fixture must fail");
        let error = TrackLayersInput::try_new("not a layer".to_owned())
            .expect_err("invalid layer input must fail");

        assert_eq!(error.to_string(), format!("invalid layer id 'not a layer': {inner_error}"));
    }

    #[test]
    fn test_track_tddd_driver_valid_baseline_graph_input_returns_render_summary() {
        let service = Arc::new(RecordingBaselineGraphService {
            commands: Mutex::new(Vec::new()),
            result: Ok((2, 5)),
        });
        let driver = TrackTdddDriver::new(
            Arc::new(RecordingService {
                commands: Mutex::new(Vec::new()),
                result: Ok(TrackBaselineCaptureResult { layers: vec![] }),
            }),
            service.clone(),
        );

        let outcome = driver.handle_baseline_graph(graph_input());

        assert_eq!(outcome.exit_code, 0);
        assert_eq!(
            outcome.stdout.as_deref(),
            Some(
                "[OK] baseline-graph: wrote depth-1 overview + depth-2 cluster files for track 'graph-track' (layers=2, files=5)"
            )
        );
        let commands = service.commands.lock().expect("command lock is available");
        assert_eq!(commands.len(), 1);
        let command = commands.first().expect("driver forwards one command");
        assert_eq!(command.items_dir.as_path(), std::path::Path::new("workspace/track/items"));
        assert!(matches!(
            &command.track,
            TrackSelection::Explicit(track_id) if track_id.as_ref() == "graph-track"
        ));
        assert!(matches!(
            &command.layers,
            TrackLayerFilter::Selected(layers)
                if layers.iter().map(|layer| layer.as_ref()).collect::<Vec<_>>()
                    == ["domain", "usecase"]
        ));
    }

    #[test]
    fn test_track_tddd_driver_invalid_baseline_graph_items_dir_returns_failure_without_service_call()
     {
        let service = Arc::new(RecordingBaselineGraphService {
            commands: Mutex::new(Vec::new()),
            result: Ok((1, 1)),
        });
        let driver = TrackTdddDriver::new(
            Arc::new(RecordingService {
                commands: Mutex::new(Vec::new()),
                result: Ok(TrackBaselineCaptureResult { layers: vec![] }),
            }),
            service.clone(),
        );
        let input = TrackTdddBaselineGraphInput {
            track_id: None,
            items_dir: TrackItemsDirectoryInput { value: PathBuf::from("../escape") },
            workspace_root: workspace_root(),
            layers: None,
        };

        let outcome = driver.handle_baseline_graph(input);

        assert_eq!(outcome.exit_code, 1);
        assert!(service.commands.lock().expect("command lock is available").is_empty());
    }

    #[test]
    fn test_track_tddd_driver_baseline_graph_service_error_maps_to_failure_outcome() {
        let service = Arc::new(RecordingBaselineGraphService {
            commands: Mutex::new(Vec::new()),
            result: Err("graph failed".to_owned()),
        });
        let driver = TrackTdddDriver::new(
            Arc::new(RecordingService {
                commands: Mutex::new(Vec::new()),
                result: Ok(TrackBaselineCaptureResult { layers: vec![] }),
            }),
            service,
        );

        let outcome = driver.handle_baseline_graph(graph_input());

        assert_eq!(outcome.stderr.as_deref(), Some("graph failed"));
        assert_eq!(outcome.exit_code, 1);
    }
}
