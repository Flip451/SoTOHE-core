//! Primary adapter for Track TDDD command contexts.

use std::path::PathBuf;
use std::sync::Arc;

use usecase::track_lifecycle::tddd::baseline_capture::{
    TrackBaselineCaptureCommand, TrackBaselineCaptureError, TrackBaselineCaptureService,
};
use usecase::track_lifecycle::{
    TrackLayerSelection, TrackLifecycleIdInput, TrackSelection, TrackSourceWorkspace,
    TrackWorkspaceRoot,
};

use crate::adr_baseline::TrackIdInput;
use crate::render::CommandOutcome;

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

/// Primary adapter for the Track TDDD command family.
pub struct TrackTdddDriver {
    baseline_capture: Arc<dyn TrackBaselineCaptureService>,
}

impl TrackTdddDriver {
    /// Creates a baseline-capture driver from its application service.
    #[must_use]
    pub fn new(baseline_capture: Arc<dyn TrackBaselineCaptureService>) -> Self {
        Self { baseline_capture }
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

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
mod tests {
    use std::sync::Mutex;

    use super::*;
    use usecase::track_lifecycle::tddd::baseline_capture::TrackBaselineCaptureResult;

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
        let driver = TrackTdddDriver::new(service.clone());

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
        let driver = TrackTdddDriver::new(service.clone());

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
        let driver = TrackTdddDriver::new(service);

        let outcome = driver.handle(TrackTdddBaselineCaptureInput {
            track_id: Some(track_id()),
            workspace_root: workspace_root(),
            source_workspace: None,
            layer: None,
        });

        assert_eq!(outcome.stderr.as_deref(), Some("capture failed"));
        assert_eq!(outcome.exit_code, 1);
    }
}
